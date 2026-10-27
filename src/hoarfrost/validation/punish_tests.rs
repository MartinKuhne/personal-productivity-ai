//! Tests for the punish-late decision — the whole of 'Reward Early,
//! Punish Late' as one pure function (`UIF-PRIN-009`).

use crate::widget::text_area_model::TextAreaModel;
use crate::widget::text_input_model::TextInputModel;

use super::PunishReason;
use super::punish;
use crate::validation::disallowed::DisallowedClass;
use crate::validation::stance::EditStance;

use std::fs;

/// The three inputs one row of the table reads.
type PunishCase = (EditStance, bool, bool);

/// What that row must produce: `(painted, reported_invalid, reason)`.
type PunishExpectation = (bool, bool, PunishReason);

/// The whole truth table, written out rather than derived.
///
/// Eight rows because that is every combination of the three inputs, and
/// the table being exhaustive is the point: a row nobody wrote down is a
/// row nobody checked.
const TRUTH_TABLE: [(PunishCase, PunishExpectation); 8] = [
    (
        (EditStance::Pristine, false, false),
        (false, false, PunishReason::Valid),
    ),
    (
        (EditStance::Pristine, false, true),
        (true, true, PunishReason::DisallowedImmediate),
    ),
    (
        (EditStance::Pristine, true, false),
        (false, true, PunishReason::WithheldPristine),
    ),
    (
        (EditStance::Pristine, true, true),
        (true, true, PunishReason::DisallowedImmediate),
    ),
    (
        (EditStance::Touched, false, false),
        (false, false, PunishReason::Valid),
    ),
    (
        (EditStance::Touched, false, true),
        (true, true, PunishReason::DisallowedImmediate),
    ),
    (
        (EditStance::Touched, true, false),
        (true, true, PunishReason::TouchedInvalid),
    ),
    (
        (EditStance::Touched, true, true),
        (true, true, PunishReason::DisallowedImmediate),
    ),
];

// -- the table --------------------------------------------------------------

#[test]
fn the_table_covers_every_combination_of_the_three_inputs() {
    let mut seen = 0;
    for stance in EditStance::ALL {
        for invalid in [false, true] {
            for disallowed in [false, true] {
                seen += 1;
                assert!(
                    TRUTH_TABLE
                        .iter()
                        .any(|row| row.0 == (stance, invalid, disallowed)),
                    "no row for {stance} / invalid={invalid} / disallowed={disallowed}"
                );
            }
        }
    }
    assert_eq!(seen, TRUTH_TABLE.len());
}

#[test]
fn every_row_of_the_truth_table_holds() {
    for ((stance, invalid, disallowed), (painted, reported, reason)) in TRUTH_TABLE {
        let outcome = punish(stance, invalid, disallowed);
        assert_eq!(
            (outcome.painted(), outcome.invalid(), outcome.reason()),
            (painted, reported, reason),
            "punish({stance}, invalid={invalid}, disallowed={disallowed})"
        );
    }
}

#[test]
fn the_reported_invalidity_is_the_host_fact_or_the_refused_character() {
    for ((stance, invalid, disallowed), _) in TRUTH_TABLE {
        let outcome = punish(stance, invalid, disallowed);
        assert_eq!(
            outcome.invalid(),
            invalid || disallowed,
            "the reported fact must be `invalid || disallowed` for {stance} / \
             {invalid} / {disallowed}"
        );
    }
}

#[test]
fn painted_always_agrees_with_the_reason() {
    for ((stance, invalid, disallowed), _) in TRUTH_TABLE {
        let outcome = punish(stance, invalid, disallowed);
        assert_eq!(
            outcome.painted(),
            outcome.reason().paints(),
            "the reason and the paint disagree for {stance} / {invalid} / {disallowed}"
        );
    }
}

// -- Punish Late ------------------------------------------------------------

#[test]
fn a_pristine_invalid_field_paints_nothing() {
    let outcome = punish(EditStance::Pristine, true, false);
    assert!(!outcome.painted());
    assert!(!outcome.shows_error_border());
    assert!(!outcome.shows_error_caption());
    assert_eq!(outcome.reason(), PunishReason::WithheldPristine);
}

#[test]
fn withholding_the_visual_does_not_forget_the_fact() {
    let outcome = punish(EditStance::Pristine, true, false);
    assert!(
        outcome.invalid(),
        "the pillar withholds the *picture*, not the knowledge"
    );
    assert!(
        outcome.blocks_submit(),
        "a pristine-but-invalid field must still stop `Ok`"
    );
    assert!(outcome.withheld());
}

#[test]
fn a_touched_invalid_field_paints() {
    let outcome = punish(EditStance::Touched, true, false);
    assert!(outcome.painted());
    assert!(outcome.shows_error_border());
    assert!(outcome.shows_error_caption());
    assert_eq!(outcome.reason(), PunishReason::TouchedInvalid);
}

#[test]
fn only_the_pristine_row_is_withheld() {
    for ((stance, invalid, disallowed), _) in TRUTH_TABLE {
        let outcome = punish(stance, invalid, disallowed);
        assert_eq!(
            outcome.withheld(),
            matches!(stance, EditStance::Pristine) && invalid && !disallowed,
            "withheld() fired for {stance} / {invalid} / {disallowed}"
        );
    }
}

// -- the carve-out ----------------------------------------------------------

#[test]
fn a_disallowed_character_punishes_while_pristine() {
    let outcome = punish(EditStance::Pristine, false, true);
    assert!(
        outcome.painted(),
        "a slash in a basename must go red on the keystroke that typed it"
    );
    assert_eq!(outcome.reason(), PunishReason::DisallowedImmediate);
}

#[test]
fn the_carve_out_outranks_the_pristine_shield_and_the_host_verdict() {
    // Disallowed + pristine + host-says-valid, and disallowed + touched +
    // host-says-invalid, both land on the same reason: nothing outranks it.
    assert_eq!(
        punish(EditStance::Pristine, false, true).reason(),
        PunishReason::DisallowedImmediate
    );
    assert_eq!(
        punish(EditStance::Touched, true, true).reason(),
        PunishReason::DisallowedImmediate
    );
    assert_eq!(
        punish(EditStance::Pristine, true, true).reason(),
        PunishReason::DisallowedImmediate
    );
}

// -- Reward Early -----------------------------------------------------------

#[test]
fn recovery_clears_on_the_exact_frame_the_value_becomes_valid() {
    let mut model = TextInputModel::new("dup.txt").with_validation("That name is taken.");
    model.note_commit();
    assert!(model.punishment().painted());

    // The very next call, with nothing else changed but the value and the
    // host's verdict, must already be clean. No decay, no latch.
    model.value = "unique.txt".to_string();
    model.validation = None;
    let outcome = model.punishment();
    assert!(
        !outcome.painted(),
        "the invalid visual survived the frame the value became valid"
    );
    assert_eq!(outcome.reason(), PunishReason::Valid);
}

#[test]
fn recovery_clears_the_carve_out_path_too() {
    let mut model = TextInputModel::new("bad/name").with_stance(EditStance::Pristine);
    assert!(
        model
            .punishment_under(DisallowedClass::FilenameBasename)
            .painted()
    );

    model.value = "good_name".to_string();
    assert!(
        !model
            .punishment_under(DisallowedClass::FilenameBasename)
            .painted(),
        "removing the refused character must clear on the same keystroke that \
         removed it"
    );
}

#[test]
fn a_latched_visual_is_impossible_because_there_is_nothing_to_latch() {
    // The same pristine-invalid inputs, called a hundred times, never
    // start painting. 'Punish Late' is not a timer that eventually
    // expires.
    for _ in 0..100 {
        assert!(!punish(EditStance::Pristine, true, false).painted());
    }
}

// -- purity ---------------------------------------------------------------

#[test]
fn the_decision_is_deterministic() {
    for ((stance, invalid, disallowed), _) in TRUTH_TABLE {
        let a = punish(stance, invalid, disallowed);
        let b = punish(stance, invalid, disallowed);
        assert_eq!(a, b);
    }
}

#[test]
fn no_module_in_this_subsystem_reads_a_clock() {
    for file in [
        "validation/mod.rs",
        "validation/stance.rs",
        "validation/disallowed.rs",
        "validation/punish.rs",
        "validation/recovery.rs",
    ] {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/validation/mod.rs");
        let path = path.replace("validation/mod.rs", file);
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{file}: {e}"));
        let code: String = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in ["Instant::now", "SystemTime", "unix_timestamp"] {
            assert!(
                !code.contains(forbidden),
                "{file} reads a clock via {forbidden}; time must enter as a value"
            );
        }
    }
}

// -- reason metadata --------------------------------------------------------

#[test]
fn every_reason_is_listed_and_paints_as_documented() {
    assert_eq!(PunishReason::ALL.len(), 4);
    assert!(!PunishReason::Valid.paints());
    assert!(!PunishReason::WithheldPristine.paints());
    assert!(PunishReason::DisallowedImmediate.paints());
    assert!(PunishReason::TouchedInvalid.paints());
}

#[test]
fn reasons_render_as_their_own_names() {
    for reason in PunishReason::ALL {
        assert_eq!(reason.to_string(), format!("{reason:?}"));
    }
}

// -- model integration ------------------------------------------------------

#[test]
fn a_pristine_field_with_no_class_never_paints() {
    let model = TextInputModel::new("x").with_validation("nope");
    assert!(!model.punishment().painted());
    assert!(model.punishment().blocks_submit());
}

#[test]
fn a_wells_carve_out_reads_its_own_buffer() {
    let mut model = TextAreaModel::new();
    model.set_text("fine");
    assert!(
        !model
            .punishment_under(DisallowedClass::FilenameBasename)
            .painted()
    );
    model.set_text("still/fine");
    assert!(
        model
            .punishment_under(DisallowedClass::FilenameBasename)
            .painted()
    );
}

#[test]
fn punishment_and_punishment_under_differ_only_by_the_carve_out() {
    let model = TextInputModel::new("clean").with_validation("taken");
    // No refused character: the class changes nothing.
    assert_eq!(
        model.punishment(),
        model.punishment_under(DisallowedClass::FilenameBasename)
    );
}

#[test]
fn a_punishment_is_a_small_copyable_value() {
    let outcome = punish(EditStance::Touched, true, false);
    let copied = outcome;
    assert_eq!(outcome, copied);
    assert_eq!(outcome.painted(), copied.painted());
}
