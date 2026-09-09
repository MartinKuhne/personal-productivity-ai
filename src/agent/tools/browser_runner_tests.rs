use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Test double for [`BrowserRunner`] allowing deterministic test behavior.
#[derive(Clone, Debug, Default)]
pub struct MockBrowserRunner {
    pub return_html: Option<String>,
    pub return_headers: HashMap<String, String>,
    pub should_timeout: bool,
    pub fail_status: Option<i32>,
    pub was_called: Arc<AtomicBool>,
}

impl MockBrowserRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_html(mut self, html: impl Into<String>) -> Self {
        self.return_html = Some(html.into());
        self
    }

    pub fn with_timeout(mut self) -> Self {
        self.should_timeout = true;
        self
    }

    pub fn with_failure(mut self, status: i32) -> Self {
        self.fail_status = Some(status);
        self
    }
}

impl BrowserRunner for MockBrowserRunner {
    fn render_url(&self, _url: &str, timeout: Duration) -> Result<RenderedDocument, BrowserError> {
        self.was_called.store(true, Ordering::SeqCst);

        if self.should_timeout {
            return Err(BrowserError::Timeout(timeout));
        }

        if let Some(status) = self.fail_status {
            return Err(BrowserError::ExecutionFailed {
                status,
                stderr: "simulated process failure".to_string(),
            });
        }

        Ok(RenderedDocument {
            html_body: self
                .return_html
                .clone()
                .unwrap_or_else(|| "<html><body><p>Mock Render</p></body></html>".to_string()),
            headers: self.return_headers.clone(),
        })
    }
}

#[test]
fn test_chrome_process_arguments_isolation() {
    let runner = SystemChromeRunner::new(PathBuf::from("chrome.exe"))
        .with_virtual_time_budget(2500)
        .with_capture_headers(true);

    let temp_profile = PathBuf::from(r"C:\temp\isolated_profile");
    let args = runner.build_args(&temp_profile, "https://example.com");

    assert!(args.contains(&CHROME_FLAG_HEADLESS.to_string()));
    assert!(args.contains(&CHROME_FLAG_DISABLE_GPU.to_string()));
    assert!(args.contains(&CHROME_FLAG_NO_FIRST_RUN.to_string()));
    assert!(args.contains(&CHROME_FLAG_NO_DEFAULT_BROWSER_CHECK.to_string()));
    assert!(args.contains(&CHROME_FLAG_INCOGNITO.to_string()));
    assert!(args.contains(&CHROME_FLAG_DISABLE_BACKGROUND_NETWORKING.to_string()));
    assert!(args.contains(&CHROME_FLAG_DISABLE_SYNC.to_string()));
    assert!(args.contains(&CHROME_FLAG_DISABLE_DEFAULT_APPS.to_string()));
    assert!(args.contains(&CHROME_FLAG_DISABLE_EXTENSIONS.to_string()));
    assert!(args.contains(&CHROME_FLAG_MUTE_AUDIO.to_string()));
    assert!(args.contains(&format!(
        "{}{}",
        CHROME_FLAG_USER_DATA_DIR_PREFIX,
        temp_profile.display()
    )));
    assert!(args.contains(&format!("{}2500", CHROME_FLAG_VIRTUAL_TIME_BUDGET_PREFIX)));
    assert!(args.contains(&CHROME_FLAG_DUMP_DOM.to_string()));
    assert_eq!(args.last().unwrap(), "https://example.com");
}

#[test]
fn test_chrome_ephemeral_tempdir_cleaned_on_exit() {
    let dir_path;
    {
        let temp_dir = tempfile::Builder::new()
            .prefix("fastmd_chrome_test_")
            .tempdir()
            .unwrap();
        dir_path = temp_dir.path().to_path_buf();
        assert!(dir_path.exists());
    }
    // After dropping temp_dir, it must not exist
    assert!(!dir_path.exists());
}

#[test]
fn test_mock_browser_runner_success() {
    let runner = MockBrowserRunner::new().with_html("<div>Rendered</div>");
    let doc = runner
        .render_url("https://example.com", Duration::from_secs(5))
        .unwrap();
    assert_eq!(doc.html_body, "<div>Rendered</div>");
    assert!(runner.was_called.load(Ordering::SeqCst));
}

#[test]
fn test_mock_browser_runner_timeout() {
    let runner = MockBrowserRunner::new().with_timeout();
    let err = runner
        .render_url("https://example.com", Duration::from_millis(50))
        .unwrap_err();
    assert!(matches!(err, BrowserError::Timeout(_)));
}

#[test]
fn test_mock_browser_runner_failure() {
    let runner = MockBrowserRunner::new().with_failure(1);
    let err = runner
        .render_url("https://example.com", Duration::from_secs(5))
        .unwrap_err();
    assert!(matches!(
        err,
        BrowserError::ExecutionFailed { status: 1, .. }
    ));
}
