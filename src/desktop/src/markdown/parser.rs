//! Pure Markdown and YAML parsing logic isolated from egui rendering.

use crate::markdown::ast::{InlineElem, RenderEvent, TextStyle};

/// Renders markdown source to an HTML string.
pub fn render_markdown_to_html(markdown: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let parser = Parser::new_ext(markdown, Options::all());
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// Parses markdown text into a sequence of render events.
pub fn parse_markdown_to_events(markdown_text: &str) -> Vec<RenderEvent> {
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
    let mut blockquote_depth: usize = 0;

    let push_inline = |events: &mut Vec<RenderEvent>,
                       elems: &mut Vec<InlineElem>,
                       bullet: &mut bool,
                       task: &mut Option<bool>,
                       indent: usize,
                       list_ordinal: Option<u64>,
                       bq_depth: usize| {
        if elems.is_empty() && !*bullet && task.is_none() {
            return;
        }
        events.push(RenderEvent::FlushInline {
            elems: elems.clone(),
            needs_bullet: *bullet,
            task_checked: *task,
            indent,
            list_ordinal,
            blockquote_depth: bq_depth,
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
                        blockquote_depth,
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
                        blockquote_depth,
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
                        blockquote_depth,
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
                        blockquote_depth,
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
                        blockquote_depth,
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
                    blockquote_depth,
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
                        blockquote_depth,
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
                    blockquote_depth,
                );
                if let Some(Some(n)) = list_ordinal_stack.last_mut() {
                    *n += 1;
                }
            }
            Event::Start(Tag::BlockQuote) => {
                if !buffered_inline.is_empty() {
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        list_ordinal_stack.last().copied().flatten(),
                        blockquote_depth,
                    );
                }
                blockquote_depth += 1;
            }
            Event::End(TagEnd::BlockQuote) => {
                push_inline(
                    &mut events,
                    &mut buffered_inline,
                    &mut needs_bullet,
                    &mut task_checked,
                    list_depth,
                    list_ordinal_stack.last().copied().flatten(),
                    blockquote_depth,
                );
                blockquote_depth = blockquote_depth.saturating_sub(1);
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
                        blockquote_depth,
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
                        blockquote_depth,
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
                        heading_elems.push(InlineElem::Link(link_url.clone(), text.to_string()));
                    } else {
                        buffered_inline.push(InlineElem::Link(link_url.clone(), text.to_string()));
                    }
                } else if in_heading {
                    heading_elems.push(InlineElem::Text(text.to_string(), current_style.clone()));
                } else {
                    buffered_inline.push(InlineElem::Text(text.to_string(), current_style.clone()));
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
                            blockquote_depth,
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
                    blockquote_depth,
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
                        blockquote_depth,
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
                    blockquote_depth,
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
                        blockquote_depth,
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
                    blockquote_depth,
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
        blockquote_depth,
    );

    events
}

/// Parses a YAML mapping into a list of key-value string pairs.
pub fn parse_yaml_to_pairs(yaml: &serde_yaml::Value) -> Option<Vec<(String, String)>> {
    let mapping = yaml.as_mapping()?;
    let mut pairs = Vec::new();
    for (key, value) in mapping {
        if let Some(key_str) = key.as_str() {
            let val_str = match value {
                serde_yaml::Value::String(s) => s.clone(),
                serde_yaml::Value::Sequence(seq) => {
                    let items: Vec<String> = seq
                        .iter()
                        .map(|v| match v {
                            serde_yaml::Value::String(s) => s.clone(),
                            _ => serde_yaml::to_string(v)
                                .unwrap_or_default()
                                .trim()
                                .to_string(),
                        })
                        .collect();
                    items.join(", ")
                }
                _ => serde_yaml::to_string(value)
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            };
            pairs.push((key_str.to_string(), val_str));
        }
    }
    Some(pairs)
}
