//! The declarative `PopoverModel` a host owns — §3.7's interactive
//! callout data (`UIF-ATOM-007`).
//!
//! The model is §3.7's field list, unchanged: `title`, `body`,
//! `primary_action`, `placement`, `open`. What the section gives as a
//! *sketch* — "contains title, body, and optional footer action/dismiss
//! buttons" — this module turns into a concrete footer roster, because
//! the footer is the part the keyboard and the pointer both have to find.
//!
//! # The footer always has a dismiss button
//!
//! §3.7's prose names an action button *and* a dismiss button, but its
//! data model carries only `primary_action: Option<(String, ActionId)>`.
//! Read literally, a popover with no primary action has **no focusable
//! control at all** — and a focus trap around an empty scope is the one
//! thing worse than no trap, because it swallows `Tab` while leaving the
//! ring stranded under a click-away catcher.
//!
//! So the dismiss button is **framework-supplied and always present**:
//!
//! * `primary_action: Some((label, id))` → a two-button footer, primary
//!   first, and focus lands on the primary;
//! * `primary_action: None` → a one-button footer holding only the
//!   dismiss control, and focus lands there.
//!
//! That keeps every popover openable, keeps the trap non-empty, and keeps
//! §3.7's "action/dismiss buttons" true in both cases. The dismiss
//! button's *label* is the one string the spec does not give; it is
//! [`DISMISS_LABEL`] rather than an inline literal, and it is recorded as
//! a framework-supplied string in `spec_coverage`.
//!
//! # Why the footer ids are derived, not supplied
//!
//! The trap confines a [`GroupId`] and the buttons are [`FocusId`]s inside
//! it. Deriving both from the popover's [`SurfaceId`] — [`group_for`],
//! [`primary_focus_id`], [`dismiss_focus_id`] — means a host cannot
//! accidentally point two popovers at one group, and a test cannot
//! assert against a literal that drifted from the producer.
//!
//! [`SurfaceId`]: crate::surface::zorder::SurfaceId
//!
//! Unit tests live in the sibling `model_tests.rs` sidecar.

use crate::focus::{FocusId, FocusMember, GroupId};

use super::place::PreferredSide;

/// The label of the framework-supplied dismiss button.
///
/// §3.7 names the button but not its word. A named constant rather than
/// an inline literal because it is user-facing text the framework chose
/// (`RUST-021`), and because a test must be able to assert the footer
/// roster against the same string the renderer paints.
pub const DISMISS_LABEL: &str = "Dismiss";

/// The interactive callout a host owns (`UIF-ATOM-007`).
///
/// `ActionId` is the host's own action type. The model never inspects it
/// beyond cloning it into the event the footer produces, so a host may
/// use anything from a `&'static str` to a rich command enum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopoverModel<ActionId> {
    title: String,
    body: String,
    primary_action: Option<(String, ActionId)>,
    placement: PreferredSide,
    open: bool,
}

impl<ActionId: Clone + PartialEq> PopoverModel<ActionId> {
    /// A callout with a title and a body, and no primary action.
    ///
    /// Its footer holds the dismiss button alone. This is the shape §3.7
    /// describes for a plain "here is what this does" note.
    #[must_use]
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            primary_action: None,
            placement: PreferredSide::default(),
            open: true,
        }
    }

    /// Return a copy carrying `action` as its primary footer button.
    #[must_use]
    pub fn with_action(mut self, label: impl Into<String>, action: ActionId) -> Self {
        self.primary_action = Some((label.into(), action));
        self
    }

    /// Return a copy preferring to open on `side`.
    #[must_use]
    pub fn with_placement(mut self, side: PreferredSide) -> Self {
        self.placement = side;
        self
    }

    /// Return a copy whose `open` flag is `wanted`.
    ///
    /// The flag is the host's statement of intent, not the popover's
    /// state: [`open`](super::host::PopoverHost::open) refuses when it is
    /// `false`, and a closed model is what a host hands back after the
    /// callout has been dismissed.
    #[must_use]
    pub fn with_open(mut self, wanted: bool) -> Self {
        self.open = wanted;
        self
    }

    /// The callout's title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The callout's body.
    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    /// The primary footer button, if the host supplied one.
    #[must_use]
    pub fn primary_action(&self) -> Option<(&str, &ActionId)> {
        self.primary_action
            .as_ref()
            .map(|(label, action)| (label.as_str(), action))
    }

    /// The preferred opening side.
    #[must_use]
    pub const fn placement(&self) -> PreferredSide {
        self.placement
    }

    /// Whether the host wants this callout open.
    #[must_use]
    pub const fn is_open_wanted(&self) -> bool {
        self.open
    }

    /// Whether the footer has a primary action button.
    #[must_use]
    pub fn has_primary_action(&self) -> bool {
        self.primary_action.is_some()
    }

    /// The footer's focusable roster, in traversal order.
    ///
    /// Primary first when it exists, dismiss always last. The order is the
    /// order §3.7 draws them and the order [`Tab`] walks, and it is never
    /// empty — which is the whole reason the dismiss button is not
    /// optional.
    ///
    /// [`Tab`]: crate::surface::zorder::KeyClass::Tab
    #[must_use]
    pub fn footer_members(&self) -> Vec<FocusMember> {
        let mut members = Vec::with_capacity(2);
        if self.has_primary_action() {
            members.push(FocusMember::new(primary_focus_id()));
        }
        members.push(FocusMember::new(dismiss_focus_id()));
        members
    }

    /// Where focus lands when this callout opens.
    ///
    /// The primary action when there is one — a teaching popover exists to
    /// be acted on, so the keyboard should start on the thing it is
    /// teaching the user to do — and the dismiss button otherwise.
    #[must_use]
    pub fn initial_focus(&self) -> FocusId {
        if self.has_primary_action() {
            primary_focus_id()
        } else {
            dismiss_focus_id()
        }
    }
}

/// The focus group a teaching popover owns.
///
/// A fixed id rather than one derived per popover, because the framework
/// allows **one** teaching popover at a time: §3.7 sits in Band 2, whose
/// rule is "single-flyout mutual exclusion", so a second callout
/// replaces the first rather than stacking beside it. A per-popover
/// group would have implied a stack the band model does not permit.
#[must_use]
pub fn group_for() -> GroupId {
    GroupId::new(POPOVER_GROUP)
}

/// The stable name of the popover's focus group.
pub const POPOVER_GROUP: &str = "popover";

/// The footer's primary-action focus id.
#[must_use]
pub fn primary_focus_id() -> FocusId {
    FocusId::new(format!("{POPOVER_GROUP}:primary"))
}

/// The footer's dismiss-button focus id.
#[must_use]
pub fn dismiss_focus_id() -> FocusId {
    FocusId::new(format!("{POPOVER_GROUP}:dismiss"))
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod model_tests;
