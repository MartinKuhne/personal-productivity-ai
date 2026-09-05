//! Platform-native Recycle Bin integration and swappable backend.
//!
//! Provides [`delete`] which moves a file or directory to the OS
//! Recycle Bin using an injected or platform-default [`RecycleBinBackend`].
//! This replaces the external `trash` crate to avoid a `windows`-crate
//! version conflict with `wgpu`.
//!
//! Unit tests live in the sibling `recycle_bin_tests.rs` sidecar.

use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
    };
    use windows::Win32::UI::Shell::{
        FOF_ALLOWUNDO, FOF_NO_UI, FOF_WANTNUKEWARNING, FileOperation, IFileOperation, IShellItem,
        SHCreateItemFromParsingName,
    };
    use windows::core::PCWSTR;

    use super::RecycleBinError;

    /// Per-thread COM initialisation guard.
    ///
    /// `CoInitializeEx` is called lazily on first use; `CoUninitialize` runs
    /// when the thread-local is dropped.
    struct ComGuard;

    impl ComGuard {
        fn new() -> ComGuard {
            // SAFETY: COM initialisation is required before any shell call.
            // If the thread was already initialised with a compatible
            // apartment model the call is a harmless no-op (returns
            // `S_FALSE`). An incompatible model returns
            // `RPC_E_CHANGED_MODE` which we intentionally ignore — the
            // thread already has COM and we can proceed.
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            ComGuard
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            // SAFETY: Paired with the `CoInitializeEx` in `new()`.
            unsafe {
                CoUninitialize();
            }
        }
    }

    thread_local! {
        static CO_INIT: ComGuard = ComGuard::new();
    }

    /// The `\\?\` wide-char prefix that `canonicalize()` prepends on Windows.
    const VERBATIM_PREFIX: [u16; 4] = [b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];

    pub fn delete(path: &Path) -> Result<(), RecycleBinError> {
        // Ensure COM is initialised on this thread.
        CO_INIT.with(|_| {});

        let canonical = std::fs::canonicalize(path).map_err(|e| RecycleBinError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        // SAFETY: All COM calls are guarded by the `ComGuard` thread-local.
        unsafe {
            let file_op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)
                .map_err(|e| RecycleBinError::Com {
                    step: "CoCreateInstance(FileOperation)",
                    source: e,
                })?;

            file_op
                .SetOperationFlags(FOF_NO_UI | FOF_ALLOWUNDO | FOF_WANTNUKEWARNING)
                .map_err(|e| RecycleBinError::Com {
                    step: "SetOperationFlags",
                    source: e,
                })?;

            let wide: Vec<u16> = canonical
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            // Strip the `\\?\` verbatim prefix — `SHCreateItemFromParsingName`
            // does not understand it.
            let wide_slice = if wide.starts_with(&VERBATIM_PREFIX) {
                &wide[VERBATIM_PREFIX.len()..]
            } else {
                &wide[..]
            };

            let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide_slice.as_ptr()), None)
                .map_err(|e| RecycleBinError::Com {
                step: "SHCreateItemFromParsingName",
                source: e,
            })?;

            file_op
                .DeleteItem(&item, None)
                .map_err(|e| RecycleBinError::Com {
                    step: "DeleteItem",
                    source: e,
                })?;

            file_op
                .PerformOperations()
                .map_err(|e| RecycleBinError::Com {
                    step: "PerformOperations",
                    source: e,
                })?;

            if file_op
                .GetAnyOperationsAborted()
                .map_err(|e| RecycleBinError::Com {
                    step: "GetAnyOperationsAborted",
                    source: e,
                })?
                .as_bool()
            {
                return Err(RecycleBinError::Aborted);
            }

            Ok(())
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::RecycleBinError;
    use std::path::Path;

    pub fn delete(path: &Path) -> Result<(), RecycleBinError> {
        let canonical = std::fs::canonicalize(path).map_err(|e| RecycleBinError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        trash::delete(&canonical).map_err(|e| RecycleBinError::Trash { source: e })
    }
}

/// Errors produced by [`delete`] or a [`RecycleBinBackend`].
#[derive(Debug, thiserror::Error)]
pub enum RecycleBinError {
    /// An I/O error occurred while canonicalising the path.
    #[error("I/O error for {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    /// A COM / shell operation failed.
    #[cfg(windows)]
    #[error("COM step `{step}` failed: {source}")]
    Com {
        step: &'static str,
        source: windows::core::Error,
    },

    /// A trash operation failed.
    #[cfg(not(windows))]
    #[error("Trash operation failed: {source}")]
    Trash { source: trash::Error },

    /// The user (or the system) aborted the operation.
    #[error("Recycle Bin operation was aborted")]
    Aborted,
}

/// Backend trait for moving files to the Recycle Bin or deleting them.
pub trait RecycleBinBackend: Send + Sync {
    /// Moves the specified path to the Recycle Bin or deletes it.
    ///
    /// # Errors
    ///
    /// Returns a [`RecycleBinError`] if the operation fails or is aborted.
    fn delete(&self, path: &Path) -> Result<(), RecycleBinError>;
}

impl<F> RecycleBinBackend for F
where
    F: Fn(&Path) -> Result<(), RecycleBinError> + Send + Sync,
{
    fn delete(&self, path: &Path) -> Result<(), RecycleBinError> {
        self(path)
    }
}

/// Default OS-native Recycle Bin backend.
///
/// On Windows, this delegates to the `IFileOperation` COM API.
/// On other platforms, it delegates to the `trash` crate.
/// Under test environments, mutating operations on existing files are protected
/// by a runtime panic shield per `[RUST-006]` unless `FASTMD_ALLOW_LIVE_RECYCLE_BIN=1`.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeRecycleBinBackend;

impl RecycleBinBackend for NativeRecycleBinBackend {
    fn delete(&self, path: &Path) -> Result<(), RecycleBinError> {
        #[cfg(test)]
        {
            if std::env::var("FASTMD_ALLOW_LIVE_RECYCLE_BIN").is_err() && path.exists() {
                panic!(
                    "RUST-006 runtime panic shield: NativeRecycleBinBackend must not mutate OS Recycle Bin in test environments without FASTMD_ALLOW_LIVE_RECYCLE_BIN=1"
                );
            }
        }
        imp::delete(path)
    }
}

/// Isolated filesystem backend for tests and non-COM environments.
///
/// Unlinks files and directories from the filesystem directly without
/// touching the OS Recycle Bin or broadcasting shell events.
#[derive(Debug, Default)]
pub struct IsolatedRecycleBinBackend {
    deleted_paths: std::sync::Mutex<Vec<std::path::PathBuf>>,
}

impl IsolatedRecycleBinBackend {
    /// Creates a new isolated backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a copy of the canonical paths deleted through this backend.
    pub fn deleted_paths(&self) -> Vec<std::path::PathBuf> {
        self.deleted_paths
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

impl RecycleBinBackend for IsolatedRecycleBinBackend {
    fn delete(&self, path: &Path) -> Result<(), RecycleBinError> {
        let canonical = std::fs::canonicalize(path).map_err(|e| RecycleBinError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        if canonical.is_dir() {
            std::fs::remove_dir_all(&canonical).map_err(|e| RecycleBinError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
        } else {
            std::fs::remove_file(&canonical).map_err(|e| RecycleBinError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
        }

        if let Ok(mut guard) = self.deleted_paths.lock() {
            guard.push(canonical);
        }
        Ok(())
    }
}

/// Configurable mock backend for simulating errors and recording invocations.
#[derive(Default)]
pub struct MockRecycleBinBackend {
    recorded_paths: std::sync::Mutex<Vec<std::path::PathBuf>>,
    error_factory: std::sync::Mutex<Option<Box<dyn Fn() -> RecycleBinError + Send + Sync>>>,
}

impl MockRecycleBinBackend {
    /// Creates a new mock backend that records calls and succeeds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a mock backend configured to return an error from `factory`.
    pub fn with_error<E>(factory: E) -> Self
    where
        E: Fn() -> RecycleBinError + Send + Sync + 'static,
    {
        Self {
            recorded_paths: std::sync::Mutex::new(Vec::new()),
            error_factory: std::sync::Mutex::new(Some(Box::new(factory))),
        }
    }

    /// Creates a mock backend configured to simulate [`RecycleBinError::Aborted`].
    pub fn aborted() -> Self {
        Self::with_error(|| RecycleBinError::Aborted)
    }

    /// Returns the list of paths passed to [`delete`].
    pub fn recorded_paths(&self) -> Vec<std::path::PathBuf> {
        self.recorded_paths
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

impl std::fmt::Debug for MockRecycleBinBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockRecycleBinBackend")
            .field("recorded_paths", &self.recorded_paths)
            .finish()
    }
}

impl RecycleBinBackend for MockRecycleBinBackend {
    fn delete(&self, path: &Path) -> Result<(), RecycleBinError> {
        if let Ok(mut guard) = self.recorded_paths.lock() {
            guard.push(path.to_path_buf());
        }
        if let Ok(guard) = self.error_factory.lock()
            && let Some(factory) = guard.as_ref()
        {
            return Err(factory());
        }
        Ok(())
    }
}

thread_local! {
    static THREAD_BACKEND: std::cell::RefCell<Option<Arc<dyn RecycleBinBackend>>> = const { std::cell::RefCell::new(None) };
}

fn default_backend() -> Arc<dyn RecycleBinBackend> {
    #[cfg(test)]
    {
        Arc::new(IsolatedRecycleBinBackend::new())
    }
    #[cfg(not(test))]
    {
        Arc::new(NativeRecycleBinBackend)
    }
}

static GLOBAL_BACKEND: OnceLock<RwLock<Option<Arc<dyn RecycleBinBackend>>>> = OnceLock::new();

fn global_backend_cell() -> &'static RwLock<Option<Arc<dyn RecycleBinBackend>>> {
    GLOBAL_BACKEND.get_or_init(|| RwLock::new(None))
}

/// Returns a clone of the active global [`RecycleBinBackend`].
pub fn get_backend() -> Arc<dyn RecycleBinBackend> {
    if let Ok(guard) = global_backend_cell().read()
        && let Some(backend) = guard.as_ref()
    {
        return backend.clone();
    }
    default_backend()
}

/// Sets the active global [`RecycleBinBackend`].
pub fn set_backend(backend: Arc<dyn RecycleBinBackend>) {
    if let Ok(mut guard) = global_backend_cell().write() {
        *guard = Some(backend);
    }
}

/// Sets a thread-local [`RecycleBinBackend`] override.
///
/// Pass `None` to clear the thread-local override and revert to the global backend.
pub fn set_thread_backend(backend: Option<Arc<dyn RecycleBinBackend>>) {
    THREAD_BACKEND.with(|b| {
        *b.borrow_mut() = backend;
    });
}

/// Runs a closure with a temporary thread-local [`RecycleBinBackend`] override.
///
/// Reverts the thread-local backend to its previous value when the closure returns or unwinds.
pub fn with_thread_backend<F, R>(backend: Arc<dyn RecycleBinBackend>, f: F) -> R
where
    F: FnOnce() -> R,
{
    struct ScopeGuard(Option<Arc<dyn RecycleBinBackend>>);
    impl Drop for ScopeGuard {
        fn drop(&mut self) {
            THREAD_BACKEND.with(|b| {
                *b.borrow_mut() = self.0.take();
            });
        }
    }

    let prev = THREAD_BACKEND.with(|b| b.borrow().clone());
    let _guard = ScopeGuard(prev);
    THREAD_BACKEND.with(|b| {
        *b.borrow_mut() = Some(backend);
    });
    f()
}

/// Move a file or directory to the Recycle Bin using the specified backend.
///
/// # Errors
///
/// Returns an error if the backend operation fails or is aborted.
pub fn delete_with_backend(
    path: impl AsRef<Path>,
    backend: &dyn RecycleBinBackend,
) -> Result<(), RecycleBinError> {
    backend.delete(path.as_ref())
}

/// Move a file or directory to the Recycle Bin.
///
/// Uses the active thread-local backend if set, or the global backend otherwise.
///
/// # Errors
///
/// Returns an error if the backend operation fails (e.g. the path does not
/// exist, is locked, or the user lacks permission).
pub fn delete(path: impl AsRef<Path>) -> Result<(), RecycleBinError> {
    let path = path.as_ref();
    if let Some(backend) = THREAD_BACKEND.with(|b| b.borrow().clone()) {
        return backend.delete(path);
    }
    get_backend().delete(path)
}

#[cfg(test)]
#[path = "recycle_bin_tests.rs"]
mod tests;
