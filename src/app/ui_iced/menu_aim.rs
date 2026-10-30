//! Driving the submenu aiming corridor from live row hover (`UIF-PRIN-008`,
//! `UIF-FMENU-002`).
//!
//! [`AimCorridor`](hoarfrost::surface::menu::aim::AimCorridor) is a pure
//! state machine that never reads a clock and never touches a cascade.
//! Something has to sit between it and the running application, and that
//! something is this module: it turns the raw pointer crossings the
//! [`MenuRow`](hoarfrost::surface::menu::view::MenuRow) widgets report
//! into corridor events, asks the corridor what it concludes, and drives
//! [`MenuCascade::hover_submenu_row`](hoarfrost::surface::menu::cascade::MenuCascade::hover_submenu_row)
//! when the answer permits a switch.
//!
//! # The split, and why it is split this way
//!
//! Every entry point here takes `now: TimeMs` as a parameter and is
//! therefore **pure with respect to time**. The reducer calls them with
//! `TimeMs::now()`; a test calls them with whatever millisecond it wants
//! to be standing at. That is the sanctioned shape for this codebase —
//! §2.6 forbids a *resolver* reading a wall clock, and a controller
//! reading one and passing the value down is the exit that clause was
//! written for. Nothing in here is reachable from a paint pass.
//!
//! # Three rules this module exists to enforce
//!
//! 1. **Only a submenu parent opens anything — but a non-parent sibling
//!    still closes something.** A plain action row, a heading, a
//!    separator and a disabled row are all reported to the corridor,
//!    which needs to see the pointer on a sibling to time the grace
//!    window. None of them may open a branch. §4.8.2 does not stop
//!    there: "moving to a sibling row on an ancestor card **retires the
//!    deeper child branch** after the `150ms` corridor check." So a
//!    non-parent sibling retires the branch the pointer walked away
//!    from, once the corridor matures. That is branch retirement, not
//!    dismissal — see rule 2 for the difference.
//! 2. **A pointer leaving never dismisses.** The corridor answers
//!    [`Collapse`](hoarfrost::surface::menu::aim::AimDecision::Collapse)
//!    when the pointer is outside everything, and that answer is
//!    *dropped* here. Dismissal belongs to the click-away blocker
//!    [`menu_surface::overlay`] mounts; a hover path that also closed
//!    menus would give one surface two owners, and every menu in the app
//!    would shut when the mouse swept past it.
//!    [`pointer_left_card`](hoarfrost::surface::menu::cascade::MenuCascade::pointer_left_card)
//!    says the same thing about itself, in the same words.
//! 3. **A held switch must mature without another event.** iced is
//!    event-driven; the corridor is not.
//!    [`wake_task`](crate::ui_iced::menu_aim::wake_task) is what stops a
//!    `Hold` from waiting on a mouse jiggle that may never come.
//!
//! # One corridor, one level
//!
//! The corridor tracks a single parent card's trigger rows by index, so
//! this module treats **the root card of the open tree** as the parent
//! and every card below it as
//! [`ChildBranch`](hoarfrost::surface::menu::aim::AimTarget::ChildBranch).
//! That is exactly right for the only menu in this application that has
//! submenus — the hamburger, which nests one level deep. A menu that
//! nested two levels would need one corridor per parent card, which is a
//! shape change, not a constant to tweak.
//!
//! Context menus are flat (`ContextGroup` yields separators, not child
//! menus), so they never reach any of this.
//!
//! Unit tests live in the sibling `menu_aim_tests.rs` sidecar.

use std::time::Duration;

use hoarfrost::focus::FocusState;
use hoarfrost::surface::menu::aim::{AimCorridor, AimDecision, AimTarget};
use hoarfrost::surface::menu::cascade::MenuCascade;
use hoarfrost::surface::menu::model::MenuId;
use hoarfrost::surface::tooltip::TimeMs;
use iced::Task;

use crate::ui_iced::message::{MenuMessage, Message};

/// What one step of the corridor decided.
///
/// Two facts, both there for the tests rather than for the reducer: the
/// reducer acts on the corridor's effect on the cascade either way, and
/// the wake-up is scheduled from the same field the corridor was asked
/// for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AimStep {
    /// A branch was opened or replaced by this step.
    pub switched: bool,
    /// Milliseconds until a held switch becomes due, `None` when there
    /// is nothing to wait for.
    pub wake_after_ms: Option<u64>,
}

impl AimStep {
    /// Nothing happened and nothing is owed.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            switched: false,
            wake_after_ms: None,
        }
    }

    /// A step that changed which branch is open.
    #[must_use]
    pub const fn switched(wake_after_ms: Option<u64>) -> Self {
        Self {
            switched: true,
            wake_after_ms,
        }
    }

    /// A step whose effect was *retiring* a branch rather than opening
    /// one — §4.8.2's sibling-crossing close.
    ///
    /// Distinct from [`switched`](Self::switched) so a test can tell the
    /// two rule firings apart instead of reading a bare `true` and
    /// guessing which half of the rule ran.
    #[must_use]
    pub const fn switched_from_retire(retired: bool) -> Self {
        Self {
            switched: retired,
            wake_after_ms: None,
        }
    }
}

/// Whether the branch the corridor is guarding is still mounted.
///
/// A guard whose branch has been closed by some *other* path — the
/// keyboard's `Left`, a click whose `ClosePolicy` retired the tree — is
/// a lie, and a lie here is expensive: the next hover on a different row
/// would be read as a sibling crossing and held for the full grace
/// window, or, worse, a stale trigger index landing on the *same* row of
/// a menu reopened later would answer
/// [`Preserve`](AimDecision::Preserve) and stop the branch opening at
/// all.
///
/// The check is card-agnostic on purpose. A
/// [`SubmenuTrigger`](hoarfrost::surface::menu::cascade::SubmenuTrigger)
/// records the parent card and the row that opened it,
/// so "some open card was opened by the guarded row of a card that is
/// itself still open" is exactly "the guard still means something".
#[must_use]
pub fn guard_is_live(aim: &AimCorridor, cascade: &MenuCascade) -> bool {
    let Some(trigger) = aim.open_trigger() else {
        // Nothing is guarded, so nothing is stale.
        return true;
    };

    cascade.open_cards().iter().any(|card| {
        card.trigger().is_some_and(|trigger_of_card| {
            trigger_of_card.row == trigger && cascade.is_card_open(&trigger_of_card.parent)
        })
    })
}

/// Where the corridor should place a report about `row` of `card`.
///
/// The root card's rows are the corridor's trigger candidates; anything
/// below the root is inside the branch the corridor is protecting, which
/// is the goal of the whole diagonal manoeuvre and therefore preserves
/// it outright (§4.9's no-flicker clause).
///
/// Purity: pure function of `(cascade, card, row)`.
#[must_use]
pub fn target_for(cascade: &MenuCascade, card: &MenuId, row: usize) -> AimTarget {
    if cascade
        .open_cards()
        .first()
        .is_some_and(|root| root.id() == card)
    {
        AimTarget::ParentRow { row }
    } else {
        AimTarget::ChildBranch
    }
}

/// The pointer crossed **into** `row` of `card`, as of `now`.
///
/// The whole hover path in one call. What it does, in order:
///
/// * drops a guard whose branch is no longer mounted;
/// * reports the pointer's position to the corridor, for **every** row
///   kind — the corridor needs to see a sibling crossing to time it,
///   and a heading is a sibling just as a trigger is;
/// * opens a branch **only** if the row is a
///   [`is_submenu_parent`](hoarfrost::surface::menu::model::Row::is_submenu_parent)
///   *and* the corridor's decision permits it, which is what
///   [`MenuCascade::hover_submenu_row`] encodes;
/// * reports how long the corridor still wants before it would allow a
///   switch, so the caller can schedule the wake-up.
///
/// Note that no `branch_opened` call appears here even though a switch
/// needs one: `hover_submenu_row` already records the new trigger when
/// the open landed and clears the corridor when it did not
/// (`cascade.rs:802-808`), which is how the two are kept from drifting.
/// Calling it again from here would be a second owner of the same fact.
pub fn on_row_entered(
    cascade: &mut MenuCascade,
    aim: &mut AimCorridor,
    card: &MenuId,
    row: usize,
    now: TimeMs,
    focus: &mut FocusState,
) -> AimStep {
    if !guard_is_live(aim, cascade) {
        aim.branch_closed();
    }

    let Some(is_submenu_parent) = cascade
        .card_menu(card)
        .and_then(|menu| menu.row(row))
        .map(|row| row.is_submenu_parent())
    else {
        return AimStep::none();
    };

    aim.pointer_enter(target_for(cascade, card, row), now);

    if !is_submenu_parent {
        // Seen, and — once the corridor says so — *acted on*. §4.8.2 is
        // explicit that "moving to a sibling row **on an ancestor
        // card** retires the deeper child branch after the `150ms`
        // corridor check", and a sibling that happens not to be a
        // submenu parent is still a sibling. Leaving the branch open
        // here would mean a submenu only ever closed by clicking.
        //
        // The ancestor-card qualifier is load-bearing, not pedantry. A
        // row *below* the root is inside the branch the user is aiming
        // at, and retiring on that would make arriving at the target be
        // what closes it — the exact opposite of the diagonal manoeuvre
        // this corridor exists to serve. `target_for` already draws that
        // line, so the retire keys off it rather than re-deriving depth.
        //
        // This is *branch retirement*, not dismissal, which is why rule
        // 2 does not forbid it: the menu itself stays open and
        // click-away still owns that. What goes is the child card the
        // pointer walked away from.
        //
        // The retire is corridor-gated rather than immediate, which is
        // what "after the 150ms corridor check" means: the deadline is
        // owed, and `on_deadline` performs the retirement once the
        // answer has matured to `Switch`. If the corridor has already
        // elapsed there is nothing left to wait on and it retires now.
        let on_ancestor = matches!(target_for(cascade, card, row), AimTarget::ParentRow { .. });
        let held = remaining(aim, now);
        if on_ancestor && held.is_none() && retire_deeper_branch(cascade, aim, focus) {
            return AimStep::switched_from_retire(true);
        }
        return AimStep {
            switched: false,
            wake_after_ms: held,
        };
    }

    let outcome = cascade.hover_submenu_row(aim, card, row, now, focus);
    if outcome.opened().is_empty() {
        AimStep {
            switched: false,
            wake_after_ms: remaining(aim, now),
        }
    } else {
        AimStep::switched(remaining(aim, now))
    }
}

/// The pointer crossed **out of** `card`, as of `now`.
///
/// Returns the corridor's advisory "this branch would close" answer so a
/// test can pin that the answer was given **and** that nothing acted on
/// it. The reducer drops it. See the module docs, rule 2.
#[must_use = "the answer is advisory and must not dismiss: see the module docs"]
pub fn on_pointer_left(aim: &mut AimCorridor, now: TimeMs) -> bool {
    MenuCascade::pointer_left_card(aim, now)
}

/// The corridor's deadline fired for `card`, as of `now`.
///
/// Re-reads the decision rather than assuming the hold matured: the
/// pointer may have moved back onto the trigger (
/// [`Preserve`](AimDecision::Preserve)), left entirely (
/// [`Collapse`](AimDecision::Collapse)), or the menu may have closed
/// underneath the timer. Only a
/// [`Switch`](AimDecision::Switch) is acted on, and a hold that has
/// *still* not elapsed re-arms itself through the returned
/// `wake_after_ms` rather than being silently dropped.
pub fn on_deadline(
    cascade: &mut MenuCascade,
    aim: &mut AimCorridor,
    card: &MenuId,
    now: TimeMs,
    focus: &mut FocusState,
) -> AimStep {
    if !guard_is_live(aim, cascade) {
        aim.branch_closed();
    }

    match aim.decision(now) {
        AimDecision::Switch { row } => {
            // A matured switch has two shapes and they must not be
            // conflated. Switching *onto* a submenu parent opens that
            // branch. Switching onto anything else — a plain action row,
            // a heading, a separator, a disabled row — retires the
            // branch the pointer walked away from (§4.8.2). Asking
            // `hover_submenu_row` for the second shape would be asking a
            // submenu opener to perform a close, and it has nothing to
            // open behind a non-submenu row.
            let onto_submenu = cascade
                .card_menu(card)
                .and_then(|menu| menu.row(row))
                .is_some_and(|row| row.is_submenu_parent());

            // Same ancestor-card gate as `on_row_entered`: a deadline
            // whose card sits below the root is standing inside the
            // branch, and nothing retires from there.
            let on_ancestor = matches!(target_for(cascade, card, row), AimTarget::ParentRow { .. });

            if !onto_submenu {
                if !on_ancestor {
                    return AimStep::none();
                }
                return AimStep::switched_from_retire(retire_deeper_branch(cascade, aim, focus));
            }

            let outcome = cascade.hover_submenu_row(aim, card, row, now, focus);
            if outcome.opened().is_empty() {
                AimStep {
                    switched: false,
                    wake_after_ms: remaining(aim, now),
                }
            } else {
                AimStep::switched(remaining(aim, now))
            }
        }
        AimDecision::Hold { remaining_ms, .. } => AimStep {
            switched: false,
            wake_after_ms: Some(remaining_ms),
        },
        AimDecision::Idle | AimDecision::Preserve | AimDecision::Collapse => AimStep::none(),
    }
}

/// Retires every card below the root, clearing the corridor with it.
///
/// "Retires the deeper child branch" (§4.8.2) means the whole subtree
/// under the card the pointer is standing on, not merely the last card,
/// so this drains to the root rather than closing one level. For the
/// hamburger — the only menu here with submenus, and one level deep —
/// that is a single close; writing it as a drain keeps the rule honest if
/// a deeper menu ever appears.
///
/// The corridor is cleared unconditionally on a real retire. A guard left
/// pointing at a row whose branch is gone is exactly the stale-guard
/// failure `guard_is_live` exists to catch, and here it would be caught
/// one hover late.
///
/// Returns whether anything was actually retired, so a caller can tell a
/// rule-firing no-op from a real change.
fn retire_deeper_branch(
    cascade: &mut MenuCascade,
    aim: &mut AimCorridor,
    focus: &mut FocusState,
) -> bool {
    if cascade.depth() <= 1 {
        // Nothing below the root to retire.
        return false;
    }

    let mut retired = false;
    while cascade.depth() > 1 {
        cascade.close_deepest(focus);
        retired = true;
    }
    if retired {
        aim.branch_closed();
    }
    retired
}

/// The task that wakes the corridor up, exactly `ms` late and no later.
///
/// # Why this sleeps on a worker thread
///
/// iced 0.14 has **no** `Task::later`. The delayed-command constructor
/// older releases shipped is gone, and the timer that would replace it —
/// `iced::time::every` — lives in
/// `iced_futures::backend::native::{tokio,smol}::time`, neither of
/// which this application enables: `iced`'s default `thread-pool`
/// backend exports an **empty** `time` module
/// (`iced_futures-0.14.0/src/backend/native/thread_pool.rs:20`). So the
/// only way to get a message at a future instant, without adding a
/// dependency or swapping the app's async backend out from under every
/// existing subscription, is to sleep on the worker the task runs on.
///
/// The cost is bounded: one sleep per hover crossing, each at most
/// [`grace_ms`](hoarfrost::surface::menu::aim::grace_ms) (150 ms), on a
/// `futures` pool sized to the CPU count — and it is never the UI thread
/// that sleeps, which is what [FMD-060] and [FMD-061] actually require.
/// A wake that arrives after its reason has gone is inert: `on_deadline`
/// re-reads the corridor and does nothing.
#[must_use]
pub fn wake_task(card: &MenuId, wake_after_ms: Option<u64>) -> Option<Task<Message>> {
    let millis = wake_after_ms?;
    let card = card.clone();

    Some(Task::future(async move {
        std::thread::sleep(Duration::from_millis(millis));
        Message::Menu(MenuMessage::AimExpired { card })
    }))
}

/// Milliseconds the corridor still wants, read from the corridor itself so
/// the sleep is never longer than the rule requires and never shorter
/// than it dares.
fn remaining(aim: &AimCorridor, now: TimeMs) -> Option<u64> {
    match aim.corridor_remaining(now) {
        0 => None,
        millis => Some(millis),
    }
}

#[cfg(test)]
#[path = "menu_aim_tests.rs"]
mod tests;
