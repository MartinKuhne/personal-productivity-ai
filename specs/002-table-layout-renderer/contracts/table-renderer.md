# Contract: Table Layout and Renderer API

> Phase 1 output of `/speckit-plan`. Describes the *public surface* of the refactored Table Layout Engine and Renderer. Format: Rust module/type signatures with doc-level contracts, appropriate for a Rust library crate consumed by `ui::render`. The contract is binding for `tasks.md`; subsequent `/speckit-implement` work may refine internal helpers without changing these signatures.

The contract is divided into the **pure core** (moving to `markdown/table_width/`, no `egui`) and the **egui adapter** (stays in `ui/table_width/`, plus the rendering entry point in `ui/render.rs`).

## Part A — Pure core (`crate::markdown::table_width`)

After the placement refactor (Decision 1 in [research.md](../research.md)), the pure module re-exports the following public symbols. None of them import `egui`, `pulldown_cmark`, or `InlineElem`.

```rust
//! Fair Table Width Algorithm (FTWA) — pure column-width solver.
//! No egui, no Markdown types; consumes `&[f32]` measurements.

/// A breakpoint in a column's wrap-cost curve. Unchanged from the existing
/// implementation in `ui::table_width` (moved verbatim).
#[derive(Clone, Debug, PartialEq)]
pub struct Breakpoint {
    pub width: f32,
    pub extra_lines: i32,
}

/// Strategy for distributing the deficit across the wrap set (B2). Unchanged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DeficitStrategy {
    ProportionalToSlack,
    BreakpointWaterFill,
}

impl DeficitStrategy {
    /// Parse from the config string. Unknown values fall back to ProportionalToSlack.
    pub fn from_config(s: &str) -> Self;
}

/// Output of FTWA. Unchanged.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnWidths {
    pub widths: Vec<f32>,
    pub needs_horizontal_scroll: bool,
}

/// Pure FTWA core. Behaviour, panic conditions, and the three regimes
/// (surplus / deficit / fallback) are identical to the existing `ftwa()`
/// in `ui::table_width/mod.rs:152` — moved unchanged.
///
/// Contract invariants (all enforced by panics — see existing `ftwa` doc):
///   * `max_content.len() == min_content.len() == breakpoints.len()`
///   * every element of `max_content`, `min_content`, `available` is finite
///   * `available >= 0.0` and every element of `max_content`, `min_content` is `>= 0.0`
///   * `max_content[j] >= min_content[j]` for every column
///
/// Returns `widths.len() == max_content.len()`; empty input → empty output,
/// `needs_horizontal_scroll == false`.
pub fn ftwa(
    max_content: &[f32],
    min_content: &[f32],
    breakpoints: &[Vec<Breakpoint>],
    available: f32,
    strategy: DeficitStrategy,
) -> ColumnWidths;

/// Compute per-column breakpoints by merging cell-level breakpoints.
/// Pure: takes only token-width vectors, no font/egui types.
///
/// This function is exposed so the egui adapter can call it from `measure()`
/// with the token-width vectors it computes via `egui::font`. It is also
/// independently unit-testable without an `egui::Ui`.
pub fn compute_column_breakpoints(
    cell_tokens: &[CellTokens],
    space_width: f32,
) -> Vec<Breakpoint>;

/// Token-width vector for a single cell. Pure data; produced by the egui
/// adapter and consumed by `compute_column_breakpoints`. The token widths
/// are the widths of whitespace-separated runs measured with the cell's
/// own font (body vs. monospace for code spans), in on-screen order.
#[derive(Clone)]
pub struct CellTokens {
    pub token_widths: Vec<f32>,
}
```

**Behaviour contract** (verbatim from the existing `ftwa()` doc, incorporated by reference): the three regimes are surplus (`available >= Σ max_content`, columns pinned at `max_content`), deficit (`Σ min_content <= available < Σ max_content`, minimum-cardinality wrap set chosen, columns shrunk via `DeficitStrategy`, never below `min_content`, float drift dumped into the deepest-slack wrap column), and fallback (`available < Σ min_content`, returns `min_content` and sets `needs_horizontal_scroll = true`). Satisfies `TBL-010`–`TBL-013`, `TBL-022` flag.

## Part B — egui adapter (`crate::ui::table_width`)

After the refactor, `ui::table_width` shrinks to the egui-bridging surface. It re-exports the pure types so existing callers in `ui/render.rs` can keep writing `crate::ui::table_width::ftwa_cached`:

```rust
//! egui-bridging adapter for the Table Layout Engine.
//! Re-exports the pure core and provides `measure`/`solve` helpers
//! that depend on `egui::Ui` for font shaping.

pub use crate::markdown::table_width::{
    Breakpoint, CellTokens, ColumnWidths, DeficitStrategy, compute_column_breakpoints, ftwa,
};

#[derive(Clone, Debug)]
pub struct TableMeasureCache { /* unchanged */ }

#[derive(Clone, Debug)]
pub struct TableDecisionCache { /* unchanged */ }

/// Measure per-column max/min content widths and breakpoints via egui
/// font shaping. Behaviour identical to the existing `measure` in
/// `ui/table_width/mod.rs:517`, with one change: **the resolved
/// `TablePadding` is added to every cell's `max_content` and `min_content`
/// (left + right) before returning** (honours `TBL-033` width accounting).
/// The padding is not added to `breakpoints` because breakpoints are
/// extra-line counts (padding does not change where a token wraps, only
/// the column's outer width budget).
pub fn measure(
    cells: &[Vec<Vec<crate::markdown::InlineElem>]],
    padding: &TablePadding,
    ui: &egui::Ui,
) -> (Vec<f32>, Vec<f32>, Vec<Vec<Breakpoint>>);

/// Cached variant — unchanged signature apart from the new `padding`
/// parameter (whose hash is folded into the cache key so padding changes
/// invalidate the cache, satisfying `TBL-044`).
pub fn measure_cached(
    table_id: egui::Id,
    cells: &[Vec<Vec<crate::markdown::InlineElem>]],
    padding: &TablePadding,
    ui: &egui::Ui,
) -> (Vec<f32>, Vec<f32>, Vec<Vec<Breakpoint>>);

/// Cached FTWA solver — unchanged apart from re-routing through
/// `crate::markdown::table_width::ftwa`.
pub fn ftwa_cached(
    table_id: egui::Id,
    max_content: &[f32],
    min_content: &[f32],
    breakpoints: &[Vec<Breakpoint>],
    available: f32,
    strategy: DeficitStrategy,
    ui: &egui::Ui,
) -> ColumnWidths;
```

## Part C — Padding and render config (`crate::ui::table_width` or `crate::ui::render`)

New value types referenced by the contracts above. The choice of owning module is an implementation detail for tasks.md; the signatures are fixed.

```rust
/// Four-sided inner cell padding, all values non-negative logical pixels.
/// Resolution: per-cell > per-column > global. Used by `measure` (to honour
/// `TBL-033`) and by `render_table_cell` (to honour `TBL-032`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TablePadding {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

impl TablePadding {
    /// Zero padding (used by the existing code path before padding config
    /// is wired in; lets `render_table` default to current behaviour).
    pub const ZERO: Self;

    /// Denormalise: clamp negative components to zero (per `TBL-50`
    /// "normalize gracefully" for malformed negative padding).
    pub fn sanitised(self) -> Self;

    /// Horizontal sum `left + right` — added to column `min_content`/`max_content`.
    pub fn horizontal(self) -> f32;

    /// Vertical sum `top + bottom` — added to row height calculations.
    pub fn vertical(self) -> f32;
}

/// App-wide table rendering configuration. Owned by `FastMdApp`.
#[derive(Clone, Debug)]
pub struct TableRenderConfig {
    pub global_padding: TablePadding,
    pub clip_overflow: bool,
}

impl Default for TableRenderConfig {
    /// Defaults: small uniform padding (4 px all sides — concrete value
    /// decided in tasks.md) and `clip_overflow = false` (matches today's
    /// behaviour and `TBL-043`'s "unless explicitly configured" default).
    fn default() -> Self;
}

/// Resolve the effective padding for one cell at `(row, col)`.
/// Implementation searches per-cell override → per-column override →
/// global default. Each `Option<TablePadding>` is documented in
/// [data-model.md](../data-model.md) as `None`-defer-upper.
pub fn resolve_padding(
    global: TablePadding,
    per_column: Option<&TablePadding>,
    per_cell: Option<&TablePadding>,
) -> TablePadding;
```

## Part D — Renderer surface (`crate::ui::render`)

The existing public call sites for tables:

```rust
/// Existing external entry point used by `markdown::render` to dispatch
/// a parsed `RenderEvent::Table` into the egui canvas. Signature unchanged
/// from the existing `render_table` in `src/desktop/src/ui/render.rs:430`,
/// except it now takes a `&TableRenderConfig` it reads padding + clip flag
/// from. The function remains private to `ui::render` (called from the
/// `RenderEvent::Table` arm of `render_markdown_events`).
fn render_table(
    ui: &mut egui::Ui,
    table_cells: &[Vec<Vec<crate::markdown::InlineElem>>],
    table_ordinal: usize,
    strategy: crate::ui::table_width::DeficitStrategy,
    config: &crate::ui::table_width::TableRenderConfig,
);

/// Cell renderer. Signature gains the resolved per-cell padding and the
/// `clip_overflow` flag. Per Decision 3 in research.md it no longer paints
/// its own perimeter stroke; only intra-grid separators and the outer
/// table's perimeter are painted. Satisfies `TBL-030`, `TBL-031`, `TBL-040`,
/// `TBL-041`, `TBL-042`, `TBL-043`.
fn render_table_cell(
    ui: &mut egui::Ui,
    cell: &[crate::markdown::InlineElem],
    pinned_width: Option<f32>,
    padding: crate::ui::table_width::TablePadding,
    clip_overflow: bool,
);
```

## Contract testability (mapping to spec success criteria)

| Spec criterion | Contract element verifying it |
|---|---|
| `SC-001` surplus | `ftwa` surplus regime (exhaustive unit tests in `markdown/table_width/tests.rs`, moved unchanged) |
| `SC-002` deficit | `ftwa` deficit regime (moved unit tests) + `measure_cached` incorporates padding |
| `SC-003` fallback + scroll flag | `ftwa` return `needs_horizontal_scroll == true`; `render_table` falls back to `egui::ScrollArea::horizontal` |
| `SC-004` wrap, prefer whitespace | `measure_cell` + `compute_column_breakpoints` (existing token-model, pure core) |
| `SC-005` markdown formatting rendered | `render_table_cell` existing inline-render branches (Text/Link/Image/Html/SoftBreak) — unchanged |
| `SC-006` alignment + padding factored | `TablePadding::horizontal` used in `measure`; `TablePadding::vertical` used in `render_table_cell`; `Layout::top_down(Align::Min)` for TOP-LEFT (existing) |
| `SC-007` distinct greys + collapse | Two-strokes rule in `render_table` (Decision 3); explicit `egui_kittest` snapshot test |
| `SC-008` malformed input | `TablePadding::sanitised`; `ftwa` panics already cover NaN/infinite/measurement invariants |
| `SC-009` zero redundant re-layout | `TableMeasureCache` keyed on `cell_hash` + `font_hash`; `TableDecisionCache` keyed on `input_hash` + `avail` + `strategy` (existing, unchanged signatures) |
| `SC-010` user-readable tables | Qualitative — exercised by the full-table snapshot tests in `render.rs::tests` (existing) plus the new snapshot tests for padding + border change |