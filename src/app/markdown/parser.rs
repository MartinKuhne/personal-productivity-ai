//! Pure Markdown and YAML parsing logic isolated from UI rendering.
//!
//! # Implementation note: pulldown-cmark inline-parser fragmentation
//!
//! Pulldown-cmark's inline parser builds a *tree of items*, not a
//! stream of inline text. Every delimiter run (`*`, `_`, `~`, `=`, `^`)
//! becomes a separate `ItemBody::Text` item in the internal tree
//! even when the delimiter fails to form a valid emphasis /
//! strikethrough / superscript / subscript pair. When the tree is
//! flattened to events, each `ItemBody::Text` becomes a separate
//! `Event::Text` (see pulldown-cmark `parse.rs::item_to_event`,
//! which returns `Event::Text(text[item.start..item.end])` per
//! `Text` item).
//!
//! The practical consequence for this codebase: a plain-text
//! fragment that *happens* to contain a delimiter run is split
//! into multiple `Event::Text` events at every delimiter. Examples:
//!
//!   - `~4,031 / ~19,000` with `ENABLE_STRIKETHROUGH` → 4 events
//!     (`~`, `4,031 / `, `~`, `19,000`). No `Strikethrough`
//!     Start/End pair fires — the single-`~` form isn't a valid
//!     GFM strikethrough — but cmark still fragments the text.
//!     (See cmark 0.13.4 `parse.rs:1063-1106`.)
//!   - `*bold*` with emphasis disabled → 3 events (`*`, `bold`, `*`).
//!   - `_em_` with emphasis disabled → 3 events.
//!   - `^super^` / `=highlight=` with those options disabled → 3 events each.
//!
//! Faithfully mapping each cmark `Event::Text` to its own
//! `InlineElem::Text` would produce ASTs that don't match the
//! table-renderer's contract: a cell containing `~text~` would
//! fragment into 3 elements, the text-measurer would tokenize it
//! as 3 runs, and the FTWA pipeline would produce wrong widths
//! (the production bug fixed by the coalescer).
//!
//! The fix lives in the two helpers `push_text_coalesce` and
//! `push_link_coalesce` (defined below) — helpers that fold
//! consecutive same-style `Text` events (or same-URL `Link` events)
//! into a single `InlineElem` at the push site. See those functions'
//! doc comments for the exact rules. The test
//! `cmark_strikethrough_fragments_single_tilde` in this module pins
//! the cmark event count so a future upgrade / option change that
//! shifts the count surfaces immediately.
//!
//! This is a workaround, not a fix at the source. The cleaner
//! long-term options (none of which we've done) are:
//!   1. Upgrade to a pulldown-cmark version that emits a single
//!      `Text` event for un-delimited fragments (if / when such a
//!      version exists). At the time of writing (0.13.4), none
//!      does.
//!   2. Switch markdown parsers (e.g. comrak). Heavy change,
//!      unjustified for this single quirk.
//!   3. Disable `ENABLE_STRIKETHROUGH` and accept the loss of
//!      GFM strikethrough. The right answer *only* if strikethrough
//!      isn't actually used in any user-facing document.
//!
//! Until any of those land, the coalescer is the right balance:
//! keeps the parser's surface tiny (two ~10-line helpers), keeps
//! GFM strikethrough working, and locks the upstream-fragmentation
//! count in a regression test.

use crate::markdown::model::{InlineElem, RenderEvent, TextStyle};

/// Renders markdown source to an HTML string.
pub fn render_markdown_to_html(markdown: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let parser = Parser::new_ext(markdown, Options::all());
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// Append a plain-text run to `buffer`, coalescing with the previous
/// `InlineElem::Text` when the style matches.
///
/// # Why this exists
///
/// Pulldown-cmark's inline parser fragments plain text at every
/// delimiter run (`*`, `_`, `~`, `=`, `^`), even when the
/// delimiter fails to form a valid emphasis / strikethrough /
/// superscript / subscript pair. See the module-level doc comment
/// for the full explanation and the upstream behavior we work
/// around. The short version: without this helper, plain content
/// like `~4,031 / ~19,000` becomes four `InlineElem::Text`
/// entries instead of one, the cell text-measurer tokenizes it
/// as four runs, and the FTWA pipeline produces wrong column
/// widths.
///
/// # Coalesce rules
///
/// - **Same `TextStyle`**: append to the previous `Text` element
///   (its string is mutated in place; no new allocation).
/// - **Different `TextStyle`**: push a new `Text` element. Bold /
///   italic / code / strikethrough transitions are preserved as
///   separate elements so the renderer can apply distinct styling
///   to each run.
/// - **Previous element is not `Text`** (e.g. `Link`, `Image`,
///   `Code`, `Html`, `SoftBreak`): push a new `Text` element.
///   The `Text` → non-`Text` → `Text` boundary always produces
///   two elements; only consecutive `Text` elements of the same
///   style coalesce.
///
/// # Where to call this
///
/// Anywhere a cmark `Event::Text` (or `Event::Code` with the same
/// style) is pushed to `buffered_inline` or `heading_elems`. The
/// parser's main `Event::Text` handler routes through this helper
/// for both the main inline path and the heading path.
///
/// # Testing
///
/// Unit tests for the four coalesce branches live in
/// `push_text_coalesce_*` and `push_link_coalesce_*` in this
/// module's test submodule. The end-to-end regression
/// (`parses_laptops_table_cells_to_single_text_element`) verifies
/// the production case.
fn push_text_coalesce(buffer: &mut Vec<InlineElem>, text: &str, style: &TextStyle) {
    if let Some(InlineElem::Text(existing, existing_style)) = buffer.last_mut()
        && existing_style == style
    {
        existing.push_str(text);
        return;
    }
    buffer.push(InlineElem::Text(text.to_string(), style.clone()));
}

/// Like [`push_text_coalesce`] but for `Link` elements.
///
/// # Why this exists
///
/// Inside a `[display text](url)` link, cmark emits one `Event::Text`
/// per delimiter run in the display text (same fragmentation as
/// for plain text). Without coalescing, multiple `Text` events
/// inside a single link become multiple `Link(url, ...)` elements
/// with the same URL but disjoint text fragments — a link to
/// `https://x.com` containing `a *b* c` (with emphasis on) would
/// come out as `Link("https://x.com", "a ")`, `Text("b", bold)`,
/// `Link("https://x.com", " c")` instead of a single
/// `Link("https://x.com", "a b c")` with a nested `Text("b", bold)`.
/// (The latter is what the link-renderer expects: a single
/// `InlineElem::Link` whose text is the flat concatenation of the
/// display content, with nested styles as separate `Text` elements
/// inside that link — but our `InlineElem::Link` carries a single
/// `String` for the text, so the inner style nuance is lost
/// either way; the practical concern is just the URL/text split.)
///
/// # Coalesce rules
///
/// Same as [`push_text_coalesce`] but keying on the URL instead of
/// the style: consecutive `Text` events with the same link URL
/// fold into a single `Link(url, concatenated_text)` element. A
/// URL change starts a new element; a different `InlineElem`
/// variant on the buffer also starts a new element.
fn push_link_coalesce(buffer: &mut Vec<InlineElem>, url: &str, text: &str) {
    if let Some(InlineElem::Link(existing_url, existing_text)) = buffer.last_mut()
        && existing_url == url
    {
        existing_text.push_str(text);
        return;
    }
    buffer.push(InlineElem::Link(url.to_string(), text.to_string()));
}

/// Scans text for bare URLs and wikilinks, coalescing with the previous
/// text element if one exists with the same style to prevent delimiter
/// fragmentation from splitting URLs.
fn push_scanned_text(buffer: &mut Vec<InlineElem>, text: &str, style: &TextStyle) {
    let combined_text;
    let text_to_scan = if let Some(InlineElem::Text(prev, prev_style)) = buffer.last() {
        if prev_style == style {
            combined_text = format!("{prev}{text}");
            buffer.pop();
            &combined_text
        } else {
            text
        }
    } else {
        text
    };

    let scanned = crate::markdown::scan_text_for_links(text_to_scan, style);
    for elem in scanned {
        match elem {
            InlineElem::Text(t, s) => push_text_coalesce(buffer, &t, &s),
            InlineElem::Link(u, t) => push_link_coalesce(buffer, &u, &t),
            other => buffer.push(other),
        }
    }
}

/// The GFM option set FastMD parses with — MD-001's feature list, and the
/// exact set [UI-043] names for the editor's pre-save validation.
///
/// Single source of truth so the renderer and [`super::validation::validate_markdown`]
/// cannot drift apart: UI-043 requires the editor to validate "with the same
/// GFM parser used for rendering", which structurally means the same option set.
///
/// [UI-043]: `src/app/ui/SPEC.md`
pub fn gfm_options() -> pulldown_cmark::Options {
    use pulldown_cmark::Options;

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    // ENABLE_STRIKETHROUGH enables GFM `~~text~~` (double tilde)
    // and `~text~` (single tilde) strikethrough. Required for
    // strikethrough support in user documents. Trade-off (managed
    // by the `push_text_coalesce` helper above): with this option
    // on, cmark also fragments plain text at every `~` delimiter
    // run — see the module-level doc comment for the full
    // explanation and the regression test for the fix.
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options
}

/// Parses markdown text into a sequence of render events.
#[tracing::instrument(skip_all, name = "markdown.parse_to_events", level = "debug")]
pub fn parse_markdown_to_events(markdown_text: &str) -> Vec<RenderEvent> {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

    let options = gfm_options();
    let parser = Parser::new_ext(markdown_text, options);
    let mut events = Vec::new();

    let mut in_code_block = false;
    let mut code_block_lang: Option<String> = None;
    let mut code_block_content = String::new();
    let mut in_heading = false;
    let mut heading_level = 0;
    let mut heading_elems: Vec<InlineElem> = Vec::new();

    let mut buffered_inline: Vec<InlineElem> = Vec::new();
    let mut current_style = TextStyle::default();
    let mut link_url = String::new();
    let mut in_link = false;
    let mut list_depth = 0;
    let mut needs_bullet = false;
    let mut task_checked = None;

    let mut in_table_cell = false;
    let mut table_cells: Vec<Vec<Vec<InlineElem>>> = Vec::new();
    let mut current_row: Vec<Vec<InlineElem>> = Vec::new();

    let mut list_ordinal_stack: Vec<Option<u64>> = Vec::new();

    let push_inline = |events: &mut Vec<RenderEvent>,
                       elems: &mut Vec<InlineElem>,
                       bullet: &mut bool,
                       task: &mut Option<bool>,
                       indent: usize,
                       list_ordinal: Option<u64>| {
        if elems.is_empty() && !*bullet && task.is_none() {
            return;
        }
        events.push(RenderEvent::FlushInline {
            elems: elems.clone(),
            needs_bullet: *bullet,
            task_checked: *task,
            indent,
            list_ordinal,
        });
        elems.clear();
        *bullet = false;
        *task = None;
    };

    macro_rules! flush_buffered {
        () => {
            push_inline(
                &mut events,
                &mut buffered_inline,
                &mut needs_bullet,
                &mut task_checked,
                list_depth,
                list_ordinal_stack.last().copied().flatten(),
            )
        };
    }

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                if !buffered_inline.is_empty() {
                    flush_buffered!();
                }
                in_code_block = true;
                code_block_content.clear();
                code_block_lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) if !lang.is_empty() => {
                        Some(lang.to_string())
                    }
                    _ => None,
                };
            }
            Event::End(TagEnd::CodeBlock) => {
                if in_code_block {
                    in_code_block = false;
                    events.push(RenderEvent::CodeBlock {
                        language: code_block_lang.take(),
                        content: code_block_content.clone(),
                    });
                }
            }
            Event::Start(Tag::Heading { level, .. }) => {
                if !buffered_inline.is_empty() {
                    flush_buffered!();
                }
                in_heading = true;
                heading_level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                heading_elems.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                events.push(RenderEvent::Heading {
                    level: heading_level,
                    elems: heading_elems.clone(),
                });
                in_heading = false;
                heading_level = 0;
            }
            Event::Start(Tag::Paragraph) => {
                if !in_table_cell && !buffered_inline.is_empty() {
                    flush_buffered!();
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if !in_table_cell {
                    flush_buffered!();
                    events.push(RenderEvent::Space(4.0));
                }
            }
            Event::Start(Tag::List(list_kind)) => {
                if !buffered_inline.is_empty() {
                    flush_buffered!();
                }
                list_depth += 1;
                list_ordinal_stack.push(list_kind);
            }
            Event::End(TagEnd::List(_)) => {
                flush_buffered!();
                list_depth = list_depth.saturating_sub(1);
                list_ordinal_stack.pop();
            }
            Event::Start(Tag::Item) => {
                if !buffered_inline.is_empty() {
                    flush_buffered!();
                }
                needs_bullet = true;
            }
            Event::End(TagEnd::Item) => {
                flush_buffered!();
                if let Some(Some(n)) = list_ordinal_stack.last_mut() {
                    *n += 1;
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                if !buffered_inline.is_empty() {
                    flush_buffered!();
                }
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                flush_buffered!();
            }
            Event::Start(Tag::Table(_)) => {
                if !buffered_inline.is_empty() {
                    flush_buffered!();
                }
                table_cells.clear();
            }
            Event::End(TagEnd::Table) => {
                if !table_cells.is_empty() {
                    events.push(RenderEvent::Table(table_cells.clone()));
                    events.push(RenderEvent::Space(4.0));
                }
                table_cells.clear();
            }
            Event::Start(Tag::TableHead) => {
                current_row.clear();
            }
            Event::End(TagEnd::TableHead) => {
                if !current_row.is_empty() {
                    table_cells.push(current_row.clone());
                    current_row.clear();
                }
            }
            Event::Start(Tag::TableRow) => {
                current_row.clear();
            }
            Event::End(TagEnd::TableRow) => {
                table_cells.push(current_row.clone());
                current_row.clear();
            }
            Event::Start(Tag::TableCell) => {
                in_table_cell = true;
                if !buffered_inline.is_empty() {
                    flush_buffered!();
                }
            }
            Event::End(TagEnd::TableCell) => {
                in_table_cell = false;
                current_row.push(buffered_inline.clone());
                buffered_inline.clear();
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) => {
                in_link = true;
                if link_type == pulldown_cmark::LinkType::Email && !dest_url.starts_with("mailto:")
                {
                    link_url = format!("mailto:{dest_url}");
                } else {
                    link_url = dest_url.to_string();
                }
            }
            Event::End(TagEnd::Link) => {
                in_link = false;
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                buffered_inline.push(InlineElem::Image(dest_url.to_string()));
            }
            Event::End(TagEnd::Image) => {}
            Event::Start(Tag::Emphasis) => current_style.italic = true,
            Event::End(TagEnd::Emphasis) => current_style.italic = false,
            Event::Start(Tag::Strong) => current_style.bold = true,
            Event::End(TagEnd::Strong) => current_style.bold = false,
            Event::Start(Tag::Strikethrough) => current_style.strikethrough = true,
            Event::End(TagEnd::Strikethrough) => current_style.strikethrough = false,
            // Text events must route through the coalescer helpers
            // (not a direct `push` to the buffer). See the
            // module-level doc comment and the `push_text_coalesce` /
            // `push_link_coalesce` doc comments for why. The short
            // version: cmark fragments plain text at every delimiter
            // run (see cmark 0.13.4 `parse.rs:1063-1106`), and a
            // direct push would propagate the fragmentation to the
            // AST the table-renderer consumes, producing wrong
            // column widths.
            Event::Text(text) => {
                if in_code_block {
                    code_block_content.push_str(&text);
                } else if in_link {
                    if in_heading {
                        push_link_coalesce(&mut heading_elems, &link_url, &text);
                    } else {
                        push_link_coalesce(&mut buffered_inline, &link_url, &text);
                    }
                } else if in_heading {
                    push_scanned_text(&mut heading_elems, &text, &current_style);
                } else {
                    push_scanned_text(&mut buffered_inline, &text, &current_style);
                }
            }
            Event::Code(code) => {
                if in_code_block {
                    code_block_content.push_str(&code);
                } else if in_link {
                    if in_heading {
                        push_link_coalesce(&mut heading_elems, &link_url, &code);
                    } else {
                        push_link_coalesce(&mut buffered_inline, &link_url, &code);
                    }
                } else if in_heading {
                    let mut s = current_style.clone();
                    s.code = true;
                    heading_elems.push(InlineElem::Text(code.to_string(), s));
                } else {
                    let mut s = current_style.clone();
                    s.code = true;
                    buffered_inline.push(InlineElem::Text(code.to_string(), s));
                }
            }
            Event::SoftBreak => {
                if !in_code_block && !in_heading {
                    buffered_inline.push(InlineElem::SoftBreak);
                }
            }
            Event::HardBreak => {
                if !in_code_block && !in_heading {
                    if !in_table_cell {
                        flush_buffered!();
                    } else {
                        buffered_inline.push(InlineElem::SoftBreak);
                    }
                }
            }
            Event::Rule => {
                flush_buffered!();
                events.push(RenderEvent::Separator);
            }
            Event::TaskListMarker(checked) => {
                task_checked = Some(checked);
                needs_bullet = false;
            }
            Event::Html(html) => {
                buffered_inline.push(InlineElem::Html(html.to_string()));
            }
            Event::InlineHtml(html) => {
                buffered_inline.push(InlineElem::Html(html.to_string()));
            }
            Event::FootnoteReference(name) => {
                let text = format!("[^{}]", name);
                let mut s = current_style.clone();
                s.code = true;
                buffered_inline.push(InlineElem::Text(text, s));
            }
            Event::Start(Tag::FootnoteDefinition(name)) => {
                if !buffered_inline.is_empty() {
                    flush_buffered!();
                }
                events.push(RenderEvent::Separator);
                let text = format!("[^{}]: ", name);
                let mut s = current_style.clone();
                s.bold = true;
                buffered_inline.push(InlineElem::Text(text, s));
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                flush_buffered!();
            }
            Event::Start(Tag::HtmlBlock) => {
                if !buffered_inline.is_empty() {
                    flush_buffered!();
                }
            }
            Event::End(TagEnd::HtmlBlock) => {
                flush_buffered!();
            }
            _ => {}
        }
    }
    flush_buffered!();

    events
}

/// Parses a YAML mapping into a list of key-value string pairs.
pub fn parse_yaml_to_pairs(yaml: &serde_norway::Value) -> Option<Vec<(String, String)>> {
    let mapping = yaml.as_mapping()?;
    let mut pairs = Vec::new();
    for (key, value) in mapping {
        let val_str = match value {
            serde_norway::Value::String(s) => s.clone(),
            serde_norway::Value::Sequence(seq) => {
                let items: Vec<String> = seq
                    .iter()
                    .map(|v| match v {
                        serde_norway::Value::String(s) => s.clone(),
                        _ => serde_norway::to_string(v)
                            .unwrap_or_default()
                            .trim()
                            .to_string(),
                    })
                    .collect();
                items.join(", ")
            }
            _ => serde_norway::to_string(value)
                .unwrap_or_default()
                .trim()
                .to_string(),
        };
        pairs.push((
            serde_norway::to_string(key)
                .unwrap_or_default()
                .trim()
                .to_string(),
            val_str,
        ));
    }
    Some(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Local helper for tests: concatenate the plain-text content of
    /// a cell's inlines. Mirrors `ui::render::tests::cell_text` but
    /// kept local so the parser tests don't depend on UI internals.
    fn cell_text(cell: &[InlineElem]) -> String {
        let mut s = String::new();
        for e in cell {
            match e {
                InlineElem::Text(t, _) => s.push_str(t),
                InlineElem::Link(_, t) => s.push_str(t),
                InlineElem::Image(url) => s.push_str(&format!("[Image: {url}]")),
                InlineElem::Html(h) => s.push_str(h),
                InlineElem::SoftBreak => s.push(' '),
            }
        }
        s
    }

    #[test]
    fn test_render_markdown_to_html() {
        let md = "# Title\n\nHello *world*";
        let html = render_markdown_to_html(md);
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<em>world</em>"));
    }

    #[test]
    fn test_parse_yaml_to_pairs() {
        let yaml: serde_norway::Value = serde_norway::from_str("key: val\nlist: [a, b]").unwrap();
        let pairs = parse_yaml_to_pairs(&yaml).unwrap();
        assert_eq!(pairs[0], ("key".to_string(), ("val").to_string()));
        assert_eq!(pairs[1], ("list".to_string(), ("a, b").to_string()));
    }

    /// **Pulldown-cmark 0.13.4 inline-parser design property** (not a
    /// bug per se, but a behavior the consumer has to handle):
    /// with `Options::ENABLE_STRIKETHROUGH` enabled, cmark emits
    /// one `Text` event per delimiter run — so the input
    /// `~4,031 / ~19,000` becomes 4 separate `Text` events
    /// (one for each `~`, one for the text between) even though
    /// no actual `Strikethrough` Start/End pair fires. This is
    /// the inline parser's "tree of items" model: every delimiter
    /// run gets its own item, whether or not it forms a valid
    /// pair. See the module-level doc comment for the full
    /// design discussion and the long-term options.
    ///
    /// Our parser works around it by coalescing consecutive
    /// `Text` events with the same `TextStyle` (see
    /// `push_text_coalesce` in this module), so the AST emitted
    /// to the rest of the renderer is a single `InlineElem::Text`
    /// per coalesced run. This test pins the cmark behavior so
    /// future upgrades to pulldown-cmark (or changes to the
    /// option set) that change the fragment count don't silently
    /// regress the coalescing.
    ///
    /// **If this test fails**, the most likely cause is a
    /// pulldown-cmark upgrade. The fix is one of:
    ///   1. Re-tune `push_text_coalesce` to match the new fragment
    ///      count (and add a test for the new case).
    ///   2. Drop `ENABLE_STRIKETHROUGH` from the options set if
    ///      GFM strikethrough isn't needed (see module doc).
    ///   3. Switch to a different markdown parser.
    ///
    /// **What this test does NOT do:** assert that the coalesced
    /// AST is correct. That's the responsibility of
    /// `parses_laptops_table_cells_to_single_text_element` and
    /// the integration test `test_parse_laptops_table_ast_shape`.
    /// This test is purely the upstream-behavior pin.
    #[test]
    fn cmark_strikethrough_fragments_single_tilde() {
        use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);

        let md = "~4,031 / ~19,000";
        let events: Vec<Event> = Parser::new_ext(md, options).collect();

        // Cmark emits 6 events for this input:
        //   Start(Paragraph), Text("~"), Text("4,031 / "),
        //   Text("~"), Text("19,000"), End(Paragraph).
        // The 4 Text events are the per-delimiter-run fragments
        // our coalescer folds back together. If a future cmark
        // version changes the count (e.g. by skipping the bare `~`
        // runs entirely), this test will fail and the coalescer
        // can be re-tuned.
        assert_eq!(
            events.len(),
            6,
            "expected 6 cmark events (Paragraph+4 Text+End) for {md:?}; got {} events: {:?}",
            events.len(),
            events
        );
        assert!(
            matches!(events[0], Event::Start(Tag::Paragraph)),
            "expected Paragraph Start at index 0; got {:?}",
            events[0]
        );
        for (i, expected_text) in ["~", "4,031 / ", "~", "19,000"].iter().enumerate() {
            let actual = match &events[i + 1] {
                Event::Text(s) => s.as_ref(),
                other => panic!(
                    "event at index {idx} (cell {i}) should be Text({expected_text:?}); got {other:?}",
                    idx = i + 1
                ),
            };
            assert_eq!(
                actual,
                *expected_text,
                "Text event at cell {i} (events[{idx}]) should be {expected_text:?}; got {actual:?}",
                idx = i + 1
            );
        }
        assert!(
            matches!(events[5], Event::End(TagEnd::Paragraph)),
            "expected Paragraph End at index 5; got {:?}",
            events[5]
        );
    }

    /// Unit tests for [`push_text_coalesce`] and [`push_link_coalesce`].
    /// These helpers fold consecutive same-style `Text` / same-URL
    /// `Link` fragments into a single `InlineElem` to undo
    /// pulldown-cmark's per-delimiter-run `Text` splitting.
    /// See the module-level `cmark_strikethrough_fragments_single_tilde`
    /// test for the upstream behavior they defend against.
    #[test]
    fn push_text_coalesce_appends_to_previous_same_style() {
        let mut buf: Vec<InlineElem> = Vec::new();
        let s = TextStyle::default();
        push_text_coalesce(&mut buf, "hello ", &s);
        push_text_coalesce(&mut buf, "world", &s);
        assert_eq!(buf.len(), 1, "two same-style pushes should coalesce to one");
        assert_eq!(cell_text(&buf), "hello world");
    }

    #[test]
    fn push_text_coalesce_starts_new_element_on_style_change() {
        let mut buf: Vec<InlineElem> = Vec::new();
        let s_bold = TextStyle {
            bold: true,
            ..TextStyle::default()
        };
        push_text_coalesce(&mut buf, "plain", &TextStyle::default());
        push_text_coalesce(&mut buf, "bold", &s_bold);
        assert_eq!(buf.len(), 2, "style change must start a new element");
        assert_eq!(cell_text(&buf), "plainbold");
        assert!(
            matches!(&buf[0], InlineElem::Text(t, s) if t == "plain" && s == &TextStyle::default())
        );
        assert!(matches!(&buf[1], InlineElem::Text(t, s) if t == "bold" && s == &s_bold));
    }

    #[test]
    fn push_text_coalesce_handles_empty_buffer() {
        let mut buf: Vec<InlineElem> = Vec::new();
        push_text_coalesce(&mut buf, "first", &TextStyle::default());
        assert_eq!(buf.len(), 1);
        assert_eq!(cell_text(&buf), "first");
    }

    #[test]
    fn push_text_coalesce_starts_new_element_after_non_text() {
        let mut buf: Vec<InlineElem> = Vec::new();
        // Seed with a non-Text variant (Image). Coalesce must
        // NOT merge into it even if the style happens to match.
        buf.push(InlineElem::Image("pic.png".to_string()));
        push_text_coalesce(&mut buf, "after image", &TextStyle::default());
        assert_eq!(
            buf.len(),
            2,
            "non-Text variant must not coalesce with a Text push"
        );
        assert!(matches!(&buf[0], InlineElem::Image(url) if url == "pic.png"));
        assert!(matches!(&buf[1], InlineElem::Text(t, _) if t == "after image"));
    }

    #[test]
    fn push_link_coalesce_appends_to_previous_same_url() {
        let mut buf: Vec<InlineElem> = Vec::new();
        push_link_coalesce(&mut buf, "https://example.com", "hello ");
        push_link_coalesce(&mut buf, "https://example.com", "world");
        assert_eq!(buf.len(), 1, "two same-URL link pushes should coalesce");
        assert!(matches!(
            &buf[0],
            InlineElem::Link(url, text) if url == "https://example.com" && text == "hello world"
        ));
    }

    #[test]
    fn push_link_coalesce_starts_new_element_on_url_change() {
        let mut buf: Vec<InlineElem> = Vec::new();
        push_link_coalesce(&mut buf, "https://a.example", "x");
        push_link_coalesce(&mut buf, "https://b.example", "y");
        assert_eq!(buf.len(), 2, "URL change must start a new element");
    }

    /// The user-visible regression: the laptops table from the
    /// `test_parse_laptops_table_ast_shape` integration test is
    /// fixed end-to-end by the coalescer. This unit test pins the
    /// same fix at the parser level so the behavior is locked in
    /// even if the integration test moves or is deleted.
    #[test]
    fn parses_laptops_table_cells_to_single_text_element() {
        let md = "| Make | Model and Model Number | Market Price | Display | Processor | PassMark Single / Multi | Summary |\n\
                  |------|----------------------|-------------|---------|-----------|------------------------|---------|\n\
                  | Acer | Swift 16 AI (SF16-71T) | $1,249-$1,799 | 16\" 3K (2880x1800) 120Hz OLED Touch | Intel Core Ultra 7 256V (8C/8T Lunar Lake) | ~4,031 / ~19,000 | Excellent value. Vibrant OLED display, exceptional battery life for a 16\" laptop, lightweight at ~3.3 lbs. Two Thunderbolt 4 ports. Praised by ZDNet, PCMag, and Notebookcheck. Great everyday performance and portability. |";
        let events = parse_markdown_to_events(md);
        let table = events
            .iter()
            .find_map(|e| {
                if let RenderEvent::Table(rows) = e {
                    Some(rows)
                } else {
                    None
                }
            })
            .expect("expected a Table event");
        assert_eq!(table.len(), 2, "header + 1 data row");
        // Cell 5 is the PassMark cell with `~4,031 / ~19,000` —
        // the case that used to fragment into 4 InlineElems.
        let cell5 = &table[1][5];
        assert_eq!(
            cell5.len(),
            1,
            "PassMark cell must coalesce to 1 element, got {cell5:?}"
        );
        assert!(matches!(
            &cell5[0],
            InlineElem::Text(t, s) if t == "~4,031 / ~19,000" && s == &TextStyle::default()
        ));
        // Cell 6 is the Summary cell with `~3.3 lbs` mid-sentence.
        let cell6 = &table[1][6];
        assert_eq!(
            cell6.len(),
            1,
            "Summary cell must coalesce to 1 element, got {cell6:?}"
        );
        assert!(matches!(
            &cell6[0],
            InlineElem::Text(t, s) if t.contains("~3.3 lbs") && s == &TextStyle::default()
        ));
    }

    /// Regression: CommonMark angle-bracket autolinks
    /// (`<https://example.com>`) must surface as a real
    /// `InlineElem::Link` in the AST, not as raw inline HTML.
    ///
    /// The autolink form is part of the CommonMark / Markdown
    /// Cheatsheet conformance requirement [MD-018] (links: inline,
    /// reference, **auto-links**). Without the
    /// `Options::ENABLE_AUTOLINKS` flag on the pulldown-cmark
    /// parser, cmark treats `<https://example.com>` as raw inline
    /// HTML and the renderer's `InlineElem::Html` branch paints it
    /// as a non-interactive gray italic label — the click does
    /// nothing. This test pins the parser contract so a future
    /// option-set refactor that drops `ENABLE_AUTOLINKS` fails
    /// loudly here instead of silently regressing in the UI.
    ///
    /// [MD-018]: `src/desktop/src/markdown/SPEC.md`
    /// Regression: CommonMark angle-bracket autolinks
    /// (`<https://example.com>`) must surface as a real
    /// `InlineElem::Link` in the AST, not as raw inline HTML.
    ///
    /// Pulldown-cmark 0.13 recognizes the CommonMark
    /// angle-bracket autolink form as a top-level parser feature
    /// (not gated on any `Options` flag) and emits it as a
    /// `Tag::Link` / `Text` pair that our parser coalesces into a
    /// single `InlineElem::Link(url, url)`. This test pins that
    /// contract: if a future cmark upgrade or option-set refactor
    /// changes that behavior, the test fails loudly here instead
    /// of silently regressing in the UI (where the autolink would
    /// be demoted to a non-clickable `InlineElem::Html` label).
    #[test]
    fn autolink_angle_bracket_becomes_inline_link() {
        let md = "<https://example.com>";
        let events = parse_markdown_to_events(md);

        // Find the inline run for the autolink. cmark wraps it in
        // Start(Paragraph) … End(Paragraph), and our parser emits
        // a single FlushInline between them.
        let flush = events
            .iter()
            .find_map(|e| {
                if let RenderEvent::FlushInline { elems, .. } = e {
                    Some(elems)
                } else {
                    None
                }
            })
            .expect("expected a FlushInline event for the autolink paragraph");

        assert_eq!(
            flush.len(),
            1,
            "angle-bracket autolink must surface as a single InlineElem::Link, got {flush:?}"
        );
        assert!(
            matches!(
                &flush[0],
                InlineElem::Link(url, text)
                    if url == "https://example.com" && text == "https://example.com"
            ),
            "autolink must become InlineElem::Link with the URL as both dest and display text, got {:?}",
            flush[0]
        );
        // Specifically: NOT raw inline HTML (the buggy pre-fix shape).
        assert!(
            !matches!(&flush[0], InlineElem::Html(_)),
            "autolink must not be silently demoted to InlineElem::Html"
        );
    }

    /// Companion to `autolink_angle_bracket_becomes_inline_link`:
    /// the autolink must coalesce cleanly with surrounding plain
    /// text. `Visit <https://example.com> for more.` must yield a
    /// three-element inline run (`Text`, `Link`, `Text`) with no
    /// spurious `Html` variant — the clickable middle element is
    /// what the renderer turns into a clickable link.
    #[test]
    fn autolink_mixed_with_plain_text_keeps_text_runs() {
        let md = "Visit <https://example.com> for more.";
        let events = parse_markdown_to_events(md);

        let flush = events
            .iter()
            .find_map(|e| {
                if let RenderEvent::FlushInline { elems, .. } = e {
                    Some(elems)
                } else {
                    None
                }
            })
            .expect("expected a FlushInline event");

        // Expected: Text("Visit "), Link(url, url), Text(" for more.")
        assert_eq!(flush.len(), 3, "got {flush:?}");
        assert!(
            matches!(&flush[0], InlineElem::Text(t, _) if t == "Visit "),
            "leading text run: got {:?}",
            flush[0]
        );
        assert!(
            matches!(
                &flush[1],
                InlineElem::Link(url, text)
                    if url == "https://example.com" && text == "https://example.com"
            ),
            "middle element must be the autolink: got {:?}",
            flush[1]
        );
        assert!(
            matches!(&flush[2], InlineElem::Text(t, _) if t == " for more."),
            "trailing text run: got {:?}",
            flush[2]
        );
    }

    /// The routers-spec table that surfaces the same pulldown-cmark
    /// fragmentation class as the laptops table, plus bold model
    /// names and a `~~strikethrough~~ **bold**` mid-cell. Per the
    /// module-level doc comment, every delimiter run (`*`, `_`,
    /// `~`, `=`, `^`) becomes its own `ItemBody::Text` even when
    /// the delimiter fails to form a valid emphasis /
    /// strikethrough / etc. pair — so:
    ///
    /// * `**TP-Link Archer BE550**` would fragment into 3 events
    ///   (`**`, `TP-Link Archer BE550`, `**`) without the
    ///   coalescer.
    /// * `~~$180–$210~~ **$125**` would fragment the `~~` runs
    ///   plus the `**` runs.
    /// * `~$180` (single tilde, invalid GFM strikethrough) would
    ///   fragment into `~` + `$180`.
    ///
    /// This test pins the parser's output for all three. The
    /// companion integration test
    /// `test_parse_routers_table_ast_shape` in
    /// `src/ui/render/tests.rs` walks the same fixture at the
    /// caller-facing level.
    #[test]
    fn parses_routers_table_cells_with_mixed_styling() {
        let md = "| Model | Price | Bands | 6 GHz | 2.5G Ports | 10G Port | Key Strength |\n\
                  |-------|-------|-------|-------|------------|----------|--------------|\n\
                  | **TP-Link Archer BE550** | $170–$200 | Tri | ✅ | 5x | ❌ | Best overall value |\n\
                  | **ASUS RT-BE92U** | $190–$200 | Tri | ✅ | 4x | ✅ (1) | 10G + Merlin |\n\
                  | **TP-Link Archer BE230** | $80–$100 | Dual | ❌ | 1x | ❌ | Budget entry |\n\
                  | **GL.iNet Flint 3** | ~~$180–$210~~ **$125** | Tri | ✅ | 5x | ❌ | OpenWrt + VPN |\n\
                  | **ASUS RT-BE59** | ~$180 | Dual | ❌ | 1x | ❌ | ASUS budget |";
        let events = parse_markdown_to_events(md);
        let table = events
            .iter()
            .find_map(|e| {
                if let RenderEvent::Table(rows) = e {
                    Some(rows)
                } else {
                    None
                }
            })
            .expect("expected a Table event");
        assert_eq!(table.len(), 6, "1 header + 5 data rows");
        assert_eq!(table[0].len(), 7, "7 columns per row");
        for (r, row) in table.iter().enumerate() {
            assert_eq!(row.len(), 7, "row {r} must have 7 cells");
        }

        // Header cells are plain text.
        let plain = TextStyle::default();
        for (j, expected) in [
            "Model",
            "Price",
            "Bands",
            "6 GHz",
            "2.5G Ports",
            "10G Port",
            "Key Strength",
        ]
        .iter()
        .enumerate()
        {
            let cell = &table[0][j];
            assert_eq!(
                cell.len(),
                1,
                "header cell {j} must be 1 element, got {cell:?}"
            );
            assert!(matches!(
                &cell[0],
                InlineElem::Text(t, s) if t == *expected && s == &plain
            ));
        }

        // Row 1: bold model, plain price, plain tri/✅/5x/❌/text.
        // `**TP-Link Archer BE550**` must coalesce to a single bold
        // Text element (the `*` runs collapse).
        let bold = TextStyle {
            bold: true,
            ..TextStyle::default()
        };
        let cell00 = &table[1][0];
        assert_eq!(
            cell00.len(),
            1,
            "Model cell (bold) must coalesce to 1 element, got {cell00:?}"
        );
        assert!(matches!(
            &cell00[0],
            InlineElem::Text(t, s) if t == "TP-Link Archer BE550" && s == &bold
        ));
        // Price cell: plain text (no delimiters).
        let cell01 = &table[1][1];
        assert_eq!(
            cell01.len(),
            1,
            "Price cell must be 1 element, got {cell01:?}"
        );
        assert!(matches!(
            &cell01[0],
            InlineElem::Text(t, s) if t == "$170–$200" && s == &plain
        ));
        // Emoji cells (✅, ❌) must be single text elements, not
        // split on the Unicode code point boundary.
        for j in [3, 5] {
            let cell = &table[1][j];
            assert_eq!(
                cell.len(),
                1,
                "row 1 col {j} emoji cell must be 1 element, got {cell:?}"
            );
            assert!(matches!(&cell[0], InlineElem::Text(_, s) if s == &plain));
        }
        // Key Strength: `10G + Merlin` is a `+` in plain text, must
        // not be parsed as anything other than text.
        let cell16 = &table[1][6];
        assert_eq!(
            cell16.len(),
            1,
            "Key Strength cell must be 1 element, got {cell16:?}"
        );
        assert!(matches!(
            &cell16[0],
            InlineElem::Text(t, s) if t == "Best overall value" && s == &plain
        ));

        // Row 4 (GL.iNet Flint 3): the `~~$180–$210~~ **$125**`
        // cell. This must produce THREE Text elements: one
        // strikethrough (the `~~…~~` span), the literal space
        // between the spans, and one bold (the `**…**` span).
        // The coalescer must not split the strikethrough into 3
        // events (`~~`, `$180–$210`, `~~`) or the bold into 3
        // (`**`, `$125`, `**`); different `TextStyle`s must
        // remain as separate elements (so the renderer can apply
        // distinct styling) but each style's own delimiters must
        // be absorbed.
        let strikethrough = TextStyle {
            strikethrough: true,
            ..TextStyle::default()
        };
        let cell41 = &table[4][1];
        assert_eq!(
            cell41.len(),
            3,
            "GL.iNet Price cell must be 3 elements (strikethrough + space + bold), got {cell41:?}"
        );
        assert!(matches!(
            &cell41[0],
            InlineElem::Text(t, s) if t == "$180–$210" && s == &strikethrough
        ));
        assert!(matches!(
            &cell41[1],
            InlineElem::Text(t, s) if t == " " && s == &plain
        ));
        assert!(matches!(
            &cell41[2],
            InlineElem::Text(t, s) if t == "$125" && s == &bold
        ));

        // Row 5 (ASUS RT-BE59): the `~$180` cell. Single tilde is
        // not a valid GFM strikethrough, so cmark should not emit
        // a Strikethrough Start/End pair — but it does still
        // fragment into `~` + `$180` events. The coalescer must
        // fold them back into a single plain Text element.
        let cell51 = &table[5][1];
        assert_eq!(
            cell51.len(),
            1,
            "ASUS RT-BE59 Price cell (single ~) must coalesce to 1 element, got {cell51:?}"
        );
        assert!(matches!(
            &cell51[0],
            InlineElem::Text(t, s) if t == "~$180" && s == &plain
        ));
    }

    #[test]
    fn parses_table_cell_with_link_and_middle_dot_and_bold_phone() {
        let md =
            "| Header |\n| --- |\n| [Link](https://example.com/shop/) \u{b7} **(555) 123-4567** |";
        let events = parse_markdown_to_events(md);
        let table = events
            .iter()
            .find_map(|e| {
                if let RenderEvent::Table(rows) = e {
                    Some(rows)
                } else {
                    None
                }
            })
            .expect("expected a Table event");

        assert_eq!(table.len(), 2, "header + 1 data row");
        assert_eq!(table[0].len(), 1, "1 column in header");
        assert_eq!(table[1].len(), 1, "1 column in data row");

        let cell = &table[1][0];
        eprintln!("Cell elements: {:?}", cell);

        // The cell should contain: Link("Link"), Text(" · "), Bold(Text("(555) 123-4567"))
        assert!(
            cell.len() >= 2,
            "cell should have at least 2 elements, got {:?}",
            cell
        );

        // Check the link
        assert!(matches!(
            &cell[0],
            InlineElem::Link(url, text)
                if url == "https://example.com/shop/"
                    && text == "Link"
        ));

        // Check the middle dot text (may be combined with space)
        let has_middle_dot = cell
            .iter()
            .any(|e| matches!(e, InlineElem::Text(t, _) if t.contains("\u{b7}")));
        assert!(
            has_middle_dot,
            "cell should contain middle dot, got {:?}",
            cell
        );

        // Check the bold phone number
        let bold = TextStyle {
            bold: true,
            ..TextStyle::default()
        };
        let has_bold_phone = cell.iter().any(|e| {
            matches!(
                e,
                InlineElem::Text(t, s)
                    if t.contains("555")
                        && t.contains("123")
                        && t.contains("4567")
                        && s == &bold
            )
        });
        assert!(
            has_bold_phone,
            "cell should contain bold phone number, got {:?}",
            cell
        );
    }

    // Regression: 5-column table where rows with
    // `[Link](url) · **(NNN) NNN-NNNN**` in the last column
    // must NOT produce an extra (6th) column.
    #[test]
    fn five_column_table_with_phone_in_last_cell_keeps_five_columns() {
        let md = r#"| Name | Score | Location | Description | Link |
|---|---|---|---|---|
| **Shop Alpha** | 86 | City A, ST | Trusted community favorite, honest, reliable | [Link](https://example.com/shop-alpha/) |
| **Shop Beta** | 10 | City B, ST | Family-owned, highly praised | [Link](https://example.com/shop-beta/) \u{b7} **(555) 111-2222** |
| **Shop Gamma** | 5 | 123 Main St, City C, ST 98000 | Honest, affordable, trustworthy | [Link](https://example.com/shop-gamma/) \u{b7} **(555) 333-4444** |
| **Shop Delta** | \u{2014} | City D, ST | Highly recommended, friendly service | [Link](https://example.com/shop-delta/) |
| **Shop Epsilon** | 10 | Area E, City F, ST | Long-established, certified | [Link](https://example.com/shop-epsilon/) |
| **Shop Zeta** | \u{2014} | 456 Oak Dr, City G, ST 98000 | Newer business | [Link](https://example.com/shop-zeta/) |
| **Shop Eta** | \u{2014} | 789 Pine Ave, City H, ST 98000 | Newer business | [Link](https://example.com/shop-eta/) |"#;
        let events = parse_markdown_to_events(md);
        let table = events
            .iter()
            .find_map(|e| {
                if let RenderEvent::Table(rows) = e {
                    Some(rows)
                } else {
                    None
                }
            })
            .expect("expected a Table event");

        assert_eq!(table.len(), 8, "header + 7 data rows");
        for (i, row) in table.iter().enumerate() {
            let label = [
                "header",
                "Shop Alpha",
                "Shop Beta",
                "Shop Gamma",
                "Shop Delta",
                "Shop Epsilon",
                "Shop Zeta",
                "Shop Eta",
            ];
            assert_eq!(
                row.len(),
                5,
                "row {} ({}) must have 5 columns, got {}: each cell={:?}",
                i,
                label[i],
                row.len(),
                row.iter()
                    .map(|c| format!("{} elems: first={:?}", c.len(), c.first()))
                    .collect::<Vec<_>>()
            );
        }

        // Verify Shop Beta last cell (row 2, col 4) has phone
        let beta_link = &table[2][4];
        assert!(
            beta_link
                .iter()
                .any(|e| matches!(e, InlineElem::Text(t, _) if t.contains("111-2222"))),
            "Shop Beta last cell must contain phone number"
        );

        // Verify Shop Gamma last cell (row 3, col 4) has phone
        let gamma_link = &table[3][4];
        assert!(
            gamma_link
                .iter()
                .any(|e| matches!(e, InlineElem::Text(t, _) if t.contains("333-4444"))),
            "Shop Gamma last cell must contain phone number"
        );
    }

    #[test]
    fn malformed_table_ragged_columns_still_parses_without_panic() {
        // Ragged `|` counts across rows must not panic the parser and
        // must still surface as a Table event. This is a regression
        // guard for a shape only touched implicitly by the random
        // proptest.
        let md = "| A | B |\n|---|---|\n| only one |\n| a | b | c |";
        let events = parse_markdown_to_events(md);
        let table = events
            .iter()
            .find_map(|e| {
                if let RenderEvent::Table(rows) = e {
                    Some(rows)
                } else {
                    None
                }
            })
            .expect("ragged table must still produce a Table event");
        // Header + 2 data rows.
        assert!(
            table.len() >= 3,
            "expected header + 2 data rows, got {}",
            table.len()
        );
    }

    #[test]
    fn malformed_table_empty_cells_and_only_pipes_parse_without_panic() {
        // An empty `| |` cell and a row that is only pipes (no text)
        // must not panic and should produce a Table event.
        let md = "| H |\n|---|\n| |\n|---|";
        let events = parse_markdown_to_events(md);
        // No panic is the primary assertion; a Table event is optional
        // depending on cmark's tolerance, so we only assert panic-freedom
        // plus that the output is well-formed events.
        let _ = events;
    }

    #[test]
    fn parse_yaml_to_pairs_non_string_and_nested_values() {
        // Non-string scalars (numbers) and nested mappings are coerced
        // to their serialized string form; keys are serialized scalars.
        let yaml: serde_norway::Value =
            serde_norway::from_str("count: 42\nnested:\n  a: b\nempty:\n").unwrap();
        let pairs = parse_yaml_to_pairs(&yaml).unwrap();
        let count = pairs
            .iter()
            .find(|(k, _)| k.trim_matches('"') == "count")
            .map(|(_, v)| v.clone());
        assert_eq!(count.as_deref(), Some("42"));
        let nested = pairs
            .iter()
            .find(|(k, _)| k.trim_matches('"') == "nested")
            .map(|(_, v)| v.clone());
        assert!(nested.is_some(), "nested mapping must be serialized");
    }

    #[test]
    fn parse_yaml_to_pairs_non_mapping_returns_none() {
        // A scalar or sequence root is not a mapping, so the function
        // returns None.
        let scalar: serde_norway::Value = serde_norway::from_str("just a string").unwrap();
        assert!(parse_yaml_to_pairs(&scalar).is_none());
    }

    #[test]
    fn parse_markdown_bare_url_becomes_link() {
        let md = "Check out https://github.com/fastmd for code.";
        let events = parse_markdown_to_events(md);
        let flush = events
            .iter()
            .find_map(|e| match e {
                RenderEvent::FlushInline { elems, .. } => Some(elems),
                _ => None,
            })
            .expect("must produce FlushInline");

        assert_eq!(flush.len(), 3);
        assert!(matches!(&flush[0], InlineElem::Text(t, _) if t == "Check out "));
        assert!(matches!(
            &flush[1],
            InlineElem::Link(url, text) if url == "https://github.com/fastmd" && text == "https://github.com/fastmd"
        ));
        assert!(matches!(&flush[2], InlineElem::Text(t, _) if t == " for code."));
    }

    #[test]
    fn parse_markdown_wikilink_becomes_link() {
        let md = "See [[Getting-Started|Quickstart]] guide.";
        let events = parse_markdown_to_events(md);
        let flush = events
            .iter()
            .find_map(|e| match e {
                RenderEvent::FlushInline { elems, .. } => Some(elems),
                _ => None,
            })
            .expect("must produce FlushInline");

        assert_eq!(flush.len(), 3);
        assert!(matches!(&flush[0], InlineElem::Text(t, _) if t == "See "));
        assert!(matches!(
            &flush[1],
            InlineElem::Link(url, text) if url == "wikilink:Getting-Started" && text == "Quickstart"
        ));
        assert!(matches!(&flush[2], InlineElem::Text(t, _) if t == " guide."));
    }

    #[test]
    fn parse_markdown_code_inside_link_coalesces() {
        let md = "[`run()`](https://example.com)";
        let events = parse_markdown_to_events(md);
        let flush = events
            .iter()
            .find_map(|e| match e {
                RenderEvent::FlushInline { elems, .. } => Some(elems),
                _ => None,
            })
            .expect("must produce FlushInline");

        assert_eq!(flush.len(), 1);
        assert!(matches!(
            &flush[0],
            InlineElem::Link(url, text) if url == "https://example.com" && text == "run()"
        ));
    }

    // ---------------------------------------------------------------
    //  Block-level structure: headings, code blocks, lists, rules
    // ---------------------------------------------------------------

    /// Snapshot of one `FlushInline` event, so list/task assertions can name
    /// each field instead of indexing a tuple.
    #[derive(Debug)]
    struct Flush {
        elems: Vec<InlineElem>,
        needs_bullet: bool,
        task_checked: Option<bool>,
        indent: usize,
        list_ordinal: Option<u64>,
    }

    /// Collect every `FlushInline` payload in emission order.
    fn flushes(events: &[RenderEvent]) -> Vec<Flush> {
        events
            .iter()
            .filter_map(|e| match e {
                RenderEvent::FlushInline {
                    elems,
                    needs_bullet,
                    task_checked,
                    indent,
                    list_ordinal,
                } => Some(Flush {
                    elems: elems.clone(),
                    needs_bullet: *needs_bullet,
                    task_checked: *task_checked,
                    indent: *indent,
                    list_ordinal: *list_ordinal,
                }),
                _ => None,
            })
            .collect()
    }

    /// Pull the single `CodeBlock` event out of a parse result.
    fn only_code_block(events: &[RenderEvent]) -> (Option<String>, String) {
        events
            .iter()
            .find_map(|e| match e {
                RenderEvent::CodeBlock { language, content } => {
                    Some((language.clone(), content.clone()))
                }
                _ => None,
            })
            .expect("must produce a CodeBlock event")
    }

    #[test]
    fn atx_headings_map_to_levels_one_through_six() {
        for level in 1u32..=6 {
            let md = format!("{} Title", "#".repeat(level as usize));
            let events = parse_markdown_to_events(&md);
            let heading = events
                .iter()
                .find_map(|e| match e {
                    RenderEvent::Heading { level, elems } => Some((*level, elems.clone())),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("H{level} must produce a Heading event for {md:?}"));
            assert_eq!(heading.0, level, "heading level for {md:?}");
            assert_eq!(
                heading.1.len(),
                1,
                "H{level} body must be a single coalesced text element"
            );
            assert_eq!(cell_text(&heading.1), "Title", "H{level} body text");
        }
    }

    /// A heading inside a link goes through the link coalescer on the heading
    /// buffer (`in_heading && in_link`), not the paragraph buffer.
    #[test]
    fn heading_containing_a_link_uses_the_link_coalescer() {
        let events = parse_markdown_to_events("# [Docs](https://example.com)");
        let heading = events
            .iter()
            .find_map(|e| match e {
                RenderEvent::Heading { level, elems } => Some((*level, elems.clone())),
                _ => None,
            })
            .expect("heading event");
        assert_eq!(heading.0, 1, "level");
        assert_eq!(
            heading.1,
            vec![InlineElem::Link(
                "https://example.com".to_string(),
                "Docs".to_string()
            )],
            "the link must land in the heading element list"
        );
    }

    /// A setext heading may span lines; the soft break between them is dropped
    /// (headings are single-line in the renderer) rather than emitted as a
    /// `SoftBreak` element.
    #[test]
    fn soft_break_inside_heading_is_dropped() {
        let events = parse_markdown_to_events("Title\nLine\n===\n");
        let elems = events
            .iter()
            .find_map(|e| match e {
                RenderEvent::Heading { level, elems } => {
                    assert_eq!(*level, 1, "a '===' underline is an H1");
                    Some(elems.clone())
                }
                _ => None,
            })
            .expect("setext heading event");
        assert!(
            !elems.iter().any(|e| matches!(e, InlineElem::SoftBreak)),
            "no SoftBreak element may appear inside a heading: {elems:?}"
        );
        let text = cell_text(&elems);
        assert!(
            text.contains("Title") && text.contains("Line"),
            "got {text:?}"
        );
    }

    #[test]
    fn fenced_code_block_keeps_language_and_content() {
        let md = "```rust\nfn main() {\n    let x = 1;\n}\n```\n";
        let (language, content) = only_code_block(&parse_markdown_to_events(md));
        assert_eq!(
            language.as_deref(),
            Some("rust"),
            "the fence info string becomes the language"
        );
        assert_eq!(
            content, "fn main() {\n    let x = 1;\n}\n",
            "code content keeps every line including its newline"
        );
    }

    #[test]
    fn fenced_code_block_without_info_has_no_language() {
        let (language, content) = only_code_block(&parse_markdown_to_events("```\nplain\n```\n"));
        assert_eq!(language, None, "an info-less fence has no language");
        assert_eq!(content, "plain\n", "content is preserved verbatim");
    }

    #[test]
    fn indented_code_block_has_no_language() {
        let (language, content) = only_code_block(&parse_markdown_to_events("    indented\n"));
        assert_eq!(language, None, "indented code has no info string");
        assert_eq!(content, "indented\n", "content is preserved verbatim");
    }

    /// The code-block buffer is filled by direct string append, never by the
    /// inline coalescers, so nothing inside a fence is ever link-scanned.
    #[test]
    fn code_block_content_is_never_link_scanned() {
        let md = "```\nhttps://example.com and [[Note]]\n```\n";
        let (language, content) = only_code_block(&parse_markdown_to_events(md));
        assert_eq!(language, None, "info-less fence");
        assert_eq!(
            content, "https://example.com and [[Note]]\n",
            "code content must never be link-scanned"
        );
    }

    #[test]
    fn task_list_markers_report_checked_state_without_a_bullet() {
        let cases = [
            ("- [x] done\n", true, "done"),
            ("- [ ] todo\n", false, "todo"),
        ];
        for (md, checked, want) in cases {
            let flushes = flushes(&parse_markdown_to_events(md));
            assert_eq!(flushes.len(), 1, "exactly one flush for {md:?}");
            let f = &flushes[0];
            assert_eq!(f.task_checked, Some(checked), "task_checked for {md:?}");
            assert!(
                !f.needs_bullet,
                "a task marker replaces the bullet for {md:?}"
            );
            assert_eq!(f.indent, 1, "indent for {md:?}");
            assert_eq!(
                f.list_ordinal, None,
                "unordered list has no ordinal for {md:?}"
            );
            assert_eq!(cell_text(&f.elems).trim(), want, "item text for {md:?}");
        }
    }

    #[test]
    fn ordered_list_items_get_increasing_ordinals() {
        let flushes = flushes(&parse_markdown_to_events("1. one\n2. two\n3. three\n"));
        assert_eq!(flushes.len(), 3, "one flush per item");
        for (i, f) in flushes.iter().enumerate() {
            assert_eq!(
                f.list_ordinal,
                Some((i + 1) as u64),
                "item {} must carry ordinal {}",
                i + 1,
                i + 1
            );
            assert!(f.needs_bullet, "item {} still needs its marker", i + 1);
            assert_eq!(f.task_checked, None, "item {} is not a task", i + 1);
            assert_eq!(f.indent, 1, "item {} indent", i + 1);
        }
        assert_eq!(
            cell_text(&flushes[2].elems).trim(),
            "three",
            "third item text"
        );
    }

    /// An ordered list that does not start at 1 keeps the author's start value
    /// and counts up from there.
    #[test]
    fn ordered_list_starting_offset_is_preserved() {
        let flushes = flushes(&parse_markdown_to_events("7. seven\n8. eight\n"));
        assert_eq!(flushes.len(), 2, "one flush per item");
        assert_eq!(flushes[0].list_ordinal, Some(7), "first ordinal");
        assert_eq!(flushes[1].list_ordinal, Some(8), "second ordinal");
    }

    #[test]
    fn nested_lists_increase_indent_and_flush_the_parent_item() {
        let flushes = flushes(&parse_markdown_to_events("- a\n  - b\n"));
        assert_eq!(
            flushes.len(),
            2,
            "parent item flushes before the nested list"
        );
        assert_eq!(cell_text(&flushes[0].elems).trim(), "a", "parent text");
        assert_eq!(flushes[0].indent, 1, "parent indent");
        assert_eq!(cell_text(&flushes[1].elems).trim(), "b", "child text");
        assert_eq!(flushes[1].indent, 2, "child indent");
    }

    #[test]
    fn thematic_break_becomes_a_separator() {
        let events = parse_markdown_to_events("before\n\n---\n\nafter\n");
        let separators = events
            .iter()
            .filter(|e| matches!(e, RenderEvent::Separator))
            .count();
        assert_eq!(
            separators, 1,
            "one '---' must produce exactly one Separator"
        );
        let texts: Vec<String> = flushes(&events)
            .iter()
            .map(|f| cell_text(&f.elems))
            .collect();
        assert_eq!(texts, vec!["before", "after"], "text around the rule");
    }

    #[test]
    fn image_becomes_an_image_element() {
        let flushes = flushes(&parse_markdown_to_events("![alt text](pic.png)"));
        assert_eq!(flushes.len(), 1, "one flush");
        assert!(
            matches!(&flushes[0].elems[0], InlineElem::Image(url) if url == "pic.png"),
            "the destination must become InlineElem::Image: {:?}",
            flushes[0].elems
        );
        assert!(
            cell_text(&flushes[0].elems).contains("alt text"),
            "the alt text stays visible: {:?}",
            flushes[0].elems
        );
    }

    /// `<a@b.c>` is parsed as an email autolink whose destination has no
    /// `mailto:` scheme, so the parser has to add it.
    #[test]
    fn email_autolink_gets_a_mailto_prefix() {
        let flushes = flushes(&parse_markdown_to_events("Mail <mkuhn@example.com> please"));
        assert_eq!(flushes.len(), 1, "one flush");
        assert_eq!(
            flushes[0].elems,
            vec![
                InlineElem::Text("Mail ".to_string(), TextStyle::default()),
                InlineElem::Link(
                    "mailto:mkuhn@example.com".to_string(),
                    "mkuhn@example.com".to_string()
                ),
                InlineElem::Text(" please".to_string(), TextStyle::default()),
            ],
            "the bare address must be prefixed with mailto:"
        );
    }

    #[test]
    fn html_block_and_inline_html_become_html_elements() {
        let block = flushes(&parse_markdown_to_events("<div>\nhello\n</div>\n"));
        let block_html: String = html_of(&block).concat();
        assert!(
            block_html.contains("<div>") && block_html.contains("hello"),
            "block HTML must survive as Html elements, got {block_html:?}"
        );

        let inline = flushes(&parse_markdown_to_events("a <b>bold</b> c"));
        let bold_tags = html_of(&inline).contains(&"<b>");
        assert!(bold_tags, "inline HTML must survive as an Html element");
        let joined: String = inline.iter().map(|f| cell_text(&f.elems)).collect();
        assert_eq!(
            joined.trim(),
            "a <b>bold</b> c",
            "the inline text must stay in order"
        );
    }

    /// Gather the raw HTML fragments of every flush so HTML assertions do not
    /// have to re-walk the element tree.
    fn html_of(flushes: &[Flush]) -> Vec<&str> {
        let mut out = Vec::new();
        for f in flushes {
            for e in &f.elems {
                if let InlineElem::Html(h) = e {
                    out.push(h.as_str());
                }
            }
        }
        out
    }

    /// A hard break (two trailing spaces) outside a table ends the inline run,
    /// so the renderer starts a new line block instead of a soft break.
    #[test]
    fn hard_break_outside_table_flushes_the_inline_run() {
        let events = parse_markdown_to_events("line one  \nline two\n");
        let flushes = flushes(&events);
        assert_eq!(
            flushes.len(),
            2,
            "a hard break must split the paragraph into two flushes"
        );
        assert_eq!(cell_text(&flushes[0].elems), "line one", "first line");
        assert_eq!(cell_text(&flushes[1].elems), "line two", "second line");
        assert!(
            !flushes[0]
                .elems
                .iter()
                .any(|e| matches!(e, InlineElem::SoftBreak)),
            "the break must not be inlined as a SoftBreak: {:?}",
            flushes[0].elems
        );
    }

    #[test]
    fn footnote_reference_and_definition_are_marked_up() {
        let events = parse_markdown_to_events("Ref[^1] here\n\n[^1]: the note\n");
        assert!(
            events.iter().any(|e| matches!(e, RenderEvent::Separator)),
            "a footnote definition must start with a Separator"
        );
        let texts: Vec<(String, TextStyle)> = text_of(&flushes(&events));
        let debug = format!("{texts:?}");
        assert!(
            texts.iter().any(|(t, s)| t == "[^1]" && s.code),
            "the reference must render as a code-styled [^1] marker: {debug}"
        );
        assert!(
            texts.iter().any(|(t, s)| t == "[^1]: " && s.bold),
            "the definition must render a bold [^1]: label: {debug}"
        );
        let body = flushes(&events)
            .iter()
            .any(|f| cell_text(&f.elems) == "the note");
        assert!(body, "the definition body must be rendered");
    }

    /// Gather the `(text, style)` pairs of every flush.
    fn text_of(flushes: &[Flush]) -> Vec<(String, TextStyle)> {
        let mut out = Vec::new();
        for f in flushes {
            for e in &f.elems {
                if let InlineElem::Text(t, s) = e {
                    out.push((t.clone(), s.clone()));
                }
            }
        }
        out
    }

    #[test]
    fn gfm_options_are_the_documented_feature_set() {
        // The renderer and the editor's pre-save validation ([UI-043]) must
        // parse with the same option set; pinning the exact set here is what
        // stops the two from drifting apart silently.
        use pulldown_cmark::Options;
        let expected = Options::ENABLE_TABLES
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS;
        assert_eq!(gfm_options(), expected, "GFM option set drifted");
    }

    // ---------------------------------------------------------------
    //  push_scanned_text
    // ---------------------------------------------------------------

    #[test]
    fn push_scanned_text_appends_to_an_empty_buffer() {
        let style = TextStyle::default();
        let mut buf: Vec<InlineElem> = Vec::new();
        push_scanned_text(&mut buf, "hello", &style);
        assert_eq!(buf, vec![InlineElem::Text("hello".to_string(), style)]);
    }

    /// The point of the combine: a URL split across two `Text` events is only
    /// recognised because the previous same-style run is re-scanned as one.
    #[test]
    fn push_scanned_text_rescans_a_same_style_run_so_split_urls_are_found() {
        let style = TextStyle::default();
        let mut buf: Vec<InlineElem> = Vec::new();
        push_scanned_text(&mut buf, "visit www", &style);
        assert_eq!(
            buf,
            vec![InlineElem::Text("visit www".to_string(), style.clone())],
            "the first fragment alone is not a URL"
        );
        push_scanned_text(&mut buf, ".example.com", &style);
        assert_eq!(
            buf,
            vec![
                InlineElem::Text("visit ".to_string(), style.clone()),
                InlineElem::Link(
                    "https://www.example.com".to_string(),
                    "www.example.com".to_string()
                ),
            ],
            "the combined run must expose the URL that neither fragment contains"
        );
    }

    #[test]
    fn push_scanned_text_does_not_combine_across_styles() {
        let bold = TextStyle {
            bold: true,
            ..Default::default()
        };
        let plain = TextStyle::default();
        let mut buf = vec![InlineElem::Text("hi".to_string(), bold.clone())];
        push_scanned_text(&mut buf, "plain", &plain);
        assert_eq!(
            buf,
            vec![
                InlineElem::Text("hi".to_string(), bold),
                InlineElem::Text("plain".to_string(), plain),
            ],
            "a style change must start a new element"
        );
    }

    #[test]
    fn push_scanned_text_starts_a_new_element_after_a_non_text_element() {
        let style = TextStyle::default();
        let mut buf = vec![InlineElem::Image("x.png".to_string())];
        push_scanned_text(&mut buf, "after", &style);
        assert_eq!(
            buf,
            vec![
                InlineElem::Image("x.png".to_string()),
                InlineElem::Text("after".to_string(), style),
            ],
            "an image must not absorb following text"
        );
    }

    /// Code-styled text is never link-scanned, so a URL inside inline code
    /// stays literal.
    #[test]
    fn push_scanned_text_leaves_code_styled_text_unscanned() {
        let code = TextStyle {
            code: true,
            ..Default::default()
        };
        let mut buf: Vec<InlineElem> = Vec::new();
        push_scanned_text(&mut buf, "https://example.com", &code);
        assert_eq!(
            buf,
            vec![InlineElem::Text("https://example.com".to_string(), code)],
            "code-styled text must not become a link"
        );
    }

    // ---------------------------------------------------------------
    //  parse_yaml_to_pairs
    // ---------------------------------------------------------------

    /// Front-matter sequences are the one shape users write most often after
    /// scalars: string items are joined, non-strings are serialised first.
    #[test]
    fn parse_yaml_to_pairs_joins_sequence_values() {
        let yaml: serde_norway::Value =
            serde_norway::from_str("tags:\n  - alpha\n  - beta\ncounts:\n  - 1\n  - true\n")
                .unwrap();
        let pairs = parse_yaml_to_pairs(&yaml).expect("mapping");
        let find = |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| k.trim_matches('"') == key)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(find("tags").as_deref(), Some("alpha, beta"), "string items");
        assert_eq!(
            find("counts").as_deref(),
            Some("1, true"),
            "non-string items"
        );
    }

    #[test]
    fn parse_yaml_to_pairs_empty_mapping_is_some() {
        let yaml: serde_norway::Value = serde_norway::from_str("{}").unwrap();
        let pairs = parse_yaml_to_pairs(&yaml).expect("an empty mapping is still a mapping");
        assert!(pairs.is_empty(), "no pairs for an empty mapping");
    }
}

#[cfg(test)]
#[path = "parser_proptests.rs"]
mod parser_proptests;
