//! The teaching-popover host — the frame that makes §3.7's interactive
//! callout behave (`UIF-ATOM-007`, `UIF-ZORD-001`).
//!
//! The host owns four things: the card rectangle, the model, the fact
//! that it is open, and the focus trap it mounted. The focus group and
//! the z-order surface are registered *into* collaborators the caller
//! passes in, because both are shared with the rest of the window — the
//! same split [`ModalHost`](crate::surface::modal::ModalHost) uses.
//!
//! # It enters the registry, and the tooltip's reasons for staying out do
//! not apply
//!
//! [`tooltip`](crate::surface::tooltip) deliberately does **not** join
//! [`SurfaceRegistry`], and its three reasons are good ones *for a
//! tooltip*. A teaching popover inverts all three, which is why it goes
//! in:
//!
//! 1. **The tooltip cannot answer [`UnwindAnswer`]; the popover answers it
//!    naturally.** A tooltip must dismiss and still hand `Escape` to
//!    whatever is underneath, and no `UnwindAnswer` arm means that. The
//!    popover *consumes* `Escape` — §3.7 puts it at `Escape Rank 3`,
//!    which is a statement about the unwind walk, not about a
//!    pre-pass. `UnwindAnswer::Dismissed` is exactly its answer.
//! 2. **Moving `topmost()` and `keyboard_owner()` is the point, not a
//!    hazard.** The tooltip's objection was that registering would
//!    change how *existing* bands route. The popover is supposed to
//!    change them: while a callout is open, Band 2 owns the keyboard,
//!    and that is the behaviour being asked for.
//! 3. **The popover answers every question a band is asked.** Backdrop:
//!    `TransparentCatcher`. Pointer rule: `CaptureInsideDismissOutside`.
//!    Escape rank: `3`. Nothing has to be faked and nothing is
//!    `None`.
//!
//! The distinguishing property is **interactivity**. Pointer-transparency
//! is what forces the tooltip out of a model built around pointer
//! ownership; click-away ownership is what forces the popover in.
//!
//! # It pushes its own trap, so it nests inside a modal
//!
//! [`open`](PopoverHost::open) mounts the way `MenuCascade::mount`
//! does — `register_group`, then `push_trap`, then focus the initial
//! control — so a callout opened *inside* a modal pushes onto the
//! modal's existing trap rather than replacing it. `Escape` closes the
//! callout and leaves the modal holding the focus it had before the
//! callout opened; the modal's own trap is untouched and still one pop
//! deep.
//!
//! [`SurfaceRegistry`]: crate::surface::zorder::SurfaceRegistry
//! [`UnwindAnswer`]: crate::surface::zorder::UnwindAnswer

use crate::focus::{FocusId, FocusState};
use crate::geometry::anchor::Placement;
use crate::surface::popover::model::{PopoverModel, dismiss_focus_id, group_for, primary_focus_id};
use crate::surface::popover::place;
use crate::surface::zorder::{
    DeclineReason, EscapeOutcome, SurfaceId, SurfaceKind, SurfaceRegistry, UnwindAnswer,
};
use iced::{Point, Rectangle, Size};

/// Why a callout closed.
///
/// Five causes, and the distinction matters downstream: a host that logs
/// a user dismissing a tour differently from a host that logs it acting
/// on one is a host that can tell whether its teaching worked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DismissCause {
    /// The primary footer button was pressed. The callout closes *with*
    /// the action, because a teaching popover that stayed open after
    /// being acted on would be teaching something already done.
    PrimaryAction,
    /// The dismiss button was pressed.
    DismissButton,
    /// `Escape` was pressed and **consumed** — the popover's Band 2 rank
    /// means nothing underneath sees it.
    Escape,
    /// The pointer went down outside the card, caught by the Band 2
    /// transparent catcher.
    ClickOutside,
    /// The application closed it.
    Programmatic,
}

/// Why a callout did not close, or did not open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IgnoredReason {
    /// The host asked something of a callout that is not open.
    NotOpen,
    /// The primary button was pressed on a callout that has no primary
    /// action. Not an error — a host may wire both buttons uniformly —
    /// but it has no meaning.
    NoPrimaryAction,
    /// A pointer event landed inside the card, where the button under the
    /// cursor owns it rather than the host.
    InsideCard,
}

/// What a callout did in response to one input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PopoverEvent<ActionId> {
    /// The primary action fired, and the callout closed with it.
    Action(ActionId),
    /// The callout closed.
    Dismissed(DismissCause),
    /// Nothing changed.
    Ignored(IgnoredReason),
}

impl<ActionId> PopoverEvent<ActionId> {
    /// Whether this event closed the callout.
    #[must_use]
    pub const fn closed_the_callout(&self) -> bool {
        matches!(self, Self::Action(_) | Self::Dismissed(_))
    }
}

/// Why [`open`](PopoverHost::open) refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OpenRefusal {
    /// The model's `open` flag was `false`. The host does not open what
    /// the host did not ask to open.
    NotRequested,
    /// The footer's initial control could not take focus. An empty or
    /// wholly-disabled scope would strand a focus ring under a
    /// click-away catcher, which is the leak `UIF-MOD-001` exists to
    /// prevent; refusing is better than mounting a trap nothing can be
    /// inside.
    NoFocusableControl,
}

/// The teaching-popover host (`UIF-ATOM-007`).
#[derive(Clone, Debug, PartialEq)]
pub struct PopoverHost<ActionId: Clone + PartialEq> {
    id: SurfaceId,
    group: crate::focus::GroupId,
    model: PopoverModel<ActionId>,
    trigger: Rectangle,
    viewport: Rectangle,
    card: Rectangle,
    placement: Placement,
    open: bool,
}

impl<ActionId: Clone + PartialEq> PopoverHost<ActionId> {
    /// Open a callout.
    ///
    /// In order: solve the card, register the footer's focus group, push
    /// the trap, move focus to the initial control, and register the
    /// Band 2 surface over the card.
    ///
    /// The trap goes on **before** focus is moved so the initial control
    /// is focused *through* it — a control outside the active scope is
    /// rejected rather than silently stealing focus from behind the trap.
    /// If focus cannot land, the group and the trap are rolled back and
    /// the open is refused: a callout with nowhere for the keyboard to
    /// go is a tooltip, not a teaching popover.
    ///
    /// No `#[must_use]`: the return is a `Result`, which is already
    /// `#[must_use]`, and a caller that ignores it is ignoring the
    /// refusal rather than a value.
    #[allow(clippy::result_large_err)]
    pub fn open(
        id: SurfaceId,
        model: PopoverModel<ActionId>,
        trigger: Rectangle,
        card_size: Size,
        viewport: Rectangle,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> Result<Self, OpenRefusal> {
        if !model.is_open_wanted() {
            return Err(OpenRefusal::NotRequested);
        }

        let placement = place::resolve(trigger, model.placement(), card_size, viewport);
        let card = placement.rect();
        let group = group_for();
        let initial = model.initial_focus();

        focus.register_group(group.clone(), model.footer_members());
        focus.push_trap(group.clone());

        if !focus.focus(&initial) {
            focus.pop_trap();
            focus.unregister_group(&group);
            return Err(OpenRefusal::NoFocusableControl);
        }

        registry.register(id.clone(), SurfaceKind::TeachingPopover, vec![card]);

        Ok(Self {
            id,
            group,
            model,
            trigger,
            viewport,
            card,
            placement,
            open: true,
        })
    }

    // -- identity ----------------------------------------------------------

    /// The callout's surface identity.
    #[must_use]
    pub fn id(&self) -> &SurfaceId {
        &self.id
    }

    /// The focus group the footer's buttons live in.
    #[must_use]
    pub fn group_id(&self) -> &crate::focus::GroupId {
        &self.group
    }

    /// The model this callout was opened with.
    #[must_use]
    pub fn model(&self) -> &PopoverModel<ActionId> {
        &self.model
    }

    /// The card rectangle as placed.
    #[must_use]
    pub const fn card(&self) -> Rectangle {
        self.card
    }

    /// The trigger the card is anchored to.
    #[must_use]
    pub const fn trigger(&self) -> Rectangle {
        self.trigger
    }

    /// The viewport the placement was solved against.
    #[must_use]
    pub const fn viewport(&self) -> Rectangle {
        self.viewport
    }

    /// The resolved placement, with its flip and clamp flags.
    #[must_use]
    pub const fn placement(&self) -> Placement {
        self.placement
    }

    /// Whether the callout is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Whether `at` is inside the card.
    #[must_use]
    pub fn is_inside_card(&self, at: Point) -> bool {
        self.card.contains(at)
    }

    /// The footer's two focus ids, primary first when it exists.
    #[must_use]
    pub fn footer_ids(&self) -> Vec<FocusId> {
        if self.model.has_primary_action() {
            vec![primary_focus_id(), dismiss_focus_id()]
        } else {
            vec![dismiss_focus_id()]
        }
    }

    // -- footer actions ----------------------------------------------------

    /// The primary footer button was pressed.
    ///
    /// Acting closes: the action is emitted and the callout tears down in
    /// the same step. A callout that stayed open after being acted on
    /// would sit there explaining something the user has already done.
    ///
    /// The pointer reaching this button at all is the point of the
    /// no-pointer-leave dismissal rule: the cursor travels from the
    /// trigger, across the [`place::POPOVER_GAP`] gap, onto the footer,
    /// and nothing along that path dismisses the card out from under it.
    #[must_use]
    pub fn activate_primary(&mut self, focus: &mut FocusState) -> PopoverEvent<ActionId> {
        if !self.open {
            return PopoverEvent::Ignored(IgnoredReason::NotOpen);
        }
        let Some((_, action)) = self.model.primary_action() else {
            return PopoverEvent::Ignored(IgnoredReason::NoPrimaryAction);
        };
        let action = action.clone();
        self.teardown(focus);
        PopoverEvent::Action(action)
    }

    /// The dismiss button was pressed.
    #[must_use]
    pub fn dismiss_dismiss_button(&mut self, focus: &mut FocusState) -> PopoverEvent<ActionId> {
        self.dismiss(DismissCause::DismissButton, focus)
    }

    /// Close the callout for `cause`, restoring focus through the trap.
    #[must_use]
    pub fn dismiss(
        &mut self,
        cause: DismissCause,
        focus: &mut FocusState,
    ) -> PopoverEvent<ActionId> {
        if !self.open {
            return PopoverEvent::Ignored(IgnoredReason::NotOpen);
        }
        self.teardown(focus);
        PopoverEvent::Dismissed(cause)
    }

    /// One pointer event, routed by where it landed.
    ///
    /// Inside the card the host does **not** act: the button under the
    /// cursor owns the event, and the host firing here would double-handle
    /// a footer press. Outside the card the Band 2 catcher has already
    /// decided the callout goes, so this dismisses with
    /// [`DismissCause::ClickOutside`].
    ///
    /// Note what is *absent*: pointer **leave** dismisses nothing. That
    /// is the difference between this and a tooltip, and it is what lets
    /// the cursor reach the footer at all.
    #[must_use]
    pub fn on_pointer(&mut self, at: Point, focus: &mut FocusState) -> PopoverEvent<ActionId> {
        if !self.open {
            return PopoverEvent::Ignored(IgnoredReason::NotOpen);
        }
        if self.is_inside_card(at) {
            return PopoverEvent::Ignored(IgnoredReason::InsideCard);
        }
        self.dismiss(DismissCause::ClickOutside, focus)
    }

    /// One `Escape`, handled by this host directly.
    ///
    /// Consumed. The tooltip's `Escape` dismisses *and* passes the key
    /// on; this one dismisses and stops, because §3.7 gives the callout
    /// `Escape Rank 3` — the first claim in the unwind walk.
    #[must_use]
    pub fn on_escape(&mut self, focus: &mut FocusState) -> PopoverEvent<ActionId> {
        self.dismiss(DismissCause::Escape, focus)
    }

    // -- teardown ----------------------------------------------------------

    /// Tear the callout down around its focus, without touching the
    /// registry.
    ///
    /// **Pop the trap, then unregister the group** — the order
    /// [`ModalHost::teardown`](crate::surface::modal::ModalHost::teardown)
    /// uses and for the same reason: popping restores focus to whatever
    /// this callout's push recorded, and that restore has to happen while
    /// the group it is leaving still exists.
    ///
    /// The pop is unconditional. The precondition is that this host's
    /// trap is the topmost one when it tears down, which holds because a
    /// teaching popover mounts no nested surface of its own and the
    /// escape walk orders same-rank surfaces newest-first.
    ///
    /// The registry is deliberately *not* part of this: the escape walk
    /// removes the surface itself, by index, and unregistering from
    /// inside that closure would shift the list under its feet.
    fn teardown(&mut self, focus: &mut FocusState) {
        focus.pop_trap();
        focus.unregister_group(&self.group);
        self.open = false;
    }

    /// Close the callout and unregister its surface.
    ///
    /// The programmatic path. Focus restoration is the same as any other
    /// close.
    #[must_use]
    pub fn close(
        &mut self,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> PopoverEvent<ActionId> {
        if !self.open {
            return PopoverEvent::Ignored(IgnoredReason::NotOpen);
        }
        self.teardown(focus);
        registry.unregister(&self.id);
        PopoverEvent::Dismissed(DismissCause::Programmatic)
    }

    // -- z-order integration -----------------------------------------------

    /// The callout's own answer when the escape chain asks it to unwind.
    ///
    /// `Dismissed` when open, `Declined(NotOpen)` once closed. There is
    /// no dirty half: a teaching popover holds no user work, so there is
    /// nothing `Escape` could destroy.
    ///
    /// **Must not mutate the registry** — the caller is normally
    /// [`SurfaceRegistry::escape`], which indexes the surface list and
    /// removes the entry it was told closed.
    #[must_use]
    pub fn unwind_answer(&mut self, focus: &mut FocusState) -> UnwindAnswer {
        if !self.open {
            return UnwindAnswer::Declined(DeclineReason::NotOpen);
        }
        self.teardown(focus);
        UnwindAnswer::Dismissed
    }

    /// Run one `Escape` through the z-order chain with this callout as
    /// the only surface it answers for.
    ///
    /// Every other surface is answered `Declined(NotOpen)` — "not mine,
    /// the key keeps travelling" — which is what lets a menu open above
    /// the callout unwind first without the callout knowing menus exist.
    pub fn escape_via(
        &mut self,
        registry: &mut SurfaceRegistry,
        focus: &mut FocusState,
    ) -> EscapeOutcome {
        let id = self.id.clone();
        registry.escape(move |surface| {
            if surface.id() != &id {
                return UnwindAnswer::Declined(DeclineReason::NotOpen);
            }
            self.unwind_answer(focus)
        })
    }
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod host_tests;
