//! End-to-end tests for the render module, split by concern so no
//! single file crosses the 1024-line limit.
//!
//! Submodules:
//!
//! - [`render_smoke`]       — top-level `render_markdown` / `render_yaml_table`
//!   smoke, YAML front-matter wrap/overlap, heading scroll-to-id, FTWA measure.
//! - [`ftwa`]               — FTWA regimes (surplus / deficit / §3.6
//!   fallback), per-cell markdown styling, padding, alignment, scroll
//!   fallback, single-line cell top-alignment in tall rows.
//! - [`table_alignment`]    — column alignment across rows, empty
//!   header cells, short-vs-long cell wrapping, the G2-drop regression
//!   guards, multi-line row top-alignment.
//! - [`interactions`]       — click handlers (copy-code, hyperlink, task
//!   checkbox), `apply_task_toggle` CRLF preservation, multi-table
//!   layout across one document.
//! - [`table_regressions`]  — off-viewport text guards at narrow / wide
//!   viewports, multi-frame height stability, phantom-row-height
//!   regression, single-line cell at row top, top-aligned compact row.
//!
//! Shared helpers (`render_table_with_viewport`, `build_uniform_table`,
//! `commands_capture`, …) live in [`helpers`].
//!
//! ## Why the re-export fan-out below
//!
//! Before the `ui/render.rs` → `ui/render/` split, the monolithic
//! `mod e2e_tests { use super::*; ... }` block lived inside the
//! same file as the rendering functions, so `use super::*;` was
//! enough to bring them in. After the split, the test files live
//! in submodules of `e2e_tests/`, while the rendering functions
//! live in sibling submodules of `render/` (e.g. `code`, `inline`,
//! `table::configured`, `table::dispatch`). The sibling submodules
//! expose their functions as `pub(super)` (visible to `render/`'s
//! children), so `render/mod.rs` re-exports them with
//! `pub(super) use` and we re-export them again here. `eframe` and
//! `egui` are also re-exported so the test files can write
//! `egui::Context::default()` and `eframe::epaint::Shape` after a
//! single `use super::*;`.

#![cfg(test)]

mod agent_restyle;
mod commonmark_parser;
mod commonmark_render;
mod commonmark_snapshots;
mod ftwa;
mod helpers;
mod interactions;
mod pulldown_config;
mod render_smoke;
mod table_alignment;
mod table_layout;
mod table_regressions;
mod table_visual_layout;

// Pull in everything the parent `render/mod.rs` re-exports (the markdown
// re-exports + the `pub(crate) use` re-exports of the sibling submodule
// functions), then re-export them all with `pub(super)` so each test
// submodule can pick them up with `use super::*;`.
pub(super) use super::{
    // Markdown re-exports (these were `pub use` in the old `render.rs`).
    InlineElem,
    InlineRenderItem,
    RenderEvent,
    TextStyle,
    apply_task_toggle,
    // Sibling-submodule re-exports widened by `render/mod.rs` to
    // `pub(crate)` so the e2e_tests tree can reach them.
    copy_code_to_output,
    heading_plain_text,
    parse_markdown_to_events,
    render_code_block,
    render_heading,
    render_inline,
    // Public functions defined in `render/mod.rs`.
    render_markdown,
    render_table_with_config,
    render_yaml_table,
};

// Re-export `eframe` and `egui` so test files can write
// `egui::Context::default()` and `eframe::epaint::Shape` after
// `use super::*;`. These crates are not directly in scope of the
// test submodules (they only see what their parent re-exports), so
// the fan-out has to happen here.
pub(super) use eframe;
pub(super) use eframe::egui;

// Re-export all the shared helpers (table-rendering closures, cell
// builders, `commands_capture`) so `use super::*;` brings them in.
pub(crate) use helpers::*;
