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
//!
//! # The frame leg
//!
//! A focus probe says where a widget *claims* to be. It cannot say whether
//! anything was drawn there, nor whether a state change actually moved the
//! picture. [`Capture`] rasterises through one headless renderer many times
//! and hands back a [`Frame`], which reads the framebuffer in logical
//! coordinates: how much ink a box holds ([`Frame::ink_count`]), *where*
//! that ink landed ([`Frame::ink_bounds`]), whether a region is one flat
//! colour ([`Frame::is_uniform`]) and — the reading that catches a control
//! whose model moved while its pixels stayed dead — how many pixels two
//! frames disagree on ([`Frame::differing_pixels`], asserted by
//! [`assert_repainted`]).
//!
//! Building that renderer costs roughly 620 ms in a debug build, so a test
//! that renders more than one frame goes through [`Capture`] rather than
//! minting a renderer per assertion. A session is **owned** by the test
//! that builds it and dropped with it. `cargo nextest` runs every test in
//! its own process, so a session could never be reused across tests in the
//! first place; there is nothing to amortise, and therefore no reason for
//! one to outlive the test that paid for it.
//!
//! Unit tests live in the sibling `test_helpers_tests.rs` sidecar.

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
use iced::Vector;
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

use std::any::Any;
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
///
/// # The `'a` on the view: borrowing, not leaking
///
/// The view closure returns `El<'a, ..>`, so the element it builds may
/// borrow from data the closure captured — a model sitting on the test's own
/// stack. Before this lifetime existed the closure had to return
/// `El<'static, ..>`, which forced every controlled widget's model to be
/// `Box::leak`ed into the process just to satisfy the bound. Six helpers
/// across this crate's tests did exactly that.
///
/// Call sites that genuinely build a `'static` element are unaffected:
/// `Element<'a>` is covariant in `'a` (it is a `Box<dyn Widget<..> + 'a>`),
/// so a closure returning `El<'static, ..>` satisfies `Fn() -> El<'a, ..>`
/// for any `'a`, and a bare `Harness::headless(|| ...)` over owned data
/// simply infers `'a = 'static`.
///
/// # The invariance trap — do not remove the annotations
///
/// `dyn Fn() -> T` is **invariant** in `T`: `Output` is an associated type,
/// and associated-type bindings on a trait object are not covariant. So
/// `Box<dyn FnMut() -> El<'static, ..>>` is *not* a
/// `Box<dyn FnMut() -> El<'a, ..>>`, and a `Harness<'static, ..>` is not a
/// `Harness<'a, ..>`.
///
/// That is why the lifetime is spelled out on the field, on every
/// constructor's `impl Fn() -> El<'a, ..> + 'a` bound, and on
/// [`begin`](Self::begin)/[`commit`](Self::commit) rather than being left to
/// elision. Drop any one of those annotations and the compiler rewrites the
/// bound back to `'static`, the borrowed-model closures stop coercing, and
/// the error lands far from its cause — it reads as "closure outlives its
/// capture" inside `UserInterface::build` rather than "this harness was
/// pinned to `'static`".
pub struct Harness<'a, Message, Renderer> {
    renderer: Renderer,
    cache: user_interface::Cache,
    view: Box<dyn FnMut() -> El<'a, Message, Renderer> + 'a>,
}

impl<'a, Message: Send + 'static, Renderer: renderer::Renderer> Harness<'a, Message, Renderer> {
    /// Wrap `renderer` around a view closure, starting from an empty cache.
    ///
    /// The `'a` on the closure's return type is what lets the element borrow
    /// from the closure's captures; see the [type-level note](Harness#the-a-on-the-view-borrowing-not-leaking).
    pub fn new(renderer: Renderer, view: impl Fn() -> El<'a, Message, Renderer> + 'a) -> Self {
        Self {
            renderer,
            cache: user_interface::Cache::default(),
            view: Box::new(view),
        }
    }

    /// Swap the tree the harness rebuilds on the next step.
    pub fn set_view(&mut self, view: impl Fn() -> El<'a, Message, Renderer> + 'a) {
        self.view = Box::new(view);
    }

    /// Rebuild the current view over the carried cache.
    ///
    /// Every step starts here, so every step is a rebuild.
    ///
    /// The returned interface borrows for `'a`, not for the call: the
    /// element the closure produced is good for `'a` no matter how briefly
    /// `self` was borrowed to build it.
    fn begin(&mut self) -> UserInterface<'a, Message, Theme, Renderer> {
        let root = (self.view)();
        let cache = std::mem::take(&mut self.cache);

        UserInterface::build(root, VIEWPORT, cache, &mut self.renderer)
    }

    /// Carry a built interface's widget state back out for the next step.
    fn commit(&mut self, ui: UserInterface<'a, Message, Theme, Renderer>) {
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

    /// Every candidate the tree publishes, tagged with the hook that
    /// produced it.
    ///
    /// The read-back for claims about *identity* rather than focus: what
    /// boxes a host can see, what names they carry, and whether a control
    /// that is not focusable still reports where it is.
    pub fn candidates(&mut self) -> Vec<Candidate> {
        self.run(Box::<RecordCandidates>::default())
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

impl<'a, Message: Send + 'static> Harness<'a, Message, HeadlessRenderer> {
    /// A harness over iced's real headless renderer.
    ///
    /// Use this when the claim is that the widget works through the actual
    /// paint path — real quads, real text, real layer calls — rather than
    /// when the claim is about a specific colour or rectangle.
    #[must_use]
    pub fn headless(view: impl Fn() -> El<'a, Message, HeadlessRenderer> + 'a) -> Self {
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

impl<'a, Message: Send + 'static> Harness<'a, Message, Recorder> {
    /// A harness over a renderer that records what it is told to draw.
    #[must_use]
    pub fn recording(view: impl Fn() -> El<'a, Message, Recorder> + 'a) -> Self {
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

/// One record a widget pushed into the operation stream.
///
/// `kind` is the hook that produced it, so a test can tell *how* a widget
/// became visible to a host and not merely *that* it did.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// The hook: `container`, `scrollable`, `focusable`, `text_input`,
    /// `text` or `custom`.
    pub kind: &'static str,
    /// The id it was published under, if the widget carried a stable one.
    pub id: Option<Id>,
    /// The box the widget reported with it.
    pub bounds: Rectangle,
}

/// The kind label for a box published through [`Operation::container`].
pub const KIND_CONTAINER: &str = "container";
/// The kind label for a box published through [`Operation::scrollable`].
pub const KIND_SCROLLABLE: &str = "scrollable";
/// The kind label for a box published through [`Operation::focusable`].
pub const KIND_FOCUSABLE: &str = "focusable";
/// The kind label for a run of text published through [`Operation::text`].
pub const KIND_TEXT: &str = "text";
/// The kind label for a box published through [`Operation::text_input`].
pub const KIND_TEXT_INPUT: &str = "text_input";
/// The kind label for a box published through [`Operation::custom`].
pub const KIND_CUSTOM: &str = "custom";

/// Every hook [`Operation`] publishes a widget through, in the order the
/// trait declares them.
///
/// The completeness checklist for [`RecordCandidates`]: a kind missing
/// from a recorded tree is not a widget that stayed quiet, it is a door the
/// recorder never opened.
pub const ALL_KINDS: [&str; 6] = [
    KIND_CONTAINER,
    KIND_SCROLLABLE,
    KIND_FOCUSABLE,
    KIND_TEXT_INPUT,
    KIND_TEXT,
    KIND_CUSTOM,
];

/// An operation that records every candidate a widget registers, tagged
/// with the hook that published it.
///
/// [`RecordFocusables`] answers "can Tab stop here?". This answers the
/// wider question a host addresses: which boxes and names the operation
/// stream carries, and through which door each arrived. Keeping `kind`
/// in the record is what lets a test assert that a control published its
/// own box as a `container` **without** also appearing as a `focusable`
/// — the exact distinction a disabled control turns on, and one a
/// focus-only probe cannot see at all.
///
/// It implements **every** publishing hook [`Operation`] declares —
/// `container`, `scrollable`, `focusable`, `text_input`, `text` and
/// `custom`. That matters because the hooks default to empty methods: a
/// recorder that omits one does not fail, it silently reports nothing for
/// that door, and every test built on it goes blind to exactly the widgets
/// that arrived through it.
#[derive(Default)]
pub struct RecordCandidates {
    seen: RefCell<Vec<Candidate>>,
}

impl RecordCandidates {
    /// Every record collected so far, in traversal order.
    #[must_use]
    pub fn candidates(&self) -> Vec<Candidate> {
        self.seen.borrow().clone()
    }

    fn record(&mut self, kind: &'static str, id: Option<&Id>, bounds: Rectangle) {
        self.seen.borrow_mut().push(Candidate {
            kind,
            id: id.cloned(),
            bounds,
        });
    }
}

impl Operation<Vec<Candidate>> for RecordCandidates {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Vec<Candidate>>)) {
        operate(self);
    }

    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        self.record(KIND_CONTAINER, id, bounds);
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        _content_bounds: Rectangle,
        _translation: Vector,
        _state: &mut dyn operation::Scrollable,
    ) {
        self.record(KIND_SCROLLABLE, id, bounds);
    }

    fn focusable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        _state: &mut dyn operation::Focusable,
    ) {
        self.record(KIND_FOCUSABLE, id, bounds);
    }

    fn text(&mut self, id: Option<&Id>, bounds: Rectangle, _text: &str) {
        self.record(KIND_TEXT, id, bounds);
    }

    fn text_input(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        _state: &mut dyn operation::TextInput,
    ) {
        self.record(KIND_TEXT_INPUT, id, bounds);
    }

    fn custom(&mut self, id: Option<&Id>, bounds: Rectangle, _state: &mut dyn Any) {
        self.record(KIND_CUSTOM, id, bounds);
    }

    fn finish(&self) -> Outcome<Vec<Candidate>> {
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
    harness: &mut Harness<'_, Message, Renderer>,
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
    harness: &mut Harness<'_, Message, Renderer>,
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
///
/// The element borrows: a controlled widget may hold a model that lives on
/// the caller's stack. Callers holding a `'static` element are unaffected,
/// because `Element` is covariant in its lifetime.
#[must_use]
pub fn screenshot<Message: 'static>(
    size: Size,
    scale: f32,
    background: Color,
    root: Element<'_, Message>,
) -> Vec<u8> {
    let mut renderer = headless_renderer();

    let mut ui: UserInterface<'_, Message, Theme, HeadlessRenderer> =
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

// ---------------------------------------------------------------------------
// The frame leg — one renderer, many frames
// ---------------------------------------------------------------------------

/// How far a channel may drift from a reference surface before a pixel
/// counts as painted over it.
///
/// Four is above the rounding noise a `f32 → u8` colour conversion and
/// the renderer's blending introduce, and far below the smallest
/// deliberate step in this crate's palette, so it separates "something
/// was painted" from "the surface underneath was merely re-emitted".
pub const SURFACE_TOLERANCE: u8 = 4;

/// The fewest pixels a state change must move for a repaint to count as
/// having happened.
///
/// Eight is below the smallest honest repaint — flipping a 16px checkbox
/// mark moves hundreds — and above the handful of pixels antialiasing
/// might shuffle on an unrelated edge, so a control that repaints nothing
/// cannot clear it by accident.
pub const MIN_CHANGED_PIXELS: usize = 8;

/// The surface a captured frame is composited onto.
///
/// Black is the darkest surface available, so every fill and every glyph
/// this crate paints differs from it by more than [`SURFACE_TOLERANCE`].
const CAPTURE_BACKGROUND: Color = Color::BLACK;

/// A headless capture session: one renderer, many frames.
///
/// Building iced's headless renderer costs roughly 620 ms in a debug
/// build — more than a layout pass and a raster together — so a test that
/// renders more than one frame should mint the renderer once and drive it
/// through here. [`screenshot`] is the one-frame shorthand; this is the
/// many-frame form.
///
/// Unlike [`screenshot`] it takes a **borrowing** element, so a controlled
/// widget can hold a model that lives on the test's own stack instead of
/// being leaked into a `static` to satisfy a `'static` bound.
///
/// # Frames do not share widget state
///
/// Every frame builds a fresh [`user_interface::Cache`], so nothing
/// travels from one captured frame to the next: a frame is a picture of
/// *this element* as it would look after *these events*. A test that wants
/// a stateful session across steps uses [`Harness`], which carries its
/// cache on purpose. What a repaint test needs — the same widget with a
/// different model — arrives through the element itself, so the fresh
/// cache costs nothing and keeps two frames independent of each other.
///
/// # Ownership
///
/// A session is built by the test that uses it and dropped when that test
/// returns. There is deliberately no shared, process-wide session: under
/// `cargo nextest` every test is a fresh process, so a session parked in a
/// thread-local would be constructed once and thrown away once — exactly
/// what a local `Capture::new()` already does, with a leak attached.
/// Dropping a locally-owned wgpu renderer inside a test body is proven
/// clean by every [`Capture::new`] call site in this crate's tests.
pub struct Capture {
    renderer: HeadlessRenderer,
}

impl Default for Capture {
    fn default() -> Self {
        Self::new()
    }
}

impl Capture {
    /// Build a session, paying the headless-renderer cost once.
    #[must_use]
    pub fn new() -> Self {
        Self {
            renderer: headless_renderer(),
        }
    }

    /// Rasterise `root` at `viewport`, at [`PIXEL_SCALE`] on black.
    #[must_use]
    pub fn frame<Message: 'static>(&mut self, viewport: Size, root: Element<'_, Message>) -> Frame {
        self.frame_at(viewport, PIXEL_SCALE, CAPTURE_BACKGROUND, root)
    }

    /// Rasterise `root` at `viewport` with the scale and background spelled out.
    #[must_use]
    pub fn frame_at<Message: 'static>(
        &mut self,
        viewport: Size,
        scale: f32,
        background: Color,
        root: Element<'_, Message>,
    ) -> Frame {
        self.render(
            viewport,
            scale,
            background,
            root,
            &[],
            mouse::Cursor::Unavailable,
        )
    }

    /// Rasterise `root` after delivering `events` with the cursor parked
    /// at the viewport centre.
    #[must_use]
    pub fn frame_after<Message: 'static>(
        &mut self,
        viewport: Size,
        root: Element<'_, Message>,
        events: &[Event],
    ) -> Frame {
        let cursor =
            mouse::Cursor::Available(Point::new(viewport.width / 2.0, viewport.height / 2.0));

        self.frame_after_at(viewport, root, events, cursor)
    }

    /// [`frame_after`](Self::frame_after) with the cursor where the test says.
    ///
    /// The cursor is a parameter because iced reads the *current* cursor
    /// rather than each event's position: a hover repaint only shows up if
    /// the tree is told the pointer is over the widget.
    #[must_use]
    pub fn frame_after_at<Message: 'static>(
        &mut self,
        viewport: Size,
        root: Element<'_, Message>,
        events: &[Event],
        cursor: mouse::Cursor,
    ) -> Frame {
        self.render(
            viewport,
            PIXEL_SCALE,
            CAPTURE_BACKGROUND,
            root,
            events,
            cursor,
        )
    }

    /// Deliver a move, a press and a release at `at` and report what the
    /// tree published.
    ///
    /// Press and release go in separate `update` calls because that is how
    /// a real pointer arrives: the tree sees the button go down on one
    /// frame and come up on the next.
    #[must_use]
    pub fn click<Message: 'static>(
        &mut self,
        root: Element<'_, Message>,
        at: Point,
    ) -> Vec<Message> {
        let mut ui: UserInterface<'_, Message, Theme, HeadlessRenderer> = UserInterface::build(
            root,
            VIEWPORT,
            user_interface::Cache::default(),
            &mut self.renderer,
        );
        let mut clipboard = Null;
        let mut messages = Vec::new();
        let cursor = mouse::Cursor::Available(at);

        ui.update(
            &[moved(at), pressed()],
            cursor,
            &mut self.renderer,
            &mut clipboard,
            &mut messages,
        );
        ui.update(
            &[released()],
            cursor,
            &mut self.renderer,
            &mut clipboard,
            &mut messages,
        );

        messages
    }

    /// The one draw path every capture goes through.
    fn render<Message: 'static>(
        &mut self,
        viewport: Size,
        scale: f32,
        background: Color,
        root: Element<'_, Message>,
        events: &[Event],
        cursor: mouse::Cursor,
    ) -> Frame {
        let mut ui: UserInterface<'_, Message, Theme, HeadlessRenderer> = UserInterface::build(
            root,
            viewport,
            user_interface::Cache::default(),
            &mut self.renderer,
        );

        if !events.is_empty() {
            let mut clipboard = Null;
            let mut messages = Vec::new();
            ui.update(
                events,
                cursor,
                &mut self.renderer,
                &mut clipboard,
                &mut messages,
            );
        }

        ui.draw(
            &mut self.renderer,
            &Theme::Dark,
            &renderer::Style {
                text_color: Color::WHITE,
            },
            mouse::Cursor::Unavailable,
        );

        let physical = Size::new(
            (viewport.width * scale).round() as u32,
            (viewport.height * scale).round() as u32,
        );

        Frame::new(
            self.renderer.screenshot(physical, scale, background),
            viewport,
            scale,
        )
    }
}

/// One rasterised frame, read in **logical** coordinates.
///
/// Every reading is absolute. Nothing here is compared against a stored
/// golden, so a frame asserts a property rather than reproducing an
/// artefact of the machine that drew a reference file.
///
/// The several readings exist because several different defects are being
/// hunted: [`ink_count`](Self::ink_count) asks whether anything a user
/// could read was drawn, [`painted_count`](Self::painted_count) asks
/// whether a dim fill covers a surface, [`ink_bounds`](Self::ink_bounds)
/// asks *where* the mark landed, and [`differing_pixels`](Self::differing_pixels)
/// asks whether a state change actually moved anything.
pub struct Frame {
    rgba: Vec<u8>,
    viewport: Size,
    scale: f32,
    width_px: u32,
    height_px: u32,
}

impl Frame {
    /// Wrap an RGBA framebuffer captured at `viewport` and `scale`.
    ///
    /// Lets a framebuffer from [`screenshot`] be read with the same
    /// readers rather than re-derived by every caller.
    #[must_use]
    pub fn new(rgba: Vec<u8>, viewport: Size, scale: f32) -> Self {
        let width_px = physical_width(viewport, scale);
        let height_px = if width_px == 0 {
            0
        } else {
            (rgba.len() / (4 * width_px as usize)) as u32
        };

        Self {
            rgba,
            viewport,
            scale,
            width_px,
            height_px,
        }
    }

    /// The viewport this frame was captured at.
    #[must_use]
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// The physical size this frame was rasterised at.
    #[must_use]
    pub fn physical_size(&self) -> Size<u32> {
        Size::new(self.width_px, self.height_px)
    }

    /// The RGB channels painted at logical point `(x, y)`.
    ///
    /// # Panics
    /// Panics when `(x, y)` falls outside the captured frame.
    #[must_use]
    pub fn pixel(&self, x: f32, y: f32) -> [u8; 3] {
        let index = self.index(x, y);
        assert!(
            index + 2 < self.rgba.len(),
            "sample ({x}, {y}) falls outside the frame"
        );

        [self.rgba[index], self.rgba[index + 1], self.rgba[index + 2]]
    }

    /// How many pixels inside `rect` clear [`INK_THRESHOLD`] on some channel.
    #[must_use]
    pub fn ink_count(&self, rect: Rectangle) -> usize {
        self.count_where(rect, |pixel| is_bright(&pixel))
    }

    /// Whether any ink lands inside `rect`.
    #[must_use]
    pub fn has_ink(&self, rect: Rectangle) -> bool {
        self.ink_count(rect) > 0
    }

    /// How many pixels inside `rect` differ from `reference` by more than
    /// `tolerance` on some channel.
    ///
    /// Catches a widget whose whole paint is dim — a disabled fill, a
    /// hairline — that the brightness reading alone waves through.
    #[must_use]
    pub fn painted_count(&self, rect: Rectangle, reference: [u8; 3], tolerance: u8) -> usize {
        self.count_where(rect, |pixel| channel_delta(&pixel, &reference) > tolerance)
    }

    /// How many pixels inside `rect` sit within `tolerance` of `reference`.
    ///
    /// The complement of [`painted_count`](Self::painted_count). Where that
    /// reading asks what a region is *not*, this one asks how much of it
    /// really **is** a given colour — which is how a status-filled node is
    /// told apart from a node that merely happens to be bright.
    #[must_use]
    pub fn matching_count(&self, rect: Rectangle, reference: [u8; 3], tolerance: u8) -> usize {
        self.count_where(rect, |pixel| channel_delta(&pixel, &reference) <= tolerance)
    }

    /// The tightest rectangle inside `rect` that holds every bright pixel.
    ///
    /// `None` when `rect` holds no ink at all. This is what lets a test
    /// assert not merely *that* a mark painted but *where* it painted —
    /// an ink-presence test passes while the mark sits outside the box that
    /// owns it; this does not.
    #[must_use]
    pub fn ink_bounds(&self, rect: Rectangle) -> Option<Rectangle> {
        let (x0, y0, x1, y1) = self.pixel_box(rect);
        let mut min_x = usize::MAX;
        let mut min_y = usize::MAX;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        let mut found = false;

        for y in y0..y1 {
            for x in x0..x1 {
                if is_bright(&self.pixel_at(x, y)) {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                    found = true;
                }
            }
        }

        if !found {
            return None;
        }

        Some(Rectangle::new(
            Point::new(min_x as f32 / self.scale, min_y as f32 / self.scale),
            Size::new(
                (max_x - min_x + 1) as f32 / self.scale,
                (max_y - min_y + 1) as f32 / self.scale,
            ),
        ))
    }

    /// Whether `rect` is entirely one colour, within `tolerance`.
    #[must_use]
    pub fn is_uniform(&self, rect: Rectangle, tolerance: u8) -> bool {
        let mut reference: Option<[u8; 3]> = None;

        !self.scan(rect, |pixel| match reference {
            Some(base) => channel_delta(&pixel, &base) > tolerance,
            None => {
                reference = Some(pixel);
                false
            }
        })
    }

    /// How many pixels inside `rect` this frame and `other` disagree on.
    ///
    /// The anti-vacuity reading: a control whose repaint draws nothing
    /// leaves the two frames byte-identical here, so a zero is a real
    /// failure rather than a test that never looked.
    ///
    /// # Panics
    /// Panics when the two frames were captured at different physical
    /// sizes, which would make a pixel-by-pixel comparison meaningless.
    #[must_use]
    pub fn differing_pixels(&self, other: &Frame, rect: Rectangle) -> usize {
        assert_eq!(
            (self.width_px, self.height_px),
            (other.width_px, other.height_px),
            "frames captured at different sizes cannot be diffed pixel by pixel"
        );

        let (x0, y0, x1, y1) = self.pixel_box(rect);
        let mut count = 0usize;

        for y in y0..y1 {
            for x in x0..x1 {
                let a = self.pixel_at(x, y);
                let b = other.pixel_at(x, y);
                if channel_delta(&a, &b) > SURFACE_TOLERANCE {
                    count += 1;
                }
            }
        }

        count
    }

    /// Walk every pixel of `rect`, stopping as soon as `visit` returns `true`.
    ///
    /// Returns whether the walk was cut short.
    fn scan(&self, rect: Rectangle, mut visit: impl FnMut([u8; 3]) -> bool) -> bool {
        let (x0, y0, x1, y1) = self.pixel_box(rect);

        for y in y0..y1 {
            for x in x0..x1 {
                if visit(self.pixel_at(x, y)) {
                    return true;
                }
            }
        }

        false
    }

    /// Count the pixels of `rect` that satisfy `predicate`.
    fn count_where(&self, rect: Rectangle, mut predicate: impl FnMut([u8; 3]) -> bool) -> usize {
        let mut count = 0usize;
        self.scan(rect, |pixel| {
            if predicate(pixel) {
                count += 1;
            }
            false
        });
        count
    }

    /// The pixel range covering `rect`, clipped to the frame.
    fn pixel_box(&self, rect: Rectangle) -> (usize, usize, usize, usize) {
        let frame_x = self.width_px as usize;
        let frame_y = self.height_px as usize;

        let x0 = (rect.x.max(0.0) * self.scale) as usize;
        let y0 = (rect.y.max(0.0) * self.scale) as usize;
        let x1 = ((rect.x + rect.width).max(0.0) * self.scale) as usize;
        let y1 = ((rect.y + rect.height).max(0.0) * self.scale) as usize;

        // Pull the origin inside the frame *before* clamping the far edge:
        // a rect wholly below the fold has `y0 > frame_y`, and `clamp` with
        // `min > max` panics. Clipping tests ask about exactly those rects,
        // so they must come back as an empty range (zero pixels), not a
        // crash.
        let x0 = x0.min(frame_x);
        let y0 = y0.min(frame_y);

        (x0, y0, x1.clamp(x0, frame_x), y1.clamp(y0, frame_y))
    }

    /// The RGB channels at physical pixel `(x, y)`, black past the buffer.
    fn pixel_at(&self, x: usize, y: usize) -> [u8; 3] {
        let stride = self.width_px as usize * 4;
        let i = y * stride + x * 4;
        if i + 2 >= self.rgba.len() {
            return [0, 0, 0];
        }

        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }

    fn index(&self, x: f32, y: f32) -> usize {
        let px = (x * self.scale) as usize;
        let py = (y * self.scale) as usize;
        py * self.width_px as usize * 4 + px * 4
    }
}

/// The 8-bit channels an iced [`Color`] must produce on screen.
#[must_use]
pub fn to_rgb(colour: Color) -> [u8; 3] {
    let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;

    [channel(colour.r), channel(colour.g), channel(colour.b)]
}

/// Whether a pixel is bright enough to be ink.
fn is_bright(pixel: &[u8; 3]) -> bool {
    let peak = pixel[0].max(pixel[1]).max(pixel[2]) as u32;

    peak > INK_THRESHOLD
}

/// The largest single-channel distance between two colours.
fn channel_delta(a: &[u8; 3], b: &[u8; 3]) -> u8 {
    let d = |x: u8, y: u8| x.abs_diff(y);

    d(a[0], b[0]).max(d(a[1], b[1])).max(d(a[2], b[2]))
}

/// Assert `after` genuinely repaints `rect` differently from `before`.
///
/// The strongest cheap render assertion in the crate: it fails on a widget
/// whose state moved while its pixels did not, which is invisible to every
/// state and geometry assertion. A control that latches open while painting
/// byte-identical ink to its closed frame passes a model check and fails
/// this one.
///
/// `what` names the change in the failure message so the report says which
/// control stayed dead.
///
/// # Panics
/// Panics when fewer than [`MIN_CHANGED_PIXELS`] pixels inside `rect`
/// moved by more than [`SURFACE_TOLERANCE`].
pub fn assert_repainted(before: &Frame, after: &Frame, rect: Rectangle, what: &str) {
    let moved = before.differing_pixels(after, rect);

    assert!(
        moved >= MIN_CHANGED_PIXELS,
        "{what} changed no pixels inside {rect:?} \
         ({moved} moved, {MIN_CHANGED_PIXELS} required) — the model moved but \
         the frame did not repaint"
    );
}

#[cfg(test)]
#[path = "test_helpers_tests.rs"]
mod test_helpers_tests;
