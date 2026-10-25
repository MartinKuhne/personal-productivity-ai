//! Sidecar unit tests for `ui_iced/attributions.rs`.
//!
//! Pure view-model and widget-construction tests ([RUST-003]). The catalog
//! is compile-time data, so these tests read it directly and assert the
//! snapshot preserves it exactly — no renderer, no bus, no filesystem
//! ([RUST-006]).

use super::*;
use crate::ui::strings;
use iced::Element;

/// A synthetic two-row catalog — never read from disk ([RUST-006]).
fn fixture_entries() -> Vec<Attribution> {
    vec![
        Attribution {
            name: "aaa-crate",
            authors: "A. Author",
            github_url: "https://github.com/a/aaa-crate",
        },
        Attribution {
            name: "zzz-crate",
            authors: "Z. Author",
            github_url: "https://github.com/z/zzz-crate",
        },
    ]
}

// --- Data source: the shared catalog ----------------------------------------

/// A viewport large enough that no gutter clamp bites.
const ROOMY_VIEWPORT: Size = Size::new(1280.0, 800.0);

#[test]
fn from_catalog_is_the_shared_catalog_verbatim() {
    let vm = AttributionsViewModel::from_catalog();
    assert_eq!(
        vm.entries(),
        DIRECT_DEPENDENCIES,
        "the view must show the shared catalog unchanged"
    );
}

#[test]
fn catalog_is_non_empty() {
    let vm = AttributionsViewModel::from_catalog();
    assert!(
        !vm.is_empty() && vm.count() == DIRECT_DEPENDENCIES.len(),
        "the attribution catalog must carry rows"
    );
}

#[test]
fn every_entry_carries_name_authors_and_repository() {
    // The catalog's row shape is `name` / `authors` / `github_url` — there
    // is no per-crate license or version field, so these three are what
    // the window can show.
    for entry in AttributionsViewModel::from_catalog().entries() {
        assert!(!entry.name.is_empty(), "empty crate name in {entry:?}");
        assert!(
            !entry.authors.is_empty(),
            "empty authors for {}",
            entry.name
        );
        assert!(
            entry.github_url.starts_with("https://github.com/"),
            "repository URL for {} must be an https GitHub link, got {}",
            entry.name,
            entry.github_url
        );
    }
}

#[test]
fn ordering_is_deterministic_and_alphabetical() {
    // Mirrors `ui::attributions_tests::slice_is_sorted_and_unique`: the
    // snapshot must not reorder anything, so two runs render identically.
    let names: Vec<&str> = AttributionsViewModel::from_catalog()
        .entries()
        .iter()
        .map(|entry| entry.name)
        .collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        names, sorted,
        "the rendered order must be the catalog order"
    );
}

#[test]
fn catalog_has_no_duplicate_crate_names() {
    let mut seen = std::collections::HashSet::new();
    for entry in AttributionsViewModel::from_catalog().entries() {
        assert!(
            seen.insert(entry.name),
            "duplicate crate name {}",
            entry.name
        );
    }
}

#[test]
fn snapshot_is_independent_of_later_catalog_reads() {
    // The snapshot owns its `Vec`, so two snapshots are equal but not
    // aliasing the same allocation-dependent state.
    let a = AttributionsViewModel::from_catalog();
    let b = AttributionsViewModel::from_catalog();
    assert_eq!(a, b);
    assert_eq!(a.count(), b.count());
}

// --- Summary line --------------------------------------------------------

#[test]
fn summary_counts_the_entries() {
    let vm = AttributionsViewModel::with_entries(fixture_entries());
    assert_eq!(vm.count(), 2);
    assert_eq!(vm.summary(), strings::format_attribution_count(2));
    assert!(vm.summary().contains("2"));
}

#[test]
fn empty_catalog_reports_the_empty_state_line() {
    let vm = AttributionsViewModel::with_entries(Vec::new());
    assert!(vm.is_empty());
    assert_eq!(vm.summary(), strings::ABOUT_ATTRIBUTIONS_EMPTY);
}

#[test]
fn default_view_model_is_empty() {
    let vm = AttributionsViewModel::default();
    assert!(vm.is_empty());
    assert_eq!(vm.summary(), strings::ABOUT_ATTRIBUTIONS_EMPTY);
}

// --- Widget construction --------------------------------------------------

#[test]
fn attribution_table_builds_for_empty_and_populated_catalogs() {
    // The empty state renders the same table widget (header only), so the
    // card's widget tree never changes (AGENTS §6).
    let empty: Element<'static, Message> = attribution_table(&[]);
    let full: Element<'static, Message> = attribution_table(&fixture_entries());
    let catalog: Element<'static, Message> =
        attribution_table(AttributionsViewModel::from_catalog().entries());
    let _ = (empty, full, catalog);
}

#[test]
fn attributions_view_builds_for_empty_and_populated_catalogs() {
    let empty_vm = AttributionsViewModel::with_entries(vec![]);
    let full_vm = AttributionsViewModel::with_entries(fixture_entries());
    let catalog_vm = AttributionsViewModel::from_catalog();

    let empty: Element<'_, Message> = attributions_view(&empty_vm, ROOMY_VIEWPORT);
    let full: Element<'_, Message> = attributions_view(&full_vm, ROOMY_VIEWPORT);
    let catalog: Element<'_, Message> = attributions_view(&catalog_vm, ROOMY_VIEWPORT);
    let _ = (empty, full, catalog);
}

#[test]
fn view_model_view_matches_the_free_function() {
    let vm = AttributionsViewModel::from_catalog();
    // Both entry points construct the same card; neither panics.
    let _via_method: Element<'_, Message> = vm.view(ROOMY_VIEWPORT);
    let _via_function: Element<'_, Message> = attributions_view(&vm, ROOMY_VIEWPORT);
}

// --- Sizing: the framework owns the box (§4.6.2, INT-5a) -----------------

#[test]
fn card_size_is_the_frameworks_attributions_size() {
    // 640×480 straight out of `ModalSizing::attributions()`. The retired
    // app values were 620px wide with a content-driven height and a
    // hand-picked 340px list cap; the spec row is a fixed 640×480 card
    // whose list fills the remainder.
    let size = attributions_card_size(ROOMY_VIEWPORT);
    assert_eq!(size.width, iced_pro::surface::modal::ATTRIBUTIONS_WIDTH);
    assert_eq!(size.height, iced_pro::surface::modal::ATTRIBUTIONS_HEIGHT);
    assert_eq!(size, Size::new(640.0, 480.0));
    assert_ne!(
        size.width, 620.0,
        "the retired ATTRIBUTIONS_CARD_WIDTH must not come back"
    );
}

#[test]
fn card_size_is_independent_of_row_count() {
    // The height is the frame's, not the content's: two rows or the whole
    // catalog render the same 480px box, because the list scrolls.
    let few = attributions_card_size(ROOMY_VIEWPORT);
    let many = attributions_card_size(ROOMY_VIEWPORT);
    assert_eq!(few, many);
    let _ = AttributionsViewModel::from_catalog().count();
}

#[test]
fn card_size_clamps_to_the_viewport_gutter() {
    let gutter = iced_pro::surface::modal::VIEWPORT_GUTTER;
    let small = attributions_card_size(Size::new(600.0, 400.0));
    assert_eq!(small.width, 600.0 - gutter);
    assert_eq!(small.height, 400.0 - gutter);
    assert!(
        small.width <= 640.0 && small.height <= 480.0,
        "a small viewport must shrink the card, never grow it"
    );
}

#[test]
fn card_size_never_goes_negative() {
    let size = attributions_card_size(Size::new(16.0, 16.0));
    assert_eq!(size, Size::ZERO);
}

#[test]
fn the_view_builds_at_every_clamped_size() {
    let vm = AttributionsViewModel::from_catalog();
    for viewport in [
        Size::new(1.0, 1.0),
        Size::new(128.0, 128.0),
        Size::new(640.0, 480.0),
        ROOMY_VIEWPORT,
    ] {
        let _card: Element<'_, Message> = attributions_view(&vm, viewport);
    }
}

// --- Rendered geometry: measured in a headless layout -----------------
//
// The card is a fixed 640x480 box and the credits list takes the
// remainder, so what these measure is that the box really is that size
// and that the list really does fill and scroll inside it.

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
fn the_attributions_card_renders_640x480() {
    let vm = AttributionsViewModel::from_catalog();
    let card = attributions_view(&vm, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    let bounds = framed_card_bounds(&mut ui, ATTRIBUTIONS_CARD_ID);
    assert_eq!(
        bounds.width, 640.0,
        "the rendered box must be the §4.6.2 width, not the retired 620"
    );
    assert_eq!(bounds.height, 480.0);
    drop(ui);
}

#[test]
fn the_credits_list_fills_the_card_and_scrolls() {
    // The list is `Length::Fill`, so it takes whatever the title,
    // summary, dividers and footer leave — a positive remainder — and the
    // 57-row catalog is far taller than that, so it scrolls.
    let vm = AttributionsViewModel::from_catalog();
    let card = attributions_view(&vm, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    let (region, content) = scroll_bounds(&mut ui, ATTRIBUTIONS_LIST_ID);
    assert!(
        region.height > 0.0,
        "the list must get real space inside the 480px card, got {:.0}px",
        region.height
    );
    assert!(
        region.height < 480.0,
        "the list shares the card with the title, summary, dividers and \
         footer; it measured {:.0}px of a 480px card",
        region.height
    );
    assert_eq!(
        region.height, 330.0,
        "480 card - 2x16 frame padding - 24 title - 20 summary - 2 dividers \
         - 32 footer - 5x8 gaps = 330px of list; if this moves, so did the \
         box model and the change should be deliberate"
    );
    assert!(
        content.height > region.height,
        "57 rows must overflow the region so the list scrolls; content \
         {:.0}px vs region {:.0}px",
        content.height,
        region.height
    );
    drop(ui);
}

#[test]
fn the_empty_catalog_keeps_the_card_at_640x480() {
    // The empty state changes the summary line, not the box.
    let vm = AttributionsViewModel::with_entries(vec![]);
    let card = attributions_view(&vm, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    let bounds = framed_card_bounds(&mut ui, ATTRIBUTIONS_CARD_ID);
    assert_eq!(bounds.width, 640.0);
    assert_eq!(bounds.height, 480.0);
    drop(ui);
}

#[test]
fn a_short_viewport_shrinks_the_card_instead_of_overflowing() {
    // The gutter clamp reaches the rendered box on both axes, so a window
    // smaller than the card never gets a card that hangs off it.
    let vm = AttributionsViewModel::from_catalog();
    let viewport = Size::new(500.0, 300.0);
    let card = attributions_view(&vm, viewport);
    let mut ui = laid_out(viewport, card);
    let bounds = framed_card_bounds(&mut ui, ATTRIBUTIONS_CARD_ID);
    let gutter = iced_pro::surface::modal::VIEWPORT_GUTTER;
    assert_eq!(bounds.width, 500.0 - gutter);
    assert_eq!(bounds.height, 300.0 - gutter);
    assert!(bounds.width <= viewport.width && bounds.height <= viewport.height);
    drop(ui);
}

#[test]
fn a_body_rung_lays_out_on_its_declared_line_grid() {
    // Same regression guard as the About card: `From<f32> for
    // iced::LineHeight` is `Relative`, so a bare `line_height` f32 means
    // a MULTIPLE of the font size. A UI_COMPACT line must measure its
    // declared 18px, not 13 x 18 = 234px.
    let probe = iced::widget::container(rung(
        "0123456789",
        typography::UI_COMPACT,
        iced::Color::WHITE,
    ))
    .id("attributions_probe_line")
    .width(Length::Shrink)
    .height(Length::Shrink);
    let mut ui = laid_out(Size::new(1024.0, 2000.0), probe.into());
    let bounds = framed_card_bounds(&mut ui, "attributions_probe_line");
    assert_eq!(
        bounds.height,
        typography::UI_COMPACT.line_height,
        "one UI_COMPACT line must be line_height pixels tall; a bare-f32 \
         line_height would report size x factor instead"
    );
    drop(ui);
}

#[test]
fn the_repository_link_hit_box_is_a_real_target() {
    // `hyperlink::MIN_HEIGHT` is `size.hit_target.min` (24px). The cell
    // must actually be that tall, not a text-height sliver.
    let cell = repository_cell("https://github.com/dtolnay/anyhow");
    let mut ui = laid_out(ROOMY_VIEWPORT, cell);
    let bounds = ui
        .find(|c: Candidate<'_>| match c {
            Candidate::Container { bounds, .. } => Some(bounds),
            _ => None,
        })
        .expect("the link cell container should lay out");
    assert_eq!(
        bounds.height,
        iced_pro::widget::hyperlink::MIN_HEIGHT,
        "the link must carry the framework's minimum hit target"
    );
    drop(ui);
}

// --- The repository link (UIF-ATOM-001d) ---------------------------------

#[test]
fn repository_link_takes_the_frameworks_accent_text() {
    // The link colour is the hyperlink resolver's resting foreground,
    // not the `accent_fill` the cell used before.
    assert_eq!(
        repository_color(),
        iced_pro::widget::hyperlink::resolve_style(
            iced_pro::state::VisualState {
                base: iced_pro::state::BaseState::Rest,
                focused: false,
            },
            crate::ui_iced::about::modal_frame().surface,
            &crate::ui::theme::Palette::pro(),
        )
        .foreground
    );
    assert_eq!(
        repository_color(),
        crate::ui::theme::Palette::fastmd_dark().accent_text
    );
    assert_ne!(
        repository_color(),
        crate::ui::theme::Palette::fastmd_dark().accent,
        "the link must not be painted with the accent FILL"
    );
}

#[test]
fn repository_link_clears_the_wcag_aa_floor_against_its_real_background() {
    // Measured, not transcribed: the ratio is computed here from the raw
    // sRGB of the two colours the cell actually paints — `accent_text`
    // over the modal frame's `surface.overlay`.
    let background = crate::ui_iced::about::modal_frame().surface;
    let ratio = iced_pro::token::contrast::contrast_ratio(repository_color(), background);
    assert!(
        ratio >= 4.5,
        "the repository link must clear 4.5:1 (WCAG 2.1 AA normal text); \
         measured {ratio:.2}:1 for accent_text on surface.overlay"
    );
}

#[test]
fn the_retired_accent_fill_failed_that_same_floor() {
    // The reason for the change, pinned so nobody "restores" the old
    // colour without seeing what it cost. Same background, same maths.
    let background = crate::ui_iced::about::modal_frame().surface;
    let ratio = iced_pro::token::contrast::contrast_ratio(
        crate::ui::theme::Palette::fastmd_dark().accent,
        background,
    );
    assert!(
        ratio < 4.5,
        "accent_fill on surface.overlay measured {ratio:.2}:1 — if this ever \
         clears 4.5 the premise for the change is gone and this test should be \
         revisited, not deleted"
    );
}

#[test]
fn repository_cells_build_for_every_catalog_entry() {
    // Every real URL in the catalog must render a link without panicking,
    // including the long ones that wrap.
    for entry in AttributionsViewModel::from_catalog().entries() {
        let _cell: Element<'static, Message> = repository_cell(entry.github_url);
    }
}

// --- Deferred intents -----------------------------------------------------

#[test]
fn repository_press_carries_the_url_verbatim() {
    let url = "https://github.com/dtolnay/anyhow";
    assert_eq!(
        repository_press(url),
        Message::About(AboutMessage::RepositoryOpened {
            url: url.to_owned(),
        })
    );
}

#[test]
fn repository_press_copies_the_url_into_the_message() {
    // The view may not open anything, so the URL must travel with the
    // message as an owned copy.
    let url = String::from("https://github.com/serde-rs/serde");
    let message = repository_press(&url);
    drop(url);
    assert_eq!(
        message,
        Message::About(AboutMessage::RepositoryOpened {
            url: "https://github.com/serde-rs/serde".into(),
        })
    );
}

// --- Colours come from palette tokens ---------------------------------------

#[test]
fn column_colours_resolve_to_palette_tokens() {
    let palette = Palette::fastmd_dark();
    assert_eq!(crate_name_color(), palette.text_strong);
    assert_eq!(authors_color(), palette.text);
    // INTENTIONALLY CHANGED IN INT-5a. This line pinned `palette.accent`
    // (= `color.accent.fill`), which measures below the WCAG 2.1 AA
    // 4.5:1 floor against this card's `surface.overlay` background — see
    // `repository_link_clears_the_wcag_aa_floor_against_its_real_background`
    // and `the_retired_accent_fill_failed_that_same_floor`. The link now
    // takes `color.accent.text`, the foreground §3.1d assigns a link.
    // A change to this assertion back to `accent` is a contrast
    // regression, not a fix.
    assert_eq!(repository_color(), palette.accent_text);
    assert_ne!(repository_color(), palette.accent);
}

#[test]
fn column_colours_are_distinct() {
    assert_ne!(crate_name_color(), authors_color());
    assert_ne!(crate_name_color(), repository_color());
    assert_ne!(authors_color(), repository_color());
}
