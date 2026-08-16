//! Path helpers — safe-basename validation, Windows `PATH` executable resolution, and
//! PDF-backing detection for Markdown files.

use std::borrow::Cow;
use std::path::{Component, Path};

/// Purpose: Validate that a user-supplied name is a single safe path segment (no traversal, no separators).
/// Inputs: `name` (the trimmed candidate basename).
/// Outputs: `true` when `name` is non-empty, contains no `..` component, and contains no `/` or `\\` separator.
/// Purity: Pure function.
/// Preconditions: Caller should trim whitespace before calling.
/// Postconditions: Returns `false` for any string that would, if joined onto a parent directory, escape that directory.
pub fn is_safe_basename(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.contains('/') || name.contains('\\') {
        return false;
    }
    if Path::new(name)
        .components()
        .any(|c| c == Component::ParentDir)
    {
        return false;
    }
    true
}

/// Returns `true` if the given path is a Markdown file with a same-stem
/// `.pdf` sibling in the same directory.
///
/// This is used to mark Markdown files that are auto-generated from a PDF
/// source (see REQ-450 — PDF derivation). PDF-backed files are rendered
/// with a sepia tint and write operations are blocked (except
/// the `write_yaml_header` tool, which is exempt).
///
/// Returns `false` for non-`.md` files, missing files, and when no
/// `.pdf` sibling exists.
///
/// # Examples
/// ```
/// use std::path::Path;
/// use std::fs;
/// use tempfile::tempdir;
///
/// let dir = tempdir().unwrap();
/// let md = dir.path().join("doc.md");
/// let pdf = dir.path().join("doc.pdf");
///
/// // No PDF sibling → false
/// fs::write(&md, "# Hello").unwrap();
/// assert!(!fastmd::utils::path::has_pdf_backing(&md));
///
/// // PDF sibling exists → true
/// fs::write(&pdf, "%PDF-1.4").unwrap();
/// assert!(fastmd::utils::path::has_pdf_backing(&md));
/// ```
pub fn has_pdf_backing(path: &Path) -> bool {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
    {
        let pdf_path = path.with_extension("pdf");
        pdf_path.exists()
    } else {
        false
    }
}

/// Default `PATHEXT` used when the environment variable is unset.
/// Mirrors the value baked into `cmd.exe`.
#[cfg(windows)]
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// Resolve a command name to a path that [`std::process::Command`] can
/// actually spawn.
///
/// On Windows, [`std::process::Command`] spawns via `CreateProcessW`,
/// which only finds `.exe` (or extension-less) files while searching
/// `PATH`. It cannot find the `.cmd` / `.bat` shims that toolchains
/// such as npm place on `PATH` (e.g. `npx` → `npx.cmd`), so a bare
/// `Command::new("npx")` fails with "program not found". This helper
/// reproduces `cmd.exe`'s resolution: it scans every `PATH` directory
/// for the command with each extension from `PATHEXT` (defaulting to
/// `.COM;.EXE;.BAT;.CMD`) and returns the first match as an absolute
/// path.
///
/// Commands that already carry a path separator or a file extension are
/// passed through unchanged, as are commands that resolve to nothing
/// (the caller then surfaces the original spawn error). On non-Windows
/// platforms the input is returned unchanged.
///
/// # Example
/// ```
/// let resolved = fastmd::utils::path::resolve_executable_path("npx");
/// assert!(resolved.contains("npx"));
/// ```
pub fn resolve_executable_path(command: &str) -> Cow<'_, str> {
    #[cfg(windows)]
    {
        resolve_windows_executable(command)
    }
    #[cfg(not(windows))]
    Cow::Borrowed(command)
}

#[cfg(windows)]
fn resolve_windows_executable(command: &str) -> Cow<'_, str> {
    let has_separator = command.contains('\\') || command.contains('/');
    let has_extension = Path::new(command).extension().is_some();
    if has_separator || has_extension {
        return Cow::Borrowed(command);
    }

    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| DEFAULT_PATHEXT.to_string());
    let extensions: Vec<String> = pathext
        .split(';')
        .map(|e| e.trim().to_ascii_uppercase())
        .filter(|e| !e.is_empty())
        .collect();
    let dirs: Vec<std::path::PathBuf> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();

    match resolve_in_dirs(command, &dirs, &extensions) {
        Some(path) => Cow::Owned(path),
        None => Cow::Borrowed(command),
    }
}

#[cfg(windows)]
fn resolve_in_dirs(
    command: &str,
    dirs: &[std::path::PathBuf],
    extensions: &[String],
) -> Option<String> {
    for dir in dirs {
        for ext in extensions {
            let candidate = dir.join(format!("{command}{ext}"));
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let resolved =
            resolve_in_dirs("hello", &dirs, &extensions).expect("hello.cmd should resolve");
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
}
