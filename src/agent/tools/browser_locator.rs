//! Multi-platform discovery of installed Chromium-based browser binaries for headless execution.
//!
//! Unit tests live in the sibling `browser_locator_tests.rs` sidecar.

use std::path::{Path, PathBuf};

/// The branding or distribution kind of a detected Chromium browser.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum BrowserKind {
    /// Official Google Chrome browser.
    GoogleChrome,
    /// Open-source Chromium browser.
    Chromium,
    /// Microsoft Edge browser (Chromium-based).
    MicrosoftEdge,
    /// Brave browser.
    Brave,
    /// Custom browser binary supplied via user configuration.
    Custom,
}

impl std::fmt::Display for BrowserKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GoogleChrome => write!(f, "Google Chrome"),
            Self::Chromium => write!(f, "Chromium"),
            Self::MicrosoftEdge => write!(f, "Microsoft Edge"),
            Self::Brave => write!(f, "Brave"),
            Self::Custom => write!(f, "Custom Browser"),
        }
    }
}

/// Information describing a verified browser executable on the host system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserInstallation {
    /// The detected browser distribution kind.
    pub kind: BrowserKind,
    /// The absolute filesystem path to the browser binary.
    pub executable_path: PathBuf,
}

/// Trait defining the contract for discovering an installed browser executable.
pub trait BrowserLocator: Send + Sync {
    /// Discovers the highest priority installed browser executable, returning None if absent.
    fn locate(&self) -> Option<BrowserInstallation>;
}

/// Standard production implementation that inspects well-known OS directories and the system PATH.
#[derive(Clone, Debug, Default)]
pub struct SystemBrowserLocator {
    custom_path: Option<PathBuf>,
}

impl SystemBrowserLocator {
    /// Creates a new system locator with an optional user-defined custom executable path.
    pub fn new(custom_path: Option<PathBuf>) -> Self {
        Self { custom_path }
    }

    /// Discovers installed browsers using an injected existence check for testability.
    pub fn locate_with_checker<F>(&self, exists_fn: F) -> Option<BrowserInstallation>
    where
        F: Fn(&Path) -> bool,
    {
        // 1. Custom path takes absolute precedence if configured and present.
        if let Some(ref path) = self.custom_path
            && exists_fn(path)
        {
            return Some(BrowserInstallation {
                kind: BrowserKind::Custom,
                executable_path: path.clone(),
            });
        }

        // 2. Inspect platform-specific well-known paths in priority order.
        let candidates = Self::standard_candidates();
        for (kind, candidate_path) in candidates {
            if exists_fn(&candidate_path) {
                return Some(BrowserInstallation {
                    kind,
                    executable_path: candidate_path,
                });
            }
        }

        // 3. Inspect system PATH for common binary names.
        let path_names = Self::path_binary_names();
        for (kind, name) in path_names {
            if let Some(found_path) = Self::find_on_path_with(name, &exists_fn) {
                return Some(BrowserInstallation {
                    kind,
                    executable_path: found_path,
                });
            }
        }

        None
    }

    /// Returns candidate paths for the current operating system.
    pub fn standard_candidates() -> Vec<(BrowserKind, PathBuf)> {
        let mut list = Vec::new();

        #[cfg(target_os = "windows")]
        {
            // Windows Program Files & LocalAppData
            let prog_files = std::env::var_os("ProgramFiles")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
            let prog_files_x86 = std::env::var_os("ProgramFiles(x86)")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"));
            let local_app_data = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\Users\Default\AppData\Local"));

            // Google Chrome
            list.push((
                BrowserKind::GoogleChrome,
                prog_files.join(r"Google\Chrome\Application\chrome.exe"),
            ));
            list.push((
                BrowserKind::GoogleChrome,
                prog_files_x86.join(r"Google\Chrome\Application\chrome.exe"),
            ));
            list.push((
                BrowserKind::GoogleChrome,
                local_app_data.join(r"Google\Chrome\Application\chrome.exe"),
            ));

            // Microsoft Edge
            list.push((
                BrowserKind::MicrosoftEdge,
                prog_files.join(r"Microsoft\Edge\Application\msedge.exe"),
            ));
            list.push((
                BrowserKind::MicrosoftEdge,
                prog_files_x86.join(r"Microsoft\Edge\Application\msedge.exe"),
            ));

            // Brave
            list.push((
                BrowserKind::Brave,
                prog_files.join(r"BraveSoftware\Brave-Browser\Application\brave.exe"),
            ));
        }

        #[cfg(target_os = "macos")]
        {
            list.push((
                BrowserKind::GoogleChrome,
                PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
            ));
            list.push((
                BrowserKind::Chromium,
                PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"),
            ));
            list.push((
                BrowserKind::MicrosoftEdge,
                PathBuf::from("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
            ));
            list.push((
                BrowserKind::Brave,
                PathBuf::from("/Applications/Brave Browser.app/Contents/MacOS/Brave Browser"),
            ));
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            list.push((
                BrowserKind::GoogleChrome,
                PathBuf::from("/usr/bin/google-chrome"),
            ));
            list.push((
                BrowserKind::GoogleChrome,
                PathBuf::from("/usr/bin/google-chrome-stable"),
            ));
            list.push((BrowserKind::Chromium, PathBuf::from("/usr/bin/chromium")));
            list.push((
                BrowserKind::Chromium,
                PathBuf::from("/usr/bin/chromium-browser"),
            ));
            list.push((
                BrowserKind::MicrosoftEdge,
                PathBuf::from("/usr/bin/microsoft-edge"),
            ));
            list.push((BrowserKind::Brave, PathBuf::from("/usr/bin/brave-browser")));
        }

        list
    }

    /// Returns the binary names to look for along PATH.
    pub fn path_binary_names() -> Vec<(BrowserKind, &'static str)> {
        #[cfg(target_os = "windows")]
        {
            vec![
                (BrowserKind::GoogleChrome, "chrome.exe"),
                (BrowserKind::MicrosoftEdge, "msedge.exe"),
                (BrowserKind::Brave, "brave.exe"),
                (BrowserKind::Chromium, "chromium.exe"),
            ]
        }

        #[cfg(not(target_os = "windows"))]
        {
            vec![
                (BrowserKind::GoogleChrome, "google-chrome"),
                (BrowserKind::GoogleChrome, "google-chrome-stable"),
                (BrowserKind::Chromium, "chromium"),
                (BrowserKind::Chromium, "chromium-browser"),
                (BrowserKind::MicrosoftEdge, "microsoft-edge"),
                (BrowserKind::Brave, "brave-browser"),
            ]
        }
    }

    /// Searches the system PATH environment variable for an executable using the provided checker.
    pub fn find_on_path_with<F>(name: &str, exists_fn: &F) -> Option<PathBuf>
    where
        F: Fn(&Path) -> bool,
    {
        let path_var = std::env::var_os("PATH")?;
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if exists_fn(&candidate) {
                return Some(candidate);
            }
        }
        None
    }
}

impl BrowserLocator for SystemBrowserLocator {
    fn locate(&self) -> Option<BrowserInstallation> {
        self.locate_with_checker(|p| p.is_file())
    }
}

#[cfg(test)]
#[path = "browser_locator_tests.rs"]
mod tests;
