//! Sidecar unit tests for `ui_iced/about.rs` ([TASK-017b2]).
//!
//! Pure view-model, state-transition and widget-construction tests
//! ([RUST-003]). No renderer, no bus, no filesystem: every fixture is a
//! synthetic literal that is never opened, and the only real data read is
//! the compile-time build metadata and the `include_str!`-embedded license
//! ([RUST-006]).

use super::*;
use crate::ui::strings;
use iced::Element;

/// A fixture snapshot with known build metadata.
fn known_vm() -> AboutViewModel {
    AboutViewModel {
        app_name: strings::ABOUT_APP_NAME,
        version: "9.8.7",
        description: "a fixture description",
        copyright: strings::ABOUT_COPYRIGHT,
        branch: "feature/fixture",
        commit_hash: "0123456789abcdef0123456789abcdef01234567",
        commit_short_hash: "01234567",
        build_date: "2026-10-18",
        license_text: "MIT License\n\nPermission is hereby granted.\n\nThe rest.".to_string(),
        attribution_count: 3,
    }
}

/// A fixture snapshot as `build.rs` produces it when git is unavailable.
fn unknown_vm() -> AboutViewModel {
    AboutViewModel {
        branch: UNKNOWN_BUILD_METADATA,
        commit_hash: UNKNOWN_BUILD_METADATA,
        commit_short_hash: UNKNOWN_BUILD_METADATA,
        build_date: UNKNOWN_BUILD_METADATA,
        ..known_vm()
    }
}

// --- View-model: identity carried through construction ------------------

#[test]
fn from_build_carries_the_shared_identity_strings() {
    let vm = AboutViewModel::from_build();
    assert_eq!(vm.app_name, strings::ABOUT_APP_NAME);
    assert_eq!(vm.copyright, strings::ABOUT_COPYRIGHT);
    assert_eq!(vm.description, strings::ABOUT_DESCRIPTION);
    assert_eq!(vm.version, APP_VERSION);
}

#[test]
fn from_build_version_looks_like_a_version() {
    let vm = AboutViewModel::from_build();
    assert!(
        vm.version.contains('.') && !vm.version.starts_with('.') && !vm.version.ends_with('.'),
        "CARGO_PKG_VERSION should be a dotted version, got {:?}",
        vm.version
    );
    assert!(
        vm.version
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c.is_ascii_alphabetic()),
        "version should carry only version characters, got {:?}",
        vm.version
    );
}

#[test]
fn from_build_description_is_not_blank() {
    let vm = AboutViewModel::from_build();
    assert!(
        !vm.description.trim().is_empty(),
        "the manifest description must not be empty"
    );
}

#[test]
fn from_build_build_metadata_is_present() {
    let vm = AboutViewModel::from_build();
    for value in [
        vm.branch,
        vm.commit_hash,
        vm.commit_short_hash,
        vm.build_date,
    ] {
        assert!(!value.is_empty(), "build.rs always emits a non-empty value");
    }
    // Data-model invariant mirrored from `ui/about_dialog_tests.rs`: the
    // short hash is the leading run of the full hash, or both fall back to
    // "unknown" (spec FR-007).
    if vm.is_build_metadata_unknown() {
        assert_eq!(vm.commit_short_hash, UNKNOWN_BUILD_METADATA);
    } else {
        assert!(
            vm.commit_hash.starts_with(vm.commit_short_hash),
            "short hash {:?} should prefix full hash {:?}",
            vm.commit_short_hash,
            vm.commit_hash
        );
    }
}

#[test]
fn from_build_license_is_the_reflowed_mit_body() {
    let vm = AboutViewModel::from_build();
    assert!(
        vm.license_text.contains("MIT License"),
        "license body should carry the MIT header"
    );
    assert!(
        vm.license_text.contains("Permission is hereby granted"),
        "license body should carry the grant paragraph"
    );
    // Reflow contract: paragraph breaks survive, single newlines do not.
    let singles = vm.license_text.replace("\n\n", "").matches('\n').count();
    assert_eq!(singles, 0, "no hard-wrap single newlines may remain");
    assert!(
        vm.license_text.matches("\n\n").count() > 1,
        "paragraph breaks must survive the reflow"
    );
}

#[test]
fn from_build_attribution_count_matches_the_reused_catalog() {
    let vm = AboutViewModel::from_build();
    assert_eq!(vm.attribution_count, DIRECT_DEPENDENCIES.len());
    assert!(vm.attribution_count > 0, "the catalog is not empty");
}

// --- View-model: derived text -------------------------------------------

#[test]
fn version_line_uses_the_label_and_the_version() {
    let vm = known_vm();
    assert_eq!(
        vm.version_line(),
        format!("{} 9.8.7", strings::ABOUT_VERSION_LABEL)
    );
}

#[test]
fn commit_tooltip_reveals_full_hash_and_hint() {
    // Mirrors `ui::about_dialog_tests::commit_tooltip_reveals_full_hash_and_hint`
    // for the iced layer: the reveal text carries the full hash, the
    // centralised prefix and the copy hint ([GUD-002]).
    let tooltip = known_vm().commit_tooltip();
    assert!(tooltip.contains("0123456789abcdef0123456789abcdef01234567"));
    assert!(tooltip.contains(strings::ABOUT_FULL_COMMIT_PREFIX));
    assert!(tooltip.contains(strings::ABOUT_COPY_COMMIT_TOOLTIP));
}

#[test]
fn attributions_summary_uses_the_shared_formatter() {
    assert_eq!(
        known_vm().attributions_summary(),
        strings::format_attribution_count(3)
    );
}

// --- Copy affordance gating (spec FR-007) --------------------------------

#[test]
fn copy_press_is_offered_when_the_hash_is_known() {
    assert_eq!(
        commit_copy_press(false),
        Some(Message::About(AboutMessage::CopyCommitHashRequested))
    );
}

#[test]
fn copy_press_is_withheld_on_an_unknown_build() {
    // Nothing meaningful to copy, so the cell must not emit anything —
    // the legacy UI's `copy_commit_hash_to_output` returns early in the same case.
    assert_eq!(commit_copy_press(true), None);
    assert!(unknown_vm().is_build_metadata_unknown());
    assert!(!known_vm().is_build_metadata_unknown());
}

// --- Banner -------------------------------------------------------------

#[test]
fn banner_glyph_is_the_shared_phosphor_bolt() {
    // The `image`/`svg` iced features are not compiled in (DEC-011), so
    // the banner is the same Phosphor stand-in the top bar uses.
    assert_eq!(banner_glyph(), strings::ICON_LIGHTNING);
    let _mark: Element<'static, Message> = banner_mark();
}

// --- Card construction ---------------------------------------------------

#[test]
fn about_card_builds_for_known_and_unknown_metadata() {
    let known = known_vm();
    let unknown = unknown_vm();
    let known_card: Element<'_, Message> = about_card(&known);
    let unknown_card: Element<'_, Message> = about_card(&unknown);
    // Constructing both states must not panic; the unknown build differs
    // only in text and press handlers, never in widget count.
    let _ = (known_card, unknown_card);
}

#[test]
fn about_card_builds_from_the_build_snapshot() {
    let vm = AboutViewModel::from_build();
    let _card: Element<'_, Message> = vm.view();
}

#[test]
fn footer_and_close_button_build() {
    let _close: Element<'static, Message> = close_button();
    let _footer: Element<'static, Message> = about_footer();
}

// --- Window state --------------------------------------------------------

#[test]
fn default_window_state_is_closed() {
    let state = AboutWindowState::new();
    assert!(!state.is_open());
    assert!(!state.is_about());
    assert!(!state.is_attributions());
    assert_eq!(state.window(), None);
    assert_eq!(state, AboutWindowState::default());
}

#[test]
fn opening_a_window_reports_the_change() {
    let mut state = AboutWindowState::new();
    assert!(state.open_about());
    assert!(state.is_about());
    assert!(state.is_open());
    assert_eq!(state.window(), Some(AboutWindow::About));
}

#[test]
fn opening_the_same_window_again_reports_no_change() {
    let mut state = AboutWindowState::new();
    assert!(state.open_attributions());
    assert!(
        !state.open_attributions(),
        "a redundant open must not report a visible change"
    );
    assert!(state.is_attributions());
}

#[test]
fn opening_a_window_replaces_the_previous_one() {
    // One window at a time — the same rule `DialogState` and
    // `OverlaySlot::open_modal` apply.
    let mut state = AboutWindowState::new();
    state.open_about();
    assert!(state.open_attributions());
    assert!(state.is_attributions());
    assert!(!state.is_about());
}

#[test]
fn close_reports_whether_a_window_was_open() {
    let mut state = AboutWindowState::new();
    assert!(
        !state.close(),
        "closing an already-closed state consumes nothing"
    );
    state.open_about();
    assert!(state.close());
    assert!(!state.is_open());
    assert!(!state.close());
}

#[test]
fn apply_attributions_requested_opens_the_credits_window() {
    let mut state = AboutWindowState::new();
    assert!(state.apply(AboutMessage::AttributionsRequested));
    assert!(state.is_attributions());
    assert!(!state.open_attributions(), "already showing credits");
}

#[test]
fn apply_close_requested_closes_only_when_open() {
    let mut state = AboutWindowState::new();
    assert!(!state.apply(AboutMessage::CloseRequested));
    state.open_about();
    assert!(state.apply(AboutMessage::CloseRequested));
    assert!(!state.is_open());
}

#[test]
fn apply_deferred_intents_change_no_window_state() {
    // The clipboard write and the browser open are the reducer's job; the
    // window state is untouched and reports "no visible change".
    let mut state = AboutWindowState::new();
    state.open_about();
    assert!(!state.apply(AboutMessage::CopyCommitHashRequested));
    assert!(state.is_about());
    assert!(!state.apply(AboutMessage::RepositoryOpened {
        url: "https://github.com/dtolnay/anyhow".into(),
    }));
    assert!(state.is_about());
}

#[test]
fn window_titles_come_from_strings() {
    assert_eq!(AboutWindow::About.title(), strings::ABOUT_DIALOG_TITLE);
    assert_eq!(
        AboutWindow::Attributions.title(),
        strings::ABOUT_ATTRIBUTIONS_HEADER
    );
}

// --- Moved helpers ([TASK-029b]) ---------------------------------------
// Ported from the deleted legacy `about_dialog_tests.rs` sidecar so the
// owned helpers keep full coverage ([RUST-003]).

#[test]
fn license_reflow_preserves_words() {
    // The reflowed license must contain exactly the same words in the same
    // order (whitespace-only change), keep paragraph breaks, and leave no
    // single newlines that would pin the label to the file's hard wrap.
    let words_original: Vec<&str> = LICENSE_TEXT.split_whitespace().collect();
    let reflowed = license_display_text();
    let words_reflowed: Vec<&str> = reflowed.split_whitespace().collect();
    assert_eq!(words_reflowed, words_original);
    assert_eq!(
        reflowed.matches("\n\n").count(),
        LICENSE_TEXT.matches("\n\n").count(),
        "paragraph breaks must survive the reflow"
    );
    let singles = reflowed.replace("\n\n", "").matches('\n').count();
    assert_eq!(singles, 0, "no single newlines may remain");
}

#[test]
fn should_auto_show_about_none_means_first_run() {
    // Spec FR-016 / SC-008: fresh UI state (no recorded shown version)
    // must auto-show the About dialog.
    assert!(
        should_auto_show_about(None, "0.2.0"),
        "fresh state should auto-show"
    );
}

#[test]
fn should_auto_show_about_same_version_stays_quiet() {
    // Spec FR-016 / SC-008: same-version restarts must not re-display.
    assert!(
        !should_auto_show_about(Some("0.2.0"), "0.2.0"),
        "same version should stay quiet"
    );
}

#[test]
fn should_auto_show_about_upgrade_reopens_once() {
    // Spec FR-016 / SC-008: each upgrade's first start re-displays once.
    assert!(
        should_auto_show_about(Some("0.1.0"), "0.2.0"),
        "upgraded version should auto-show"
    );
}

#[test]
fn apply_first_run_auto_show_opens_and_records() {
    let mut persisted = crate::ui::PersistedUiState::default();
    let mut dialogs = crate::ui::Dialogs::new();
    assert!(apply_first_run_auto_show(
        &mut persisted,
        &mut dialogs,
        "0.2.0"
    ));
    assert!(dialogs.about_dialog_open);
    assert_eq!(persisted.about_shown_for_version.as_deref(), Some("0.2.0"));
    // Second run at the same version stays quiet and changes nothing.
    dialogs.about_dialog_open = false;
    assert!(!apply_first_run_auto_show(
        &mut persisted,
        &mut dialogs,
        "0.2.0"
    ));
    assert!(!dialogs.about_dialog_open);
}
