//! Path helpers — safe-basename validation, Windows `PATH` executable resolution,
//! PDF-backing detection for Markdown files, and new-document naming/writing.

use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};

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

/// Append `.md` to a user-entered document name when it has no
/// extension, so the created file is always a markdown document.
///
/// Relocated from the deleted `ui/modals.rs` by [TASK-029b]; the
/// create-document command path is its only consumer.
#[must_use]
pub fn unique_document_name(entered: &str) -> String {
    if Path::new(entered).extension().is_some() {
        entered.to_owned()
    } else {
        format!("{}.md", entered)
    }
}

/// Write a new markdown document into `parent` using the
/// user-entered name (`.md` appended when the name has no extension).
/// If a file with the preferred name already exists, a
/// `<stem> <date-time><ext>` name is generated instead so the created
/// file is always unique. Returns the created file's path.
///
/// Relocated from the deleted `ui/modals.rs` by [TASK-029b].
pub fn write_new_document(parent: &Path, entered: &str) -> std::io::Result<PathBuf> {
    let file_name = unique_document_name(entered);
    let mut new_path = parent.join(&file_name);
    if new_path.exists() {
        new_path = parent.join(date_suffixed_name(&file_name));
    }
    std::fs::write(&new_path, "")?;
    Ok(new_path)
}

/// Build `<stem> <timestamp><ext>` for a file name, used when the
/// preferred document name already exists.
///
/// Relocated from the deleted `ui/modals.rs` by [TASK-029b].
#[must_use]
pub fn date_suffixed_name(file_name: &str) -> String {
    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();
    let now = chrono::Local::now();
    let date_str = now.format("%Y-%m-%d %H-%M-%S");
    format!("{} {}{}", stem, date_str, ext)
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
