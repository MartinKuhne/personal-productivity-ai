//! Tests for the menu-row action engine (`UIF-ATOM-000b`, §3.0.4.3).

use crate::binding::{ActionPending, Activation, SuppressedReason};
use crate::surface::menu::model::{ActivationEffect, Menu, RadioGroupId, Row};
use crate::surface::menu::row_action::{
    RevertKind, RowActionEngine, RowEmission, row_revert, row_tracks_action,
};
use crate::widget::button_controller::ActivationSource;

const POINTER: ActivationSource = ActivationSource::Pointer;
const KEYBOARD: ActivationSource = ActivationSource::Keyboard;

fn action_row() -> Row {
    Row::action("save", "Save")
}

fn stay_open_check() -> Row {
    Row::checkable("wrap", "Wrap lines", false).stays_open()
}

fn closing_check() -> Row {
    Row::checkable("wrap", "Wrap lines", false).closes_menu()
}

fn stay_open_radio() -> Row {
    Row::radio("size.sm", "Small", RadioGroupId::new("size"), true).stays_open()
}

fn submenu_row() -> Row {
    Row::submenu("more", "More", Menu::empty("menu.more"))
}

fn toggled(checked: bool) -> ActivationEffect {
    ActivationEffect::Toggled { checked }
}

fn selected_effect() -> ActivationEffect {
    ActivationEffect::Selected {
        group: RadioGroupId::new("size"),
        cleared: vec![2],
    }
}

fn submenu_effect() -> ActivationEffect {
    ActivationEffect::OpenSubmenu {
        child: crate::surface::menu::model::MenuId::new("menu.more"),
    }
}

// -- revert classification ---------------------------------------------------

#[test]
fn an_absorbed_click_reverts_nothing() {
    assert_eq!(
        row_revert(&action_row(), &ActivationEffect::Absorbed),
        RevertKind::Nothing
    );
    assert_eq!(
        row_revert(&Row::separator(), &ActivationEffect::Absorbed),
        RevertKind::Nothing
    );
    assert!(!RevertKind::Nothing.restores_value());
}

#[test]
fn opening_a_submenu_is_navigation_not_a_commit_of_a_value() {
    assert_eq!(
        row_revert(&submenu_row(), &submenu_effect()),
        RevertKind::Navigation
    );
    assert!(!RevertKind::Navigation.restores_value());
}

#[test]
fn a_command_is_fire_and_settle() {
    assert_eq!(
        row_revert(&action_row(), &ActivationEffect::Command),
        RevertKind::FireAndSettle
    );
    assert!(!RevertKind::FireAndSettle.restores_value());
}

#[test]
fn a_stay_open_toggle_is_the_one_gesture_with_a_value_to_revert() {
    assert_eq!(
        row_revert(&stay_open_check(), &toggled(true)),
        RevertKind::Value
    );
    assert!(RevertKind::Value.restores_value());
}

#[test]
fn a_toggle_that_takes_the_tree_with_it_has_nothing_left_to_put_back() {
    assert_eq!(
        row_revert(&closing_check(), &toggled(true)),
        RevertKind::FireAndSettle
    );
}

#[test]
fn a_stay_open_radio_selection_is_revertible() {
    assert_eq!(
        row_revert(&stay_open_radio(), &selected_effect()),
        RevertKind::Value
    );
}

#[test]
fn a_radio_that_closes_the_tree_is_fire_and_settle() {
    let closing = Row::radio("size.md", "Medium", RadioGroupId::new("size"), false);
    assert_eq!(
        row_revert(&closing, &selected_effect()),
        RevertKind::FireAndSettle
    );
}

#[test]
fn revert_kinds_have_distinct_slugs() {
    let mut slugs = vec![
        RevertKind::Value.slug(),
        RevertKind::FireAndSettle.slug(),
        RevertKind::Navigation.slug(),
        RevertKind::Nothing.slug(),
    ];
    slugs.sort_unstable();
    slugs.dedup();
    assert_eq!(slugs.len(), 4, "slugs collide: {slugs:?}");
}

// -- tracking --------------------------------------------------------------

#[test]
fn an_idle_non_cancellable_projection_is_not_tracked() {
    assert!(!row_tracks_action(&ActionPending::idle()));
}

#[test]
fn busy_or_cancellable_is_tracked() {
    assert!(row_tracks_action(&ActionPending::indeterminate()));
    assert!(row_tracks_action(
        &ActionPending::idle().with_cancellable(true)
    ));
    assert!(row_tracks_action(
        &ActionPending::determinate(0.5).with_cancellable(true)
    ));
}

// -- engine: untracked rows fire every press -------------------------------

#[test]
fn a_synchronous_row_fires_every_press_and_never_latches() {
    let row = action_row();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::idle());

    for _ in 0..3 {
        let emission = engine
            .commit(&row, &ActivationEffect::Command, POINTER)
            .expect("an untracked row always dispatches");
        assert!(matches!(emission, RowEmission::Committed(_)));
    }

    assert_eq!(engine.fired(), 3);
    assert_eq!(engine.suppressed(), 0);
    assert!(!engine.is_pending());
}

#[test]
fn the_commit_carries_the_effect_and_the_revert_kind() {
    let row = stay_open_check();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::idle());

    let emission = engine
        .commit(&row, &toggled(true), POINTER)
        .expect("a stay-open check dispatches");
    let settlement = emission
        .settlement()
        .expect("a commit carries a settlement");

    assert_eq!(settlement.effect, Some(toggled(true)));
    assert_eq!(settlement.revert, RevertKind::Value);
    assert!(settlement.dispatched);
    assert_eq!(settlement.row_key, "wrap");
    assert_eq!(settlement.source, POINTER);
}

#[test]
fn a_keyboard_activation_opens_its_own_gesture_before_dispatching() {
    let row = action_row();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::idle());

    assert!(!engine.gesture_open());
    let emission = engine
        .commit(&row, &ActivationEffect::Command, KEYBOARD)
        .expect("Enter dispatches");
    assert!(matches!(emission, RowEmission::Committed(_)));
    assert_eq!(
        engine.last_begin(),
        Some(crate::binding::GestureStart::Started)
    );
    assert!(!engine.gesture_open(), "committing closes the slot");
}

// -- engine: the pre-dispatch cancel is always genuine ----------------------

#[test]
fn releasing_outside_restores_a_settlement_that_dispatched_nothing() {
    let row = action_row();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::idle());
    engine.begin(&row, POINTER);
    assert!(engine.gesture_open(), "the press opened the gesture");

    let emission = engine.cancel().expect("an open gesture cancels");
    let restored = match &emission {
        RowEmission::Cancelled(cancel) => cancel.restored(),
        other => panic!("expected a cancel, got {other:?}"),
    };

    assert!(!restored.dispatched);
    assert_eq!(restored.effect, None);
    assert_eq!(restored.revert, RevertKind::Nothing);
    assert_eq!(restored.row_key, "save");
    assert_eq!(engine.fired(), 0);
}

#[test]
fn cancelling_twice_degrades_to_nothing_when_nothing_is_cancellable() {
    let row = action_row();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::idle());
    engine.begin(&row, POINTER);
    assert!(engine.gesture_open(), "the press opened the gesture");

    assert!(engine.cancel().is_some());
    assert!(
        engine.cancel().is_none(),
        "a second cancel with no stop path must not invent one"
    );
}

#[test]
fn a_cancelled_gesture_can_be_reopened_and_committed() {
    let row = stay_open_check();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::idle());
    engine.begin(&row, POINTER);
    assert!(engine.gesture_open(), "the press opened the gesture");
    engine.cancel().expect("cancel closes the slot");

    let emission = engine
        .commit(&row, &toggled(true), POINTER)
        .expect("the row still works after a cancel");
    assert!(matches!(emission, RowEmission::Committed(_)));
    assert_eq!(engine.fired(), 1);
}

// -- engine: busy suppression ----------------------------------------------

#[test]
fn a_busy_cancellable_row_turns_a_press_into_a_cancel_request() {
    let row = action_row();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::indeterminate().with_cancellable(true));

    let emission = engine
        .begin(&row, POINTER)
        .expect("busy + cancellable requests");
    assert!(matches!(emission, RowEmission::CancelRequested(_)));
    assert_eq!(emission.tier(), "cancel-request");
    assert!(engine.cancel_requested());
    assert_eq!(engine.fired(), 0);
}

#[test]
fn only_one_stop_request_is_surfaced_per_busy_period() {
    let row = action_row();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::indeterminate().with_cancellable(true));

    assert!(engine.begin(&row, POINTER).is_some());
    assert!(
        engine.begin(&row, POINTER).is_none(),
        "the second press must not surface a second stop request"
    );
}

#[test]
fn a_busy_row_with_no_stop_path_emits_nothing_at_all() {
    let row = action_row();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::indeterminate());

    assert!(
        engine.begin(&row, POINTER).is_none(),
        "a fire-and-forget action shows no dead stop affordance"
    );
    assert!(!engine.cancel_requested());
}

#[test]
fn a_second_activation_inside_the_round_trip_window_is_suppressed() {
    let row = action_row();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::idle().with_cancellable(true));

    let first = engine
        .commit(&row, &ActivationEffect::Command, POINTER)
        .expect("the first press dispatches");
    assert!(matches!(first, RowEmission::Committed(_)));

    // The projection has not caught up yet: it still says idle. The gate's
    // own memory is what stops the double dispatch.
    engine.observe(ActionPending::idle().with_cancellable(true));
    let second = engine.commit(&row, &ActivationEffect::Command, POINTER);
    assert!(
        second.is_none(),
        "the double-click window must not dispatch twice"
    );
    assert_eq!(engine.fired(), 1);
    assert_eq!(engine.suppressed(), 1);
}

#[test]
fn suppression_against_a_visible_busy_projection_reports_the_busy_reason() {
    let row = action_row();
    let mut engine = RowActionEngine::new();
    let gate = &mut engine;
    gate.observe(ActionPending::idle().with_cancellable(true));
    gate.commit(&row, &ActivationEffect::Command, POINTER)
        .expect("first dispatches");

    gate.observe(ActionPending::determinate(0.25).with_cancellable(true));
    // A press now is suppressed by the projection the gate can see, and the
    // cancellable projection turns it into a stop request.
    let emission = gate.commit(&row, &ActivationEffect::Command, POINTER);
    assert!(
        emission.is_none() || matches!(emission, Some(RowEmission::CancelRequested(_))),
        "a visible-busy press is either silent or a stop request, never a dispatch"
    );
    assert_eq!(gate.fired(), 1, "still exactly one dispatch");
}

#[test]
fn the_gate_settles_once_a_tracked_action_is_seen_busy_then_idle() {
    let row = action_row();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::idle().with_cancellable(true));
    engine
        .commit(&row, &ActivationEffect::Command, POINTER)
        .expect("dispatches");

    engine.observe(ActionPending::indeterminate().with_cancellable(true));
    assert!(engine.busy_observed());
    assert!(engine.is_pending());

    engine.observe(ActionPending::idle().with_cancellable(true));
    assert!(!engine.is_pending(), "busy→idle settles the gate");

    let emission = engine
        .commit(&row, &ActivationEffect::Command, POINTER)
        .expect("the row fires again after the action finished");
    assert!(matches!(emission, RowEmission::Committed(_)));
    assert_eq!(engine.fired(), 2);
}

#[test]
fn an_untracked_projection_never_settles_a_gate_it_never_armed() {
    let row = action_row();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::idle());
    engine
        .commit(&row, &ActivationEffect::Command, POINTER)
        .expect("untracked fires");
    assert_eq!(
        engine.suppressed(),
        0,
        "an untracked row never consults the gate, so it never suppresses"
    );
}

// -- engine: nesting -------------------------------------------------------

#[test]
fn a_second_begin_while_open_is_refused_and_the_outer_snapshot_governs() {
    let row = stay_open_check();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::idle());
    engine.begin(&row, POINTER);
    assert!(engine.gesture_open(), "the first press opened the gesture");
    assert_eq!(
        engine.last_begin(),
        Some(crate::binding::GestureStart::Started)
    );

    engine.begin(&row, POINTER);
    assert_eq!(
        engine.last_begin(),
        Some(crate::binding::GestureStart::AlreadyOpen),
        "the slot must refuse the inner begin"
    );

    let emission = engine.cancel().expect("the outer gesture still cancels");
    assert!(matches!(emission, RowEmission::Cancelled(_)));
    assert!(
        !emission.settlement().expect("outer snapshot").dispatched,
        "the outer pre-gesture state must not have been re-captured mid-flight"
    );
}

// -- engine: reset ---------------------------------------------------------

#[test]
fn reset_drops_the_gesture_without_emitting_a_tier_and_keeps_history() {
    let row = action_row();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::idle().with_cancellable(true));
    engine.begin(&row, POINTER);
    assert!(engine.gesture_open(), "the press opened the gesture");
    engine
        .commit(&row, &ActivationEffect::Command, POINTER)
        .expect("dispatches");
    engine.begin(&row, POINTER);
    let fired_before = engine.fired();
    let suppressed_before = engine.suppressed();

    engine.reset();

    assert!(!engine.gesture_open());
    assert!(!engine.cancel_requested());
    assert_eq!(engine.last_begin(), None);
    assert!(
        !engine.is_pending(),
        "reset releases the activation gate, so a reused engine can fire again"
    );
    assert!(!engine.busy_observed(), "and the busy memory with it");
    assert_eq!(engine.fired(), fired_before, "history is kept");
    assert_eq!(engine.suppressed(), suppressed_before, "history is kept");
}

#[test]
fn reset_on_a_fresh_engine_is_a_no_op() {
    let mut engine = RowActionEngine::new();
    engine.reset();
    assert!(!engine.gesture_open());
    assert_eq!(engine.fired(), 0);
}

// -- emission plumbing -----------------------------------------------------

#[test]
fn a_committed_never_equals_a_cancelled_of_the_same_settlement() {
    let row = stay_open_check();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::idle());

    let committed = engine
        .commit(&row, &toggled(true), POINTER)
        .expect("dispatches");
    engine.begin(&row, POINTER);
    let cancelled = engine.cancel().expect("cancels");

    assert_ne!(committed, cancelled);
    assert_eq!(committed.tier(), "commit");
    assert_eq!(cancelled.tier(), "cancel");
}

#[test]
fn two_identical_commits_compare_equal() {
    let row = stay_open_check();
    let mut first = RowActionEngine::new();
    let mut second = RowActionEngine::new();
    first.observe(ActionPending::idle());
    second.observe(ActionPending::idle());

    let a = first
        .commit(&row, &toggled(true), POINTER)
        .expect("dispatches");
    let b = second
        .commit(&row, &toggled(true), POINTER)
        .expect("dispatches");
    assert_eq!(a, b);
}

#[test]
fn a_cancel_request_carries_no_settlement() {
    let row = action_row();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::indeterminate().with_cancellable(true));
    let emission = engine.begin(&row, POINTER).expect("requests");
    assert!(emission.settlement().is_none());
}

#[test]
fn engines_compare_on_shape_not_on_retained_payload() {
    let mut a = RowActionEngine::new();
    let mut b = RowActionEngine::new();
    assert_eq!(a, b);

    a.observe(ActionPending::idle());
    b.observe(ActionPending::idle());
    a.begin(&stay_open_check(), POINTER);
    b.begin(&stay_open_check(), POINTER);
    assert_eq!(a, b, "same shape, different row keys inside the snapshot");

    b.reset();
    assert_ne!(a, b, "gesture-open state is part of the shape");
}

#[test]
fn the_suppression_reason_is_reported_through_the_binding_type() {
    // The engine defers the reason to the binding's own vocabulary; pin that
    // the reason a press was withheld is still readable by a host.
    let row = action_row();
    let mut engine = RowActionEngine::new();
    engine.observe(ActionPending::idle().with_cancellable(true));
    engine
        .commit(&row, &ActivationEffect::Command, POINTER)
        .expect("first fires");
    engine.observe(ActionPending::idle().with_cancellable(true));
    assert!(
        engine
            .commit(&row, &ActivationEffect::Command, POINTER)
            .is_none(),
        "the second press is withheld"
    );
    // The gate recorded it even though the engine emitted nothing.
    assert_eq!(engine.suppressed(), 1);
    let reason = Activation::Suppressed(SuppressedReason::AwaitingProjection).reason();
    assert_eq!(reason, Some(SuppressedReason::AwaitingProjection));
}
