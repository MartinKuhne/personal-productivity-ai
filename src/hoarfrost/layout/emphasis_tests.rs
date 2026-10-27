//! Tests for [`super::emphasis`] — the single high-emphasis focal point
//! rule (`UIF-PRIN-006`).

use super::*;
use crate::surface::dialog_button::DialogButtonRole;
use crate::widget::button::ButtonVariant;

// ---------------------------------------------------------------------------
// Variant classification
// ---------------------------------------------------------------------------

#[test]
fn test_primary_and_danger_are_both_high_emphasis() {
    assert_eq!(Emphasis::from(ButtonVariant::Primary), Emphasis::High);
    assert_eq!(Emphasis::from(ButtonVariant::Danger), Emphasis::High);
}

#[test]
fn test_secondary_steps_down_to_medium_and_ghost_to_quiet() {
    assert_eq!(Emphasis::from(ButtonVariant::Secondary), Emphasis::Medium);
    assert_eq!(Emphasis::from(ButtonVariant::Ghost), Emphasis::Quiet);
}

#[test]
fn test_only_two_variants_count_against_the_budget() {
    let high: Vec<ButtonVariant> = [
        ButtonVariant::Primary,
        ButtonVariant::Secondary,
        ButtonVariant::Ghost,
        ButtonVariant::Danger,
    ]
    .into_iter()
    .filter(|variant| Emphasis::from(*variant).is_high())
    .collect();
    assert_eq!(high, vec![ButtonVariant::Primary, ButtonVariant::Danger]);
}

#[test]
fn test_dialog_role_primary_is_high_and_secondary_is_medium() {
    assert_eq!(Emphasis::from(DialogButtonRole::Primary), Emphasis::High);
    assert_eq!(
        Emphasis::from(DialogButtonRole::Secondary),
        Emphasis::Medium,
        "the discarding control must step down out of the focal-point budget"
    );
}

#[test]
fn test_slugs_are_unique_per_level() {
    let slugs: Vec<&str> = Emphasis::ALL.iter().map(|e| e.slug()).collect();
    let unique: std::collections::BTreeSet<&str> = slugs.iter().copied().collect();
    assert_eq!(
        slugs.len(),
        unique.len(),
        "emphasis slugs collide: {slugs:?}"
    );
}

#[test]
fn test_container_scopes_are_the_four_the_spec_names() {
    let slugs: Vec<&str> = ContainerScope::ALL.iter().map(|s| s.slug()).collect();
    for expected in ["modal-footer", "toolbar-cluster", "card", "form"] {
        assert!(
            slugs.contains(&expected),
            "§1.1 names a container scope this check cannot report: {expected} (have {slugs:?})"
        );
    }
    assert_eq!(ContainerScope::ALL.len(), 4);
}

// ---------------------------------------------------------------------------
// The count
// ---------------------------------------------------------------------------

#[test]
fn test_a_single_high_control_is_compliant() {
    let slots = [
        EmphasisSlot::new("cancel", Emphasis::Medium),
        EmphasisSlot::high("save"),
        EmphasisSlot::new("help", Emphasis::Quiet),
    ];
    let report = check_focal_point(ContainerScope::ModalFooter, &slots);
    assert!(report.is_single_focal_point(), "{report}");
    assert_eq!(report.excess(), 0);
    assert_eq!(report.total, 3);
    assert_eq!(report.high_names(), vec!["save"]);
}

#[test]
fn test_two_primaries_in_one_container_fail() {
    let slots = [
        EmphasisSlot::of_button_variant("save", ButtonVariant::Primary),
        EmphasisSlot::of_button_variant("apply", ButtonVariant::Primary),
    ];
    let report = check_focal_point(ContainerScope::ModalFooter, &slots);
    assert!(!report.is_single_focal_point());
    assert_eq!(report.excess(), 1);
    assert_eq!(report.high_names(), vec!["save", "apply"]);
}

#[test]
fn test_primary_plus_danger_is_two_focal_points_not_one_each() {
    // The clause is "one focal point", not "one of each colour". A
    // confirm-and-delete pair has two things shouting even though neither
    // variant repeats.
    let slots = [
        EmphasisSlot::of_button_variant("confirm", ButtonVariant::Primary),
        EmphasisSlot::of_button_variant("delete", ButtonVariant::Danger),
    ];
    let report = check_focal_point(ContainerScope::ModalFooter, &slots);
    assert!(
        !report.is_single_focal_point(),
        "Primary + Danger must not read as compliant: {report}"
    );
    assert_eq!(report.high.len(), 2);
    assert_eq!(report.excess(), 1);
}

#[test]
fn test_three_high_controls_report_every_offender() {
    let slots = [
        EmphasisSlot::high("a"),
        EmphasisSlot::new("quiet", Emphasis::Quiet),
        EmphasisSlot::high("b"),
        EmphasisSlot::high("c"),
    ];
    let report = check_focal_point(ContainerScope::ToolbarCluster, &slots);
    assert_eq!(report.high_names(), vec!["a", "b", "c"]);
    assert_eq!(report.excess(), 2);
}

#[test]
fn test_no_high_control_is_compliant() {
    let slots = [
        EmphasisSlot::new("a", Emphasis::Medium),
        EmphasisSlot::new("b", Emphasis::Quiet),
    ];
    let report = check_focal_point(ContainerScope::Card, &slots);
    assert!(report.is_single_focal_point(), "{report}");
    assert!(report.high.is_empty());
}

#[test]
fn test_empty_container_is_compliant() {
    let report = check_focal_point(ContainerScope::Form, &[]);
    assert!(report.is_single_focal_point());
    assert_eq!(report.total, 0);
}

#[test]
fn test_two_dialog_primary_roles_in_one_footer_fail() {
    // The exact shape the ledger called out as passing every existing
    // test: a host building two `Primary` dialog controllers.
    let slots = [
        EmphasisSlot::of_dialog_role("ok", DialogButtonRole::Primary),
        EmphasisSlot::of_dialog_role("also_ok", DialogButtonRole::Primary),
    ];
    let report = check_focal_point(ContainerScope::ModalFooter, &slots);
    assert!(
        !report.is_single_focal_point(),
        "two dispatching footer controls must not both be the focal point"
    );
    assert_eq!(report.high_names(), vec!["ok", "also_ok"]);
}

#[test]
fn test_a_secondary_role_control_painted_loudly_is_counted_via_the_escape_hatch() {
    // The role type cannot see paint, so a host that fills a
    // secondary-role control with the accent must declare it high here
    // for the rule to catch it.
    let slots = [
        EmphasisSlot::of_dialog_role("cancel", DialogButtonRole::Secondary),
        EmphasisSlot::high("custom_cta"),
        EmphasisSlot::high("another_custom_cta"),
    ];
    let report = check_focal_point(ContainerScope::Card, &slots);
    assert_eq!(report.excess(), 1);
    assert!(!report.high_names().contains(&"cancel"));
}

#[test]
fn test_report_display_names_the_scope_and_every_offender() {
    let slots = [EmphasisSlot::high("x"), EmphasisSlot::high("y")];
    let report = check_focal_point(ContainerScope::ToolbarCluster, &slots);
    let text = report.to_string();
    assert!(text.contains("toolbar-cluster"), "{text}");
    assert!(text.contains('x'), "{text}");
    assert!(text.contains('y'), "{text}");
    assert!(text.contains("at most 1"), "{text}");
}

#[test]
fn test_compliant_report_display_says_single_focal_point() {
    let slots = [EmphasisSlot::high("only")];
    let report = check_focal_point(ContainerScope::Form, &slots);
    assert!(
        report.to_string().contains("single focal point"),
        "{report}"
    );
}
