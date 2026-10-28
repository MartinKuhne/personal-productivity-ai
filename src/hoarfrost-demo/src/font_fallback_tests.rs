//! Tests for [`crate::font_fallback`].

use std::path::{Path, PathBuf};

use crate::font_fallback::{
    candidate_fallback_font_paths, first_readable, load_fallback_font,
    load_fallback_font_with_path, override_candidate, register, register_from, registered_path,
    registration_attempted, suppression_requested,
};

fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

#[test]
fn an_override_value_becomes_the_only_override_candidate() {
    assert_eq!(override_candidate(Some("my.ttf")), paths(&["my.ttf"]));
}

#[test]
fn an_unset_or_empty_override_contributes_no_candidate() {
    assert!(override_candidate(None).is_empty());
    assert!(override_candidate(Some("")).is_empty());
}

#[test]
fn only_the_exact_value_one_suppresses_registration() {
    assert!(suppression_requested(Some("1")));
    assert!(!suppression_requested(None));
    assert!(!suppression_requested(Some("0")));
    assert!(!suppression_requested(Some("true")));
    assert!(!suppression_requested(Some("")));
}

#[test]
fn the_platform_list_is_not_empty_on_every_supported_os() {
    // `candidate_fallback_font_paths` is the override plus the platform
    // list, so a non-empty platform list is what makes the demo work with
    // no override set at all.
    let candidates = candidate_fallback_font_paths();
    let platform_only_len = candidates.len() - override_candidate(None).len();
    assert!(
        platform_only_len > 0,
        "no platform fallback candidates were contributed on this target"
    );
}

#[cfg(windows)]
#[test]
fn segoe_ui_symbol_is_the_first_windows_candidate() {
    // The ordering is the whole point of the Windows list: `seguisym.ttf`
    // covers all fourteen needed codepoints, `segoeui.ttf` and `arial.ttf`
    // only six each.
    let candidates = candidate_fallback_font_paths();
    let first = candidates.first().expect("Windows must offer candidates");
    let file = first
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let overridden = std::env::var(crate::font_fallback::FALLBACK_FONT_ENV).is_ok();
    if !overridden {
        assert_eq!(file, "seguisym.ttf", "candidate order drifted: {first:?}");
    }
}

#[test]
fn no_candidates_reads_as_no_font() {
    assert!(first_readable(&[]).is_none());
}

#[test]
fn an_unreadable_candidate_is_skipped_rather_than_fatal() {
    let dir = tempfile::tempdir().expect("a temp dir is available");
    let good = dir.path().join("good.ttf");
    std::fs::write(&good, b"font bytes").expect("the fixture must be writable");

    let missing = dir.path().join("missing.ttf");
    let result = first_readable(&[missing.clone(), good.clone()]);

    assert_eq!(
        result.map(|(path, _)| path),
        Some(good),
        "an unreadable path must not stop the scan"
    );
    assert!(!Path::new(&missing).exists());
}

#[test]
fn the_first_readable_candidate_wins_over_later_ones() {
    let dir = tempfile::tempdir().expect("a temp dir is available");
    let first = dir.path().join("first.ttf");
    let second = dir.path().join("second.ttf");
    std::fs::write(&first, b"first bytes").expect("writable");
    std::fs::write(&second, b"second bytes").expect("writable");

    let (path, bytes) = first_readable(&[first.clone(), second]).expect("both are readable");
    assert_eq!(path, first);
    assert_eq!(bytes, b"first bytes");
}

#[test]
fn the_bare_and_pathed_loaders_agree() {
    let with_path = load_fallback_font_with_path();
    let bare = load_fallback_font();
    assert_eq!(with_path.map(|(_path, bytes)| bytes), bare);
}

#[test]
fn registering_nothing_is_a_no_op_and_never_panics() {
    assert_eq!(register_from(&[]), None);
}

#[test]
fn registering_only_unreadable_paths_is_a_no_op() {
    let candidates = paths(&[
        "Z:/definitely/not/a/real/font.ttf",
        "/no/such/font/anywhere.ttf",
    ]);
    assert_eq!(register_from(&candidates), None);
}

#[test]
fn registering_a_readable_font_reports_that_font() {
    let dir = tempfile::tempdir().expect("a temp dir is available");
    let font = dir.path().join("fallback.ttf");
    let bytes = load_fallback_font().unwrap_or_else(|| b"not a real font".to_vec());
    std::fs::write(&font, bytes).expect("the fixture must be writable");

    assert_eq!(register_from(std::slice::from_ref(&font)), Some(font));
}

#[test]
fn register_is_idempotent_and_its_result_is_readable_afterwards() {
    let first = register();
    let second = register();

    assert_eq!(
        first, second,
        "register must return the same answer every time"
    );
    assert!(
        registration_attempted(),
        "calling register must leave the latch set even when it found nothing"
    );
    assert_eq!(
        registered_path(),
        first,
        "registered_path must report what register returned"
    );
}
