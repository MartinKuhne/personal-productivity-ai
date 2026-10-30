//! Tests for [`super::notification`] — the closed severity ladder, the
//! auto-dismiss gate that refuses the severities §3B.2 forbids a timer
//! for, and the dismissal taxonomy that separates an acknowledgement
//! from a disappearance (`UIF-COMP-002`).

use crate::composite::notification::{
    DEFAULT_AUTO_DISMISS_MS, DismissReason, NotificationAction, NotificationId, NotificationModel,
    Severity, effective_auto_dismiss_ms,
};
use crate::token::color::Palette;
use crate::token::contrast::{contrast_ratio, meets_text_contrast};

fn palette() -> Palette {
    Palette::dark()
}

/// A contrast ratio compared at the four decimal places the pinned figures
/// are written to.
fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 5e-4
}

// ---------------------------------------------------------------------------
// 1 — the ladder is closed and ordered
// ---------------------------------------------------------------------------

#[test]
fn the_ladder_has_exactly_the_four_severities_the_spec_names() {
    assert_eq!(Severity::ALL.len(), 4);
    assert_eq!(
        Severity::ALL,
        [
            Severity::Info,
            Severity::Success,
            Severity::Warning,
            Severity::Danger
        ]
    );
}

#[test]
fn every_severity_slug_is_distinct_and_lowercase() {
    let slugs: Vec<&str> = Severity::ALL.iter().map(|s| s.slug()).collect();
    let mut sorted = slugs.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        slugs.len(),
        sorted.len(),
        "slugs must be distinct: {slugs:?}"
    );
    for slug in &slugs {
        assert_eq!(
            slug.to_lowercase().as_str(),
            *slug,
            "slug {slug} must be lowercase"
        );
    }
}

#[test]
fn the_default_severity_is_info() {
    assert_eq!(Severity::default(), Severity::Info);
}

#[test]
fn every_severity_carries_a_non_empty_glyph() {
    for severity in Severity::ALL {
        assert!(
            !severity.glyph().is_empty(),
            "{:?} must carry a glyph: a colour-only severity cue would fail UIF-PRIN-005",
            severity
        );
    }
}

#[test]
fn the_glyphs_are_all_distinct() {
    let glyphs: Vec<&str> = Severity::ALL.iter().map(|s| s.glyph()).collect();
    let mut deduped = glyphs.clone();
    deduped.dedup();
    assert_eq!(
        glyphs.len(),
        deduped.len(),
        "glyphs must differ: {glyphs:?}"
    );
}

// ---------------------------------------------------------------------------
// 2 — the severity → token map
// ---------------------------------------------------------------------------

#[test]
fn each_severity_resolves_the_token_section_3b_2_names() {
    let p = palette();
    assert_eq!(Severity::Info.accent(&p), p.accent_text);
    assert_eq!(Severity::Success.accent(&p), p.status_success);
    assert_eq!(Severity::Warning.accent(&p), p.status_warning);
    assert_eq!(Severity::Danger.accent(&p), p.status_danger_text);
}

#[test]
fn danger_resolves_the_text_token_not_the_fill() {
    let p = palette();
    // `status.danger.fill` is a button background: RGB(216,28,20). Reading
    // a title in it against a raised bar would be unreadable, so §3B.2's
    // Danger is `.text`, and this pins that the two were not confused.
    assert_ne!(Severity::Danger.accent(&p), p.status_danger_fill);
    assert_eq!(Severity::Danger.accent(&p), p.status_danger_text);
}

#[test]
fn every_severity_accent_clears_the_text_floor_on_both_permitted_bar_fills() {
    let p = palette();
    for fill in [("raised", p.surface_raised), ("overlay", p.surface_overlay)] {
        for severity in Severity::ALL {
            let accent = severity.accent(&p);
            assert!(
                meets_text_contrast(accent, fill.1),
                "{severity:?} accent {} on {} measures {:.4}:1, under the 4.50:1 text floor",
                crate::gallery::rgb8(accent),
                fill.0,
                contrast_ratio(accent, fill.1),
            );
        }
    }
}

#[test]
fn the_recomputed_severity_ratios_on_raised_match_the_module_table() {
    let p = palette();
    let raised = p.surface_raised;
    let expected = [
        (Severity::Info, 10.0215_f32),
        (Severity::Success, 14.2825),
        (Severity::Warning, 6.4474),
        (Severity::Danger, 6.1812),
    ];
    for (severity, figure) in expected {
        let actual = contrast_ratio(severity.accent(&p), raised);
        assert!(
            approx(actual, figure),
            "{severity:?} on surface.raised recomputes to {actual:.4}:1, not {figure:.4}:1"
        );
    }
}

#[test]
fn the_recomputed_severity_ratios_on_overlay_match_the_module_table() {
    let p = palette();
    let overlay = p.surface_overlay;
    let expected = [
        (Severity::Info, 9.4891_f32),
        (Severity::Success, 13.5237),
        (Severity::Warning, 6.1048),
        (Severity::Danger, 5.8528),
    ];
    for (severity, figure) in expected {
        let actual = contrast_ratio(severity.accent(&p), overlay);
        assert!(
            approx(actual, figure),
            "{severity:?} on surface.overlay recomputes to {actual:.4}:1, not {figure:.4}:1"
        );
    }
}

#[test]
fn the_recomputed_ratios_agree_with_the_printed_token_table() {
    // §2.5 prints contrast vs `surface.base`, `surface.overlay` and
    // `surface.active` for each severity token. Recomputing all three and
    // comparing is the check that the palette this crate ships is the
    // palette that table was written against.
    let p = palette();
    let expected = [
        (
            Severity::Info,
            [("base", 10.66), ("overlay", 9.49), ("active", 7.66)],
        ),
        (
            Severity::Success,
            [("base", 15.19), ("overlay", 13.52), ("active", 10.91)],
        ),
        (
            Severity::Warning,
            [("base", 6.86), ("overlay", 6.10), ("active", 4.93)],
        ),
        (
            Severity::Danger,
            [("base", 6.57), ("overlay", 5.85), ("active", 4.72)],
        ),
    ];
    let surfaces = [
        ("base", p.surface_base),
        ("overlay", p.surface_overlay),
        ("active", p.surface_active),
    ];
    for (severity, figures) in expected {
        for (name, printed) in figures {
            let surface = surfaces
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, c)| *c)
                .expect("surface present");
            let recomputed = contrast_ratio(severity.accent(&p), surface);
            assert!(
                (recomputed - printed as f32).abs() < 0.005,
                "{severity:?} vs {name}: recomputed {recomputed:.4}:1 vs printed {printed}:1"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3 — the auto-dismiss gate
// ---------------------------------------------------------------------------

#[test]
fn only_info_and_success_may_auto_dismiss() {
    assert!(Severity::Info.may_auto_dismiss());
    assert!(Severity::Success.may_auto_dismiss());
    assert!(!Severity::Warning.may_auto_dismiss());
    assert!(!Severity::Danger.may_auto_dismiss());
}

#[test]
fn the_gate_refuses_a_timer_for_warning_and_danger_however_it_is_asked() {
    // The whole point of the function: the refusal is the framework's,
    // not the host's. Every non-zero request is refused.
    for requested in [1_u32, 5_000, 60_000, u32::MAX] {
        assert_eq!(
            effective_auto_dismiss_ms(Severity::Warning, Some(requested)),
            None,
            "Warning must never get a timer, not even {requested}ms"
        );
        assert_eq!(
            effective_auto_dismiss_ms(Severity::Danger, Some(requested)),
            None,
            "Danger must never get a timer, not even {requested}ms"
        );
    }
}

#[test]
fn the_gate_passes_through_what_info_and_success_ask_for() {
    for requested in [1_u32, 5_000, 60_000] {
        assert_eq!(
            effective_auto_dismiss_ms(Severity::Info, Some(requested)),
            Some(requested)
        );
        assert_eq!(
            effective_auto_dismiss_ms(Severity::Success, Some(requested)),
            Some(requested)
        );
    }
}

#[test]
fn the_gate_passes_a_request_of_zero_through_rather_than_inventing_one() {
    // `Some(0)` means "dismiss immediately", which is a real request.
    // Turning it into `None` would make the notification stick around
    // forever — the opposite of what was asked.
    assert_eq!(effective_auto_dismiss_ms(Severity::Info, Some(0)), Some(0));
    assert_eq!(
        effective_auto_dismiss_ms(Severity::Success, Some(0)),
        Some(0)
    );
}

#[test]
fn the_gate_passes_none_through_unchanged() {
    assert_eq!(effective_auto_dismiss_ms(Severity::Info, None), None);
    assert_eq!(effective_auto_dismiss_ms(Severity::Danger, None), None);
}

#[test]
fn the_default_window_is_five_seconds_for_the_two_that_may_time_out() {
    assert_eq!(DEFAULT_AUTO_DISMISS_MS, 5_000);
    assert_eq!(Severity::Info.default_auto_dismiss_ms(), Some(5_000));
    assert_eq!(Severity::Success.default_auto_dismiss_ms(), Some(5_000));
    assert_eq!(Severity::Warning.default_auto_dismiss_ms(), None);
    assert_eq!(Severity::Danger.default_auto_dismiss_ms(), None);
}

// ---------------------------------------------------------------------------
// 4 — the model
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_model_is_dismissible_with_no_action_and_no_timer() {
    let model: NotificationModel<&str> =
        NotificationModel::new("n1", Severity::Info, "Title", "Message");
    assert_eq!(model.id, NotificationId::new("n1"));
    assert_eq!(model.severity, Severity::Info);
    assert_eq!(model.title, "Title");
    assert_eq!(model.message, "Message");
    assert!(model.action.is_none());
    assert!(model.dismissible);
    assert_eq!(model.auto_dismiss_ms, None);
}

#[test]
fn the_builder_attaches_an_action_in_the_right_order() {
    let model = NotificationModel::new("n1", Severity::Success, "Saved", "All done")
        .with_action("Open file", "open:file.md");
    let action = model.action.expect("action attached");
    assert_eq!(action.label, "Open file");
    assert_eq!(action.id, "open:file.md");
}

#[test]
fn the_builder_toggles_dismissibility_and_requests_a_window() {
    let model: NotificationModel<&str> = NotificationModel::new("n1", Severity::Info, "T", "M")
        .with_dismissible(false)
        .with_auto_dismiss(2_500);
    assert!(!model.dismissible);
    assert_eq!(model.auto_dismiss_ms, Some(2_500));
}

#[test]
fn the_model_reports_what_it_will_get_not_what_it_asked_for() {
    let danger: NotificationModel<&str> =
        NotificationModel::new("d", Severity::Danger, "Failed", "Could not save")
            .with_auto_dismiss(5_000);
    assert_eq!(
        danger.auto_dismiss_ms,
        Some(5_000),
        "the request is still visible on the model"
    );
    assert_eq!(
        danger.effective_auto_dismiss_ms(),
        None,
        "and the effective window is refused"
    );

    let info: NotificationModel<&str> =
        NotificationModel::new("i", Severity::Info, "FYI", "Nothing happened")
            .with_auto_dismiss(5_000);
    assert_eq!(info.effective_auto_dismiss_ms(), Some(5_000));
}

#[test]
fn a_model_with_no_action_carries_none_rather_than_an_empty_one() {
    let model: NotificationModel<&str> = NotificationModel::new("n", Severity::Info, "T", "M");
    assert_eq!(model.action, None);
}

#[test]
fn an_action_keeps_its_label_and_id_together() {
    let action = NotificationAction::new("Retry", "retry-1");
    assert_eq!(action.label, "Retry");
    assert_eq!(action.id, "retry-1");
}

// ---------------------------------------------------------------------------
// 5 — identity
// ---------------------------------------------------------------------------

#[test]
fn a_notification_id_round_trips_from_every_source_form() {
    assert_eq!(NotificationId::new("abc").as_str(), "abc");
    assert_eq!(NotificationId::from("abc").as_str(), "abc");
    assert_eq!(NotificationId::from(String::from("abc")).as_str(), "abc");
    assert_eq!(NotificationId::new("abc"), NotificationId::from("abc"));
}

#[test]
fn two_ids_with_different_text_are_not_equal() {
    assert_ne!(NotificationId::new("a"), NotificationId::new("b"));
}

// ---------------------------------------------------------------------------
// 6 — the dismissal taxonomy
// ---------------------------------------------------------------------------

#[test]
fn the_three_user_exits_are_acknowledgements() {
    assert!(DismissReason::User.acknowledged());
    assert!(DismissReason::Action.acknowledged());
    assert!(DismissReason::Escape.acknowledged());
}

#[test]
fn the_two_invisible_exits_are_not_acknowledgements() {
    // A notification that vanished on a timer, or to make room for a
    // newer one, was never confirmed. A host that treated these as
    // acknowledgements would be claiming the user read something they
    // may have been away from the screen for.
    assert!(!DismissReason::Timeout.acknowledged());
    assert!(!DismissReason::Overflow.acknowledged());
}

#[test]
fn a_host_withdrawal_is_not_an_acknowledgement() {
    assert!(!DismissReason::Host.acknowledged());
}

#[test]
fn every_dismiss_reason_is_distinct() {
    let all = [
        DismissReason::User,
        DismissReason::Action,
        DismissReason::Timeout,
        DismissReason::Overflow,
        DismissReason::Escape,
        DismissReason::Host,
    ];
    let mut deduped = all.to_vec();
    deduped.dedup();
    assert_eq!(all.len(), deduped.len());
}

#[test]
fn no_severity_that_cannot_time_out_can_ever_be_dismissed_by_timeout() {
    // The cross-check between the two halves of the rule: the reason
    // `Timeout` exists is the severities that may auto-dismiss, and the
    // gate makes sure the others can never reach it.
    for severity in Severity::ALL {
        let reached_timeout = effective_auto_dismiss_ms(severity, Some(DEFAULT_AUTO_DISMISS_MS))
            .is_some()
            && severity.may_auto_dismiss();
        assert_eq!(
            reached_timeout,
            severity.may_auto_dismiss(),
            "{severity:?}: the gate and the ladder disagree about whether a timeout is reachable"
        );
    }
}
