---

description: "Task list for the Table Layout and Renderer Subsystem feature"
---

# Tasks: Table Layout and Renderer Subsystem

**Input**: Design documents from `/specs/002-table-layout-renderer/`

**Prerequisites**: [plan.md](plan.md) (tech stack, libraries, structure), [spec.md](spec.md) (user stories P1-P3), [research.md](research.md) (six decisions), [data-model.md](data-model.md) (entities), [contracts/table-renderer.md](contracts/table-renderer.md) (Rust API surface), [quickstart.md](quickstart.md) (7 validation scenarios).

**Tests**: INCLUDED — `src/desktop/AGENTS.md §2` mandates test-driven changes and §6 makes the quality gate (`cargo nextest run`, etc.) a per-task requirement. Tests must be written and FAIL before implementation work lands.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story. The feature is largely **already implemented** (the FTWA solver, `render_table`, and `render_table_cell` exist in `src/desktop/src/ui/table_width/mod.rs` and `src/desktop/src/ui/render.rs`); the actual gap work is (a) a placement refactor moving the pure FTWA core to `src/desktop/src/markdown/table_width/` per `src/desktop/AGENTS.md §5`, and (b) three additive capability gaps (configurable padding, two-greys border styling with collapsing, clip-overflow flag).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1=fit+wrap, US2=markdown content, US3=alignment+padding+borders, US4=overflow+clip)
- Include exact file paths in descriptions

## Path Conventions

- **Single crate**: all paths relative to `src/desktop/` (the `fastmd` crate root). Examples: `src/ui/render.rs`, `src/markdown/table_width/mod.rs` mean `src/desktop/src/ui/render.rs` and `src/desktop/src/markdown/table_width/mod.rs` respectively.
- **Quality gate** (run from `src/desktop/`): `cargo check`, `cargo nextest run`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps --quiet` — must be clean after every task per `src/desktop/AGENTS.md §6`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Skeleton for new modules/types before any refactor or feature work.

- [X] T001 Create placeholder `src/markdown/table_width/mod.rs` with a `//!` module doc comment "Fair Table Width Algorithm (FTWA) — pure column-width solver. No egui, no Markdown types; consumes `&[f32]` measurements." and no items yet.
- [X] T002 [P] Scaffold `TablePadding` and `TableRenderConfig` type stubs (fields + `Default` returning `TablePadding::ZERO` and `clip_overflow = false`, no behaviour) in `src/ui/table_width/mod.rs`. Add `TablePadding::ZERO` const, `sanitised`, `horizontal`, `vertical` stub methods returning `self`/zero values (filled in for real by T016).

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Placement refactor moving the pure FTWA core from `src/ui/table_width/` to `src/markdown/table_width/` per `src/desktop/AGENTS.md §5` ("Pure layout math with no egui dependency lives in `markdown/table_width/`, not `ui/`"). Decision 1 in [research.md](research.md). MUST complete before any user story work — stories 3 and 4 modify the moved module.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete and the quality gate is green.

- [X] T003 Move the pure FTWA items from `src/ui/table_width/mod.rs` to `src/markdown/table_width/mod.rs`: `ftwa`, `Breakpoint`, `ColumnWidths`, `DeficitStrategy` (with `from_config`), `ShrinkStep`, the helpers `b2_proportional_to_slack`, `b2_breakpoint_water_fill`, `next_breakpoint_below`, `compute_column_breakpoints`, `cell_breakpoints`, `greedy_line_count`, `extra_lines_at_width`, `CellTokens`, and the existing `#[cfg(test)] mod tests` block verbatim (no test source changes). Promote `compute_column_breakpoints` and `CellTokens` to `pub` so the egui adapter (`src/ui/table_width/`) can call `compute_column_breakpoints` after producing `CellTokens` from `measure_cell`. Drop the `use crate::ui::render::InlineElem;` import from the moved module — the moved module has no `InlineElem` references (only `&[f32]`).
- [X] T004 Update `src/markdown/mod.rs` to register `pub mod table_width;` and re-export the public pure API at the `markdown::` facade level: `pub use table_width::{Breakpoint, CellTokens, ColumnWidths, DeficitStrategy, compute_column_breakpoints, ftwa};`. Doc comment bullet list updated to mention the new submodule.
- [X] T005 Trim `src/ui/table_width/mod.rs` to the egui-bridging surface only: keep `TableMeasureCache`, `TableDecisionCache`, `measure`, `measure_cached`, `ftwa_cached`, and `measure_cell`; re-export the pure items via `pub use crate::markdown::table_width::{Breakpoint, CellTokens, ColumnWidths, DeficitStrategy, compute_column_breakpoints, ftwa};` so existing `crate::ui::table_width::ftwa_cached` call sites in `src/ui/render.rs::render_table` keep resolving unchanged. Verify all existing test tags (`ftwa`, `table_*`) still pass.
- [X] T006 Verify the placement refactor via the full quality gate from `src/desktop/` (`cargo check`, `cargo nextest run`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps --quiet`) — all must pass clean with zero warnings. If any fail, fix the moved/re-exported surface before any user story work begins.

**Checkpoint**: Foundation ready — pure FTWA core in `markdown::table_width` (no egui), egui adapter in `ui::table_width` (re-exports the pure core). User story implementation can now begin.

---

## Phase 3: User Story 1 — View a Markdown table rendered to fit the available width (Priority: P1) 🎯 MVP

**Goal**: Accept a parsed Markdown table and render it so columns fit the parent-supplied available width (`W_max`): columns at preferred widths when they fit; shrunk down to (never below) minimum widths when they don't; cell content wraps to subsequent lines rather than being clipped; lines prefer to break at whitespace. Satisfies `TBL-001`, `TBL-010`–`TBL-013`, `TBL-020`, `TBL-021`.

**Independent Test**: Render one Markdown table per regime (surplus, deficit, fallback) via `render_table_with_paint_output` (the existing harness at `src/ui/render.rs:2222`). Assert each regime's painted contract: surplus → columns at preferred widths, no truncation; deficit → columns shrunk but never below minimum; fallback → no column below minimum and horizontal scroll enabled. Whitespace preference (`TBL-021`): greedy line packing never splits a token.

### Tests for User Story 1

> **NOTE**: Written first, must FAIL before implementation. Most of US1 is already implemented by the moved FTWA core and `render_table`; the new tests guard against regressions in the moved code and add the one missing assertion for `TBL-021` whitespace preference.

- [X] T007 [P] [US1] Add test `wrap_breaks_at_whitespace_never_inside_token` to the moved `tests` module in `src/markdown/table_width/mod.rs` — build token widths `[20.0, 30.0, 25.0]` with `space_width = 10.0`, call `greedy_line_count(token_widths, space_width, 65.0)`, and assert the resulting line count is 2 with the breakpoint only between tokens (i.e. `greedy_line_count` returns `2`, which by construction only ever breaks after a whitespace-completed token — never inside one).
- [X] T008 [P] [US1] Add test `test_render_table_surplus_deficit_fallback_visible_end_to_end` in `src/ui/render.rs::tests` — render three single-row tables (one per FTWA regime) via `render_table_with_paint_output` and assert: (a) surplus → painted column widths equal the max-content widths and no cell wraps; (b) deficit → painted column widths sum exactly to `available` and at least one column wrapped (painted glyph count > single-line); (c) fallback → a horizontal `egui::ScrollArea` painter shape is present in the output (`needs_horizontal_scroll` branch of `render_table`).

### Implementation for User Story 1

- [X] T009 [US1] No new implementation code required. Audit the moved `ftwa` for accidental regressions during the Phase 2 refactor (T003–T005) and restore any helper signatures that the move broke; if everything passes T007 and T008 unchanged, this task is a no-op recorded as completed.

**Checkpoint**: User Story 1 fully functional and testable independently — surplus/deficit/fallback regimes render per spec, text wraps at whitespace. **MVP ready: stop and validate** if incremental delivery is the chosen strategy.

---

## Phase 4: User Story 2 — Render formatted (markdown) cell content, not just plain text (Priority: P2)

**Goal**: Cells whose content carries inline Markdown formatting (bold, italic, code span, strikethrough, link, image placeholder) render with that formatting applied per the Markdown spec — not as opaque plain text — while still honouring the width-fitting rule from US1. Satisfies `TBL-002`, `TBL-003`.

**Independent Test**: A 1×3 table whose three cells contain bold, italic, and monospace-code `InlineElem` instances renders with the bold/italic/monospace fonts visible in the painted galley shapes. A separate 1×2 table with link and image placeholder cells asserts the link display text and the `[Image: <url>]` placeholder string are painted in the body font.

### Tests for User Story 2

- [X] T010 [P] [US2] Add test `test_render_table_cell_markdown_bold_italic_code` in `src/ui/render.rs::tests` — build a 1×3 cell matrix with `InlineElem::Text("bold", TextStyle{bold:true, ..})`, `InlineElem::Text("italic", TextStyle{italic:true, ..})`, `InlineElem::Text("code", TextStyle{code:true, ..})` and use `render_table_with_paint_output` to inspect painted `Shape::galley` font IDs, asserting that bold, italic, and `Monospace` family font IDs are each present in the corresponding cell's painted text.
- [X] T011 [P] [US2] Add test `test_render_table_cell_link_and_image_placeholder` in `src/ui/render.rs::tests` — build a 1×2 cell matrix with `InlineElem::Link("https://example.com", "Example")` and `InlineElem::Image("pic.png")`, render via `render_table_with_paint_output`, and assert the painted glyphs include the link display string "Example" and the placeholder string "[Image: pic.png]" in the body font.

### Implementation for User Story 2

- [X] T012 [US2] No new implementation code required. Verify the existing `render_table_cell` and `measure_cell` in `src/ui/render.rs` / `src/ui/table_width/mod.rs` already handle `InlineElem::{Text, Link, Image, Html, SoftBreak}` with the correct fonts (body for text/link/html/image-placeholder, monospace for code spans); if T010 or T011 reveals a gap, fill it on the cell-render path only (no changes to the pure FTWA core).

**Checkpoint**: User Stories 1 and 2 both functional and independently testable — formatted markdown cells render with formatting visible and width-fitting still honoured.

---

## Phase 5: User Story 3 — Render tables with consistent alignment, padding, and borders (Priority: P3)

**Goal**: Every cell is horizontally LEFT and vertically TOP aligned. Inner cell padding is configurable at global, per-column, and per-cell levels and is factored into all column width and row height calculations. The table perimeter uses a medium-gray border; adjacent cells are separated by a dark-gray border; cell border intersections are collapsed so junctions render cleanly. Satisfies `TBL-030`–`TBL-033`, `TBL-040`–`TBL-042`.

**Independent Test**: A 2×2 Markdown table rendered with a non-zero global padding `[0, 0, 8, 8]` produces measured column widths exactly 16 px larger than the same table rendered with `TablePadding::ZERO`, and row heights at least `padding.vertical()` larger. Paint output contains exactly one perimeter rectangle in medium gray, inter-cell separators in dark gray, and no doubled junction strokes.

### Tests for User Story 3

- [X] T013 [P] [US3] Add unit tests for `TablePadding` in `src/ui/table_width/mod.rs::tests` covering: `TablePadding::ZERO` has all four components `0.0`; `sanitised()` clamps each negative component to `0.0` (covers `TBL-50` malformed-input branch for negative padding); `horizontal()` returns `left + right`; `vertical()` returns `top + bottom`; `Default::default()` equals `ZERO`.
- [X] T014 [P] [US3] Add unit tests for `resolve_padding` in `src/ui/table_width/mod.rs::tests` covering: per-cell override present → result equals per-cell value regardless of column/global; per-cell `None` + per-column present → result equals per-column; per-cell `None` + per-column `None` → result equals global default.
- [X] T015 [P] [US3] Add test `test_render_table_padding_factored_into_width_and_height` in `src/ui/render.rs::tests` — render the same 2×2 table twice via `render_table_with_paint_output`, once with global `TablePadding::ZERO` and once with global `TablePadding{0, 0, 8, 8}`; assert each column's measured width with padding is exactly 16 px greater than without, and each row's height with padding is at least 16 px greater than without (`TBL-033` width + height accounting).
- [X] T016 [P] [US3] Add test `test_render_table_borders_two_greys_and_collapsed_junctions` in `src/ui/render.rs::tests` — render a 2×2 table and inspect `FullOutput::shapes`; assert: (a) exactly one outer perimeter rectangle stroke with the medium-gray color (≈ `Color32::from_gray(120)`); (b) no per-cell perimeter frame strokes (i.e., the old `TABLE_CELL_STROKE` painted on every cell is gone); (c) inter-cell separator strokes are painted in the dark-gray color (≈ `Color32::from_gray(40)`); (d) total inter-cell separator stroke count equals `(cols-1) × rows + (rows-1) × cols` = `2 + 2 = 4`, confirming collapsed junctions (no doubled lines at intersections).
- [X] T017 [P] [US3] Add test `test_render_table_cell_alignment_left_top` in `src/ui/render.rs::tests` — render a 2×2 table where row 1 cell heights vary (one tall cell forces the row to grow), and assert via the painted galley shapes that the first glyph of every cell sits at x = cell_left + padding.left and y = cell_top + padding.top (LEFT + TOP alignment per `TBL-030`, `TBL-031`).

### Implementation for User Story 3

- [X] T018 [P] [US3] Implement `TablePadding::sanitised`, `TablePadding::horizontal`, `TablePadding::vertical`, the `TablePadding::ZERO` const, and `Default` for `TablePadding` (the stubs from T002) in `src/ui/table_width/mod.rs` per the signatures in [contracts/table-renderer.md](contracts/table-renderer.md) Part C. `sanitised()` clamps each negative component to `0.0`.
- [X] T019 [P] [US3] Implement `resolve_padding(global, per_column, per_cell)` in `src/ui/table_width/mod.rs` returning the most-specific non-`None` layer (per-cell → per-column → global) per the signatures in [contracts/table-renderer.md](contracts/table-renderer.md) Part C.
- [X] T020 [US3] Update `measure` and `measure_cached` signatures in `src/ui/table_width/mod.rs` to take a `&TablePadding` (the resolved per-column padding or global default) per [contracts/table-renderer.md](contracts/table-renderer.md) Part B. Fold `padding.horizontal()` into every returned `max_content` and `min_content` value so `ftwa`'s output respects `TBL-033` width accounting. `breakpoints` are unchanged (padding does not move token wrap points). Fold the padding hash (or just `left + right` bits) into the `TableMeasureCache`/`TableDecisionCache` cache keys so padding changes invalidate the caches per `TBL-044`. Update the single call site in `src/ui/render.rs::render_table` to resolve the padding per column (default `None` per-column override in v1) and pass it through.
- [X] T021 [US3] Honour `TablePadding::vertical()` in `render_table_cell` in `src/ui/render.rs` — apply top padding by `ui.add_space(padding.top)` before the cell content, reserve bottom padding by setting the cell's minimum height to `content_height + padding.vertical()`, and apply left/right padding via `egui::Frame::inner_margin(egui::Margin { left, right, top, bottom })` on the cell's inner frame. The existing `Layout::top_down(Align::Min)` already provides TOP alignment (`TBL-031`); confirm LEFT alignment is preserved by the inner Label layout (`TBL-030`).
- [X] T022 [US3] Replace the single per-cell `TABLE_CELL_STROKE` frame with a two-tier border approach in `src/ui/render.rs::render_table` and `render_table_cell`: (a) wrap the entire `egui::Grid` in an `egui::Frame::NONE.stroke(medium_gray).inner_margin(0)` providing the medium-gray outer perimeter (`TBL-040`); (b) drop the per-cell `cell_frame` stroke so cells do not paint their own perimeter — eliminate the doubled lines at every cell boundary; (c) paint inter-cell separators with dark-gray color (`TBL-041`) using `ui.painter().line_segment(...)` inside the Grid closure — vertical separators between adjacent columns and horizontal separators between adjacent rows; (d) paint each separator exactly once so junctions are collapsed (`TBL-042`). Introduce two new const strokes `TABLE_PERIMETER_STROKE` (medium gray ≈ `Color32::from_gray(120)`) and `TABLE_INTERCELL_STROKE` (dark gray ≈ `Color32::from_gray(40)`, unchanged from the existing `TABLE_CELL_STROKE` value). Delete the old `TABLE_CELL_STROKE` const.
- [X] T023 [US3] Plumb per-column and per-cell padding overrides through `render_table` in `src/ui/render.rs`: extend the `render_table` signature to accept `per_column_padding: Option<&[TablePadding]>` and `per_cell_padding: Option<&Vec<Vec<TablePadding>>>` slices (both default `None` in v1, populated only when overrides exist). Resolve each cell's padding via `resolve_padding(global, per_column, per_cell)` before building the `measure_cached`/`ftwa_cached` per-column padding input and before invoking `render_table_cell`. Update `markdown::render_markdown_events`'s `RenderEvent::Table(cells)` dispatch arm to pass the corresponding overrides and the app-level `TableRenderConfig`.

**Checkpoint**: User Story 3 functional — three-level padding, two-greys collapsed borders, LEFT/TOP alignment all visible. User Stories 1, 2, and 3 now independently testable.

---

## Phase 6: User Story 4 — Handle overflow gracefully and avoid masking content unless clipping is explicitly requested (Priority: P3)

**Goal**: When a table's total minimum width exceeds `W_max` (or a cell has a single unbreakable word longer than the column width), the System falls back to horizontal scrolling so overflow content remains reachable — it never visually truncates or obscures text unless the table has been explicitly configured (via `TableRenderConfig.clip_overflow = true`) to clip overflow. Satisfies `TBL-013`'s overflow case, `TBL-022`, `TBL-043`.

**Independent Test**: A 1×2 table where each cell contains a single 200 px-wide unbreakable token (column min-content widths exceed available width) renders with the tokens NOT clipped at the column boundary and a horizontal `egui::ScrollArea` present (default `clip_overflow = false`). The same table rendered with `clip_overflow = true` paints glyphs that do not extend beyond the cell's right interior edge.

### Tests for User Story 4

- [X] T024 [P] [US4] Add test `test_render_table_horizontal_scroll_fallback_no_clip` in `src/ui/render.rs::tests` — build a 1×2 table with two cells each containing a single long-token `InlineElem::Text` (so `min_content ≥ 200 px`) and available width `100 px`; render via `render_table_with_paint_output` with `clip_overflow = false`; assert (a) the painted text glyphs extend beyond the column boundary (NOT clipped) and (b) a horizontal `egui::ScrollArea` painter shape is present in the output (`needs_horizontal_scroll` branch of `render_table`).
- [X] T025 [P] [US4] Add test `test_render_table_clip_overflow_true_clips_overflowing_text` in `src/ui/render.rs::tests` — same table as T024 but with `clip_overflow = true`; assert the painted glyph x-extent within each cell does not exceed the cell's right interior edge (clipping active, not scrolling).

### Implementation for User Story 4

- [X] T026 [US4] Extend `TableRenderConfig` in `src/ui/table_width/mod.rs` with a `clip_overflow: bool` field (per [contracts/table-renderer.md](contracts/table-renderer.md) Part C); default `false` matches `TBL-043`'s "unless explicitly configured" branch. Update `Default::default()`.
- [X] T027 [US4] Thread `clip_overflow` from `TableRenderConfig` through `render_table` and `render_table_cell` in `src/ui/render.rs`: update `render_table_cell`'s signature to accept `clip_overflow: bool`; when `true` and a cell's content width exceeds its pinned width, wrap the inner content `egui::Ui` in `ui.clip_rect()` so glyphs beyond the cell boundary are not painted (clip takes precedence over the horizontal-scroll fallback when both would apply). When `false`, behaviour is unchanged (the existing `needs_horizontal_scroll` + `egui::ScrollArea::horizontal` fallback path applies).
- [X] T028 [US4] Thread the full `&TableRenderConfig` (carrying both `global_padding` and `clip_overflow`) through `render_table` and through the `RenderEvent::Table(cells)` dispatch arm in `markdown::render_markdown_events` at `src/ui/render.rs`; all existing test call sites in `src/ui/render.rs::tests` updated to pass `&TableRenderConfig::default()` (preserves current behaviour).

**Checkpoint**: All four user stories now independently functional. Overflow renders correctly in both the scrolling (default) and the explicit-clip branches.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation and architecture-record updates that span multiple user stories (per `src/desktop/AGENTS.md §4` spec traceability).

- [X] T029 [P] Update `doc/technical-context/ARCHITECTURE_C4.md` to record the new `markdown::table_width` subsystem boundary (the move from `ui::table_width` to `markdown::table_width` + the `ui::table_width` adapter) per `src/desktop/AGENTS.md §4` ("update ARCHITECTURE_C4.md when module boundaries change").
- [X] T030 [P] Update the `//!` doc comment in `src/markdown/mod.rs` to add `table_width` to the bullet list of submodules (the existing list mentions `ast`, `document`, `parser`, `toc`).
- [X] T031 [P] Audit `src/ui/render.rs` and `src/ui/table_width/mod.rs` for any lingering references to `TABLE_CELL_STROKE` or moved FTWA items after T022 removed the old const; delete the dead code if any remains.
- [X] T032 Run the full `quickstart.md` validation suite end-to-end from `src/desktop/` — Scenarios 1 through 7 — and confirm each passes. If any scenario fails, file follow-up tasks for `/speckit-implement` (out of scope for this tasks.md): the plan and tasks describe what *should* work; this task is the integration sanity check.
- [X] T033 Run the full quality gate on the merged tree from `src/desktop/` (`cargo check`, `cargo nextest run`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps --quiet`) and confirm all five are clean — final verification per `src/desktop/AGENTS.md §6`.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately. T001 and T002 are independent of each other ([P]).
- **Foundational (Phase 2)**: Depends on Setup (T001 for the new file, T002 for the stub types). BLOCKS all user stories.
- **User Stories (Phase 3+)**: All depend on Phase 2 completion:
  - US1 (Phase 3): No story-on-story dependency.
  - US2 (Phase 4): No story-on-story dependency — can proceed in parallel with US1.
  - US3 (Phase 5): No story-on-story dependency — can proceed in parallel with US1/US2.
  - US4 (Phase 6): Depends on US3 — US4's `clip_overflow` flag rides the `TableRenderConfig` created in US3 (T018 / T026 builds on the stub from T002 already substantially filled by US3).
- **Polish (Phase 7)**: Depends on all four user stories being complete.

### User Story Dependencies

- **US1 (P1)** MVP — start after Phase 2. No dependencies on other stories.
- **US2 (P2)** — start after Phase 2. No story dependencies; can integrate with US1 in parallel.
- **US3 (P3)** — start after Phase 2. No story dependencies; introduce `TableRenderConfig` and the padding/border work.
- **US4 (P3)** — start after US3 completes (`TableRenderConfig` must already exist with `global_padding`; US4 adds `clip_overflow`).

### Within Each User Story

- Tests (T007-T017, T024-T025) MUST be written and FAIL before implementation per `src/desktop/AGENTS.md §2` and §10.
- Type/value implementations (`TablePadding`, `TableRenderConfig`, `resolve_padding`, `Default`) before renderer changes that consume them (T018/T019 before T020; T020 before T021/T022/T023).
- Story complete before moving to the next priority.

### Parallel Opportunities

- **Phase 1**: T001 and T002 (different files / different parts of `src/ui/table_width/mod.rs`) — both `[P]`.
- **Phase 2**: T003 is sequential (the move touches one source file's contents and feeds T004 + T005); T004 and T005 may run in parallel once T003's file movement lands, since they touch different receiving files (`src/markdown/mod.rs` vs `src/ui/table_width/mod.rs`). T006 is a verification gate after T003–T005.
- **US1**: T007 (pure-core test) + T008 (render test) — different files, both `[P]`.
- **US2**: T010 + T011 — both render tests in `src/ui/render.rs::tests`; sequential within the file but independent of US1.
- **US3**: T013 + T014 (padding/resolve_padding unit tests, `src/ui/table_width/mod.rs`) parallel with T015 + T016 + T017 (render paint tests, `src/ui/render.rs::tests`) since they target different files — all `[P]`. Implementation: T018 + T019 in `src/ui/table_width/mod.rs` parallel; T020 must follow T018 (`measure_cached` consumes `TablePadding::horizontal`); T021, T022, T023 all touch `src/ui/render.rs::render_table` / `render_table_cell` — sequential within the renderer.
- **US4**: T024 + T025 parallel render tests (`[P]`); T026 (config field) → T027 (renderer threading) → T028 (dispatcher thread) sequential.
- **Polish**: T029 + T030 + T031 — three different files (`doc/technical-context/ARCHITECTURE_C4.md`, `src/markdown/mod.rs`, `src/ui/render.rs` audit) all `[P]`; T032 + T033 are final integration gates sequential after all earlier phases.

---

## Parallel Example: User Story 3

```text
# Tests — written first, all in parallel:
Task T013: "Unit test TablePadding helpers in src/ui/table_width/mod.rs::tests"
Task T014: "Unit test resolve_padding in src/ui/table_width/mod.rs::tests"
Task T015: "Render test padding factored into width/height in src/ui/render.rs::tests"
Task T016: "Render test two-greys collapsed borders in src/ui/render.rs::tests"
Task T017: "Render test LEFT+TOP alignment in src/ui/render.rs::tests"

# Implementations — once tests fail as expected:
Task T018 [P]: "Implement TablePadding methods in src/ui/table_width/mod.rs"   # parallel with T019
Task T019 [P]: "Implement resolve_padding in src/ui/table_width/mod.rs"        # parallel with T018

# Sequential renderer work (same files `src/ui/render.rs` + `src/ui/table_width/mod.rs`):
Task T020:    "Plumb padding through measure_cached/ftwa_cached"
Task T021:    "Honour TablePadding::vertical in render_table_cell"
Task T022:    "Two-tier border (medium-gray perimeter + dark-gray inter-cell + collapse)"
Task T023:    "Per-column/per-cell padding override plumbing through render_table"
```

---

## Implementation Strategy

### MVP First (User Story 1 only)

1. Complete Phase 1: Setup (T001, T002).
2. Complete Phase 2: Foundational (T003–T006) — the placement refactor.
3. Complete Phase 3: User Story 1 (T007–T009) — acceptance tests for the already-implemented fitting/wrapping behaviour; no new code expected.
4. **STOP and VALIDATE**: Run the [quickstart.md](quickstart.md) Scenarios 1, 6, 7; demo a view of a Markdown table rendered to fit.

### Incremental Delivery

1. Setup + Foundational → foundation ready (pure FTWA in `markdown::table_width`).
2. + US1 → MVP validated (fitting/wrapping end-to-end visible).
3. + US2 → formatted cells render with markdown formatting visible.
4. + US3 → padding, two-greys borders, alignment visible end-to-end.
5. + US4 → overflow and clip flag branches both verified.
6. Polish + final gate → ARCHITECTURE_C4 reflects the subsystem boundary; full quality gate green.

### Parallel Team Strategy

With three developers after Phase 2:

- **Developer A**: US2 (Phase 4) — 2 render acceptance tests, no new implementation expected.
- **Developer B**: US3 (Phase 5) — the bulk of the gap work (padding, borders, alignment + new types).
- **Developer C**: US1 (Phase 3) MVP path → then converge with A on the polish phase.

US4 (Phase 6) starts once US3 lands `TableRenderConfig`. Polish converges after all four stories complete.

---

## Notes

- `[P]` tasks = different files, no dependencies on incomplete tasks.
- `[Story]` labels map tasks to spec user stories for traceability.
- Each user story is independently completable and testable per spec acceptance scenarios.
- Tests are written FIRST and must FAIL before implementation lands (per `src/desktop/AGENTS.md §2`, §10).
- Commit after each task or logical group; do not commit secrets.
- Stop at any checkpoint to validate a story independently — incremental delivery is the intended mode.
- The `fastmd` crate's quality gate (5 commands in `src/desktop/AGENTS.md §6`) runs after every task; do not mark a task complete with warnings present.
- The single `use crate::ui::render::InlineElem` import in the moved FTWA code is removed (the pure core must not depend on `ui::`); `measure_cell` retains it because it stays in `ui/`.