//! Tests for [`BrowserSession`] (`agent::session::browser_session`).

use super::*;
use crate::agent::tools::browser::BrowserAutomationExt;
use crate::config::{AppConfig, BrowserConfig, ContentLibrary};
use std::path::PathBuf;

fn empty_resolved(dir: PathBuf) -> ResolvedBrowserConfig {
    let storage_path = dir.join("storage.json");
    ResolvedBrowserConfig {
        screenshot_dir: dir,
        headless: true,
        browser_type: "firefox".to_string(),
        idle_timeout_seconds: 60,
        page_load_timeout_ms: 30_000,
        storage_state_path: storage_path,
    }
}

/// A config whose `browser_type` is rejected before Playwright is ever
/// contacted. Every test that needs `page()` to fail fast uses this so no
/// test can accidentally spawn a browser process.
fn unsupported_resolved(dir: PathBuf) -> ResolvedBrowserConfig {
    let mut cfg = empty_resolved(dir);
    cfg.browser_type = "chromium".to_string();
    cfg
}

#[test]
fn test_resolve_screenshot_path_accepts_simple_name() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    let path = session.resolve_screenshot_path("page.png").unwrap();
    assert_eq!(path, tmp.path().join("page.png"));
}

#[test]
fn test_resolve_screenshot_path_rejects_traversal() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    assert!(session.resolve_screenshot_path("..").is_err());
    assert!(session.resolve_screenshot_path("../etc/passwd").is_err());
    assert!(session.resolve_screenshot_path("a/b.png").is_err());
    assert!(session.resolve_screenshot_path("a\\b.png").is_err());
}

#[test]
fn test_resolve_screenshot_path_rejects_dotfile() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    assert!(session.resolve_screenshot_path(".hidden").is_err());
}

#[test]
fn test_resolve_screenshot_path_rejects_too_long() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    let long = "a".repeat(129) + ".png";
    assert!(session.resolve_screenshot_path(&long).is_err());
}

#[test]
fn test_resolve_screenshot_path_rejects_forbidden_chars() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    for bad in ["foo bar.png", "foo\nbar.png", "foo;rm.png", "foo$bar.png"] {
        assert!(
            session.resolve_screenshot_path(bad).is_err(),
            "should reject {:?}",
            bad
        );
    }
}

#[test]
fn test_session_starts_not_live() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    assert!(!session.is_live());
}

#[test]
fn test_unsupported_browser_type_rejected() {
    let tmp = tempdir();
    let mut cfg = empty_resolved(tmp.path().to_path_buf());
    cfg.browser_type = "chromium".to_string();
    let session = BrowserSession::with_resolved(cfg);
    let err = session.page().unwrap_err();
    assert!(matches!(err, SessionError::UnsupportedBrowserType(_)));
}

#[test]
fn test_tick_does_nothing_when_no_live_session() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    assert!(!session.tick());
}

#[test]
fn test_browser_config_resolve_uses_appdata_on_windows() {
    let cfg = BrowserConfig::default();
    let libs: Vec<ContentLibrary> = vec![];
    let resolved = cfg.resolve(&libs);
    assert!(!resolved.storage_state_path.as_os_str().is_empty());
}

fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

// ---------------------------------------------------------------------------
// SessionError display surface
// ---------------------------------------------------------------------------

#[test]
fn test_session_error_disabled_has_stable_display_text() {
    assert_eq!(
        SessionError::Disabled.to_string(),
        "browser tool group is disabled"
    );
}

#[test]
fn test_session_error_unsupported_browser_type_names_the_rejected_type() {
    let err = SessionError::UnsupportedBrowserType("chromium".into());
    assert_eq!(
        err.to_string(),
        "unsupported browser type 'chromium' (only 'firefox')"
    );
}

#[test]
fn test_session_error_launch_prefixes_the_underlying_reason() {
    let err = SessionError::Launch("firefox binary missing".into());
    assert_eq!(
        err.to_string(),
        "browser launch failed: firefox binary missing"
    );
}

#[test]
fn test_session_error_invalid_filename_prefixes_the_underlying_reason() {
    let err = SessionError::InvalidFilename("filename is empty".into());
    assert_eq!(
        err.to_string(),
        "invalid screenshot filename: filename is empty"
    );
}

#[test]
fn test_session_error_io_prefixes_the_underlying_reason() {
    let err = SessionError::Io("permission denied".into());
    assert_eq!(err.to_string(), "I/O error: permission denied");
}

#[test]
fn test_session_error_other_is_emitted_verbatim() {
    assert_eq!(
        SessionError::Other("nav timeout".into()).to_string(),
        "nav timeout"
    );
}

#[test]
fn test_session_error_usable_as_std_error_trait_object() {
    let err: &dyn std::error::Error = &SessionError::Disabled;
    assert_eq!(err.to_string(), "browser tool group is disabled");
}

// ---------------------------------------------------------------------------
// resolve_screenshot_path — boundary and I/O branches
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_screenshot_path_rejects_empty_filename_with_exact_message() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    let err = session
        .resolve_screenshot_path("")
        .expect_err("the empty filename must be rejected");
    match err {
        SessionError::InvalidFilename(msg) => assert_eq!(
            msg, "filename is empty",
            "the empty-name branch must report its own reason"
        ),
        other => panic!("expected InvalidFilename, got {other}"),
    }
}

#[test]
fn test_resolve_screenshot_path_accepts_exactly_128_characters() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    // BRWS-CONF-002 caps the name at 128 chars; the boundary itself is legal.
    let name = "a".repeat(128);
    let path = session
        .resolve_screenshot_path(&name)
        .expect("a 128-character filename is within the limit");
    assert_eq!(path, tmp.path().join(&name));
}

#[test]
fn test_resolve_screenshot_path_accepts_every_allowed_punctuation_char() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    let path = session
        .resolve_screenshot_path("Shot_01-a.b.png")
        .expect("letters, digits, '.', '_' and '-' are all allowed");
    assert_eq!(path, tmp.path().join("Shot_01-a.b.png"));
}

#[test]
fn test_resolve_screenshot_path_rejects_non_ascii_characters() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    for bad in ["shot-ü.png", "shoté.png", "shot😀.png"] {
        let err = session
            .resolve_screenshot_path(bad)
            .expect_err("only the ASCII alnum/._- class is allowed");
        assert!(
            matches!(err, SessionError::InvalidFilename(_)),
            "should reject {bad:?}, got {err}"
        );
    }
}

#[test]
fn test_resolve_screenshot_path_rejects_dot_dot_substring_even_mid_name() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    // "a..b.png" is a perfectly legal filesystem name, but the '..' policy is
    // a plain substring check and is deliberately not path-aware.
    let err = session
        .resolve_screenshot_path("a..b.png")
        .expect_err("the '..' substring policy must reject this");
    match err {
        SessionError::InvalidFilename(msg) => assert!(
            msg.contains("'..'"),
            "expected the '..' rejection reason, got: {msg}"
        ),
        other => panic!("expected InvalidFilename, got {other}"),
    }
}

#[test]
fn test_resolve_screenshot_path_creates_a_missing_directory_tree() {
    let tmp = tempdir();
    let nested = tmp.path().join("a").join("b").join("c");
    let session = BrowserSession::with_resolved(empty_resolved(nested.clone()));
    let path = session
        .resolve_screenshot_path("deep.png")
        .expect("the screenshot directory is created on demand");
    assert!(nested.is_dir(), "expected {nested:?} to have been created");
    assert_eq!(path, nested.join("deep.png"));
}

#[test]
fn test_resolve_screenshot_path_maps_mkdir_failure_to_io_error() {
    let tmp = tempdir();
    // A regular file cannot act as a parent directory, so `create_dir_all`
    // fails and the error must surface as SessionError::Io, not panic.
    let blocker = tmp.path().join("not-a-dir");
    std::fs::write(&blocker, b"x").expect("test fixture must be writable");
    let session = BrowserSession::with_resolved(empty_resolved(blocker.join("shots")));
    let err = session
        .resolve_screenshot_path("shot.png")
        .expect_err("the screenshot directory cannot be created underneath a file");
    assert!(
        matches!(err, SessionError::Io(_)),
        "expected SessionError::Io, got {err}"
    );
}

// ---------------------------------------------------------------------------
// Construction and config snapshot
// ---------------------------------------------------------------------------

#[test]
fn test_config_snapshot_is_a_clone_and_not_a_shared_view() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    let mut first = session.config();
    assert_eq!(first.browser_type, "firefox");
    assert!(first.headless, "the fixture config is headless");
    assert_eq!(first.idle_timeout_seconds, 60);
    assert_eq!(first.page_load_timeout_ms, 30_000);
    assert_eq!(first.screenshot_dir, tmp.path());
    assert_eq!(first.storage_state_path, tmp.path().join("storage.json"));

    first.browser_type = "webkit".to_string();
    assert_eq!(
        session.config().browser_type,
        "firefox",
        "mutating a snapshot must not leak back into the session"
    );
}

#[test]
fn test_new_from_app_config_uses_explicit_browser_paths() {
    let tmp = tempdir();
    let shots = tmp.path().join("shots");
    let storage = tmp.path().join("state").join("storage.json");

    let mut app = AppConfig::default();
    app.browser.screenshot_dir = shots.display().to_string();
    app.browser.storage_state_path = storage.display().to_string();
    app.browser.headless = false;
    app.browser.idle_timeout_seconds = 12;
    app.browser.page_load_timeout_ms = 4_321;

    let session = BrowserSession::new(&app);
    let resolved = session.config();
    assert_eq!(resolved.screenshot_dir, shots);
    assert_eq!(resolved.storage_state_path, storage);
    assert!(
        !resolved.headless,
        "the explicit non-headless setting is kept"
    );
    assert_eq!(resolved.idle_timeout_seconds, 12);
    assert_eq!(resolved.page_load_timeout_ms, 4_321);
    assert_eq!(resolved.browser_type, "firefox");
    assert!(
        !session.is_live(),
        "constructing a session must not spawn a browser"
    );
}

#[test]
fn test_new_from_app_config_falls_back_to_non_empty_default_paths() {
    let app = AppConfig::default();
    let session = BrowserSession::new(&app);
    let resolved = session.config();
    assert!(
        !resolved.screenshot_dir.as_os_str().is_empty(),
        "an empty screenshot_dir must be filled in by resolve()"
    );
    assert!(
        !resolved.storage_state_path.as_os_str().is_empty(),
        "an empty storage_state_path must be filled in by resolve()"
    );
}

#[test]
fn test_new_from_app_config_derives_screenshot_dir_from_first_library() {
    let tmp = tempdir();
    let mut app = AppConfig::default();
    app.content_libraries = vec![ContentLibrary {
        root_folder: tmp.path().display().to_string(),
        name: "wiki".to_string(),
        kind: "markdown".to_string(),
        readonly: true,
        priority: 0,
    }];
    let session = BrowserSession::new(&app);
    assert_eq!(
        session.config().screenshot_dir,
        tmp.path().join("browser-screenshots"),
        "the default screenshot dir hangs off the first content library"
    );
}

// ---------------------------------------------------------------------------
// Storage persistence without a live session
// ---------------------------------------------------------------------------

#[test]
fn test_save_storage_without_live_session_writes_nothing() {
    let tmp = tempdir();
    let storage = tmp.path().join("storage.json");
    let mut cfg = empty_resolved(tmp.path().to_path_buf());
    cfg.storage_state_path = storage.clone();
    let session = BrowserSession::with_resolved(cfg);

    BrowserSession::save_storage(&session).expect("no live session means nothing to persist");
    assert!(
        !storage.exists(),
        "no storage file may be written when no page is open"
    );
}

#[test]
fn test_forget_without_storage_file_is_not_an_error() {
    let tmp = tempdir();
    let storage = tmp.path().join("storage.json");
    let mut cfg = empty_resolved(tmp.path().to_path_buf());
    cfg.storage_state_path = storage.clone();
    let session = BrowserSession::with_resolved(cfg);

    session
        .forget()
        .expect("a missing storage file is the NotFound branch and must be tolerated");
    assert!(!storage.exists());
}

#[test]
fn test_forget_deletes_an_existing_storage_file() {
    let tmp = tempdir();
    let storage = tmp.path().join("storage.json");
    std::fs::write(&storage, b"{\"cookies\":[]}").expect("test fixture must be writable");
    let mut cfg = empty_resolved(tmp.path().to_path_buf());
    cfg.storage_state_path = storage.clone();
    let session = BrowserSession::with_resolved(cfg);

    session
        .forget()
        .expect("forget must remove the persisted storage file");
    assert!(
        !storage.exists(),
        "the persisted storage file must be gone after forget()"
    );
}

#[test]
fn test_forget_reports_io_error_when_storage_path_is_not_a_file() {
    let tmp = tempdir();
    let dir = tmp.path().join("storage-dir");
    std::fs::create_dir(&dir).expect("test fixture must be creatable");
    let mut cfg = empty_resolved(tmp.path().to_path_buf());
    cfg.storage_state_path = dir.clone();
    let session = BrowserSession::with_resolved(cfg);

    let err = session
        .forget()
        .expect_err("removing a directory through remove_file is an I/O failure");
    assert!(
        matches!(err, SessionError::Io(_)),
        "expected SessionError::Io, got {err}"
    );
    assert!(dir.exists(), "the directory must still be present");
}

// ---------------------------------------------------------------------------
// Idle-timeout clock
// ---------------------------------------------------------------------------

#[test]
fn test_tick_short_circuits_when_idle_timeout_is_zero() {
    let tmp = tempdir();
    let mut cfg = unsupported_resolved(tmp.path().to_path_buf());
    cfg.idle_timeout_seconds = 0;
    let session = BrowserSession::with_resolved(cfg);
    // A failed launch still stamps `last_used`; the zero timeout must return
    // before that clock is ever inspected.
    assert!(
        session.page().is_err(),
        "an unsupported type must not launch"
    );
    assert!(
        !session.tick(),
        "idle_timeout_seconds == 0 disables the idle timeout entirely"
    );
}

#[test]
fn test_tick_keeps_session_while_idle_clock_is_fresh() {
    let tmp = tempdir();
    let mut cfg = unsupported_resolved(tmp.path().to_path_buf());
    cfg.idle_timeout_seconds = 3_600;
    let session = BrowserSession::with_resolved(cfg);

    let err = session.page().unwrap_err();
    assert!(
        matches!(err, SessionError::UnsupportedBrowserType(_)),
        "expected the unsupported-type error, got {err}"
    );
    assert!(
        !session.is_live(),
        "a failed launch must not install a live session"
    );
    assert!(
        !session.tick(),
        "a freshly stamped idle clock must not expire the session"
    );
}

#[test]
fn test_failed_launch_leaves_session_not_live_and_retriable() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(unsupported_resolved(tmp.path().to_path_buf()));
    for attempt in 0..3 {
        let launched = session.page();
        assert!(
            launched.is_err(),
            "a repeated launch attempt must keep failing"
        );
        let err = launched.unwrap_err();
        assert!(
            matches!(err, SessionError::UnsupportedBrowserType(_)),
            "attempt {attempt} returned {err}"
        );
        assert!(!session.is_live(), "attempt {attempt} must not go live");
    }
}

// ---------------------------------------------------------------------------
// BrowserAutomationExt surface — every arm reachable without a browser
// ---------------------------------------------------------------------------

#[test]
fn test_ext_navigate_reports_unsupported_browser_without_launching() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(unsupported_resolved(tmp.path().to_path_buf()));
    let err = match session.navigate("https://example.invalid") {
        Ok(_) => panic!("navigation cannot succeed without a supported browser"),
        Err(e) => e,
    };
    assert!(
        err.contains("unsupported browser type 'chromium'"),
        "unexpected error text: {err}"
    );
    assert!(!session.is_live(), "no browser may have been launched");
}

#[test]
fn test_ext_page_mutations_all_fail_before_playwright_is_contacted() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(unsupported_resolved(tmp.path().to_path_buf()));
    let checks: Vec<(&str, Result<(), String>)> = vec![
        ("click", session.click("#btn")),
        ("fill_input", session.fill_input("#in", "hello")),
        ("select_dropdown", session.select_dropdown("#sel", "opt")),
        ("press_key", session.press_key("Enter")),
    ];
    for (name, result) in checks {
        match result {
            Ok(()) => panic!("{name} must fail without a live page"),
            Err(e) => assert!(
                e.contains("unsupported browser type 'chromium'"),
                "{name} returned the wrong error: {e}"
            ),
        }
    }
    assert!(!session.is_live(), "no browser may have been launched");
}

#[test]
fn test_ext_get_page_state_reports_unsupported_browser() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(unsupported_resolved(tmp.path().to_path_buf()));
    let err = match session.get_page_state() {
        Ok(v) => panic!("page state cannot be read without a browser, got {v:?}"),
        Err(e) => e,
    };
    assert!(
        err.contains("unsupported browser type 'chromium'"),
        "unexpected error text: {err}"
    );
}

#[test]
fn test_ext_evaluate_js_reports_unsupported_browser() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(unsupported_resolved(tmp.path().to_path_buf()));
    let err = match session.evaluate_js("1 + 1") {
        Ok(v) => panic!("JS cannot be evaluated without a browser, got {v}"),
        Err(e) => e,
    };
    assert!(
        err.contains("unsupported browser type 'chromium'"),
        "unexpected error text: {err}"
    );
}

#[test]
fn test_ext_screenshot_validates_the_filename_before_touching_the_page() {
    let tmp = tempdir();
    // The session config here is a *supported* type, so the only way this can
    // fail is the filename policy — proving validation runs before launch().
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    let err = match session.screenshot("../escape.png", false) {
        Ok(v) => panic!("an escaping filename must be rejected, got {v:?}"),
        Err(e) => e,
    };
    assert!(
        err.contains("invalid screenshot filename"),
        "unexpected error text: {err}"
    );
    assert!(
        !session.is_live(),
        "filename rejection must happen before any browser launch"
    );
}

#[test]
fn test_ext_screenshot_fails_after_validation_when_browser_is_unsupported() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(unsupported_resolved(tmp.path().to_path_buf()));
    let err = match session.screenshot("ok.png", true) {
        Ok(v) => panic!(
            "a screenshot cannot be taken without a browser, got {:?}",
            v.0
        ),
        Err(e) => e,
    };
    assert!(
        err.contains("unsupported browser type 'chromium'"),
        "unexpected error text: {err}"
    );
}

#[test]
fn test_ext_save_storage_succeeds_when_no_session_is_live() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    BrowserAutomationExt::save_storage(&session)
        .expect("saving storage with no live session is a successful no-op");
}

#[test]
fn test_ext_resolve_screenshot_path_maps_session_errors_to_strings() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    let ok = BrowserAutomationExt::resolve_screenshot_path(&session, "a.png")
        .expect("a plain filename resolves against the screenshot dir");
    assert_eq!(ok, tmp.path().join("a.png"));
    assert!(
        ok.starts_with(tmp.path()),
        "the resolved path must stay inside the configured screenshot dir"
    );
}

#[test]
fn test_ext_resolve_screenshot_path_rejects_empty_name_as_string_error() {
    let tmp = tempdir();
    let session = BrowserSession::with_resolved(empty_resolved(tmp.path().to_path_buf()));
    let result = BrowserAutomationExt::resolve_screenshot_path(&session, "");
    assert!(
        result.is_err(),
        "the empty filename must be rejected through the trait surface too"
    );
    match result {
        Err(e) => assert!(
            e.contains("invalid screenshot filename"),
            "unexpected error text: {e}"
        ),
        Ok(p) => panic!("expected an error, got {p:?}"),
    }
}
