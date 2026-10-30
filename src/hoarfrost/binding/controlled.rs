//! Who owns the value, and what the widget is allowed to do with it (§3.0.4.1).
//!
//! §3.0.4.1 splits a control's state by **owner**, not by lifetime, because the
//! two failure modes it exists to prevent come from getting the ownership wrong in
//! opposite directions:
//!
//! - The framework keeps its own copy of a value the application owns. The app
//!   changes it, the widget does not notice, and the control **lies**.
//! - Application state is consulted for pointer feedback. Every hover is a
//!   round trip through a subsystem that has no business being on that path, and
//!   the control **lags**.
//!
//! So the split is not "fast state vs. slow state" — it is *theirs* vs. *ours*,
//! and each half is expressed as a type that cannot be confused for the other.
//!
//! # [`Controlled<'a, M>`] — theirs
//!
//! A shared borrow of the application's model, handed to the widget for the frame.
//! The whole guarantee is carried by that being a `&`: there is deliberately **no
//! `get_mut`**, no interior mutability, and no `Cell` hiding behind an immutable
//! API. The widget can read the model as often as it likes and cannot write it at
//! all.
//!
//! That is a stronger statement than "the widget promises not to write". It is the
//! reason this is a wrapper rather than a bare `&M` at each call site: the wrapper
//! makes the *role* visible in the signature, so a reviewer checking §3.0.4.1
//! reads `Controlled<ButtonModel>` and knows the boundary is there without having
//! to trace where the reference came from.
//!
//! # [`Ephemeral<S>`] — ours
//!
//! The mirror image: state the widget layer owns outright and the application
//! never sees unless it is explicitly emitted. [`VisualState`](crate::state::VisualState),
//! hover timers, the active drag anchor, the IME composition buffer, scroll
//! velocity and open-flyout flags all belong here. They are read during layout and
//! paint of the *same* frame they were written in, which is what makes
//! pointer feedback land inside one frame instead of round-tripping.
//!
//! # The one-way door
//!
//! Nothing in this module writes from the ephemeral side into the controlled side.
//! The only path back to the application is a tier emission
//! ([`Live`](crate::binding::tier::Live) or [`Commit`](crate::binding::tier::Commit)),
//! which is a value handed *out*, not an assignment made *in*. A widget that
//! wanted to shortcut the round trip by mutating its own `Controlled` would find
//! no method to do it with — which is the point.
//!
//! Unit tests live in the sibling `controlled_tests.rs` sidecar.

use std::fmt;

/// Application-owned model data, borrowed read-only for the duration of a frame.
///
/// `M` is whatever the widget's controlled data model is — `ButtonModel`, a text
/// field's `value` and `validation`, a list's `selected_ids`. The wrapper does not
/// constrain `M`'s shape, because no shape is common to every control (see the
/// trait-rejection rationale in [`binding`](crate::binding)).
///
/// There is no mutable accessor. A widget holding a `Controlled<'a, M>` can read
/// the model and cannot change it; changes travel back out as tier emissions.
pub struct Controlled<'a, M> {
    model: &'a M,
}

/// Hand-written so it follows `&M` semantics: a shared borrow is copyable no
/// matter how heavy `M` is. Deriving would demand `M: Copy`, which would make the
/// wrapper *less* usable than the reference it exists to wrap.
impl<'a, M> Clone for Controlled<'a, M> {
    fn clone(&self) -> Self {
        *self
    }
}

/// See [`Clone`]: copying a `Controlled` copies the borrow, never the model.
impl<'a, M> Copy for Controlled<'a, M> {}

impl<'a, M> Controlled<'a, M> {
    /// Wrap a borrowed model for the frame.
    #[must_use]
    pub fn new(model: &'a M) -> Self {
        Self { model }
    }

    /// Read the model.
    #[must_use]
    pub fn get(&self) -> &'a M {
        self.model
    }

    /// Project the borrow onto a field or sub-part of the model.
    ///
    /// Lets a child control receive `Controlled<'a, Validation>` where the parent
    /// holds `Controlled<'a, FormModel>`, without any clone and without the child
    /// being able to reach the rest of the model.
    #[must_use]
    pub fn map<N>(&self, project: impl FnOnce(&'a M) -> &'a N) -> Controlled<'a, N> {
        Controlled {
            model: project(self.model),
        }
    }
}

impl<'a, M> From<&'a M> for Controlled<'a, M> {
    fn from(model: &'a M) -> Self {
        Self::new(model)
    }
}

impl<M: fmt::Debug> fmt::Debug for Controlled<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Controlled")
            .field("model", self.model)
            .finish()
    }
}

/// Widget-layer interaction state, owned outright by the framework side.
///
/// Unlike [`Controlled`], this is held by value and written freely during event
/// handling. It is the half of §3.0.4.1 that must never be round-tripped: the
/// value written while processing a pointer event is readable in the layout and
/// paint pass of the same frame.
///
/// The application does not see this state. If a piece of ephemeral state becomes
/// something the app must know, it stops being ephemeral and becomes a tier
/// emission — not a field the app reads out of the widget.
#[derive(Clone, Debug, Default)]
pub struct Ephemeral<S> {
    state: S,
}

impl<S> Ephemeral<S> {
    /// Start from an explicit initial value.
    #[must_use]
    pub fn new(state: S) -> Self {
        Self { state }
    }

    /// Read the current value.
    #[must_use]
    pub fn get(&self) -> &S {
        &self.state
    }

    /// Borrow the value for in-place mutation during event handling.
    #[must_use]
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.state
    }

    /// Replace the value, returning what it was.
    pub fn set(&mut self, state: S) -> S {
        std::mem::replace(&mut self.state, state)
    }

    /// Restore the type's default, returning what was discarded.
    ///
    /// This is the "widget forgets it was ever hovered" path — the control is not
    /// reverted to an application value (it never had one) but to a blank slate.
    pub fn reset(&mut self) -> S
    where
        S: Default,
    {
        std::mem::take(&mut self.state)
    }
}

impl<S> From<S> for Ephemeral<S> {
    fn from(state: S) -> Self {
        Self::new(state)
    }
}

#[cfg(test)]
#[path = "controlled_tests.rs"]
mod controlled_tests;
