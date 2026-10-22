//! Proofs that a custom widget can hold keyboard focus in iced 0.14, and
//! the technique for driving iced's focus router without `iced_test`
//! support (**D-7**, **G-10**).
//!
//! # How focus is driven here
//!
//! `iced_test` 0.14 has no focus API — `Simulator` can click, type and
//! snapshot, but cannot focus anything, because iced itself has no focus
//! event to simulate. Focus lives one level down, in the widget-tree
//! `Operation` protocol, so these tests drive that protocol directly:
//!
//! * a headless `iced_test` renderer builds a real
//!   [`UserInterface`] over a real widget tree with real layout;
//! * [`UserInterface::operate`] runs a real `operation::focusable::*`
//!   operation across that tree;
//! * the [`user_interface::Cache`] is carried from one build to the
//!   next, so the widget `Tree` — where [`FocusableState`] actually
//!   lives — survives a rebuild the same way it does across `view`
//!   calls in a running app.
//!
//! [`Harness::run`] takes a *view closure* rather than an `Element`
//! because `Element` owns a `Box<dyn Widget>` and is not `Clone`: the
//! closure is re-run for every operation, which is exactly the
//! rebuild-per-frame a real application does, and makes "the tree
//! changed underneath" a one-line test rather than a special case.
//!
//! One thing the harness has to supply that `UserInterface::operate`
//! does not: **the operation chain loop**. `operation::focus_next` is
//! built as `operation::then(count(), …)`, whose `finish` returns
//! `Outcome::Chain(next)`. `UserInterface::operate` performs a single
//! pass and stops; the real runtime re-runs the chained operation in
//! `iced_winit`'s `Action::Widget` handler. A test that skipped the
//! loop would run only the `count` half of `focus_next` and conclude
//! focus was broken when the harness was.
//!
//! Results are read back with `find_focused` / `is_focused`, and with a
//! custom recording operation that captures every `(id, bounds)` the
//! router sees — which is how traversal *order* and layout transparency
//! are checked.

use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::focus::GroupId;
use crate::widget::focusable::Focusable;
use crate::widget::focusable::FocusableState;
use crate::widget::focusable::focusable;

use iced::Element;
use iced::Font;
use iced::Pixels;
use iced::Rectangle;
use iced::Theme;
use iced::widget::button;
use iced::widget::column;
use iced::widget::text;
use iced_test::core::Size;
use iced_test::core::widget::Id;
use iced_test::core::widget::Operation;
use iced_test::core::widget::operation::{self, Outcome, black_box};
use iced_test::futures::futures::executor::block_on;
use iced_test::renderer::Renderer;
use iced_test::runtime::UserInterface;
use iced_test::runtime::user_interface;

use std::cell::RefCell;

type Ui<'a> = UserInterface<'a, (), Theme, Renderer>;
type El<'a> = Element<'a, (), Theme, Renderer>;
/// A wrapper with the harness's concrete `Theme`/`Renderer`, needed where
/// a bare `Focusable::new` leaves them unresolvable.
type Wrapped = Focusable<'static, (), Theme, Renderer>;

const VIEWPORT: Size = Size::new(400.0, 300.0);

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// A headless iced interface whose widget state is carried across
/// rebuilds of whatever its view closure produces.
///
/// The [`user_interface::Cache`] owns the widget
/// [`Tree`](iced_test::core::widget::Tree), which is where
/// [`FocusableState`] lives, so carrying it from one
/// [`UserInterface::build`] to the next is what makes "the same UI,
/// rebuilt" testable — the same thing that lets focus survive a `view`
/// call in a running application.
struct Harness {
    renderer: Renderer,
    cache: user_interface::Cache,
    view: Box<dyn FnMut() -> El<'static>>,
}

impl Harness {
    fn new(view: impl Fn() -> El<'static> + 'static) -> Self {
        let renderer = block_on(<Renderer as iced_test::core::renderer::Headless>::new(
            Font::with_name("Fira Sans"),
            Pixels(16.0),
            None,
        ))
        .expect("a headless renderer should be available for focus tests");

        Self {
            renderer,
            cache: user_interface::Cache::default(),
            view: Box::new(view),
        }
    }

    /// Swap the tree the harness rebuilds on the next operation.
    fn set_view(&mut self, view: impl Fn() -> El<'static> + 'static) {
        self.view = Box::new(view);
    }

    /// Run one operation — and its whole chain — against a freshly
    /// rebuilt interface, then carry the widget state back out.
    ///
    /// The chain loop mirrors `iced_winit`'s `Action::Widget` handler.
    fn run<T>(&mut self, mut operation: Box<dyn Operation<T>>) -> Option<T> {
        let root = (self.view)();
        let cache = std::mem::take(&mut self.cache);
        let mut ui: Ui<'_> = UserInterface::build(root, VIEWPORT, cache, &mut self.renderer);

        let result = loop {
            ui.operate(&self.renderer, &mut black_box(operation.as_mut()));

            match operation.finish() {
                Outcome::None => break None,
                Outcome::Some(value) => break Some(value),
                Outcome::Chain(next) => operation = next,
            }
        };

        self.cache = ui.into_cache();
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

    /// Ask the router to focus `id`.
    fn focus(&mut self, id: &Id) {
        self.run(Box::new(operation::focusable::focus::<()>(id.clone())));
    }

    /// Ask the router to drop focus.
    fn unfocus(&mut self) {
        self.run(Box::new(operation::focusable::unfocus::<()>()));
    }

    /// Ask the router to move focus to the next widget it sees.
    fn focus_next(&mut self) {
        self.run(Box::new(operation::focusable::focus_next::<()>()));
    }

    /// Ask the router to move focus to the previous widget it sees.
    fn focus_previous(&mut self) {
        self.run(Box::new(operation::focusable::focus_previous::<()>()));
    }
}

/// An operation that records every focusable widget the router visits.
///
/// `Send` holds because the recording sits behind a `RefCell`, which is
/// `Send` for a `Send` payload, and the operation never leaves its
/// thread.
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
// Fixtures
// ---------------------------------------------------------------------------

fn iced_id(label: &str) -> Id {
    Id::from(label.to_owned())
}

fn ids(labels: &[&'static str]) -> Vec<Id> {
    labels.iter().map(|label| iced_id(label)).collect()
}

/// The same ids as the router reports them: the wrapper hands over
/// `Option<&Id>`, so a named widget shows up as `Some(id)`.
fn opt_ids(labels: &[&'static str]) -> Vec<Option<Id>> {
    ids(labels).into_iter().map(Some).collect()
}

/// A focusable wrapper around a label, as a host would build one.
fn wrapped(label: &'static str, enabled: bool) -> El<'static> {
    focusable(text(label), FocusId::new(label), enabled)
}

fn tree(labels: &[&'static str]) -> impl Fn() -> El<'static> + 'static {
    let labels: Vec<&'static str> = labels.to_vec();

    move || column(labels.iter().map(|label| wrapped(label, true))).into()
}

/// A tree whose members carry their own enabled flags.
fn mixed_tree(entries: &[(&'static str, bool)]) -> impl Fn() -> El<'static> + 'static {
    let entries: Vec<(&'static str, bool)> = entries.to_vec();

    move || {
        column(
            entries
                .iter()
                .map(|(label, enabled)| wrapped(label, *enabled)),
        )
        .into()
    }
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

/// Push the model's current focus into the router — the direction
/// `INC-11` uses. An empty model pushes `unfocus`, because "nothing
/// focused" is a state the router has to be told about too.
fn sync(harness: &mut Harness, model: &FocusState) {
    match model.current() {
        Some(id) => harness.focus(&iced_id(id.as_str())),
        None => harness.unfocus(),
    }
}

fn assert_agree(harness: &mut Harness, model: &FocusState, step: &str) {
    let expected = model.current().map(|id| iced_id(id.as_str()));
    assert_eq!(
        harness.focused(),
        expected,
        "after {step}: the router and `FocusState` must name the same control"
    );
}

// ---------------------------------------------------------------------------
// 1. Registration
// ---------------------------------------------------------------------------

#[test]
fn a_wrapper_registers_with_the_focus_router() {
    let mut harness = Harness::new(tree(&["alpha"]));

    assert_eq!(
        harness.count().total,
        1,
        "one wrapper must appear as one focusable widget in the router"
    );
    assert_eq!(
        harness.registered(),
        vec![Some(iced_id("alpha"))],
        "the wrapper must register under the `Id` derived from its `FocusId`"
    );
}

#[test]
fn a_registered_wrapper_starts_unfocused() {
    let mut harness = Harness::new(tree(&["alpha"]));
    let id = iced_id("alpha");

    assert!(
        !harness.is_focused(&id),
        "registration alone must not put focus on the wrapper"
    );
    assert_eq!(harness.focused(), None);
}

#[test]
fn the_focus_operation_reaches_the_wrapper_and_its_state_becomes_focused() {
    let mut harness = Harness::new(tree(&["alpha"]));
    let id = iced_id("alpha");

    harness.focus(&id);

    assert!(
        harness.is_focused(&id),
        "`operation::focus` must reach the wrapper and flip \
         `FocusableState::is_focused`"
    );
    assert_eq!(harness.focused(), Some(id));
}

#[test]
fn stock_widgets_register_nothing_so_the_wrapper_is_what_adds_them() {
    let mut harness = Harness::new(|| column(vec![text("a").into(), button("b").into()]).into());

    assert_eq!(
        harness.count().total,
        0,
        "a tree of stock iced widgets must expose no focusable widget at all \
         (G-10): every registration these tests see comes from our wrapper"
    );

    harness.set_view(|| {
        column(vec![
            text("a").into(),
            button("b").into(),
            wrapped("c", true),
        ])
        .into()
    });

    assert_eq!(
        harness.count().total,
        1,
        "adding one wrapper to a stock tree adds exactly one focusable widget"
    );
}

#[test]
fn registration_follows_tree_order_and_carries_real_bounds() {
    let mut harness = Harness::new(tree(&["one", "two", "three"]));

    let recorded = harness.recorded();

    assert_eq!(
        recorded
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>(),
        opt_ids(&["one", "two", "three"]),
        "the router must visit wrappers in tree order"
    );

    let heights: Vec<f32> = recorded.iter().map(|(_, bounds)| bounds.height).collect();
    assert!(
        heights.iter().all(|height| *height > 0.0),
        "every wrapper must report real laid-out bounds, got {heights:?}"
    );

    let tops: Vec<f32> = recorded.iter().map(|(_, bounds)| bounds.y).collect();
    assert!(
        tops.windows(2).all(|pair| pair[1] > pair[0]),
        "a column lays each member further down the screen, got {tops:?}"
    );
}

// ---------------------------------------------------------------------------
// 2. Traversal
// ---------------------------------------------------------------------------

#[test]
fn focus_next_walks_the_wrappers_in_tree_order() {
    let mut harness = Harness::new(tree(&["a", "b", "c"]));
    let expected = ids(&["a", "b", "c"]);

    harness.focus_next();
    assert_eq!(
        harness.focused(),
        Some(expected[0].clone()),
        "with nothing focused, Tab lands on the first wrapper"
    );

    for id in &expected[1..] {
        harness.focus_next();
        assert_eq!(harness.focused(), Some(id.clone()));
    }
}

#[test]
fn focus_previous_walks_the_wrappers_in_reverse_tree_order() {
    let mut harness = Harness::new(tree(&["a", "b", "c"]));
    let expected = ids(&["a", "b", "c"]);

    harness.focus(&expected[2]);

    harness.focus_previous();
    assert_eq!(harness.focused(), Some(expected[1].clone()));

    harness.focus_previous();
    assert_eq!(harness.focused(), Some(expected[0].clone()));
}

#[test]
fn only_one_wrapper_holds_focus_at_a_time() {
    let mut harness = Harness::new(tree(&["a", "b", "c"]));
    let all = ids(&["a", "b", "c"]);

    harness.focus(&all[1]);

    assert!(harness.is_focused(&all[1]));
    assert!(!harness.is_focused(&all[0]));
    assert!(!harness.is_focused(&all[2]));
    assert_eq!(
        harness.count().focused,
        Some(1),
        "the router's focused index must agree with which wrapper reports focus"
    );
}

#[test]
fn iced_traversal_drops_focus_at_the_ends_instead_of_wrapping() {
    // A limitation of iced's own traversal, pinned rather than worked
    // around. `FocusNext` unfocuses the last widget and has no widget at
    // index `total` to focus next, so Tab past the end loses focus
    // instead of cycling; `FocusPrevious` drops it before the first.
    // `FocusState::focus_next` wraps, which is what `UIF-CMENU-003`
    // requires, so the bridge must compute the next id from the model
    // and push it with `focus(id)` — never hand `Tab` to
    // `operation::focusable::focus_next`.
    let mut harness = Harness::new(tree(&["a", "b", "c"]));

    harness.focus(&iced_id("c"));
    harness.focus_next();
    assert_eq!(
        harness.focused(),
        None,
        "Tab past the last wrapper loses focus in iced rather than wrapping"
    );

    harness.focus(&iced_id("a"));
    harness.focus_previous();
    assert_eq!(
        harness.focused(),
        None,
        "Shift+Tab before the first wrapper loses focus in iced rather than wrapping"
    );
}

// ---------------------------------------------------------------------------
// 3. Enabled gating
// ---------------------------------------------------------------------------

#[test]
fn a_disabled_wrapper_is_not_registered_at_all() {
    let mut harness = Harness::new(mixed_tree(&[("a", true), ("b", false), ("c", true)]));

    assert_eq!(
        harness.count().total,
        2,
        "a disabled wrapper must be absent from the router, not merely skipped by name"
    );
    assert_eq!(
        harness.registered(),
        opt_ids(&["a", "c"]),
        "only enabled wrappers may appear"
    );
}

#[test]
fn traversal_steps_over_a_disabled_wrapper() {
    let mut harness = Harness::new(mixed_tree(&[("a", true), ("b", false), ("c", true)]));

    harness.focus_next();
    assert_eq!(harness.focused(), Some(iced_id("a")));

    harness.focus_next();
    assert_eq!(
        harness.focused(),
        Some(iced_id("c")),
        "Tab must go straight from `a` to `c`, never stopping on the disabled `b`"
    );
}

#[test]
fn focusing_a_disabled_wrapper_is_a_no_op() {
    let mut harness = Harness::new(mixed_tree(&[("a", true), ("b", false)]));
    let disabled = iced_id("b");

    harness.focus(&disabled);

    assert_eq!(
        harness.focused(),
        None,
        "a focus request for a disabled wrapper must not move focus anywhere"
    );
    assert!(
        !harness.is_focused(&disabled),
        "the disabled wrapper must not report focus"
    );
}

#[test]
fn disabling_the_focused_wrapper_clears_its_focus_on_the_next_pass() {
    let mut harness = Harness::new(tree(&["a", "b"]));
    let id = iced_id("a");

    harness.focus(&id);
    assert!(harness.is_focused(&id));

    harness.set_view(mixed_tree(&[("a", false), ("b", true)]));

    assert!(
        !harness.is_focused(&id),
        "a wrapper that becomes disabled must drop the focus it was holding: it \
         is invisible to the router, so nothing else can take it back"
    );
    assert_eq!(
        harness.focused(),
        None,
        "no phantom focus may survive the transition"
    );
}

#[test]
fn a_tree_of_only_disabled_wrappers_cannot_hold_focus() {
    let mut harness = Harness::new(mixed_tree(&[("a", false), ("b", false)]));

    assert_eq!(harness.count().total, 0);

    harness.focus_next();
    harness.focus(&iced_id("a"));

    assert_eq!(harness.focused(), None);
}

#[test]
fn re_enabling_a_wrapper_makes_it_reachable_again() {
    let mut harness = Harness::new(mixed_tree(&[("a", false), ("b", true)]));
    let a = iced_id("a");

    harness.focus(&a);
    assert_eq!(harness.focused(), None);

    harness.set_view(mixed_tree(&[("a", true), ("b", true)]));
    harness.focus(&a);

    assert_eq!(
        harness.focused(),
        Some(a),
        "gating follows the flag on the current frame, not the flag the wrapper \
         was first built with"
    );
}

// ---------------------------------------------------------------------------
// 4. Round-trip with the pure model
// ---------------------------------------------------------------------------

#[test]
fn the_model_and_the_router_agree_across_a_full_walk() {
    let mut harness = Harness::new(tree(&["a", "b", "c"]));
    let (group, mut model) = model_of(&["a", "b", "c"]);

    sync(&mut harness, &model);
    assert_agree(&mut harness, &model, "registration with nothing focused");

    model.focus_next();
    sync(&mut harness, &model);
    assert_agree(&mut harness, &model, "focus_next into the first member");

    model.focus_next();
    sync(&mut harness, &model);
    assert_agree(&mut harness, &model, "focus_next to the second member");

    model.focus_next();
    sync(&mut harness, &model);
    assert_agree(&mut harness, &model, "focus_next to the last member");

    model.focus_next();
    sync(&mut harness, &model);
    assert_agree(&mut harness, &model, "focus_next wrapping to the front");

    model.focus_previous();
    sync(&mut harness, &model);
    assert_agree(&mut harness, &model, "focus_previous wrapping to the back");

    model.focus_first_in_group(&group);
    sync(&mut harness, &model);
    assert_agree(&mut harness, &model, "focus_first_in_group");

    model.focus_last_in_group(&group);
    sync(&mut harness, &model);
    assert_agree(&mut harness, &model, "focus_last_in_group");

    model.move_within(&group, 1);
    sync(&mut harness, &model);
    assert_agree(
        &mut harness,
        &model,
        "move_within(+1) from the last row, wrapping to the first",
    );

    model.move_within(&group, -1);
    sync(&mut harness, &model);
    assert_agree(
        &mut harness,
        &model,
        "move_within(-1) from the first row, wrapping to the last",
    );
}

#[test]
fn the_model_skips_disabled_members_and_the_router_follows() {
    let mut harness = Harness::new(mixed_tree(&[("a", true), ("b", false), ("c", true)]));

    let group = GroupId::new("menu");
    let mut model = FocusState::new();
    model.register_group(
        group.clone(),
        vec![
            FocusMember::new("a"),
            FocusMember::disabled("b"),
            FocusMember::new("c"),
        ],
    );

    model.focus_first_in_group(&group);
    sync(&mut harness, &model);
    assert_eq!(harness.focused(), Some(iced_id("a")), "first enabled row");

    model.focus_next();
    sync(&mut harness, &model);
    assert_eq!(
        harness.focused(),
        Some(iced_id("c")),
        "both layers must skip the disabled row the same way"
    );

    model.focus_next();
    sync(&mut harness, &model);
    assert_agree(
        &mut harness,
        &model,
        "wrapping past the last enabled row skips the disabled one again",
    );
}

#[test]
fn a_model_rejection_leaves_the_router_untouched() {
    let mut harness = Harness::new(mixed_tree(&[("a", true), ("b", false), ("c", true)]));

    let group = GroupId::new("menu");
    let mut model = FocusState::new();
    model.register_group(
        group.clone(),
        vec![
            FocusMember::new("a"),
            FocusMember::disabled("b"),
            FocusMember::new("c"),
        ],
    );
    assert!(model.focus(&FocusId::new("a")));
    sync(&mut harness, &model);
    assert_agree(&mut harness, &model, "focus on `a`");

    assert!(
        !model.focus(&FocusId::new("b")),
        "the model must reject a disabled member"
    );
    assert!(
        !model.focus(&FocusId::new("unregistered")),
        "the model must reject an unknown id"
    );
    sync(&mut harness, &model);
    assert_eq!(
        harness.focused(),
        Some(iced_id("a")),
        "a rejected transition must not move the router either"
    );

    model.push_trap(GroupId::new("modal"));
    assert!(
        !model.focus(&FocusId::new("c")),
        "the trap must reject a member outside its group"
    );
    sync(&mut harness, &model);
    assert_eq!(
        harness.focused(),
        Some(iced_id("a")),
        "the router must still show the trapped focus, not the rejected one"
    );
}

#[test]
fn the_router_projection_is_pushed_from_the_model_because_iced_traversal_diverges() {
    // Same starting point, two traversal engines, one divergence: at the
    // last row the model wraps and iced's `focus_next` drops focus. If
    // the bridge ever handed `Tab` to the router instead of pushing the
    // model's answer, this is exactly where the two would come apart.
    let mut harness = Harness::new(tree(&["a", "b", "c"]));
    let (group, mut model) = model_of(&["a", "b", "c"]);

    model.focus_last_in_group(&group);
    sync(&mut harness, &model);
    assert_eq!(harness.focused(), Some(iced_id("c")));

    model.focus_next();
    assert_eq!(model.current(), Some(&FocusId::new("a")), "the model wraps");

    harness.focus_next();
    assert_eq!(
        harness.focused(),
        None,
        "iced's own traversal drops focus here — which is why the bridge is \
         push-from-the-model, not delegate-to-the-router"
    );

    sync(&mut harness, &model);
    assert_agree(&mut harness, &model, "pushing the model's wrapped answer");
}

#[test]
fn clearing_the_model_clears_the_router() {
    let mut harness = Harness::new(tree(&["a", "b"]));
    let (group, mut model) = model_of(&["a", "b"]);

    model.focus_first_in_group(&group);
    sync(&mut harness, &model);
    assert_eq!(harness.focused(), Some(iced_id("a")));

    assert_eq!(model.clear(), Some(FocusId::new("a")));
    sync(&mut harness, &model);
    assert_eq!(harness.focused(), None);
}

// ---------------------------------------------------------------------------
// 5. Cleanup
// ---------------------------------------------------------------------------

#[test]
fn removing_the_focused_wrapper_leaves_no_phantom_focus() {
    let mut harness = Harness::new(tree(&["a", "b"]));
    let b = iced_id("b");

    harness.focus(&b);
    assert_eq!(harness.focused(), Some(b.clone()));

    harness.set_view(tree(&["a"]));

    assert_eq!(
        harness.focused(),
        None,
        "`b` is gone from the tree; its focus must have gone with it rather than \
         lingering as an id nothing can reach"
    );
    assert_eq!(
        harness.count().total,
        1,
        "the router must see only the wrapper that is still mounted"
    );
}

#[test]
fn a_wrapper_that_survives_a_rebuild_keeps_its_focus() {
    let mut harness = Harness::new(tree(&["a", "b"]));
    let b = iced_id("b");

    harness.focus(&b);
    harness.focus(&b);

    assert_eq!(
        harness.focused(),
        Some(b),
        "rebuilding the same tree must not reset focus — the state is pinned by \
         the wrapper's tag"
    );
}

#[test]
fn removing_a_different_wrapper_keeps_the_focus() {
    let mut harness = Harness::new(tree(&["a", "b", "c"]));
    let a = iced_id("a");

    harness.focus(&a);
    harness.set_view(tree(&["a", "c"]));

    assert_eq!(
        harness.focused(),
        Some(a),
        "dropping a sibling must not disturb the focused wrapper"
    );
}

#[test]
fn a_wrapper_disabled_while_focused_stays_unfocused_across_rebuilds() {
    let mut harness = Harness::new(tree(&["a", "b"]));
    let a = iced_id("a");

    harness.focus(&a);
    assert!(harness.is_focused(&a));

    harness.set_view(mixed_tree(&[("a", false), ("b", true)]));
    assert!(!harness.is_focused(&a));

    harness.set_view(mixed_tree(&[("a", false), ("b", true)]));
    assert_eq!(
        harness.focused(),
        None,
        "the cleared state must stay cleared across rebuilds rather than be \
         restored by the surviving tag"
    );
}

// ---------------------------------------------------------------------------
// Wrapper contract
// ---------------------------------------------------------------------------

#[test]
fn the_widget_id_is_derived_from_the_focus_id() {
    let widget: Wrapped = Focusable::new(text("save"), FocusId::new("dialog.save"), true);

    assert_eq!(widget.focus_id(), &FocusId::new("dialog.save"));
    assert_eq!(
        widget.widget_id(),
        &iced_id("dialog.save"),
        "the iced `Id` must be the `FocusId`'s own string, so a rebuilt wrapper \
         re-claims the same router slot"
    );
    assert!(widget.is_enabled());

    let dead: Wrapped = Focusable::new(text("save"), FocusId::new("dialog.save"), false);
    assert!(!dead.is_enabled());
    assert_eq!(dead.widget_id(), widget.widget_id());
}

#[test]
fn the_wrapper_is_layout_transparent() {
    // Two wrappers around the same child must report the same bounds, or
    // the outer one is adding a layout level — a styling bug wearing a
    // focus fix. Nesting is the only way to see this through the focus
    // router alone, because both layers register.
    let mut harness =
        Harness::new(|| Focusable::new(wrapped("inner", true), FocusId::new("outer"), true).into());

    let recorded = harness.recorded();

    assert_eq!(
        recorded
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>(),
        vec![Some(iced_id("outer")), Some(iced_id("inner"))],
        "the outer wrapper registers first, then descends into its child"
    );
    assert_eq!(
        recorded[0].1, recorded[1].1,
        "the outer wrapper must not change the bounds of what it wraps"
    );
}

#[test]
fn a_disabled_wrapper_still_lets_its_children_register() {
    // The flag gates the wrapper's own registration only. A disabled
    // wrapper is not a subtree blackout: whether something mounted
    // inside one is focusable is the host's decision, not this widget's.
    let mut harness = Harness::new(|| {
        Focusable::new(wrapped("inner", true), FocusId::new("outer"), false).into()
    });

    assert_eq!(
        harness.registered(),
        vec![Some(iced_id("inner"))],
        "the disabled outer wrapper must be absent while its inner one stays"
    );
}

#[test]
fn the_wrapper_state_round_trips_through_the_router_protocol_directly() {
    // The bare protocol, with no widget or tree in the way: the router's
    // three verbs are the only way in and out of focus.
    let mut state = FocusableState::new();
    assert!(!state.is_focused());

    <FocusableState as operation::Focusable>::focus(&mut state);
    assert!(state.is_focused());
    assert!(<FocusableState as operation::Focusable>::is_focused(&state));

    <FocusableState as operation::Focusable>::unfocus(&mut state);
    assert!(!state.is_focused());
    assert_eq!(FocusableState::default(), FocusableState::new());
}
