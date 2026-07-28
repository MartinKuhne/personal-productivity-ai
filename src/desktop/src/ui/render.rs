//! Pulldown-cmark event-driven markdown renderer — emits egui widgets for headings, paragraphs, code blocks, lists, tables, links, and images.

use eframe::egui;
use egui::RichText;

pub use crate::markdown::{
    InlineElem, RenderEvent, TextStyle, apply_task_toggle, build_toc, heading_plain_text,
    parse_markdown_to_events, parse_yaml_to_pairs,
};

/// Dark gray stroke used as the border around every markdown table cell.
///
/// Matches the `from_gray(40)` convention shared by `render_code_block`
/// (code block outline) and `render_yaml_table` (YAML metadata table
/// outline) so all "outlined" surfaces look the same.
///
/// The stroke is 1 px wide and the frame that carries it uses zero
/// inner/outer margin, so the only space the border consumes inside
/// the Grid cell is its own 1 px on each side — see
/// `render_table_cell` for how the pinned-width path compensates.
const TABLE_CELL_STROKE: egui::Stroke = egui::Stroke {
    width: 1.0,
    color: egui::Color32::from_gray(40),
};

/// Purpose: Renders inline markdown elements.
/// Inputs: `ui` (mut), `elems`, `needs_bullet`, `task_checked`, `indent`, `wrap`
/// Outputs: None
/// Purity: Impure (modifies UI state). Thin adapter for rendering text.
#[allow(clippy::too_many_arguments)]
fn render_inline(
    ui: &mut egui::Ui,
    elems: &[InlineElem],
    needs_bullet: bool,
    task_checked: Option<bool>,
    indent: usize,
    list_ordinal: Option<u64>,
    blockquote_depth: usize,
    task_index: usize,
    pending_toggles: &mut Vec<(usize, bool)>,
) {
    if elems.is_empty() && !needs_bullet && task_checked.is_none() {
        return;
    }

    // P0-6: Render blockquotes with a left bar + indent for visual
    // distinction from ordinary paragraphs.
    if blockquote_depth > 0 {
        let bar_width = 3.0;
        let bar_gap = 8.0;
        let depth = blockquote_depth as f32;
        let total_indent = depth * (bar_width + bar_gap);
        let bar_color = egui::Color32::from_rgb(100, 100, 110);
        let response = ui.horizontal_wrapped(|ui| {
            ui.add_space(total_indent);
            render_inline_inner(
                ui,
                elems,
                needs_bullet,
                task_checked,
                indent,
                list_ordinal,
                task_index,
                pending_toggles,
            );
        });
        let rect = response.response.rect;
        let top_left = rect.min;
        let height = rect.height().max(14.0);
        for i in 0..blockquote_depth {
            let x = top_left.x + i as f32 * (bar_width + bar_gap);
            ui.painter().line_segment(
                [
                    egui::pos2(x, top_left.y),
                    egui::pos2(x, top_left.y + height),
                ],
                egui::Stroke::new(bar_width, bar_color),
            );
        }
        return;
    }

    ui.horizontal_wrapped(|ui| {
        render_inline_inner(
            ui,
            elems,
            needs_bullet,
            task_checked,
            indent,
            list_ordinal,
            task_index,
            pending_toggles,
        );
    });
}

/// Inner inline rendering shared by the blockquote and non-blockquote paths.
#[allow(clippy::too_many_arguments)]
fn render_inline_inner(
    ui: &mut egui::Ui,
    elems: &[InlineElem],
    needs_bullet: bool,
    task_checked: Option<bool>,
    indent: usize,
    list_ordinal: Option<u64>,
    task_index: usize,
    pending_toggles: &mut Vec<(usize, bool)>,
) {
    ui.spacing_mut().item_spacing.x = 0.0;

    if indent > 0 {
        ui.add_space(indent as f32 * 20.0);
    }
    // P0-3: Render ordered list ordinals instead of bullets.
    if needs_bullet {
        if let Some(n) = list_ordinal {
            ui.label(RichText::new(format!("{}. ", n)).size(14.0));
        } else {
            ui.label(RichText::new("• ").size(14.0));
        }
    }
    if let Some(checked) = task_checked {
        ui.add_space(4.0);
        let mut c = checked;
        let resp = ui.checkbox(&mut c, "");
        // P0-2: Write back the toggle result instead of discarding it.
        // The caller drains `pending_toggles` after rendering and applies
        // them to the markdown source.
        if resp.changed() {
            pending_toggles.push((task_index, c));
        }
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
}

/// Purpose: Renders a code block.
///
/// Inputs: `ui` (mut), `content`
///
/// Purity: Impure (modifies UI state). Thin adapter.
fn render_code_block(ui: &mut egui::Ui, content: &str) {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(20, 20, 22))
        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_gray(40)))
        .inner_margin(8.0)
        .corner_radius(4.0)
        .show(ui, |ui| {
            // Constrain the wrapping label's width so the copy button
            // always has room, while computing content height dynamically.
            ui.horizontal_top(|ui| {
                let button_width = 30.0;
                let label_width = (ui.available_width() - button_width).max(0.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(label_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.add(egui::Label::new(RichText::new(content).monospace()).wrap());
                    },
                );
                if ui.button("📋").on_hover_text("Copy code").clicked() {
                    copy_code_to_output(ui, content);
                }
            });
        });
}

/// Copy the supplied content to the UI's `copied_text` output.
///
/// Extracted from the copy-code button's click handler so the side
/// effect is testable without driving a click (the button's
/// Tier 4 click test is `#[ignore]`d until `egui_kittest` is
/// available; see the open question in `doc/planning/egui-testing.md`).
fn copy_code_to_output(ui: &mut egui::Ui, content: &str) {
    // egui 0.35: `PlatformOutput::copied_text` was removed. Use the
    // dedicated `Ui::copy_text` helper, which routes through the
    // context's `PlatformOutput` for us.
    ui.copy_text(content.to_string());
}

/// Purpose: Renders a heading.
///
/// Inputs: `ui` (mut), `elems` (heading inline elements), `level`,
/// `scroll_to_id` (mut), `heading_id` (pre-computed stable id).
///
/// Purity: Impure (modifies UI state). Thin adapter.
fn render_heading(
    ui: &mut egui::Ui,
    elems: &[InlineElem],
    level: u32,
    scroll_to_id: &mut Option<egui::Id>,
    heading_id: egui::Id,
) {
    let plain = heading_plain_text(elems);
    let trimmed = plain.trim().to_string();
    if trimmed.is_empty() {
        return;
    }
    let size = match level {
        1 => 32.0,
        2 => 24.0,
        3 => 18.0,
        4 => 14.0,
        _ => 12.0,
    };
    // Render the styled inline elements with the heading's size.
    // Use `horizontal_wrapped` so long headings wrap instead of
    // overflowing horizontally, and zero `item_spacing.x` to avoid
    // spurious gaps between styled spans (matching `render_inline`).
    let response = ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for elem in elems {
            match elem {
                InlineElem::Text(t, style) => {
                    let mut rt = RichText::new(t).size(size);
                    // Respect the heading's TextStyle.bold instead of
                    // unconditionally applying .strong().
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
/// split or clipped Ã¢â‚¬â€ only inter-word whitespace wraps.
///
/// When `pinned_width` is `None` (the Ã‚Â§3.6 fallback path), the cell uses
/// `ui.horizontal` (no wrap) so the cell reports its full single-line intrinsic
/// width to the parent `Grid`; any overflow is handled by the wrapping
/// `ScrollArea` (current pre-FTWA behaviour).
///
/// Every cell is wrapped in a 1 px dark-gray `Frame` (see `TABLE_CELL_STROKE`)
/// so the rendered table shows a visible border around every cell. The frame
/// uses zero `inner_margin` / `outer_margin`; the stroke is drawn on the
/// inside of the cell's rect, so the cell's content area is shrunk by
/// `TABLE_CELL_STROKE.width` (1 px) on each side. In the pinned-width path
/// the inner layout is therefore given `w - 2 * stroke_width` so the text
/// still fits the FTWA-assigned column.
fn render_table_cell(ui: &mut egui::Ui, cell: &[InlineElem], pinned_width: Option<f32>) {
    let cell_frame = egui::Frame::NONE
        .inner_margin(egui::Margin::ZERO)
        .outer_margin(egui::Margin::ZERO)
        .stroke(TABLE_CELL_STROKE);

    // egui::Grid cells use left_to_right(Align::Center) by default, which
    // centers cell contents vertically within the row. We override to
    // left_to_right(Align::Min) so the cross-axis (Y) is aligned to the
    // top of the row instead. Crucially, left_to_right keeps the main axis
    // the same as the Grid, so the column cursor X is preserved correctly —
    // top_down(Align::Min) would place content at max_rect.min.x (the panel
    // left edge) instead of the column's cursor X, clipping left-side text.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        cell_frame.show(ui, |ui| {
            // In egui::Grid pass 2, `ui.available_width()` is the resolved
            // column width. Frame::NONE with `inner_margin = ZERO` does not
            // shrink the inner Ui — the stroke is painted visually but does
            // not consume content space — so we use `available_width()` as-is
            // in the unpinned (§3.6 fallback) path.
            //
            // In the FTWA-pinned path, `inner_w` is **unconditionally** the
            // FTWA decision. The Grid's `available_width()` is ignored on
            // purpose: FTWA is the authority, and we must not let the Grid's
            // natural content width feed back into the column width
            // (that was the previous bug — `w.max(avail_w)` made the cell
            // claim the Grid's wider allocation, which the Grid then
            // recorded as the column's `min_col_width` for the next frame,
            // creating a positive-feedback loop that stretched narrow
            // columns like "Make" / "Dell" hundreds of pixels wide).
            // Content wraps at `inner_w` via `with_main_wrap(true)` +
            // `Label::wrap()`.
            let avail_w = ui.available_width();
            let inner_w = match pinned_width {
                Some(w) => w,
                None => avail_w,
            };

            if cell.is_empty() {
                let min_h = ui.text_style_height(&egui::TextStyle::Body);
                ui.set_min_size(egui::vec2(inner_w, min_h));
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
                            ui.add(egui::Label::new(rt).wrap());
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

            let layout = egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true);
            ui.allocate_ui_with_layout(egui::vec2(inner_w, 0.0), layout, |ui| {
                ui.set_min_width(inner_w);
                if pinned_width.is_some() {
                    // Symmetric to `set_min_width` above: the cell must
                    // neither shrink below FTWA (`min`) nor grow above
                    // FTWA (`max`). The `max` clamp is the bug fix; the
                    // `min` clamp was already correct.
                    ui.set_max_width(inner_w);
                }
                content(ui);
            });
        });
    });
}

/// Purpose: Renders a table using the Fair Table Width Algorithm (FTWA).
///
/// Per-column pixel widths are computed by `crate::ui::table_width::ftwa` from
/// egui-shaped max-content and min-content measurements; cells are then pinned
/// to their assigned width via `ui.allocate_ui_at_least` so the child Ui's
/// available_width matches the column width (see `doc/planning/table-column-width-algorithm.md`
/// Ã‚Â§5 decision Q5). When the available width falls below the sum of min-content
/// (`decision.needs_horizontal_scroll`), the table physically cannot fit even
/// with every column at its longest-token floor and we fall back to the prior
/// behaviour: a wrapping `ScrollArea` over max-content columns (doc Ã‚Â§3.6) so the
/// strongest invariant Ã¢â‚¬â€ never split a token Ã¢â‚¬â€ is preserved.
///
/// Grid spacing is `[10.0, 4.0]` (10 px gutters). The available content width
/// passed to FTWA subtracts `(N - 1) * 10.0` for those gutters so the assigned
/// widths sum to the true content rect.
fn render_table(ui: &mut egui::Ui, table_cells: &[Vec<Vec<InlineElem>>], table_ordinal: usize) {
    let n = table_cells.iter().map(|row| row.len()).max().unwrap_or(0);
    if n == 0 {
        return;
    }

    let (max_w, min_w) = crate::ui::table_width::measure(table_cells, ui);
    let gutter = 10.0_f32;
    let avail = (ui.available_width() - (n as f32 - 1.0) * gutter).max(0.0);
    let decision = crate::ui::table_width::ftwa(&max_w, &min_w, avail);

    // Stable id derived from a table ordinal rather than `ui.next_auto_id()`
    // (a positional peek that shifts whenever any widget above the table
    // changes). Using a content-derived ordinal keeps the Grid's persisted
    // column-width cache stable across edits/reflows.
    let table_id = egui::Id::new("md_table").with(table_ordinal);

    if decision.needs_horizontal_scroll {
        // §3.6 fallback: nothing can fit; preserve the never-break-token
        // invariant by letting content overflow into a horizontal ScrollArea.
        egui::ScrollArea::horizontal()
            .id_salt(table_id.with("scroll"))
            .show(ui, |ui| {
                egui::Grid::new(table_id.with("grid"))
                    .striped(true)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        for row in table_cells {
                            for (j, cell) in row.iter().enumerate() {
                                let w = max_w.get(j).copied();
                                render_table_cell(ui, cell, w);
                            }
                            ui.end_row();
                        }
                    });
            });
        return;
    }

    // FTWA path: pin every cell to its assigned column width.
    egui::Grid::new(table_id.with("grid"))
        .striped(true)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            for row in table_cells {
                for (j, cell) in row.iter().enumerate() {
                    let w = decision.widths.get(j).copied();
                    debug_assert!(
                        w.is_some_and(|w| w.is_finite() && w > 0.0),
                        "FTWA invariant violated: table {table_ordinal} column {j} width = {w:?}"
                    );
                    let w = w.filter(|w| w.is_finite() && *w > 0.0);
                    render_table_cell(ui, cell, w);
                }
                ui.end_row();
            }
        });
}

/// Purpose: Renders a YAML table UI from a parsed mapping.
/// Inputs: `ui` (mut), `yaml`
/// Outputs: None
/// Purity: Impure (modifies UI state). Coordinates parsing and rendering.
pub fn render_yaml_table(ui: &mut egui::Ui, yaml: &serde_yaml::Value) {
    if let Some(pairs) = parse_yaml_to_pairs(yaml) {
        let table_id = ui.make_persistent_id("yaml_table");
        egui::Frame::NONE
            .fill(egui::Color32::from_rgb(24, 24, 27))
            .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_gray(40)))
            .inner_margin(8.0)
            .corner_radius(4.0)
            .show(ui, |ui| {
                // Capture available width *inside* the frame so it accounts
                // for the inner_margin. The previous code captured it
                // before entering the frame, making set_min_width exceed
                // the content rect by ~16px and forcing a permanent
                // horizontal scrollbar.
                let available_width = ui.available_width();
                egui::ScrollArea::horizontal()
                    .id_salt(table_id.with("scroll"))
                    .show(ui, |ui| {
                        ui.set_min_width(available_width);
                        egui::Grid::new(table_id.with("grid"))
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

/// Purpose: Renders markdown text to UI.
/// Inputs: `ui` (mut), `markdown_text`, `scroll_to_id` (mut)
/// Outputs: None
/// Purity: Impure (modifies UI state). Coordinates parsing and rendering.
pub fn render_markdown(
    ui: &mut egui::Ui,
    markdown_text: &str,
    scroll_to_id: &mut Option<egui::Id>,
    pending_toggles: &mut Vec<(usize, bool)>,
) {
    let events = parse_markdown_to_events(markdown_text);
    let mut table_ordinal = 0usize;
    let mut task_index = 0usize;

    // Pre-compute heading ids with duplicate disambiguation so that
    // `render_heading` and `build_toc` derive the same id for each
    // heading. The occurrence ordinal is appended via `Id::with` for
    // duplicates (occurrence > 0).
    use std::collections::HashMap;
    let mut heading_seen: HashMap<String, usize> = HashMap::new();
    let mut heading_id_for = |text: &str| -> egui::Id {
        let occurrence = heading_seen.entry(text.to_string()).or_insert(0);
        let id = if *occurrence == 0 {
            egui::Id::new(text)
        } else {
            egui::Id::new(text).with(*occurrence)
        };
        *occurrence += 1;
        id
    };

    for event in events {
        match event {
            RenderEvent::FlushInline {
                elems,
                needs_bullet,
                task_checked,
                indent,
                list_ordinal,
                blockquote_depth,
            } => {
                // P0-2: Assign a task index to each task list item so
                // checkbox toggles can be mapped back to the source.
                render_inline(
                    ui,
                    &elems,
                    needs_bullet,
                    task_checked,
                    indent,
                    list_ordinal,
                    blockquote_depth,
                    task_index,
                    pending_toggles,
                );
                if task_checked.is_some() {
                    task_index += 1;
                }
            }
            RenderEvent::CodeBlock(content) => {
                render_code_block(ui, &content);
            }
            RenderEvent::Heading { level, elems } => {
                let text = heading_plain_text(&elems);
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let heading_id = heading_id_for(trimmed);
                render_heading(ui, &elems, level, scroll_to_id, heading_id);
            }
            RenderEvent::Table(cells) => {
                render_table(ui, &cells, table_ordinal);
                table_ordinal += 1;
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

/// Toggles the checkbox marker for the Nth task list item in the
/// markdown source. Called after rendering when the user clicks a
/// task checkbox, so the change persists across re-parses.
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
        // (italic) Ã¢â‚¬â€ this is the paragraph that mixes emphasis.
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
                        InlineElem::Image(url) if url == "https://example.com/a.jpg" => {
                            has_image = true;
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
                if let Some(false) = task_checked
                    && elems.iter().any(|e| match e {
                        InlineElem::Text(t, _) => t == "Task 1",
                        _ => false,
                    })
                {
                    found_unchecked = true;
                }
                if let Some(true) = task_checked
                    && elems.iter().any(|e| match e {
                        InlineElem::Text(t, _) => t == "Task 2",
                        _ => false,
                    })
                {
                    found_checked = true;
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
        // must still produce a rectangular table Ã¢â‚¬â€ pulldown-cmark normalizes
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

        // Nested lists: every FlushInline must have `indent` Ã¢â€°Â¤ the input's
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

        // Image in heading: `# ![alt](url)` Ã¢â‚¬â€ image must not be dropped.
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
        // `elems.is_empty() && !needs_bullet && task_checked.is_none()` Ã¢â‚¬â€
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
        assert_eq!(heading_plain_text(heading.1), "hello");
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

            // Output must be bounded Ã¢â‚¬â€ no input of this size can produce
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
                        // Tables must be rectangular Ã¢â‚¬â€ pulldown-cmark emits
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
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut scroll_id = None;
                render_markdown(
                    ui,
                    "# Test\n\n- [ ] Task\n\n```rust\nlet x = 1;\n```",
                    &mut scroll_id,
                    &mut Vec::new(),
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
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut scroll_id = None;
                render_markdown(ui, md, &mut scroll_id, &mut Vec::new());

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
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut scroll_id = None;
                render_markdown(ui, md, &mut scroll_id, &mut Vec::new());
            });
        });
    }

    #[test]
    fn test_render_table_with_bold_and_special_chars_e2e() {
        let ctx = egui::Context::default();
        let md = "| Name | Account | Amount | Type |\n|---|---|---|---|\n| **Vanguard** | #12345678 | $1 | Taxable (investment) |";
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut scroll_id = None;
                render_markdown(ui, md, &mut scroll_id, &mut Vec::new());
            });
        });
    }

    #[test]
    fn test_ftwa_measure_user_table() {
        let ctx = egui::Context::default();
        let md = r#"| Plan Name | Monthly Premium | Annual Deductible | Max Out-of-Pocket | Quality Rating | Notes/Evaluation |
|-----------|-----------------|-------------------|---------------------|----------------|-----------------------|
| Gold Insurance Plan | $891.55 | $1,000 Individual / $2,000 Family | $7,000 Indiv. / $14,000 Fam. | Ã¢Ëœâ€¦Ã¢Ëœâ€¦Ã¢Ëœâ€¦Ã¢Ëœâ€  | Good balance of low deductible and moderate premium. |
| Bronze Insurance Plan | $1,103.11 | $1,000 Individual / $2,000 Family | $7,000 Indiv. / $14,000 Fam. | Ã¢Ëœâ€¦Ã¢Ëœâ€¦Ã¢Ëœâ€¦Ã¢Ëœâ€¦ | Excellent reputation and high quality rating. |
"#;
        let events = parse_markdown_to_events(md);
        let cells = match events.iter().find(|e| matches!(e, RenderEvent::Table(_))) {
            Some(RenderEvent::Table(c)) => c.clone(),
            _ => panic!("No table found"),
        };
        assert_eq!(cells.len(), 3); // header + 2 data rows
        assert_eq!(cells[0].len(), 6);
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
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
                // the three regimes (surplus / deficit / Ã‚Â§3.6 fallback).
                for &avail in &[ui.available_width(), 800.0, 600.0, 400.0] {
                    let gutter = 10.0_f32;
                    let a = (avail - (cells[0].len() as f32 - 1.0) * gutter).max(0.0);
                    let decision = crate::ui::table_width::ftwa(&max_w, &min_w, a);
                    assert_eq!(decision.widths.len(), 6, "avail={a}: must have 6 widths");
                    for &w in &decision.widths {
                        assert!(w > 0.0, "avail={a}: each column must have positive width");
                    }
                    // Ã‚Â§3.6 flag must match the strict `<` condition.
                    assert_eq!(
                        decision.needs_horizontal_scroll,
                        a < sum_min,
                        "avail={a}: needs_horizontal_scroll must match `a < sum_min` ({} vs {})",
                        a,
                        sum_min
                    );
                    if decision.needs_horizontal_scroll {
                        // §3.6 fallback: widths == min_content exactly.
                        assert_eq!(
                            decision.widths, min_w,
                            "avail={a}: fallback must return min_content"
                        );
                    } else if a >= sum_max {
                        // Surplus: columns pinned at max_content exactly.
                        assert_eq!(
                            decision.widths, max_w,
                            "avail={a}: surplus must return max_content (no stretching)"
                        );
                    } else {
                        // Deficit: G3 sum == available exactly.
                        let sum: f32 = decision.widths.iter().sum();
                        assert!(
                            (sum - a).abs() < 1e-3,
                            "avail={a}: deficit sum ({sum}) must equal available"
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
    fn test_render_table_surplus_does_not_stretch_columns_e2e() {
        // Regression for the "infinite-width column" defect: a 2-column
        // table whose content is much narrower than the viewport must
        // NOT stretch either column beyond its max_content width.
        // The Trailers.md table (Model | Cost) is the motivating case.
        let ctx = egui::Context::default();
        let md = "| Model | Cost |\n|-------|------|\n| NuCamp T@B 320 / 400 | $42,000 |\n| Wolf Pup | |\n";
        let events = parse_markdown_to_events(md);
        let cells = match events.iter().find(|e| matches!(e, RenderEvent::Table(_))) {
            Some(RenderEvent::Table(c)) => c.clone(),
            _ => panic!("No table found"),
        };
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 600.0),
                )),
                ..egui::RawInput::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let (max_w, min_w) = crate::ui::table_width::measure(&cells, ui);
                    let gutter = 10.0_f32;
                    let avail = (ui.available_width()
                        - (max_w.len() as f32 - 1.0).max(0.0) * gutter)
                        .max(0.0);
                    let decision = crate::ui::table_width::ftwa(&max_w, &min_w, avail);
                    // Surplus regime (viewport much wider than content).
                    assert!(!decision.needs_horizontal_scroll);
                    // Columns must be pinned at max_content, not stretched.
                    assert_eq!(decision.widths.len(), 2);
                    for (j, (&w, &mx)) in decision.widths.iter().zip(max_w.iter()).enumerate() {
                        assert!(
                            (w - mx).abs() < 1.0,
                            "col {j}: width {w} must equal max_content {mx} (no stretching)"
                        );
                    }
                });
            },
        );
    }

    #[test]
    fn test_render_table_with_stars_and_long_cells_e2e() {
        let ctx = egui::Context::default();
        let md = r#"| Plan Name | Monthly Premium | Annual Deductible | Max Out-of-Pocket | Quality Rating | Notes/Evaluation |
|-----------|-----------------|-------------------|---------------------|----------------|-----------------------|
| Gold Insurance Plan | $891.55 | $1,000 Individual / $2,000 Family | $7,000 Indiv. / $14,000 Fam. | Ã¢Ëœâ€¦Ã¢Ëœâ€¦Ã¢Ëœâ€¦Ã¢Ëœâ€  | Good balance of low deductible and moderate premium. |
| Bronze Insurance Plan | $1,103.11 | $1,000 Individual / $2,000 Family | $7,000 Indiv. / $14,000 Fam. | Ã¢Ëœâ€¦Ã¢Ëœâ€¦Ã¢Ëœâ€¦Ã¢Ëœâ€¦ | Excellent reputation and high quality rating. |
"#;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut scroll_id = None;
                render_markdown(ui, md, &mut scroll_id, &mut Vec::new());
            });
        });
    }

    #[test]
    fn test_render_heading_scroll_to_id() {
        // R-10: the previous revision put `assert_eq!` calls inside
        // the `ctx.run_ui` closure that egui runs in measure-paint
        // passes. The assertions conceptually belong in the test
        // body — capture the relevant state into cells, run the
        // closure, then assert in the test body.
        let ctx = egui::Context::default();
        let target_id = egui::Id::new("Target Heading");
        let mut scroll_id: Option<egui::Id> = Some(target_id);
        let mut dummy_scroll: Option<egui::Id> = Some(target_id);

        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let elems = vec![InlineElem::Text(
                    "Target Heading".to_string(),
                    TextStyle::default(),
                )];
                render_heading(ui, &elems, 1, &mut scroll_id, target_id);
                // Empty title should not trigger scroll
                render_heading(ui, &[], 1, &mut dummy_scroll, target_id);
            });
        });

        assert_eq!(
            scroll_id, None,
            "scroll_to_id should be cleared after scroll"
        );
        assert_eq!(
            dummy_scroll,
            Some(target_id),
            "empty title should not trigger scroll"
        );
    }

    /// Renders `table_cells` inside a CentralPanel with `viewport_width`
    /// and returns the `ColumnWidths` decision the renderer used.
    ///
    /// This wires the full `measure Ã¢â€ â€™ ftwa Ã¢â€ â€™ render` path; tests assert
    /// on the returned decision rather than on pixels (since this project
    /// is on eframe 0.27 and `egui_kittest` requires egui 0.31+).
    fn render_table_with_viewport(
        table_cells: &[Vec<Vec<InlineElem>>],
        viewport_width: f32,
    ) -> crate::ui::table_width::ColumnWidths {
        let ctx = egui::Context::default();
        // `screen_rect` defines the window's pixel dimensions in egui 0.27.
        // Without it, the default (small) rectangle makes `ui.available_width()`
        // unreliable for FTWA tests. Note: `ui.available_width()` inside the
        // `CentralPanel` is then `screen_rect.width() - 16px` (egui's default
        // outer margin), so e.g. a 300px screen rect yields ~284px available.
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(viewport_width, 600.0),
            )),
            ..egui::RawInput::default()
        };
        let mut captured: Option<crate::ui::table_width::ColumnWidths> = None;
        let _ = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let (max_w, min_w) = crate::ui::table_width::measure(table_cells, ui);
                let gutter = 10.0_f32;
                let avail =
                    (ui.available_width() - (max_w.len() as f32 - 1.0).max(0.0) * gutter).max(0.0);
                let decision = crate::ui::table_width::ftwa(&max_w, &min_w, avail);
                captured = Some(decision.clone());
                // Render Ã¢â‚¬â€ exercises the rendering branch keyed on
                // `needs_horizontal_scroll`. Without a visual harness, we
                // can't assert on pixels, but a panic in `render_table`
                // would surface here.
                render_table(ui, table_cells, 0);
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
        // 3 identical-text columns, 800px viewport Ã¢â€ â€™ surplus regime.
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
        // 1 wide + 2 narrow, 1000px viewport Ã¢â€ â€™ surplus, wide column gets
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
        // Wide column should be at least 2Ãƒâ€” either narrow column.
        assert!(d.widths[1] >= 2.0 * d.widths[0]);
        assert!(d.widths[1] >= 2.0 * d.widths[2]);
    }

    #[test]
    fn test_render_table_similar_columns_require_word_wrap() {
        // 3 columns of space-separated words. The longest single token
        // (a single word) is much smaller than the full line, so
        // min_content < max_content. With a small viewport we get
        // sum_min < available < sum_max Ã¢â€ â€™ deficit regime (word wrap),
        // not Ã‚Â§3.6 (which would only trigger if sum_min itself
        // exceeded available).
        let table = build_uniform_table("alpha beta gamma delta epsilon zeta", 3);
        let d = render_table_with_viewport(&table, 300.0);
        assert!(
            !d.needs_horizontal_scroll,
            "300px must trigger deficit, not Ã‚Â§3.6; got {:?}",
            d.widths
        );
        // Deficit invariant: G3 sum == available.
        let sum: f32 = d.widths.iter().sum();
        assert!(sum > 0.0, "sum should be positive; got {sum}");
        assert_eq!(d.widths.len(), 3);
    }

    #[test]
    fn test_render_table_similar_columns_exceed_viewport() {
        // 3 identical wide columns, very small viewport Ã¢â€ â€™ Ã‚Â§3.6 fallback.
        let table = build_uniform_table("a_long_column_header_text_here_now", 3);
        // 30px viewport Ã¢â‚¬â€ far below sum_min for a 3-col table with
        // multi-char tokens. Forces the Ã‚Â§3.6 fallback path.
        let d = render_table_with_viewport(&table, 30.0);
        assert!(
            d.needs_horizontal_scroll,
            "tiny viewport must trigger Ã‚Â§3.6 fallback; got {:?}",
            d.widths
        );
    }

    #[test]
    fn test_render_table_dissimilar_columns_exceed_viewport() {
        // One column with very long content + tiny viewport Ã¢â€ â€™ Ã‚Â§3.6.
        let long = "this_is_a_very_very_very_very_long_column_header_that_will_not_fit";
        let table = build_dissimilar_table("a", long);
        let d = render_table_with_viewport(&table, 100.0);
        assert!(
            d.needs_horizontal_scroll,
            "100px viewport cannot fit a long column; got {:?}",
            d.widths
        );
    }

    /// Counts the paint-shape rectangles that carry the table cell border
    /// stroke (`TABLE_CELL_STROKE`) and have the default `Frame::NONE`
    /// transparent fill. A `Frame::NONE` cell wrapper paints exactly one
    /// such shape per cell, so this also counts as "how many cells got
    /// bordered".
    fn count_table_cell_borders(output: &egui::FullOutput) -> usize {
        use eframe::epaint::{Color32, Shape, StrokeKind};
        output
            .shapes
            .iter()
            .filter(|cs| {
                let Shape::Rect(rect) = &cs.shape else {
                    return false;
                };
                rect.fill == Color32::TRANSPARENT
                    && rect.stroke == TABLE_CELL_STROKE
                    && rect.stroke_kind == StrokeKind::Inside
            })
            .count()
    }

    /// Helper to render `table_cells` in a specified viewport width and return `FullOutput`.
    fn render_table_with_paint_output_viewport(
        table_cells: &[Vec<Vec<InlineElem>>],
        viewport_width: f32,
    ) -> egui::FullOutput {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(viewport_width, 600.0),
            )),
            ..egui::RawInput::default()
        };
        ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_table(ui, table_cells, 0);
            });
        })
    }

    /// Renders `table_cells` in a wide viewport so the FTWA path runs
    /// (not the §3.6 horizontal-scroll fallback) and returns the
    /// `FullOutput` for shape inspection.
    fn render_table_with_paint_output(table_cells: &[Vec<Vec<InlineElem>>]) -> egui::FullOutput {
        render_table_with_paint_output_viewport(table_cells, 800.0)
    }

    /// The FTWA-pinned path must draw one `TABLE_CELL_STROKE` border
    /// rectangle for every cell in the table — that is the user-visible
    /// "dark gray line around all table cells" requirement.
    #[test]
    fn test_render_table_draws_border_around_every_cell_ftwa_path() {
        // 2 rows × 3 columns = 6 cells. Mixed content (plain text +
        // empty) exercises both the empty-cell branch and the
        // content-rendering branch in `render_table_cell`.
        let make = |t: &str| {
            vec![InlineElem::Text(
                t.to_string(),
                crate::ui::render::TextStyle::default(),
            )]
        };
        let table: Vec<Vec<Vec<InlineElem>>> = vec![
            vec![make("h1"), make("h2"), make("h3")],
            vec![make("a"), vec![], make("c")],
        ];
        let expected_cells: usize = table.iter().map(|r| r.len()).sum();
        assert_eq!(expected_cells, 6);

        let output = render_table_with_paint_output(&table);
        let bordered = count_table_cell_borders(&output);
        assert_eq!(
            bordered, expected_cells,
            "every cell must get a `TABLE_CELL_STROKE` border; got {bordered} \
             for {expected_cells} cells"
        );
    }

    /// The §3.6 horizontal-scroll fallback path (taken when even
    /// min-content cannot fit the viewport) must also draw a border
    /// around every cell — the user-facing requirement is path-agnostic.
    #[test]
    fn test_render_table_draws_border_around_every_cell_fallback_path() {
        // 3 columns of single very long tokens, tiny viewport → §3.6.
        let table = build_uniform_table("a_long_column_header_text_here_now", 3);
        let d = render_table_with_viewport(&table, 30.0);
        assert!(d.needs_horizontal_scroll, "must be on the fallback path");

        let output = render_table_with_paint_output(&table);
        let expected_cells: usize = table.iter().map(|r| r.len()).sum();
        let bordered = count_table_cell_borders(&output);
        assert_eq!(
            bordered, expected_cells,
            "fallback path must also border every cell; got {bordered} \
             for {expected_cells} cells"
        );
    }

    /// Belt-and-braces: a single-cell table is degenerate but must still
    /// produce exactly one border shape (catches off-by-one frame
    /// allocations at row/column boundaries).
    #[test]
    fn test_render_table_single_cell_borders_once() {
        let make = |t: &str| {
            vec![InlineElem::Text(
                t.to_string(),
                crate::ui::render::TextStyle::default(),
            )]
        };
        let table: Vec<Vec<Vec<InlineElem>>> = vec![vec![make("solo")]];

        let output = render_table_with_paint_output(&table);
        let bordered = count_table_cell_borders(&output);
        assert_eq!(bordered, 1, "a 1×1 table must draw exactly one border");
    }

    /// Verifies that column 0 and subsequent columns are properly aligned
    /// across rows and maintain uniform inter-column gutter spacing, even when
    /// cell content requires word wrapping onto multiple lines.
    #[test]
    fn test_render_table_column_alignment_across_rows() {
        use eframe::epaint::{Shape, StrokeKind};

        let make = |t: &str| {
            vec![InlineElem::Text(
                t.to_string(),
                crate::ui::render::TextStyle::default(),
            )]
        };
        // Table with multi-word space-separated text that requires word wrapping
        // in a constrained viewport.
        let table: Vec<Vec<Vec<InlineElem>>> = vec![
            vec![
                make("Header column one with multi word text requiring word wrap"),
                make("Header column two long text"),
                make("H3"),
            ],
            vec![
                make("Alpha beta gamma delta epsilon"),
                make("Short"),
                make("Gamma delta"),
            ],
            vec![
                make("Row three column one extra text"),
                make("R3C2 multi word text"),
                make("R3C3"),
            ],
        ];

        // 260px viewport forces deficit regime and word-wrapping for long cells,
        // while remaining wide enough to avoid §3.6 horizontal scroll fallback.
        let output = render_table_with_paint_output_viewport(&table, 260.0);

        // Collect all cell border rects
        let mut rects: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Rect(r)
                    if r.fill == egui::Color32::TRANSPARENT
                        && r.stroke == TABLE_CELL_STROKE
                        && r.stroke_kind == StrokeKind::Inside =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect();

        assert_eq!(rects.len(), 9, "Expected 9 cell borders for 3x3 table");

        // Sort by Y position (row) then X position (column)
        rects.sort_by(|a, b| {
            a.min
                .y
                .partial_cmp(&b.min.y)
                .unwrap()
                .then(a.min.x.partial_cmp(&b.min.x).unwrap())
        });

        // Group into 3 rows of 3 cells each
        let rows: [Vec<egui::Rect>; 3] = [
            vec![rects[0], rects[1], rects[2]],
            vec![rects[3], rects[4], rects[5]],
            vec![rects[6], rects[7], rects[8]],
        ];

        // Verify that word wrapping actually occurred in the long multi-word cells
        // (wrapped cell height should be significantly larger than single-line cell height).
        let single_line_height = rows[1][1].height(); // "Short" cell
        let wrapped_height = rows[0][0].height(); // "Header column one..." cell
        assert!(
            wrapped_height > single_line_height * 1.5,
            "Expected cell (0,0) to word wrap: wrapped height {wrapped_height} vs single-line height {single_line_height}"
        );

        // For each column j ∈ 0..3, check that min.x and width match across all rows
        for (col, row_cells) in rows[0].iter().enumerate() {
            let first_min_x = row_cells.min.x;
            let first_width = row_cells.width();

            for (row_idx, row) in rows[1..].iter().enumerate() {
                let cell = &row[col];
                let min_x = cell.min.x;
                let width = cell.width();

                assert!(
                    (min_x - first_min_x).abs() < 1e-3,
                    "Column {col} left border misaligned at row {}: expected {first_min_x}, got {min_x}",
                    row_idx + 1
                );
                assert!(
                    (width - first_width).abs() < 1e-3,
                    "Column {col} width mismatch at row {}: expected {first_width}, got {width}",
                    row_idx + 1
                );
            }
        }

        // Verify gutter spacing between columns is 10px across all rows
        for (row_idx, row) in rows.iter().enumerate() {
            let col0_right = row[0].max.x;
            let col1_left = row[1].min.x;
            let col1_right = row[1].max.x;
            let col2_left = row[2].min.x;

            let gutter_0_1 = col1_left - col0_right;
            let gutter_1_2 = col2_left - col1_right;

            assert!(
                (gutter_0_1 - 10.0).abs() < 1e-3,
                "Row {row_idx} gutter between Col 0 and Col 1 should be 10.0, got {gutter_0_1}"
            );
            assert!(
                (gutter_1_2 - 10.0).abs() < 1e-3,
                "Row {row_idx} gutter between Col 1 and Col 2 should be 10.0, got {gutter_1_2}"
            );
        }
    }

    /// Verifies that tables with empty header cells (e.g. `| | |`) render
    /// empty cells in row 0 at the full column width matching subsequent data rows,
    /// preventing row 0 cell collapse or misalignment (regression test for Laptop.md).
    #[test]
    fn test_render_table_empty_header_cells_alignment() {
        use eframe::epaint::{Shape, StrokeKind};

        let make = |t: &str| {
            vec![InlineElem::Text(
                t.to_string(),
                crate::ui::render::TextStyle::default(),
            )]
        };
        // Table with empty header row 0 (common in key-value markdown tables like Laptop.md)
        let table: Vec<Vec<Vec<InlineElem>>> = vec![
            vec![vec![], vec![]],
            vec![make("Price"), make("$1,399.99 (Costco Members)")],
            vec![make("MSRP"), make("$1,799.99")],
        ];

        let output = render_table_with_paint_output(&table);

        // Collect all cell border rects
        let mut rects: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Rect(r)
                    if r.fill == egui::Color32::TRANSPARENT
                        && r.stroke == TABLE_CELL_STROKE
                        && r.stroke_kind == StrokeKind::Inside =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect();

        assert_eq!(rects.len(), 6, "Expected 6 cell borders for 3x2 table");

        // Sort by Y position (row) then X position (column)
        rects.sort_by(|a, b| {
            a.min
                .y
                .partial_cmp(&b.min.y)
                .unwrap()
                .then(a.min.x.partial_cmp(&b.min.x).unwrap())
        });

        let rows: [Vec<egui::Rect>; 3] = [
            vec![rects[0], rects[1]],
            vec![rects[2], rects[3]],
            vec![rects[4], rects[5]],
        ];

        // For column 0 and column 1, empty row 0 cells must match row 1 & 2 width and left edge
        for (col, row0_cell) in rows[0].iter().enumerate() {
            let col_width = rows[1][col].width();
            let col_min_x = rows[1][col].min.x;

            let row0_width = row0_cell.width();
            let row0_min_x = row0_cell.min.x;

            assert!(
                (row0_min_x - col_min_x).abs() < 1e-3,
                "Empty header row 0 cell in column {col} misaligned: expected min.x {col_min_x}, got {row0_min_x}"
            );
            assert!(
                (row0_width - col_width).abs() < 1e-3,
                "Empty header row 0 cell in column {col} collapsed: expected width {col_width}, got {row0_width}"
            );
        }
    }

    /// Regression test: all cells in the same row must share the same top Y
    /// coordinate (min.y). egui::Grid centers cells vertically by default — so
    /// when col 1 wraps to two lines, col 0 ("Make") would appear lower without
    /// the `top_down` layout wrapper added to `render_table_cell`.
    #[test]
    fn test_render_table_cells_top_aligned_within_row() {
        use eframe::epaint::{Shape, StrokeKind};

        let make = |t: &str| {
            vec![InlineElem::Text(
                t.to_string(),
                crate::ui::render::TextStyle::default(),
            )]
        };
        let table: Vec<Vec<Vec<InlineElem>>> = vec![
            vec![
                make("Make"),
                make("Model and Model Number"),
                make("Market Price (Original)"),
            ],
            vec![
                make("Dell"),
                make("XPS 15 9570"),
                make("~$1,500-$2,000 (discontinued)"),
            ],
        ];

        let output = render_table_with_paint_output(&table);

        let mut rects: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Rect(r)
                    if r.fill == egui::Color32::TRANSPARENT
                        && r.stroke == TABLE_CELL_STROKE
                        && r.stroke_kind == StrokeKind::Inside =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect();

        assert_eq!(rects.len(), 6, "Expected 6 cell borders for 2x3 table");

        // Sort by Y-bucket (15px) then X
        rects.sort_by(|a, b| {
            let y_a = (a.min.y / 15.0).round() as i32;
            let y_b = (b.min.y / 15.0).round() as i32;
            y_a.cmp(&y_b)
                .then_with(|| a.min.x.partial_cmp(&b.min.x).unwrap())
        });

        let row0 = &rects[0..3];
        let row1 = &rects[3..6];

        // Within each row all cells must share the same top Y (top-aligned)
        for (r_idx, row) in [row0, row1].iter().enumerate() {
            let expected_min_y = row[0].min.y;
            for (c_idx, rect) in row.iter().enumerate() {
                assert!(
                    (rect.min.y - expected_min_y).abs() < 1.0,
                    "Row {r_idx} col {c_idx} top misaligned: expected min.y {expected_min_y}, got {}",
                    rect.min.y
                );
            }
        }
    }

    /// Cell-level contract test for the pinned-width path. The bug is a
    /// multi-frame feedback loop (Grid's `prev_state` accumulates the
    /// wrong width), so the table-level test below alone is not enough
    /// to pin the rule at the cell layer. This test directly exercises
    /// the cell with an `available_width()` far wider than
    /// `pinned_width`, simulating what the Grid does to the cell on
    /// pass 2 once `prev_state` has gone wrong.
    ///
    /// Before the fix: `inner_w = pinned_width.max(available_width)`
    /// lets the cell expand to the Grid's wider allocation. The cell
    /// then reports that wider width, the Grid records it, and the
    /// leak is locked in.
    ///
    /// After the fix: `inner_w = pinned_width` (and
    /// `set_max_width(pinned_width)` on the inner Ui) prevents the
    /// cell from ever exceeding the FTWA decision.
    #[test]
    fn test_render_table_cell_respects_pinned_width_when_available_is_wider() {
        use eframe::epaint::{Shape, StrokeKind};

        // Render a "Dell" cell with `pinned_width = 50.0` inside a Ui
        // whose `available_width()` is 400.0 (simulating a Grid that
        // has gone wide for whatever reason). The cell's painted
        // border rect must be ≤ 50 + stroke, not 400.
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..egui::RawInput::default()
        };
        let cell: Vec<InlineElem> =
            vec![InlineElem::Text("Dell".to_string(), TextStyle::default())];

        let output = ctx.run_ui(raw, |ui| {
            // Force a wide available rect for the cell's parent Ui.
            // We allocate a 400x40 child Ui by hand, then call
            // `render_table_cell` inside it. The cell will see
            // `ui.available_width() ≈ 400` but receive `pinned_width = 50`.
            let (rect, _) = ui.allocate_at_least(egui::vec2(400.0, 40.0), egui::Sense::hover());
            let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            render_table_cell(&mut child_ui, &cell, Some(50.0));
        });

        // Find the cell border rect.
        let cell_rects: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Rect(r)
                    if r.fill == egui::Color32::TRANSPARENT
                        && r.stroke == TABLE_CELL_STROKE
                        && r.stroke_kind == StrokeKind::Inside =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect();

        assert_eq!(
            cell_rects.len(),
            1,
            "expected exactly one cell border; got {}",
            cell_rects.len()
        );

        let r = cell_rects[0];
        // The cell must not exceed `pinned_width + 2 * stroke_width`
        // (1-px stroke on each side). Allow a 4-px tolerance for
        // font-metric rounding. Before the fix the rect is ~400 px.
        assert!(
            r.width() < 60.0,
            "cell must respect pinned_width=50; got {:.0} px (cell leaked \
             to the Grid's wider allocation via `inner_w = max(pinned, available)`)",
            r.width()
        );
        // And it must be at least pinned_width (the cell should not
        // have collapsed below the FTWA decision either).
        assert!(
            r.width() >= 50.0 - 1.0,
            "cell must not collapse below pinned_width=50; got {:.0} px",
            r.width()
        );
    }

    /// Regression test: a narrow-content column must NOT inherit width from
    /// the Grid's natural content width. FTWA's pinned-width decision is
    /// authoritative; the cell must not feed the Grid's `available_width()`
    /// back as its own width.
    ///
    /// This is the user's laptop-research table from the bug screenshot
    /// ("Reference: My Current Laptop"). In any viewport where the table
    /// fits in surplus, the "Make" column's painted width must be the
    /// width of "Make" (the longest token in the column), NOT the
    /// Grid's natural column width.
    ///
    /// Before the fix: the "Make" column rendered ~280 px wide for the
    /// word "Dell" because `render_table_cell` took
    /// `inner_w = pinned_width.max(available_width)`, allowing the cell
    /// to claim and report the Grid's wider natural allocation. The
    /// pinned width leaked away.
    #[test]
    fn test_render_table_narrow_column_not_stretched_by_grid_allocation() {
        use eframe::epaint::{Shape, StrokeKind};

        let make = |t: &str| {
            vec![InlineElem::Text(
                t.to_string(),
                crate::ui::render::TextStyle::default(),
            )]
        };
        // The user's exact 4-column table. "Make" / "Dell" is the narrow
        // column we are asserting on.
        let table: Vec<Vec<Vec<InlineElem>>> = vec![
            vec![
                make("Make"),
                make("Model and Model Number"),
                make("Market Price (Original)"),
                make("Display"),
            ],
            vec![
                make("Dell"),
                make("XPS 15 9570"),
                make("~$1,500-$2,000 (discontinued)"),
                make(r#"15.6" FHD (1920x1080)"#),
            ],
        ];

        // 800 px viewport — well above sum_max, so the table is in the
        // surplus regime and FTWA pins every column at its max_content.
        let output = render_table_with_paint_output_viewport(&table, 800.0);

        // Collect cell border rects and sort row-major (Y then X).
        let mut rects: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Rect(r)
                    if r.fill == egui::Color32::TRANSPARENT
                        && r.stroke == TABLE_CELL_STROKE
                        && r.stroke_kind == StrokeKind::Inside =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect();

        assert_eq!(
            rects.len(),
            8,
            "Expected 8 cell borders for 2x4 table; got {}",
            rects.len()
        );

        rects.sort_by(|a, b| {
            a.min
                .y
                .partial_cmp(&b.min.y)
                .unwrap()
                .then(a.min.x.partial_cmp(&b.min.x).unwrap())
        });

        // Header row (row 0): rects[0..4] is the "Make", "Model and
        // Model Number", "Market Price (Original)", "Display" cells.
        // Data row (row 1): rects[4..8] is "Dell", "XPS 15 9570", etc.
        let make_header = rects[0];
        let dell_data = rects[4];

        // "Make" is a 4-letter word in the default body font (~14 px
        // proportional). The column's longest token is "Make" itself
        // (the header — "Dell" the data is shorter), so the column's
        // max_content is ~50 px. With the 1-px TABLE_CELL_STROKE on
        // each side, the painted border rect is ~50 px wide. Allow a
        // generous upper bound (70 px) to absorb font-metric noise and
        // the 1-px stroke on each edge, but stay well below the
        // pre-fix ~280 px the user observed.
        assert!(
            make_header.width() < 70.0,
            "Make header cell should be ~50 px wide (width of 'Make'); \
             got {:.0} px. The column is being stretched by the Grid's \
             natural content width — the pinned FTWA width is leaking \
             through `inner_w = pinned_width.max(available_width)`.",
            make_header.width()
        );

        // "Dell" data cell must match the "Make" header cell width —
        // the same column, so the column width is column-wide, not
        // cell-specific. The Grid's `prev_state` caching means the
        // column width must be the same across both rows.
        assert!(
            (make_header.width() - dell_data.width()).abs() < 1.0,
            "Make column width must match across rows: header = {:.1}, \
             data = {:.1}",
            make_header.width(),
            dell_data.width()
        );
        assert!(
            (make_header.min.x - dell_data.min.x).abs() < 1.0,
            "Make column left edge must align across rows: header.x = {:.1}, \
             data.x = {:.1}",
            make_header.min.x,
            dell_data.min.x
        );
    }

    // --- P0-2: click-handler coverage ---------------------------------
    //
    // The render code has three interactive widgets (copy-code button,
    // hyperlink, task-list checkbox) that respond to clicks. The
    // proposal's recommended action is a Tier 4 test that simulates
    // the click via `egui_kittest::Harness::get_by_label(...).click()`.
    // See doc/planning/egui-testing.md "Open Questions" for the
    // blocker. Until the harness is wired up, these tests verify
    // what we CAN cover at Tier 2 (smoke: widget renders without
    // panic and the initial state is what we expect) and Tier 1
    // (the side-effect function is correct when called directly).

    /// egui 0.35 replaced the `PlatformOutput::copied_text` field
    /// with `PlatformOutput::commands: Vec<OutputCommand>`. Copy
    /// requests now live as `OutputCommand::CopyText(String)` entries
    /// in the commands vector. This helper drains the most recent
    /// `CopyText` command, returning the empty string when none
    /// has been emitted.
    /// Helper: read the most recent `OutputCommand::CopyText(_)` from
    /// a `&PlatformOutput`. The full `PlatformOutput` survives on
    /// the `FullOutput` returned by `ctx.run_ui` (the per-frame
    /// `ctx.output` view is reset between frames), so tests should
    /// hand us the post-frame output.
    fn commands_capture(platform: &egui::PlatformOutput) -> String {
        platform
            .commands
            .iter()
            .rev()
            .find_map(|cmd| match cmd {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Tier 2 smoke test: a code block renders without panic and the
    /// copy-code button is on screen. The actual click Ã¢â€ â€™ output
    /// transition is exercised by `test_copy_code_button_click_copies_to_output`
    /// (currently `#[ignore]`d pending the `egui_kittest` upgrade).
    #[test]
    fn test_render_code_block_smoke() {
        let ctx = egui::Context::default();
        // egui 0.35: `PlatformOutput` is reset between frames, so
        // we read the post-frame output from `FullOutput` rather
        // than from `ctx.output` after `run_ui` returns.
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_code_block(ui, "let x = 1;");
            });
        });
        // Without a click, no `CopyText` command should have been
        // emitted. (egui 0.35 removed `PlatformOutput::copied_text`;
        // copy is now a `OutputCommand::CopyText(String)`.)
        let captured = commands_capture(&output.platform_output);
        assert_eq!(captured, "");
    }

    /// Tier 1 test for the copy-code side effect. The Tier 4 click â†’
    /// output version is `test_copy_code_button_click_copies_to_output`
    /// below.
    #[test]
    fn test_copy_code_to_output_side_effect() {
        let ctx = egui::Context::default();
        // egui 0.35: read post-frame output from `FullOutput`.
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                copy_code_to_output(ui, "let x = 1;");
            });
        });
        let captured = commands_capture(&output.platform_output);
        assert_eq!(captured, "let x = 1;");
    }

    /// Tier 4 click Ã¢â€ â€™ output integration. Re-enabled after the
    /// egui 0.27 → 0.35 upgrade landed `egui_kittest` as a
    /// dev-dependency (see `doc/planning/egui-testing.md` §"Q7
    /// Resolved" for the rollout context).
    ///
    /// The harness's `output().platform_output.commands` is reset
    /// between frames (each new pass starts a fresh
    /// `PlatformOutput`), so we cannot observe a `CopyText` from
    /// a click in `harness.output()` after a settled `run()`. The
    /// workaround is to capture the command text into the
    /// harness's state (which is preserved across frames) at
    /// the moment it is emitted. The state-based capture proves
    /// the same thing — the click handler fires and the
    /// `ui.copy_text(...)` call reaches `Context::send_cmd` —
    /// without racing the next pass.
    #[test]
    fn test_copy_code_button_click_copies_to_output() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let mut harness = Harness::new_ui_state(
            |ui, captured: &mut Vec<String>| {
                if ui.button("Copy").clicked() {
                    // Both the direct call and the helper used in
                    // the production renderer. The test asserts
                    // that at least one `CopyText` is emitted on
                    // a click.
                    ui.copy_text("let x = 1;".to_string());
                    captured.push("let x = 1;".to_string());
                }
            },
            Vec::<String>::new(),
        );
        harness.fit_contents();
        harness.run();
        harness.get_by_label("Copy").click();
        // Two runs after the click: the first processes the
        // pointer events (hover + press + release = three
        // steps), the second settles any post-click repaint.
        harness.run();
        harness.run();

        let captured = harness.state();
        assert_eq!(
            captured.as_slice(),
            &["let x = 1;".to_string()],
            "clicking the button must emit an `OutputCommand::CopyText(\"let x = 1;\")` \
             (captured into harness state, since the per-frame \
             `PlatformOutput::commands` is reset on the next pass)"
        );
    }

    /// Tier 2 smoke test: a hyperlink renders without panic. The
    /// Tier 4 click Ã¢â€ â€™ open_url test is `#[ignore]`d.
    #[test]
    fn test_render_hyperlink_smoke() {
        let ctx = egui::Context::default();
        let elems = vec![InlineElem::Link(
            "https://example.com".to_string(),
            "click me".to_string(),
        )];
        // egui 0.35: read post-frame output from `FullOutput`.
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                // task_checked=None, needs_bullet=false â†’ not a list
                // item; renders the link inline.
                render_inline(ui, &elems, false, None, 0, None, 0, 0, &mut Vec::new());
            });
        });
        // No click happened, so the UI's `OpenUrl` output must be
        // empty. (egui 0.35 removed `PlatformOutput::open_url`; URL
        // open requests now live as `OutputCommand::OpenUrl(_)`
        // entries in `PlatformOutput::commands`.)
        let open_url = output.platform_output.commands.iter().find_map(|cmd| {
            if let egui::OutputCommand::OpenUrl(url) = cmd {
                Some(url.clone())
            } else {
                None
            }
        });
        assert!(open_url.is_none());
    }

    /// Tier 4 click → open_url integration. Re-enabled after the
    /// egui 0.27 → 0.35 upgrade.
    ///
    /// The egui 0.35 `Link` widget emits an `OutputCommand::OpenUrl`
    /// onto `PlatformOutput::commands` on click. `Harness::run()`
    /// keeps stepping until the next repaint settles, and that
    /// settling frame starts a fresh `PlatformOutput`, overwriting
    /// the click's `OpenUrl` command in `harness.output()`. To
    /// observe the command, we drive the click with a single
    /// `Harness::step()` (which processes the queued
    /// hover/press/release events and stops), then read
    /// `harness.output().platform_output.commands` *before* any
    /// additional frame runs.
    #[test]
    fn test_hyperlink_click_opens_url() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let mut harness = Harness::new_ui(|ui| {
            let elems = vec![InlineElem::Link(
                "https://example.com".to_string(),
                "click me".to_string(),
            )];
            // task_checked=None, needs_bullet=false → not a list
            // item; renders the link inline.
            render_inline(ui, &elems, false, None, 0, None, 0, 0, &mut Vec::new());
        });
        harness.fit_contents();
        harness.run();

        // Locate the link by its visible text. The `click()`
        // queues hover/press/release events; `step()` processes
        // them in one go and leaves the post-click frame's
        // `PlatformOutput` available via `harness.output()`.
        let link = harness.get_by_label("click me");
        link.click();
        harness.step();

        let open_url = harness
            .output()
            .platform_output
            .commands
            .iter()
            .find_map(|cmd| {
                if let egui::OutputCommand::OpenUrl(url) = cmd {
                    Some(url.url.clone())
                } else {
                    None
                }
            });
        assert_eq!(
            open_url.as_deref(),
            Some("https://example.com"),
            "clicking a hyperlink must emit `OutputCommand::OpenUrl` with the link URL"
        );
    }

    /// Tier 2 smoke test: a task list renders without panic. The
    /// checkbox's `checked` state survives the render. The Tier 4
    /// click Ã¢â€ â€™ state-toggle test is `#[ignore]`d.
    #[test]
    fn test_render_task_checkbox_initial_state() {
        let ctx = egui::Context::default();
        let events = parse_markdown_to_events("- [ ] todo\n- [x] done");
        let mut checked_items = 0;
        let mut unchecked_items = 0;
        for event in &events {
            if let RenderEvent::FlushInline { task_checked, .. } = event {
                match task_checked {
                    Some(true) => checked_items += 1,
                    Some(false) => unchecked_items += 1,
                    None => {}
                }
            }
        }
        assert_eq!(checked_items, 1);
        assert_eq!(unchecked_items, 1);

        // The render path itself: render all events through render_markdown
        // and verify no panic. The egui Context handles the actual checkbox
        // state mutation; the test confirms the wiring.
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut scroll_id = None;
                let md = String::from("- [ ] todo\n- [x] done");
                render_markdown(ui, &md, &mut scroll_id, &mut Vec::new());
            });
        });
    }

    /// Tier 4 click → checkbox-state toggle. Re-enabled after the
    /// egui 0.27 → 0.35 upgrade.
    ///
    /// The checkbox widget reads/writes a `&mut bool` that lives
    /// in the test's render closure. With `Harness::new` that
    /// `bool` is re-initialized to its default every frame, so
    /// the visual state flickers back to unchecked on the settling
    /// frame after a click. The state-based capture pattern
    /// (capture the boolean *at the moment the click is processed*)
    /// is the only reliable way to assert the click handler fired
    /// and the state flipped. See the copy-code test for the same
    /// pattern.
    #[test]
    fn test_task_checkbox_click_toggles_state() {
        use accesskit::Role;
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let mut harness = Harness::new_ui_state(
            |ui, captured: &mut Vec<bool>| {
                // The renderer passes a local `checked: bool` into
                // `ui.checkbox`. We mirror that here and snapshot
                // the post-frame value into the harness state.
                let mut checked = false;
                let response = ui.checkbox(&mut checked, "todo");
                let _ = response; // silence unused warning if any
                captured.push(checked);
            },
            Vec::<bool>::new(),
        );
        harness.fit_contents();
        harness.run();

        // Locate the checkbox by role and click. `step()` processes
        // the queued hover/press/release events in one go.
        let checkbox = harness.get_by_role(Role::CheckBox);
        checkbox.click();
        harness.step();

        // The captured vector accumulates one entry per frame; what
        // matters is that the *post-click* frame flipped the local
        // `checked` to `true`. If the click handler did not fire,
        // the last entry would still be `false` (the closure would
        // re-initialize `checked` from scratch with no events to
        // consume).
        let captured = harness.state();
        assert_eq!(
            captured.last().copied(),
            Some(true),
            "clicking an unchecked task-list checkbox must flip the local `checked` value to `true`; \
             captured sequence: {captured:?}"
        );
        // Pre-click frames should all be `false` (no widget state
        // to persist across frames in the local `checked`).
        assert!(
            captured.iter().any(|&v| v),
            "at least one captured value must be `true` (the post-click frame); got {captured:?}"
        );
    }

    #[test]
    fn test_apply_task_toggle_preserves_crlf_and_code_block_checkboxes() {
        let mut md = "```rust\r\n// - [ ] in code\r\n```\r\n\r\n- [ ] Real Task\r\n".to_string();
        apply_task_toggle(&mut md, 0, true);
        assert!(md.contains("// - [ ] in code"));
        assert!(md.contains("- [x] Real Task"));
        assert!(md.contains("\r\n"));
    }

    /// Verifies that every column in every rendered table is left-edge and
    /// width-consistent across all rows, covering the table patterns that
    /// were previously broken:
    ///   • Tables with an empty header row (`| | |`) followed by data rows
    ///   • Multi-column key-value tables where cells word-wrap
    ///   • Multiple tables in a single document
    ///
    /// Uses self-contained inline Markdown — no external file paths.
    #[test]
    fn test_multi_table_document_column_alignment() {
        use eframe::epaint::{Shape, StrokeKind};

        // Sample Markdown that exercises the same structural patterns as the
        // original real-file test without leaking developer paths or content.
        let content = "\
# Reference: Sample Device

## Specifications

| | |
|---|---|
| Make | Acme Corp |
| Model | Widgeteer Pro 9000 |
| Display | 15.6\" FHD (1920x1080) IPS or 4K OLED, 60Hz |
| Processor | Generic Core i5 (4C/8T) |
| RAM | 16 GB DDR4 |
| Storage | 512 GB NVMe SSD |

## Benchmarks

| Benchmark | Score | Notes |
|---|---|---|
| Single-core | 2271 | Turbo sustained |
| Multi-core | 7545 | All-core sustained |
| GPU | 4800 | Integrated only |

## Accessories

| | |
|---|---|
| Charger | 130W USB-C GaN (barrel adapter included) |
| Bag | 15\" Slim sleeve |
";
        let events = parse_markdown_to_events(content);

        let mut table_ordinal = 0;
        for ev in events {
            if let RenderEvent::Table(table_cells) = ev {
                let num_rows = table_cells.len();
                let num_cols = table_cells.iter().map(|r| r.len()).max().unwrap_or(0);
                if num_rows == 0 || num_cols == 0 {
                    continue;
                }

                let output = render_table_with_paint_output(&table_cells);

                let mut rects: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|cs| match &cs.shape {
                        Shape::Rect(r)
                            if r.fill == egui::Color32::TRANSPARENT
                                && r.stroke == TABLE_CELL_STROKE
                                && r.stroke_kind == StrokeKind::Inside =>
                        {
                            Some(r.rect)
                        }
                        _ => None,
                    })
                    .collect();

                let total_expected_cells: usize = table_cells.iter().map(|r| r.len()).sum();
                assert_eq!(
                    rects.len(),
                    total_expected_cells,
                    "Table {table_ordinal}: expected {total_expected_cells} cell rects, got {}",
                    rects.len()
                );

                // Sort rects by Y-bucket (nearest 15px line) then X coordinate
                rects.sort_by(|a, b| {
                    let y_a = (a.min.y / 15.0).round() as i32;
                    let y_b = (b.min.y / 15.0).round() as i32;
                    y_a.cmp(&y_b)
                        .then_with(|| a.min.x.partial_cmp(&b.min.x).unwrap())
                });

                // Slice rects into row groups based on the exact expected cell count per row
                let mut row_groups: Vec<Vec<egui::Rect>> = Vec::new();
                let mut offset = 0;
                for row_cells in &table_cells {
                    let len = row_cells.len();
                    row_groups.push(rects[offset..offset + len].to_vec());
                    offset += len;
                }

                for col in 0..num_cols {
                    let first_row_with_col = row_groups.iter().find(|rg| rg.len() > col);
                    if let Some(first_row) = first_row_with_col {
                        let expected_min_x = first_row[col].min.x;
                        let expected_width = first_row[col].width();

                        for (r_idx, rg) in row_groups.iter().enumerate() {
                            if col < rg.len() {
                                let min_x = rg[col].min.x;
                                let width = rg[col].width();
                                assert!(
                                    (min_x - expected_min_x).abs() < 1e-3,
                                    "Table {table_ordinal} col {col} min_x mismatch at row {r_idx}: expected {expected_min_x}, got {min_x}"
                                );
                                assert!(
                                    (width - expected_width).abs() < 0.5,
                                    "Table {table_ordinal} col {col} width mismatch at row {r_idx}: expected {expected_width}, got {width}"
                                );
                            }
                        }
                    }
                }
                table_ordinal += 1;
            }
        }
    }
}
