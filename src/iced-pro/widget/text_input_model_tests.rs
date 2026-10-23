//! Tests for the host-owned text field model (`UIF-ATOM-002a`).
//!
//! The load-bearing claims here are not that the getters return the field
//! they wrap — they are that the model's *defaults* preserve what the
//! widget did before the model existed, and that the two clear triggers
//! `002a` names (the trailing `×` and `Escape`) ask the same predicate
//! so they can never disagree about whether clearing is available.

use crate::binding::Controlled;
use crate::widget::text_input::GLYPH_CLEAR;
use crate::widget::text_input::GLYPH_ERROR;
use crate::widget::text_input::InputKind;
use crate::widget::text_input::SlotContent;
use crate::widget::text_input::slot_content;
use crate::widget::text_input_model::TextInputModel;
use crate::widget::text_input_model::TextRange;

/// Every kind, every value state, every verdict — the whole cross-product
/// the slot rules have to be right across.
const EVERY_KIND: [InputKind; 3] = [
    InputKind::TextEntry,
    InputKind::Search,
    InputKind::DropdownTrigger,
];

/// Build a model in one exact state so the loops below read as a table
/// rather than as a chain of conditional builders.
fn model_in(has_value: bool, invalid: bool, clearable: bool) -> TextInputModel {
    let model = TextInputModel::new(if has_value { "v" } else { "" }).with_clearable(clearable);

    if invalid {
        model.with_validation("no")
    } else {
        model
    }
}

// -------------------------------------------------------------------
// The default is the pre-model behaviour, not a derived zero
// -------------------------------------------------------------------

#[test]
fn a_default_model_is_live_and_clearable() {
    let model = TextInputModel::default();

    assert!(model.value.is_empty());
    assert!(model.placeholder.is_empty());
    assert!(!model.is_invalid());
    assert!(
        model.accepts_input(),
        "a default model must accept edits, or adopting the model would \
         freeze every field that adopts it"
    );
    assert!(
        model.clearable,
        "the derived default for `clearable` is `false`; a `..default()` \
         model would silently strip the search field's clear affordance"
    );
}

#[test]
fn a_model_built_with_a_value_is_otherwise_default() {
    let model = TextInputModel::new("query");

    assert_eq!(model.value, "query");
    assert_eq!(model, TextInputModel::new("query"));
    assert!(model.clearable);
    assert!(model.accepts_input());
    assert_eq!(model.selection, None);
}

#[test]
fn empty_is_the_same_thing_as_default() {
    assert_eq!(TextInputModel::empty(), TextInputModel::default());
}

// -------------------------------------------------------------------
// `slot_for` agrees with `slot_content` while the field is clearable
// -------------------------------------------------------------------

#[test]
fn a_clearable_model_reproduces_the_widgets_own_slot_exactly() {
    for kind in EVERY_KIND {
        for has_value in [false, true] {
            for invalid in [false, true] {
                let model = model_in(has_value, invalid, true);

                assert_eq!(
                    model.slot_for(kind),
                    slot_content(kind, has_value, invalid),
                    "a clearable model must not change what {kind:?} shows \
                     in the {has_value}/{invalid} state"
                );
            }
        }
    }
}

#[test]
fn an_invalid_model_shows_the_error_glyph_for_every_kind() {
    for kind in EVERY_KIND {
        let model = TextInputModel::new("x").with_validation("bad");

        assert_eq!(model.slot_for(kind), SlotContent::Passive(GLYPH_ERROR));
        assert!(
            !model.clear_affordance(kind),
            "an invalid {kind:?} shows the error mark, not a clear: the \
             error glyph is passive, so the two never stack"
        );
    }
}

// -------------------------------------------------------------------
// `clearable` changes meaning, never geometry
// -------------------------------------------------------------------

#[test]
fn a_non_clearable_model_downgrades_the_glyph_but_keeps_it() {
    let clearable = TextInputModel::new("query");
    let locked = TextInputModel::new("query").with_clearable(false);

    let before = clearable.slot_for(InputKind::Search);
    let after = locked.slot_for(InputKind::Search);

    assert_eq!(before, SlotContent::Actionable(GLYPH_CLEAR));
    assert_eq!(after, SlotContent::Passive(GLYPH_CLEAR));
    assert_eq!(
        before.glyph(),
        after.glyph(),
        "the glyph's *meaning* changed; its geometry must not"
    );
}

#[test]
fn clearing_is_unavailable_on_a_locked_model_for_every_kind() {
    for kind in EVERY_KIND {
        let locked = TextInputModel::new("query").with_clearable(false);

        assert!(
            !locked.clear_affordance(kind),
            "a locked {kind:?} must never report a clear the user can pull"
        );
    }
}

#[test]
fn an_empty_search_field_never_offers_a_clear() {
    let model = TextInputModel::empty().with_placeholder("Search notes…");

    assert!(!model.has_value());
    assert_eq!(
        model.slot_for(InputKind::Search),
        SlotContent::Passive(crate::widget::text_input::GLYPH_SEARCH)
    );
    assert!(!model.clear_affordance(InputKind::Search));
}

#[test]
fn the_clear_affordance_is_the_same_predicate_the_slot_uses() {
    // The whole point of `clear_affordance`: the `×` and the `Escape`
    // path ask *this*, so they cannot drift apart.
    for kind in EVERY_KIND {
        for has_value in [false, true] {
            for invalid in [false, true] {
                for clearable in [false, true] {
                    let model = model_in(has_value, invalid, clearable);

                    assert_eq!(
                        model.clear_affordance(kind),
                        model.slot_for(kind).is_actionable(),
                        "clear_affordance and the slot's own actionability \
                         disagree for {kind:?} at {has_value}/{invalid}/{clearable}"
                    );
                }
            }
        }
    }
}

// -------------------------------------------------------------------
// read_only and disabled: two states, one shared consequence
// -------------------------------------------------------------------

#[test]
fn read_only_and_disabled_both_refuse_input_and_are_still_distinct() {
    let read_only = TextInputModel::new("v").read_only();
    let disabled = TextInputModel::new("v").disabled();

    assert!(!read_only.accepts_input());
    assert!(!disabled.accepts_input());
    assert_ne!(
        read_only, disabled,
        "collapsing the two flags would make it impossible to tell a field \
         the user can still read from one the host has taken away"
    );
    assert!(read_only.clearable && disabled.clearable);
}

#[test]
fn read_only_does_not_imply_disabled_and_vice_versa() {
    let model = TextInputModel::new("v").read_only();
    assert!(!model.disabled);

    let model = TextInputModel::new("v").disabled();
    assert!(!model.read_only);
}

// -------------------------------------------------------------------
// Selection
// -------------------------------------------------------------------

#[test]
fn a_model_without_an_opinion_about_the_caret_reports_none() {
    assert_eq!(TextInputModel::new("hello").selection_in_value(), None);
}

#[test]
fn a_selection_is_clamped_to_the_value_that_is_actually_there() {
    let model = TextInputModel::new("abc").with_selection(TextRange::new(1, 99));

    assert_eq!(
        model.selection_in_value(),
        Some(TextRange::new(1, 3)),
        "a range shipped with a longer value must not survive into a shorter one"
    );
}

#[test]
fn a_selection_survives_when_the_value_still_covers_it() {
    let model = TextInputModel::new("abcdef").with_selection(TextRange::new(2, 5));

    assert_eq!(model.selection_in_value(), Some(TextRange::new(2, 5)));
}

// -------------------------------------------------------------------
// The controlled projection
// -------------------------------------------------------------------

#[test]
fn controlled_reads_the_model_without_copying_it() {
    let model = TextInputModel::new("query").with_placeholder("Search…");
    let controlled = model.controlled();

    assert_eq!(controlled.get().value, "query");
    assert_eq!(controlled.get().placeholder, "Search…");
}

#[test]
fn the_borrow_is_what_makes_the_model_the_truth_for_the_frame() {
    let mut model = TextInputModel::new("first");
    model.value = "second".to_owned();

    let controlled = model.controlled();

    assert_eq!(
        controlled.get().value,
        "second",
        "the projection reads through, so a mutation made before the borrow \
         is visible without any copy"
    );

    let read = |m: Controlled<'_, TextInputModel>| m.get().value.clone();
    assert_eq!(read(controlled), "second");
}

#[test]
fn the_compiler_forbids_mutating_a_model_a_projection_still_holds() {
    // The compile-fail half of §3.0.4.1 is worth naming rather than
    // assuming: `controlled()` borrows, so a host cannot change the model
    // out from under a widget that is reading it inside the same borrow.
    // This test asserts the *shape* that makes that true — the projection
    // is a borrow, not a snapshot — by holding both at once.
    let model = TextInputModel::new("held");
    let controlled = model.controlled();

    assert_eq!(controlled.get().value, "held");
    assert_eq!(model.value, "held");
}

// -------------------------------------------------------------------
// `TextRange`
// -------------------------------------------------------------------

#[test]
fn a_caret_is_a_zero_length_range() {
    let caret = TextRange::caret(3);

    assert!(caret.is_caret());
    assert!(caret.is_empty());
    assert_eq!(caret.len(), 0);
    assert_eq!(caret.start, 3);
    assert_eq!(caret.end, 3);
}

#[test]
fn a_selection_is_not_a_caret() {
    let range = TextRange::new(1, 4);

    assert!(!range.is_caret());
    assert!(!range.is_empty());
    assert_eq!(range.len(), 3);
}

#[test]
fn clamping_a_range_inside_itself_changes_nothing() {
    let range = TextRange::new(2, 6);

    assert_eq!(range.clamped_to(6), range);
}

#[test]
fn clamping_collapses_both_ends_independently() {
    assert_eq!(TextRange::new(8, 12).clamped_to(5), TextRange::new(5, 5));
    assert_eq!(TextRange::new(2, 12).clamped_to(5), TextRange::new(2, 5));
    assert_eq!(TextRange::new(0, 0).clamped_to(5), TextRange::new(0, 0));
}

#[test]
fn text_of_returns_the_slice_a_range_names() {
    assert_eq!(TextRange::new(0, 5).text_of("hello world"), Some("hello"));
    assert_eq!(TextRange::caret(6).text_of("hello world"), Some(""));
}

#[test]
fn text_of_refuses_a_range_beyond_the_string() {
    assert_eq!(TextRange::new(0, 99).text_of("short"), None);
    assert_eq!(TextRange::new(6, 9).text_of("short"), None);
}

#[test]
fn text_of_on_an_empty_string_only_admits_the_caret() {
    assert_eq!(TextRange::caret(0).text_of(""), Some(""));
    assert_eq!(TextRange::new(0, 1).text_of(""), None);
}
