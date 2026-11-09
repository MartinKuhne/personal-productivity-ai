//! Sidecar unit tests for `ui_iced/attributions.rs`.
//!
//! This module is the credits **renderer**: the catalog snapshot, the table,
//! the scroll area and the link cells. The tests here therefore cover the
//! data, the widget tree the renderer builds, the geometry the renderer is
//! *given*, and the deferred intent a link emits.
//!
//! What used to be here and is now owned elsewhere, because the credits list
//! is a region of the About card rather than a window of its own:
//!
//! | Case | Where it lives now |
//! |---|---|
//! | Card size, viewport fill, gutter clamp | [`crate::ui_iced::about_tests`] (`the_card_fills_the_viewport_on_both_axes`) |
//! | The solved list height, the cap, the `Fill` fallback | [`crate::ui_iced::about_tests`] (`the_credits_region_is_solved_from_the_card_it_sits_in`) |
//! | The list never crossing the footer band | [`crate::ui_iced::about_tests`] (`the_credits_list_is_inside_the_card_and_gets_its_solved_height`) |
//! | Short-viewport collapse | [`crate::ui_iced::about_tests`] (`a_short_viewport_collapses_the_credits_region_without_losing_it`) |
//!
//! Cases that share a fixture are grouped into one process: the catalog
//! integrity checks read one snapshot, and each headless-layout group lays
//! its widget out once and then measures everything that layout can answer.
//! Every check names its case in the failure message.
//!
//! No test reads the filesystem, the bus or a renderer ([RUST-006]); the
//! catalog is compile-time data.

use super::*;
use crate::ui::strings;
use crate::ui_iced::about::ABOUT_CREDITS_FOOTER_BAND_HEIGHT;
use hoarfrost::surface::modal::ATTRIBUTIONS_LIST_MAX_HEIGHT;
use hoarfrost::token::size;
use iced::{Element, Point, Size};
use iced_test::Selector;
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
    // card can show.
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

/// The table and the framed scroll area build for every catalog shape —
/// empty, synthetic and the shipped catalog.
///
/// The empty state renders the same table widget (header only), so the
/// widget tree never changes (AGENTS §6).
#[test]
fn the_table_and_the_scroll_area_build_for_every_catalog_shape() {
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
        // The height is handed in by the host — the About card solves it.
        let list = credits_list(table, Length::Fixed(ATTRIBUTIONS_LIST_MAX_HEIGHT));
        let element: Element<'_, Message> = list.into();

        // The assertion is that the tree exists and is addressable: a shape
        // that failed to render would panic on build or be unreachable here.
        let mut ui = laid_out(ROOMY_VIEWPORT, element);
        let (region, _) = scroll_bounds(&mut ui, ATTRIBUTIONS_LIST_ID);
        assert_eq!(
            region.height, ATTRIBUTIONS_LIST_MAX_HEIGHT,
            "{label}: the scroll area is in the tree and carries the length it was given"
        );
    }
}

// --- Headless layout helpers ----------------------------------------------

type Sim<'a> = iced_test::Simulator<'a, Message, iced::Theme, iced::Renderer>;

/// Lays `element` out in a window of `viewport` size, headless.
fn laid_out(viewport: Size, element: Element<'_, Message>) -> Sim<'_> {
    iced_test::Simulator::with_size(iced_test::core::Settings::default(), viewport, element)
}

/// The laid-out bounds of the container carrying `id`.
fn framed_card_bounds(ui: &mut Sim<'_>, id: &'static str) -> iced::Rectangle {
    ui.find(iced::widget::Id::from(id))
        .unwrap_or_else(|err| panic!("container {id:?} should be reachable: {err:?}"))
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

/// The `nth` `Candidate::Text` whose content equals `label`.
///
/// `Simulator::find` stops at the first match, so proving a label appears
/// **exactly once** needs a selector that counts the ones it skipped.
fn nth_text(label: &str, nth: usize) -> impl Selector<Output = Target> {
    let mut skipped = 0usize;
    move |candidate: Candidate<'_>| {
        let is_match = matches!(candidate, Candidate::Text { content, .. } if content == label);
        if is_match {
            if skipped == nth {
                return Some(Target::from(candidate));
            }
            skipped += 1;
        }
        None
    }
}

/// Points the cursor at `position` and left-clicks it.
fn click_at(ui: &mut Sim<'_>, position: Point) {
    ui.point_at(position);
    let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::CursorMoved {
        position,
    })]);
    let _ = ui.simulate([
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ]);
}

/// The credits list laid out on its own at a fixed length, so the renderer
/// can be measured without the card that normally hosts it.
fn list_at_length(entries: &[Attribution], length: Length) -> Element<'_, Message> {
    credits_list(attribution_table(entries), length).into()
}

// --- The scroll area ------------------------------------------------------

/// [`credits_list`] is a wrapper, not a layout decision: it stamps the stable
/// id ([`ATTRIBUTIONS_LIST_ID`]) and hands iced the [`Length`] the host solved.
/// It never invents a height of its own, and it keeps the widget in the tree
/// even when the host solved zero room — the case the `Fill` fallback in
/// [`crate::ui_iced::about::credits_list_length`] exists for.
#[test]
fn the_scroll_area_honours_the_length_the_host_solved() {
    let vm = AttributionsViewModel::from_catalog();

    // The capped path: the host handed in the §2.2 cap.
    let mut ui = laid_out(
        ROOMY_VIEWPORT,
        list_at_length(vm.entries(), Length::Fixed(ATTRIBUTIONS_LIST_MAX_HEIGHT)),
    );
    let (region, content) = scroll_bounds(&mut ui, ATTRIBUTIONS_LIST_ID);
    drop(ui);
    assert_eq!(
        region.height, ATTRIBUTIONS_LIST_MAX_HEIGHT,
        "Fixed(200): the rendered region is exactly the length handed in, not a \
         height the wrapper chose"
    );
    assert!(
        content.height > region.height,
        "the {}-row catalog overflows a 200px region, so the list scrolls: \
         content {:.1}px vs region {:.1}px",
        vm.count(),
        content.height,
        region.height,
    );

    // The zero-room path: `Fill` keeps the widget addressable instead of a
    // void `Fixed(0.0)`, which iced would drop from the tree entirely.
    let mut ui = laid_out(ROOMY_VIEWPORT, list_at_length(vm.entries(), Length::Fill));
    let (region, _) = scroll_bounds(&mut ui, ATTRIBUTIONS_LIST_ID);
    drop(ui);
    assert!(
        region.height > 0.0,
        "Fill: the scrollable is present and takes the viewport it was given"
    );
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

    let mut ui = laid_out(
        ROOMY_VIEWPORT,
        list_at_length(&big, Length::Fixed(ATTRIBUTIONS_LIST_MAX_HEIGHT)),
    );
    let (list, content) = scroll_bounds(&mut ui, ATTRIBUTIONS_LIST_ID);
    drop(ui);

    assert_eq!(
        list.height, ATTRIBUTIONS_LIST_MAX_HEIGHT,
        "5,000 rows must not grow the region past the length it was given"
    );
    assert!(
        content.height > 100_000.0,
        "5,000 rows should be a very tall content extent, got {:.0}",
        content.height
    );
}

// --- The table ------------------------------------------------------------

/// The table renders one row per crate: every fixture crate name and author
/// appears as its own text run, and nothing else does.
#[test]
fn the_table_renders_one_row_per_crate() {
    let entries = fixture_entries();
    let table = attribution_table(&entries);
    let mut ui = laid_out(ROOMY_VIEWPORT, table);

    for entry in &entries {
        assert!(
            ui.find(nth_text(entry.name, 0)).is_ok(),
            "row for {}: the crate name must be rendered",
            entry.name
        );
        assert!(
            ui.find(nth_text(entry.authors, 0)).is_ok(),
            "row for {}: the authors cell must be rendered",
            entry.name
        );
        // The repository cell is `rich_text`, which iced's widget visitor does
        // not report as a `Candidate::Text` — it is reached as a hit target
        // instead (`the_repository_link_hit_box_is_a_real_target` and
        // `pressing_the_repository_link_raises_the_deferred_intent`).
        // Exactly once: a row that rendered twice would double-count the list.
        assert!(
            ui.find(nth_text(entry.name, 1)).is_err(),
            "row for {}: the crate name must appear once, not twice",
            entry.name
        );
    }

    for caption in [
        strings::ABOUT_COL_CRATE,
        strings::ABOUT_COL_AUTHORS,
        strings::ABOUT_COL_REPO,
    ] {
        assert!(
            ui.find(nth_text(caption, 0)).is_ok(),
            "header: the column caption {caption:?} must be rendered"
        );
    }
    drop(ui);
}

/// The empty catalog renders the header row and no data rows, so the shape of
/// the tree does not change with the catalog (AGENTS §6).
#[test]
fn an_empty_catalog_renders_the_header_only_table() {
    let mut ui = laid_out(
        ROOMY_VIEWPORT,
        list_at_length(&[], Length::Fixed(ATTRIBUTIONS_LIST_MAX_HEIGHT)),
    );
    let (region, content) = scroll_bounds(&mut ui, ATTRIBUTIONS_LIST_ID);

    for caption in [
        strings::ABOUT_COL_CRATE,
        strings::ABOUT_COL_AUTHORS,
        strings::ABOUT_COL_REPO,
    ] {
        assert!(
            ui.find(nth_text(caption, 0)).is_ok(),
            "empty catalog: the caption {caption:?} still renders — the empty \
             state is an empty table, not a different widget"
        );
    }
    assert!(
        content.height <= region.height,
        "empty catalog: an empty table must not overflow its region: content \
         {:.1} vs region {:.1}",
        content.height,
        region.height,
    );
    drop(ui);
}

/// The row grid the table lays out is the declared one: a `UI_COMPACT` line
/// measures its own leading, not `size × factor`. Same regression guard as the
/// About card, measured on this module's own `rung` helper.
#[test]
fn a_body_rung_lays_out_on_its_declared_line_grid() {
    // `From<f32> for iced::LineHeight` is `Relative`, so a bare `line_height`
    // f32 means a MULTIPLE of the font size. A UI_COMPACT line must measure
    // its declared 18px, not 13 x 18 = 234px.
    let probe = container(rung(
        "0123456789",
        typography::UI_COMPACT,
        crate_name_color(),
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

/// The footer band the host reserves is at least the footer's own painted
/// extent, so the arithmetic in
/// [`crate::ui_iced::about::credits_list_height`] cannot understate the space
/// below the list by claiming less than the footer actually paints.
#[test]
fn the_footer_band_covers_what_the_footer_paints() {
    let painted = shape::BORDER_HAIRLINE + size::CONTROL_MD;
    assert!(
        ABOUT_CREDITS_FOOTER_BAND_HEIGHT >= painted,
        "the band must cover the divider ({:.1}) plus the footer row ({:.1}); \
         the band is {:.1}",
        shape::BORDER_HAIRLINE,
        size::CONTROL_MD,
        ABOUT_CREDITS_FOOTER_BAND_HEIGHT,
    );
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

/// The link is not only tall, it is **pressable**: a click inside the hit box
/// raises the deferred intent, and it raises exactly one message.
///
/// This is the tier-3 half of the claim the colour and geometry tests make —
/// they prove the cell looks like a link and is as big as a link; only a
/// simulated press proves the `mouse_area` is wired to anything.
#[test]
fn pressing_the_repository_link_raises_the_deferred_intent() {
    let url = "https://github.com/dtolnay/anyhow";
    let mut ui = laid_out(ROOMY_VIEWPORT, repository_cell(url));
    let bounds = ui
        .find(|c: Candidate<'_>| match c {
            Candidate::Container { bounds, .. } => Some(bounds),
            _ => None,
        })
        .expect("the link cell container should lay out");
    click_at(&mut ui, bounds.center());

    let messages: Vec<Message> = ui.into_messages().collect();
    assert_eq!(
        messages,
        vec![Message::About(AboutMessage::RepositoryOpened {
            url: url.to_owned(),
        })],
        "one press of one link must raise exactly one deferred intent, and the \
         reducer — not the view — is the thing that opens the url"
    );
}

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
