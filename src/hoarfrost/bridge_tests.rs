//! Proofs about the model → router bridge: what it pushes, what it refuses
//! to push, and the mechanical guard that keeps iced's own traversal out of
//! it (`UIF-PRIN-005`, `INC-10` constraints 1 and 2).

use crate::bridge::Direction;
use crate::bridge::focus_operation;
use crate::bridge::focus_task;
use crate::bridge::tab_operation;
use crate::bridge::tab_task;
use crate::bridge::widget_id;
use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::focus::GroupId;
use crate::test_helpers::Harness;
use crate::test_helpers::assert_agree;
use crate::test_helpers::focusable_tree;
use crate::test_helpers::iced_id;
use crate::test_helpers::mixed_focusable_tree;
use crate::test_helpers::model_with;
use crate::test_helpers::opt_ids;
use crate::test_helpers::wrapped;

use iced::Task;
use iced::advanced::widget::Operation;
use iced::widget::column;

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

#[test]
fn widget_id_is_the_focus_id_s_own_string() {
    let focus_id = FocusId::new("dialog.save");

    assert_eq!(widget_id(&focus_id), iced_id("dialog.save"));
    assert_eq!(
        widget_id(&FocusId::new("dialog.save")),
        widget_id(&focus_id),
        "two `FocusId`s with the same string must address the same router slot, \
         or a rebuilt widget could not re-claim its focus"
    );
    assert_ne!(
        widget_id(&FocusId::new("dialog.save")),
        widget_id(&FocusId::new("dialog.cancel")),
        "distinct ids must not collide onto one address"
    );
}

// ---------------------------------------------------------------------------
// Projection
// ---------------------------------------------------------------------------

#[test]
fn the_bridge_projects_a_focused_model_onto_the_router() {
    let mut harness = Harness::headless(|| focusable_tree(&["a", "b", "c"]));
    let (group, mut model) = model_with("panel", vec![member("a"), member("b"), member("c")]);

    assert_eq!(
        harness.focused(),
        None,
        "registration alone focuses nothing, and neither does a model that has \
         not been pushed"
    );

    assert!(model.focus_first_in_group(&group).is_some());
    harness.sync(&model);

    assert_agree(&mut harness, &model, "pushing the model's first member");
    assert!(harness.is_focused(&iced_id("a")));
    assert!(!harness.is_focused(&iced_id("b")));
}

#[test]
fn the_bridge_pushes_unfocus_when_the_model_names_nothing() {
    // "Nothing is focused" has to be *said*. A silent bridge would leave
    // whatever the router was already showing lit, and the screen would
    // disagree with the model.
    let mut harness = Harness::headless(|| focusable_tree(&["a", "b"]));
    let (group, mut model) = model_with("panel", vec![member("a"), member("b")]);

    model.focus_first_in_group(&group);
    harness.sync(&model);
    assert_eq!(harness.focused(), Some(iced_id("a")));

    assert_eq!(model.clear(), Some(FocusId::new("a")));
    harness.sync(&model);

    assert_eq!(
        harness.focused(),
        None,
        "the router must stop showing `a` the moment the model stops naming it"
    );
}

#[test]
fn the_bridge_never_focuses_an_id_the_model_does_not_name() {
    // The empty-model leg of the same rule: the projection of "nothing"
    // focuses nothing, rather than defaulting to the first widget.
    let mut harness = Harness::headless(|| focusable_tree(&["a", "b"]));
    let model = FocusState::new();

    harness.sync(&model);

    assert_eq!(harness.focused(), None);
    assert!(
        !harness.is_focused(&iced_id("a")),
        "an empty model must not fall through to the first registered widget"
    );
}

// ---------------------------------------------------------------------------
// Tab, forward and back
// ---------------------------------------------------------------------------

#[test]
fn tab_forward_moves_the_router_to_the_next_widget() {
    let mut harness = Harness::headless(|| focusable_tree(&["a", "b", "c"]));
    let (_group, mut model) = model_with("panel", vec![member("a"), member("b"), member("c")]);

    for expected in ["a", "b", "c"] {
        let landed = harness.tab(&mut model, Direction::Forward);

        assert_eq!(landed, Some(FocusId::new(expected)));
        assert_agree(&mut harness, &model, &format!("Tab forward to {expected}"));
    }
}

#[test]
fn tab_backward_moves_the_router_to_the_previous_widget() {
    let mut harness = Harness::headless(|| focusable_tree(&["a", "b", "c"]));
    let (_group, mut model) = model_with("panel", vec![member("a"), member("b"), member("c")]);

    for expected in ["c", "b", "a"] {
        let landed = harness.tab(&mut model, Direction::Backward);

        assert_eq!(landed, Some(FocusId::new(expected)));
        assert_agree(
            &mut harness,
            &model,
            &format!("Shift+Tab backward to {expected}"),
        );
    }
}

#[test]
fn tab_wraps_past_the_end_where_iced_would_have_lost_focus() {
    // The whole reason the bridge exists, proved against a real tree: at
    // the last control iced's own traversal drops focus, the model wraps,
    // and pushing the model's answer is what keeps the keyboard alive.
    let mut harness = Harness::headless(|| focusable_tree(&["a", "b", "c"]));
    let (group, mut model) = model_with("panel", vec![member("a"), member("b"), member("c")]);

    model.focus_last_in_group(&group);
    harness.sync(&model);
    assert_eq!(harness.focused(), Some(iced_id("c")));

    harness.iced_focus_next();
    assert_eq!(
        harness.focused(),
        None,
        "iced's own traversal drops focus here — which is why the bridge is \
         push-from-the-model, never delegate-to-the-router"
    );

    let landed = harness.tab(&mut model, Direction::Forward);
    assert_eq!(landed, Some(FocusId::new("a")), "the model wraps");
    assert_eq!(
        harness.focused(),
        Some(iced_id("a")),
        "and the push lands on the first widget rather than nowhere"
    );
}

#[test]
fn tab_wraps_before_the_start_where_iced_would_have_lost_focus() {
    let mut harness = Harness::headless(|| focusable_tree(&["a", "b", "c"]));
    let (group, mut model) = model_with("panel", vec![member("a"), member("b"), member("c")]);

    model.focus_first_in_group(&group);
    harness.sync(&model);
    assert_eq!(harness.focused(), Some(iced_id("a")));

    harness.iced_focus_previous();
    assert_eq!(
        harness.focused(),
        None,
        "Shift+Tab before the first widget loses focus in iced rather than \
         wrapping to the last"
    );

    let landed = harness.tab(&mut model, Direction::Backward);
    assert_eq!(landed, Some(FocusId::new("c")));
    assert_eq!(harness.focused(), Some(iced_id("c")));
}

#[test]
fn tab_skips_what_the_model_skips_and_the_router_follows() {
    let mut harness =
        Harness::headless(|| mixed_focusable_tree(&[("a", true), ("b", false), ("c", true)]));
    let (_group, mut model) = model_with(
        "menu",
        vec![member("a"), FocusMember::disabled("b"), member("c")],
    );

    let landed = harness.tab(&mut model, Direction::Forward);
    assert_eq!(landed, Some(FocusId::new("a")));

    let landed = harness.tab(&mut model, Direction::Forward);
    assert_eq!(
        landed,
        Some(FocusId::new("c")),
        "both layers must step over the disabled row the same way"
    );
    assert!(
        !harness.is_focused(&iced_id("b")),
        "a disabled widget must never be the bridge's answer"
    );
}

// ---------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------

#[test]
fn a_tab_the_model_refuses_leaves_the_router_exactly_where_it_was() {
    // A trap around a group that has not mounted yet makes the active
    // scope empty, so `focus_next` returns `None` and the model is
    // unchanged. The projection of an unchanged model must be unchanged:
    // the router keeps showing `a` rather than being cleared.
    let mut harness = Harness::headless(|| focusable_tree(&["a", "b"]));
    let (group, mut model) = model_with("panel", vec![member("a"), member("b")]);

    model.focus_first_in_group(&group);
    harness.sync(&model);
    assert_eq!(harness.focused(), Some(iced_id("a")));

    model.push_trap(GroupId::new("not_mounted_yet"));
    let landed = harness.tab(&mut model, Direction::Forward);

    assert_eq!(
        landed.as_ref(),
        Some(&FocusId::new("a")),
        "the empty scope has nowhere to go, so the model must still be on `a` \
         — the harness reports what the bridge actually pushed, never what a \
         step would have reached"
    );
    assert_eq!(
        harness.focused(),
        Some(iced_id("a")),
        "a refused step must not move the router, and must not clear it either"
    );
    assert_eq!(
        harness.registered(),
        opt_ids(&["a", "b"]),
        "a refused step must not disturb what the tree registered either"
    );
}

#[test]
fn tab_emits_no_focus_when_nothing_is_focusable() {
    // Every member disabled: the model has no answer, so the bridge must
    // not put focus anywhere — not even on the one widget the tree happily
    // registered.
    let mut harness = Harness::headless(|| focusable_tree(&["a"]));
    let (_group, mut model) = model_with("menu", vec![FocusMember::disabled("a")]);

    let landed = harness.tab(&mut model, Direction::Forward);

    assert_eq!(landed, None);
    assert_eq!(harness.focused(), None);
    assert!(
        !harness.is_focused(&iced_id("a")),
        "a widget the model refuses must not be focused just because it exists"
    );
}

#[test]
fn the_bridge_pushes_only_what_the_active_trap_allows() {
    // Confinement is the model's decision and the bridge carries it
    // verbatim: with a trap on `modal`, no amount of Tabbing reaches the
    // background group (`UIF-MOD-001`).
    let mut harness = Harness::headless(|| {
        column(vec![
            wrapped("base_a", true),
            wrapped("base_b", true),
            wrapped("modal_a", true),
            wrapped("modal_b", true),
        ])
        .into()
    });

    let base = GroupId::new("base");
    let modal = GroupId::new("modal");
    let mut model = FocusState::new();
    model.register_group(base.clone(), vec![member("base_a"), member("base_b")]);
    model.register_group(modal.clone(), vec![member("modal_a"), member("modal_b")]);

    model.push_trap(modal.clone());
    assert!(model.focus_first_in_group(&modal).is_some());
    harness.sync(&model);
    assert_eq!(harness.focused(), Some(iced_id("modal_a")));

    for _ in 0..4 {
        let landed = harness.tab(&mut model, Direction::Forward);
        assert!(
            landed
                .as_ref()
                .is_some_and(|id| id.as_str().starts_with("modal_")),
            "a trapped Tab must never land on the background, got {landed:?}"
        );
        assert_agree(&mut harness, &model, "a trapped Tab");
    }

    assert!(
        !model.focus(&FocusId::new("base_a")),
        "the model must reject a focus outside the trap"
    );
    let before_rejection = harness.focused();
    harness.sync(&model);
    assert_eq!(
        harness.focused(),
        before_rejection,
        "a rejected focus must not move the router either — the trap has no \
         hole for a programmatic path to fall through"
    );
    assert!(
        harness
            .focused()
            .is_some_and(|id| { id == iced_id("modal_a") || id == iced_id("modal_b") }),
        "and the router must still be inside the trap"
    );
}

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------

#[test]
fn the_bridge_tasks_are_send_so_the_runtime_can_carry_them() {
    fn assert_send<T: Send>(_: &T) {}

    let (group, mut model) = model_with("panel", vec![member("a")]);
    model.focus_first_in_group(&group);

    let focused: Task<()> = focus_task(&model);
    let empty: Task<()> = focus_task(&FocusState::new());
    let forward: Task<()> = tab_task(&mut model, Direction::Forward);
    let backward: Task<()> = tab_task(&mut model, Direction::Backward);

    assert_send(&focused);
    assert_send(&empty);
    assert_send(&forward);
    assert_send(&backward);
}

#[test]
fn the_projection_operation_runs_through_the_harness_chain_loop() {
    // The `Operation` leg exists so a host can build the projection
    // without a runtime. Run it through the harness's chain loop to show
    // the boxed form is the same projection `sync` uses.
    let mut harness = Harness::headless(|| focusable_tree(&["a", "b"]));
    let (group, mut model) = model_with("panel", vec![member("a"), member("b")]);
    model.focus_first_in_group(&group);

    let projection: Box<dyn Operation<()>> = focus_operation(&model);
    harness.run(projection);

    assert_agree(&mut harness, &model, "running the projection directly");
}

#[test]
fn building_the_tab_operation_advances_the_model_immediately() {
    // The host computes the target in `update` and hands iced the task, so
    // the model must already be on the new answer by the time the task is
    // queued — otherwise a second `Tab` in the same frame would compute
    // from the old one and the two would fight over the router.
    let (group, mut model) = model_with("panel", vec![member("a"), member("b")]);
    model.focus_first_in_group(&group);

    let _forward = tab_operation::<()>(&mut model, Direction::Forward);
    assert_eq!(
        model.current(),
        Some(&FocusId::new("b")),
        "the model moves when the operation is built, not when iced runs it"
    );

    let _backward = tab_operation::<()>(&mut model, Direction::Backward);
    assert_eq!(model.current(), Some(&FocusId::new("a")));
}

// ---------------------------------------------------------------------------
// Direction
// ---------------------------------------------------------------------------

#[test]
fn direction_from_shift_maps_both_ways() {
    assert_eq!(Direction::from_shift(false), Direction::Forward);
    assert_eq!(Direction::from_shift(true), Direction::Backward);
}

// ---------------------------------------------------------------------------
// The mechanical guard on INC-10 constraint 1
// ---------------------------------------------------------------------------

#[test]
fn the_bridge_never_names_iced_s_own_traversal() {
    // `INC-10` constraint 1: handing `Tab` to iced's `focus_next` /
    // `focus_previous` loses focus at the ends instead of wrapping, so it
    // must never appear in this module. Asserting it against the source is
    // the only way to make the rule outlast the person who wrote it — a
    // later "simplification" that delegates to the router would otherwise
    // look like fewer lines rather than like a dropped keyboard.
    let code = executable_source();

    for forbidden in [
        "focusable::focus_next",
        "focusable::focus_previous",
        "operation::focus_next",
        "operation::focus_previous",
    ] {
        assert!(
            !code.contains(forbidden),
            "`{forbidden}` must never appear in `bridge.rs`: the bridge \
             computes its target from `FocusState` and pushes it with \
             `focus(id)`, because iced's own traversal loses focus at the ends"
        );
    }

    assert!(
        code.contains("focusable::focus::"),
        "the guard is not vacuous: the bridge must still push with \
         `operation::focusable::focus`"
    );
}

/// `bridge.rs` with every comment line stripped, so the guard reads code
/// rather than the prose that explains why the code is absent.
fn executable_source() -> String {
    include_str!("bridge.rs")
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// An enabled member, spelled shortly for the fixtures above.
fn member(label: &str) -> FocusMember {
    FocusMember::new(label)
}
