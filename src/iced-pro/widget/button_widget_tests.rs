//! Proofs that the pure layers are sufficient to build a real, painting,
//! keyboard-focusable button — and the technique for asserting what a
//! widget actually painted without a screen.
//!
//! # The harness is `INC-10`'s
//!
//! These tests drive focus the same way `focusable_tests.rs` does, because
//! that is the only way focus can be driven at all in iced 0.14
//! (`INC-10` constraints 3 and 4): a headless renderer, a real
//! [`UserInterface`], a [`user_interface::Cache`] carried from one build
//! to the next so the widget [`Tree`] — where [`ButtonState`] lives —
//! survives a rebuild the way it does across `view` calls, and the
//! operation chain loop `UserInterface::operate` does not run for itself.
//!
//! The harness is generic over the renderer rather than fixed to one, which
//! is what lets the *same* harness answer two different questions instead
//! of growing a second one:
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
//! crate. [`Recorder`] says it directly.
//!
//! [`Blank`] is the content that makes the recording leg possible: a
//! widget that draws nothing and needs no text renderer, so the button's
//! own layers are the only thing in the recording.
//!
//! # Reading state back
//!
//! The widget tree is owned by the cache and the cache exposes nothing, so
//! hover, press and focus are read through what the button *paints* rather
//! than through an accessor that would exist only for tests. That is the
//! stronger assertion anyway: `state.hovered == true` is an internal
//! fact; "this frame painted the hover fill and the ring" is the behaviour
//! the spec has to hold.

use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::focus::GroupId;
use crate::state::BaseState;
use crate::state::VisualState;
use crate::token::color::Palette;
use crate::widget::button;
use crate::widget::button::ButtonStyle;
use crate::widget::button::ButtonVariant;
use crate::widget::button_widget::Button;
use crate::widget::button_widget::ButtonState;
use crate::widget::button_widget::background_layer;
use crate::widget::button_widget::focus_operation;
use crate::widget::button_widget::focus_ring_layer;
use crate::widget::button_widget::focus_task;
use crate::widget::button_widget::widget_id;
use crate::widget::focus::resolve_ring_color_for_background;
use crate::widget::focusable::FocusableState;

use iced::Color;
use iced::Element;
use iced::Event;
use iced::Font;
use iced::Length;
use iced::Pixels;
use iced::Point;
use iced::Rectangle;
use iced::Size;
use iced::Task;
use iced::advanced::image;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{Operation, Tree, Widget};
use iced::mouse;
use iced::widget::column;
use iced::widget::text;

use iced_test::core::Background;
use iced_test::core::Border;
use iced_test::core::Transformation;
use iced_test::core::clipboard::Null;
use iced_test::core::widget::Id;
use iced_test::core::widget::operation::{self};
use iced_test::core::widget::operation::{Outcome, black_box};
use iced_test::futures::futures::executor::block_on;
use iced_test::renderer::Renderer as HeadlessRenderer;
use iced_test::runtime::UserInterface;
use iced_test::runtime::user_interface;

use std::cell::RefCell;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Save,
    Cancel,
}

type Theme = iced::Theme;
type El<'a, R> = Element<'a, Msg, Theme, R>;
type Ui<'a, R> = UserInterface<'a, Msg, Theme, R>;

const VIEWPORT: Size = Size::new(400.0, 300.0);

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
// Harness — the INC-10 focus harness, generic over the renderer
// ---------------------------------------------------------------------------

/// A headless iced interface whose widget state is carried across
/// rebuilds of whatever its view closure produces.
///
/// Identical in shape to `focusable_tests.rs`'s harness — same carried
/// [`user_interface::Cache`], same operation chain loop, same
/// [`RecordFocusables`] read-back — with two extra legs a button needs
/// and a bare focus wrapper does not: [`pointer`](Self::pointer), which
/// drives real `UserInterface::update` pointer events and hands back the
/// messages the tree published, and [`paint`](Self::paint), which runs a
/// real `UserInterface::draw` and hands back what the renderer was asked
/// to draw.
///
/// Being generic over `R` is what keeps this from being a second focus
/// harness: the focus protocol, the cache carrying and the chain loop are
/// one implementation, used by both the real-renderer and the
/// recording-renderer tests.
struct Harness<R> {
    renderer: R,
    cache: user_interface::Cache,
    view: Box<dyn FnMut() -> El<'static, R>>,
}

impl<R: renderer::Renderer> Harness<R> {
    fn new(renderer: R, view: impl Fn() -> El<'static, R> + 'static) -> Self {
        Self {
            renderer,
            cache: user_interface::Cache::default(),
            view: Box::new(view),
        }
    }

    /// Swap the tree the harness rebuilds on the next step.
    fn set_view(&mut self, view: impl Fn() -> El<'static, R> + 'static) {
        self.view = Box::new(view);
    }

    /// Rebuild the current view over the carried cache.
    ///
    /// Every step of this harness starts here, so every step is a rebuild:
    /// a state that survives one survives a `view` call.
    fn begin(&mut self) -> Ui<'static, R> {
        let root = (self.view)();
        let cache = std::mem::take(&mut self.cache);

        UserInterface::build(root, VIEWPORT, cache, &mut self.renderer)
    }

    /// Carry a built interface's widget state back out for the next step.
    fn commit(&mut self, ui: Ui<'static, R>) {
        self.cache = ui.into_cache();
    }

    /// Run one operation — and its whole chain — against a freshly
    /// rebuilt interface.
    ///
    /// The chain loop mirrors `iced_winit`'s `Action::Widget` handler;
    /// without it `focus_next` would run only its `count` half.
    fn run<T>(&mut self, mut operation: Box<dyn Operation<T>>) -> Option<T> {
        let mut ui = self.begin();

        let result = loop {
            ui.operate(&self.renderer, &mut black_box(operation.as_mut()));

            match operation.finish() {
                Outcome::None => break None,
                Outcome::Some(value) => break Some(value),
                Outcome::Chain(next) => operation = next,
            }
        };

        self.commit(ui);
        result
    }

    /// Which id the router reports as focused, if any.
    fn focused(&mut self) -> Option<Id> {
        self.run(Box::new(operation::focusable::find_focused()))
    }

    /// Whether the widget with `id` reports itself focused.
    fn is_focused(&mut self, id: &Id) -> bool {
        self.run(Box::new(operation::focusable::is_focused(id.clone())))
            .unwrap_or(false)
    }

    /// How many widgets the router sees, and which index holds focus.
    fn count(&mut self) -> operation::focusable::Count {
        self.run(Box::new(operation::focusable::count()))
            .unwrap_or_default()
    }

    /// The ids the router sees, in traversal order.
    fn registered(&mut self) -> Vec<Option<Id>> {
        self.recorded()
            .into_iter()
            .map(|(id, _bounds)| id)
            .collect()
    }

    /// The `(id, bounds)` pairs the router sees, in traversal order.
    fn recorded(&mut self) -> Vec<(Option<Id>, Rectangle)> {
        self.run(Box::<RecordFocusables>::default())
            .unwrap_or_default()
    }

    /// Ask iced's own router to move focus to the next widget it sees.
    ///
    /// Only used to pin that it *loses* focus at the ends, which is why
    /// the bridge must not use it.
    fn iced_focus_next(&mut self) {
        self.run(Box::new(operation::focusable::focus_next::<Msg>()));
    }

    /// Deliver pointer events with the cursor where the test says, and
    /// report what the tree published.
    ///
    /// One call is one frame: the tree is rebuilt, the events delivered,
    /// the state carried back out.
    fn pointer(&mut self, events: &[Event], cursor: mouse::Cursor) -> Vec<Msg> {
        let mut ui = self.begin();
        let mut messages = Vec::new();
        let mut clipboard = Null;

        ui.update(
            events,
            cursor,
            &mut self.renderer,
            &mut clipboard,
            &mut messages,
        );

        self.commit(ui);
        messages
    }
}

impl Harness<Recorder> {
    /// Paint the current tree and report every quad the renderer was
    /// asked to fill, in the order it was asked.
    ///
    /// A paint is a rebuild, so this reads the state that survived rather
    /// than the state a previous step left in a local variable.
    fn paint(&mut self) -> Vec<Paint> {
        self.renderer.painted.borrow_mut().clear();

        let mut ui = self.begin();

        ui.draw(
            &mut self.renderer,
            &Theme::Dark,
            &renderer::Style {
                text_color: Color::BLACK,
            },
            mouse::Cursor::Unavailable,
        );

        self.commit(ui);
        self.renderer.painted.borrow().clone()
    }

    /// The box the button laid out to, read from the layer that fills it.
    ///
    /// [`button::resolve_style`] always sets a non-zero border width, so
    /// [`background_layer`] always produces a layer and the first recorded
    /// quad is always the button's own box.
    fn button_bounds(&mut self) -> Rectangle {
        self.paint()
            .first()
            .copied()
            .unwrap_or_else(|| panic!("a button must paint its own box"))
            .bounds
    }
}

impl Harness<HeadlessRenderer> {
    /// A harness over iced's real headless renderer.
    fn headless(view: impl Fn() -> El<'static, HeadlessRenderer> + 'static) -> Self {
        let renderer = block_on(<HeadlessRenderer as renderer::Headless>::new(
            Font::with_name("Fira Sans"),
            Pixels(16.0),
            None,
        ))
        .expect("a headless renderer should be available for button tests");

        Self::new(renderer, view)
    }

    /// Run a real draw pass over the current tree.
    ///
    /// Nothing is asserted about pixels: this proves [`Widget::draw`] runs
    /// to completion against a real renderer — real quads, real text,
    /// real layer calls — in whatever state the tree is currently in.
    fn render(&mut self) {
        let mut ui = self.begin();

        ui.draw(
            &mut self.renderer,
            &Theme::Dark,
            &renderer::Style {
                text_color: Color::BLACK,
            },
            mouse::Cursor::Unavailable,
        );

        self.commit(ui);
    }
}

/// An operation that records every focusable widget the router visits.
///
/// `Send` holds because the recording sits behind a `RefCell`, which is
/// `Send` for a `Send` payload, and the operation never leaves its thread.
#[derive(Default)]
struct RecordFocusables {
    seen: RefCell<Vec<(Option<Id>, Rectangle)>>,
}

type Recorded = Vec<(Option<Id>, Rectangle)>;

impl Operation<Recorded> for RecordFocusables {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Recorded>)) {
        operate(self);
    }

    fn focusable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        _state: &mut dyn operation::Focusable,
    ) {
        self.seen.borrow_mut().push((id.cloned(), bounds));
    }

    fn finish(&self) -> Outcome<Recorded> {
        Outcome::Some(self.seen.borrow().clone())
    }
}

// ---------------------------------------------------------------------------
// Recorder — a renderer that reports instead of rasterising
// ---------------------------------------------------------------------------

/// One `fill_quad` a widget asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Paint {
    bounds: Rectangle,
    background: Background,
    border: Border,
}

/// A renderer that records what it is told to draw.
///
/// Implements only the base [`renderer::Renderer`] protocol, which is all
/// [`Button`] needs: its content is [`Blank`], so no text renderer is
/// reachable and the recording contains nothing but the button's own
/// layers.
#[derive(Default)]
struct Recorder {
    painted: RefCell<Vec<Paint>>,
}

impl renderer::Renderer for Recorder {
    fn start_layer(&mut self, _bounds: Rectangle) {}

    fn end_layer(&mut self) {}

    fn start_transformation(&mut self, _transformation: Transformation) {}

    fn end_transformation(&mut self) {}

    fn fill_quad(&mut self, quad: renderer::Quad, background: impl Into<Background>) {
        self.painted.borrow_mut().push(Paint {
            bounds: quad.bounds,
            background: background.into(),
            border: quad.border,
        });
    }

    fn reset(&mut self, _new_bounds: Rectangle) {}

    fn allocate_image(
        &mut self,
        _handle: &image::Handle,
        _callback: impl FnOnce(Result<image::Allocation, image::Error>) + Send + 'static,
    ) {
    }
}

/// Content that draws nothing and needs no text renderer.
///
/// Lets the recording leg see only the button's own paint.
struct Blank(Size);

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Blank
where
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.0.width), Length::Fixed(self.0.height))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(self.0)
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn iced_id(label: &str) -> Id {
    Id::from(label.to_owned())
}

fn opt_ids(labels: &[&'static str]) -> Vec<Option<Id>> {
    labels.iter().map(|label| Some(iced_id(label))).collect()
}

/// A recording harness around one button with blank content.
fn recorder_button(
    variant: ButtonVariant,
    label: &'static str,
    configure: impl Fn(Button<'static, Msg, Theme, Recorder>) -> Button<'static, Msg, Theme, Recorder>
    + 'static,
) -> Harness<Recorder> {
    Harness::new(Recorder::default(), move || {
        configure(Button::new(
            variant,
            Element::new(Blank(CONTENT)),
            Msg::Save,
            label,
        ))
        .into()
    })
}

/// A recording harness around one button that publishes a message
/// distinguishable from the fixtures' default, so "fired" and "did not
/// fire" cannot be confused with a fixture wiring mistake.
fn armed_button(label: &'static str) -> Harness<Recorder> {
    Harness::new(Recorder::default(), move || {
        Button::new(
            ButtonVariant::Primary,
            Element::new(Blank(CONTENT)),
            Msg::Cancel,
            label,
        )
        .into()
    })
}

/// A model holding `labels` as one enabled group, in traversal order.
fn model_of(labels: &[&'static str]) -> (GroupId, FocusState) {
    let group = GroupId::new("panel");
    let mut model = FocusState::new();
    model.register_group(
        group.clone(),
        labels
            .iter()
            .map(|label| FocusMember::new(*label))
            .collect(),
    );

    (group, model)
}

/// Push the model's current focus into the router through the bridge.
fn sync<R: renderer::Renderer>(harness: &mut Harness<R>, model: &FocusState) {
    harness.run(focus_operation::<Msg>(model));
}

fn assert_agree<R: renderer::Renderer>(harness: &mut Harness<R>, model: &FocusState, step: &str) {
    let expected = model.current().map(|id| iced_id(id.as_str()));
    assert_eq!(
        harness.focused(),
        expected,
        "after {step}: the router and `FocusState` must name the same control"
    );
}

/// Focus `label` through a one-member model, the way a host would.
fn focus_one<R: renderer::Renderer>(harness: &mut Harness<R>, label: &'static str) {
    let mut model = FocusState::new();
    model.register_group(GroupId::new("panel"), vec![FocusMember::new(label)]);
    assert!(model.focus(&FocusId::new(label)));
    sync(harness, &model);
}

fn moved(to: Point) -> Event {
    Event::Mouse(mouse::Event::CursorMoved { position: to })
}

fn pressed() -> Event {
    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
}

fn released() -> Event {
    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
}

/// The style a `(variant, state)` pair is supposed to paint, against the
/// palette every fixture uses.
fn expected_style(variant: ButtonVariant, visual: VisualState) -> ButtonStyle {
    button::resolve_style(variant, visual, &Palette::dark())
}

/// The centre of a rectangle.
fn centre(bounds: Rectangle) -> Point {
    Point::new(
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    )
}

/// Drive the pointer so the button lands in `base`.
///
/// `Disabled` and `Invalid` are driven the same hard way as `Pressed` —
/// cursor over, button down — because that is the only way to show the
/// state machine overrode the pointer rather than never received it.
fn drive<R: renderer::Renderer>(harness: &mut Harness<R>, base: BaseState, over: Point) {
    match base {
        BaseState::Rest | BaseState::Invalid => {
            harness.pointer(&[moved(over)], mouse::Cursor::Unavailable);
        }
        BaseState::Hovered => {
            harness.pointer(&[moved(over)], mouse::Cursor::Available(over));
        }
        BaseState::Pressed | BaseState::Disabled => {
            harness.pointer(&[moved(over)], mouse::Cursor::Available(over));
            harness.pointer(&[pressed()], mouse::Cursor::Available(over));
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
        &layout::Limits::new(Size::ZERO, VIEWPORT),
    );

    (button, tree, node)
}

fn save_button(
    variant: ButtonVariant,
    label: impl Into<FocusId>,
) -> Button<'static, Msg, Theme, Recorder> {
    Button::new(variant, Element::new(Blank(CONTENT)), Msg::Save, label)
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

                let over = centre(harness.button_bounds());
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
    let over = centre(harness.button_bounds());

    harness.pointer(&[moved(over)], mouse::Cursor::Available(over));

    assert_eq!(
        harness.paint()[0].background,
        Background::Color(expected_style(ButtonVariant::Secondary, HOVERED).background),
        "a cursor inside the bounds must paint the hover fill"
    );
}

#[test]
fn the_cursor_leaving_the_button_clears_the_hover_fill() {
    let mut harness = recorder_button(ButtonVariant::Secondary, "btn", |b| b);
    let over = centre(harness.button_bounds());

    harness.pointer(&[moved(over)], mouse::Cursor::Available(over));
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
    let over = centre(harness.button_bounds());

    harness.pointer(&[moved(over)], mouse::Cursor::Available(over));
    harness.pointer(&[pressed()], mouse::Cursor::Available(over));
    assert_eq!(
        harness.paint()[0].background,
        Background::Color(expected_style(ButtonVariant::Primary, PRESSED).background),
        "button-down inside the bounds must paint the pressed fill"
    );

    harness.pointer(&[released()], mouse::Cursor::Available(over));
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
        width: VIEWPORT.width,
        height: VIEWPORT.height,
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
    let over = centre(harness.button_bounds());

    harness.pointer(&[moved(over)], mouse::Cursor::Available(over));
    harness.pointer(&[pressed()], mouse::Cursor::Available(over));
    let fired = harness.pointer(&[released()], mouse::Cursor::Available(over));

    assert_eq!(fired, vec![Msg::Cancel], "release inside the box must fire");
}

#[test]
fn a_press_dragged_outside_the_button_does_not_fire() {
    let mut harness = armed_button("btn");
    let over = centre(harness.button_bounds());

    harness.pointer(&[moved(over)], mouse::Cursor::Available(over));
    harness.pointer(&[pressed()], mouse::Cursor::Available(over));
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
    let over = centre(harness.button_bounds());

    harness.pointer(&[pressed()], mouse::Cursor::Available(FAR_AWAY));
    let fired = harness.pointer(&[released()], mouse::Cursor::Available(over));

    assert!(
        fired.is_empty(),
        "a release may only complete a press this button actually took"
    );
}

#[test]
fn a_second_release_after_the_first_does_not_fire_again() {
    let mut harness = armed_button("btn");
    let over = centre(harness.button_bounds());

    harness.pointer(&[moved(over)], mouse::Cursor::Available(over));
    harness.pointer(&[pressed()], mouse::Cursor::Available(over));
    assert_eq!(
        harness.pointer(&[released()], mouse::Cursor::Available(over)),
        vec![Msg::Cancel]
    );
    assert!(
        harness
            .pointer(&[released()], mouse::Cursor::Available(over))
            .is_empty(),
        "one press commits exactly one message"
    );
}

// ---------------------------------------------------------------------------
// 4. Disabled is absence
// ---------------------------------------------------------------------------

#[test]
fn a_disabled_button_publishes_nothing_through_a_full_press_cycle() {
    let mut harness = Harness::new(Recorder::default(), || {
        Button::new(
            ButtonVariant::Primary,
            Element::new(Blank(CONTENT)),
            Msg::Cancel,
            "btn",
        )
        .disabled()
        .into()
    });
    let over = centre(harness.button_bounds());

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
    let mut harness = Harness::new(Recorder::default(), || {
        Button::new(
            ButtonVariant::Primary,
            Element::new(Blank(CONTENT)),
            Msg::Cancel,
            "btn",
        )
        .disabled()
        .into()
    });
    let over = centre(harness.button_bounds());
    let disabled_fill =
        Background::Color(expected_style(ButtonVariant::Primary, DISABLED).background);

    assert_eq!(harness.paint()[0].background, disabled_fill, "at rest");

    harness.pointer(&[moved(over)], mouse::Cursor::Available(over));
    assert_eq!(harness.paint()[0].background, disabled_fill, "hovered");

    harness.pointer(&[pressed()], mouse::Cursor::Available(over));
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
        harness.count().total,
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

    sync(&mut harness, &model);

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
            Element::new(Blank(CONTENT)),
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
    let bounds = harness.button_bounds();

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
    // G-9: §3.1 names `accent.primary` for Danger's ring, and indigo on
    // the hot red fill measures 1.1779:1 — an invisible focus indicator
    // at the moment a keyboard user most needs one. The widget must end up
    // painting the adaptive colour without ever mentioning Danger's ring.
    let mut harness = recorder_button(ButtonVariant::Danger, "btn", |b| b);
    let over = centre(harness.button_bounds());

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
        ring, palette.accent_primary,
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
        Palette::dark().accent_primary,
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
// 6. The Tab bridge
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
    sync(&mut harness, &model);

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
    sync(&mut harness, &model);
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

    sync(&mut harness, &model);
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
    assert_eq!(harness.count().total, 2);

    let (_group, mut model) = model_of(&["a", "b"]);
    for expected in ["a", "b", "a"] {
        model.focus_next();
        sync(&mut harness, &model);
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
    sync(&mut harness, &model);
    assert_eq!(harness.focused(), Some(iced_id("a")));

    model.clear();
    sync(&mut harness, &model);

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
    let over = centre(harness.button_bounds());
    harness.pointer(&[moved(over)], mouse::Cursor::Available(over));

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
    let over = centre(harness.button_bounds());

    harness.pointer(&[moved(over)], mouse::Cursor::Available(over));
    harness.pointer(&[pressed()], mouse::Cursor::Available(over));

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
    let over = centre(harness.button_bounds());
    harness.pointer(&[moved(over)], mouse::Cursor::Available(over));
    assert_eq!(
        harness.paint()[0].background,
        Background::Color(expected_style(ButtonVariant::Primary, HOVERED).background)
    );

    harness.set_view(|| Element::new(Blank(CONTENT)));
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
    let bounds = harness.button_bounds();

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
    let mut harness = Harness::new(Recorder::default(), || {
        Button::new(
            ButtonVariant::Ghost,
            Element::new(Blank(Size::new(1.0, 1.0))),
            Msg::Save,
            "btn",
        )
        .into()
    });
    let bounds = harness.button_bounds();

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

    assert_eq!(harness.button_bounds().height, button::HEIGHT_COMPACT);
}

#[test]
fn the_standard_builder_matches_the_default_height() {
    let mut compact_then_standard =
        recorder_button(ButtonVariant::Secondary, "btn", |b| b.compact().standard());
    let mut default_height = recorder_button(ButtonVariant::Secondary, "btn", |b| b);

    assert_eq!(
        compact_then_standard.button_bounds().height,
        default_height.button_bounds().height
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
