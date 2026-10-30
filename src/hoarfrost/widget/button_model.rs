//! The declarative Button model — `UIF-ATOM-001a`'s `ButtonModel` (spec §3.1).
//!
//! A [`ButtonModel`] is plain data: what the button says, which variant it wears,
//! how busy it is, whether the host will settle the action it dispatches, and what
//! tooltip it owes when its label collapses away. It carries no interaction state
//! and no callbacks — the host owns one of these per button and hands it to
//! [`Button::controlled`] every frame (spec §3.0.4.1: "the model is the truth").
//!
//! The split this module enforces:
//!
//! - **Controlled** — the host owns the model, so the button reads it as
//!   [`Controlled`] (a shared reference, never a copy). See [`ButtonModel`] and
//!   [`controlled`](ButtonModel::controlled).
//! - **Ephemeral** — the button's own per-frame interaction state (which
//!   primitives are pressed, the [`ButtonController`] riding along) lives in the
//!   widget tree under the [`Ephemeral`] tag, so a rebuild can never resurrect it.
//!
//! Density is expressed as [`ButtonDensity`] and resolved through
//! [`ResponsiveLayout`] — the button never reads a live width and never invents a
//! breakpoint of its own ([`responsive`](ButtonModel::responsive)).
//!
//! [`Button::controlled`]: crate::widget::button_widget::Button::controlled
//! [`Button`]: crate::widget::button_widget::Button
//! [`Controlled`]: crate::binding::Controlled
//! [`Ephemeral`]: crate::binding::Ephemeral
//! [`ResponsiveLayout`]: crate::binding::ResponsiveLayout
//! [`ButtonController`]: crate::widget::button_controller::ButtonController

use crate::binding::{
    ActionPending, Breakpoint, Controlled, LabelMode, Measurement, ResponsiveLayout,
};
use crate::token::size;
use crate::token::space;
use crate::widget::button::ButtonVariant;

/// The reserved spinner slot ([`size::SPINNER_SM`]).
///
/// The slot is **reserved, not painted**: the spinner glyph itself is
/// `UIF-ATOM-009`'s deliverable. Until that lands the slot is empty space, which
/// is the honest rendering of "an action is in flight and we have no glyph yet".
///
/// The rung is `size.spinner.sm`, not the `size.icon.sm` this constant used to
/// borrow. The two are the same `14` — the busy indicator occupies exactly the
/// small-icon box — so re-pointing it changes no painted pixel and no measured
/// width; what it changes is *which question the name answers*. A slot sized by
/// an icon constant says "as wide as a glyph"; one sized by `SPINNER_SM` says
/// "as wide as the spinner that will fill it", which is what §2.2 actually
/// specifies and what `UIF-ATOM-009` will be held to.
pub const SPINNER_SLOT: f32 = size::SPINNER_SM;

/// What a button can put in the leading or trailing icon slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ButtonIcon {
    /// A glyph from the icon font, painted through the variant's icon colour.
    Glyph(&'static str),
    /// A reserved, currently empty slot — occupies [`SPINNER_SLOT`] of width and
    /// nothing more. Used where an icon is planned but not supplied.
    Reserved,
}

impl ButtonIcon {
    /// The width this icon occupies.
    pub fn width(&self) -> f32 {
        match self {
            Self::Glyph(_) => size::ICON_SM,
            Self::Reserved => SPINNER_SLOT,
        }
    }
}

/// How much room the button was told it has.
///
/// The host declares this — normally from a real container query, or from the
/// [`Breakpoint`] of the region the button sits in. The button itself never
/// measures a live width to decide: §3.0.4.4 requires adaptation "based on
/// **allocated parent container width** via a pure solver taking no global
/// window-width parameter".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonDensity {
    /// Full rung: label plus both icon slots.
    #[default]
    Standard,
    /// Compact rung: icon-only, and therefore tooltip-mandatory.
    Compact,
}

impl ButtonDensity {
    /// The control height corresponding to this density rung (§3.1, `UIF-ATOM-001a`).
    #[must_use]
    pub const fn height(self) -> f32 {
        match self {
            Self::Standard => size::CONTROL_MD,
            Self::Compact => size::CONTROL_SM,
        }
    }

    /// The [`Breakpoint`] this density asks the responsive layout to resolve.
    pub fn breakpoint(self) -> Breakpoint {
        match self {
            Self::Standard => Breakpoint::Expanded,
            Self::Compact => Breakpoint::Compact,
        }
    }

    /// Is this the collapsed rung?
    pub fn is_collapsed(self) -> bool {
        matches!(self, Self::Compact)
    }
}

/// The declarative state of one button (`UIF-ATOM-001a`).
///
/// ```text
/// label ─────────────────────────────── what it says
/// leading_icon / trailing_icon ──────── what frames the label
/// variant ──────────────────────────── how it is painted
/// density ──────────────────────────── how much room the host declared
/// busy ─────────────────────────────── an action is in flight
/// disabled ─────────────────────────── the host refuses interaction
/// cancellable ──────────────────────── the host can stop a dispatched action
/// tooltip ──────────────────────────── what it owes when the label is gone
/// ```
///
/// ## The Controlled / Ephemeral split
///
/// Everything here is host-owned, so the button takes it as
/// [`Controlled<'_, ButtonModel>`](Controlled) and reads it without copying. What
/// is *not* here — pressed, hovered, focused, the activation gate — is the
/// button's [`Ephemeral`] half, stored in the widget tree and rebuilt from zero
/// whenever the tree is.
///
/// ## `cancellable`
///
/// §3.0.4.3 states the condition outright: a busy control "emits
/// `Cancel<CancelRequest>` (`on_cancel_request()`) on press or `Escape` if and
/// only if `cancellable == true`". The model makes that enforceable rather than
/// advisory: [`ActionPending::try_cancel`] can only grant a
/// cancellation the host actually declared. A model that never sets
/// [`cancellable`](Self::cancellable) can still be cancelled *before* it
/// dispatches (a drag-out is always allowed), but once the action is in flight
/// the request comes back [`NotCancellable`] — which is the truth about a
/// fire-and-forget command, not a bug.
///
/// [`ActionPending::try_cancel`]: crate::binding::ActionPending::try_cancel
/// [`NotCancellable`]: crate::binding::CancelRefusal::NotCancellable
/// [`Ephemeral`]: crate::binding::Ephemeral
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ButtonModel {
    /// The button's text. `None` makes the button icon-only at every rung.
    pub label: Option<String>,
    /// Leading icon.
    pub leading_icon: Option<ButtonIcon>,
    /// Trailing icon.
    pub trailing_icon: Option<ButtonIcon>,
    /// The visual variant.
    pub variant: ButtonVariant,
    /// How much room the host declared.
    pub density: ButtonDensity,
    /// The host reports an action in flight.
    pub busy: bool,
    /// The host refuses interaction.
    pub disabled: bool,
    /// The host can settle a dispatched action with a cancellation.
    pub cancellable: bool,
    /// The tooltip shown when the label is collapsed away.
    pub tooltip: Option<String>,
}

impl ButtonModel {
    /// A text button in the standard density.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
            leading_icon: None,
            trailing_icon: None,
            variant: ButtonVariant::Secondary,
            density: ButtonDensity::Standard,
            busy: false,
            disabled: false,
            cancellable: false,
            tooltip: None,
        }
    }

    /// An icon-only button — which makes [`tooltip`](Self::tooltip) mandatory.
    ///
    /// Rests as a [`Ghost`], the quiet toolbar form.
    ///
    /// [`Ghost`]: ButtonVariant::Ghost
    pub fn icon_only(icon: &'static str) -> Self {
        Self {
            label: None,
            leading_icon: Some(ButtonIcon::Glyph(icon)),
            trailing_icon: None,
            variant: ButtonVariant::Ghost,
            ..Self::new("")
        }
    }

    /// Sets the variant.
    pub fn with_variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets the leading icon.
    pub fn leading(mut self, icon: ButtonIcon) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    /// Sets the trailing icon.
    pub fn trailing(mut self, icon: ButtonIcon) -> Self {
        self.trailing_icon = Some(icon);
        self
    }

    /// Declares the density the host has room for.
    pub fn with_density(mut self, density: ButtonDensity) -> Self {
        self.density = density;
        self
    }

    /// Declares the tooltip shown when the label collapses.
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Declares that an action is in flight.
    pub fn busy(mut self) -> Self {
        self.busy = true;
        self
    }

    /// Declares that the host can settle a dispatched action with a cancellation.
    pub fn cancellable(mut self) -> Self {
        self.cancellable = true;
        self
    }

    /// Declares that the host refuses interaction.
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Wraps this model as host-owned state.
    pub fn controlled(&self) -> Controlled<'_, ButtonModel> {
        Controlled::new(self)
    }

    /// Does this model have an action the button should gate?
    ///
    /// A button gates when the host says work is underway ([`busy`]) **or** when
    /// the host says it can settle what gets dispatched ([`cancellable`]). Both
    /// mean "a `Commit` from this button starts something the host will report
    /// back on", which is exactly what [`ActivationGate`] exists to police.
    ///
    /// A model with neither is a synchronous command: the gate stays shut, so the
    /// button fires every press instead of latching forever on an action nobody
    /// will ever settle.
    ///
    /// [`busy`]: Self::busy
    /// [`cancellable`]: Self::cancellable
    /// [`ActivationGate`]: crate::binding::ActivationGate
    pub fn tracks_action(&self) -> bool {
        self.busy || self.cancellable
    }

    /// The pending-action value this model projects, as a fresh value per frame.
    ///
    /// This is the whole of the button's time-and-busy contract: the model's
    /// `busy` flag becomes an [`ActionPending`] the widget's activation gate can
    /// be asked about. No clock is read, and no duration is stored — `busy` is a
    /// snapshot of the host's state at the moment the frame was built.
    ///
    /// [`ActionPending`]: crate::binding::ActionPending
    pub fn pending(&self) -> ActionPending {
        let pending = if self.busy {
            ActionPending::indeterminate()
        } else {
            ActionPending::idle()
        };
        pending.with_cancellable(self.cancellable)
    }

    /// Is this button icon-only at the given rung?
    ///
    /// Icon-only means the label is not being shown, which is precisely when the
    /// tooltip becomes mandatory — see [`LabelMode::requires_tooltip`].
    pub fn is_icon_only(&self, mode: LabelMode) -> bool {
        mode.requires_tooltip()
    }

    /// The tooltip this button must show at the given rung, if any.
    ///
    /// `None` on the full rung: the label is already on screen, so there is
    /// nothing to say twice. On the collapsed rung it returns the model's
    /// tooltip — and a collapsed rung with no tooltip is a violation the caller
    /// must surface rather than render.
    pub fn tooltip_for(&self, mode: LabelMode) -> Option<&str> {
        if self.is_icon_only(mode) {
            self.tooltip.as_deref()
        } else {
            None
        }
    }

    /// The narrowest width the collapsed rung fits at: both icon slots, the
    /// spinner slot, and the compact padding that keeps them off the edge.
    pub fn collapsed_min_width(&self) -> f32 {
        self.icon_width() + SPINNER_SLOT + 2.0 * space::SM
    }

    /// The control's own floor on the full rung: both icon slots, the spinner
    /// slot and the expanded padding.
    ///
    /// The label is deliberately **not** in this number. Measuring a string
    /// needs a font context, and a declarative model does not own one; a
    /// character-count proxy would be a made-up measurement that drifts the
    /// moment the typeface changes. A host that has measured its label adds the
    /// real number through [`measurement_with_label`](Self::measurement_with_label).
    pub fn expanded_min_width(&self) -> f32 {
        self.icon_width() + SPINNER_SLOT + 2.0 * space::MD
    }

    /// The floor pair with a measured label width added to the full rung.
    pub fn measurement_with_label(&self, label_width: f32) -> Measurement {
        Measurement::new(
            self.expanded_min_width() + label_width.max(0.0),
            self.collapsed_min_width(),
        )
    }

    /// Combined width of the two icon slots.
    fn icon_width(&self) -> f32 {
        self.leading_icon.as_ref().map_or(0.0, ButtonIcon::width)
            + self.trailing_icon.as_ref().map_or(0.0, ButtonIcon::width)
    }

    /// What this button's content needs at each rung, for a host running a real
    /// container query.
    pub fn measurement(&self) -> Measurement {
        Measurement::new(self.expanded_min_width(), self.collapsed_min_width())
    }

    /// The [`ResponsiveLayout`] for the density this host declared.
    ///
    /// The declared density is turned into the allocation that sits
    /// definitionally on that rung — the full rung's own width for
    /// [`Standard`], one step short of it for [`Compact`] — and
    /// [`ResponsiveLayout::solve`] answers. The button never bypasses the solver
    /// to assert a rung: it asks the same question a live container query asks
    /// and gets the same kind of answer. It never asks a window, a surface or an
    /// ambient context anything.
    ///
    /// [`Standard`]: ButtonDensity::Standard
    /// [`Compact`]: ButtonDensity::Compact
    pub fn responsive(&self) -> ResponsiveLayout {
        let measurement = self.measurement();
        let allocated = match self.density {
            ButtonDensity::Standard => measurement.expanded_width(),
            ButtonDensity::Compact => measurement.expanded_width() - RUNG_PROBE,
        };
        ResponsiveLayout::solve(allocated, &measurement)
    }

    /// The live container query: the rung for a width the host actually has.
    ///
    /// A host that knows the pixels its container got should prefer this over
    /// [`responsive`](Self::responsive), which answers from a declared density
    /// because it has no width to ask with.
    pub fn responsive_to(&self, allocated: f32) -> ResponsiveLayout {
        ResponsiveLayout::solve(allocated, &self.measurement())
    }
}

/// One step short of the expanded rung — the allocation that means `Compact`.
const RUNG_PROBE: f32 = 1.0;

#[cfg(test)]
#[path = "button_model_tests.rs"]
mod button_model_tests;

/// A button model that is host-owned, i.e. [`Controlled`].
///
/// Re-exported here so a caller building a button reads
/// `Controlled<'_, ButtonModel>` without reaching for the binding module.
pub type ControlledModel<'a> = Controlled<'a, ButtonModel>;
