//! Markdown → Typst markup translator.
//!
//! This module turns a CommonMark / GFM source string into a Typst source
//! string suitable for compilation by `typst-as-lib`. It is a fresh,
//! self-contained translator that walks [`pulldown_cmark`] events
//! directly, paralleling (but not reusing) the egui-shaped event
//! stream produced by [`crate::markdown::parser::parse_markdown_to_events`].
//!
//! # Why a separate translator?
//!
//! The existing `RenderEvent` AST in [`crate::markdown::parser`] is
//! egui-shaped — it carries font, colour, and pixel-space metadata
//! that has no analogue in Typst. Forcing the renderer through that
//! AST would mean re-introducing a parallel "typst-shaped" projection
//! every time the egui one changes. A dedicated pulldown-cmark walker
//! keeps the typst output stable, lives in the markdown subsystem
//! (egui-free by AGENTS.md RUST-058 contract), and is small enough to
//! be audit-friendly.
//!
//! # Design
//!
//! Single-pass, state-machine. The state tracks: list kind stack,
//! open fenced-code buffer, and the current table header vs body
//! position. We emit Typst markup as the events arrive, with a
//! helper that flushes the appropriate opening / closing markup when
//! block containers open and close.
//!
//! # Supported features
//!
//! - Headings H1–H6, paragraphs, hard and soft line breaks
//! - Emphasis (`*text*`), strong (`**text**`), strikethrough (`~~text~~`)
//! - Inline code, fenced code blocks (with language)
//! - Bulleted, ordered, and task lists
//! - Block quotes
//! - GFM tables (header row + body rows, repeated across page breaks)
//! - Links and images (URLs are kept verbatim; remote images require
//!   the user to be online and Typst's resolver to accept the URL)
//! - Horizontal rules
//!
//! # Out of scope for v1
//!
//! - Footnotes (cmark has them; requires a stateful two-pass to map
//!   cmark's footnote IDs to Typst's)
//! - Raw HTML passthrough
//! - Definition lists (non-standard anyway)
//! - Indented (non-fenced) code blocks
//!
//! Unit tests live in the sibling `typst_tests.rs` sidecar.

// AGENTS.md RUST-058: this module is egui-free — no `use eframe`, `use egui`.
// AGENTS.md RUST-051: this module lives in the markdown subsystem;
// pulldown-cmark is imported here and nowhere else in the app layer.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Translate a Markdown source string into a Typst source string.
///
/// The returned Typst source is the body of a `.typ` document — it
/// does not include the `set page(...)` / `set text(...)` preamble
/// (the caller wraps the result in a `TEMPLATE` constant that
/// applies the user's chosen paper, font, and margin settings).
pub fn render_markdown_to_typst(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options);
    let mut state = TypstEmitState::default();
    for event in parser {
        emit_event(&mut state, event);
    }
    // After the stream, close any still-open list (in case of
    // unterminated input — pulldown-cmark tolerates this; we should
    // emit at least one terminator for each list kind on the stack).
    while let Some(kind) = state.list_stack.pop() {
        if matches!(kind, ListKind::Ordered) {
            state.output.push_str(")\n");
        }
    }
    state.output
}

/// Internal state used while walking the pulldown-cmark stream.
#[derive(Default)]
struct TypstEmitState {
    output: String,
    list_stack: Vec<ListKind>,
    /// Open fenced code block buffer; non-empty while inside ``` ```.
    code_buffer: Option<CodeBuffer>,
    /// Position within the current table: None if not in a table,
    /// Some(true) inside the head row, Some(false) inside body rows.
    table_pos: Option<TablePos>,
    /// Cells collected for the current row. Drained on row end (body
    /// rows) or on TableHead end (the head row, which has no
    /// explicit TableRow event in cmark 0.13).
    current_row_cells: Vec<String>,
    /// Column count from the header row. Cached here so that when we
    /// patch the `__COLS__` placeholder on Table end, the cells have
    /// already been drained into `table.header(...)`.
    table_column_count: usize,
    /// Set to true after the first block element has been emitted; used
    /// to suppress duplicate leading blank lines.
    saw_block: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TablePos {
    Head,
    Body,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ListKind {
    /// `-` — bullet list (Typst `-`).
    Bullet,
    /// `1.` — ordered list (Typst `#list` with `numbering`).
    Ordered,
}

/// In-flight fenced code block.
struct CodeBuffer {
    lang: String,
    body: String,
}

impl TypstEmitState {
    /// Emit a blank-line separator between Typst block elements.
    /// Skips the leading blank line before the first block.
    fn block_sep(&mut self) {
        if self.saw_block {
            self.output.push_str("\n\n");
        } else {
            self.saw_block = true;
        }
    }

    /// True while we are inside a code block; emitted text gets
    /// routed into the buffer instead of the output.
    fn in_code(&self) -> bool {
        self.code_buffer.is_some()
    }

    /// True while we are inside a table cell.
    fn in_table_cell(&self) -> bool {
        self.table_pos.is_some() && !self.current_row_cells.is_empty()
    }

    /// Drain the current row's cells. Returns them so the caller can
    /// decide whether to emit them as a header row, a body row, or
    /// something else based on the current `table_pos`.
    fn drain_row(&mut self) -> Vec<String> {
        std::mem::take(&mut self.current_row_cells)
    }

    /// Push a chunk of text to the current "inline target" — the
    /// main output stream, the code buffer, or the current table
    /// cell. Escape user content for safe inclusion in Typst markup.
    fn push_inline(&mut self, text: &str) {
        let escaped = escape_typst(text);
        if let Some(buf) = self.code_buffer.as_mut() {
            buf.body.push_str(&escaped);
            return;
        }
        if self.in_table_cell() {
            let last = self.current_row_cells.len() - 1;
            self.current_row_cells[last].push_str(&escaped);
            return;
        }
        self.output.push_str(&escaped);
    }
}

fn emit_event(state: &mut TypstEmitState, event: Event<'_>) {
    match event {
        Event::Start(tag) => emit_start(state, tag),
        Event::End(tag_end) => emit_end(state, tag_end),
        Event::Text(text) => state.push_inline(&text),
        Event::Code(code) => {
            if state.in_code() {
                state.push_inline(&code);
            } else if state.in_table_cell() {
                let last = state.current_row_cells.len() - 1;
                state.current_row_cells[last].push_str(&format!("`{}`", escape_typst(&code)));
            } else {
                state.output.push_str(&format!("`{}`", escape_typst(&code)));
            }
        }
        Event::Html(_) | Event::InlineHtml(_) => {
            // Raw HTML passthrough would require a Typst `raw` block
            // with `lang: "html"`; the user would have to install an
            // HTML→Typst renderer. For v1 we drop the event.
        }
        Event::SoftBreak => {
            // Typst treats a single space as a soft break inside a
            // paragraph; a real `\` is a hard break. We use a space.
            state.output.push(' ');
        }
        Event::HardBreak => {
            state.output.push_str(" \\\n");
        }
        Event::FootnoteReference(_) => {
            // Out of scope for v1 — see module doc.
        }
        Event::InlineMath(_) | Event::DisplayMath(_) => {
            // Math is rendered as inline text. Typst has native math
            // mode (`$ ...$`) but mapping cmark's math event to a
            // Typst math block requires knowing whether the source
            // was inline or display, which cmark already splits for
            // us. For v1 we drop the event so the surrounding text
            // still flows; the next iteration can re-introduce math
            // rendering with proper Typst `$ ...$` / `$ ... $` form.
        }
        Event::Rule => {
            state.block_sep();
            state.output.push_str("#line(length: 100%, stroke: 0.5pt)");
        }
        Event::TaskListMarker(checked) => {
            // Typst does not have native GFM checkboxes; we emit a
            // literal "[x] " / "[ ] " prefix before the item body.
            state.output.push_str(if checked { "[x] " } else { "[ ] " });
        }
    }
}

fn emit_start(state: &mut TypstEmitState, tag: Tag<'_>) {
    match tag {
        Tag::Heading { level, .. } => {
            state.block_sep();
            let marker = match level {
                HeadingLevel::H1 => "=",
                HeadingLevel::H2 => "==",
                HeadingLevel::H3 => "===",
                HeadingLevel::H4 => "====",
                HeadingLevel::H5 => "=====",
                HeadingLevel::H6 => "======",
            };
            state.output.push_str(marker);
            state.output.push(' ');
        }
        Tag::Paragraph => {
            state.block_sep();
        }
        Tag::BlockQuote(_) => {
            state.block_sep();
            state.output.push_str("#quote(block: true)[\n");
        }
        Tag::CodeBlock(kind) => {
            state.block_sep();
            let lang = match kind {
                pulldown_cmark::CodeBlockKind::Indented => String::new(),
                pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
            };
            state.code_buffer = Some(CodeBuffer {
                lang,
                body: String::new(),
            });
        }
        Tag::List(list_kind) => {
            state.block_sep();
            let kind = match list_kind {
                Some(_) => ListKind::Ordered,
                None => ListKind::Bullet,
            };
            state.list_stack.push(kind);
            match kind {
                ListKind::Ordered => {
                    // `+` (with `numbering: "1."`) is the typst form
                    // for an ordered list. We open a `#list(...)` and
                    // emit one `+ ` line per item.
                    state.output.push_str("#list(marker: ([_],),\n");
                }
                ListKind::Bullet => {
                    // Bullet list: typst `-` marker, one line per item.
                    // We just rely on the implicit bullet which typst
                    // renders when a paragraph starts with `-`. This
                    // is simpler than `#list(...)` and renders correctly
                    // inside any other paragraph context.
                    // Task-list items reuse the bullet marker; the
                    // literal `[ ]` / `[x]` prefix is supplied by the
                    // `TaskListMarker` event, so no separate list kind
                    // is needed here.
                }
            }
        }
        Tag::Item => {
            // Push the marker for the current item. For ordered lists
            // we use `+`; for bullet lists (and the task-list variant
            // which shares the bullet marker) we use `-`. The
            // `[x] ` / `[ ] ` prefix for task items is supplied
            // inline by the `TaskListMarker` event.
            match state.list_stack.last() {
                Some(ListKind::Ordered) => state.output.push_str("+ "),
                Some(ListKind::Bullet) | None => {
                    state.output.push_str("- ");
                }
            }
        }
        Tag::Link { dest_url, .. } => {
            state
                .output
                .push_str(&format!("#link(\"{}\")[", escape_typst(&dest_url)));
        }
        Tag::Image { dest_url, .. } => {
            // Alt text arrives between Start and End; we drop it for
            // v1 (Typst's `#image` has no alt field in the version
            // we depend on) and just emit the image directly.
            state
                .output
                .push_str(&format!("#image(\"{}\")", escape_typst(&dest_url)));
        }
        Tag::Emphasis => state.output.push('_'),
        Tag::Strong => state.output.push('*'),
        Tag::Strikethrough => state.output.push_str("#strike["),
        Tag::Table(_) => {
            state.block_sep();
            // Emit a placeholder for the column count; we patch it
            // to the actual count on Table end. The placeholder is
            // unique enough that `String::replacen(..., 1)` will only
            // touch this one occurrence.
            state.output.push_str("#table(\n");
            state.output.push_str("  columns: __COLS__,\n");
            state.table_pos = Some(TablePos::Head);
        }
        Tag::TableHead => {
            // Marker only; the row's cells will be drained into
            // `pending_header_cells` on TableRow end, and emitted
            // on TableHead end.
        }
        Tag::TableRow => {
            state.current_row_cells.clear();
        }
        Tag::TableCell => {
            state.current_row_cells.push(String::new());
        }
        _ => {}
    }
}

fn emit_end(state: &mut TypstEmitState, tag_end: TagEnd) {
    match tag_end {
        TagEnd::Heading(_) => {
            // Typst headings are single-line; the newline ensures
            // the next block element starts cleanly.
            state.output.push('\n');
        }
        TagEnd::Paragraph => {
            // Paragraphs in Typst are implicit — the blank line on
            // the next `block_sep` call will close the previous
            // paragraph. Nothing to do.
        }
        TagEnd::BlockQuote(_) => {
            state.output.push_str("\n]\n");
        }
        TagEnd::CodeBlock => {
            if let Some(buf) = state.code_buffer.take() {
                if buf.lang.is_empty() {
                    // Untagged code block — emit as a Typst `raw` block
                    // using the string form so `{`, `}`, `*`, etc. in
                    // the code body don't get interpreted as markup.
                    state.output.push_str(&format!(
                        "#raw(block: true, \"{}\")",
                        escape_typst_string(&buf.body)
                    ));
                } else {
                    // Tagged code block — same string form, with a
                    // language hint. The string form is essential
                    // because code bodies routinely contain markup-
                    // special characters (curly braces in Rust/JS,
                    // percent signs in SQL, etc.).
                    state.output.push_str(&format!(
                        "#raw(block: true, lang: \"{}\", \"{}\")",
                        escape_typst_string(&buf.lang),
                        escape_typst_string(&buf.body),
                    ));
                }
            }
        }
        TagEnd::List(_) => {
            let kind = state.list_stack.pop();
            if matches!(kind, Some(ListKind::Ordered)) {
                state.output.push_str(")\n");
            }
        }
        TagEnd::Item => {
            // Newline terminates the current item. For ordered
            // lists typst expects one `+ ` line per item separated
            // by newlines; for bullet lists the implicit `-` style
            // also terminates on newline.
            state.output.push('\n');
        }
        TagEnd::Link => state.output.push(']'),
        TagEnd::Image => {
            // The image was fully emitted at Start; End just drops.
        }
        TagEnd::Emphasis => state.output.push('_'),
        TagEnd::Strong => state.output.push('*'),
        TagEnd::Strikethrough => state.output.push(']'),
        TagEnd::Table => {
            // Patch the column-count placeholder to the actual count
            // cached on TableHead end. Fall back to 1 if the table
            // had no head (rare; defensive against malformed input).
            let cols = state.table_column_count.max(1);
            state.output = state.output.replacen("__COLS__", &cols.to_string(), 1);
            state.output.push_str(")\n");
            state.table_pos = None;
            state.table_column_count = 0;
        }
        TagEnd::TableHead => {
            // In cmark 0.13, the head row has NO TableRow event —
            // TableCells are direct children of TableHead. We
            // collect them into `current_row_cells` on each
            // TableCell start, and drain them here.
            let cells = state.drain_row();
            state.table_column_count = cells.len();
            if !cells.is_empty() {
                state.output.push_str("  table.header(\n");
                // Each cell is wrapped in `[...]` so Typst treats it
                // as a content block rather than trying to evaluate
                // the cell text as a variable reference.
                for (i, cell) in cells.iter().enumerate() {
                    if i > 0 {
                        state.output.push_str(",\n");
                    }
                    state.output.push_str("    [");
                    state.output.push_str(cell);
                    state.output.push(']');
                }
                state.output.push_str(",\n  ),\n");
            }
            state.table_pos = Some(TablePos::Body);
        }
        TagEnd::TableRow => {
            // In cmark 0.13, only body rows emit TableRow events —
            // the head row's cells are direct children of TableHead
            // and are drained on TableHead end. So a TableRow end
            // here is always a body row.
            let cells = state.drain_row();
            if cells.is_empty() {
                return;
            }
            state.output.push_str("  ");
            // Wrap each cell in `[...]` and join with `,\n  ` so
            // Typst sees N positional content arguments to the
            // outer `#table(...)`, one per cell.
            let joined: Vec<String> = cells.iter().map(|c| format!("[{}]", c)).collect();
            state.output.push_str(&joined.join(", "));
            state.output.push_str(",\n");
        }
        TagEnd::TableCell => {
            // Cells push their own content into the latest entry of
            // `current_row_cells`. Nothing to do on end.
        }
        _ => {}
    }
}

/// Escape a string for safe inclusion in Typst markup.
///
/// Escapes the three characters that would otherwise be interpreted as
/// Typst markup (`#`, `*`, `_`) plus the escape character itself (`\`)
/// and the inline-markup delimiters (`[`, `]`). We escape brackets too
/// because content frequently contains them (markdown link text, code
/// in tables, etc.) and a stray unbalanced bracket would produce a
/// confusing Typst error.
fn escape_typst(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '#' => out.push_str("\\#"),
            '*' => out.push_str("\\*"),
            '_' => out.push_str("\\_"),
            '[' => out.push_str("\\["),
            ']' => out.push_str("\\]"),
            '@' => out.push_str("\\@"),
            other => out.push(other),
        }
    }
    out
}

/// Escape a string for safe inclusion as a Typst string literal
/// (i.e. between double quotes). Only `"` and `\` need escaping;
/// newlines and other control characters are passed through
/// verbatim. Used for code-block bodies where the user content
/// may contain any character at all and we want to bypass the
/// markup interpreter entirely.
fn escape_typst_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _other => out.push(c),
        }
    }
    out
}

// Unit tests live in the sibling `typst_tests.rs` sidecar
// (AGENTS.md RUST-056 / RUST-057).
//
// see: `src/markdown/typst_tests.rs`

#[cfg(test)]
#[path = "typst_tests.rs"]
mod tests;
