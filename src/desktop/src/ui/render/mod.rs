//! Pulldown-cmark event-driven markdown renderer — emits egui widgets for
//! headings, paragraphs, code blocks, lists, tables, links, and images.
//!
//! The module is split into per-concern submodules:
//!
//! - [`inline`]    — inline text styling (the leaf for paragraphs/list items)
//! - [`code`]      — fenced code blocks with copy-to-clipboard button
//! - [`heading`]   — heading widgets and the scroll-to-me side effect
//! - [`yaml_table`] — YAML front-matter key/value table
//! - `table::cell`        — single table cell at a known pinned width
//! - `table::configured`  — table dispatcher with explicit `TableRenderConfig`
//! - `table::dispatch`    — test-only default-config wrapper
//!
//! The top-level [`render_markdown`] function is the only entry point
//! used by the UI; it walks the `RenderEvent` stream and dispatches
//! each event to the appropriate submodule. Each submodule is
//! independently testable: the leaf functions are stateless apart
//! from the `pending_toggles` sink that `render_markdown` plumbs
//! through to `inline::render_inline` for the task-checkbox case.

pub mod code;
pub mod heading;
pub mod inline;
mod table;
pub mod yaml_table;

// Markdown re-exports — preserved verbatim from the old `render.rs`
// `pub use` block so the public surface is identical to before the
// module split. `apply_task_toggle` and `build_toc` are used by
// `ui/panels/center.rs`; the rest are used by the renderer itself
// and the e2e tests in this module.
pub use crate::markdown::{
    InlineElem, RenderEvent, TextStyle, apply_task_toggle, build_toc, heading_plain_text,
    parse_markdown_to_events, parse_yaml_to_pairs,
};

// `pub(crate) use` (not plain `use`) so the e2e_tests submodule (a sibling
// of `code`/`heading`/`inline`/`table` under `render`) can reach these
// items via `super::render_code_block` etc. and re-export them to its
// own test submodules. The original monolithic `render.rs` had them all
// in one module; after the split we have to widen visibility to
// `pub(crate)` so the re-exports don't cause E0364.
#[cfg(test)]
pub(crate) use code::copy_code_to_output;
pub(crate) use code::render_code_block;
pub(crate) use heading::render_heading;
pub(crate) use inline::render_inline;
pub(crate) use table::render_table_with_config;
pub(crate) use yaml_table::render_yaml_table;

use eframe::egui;

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
///
/// `heading_ids` is an optional pre-computed slice of heading IDs
/// (with duplicate disambiguation). If provided, avoids re-parsing
/// headings and re-computing IDs on every frame.
pub fn render_markdown(
    ui: &mut egui::Ui,
    markdown_text: &str,
    scroll_to_id_str: &mut Option<String>,
    pending_toggles: &mut Vec<(usize, bool)>,
    strategy: crate::ui::table_width::DeficitStrategy,
    heading_ids: Option<&[String]>,
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

    // Iterator over pre-computed heading IDs if provided.
    let mut heading_ids_iter = heading_ids.map(|ids| ids.iter().peekable());

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
                RenderEvent::CodeBlock { content, .. } => {
                    let line_count = content.lines().count().max(1) as f32;
                    let est_h = line_count * 18.0 + 20.0;
                    ui.add_space(est_h);
                    continue;
                }
                RenderEvent::Heading { level, .. } => {
                    let size = match level {
                        1 => 32.0,
                        2 => 24.0,
                        3 => 18.0,
                        4 => 14.0,
                        _ => 12.0,
                    };
                    ui.add_space(size + 8.0);
                    continue;
                }
                RenderEvent::Table(cells) => {
                    // Estimate table height: header + rows
                    let row_count = cells.len().max(1) as f32;
                    let est_h = row_count * 22.0 + 20.0;
                    ui.add_space(est_h);
                    continue;
                }
                RenderEvent::Space(amount) => {
                    ui.add_space(*amount);
                    continue;
                }
                RenderEvent::Separator => {
                    ui.add_space(4.0);
                    continue;
                }
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
            RenderEvent::CodeBlock { content, .. } => {
                render_code_block(ui, content);
            }
            RenderEvent::Heading { level, elems } => {
                let text = heading_plain_text(elems);
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let heading_id_str = if let Some(iter) = &mut heading_ids_iter {
                    iter.next().unwrap().clone()
                } else {
                    heading_id_for(trimmed)
                };
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;

#[cfg(test)]
mod e2e_tests;
