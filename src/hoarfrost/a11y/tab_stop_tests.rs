//! Tests for the declared single-`Tab`-stop boundary and the
//! bounded-wraps / spatial-clamps rule (`UIF-PRIN-008`).
//!
//! Beyond the boundary type itself, this sidecar pins the *real*
//! implementations against the rule the pillar states, so the claim
//! "bounded groups wrap and spatial collections clamp" is checked
//! against the code that ships rather than against a comment.

use crate::a11y::tab_stop::{
    CompositeBoundary, CompositeCategory, EdgeBehaviour, TabStopViolation, check_composite,
    check_roster, total_members, total_tab_stops,
};
use crate::composite::table_view::TableFocus;
use crate::composite::table_view::TableView;
use crate::composite::tree_view::{SelectionMode, TreeView, VisibleRow};
use crate::widget::rove::{RoveDirection, rove_index};

fn bounded<'a>(id: &'a str, enabled: &'a [usize], active: Option<usize>) -> CompositeBoundary<'a> {
    CompositeBoundary::new(
        id,
        CompositeCategory::BoundedGroup,
        EdgeBehaviour::Wraps,
        enabled.len(),
        enabled,
        active,
    )
}

fn spatial<'a>(
    id: &'a str,
    members: usize,
    enabled: &'a [usize],
    active: Option<usize>,
) -> CompositeBoundary<'a> {
    CompositeBoundary::new(
        id,
        CompositeCategory::SpatialCollection,
        EdgeBehaviour::Clamps,
        members,
        enabled,
        active,
    )
}

// -- stop counting ------------------------------------------------------------

#[test]
fn a_composite_with_a_live_member_is_exactly_one_stop() {
    let composite = bounded("menu", &[0, 1, 2], Some(1));
    assert_eq!(composite.tab_stops(), 1);
    assert!(composite.is_single_stop());
}

#[test]
fn a_composite_with_every_member_disabled_contributes_no_stop() {
    let composite = bounded("dead-menu", &[], Some(0));
    assert_eq!(composite.tab_stops(), 0);
    assert!(!composite.is_single_stop());
}

#[test]
fn forty_members_still_make_one_stop() {
    let enabled: Vec<usize> = (0..40).collect();
    let composite = CompositeBoundary::new(
        "toolbar",
        CompositeCategory::BoundedGroup,
        EdgeBehaviour::Wraps,
        40,
        &enabled,
        Some(17),
    );
    assert_eq!(composite.tab_stops(), 1);
}

#[test]
fn the_roster_stop_count_is_the_composite_count_not_the_member_count() {
    let roster = vec![
        bounded("menu", &[0, 1, 2, 3], Some(2)),
        bounded("segmented", &[0, 1], None),
        spatial("tree", 100_000, &[0, 1, 2], Some(1)),
    ];
    assert_eq!(total_tab_stops(&roster), 3);
    assert_eq!(total_members(&roster), 100_006);
    assert_ne!(
        total_tab_stops(&roster),
        total_members(&roster),
        "a stop per member is exactly the bug this type exists to prevent"
    );
}

#[test]
fn an_all_disabled_composite_is_absent_from_the_stop_count() {
    // Three members, none of them live: the composite exists, but it is
    // not a stop the user can enter and find nothing inside.
    let dead: CompositeBoundary<'static> = CompositeBoundary::new(
        "dead",
        CompositeCategory::BoundedGroup,
        EdgeBehaviour::Wraps,
        3,
        &[],
        None,
    );
    let roster = vec![
        bounded("live", &[0, 1], Some(0)),
        dead,
        bounded("also-live", &[0], None),
    ];
    assert_eq!(total_tab_stops(&roster), 2);
    assert_eq!(total_members(&roster), 6);
}

#[test]
fn an_empty_roster_makes_no_stops() {
    let roster: Vec<CompositeBoundary<'static>> = Vec::new();
    assert_eq!(total_tab_stops(&roster), 0);
    assert_eq!(total_members(&roster), 0);
    assert!(check_roster(&roster).is_empty());
}

// -- entry member -------------------------------------------------------------

#[test]
fn entry_lands_on_the_active_member_not_the_first() {
    let composite = bounded("menu", &[0, 1, 2, 3], Some(2));
    assert_eq!(composite.entry_member(), Some(2));
    assert_ne!(composite.entry_member(), Some(0));
}

#[test]
fn entry_falls_back_to_the_first_enabled_member_when_nothing_is_active() {
    let composite = bounded("menu", &[3, 4, 5], None);
    assert_eq!(
        composite.entry_member(),
        Some(3),
        "must fall back to the first *enabled* member, not index 0"
    );
}

#[test]
fn entry_never_lands_on_a_disabled_member_even_when_named_active() {
    // A stale active index pointing at a disabled member must degrade to
    // the first live member rather than trap the keyboard on a control
    // it cannot use.
    let composite = bounded("menu", &[1, 2], Some(0));
    assert_eq!(composite.entry_member(), Some(1));
}

#[test]
fn entry_is_none_when_nothing_can_be_entered() {
    assert_eq!(bounded("dead", &[], None).entry_member(), None);
    assert_eq!(bounded("dead", &[], Some(3)).entry_member(), None);
}

// -- edge behaviour -----------------------------------------------------------

#[test]
fn each_category_demands_its_own_edge_rule() {
    assert_eq!(
        CompositeCategory::BoundedGroup.required_edge(),
        EdgeBehaviour::Wraps
    );
    assert_eq!(
        CompositeCategory::SpatialCollection.required_edge(),
        EdgeBehaviour::Clamps
    );
}

#[test]
fn a_bounded_group_that_clamps_is_reported() {
    let wrong = CompositeBoundary::new(
        "bad-menu",
        CompositeCategory::BoundedGroup,
        EdgeBehaviour::Clamps,
        3,
        &[0, 1, 2],
        Some(2),
    );
    let violations = check_composite(&wrong);
    assert_eq!(violations.len(), 1);
    assert_eq!(
        violations[0],
        TabStopViolation::WrongEdgeBehaviour {
            id: "bad-menu",
            category: CompositeCategory::BoundedGroup,
            declared: EdgeBehaviour::Clamps,
            required: EdgeBehaviour::Wraps,
        }
    );
    assert!(violations[0].to_string().contains("bad-menu"));
}

#[test]
fn a_spatial_collection_that_wraps_is_reported_as_the_worse_bug() {
    // Wrapping a 100,000-row table is the teleport §1.1 names outright.
    let wrong = CompositeBoundary::new(
        "bad-table",
        CompositeCategory::SpatialCollection,
        EdgeBehaviour::Wraps,
        100_000,
        &[0, 1, 2],
        Some(2),
    );
    let violations = check_composite(&wrong);
    assert_eq!(violations.len(), 1);
    assert!(matches!(
        violations[0],
        TabStopViolation::WrongEdgeBehaviour {
            category: CompositeCategory::SpatialCollection,
            declared: EdgeBehaviour::Wraps,
            ..
        }
    ));
}

#[test]
fn a_coherent_composite_reports_nothing() {
    assert!(check_composite(&bounded("menu", &[0, 1, 2], Some(1))).is_empty());
    assert!(check_composite(&spatial("tree", 10, &[0, 1, 2], Some(1))).is_empty());
}

#[test]
fn more_enabled_indices_than_members_is_incoherent() {
    let wrong = CompositeBoundary::new(
        "impossible",
        CompositeCategory::BoundedGroup,
        EdgeBehaviour::Wraps,
        2,
        &[0, 1, 2],
        None,
    );
    let violations = check_composite(&wrong);
    assert!(
        violations.contains(&TabStopViolation::EnabledExceedsMembers {
            id: "impossible",
            member_count: 2,
            enabled_count: 3,
        })
    );
}

#[test]
fn an_active_member_the_walk_cannot_reach_is_reported() {
    let wrong = CompositeBoundary::new(
        "stale",
        CompositeCategory::BoundedGroup,
        EdgeBehaviour::Wraps,
        4,
        &[0, 1],
        Some(3),
    );
    let violations = check_composite(&wrong);
    assert!(
        violations.contains(&TabStopViolation::ActiveMemberNotEnabled {
            id: "stale",
            active: 3,
        })
    );
}

#[test]
fn a_roster_reports_every_finding_not_just_the_first() {
    let roster = vec![
        bounded("ok", &[0, 1], Some(0)),
        CompositeBoundary::new(
            "bad-edge",
            CompositeCategory::BoundedGroup,
            EdgeBehaviour::Clamps,
            2,
            &[0, 1],
            None,
        ),
        CompositeBoundary::new(
            "bad-active",
            CompositeCategory::BoundedGroup,
            EdgeBehaviour::Wraps,
            2,
            &[0],
            Some(1),
        ),
    ];
    let violations = check_roster(&roster);
    assert_eq!(violations.len(), 2, "both broken composites must surface");
    assert!(check_composite(&roster[0]).is_empty());
}

// -- internal navigation ------------------------------------------------------

#[test]
fn a_wrapping_composite_cycles_from_last_to_first() {
    let composite = bounded("menu", &[0, 1, 2], Some(2));
    assert_eq!(composite.next_within(RoveDirection::Forward), Some(0));
    let at_first = bounded("menu", &[0, 1, 2], Some(0));
    assert_eq!(at_first.next_within(RoveDirection::Backward), Some(2));
}

#[test]
fn a_clamping_composite_stops_at_both_ends() {
    let at_last = spatial("tree", 3, &[0, 1, 2], Some(2));
    assert_eq!(at_last.next_within(RoveDirection::Forward), Some(2));
    let at_first = spatial("tree", 3, &[0, 1, 2], Some(0));
    assert_eq!(at_first.next_within(RoveDirection::Backward), Some(0));
}

#[test]
fn a_clamping_composite_still_moves_through_the_middle() {
    let composite = spatial("tree", 4, &[0, 1, 2, 3], Some(1));
    assert_eq!(composite.next_within(RoveDirection::Forward), Some(2));
    assert_eq!(composite.next_within(RoveDirection::Backward), Some(0));
}

#[test]
fn a_clamping_composite_with_no_active_enters_at_the_near_end() {
    let composite = spatial("tree", 3, &[0, 1, 2], None);
    assert_eq!(composite.next_within(RoveDirection::Forward), Some(0));
    assert_eq!(composite.next_within(RoveDirection::Backward), Some(2));
}

#[test]
fn a_clamping_composite_skips_disabled_members() {
    // 1 is disabled; the walk goes 0 → 2 in one step.
    let composite = spatial("tree", 3, &[0, 2], Some(0));
    assert_eq!(composite.next_within(RoveDirection::Forward), Some(2));
    assert_eq!(composite.next_within(RoveDirection::Forward), Some(2));
}

#[test]
fn no_boundary_moves_when_nothing_is_enabled() {
    let composite = bounded("dead", &[], Some(0));
    assert_eq!(composite.next_within(RoveDirection::Forward), None);
    assert_eq!(composite.next_within(RoveDirection::Backward), None);
}

#[test]
fn a_wrapping_boundary_agrees_with_rove_index_exactly() {
    // The declared boundary must not reimplement the walk differently
    // from the shared function it delegates to.
    let enabled = [0usize, 2, 5, 7];
    for active in [None, Some(0), Some(2), Some(5), Some(7), Some(6)] {
        for direction in [RoveDirection::Forward, RoveDirection::Backward] {
            let composite = bounded("menu", &enabled, active);
            assert_eq!(
                composite.next_within(direction),
                rove_index(&enabled, active, direction),
                "boundary and rove_index disagree for active={active:?} {direction:?}"
            );
        }
    }
}

// -- the real implementations obey the rule -----------------------------------

#[test]
fn rove_index_wraps_which_is_what_a_bounded_group_requires() {
    let enabled = [0usize, 1, 2];
    assert_eq!(
        rove_index(&enabled, Some(2), RoveDirection::Forward),
        Some(0)
    );
    assert_eq!(
        rove_index(&enabled, Some(0), RoveDirection::Backward),
        Some(2)
    );
}

#[test]
fn the_tree_view_clamps_rather_than_teleporting_the_viewport() {
    let rows: Vec<VisibleRow<usize>> = (0..1_000)
        .map(|index| VisibleRow {
            id: index,
            depth: 0,
            expandable: false,
            expanded: false,
        })
        .collect();
    let mut tree = TreeView::<usize>::new(SelectionMode::Single);
    tree.focus_index(&rows, 999).expect("focus the last row");
    for _ in 0..5 {
        assert_eq!(
            tree.move_focus(&rows, 1),
            Some(999),
            "holding Down at the bottom must never wrap to row 0"
        );
    }
    tree.focus_index(&rows, 0).expect("focus the first row");
    for _ in 0..5 {
        assert_eq!(tree.move_focus(&rows, -1), Some(0));
    }
}

#[test]
fn the_table_view_clamps_in_both_axes() {
    let rows: Vec<usize> = (0..500).collect();
    let mut table = TableView::<usize>::new(SelectionMode::Single);
    table.move_row(&rows, 1).expect("enter the table");
    for _ in 0..600 {
        table.move_row(&rows, 1);
    }
    let focused = table.focus().and_then(TableFocus::row).copied();
    assert_eq!(focused, Some(499), "the row walk must stop at the last row");

    table.move_row(&rows, -1);
    for _ in 0..600 {
        table.move_row(&rows, -1);
    }
    assert_eq!(
        table.focus().and_then(TableFocus::row).copied(),
        Some(0),
        "the row walk must stop at the first row"
    );
}

#[test]
fn the_clamp_and_wrap_split_is_consistent_with_the_declared_categories() {
    // The tree and the table are spatial collections; the shared rove
    // walk is what bounded groups use. Assert the two behaviours are
    // actually different, otherwise the split is vacuous.
    let enabled = [0usize, 1, 2];
    let wrapping = bounded("menu", &enabled, Some(2)).next_within(RoveDirection::Forward);
    let clamping = spatial("tree", 3, &enabled, Some(2)).next_within(RoveDirection::Forward);
    assert_eq!(wrapping, Some(0));
    assert_eq!(clamping, Some(2));
    assert_ne!(wrapping, clamping);
}

// -- determinism --------------------------------------------------------------

#[test]
fn the_same_roster_produces_the_same_findings_in_the_same_order() {
    let build = || {
        vec![
            bounded("a", &[0, 1], Some(0)),
            CompositeBoundary::new(
                "b",
                CompositeCategory::SpatialCollection,
                EdgeBehaviour::Wraps,
                2,
                &[0, 1],
                None,
            ),
            bounded("c", &[0], Some(9)),
        ]
    };
    assert_eq!(check_roster(&build()), check_roster(&build()));
}

#[test]
fn slugs_are_stable_and_distinct() {
    for category in CompositeCategory::ALL {
        for edge in EdgeBehaviour::ALL {
            assert!(!category.slug().is_empty());
            assert!(!edge.slug().is_empty());
        }
    }
    assert_ne!(
        CompositeCategory::BoundedGroup.slug(),
        CompositeCategory::SpatialCollection.slug()
    );
    assert_ne!(EdgeBehaviour::Wraps.slug(), EdgeBehaviour::Clamps.slug());
}

#[test]
fn test_all_eleven_collections_in_the_pillar_have_boundaries() {
    // §1.1 UIF-PRIN-008 names 11 collections:
    // Bounded groups (wraps):
    // 1. Menu
    // 2. RadioGroup
    // 3. SegmentedButton
    // 4. NavigationRail
    // 5. CommandPalette
    // 6. TabStrip
    // 7. Stepper
    // Spatial collections (clamps):
    // 8. TreeView
    // 9. TableView
    // 10. GridView
    // 11. CalendarView
    let enabled_three = [0, 1, 2];
    let roster = vec![
        bounded("Menu", &enabled_three, Some(0)),
        bounded("RadioGroup", &enabled_three, Some(0)),
        bounded("SegmentedButton", &enabled_three, Some(0)),
        bounded("NavigationRail", &enabled_three, Some(0)),
        bounded("CommandPalette", &enabled_three, Some(0)),
        bounded("TabStrip", &enabled_three, Some(0)),
        bounded("Stepper", &enabled_three, Some(0)),
        spatial("TreeView", 100, &enabled_three, Some(0)),
        spatial("TableView", 100, &enabled_three, Some(0)),
        spatial("GridView", 100, &enabled_three, Some(0)),
        spatial("CalendarView", 42, &enabled_three, Some(0)),
    ];

    assert_eq!(roster.len(), 11, "exactly 11 collections named in §1.1");
    assert_eq!(
        total_tab_stops(&roster),
        11,
        "each collection constitutes exactly one tab stop"
    );
    assert!(
        check_roster(&roster).is_empty(),
        "no edge rule violations across all 11 collections"
    );
}
