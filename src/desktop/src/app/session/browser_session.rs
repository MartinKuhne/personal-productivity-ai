//! Long-lived headless Firefox session.
//!
//! [`BrowserSession`] holds an optional
//! `Playwright` + `Browser` + `BrowserContext` + `Page` chain and
//! reuses the same `Page` across every mutating tool call so the
//! LLM can drive multi-step flows (log in once, click, fill,
//! screenshot). The session launches lazily on the first call
//! to [`BrowserSession::page`] and persists cookies / local
//! storage to the `storage_state` JSON file configured under
//! `browser.storage_state_path` after every mutating call.
//!
//! Idle timeout: [`BrowserSession::tick`] should be called once
//! per UI frame; it closes the live browser after
//! `idle_timeout_seconds` of tool-call silence and frees the
//! Firefox subprocess. The next call relaunches and reloads
//! from the storage file, so persistent login survives idle
//! timeouts.
//!
//! **Home:** this type lives in `crate::app::session` so that
//! the LLM agent and the application orchestrator can share it
//! without either having to reach into the Playwright wrapper
//! (`crate::app::browser`) or the file-watcher plumbing
//! (`crate::app::watcher`) directly. The old paths still
//! re-export this module for backwards compatibility.

use crate::config::{AppConfig, ResolvedBrowserConfig};
use playwright_rs::LaunchOptions;
use playwright_rs::protocol::{
    Browser, BrowserContext, BrowserContextOptions, Page, Playwright, StorageState,
};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Convenience alias for the long-lived browser triple. Returned
/// by [`BrowserSession::page`] and passed to every `Tool::execute`
/// that needs to drive the browser.
pub struct PageHandle {
    /// Persistent page. Same instance across calls in a single
    /// session; new instance after an idle-timeout close.
    pub page: Page,
    /// The owning context, kept so storage can be saved without
    /// keeping a separate handle.
    pub context: BrowserContext,
    /// The owning browser, kept so the session can be closed
    /// without losing the context first.
    pub browser: Browser,
}

impl std::fmt::Debug for PageHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageHandle").finish_non_exhaustive()
    }
}

/// Errors that can surface from the session. We keep a small
/// enum instead of leaking `playwright_rs::Error` everywhere so
/// the tool executor can convert to a stable error string
/// without depending on the underlying crate's type.
#[derive(Debug)]
pub enum SessionError {
    /// User has the Browser tool group disabled. Tools that need
    /// the session should check this first.
    Disabled,
    /// `BrowserConfig.browser_type` is not one of the supported
    /// channels (currently only `"firefox"`).
    UnsupportedBrowserType(String),
    /// The browser process failed to launch (e.g. Firefox not
    /// installed; `playwright install firefox` required).
    Launch(String),
    /// The user-supplied filename was rejected by the
    /// screenshot path policy.
    InvalidFilename(String),
    /// Generic I/O error (storage file read/write, mkdir, ...).
    Io(String),
    /// Anything else (Playwright-side error, page navigation
    /// timeout, JS error, ...).
    Other(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => write!(f, "browser tool group is disabled"),
            Self::UnsupportedBrowserType(t) => {
                write!(f, "unsupported browser type '{}' (only 'firefox')", t)
            }
            Self::Launch(s) => write!(f, "browser launch failed: {}", s),
            Self::InvalidFilename(s) => write!(f, "invalid screenshot filename: {}", s),
            Self::Io(s) => write!(f, "I/O error: {}", s),
            Self::Other(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for SessionError {}

/// Inner state guarded by the session mutex. The mutex is held
/// for short critical sections only (state construction /
/// inspection); long Playwright work runs outside the lock.
struct Inner {
    /// Lazily launched Playwright triple. `None` until the first
    /// `page()` call OR the very first config-driven check.
    live: Option<LiveSession>,
    /// Timestamp of the most recent mutating tool call. `None`
    /// until the first call. Compared against `now()` in `tick`.
    last_used: Option<Instant>,
}

/// A live Playwright triple: the top-level `Playwright` driver
/// (manages the websocket to the playwright-rs server binary), the
/// `Browser` process, the `BrowserContext` (cookie jar), and the
/// `Page` (tab).
struct LiveSession {
    _playwright: Playwright,
    browser: Browser,
    context: BrowserContext,
    page: Page,
}

/// Long-lived headless browser session. Cheap to construct; the
/// Playwright triple is built on the first call to
/// [`BrowserSession::page`].
///
/// All mutating methods acquire the internal [`Mutex`] briefly,
/// then call into Playwright outside the lock. The session is
/// therefore safe to share across the UI thread and the agent
/// thread via `Arc<BrowserSession>`.
pub struct BrowserSession {
    inner: Mutex<Inner>,
    config: Mutex<ResolvedBrowserConfig>,
}

impl BrowserSession {
    /// Build a session from the current [`AppConfig`]. Does not
    /// touch the network or spawn any process.
    pub fn new(config: &AppConfig) -> Self {
        let resolved = config.browser.resolve(&config.content_libraries);
        Self {
            inner: Mutex::new(Inner {
                live: None,
                last_used: None,
            }),
            config: Mutex::new(resolved),
        }
    }

    /// Build a session from a pre-resolved config. Used by tests
    /// that don't want to go through the env-dependent defaults
    /// of [`crate::config::BrowserConfig::resolve`].
    pub fn with_resolved(resolved: ResolvedBrowserConfig) -> Self {
        Self {
            inner: Mutex::new(Inner {
                live: None,
                last_used: None,
            }),
            config: Mutex::new(resolved),
        }
    }

    /// Cheap snapshot of the resolved config (cloned).
    pub fn config(&self) -> ResolvedBrowserConfig {
        self.config.lock().expect("config mutex poisoned").clone()
    }

    /// Return a live [`PageHandle`], launching the browser on
    /// first use. Updates `last_used` so the idle-timeout clock
    /// is reset on every call.
    ///
    /// Blocks the caller on a process-wide Tokio runtime (the
    /// same one the CalDAV / CardDAV tools use via
    /// `tools::blocking::block_on`). The lock is held only across
    /// the live-session check; the actual launch + page creation
    /// happens on the Tokio runtime.
    pub fn page(&self) -> Result<PageHandle, SessionError> {
        // Fast path: already live, just touch the timestamp.
        {
            let inner = self.inner.lock().expect("inner mutex poisoned");
            if let Some(live) = &inner.live {
                // We can't return a reference into the lock; the
                // caller needs an owned PageHandle. Clone the
                // inner types — they're cheap to clone (Arc
                // handles inside playwright-rs).
                let handle = PageHandle {
                    page: live.page.clone(),
                    context: live.context.clone(),
                    browser: live.browser.clone(),
                };
                return Ok(handle);
            }
        }

        // Slow path: need to launch. Drop the lock first so a
        // concurrent second call doesn't try to launch twice.
        self.launch_and_return()
    }

    /// Persist the current cookie jar + local storage to the
    /// configured `storage_state_path`. Called from the tool
    /// impls after every mutating call. Silent on `Inner` lock
    /// contention so it never blocks the agent.
    ///
    /// TODO (post-merge polish): debounce this so we only write
    /// once per N seconds even on bursty sessions. For v1 a
    /// write per call is fine; the JSON is small and the file
    /// is overwritten atomically.
    pub fn save_storage(&self) -> Result<(), SessionError> {
        let live = {
            let inner = self.inner.lock().expect("inner mutex poisoned");
            match &inner.live {
                Some(live) => (live.context.clone(), live._playwright.clone()),
                None => return Ok(()), // no live session; nothing to save
            }
        };
        let (context, _playwright) = live;
        let state =
            crate::agent::tools::blocking::block_on(async { context.storage_state().await })
                .map_err(|e| SessionError::Other(format!("storage_state: {}", e)))?;

        let path = self
            .config
            .lock()
            .expect("config mutex poisoned")
            .storage_state_path
            .clone();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SessionError::Io(e.to_string()))?;
        }
        let json = serde_json::to_string_pretty(&state)
            .map_err(|e| SessionError::Other(format!("serialize storage_state: {}", e)))?;
        std::fs::write(&path, json).map_err(|e| SessionError::Io(e.to_string()))?;
        Ok(())
    }

    /// Close the live browser (if any) and delete the persisted
    /// storage file. Invoked from the UI's "Forget Browser
    /// Session" action — gives the user a clean logout.
    pub fn forget(&self) -> Result<(), SessionError> {
        // Drop the live session if any. The Playwright types are
        // not `Send`-droppable from arbitrary contexts, so we
        // hand the close-off to the Tokio runtime explicitly.
        let to_close = {
            let mut inner = self.inner.lock().expect("inner mutex poisoned");
            inner.last_used = None;
            inner.live.take()
        };
        if let Some(live) = to_close {
            crate::agent::tools::blocking::block_on(async {
                let _ = live.context.close().await;
                let _ = live.browser.close().await;
            });
        }

        let path = self
            .config
            .lock()
            .expect("config mutex poisoned")
            .storage_state_path
            .clone();
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(SessionError::Io(e.to_string())),
        }
        Ok(())
    }

    /// Enforce the idle timeout. Called once per UI frame.
    /// Returns `true` if the session was closed by this call.
    pub fn tick(&self) -> bool {
        let cfg = self.config.lock().expect("config mutex poisoned").clone();
        if cfg.idle_timeout_seconds == 0 {
            return false; // disabled
        }
        let to_close = {
            let mut inner = self.inner.lock().expect("inner mutex poisoned");
            match inner.last_used {
                Some(last) if last.elapsed() >= Duration::from_secs(cfg.idle_timeout_seconds) => {
                    inner.last_used = None;
                    inner.live.take()
                }
                _ => None,
            }
        };
        if let Some(live) = to_close {
            crate::agent::tools::blocking::block_on(async {
                let _ = live.context.close().await;
                let _ = live.browser.close().await;
            });
            true
        } else {
            false
        }
    }

    /// True if a live Firefox process is currently held.
    pub fn is_live(&self) -> bool {
        self.inner
            .lock()
            .expect("inner mutex poisoned")
            .live
            .is_some()
    }

    /// Validate and join a user-supplied screenshot filename
    /// against the configured screenshot directory. Returns the
    /// absolute output path on success. The filename must
    /// contain only `[A-Za-z0-9._-]`, must not be empty, must
    /// not start with `.`, must be ≤ 128 chars, and must not
    /// contain `..` (BRWS-CONF-002).
    pub fn resolve_screenshot_path(&self, filename: &str) -> Result<PathBuf, SessionError> {
        if filename.is_empty() {
            return Err(SessionError::InvalidFilename("filename is empty".into()));
        }
        if filename.len() > 128 {
            return Err(SessionError::InvalidFilename(
                "filename exceeds 128 characters".into(),
            ));
        }
        if filename.starts_with('.') {
            return Err(SessionError::InvalidFilename(
                "filename may not start with '.'".into(),
            ));
        }
        if filename.contains("..") {
            return Err(SessionError::InvalidFilename(
                "filename may not contain '..'".into(),
            ));
        }
        for ch in filename.chars() {
            if !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-') {
                return Err(SessionError::InvalidFilename(format!(
                    "filename contains forbidden character '{}'",
                    ch
                )));
            }
        }
        let dir = self
            .config
            .lock()
            .expect("config mutex poisoned")
            .screenshot_dir
            .clone();
        std::fs::create_dir_all(&dir).map_err(|e| SessionError::Io(e.to_string()))?;
        Ok(dir.join(filename))
    }

    /// Launch the browser, create a context + page, and stash
    /// the triple. Called on the slow path of `page()`. Drops
    /// the inner lock before doing the I/O so a concurrent
    /// second call sees a consistent "launching" state.
    fn launch_and_return(&self) -> Result<PageHandle, SessionError> {
        // Mark that we are about to launch by setting last_used
        // — the tick logic already considers a recently-active
        // session as "not yet idle", which is what we want
        // during a slow launch.
        {
            let mut inner = self.inner.lock().expect("inner mutex poisoned");
            // Double-check: another thread may have raced us.
            if let Some(live) = &inner.live {
                return Ok(PageHandle {
                    page: live.page.clone(),
                    context: live.context.clone(),
                    browser: live.browser.clone(),
                });
            }
            inner.last_used = Some(Instant::now());
        }

        let cfg = self.config.lock().expect("config mutex poisoned").clone();
        if cfg.browser_type != "firefox" {
            return Err(SessionError::UnsupportedBrowserType(cfg.browser_type));
        }

        let resolved = ResolvedLaunch {
            headless: cfg.headless,
            page_load_timeout_ms: cfg.page_load_timeout_ms,
            storage_state_path: cfg.storage_state_path.clone(),
        };

        // Run the entire launch on the Tokio runtime so we
        // never block the UI thread on a Playwright future.
        let live =
            crate::agent::tools::blocking::block_on(async move { launch_firefox(resolved).await })
                .map_err(SessionError::Launch)?;

        let handle = PageHandle {
            page: live.page.clone(),
            context: live.context.clone(),
            browser: live.browser.clone(),
        };

        {
            let mut inner = self.inner.lock().expect("inner mutex poisoned");
            inner.live = Some(live);
            inner.last_used = Some(Instant::now());
        }
        Ok(handle)
    }
}

/// Launch-time config snapshot — extracted from
/// [`ResolvedBrowserConfig`] so we can move it into the async
/// block without holding the config mutex.
struct ResolvedLaunch {
    headless: bool,
    page_load_timeout_ms: u64,
    storage_state_path: PathBuf,
}

async fn launch_firefox(cfg: ResolvedLaunch) -> Result<LiveSession, String> {
    let playwright = Playwright::launch().await.map_err(|e| e.to_string())?;
    let firefox = playwright.firefox();
    let options = LaunchOptions::default().headless(cfg.headless);
    let browser = firefox
        .launch_with_options(options)
        .await
        .map_err(|e| e.to_string())?;

    // Build the BrowserContextOptions via its builder. If a
    // storage_state file exists from a previous run, pass it as
    // the initial state so the user's cookies are restored.
    let mut builder = BrowserContextOptions::builder();
    if cfg.storage_state_path.exists() {
        match std::fs::read_to_string(&cfg.storage_state_path) {
            Ok(json) => match serde_json::from_str::<StorageState>(&json) {
                Ok(state) => {
                    builder = builder.storage_state(state);
                }
                Err(e) => {
                    tracing::warn!(
                        name = "app.browser.storage_state_parse_failed",
                        path = %cfg.storage_state_path.display(),
                        error = %e,
                        "Failed to parse storage_state file; launching with empty context"
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    name = "app.browser.storage_state_read_failed",
                    path = %cfg.storage_state_path.display(),
                    error = %e,
                    "Failed to read storage_state file; launching with empty context"
                );
            }
        }
    }
    let ctx_options = builder.build();

    let context = browser
        .new_context_with_options(ctx_options)
        .await
        .map_err(|e| e.to_string())?;
    let page = context.new_page().await.map_err(|e| e.to_string())?;

    // Apply the per-page load timeout by default. Tools that
    // need a different timeout can override per-call.
    let _ = page
        .set_default_timeout(cfg.page_load_timeout_ms as f64)
        .await;

    Ok(LiveSession {
        _playwright: playwright,
        browser,
        context,
        page,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BrowserConfig, ContentLibrary};
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
        // Sanity check that resolve() does not panic and
        // returns a non-empty storage_state_path.
        let cfg = BrowserConfig::default();
        let libs: Vec<ContentLibrary> = vec![];
        let resolved = cfg.resolve(&libs);
        assert!(!resolved.storage_state_path.as_os_str().is_empty());
    }

    fn tempdir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }
}
