//! Tests for the diagonal submenu aiming corridor (`UIF-PRIN-008`,
//! `UIF-FMENU-002`).
//!
//! Every test feeds time in explicitly as a [`TimeMs`] value. Nothing
//! here sleeps, and nothing here can drift with wall-clock time: the
//! corridor's whole contract is that a given event sequence at given
//! timestamps produces a given decision.

use crate::surface::menu::aim::{AimCorridor, AimDecision, AimTarget, grace_ms};
use crate::surface::tooltip::TimeMs;
use crate::token::time::DELAY_SUBMENU;

const TRIGGER: usize = 2;
const SIBLING: usize = 3;

const fn t(ms: u64) -> TimeMs {
    TimeMs(ms)
}

/// Open a branch on `TRIGGER` at `t(0)` with the pointer on it.
fn corridor_with_open_branch() -> AimCorridor {
    let mut aim = AimCorridor::new();
    aim.branch_opened(TRIGGER, t(0));
    aim.pointer_enter(AimTarget::ParentRow { row: TRIGGER }, t(0));
    aim
}

// -- the token is the source of the number ------------------------------------

#[test]
fn the_grace_window_is_the_submenu_token_not_a_restated_number() {
    assert_eq!(grace_ms(), DELAY_SUBMENU.ms);
    assert_eq!(grace_ms(), 150, "time.delay.submenu is 150ms");
}

// -- nothing open -------------------------------------------------------------

#[test]
fn an_empty_corridor_has_nothing_to_protect() {
    let aim = AimCorridor::new();
    assert_eq!(aim.decision(t(0)), AimDecision::Idle);
    assert_eq!(aim.decision(t(10_000)), AimDecision::Idle);
    assert_eq!(aim.open_trigger(), None);
    assert_eq!(aim.corridor_remaining(t(0)), 0);
    assert!(!aim.is_aiming(t(0)));
}

#[test]
fn idle_does_not_become_a_switch_however_long_it_waits() {
    let aim = AimCorridor::new();
    for ms in [0, 1, 149, 150, 151, 60_000] {
        assert_eq!(aim.decision(t(ms)), AimDecision::Idle);
    }
}

// -- the trigger row preserves ------------------------------------------------

#[test]
fn hovering_the_open_branchs_own_trigger_preserves_it_indefinitely() {
    let aim = corridor_with_open_branch();
    for ms in [0, 1, 149, 150, 151, 10_000] {
        assert_eq!(
            aim.decision(t(ms)),
            AimDecision::Preserve,
            "§4.9: the trigger row must never flicker the branch shut"
        );
        assert!(aim.decision(t(ms)).keeps_branch());
    }
    assert_eq!(
        aim.corridor_remaining(t(5_000)),
        0,
        "nothing is being held off"
    );
}

#[test]
fn entering_the_child_branch_itself_preserves_it() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ChildBranch, t(40));
    assert_eq!(aim.decision(t(40)), AimDecision::Preserve);
    // The whole point of the corridor: arriving in the branch late still
    // preserves, because the target is the branch not a sibling.
    assert_eq!(aim.decision(t(5_000)), AimDecision::Preserve);
}

// -- the corridor suppresses the switch ---------------------------------------

#[test]
fn switching_to_a_sibling_before_150ms_is_suppressed() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(100));
    let decision = aim.decision(t(200));
    assert_eq!(
        decision,
        AimDecision::Hold {
            row: SIBLING,
            remaining_ms: 50,
        },
        "100ms of the corridor was spent before entering, 50ms remain"
    );
    assert!(decision.keeps_branch());
    assert!(!decision.switches());
    assert!(aim.is_aiming(t(200)));
}

#[test]
fn branch_opened_at_protects_sibling_switch_even_if_pointer_was_reported_earlier() {
    let mut aim = AimCorridor::new();
    // Pointer was reported on a sibling row before the branch was marked open
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(50));
    // The branch actually opens at t(200)
    aim.branch_opened(TRIGGER, t(200));

    // At t(210), 10ms after branch opened, the grace window must hold
    let decision = aim.decision(t(210));
    assert_eq!(
        decision,
        AimDecision::Hold {
            row: SIBLING,
            remaining_ms: 140,
        },
        "grace window must measure from branch_opened when pointer was set earlier"
    );
    assert!(aim.is_aiming(t(210)));

    // At t(350), 150ms after branch opened, switch is permitted
    assert_eq!(aim.decision(t(350)), AimDecision::Switch { row: SIBLING });
}

#[test]
fn the_hold_counts_down_exactly() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(0));
    for (now, remaining) in [(0, 150), (1, 149), (74, 76), (149, 1)] {
        assert_eq!(
            aim.decision(t(now)),
            AimDecision::Hold {
                row: SIBLING,
                remaining_ms: remaining,
            }
        );
        assert_eq!(aim.corridor_remaining(t(now)), remaining);
    }
}

#[test]
fn the_switch_is_permitted_at_exactly_150ms() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(0));
    assert_eq!(
        aim.decision(t(149)),
        AimDecision::Hold {
            row: SIBLING,
            remaining_ms: 1
        }
    );
    assert_eq!(aim.decision(t(150)), AimDecision::Switch { row: SIBLING });
    assert!(aim.decision(t(150)).switches());
}

#[test]
fn the_switch_is_permitted_after_the_corridor_elapses() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(0));
    for ms in [151, 200, 10_000] {
        assert_eq!(aim.decision(t(ms)), AimDecision::Switch { row: SIBLING });
        assert_eq!(aim.corridor_remaining(t(ms)), 0);
        assert!(!aim.is_aiming(t(ms)));
    }
}

#[test]
fn a_diagonal_crossing_that_reaches_the_branch_before_150ms_never_collapses_it() {
    // The manoeuvre the corridor exists for: leave the trigger, cross the
    // sibling row, arrive inside the branch — all inside the window.
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(10));
    assert!(aim.decision(t(30)).keeps_branch());
    aim.pointer_enter(AimTarget::ChildBranch, t(50));
    assert_eq!(aim.decision(t(50)), AimDecision::Preserve);
    // And it stays preserved long after the window would have closed,
    // because the pointer is no longer on a sibling.
    assert_eq!(aim.decision(t(10_000)), AimDecision::Preserve);
}

// -- re-entry resets ----------------------------------------------------------

#[test]
fn re_entering_the_sibling_restars_the_corridor() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(0));
    assert_eq!(aim.decision(t(150)), AimDecision::Switch { row: SIBLING });

    // Back to the trigger, then out to the sibling again: the window is
    // measured from the new arrival, not the original one.
    aim.pointer_enter(AimTarget::ParentRow { row: TRIGGER }, t(160));
    assert_eq!(aim.decision(t(160)), AimDecision::Preserve);
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(170));
    assert_eq!(
        aim.decision(t(170)),
        AimDecision::Hold {
            row: SIBLING,
            remaining_ms: 150
        },
        "re-entry must restart the full window"
    );
    assert_eq!(
        aim.decision(t(319)),
        AimDecision::Hold {
            row: SIBLING,
            remaining_ms: 1
        }
    );
    assert_eq!(aim.decision(t(320)), AimDecision::Switch { row: SIBLING });
}

#[test]
fn leaving_and_coming_back_through_outside_restars_the_corridor() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(0));
    aim.pointer_out(t(50));
    assert_eq!(aim.decision(t(50)), AimDecision::Collapse);
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(60));
    assert_eq!(
        aim.decision(t(60)),
        AimDecision::Hold {
            row: SIBLING,
            remaining_ms: 150
        }
    );
}

#[test]
fn re_reporting_the_same_target_does_not_push_the_window_forward() {
    // A host polling every frame must not be able to hold the corridor
    // open forever by re-announcing where the pointer already is.
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(0));
    for frame in 0..100 {
        aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(frame));
    }
    assert_eq!(
        aim.decision(t(150)),
        AimDecision::Switch { row: SIBLING },
        "repeated identical reports must not extend the grace window"
    );
}

// -- going outside ------------------------------------------------------------

#[test]
fn moving_outside_reports_collapse() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_out(t(30));
    assert_eq!(aim.decision(t(30)), AimDecision::Collapse);
    assert!(!aim.decision(t(30)).keeps_branch());
    assert!(!aim.decision(t(30)).switches());
}

#[test]
fn a_lost_target_reports_collapse_not_idle() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::Outside, t(10));
    assert_eq!(aim.decision(t(10)), AimDecision::Collapse);
    // Collapse is reported, never applied: the corridor still believes a
    // branch is open until the owner says otherwise.
    assert_eq!(aim.open_trigger(), Some(TRIGGER));
}

#[test]
fn closing_the_branch_clears_the_trigger_but_keeps_the_reported_pointer() {
    let mut aim = corridor_with_open_branch();
    aim.branch_closed();
    assert_eq!(aim.open_trigger(), None);
    assert_eq!(aim.target(), Some(AimTarget::ParentRow { row: TRIGGER }));
    assert_eq!(aim.decision(t(0)), AimDecision::Idle);
}

#[test]
fn reset_forgets_everything() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(10));
    aim.reset();
    assert_eq!(aim, AimCorridor::new());
    assert_eq!(aim.decision(t(10)), AimDecision::Idle);
    assert_eq!(aim.target(), None);
}

// -- opening a later branch ---------------------------------------------------

#[test]
fn a_newly_opened_branch_gets_its_own_full_window() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(0));
    assert_eq!(aim.decision(t(150)), AimDecision::Switch { row: SIBLING });
    // The host commits the switch.
    aim.branch_opened(SIBLING, t(150));
    assert_eq!(aim.open_trigger(), Some(SIBLING));
    // Now the *old* trigger is the sibling, and gets the window too.
    aim.pointer_enter(AimTarget::ParentRow { row: TRIGGER }, t(160));
    assert_eq!(
        aim.decision(t(200)),
        AimDecision::Hold {
            row: TRIGGER,
            remaining_ms: 110
        }
    );
}

#[test]
fn moving_to_a_third_row_restarts_the_window_for_that_row() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(0));
    assert_eq!(
        aim.decision(t(100)),
        AimDecision::Hold {
            row: SIBLING,
            remaining_ms: 50
        }
    );
    aim.pointer_enter(AimTarget::ParentRow { row: 7 }, t(100));
    assert_eq!(
        aim.decision(t(100)),
        AimDecision::Hold {
            row: 7,
            remaining_ms: 150
        }
    );
}

// -- clock hygiene ------------------------------------------------------------

#[test]
fn a_clock_that_went_backwards_cannot_produce_a_negative_wait() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(1_000));
    // `now` earlier than the arrival time.
    let decision = aim.decision(t(900));
    assert_eq!(
        decision,
        AimDecision::Hold {
            row: SIBLING,
            remaining_ms: 150
        }
    );
    assert_eq!(aim.corridor_remaining(t(0)), 150);
}

// -- determinism --------------------------------------------------------------

#[test]
fn the_same_events_at_the_same_times_produce_the_same_decisions() {
    let run = || {
        let mut aim = AimCorridor::new();
        let mut log = Vec::new();
        aim.branch_opened(TRIGGER, t(0));
        log.push(aim.decision(t(0)));
        aim.pointer_enter(AimTarget::ParentRow { row: TRIGGER }, t(5));
        log.push(aim.decision(t(5)));
        aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(50));
        log.push(aim.decision(t(50)));
        log.push(aim.decision(t(100)));
        log.push(aim.decision(t(199)));
        log.push(aim.decision(t(200)));
        aim.pointer_enter(AimTarget::ChildBranch, t(210));
        log.push(aim.decision(t(210)));
        aim.pointer_out(t(300));
        log.push(aim.decision(t(300)));
        aim.branch_closed();
        log.push(aim.decision(t(300)));
        log
    };
    let first = run();
    let second = run();
    assert_eq!(first, second, "the corridor is not deterministic");
    assert_eq!(first.len(), 9);
}

#[test]
fn decision_is_a_pure_query_and_never_mutates() {
    let mut aim = corridor_with_open_branch();
    aim.pointer_enter(AimTarget::ParentRow { row: SIBLING }, t(0));
    let before = aim.clone();
    for ms in 0..400 {
        let _ = aim.decision(t(ms));
    }
    assert_eq!(aim, before, "decision() must not change the corridor");
}

#[test]
fn the_decision_helpers_agree_with_the_variants() {
    assert!(AimDecision::Preserve.keeps_branch());
    assert!(
        AimDecision::Hold {
            row: 1,
            remaining_ms: 1
        }
        .keeps_branch()
    );
    assert!(!AimDecision::Switch { row: 1 }.keeps_branch());
    assert!(!AimDecision::Collapse.keeps_branch());
    assert!(!AimDecision::Idle.keeps_branch());
    assert!(AimDecision::Switch { row: 4 }.switches());
    assert!(
        !AimDecision::Hold {
            row: 4,
            remaining_ms: 9
        }
        .switches()
    );
}

// -- target shape -------------------------------------------------------------

#[test]
fn only_a_parent_row_target_names_a_row() {
    assert_eq!(AimTarget::ParentRow { row: 3 }.row(), Some(3));
    assert_eq!(AimTarget::ChildBranch.row(), None);
    assert_eq!(AimTarget::Outside.row(), None);
}

#[test]
fn pointer_left_card_reports_departure_without_closing_anything() {
    let mut aim = corridor_with_open_branch();
    assert!(
        MenuCascade::pointer_left_card(&mut aim, t(30)),
        "an open branch that just lost the pointer should collapse"
    );
    assert_eq!(aim.target(), Some(AimTarget::Outside));
    assert_eq!(
        aim.open_trigger(),
        Some(TRIGGER),
        "the corridor reports the collapse, it never applies it"
    );
    // A repeated report is idempotent: same answer, same state.
    let after_first = aim.clone();
    assert!(MenuCascade::pointer_left_card(&mut aim, t(40)));
    assert_eq!(
        aim, after_first,
        "a repeated report must not advance anything"
    );
}

#[test]
fn pointer_left_card_says_no_when_nothing_is_open() {
    let mut aim = AimCorridor::new();
    assert!(
        !MenuCascade::pointer_left_card(&mut aim, t(0)),
        "with no branch open there is nothing to collapse"
    );
}

// -- the corridor wired into the cascade --------------------------------------

use crate::focus::FocusState;
use crate::surface::menu::cascade::{MenuCascade, MenuTreeKind};
use crate::surface::menu::model::{Menu, Row};

/// A root card with two submenu parents, so a sibling switch is possible.
fn two_branch_menu() -> Menu {
    Menu::new(
        "aim-root",
        vec![
            Row::submenu("a", "Branch A", leaf("a")),
            Row::submenu("b", "Branch B", leaf("b")),
        ],
    )
}

fn leaf(which: &str) -> Menu {
    Menu::new(format!("aim-{which}"), vec![Row::action("x", "Item")])
}

/// Open the root and hover branch A into existence.
fn open_with_branch_a() -> (MenuCascade, AimCorridor, FocusState) {
    let menu = two_branch_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    let mut aim = AimCorridor::new();
    cascade.open(&menu, MenuTreeKind::Context, &mut focus);
    let root = menu.id().clone();
    cascade.hover_submenu_row(&mut aim, &root, 0, t(0), &mut focus);
    assert_eq!(cascade.depth(), 2, "branch A should be open");
    assert_eq!(aim.open_trigger(), Some(0));
    (cascade, aim, focus)
}

#[test]
fn hovering_a_sibling_before_150ms_leaves_the_open_branch_alone() {
    let (mut cascade, mut aim, mut focus) = open_with_branch_a();
    let root = cascade.root_id().expect("root").clone();

    let outcome = cascade.hover_submenu_row(&mut aim, &root, 1, t(50), &mut focus);
    assert!(
        outcome.opened().is_empty(),
        "the corridor must suppress the switch: {:?}",
        outcome.opened()
    );
    assert_eq!(cascade.depth(), 2, "branch A must still be open");
    assert_eq!(aim.open_trigger(), Some(0));
    assert!(aim.is_aiming(t(50)));
}

#[test]
fn hovering_a_sibling_after_150ms_replaces_the_branch() {
    let (mut cascade, mut aim, mut focus) = open_with_branch_a();
    let root = cascade.root_id().expect("root").clone();

    // First crossing is suppressed: the corridor is open.
    cascade.hover_submenu_row(&mut aim, &root, 1, t(50), &mut focus);
    assert_eq!(cascade.depth(), 2);
    assert_eq!(aim.open_trigger(), Some(0));

    // Back to the trigger, then out to the sibling again — the arrival
    // time is now 100ms, so by 250ms the corridor has elapsed.
    cascade.hover_submenu_row(&mut aim, &root, 0, t(100), &mut focus);
    assert_eq!(aim.open_trigger(), Some(0), "the trigger preserves itself");
    cascade.hover_submenu_row(&mut aim, &root, 1, t(100), &mut focus);
    assert_eq!(cascade.depth(), 2, "still suppressed at arrival");

    let outcome = cascade.hover_submenu_row(&mut aim, &root, 1, t(250), &mut focus);
    assert_eq!(
        outcome.opened().len(),
        1,
        "branch B must open once the corridor has elapsed"
    );
    assert_eq!(cascade.depth(), 2, "the sibling replaced, not stacked");
    assert_eq!(aim.open_trigger(), Some(1));
}

#[test]
fn hovering_the_open_branchs_trigger_through_the_cascade_preserves_it() {
    let (mut cascade, mut aim, mut focus) = open_with_branch_a();
    let root = cascade.root_id().expect("root").clone();

    let outcome = cascade.hover_submenu_row(&mut aim, &root, 0, t(5_000), &mut focus);
    assert!(
        outcome.opened().is_empty(),
        "re-hovering the open trigger must not re-open anything"
    );
    assert_eq!(cascade.depth(), 2);
    assert_eq!(aim.open_trigger(), Some(0));
}

#[test]
fn the_cascade_and_corridor_never_drift_apart() {
    let (cascade, aim, mut focus) = open_with_branch_a();

    // A corridor that claims row 1 is the trigger, pointed at a menu
    // whose row 0 has no child at all. The corridor permits the switch
    // once its window has elapsed, the open fails, and the corridor must
    // clear rather than go on guarding a branch that does not exist.
    let plain = Menu::new("aim-plain", vec![Row::action("only", "Only")]);
    let mut plain_cascade = MenuCascade::new();
    plain_cascade.open(&plain, MenuTreeKind::Context, &mut focus);
    let plain_root = plain.id().clone();
    let mut plain_aim = AimCorridor::new();
    plain_aim.branch_opened(1, t(0));
    plain_aim.pointer_enter(AimTarget::ParentRow { row: 1 }, t(0));

    // Arrival at 500ms: the corridor is fresh, so nothing happens yet.
    let first = plain_cascade.hover_submenu_row(&mut plain_aim, &plain_root, 0, t(500), &mut focus);
    assert!(
        first.opened().is_empty(),
        "a fresh arrival must not switch immediately"
    );
    assert_eq!(plain_aim.open_trigger(), Some(1));

    // The host comes back after the window: the switch is permitted, and
    // the open fails because the row has no child.
    let outcome =
        plain_cascade.hover_submenu_row(&mut plain_aim, &plain_root, 0, t(700), &mut focus);
    assert!(outcome.is_ignored(), "row 0 of a plain menu has no child");
    assert_eq!(
        plain_aim.open_trigger(),
        None,
        "a failed open must clear the corridor, not leave it guarding nothing"
    );

    // The original pair is untouched by any of the above.
    assert_eq!(cascade.depth(), 2);
    assert_eq!(aim.open_trigger(), Some(0));
}

#[test]
fn the_cascade_hover_path_is_deterministic() {
    let run = || {
        let (mut cascade, mut aim, mut focus) = open_with_branch_a();
        let root = cascade.root_id().expect("root").clone();
        let mut log = Vec::new();
        for ms in [60u64, 100, 149, 150, 151, 400] {
            let outcome = cascade.hover_submenu_row(&mut aim, &root, 1, t(ms), &mut focus);
            log.push((
                ms,
                cascade.depth(),
                aim.open_trigger(),
                outcome.opened().len(),
            ));
        }
        log
    };
    assert_eq!(run(), run(), "the cascade hover path is not deterministic");
}
