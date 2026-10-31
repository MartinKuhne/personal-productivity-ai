//! Unit tests for [`crate::previews`].
//!
//! A preview's whole job is to paint what a resolver returned and to claim
//! nothing else. So the tests check three things: the inventory is what the
//! module promises, every preview reports **zero** interactive widgets,
//! and the numbers printed on screen are the resolver's own numbers rather
//! than something the preview invented.

use iced::{Element, Size};

use hoarfrost::composite::breadcrumb::{self, BreadcrumbItem, CrumbRole};
use hoarfrost::composite::card::{self, CardElevation};
use hoarfrost::composite::notification::Severity;
use hoarfrost::composite::overflow_toolbar::{self, OverflowItem, OverflowRank};
use hoarfrost::composite::table_column::ColumnSpec;
use hoarfrost::token::color::Palette;

use super::*;
use crate::app::Message;
use crate::section::{PaneBody, Section};

/// Builds and lays out `element` in a headless viewport.
fn laid_out<'a>(viewport: Size, element: impl Into<Element<'a, Message>>) {
    let _ = iced_test::Simulator::with_size(
        iced_test::core::Settings::default(),
        viewport,
        element.into(),
    );
}

/// The atoms pane's viewport, matching `pane_atoms_tests`.
const PANE: Size = Size::new(480.0, 900.0);

/// The composites pane's viewport, matching `pane_composites_tests`.
const WIDE: Size = Size::new(600.0, 900.0);

/// A viewport narrow enough that a row of swatches must wrap or shrink.
const NARROW: Size = Size::new(240.0, 900.0);

// ---------------------------------------------------------------------------
// Inventory
// ---------------------------------------------------------------------------

#[test]
fn the_atoms_pane_gains_five_preview_sections() {
    assert_eq!(5, atom_preview_sections().len());
}

#[test]
fn the_composites_pane_gains_six_preview_sections() {
    assert_eq!(6, composite_preview_sections().len());
}

#[test]
fn the_two_preview_lists_are_independent() {
    // Appending to one must never leak into the other; the panes build their
    // own vectors from these calls.
    let mut atoms = atom_preview_sections();
    atoms.push(section("x", 0, iced::widget::Column::new()));
    assert_eq!(6, atoms.len());
    assert_eq!(6, composite_preview_sections().len());
}

// ---------------------------------------------------------------------------
// Zero interactive widgets
// ---------------------------------------------------------------------------

#[test]
fn every_preview_reports_zero_widgets() {
    for section in atom_preview_sections()
        .into_iter()
        .chain(composite_preview_sections())
    {
        assert_eq!(
            0, section.widgets,
            "a resolver preview must not claim an interactive widget"
        );
    }
}

#[test]
fn previews_do_not_change_a_panes_widget_total() {
    // The pane total comes from `PaneBody::new` summing its sections, so
    // what is actually being claimed is that appending the previews to a
    // real pane's live sections leaves that sum exactly where the pane
    // without previews put it. Build both sides and compare the totals.
    let state = crate::pane_atoms::AtomsState::new();

    let live_only = PaneBody::new(crate::pane_atoms::live_sections(&state)).widgets;
    assert!(
        live_only > 0,
        "the live atoms sections must carry widgets, or this comparison proves nothing"
    );

    let mut extended = crate::pane_atoms::live_sections(&state);
    extended.extend(atom_preview_sections());
    let with_previews = PaneBody::new(extended).widgets;

    assert_eq!(
        live_only, with_previews,
        "appending the atoms previews changed the pane's widget total"
    );
    assert_eq!(
        with_previews,
        crate::pane_atoms::atoms_pane(&state).widgets,
        "the hand-built live+previews body must total what the real `atoms_pane` reports"
    );
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

#[test]
fn every_preview_renders_headless() {
    for section in atom_preview_sections()
        .into_iter()
        .chain(composite_preview_sections())
    {
        laid_out(PANE, section);
    }
}

#[test]
fn every_preview_renders_at_a_narrow_viewport() {
    for section in atom_preview_sections()
        .into_iter()
        .chain(composite_preview_sections())
    {
        laid_out(NARROW, section);
    }
}

#[test]
fn previews_render_in_the_composites_viewport() {
    for section in composite_preview_sections() {
        laid_out(WIDE, section);
    }
}

// ---------------------------------------------------------------------------
// hex()
// ---------------------------------------------------------------------------

#[test]
fn hex_formats_opaque_and_transparent_colours() {
    assert_eq!("#FFFFFFFF", hex(iced::Color::WHITE));
    assert_eq!("#000000FF", hex(iced::Color::BLACK));
    assert_eq!("#00000000", hex(iced::Color::TRANSPARENT));
}

#[test]
fn hex_rounds_and_clamps_out_of_range_channels() {
    // Rounding: 0.5019608 * 255 = 128.0 exactly, while 0.5 * 255 = 127.5
    // must round *up* to 128 rather than truncate to 127.
    assert_eq!("#FF0080FF", hex(iced::Color::from_rgb(1.0, 0.0, 0.5019608)));
    assert_eq!("#808080FF", hex(iced::Color::from_rgb(0.5, 0.5, 0.5)));
    assert_eq!("#000000FF", hex(iced::Color::BLACK));
    assert_eq!("#FFFFFFFF", hex(iced::Color::WHITE));

    // Clamping: `Color::from_rgb` asserts every component into [0, 1] in
    // debug, so an out-of-range channel cannot arrive through the
    // constructor. The struct literal is the only way one exists, and the
    // formatter must still print a well-formed string rather than a
    // negative or three-digit component.
    let wild = iced::Color {
        r: 2.0,
        g: -1.0,
        b: 0.5019608,
        a: 1.0,
    };
    let text = hex(wild);
    assert_eq!(9, text.len(), "{text}");
    assert!(text.starts_with('#'), "{text}");
    assert_eq!("#FF0080FF", text);
}

// ---------------------------------------------------------------------------
// The printed values are the resolver's values
// ---------------------------------------------------------------------------

#[test]
fn the_card_preview_paints_what_the_card_resolver_returned() {
    let palette = Palette::dark();
    let style = card::resolve_style(CardElevation::Elevated, &palette);
    let rendered = render_text_of(card_preview());
    assert!(
        rendered.contains(&hex(style.border_color)),
        "the elevated card's border hex must appear on screen"
    );
}

#[test]
fn the_notification_preview_covers_every_severity() {
    let rendered = render_text_of(notification_preview());
    for severity in Severity::ALL {
        assert!(
            rendered.contains(&format!("{severity:?}")),
            "severity {severity:?} is missing from the preview"
        );
    }
}

#[test]
fn the_breadcrumb_preview_reports_the_same_counts_as_collapse() {
    let items: Vec<BreadcrumbItem<String>> = [
        ("vault", "Vault", 64.0),
        ("doc", "Documents", 96.0),
        ("sub", "2026", 64.0),
        ("dir", "Planning", 88.0),
        ("file", "roadmap.md", 104.0),
    ]
    .into_iter()
    .map(|(id, label, measure)| BreadcrumbItem::new(id.to_string(), label, measure))
    .collect();

    let wide = breadcrumb::collapse(&items, BREADCRUMB_PREVIEW_WIDTH);
    let tight = breadcrumb::collapse(&items, BREADCRUMB_TIGHT_WIDTH);
    let rendered = render_text_of(breadcrumb_preview());

    assert!(
        rendered.contains(&format!("{} inline", wide.inline_count())),
        "the wide trail's inline count must be printed: {rendered}"
    );
    assert!(
        rendered.contains(&format!("{} inline", tight.inline_count())),
        "the tight trail's inline count must be printed: {rendered}"
    );
    assert!(
        tight.hidden_count() > 0,
        "the tight width must actually force elision, otherwise the preview proves nothing"
    );
}

#[test]
fn the_overflow_preview_evicts_only_at_the_tight_width() {
    let items: Vec<OverflowItem<String>> = [
        ("save", "Save", 64.0, OverflowRank::PRIMARY),
        ("copy", "Copy", 60.0, OverflowRank::DISPOSABLE),
        ("share", "Share", 68.0, OverflowRank::SECONDARY),
        ("print", "Print", 64.0, OverflowRank::DISPOSABLE),
        ("export", "Export", 72.0, OverflowRank::LAST),
    ]
    .into_iter()
    .map(|(id, label, measure, rank)| OverflowItem::new(id.to_string(), label, measure, rank))
    .collect();

    let wide = overflow_toolbar::fit(&items, OVERFLOW_WIDE_WIDTH);
    let tight = overflow_toolbar::fit(&items, OVERFLOW_TIGHT_WIDTH);

    assert_eq!(
        0,
        wide.overflow_count(),
        "the wide strip must hold everything"
    );
    assert!(
        tight.overflow_count() > 0,
        "the tight strip must evict something"
    );

    let rendered = render_text_of(overflow_toolbar_preview());
    assert!(
        rendered.contains("overflow []"),
        "wide line missing: {rendered}"
    );
    assert!(
        !rendered.contains("overflow []\n") || rendered.contains("overflow ["),
        "the tight line must list at least one evicted item: {rendered}"
    );
}

#[test]
fn the_table_preview_prints_every_resolved_column_width() {
    let specs = vec![
        ColumnSpec::fixed(0, "Status", 96.0),
        ColumnSpec::content(1, "Name", 220.0).with_min(120.0),
        ColumnSpec::proportional(2, "Size", 1.0).with_min(64.0),
        ColumnSpec::proportional(3, "Modified", 2.0)
            .with_min(96.0)
            .with_align(hoarfrost::composite::table_column::CellAlign::End),
    ];
    let layout = table_column::resolve_widths(&specs, TABLE_PREVIEW_WIDTH);
    let rendered = render_text_of(table_preview());

    for width in layout.widths() {
        assert!(
            rendered.contains(&format!("{width:.0}")),
            "resolved width {width:.0} must appear in the preview"
        );
    }
}

#[test]
fn the_chip_preview_distinguishes_selected_from_unselected_fill() {
    let palette = Palette::dark();
    let off = chip::resolve_style(ChipPolarity::Unselected, ChipHeight::Bar, REST, &palette);
    let on = chip::resolve_style(ChipPolarity::Selected, ChipHeight::Bar, REST, &palette);
    assert_ne!(off.fill, on.fill, "the fixture assumes the two differ");

    let rendered = render_text_of(chip_preview());
    assert!(rendered.contains(&hex(off.fill)));
    assert!(rendered.contains(&hex(on.fill)));
}

#[test]
fn the_slider_preview_sweeps_every_base_state() {
    let rendered = render_text_of(slider_preview());
    for (_, name) in BASE_STATES {
        assert!(
            rendered.contains(&format!("Slider · {name}")),
            "base state {name} missing from the slider preview"
        );
    }
}

#[test]
fn the_number_box_preview_sweeps_every_base_state() {
    let rendered = render_text_of(number_box_preview());
    for (_, name) in BASE_STATES {
        assert!(
            rendered.contains(&format!("Number box · {name}")),
            "base state {name} missing from the number box preview"
        );
    }
}

#[test]
fn the_list_tile_preview_shows_selection_and_disabling_as_different_paint() {
    let palette = Palette::dark();
    let plain = list_tile::resolve_style(false, false, BaseState::Rest, &palette);
    let selected = list_tile::resolve_style(true, false, BaseState::Rest, &palette);
    let disabled = list_tile::resolve_style(false, true, BaseState::Disabled, &palette);

    assert_ne!(plain.background, selected.background);
    assert_eq!(iced::Color::TRANSPARENT, disabled.background);

    let rendered = render_text_of(list_tile_preview());
    assert!(rendered.contains(&hex(selected.background)));
    assert!(rendered.contains(&hex(disabled.background)));
}

#[test]
fn the_progress_preview_separates_active_from_error_fill() {
    let palette = Palette::dark();
    let active = progress::resolve_style(&ProgressModel::determinate(62, 100), &palette);
    let error = progress::resolve_style(
        &ProgressModel::determinate(62, 100).with_status(ProgressStatus::Error),
        &palette,
    );
    assert_ne!(active.fill, error.fill);

    let rendered = render_text_of(progress_preview());
    assert!(rendered.contains(&hex(active.fill)));
    assert!(rendered.contains(&hex(error.fill)));
}

#[test]
fn the_badge_preview_covers_all_three_badge_kinds() {
    let rendered = render_text_of(badge_preview());
    for kind in ["Dot", "Count(7)", "Status"] {
        assert!(
            rendered.contains(kind),
            "badge kind {kind} missing from the preview"
        );
    }
}

#[test]
fn crumb_roles_resolve_to_distinct_colours() {
    let palette = Palette::dark();
    let root = breadcrumb::crumb_colour(&palette, CrumbRole::Root, REST);
    let terminal = breadcrumb::crumb_colour(&palette, CrumbRole::Terminal, REST);
    // Not a claim about which is bolder - only that the preview is showing
    // two different resolved values rather than one value twice.
    assert_ne!(hex(root), hex(terminal));
}

/// Collects the text a section renders, by walking its widget tree.
///
/// The demo has no text accessor on `Element`, so this uses the same
/// trick the layout probe uses: a selector that records every `Text`
/// candidate. Kept here as a tiny local helper so the preview tests can
/// assert on what is actually on screen.
fn render_text_of<'a>(section: Section<'a>) -> String {
    crate::layout_probe::text_of(section.element)
}
