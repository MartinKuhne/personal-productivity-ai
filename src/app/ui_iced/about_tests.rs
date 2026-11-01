//! Sidecar unit tests for `ui_iced/about.rs`.
//!
//! Pure view-model, state-transition and widget-construction tests
//! ([RUST-003]). No renderer, no bus, no filesystem: every fixture is a
//! synthetic literal that is never opened, and the only real data read is
//! the compile-time build metadata and the `include_str!`-embedded license
//! ([RUST-006]).
//!
//! The geometry tests split in two. The tier-1 ones read the sizing policy
//! ([`ABOUT_SIZING`], [`credits_list_height`]); the tier-2 ones lay the card
//! out in a headless `iced_test::Simulator` and measure the box, because an
//! 80%-of-the-window card is only worth claiming if the tree actually
//! produces 80% and the inline credits list actually fits inside it.

use super::*;
use crate::ui::attributions::{Attribution, DIRECT_DEPENDENCIES};
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

/// The credits snapshot the card renders inline, over three synthetic rows.
///
/// The rows are never opened: the repository cell only carries a deferred
/// open-in-browser intent, and the reducer is what would act on it
/// ([RUST-006]).
fn credits_vm() -> AttributionsViewModel {
    let row = |name: &'static str| Attribution {
        name,
        authors: "Fixture Author",
        github_url: "https://example.invalid/fixture",
    };
    AttributionsViewModel::with_entries(vec![row("alpha"), row("beta"), row("gamma")])
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
fn the_credits_snapshot_the_card_uses_is_the_reused_catalog() {
    // The card's count line and its rows come from one snapshot, so the only
    // honest pin is the catalog itself: the production snapshot carries every
    // entry and nothing else.
    let vm = AttributionsViewModel::from_catalog();
    assert_eq!(vm.count(), DIRECT_DEPENDENCIES.len());
    assert!(vm.count() > 0, "the catalog is not empty");
    assert_eq!(vm.entries(), DIRECT_DEPENDENCIES);
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
fn the_credits_count_line_uses_the_shared_formatter() {
    // The line the card renders above the inline table is the shared
    // formatter over the same snapshot the rows come from.
    assert_eq!(credits_vm().summary(), strings::format_attribution_count(3));
    assert_eq!(
        AttributionsViewModel::with_entries(Vec::new()).summary(),
        strings::ABOUT_ATTRIBUTIONS_EMPTY
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

/// A viewport wide and tall enough that neither the gutter clamp nor the
/// width floor bites.
const ROOMY_VIEWPORT: Size = Size::new(1280.0, 800.0);

#[test]
fn about_card_builds_for_known_and_unknown_metadata() {
    let known = known_vm();
    let unknown = unknown_vm();
    let credits = credits_vm();
    let known_card: Element<'_, Message> = about_card(&known, &credits, ROOMY_VIEWPORT);
    let unknown_card: Element<'_, Message> = about_card(&unknown, &credits, ROOMY_VIEWPORT);
    // Constructing both states must not panic; the unknown build differs
    // only in text and press handlers, never in widget count.
    let _ = (known_card, unknown_card);
}

#[test]
fn about_card_builds_from_the_build_snapshot() {
    let vm = AboutViewModel::from_build();
    // The card borrows the credits snapshot, so it must outlive the card.
    let credits = credits_vm();
    let _card: Element<'_, Message> = vm.view(&credits, ROOMY_VIEWPORT);
}

#[test]
fn about_card_builds_over_an_empty_catalog() {
    // The empty state changes the count line's text, not the widget count
    // (AGENTS §6): the section, the table header and the scroll area stay.
    let vm = known_vm();
    let empty = AttributionsViewModel::with_entries(Vec::new());
    let _card: Element<'_, Message> = about_card(&vm, &empty, ROOMY_VIEWPORT);
}

#[test]
fn footer_and_close_button_build() {
    let _close: Element<'static, Message> = close_button();
    let _footer: Element<'static, Message> = about_footer();
}

// --- Sizing: an app-side override of §4.6.2 ------------------------------

#[test]
fn the_card_fills_the_viewport_on_both_axes() {
    // The owner's ruling: the card takes most of the window instead of
    // fitting its content. One ratio answers both axes.
    assert_eq!(ABOUT_VIEWPORT_FILL, 0.8);
    assert_eq!(about_card_width(1200.0), 960.0);
    assert_eq!(about_card_height(800.0), 640.0);
    assert_eq!(
        about_card_size(Size::new(1200.0, 800.0)),
        Size::new(960.0, 640.0)
    );
}

#[test]
fn the_card_states_its_own_policy_not_the_spec_row() {
    // The retired policy pinned the card to `ModalSizing::about()`: 440px
    // wide, content-fit tall. Both are gone on purpose, and this pin keeps
    // the replacement deliberate rather than accidental.
    assert!(about_card_width(ROOMY_VIEWPORT.width) > hoarfrost::surface::modal::ABOUT_WIDTH);
    assert_eq!(AboutWindow::About.sizing(), ABOUT_SIZING);
    assert_ne!(
        AboutWindow::About.sizing(),
        hoarfrost::surface::modal::ModalSizing::about(),
        "the card overrides §4.6.2 rather than restating it"
    );
}

#[test]
fn the_width_keeps_the_retired_spec_width_as_its_floor() {
    // Below a ~550px window 80% is narrower than the width the metadata row
    // and the three-column credits table were measured in, so `ABOUT_WIDTH`
    // holds the floor until the fill passes it.
    let spec = hoarfrost::surface::modal::ABOUT_WIDTH;
    assert_eq!(about_card_width(500.0), spec, "floored");
    assert_eq!(about_card_width(550.0), spec, "exactly at the crossover");
    assert_eq!(about_card_width(600.0), 480.0, "the fill has passed it");
}

#[test]
fn the_height_has_no_floor_and_the_gutter_wins_last() {
    // §4.6.2 gives About a content-fit height with no ceiling, so there is no
    // framework figure for the override to defend: a short window gets a
    // short card and the scroll regions inside it absorb the difference.
    let gutter = hoarfrost::surface::modal::VIEWPORT_GUTTER;
    assert_eq!(about_card_height(400.0), 320.0);
    assert_eq!(about_card_height(gutter + 100.0), 100.0, "gutter clamp");
    assert_eq!(about_card_height(100.0), 100.0 - gutter, "gutter over fill");
}

#[test]
fn about_card_width_clamps_to_the_viewport_gutter() {
    // The gutter is applied last and always wins, so neither the 80% fill nor
    // the width floor can overflow a tiny viewport.
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
    assert_eq!(about_card_height(10.0), 0.0);
    assert_eq!(about_card_height(0.0), 0.0);
}

#[test]
fn about_card_builds_at_every_clamped_width() {
    // The card must construct at a hairline viewport as well as a roomy
    // one — the gutter clamp must not leave the layout unable to build.
    let vm = known_vm();
    let credits = credits_vm();
    for width in [1.0, 32.0, 440.0, 1280.0, 4096.0] {
        let viewport = Size::new(width, 800.0);
        let _card: Element<'_, Message> = about_card(&vm, &credits, viewport);
    }
}

/// Everything the card spends around the credits list, from the same terms
/// [`credits_list_height`] charges ([`SubRegionBands::total`]).
fn credits_furniture() -> f32 {
    ABOUT_CREDITS_HEADER_BAND_HEIGHT
        + 2.0 * modal_frame().content_padding
        + ABOUT_CREDITS_FOOTER_BAND_HEIGHT
}

#[test]
fn the_credits_region_is_solved_from_the_card_it_sits_in() {
    // UIF-MOD-004: the list gets what the card has left after its own
    // furniture, never a number of its own — and never more than the §2.2 cap.
    let ordinary = about_card_height(800.0);
    assert_eq!(
        credits_list_height(ordinary),
        ordinary - credits_furniture(),
        "an ordinary window leaves the list the card's remainder"
    );
    assert!(
        credits_list_height(ordinary) < ATTRIBUTIONS_LIST_MAX_HEIGHT,
        "on an ordinary window the remainder is what binds"
    );
    assert_eq!(
        credits_list_height(about_card_height(2000.0)),
        ATTRIBUTIONS_LIST_MAX_HEIGHT,
        "on a tall window the §2.2 cap binds instead"
    );
    assert_eq!(
        credits_list_height(credits_furniture()),
        0.0,
        "a card with nothing left gives the list nothing"
    );
}

#[test]
fn a_solved_zero_length_falls_back_to_fill_rather_than_a_void_child() {
    // `Length::Fixed(0.0)` is void in iced 0.14 and would drop the scrollable
    // — and with it its stable id — out of the tree entirely (AGENTS §5).
    assert_eq!(credits_list_length(credits_furniture()), Length::Fill);
    let ordinary = about_card_height(800.0);
    assert_eq!(
        credits_list_length(ordinary),
        Length::Fixed(credits_list_height(ordinary))
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
fn the_about_card_renders_at_eighty_percent_of_the_window() {
    // The claim the owner asked for, measured on the laid-out box rather than
    // read off the policy: 80% of a 1280x800 window is 1024x640.
    let vm = known_vm();
    // The card borrows the credits snapshot, so it must outlive the card.
    let credits = credits_vm();
    let card = about_card(&vm, &credits, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    let bounds = framed_card_bounds(&mut ui, ABOUT_CARD_ID);
    assert_eq!(
        bounds.width,
        ROOMY_VIEWPORT.width * ABOUT_VIEWPORT_FILL,
        "the rendered card box must be the fill it declares"
    );
    assert_eq!(
        bounds.height,
        ROOMY_VIEWPORT.height * ABOUT_VIEWPORT_FILL,
        "the rendered card box must be as tall as it is wide, proportionally"
    );
    assert_eq!(bounds.width, 1024.0);
    assert_eq!(bounds.height, 640.0);
    drop(ui);
}

#[test]
fn the_about_card_content_is_the_width_less_the_frame_padding() {
    // The card is its width *including* padding, so the usable width is not
    // the fill — a claim a caller could otherwise easily make.
    let frame = modal_frame();
    let content = about_card_width(ROOMY_VIEWPORT.width) - 2.0 * frame.content_padding;
    assert_eq!(content, 992.0);
    assert_eq!(frame.content_padding, hoarfrost::token::space::LG);
}

#[test]
fn the_about_card_fits_a_768px_viewport_without_overflow() {
    // A viewport-sized card cannot be taller than its window by construction,
    // which is what the retired content-fit card could not promise: that one
    // measured 624px and only fitted a 768px window by luck.
    let vm = AboutViewModel::from_build();
    let viewport = Size::new(1024.0, 768.0);
    // The card borrows the credits snapshot, so it must outlive the card.
    let credits = credits_vm();
    let card = about_card(&vm, &credits, viewport);
    let mut ui = laid_out(viewport, card);
    let bounds = framed_card_bounds(&mut ui, ABOUT_CARD_ID);
    assert!(
        bounds.height <= viewport.height,
        "the About card measured {:.0}px tall against a {:.0}px viewport — a \
         viewport-sized card must never outgrow its own window",
        bounds.height,
        viewport.height
    );
    assert_eq!(bounds.width, viewport.width * ABOUT_VIEWPORT_FILL);
    assert_eq!(bounds.height, viewport.height * ABOUT_VIEWPORT_FILL);
    drop(ui);
}

#[test]
fn the_license_region_keeps_its_declared_cap() {
    // Whatever the window, the licence region is *given* the cap it declares.
    // Whether the MIT body overflows it is a question of width: at the retired
    // 440px card it measured 480px of text and scrolled; at the wide 80% card
    // it fits and simply does not. Either way nothing is clipped, because the
    // region is a scrollable and not a fixed label.
    let vm = AboutViewModel::from_build();
    // The card borrows the credits snapshot, so it must outlive the card.
    let credits = credits_vm();
    let card = about_card(&vm, &credits, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    let (region, content) = scroll_bounds(&mut ui, LICENSE_SCROLL_ID);
    assert_eq!(
        region.height, LICENSE_MAX_HEIGHT,
        "the licence region must be exactly the cap it declares"
    );
    assert!(
        content.height > 0.0,
        "the MIT body must be laid out inside the region, not dropped"
    );
    drop(ui);
}

#[test]
fn a_license_body_longer_than_the_region_scrolls() {
    // The hazard the cap exists for, measured where it still bites: a narrow
    // card reflows the MIT body to far more than 200px, so the content
    // overflows the region and every word stays reachable. If this ever stops
    // holding, the licence has shrunk and the cap should be re-decided rather
    // than this assertion quietly widened.
    let vm = AboutViewModel::from_build();
    let viewport = Size::new(560.0, 1200.0);
    // The card borrows the credits snapshot, so it must outlive the card.
    let credits = credits_vm();
    let card = about_card(&vm, &credits, viewport);
    let mut ui = laid_out(viewport, card);
    let (region, content) = scroll_bounds(&mut ui, LICENSE_SCROLL_ID);
    assert_eq!(region.height, LICENSE_MAX_HEIGHT);
    assert!(
        content.height > region.height,
        "expected the licence body ({:.0}px) to overflow the {:.0}px region so \
         it scrolls",
        content.height,
        region.height
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
fn the_about_card_tracks_the_window_not_its_content() {
    // The inverse of the retired behaviour: the card used to be the same
    // height in a 768px window and a 2000px one (624px of content, both
    // times). A viewport-sized card must instead track its window, so the
    // same content in a bigger window produces a bigger card.
    let vm = AboutViewModel::from_build();
    let credits = credits_vm();
    let short_window = Size::new(1024.0, 768.0);
    let tall_window = Size::new(1024.0, 2000.0);

    let short_card = about_card(&vm, &credits, short_window);
    let mut ui = laid_out(short_window, short_card);
    let short = framed_card_bounds(&mut ui, ABOUT_CARD_ID);
    drop(ui);

    let tall_card = about_card(&vm, &credits, tall_window);
    let mut ui = laid_out(tall_window, tall_card);
    let tall = framed_card_bounds(&mut ui, ABOUT_CARD_ID);
    drop(ui);

    assert_ne!(
        short.height, tall.height,
        "a viewport-sized card must track the window: {short:?} vs {tall:?}"
    );
    assert_eq!(short.height, short_window.height * ABOUT_VIEWPORT_FILL);
    assert_eq!(tall.height, tall_window.height * ABOUT_VIEWPORT_FILL);
    assert!(tall.height < tall_window.height, "and never fill it");
}

#[test]
fn a_short_viewport_collapses_the_credits_region_without_losing_it() {
    // A 400px window gives a 320px card, which is less than the card's own
    // furniture. The licence keeps its declared cap and the credits list takes
    // the shortfall — and because the solved zero is rendered as `Fill` rather
    // than `Length::Fixed(0.0)`, the scrollable is still in the tree with its
    // stable id instead of being silently dropped (AGENTS §5, §6).
    let vm = AboutViewModel::from_build();
    let viewport = Size::new(1024.0, 400.0);
    // The card borrows the credits snapshot, so it must outlive the card.
    let credits = credits_vm();
    let card = about_card(&vm, &credits, viewport);
    let mut ui = laid_out(viewport, card);
    let bounds = framed_card_bounds(&mut ui, ABOUT_CARD_ID);
    let (licence, licence_content) = scroll_bounds(&mut ui, LICENSE_SCROLL_ID);
    let (credits_region, _) =
        scroll_bounds(&mut ui, crate::ui_iced::attributions::ATTRIBUTIONS_LIST_ID);
    assert_eq!(bounds.height, viewport.height * ABOUT_VIEWPORT_FILL);
    assert!(
        licence.height <= LICENSE_MAX_HEIGHT,
        "the licence region must not exceed its cap, got {:.0}px",
        licence.height
    );
    assert!(licence_content.height > licence.height, "and still scroll");
    assert!(
        credits_region.height <= credits_list_height(bounds.height),
        "the credits list must not claim more than the solver gave it"
    );
    drop(ui);
}

#[test]
fn the_card_renders_inside_a_narrow_viewport_without_clipping_itself() {
    // At a clamped width the card must still lay out — the gutter clamp
    // has to reach the box, not just the reported number.
    let vm = known_vm();
    let viewport = Size::new(300.0, 800.0);
    // The card borrows the credits snapshot, so it must outlive the card.
    let credits = credits_vm();
    let card = about_card(&vm, &credits, viewport);
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
    // The card borrows the credits snapshot, so it must outlive the card.
    let credits = credits_vm();
    let card = about_card(&vm, &credits, viewport);
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
    // The card borrows the credits snapshot, so it must outlive the card.
    let credits = credits_vm();
    let card = about_card(&vm, &credits, ROOMY_VIEWPORT);
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

// --- The credits list is inline ------------------------------------------

#[test]
fn the_credits_list_is_inside_the_card_and_gets_its_solved_height() {
    use crate::ui_iced::attributions::ATTRIBUTIONS_LIST_ID;

    let vm = known_vm();
    // The card borrows the credits snapshot, so it must outlive the card.
    let credits = credits_vm();
    let card = about_card(&vm, &credits, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    let card_bounds = framed_card_bounds(&mut ui, ABOUT_CARD_ID);
    let (list, list_content) = scroll_bounds(&mut ui, ATTRIBUTIONS_LIST_ID);

    // The height is the solver's answer, not a number the widget picked.
    match credits_list_length(card_bounds.height) {
        Length::Fixed(solved) => assert_eq!(list.height, solved),
        other => panic!("an ordinary window leaves room, so the list must be Fixed, got {other:?}"),
    }
    assert!(
        list_content.height > 0.0,
        "the credits table must be laid out inside the scroll area"
    );
    // The overflow hazard the solved height carries: an understated band
    // would push the list past the card it is inside.
    assert!(
        list.y >= card_bounds.y && list.x >= card_bounds.x,
        "the list must start inside the card: list {list:?} card {card_bounds:?}"
    );
    assert!(
        list.y + list.height <= card_bounds.y + card_bounds.height + 0.01,
        "the credits list must not hang below its card: \
         list {list:?} card {card_bounds:?}"
    );
    drop(ui);
}

#[test]
fn the_card_offers_no_button_to_open_a_credits_window() {
    // The teaser button and the separate credits window were removed by
    // ruling. The label is written out here rather than referenced, because
    // the constant it used to read is gone — this assertion is the pin that
    // keeps it gone.
    let vm = known_vm();
    // The card borrows the credits snapshot, so it must outlive the card.
    let credits = credits_vm();
    let card = about_card(&vm, &credits, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    assert!(
        ui.find("View third-party attributions...").is_err(),
        "the teaser button must not come back: the credits list is inline"
    );
    assert!(
        ui.find(strings::ABOUT_ATTRIBUTIONS_HEADER).is_ok(),
        "the section header is how the credits list is introduced instead"
    );
    assert!(
        ui.find(strings::format_attribution_count(3).as_str())
            .is_ok(),
        "the count line names the rows the card is showing"
    );
    drop(ui);
}

#[test]
fn an_empty_catalog_still_renders_the_credits_section() {
    // AGENTS §6: the empty state is text, not a different widget tree.
    let vm = known_vm();
    let empty = AttributionsViewModel::with_entries(Vec::new());
    let card = about_card(&vm, &empty, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    assert!(
        ui.find(strings::ABOUT_ATTRIBUTIONS_EMPTY).is_ok(),
        "the empty-state line must be shown"
    );
    let _ = scroll_bounds(&mut ui, crate::ui_iced::attributions::ATTRIBUTIONS_LIST_ID);
    drop(ui);
}

// --- Window state --------------------------------------------------------

#[test]
fn default_window_state_is_closed() {
    let state = AboutWindowState::new();
    assert!(!state.is_open());
    assert!(!state.is_about());
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
    assert!(state.open_about());
    assert!(
        !state.open_about(),
        "a redundant open must not report a visible change"
    );
    assert!(state.is_about());
}

#[test]
fn the_window_state_names_the_one_window_it_can_open() {
    // `AboutWindow` is a single-variant marker now that the credits list is
    // inside the card. The pin is here so a second variant stays a decision
    // rather than an accident: it would need a registry surface id, an
    // `OverlayLayer` and an escape-rank row with it.
    assert_eq!(AboutWindow::About.title(), strings::ABOUT_DIALOG_TITLE);
    assert_eq!(AboutWindow::About.sizing(), ABOUT_SIZING);
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
    let mut state = AboutWindowState::new();
    state.open(AboutWindow::About);
    assert!(
        !state.apply(AboutMessage::Absorbed),
        "absorption must not report a visible change"
    );
    assert_eq!(
        state.window(),
        Some(AboutWindow::About),
        "absorption must not close the card"
    );
}

#[test]
fn absorbed_is_not_close_requested() {
    // Guards against the two being collapsed into one variant: one closes,
    // the other must not.
    assert_ne!(AboutMessage::Absorbed, AboutMessage::CloseRequested);
}

#[test]
fn the_credits_header_is_a_section_title_from_strings() {
    // It used to double as the second window's title. Now it is only the
    // heading of the section inside the About card, and it still comes from
    // `strings` rather than being written at the call site.
    assert_eq!(
        strings::ABOUT_ATTRIBUTIONS_HEADER,
        "Third-Party Attributions"
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
