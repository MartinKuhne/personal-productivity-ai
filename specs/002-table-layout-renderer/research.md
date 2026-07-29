# Research: Table Layout and Renderer Subsystem

> Phase 0 output of `/speckit-plan`. Consolidates design decisions for the three gap areas identified in [plan.md](plan.md): (1) configurable padding, (2) differentiated border styling with collapsed junctions, (3) explicit clip-overflow configuration. Also records the placement-refactor decision (FTWA core move).

## Decision 1: Pure FTWA core moves to `markdown/table_width/`

**Decision**: Move the pure `ftwa()` function, `Breakpoint`, `ColumnWidths`, `DeficitStrategy`, `ShrinkStep`, and the helper functions (`b2_proportional_to_slack`, `b2_breakpoint_water_fill`, `next_breakpoint_below`, `compute_column_breakpoints`, `cell_breakpoints`, `greedy_line_count`, `extra_lines_at_width`) — *i.e.* everything that operates on `&[f32]` and has no egui dependency — from `src/desktop/src/ui/table_width/mod.rs` to a new `src/desktop/src/markdown/table_width/mod.rs`. The egui-bridging `measure()`, `measure_cached()`, `ftwa_cached()`, `TableMeasureCache`, `TableDecisionCache`, `CellTokens`, and `measure_cell` (which require `&egui::Ui`) stay in `ui/table_width/`. The `CellTokens` struct currently has a private fieldless helper role in `compute_column_breakpoints`; since `compute_column_breakpoints` is pure it moves too, and `CellTokens` moves with it. `measure` will serialize `InlineElem` data into the pure-core's expected token `Vec<f32>` shapes so the pure core has no `InlineElem` import.

**Rationale**: `src/desktop/AGENTS.md §5` placement guidance says verbatim: *"Pure layout math with no egui dependency lives in `markdown/table_width/`, not `ui/`"* and *"Anything that knows about Markdown as a format […] goes in `markdown/`."* The FTWA solver and its breakpoint computation are pure layout math. The current placement in `ui/table_width/` is an existing violation that this feature's scope corrects opportunistically while touching the module for the gap work. Splitting cleanly along the existing pure-vs-egui boundary is low-risk: `ftwa()` already takes `&[f32]`, so it has no egui dependency today.

**Alternatives considered**:
- *Leave it in `ui/table_width/`*: avoids the move but perpetuates the documented violation; future maintainers would have to grep `ui/` for "pure" math, which the placement guidance is specifically designed to prevent.
- *Move everything including `measure()`*: rejected because `measure()` requires `egui::Ui` for `fonts_mut().layout_no_wrap()`. Per AGENTS.md, ui-dependent code belongs in `ui/`. This would be a layering inversion in the other direction.
- *Inline FTWA into `markdown::ast`*: rejected; the algorithm is non-trivial and deserves its own module for searchability and test locality.

## Decision 2: Padding model — global defaults + per-column/per-cell override

**Decision**: Introduce a small `TablePadding` value type (top, bottom, left, right, each `f32` logical pixels, non-negative) with three resolution layers:
1. **Global** default applied to every table — constant, hard-coded near the new `TableRenderConfig` in `ui/` (e.g. 4 px all sides as a reasonable default; final value is an implementation detail decided in tasks.md).
2. **Per-column** override stored on the table's `Column` metadata; takes precedence over global.
3. **Per-cell** override stored on the `Cell`; takes precedence over per-column.

The renderer resolves the effective padding for a given cell by taking the most-specific non-`None` override and falls back to global otherwise. The `measure_cached` and `ftwa_cached` functions must incorporate the resolved padding into both `W_min`/`W_pref` (so `TBL-033` width accounting is honoured) and into row-height computation (so `TBL-033` height accounting is honoured). Padding configurable as zero is explicit and respected.

**Rationale**: `TBL-032` says padding "SHOULD have inner cell padding (top, bottom, left, right) on a per-cell, per-column, or global table level" and `TBL-033` says padding "MUST be factored into all column width and row height calculations." The three-level override chain mirrors CSS specificity (element → class → id) and is the common pattern used by browser auto-table-layout, Apache POI, and `egui::Frame`'s `inner_margin`. A single global value with optional per-column and per-cell overrides is the minimum that satisfies `TBL-032`'s three explicit levels.

**Alternatives considered**:
- *A single global padding only* (`TBL-032` is SHOULD, so reducing scope is tempting): rejected — the spec explicitly enumerates three levels and success criteria `SC-006` reference "all configured padding (global, per-column, per-cell)" as a measurable outcome. Skipping two levels fails that criterion.
- *Padding stored on cells only, no global/per-column*: rejected; would force every cell to sport padding and bloat the data model. Override-with-default is more compact.
- *Use `egui::Margin` directly*: `egui::Margin` exists and is the type egui's `Frame::inner_margin` takes. The plan uses a thin `TablePadding` alias/newtype for the three-level resolution; the renderer converts to `egui::Margin` at the leaf. Re-uses egui's primitive without coupling the data model to `egui`.

## Decision 3: Border styling — perimeter vs. inter-cell, with collapsing

**Decision**: Replace the single `TABLE_CELL_STROKE` constant (`Color32::from_gray(40)`, dark gray) with two distinct strokes plus an explicit junction-collapse pass:
- **Perimeter**: medium gray stroke around the outer table rect (exposed via a wrapping `egui::Frame` with `stroke = medium_gray` and `inner_margin = 0`).
- **Inter-cell**: dark gray stroke between adjacent cells (`Color32::from_gray(40)`, the existing value, kept as-is for dark gray since 40/255 ≈ 16% is clearly "dark" in the 0-255 luminance scale).
- **Junction collapsing**: rather than drawing each cell's full frame (which paints every cell's border on all four sides and produces an obvious doubled line where two cells meet), paint each cell's border on only the **two interior-facing sides** (or one for edge cells) plus a single perimeter frame. Practical implementation: drop the per-cell `stroke` on `cell_frame` (currently `TABLE_CELL_STROKE` on every cell), put a single `egui::Frame` around the whole `Grid` for the perimeter, and draw inter-cell separators with the dark-gray stroke. Concrete medium-gray value is an implementation detail for tasks.md (the spec says "medium gray" qualitatively); a value around `Color32::from_gray(120)` is a starting point.

**Rationale**: `TBL-040` ("medium gray border around the table perimeter") and `TBL-041` ("dark-gray border between adjacent cells") explicitly call for two distinct greys. The current `TABLE_CELL_STROKE` paints the *same* dark gray on every cell, which (a) is one shade not two and (b) doubles at every junction. `TBL-042` says cell border intersections "SHOULD perform border collapsing to ensure clean junction rendering". The single-perimeter-frame + thin-separators approach is the simplest collapsing strategy and matches how `render_yaml_table` (same file, lines 518-521) already wraps inside a single `Frame::NONE.stroke(...)`: the precedent exists in the codebase.

**Alternatives considered**:
- *Keep per-cell framing, pick two greys, accept the doubles*: rejected as a `TBL-042` ("SHOULD") violation; the doubles are visually loud at low DPI.
- *Implement CSS-style `border-collapse: collapse` with painter primitives, computing intersection positions*: maximally flexible but over-engineered for this fixed grid use case. The single-perimeter-frame approach collapses for free because the outer frame's rectangle and the inner separators remain the only paint calls.
- *Custom `egui::Grid` subclass*: egui doesn't expose Grid subclassing; the painter-primitives route is what a subclass would do internally anyway.

## Decision 4: Clip-overflow configuration — opt-in flag on `TableRenderConfig`

**Decision**: Add an explicit `clip_overflow: bool` field to the new `TableRenderConfig` (default `false`). When `true`, `render_table_cell` wraps the cell content in an `egui::ScrollArea::vertical` *or* sets `Label::wrap_mode(egui::TextWrapMode::Extend)` + clip rect — the concrete mechanism is an implementation choice for tasks.md. When `false` (today's default behaviour), the existing flow holds: cell content wraps via `Label::wrap()` and, when a single token exceeds the column width, the FTWA fallback path enables horizontal scrolling (`TBL-022`). The spec's "MUST NOT visually truncating or obscuring text unless explicitly configured" (`TBL-043`) becomes "explicitly configured = `clip_overflow == true`".

**Rationale**: Today, clipping is never done — `render_table_cell` uses `Label::new(rt).wrap()` and the fall-back `ScrollArea::horizontal` exposes long content (see `render.rs:430-501`). `TBL-043`'s "unless explicitly configured" phrase implies an explicit configuration knob exists; without it, the spec's "unless" branch is unreachable, leaving `TBL-043` formally untestable for `false` and `true` separately. The flag is one boolean; plumbing cost is minimal.

**Alternatives considered**:
- *Treat `TBL-043` as already satisfied and add no flag*: rejected — `SC-003` and `SC-010` reference clipping being "explicitly configured"; an untestable SHOULD clause is the wrong risk posture for test-first AGENTS.md §2.
- *Add a richer enum (`Wrap`, `Truncate`, `Ellipsize`, `ScrollHorizontal`)*: rejected as scope creep; `TBL-022` already mandates the horizontal-scroll fallback, so those modes overlap. One boolean keeps the surface area at the minimum the spec asks for.

## Decision 5: Placement of the new `TableRenderConfig`

**Decision**: The `TableRenderConfig` struct (carrying global padding default + `clip_overflow`) lives on `FastMdApp` in `src/desktop/src/ui/app.rs` as a small cross-cutting value, *not* in `markdown/table_width/`. `src/desktop/AGENTS.md §2` says "All cross-cutting state lives on `FastMdApp`" and "split new UI concerns into a dedicated manager struct (cf. `DialogManager`, `SelectionManager`, `TabManager`) rather than growing `app.rs`". For v1 with one bool and one `TablePadding`, an inline struct on `FastMdApp` is fine; if it grows to include per-column/per-cell override plumbing into the markdown subsystem, future tasks should extract a `TableManager`. The pure FTWA core in `markdown/table_width/` does **not** see the config — it sees only `&[f32]` measurements after the adapter has incorporated padding (per `TBL-033`).

**Rationale**: Keeps the pure core pure (constitution Principle III) and `egui`/app state on `FastMdApp` (desktop AGENTS.md). Avoids creating a new manager for two scalars — fits the "≤ 400 lines" file-size guard before extraction.

**Alternatives considered**:
- *Put the config in `markdown/table_width/`*: rejected; couples pure algorithm to egui/app state.
- *Extract `TableManager` immediately*: premature; no behaviour to manager-ify yet.

## Decision 6: Parallelism (`TBL-51`) — explicit non-goal for v1

**Decision**: Do **not** parallelise `measure()` or `ftwa()` in this iteration. `TBL-51` is a SHOULD ("SHOULD use available resources"), and the prior performance problem (`TBL-44`, redundant re-layout) is already solved by `measure_cached`/`ftwa_cached`. Profile before parallelising; parallelism adds a `rayon` dependency (not currently in `Cargo.toml`), a layer of complexity in a function that's already O(K log|S|) worst case, and a `Send`/`Sync` audit on `egui::Ui` borrowing — the latter would force a more invasive refactor than the spec's small gap work warrants.

**Rationale**: Spec Assumptions call out "no explicit latency budget is required". The visible performance marker (`SC-009`) is "zero redundant re-layout passes", which is already satisfied. Constitution Principle VIII ("prefer existing libraries") doesn't compel adding one when no budget is set.

**Alternatives considered**:
- *Parallel `measure()` with `rayon`*: viable once a latency budget is set; defer to a follow-on task if a profile shows `measure` is hot. Not in scope for this feature's gap-fixing mission.

## Open questions resolved (none)

No `[NEEDS CLARIFICATION]` markers were emitted in Technical Context. All decisions above were made on documented reasonable defaults per the spec's *Limit clarifications* guideline. Final concrete pixel values (medium-gray shade, default padding) are implementation details for tasks.md and not spec-level decisions.