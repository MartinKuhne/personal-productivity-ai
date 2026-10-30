//! Proofs about the model → router bridge: what it pushes, what it refuses
//! to push, and the mechanical guard that keeps iced's own traversal out of
//! it (`UIF-PRIN-005`, `INC-10` constraints 1 and 2).
//!
//! # One session, many cases
//!
//! A headless harness pays roughly 600 ms to build iced's renderer, so the
//! projection, tab and refusal legs each drive **one** session over several
//! cases instead of building one session per case. Every case registers its
//! widgets under its own namespaced focus ids ([`scoped`]), so no case can
//! observe another case's router state: each still sees freshly registered,
//! unfocused widgets exactly as a harness of its own would have given it.
//! The assertions are unchanged in number and in kind — only the number of
//! renderers built has dropped.

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
use crate::test_helpers::El;
use crate::test_helpers::Harness;
use crate::test_helpers::assert_agree;
use crate::test_helpers::focusable_tree;
use crate::test_helpers::iced_id;
use crate::test_helpers::model_with;
use crate::test_helpers::wrapped;

use iced::Task;
use iced::advanced::widget::Operation;
use iced::widget::Id;
use iced::widget::column;
use iced_test::renderer::Renderer as HeadlessRenderer;

// ---------------------------------------------------------------------------
// Namespaced fixtures
// ---------------------------------------------------------------------------

/// The focus id case `scope` gives the label `label`.
///
/// Namespacing is what makes a shared harness safe: focus lives in the
/// widget state the router keys by id, so two cases that never share an id
/// never share focus either.
fn scoped_name(scope: &str, label: &str) -> String {
    format!("{scope}_{label}")
}

/// [`scoped_name`] as a [`FocusId`].
fn scoped(scope: &str, label: &str) -> FocusId {
    FocusId::new(scoped_name(scope, label))
}

/// [`scoped_name`] as the iced [`Id`] the router reports.
fn scoped_id(scope: &str, label: &str) -> Id {
    iced_id(&scoped_name(scope, label))
}

/// An enabled member of case `scope`.
fn scoped_member(scope: &str, label: &str) -> FocusMember {
    FocusMember::new(scoped(scope, label))
}

/// A disabled member of case `scope`.
fn scoped_disabled(scope: &str, label: &str) -> FocusMember {
    FocusMember::disabled(scoped(scope, label))
}

/// A column of enabled focusable wrappers named `{scope}_{label}` — the
/// same tree [`focusable_tree`] builds, under the case's own ids.
fn scoped_tree(scope: &str, labels: &[&str]) -> El<'static, (), HeadlessRenderer> {
    column(
        labels
            .iter()
            .map(|label| wrapped(&scoped_name(scope, label), true))
            .collect::<Vec<_>>(),
    )
    .into()
}

/// [`scoped_tree`] with each wrapper carrying its own enabled flag, the
/// namespaced form of `mixed_focusable_tree`.
fn scoped_mixed_tree(scope: &str, entries: &[(&str, bool)]) -> El<'static, (), HeadlessRenderer> {
    column(
        entries
            .iter()
            .map(|(label, enabled)| wrapped(&scoped_name(scope, label), *enabled))
            .collect::<Vec<_>>(),
    )
    .into()
}

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
// Projection — one session, three cases
// ---------------------------------------------------------------------------

#[test]
fn the_bridge_projects_exactly_what_the_model_names() {
    // Case 1 — a model that names its first member, pushed onto the router.
    let scope = "projection_named";
    let (group, mut model) = model_with(
        "panel",
        vec![
            scoped_member(scope, "a"),
            scoped_member(scope, "b"),
            scoped_member(scope, "c"),
        ],
    );
    let mut harness = Harness::headless(move || scoped_tree(scope, &["a", "b", "c"]));

    assert_eq!(
        harness.focused(),
        None,
        "[{scope}] registration alone focuses nothing, and neither does a model \
         that has not been pushed"
    );

    assert!(model.focus_first_in_group(&group).is_some());
    harness.sync(&model);

    assert_agree(
        &mut harness,
        &model,
        "[projection_named] pushing the model's first member",
    );
    assert!(
        harness.is_focused(&scoped_id(scope, "a")),
        "[{scope}] the member the model named must be the one the router shows"
    );
    assert!(
        !harness.is_focused(&scoped_id(scope, "b")),
        "[{scope}] the projection must not light anything the model did not name"
    );

    // Case 2 — "nothing is focused" has to be *said*. A silent bridge would
    // leave whatever the router was already showing lit, and the screen
    // would disagree with the model.
    let scope = "projection_cleared";
    let (group, mut model) = model_with(
        "panel",
        vec![scoped_member(scope, "a"), scoped_member(scope, "b")],
    );
    harness.set_view(move || scoped_tree(scope, &["a", "b"]));

    model.focus_first_in_group(&group);
    harness.sync(&model);
    assert_eq!(
        harness.focused(),
        Some(scoped_id(scope, "a")),
        "[{scope}] the push must land on the first member"
    );

    assert_eq!(
        model.clear(),
        Some(scoped(scope, "a")),
        "[{scope}] clearing must report the member it dropped"
    );
    harness.sync(&model);

    assert_eq!(
        harness.focused(),
        None,
        "[{scope}] the router must stop showing `a` the moment the model stops \
         naming it"
    );

    // Case 3 — the empty-model leg of the same rule: the projection of
    // "nothing" focuses nothing, rather than defaulting to the first widget.
    let scope = "projection_empty";
    harness.set_view(move || scoped_tree(scope, &["a", "b"]));
    let model = FocusState::new();

    harness.sync(&model);

    assert_eq!(
        harness.focused(),
        None,
        "[{scope}] an empty model must leave the router naming nothing"
    );
    assert!(
        !harness.is_focused(&scoped_id(scope, "a")),
        "[{scope}] an empty model must not fall through to the first registered \
         widget"
    );
}

// ---------------------------------------------------------------------------
// Tab, forward and back — one session, five cases
// ---------------------------------------------------------------------------

#[test]
fn tab_moves_the_router_the_way_the_model_moves() {
    // Case 1 — Tab forward walks the tree in traversal order.
    let scope = "tab_forward";
    let (_group, mut model) = model_with(
        "panel",
        vec![
            scoped_member(scope, "a"),
            scoped_member(scope, "b"),
            scoped_member(scope, "c"),
        ],
    );
    let mut harness = Harness::headless(move || scoped_tree(scope, &["a", "b", "c"]));

    for expected in ["a", "b", "c"] {
        let landed = harness.tab(&mut model, Direction::Forward);

        assert_eq!(
            landed,
            Some(scoped(scope, expected)),
            "[{scope}] Tab forward must land on `{expected}`"
        );
        assert_agree(
            &mut harness,
            &model,
            &format!("[{scope}] Tab forward to {expected}"),
        );
    }

    // Case 2 — Shift+Tab backward walks it in reverse.
    let scope = "tab_backward";
    let (_group, mut model) = model_with(
        "panel",
        vec![
            scoped_member(scope, "a"),
            scoped_member(scope, "b"),
            scoped_member(scope, "c"),
        ],
    );
    harness.set_view(move || scoped_tree(scope, &["a", "b", "c"]));

    for expected in ["c", "b", "a"] {
        let landed = harness.tab(&mut model, Direction::Backward);

        assert_eq!(
            landed,
            Some(scoped(scope, expected)),
            "[{scope}] Shift+Tab must land on `{expected}`"
        );
        assert_agree(
            &mut harness,
            &model,
            &format!("[{scope}] Shift+Tab backward to {expected}"),
        );
    }

    // Case 3 — the whole reason the bridge exists, proved against a real
    // tree: at the last control iced's own traversal drops focus, the model
    // wraps, and pushing the model's answer is what keeps the keyboard alive.
    let scope = "tab_wrap_end";
    let (group, mut model) = model_with(
        "panel",
        vec![
            scoped_member(scope, "a"),
            scoped_member(scope, "b"),
            scoped_member(scope, "c"),
        ],
    );
    harness.set_view(move || scoped_tree(scope, &["a", "b", "c"]));

    model.focus_last_in_group(&group);
    harness.sync(&model);
    assert_eq!(
        harness.focused(),
        Some(scoped_id(scope, "c")),
        "[{scope}] the model starts on the last member"
    );

    harness.iced_focus_next();
    assert_eq!(
        harness.focused(),
        None,
        "[{scope}] iced's own traversal drops focus here — which is why the \
         bridge is push-from-the-model, never delegate-to-the-router"
    );

    let landed = harness.tab(&mut model, Direction::Forward);
    assert_eq!(
        landed,
        Some(scoped(scope, "a")),
        "[{scope}] the model wraps"
    );
    assert_eq!(
        harness.focused(),
        Some(scoped_id(scope, "a")),
        "[{scope}] and the push lands on the first widget rather than nowhere"
    );

    // Case 4 — the mirror image before the first control.
    let scope = "tab_wrap_start";
    let (group, mut model) = model_with(
        "panel",
        vec![
            scoped_member(scope, "a"),
            scoped_member(scope, "b"),
            scoped_member(scope, "c"),
        ],
    );
    harness.set_view(move || scoped_tree(scope, &["a", "b", "c"]));

    model.focus_first_in_group(&group);
    harness.sync(&model);
    assert_eq!(
        harness.focused(),
        Some(scoped_id(scope, "a")),
        "[{scope}] the model starts on the first member"
    );

    harness.iced_focus_previous();
    assert_eq!(
        harness.focused(),
        None,
        "[{scope}] Shift+Tab before the first widget loses focus in iced rather \
         than wrapping to the last"
    );

    let landed = harness.tab(&mut model, Direction::Backward);
    assert_eq!(
        landed,
        Some(scoped(scope, "c")),
        "[{scope}] the model wraps to the last member"
    );
    assert_eq!(
        harness.focused(),
        Some(scoped_id(scope, "c")),
        "[{scope}] and the router follows the model there"
    );

    // Case 5 — a disabled row is stepped over by both layers alike.
    let scope = "tab_skip_disabled";
    let (_group, mut model) = model_with(
        "menu",
        vec![
            scoped_member(scope, "a"),
            scoped_disabled(scope, "b"),
            scoped_member(scope, "c"),
        ],
    );
    harness.set_view(move || scoped_mixed_tree(scope, &[("a", true), ("b", false), ("c", true)]));

    let landed = harness.tab(&mut model, Direction::Forward);
    assert_eq!(
        landed,
        Some(scoped(scope, "a")),
        "[{scope}] the first step lands on the first live member"
    );

    let landed = harness.tab(&mut model, Direction::Forward);
    assert_eq!(
        landed,
        Some(scoped(scope, "c")),
        "[{scope}] both layers must step over the disabled row the same way"
    );
    assert!(
        !harness.is_focused(&scoped_id(scope, "b")),
        "[{scope}] a disabled widget must never be the bridge's answer"
    );
}

// ---------------------------------------------------------------------------
// Refusals — one session, three cases
// ---------------------------------------------------------------------------

#[test]
fn a_step_the_model_refuses_leaves_the_router_alone() {
    // Case 1 — a trap around a group that has not mounted yet makes the
    // active scope empty, so `focus_next` returns `None` and the model is
    // unchanged. The projection of an unchanged model must be unchanged:
    // the router keeps showing `a` rather than being cleared.
    let scope = "refusal_empty_trap";
    let (group, mut model) = model_with(
        "panel",
        vec![scoped_member(scope, "a"), scoped_member(scope, "b")],
    );
    let mut harness = Harness::headless(move || scoped_tree(scope, &["a", "b"]));

    model.focus_first_in_group(&group);
    harness.sync(&model);
    assert_eq!(
        harness.focused(),
        Some(scoped_id(scope, "a")),
        "[{scope}] the model starts on the first member"
    );

    model.push_trap(GroupId::new("not_mounted_yet"));
    let landed = harness.tab(&mut model, Direction::Forward);

    assert_eq!(
        landed.as_ref(),
        Some(&scoped(scope, "a")),
        "[{scope}] the empty scope has nowhere to go, so the model must still be \
         on `a` — the harness reports what the bridge actually pushed, never what \
         a step would have reached"
    );
    assert_eq!(
        harness.focused(),
        Some(scoped_id(scope, "a")),
        "[{scope}] a refused step must not move the router, and must not clear it \
         either"
    );
    assert_eq!(
        harness.registered(),
        vec![Some(scoped_id(scope, "a")), Some(scoped_id(scope, "b"))],
        "[{scope}] a refused step must not disturb what the tree registered either"
    );

    // Case 2 — every member disabled: the model has no answer, so the bridge
    // must not put focus anywhere — not even on the one widget the tree
    // happily registered.
    let scope = "refusal_nothing_focusable";
    let (_group, mut model) = model_with("menu", vec![scoped_disabled(scope, "a")]);
    harness.set_view(move || scoped_tree(scope, &["a"]));

    let landed = harness.tab(&mut model, Direction::Forward);

    assert_eq!(
        landed, None,
        "[{scope}] a model with no live member has no answer to give"
    );
    assert_eq!(
        harness.focused(),
        None,
        "[{scope}] and the router must therefore name nothing"
    );
    assert!(
        !harness.is_focused(&scoped_id(scope, "a")),
        "[{scope}] a widget the model refuses must not be focused just because it \
         exists"
    );

    // Case 3 — confinement is the model's decision and the bridge carries it
    // verbatim: with a trap on `modal`, no amount of Tabbing reaches the
    // background group (`UIF-MOD-001`).
    let scope = "refusal_trap";
    harness.set_view(move || {
        column(vec![
            wrapped(&scoped_name(scope, "base_a"), true),
            wrapped(&scoped_name(scope, "base_b"), true),
            wrapped(&scoped_name(scope, "modal_a"), true),
            wrapped(&scoped_name(scope, "modal_b"), true),
        ])
        .into()
    });

    let base = GroupId::new("base");
    let modal = GroupId::new("modal");
    let mut model = FocusState::new();
    model.register_group(
        base.clone(),
        vec![
            scoped_member(scope, "base_a"),
            scoped_member(scope, "base_b"),
        ],
    );
    model.register_group(
        modal.clone(),
        vec![
            scoped_member(scope, "modal_a"),
            scoped_member(scope, "modal_b"),
        ],
    );

    model.push_trap(modal.clone());
    assert!(model.focus_first_in_group(&modal).is_some());
    harness.sync(&model);
    assert_eq!(
        harness.focused(),
        Some(scoped_id(scope, "modal_a")),
        "[{scope}] the trap's own group is where the push lands"
    );

    let trapped_prefix = format!("{scope}_modal_");
    for _ in 0..4 {
        let landed = harness.tab(&mut model, Direction::Forward);
        assert!(
            landed
                .as_ref()
                .is_some_and(|id| id.as_str().starts_with(&trapped_prefix)),
            "[{scope}] a trapped Tab must never land on the background, got \
             {landed:?}"
        );
        assert_agree(&mut harness, &model, "[refusal_trap] a trapped Tab");
    }

    assert!(
        !model.focus(&scoped(scope, "base_a")),
        "[{scope}] the model must reject a focus outside the trap"
    );
    let before_rejection = harness.focused();
    harness.sync(&model);
    assert_eq!(
        harness.focused(),
        before_rejection,
        "[{scope}] a rejected focus must not move the router either — the trap \
         has no hole for a programmatic path to fall through"
    );
    assert!(
        harness.focused().is_some_and(|id| {
            id == scoped_id(scope, "modal_a") || id == scoped_id(scope, "modal_b")
        }),
        "[{scope}] and the router must still be inside the trap"
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
