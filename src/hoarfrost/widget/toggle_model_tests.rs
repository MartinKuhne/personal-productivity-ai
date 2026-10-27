//! Tests for the toggle-family data models — `UIF-ATOM-003`, §3.0.4.1.
//!
//! Four groups, in order:
//!
//! 1. **The tri-state transition.** The whole table, run exhaustively,
//!    plus the three properties that make it "deterministic": total,
//!    pure, and stateless.
//! 2. **The checkbox model.** Emission vs silence, and the disabled
//!    control that answers nothing.
//! 3. **The radio group.** Roving over enabled options only, wrapping at
//!    both ends, and the re-click that must not emit.
//! 4. **The switch.** The busy refusal, and the `pending` projection
//!    that refuses to advertise a cancellation nobody implemented.

use super::*;
use crate::widget::toggle::TogglePolarity;

/// Every tri-state, in declaration order.
const STATES: [CheckState; 3] = [
    CheckState::Unchecked,
    CheckState::Indeterminate,
    CheckState::Checked,
];

/// A spread of child summaries covering every shape the rule can be given:
/// a leaf, nothing enabled, all checked, some checked, one checked.
const CHILDREN: [ChildSummary; 6] = [
    ChildSummary::LEAF,
    ChildSummary {
        enabled: 3,
        checked: 0,
    },
    ChildSummary {
        enabled: 3,
        checked: 1,
    },
    ChildSummary {
        enabled: 3,
        checked: 2,
    },
    ChildSummary {
        enabled: 3,
        checked: 3,
    },
    ChildSummary {
        enabled: 1,
        checked: 1,
    },
];

fn options() -> Vec<RadioOption<&'static str>> {
    vec![
        RadioOption::new("a", "Alpha"),
        RadioOption::new("b", "Beta").disabled(),
        RadioOption::new("c", "Gamma"),
        RadioOption::new("d", "Delta"),
    ]
}

// ---------------------------------------------------------------------------
// 1. The tri-state transition
// ---------------------------------------------------------------------------

#[test]
fn the_transition_table_is_exactly_the_spec_rule() {
    // Unchecked -> Checked, Checked -> Unchecked, for every child shape.
    for children in CHILDREN {
        assert_eq!(
            CheckState::Unchecked.next_with(children),
            CheckState::Checked
        );
        assert_eq!(
            CheckState::Checked.next_with(children),
            CheckState::Unchecked
        );
    }

    // Indeterminate -> Unchecked only when every enabled child is already
    // checked; otherwise Checked.
    let all_checked = ChildSummary {
        enabled: 3,
        checked: 3,
    };
    assert_eq!(
        CheckState::Indeterminate.next_with(all_checked),
        CheckState::Unchecked
    );
    for children in CHILDREN {
        let expected = if children.all_checked() {
            CheckState::Unchecked
        } else {
            CheckState::Checked
        };
        assert_eq!(
            CheckState::Indeterminate.next_with(children),
            expected,
            "children {children:?}"
        );
    }
}

#[test]
fn a_leaf_indeterminate_box_becomes_checked_not_unchecked() {
    // The vacuous-truth trap: "all zero of my children are checked" is
    // vacuously true in ordinary logic, and the rule reads it as FALSE so
    // that a leaf simply becomes checked like any other control.
    assert!(!ChildSummary::LEAF.all_checked());
    assert_eq!(
        CheckState::Indeterminate.next_with(ChildSummary::LEAF),
        CheckState::Checked
    );
    assert_eq!(CheckState::Indeterminate.next(), CheckState::Checked);
}

#[test]
fn a_parent_whose_children_are_all_checked_collapses_to_unchecked() {
    let all = ChildSummary {
        enabled: 4,
        checked: 4,
    };
    assert!(all.all_checked());
    assert_eq!(
        CheckState::Indeterminate.next_with(all),
        CheckState::Unchecked,
        "the only deterministic reading of a fully-checked parent being clicked is to \
             clear it"
    );
}

#[test]
fn zero_enabled_children_never_satisfy_all_checked() {
    let none_enabled = ChildSummary {
        enabled: 0,
        checked: 0,
    };
    assert!(!none_enabled.all_checked());
    assert!(none_enabled.none_checked());

    let enabled_but_none_checked = ChildSummary {
        enabled: 2,
        checked: 0,
    };
    assert!(!enabled_but_none_checked.all_checked());
    assert!(enabled_but_none_checked.none_checked());
}

#[test]
fn the_transition_is_total_pure_and_stateless() {
    // Total: every (state, children) pair has an answer.
    // Pure: the same inputs give the same output, twice over.
    // Stateless: nothing but the two arguments is consulted — no clock,
    // no history, no previous call.
    for state in STATES {
        for children in CHILDREN {
            let first = state.next_with(children);
            let second = state.next_with(children);
            let third = state.next_with(children);
            assert_eq!(first, second, "{state:?} {children:?} not pure");
            assert_eq!(second, third, "{state:?} {children:?} not stateless");
            assert_ne!(
                first, state,
                "{state:?} {children:?}: an activation must always change the state"
            );
        }
    }
}

#[test]
fn every_state_maps_to_the_polarity_the_spec_row_names() {
    assert_eq!(CheckState::Unchecked.polarity(), TogglePolarity::Off);
    assert_eq!(CheckState::Indeterminate.polarity(), TogglePolarity::On);
    assert_eq!(CheckState::Checked.polarity(), TogglePolarity::On);
}

#[test]
fn unchecked_is_the_default_state() {
    assert_eq!(CheckState::default(), CheckState::Unchecked);
}

// ---------------------------------------------------------------------------
// 2. The checkbox model
// ---------------------------------------------------------------------------

#[test]
fn a_new_checkbox_is_unchecked_and_has_no_description() {
    let model = CheckboxModel::new("Include subfolders");
    assert_eq!(model.state, CheckState::Unchecked);
    assert_eq!(model.label, "Include subfolders");
    assert_eq!(model.description, None);
    assert!(!model.disabled);
}

#[test]
fn the_checkbox_builders_set_what_they_name() {
    let model = CheckboxModel::new("x")
        .with_state(CheckState::Indeterminate)
        .with_description("Some children are on")
        .disabled();

    assert_eq!(model.state, CheckState::Indeterminate);
    assert_eq!(model.description.as_deref(), Some("Some children are on"));
    assert!(model.disabled);
}

#[test]
fn an_enabled_checkbox_emits_the_next_state() {
    let model = CheckboxModel::new("x").with_state(CheckState::Indeterminate);
    let emitted = model
        .toggle(ChildSummary {
            enabled: 2,
            checked: 1,
        })
        .expect("an enabled control emits");
    assert_eq!(*emitted.value(), CheckState::Checked);
}

#[test]
fn a_disabled_checkbox_emits_nothing_rather_than_its_own_value_back() {
    for state in STATES {
        let model = CheckboxModel::new("x").with_state(state).disabled();
        for children in CHILDREN {
            assert!(
                model.toggle(children).is_none(),
                "{state:?} {children:?}: an inert control is present but does not answer"
            );
        }
    }
}

#[test]
fn toggling_does_not_mutate_the_model() {
    // §3.0.4.1: the app owns the value. The model hands back what the
    // app should adopt; it does not adopt it itself.
    let model = CheckboxModel::new("x");
    let _ = model.toggle(ChildSummary::LEAF);
    assert_eq!(model.state, CheckState::Unchecked);
}

// ---------------------------------------------------------------------------
// 3. The radio group
// ---------------------------------------------------------------------------

#[test]
fn roving_skips_disabled_options() {
    let group = RadioGroupModel::new(options());
    assert_eq!(group.enabled_indices(), vec![0, 2, 3]);

    assert_eq!(group.rove(Some(0), RoveDirection::Forward), Some(2));
    assert_eq!(group.rove(Some(2), RoveDirection::Backward), Some(0));
    assert_eq!(group.rove(Some(2), RoveDirection::Forward), Some(3));
}

#[test]
fn roving_wraps_at_both_ends() {
    let group = RadioGroupModel::new(options());
    assert_eq!(group.rove(Some(3), RoveDirection::Forward), Some(0));
    assert_eq!(group.rove(Some(0), RoveDirection::Backward), Some(3));
}

#[test]
fn roving_with_no_current_focus_lands_on_the_first_or_last_live_option() {
    let group = RadioGroupModel::new(options());
    assert_eq!(group.rove(None, RoveDirection::Forward), Some(0));
    assert_eq!(group.rove(None, RoveDirection::Backward), Some(3));
}

#[test]
fn roving_off_a_disabled_index_lands_on_the_next_live_option_in_direction() {
    // The focus can be left sitting on an option that later became
    // disabled; the next arrow must not strand the user there.
    let group = RadioGroupModel::new(options());
    assert_eq!(group.rove(Some(1), RoveDirection::Forward), Some(3));
    assert_eq!(group.rove(Some(1), RoveDirection::Backward), Some(0));
}

#[test]
fn roving_a_group_with_no_live_option_returns_none() {
    let all_disabled = RadioGroupModel::new(vec![
        RadioOption::new("a", "A").disabled(),
        RadioOption::new("b", "B").disabled(),
    ]);
    assert!(all_disabled.enabled_indices().is_empty());
    assert_eq!(all_disabled.rove(None, RoveDirection::Forward), None);
    assert_eq!(all_disabled.rove(Some(0), RoveDirection::Backward), None);

    let empty = RadioGroupModel::<&'static str>::new(Vec::new());
    assert_eq!(empty.rove(None, RoveDirection::Forward), None);
}

#[test]
fn a_disabled_group_has_no_live_options_whatever_its_members_declare() {
    let group = RadioGroupModel::new(options()).disabled();
    assert!(group.enabled_indices().is_empty());
    assert_eq!(group.rove(Some(0), RoveDirection::Forward), None);
}

#[test]
fn a_single_option_group_roves_onto_itself() {
    let group = RadioGroupModel::new(vec![RadioOption::new("only", "Only")]);
    assert_eq!(group.rove(None, RoveDirection::Forward), Some(0));
    assert_eq!(group.rove(Some(0), RoveDirection::Forward), Some(0));
    assert_eq!(group.rove(Some(0), RoveDirection::Backward), Some(0));
}

#[test]
fn a_rove_emits_the_selection_it_landed_on() {
    let group = RadioGroupModel::new(options());
    let roving = group
        .rove_and_select(Some(0), ToggleKey::Forward)
        .expect("there is a live option ahead");
    assert_eq!(roving.focus, 2);
    assert_eq!(*roving.selected.value(), "c");
}

#[test]
fn activate_is_not_a_rove_direction() {
    let group = RadioGroupModel::new(options());
    assert!(
        group
            .rove_and_select(Some(0), ToggleKey::Activate)
            .is_none()
    );
    assert_eq!(ToggleKey::Activate.rove_direction(), None);
    assert_eq!(
        ToggleKey::Forward.rove_direction(),
        Some(RoveDirection::Forward)
    );
    assert_eq!(
        ToggleKey::Backward.rove_direction(),
        Some(RoveDirection::Backward)
    );
}

#[test]
fn selecting_a_live_option_emits_it() {
    let group = RadioGroupModel::new(options());
    let emitted = group.select(&"c").expect("c is live and unselected");
    assert_eq!(*emitted.value(), "c");
}

#[test]
fn re_clicking_the_selected_option_is_a_no_op() {
    let group = RadioGroupModel::new(options()).with_selected("c");
    assert_eq!(group.selected_index(), Some(2));
    assert!(
        group.select(&"c").is_none(),
        "re-selecting must not give the host an event for a value that did not change"
    );
}

#[test]
fn selecting_a_disabled_or_unknown_option_emits_nothing() {
    let group = RadioGroupModel::new(options());
    assert!(group.select(&"b").is_none(), "b is a disabled option");
    assert!(group.select(&"zzz").is_none(), "not in the group at all");
}

#[test]
fn a_disabled_group_selects_nothing() {
    let group = RadioGroupModel::new(options()).disabled();
    assert!(group.select(&"a").is_none());
    assert!(!group.option_enabled(0));
}

#[test]
fn the_selected_index_tracks_the_list_and_survives_removal_lookups() {
    let group = RadioGroupModel::new(options()).with_selected("d");
    assert_eq!(group.selected_index(), Some(3));

    let stale = RadioGroupModel::new(vec![RadioOption::new("a", "A")]).with_selected("gone");
    assert_eq!(stale.selected_index(), None);
}

#[test]
fn the_orientation_is_carried_and_defaulted() {
    let group = RadioGroupModel::new(options());
    assert_eq!(group.orientation, RadioOrientation::Vertical);
    assert_eq!(
        group
            .clone()
            .with_orientation(RadioOrientation::HorizontalWrap)
            .orientation,
        RadioOrientation::HorizontalWrap
    );
    assert_eq!(RadioOrientation::default(), RadioOrientation::Vertical);
}

#[test]
fn the_rove_is_pure_and_does_not_touch_the_model() {
    let group = RadioGroupModel::new(options()).with_selected("a");
    let before = group.clone();
    let _ = group.rove(Some(0), RoveDirection::Forward);
    let _ = group.rove_and_select(Some(2), ToggleKey::Backward);
    assert_eq!(
        group, before,
        "a rove reports where to go; it does not go there itself"
    );
}

// ---------------------------------------------------------------------------
// 4. The switch
// ---------------------------------------------------------------------------

#[test]
fn a_new_switch_is_off_with_no_position_texts() {
    let model = SwitchModel::new("Auto-save");
    assert!(!model.on);
    assert_eq!(model.on_text, None);
    assert_eq!(model.off_text, None);
    assert!(!model.busy);
    assert!(!model.disabled);
}

#[test]
fn the_switch_builders_set_what_they_name() {
    let model = SwitchModel::new("x")
        .with_on(true)
        .with_texts("On", "Off")
        .busy()
        .disabled();
    assert!(model.on);
    assert_eq!(model.on_text.as_deref(), Some("On"));
    assert_eq!(model.off_text.as_deref(), Some("Off"));
    assert!(model.busy);
    assert!(model.disabled);
}

#[test]
fn an_idle_switch_toggles_immediately_in_both_directions() {
    let off = SwitchModel::new("x");
    assert!(*off.toggle().expect("idle switch emits").value());

    let on = SwitchModel::new("x").with_on(true);
    assert!(!*on.toggle().expect("idle switch emits").value());
}

#[test]
fn a_busy_switch_refuses_the_toggle() {
    let model = SwitchModel::new("x").busy();
    assert!(
        model.toggle().is_none(),
        "an immediate live toggle that fires during a round trip would emit the opposite \
         of what the host is about to report back"
    );
    assert!(model.pending().is_busy());
}

#[test]
fn a_disabled_switch_refuses_the_toggle() {
    let model = SwitchModel::new("x").disabled();
    assert!(model.toggle().is_none());
    assert!(!model.pending().is_busy());
}

#[test]
fn toggling_does_not_mutate_the_switch_model() {
    let model = SwitchModel::new("x");
    let _ = model.toggle();
    assert!(!model.on);
}

#[test]
fn the_switch_polarity_tracks_its_position() {
    assert_eq!(SwitchModel::new("x").polarity(), TogglePolarity::Off);
    assert_eq!(
        SwitchModel::new("x").with_on(true).polarity(),
        TogglePolarity::On
    );
    // Busy and disabled do not change the polarity: the track still shows
    // where the switch actually is.
    assert_eq!(
        SwitchModel::new("x").with_on(true).busy().polarity(),
        TogglePolarity::On
    );
}

#[test]
fn the_pending_projection_never_offers_a_cancellation() {
    // §3.3 gives the switch no `on_cancel`, so a cancellable projection
    // would advertise something the framework cannot honour.
    let busy = SwitchModel::new("x").busy().pending();
    assert!(busy.is_busy());
    assert!(
        busy.try_cancel().is_err(),
        "the switch must not grant a cancellation nobody implemented"
    );

    let idle = SwitchModel::new("x").pending();
    assert!(!idle.is_busy());
    assert!(idle.try_cancel().is_err());
}

#[test]
fn every_model_transition_is_deterministic() {
    let checkbox = CheckboxModel::new("x").with_state(CheckState::Indeterminate);
    for children in CHILDREN {
        let a = checkbox.toggle(children).map(|c| *c.value());
        let b = checkbox.toggle(children).map(|c| *c.value());
        assert_eq!(
            a, b,
            "checkbox toggle is not deterministic for {children:?}"
        );
    }

    let group = RadioGroupModel::new(options()).with_selected("a");
    for key in [ToggleKey::Activate, ToggleKey::Forward, ToggleKey::Backward] {
        for from in [None, Some(0), Some(1), Some(2), Some(3), Some(9)] {
            let a = group.rove_and_select(from, key);
            let b = group.rove_and_select(from, key);
            match (a, b) {
                (Some(x), Some(y)) => {
                    assert_eq!(x.focus, y.focus);
                    assert_eq!(x.selected.value(), y.selected.value());
                }
                (None, None) => {}
                _ => panic!("rove_and_select is not deterministic for {from:?} {key:?}"),
            }
        }
    }

    let switch = SwitchModel::new("x").with_on(true);
    assert_eq!(
        format!("{:?}", switch.toggle()),
        format!("{:?}", switch.toggle())
    );
}
