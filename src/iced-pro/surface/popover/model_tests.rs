//! Tests for the teaching popover's data model — `UIF-ATOM-007`, §3.7.
//!
//! Three groups:
//!
//! 1. **Construction and the field list.** That §3.7's five fields are
//!    all present, defaulted the way the section implies, and reachable.
//! 2. **The footer roster.** That it is never empty, that the primary
//!    action comes first when it exists, and that the dismiss control is
//!    the framework-supplied floor the trap always has something to
//!    hold.
//! 3. **Identity.** That the derived focus ids and the group id are
//!    stable, distinct, and match the roster.

use super::*;
use crate::focus::FocusMember;

/// The action type the tests use: a plain host command id.
type Action = &'static str;

// ---------------------------------------------------------------------------
// 1. Construction and the field list
// ---------------------------------------------------------------------------

#[test]
fn a_new_callout_is_wanted_open_and_prefers_below() {
    let model: PopoverModel<Action> = PopoverModel::new("Title", "Body text");
    assert_eq!(model.title(), "Title");
    assert_eq!(model.body(), "Body text");
    assert!(model.is_open_wanted());
    assert_eq!(model.placement(), PreferredSide::Below);
    assert!(!model.has_primary_action());
}

#[test]
fn with_action_adds_the_primary_button() {
    let model = PopoverModel::<Action>::new("T", "B").with_action("Got it", "ack");
    assert!(model.has_primary_action());
    assert_eq!(model.primary_action(), Some(("Got it", &"ack")));
}

#[test]
fn with_placement_sets_the_side() {
    for side in [
        PreferredSide::Below,
        PreferredSide::Above,
        PreferredSide::Leading,
        PreferredSide::Trailing,
    ] {
        let model = PopoverModel::<Action>::new("T", "B").with_placement(side);
        assert_eq!(model.placement(), side);
    }
}

#[test]
fn with_open_sets_the_intent_flag() {
    let closed = PopoverModel::<Action>::new("T", "B").with_open(false);
    assert!(!closed.is_open_wanted());
    let open = closed.with_open(true);
    assert!(open.is_open_wanted());
}

#[test]
fn builders_do_not_mutate_the_receiver() {
    let base = PopoverModel::<Action>::new("T", "B");
    let built = base
        .clone()
        .with_action("Go", "go")
        .with_placement(PreferredSide::Above)
        .with_open(false);
    assert!(!base.has_primary_action());
    assert_eq!(base.placement(), PreferredSide::Below);
    assert!(base.is_open_wanted());
    assert!(!built.is_open_wanted());
}

// ---------------------------------------------------------------------------
// 2. The footer roster
// ---------------------------------------------------------------------------

#[test]
fn the_footer_never_has_zero_members() {
    // The invariant the whole open path rests on: a trap around an empty
    // scope swallows Tab while leaving the ring stranded.
    for wanted in [true, false] {
        for with_action in [true, false] {
            let mut model = PopoverModel::<Action>::new("T", "B").with_open(wanted);
            if with_action {
                model = model.with_action("Go", "go");
            }
            assert!(
                !model.footer_members().is_empty(),
                "a callout with with_open({wanted}) and an action of {with_action} \
                 must still have a focusable footer control",
            );
        }
    }
}

#[test]
fn a_callout_without_an_action_has_only_the_dismiss_button() {
    let model = PopoverModel::<Action>::new("T", "B");
    assert_eq!(
        model.footer_members(),
        vec![FocusMember::new(dismiss_focus_id())],
    );
}

#[test]
fn a_callout_with_an_action_has_both_buttons_in_order() {
    let model = PopoverModel::<Action>::new("T", "B").with_action("Go", "go");
    assert_eq!(
        model.footer_members(),
        vec![
            FocusMember::new(primary_focus_id()),
            FocusMember::new(dismiss_focus_id()),
        ],
    );
}

#[test]
fn the_initial_focus_is_the_primary_action_when_there_is_one() {
    let model = PopoverModel::<Action>::new("T", "B").with_action("Go", "go");
    assert_eq!(model.initial_focus(), primary_focus_id());
}

#[test]
fn the_initial_focus_is_dismiss_when_there_is_no_action() {
    let model = PopoverModel::<Action>::new("T", "B");
    assert_eq!(model.initial_focus(), dismiss_focus_id());
}

#[test]
fn the_initial_focus_is_always_in_the_roster() {
    let with = PopoverModel::<Action>::new("T", "B").with_action("Go", "go");
    let without = PopoverModel::<Action>::new("T", "B");
    for model in [&with, &without] {
        let ids: Vec<_> = model
            .footer_members()
            .iter()
            .map(|m| m.id.clone())
            .collect();
        assert!(
            ids.contains(&model.initial_focus()),
            "the initial focus must be a member of the roster it lands in",
        );
    }
}

#[test]
fn every_roster_member_is_enabled() {
    let model = PopoverModel::<Action>::new("T", "B").with_action("Go", "go");
    for member in model.footer_members() {
        assert!(member.enabled, "{} must be traversable", member.id);
    }
}

// ---------------------------------------------------------------------------
// 3. Identity
// ---------------------------------------------------------------------------

#[test]
fn the_two_focus_ids_are_distinct() {
    assert_ne!(primary_focus_id(), dismiss_focus_id());
}

#[test]
fn the_ids_are_stable_across_calls() {
    assert_eq!(primary_focus_id(), primary_focus_id());
    assert_eq!(dismiss_focus_id(), dismiss_focus_id());
    assert_eq!(group_for(), group_for());
}

#[test]
fn the_group_and_the_members_share_the_popover_namespace() {
    assert_eq!(group_for().as_str(), POPOVER_GROUP);
    assert!(primary_focus_id().as_str().starts_with(POPOVER_GROUP));
    assert!(dismiss_focus_id().as_str().starts_with(POPOVER_GROUP));
}

#[test]
fn the_dismiss_label_is_the_framework_supplied_string() {
    assert_eq!(DISMISS_LABEL, "Dismiss");
    assert!(!DISMISS_LABEL.is_empty());
}

#[test]
fn a_rich_action_type_survives_the_round_trip() {
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Command {
        OpenSettings { tab: u8 },
        Dismiss,
    }

    let model = PopoverModel::new("T", "B").with_action("Open", Command::OpenSettings { tab: 3 });
    assert_eq!(
        model.primary_action(),
        Some(("Open", &Command::OpenSettings { tab: 3 })),
    );

    // A unit-like variant is just as valid an action id; the model never
    // inspects the type beyond cloning it.
    let plain = PopoverModel::new("T", "B").with_action("Nope", Command::Dismiss);
    assert_eq!(plain.primary_action(), Some(("Nope", &Command::Dismiss)));
    assert_eq!(model.clone(), model);
}

#[test]
fn the_model_clones_its_strings_not_its_aliases() {
    let model = PopoverModel::<Action>::new("T", "B").with_action("Go", "go");
    let clone = model.clone();
    assert_eq!(clone.title(), model.title());
    assert_eq!(clone.primary_action(), model.primary_action());
}
