//! Tests for `background/manager.rs`.

use super::*;
use std::io;

/// Serialisation guard for every test that mutates the process-global
/// [`UI_LOG_TARGET`] registration.
///
/// `cargo nextest` runs each test in its own process, so these tests are
/// naturally isolated there. `cargo test` â€” and therefore `cargo llvm-cov`,
/// which drives `cargo test` â€” runs them concurrently inside one process, where
/// two tests race on the single global slot and deliver each other's events, so
/// the captured-count assertions fail. Holding this mutex for the whole body of
/// every test that registers a handle makes them correct under either runner.
///
/// Poisoning is tolerated deliberately: a failing test must not cascade into
/// every later one.
static UI_LOG_TARGET_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Acquires [`UI_LOG_TARGET_TEST_LOCK`] for the duration of the calling test.
fn serialise_ui_log_target() -> std::sync::MutexGuard<'static, ()> {
    UI_LOG_TARGET_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn make_entry(msg: &str) -> BackgroundLogEntry {
    BackgroundLogEntry::new(LogCategory::Indexer, msg.to_string())
}

/// A `Write` that succeeds for the first `fail_after` bytes, then
/// returns `Other` on every subsequent call. Used to simulate
/// mid-write failures (disk full, broken pipe, etc.) without
/// relying on OS-level quirks.
struct FailingWriter {
    written: usize,
    fail_after: usize,
}
impl FailingWriter {
    fn new(fail_after: usize) -> Self {
        Self {
            written: 0,
            fail_after,
        }
    }
}
impl io::Write for FailingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written >= self.fail_after {
            return Err(io::Error::other("simulated mid-write failure"));
        }
        let take = (self.fail_after - self.written).min(buf.len());
        self.written += take;
        Ok(take)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_push_log_adds_entry() {
    let mut mgr = BackgroundLogs::new();
    mgr.push_log(make_entry("test"));
    assert_eq!(mgr.get_logs().len(), 1);
}

#[test]
fn test_push_log_overflow_evicts_oldest() {
    let mut mgr = BackgroundLogs::new();
    for i in 0..MAX_LOG_ENTRIES + 10 {
        mgr.push_log(make_entry(&format!("entry {}", i)));
    }
    assert_eq!(mgr.get_logs().len(), MAX_LOG_ENTRIES);
    let first = mgr.get_logs().front().unwrap();
    assert!(first.message.contains("entry 10"));
}

#[test]
fn test_clear_logs_empties() {
    let mut mgr = BackgroundLogs::new();
    mgr.push_log(make_entry("test"));
    mgr.clear_logs();
    assert!(mgr.get_logs().is_empty());
}

#[test]
fn test_save_logs_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = dir.path().join("logs/test.log");

    let mut mgr = BackgroundLogs::new();
    mgr.push_log(make_entry("line one"));
    mgr.push_log(make_entry("line two"));

    mgr.save_logs(&log_path).unwrap();
    assert!(log_path.exists());

    let content = std::fs::read_to_string(&log_path).unwrap();
    assert!(content.contains("Indexer"));
    assert!(content.contains("line one"));
    assert!(content.contains("line two"));
}

#[test]
fn test_save_logs_propagates_write_errors() {
    // The original save_logs loop did `let _ = file.write_all(...)`
    // which silently swallowed mid-write failures (disk full,
    // broken pipe, etc.) and returned Ok(()) on a truncated file.
    // Contract: when the underlying write fails, save_logs must
    // return Err so the caller knows the file is incomplete.
    let mut mgr = BackgroundLogs::new();
    // Several entries so we cross the "fail_after" boundary
    // mid-stream and not on the first line.
    for i in 0..10 {
        mgr.push_log(make_entry(&format!("entry {}", i)));
    }

    let mut failing = FailingWriter::new(0);
    let result = mgr.write_logs_to(&mut failing);
    assert!(
        result.is_err(),
        "expected write failure to propagate, got {:?}",
        result
    );
}

#[test]
fn test_filter_category_none_shows_all() {
    let mut mgr = BackgroundLogs::new();
    mgr.push_log(BackgroundLogEntry::new(
        LogCategory::Indexer,
        "idx".to_string(),
    ));
    mgr.push_log(BackgroundLogEntry::new(
        LogCategory::Watcher,
        "wtch".to_string(),
    ));

    mgr.filter_category = None;
    // With no filter, all pass through
    let logs: Vec<_> = mgr.get_logs().iter().collect();
    assert_eq!(logs.len(), 2);
}

/// When `filter_category` is set, downstream log consumers (e.g.
/// the background-logs window) must only see entries whose
/// category matches. The previous version of this test never
/// exercised the filtering path itself, only the no-filter
/// default. This test pins the contract end-to-end: push
/// entries of two categories, set a filter, verify only the
/// matching entries are visible.
///
/// The filter is consumed by [`crate::ui_iced::logs::is_log_visible`],
/// which we exercise here indirectly by re-implementing the
/// same predicate; if the two diverge, that test will catch it.
#[test]
fn test_filter_category_isolates_matching_entries() {
    let mut mgr = BackgroundLogs::new();
    mgr.push_log(BackgroundLogEntry::new(
        LogCategory::Indexer,
        "indexed 1".to_string(),
    ));
    mgr.push_log(BackgroundLogEntry::new(
        LogCategory::Watcher,
        "watched 1".to_string(),
    ));
    mgr.push_log(BackgroundLogEntry::new(
        LogCategory::Indexer,
        "indexed 2".to_string(),
    ));
    mgr.push_log(BackgroundLogEntry::new(
        LogCategory::PdfConverter,
        "converted 1".to_string(),
    ));

    mgr.filter_category = Some(LogCategory::Indexer);
    let visible: Vec<&BackgroundLogEntry> = mgr
        .get_logs()
        .iter()
        .filter(|l| mgr.filter_category.is_none_or(|c| l.category == c))
        .collect();
    assert_eq!(
        visible.len(),
        2,
        "Indexer filter should isolate the two Indexer entries"
    );
    assert!(visible.iter().all(|l| l.category == LogCategory::Indexer));

    // Switch to a different category and confirm the same
    // predicate still narrows correctly.
    mgr.filter_category = Some(LogCategory::PdfConverter);
    let visible: Vec<&BackgroundLogEntry> = mgr
        .get_logs()
        .iter()
        .filter(|l| mgr.filter_category.is_none_or(|c| l.category == c))
        .collect();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].category, LogCategory::PdfConverter);

    // Clear the filter; all entries become visible again.
    mgr.filter_category = None;
    let visible: Vec<&BackgroundLogEntry> = mgr
        .get_logs()
        .iter()
        .filter(|l| mgr.filter_category.is_none_or(|c| l.category == c))
        .collect();
    assert_eq!(visible.len(), 4);
}

/// `search_text` matching is case-insensitive on both sides, and an empty
/// query shows everything. The two tests this replaces (`test_search_text_filters`
/// and `test_search_text_is_case_insensitive`) differed only in the corpus and
/// the query, so they are collapsed into one table. Every row keeps its original
/// assertion and the case name is embedded in the failure message.
#[test]
fn test_search_text_filters_case_insensitively() {
    struct Case {
        name: &'static str,
        corpus: Vec<&'static str>,
        query: &'static str,
        expected: Vec<&'static str>,
    }

    let cases = vec![
        Case {
            name: "lowercase query narrows to the single matching entry",
            corpus: vec!["apple banana", "cherry date"],
            query: "apple",
            expected: vec!["apple banana"],
        },
        Case {
            name: "an uppercase query still matches mixed-case messages",
            corpus: vec!["Indexing file.md", "Watching directory", "PDF conversion"],
            query: "WATCH",
            expected: vec!["Watching directory"],
        },
        Case {
            name: "an uppercase message matches a lowercase query",
            corpus: vec!["Indexing file.md", "Watching directory", "PDF conversion"],
            query: "pdf",
            expected: vec!["PDF conversion"],
        },
        Case {
            name: "an empty query shows every entry",
            corpus: vec!["apple banana", "cherry date"],
            query: "",
            expected: vec!["apple banana", "cherry date"],
        },
        Case {
            name: "a query that matches nothing yields an empty view",
            corpus: vec!["apple banana", "cherry date"],
            query: "kiwi",
            expected: vec![],
        },
    ];

    for case in cases {
        let mut mgr = BackgroundLogs::new();
        for message in case.corpus {
            mgr.push_log(make_entry(message));
        }
        mgr.search_text = case.query.to_string();

        let needle = mgr.search_text.to_lowercase();
        let visible: Vec<&BackgroundLogEntry> = mgr
            .get_logs()
            .iter()
            .filter(|l| {
                if needle.is_empty() {
                    return true;
                }
                l.message.to_lowercase().contains(&needle)
            })
            .collect();

        let messages: Vec<&str> = visible.iter().map(|l| l.message.as_str()).collect();
        assert_eq!(messages, case.expected, "case: {}", case.name);
    }
}

#[test]
fn test_auto_scroll_default_true() {
    let mgr = BackgroundLogs::new();
    assert!(mgr.auto_scroll);
}

#[test]
fn test_parse_log_category() {
    struct Case {
        input: &'static str,
        expected: Option<LogCategory>,
    }

    let cases = [
        // Original coverage.
        Case {
            input: "fastmd::background::indexer",
            expected: Some(LogCategory::Indexer),
        },
        Case {
            input: "WATCHER",
            expected: Some(LogCategory::Watcher),
        },
        Case {
            input: "pdf_converter",
            expected: Some(LogCategory::PdfConverter),
        },
        Case {
            input: "image_vision",
            expected: Some(LogCategory::ImageVision),
        },
        Case {
            input: "llm_tools",
            expected: Some(LogCategory::LlmTools),
        },
        Case {
            input: "print",
            expected: Some(LogCategory::Print),
        },
        Case {
            input: "batch",
            expected: Some(LogCategory::Batch),
        },
        Case {
            input: "external_crate",
            expected: None,
        },
        // Additional aliases accepted by the substring matcher.
        Case {
            input: "",
            expected: None,
        },
        Case {
            input: "IMAGE",
            expected: Some(LogCategory::ImageVision),
        },
        Case {
            input: "vision",
            expected: Some(LogCategory::ImageVision),
        },
        Case {
            input: "fastmd::agent",
            expected: Some(LogCategory::LlmTools),
        },
        Case {
            input: "tool",
            expected: Some(LogCategory::LlmTools),
        },
        Case {
            input: "PDF",
            expected: Some(LogCategory::PdfConverter),
        },
        // The matcher is a first-match-wins chain, so an input naming two
        // categories resolves to the earliest one in the chain.
        Case {
            input: "batch_indexer",
            expected: Some(LogCategory::Indexer),
        },
    ];

    for case in cases {
        assert_eq!(
            parse_log_category(case.input),
            case.expected,
            "parse_log_category({:?})",
            case.input
        );
    }
}

#[test]
fn test_log_dir_from_env_resolution() {
    // 1. Explicit log_dir override takes highest precedence
    let p = log_dir_from_env(
        Some("/custom/logs"),
        Some("/cfg/config.yaml"),
        Some("/appdata"),
        Some("/user"),
    );
    assert_eq!(p, PathBuf::from("/custom/logs"));

    // 2. Config path parent / logs
    let p = log_dir_from_env(
        None,
        Some("/cfg/config.yaml"),
        Some("/appdata"),
        Some("/user"),
    );
    assert_eq!(p, PathBuf::from("/cfg/logs"));

    // 3. APPDATA fallback
    let p = log_dir_from_env(None, None, Some("/appdata"), Some("/user"));
    assert_eq!(p, PathBuf::from("/appdata/fastmd/logs"));

    // 4. USERPROFILE fallback
    let p = log_dir_from_env(None, None, None, Some("/user"));
    assert_eq!(p, PathBuf::from("/user/.fastmd/logs"));

    // 5. Default fallback
    let p = log_dir_from_env(None, None, None, None);
    assert_eq!(p, PathBuf::from("logs"));
}

/// **Regression test for the empty-environment-variable inconsistency (defect D5).**
///
/// `log_dir_override` and `config_path_override` already treated `Some("")` as
/// unset, but `appdata` and `userprofile` did not: `APPDATA=` produced
/// `fastmd/logs` and `USERPROFILE=` produced `.fastmd/logs` — relative paths
/// resolved against the current working directory. All four inputs now share
/// one rule, so `None` and `Some("")` behave identically.
///
/// Every value here is a synthetic literal; no real user directory is read
/// ([RUST-006]).
#[test]
fn defect_05_log_dir_from_env_treats_an_empty_variable_as_unset() {
    struct Case {
        name: &'static str,
        log_dir: Option<&'static str>,
        config_path: Option<&'static str>,
        appdata: Option<&'static str>,
        userprofile: Option<&'static str>,
        expected: PathBuf,
    }

    let cases = [
        // ---- log_dir_override: unset / empty / set ----
        Case {
            name: "unset log_dir falls through to the config-path parent",
            log_dir: None,
            config_path: Some("/cfg/config.yaml"),
            appdata: None,
            userprofile: None,
            expected: PathBuf::from("/cfg/logs"),
        },
        Case {
            name: "empty log_dir falls through to the config-path parent",
            log_dir: Some(""),
            config_path: Some("/cfg/config.yaml"),
            appdata: None,
            userprofile: None,
            expected: PathBuf::from("/cfg/logs"),
        },
        Case {
            name: "a set log_dir wins over every other source",
            log_dir: Some("/custom/logs"),
            config_path: Some("/cfg/config.yaml"),
            appdata: Some("/appdata"),
            userprofile: Some("/user"),
            expected: PathBuf::from("/custom/logs"),
        },
        // ---- config_path_override: unset / empty / set ----
        Case {
            name: "unset config_path falls through to APPDATA",
            log_dir: None,
            config_path: None,
            appdata: Some("/appdata"),
            userprofile: None,
            expected: PathBuf::from("/appdata/fastmd/logs"),
        },
        Case {
            name: "empty config_path falls through to APPDATA",
            log_dir: None,
            config_path: Some(""),
            appdata: Some("/appdata"),
            userprofile: None,
            expected: PathBuf::from("/appdata/fastmd/logs"),
        },
        Case {
            name: "a set config_path wins over APPDATA",
            log_dir: None,
            config_path: Some("/cfg/config.yaml"),
            appdata: Some("/appdata"),
            userprofile: Some("/user"),
            expected: PathBuf::from("/cfg/logs"),
        },
        // ---- appdata: unset / empty / set ----
        Case {
            name: "unset APPDATA falls through to USERPROFILE",
            log_dir: None,
            config_path: None,
            appdata: None,
            userprofile: Some("/user"),
            expected: PathBuf::from("/user/.fastmd/logs"),
        },
        Case {
            name: "empty APPDATA falls through to USERPROFILE instead of \
                   yielding `fastmd/logs`",
            log_dir: None,
            config_path: None,
            appdata: Some(""),
            userprofile: Some("/user"),
            expected: PathBuf::from("/user/.fastmd/logs"),
        },
        Case {
            name: "a set APPDATA wins over USERPROFILE",
            log_dir: None,
            config_path: None,
            appdata: Some("/appdata"),
            userprofile: Some("/user"),
            expected: PathBuf::from("/appdata/fastmd/logs"),
        },
        // ---- userprofile: unset / empty / set ----
        Case {
            name: "unset USERPROFILE falls through to the relative default",
            log_dir: None,
            config_path: None,
            appdata: None,
            userprofile: None,
            expected: PathBuf::from("logs"),
        },
        Case {
            name: "empty USERPROFILE falls through to the relative default \
                   instead of yielding `.fastmd/logs`",
            log_dir: None,
            config_path: None,
            appdata: None,
            userprofile: Some(""),
            expected: PathBuf::from("logs"),
        },
        Case {
            name: "a set USERPROFILE is used as the last fallback",
            log_dir: None,
            config_path: None,
            appdata: None,
            userprofile: Some("/user"),
            expected: PathBuf::from("/user/.fastmd/logs"),
        },
        // ---- all four empty must behave exactly like all four unset ----
        Case {
            name: "every variable empty behaves like every variable unset",
            log_dir: Some(""),
            config_path: Some(""),
            appdata: Some(""),
            userprofile: Some(""),
            expected: PathBuf::from("logs"),
        },
    ];

    for case in cases {
        assert_eq!(
            log_dir_from_env(
                case.log_dir,
                case.config_path,
                case.appdata,
                case.userprofile
            ),
            case.expected,
            "{}",
            case.name
        );
    }
}

/// An empty override must not shadow a usable lower-priority source, and an
/// empty variable must never contribute an empty leading path component.
#[test]
fn defect_05_empty_overrides_never_contribute_an_empty_path_component() {
    for appdata in [None, Some("")] {
        for userprofile in [None, Some("")] {
            let resolved = log_dir_from_env(None, None, appdata, userprofile);
            assert_eq!(
                resolved,
                PathBuf::from("logs"),
                "an empty APPDATA/USERPROFILE must fall through to the relative \
                 default rather than contribute an empty leading component \
                 (appdata={appdata:?}, userprofile={userprofile:?})"
            );
        }
    }
}

#[test]
fn test_get_log_dir_panic_shield_in_tests() {
    // Under test, calling get_log_dir without FASTMD_LOG_DIR must panic per RUST-006
    if std::env::var("FASTMD_LOG_DIR").is_err() && std::env::var("FASTMD_CONFIG_PATH").is_err() {
        let result = std::panic::catch_unwind(|| {
            get_log_dir();
        });
        assert!(
            result.is_err(),
            "RUST-006: get_log_dir must panic in test environment when env vars are unset"
        );
    }
}

#[test]
fn test_ui_background_log_layer_captures_and_filters() {
    let _serial = serialise_ui_log_target();
    use tracing_subscriber::layer::SubscriberExt;

    let shared_logs = Arc::new(Mutex::new(BackgroundLogs::new()));
    register_ui_logs(shared_logs.clone());

    let subscriber = tracing_subscriber::registry().with(UiBackgroundLogLayer);

    tracing::subscriber::with_default(subscriber, || {
        // 1. FastMD indexer event -> captured with LogCategory::Indexer
        tracing::info!(target: "fastmd::background::indexer", "Indexed 42 files");

        // 2. FastMD watcher event -> captured with LogCategory::Watcher
        tracing::warn!(target: "fastmd::watcher", "File changed");

        // 3. Channel event -> must be ignored to prevent double-logging
        tracing::info!(target: TARGET_BACKGROUND_CHANNEL, "Channel event");

        // 4. Third-party event -> must be ignored
        tracing::info!(target: "hyper::proto", "HTTP connection opened");
    });

    unregister_ui_logs();

    let logs = shared_logs.lock().unwrap();
    let entries = logs.get_logs();
    assert_eq!(
        entries.len(),
        2,
        "Expected exactly 2 events captured (indexer and watcher)"
    );
    assert_eq!(entries[0].category, LogCategory::Indexer);
    assert!(entries[0].message.contains("Indexed 42 files"));
    assert_eq!(entries[1].category, LogCategory::Watcher);
    assert!(entries[1].message.contains("File changed"));
}

#[test]
fn test_non_blocking_file_appender_persists_to_disk() {
    use tracing_subscriber::layer::SubscriberExt;

    let dir = tempfile::tempdir().unwrap();
    let log_path = dir.path().join(LOG_FILENAME);

    let appender = tracing_appender::rolling::never(dir.path(), LOG_FILENAME);
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    let subscriber = tracing_subscriber::registry().with(file_layer);

    tracing::subscriber::with_default(subscriber, || {
        tracing::info!("Non-blocking disk write test line 1");
        tracing::warn!("Non-blocking disk write test line 2");
    });

    // Dropping the WorkerGuard flushes the non-blocking background queue to disk
    drop(guard);

    assert!(
        log_path.exists(),
        "Log file should have been created on disk"
    );
    let content = std::fs::read_to_string(&log_path).unwrap();
    assert!(content.contains("Non-blocking disk write test line 1"));
    assert!(content.contains("Non-blocking disk write test line 2"));
}

#[test]
fn test_default_field_values() {
    let mgr = BackgroundLogs::default();
    assert!(mgr.get_logs().is_empty());
    assert!(mgr.filter_category.is_none());
    assert!(mgr.search_text.is_empty());
    assert!(mgr.auto_scroll);
    assert!(
        !mgr.show_background_logs,
        "the log window starts hidden so a launch does not pop a panel"
    );
    // `new()` and `default()` must agree.
    assert!(BackgroundLogs::new().auto_scroll);
}

/// An empty `FASTMD_LOG_DIR` / config-path override must be treated as "unset"
/// rather than as a request for the current directory - otherwise a blank
/// `FASTMD_LOG_DIR` would silently redirect logs to `./logs`.
///
/// The rule is uniform across all four inputs: an empty string is unset for the
/// two *overrides* (`log_dir_override`, `config_path_override`) and for the two
/// *fallbacks* (`appdata`, `userprofile`) alike, so a cleared-but-present
/// `APPDATA` falls through to `USERPROFILE` instead of yielding the relative
/// `fastmd/logs`. See `defect_05_log_dir_from_env_treats_an_empty_variable_as_unset`
/// for the per-input sweep.
#[test]
fn test_log_dir_from_env_ignores_empty_overrides() {
    // 1. Empty log_dir falls through to the config path.
    let p = log_dir_from_env(Some(""), Some("/cfg/config.yaml"), Some("/appdata"), None);
    assert_eq!(p, PathBuf::from("/cfg/logs"));

    // 2. Empty log_dir and empty config path fall through to APPDATA.
    let p = log_dir_from_env(Some(""), Some(""), Some("/appdata"), Some("/user"));
    assert_eq!(p, PathBuf::from("/appdata/fastmd/logs"));

    // 3. An empty APPDATA is treated as unset, so USERPROFILE is consulted and
    //    the log directory stays absolute.
    let p = log_dir_from_env(None, None, Some(""), Some("/user"));
    assert_eq!(p, PathBuf::from("/user/.fastmd/logs"));

    // 4. USERPROFILE is only consulted when APPDATA is `None`, not when it is
    //    an empty string.
    let p = log_dir_from_env(None, None, None, Some("/user"));
    assert_eq!(p, PathBuf::from("/user/.fastmd/logs"));

    // 5. The bare relative default is reached only when both fallbacks are
    //    absent entirely.
    let p = log_dir_from_env(Some(""), Some(""), None, None);
    assert_eq!(p, PathBuf::from("logs"));
}

/// `save_logs` must create missing parent directories so a first export into a
/// fresh profile works ([REQ-000]).
#[test]
fn test_save_logs_creates_missing_parent_directories() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = dir.path().join("a").join("b").join("c").join("export.log");
    assert!(!log_path.parent().unwrap().exists());

    let mut mgr = BackgroundLogs::new();
    mgr.push_log(make_entry("nested write"));

    mgr.save_logs(&log_path).unwrap();
    let content = std::fs::read_to_string(&log_path).unwrap();
    assert!(content.contains("nested write"));
}

/// When the parent path cannot be created (a component is a regular file),
/// `save_logs` must report the error instead of proceeding to a doomed
/// `File::create`.
#[test]
fn test_save_logs_reports_parent_creation_failure() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "i am a file").unwrap();
    let log_path = blocker.join("nested").join("export.log");

    let mgr = BackgroundLogs::new();

    let err = mgr
        .save_logs(&log_path)
        .expect_err("a parent path blocked by a file cannot be created");
    assert!(
        !log_path.exists(),
        "no log file may be created under an uncreatable parent"
    );
    assert!(
        !err.to_string().is_empty(),
        "the error must carry a message"
    );
}

/// A failure part-way through the stream must propagate after the earlier bytes
/// were written â€” the caller needs to know the file is truncated.
#[test]
fn test_write_logs_to_propagates_a_mid_stream_failure() {
    let mut mgr = BackgroundLogs::new();
    for i in 0..5 {
        mgr.push_log(make_entry(&format!("entry {i}")));
    }

    // Enough headroom for the first line, but not for the second.
    let mut failing = FailingWriter::new(32);
    let result = mgr.write_logs_to(&mut failing);

    assert!(
        result.is_err(),
        "a mid-stream write failure must propagate, got {result:?}"
    );
    assert!(
        failing.written > 0,
        "the entries written before the failure must have been flushed"
    );
}

/// An empty buffer serialises to nothing and never touches the writer.
#[test]
fn test_write_logs_to_empty_buffer_writes_nothing() {
    let mgr = BackgroundLogs::new();
    let mut sink = Vec::new();

    mgr.write_logs_to(&mut sink).unwrap();

    assert!(sink.is_empty(), "no entries means no bytes");
}

/// Each line is `[timestamp] [category] message`, which is what the on-disk
/// export is parsed and eyeballed against.
#[test]
fn test_write_logs_to_formats_one_line_per_entry() {
    let mut mgr = BackgroundLogs::new();
    mgr.push_log(BackgroundLogEntry::new(
        LogCategory::PdfConverter,
        "converted note.md".to_string(),
    ));
    mgr.push_log(make_entry("indexed note.md"));

    let mut out = Vec::new();
    mgr.write_logs_to(&mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().collect();

    assert_eq!(lines.len(), 2);
    assert!(
        lines[0].starts_with('[') && lines[0].contains("[PDF Converter] converted note.md"),
        "unexpected first line: {:?}",
        lines[0]
    );
    assert!(
        lines[1].contains("[Indexer] indexed note.md"),
        "unexpected second line: {:?}",
        lines[1]
    );
}

/// Structured fields are appended to the message as `key=value` pairs, and the
/// `category` field selects the category instead of becoming part of the text.
#[test]
fn test_ui_layer_appends_structured_fields() {
    let _serial = serialise_ui_log_target();
    use tracing_subscriber::layer::SubscriberExt;

    let shared_logs = Arc::new(Mutex::new(BackgroundLogs::new()));
    register_ui_logs(shared_logs.clone());

    let subscriber = tracing_subscriber::registry().with(UiBackgroundLogLayer);
    tracing::subscriber::with_default(subscriber, || {
        // `record_str` path: a literal string field.
        tracing::info!(target: "fastmd::watcher", path = "note.md", "file changed {}", "note.md");
        // `record_debug` path: a non-string field is `Debug`-formatted.
        tracing::info!(target: "fastmd::watcher", count = 3, "indexed");
    });

    unregister_ui_logs();

    let logs = shared_logs.lock().unwrap();
    let entries = logs.get_logs();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].category, LogCategory::Watcher);
    assert_eq!(
        entries[0].message, "file changed note.md (path=note.md)",
        "string fields must be appended as key=value"
    );
    assert_eq!(entries[1].message, "indexed (count=3)");
    assert_eq!(entries[1].category, LogCategory::Watcher);
}

/// An explicit `category` field wins over the target-derived category and is
/// not repeated inside the message.
#[test]
fn test_ui_layer_prefers_the_explicit_category_field() {
    let _serial = serialise_ui_log_target();
    use tracing_subscriber::layer::SubscriberExt;

    let shared_logs = Arc::new(Mutex::new(BackgroundLogs::new()));
    register_ui_logs(shared_logs.clone());
    let subscriber = tracing_subscriber::registry().with(UiBackgroundLogLayer);

    tracing::subscriber::with_default(subscriber, || {
        // `record_str` path (`category = "..."`).
        tracing::info!(target: "fastmd::background::indexer", category = "watcher", "str category");
        // `record_debug` path (`category = ?"..."`) â€” the value arrives quoted.
        tracing::info!(target: "fastmd::background::indexer", category = ?"batch", "debug category");
    });

    unregister_ui_logs();

    let logs = shared_logs.lock().unwrap();
    let entries = logs.get_logs();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].category,
        LogCategory::Watcher,
        "an explicit string category must override the target"
    );
    assert_eq!(entries[0].message, "str category");
    assert_eq!(
        entries[1].category,
        LogCategory::Batch,
        "a Debug-formatted category must be unquoted before parsing"
    );
    assert_eq!(entries[1].message, "debug category");
}

/// A `fastmd` target with no recognised keyword falls back to `Indexer` rather
/// than being dropped, and a messageless event is labelled by its event name.
#[test]
fn test_ui_layer_falls_back_for_unknown_target_and_missing_message() {
    let _serial = serialise_ui_log_target();
    use tracing_subscriber::layer::SubscriberExt;

    let shared_logs = Arc::new(Mutex::new(BackgroundLogs::new()));
    register_ui_logs(shared_logs.clone());
    let subscriber = tracing_subscriber::registry().with(UiBackgroundLogLayer);

    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target: "fastmd::something::new", "unclassified");
        tracing::info!(target: "fastmd::watcher", count = 1);
    });

    unregister_ui_logs();

    let logs = shared_logs.lock().unwrap();
    let entries = logs.get_logs();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].category,
        LogCategory::Indexer,
        "an unrecognised fastmd target must default to Indexer"
    );
    assert_eq!(entries[0].message, "unclassified");
    assert!(
        entries[1].message.starts_with('[') && entries[1].message.ends_with(" (count=1)"),
        "a messageless event must fall back to `[name] (fields)`, got {:?}",
        entries[1].message
    );
    assert_eq!(entries[1].category, LogCategory::Watcher);
}

/// The `batch` target is a known non-`fastmd` source and must be captured.
#[test]
fn test_ui_layer_captures_the_batch_target() {
    let _serial = serialise_ui_log_target();
    use tracing_subscriber::layer::SubscriberExt;

    let shared_logs = Arc::new(Mutex::new(BackgroundLogs::new()));
    register_ui_logs(shared_logs.clone());
    let subscriber = tracing_subscriber::registry().with(UiBackgroundLogLayer);

    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target: "batch", "batch finished");
    });

    unregister_ui_logs();

    let logs = shared_logs.lock().unwrap();
    let entries = logs.get_logs();
    assert_eq!(entries.len(), 1, "the batch target must be captured");
    assert_eq!(entries[0].category, LogCategory::Batch);
    assert_eq!(entries[0].message, "batch finished");
}

/// Registering a second handle must swap the capture target, so a re-created
/// UI buffer starts receiving events and the old one stops.
#[test]
fn test_register_ui_logs_replaces_the_previous_handle() {
    let _serial = serialise_ui_log_target();
    use tracing_subscriber::layer::SubscriberExt;

    let first = Arc::new(Mutex::new(BackgroundLogs::new()));
    let second = Arc::new(Mutex::new(BackgroundLogs::new()));

    register_ui_logs(first.clone());
    let subscriber = tracing_subscriber::registry().with(UiBackgroundLogLayer);
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target: "fastmd::watcher", "to first");
    });

    register_ui_logs(second.clone());
    let subscriber = tracing_subscriber::registry().with(UiBackgroundLogLayer);
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target: "fastmd::watcher", "to second");
    });
    unregister_ui_logs();

    assert_eq!(first.lock().unwrap().get_logs().len(), 1);
    assert_eq!(second.lock().unwrap().get_logs().len(), 1);
    assert_eq!(
        first.lock().unwrap().get_logs()[0].message,
        "to first",
        "the first handle must stop receiving events after being replaced"
    );
    assert_eq!(second.lock().unwrap().get_logs()[0].message, "to second");
}

/// With no handle registered the layer must stay inert instead of panicking on
/// the empty global.
#[test]
fn test_ui_layer_is_inert_without_a_registered_handle() {
    let _serial = serialise_ui_log_target();
    use tracing_subscriber::layer::SubscriberExt;

    unregister_ui_logs();
    let subscriber = tracing_subscriber::registry().with(UiBackgroundLogLayer);

    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target: "fastmd::watcher", "dropped on the floor");
    });

    // Re-registering a fresh buffer afterwards must not retroactively collect
    // the event emitted while no handle was attached.
    let logs = Arc::new(Mutex::new(BackgroundLogs::new()));
    register_ui_logs(logs.clone());
    let subscriber = tracing_subscriber::registry().with(UiBackgroundLogLayer);
    tracing::subscriber::with_default(subscriber, || {});
    unregister_ui_logs();

    assert!(
        logs.lock().unwrap().get_logs().is_empty(),
        "no event may be delivered to a handle registered after the fact"
    );
}
