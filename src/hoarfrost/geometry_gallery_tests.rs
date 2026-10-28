//! Tests for the standalone geometry gallery.
//!
//! A failing assertion here is a reportable framework finding, not something
//! to tune the threshold around.
//!
//! Standalone geometry defects are **ratcheted** rather than fixed or
//! ignored: see [`KNOWN_FRAMEWORK_DEFECTS`]. The ratchet keeps the suite
//! green while a defect is being carried, and makes it impossible to add a
//! new standalone geometry defect unnoticed — it fails loudly if a pinned
//! defect is *fixed*, so the pin gets removed. This is the
//! `--write-baseline` / `--fail-on-new-drift` pattern; a permanently-red
//! test is as useless as a missing one, because people stop reading it.

use super::{
    Candidate, GalleryEl, Measured, NOT_RENDERABLE, PIN, TOLERANCE, candidate_report, format_table,
    framework_geometry_conformance, gallery_renderer, measure_all, measure_raw,
};
use crate::composite::settings_row::{
    self, DESCRIPTION_RUNG, TITLE_RUNG, horizontal, resolve_style, row_height,
};
use crate::test_helpers::Blank;
use crate::token::color::Palette;
use crate::token::size;
use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::Tree;
use iced::{Element, Rectangle, Size};

/// Framework geometry defects found by this gallery and pinned as known-bad.
///
/// Each entry is the exact `Measured::widget` label whose standalone layout
/// does not match what its own module declares. Adding to this list is a
/// decision about the framework, not about the test — say why in the PR.
///
/// The list is empty. It used to carry three `settings_row::horizontal`
/// entries, excusing a text column built with `Row::new()` (description
/// beside the title) and a builder that applied none of `PADDING_VERTICAL`,
/// `PADDING_HORIZONTAL` or `MIN_HEIGHT`. Both are fixed: the text column is
/// a `Column`, and the builder reads the same constants `bands()` does, so
/// the probes land on `row_height`. The ratchet stays — the next defect is
/// not known yet.
const KNOWN_FRAMEWORK_DEFECTS: [&str; 0] = [];

/// Slack allowed when comparing two laid-out edges against each other.
///
/// Tighter than [`TOLERANCE`]: these are edges the builder places by
/// construction, not a rendered glyph run, so anything above a hair means
/// the widget genuinely put them somewhere else.
const EDGE_EPSILON: f32 = 0.01;

/// Print the whole gallery table so a human can read the numbers directly.
///
/// Also a guard: the table must be complete, and every row must carry a
/// finite measurement.
#[test]
fn gallery_table_is_complete() {
    let rows = measure_all();

    print!("{}", format_table(&rows));

    assert!(
        rows.len() >= 30,
        "the gallery shrank to {} rows; it is meant to cover every framework widget",
        rows.len()
    );
    assert!(
        rows.iter()
            .all(|row| row.measured_height.is_finite() && row.measured_width.is_finite()),
        "a probe produced a non-finite measurement"
    );
}

/// Every widget must lay out, alone, at the height its own module declares —
/// except for the pinned entries in [`KNOWN_FRAMEWORK_DEFECTS`].
///
/// Two claims are made together, so the pin cannot rot in either direction:
/// no defect outside the pinned list may appear, and every pinned defect must
/// still be present. A fixed defect fails this test until its pin is removed.
#[test]
fn every_widget_lays_out_at_its_declared_height() {
    let rows = measure_all();

    let failing: Vec<&str> = rows
        .iter()
        .filter(|row| !row.matches())
        .map(|row| row.widget.as_str())
        .collect();

    let unexpected: Vec<&&str> = failing
        .iter()
        .filter(|widget| !KNOWN_FRAMEWORK_DEFECTS.contains(widget))
        .collect();
    assert!(
        unexpected.is_empty(),
        "new standalone geometry defect(s) not in KNOWN_FRAMEWORK_DEFECTS: {:?}\n\
         Details:\n{}",
        unexpected,
        rows.iter()
            .filter(|row| !row.matches())
            .map(|row| format!(
                "  {} declared {:.1}px, measured {:.1}px (delta {:+.2}px, tolerance {:.1}px)",
                row.widget,
                row.declared_height,
                row.measured_height,
                row.delta(),
                TOLERANCE
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let healed: Vec<&&str> = KNOWN_FRAMEWORK_DEFECTS
        .iter()
        .filter(|widget| !failing.contains(widget))
        .collect();
    assert!(
        healed.is_empty(),
        "pinned defect(s) now pass and must be removed from KNOWN_FRAMEWORK_DEFECTS: {healed:?}"
    );
}

/// The conformance report must contain nothing but the pinned defects.
///
/// Same ratchet as the height test, applied to the report surface so a new
/// off-ladder or mismatched probe cannot slip in unnoticed.
#[test]
fn framework_geometry_conformance_report_is_empty() {
    let problems = framework_geometry_conformance();

    let unexpected: Vec<&String> = problems
        .iter()
        .filter(|problem| {
            !KNOWN_FRAMEWORK_DEFECTS
                .iter()
                .any(|pinned| problem.starts_with(pinned))
        })
        .collect();

    assert!(
        unexpected.is_empty(),
        "framework geometry gained {} non-pinned conformance problem(s):\n{}",
        unexpected.len(),
        unexpected
            .iter()
            .map(|problem| format!("  {problem}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    assert_eq!(
        problems.len(),
        KNOWN_FRAMEWORK_DEFECTS.len(),
        "the conformance report no longer matches the pinned baseline of {} defect(s); \
         actual {} — every standalone geometry claim the framework makes must hold, \
         or the gap must be pinned with a reason.\n{}",
        KNOWN_FRAMEWORK_DEFECTS.len(),
        problems.len(),
        problems
            .iter()
            .map(|problem| format!("  {problem}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Measuring twice must give the same answer.
///
/// A gallery that drifts between runs cannot be used to attribute a defect,
/// so determinism is a precondition for every other claim in this file.
#[test]
fn measure_all_is_deterministic() {
    let first = measure_all();
    let second = measure_all();

    assert_eq!(
        first.len(),
        second.len(),
        "the gallery changed size between runs: {} then {}",
        first.len(),
        second.len()
    );

    let drift: Vec<String> = first
        .iter()
        .zip(&second)
        .filter(|(a, b)| a != b)
        .map(|(a, b)| format!("  {} : {:?} != {:?}", a.widget, a, b))
        .collect();

    assert!(
        drift.is_empty(),
        "non-deterministic measurements between two runs of measure_all():\n{}",
        drift.join("\n")
    );
}

/// The gallery must still cover everything the harness was built to cover.
///
/// Guards against a probe silently dropping out of the table and turning a
/// real gap into a passing run.
#[test]
fn gallery_covers_every_required_widget() {
    let rows = measure_all();
    let names: Vec<&str> = rows.iter().map(|row| row.widget.as_str()).collect();

    let required = [
        "Button::primary (standard)",
        "Button::primary (compact)",
        "Button::secondary (standard)",
        "Button::secondary (compact)",
        "Button::ghost (standard)",
        "Button::ghost (compact)",
        "Button::danger (standard)",
        "Button::danger (compact)",
        "TextInput::text (standard)",
        "TextInput::search (standard)",
        "TextInput::dropdown (standard)",
        "TextInput::password (standard)",
        "TextInput::textarea (standard)",
        "TextInput::text (compact)",
        "Checkbox (standalone)",
        "ToggleSwitch (standalone)",
        "RadioGroup (one option, vertical)",
        "RangeSlider",
        "Rating (5)",
        "Hyperlink (standalone)",
        "Segmented (standard)",
        "Segmented (compact)",
        "form_field::stacked (header+control+caption)",
        "settings_row::horizontal (title only, 32px control)",
        "NavBar (standard)",
        "NavBar (compact)",
    ];

    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|name| !names.contains(name))
        .collect();

    assert!(
        missing.is_empty(),
        "the gallery no longer measures: {}",
        missing.join(", ")
    );
}

/// Every probe must produce at least one measurable row.
///
/// A probe that laid out to zero in both axes is not a measurement, and a
/// probe that vanished from the table would silently shrink the gallery.
#[test]
fn no_probe_collapses_to_nothing() {
    let rows = measure_all();
    let collapsed: Vec<&Measured> = rows
        .iter()
        .filter(|row| row.measured_height <= 0.0 || row.measured_width <= 0.0)
        .collect();

    assert!(
        collapsed.is_empty(),
        "probes that laid out to nothing: {:?}",
        collapsed
            .iter()
            .map(|row| row.widget.as_str())
            .collect::<Vec<_>>()
    );
}

/// The un-renderable list must stay populated and honest.
///
/// If one of these gains an element builder the list must shrink, not be
/// left claiming a gap that no longer exists.
#[test]
fn unrenderable_list_is_populated() {
    let names: Vec<&str> = NOT_RENDERABLE.iter().map(|(name, _)| *name).collect();

    assert!(
        names.contains(&"Slider (single-thumb)"),
        "the single-thumb slider gap must stay reported: {names:?}"
    );
    assert!(
        names.contains(&"list_tile"),
        "the list_tile gap must stay reported: {names:?}"
    );
    for (name, reason) in NOT_RENDERABLE {
        assert!(
            !reason.trim().is_empty(),
            "{name} is reported un-renderable with no reason"
        );
    }
}

/// The candidate diagnostic must run over every probe without blowing up.
///
/// No assertion about *where* a widget registers its candidate — that is
/// the number a human reads. This only pins that the probe reaches every
/// widget and returns a rectangle per registration.
#[test]
fn candidate_report_reaches_every_probe() {
    let report = candidate_report();
    let measured = measure_all();

    assert_eq!(
        report.len(),
        measured.len(),
        "the candidate probe visited a different number of widgets than the layout probe"
    );

    for (label, candidates) in &report {
        println!("{label}");
        if candidates.is_empty() {
            println!("    (no candidate registered at all)");
        }
        for candidate in candidates {
            let _: &Candidate = candidate;
            println!(
                "    {:<10} x {:>6.1} y {:>6.1}  {:.1} x {:.1}",
                candidate.kind,
                candidate.bounds.x,
                candidate.bounds.y,
                candidate.bounds.width,
                candidate.bounds.height
            );
            assert!(
                candidate.bounds.width.is_finite() && candidate.bounds.height.is_finite(),
                "{label} registered a non-finite candidate: {:?}",
                candidate.bounds
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The settings row's own claims, read off the laid-out widget
// ---------------------------------------------------------------------------

/// A settings row with a control of the caller's own size.
fn settings_row_element(has_description: bool, control: Size) -> GalleryEl {
    let style = resolve_style(false, &Palette::dark());
    let description = if has_description {
        Some("Pull the workspace before the first frame")
    } else {
        None
    };

    horizontal(
        "Sync on startup",
        description,
        Element::new(Blank::new(control)),
        &style,
    )
}

/// Lay a settings row out alone inside [`PIN`] and return the absolute
/// bounds of its three parts: the row, its text column, and its control
/// column.
///
/// The tree the builder produces is `Row(padding) -> [Column(text),
/// Container(control)]`, so the parts are reachable by position and no
/// guess is needed about which rectangle belongs to which line.
fn laid_out_settings_row(has_description: bool, control: Size) -> [Rectangle; 3] {
    let renderer = gallery_renderer();
    let mut element = settings_row_element(has_description, control);

    let mut tree = Tree::empty();
    tree.diff(element.as_widget());

    let limits = layout::Limits::new(Size::ZERO, PIN);
    let node = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);

    let root = Layout::new(&node);
    let mut children = root.children();
    let text_column = children.next().expect("the text column").bounds();
    let control_column = children.next().expect("the control column").bounds();

    [root.bounds(), text_column, control_column]
}

/// The two text lines a title-and-description row lays out, in order.
fn laid_out_text_lines() -> [Rectangle; 2] {
    let renderer = gallery_renderer();
    let mut element = settings_row_element(true, Size::new(size::CONTROL_MD, size::CONTROL_MD));

    let mut tree = Tree::empty();
    tree.diff(element.as_widget());

    let limits = layout::Limits::new(Size::ZERO, PIN);
    let node = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);

    let text_column = Layout::new(&node)
        .children()
        .next()
        .expect("the text column");

    let mut lines = text_column.children();
    let title = lines.next().expect("the title line").bounds();
    let description = lines.next().expect("the description line").bounds();

    [title, description]
}

/// The description renders **below** the title, not beside it.
///
/// This is the defect the row carried: its text column was a `Row`, so the
/// two landed on one baseline. A side-by-side pair shares a `y` and differs
/// in `x`; a stacked pair shares an `x` and differs in `y`, with the
/// description starting exactly where the title's line box ends.
#[test]
fn the_description_stacks_below_the_title_rather_than_beside_it() {
    let [title, description] = laid_out_text_lines();

    println!(
        "title        x {:.2} y {:.2} {:.1} x {:.1}",
        title.x, title.y, title.width, title.height
    );
    println!(
        "description  x {:.2} y {:.2} {:.1} x {:.1}",
        description.x, description.y, description.width, description.height
    );

    assert!(
        description.y > title.y,
        "description top y {:.2} is not below the title top y {:.2}: the two are on \
         one line, not stacked",
        description.y,
        title.y,
    );
    assert!(
        (description.y - (title.y + title.height)).abs() <= EDGE_EPSILON,
        "description top y {:.2} must abut the title's bottom edge {:.2}; a gap or an \
         overlap means the lines are not stacked flush",
        description.y,
        title.y + title.height,
    );
    assert!(
        (description.x - title.x).abs() <= EDGE_EPSILON,
        "description x {:.2} and title x {:.2} must share a left edge; a horizontal \
         offset is what side-by-side placement produces",
        description.x,
        title.x,
    );
    assert!(
        description.y >= title.y + title.height - EDGE_EPSILON,
        "the title's line box [{:.2}, {:.2}] and the description's [{:.2}, {:.2}] \
         overlap vertically, which is the side-by-side signature",
        title.y,
        title.y + title.height,
        description.y,
        description.y + description.height,
    );
    assert_eq!(
        title.height, TITLE_RUNG.line_height,
        "the title lays out on its own rung"
    );
    assert_eq!(
        description.height, DESCRIPTION_RUNG.line_height,
        "the description lays out on its own rung"
    );
}

/// The stacked pair measures what [`settings_row::text_column_height`] says.
///
/// A row that laid the two side by side would return the title's single
/// line here, which is exactly the number the pinned defect used to produce.
#[test]
fn the_stacked_text_column_is_the_sum_of_its_two_lines() {
    let [title, description] = laid_out_text_lines();
    let stacked = description.y + description.height - title.y;

    assert!(
        (stacked - settings_row::text_column_height(true)).abs() <= EDGE_EPSILON,
        "the two lines span {:.2}px, text_column_height(true) declares {:.1}px",
        stacked,
        settings_row::text_column_height(true),
    );
}

/// The built row's height is [`row_height`] at every control height, not
/// only the two the probe table samples.
///
/// The floor, the text column and the hosted control are the three terms
/// `row_height` balances, and the widget has to balance them for all of
/// them — a control that ignores the floor or double-counts the padding
/// passes the two sampled probes and still ships wrong.
#[test]
fn the_built_row_height_is_the_declared_row_height_at_every_control_height() {
    let renderer = gallery_renderer();

    for has_description in [false, true] {
        for control_height in [0.0, 1.0, 24.0, size::CONTROL_SM, size::CONTROL_MD, 48.0] {
            let control = Size::new(size::CONTROL_MD, control_height);
            let built = measure_raw(&renderer, &move || {
                settings_row_element(has_description, control)
            })
            .0;
            let declared = row_height(has_description, control_height);

            assert!(
                (built - declared).abs() <= TOLERANCE,
                "has_description {has_description}, control {control_height}px: built \
                 {built:.1}px but row_height declares {declared:.1}px",
            );
        }
    }
}

/// The columns the widget lays out are the columns [`settings_row::bands`]
/// reserves for the same control.
///
/// The pure geometry and the built widget used to be written independently,
/// which is how the row came to render at `max(title, control)` with no
/// padding while `bands()` reported a padded 60px card. The horizontal
/// split is compared here; the control's own `y` is not, because the
/// control is centred inside the band `bands` reserves rather than filling
/// it.
#[test]
fn the_built_columns_match_the_bands_bands_reserves() {
    let control_width = 96.0;
    let [row, text, control] =
        laid_out_settings_row(true, Size::new(control_width, size::CONTROL_MD));
    let expected = settings_row::bands(row, control_width);

    assert!(
        (text.x - expected.text.x).abs() <= EDGE_EPSILON,
        "text column x {:.2} vs bands {:.2}",
        text.x,
        expected.text.x,
    );
    assert!(
        (text.width - expected.text.width).abs() <= EDGE_EPSILON,
        "text column width {:.2} vs bands {:.2}",
        text.width,
        expected.text.width,
    );
    assert!(
        (control.x - expected.control.x).abs() <= EDGE_EPSILON,
        "control column x {:.2} vs bands {:.2}",
        control.x,
        expected.control.x,
    );
    assert!(
        (control.width - expected.control.width).abs() <= EDGE_EPSILON,
        "control column width {:.2} vs bands {:.2}",
        control.width,
        expected.control.width,
    );
    assert!(
        (text.y - expected.text.y).abs() <= EDGE_EPSILON,
        "text column y {:.2} vs bands {:.2}: the row's vertical padding is not being \
         applied where bands() says it is",
        text.y,
        expected.text.y,
    );
}

/// The declared padding is actually on the built widget.
///
/// Read straight off the row: the distance from the card's left edge to
/// the text column is `PADDING_HORIZONTAL`, and the distance from the
/// card's top to the first line is `PADDING_VERTICAL`.
#[test]
fn the_declared_padding_is_on_the_built_widget() {
    let [row, text, _control] =
        laid_out_settings_row(true, Size::new(size::CONTROL_MD, size::CONTROL_MD));

    assert!(
        (text.x - row.x - settings_row::PADDING_HORIZONTAL).abs() <= EDGE_EPSILON,
        "left inset {:.2} vs PADDING_HORIZONTAL {:.1}",
        text.x - row.x,
        settings_row::PADDING_HORIZONTAL,
    );
    assert!(
        (text.y - row.y - settings_row::PADDING_VERTICAL).abs() <= EDGE_EPSILON,
        "top inset {:.2} vs PADDING_VERTICAL {:.1}",
        text.y - row.y,
        settings_row::PADDING_VERTICAL,
    );
    assert_eq!(
        settings_row::row_padding().y(),
        settings_row::PADDING_VERTICAL + settings_row::PADDING_VERTICAL
    );
    assert_eq!(
        settings_row::row_padding().x(),
        settings_row::PADDING_HORIZONTAL + settings_row::PADDING_HORIZONTAL
    );
}
