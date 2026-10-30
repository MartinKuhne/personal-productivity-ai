//! Tests for the combobox trigger's geometry and paint â€” `UIF-ATOM-004`, Â§3.4.
//!
//! Seven groups, in order:
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
//! 4. **The wrapper's own box on the operation stream**, for the live,
//!    disabled and open states, and for `Select`.
//! 5. **The accessors the wrapper hands back** — the model it borrows,
//!    the rung, the palette and the message slots.
//! 6. **Driving the trigger and the card** through a real headless tree:
//!    what a press, a dismiss and Escape each publish, and where the
//!    card lands when there is no room below.
//! 7. **The stateless `Select`** — its shut-state box and messages, the
//!    prefix type-ahead of a closed control, the empty option list that can
//!    never mount, and the card it *does* mount: measuring it, picking a row,
//!    dismissing it with a click away, and type-ahead while open. Each is
//!    pinned as `defect_01_*` after the fix to the tree slot the flyout lays
//!    itself out against.

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

/// Every `(state, open)` pair whose colour §3.4 or the dropdown-trigger atom
/// names, in the priority order [`chevron_color`] documents.
///
/// One table rather than one test per row: the rows are nine readings of the
/// same five-arm `match`, and reading them top-to-bottom shows the precedence
/// that six separate test names cannot. Every assertion the split tests made is
/// here, labelled, plus the `Pressed` rung §3.4 leaves to the atom.
#[test]
fn the_chevron_colour_follows_the_documented_priority_order() {
    let palette = Palette::dark();
    let at = |base: BaseState, focused: bool| VisualState { base, focused };

    let cases: &[(&str, VisualState, bool, Color)] = &[
        // §3.4's rest state — the one place the spec and the atom agree.
        (
            "rest, closed",
            at(BaseState::Rest, false),
            false,
            palette.text_muted,
        ),
        // §3.4's hover rule, shut and open.
        (
            "hovered, closed",
            at(BaseState::Hovered, false),
            false,
            palette.text_strong,
        ),
        (
            "hovered, open",
            at(BaseState::Hovered, false),
            true,
            palette.text_strong,
        ),
        // §3.4's open rule reached from a plain resting state.
        (
            "rest, open",
            at(BaseState::Rest, false),
            true,
            palette.text_strong,
        ),
        // A pressed trigger reads as the active state, same as hover.
        (
            "pressed, closed",
            at(BaseState::Pressed, false),
            false,
            palette.text_strong,
        ),
        // Neither §3.4 rule applies, so the atom's focused-icon colour.
        (
            "focused, closed",
            at(BaseState::Rest, true),
            false,
            palette.text_primary,
        ),
        // The atom's own danger icon, flyout shut or open.
        (
            "invalid, closed",
            at(BaseState::Invalid, false),
            false,
            palette.status_danger_text,
        ),
        (
            "invalid, open",
            at(BaseState::Invalid, false),
            true,
            palette.status_danger_text,
        ),
        // A dead control's glyph is dead, however it is hovered.
        (
            "disabled, closed",
            at(BaseState::Disabled, false),
            false,
            palette.text_disabled,
        ),
        (
            "disabled, open",
            at(BaseState::Disabled, false),
            true,
            palette.text_disabled,
        ),
    ];

    for (label, state, open, expected) in cases {
        assert_eq!(
            chevron_color(*state, *open, &palette),
            *expected,
            "{label}: unexpected chevron colour"
        );
    }
}

#[test]
fn focused_but_closed_takes_the_atoms_own_focused_icon_not_the_hover_rule() {
    // Â§3.4 names hover and open. A tabbed-to select with its flyout shut
    // is neither, so it falls through to the DropdownTrigger icon channel.
    // Kept apart from the table above because equality alone would not tell
    // the two "brighter than muted" readings apart — only the `assert_ne!`
    // proves this is the atom's channel and not §3.4's hover rule.
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

// ---------------------------------------------------------------------------
// 5 — the accessors the wrapper hands back
// ---------------------------------------------------------------------------

/// The message vocabulary the interaction tests drive.
///
/// One enum carrying both tiers so a single harness can tell a toggle apart
/// from a pick — the claim "a row press reports the pick and *no* dismiss"
/// is only checkable if both emissions are visible in one log.
#[derive(Clone, Debug, PartialEq)]
enum ComboMsg {
    /// `on_toggle_open` — the trigger fired, or a dismiss was reported.
    Toggle,
    /// `on_select` — an option row was picked.
    Pick(&'static str),
}

/// The `Select` vocabulary: it has one emission, the pick, and no message at
/// all for opening or closing.
#[derive(Clone, Debug, PartialEq)]
enum PickMsg {
    /// `on_select` — from a row press or from a type-ahead hit.
    Chosen(String),
}

/// A palette that is recognisably not [`Palette::dark()`].
fn alt_palette() -> Palette {
    let mut other = Palette::dark();
    other.text_muted = Color::from_rgb(0.25, 0.5, 0.75);
    other
}

/// The smallest menu that can lay out more than one row.
fn two_options() -> Vec<SelectOption<&'static str>> {
    vec![
        SelectOption::new("one", "One"),
        SelectOption::new("two", "Two"),
    ]
}

/// A combo wired to [`ComboMsg`] at `Length::Fill`, so the trigger spans the
/// harness viewport and the card is as wide as the trigger.
fn wired_combo<'a>(model: &'a ComboBoxModel<&'static str>) -> ComboBox<'a, ComboMsg, &'static str> {
    ComboBox::new(model)
        .with_width(Length::Fill)
        .on_toggle_open(ComboMsg::Toggle)
        .on_select(ComboMsg::Pick)
}

/// [`wired_combo`] already resolved to an `Element`.
///
/// Same reason as [`spacer`]: the conversion has to happen where the target type
/// is concrete. Inside `column!` the target is still an inference variable, and
/// `ComboBox` reaches `Element` by two routes at once, so rustc reports the
/// child ambiguous rather than picking one.
fn combo_el<'a>(
    model: &'a ComboBoxModel<&'static str>,
) -> crate::test_helpers::El<'a, ComboMsg, iced_renderer::Renderer> {
    wired_combo(model).into()
}

/// A full-width spacer of `height`, already resolved to an `Element`.
///
/// The type has to be decided somewhere, and a function signature is the one
/// place rustc will do it for us: `column!` expands each child to its own
/// `.into()`, so with nothing anchored the renderer stays an inference variable
/// and every child inside the macro is reported ambiguous at once.
fn spacer(height: f32) -> crate::test_helpers::El<'static, ComboMsg, iced_renderer::Renderer> {
    iced::widget::Space::new()
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .into()
}

/// The trigger's centre when it fills the harness viewport.
fn combo_trigger_centre() -> Point {
    Point::new(
        crate::test_helpers::VIEWPORT.width / 2.0,
        size::CONTROL_MD / 2.0,
    )
}

/// The card's own box, read back off the operation stream.
///
/// The flyout's `container` publishes before anything inside it, so the first
/// record at or below the trigger's bottom edge is the card rather than a row.
fn flyout_of(cands: &[crate::test_helpers::Candidate], trigger: Rectangle) -> Rectangle {
    cands
        .iter()
        .map(|c| c.bounds)
        .find(|b| b.y >= trigger.y + trigger.height)
        .expect("an open combo must publish something below the trigger")
}

/// The centre of option row `index` inside a card laid out at `flyout`.
///
/// The card pads by [`space::XS`] and each row is [`size::CONTROL_MD`] tall
/// with [`space::XXS`] between them.
fn row_centre(flyout: Rectangle, index: usize) -> Point {
    Point::new(
        flyout.x + flyout.width / 2.0,
        flyout.y + space::XS + size::CONTROL_MD * (index as f32 + 0.5) + space::XXS * index as f32,
    )
}

/// A named key with the pointer nowhere near the control.
fn named_key(name: keyboard::key::Named) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(name),
        modified_key: keyboard::Key::Named(name),
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::NONE,
        text: None,
        repeat: false,
    })
}

/// A typed character, which is what `Select`'s type-ahead arm keys off — it
/// reads `text`, not the named key.
fn typed(c: &'static str) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Character(c.into()),
        modified_key: keyboard::Key::Character(c.into()),
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::NONE,
        text: Some(c.into()),
        repeat: false,
    })
}

#[test]
fn the_helper_and_the_builder_agree_and_the_accessors_report_the_same_values() {
    let model = ComboBoxModel::new(two_options(), "Pick")
        .with_selected("two")
        .with_open(true);

    let built = ComboBox::new(&model)
        .with_rung(ComboRung::Compact)
        .with_palette(alt_palette())
        .on_press(ComboMsg::Toggle);

    assert!(
        std::ptr::eq(built.model(), &model),
        "`model()` must hand back the host's own model, not a copy of it"
    );
    assert_eq!(built.rung(), ComboRung::Compact);
    assert_eq!(built.palette(), &alt_palette());
    assert_eq!(built.toggle_open_message(), Some(&ComboMsg::Toggle));
    assert_eq!(built.display_label(), "Two");
    assert!(!built.shows_placeholder());
    assert!(!built.is_disabled());
    assert!(built.is_flyout_mounted());
    assert_eq!(built.trigger_height(), size::CONTROL_SM);

    // `combo_box()` is the free-function spelling of `ComboBox::new`, and a
    // bare control reports the defaults rather than a plausible guess.
    let plain = combo_box::<(), &'static str>(&model);
    assert!(std::ptr::eq(plain.model(), &model));
    assert_eq!(plain.rung(), ComboRung::Standard);
    assert_eq!(plain.width(), Length::Shrink);
    assert_eq!(plain.palette(), &Palette::dark());
    assert_eq!(plain.toggle_open_message(), None);
    assert_eq!(plain.trigger_height(), size::CONTROL_MD);
}

#[test]
fn on_press_is_the_alias_of_on_toggle_open() {
    let model = ComboBoxModel::new(two_options(), "Pick");

    let via_alias = ComboBox::new(&model).on_press(ComboMsg::Toggle);
    let via_name = ComboBox::new(&model).on_toggle_open(ComboMsg::Toggle);

    assert_eq!(
        via_alias.toggle_open_message(),
        via_name.toggle_open_message(),
        "`on_press` is documented as an alias, so it must set the same slot"
    );
}

// ---------------------------------------------------------------------------
// 6 — driving the trigger and the card
// ---------------------------------------------------------------------------

/// The trigger is an `iced::widget::button` with `on_press` wired to the
/// toggle message, and iced's button arms itself on press and **fires on
/// release** (`iced_widget::button` sets `is_pressed` on `ButtonPressed` and
/// publishes on `ButtonReleased`). The message therefore arrives on the
/// release, not on the press — which is what a host sees and what this pins.
#[test]
fn the_trigger_toggle_arrives_on_the_release_the_way_an_iced_button_fires() {
    let model = ComboBoxModel::new(two_options(), "Pick").with_open(false);
    let mut harness =
        crate::test_helpers::Harness::<ComboMsg, _>::headless(|| wired_combo(&model).into());

    assert_eq!(
        harness.press(combo_trigger_centre()),
        Vec::new(),
        "the press only arms the button; nothing is published yet"
    );
    assert_eq!(
        harness.release(combo_trigger_centre()),
        vec![ComboMsg::Toggle],
        "the release is where the toggle message actually arrives"
    );
}

#[test]
fn a_disabled_trigger_publishes_nothing_however_it_is_pressed() {
    let model = ComboBoxModel::new(two_options(), "Pick").disabled();
    let mut harness =
        crate::test_helpers::Harness::<ComboMsg, _>::headless(|| wired_combo(&model).into());

    assert_eq!(
        harness.press(combo_trigger_centre()),
        Vec::new(),
        "a disabled trigger carries no `on_press`"
    );
    assert!(harness.release(combo_trigger_centre()).is_empty());
    assert!(
        harness.hover(combo_trigger_centre()).is_empty(),
        "hovering a disabled control may not publish either"
    );
}

/// The rows are `iced::widget::button`s too, so — exactly as on the trigger —
/// the pick arrives on the release. The press only arms the row, and because
/// the press landed inside the card the overlay's dismiss branch is not taken:
/// a press on a row publishes nothing at all.
#[test]
fn a_press_on_a_row_reports_that_row_and_no_dismiss() {
    let model = ComboBoxModel::new(two_options(), "Pick").with_open(true);
    let mut harness =
        crate::test_helpers::Harness::<ComboMsg, _>::headless(|| wired_combo(&model).into());

    let cands = harness.candidates();
    let trigger = cands[0].bounds;
    let flyout = flyout_of(&cands, trigger);

    assert_eq!(
        harness.press(row_centre(flyout, 1)),
        Vec::new(),
        "the press arms the row and, being inside the card, dismisses nothing"
    );
    assert_eq!(
        harness.release(row_centre(flyout, 1)),
        vec![ComboMsg::Pick("two")],
        "the row under the pointer is the row that gets picked, and the pick \
         is the only thing published"
    );
}

#[test]
fn a_press_in_the_cards_padding_picks_nothing_and_dismisses_nothing() {
    let model = ComboBoxModel::new(two_options(), "Pick").with_open(true);
    let mut harness =
        crate::test_helpers::Harness::<ComboMsg, _>::headless(|| wired_combo(&model).into());

    let cands = harness.candidates();
    let trigger = cands[0].bounds;
    let flyout = flyout_of(&cands, trigger);
    // Inside the card, outside the scrollable: the `space::XS` padding band.
    let padding = Point::new(flyout.x + 2.0, flyout.y + 2.0);

    assert!(flyout.contains(padding), "the fixture is inside the card");
    assert_eq!(
        harness.press(padding),
        Vec::new(),
        "the card's padding is not a hit target, and a click inside the card \
         is not a dismiss"
    );
}

#[test]
fn a_press_away_from_an_open_card_dismisses_exactly_once() {
    let model = ComboBoxModel::new(two_options(), "Pick").with_open(true);
    let mut harness =
        crate::test_helpers::Harness::<ComboMsg, _>::headless(|| wired_combo(&model).into());

    let far = Point::new(
        crate::test_helpers::VIEWPORT.width - 10.0,
        crate::test_helpers::VIEWPORT.height - 10.0,
    );

    assert_eq!(
        harness.press(far),
        vec![ComboMsg::Toggle],
        "a click away from an open combo reports one dismiss and not two: the \
         overlay captures the event, so the trigger underneath never fires"
    );
}

#[test]
fn pressing_the_trigger_of_an_open_card_dismisses_once_rather_than_twice() {
    let model = ComboBoxModel::new(two_options(), "Pick").with_open(true);
    let mut harness =
        crate::test_helpers::Harness::<ComboMsg, _>::headless(|| wired_combo(&model).into());

    assert_eq!(
        harness.press(combo_trigger_centre()),
        vec![ComboMsg::Toggle],
        "the trigger sits outside the card, so the overlay treats it as a \
         dismiss and consumes the event before the button sees it"
    );
}

#[test]
fn escape_dismisses_an_open_card_once_and_a_closed_one_not_at_all() {
    let open_model = ComboBoxModel::new(two_options(), "Pick").with_open(true);
    let mut open =
        crate::test_helpers::Harness::<ComboMsg, _>::headless(|| wired_combo(&open_model).into());

    assert_eq!(
        open.pointer(
            &[named_key(keyboard::key::Named::Escape)],
            mouse::Cursor::Unavailable
        ),
        vec![ComboMsg::Toggle],
        "Escape reports the dismiss the host asked for"
    );

    let shut_model = ComboBoxModel::new(two_options(), "Pick").with_open(false);
    let mut shut =
        crate::test_helpers::Harness::<ComboMsg, _>::headless(|| wired_combo(&shut_model).into());

    assert_eq!(
        shut.pointer(
            &[named_key(keyboard::key::Named::Escape)],
            mouse::Cursor::Unavailable
        ),
        Vec::new(),
        "with no card mounted there is nothing to dismiss"
    );
}

#[test]
fn the_card_flips_above_the_trigger_when_there_is_no_room_below() {
    let model = ComboBoxModel::new(two_options(), "Pick").with_open(true);
    let mut harness = crate::test_helpers::Harness::<ComboMsg, _>::headless(|| {
        iced::widget::column![
            spacer(crate::test_helpers::VIEWPORT.height - 60.0),
            combo_el(&model),
        ]
        .width(Length::Fill)
        .into()
    });

    let cands = harness.candidates();
    // The wrapper column publishes a container of its own before the combo
    // does, so the trigger is found by shape, not by index: it is the first box
    // exactly one standard rung tall, and the base tree is always operated
    // before the overlay.
    let trigger = cands
        .iter()
        .map(|c| c.bounds)
        .find(|b| (b.height - size::CONTROL_MD).abs() < 0.01)
        .expect("the combo must publish its own trigger box");
    assert!(
        trigger.y > 200.0,
        "the fixture must push the trigger to the bottom of the viewport, got {trigger:?}"
    );

    // The card is the record that sits flush on the trigger's top edge, so it
    // is the one whose bottom edge reaches furthest down while staying above
    // it — further than any row inside it.
    let card = cands
        .iter()
        .map(|c| c.bounds)
        .filter(|b| b.y + b.height <= trigger.y)
        .max_by(|a, b| (a.y + a.height).partial_cmp(&(b.y + b.height)).unwrap())
        .expect("with no room below, the card must be laid out above the trigger");

    assert!(
        card.y < trigger.y,
        "the card sat at {card:?}, below the trigger's top edge at {}",
        trigger.y
    );
    assert!(
        (card.y + card.height - trigger.y).abs() < 0.01,
        "a flipped card hangs from the trigger's top edge, not somewhere above \
         it: {card:?} against {trigger:?}"
    );
    assert!(
        card.y + card.height <= crate::test_helpers::VIEWPORT.height,
        "a flipped card must still fit the viewport: {card:?}"
    );
}

/// Hovering a mounted card, then rasterising the tree.
///
/// The hover is the part with teeth: `UserInterface::update` asks the
/// **overlay** for the cursor interaction before it hands the root a cursor,
/// so a hover over a row walks `ComboOverlay::mouse_interaction` and the
/// row's own button interaction. What the tree publishes must not move.
///
/// Note what this leg cannot reach: `UserInterface::draw` only paints an
/// overlay when the *same* interface has already run `update`, and every
/// `Harness` step rebuilds one from the cache — so `ComboOverlay::draw` is
/// only ever walked by the single-pass `Capture` helpers, which need a real
/// GPU session and are out of scope for these tests.
#[test]
fn hovering_a_mounted_card_leaves_what_it_publishes_untouched() {
    let model = ComboBoxModel::new(two_options(), "Pick")
        .with_selected("one")
        .with_open(true);
    let mut harness =
        crate::test_helpers::Harness::<ComboMsg, _>::headless(|| wired_combo(&model).into());

    let cands = harness.candidates();
    let flyout = flyout_of(&cands, cands[0].bounds);

    assert!(
        harness.hover(row_centre(flyout, 0)).is_empty(),
        "hovering a row may not publish"
    );
    harness.render();

    assert_eq!(
        harness.candidates(),
        cands,
        "hovering and painting must not change what the tree publishes"
    );
}

// ---------------------------------------------------------------------------
// 7 — the stateless `Select`
// ---------------------------------------------------------------------------

/// Four options, the smallest list where type-ahead has somewhere to wrap.
fn languages() -> Vec<String> {
    vec![
        String::from("Rust"),
        String::from("Go"),
        String::from("C++"),
        String::from("Python"),
    ]
}

#[test]
fn a_letter_jumps_to_the_option_that_starts_with_it() {
    let options = languages();
    let mut harness = crate::test_helpers::Harness::<PickMsg, _>::headless(|| {
        let none: Option<&String> = None;
        Select::new(&options, none, PickMsg::Chosen)
            .with_width(Length::Fill)
            .into()
    });

    for (letter, expected) in [
        ("g", String::from("Go")),
        ("C", String::from("C++")),
        ("p", String::from("Python")),
    ] {
        assert_eq!(
            harness.pointer(&[typed(letter)], mouse::Cursor::Unavailable),
            vec![PickMsg::Chosen(expected.clone())],
            "typing {letter} must land on {expected:?}"
        );
    }
}

#[test]
fn type_ahead_searches_past_the_current_pick_and_wraps() {
    let options = languages();
    let mut harness = crate::test_helpers::Harness::<PickMsg, _>::headless(|| {
        let picked = Some(&options[0]);
        Select::new(&options, picked, PickMsg::Chosen)
            .with_width(Length::Fill)
            .into()
    });

    // "Rust" is already selected, so the search starts at the *next* row and
    // only comes back to "Rust" after walking the whole list.
    assert_eq!(
        harness.pointer(&[typed("r")], mouse::Cursor::Unavailable),
        vec![PickMsg::Chosen(String::from("Rust"))],
        "the search must wrap rather than refuse to leave the current pick"
    );
}

#[test]
fn a_letter_that_matches_nothing_publishes_nothing() {
    let options = languages();
    let mut harness = crate::test_helpers::Harness::<PickMsg, _>::headless(|| {
        let none: Option<&String> = None;
        Select::new(&options, none, PickMsg::Chosen)
            .with_width(Length::Fill)
            .into()
    });

    assert_eq!(
        harness.pointer(&[typed("z")], mouse::Cursor::Unavailable),
        Vec::new(),
        "no option starts with z"
    );
    assert_eq!(
        harness.pointer(&[typed("#")], mouse::Cursor::Unavailable),
        Vec::new(),
        "a non-alphanumeric character is not a type-ahead at all"
    );
    assert_eq!(
        harness.pointer(
            &[named_key(keyboard::key::Named::Tab)],
            mouse::Cursor::Unavailable
        ),
        Vec::new(),
        "a named key carries no text, so the type-ahead arm cannot fire"
    );
}

// ---------------------------------------------------------------------------
// 7b — the `Select` card mounts: measuring it, picking a row, dismissing it
// ---------------------------------------------------------------------------

// **Defect 01, fixed.** `Select::children()` hands back exactly one slot —
// `Tree::empty()` — and the card is only assembled inside [`Select::overlay`],
// so that slot used to be laid out *as* the card. The flyout is
// `container(scrollable(column))`, and iced's `Container` is tree-transparent:
// it passes the tree straight through to its content, so the scrollable was
// handed a tree with no children of its own and reached for `tree.children[0]`
// at `iced_widget::scrollable.rs:467`. Index out of bounds. And because
// `UserInterface::operate` lays the overlay out on the first read-back, even
// *measuring* an open `Select` took the process down. `overlay` now fills the
// slot with a tree built from the real card before mounting it.
//
// A second fault sat directly behind that one. The overlay dismissed the card on
// a press *inside* it, and an iced button fires on release — so a row's press
// tore the card down before its own release could arrive, and the pick was
// swallowed. The card now falls when its content consumes the release, not when
// it is merely clicked.
//
// Four user-facing capabilities sat behind the two faults, each pinned on its
// own so the fix had to answer for all four: measuring a mounted card, picking a
// row, dismissing with a click away, and type-ahead while open.

/// Open a [`Select`] by pressing its trigger, and hand the harness back with the
/// trigger's own box.
///
/// `Select` toggles its own openness on the press, not on a release, and reports
/// openness through no message of its own — so there is nothing to assert about
/// what the press itself returns.
fn open_a_select(
    options: &[String],
) -> (
    crate::test_helpers::Harness<'_, PickMsg, iced_renderer::Renderer>,
    Rectangle,
) {
    let mut harness = crate::test_helpers::Harness::<PickMsg, _>::headless(|| {
        let none: Option<&String> = None;
        Select::new(options, none, PickMsg::Chosen)
            .with_width(Length::Fill)
            .into()
    });

    let closed = harness.candidates();
    let trigger = closed[0].bounds;
    // `Select` toggles its own openness on the press, not on a release.
    harness.press(trigger.center());
    (harness, trigger)
}

/// The card a full-width [`Select`] hangs below its trigger, computed from the
/// tokens rather than read back off the operation stream.
///
/// `ComboOverlay::layout` puts the card on the trigger's bottom edge; inside it
/// are [`space::XS`] paddings, rows [`size::CONTROL_MD`] tall with
/// [`space::XXS`] between them.
/// `defect_01_an_open_select_mounts_a_card_the_host_can_measure` checks this
/// against the box the mounted card actually publishes, so the helper cannot
/// drift away from the layout it describes.
fn select_card_below(trigger: Rectangle) -> Rectangle {
    let rows = 4.0_f32;
    Rectangle::new(
        Point::new(trigger.x, trigger.y + trigger.height),
        Size::new(
            trigger.width,
            2.0 * space::XS + rows * size::CONTROL_MD + (rows - 1.0) * space::XXS,
        ),
    )
}

/// The option rows a mounted card publishes, in traversal order.
///
/// An iced `button` announces itself through the operation stream's `container`
/// hook, *not* its `focusable` hook — `iced_widget::button::operate` publishes
/// `container(None, bounds)` and never a focus stop, because a button is a hit
/// target rather than a Tab stop. Filtering on `focusable` here would report an
/// empty card no matter how well the card worked.
///
/// Containers are also what `column` and `row` publish, so the card nests four
/// deep and every level reports a box. The rows are picked out by the box the
/// card's own layout gives them: one control tall, and as wide as the card's
/// content area — wider than anything nested *inside* a row, which the button's
/// own horizontal padding shrinks, and taller than the check slot beside the
/// label.
fn option_rows(cands: &[crate::test_helpers::Candidate], card: Rectangle) -> Vec<Rectangle> {
    let row_width = card.width - 2.0 * space::XS;
    cands
        .iter()
        .filter(|c| {
            c.kind == crate::test_helpers::KIND_CONTAINER
                && c.bounds.height == size::CONTROL_MD
                && c.bounds.width == row_width
                && c.bounds.y >= card.y
        })
        .map(|c| c.bounds)
        .collect()
}

/// A shut [`Select`] publishes its own box and no message of its own — the half
/// of the interaction that still works.
#[test]
fn a_shut_select_publishes_its_box_and_no_message() {
    let options = languages();
    let mut harness = crate::test_helpers::Harness::<PickMsg, _>::headless(|| {
        let none: Option<&String> = None;
        Select::new(&options, none, PickMsg::Chosen)
            .with_width(Length::Fill)
            .into()
    });

    let closed = harness.candidates();
    assert_eq!(closed.len(), 1, "a shut Select publishes its box only");
    assert_eq!(
        harness.press(closed[0].bounds.center()),
        Vec::new(),
        "a Select reports openness through no message of its own"
    );
}

/// **Defect 01, flipped.** Opening the trigger mounts a card and a host can
/// measure it: the card publishes its own box below the trigger and every option
/// publishes a row inside it. Before the fix this read-back panicked inside
/// iced's scrollable, because the card had no widget tree to lay out against.
#[test]
fn defect_01_an_open_select_mounts_a_card_the_host_can_measure() {
    let options = languages();
    let (mut harness, trigger) = open_a_select(&options);

    let open = harness.candidates();
    assert!(
        open.len() > 1,
        "defect 01: an open Select publishes the card as well as its trigger"
    );

    let card = flyout_of(&open, trigger);
    assert_eq!(
        card,
        select_card_below(trigger),
        "defect 01: the mounted card sits exactly where the overlay's geometry puts it"
    );

    let rows = option_rows(&open, card);
    assert_eq!(
        rows.len(),
        options.len(),
        "defect 01: every option publishes its own row box"
    );
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(
            row.center(),
            row_centre(card, index),
            "defect 01: row {index} sits exactly where the press fixture aims it"
        );
        assert!(
            card.contains(row.center()),
            "defect 01: row {index} sits inside the card that hosts it"
        );
    }
}

/// **Defect 01, flipped.** Pressing and releasing an option row publishes that
/// option's message and ends the interaction.
///
/// Before the fix the press panicked before the row could be reached. Behind that
/// panic sat a second fault: the overlay used to dismiss the card on a press
/// *inside* it, which would have swallowed the pick even once the panic was gone,
/// because an iced button answers on release and the card would already have been
/// gone by then.
#[test]
fn defect_01_a_row_press_publishes_that_options_message() {
    let options = languages();
    let (mut harness, trigger) = open_a_select(&options);
    let card = flyout_of(&harness.candidates(), trigger);
    let row = row_centre(card, 2);
    assert!(card.contains(row), "the fixture must press inside the card");

    assert_eq!(
        harness.press(row),
        Vec::new(),
        "defect 01: the press arms the row — an iced button answers on release"
    );
    assert_eq!(
        harness.release(row),
        vec![PickMsg::Chosen(String::from("C++"))],
        "defect 01: releasing the third row publishes that option"
    );
    assert_eq!(
        harness.candidates().len(),
        1,
        "defect 01: picking an option dismisses the card"
    );
}

/// **Defect 01, flipped.** A click away from the card shuts it and publishes
/// nothing, because a `Select` owns its own openness and has no dismiss message
/// to report it through.
#[test]
fn defect_01_a_press_away_shuts_an_open_select_and_publishes_nothing() {
    let options = languages();
    let (mut harness, trigger) = open_a_select(&options);
    let card = flyout_of(&harness.candidates(), trigger);
    let far = Point::new(card.x + 5.0, crate::test_helpers::VIEWPORT.height - 10.0);
    assert!(
        !card.contains(far),
        "the fixture must land outside the card"
    );

    assert_eq!(
        harness.press(far),
        Vec::new(),
        "defect 01: a click away publishes nothing — a Select owns its own openness"
    );
    assert_eq!(
        harness.candidates().len(),
        1,
        "defect 01: the click away did shut the card"
    );
}

/// **Defect 01, flipped.** With the card open, a letter belongs to whatever the
/// host has focused, not to the closed-select prefix type-ahead — so the tree
/// publishes nothing.
///
/// The same letter on a shut control *is* a type-ahead, which is what makes the
/// first assertion a guard rather than a tautology: it proves the silence comes
/// from the card being open, not from type-ahead being broken.
#[test]
fn defect_01_a_keystroke_while_the_card_is_open_is_not_type_ahead() {
    let options = languages();
    let (mut harness, trigger) = open_a_select(&options);

    let open = harness.candidates();
    let card = flyout_of(&open, trigger);
    assert_eq!(
        option_rows(&open, card).len(),
        options.len(),
        "the fixture must have a mounted card to type over"
    );

    assert_eq!(
        harness.pointer(&[typed("g")], mouse::Cursor::Unavailable),
        Vec::new(),
        "defect 01: the open card takes the letter instead of type-ahead picking"
    );

    // A press on the trigger dismisses the card, and the very same keystroke is
    // then the closed-control type-ahead.
    assert_eq!(
        harness.press(trigger.center()),
        Vec::new(),
        "a press on the trigger shuts the card and publishes nothing"
    );
    assert_eq!(
        harness.candidates().len(),
        1,
        "the trigger press did shut the card"
    );
    assert_eq!(
        harness.pointer(&[typed("g")], mouse::Cursor::Unavailable),
        vec![PickMsg::Chosen(String::from("Go"))],
        "defect 01: shut, the same letter is the prefix type-ahead"
    );
}

// The four capabilities defect 01 locked behind a panic are pinned one apiece:
// measuring the mounted card by
// `defect_01_an_open_select_mounts_a_card_the_host_can_measure`, picking a row by
// `defect_01_a_row_press_publishes_that_options_message`, dismissing with a click
// away by `defect_01_a_press_away_shuts_an_open_select_and_publishes_nothing`, and
// type-ahead while open by
// `defect_01_a_keystroke_while_the_card_is_open_is_not_type_ahead`. The shut-state
// half of the first claim lives in `a_shut_select_publishes_its_box_and_no_message`.

#[test]
fn a_select_with_no_options_never_mounts_a_card() {
    let options: Vec<String> = Vec::new();
    let mut harness = crate::test_helpers::Harness::<PickMsg, _>::headless(|| {
        let none: Option<&String> = None;
        Select::new(&options, none, PickMsg::Chosen)
            .with_width(Length::Fill)
            .into()
    });

    let before = harness.candidates();
    assert_eq!(before.len(), 1);

    harness.press(before[0].bounds.center());
    harness.release(before[0].bounds.center());

    assert_eq!(
        harness.candidates().len(),
        1,
        "an empty option list has nothing to show, so no card may mount"
    );
}

/// What the builder setters leave behind, read back through layout.
///
/// **Why this test does not read the fields.** `Select` keeps `palette`,
/// `width` and `rung` in private fields and — unlike [`ComboBox`], which
/// answers `palette()`, `width()` and `rung()` — publishes **no getters** for
/// any of them. So the claims are pinned through the only door that is open:
/// the box the control publishes on the operation stream, which `layout`
/// resolves from exactly those fields.
///
/// * **Width** is observable: `with_width` clamps a `Fixed` request to the
///   rung's minimum, and the published container is that wide.
/// * **Rung** is observable through the published height.
/// * **Palette** and **placeholder** are *not* observable here. `Select` is
///   implemented for `iced::Renderer` only, and the recording harness captures
///   quads and no glyphs, so neither the tint nor the placeholder text can be
///   read back. The setters are still called so a regression in them fails to
///   compile or panics rather than slipping past untouched.
#[test]
fn a_select_clamps_the_width_it_was_given_to_its_rungs_minimum() {
    let options = languages();

    let mut harness = crate::test_helpers::Harness::<PickMsg, _>::headless(|| {
        let none: Option<&String> = None;
        Select::new(&options, none, PickMsg::Chosen)
            .with_placeholder("Pick a language")
            .with_palette(alt_palette())
            .with_width(Length::Fixed(96.0))
            .into()
    });

    let published = harness.candidates();
    assert_eq!(published[0].kind, crate::test_helpers::KIND_CONTAINER);
    assert_eq!(
        published[0].bounds.width, MIN_WIDTH,
        "a standard-rung select clamps to MIN_WIDTH, not to the 96 asked for"
    );
    assert_eq!(
        published[0].bounds.height,
        size::CONTROL_MD,
        "the default rung is the standard one, so the box is CONTROL_MD tall"
    );
}

/// The compact rung's floor is its own, not the standard one — the clamp in
/// [`Select::with_width`] reads the rung, and the published box proves which
/// branch it took.
#[test]
fn a_compact_select_clamps_to_the_compact_minimum_instead() {
    let options = languages();

    let mut harness = crate::test_helpers::Harness::<PickMsg, _>::headless(|| {
        let none: Option<&String> = None;
        Select::new(&options, none, PickMsg::Chosen)
            .with_rung(ComboRung::Compact)
            .with_width(Length::Fixed(24.0))
            .into()
    });

    let published = harness.candidates();
    assert_eq!(published[0].kind, crate::test_helpers::KIND_CONTAINER);
    assert_eq!(
        published[0].bounds.width, MIN_WIDTH_COMPACT,
        "a compact-rung select clamps to MIN_WIDTH_COMPACT, not to the 24 asked for"
    );
    assert_eq!(published[0].bounds.height, size::CONTROL_SM);
}

#[test]
fn a_select_is_painted_through_the_real_renderer() {
    let options = languages();
    let mut harness = crate::test_helpers::Harness::<PickMsg, _>::headless(|| {
        let picked = Some(&options[1]);
        Select::new(&options, picked, PickMsg::Chosen)
            .with_width(Length::Fill)
            .into()
    });

    let published = harness.candidates();
    let trigger = published[0].bounds;

    // Hover first so the painted state is `Hovered` rather than `Rest`, then
    // raster: this is the leg that walks `Select::draw`'s quad and both glyphs.
    harness.hover(trigger.center());
    harness.render();

    let after = harness.candidates();
    assert_eq!(after.len(), published.len());
    assert_eq!(
        after[0].bounds, trigger,
        "the box a host reads is the box that was painted"
    );
}
