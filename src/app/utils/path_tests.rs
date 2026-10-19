//! Unit tests for `utils/path.rs` (extracted to a sidecar per [RUST-056],
//! [TASK-029b]).

use super::*;
use std::fs;

#[test]
fn test_is_safe_basename_accepts_normal_names() {
    assert!(is_safe_basename("file.md"));
    assert!(is_safe_basename("a"));
    assert!(is_safe_basename("with spaces.txt"));
    assert!(is_safe_basename("file.name.with.dots"));
}

#[test]
fn test_is_safe_basename_rejects_empty() {
    assert!(!is_safe_basename(""));
}

#[test]
fn test_is_safe_basename_rejects_separators() {
    assert!(!is_safe_basename("a/b"));
    assert!(!is_safe_basename(r"a\b"));
    assert!(!is_safe_basename("/etc/passwd"));
    assert!(!is_safe_basename(r"\windows\system32"));
}

#[test]
fn test_is_safe_basename_rejects_traversal() {
    assert!(!is_safe_basename(".."));
    assert!(!is_safe_basename("../etc"));
    assert!(!is_safe_basename("a/../b"));
}

#[test]
fn test_resolve_executable_path_passthrough_on_any_platform() {
    // Absolute paths, names with separators, and names with an
    // extension all pass through unchanged on every platform.
    for cmd in [
        "cmd.exe",
        r"C:\Windows\System32\cmd.exe",
        "tools/mcp",
        "foo bar",
    ] {
        let resolved = resolve_executable_path(cmd);
        assert_eq!(resolved.as_ref(), cmd);
    }
}

#[cfg(windows)]
#[test]
fn test_resolve_windows_finds_cmd_in_temp_dir() {
    let dir = tempfile::tempdir().unwrap();
    let tool = dir.path().join("hello.cmd");
    std::fs::write(&tool, "@echo off\r\n").unwrap();

    let dirs = vec![dir.path().to_path_buf()];
    let extensions = vec![".EXE".to_string(), ".CMD".to_string()];
    let resolved = resolve_in_dirs("hello", &dirs, &extensions).expect("hello.cmd should resolve");
    assert!(
        resolved.eq_ignore_ascii_case(tool.to_string_lossy().as_ref()),
        "resolved: {resolved}"
    );
}

#[cfg(windows)]
#[test]
fn test_resolve_windows_prefers_pathext_order() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("app.exe"), "MZ").unwrap();
    std::fs::write(dir.path().join("app.cmd"), "@echo off\r\n").unwrap();

    let dirs = vec![dir.path().to_path_buf()];

    let exe_first = resolve_in_dirs("app", &dirs, &[".EXE".to_string(), ".CMD".to_string()])
        .expect("app.exe should be found first");
    assert!(exe_first.to_ascii_lowercase().ends_with("app.exe"));

    let cmd_first = resolve_in_dirs("app", &dirs, &[".CMD".to_string(), ".EXE".to_string()])
        .expect("app.cmd should be found first");
    assert!(cmd_first.to_ascii_lowercase().ends_with("app.cmd"));
}

#[cfg(windows)]
#[test]
fn test_resolve_windows_missing_command_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let dirs = vec![dir.path().to_path_buf()];
    assert!(resolve_in_dirs("no_such_tool_xyz", &dirs, &[".EXE".to_string()]).is_none());
}

#[cfg(windows)]
#[test]
fn test_resolve_windows_unchanged_when_not_found() {
    let unique = format!(
        "fastmd_no_such_binary_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let resolved = resolve_executable_path(&unique);
    assert_eq!(resolved.as_ref(), unique.as_str());
}

// ---- has_pdf_backing ----

#[test]
fn test_has_pdf_backing_returns_true_when_pdf_sibling_exists() {
    let dir = tempfile::tempdir().unwrap();
    let md_path = dir.path().join("doc.md");
    let pdf_path = dir.path().join("doc.pdf");
    std::fs::write(&md_path, "# Hello").unwrap();
    std::fs::write(&pdf_path, "%PDF-1.4").unwrap();
    assert!(has_pdf_backing(&md_path));
}

#[test]
fn test_has_pdf_backing_returns_false_when_no_pdf_sibling() {
    let dir = tempfile::tempdir().unwrap();
    let md_path = dir.path().join("doc.md");
    std::fs::write(&md_path, "# Hello").unwrap();
    assert!(!has_pdf_backing(&md_path));
}

#[test]
fn test_has_pdf_backing_returns_false_for_non_md_file() {
    let dir = tempfile::tempdir().unwrap();
    let pdf_path = dir.path().join("doc.pdf");
    std::fs::write(&pdf_path, "%PDF-1.4").unwrap();
    assert!(!has_pdf_backing(&pdf_path));
}

#[test]
fn test_has_pdf_backing_returns_false_for_nonexistent_file() {
    let dir = tempfile::tempdir().unwrap();
    let md_path = dir.path().join("nonexistent.md");
    assert!(!has_pdf_backing(&md_path));
}

#[test]
fn test_has_pdf_backing_is_case_insensitive() {
    let dir = tempfile::tempdir().unwrap();
    let md_path = dir.path().join("doc.MD");
    let pdf_path = dir.path().join("doc.pdf");
    std::fs::write(&md_path, "# Hello").unwrap();
    std::fs::write(&pdf_path, "%PDF-1.4").unwrap();
    assert!(has_pdf_backing(&md_path));
}

#[cfg(windows)]
#[test]
fn test_resolve_windows_cmd_resolves_to_absolute_exe() {
    // `cmd` is resolvable through PATH on every normal Windows
    // install (System32 is on PATH). If for some reason it is not,
    // resolution degrades to passthrough, which is still correct.
    let resolved = resolve_executable_path("cmd");
    if resolved.as_ref() != "cmd" {
        assert!(Path::new(resolved.as_ref()).is_absolute());
        assert!(resolved.to_ascii_lowercase().ends_with("cmd.exe"));
    }
}

// ---- document naming helpers (ported from the deleted `modals_tests.rs`) ----

#[test]
fn unique_document_name_appends_md_only_when_no_extension() {
    assert_eq!(unique_document_name("my notes"), "my notes.md");
    assert_eq!(unique_document_name("report.md"), "report.md");
    assert_eq!(unique_document_name("draft.txt"), "draft.txt");
}

#[test]
fn date_suffixed_name_preserves_stem_and_extension() {
    let name = date_suffixed_name("report.md");
    assert!(name.starts_with("report "), "unexpected: {}", name);
    assert!(name.ends_with(".md"), "unexpected: {}", name);
    let stem = name.trim_end_matches(".md");
    let date_part = stem.trim_start_matches("report ").trim();
    // `%Y-%m-%d %H-%M-%S` → e.g. `2026-08-01 14-30-00`.
    assert!(
        date_part.len() == 19 && date_part.as_bytes()[10] == b' ',
        "unexpected timestamp: {:?}",
        date_part
    );

    let no_ext = date_suffixed_name("report");
    assert!(no_ext.starts_with("report "), "unexpected: {}", no_ext);
    assert!(!no_ext.ends_with(".md"), "unexpected: {}", no_ext);
}

#[test]
fn write_new_document_creates_empty_markdown() {
    let temp_dir = tempfile::tempdir().unwrap();

    let path = write_new_document(temp_dir.path(), "first notes").unwrap();
    assert_eq!(path, temp_dir.path().join("first notes.md"));
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, "");
}

#[test]
fn write_new_document_avoids_existing_name_with_date_suffix() {
    let temp_dir = tempfile::tempdir().unwrap();
    let _ = fs::write(temp_dir.path().join("dup.md"), "existing");

    let path = write_new_document(temp_dir.path(), "dup").unwrap();
    assert_ne!(path, temp_dir.path().join("dup.md"));
    assert!(path.exists(), "unique file must be created");
    // The unique name must keep the `.md` extension.
    assert!(
        path.extension().and_then(|e| e.to_str()) == Some("md"),
        "unique name must retain .md extension: {:?}",
        path
    );
}
