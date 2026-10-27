//! The focusable wrapper — the plumbing that lets a custom widget hold
//! keyboard focus in iced 0.14 (decision **D-7**, gap **G-10**).
//!
//! # What this proves
//!
//! [`crate::focus`] owns the *policy*: which id holds focus, what `Tab`
//! does, where a trap confines movement. It is deliberately free of iced,
//! which leaves the question this module answers: can any of that policy
//! actually be *attached* to a real widget tree, so that a widget of our
//! own holds keyboard focus the way a `text_input` does?
//!
//! It can. [`Focusable`] is a wrapper around any child widget that
//! registers itself with iced's focus router, and the tests in the
//! sidecar drive that registration with real `operation`s against a real
//! laid-out tree. This is the mechanism every keyboard-reachable atom in
//! this framework is built on, so it is proved here once, before ten
//! widgets depend on it.
//!
//! # The mechanism
//!
//! iced has no widget-level focus event and no automatic `Tab` traversal
//! (**G-10**). Focus is entirely a *router protocol*: a widget announces
//! itself to an in-flight [`Operation`] and that operation mutates the
//! widget's own state. Three pieces, mirrored from `text_input` — the
//! only stock widget in iced 0.14 that can hold focus:
//!
//! 1. [`FocusableState`] — a `Copy` type implementing
//!    [`operation::Focusable`], the trait the router talks to. It is
//!    *the* focus state: `is_focused` / `focus` / `unfocus`.
//! 2. [`Widget::operate`] — calls
//!    [`operation::Operation::focusable`] with the widget's [`Id`], its
//!    layout bounds and that state. This single call is what makes the
//!    widget visible to the router. `Button::operate` never makes it,
//!    which is precisely why a button cannot be focused.
//! 3. [`Widget::tag`] / [`Widget::state`] — pin [`FocusableState`] into
//!    the widget's [`Tree`] slot so the state survives a rebuild instead
//!    of being recreated unfocused every frame.
//!
//! Everything else — traversal order, skipping, traps — is the
//! *operation's* job. `operation::focusable::focus_next` counts the
//! widgets that announced themselves and focuses the next one; it works
//! on our wrapper exactly as it works on a `text_input`, because the
//! router cannot tell them apart.
//!
//! # Disabled means unregistered
//!
//! The router has no concept of a disabled widget. `focus_next` stops at
//! every widget that called `operation.focusable`, and `focus(id)`
//! happily focuses any id that is present. So gating is done the only way
//! the router can hear: a disabled wrapper **does not register at all**.
//! It is absent from `count`, stepped over by `focus_next` /
//! `focus_previous`, and a `focus(disabled_id)` request finds nothing to
//! mutate and is a no-op rather than a refusal it would have to invent.
//!
//! That mirrors [`crate::focus::FocusState`], where a disabled member is
//! rejected by [`crate::focus::FocusState::focus`] and skipped by
//! [`crate::focus::FocusState::focus_next`], and it means the two layers
//! refuse the same control for the same reason rather than by two
//! different accidents.
//!
//! # A focus nobody can leave is a bug, not a state
//!
//! Because a disabled wrapper never registers, the router cannot take
//! focus back from it — it cannot see it. A wrapper that was focused
//! while enabled and is *then* disabled would keep `is_focused == true`
//! forever: invisible to traversal, unreachable by `unfocus()`, and
//! still painting a ring on a dead control. [`crate::state::resolve`]
//! zeroes the ring for a disabled control, so the user would be holding
//! focus nothing on screen marks as theirs.
//!
//! [`Focusable`] therefore clears its own focus on every pass in which
//! it is disabled. `operate` runs on every operation the runtime
//! dispatches, so the state cannot drift: disabled ⇒ unfocused, checked
//! in the one place that knows both facts. This is also what keeps a
//! wrapper that leaves the tree from leaving a phantom behind — see the
//! cleanup tests in the sidecar.
//!
//! # The bridge runs one way
//!
//! [`crate::focus::FocusState`] is authoritative. The iced router is a
//! **projection** of it, pushed down with
//! `operation::focusable::focus(id)` after every policy transition, and
//! read back with `operation::focusable::find_focused` only to verify
//! the projection took.
//!
//! That direction is not a style preference. The pure model holds the
//! rules the router does not have — groups, traps, slot-preserving
//! fall-forward — so a router-driven design would have to re-implement
//! them somewhere anyway, and the router has no place to put them.
//! Reading the router *up* into the model would let a widget that
//! registered itself outside the policy (a `text_input` in a modal,
//! say) move focus against the trap. The sidecar's round-trip test pins
//! the two agreeing after every transition so a divergence is a failing
//! test rather than a heisenbug in `INC-11`.
//!
//! # Layout is transparent
//!
//! The wrapper adds no layout node: it hands the child the limits it was
//! given and returns the child's own [`layout::Node`],
//! passing that same [`Layout`] down to the child in `operate`,
//! `update`, `draw` and `overlay`. A focus wrapper that changed the
//! geometry of what it wraps would be a styling bug disguised as a
//! focus fix, and every atom in this framework is specified down to the
//! pixel.
//!
//! The consequence is that the child is *not* at
//! `layout.children().next()` — it is at `layout`. That is the
//! pass-through shape [`Widget::operate`] uses, and it is why the child
//! still receives its own children correctly.
//!
//! # What is deliberately not here
//!
//! The focus ring. [`crate::widget::focus`] already resolves what it
//! looks like; wiring it to this state is a later increment, and until
//! then [`FocusableState`] drives nothing visual. The wrapper also does
//! not announce itself as a `container`: its id names a *focus* target,
//! not a scope, and `operation::scope` keys off container ids, so
//! claiming one would let a focus id hijack scoped traversal.
//!
//! Unit tests live in the sibling `focusable_tests.rs` sidecar.

use crate::focus::FocusId;
use iced::Element;
use iced::Event;
use iced::Length;
use iced::Rectangle;
use iced::Size;
use iced::Vector;
use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay::Element as OverlayElement;
use iced::advanced::renderer;
use iced::advanced::widget::operation;
use iced::advanced::widget::{Id, Operation, Tree, Widget, tree};
use iced::advanced::{Clipboard, Shell};
use iced::mouse;

/// The focus state a [`Focusable`] wrapper keeps in iced's widget tree.
///
/// This is the type the focus router mutates. Implementing
/// [`operation::Focusable`] on it is the entire contract: the router
/// calls [`focus`](operation::Focusable::focus) when an operation wants
/// this widget focused and [`unfocus`](operation::Focusable::unfocus)
/// when it wants it released, and reads
/// [`is_focused`](operation::Focusable::is_focused) to find out where
/// focus currently is.
///
/// It carries nothing else. In particular it does not carry the
/// [`FocusId`] — the id is the wrapper's, passed to the router at
/// registration, and duplicating it here would give one fact two
/// owners.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FocusableState {
    is_focused: bool,
}

impl FocusableState {
    /// A state that does not hold focus.
    #[must_use]
    pub const fn new() -> Self {
        Self { is_focused: false }
    }

    /// Whether this wrapper currently holds keyboard focus.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.is_focused
    }
}

impl operation::Focusable for FocusableState {
    fn is_focused(&self) -> bool {
        FocusableState::is_focused(self)
    }

    fn focus(&mut self) {
        self.is_focused = true;
    }

    fn unfocus(&mut self) {
        self.is_focused = false;
    }
}

/// A wrapper that gives any child widget a slot in iced's focus router.
///
/// The child is untouched — it keeps its own layout, events, drawing and
/// overlays — and gains the ability to hold keyboard focus under a
/// stable [`FocusId`]. See the [module docs](self) for the mechanism
/// and for what the `enabled` flag does and does not gate.
pub struct Focusable<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    focus_id: FocusId,
    widget_id: Id,
    enabled: bool,
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme, Renderer> Focusable<'a, Message, Theme, Renderer> {
    /// Wrap `content` so it can hold keyboard focus under `id`.
    ///
    /// `enabled` decides whether the wrapper registers with the router
    /// at all: `false` makes it invisible to traversal and immune to a
    /// focus request, and forces any focus it already held to be
    /// released on the next pass.
    ///
    /// The iced [`Id`] the router addresses is derived from the
    /// [`FocusId`]'s string, so the two name the same control and a
    /// rebuilt wrapper re-claims the same slot.
    #[must_use]
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        id: FocusId,
        enabled: bool,
    ) -> Self {
        let widget_id = Id::from(id.as_str().to_owned());

        Self {
            focus_id: id,
            widget_id,
            enabled,
            content: content.into(),
        }
    }

    /// The framework-level identity this wrapper answers to.
    #[must_use]
    pub fn focus_id(&self) -> &FocusId {
        &self.focus_id
    }

    /// The iced [`Id`] this wrapper registers with the focus router.
    #[must_use]
    pub fn widget_id(&self) -> &Id {
        &self.widget_id
    }

    /// Whether this wrapper is currently registered with the router.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Focusable<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<FocusableState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(FocusableState::new())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<FocusableState>();

        if self.enabled {
            operation.focusable(Some(&self.widget_id), layout.bounds(), state);
        } else {
            // Unregistered, so the router cannot take this focus back —
            // it cannot see the widget at all. Drop it here rather than
            // leave a ring lit on a control nothing can traverse to.
            // Called through the trait because `FocusableState` has no
            // inherent `unfocus`: the router's protocol is the only way
            // in and out of focus.
            <FocusableState as operation::Focusable>::unfocus(state);
        }

        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                operation,
            );
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<OverlayElement<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Focusable<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer,
{
    fn from(
        focusable: Focusable<'a, Message, Theme, Renderer>,
    ) -> Element<'a, Message, Theme, Renderer> {
        Element::new(focusable)
    }
}

/// Wrap `content` in a [`Focusable`] and hand back an [`Element`].
///
/// The ergonomic entry point for hosts: `focusable(row, id, enabled)`.
/// Equivalent to `Focusable::new(content, id, enabled).into()`.
pub fn focusable<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    id: FocusId,
    enabled: bool,
) -> Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer,
{
    Element::new(Focusable::new(content, id, enabled))
}

#[cfg(test)]
#[path = "focusable_tests.rs"]
mod focusable_tests;
