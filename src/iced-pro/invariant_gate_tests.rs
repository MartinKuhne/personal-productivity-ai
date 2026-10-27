//! Unit tests for the Style Guide, Microcopy & Ergonomic Invariant Gate (`UIF-VER-005`).

use std::fmt;

use super::*;
use crate::layout::density::{ControlDensity, RowSlot};
use crate::layout::emphasis::{ContainerScope, Emphasis, EmphasisSlot};
use crate::microcopy::action_label::PromptBehaviour;
use crate::microcopy::truncate::LabelSlot;
use crate::validation::stance::EditStance;
use crate::widget::password_model::SecretInput;

// ---------------------------------------------------------------------------
// 1. Sentence-case chrome labels
// ---------------------------------------------------------------------------

#[test]
fn test_sentence_case_happy_paths() {
    assert!(check_sentence_case("Save changes", true).is_ok());
    assert!(check_sentence_case("Open file", true).is_ok());
    assert!(check_sentence_case("Export document as PDF", true).is_ok());
    assert!(check_sentence_case("Format JSON", true).is_ok());
    assert!(check_sentence_case("Configure MCP", true).is_ok());
    assert!(check_sentence_case("3 items selected", true).is_ok());
}

#[test]
fn test_sentence_case_skips_non_chrome() {
    assert!(check_sentence_case("ALL CAPS PROSE TITLE", false).is_ok());
    assert!(check_sentence_case("Title Case Document Heading", false).is_ok());
    assert!(check_sentence_case("lower case raw input", false).is_ok());
}

#[test]
fn test_sentence_case_pinned_literals_are_exempt() {
    let pinned_exempt = [
        "Open in Editor",
        "Show in File Explorer",
        "Create Directory ...",
        "Close Others",
        "Close All",
        "Copy Path",
        "Windows...",
    ];
    for literal in pinned_exempt {
        assert!(
            check_sentence_case(literal, true).is_ok(),
            "pinned literal '{literal}' must be exempt from sentence-case gate"
        );
    }
}

#[test]
fn test_sentence_case_violations_flagged() {
    // Title Case violation
    let res_title = check_sentence_case("Save Changes", true);
    assert!(res_title.is_err());
    let v_title = res_title.unwrap_err();
    assert_eq!(v_title.category, InvariantCategory::SentenceCase);
    assert_eq!(v_title.target, "Save Changes");

    // ALL CAPS violation
    let res_caps = check_sentence_case("SAVE ALL", true);
    assert!(res_caps.is_err());
    assert_eq!(
        res_caps.unwrap_err().category,
        InvariantCategory::SentenceCase
    );

    // Lowercase first letter violation
    let res_lower = check_sentence_case("save changes", true);
    assert!(res_lower.is_err());
    assert_eq!(
        res_lower.unwrap_err().category,
        InvariantCategory::SentenceCase
    );

    // Mid-word capital violation
    let res_mid = check_sentence_case("saveChanges", true);
    assert!(res_mid.is_err());
    assert_eq!(
        res_mid.unwrap_err().category,
        InvariantCategory::SentenceCase
    );
}

// ---------------------------------------------------------------------------
// 2. Action ellipsis biconditional
// ---------------------------------------------------------------------------

#[test]
fn test_action_ellipsis_happy_paths() {
    assert!(check_action_ellipsis("Save", PromptBehaviour::Immediate).is_ok());
    assert!(check_action_ellipsis("Close", PromptBehaviour::Immediate).is_ok());
    assert!(check_action_ellipsis("Rename...", PromptBehaviour::LaunchesPrompt).is_ok());
    assert!(check_action_ellipsis("Move...", PromptBehaviour::LaunchesPrompt).is_ok());
    assert!(check_action_ellipsis("Export as PDF...", PromptBehaviour::LaunchesPrompt).is_ok());
}

#[test]
fn test_action_ellipsis_pinned_literal_exempt() {
    // UI-015 pins 'Create Directory ...' with space before ellipsis
    assert!(check_action_ellipsis("Create Directory ...", PromptBehaviour::LaunchesPrompt).is_ok());
}

#[test]
fn test_action_ellipsis_violations() {
    // Spurious ellipsis on immediate action
    let res_spur = check_action_ellipsis("Save...", PromptBehaviour::Immediate);
    assert!(res_spur.is_err());
    let v_spur = res_spur.unwrap_err();
    assert_eq!(v_spur.category, InvariantCategory::ActionEllipsis);
    assert!(v_spur.message.contains("runs immediately"));

    // Missing ellipsis on prompt-launching action
    let res_miss = check_action_ellipsis("Rename", PromptBehaviour::LaunchesPrompt);
    assert!(res_miss.is_err());
    let v_miss = res_miss.unwrap_err();
    assert_eq!(v_miss.category, InvariantCategory::ActionEllipsis);
    assert!(v_miss.message.contains("opens a secondary prompt"));

    // Spaced ellipsis on non-pinned prompt action
    let res_spaced = check_action_ellipsis("Rename ...", PromptBehaviour::LaunchesPrompt);
    assert!(res_spaced.is_err());
    assert!(res_spaced.unwrap_err().message.contains("spaced"));

    // Unicode ellipsis on action label
    let res_unicode = check_action_ellipsis("Rename…", PromptBehaviour::LaunchesPrompt);
    assert!(res_unicode.is_err());
    assert!(res_unicode.unwrap_err().message.contains("U+2026"));
}

#[test]
fn test_navbar_trigger_ellipsis_gate_compliance() {
    // Valid immediate trigger
    let trig_imm = crate::composite::navbar::NavBarMenuTrigger::new("file", "File");
    assert!(check_navbar_trigger_ellipsis(&trig_imm).is_ok());

    // Valid dialog trigger
    let trig_dlg = crate::composite::navbar::NavBarMenuTrigger::new("rename", "Rename...")
        .with_dialog_prompt(true);
    assert!(check_navbar_trigger_ellipsis(&trig_dlg).is_ok());

    // Valid icon-only dialog trigger
    let trig_icon = crate::composite::navbar::NavBarMenuTrigger::new("export", "")
        .with_tooltip("Export...")
        .with_dialog_prompt(true);
    assert!(check_navbar_trigger_ellipsis(&trig_icon).is_ok());
}

#[test]
fn test_navbar_trigger_ellipsis_gate_violations() {
    // Button launching a dialog without `...` is rejected
    let trig_no_ell = crate::composite::navbar::NavBarMenuTrigger::new("rename", "Rename")
        .with_dialog_prompt(true);
    let res_no_ell = check_navbar_trigger_ellipsis(&trig_no_ell);
    assert!(res_no_ell.is_err());
    let v_no_ell = res_no_ell.unwrap_err();
    assert_eq!(v_no_ell.category, InvariantCategory::ActionEllipsis);
    assert_eq!(v_no_ell.target, "Rename");
    assert!(v_no_ell.message.contains("opens a secondary prompt"));

    // Immediate button with `...` is rejected
    let trig_spurious = crate::composite::navbar::NavBarMenuTrigger::new("save", "Save...");
    let res_spur = check_navbar_trigger_ellipsis(&trig_spurious);
    assert!(res_spur.is_err());
    let v_spur = res_spur.unwrap_err();
    assert_eq!(v_spur.category, InvariantCategory::ActionEllipsis);
    assert_eq!(v_spur.target, "Save...");
    assert!(v_spur.message.contains("runs immediately"));

    // Icon-only button tooltip without `...` when opening a dialog is rejected
    let trig_icon_no_ell = crate::composite::navbar::NavBarMenuTrigger::new("save_as", "")
        .with_tooltip("Save As")
        .with_dialog_prompt(true);
    let res_icon = check_navbar_trigger_ellipsis(&trig_icon_no_ell);
    assert!(res_icon.is_err());
    let v_icon = res_icon.unwrap_err();
    assert_eq!(v_icon.category, InvariantCategory::ActionEllipsis);
    assert_eq!(v_icon.target, "Save As");
    assert!(v_icon.message.contains("opens a secondary prompt"));
}

// ---------------------------------------------------------------------------
// 3. Single high-emphasis focal point
// ---------------------------------------------------------------------------

#[test]
fn test_emphasis_focal_point_compliant() {
    for scope in ContainerScope::ALL {
        // Empty scope
        assert!(check_emphasis_focal_point(scope, &[]).is_ok());

        // Zero high emphasis
        let quiet_slots = [
            EmphasisSlot::new("Btn1", Emphasis::Medium),
            EmphasisSlot::new("Btn2", Emphasis::Quiet),
        ];
        assert!(check_emphasis_focal_point(scope, &quiet_slots).is_ok());

        // Exactly one high emphasis
        let one_high = [
            EmphasisSlot::high("PrimaryBtn"),
            EmphasisSlot::new("SecondaryBtn", Emphasis::Medium),
            EmphasisSlot::new("GhostBtn", Emphasis::Quiet),
        ];
        assert!(check_emphasis_focal_point(scope, &one_high).is_ok());
    }
}

#[test]
fn test_emphasis_focal_point_violations() {
    let two_primary = [
        EmphasisSlot::high("PrimaryConfirm"),
        EmphasisSlot::high("PrimaryApply"),
    ];
    let res_primary = check_emphasis_focal_point(ContainerScope::ModalFooter, &two_primary);
    assert!(res_primary.is_err());
    let v = res_primary.unwrap_err();
    assert_eq!(v.category, InvariantCategory::EmphasisFocalPoint);
    assert_eq!(v.target, "modal-footer");
    assert!(v.message.contains("PrimaryConfirm"));
    assert!(v.message.contains("PrimaryApply"));

    let primary_and_danger = [
        EmphasisSlot::high("PrimarySave"),
        EmphasisSlot::high("DangerDelete"),
    ];
    let res_danger = check_emphasis_focal_point(ContainerScope::Card, &primary_and_danger);
    assert!(res_danger.is_err());
    assert_eq!(res_danger.unwrap_err().target, "card");
}

// ---------------------------------------------------------------------------
// 4. Homogeneous row density
// ---------------------------------------------------------------------------

#[test]
fn test_row_density_compliant() {
    // Declared Compact
    let compact_slots = [
        RowSlot::at_density("c1", ControlDensity::Compact),
        RowSlot::at_density("c2", ControlDensity::Compact),
    ];
    assert!(check_row_density(Some(ControlDensity::Compact), &compact_slots).is_ok());

    // Declared Standard
    let standard_slots = [
        RowSlot::at_density("s1", ControlDensity::Standard),
        RowSlot::at_density("s2", ControlDensity::Standard),
    ];
    assert!(check_row_density(Some(ControlDensity::Standard), &standard_slots).is_ok());

    // Inferred homogeneous
    assert!(check_row_density(None, &compact_slots).is_ok());
    assert!(check_row_density(None, &standard_slots).is_ok());

    // Empty row
    assert!(check_row_density(Some(ControlDensity::Compact), &[]).is_ok());
    assert!(check_row_density(None, &[]).is_ok());
}

#[test]
fn test_row_density_violations() {
    let mixed_slots = [
        RowSlot::at_density("compact_btn", ControlDensity::Compact),
        RowSlot::at_density("standard_field", ControlDensity::Standard),
    ];

    // Mixed row declared Compact
    let res_c = check_row_density(Some(ControlDensity::Compact), &mixed_slots);
    assert!(res_c.is_err());
    let v_c = res_c.unwrap_err();
    assert_eq!(v_c.category, InvariantCategory::RowDensity);
    assert_eq!(v_c.target, "standard_field");

    // Mixed row inferred
    let res_inferred = check_row_density(None, &mixed_slots);
    assert!(res_inferred.is_err());
    assert_eq!(res_inferred.unwrap_err().target, "standard_field");

    // Off-rung control
    let off_rung_slot = [RowSlot::new("weird_input", 25.0)];
    let res_off = check_row_density(None, &off_rung_slot);
    assert!(res_off.is_err());
    let v_off = res_off.unwrap_err();
    assert_eq!(v_off.target, "weird_input");
    assert!(v_off.message.contains("25px"));
}

// ---------------------------------------------------------------------------
// 5. Automatic tooltip obligation
// ---------------------------------------------------------------------------

#[test]
fn test_label_truncation_tooltip_happy_paths() {
    // Truncated label with non-empty, full tooltip
    let truncated_with_tip = LabelSlot {
        id: "path_slot",
        text: "src/…/mod.rs",
        tooltip: Some("src/iced-pro/mod.rs"),
    };
    assert!(check_label_truncation_tooltip(&truncated_with_tip).is_ok());

    // Untruncated label does not require tooltip
    let short_slot = LabelSlot {
        id: "short_slot",
        text: "Cancel",
        tooltip: None,
    };
    assert!(check_label_truncation_tooltip(&short_slot).is_ok());
}

#[test]
fn test_label_truncation_tooltip_violations() {
    // Truncated label without tooltip
    let no_tip = LabelSlot {
        id: "missing_tip",
        text: "src/app/…/main.rs",
        tooltip: None,
    };
    let res_no_tip = check_label_truncation_tooltip(&no_tip);
    assert!(res_no_tip.is_err());
    let v = res_no_tip.unwrap_err();
    assert_eq!(v.category, InvariantCategory::TooltipObligation);
    assert_eq!(v.target, "missing_tip");
    assert!(v.message.contains("no tooltip"));

    // Truncated label where tooltip merely repeats truncated text
    let truncated_tip = LabelSlot {
        id: "echo_tip",
        text: "Documents/…/report.pdf",
        tooltip: Some("Documents/…/report.pdf"),
    };
    let res_echo = check_label_truncation_tooltip(&truncated_tip);
    assert!(res_echo.is_err());
    assert!(
        res_echo
            .unwrap_err()
            .message
            .contains("requires the full untruncated string")
    );
}

#[test]
fn test_icon_only_button_tooltip() {
    // Icon-only with valid tooltip
    assert!(check_icon_only_button_tooltip(true, Some("Close window")).is_ok());

    // Labelled button without tooltip is allowed
    assert!(check_icon_only_button_tooltip(false, None).is_ok());

    // Icon-only with missing tooltip
    let res_missing = check_icon_only_button_tooltip(true, None);
    assert!(res_missing.is_err());
    let v_missing = res_missing.unwrap_err();
    assert_eq!(v_missing.category, InvariantCategory::TooltipObligation);
    assert!(
        v_missing
            .message
            .contains("icon-only button must carry a tooltip")
    );

    // Icon-only with empty / whitespace tooltip
    let res_empty = check_icon_only_button_tooltip(true, Some("   "));
    assert!(res_empty.is_err());
    assert!(res_empty.unwrap_err().message.contains("must not be empty"));
}

// ---------------------------------------------------------------------------
// 6. Secret redaction (`NFR-009`)
// ---------------------------------------------------------------------------

#[test]
fn test_secret_redaction_secret_input() {
    let secret = SecretInput::new("hunter2_super_secret");
    assert!(check_secret_redaction(&secret, "hunter2_super_secret").is_ok());

    let empty_secret = SecretInput::empty();
    assert!(check_secret_redaction(&empty_secret, "").is_ok());
}

struct LeakyDebugSecret(&'static str);
impl fmt::Debug for LeakyDebugSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Leaky({})", self.0)
    }
}
impl fmt::Display for LeakyDebugSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("••••")
    }
}

struct LeakyDisplaySecret(&'static str);
impl fmt::Debug for LeakyDisplaySecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("••••")
    }
}
impl fmt::Display for LeakyDisplaySecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

struct UnmaskedSecret;
impl fmt::Debug for UnmaskedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}
impl fmt::Display for UnmaskedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[test]
fn test_secret_redaction_detects_leaks() {
    // Leak in Debug
    let leaky_debug = LeakyDebugSecret("plaintext123");
    let res_debug = check_secret_redaction(&leaky_debug, "plaintext123");
    assert!(res_debug.is_err());
    let v_d = res_debug.unwrap_err();
    assert_eq!(v_d.category, InvariantCategory::SecretRedaction);
    assert!(v_d.message.contains("Debug representation"));

    // Leak in Display
    let leaky_display = LeakyDisplaySecret("plaintext456");
    let res_display = check_secret_redaction(&leaky_display, "plaintext456");
    assert!(res_display.is_err());
    let v_disp = res_display.unwrap_err();
    assert_eq!(v_disp.category, InvariantCategory::SecretRedaction);
    assert!(v_disp.message.contains("Display representation"));

    // Missing mask character
    let unmasked = UnmaskedSecret;
    let res_unmasked = check_secret_redaction(&unmasked, "plaintext789");
    assert!(res_unmasked.is_err());
    assert!(
        res_unmasked
            .unwrap_err()
            .message
            .contains("expected mask character")
    );
}

// ---------------------------------------------------------------------------
// 7. Validation transition truth table (`UIF-PRIN-009`)
// ---------------------------------------------------------------------------

#[test]
fn test_validation_transition_truth_table() {
    // 1. Pristine, valid, no disallowed -> clean
    let p1 = check_validation_transition(EditStance::Pristine, false, false);
    assert!(!p1.invalid());
    assert!(!p1.painted());
    assert!(!p1.withheld());

    // 2. Pristine, invalid, no disallowed -> WITHHELD (Punish Late)
    let p2 = check_validation_transition(EditStance::Pristine, true, false);
    assert!(p2.invalid());
    assert!(!p2.painted());
    assert!(p2.withheld());

    // 3. Pristine, disallowed -> PAINTED (Carve-out outranks pristine)
    let p3 = check_validation_transition(EditStance::Pristine, false, true);
    assert!(p3.invalid());
    assert!(p3.painted());
    assert!(!p3.withheld());

    // 4. Pristine, invalid + disallowed -> PAINTED
    let p4 = check_validation_transition(EditStance::Pristine, true, true);
    assert!(p4.invalid());
    assert!(p4.painted());

    // 5. Touched, valid, no disallowed -> clean (Reward Early)
    let p5 = check_validation_transition(EditStance::Touched, false, false);
    assert!(!p5.invalid());
    assert!(!p5.painted());
    assert!(!p5.withheld());

    // 6. Touched, invalid, no disallowed -> PAINTED (Punish Touched)
    let p6 = check_validation_transition(EditStance::Touched, true, false);
    assert!(p6.invalid());
    assert!(p6.painted());
    assert!(!p6.withheld());

    // 7. Touched, disallowed -> PAINTED
    let p7 = check_validation_transition(EditStance::Touched, false, true);
    assert!(p7.invalid());
    assert!(p7.painted());

    // 8. Touched, invalid + disallowed -> PAINTED
    let p8 = check_validation_transition(EditStance::Touched, true, true);
    assert!(p8.invalid());
    assert!(p8.painted());
}

// ---------------------------------------------------------------------------
// 8. InvariantReport and audit_invariants
// ---------------------------------------------------------------------------

#[test]
fn test_audit_invariants_runs_clean() {
    let report = audit_invariants();
    assert!(
        report.is_clean(),
        "audit_invariants() must have zero violations; found: {:?}",
        report.violations
    );
    assert_eq!(report.violation_count(), 0);
    assert!(
        report.total_checked >= 15,
        "audit_invariants() must execute a representative suite across all 7 invariants"
    );
    let desc = report.describe();
    assert!(desc.contains("PASS"));
    assert!(desc.contains("0 violations"));
    assert_eq!(format!("{report}"), desc);
}

#[test]
fn test_invariant_report_records_violations() {
    let mut report = InvariantReport::new();
    assert!(report.is_clean());
    assert_eq!(report.violation_count(), 0);

    report.record_result(Ok(()));
    assert!(report.is_clean());
    assert_eq!(report.total_checked, 1);

    report.record_result(Err(InvariantViolation::new(
        InvariantCategory::SentenceCase,
        "bad_label",
        "Title Case violation",
    )));
    assert!(!report.is_clean());
    assert_eq!(report.violation_count(), 1);
    assert_eq!(report.total_checked, 2);

    let desc = report.describe();
    assert!(desc.contains("FAIL"));
    assert!(desc.contains("[sentence-case] bad_label: Title Case violation"));
}

#[test]
fn test_invariant_category_slugs() {
    let slugs: Vec<&'static str> = InvariantCategory::ALL.iter().map(|c| c.slug()).collect();
    let set: std::collections::BTreeSet<&'static str> = slugs.iter().copied().collect();
    assert_eq!(slugs.len(), set.len(), "slugs must be unique");
    for cat in InvariantCategory::ALL {
        assert_eq!(cat.slug(), format!("{cat}"));
    }
}
