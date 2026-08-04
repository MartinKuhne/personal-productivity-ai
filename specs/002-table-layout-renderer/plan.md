# Implementation Plan: Table Layout and Renderer Subsystem

**Branch**: `002-table-layout-renderer` | **Date**: 2026-07-28 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-table-layout-renderer/spec.md`

## Summary

The feature covers the Table Layout Engine and Renderer subsystem for the `fastmd` desktop crate, satisfying `TBL-001`…`TBL-051` from `src/desktop/src/ui/SPEC.md`. **Most of the geometry path (FTWA: Fair Table Width Algorithm) is already implemented** in `src/desktop/src/ui/table_width/mod.rs` and rendered by `src/desktop/src/ui/render.rs::render_table` / `render_table_cell`. The implementation gap vs. the spec is concentrated in three areas: (1) configurable cell padding at global/per-column/per-cell levels (`TBL-032`, `TBL-033`), (2) differentiated border styling — medium-gray perimeter vs. dark-gray inter-cell with collapsed junctions (`TBL-040`, `TBL-041`, `TBL-042`), and (3) explicit clip-overflow configuration that today is implicit (`TBL-043`). A separate, secondary concern is the **architectural placement** flagged by `src/desktop/AGENTS.md`: pure layout math should live in `src/desktop/src/markdown/table_width/`, not `src/desktop/src/ui/table_width/`. The current module couples FTWA to `egui` through `measure`; the move is non-trivial because measurement needs egui font shaping. The plan resolves this by splitting FTWA into a pure-`f32` core that already exists (the `ftwa()` function takes `&[f32]`, not `egui`) and an egui-bridging `measure()` adapter that stays in `ui/`. Technical approach: refactor + additive feature work, not greenfield.

## Technical Context

**Language/Version**: Rust 1.75+ (edition 2024, per `src/desktop/Cargo.toml`).

**Primary Dependencies**: `eframe`/`egui` 0.35 (UI framework), `pulldown-cmark` 0.10 (Markdown parser, used by `markdown::parser`), `egui_kittest` 0.35 (dev-dep, snapshot/interaction tests). No new external deps expected for this feature; padding config and border styling use existing egui primitives (`egui::Margin`, `egui::Stroke`, `egui::Frame`, painter primitives).

**Storage**: N/A — the subsystem is stateless layout math + immediate-mode rendering. Persistent caches (`TableMeasureCache`, `TableDecisionCache`) live in `egui` `data` temp via `egui::Id`, not in files.

**Testing**: `cargo nextest run` (per `src/desktop/AGENTS.md §6`). Existing unit tests for `ftwa` are in `src/desktop/src/ui/table_width/mod.rs::tests`; UI snapshot/interaction tests use `egui_kittest` and are in `src/desktop/src/ui/render.rs::tests`. New tests will follow the same patterns: pure-algorithm tests beside the algorithm, `egui_kittest` snapshot tests for rendering.

**Target Platform**: Desktop (`fastmd` binary, `eframe::run_native`); Windows/Linux/macOS via eframe's `glow` backend. No mobile (per spec Assumptions).

**Project Type**: Desktop app with a `lib.rs` facade — Rust crate `fastmd`.

**Performance Goals**: No explicit latency budget required (`TBL-51` is "use available resources sensibly"). The visible success marker is "no redundant re-layout passes when content and viewport are unchanged" (`TBL-044`, already satisfied by `measure_cached`/`ftwa_cached` — see `SC-009`). Tests must verify no shaping re-runs on unchanged input.

**Constraints**: Must fit within `eframe`/`egui` immediate-mode semantics, including the `egui::Id` pass-to-pass stability rules in `src/desktop/AGENTS.md §2`. The existing `render_table` already uses a content-derived `table_ordinal` for stable ids; new padding/border config must not break this.

**Scale/Scope**: 1-50 columns × 1-1000 rows is the realistic worst case already covered by `very_large_column_count_is_well_formed` (1000 columns). No new scale pressure; the gap work is per-cell/per-frame config plumbing, not algorithmic complexity.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Constitution: `.specify/memory/constitution.md` v1.0.0 (Personal Productivity AI Constitution).

| Principle | Status | Notes |
|---|---|---|
| **I. Testability** | ✅ Pass | Plan calls for unit tests for each new gap (padding resolution, border styling differentiation, clip config) beside the algorithm; `egui_kittest` snapshot/interaction tests for rendering changes. Regression tests added for every bug fix per AGENTS.md §3. |
| **II. Security** | ✅ Pass | No new input surfaces. Padding config and border tokens come from trusted `AppConfig` (`config/`), not user-supplied Markdown. The malformed-input handling (`TBL-50`) is already asserted in FTWA. |
| **III. Modularity** | ⚠️ Note | The placement refactor (`ui/table_width` → `markdown/table_width` + `ui/table_width` adapter) aligns the subsystem with `src/desktop/AGENTS.md §5` placement guidance. This is a single, scoped move — not a sweeping refactor. The pure `ftwa()` core already takes `&[f32]`; the move swaps only module paths and an import line. |
| **IV. Open Source Leverage** | ✅ Pass | No new libraries proposed; everything uses existing `egui`/`pulldown-cmark` deps. Padding/style are egui `Frame`/`Stroke` primitives, not bespoke. |
| **V. SDLC Best Practices** | ✅ Pass | Test-driven (per AGENTS.md §2/§10), warnings fixed before done (per AGENTS.md §4 / desktop §6 quality gate), specs traceable to `TBL-xxx` requirements (per desktop §4). |

**Gate verdict**: PASS. No Complexity Tracking table needed — Principle III note is a justification *for* modularity, not a violation; the existing placement in `ui/` is the violation and the plan fixes it.

## Project Structure

### Documentation (this feature)

```text
specs/002-table-layout-renderer/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── table-renderer.md   # Phase 1 output (Rust trait/module API contract)
└── tasks.md             # Phase 2 output (/speckit-tasks — not created by /speckit-plan)
```

### Source Code (repository root)

```text
src/desktop/src/
├── markdown/
│   ├── table_width/           # NEW home for the pure FTWA core (split from ui/table_width/)
│   │   ├── mod.rs             # `ftwa()`, `Breakpoint`, `ColumnWidths`, `DeficitStrategy` — pure &[f32] math
│   │   └── tests.rs           # existing ftwa unit tests moved verbatim
│   └── ...                    # ast.rs, document.rs, parser.rs, toc.rs unchanged
├── ui/
│   ├── table_width/           # SHRUNK to the egui-bridging adapter only
│   │   └── mod.rs             # `measure()`, `measure_cached()`, `ftwa_cached()`, `TableMeasureCache`, `TableDecisionCache` — depend on `markdown::table_width::ftwa`
│   ├── render.rs              # `render_table` / `render_table_cell` updated: padding resolution + border styling
│   ├── app.rs                 # `FastMdApp` gains a `TableRenderConfig` slot on the app (per desktop AGENTS.md: cross-cutting state on FastMdApp, split into a dedicated manager when it grows)
│   ├── strings.rs             # any new user-facing config strings (per AGENTS.md §2 string isolation) — likely empty for this feature; no user-facing toggle planned in scope
│   └── panels/                # unchanged; tables render inside center.rs which already calls markdown::render
└── config/
    └── ...                    # if padding defaults are made user-configurable, add a `TableRenderConfig` here (TBD by tasks.md); scope assumption: v1 hardcodes defaults in `ui/table_width/` and `config/` is untouched
```

**Structure Decision**: Single-project layout (option 1 in template), bounded to the `fastmd` crate's existing subsystem boundaries. The only structural change is the **subsystem move** of the pure FTWA algorithm from `ui/table_width/` to `markdown/table_width/` per `src/desktop/AGENTS.md §5` ("Pure layout math with no egui dependency lives in `markdown/table_width/`, not `ui/`"). The egui-dependent `measure()`/`measure_cached()`/`ftwa_cached()` adapter stays in `ui/table_width/` because it requires `egui::Ui` for font shaping. The split cleans up the existing dependency inversion: today `markdown::ast::InlineElem` is imported into `ui/table_width/` via `crate::ui::render::InlineElem`; after the split, the pure core has no Markdown or egui imports.

## Complexity Tracking

> None. No Constitution Check violations require justification.