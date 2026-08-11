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
    // Footnotes (`[^label]` references + `[^label]: body`
    // definitions) are gated on this option. The translator
    // does a two-pass walk: first pass collects all definition
    // bodies keyed by label, second pass emits the body at each
    // reference site. Both passes need the option enabled or
    // the events would never fire.
    options.insert(Options::ENABLE_FOOTNOTES);

    // Collect the full event stream up front. The cmark spec
    // says footnote definitions and references may occur in any
    // order — a reference can appear before its definition in
    // the source. To handle this, we need to see the definition
    // body before emitting at a reference site, which requires
    // either buffering or a two-pass walk. Buffering into a
    // `Vec` is the simpler of the two.
    let events: Vec<Event<'static>> = Parser::new_ext(markdown, options)
        .map(|e| e.into_static())
        .collect();

    // First pass: walk the stream, collect each
    // `Tag::FootnoteDefinition` body's events and translate them
    // into Typst markup using a fresh state. The bodies are
    // stored in a `label -> body_typst` map for the second pass.
    let footnote_bodies = collect_footnote_bodies(&events);

    // Second pass: walk the stream again, skipping footnote
    // definition bodies (they were translated in the first pass)
    // and emitting `#footnote[body]` at every `Event::FootnoteReference`
    // site using the bodies map built in the first pass.
    let mut state = TypstEmitState::default();
    translate_event_stream(&mut state, &events, &footnote_bodies);
    close_state(&mut state);
    state.output
}

/// Close any still-open structural elements in `state`. Used as
/// the post-pass cleanup in both the main translation and the
/// inner footnote-body translation. Without this, unterminated
/// input (which pulldown-cmark tolerates) would leave dangling
/// `]`s in the emitted Typst source.
fn close_state(state: &mut TypstEmitState) {
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
}

/// Walk a pre-collected event stream and emit Typst markup.
/// Skips footnote definition bodies (their inner events were
/// already translated in the first pass and stored in
/// `footnote_bodies`); for footnote references, looks up the
/// body and emits `#footnote[body]`.
fn translate_event_stream(
    state: &mut TypstEmitState,
    events: &[Event<'static>],
    footnote_bodies: &std::collections::HashMap<String, String>,
) {
    let mut i = 0;
    while i < events.len() {
        // Skip footnote definition bodies — their inner events
        // were translated in the first pass and will be emitted
        // at reference sites. Walking them again would duplicate
        // the content.
        if let Event::Start(Tag::FootnoteDefinition(_)) = &events[i] {
            let mut depth = 1;
            i += 1;
            while i < events.len() && depth > 0 {
                match &events[i] {
                    Event::Start(Tag::FootnoteDefinition(_)) => depth += 1,
                    Event::End(TagEnd::FootnoteDefinition) => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            continue;
        }
        emit_event(state, events[i].clone(), footnote_bodies);
        i += 1;
    }
}

/// First pass: collect all footnote definition bodies keyed by
/// their label. The body for a definition with label `L` is the
/// sub-stream of events between `Tag::FootnoteDefinition(L)` and
/// the matching `TagEnd::FootnoteDefinition` (depth-tracked so
/// nested definitions work). Each body is translated to Typst
/// using a fresh state, and stored in the map for the second
/// pass to emit at reference sites.
fn collect_footnote_bodies(events: &[Event<'static>]) -> std::collections::HashMap<String, String> {
    let mut bodies: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut i = 0;
    while i < events.len() {
        if let Event::Start(Tag::FootnoteDefinition(label)) = &events[i] {
            let label_str = label.to_string();
            let body_start = i + 1;
            let mut depth = 1;
            let mut j = body_start;
            while j < events.len() {
                match &events[j] {
                    Event::Start(Tag::FootnoteDefinition(_)) => depth += 1,
                    Event::End(TagEnd::FootnoteDefinition) => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            // events[body_start..j] are the inner events for
            // this definition. Translate them with a fresh
            // state. We pass the (still-empty) `bodies` map
            // so nested footnote references inside the body
            // also resolve correctly.
            let inner_events: Vec<Event<'static>> = events[body_start..j].to_vec();
            let body_typst = translate_inner_events(&inner_events, &bodies);
            bodies.insert(label_str, body_typst);
            i = j + 1;
        } else {
            i += 1;
        }
    }
    bodies
}

/// Translate a sub-stream of events (the inner events of a
/// footnote definition body) to Typst markup. Uses a fresh
/// state, so the result is self-contained and can be inlined
/// into a `#footnote[...]` content block at a reference site.
fn translate_inner_events(
    events: &[Event<'static>],
    footnote_bodies: &std::collections::HashMap<String, String>,
) -> String {
    let mut state = TypstEmitState::default();
    translate_event_stream(&mut state, events, footnote_bodies);
    close_state(&mut state);
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
    /// cell. Escape user content for safe inclusion in the right
    /// destination:
    ///
    /// - **Code buffer**: the body is later wrapped in a Typst
    ///   string literal (`"...body..."`) and only `\` and `"`
    ///   need to be escaped there. Every other char — including
    ///   `{`, `}`, `*`, `_`, `#` — is literal inside a string
    ///   literal. The previous behaviour routed through
    ///   `escape_typst` here too, which was a latent
    ///   double-escape (harmless until `escape_typst` started
    ///   escaping `{` and `}` for content-block safety; the
    ///   double-escape became visible as `\\{\\}` in the
    ///   generated Typst). The code-buffer target is the only
    ///   string-literal destination; the main output and table
    ///   cells are content blocks and continue to use
    ///   `escape_typst`.
    /// - **Autolink text** (the URL is rendered as the link's
    ///   text content): the URL may contain `:` and `//` which
    ///   Typst treats as markup-active in content mode (label
    ///   terminator and line-break marker). The stricter
    ///   `escape_typst_autolink` is autolink-only — escaping
    ///   `:` in general text would break labelled content the
    ///   user actually wants (e.g. "Step 1: do X" inside a
    ///   callout).
    /// - **Other text**: the standard markup escape.
    fn push_inline(&mut self, text: &str) {
        if let Some(buf) = self.code_buffer.as_mut() {
            // Code body is going into a string literal — only
            // `\` and `"` are active there.
            buf.body.push_str(&escape_typst_string(text));
            return;
        }
        let escaped = if self.in_autolink {
            escape_typst_autolink(text)
        } else {
            escape_typst(text)
        };
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

fn emit_event(
    state: &mut TypstEmitState,
    event: Event<'_>,
    footnote_bodies: &std::collections::HashMap<String, String>,
) {
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
                // Emit a Typst `box` wrapping a `text` call holding
                // the inline code body. The previous form used
                // `raw("...")` which renders the inline background
                // and inset but drops the body glyphs in
                // typst-as-lib 0.16 / typst 0.15.1 (ADR gap #2).
                // The `text` function bypasses the broken `raw`
                // element entirely.
                //
                // The body is a string literal so no markup
                // escaping is needed — only `\` and `"` are escaped
                // via `escape_typst_string`. Embedded backticks
                // (which were the original motivation for the
                // string form) render literally. The trailing space
                // is the chain break used for inline code, inline
                // HTML, and math: a function call followed by
                // `(...)` or `[...]` chains, and content can't be
                // called. The space forces a new content sequence.
                // See [`Event::InlineHtml`] for the full rationale.
                let rendered = format!(
                    "#box(fill: luma(245), inset: 2pt, radius: 2pt, \
                     text(font: (\"DejaVu Sans Mono\", \
                     \"Liberation Mono\", \"Courier New\"), \
                     size: 0.9em, \"{}\")) ",
                    escape_typst_string(&code)
                );
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
        Event::FootnoteReference(label) => {
            // Footnote reference: look up the definition body in
            // `footnote_bodies` (collected in the first pass) and
            // emit Typst's `#footnote[body]` content block. The
            // trailing space is the chain-break used everywhere
            // else (inline code, inline HTML, math) — content
            // can't be called, so a function-call-form expression
            // followed by `(...)` or `[...]` would chain and
            // error out.
            //
            // Undefined references (the spec allows references
            // without a matching definition) emit a visible
            // placeholder rather than failing. The placeholder
            // is a `#footnote` with a "missing:" prefix and the
            // label as inline content, so the user sees the
            // dangling reference in the PDF.
            let body = footnote_bodies.get(label.as_ref());
            match body {
                Some(b) => state.push_raw(&format!("#footnote[{b}] ")),
                None => state.push_raw(&format!(
                    "#footnote[missing: {}] ",
                    escape_typst(label.as_ref())
                )),
            }
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
                // Emit a Typst `block` wrapping a `text` call holding
                // the code body. The previous form used
                // `raw(block: true, lang: ..., "...")` which renders
                // the block border, padding, and framing correctly
                // but drops the body glyphs in typst-as-lib 0.16 /
                // typst 0.15.1 (ADR gap #5). The `text` function
                // bypasses the broken `raw` element entirely; the
                // styling matches the `#show raw.where(block: true)`
                // rule in the template (fill, inset, radius, width),
                // which becomes a no-op for code blocks but is still
                // used by HTML blocks.
                //
                // The body is a string literal so no markup escaping
                // is needed — only `\` and `"` are escaped via
                // `escape_typst_string`. Newlines in the body are
                // preserved as soft breaks and render as line breaks
                // because `#set par(justify: false)` is in scope.
                // The language hint is preserved as a comment for
                // debuggability — the new path doesn't have access to
                // typst's syntax highlighter, so the hint is
                // informational only. The lang is set from
                // `Tag::CodeBlock` (not via `push_inline`), so
                // it has not been escaped yet — the string
                // escape here is the *only* escape.
                let lang_comment = if buf.lang.is_empty() {
                    String::new()
                } else {
                    format!("\n  // lang: {}", escape_typst_string(&buf.lang))
                };
                // The body is accumulated by `push_inline`,
                // which routes code-buffer text through
                // `escape_typst_string` (see the comment on
                // that function). The body is therefore
                // already escaped for inclusion in a string
                // literal — running `escape_typst_string` on
                // it again would double-escape the backslashes
                // (a `\` would become `\\\\` instead of `\\`).
                // The proptest `escape_typst_string_is_idempotent`
                // catches this class of regression: the
                // function is not idempotent, and the body
                // must not be re-escaped here.
                state.output.push_str(&format!(
                    "#block(\n  fill: luma(245),\n  inset: 8pt,\n  \
                     radius: 4pt,\n  width: 100%\n)[\n  \
                     #set text(font: (\"DejaVu Sans Mono\", \
                     \"Liberation Mono\", \"Courier New\"), size: 9pt)\n  \
                     #set par(justify: false, leading: 0.5em){lang_comment}\n  \
                     #text(\"{}\")\n]",
                    buf.body
                ));
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
/// - `{` `}` (code-block delimiters — markup-active in content
///   mode; a literal `{` in user text would otherwise open a
///   code block and the following content would be parsed as
///   script until the matching `}`. Added after the code-block
///   emit switched from `raw` to `block + text`; even though the
///   new emit routes code bodies through a string literal where
///   `{` is literal, any other place the translator emits body
///   content (paragraphs, headings, table cells) needs `{`
///   escaped to survive user-supplied braces in prose.)
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
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
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
#[path = "typst_translator_tests.rs"]
mod tests;

// Property tests for the escape_typst* family. The four closed
// escape gaps from the ADR (gaps #2, #5, #8, #9) have
// example-based unit tests; this proptest layers a stronger
// property check on top — every char in the documented escape
// set is verified against random inputs. Sidecar of
// 	ypst_translator.rs per AGENTS.md RUST-056 / RUST-057.
#[cfg(test)]
#[path = "typst_translator_proptests.rs"]
mod proptests;
