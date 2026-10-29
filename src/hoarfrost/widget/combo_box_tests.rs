//! Tests for the combobox trigger's geometry and paint â€” `UIF-ATOM-004`, Â§3.4.
//!
//! Three groups, in order:
//!
//! 1. **The rungs and the box.** The two heights come from the two
//!    control tokens, and the trigger's bounds follow the rung.
//! 2. **The trailing slot.** The slot is a hit target, not a glyph: it
//!    is 24px, flush right, vertically centred, and the 14px chevron
//!    sits inside it rather than filling it.
//! 3. **The chevron's colour.** Â§3.4's two named states, and the
//!    fall-through to the dropdown-trigger atom's own icon channel for
//!    every state Â§3.4 leaves alone â€” including the proof that no
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
// 1 â€” the rungs and the box
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
// 2 â€” the trailing slot is a hit target
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
// 3 â€” the chevron's colour
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
    // Â§3.4 names hover and open. A tabbed-to select with its flyout shut
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
    // The safety proof behind Â§3.4's `text.muted` chevron. `text.muted`
    // on `accent.fill` is 1.8363:1 â€” the unusable pairing `INC-40`
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
    // The one state where Â§3.4 and the input atom's icon channel agree,
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
    // Â§3.4 says `text.strong` on hover; the input atom's icon goes
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
// 4 â€” the ComboBox widget wrapper (Â§3.4, UIF-ATOM-004)
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

// ---------------------------------------------------------------------------
// Publishing the control's own box (UIF-ATOM-004)
//
// The rewrite at `a74e143` turned the ComboBox into a custom-drawn widget and,
// in doing so, dropped the anonymous `container` record the control used to put
// on the operation stream. Measured against the demo's own probe: 3 ComboBox
// instances, `anonymous 1823` â†’ `1820` at the root manifest and `963` â†’ `960`
// in the atoms pane, with the named count unchanged at 183 / 105. The control
// drew a full trigger and no host could see it. `Select` was worse â€” no
// `operate` at all, so `RecordCandidates` came back empty.
//
// These tests pin the restored publish and, just as importantly, pin that it is
// exactly one record: the wrapper sits on top of an iced `button` that publishes
// itself through the pass-through, so a careless fix lands two boxes where one
// belongs.
// ---------------------------------------------------------------------------

/// A trigger stand-in whose own box is **inset** from the wrapper's.
///
/// The whole point of the inset is that the wrapper's box and the trigger's box
/// are then different rectangles. Against the real trigger they coincide â€” the
/// wrapper delegates `layout` to the button, so both publish at the same
/// bounds â€” and a duplicate at one rectangle is indistinguishable from the
/// original. Give the two boxes different geometry and "did the wrapper add one
/// record or two?" becomes a count anyone can read.
struct InsetTrigger {
    inset: f32,
    node_size: Size,
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for InsetTrigger {
    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(self.node_size.width),
            Length::Fixed(self.node_size.height),
        )
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &iced::Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(self.node_size)
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn operate(
        &mut self,
        _tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, inset(layout.bounds(), self.inset));
    }
}

/// `rect` shrunk by `by` on every side.
fn inset(rect: Rectangle, by: f32) -> Rectangle {
    Rectangle::new(
        Point::new(rect.x + by, rect.y + by),
        Size::new(rect.width - by * 2.0, rect.height - by * 2.0),
    )
}

/// How many records were published at exactly `rect`.
fn at(rect: Rectangle, cands: &[crate::test_helpers::Candidate]) -> usize {
    cands.iter().filter(|c| c.bounds == rect).count()
}

/// The wrapper publishes its own box â€” exactly one, ahead of the trigger.
///
/// Built over [`InsetTrigger`] so the wrapper's rectangle and the trigger's are
/// not the same box. The whole tree must publish exactly two records: the
/// wrapper's at the outer rectangle and the trigger's at the inset one. A
/// wrapper that published twice would show three, or two at the outer rectangle;
/// a wrapper that published nothing would show one.
#[test]
fn the_wrapper_publishes_exactly_one_box_of_its_own() {
    #[derive(Clone, Debug, PartialEq)]
    struct Msg;

    let own = Size::new(200.0, 32.0);
    let inset_by = 4.0;
    let own_box = Rectangle::new(Point::new(0.0, 0.0), own);
    let trigger_box = inset(own_box, inset_by);

    let mut harness = crate::test_helpers::Harness::<Msg, _>::headless(|| {
        Element::new(ComboBoxWidget {
            trigger: Element::new(InsetTrigger {
                inset: inset_by,
                node_size: own,
            }),
            flyout: None,
            is_open: false,
            width: Length::Fixed(own.width),
            height: own.height,
            on_dismiss: None,
        })
    });

    let cands = harness.candidates();

    assert_eq!(
        cands.len(),
        2,
        "the wrapper plus its trigger must publish exactly two records, got {cands:?}"
    );
    assert_eq!(cands[0].kind, crate::test_helpers::KIND_CONTAINER);
    assert_eq!(
        cands[0].bounds, own_box,
        "the wrapper publishes its own box, first, before the trigger subtree"
    );
    assert_eq!(cands[1].kind, crate::test_helpers::KIND_CONTAINER);
    assert_eq!(cands[1].bounds, trigger_box);
    assert_eq!(
        at(own_box, &cands),
        1,
        "exactly one record at the wrapper's own box â€” a second one is the \
         double-publish this test exists to catch"
    );
}

/// A live ComboBox publishes its own box, at the trigger's real bounds.
///
/// The wrapper's record and the iced `button`'s own record are the same
/// rectangle because the wrapper delegates `layout` to the button â€” that is the
/// single-source guarantee, not an accident of the fixture.
#[test]
fn the_live_combobox_publishes_its_own_box_at_the_trigger_bounds() {
    #[derive(Clone, Debug, PartialEq)]
    struct Msg;

    let items = vec![SelectOption::new("1", "One"), SelectOption::new("2", "Two")];
    let model = ComboBoxModel::new(items, "Pick").with_open(false);

    let mut harness = crate::test_helpers::Harness::<Msg, _>::headless(|| {
        ComboBox::new(&model)
            .with_width(Length::Fill)
            .on_toggle_open(Msg)
            .into()
    });

    let cands = harness.candidates();

    assert_eq!(cands[0].kind, crate::test_helpers::KIND_CONTAINER);
    assert_eq!(
        cands[0].bounds, cands[1].bounds,
        "the wrapper's box must be the trigger's real box, not a box of similar size"
    );
    assert_eq!(
        cands[0].bounds.height,
        ComboRung::Standard.height(),
        "the published box is the standard control rung"
    );
    // The discriminating count. Three records sit at the trigger's box: the
    // wrapper's own, iced's `button`, and the inner `container` â€” the latter
    // two because the wrapper delegates `layout` to the button and the
    // button's padding is `ZERO`, so all three rectangles coincide. Two means
    // the wrapper published nothing of its own. The `cands[0] == cands[1]`
    // check above cannot see that: with the wrapper's record gone, `cands[0]`
    // and `cands[1]` become the button and the inner container, and they are
    // still the same rectangle.
    assert_eq!(
        at(cands[0].bounds, &cands),
        3,
        "the wrapper must contribute one of the three records at the trigger's \
         box; two means it published nothing of its own"
    );
    assert!(
        cands[0].id.is_none(),
        "the control publishes under no Id: naming is a host concern, and a name \
         invented here would move the demo's named count without the host asking"
    );
}

/// A **disabled** ComboBox still publishes its own box.
///
/// The fires-in-every-state rule. A disabled trigger carries no `on_press`, so
/// iced never registers a focus target for it â€” but the box is still where the
/// control is, and a host that cannot see a disabled control cannot grey out a
/// menu item pointing at it or report where it sits.
#[test]
fn the_disabled_combobox_still_publishes_its_own_box() {
    #[derive(Clone, Debug, PartialEq)]
    struct Msg;

    let items = vec![SelectOption::new("1", "One")];
    let model = ComboBoxModel::new(items, "Pick").disabled();

    let mut harness = crate::test_helpers::Harness::<Msg, _>::headless(|| {
        ComboBox::new(&model).with_width(Length::Fill).into()
    });

    let cands = harness.candidates();

    assert_eq!(cands[0].kind, crate::test_helpers::KIND_CONTAINER);
    assert_eq!(
        cands[0].bounds.height,
        ComboRung::Standard.height(),
        "a disabled trigger publishes the same box it draws"
    );
    // Same absolute count as the live control: the box is published in every
    // state, disabled included. A disabled trigger carries no `on_press`, so
    // nothing else in the tree is obliged to put a record at that rectangle on
    // the wrapper's behalf â€” drop the wrapper's publish and this reads two.
    assert_eq!(
        at(cands[0].bounds, &cands),
        3,
        "a disabled ComboBox still contributes its own box: three records at \
         the trigger rectangle, same as the live control"
    );
    assert!(
        !cands
            .iter()
            .any(|c| c.kind == crate::test_helpers::KIND_FOCUSABLE),
        "a disabled trigger must not become a Tab stop just because it now \
         publishes a box: {cands:?}"
    );
}

/// Opening the flyout does not add a second box at the trigger, and the flyout
/// itself is published exactly once.
///
/// The overlay is the one place in this module that must **not** publish its own
/// box: `ComboOverlay::layout` returns the flyout content's own node moved into
/// position, so the overlay's rectangle and the flyout container's rectangle are
/// the same node, and iced's `container` already publishes it.
#[test]
fn opening_the_flyout_publishes_the_flyout_once_and_not_the_trigger_twice() {
    #[derive(Clone, Debug, PartialEq)]
    struct Msg;

    let items = vec![SelectOption::new("1", "One"), SelectOption::new("2", "Two")];

    let closed_model = ComboBoxModel::new(items.clone(), "Pick").with_open(false);
    let closed: Vec<crate::test_helpers::Candidate> = {
        let mut harness = crate::test_helpers::Harness::<Msg, _>::headless(|| {
            ComboBox::new(&closed_model)
                .with_width(Length::Fill)
                .on_toggle_open(Msg)
                .into()
        });
        harness.candidates()
    };

    let open_model = ComboBoxModel::new(items, "Pick").with_open(true);
    let open: Vec<crate::test_helpers::Candidate> = {
        let mut harness = crate::test_helpers::Harness::<Msg, _>::headless(|| {
            ComboBox::new(&open_model)
                .with_width(Length::Fill)
                .on_toggle_open(Msg)
                .into()
        });
        harness.candidates()
    };

    let trigger_box = closed[0].bounds;
    // Absolute on both sides, not merely equal. A purely relative check passes
    // when the wrapper publishes nothing at all, because closed and open are
    // then short by the very same record and still match each other. Pinning
    // three at the trigger box in each state is what makes "the flyout added a
    // box" and "the wrapper never published one" two different failures.
    assert_eq!(
        at(trigger_box, &closed),
        3,
        "closed: the wrapper's own box plus iced's button and inner container"
    );
    assert_eq!(
        at(trigger_box, &open),
        3,
        "open: the wrapper still contributes its own box, and opening the \
         flyout adds nothing at the trigger's rectangle"
    );
    assert_eq!(
        at(trigger_box, &closed),
        at(trigger_box, &open),
        "opening the flyout must not add a record at the trigger's box"
    );

    // The flyout sits flush against the trigger's bottom edge, so its `y`
    // *equals* the trigger's height rather than exceeding it. Probing with
    // `>` lands on the `scrollable` inside the flyout instead, which the
    // flyout's own `container` also covers â€” two records at one rectangle,
    // and the count reads as a double-publish that never happened.
    let flyout = open
        .iter()
        .map(|c| c.bounds)
        .find(|b| b.y >= trigger_box.height)
        .expect("an open combo must publish something below the trigger");
    assert_eq!(
        at(flyout, &open),
        1,
        "the flyout box is published once, by the flyout's own container; the \
         overlay adding a second one is the double-publish this pins out"
    );
    assert!(
        open.len() > closed.len(),
        "the open tree must publish more than the closed one"
    );
}

/// `Select` publishes its own box.
///
/// Before this, `Select` had **no `operate` method at all** and iced's default
/// is an empty method, so the probe came back with `count=0`: no `container`,
/// no `text`, no `focusable`. A control that published nothing through every
/// hook is a strictly worse case than the ComboBox's missing own-box record, and
/// it is fixed the same way â€” same [`trigger_box`], same single source, and
/// `Select::draw` now paints from that same call.
#[test]
fn the_select_publishes_its_own_box() {
    #[derive(Clone, Debug, PartialEq)]
    struct Msg;

    let opts = vec![String::from("Alpha"), String::from("Beta")];

    let mut harness = crate::test_helpers::Harness::<Msg, _>::headless(|| {
        let sel: Option<&String> = None;
        Select::new(&opts, sel, |_| Msg)
            .with_width(Length::Fill)
            .into()
    });

    let cands = harness.candidates();

    assert_eq!(
        cands.len(),
        1,
        "a Select publishes exactly one record, got {cands:?}"
    );
    assert_eq!(cands[0].kind, crate::test_helpers::KIND_CONTAINER);
    assert_eq!(
        cands[0].bounds.height,
        ComboRung::Standard.height(),
        "the published box is the rung the trigger paints"
    );
    assert!(cands[0].id.is_none(), "no invented name");
}

/// A `Select` with no options still publishes its box.
///
/// An empty option list means the control can never be pressed and never opens a
/// flyout. It still occupies a rectangle, and that rectangle still has to be
/// findable.
#[test]
fn an_empty_select_still_publishes_its_own_box() {
    #[derive(Clone, Debug, PartialEq)]
    struct Msg;

    let opts: Vec<String> = Vec::new();

    let mut harness = crate::test_helpers::Harness::<Msg, _>::headless(|| {
        let sel: Option<&String> = None;
        Select::new(&opts, sel, |_| Msg)
            .with_width(Length::Fill)
            .into()
    });

    let cands = harness.candidates();

    assert_eq!(
        cands.len(),
        1,
        "an empty Select is still a control at a real box, got {cands:?}"
    );
    assert_eq!(cands[0].kind, crate::test_helpers::KIND_CONTAINER);
}

/// The compact rung publishes the compact box.
///
/// Guards that the published rectangle follows the rung rather than being pinned
/// to the standard height.
#[test]
fn the_compact_rung_publishes_the_compact_box() {
    #[derive(Clone, Debug, PartialEq)]
    struct Msg;

    let opts = vec![String::from("Alpha")];

    let mut harness = crate::test_helpers::Harness::<Msg, _>::headless(|| {
        let sel: Option<&String> = None;
        Select::new(&opts, sel, |_| Msg)
            .with_rung(ComboRung::Compact)
            .with_width(Length::Fill)
            .into()
    });

    let cands = harness.candidates();

    assert_eq!(cands.len(), 1);
    assert_eq!(cands[0].kind, crate::test_helpers::KIND_CONTAINER);
    assert_eq!(
        cands[0].bounds.height,
        ComboRung::Compact.height(),
        "the published box follows the rung"
    );
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
    let retained = ComboBox::<(), _>::new(&model)
        .with_rung(ComboRung::Compact)
        .with_width(Length::Fixed(80.0));
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
