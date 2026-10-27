//! The single high-emphasis focal point (`UIF-PRIN-006`, §1.1 pillar 6).
//!
//! §1.1: *"At most **one** control per surface container (modal footer,
//! toolbar cluster, card, or form) uses the high-emphasis `Primary`
//! (`color.accent.fill`) or `Danger` (`color.status.danger.fill`) solid
//! fill."*
//!
//! # Why the atom cannot hold this rule
//!
//! [`ButtonVariant`] already names
//! the four-step descent — a solid `accent.fill` CTA, a bordered
//! `surface.raised` secondary, a quiet ghost, a solid
//! `status.danger.fill` destructive — and it names them correctly. But
//! the rule is not about any one variant. It is a count over a set of
//! siblings, and no control can see its own neighbours. A
//! [`DialogButtonController`](crate::surface::dialog_button::DialogButtonController)
//! knows its role and nothing else, which is precisely why a host could
//! build two `Primary` controllers for one footer and pass every test in
//! this crate. The count has to live above the atom, which is what
//! [`check_focal_point`] is.
//!
//! # `Primary` + `Danger` is two, not one-each
//!
//! The clause is one *focal point*, not one focal point per colour. A
//! footer with a `Primary` confirm and a `Danger` delete has two things
//! shouting at the user even though neither colour is repeated, so both
//! variants map to [`Emphasis::High`] and the pair fails the check. That
//! is the reading the words support, and it is pinned by test rather than
//! left to be inferred from a comment.
//!
//! # What the classification can and cannot see
//!
//! [`Emphasis`] classifies *declared intent*, and the two conversions
//! differ in how much they actually know:
//!
//! * [`From<ButtonVariant>`] is sound. The variant *is* the paint
//!   contract — `Primary` means a solid `accent.fill` background, and
//!   [`resolve_style`](crate::widget::button::resolve_style) will paint
//!   exactly that.
//! * [`From<DialogButtonRole>`] reads intent rather than paint.
//!   `DialogButtonRole` says whether a footer control dispatches or
//!   discards; it carries no fill. Mapping `Primary`→`High` is the
//!   framework's reading that the dispatching control is the CTA, and it
//!   is the reading the spec's footer clause assumes. A host that paints
//!   a `Secondary`-role control with a solid accent fill is invisible to
//!   this type, because the role genuinely does not know. Such a host
//!   must classify by what it paints — construct
//!   [`EmphasisSlot::high`](EmphasisSlot::high) directly instead of
//!   converting a role.
//!
//! Unit tests live in the sibling `emphasis_tests.rs` sidecar.

use crate::surface::dialog_button::DialogButtonRole;
use crate::widget::button::ButtonVariant;

/// How loudly a control paints, in the sense §1.1 pillar 6 counts
/// (`UIF-PRIN-006`).
///
/// Three levels rather than four because the rule counts exactly one
/// thing — solid high-emphasis fills — and needs a word for everything
/// that steps down out of it. `Medium` and `Quiet` are not distinguished
/// by the rule; they are distinguished because a report that said only
/// "not high" could not tell a bordered secondary from an invisible
/// ghost, and the difference matters to whoever is fixing the row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Emphasis {
    /// A solid high-emphasis fill: `Primary` on
    /// [`accent_fill`](crate::token::color::Palette::accent_fill), or a
    /// destructive control on
    /// [`status_danger_fill`](crate::token::color::Palette::status_danger_fill).
    ///
    /// Note that `Danger` is classified here by its *peak* fill, not its
    /// rest fill: `danger_paint` rests on `surface_raised` with
    /// `status_danger_text` and only reaches `status_danger_fill` on
    /// hover. §1.1 names `Danger` (`color.status.danger.fill`) as a
    /// high-emphasis control regardless, and the rule is about which
    /// control is *the* loud one, so the variant identity governs.
    High,
    /// A bordered, surface-filled control — `Secondary`
    /// (`surface.raised` + `border.strong`).
    Medium,
    /// A control with no fill at rest — `Ghost` (`transparent`).
    Quiet,
}

impl Emphasis {
    /// Every level, loudest first.
    pub const ALL: [Self; 3] = [Self::High, Self::Medium, Self::Quiet];

    /// Stable machine-readable name for reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Quiet => "quiet",
        }
    }

    /// Whether this level counts toward the one-focal-point budget.
    #[must_use]
    pub const fn is_high(self) -> bool {
        matches!(self, Self::High)
    }
}

/// A container scope the focal-point rule is counted over
/// (`UIF-PRIN-006`).
///
/// The four scopes are §1.1's own list. Carrying them as a type means a
/// report says *"the modal footer has two focal points"* rather than
/// *"this set has two focal points"*, and a host cannot accidentally
/// count a scope the clause never named and call it compliance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContainerScope {
    /// A modal dialog's footer.
    ModalFooter,
    /// A cluster of controls inside a toolbar.
    ToolbarCluster,
    /// A card.
    Card,
    /// A form.
    Form,
}

impl ContainerScope {
    /// Every scope the clause covers.
    pub const ALL: [Self; 4] = [
        Self::ModalFooter,
        Self::ToolbarCluster,
        Self::Card,
        Self::Form,
    ];

    /// Stable machine-readable name for reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::ModalFooter => "modal-footer",
            Self::ToolbarCluster => "toolbar-cluster",
            Self::Card => "card",
            Self::Form => "form",
        }
    }
}

/// One control inside a container scope, as the focal-point rule sees it
/// (`UIF-PRIN-006`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmphasisSlot<'a> {
    /// What to call this control in a violation report.
    pub name: &'a str,
    /// How loudly it paints.
    pub emphasis: Emphasis,
}

impl<'a> EmphasisSlot<'a> {
    /// A slot at any emphasis level.
    #[must_use]
    pub const fn new(name: &'a str, emphasis: Emphasis) -> Self {
        Self { name, emphasis }
    }

    /// A slot declared high-emphasis directly.
    ///
    /// The escape hatch for paint the framework cannot see through — a
    /// host filling a `Secondary`-role control with `accent.fill` has to
    /// say so here, because no conversion can discover it. See the
    /// module docs.
    #[must_use]
    pub const fn high(name: &'a str) -> Self {
        Self {
            name,
            emphasis: Emphasis::High,
        }
    }

    /// The emphasis a [`ButtonVariant`] paints at.
    #[must_use]
    pub fn of_button_variant(name: &'a str, variant: ButtonVariant) -> Self {
        Self {
            name,
            emphasis: Emphasis::from(variant),
        }
    }

    /// The emphasis a [`DialogButtonRole`] implies.
    ///
    /// Intent, not paint — see the module docs for the limit of what
    /// this conversion can know.
    #[must_use]
    pub fn of_dialog_role(name: &'a str, role: DialogButtonRole) -> Self {
        Self {
            name,
            emphasis: Emphasis::from(role),
        }
    }
}

impl From<ButtonVariant> for Emphasis {
    /// Map a button variant onto the emphasis the rule counts.
    ///
    /// `Primary` and `Danger` are both `High`: the clause is one focal
    /// point, not one per colour, so a confirm-and-delete pair is two.
    fn from(variant: ButtonVariant) -> Self {
        match variant {
            ButtonVariant::Primary | ButtonVariant::Danger => Self::High,
            ButtonVariant::Secondary => Self::Medium,
            ButtonVariant::Ghost => Self::Quiet,
        }
    }
}

impl From<DialogButtonRole> for Emphasis {
    /// Map a dialog button role onto the emphasis the rule counts.
    ///
    /// The dispatching role is read as the footer's CTA and therefore
    /// `High`; the discarding role steps down to `Medium`. This is a
    /// reading of intent: `DialogButtonRole` carries no fill, so a host
    /// that paints a `Secondary`-role control loudly is invisible to
    /// this conversion and must classify through
    /// [`EmphasisSlot::high`](EmphasisSlot::high).
    fn from(role: DialogButtonRole) -> Self {
        match role {
            DialogButtonRole::Primary => Self::High,
            DialogButtonRole::Secondary => Self::Medium,
        }
    }
}

/// The verdict of one focal-point count (`UIF-PRIN-006`).
#[derive(Clone, Debug, PartialEq)]
pub struct FocalPointReport<'a> {
    /// The scope that was counted.
    pub scope: ContainerScope,
    /// Every high-emphasis control found, in the order supplied.
    ///
    /// Carries the full slot rather than just the name so the report can
    /// show *which* high variants collided — `Primary` + `Danger` is a
    /// different fix from `Primary` + `Primary`.
    pub high: Vec<EmphasisSlot<'a>>,
    /// How many controls were counted in total.
    pub total: usize,
}

impl<'a> FocalPointReport<'a> {
    /// Whether the container has at most one focal point.
    #[must_use]
    pub fn is_single_focal_point(&self) -> bool {
        self.high.len() <= 1
    }

    /// How many high-emphasis controls are over budget.
    ///
    /// Zero when compliant; the number of controls that must step down
    /// otherwise.
    #[must_use]
    pub fn excess(&self) -> usize {
        self.high.len().saturating_sub(1)
    }

    /// The names of the high-emphasis controls, in the order supplied.
    ///
    /// Lets a caller log the collision without walking
    /// [`Self::high`].
    #[must_use]
    pub fn high_names(&self) -> Vec<&'a str> {
        self.high.iter().map(|slot| slot.name).collect()
    }
}

impl std::fmt::Display for FocalPointReport<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_single_focal_point() {
            return write!(
                f,
                "{} has a single focal point ({} control(s) counted)",
                self.scope.slug(),
                self.total
            );
        }
        write!(
            f,
            "{} has {} high-emphasis controls where at most 1 is allowed: {}",
            self.scope.slug(),
            self.high.len(),
            self.high
                .iter()
                .map(|slot| slot.name)
                .collect::<Vec<_>>()
                .join(", "),
        )
    }
}

/// Count the high-emphasis controls in one container scope
/// (`UIF-PRIN-006`).
///
/// A pure count over the slots supplied: the caller decides what is in
/// the container, this decides whether that is allowed. An empty scope
/// passes — nothing is shouting.
#[must_use]
pub fn check_focal_point<'a>(
    scope: ContainerScope,
    slots: &[EmphasisSlot<'a>],
) -> FocalPointReport<'a> {
    let high = slots
        .iter()
        .filter(|slot| slot.emphasis.is_high())
        .copied()
        .collect();
    FocalPointReport {
        scope,
        high,
        total: slots.len(),
    }
}

#[cfg(test)]
#[path = "emphasis_tests.rs"]
mod emphasis_tests;
