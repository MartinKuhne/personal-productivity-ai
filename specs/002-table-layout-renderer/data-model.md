# Data Model: Table Layout and Renderer Subsystem

> Phase 1 output of `/speckit-plan`. Derived from [spec.md](spec.md) §"Key Entities" with full field-level and validation detail. States the *what*, not the *how* (no Rust syntax choices beyond the minimum needed for clear reference; the contracts file carries the API surface).

## Entities

### Table

The top-level input/output unit. Built from parsed markdown events (`RenderEvent::Table(Vec<Vec<Vec<InlineElem>>>)`) and rendered geometry metadata.

**Fields**:
- `rows: Vec<Row>` — ordered list of rows; never empty (an empty table is filtered upstream in `render_table`).
- `n_columns: usize` — equal to max row length; drives the FTWA vector lengths.
- `available_width: f32` (`W_max`) — supplied by the parent rendering context (`ui.available_width()` minus gutters); must be finite and non-negative (`ftwa` invariant — panics otherwise).
- `ordinal: usize` — content-derived ordinal used as a stable `egui::Id` seed; insulates persisted caches from auto-id positional drift (per desktop AGENTS.md §2 id stability rules).
- `global_padding: TablePadding` — resolved global default.
- `config: TableRenderConfig` — borrowed reference to the app-level config (global padding), so the table does not own a copy.

**Relationships**: owns many `Row`s; derives `Column`s for measurement. Belongs to a rendering pass in `render_table`.

**State transitions**: none (immutable during a render pass). Cached state lives separately in `TableMeasureCache`/`TableDecisionCache` keyed by the table's `egui::Id`.

### Row

Ordered sequence of cells whose index corresponds to a column position.

**Fields**:
- `index: usize` — 0-based row position; used for striped-row styling and for snapshot tests.
- `cells: Vec<Cell>` — cells in column order; length may be ≤ `n_columns` (ragged rows tolerated by `measure`, see Decision 1 in research.md; missing cells contribute zero width to their column).

**Relationships**: belongs to one `Table`; owns one or more `Cell`s.

**Validation**: ragged rows are accepted (per `TBL-50` and `measure`'s current behaviour). Negative row count is impossible by construction.

### Column

Columnar metadata shared by every `Cell` at a given index across rows. **Not stored directly** — derived during `measure()`.

**Fields** (the `&[f32]` inputs to `ftwa`):
- `index: usize`
- `max_content: f32` (`W_pref`, the cell with the greatest single-line width)
- `min_content: f32` (`W_min`, the longest unbreakable token's width)
- `breakpoints: Vec<Breakpoint>` — step function of `extra_lines` per `width`; sorted ascending by width.
- `padding_override: Option<TablePadding>` — per-column override over global.
- `final_width: f32` — output of `ftwa` (per-column allocated pixel width).

**Relationships**: aligned with one `Cell` per `Row`; lives within a `Table`.

**Validation / invariants** (carried over from existing `ftwa` doc):
- `max_content >= min_content` (slack ≥ 0); violation panics.
- All widths finite, non-negative; NaN/infinity panics.
- `max_content > 0.0` (post-degenerate-cell guard in `measure`).

### Cell

A single content unit at a `Row`–`Column` intersection.

**Fields**:
- `row_index: usize`
- `col_index: usize`
- `content: Vec<InlineElem>` — the previously-parsed markdown AST for this cell (`Text`, `Link`, `Image`, `Html`, `SoftBreak`). Per `TBL-002` and `TBL-003` — both plain text and inline markdown formatting are supported by the existing parser.
- `padding_override: Option<TablePadding>` — per-cell override; takes precedence over the column's and then the global.
- `tokens: Vec<f32>` — measured token widths (computed in `measure_cell`); consumed by breakpoint computation. Not persisted beyond the measure pass.

**Relationships**: belongs to one `Row` and one `Column`.

**Validation**: missing-cell positions (j ≥ `row.cells.len()`) contribute zero width to `Column[j]`'s measurement; they do not appear as `Cell` instances (handled in `measure`).

### TablePadding

Value type for one cell's four-side inner padding, all non-negative.

**Fields** (all `f32`, logical pixels, ≥ 0):
- `top: f32`
- `bottom: f32`
- `left: f32`
- `right: f32`

**Resolution rule** (per Decision 2 in research.md): per-cell override > per-column override > global default. `None` at a given level defers to the next level up.

**Validation**: any negative value is malformed input under `TBL-50`; the system normalises to zero (clamps) and returns a descriptive warning rather than panicking. Per the spec's Edge Cases ("negative padding values"), the chosen behaviour is "normalize the input gracefully" (the alternative branch of `TBL-50`).

**Effect on geometry** (per `TBL-033`):
- Each column's effective `min_content` and `max_content` increase by `left + right` (the padding adds to the horizontal content box the layout solver sees).
- Each row's height increases by `top + bottom` (added to the tallest cell's text height).

### TableRenderConfig

Small app-wide config value, stored inline on `FastMdApp` (Decision 5 in research.md).

**Fields**:
- `global_padding: TablePadding` — default padding applied to every table cell unless overridden.

**Relationships**: owned by `FastMdApp`; shared by reference into every `Table` built by `render_table`.

### Breakpoint (carried over from existing `ui/table_width/mod.rs`)

A point on a column's wrap-cost curve.

**Fields**:
- `width: f32` — column width in pixels at which the breakpoint applies.
- `extra_lines: i32` — number of *additional* wrapped lines at this width vs. the no-wrap baseline; `0` = no wrapping. Monotone non-increasing in `width`.

**Validation**: sorted ascending by `width` within a column's list (enforced by `cell_breakpoints` / `compute_column_breakpoints`).

### ColumnWidths (carried over from existing)

**Fields**:
- `widths: Vec<f32>` — per-column assigned pixel widths, in input order; length matches column count.
- `needs_horizontal_scroll: bool` — `true` when `W_max < Σ W_min`, triggering the `TBL-022` fallback.

## Caches (existing, no schema change)

### TableMeasureCache

Stored under `egui::Id::with("measure_cache")` via `ui.data().get_temp`. Keyed by:
- `cell_hash: u64` — hash of the parsed `cells` AST.
- `font_hash: u64` — hash of the body font family + size.

Holds the measured `max_w: Vec<f32>`, `min_w: Vec<f32>`, `breakpoints: Vec<Vec<Breakpoint>>`. Satisfies `TBL-044`.

### TableDecisionCache

Stored under `egui::Id::with("decision_cache")`. Keyed by:
- `input_hash: u64` — hash of `max_content` + `min_content`.
- `avail: f32` — available width at the solver pass.
- `strategy: DeficitStrategy` — current strategy enum value.

Holds `decision: ColumnWidths`. Satisfies `TBL-044`.

## File-level data shapes (status)

| Entity | Owner file after refactor | Notes |
|---|---|---|
| `Table`, `Row`, `Cell`, `Column` | `markdown/table_width/mod.rs` or `markdown/ast.rs` (TBD by tasks.md). | Many of these are thin nominal wrappers over existing `Vec<Vec<Vec<InlineElem>>>` shapes the renderer already passes around; they may not appear as concrete types if tasks.md prefers lightweight aliases. The data model names them so the contracts file can refer to them unambiguously. |
| `TablePadding`, `TableRenderConfig` | `ui/app.rs` (instance) + `ui/table_width/mod.rs` (type) or `ui/render.rs`. | Implementation-placement detail for tasks.md. |
| `Breakpoint`, `ColumnWidths`, `DeficitStrategy` | `markdown/table_width/mod.rs`. | Already exist in `ui/table_width/mod.rs`; moved verbatim. |
| `TableMeasureCache`, `TableDecisionCache` | `ui/table_width/mod.rs`. | Stays in `ui/` (egui dependency). |