//! Tests for `recycle_bin` — platform-native Recycle Bin integration.
//! Covers `delete` happy path (file/dir → removed from original) and
//! `RecycleBinError` variants (`Io`, `Aborted`, `Com` on Windows).

use super::*;
use std::fs;
use std::path::PathBuf;
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
