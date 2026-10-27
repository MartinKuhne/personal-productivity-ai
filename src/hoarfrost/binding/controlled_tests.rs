//! Tests for the §3.0.4.1 controlled / ephemeral split.
//!
//! The interesting assertions here are not "the getter returns what was put in".
//! They are that the *read-only* half really is read-only, that the ephemeral half
//! really is owned, and that the two never touch each other.

use crate::binding::controlled::Controlled;
use crate::binding::controlled::Ephemeral;

#[derive(Debug, PartialEq)]
struct FormModel {
    value: String,
    invalid: bool,
}

#[derive(Debug, PartialEq, Default)]
struct HoverState {
    hovered: bool,
    pressed: bool,
}

// ---------------------------------------------------------------------------
// Controlled: read-only by construction
// ---------------------------------------------------------------------------

#[test]
fn a_controlled_borrow_reads_the_model_it_was_handed() {
    let model = FormModel {
        value: "hello".to_string(),
        invalid: false,
    };
    let controlled = Controlled::new(&model);
    assert_eq!(controlled.get(), &model);
    assert_eq!(controlled.get().value, "hello");
}

#[test]
fn a_controlled_borrow_comes_from_a_plain_reference() {
    let model = FormModel {
        value: "x".to_string(),
        invalid: true,
    };
    let controlled = Controlled::from(&model);
    assert_eq!(controlled.get().value, "x");
    assert!(controlled.get().invalid);
}

#[test]
fn mapping_projects_a_sub_part_without_exposing_the_rest() {
    let model = FormModel {
        value: "hello".to_string(),
        invalid: true,
    };
    let controlled = Controlled::new(&model);
    let projected: Controlled<'_, bool> = controlled.map(|m| &m.invalid);
    assert!(*projected.get());
}

#[test]
fn a_mapped_borrow_still_borrows_the_original_for_its_whole_lifetime() {
    // Both borrows are alive at once and both read the same bytes; if `map` had
    // copied, the outer borrow would have been droppable mid-scope.
    let model = FormModel {
        value: "shared".to_string(),
        invalid: false,
    };
    let outer = Controlled::new(&model);
    let inner = outer.map(|m| &m.value);
    assert_eq!(inner.get().as_str(), "shared");
    assert_eq!(outer.get().value, "shared");
}

#[test]
fn a_controlled_borrow_is_copy_so_it_moves_freely_without_cloning_the_model() {
    let model = FormModel {
        value: "copy".to_string(),
        invalid: false,
    };
    let a = Controlled::new(&model);
    let b = a;
    assert_eq!(a.get().value, b.get().value);
}

#[test]
fn the_debug_rendering_shows_the_model_not_a_placeholder() {
    let model = FormModel {
        value: "shown".to_string(),
        invalid: false,
    };
    let text = format!("{:?}", Controlled::new(&model));
    assert!(text.contains("Controlled"), "got {text}");
    assert!(text.contains("shown"), "got {text}");
}

// ---------------------------------------------------------------------------
// Ephemeral: owned, mutable, resettable
// ---------------------------------------------------------------------------

#[test]
fn an_ephemeral_starts_from_an_explicit_value() {
    let state = Ephemeral::new(HoverState {
        hovered: true,
        pressed: false,
    });
    assert!(state.get().hovered);
    assert!(!state.get().pressed);
}

#[test]
fn an_ephemeral_starts_from_the_default_when_told_to() {
    let state = Ephemeral::<HoverState>::default();
    assert_eq!(state.get(), &HoverState::default());
    assert!(!state.get().hovered);
}

#[test]
fn an_ephemeral_comes_from_a_plain_value() {
    let state = Ephemeral::from(HoverState {
        hovered: false,
        pressed: true,
    });
    assert!(state.get().pressed);
}

#[test]
fn get_mut_writes_in_place_during_event_handling() {
    let mut state = Ephemeral::new(HoverState::default());
    state.get_mut().hovered = true;
    state.get_mut().pressed = true;
    assert!(state.get().hovered && state.get().pressed);
}

#[test]
fn set_returns_the_value_it_replaced() {
    let mut state = Ephemeral::new(HoverState {
        hovered: true,
        pressed: false,
    });
    let previous = state.set(HoverState {
        hovered: false,
        pressed: true,
    });
    assert!(previous.hovered, "the old value should come back out");
    assert!(!state.get().hovered);
    assert!(state.get().pressed);
}

#[test]
fn reset_returns_what_it_discarded_and_leaves_a_blank_slate() {
    let mut state = Ephemeral::new(HoverState {
        hovered: true,
        pressed: true,
    });
    let discarded = state.reset();
    assert!(discarded.hovered && discarded.pressed);
    assert_eq!(state.get(), &HoverState::default());
}

#[test]
fn resetting_an_already_blank_ephemeral_is_still_well_defined() {
    let mut state = Ephemeral::<HoverState>::default();
    let discarded = state.reset();
    assert_eq!(discarded, HoverState::default());
    assert_eq!(state.get(), &HoverState::default());
}

#[test]
fn an_ephemeral_round_trips_through_a_whole_interaction_without_any_app_input() {
    // The §3.0.4.1 latency claim, expressed as a test: every write here is made
    // and read back with nothing but the widget's own state in scope.
    let mut state = Ephemeral::<HoverState>::default();
    state.get_mut().hovered = true;
    assert!(
        state.get().hovered,
        "readable in the same scope it was written"
    );
    state.get_mut().pressed = true;
    assert!(state.get().pressed);
    let previous = state.set(HoverState::default());
    assert!(previous.hovered && previous.pressed);
    assert!(!state.get().hovered);
}
