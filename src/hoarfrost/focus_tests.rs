//! Tests for [`FocusState`] — the keyboard focus model behind
//! `UIF-PRIN-005`, `UIF-MOD-001` and `UIF-CMENU-003`.
//!
//! Seven groups, in the order the model is built up:
//!
//! 1. **Identity** — `FocusId` / `GroupId` round-trip, compare by content,
//!    and the `FocusMember` constructors.
//! 2. **Registration** — ordering, replacement in place, removal, lookups.
//! 3. **Direct focus** — `focus` / `clear`, and the rejections.
//! 4. **Traversal** — forward and backward across groups, wrap at both
//!    ends, disabled skipping, and the empty-scope floor.
//! 5. **Roving focus** — `move_within`, the `Up`/`Down` primitive of
//!    `UIF-CMENU-003`, including the documented wrap-not-clamp choice.
//! 6. **Traps** — confinement, nesting, and restore through the stack.
//! 7. **Structural change under a live focus**, plus **determinism**.
//!
//! # The scenarios the tests lean on
//!
//! Most traversal tests use [`standard`], whose enabled order is
//! `a, c, d, e, g` across three groups with a disabled member planted in
//! two of them. Roving uses [`roving_scenery`] (enabled `m1, m3, m4`) and
//! the trap tests use [`trap_scenery`] (a base group plus two dialog
//! groups). Keeping the fixtures in one place means a change to the model
//! shows up as a failure in the scenario doc rather than in forty
//! hand-built states.
//!
//! # What the tests pin that a live run would not show
//!
//! The four decisions called out in the module docs each get their own
//! tests, because all four are places where a plausible implementation
//! could silently differ: where focus goes when the focused member is
//! unregistered (slot-preserving), whether roving wraps or clamps
//! (wraps), what happens when the focused member is disabled (moves
//! forward), and whether focus can be placed outside an active trap
//! (rejected).

use super::*;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Shorthand for a member id.
fn id(name: &str) -> FocusId {
    FocusId::new(name)
}

/// Shorthand for a group id.
fn gid(name: &str) -> GroupId {
    GroupId::new(name)
}

/// Build a roster from `(name, enabled)` pairs.
fn roster(rows: &[(&str, bool)]) -> Vec<FocusMember> {
    rows.iter()
        .map(|(name, enabled)| FocusMember {
            id: id(name),
            enabled: *enabled,
        })
        .collect()
}

/// Build a model from `(group, roster)` pairs, registered in the order
/// given.
fn model(spec: &[(&str, &[(&str, bool)])]) -> FocusState {
    let mut state = FocusState::new();
    for (name, rows) in spec {
        state.register_group(gid(name), roster(rows));
    }
    state
}

/// The focused id as a bare string, so assertions read as `Some("c")`.
fn cur(state: &FocusState) -> Option<&str> {
    state.current().map(FocusId::as_str)
}

/// Borrowed ids as bare strings.
fn names<'a>(list: &'a [&'a FocusId]) -> Vec<&'a str> {
    list.iter().map(|item| item.as_str()).collect()
}

/// Three groups, two of them carrying a disabled member.
///
/// Enabled traversal order: `a, c, d, e, g`.
fn standard() -> FocusState {
    model(&[
        ("nav", &[("a", true), ("b", false), ("c", true)]),
        ("body", &[("d", true), ("e", true)]),
        ("foot", &[("f", false), ("g", true)]),
    ])
}

/// One menu-shaped group: `m2` and `m5` are disabled, standing in for the
/// headings and separators a menu registers as inert.
///
/// Enabled order: `m1, m3, m4`.
fn roving_scenery() -> FocusState {
    model(&[(
        "menu",
        &[
            ("m1", true),
            ("m2", false),
            ("m3", true),
            ("m4", true),
            ("m5", false),
        ],
    )])
}

/// A base workspace and two dialogs, all fully enabled, for the trap tests.
fn trap_scenery() -> FocusState {
    model(&[
        ("base", &[("x", true), ("y", true)]),
        ("dlgA", &[("a1", true), ("a2", true)]),
        ("dlgB", &[("b1", true), ("b2", true)]),
    ])
}

// ---------------------------------------------------------------------------
// 1. Identity
// ---------------------------------------------------------------------------

#[test]
fn test_focus_id_round_trips_its_inner_string() {
    let focus = id("menu:file:save");
    assert_eq!(focus.as_str(), "menu:file:save");
}

#[test]
fn test_focus_id_equality_is_by_content() {
    assert_eq!(id("row-1"), id("row-1"));
    assert_ne!(id("row-1"), id("row-2"));
}

#[test]
fn test_focus_id_hash_is_by_content_so_it_can_key_a_map() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(id("dialog:save"));
    set.insert(id("dialog:save"));
    assert_eq!(set.len(), 1);

    set.insert(id("dialog:cancel"));
    assert_eq!(set.len(), 2);
    assert!(set.contains(&id("dialog:save")));
}

#[test]
fn test_focus_id_display_prints_the_inner_string() {
    assert_eq!(format!("{}", id("tab:3")), "tab:3");
}

#[test]
fn test_group_id_round_trips_and_displays() {
    let group = gid("workspace");
    assert_eq!(group.as_str(), "workspace");
    assert_eq!(format!("{group}"), "workspace");
    assert_eq!(gid("workspace"), gid("workspace"));
    assert_ne!(gid("workspace"), gid("dialog"));
}

#[test]
fn test_member_constructors_set_the_enabled_flag_only() {
    let live = FocusMember::new("a");
    assert!(live.enabled);
    assert_eq!(live.id, id("a"));

    let dead = FocusMember::disabled("b");
    assert!(!dead.enabled);
    assert_eq!(dead.id, id("b"));
}

// ---------------------------------------------------------------------------
// 2. Registration
// ---------------------------------------------------------------------------

#[test]
fn test_register_group_appends_in_registration_order() {
    let state = standard();
    let order: Vec<&str> = state.groups().iter().map(|g| g.id().as_str()).collect();
    assert_eq!(order, vec!["nav", "body", "foot"]);
}

#[test]
fn test_reregistering_replaces_members_without_moving_the_group() {
    let mut state = standard();
    state.register_group(gid("nav"), roster(&[("z", true)]));

    let order: Vec<&str> = state.groups().iter().map(|g| g.id().as_str()).collect();
    assert_eq!(order, vec!["nav", "body", "foot"]);
    assert_eq!(state.focusable_members(&gid("nav")), vec![&id("z")]);
}

#[test]
fn test_reregistering_replaces_rather_than_extends() {
    let mut state = model(&[("g", &[("a", true), ("b", true)])]);
    state.register_group(gid("g"), roster(&[("c", true)]));

    let members: Vec<&str> = state
        .group(&gid("g"))
        .expect("group present")
        .members()
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(members, vec!["c"]);
}

#[test]
fn test_unregister_group_removes_it() {
    let mut state = standard();
    assert_eq!(state.unregister_group(&gid("body")), None);

    let order: Vec<&str> = state.groups().iter().map(|g| g.id().as_str()).collect();
    assert_eq!(order, vec!["nav", "foot"]);
    assert!(state.group(&gid("body")).is_none());
}

#[test]
fn test_unregistering_an_unknown_group_changes_nothing() {
    let mut state = standard();
    let before = state.clone();
    assert_eq!(state.unregister_group(&gid("ghost")), None);
    assert_eq!(state, before);
}

#[test]
fn test_group_and_member_lookups() {
    let state = standard();
    assert_eq!(state.group(&gid("nav")).map(|g| g.members().len()), Some(3));
    assert_eq!(
        state.group_of(&id("d")).map(|g| g.id().as_str()),
        Some("body")
    );
    assert_eq!(state.group_of(&id("ghost")), None);
}

#[test]
fn test_focusable_members_skip_disabled_and_keep_order() {
    let state = standard();
    assert_eq!(names(&state.focusable_members(&gid("nav"))), vec!["a", "c"]);
    assert_eq!(
        names(&state.focusable_members(&gid("body"))),
        vec!["d", "e"]
    );
    assert_eq!(names(&state.focusable_members(&gid("foot"))), vec!["g"]);
    assert!(state.focusable_members(&gid("ghost")).is_empty());
}

#[test]
fn test_is_focusable_ignores_traps_and_reports_registration() {
    let mut state = trap_scenery();
    state.push_trap(gid("dlgA"));

    assert!(state.is_focusable(&id("x")));
    assert!(state.is_focusable(&id("a1")));
    assert!(!state.is_focusable(&id("ghost")));

    state.set_member_enabled(&gid("base"), &id("y"), false);
    assert!(!state.is_focusable(&id("y")));
}

#[test]
fn test_in_active_scope_reports_the_trap_boundary() {
    let mut state = trap_scenery();
    assert!(state.in_active_scope(&id("x")));
    assert!(state.in_active_scope(&id("b1")));

    state.push_trap(gid("dlgA"));
    assert!(state.in_active_scope(&id("a1")));
    assert!(!state.in_active_scope(&id("x")));
    assert!(!state.in_active_scope(&id("b1")));
    assert!(!state.in_active_scope(&id("ghost")));
}

// ---------------------------------------------------------------------------
// 3. Direct focus
// ---------------------------------------------------------------------------

#[test]
fn test_focus_sets_current_and_reports_success() {
    let mut state = standard();
    assert!(state.focus(&id("c")));
    assert_eq!(cur(&state), Some("c"));
    assert!(state.is_focused(&id("c")));
    assert!(!state.is_focused(&id("a")));
}

#[test]
fn test_focus_replaces_the_previous_focus() {
    let mut state = standard();
    state.focus(&id("a"));
    assert!(state.focus(&id("e")));
    assert_eq!(cur(&state), Some("e"));
}

#[test]
fn test_focusing_an_unregistered_id_is_rejected() {
    let mut state = standard();
    let before = state.clone();
    assert!(!state.focus(&id("ghost")));
    assert_eq!(state, before);
}

#[test]
fn test_focusing_a_disabled_member_is_rejected() {
    let mut state = standard();
    let before = state.clone();
    assert!(!state.focus(&id("b")));
    assert!(!state.focus(&id("f")));
    assert_eq!(state, before);
}

#[test]
fn test_focusing_what_is_already_focused_is_a_no_op_that_succeeds() {
    let mut state = standard();
    state.focus(&id("d"));
    let before = state.clone();
    assert!(state.focus(&id("d")));
    assert_eq!(state, before);
}

#[test]
fn test_clear_returns_what_it_cleared_and_then_stays_empty() {
    let mut state = standard();
    state.focus(&id("c"));
    assert_eq!(state.clear(), Some(id("c")));
    assert_eq!(cur(&state), None);
    assert_eq!(state.clear(), None);
}

// ---------------------------------------------------------------------------
// 4. Traversal
// ---------------------------------------------------------------------------

#[test]
fn test_forward_traversal_walks_every_group_in_registration_order() {
    let mut state = standard();
    assert_eq!(state.focus_next(), Some(id("a")));
    assert_eq!(state.focus_next(), Some(id("c")));
    assert_eq!(state.focus_next(), Some(id("d")));
    assert_eq!(state.focus_next(), Some(id("e")));
    assert_eq!(state.focus_next(), Some(id("g")));
}

#[test]
fn test_forward_traversal_wraps_at_the_end() {
    let mut state = standard();
    state.focus(&id("g"));
    assert_eq!(state.focus_next(), Some(id("a")));
    assert_eq!(cur(&state), Some("a"));
}

#[test]
fn test_backward_traversal_walks_groups_in_reverse() {
    let mut state = standard();
    assert_eq!(state.focus_previous(), Some(id("g")));
    assert_eq!(state.focus_previous(), Some(id("e")));
    assert_eq!(state.focus_previous(), Some(id("d")));
    assert_eq!(state.focus_previous(), Some(id("c")));
    assert_eq!(state.focus_previous(), Some(id("a")));
}

#[test]
fn test_backward_traversal_wraps_at_the_start() {
    let mut state = standard();
    state.focus(&id("a"));
    assert_eq!(state.focus_previous(), Some(id("g")));
}

#[test]
fn test_forward_from_no_focus_lands_on_the_first_member() {
    let mut state = standard();
    assert_eq!(state.focus_next(), Some(id("a")));
}

#[test]
fn test_backward_from_no_focus_lands_on_the_last_member() {
    let mut state = standard();
    assert_eq!(state.focus_previous(), Some(id("g")));
}

#[test]
fn test_disabled_members_are_skipped_forward() {
    let mut state = standard();
    state.focus(&id("a"));
    // `b` sits between `a` and `c` and must not be stoppable.
    assert_eq!(state.focus_next(), Some(id("c")));
    state.focus(&id("e"));
    // `f` sits between `e` and `g`.
    assert_eq!(state.focus_next(), Some(id("g")));
}

#[test]
fn test_disabled_members_are_skipped_backward() {
    let mut state = standard();
    state.focus(&id("c"));
    assert_eq!(state.focus_previous(), Some(id("a")));
    state.focus(&id("g"));
    assert_eq!(state.focus_previous(), Some(id("e")));
}

#[test]
fn test_an_all_disabled_group_is_skipped_entirely() {
    let mut state = model(&[
        ("dead", &[("d1", false), ("d2", false)]),
        ("live", &[("l1", true)]),
    ]);
    assert_eq!(state.focus_next(), Some(id("l1")));
    assert_eq!(state.focus_previous(), Some(id("l1")));

    let mut state = model(&[
        ("live", &[("l1", true)]),
        ("dead", &[("d1", false), ("d2", false)]),
    ]);
    state.focus(&id("l1"));
    assert_eq!(state.focus_next(), Some(id("l1")));
    assert_eq!(state.focus_previous(), Some(id("l1")));
}

#[test]
fn test_nothing_focusable_returns_none_and_leaves_the_state_untouched() {
    let mut state = model(&[
        ("g1", &[("a", false), ("b", false)]),
        ("g2", &[("c", false)]),
    ]);
    let before = state.clone();

    assert_eq!(state.focus_next(), None);
    assert_eq!(state.focus_previous(), None);
    assert_eq!(state.focus_first_in_group(&gid("g1")), None);
    assert_eq!(state.focus_last_in_group(&gid("g1")), None);
    assert_eq!(state.move_within(&gid("g2"), 1), None);
    assert_eq!(state, before);
}

#[test]
fn test_a_full_cycle_visits_each_enabled_member_once_and_returns_home() {
    let mut state = standard();
    state.focus(&id("a"));

    let mut visited = Vec::new();
    for _ in 0..5 {
        visited.push(state.focus_next().expect("focusable"));
    }
    assert_eq!(
        visited.iter().map(|i| i.as_str()).collect::<Vec<_>>(),
        vec!["c", "d", "e", "g", "a"]
    );
    assert!(state.is_focused(&id("a")));
}

#[test]
fn test_traversal_only_counts_enabled_members() {
    let state = standard();
    let mut counted = 0;
    for group in state.groups() {
        counted += group.enabled_members().count();
    }
    assert_eq!(counted, 5);
    assert_eq!(
        state
            .groups()
            .iter()
            .map(|g| g.members().len())
            .sum::<usize>(),
        7
    );
}

// ---------------------------------------------------------------------------
// Group edges
// ---------------------------------------------------------------------------

#[test]
fn test_focus_first_in_group_skips_leading_disabled_members() {
    let mut state = model(&[("h", &[("h1", false), ("h2", true), ("h3", true)])]);
    assert_eq!(state.focus_first_in_group(&gid("h")), Some(id("h2")));
}

#[test]
fn test_focus_last_in_group_skips_trailing_disabled_members() {
    let mut state = model(&[("i", &[("i1", true), ("i2", false), ("i3", false)])]);
    assert_eq!(state.focus_last_in_group(&gid("i")), Some(id("i1")));
}

#[test]
fn test_group_edges_on_an_unknown_group_are_rejected() {
    let mut state = standard();
    let before = state.clone();
    assert_eq!(state.focus_first_in_group(&gid("ghost")), None);
    assert_eq!(state.focus_last_in_group(&gid("ghost")), None);
    assert_eq!(state, before);
}

#[test]
fn test_group_edges_on_an_all_disabled_group_leave_focus_alone() {
    let mut state = model(&[("live", &[("l1", true)]), ("dead", &[("d1", false)])]);
    state.focus(&id("l1"));
    let before = state.clone();

    assert_eq!(state.focus_first_in_group(&gid("dead")), None);
    assert_eq!(state.focus_last_in_group(&gid("dead")), None);
    assert_eq!(state, before);
}

// ---------------------------------------------------------------------------
// 5. Roving focus (`UIF-CMENU-003` Up/Down)
// ---------------------------------------------------------------------------

#[test]
fn test_roving_forward_steps_to_the_next_enabled_member() {
    let mut state = roving_scenery();
    state.focus(&id("m1"));
    assert_eq!(state.move_within(&gid("menu"), 1), Some(id("m3")));
    assert_eq!(state.move_within(&gid("menu"), 1), Some(id("m4")));
}

#[test]
fn test_roving_backward_steps_to_the_previous_enabled_member() {
    let mut state = roving_scenery();
    state.focus(&id("m4"));
    assert_eq!(state.move_within(&gid("menu"), -1), Some(id("m3")));
    assert_eq!(state.move_within(&gid("menu"), -1), Some(id("m1")));
}

#[test]
fn test_roving_wraps_forward_past_the_last_member() {
    let mut state = roving_scenery();
    state.focus(&id("m4"));
    assert_eq!(state.move_within(&gid("menu"), 1), Some(id("m1")));
}

#[test]
fn test_roving_wraps_backward_past_the_first_member() {
    let mut state = roving_scenery();
    state.focus(&id("m1"));
    assert_eq!(state.move_within(&gid("menu"), -1), Some(id("m4")));
}

#[test]
fn test_roving_multi_step_wraps_by_modulo_in_both_directions() {
    let mut state = roving_scenery();
    state.focus(&id("m1"));
    // Three enabled members: +4 lands one past the start, -4 lands two back.
    assert_eq!(state.move_within(&gid("menu"), 4), Some(id("m3")));
    state.focus(&id("m1"));
    assert_eq!(state.move_within(&gid("menu"), -4), Some(id("m4")));
}

#[test]
fn test_roving_a_full_lap_returns_to_the_same_member() {
    let mut state = roving_scenery();
    state.focus(&id("m3"));
    assert_eq!(state.move_within(&gid("menu"), 3), Some(id("m3")));
    assert_eq!(state.move_within(&gid("menu"), -3), Some(id("m3")));
}

#[test]
fn test_roving_with_zero_delta_never_moves() {
    let mut state = roving_scenery();
    state.focus(&id("m3"));
    let before = state.clone();
    assert_eq!(state.move_within(&gid("menu"), 0), Some(id("m3")));
    assert_eq!(state, before);

    // With nothing focused, zero still refuses to invent a focus.
    let mut empty = roving_scenery();
    let empty_before = empty.clone();
    assert_eq!(empty.move_within(&gid("menu"), 0), None);
    assert_eq!(empty, empty_before);
}

#[test]
fn test_roving_from_outside_the_group_enters_at_the_edge() {
    let mut state = model(&[
        ("other", &[("o1", true)]),
        (
            "menu",
            &[("m1", true), ("m2", false), ("m3", true), ("m4", true)],
        ),
    ]);
    state.focus(&id("o1"));
    assert_eq!(state.move_within(&gid("menu"), 1), Some(id("m1")));

    state.focus(&id("o1"));
    assert_eq!(state.move_within(&gid("menu"), -1), Some(id("m4")));
}

#[test]
fn test_roving_never_leaves_its_own_group() {
    let mut state = model(&[
        ("menu", &[("m1", true), ("m2", false), ("m3", true)]),
        ("other", &[("o1", true), ("o2", true)]),
    ]);
    state.focus(&id("m1"));
    for _ in 0..6 {
        state.move_within(&gid("menu"), 1);
        let focused = cur(&state).expect("focus is held");
        assert_eq!(
            state
                .group_of(&id(focused))
                .map(|group| group.id().as_str()),
            Some("menu")
        );
    }
}

#[test]
fn test_roving_an_unknown_group_is_rejected() {
    let mut state = roving_scenery();
    state.focus(&id("m1"));
    let before = state.clone();
    assert_eq!(state.move_within(&gid("ghost"), 1), None);
    assert_eq!(state, before);
}

#[test]
fn test_roving_an_all_disabled_group_is_rejected() {
    let mut state = model(&[("dead", &[("d1", false), ("d2", false)])]);
    let before = state.clone();
    assert_eq!(state.move_within(&gid("dead"), 1), None);
    assert_eq!(state, before);
}

// ---------------------------------------------------------------------------
// 6. Traps (`UIF-MOD-001`)
// ---------------------------------------------------------------------------

#[test]
fn test_push_trap_reports_the_confinement() {
    let mut state = trap_scenery();
    assert!(!state.is_trapped());
    assert_eq!(state.trap_depth(), 0);
    assert_eq!(state.active_trap(), None);

    state.push_trap(gid("dlgA"));
    assert!(state.is_trapped());
    assert_eq!(state.trap_depth(), 1);
    assert_eq!(state.active_trap(), Some(&gid("dlgA")));
}

#[test]
fn test_a_trap_confines_forward_traversal_and_wraps_inside_it() {
    let mut state = trap_scenery();
    state.focus(&id("x"));
    state.push_trap(gid("dlgA"));
    state.focus_first_in_group(&gid("dlgA"));

    assert_eq!(state.focus_next(), Some(id("a2")));
    assert_eq!(state.focus_next(), Some(id("a1")));
    assert_eq!(state.focus_next(), Some(id("a2")));
}

#[test]
fn test_a_trap_confines_backward_traversal_and_wraps_inside_it() {
    let mut state = trap_scenery();
    state.focus(&id("x"));
    state.push_trap(gid("dlgA"));
    state.focus_first_in_group(&gid("dlgA"));

    assert_eq!(state.focus_previous(), Some(id("a2")));
    assert_eq!(state.focus_previous(), Some(id("a1")));
}

#[test]
fn test_a_trap_rejects_focusing_a_background_control() {
    let mut state = trap_scenery();
    state.focus(&id("a1"));
    state.push_trap(gid("dlgA"));
    let before = state.clone();

    assert!(!state.focus(&id("x")));
    assert!(!state.focus(&id("y")));
    assert!(!state.focus(&id("b1")));
    assert_eq!(state, before);
    assert!(state.focus(&id("a2")));
}

#[test]
fn test_a_trap_rejects_group_edges_and_roving_outside_it() {
    let mut state = trap_scenery();
    state.push_trap(gid("dlgA"));
    state.focus_first_in_group(&gid("dlgA"));
    let before = state.clone();

    assert_eq!(state.focus_first_in_group(&gid("base")), None);
    assert_eq!(state.focus_last_in_group(&gid("dlgB")), None);
    assert_eq!(state.move_within(&gid("base"), 1), None);
    assert_eq!(state, before);
}

#[test]
fn test_pop_restores_the_exact_pre_trap_focus() {
    let mut state = trap_scenery();
    state.focus(&id("y"));
    state.push_trap(gid("dlgA"));
    state.focus_first_in_group(&gid("dlgA"));
    assert_eq!(cur(&state), Some("a1"));

    let popped = state.pop_trap().expect("a trap was active");
    assert_eq!(popped.group, gid("dlgA"));
    assert_eq!(popped.recorded, Some(id("y")));
    assert_eq!(popped.restored, Some(id("y")));
    assert_eq!(popped.lost, Some(id("a1")));
    assert_eq!(cur(&state), Some("y"));
    assert!(!state.is_trapped());
}

#[test]
fn test_pop_with_no_trap_returns_none() {
    let mut state = standard();
    let before = state.clone();
    assert_eq!(state.pop_trap(), None);
    assert_eq!(state, before);
}

#[test]
fn test_pop_returns_to_the_recorded_focus_not_to_where_the_user_drifted() {
    let mut state = trap_scenery();
    state.focus(&id("x"));
    state.push_trap(gid("dlgA"));
    state.focus(&id("a2"));

    let popped = state.pop_trap().expect("a trap was active");
    assert_eq!(popped.restored, Some(id("x")));
    assert_eq!(cur(&state), Some("x"));
}

#[test]
fn test_a_trap_pushed_with_nothing_focused_restores_nothing() {
    let mut state = trap_scenery();
    state.push_trap(gid("dlgA"));
    state.focus_first_in_group(&gid("dlgA"));

    let popped = state.pop_trap().expect("a trap was active");
    assert_eq!(popped.recorded, None);
    assert_eq!(popped.restored, None);
    assert_eq!(cur(&state), None);
}

#[test]
fn test_nested_traps_confine_to_the_topmost_one() {
    let mut state = trap_scenery();
    state.focus(&id("x"));
    state.push_trap(gid("dlgA"));
    state.focus_first_in_group(&gid("dlgA"));
    state.push_trap(gid("dlgB"));
    state.focus_first_in_group(&gid("dlgB"));

    assert_eq!(state.trap_depth(), 2);
    assert_eq!(state.active_trap(), Some(&gid("dlgB")));
    assert_eq!(state.focus_next(), Some(id("b2")));
    assert_eq!(state.focus_next(), Some(id("b1")));

    let before = state.clone();
    assert!(!state.focus(&id("a1")));
    assert!(!state.focus(&id("x")));
    assert_eq!(state.move_within(&gid("dlgA"), 1), None);
    assert_eq!(state, before);
}

#[test]
fn test_nested_traps_restore_through_every_level() {
    let mut state = trap_scenery();
    state.focus(&id("x"));
    state.push_trap(gid("dlgA"));
    state.focus_first_in_group(&gid("dlgA"));
    state.push_trap(gid("dlgB"));
    state.focus_first_in_group(&gid("dlgB"));

    let outer = state.pop_trap().expect("dlgB trap");
    assert_eq!(outer.group, gid("dlgB"));
    assert_eq!(outer.recorded, Some(id("a1")));
    assert_eq!(outer.restored, Some(id("a1")));
    assert_eq!(state.active_trap(), Some(&gid("dlgA")));
    assert_eq!(state.focus_next(), Some(id("a2")));

    let base = state.pop_trap().expect("dlgA trap");
    assert_eq!(base.group, gid("dlgA"));
    assert_eq!(base.restored, Some(id("x")));
    assert_eq!(cur(&state), Some("x"));
    assert!(!state.is_trapped());
    assert_eq!(state.focus_next(), Some(id("y")));
}

#[test]
fn test_a_trap_on_an_unregistered_group_blocks_the_background_rather_than_ignoring_itself() {
    let mut state = trap_scenery();
    state.focus(&id("x"));
    state.push_trap(gid("not-yet-mounted"));

    // The scope is empty, so traversal is a no-op — it does NOT fall
    // through to the base workspace underneath the scrim.
    let before = state.clone();
    assert_eq!(state.focus_next(), None);
    assert_eq!(state.focus_previous(), None);
    assert!(!state.focus(&id("y")));
    assert_eq!(state, before);

    // Once the group registers, the trap resolves against it.
    state.register_group(gid("not-yet-mounted"), roster(&[("n1", true)]));
    assert_eq!(state.focus_next(), Some(id("n1")));
    assert_eq!(state.focus_previous(), Some(id("n1")));
}

#[test]
fn test_popping_a_trap_whose_recorded_focus_is_gone_falls_forward_in_the_wider_scope() {
    let mut state = trap_scenery();
    state.focus(&id("x"));
    state.push_trap(gid("dlgA"));
    state.focus_first_in_group(&gid("dlgA"));
    // The control that opened the dialog disappears while it is open.
    state.unregister_group(&gid("base"));

    let popped = state.pop_trap().expect("a trap was active");
    assert_eq!(popped.recorded, Some(id("x")));
    assert_eq!(popped.restored, Some(id("a1")));
    assert_eq!(cur(&state), Some("a1"));
}

// ---------------------------------------------------------------------------
// 7. Structural change under a live focus
// ---------------------------------------------------------------------------

#[test]
fn test_unregistering_the_focused_group_preserves_its_slot() {
    let mut state = standard();
    state.focus(&id("d"));
    // Enabled order `a c d e g`; removing `d e` slides `g` into slot 2.
    assert_eq!(state.unregister_group(&gid("body")), Some(id("d")));
    assert_eq!(cur(&state), Some("g"));
}

#[test]
fn test_unregistering_wraps_the_slot_into_the_shorter_order() {
    let mut state = standard();
    state.focus(&id("e"));
    // Enabled order `a c d e g`; `e` holds slot 3. Removing `body` leaves
    // `a c g`, and 3 % 3 == 0 → `a`.
    assert_eq!(state.unregister_group(&gid("body")), Some(id("e")));
    assert_eq!(cur(&state), Some("a"));
}

#[test]
fn test_unregistering_a_group_that_does_not_hold_focus_leaves_focus_alone() {
    let mut state = standard();
    state.focus(&id("a"));
    assert_eq!(state.unregister_group(&gid("foot")), None);
    assert_eq!(cur(&state), Some("a"));
    assert!(state.is_focusable(&id("c")));
    assert!(!state.is_focusable(&id("g")));
}

#[test]
fn test_reregistering_a_group_that_dropped_the_focused_member_falls_forward() {
    let mut state = standard();
    state.focus(&id("c"));
    // `c` held slot 1 of `a c d e g`; the order becomes `a d e g` and
    // slot 1 is `d`.
    state.register_group(gid("nav"), roster(&[("a", true)]));
    assert_eq!(cur(&state), Some("d"));
}

#[test]
fn test_reregistering_a_group_that_keeps_the_focused_member_changes_nothing() {
    let mut state = standard();
    state.focus(&id("c"));
    let before = state.clone();
    state.register_group(
        gid("nav"),
        roster(&[("a", true), ("b", false), ("c", true)]),
    );
    assert_eq!(state, before);
}

#[test]
fn test_unregistering_the_topmost_trap_group_restores_the_pre_trap_focus() {
    let mut state = trap_scenery();
    state.focus(&id("x"));
    state.push_trap(gid("dlgA"));
    state.focus_first_in_group(&gid("dlgA"));

    assert_eq!(state.unregister_group(&gid("dlgA")), Some(id("a1")));
    assert_eq!(cur(&state), Some("x"));
    assert_eq!(state.trap_depth(), 0);
    assert_eq!(state.focus_next(), Some(id("y")));
}

#[test]
fn test_unregistering_a_deeper_trap_group_keeps_the_upper_trap_governing() {
    let mut state = trap_scenery();
    state.focus(&id("x"));
    state.push_trap(gid("dlgA"));
    state.focus_first_in_group(&gid("dlgA"));
    state.push_trap(gid("dlgB"));
    state.focus_first_in_group(&gid("dlgB"));

    // The dialog underneath closes; the one on top must stay trapped.
    assert_eq!(state.unregister_group(&gid("dlgA")), None);
    assert_eq!(state.trap_depth(), 1);
    assert_eq!(state.active_trap(), Some(&gid("dlgB")));
    assert_eq!(cur(&state), Some("b1"));
    assert!(!state.focus(&id("x")));
    assert_eq!(state.focus_next(), Some(id("b2")));
}

#[test]
fn test_unregistering_a_trap_group_while_focus_is_outside_it_changes_nothing() {
    let mut state = trap_scenery();
    state.focus(&id("x"));
    state.push_trap(gid("dlgA"));
    // Focus never entered the dialog; closing it returns to the same place.
    assert_eq!(state.unregister_group(&gid("dlgA")), None);
    assert_eq!(cur(&state), Some("x"));
    assert_eq!(state.trap_depth(), 0);
}

#[test]
fn test_disabling_the_focused_member_moves_focus_forward() {
    let mut state = standard();
    state.focus(&id("c"));
    // `c` held slot 1 of `a c d e g`; the order becomes `a d e g` and
    // slot 1 is `d`.
    assert_eq!(
        state.set_member_enabled(&gid("nav"), &id("c"), false),
        Some(id("c"))
    );
    assert_eq!(cur(&state), Some("d"));
}

#[test]
fn test_disabling_the_last_enabled_member_wraps_to_the_first() {
    let mut state = standard();
    state.focus(&id("g"));
    assert_eq!(
        state.set_member_enabled(&gid("foot"), &id("g"), false),
        Some(id("g"))
    );
    assert_eq!(cur(&state), Some("a"));
}

#[test]
fn test_disabling_the_only_enabled_member_clears_focus() {
    let mut state = model(&[("solo", &[("only", true)])]);
    state.focus(&id("only"));
    assert_eq!(
        state.set_member_enabled(&gid("solo"), &id("only"), false),
        Some(id("only"))
    );
    assert_eq!(cur(&state), None);
    assert_eq!(state.focus_next(), None);
}

#[test]
fn test_disabling_a_member_that_is_not_focused_leaves_focus_alone() {
    let mut state = standard();
    state.focus(&id("a"));
    assert_eq!(state.set_member_enabled(&gid("nav"), &id("c"), false), None);
    assert_eq!(cur(&state), Some("a"));
    assert!(!state.is_focusable(&id("c")));
}

#[test]
fn test_enabling_a_member_never_moves_focus() {
    let mut state = standard();
    state.focus(&id("a"));
    assert_eq!(state.set_member_enabled(&gid("nav"), &id("b"), true), None);
    assert_eq!(cur(&state), Some("a"));
    // …and the member is now stoppable, in its original position.
    assert_eq!(state.focus_next(), Some(id("b")));
    assert_eq!(state.focus_next(), Some(id("c")));
}

#[test]
fn test_setting_an_unchanged_flag_is_a_no_op() {
    let mut state = standard();
    state.focus(&id("a"));
    let before = state.clone();
    assert_eq!(state.set_member_enabled(&gid("nav"), &id("a"), true), None);
    assert_eq!(state.set_member_enabled(&gid("nav"), &id("b"), false), None);
    assert_eq!(state, before);
}

#[test]
fn test_toggling_an_unknown_group_or_member_is_rejected() {
    let mut state = standard();
    state.focus(&id("a"));
    let before = state.clone();
    assert_eq!(
        state.set_member_enabled(&gid("ghost"), &id("a"), false),
        None
    );
    assert_eq!(
        state.set_member_enabled(&gid("nav"), &id("ghost"), false),
        None
    );
    assert_eq!(state, before);
}

#[test]
fn test_disabling_a_background_member_while_trapped_cannot_steal_focus() {
    let mut state = trap_scenery();
    state.focus(&id("a1"));
    state.push_trap(gid("dlgA"));

    assert_eq!(
        state.set_member_enabled(&gid("base"), &id("x"), false),
        None
    );
    assert_eq!(cur(&state), Some("a1"));
    assert!(!state.is_focusable(&id("x")));
    // Traversal is still confined to the dialog.
    assert_eq!(state.focus_next(), Some(id("a2")));
    assert_eq!(state.focus_next(), Some(id("a1")));
}

#[test]
fn test_disabling_everything_inside_a_trap_clears_focus_without_escaping() {
    let mut state = trap_scenery();
    state.focus(&id("a1"));
    state.push_trap(gid("dlgA"));

    assert_eq!(
        state.set_member_enabled(&gid("dlgA"), &id("a1"), false),
        Some(id("a1"))
    );
    assert_eq!(cur(&state), Some("a2"));
    assert_eq!(
        state.set_member_enabled(&gid("dlgA"), &id("a2"), false),
        Some(id("a2"))
    );
    assert_eq!(cur(&state), None);

    // Nothing in the trapped scope can hold focus, so traversal refuses
    // rather than handing focus to the unscrimmed background.
    assert_eq!(state.focus_next(), None);
    assert_eq!(state.focus_previous(), None);
    assert!(!state.focus(&id("x")));
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

/// A scripted drive through every kind of operation, run twice.
fn script(state: &mut FocusState) {
    state.register_group(
        gid("nav"),
        roster(&[("a", true), ("b", false), ("c", true)]),
    );
    state.register_group(gid("body"), roster(&[("d", true), ("e", true)]));
    state.focus_next();
    state.focus_next();
    state.focus_previous();
    state.register_group(gid("foot"), roster(&[("f", false), ("g", true)]));
    state.focus(&id("e"));
    state.set_member_enabled(&gid("body"), &id("e"), false);
    state.push_trap(gid("foot"));
    state.focus_first_in_group(&gid("foot"));
    state.move_within(&gid("foot"), 1);
    state.push_trap(gid("body"));
    state.focus_last_in_group(&gid("body"));
    state.pop_trap();
    state.pop_trap();
    state.unregister_group(&gid("foot"));
    state.focus_next();
    state.clear();
}

#[test]
fn test_identical_operation_sequences_produce_identical_state() {
    let mut first = FocusState::new();
    let mut second = FocusState::new();
    script(&mut first);
    script(&mut second);
    assert_eq!(first, second);
}

#[test]
fn test_a_fresh_model_is_empty() {
    let state = FocusState::default();
    assert!(state.groups().is_empty());
    assert_eq!(state.current(), None);
    assert!(!state.is_trapped());
    assert_eq!(state.trap_depth(), 0);
}

#[test]
fn test_a_cloned_model_is_independent_of_its_original() {
    let mut original = standard();
    original.focus(&id("c"));
    let mut copy = original.clone();

    copy.focus_next();
    copy.push_trap(gid("body"));
    copy.set_member_enabled(&gid("nav"), &id("a"), false);

    assert_eq!(cur(&original), Some("c"));
    assert!(!original.is_trapped());
    assert!(original.is_focusable(&id("a")));
    assert_eq!(cur(&copy), Some("d"));
}

#[test]
fn test_queries_never_mutate_the_model() {
    let mut state = standard();
    state.focus(&id("c"));
    state.push_trap(gid("body"));
    let before = state.clone();

    let _ = state.current();
    let _ = state.is_focused(&id("c"));
    let _ = state.is_focusable(&id("b"));
    let _ = state.in_active_scope(&id("a"));
    let _ = state.groups();
    let _ = state.group(&gid("nav"));
    let _ = state.group_of(&id("d"));
    let _ = state.focusable_members(&gid("body"));
    let _ = state.active_trap();
    let _ = state.trap_depth();
    let _ = state.is_trapped();

    assert_eq!(state, before);
}

#[test]
fn test_the_model_holds_no_shared_or_borrowed_state() {
    fn assert_static_send_sync<T: 'static + Send + Sync>() {}
    assert_static_send_sync::<FocusState>();
    assert_static_send_sync::<FocusId>();
    assert_static_send_sync::<GroupId>();
    assert_static_send_sync::<FocusMember>();
    assert_static_send_sync::<TrapPop>();
}
