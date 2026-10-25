//! The dialog / form footer as an ordered, focusable composition
//! (`UIF-PRIN-006`, §1.1 pillar 6).
//!
//! §1.1 fixes the footer's shape in one sentence: *"Dialog and form
//! footers right-align their button group with a `space.sm` (`8px`) gap
//! in the order **`[Secondary / Cancel] [Primary / Confirm]`**, with
//! utility or help actions pinned far-left (`<── Fill ──>`). In
//! destructive confirmations, initial keyboard focus lands on
//! `[Cancel]`."*
//!
//! # Order as a type rather than a convention
//!
//! [`FooterSlot`] has exactly three members and an ordinal, so "Cancel
//! comes before Confirm" is a fact about the enum that
//! [`FooterSlot::position`] can be asked, not a comment a host may or
//! may not have read. A host that appends its buttons in the wrong order
//! gets [`FooterViolation::OutOfOrder`] naming both controls, which is
//! the difference between a convention and a rule.
//!
//! # Focus is data
//!
//! The destructive-confirmation rule is the one clause here that is easy
//! to write as prose and impossible to enforce from prose. It is modelled
//! as [`FooterPlan::initial_focus`]: a name the plan carries, which
//! [`FooterPlan::validate`] requires to be the `Cancel` action whenever
//! [`FooterPlan::kind`] is [`FooterKind::Destructive`]. A host that
//! forgets to focus Cancel, focuses the confirm, or names a control that
//! is not in the footer at all, fails the check rather than shipping a
//! dialog that presses Delete on an accidental `Enter`.
//!
//! # What is *not* claimed
//!
//! This module models and checks the composition. It does not drive a
//! keyboard: nothing here can move focus, because focus lives in the
//! [`bridge`](crate::bridge) and the widget tree, not in a plan. What the
//! plan guarantees is that the value a host must honour is present and
//! assertable, so "we forgot the Cancel-focus rule" becomes a failing
//! test rather than a bug report.
//!
//! Unit tests live in the sibling `footer_tests.rs` sidecar.

use crate::layout::density::{
    ControlDensity, DensityBand, DensityReport, RowSlot, check_row_density,
};
use crate::layout::emphasis::{ContainerScope, Emphasis, EmphasisSlot, check_focal_point};
use crate::token::space;

/// The gap the right-aligned footer group is laid out at
/// (`UIF-PRIN-006`).
///
/// Read from [`space::SM`] rather than restated as `8.0`, so the footer
/// cannot drift off the spacing scale it names.
pub const GROUP_GAP: f32 = space::SM;

/// Where a footer slot sits horizontally (`UIF-PRIN-006`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FooterAlignment {
    /// Pinned to the far left, away from the decision group.
    FarLeft,
    /// Part of the right-aligned decision group.
    Right,
}

/// One of the three ordered footer slots (`UIF-PRIN-006`).
///
/// The three are the spec's own, and the enum's discriminant order *is*
/// the layout order: [`FooterSlot::Utility`] far-left, then
/// [`FooterSlot::Cancel`], then [`FooterSlot::Confirm`] at the right
/// edge. A fourth kind of footer control is not expressible here, which
/// is the point — a host that thinks it needs one should be asking why
/// the shape does not fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FooterSlot {
    /// Utility or help actions, pinned far-left.
    Utility,
    /// `[Secondary / Cancel]` — the discard control.
    Cancel,
    /// `[Primary / Confirm]` — the right-edge commit control.
    Confirm,
}

impl FooterSlot {
    /// The three slots in layout order, left to right.
    pub const ORDER: [Self; 3] = [Self::Utility, Self::Cancel, Self::Confirm];

    /// Every slot.
    pub const ALL: [Self; 3] = Self::ORDER;

    /// The ordinal of this slot in the layout order — `0` leftmost.
    #[must_use]
    pub const fn position(self) -> u8 {
        match self {
            Self::Utility => 0,
            Self::Cancel => 1,
            Self::Confirm => 2,
        }
    }

    /// Whether this slot is pinned away from the decision group.
    #[must_use]
    pub const fn is_left_pinned(self) -> bool {
        matches!(self, Self::Utility)
    }

    /// The horizontal alignment this slot belongs to.
    #[must_use]
    pub const fn alignment(self) -> FooterAlignment {
        match self {
            Self::Utility => FooterAlignment::FarLeft,
            Self::Cancel | Self::Confirm => FooterAlignment::Right,
        }
    }

    /// Stable machine-readable name for reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Utility => "utility",
            Self::Cancel => "cancel",
            Self::Confirm => "confirm",
        }
    }
}

/// What a footer is confirming (`UIF-PRIN-006`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FooterKind {
    /// An ordinary accept / discard footer with no focus mandate.
    Plain,
    /// A destructive confirmation, for which §1.1 mandates that initial
    /// keyboard focus lands on `[Cancel]`.
    Destructive,
}

impl FooterKind {
    /// Every kind.
    pub const ALL: [Self; 2] = [Self::Plain, Self::Destructive];

    /// Whether the cancel-focus mandate applies.
    #[must_use]
    pub const fn is_destructive(self) -> bool {
        matches!(self, Self::Destructive)
    }

    /// Stable machine-readable name for reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Destructive => "destructive",
        }
    }
}

/// One control in a footer composition (`UIF-PRIN-006`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FooterAction<'a> {
    /// What to call this control in a report.
    pub name: &'a str,
    /// Which ordered slot it occupies.
    pub slot: FooterSlot,
    /// How loudly it paints, which the slot constrains.
    pub emphasis: Emphasis,
    /// The height this control is actually built at, or `None` to build
    /// it at the plan's declared density.
    ///
    /// `Option` rather than a plain height so the common case cannot be
    /// wrong: an action that does not say otherwise *is* the band's
    /// density. The override exists because that is exactly where a
    /// mixture comes from — a host reusing a control built for a
    /// different rung — and a density check that could only read
    /// heights the plan itself supplied would have nothing to find.
    pub height: Option<f32>,
}

impl<'a> FooterAction<'a> {
    /// An action in a given slot at a given emphasis, built at the
    /// plan's density.
    #[must_use]
    pub const fn new(name: &'a str, slot: FooterSlot, emphasis: Emphasis) -> Self {
        Self {
            name,
            slot,
            emphasis,
            height: None,
        }
    }

    /// An action built at an explicit height rather than the band's rung.
    ///
    /// The shape that triggers a density violation, and therefore the
    /// shape a host reaches for when it is reusing a control from
    /// somewhere else.
    #[must_use]
    pub const fn at_height(
        name: &'a str,
        slot: FooterSlot,
        emphasis: Emphasis,
        height: f32,
    ) -> Self {
        Self {
            name,
            slot,
            emphasis,
            height: Some(height),
        }
    }
}

/// One thing wrong with a footer composition (`UIF-PRIN-006`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FooterViolation<'a> {
    /// A `[Cancel]` slot control wears a solid high-emphasis fill, where
    /// §1.1 requires `Secondary`.
    CancelIsHighEmphasis {
        /// The offending control.
        control: &'a str,
    },
    /// A `[Confirm]` slot control is not high-emphasis, where §1.1
    /// requires `Primary`.
    ConfirmIsNotHighEmphasis {
        /// The offending control.
        control: &'a str,
        /// What it actually is.
        actual: &'static str,
    },
    /// Two or more footer controls are high-emphasis — the focal-point
    /// clause counted over the footer scope.
    MultipleHighEmphasis {
        /// The colliding controls, in footer order.
        controls: Vec<&'a str>,
    },
    /// Two controls are laid out against the fixed slot order.
    OutOfOrder {
        /// The control placed too far right.
        earlier: &'a str,
        /// The control it was placed before.
        later: &'a str,
    },
    /// A destructive footer has no declared initial focus, so the
    /// `[Cancel]` mandate cannot be satisfied.
    DestructiveWithoutInitialFocus,
    /// A destructive footer has no `Cancel` action to focus.
    DestructiveWithoutCancel,
    /// The declared initial focus names a control that is not in the
    /// footer.
    InitialFocusUnknown {
        /// The name that was declared.
        focused: &'a str,
    },
    /// A destructive footer declares initial focus on something other
    /// than the `Cancel` action.
    InitialFocusNotCancel {
        /// What the plan focuses.
        focused: &'a str,
        /// What §1.1 requires.
        expected: &'a str,
    },
}

impl std::fmt::Display for FooterViolation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CancelIsHighEmphasis { control } => write!(
                f,
                "footer cancel '{control}' is high-emphasis; the cancel slot must step down to Secondary or Ghost"
            ),
            Self::ConfirmIsNotHighEmphasis { control, actual } => write!(
                f,
                "footer confirm '{control}' is {actual}; the confirm slot must be the high-emphasis Primary"
            ),
            Self::MultipleHighEmphasis { controls } => write!(
                f,
                "footer has {} high-emphasis controls where at most 1 is allowed: {}",
                controls.len(),
                controls.join(", ")
            ),
            Self::OutOfOrder { earlier, later } => write!(
                f,
                "footer order violated: '{earlier}' sits before '{later}', against the Utility → Cancel → Confirm order"
            ),
            Self::DestructiveWithoutInitialFocus => {
                write!(
                    f,
                    "destructive footer declares no initial focus; §1.1 requires it on Cancel"
                )
            }
            Self::DestructiveWithoutCancel => write!(
                f,
                "destructive footer has no Cancel action, so the cancel-focus mandate cannot be met"
            ),
            Self::InitialFocusUnknown { focused } => write!(
                f,
                "footer initial focus '{focused}' names no control in the footer"
            ),
            Self::InitialFocusNotCancel { focused, expected } => write!(
                f,
                "destructive footer focuses '{focused}'; §1.1 requires initial focus on '{expected}'"
            ),
        }
    }
}

/// A footer composition a host intends to build (`UIF-PRIN-006`).
///
/// Every clause §1.1 states about a footer is a field on this struct, so
/// [`FooterPlan::validate`] can assert all of them at once against the
/// same data the host is about to render.
#[derive(Clone, Debug, PartialEq)]
pub struct FooterPlan<'a> {
    /// Whether this is a destructive confirmation.
    pub kind: FooterKind,
    /// The density every control in the footer is built at.
    pub density: ControlDensity,
    /// The container the footer sits in, which the focal-point clause
    /// counts over.
    pub scope: ContainerScope,
    /// The footer's controls, in the order they are laid out.
    pub actions: Vec<FooterAction<'a>>,
    /// The control that receives initial keyboard focus.
    ///
    /// This is the destructive-confirmation rule carried as data rather
    /// than as a comment: for a [`FooterKind::Destructive`] plan it must
    /// name the `Cancel` action.
    pub initial_focus: Option<&'a str>,
}

impl<'a> FooterPlan<'a> {
    /// An empty footer plan at a given kind and density.
    ///
    /// Scoped to a [`ContainerScope::ModalFooter`]; a host building a
    /// form footer sets [`Self::scope`] to match.
    #[must_use]
    pub const fn new(kind: FooterKind, density: ControlDensity) -> Self {
        Self {
            kind,
            density,
            scope: ContainerScope::ModalFooter,
            actions: Vec::new(),
            initial_focus: None,
        }
    }

    /// The gap the right-aligned group is laid out at — `space.sm`.
    #[must_use]
    pub const fn group_gap() -> f32 {
        GROUP_GAP
    }

    /// The action carrying the initial focus, if any.
    #[must_use]
    pub fn focused_action(&self) -> Option<&FooterAction<'a>> {
        let wanted = self.initial_focus?;
        self.actions.iter().find(|action| action.name == wanted)
    }

    /// The `Cancel`-slot action, if the footer has one.
    #[must_use]
    pub fn cancel_action(&self) -> Option<&FooterAction<'a>> {
        self.actions
            .iter()
            .find(|action| action.slot == FooterSlot::Cancel)
    }

    /// The `Confirm`-slot action, if the footer has one.
    #[must_use]
    pub fn confirm_action(&self) -> Option<&FooterAction<'a>> {
        self.actions
            .iter()
            .find(|action| action.slot == FooterSlot::Confirm)
    }

    /// The height a control in this footer is actually built at.
    ///
    /// The action's own override if it declared one, otherwise the
    /// plan's rung. A single source so the density adapter and anything
    /// else that measures a footer control cannot disagree.
    #[must_use]
    pub fn effective_height(&self, action: &FooterAction<'_>) -> f32 {
        action.height.unwrap_or(self.density.height())
    }

    /// Check the footer's own density clause (§1.1 row homogeneity).
    ///
    /// Measures every action at [`Self::effective_height`] against the
    /// plan's declared rung, so a host that drops a control built for
    /// another density into the footer is reported rather than assumed
    /// away.
    #[must_use]
    pub fn density_report(&self) -> DensityReport<'a> {
        let slots: Vec<RowSlot<'a>> = self
            .actions
            .iter()
            .map(|action| RowSlot::new(action.name, self.effective_height(action)))
            .collect();
        check_row_density(DensityBand::DialogFooter, self.density, &slots)
    }

    /// Check every clause §1.1 states about a footer, in one pass.
    ///
    /// Returns an empty `Vec` only when the plan satisfies the slot
    /// emphasis rules, the slot order, the single-focal-point count and
    /// the destructive-focus mandate together.
    #[must_use]
    pub fn validate(&self) -> Vec<FooterViolation<'a>> {
        let mut violations = Vec::new();

        for action in &self.actions {
            match action.slot {
                FooterSlot::Cancel if action.emphasis.is_high() => {
                    violations.push(FooterViolation::CancelIsHighEmphasis {
                        control: action.name,
                    });
                }
                FooterSlot::Confirm if !action.emphasis.is_high() => {
                    violations.push(FooterViolation::ConfirmIsNotHighEmphasis {
                        control: action.name,
                        actual: action.emphasis.slug(),
                    });
                }
                _ => {}
            }
        }

        for (index, earlier) in self.actions.iter().enumerate() {
            for later in &self.actions[index + 1..] {
                if earlier.slot.position() > later.slot.position() {
                    violations.push(FooterViolation::OutOfOrder {
                        earlier: earlier.name,
                        later: later.name,
                    });
                }
            }
        }

        let emphasis_slots: Vec<EmphasisSlot<'a>> = self
            .actions
            .iter()
            .map(|action| EmphasisSlot::new(action.name, action.emphasis))
            .collect();
        let focal = check_focal_point(self.scope, &emphasis_slots);
        if !focal.is_single_focal_point() {
            violations.push(FooterViolation::MultipleHighEmphasis {
                controls: focal.high.iter().map(|slot| slot.name).collect(),
            });
        }

        if self.kind.is_destructive() {
            match self.initial_focus {
                None => violations.push(FooterViolation::DestructiveWithoutInitialFocus),
                Some(focused) => {
                    let Some(target) = self.focused_action() else {
                        violations.push(FooterViolation::InitialFocusUnknown { focused });
                        return violations;
                    };
                    let Some(cancel) = self.cancel_action() else {
                        violations.push(FooterViolation::DestructiveWithoutCancel);
                        return violations;
                    };
                    if target.slot != FooterSlot::Cancel {
                        violations.push(FooterViolation::InitialFocusNotCancel {
                            focused,
                            expected: cancel.name,
                        });
                    }
                }
            }
        } else if let Some(focused) = self.initial_focus
            && self.focused_action().is_none()
        {
            violations.push(FooterViolation::InitialFocusUnknown { focused });
        }

        violations
    }

    /// Whether the plan satisfies every footer clause.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }
}

#[cfg(test)]
#[path = "footer_tests.rs"]
mod footer_tests;
