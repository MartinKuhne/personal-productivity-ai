# Quickstart: Table Layout and Renderer Subsystem

> Phase 1 output of `/speckit-plan`. Runnable validation scenarios for the feature. Each scenario cites the contract in [contracts/table-renderer.md](contracts/table-renderer.md) and the entities in [data-model.md](data-model.md) without duplicating them.

## Prerequisites

- Working tree on git branch `002-table-layout-renderer` (created by the `before_specify` hook / matching the spec directory).
- Rust toolchain capable of edition 2024 (per `src/desktop/Cargo.toml`); `cargo-nextest` installed (`cargo install cargo-nextest --locked`).
- The quality gate defined in `src/desktop/AGENTS.md §6` must pass cleanly:
  ```powershell
  PS> cargo check
  PS> cargo nextest run
  PS> cargo clippy -- -D warnings
  PS> cargo fmt --check
  PS> cargo doc --no-deps --quiet
  ```
  All commands run from `src/desktop/`.
- No external data files or network; the subsystem is self-contained.

## Validation scenarios

### Scenario 1 — Pure FTWA regime invariants (`SC-001`, `SC-002`, `SC-003`, `SC-008`)

Cells are pure `f32` vectors; no `egui` dependency required.

```powershell
PS> cargo nextest run -p fastmd ftwa
```
Run from `src/desktop/`. The test binary already contains the full FTWA test suite (surplus, deficit, fallback, NaN/infinity panics, minimum-floor, single-token-column, 1000-column stress, sum-equals-available float drift). After the Phase A refactor (per [contracts/table-renderer.md](contracts/table-renderer.md) Part A), the suite runs *unchanged* under the new module path `markdown::table_width::tests`; if the move is done correctly, every test in this filter passes without modification.

**Expected outcome**: every matched test passes; no extra `-D warnings` complaints from the moved module; `cargo doc --no-deps` succeeds with the new module's `//!` doc comment present.

### Scenario 2 — Padding is factored into width and height (`SC-006`, FR-033)

After Phase B (per [contracts/table-renderer.md](contracts/table-renderer.md) Part C), `TablePadding` exists and `measure_cached` adds `TablePadding::horizontal()` to every `min_content`/`max_content`:

```powershell
PS> cargo nextest run -p fastmd table_padding
```
New unit tests (added by `/speckit-implement`) must demonstrate:
- A table rendered with padding `[0, 0, 8, 8]` produces column `min_content` / `max_content` exactly 16 px greater than the same table rendered with `TablePadding::ZERO`.
- A cell's contributing row height increases by `padding.vertical()` (verifiable by querying the egui `Response::rect.height()` inside an `egui_kittest` harness, comparing the padded vs. unpadded case).
- `TablePadding::sanitised()` clamps negative components to zero (covers `TBL-50` for malformed negative padding).

**Expected outcome**: tests pass; `cargo clippy -- -D warnings` is clean.

### Scenario 3 — Padding overrides resolve per-cell > per-column > global (`SC-006`)

Targets `resolve_padding` (Part C of the contract):

```powershell
PS> cargo nextest run -p fastmd resolve_padding
```
Add unit tests (by `/speckit-implement`, not part of `/speckit-plan`):
- per-cell override present → result equals per-cell value, regardless of column/global.
- per-cell `None`, per-column present → result equals per-column value.
- both contents `None` → result equals global default.

**Expected outcome**: all three resolution tests pass.

### Scenario 4 — Two distinct border greys + collapsed junctions (`SC-007`, FR-040, FR-041, FR-042)

UI snapshot + paint-output test; relies on `egui_kittest`:

```powershell
PS> cargo nextest run -p fastmd table_border_styles
```
New tests:
- Two-cell-row table snapshot: the painted strokes include exactly one medium-gray rectangle along the outer perimeter and exactly one dark-gray line between the cells. Implement by capturing `FullOutput::shapes` from an `egui_kittest` harness (the existing `render_table_with_paint_output` helper in `src/desktop/src/ui/render.rs:2222` already does this) and counting `Shape::stroke` entries whose `Color32` matches each gray band.
- Snapshot PNG at `src/desktop/tests/snapshots/` for visual regression of the medium-gray perimeter + dark-gray interior on a representative table (single header row + two data rows with short text).
- A 1-cell table snapshot has the medium-gray perimeter and **no** dark-gray interior separators (verifying the collapse path doesn't paint spurious lines on edge cells).

**Expected outcome**: snapshots pass; the shape-counting assertions pass; `-D warnings` clean.

### Scenario 5 — Clip-overflow flag toggles text masking (`SC-003`, FR-043)

Targets the new `clip_overflow: bool` on `TableRenderConfig`:

```powershell
PS> cargo nextest run -p fastmd table_clip_overflow
```
Add tests:
- `clip_overflow = false` (default) — a cell containing a single continuous word longer than the allocated column width still appears fully on-screen via the horizontal `ScrollArea` fallback path (`TBL-022`).
- `clip_overflow = true` — the same cell's overflowing characters are clipped at the column boundary and the rest of the cell remains visible. Verify by checking the rendered text glyphs' x-extent does not exceed the cell's right edge.

**Expected outcome**: both branches pass; the default applies the no-clip path; the explicit `true` enables clip precisely.

### Scenario 6 — End-to-end Markdown table render including formatted cells (`SC-004`, `SC-005`, `SC-010`)

```powershell
PS> cargo nextest run -p fastmd render_table
```
The existing tests in `src/desktop/src/ui/render.rs::tests` (`test_parse_markdown_table`, `test_ftwa_measure_user_table`, `test_render_table_cells_top_aligned_within_row`, `test_render_table_cell_text_is_top_aligned_in_tall_row`, `test_render_table_cell_no_internal_vertical_gap_or_centering`, and the permutation-matrix tests referenced around `render.rs:1804-2200`) cover the end-to-end path. After the refactor, these must continue to pass **without** source modification — they exercise `render_table` and `render_table_cell` through their public `markdown::render` dispatch, and the contract changes only add trailing parameters (`&TableRenderConfig`) with a default-compatible implementation.

**Expected outcome**: every existing `render_table`-tagged test passes; the snapshot harnesses unchanged; `cargo doc --no-deps` succeeds.

### Scenario 7 — Full quality gate (`SC-009`, `TBL-044`, `TBL-50`, `TBL-51`)

Run the entire gate from `src/desktop/`:

```powershell
PS> cargo check
PS> cargo nextest run
PS> cargo clippy -- -D warnings
PS> cargo fmt --check
PS> cargo doc --no-deps --quiet
```
The full suite must pass cleanly. `SC-009` ("zero redundant re-layout passes") is verified by the existing `measure_cached`/`ftwa_cached` tests plus the cache-key-must-include-padding unit test added in Scenario 2 (verifying that changing the padding invalidates the cache so the redundant-re-layout invariant is preserved under the new keyed-with-padding configuration).

**Expected outcome**: zero warnings, zero failing tests, zero doc warnings.

## Stop conditions

If any validation scenario fails *after* the refactor work, that is a task for `/speckit-implement` to fix; `/speckit-plan` does not edit source. Wiring the new types and the refactor moves into actual code is the job of `tasks.md` (the Phase 2 output of `/speckit-tasks`).