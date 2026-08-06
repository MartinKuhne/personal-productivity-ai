//! Pure Markdown and YAML parsing logic isolated from egui rendering.

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
/// `InlineElem::Text` when the style matches. Pulldown-cmark's inline
/// parser builds a tree of items where each delimiter run (`*`, `_`,
/// `~`, `=`, `^`) creates a separate `Text` item for the text on
/// either side, even when the delimiter fails to form a valid
/// emphasis/strikethrough pair. Without coalescing, plain content
/// like `~4,031 / ~19,000` becomes four `InlineElem::Text` entries
/// instead of one. This helper folds consecutive same-style text
/// fragments back into a single element while preserving style
/// transitions (different `TextStyle`s still produce separate
/// elements, as do `Link` / `Code` / `Html` / `SoftBreak` /
/// `Image` boundaries).
fn push_text_coalesce(buffer: &mut Vec<InlineElem>, text: &str, style: &TextStyle) {
    if let Some(InlineElem::Text(existing, existing_style)) = buffer.last_mut()
        && existing_style == style
    {
        existing.push_str(text);
        return;
    }
    buffer.push(InlineElem::Text(text.to_string(), style.clone()));
}

/// Like [`push_text_coalesce`] but for `Link` elements. Multiple
/// consecutive `Text` events inside a single `[…](url)` link would
/// otherwise become multiple `Link(url, …)` elements with the same
/// URL but disjoint text fragments. Folds them into one.
fn push_link_coalesce(buffer: &mut Vec<InlineElem>, url: &str, text: &str) {
    if let Some(InlineElem::Link(existing_url, existing_text)) = buffer.last_mut()
        && existing_url == url
    {
        existing_text.push_str(text);
        return;
    }
    buffer.push(InlineElem::Link(url.to_string(), text.to_string()));
}

/// Parses markdown text into a sequence of render events.
pub fn parse_markdown_to_events(markdown_text: &str) -> Vec<RenderEvent> {
    #[cfg(feature = "profiling")]
    puffin::profile_scope!("parse_markdown_to_events");

    use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown_text, options);
    let mut events = Vec::new();

    let mut in_code_block = false;
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

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(_info)) => {
                if !buffered_inline.is_empty() {
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        list_ordinal_stack.last().copied().flatten(),
                    );
                }
                in_code_block = true;
                code_block_content.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                if in_code_block {
                    in_code_block = false;
                    events.push(RenderEvent::CodeBlock(code_block_content.clone()));
                }
            }
            Event::Start(Tag::Heading { level, .. }) => {
                if !buffered_inline.is_empty() {
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        list_ordinal_stack.last().copied().flatten(),
                    );
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
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        list_ordinal_stack.last().copied().flatten(),
                    );
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if !in_table_cell {
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        list_ordinal_stack.last().copied().flatten(),
                    );
                    events.push(RenderEvent::Space(4.0));
                }
            }
            Event::Start(Tag::List(list_kind)) => {
                if !buffered_inline.is_empty() {
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        list_ordinal_stack.last().copied().flatten(),
                    );
                }
                list_depth += 1;
                list_ordinal_stack.push(list_kind);
            }
            Event::End(TagEnd::List(_)) => {
                push_inline(
                    &mut events,
                    &mut buffered_inline,
                    &mut needs_bullet,
                    &mut task_checked,
                    list_depth,
                    list_ordinal_stack.last().copied().flatten(),
                );
                list_depth = list_depth.saturating_sub(1);
                list_ordinal_stack.pop();
            }
            Event::Start(Tag::Item) => {
                if !buffered_inline.is_empty() {
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        list_ordinal_stack.last().copied().flatten(),
                    );
                }
                needs_bullet = true;
            }
            Event::End(TagEnd::Item) => {
                push_inline(
                    &mut events,
                    &mut buffered_inline,
                    &mut needs_bullet,
                    &mut task_checked,
                    list_depth,
                    list_ordinal_stack.last().copied().flatten(),
                );
                if let Some(Some(n)) = list_ordinal_stack.last_mut() {
                    *n += 1;
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                if !buffered_inline.is_empty() {
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        list_ordinal_stack.last().copied().flatten(),
                    );
                }
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                push_inline(
                    &mut events,
                    &mut buffered_inline,
                    &mut needs_bullet,
                    &mut task_checked,
                    list_depth,
                    list_ordinal_stack.last().copied().flatten(),
                );
            }
            Event::Start(Tag::Table(_)) => {
                if !buffered_inline.is_empty() {
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        list_ordinal_stack.last().copied().flatten(),
                    );
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
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        list_ordinal_stack.last().copied().flatten(),
                    );
                }
            }
            Event::End(TagEnd::TableCell) => {
                in_table_cell = false;
                current_row.push(buffered_inline.clone());
                buffered_inline.clear();
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                in_link = true;
                link_url = dest_url.to_string();
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
                    push_text_coalesce(&mut heading_elems, &text, &current_style);
                } else {
                    push_text_coalesce(&mut buffered_inline, &text, &current_style);
                }
            }
            Event::Code(code) => {
                if in_code_block {
                    code_block_content.push_str(&code);
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
                        push_inline(
                            &mut events,
                            &mut buffered_inline,
                            &mut needs_bullet,
                            &mut task_checked,
                            list_depth,
                            list_ordinal_stack.last().copied().flatten(),
                        );
                    } else {
                        buffered_inline.push(InlineElem::SoftBreak);
                    }
                }
            }
            Event::Rule => {
                push_inline(
                    &mut events,
                    &mut buffered_inline,
                    &mut needs_bullet,
                    &mut task_checked,
                    list_depth,
                    list_ordinal_stack.last().copied().flatten(),
                );
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
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        list_ordinal_stack.last().copied().flatten(),
                    );
                }
                events.push(RenderEvent::Separator);
                let text = format!("[^{}]: ", name);
                let mut s = current_style.clone();
                s.bold = true;
                buffered_inline.push(InlineElem::Text(text, s));
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                push_inline(
                    &mut events,
                    &mut buffered_inline,
                    &mut needs_bullet,
                    &mut task_checked,
                    list_depth,
                    list_ordinal_stack.last().copied().flatten(),
                );
            }
            Event::Start(Tag::HtmlBlock) => {
                if !buffered_inline.is_empty() {
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        list_ordinal_stack.last().copied().flatten(),
                    );
                }
            }
            Event::End(TagEnd::HtmlBlock) => {
                push_inline(
                    &mut events,
                    &mut buffered_inline,
                    &mut needs_bullet,
                    &mut task_checked,
                    list_depth,
                    list_ordinal_stack.last().copied().flatten(),
                );
            }
            _ => {}
        }
    }
    push_inline(
        &mut events,
        &mut buffered_inline,
        &mut needs_bullet,
        &mut task_checked,
        list_depth,
        list_ordinal_stack.last().copied().flatten(),
    );

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

    /// **Pulldown-cmark 0.13.4 inline-parser design note** (not a
    /// bug per se, but a property the consumer has to handle):
    /// with `Options::ENABLE_STRIKETHROUGH` enabled, cmark emits
    /// one `Text` event per delimiter run — so the input
    /// `~4,031 / ~19,000` becomes 4 separate `Text` events
    /// (one for each `~`, one for the text between) even though
    /// no actual `Strikethrough` Start/End pair fires. This is
    /// the inline parser's "tree of items" model: every delimiter
    /// run gets its own item, whether or not it forms a valid
    /// pair.
    ///
    /// Our parser works around it by coalescing consecutive
    /// `Text` events with the same `TextStyle` (see
    /// `push_text_coalesce` in `parse.rs`), so the AST emitted
    /// to the rest of the renderer is a single `InlineElem::Text`
    /// per coalesced run. This test pins the cmark behavior so
    /// future upgrades to pulldown-cmark (or changes to the
    /// option set) that change the fragment count don't silently
    /// regress the coalescing.
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
        let mut s_bold = TextStyle::default();
        s_bold.bold = true;
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
}
