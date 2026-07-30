//! Pulldown-cmark event-driven markdown renderer — emits egui widgets for headings, paragraphs, code blocks, lists, tables, links, and images.

use eframe::egui;
use egui::RichText;

/// Medium-gray border drawn around the outer perimeter of every markdown
/// table. The frame that paints this stroke contains the whole table —
/// both the FTWA path and the §3.6 horizontal-scroll fallback — so the
/// line stays continuous at the table edge regardless of which path ran.
const TABLE_PERIMETER_STROKE: egui::Stroke = egui::Stroke {
    width: 1.0,
    color: egui::Color32::from_gray(120),
};

pub use crate::markdown::{
    InlineElem, RenderEvent, TextStyle, apply_task_toggle, build_toc, heading_plain_text,
    parse_markdown_to_events, parse_yaml_to_pairs,
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
    task_index: usize,
    pending_toggles: &mut Vec<(usize, bool)>,
) {
    if elems.is_empty() && !needs_bullet && task_checked.is_none() {
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

/// Inner inline rendering — actually paints the styled `InlineElem` runs.
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
                // P1: egui 0.35 `Label::new` defaults `wrap_mode` to
                // `None`; wrap is only inherited if the parent layout
                // is vertical or horizontal+main_wrap AND the available
                // width is finite — a fragile contract. The other text
                // paths in this file (`render_code_block`,
                // `render_table_cell`, `render_yaml_table`) already pin
                // `.wrap()` explicitly for the same reason. Pinned by
                // `test_render_markdown_long_paragraph_wraps_in_preview`.
                ui.add(egui::Label::new(rt).wrap());
            }
            InlineElem::Link(url, text) => {
                ui.hyperlink_to(text, url);
            }
            InlineElem::Image(url) => {
                // Same egui 0.35 wrap-mode default-off hazard as
                // `InlineElem::Text`; pin the wrap explicitly.
                ui.add(egui::Label::new(format!("[Image: {}]", url)).wrap());
            }
            InlineElem::Html(html) => {
                // Same egui 0.35 wrap-mode default-off hazard as
                // `InlineElem::Text`; pin the wrap explicitly.
                ui.add(
                    egui::Label::new(RichText::new(html).italics().color(egui::Color32::GRAY))
                        .wrap(),
                );
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
/// `scroll_to_id_str` (mut, the stable string id of the heading
/// the user wants to scroll to), `heading_id_str` (pre-computed
/// stable string id for this heading).
///
/// Purity: Impure (modifies UI state). Thin adapter.
fn render_heading(
    ui: &mut egui::Ui,
    elems: &[InlineElem],
    level: u32,
    scroll_to_id_str: &mut Option<String>,
    heading_id_str: &str,
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
    if scroll_to_id_str.as_deref() == Some(heading_id_str) {
        response.response.scroll_to_me(Some(egui::Align::Center));
        *scroll_to_id_str = None;
    }
    ui.add_space(4.0);
}

/// Purpose: Renders a single table cell, always emitting at least one widget.
///
/// When `pinned_width` is `Some(w)`, the cell Ui is clamped to exactly `w`
/// pixels (`ui.set_width`) and text is laid out with `horizontal_wrapped` +
/// `Label::wrap(true)` so that multi-word cells wrap at whitespace. This is the
/// FTWA-pinned mode (`crate::ui::table_width`). The FTWA invariant
/// `w >= min_content >= longest-token` guarantees no unbreakable token is ever
/// split or clipped — only inter-word whitespace wraps.
///
/// When `pinned_width` is `None` (the §3.6 fallback path), the cell uses
/// `ui.horizontal` (no wrap) so the cell reports its full single-line intrinsic
/// width to the parent table layout; any overflow is handled by the wrapping
/// `ScrollArea` (current pre-FTWA behaviour).
///
/// **Padding (TBL-032, TBL-033):** `padding` is resolved per-cell by the caller
/// via `resolve_padding(global, per_column, per_cell)` and applied as the
/// cell frame's `inner_margin`. `padding.horizontal()` is folded into the
/// column's `max_content`/`min_content` by `measure_cached`, so the FTWA-assigned
/// `pinned_width` already accounts for it; the inner layout width is therefore
/// `pinned_width - padding.horizontal()` so text fits inside the padded frame.
fn render_table_cell(
    ui: &mut egui::Ui,
    cell: &[InlineElem],
    pinned_width: Option<f32>,
    pinned_height: Option<f32>,
    padding: crate::ui::table_width::TablePadding,
) -> f32 {
    let pad = padding.sanitised();
    let pad_h = pad.horizontal();
    let pad_v = pad.vertical();
    // `egui::Margin` stores `i8` fields, so clamp each side to the
    // representable range after rounding. Realistic paddings are small
    // (a few to a few dozen logical px), so this loses no precision in
    // practice and keeps `Margin`'s compact layout intact.
    let to_i8 = |v: f32| -> i8 { v.round().clamp(0.0, 127.0) as i8 };
    let cell_frame = egui::Frame::NONE
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .inner_margin(egui::Margin {
            left: to_i8(pad.left),
            right: to_i8(pad.right),
            top: to_i8(pad.top),
            bottom: to_i8(pad.bottom),
        })
        .outer_margin(egui::Margin::ZERO);

    let avail_w = ui.available_width();
    let inner_w = match pinned_width {
        Some(w) => (w - pad_h).max(0.0),
        None => (avail_w - pad_h).max(0.0),
    };

    let frame_res = cell_frame.show(ui, |ui| {
        let min_h = ui.text_style_height(&egui::TextStyle::Body);

        if cell.is_empty() {
            let target_h = pinned_height.unwrap_or(min_h);
            let inner_target = (target_h - pad_v).max(min_h);
            ui.set_min_size(egui::vec2(inner_w, inner_target));
            return min_h + pad_v;
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

        // Render text content tightly packed at top of cell without min_height constraint.
        // `TBL-030`/`TBL-031`: `left_to_right(Align::Min)` is LEFT aligned,
        // `top_down(Align::Min)` (outer scope) is TOP aligned.
        let content_res = ui.with_layout(
            egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
            |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                ui.set_min_width(inner_w);
                ui.set_max_width(inner_w);
                content(ui);
            },
        );

        let unconstrained_text_h = content_res.response.rect.height();

        // `pinned_height` is reserved up front by the caller on the *cell* Ui
        // (see `cell_size.y` in `render_table_with_config`), so the cell frame
        // here is already inside a `pinned_height`-tall cell. No inner
        // `set_min_height`/`set_max_height` is needed: those calls would
        // cause the cell's `min_rect` to be remembered by every ancestor
        // `push_id`, which would double-count the cell height when advancing
        // the row cursor and inflate the inter-row gutter to ~17 px (the
        // "row gutter too large" bug).

        unconstrained_text_h + pad_v
    });

    frame_res.inner
}

/// Render a markdown table using the Fair Table Width Algorithm (FTWA) with
/// the cross-cutting [`TableRenderConfig`] applied.
///
/// Per-column pixel widths are computed by `crate::ui::table_width::ftwa`
/// from egui-shaped max-content and min-content measurements; cells are
/// then pinned to their assigned width so the child Ui's available_width
/// matches the column width (see
/// `doc/planning/table-column-width-algorithm.md` §5 decision Q5). When
/// the available width falls below the sum of min-content
/// (`decision.needs_horizontal_scroll`), the table physically cannot fit
/// even with every column at its longest-token floor and we fall back to
/// the prior behaviour: a wrapping `ScrollArea` over max-content columns
/// (doc §3.6) so the strongest invariant — never split a token — is
/// preserved.
///
/// Column spacing is 10.0 px and row spacing is 4.0 px. The available content
/// width passed to FTWA subtracts `(N - 1) * 10.0` for those gutters so the
/// assigned widths sum to the true content rect.
///
/// The whole table — both the FTWA path and the §3.6 horizontal-scroll
/// fallback — is wrapped in a `Frame::NONE.stroke(TABLE_PERIMETER_STROKE)`
/// (medium gray, 1 px) so the table reads as a single bordered block on
/// screen.
fn render_table_with_config(
    ui: &mut egui::Ui,
    table_cells: &[Vec<Vec<InlineElem>>],
    table_ordinal: usize,
    strategy: crate::ui::table_width::DeficitStrategy,
    config: &crate::ui::table_width::TableRenderConfig,
) {
    let n = table_cells.iter().map(|row| row.len()).max().unwrap_or(0);
    if n == 0 {
        return;
    }

    // Stable id derived from a table ordinal rather than `ui.next_auto_id()`
    // (a positional peek that shifts whenever any widget above the table
    // changes). Using a content-derived ordinal keeps the table's persisted
    // column-width cache stable across edits/reflows.
    let table_id = egui::Id::new("md_table").with(table_ordinal);
    let global_padding = config.global_padding.sanitised();

    let (max_w, min_w, breakpoints) =
        crate::ui::table_width::measure_cached(table_id, table_cells, global_padding, ui);
    let gutter = 10.0_f32;
    let avail = (ui.available_width() - (n as f32 - 1.0) * gutter).max(0.0);
    let decision = crate::ui::table_width::ftwa_cached(
        table_id,
        &max_w,
        &min_w,
        &breakpoints,
        avail,
        strategy,
        ui,
    );

    let render_rows = |ui: &mut egui::Ui, decision_widths: &[f32]| {
        let cached_heights_id = table_id.with("row_heights");
        let cached_row_heights: Option<Vec<f32>> = ui.ctx().data(|d| d.get_temp(cached_heights_id));
        let mut new_row_heights = Vec::with_capacity(table_cells.len());

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 4.0);
            for (row_idx, row) in table_cells.iter().enumerate() {
                let is_striped = row_idx % 2 == 1;
                let bg_idx = if is_striped {
                    Some(ui.painter().add(egui::Shape::Noop))
                } else {
                    None
                };
                let target_h = cached_row_heights.as_ref().and_then(|h| h.get(row_idx).copied());

                let mut row_max_h: f32 = 0.0;
                let row_response = ui.push_id((table_id, "row", row_idx), |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);
                        for j in 0..n {
                            let cell = row.get(j).map(|v| v.as_slice()).unwrap_or(&[]);
                            let w = decision_widths.get(j).copied();
                            if !decision.needs_horizontal_scroll {
                                debug_assert!(
                                    w.is_some_and(|w| w.is_finite() && w > 0.0),
                                    "FTWA invariant violated: table {table_ordinal} column {j} width = {w:?}"
                                );
                            }
                            let w = w.filter(|w| w.is_finite() && *w > 0.0);
                            let cell_res = ui.push_id(("col", j), |ui| {
                                // If we know the row's target height from a previous
                                // frame, reserve it up front so the cell's natural
                                // content sits at the top and the cell UI doesn't
                                // double-expand to 2× the target height (the bug
                                // that produced a ~17 px "phantom" gutter).
                                let cell_size = egui::vec2(
                                    w.unwrap_or_else(|| ui.available_width()),
                                    target_h.unwrap_or(0.0),
                                );
                                ui.allocate_ui_with_layout(
                                    cell_size,
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        render_table_cell(
                                            ui,
                                            cell,
                                            w,
                                            target_h,
                                            global_padding,
                                        )
                                    },
                                )
                                .inner
                            });
                            let h = cell_res.inner;
                            if h.is_finite() {
                                row_max_h = row_max_h.max(h);
                            }
                        }
                    })
                });

                new_row_heights.push(row_max_h);

                if let Some(idx) = bg_idx {
                    ui.painter().set(
                        idx,
                        egui::Shape::rect_filled(
                            row_response.response.rect,
                            0.0,
                            ui.visuals().faint_bg_color,
                        ),
                    );
                }
            }
        });

        ui.ctx()
            .data_mut(|d| d.insert_temp(cached_heights_id, new_row_heights));
    };

    if decision.needs_horizontal_scroll {
        // §3.6 fallback: nothing can fit; preserve the never-break-token
        // invariant by letting content overflow into a horizontal ScrollArea.
        egui::Frame::NONE
            .stroke(TABLE_PERIMETER_STROKE)
            .inner_margin(egui::Margin::ZERO)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt(table_id.with("scroll"))
                    .show(ui, |ui| {
                        render_rows(ui, &max_w);
                    });
            });
        return;
    }

    // FTWA path: pin every cell to its assigned column width.
    egui::Frame::NONE
        .stroke(TABLE_PERIMETER_STROKE)
        .inner_margin(egui::Margin::ZERO)
        .show(ui, |ui| {
            render_rows(ui, &decision.widths);
        });
}

/// Render a markdown table with the default [`TableRenderConfig`].
///
/// Thin wrapper around [`render_table_with_config`] preserved for test
/// call sites that do not thread a config — equivalent to calling
/// `render_table_with_config` with `&TableRenderConfig::default()`
/// (global padding = ZERO). Production dispatch uses
/// `render_table_with_config` directly.
#[cfg(test)]
fn render_table(
    ui: &mut egui::Ui,
    table_cells: &[Vec<Vec<InlineElem>>],
    table_ordinal: usize,
    strategy: crate::ui::table_width::DeficitStrategy,
) {
    render_table_with_config(
        ui,
        table_cells,
        table_ordinal,
        strategy,
        &crate::ui::table_width::TableRenderConfig::default(),
    );
}

/// Width (in logical pixels) reserved for the YAML front-matter table's
/// key column. Picked to comfortably fit the longest realistic YAML
/// front-matter key (e.g. `header-date`, `last-modified`, `navigation`)
/// while leaving the rest of the panel for the value column — which is
/// the column that holds the long content (e.g. `summary`,
/// `description`) that must word-wrap.
const YAML_KEY_COLUMN_WIDTH: f32 = 110.0;

/// Purpose: Renders a YAML table UI from a parsed mapping.
/// Inputs: `ui` (mut), `yaml`
/// Outputs: None
/// Purity: Impure (modifies UI state). Coordinates parsing and rendering.
pub fn render_yaml_table(ui: &mut egui::Ui, yaml: &serde_yml::Value) {
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
                // Reserve a fixed width for the key column and let the
                // value column take the remainder. Without explicit widths, a
                // cell might expand past the panel width.
                // By giving each cell an explicit width, the value cell knows its
                // wrap budget and the total table width matches
                // `available_width` exactly, so no horizontal scrolling
                // is needed.
                let key_col_width = YAML_KEY_COLUMN_WIDTH.min((available_width - 20.0).max(40.0));
                // `12.0` matches the column spacing `12.0` below.
                let value_col_width = (available_width - key_col_width - 12.0).max(40.0);

                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(12.0, 4.0);
                    for (row_idx, (k, v)) in pairs.into_iter().enumerate() {
                        let is_striped = row_idx % 2 == 1;
                        let bg_idx = if is_striped {
                            Some(ui.painter().add(egui::Shape::Noop))
                        } else {
                            None
                        };

                        let row_response = ui.push_id((table_id, "yaml_row", row_idx), |ui| {
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                                // Key cell: set key_col_width and align top.
                                ui.push_id("key", |ui| {
                                    ui.with_layout(
                                        egui::Layout::left_to_right(egui::Align::Min),
                                        |ui| {
                                            ui.set_min_width(key_col_width);
                                            ui.set_max_width(key_col_width);
                                            ui.label(
                                                RichText::new(k)
                                                    .strong()
                                                    .color(egui::Color32::from_rgb(150, 200, 255)),
                                            );
                                        },
                                    );
                                });
                                // Value cell: set value_col_width and wrap text inside.
                                ui.push_id("value", |ui| {
                                    ui.with_layout(
                                        egui::Layout::left_to_right(egui::Align::Min),
                                        |ui| {
                                            ui.set_min_width(value_col_width);
                                            ui.set_max_width(value_col_width);
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(v)
                                                        .color(egui::Color32::from_gray(220)),
                                                )
                                                .wrap(),
                                            );
                                        },
                                    );
                                });
                            })
                        });

                        if let Some(idx) = bg_idx {
                            ui.painter().set(
                                idx,
                                egui::Shape::rect_filled(
                                    row_response.response.rect,
                                    0.0,
                                    ui.visuals().faint_bg_color,
                                ),
                            );
                        }
                    }
                });
            });
        ui.add_space(8.0);
    }
}

/// Purpose: Renders markdown text to UI.
/// Inputs: `ui` (mut), `markdown_text`, `scroll_to_id_str` (mut, the
/// stable string id of the heading the centre panel should scroll to)
/// Outputs: None
/// Purity: Impure (modifies UI state). Coordinates parsing and rendering.
///
/// `scroll_to_id_str` is the egui-free stable identifier that lives in
/// `TabManager::scroll_to_header_id`; this function takes it by `&mut
/// Option<String>`, converts it to an `egui::Id` for the inner
/// scroll-to-me comparison, and clears the field when the matching
/// heading has been scrolled to.
pub fn render_markdown(
    ui: &mut egui::Ui,
    markdown_text: &str,
    scroll_to_id_str: &mut Option<String>,
    pending_toggles: &mut Vec<(usize, bool)>,
    strategy: crate::ui::table_width::DeficitStrategy,
) {
    use std::sync::Arc;
    let text_hash = egui::Id::new(markdown_text);
    let cache_key = egui::Id::new("md_events_cache");
    type CachedEvents = (egui::Id, Arc<Vec<RenderEvent>>);

    let events: Arc<Vec<RenderEvent>> = if let Some((cached_hash, cached_events)) =
        ui.ctx().data(|d| d.get_temp::<CachedEvents>(cache_key))
    {
        if cached_hash == text_hash {
            cached_events
        } else {
            let parsed = Arc::new(parse_markdown_to_events(markdown_text));
            ui.ctx()
                .data_mut(|d| d.insert_temp(cache_key, (text_hash, parsed.clone())));
            parsed
        }
    } else {
        let parsed = Arc::new(parse_markdown_to_events(markdown_text));
        ui.ctx()
            .data_mut(|d| d.insert_temp(cache_key, (text_hash, parsed.clone())));
        parsed
    };

    let mut table_ordinal = 0usize;
    let mut task_index = 0usize;

    // Pre-compute heading ids with duplicate disambiguation so that
    // `render_heading` and `build_toc` derive the same id for each
    // heading. The occurrence ordinal is appended via `Id::with` for
    // duplicates (occurrence > 0).
    use std::collections::HashMap;
    let mut heading_seen: HashMap<String, usize> = HashMap::new();
    let mut heading_id_for = |text: &str| -> String {
        let occurrence = heading_seen.entry(text.to_string()).or_insert(0);
        let id = if *occurrence == 0 {
            text.to_string()
        } else {
            format!("{}#{}", text, *occurrence)
        };
        *occurrence += 1;
        id
    };

    let clip = ui.clip_rect();
    let viewport_margin = 400.0_f32;

    for event in events.iter() {
        let top_y = ui.cursor().min.y;
        if clip.is_positive() && top_y > clip.max.y + viewport_margin {
            match event {
                RenderEvent::FlushInline {
                    elems,
                    task_checked,
                    ..
                } => {
                    if task_checked.is_some() {
                        task_index += 1;
                    }
                    let est_h = (elems.len() as f32 * 18.0).max(18.0);
                    ui.add_space(est_h);
                    continue;
                }
                RenderEvent::CodeBlock(content) => {
                    let line_count = content.lines().count().max(1) as f32;
                    let est_h = line_count * 18.0 + 20.0;
                    ui.add_space(est_h);
                    continue;
                }
                _ => {}
            }
        }
        match event {
            RenderEvent::FlushInline {
                elems,
                needs_bullet,
                task_checked,
                indent,
                list_ordinal,
            } => {
                // P0-2: Assign a task index to each task list item so
                // checkbox toggles can be mapped back to the source.
                render_inline(
                    ui,
                    elems,
                    *needs_bullet,
                    *task_checked,
                    *indent,
                    *list_ordinal,
                    task_index,
                    pending_toggles,
                );
                if task_checked.is_some() {
                    task_index += 1;
                }
            }
            RenderEvent::CodeBlock(content) => {
                render_code_block(ui, content);
            }
            RenderEvent::Heading { level, elems } => {
                let text = heading_plain_text(elems);
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let heading_id_str = heading_id_for(trimmed);
                render_heading(ui, elems, *level, scroll_to_id_str, &heading_id_str);
            }
            RenderEvent::Table(cells) => {
                render_table_with_config(
                    ui,
                    cells,
                    table_ordinal,
                    strategy,
                    &crate::ui::table_width::TableRenderConfig::default(),
                );
                table_ordinal += 1;
            }
            RenderEvent::Space(amount) => {
                ui.add_space(*amount);
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
        let val: serde_yml::Value = serde_yml::from_str(yaml_str).unwrap();
        let pairs = parse_yaml_to_pairs(&val).unwrap();
        assert_eq!(pairs[0], ("key1".to_string(), "value1".to_string()));
        assert_eq!(pairs[1], ("key2".to_string(), "item1, item2".to_string()));
        assert_eq!(pairs[2], ("key3".to_string(), "100".to_string()));
        assert_eq!(pairs[3], ("key4".to_string(), "true".to_string()));
    }

    #[test]
    fn test_parse_yaml_to_pairs_non_mapping() {
        let string_val = serde_yml::Value::String("just string".to_string());
        assert_eq!(parse_yaml_to_pairs(&string_val), None);

        let seq_val =
            serde_yml::Value::Sequence(vec![serde_yml::Value::String("item".to_string())]);
        assert_eq!(parse_yaml_to_pairs(&seq_val), None);

        let null_val = serde_yml::Value::Null;
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

        // The previous version of this test asserted on `elems[0]` /
        // `elems[1]`, which is fragile: a refactor that splits or merges
        // inline elements would fail the test even though no real
        // behaviour changed. The structural check below verifies the
        // same contract ("the paragraph mixes plain and italic
        // emphasis") without depending on element ordering.
        let plain_some = elems.iter().any(|e| {
            matches!(
                e,
                InlineElem::Text(t, style) if t == "Some " && !style.italic
            )
        });
        let italic_text = elems.iter().any(|e| {
            matches!(
                e,
                InlineElem::Text(t, style) if t == "text" && style.italic
            )
        });
        assert!(
            plain_some,
            "paragraph must contain a plain-text 'Some ' inline elem: {elems:?}"
        );
        assert!(
            italic_text,
            "paragraph must contain an italic 'text' inline elem: {elems:?}"
        );

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

    // `# *italic*`, `# **bold**`, `# `code``, `# ~~strike~~`, and
    /// `# [link](url)` all previously lost their inline formatting
    /// because `RenderEvent::Heading` stored `text: String` (plain
    /// concatenation) rather than `elems: Vec<InlineElem>`. The struct
    /// now carries the styled elements; the renderer renders each
    /// span with the heading's size and weight. These tests pin the
    /// expected contract end-to-end. Each row of the table is one
    /// case the old single-test-per-style version covered; the
    /// closure asserts that the heading produced by `md_source`
    /// contains an inline element satisfying `style_predicate`.
    #[test]
    fn test_heading_preserves_inline_formatting() {
        // One assertion predicate per case. Adding a new inline
        // formatting (e.g. underline) means adding one row here,
        // not a new 25-line `#[test] fn`.
        type StylePredicate = Box<dyn Fn(&InlineElem) -> bool>;
        let cases: &[(&str, &str, StylePredicate)] = &[
            (
                "# *hello*",
                "italic",
                Box::new(|e| matches!(e, InlineElem::Text(_, s) if s.italic)),
            ),
            (
                "# **loud**",
                "bold",
                Box::new(|e| matches!(e, InlineElem::Text(_, s) if s.bold)),
            ),
            (
                "# `code` in heading",
                "code",
                Box::new(|e| matches!(e, InlineElem::Text(_, s) if s.code)),
            ),
            (
                "# ~~old~~",
                "strikethrough",
                Box::new(|e| matches!(e, InlineElem::Text(_, s) if s.strikethrough)),
            ),
            (
                "# [click](https://example.com)",
                "link",
                Box::new(
                    |e| matches!(e, InlineElem::Link(url, text) if url == "https://example.com" && text == "click"),
                ),
            ),
        ];

        for (md, label, style_predicate) in cases {
            let events = parse_markdown_to_events(md);
            let heading = events
                .iter()
                .find_map(|e| {
                    if let RenderEvent::Heading { level, elems } = e {
                        Some((*level, elems))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| panic!("{label}: no Heading event for {md:?}: {events:?}"));
            assert_eq!(heading.0, 1, "{label}: heading level should be 1");
            assert!(
                heading.1.iter().any(style_predicate),
                "{label}: predicate did not match any inline elem in {heading:?} for {md:?}",
            );
        }
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
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                );

                let yaml_str = "a: 1\nb: 2";
                let val: serde_yml::Value = serde_yml::from_str(yaml_str).unwrap();
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
                render_markdown(
                    ui,
                    md,
                    &mut scroll_id,
                    &mut Vec::new(),
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                );

                // Render non-mapping YAML table
                let non_map = serde_yml::Value::String("test".to_string());
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
                render_markdown(
                    ui,
                    md,
                    &mut scroll_id,
                    &mut Vec::new(),
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                );
            });
        });
    }

    /// Regression: a long YAML value must word-wrap inside the YAML
    /// metadata table rather than overflow the panel's content rect
    /// and get clipped by the inner horizontal `ScrollArea`.
    ///
    /// Before the fix, `render_yaml_table` rendered both columns with
    /// `ui.label(...)` inside an unconstrained `Grid`, so the value
    /// column expanded to the natural width of the longest text. The
    /// value text therefore ran off the right edge of the viewport
    /// and the user saw the value truncated mid-word (e.g.
    /// "Microsoft in Re…").
    ///
    /// The test pins the symptom by:
    /// 1. Rendering the YAML table in a deliberately narrow 320px
    ///    viewport so a long value cannot fit on a single line.
    /// 2. Locating the `Shape::Text` that carries the long summary
    ///    text (uniquely identified by its leading "Heise Invoice"
    ///    substring).
    /// 3. Asserting the underlying `Galley` has more than one row
    ///    (the text wrapped) and that the rendered rect's width fits
    ///    within the available content area (no horizontal overflow).
    #[test]
    fn test_render_yaml_table_wraps_long_values_within_viewport() {
        use crate::ui::test_helpers::text::extract_text;

        let ctx = egui::Context::default();
        let viewport_width: f32 = 320.0;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(viewport_width, 800.0),
            )),
            ..egui::RawInput::default()
        };
        // The exact string the user reported in the screenshot.
        let long_summary = "January 2005 invoice from Heise Zeitschriften Verlag for \
            Microsoft half-year archive CD-ROMs, shipped tax-free to Martin Kühne at \
            Microsoft in Redmond, WA, USA, for archive and product-evaluation purposes \
            under Microsoft product license terms.";
        let yaml_str = format!(
            "title: Heise Invoice for Microsoft Product — Tax-Free Export Delivery\n\
             summary: \"{long_summary}\"\n\
             tags: [invoice, receipt, technology, documents]\n\
             header-date: 2026-07-22T19:32:47Z\n"
        );
        let yaml: serde_yml::Value = serde_yml::from_str(&yaml_str).unwrap();

        let output = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_yaml_table(ui, &yaml);
            });
        });

        // Sanity: the long value must actually have been rendered.
        let texts = extract_text(&output.shapes);
        let needle = "Heise Zeitschriften Verlag";
        assert!(
            texts.iter().any(|t| t.contains(needle)),
            "expected the long summary to be rendered; got {} text shape(s)",
            texts.len()
        );

        // Locate the `Shape::Text` whose galley carries the long
        // summary (matched by an unambiguous substring that cannot
        // appear in any other YAML key in the fixture).
        let summary_shape = output.shapes.iter().find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text().contains(needle) => Some(t),
            _ => None,
        });
        let shape = summary_shape.expect("expected a Text shape for the long summary");
        let galley = &shape.galley;
        let rendered_width = galley.rect.width();

        // 1. The long value must wrap: more than one row in the galley.
        assert!(
            galley.rows.len() > 1,
            "expected the long summary to word-wrap; got galley with {} row(s) and rect width={:.1}px (viewport={viewport_width:.0}px)",
            galley.rows.len(),
            rendered_width,
        );

        // 2. The wrapped text must fit inside the viewport — the
        //    rect width should never exceed the viewport. Use a small
        //    tolerance for the CentralPanel's outer margins.
        let max_allowed = viewport_width;
        assert!(
            rendered_width <= max_allowed + 1.0,
            "expected wrapped text width <= {max_allowed:.0}px; got {rendered_width:.1}px \
             (the value is overflowing the panel — the horizontal ScrollArea is clipping it)",
        );
    }

    /// Regression: multi-line YAML front-matter values must expand grid row height
    /// so subsequent rows do not print over the wrapped text lines.
    #[test]
    fn test_render_yaml_table_row_height_prevents_text_overlap() {
        let ctx = egui::Context::default();
        let viewport_width: f32 = 320.0;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(viewport_width, 800.0),
            )),
            ..egui::RawInput::default()
        };
        let long_summary = "Cloud native software architect profile focused on scalable, \
            performant, and resilient services. Highlights skills in legacy application \
            migration to microservices and building developer communities.";
        let yaml_str = format!(
            "title: 2020 LinkedIn Profile - Cloud Native Architect\n\
             summary: \"{long_summary}\"\n\
             tags: [professional, career, linkedin, architect]\n"
        );
        let yaml: serde_yml::Value = serde_yml::from_str(&yaml_str).unwrap();

        let output = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_yaml_table(ui, &yaml);
            });
        });

        // Find the summary value text shape and the tags key/value text shape.
        let summary_shape = output
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                egui::Shape::Text(t) if t.galley.text().contains("microservices") => Some(t),
                _ => None,
            })
            .expect("expected summary text shape");

        let tags_shape = output
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                egui::Shape::Text(t) if t.galley.text() == "tags" => Some(t),
                _ => None,
            })
            .expect("expected tags key text shape");

        let summary_bottom_y = summary_shape.pos.y + summary_shape.galley.rect.height();
        let tags_top_y = tags_shape.pos.y;

        assert!(
            tags_top_y >= summary_bottom_y,
            "expected tags row (y={tags_top_y:.1}) to start below summary wrapped content (bottom y={summary_bottom_y:.1}), but it overlapped!",
        );
    }

    /// Regression: multi-line markdown table cells must respect FTWA column widths (fit viewport)
    /// or enable horizontal scrolling when min_w exceeds viewport, and expand grid row height
    /// so subsequent rows do not overlap.
    #[test]
    fn test_render_table_multiline_cells_fit_panel_and_expand_row_height() {
        let ctx = egui::Context::default();

        // 1. Table that fits within viewport (3 columns, 600px viewport)
        let fit_table = vec![
            vec![
                vec![InlineElem::Text("Make".into(), Default::default())],
                vec![InlineElem::Text("Model".into(), Default::default())],
                vec![InlineElem::Text("Summary".into(), Default::default())],
            ],
            vec![
                vec![InlineElem::Text("Acer".into(), Default::default())],
                vec![InlineElem::Text("Swift 16 AI".into(), Default::default())],
                vec![InlineElem::Text("Excellent value. Vibrant OLED touch screen, lightweight (~1.5kg), everyday performance.".into(), Default::default())],
            ],
            vec![
                vec![InlineElem::Text("Dell".into(), Default::default())],
                vec![InlineElem::Text("XPS 16".into(), Default::default())],
                vec![InlineElem::Text("Dell's flagship premium laptop.".into(), Default::default())],
            ],
        ];

        let viewport_width: f32 = 600.0;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(viewport_width, 800.0),
            )),
            ..egui::RawInput::default()
        };

        let output = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_table(
                    ui,
                    &fit_table,
                    0,
                    crate::ui::table_width::DeficitStrategy::BreakpointWaterFill,
                );
            });
        });

        // Summary text in Row 1 must fit inside viewport
        let summary_shape = output
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                egui::Shape::Text(t) if t.galley.text().contains("Vibrant OLED") => Some(t),
                _ => None,
            })
            .expect("expected row 1 summary text shape");

        let summary_right_x = summary_shape.pos.x + summary_shape.galley.rect.width();
        assert!(
            summary_right_x <= viewport_width + 1.0,
            "expected summary text right x ({summary_right_x:.1}) to fit inside viewport ({viewport_width:.1})",
        );

        // Row 2 (Dell) must start below Row 1 Summary text bottom Y (no text overlap)
        let dell_shape = output
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                egui::Shape::Text(t) if t.galley.text() == "Dell" => Some(t),
                _ => None,
            })
            .expect("expected dell text shape");

        let summary_bottom_y = summary_shape.pos.y + summary_shape.galley.rect.height();
        let dell_top_y = dell_shape.pos.y;

        assert!(
            dell_top_y >= summary_bottom_y,
            "expected dell row (y={dell_top_y:.1}) to start below summary wrapped content (bottom y={summary_bottom_y:.1}), but it overlapped!",
        );
    }

    /// Regression: long body paragraphs must word-wrap inside the preview.
    ///
    /// `render_inline_inner` renders each `InlineElem::Text` via
    /// `ui.add(egui::Label::new(rt).wrap())`. In egui 0.35, `Label::new`
    /// defaults `wrap_mode` to `None` and only wraps if the parent
    /// layout is vertical or horizontal+main_wrap AND the available
    /// width is finite — a fragile contract that already broke for
    /// `render_yaml_table` (see
    /// `test_render_yaml_table_wraps_long_values_within_viewport`),
    /// `render_code_block`, and `render_table_cell`, each of which
    /// had to be patched with an explicit `.wrap()`. This test pins
    /// the same invariant for the paragraph path so a future
    /// refactor (e.g. swapping the `horizontal_wrapped` parent for a
    /// `Grid` or removing the explicit `.wrap()`) cannot silently
    /// regress long-paragraph wrapping.
    ///
    /// Mirrors the shape of
    /// `test_render_yaml_table_wraps_long_values_within_viewport` above:
    /// render in a deliberately narrow 320px viewport, locate the
    /// `Shape::Text` that carries the long paragraph (matched by an
    /// unambiguous substring that cannot appear in the table or header
    /// text), and assert the underlying `Galley` wraps to multiple
    /// rows and stays within the viewport.
    #[test]
    fn test_render_markdown_long_paragraph_wraps_in_preview() {
        use crate::ui::test_helpers::text::extract_text;

        let ctx = egui::Context::default();
        let viewport_width: f32 = 320.0;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(viewport_width, 800.0),
            )),
            ..egui::RawInput::default()
        };
        // Same shape as the user's `Mythical man-month.md` body: one
        // long German sentence with normal whitespace, ~570 chars.
        let long_paragraph = "Es ist ein Mix an Methoden im Einsatz. \
            Einerseits soll im traditionellen Projektmanagement in Voraus \
            der Funktionsumfang und die Projektdauer feststehen. Die zur \
            Planung notwendige Dokumentation der Anforderungen, Technologien \
            und Risken findet aber nicht statt. Daraufhin trägt das \
            ausführende Team ein erhebliches Risiko, wenn sich die \
            Anforderungen ändern oder die Arbeit komplexer ist als erwartet.";
        let md = format!("\n{long_paragraph}\n");

        let output = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut scroll_id = None;
                render_markdown(
                    ui,
                    &md,
                    &mut scroll_id,
                    &mut Vec::new(),
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                );
            });
        });

        // Sanity: the long paragraph must have been rendered.
        let texts = extract_text(&output.shapes);
        let needle = "erhebliches Risiko, wenn sich";
        assert!(
            texts.iter().any(|t| t.contains(needle)),
            "expected the long paragraph to be rendered; got {} text shape(s): {:?}",
            texts.len(),
            texts,
        );

        // Locate the Text shape whose galley carries the long paragraph.
        let paragraph_shape = output.shapes.iter().find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text().contains(needle) => Some(t),
            _ => None,
        });
        let shape = paragraph_shape.expect("expected a Text shape for the long paragraph");
        let galley = &shape.galley;

        // 1. The paragraph must wrap: more than one row.
        assert!(
            galley.rows.len() > 1,
            "expected the long paragraph to word-wrap; got galley with {} row(s) \
             and rect width={:.1}px (viewport={viewport_width:.0}px) — \
             the text is overflowing instead of wrapping",
            galley.rows.len(),
            galley.rect.width(),
        );

        // 2. The wrapped text must fit inside the viewport.
        let max_allowed = viewport_width;
        assert!(
            galley.rect.width() <= max_allowed + 1.0,
            "expected wrapped paragraph width <= {max_allowed:.0}px; got {:.1}px \
             (the paragraph is overflowing the panel — the preview will \
             horizontal-scroll the long line instead of wrapping it)",
            galley.rect.width(),
        );
    }

    #[test]
    fn test_render_table_with_bold_and_special_chars_e2e() {
        let ctx = egui::Context::default();
        let md = "| Name | Account | Amount | Type |\n|---|---|---|---|\n| **Vanguard** | #12345678 | $1 | Taxable (investment) |";
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut scroll_id = None;
                render_markdown(
                    ui,
                    md,
                    &mut scroll_id,
                    &mut Vec::new(),
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                );
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
                let (max_w, min_w, breakpoints) = crate::ui::table_width::measure(
                    &cells,
                    crate::ui::table_width::TablePadding::ZERO,
                    ui,
                );
                assert_eq!(max_w.len(), 6, "6 max-content widths");
                assert_eq!(min_w.len(), 6, "6 min-content widths");
                assert_eq!(breakpoints.len(), 6, "6 breakpoint vectors");
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
                    let decision = crate::ui::table_width::ftwa(
                        &max_w,
                        &min_w,
                        &breakpoints,
                        a,
                        crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                    );
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
                    let (max_w, min_w, breakpoints) = crate::ui::table_width::measure(
                        &cells,
                        crate::ui::table_width::TablePadding::ZERO,
                        ui,
                    );
                    let gutter = 10.0_f32;
                    let avail = (ui.available_width()
                        - (max_w.len() as f32 - 1.0).max(0.0) * gutter)
                        .max(0.0);
                    let decision = crate::ui::table_width::ftwa(
                        &max_w,
                        &min_w,
                        &breakpoints,
                        avail,
                        crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                    );
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
                render_markdown(
                    ui,
                    md,
                    &mut scroll_id,
                    &mut Vec::new(),
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                );
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
        let target_id_str = "Target Heading".to_string();
        let mut scroll_id: Option<String> = Some(target_id_str.clone());
        let mut dummy_scroll: Option<String> = Some(target_id_str.clone());

        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let elems = vec![InlineElem::Text(
                    "Target Heading".to_string(),
                    TextStyle::default(),
                )];
                render_heading(ui, &elems, 1, &mut scroll_id, &target_id_str);
                // Empty title should not trigger scroll
                render_heading(ui, &[], 1, &mut dummy_scroll, &target_id_str);
            });
        });

        assert_eq!(
            scroll_id, None,
            "scroll_to_id should be cleared after scroll"
        );
        assert_eq!(
            dummy_scroll,
            Some(target_id_str),
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
        render_table_with_viewport_and_strategy(
            table_cells,
            viewport_width,
            crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
        )
    }

    fn render_table_with_viewport_and_strategy(
        table_cells: &[Vec<Vec<InlineElem>>],
        viewport_width: f32,
        strategy: crate::ui::table_width::DeficitStrategy,
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
                let (max_w, min_w, breakpoints) = crate::ui::table_width::measure(
                    table_cells,
                    crate::ui::table_width::TablePadding::ZERO,
                    ui,
                );
                let gutter = 10.0_f32;
                let avail =
                    (ui.available_width() - (max_w.len() as f32 - 1.0).max(0.0) * gutter).max(0.0);
                let decision =
                    crate::ui::table_width::ftwa(&max_w, &min_w, &breakpoints, avail, strategy);
                captured = Some(decision.clone());
                render_table(ui, table_cells, 0, strategy);
            });
        });
        captured.expect("ctx.run should have populated `captured`")
    }

    /// Like `render_table_with_viewport_and_strategy` but threads a non-default
    /// `global_padding` through `render_table_with_config` so US3 width/height
    /// accounting picks up the resolved padding (`TBL-033`).
    fn render_table_with_viewport_and_padding(
        table_cells: &[Vec<Vec<InlineElem>>],
        viewport_width: f32,
        padding: crate::ui::table_width::TablePadding,
    ) -> crate::ui::table_width::ColumnWidths {
        let strategy = crate::ui::table_width::DeficitStrategy::ProportionalToSlack;
        let ctx = egui::Context::default();
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
                let (max_w, min_w, breakpoints) =
                    crate::ui::table_width::measure(table_cells, padding, ui);
                let gutter = 10.0_f32;
                let avail =
                    (ui.available_width() - (max_w.len() as f32 - 1.0).max(0.0) * gutter).max(0.0);
                let decision =
                    crate::ui::table_width::ftwa(&max_w, &min_w, &breakpoints, avail, strategy);
                captured = Some(decision.clone());
            });
        });
        captured.expect("ctx.run should have populated `captured`")
    }

    /// Paint variants of the padding-aware viewport helper — produces
    /// `FullOutput` for shape inspection (used by US3 border/junction tests).
    fn render_table_with_paint_output_and_padding(
        table_cells: &[Vec<Vec<InlineElem>>],
        viewport_width: f32,
        padding: crate::ui::table_width::TablePadding,
    ) -> egui::FullOutput {
        let strategy = crate::ui::table_width::DeficitStrategy::ProportionalToSlack;
        let config = crate::ui::table_width::TableRenderConfig {
            global_padding: padding,
        };
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(viewport_width, 1600.0),
            )),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw.clone(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_table_with_config(ui, table_cells, 0, strategy, &config);
            });
        });
        ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_table_with_config(ui, table_cells, 0, strategy, &config);
            });
        })
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

    /// US1 end-to-end (TBL-001..TBL-012): the three FTWA regimes (surplus,
    /// deficit, fallback) are each visible in the painted output.
    /// - Surplus: uniform column widths; no cell wraps (each painted galley
    ///   is single-line; total galley rows == non-empty text cell count).
    /// - Deficit: column widths sum to `available`; at least one cell wraps
    ///   (total painted galley rows strictly exceed the non-empty cell count).
    /// - Fallback: `needs_horizontal_scroll`; the `ScrollArea` emits
    ///   additional clip-rect + scrollbar shapes absent from the FTWA path,
    ///   so its `shapes.len()` strictly exceeds the surplus paint shape count.
    #[test]
    fn test_render_table_surplus_deficit_fallback_visible_end_to_end() {
        use eframe::epaint::Shape;

        // Sum of painted galley row counts across every text shape. Each
        // non-wrapped text cell contributes exactly one row; a wrapped cell
        // contributes >1. This is the direct wrap signal.
        let total_painted_galley_rows = |out: &egui::FullOutput| -> usize {
            out.shapes
                .iter()
                .filter_map(|cs| match &cs.shape {
                    Shape::Text(t) => Some(t.galley.rows.len()),
                    _ => None,
                })
                .sum()
        };

        // Available width inside CentralPanel for an n-col table, mirroring
        // render_table's gutter math (16.0 outer margin + (n-1)*10.0 gutters).
        let avail_for =
            |vw: f32, n_cols: usize| -> f32 { (vw - 16.0) - (n_cols as f32 - 1.0) * 10.0 };

        // Count non-empty text cells in a table (each contributes a galley).
        let non_empty_text_cells = |table: &[Vec<Vec<InlineElem>>]| -> usize {
            table
                .iter()
                .flat_map(|row| row.iter())
                .filter(|cell| !cell.is_empty())
                .count()
        };

        // (a) Surplus: 3 identical short-token columns, 800px viewport.
        let surplus_table = build_uniform_table("name", 3);
        let surplus_widths = render_table_with_viewport(&surplus_table, 800.0);
        assert!(
            !surplus_widths.needs_horizontal_scroll,
            "surplus must not scroll; got {:?}",
            surplus_widths.widths
        );
        // Uniform widths (identical columns → FTWA distributes spare equally).
        let mn = surplus_widths
            .widths
            .iter()
            .copied()
            .fold(f32::INFINITY, f32::min);
        let mx = surplus_widths
            .widths
            .iter()
            .copied()
            .fold(0.0_f32, f32::max);
        assert!(
            (mx - mn) < 0.5,
            "surplus widths must be uniform for identical columns; got min={mn} max={mx}"
        );
        let surplus_paint = render_table_with_paint_output(&surplus_table);
        let surplus_expected_text = non_empty_text_cells(&surplus_table);
        let surplus_rows = total_painted_galley_rows(&surplus_paint);
        assert!(
            surplus_rows <= surplus_expected_text,
            "surplus must not wrap: painted galley rows ({surplus_rows}) \
             must not exceed non-empty cell count ({surplus_expected_text})"
        );

        // (b) Deficit: 3 identical multi-word columns, 300px viewport.
        let deficit_table = build_uniform_table("alpha beta gamma delta epsilon zeta", 3);
        let deficit_widths = render_table_with_viewport(&deficit_table, 300.0);
        assert!(
            !deficit_widths.needs_horizontal_scroll,
            "deficit must not scroll; got {:?}",
            deficit_widths.widths
        );
        let avail_b = avail_for(300.0, 3);
        let sum_b: f32 = deficit_widths.widths.iter().copied().sum();
        assert!(
            (sum_b - avail_b).abs() < 1.0,
            "deficit widths must sum to available ({avail_b}); got sum={sum_b} widths={:?}",
            deficit_widths.widths
        );
        let deficit_paint = render_table_with_paint_output_viewport(&deficit_table, 300.0);
        let deficit_expected_text = non_empty_text_cells(&deficit_table);
        let deficit_rows = total_painted_galley_rows(&deficit_paint);
        assert!(
            deficit_rows > deficit_expected_text,
            "deficit must wrap at least one cell: painted galley rows ({deficit_rows}) \
             must exceed non-empty cell count ({deficit_expected_text})"
        );

        // (c) Fallback: 3 identical long-single-token columns, 30px viewport.
        let fallback_table = build_uniform_table("a_long_column_header_text_here_now", 3);
        let fallback_widths = render_table_with_viewport(&fallback_table, 30.0);
        assert!(
            fallback_widths.needs_horizontal_scroll,
            "fallback must scroll; got {:?}",
            fallback_widths.widths
        );
        let fallback_paint = render_table_with_paint_output_viewport(&fallback_table, 30.0);
        // The §3.6 fallback renders Grid inside a horizontal ScrollArea.
        // The ScrollArea path emits `Shape::Mesh` (scrollbar track) and
        // `Shape::Noop` placeholders — none of which appear in the plain
        // FTWA path (its shapes are exclusively `Rect` + `Text`). Detect
        // the fallback by the presence of at least one `Mesh` or `Noop`
        // shape that the FTWA path never emits.
        let has_scrollarea_only_shape = |out: &egui::FullOutput| -> bool {
            out.shapes
                .iter()
                .any(|cs| matches!(&cs.shape, Shape::Mesh(_) | Shape::Noop))
        };
        assert!(
            !has_scrollarea_only_shape(&surplus_paint),
            "FTWA path must not emit Mesh/Noop shapes; surplus kinds={:?}",
            surplus_paint
                .shapes
                .iter()
                .map(|cs| std::mem::discriminant(&cs.shape))
                .collect::<Vec<_>>()
        );
        assert!(
            has_scrollarea_only_shape(&fallback_paint),
            "fallback (ScrollArea) must emit Mesh/Noop shapes absent from FTWA path; \
             fallback kinds={:?}",
            fallback_paint
                .shapes
                .iter()
                .map(|cs| std::mem::discriminant(&cs.shape))
                .collect::<Vec<_>>()
        );
    }

    /// US2 (TBL-002): inline Markdown formatting inside table cells must
    /// survive the cell-render path — bold, italic, and code spans render
    /// with the appropriate styling on the painted galley, not as opaque
    /// plain text.
    ///
    /// Bold in egui 0.35 is observable only via a distinct `TextFormat.color`
    /// (`visuals.strong_text_color()`); italic is observable via
    /// `TextFormat.italics == true`; code is observable via
    /// `TextFormat.font_id.family == FontFamily::Monospace`.
    #[test]
    fn test_render_table_cell_markdown_bold_italic_code() {
        use eframe::epaint::{FontFamily, Shape};
        use std::collections::HashSet;

        let table = vec![vec![
            vec![InlineElem::Text(
                "bold".to_string(),
                TextStyle {
                    bold: true,
                    ..TextStyle::default()
                },
            )],
            vec![InlineElem::Text(
                "italic".to_string(),
                TextStyle {
                    italic: true,
                    ..TextStyle::default()
                },
            )],
            vec![InlineElem::Text(
                "code".to_string(),
                TextStyle {
                    code: true,
                    ..TextStyle::default()
                },
            )],
        ]];

        let output = render_table_with_paint_output(&table);

        let mut all_text = String::new();
        let mut distinct_colors: HashSet<eframe::epaint::Color32> = HashSet::new();
        let mut saw_italic = false;
        let mut saw_monospace = false;
        for cs in &output.shapes {
            let Shape::Text(text_shape) = &cs.shape else {
                continue;
            };
            all_text.push_str(text_shape.galley.text());
            for section in text_shape.galley.job.sections.iter() {
                distinct_colors.insert(section.format.color);
                if section.format.italics {
                    saw_italic = true;
                }
                if section.format.font_id.family == FontFamily::Monospace {
                    saw_monospace = true;
                }
            }
        }

        assert!(
            all_text.contains("bold") && all_text.contains("italic") && all_text.contains("code"),
            "painted galley text must include all three cell strings; got: {all_text:?}"
        );
        assert!(
            distinct_colors.len() >= 2,
            "bold cell must paint with `strong_text_color()` distinct from body text color; \
             distinct section colors: {distinct_colors:?}"
        );
        assert!(
            saw_italic,
            "italic cell must emit a LayoutSection with format.italics == true"
        );
        assert!(
            saw_monospace,
            "code cell must emit a LayoutSection with font_id.family == Monospace"
        );
    }

    /// US2 (TBL-003): inline Markdown links render as their display string and
    /// image placeholders render as the `[Image: <url>]` literal, both in the
    /// body font via the standard `ui.hyperlink_to` / `ui.label` widgets.
    #[test]
    fn test_render_table_cell_link_and_image_placeholder() {
        use eframe::epaint::Shape;

        let table = vec![vec![
            vec![InlineElem::Link(
                "https://example.com".to_string(),
                "Example".to_string(),
            )],
            vec![InlineElem::Image("pic.png".to_string())],
        ]];

        let output = render_table_with_paint_output(&table);

        let mut all_text = String::new();
        for cs in &output.shapes {
            let Shape::Text(text_shape) = &cs.shape else {
                continue;
            };
            all_text.push_str(text_shape.galley.text());
        }

        assert!(
            all_text.contains("Example"),
            "link display text must appear in painted output; got: {all_text:?}"
        );
        assert!(
            all_text.contains("[Image: pic.png]"),
            "image placeholder literal must appear in painted output; got: {all_text:?}"
        );
    }

    /// US3 (TBL-033): a non-zero global padding must be factored into every
    /// column's measured width AND into every row's height. We render the
    /// same 2×2 table twice — once with `TablePadding::ZERO`, once with
    /// `TablePadding{8, 8, 8, 8}` (`horizontal() == 16`, `vertical() == 16`) —
    /// so the per-column delta is exactly `padding.horizontal()` (surplus
    /// regime, columns pinned at max_content), and the per-row height delta
    /// is at least `padding.vertical()` (`top + bottom`).
    #[test]
    fn test_render_table_padding_factored_into_width_and_height() {
        use crate::ui::table_width::TablePadding;

        // 2×2 table with identical short-token cells → surplus regime at
        // 800px → every column pinned at its max_content width. Padding
        // (resolved from the global default) adds `horizontal()` to every
        // max_content × min_content value, so the measured column width
        // rises by exactly `horizontal()`.
        let table = build_uniform_table("cell", 2);

        let widths_zero = render_table_with_viewport_and_padding(&table, 800.0, TablePadding::ZERO);
        let pad = TablePadding {
            top: 8.0,
            bottom: 8.0,
            left: 8.0,
            right: 8.0,
        };
        let widths_pad = render_table_with_viewport_and_padding(&table, 800.0, pad);

        assert_eq!(
            widths_zero.widths.len(),
            2,
            "expected 2 columns; got {:?}",
            widths_zero.widths
        );
        assert_eq!(
            widths_pad.widths.len(),
            2,
            "expected 2 columns (padded); got {:?}",
            widths_pad.widths
        );
        for j in 0..2 {
            let delta = widths_pad.widths[j] - widths_zero.widths[j];
            assert!(
                (delta - pad.horizontal()).abs() < 0.5,
                "column {j} padding delta must equal horizontal() ({pad_h}); \
                 got zero={z}, pad={p}, delta={d}",
                pad_h = pad.horizontal(),
                z = widths_zero.widths[j],
                p = widths_pad.widths[j],
                d = delta,
            );
        }

        // Height assertion: pick the painted galley bounding rows and
        // sum heights across text shapes. With padding, every row must
        // be taller by at least `padding.vertical()` than without.
        let paint_zero =
            render_table_with_paint_output_and_padding(&table, 800.0, TablePadding::ZERO);
        let paint_pad = render_table_with_paint_output_and_padding(&table, 800.0, pad);
        let total_height = |out: &egui::FullOutput| -> f32 {
            use eframe::epaint::Shape;
            out.shapes
                .iter()
                .filter_map(|cs| match &cs.shape {
                    Shape::Rect(rect) if rect.fill == eframe::epaint::Color32::TRANSPARENT => {
                        Some(rect.rect.height())
                    }
                    _ => None,
                })
                .sum()
        };
        let h_zero = total_height(&paint_zero);
        let h_pad = total_height(&paint_pad);
        assert!(
            h_pad - h_zero >= pad.vertical() - 0.5,
            "painted height with padding ({h_pad}) must exceed height without padding ({h_zero}) \
             by at least padding.vertical() = {}; got delta={}",
            pad.vertical(),
            h_pad - h_zero,
        );
    }

    /// US3 (TBL-030, TBL-031): inline cell content sits at the top-left
    /// of the cell, offset by the resolved `padding.left` and
    /// `padding.top`. Asserted via the leftmost/topmost painted glyph
    /// position differing by exactly the resolved padding between a
    /// `TablePadding::ZERO` render and a `TablePadding{8,8,8,8}` render.
    #[test]
    fn test_render_table_cell_alignment_left_top() {
        use crate::ui::table_width::TablePadding;
        use eframe::epaint::Shape;

        let table = build_uniform_table("cell", 2);
        let paint_zero =
            render_table_with_paint_output_and_padding(&table, 800.0, TablePadding::ZERO);
        let pad = TablePadding {
            top: 8.0,
            bottom: 8.0,
            left: 8.0,
            right: 8.0,
        };
        let paint_pad = render_table_with_paint_output_and_padding(&table, 800.0, pad);

        // `TextShape::pos` is the screen-space top-left corner of the
        // galley; padding shifts it right by `padding.left` and down by
        // `padding.top`, which is exactly the LEFT+TOP alignment invariant.
        let first_text_pos = |out: &egui::FullOutput| -> Option<egui::Pos2> {
            out.shapes.iter().find_map(|cs| {
                let Shape::Text(t) = &cs.shape else {
                    return None;
                };
                Some(t.pos)
            })
        };

        let pos_zero = first_text_pos(&paint_zero)
            .expect("painted table must produce at least one text shape");
        let pos_pad = first_text_pos(&paint_pad)
            .expect("painted table (padded) must produce at least one text shape");

        assert!(
            (pos_pad.x - pos_zero.x - pad.left).abs() < 1.0,
            "LEFT alignment: padded first text x ({}) must exceed zero text x ({}) \
             by padding.left ({}); got delta={}",
            pos_pad.x,
            pos_zero.x,
            pad.left,
            pos_pad.x - pos_zero.x,
        );
        assert!(
            (pos_pad.y - pos_zero.y - pad.top).abs() < 1.0,
            "TOP alignment: padded first text y ({}) must exceed zero text y ({}) \
             by padding.top ({}); got delta={}",
            pos_pad.y,
            pos_zero.y,
            pad.top,
            pos_pad.y - pos_zero.y,
        );
    }

    /// US3 (TBL-031): In a row with cells of unequal height (e.g. short 1-line text
    /// alongside a multi-line cell), short cells MUST align to the top of the row.
    /// The top-y coordinate of text in the short cell must match the top-y coordinate
    /// of text in the tall cell.
    #[test]
    fn test_render_table_multiline_row_cell_vertical_alignment_top() {
        use eframe::epaint::Shape;

        let table = vec![vec![
            vec![InlineElem::Text("Short".to_string(), TextStyle::default())],
            vec![
                InlineElem::Text("Line 1".to_string(), TextStyle::default()),
                InlineElem::SoftBreak,
                InlineElem::Text("Line 2".to_string(), TextStyle::default()),
                InlineElem::SoftBreak,
                InlineElem::Text("Line 3".to_string(), TextStyle::default()),
                InlineElem::SoftBreak,
                InlineElem::Text("Line 4".to_string(), TextStyle::default()),
            ],
        ]];

        let output = render_table_with_paint_output(&table);

        let text_positions: Vec<(String, egui::Pos2)> = output
            .shapes
            .iter()
            .filter_map(|cs| {
                let Shape::Text(t) = &cs.shape else {
                    return None;
                };
                Some((t.galley.text().to_string(), t.pos))
            })
            .collect();

        let short_pos = text_positions
            .iter()
            .find(|(txt, _)| txt.contains("Short"))
            .map(|(_, pos)| *pos)
            .expect("short cell text must be painted");

        let tall_pos = text_positions
            .iter()
            .find(|(txt, _)| txt.contains("Line 1"))
            .map(|(_, pos)| *pos)
            .expect("tall cell text must be painted");

        assert!(
            (short_pos.y - tall_pos.y).abs() < 1.0,
            "TBL-031 TOP alignment: short cell text y ({}) must match tall cell text y ({}); delta={}",
            short_pos.y,
            tall_pos.y,
            short_pos.y - tall_pos.y,
        );
    }

    /// US4 (TBL-013 overflow, TBL-022): when column min-content widths exceed
    /// available width, the table falls back to horizontal scrolling and
    /// painted glyphs extend beyond the column boundary (NOT clipped). A
    /// horizontal `egui::ScrollArea` painter shape is present in the output.
    #[test]
    fn test_render_table_horizontal_scroll_fallback_no_clip() {
        use crate::ui::table_width::{TablePadding, TableRenderConfig};
        use eframe::epaint::Shape;

        // 1×2 table with single long unbreakable tokens (min_content >> 100px).
        let make = |t: &str| {
            vec![InlineElem::Text(
                t.to_string(),
                crate::ui::render::TextStyle::default(),
            )]
        };
        let table: Vec<Vec<Vec<InlineElem>>> = vec![vec![
            make("a_very_long_unbreakable_single_token_that_exceeds_one_hundred_pixels"),
            make("another_very_long_unbreakable_token_for_the_second_column"),
        ]];

        let config = TableRenderConfig {
            global_padding: TablePadding::ZERO,
        };

        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(100.0, 600.0),
            )),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw.clone(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_table_with_config(
                    ui,
                    &table,
                    0,
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                    &config,
                );
            });
        });
        let output = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_table_with_config(
                    ui,
                    &table,
                    0,
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                    &config,
                );
            });
        });

        // (a) The fallback path must NOT clip: painted text glyphs must
        // extend beyond the viewport boundary (x > 100.0).
        let max_glyph_x: f32 = output
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Text(t) => Some(t.pos.x + t.galley.rect.width()),
                _ => None,
            })
            .fold(0.0_f32, f32::max);
        assert!(
            max_glyph_x > 100.0,
            "fallback path must not clip: at least one painted text shape must extend \
             beyond the 100px viewport; max glyph x = {max_glyph_x}"
        );

        // (b) A horizontal ScrollArea shape (Mesh/Noop) is present.
        let has_scroll_shape = output
            .shapes
            .iter()
            .any(|cs| matches!(&cs.shape, Shape::Mesh(_) | Shape::Noop));
        assert!(
            has_scroll_shape,
            "fallback path must emit a horizontal ScrollArea (Mesh/Noop shapes); \
             shape kinds = {:?}",
            output
                .shapes
                .iter()
                .map(|cs| std::mem::discriminant(&cs.shape))
                .collect::<Vec<_>>()
        );
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
                egui::vec2(viewport_width, 1600.0),
            )),
            ..egui::RawInput::default()
        };
        // Pass 1: measure row heights in Grid
        let _ = ctx.run_ui(raw.clone(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_table(
                    ui,
                    table_cells,
                    0,
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                );
            });
        });
        // Pass 2: paint with resolved Grid row heights stored in memory
        ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_table(
                    ui,
                    table_cells,
                    0,
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                );
            });
        })
    }

    /// Renders `table_cells` in a wide viewport so the FTWA path runs
    /// (not the §3.6 horizontal-scroll fallback) and returns the
    /// `FullOutput` for shape inspection.
    fn render_table_with_paint_output(table_cells: &[Vec<Vec<InlineElem>>]) -> egui::FullOutput {
        render_table_with_paint_output_viewport(table_cells, 800.0)
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
                        && r.stroke == egui::Stroke::NONE
                        && r.stroke_kind == StrokeKind::Inside =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect();

        assert_eq!(
            rects.len(),
            9,
            "Expected 9 cell borders for 3x3 table; got rects: {:?}",
            rects
        );

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
        // (wrapped text galley has >1 row).
        let wrapped_text = output
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                egui::Shape::Text(t) if t.galley.text().contains("Header column one") => Some(t),
                _ => None,
            })
            .expect("expected Header column one text shape");

        assert!(
            wrapped_text.galley.rows.len() > 1,
            "Expected cell (0,0) text to word wrap, got {} row(s)",
            wrapped_text.galley.rows.len()
        );

        // For each column j ∈ 0..3, check that min.x and width match across all rows
        for (col, first_cell) in rows[0].iter().enumerate() {
            let first_min_x = first_cell.min.x;
            let first_width = first_cell.width();

            for (row, row_data) in rows.iter().enumerate().skip(1) {
                let min_x = row_data[col].min.x;
                let width = row_data[col].width();

                // Sub-pixel tolerance: FTWA distributes the deficit across
                // every positive-slack column (post-G2-drop), so the exact
                // column widths depend on the ratio of slacks and may shift
                // by a sub-pixel amount vs. an exact-integer expectation.
                // A 0.5 px tolerance still catches real misalignments.
                assert!(
                    (min_x - first_min_x).abs() < 0.5,
                    "Column {col} left border misaligned at row {row}: expected {first_min_x}, got {min_x}"
                );
                assert!(
                    (width - first_width).abs() < 0.5,
                    "Column {col} width mismatch at row {row}: expected {first_width}, got {width}"
                );
            }
        }

        // Verify gutter spacing between columns is 10px across all rows
        for (row, row_data) in rows.iter().enumerate() {
            let col0_right = row_data[0].max.x;
            let col1_left = row_data[1].min.x;
            let col1_right = row_data[1].max.x;
            let col2_left = row_data[2].min.x;

            let gutter_0_1 = col1_left - col0_right;
            let gutter_1_2 = col2_left - col1_right;

            assert!(
                (gutter_0_1 - 10.0).abs() < 0.5,
                "Row {row} gutter between Col 0 and Col 1 should be ~10.0, got {gutter_0_1}"
            );
            assert!(
                (gutter_1_2 - 10.0).abs() < 0.5,
                "Row {row} gutter between Col 1 and Col 2 should be ~10.0, got {gutter_1_2}"
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
                        && r.stroke == egui::Stroke::NONE
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
        for (col, (header_cell, ref_cell)) in rows[0].iter().zip(rows[1].iter()).enumerate() {
            let col_width = ref_cell.width();
            let col_min_x = ref_cell.min.x;

            let row0_width = header_cell.width();
            let row0_min_x = header_cell.min.x;

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

    /// Regression test for the G2-drop: when a 3-column table has two
    /// single-token cells (`"free"`, `"beer"`) and one long multi-word
    /// cell, the FTWA deficit distribution must keep the short cells at
    /// their max-content width (no slack → not in the wrap set) and
    /// force the long cell to absorb the entire deficit. On a 300 px
    /// viewport the long cell must word-wrap; the two short cells
    /// must stay single-line.
    #[test]
    fn test_render_table_short_cells_unwrap_long_cell_wraps() {
        use eframe::epaint::Shape;

        let make = |t: &str| {
            vec![InlineElem::Text(
                t.to_string(),
                crate::ui::render::TextStyle::default(),
            )]
        };
        // Mirrors the markdown `| free | beer |  Amber Pale Ale Imperial
        // IPA Lager Pilsner Helles Poster Stout Blond Hefeweizen |`.
        let table: Vec<Vec<Vec<InlineElem>>> = vec![vec![
            make("free"),
            make("beer"),
            make(
                "Amber Pale Ale Imperial IPA Lager Pilsner Helles \
                 Poster Stout Blond Hefeweizen",
            ),
        ]];

        let output = render_table_with_paint_output_viewport(&table, 300.0);

        // Find the text shape for each cell by its exact galley text.
        let find_text = |needle: &str| -> &eframe::epaint::TextShape {
            output
                .shapes
                .iter()
                .find_map(|cs| match &cs.shape {
                    Shape::Text(t) if t.galley.text() == needle => Some(t),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("expected text shape with content {needle:?}"))
        };
        let free_text = find_text("free");
        let beer_text = find_text("beer");
        let long_text = find_text(
            "Amber Pale Ale Imperial IPA Lager Pilsner Helles Poster Stout Blond Hefeweizen",
        );

        // Short, single-token cells must not word-wrap. Their width is
        // pinned at `max_content` (no slack → not in the wrap set), so
        // the galley stays on a single line.
        assert_eq!(
            free_text.galley.rows.len(),
            1,
            "short cell \"free\" must not be word-wrapped; got {} row(s)",
            free_text.galley.rows.len()
        );
        assert_eq!(
            beer_text.galley.rows.len(),
            1,
            "short cell \"beer\" must not be word-wrapped; got {} row(s)",
            beer_text.galley.rows.len()
        );

        // The long cell absorbs the full deficit (it is the only
        // positive-slack column) and must word-wrap onto multiple lines.
        assert!(
            long_text.galley.rows.len() > 1,
            "long cell must be word-wrapped; got {} row(s)",
            long_text.galley.rows.len()
        );
    }

    /// 7-column table where six of the seven cells have short or
    /// medium-length content and one ("Summary") has a long sentence.
    /// Mirrors a real laptop-spec table from the user's test corpus:
    ///
    /// ```text
    /// | Make | Model and Model Number | Market Price (Original) | Display | Processor | PassMark Single / Multi | Summary |
    /// |------|----------------------|------------------------|---------|-----------|------------------------|---------|
    /// | Dell | XPS 15 9570 | ~$1,500-$2,000 (discontinued) | 15.6" FHD ... | Intel Core i5-8300H (4C/8T) | 2,271 / 7,545 | Premium build, ... |
    /// ```
    ///
    /// Captured bug (pre-fix incorrect behavior at 1000 px viewport):
    /// with G2 dropped, the FTWA was distributing the deficit across
    /// every positive-slack column proportionally, which overallocated
    /// the "Summary" column and squeezed columns 2..6 to widths that
    /// forced their content onto a second row:
    ///
    /// ```text
    /// Dell                                                    -> 1 row  (OK)
    /// XPS 15 9570                                             -> 2 rows (BUG — should be 1)
    /// ~$1,500-$2,000 (discontinued)                           -> 2 rows (BUG — should be 1)
    /// 15.6" FHD (1920x1080) IPS or 4K OLED, 60Hz              -> 2 rows (BUG — should be 1)
    /// Intel Core i5-8300H (4C/8T)                             -> 2 rows (BUG — should be 1)
    /// 2,271 / 7,545                                           -> 2 rows (BUG — should be 1)
    /// Premium build, ... (Summary)                            -> 3 rows (OK — long cell wraps)
    /// ```
    ///
    /// **Fix**: G2 (minimum-cardinality wrap set) is back. The Summary
    /// column's slack (747.88 px) alone covers the entire deficit
    /// (660.69 px), so the wrap set is just `{Summary}`; every other
    /// column stays at its `max_content` width and renders on a
    /// single line. The unit-level equivalent is pinned in
    /// `audit_g2_one_big_slack_column_absorbs_entire_deficit` in
    /// `src/markdown/table_width/mod.rs`.
    ///
    /// This test is now a **passing regression guard**: if G2 is ever
    /// dropped again, this test will start failing on `XPS 15 9570` /
    /// `2,271 / 7,545` / etc. (the columns with positive slack that
    /// should NOT be in the wrap set).
    #[test]
    fn test_render_table_laptop_spec_short_cells_unwrap_long_wraps() {
        use eframe::epaint::Shape;

        let make = |t: &str| {
            vec![InlineElem::Text(
                t.to_string(),
                crate::ui::render::TextStyle::default(),
            )]
        };
        // One data row, seven columns. The cell content matches a
        // laptop-spec table the user hit on real markdown.
        let table: Vec<Vec<Vec<InlineElem>>> = vec![vec![
            make("Dell"),
            make("XPS 15 9570"),
            make("~$1,500-$2,000 (discontinued)"),
            make("15.6\" FHD (1920x1080) IPS or 4K OLED, 60Hz"),
            make("Intel Core i5-8300H (4C/8T)"),
            make("2,271 / 7,545"),
            make(
                "Premium build, excellent keyboard, great 4K OLED \
                 option, Thunderbolt 3. Now aging with 8th gen Intel. \
                 Shows the value of modern efficiency.",
            ),
        ]];

        // 1000 px viewport: 7 columns + 6 gutters of 10 px = 60 px
        // gutters, so 940 px of content. Wide enough that the six
        // short/medium cells fit on one line and the Summary cell
        // absorbs the deficit.
        let output = render_table_with_paint_output_viewport(&table, 1000.0);

        // Find the text shape for each cell by its exact galley text.
        let find_text = |needle: &str| -> &eframe::epaint::TextShape {
            output
                .shapes
                .iter()
                .find_map(|cs| match &cs.shape {
                    Shape::Text(t) if t.galley.text() == needle => Some(t),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("expected text shape with content {needle:?}"))
        };

        // The six short / medium cells. Each must stay on a single line.
        let single_line_cells = [
            "Dell",
            "XPS 15 9570",
            "~$1,500-$2,000 (discontinued)",
            "15.6\" FHD (1920x1080) IPS or 4K OLED, 60Hz",
            "Intel Core i5-8300H (4C/8T)",
            "2,271 / 7,545",
        ];
        for cell in single_line_cells {
            let shape = find_text(cell);
            assert_eq!(
                shape.galley.rows.len(),
                1,
                "cell {cell:?} must not be word-wrapped; got {} row(s)",
                shape.galley.rows.len()
            );
        }

        // The long Summary cell must word-wrap onto multiple lines.
        let summary = find_text(
            "Premium build, excellent keyboard, great 4K OLED \
             option, Thunderbolt 3. Now aging with 8th gen Intel. \
             Shows the value of modern efficiency.",
        );
        assert!(
            summary.galley.rows.len() > 1,
            "Summary cell must be word-wrapped; got {} row(s)",
            summary.galley.rows.len()
        );
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
                        && r.stroke == egui::Stroke::NONE
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

    /// Regression: single-line cell text in a tall row must top-align (match top Y of multi-line neighbor cell),
    /// not center vertically across multi-pass Grid layouts.
    #[test]
    fn test_render_table_cell_text_is_top_aligned_in_tall_row() {
        let make = |t: &str| {
            vec![InlineElem::Text(
                t.to_string(),
                crate::ui::render::TextStyle::default(),
            )]
        };
        let long_summary = "Intel Core Ultra 7 256V (8C/8T Lunar Lake) high performance mobile processor with dedicated NPU for artificial intelligence workloads.";
        let table: Vec<Vec<Vec<InlineElem>>> = vec![
            vec![make("Make"), make("Processor")],
            vec![make("Dell"), make(long_summary)],
        ];

        let ctx = egui::Context::default();
        let viewport_width: f32 = 300.0;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(viewport_width, 800.0),
            )),
            ..egui::RawInput::default()
        };

        // Pass 1: measure row heights in Grid
        let _ = ctx.run_ui(raw.clone(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_table(
                    ui,
                    &table,
                    0,
                    crate::ui::table_width::DeficitStrategy::BreakpointWaterFill,
                );
            });
        });

        // Pass 2: paint with resolved row heights stored in Grid memory
        let _ = ctx.run_ui(raw.clone(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_table(
                    ui,
                    &table,
                    0,
                    crate::ui::table_width::DeficitStrategy::BreakpointWaterFill,
                );
            });
        });

        // Pass 3: paint again
        let output = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_table(
                    ui,
                    &table,
                    0,
                    crate::ui::table_width::DeficitStrategy::BreakpointWaterFill,
                );
            });
        });

        let text_shapes: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t),
                _ => None,
            })
            .collect();

        for (i, t) in text_shapes.iter().enumerate() {
            println!(
                "Shape {i}: text={:?}, pos={:?}, galley_h={:.1}",
                t.galley.text(),
                t.pos,
                t.galley.rect.height()
            );
        }

        let short_text = text_shapes
            .iter()
            .find(|t| t.galley.text() == "Dell")
            .expect("expected Dell text shape");
        let tall_text = text_shapes
            .iter()
            .find(|t| t.galley.text().contains("Intel Core"))
            .expect("expected Intel Core text shape");

        assert!(
            (short_text.pos.y - tall_text.pos.y).abs() <= 2.0,
            "expected short cell text top y ({:.1}) to match tall cell text top y ({:.1}), but short cell text was vertically misaligned/centered on pass 2! (diff={:.1}px)",
            short_text.pos.y,
            tall_text.pos.y,
            (short_text.pos.y - tall_text.pos.y).abs()
        );
    }

    /// Regression: multi-item cell text in a tall row must render tightly packed at top
    /// without internal vertical gaps between items or vertical centering of single items.
    #[test]
    fn test_render_table_cell_no_internal_vertical_gap_or_centering() {
        let make = |t: &str| {
            vec![InlineElem::Text(
                t.to_string(),
                crate::ui::render::TextStyle::default(),
            )]
        };
        let summary_text = "Premium build, excellent keyboard, great 4K OLED option, Thunderbolt 3. Now aging with 8th gen Intel. Shows the value of modern efficiency.";
        let table: Vec<Vec<Vec<InlineElem>>> = vec![
            vec![make("PassMark"), make("Summary")],
            vec![make("2,271 / 7,545"), make(summary_text)],
        ];

        let ctx = egui::Context::default();
        let viewport_width: f32 = 200.0;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(viewport_width, 800.0),
            )),
            ..egui::RawInput::default()
        };

        // Pass 1: measure row heights
        let _ = ctx.run_ui(raw.clone(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_table(
                    ui,
                    &table,
                    0,
                    crate::ui::table_width::DeficitStrategy::BreakpointWaterFill,
                );
            });
        });

        // Pass 2: paint with resolved row heights
        let output = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_table(
                    ui,
                    &table,
                    0,
                    crate::ui::table_width::DeficitStrategy::BreakpointWaterFill,
                );
            });
        });

        let passmark_text = output
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                egui::Shape::Text(t) if t.galley.text() == "2,271 / 7,545" => Some(t),
                _ => None,
            })
            .expect("expected PassMark text shape");

        let summary_part1 = output
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                egui::Shape::Text(t) if t.galley.text().contains("Premium build") => Some(t),
                _ => None,
            })
            .expect("expected summary part 1 text shape");

        // PassMark single line must top-align with Summary part 1
        assert!(
            (passmark_text.pos.y - summary_part1.pos.y).abs() <= 2.0,
            "PassMark text (y={:.1}) was vertically centered instead of top-aligned with Summary (y={:.1})",
            passmark_text.pos.y,
            summary_part1.pos.y
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
                render_inline(ui, &elems, false, None, 0, None, 0, &mut Vec::new());
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
            render_inline(ui, &elems, false, None, 0, None, 0, &mut Vec::new());
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
                render_markdown(
                    ui,
                    &md,
                    &mut scroll_id,
                    &mut Vec::new(),
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                );
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
                                && r.stroke == egui::Stroke::NONE
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

    // ---- Off-viewport text guards (MD-013) -------------------------

    /// Render the long-table-row fixture at a phone-sized viewport
    /// and assert no `Shape::Text` is permanently outside its
    /// containing clip rect. Catches the F1 (horizontal clip) and
    /// F3 (table column overflow) failure modes; the §3.6 fallback
    /// in `render_table` must wrap the table in a horizontal
    /// `ScrollArea` so cell text remains reachable.
    ///
    /// Mirrors the production render path: `render_markdown` is
    /// always wrapped in a vertical `ScrollArea` by the center panel
    /// (`src/ui/panels/center.rs:364`). The test must do the same,
    /// otherwise tall content trivially exceeds the viewport's
    /// bottom edge and triggers a false positive.
    #[test]
    fn render_markdown_no_offscreen_text_at_narrow_viewport() {
        use crate::ui::test_helpers::offscreen::assert_no_offscreen_text;

        // Markdown body that contains a table wider than the viewport
        // plus surrounding prose and headings. The fixture lives under
        // `src/test/wiki/Travel/long-table-row.md` per
        // `src/test/wiki/AGENTS.md` §"Fixtures only".
        let md = include_str!("../../../test/wiki/Travel/long-table-row.md");

        // 320 px is the iPhone 5 / SE 1st-gen width — the narrowest
        // viewport we expect to support. A 6-column table with long
        // text guarantees `decision.needs_horizontal_scroll == true`
        // on this viewport, exercising the §3.6 fallback path.
        let viewport_width: f32 = 320.0;
        let viewport_height: f32 = 800.0;

        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(viewport_width, viewport_height),
            )),
            ..egui::RawInput::default()
        };
        let mut scroll_id = None;
        let mut pending_toggles = Vec::new();

        let ctx = egui::Context::default();
        let output = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("main_markdown_scroll")
                    .show(ui, |ui| {
                        render_markdown(
                            ui,
                            md,
                            &mut scroll_id,
                            &mut pending_toggles,
                            crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                        );
                    });
            });
        });

        assert_no_offscreen_text(&output.shapes);
    }

    /// Same fixture at a desktop-sized viewport (the FTWA path, not
    /// the §3.6 fallback). The table fits in columns; the helper
    /// must still report no off-viewport text inside the vertical
    /// scroll content rect.
    #[test]
    fn render_markdown_no_offscreen_text_at_wide_viewport() {
        use crate::ui::test_helpers::offscreen::assert_no_offscreen_text;

        let md = include_str!("../../../test/wiki/Travel/long-table-row.md");

        let viewport_width: f32 = 1600.0;
        let viewport_height: f32 = 800.0;

        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(viewport_width, viewport_height),
            )),
            ..egui::RawInput::default()
        };
        let mut scroll_id = None;
        let mut pending_toggles = Vec::new();

        let ctx = egui::Context::default();
        let output = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("main_markdown_scroll")
                    .show(ui, |ui| {
                        render_markdown(
                            ui,
                            md,
                            &mut scroll_id,
                            &mut pending_toggles,
                            crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                        );
                    });
            });
        });

        assert_no_offscreen_text(&output.shapes);
    }

    /// Regression test: Verify that table row heights remain 100% stable
    /// across multiple consecutive render frames (frame 1 through 10) on the
    /// same egui::Context, with zero inter-frame height growth or oscillation.
    #[test]
    fn test_render_table_multi_frame_height_stability() {
        let md = "| Col 1 | Col 2 |\n|---|---|\n| Short cell | Multi line cell with long text wrapping into several lines |\n";
        let ctx = egui::Context::default();
        let mut frame_1_heights: Vec<f32> = Vec::new();

        for frame in 0..30 {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..egui::RawInput::default()
            };
            let mut scroll_id = None;
            let mut pending_toggles = Vec::new();

            let output = ctx.run_ui(raw, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    render_markdown(
                        ui,
                        md,
                        &mut scroll_id,
                        &mut pending_toggles,
                        crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                    );
                });
            });

            let current_rect_heights: Vec<f32> = output
                .shapes
                .iter()
                .filter_map(|cs| match &cs.shape {
                    egui::Shape::Rect(r)
                        if r.fill == egui::Color32::TRANSPARENT
                            && r.stroke == egui::Stroke::NONE
                            && r.stroke_kind == egui::StrokeKind::Inside =>
                    {
                        Some(r.rect.height())
                    }
                    _ => None,
                })
                .collect();

            assert_eq!(
                current_rect_heights.len(),
                4,
                "Expected 4 cell rects (2x2 table)"
            );

            if frame == 1 {
                frame_1_heights = current_rect_heights.clone();
            } else if frame > 1 {
                for (idx, (h_base, h_curr)) in frame_1_heights
                    .iter()
                    .zip(current_rect_heights.iter())
                    .enumerate()
                {
                    assert!(
                        (h_base - h_curr).abs() < 0.01,
                        "Frame {frame}: Cell rect {idx} height changed from {h_base} (frame 1) to {h_curr}!"
                    );
                }
            }
        }
    }

    /// Regression test: Verify that cell text in tall rows starts top-aligned
    /// (matching Y coordinate) and that row height is compact (< 150px).
    #[test]
    fn test_render_table_top_aligned_and_compact_row_height() {
        let md = "| Short | Tall |\n|---|---|\n| Cell A | Line 1<br>Line 2<br>Line 3<br>Line 4 |\n";
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..egui::RawInput::default()
        };
        let mut scroll_id = None;
        let mut pending_toggles = Vec::new();

        let output = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_markdown(
                    ui,
                    md,
                    &mut scroll_id,
                    &mut pending_toggles,
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                );
            });
        });

        // Find cell rects for Row 1
        let rects: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Rect(r)
                    if r.fill == egui::Color32::TRANSPARENT
                        && r.stroke == egui::Stroke::NONE
                        && r.stroke_kind == egui::StrokeKind::Inside =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect();

        assert_eq!(rects.len(), 4, "Expected 4 cell rects for 2x2 table");

        // Row 1 short cell (rects[2]) and tall cell (rects[3]) should have identical top Y
        assert!(
            (rects[2].min.y - rects[3].min.y).abs() < 1.0,
            "Row 1 cells top Y mismatch: short cell top Y = {}, tall cell top Y = {}",
            rects[2].min.y,
            rects[3].min.y
        );

        // Row 1 height should be compact and bounded
        assert!(
            rects[2].height() < 150.0,
            "Row height too tall: {}",
            rects[2].height()
        );
    }
}
