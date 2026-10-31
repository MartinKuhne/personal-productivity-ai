//! Style guide, microcopy and ergonomic invariant gate (§6.5, `UIF-VER-005`).
//!
//! Automated invariant gate uniting the seven machine-checkable design and microcopy
//! invariants mandated by the FastMD UI framework specification (§6.5):
//!
//! 1. **Sentence-case chrome labels** — chrome labels use Sentence case, capitalizing
//!    only the first word and licensed acronyms/proper nouns (`UIF-PRIN-007`), skipping
//!    the closed Host `SPEC.md` Literal Pin Table.
//! 2. **Action ellipsis biconditional** — ASCII `...` (U+002E × 3) suffix appears if and
//!    only if activating the control opens a secondary dialog or prompt requiring further
//!    input (`UIF-PRIN-007`), while Unicode `…` (U+2026) is reserved exclusively for
//!    truncation, placeholders and breadcrumb elision triggers.
//! 3. **Single high-emphasis focal point** — maximum one high-emphasis control (`Primary`
//!    solid `accent.fill` or `Danger` solid `status.danger.fill`) per surface container
//!    scope (`ModalFooter`, `ToolbarCluster`, `Card`, `Form`) (`UIF-PRIN-006`).
//! 4. **Homogeneous row density** — controls in a single horizontal row must share the
//!    same `ControlDensity` (`Compact = 28px` or `Standard = 32px`), strictly forbidding
//!    mixing rungs within a single band (`UIF-PRIN-006`).
//! 5. **Automatic tooltip obligation** — labels truncated to fit (`…`) and icon-only
//!    buttons incur an automatic, non-optional obligation to supply an accessible
//!    tooltip exposing the full untruncated string or action name (`UIF-PRIN-007`,
//!    `UIF-ATOM-001`, `UIF-NAV-003`, `UIF-TAB-003`).
//! 6. **Secret redaction** — `SecretInput` redaction in `Debug` and `Display` formatting
//!    ensures plaintext password material cannot leak into logs, format strings or panic
//!    payloads (`NFR-009`).
//! 7. **Reward Early, Punish Late validation transitions** — pure-state validation
//!    transitions withholding error visuals on pristine invalid inputs while punishing
//!    disallowed characters immediately and clearing error visuals the exact frame a
//!    correction is made (`UIF-PRIN-009`).
//!
//! Unit tests live in the sibling `invariant_gate_tests.rs` sidecar.

use std::fmt;

use crate::layout::density::{
    ControlDensity, DensityBand, RowSlot, check_row_density as run_density_check,
};
use crate::layout::emphasis::{ContainerScope, EmphasisSlot, check_focal_point};
use crate::microcopy::action_label::{
    ActionLabel, PromptBehaviour, check_action_ellipsis as run_action_ellipsis_check,
};
use crate::microcopy::case_rule::check_label;
use crate::microcopy::pins::{PinExemption, is_exempt};
use crate::microcopy::truncate::{ELLIPSIS, LabelSlot, TruncationTooltipViolation};
use crate::validation::punish::{Punishment, punish};
use crate::validation::stance::EditStance;
use crate::widget::password_model::SECRET_MASK;

/// One of the seven invariant categories verified by the gate (`UIF-VER-005`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InvariantCategory {
    /// Sentence-case chrome labels (skipping the closed Host SPEC.md Literal Pin Table).
    SentenceCase,
    /// ASCII `...` dialog-launcher vs Unicode `…` truncation/placeholder consistency.
    ActionEllipsis,
    /// Maximum one `Primary`/`Danger` CTA per container scope.
    EmphasisFocalPoint,
    /// Homogeneous `ControlDensity` (28px vs 32px) per horizontal row.
    RowDensity,
    /// Automatic `Tooltip` obligation for truncated (`…`) labels and icon-only buttons.
    TooltipObligation,
    /// `SecretInput` redaction in `Debug` and `Display` (`NFR-009`).
    SecretRedaction,
    /// "Reward Early, Punish Late" validation transitions (`UIF-PRIN-009`).
    ValidationPunishment,
}

impl InvariantCategory {
    /// All seven categories in specification order (`UIF-VER-005`).
    pub const ALL: [Self; 7] = [
        Self::SentenceCase,
        Self::ActionEllipsis,
        Self::EmphasisFocalPoint,
        Self::RowDensity,
        Self::TooltipObligation,
        Self::SecretRedaction,
        Self::ValidationPunishment,
    ];

    /// Stable machine-readable slug for logs and audit manifests (`UIF-VER-005`).
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::SentenceCase => "sentence-case",
            Self::ActionEllipsis => "action-ellipsis",
            Self::EmphasisFocalPoint => "emphasis-focal-point",
            Self::RowDensity => "row-density",
            Self::TooltipObligation => "tooltip-obligation",
            Self::SecretRedaction => "secret-redaction",
            Self::ValidationPunishment => "validation-punishment",
        }
    }
}

impl fmt::Display for InvariantCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

/// A violation identified by the invariant gate (`UIF-VER-005`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvariantViolation {
    /// The invariant category that was violated.
    pub category: InvariantCategory,
    /// The target identifier (e.g. control name, label text, or scope slug).
    pub target: String,
    /// A human-readable description of why the invariant was violated.
    pub message: String,
}

impl InvariantViolation {
    /// Construct a new invariant violation (`UIF-VER-005`).
    #[must_use]
    pub fn new(
        category: InvariantCategory,
        target: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category,
            target: target.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for InvariantViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {}: {}",
            self.category.slug(),
            self.target,
            self.message
        )
    }
}

/// The report produced by an invariant audit run (`UIF-VER-005`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InvariantReport {
    /// Total number of individual checks executed across all categories.
    pub total_checked: usize,
    /// List of violations encountered.
    pub violations: Vec<InvariantViolation>,
}

impl InvariantReport {
    /// Create a new, empty invariant report (`UIF-VER-005`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a single check execution, incrementing the total check count (`UIF-VER-005`).
    pub fn record_check(&mut self) {
        self.total_checked += 1;
    }

    /// Record the result of a check: increments `total_checked` and pushes any violation (`UIF-VER-005`).
    pub fn record_result(&mut self, result: Result<(), InvariantViolation>) {
        self.record_check();
        if let Err(violation) = result {
            self.violations.push(violation);
        }
    }

    /// Returns `true` if zero violations were recorded (`UIF-VER-005`).
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    /// Number of violations currently in the report (`UIF-VER-005`).
    #[must_use]
    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }

    /// Describe the report outcome as a formatted summary string (`UIF-VER-005`).
    #[must_use]
    pub fn describe(&self) -> String {
        if self.is_clean() {
            format!(
                "Invariant Gate: PASS ({} check(s) across 7 categories, 0 violations)",
                self.total_checked
            )
        } else {
            let mut out = format!(
                "Invariant Gate: FAIL ({} violation(s) across {} check(s)):\n",
                self.violation_count(),
                self.total_checked
            );
            for v in &self.violations {
                out.push_str(&format!("  {v}\n"));
            }
            out
        }
    }
}

impl fmt::Display for InvariantReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.describe())
    }
}

/// Check that a chrome label conforms to Sentence case (`UIF-VER-005`, `UIF-PRIN-007`).
///
/// If `is_chrome` is `false`, the check immediately succeeds because sentence-case
/// rules apply exclusively to chrome labels.
///
/// Pinned literals from the closed Host `SPEC.md` Literal Pin Table that carry
/// the `SentenceCase` exemption (`PinExemption::SentenceCase`) are recognized and
/// exempted.
pub fn check_sentence_case(label: &str, is_chrome: bool) -> Result<(), InvariantViolation> {
    if !is_chrome {
        return Ok(());
    }

    if is_exempt(label, PinExemption::SentenceCase) {
        return Ok(());
    }

    let report = check_label(label);
    if report.is_compliant() {
        Ok(())
    } else {
        Err(InvariantViolation::new(
            InvariantCategory::SentenceCase,
            label,
            format!("{report}"),
        ))
    }
}

/// Check that an action label conforms to the ellipsis biconditional (`UIF-VER-005`, `UIF-PRIN-007`).
///
/// Ensures:
/// - Actions that open a secondary prompt carry the unspaced ASCII `...` suffix.
/// - Immediate actions do not carry `...`.
/// - The Unicode truncation mark `…` (U+2026) is never used as an action suffix.
/// - Pinned literals carrying `PinExemption::Ellipsis` are exempt.
pub fn check_action_ellipsis(
    label: &str,
    behaviour: PromptBehaviour,
) -> Result<(), InvariantViolation> {
    let action = ActionLabel { label, behaviour };
    let verdict = run_action_ellipsis_check(&action);
    if verdict.is_compliant() {
        Ok(())
    } else {
        let msg = verdict
            .violation
            .map(|v| v.to_string())
            .unwrap_or_else(|| format!("action label '{label}' breaks ellipsis rules"));
        Err(InvariantViolation::new(
            InvariantCategory::ActionEllipsis,
            label,
            msg,
        ))
    }
}

/// Check that a navigation bar menu trigger conforms to the ellipsis biconditional (`UIF-NAV-002`, `UIF-PRIN-007`, `UIF-VER-005`).
///
/// Delegates to [`NavBarMenuTrigger::validate_ellipsis`](crate::composite::navbar::NavBarMenuTrigger::validate_ellipsis).
pub fn check_navbar_trigger_ellipsis(
    trigger: &crate::composite::navbar::NavBarMenuTrigger,
) -> Result<(), InvariantViolation> {
    trigger.validate_ellipsis()
}

/// Check that a container scope contains at most one high-emphasis control (`UIF-VER-005`, `UIF-PRIN-006`).
///
/// Solid `Primary` (`accent.fill`) and solid `Danger` (`status.danger.fill`) both count
/// toward the single focal point budget. A scope containing both or multiples of either
/// produces an `InvariantViolation`.
pub fn check_emphasis_focal_point<'a>(
    scope: ContainerScope,
    slots: &[EmphasisSlot<'a>],
) -> Result<(), InvariantViolation> {
    let report = check_focal_point(scope, slots);
    if report.is_single_focal_point() {
        Ok(())
    } else {
        Err(InvariantViolation::new(
            InvariantCategory::EmphasisFocalPoint,
            scope.slug(),
            format!("{report}"),
        ))
    }
}

/// Check that a horizontal row of controls maintains homogeneous density (`UIF-VER-005`, `UIF-PRIN-006`).
///
/// If `declared` is `Some(rung)`, all controls must sit at that exact rung (28px or 32px).
/// If `declared` is `None`, the row must be internally homogeneous at the first control's rung,
/// and no control may sit off-rung (e.g. neither 28px nor 32px).
pub fn check_row_density<'a>(
    declared: Option<ControlDensity>,
    slots: &[RowSlot<'a>],
) -> Result<(), InvariantViolation> {
    if slots.is_empty() {
        return Ok(());
    }

    let target = match declared {
        Some(d) => d,
        None => {
            let first = &slots[0];
            match first.rung() {
                Some(rung) => rung,
                None => {
                    return Err(InvariantViolation::new(
                        InvariantCategory::RowDensity,
                        first.name,
                        format!(
                            "control '{}' height {}px is on no density rung",
                            first.name, first.height
                        ),
                    ));
                }
            }
        }
    };

    let report = run_density_check(DensityBand::FormRow, target, slots);
    if report.is_homogeneous() {
        Ok(())
    } else {
        let first_viol = &report.violations[0];
        Err(InvariantViolation::new(
            InvariantCategory::RowDensity,
            first_viol.control,
            format!("{report}"),
        ))
    }
}

/// Check that a truncated label satisfies the automatic tooltip obligation (`UIF-VER-005`, `UIF-PRIN-007`).
///
/// If the label text contains the Unicode truncation mark `…` ([`ELLIPSIS`]), it MUST carry
/// a non-empty tooltip that exposes the untruncated string (and does not itself contain `…`).
pub fn check_label_truncation_tooltip<'a>(slot: &LabelSlot<'a>) -> Result<(), InvariantViolation> {
    if slot.text.contains(ELLIPSIS) {
        match slot.tooltip {
            None => {
                let v = TruncationTooltipViolation::MissingTooltip {
                    id: slot.id,
                    full: slot.text,
                };
                Err(InvariantViolation::new(
                    InvariantCategory::TooltipObligation,
                    slot.id,
                    format!("{v}"),
                ))
            }
            Some(tooltip) if tooltip == slot.text || tooltip.contains(ELLIPSIS) => {
                let v = TruncationTooltipViolation::TooltipNotFullText {
                    id: slot.id,
                    full: slot.text,
                    tooltip,
                };
                Err(InvariantViolation::new(
                    InvariantCategory::TooltipObligation,
                    slot.id,
                    format!("{v}"),
                ))
            }
            Some(_) => Ok(()),
        }
    } else {
        Ok(())
    }
}

/// Check that an icon-only button satisfies the automatic tooltip obligation (`UIF-VER-005`, `UIF-ATOM-001`).
///
/// An icon-only button carries no textual label in its content box, so its label lives
/// entirely in its tooltip (`UIF-NAV-003`, `UIF-TAB-003`). Building an icon-only button
/// without a non-empty tooltip is forbidden.
pub fn check_icon_only_button_tooltip(
    is_icon_only: bool,
    tooltip_label: Option<&str>,
) -> Result<(), InvariantViolation> {
    if is_icon_only {
        match tooltip_label {
            None => Err(InvariantViolation::new(
                InvariantCategory::TooltipObligation,
                "icon_only_button",
                "an icon-only button must carry a tooltip (`UIF-ATOM-001` icon-only cell depends on `UIF-NAV-003`/`UIF-TAB-003`)",
            )),
            Some(label) if label.trim().is_empty() => Err(InvariantViolation::new(
                InvariantCategory::TooltipObligation,
                "icon_only_button",
                "icon-only button tooltip must not be empty",
            )),
            Some(_) => Ok(()),
        }
    } else {
        Ok(())
    }
}

/// Check that a secret value is properly redacted in `Debug` and `Display` (`UIF-VER-005`, `NFR-009`).
///
/// Asserts that the plaintext password does not appear in `Debug` or `Display` outputs,
/// and that the expected mask character ([`SECRET_MASK`]) appears when plaintext is non-empty.
pub fn check_secret_redaction<T: fmt::Debug + fmt::Display>(
    secret: &T,
    plaintext: &str,
) -> Result<(), InvariantViolation> {
    let debug_out = format!("{secret:?}");
    let display_out = format!("{secret}");

    if !plaintext.is_empty() {
        if debug_out.contains(plaintext) {
            return Err(InvariantViolation::new(
                InvariantCategory::SecretRedaction,
                "SecretInput",
                format!("plaintext secret leaked in Debug representation: '{debug_out}'"),
            ));
        }
        if display_out.contains(plaintext) {
            return Err(InvariantViolation::new(
                InvariantCategory::SecretRedaction,
                "SecretInput",
                format!("plaintext secret leaked in Display representation: '{display_out}'"),
            ));
        }
        if !debug_out.contains(SECRET_MASK) && !display_out.contains(SECRET_MASK) {
            return Err(InvariantViolation::new(
                InvariantCategory::SecretRedaction,
                "SecretInput",
                format!(
                    "neither Debug nor Display output contains expected mask character '{SECRET_MASK}'"
                ),
            ));
        }
    }

    Ok(())
}

/// Check validation stance transition adherence to "Reward Early, Punish Late" (`UIF-VER-005`, `UIF-PRIN-009`).
///
/// Evaluates stance, invalidity and disallowed character presence through [`punish`].
pub fn check_validation_transition(
    stance: EditStance,
    invalid: bool,
    disallowed: bool,
) -> Punishment {
    punish(stance, invalid, disallowed)
}

/// Run an audit of all seven automated invariants over representative fixture sets (`UIF-VER-005`).
///
/// Verifies:
/// 1. Sentence-case compliance on standard chrome copy and pinned literal exemptions.
/// 2. Ellipsis biconditional on prompt-launching, immediate and pinned actions.
/// 3. Single focal point compliance across container scopes.
/// 4. Control density homogeneity across Compact and Standard rungs.
/// 5. Automatic tooltip obligation for truncated labels and icon-only buttons.
/// 6. Secret redaction under `Debug` and `Display`.
/// 7. "Reward Early, Punish Late" validation transitions.
///
/// Returns an [`InvariantReport`] reporting clean pass across all 7 invariants.
#[must_use]
pub fn audit_invariants() -> InvariantReport {
    let mut report = InvariantReport::new();

    // 1. Sentence Case
    report.record_result(check_sentence_case("Save changes", true));
    report.record_result(check_sentence_case("Export as PDF", true));
    report.record_result(check_sentence_case("Open in Editor", true)); // Pinned
    report.record_result(check_sentence_case("RAW USER INPUT", false)); // Non-chrome

    // 2. Action Ellipsis
    report.record_result(check_action_ellipsis("Save", PromptBehaviour::Immediate));
    report.record_result(check_action_ellipsis(
        "Rename...",
        PromptBehaviour::LaunchesPrompt,
    ));
    report.record_result(check_action_ellipsis(
        "Create Directory ...",
        PromptBehaviour::LaunchesPrompt,
    )); // Pinned
    let nav_trigger_immediate = crate::composite::navbar::NavBarMenuTrigger::new("file", "File");
    report.record_result(check_navbar_trigger_ellipsis(&nav_trigger_immediate));
    let nav_trigger_prompt =
        crate::composite::navbar::NavBarMenuTrigger::new("rename", "Rename...")
            .with_dialog_prompt(true);
    report.record_result(check_navbar_trigger_ellipsis(&nav_trigger_prompt));
    let nav_trigger_icon = crate::composite::navbar::NavBarMenuTrigger::new("export", "")
        .with_tooltip("Export...")
        .with_dialog_prompt(true);
    report.record_result(check_navbar_trigger_ellipsis(&nav_trigger_icon));

    // 3. Emphasis Focal Point
    let modal_slots = [
        EmphasisSlot::high("Confirm"),
        EmphasisSlot::new("Cancel", crate::layout::emphasis::Emphasis::Medium),
    ];
    report.record_result(check_emphasis_focal_point(
        ContainerScope::ModalFooter,
        &modal_slots,
    ));

    let toolbar_slots = [
        EmphasisSlot::new("Bold", crate::layout::emphasis::Emphasis::Quiet),
        EmphasisSlot::new("Italic", crate::layout::emphasis::Emphasis::Quiet),
    ];
    report.record_result(check_emphasis_focal_point(
        ContainerScope::ToolbarCluster,
        &toolbar_slots,
    ));

    // 4. Row Density
    let compact_row = [
        RowSlot::at_density("btn_prev", ControlDensity::Compact),
        RowSlot::at_density("btn_next", ControlDensity::Compact),
    ];
    report.record_result(check_row_density(
        Some(ControlDensity::Compact),
        &compact_row,
    ));

    let standard_row = [
        RowSlot::at_density("field_name", ControlDensity::Standard),
        RowSlot::at_density("field_email", ControlDensity::Standard),
    ];
    report.record_result(check_row_density(None, &standard_row));

    // 5. Tooltip Obligation
    let truncated_slot = LabelSlot {
        id: "long_path",
        text: "src/…/invariant_gate.rs",
        tooltip: Some("src/hoarfrost/invariant_gate.rs"),
    };
    report.record_result(check_label_truncation_tooltip(&truncated_slot));

    let short_slot = LabelSlot {
        id: "short_label",
        text: "Save",
        tooltip: None,
    };
    report.record_result(check_label_truncation_tooltip(&short_slot));

    report.record_result(check_icon_only_button_tooltip(true, Some("Settings")));
    report.record_result(check_icon_only_button_tooltip(false, None));

    // 6. Secret Redaction
    let secret = crate::widget::password_model::SecretInput::new("sensitive-auth-token-987");
    report.record_result(check_secret_redaction(&secret, "sensitive-auth-token-987"));

    let empty_secret = crate::widget::password_model::SecretInput::empty();
    report.record_result(check_secret_redaction(&empty_secret, ""));

    // 7. Validation Transition
    let p_pristine = check_validation_transition(EditStance::Pristine, true, false);
    if !p_pristine.withheld() || p_pristine.painted() || !p_pristine.invalid() {
        report.record_result(Err(InvariantViolation::new(
            InvariantCategory::ValidationPunishment,
            "PristineInvalid",
            "Pristine invalid input must withhold error visual",
        )));
    } else {
        report.record_check();
    }

    let p_touched = check_validation_transition(EditStance::Touched, true, false);
    if !p_touched.painted() || !p_touched.invalid() {
        report.record_result(Err(InvariantViolation::new(
            InvariantCategory::ValidationPunishment,
            "TouchedInvalid",
            "Touched invalid input must paint error visual",
        )));
    } else {
        report.record_check();
    }

    let p_disallowed = check_validation_transition(EditStance::Pristine, false, true);
    if !p_disallowed.painted() || !p_disallowed.invalid() {
        report.record_result(Err(InvariantViolation::new(
            InvariantCategory::ValidationPunishment,
            "DisallowedImmediate",
            "Disallowed character must paint error visual immediately regardless of stance",
        )));
    } else {
        report.record_check();
    }

    let p_valid = check_validation_transition(EditStance::Touched, false, false);
    if p_valid.painted() || p_valid.invalid() {
        report.record_result(Err(InvariantViolation::new(
            InvariantCategory::ValidationPunishment,
            "RewardEarly",
            "Valid input must clear error visual immediately",
        )));
    } else {
        report.record_check();
    }

    report
}

#[cfg(test)]
#[path = "invariant_gate_tests.rs"]
mod tests;
