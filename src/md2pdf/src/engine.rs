//! Typst CLI engine and PDF generation.
//!
//! Wraps the official `typst` CLI binary with a fixed template.
//! Unit tests live in the sibling `save_tests.rs` sidecar.

use std::path::{Path, PathBuf};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const TEMPLATE: &str = r##"
#set page(
  paper: "a4",
  margin: (x: 2.4cm, y: 2.6cm),
  header: align(right, text(size: 9pt, fill: luma(120))[#title]),
  numbering: "1 / 1",
)
#set text(
  size: 10pt,
  lang: "en",
  font: ("Segoe UI", "Helvetica Neue", "Liberation Sans", "Arial"),
)
#set par(justify: true, leading: 0.65em)
#show heading: set text(font: ("Segoe UI", "Helvetica Neue", "Liberation Sans", "Arial"))
#show heading.where(level: 1): set text(size: 16pt)
#show heading.where(level: 2): set text(size: 14pt)
#show heading.where(level: 3): set text(size: 12pt)
#show raw.where(block: true): block(
  fill: luma(245),
  inset: 8pt,
  radius: 4pt,
  width: 100%,
)
#show raw.where(block: false): box(
  fill: luma(245),
  inset: 2pt,
  radius: 2pt,
)
#show table.cell: cell => pad(cell, x: 4pt, y: 3pt)
#show table.cell.where(y: 0): strong

#body
"##;

/// Encode a string as a Typst string literal.
fn typst_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// Build a fully-formed Typst document by interpolating the body into the template.
fn build_typst_document(title: &str, body: &str) -> String {
    TEMPLATE
        .replace("#title", &typst_string_literal(title))
        .replace("#body", body)
}

/// Search for the official `typst` binary in system PATH.
pub fn find_typst_binary() -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        #[cfg(windows)]
        {
            for ext in &["exe", "cmd", "bat"] {
                let candidate = dir.join(format!("typst.{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        let candidate = dir.join("typst");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Check if the official `typst` binary is available in system PATH.
pub fn is_typst_available() -> bool {
    find_typst_binary().is_some()
}

/// Generate a PDF byte vector from a Typst body and title by invoking the Typst CLI.
///
/// The `typst_body` is the output of [`crate::translator::render_markdown_to_typst`],
/// already escaped and shaped as Typst markup. The `title` is interpolated
/// as the page header.
pub fn generate(title: &str, typst_body: &str) -> Result<Vec<u8>, String> {
    let typst_bin = find_typst_binary()
        .ok_or_else(|| "Typst binary ('typst') was not found in PATH".to_string())?;

    let document = build_typst_document(title, typst_body);

    use std::io::Write;
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    let mut cmd = Command::new(&typst_bin);
    cmd.arg("compile")
        .arg("-")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn typst process: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(document.as_bytes())
            .map_err(|e| format!("Failed to write to typst stdin: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait for typst process: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Typst compilation failed: {stderr}"));
    }

    if output.stdout.is_empty() {
        return Err("Typst compilation produced an empty PDF".to_string());
    }

    Ok(output.stdout)
}

/// Generate a PDF file directly at `output_path` by invoking the Typst CLI.
pub fn generate_to_file(title: &str, typst_body: &str, output_path: &Path) -> Result<(), String> {
    let typst_bin = find_typst_binary()
        .ok_or_else(|| "Typst binary ('typst') was not found in PATH".to_string())?;

    let document = build_typst_document(title, typst_body);

    use std::io::Write;
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    let mut cmd = Command::new(&typst_bin);
    cmd.arg("compile")
        .arg("-")
        .arg(output_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn typst process: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(document.as_bytes())
            .map_err(|e| format!("Failed to write to typst stdin: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait for typst process: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Typst compilation failed: {stderr}"));
    }

    Ok(())
}
