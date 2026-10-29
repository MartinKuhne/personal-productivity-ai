//! Tests for the combobox trigger's geometry and paint — `UIF-ATOM-004`, §3.4.
//!
//! Three groups, in order:
//!
//! 1. **The rungs and the box.** The two heights come from the two
//!    control tokens, and the trigger's bounds follow the rung.
//! 2. **The trailing slot.** The slot is a hit target, not a glyph: it
//!    is 24px, flush right, vertically centred, and the 14px chevron
//!    sits inside it rather than filling it.
//! 3. **The chevron's colour.** §3.4's two named states, and the
//!    fall-through to the dropdown-trigger atom's own icon channel for
//!    every state §3.4 leaves alone — including the proof that no
//!    reachable trigger fill is a locked solid fill, which is what keeps
//!    `text.muted` from ever becoming the `1.84:1` pairing `INC-40`
//!    established is unusable.

use super::*;
use crate::token::contrast::fill_lock_role;
use crate::widget::combo_model::{ComboBoxModel, SelectOption};
use crate::widget::text_input::InputKind;
use iced::{Element, Length};

/// Every state a trigger can be resolved at, including the two the
/// `focused` flag doubles up.
fn every_state() -> Vec<VisualState> {
    let bases = [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Invalid,
        BaseState::Disabled,
    ];
    let mut out = Vec::new();
    for base in bases {
        for focused in [false, true] {
            out.push(VisualState { base, focused });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 1 — the rungs and the box
// ---------------------------------------------------------------------------

#[test]
fn the_two_rungs_are_the_two_control_height_tokens() {
    assert_eq!(ComboRung::Standard.height(), size::CONTROL_MD);
    assert_eq!(ComboRung::Compact.height(), size::CONTROL_SM);
    assert_eq!(HEIGHT_STANDARD, size::CONTROL_MD);
    assert_eq!(HEIGHT_COMPACT, size::CONTROL_SM);
}

#[test]
fn the_rungs_are_sampled_standard_first() {
    assert_eq!(RUNGS, [ComboRung::Standard, ComboRung::Compact]);
    assert_eq!(ComboRung::Standard.slug(), "std");
    assert_eq!(ComboRung::Compact.slug(), "cmp");
}

#[test]
fn the_trigger_box_follows_the_rung_and_never_goes_negative() {
    for rung in RUNGS {
        let box_ = trigger_bounds(Point::new(10.0, 20.0), 160.0, rung);
        assert_eq!(box_.x, 10.0);
        assert_eq!(box_.y, 20.0);
        assert_eq!(box_.width, 160.0);
        assert_eq!(box_.height, rung.height());
    }
    let clamped = trigger_bounds(Point::ORIGIN, -50.0, ComboRung::Standard);
    assert_eq!(clamped.width, 0.0);
}

#[test]
fn the_padding_and_slot_constants_are_tokens() {
    assert_eq!(HORIZONTAL_PADDING, space::MD);
    assert_eq!(SLOT_GAP, space::SM);
    assert_eq!(TRAILING_SLOT, size::HIT_TARGET_MIN);
    assert_eq!(CHEVRON_SIZE, size::ICON_SM);
    assert_eq!(RADIUS, shape::RADIUS_WIDGET);
}

// ---------------------------------------------------------------------------
// 2 — the trailing slot is a hit target
// ---------------------------------------------------------------------------

#[test]
fn the_slot_is_a_square_flush_to_the_right_edge() {
    let trigger = trigger_bounds(Point::ORIGIN, 160.0, ComboRung::Standard);
    let slot = trailing_slot_bounds(trigger);
    assert_eq!(slot.width, TRAILING_SLOT);
    assert_eq!(slot.height, TRAILING_SLOT);
    assert_eq!(slot.x, trigger.x + trigger.width - TRAILING_SLOT);
    assert_eq!(slot.y, trigger.y + (trigger.height - TRAILING_SLOT) / 2.0);
}

#[test]
fn the_slot_is_centred_on_both_rungs() {
    for rung in RUNGS {
        let trigger = trigger_bounds(Point::ORIGIN, 160.0, rung);
        let slot = trailing_slot_bounds(trigger);
        let above = slot.y - trigger.y;
        let below = trigger.y + trigger.height - (slot.y + slot.height);
        assert!(
            (above - below).abs() < 1e-4,
            "{}: slot is not vertically centred ({above} vs {below})",
            rung.slug()
        );
    }
}

#[test]
fn the_slot_clamps_to_a_trigger_shorter_than_the_hit_target() {
    let short = Rectangle::new(Point::ORIGIN, Size::new(100.0, 10.0));
    let slot = trailing_slot_bounds(short);
    assert_eq!(slot.height, 10.0, "the slot must not overflow its control");
    assert_eq!(slot.y, 0.0);
    assert_eq!(slot.x, 90.0);
}

#[test]
fn the_chevron_is_centred_inside_the_slot_and_smaller_than_it() {
    let trigger = trigger_bounds(Point::ORIGIN, 160.0, ComboRung::Standard);
    let slot = trailing_slot_bounds(trigger);
    let chevron = chevron_bounds(trigger);
    assert_eq!(chevron.width, CHEVRON_SIZE);
    assert_eq!(chevron.height, CHEVRON_SIZE);
    const { assert!(CHEVRON_SIZE < TRAILING_SLOT) };
    assert!(
        (chevron.x + CHEVRON_SIZE / 2.0 - (slot.x + slot.width / 2.0)).abs() < 1e-4,
        "the glyph must sit in the middle of the area that responds"
    );
    assert!(
        (chevron.y + CHEVRON_SIZE / 2.0 - (slot.y + slot.height / 2.0)).abs() < 1e-4,
        "the glyph must sit in the middle of the area that responds"
    );
}

#[test]
fn the_chevron_is_always_inside_the_trigger() {
    for rung in RUNGS {
        let trigger = trigger_bounds(Point::new(3.0, 7.0), 120.0, rung);
        let chevron = chevron_bounds(trigger);
        assert!(chevron.x >= trigger.x);
        assert!(chevron.y >= trigger.y);
        assert!(chevron.x + chevron.width <= trigger.x + trigger.width);
        assert!(chevron.y + chevron.height <= trigger.y + trigger.height);
    }
}

#[test]
fn the_value_area_runs_from_padding_to_the_slot_and_never_overlaps_it() {
    let trigger = trigger_bounds(Point::ORIGIN, 160.0, ComboRung::Standard);
    let value = value_bounds(trigger);
    let slot = trailing_slot_bounds(trigger);
    assert_eq!(value.x, HORIZONTAL_PADDING);
    assert_eq!(value.height, trigger.height);
    assert!(
        value.x + value.width <= slot.x,
        "the value must not run under the glyph"
    );
    assert!(
        (slot.x - (value.x + value.width) - SLOT_GAP).abs() < 1e-4,
        "the gap is charged to the slot's side"
    );
}

#[test]
fn a_trigger_too_narrow_for_both_cannot_produce_an_inverted_value_area() {
    let tiny = Rectangle::new(Point::ORIGIN, Size::new(20.0, 32.0));
    let value = value_bounds(tiny);
    assert!(value.width >= 0.0);
}

// ---------------------------------------------------------------------------
// 3 — the chevron's colour
// ---------------------------------------------------------------------------

#[test]
fn at_rest_the_chevron_is_text_muted() {
    let palette = Palette::dark();
    let rest = VisualState {
        base: BaseState::Rest,
        focused: false,
    };
    assert_eq!(chevron_color(rest, false, &palette), palette.text_muted);
}

#[test]
fn on_hover_the_chevron_is_text_strong_as_3_4_states() {
    let palette = Palette::dark();
    let hovered = VisualState {
        base: BaseState::Hovered,
        focused: false,
    };
    assert_eq!(chevron_color(hovered, false, &palette), palette.text_strong);
    assert_eq!(chevron_color(hovered, true, &palette), palette.text_strong);
}

#[test]
fn an_open_flyout_makes_the_chevron_strong_from_any_live_state() {
    let palette = Palette::dark();
    let rest = VisualState {
        base: BaseState::Rest,
        focused: false,
    };
    assert_eq!(chevron_color(rest, true, &palette), palette.text_strong);
}

#[test]
fn focused_but_closed_takes_the_atoms_own_focused_icon_not_the_hover_rule() {
    // §3.4 names hover and open. A tabbed-to select with its flyout shut
    // is neither, so it falls through to the DropdownTrigger icon channel.
    let palette = Palette::dark();
    let focused = VisualState {
        base: BaseState::Rest,
        focused: true,
    };
    assert_eq!(
        chevron_color(focused, false, &palette),
        palette.text_primary
    );
    assert_ne!(chevron_color(focused, false, &palette), palette.text_strong);
}

#[test]
fn invalid_takes_the_dropdown_triggers_own_danger_icon() {
    let palette = Palette::dark();
    let invalid = VisualState {
        base: BaseState::Invalid,
        focused: false,
    };
    assert_eq!(
        chevron_color(invalid, false, &palette),
        palette.status_danger_text
    );
    assert_eq!(
        chevron_color(invalid, true, &palette),
        palette.status_danger_text
    );
}

#[test]
fn a_disabled_controls_chevron_is_dead_however_it_is_hovered() {
    let palette = Palette::dark();
    let disabled = VisualState {
        base: BaseState::Disabled,
        focused: false,
    };
    assert_eq!(
        chevron_color(disabled, false, &palette),
        palette.text_disabled
    );
    assert_eq!(
        chevron_color(disabled, true, &palette),
        palette.text_disabled
    );
}

#[test]
fn the_chevron_never_resolves_to_an_unmapped_or_transparent_colour() {
    let palette = Palette::dark();
    for state in every_state() {
        for open in [false, true] {
            let colour = chevron_color(state, open, &palette);
            assert!(
                colour.a > 0.99,
                "{:?} open={open}: chevron is translucent",
                state.base
            );
        }
    }
}

#[test]
fn no_reachable_trigger_fill_is_a_locked_solid_fill() {
    // The safety proof behind §3.4's `text.muted` chevron. `text.muted`
    // on `accent.fill` is 1.8363:1 — the unusable pairing `INC-40`
    // established. If any state ever resolved the trigger onto a locked
    // solid fill, the muted glyph would silently become that pairing.
    let palette = Palette::dark();
    for state in every_state() {
        let style = resolve_trigger(state, &palette);
        assert!(
            !fill_lock_role(style.background, &palette).is_locked_fill(),
            "{:?} resolves the trigger onto a locked solid fill; the \
             text.muted chevron cannot be painted there",
            state.base
        );
    }
}

#[test]
fn the_trigger_resolves_through_the_dropdown_trigger_input_row() {
    let palette = Palette::dark();
    for state in every_state() {
        assert_eq!(
            resolve_trigger(state, &palette),
            text_input::resolve_input_style(InputKind::DropdownTrigger, state, &palette),
            "{:?} must take the atom's own dropdown row, not a copy of it",
            state.base
        );
    }
}

#[test]
fn the_resting_chevron_agrees_with_the_atom_and_the_spec() {
    // The one state where §3.4 and the input atom's icon channel agree,
    // stated as a test so a change to either side has to notice.
    let palette = Palette::dark();
    let rest = VisualState {
        base: BaseState::Rest,
        focused: false,
    };
    assert_eq!(
        chevron_color(rest, false, &palette),
        resolve_trigger(rest, &palette).icon
    );
}

#[test]
fn the_hover_chevron_deliberately_differs_from_the_atoms_icon_channel() {
    // §3.4 says `text.strong` on hover; the input atom's icon goes
    // `text_primary`. The override is the point, so the difference is
    // asserted rather than allowed to drift back into agreement.
    let palette = Palette::dark();
    let hovered = VisualState {
        base: BaseState::Hovered,
        focused: false,
    };
    assert_eq!(chevron_color(hovered, false, &palette), palette.text_strong);
    assert_ne!(
        chevron_color(hovered, false, &palette),
        resolve_trigger(hovered, &palette).icon
    );
}

// ---------------------------------------------------------------------------
// 4 — the ComboBox widget wrapper (§3.4, UIF-ATOM-004)
// ---------------------------------------------------------------------------

#[test]
fn test_combobox_rung_dimensions() {
    let items = vec![SelectOption::new("1", "One"), SelectOption::new("2", "Two")];
    let model = ComboBoxModel::new(items, "Select number...");

    // Default rung is Standard (32px)
    let combo = ComboBox::<(), _>::new(&model);
    assert_eq!(combo.rung(), ComboRung::Standard);
    assert_eq!(combo.trigger_height(), size::CONTROL_MD);
    assert_eq!(combo.trigger_height(), 32.0);

    // Compact rung is 28px
    let compact = combo.with_rung(ComboRung::Compact);
    assert_eq!(compact.rung(), ComboRung::Compact);
    assert_eq!(compact.trigger_height(), size::CONTROL_SM);
    assert_eq!(compact.trigger_height(), 28.0);
}

#[test]
fn test_combobox_placeholder_when_selection_is_none() {
    let items = vec![
        SelectOption::new("a", "Alpha"),
        SelectOption::new("b", "Beta"),
    ];
    let model = ComboBoxModel::new(items, "Pick a greek letter");
    let combo = ComboBox::<(), _>::new(&model);

    assert!(combo.shows_placeholder());
    assert_eq!(combo.display_label(), "Pick a greek letter");
}

#[test]
fn test_combobox_selected_item_label_when_selection_is_some() {
    let items = vec![
        SelectOption::new("a", "Alpha"),
        SelectOption::new("b", "Beta"),
    ];
    let model = ComboBoxModel::new(items, "Pick a greek letter").with_selected("b");
    let combo = ComboBox::<(), _>::new(&model);

    assert!(!combo.shows_placeholder());
    assert_eq!(combo.display_label(), "Beta");
}

#[test]
fn test_combobox_chevron_color_across_states() {
    let items = vec![SelectOption::new("x", "X-Ray")];
    let palette = Palette::dark();

    // Rest, closed
    let model_closed = ComboBoxModel::new(items.clone(), "Select...").with_open(false);
    let combo_closed = ComboBox::<(), _>::new(&model_closed).with_palette(palette);
    let rest_state = VisualState {
        base: BaseState::Rest,
        focused: false,
    };
    assert_eq!(combo_closed.chevron_color(rest_state), palette.text_muted);

    // Open
    let model_open = ComboBoxModel::new(items.clone(), "Select...").with_open(true);
    let combo_open = ComboBox::<(), _>::new(&model_open).with_palette(palette);
    assert_eq!(combo_open.chevron_color(rest_state), palette.text_strong);

    // Disabled
    let model_disabled = ComboBoxModel::new(items, "Select...").disabled();
    let combo_disabled = ComboBox::<(), _>::new(&model_disabled).with_palette(palette);
    let disabled_state = VisualState {
        base: BaseState::Disabled,
        focused: false,
    };
    assert_eq!(
        combo_disabled.chevron_color(disabled_state),
        palette.text_disabled
    );
}

#[test]
fn test_combobox_min_width_clamping() {
    let items = vec![SelectOption::new("1", "One")];
    let model = ComboBoxModel::new(items, "Select...");

    // Default width is Length::Shrink
    let combo = ComboBox::<(), _>::new(&model);
    assert_eq!(combo.width(), Length::Shrink);

    // Explicit small width clamps to MIN_WIDTH (120px)
    let clamped = combo.with_width(Length::Fixed(80.0));
    assert_eq!(clamped.width(), Length::Fixed(MIN_WIDTH));
    assert_eq!(clamped.width(), Length::Fixed(120.0));

    // Width larger than MIN_WIDTH is retained
    let wide = clamped.with_width(Length::Fixed(240.0));
    assert_eq!(wide.width(), Length::Fixed(240.0));

    // Fill width is retained
    let fill = wide.with_width(Length::Fill);
    assert_eq!(fill.width(), Length::Fill);
}

#[test]
fn test_combobox_element_conversion_and_flyout_mounting() {
    #[derive(Clone, Debug, PartialEq)]
    enum Msg {
        Toggle,
        Select(&'static str),
    }

    let items = vec![
        SelectOption::new("1", "First"),
        SelectOption::new("2", "Second"),
    ];

    // Closed combobox: flyout not mounted
    let model_closed = ComboBoxModel::new(items.clone(), "Select...").with_open(false);
    let combo_closed = ComboBox::new(&model_closed)
        .on_toggle_open(Msg::Toggle)
        .on_select(Msg::Select);
    assert!(!combo_closed.is_flyout_mounted());
    let _el_closed: Element<'_, Msg> = combo_closed.into();

    // Open combobox: flyout mounted
    let model_open = ComboBoxModel::new(items, "Select...").with_open(true);
    let combo_open = ComboBox::new(&model_open)
        .on_toggle_open(Msg::Toggle)
        .on_select(Msg::Select);
    assert!(combo_open.is_flyout_mounted());
    let _el_open: Element<'_, Msg> = combo_open.into();

    // Open combobox with empty items: flyout not mounted
    let empty_items: Vec<SelectOption<&'static str>> = Vec::new();
    let model_empty = ComboBoxModel::new(empty_items, "Select...").with_open(true);
    let combo_empty = ComboBox::<Msg, _>::new(&model_empty);
    assert!(!combo_empty.is_flyout_mounted());
    let _el_empty: Element<'_, Msg> = combo_empty.into();
}

#[test]
fn test_combobox_disabled_suppresses_interaction() {
    #[derive(Clone, Debug, PartialEq)]
    struct Msg;

    let items = vec![SelectOption::new("1", "One")];
    let model = ComboBoxModel::new(items, "Select...").disabled();
    let combo = ComboBox::new(&model).on_toggle_open(Msg);

    assert!(combo.is_disabled());
    let _el: Element<'_, Msg> = combo.into();
}

#[test]
fn test_combobox_compact_min_width_clamping() {
    assert_eq!(MIN_WIDTH_COMPACT, 64.0);

    let items = vec![SelectOption::new("1", "One")];
    let model = ComboBoxModel::new(items, "Select...");

    let combo = ComboBox::<(), _>::new(&model).with_rung(ComboRung::Compact);

    // Clamps below MIN_WIDTH_COMPACT (64px)
    let clamped = combo.with_width(Length::Fixed(40.0));
    assert_eq!(clamped.width(), Length::Fixed(MIN_WIDTH_COMPACT));
    assert_eq!(clamped.width(), Length::Fixed(64.0));

    // Above 64px is retained
    let retained = combo.with_width(Length::Fixed(80.0));
    assert_eq!(retained.width(), Length::Fixed(80.0));

    // Explicit with_compact_width
    let explicit = ComboBox::<(), _>::new(&model).with_compact_width(Length::Fixed(30.0));
    assert_eq!(explicit.width(), Length::Fixed(64.0));
}

#[test]
fn test_stateless_select_helper_ergonomics() {
    #[derive(Clone, Debug, PartialEq)]
    enum Msg {
        Chosen(String),
    }

    let options = vec!["Rust".to_string(), "Go".to_string(), "C++".to_string()];
    let selected = Some(&options[0]);

    let widget = select(&options, selected, Msg::Chosen)
        .with_placeholder("Pick language...")
        .with_rung(ComboRung::Compact)
        .with_width(Length::Fixed(96.0));

    let _el: Element<'_, Msg> = widget.into();
}

#[test]
fn test_combobox_zero_content_shift_contract() {
    // Both open and closed ComboBox elements produce the exact same trigger height (UIF-PRIN-003)
    let items = vec![SelectOption::new("1", "One"), SelectOption::new("2", "Two")];

    let model_closed = ComboBoxModel::new(items.clone(), "Select...").with_open(false);
    let combo_closed = ComboBox::<(), _>::new(&model_closed);
    assert_eq!(combo_closed.trigger_height(), 32.0);

    let model_open = ComboBoxModel::new(items, "Select...").with_open(true);
    let combo_open = ComboBox::<(), _>::new(&model_open);
    assert_eq!(combo_open.trigger_height(), 32.0);
    assert_eq!(combo_closed.trigger_height(), combo_open.trigger_height());
}
