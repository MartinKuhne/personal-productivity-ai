//! Fair Table Width Algorithm (FTWA) — assigns per-column pixel widths to a markdown/GFM table.
//!
//! Reconciles two goals (see `doc/planning/table-column-width-algorithm.md`):
//!   **G1** minimize total word-wrap (extra wrapped lines) and
//!   **G3** use all available horizontal space.
//!
//! G2 ("minimize the number of columns that wrap") is no longer a goal: every
//! positive-slack column can participate in shrinking the deficit, and B2
//! distributes the deficit across all of them.
//!
//! Pure `ftwa` core lives in `crate::markdown::table_width` (pure layout math,
//! per `src/desktop/AGENTS.md §5`); this module bridges to egui text shaping via
//! `measure` / `measure_cached` / `ftwa_cached` and re-exports the pure API so
//! existing call sites (`crate::ui::table_width::ftwa_cached`) keep resolving
//! unchanged.

pub use crate::markdown::table_layout::{
    LayoutCell, TableLayout, TableLayoutBuilder, TextMeasurer,
};
pub use crate::markdown::table_width::{
    Breakpoint, CellTokens, ColumnWidths, DeficitStrategy, compute_column_breakpoints, ftwa,
};
use crate::ui::render::InlineElem;
use eframe::egui;

/// Per-cell padding in logical pixels applied around cell content.
///
/// Padding is resolved per cell by taking the most-specific non-`None` layer
/// (per-cell override > per-column override > global default), then sanitised
/// (negative components clamped to `0.0`, satisfying `TBL-050`). Resolved
/// horizontal padding is factored into column width accounting (`TBL-033`):
/// every returned `max_content`/`min_content` is increased by
/// `padding.horizontal()`. Vertical padding is applied as inner frame margin
/// in `render_table_cell` and increases the rendered row height.
///
/// See [contracts/table-renderer.md](../../../specs/002-table-layout-renderer/contracts/table-renderer.md) Part C.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TablePadding {
    /// Top padding in logical pixels.
    pub top: f32,
    /// Bottom padding in logical pixels.
    pub bottom: f32,
    /// Left padding in logical pixels.
    pub left: f32,
    /// Right padding in logical pixels.
    pub right: f32,
}

impl TablePadding {
    /// Zero padding on every side — the no-arg default.
    pub const ZERO: Self = Self {
        top: 0.0,
        bottom: 0.0,
        left: 0.0,
        right: 0.0,
    };

    /// Clamp each negative component to `0.0` (`TBL-050` malformed-input branch).
    pub fn sanitised(self) -> Self {
        Self {
            top: self.top.max(0.0),
            bottom: self.bottom.max(0.0),
            left: self.left.max(0.0),
            right: self.right.max(0.0),
        }
    }

    /// Horizontal padding (`left + right`) added to width accounting.
    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }

    /// Vertical padding (`top + bottom`) added to row height accounting.
    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

impl Default for TablePadding {
    fn default() -> Self {
        Self::ZERO
    }
}

/// Resolve the effective padding for one cell from the most-specific
/// non-`None` layer (per-cell override > per-column override > global
/// default), per [contracts/table-renderer.md](../../../specs/002-table-layout-renderer/contracts/table-renderer.md) Part C.
///
/// Each `Option<TablePadding>` argument is documented in
/// [data-model.md](../../../specs/002-table-layout-renderer/data-model.md)
/// as `None`-defer-upper. `None` at a layer defers to the upper layer;
/// `Some` at any layer is taken as-is. The fallback when every layer is
/// `None` is `global`. Resolved value is sanitised (negative components
/// clamped to `0.0`) before returning.
pub fn resolve_padding(
    global: TablePadding,
    per_column: Option<&TablePadding>,
    per_cell: Option<&TablePadding>,
) -> TablePadding {
    let resolved = match (per_cell, per_column) {
        (Some(cell), _) => *cell,
        (None, Some(col)) => *col,
        (None, None) => global,
    };
    resolved.sanitised()
}

/// Cross-cutting table rendering configuration carried on `FastMdApp`
/// (per `src/desktop/AGENTS.md §2` "all cross-cutting state lives on
/// `FastMdApp`"). Holds the global padding default applied to every
/// cell that has no per-column or per-cell override.
///
/// See [contracts/table-renderer.md](../../../specs/002-table-layout-renderer/contracts/table-renderer.md) Part C.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct TableRenderConfig {
    /// Padding applied to every cell with no per-column/per-cell override.
    pub global_padding: TablePadding,
}

/// Measure the per-column max-content, min-content widths, and breakpoints of a table.
///
/// `max_content[j]` is the single-line width of column `j`'s widest cell;
/// `min_content[j]` is the width of its longest unbreakable token (whitespace
/// splits are the only break opportunities, per doc §5 Q2 "never break tokens").
/// `breakpoints[j]` is column `j`'s wrap-cost curve: at width `w`, the column
/// produces `extra_lines` wrapped lines (summed across all cells in the column).
/// Empty tables return empty vectors. Ragged rows are tolerated — only columns
/// that exist in some row get measured; missing cells contribute zero.
///
/// Font selection matches what `render_table_cell` actually paints: body font
/// for normal text, monospace for code spans, body font for links/html and for
/// the `[Image: …]` placeholder string.
///
/// `padding.horizontal()` is folded into every returned `max_content[j]` and
/// `min_content[j]` (`TBL-033` width accounting). The pure FTWA solver in
/// `ftwa` therefore receives column widths that already include the column's
/// horizontal padding; `render_table_cell` subtracts `padding.horizontal()`
/// from the FTWA-assigned width to get the content area for its inner layout.
pub fn measure(
    cells: &[Vec<Vec<InlineElem>>],
    padding: TablePadding,
    ui: &egui::Ui,
) -> (Vec<f32>, Vec<f32>, Vec<Vec<Breakpoint>>) {
    let n = cells.iter().map(|row| row.len()).max().unwrap_or(0);
    if n == 0 {
        return (Vec::new(), Vec::new(), Vec::new());
    }

    let body_font = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .cloned()
        .unwrap_or_else(|| egui::FontId::proportional(14.0));
    let mono_font = egui::FontId::new(body_font.size, egui::FontFamily::Monospace);
    // Color does not influence width, but the layout API requires one.
    let color = egui::Color32::WHITE;

    // Measure space width for breakpoint computation (use body font).
    let space_width = {
        let g = ui.fonts_mut(|f| f.layout_no_wrap(" ".to_string(), body_font.clone(), color));
        g.size().x
    };

    let mut max_w = vec![0.0_f32; n];
    let mut min_w = vec![0.0_f32; n];
    let mut cell_tokens_per_col: Vec<Vec<CellTokens>> = vec![Vec::new(); n];

    for row in cells {
        for (j, cell) in row.iter().enumerate() {
            let (cell_max, cell_min, tokens) =
                measure_cell(cell, ui, &body_font, &mono_font, color);
            if cell_max > max_w[j] {
                max_w[j] = cell_max;
            }
            if cell_min > min_w[j] {
                min_w[j] = cell_min;
            }
            cell_tokens_per_col[j].push(tokens);
        }
    }

    // Guard against degenerate all-empty tables producing zero-width columns:
    // egui would otherwise collapse those columns entirely.
    for w in &mut max_w {
        if *w <= 0.0 {
            *w = 1.0;
        }
    }
    for w in &mut min_w {
        if *w <= 0.0 {
            *w = 1.0;
        }
    }

    // Compute per-column breakpoints by merging cell-level breakpoints.
    let breakpoints = cell_tokens_per_col
        .iter()
        .map(|cell_tokens| compute_column_breakpoints(cell_tokens, space_width))
        .collect();

    // Fold horizontal padding into the returned column widths so FTWA
    // allocates "content + padding" per column (`TBL-033`). Vertical padding
    // is applied by `render_table_cell`'s inner frame and grows row height
    // at paint time — it does not feed back into column width accounting.
    let pad_h = padding.sanitised().horizontal();
    for w in &mut max_w {
        *w += pad_h;
    }
    for w in &mut min_w {
        *w += pad_h;
    }

    (max_w, min_w, breakpoints)
}

/// Internal cached measurements for a table to avoid redundant egui text shaping.
#[derive(Clone, Debug)]
pub struct TableMeasureCache {
    /// Hash of the table cells AST.
    pub cell_hash: u64,
    /// Hash of the egui font style parameters.
    pub font_hash: u64,
    /// Measured max-content widths per column.
    pub max_w: Vec<f32>,
    /// Measured min-content widths per column.
    pub min_w: Vec<f32>,
    /// Measured breakpoints per column.
    pub breakpoints: Vec<Vec<Breakpoint>>,
}

/// Internal cached decision for a table solver invocation to avoid redundant FTWA calculation.
#[derive(Clone, Debug)]
pub struct TableDecisionCache {
    /// Hash of input max_content and min_content widths.
    pub input_hash: u64,
    /// Hash of the egui font style parameters.
    pub font_hash: u64,
    /// Available width during solver pass.
    pub avail: f32,
    /// Strategy used for solver pass.
    pub strategy: DeficitStrategy,
    /// Resulting layout decision.
    pub layout: TableLayout,
}

pub struct EguiTextMeasurer<'a> {
    ui: &'a mut egui::Ui,
    body_font: egui::FontId,
    mono_font: egui::FontId,
    color: egui::Color32,
    space_width: f32,
    line_height: f32,
}

impl<'a> EguiTextMeasurer<'a> {
    pub fn new(ui: &'a mut egui::Ui) -> Self {
        let body_font = ui
            .style()
            .text_styles
            .get(&egui::TextStyle::Body)
            .cloned()
            .unwrap_or_else(|| egui::FontId::proportional(14.0));
        let mono_font = egui::FontId::new(body_font.size, egui::FontFamily::Monospace);
        let color = egui::Color32::WHITE;
        let space_width = {
            let g = ui.fonts_mut(|f| f.layout_no_wrap(" ".to_string(), body_font.clone(), color));
            g.size().x
        };
        let line_height = ui.text_style_height(&egui::TextStyle::Body);

        Self {
            ui,
            body_font,
            mono_font,
            color,
            space_width,
            line_height,
        }
    }
}

impl<'a> TextMeasurer for EguiTextMeasurer<'a> {
    fn measure_cell(&self, cell: &[InlineElem]) -> (f32, f32, CellTokens) {
        measure_cell(cell, self.ui, &self.body_font, &self.mono_font, self.color)
    }

    fn space_width(&self) -> f32 {
        self.space_width
    }

    fn line_height(&self) -> f32 {
        self.line_height
    }
}

/// Computes the table layout with decision memoization via egui memory.
pub fn table_layout_cached(
    table_id: egui::Id,
    cells: &[Vec<Vec<InlineElem>>],
    padding: TablePadding,
    available_width: f32,
    strategy: DeficitStrategy,
    ui: &mut egui::Ui,
) -> TableLayout {
    use std::hash::{Hash, Hasher};

    let pad_h = padding.sanitised().horizontal();
    let pad_v = padding.sanitised().vertical();

    let mut cell_hasher = std::collections::hash_map::DefaultHasher::new();
    cells.hash(&mut cell_hasher);
    pad_h.to_bits().hash(&mut cell_hasher);
    pad_v.to_bits().hash(&mut cell_hasher);
    let cell_hash = cell_hasher.finish();

    let font_hash = {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        ui.style().text_styles.hash(&mut hasher);
        hasher.finish()
    };

    let cache_id = table_id.with("layout_cache");
    if let Some(cached) = ui
        .data(|d| d.get_temp::<TableDecisionCache>(cache_id))
        .filter(|c| {
            c.input_hash == cell_hash
                && c.font_hash == font_hash
                && c.strategy == strategy
                && (c.avail - available_width).abs() < 1e-3
        })
    {
        return cached.layout;
    }

    let measurer = EguiTextMeasurer::new(ui);
    let layout = TableLayoutBuilder::new(&measurer, available_width, strategy)
        .with_padding(pad_h, pad_v)
        .build(cells);

    let cache = TableDecisionCache {
        input_hash: cell_hash,
        font_hash,
        avail: available_width,
        strategy,
        layout: layout.clone(),
    };
    ui.data_mut(|d| d.insert_temp(cache_id, cache));

    layout
}

/// Measure table column widths and breakpoints with result memoization via egui memory.
///
/// Prevents re-running costly font text shaping on every frame when table content and font style
/// remain identical across renders.
///
/// `padding` is folded into the cache key (via `padding.horizontal().to_bits()`)
/// and threaded through to [`measure`] so
/// that changing the table's padding invalidates the cached measurement even
/// when the cell content and font are otherwise identical (`TBL-033`).
pub fn measure_cached(
    table_id: egui::Id,
    cells: &[Vec<Vec<InlineElem>>],
    padding: TablePadding,
    ui: &egui::Ui,
) -> (Vec<f32>, Vec<f32>, Vec<Vec<Breakpoint>>) {
    use std::hash::{Hash, Hasher};

    let pad_h = padding.sanitised().horizontal();

    let mut cell_hasher = std::collections::hash_map::DefaultHasher::new();
    cells.hash(&mut cell_hasher);
    pad_h.to_bits().hash(&mut cell_hasher);
    let cell_hash = cell_hasher.finish();

    let body_font = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .cloned()
        .unwrap_or_else(|| egui::FontId::proportional(14.0));
    let mut font_hasher = std::collections::hash_map::DefaultHasher::new();
    body_font.family.hash(&mut font_hasher);
    body_font.size.to_bits().hash(&mut font_hasher);
    let font_hash = font_hasher.finish();

    let cache_id = table_id.with("measure_cache");
    if let Some(cached) = ui
        .data(|d| d.get_temp::<TableMeasureCache>(cache_id))
        .filter(|c| c.cell_hash == cell_hash && c.font_hash == font_hash)
    {
        return (cached.max_w, cached.min_w, cached.breakpoints);
    }

    let (max_w, min_w, breakpoints) = measure(cells, padding, ui);

    let cache = TableMeasureCache {
        cell_hash,
        font_hash,
        max_w: max_w.clone(),
        min_w: min_w.clone(),
        breakpoints: breakpoints.clone(),
    };
    ui.data_mut(|d| d.insert_temp(cache_id, cache));

    (max_w, min_w, breakpoints)
}

/// Measure one cell's `(max_content, min_content)` width and collect token data.
///
/// `max_content` is the sum of every fragment's single-line `layout_no_wrap`
/// width (fragments are laid out flush, item_spacing.x = 0, in
/// `render_table_cell`). `min_content` is the longest whitespace-separated
/// token across all fragments, measured with the fragment's own font.
/// The returned `CellTokens` contains ordered token widths for breakpoint
/// computation.
fn measure_cell(
    cell: &[InlineElem],
    ui: &egui::Ui,
    body_font: &egui::FontId,
    mono_font: &egui::FontId,
    color: egui::Color32,
) -> (f32, f32, CellTokens) {
    let mut max_w = 0.0_f32;
    let mut min_w = 0.0_f32;
    let mut token_widths: Vec<f32> = Vec::new();

    let mut current_token = String::new();
    let mut current_font = body_font;

    let measure_token = |tok: &str, font: &egui::FontId, min_w: &mut f32, widths: &mut Vec<f32>| {
        if !tok.is_empty() {
            let g = ui.fonts_mut(|f| f.layout_no_wrap(tok.to_string(), font.clone(), color));
            let w = g.size().x;
            if w > *min_w {
                *min_w = w;
            }
            widths.push(w);
        }
    };

    for elem in cell {
        let (displayed, font) = match elem {
            InlineElem::Text(t, style) => {
                let f = if style.code { mono_font } else { body_font };
                (t.clone(), f)
            }
            InlineElem::Link(_, display) => (display.clone(), body_font),
            InlineElem::Image(url) => (format!("[Image: {}]", url), body_font),
            InlineElem::Html(h) => (h.clone(), body_font),
            InlineElem::SoftBreak => (" ".to_string(), body_font),
        };

        if displayed.is_empty() {
            continue;
        }

        let g = ui.fonts_mut(|f| f.layout_no_wrap(displayed.clone(), font.clone(), color));
        max_w += g.size().x;

        let parts: Vec<&str> = displayed.split_whitespace().collect();
        if parts.is_empty() {
            if !current_token.is_empty() {
                let tok = std::mem::take(&mut current_token);
                measure_token(&tok, current_font, &mut min_w, &mut token_widths);
            }
        } else {
            let starts_ws = displayed.chars().next().is_some_and(char::is_whitespace);
            let ends_ws = displayed.chars().last().is_some_and(char::is_whitespace);

            if starts_ws {
                if !current_token.is_empty() {
                    let tok = std::mem::take(&mut current_token);
                    measure_token(&tok, current_font, &mut min_w, &mut token_widths);
                }
                for &p in &parts[..parts.len() - 1] {
                    measure_token(p, font, &mut min_w, &mut token_widths);
                }
                if ends_ws {
                    measure_token(parts.last().unwrap(), font, &mut min_w, &mut token_widths);
                } else {
                    current_token.push_str(parts.last().unwrap());
                    current_font = font;
                }
            } else {
                current_token.push_str(parts[0]);
                if parts.len() > 1 {
                    let tok = std::mem::take(&mut current_token);
                    measure_token(&tok, current_font, &mut min_w, &mut token_widths);
                    for &p in &parts[1..parts.len() - 1] {
                        measure_token(p, font, &mut min_w, &mut token_widths);
                    }
                    if ends_ws {
                        measure_token(parts.last().unwrap(), font, &mut min_w, &mut token_widths);
                    } else {
                        current_token.push_str(parts.last().unwrap());
                        current_font = font;
                    }
                } else if ends_ws {
                    let tok = std::mem::take(&mut current_token);
                    measure_token(&tok, current_font, &mut min_w, &mut token_widths);
                } else {
                    current_font = font;
                }
            }
        }
    }

    if !current_token.is_empty() {
        measure_token(&current_token, current_font, &mut min_w, &mut token_widths);
    }

    (max_w, min_w, CellTokens { token_widths })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::test_helpers::run_ui_test;

    /// US3 (TBL-050, TBL-032): `TablePadding` is the four-sided inner padding
    /// value type. `ZERO` is the no-op identity; `sanitised()` clamps negative
    /// inputs to `0.0` (malformed-input branch of `TBL-050`); `horizontal()`
    /// and `vertical()` are the additive sums consumed by width/height
    /// accounting; `Default::default()` equals `ZERO`.
    #[test]
    fn table_padding_zero_default_and_accessors() {
        // ZERO has all four components equal to 0.0.
        assert_eq!(TablePadding::ZERO.top, 0.0);
        assert_eq!(TablePadding::ZERO.bottom, 0.0);
        assert_eq!(TablePadding::ZERO.left, 0.0);
        assert_eq!(TablePadding::ZERO.right, 0.0);
        // Default::default() == ZERO.
        assert_eq!(TablePadding::default(), TablePadding::ZERO);
    }

    #[test]
    fn table_padding_horizontal_and_vertical_sums() {
        let p = TablePadding {
            top: 3.0,
            bottom: 5.0,
            left: 8.0,
            right: 8.0,
        };
        assert_eq!(p.horizontal(), 16.0, "horizontal = left + right");
        assert_eq!(p.vertical(), 8.0, "vertical = top + bottom");
    }

    #[test]
    fn table_padding_sanitised_clamps_negative_components_to_zero() {
        let bad = TablePadding {
            top: -2.0,
            bottom: 4.0,
            left: -1.0,
            right: 0.0,
        };
        let cleaned = bad.sanitised();
        assert_eq!(
            cleaned,
            TablePadding {
                top: 0.0,
                bottom: 4.0,
                left: 0.0,
                right: 0.0,
            },
            "sanitised() must clamp negatives to 0.0 (TBL-050 malformed-input branch)"
        );
    }

    /// US3 (TBL-032 three-level override chain): `resolve_padding` picks the
    /// most-specific non-`None` layer.
    #[test]
    fn resolve_padding_per_cell_wins_over_column_and_global() {
        let global = TablePadding {
            top: 1.0,
            bottom: 1.0,
            left: 1.0,
            right: 1.0,
        };
        let per_column = TablePadding {
            top: 2.0,
            bottom: 2.0,
            left: 2.0,
            right: 2.0,
        };
        let per_cell = TablePadding {
            top: 3.0,
            bottom: 3.0,
            left: 3.0,
            right: 3.0,
        };
        assert_eq!(
            resolve_padding(global, Some(&per_column), Some(&per_cell)),
            per_cell
        );
    }

    #[test]
    fn resolve_padding_per_column_wins_when_no_per_cell() {
        let global = TablePadding {
            top: 1.0,
            bottom: 1.0,
            left: 1.0,
            right: 1.0,
        };
        let per_column = TablePadding {
            top: 2.0,
            bottom: 2.0,
            left: 2.0,
            right: 2.0,
        };
        assert_eq!(resolve_padding(global, Some(&per_column), None), per_column);
    }

    #[test]
    fn resolve_padding_falls_back_to_global_when_overrides_absent() {
        let global = TablePadding {
            top: 1.0,
            bottom: 1.0,
            left: 1.0,
            right: 1.0,
        };
        assert_eq!(resolve_padding(global, None, None), global);
    }

    #[test]
    fn test_measure_cell_fragmented_tokens() {
        let ctx = egui::Context::default();
        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let cell_fragmented = vec![
                    InlineElem::Text("super".to_string(), crate::ui::render::TextStyle { bold: true, ..Default::default() }),
                    InlineElem::Text("califragilistic".to_string(), crate::ui::render::TextStyle::default()),
                ];
                let cell_single = vec![
                    InlineElem::Text("supercalifragilistic".to_string(), crate::ui::render::TextStyle::default()),
                ];
                let body_font = egui::FontId::proportional(14.0);
                let mono_font = egui::FontId::monospace(14.0);
                let color = egui::Color32::WHITE;

                let (_, min_frag, _) = measure_cell(&cell_fragmented, ui, &body_font, &mono_font, color);
                let (_, min_sing, _) = measure_cell(&cell_single, ui, &body_font, &mono_font, color);

                assert!(
                    (min_frag - min_sing).abs() < 1.0,
                    "fragmented token min_content {min_frag} should match single token min_content {min_sing}"
                );
            });
        });
    }

    #[test]
    fn test_table_layout_cached_memoizes_and_invalidates_correctly() {
        let ctx = egui::Context::default();
        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let table_id = egui::Id::new("test_cache_table");
                let cells = vec![vec![vec![InlineElem::Text(
                    "Hello world this is a very long string that will wrap".to_string(),
                    Default::default(),
                )]]];

                let l1 = table_layout_cached(
                    table_id,
                    &cells,
                    TablePadding::ZERO,
                    250.0,
                    DeficitStrategy::ProportionalToSlack,
                    ui,
                );

                let l2 = table_layout_cached(
                    table_id,
                    &cells,
                    TablePadding::ZERO,
                    250.0,
                    DeficitStrategy::ProportionalToSlack,
                    ui,
                );
                assert_eq!(l1, l2);

                let l3 = table_layout_cached(
                    table_id,
                    &cells,
                    TablePadding::ZERO,
                    200.0,
                    DeficitStrategy::ProportionalToSlack,
                    ui,
                );
                assert_ne!(l1.total_width, l3.total_width);
            });
        });
    }

    #[test]
    fn test_table_layout_cached_invalidates_on_font_change() {
        let ctx = egui::Context::default();
        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let table_id = egui::Id::new("test_cache_font");
                let cells = vec![vec![vec![InlineElem::Text(
                    "Hello".to_string(),
                    Default::default(),
                )]]];

                let l1 = table_layout_cached(
                    table_id,
                    &cells,
                    TablePadding::ZERO,
                    250.0,
                    DeficitStrategy::ProportionalToSlack,
                    ui,
                );

                // Mutate font styles to simulate zoom
                if let Some(font_id) = ui.style_mut().text_styles.get_mut(&egui::TextStyle::Body) {
                    font_id.size += 10.0;
                }

                let l2 = table_layout_cached(
                    table_id,
                    &cells,
                    TablePadding::ZERO,
                    250.0,
                    DeficitStrategy::ProportionalToSlack,
                    ui,
                );

                assert_ne!(
                    l1.total_width, l2.total_width,
                    "Cache should invalidate and recalculate layout upon font size change"
                );
            });
        });
    }

    #[test]
    fn test_measure_cached_memoizes_and_invalidates_on_cell_change() {
        let ctx = egui::Context::default();
        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let table_id = egui::Id::new("test_measure_table");
                let cells1 = vec![vec![vec![InlineElem::Text(
                    "Hello".to_string(),
                    Default::default(),
                )]]];
                let cells2 = vec![vec![vec![InlineElem::Text(
                    "Hello World Long Text".to_string(),
                    Default::default(),
                )]]];

                let (max1, min1, _bp1) = measure_cached(table_id, &cells1, TablePadding::ZERO, ui);
                let (max1_again, min1_again, _bp1_again) =
                    measure_cached(table_id, &cells1, TablePadding::ZERO, ui);
                assert_eq!(max1, max1_again);
                assert_eq!(min1, min1_again);

                // Changing cell content invalidates measure cache
                let (max2, min2, _bp2) = measure_cached(table_id, &cells2, TablePadding::ZERO, ui);
                assert_ne!(max1, max2);
                assert_ne!(min1, min2);
            });
        });
    }
}
