use eframe::egui;
use egui::RichText;

#[derive(Clone)]
enum InlineElem {
    Text(String, TextStyle),
    Link(String, String),
    Image(String),
    Html(String),
    SoftBreak,
}

#[derive(Clone, Default)]
struct TextStyle {
    bold: bool,
    italic: bool,
    code: bool,
    strikethrough: bool,
}

fn flush_inline(ui: &mut egui::Ui, elems: &mut Vec<InlineElem>, needs_bullet: &mut bool, task_checked: &mut Option<bool>, indent: usize, wrap: bool) {
    if elems.is_empty() && !*needs_bullet && task_checked.is_none() {
        return;
    }
    
    let add_content = |ui: &mut egui::Ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        
        if indent > 0 {
            ui.add_space(indent as f32 * 20.0);
        }
        if *needs_bullet {
            ui.label(RichText::new("• ").size(14.0));
            *needs_bullet = false;
        }
        if let Some(checked) = task_checked.take() {
            ui.add_space(4.0);
            ui.checkbox(&mut checked.clone(), "");
            ui.add_space(4.0);
        }
        
        for elem in elems.drain(..) {
            match elem {
                InlineElem::Text(t, style) => {
                    let mut rt = RichText::new(t);
                    if style.bold { rt = rt.strong(); }
                    if style.italic { rt = rt.italics(); }
                    if style.code { 
                        rt = rt.monospace().background_color(egui::Color32::from_gray(40)); 
                    }
                    if style.strikethrough { rt = rt.strikethrough(); }
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

fn render_code_block(ui: &mut egui::Ui, content: &str, _idx: &mut usize) {
    egui::Frame::none()
        .fill(egui::Color32::from_gray(30))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(content).monospace()).wrap(true));
        });
}

fn render_heading(ui: &mut egui::Ui, title: &str, level: u32, scroll_to_id: &mut Option<egui::Id>) {
    if level > 0 {
        let trimmed = title.trim().to_string();
        if !trimmed.is_empty() {
            let heading_id = egui::Id::new(&trimmed);
            if *scroll_to_id == Some(heading_id) {
                ui.scroll_to_rect(ui.max_rect(), None);
                *scroll_to_id = None;
            }
            let size = match level {
                1 => 24.0,
                2 => 20.0,
                3 => 16.0,
                _ => 14.0,
            };
            ui.heading(RichText::new(trimmed).size(size).strong());
        }
    }
}

pub fn render_yaml_table(ui: &mut egui::Ui, yaml: &serde_yaml::Value) {
    if let Some(mapping) = yaml.as_mapping() {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(15, 25, 65)) // Dark blue background
            .inner_margin(8.0)
            .rounding(4.0)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal().id_source("yaml_scroll").show(ui, |ui| {
                    egui::Grid::new("yaml_grid")
                        .num_columns(2)
                        .striped(true)
                        .spacing([10.0, 4.0])
                        .show(ui, |ui| {
                            for (key, value) in mapping {
                                if let Some(key_str) = key.as_str() {
                                    ui.label(RichText::new(key_str).strong().monospace().color(egui::Color32::WHITE));
                                    let val_str = match value {
                                        serde_yaml::Value::String(s) => s.clone(),
                                        serde_yaml::Value::Sequence(seq) => {
                                            let items: Vec<String> = seq.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect();
                                            items.join(", ")
                                        }
                                        _ => serde_yaml::to_string(value).unwrap_or_default(),
                                    };
                                    ui.label(RichText::new(val_str).monospace().color(egui::Color32::WHITE));
                                    ui.end_row();
                                }
                            }
                        });
                });
            });
        ui.add_space(8.0);
    }
}

pub fn render_markdown(ui: &mut egui::Ui, markdown_text: &str, scroll_to_id: &mut Option<egui::Id>) {
    use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    
    let parser = Parser::new_ext(markdown_text, options);

    let mut in_code_block = false;
    let mut code_block_content = String::new();
    let mut in_heading = false;
    let mut heading_level = 0;
    let mut heading_text = String::new();
    let mut code_block_idx = 0;

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

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(_info)) => {
                if !buffered_inline.is_empty() {
                    flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
                }
                in_code_block = true;
                code_block_content.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                render_code_block(ui, &code_block_content, &mut code_block_idx);
            }
            Event::Start(Tag::Heading { level, .. }) => {
                if !buffered_inline.is_empty() {
                    flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
                }
                in_heading = true;
                heading_level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    _ => 0,
                };
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                render_heading(ui, &heading_text, heading_level, scroll_to_id);
                in_heading = false;
                heading_level = 0;
            }
            Event::Start(Tag::Paragraph) => {
                if !in_table_cell {
                    if !buffered_inline.is_empty() {
                        flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
                    }
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if !in_table_cell {
                    flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
                    ui.add_space(4.0);
                }
            }
            Event::Start(Tag::List(_)) => {
                if !buffered_inline.is_empty() {
                    flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
                }
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
                if list_depth > 0 {
                    list_depth -= 1;
                }
            }
            Event::Start(Tag::Item) => {
                if !buffered_inline.is_empty() {
                    flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
                }
                needs_bullet = true;
            }
            Event::End(TagEnd::Item) => {
                flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
            }
            Event::Start(Tag::BlockQuote) => {
                if !buffered_inline.is_empty() {
                    flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
                }
            }
            Event::End(TagEnd::BlockQuote) => {
                flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
            }
            Event::Start(Tag::Table(_)) => {
                if !buffered_inline.is_empty() {
                    flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
                }
                table_cells.clear();
            }
            Event::End(TagEnd::Table) => {
                egui::ScrollArea::horizontal().id_source(ui.next_auto_id()).show(ui, |ui| {
                    egui::Grid::new(ui.next_auto_id())
                        .striped(true)
                        .spacing([10.0, 4.0])
                        .show(ui, |ui| {
                            for row in &table_cells {
                                for cell in row {
                                    let mut mut_cell = cell.clone();
                                    let mut dummy_needs_bullet = false;
                                    let mut dummy_task = None;
                                    flush_inline(ui, &mut mut_cell, &mut dummy_needs_bullet, &mut dummy_task, 0, false);
                                }
                                ui.end_row();
                            }
                        });
                });
                table_cells.clear();
                ui.add_space(4.0);
            }
            Event::Start(Tag::TableHead) => {}
            Event::End(TagEnd::TableHead) => {}
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
                        flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
                    } else {
                        buffered_inline.push(InlineElem::SoftBreak);
                    }
                }
            }
            Event::Rule => {
                flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
                ui.separator();
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
                    flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
                }
                ui.separator();
                let text = format!("[^{}]: ", name);
                let mut s = current_style.clone();
                s.bold = true;
                buffered_inline.push(InlineElem::Text(text, s));
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
            }
            Event::Start(Tag::HtmlBlock) => {
                if !buffered_inline.is_empty() {
                    flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
                }
            }
            Event::End(TagEnd::HtmlBlock) => {
                flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
            }
            _ => {}
        }
    }
    flush_inline(ui, &mut buffered_inline, &mut needs_bullet, &mut task_checked, list_depth, true);
}

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
                    _ => 0,
                };
                if lvl > 0 {
                    heading_level = Some(lvl);
                    current_header.clear();
                }
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
