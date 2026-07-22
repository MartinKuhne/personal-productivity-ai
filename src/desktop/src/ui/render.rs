//! Pulldown-cmark event-driven markdown renderer — emits egui widgets for headings, paragraphs, code blocks, lists, tables, links, and images.

use eframe::egui;
use egui::RichText;

#[derive(Clone, Debug, PartialEq)]
pub enum InlineElem {
    Text(String, TextStyle),
    Link(String, String),
    Image(String),
    Html(String),
    SoftBreak,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strikethrough: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RenderEvent {
    FlushInline {
        elems: Vec<InlineElem>,
        needs_bullet: bool,
        task_checked: Option<bool>,
        indent: usize,
        wrap: bool,
    },
    CodeBlock(String),
    Heading {
        level: u32,
        text: String,
    },
    Table(Vec<Vec<Vec<InlineElem>>>),
    Space(f32),
    Separator,
}

/// Purpose: Renders inline markdown elements.
/// Inputs: `ui` (mut), `elems`, `needs_bullet`, `task_checked`, `indent`, `wrap`
/// Outputs: None
/// Purity: Impure (modifies UI state). Thin adapter for rendering text.
fn render_inline(
    ui: &mut egui::Ui,
    elems: &[InlineElem],
    needs_bullet: bool,
    task_checked: Option<bool>,
    indent: usize,
    wrap: bool,
) {
    if elems.is_empty() && !needs_bullet && task_checked.is_none() {
        return;
    }

    let add_content = |ui: &mut egui::Ui| {
        ui.spacing_mut().item_spacing.x = 0.0;

        if indent > 0 {
            ui.add_space(indent as f32 * 20.0);
        }
        if needs_bullet {
            ui.label(RichText::new("• ").size(14.0));
        }
        if let Some(checked) = task_checked {
            ui.add_space(4.0);
            let mut c = checked;
            ui.checkbox(&mut c, "");
            ui.add_space(4.0);
        }

        for elem in elems {
            match elem {
                InlineElem::Text(t, style) => {
                    let mut rt = RichText::new(t);
                    if style.bold {
                        rt = rt.strong();
                    }
                    if style.italic {
                        rt = rt.italics();
                    }
                    if style.code {
                        rt = rt
                            .monospace()
                            .background_color(egui::Color32::from_gray(40));
                    }
                    if style.strikethrough {
                        rt = rt.strikethrough();
                    }
                    ui.label(rt);
                }
                InlineElem::Link(url, text) => {
                    ui.hyperlink_to(text, url);
                }
                InlineElem::Image(url) => {
                    ui.label(format!("[Image: {}]", url));
                }
                InlineElem::Html(html) => {
                    ui.label(RichText::new(html).italics().color(egui::Color32::GRAY));
                }
                InlineElem::SoftBreak => {
                    ui.label(" ");
                }
            }
        }
    };

    if wrap {
        ui.horizontal_wrapped(add_content);
    } else {
        ui.horizontal(add_content);
    }
}

/// Purpose: Renders a code block.
/// Inputs: `ui` (mut), `content`, `_idx` (mut)
/// Outputs: None
/// Purity: Impure (modifies UI state). Thin adapter.
fn render_code_block(ui: &mut egui::Ui, content: &str, _idx: &mut usize) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(20, 20, 22))
        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_gray(40)))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.add(egui::Label::new(RichText::new(content).monospace()).wrap(true));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    if ui.button("📋").on_hover_text("Copy code").clicked() {
                        ui.output_mut(|o| o.copied_text = content.to_string());
                    }
                });
            });
        });
}

/// Purpose: Renders a heading.
/// Inputs: `ui` (mut), `title`, `level`, `scroll_to_id` (mut)
/// Outputs: None
/// Purity: Impure (modifies UI state). Thin adapter.
fn render_heading(ui: &mut egui::Ui, title: &str, level: u32, scroll_to_id: &mut Option<egui::Id>) {
    let trimmed = title.trim().to_string();
    if !trimmed.is_empty() {
        let heading_id = egui::Id::new(&trimmed);
        if *scroll_to_id == Some(heading_id) {
            ui.scroll_to_rect(ui.max_rect(), None);
            *scroll_to_id = None;
        }
        let size = match level {
            1 => 32.0,
            2 => 24.0,
            3 => 18.0,
            4 => 14.0,
            _ => 12.0,
        };
        ui.heading(RichText::new(trimmed).size(size).strong());
        ui.add_space(4.0);
    }
}

/// Purpose: Renders a table.
/// Inputs: `ui` (mut), `table_cells`
/// Outputs: None
/// Purity: Impure (modifies UI state). Thin adapter.
fn render_table(ui: &mut egui::Ui, table_cells: &[Vec<Vec<InlineElem>>]) {
    egui::ScrollArea::horizontal()
        .id_source(ui.next_auto_id())
        .show(ui, |ui| {
            egui::Grid::new(ui.next_auto_id())
                .striped(true)
                .spacing([10.0, 4.0])
                .show(ui, |ui| {
                    for row in table_cells {
                        for cell in row {
                            render_inline(ui, cell, false, None, 0, false);
                        }
                        ui.end_row();
                    }
                });
        });
}

/// Purpose: Parses a YAML mapping into a list of key-value string pairs.
/// Inputs: `yaml`
/// Outputs: List of (String, String) if valid mapping, else None.
/// Purity: Pure function.
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
                        .map(|v| v.as_str().unwrap_or("").to_string())
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

/// Purpose: Renders a YAML table UI from a parsed mapping.
/// Inputs: `ui` (mut), `yaml`
/// Outputs: None
/// Purity: Impure (modifies UI state). Coordinates parsing and rendering.
pub fn render_yaml_table(ui: &mut egui::Ui, yaml: &serde_yaml::Value) {
    if let Some(pairs) = parse_yaml_to_pairs(yaml) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 24, 27))
            .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_gray(40)))
            .inner_margin(8.0)
            .rounding(4.0)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .id_source("yaml_scroll")
                    .show(ui, |ui| {
                        egui::Grid::new("yaml_grid")
                            .num_columns(2)
                            .striped(true)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                for (k, v) in pairs {
                                    ui.label(
                                        RichText::new(k)
                                            .strong()
                                            .color(egui::Color32::from_rgb(150, 200, 255)),
                                    );
                                    ui.label(RichText::new(v).color(egui::Color32::from_gray(220)));
                                    ui.end_row();
                                }
                            });
                    });
            });
        ui.add_space(8.0);
    }
}

/// Purpose: Parses markdown text into a sequence of render events.
/// Inputs: `markdown_text` (&str)
/// Outputs: `Vec<RenderEvent>` representing the logical blocks to draw.
/// Purity: Pure function.
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
    let mut heading_text = String::new();

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

    let push_inline = |events: &mut Vec<RenderEvent>,
                       elems: &mut Vec<InlineElem>,
                       bullet: &mut bool,
                       task: &mut Option<bool>,
                       indent: usize,
                       wrap: bool| {
        if elems.is_empty() && !*bullet && task.is_none() {
            return;
        }
        events.push(RenderEvent::FlushInline {
            elems: elems.clone(),
            needs_bullet: *bullet,
            task_checked: *task,
            indent,
            wrap,
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
                        true,
                    );
                }
                in_code_block = true;
                code_block_content.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                events.push(RenderEvent::CodeBlock(code_block_content.clone()));
            }
            Event::Start(Tag::Heading { level, .. }) => {
                if !buffered_inline.is_empty() {
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        true,
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
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                events.push(RenderEvent::Heading {
                    level: heading_level,
                    text: heading_text.clone(),
                });
                in_heading = false;
                heading_level = 0;
            }
            Event::Start(Tag::Paragraph) => {
                if !in_table_cell {
                    if !buffered_inline.is_empty() {
                        push_inline(
                            &mut events,
                            &mut buffered_inline,
                            &mut needs_bullet,
                            &mut task_checked,
                            list_depth,
                            true,
                        );
                    }
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
                        true,
                    );
                    events.push(RenderEvent::Space(4.0));
                }
            }
            Event::Start(Tag::List(_)) => {
                if !buffered_inline.is_empty() {
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        true,
                    );
                }
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                push_inline(
                    &mut events,
                    &mut buffered_inline,
                    &mut needs_bullet,
                    &mut task_checked,
                    list_depth,
                    true,
                );
                if list_depth > 0 {
                    list_depth -= 1;
                }
            }
            Event::Start(Tag::Item) => {
                if !buffered_inline.is_empty() {
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        true,
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
                    true,
                );
            }
            Event::Start(Tag::BlockQuote) => {
                if !buffered_inline.is_empty() {
                    push_inline(
                        &mut events,
                        &mut buffered_inline,
                        &mut needs_bullet,
                        &mut task_checked,
                        list_depth,
                        true,
                    );
                }
            }
            Event::End(TagEnd::BlockQuote) => {
                push_inline(
                    &mut events,
                    &mut buffered_inline,
                    &mut needs_bullet,
                    &mut task_checked,
                    list_depth,
                    true,
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
                        true,
                    );
                }
                table_cells.clear();
            }
            Event::End(TagEnd::Table) => {
                events.push(RenderEvent::Table(table_cells.clone()));
                table_cells.clear();
                events.push(RenderEvent::Space(4.0));
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
                buffered_inline.clear();
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
                } else if in_heading {
                    heading_text.push_str(&text);
                } else if in_link {
                    buffered_inline.push(InlineElem::Link(link_url.clone(), text.to_string()));
                } else {
                    buffered_inline.push(InlineElem::Text(text.to_string(), current_style.clone()));
                }
            }
            Event::Code(code) => {
                if in_code_block {
                    code_block_content.push_str(&code);
                } else if in_heading {
                    heading_text.push_str(&code);
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
                            true,
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
                    true,
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
                        true,
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
                    true,
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
                        true,
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
                    true,
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
        true,
    );

    events
}

/// Purpose: Renders markdown text to UI.
/// Inputs: `ui` (mut), `markdown_text`, `scroll_to_id` (mut)
/// Outputs: None
/// Purity: Impure (modifies UI state). Coordinates parsing and rendering.
pub fn render_markdown(
    ui: &mut egui::Ui,
    markdown_text: &str,
    scroll_to_id: &mut Option<egui::Id>,
) {
    let events = parse_markdown_to_events(markdown_text);
    let mut code_block_idx = 0;

    for event in events {
        match event {
            RenderEvent::FlushInline {
                elems,
                needs_bullet,
                task_checked,
                indent,
                wrap,
            } => {
                render_inline(ui, &elems, needs_bullet, task_checked, indent, wrap);
            }
            RenderEvent::CodeBlock(content) => {
                render_code_block(ui, &content, &mut code_block_idx);
            }
            RenderEvent::Heading { level, text } => {
                render_heading(ui, &text, level, scroll_to_id);
            }
            RenderEvent::Table(cells) => {
                render_table(ui, &cells);
            }
            RenderEvent::Space(amount) => {
                ui.add_space(amount);
            }
            RenderEvent::Separator => {
                ui.separator();
            }
        }
    }
}

/// Purpose: Builds a Table of Contents from markdown.
/// Inputs: `markdown_text`
/// Outputs: List of `ToCEntry` elements.
/// Purity: Pure function.
pub fn build_toc(markdown_text: &str) -> Vec<crate::ui::ToCEntry> {
    use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(markdown_text, options);

    let mut toc = Vec::new();
    let mut current_header = String::new();
    let mut heading_level = None;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let lvl = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                heading_level = Some(lvl);
                current_header.clear();
            }
            Event::Text(text) => {
                if heading_level.is_some() {
                    current_header.push_str(&text);
                }
            }
            Event::Code(code) => {
                if heading_level.is_some() {
                    current_header.push_str(&code);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(lvl) = heading_level.take() {
                    let title = current_header.trim().to_string();
                    if !title.is_empty() {
                        let id = egui::Id::new(&title);
                        toc.push(super::ToCEntry {
                            title,
                            level: lvl,
                            id,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    toc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yaml_to_pairs() {
        let yaml_str = "key1: value1\nkey2: [item1, item2]\nkey3: 100\nkey4: true";
        let val: serde_yaml::Value = serde_yaml::from_str(yaml_str).unwrap();
        let pairs = parse_yaml_to_pairs(&val).unwrap();
        assert_eq!(pairs[0], ("key1".to_string(), "value1".to_string()));
        assert_eq!(pairs[1], ("key2".to_string(), "item1, item2".to_string()));
        assert_eq!(pairs[2], ("key3".to_string(), "100".to_string()));
        assert_eq!(pairs[3], ("key4".to_string(), "true".to_string()));
    }

    #[test]
    fn test_parse_yaml_to_pairs_non_mapping() {
        let string_val = serde_yaml::Value::String("just string".to_string());
        assert_eq!(parse_yaml_to_pairs(&string_val), None);

        let seq_val =
            serde_yaml::Value::Sequence(vec![serde_yaml::Value::String("item".to_string())]);
        assert_eq!(parse_yaml_to_pairs(&seq_val), None);

        let null_val = serde_yaml::Value::Null;
        assert_eq!(parse_yaml_to_pairs(&null_val), None);
    }

    #[test]
    fn test_parse_markdown_to_events() {
        let md = "# Heading 1\nSome *text*\n- List item";
        let events = parse_markdown_to_events(md);

        assert_eq!(
            events[0],
            RenderEvent::Heading {
                level: 1,
                text: "Heading 1".to_string()
            }
        );

        // Next is the paragraph "Some text"
        match &events[1] {
            RenderEvent::FlushInline { elems, .. } => {
                assert_eq!(elems.len(), 2);
                match &elems[0] {
                    InlineElem::Text(t, style) => {
                        assert_eq!(t, "Some ");
                        assert!(!style.italic);
                    }
                    _ => panic!("Expected text"),
                }
                match &elems[1] {
                    InlineElem::Text(t, style) => {
                        assert_eq!(t, "text");
                        assert!(style.italic);
                    }
                    _ => panic!("Expected italic text"),
                }
            }
            _ => panic!("Expected FlushInline"),
        }

        // Then paragraph space
        assert_eq!(events[2], RenderEvent::Space(4.0));

        // Then list item
        match &events[3] {
            RenderEvent::FlushInline {
                elems,
                needs_bullet,
                indent,
                ..
            } => {
                assert!(*needs_bullet);
                assert_eq!(*indent, 1);
                assert_eq!(elems.len(), 1);
                match &elems[0] {
                    InlineElem::Text(t, _) => assert_eq!(t, "List item"),
                    _ => panic!("Expected text"),
                }
            }
            _ => panic!("Expected FlushInline"),
        }
    }

    #[test]
    fn test_parse_markdown_heading_levels() {
        let md = "# H1\n## H2\n### H3\n#### H4";
        let events = parse_markdown_to_events(md);
        assert_eq!(
            events[0],
            RenderEvent::Heading {
                level: 1,
                text: "H1".to_string()
            }
        );
        assert_eq!(
            events[1],
            RenderEvent::Heading {
                level: 2,
                text: "H2".to_string()
            }
        );
        assert_eq!(
            events[2],
            RenderEvent::Heading {
                level: 3,
                text: "H3".to_string()
            }
        );
        assert_eq!(
            events[3],
            RenderEvent::Heading {
                level: 4,
                text: "H4".to_string()
            }
        );
    }

    #[test]
    fn test_parse_markdown_code_block() {
        let md = "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let events = parse_markdown_to_events(md);
        assert_eq!(events.len(), 1);
        match &events[0] {
            RenderEvent::CodeBlock(content) => {
                assert!(content.contains("fn main()"));
            }
            _ => panic!("Expected CodeBlock event"),
        }
    }

    #[test]
    fn test_parse_markdown_inline_elements() {
        let md = "**bold** *italic* ~~strikethrough~~ `code` [link](https://example.com) ![img](https://example.com/a.jpg)";
        let events = parse_markdown_to_events(md);
        assert!(!events.is_empty());
        match &events[0] {
            RenderEvent::FlushInline { elems, .. } => {
                let mut has_bold = false;
                let mut has_italic = false;
                let mut has_strikethrough = false;
                let mut has_code = false;
                let mut has_link = false;
                let mut has_image = false;

                for elem in elems {
                    match elem {
                        InlineElem::Text(t, style) => {
                            if t == "bold" && style.bold {
                                has_bold = true;
                            }
                            if t == "italic" && style.italic {
                                has_italic = true;
                            }
                            if t == "strikethrough" && style.strikethrough {
                                has_strikethrough = true;
                            }
                            if t == "code" && style.code {
                                has_code = true;
                            }
                        }
                        InlineElem::Link(url, text) => {
                            if url == "https://example.com" && text == "link" {
                                has_link = true;
                            }
                        }
                        InlineElem::Image(url) => {
                            if url == "https://example.com/a.jpg" {
                                has_image = true;
                            }
                        }
                        _ => {}
                    }
                }
                assert!(has_bold, "Missing bold element");
                assert!(has_italic, "Missing italic element");
                assert!(has_strikethrough, "Missing strikethrough element");
                assert!(has_code, "Missing code element");
                assert!(has_link, "Missing link element");
                assert!(has_image, "Missing image element");
            }
            _ => panic!("Expected FlushInline"),
        }
    }

    #[test]
    fn test_parse_markdown_task_list() {
        let md = "- [ ] Task 1\n- [x] Task 2";
        let events = parse_markdown_to_events(md);

        let mut found_unchecked = false;
        let mut found_checked = false;

        for ev in &events {
            if let RenderEvent::FlushInline {
                task_checked,
                elems,
                ..
            } = ev
            {
                if let Some(false) = task_checked {
                    if elems.iter().any(|e| match e {
                        InlineElem::Text(t, _) => t == "Task 1",
                        _ => false,
                    }) {
                        found_unchecked = true;
                    }
                }
                if let Some(true) = task_checked {
                    if elems.iter().any(|e| match e {
                        InlineElem::Text(t, _) => t == "Task 2",
                        _ => false,
                    }) {
                        found_checked = true;
                    }
                }
            }
        }
        assert!(found_unchecked, "Missing unchecked task");
        assert!(found_checked, "Missing checked task");
    }

    #[test]
    fn test_parse_markdown_table() {
        let md = "| Col A | Col B |\n|---|---|\n| Val A | Val B |";
        let events = parse_markdown_to_events(md);

        let mut found_table = false;
        for ev in events {
            if let RenderEvent::Table(rows) = ev {
                found_table = true;
                assert_eq!(rows.len(), 2); // Header row + 1 data row
                assert_eq!(rows[0].len(), 2);
                assert_eq!(rows[1].len(), 2);
            }
        }
        assert!(found_table, "Expected Table event");
    }

    #[test]
    fn test_parse_markdown_rule_and_blockquote() {
        let md = "---\n\n> Quote text";
        let events = parse_markdown_to_events(md);

        let has_rule = events.iter().any(|e| matches!(e, RenderEvent::Separator));
        assert!(has_rule, "Expected Separator event");

        let has_quote = events.iter().any(|e| match e {
            RenderEvent::FlushInline { elems, .. } => elems.iter().any(|elem| match elem {
                InlineElem::Text(t, _) => t.contains("Quote text"),
                _ => false,
            }),
            _ => false,
        });
        assert!(has_quote, "Expected blockquote text");
    }

    #[test]
    fn test_parse_markdown_html_and_footnotes() {
        let md = "<span>Inline HTML</span>\n\nFootnote[^1]\n\n[^1]: Footnote details";
        let events = parse_markdown_to_events(md);

        let has_html = events.iter().any(|e| match e {
            RenderEvent::FlushInline { elems, .. } => {
                elems.iter().any(|elem| matches!(elem, InlineElem::Html(_)))
            }
            _ => false,
        });
        assert!(has_html, "Expected Html inline element");

        let has_fn_ref = events.iter().any(|e| match e {
            RenderEvent::FlushInline { elems, .. } => elems.iter().any(|elem| match elem {
                InlineElem::Text(t, _) => t.contains("[^1]"),
                _ => false,
            }),
            _ => false,
        });
        assert!(has_fn_ref, "Expected footnote reference");
    }

    #[test]
    fn test_build_toc() {
        let md = "# Title\nSome text\n## Subtitle";
        let toc = build_toc(md);
        assert_eq!(toc.len(), 2);
        assert_eq!(toc[0].title, "Title");
        assert_eq!(toc[0].level, 1);
        assert_eq!(toc[1].title, "Subtitle");
        assert_eq!(toc[1].level, 2);
    }

    #[test]
    fn test_parse_markdown_fuzz_property() {
        let random_markdowns = [
            "",
            "#",
            "*bold*",
            "```\ncode\n```",
            "|a|b|\n|-|-|\n|1|2|",
            "[link](http://example.com)",
            "- [ ] task",
        ];
        for md in random_markdowns {
            let _events = parse_markdown_to_events(md);
        }
    }
}

#[cfg(test)]
mod e2e_tests {
    use super::*;

    #[test]
    fn test_render_markdown_e2e() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut scroll_id = None;
                render_markdown(
                    ui,
                    "# Test\n\n- [ ] Task\n\n```rust\nlet x = 1;\n```",
                    &mut scroll_id,
                );

                let yaml_str = "a: 1\nb: 2";
                let val: serde_yaml::Value = serde_yaml::from_str(yaml_str).unwrap();
                render_yaml_table(ui, &val);
            });
        });
    }

    #[test]
    fn test_render_markdown_all_elements_e2e() {
        let ctx = egui::Context::default();
        let md = r#"# Heading 1
## Heading 2
### Heading 3

Paragraph with **bold**, *italic*, ~~strikethrough~~, `inline code`, [link](https://example.com), and ![img](https://example.com/img.png).

- [ ] Unchecked Task
- [x] Checked Task
- Regular list item

| Header 1 | Header 2 |
| --- | --- |
| Cell 1 | Cell 2 |

---

> Blockquote text

```python
def foo():
    return 42
```

<div>Html block</div>
"#;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut scroll_id = None;
                render_markdown(ui, md, &mut scroll_id);

                // Render non-mapping YAML table
                let non_map = serde_yaml::Value::String("test".to_string());
                render_yaml_table(ui, &non_map);
            });
        });
    }

    #[test]
    fn test_render_heading_scroll_to_id() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let target_id = egui::Id::new("Target Heading");
                let mut scroll_id = Some(target_id);

                render_heading(ui, "Target Heading", 1, &mut scroll_id);
                assert_eq!(
                    scroll_id, None,
                    "scroll_to_id should be cleared after scroll"
                );

                // Empty title should not trigger scroll
                let mut dummy_scroll = Some(target_id);
                render_heading(ui, "", 1, &mut dummy_scroll);
                assert_eq!(dummy_scroll, Some(target_id));
            });
        });
    }
}
