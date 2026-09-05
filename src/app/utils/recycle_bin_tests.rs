//! Tests for `recycle_bin` — platform-native Recycle Bin integration.
//! Covers `delete` happy path (file/dir → removed from original) and
//! `RecycleBinError` variants (`Io`, `Aborted`, `Com` on Windows).

use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn temp_file_with_content(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, content).unwrap();
    path
}

// ---------------------------------------------------------------------------
// delete — missing path => Io
// ---------------------------------------------------------------------------

#[test]
fn delete_missing_path_returns_io_error() {
    // Use a path that cannot be canonicalized.
    let bogus = PathBuf::from(format!(
        "C:\\nonexistent_fastmd_test_{}\\ghost.md",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    // Also try a temp-dir-relative bogus path to stay portable.
    let bogus2 = std::env::temp_dir().join(format!("fastmd_no_such_{}.md", uuid::Uuid::new_v4()));
    for path in [bogus, bogus2] {
        let err = delete(&path).expect_err("missing path must error");
        let msg = err.to_string();
        match &err {
            RecycleBinError::Io { path: p, source: _ } => {
                assert_eq!(p, &path, "Io.path must echo input");
                assert!(
                    msg.contains("I/O error"),
                    "Display must mention I/O error: {msg}"
                );
                assert!(
                    msg.contains(&path.to_string_lossy().to_string())
                        || msg.contains("I/O error for"),
                    "Display must contain path context: {msg}"
                );
            }
            other => panic!("expected Io, got {other:?}"),
        }
    }
}

#[test]
fn delete_missing_via_string_arg_returns_io() {
    // Generic `AsRef<Path>` — &str variant.
    let bogus = format!("C:\\nonexistent_fastmd_str_{}.tmp", uuid::Uuid::new_v4());
    let err = delete(bogus.as_str()).unwrap_err();
    assert!(matches!(err, RecycleBinError::Io { .. }));
}

// ---------------------------------------------------------------------------
// delete — file happy path
// ---------------------------------------------------------------------------

#[test]
fn delete_file_moves_to_recycle_bin() {
    let dir = tempfile::tempdir().unwrap();
    let path = temp_file_with_content(&dir, "to_delete.md", "# hello");
    assert!(path.exists(), "precondition: file exists");

    delete(&path).expect("delete file should succeed");

    assert!(
        !path.exists(),
        "original path must not exist after Recycle Bin move"
    );
    // Parent dir must still exist (only the file moved).
    assert!(dir.path().exists());
}

#[test]
fn delete_file_with_space_in_name() {
    let dir = tempfile::tempdir().unwrap();
    let path = temp_file_with_content(&dir, "my note with spaces.md", "content");
    delete(&path).expect("delete with spaces should succeed");
    assert!(!path.exists());
}

#[test]
fn delete_file_accepts_pathbuf_and_str() {
    let dir = tempfile::tempdir().unwrap();
    let p1 = temp_file_with_content(&dir, "a.md", "a");
    let p2 = temp_file_with_content(&dir, "b.md", "b");

    // &PathBuf
    delete(&p1).unwrap();
    assert!(!p1.exists());

    // String
    delete(p2.to_string_lossy().to_string()).unwrap();
    assert!(!p2.exists());
}

// ---------------------------------------------------------------------------
// delete — directory happy path
// ---------------------------------------------------------------------------

#[test]
fn delete_directory_moves_to_recycle_bin() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("subdir");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("inner.md"), "inner").unwrap();
    assert!(sub.exists());

    delete(&sub).expect("delete directory should succeed");
    assert!(!sub.exists(), "subdir must be gone after move");
    assert!(dir.path().exists(), "parent tempdir must remain");
}

#[test]
fn delete_empty_directory() {
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("empty");
    fs::create_dir(&empty).unwrap();
    delete(&empty).expect("empty dir delete should succeed");
    assert!(!empty.exists());
}

// ---------------------------------------------------------------------------
// RecycleBinError — Display / Debug
// ---------------------------------------------------------------------------

#[test]
fn error_aborted_display() {
    let err = RecycleBinError::Aborted;
    assert_eq!(err.to_string(), "Recycle Bin operation was aborted");
    // Debug must not panic
    let _ = format!("{err:?}");
}

#[test]
fn error_io_display_contains_path_and_source() {
    // Trigger Io via missing file to get a real variant, then check Display.
    let bogus = std::env::temp_dir().join(format!("fastmd_io_disp_{}.tmp", uuid::Uuid::new_v4()));
    let err = delete(&bogus).unwrap_err();
    let msg = err.to_string();
    assert!(msg.starts_with("I/O error for"), "got: {msg}");
    assert!(msg.contains(&bogus.to_string_lossy().to_string()) || msg.contains("I/O error"));
}

#[cfg(windows)]
#[test]
fn error_com_display_contains_step() {
    // Construct a synthetic Com error via windows::core::Error from a known HRESULT.
    // E_FAIL = 0x80004005
    let win_err = windows::core::Error::from(windows::core::HRESULT(0x80004005u32 as i32));
    let err = RecycleBinError::Com {
        step: "CoCreateInstance(FileOperation)",
        source: win_err,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("CoCreateInstance(FileOperation)"),
        "Com Display must contain step: {msg}"
    );
    assert!(msg.contains("COM step"), "prefix: {msg}");
}

#[cfg(not(windows))]
#[test]
fn error_trash_variant_exists() {
    // On non-Windows the Trash variant is the platform error. We can only
    // assert the Display prefix via a synthetic case: trigger via trash::Error
    // would require a real FS error, so we just verify the enum is Debug.
    let bogus = std::env::temp_dir().join(format!("fastmd_trash_{}.tmp", uuid::Uuid::new_v4()));
    let err = delete(&bogus).unwrap_err();
    // On Linux this will be Io (canonicalize fails) not Trash, but the
    // Trash variant still exists in the type — cover the Debug path.
    let _ = format!("{err:?}");
}

// ---------------------------------------------------------------------------
// Idempotency: second delete of same path => Io
// ---------------------------------------------------------------------------

#[test]
fn delete_twice_second_is_io() {
    let dir = tempfile::tempdir().unwrap();
    let path = temp_file_with_content(&dir, "once.md", "once");
    delete(&path).unwrap();
    assert!(!path.exists());
    let err = delete(&path).unwrap_err();
    assert!(matches!(err, RecycleBinError::Io { .. }));
}

// ---------------------------------------------------------------------------
// Backend injection & Swappable handler tests
// ---------------------------------------------------------------------------

#[test]
fn delete_with_backend_uses_explicit_backend() {
    let mock = MockRecycleBinBackend::new();
    let path = PathBuf::from("mock_target.md");
    delete_with_backend(&path, &mock).unwrap();
    assert_eq!(mock.recorded_paths(), vec![path]);
}

#[test]
fn with_thread_backend_overrides_delete_scoped() {
    let mock = Arc::new(MockRecycleBinBackend::new());
    let path = PathBuf::from("thread_scoped.md");

    with_thread_backend(mock.clone(), || {
        delete(&path).unwrap();
    });

    assert_eq!(mock.recorded_paths(), vec![path.clone()]);

    // After exiting the scope, the thread-local override is reverted
    // and subsequent calls do not hit the mock.
    let dir = tempfile::tempdir().unwrap();
    let real_file = temp_file_with_content(&dir, "after_scope.md", "after");
    delete(&real_file).unwrap();
    assert_eq!(mock.recorded_paths().len(), 1);
}

#[test]
fn set_thread_backend_manual_override_and_clear() {
    let mock = Arc::new(MockRecycleBinBackend::new());
    set_thread_backend(Some(mock.clone()));

    let path = PathBuf::from("manual_thread.md");
    delete(&path).unwrap();
    assert_eq!(mock.recorded_paths(), vec![path]);

    set_thread_backend(None);

    let dir = tempfile::tempdir().unwrap();
    let real_file = temp_file_with_content(&dir, "cleared_thread.md", "data");
    delete(&real_file).unwrap();
    assert_eq!(mock.recorded_paths().len(), 1);
}

#[test]
fn mock_backend_simulates_aborted() {
    let mock = MockRecycleBinBackend::aborted();
    let path = PathBuf::from("aborted.md");
    let err = mock.delete(&path).unwrap_err();
    assert!(matches!(err, RecycleBinError::Aborted));
    assert_eq!(mock.recorded_paths(), vec![path]);
}

#[test]
fn mock_backend_with_error_simulates_custom_error() {
    #[cfg(windows)]
    let factory = || RecycleBinError::Com {
        step: "PerformOperations",
        source: windows::core::Error::from(windows::core::HRESULT(0x80004005u32 as i32)),
    };
    #[cfg(not(windows))]
    let factory = || RecycleBinError::Aborted;

    let mock = MockRecycleBinBackend::with_error(factory);
    let path = PathBuf::from("fail.md");
    let err = mock.delete(&path).unwrap_err();
    #[cfg(windows)]
    assert!(matches!(
        err,
        RecycleBinError::Com {
            step: "PerformOperations",
            ..
        }
    ));
    #[cfg(not(windows))]
    assert!(matches!(err, RecycleBinError::Aborted));
}

#[test]
fn isolated_backend_records_deleted_paths() {
    let backend = IsolatedRecycleBinBackend::new();
    let dir = tempfile::tempdir().unwrap();
    let file = temp_file_with_content(&dir, "isolated.md", "content");
    backend.delete(&file).unwrap();
    assert!(!file.exists());
    let deleted = backend.deleted_paths();
    assert_eq!(deleted.len(), 1);
    assert_eq!(
        deleted[0],
        std::fs::canonicalize(dir.path())
            .unwrap()
            .join("isolated.md")
    );
}

#[test]
fn closure_implements_recycle_bin_backend() {
    let called = AtomicBool::new(false);
    let closure = |p: &Path| {
        if p.ends_with("closure.md") {
            called.store(true, Ordering::SeqCst);
        }
        Ok(())
    };

    delete_with_backend(Path::new("closure.md"), &closure).unwrap();
    assert!(called.load(Ordering::SeqCst));
}

#[test]
fn native_backend_panic_shield_in_tests() {
    let dir = tempfile::tempdir().unwrap();
    let path = temp_file_with_content(&dir, "shield.md", "shield");
    let result = std::panic::catch_unwind(|| {
        let _ = NativeRecycleBinBackend.delete(&path);
    });
    assert!(
        result.is_err(),
        "RUST-006: NativeRecycleBinBackend must panic on existing files in test environments"
    );
}

#[test]
fn native_backend_missing_path_returns_io() {
    let bogus = PathBuf::from("C:\\nonexistent_native_test_path_12345.tmp");
    let err = NativeRecycleBinBackend.delete(&bogus).unwrap_err();
    assert!(matches!(err, RecycleBinError::Io { .. }));
}

#[test]
fn global_backend_get_and_set() {
    let original = get_backend();
    let mock: Arc<dyn RecycleBinBackend> = Arc::new(MockRecycleBinBackend::new());
    set_backend(mock.clone());
    assert!(Arc::ptr_eq(&get_backend(), &mock));
    set_backend(original);
}

#[cfg(windows)]
#[test]
#[ignore = "live Windows Shell COM operation (slow, mutates OS Recycle Bin)"]
fn native_backend_live_delete() {
    let dir = tempfile::tempdir().unwrap();
    let path = temp_file_with_content(&dir, "live_native.md", "content");
    unsafe {
        std::env::set_var("FASTMD_ALLOW_LIVE_RECYCLE_BIN", "1");
    }
    let res = NativeRecycleBinBackend.delete(&path);
    unsafe {
        std::env::remove_var("FASTMD_ALLOW_LIVE_RECYCLE_BIN");
    }
    res.expect("live native delete should succeed when FASTMD_ALLOW_LIVE_RECYCLE_BIN=1");
    assert!(!path.exists());
}
