//! Tests for the progress data model — `UIF-ATOM-009`, §3.9.
//!
//! Three groups, in order:
//!
//! 1. **The fraction.** Every arithmetic edge: zero total, exact
//!    halves, over-delivery, and the indeterminate case where the
//!    question has no answer.
//! 2. **The transitions.** `with_*` copies that never touch the
//!    original, and the indeterminate model that refuses a completion
//!    it cannot represent.
//! 3. **The defaults.** Status, cancellation and label as they arrive
//!    before a host says otherwise.

use super::*;

/// Every status, in declaration order.
const STATUSES: [ProgressStatus; 3] = [
    ProgressStatus::Active,
    ProgressStatus::Paused,
    ProgressStatus::Error,
];

#[test]
fn a_determinate_bar_reports_its_fraction() {
    assert_eq!(ProgressModel::determinate(1, 4).fraction(), Some(0.25));
    assert_eq!(ProgressModel::determinate(2, 4).fraction(), Some(0.5));
    assert_eq!(ProgressModel::determinate(3, 4).fraction(), Some(0.75));
    assert_eq!(ProgressModel::determinate(4, 4).fraction(), Some(1.0));
}

#[test]
fn an_indeterminate_bar_has_no_fraction() {
    assert_eq!(ProgressModel::indeterminate().fraction(), None);
    assert!(ProgressModel::indeterminate().is_indeterminate());
    assert!(!ProgressModel::determinate(1, 1).is_indeterminate());
}

#[test]
fn a_zero_total_is_an_empty_bar_not_an_error() {
    assert_eq!(ProgressModel::determinate(0, 0).fraction(), Some(0.0));
    assert_eq!(ProgressModel::determinate(5, 0).fraction(), Some(0.0));
}

#[test]
fn over_delivery_clamps_rather_than_leaving_the_track() {
    assert_eq!(ProgressModel::determinate(5, 4).fraction(), Some(1.0));
    assert_eq!(
        ProgressModel::determinate(u64::MAX, 4).fraction(),
        Some(1.0)
    );
}

#[test]
fn the_fraction_never_leaves_the_unit_interval() {
    for completed in [0_u64, 1, 7, 99, 100, 101, 10_000] {
        for total in [1_u64, 2, 99, 100, 101] {
            let f = ProgressModel::determinate(completed, total)
                .fraction()
                .expect("determinate has a fraction");
            assert!(
                (0.0..=1.0).contains(&f),
                "{completed}/{total} produced {f}, outside 0.0..=1.0"
            );
        }
    }
}

#[test]
fn with_completed_moves_the_position_and_keeps_the_total() {
    let model = ProgressModel::determinate(1, 8).with_completed(6);

    assert_eq!(
        model.mode(),
        ProgressMode::Determinate {
            completed: 6,
            total: 8
        }
    );
    assert_eq!(model.fraction(), Some(0.75));
}

#[test]
fn with_completed_on_an_indeterminate_model_changes_nothing() {
    let model = ProgressModel::indeterminate().with_completed(50);

    assert_eq!(model.mode(), ProgressMode::Indeterminate);
    assert_eq!(model.fraction(), None);
}

#[test]
fn with_completed_leaves_the_original_alone() {
    let original = ProgressModel::determinate(1, 8);
    let advanced = original.clone().with_completed(7);

    assert_eq!(original.fraction(), Some(0.125));
    assert_eq!(advanced.fraction(), Some(0.875));
}

#[test]
fn with_mode_replaces_the_mode_wholesale() {
    let model = ProgressModel::determinate(3, 4).with_mode(ProgressMode::Indeterminate);
    assert!(model.is_indeterminate());

    let back = model.with_mode(ProgressMode::Determinate {
        completed: 1,
        total: 2,
    });
    assert_eq!(back.fraction(), Some(0.5));
}

#[test]
fn a_bar_arrives_active_and_not_cancellable() {
    for model in [
        ProgressModel::determinate(0, 10),
        ProgressModel::indeterminate(),
    ] {
        assert_eq!(model.status(), ProgressStatus::Active);
        assert!(!model.cancellable());
        assert_eq!(model.label(), None);
    }
}

#[test]
fn status_defaults_to_active() {
    assert_eq!(ProgressStatus::default(), ProgressStatus::Active);
}

#[test]
fn only_active_work_is_moving() {
    assert!(ProgressStatus::Active.is_moving());
    assert!(!ProgressStatus::Paused.is_moving());
    assert!(!ProgressStatus::Error.is_moving());
}

#[test]
fn the_label_and_the_cancellation_flag_are_independent() {
    let model = ProgressModel::indeterminate()
        .with_label("Indexing")
        .with_cancellable(true);

    assert_eq!(model.label(), Some("Indexing"));
    assert!(model.cancellable());

    let other = ProgressModel::indeterminate().with_cancellable(true);
    assert_eq!(other.label(), None);
    assert!(other.cancellable());
}

#[test]
fn every_status_round_trips_through_every_mode() {
    for status in STATUSES {
        for model in [
            ProgressModel::determinate(2, 4).with_status(status),
            ProgressModel::indeterminate().with_status(status),
        ] {
            assert_eq!(model.status(), status);
            assert_eq!(
                model.clone().with_status(status),
                model,
                "setting the same status twice must be a no-op"
            );
        }
    }
}

#[test]
fn models_are_value_equal() {
    assert_eq!(
        ProgressModel::determinate(2, 4),
        ProgressModel::determinate(2, 4)
    );
    assert_ne!(
        ProgressModel::determinate(2, 4),
        ProgressModel::determinate(2, 5)
    );
    assert_ne!(
        ProgressModel::determinate(2, 4).with_status(ProgressStatus::Paused),
        ProgressModel::determinate(2, 4).with_status(ProgressStatus::Error)
    );
}
