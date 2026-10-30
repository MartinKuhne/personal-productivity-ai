//! The `SplitButton` / `DropDownButton` data model of §3.1c
//! (`UIF-ATOM-001c`).
//!
//! # The primary half *is* a [`ButtonModel`]
//!
//! §3.1c lists `label`, `leading_icon`, `variant`, `busy` and `disabled`
//! on `SplitButtonModel` — the same five fields `UIF-ATOM-001a` puts on
//! [`ButtonModel`]. Rather than restate them and keep two copies honest,
//! this model *contains* a `ButtonModel` and exposes it through
//! [`SplitButtonModel::primary`]. The split button's primary half is
//! therefore driven by the same busy suppression, `ActionPending`
//! projection and activation gating the button atom already has, and a
//! change to `001a`'s model moves the split button with it.
//!
//! The grouping is the only difference from the spec's flat field list,
//! and it is the difference that makes the composition real: `label` and
//! `variant` are read through the contained button, not copied out of it.
//!
//! # `menu_open` is a projection, not a second source of truth
//!
//! The open/closed fact lives in the `MenuCascade` the host drives —
//! that is what `UIF-CMENU-002`'s mutual exclusion is enforced against.
//! [`SplitButtonModel::menu_open`] mirrors it so a host rendering from
//! the model alone can still tell whether the chevron should read as
//! open, and the host is what keeps the mirror accurate. Nothing in this
//! model opens or closes anything by itself.
//!
//! # Menu rows carry the app's own `ActionId`
//!
//! The spec's `Vec<MenuRow<ActionId>>` is [`SplitMenuRow`]: the
//! application's id, a label, and the two flags the menu engine needs.
//! [`SplitButtonModel::menu`] turns them into the real
//! `Menu` the cascade opens, so the
//! flyout is the crate's one menu engine and not a second implementation
//! wearing the split button's clothes. `ActionId` must be `Display`
//! because the menu engine keys rows by string; the id itself is what
//! [`MenuSelected`](crate::widget::split_button_host::SplitButtonEmission::MenuSelected)
//! carries back, never the string form.
//!
//! Unit tests live in the sibling `split_button_model_tests.rs` sidecar.

use std::fmt;

use crate::binding::ActionPending;
use crate::surface::menu::model::{Menu, MenuId, Row};
use crate::widget::button::ButtonVariant;
use crate::widget::button_model::{ButtonDensity, ButtonIcon, ButtonModel};

/// The menu id prefix every split button derives its flyout id from.
///
/// A constant rather than an inline literal because the cascade's
/// already-open refusal compares these ids, so the shape of the derived
/// id is part of the control's identity.
pub const MENU_ID_PREFIX: &str = "split-button";

/// One row of a split button's flyout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitMenuRow<ActionId> {
    /// The application's own identity for this command.
    pub id: ActionId,
    /// The row's visible label.
    pub label: String,
    /// Whether the row can be activated.
    pub enabled: bool,
    /// Whether the row is destructive.
    pub destructive: bool,
    /// A trailing shortcut hint.
    pub shortcut: Option<String>,
}

impl<ActionId: Clone + fmt::Display> SplitMenuRow<ActionId> {
    /// A live, non-destructive command row.
    #[must_use]
    pub fn new(id: ActionId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            enabled: true,
            destructive: false,
            shortcut: None,
        }
    }

    /// Mark the row inert.
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Flag the row destructive.
    #[must_use]
    pub const fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// Attach a trailing shortcut hint.
    #[must_use]
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// The engine-side row this maps to.
    ///
    /// The row's key is the `ActionId`'s `Display` form, which is what
    /// makes the flyout's row identity stable across rebuilds; the
    /// `ActionId` itself is what travels back out on activation.
    #[must_use]
    pub fn to_row(&self) -> Row {
        let row = Row::action(self.id.to_string(), self.label.clone())
            .with_enabled(self.enabled)
            .with_destructive(self.destructive);
        match &self.shortcut {
            Some(shortcut) => row.with_shortcut(shortcut.clone()),
            None => row,
        }
    }
}

/// The state of one split-button control (§3.1c, `UIF-ATOM-001c`).
#[derive(Clone, Debug, PartialEq)]
pub struct SplitButtonModel<ActionId> {
    /// The primary action half — a real button model, not a copy of one.
    primary: ButtonModel,
    /// The flyout's rows.
    pub menu_rows: Vec<SplitMenuRow<ActionId>>,
    /// Whether the flyout is showing. A mirror of the cascade's state.
    pub menu_open: bool,
    /// The flyout's identity, derived from [`MENU_ID_PREFIX`] unless
    /// overridden.
    menu_id: MenuId,
}

impl<ActionId: Clone + fmt::Display> SplitButtonModel<ActionId> {
    /// A Secondary split button with `label` and the given flyout rows.
    #[must_use]
    pub fn new(label: impl Into<String>, menu_rows: Vec<SplitMenuRow<ActionId>>) -> Self {
        let label = label.into();
        let menu_id = MenuId::new(format!("{MENU_ID_PREFIX}:{label}"));
        Self {
            primary: ButtonModel::new(label.clone()).with_variant(ButtonVariant::Secondary),
            menu_rows,
            menu_open: false,
            menu_id,
        }
    }

    /// A Primary split button.
    #[must_use]
    pub fn primary(label: impl Into<String>, menu_rows: Vec<SplitMenuRow<ActionId>>) -> Self {
        let mut model = Self::new(label, menu_rows);
        model.primary.variant = ButtonVariant::Primary;
        model
    }

    /// Override the flyout's identity.
    ///
    /// Needed when two split buttons share a label and would otherwise
    /// claim the same menu id — which the cascade would read as the same
    /// tree being re-opened.
    #[must_use]
    pub fn with_menu_id(mut self, id: impl Into<String>) -> Self {
        self.menu_id = MenuId::new(id);
        self
    }

    /// Set the flyout rows.
    #[must_use]
    pub fn with_menu_rows(mut self, menu_rows: Vec<SplitMenuRow<ActionId>>) -> Self {
        self.menu_rows = menu_rows;
        self
    }

    /// Attach a leading icon to the primary half.
    #[must_use]
    pub fn with_leading_icon(mut self, icon: ButtonIcon) -> Self {
        self.primary.leading_icon = Some(icon);
        self
    }

    /// Set the density of both halves.
    #[must_use]
    pub fn with_density(mut self, density: ButtonDensity) -> Self {
        self.primary.density = density;
        self
    }

    /// Attach a tooltip to the primary half.
    #[must_use]
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.primary.tooltip = Some(tooltip.into());
        self
    }

    /// Mark the whole control busy.
    #[must_use]
    pub fn busy(mut self) -> Self {
        self.primary.busy = true;
        self
    }

    /// Mark the whole control inert.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.primary.disabled = true;
        self
    }

    /// Say that the primary half's action can be cancelled mid-flight.
    #[must_use]
    pub fn cancellable(mut self) -> Self {
        self.primary.cancellable = true;
        self
    }

    /// The primary action half.
    ///
    /// The whole of the composition: the host hands this straight to
    /// [`ButtonController`](crate::widget::button_controller::ButtonController),
    /// so busy suppression and the activation gate are the button atom's
    /// own and not a re-implementation of them.
    #[must_use]
    pub const fn primary_model(&self) -> &ButtonModel {
        &self.primary
    }

    /// The visible label of the primary half.
    #[must_use]
    pub fn label(&self) -> &str {
        self.primary.label.as_deref().unwrap_or_default()
    }

    /// The control's variant.
    #[must_use]
    pub fn variant(&self) -> ButtonVariant {
        self.primary.variant
    }

    /// The control's density.
    #[must_use]
    pub fn density(&self) -> ButtonDensity {
        self.primary.density
    }

    /// Whether the whole control is inert.
    #[must_use]
    pub const fn disabled_flag(&self) -> bool {
        self.primary.disabled
    }

    /// Whether work is underway on the primary half.
    #[must_use]
    pub const fn busy_flag(&self) -> bool {
        self.primary.busy
    }

    /// The `ActionPending` projection of the primary half.
    ///
    /// This is the `ActionPending` adoption `001c` asks for: the split
    /// button does not model "busy" as a boolean it invented, it forwards
    /// the same three-state projection `001a` settled on.
    #[must_use]
    pub fn pending(&self) -> ActionPending {
        self.primary.pending()
    }

    /// Whether the primary half's dispatches are tracked.
    #[must_use]
    pub fn tracks_action(&self) -> bool {
        self.primary.tracks_action()
    }

    /// The flyout's identity.
    #[must_use]
    pub const fn menu_id(&self) -> &MenuId {
        &self.menu_id
    }

    /// Whether the flyout has anything a menu can focus.
    ///
    /// A flyout of only headings or separators is refused by the cascade
    /// ([`IgnoredReason::NoFocusableRows`](crate::surface::menu::cascade::IgnoredReason));
    /// reporting it here lets the control stop drawing an affordance it
    /// cannot honour.
    #[must_use]
    pub fn has_navigable_menu(&self) -> bool {
        self.menu_rows.iter().any(|row| row.enabled)
    }

    /// Build the engine-side flyout.
    #[must_use]
    pub fn menu(&self) -> Menu {
        Menu::new(
            self.menu_id.clone(),
            self.menu_rows.iter().map(SplitMenuRow::to_row).collect(),
        )
    }

    /// The application id behind engine row `index`.
    #[must_use]
    pub fn action_at(&self, index: usize) -> Option<&ActionId> {
        self.menu_rows.get(index).map(|row| &row.id)
    }
}

#[cfg(test)]
#[path = "split_button_model_tests.rs"]
mod split_button_model_tests;
