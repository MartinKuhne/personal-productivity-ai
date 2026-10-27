//! The shared headless harness every focus- and paint-level widget test runs
//! on (`INC-10`, `INC-11`, `INC-11b`).
//!
//! # Why this module exists
//!
//! `iced_test` 0.14 cannot drive focus at all — `Simulator` clicks, types
//! and snapshots, and there is nothing to click, type or snapshot with,
//! because iced has no widget-level focus event to simulate. Focus lives one
//! level down, in the widget-tree `Operation` protocol, so testing it means
//! assembling the parts the real runtime assembles by hand.
//!
//! `INC-10` assembled them once, `INC-11` copied them, and a third copy was
//! due with the menu. Two copies of a loop this subtle is tolerable; three is
//! where they start disagreeing and a test starts failing for a reason that
//! is not in the widget under test. So they are assembled here, once.
//!
//! # The five things a focus test cannot do without
//!
//! 1. **A headless renderer.** [`Harness::headless`] builds iced's real
//!    renderer with no window, so layout, text and the paint path are real.
//! 2. **A real `UserInterface::build` over a real widget tree.** Not a mock
//!    of one: [`FocusableState`](crate::widget::focusable::FocusableState)
//!    is only reachable through the tree the build produces.
//! 3. **A carried [`user_interface::Cache`].** The cache owns the widget
//!    `Tree`, and the `Tree` is where widget state lives. A fresh cache per
//!    call silently loses focus, and the failure reads as "the wrapper does
//!    not hold focus" rather than "the harness threw the state away".
//!    [`Harness`] carries it from one build to the next, which is exactly
//!    what a running application does across `view` calls.
//! 4. **The operation chain loop.** `operation::focusable::focus_next` is
//!    built as `then(count(), …)`, whose `finish` returns
//!    `Outcome::Chain(next)`. `UserInterface::operate` performs a single
//!    pass and stops; the real runtime re-runs the chained operation in
//!    `iced_winit`'s `Action::Widget` handler. A harness that skipped the
//!    loop would run only the `count` half of `focus_next` and report focus
//!    broken when the harness was. [`Harness::run`] owns the loop.
//! 5. **Read-back.** The tree is owned by the cache and the cache exposes
//!    nothing, so focus is read through operations — `find_focused`,
//!    `is_focused`, `count`, and [`RecordFocusables`], which captures the
//!    `(id, bounds)` pair of every widget the router visits and is how
//!    traversal *order* and layout transparency get checked.
//!
//! # Two renderers, one harness
//!
//! The harness is generic over the renderer rather than fixed to one, which
//! is what keeps "does it work" and "what exactly did it draw" from being
//! two harnesses:
//!
//! * [`HeadlessRenderer`] — iced's real renderer. Use it when the claim is
//!   that the widget lays out, registers, holds focus and paints through
//!   the actual paint path ([`Harness::render`]).
//! * [`Recorder`] — a renderer that records every `fill_quad` instead of
//!   rasterising it. Use it when the claim is a *position* claim, e.g.
//!   that a focus ring sits outside the box the fill covers by exactly the
//!   ring width. No snapshot of a rendered pixel grid says that without
//!   also freezing every glyph in the crate; [`Harness::paint`] says it
//!   directly.
//!
//! Paired with [`Blank`] — content that draws nothing and needs no text
//! renderer — the recording contains nothing but the widget's own layers.
//!
//! # Reading state back is a paint, not an accessor
//!
//! Hover, press and focus are read through what the widget *paints* rather
//! than through an accessor that would exist only for tests. That is the
//! stronger assertion: `state.hovered == true` is an internal fact; "this
//! frame painted the hover fill and the ring" is the behaviour the spec has
//! to hold.
//!
//! # The view is a closure, not an `Element`
//!
//! `Element` owns a `Box<dyn Widget>` and is not `Clone`, so the harness
//! takes a *view closure* and re-runs it for every step — rebuild-per-frame,
//! exactly what an application does. That makes "the tree changed
//! underneath" a [`set_view`](Harness::set_view) call rather than a special
//! case, and means anything that survives one step survives a `view` call.

use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::focus::GroupId;
use crate::widget::focusable::focusable;

use iced::Color;
use iced::Element;
use iced::Event;
use iced::Font;
use iced::Length;
use iced::Pixels;
use iced::Point;
use iced::Rectangle;
use iced::Size;
use iced::advanced::image;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::renderer::Headless as _;
use iced::advanced::widget::Operation;
use iced::advanced::widget::Tree;
use iced::advanced::widget::Widget;
use iced::advanced::widget::operation::{self, Outcome, black_box};
use iced::mouse;
use iced::widget::column;
use iced::widget::text;

use iced_test::core::Background;
use iced_test::core::Border;
use iced_test::core::Transformation;
use iced_test::core::clipboard::Null;
use iced_test::core::widget::Id;
use iced_test::futures::futures::executor::block_on;
use iced_test::renderer::Renderer as HeadlessRenderer;
use iced_test::runtime::UserInterface;
use iced_test::runtime::user_interface;

use std::cell::RefCell;

/// The theme every harness builds with.
pub type Theme = iced::Theme;

/// The viewport every harness lays its interface out in.
///
/// Big enough that a control and a point well outside it both fit, so a
/// test never has to reason about the viewport to know whether the cursor
/// was over the widget.
pub const VIEWPORT: Size = Size::new(400.0, 300.0);

/// The element type a harness view closure produces.
pub type El<'a, Message, Renderer> = Element<'a, Message, Theme, Renderer>;

// ---------------------------------------------------------------------------
// The harness
// ---------------------------------------------------------------------------

/// A headless iced interface whose widget state is carried across rebuilds
/// of whatever its view closure produces.
///
/// Construct one with [`Harness::headless`] (iced's real renderer) or
/// [`Harness::recording`] (a renderer that reports what it was asked to
/// draw). Every method rebuilds first, so every method observes the state a
/// running application would have kept rather than the state a previous
/// call left in a local variable.
pub struct Harness<Message, Renderer> {
    renderer: Renderer,
    cache: user_interface::Cache,
    view: Box<dyn FnMut() -> El<'static, Message, Renderer>>,
}

impl<Message: Send + 'static, Renderer: renderer::Renderer> Harness<Message, Renderer> {
    /// Wrap `renderer` around a view closure, starting from an empty cache.
    pub fn new(
        renderer: Renderer,
        view: impl Fn() -> El<'static, Message, Renderer> + 'static,
    ) -> Self {
        Self {
            renderer,
            cache: user_interface::Cache::default(),
            view: Box::new(view),
        }
    }

    /// Swap the tree the harness rebuilds on the next step.
    pub fn set_view(&mut self, view: impl Fn() -> El<'static, Message, Renderer> + 'static) {
        self.view = Box::new(view);
    }

    /// Rebuild the current view over the carried cache.
    ///
    /// Every step starts here, so every step is a rebuild.
    fn begin(&mut self) -> UserInterface<'static, Message, Theme, Renderer> {
        let root = (self.view)();
        let cache = std::mem::take(&mut self.cache);

        UserInterface::build(root, VIEWPORT, cache, &mut self.renderer)
    }

    /// Carry a built interface's widget state back out for the next step.
    fn commit(&mut self, ui: UserInterface<'static, Message, Theme, Renderer>) {
        self.cache = ui.into_cache();
    }

    /// Run one operation — and its whole chain — against a freshly rebuilt
    /// interface, then carry the widget state back out.
    ///
    /// The chain loop mirrors `iced_winit`'s `Action::Widget` handler.
    /// Without it `operation::focusable::focus_next` would run only its
    /// `count` half, and a test would conclude that focus is broken when the
    /// harness was.
    pub fn run<T>(&mut self, mut operation: Box<dyn Operation<T>>) -> Option<T> {
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

    // -- focus read-back ---------------------------------------------------

    /// Which id the router reports as focused, if any.
    pub fn focused(&mut self) -> Option<Id> {
        self.run(Box::new(operation::focusable::find_focused()))
    }

    /// Whether the widget with `id` reports itself focused.
    pub fn is_focused(&mut self, id: &Id) -> bool {
        self.run(Box::new(operation::focusable::is_focused(id.clone())))
            .unwrap_or(false)
    }

    /// How many widgets the router sees, and which index holds focus.
    pub fn focus_count(&mut self) -> operation::focusable::Count {
        self.run(Box::new(operation::focusable::count()))
            .unwrap_or_default()
    }

    /// The ids the router sees, in traversal order.
    pub fn registered(&mut self) -> Vec<Option<Id>> {
        self.recorded()
            .into_iter()
            .map(|(id, _bounds)| id)
            .collect()
    }

    /// The `(id, bounds)` pairs the router sees, in traversal order.
    pub fn recorded(&mut self) -> Vec<(Option<Id>, Rectangle)> {
        self.run(Box::<RecordFocusables>::default())
            .unwrap_or_default()
    }

    // -- focus writes ------------------------------------------------------

    /// Ask the router to focus `id` directly.
    ///
    /// A test-level write. Production code pushes focus through
    /// [`crate::bridge`], and a test that wants to exercise the bridge uses
    /// [`sync`](Self::sync) or [`tab`](Self::tab) instead of this.
    pub fn focus(&mut self, id: &Id) {
        self.run(Box::new(operation::focusable::focus::<Message>(id.clone())));
    }

    /// Ask the router to drop focus.
    pub fn unfocus(&mut self) {
        self.run(Box::new(operation::focusable::unfocus::<Message>()));
    }

    /// Ask iced's *own* router to move focus to the next widget it sees.
    ///
    /// Exists only so a test can pin that it **loses** focus at the ends
    /// rather than wrapping — the fact `INC-10` constraint 1 rests on. No
    /// production path may call this; the bridge computes its target from
    /// the model instead.
    pub fn iced_focus_next(&mut self) {
        self.run(Box::new(operation::focusable::focus_next::<Message>()));
    }

    /// The mirror of [`iced_focus_next`](Self::iced_focus_next), same
    /// pin-only purpose.
    pub fn iced_focus_previous(&mut self) {
        self.run(Box::new(operation::focusable::focus_previous::<Message>()));
    }

    /// Push the model's current focus into the router through the
    /// production bridge.
    ///
    /// This is the assertion every focus test is really making: after the
    /// host's transition, the router and [`FocusState`] name the same
    /// control. Going through [`crate::bridge::focus_operation`] rather
    /// than a hand-rolled `focus(id)` means the test covers the bridge as
    /// well as the widget.
    pub fn sync(&mut self, model: &FocusState) {
        self.run(crate::bridge::focus_operation::<Message>(model));
    }

    /// Drive one `Tab` / `Shift+Tab` through the production bridge: the
    /// model computes the target, the router is pushed to it.
    ///
    /// Returns what the bridge pushed — the model's focus after the step.
    /// A step the model refuses leaves it where it was, which is the point:
    /// the harness does not compute the move itself, so a test cannot drift
    /// from what the bridge actually did.
    pub fn tab(
        &mut self,
        model: &mut FocusState,
        direction: crate::bridge::Direction,
    ) -> Option<FocusId> {
        self.run(crate::bridge::tab_operation::<Message>(model, direction));

        model.current().cloned()
    }

    // -- pointer ----------------------------------------------------------

    /// Deliver pointer events with the cursor where the test says, and
    /// report what the tree published.
    ///
    /// One call is one frame: the tree is rebuilt, the events delivered,
    /// the state carried back out.
    ///
    /// The cursor is passed separately from the events because iced reads
    /// the *current* cursor, not the event's position — which is how a
    /// test gets a `CursorMoved` whose position the tree is told to ignore.
    pub fn pointer(&mut self, events: &[Event], cursor: mouse::Cursor) -> Vec<Message> {
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

    /// Put the cursor on `at` and let the tree see it there.
    pub fn hover(&mut self, at: Point) -> Vec<Message> {
        self.pointer(&[moved(at)], mouse::Cursor::Available(at))
    }

    /// Press the left button with the cursor on `at`.
    pub fn press(&mut self, at: Point) -> Vec<Message> {
        self.pointer(&[pressed()], mouse::Cursor::Available(at))
    }

    /// Release the left button with the cursor on `at`.
    pub fn release(&mut self, at: Point) -> Vec<Message> {
        self.pointer(&[released()], mouse::Cursor::Available(at))
    }
}

// ---------------------------------------------------------------------------
// The real-renderer leg
// ---------------------------------------------------------------------------

impl<Message: Send + 'static> Harness<Message, HeadlessRenderer> {
    /// A harness over iced's real headless renderer.
    ///
    /// Use this when the claim is that the widget works through the actual
    /// paint path — real quads, real text, real layer calls — rather than
    /// when the claim is about a specific colour or rectangle.
    #[must_use]
    pub fn headless(view: impl Fn() -> El<'static, Message, HeadlessRenderer> + 'static) -> Self {
        let renderer = block_on(<HeadlessRenderer as renderer::Headless>::new(
            Font::with_name("Fira Sans"),
            Pixels(16.0),
            None,
        ))
        .expect("a headless renderer should be available for hoarfrost widget tests");

        Self::new(renderer, view)
    }

    /// Run a real draw pass over the current tree.
    ///
    /// Nothing is asserted about pixels: the claim is that
    /// [`Widget::draw`] runs to completion against a real renderer, in
    /// whatever state the tree is currently in.
    pub fn render(&mut self) {
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

// ---------------------------------------------------------------------------
// The recording leg
// ---------------------------------------------------------------------------

impl<Message: Send + 'static> Harness<Message, Recorder> {
    /// A harness over a renderer that records what it is told to draw.
    #[must_use]
    pub fn recording(view: impl Fn() -> El<'static, Message, Recorder> + 'static) -> Self {
        Self::new(Recorder::default(), view)
    }

    /// Paint the current tree and report every quad the renderer was asked
    /// to fill, in the order it was asked.
    ///
    /// A paint is a rebuild, so this reads the state that survived rather
    /// than the state a previous step left in a local variable.
    pub fn paint(&mut self) -> Vec<Paint> {
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

    /// The bounds of the first layer the tree painted.
    ///
    /// For a widget whose own box is its first layer — a button, a menu
    /// row — this is the box it laid out to, which is what a test needs
    /// before it can put the cursor in the middle of it.
    ///
    /// # Panics
    /// Panics when the tree painted nothing at all, which means the widget
    /// under test drew no box.
    pub fn first_bounds(&mut self) -> Rectangle {
        self.paint()
            .first()
            .copied()
            .unwrap_or_else(|| panic!("the widget under test must paint its own box"))
            .bounds
    }
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// An operation that records every focusable widget the router visits.
///
/// `Send` holds because the recording sits behind a `RefCell`, which is
/// `Send` for a `Send` payload, and the operation never leaves its thread.
#[derive(Default)]
pub struct RecordFocusables {
    seen: RefCell<Vec<(Option<Id>, Rectangle)>>,
}

/// What [`RecordFocusables`] collects.
pub type Recorded = Vec<(Option<Id>, Rectangle)>;

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
// A renderer that reports instead of rasterising
// ---------------------------------------------------------------------------

/// One `fill_quad` a widget asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    /// The rectangle the quad covered.
    pub bounds: Rectangle,
    /// What was painted inside it.
    pub background: Background,
    /// The stroke drawn on its inside edge.
    pub border: Border,
}

/// A renderer that records what it is told to draw.
///
/// Implements only the base [`renderer::Renderer`] protocol, which is all
/// a framework widget's own layers need: paired with [`Blank`] content, no
/// text renderer is reachable and the recording contains nothing but the
/// widget under test.
#[derive(Default)]
pub struct Recorder {
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
/// Lets the recording leg see only the widget's own paint.
pub struct Blank(Size);

impl Blank {
    /// Blank content laid out at exactly `size`.
    #[must_use]
    pub const fn new(size: Size) -> Self {
        Self(size)
    }
}

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

/// The iced [`Id`] a label maps to, matching [`crate::bridge::widget_id`].
#[must_use]
pub fn iced_id(label: &str) -> Id {
    Id::from(label.to_owned())
}

/// Several labels as iced ids, in order.
#[must_use]
pub fn iced_ids(labels: &[&'static str]) -> Vec<Id> {
    labels.iter().map(|label| iced_id(label)).collect()
}

/// The same ids as the router reports them: a wrapper hands the router an
/// `Option<&Id>`, so a named widget shows up as `Some(id)`.
#[must_use]
pub fn opt_ids(labels: &[&'static str]) -> Vec<Option<Id>> {
    labels.iter().map(|label| Some(iced_id(label))).collect()
}

/// A model holding `labels` as one enabled group named `panel`, in
/// traversal order.
#[must_use]
pub fn model_of(labels: &[&'static str]) -> (GroupId, FocusState) {
    model_with(
        "panel",
        labels
            .iter()
            .map(|label| FocusMember::new(*label))
            .collect(),
    )
}

/// A model holding `members` as one group named `group`, in traversal order.
#[must_use]
pub fn model_with(group: &str, members: Vec<FocusMember>) -> (GroupId, FocusState) {
    let group = GroupId::new(group);
    let mut model = FocusState::new();
    model.register_group(group.clone(), members);

    (group, model)
}

/// A focusable wrapper around a label, as a host would build one.
///
/// The label is owned so the element it produces is `'static` and the
/// harness can rebuild it from a closure on every step.
#[must_use]
pub fn wrapped(label: &str, enabled: bool) -> El<'static, (), HeadlessRenderer> {
    focusable(text(label.to_owned()), FocusId::new(label), enabled)
}

/// A column of enabled focusable wrappers, in `labels` order.
#[must_use]
pub fn focusable_tree(labels: &[&'static str]) -> El<'static, (), HeadlessRenderer> {
    column(labels.iter().map(|label| wrapped(label, true))).into()
}

/// A column of focusable members carrying their own enabled flags.
#[must_use]
pub fn mixed_focusable_tree(entries: &[(&'static str, bool)]) -> El<'static, (), HeadlessRenderer> {
    column(
        entries
            .iter()
            .map(|(label, enabled)| wrapped(label, *enabled)),
    )
    .into()
}

/// Assert that the router and the model name the same control.
///
/// The single assertion the whole focus stack is built to make: the model
/// decided, the bridge pushed, the widget holds it.
pub fn assert_agree<Message: Send + 'static, Renderer: renderer::Renderer>(
    harness: &mut Harness<Message, Renderer>,
    model: &FocusState,
    step: &str,
) {
    let expected = model.current().map(|id| iced_id(id.as_str()));

    assert_eq!(
        harness.focused(),
        expected,
        "after {step}: the router and `FocusState` must name the same control"
    );
}

/// Focus `label` through a one-member model, the way a host would.
pub fn focus_one<Message: Send + 'static, Renderer: renderer::Renderer>(
    harness: &mut Harness<Message, Renderer>,
    label: &str,
) {
    let mut model = FocusState::new();
    model.register_group(GroupId::new("panel"), vec![FocusMember::new(label)]);
    assert!(
        model.focus(&FocusId::new(label)),
        "a freshly registered, enabled member must be focusable"
    );

    harness.sync(&model);
}

/// A `CursorMoved` event to `to`.
#[must_use]
pub fn moved(to: Point) -> Event {
    Event::Mouse(mouse::Event::CursorMoved { position: to })
}

/// A left-button press where the cursor already is.
#[must_use]
pub fn pressed() -> Event {
    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
}

/// A left-button release where the cursor already is.
#[must_use]
pub fn released() -> Event {
    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
}

/// The centre of a rectangle — the safest point a test can put the cursor
/// on to be inside it.
#[must_use]
pub fn centre(bounds: Rectangle) -> Point {
    Point::new(
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    )
}

// ---------------------------------------------------------------------------
// The pixel leg
// ---------------------------------------------------------------------------

/// The scale every pixel-level assertion renders at.
///
/// Two physical pixels per logical one, so a glyph's landing point is
/// resolved to half a logical pixel rather than being rounded away.
pub const PIXEL_SCALE: f32 = 2.0;

/// A channel value above which a pixel counts as ink.
///
/// Well above the darkest token fill this crate paints and well below any
/// text colour it resolves, so a background never reads as a glyph.
pub const INK_THRESHOLD: u32 = 90;

/// A headless renderer built the way every paint-level test wants it.
///
/// Split out from [`Harness::headless`] so a test that needs raw pixels
/// rather than a stateful interface can render one frame without one.
#[must_use]
pub fn headless_renderer() -> HeadlessRenderer {
    block_on(<HeadlessRenderer as renderer::Headless>::new(
        Font::with_name("Fira Sans"),
        Pixels(16.0),
        None,
    ))
    .expect("a headless renderer should be available for hoarfrost widget tests")
}

/// Draw `root` once at `size` and return the RGBA framebuffer.
///
/// Bounds cannot answer where a glyph was actually painted. A text widget
/// can lay out at exactly the size its caller asked for and still put its
/// ink outside that box — which is precisely how the `line_height`
/// unit-confusion bug hid behind a fixed-height slot. Only the pixels can
/// tell those two cases apart.
#[must_use]
pub fn screenshot<Message: 'static>(
    size: Size,
    scale: f32,
    background: Color,
    root: Element<'static, Message>,
) -> Vec<u8> {
    let mut renderer = headless_renderer();

    let mut ui: UserInterface<'static, Message, Theme, HeadlessRenderer> =
        UserInterface::build(root, size, user_interface::Cache::default(), &mut renderer);

    ui.draw(
        &mut renderer,
        &Theme::Dark,
        &renderer::Style {
            text_color: Color::WHITE,
        },
        mouse::Cursor::Unavailable,
    );

    let physical = Size::new(
        (size.width * scale).round() as u32,
        (size.height * scale).round() as u32,
    );
    renderer.screenshot(physical, scale, background)
}

/// The vertical band of ink between logical `y_min` and `y_max`, in
/// **logical** pixels.
///
/// `None` when the window carries no ink at all — which is what a widget
/// that clips its own text looks like from the outside. The windowed form
/// is what lets a test ask "did *this* row paint?" rather than "did
/// anything on the frame paint?", which matters because one populated row
/// can otherwise hide an empty neighbour.
#[must_use]
pub fn ink_band_between(
    rgba: &[u8],
    width: u32,
    scale: f32,
    y_min: f32,
    y_max: f32,
) -> Option<(f32, f32)> {
    let height = (rgba.len() as u64 / 4 / u64::from(width)) as u32;
    let top = ((y_min * scale).floor() as i64).clamp(0, height as i64) as u32;
    let bottom = ((y_max * scale).ceil() as i64).clamp(0, height as i64) as u32;

    let mut first = u32::MAX;
    let mut last = 0u32;

    for y in top..bottom {
        for x in 0..width {
            let i = ((y * width + x) * 4) as usize;
            if i + 3 >= rgba.len() {
                continue;
            }
            let peak = rgba[i].max(rgba[i + 1]).max(rgba[i + 2]) as u32;
            if peak > INK_THRESHOLD {
                first = first.min(y);
                last = last.max(y);
            }
        }
    }

    if first == u32::MAX || last < first {
        return None;
    }

    Some((first as f32 / scale, (last + 1) as f32 / scale))
}

/// The vertical band of ink in a framebuffer, in **logical** pixels.
///
/// Shorthand for [`ink_band_between`] over the whole frame.
#[must_use]
pub fn ink_band(rgba: &[u8], physical_width: u32, scale: f32) -> Option<(f32, f32)> {
    ink_band_between(rgba, physical_width, scale, f32::MIN, f32::MAX)
}

/// The physical width a framebuffer of `size` was rendered at.
#[must_use]
pub fn physical_width(size: Size, scale: f32) -> u32 {
    (size.width * scale).round() as u32
}
