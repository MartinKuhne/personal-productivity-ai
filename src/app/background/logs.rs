//! Background log manager — bounded ring buffer of log entries, non-blocking logging framework setup, and UI layer.
//!
//! Unit tests live in the sibling `logs_tests.rs` sidecar.

use crate::background::models::{BackgroundLogEntry, LogCategory};
use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

/// Maximum number of log entries to retain in memory.
/// This prevents memory exhaustion from runaway background processes.
pub const MAX_LOG_ENTRIES: usize = 10_000;

/// Default filename for the persistent background process log.
pub const LOG_FILENAME: &str = "background-process.log";

/// Tracing target used when routing events already captured by the background channel.
pub const TARGET_BACKGROUND_CHANNEL: &str = "fastmd::background::channel";

static UI_LOG_TARGET: RwLock<Option<SharedBackgroundLogs>> = RwLock::new(None);

/// Registers a shared background logs handle for UI display.
pub fn register_ui_logs(logs: SharedBackgroundLogs) {
    if let Ok(mut guard) = UI_LOG_TARGET.write() {
        *guard = Some(logs);
    }
}

/// Unregisters the shared background logs handle.
pub fn unregister_ui_logs() {
    if let Ok(mut guard) = UI_LOG_TARGET.write() {
        *guard = None;
    }
}

/// Parses a string into a [`LogCategory`], case-insensitively.
pub fn parse_log_category(s: &str) -> Option<LogCategory> {
    let lower = s.to_ascii_lowercase();
    if lower.contains("indexer") {
        Some(LogCategory::Indexer)
    } else if lower.contains("watcher") {
        Some(LogCategory::Watcher)
    } else if lower.contains("pdf") {
        Some(LogCategory::PdfConverter)
    } else if lower.contains("vision") || lower.contains("image") {
        Some(LogCategory::ImageVision)
    } else if lower.contains("llm") || lower.contains("agent") || lower.contains("tool") {
        Some(LogCategory::LlmTools)
    } else if lower.contains("print") {
        Some(LogCategory::Print)
    } else if lower.contains("batch") {
        Some(LogCategory::Batch)
    } else {
        None
    }
}

/// Pure helper resolving log directory from environment variables without mutating global state.
pub fn log_dir_from_env(
    log_dir_override: Option<&str>,
    config_path_override: Option<&str>,
    appdata: Option<&str>,
    userprofile: Option<&str>,
) -> PathBuf {
    if let Some(dir) = log_dir_override
        && !dir.is_empty()
    {
        return PathBuf::from(dir);
    }
    if let Some(config_path) = config_path_override
        && !config_path.is_empty()
        && let Some(parent) = Path::new(config_path).parent()
    {
        return parent.join("logs");
    }
    if let Some(app_data) = appdata {
        PathBuf::from(app_data).join("fastmd").join("logs")
    } else if let Some(user_profile) = userprofile {
        PathBuf::from(user_profile).join(".fastmd").join("logs")
    } else {
        PathBuf::from("logs")
    }
}

/// Resolves the default log directory for FastMD.
///
/// In test environments (`cfg(test)`), contains a runtime panic shield to prevent tests
/// from touching live user directories (`[RUST-006]`).
pub fn get_log_dir() -> PathBuf {
    #[cfg(test)]
    {
        if std::env::var("FASTMD_LOG_DIR").is_err() && std::env::var("FASTMD_CONFIG_PATH").is_err()
        {
            panic!(
                "RUST-006 runtime panic shield: unit tests must not access platform-default log directory"
            );
        }
    }
    let log_dir = std::env::var("FASTMD_LOG_DIR").ok();
    let config_path = std::env::var("FASTMD_CONFIG_PATH").ok();
    let appdata = std::env::var("APPDATA").ok();
    let userprofile = std::env::var("USERPROFILE").ok();
    log_dir_from_env(
        log_dir.as_deref(),
        config_path.as_deref(),
        appdata.as_deref(),
        userprofile.as_deref(),
    )
}

/// A `tracing_subscriber::Layer` that captures background and application events into the UI log buffer.
pub struct UiBackgroundLogLayer;

impl<S> tracing_subscriber::Layer<S> for UiBackgroundLogLayer
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let target = metadata.target();

        // Events routed from ProcessEvent::LogEntry are tagged with target "fastmd::background::channel".
        // To prevent duplicating logs already pushed into BackgroundLogs, we ignore this target.
        if target == TARGET_BACKGROUND_CHANNEL {
            return;
        }

        // Only capture FastMD events or known background targets
        if !target.starts_with("fastmd") && target != "batch" {
            return;
        }

        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);

        let category = visitor
            .category
            .or_else(|| parse_log_category(target))
            .unwrap_or(LogCategory::Indexer);

        let mut message = if visitor.message.is_empty() {
            format!("[{}]", metadata.name())
        } else {
            visitor.message
        };

        if !visitor.fields.is_empty() {
            let extra = visitor
                .fields
                .into_iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(", ");
            message = format!("{message} ({extra})");
        }

        let entry = BackgroundLogEntry {
            timestamp: chrono::Local::now(),
            category,
            message,
        };

        if let Ok(guard) = UI_LOG_TARGET.read()
            && let Some(shared_logs) = guard.as_ref()
            && let Ok(mut logs) = shared_logs.lock()
        {
            logs.push_log(entry);
        }
    }
}

#[derive(Default)]
struct EventVisitor {
    message: String,
    category: Option<LogCategory>,
    fields: Vec<(String, String)>,
}

impl tracing::field::Visit for EventVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        if name == "message" {
            let s = format!("{value:?}");
            self.message = if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                s[1..s.len() - 1].to_string()
            } else {
                s
            };
        } else if name == "category" {
            let s = format!("{value:?}");
            let clean = s.trim_matches('"');
            self.category = parse_log_category(clean);
        } else {
            self.fields.push((name.to_string(), format!("{value:?}")));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let name = field.name();
        if name == "message" {
            self.message = value.to_string();
        } else if name == "category" {
            self.category = parse_log_category(value);
        } else {
            self.fields.push((name.to_string(), value.to_string()));
        }
    }
}

/// Initializes the logging framework with non-blocking file appender and UI layer.
///
/// Returns a [`tracing_appender::non_blocking::WorkerGuard`] that MUST be retained for
/// the lifetime of the application to ensure all queued log messages are flushed upon exit.
pub fn init_logging(
    log_dir: PathBuf,
) -> Result<tracing_appender::non_blocking::WorkerGuard, std::io::Error> {
    let (non_blocking_file, guard) = match std::fs::create_dir_all(&log_dir) {
        Ok(()) => {
            let appender = tracing_appender::rolling::never(&log_dir, LOG_FILENAME);
            tracing_appender::non_blocking(appender)
        }
        Err(e) => {
            eprintln!("Failed to create log directory {}: {e}", log_dir.display());
            tracing_appender::non_blocking(std::io::sink())
        }
    };

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking_file)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true);

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(file_layer)
        .with(UiBackgroundLogLayer)
        .try_init();

    Ok(guard)
}

/// In-memory ring buffer of background log entries with UI filtering and search state.
pub struct BackgroundLogs {
    logs: VecDeque<BackgroundLogEntry>,
    /// Active category filter, if any.
    pub filter_category: Option<LogCategory>,
    /// Search substring filter.
    pub search_text: String,
    /// Whether auto-scrolling is enabled in the UI log panel.
    pub auto_scroll: bool,
    /// Visibility state of the background logs window.
    pub show_background_logs: bool,
}

impl Default for BackgroundLogs {
    fn default() -> Self {
        Self {
            logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            filter_category: None,
            search_text: String::new(),
            auto_scroll: true,
            show_background_logs: false,
        }
    }
}

impl BackgroundLogs {
    /// Creates a new, empty log buffer with default capacity.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a new log entry, evicting the oldest entry if `MAX_LOG_ENTRIES` is exceeded.
    pub fn push_log(&mut self, entry: BackgroundLogEntry) {
        if self.logs.len() >= MAX_LOG_ENTRIES {
            self.logs.pop_front();
        }
        self.logs.push_back(entry);
    }

    /// Returns a reference to the internal ring buffer of log entries.
    pub fn get_logs(&self) -> &VecDeque<BackgroundLogEntry> {
        &self.logs
    }

    /// Clears all log entries from the buffer.
    pub fn clear_logs(&mut self) {
        self.logs.clear();
    }

    /// Saves the current log buffer to disk synchronously.
    pub fn save_logs(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = File::create(path)?;
        self.write_logs_to(&mut file)
    }

    /// Serialize the log buffer into the provided writer.
    ///
    /// Splitting this out from `save_logs` makes the write path unit-
    /// testable with a mock `Write` (see `FailingWriter` in the test
    /// module) and ensures that mid-write failures propagate instead
    /// of being silently swallowed.
    pub(crate) fn write_logs_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        for log in &self.logs {
            let line = format!(
                "[{}] [{}] {}\n",
                log.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
                log.category,
                log.message
            );
            writer.write_all(line.as_bytes())?;
        }
        Ok(())
    }
}

pub type SharedBackgroundLogs = Arc<Mutex<BackgroundLogs>>;

// ---------------------------------------------------------------------------
// Tests live in the sibling `logs_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "logs_tests.rs"]
mod tests;
