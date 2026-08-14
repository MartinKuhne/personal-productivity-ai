//! Platform-native Recycle Bin integration.
//!
//! Provides [`delete`] which moves a file or directory to the OS
//! Recycle Bin (Windows) using the `IFileOperation` COM API.
//! This replaces the external `trash` crate to avoid a `windows`-crate
//! version conflict with `wgpu`.

use std::path::Path;

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

/// Move a file or directory to the Recycle Bin.
///
/// This is the drop-in replacement for `trash::delete`.
///
/// # Errors
///
/// Returns an error if the COM operation fails (e.g. the path does not
/// exist, is locked, or the user lacks permission).
pub fn delete(path: impl AsRef<Path>) -> Result<(), RecycleBinError> {
    imp::delete(path.as_ref())
}

/// Errors produced by [`delete`].
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
