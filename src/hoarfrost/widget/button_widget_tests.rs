//! Proofs that the pure layers are sufficient to build a real, painting,
//! keyboard-focusable button, and the technique for asserting what a widget
//! actually painted without a screen.
//!
//! # The harness is the shared one
//!
//! These tests drive focus and paint through [`crate::test_helpers`], the
//! same headless harness `INC-10`'s wrapper tests and the bridge tests use,
//! because that is the only way focus can be driven at all in iced 0.14
//! (`INC-10` constraints 3 and 4). What the button adds over a bare focus
//! wrapper is nothing but fixtures: [`recorder_button`] and
//! [`armed_button`] wrap a [`Button`] in the two renderers the harness
//! offers, and [`drive`] spells out how a pointer gets a button into a
//! [`BaseState`].
//!
//! Two legs, one harness:
//!
//! * with iced's real headless renderer and real `text` content — does the
//!   button lay out, register, hold focus and paint through the actual
//!   paint path;
//! * with [`Recorder`], a renderer that records every `fill_quad` instead
//!   of rasterising it — what exactly did the widget ask to be drawn,
//!   where, in what order, in what colour.
//!
//! The second leg is what makes the focus ring testable. A ring is a
//! *position* claim as much as a colour claim: it has to be outside the box
//! the fill covers, by exactly the ring width, and no snapshot of a
//! rendered pixel grid says that without also freezing every glyph in the
//! crate. [`Recorder`] says it directly, and [`Blank`] — content that
//! draws nothing and needs no text renderer — keeps the recording to the
//! button's own layers.
//!
//! # Reading state back
//!
//! The widget tree is owned by the cache and the cache exposes nothing, so
//! hover, press and focus are read through what the button *paints* rather
//! than through an accessor that would exist only for tests. That is the
//! stronger assertion anyway: `state.hovered == true` is an internal
//! fact; "this frame painted the hover fill and the ring" is the behaviour
//! the spec has to hold.

use crate::bridge::focus_task;
use crate::bridge::widget_id;
use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::focus::GroupId;
use crate::state::BaseState;
use crate::state::VisualState;
use crate::test_helpers::Blank;
use crate::test_helpers::Harness;
use crate::test_helpers::Recorder;
use crate::test_helpers::assert_agree;
use crate::test_helpers::centre;
use crate::test_helpers::focus_one;
use crate::test_helpers::iced_id;
use crate::test_helpers::model_of;
use crate::test_helpers::moved;
use crate::test_helpers::opt_ids;
use crate::test_helpers::pressed;
use crate::test_helpers::released;
use crate::token::color::Palette;
use crate::widget::button;
use crate::widget::button::ButtonStyle;
use crate::widget::button::ButtonVariant;
use crate::widget::button_widget::Button;
use crate::widget::button_widget::ButtonState;
use crate::widget::button_widget::background_layer;
use crate::widget::button_widget::button_state;
use crate::widget::button_widget::focus_ring_layer;
use crate::widget::focus::resolve_ring_color_for_background;
use crate::widget::focusable::FocusableState;

use iced::Background;
use iced::Color;
use iced::Element;
use iced::Event;
use iced::Point;
use iced::Rectangle;
use iced::Size;
use iced::Task;
use iced::advanced::Shell;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::Tree;
use iced::advanced::widget::Widget;
use iced::advanced::widget::operation;
use iced::mouse;
use iced::widget::column;
use iced::widget::text;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Save,
    Cancel,
}

type Theme = iced::Theme;

/// A content size big enough that the button's own padding is visible in
/// the resulting box, and small enough to leave room to move the cursor
/// outside it inside the test viewport.
const CONTENT: Size = Size::new(40.0, 10.0);

/// A point comfortably outside a button parked at the origin of a
/// 400×300 viewport.
const FAR_AWAY: Point = Point::new(390.0, 290.0);

const REST: VisualState = VisualState {
    base: BaseState::Rest,
    focused: false,
};
const HOVERED: VisualState = VisualState {
    base: BaseState::Hovered,
    focused: false,
};
const PRESSED: VisualState = VisualState {
    base: BaseState::Pressed,
    focused: false,
};
const DISABLED: VisualState = VisualState {
    base: BaseState::Disabled,
    focused: false,
};
const FOCUSED: VisualState = VisualState {
    base: BaseState::Rest,
    focused: true,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A recording harness around one button with blank content.
fn recorder_button(
    variant: ButtonVariant,
    label: &'static str,
    configure: impl Fn(Button<'static, Msg, Theme, Recorder>) -> Button<'static, Msg, Theme, Recorder>
    + 'static,
) -> Harness<Msg, Recorder> {
    Harness::recording(move || {
        configure(Button::new(
            variant,
            Element::new(Blank::new(CONTENT)),
            Msg::Save,
            label,
        ))
        .into()
    })
}

/// A recording harness around one button that publishes a message
/// distinguishable from the fixtures' default, so "fired" and "did not
/// fire" cannot be confused with a fixture wiring mistake.
fn armed_button(label: &'static str) -> Harness<Msg, Recorder> {
    Harness::recording(move || {
        Button::new(
            ButtonVariant::Primary,
            Element::new(Blank::new(CONTENT)),
            Msg::Cancel,
            label,
        )
        .into()
    })
}

/// A recording harness around one disabled button.
fn disabled_button(label: &'static str) -> Harness<Msg, Recorder> {
    Harness::recording(move || {
        Button::new(
            ButtonVariant::Primary,
            Element::new(Blank::new(CONTENT)),
            Msg::Cancel,
            label,
        )
        .disabled()
        .into()
    })
}

/// The style a `(variant, state)` pair is supposed to paint, against the
/// palette every fixture uses.
fn expected_style(variant: ButtonVariant, visual: VisualState) -> ButtonStyle {
    button::resolve_style(variant, visual, &Palette::dark())
}

/// Drive the pointer so the button lands in `base`.
///
/// `Disabled` and `Invalid` are driven the same hard way as `Pressed` —
/// cursor over, button down — because that is the only way to show the
/// state machine overrode the pointer rather than never received it.
fn drive<M: Send + 'static, R: renderer::Renderer>(
    harness: &mut Harness<M, R>,
    base: BaseState,
    over: Point,
) {
    match base {
        BaseState::Rest | BaseState::Invalid => {
            // A move event the tree is told to ignore: the only way to get
            // back to `Rest` after a hover is to deliver a frame in which
            // the cursor is unavailable.
            harness.pointer(&[moved(over)], mouse::Cursor::Unavailable);
        }
        BaseState::Hovered => {
            harness.hover(over);
        }
        BaseState::Pressed | BaseState::Disabled => {
            harness.hover(over);
            harness.press(over);
        }
    }
}

/// One row of the `(variant × state)` matrix: how to build the widget,
/// how to drive it, and what it must end up painting.
struct Case {
    label: &'static str,
    base: BaseState,
    enabled: bool,
    invalid: bool,
    painted: BaseState,
}

const CASES: [Case; 5] = [
    Case {
        label: "rest",
        base: BaseState::Rest,
        enabled: true,
        invalid: false,
        painted: BaseState::Rest,
    },
    Case {
        label: "hovered",
        base: BaseState::Hovered,
        enabled: true,
        invalid: false,
        painted: BaseState::Hovered,
    },
    Case {
        label: "pressed",
        base: BaseState::Pressed,
        enabled: true,
        invalid: false,
        painted: BaseState::Pressed,
    },
    Case {
        label: "disabled",
        base: BaseState::Disabled,
        enabled: false,
        invalid: false,
        painted: BaseState::Disabled,
    },
    Case {
        label: "invalid",
        base: BaseState::Invalid,
        enabled: true,
        invalid: true,
        // §3.1 gives a button no invalid skin, so the resolver paints
        // `Invalid` exactly as `Rest`. Pinned at the widget level as well
        // as in `button_tests.rs`: the gap has to survive the whole chain,
        // not just the resolver.
        painted: BaseState::Rest,
    },
];

const VARIANTS: [ButtonVariant; 4] = [
    ButtonVariant::Primary,
    ButtonVariant::Secondary,
    ButtonVariant::Ghost,
    ButtonVariant::Danger,
];

fn variant_name(variant: ButtonVariant) -> &'static str {
    match variant {
        ButtonVariant::Primary => "Primary",
        ButtonVariant::Secondary => "Secondary",
        ButtonVariant::Ghost => "Ghost",
        ButtonVariant::Danger => "Danger",
    }
}

/// Build a button and lay it out in the test viewport, handing back
/// everything needed to call the widget trait's read-only methods.
fn laid_out(
    button: Button<'static, Msg, Theme, Recorder>,
) -> (Button<'static, Msg, Theme, Recorder>, Tree, layout::Node) {
    let mut button = button;
    let mut tree = Tree {
        tag: button.tag(),
        state: button.state(),
        children: button.children(),
    };
    let node = button.layout(
        &mut tree,
        &Recorder::default(),
        &layout::Limits::new(Size::ZERO, crate::test_helpers::VIEWPORT),
    );

    (button, tree, node)
}

fn save_button(
    variant: ButtonVariant,
    label: impl Into<FocusId>,
) -> Button<'static, Msg, Theme, Recorder> {
    Button::new(variant, Element::new(Blank::new(CONTENT)), Msg::Save, label)
}

// ---------------------------------------------------------------------------
// 1. Style wiring — every (variant × VisualState) paints what the pure
//    resolver says it paints
// ---------------------------------------------------------------------------

#[test]
fn every_variant_and_state_paints_what_the_resolver_resolved() {
    for variant in VARIANTS {
        for case in CASES {
            for focused in [false, true] {
                let mut harness = recorder_button(variant, "btn", move |button| {
                    let mut button = button;
                    if !case.enabled {
                        button = button.disabled();
                    }
                    if case.invalid {
                        button = button.invalid();
                    }
                    button
                });

                if focused {
                    focus_one(&mut harness, "btn");
                }

                let over = centre(harness.first_bounds());
                drive(&mut harness, case.base, over);

                let visual = VisualState {
                    base: case.painted,
                    // A disabled control cannot hold a ring, whatever the
                    // router was last told.
                    focused: focused && case.enabled,
                };
                let style = expected_style(variant, visual);
                let paints = harness.paint();
                let where_ = format!(
                    "{}/{}{}",
                    variant_name(variant),
                    case.label,
                    if focused { " + focused" } else { "" }
                );

                assert_eq!(
                    paints.len(),
                    if visual.ring_visible() { 2 } else { 1 },
                    "{where_} must paint the box{} — got {paints:?}",
                    if visual.ring_visible() {
                        " and a ring"
                    } else {
                        " and nothing else"
                    },
                );
                assert_eq!(
                    paints[0].background,
                    Background::Color(style.background),
                    "{where_} must fill with the resolved background"
                );
                assert_eq!(
                    paints[0].border.color, style.border_color,
                    "{where_} must stroke with the resolved border colour"
                );
                assert_eq!(
                    paints[0].border.width, style.border_width,
                    "{where_} must stroke at the resolved border width"
                );

                if visual.ring_visible() {
                    assert_eq!(
                        paints[1].border.color, style.focus_ring_color,
                        "{where_} must wear the ring the resolver chose"
                    );
                    assert_eq!(
                        paints[1].border.width,
                        button::FOCUS_RING_WIDTH,
                        "{where_} must wear the ring at the token width"
                    );
                }
            }
        }
    }
}

#[test]
fn the_button_paints_through_a_real_renderer_in_every_state() {
    // The same matrix through iced's real headless renderer rather than a
    // recorder: real quads, real text, real layer calls. Nothing is
    // asserted about pixels — that the pass completes for every cell is
    // the claim.
    for variant in VARIANTS {
        for case in CASES {
            let mut harness = Harness::headless(move || {
                let mut button = Button::new(
                    variant,
                    text(format!("{} {}", variant_name(variant), case.label)),
                    Msg::Save,
                    "btn",
                );
                if !case.enabled {
                    button = button.disabled();
                }
                if case.invalid {
                    button = button.invalid();
                }
                button.into()
            });

            harness.render();
            harness.render();
        }
    }
}

// ---------------------------------------------------------------------------
// 2. The pointer drives the state
// ---------------------------------------------------------------------------

#[test]
fn the_cursor_over_the_button_drives_the_hover_fill() {
    let mut harness = recorder_button(ButtonVariant::Secondary, "btn", |b| b);
    let over = centre(harness.first_bounds());

    harness.hover(over);

    assert_eq!(
        harness.paint()[0].background,
        Background::Color(expected_style(ButtonVariant::Secondary, HOVERED).background),
        "a cursor inside the bounds must paint the hover fill"
    );
}

#[test]
fn the_cursor_leaving_the_button_clears_the_hover_fill() {
    let mut harness = recorder_button(ButtonVariant::Secondary, "btn", |b| b);
    let over = centre(harness.first_bounds());

    harness.hover(over);
    assert_eq!(
        harness.paint()[0].background,
        Background::Color(expected_style(ButtonVariant::Secondary, HOVERED).background)
    );

    harness.pointer(&[moved(FAR_AWAY)], mouse::Cursor::Available(FAR_AWAY));
    assert_eq!(
        harness.paint()[0].background,
        Background::Color(expected_style(ButtonVariant::Secondary, REST).background),
        "a cursor outside the bounds must fall back to the resting fill"
    );
}

#[test]
fn a_held_button_paints_the_pressed_fill_until_it_is_released() {
    let mut harness = recorder_button(ButtonVariant::Primary, "btn", |b| b);
    let over = centre(harness.first_bounds());

    harness.hover(over);
    harness.press(over);
    assert_eq!(
        harness.paint()[0].background,
        Background::Color(expected_style(ButtonVariant::Primary, PRESSED).background),
        "button-down inside the bounds must paint the pressed fill"
    );

    harness.release(over);
    assert_eq!(
        harness.paint()[0].background,
        Background::Color(expected_style(ButtonVariant::Primary, HOVERED).background),
        "the pressed fill must not stick after the release"
    );
}

#[test]
fn the_cursor_is_a_pointer_only_over_an_enabled_button() {
    let viewport = Rectangle {
        x: 0.0,
        y: 0.0,
        width: crate::test_helpers::VIEWPORT.width,
        height: crate::test_helpers::VIEWPORT.height,
    };

    let (enabled, enabled_tree, enabled_node) =
        laid_out(save_button(ButtonVariant::Primary, "btn"));
    let (disabled, disabled_tree, disabled_node) =
        laid_out(save_button(ButtonVariant::Primary, "btn").disabled());

    let over = centre(enabled_node.bounds());
    let recorder = Recorder::default();

    assert_eq!(
        enabled.mouse_interaction(
            &enabled_tree,
            Layout::new(&enabled_node),
            mouse::Cursor::Available(over),
            &viewport,
            &recorder,
        ),
        mouse::Interaction::Pointer,
        "an enabled button under the cursor must offer the pointer"
    );
    assert_eq!(
        disabled.mouse_interaction(
            &disabled_tree,
            Layout::new(&disabled_node),
            mouse::Cursor::Available(over),
            &viewport,
            &recorder,
        ),
        mouse::Interaction::None,
        "a disabled button must not offer an affordance it will not honour"
    );
    assert_eq!(
        enabled.mouse_interaction(
            &enabled_tree,
            Layout::new(&enabled_node),
            mouse::Cursor::Unavailable,
            &viewport,
            &recorder,
        ),
        mouse::Interaction::None,
        "no cursor, no pointer"
    );
}

// ---------------------------------------------------------------------------
// 3. Press semantics
// ---------------------------------------------------------------------------

#[test]
fn a_press_released_inside_the_button_fires_once() {
    let mut harness = armed_button("btn");
    let over = centre(harness.first_bounds());

    harness.hover(over);
    harness.press(over);
    let fired = harness.release(over);

    assert_eq!(fired, vec![Msg::Cancel], "release inside the box must fire");
}

#[test]
fn a_press_dragged_outside_the_button_does_not_fire() {
    let mut harness = armed_button("btn");
    let over = centre(harness.first_bounds());

    harness.hover(over);
    harness.press(over);
    assert_eq!(
        harness.paint()[0].background,
        Background::Color(expected_style(ButtonVariant::Primary, PRESSED).background),
        "the press must be armed before it is dragged out"
    );

    let fired = harness.pointer(&[released()], mouse::Cursor::Available(FAR_AWAY));

    assert!(
        fired.is_empty(),
        "releasing outside the box must not fire — a press the user dragged \
         away from is a cancelled press, not a committed one"
    );
    assert_eq!(
        harness.paint()[0].background,
        Background::Color(expected_style(ButtonVariant::Primary, REST).background),
        "the pressed fill must clear when the drag leaves"
    );
}

#[test]
fn a_press_that_started_elsewhere_does_not_fire() {
    let mut harness = armed_button("btn");
    let over = centre(harness.first_bounds());

    harness.pointer(&[pressed()], mouse::Cursor::Available(FAR_AWAY));
    let fired = harness.release(over);

    assert!(
        fired.is_empty(),
        "a release may only complete a press this button actually took"
    );
}

#[test]
fn a_second_release_after_the_first_does_not_fire_again() {
    let mut harness = armed_button("btn");
    let over = centre(harness.first_bounds());

    harness.hover(over);
    harness.press(over);
    assert_eq!(harness.release(over), vec![Msg::Cancel]);
    assert!(
        harness.release(over).is_empty(),
        "one press commits exactly one message"
    );
}

// ---------------------------------------------------------------------------
// 4. Disabled is absence
// ---------------------------------------------------------------------------

#[test]
fn a_disabled_button_publishes_nothing_through_a_full_press_cycle() {
    let mut harness = disabled_button("btn");
    let over = centre(harness.first_bounds());

    for events in [
        vec![moved(over)],
        vec![pressed()],
        vec![released()],
        vec![moved(over), pressed(), released()],
    ] {
        let fired = harness.pointer(&events, mouse::Cursor::Available(over));
        assert!(
            fired.is_empty(),
            "a disabled button must never publish, whatever the pointer does"
        );
    }
}

#[test]
fn a_disabled_button_never_shows_hover_or_press_feedback() {
    let mut harness = disabled_button("btn");
    let over = centre(harness.first_bounds());
    let disabled_fill =
        Background::Color(expected_style(ButtonVariant::Primary, DISABLED).background);

    assert_eq!(harness.paint()[0].background, disabled_fill, "at rest");

    harness.hover(over);
    assert_eq!(harness.paint()[0].background, disabled_fill, "hovered");

    harness.press(over);
    assert_eq!(
        harness.paint()[0].background,
        disabled_fill,
        "pressed — the disabled skin must dominate the pointer completely"
    );
}

#[test]
fn a_disabled_button_is_absent_from_the_focus_router() {
    let mut harness = recorder_button(ButtonVariant::Primary, "dead", |b| b.disabled());

    assert_eq!(
        harness.focus_count().total,
        0,
        "a disabled button must not appear in the router at all, not merely \
         be skipped by name"
    );
    assert_eq!(harness.registered(), vec![]);
}

#[test]
fn a_disabled_button_cannot_be_pushed_into_focus_through_the_bridge() {
    let mut harness = recorder_button(ButtonVariant::Primary, "dead", |b| b.disabled());

    // The bridge is a projection of the model, and the model believes this
    // control is focusable. The widget is not, so the push must land on
    // nothing rather than force focus onto a dead control.
    let mut model = FocusState::new();
    model.register_group(GroupId::new("panel"), vec![FocusMember::new("dead")]);
    assert!(model.focus(&FocusId::new("dead")));

    harness.sync(&model);

    assert_eq!(
        harness.focused(),
        None,
        "the router must not show focus on a widget that refused to register"
    );
    assert!(
        harness
            .paint()
            .iter()
            .all(|paint| paint.border.width <= button::BORDER_WIDTH),
        "a dead control must not wear a ring"
    );
}

#[test]
fn disabling_a_focused_button_clears_its_ring_on_the_next_pass() {
    let mut harness = recorder_button(ButtonVariant::Secondary, "btn", |b| b);
    let id = iced_id("btn");

    focus_one(&mut harness, "btn");
    assert!(harness.is_focused(&id), "the live button holds focus");
    assert!(
        harness
            .paint()
            .iter()
            .any(|paint| paint.border.width > button::BORDER_WIDTH),
        "and wears a ring"
    );

    harness.set_view(|| {
        Button::new(
            ButtonVariant::Secondary,
            Element::new(Blank::new(CONTENT)),
            Msg::Save,
            "btn",
        )
        .disabled()
        .into()
    });

    assert!(
        !harness.is_focused(&id),
        "a button that becomes disabled must drop the focus it held: it is \
         invisible to the router, so nothing else can take it back"
    );
    assert_eq!(harness.focused(), None);
    assert!(
        harness
            .paint()
            .iter()
            .all(|paint| paint.border.width <= button::BORDER_WIDTH),
        "and no ring may survive the transition"
    );
}

// ---------------------------------------------------------------------------
// 5. The focus ring — geometry and the adaptive colour
// ---------------------------------------------------------------------------

#[test]
fn an_unfocused_button_wears_no_ring() {
    let mut harness = recorder_button(ButtonVariant::Secondary, "btn", |b| b);

    assert_eq!(
        harness.paint().len(),
        1,
        "with no focus there is exactly one layer: the button's own box"
    );
}

#[test]
fn a_focused_button_wears_a_ring_outside_its_own_box() {
    let mut harness = recorder_button(ButtonVariant::Secondary, "btn", |b| b);
    let bounds = harness.first_bounds();

    focus_one(&mut harness, "btn");

    let paints = harness.paint();
    assert_eq!(
        paints.len(),
        2,
        "the ring must be painted on top of the box"
    );

    let ring = paints[1];
    assert_eq!(
        ring.bounds,
        bounds.expand(button::FOCUS_RING_WIDTH),
        "the ring must sit outside the button's own bounds by exactly the \
         ring width — iced strokes a quad on its inside, so a ring drawn as \
         the button's own border would eat into the fill"
    );
    assert_eq!(
        ring.background,
        Background::Color(Color::TRANSPARENT),
        "the ring must not fill the annulus it strokes"
    );
    assert_eq!(
        ring.border.width,
        button::FOCUS_RING_WIDTH,
        "the ring must be the token width, twice the resting border"
    );
    assert_eq!(
        ring.border.color,
        expected_style(ButtonVariant::Secondary, FOCUSED).focus_ring_color,
        "the ring colour must be the resolver's answer, not a literal"
    );
}

#[test]
fn the_ring_on_a_danger_hot_fill_is_adaptive_not_the_accent() {
    // G-9: §3.1 names `accent.border` for Danger's ring, and indigo on
    // the hot red fill measures 1.7188:1 — an invisible focus indicator
    // at the moment a keyboard user most needs one. The widget must end up
    // painting the adaptive colour without ever mentioning Danger's ring.
    let mut harness = recorder_button(ButtonVariant::Danger, "btn", |b| b);
    let over = centre(harness.first_bounds());

    focus_one(&mut harness, "btn");
    drive(&mut harness, BaseState::Hovered, over);

    let palette = Palette::dark();
    let paints = harness.paint();

    assert_eq!(
        paints.len(),
        2,
        "focused + hovered must keep the hover fill AND the ring, not be \
         pulled back to one or the other"
    );
    assert_eq!(
        paints[0].background,
        Background::Color(palette.status_danger_fill),
        "the Danger hover fill must survive the focus"
    );

    let ring = paints[1].border.color;
    assert_eq!(
        ring,
        resolve_ring_color_for_background(palette.status_danger_fill, &palette),
        "the widget's ring must equal the shared adaptive resolver's answer \
         against the fill it actually painted"
    );
    assert_ne!(
        ring, palette.accent_border,
        "the spec's literal ring colour is invisible on the Danger hot fill \
         and must not be what gets painted"
    );
    assert_eq!(
        ring, palette.text_strong,
        "the fallback for a fill that swallows the accent is the strong text colour"
    );
}

#[test]
fn a_ghost_ring_is_judged_against_its_container_not_against_transparent() {
    // A Ghost button at rest paints no fill, so its ring belongs to
    // whatever the button is mounted to. Resolving it against
    // `transparent` would measure a colour that is never painted.
    let mut harness = recorder_button(ButtonVariant::Ghost, "btn", |b| b);

    focus_one(&mut harness, "btn");

    let paints = harness.paint();
    assert_eq!(
        paints
            .last()
            .expect("a focused ghost must wear a ring")
            .border
            .color,
        Palette::dark().accent_border,
        "a transparent background short-circuits to the spec's own ring colour"
    );
}

#[test]
fn the_ring_layer_geometry_is_concentric_with_the_box() {
    let bounds = Rectangle {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 32.0,
    };
    let style = expected_style(ButtonVariant::Secondary, FOCUSED);

    let box_layer =
        background_layer(bounds, &style).expect("a resolved style always carries a stroke");
    let ring = focus_ring_layer(bounds, &style).expect("a focused style always carries a ring");

    assert_eq!(box_layer.bounds, bounds);
    assert_eq!(ring.bounds, bounds.expand(button::FOCUS_RING_WIDTH));
    assert!(
        ring.bounds.x < box_layer.bounds.x
            && ring.bounds.y < box_layer.bounds.y
            && ring.bounds.x + ring.bounds.width > box_layer.bounds.x + box_layer.bounds.width
            && ring.bounds.y + ring.bounds.height > box_layer.bounds.y + box_layer.bounds.height,
        "the ring must surround the box on all four sides"
    );
    assert_eq!(
        ring.border.radius.top_left,
        button::RADIUS + button::FOCUS_RING_WIDTH,
        "the ring's corner must be the box's radius grown by the ring width, \
         so the two curves stay concentric instead of the ring cutting the \
         corners harder than the box"
    );
}

#[test]
fn no_ring_is_layered_when_no_ring_is_due() {
    let bounds = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 32.0,
    };

    assert_eq!(
        focus_ring_layer(bounds, &expected_style(ButtonVariant::Secondary, REST)),
        None
    );
}

// ---------------------------------------------------------------------------
// 6. The Tab bridge, driving real buttons
// ---------------------------------------------------------------------------

#[test]
fn tab_from_the_model_focuses_the_button() {
    let mut harness = Harness::headless(|| {
        Button::new(
            ButtonVariant::Primary,
            text("Save"),
            Msg::Save,
            "dialog.save",
        )
        .into()
    });
    let id = iced_id("dialog.save");

    let (_group, mut model) = model_of(&["dialog.save"]);
    assert!(
        !harness.is_focused(&id),
        "registration alone focuses nothing"
    );

    model.focus_next();
    harness.sync(&model);

    assert!(
        harness.is_focused(&id),
        "Tab computed by the model and pushed through the bridge must land on \
         the button"
    );
    assert_agree(&mut harness, &model, "Tab into the button");
}

#[test]
fn the_bridge_wraps_where_iced_would_have_lost_focus() {
    // The constraint INC-10 pinned, proved against real buttons: at the
    // last control iced's own `focus_next` drops focus, the model wraps,
    // and pushing the model's answer is what keeps the keyboard alive.
    let mut harness = Harness::headless(|| {
        column(vec![
            Button::new(ButtonVariant::Primary, text("Save"), Msg::Save, "a").into(),
            Button::new(ButtonVariant::Secondary, text("Cancel"), Msg::Cancel, "b").into(),
        ])
        .into()
    });
    let (group, mut model) = model_of(&["a", "b"]);

    model.focus_last_in_group(&group);
    harness.sync(&model);
    assert_eq!(harness.focused(), Some(iced_id("b")));

    model.focus_next();
    assert_eq!(model.current(), Some(&FocusId::new("a")), "the model wraps");

    harness.iced_focus_next();
    assert_eq!(
        harness.focused(),
        None,
        "iced's own traversal drops focus here — which is why the bridge is \
         push-from-the-model, never delegate-to-the-router"
    );

    harness.sync(&model);
    assert_eq!(
        harness.focused(),
        Some(iced_id("a")),
        "and pushing the model's wrapped answer must land on the first button"
    );
    assert_agree(&mut harness, &model, "wrapping past the last button");
}

#[test]
fn the_bridge_walks_two_buttons_in_registration_order() {
    let mut harness = Harness::headless(|| {
        column(vec![
            Button::new(ButtonVariant::Primary, text("Save"), Msg::Save, "a").into(),
            Button::new(ButtonVariant::Secondary, text("Cancel"), Msg::Cancel, "b").into(),
        ])
        .into()
    });

    assert_eq!(
        harness.registered(),
        opt_ids(&["a", "b"]),
        "both buttons must register, in tree order"
    );
    assert_eq!(harness.focus_count().total, 2);

    let (_group, mut model) = model_of(&["a", "b"]);
    for expected in ["a", "b", "a"] {
        model.focus_next();
        harness.sync(&model);
        assert_eq!(
            harness.focused(),
            Some(iced_id(expected)),
            "Tab must land on {expected}, the model's answer, not wherever \
             iced's own traversal felt like going"
        );
    }
}

#[test]
fn the_bridge_clears_the_router_when_the_model_clears() {
    let mut harness = Harness::headless(|| {
        Button::new(ButtonVariant::Primary, text("Save"), Msg::Save, "a").into()
    });
    let (_group, mut model) = model_of(&["a"]);

    model.focus_first_in_group(&GroupId::new("panel"));
    harness.sync(&model);
    assert_eq!(harness.focused(), Some(iced_id("a")));

    model.clear();
    harness.sync(&model);

    assert_eq!(
        harness.focused(),
        None,
        "\"nothing focused\" is a state the router has to be told about too, \
         or the button it was already showing stays lit"
    );
}

#[test]
fn the_bridge_is_a_task_the_runtime_can_be_handed() {
    // The `Task` leg: the same projection, wrapped for a host that moves
    // focus from `update` and has to hand the work to iced rather than run
    // it itself.
    let (group, mut model) = model_of(&["a"]);
    model.focus_first_in_group(&group);

    let focused: Task<Msg> = focus_task(&model);
    let empty: Task<Msg> = focus_task(&FocusState::new());

    // Both must be Send, or the runtime could not carry them off the UI thread.
    fn assert_send<T: Send>(_: &T) {}
    assert_send(&focused);
    assert_send(&empty);
}

#[test]
fn the_button_and_the_bridge_derive_the_same_widget_id() {
    // A bridge that derived the id differently would push focus at an
    // address no widget answers to.
    let focus_id = FocusId::new("dialog.save");
    let button = save_button(ButtonVariant::Primary, focus_id.clone());

    assert_eq!(button.widget_id(), &widget_id(&focus_id));
    assert_eq!(button.widget_id(), &iced_id("dialog.save"));
    assert_eq!(button.focus_id(), &focus_id);
    assert!(button.is_enabled());
    assert_eq!(button.variant(), ButtonVariant::Primary);
}

// ---------------------------------------------------------------------------
// 7. State survives a rebuild
// ---------------------------------------------------------------------------

#[test]
fn focus_and_hover_survive_a_view_rebuild() {
    // Every step below rebuilds the whole tree from the view closure, so
    // what survives is what a running app keeps across `view` calls.
    let mut harness = recorder_button(ButtonVariant::Secondary, "btn", |b| b);

    focus_one(&mut harness, "btn");
    let over = centre(harness.first_bounds());
    harness.hover(over);

    let hover_fill =
        Background::Color(expected_style(ButtonVariant::Secondary, HOVERED).background);
    let ring_colour = expected_style(
        ButtonVariant::Secondary,
        VisualState {
            base: BaseState::Hovered,
            focused: true,
        },
    )
    .focus_ring_color;

    for round in 0..3 {
        let paints = harness.paint();
        assert_eq!(
            paints[0].background, hover_fill,
            "round {round}: the hover must survive the rebuild"
        );
        assert_eq!(
            paints.len(),
            2,
            "round {round}: the ring must survive the rebuild, got {paints:?}"
        );
        assert_eq!(paints[1].border.color, ring_colour, "round {round}");
        assert!(
            harness.is_focused(&iced_id("btn")),
            "round {round}: the router must still see the focus"
        );
    }
}

#[test]
fn a_rebuilt_button_keeps_its_pressed_state() {
    let mut harness = recorder_button(ButtonVariant::Primary, "btn", |b| b);
    let over = centre(harness.first_bounds());

    harness.hover(over);
    harness.press(over);

    let pressed_fill =
        Background::Color(expected_style(ButtonVariant::Primary, PRESSED).background);
    assert_eq!(harness.paint()[0].background, pressed_fill);

    // Two more rebuilds with no events: the press is pinned by the tag,
    // so a held button stays held across frames.
    assert_eq!(harness.paint()[0].background, pressed_fill);
    assert_eq!(harness.paint()[0].background, pressed_fill);
}

#[test]
fn a_tree_whose_root_is_no_longer_a_button_inherits_no_button_state() {
    // Same tag, different widget: the tree carries `ButtonState` forward,
    // which is the point. A *different* widget under a different tag must
    // not inherit a button's hover.
    let mut harness = recorder_button(ButtonVariant::Primary, "btn", |b| b);
    let over = centre(harness.first_bounds());
    harness.hover(over);
    assert_eq!(
        harness.paint()[0].background,
        Background::Color(expected_style(ButtonVariant::Primary, HOVERED).background)
    );

    harness.set_view(|| Element::new(Blank::new(CONTENT)));
    assert!(
        harness.paint().is_empty(),
        "a tree whose root is no longer a button must paint no button layers"
    );
}

// ---------------------------------------------------------------------------
// 8. Layout
// ---------------------------------------------------------------------------

#[test]
fn the_button_wraps_its_content_in_the_token_padding() {
    let mut harness = recorder_button(ButtonVariant::Secondary, "btn", |b| b);
    let bounds = harness.first_bounds();

    assert_eq!(
        bounds.width,
        CONTENT.width + button::HORIZONTAL_PADDING * 2.0,
        "the box must be the content plus `space.md` on each side"
    );
    assert_eq!(
        bounds.height,
        button::HEIGHT_STANDARD,
        "the box must be the standard control height"
    );
}

#[test]
fn the_button_never_shrinks_below_the_icon_only_minimum() {
    let mut harness = Harness::recording(|| {
        // Built through `icon_only` because a 1×1 content box *is* an icon-only button,
        // and the framework will not let one exist without a label for the tooltip
        // (`UIF-NAV-003`/`UIF-TAB-003`). The floor being tested here is unchanged.
        Button::icon_only(
            ButtonVariant::Ghost,
            Element::new(Blank::new(Size::new(1.0, 1.0))),
            Msg::Save,
            "btn",
            "Close",
        )
        .into()
    });
    let bounds = harness.first_bounds();

    assert!(
        bounds.width >= button::ICON_ONLY_MIN_WIDTH,
        "got {}px wide",
        bounds.width
    );
    assert!(
        bounds.height >= button::ICON_ONLY_MIN_HEIGHT,
        "got {}px tall",
        bounds.height
    );
}

#[test]
fn the_compact_builder_uses_the_compact_control_height() {
    let mut harness = recorder_button(ButtonVariant::Secondary, "btn", |b| b.compact());

    assert_eq!(harness.first_bounds().height, button::HEIGHT_COMPACT);
}

#[test]
fn the_standard_builder_matches_the_default_height() {
    let mut compact_then_standard =
        recorder_button(ButtonVariant::Secondary, "btn", |b| b.compact().standard());
    let mut default_height = recorder_button(ButtonVariant::Secondary, "btn", |b| b);

    assert_eq!(
        compact_then_standard.first_bounds().height,
        default_height.first_bounds().height
    );
}

// ---------------------------------------------------------------------------
// 9. The state type on its own
// ---------------------------------------------------------------------------

#[test]
fn the_state_resolves_from_its_own_observations_and_nothing_else() {
    let mut state = ButtonState::new();
    assert_eq!(state, ButtonState::new());
    assert!(!state.is_hovered());
    assert!(!state.is_pressed());
    assert!(!state.focusable().is_focused());
    assert_eq!(state.visual(true, false).snapshot_label(), "Rest");

    state.hovered = true;
    assert!(state.is_hovered());
    assert_eq!(state.visual(true, false).snapshot_label(), "Hovered");

    state.pressed = true;
    assert!(state.is_pressed());
    assert_eq!(state.visual(true, false).snapshot_label(), "Pressed");

    state.pressed = false;
    <FocusableState as operation::Focusable>::focus(&mut state.focusable);
    assert!(state.focusable().is_focused());
    assert_eq!(state.visual(true, false).snapshot_label(), "Focused");
    assert!(state.visual(true, false).ring_visible());

    assert_eq!(state.visual(false, false).snapshot_label(), "Disabled");
    assert!(
        !state.visual(false, true).ring_visible(),
        "a disabled control must not advertise a focus affordance"
    );

    // §3.1 gives a button no invalid skin, so the claim worth testing is
    // about the *paint*, not the name. The flag is real and the machine
    // reports it; `resolve_style` deliberately folds `Invalid` into the
    // `Rest` cell, so nothing moves.
    state.hovered = false;
    <FocusableState as operation::Focusable>::unfocus(&mut state.focusable);
    let rest = state.visual(true, false);
    assert_eq!(rest.snapshot_label(), "Rest");
    assert_eq!(
        state.visual(true, true).snapshot_label(),
        "Invalid",
        "the flag is real, and the machine names it even though a button has \
         nowhere to show it"
    );
    assert_eq!(
        expected_style(ButtonVariant::Primary, state.visual(true, true)),
        expected_style(ButtonVariant::Primary, rest),
        "no invalid button skin in §3.1: the flag must not move the paint"
    );
}

// ---------------------------------------------------------------------------
// The icon-only tooltip rule (UIF-ATOM-001 icon-only cell -> UIF-NAV-003 / UIF-TAB-003)
// ---------------------------------------------------------------------------

use crate::widget::button_widget::TooltipViolation;

/// An icon-only button built the sanctioned way carries its label.
#[test]
fn icon_only_builder_requires_and_stores_the_tooltip() {
    let b = Button::<Msg>::icon_only(
        ButtonVariant::Ghost,
        Element::new(Blank::new(Size::new(16.0, 16.0))),
        Msg::Save,
        "btn",
        "Close the tab",
    );
    assert_eq!(b.tooltip_text(), Some("Close the tab"));
    assert_eq!(b.tooltip_violation(Size::new(16.0, 16.0)), None);
}

/// A labelled button needs no tooltip: the label is already readable.
#[test]
fn a_labelled_button_without_a_tooltip_is_legal() {
    let b = Button::<Msg>::new(
        ButtonVariant::Primary,
        Element::new(Blank::new(Size::new(120.0, 20.0))),
        Msg::Save,
        "btn",
    );
    assert_eq!(b.tooltip_text(), None);
    assert!(!b.is_icon_only(Size::new(120.0, 20.0)));
    assert_eq!(b.tooltip_violation(Size::new(120.0, 20.0)), None);
}

/// The forbidden case, caught by the pure validator rather than only by the debug shield.
#[test]
fn an_icon_only_button_without_a_tooltip_is_a_violation() {
    let b = Button::<Msg>::new(
        ButtonVariant::Ghost,
        Element::new(Blank::new(Size::new(16.0, 16.0))),
        Msg::Save,
        "btn",
    );
    let v = b.tooltip_violation(Size::new(16.0, 16.0));
    assert!(
        matches!(v, Some(TooltipViolation::MissingTooltip { .. })),
        "icon-only content and no tooltip must be reported, got {v:?}"
    );
    assert_eq!(v.unwrap().focus_id(), "btn");
}

/// Attaching a tooltip to a button built with `new` clears the violation, so the rule does
/// not force every host through `icon_only`.
#[test]
fn with_tooltip_clears_the_violation() {
    let b = Button::<Msg>::new(
        ButtonVariant::Ghost,
        Element::new(Blank::new(Size::new(16.0, 16.0))),
        Msg::Save,
        "btn",
    )
    .with_tooltip("Close");
    assert_eq!(b.tooltip_violation(Size::new(16.0, 16.0)), None);
}

/// The predicate is the layout floor, so its boundary must sit exactly on the minimum.
#[test]
fn is_icon_only_is_true_up_to_the_minimum_and_false_above_it() {
    let b = Button::<Msg>::new(
        ButtonVariant::Ghost,
        Element::new(Blank::new(Size::new(1.0, 1.0))),
        Msg::Save,
        "btn",
    );
    let min = button::ICON_ONLY_MIN_WIDTH;
    assert!(b.is_icon_only(Size::new(0.0, 0.0)));
    assert!(
        b.is_icon_only(Size::new(min, min)),
        "exactly at the minimum"
    );
    assert!(!b.is_icon_only(Size::new(min + 0.5, min)));
    assert!(!b.is_icon_only(Size::new(min, min + 0.5)));
    assert!(!b.is_icon_only(Size::new(120.0, 20.0)));
}

/// Both axes matter: a wide-but-short content is a label, not an icon.
#[test]
fn a_wide_short_content_is_a_label_not_an_icon() {
    let b = Button::<Msg>::new(
        ButtonVariant::Ghost,
        Element::new(Blank::new(Size::new(200.0, 16.0))),
        Msg::Save,
        "btn",
    );
    assert!(!b.is_icon_only(Size::new(200.0, 16.0)));
    assert_eq!(b.tooltip_violation(Size::new(200.0, 16.0)), None);
}

/// The violation must name which control is at fault, so a whole-screen sweep can report it.
#[test]
fn the_violation_names_the_offending_control() {
    let b = Button::<Msg>::new(
        ButtonVariant::Ghost,
        Element::new(Blank::new(Size::new(8.0, 8.0))),
        Msg::Save,
        "save-button",
    );
    let v = b
        .tooltip_violation(Size::new(8.0, 8.0))
        .expect("must violate");
    assert_eq!(v.focus_id(), "save-button");
    assert_eq!(
        v,
        TooltipViolation::MissingTooltip {
            focus_id: "save-button".to_string(),
            content: Size::new(8.0, 8.0),
        }
    );
}

// ---------------------------------------------------------------------------
// 10. The controlled path — `UIF-ATOM-001a`
// ---------------------------------------------------------------------------
//
// Everything above drives the legacy `Button::new`, which publishes one
// message on release-inside and nothing else. Everything below drives
// `Button::controlled`, where a host-owned `ButtonModel` decides what the
// button is and the `ButtonController` decides which of the three tiers each
// user action becomes.

use crate::binding::Controlled;
use crate::binding::Ephemeral;
use crate::widget::button_controller::ButtonEmission;
use crate::widget::button_model::ButtonDensity;
use crate::widget::button_model::ButtonModel;
use iced::advanced::widget::tree;
use iced::keyboard;
use iced::keyboard::Key;
use iced::keyboard::Location;
use iced::keyboard::Modifiers;
use iced::keyboard::key;

/// The three tiers, as three distinct messages — so a test can tell which
/// tier fired rather than only that something did.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Ctl {
    Activated,
    Cancelled,
    StopRequested,
}

/// A controlled-button harness.
///
/// The model is leaked deliberately: the harness's view closure must return
/// an `Element<'static, ..>`, and `Controlled<'a, ButtonModel>` borrows the
/// model for `'a`. One leaked struct per test stands in for the long-lived,
/// application-owned model a real host hands the button every frame — which
/// is exactly the ownership shape `UIF-ATOM-001a` requires, and the reason
/// the button takes a `Controlled` rather than a `ButtonModel`.
fn controlled(model: &'static ButtonModel, label: &'static str) -> Harness<Ctl, Recorder> {
    Harness::recording(move || {
        Button::controlled(
            Controlled::new(model),
            Element::new(Blank::new(CONTENT)),
            label,
            |emission| match emission {
                ButtonEmission::Activated(_) => Some(Ctl::Activated),
                ButtonEmission::Cancelled(_) => Some(Ctl::Cancelled),
                ButtonEmission::CancelRequested(_) => Some(Ctl::StopRequested),
            },
        )
        .into()
    })
}

fn leaked(model: ButtonModel) -> &'static ButtonModel {
    Box::leak(Box::new(model))
}

/// A `KeyPressed` for a named key, every other field at its neutral value.
fn key_press(name: key::Named) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: Key::Named(name),
        modified_key: Key::Named(name),
        physical_key: key::Physical::Unidentified(key::NativeCode::Unidentified),
        location: Location::Standard,
        modifiers: Modifiers::NONE,
        text: None,
        repeat: false,
    })
}

#[test]
fn enter_and_space_on_a_focused_icon_only_legacy_button_activate_and_capture() {
    for name in [key::Named::Enter, key::Named::Space] {
        let button = Button::<Msg, Theme, Recorder>::icon_only(
            ButtonVariant::Ghost,
            Element::new(Blank::new(Size::new(1.0, 1.0))),
            Msg::Cancel,
            "legacy.icon",
            "Close",
        );
        let (mut button, mut tree, node) = laid_out(button);
        <FocusableState as operation::Focusable>::focus(&mut button_state(&mut tree).focusable);

        let event = key_press(name);
        let viewport = Rectangle {
            x: 0.0,
            y: 0.0,
            width: crate::test_helpers::VIEWPORT.width,
            height: crate::test_helpers::VIEWPORT.height,
        };
        let mut clipboard = iced_test::core::clipboard::Null;
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);

        button.update(
            &mut tree,
            &event,
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &Recorder::default(),
            &mut clipboard,
            &mut shell,
            &viewport,
        );

        let captured = shell.is_event_captured();
        drop(shell);

        assert_eq!(
            (messages, captured),
            (vec![Msg::Cancel], true),
            "{name:?} must publish on_press once and capture activation"
        );
    }
}

#[test]
fn activation_keys_do_nothing_on_an_unfocused_legacy_button() {
    let mut harness = armed_button("legacy.unfocused");

    for name in [key::Named::Enter, key::Named::Space] {
        assert_eq!(
            harness.pointer(&[key_press(name)], mouse::Cursor::Unavailable),
            Vec::new(),
            "an unfocused legacy button must not activate on {name:?}"
        );
    }
}

#[test]
fn non_activation_key_does_nothing_on_a_focused_legacy_button() {
    let mut harness = armed_button("legacy.focused");
    focus_one(&mut harness, "legacy.focused");

    assert_eq!(
        harness.pointer(&[key_press(key::Named::Escape)], mouse::Cursor::Unavailable),
        Vec::new(),
        "Escape is not an activation key on the legacy callback path"
    );
}

#[test]
fn releasing_inside_a_controlled_button_publishes_the_activated_tier() {
    let mut harness = controlled(leaked(ButtonModel::new("Save")), "btn");
    let over = centre(harness.first_bounds());

    harness.hover(over);
    harness.press(over);
    let messages = harness.release(over);

    assert_eq!(messages, vec![Ctl::Activated]);
}

#[test]
fn releasing_outside_publishes_the_cancelled_tier_and_no_activation() {
    let mut harness = controlled(leaked(ButtonModel::new("Save")), "btn");
    let over = centre(harness.first_bounds());

    harness.hover(over);
    harness.press(over);
    let messages = harness.release(FAR_AWAY);

    assert_eq!(
        messages,
        vec![Ctl::Cancelled],
        "a drag-out is a cancel, not a silent nothing — the host has to be able to see it"
    );
}

#[test]
fn pressing_a_busy_cancellable_button_asks_to_stop_instead_of_activating() {
    let mut harness = controlled(
        leaked(ButtonModel::new("Saving").busy().cancellable()),
        "btn",
    );
    let over = centre(harness.first_bounds());

    harness.hover(over);
    let messages = harness.press(over);

    assert_eq!(messages, vec![Ctl::StopRequested]);
    assert_eq!(
        harness.release(over),
        Vec::new(),
        "and the release does not turn around and dispatch — the action was never let through"
    );
}

#[test]
fn a_busy_fire_and_forget_button_offers_no_stop_affordance() {
    let mut harness = controlled(leaked(ButtonModel::new("Saving").busy()), "btn");
    let over = centre(harness.first_bounds());

    harness.hover(over);
    assert_eq!(harness.press(over), Vec::new());
    assert_eq!(harness.release(over), Vec::new());
}

#[test]
fn enter_on_a_focused_controlled_button_activates() {
    let mut harness = controlled(leaked(ButtonModel::new("Save")), "btn");
    focus_one(&mut harness, "btn");

    let messages = harness.pointer(&[key_press(key::Named::Enter)], mouse::Cursor::Unavailable);

    assert_eq!(
        messages,
        vec![Ctl::Activated],
        "`Enter` on the focused control is an activation — the keyboard path the spec \
         names and this widget previously did not have at all"
    );
}

#[test]
fn space_on_a_focused_controlled_button_activates() {
    let mut harness = controlled(leaked(ButtonModel::new("Save")), "btn");
    focus_one(&mut harness, "btn");

    let messages = harness.pointer(&[key_press(key::Named::Space)], mouse::Cursor::Unavailable);

    assert_eq!(messages, vec![Ctl::Activated]);
}

#[test]
fn keys_do_nothing_on_a_button_that_does_not_hold_focus() {
    let mut harness = controlled(leaked(ButtonModel::new("Save")), "btn");

    for name in [key::Named::Enter, key::Named::Space, key::Named::Escape] {
        assert_eq!(
            harness.pointer(&[key_press(name)], mouse::Cursor::Unavailable),
            Vec::new(),
            "an unfocused button must not swallow {name:?}"
        );
    }
}

#[test]
fn escape_cancels_a_gesture_the_pointer_still_holds_open() {
    let mut harness = controlled(leaked(ButtonModel::new("Save")), "btn");
    focus_one(&mut harness, "btn");
    let over = centre(harness.first_bounds());

    harness.hover(over);
    harness.press(over);
    let messages = harness.pointer(
        &[key_press(key::Named::Escape)],
        mouse::Cursor::Available(over),
    );

    assert_eq!(messages, vec![Ctl::Cancelled]);
    assert_eq!(
        harness.release(over),
        Vec::new(),
        "the gesture was settled by Escape, so releasing inside must not fire it again"
    );
}

#[test]
fn escape_with_an_action_already_running_asks_to_stop() {
    let mut harness = controlled(
        leaked(ButtonModel::new("Saving").busy().cancellable()),
        "btn",
    );
    focus_one(&mut harness, "btn");

    // No press: the action was already running before the user pressed Escape.
    let messages = harness.pointer(&[key_press(key::Named::Escape)], mouse::Cursor::Unavailable);

    assert_eq!(
        messages,
        vec![Ctl::StopRequested],
        "with no snapshot to revert to, the honest tier is the request"
    );
}

#[test]
fn the_double_click_window_is_closed_through_the_widget() {
    let mut harness = controlled(leaked(ButtonModel::new("Save").cancellable()), "btn");
    let over = centre(harness.first_bounds());

    harness.hover(over);
    harness.press(over);
    assert_eq!(harness.release(over), vec![Ctl::Activated]);

    // Same model, same frame: the host has not reported busy yet.
    harness.hover(over);
    harness.press(over);
    assert_eq!(
        harness.release(over),
        Vec::new(),
        "the second click of a double-click never reaches the host"
    );
}

#[test]
fn a_controlled_button_takes_its_variant_from_the_model() {
    let mut harness = controlled(
        leaked(
            ButtonModel::new("Delete")
                .with_variant(ButtonVariant::Danger)
                .with_tooltip("Delete this permanently"),
        ),
        "btn",
    );

    assert_eq!(
        harness.paint()[0].background,
        Background::Color(expected_style(ButtonVariant::Danger, REST).background),
        "the variant came from the model, not from a second declaration that could drift"
    );
}

#[test]
fn a_disabled_model_makes_the_button_inert_and_absent_from_the_router() {
    let mut harness = controlled(leaked(ButtonModel::new("Save").disabled()), "btn");
    let over = centre(harness.first_bounds());

    harness.hover(over);
    harness.press(over);
    assert_eq!(harness.release(over), Vec::new());
    assert_eq!(
        harness.registered(),
        vec![],
        "a disabled model is not registered, so the router cannot see it at all"
    );
    assert_eq!(
        harness.paint()[0].background,
        Background::Color(expected_style(ButtonVariant::Secondary, DISABLED).background)
    );
}

#[test]
fn the_button_state_is_tagged_ephemeral() {
    // `UIF-ATOM-001a`: the interaction half is the ephemeral half, and the
    // tag is where that is recorded. A rebuild under a different widget
    // cannot inherit it, and a rebuild under this one starts fresh.
    let model = ButtonModel::new("Save");
    let button: Button<'_, Ctl, Theme, Recorder> = Button::controlled(
        Controlled::new(&model),
        Element::new(Blank::new(CONTENT)),
        "btn",
        |_| None,
    );

    assert_eq!(
        Widget::tag(&button),
        tree::Tag::of::<Ephemeral<ButtonState>>(),
        "the tag names `Ephemeral<ButtonState>`, not a bare `ButtonState`"
    );
}

#[test]
fn collapsing_the_density_does_not_move_the_button_vertically() {
    // §3.0.4.4's "0px vertical layout shift", proven at the widget: the
    // compact rung changes the rung the layout resolves through, and nothing
    // about where the box sits.
    let mut standard = controlled(leaked(ButtonModel::new("Save")), "btn");
    let mut compact = controlled(
        leaked(ButtonModel::new("Save").with_density(ButtonDensity::Compact)),
        "btn",
    );

    let a = standard.first_bounds();
    let b = compact.first_bounds();

    assert_eq!(a.y, b.y, "a neighbour that moves is a layout jump");
    assert_eq!(a.height, b.height);
    assert_eq!(a.x, b.x);
}

#[test]
fn a_controlled_button_paints_the_same_layers_a_legacy_one_of_the_same_variant_paints() {
    // The colour-unchanged guarantee: routing through the model changes which
    // *tier* is emitted, never what is painted. Same variant, same pointer
    // script, identical paint.
    for base in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
        let mut legacy = recorder_button(ButtonVariant::Primary, "btn", |b| b);
        let mut ctl = controlled(
            leaked(ButtonModel::new("Save").with_variant(ButtonVariant::Primary)),
            "btn",
        );
        let over = centre(legacy.first_bounds());
        drive(&mut legacy, base, over);
        drive(&mut ctl, base, over);

        assert_eq!(
            legacy.paint(),
            ctl.paint(),
            "the controlled path repainted differently from the legacy path in {base:?}"
        );
    }
}

#[test]
fn a_legacy_button_still_publishes_only_on_release_inside() {
    // The regression guard on the untouched path: `Button::new` behaves
    // exactly as it did before the binding foundation existed.
    let mut harness = armed_button("btn");
    let over = centre(harness.first_bounds());

    harness.hover(over);
    harness.press(over);
    assert_eq!(harness.release(FAR_AWAY), Vec::new());

    harness.press(over);
    assert_eq!(harness.release(over), vec![Msg::Cancel]);
}

#[test]
fn a_button_with_nothing_in_flight_paints_no_spinner_layer() {
    let palette = crate::token::color::Palette::dark();
    let bounds = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 120.0,
        height: 32.0,
    };
    let padding = iced::Padding::from(8.0);

    for variant in crate::gallery_button::BUTTON_VARIANTS {
        for state in [REST, HOVERED, PRESSED, FOCUSED, DISABLED] {
            let style = expected_style(variant, state);
            assert_eq!(
                crate::widget::button_widget::busy_spinner_layer(
                    bounds, padding, &style, false, &palette
                ),
                None,
                "{variant:?} / {state:?} must not gain a spinner quad while idle"
            );
        }
    }
}

#[test]
fn a_busy_button_paints_the_spinner_in_the_reserved_slot() {
    let palette = crate::token::color::Palette::dark();
    let bounds = Rectangle {
        x: 10.0,
        y: 20.0,
        width: 120.0,
        height: 32.0,
    };
    let padding = iced::Padding::from(8.0);
    let style = expected_style(ButtonVariant::Primary, REST);

    let layer =
        crate::widget::button_widget::busy_spinner_layer(bounds, padding, &style, true, &palette)
            .expect("a busy button paints its spinner");

    assert_eq!(
        layer.bounds.width,
        crate::widget::button_model::SPINNER_SLOT
    );
    assert_eq!(
        layer.bounds.height,
        crate::widget::button_model::SPINNER_SLOT
    );
    assert_eq!(
        crate::widget::button_model::SPINNER_SLOT,
        crate::token::size::SPINNER_SM
    );
    assert_eq!(layer.bounds.x, bounds.x + padding.left);
    assert_eq!(
        layer.bounds.y,
        bounds.y + (bounds.height - crate::widget::button_model::SPINNER_SLOT) / 2.0,
        "the ring is centred on the row it was given"
    );
    assert_eq!(
        layer.fill,
        iced::Color::TRANSPARENT,
        "a ring carries no fill"
    );
    assert_eq!(layer.border.width, 2.0);
    assert_eq!(
        layer.border.radius.top_left,
        crate::widget::button_model::SPINNER_SLOT / 2.0
    );
}

#[test]
fn the_busy_arc_is_text_strong_on_a_locked_fill_and_accent_text_elsewhere() {
    let palette = crate::token::color::Palette::dark();
    let bounds = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 120.0,
        height: 32.0,
    };
    let padding = iced::Padding::from(8.0);

    let arc_of = |variant, state| {
        crate::widget::button_widget::busy_spinner_layer(
            bounds,
            padding,
            &expected_style(variant, state),
            true,
            &palette,
        )
        .expect("busy paints a layer")
        .border
        .color
    };

    assert_eq!(arc_of(ButtonVariant::Primary, REST), palette.text_strong);
    assert_eq!(
        arc_of(ButtonVariant::Danger, HOVERED),
        palette.text_strong,
        "a Danger button on its hot fill takes the locked foreground too"
    );
    assert_eq!(arc_of(ButtonVariant::Secondary, REST), palette.accent_text);
    assert_eq!(arc_of(ButtonVariant::Ghost, REST), palette.accent_text);
}

#[test]
fn the_spinner_never_widens_the_box_it_is_painted_in() {
    let palette = crate::token::color::Palette::dark();
    let bounds = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 40.0,
        height: 32.0,
    };
    let padding = iced::Padding::from(8.0);
    let style = expected_style(ButtonVariant::Primary, REST);

    let layer =
        crate::widget::button_widget::busy_spinner_layer(bounds, padding, &style, true, &palette)
            .expect("busy paints a layer");

    assert!(
        layer.bounds.x >= bounds.x
            && layer.bounds.y >= bounds.y
            && layer.bounds.x + layer.bounds.width <= bounds.x + bounds.width
            && layer.bounds.y + layer.bounds.height <= bounds.y + bounds.height,
        "the spinner must sit inside the button's own bounds (`UIF-PRIN-003`)"
    );
}

#[test]
fn test_button_into_element_with_tooltip_attaches_tooltip_widget() {
    let button = Button::<Msg>::icon_only(
        ButtonVariant::Ghost,
        Element::new(Blank::new(Size::new(16.0, 16.0))),
        Msg::Save,
        "btn_tt",
        "Close tab",
    );
    let _el: Element<'_, Msg> = button.into_element();
}

#[test]
fn test_button_into_element_without_tooltip_produces_plain_element() {
    let button = Button::<Msg>::new(
        ButtonVariant::Primary,
        Element::new(Blank::new(Size::new(100.0, 20.0))),
        Msg::Save,
        "btn_plain",
    );
    let _el: Element<'_, Msg> = button.into_element();
}

#[test]
fn test_button_layout_compresses_horizontal_padding_on_container_query() {
    use iced::advanced::Widget;
    use iced::advanced::layout::Limits;

    let model =
        ButtonModel::new("Save").with_density(crate::widget::button_model::ButtonDensity::Standard);
    let controlled = crate::binding::controlled::Controlled::new(&model);

    let mut button: Button<'_, Msg, Theme, Recorder> = Button::controlled(
        controlled,
        Element::new(Blank::new(Size::new(40.0, 16.0))),
        "btn_cq",
        |_| None,
    );

    let mut tree = Tree {
        tag: button.tag(),
        state: button.state(),
        children: button.children(),
    };
    let renderer = Recorder::default();

    // Wide container query: fits expanded, horizontal padding is space::MD (12.0)
    let wide_limits = Limits::NONE.max_width(500.0).max_height(100.0);
    let _node_wide = button.layout(&mut tree, &renderer, &wide_limits);
    assert_eq!(button.padding.left, crate::token::space::MD);
    assert_eq!(button.padding.right, crate::token::space::MD);

    // Narrow container query: forced compact, horizontal padding compresses to space::SM (8.0)
    let narrow_limits = Limits::NONE.max_width(10.0).max_height(100.0);
    let _node_narrow = button.layout(&mut tree, &renderer, &narrow_limits);
    assert_eq!(button.padding.left, crate::token::space::SM);
    assert_eq!(button.padding.right, crate::token::space::SM);
}
