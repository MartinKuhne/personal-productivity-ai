//! Sidecar unit tests for `ui_iced/about.rs`.
//!
//! Pure view-model, state-transition and widget-construction tests
//! ([RUST-003]). No renderer, no bus, no filesystem: every fixture is a
//! synthetic literal that is never opened, and the only real data read is
//! the compile-time build metadata and the `include_str!`-embedded license
//! ([RUST-006]).

use super::*;
use crate::ui::strings;
use iced::Element;

/// A fixture snapshot with known build metadata.
fn known_vm() -> AboutViewModel {
    AboutViewModel {
        app_name: strings::ABOUT_APP_NAME,
        version: "9.8.7",
        description: "a fixture description",
        copyright: strings::ABOUT_COPYRIGHT,
        branch: "feature/fixture",
        commit_hash: "0123456789abcdef0123456789abcdef01234567",
        commit_short_hash: "01234567",
        build_date: "2026-10-18",
        license_text: "MIT License\n\nPermission is hereby granted.\n\nThe rest.".to_string(),
        attribution_count: 3,
    }
}

/// A fixture snapshot as `build.rs` produces it when git is unavailable.
fn unknown_vm() -> AboutViewModel {
    AboutViewModel {
        branch: UNKNOWN_BUILD_METADATA,
        commit_hash: UNKNOWN_BUILD_METADATA,
        commit_short_hash: UNKNOWN_BUILD_METADATA,
        build_date: UNKNOWN_BUILD_METADATA,
        ..known_vm()
    }
}

// --- View-model: identity carried through construction ------------------

#[test]
fn from_build_carries_the_shared_identity_strings() {
    let vm = AboutViewModel::from_build();
    assert_eq!(vm.app_name, strings::ABOUT_APP_NAME);
    assert_eq!(vm.copyright, strings::ABOUT_COPYRIGHT);
    assert_eq!(vm.description, strings::ABOUT_DESCRIPTION);
    assert_eq!(vm.version, APP_VERSION);
}

#[test]
fn from_build_version_looks_like_a_version() {
    let vm = AboutViewModel::from_build();
    assert!(
        vm.version.contains('.') && !vm.version.starts_with('.') && !vm.version.ends_with('.'),
        "CARGO_PKG_VERSION should be a dotted version, got {:?}",
        vm.version
    );
    assert!(
        vm.version
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c.is_ascii_alphabetic()),
        "version should carry only version characters, got {:?}",
        vm.version
    );
}

#[test]
fn from_build_description_is_not_blank() {
    let vm = AboutViewModel::from_build();
    assert!(
        !vm.description.trim().is_empty(),
        "the manifest description must not be empty"
    );
}

#[test]
fn from_build_build_metadata_is_present() {
    let vm = AboutViewModel::from_build();
    for value in [
        vm.branch,
        vm.commit_hash,
        vm.commit_short_hash,
        vm.build_date,
    ] {
        assert!(!value.is_empty(), "build.rs always emits a non-empty value");
    }
    // Data-model invariant: the short hash is the leading run of the full
    // hash, or both fall back to "unknown" (spec FR-007).
    if vm.is_build_metadata_unknown() {
        assert_eq!(vm.commit_short_hash, UNKNOWN_BUILD_METADATA);
    } else {
        assert!(
            vm.commit_hash.starts_with(vm.commit_short_hash),
            "short hash {:?} should prefix full hash {:?}",
            vm.commit_short_hash,
            vm.commit_hash
        );
    }
}

#[test]
fn from_build_license_is_the_reflowed_mit_body() {
    let vm = AboutViewModel::from_build();
    assert!(
        vm.license_text.contains("MIT License"),
        "license body should carry the MIT header"
    );
    assert!(
        vm.license_text.contains("Permission is hereby granted"),
        "license body should carry the grant paragraph"
    );
    // Reflow contract: paragraph breaks survive, single newlines do not.
    let singles = vm.license_text.replace("\n\n", "").matches('\n').count();
    assert_eq!(singles, 0, "no hard-wrap single newlines may remain");
    assert!(
        vm.license_text.matches("\n\n").count() > 1,
        "paragraph breaks must survive the reflow"
    );
}

#[test]
fn from_build_attribution_count_matches_the_reused_catalog() {
    let vm = AboutViewModel::from_build();
    assert_eq!(vm.attribution_count, DIRECT_DEPENDENCIES.len());
    assert!(vm.attribution_count > 0, "the catalog is not empty");
}

// --- View-model: derived text -------------------------------------------

#[test]
fn version_line_uses_the_label_and_the_version() {
    let vm = known_vm();
    assert_eq!(
        vm.version_line(),
        format!("{} 9.8.7", strings::ABOUT_VERSION_LABEL)
    );
}

#[test]
fn commit_tooltip_reveals_full_hash_and_hint() {
    // The reveal text carries the full hash, the centralised prefix and the
    // copy hint.
    let tooltip = known_vm().commit_tooltip();
    assert!(tooltip.contains("0123456789abcdef0123456789abcdef01234567"));
    assert!(tooltip.contains(strings::ABOUT_FULL_COMMIT_PREFIX));
    assert!(tooltip.contains(strings::ABOUT_COPY_COMMIT_TOOLTIP));
}

#[test]
fn attributions_summary_uses_the_shared_formatter() {
    assert_eq!(
        known_vm().attributions_summary(),
        strings::format_attribution_count(3)
    );
}

// --- Copy affordance gating (spec FR-007) --------------------------------

#[test]
fn copy_press_is_offered_when_the_hash_is_known() {
    assert_eq!(
        commit_copy_press(false),
        Some(Message::About(AboutMessage::CopyCommitHashRequested))
    );
}

#[test]
fn copy_press_is_withheld_on_an_unknown_build() {
    // Nothing meaningful to copy, so the cell must not emit anything.
    assert_eq!(commit_copy_press(true), None);
    assert!(unknown_vm().is_build_metadata_unknown());
    assert!(!known_vm().is_build_metadata_unknown());
}

// --- Banner -------------------------------------------------------------

#[test]
fn banner_glyph_is_the_shared_phosphor_bolt() {
    // The `image`/`svg` iced features are not compiled in, so
    // the banner is the same Phosphor stand-in the top bar uses.
    assert_eq!(banner_glyph(), strings::ICON_LIGHTNING);
    let _mark: Element<'static, Message> = banner_mark();
}

// --- Card construction ---------------------------------------------------

/// A viewport wide enough that no gutter clamp bites.
const ROOMY_VIEWPORT: Size = Size::new(1280.0, 800.0);

#[test]
fn about_card_builds_for_known_and_unknown_metadata() {
    let known = known_vm();
    let unknown = unknown_vm();
    let known_card: Element<'_, Message> = about_card(&known, ROOMY_VIEWPORT);
    let unknown_card: Element<'_, Message> = about_card(&unknown, ROOMY_VIEWPORT);
    // Constructing both states must not panic; the unknown build differs
    // only in text and press handlers, never in widget count.
    let _ = (known_card, unknown_card);
}

#[test]
fn about_card_builds_from_the_build_snapshot() {
    let vm = AboutViewModel::from_build();
    let _card: Element<'_, Message> = vm.view(ROOMY_VIEWPORT);
}

#[test]
fn footer_and_close_button_build() {
    let _close: Element<'static, Message> = close_button();
    let _footer: Element<'static, Message> = about_footer();
}

// --- Sizing: the framework owns the box (§4.6.2, INT-5a) -----------------

#[test]
fn about_card_width_is_the_frameworks_about_width() {
    // The card width is `ModalSizing::about()`'s figure, read through the
    // framework — not a number this module owns. The retired app value was
    // 620px; the spec row is 440px, so this assertion is the pin that
    // keeps the 180px reduction intentional rather than accidental.
    assert_eq!(
        about_card_width(ROOMY_VIEWPORT.width),
        hoarfrost::surface::modal::ABOUT_WIDTH
    );
    assert_eq!(about_card_width(ROOMY_VIEWPORT.width), 440.0);
    assert_ne!(
        about_card_width(ROOMY_VIEWPORT.width),
        620.0,
        "the retired ABOUT_CARD_WIDTH must not come back"
    );
}

#[test]
fn about_card_width_clamps_to_the_viewport_gutter() {
    // The clamp bites only once the viewport is too narrow to hold 440px
    // plus the gutter: below 472px the card gives way to the viewport and
    // never crosses the gutter the framework declares.
    let gutter = hoarfrost::surface::modal::VIEWPORT_GUTTER;
    assert_eq!(about_card_width(440.0 + gutter), 440.0, "exactly roomy");
    assert_eq!(about_card_width(400.0), 400.0 - gutter, "clamped");
    assert_eq!(about_card_width(300.0), 300.0 - gutter, "clamped harder");
    assert!(
        about_card_width(300.0) <= 300.0 - gutter,
        "a narrow viewport must never produce a card that crosses the gutter"
    );
}

#[test]
fn about_card_width_never_goes_negative() {
    // A viewport narrower than the gutter still resolves to a usable
    // (zero) width rather than a negative rectangle.
    assert_eq!(about_card_width(10.0), 0.0);
    assert_eq!(about_card_width(0.0), 0.0);
}

#[test]
fn about_card_builds_at_every_clamped_width() {
    // The card must construct at a hairline viewport as well as a roomy
    // one — the gutter clamp must not leave the layout unable to build.
    let vm = known_vm();
    for width in [1.0, 32.0, 440.0, 1280.0, 4096.0] {
        let viewport = Size::new(width, 800.0);
        let _card: Element<'_, Message> = about_card(&vm, viewport);
    }
}

#[test]
fn each_window_names_its_own_sizing_policy() {
    assert_eq!(
        AboutWindow::About.sizing(),
        hoarfrost::surface::modal::ModalSizing::about()
    );
    assert_eq!(
        AboutWindow::Attributions.sizing(),
        hoarfrost::surface::modal::ModalSizing::attributions()
    );
    assert_ne!(
        AboutWindow::About.sizing(),
        AboutWindow::Attributions.sizing(),
        "the two windows are different §4.6.2 rows and must not share a policy"
    );
}

#[test]
fn banner_uses_the_brand_mark_icon_rung() {
    // The banner glyph is sized by `size.icon.lg`, the token whose
    // declared role is the brand mark. The retired value was 48px, which
    // no token covered; 20px is the nearest real rung.
    assert_eq!(banner_size(), hoarfrost::token::size::ICON_LG);
    assert_eq!(banner_size(), 20.0);
}

// --- Rendered geometry: measured in a headless layout -----------------
//
// The assertions above read the policy. These ones lay the card out in a
// real iced `UserInterface` and measure what came back, because "the card
// is 440px wide" is only worth anything if the widget tree actually
// produces 440px — and because the 620→440 change is large enough that
// the licence body, heading and metadata row have to be shown not to
// overflow or clip at the new width.

use iced_test::selector::{Candidate, Target};

type Sim<'a> = iced_test::Simulator<'a, Message, iced::Theme, iced::Renderer>;

/// Lays `element` out in a window of `viewport` size, headless.
fn laid_out(viewport: Size, element: Element<'_, Message>) -> Sim<'_> {
    iced_test::Simulator::with_size(iced_test::core::Settings::default(), viewport, element)
}

/// The laid-out bounds of the framed card box carrying `id`.
fn framed_card_bounds(ui: &mut Sim<'_>, id: &'static str) -> iced::Rectangle {
    ui.find(iced::widget::Id::from(id))
        .unwrap_or_else(|err| panic!("card container {id:?} should be reachable: {err:?}"))
        .bounds()
}

/// The laid-out viewport and content bounds of the scrollable carrying `id`.
fn scroll_bounds(ui: &mut Sim<'_>, id: &'static str) -> (iced::Rectangle, iced::Rectangle) {
    let target = ui
        .find(iced::widget::Id::from(id))
        .unwrap_or_else(|err| panic!("scrollable {id:?} should be reachable: {err:?}"));
    match target {
        Target::Scrollable {
            bounds,
            content_bounds,
            ..
        } => (bounds, content_bounds),
        other => panic!("expected a scrollable for {id:?}, got {other:?}"),
    }
}

#[test]
fn the_about_card_renders_exactly_440_wide() {
    let vm = known_vm();
    let card = about_card(&vm, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    let bounds = framed_card_bounds(&mut ui, ABOUT_CARD_ID);
    assert_eq!(
        bounds.width, 440.0,
        "the rendered card box must be the §4.6.2 width, not the retired 620"
    );
    drop(ui);
}

#[test]
fn the_about_card_content_is_the_width_less_the_frame_padding() {
    // 440 outer with space::LG on both sides leaves 408 of content. The
    // card is 440 *including* its padding, so the usable width is not
    // 440 — a claim a caller could otherwise easily make.
    let frame = modal_frame();
    let content = about_card_width(ROOMY_VIEWPORT.width) - 2.0 * frame.content_padding;
    assert_eq!(content, 408.0);
    assert_eq!(frame.content_padding, hoarfrost::token::space::LG);
}

#[test]
fn the_about_card_fits_a_768px_viewport_without_overflow() {
    // The §4.6.2 height policy for About is content-fit with no ceiling,
    // so the only real bound is the viewport. Measured at 768px — the
    // card is 624 tall, leaving 144px of headroom, so the footer and the
    // bottom divider stay on screen.
    let vm = AboutViewModel::from_build();
    let viewport = Size::new(1024.0, 768.0);
    let card = about_card(&vm, viewport);
    let mut ui = laid_out(viewport, card);
    let bounds = framed_card_bounds(&mut ui, ABOUT_CARD_ID);
    assert_eq!(bounds.width, 440.0);
    assert!(
        bounds.height <= viewport.height,
        "the About card measured {:.0}px tall against a {:.0}px viewport — \
         a content-fit card taller than the window would push its footer off \
         screen",
        bounds.height,
        viewport.height
    );
    drop(ui);
}

#[test]
fn the_real_license_body_is_scrolled_not_clipped() {
    // The real MIT body reflowed at 408px is taller than the 200px
    // region, so the scrollable's content exceeds its viewport and every
    // word stays reachable. If this ever stops holding, the licence text
    // has shrunk below the cap and the cap itself should be revisited —
    // not the assertion quietly widened.
    let vm = AboutViewModel::from_build();
    let card = about_card(&vm, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    let (viewport_bounds, content_bounds) = scroll_bounds(&mut ui, LICENSE_SCROLL_ID);
    assert_eq!(
        viewport_bounds.height, LICENSE_MAX_HEIGHT,
        "the licence region must be exactly the cap it declares"
    );
    assert_eq!(
        content_bounds.height, 480.0,
        "the MIT body reflowed at 408px lays out to 24 BODY lines (24 x 20 = \
         480px); if this changes the licence changed, and the cap should be \
         re-decided rather than this number quietly widened"
    );
    assert!(
        content_bounds.height > viewport_bounds.height,
        "expected the licence body ({:.0}px) to overflow the {:.0}px region so \
         it scrolls; if it no longer does, LICENSE_MAX_HEIGHT is dead weight \
         and should be dropped rather than left lying",
        content_bounds.height,
        viewport_bounds.height
    );
    drop(ui);
}

#[test]
fn a_body_rung_lays_out_on_its_declared_line_grid() {
    // REGRESSION GUARD for the bug INT-5a introduced and this caught.
    // `impl From<f32> for iced::LineHeight` yields
    // `LineHeight::Relative(factor)` — a MULTIPLE of the font size
    // (`iced_core-0.14.0/src/text.rs:221-225`). Handing `rung.line_height`
    // (20.0) over as a bare f32 therefore asked iced for twenty times 14px
    // = 280px per line, and the whole card ballooned to the viewport height.
    // Only `iced::Pixels` reaches `LineHeight::Absolute`. A BODY line must
    // measure its declared 20px, not 280px.
    let probe = iced::widget::container(rung("0123456789", typography::BODY, iced::Color::WHITE))
        .id("about_probe_body_line")
        .width(Length::Shrink)
        .height(Length::Shrink);
    let mut ui = laid_out(Size::new(1024.0, 2000.0), probe.into());
    let bounds = framed_card_bounds(&mut ui, "about_probe_body_line");
    assert_eq!(
        bounds.height,
        typography::BODY.line_height,
        "one BODY line must be line_height pixels tall; a bare-f32 line_height \
         would report size x factor (14 x 20 = 280) instead"
    );
    assert!(bounds.height < typography::BODY.size * 3.0);
    drop(ui);
}

#[test]
fn the_about_card_is_content_fit_not_viewport_stretched() {
    // §4.6.2 gives About a content-fit height. Measured: the card is the
    // same height in a 768px window and a 2000px one, i.e. it takes the
    // height its content needs and no more. Before the line-height fix this
    // assertion failed — the card tracked the viewport exactly, because
    // every text line claimed 280px and the natural height blew past any
    // window.
    let vm = AboutViewModel::from_build();
    let short_window = Size::new(1024.0, 768.0);
    let tall_window = Size::new(1024.0, 2000.0);

    let short_card = about_card(&vm, short_window);
    let mut ui = laid_out(short_window, short_card);
    let short = framed_card_bounds(&mut ui, ABOUT_CARD_ID);
    drop(ui);

    let tall_card = about_card(&vm, tall_window);
    let mut ui = laid_out(tall_window, tall_card);
    let tall = framed_card_bounds(&mut ui, ABOUT_CARD_ID);
    drop(ui);

    assert_eq!(
        short.height, tall.height,
        "a content-fit card must not track the window: {short:?} vs {tall:?}"
    );
    assert_ne!(short.height, tall_window.height);
    assert_eq!(
        short.height, 624.0,
        "the About card lays out 440 x 624 at the §4.6.2 width"
    );
}

#[test]
fn a_short_viewport_compresses_the_licence_region_rather_than_overflowing() {
    // The card cannot be shorter than its fixed rows, so on a very short
    // window the licence region is what gives. Measured at 400px tall:
    // the card is 400 and the licence region 150 — still scrollable, and
    // nothing is pushed off screen.
    let vm = AboutViewModel::from_build();
    let viewport = Size::new(1024.0, 400.0);
    let card = about_card(&vm, viewport);
    let mut ui = laid_out(viewport, card);
    let bounds = framed_card_bounds(&mut ui, ABOUT_CARD_ID);
    let (region, content) = scroll_bounds(&mut ui, LICENSE_SCROLL_ID);
    assert_eq!(bounds.height, 400.0);
    assert!(
        region.height > 0.0 && region.height < LICENSE_MAX_HEIGHT,
        "the licence region should absorb the shortfall, got {:.0}px",
        region.height
    );
    assert!(content.height > region.height, "and still scroll");
    drop(ui);
}

#[test]
fn the_card_renders_inside_a_narrow_viewport_without_clipping_itself() {
    // At a clamped width the card must still lay out — the gutter clamp
    // has to reach the box, not just the reported number.
    let vm = known_vm();
    let viewport = Size::new(300.0, 800.0);
    let card = about_card(&vm, viewport);
    let mut ui = laid_out(viewport, card);
    let bounds = framed_card_bounds(&mut ui, ABOUT_CARD_ID);
    assert_eq!(
        bounds.width,
        300.0 - hoarfrost::surface::modal::VIEWPORT_GUTTER
    );
    assert!(bounds.width > 0.0);
    drop(ui);
}

#[test]
fn the_close_button_survives_the_narrow_card_and_is_not_icon_only() {
    // `Button::is_icon_only` is geometric; the footer label is mounted
    // wide enough that a different font measurement cannot trip the
    // icon-only debug_assert. Laying the card out at the clamped width
    // exercises that path for real.
    let vm = known_vm();
    let viewport = Size::new(300.0, 800.0);
    let card = about_card(&vm, viewport);
    let mut ui = laid_out(viewport, card);
    let bounds = ui
        .find(|candidate: Candidate<'_>| match candidate {
            Candidate::Focusable { bounds, .. } => Some(bounds),
            _ => None,
        })
        .expect("the footer button should be focusable in the laid-out card");
    assert!(
        bounds.width >= hoarfrost::token::size::CONTROL_SM,
        "the Close button measured {:.0}px wide — at or under CONTROL_SM it \
         would be classed icon-only and needs a tooltip",
        bounds.width
    );
    drop(ui);
}

#[test]
fn the_bounded_trait_reports_the_card_as_visible() {
    // Sanity check on the measurement harness itself: the card reports
    // visible bounds inside the window, so the numbers above are being
    // read from a laid-out tree and not a default rectangle.
    let vm = known_vm();
    let card = about_card(&vm, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    let target = ui
        .find(iced::widget::Id::from(ABOUT_CARD_ID))
        .expect("card reachable");
    let visible = target
        .visible_bounds()
        .expect("the card should report visible bounds");
    assert!(visible.width > 0.0 && visible.height > 0.0);
    drop(ui);
}

// --- Window state --------------------------------------------------------

#[test]
fn default_window_state_is_closed() {
    let state = AboutWindowState::new();
    assert!(!state.is_open());
    assert!(!state.is_about());
    assert!(!state.is_attributions());
    assert_eq!(state.window(), None);
    assert_eq!(state, AboutWindowState::default());
}

#[test]
fn opening_a_window_reports_the_change() {
    let mut state = AboutWindowState::new();
    assert!(state.open_about());
    assert!(state.is_about());
    assert!(state.is_open());
    assert_eq!(state.window(), Some(AboutWindow::About));
}

#[test]
fn opening_the_same_window_again_reports_no_change() {
    let mut state = AboutWindowState::new();
    assert!(state.open_attributions());
    assert!(
        !state.open_attributions(),
        "a redundant open must not report a visible change"
    );
    assert!(state.is_attributions());
}

#[test]
fn opening_a_window_replaces_the_previous_one() {
    // One window at a time — the same rule `DialogState` and
    // `OverlaySlot::open_modal` apply.
    let mut state = AboutWindowState::new();
    state.open_about();
    assert!(state.open_attributions());
    assert!(state.is_attributions());
    assert!(!state.is_about());
}

#[test]
fn close_reports_whether_a_window_was_open() {
    let mut state = AboutWindowState::new();
    assert!(
        !state.close(),
        "closing an already-closed state consumes nothing"
    );
    state.open_about();
    assert!(state.close());
    assert!(!state.is_open());
    assert!(!state.close());
}

#[test]
fn apply_attributions_requested_opens_the_credits_window() {
    let mut state = AboutWindowState::new();
    assert!(state.apply(AboutMessage::AttributionsRequested));
    assert!(state.is_attributions());
    assert!(!state.open_attributions(), "already showing credits");
}

#[test]
fn apply_close_requested_closes_only_when_open() {
    let mut state = AboutWindowState::new();
    assert!(!state.apply(AboutMessage::CloseRequested));
    state.open_about();
    assert!(state.apply(AboutMessage::CloseRequested));
    assert!(!state.is_open());
}

#[test]
fn apply_deferred_intents_change_no_window_state() {
    // The clipboard write and the browser open are the reducer's job; the
    // window state is untouched and reports "no visible change".
    let mut state = AboutWindowState::new();
    state.open_about();
    assert!(!state.apply(AboutMessage::CopyCommitHashRequested));
    assert!(state.is_about());
    assert!(!state.apply(AboutMessage::RepositoryOpened {
        url: "https://github.com/dtolnay/anyhow".into(),
    }));
    assert!(state.is_about());
}

#[test]
fn apply_absorbed_is_an_observable_no_op() {
    // The card-boundary message exists so absorption can be *seen*: the
    // reducer receives it and changes nothing. That is the difference
    // between "the press was swallowed by the card" and "the press never
    // arrived" (FMD-071), and it is what keeps a padding click from
    // reading as a dismissal.
    for window in [AboutWindow::About, AboutWindow::Attributions] {
        let mut state = AboutWindowState::new();
        state.open(window);
        assert!(
            !state.apply(AboutMessage::Absorbed),
            "absorption must not report a visible change"
        );
        assert_eq!(
            state.window(),
            Some(window),
            "absorption must not close the {window:?} window"
        );
    }
}

#[test]
fn absorbed_is_not_close_requested() {
    // Guards against the two being collapsed into one variant: one closes,
    // the other must not.
    assert_ne!(AboutMessage::Absorbed, AboutMessage::CloseRequested);
}

#[test]
fn window_titles_come_from_strings() {
    assert_eq!(AboutWindow::About.title(), strings::ABOUT_DIALOG_TITLE);
    assert_eq!(
        AboutWindow::Attributions.title(),
        strings::ABOUT_ATTRIBUTIONS_HEADER
    );
}

// --- License reflow and first-run auto-show ([RUST-003]) ----------------

#[test]
fn license_reflow_preserves_words() {
    // The reflowed license must contain exactly the same words in the same
    // order (whitespace-only change), keep paragraph breaks, and leave no
    // single newlines that would pin the label to the file's hard wrap.
    let words_original: Vec<&str> = LICENSE_TEXT.split_whitespace().collect();
    let reflowed = license_display_text();
    let words_reflowed: Vec<&str> = reflowed.split_whitespace().collect();
    assert_eq!(words_reflowed, words_original);
    assert_eq!(
        reflowed.matches("\n\n").count(),
        LICENSE_TEXT.matches("\n\n").count(),
        "paragraph breaks must survive the reflow"
    );
    let singles = reflowed.replace("\n\n", "").matches('\n').count();
    assert_eq!(singles, 0, "no single newlines may remain");
}

#[test]
fn should_auto_show_about_none_means_first_run() {
    // Spec FR-016 / SC-008: fresh UI state (no recorded shown version)
    // must auto-show the About dialog.
    assert!(
        should_auto_show_about(None, "0.2.0"),
        "fresh state should auto-show"
    );
}

#[test]
fn should_auto_show_about_same_version_stays_quiet() {
    // Spec FR-016 / SC-008: same-version restarts must not re-display.
    assert!(
        !should_auto_show_about(Some("0.2.0"), "0.2.0"),
        "same version should stay quiet"
    );
}

#[test]
fn should_auto_show_about_upgrade_reopens_once() {
    // Spec FR-016 / SC-008: each upgrade's first start re-displays once.
    assert!(
        should_auto_show_about(Some("0.1.0"), "0.2.0"),
        "upgraded version should auto-show"
    );
}

#[test]
fn apply_first_run_auto_show_opens_and_records() {
    let mut persisted = crate::ui::PersistedUiState::default();
    let mut dialogs = crate::ui::Dialogs::new();
    assert!(apply_first_run_auto_show(
        &mut persisted,
        &mut dialogs,
        "0.2.0"
    ));
    assert!(dialogs.about_dialog_open);
    assert_eq!(persisted.about_shown_for_version.as_deref(), Some("0.2.0"));
    // Second run at the same version stays quiet and changes nothing.
    dialogs.about_dialog_open = false;
    assert!(!apply_first_run_auto_show(
        &mut persisted,
        &mut dialogs,
        "0.2.0"
    ));
    assert!(!dialogs.about_dialog_open);
}
