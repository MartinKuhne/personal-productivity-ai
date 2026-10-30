//! Proofs that a custom widget can hold keyboard focus in iced 0.14, and
//! that the wrapper's registration, gating and rebuild behaviour match what
//! the pure model decides (**D-7**, **G-10**).
//!
//! # How focus is driven here
//!
//! `iced_test` 0.14 has no focus API — `Simulator` can click, type and
//! snapshot, but cannot focus anything, because iced itself has no focus
//! event to simulate. Focus lives one level down, in the widget-tree
//! `Operation` protocol, so these tests drive that protocol through the
//! shared [`Harness`]: a headless renderer, a real `UserInterface` over a
//! real widget tree, the [`user_interface::Cache`] carried from build to
//! build so the `Tree` that holds [`FocusableState`] survives a rebuild the
//! way it does across `view` calls, and the operation chain loop
//! `UserInterface::operate` does not run for itself.
//!
//! What that harness is for is documented on
//! [`crate::test_helpers`]; what these tests assert is that the wrapper
//! keeps its four promises:
//!
//! * **Registration** — one enabled wrapper is exactly one focusable
//!   widget in the router, in tree order, with real laid-out bounds. Stock
//!   iced widgets contribute none, which is the whole of **G-10**.
//! * **Gating** — a disabled wrapper is *absent*, not merely skipped by
//!   name, so the router cannot traverse to it, focus it, or take its focus
//!   back.
//! * **Agreement** — every transition of [`FocusState`], pushed through
//!   [`crate::bridge`], leaves the router naming the same control.
//! * **Survival** — focus rides out a rebuild when the wrapper does, and
//!   does not ride out one when the wrapper is gone or disabled.
//!
//! Results are read back with [`Harness::focused`] / [`Harness::is_focused`],
//! and with [`RecordFocusables`](crate::test_helpers::RecordFocusables),
//! which captures every `(id, bounds)` the router visits — which is how
//! traversal *order* and layout transparency are checked.

use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::focus::GroupId;
use crate::test_helpers::Harness;
use crate::test_helpers::assert_agree;
use crate::test_helpers::focusable_tree;
use crate::test_helpers::iced_id;
use crate::test_helpers::mixed_focusable_tree;
use crate::test_helpers::model_of;
use crate::test_helpers::opt_ids;
use crate::test_helpers::wrapped;
use crate::widget::focusable::Focusable;
use crate::widget::focusable::FocusableState;

use iced::Theme;
use iced::widget::button;
use iced::widget::column;
use iced::widget::text;

use iced_test::core::widget::operation;

/// A wrapper with the harness's concrete `Theme`/`Renderer`, needed where
/// a bare `Focusable::new` leaves them unresolvable.
type Wrapped = Focusable<'static, (), Theme, iced_test::renderer::Renderer>;

/// A headless harness over a view of focusable wrappers.
fn harness(labels: &'static [&'static str]) -> Harness<'static, (), iced_test::renderer::Renderer> {
    Harness::headless(move || focusable_tree(labels))
}

/// A headless harness whose members carry their own enabled flags.
fn mixed_harness(
    entries: &'static [(&'static str, bool)],
) -> Harness<'static, (), iced_test::renderer::Renderer> {
    Harness::headless(move || mixed_focusable_tree(entries))
}

// ---------------------------------------------------------------------------
// 1. Registration
// ---------------------------------------------------------------------------

#[test]
fn a_wrapper_registers_with_the_focus_router() {
    let mut harness = harness(&["alpha"]);

    assert_eq!(
        harness.focus_count().total,
        1,
        "one wrapper must appear as one focusable widget in the router"
    );
    assert_eq!(
        harness.registered(),
        vec![Some(iced_id("alpha"))],
        "the wrapper must register under the `Id` derived from its `FocusId`"
    );
}

#[test]
fn a_registered_wrapper_starts_unfocused() {
    let mut harness = harness(&["alpha"]);
    let id = iced_id("alpha");

    assert!(
        !harness.is_focused(&id),
        "registration alone must not put focus on the wrapper"
    );
    assert_eq!(harness.focused(), None);
}

#[test]
fn the_focus_operation_reaches_the_wrapper_and_its_state_becomes_focused() {
    let mut harness = harness(&["alpha"]);
    let id = iced_id("alpha");

    harness.focus(&id);

    assert!(
        harness.is_focused(&id),
        "`operation::focus` must reach the wrapper and flip \
         `FocusableState::is_focused`"
    );
    assert_eq!(harness.focused(), Some(id));
}

#[test]
fn stock_widgets_register_nothing_so_the_wrapper_is_what_adds_them() {
    let mut harness =
        Harness::headless(|| column(vec![text("a").into(), button("b").into()]).into());

    assert_eq!(
        harness.focus_count().total,
        0,
        "a tree of stock iced widgets must expose no focusable widget at all \
         (G-10): every registration these tests see comes from our wrapper"
    );

    harness.set_view(|| {
        column(vec![
            text("a").into(),
            button("b").into(),
            wrapped("c", true),
        ])
        .into()
    });

    assert_eq!(
        harness.focus_count().total,
        1,
        "adding one wrapper to a stock tree adds exactly one focusable widget"
    );
}

#[test]
fn registration_follows_tree_order_and_carries_real_bounds() {
    let mut harness = harness(&["one", "two", "three"]);

    let recorded = harness.recorded();

    assert_eq!(
        recorded
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>(),
        opt_ids(&["one", "two", "three"]),
        "the router must visit wrappers in tree order"
    );

    let heights: Vec<f32> = recorded.iter().map(|(_, bounds)| bounds.height).collect();
    assert!(
        heights.iter().all(|height| *height > 0.0),
        "every wrapper must report real laid-out bounds, got {heights:?}"
    );

    let tops: Vec<f32> = recorded.iter().map(|(_, bounds)| bounds.y).collect();
    assert!(
        tops.windows(2).all(|pair| pair[1] > pair[0]),
        "a column lays each member further down the screen, got {tops:?}"
    );
}

// ---------------------------------------------------------------------------
// 2. Traversal
// ---------------------------------------------------------------------------

#[test]
fn focus_next_walks_the_wrappers_in_tree_order() {
    let mut harness = harness(&["a", "b", "c"]);
    let expected = opt_ids(&["a", "b", "c"]);

    harness.iced_focus_next();
    assert_eq!(
        harness.focused(),
        expected[0].clone(),
        "with nothing focused, Tab lands on the first wrapper"
    );

    for id in expected[1..].iter() {
        harness.iced_focus_next();
        assert_eq!(harness.focused(), id.clone());
    }
}

#[test]
fn focus_previous_walks_the_wrappers_in_reverse_tree_order() {
    let mut harness = harness(&["a", "b", "c"]);
    let expected = opt_ids(&["a", "b", "c"]);

    harness.focus(expected[2].as_ref().unwrap());

    harness.iced_focus_previous();
    assert_eq!(harness.focused(), expected[1].clone());

    harness.iced_focus_previous();
    assert_eq!(harness.focused(), expected[0].clone());
}

#[test]
fn only_one_wrapper_holds_focus_at_a_time() {
    let mut harness = harness(&["a", "b", "c"]);
    let all = opt_ids(&["a", "b", "c"]);

    harness.focus(all[1].as_ref().unwrap());

    assert!(harness.is_focused(all[1].as_ref().unwrap()));
    assert!(!harness.is_focused(all[0].as_ref().unwrap()));
    assert!(!harness.is_focused(all[2].as_ref().unwrap()));
    assert_eq!(
        harness.focus_count().focused,
        Some(1),
        "the router's focused index must agree with which wrapper reports focus"
    );
}

#[test]
fn iced_traversal_drops_focus_at_the_ends_instead_of_wrapping() {
    // A limitation of iced's own traversal, pinned rather than worked
    // around. `FocusNext` unfocuses the last widget and has no widget at
    // index `total` to focus next, so Tab past the end loses focus
    // instead of cycling; `FocusPrevious` drops it before the first.
    // `FocusState::focus_next` wraps, which is what `UIF-CMENU-003`
    // requires, so the bridge must compute the next id from the model
    // and push it with `focus(id)` — never hand `Tab` to
    // `operation::focusable::focus_next`.
    let mut harness = harness(&["a", "b", "c"]);

    harness.focus(&iced_id("c"));
    harness.iced_focus_next();
    assert_eq!(
        harness.focused(),
        None,
        "Tab past the last wrapper loses focus in iced rather than wrapping"
    );

    harness.focus(&iced_id("a"));
    harness.iced_focus_previous();
    assert_eq!(
        harness.focused(),
        None,
        "Shift+Tab before the first wrapper loses focus in iced rather than wrapping"
    );
}

// ---------------------------------------------------------------------------
// 3. Enabled gating
// ---------------------------------------------------------------------------

#[test]
fn a_disabled_wrapper_is_not_registered_at_all() {
    let mut harness = mixed_harness(&[("a", true), ("b", false), ("c", true)]);

    assert_eq!(
        harness.focus_count().total,
        2,
        "a disabled wrapper must be absent from the router, not merely skipped by name"
    );
    assert_eq!(
        harness.registered(),
        opt_ids(&["a", "c"]),
        "only enabled wrappers may appear"
    );
}

#[test]
fn traversal_steps_over_a_disabled_wrapper() {
    let mut harness = mixed_harness(&[("a", true), ("b", false), ("c", true)]);

    harness.iced_focus_next();
    assert_eq!(harness.focused(), Some(iced_id("a")));

    harness.iced_focus_next();
    assert_eq!(
        harness.focused(),
        Some(iced_id("c")),
        "Tab must go straight from `a` to `c`, never stopping on the disabled `b`"
    );
}

#[test]
fn focusing_a_disabled_wrapper_is_a_no_op() {
    let mut harness = mixed_harness(&[("a", true), ("b", false)]);
    let disabled = iced_id("b");

    harness.focus(&disabled);

    assert_eq!(
        harness.focused(),
        None,
        "a focus request for a disabled wrapper must not move focus anywhere"
    );
    assert!(
        !harness.is_focused(&disabled),
        "the disabled wrapper must not report focus"
    );
}

#[test]
fn disabling_the_focused_wrapper_clears_its_focus_on_the_next_pass() {
    let mut harness = harness(&["a", "b"]);
    let id = iced_id("a");

    harness.focus(&id);
    assert!(harness.is_focused(&id));

    harness.set_view(move || mixed_focusable_tree(&[("a", false), ("b", true)]));

    assert!(
        !harness.is_focused(&id),
        "a wrapper that becomes disabled must drop the focus it was holding: it \
         is invisible to the router, so nothing else can take it back"
    );
    assert_eq!(
        harness.focused(),
        None,
        "no phantom focus may survive the transition"
    );
}

#[test]
fn a_tree_of_only_disabled_wrappers_cannot_hold_focus() {
    let mut harness = mixed_harness(&[("a", false), ("b", false)]);

    assert_eq!(harness.focus_count().total, 0);

    harness.iced_focus_next();
    harness.focus(&iced_id("a"));

    assert_eq!(harness.focused(), None);
}

#[test]
fn re_enabling_a_wrapper_makes_it_reachable_again() {
    let mut harness = mixed_harness(&[("a", false), ("b", true)]);
    let a = iced_id("a");

    harness.focus(&a);
    assert_eq!(harness.focused(), None);

    harness.set_view(move || mixed_focusable_tree(&[("a", true), ("b", true)]));
    harness.focus(&a);

    assert_eq!(
        harness.focused(),
        Some(a),
        "gating follows the flag on the current frame, not the flag the wrapper \
         was first built with"
    );
}

// ---------------------------------------------------------------------------
// 4. Round-trip with the pure model
// ---------------------------------------------------------------------------

#[test]
fn the_model_and_the_router_agree_across_a_full_walk() {
    let mut harness = harness(&["a", "b", "c"]);
    let (group, mut model) = model_of(&["a", "b", "c"]);

    harness.sync(&model);
    assert_agree(&mut harness, &model, "registration with nothing focused");

    model.focus_next();
    harness.sync(&model);
    assert_agree(&mut harness, &model, "focus_next into the first member");

    model.focus_next();
    harness.sync(&model);
    assert_agree(&mut harness, &model, "focus_next to the second member");

    model.focus_next();
    harness.sync(&model);
    assert_agree(&mut harness, &model, "focus_next to the last member");

    model.focus_next();
    harness.sync(&model);
    assert_agree(&mut harness, &model, "focus_next wrapping to the front");

    model.focus_previous();
    harness.sync(&model);
    assert_agree(&mut harness, &model, "focus_previous wrapping to the back");

    model.focus_first_in_group(&group);
    harness.sync(&model);
    assert_agree(&mut harness, &model, "focus_first_in_group");

    model.focus_last_in_group(&group);
    harness.sync(&model);
    assert_agree(&mut harness, &model, "focus_last_in_group");

    model.move_within(&group, 1);
    harness.sync(&model);
    assert_agree(
        &mut harness,
        &model,
        "move_within(+1) from the last row, wrapping to the first",
    );

    model.move_within(&group, -1);
    harness.sync(&model);
    assert_agree(
        &mut harness,
        &model,
        "move_within(-1) from the first row, wrapping to the last",
    );
}

#[test]
fn the_model_skips_disabled_members_and_the_router_follows() {
    let mut harness = mixed_harness(&[("a", true), ("b", false), ("c", true)]);

    let group = GroupId::new("menu");
    let mut model = FocusState::new();
    model.register_group(
        group.clone(),
        vec![
            FocusMember::new("a"),
            FocusMember::disabled("b"),
            FocusMember::new("c"),
        ],
    );

    model.focus_first_in_group(&group);
    harness.sync(&model);
    assert_eq!(harness.focused(), Some(iced_id("a")), "first enabled row");

    model.focus_next();
    harness.sync(&model);
    assert_eq!(
        harness.focused(),
        Some(iced_id("c")),
        "both layers must skip the disabled row the same way"
    );

    model.focus_next();
    harness.sync(&model);
    assert_agree(
        &mut harness,
        &model,
        "wrapping past the last enabled row skips the disabled one again",
    );
}

#[test]
fn a_model_rejection_leaves_the_router_untouched() {
    let mut harness = mixed_harness(&[("a", true), ("b", false), ("c", true)]);

    let group = GroupId::new("menu");
    let mut model = FocusState::new();
    model.register_group(
        group.clone(),
        vec![
            FocusMember::new("a"),
            FocusMember::disabled("b"),
            FocusMember::new("c"),
        ],
    );
    assert!(model.focus(&FocusId::new("a")));
    harness.sync(&model);
    assert_agree(&mut harness, &model, "focus on `a`");

    assert!(
        !model.focus(&FocusId::new("b")),
        "the model must reject a disabled member"
    );
    assert!(
        !model.focus(&FocusId::new("unregistered")),
        "the model must reject an unknown id"
    );
    harness.sync(&model);
    assert_eq!(
        harness.focused(),
        Some(iced_id("a")),
        "a rejected transition must not move the router either"
    );

    model.push_trap(GroupId::new("modal"));
    assert!(
        !model.focus(&FocusId::new("c")),
        "the trap must reject a member outside its group"
    );
    harness.sync(&model);
    assert_eq!(
        harness.focused(),
        Some(iced_id("a")),
        "the router must still show the trapped focus, not the rejected one"
    );
}

#[test]
fn the_router_projection_is_pushed_from_the_model_because_iced_traversal_diverges() {
    // Same starting point, two traversal engines, one divergence: at the
    // last row the model wraps and iced's `focus_next` drops focus. If
    // the bridge ever handed `Tab` to the router instead of pushing the
    // model's answer, this is exactly where the two would come apart.
    let mut harness = harness(&["a", "b", "c"]);
    let (group, mut model) = model_of(&["a", "b", "c"]);

    model.focus_last_in_group(&group);
    harness.sync(&model);
    assert_eq!(harness.focused(), Some(iced_id("c")));

    model.focus_next();
    assert_eq!(model.current(), Some(&FocusId::new("a")), "the model wraps");

    harness.iced_focus_next();
    assert_eq!(
        harness.focused(),
        None,
        "iced's own traversal drops focus here — which is why the bridge is \
         push-from-the-model, not delegate-to-the-router"
    );

    harness.sync(&model);
    assert_agree(&mut harness, &model, "pushing the model's wrapped answer");
}

#[test]
fn clearing_the_model_clears_the_router() {
    let mut harness = harness(&["a", "b"]);
    let (group, mut model) = model_of(&["a", "b"]);

    model.focus_first_in_group(&group);
    harness.sync(&model);
    assert_eq!(harness.focused(), Some(iced_id("a")));

    assert_eq!(model.clear(), Some(FocusId::new("a")));
    harness.sync(&model);
    assert_eq!(harness.focused(), None);
}

// ---------------------------------------------------------------------------
// 5. Cleanup
// ---------------------------------------------------------------------------

#[test]
fn removing_the_focused_wrapper_leaves_no_phantom_focus() {
    let mut harness = harness(&["a", "b"]);
    let b = iced_id("b");

    harness.focus(&b);
    assert_eq!(harness.focused(), Some(b.clone()));

    harness.set_view(move || focusable_tree(&["a"]));

    assert_eq!(
        harness.focused(),
        None,
        "`b` is gone from the tree; its focus must have gone with it rather than \
         lingering as an id nothing can reach"
    );
    assert_eq!(
        harness.focus_count().total,
        1,
        "the router must see only the wrapper that is still mounted"
    );
}

#[test]
fn a_wrapper_that_survives_a_rebuild_keeps_its_focus() {
    let mut harness = harness(&["a", "b"]);
    let b = iced_id("b");

    harness.focus(&b);
    harness.focus(&b);

    assert_eq!(
        harness.focused(),
        Some(b),
        "rebuilding the same tree must not reset focus — the state is pinned by \
         the wrapper's tag"
    );
}

#[test]
fn removing_a_different_wrapper_keeps_the_focus() {
    let mut harness = harness(&["a", "b", "c"]);
    let a = iced_id("a");

    harness.focus(&a);
    harness.set_view(move || focusable_tree(&["a", "c"]));

    assert_eq!(
        harness.focused(),
        Some(a),
        "dropping a sibling must not disturb the focused wrapper"
    );
}

#[test]
fn a_wrapper_disabled_while_focused_stays_unfocused_across_rebuilds() {
    let mut harness = harness(&["a", "b"]);
    let a = iced_id("a");

    harness.focus(&a);
    assert!(harness.is_focused(&a));

    harness.set_view(move || mixed_focusable_tree(&[("a", false), ("b", true)]));
    assert!(!harness.is_focused(&a));

    harness.set_view(move || mixed_focusable_tree(&[("a", false), ("b", true)]));
    assert_eq!(
        harness.focused(),
        None,
        "the cleared state must stay cleared across rebuilds rather than be \
         restored by the surviving tag"
    );
}

// ---------------------------------------------------------------------------
// Wrapper contract
// ---------------------------------------------------------------------------

#[test]
fn the_widget_id_is_derived_from_the_focus_id() {
    let widget: Wrapped = Focusable::new(text("save"), FocusId::new("dialog.save"), true);

    assert_eq!(widget.focus_id(), &FocusId::new("dialog.save"));
    assert_eq!(
        widget.widget_id(),
        &iced_id("dialog.save"),
        "the iced `Id` must be the `FocusId`'s own string, so a rebuilt wrapper \
         re-claims the same router slot"
    );
    assert!(widget.is_enabled());

    let dead: Wrapped = Focusable::new(text("save"), FocusId::new("dialog.save"), false);
    assert!(!dead.is_enabled());
    assert_eq!(dead.widget_id(), widget.widget_id());
}

#[test]
fn the_wrapper_is_layout_transparent() {
    // Two wrappers around the same child must report the same bounds, or
    // the outer one is adding a layout level — a styling bug wearing a
    // focus fix. Nesting is the only way to see this through the focus
    // router alone, because both layers register.
    let mut harness = Harness::headless(|| {
        Focusable::new(wrapped("inner", true), FocusId::new("outer"), true).into()
    });

    let recorded = harness.recorded();

    assert_eq!(
        recorded
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>(),
        vec![Some(iced_id("outer")), Some(iced_id("inner"))],
        "the outer wrapper registers first, then descends into its child"
    );
    assert_eq!(
        recorded[0].1, recorded[1].1,
        "the outer wrapper must not change the bounds of what it wraps"
    );
}

#[test]
fn a_disabled_wrapper_still_lets_its_children_register() {
    // The flag gates the wrapper's own registration only. A disabled
    // wrapper is not a subtree blackout: whether something mounted
    // inside one is focusable is the host's decision, not this widget's.
    let mut harness = Harness::headless(|| {
        Focusable::new(wrapped("inner", true), FocusId::new("outer"), false).into()
    });

    assert_eq!(
        harness.registered(),
        vec![Some(iced_id("inner"))],
        "the disabled outer wrapper must be absent while its inner one stays"
    );
}

#[test]
fn the_wrapper_state_round_trips_through_the_router_protocol_directly() {
    // The bare protocol, with no widget or tree in the way: the router's
    // three verbs are the only way in and out of focus.
    let mut state = FocusableState::new();
    assert!(!state.is_focused());

    <FocusableState as operation::Focusable>::focus(&mut state);
    assert!(state.is_focused());
    assert!(<FocusableState as operation::Focusable>::is_focused(&state));

    <FocusableState as operation::Focusable>::unfocus(&mut state);
    assert!(!state.is_focused());
    assert_eq!(FocusableState::default(), FocusableState::new());
}
