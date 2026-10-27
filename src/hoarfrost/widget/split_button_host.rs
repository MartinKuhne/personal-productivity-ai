//! The split-button host — one menu cascade, one button controller
//! (§3.1c, `UIF-ATOM-001c`).
//!
//! This is where the two halves of the control meet, and the whole point of
//! the increment is *what this module is not allowed to do*: it may not
//! own a second menu, and it may not invent a second activation engine.
//! So it owns exactly one [`MenuCascade`] and one [`ButtonController`],
//! and every behaviour below is those two existing types being driven.
//!
//! # The menu is the crate's, opened through the crate's engine
//!
//! [`SplitButtonHost::open_menu`] calls [`MenuCascade::toggle`] with
//! [`MenuTreeKind::Fixed`] — the same entry point the navbar trigger uses.
//! That buys `UIF-CMENU-002`'s mutual exclusion for free: opening a split
//! button's flyout retires any context menu that was showing, and vice
//! versa, because both go through the one cascade rather than each keeping
//! its own popup.
//!
//! # Escape returns to the control because the control was focused first
//!
//! The cascade's `mount` pushes a focus trap that records whatever held
//! focus at push time, and `close_tree` pops it back. So
//! [`SplitButtonHost::open_menu`] focuses the split button *before*
//! asking the cascade to open, which makes the recorded focus the control
//! itself. Escape then lands the caret back on the split button — and
//! [`SplitButtonHost::escape`] emits nothing for the primary half on the
//! way out, which is the difference between "cancel" and "the menu closed
//! by pressing the button again".
//!
//! If the control was not focusable when the menu opened, the trap
//! records nothing and focus clears on close. That is the trap's existing
//! contract, not a fallback added here.
//!
//! # The chevron opens on *release*
//!
//! §3.1c says "chevron click". Under §3.0.3.3 a click is press-and-
//! release inside the same hit target, so
//! [`SplitButtonHost::press`] on the chevron does nothing at all and the
//! toggle happens in [`SplitButtonHost::release`] — and only if the
//! release is still inside the chevron zone. A press that slides off and
//! lets go over the label opens nothing and fires nothing.
//!
//! # The halves never borrow each other's state
//!
//! [`SplitButtonHost::hover`] records one zone;
//! [`SplitButtonHost::half_state`] reads it back per half. There is no
//! path by which hovering the label puts `Hovered` on the chevron's
//! state, and no path by which pressing the label reaches the cascade. The
//! sidecar asserts both directions rather than trusting the shape of the
//! struct.
//!
//! Unit tests live in the sibling `split_button_host_tests.rs` sidecar.

use std::fmt;

use crate::binding::{Cancel, CancelRequest, Commit};
use crate::focus::{FocusId, FocusState};
use crate::state::{BaseState, VisualState};
use crate::surface::menu::cascade::{CascadeOutcome, MenuCascade, MenuKey, MenuTreeKind};
use crate::surface::menu::model::ActivationEffect;
use crate::widget::button_controller::{
    ActivationSource, ButtonController, ButtonEmission, ButtonSettlement,
};
use crate::widget::split_button::SplitZone;
use crate::widget::split_button_model::SplitButtonModel;

/// One piece of output the host produced for the application.
///
/// A sum type rather than a bag of optional callbacks, for the same reason
/// [`ButtonEmission`] is one: a host wiring all of these is forced to
/// write an arm for each, and cannot silently drop the one it did not
/// demo.
#[derive(Clone)]
pub enum SplitButtonEmission<ActionId> {
    /// The primary half dispatched a command.
    PrimaryActivated(Commit<ButtonSettlement>),
    /// A primary gesture was abandoned before dispatching.
    PrimaryCancelled(Cancel<ButtonSettlement>),
    /// The user asked to stop a running primary action.
    StopRequested(CancelRequest),
    /// The flyout opened (`true`) or closed (`false`) — `on_menu_toggle`.
    MenuToggled(bool),
    /// A flyout row fired its application command — `on_menu_select`.
    MenuSelected(ActionId),
}

impl<ActionId> SplitButtonEmission<ActionId> {
    /// The settlement carried by the primary half's tiers, when there was one.
    #[must_use]
    pub fn settlement(&self) -> Option<&ButtonSettlement> {
        match self {
            Self::PrimaryActivated(commit) => Some(commit.value()),
            Self::PrimaryCancelled(cancel) => Some(cancel.restored()),
            Self::StopRequested(_) | Self::MenuToggled(_) | Self::MenuSelected(_) => None,
        }
    }

    /// Whether this emission came from the primary half rather than the menu.
    #[must_use]
    pub fn is_primary(&self) -> bool {
        matches!(
            self,
            Self::PrimaryActivated(_) | Self::PrimaryCancelled(_) | Self::StopRequested(_)
        )
    }
}

impl<ActionId: PartialEq> PartialEq for SplitButtonEmission<ActionId> {
    /// Compares the payload each tier carries.
    ///
    /// Hand-written because [`Commit`] and [`Cancel`] are linear wrappers
    /// that deliberately do not implement `PartialEq`. Matching the tiers
    /// first means an activation can never compare equal to a cancellation.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::PrimaryActivated(a), Self::PrimaryActivated(b)) => a.value() == b.value(),
            (Self::PrimaryCancelled(a), Self::PrimaryCancelled(b)) => a.restored() == b.restored(),
            (Self::StopRequested(a), Self::StopRequested(b)) => a == b,
            (Self::MenuToggled(a), Self::MenuToggled(b)) => a == b,
            (Self::MenuSelected(a), Self::MenuSelected(b)) => a == b,
            _ => false,
        }
    }
}

impl<ActionId: fmt::Debug> fmt::Debug for SplitButtonEmission<ActionId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrimaryActivated(commit) => f
                .debug_tuple("PrimaryActivated")
                .field(commit.value())
                .finish(),
            Self::PrimaryCancelled(cancel) => f
                .debug_tuple("PrimaryCancelled")
                .field(cancel.restored())
                .finish(),
            Self::StopRequested(request) => f.debug_tuple("StopRequested").field(request).finish(),
            Self::MenuToggled(open) => f.debug_tuple("MenuToggled").field(open).finish(),
            Self::MenuSelected(id) => f.debug_tuple("MenuSelected").field(id).finish(),
        }
    }
}

/// Everything one host input did.
///
/// The cascade's own report travels alongside the emissions rather than
/// being flattened into them: the caller still needs `opened` and `closed`
/// to mount and tear down the flyout surface, and re-deriving those from a
/// `MenuToggled` bool would lose which card moved.
#[derive(Clone, Debug)]
pub struct SplitButtonOutcome<ActionId> {
    /// The engine's report for the cascade side of the input.
    pub cascade: CascadeOutcome,
    /// What the application is being told, in the order it happened.
    pub emissions: Vec<SplitButtonEmission<ActionId>>,
}

impl<ActionId> SplitButtonOutcome<ActionId> {
    /// An outcome that says nothing happened.
    #[must_use]
    pub fn noop() -> Self {
        Self {
            cascade: CascadeOutcome::noop(),
            emissions: Vec::new(),
        }
    }

    /// An outcome carrying `emissions` and no cascade movement.
    #[must_use]
    pub fn emissions(emissions: Vec<SplitButtonEmission<ActionId>>) -> Self {
        Self {
            cascade: CascadeOutcome::noop(),
            emissions,
        }
    }

    /// Whether this outcome changed nothing at all.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.emissions.is_empty()
            && self.cascade.opened().is_empty()
            && self.cascade.closed().is_empty()
            && self.cascade.activation().is_none()
    }
}

/// The per-control memory a split button needs: the flyout's cascade, the
/// primary half's activation engine, and which half the pointer is on.
#[derive(Debug)]
pub struct SplitButtonHost {
    /// The one menu engine this control is allowed to have.
    cascade: MenuCascade,
    /// The one activation engine the primary half runs on.
    primary: ButtonController,
    /// The control's focus identity — what the trap is told to come back to.
    control: FocusId,
    /// The half the pointer is currently over.
    hovered: SplitZone,
    /// The half a press started in.
    pressed: Option<SplitZone>,
}

impl SplitButtonHost {
    /// A host for the control focusable as `control`.
    #[must_use]
    pub fn new(control: FocusId) -> Self {
        Self {
            cascade: MenuCascade::new(),
            primary: ButtonController::new(),
            control,
            hovered: SplitZone::Outside,
            pressed: None,
        }
    }

    /// Record the model's current projection. Call once per frame, before
    /// any input.
    pub fn observe<ActionId: Clone + fmt::Display>(&mut self, model: &SplitButtonModel<ActionId>) {
        self.primary.observe(model.primary_model());
    }

    /// Whether the flyout is showing.
    #[must_use]
    pub fn is_menu_open(&self) -> bool {
        self.cascade.is_open()
    }

    /// The half a press started in, if one is open.
    #[must_use]
    pub const fn pressed_zone(&self) -> Option<SplitZone> {
        self.pressed
    }

    /// The half the pointer is over.
    #[must_use]
    pub const fn hovered_zone(&self) -> SplitZone {
        self.hovered
    }

    /// The visual state of one half.
    ///
    /// Reads the pointer state **for that half only**. `focused` is
    /// control-level and passed in, because the ring belongs to the whole
    /// control the way §3.0.2 says it does, not to whichever half the
    /// keyboard happens to be nearest.
    #[must_use]
    pub fn half_state(&self, half: SplitZone, focused: bool) -> VisualState {
        let base = if self.pressed == Some(half) {
            BaseState::Pressed
        } else if self.hovered == half {
            BaseState::Hovered
        } else {
            BaseState::Rest
        };
        VisualState { base, focused }
    }

    /// The cascade, read-only.
    #[must_use]
    pub const fn cascade(&self) -> &MenuCascade {
        &self.cascade
    }

    /// The primary half's controller, read-only.
    #[must_use]
    pub const fn primary(&self) -> &ButtonController {
        &self.primary
    }

    /// The control's focus identity.
    #[must_use]
    pub const fn control(&self) -> &FocusId {
        &self.control
    }

    // -- pointer ---------------------------------------------------------

    /// Move the pointer onto `zone`.
    ///
    /// Deliberately inert: hover paints, it does not emit. Setting this is
    /// the only thing that can make a half read as `Hovered`, and it can
    /// only ever be the half the pointer actually entered.
    pub fn hover(&mut self, zone: SplitZone) {
        self.hovered = zone;
    }

    /// The pointer left the control entirely.
    pub fn pointer_leave(&mut self) {
        self.hovered = SplitZone::Outside;
    }

    /// A pointer press started in `zone` (`UIF-ATOM-001c`).
    ///
    /// The chevron half produces **nothing** here. Opening on press would
    /// make a press-and-drag onto a menu row a different gesture than a
    /// click, and would fire the flyout for a press the user meant to
    /// abort. See the [module docs](self#the-chevron-opens-on-release).
    ///
    /// An inert control opens no gesture at all: [`ButtonController`]
    /// gates *busy*, not *dead*, so the refusal belongs here rather than
    /// two layers down.
    pub fn press<ActionId: Clone + fmt::Display>(
        &mut self,
        zone: SplitZone,
        model: &SplitButtonModel<ActionId>,
    ) -> SplitButtonOutcome<ActionId> {
        if !zone.is_hit() || model.disabled_flag() {
            return SplitButtonOutcome::noop();
        }
        self.pressed = Some(zone);
        match zone {
            SplitZone::Primary => SplitButtonOutcome::emissions(
                self.primary
                    .begin(model.primary_model(), ActivationSource::Pointer)
                    .into_iter()
                    .map(map_button)
                    .collect(),
            ),
            SplitZone::Menu | SplitZone::Outside => SplitButtonOutcome::noop(),
        }
    }

    /// A pointer press ended over `zone` (`UIF-ATOM-001c`).
    ///
    /// The gesture that started in one half is settled by where it *ended*:
    /// released in the same half it started, that half acts; released
    /// anywhere else, the primary half cancels and the chevron opens
    /// nothing.
    pub fn release<ActionId: Clone + fmt::Display>(
        &mut self,
        zone: SplitZone,
        model: &SplitButtonModel<ActionId>,
        focus: &mut FocusState,
    ) -> SplitButtonOutcome<ActionId> {
        let Some(started) = self.pressed.take() else {
            return SplitButtonOutcome::noop();
        };
        if model.disabled_flag() {
            // The control died under an open gesture. Neither a commit nor
            // a revert is owed: nothing was dispatched and nothing can be.
            self.primary.reset();
            return SplitButtonOutcome::noop();
        }
        match started {
            SplitZone::Primary => {
                let emission = if zone == SplitZone::Primary {
                    self.primary
                        .commit(model.primary_model(), ActivationSource::Pointer)
                } else {
                    self.primary.cancel(model.primary_model())
                };
                SplitButtonOutcome::emissions(emission.into_iter().map(map_button).collect())
            }
            SplitZone::Menu => {
                if zone == SplitZone::Menu {
                    self.toggle_menu(model, focus)
                } else {
                    SplitButtonOutcome::noop()
                }
            }
            SplitZone::Outside => SplitButtonOutcome::noop(),
        }
    }

    // -- keyboard --------------------------------------------------------

    /// `Enter` / `Space` on the primary half (`on_primary_activate`).
    ///
    /// Refused outright for an inert control — a keyboard activation has
    /// no press behind it to refuse first, so the guard cannot live only
    /// in [`press`](Self::press).
    pub fn activate_primary<ActionId: Clone + fmt::Display>(
        &mut self,
        model: &SplitButtonModel<ActionId>,
    ) -> SplitButtonOutcome<ActionId> {
        if model.disabled_flag() {
            return SplitButtonOutcome::noop();
        }
        SplitButtonOutcome::emissions(
            self.primary
                .commit(model.primary_model(), ActivationSource::Keyboard)
                .into_iter()
                .map(map_button)
                .collect(),
        )
    }

    /// Open the flyout, focusing the control first so Escape has
    /// somewhere to return to (`on_menu_toggle(true)`).
    pub fn open_menu<ActionId: Clone + fmt::Display>(
        &mut self,
        model: &SplitButtonModel<ActionId>,
        focus: &mut FocusState,
    ) -> SplitButtonOutcome<ActionId> {
        if model.disabled_flag() || !model.has_navigable_menu() {
            return SplitButtonOutcome::noop();
        }
        if focus.current() != Some(&self.control) {
            focus.focus(&self.control);
        }
        let cascade = self.cascade.open(&model.menu(), MenuTreeKind::Fixed, focus);
        if cascade.ignored_reason().is_some() {
            return SplitButtonOutcome {
                cascade,
                emissions: Vec::new(),
            };
        }
        SplitButtonOutcome {
            cascade,
            emissions: vec![SplitButtonEmission::MenuToggled(true)],
        }
    }

    /// Close the flyout (`on_menu_toggle(false)`).
    pub fn close_menu<ActionId>(&mut self, focus: &mut FocusState) -> SplitButtonOutcome<ActionId> {
        if !self.cascade.is_open() {
            return SplitButtonOutcome::noop();
        }
        let cascade = self.cascade.close_tree(focus);
        SplitButtonOutcome {
            cascade,
            emissions: vec![SplitButtonEmission::MenuToggled(false)],
        }
    }

    /// Toggle the flyout from the chevron.
    fn toggle_menu<ActionId: Clone + fmt::Display>(
        &mut self,
        model: &SplitButtonModel<ActionId>,
        focus: &mut FocusState,
    ) -> SplitButtonOutcome<ActionId> {
        if self.cascade.is_open() {
            self.close_menu(focus)
        } else {
            self.open_menu(model, focus)
        }
    }

    /// Drive one menu key through the engine (`UIF-CMENU-003`).
    ///
    /// The row the engine activated is translated back into the
    /// application's own `ActionId` here, so nothing downstream has to
    /// know that the menu engine keys its rows by string.
    pub fn menu_key<ActionId: Clone + fmt::Display>(
        &mut self,
        key: MenuKey,
        model: &SplitButtonModel<ActionId>,
        focus: &mut FocusState,
    ) -> SplitButtonOutcome<ActionId> {
        let was_open = self.cascade.is_open();
        let cascade = self.cascade.on_key(key, focus);
        let mut emissions = Vec::new();
        if let Some(activation) = cascade.activation()
            && !activation.is_absorbed()
            && !matches!(activation.effect, ActivationEffect::OpenSubmenu { .. })
            && let Some(id) = model.action_at(activation.row)
        {
            emissions.push(SplitButtonEmission::MenuSelected(id.clone()));
        }
        if was_open && !self.cascade.is_open() {
            emissions.push(SplitButtonEmission::MenuToggled(false));
        }
        SplitButtonOutcome { cascade, emissions }
    }

    /// `Escape`, with the precedence a user expects.
    ///
    /// An open flyout is dismissed first, and dismissing it is *not* a
    /// primary activation — the primary half is not even consulted on
    /// this path. Only with no menu open does `Escape` fall through to the
    /// primary half's cancel tier.
    pub fn escape<ActionId: Clone + fmt::Display>(
        &mut self,
        model: &SplitButtonModel<ActionId>,
        focus: &mut FocusState,
    ) -> SplitButtonOutcome<ActionId> {
        if self.cascade.is_open() {
            return self.close_menu(focus);
        }
        if !self.primary.gesture_open() {
            return SplitButtonOutcome::noop();
        }
        SplitButtonOutcome::emissions(
            self.primary
                .cancel(model.primary_model())
                .into_iter()
                .map(map_button)
                .collect(),
        )
    }

    /// Drop the pointer state and any open primary gesture without
    /// emitting, leaving the flyout alone.
    ///
    /// For the path where the control is torn down or disabled mid-gesture.
    /// A live cascade is deliberately *not* closed here — closing a flyout
    /// is a visible event the host has to be able to emit for, and
    /// silently tearing one down from a reset would drop that report.
    pub fn reset(&mut self) {
        self.hovered = SplitZone::Outside;
        self.pressed = None;
        self.primary.reset();
    }
}

/// Map the button atom's tiers onto this control's emission set.
fn map_button<ActionId>(emission: ButtonEmission) -> SplitButtonEmission<ActionId> {
    match emission {
        ButtonEmission::Activated(commit) => SplitButtonEmission::PrimaryActivated(commit),
        ButtonEmission::Cancelled(cancel) => SplitButtonEmission::PrimaryCancelled(cancel),
        ButtonEmission::CancelRequested(request) => SplitButtonEmission::StopRequested(request),
    }
}

#[cfg(test)]
#[path = "split_button_host_tests.rs"]
mod split_button_host_tests;
