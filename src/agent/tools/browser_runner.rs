//! Headless browser runner for executing Chromium with DevTools DOM dumping and isolation.
//!
//! Unit tests live in the sibling `browser_runner_tests.rs` sidecar.

use crate::tools::registry::builtin::strings::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Errors that can occur during headless browser execution.
#[derive(Debug, thiserror::Error)]
pub enum BrowserError {
    /// The browser process failed to launch.
    #[error("failed to launch browser: {0}")]
    Launch(String),

    /// The browser execution timed out.
    #[error("[{code}] browser execution timed out after {0:?}", code = TOOL_E001_BROWSER_TIMEOUT)]
    Timeout(Duration),

    /// The browser exited with a non-zero exit code.
    #[error("browser process exited with status {status}: {stderr}")]
    ExecutionFailed {
        /// Process exit code, or -1 if terminated by signal.
        status: i32,
        /// Standard error output captured from the browser.
        stderr: String,
    },

    /// Generic I/O error during execution.
    #[error("I/O error during browser execution: {0}")]
    Io(String),

    /// Standard output was not valid UTF-8.
    #[error("browser stdout was not valid UTF-8: {0}")]
    Utf8(String),
}

/// The document output rendered by the headless browser.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedDocument {
    /// Rendered HTML body extracted from the DOM.
    pub html_body: String,
    /// HTTP response headers captured from the response.
    pub headers: HashMap<String, String>,
}

/// Trait defining the contract for rendering a URL via a headless browser.
pub trait BrowserRunner: Send + Sync {
    /// Loads the given URL in a headless browser and returns the rendered HTML document.
    fn render_url(&self, url: &str, timeout: Duration) -> Result<RenderedDocument, BrowserError>;
}

/// Production implementation of [`BrowserRunner`] utilizing headless Chromium.
#[derive(Clone, Debug)]
pub struct SystemChromeRunner {
    executable_path: PathBuf,
    virtual_time_budget_ms: u32,
    capture_headers: bool,
}

impl SystemChromeRunner {
    /// Creates a new headless Chrome runner targeting the specified binary.
    pub fn new(executable_path: PathBuf) -> Self {
        Self {
            executable_path,
            virtual_time_budget_ms: 3000,
            capture_headers: false,
        }
    }

    /// Configures the virtual time budget allocated for JavaScript evaluation (in milliseconds).
    pub fn with_virtual_time_budget(mut self, ms: u32) -> Self {
        self.virtual_time_budget_ms = ms;
        self
    }

    /// Configures whether HTTP response headers should also be captured via standard HTTP request.
    pub fn with_capture_headers(mut self, capture: bool) -> Self {
        self.capture_headers = capture;
        self
    }

    /// Builds the list of command-line arguments for headless isolated DOM extraction.
    pub fn build_args(&self, user_data_dir: &Path, url: &str) -> Vec<String> {
        vec![
            CHROME_FLAG_HEADLESS.to_string(),
            CHROME_FLAG_DISABLE_GPU.to_string(),
            CHROME_FLAG_NO_FIRST_RUN.to_string(),
            CHROME_FLAG_NO_DEFAULT_BROWSER_CHECK.to_string(),
            CHROME_FLAG_INCOGNITO.to_string(),
            CHROME_FLAG_DISABLE_BACKGROUND_NETWORKING.to_string(),
            CHROME_FLAG_DISABLE_SYNC.to_string(),
            CHROME_FLAG_DISABLE_DEFAULT_APPS.to_string(),
            CHROME_FLAG_DISABLE_EXTENSIONS.to_string(),
            CHROME_FLAG_MUTE_AUDIO.to_string(),
            format!(
                "{}{}",
                CHROME_FLAG_USER_DATA_DIR_PREFIX,
                user_data_dir.display()
            ),
            format!(
                "{}{}",
                CHROME_FLAG_VIRTUAL_TIME_BUDGET_PREFIX, self.virtual_time_budget_ms
            ),
            CHROME_FLAG_DUMP_DOM.to_string(),
            url.to_string(),
        ]
    }
}

impl BrowserRunner for SystemChromeRunner {
    fn render_url(&self, url: &str, timeout: Duration) -> Result<RenderedDocument, BrowserError> {
        let temp_dir = tempfile::Builder::new()
            .prefix("fastmd_chrome_")
            .tempdir()
            .map_err(|e| {
                BrowserError::Io(format!("failed to create temporary profile dir: {}", e))
            })?;

        let args = self.build_args(temp_dir.path(), url);

        let mut cmd = Command::new(&self.executable_path);
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| BrowserError::Launch(format!("failed to spawn browser process: {}", e)))?;

        let mut stdout_handle = child.stdout.take().ok_or_else(|| {
            BrowserError::Io("failed to capture browser stdout handle".to_string())
        })?;
        let mut stderr_handle = child.stderr.take().ok_or_else(|| {
            BrowserError::Io("failed to capture browser stderr handle".to_string())
        })?;

        let stdout_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = std::io::Read::read_to_end(&mut stdout_handle, &mut buf);
            buf
        });

        let stderr_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = std::io::Read::read_to_end(&mut stderr_handle, &mut buf);
            buf
        });

        let start = std::time::Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        let _ = stdout_thread.join();
                        let _ = stderr_thread.join();
                        return Err(BrowserError::Timeout(timeout));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_thread.join();
                    let _ = stderr_thread.join();
                    return Err(BrowserError::Io(e.to_string()));
                }
            }
        };

        let stdout_bytes = stdout_thread.join().unwrap_or_default();
        let stderr_bytes = stderr_thread.join().unwrap_or_default();

        if !status.success() {
            let stderr = String::from_utf8_lossy(&stderr_bytes).into_owned();
            return Err(BrowserError::ExecutionFailed {
                status: status.code().unwrap_or(-1),
                stderr,
            });
        }

        let html_body = String::from_utf8(stdout_bytes)
            .map_err(|e| BrowserError::Utf8(format!("invalid UTF-8 in stdout: {}", e)))?;

        let mut headers = HashMap::new();
        if self.capture_headers
            && let Ok(resp) = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .and_then(|client| client.head(url).send())
        {
            for (name, val) in resp.headers() {
                if let Ok(str_val) = val.to_str() {
                    headers.insert(name.as_str().to_string(), str_val.to_string());
                }
            }
        }

        Ok(RenderedDocument { html_body, headers })
    }
}

#[cfg(test)]
#[path = "browser_runner_tests.rs"]
pub mod tests;
