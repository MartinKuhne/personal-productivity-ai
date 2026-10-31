//! Tests for the segmented-control data model — `UIF-ATOM-001b`,
//! §3.0.4.1.
//!
//! Five groups, in order:
//!
//! 1. **The selection transition.** Every row of the emission table,
//!    run against both modes, plus the refusals.
//! 2. **Purity.** The transition never writes through `&self`, and the
//!    same inputs give the same output.
//! 3. **The rove.** Delegates to the shared walk: skips disabled
//!    segments, wraps, survives a segment disabled under the focus.
//! 4. **The `ToggleButton`.** The one-segment strip behaves as a
//!    boolean in both directions, which is the property a
//!    `SingleSelect`-with-a-cap has to give it.
//! 5. **Set ordering.** The emission iterates in `Ord` order, so the
//!    same selection is the same value every time.

use super::*;

fn segments() -> Vec<SegmentItem<&'static str>> {
    vec![
        SegmentItem::new("a", "Alpha"),
        SegmentItem::new("b", "Beta").disabled(),
        SegmentItem::new("c", "Gamma"),
        SegmentItem::new("d", "Delta"),
    ]
}

fn selected(set: &[&'static str]) -> BTreeSet<&'static str> {
    set.iter().copied().collect()
}

// ---------------------------------------------------------------------------
// 1. The selection transition
// ---------------------------------------------------------------------------

#[test]
fn single_select_picks_an_unselected_segment() {
    let model = SegmentedModel::new(segments());
    let commit = model.toggle(&"c").expect("a live segment toggles");
    assert_eq!(commit.value(), &selected(&["c"]));
}

#[test]
fn single_select_replaces_the_previous_member() {
    let model = SegmentedModel::new(segments()).with_selected("a");
    let commit = model
        .toggle(&"d")
        .expect("a different live segment toggles");
    assert_eq!(
        commit.value(),
        &selected(&["d"]),
        "the old member is evicted, not added to"
    );
}

#[test]
fn single_select_re_clicking_the_selected_segment_emits_nothing() {
    let model = SegmentedModel::new(segments()).with_selected("a");
    assert!(
        model.toggle(&"a").is_none(),
        "a transition that changes nothing must not emit"
    );
}

#[test]
fn multi_select_adds_an_unselected_segment_and_keeps_the_rest() {
    let model = SegmentedModel::multi(segments()).with_selection(selected(&["a", "c"]));
    let commit = model.toggle(&"d").expect("a live segment toggles");
    assert_eq!(commit.value(), &selected(&["a", "c", "d"]));
}

#[test]
fn multi_select_removes_an_already_selected_segment() {
    let model = SegmentedModel::multi(segments()).with_selection(selected(&["a", "c"]));
    let commit = model
        .toggle(&"a")
        .expect("a selected live segment un-toggles");
    assert_eq!(commit.value(), &selected(&["c"]));
}

#[test]
fn multi_select_can_empty_the_selection() {
    let model = SegmentedModel::multi(segments()).with_selected("a");
    let commit = model
        .toggle(&"a")
        .expect("the last member still un-toggles");
    assert!(commit.value().is_empty());
}

#[test]
fn a_disabled_segment_never_emits_in_either_mode() {
    for model in [
        SegmentedModel::new(segments()),
        SegmentedModel::multi(segments()),
    ] {
        assert!(
            model.toggle(&"b").is_none(),
            "the disabled segment is inert"
        );
    }
}

#[test]
fn an_unknown_id_never_emits_in_either_mode() {
    for model in [
        SegmentedModel::new(segments()),
        SegmentedModel::multi(segments()),
    ] {
        assert!(
            model.toggle(&"zzz").is_none(),
            "an id not in the strip is inert"
        );
    }
}

#[test]
fn a_disabled_strip_emits_nothing_even_for_a_live_segment() {
    let live = SegmentedModel::new(segments());
    let dead = SegmentedModel::new(segments()).disabled();
    assert!(live.toggle(&"a").is_some(), "the same strip live does emit");
    assert!(
        dead.toggle(&"a").is_none(),
        "the strip flag dominates the segment flag"
    );
}

#[test]
fn toggle_index_addresses_the_same_transition() {
    let model = SegmentedModel::new(segments());
    let by_index = model.toggle_index(2).expect("index 2 is 'c'");
    let by_id = model.toggle(&"c").expect("'c' toggles");
    assert_eq!(by_index.value(), by_id.value());
}

#[test]
fn toggle_index_past_the_end_emits_nothing() {
    let model = SegmentedModel::new(segments());
    assert!(model.toggle_index(99).is_none());
}

#[test]
fn toggle_index_of_a_disabled_segment_emits_nothing() {
    let model = SegmentedModel::multi(segments());
    assert!(
        model.toggle_index(1).is_none(),
        "index 1 is the disabled segment"
    );
}

// ---------------------------------------------------------------------------
// 2. Purity
// ---------------------------------------------------------------------------

#[test]
fn toggling_does_not_mutate_the_model() {
    let before = SegmentedModel::new(segments()).with_selected("a");
    let after = before.clone();
    let _ = before.toggle(&"c");
    assert_eq!(before, after, "the model is host-owned and never written");
}

#[test]
fn the_same_inputs_produce_the_same_emission_twice() {
    let model = SegmentedModel::multi(segments()).with_selection(selected(&["a", "c"]));
    let first = model.toggle(&"d").expect("emits");
    let second = model.toggle(&"d").expect("emits again");
    assert_eq!(first.value(), second.value());
}

// ---------------------------------------------------------------------------
// 3. The rove
// ---------------------------------------------------------------------------

#[test]
fn roving_forward_skips_the_disabled_segment() {
    let model = SegmentedModel::new(segments());
    assert_eq!(model.rove(Some(0), RoveDirection::Forward), Some(2));
}

#[test]
fn roving_backward_skips_the_disabled_segment() {
    let model = SegmentedModel::new(segments());
    assert_eq!(model.rove(Some(2), RoveDirection::Backward), Some(0));
}

#[test]
fn roving_wraps_at_both_ends() {
    let model = SegmentedModel::new(segments());
    assert_eq!(model.rove(Some(3), RoveDirection::Forward), Some(0));
    assert_eq!(model.rove(Some(0), RoveDirection::Backward), Some(3));
}

#[test]
fn roving_from_nowhere_lands_on_the_end_of_the_roster_in_direction() {
    let model = SegmentedModel::new(segments());
    assert_eq!(model.rove(None, RoveDirection::Forward), Some(0));
    assert_eq!(model.rove(None, RoveDirection::Backward), Some(3));
}

#[test]
fn a_strip_with_no_live_segment_has_nowhere_to_rove() {
    let model = SegmentedModel::new(vec![
        SegmentItem::new("a", "Alpha").disabled(),
        SegmentItem::new("b", "Beta").disabled(),
    ]);
    assert_eq!(model.rove(Some(0), RoveDirection::Forward), None);
}

#[test]
fn a_disabled_strip_has_nowhere_to_rove() {
    let model = SegmentedModel::new(segments()).disabled();
    assert_eq!(model.rove(Some(0), RoveDirection::Forward), None);
}

#[test]
fn enabled_indices_are_the_live_ones_in_display_order() {
    let model = SegmentedModel::new(segments());
    assert_eq!(model.enabled_indices(), vec![0, 2, 3]);
}

// ---------------------------------------------------------------------------
// 4. The ToggleButton
// ---------------------------------------------------------------------------

#[test]
fn a_toggle_button_is_one_segment_under_multi_select() {
    let model = SegmentedModel::toggle_button(false);
    assert_eq!(model.segments.len(), 1);
    assert_eq!(model.segments[0].id, TOGGLE_SEGMENT_ID);
    // `MultiSelect`, not `SingleSelect`: a single-select strip treats
    // re-picking the member it already holds as a no-op, so a
    // one-segment strip built that way could be switched on and never
    // off. Multi-select over one segment flips between empty and full
    // and can never hold two — which is exactly a toggle.
    assert_eq!(model.mode, SelectionMode::MultiSelect);
    assert!(!model.is_on());
}

#[test]
fn a_toggle_button_seeded_on_reports_on() {
    let model = SegmentedModel::toggle_button(true);
    assert!(model.is_on());
    assert_eq!(model.selected_count(), 1);
}

#[test]
fn toggling_a_toggle_button_on_emits_the_segment_id() {
    let model = SegmentedModel::toggle_button(false);
    let commit = model
        .toggle(&TOGGLE_SEGMENT_ID)
        .expect("a live toggle button turns on");
    assert_eq!(commit.value(), &selected(&[TOGGLE_SEGMENT_ID]));
}

#[test]
fn toggling_a_toggle_button_off_emits_the_empty_set() {
    let model = SegmentedModel::toggle_button(true);
    let commit = model
        .toggle(&TOGGLE_SEGMENT_ID)
        .expect("a live toggle button turns off");
    assert!(
        commit.value().is_empty(),
        "SingleSelect caps the set but does not forbid the empty one"
    );
}

#[test]
fn a_toggle_button_round_trips_on_off_on() {
    let off = SegmentedModel::toggle_button(false);
    let on = off.toggle(&TOGGLE_SEGMENT_ID).expect("on");
    let back_off = off
        .clone()
        .with_selection(on.value().clone())
        .toggle(&TOGGLE_SEGMENT_ID)
        .expect("off");
    assert!(off.selected.is_empty());
    assert!(on.value().contains(TOGGLE_SEGMENT_ID));
    assert!(back_off.value().is_empty());
}

#[test]
fn a_disabled_toggle_button_answers_nothing() {
    let model = SegmentedModel::toggle_button(true).disabled();
    assert!(model.is_on(), "the position is still what the host set");
    assert!(model.toggle(&TOGGLE_SEGMENT_ID).is_none());
}

// ---------------------------------------------------------------------------
// 5. Set ordering
// ---------------------------------------------------------------------------

#[test]
fn the_emitted_set_iterates_in_ord_order_not_insertion_order() {
    let model = SegmentedModel::multi(segments())
        .with_selection(selected(&["d"]))
        .clone();
    let with_c = model.toggle(&"c").expect("adds c");
    let built_other_way = {
        let m = SegmentedModel::multi(segments())
            .with_selection(selected(&["c"]))
            .clone();
        m.toggle(&"d").expect("adds d")
    };
    assert_eq!(
        with_c.value(),
        built_other_way.value(),
        "the same two members are the same set whichever order they were added"
    );
    let collected: Vec<&&str> = with_c.value().iter().collect();
    assert_eq!(collected, vec![&"c", &"d"], "iteration is Ord order");
}

// ---------------------------------------------------------------------------
// Mode helpers
// ---------------------------------------------------------------------------

#[test]
fn the_modes_answer_the_two_questions_the_transition_asks() {
    assert!(SelectionMode::SingleSelect.replaces_selection());
    assert!(!SelectionMode::SingleSelect.allows_multiple());
    assert!(!SelectionMode::MultiSelect.replaces_selection());
    assert!(SelectionMode::MultiSelect.allows_multiple());
}

#[test]
fn single_select_is_the_default_mode() {
    assert_eq!(SelectionMode::default(), SelectionMode::SingleSelect);
}

#[test]
fn index_of_finds_a_member_and_misses_a_stranger() {
    let model = SegmentedModel::new(segments());
    assert_eq!(model.index_of(&"c"), Some(2));
    assert_eq!(model.index_of(&"zzz"), None);
}
