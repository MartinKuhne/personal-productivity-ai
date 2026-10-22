//! The focus bridge — the one-way path from the pure focus model to iced's
//! focus router (`UIF-PRIN-005`, decision **D-7**).
//!
//! # Why a bridge exists at all
//!
//! [`FocusState`] decides *which* control holds focus, but it decides it in
//! the dark: it knows ids and groups and traps and nothing about widgets.
//! iced's router is the thing that actually delivers focus into a widget
//! tree — a widget learns it is focused only because an [`Operation`]
//! reached it and called [`operation::Focusable::focus`] on the state it
//! registered. Something has to turn "the model says `dialog.save`" into
//! "the widget answering that id is now focused".
//!
//! That translation is this module, and it is **production code, not test
//! scaffolding**. Every focusable widget, every menu and eventually `fastmd`
//! itself pushes focus through here rather than re-deriving it, so the
//! rules below are enforced once for the whole framework instead of being
//! re-implemented — and re-gotten-wrong — at each call site.
//!
//! # The bridge runs strictly one way: model → router
//!
//! This is the constraint `INC-10` pinned and the one this module exists to
//! enforce, so it is spelled out at the function that obeys it
//! ([`focus_operation`]) rather than left to convention:
//!
//! * **The router has nowhere to put policy.** It tracks a flat, positional
//!   list of whatever registered itself this pass. It cannot hold groups,
//!   cannot nest a trap, cannot preserve an ordinal slot across a rebuild —
//!   every one of the decisions [`focus.rs`](crate::focus) had to make is a
//!   decision the router is structurally incapable of making.
//! * **Reading focus *up* out of the router would open a hole.** Anything
//!   that registers itself — a stock `text_input` inside a modal, a widget
//!   a third party mounted — would become a source of truth about where
//!   focus is, and the host would then move focus *against* the trap that
//!   is supposed to confine it. `UIF-MOD-001` says Tab must never reach
//!   the background under the scrim; a bridge that asked the router where
//!   focus was would have to include that background control in the answer.
//!
//! So the model is authoritative, the router is a projection of it, and
//! the projection runs after every policy transition. Nothing in this
//! module reads focus back. The read-back operations that exist
//! (`find_focused`, `is_focused`) live on the `#[cfg(test)]`
//! `test_helpers` harness instead, precisely so production code cannot
//! reach them by accident.
//!
//! # `Tab` is never handed to iced
//!
//! `operation::focusable::focus_next` **does not wrap — it *loses* focus**
//! at both ends: past the last registered widget it unfocuses the current
//! one and focuses nothing, and before the first it does the same.
//! `UIF-CMENU-003` requires a menu to *cycle*, so a keyboard move must
//! never be delegated to iced's traversal. [`tab_operation`] and
//! [`tab_task`] compute the target from [`FocusState::focus_next`] /
//! [`FocusState::focus_previous`] — which wrap, skip disabled members and
//! honour the active trap — and push that answer down with
//! `operation::focusable::focus(id)`.
//!
//! The guard test in the sidecar asserts mechanically that this module
//! never names iced's traversal operations, so the rule cannot be broken by
//! a later edit that looked like a simplification.
//!
//! # "Nothing is focusable" is a state, not a no-op
//!
//! When the model names no control, the bridge emits an explicit
//! `operation::focusable::unfocus` rather than nothing at all. "No
//! control holds focus" is a fact the router has to be told about: if the
//! bridge simply stayed silent, the control it was already showing stays
//! lit and the screen disagrees with the model. What the bridge never does
//! is emit `focus(id)` for an id the model does not name — a Tab the model
//! refuses (empty scope, everything disabled, a trap around a group that
//! has not mounted yet) leaves the router exactly where it was, because
//! the projection of an unchanged model is unchanged.
//!
//! Unit tests live in the sibling `bridge_tests.rs` sidecar.

use crate::focus::FocusId;
use crate::focus::FocusState;

use iced::Task;
use iced::advanced::widget::Id;
use iced::advanced::widget::operate;
use iced::advanced::widget::operation::{self, Operation};

/// The iced [`Id`] a [`FocusId`] is registered under.
///
/// The [`FocusId`]'s own string, so the model and the router name the same
/// control and a rebuilt widget re-claims the same router slot. Every
/// focusable widget derives its registration id through this function and
/// every bridge push addresses it through the same one: a bridge that
/// derived the id differently would push focus at an address no widget
/// answers to, and the failure would be silent — the operation would run,
/// find nothing, and leave focus wherever it last was.
#[must_use]
pub fn widget_id(focus_id: &FocusId) -> Id {
    Id::from(focus_id.as_str().to_owned())
}

/// Which way a `Tab` keystroke moves focus.
///
/// A named direction rather than a `bool` or a `+1 / -1` because both spell
/// the same call site ambiguously: `tab(model, true)` and `tab(model, 1)`
/// each require the reader to remember which end of the mapping Shift-Tab
/// lands on, and getting it backwards is invisible until someone presses
/// the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// `Tab` — forward through the traversal order.
    Forward,
    /// `Shift+Tab` — backward through the traversal order.
    Backward,
}

impl Direction {
    /// Map the Shift modifier of a `Tab` key press onto a direction.
    #[must_use]
    pub const fn from_shift(shifted: bool) -> Self {
        if shifted {
            Self::Backward
        } else {
            Self::Forward
        }
    }
}

/// The [`Operation`] that projects the model's current focus onto iced's
/// focus router (`UIF-PRIN-005`).
///
/// **This is the one-way door.** It reads [`FocusState::current`] and
/// writes it into the router; it never reads focus out of the router, and
/// the module [docs](self) explain why nothing may. Every keyboard move,
/// every click-to-focus and every programmatic focus in the framework ends
/// up here, so the direction is not a property of a particular caller —
/// it is a property of the framework.
///
/// A model with nothing focused pushes an explicit `unfocus` rather than
/// doing nothing: "no control holds focus" is a state the router has to be
/// told about too, or the control it was already showing stays lit.
///
/// Exposed as an [`Operation`] rather than only as a [`Task`] so a host can
/// compose it, and so the test harness can drive it against a real widget
/// tree without a running runtime.
pub fn focus_operation<Message: Send + 'static>(model: &FocusState) -> Box<dyn Operation<Message>> {
    match model.current() {
        Some(id) => Box::new(operation::focusable::focus::<Message>(widget_id(id))),
        None => Box::new(operation::focusable::unfocus::<Message>()),
    }
}

/// The same projection as [`focus_operation`], wrapped as a runtime
/// [`Task`] for a host that moves focus from `update` and has to hand the
/// work to iced rather than run it itself.
pub fn focus_task<Message: Send + 'static>(model: &FocusState) -> Task<Message> {
    operate(focus_operation(model))
}

/// Advance the model by one `Tab` step and project the result onto the
/// router.
///
/// The step is taken by [`FocusState`], never by iced: the model wraps at
/// both ends, skips disabled members and confines the walk to the active
/// trap, none of which the router does. What comes back is pushed with
/// `focus(id)` — see the [module docs](self) for why handing `Tab` to
/// `operation::focusable::focus_next` would drop focus off the bottom of
/// every menu in the framework.
///
/// A step the model refuses leaves [`FocusState::current`] untouched, and
/// the projection of an unchanged model is unchanged: the router stays
/// exactly where it was rather than being cleared.
pub fn tab_operation<Message: Send + 'static>(
    model: &mut FocusState,
    direction: Direction,
) -> Box<dyn Operation<Message>> {
    match direction {
        Direction::Forward => {
            model.focus_next();
        }
        Direction::Backward => {
            model.focus_previous();
        }
    }

    focus_operation(model)
}

/// [`tab_operation`] as a runtime [`Task`].
///
/// This is what a host returns from `update` when the user presses `Tab`
/// or `Shift+Tab`: compute the target from the model, hand iced the
/// projection, and let the next `view` paint the ring wherever the model
/// decided it should be.
pub fn tab_task<Message: Send + 'static>(
    model: &mut FocusState,
    direction: Direction,
) -> Task<Message> {
    operate(tab_operation(model, direction))
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod bridge_tests;
