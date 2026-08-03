//! Default-config dispatch wrapper for [`super::configured::render_table_with_config`].
//!
//! Production dispatch goes through `render_table_with_config` directly
//! with a real `TableRenderConfig`. The thin wrapper here is preserved
//! for test call sites that do not thread a config — equivalent to
//! calling `render_table_with_config` with `&TableRenderConfig::default()`
//! (global padding = ZERO).

#[cfg(test)]
use eframe::egui;

#[cfg(test)]
use super::configured::render_table_with_config;
#[cfg(test)]
use crate::markdown::InlineElem;

/// Render a markdown table with the default [`crate::ui::table_width::TableRenderConfig`].
///
/// Thin wrapper around [`render_table_with_config`] preserved for test
/// call sites that do not thread a config — equivalent to calling
/// `render_table_with_config` with `&TableRenderConfig::default()`
/// (global padding = ZERO). Production dispatch uses
/// `render_table_with_config` directly.
#[cfg(test)]
pub(crate) fn render_table(
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
