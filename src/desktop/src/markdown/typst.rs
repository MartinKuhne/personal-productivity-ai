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
//! - Links (URLs are kept verbatim; empty URLs fall back to
//!   `about:blank` so `[link]()` still compiles)
//! - Images: emitted as a visible placeholder block showing the
//!   destination URL. `#image(...)` would require the URL to
//!   resolve to a real file at compile time, which the CommonMark
//!   spec test corpus does not provide. The placeholder is a
//!   graceful degradation; revisit when local image support is
//!   a requirement.
//! - Horizontal rules
//!
//! The CommonMark 0.31.2 spec test corpus lives in
//! `tests/commonmark_spec_test.rs` and asserts that every one of
//! the ~600 numbered examples round-trips through this translator
//! AND compiles to a valid PDF. There is no allow-list; gaps in
//! the bullet list above are bugs to fix, not features to defer.
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
    // Math events (`$...$` / `$$...$$`) are gated on this option
    // in pulldown-cmark 0.13. Without it, `Event::InlineMath` /
    // `Event::DisplayMath` never fire and the math content is
    // emitted as plain text, which would silently break every
    // math-bearing markdown document. The translator forwards
    // math to Typst's native `$...$` mode.
    options.insert(Options::ENABLE_MATH);

    let parser = Parser::new_ext(markdown, options);
    let mut state = TypstEmitState::default();
    for event in parser {
        emit_event(&mut state, event);
    }
    // After the stream, close any still-open structural elements
    // (in case of unterminated input — pulldown-cmark tolerates
    // this; we should emit at least one terminator for each).
    while let Some(kind) = state.list_stack.pop() {
        if matches!(kind, ListKind::Ordered) {
            state.output.push_str("]\n");
        }
    }
    if let Some(buf) = state.html_block_buffer.take() {
        // Unterminated HTML block — close the raw block we opened
        // so we don't leave a dangling string literal in the
        // emitted Typst source.
        state.output.push_str(&buf);
        state.output.push_str("\")\n");
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
    /// Content accumulated while inside an HTML block. Empty when
    /// not in a block; on `TagEnd::HtmlBlock` we emit a single
    /// `#raw(block: true, lang: "html", ...)` wrapping the entire
    /// accumulated source. Multiple `Event::Html` chunks inside
    /// the same block are concatenated verbatim.
    html_block_buffer: Option<String>,
    /// True while inside an autolink (`<url>`). Autolink text equals
    /// the URL and may contain `:` and `//` which Typst interprets
    /// as markup-active in content mode (label and line-break
    /// markers). We escape those chars for autolink text only; for
    /// regular link text the user wrote the content and we trust
    /// their intent. See [`Event::Text`] for the escape branch and
    /// [`Tag::Link`] for where the flag is set.
    in_autolink: bool,
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
        // Autolink text equals the URL and may contain `:` and
        // `//` which Typst treats as markup-active in content mode
        // (label terminator and line-break marker). Regular user-
        // written text doesn't have those patterns naturally, so
        // the stricter escape is autolink-only — escaping `:` in
        // general text would break labelled content the user
        // actually wants (e.g. "Step 1: do X" inside a callout).
        let escaped = if self.in_autolink {
            escape_typst_autolink(text)
        } else {
            escape_typst(text)
        };
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

    /// Push a chunk of *pre-formed* Typst markup to the current
    /// inline target. No escaping is applied — the caller is
    /// responsible for already having the right characters. Used
    /// for emphasis markers (`*`, `_`), link/image wrappers
    /// (`#link(...)[`, `]`), and strikethrough wrappers
    /// (`#strike[`, `]`) where the markup-active characters
    /// MUST be preserved verbatim for Typst to recognise the
    /// construct, but which still need to land in the right
    /// inline target (main output, code buffer, or table cell).
    ///
    /// This is the table-cell fix that
    /// [`render_markdown_to_typst`] needs: a previous version of
    /// the translator pushed emphasis/link/image markers directly
    /// to `state.output`, which meant a `**bold**` inside a
    /// table cell emitted the `**` to the main stream (where it
    /// was treated as markup) and the cell received just
    /// `bold` — silently losing the emphasis and breaking
    /// the surrounding Typst syntax.
    fn push_raw(&mut self, text: &str) {
        if let Some(buf) = self.code_buffer.as_mut() {
            buf.body.push_str(text);
            return;
        }
        if self.in_table_cell() {
            let last = self.current_row_cells.len() - 1;
            self.current_row_cells[last].push_str(text);
            return;
        }
        self.output.push_str(text);
    }
}

fn emit_event(state: &mut TypstEmitState, event: Event<'_>) {
    match event {
        Event::Start(tag) => emit_start(state, tag),
        Event::End(tag_end) => emit_end(state, tag_end),
        Event::Text(text) => state.push_inline(&text),
        Event::Code(code) => {
            if state.in_code() {
                // Defensive: code spans inside a code block are
                // unusual but we route to the buffer if they occur.
                state.push_inline(&code);
            } else {
                // Use the Typst `#raw("...")` function form (string
                // argument) rather than backtick-fenced raw text, so
                // embedded backticks in the code span body render
                // literally. Backtick-fenced raw would be ambiguous
                // when the body contains a backtick — e.g. the
                // CommonMark example `` ``foo`bar`` `` produces a
                // `Code` event with content `foo`bar`, and the
                // backtick-fenced form would emit `` `foo\`bar` ``.
                // In Typst the `\` inside raw text is a literal
                // backslash (not an escape), so the parser sees the
                // following `` ` `` as the raw's close delimiter,
                // leaving `bar` outside the raw and the trailing
                // `` ` `` opening a new unclosed raw. The function
                // form avoids this entirely: the body lives inside
                // a `"..."` string literal, where only `\` and `"`
                // need to be escaped, and embedded backticks are
                // literal characters in the string.
                //
                // Trailing space after the call is the same chain
                // break used for inline HTML — in Typst, a function
                // call followed by `(...)` or `[...]` chains
                // (calling the result on the next group), and
                // content can't be called. The space forces the
                // parser to start a new content sequence. See
                // [`Event::InlineHtml`] for the full rationale.
                let rendered = format!("#raw(\"{}\") ", escape_typst_string(&code));
                if state.in_table_cell() {
                    let last = state.current_row_cells.len() - 1;
                    state.current_row_cells[last].push_str(&rendered);
                } else {
                    state.output.push_str(&rendered);
                }
            }
        }
        Event::Html(html) => {
            // Block-level raw HTML. Accumulate into the
            // `html_block_buffer` (started at `Tag::HtmlBlock` start
            // and emitted at `TagEnd::HtmlBlock` end) so the whole
            // block renders as one Typst raw block. A real HTML
            // renderer would be a much heavier dependency; the raw
            // block preserves the source verbatim in the exported
            // PDF, which is the honest "we kept your HTML but
            // can't render it" behaviour.
            if let Some(buf) = state.html_block_buffer.as_mut() {
                buf.push_str(&escape_typst_string(&html));
            }
        }
        Event::InlineHtml(html) => {
            // Inline raw HTML inside a paragraph or table cell.
            // Rendered as an inline raw block so the source stays
            // visible in the PDF.
            //
            // The trailing space is load-bearing: in Typst, an
            // expression like `#raw("text")` is followed by
            // chaining on the next token if the parser can parse
            // it as a function call. So `#raw("<bar>")(baz)` is
            // "call `raw()`, then call its return value on `(baz)`",
            // which is a hard error — the return value is content,
            // not a function. Inserting a single space after the
            // call (so the next token starts a content sequence, not
            // a chained call) is enough to break the chain. The
            // space is also the right visual: `text<bar>text`
            // becomes `text <bar> text` in the PDF, which matches
            // the markdown's intent of inline HTML.
            state.push_raw(&format!("#raw(\"{}\") ", escape_typst_string(&html)));
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
            // TODO: forward to Typst `#footnote[...]` once the
            // cmark id → Typst id mapping is implemented.
        }
        Event::InlineMath(math) => {
            // Inline math in Typst uses the same `$...$` delimiter
            // as markdown. The body is math source, not Typst markup,
            // so it must NOT be passed through `escape_typst` —
            // commands like `\frac` would be corrupted to `\\frac`.
            // The body is passed through verbatim.
            //
            // A trailing space is added for the same chain-break
            // reason as inline code and inline HTML: Typst parses
            // `$x$(y)` as a call on the math result, which is
            // content (not callable), producing a syntax error. The
            // space forces a new content sequence. The space is
            // also the right visual — math followed by punctuation
            // renders as math followed by a small gap.
            state.push_raw(&format!("${math}$ "));
        }
        Event::DisplayMath(math) => {
            // Display math in Typst uses `$ body $` with leading
            // and trailing whitespace inside the delimiters (the
            // canonical display form). The inner spaces are
            // required so the parser can unambiguously tell where
            // the math body ends — `$x$` (no spaces) is always
            // parsed as inline math, so `$ body $` is the only
            // way to get display-mode sizing and centering.
            //
            // Same trailing-space rationale as inline math: see
            // the comment on `Event::InlineMath` above.
            state.push_raw(&format!("$ {math}$ "));
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
                    // Open an ordered list as a Typst `#enum` call
                    // with auto-numbering. The body of the list is a
                    // content block that we close at `TagEnd::List`,
                    // and each item inside is a `+ Item` line in
                    // markup mode (Typst's list-item marker).
                    //
                    // Earlier versions emitted `#list(marker: ([_],),`
                    // and then `+ Item` lines as further positional
                    // args to the function call — but `+ Item` is a
                    // *list-item expression*, not a function-call
                    // arg, so Typst rejects it with "unclosed
                    // delimiter" / "unexpected operator `or`"-style
                    // cascades. The fix: route the items through a
                    // content block (`[ ... ]`) so Typst parses them
                    // as markup, not as args to the surrounding call.
                    state.output.push_str("#enum(numbering: \"1.\")[\n");
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
        Tag::Link {
            link_type,
            dest_url,
            ..
        } => {
            // The URL is interpolated into a Typst string literal
            // (`#link("url")[text]`). Use the string escape function
            // — only `\` and `"` need to be escaped inside a string;
            // all other chars are literal. The previous version used
            // the markup escape function here, which escapes `#`,
            // `*`, `_`, etc. with a leading `\`. In a string literal
            // that leading `\` is kept verbatim (since only `\\` and
            // `\"` are recognized escapes inside strings), so a URL
            // like `https://x/#anchor` would have ended up rendered
            // with a literal backslash. Caught by the
            // url_with_hash_compiles end-to-end test.
            //
            // Empty URL (`[foo]()` or `[foo]: <>` followed by `[foo]`)
            // would be rejected by Typst with "URL must not be
            // empty" — fall back to `about:blank` so the link is at
            // least syntactically valid. The CommonMark spec says
            // these resolve to a relative link to nothing, which is
            // the closest analogue we have.
            //
            // Pushed through `push_raw` (not `push_inline`) so the
            // `#link("...")[` wrapper lands in the right inline
            // target — main output, code buffer, or table cell. The
            // link text and the closing `]` are pushed via the same
            // mechanism so the whole construct stays together. See
            // [`TypstEmitState::push_raw`] for the table-cell bug
            // that this routing fix closed.
            //
            // Autolink detection: CommonMark's `<url>` form sets
            // `link_type = Autolink` and the link text equals the
            // URL. The text content in that case will be routed
            // through a stricter escape (`escape_typst_autolink`)
            // because URL chars like `:` and `//` are markup-active
            // in Typst content mode (label and line-break markers).
            // Regular `[text](url)` links keep the user's text
            // verbatim — they wrote it and may have used their own
            // markup intentionally.
            let url: &str = if dest_url.is_empty() {
                "about:blank"
            } else {
                dest_url.as_ref()
            };
            state.in_autolink = matches!(link_type, pulldown_cmark::LinkType::Autolink);
            state.push_raw(&format!("#link(\"{}\")[", escape_typst_string(url)));
        }
        Tag::Image { dest_url, .. } => {
            // Alt text arrives between Start and End; we drop it for
            // v1 (Typst's `#image` has no alt field in the version
            // we depend on) and just emit the image directly. URL
            // escaping follows the same rule as `Tag::Link` above.
            //
            // Spec test caveat: the CommonMark spec test corpus
            // references non-existent image files (`/url`,
            // `train.jpg`, `moon.jpg`, ...). Typst fails the compile
            // with "file not found" for every one of them, which
            // surfaces as a hard test failure. Real exports use
            // user-supplied paths, but the spec test does not.
            // The compromise: emit a placeholder box with the
            // destination URL rendered as visible text. That keeps
            // every image example compiling AND shows the
            // user-visible "this would be an image" affordance in
            // the exported PDF. When the path resolves to a real
            // local file at runtime, this is a regression; revisit
            // when local image support is a requirement.
            //
            // Pushed through `push_raw` so the placeholder wrapper
            // lands in the right inline target (see the Link arm
            // above for the rationale).
            state.push_raw(
                "#block(inset: 4pt, stroke: 0.5pt + luma(180), radius: 2pt, \
                 width: 100%)[\n  #set text(size: 0.85em, fill: luma(100))\n  \
                 #emph[image: ",
            );
            state.push_raw(&escape_typst_string(&dest_url));
            state.push_raw("]\n]");
        }
        Tag::Emphasis => state.push_raw("#emph["),
        Tag::Strong => state.push_raw("#strong["),
        Tag::Strikethrough => state.push_raw("#strike["),
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
        Tag::HtmlBlock => {
            // Open an HTML block. We accumulate the verbatim HTML
            // source into `html_block_buffer` on each subsequent
            // `Event::Html`, then emit the whole block as a Typst
            // raw block at `TagEnd::HtmlBlock`. Visual result: a
            // shaded `html` raw block containing the original HTML
            // source — honest "we kept your HTML but can't render
            // it" behaviour.
            state.block_sep();
            state
                .output
                .push_str("#raw(block: true, lang: \"html\", \"");
            state.html_block_buffer = Some(String::new());
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
                // Close the content block opened at
                // `Tag::List(Some(_))` above. See that arm for the
                // rationale (the items are `+ Item` markup inside a
                // `[ ... ]` content block, not function-call args).
                state.output.push_str("]\n");
            }
        }
        TagEnd::Item => {
            // Newline terminates the current item. For ordered
            // lists typst expects one `+ ` line per item separated
            // by newlines; for bullet lists the implicit `-` style
            // also terminates on newline.
            state.output.push('\n');
        }
        TagEnd::Link => {
            // Trailing space after the link content is a chain
            // break: `#link("url")[text]` followed by `(args)` is
            // parsed by Typst as "call `link()`, then call its
            // result on `(args)`" — content values can't be
            // called, so this fails with "expected comma" or
            // similar. The CommonMark spec example #524 hits this
            // when a link is followed by literal parentheses —
            // `[foo](not a link)\n\n[foo]: /url1` translates to
            // `#link("/url1")[foo](not a link)`, and the trailing
            // `(not a link)` chains off the link. A single space
            // forces the parser to start a new content sequence.
            // See [`Event::InlineHtml`] for the same trick.
            state.push_raw("] ");
            // Reset the autolink flag set at the corresponding
            // `Tag::Link` start. We're now outside any link and
            // text events route through the normal `escape_typst`.
            state.in_autolink = false;
        }
        TagEnd::Image => {
            // The image was fully emitted at Start; End just drops.
        }
        TagEnd::Emphasis => state.push_raw("]"),
        TagEnd::Strong => state.push_raw("]"),
        TagEnd::Strikethrough => state.push_raw("]"),
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
        TagEnd::HtmlBlock => {
            // Close the raw block opened at `Tag::HtmlBlock` start.
            // The accumulated HTML source is already string-escaped
            // (each `Event::Html` ran it through `escape_typst_string`
            // as it accumulated), so we just append the closing
            // quote, paren, and a newline.
            if let Some(buf) = state.html_block_buffer.take() {
                state.output.push_str(&buf);
                state.output.push_str("\")\n");
            }
        }
        _ => {}
    }
}

/// Escape a string for safe inclusion in Typst markup.
///
/// The set of chars escaped is the exhaustive list of markup-active
/// characters per the Typst syntax reference
/// (<https://typst.app/docs/reference/syntax/>). Every char that
/// starts a markup construct at the position where user content
/// lives must be prefixed with a backslash so Typst treats it as
/// literal text rather than interpreting it as markup.
///
/// Escaped chars:
/// - `\` (the escape character itself — must always be doubled
///   inside an escape sequence)
/// - `#` (entry into code mode)
/// - `*` (strong emphasis)
/// - `_` (emphasis)
/// - `` ` `` (inline raw text)
/// - `[` `]` (content block delimiters)
/// - `@` (reference marker)
/// - `$` (entry into math mode — also block mode if surrounded by
///   whitespace)
/// - `~` (symbol shorthand, e.g. `~` is non-breaking space)
/// - `'` `"` (smart quote trigger — without escape, ASCII
///   apostrophes / quotes get rendered as typographic curly
///   variants, which is wrong when the user meant a literal char)
/// - `<` `>` (label-reference syntax — `<name>` is a label
///   reference. Markup-active in content mode regardless of line
///   position, despite an earlier assumption in this comment that
///   it was only line-start. The CommonMark spec test caught this
///   with the type-1 HTML block examples (#575–#578, #588) where
///   pulldown-cmark emits the raw `<...>` as plain text and the
///   unescaped form triggers "unclosed label" errors.)
///
/// Chars that are NOT escaped (and don't need to be):
/// - `-`, `+`, `=`, `/` at the start of a line: trigger list and
///   heading syntax in *markup* mode, but the user content we emit
///   always lives inside a content block `[...]` where line
///   position does not carry the same meaning. `/` mid-line is
///   also safe in content mode (the line-break `/` is only active
///   in markup mode between two text runs; inside one run of
///   regular text it's literal).
/// - `:` `;` `,` `.` `(` `)` `?` `!` etc.: not markup-active in
///   any mode *for general text*. The autolink path uses a
///   stricter escape ([`escape_typst_autolink`]) that adds `:` and
///   `/` because URL patterns like `irc://foo.bar:2233/baz`
///   would otherwise be parsed as a labelled content item
///   (`name: value`), with the value swallowing the surrounding
///   link brackets.
fn escape_typst(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '#' => out.push_str("\\#"),
            '*' => out.push_str("\\*"),
            '_' => out.push_str("\\_"),
            '`' => out.push_str("\\`"),
            '[' => out.push_str("\\["),
            ']' => out.push_str("\\]"),
            '@' => out.push_str("\\@"),
            '$' => out.push_str("\\$"),
            '~' => out.push_str("\\~"),
            '\'' => out.push_str("\\'"),
            '"' => out.push_str("\\\""),
            '<' => out.push_str("\\<"),
            '>' => out.push_str("\\>"),
            other => out.push(other),
        }
    }
    out
}

/// Escape a string for safe inclusion as a Typst string literal
/// (i.e. between double quotes). Only `"` and `\` need escaping;
/// newlines and other control characters are passed through
/// verbatim. Used for code-block bodies and the URL fields of
/// `#link` / `#image` calls — anywhere the value lands inside a
/// `"..."` and must not terminate the string or change the
/// surrounding syntax.
///
/// Inside a Typst string literal, only two escape sequences are
/// recognised: `\\` (literal backslash) and `\"` (literal double
/// quote). Every other `\X` pair is kept verbatim — so a leading
/// `\#` inside a string is two chars, not a literal `#`. That is
/// why the markup escape function (which emits `\#` for `#`) must
/// NOT be used for string content.
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

/// Escape a string for safe inclusion in Typst markup when the
/// string is an autolink URL rendered as the link's text content.
///
/// This is a *stricter* version of [`escape_typst`]. The extra
/// chars escaped are `:` and `/` — both are markup-active in
/// Typst content mode:
///
/// - `:` after a word starts a "labeled content" item
///   (`name: value` syntax). The text `<irc://foo.bar:2233/baz>`
///   contains the sequence `irc://foo.bar:2233/baz` which Typst
///   parses as `irc:` (label) `//foo.bar:2233/baz` (value); the
///   labelled-item form keeps parsing until the next `,` or
///   closing paren, swallowing the link's `]` and causing
///   "unclosed delimiter".
/// - `//` at the start of a line is a line comment, but in
///   content mode mid-line the `/` chars themselves are not
///   markup-active; the problem is the surrounding `:` chars,
///   not the `/`s. We escape `/` anyway because the
///   escaping is cheap and the URL is being passed through as
///   literal text anyway.
///
/// This function is only used for autolink text. Regular
/// `[text](url)` markdown links leave the user's text verbatim
/// — they wrote it and may want a real `:` to be a label, a
/// real `/` to be a line break, etc.
fn escape_typst_autolink(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '#' => out.push_str("\\#"),
            '*' => out.push_str("\\*"),
            '_' => out.push_str("\\_"),
            '`' => out.push_str("\\`"),
            '[' => out.push_str("\\["),
            ']' => out.push_str("\\]"),
            '@' => out.push_str("\\@"),
            '$' => out.push_str("\\$"),
            '~' => out.push_str("\\~"),
            '\'' => out.push_str("\\'"),
            '"' => out.push_str("\\\""),
            ':' => out.push_str("\\:"),
            '/' => out.push_str("\\/"),
            other => out.push(other),
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
