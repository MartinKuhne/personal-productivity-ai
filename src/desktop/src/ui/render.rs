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
        /// Styled inline elements that make up the heading text. Captures
        /// bold, italic, code, strikethrough, links, and images — the
        /// previous `text: String` field discarded all of these, so
        /// `# *italic*` rendered as a plain bold heading with the text
        /// "italic" (the emphasis marker was silently dropped).
        elems: Vec<InlineElem>,
    },
    Table(Vec<Vec<Vec<InlineElem>>>),
    Space(f32),
    Separator,
}

/// Concatenate the plain-text content of inline elements. Used to derive
/// the scroll-id key and the ToC title from a heading's styled elements.
pub fn heading_plain_text(elems: &[InlineElem]) -> String {
    let mut out = String::new();
    for e in elems {
        match e {
            InlineElem::Text(t, _) => out.push_str(t),
            InlineElem::Link(_, t) => out.push_str(t),
            InlineElem::Image(url) => {
                out.push_str(&format!("[Image: {}]", url));
            }
            InlineElem::Html(h) => out.push_str(h),
            InlineElem::SoftBreak => out.push(' '),
        }
    }
    out
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
/// Inputs: `ui` (mut), `elems` (heading inline elements), `level`, `scroll_to_id` (mut)
/// Outputs: None
/// Purity: Impure (modifies UI state). Thin adapter.
fn render_heading(
    ui: &mut egui::Ui,
    elems: &[InlineElem],
    level: u32,
    scroll_to_id: &mut Option<egui::Id>,
) {
    let plain = heading_plain_text(elems);
    let trimmed = plain.trim().to_string();
    if trimmed.is_empty() {
        return;
    }
    let heading_id = egui::Id::new(&trimmed);
    let size = match level {
        1 => 32.0,
        2 => 24.0,
        3 => 18.0,
        4 => 14.0,
        _ => 12.0,
    };
    // Render the styled inline elements with the heading's size + weight.
    // Each InlineElem becomes its own label inside a horizontal flow, so
    // bold/italic/code/strikethrough all survive into the rendered
    // heading. The trailing space is added via the layout's natural
    // padding; no explicit spacer needed because each label's contents
    // already include their own whitespace.
    let response = ui.horizontal(|ui| {
        for elem in elems {
            match elem {
                InlineElem::Text(t, style) => {
                    let mut rt = RichText::new(t).size(size).strong();
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
                    // Links inside a heading inherit the heading size and
                    // weight (no .strong() override, since ui.hyperlink
                    // uses its own styling).
                    ui.hyperlink_to(egui::RichText::new(text).size(size), url);
                }
                InlineElem::Image(url) => {
                    ui.label(RichText::new(format!("[Image: {}]", url)).size(size));
                }
                InlineElem::Html(h) => {
                    ui.label(
                        RichText::new(h)
                            .size(size)
                            .italics()
                            .color(egui::Color32::GRAY),
                    );
                }
                InlineElem::SoftBreak => {
                    ui.label(RichText::new(" ").size(size));
                }
            }
        }
    });
    // `ui.horizontal` returns a response wrapping the whole row; use
    // its bounding rect for scroll-to-me so the whole heading scrolls
    // into view (not just the first styled element).
    if *scroll_to_id == Some(heading_id) {
        response.response.scroll_to_me(Some(egui::Align::Center));
        *scroll_to_id = None;
    }
    ui.add_space(4.0);
}

/// Purpose: Renders a single table cell, always emitting at least one widget.
///
/// When `pinned_width` is `Some(w)`, the cell Ui is clamped to exactly `w`
/// pixels (`ui.set_width`) and text is laid out with `horizontal_wrapped` +
/// `Label::wrap(true)` so that multi-word cells wrap at whitespace. This is the
/// FTWA-pinned mode (`crates::ui::table_width`). The FTWA invariant
/// `w >= min_content >= longest-token` guarantees no unbreakable token is ever
/// split or clipped — only inter-word whitespace wraps.
///
/// When `pinned_width` is `None` (the §3.6 fallback path), the cell uses
/// `ui.horizontal` (no wrap) so the cell reports its full single-line intrinsic
/// width to the parent `Grid`; any overflow is handled by the wrapping
/// `ScrollArea` (current pre-FTWA behaviour).
fn render_table_cell(ui: &mut egui::Ui, cell: &[InlineElem], pinned_width: Option<f32>) {
    if cell.is_empty() {
        if let Some(w) = pinned_width {
            ui.allocate_at_least(egui::vec2(w, 0.0), egui::Sense::hover());
        } else {
            ui.label("");
        }
        return;
    }
    let content = |ui: &mut egui::Ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for elem in cell {
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
                    ui.add(egui::Label::new(rt).wrap(true));
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
    if let Some(w) = pinned_width {
        let (rect, _) = ui.allocate_at_least(egui::vec2(w, 0.0), egui::Sense::hover());
        let layout = egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true);
        let mut child_ui = ui.child_ui(rect, layout);
        content(&mut child_ui);
    } else {
        ui.horizontal(content);
    }
}

/// Purpose: Renders a table using the Fair Table Width Algorithm (FTWA).
///
/// Per-column pixel widths are computed by `crate::ui::table_width::ftwa` from
/// egui-shaped max-content and min-content measurements; cells are then pinned
/// to their assigned width via `ui.allocate_ui_at_least` so the child Ui's
/// available_width matches the column width (see `doc/planning/table-column-width-algorithm.md`
/// §5 decision Q5). When the available width falls below the sum of min-content
/// (`decision.needs_horizontal_scroll`), the table physically cannot fit even
/// with every column at its longest-token floor and we fall back to the prior
/// behaviour: a wrapping `ScrollArea` over max-content columns (doc §3.6) so the
/// strongest invariant — never split a token — is preserved.
///
/// Grid spacing is `[10.0, 4.0]` (10 px gutters). The available content width
/// passed to FTWA subtracts `(N - 1) * 10.0` for those gutters so the assigned
/// widths sum to the true content rect.
fn render_table(ui: &mut egui::Ui, table_cells: &[Vec<Vec<InlineElem>>]) {
    let n = table_cells.iter().map(|row| row.len()).max().unwrap_or(0);
    if n == 0 {
        return;
    }

    let (max_w, min_w) = crate::ui::table_width::measure(table_cells, ui);
    let gutter = 10.0_f32;
    let avail = (ui.available_width() - (n as f32 - 1.0) * gutter).max(0.0);
    let decision = crate::ui::table_width::ftwa(&max_w, &min_w, avail);

    if decision.needs_horizontal_scroll {
        // §3.6 fallback: nothing can fit; preserve the never-break-token
        // invariant by letting content overflow into a horizontal ScrollArea.
        egui::ScrollArea::horizontal()
            .id_source(ui.next_auto_id())
            .show(ui, |ui| {
                egui::Grid::new(ui.next_auto_id())
                    .striped(true)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        for row in table_cells {
                            for cell in row {
                                render_table_cell(ui, cell, None);
                            }
                            ui.end_row();
                        }
                    });
            });
        return;
    }

    // FTWA path: pin every cell to its assigned column width.
    egui::Grid::new(ui.next_auto_id())
        .striped(true)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            for row in table_cells {
                for (j, cell) in row.iter().enumerate() {
                    let w = decision
                        .widths
                        .get(j)
                        .copied()
                        .filter(|w| w.is_finite() && *w > 0.0);
                    render_table_cell(ui, cell, w);
                }
                ui.end_row();
            }
        });
}

/// Purpose: Parses a YAML mapping into a list of key-value string pairs.
/// Inputs: `yaml`
/// Outputs: List of (String, String) if valid mapping, else None.
/// Purity: Pure function.
///
/// # Examples
///
/// ```
/// use fastmd::ui::render::parse_yaml_to_pairs;
/// use serde_yaml::Value;
///
/// let yaml: Value = serde_yaml::from_str("a: 1\nb: hello\nc: [x, y]").unwrap();
/// let pairs = parse_yaml_to_pairs(&yaml).unwrap();
/// assert_eq!(pairs.len(), 3);
/// assert_eq!(pairs[0], ("a".to_string(), "1".to_string()));
/// assert_eq!(pairs[1], ("b".to_string(), "hello".to_string()));
/// assert_eq!(pairs[2], ("c".to_string(), "x, y".to_string()));
///
/// // Non-mapping values produce None.
/// let s: Value = serde_yaml::from_str("just a string").unwrap();
/// assert!(parse_yaml_to_pairs(&s).is_none());
/// ```
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
        let available_width = ui.available_width();
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 24, 27))
            .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_gray(40)))
            .inner_margin(8.0)
            .rounding(4.0)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .id_source("yaml_scroll")
                    .show(ui, |ui| {
                        ui.set_min_width(available_width);
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
///
/// # Examples
///
/// ```
/// use fastmd::ui::render::{parse_markdown_to_events, RenderEvent, InlineElem, TextStyle};
///
/// let events = parse_markdown_to_events("# Title\n\nhello *world*");
/// // First event is the H1 heading.
/// let heading_text = match &events[0] {
///     RenderEvent::Heading { elems, .. } => fastmd::ui::render::heading_plain_text(elems),
///     _ => panic!("expected first event to be a heading"),
/// };
/// assert_eq!(heading_text, "Title");
/// // The paragraph flushes inline elements with mixed styling.
/// let para = events.iter().find_map(|e| match e {
///     RenderEvent::FlushInline { elems, needs_bullet: false, .. } if !elems.is_empty() => Some(elems),
///     _ => None,
/// });
/// assert!(para.is_some());
/// ```
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
    // Headings preserve their inline elements (styled spans, links,
    // images) instead of collapsing to plain text. See the `Heading`
    // variant of `RenderEvent` for the motivation.
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
                        true,
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
                list_depth = list_depth.saturating_sub(1);
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
                } else if in_link {
                    // A link is its own inline element regardless of
                    // whether we're inside a heading — a link inside a
                    // heading stays a link.
                    if in_heading {
                        heading_elems.push(InlineElem::Link(link_url.clone(), text.to_string()));
                    } else {
                        buffered_inline.push(InlineElem::Link(link_url.clone(), text.to_string()));
                    }
                } else if in_heading {
                    // Styled spans inside a heading. The renderer uses
                    // these directly; `heading_plain_text` derives the
                    // scroll-id key from them.
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
            RenderEvent::Heading { level, elems } => {
                render_heading(ui, &elems, level, scroll_to_id);
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
/// Builds a Table of Contents from markdown.
///
/// # Examples
///
/// ```
/// use fastmd::ui::render::build_toc;
///
/// let toc = build_toc("# Title\n\n## Sub\n\nbody");
/// assert_eq!(toc.len(), 2);
/// assert_eq!(toc[0].title, "Title");
/// assert_eq!(toc[0].level, 1);
/// assert_eq!(toc[1].title, "Sub");
/// assert_eq!(toc[1].level, 2);
///
/// // No headings → empty TOC.
/// assert!(build_toc("just a paragraph").is_empty());
/// ```
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
        // Uses structural lookups (find / filter) rather than indexed
        // access so the test doesn't break when events are reordered or
        // when the parser gains a new event type between existing ones.
        let md = "# Heading 1\nSome *text*\n- List item";
        let events = parse_markdown_to_events(md);

        // H1 heading must be present, regardless of position.
        assert!(
            events.iter().any(|e| matches!(
                e,
                RenderEvent::Heading { level: 1, elems } if heading_plain_text(elems) == "Heading 1"
            )),
            "missing H1 'Heading 1' in {events:?}"
        );

        // A FlushInline carrying "Some " (not italic) followed by "text"
        // (italic) — this is the paragraph that mixes emphasis.
        let paragraph = events.iter().find_map(|e| match e {
            RenderEvent::FlushInline {
                elems,
                needs_bullet: false,
                ..
            } if !elems.is_empty() => Some(elems),
            _ => None,
        });
        let elems = paragraph.expect("expected a non-bullet FlushInline for the paragraph");
        assert_eq!(elems.len(), 2, "paragraph should have 2 inline elems");
        match &elems[0] {
            InlineElem::Text(t, style) => {
                assert_eq!(t, "Some ");
                assert!(!style.italic, "'Some ' must not be italic");
            }
            other => panic!("expected 'Some ' text, got {other:?}"),
        }
        match &elems[1] {
            InlineElem::Text(t, style) => {
                assert_eq!(t, "text");
                assert!(style.italic, "'text' must be italic");
            }
            other => panic!("expected italic 'text', got {other:?}"),
        }

        // The paragraph's trailing space event.
        assert!(
            events.iter().any(|e| matches!(e, RenderEvent::Space(4.0))),
            "missing Space(4.0) event in {events:?}"
        );

        // The bulleted list item, at indent 1.
        let list_item = events.iter().find_map(|e| match e {
            RenderEvent::FlushInline {
                elems,
                needs_bullet: true,
                indent: 1,
                ..
            } => Some(elems),
            _ => None,
        });
        let elems = list_item.expect("expected a bulleted FlushInline at indent 1");
        assert_eq!(elems.len(), 1, "list item should have 1 inline elem");
        match &elems[0] {
            InlineElem::Text(t, _) => assert_eq!(t, "List item"),
            other => panic!("expected 'List item' text, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_markdown_heading_levels() {
        // Structural check: every level 1..=4 appears with the right text.
        // Doesn't depend on event ordering or extra events between them.
        let md = "# H1\n## H2\n### H3\n#### H4";
        let events = parse_markdown_to_events(md);
        for (level, text) in [(1, "H1"), (2, "H2"), (3, "H3"), (4, "H4")] {
            assert!(
                events.iter().any(|e| matches!(
                    e,
                    RenderEvent::Heading { level: l, elems } if *l == level && heading_plain_text(elems) == text
                )),
                "missing H{level} '{text}' in {events:?}"
            );
        }
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
    fn test_parse_markdown_table_empty_cells() {
        let md = "| A | | C |\n|---|---|---|\n| | B | |";
        let events = parse_markdown_to_events(md);

        let mut found_table = false;
        for ev in events {
            if let RenderEvent::Table(rows) = ev {
                found_table = true;
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].len(), 3);
                assert_eq!(rows[1].len(), 3);
                assert!(rows[0][1].is_empty(), "Header cell 1 should be empty");
                assert!(rows[1][0].is_empty(), "Data cell 0 should be empty");
                assert!(rows[1][2].is_empty(), "Data cell 2 should be empty");
            }
        }
        assert!(found_table, "Expected Table event");
    }

    #[test]
    fn test_parse_markdown_table_with_bold_and_special_chars() {
        let md = "| Name | Account | Amount | Type |\n|---|---|---|---|\n| **Vanguard** | #12345678 | $1 | Taxable (investment) |";
        let events = parse_markdown_to_events(md);

        let mut found_table = false;
        for ev in events {
            if let RenderEvent::Table(rows) = ev {
                found_table = true;
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].len(), 4);
                assert_eq!(rows[1].len(), 4);
                let vanguard_cell = &rows[1][0];
                assert_eq!(vanguard_cell.len(), 1);
                match &vanguard_cell[0] {
                    InlineElem::Text(t, style) => {
                        assert_eq!(t, "Vanguard");
                        assert!(style.bold, "Vanguard should be bold");
                    }
                    _ => panic!("Expected Text element"),
                }
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
        // Covers the full matrix: empty, missing headings, single and
        // multiple levels (H1..H6), code-in-heading, special chars,
        // and the order of headings in the source.
        let md = "# Title\nSome text\n## Subtitle";
        let toc = build_toc(md);
        assert_eq!(toc.len(), 2);
        assert_eq!(toc[0].title, "Title");
        assert_eq!(toc[0].level, 1);
        assert_eq!(toc[1].title, "Subtitle");
        assert_eq!(toc[1].level, 2);

        assert!(
            build_toc("").is_empty(),
            "empty input must produce empty TOC"
        );
        assert!(
            build_toc("Just a paragraph.\n\nAnother paragraph.").is_empty(),
            "no-heading input must produce empty TOC"
        );

        let h1 = build_toc("# Title\n\nContent");
        assert_eq!(h1.len(), 1);
        assert_eq!(h1[0].level, 1);
        assert_eq!(h1[0].title, "Title");

        let mixed = build_toc("# H1\n\n## H2\n\n### H3");
        assert_eq!(mixed.len(), 3);
        assert_eq!(mixed[0].level, 1);
        assert_eq!(mixed[0].title, "H1");
        assert_eq!(mixed[1].level, 2);
        assert_eq!(mixed[1].title, "H2");
        assert_eq!(mixed[2].level, 3);
        assert_eq!(mixed[2].title, "H3");

        let deep = build_toc("# H1\n\n#### H4\n\n##### H5\n\n###### H6");
        assert_eq!(deep.len(), 4);
        assert_eq!(deep[1].level, 4);
        assert_eq!(deep[2].level, 5);
        assert_eq!(deep[3].level, 6);

        let code_in_heading = build_toc("# `code` in heading");
        assert_eq!(code_in_heading.len(), 1);
        assert!(code_in_heading[0].title.contains("code"));

        let ignored =
            build_toc("# Real Title\n\nSome text\n\n## Another\n\n- list item\n\n> blockquote");
        assert_eq!(ignored.len(), 2);
        assert_eq!(ignored[0].title, "Real Title");
        assert_eq!(ignored[1].title, "Another");

        let order = build_toc("## Second\n\n# First\n\n### Third");
        assert_eq!(order.len(), 3);
        // Headings appear in source order, not sorted by level.
        assert_eq!(order[0].title, "Second");
        assert_eq!(order[1].title, "First");
        assert_eq!(order[2].title, "Third");

        let special = build_toc("# H1: Introduction & Conclusion");
        assert_eq!(special.len(), 1);
        assert!(special[0].title.contains("H1: Introduction"));
    }

    #[test]
    fn test_parse_edge_cases_expose_quirks() {
        // Targeted probes for known-fragile areas. Each assertion captures
        // the expected behavior; a failure here is a parser defect.

        // Empty input must produce zero events (no spurious separators).
        assert_eq!(
            parse_markdown_to_events(""),
            vec![],
            "empty input should produce no events"
        );

        // Whitespace-only input must produce zero events.
        assert_eq!(
            parse_markdown_to_events("   \n\n\n"),
            vec![],
            "whitespace input should produce no events"
        );

        // A table with all empty cells must have all rows with N cells.
        let events = parse_markdown_to_events("| | | |\n|---|---|---|\n");
        for ev in &events {
            if let RenderEvent::Table(rows) = ev {
                for (i, row) in rows.iter().enumerate() {
                    assert_eq!(row.len(), 3, "empty-cell table row {i} should have 3 cells");
                }
            }
        }

        // A table where the separator has fewer columns than the header
        // must still produce a rectangular table — pulldown-cmark normalizes
        // this. If the parser blindly concatenates, the row would be ragged.
        let events = parse_markdown_to_events("| a | b | c |\n|---|---|\n| 1 | 2 | 3 |");
        for ev in &events {
            if let RenderEvent::Table(rows) = ev {
                for (i, row) in rows.iter().enumerate() {
                    assert!(
                        row.iter().all(|c| c.len() == row.len()),
                        "mismatched-col table row {i} has inconsistent cell count: {:?}",
                        row.iter().map(Vec::len).collect::<Vec<_>>()
                    );
                }
            }
        }

        // Nested lists: every FlushInline must have `indent` ≤ the input's
        // list depth. A 3-deep nested list should produce indents up to 3.
        let events = parse_markdown_to_events("- a\n  - b\n    - c\n- d");
        for ev in &events {
            if let RenderEvent::FlushInline { indent, .. } = ev {
                assert!(*indent <= 3, "3-deep nested list produced indent={indent}");
            }
        }

        // Heading inside a blockquote: the heading must still emit a
        // Heading event, not be swallowed by the blockquote handling.
        let events = parse_markdown_to_events("> # heading in quote");
        assert!(
            events.iter().any(|e| matches!(
                e,
                RenderEvent::Heading { level: 1, elems } if heading_plain_text(elems).contains("heading in quote")
            )),
            "heading inside blockquote was lost: {events:?}"
        );
    }

    #[test]
    fn test_parse_suspicious_paths() {
        // These probe paths the existing tests don't exercise.
        // Each captures an expected invariant; failure = parser bug.

        // Empty link: `[text]()` should produce a Link with empty URL.
        let events = parse_markdown_to_events("[text]()");
        assert!(
            events.iter().any(|e| matches!(
                e,
                RenderEvent::FlushInline { elems, .. } if elems.iter().any(|el| matches!(
                    el,
                    InlineElem::Link(url, text) if url.is_empty() && text == "text"
                ))
            )),
            "empty-URL link lost: {events:?}"
        );

        // Empty code block: ```\n``` should produce a CodeBlock with
        // empty content, not be dropped entirely.
        let events = parse_markdown_to_events("```\n```");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, RenderEvent::CodeBlock(c) if c.is_empty())),
            "empty code block lost: {events:?}"
        );

        // Image in heading: `# ![alt](url)` — image must not be dropped.
        let events = parse_markdown_to_events("# ![alt text](https://x/y.png)");
        assert!(
            events.iter().any(|e| matches!(
                e,
                RenderEvent::Heading { level: 1, elems } if heading_plain_text(elems).contains("alt text")
            )),
            "image alt text lost from heading: {events:?}"
        );

        // Heading immediately followed by heading: `# A\n# B`
        let events = parse_markdown_to_events("# A\n# B");
        let headings: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
                _ => None,
            })
            .collect();
        assert_eq!(
            headings,
            vec![(1, "A".to_string()), (1, "B".to_string())],
            "consecutive headings: {events:?}"
        );

        // An empty list `- ` (item with no text). The parser should still
        // emit a FlushInline (with empty elems but bullet) so the bullet
        // gets rendered. The current `push_inline` helper skips when
        // `elems.is_empty() && !needs_bullet && task_checked.is_none()` —
        // but `needs_bullet` is true here, so the bullet *should* render.
        let events = parse_markdown_to_events("- ");
        assert!(
            events.iter().any(|e| matches!(
                e,
                RenderEvent::FlushInline {
                    needs_bullet: true,
                    ..
                }
            )),
            "empty list item lost: {events:?}"
        );

        // A table with only the header row, no data rows. The Table event
        // should still emit (with 1 row), not be dropped.
        let events = parse_markdown_to_events("| H1 | H2 |\n|---|---|\n");
        let table_event = events.iter().find_map(|e| {
            if let RenderEvent::Table(rows) = e {
                Some(rows.len())
            } else {
                None
            }
        });
        assert_eq!(
            table_event,
            Some(1),
            "header-only table dropped: {events:?}"
        );
    }

    // TODO(TDD follow-up): `# *italic*`, `# **bold**`, `# `code``,
    /// `# *italic*`, `# **bold**`, `# \`code\``, `# ~~strike~~`, and
    /// `# [link](url)` all previously lost their inline formatting
    /// because `RenderEvent::Heading` stored `text: String` (plain
    /// concatenation) rather than `elems: Vec<InlineElem>`. The struct
    /// now carries the styled elements; the renderer renders each
    /// span with the heading's size and weight. These tests pin the
    /// expected contract end-to-end.
    #[test]
    fn test_heading_preserves_italic() {
        let events = parse_markdown_to_events("# *hello*");
        let heading = events
            .iter()
            .find_map(|e| {
                if let RenderEvent::Heading { level, elems } = e {
                    Some((*level, elems))
                } else {
                    None
                }
            })
            .expect("must have a Heading event");
        assert_eq!(heading.0, 1);
        assert_eq!(heading_plain_text(&heading.1), "hello");
        // At least one elem carries the italic style.
        assert!(
            heading.1.iter().any(|e| matches!(
                e,
                InlineElem::Text(_, style) if style.italic
            )),
            "italic style not preserved in heading elems: {:?}",
            heading.1
        );
    }

    #[test]
    fn test_heading_preserves_bold() {
        let events = parse_markdown_to_events("# **loud**");
        let heading = events
            .iter()
            .find_map(|e| {
                if let RenderEvent::Heading { level, elems } = e {
                    Some((*level, elems))
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(heading.0, 1);
        assert!(
            heading.1.iter().any(|e| matches!(
                e,
                InlineElem::Text(_, style) if style.bold
            )),
            "bold style not preserved: {:?}",
            heading.1
        );
    }

    #[test]
    fn test_heading_preserves_code() {
        let events = parse_markdown_to_events("# `code` in heading");
        let heading = events
            .iter()
            .find_map(|e| {
                if let RenderEvent::Heading { level, elems } = e {
                    Some((*level, elems))
                } else {
                    None
                }
            })
            .unwrap();
        assert!(
            heading.1.iter().any(|e| matches!(
                e,
                InlineElem::Text(_, style) if style.code
            )),
            "code style not preserved: {:?}",
            heading.1
        );
    }

    #[test]
    fn test_heading_preserves_link() {
        let events = parse_markdown_to_events("# [click](https://example.com)");
        let heading = events
            .iter()
            .find_map(|e| {
                if let RenderEvent::Heading { level, elems } = e {
                    Some((*level, elems))
                } else {
                    None
                }
            })
            .unwrap();
        assert!(
            heading.1.iter().any(|e| matches!(
                e,
                InlineElem::Link(url, text) if url == "https://example.com" && text == "click"
            )),
            "link not preserved in heading: {:?}",
            heading.1
        );
    }

    #[test]
    fn test_heading_preserves_strikethrough() {
        let events = parse_markdown_to_events("# ~~old~~");
        let heading = events
            .iter()
            .find_map(|e| {
                if let RenderEvent::Heading { level, elems } = e {
                    Some((*level, elems))
                } else {
                    None
                }
            })
            .unwrap();
        assert!(
            heading.1.iter().any(|e| matches!(
                e,
                InlineElem::Text(_, style) if style.strikethrough
            )),
            "strikethrough not preserved: {:?}",
            heading.1
        );
    }

    #[test]
    fn test_parse_markdown_fuzz_property() {
        use proptest::prelude::*;
        use proptest::strategy::ValueTree;

        // Generates a string of common markdown elements joined by blank
        // lines, so the parser sees a realistic mix of constructs.
        fn md_grammar() -> impl Strategy<Value = String> {
            let heading = "[#]{1,6}[ \\t]+[A-Za-z ]{1,30}";
            let para = "[A-Za-z ,.!?]{0,80}";
            let code_block = "```[a-z]*\\n[a-zA-Z0-9 ;]{0,40}\\n```";
            let bullet = "- [ \\t]{0,3}[A-Za-z ]{1,30}";
            let task = "- \\[[ x]\\] [A-Za-z ]{1,30}";
            let table_row = "\\|?[A-Za-z ]{1,5}(\\|[A-Za-z ]{1,5})*\\|?";
            let table_sep = "\\|?[ -]{3}(\\|[ -]{3})*\\|?";
            let link = "\\[[A-Za-z ]{1,20}\\]\\(https?://[a-z.]+\\)";
            let inline = prop_oneof![
                2 => Just(para.to_string()),
                1 => Just(heading.to_string()),
                1 => Just(code_block.to_string()),
                1 => Just(bullet.to_string()),
                1 => Just(task.to_string()),
                1 => Just(format!("{table_row}\\n{table_sep}\\n{table_row}")),
                1 => Just(link.to_string()),
            ];
            proptest::collection::vec(inline, 0..8).prop_map(|v| v.join("\n\n"))
        }

        let mut runner = proptest::test_runner::TestRunner::default();
        let strategy = md_grammar();
        for _ in 0..64 {
            let input = strategy
                .new_tree(&mut runner)
                .expect("strategy should generate a value")
                .current();
            let events = parse_markdown_to_events(&input);

            // Output must be bounded — no input of this size can produce
            // more than a small constant multiple of its byte count in events.
            assert!(
                events.len() < 1_000,
                "event count exploded for input {input:?}: {} events",
                events.len()
            );

            for event in &events {
                match event {
                    RenderEvent::Heading { level, elems } => {
                        assert!(
                            (1..=6).contains(level),
                            "heading level out of range: {level} in {elems:?}"
                        );
                    }
                    RenderEvent::Table(rows) => {
                        // Tables must be rectangular — pulldown-cmark emits
                        // them as a sequence of `TableRow` / `TableCell`
                        // events; the parser concatenates them and a
                        // non-rectangular result is a parser bug.
                        if let Some(first) = rows.first() {
                            let expected = first.len();
                            for (i, row) in rows.iter().enumerate() {
                                assert_eq!(
                                    row.len(),
                                    expected,
                                    "table row {i} has {} cells, expected {expected}",
                                    row.len()
                                );
                            }
                        }
                    }
                    RenderEvent::FlushInline { indent, .. } => {
                        // `indent` must not exceed the observed list depth.
                        // The parser increments `list_depth` on `Tag::List`
                        // and decrements on `TagEnd::List`; an indent > 8
                        // is impossible for a small input.
                        assert!(*indent <= 8, "indent {indent} exceeds safe bound");
                    }
                    _ => {}
                }
            }
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
    fn test_render_table_with_empty_cells_e2e() {
        let ctx = egui::Context::default();
        let md = "| A | | C |\n|---|---|---|\n| | B | |";
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut scroll_id = None;
                render_markdown(ui, md, &mut scroll_id);
            });
        });
    }

    #[test]
    fn test_render_table_with_bold_and_special_chars_e2e() {
        let ctx = egui::Context::default();
        let md = "| Name | Account | Amount | Type |\n|---|---|---|---|\n| **Vanguard** | #12345678 | $1 | Taxable (investment) |";
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut scroll_id = None;
                render_markdown(ui, md, &mut scroll_id);
            });
        });
    }

    #[test]
    fn test_ftwa_measure_user_table() {
        let ctx = egui::Context::default();
        let md = r#"| Plan Name | Monthly Premium | Annual Deductible | Max Out-of-Pocket | Quality Rating | Notes/Evaluation |
|-----------|-----------------|-------------------|---------------------|----------------|-----------------------|
| Ambetter Cascade Select Complete Gold | $891.55 | $1,000 Individual / $2,000 Family | $7,000 Indiv. / $14,000 Fam. | ★★★☆ | Good balance of low deductible and moderate premium. |
| Kaiser Permanente Cascade Complete Gold | $1,103.11 | $1,000 Individual / $2,000 Family | $7,000 Indiv. / $14,000 Fam. | ★★★★ | Excellent reputation and high quality rating. |
"#;
        let events = parse_markdown_to_events(md);
        let cells = match events.iter().find(|e| matches!(e, RenderEvent::Table(_))) {
            Some(RenderEvent::Table(c)) => c.clone(),
            _ => panic!("No table found"),
        };
        assert_eq!(cells.len(), 3); // header + 2 data rows
        assert_eq!(cells[0].len(), 6);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (max_w, min_w) = crate::ui::table_width::measure(&cells, ui);
                assert_eq!(max_w.len(), 6, "6 max-content widths");
                assert_eq!(min_w.len(), 6, "6 min-content widths");
                for (i, (&mx, &mn)) in max_w.iter().zip(min_w.iter()).enumerate() {
                    assert!(mx >= mn, "col {i}: max {mx} < min {mn}");
                    assert!(mx > 0.0, "col {i}: max-content must be > 0");
                    assert!(mn > 0.0, "col {i}: min-content must be > 0");
                }

                let sum_min: f32 = min_w.iter().sum();
                let sum_max: f32 = max_w.iter().sum();

                // Test the same 6-column table at four viewports, covering
                // the three regimes (surplus / deficit / §3.6 fallback).
                for &avail in &[ui.available_width(), 800.0, 600.0, 400.0] {
                    let gutter = 10.0_f32;
                    let a = (avail - (cells[0].len() as f32 - 1.0) * gutter).max(0.0);
                    let decision = crate::ui::table_width::ftwa(&max_w, &min_w, a);
                    assert_eq!(decision.widths.len(), 6, "avail={a}: must have 6 widths");
                    for &w in &decision.widths {
                        assert!(w > 0.0, "avail={a}: each column must have positive width");
                    }
                    // §3.6 flag must match the strict `<` condition.
                    assert_eq!(
                        decision.needs_horizontal_scroll,
                        a < sum_min,
                        "avail={a}: needs_horizontal_scroll must match `a < sum_min` ({} vs {})",
                        a,
                        sum_min
                    );
                    if !decision.needs_horizontal_scroll {
                        // G3 invariant: in any non-§3.6 regime, sum exactly
                        // equals available. (In §3.6 the function returns
                        // min-content widths and signals horizontal scroll.)
                        let sum: f32 = decision.widths.iter().sum();
                        assert!(
                            (sum - a).abs() < 1e-3,
                            "avail={a}: Σ widths ({sum}) must equal available"
                        );
                    }
                    // Reference: sum_min = {sum_min:.0}, sum_max = {sum_max:.0}
                    // (compile-time constant for this fixture).
                    let _ = sum_max;
                }
            });
        });
    }

    #[test]
    fn test_render_table_with_stars_and_long_cells_e2e() {
        let ctx = egui::Context::default();
        let md = r#"| Plan Name | Monthly Premium | Annual Deductible | Max Out-of-Pocket | Quality Rating | Notes/Evaluation |
|-----------|-----------------|-------------------|---------------------|----------------|-----------------------|
| Ambetter Cascade Select Complete Gold | $891.55 | $1,000 Individual / $2,000 Family | $7,000 Indiv. / $14,000 Fam. | ★★★☆ | Good balance of low deductible and moderate premium. |
| Kaiser Permanente Cascade Complete Gold | $1,103.11 | $1,000 Individual / $2,000 Family | $7,000 Indiv. / $14,000 Fam. | ★★★★ | Excellent reputation and high quality rating. |
"#;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut scroll_id = None;
                render_markdown(ui, md, &mut scroll_id);
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

                let elems = vec![InlineElem::Text(
                    "Target Heading".to_string(),
                    TextStyle::default(),
                )];
                render_heading(ui, &elems, 1, &mut scroll_id);
                assert_eq!(
                    scroll_id, None,
                    "scroll_to_id should be cleared after scroll"
                );

                // Empty title should not trigger scroll
                let mut dummy_scroll = Some(target_id);
                render_heading(ui, &[], 1, &mut dummy_scroll);
                assert_eq!(dummy_scroll, Some(target_id));
            });
        });
    }

    /// Renders `table_cells` inside a CentralPanel with `viewport_width`
    /// and returns the `ColumnWidths` decision the renderer used.
    ///
    /// This wires the full `measure → ftwa → render` path; tests assert
    /// on the returned decision rather than on pixels (since this project
    /// is on eframe 0.27 and `egui_kittest` requires egui 0.31+).
    fn render_table_with_viewport(
        table_cells: &[Vec<Vec<InlineElem>>],
        viewport_width: f32,
    ) -> crate::ui::table_width::ColumnWidths {
        let ctx = egui::Context::default();
        let mut raw = egui::RawInput::default();
        // `screen_rect` defines the window's pixel dimensions in egui 0.27.
        // Without it, the default (small) rectangle makes `ui.available_width()`
        // unreliable for FTWA tests. Note: `ui.available_width()` inside the
        // `CentralPanel` is then `screen_rect.width() - 16px` (egui's default
        // outer margin), so e.g. a 300px screen rect yields ~284px available.
        raw.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, 600.0),
        ));
        let mut captured: Option<crate::ui::table_width::ColumnWidths> = None;
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (max_w, min_w) = crate::ui::table_width::measure(table_cells, ui);
                let gutter = 10.0_f32;
                let avail =
                    (ui.available_width() - (max_w.len() as f32 - 1.0).max(0.0) * gutter).max(0.0);
                let decision = crate::ui::table_width::ftwa(&max_w, &min_w, avail);
                captured = Some(decision.clone());
                // Render — exercises the rendering branch keyed on
                // `needs_horizontal_scroll`. Without a visual harness, we
                // can't assert on pixels, but a panic in `render_table`
                // would surface here.
                render_table(ui, table_cells);
            });
        });
        captured.expect("ctx.run should have populated `captured`")
    }

    /// Helper: build a table where every column has the same `cell_text`
    /// in both the header and the (single) data row. Used to make
    /// column-width measurements identical so the FTWA widths reflect
    /// the algorithm's own distribution rather than font-metric noise.
    fn build_uniform_table(cell_text: &str, n_columns: usize) -> Vec<Vec<Vec<InlineElem>>> {
        let make_cell = || {
            vec![InlineElem::Text(
                cell_text.to_string(),
                crate::ui::render::TextStyle::default(),
            )]
        };
        let row: Vec<Vec<InlineElem>> = (0..n_columns).map(|_| make_cell()).collect();
        vec![row.clone(), row]
    }

    /// Helper: build a table where one column (the "wide" one) has much
    /// longer text than the others. The other columns use `narrow_text`.
    fn build_dissimilar_table(narrow_text: &str, wide_text: &str) -> Vec<Vec<Vec<InlineElem>>> {
        let make = |t: &str| {
            vec![InlineElem::Text(
                t.to_string(),
                crate::ui::render::TextStyle::default(),
            )]
        };
        vec![
            vec![make(narrow_text), make(wide_text), make(narrow_text)],
            vec![make(narrow_text), make(wide_text), make(narrow_text)],
        ]
    }

    #[test]
    fn test_render_table_similar_columns_fit_viewport() {
        // 3 identical-text columns, 800px viewport → surplus regime.
        // All columns have identical text, so identical max/min widths;
        // FTWA distributes the spare equally.
        let table = build_uniform_table("name", 3);
        let d = render_table_with_viewport(&table, 800.0);
        assert!(!d.needs_horizontal_scroll, "should not scroll");
        let mn = d.widths.iter().copied().fold(f32::INFINITY, f32::min);
        let mx = d.widths.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            (mx - mn).abs() < 0.5,
            "identical columns must have equal widths; got {:?}",
            d.widths
        );
    }

    #[test]
    fn test_render_table_dissimilar_columns_fit_viewport() {
        // 1 wide + 2 narrow, 1000px viewport → surplus, wide column gets
        // the largest share of the spare.
        let table = build_dissimilar_table("a", "a much wider middle column");
        let d = render_table_with_viewport(&table, 1000.0);
        assert!(!d.needs_horizontal_scroll);
        let (mx_idx, _) = d
            .widths
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();
        assert_eq!(
            mx_idx, 1,
            "the wide column should be the widest; widths = {:?}",
            d.widths
        );
        // Wide column should be at least 2× either narrow column.
        assert!(d.widths[1] >= 2.0 * d.widths[0]);
        assert!(d.widths[1] >= 2.0 * d.widths[2]);
    }

    #[test]
    fn test_render_table_similar_columns_require_word_wrap() {
        // 3 columns of space-separated words. The longest single token
        // (a single word) is much smaller than the full line, so
        // min_content < max_content. With a small viewport we get
        // sum_min < available < sum_max → deficit regime (word wrap),
        // not §3.6 (which would only trigger if sum_min itself
        // exceeded available).
        let table = build_uniform_table("alpha beta gamma delta epsilon zeta", 3);
        let d = render_table_with_viewport(&table, 300.0);
        assert!(
            !d.needs_horizontal_scroll,
            "300px must trigger deficit, not §3.6; got {:?}",
            d.widths
        );
        // Deficit invariant: G3 sum == available.
        let sum: f32 = d.widths.iter().sum();
        assert!(sum > 0.0, "sum should be positive; got {sum}");
        assert_eq!(d.widths.len(), 3);
    }

    #[test]
    fn test_render_table_similar_columns_exceed_viewport() {
        // 3 identical wide columns, very small viewport → §3.6 fallback.
        let table = build_uniform_table("a_long_column_header_text_here_now", 3);
        // 30px viewport — far below sum_min for a 3-col table with
        // multi-char tokens. Forces the §3.6 fallback path.
        let d = render_table_with_viewport(&table, 30.0);
        assert!(
            d.needs_horizontal_scroll,
            "tiny viewport must trigger §3.6 fallback; got {:?}",
            d.widths
        );
    }

    #[test]
    fn test_render_table_dissimilar_columns_exceed_viewport() {
        // One column with very long content + tiny viewport → §3.6.
        let long = "this_is_a_very_very_very_very_long_column_header_that_will_not_fit";
        let table = build_dissimilar_table("a", long);
        let d = render_table_with_viewport(&table, 100.0);
        assert!(
            d.needs_horizontal_scroll,
            "100px viewport cannot fit a long column; got {:?}",
            d.widths
        );
    }
}
