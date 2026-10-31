//! Render-level ("ink") coverage for the atoms pane.
//!
//! Every test here asks the **framebuffer** a question. None of them
//! reads a model field to decide whether the UI worked: a section that
//! reports the right widget count, lays out at the right bounds and
//! still paints nothing is the failure class these tests exist for, and
//! it is the class the checkbox mark, the navbar card and
//! `settings_row::horizontal` all belonged to.
//!
//! Three readings are used, in increasing strength:
//!
//! 1. **Ink presence** — the section's body region contains readable
//!    ink and is not a flat rectangle.
//! 2. **Control visibility** — a widget paints over the card behind it
//!    at the rect it recorded.
//! 3. **Repaint** — a state change actually moves pixels. This is the
//!    one a widget that never drew cannot satisfy by accident.
//!
//! Sections whose widgets register no probe-addressable id (combo box,
//! search box, avatar, date/time/colour picker, tooltip) are covered at
//! the body-region and repaint level only; there is no rect inside them a
//! test can name. That gap is reported rather than papered over.

use std::collections::BTreeSet;

use iced::{Point, Rectangle, Size};

use hoarfrost::test_helpers::{MIN_CHANGED_PIXELS, SURFACE_TOLERANCE, assert_repainted};
use hoarfrost::widget::range_slider_model::RangeSliderModel;
use hoarfrost::widget::text_input_model::TextInputModel;
use hoarfrost::widget::toggle::ToggleControl;
use hoarfrost::widget::toggle::{self};
use hoarfrost::widget::toggle_model::CheckState;

use crate::data;
use crate::layout_probe::{PANE_VIEWPORT, Probe, capture_session};
use crate::pane_atoms::{self, AtomsState};
use crate::render_section::{self, Captured, assert_body_painted, capture_diff};
use crate::section::Section;

/// A section builder that can be addressed by name in a sweep.
type AtomBuilder = for<'a> fn(&'a AtomsState) -> Section<'a>;

/// Every live atoms section, in display order.
fn atom_builders() -> Vec<(&'static str, AtomBuilder)> {
    vec![
        ("Buttons", pane_atoms::buttons_section as AtomBuilder),
        ("Checkbox", pane_atoms::checkbox_section as AtomBuilder),
        ("Radio group", pane_atoms::radio_section as AtomBuilder),
        ("Toggle switch", pane_atoms::switch_section as AtomBuilder),
        ("Segmented", pane_atoms::segmented_section as AtomBuilder),
        (
            "Split button",
            pane_atoms::split_button_section as AtomBuilder,
        ),
        ("Hyperlink", |_| pane_atoms::hyperlink_section()),
        ("Text input", pane_atoms::text_input_section as AtomBuilder),
        ("Password", pane_atoms::password_section as AtomBuilder),
        ("Combo box", pane_atoms::combo_box_section as AtomBuilder),
        ("Search box", pane_atoms::search_box_section as AtomBuilder),
        ("Rating", pane_atoms::rating_section as AtomBuilder),
        (
            "Range slider",
            pane_atoms::range_slider_section as AtomBuilder,
        ),
        ("Avatar", |_| pane_atoms::avatar_section()),
        (
            "Date picker",
            pane_atoms::date_picker_section as AtomBuilder,
        ),
        (
            "Time picker",
            pane_atoms::time_picker_section as AtomBuilder,
        ),
        (
            "Colour picker",
            pane_atoms::color_picker_section as AtomBuilder,
        ),
        ("Tooltip", |_| pane_atoms::tooltip_section()),
    ]
}

/// Grows a rectangle outward on every side.
fn grown(rect: Rectangle, by: f32) -> Rectangle {
    Rectangle {
        x: rect.x - by,
        y: rect.y - by,
        width: rect.width + by * 2.0,
        height: rect.height + by * 2.0,
    }
}

/// Whether `inner` lies inside `outer`, within `slop` on each edge.
fn contained_within(inner: Rectangle, outer: Rectangle, slop: f32) -> bool {
    inner.x >= outer.x - slop
        && inner.y >= outer.y - slop
        && inner.x + inner.width <= outer.x + outer.width + slop
        && inner.y + inner.height <= outer.y + outer.height + slop
}

/// The checkbox indicator inside a checkbox row's recorded bounds.
///
/// Mirrors the widget's own draw: the box is left-aligned in the row and
/// vertically centred within it.
fn checkbox_indicator(row: Rectangle) -> Rectangle {
    let (width, height) = toggle::indicator_size(ToggleControl::Checkbox);
    Rectangle::new(
        Point::new(row.x, row.y + (row.height - height) / 2.0),
        Size::new(width, height),
    )
}

/// A range-slider model with the fixture's bounds but a moved thumb pair.
fn range_with(lower: f64, upper: f64) -> RangeSliderModel {
    let (min, max, _, _) = data::RANGE_BOUNDS;
    RangeSliderModel::new(min, max, lower, upper)
        .with_step(Some(data::RANGE_STEP))
        .with_unit_suffix(data::RANGE_UNIT)
}

// ---------------------------------------------------------------------------
// Breadth — every section paints inside its own card
// ---------------------------------------------------------------------------

#[test]
fn every_atoms_section_paints_inside_its_own_body() {
    let state = AtomsState::new();
    let mut capture = capture_session();

    for (name, build) in atom_builders() {
        let captured = Captured::capture(&mut capture, PANE_VIEWPORT, || build(&state));
        assert_body_painted(&captured);
        assert_eq!(
            captured.id,
            crate::section::section_id(name),
            "section {name} mounted under an unexpected id"
        );
    }
}

#[test]
fn no_atoms_section_is_a_flat_rectangle() {
    let state = AtomsState::new();
    let mut capture = capture_session();

    for (name, build) in atom_builders() {
        let captured = Captured::capture(&mut capture, PANE_VIEWPORT, || build(&state));
        assert!(
            !captured.frame.is_uniform(captured.body, SURFACE_TOLERANCE),
            "section {name}'s body {:?} is one flat colour — nothing was drawn \
             on top of the card",
            captured.body
        );
    }
}

// ---------------------------------------------------------------------------
// Buttons (UIF-ATOM-001)
// ---------------------------------------------------------------------------

#[test]
fn every_button_paints_over_the_card_at_its_own_rect() {
    let state = AtomsState::new();
    let mut capture = capture_session();
    let captured = Captured::capture(&mut capture, PANE_VIEWPORT, || {
        pane_atoms::buttons_section(&state)
    });

    for variant in ["Primary", "Secondary", "Ghost", "Danger"] {
        for kind in ["live", "busy"] {
            let id = format!("demo.atoms.button.{variant}.{kind}");
            let paint = captured.control_paint(&id);
            assert!(
                paint >= render_section::MIN_CONTROL_PAINT,
                "the {variant} {kind} button lays out at {:?} but paints {paint} \
                 pixels over its card — it is invisible",
                captured.control(&id)
            );
        }
    }
}

#[test]
fn every_button_row_label_paints_inside_its_own_box() {
    let state = AtomsState::new();
    let mut capture = capture_session();
    let captured = Captured::capture(&mut capture, PANE_VIEWPORT, || {
        pane_atoms::buttons_section(&state)
    });

    for variant in ["primary", "secondary", "ghost", "danger"] {
        let id = crate::section::label_id(pane_atoms::BUTTON_ROW_LABEL_PREFIX, variant);
        let bounds = captured.control(&id);
        let painted = captured
            .frame
            .ink_bounds(bounds)
            .unwrap_or_else(|| panic!("the {variant} row label at {bounds:?} painted no ink"));
        assert!(
            painted.width > 8.0,
            "the {variant} row label painted only a {painted:?} sliver — the \
             word is missing or clipped away"
        );
        assert!(
            contained_within(painted, bounds, 1.0),
            "the {variant} row label's ink {painted:?} escapes its own box {bounds:?}"
        );
    }
}

#[test]
fn the_disabled_button_is_still_visible_though_it_registers_nothing() {
    let state = AtomsState::new();
    let mut capture = capture_session();
    let captured = Captured::capture(&mut capture, PANE_VIEWPORT, || {
        pane_atoms::buttons_section(&state)
    });

    // A disabled hoarfrost `Button` registers no focusable candidate, so
    // the probe cannot name it — but the user must still see it. Its slot
    // is derivable: it sits one `space::SM` after the live button, and
    // the busy button follows it the same way.
    let gap = hoarfrost::token::space::SM;
    for variant in ["Primary", "Secondary", "Ghost", "Danger"] {
        let live = captured.control(&format!("demo.atoms.button.{variant}.live"));
        let busy = captured.control(&format!("demo.atoms.button.{variant}.busy"));
        let slot = Rectangle::new(
            Point::new(live.x + live.width + gap, live.y),
            Size::new(live.width, live.height),
        );
        assert!(
            (busy.x - (slot.x + slot.width) - gap).abs() < 1.0,
            "{variant}: the disabled slot does not fall between live and busy \
             (live {live:?}, busy {busy:?})"
        );

        assert!(
            captured
                .frame
                .painted_count(slot, render_section::card_fill_rgb(), 2)
                >= MIN_CHANGED_PIXELS,
            "the {variant} disabled button at {slot:?} paints nothing over the \
             card — a disabled control that is invisible reads as a hole in the \
             form"
        );
    }
}

// ---------------------------------------------------------------------------
// Checkbox (UIF-ATOM-003) — the mark must land inside the box that owns it
// ---------------------------------------------------------------------------

#[test]
fn the_checked_mark_paints_inside_its_own_box() {
    let state = AtomsState::new();
    let mut capture = capture_session();
    let captured = Captured::capture(&mut capture, PANE_VIEWPORT, || {
        pane_atoms::checkbox_section(&state)
    });

    // Sample wider than the box so a displaced mark is caught, but stop
    // short of the row's own label text on the right.
    let slop = 2.0;
    for index in 1..=2 {
        let row = captured.control(&format!("demo.atoms.checkbox.{index}"));
        let box_ = checkbox_indicator(row);
        let sample = Rectangle {
            x: box_.x - 12.0,
            y: box_.y - 12.0,
            width: box_.width + 14.0,
            height: box_.height + 24.0,
        };

        let painted = captured
            .frame
            .ink_bounds(sample)
            .unwrap_or_else(|| panic!("checkbox {index} painted no ink near its box {box_:?}"));

        assert!(
            contained_within(painted, grown(box_, slop), 0.5),
            "checkbox {index}'s ink {painted:?} escapes its own indicator box \
             {box_:?} by more than {slop}px — the mark is painted outside the \
             control that owns it"
        );
    }
}

#[test]
fn a_marked_box_paints_more_ink_than_an_empty_one() {
    let state = AtomsState::new();
    let mut capture = capture_session();
    let captured = Captured::capture(&mut capture, PANE_VIEWPORT, || {
        pane_atoms::checkbox_section(&state)
    });

    let ink_in_box = |index: usize| {
        let row = captured.control(&format!("demo.atoms.checkbox.{index}"));
        captured.frame.ink_count(checkbox_indicator(row))
    };

    let empty = ink_in_box(0);
    let ticked = ink_in_box(1);
    let dashed = ink_in_box(2);

    assert!(
        ticked > empty,
        "the checked box paints {ticked} ink pixels against the unchecked box's \
         {empty} — the tick glyph is not being drawn"
    );
    assert!(
        dashed > empty,
        "the indeterminate box paints {dashed} ink pixels against the unchecked \
         box's {empty} — the dash glyph is not being drawn"
    );
}

#[test]
fn ticking_a_checkbox_changes_the_pixels_in_its_box() {
    let mut capture = capture_session();

    let off = AtomsState {
        checkboxes: vec![CheckState::Unchecked; data::CHECKBOX_ROWS.len()],
        ..AtomsState::new()
    };
    let on = AtomsState {
        checkboxes: vec![CheckState::Checked; data::CHECKBOX_ROWS.len()],
        ..AtomsState::new()
    };

    let probe = Probe::capture_with(
        capture.renderer_mut(),
        PANE_VIEWPORT,
        pane_atoms::checkbox_section(&off).element,
    );
    let row = probe
        .find("demo.atoms.checkbox.0")
        .expect("the first checkbox row must be addressable")
        .bounds;

    let diff = capture_diff(
        &mut capture,
        PANE_VIEWPORT,
        pane_atoms::checkbox_section as AtomBuilder,
        &off,
        &on,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        checkbox_indicator(row),
        "moving the first checkbox from Unchecked to Checked",
    );
}

// ---------------------------------------------------------------------------
// Radio group, switch, segmented, split button
// ---------------------------------------------------------------------------

#[test]
fn the_radio_group_paints_every_option_it_carries() {
    let state = AtomsState::new();
    let mut capture = capture_session();
    let captured = Captured::capture(&mut capture, PANE_VIEWPORT, || {
        pane_atoms::radio_section(&state)
    });

    let group = captured.control("demo.atoms.radio");
    let per_option = group.width / data::RADIO_OPTIONS.len() as f32;

    for (index, (id, label)) in data::RADIO_OPTIONS.iter().enumerate() {
        let band = Rectangle {
            x: group.x + per_option * index as f32,
            y: group.y,
            width: per_option,
            height: group.height,
        };
        assert!(
            captured.frame.ink_count(band) >= MIN_CHANGED_PIXELS,
            "radio option `{id}` ({label}) paints no readable ink in its band \
             {band:?} of the group {group:?}"
        );
    }
}

#[test]
fn flipping_the_live_switch_repaints_its_row() {
    let mut capture = capture_session();

    let on = AtomsState {
        switch_on: true,
        ..AtomsState::new()
    };
    let off = AtomsState {
        switch_on: false,
        ..AtomsState::new()
    };

    let probe = Probe::capture_with(
        capture.renderer_mut(),
        PANE_VIEWPORT,
        pane_atoms::switch_section(&on).element,
    );
    let row = probe
        .find("demo.atoms.switch.live")
        .expect("the live switch must be addressable")
        .bounds;

    let diff = capture_diff(
        &mut capture,
        PANE_VIEWPORT,
        pane_atoms::switch_section as AtomBuilder,
        &on,
        &off,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        row,
        "flipping the live toggle switch from on to off",
    );
}

#[test]
fn selecting_a_different_segment_repaints_the_strip() {
    let mut capture = capture_session();

    let before = AtomsState::new();
    let after = AtomsState {
        segmented: before.segmented.clone().with_selection(
            data::SEGMENT_ITEMS
                .iter()
                .filter(|(id, _)| *id == "month")
                .map(|(id, _)| id.to_string())
                .collect::<BTreeSet<String>>(),
        ),
        ..AtomsState::new()
    };

    let probe = Probe::capture_with(
        capture.renderer_mut(),
        PANE_VIEWPORT,
        pane_atoms::segmented_section(&before).element,
    );
    let strip = probe
        .find("demo.atoms.segmented")
        .expect("the segmented strip must be addressable")
        .bounds;

    let diff = capture_diff(
        &mut capture,
        PANE_VIEWPORT,
        pane_atoms::segmented_section as AtomBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        strip,
        "moving the segmented selection from `day` to `month`",
    );
}

#[test]
fn the_split_button_paints_across_its_whole_control() {
    let state = AtomsState::new();
    let mut capture = capture_session();
    let captured = Captured::capture(&mut capture, PANE_VIEWPORT, || {
        pane_atoms::split_button_section(&state)
    });

    let control = captured.control("demo.atoms.split.control");
    assert!(
        captured.frame.ink_count(control) >= MIN_CHANGED_PIXELS,
        "the split button at {control:?} paints no readable ink"
    );
    assert!(
        !captured.frame.is_uniform(control, SURFACE_TOLERANCE),
        "the split button at {control:?} is a flat patch — neither the label \
         nor the divider is being drawn"
    );
}

// ---------------------------------------------------------------------------
// Hyperlinks and text fields
// ---------------------------------------------------------------------------

#[test]
fn every_hyperlink_paints_its_label_inside_its_own_box() {
    let mut capture = capture_session();
    let captured = Captured::capture(&mut capture, PANE_VIEWPORT, pane_atoms::hyperlink_section);

    for id in ["demo.atoms.link.internal", "demo.atoms.link.external"] {
        let bounds = captured.control(id);
        let painted = captured
            .frame
            .ink_bounds(bounds)
            .unwrap_or_else(|| panic!("{id} painted no ink inside its own bounds {bounds:?}"));
        assert!(
            contained_within(painted, bounds, 1.0),
            "{id}'s ink {painted:?} runs outside its own bounds {bounds:?}"
        );
        assert!(
            painted.width > 8.0,
            "{id} painted only a {painted:?} sliver of text — the label is \
             clipped away"
        );
    }
}

#[test]
fn every_text_field_paints_at_its_recorded_rect() {
    let state = AtomsState::new();
    let mut capture = capture_session();
    let captured = Captured::capture(&mut capture, PANE_VIEWPORT, || {
        pane_atoms::text_input_section(&state)
    });

    for id in [
        "demo.atoms.text.live",
        "demo.atoms.text.readonly",
        "demo.atoms.text.disabled",
        "demo.atoms.text.controlled",
    ] {
        let bounds = captured.control(id);
        assert!(
            captured
                .frame
                .painted_count(bounds, render_section::card_fill_rgb(), 2)
                >= MIN_CHANGED_PIXELS,
            "the text field `{id}` lays out at {bounds:?} but paints nothing \
             over the card behind it"
        );
    }
}

#[test]
fn an_invalid_field_paints_differently_from_a_clean_one() {
    let mut capture = capture_session();

    let before = AtomsState::new();
    let after = AtomsState {
        text_invalid_model: TextInputModel::new(data::TEXT_INVALID_SAMPLE),
        ..AtomsState::new()
    };

    let diff = capture_diff(
        &mut capture,
        PANE_VIEWPORT,
        pane_atoms::text_input_section as AtomBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "clearing the invalid field's validation message and touched stance",
    );
}

#[test]
fn revealing_the_password_changes_the_painted_pixels() {
    let mut capture = capture_session();

    let masked = AtomsState {
        revealed: false,
        password: String::from("hunter2-secret"),
        ..AtomsState::new()
    };
    let shown = AtomsState {
        revealed: true,
        password: String::from("hunter2-secret"),
        ..AtomsState::new()
    };

    let probe = Probe::capture_with(
        capture.renderer_mut(),
        PANE_VIEWPORT,
        pane_atoms::password_section(&masked).element,
    );
    let field = probe
        .find("demo.atoms.password.live")
        .expect("the live password field must be addressable")
        .bounds;

    let diff = capture_diff(
        &mut capture,
        PANE_VIEWPORT,
        pane_atoms::password_section as AtomBuilder,
        &masked,
        &shown,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        field,
        "revealing the password field",
    );
}

// ---------------------------------------------------------------------------
// Sliders
// ---------------------------------------------------------------------------

#[test]
fn each_range_thumb_paints_at_its_recorded_rect() {
    let state = AtomsState::new();
    let mut capture = capture_session();
    let captured = Captured::capture(&mut capture, PANE_VIEWPORT, || {
        pane_atoms::range_slider_section(&state)
    });

    let lower = captured.control("demo.atoms.range.live.lower");
    let upper = captured.control("demo.atoms.range.live.upper");

    for (name, rect) in [("lower", lower), ("upper", upper)] {
        assert!(
            captured
                .frame
                .painted_count(rect, render_section::card_fill_rgb(), 2)
                >= MIN_CHANGED_PIXELS,
            "the {name} range thumb lays out at {rect:?} and paints nothing"
        );
    }
    assert!(
        upper.x > lower.x,
        "the upper thumb ({upper:?}) is not to the right of the lower one ({lower:?})"
    );
}

#[test]
fn moving_the_range_thumbs_moves_the_ink() {
    let mut capture = capture_session();

    let before = AtomsState::new();
    let after = AtomsState {
        range: range_with(180.0, 220.0),
        ..AtomsState::new()
    };

    let diff = capture_diff(
        &mut capture,
        PANE_VIEWPORT,
        pane_atoms::range_slider_section as AtomBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "dragging both range thumbs toward the top of the scale",
    );
}

// ---------------------------------------------------------------------------
// The remaining sections — body-region and repaint coverage only
// ---------------------------------------------------------------------------

#[test]
fn the_avatar_row_paints_a_patch_per_roster_member() {
    let mut capture = capture_session();
    let captured = Captured::capture(&mut capture, PANE_VIEWPORT, pane_atoms::avatar_section);

    // The avatars are a left-packed `Row` at three different sizes, not
    // an even grid, so the bands are taken from the span the row actually
    // painted rather than from the card's width.
    let span = captured
        .frame
        .ink_bounds(captured.body)
        .expect("the avatar row painted no ink at all inside its own body");
    let per_avatar = span.width / data::AVATAR_ROSTER.len() as f32;

    for (index, (name, _)) in data::AVATAR_ROSTER.iter().enumerate() {
        let band = Rectangle {
            x: span.x + per_avatar * index as f32,
            y: span.y,
            width: per_avatar,
            height: span.height,
        };
        assert!(
            captured.frame.ink_count(band) >= MIN_CHANGED_PIXELS,
            "avatar `{name}` paints no readable ink in its band {band:?} of \
             the row's painted span {span:?}"
        );
    }
}

#[test]
fn the_picker_sections_paint_their_closed_fields() {
    let state = AtomsState::new();
    let mut capture = capture_session();

    for (name, build) in [
        (
            "date picker",
            pane_atoms::date_picker_section as AtomBuilder,
        ),
        (
            "time picker",
            pane_atoms::time_picker_section as AtomBuilder,
        ),
        (
            "colour picker",
            pane_atoms::color_picker_section as AtomBuilder,
        ),
    ] {
        let captured = Captured::capture(&mut capture, PANE_VIEWPORT, || build(&state));
        assert!(
            captured.body_paint() >= MIN_CHANGED_PIXELS * 4,
            "the {name} section paints almost nothing in its body {:?} — the \
             closed field is not being drawn",
            captured.body
        );
    }
}

#[test]
fn clearing_the_search_query_repaints_the_box() {
    let mut capture = capture_session();

    let before = AtomsState::new();
    let after = AtomsState {
        search: String::new(),
        ..AtomsState::new()
    };

    let diff = capture_diff(
        &mut capture,
        PANE_VIEWPORT,
        pane_atoms::search_box_section as AtomBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "clearing the search query, which must swap the trailing `×` for the magnifier",
    );
}

#[test]
fn clearing_the_rating_repaints_the_pips() {
    let mut capture = capture_session();

    let before = AtomsState::new();
    let after = AtomsState {
        rating: before.rating.clone().with_value(None),
        ..AtomsState::new()
    };

    let diff = capture_diff(
        &mut capture,
        PANE_VIEWPORT,
        pane_atoms::rating_section as AtomBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "clearing the rating from 4 to none",
    );
}

#[test]
fn opening_the_combo_box_repaints_the_section() {
    let mut capture = capture_session();

    let before = AtomsState::new();
    let after = AtomsState {
        combo: before.combo.clone().with_open(true),
        ..AtomsState::new()
    };

    let diff = capture_diff(
        &mut capture,
        PANE_VIEWPORT,
        pane_atoms::combo_box_section as AtomBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "opening the combo box flyout",
    );
}

#[test]
fn the_tooltip_targets_paint_their_labels() {
    let mut capture = capture_session();
    let captured = Captured::capture(&mut capture, PANE_VIEWPORT, pane_atoms::tooltip_section);

    assert!(
        captured.body_ink() >= MIN_CHANGED_PIXELS,
        "the tooltip section's body {:?} shows no readable ink — the hover \
         targets are blank",
        captured.body
    );
}

#[test]
fn the_search_box_and_combo_box_paint_their_closed_fields() {
    let state = AtomsState::new();
    let mut capture = capture_session();

    for (name, build) in [
        ("search box", pane_atoms::search_box_section as AtomBuilder),
        ("combo box", pane_atoms::combo_box_section as AtomBuilder),
    ] {
        let captured = Captured::capture(&mut capture, PANE_VIEWPORT, || build(&state));
        assert!(
            captured.body_paint() >= MIN_CHANGED_PIXELS * 4,
            "the {name} paints almost nothing in its body {:?} — the closed \
             field is not being drawn",
            captured.body
        );
    }
}

#[test]
fn every_atoms_section_still_paints_after_its_state_moves() {
    let mut capture = capture_session();

    let before = AtomsState::new();
    let after = AtomsState {
        switch_on: false,
        revealed: true,
        search: String::new(),
        checkboxes: vec![CheckState::Unchecked; data::CHECKBOX_ROWS.len()],
        ..AtomsState::new()
    };

    for (name, build) in atom_builders() {
        let diff = capture_diff(&mut capture, PANE_VIEWPORT, build, &before, &after);
        assert!(
            diff.after.ink_count(diff.body) >= render_section::MIN_BODY_INK,
            "section {name} went blank after the pane's state moved: body {:?} \
             holds {} ink pixels",
            diff.body,
            diff.after.ink_count(diff.body)
        );
    }
}
