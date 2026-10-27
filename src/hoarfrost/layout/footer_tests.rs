//! Tests for [`super::footer`] — the ordered footer composition and its
//! destructive-focus mandate (`UIF-PRIN-006`).

use super::*;
use crate::layout::density::ControlDensity;
use crate::token::space;

/// The footer §1.1 describes: utility far-left, `[Secondary / Cancel]`,
/// `[Primary / Confirm]`, destructive, focused on Cancel.
fn canonical_destructive() -> FooterPlan<'static> {
    FooterPlan {
        kind: FooterKind::Destructive,
        density: ControlDensity::Compact,
        scope: ContainerScope::ModalFooter,
        actions: vec![
            FooterAction::new("help", FooterSlot::Utility, Emphasis::Quiet),
            FooterAction::new("cancel", FooterSlot::Cancel, Emphasis::Medium),
            FooterAction::new("delete", FooterSlot::Confirm, Emphasis::High),
        ],
        initial_focus: Some("cancel"),
    }
}

/// An ordinary accept / discard footer with no focus mandate.
fn canonical_plain() -> FooterPlan<'static> {
    FooterPlan {
        kind: FooterKind::Plain,
        density: ControlDensity::Standard,
        scope: ContainerScope::Form,
        actions: vec![
            FooterAction::new("reset", FooterSlot::Utility, Emphasis::Quiet),
            FooterAction::new("cancel", FooterSlot::Cancel, Emphasis::Medium),
            FooterAction::new("save", FooterSlot::Confirm, Emphasis::High),
        ],
        initial_focus: None,
    }
}

// ---------------------------------------------------------------------------
// Slot geometry
// ---------------------------------------------------------------------------

#[test]
fn test_slot_order_is_utility_cancel_confirm() {
    assert_eq!(
        FooterSlot::ORDER,
        [FooterSlot::Utility, FooterSlot::Cancel, FooterSlot::Confirm]
    );
    assert_eq!(FooterSlot::ALL, FooterSlot::ORDER);
}

#[test]
fn test_slot_positions_are_ascending_left_to_right() {
    assert_eq!(FooterSlot::Utility.position(), 0);
    assert_eq!(FooterSlot::Cancel.position(), 1);
    assert_eq!(FooterSlot::Confirm.position(), 2);
}

#[test]
fn test_only_the_utility_slot_is_far_left_pinned() {
    assert!(FooterSlot::Utility.is_left_pinned());
    assert!(!FooterSlot::Cancel.is_left_pinned());
    assert!(!FooterSlot::Confirm.is_left_pinned());
}

#[test]
fn test_the_decision_group_is_right_aligned() {
    assert_eq!(FooterSlot::Utility.alignment(), FooterAlignment::FarLeft);
    assert_eq!(FooterSlot::Cancel.alignment(), FooterAlignment::Right);
    assert_eq!(FooterSlot::Confirm.alignment(), FooterAlignment::Right);
}

#[test]
fn test_group_gap_is_the_space_sm_token() {
    assert_eq!(FooterPlan::group_gap(), space::SM);
    assert_eq!(GROUP_GAP, space::SM);
}

#[test]
fn test_slot_slugs_are_unique() {
    let slugs: Vec<&str> = FooterSlot::ALL.iter().map(|s| s.slug()).collect();
    let unique: std::collections::BTreeSet<&str> = slugs.iter().copied().collect();
    assert_eq!(slugs.len(), unique.len(), "slot slugs collide: {slugs:?}");
}

// ---------------------------------------------------------------------------
// The compliant shapes
// ---------------------------------------------------------------------------

#[test]
fn test_canonical_destructive_footer_is_valid() {
    let plan = canonical_destructive();
    assert_eq!(plan.validate(), Vec::new(), "{plan:?}");
    assert!(plan.is_valid());
}

#[test]
fn test_canonical_plain_footer_is_valid() {
    let plan = canonical_plain();
    assert_eq!(plan.validate(), Vec::new(), "{plan:?}");
    assert!(plan.is_valid());
}

#[test]
fn test_a_footer_with_several_utility_actions_is_valid() {
    let mut plan = canonical_plain();
    plan.actions.insert(
        0,
        FooterAction::new("docs", FooterSlot::Utility, Emphasis::Quiet),
    );
    assert_eq!(plan.validate(), Vec::new(), "{plan:?}");
}

#[test]
fn test_a_footer_with_only_a_confirm_is_valid() {
    let mut plan = canonical_plain();
    plan.actions = vec![FooterAction::new(
        "save",
        FooterSlot::Confirm,
        Emphasis::High,
    )];
    assert_eq!(plan.validate(), Vec::new(), "{plan:?}");
}

#[test]
fn test_an_empty_footer_is_valid() {
    let plan = FooterPlan::new(FooterKind::Plain, ControlDensity::Standard);
    assert_eq!(plan.validate(), Vec::new());
}

// ---------------------------------------------------------------------------
// Order
// ---------------------------------------------------------------------------

#[test]
fn test_confirm_before_cancel_is_out_of_order() {
    let mut plan = canonical_plain();
    plan.actions = vec![
        FooterAction::new("save", FooterSlot::Confirm, Emphasis::High),
        FooterAction::new("cancel", FooterSlot::Cancel, Emphasis::Medium),
    ];
    let violations = plan.validate();
    assert_eq!(
        violations,
        vec![FooterViolation::OutOfOrder {
            earlier: "save",
            later: "cancel",
        }],
        "{violations:?}"
    );
}

#[test]
fn test_a_decision_control_before_a_utility_action_is_out_of_order() {
    let mut plan = canonical_plain();
    plan.actions = vec![
        FooterAction::new("cancel", FooterSlot::Cancel, Emphasis::Medium),
        FooterAction::new("help", FooterSlot::Utility, Emphasis::Quiet),
        FooterAction::new("save", FooterSlot::Confirm, Emphasis::High),
    ];
    let violations = plan.validate();
    assert!(
        violations.contains(&FooterViolation::OutOfOrder {
            earlier: "cancel",
            later: "help",
        }),
        "{violations:?}"
    );
}

#[test]
fn test_every_inversion_is_reported_not_just_the_first() {
    let mut plan = canonical_plain();
    plan.actions = vec![
        FooterAction::new("save", FooterSlot::Confirm, Emphasis::High),
        FooterAction::new("cancel", FooterSlot::Cancel, Emphasis::Medium),
        FooterAction::new("help", FooterSlot::Utility, Emphasis::Quiet),
    ];
    let inversions = plan
        .validate()
        .into_iter()
        .filter(|v| matches!(v, FooterViolation::OutOfOrder { .. }))
        .count();
    assert_eq!(inversions, 3, "all three pairs are inverted");
}

// ---------------------------------------------------------------------------
// Slot emphasis
// ---------------------------------------------------------------------------

#[test]
fn test_a_high_emphasis_cancel_is_rejected() {
    let mut plan = canonical_destructive();
    plan.actions[1] = FooterAction::new("cancel", FooterSlot::Cancel, Emphasis::High);
    let violations = plan.validate();
    assert!(
        violations.contains(&FooterViolation::CancelIsHighEmphasis { control: "cancel" }),
        "{violations:?}"
    );
}

#[test]
fn test_a_ghost_confirm_is_rejected() {
    let mut plan = canonical_plain();
    plan.actions[2] = FooterAction::new("save", FooterSlot::Confirm, Emphasis::Quiet);
    let violations = plan.validate();
    assert!(
        violations.contains(&FooterViolation::ConfirmIsNotHighEmphasis {
            control: "save",
            actual: "quiet",
        }),
        "{violations:?}"
    );
}

#[test]
fn test_a_secondary_confirm_is_rejected_naming_what_it_actually_is() {
    let mut plan = canonical_plain();
    plan.actions[2] = FooterAction::new("save", FooterSlot::Confirm, Emphasis::Medium);
    let violations = plan.validate();
    assert!(
        violations.contains(&FooterViolation::ConfirmIsNotHighEmphasis {
            control: "save",
            actual: "medium",
        }),
        "{violations:?}"
    );
}

#[test]
fn test_two_high_emphasis_controls_are_reported_as_multiple() {
    let mut plan = canonical_plain();
    plan.actions[0] = FooterAction::new("reset", FooterSlot::Utility, Emphasis::High);
    let violations = plan.validate();
    assert!(
        violations.contains(&FooterViolation::MultipleHighEmphasis {
            controls: vec!["reset", "save"],
        }),
        "{violations:?}"
    );
}

// ---------------------------------------------------------------------------
// The destructive-focus mandate
// ---------------------------------------------------------------------------

#[test]
fn test_destructive_footer_without_initial_focus_is_rejected() {
    let mut plan = canonical_destructive();
    plan.initial_focus = None;
    let violations = plan.validate();
    assert_eq!(
        violations,
        vec![FooterViolation::DestructiveWithoutInitialFocus],
        "{violations:?}"
    );
}

#[test]
fn test_destructive_footer_focusing_the_confirm_is_rejected() {
    let mut plan = canonical_destructive();
    plan.initial_focus = Some("delete");
    let violations = plan.validate();
    assert_eq!(
        violations,
        vec![FooterViolation::InitialFocusNotCancel {
            focused: "delete",
            expected: "cancel",
        }],
        "{violations:?}"
    );
}

#[test]
fn test_destructive_footer_focusing_a_utility_action_is_rejected() {
    let mut plan = canonical_destructive();
    plan.initial_focus = Some("help");
    let violations = plan.validate();
    assert!(
        violations.contains(&FooterViolation::InitialFocusNotCancel {
            focused: "help",
            expected: "cancel",
        }),
        "{violations:?}"
    );
}

#[test]
fn test_destructive_footer_with_no_cancel_action_is_rejected() {
    let mut plan = canonical_destructive();
    plan.actions = vec![
        FooterAction::new("help", FooterSlot::Utility, Emphasis::Quiet),
        FooterAction::new("delete", FooterSlot::Confirm, Emphasis::High),
    ];
    plan.initial_focus = Some("help");
    let violations = plan.validate();
    assert_eq!(
        violations,
        vec![FooterViolation::DestructiveWithoutCancel],
        "{violations:?}"
    );
}

#[test]
fn test_initial_focus_naming_a_control_that_is_not_in_the_footer_is_unknown() {
    let mut plan = canonical_destructive();
    plan.initial_focus = Some("ghost");
    let violations = plan.validate();
    assert_eq!(
        violations,
        vec![FooterViolation::InitialFocusUnknown { focused: "ghost" }],
        "{violations:?}"
    );
}

#[test]
fn test_a_plain_footer_may_focus_any_control_it_actually_has() {
    let mut plan = canonical_plain();
    plan.initial_focus = Some("reset");
    assert_eq!(plan.validate(), Vec::new(), "{plan:?}");
}

#[test]
fn test_a_plain_footer_with_an_unknown_focus_is_still_rejected() {
    let mut plan = canonical_plain();
    plan.initial_focus = Some("nowhere");
    assert_eq!(
        plan.validate(),
        vec![FooterViolation::InitialFocusUnknown { focused: "nowhere" }],
    );
}

#[test]
fn test_focused_action_resolves_the_declared_name() {
    let plan = canonical_destructive();
    let focused = plan.focused_action().expect("focus resolves");
    assert_eq!(focused.name, "cancel");
    assert_eq!(focused.slot, FooterSlot::Cancel);
}

#[test]
fn test_focused_action_is_none_when_nothing_is_declared() {
    let plan = canonical_plain();
    assert!(plan.focused_action().is_none());
}

#[test]
fn test_cancel_and_confirm_accessors_find_the_right_slots() {
    let plan = canonical_destructive();
    assert_eq!(
        plan.cancel_action().map(|a| a.name),
        Some("cancel"),
        "cancel accessor"
    );
    assert_eq!(
        plan.confirm_action().map(|a| a.name),
        Some("delete"),
        "confirm accessor"
    );
}

// ---------------------------------------------------------------------------
// Density wiring
// ---------------------------------------------------------------------------

#[test]
fn test_effective_height_defaults_to_the_plan_density() {
    let plan = canonical_destructive();
    for action in &plan.actions {
        assert_eq!(
            plan.effective_height(action),
            ControlDensity::Compact.height(),
            "{} declares no override",
            action.name
        );
    }
}

#[test]
fn test_effective_height_honours_an_explicit_override() {
    let action = FooterAction::at_height(
        "legacy",
        FooterSlot::Utility,
        Emphasis::Quiet,
        ControlDensity::Standard.height(),
    );
    let plan = canonical_destructive();
    assert_eq!(
        plan.effective_height(&action),
        ControlDensity::Standard.height(),
        "the override must win over the plan's rung"
    );
}

#[test]
fn test_a_mixed_density_footer_is_reported() {
    let mut plan = canonical_destructive();
    plan.actions[2] = FooterAction::at_height(
        "delete",
        FooterSlot::Confirm,
        Emphasis::High,
        ControlDensity::Standard.height(),
    );
    let report = plan.density_report();
    assert!(!report.is_homogeneous(), "{report}");
    assert_eq!(report.band, DensityBand::DialogFooter);
    assert_eq!(report.declared, ControlDensity::Compact);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].control, "delete");
}

#[test]
fn test_a_uniform_footer_density_report_is_clean() {
    let plan = canonical_destructive();
    let report = plan.density_report();
    assert!(report.is_homogeneous(), "{report}");
    assert_eq!(report.band, DensityBand::DialogFooter);
}

#[test]
fn test_an_off_ladder_footer_control_is_flagged_off_rung() {
    let mut plan = canonical_destructive();
    plan.actions[1] = FooterAction::at_height("cancel", FooterSlot::Cancel, Emphasis::Medium, 24.0);
    let report = plan.density_report();
    assert_eq!(report.off_rung_count(), 1);
    assert!(report.violations[0].is_off_rung());
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

#[test]
fn test_violations_render_readable_messages() {
    let cases = [
        FooterViolation::CancelIsHighEmphasis { control: "c" },
        FooterViolation::ConfirmIsNotHighEmphasis {
            control: "s",
            actual: "quiet",
        },
        FooterViolation::MultipleHighEmphasis {
            controls: vec!["a", "b"],
        },
        FooterViolation::OutOfOrder {
            earlier: "x",
            later: "y",
        },
        FooterViolation::DestructiveWithoutInitialFocus,
        FooterViolation::DestructiveWithoutCancel,
        FooterViolation::InitialFocusUnknown { focused: "g" },
        FooterViolation::InitialFocusNotCancel {
            focused: "d",
            expected: "c",
        },
    ];
    for case in cases {
        let text = case.to_string();
        assert!(
            text.len() > 20,
            "violation message is too thin to be a finding: {text}"
        );
    }
}
