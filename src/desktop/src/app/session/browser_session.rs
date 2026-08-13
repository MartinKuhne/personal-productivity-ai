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
//! ## Always-compiled, conditional internals
//!
//! The session and its types ([`BrowserSession`], [`PageHandle`],
//! [`SessionError`]) are always compiled so the `agent` and `ui`
//! modules can hold an `Arc<BrowserSession>` without sprinkling
//! `#[cfg(feature = "browser")]` across every consumer. The
//! Playwright-backed internals are gated: when the `browser`
//! feature is off, the session is a stub and every operation that
//! requires a live browser returns
//! [`SessionError::Disabled`]. The `playwright-rs` dependency is
//! only pulled in when the `browser` feature is enabled.
//!
//! **Home:** this type lives in `crate::app::session` so that
//! the LLM agent and the application orchestrator can share it
//! without either having to reach into the Playwright wrapper
//! (`crate::app::browser`) or the file-watcher plumbing
//! (`crate::app::watcher`) directly. The old paths still
//! re-export this module for backwards compatibility.

use crate::config::{AppConfig, ResolvedBrowserConfig};
use std::path::PathBuf;
use std::sync::Mutex;
#[cfg(feature = "browser")]
use std::time::{Duration, Instant};

#[cfg(feature = "browser")]
use playwright_rs::LaunchOptions;
#[cfg(feature = "browser")]
use playwright_rs::protocol::{
    Browser, BrowserContext, BrowserContextOptions, Page, Playwright, StorageState,
};

/// Errors that can surface from the session. We keep a small
/// enum instead of leaking `playwright_rs::Error` everywhere so
/// the tool executor can convert to a stable error string
/// without depending on the underlying crate's type.
#[derive(Debug)]
pub enum SessionError {
    /// User has the Browser tool group disabled. Tools that need
    /// the session should check this first. Returned by the
    /// stub implementations of the session methods when the
    /// `browser` Cargo feature is off.
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

/// Convenience alias for the long-lived browser triple. Returned
/// by [`BrowserSession::page`] and passed to every `Tool::execute`
/// that needs to drive the browser.
///
/// When the `browser` Cargo feature is off the struct is a
/// zero-sized type; consumers that read the `page` / `context` /
/// `browser` fields must be gated on the feature too.
pub struct PageHandle {
    /// Persistent page. Same instance across calls in a single
    /// session; new instance after an idle-timeout close.
    #[cfg(feature = "browser")]
    pub page: Page,
    /// The owning context, kept so storage can be saved without
    /// keeping a separate handle.
    #[cfg(feature = "browser")]
    pub context: BrowserContext,
    /// The owning browser, kept so the session can be closed
    /// without losing the context first.
    #[cfg(feature = "browser")]
    pub browser: Browser,
}

impl std::fmt::Debug for PageHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageHandle").finish_non_exhaustive()
    }
}

#[cfg(feature = "browser")]
struct Inner {
    live: Option<LiveSession>,
    last_used: Option<Instant>,
}

#[cfg(feature = "browser")]
struct LiveSession {
    _playwright: Playwright,
    browser: Browser,
    context: BrowserContext,
    page: Page,
}

/// Long-lived headless browser session. Cheap to construct; the
/// Playwright triple is built on the first call to
/// [`BrowserSession::page`].
pub struct BrowserSession {
    config: Mutex<ResolvedBrowserConfig>,
    #[cfg(feature = "browser")]
    inner: Mutex<Inner>,
}

impl BrowserSession {
    /// Build a session from the current [`AppConfig`]. Does not
    /// touch the network or spawn any process.
    pub fn new(config: &AppConfig) -> Self {
        let resolved = config.browser.resolve(&config.content_libraries);
        Self {
            config: Mutex::new(resolved),
            #[cfg(feature = "browser")]
            inner: Mutex::new(Inner {
                live: None,
                last_used: None,
            }),
        }
    }

    /// Build a session from a pre-resolved config. Used by tests
    /// that don't want to go through the env-dependent defaults
    /// of [`crate::config::BrowserConfig::resolve`].
    pub fn with_resolved(resolved: ResolvedBrowserConfig) -> Self {
        Self {
            config: Mutex::new(resolved),
            #[cfg(feature = "browser")]
            inner: Mutex::new(Inner {
                live: None,
                last_used: None,
            }),
        }
    }

    /// Cheap snapshot of the resolved config (cloned).
    pub fn config(&self) -> ResolvedBrowserConfig {
        self.config.lock().expect("config mutex poisoned").clone()
    }

    /// True if a live Firefox process is currently held.
    #[cfg(feature = "browser")]
    pub fn is_live(&self) -> bool {
        self.inner
            .lock()
            .expect("inner mutex poisoned")
            .live
            .is_some()
    }

    /// Stub for the no-`browser`-feature build; always reports
    /// no live session.
    #[cfg(not(feature = "browser"))]
    pub fn is_live(&self) -> bool {
        false
    }

    /// Enforce the idle timeout. Called once per UI frame.
    /// Returns `true` if the session was closed by this call.
    #[cfg(feature = "browser")]
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

    /// Stub for the no-`browser`-feature build; idle-timeout is
    /// a no-op without Playwright.
    #[cfg(not(feature = "browser"))]
    pub fn tick(&self) -> bool {
        false
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

    /// Return a live [`PageHandle`], launching the browser on
    /// first use. Updates `last_used` so the idle-timeout clock
    /// is reset on every call.
    #[cfg(feature = "browser")]
    pub fn page(&self) -> Result<PageHandle, SessionError> {
        {
            let inner = self.inner.lock().expect("inner mutex poisoned");
            if let Some(live) = &inner.live {
                let handle = PageHandle {
                    page: live.page.clone(),
                    context: live.context.clone(),
                    browser: live.browser.clone(),
                };
                return Ok(handle);
            }
        }
        self.launch_and_return()
    }

    /// Stub for the no-`browser`-feature build; reports the
    /// browser tool group as disabled.
    #[cfg(not(feature = "browser"))]
    pub fn page(&self) -> Result<PageHandle, SessionError> {
        Err(SessionError::Disabled)
    }

    /// Persist the current cookie jar + local storage to the
    /// configured `storage_state_path`. Called from the tool
    /// impls after every mutating call.
    #[cfg(feature = "browser")]
    pub fn save_storage(&self) -> Result<(), SessionError> {
        let live = {
            let inner = self.inner.lock().expect("inner mutex poisoned");
            match &inner.live {
                Some(live) => (live.context.clone(), live._playwright.clone()),
                None => return Ok(()),
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

    /// Stub for the no-`browser`-feature build; nothing to save.
    #[cfg(not(feature = "browser"))]
    pub fn save_storage(&self) -> Result<(), SessionError> {
        Ok(())
    }

    /// Close the live browser (if any) and delete the persisted
    /// storage file. Invoked from the UI's "Forget Browser
    /// Session" action — gives the user a clean logout.
    #[cfg(feature = "browser")]
    pub fn forget(&self) -> Result<(), SessionError> {
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

    /// Stub for the no-`browser`-feature build; nothing to
    /// forget and no storage file to remove.
    #[cfg(not(feature = "browser"))]
    pub fn forget(&self) -> Result<(), SessionError> {
        Ok(())
    }

    /// Launch the browser, create a context + page, and stash
    /// the triple. Called on the slow path of `page()`.
    #[cfg(feature = "browser")]
    fn launch_and_return(&self) -> Result<PageHandle, SessionError> {
        {
            let mut inner = self.inner.lock().expect("inner mutex poisoned");
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

#[cfg(feature = "browser")]
struct ResolvedLaunch {
    headless: bool,
    page_load_timeout_ms: u64,
    storage_state_path: PathBuf,
}

#[cfg(feature = "browser")]
async fn launch_firefox(cfg: ResolvedLaunch) -> Result<LiveSession, String> {
    let playwright = Playwright::launch().await.map_err(|e| e.to_string())?;
    let firefox = playwright.firefox();
    let options = LaunchOptions::default().headless(cfg.headless);
    let browser = firefox
        .launch_with_options(options)
        .await
        .map_err(|e| e.to_string())?;

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

#[cfg(all(test, feature = "browser"))]
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
        let cfg = BrowserConfig::default();
        let libs: Vec<ContentLibrary> = vec![];
        let resolved = cfg.resolve(&libs);
        assert!(!resolved.storage_state_path.as_os_str().is_empty());
    }

    fn tempdir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }
}

#[cfg(feature = "browser")]
impl crate::agent::tools::browser::BrowserAutomationExt for BrowserSession {
    fn navigate(&self, url: &str) -> Result<(String, String), String> {
        let handle = self.page().map_err(|e| e.to_string())?;
        crate::agent::tools::blocking::block_on(async { handle.page.goto(url, None).await })
            .map_err(|e| e.to_string())?;
        let final_url = handle.page.url().unwrap_or_default();
        let title = crate::agent::tools::blocking::block_on(async { handle.page.title().await })
            .unwrap_or_default();
        Ok((final_url, title))
    }

    fn get_page_state(&self) -> Result<(String, String, String, usize), String> {
        let handle = self.page().map_err(|e| e.to_string())?;
        let script = r#"
            () => {
                const elements = document.querySelectorAll('a, button, input, select, textarea');
                const out = [];
                elements.forEach((el, i) => {
                    el.setAttribute('data-agent-id', i);
                    out.push({
                        agent_id: i,
                        tag: el.tagName,
                        text: (el.innerText || el.value || '').slice(0, 200),
                        placeholder: el.getAttribute('placeholder') || '',
                        name: el.getAttribute('name') || '',
                        type: el.getAttribute('type') || '',
                        href: el.getAttribute('href') || null
                    });
                });
                return out;
            }
        "#;
        let value: serde_json::Value = crate::agent::tools::blocking::block_on(async {
            handle.page.evaluate(script, None::<&()>).await
        })
        .map_err(|e| e.to_string())?;

        let elements_json = serde_json::to_string(&value).unwrap_or_else(|_| "[]".to_string());
        let total = value.as_array().map(|a| a.len()).unwrap_or(0);
        let url = handle.page.url().unwrap_or_default();
        let title = crate::agent::tools::blocking::block_on(async { handle.page.title().await })
            .unwrap_or_default();
        Ok((url, title, elements_json, total))
    }

    fn click(&self, selector: &str) -> Result<(), String> {
        let handle = self.page().map_err(|e| e.to_string())?;
        let locator = handle.page.locator(selector);
        crate::agent::tools::blocking::block_on(async { locator.click(None).await })
            .map_err(|e| e.to_string())
    }

    fn fill_input(&self, selector: &str, text: &str) -> Result<(), String> {
        let handle = self.page().map_err(|e| e.to_string())?;
        let locator = handle.page.locator(selector);
        crate::agent::tools::blocking::block_on(async { locator.fill(text, None).await })
            .map_err(|e| e.to_string())
    }

    fn select_dropdown(&self, selector: &str, value: &str) -> Result<(), String> {
        let handle = self.page().map_err(|e| e.to_string())?;
        let locator = handle.page.locator(selector);
        crate::agent::tools::blocking::block_on(async { locator.select_option(value, None).await })
            .map_err(|e| e.to_string())
    }

    fn press_key(&self, key: &str) -> Result<(), String> {
        let handle = self.page().map_err(|e| e.to_string())?;
        crate::agent::tools::blocking::block_on(async {
            handle.page.keyboard().press(key, None).await
        })
        .map_err(|e| e.to_string())
    }

    fn evaluate_js(&self, script: &str) -> Result<serde_json::Value, String> {
        let handle = self.page().map_err(|e| e.to_string())?;
        crate::agent::tools::blocking::block_on(async {
            handle.page.evaluate(script, None::<&()>).await
        })
        .map_err(|e| e.to_string())
    }

    fn screenshot(
        &self,
        filename: &str,
        full_page: bool,
    ) -> Result<(std::path::PathBuf, Vec<u8>), String> {
        let out_path = self
            .resolve_screenshot_path(filename)
            .map_err(|e| e.to_string())?;
        let handle = self.page().map_err(|e| e.to_string())?;
        let bytes = crate::agent::tools::blocking::block_on(async {
            use playwright_rs::ScreenshotOptions;
            let opts = ScreenshotOptions::builder().full_page(full_page).build();
            handle.page.screenshot(Some(opts)).await
        })
        .map_err(|e| e.to_string())?;
        Ok((out_path, bytes))
    }

    fn save_storage(&self) -> Result<(), String> {
        BrowserSession::save_storage(self).map_err(|e| e.to_string())
    }

    fn resolve_screenshot_path(&self, filename: &str) -> Result<std::path::PathBuf, String> {
        BrowserSession::resolve_screenshot_path(self, filename).map_err(|e| e.to_string())
    }
}

#[cfg(not(feature = "browser"))]
impl crate::agent::tools::browser::BrowserAutomationExt for BrowserSession {
    fn navigate(&self, _url: &str) -> Result<(String, String), String> {
        Err("Browser automation disabled (build without 'browser' feature)".into())
    }
    fn get_page_state(&self) -> Result<(String, String, String, usize), String> {
        Err("Browser automation disabled (build without 'browser' feature)".into())
    }
    fn click(&self, _selector: &str) -> Result<(), String> {
        Err("Browser automation disabled (build without 'browser' feature)".into())
    }
    fn fill_input(&self, _selector: &str, _text: &str) -> Result<(), String> {
        Err("Browser automation disabled (build without 'browser' feature)".into())
    }
    fn select_dropdown(&self, _selector: &str, _value: &str) -> Result<(), String> {
        Err("Browser automation disabled (build without 'browser' feature)".into())
    }
    fn press_key(&self, _key: &str) -> Result<(), String> {
        Err("Browser automation disabled (build without 'browser' feature)".into())
    }
    fn evaluate_js(&self, _script: &str) -> Result<serde_json::Value, String> {
        Err("Browser automation disabled (build without 'browser' feature)".into())
    }
    fn screenshot(
        &self,
        _filename: &str,
        _full_page: bool,
    ) -> Result<(std::path::PathBuf, Vec<u8>), String> {
        Err("Browser automation disabled (build without 'browser' feature)".into())
    }
    fn save_storage(&self) -> Result<(), String> {
        Err("Browser automation disabled (build without 'browser' feature)".into())
    }
    fn resolve_screenshot_path(&self, filename: &str) -> Result<std::path::PathBuf, String> {
        BrowserSession::resolve_screenshot_path(self, filename).map_err(|e| e.to_string())
    }
}
