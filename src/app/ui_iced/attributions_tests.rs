//! Sidecar unit tests for `ui_iced/attributions.rs`.
//!
//! Pure view-model and widget-construction tests ([RUST-003]). The catalog
//! is compile-time data, so these tests read it directly and assert the
//! snapshot preserves it exactly — no renderer, no bus, no filesystem
//! ([RUST-006]).
//!
//! Cases that share a fixture are grouped into one process: the catalog
//! integrity checks read one snapshot, the sizing cases solve one table of
//! viewports, and each headless-layout group lays the card out once and then
//! measures everything that layout can answer. Every check names its case in
//! the failure message.

use super::*;
use crate::ui::strings;
use crate::ui_iced::about::{AboutViewModel, LICENSE_MAX_HEIGHT, LICENSE_SCROLL_ID};
use hoarfrost::surface::modal::ATTRIBUTIONS_LIST_MAX_HEIGHT;
use iced::Element;
use iced_test::selector::{Candidate, Target};

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

/// The snapshot carries the shared catalog exactly as it is: same rows, same
/// order, no duplicates, every row complete, and independent of the next read.
#[test]
fn from_catalog_is_the_shared_catalog_verbatim() {
    let vm = AttributionsViewModel::from_catalog();

    assert_eq!(
        vm.entries(),
        DIRECT_DEPENDENCIES,
        "verbatim: the view must show the shared catalog unchanged"
    );
    assert!(
        !vm.is_empty() && vm.count() == DIRECT_DEPENDENCIES.len(),
        "non-empty: the attribution catalog must carry rows"
    );

    // The catalog's row shape is `name` / `authors` / `github_url` — there
    // is no per-crate license or version field, so these three are what the
    // window can show.
    for entry in vm.entries() {
        assert!(
            !entry.name.is_empty(),
            "row shape: empty crate name in {entry:?}"
        );
        assert!(
            !entry.authors.is_empty(),
            "row shape: empty authors for {}",
            entry.name
        );
        assert!(
            entry.github_url.starts_with("https://github.com/"),
            "row shape: repository URL for {} must be an https GitHub link, got {}",
            entry.name,
            entry.github_url
        );
    }

    // Mirrors `ui::attributions_tests::slice_is_sorted_and_unique`: the
    // snapshot must not reorder anything, so two runs render identically.
    let names: Vec<&str> = vm.entries().iter().map(|entry| entry.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        names, sorted,
        "ordering: the rendered order must be the catalog order"
    );

    let mut seen = std::collections::HashSet::new();
    for entry in vm.entries() {
        assert!(
            seen.insert(entry.name),
            "uniqueness: duplicate crate name {}",
            entry.name
        );
    }

    // The snapshot owns its `Vec`, so two snapshots are equal but not
    // aliasing the same allocation-dependent state.
    let other = AttributionsViewModel::from_catalog();
    assert_eq!(
        vm, other,
        "snapshot independence: two reads of the catalog are equal"
    );
    assert_eq!(
        vm.count(),
        other.count(),
        "snapshot independence: and they carry the same number of rows"
    );
}

// --- Summary line --------------------------------------------------------

/// The summary line is the count when there are rows and the empty-state line
/// when there are none — whichever empty construction produced the model.
#[test]
fn summary_counts_the_entries_or_reports_the_empty_state() {
    let populated = AttributionsViewModel::with_entries(fixture_entries());
    assert_eq!(
        populated.count(),
        2,
        "two-row catalog: the count is the row count"
    );
    assert_eq!(
        populated.summary(),
        strings::format_attribution_count(2),
        "two-row catalog: the summary names the count"
    );
    assert!(
        populated.summary().contains("2"),
        "two-row catalog: the summary carries the digit"
    );

    let empty_cases = [
        (
            "with_entries(vec![])",
            AttributionsViewModel::with_entries(Vec::new()),
        ),
        (
            "AttributionsViewModel::default()",
            AttributionsViewModel::default(),
        ),
    ];
    for (label, vm) in &empty_cases {
        assert!(
            vm.is_empty(),
            "{label}: an empty catalog reports itself as empty"
        );
        assert_eq!(
            vm.summary(),
            strings::ABOUT_ATTRIBUTIONS_EMPTY,
            "{label}: the empty catalog reports the empty-state line"
        );
    }
}

// --- Widget construction --------------------------------------------------

/// The table and the framed card build for every catalog shape — empty,
/// synthetic and the shipped catalog — through both view entry points.
///
/// The empty state renders the same table widget (header only), so the card's
/// widget tree never changes (AGENTS §6).
#[test]
fn the_table_and_the_view_build_for_every_catalog_shape() {
    let models = [
        (
            "empty catalog",
            AttributionsViewModel::with_entries(Vec::new()),
        ),
        (
            "two-row catalog",
            AttributionsViewModel::with_entries(fixture_entries()),
        ),
        ("the shipped catalog", AttributionsViewModel::from_catalog()),
    ];

    for (label, vm) in &models {
        let table: Element<'static, Message> = attribution_table(vm.entries());
        let view: Element<'_, Message> = attributions_view(vm, ROOMY_VIEWPORT);
        // Both entry points construct the same card; neither panics.
        let via_method: Element<'_, Message> = vm.view(ROOMY_VIEWPORT);
        let _ = (label, table, view, via_method);
    }
}

// --- Sizing: the framework owns the box (§4.6.2, INT-5a) -----------------

/// The card is 640×480 straight out of `ModalSizing::attributions()`, clamped
/// to the viewport gutter and never negative. The retired app values were
/// 620px wide with a content-driven height and a hand-picked 340px list cap;
/// the spec row is a fixed 640×480 card whose list fills the remainder.
#[test]
fn card_size_is_the_frameworks_attributions_size_clamped_to_the_viewport() {
    let gutter = hoarfrost::surface::modal::VIEWPORT_GUTTER;
    let cases = [
        ("roomy viewport", ROOMY_VIEWPORT, Size::new(640.0, 480.0)),
        (
            "small viewport",
            Size::new(600.0, 400.0),
            Size::new(600.0 - gutter, 400.0 - gutter),
        ),
        (
            "a viewport smaller than the card",
            Size::new(16.0, 16.0),
            Size::ZERO,
        ),
    ];

    for (label, viewport, expected) in &cases {
        let size = attributions_card_size(*viewport);
        assert_eq!(
            size, *expected,
            "{label}: the solved box must be the framework policy clamped to the viewport"
        );
        // The height is the frame's, not the content's: two rows or the whole
        // catalog render the same box, because the list scrolls.
        assert_eq!(
            attributions_card_size(*viewport),
            size,
            "{label}: the policy is deterministic across repeated solves"
        );
    }

    let roomy = attributions_card_size(ROOMY_VIEWPORT);
    assert_eq!(
        roomy.width,
        hoarfrost::surface::modal::ATTRIBUTIONS_WIDTH,
        "roomy viewport: the width is the framework's own constant"
    );
    assert_eq!(
        roomy.height,
        hoarfrost::surface::modal::ATTRIBUTIONS_HEIGHT,
        "roomy viewport: the height is the framework's own constant"
    );
    assert_ne!(
        roomy.width, 620.0,
        "roomy viewport: the retired ATTRIBUTIONS_CARD_WIDTH must not come back"
    );

    let small = attributions_card_size(Size::new(600.0, 400.0));
    assert!(
        small.width <= 640.0 && small.height <= 480.0,
        "small viewport: a small viewport must shrink the card, never grow it"
    );
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

/// **Replaces `the_credits_list_fills_the_card_and_scrolls`.**
///
/// The retired test pinned the list at `330.0` — the uncapped `Length::Fill`
/// remainder. `UIF-MOD-004` requires the sub-region to be *solved and
/// capped*, at `ATTRIBUTIONS_LIST_MAX_HEIGHT = 200`, so that pin recorded
/// a height 130px above the number the §2.2 closed table names. The test
/// is rewritten around the cap rather than its number quietly edited: what
/// is asserted now is that the box is 640×480, that the cap binds, that it
/// arrived from the solver rather than being restated by the view, that the
/// list still scrolls, and what the cap costs.
///
/// **The cost of the cap, measured and recorded rather than buried.** The spec
/// pins the card at 480px *and* caps the list at 200px. Those two numbers are
/// in tension for this card: the furniture is 150px, so the card has 330px to
/// give and the cap only lets 200 of it be used. The remaining **130px is dead
/// card** — nothing is laid out there — and the list loses roughly **4.4
/// visible rows**. That trade-off is asserted here as a pinned, visible fact
/// rather than something a reader has to recompute to notice; if the spec
/// author resolves it (by shrinking the card, raising the cap, or letting the
/// card content-fit), this is the test that changes.
#[test]
fn the_card_and_credits_list_measure_the_solved_geometry() {
    let vm = AttributionsViewModel::from_catalog();
    let card = attributions_view(&vm, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    let bounds = framed_card_bounds(&mut ui, ATTRIBUTIONS_CARD_ID);
    let (region, content) = scroll_bounds(&mut ui, ATTRIBUTIONS_LIST_ID);
    drop(ui);

    assert_eq!(
        bounds.width, 640.0,
        "rendered box: must be the §4.6.2 width, not the retired 620"
    );
    assert_eq!(
        bounds.height, 480.0,
        "rendered box: must be the §4.6.2 height"
    );

    assert_eq!(
        region.height, ATTRIBUTIONS_LIST_MAX_HEIGHT,
        "credits list: the card leaves 330px of remainder; §2.2 caps the list at 200, so \
         the rendered region must be the cap"
    );
    assert_eq!(
        region.height,
        credits_list_height(attributions_card_size(ROOMY_VIEWPORT).height),
        "credits list: the rendered height is the solver's answer, not a number the view \
         restates"
    );
    assert!(
        content.height > region.height,
        "credits list: the {}-row catalog overflows the capped region, so the list still \
         scrolls: content {:.1}px vs region {:.1}px",
        vm.count(),
        content.height,
        region.height,
    );

    let rows = vm.count() as f32;
    assert_eq!(rows, 61.0, "fixture: the shipped catalog is 61 crates");
    let row_h = content.height / rows;
    let uncapped = 330.0;

    // The dead space the cap creates inside the fixed 480px card.
    let dead = uncapped - region.height;
    assert_eq!(
        dead, 130.0,
        "dead card: 330px remainder - 200px cap = 130px unused"
    );

    // What that costs in visible rows.
    let rows_before = uncapped / row_h;
    let rows_after = region.height / row_h;
    let lost = rows_before - rows_after;
    assert!(
        lost > 4.0 && lost < 5.0,
        "visible rows: the cap costs {lost:.2} visible rows ({rows_before:.2} -> {rows_after:.2} \
         at {row_h:.2}px/row) — a real regression in what a reader can see \
         without scrolling, recorded deliberately"
    );
}

/// **Overflow safety, re-measured after replacing `Fill` with a solved
/// fixed height.**
///
/// `Fill` made overflow structurally impossible. A solved `Length::Fixed`
/// is safe only because the band terms it was computed from are the same
/// terms this `Column` actually consumes — so `solved + bands` equals the
/// card's inner height and the list lands exactly where `Fill` would have
/// put it, never further. An *understated* band would over-solve and push
/// the list through the footer, which is the exact hazard `UIF-MOD-004`
/// names. This test is what makes that hazard observable rather than
/// theoretical.
#[test]
fn the_credits_list_never_crosses_the_footer_band_at_any_viewport() {
    let frame = modal_frame();
    // The footer's own painted extent: the divider above it plus the
    // control row. The gaps above that are not painted, so the hard limit
    // is this, not the full footer band.
    let footer_paint = shape::BORDER_HAIRLINE + size::CONTROL_MD;

    // Every viewport tall enough to hold the card's own 150px of furniture
    // plus its 32px of padding.
    for (w, h) in [
        (1280.0f32, 800.0f32),
        (900.0, 600.0),
        (700.0, 520.0),
        (500.0, 300.0),
        (420.0, 214.0),
    ] {
        let viewport = Size::new(w, h);
        let vm = AttributionsViewModel::from_catalog();
        let card = attributions_view(&vm, viewport);
        let mut ui = laid_out(viewport, card);
        let card_b = framed_card_bounds(&mut ui, ATTRIBUTIONS_CARD_ID);
        let (list, _) = scroll_bounds(&mut ui, ATTRIBUTIONS_LIST_ID);
        drop(ui);

        let inner_bottom = card_b.y + card_b.height - frame.content_padding;
        let footer_top = inner_bottom - footer_paint;
        let list_bottom = list.y + list.height;

        assert!(
            list_bottom <= footer_top + 0.01,
            "{w}x{h}: the list bottom {list_bottom:.1} reached into the footer \
             band (footer top {footer_top:.1}) — the band terms are understated"
        );
        assert!(
            list_bottom <= card_b.y + card_b.height + 0.01,
            "{w}x{h}: the list escaped the card entirely \
             (list bottom {list_bottom:.1}, card bottom {:.1})",
            card_b.y + card_b.height
        );
        assert!(
            list.y >= card_b.y + frame.content_padding - 0.01,
            "{w}x{h}: the list started above the card's inner top"
        );
    }
}

/// The two short-viewport branches of the solver: a viewport too short for the
/// card's own furniture clamps the list to zero rather than producing a
/// negative height or panicking, and a short-but-viable card gives the list its
/// remainder instead of the cap.
///
/// The card's furniture is `150px` and its padding `32px`, so below a
/// `214px` viewport the card cannot hold itself. That condition is
/// pre-existing and independent of the sub-region solver — `Fill` hit it
/// too — but the solver is now the thing that decides what the list gets
/// there, so its clamp needs its own pin.
#[test]
fn the_solver_clamps_the_list_on_short_viewports() {
    // 360x180: below the card's own furniture.
    {
        let viewport = Size::new(360.0, 180.0);
        let vm = AttributionsViewModel::from_catalog();
        let card = attributions_view(&vm, viewport);
        let mut ui = laid_out(viewport, card);
        let (list, _) = scroll_bounds(&mut ui, ATTRIBUTIONS_LIST_ID);
        drop(ui);

        assert_eq!(
            list.height, 0.0,
            "360x180: 148px of card cannot hold 150px of furniture; the solver clamps to \
             zero rather than handing iced a negative extent"
        );
    }

    // 300px viewport -> 268px card -> 118px remainder, under the 200 cap.
    {
        let viewport = Size::new(900.0, 300.0);
        let vm = AttributionsViewModel::from_catalog();
        let card = attributions_view(&vm, viewport);
        let mut ui = laid_out(viewport, card);
        let (list, _) = scroll_bounds(&mut ui, ATTRIBUTIONS_LIST_ID);
        drop(ui);

        let card_h = attributions_card_size(viewport).height;
        let expected = card_h
            - (HEADER_BAND_HEIGHT + 2.0 * modal_frame().content_padding + FOOTER_BAND_HEIGHT);
        assert!(
            expected < ATTRIBUTIONS_LIST_MAX_HEIGHT,
            "900x300: fixture: this card is below the cap"
        );
        assert_eq!(
            list.height, expected,
            "900x300: below the cap the list gets everything the card has left"
        );
    }
}

/// A very large catalog does not change the region, only the scroll extent.
#[test]
fn a_very_large_catalog_changes_the_scroll_extent_not_the_region() {
    let big: Vec<Attribution> = (0..5_000)
        .map(|_| Attribution {
            name: "synthetic-crate",
            authors: "Nobody At All",
            github_url: "https://github.com/x/y",
        })
        .collect();

    let vm = AttributionsViewModel::with_entries(big);
    let card = attributions_view(&vm, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    let (list, content) = scroll_bounds(&mut ui, ATTRIBUTIONS_LIST_ID);
    drop(ui);

    assert_eq!(
        list.height, ATTRIBUTIONS_LIST_MAX_HEIGHT,
        "5,000 rows must not grow the region past the cap"
    );
    assert!(
        content.height > 100_000.0,
        "5,000 rows should be a very tall content extent, got {:.0}",
        content.height
    );
}

/// The About sibling is untouched by this change: it keeps its own cap and
/// never routes through the attributions solver.
#[test]
fn the_about_card_is_unaffected_and_keeps_its_own_cap() {
    assert_eq!(
        LICENSE_MAX_HEIGHT, ATTRIBUTIONS_LIST_MAX_HEIGHT,
        "both are 200 by coincidence of the spec, not by shared plumbing; if \
         one moves and the other does not, that is the point"
    );
    let vm = AboutViewModel::from_build();
    let card = crate::ui_iced::about::about_card(&vm, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    let (region, _) = scroll_bounds(&mut ui, LICENSE_SCROLL_ID);
    drop(ui);
    assert!(
        region.height <= LICENSE_MAX_HEIGHT + 0.01,
        "the About license list keeps its own cap: {:.1} vs {LICENSE_MAX_HEIGHT}",
        region.height
    );
}

/// Empty catalog: the list is still allocated, still capped, the card does not
/// collapse, and the empty table does not overflow its region.
#[test]
fn an_empty_catalog_keeps_the_card_and_the_capped_list() {
    // The empty state changes the summary line, not the box.
    let vm = AttributionsViewModel::with_entries(vec![]);
    let card = attributions_view(&vm, ROOMY_VIEWPORT);
    let mut ui = laid_out(ROOMY_VIEWPORT, card);
    let (list, content) = scroll_bounds(&mut ui, ATTRIBUTIONS_LIST_ID);
    let card_b = framed_card_bounds(&mut ui, ATTRIBUTIONS_CARD_ID);
    drop(ui);

    assert_eq!(
        list.height, ATTRIBUTIONS_LIST_MAX_HEIGHT,
        "empty catalog: the list is still allocated and still capped"
    );
    assert_eq!(
        card_b.width, 640.0,
        "empty catalog: the empty state must not resize the card"
    );
    assert_eq!(
        card_b.height, 480.0,
        "empty catalog: the empty state must not resize the card"
    );
    assert!(
        content.height <= list.height,
        "empty catalog: an empty table must not overflow the region: content {:.1} vs \
         region {:.1}",
        content.height,
        list.height
    );
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
    let gutter = hoarfrost::surface::modal::VIEWPORT_GUTTER;
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
        hoarfrost::widget::hyperlink::MIN_HEIGHT,
        "the link must carry the framework's minimum hit target"
    );
    drop(ui);
}

// --- The repository link (UIF-ATOM-001d) ---------------------------------

/// Every column colour is a palette token, the link takes the resolver's
/// resting foreground rather than the accent fill, and the three columns are
/// distinguishable from each other.
#[test]
fn column_colours_resolve_to_distinct_palette_tokens() {
    let palette = Palette::fastmd_dark();
    assert_eq!(
        crate_name_color(),
        palette.text_strong,
        "crate-name column: the row's primary label is text_strong"
    );
    assert_eq!(
        authors_color(),
        palette.text,
        "authors column: the secondary column is the body text token"
    );
    // INTENTIONALLY CHANGED IN INT-5a. This line pinned `palette.accent`
    // (= `color.accent.fill`), which measures below the WCAG 2.1 AA
    // 4.5:1 floor against this card's `surface.overlay` background — see
    // `the_link_colour_clears_the_floor_the_retired_accent_fill_failed`.
    // The link now takes `color.accent.text`, the foreground §3.1d assigns
    // a link. A change to this assertion back to `accent` is a contrast
    // regression, not a fix.
    assert_eq!(
        repository_color(),
        palette.accent_text,
        "repository column: the link takes color.accent.text"
    );
    assert_ne!(
        repository_color(),
        palette.accent,
        "repository column: the link must not be painted with the accent FILL"
    );

    // The link colour is the hyperlink resolver's resting foreground,
    // not the `accent_fill` the cell used before.
    assert_eq!(
        repository_color(),
        hoarfrost::widget::hyperlink::resolve_style(
            hoarfrost::state::VisualState {
                base: hoarfrost::state::BaseState::Rest,
                focused: false,
            },
            crate::ui_iced::about::modal_frame().surface,
            &crate::ui::theme::Palette::pro(),
        )
        .foreground,
        "resolver: the colour is the framework's own resting foreground"
    );

    assert_ne!(
        crate_name_color(),
        authors_color(),
        "distinctness: the crate-name column must differ from the authors column"
    );
    assert_ne!(
        crate_name_color(),
        repository_color(),
        "distinctness: the crate-name column must differ from the repository column"
    );
    assert_ne!(
        authors_color(),
        repository_color(),
        "distinctness: the authors column must differ from the repository column"
    );
}

/// The link colour clears the WCAG AA floor against the surface it is actually
/// painted on, and the retired accent fill did not — the reason for the change,
/// pinned so nobody "restores" the old colour without seeing what it cost.
#[test]
fn the_link_colour_clears_the_floor_the_retired_accent_fill_failed() {
    // Measured, not transcribed: the ratio is computed here from the raw
    // sRGB of the two colours the cell actually paints — `accent_text`
    // over the modal frame's `surface.overlay`.
    let background = crate::ui_iced::about::modal_frame().surface;

    let link = hoarfrost::token::contrast::contrast_ratio(repository_color(), background);
    assert!(
        link >= 4.5,
        "accent_text on surface.overlay: the repository link must clear 4.5:1 \
         (WCAG 2.1 AA normal text); measured {link:.2}:1"
    );

    // Same background, same maths.
    let retired = hoarfrost::token::contrast::contrast_ratio(
        crate::ui::theme::Palette::fastmd_dark().accent,
        background,
    );
    assert!(
        retired < 4.5,
        "accent_fill on surface.overlay: measured {retired:.2}:1 — if this ever \
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

/// The press carries an owned copy of the URL, so the view never borrows the
/// row it came from.
#[test]
fn repository_press_carries_an_owned_copy_of_the_url() {
    let url = "https://github.com/dtolnay/anyhow";
    assert_eq!(
        repository_press(url),
        Message::About(AboutMessage::RepositoryOpened {
            url: url.to_owned(),
        }),
        "borrowed url: the message carries the url verbatim"
    );

    // The view may not open anything, so the URL must travel with the
    // message as an owned copy.
    let owned = String::from("https://github.com/serde-rs/serde");
    let message = repository_press(&owned);
    drop(owned);
    assert_eq!(
        message,
        Message::About(AboutMessage::RepositoryOpened {
            url: "https://github.com/serde-rs/serde".into(),
        }),
        "owned url: the copy outlives the string it was made from"
    );
}
