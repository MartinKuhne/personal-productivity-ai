//! Tests for the app-side submenu aiming driver (`UIF-PRIN-008`,
//! `UIF-FMENU-002`).
//!
//! Every test here drives the corridor at a **chosen millisecond** rather
//! than a slept one, which is the whole point of
//! [`menu_aim`](super) taking `now` as a parameter: the 150 ms window is
//! verified by arithmetic, not by timing, so none of these can be flaky
//! on a loaded machine.
//!
//! The reducer wiring — that a `MenuMessage` actually reaches these
//! functions and that nothing else fires — is pinned in
//! [`update_tests`](crate::ui_iced::update), where the live `AppState`
//! fixtures live.

use super::*;

use iced_pro::surface::menu::aim::grace_ms;
use iced_pro::surface::menu::cascade::MenuTreeKind;
use iced_pro::surface::menu::model::{Menu, MenuId, Row, submenu_id};

/// Root id of the fixture tree.
const ROOT: &str = "aim-root";

/// Row indices of the fixture's root card.
const PLAIN: usize = 0;
const BRANCH_A: usize = 1;
const BRANCH_B: usize = 2;

/// A moment on the injected clock.
fn at(ms: u64) -> TimeMs {
    TimeMs::millis(ms)
}

/// The id the framework derives for the submenu hanging off row `key`.
fn child_id(key: &str) -> MenuId {
    submenu_id(&MenuId::new(ROOT), key)
}

/// A one-row child card.
fn leaf(which: &str) -> Menu {
    Menu::new(
        child_id(which),
        vec![Row::action(format!("{which}-1"), "a navigable row")],
    )
}

/// The fixture tree: one plain action and two submenu parents.
fn root_menu() -> Menu {
    Menu::new(
        ROOT,
        vec![
            Row::action("plain", "Plain action"),
            Row::submenu("a", "Branch A", leaf("a")),
            Row::submenu("b", "Branch B", leaf("b")),
        ],
    )
}

/// Cascade, corridor and focus, with the root card open and nothing else.
struct Fixture {
    cascade: MenuCascade,
    aim: AimCorridor,
    focus: FocusState,
    root: MenuId,
}

fn fixture() -> Fixture {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    cascade.open(&root_menu(), MenuTreeKind::Fixed, &mut focus);

    Fixture {
        cascade,
        aim: AimCorridor::new(),
        focus,
        root: MenuId::new(ROOT),
    }
}

impl Fixture {
    /// The id of the deepest card currently mounted.
    fn deepest(&self) -> Option<MenuId> {
        self.cascade
            .open_cards()
            .last()
            .map(|card| card.id().clone())
    }

    /// Hover `row` of the root card at `now`.
    fn enter_root(&mut self, row: usize, now: TimeMs) -> AimStep {
        on_row_entered(
            &mut self.cascade,
            &mut self.aim,
            &self.root.clone(),
            row,
            now,
            &mut self.focus,
        )
    }

    /// Hover `row` of the card `card` at `now`.
    fn enter(&mut self, card: &MenuId, row: usize, now: TimeMs) -> AimStep {
        on_row_entered(
            &mut self.cascade,
            &mut self.aim,
            card,
            row,
            now,
            &mut self.focus,
        )
    }

    /// Let the corridor's deadline fire at `now`.
    fn deadline(&mut self, now: TimeMs) -> AimStep {
        on_deadline(
            &mut self.cascade,
            &mut self.aim,
            &self.root.clone(),
            now,
            &mut self.focus,
        )
    }

    /// Open branch A from its trigger row and return the step that did it.
    fn open_branch_a(&mut self, now: TimeMs) -> AimStep {
        let step = self.enter_root(BRANCH_A, now);
        assert!(step.switched, "branch A opens straight away from idle");
        assert_eq!(self.deepest(), Some(child_id("a")));
        step
    }
}

// ---------------------------------------------------------------------------
// Opening from idle
// ---------------------------------------------------------------------------

#[test]
fn test_hovering_a_submenu_parent_opens_its_branch_from_idle() {
    let mut fx = fixture();

    let step = fx.enter_root(BRANCH_A, at(0));

    assert!(step.switched);
    assert_eq!(fx.deepest(), Some(child_id("a")));
    assert_eq!(
        step.wake_after_ms, None,
        "nothing is being held off, so no wake-up is owed"
    );
    assert_eq!(fx.aim.open_trigger(), Some(BRANCH_A));
}

#[test]
fn test_a_plain_action_row_is_reported_but_opens_nothing() {
    let mut fx = fixture();

    let step = fx.enter_root(PLAIN, at(0));

    assert!(
        !step.switched,
        "there is no branch behind a plain action row to open"
    );
    assert_eq!(
        fx.cascade.open_cards().len(),
        1,
        "the root card is all that is mounted"
    );
    assert_eq!(
        fx.aim.target(),
        Some(AimTarget::ParentRow { row: PLAIN }),
        "the corridor still has to see the pointer, or it cannot time a \
         later crossing honestly"
    );
}

// ---------------------------------------------------------------------------
// §4.8.2 — a non-parent sibling retires the branch, after the corridor
// ---------------------------------------------------------------------------

#[test]
fn test_crossing_to_a_plain_sibling_holds_before_retiring_the_branch() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));

    // Same-instant crossing: the corridor is wide open, so the branch
    // must survive this frame even though the row landed on is not the
    // trigger.
    let step = fx.enter_root(PLAIN, at(0));

    assert!(
        !step.switched,
        "a crossing inside the corridor must not retire the branch yet"
    );
    assert_eq!(
        fx.deepest(),
        Some(child_id("a")),
        "branch A is still mounted while the window is open"
    );
    assert_eq!(
        step.wake_after_ms,
        Some(grace_ms()),
        "the retire is deferred to the deadline, not dropped"
    );
}

#[test]
fn test_the_plain_sibling_crossing_retires_the_branch_once_the_window_elapses() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));
    fx.enter_root(PLAIN, at(1_000));

    let step = fx.deadline(at(1_000 + grace_ms()));

    assert!(
        step.switched,
        "§4.8.2: moving to a sibling row on an ancestor card retires the \
         deeper child branch after the corridor check"
    );
    assert_eq!(
        fx.cascade.depth(),
        1,
        "only the root card remains — the branch the pointer walked away \
         from is gone"
    );
    assert_eq!(
        fx.deepest(),
        Some(MenuId::new(ROOT)),
        "the retire stops at the root and never takes the menu itself"
    );
    assert_eq!(
        fx.aim.open_trigger(),
        None,
        "a guard over a branch that no longer exists must not survive the retire"
    );
    assert_eq!(
        step.wake_after_ms, None,
        "the retire is terminal; nothing further is owed"
    );
}

#[test]
fn test_retiring_when_no_branch_is_open_is_a_no_op() {
    let mut fx = fixture();

    // No branch was ever opened, so the retire rule fires against an
    // empty subtree and must report that nothing changed rather than
    // claiming a switch.
    let step = fx.enter_root(PLAIN, at(0));

    assert!(!step.switched, "there was no deeper branch to retire");
    assert_eq!(fx.cascade.depth(), 1, "the root card is untouched");
}

#[test]
fn test_a_retire_leaves_the_menu_open_so_dismissal_stays_with_click_away() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));
    fx.enter_root(PLAIN, at(1_000));
    fx.deadline(at(1_000 + grace_ms()));

    assert!(
        fx.cascade.is_open(),
        "retiring a branch is not dismissing the menu — the root card is \
         still up and only the click-away blocker may close it"
    );
}

#[test]
fn test_retiring_frees_the_corridor_so_the_next_crossing_is_not_held() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));
    fx.enter_root(PLAIN, at(1_000));
    fx.deadline(at(1_000 + grace_ms()));

    // With the guard cleared, opening branch B from here is a fresh open
    // from idle, not a sibling crossing that owes another 150 ms.
    let step = fx.enter_root(BRANCH_B, at(1_200));

    assert!(
        step.switched,
        "a cleared corridor opens the new branch immediately rather than \
         holding on a retired trigger"
    );
    assert_eq!(
        fx.deepest(),
        Some(child_id("b")),
        "branch B is now the mounted branch"
    );
}

// ---------------------------------------------------------------------------
// The grace window holds
// ---------------------------------------------------------------------------

#[test]
fn test_a_sibling_crossing_inside_the_grace_window_does_not_switch() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));

    // The pointer leaves branch A's trigger and lands on the next
    // sibling in the same instant the branch opened — the diagonal
    // crossing this whole module exists to tolerate.
    let step = fx.enter_root(BRANCH_B, at(0));

    assert!(
        !step.switched,
        "crossing a sibling inside the corridor must not retire the branch"
    );
    assert_eq!(
        fx.deepest(),
        Some(child_id("a")),
        "branch A survives the crossing"
    );
    assert_eq!(
        step.wake_after_ms,
        Some(grace_ms()),
        "the full grace window is what is left, read from the token"
    );
}

#[test]
fn test_the_window_still_holds_one_millisecond_before_it_elapses() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));
    fx.enter_root(BRANCH_B, at(0));

    let just_short = at(grace_ms() - 1);
    let step = fx.deadline(just_short);

    assert!(!step.switched);
    assert_eq!(fx.deepest(), Some(child_id("a")));
    assert_eq!(
        step.wake_after_ms,
        Some(1),
        "one millisecond of corridor left, so one millisecond of sleep owed"
    );
}

// ---------------------------------------------------------------------------
// The grace window matures
// ---------------------------------------------------------------------------

#[test]
fn test_the_sibling_switch_fires_once_the_window_has_elapsed() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));
    fx.enter_root(BRANCH_B, at(0));

    let step = fx.deadline(at(grace_ms()));

    assert!(
        step.switched,
        "the held switch must mature on the frame the corridor elapses"
    );
    assert_eq!(fx.deepest(), Some(child_id("b")));
    assert_eq!(fx.aim.open_trigger(), Some(BRANCH_B));
    assert_eq!(step.wake_after_ms, None);
}

#[test]
fn test_a_wake_that_lands_early_re_arms_rather_than_being_lost() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));
    fx.enter_root(BRANCH_B, at(0));

    // A wake delivered early — a coarse clock, a busy queue — must not
    // be mistaken for "nothing to do".
    let early = fx.deadline(at(20));
    assert!(!early.switched);
    assert_eq!(early.wake_after_ms, Some(grace_ms() - 20));

    let later = fx.deadline(at(grace_ms()));
    assert!(later.switched);
    assert_eq!(fx.deepest(), Some(child_id("b")));
}

// ---------------------------------------------------------------------------
// No-flicker (§4.9)
// ---------------------------------------------------------------------------

#[test]
fn test_hovering_the_open_trigger_row_preserves_without_reopening() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));

    // Coming back to the trigger that is already open must be a no-op:
    // closing and re-opening under the cursor is the flicker §4.9 names.
    let step = fx.enter_root(BRANCH_A, at(10));

    assert!(!step.switched);
    assert_eq!(fx.deepest(), Some(child_id("a")));
    assert_eq!(fx.aim.open_trigger(), Some(BRANCH_A));
    assert_eq!(fx.aim.decision(at(10)), AimDecision::Preserve);
    assert_eq!(step.wake_after_ms, None);
}

#[test]
fn test_the_pointer_inside_the_open_child_branch_preserves_the_branch() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));
    let branch = child_id("a");

    // The diagonal manoeuvre completing: the pointer has arrived in the
    // card it was aiming at.
    let step = fx.enter(&branch, 0, at(50));

    assert!(!step.switched);
    assert_eq!(
        fx.aim.target(),
        Some(AimTarget::ChildBranch),
        "a card below the root is the branch, not a sibling row"
    );
    assert_eq!(fx.aim.decision(at(50)), AimDecision::Preserve);
    assert!(
        fx.cascade.is_card_open(&branch),
        "arriving at the target must not be what closes it"
    );
    assert_eq!(step.wake_after_ms, None);
}

#[test]
fn test_returning_from_the_child_branch_to_a_sibling_re_opens_the_window() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));
    let branch = child_id("a");
    fx.enter(&branch, 0, at(50));

    // Out of the branch and onto the next sibling: a fresh crossing, so
    // a fresh grace window applies from the arrival.
    let step = fx.enter_root(BRANCH_B, at(100));
    assert!(!step.switched);
    assert_eq!(step.wake_after_ms, Some(grace_ms()));

    let matured = fx.deadline(at(100 + grace_ms()));
    assert!(matured.switched);
    assert_eq!(fx.deepest(), Some(child_id("b")));
}

// ---------------------------------------------------------------------------
// Dismissal is NOT hijacked
// ---------------------------------------------------------------------------

#[test]
fn test_the_pointer_leaving_the_menu_closes_nothing() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));
    let branch = child_id("a");

    // The pointer sweeps out past the edge of everything, and the
    // corridor answers exactly what it is allowed to say: "would close".
    let corridor_says_collapse = on_pointer_left(&mut fx.aim, at(50));

    assert!(
        corridor_says_collapse,
        "the corridor does report the pointer as outside everything"
    );

    // ...and that report changed **nothing**. Both cards are still up.
    // This is the regression guard for the advisory contract: were this
    // ever wired to a dismissal, every menu in the app would shut when
    // the mouse swept past it.
    assert!(
        fx.cascade.is_open(),
        "a pointer-out report must never dismiss the tree"
    );
    assert!(
        fx.cascade.is_card_open(&fx.root.clone()),
        "the root card survives"
    );
    assert!(fx.cascade.is_card_open(&branch), "the open branch survives");
    assert_eq!(fx.cascade.open_cards().len(), 2);
    assert_eq!(fx.deepest(), Some(branch));
}

#[test]
fn test_a_deadline_after_the_pointer_left_still_closes_nothing() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));
    let branch = child_id("a");

    let _corridor_says_collapse = on_pointer_left(&mut fx.aim, at(50));

    // The wake-up that was already scheduled lands after the pointer is
    // gone. `Collapse` is not a switch, so nothing is acted on.
    let step = fx.deadline(at(200));

    assert!(!step.switched);
    assert_eq!(step.wake_after_ms, None);
    assert!(fx.cascade.is_card_open(&branch));
    assert_eq!(fx.cascade.open_cards().len(), 2);
}

// ---------------------------------------------------------------------------
// Guard liveness
// ---------------------------------------------------------------------------

#[test]
fn test_a_guard_whose_branch_closed_elsewhere_is_cleared_before_the_next_hover() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));

    // The keyboard's `Left` retires the branch without touching the
    // corridor, leaving the guard pointing at nothing.
    fx.cascade.close_deepest(&mut fx.focus);
    assert!(!guard_is_live(&fx.aim, &fx.cascade));
    assert_eq!(fx.aim.open_trigger(), Some(BRANCH_A));

    // The next hover must not be held for a window it never earned.
    let step = fx.enter_root(BRANCH_B, at(10));
    assert!(step.switched, "a stale guard must not delay a fresh open");
    assert_eq!(fx.deepest(), Some(child_id("b")));
    assert_eq!(step.wake_after_ms, None);
}

#[test]
fn test_a_live_guard_is_left_alone_by_the_liveness_check() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));

    assert!(guard_is_live(&fx.aim, &fx.cascade));
    let _ = fx.enter_root(BRANCH_A, at(5));
    assert!(guard_is_live(&fx.aim, &fx.cascade));
    assert_eq!(fx.aim.open_trigger(), Some(BRANCH_A));
}

// ---------------------------------------------------------------------------
// Target resolution
// ---------------------------------------------------------------------------

#[test]
fn test_the_root_cards_rows_are_trigger_candidates_and_everything_below_is_the_branch() {
    let fx = fixture();

    assert_eq!(
        target_for(&fx.cascade, &fx.root.clone(), 3),
        AimTarget::ParentRow { row: 3 }
    );
    assert_eq!(
        target_for(&fx.cascade, &child_id("a"), 0),
        AimTarget::ChildBranch,
        "one level down is inside the protected branch"
    );
}

// ---------------------------------------------------------------------------
// The wake-up task
// ---------------------------------------------------------------------------

#[test]
fn test_no_wake_task_is_built_when_nothing_is_held() {
    let root = MenuId::new(ROOT);

    assert!(
        wake_task(&root, None).is_none(),
        "a step with nothing outstanding must not schedule anything"
    );
    assert!(
        wake_task(&root, Some(grace_ms())).is_some(),
        "a held switch must schedule its own wake-up, or it only fires \
         when the user jiggles the mouse"
    );
}

#[test]
fn test_a_closed_tree_never_leaves_a_wake_owed() {
    let mut fx = fixture();
    fx.open_branch_a(at(0));
    fx.enter_root(BRANCH_B, at(0));
    assert!(fx.aim.corridor_remaining(at(0)) > 0);

    // The tree goes away with a hold outstanding.
    fx.cascade.close_tree(&mut fx.focus);
    assert!(!guard_is_live(&fx.aim, &fx.cascade));

    // A wake landing now must be inert rather than opening a branch
    // under a closed surface.
    let step = fx.deadline(at(grace_ms()));
    assert!(!step.switched);
    assert!(!fx.cascade.is_open());
    assert_eq!(step.wake_after_ms, None);
}
