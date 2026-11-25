//! Pre-save Markdown validation for the inline editor (UI-043).
//!
//! UI-043 requires the editor to validate the body "by parsing it with the
//! same GFM parser used for rendering (pulldown-cmark with `ENABLE_TABLES`,
//! `ENABLE_FOOTNOTES`, `ENABLE_STRIKETHROUGH`, `ENABLE_TASKLISTS`)" and, on
//! failure, to abort the save and show the parse-error location.
//!
//! # What "parsing fails" can actually mean
//!
//! [`pulldown_cmark::Parser`] is a **total** function: `new_ext` returns an
//! iterator, never a `Result`, and every malformed construct degrades to
//! literal text rather than an error. Verified against the 0.13 event stream:
//!
//! | Input | What the parser does |
//! | --- | --- |
//! | `text[^missing]` (no definition) | emits literal `Text("[")`, `Text("^missing")`, `Text("]")` — **no** `FootnoteReference`, no error |
//! | short table row (`\| 1 \|` under a 2-column header) | **pads** the row with an empty `TableCell` |
//! | long table row (extra cell) | **drops** the extra cell |
//! | unterminated ` ``` ` fence | auto-closes the block at EOF and swallows the rest of the document as code |
//!
//! So UI-043's failure branch cannot be read literally. It is realised here as
//! the **structural defects the parser tolerates silently** that (a) are real
//! authoring errors and (b) have a determinate location. [`validate_markdown`]
//! still drives the parser with the renderer's exact option set
//! ([`super::parser::gfm_options`]) — that is the step UI-043 names — and then
//! applies the checks below.
//!
//! # Checks
//!
//! * [`MarkdownErrorKind::UnterminatedCodeFence`] — a ` ``` ` / `~~~` fence
//!   that is opened and never closed. This is the one tolerated defect that
//!   visibly destroys the rest of a document (everything after the stray
//!   fence silently becomes a code block), and its location is the opening
//!   fence itself.
//!
//! Follow-up candidates (each needs its own location story before it is
//! honest to block a save on it): dangling footnote references, table
//! column-count mismatches, unbalanced emphasis.
//!
//! Unit tests live in the sibling `validation_tests.rs` sidecar.

use pulldown_cmark::Parser;

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;

/// Minimum fence width that opens a fenced code block (CommonMark: three or
/// more fence characters).
const MIN_FENCE_WIDTH: usize = 3;

/// Maximum indentation (in spaces) a fence line may carry and still be a
/// fence. Four or more spaces makes the line indented code instead.
const MAX_FENCE_INDENT: usize = 3;

/// The class of Markdown defect [`validate_markdown`] found.
///
/// Carries no message text: user-facing wording belongs in
/// [`crate::ui::strings`], so the view maps this kind onto a
/// label and this type stays a machine-readable fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownErrorKind {
    /// A fenced code block (```` ``` ```` or `~~~`) is opened and never
    /// closed, so everything after the opening fence renders as code.
    UnterminatedCodeFence,
}

/// A located Markdown validation failure (UI-043's "parse error location").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkdownParseError {
    /// Which defect was found.
    pub kind: MarkdownErrorKind,
    /// Byte offset of the defect in the validated body.
    pub byte_offset: usize,
    /// 1-based line number of the defect.
    pub line: usize,
    /// 1-based column (in characters) of the defect.
    pub column: usize,
}

/// Validates a Markdown body before a save (UI-043).
///
/// `body` is the editor's text with the YAML front matter already stripped
/// (UI-037) — the front matter is not markdown and must not be parsed as
/// such.
///
/// Returns `Ok(())` when the body parses cleanly under the renderer's GFM
/// option set, or the first [`MarkdownParseError`] found. The caller aborts
/// the save and surfaces the error's `line`/`column` (UI-043).
pub fn validate_markdown(body: &str) -> Result<(), MarkdownParseError> {
    // Step 1 — parse with the renderer's exact GFM option set. The parse
    // itself cannot fail (see the module docs); driving the iterator is what
    // guarantees the body goes through the same parser as the renderer, and
    // keeps this honest should a future pulldown-cmark gain a fallible API.
    Parser::new_ext(body, crate::markdown::parser::gfm_options()).for_each(drop);

    // Step 2 — the structural checks the parser tolerates silently.
    match unterminated_code_fence(body) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Finds a fenced code block that is opened and never closed.
///
/// Reports the position of the **opening** fence, because that is the line
/// the user has to fix. Fence rules follow CommonMark closely enough for
/// authoring-error detection:
///
/// * a fence is a run of [`MIN_FENCE_WIDTH`] or more `` ` `` or `~`,
/// * at most [`MAX_FENCE_INDENT`] leading spaces (more is indented code),
/// * a closing fence must use the same character, be at least as long as the
///   opener, and carry nothing after it but whitespace — so an info string
///   (```` ```rust ````) opens but never closes,
/// * a fence shorter than the opener cannot close it — it is content.
fn unterminated_code_fence(body: &str) -> Option<MarkdownParseError> {
    let mut open: Option<OpenFence> = None;
    let mut byte_offset = 0usize;

    for (index, raw_line) in body.split_inclusive('\n').enumerate() {
        let line = strip_line_ending(raw_line);
        let indent = leading_spaces(line);
        let rest = &line[indent..];

        match open {
            None => {
                if indent <= MAX_FENCE_INDENT
                    && let Some((fence_char, fence_len)) = fence_run(rest)
                {
                    open = Some(OpenFence {
                        fence_char,
                        fence_len,
                        byte_offset: byte_offset + indent,
                        line: index + 1,
                        column: indent + 1,
                    });
                }
            }
            Some(fence) if closes_fence(rest, fence) => open = None,
            Some(_) => {}
        }

        byte_offset += raw_line.len();
    }

    open.map(|fence| MarkdownParseError {
        kind: MarkdownErrorKind::UnterminatedCodeFence,
        byte_offset: fence.byte_offset,
        line: fence.line,
        column: fence.column,
    })
}

/// An open fence awaiting its closer.
#[derive(Clone, Copy, Debug)]
struct OpenFence {
    /// The fence character (`` ` `` or `~`).
    fence_char: char,
    /// How many fence characters opened the block.
    fence_len: usize,
    /// Byte offset of the first fence character.
    byte_offset: usize,
    /// 1-based line of the opening fence.
    line: usize,
    /// 1-based column of the opening fence.
    column: usize,
}

/// Drops the trailing `\n` (and a preceding `\r`) from a line.
fn strip_line_ending(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

/// Counts the leading ASCII spaces on a line.
fn leading_spaces(line: &str) -> usize {
    line.bytes().take_while(|byte| *byte == b' ').count()
}

/// The fence run opening a line, as `(fence_char, run_length)`.
///
/// `None` when the line does not start with at least [`MIN_FENCE_WIDTH`]
/// identical backticks or tildes.
fn fence_run(line: &str) -> Option<(char, usize)> {
    let fence_char = line.chars().next()?;
    if fence_char != '`' && fence_char != '~' {
        return None;
    }

    let fence_len = line.chars().take_while(|&c| c == fence_char).count();
    (fence_len >= MIN_FENCE_WIDTH).then_some((fence_char, fence_len))
}

/// Whether `line` closes `fence`.
///
/// The closer must use the same character, be at least as long as the opener,
/// and be followed by nothing but whitespace.
fn closes_fence(line: &str, fence: OpenFence) -> bool {
    match fence_run(line) {
        // Backticks and tildes are ASCII, so the char count is a byte index.
        Some((fence_char, run_len))
            if fence_char == fence.fence_char && run_len >= fence.fence_len =>
        {
            line[run_len..].trim().is_empty()
        }
        _ => false,
    }
}
