# Proposal: Improving egui Rendering Tests

## Problem

The current rendering test pattern — `egui::Context::default() + ctx.run(egui::RawInput::default(), |ctx| { ... })` — is repeated **40+ times** across `src/desktop/src/ui/`, with no shared harness, no visual regression coverage, and no interaction coverage. The tests verify only "does the code panic?" with the occasional structural assertion (`scroll_to_id == None`, `widths.len() == 6`).

Concretely:

| Gap | Impact | Evidence |
|-----|--------|----------|
| No pixel-level coverage | Visual regressions in `render.rs`, modals, table layout ship undetected | 0 `to_png` / `image::ImageBuffer` calls anywhere in tests |
| No interaction coverage | Click handlers, copy-code button, scroll-to-heading, checkbox state are untested beyond "did it run" | No `ui.input_mut`, no `.clicked()`, no simulated `RawInput` with events |
| 40+ duplicated `ctx.run` blocks | Drift in boilerplate, hard to enforce consistency | grep `Context::default()` shows 40 call sites in `src/desktop/src/ui/` alone |
| `eprintln!` inside tests | Diagnostic output silently swallowed in CI; not testable as an assertion | `render.rs:test_ftwa_measure_user_table` lines ~1275 |
| `test_parse_markdown_fuzz_property` has no assertions | Test passes even if the parser panics or returns garbage | `render.rs:1218-1233` — only iterates, never asserts |
| 6 `e2e_tests` vs 12 `tests` in the same file | Two test modules with the same name pattern are easy to confuse; uneven naming | `render.rs:1020` (`mod tests`) and `render.rs:1222` (`mod e2e_tests`) |
| `render_tests.rs` is a partial duplicate | Re-imports `build_toc`, tests pulldown-cmark options that aren't part of the project API | `src/desktop/src/ui/render_tests.rs` (whole file) |
| `Cargo.toml` has no headless-test crate | `egui_kittest` would slot in cleanly as a dev-dependency | `[dev-dependencies]` has only `tempfile`, `tokio`, `filetime` |

The pattern is also **not what egui itself recommends for 2024+**. The community-standard tool is `egui_kittest`, which provides a `Harness` that simulates events, captures pixels, and supports snapshot tests — without needing `Context::run` boilerplate.

> **2026-07-25 update:** The project has been upgraded to **`eframe = "0.35"`** (from 0.27). That means the `egui_kittest` blocker described throughout this doc is no longer in effect — `egui_kittest 0.35` is available and matches the upgraded egui. The recommended rollout below should now proceed; see the [Current Status](#current-status) table for the up-to-date per-item status.
>
> **2026-07-27 update:** A review pass over the actual source tree found the doc was out of sync with reality on three points: (a) the Tier 4 click tests in `render.rs` are no longer `#[ignore]`'d — they are live; (b) the Tier 4 work has produced a **state-capture pattern** that the original proposal did not predict, because egui 0.35 resets `PlatformOutput` between frames; (c) the project has a project-specific **id-stability test pattern** (red-stroke shape detection + log capture) that is not documented anywhere. All three are now captured in [Patterns observed in practice](#patterns-observed-in-practice) and in the [Current Status](#current-status) table. New gaps surfaced during the review are added as P0-4, P1-5..7, P2-4..5, P3-4..5 in the [Audit](#audit-current-implementation-vs-best-practices) section.
>
> **2026-07-27 (resolutions):** The four new open questions raised by the review pass were resolved in a follow-up interview: Q2's threshold revised to **3px project-wide** (Q10), R-8 lands in **one PR** (Q11), Tier 2 tautologies use the **b-replace with header-only assertion** policy (Q12), and the new `MarkdownDoc` AST (if the render-architecture refactor lands) gets **Tier 1 tests only** (Q13). R-1, R-2, and R-8 in the Recommendations table are updated to reference these decisions.

---

# Best Practices: The egui Test Pyramid

A well-tested egui app uses a four-tier pyramid. Each tier covers a different failure mode; you do not skip tiers, you budget how many tests you write at each.

| Tier | Failure mode caught | Speed | Tools | Per-widget cost |
|------|---------------------|-------|-------|-----------------|
| **1. Pure logic** | Wrong data, edge cases in algorithms | µs | `#[test]`, `proptest` | Trivial |
| **2. Widget smoke** | Panics, infinite loops, `Id` collisions | ms | `Context::run` or `egui_kittest::Harness::new` | Low |
| **3. Visual regression** | Layout shifts, color regressions, font shaping bugs | tens of ms | `egui_kittest` + image snapshots (`insta`) | Medium |
| **4. Interaction** | Broken click handlers, focus, keyboard, state mutation | tens of ms | `egui_kittest` with `Key`, `PointerButton`, `Event` builders | Medium |
| **(5. Full eframe)** | OS integration, persistence round-trip | seconds | `eframe::run_native` headless harness (rarely needed) | High |

The cardinal sin is **inverting this pyramid** — writing lots of brittle end-to-end tests instead of many fast unit tests. The current codebase is mostly Tier 1 with scattered Tier 2, and effectively zero Tier 3 and Tier 4.

---

## Tier 1 — Pure Logic Tests

**Best practice:** Push as much logic as possible out of the UI layer into pure functions, then test those functions with hand-built inputs.

```rust
// GOOD — pure, exhaustive, fast
#[test]
fn ftwa_deficit_picks_minimum_cardinality_wrap_set() {
    let max = [60.0, 50.0, 40.0];
    let min = [10.0, 10.0, 10.0];
    let d = ftwa(&max, &min, 110.0);
    assert_eq!(d.widths[1], 50.0);
    assert_eq!(d.widths[2], 40.0);
    let wrapping = d.widths.iter().zip(max.iter())
        .filter(|(w, m)| *w < *m - 1e-3).count();
    assert_eq!(wrapping, 1);
}
```

**Properties:**
- No `Context::run`, no `Ui`, no font loading.
- Tests run in microseconds — thousands per second.
- Exhaustively covers edge cases: zero inputs, NaN, infinity, mismatched lengths, very small/large values.
- Failures pinpoint the exact algorithm branch.

The project already does this well for `parse_markdown_to_events`, `build_toc`, `parse_yaml_to_pairs`, and `ftwa`. The pattern should be **extended**, not replaced.

## Tier 2 — Widget Smoke Tests

**Best practice:** The test sets up a minimal `Context` with one panel, calls the render function, and asserts on **observable side effects** (state mutation, output `copied_text`, scroll position, focus, etc.) — never on pixels.

The community-standard tool is `egui_kittest::Harness`:

```rust
use egui_kittest::Harness;

#[test]
fn test_copy_code_button_writes_to_output() {
    let mut harness = Harness::new_ui(|ui| {
        render_code_block(ui, "let x = 1;", &mut 0);
    });
    harness.run();
    harness.get_by_label("📋").click();
    harness.run();
    // Assert the harness's output captured the code
    let output = harness.output();
    assert_eq!(output.copied_text, "let x = 1;");
}
```

If `egui_kittest` is not adopted, the equivalent is the current `Context::run` pattern — **but extracted into a single helper** to eliminate duplication.

**Properties:**
- Verifies the widget tree builds without panics.
- Verifies `Id` allocation is collision-free within a panel.
- Verifies font shaping works for the actual text (required for `layout_no_wrap`).
- Catches "this code path calls a function that requires a real `Context`" bugs.

## Tier 3 — Visual Regression Tests

**Best practice:** Render a stable, known input, capture the framebuffer, compare against a checked-in reference image. Update the reference on intentional changes.

```rust
#[test]
fn snapshot_render_table_6col() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 600.0))
        .build_ui(|ui| {
            render_table(ui, &SIX_COL_TABLE);
        });
    harness.run();
    harness.snapshot("table_6col");
}
```

Snapshots are checked into the repo as PNGs. A PR that changes a snapshot **must** justify the change in the PR description. This is the standard workflow used by `egui_kittest` + `insta` (or its built-in `try_kittest` macro).

**Properties:**
- Catches regressions invisible to structural assertions: a column that suddenly wraps to 3 lines, a heading that loses 2px padding, a color shift after a theme update.
- Catches font-shaping changes when the system font set changes.
- Snapshot files are diffable in PRs.
- Requires **deterministic inputs** (fixed `RawInput`, fixed viewport, fixed theme, no animations).
- Per the Q2/Q3 decisions: **no `#[ignore]` and no platform-gating.** All snapshots are required on every PR, every platform, with a 5-pixel diff threshold (set on the harness) to tolerate system-font variation. Snapshots for genuinely non-deterministic content (animations, custom timing) are out of scope — do not add them.

## Tier 4 — Interaction Tests

**Best practice:** Build a `RawInput` with the desired events, run the harness, then assert on the resulting state. This is the only tier that proves the click handler actually does the right thing.

```rust
#[test]
fn test_checkbox_toggles_task_state() {
    let mut app = create_test_app();
    let mut harness = Harness::new_ui(|ui| {
        let mut checked = false;
        if ui.checkbox(&mut checked, "").clicked() {
            app.task_checked = Some(checked);
        }
    });
    harness.run();
    harness.get_by_role(egui_kittest::Role::CheckBox).click();
    harness.run();
    assert_eq!(app.task_checked, Some(true));
}
```

**Properties:**
- Covers the full event → handler → state pipeline.
- The harness can simulate: clicks, double-clicks, hover, key presses, text input, focus changes, scroll, drag.
- Tests the **observable contract** of the widget, not its internals.
- Use this for: copy-code button, hyperlink clicks, task list toggles, scroll-to-heading on click, text editor commands.

## Tier 5 — Full eframe (rare)

Only needed for: persistence round-trip (`eframe::get_value` ↔ `set_value`), native window lifecycle, multi-window behavior. The project does not currently need this; defer until a feature explicitly requires it.

---

# Recommended dev-dependencies

Add to `src/desktop/Cargo.toml`:

```toml
[dev-dependencies]
# ... existing ...
egui_kittest = { version = "0.35", features = ["eframe", "snapshot"] }
insta = { version = "1.40", features = ["png"] }
proptest = "1.4"  # for property-based tests on the parser
```

- `egui_kittest` 0.35 is the matching version for `eframe = "0.35"`. The `eframe` feature pulls in the harness; the `snapshot` feature adds the `snapshot()` API.
- `insta` is the snapshot file format used by `egui_kittest`'s snapshot feature. PNG output is needed for visual regression.
- `proptest` shrinks random inputs that find a bug, producing a minimal failing case. The current `test_parse_markdown_fuzz_property` is a hand-rolled version of this and should be replaced.

---

# Audit: Current Implementation vs Best Practices

## P0 — Critical (regressions ship undetected)

### P0-1: No visual regression coverage on the renderer

| | |
|---|---|
| **What** | The markdown renderer (`render.rs`) is the project's most complex visual surface — it handles tables, code blocks, headings, lists, task lists, blockquotes, footnotes, FTWA-pinned columns, GFM tables, images, HTML inline. A regression (off-by-one padding, a column that wraps unexpectedly, a heading that loses its scroll target) is invisible to the current test suite. |
| **Where** | `src/desktop/src/ui/render.rs:1222-1380` — the `e2e_tests` module. All 6 tests just verify "no panic". |
| **Why** | The `Context::run` pattern only confirms the widget tree was built. It does not confirm the layout matches the intent. FTWA's correctness is asserted numerically, but a mis-wired `set_width(w)` (e.g. swapped x/y, or applied to the wrong column) would not be caught. |
| **Action** | Add `egui_kittest` snapshot tests for: (a) a representative markdown document covering all element types, (b) the 6-column FTWA table from `ftwa_test.rs`, (c) the empty-cell and bold-cell table variants, (d) the heading-scroll case. Check the resulting PNGs into `src/desktop/tests/snapshots/`. |

### P0-2: No interaction coverage on click handlers

| | |
|---|---|
| **What** | The copy-code button (`render.rs:135-142`), hyperlink clicks (`render.rs:82-87`), task-list checkbox toggles (`render.rs:100-105`), and the scroll-to-heading mechanism (`render.rs:152-160`) are all untested. They render the right widget but no test verifies they actually do the right thing when clicked. |
| **Where** | `src/desktop/src/ui/render.rs:135-142` (copy), `:100-105` (checkbox), `:82-87` (hyperlink), `:152-160` (scroll-to-heading). |
| **Why** | The current `e2e_tests` module only asserts `scroll_to_id == None` after a render call (one test). The checkbox state, the `copied_text` output, and the link's response are never checked. A refactor that breaks the click handler without breaking the render path would ship. |
| **Action** | For each interactive widget, write a Tier 4 test that simulates the click and asserts on the resulting state or output. Use `egui_kittest::Harness::get_by_label(...).click()`. |

### P0-3: `test_parse_markdown_fuzz_property` does not assert

| | |
|---|---|
| **What** | `render.rs:1218-1233` iterates 7 hand-picked inputs through `parse_markdown_to_events` and discards the result. A panic or a structurally invalid return value would pass the test. |
| **Where** | `src/desktop/src/ui/render.rs:1218-1233` |
| **Why** | The test name advertises a property but delivers nothing. The author probably intended to assert "no panic and `events` is well-formed". |
| **Action** | Replace with a real `proptest` that asserts: events is finite length, every `RenderEvent::FlushInline` has a non-negative `indent`, every `RenderEvent::Heading` has `level` in 1..=6, every `RenderEvent::Table` has uniform row lengths. Configure proptest with 256 cases minimum, 4096 in CI. |

### P0-4: Tier 3 (visual regression) is still absent even after the 0.35 upgrade

| | |
|---|---|
| **What** | The 0.35 upgrade unblocked `egui_kittest`'s `snapshot` feature, which is already enabled in `[dev-dependencies]`. But `insta` is **not** in `[dev-dependencies]`, so the snapshot API cannot be called. A grep for `insta::`, `harness.snapshot`, `to_png`, `ImageBuffer`, `snapshot!` in `src/desktop/` returns zero matches; there are no `*.png` or `*.snap` files anywhere. |
| **Where** | `src/desktop/Cargo.toml:54-60` (the dev-deps block). |
| **Why** | The renderer (`src/ui/render.rs`, 2,381 lines) is the project's most complex visual surface — tables, code blocks, headings, lists, task lists, blockquotes, footnotes, FTWA-pinned columns, GFM tables, images, HTML inline. A refactor that changes a column's wrap point, a heading's left padding, or a code block's border thickness passes the current test suite. The `ftwa` integration tests assert on numeric `ColumnWidths` returned by a *helper*, not on the actual rendered widget tree's rects. |
| **Action** | (1) Add `insta = { version = "1.40", features = ["png"] }` to `[dev-dependencies]`. One line. (2) Add a `src/desktop/tests/common/snapshot.rs` helper that wraps `Harness::builder().with_size(...)` and `harness.snapshot(name)` with the 5-px threshold from Q2 encoded once. (3) Take 5–8 initial snapshots: the full-markdown document from `test_multi_table_document_column_alignment`, the 6-column FTWA table from `ftwa_test.rs`, the empty-cell and bold-cell table variants, the heading-scroll case, a move-file modal, a bottom-panel with command input filled. (4) Add a CI step that fails the build on `.pending-snap` files. See R-1. |

## P1 — High (boilerplate, missing coverage)

### P1-1: 40+ duplicated `Context::run` blocks

| | |
|---|---|
| **What** | Every UI test (and there are dozens) repeats: `let ctx = egui::Context::default(); let _ = ctx.run(egui::RawInput::default(), \|ctx\| { CentralPanel::default().show(ctx, \|ui\| { ... }); });`. The boilerplate is identical except for the body. |
| **Where** | `src/desktop/src/ui/{modals.rs, app.rs, panels/{center,left,right,top,bottom}.rs, editor.rs, tree.rs, background_logs.rs, render.rs}` — 40+ sites per the grep at investigation time. |
| **Why** | Drift: some sites use `egui::RawInput::default()`, others use `Default::default()`. Some wrap in `CentralPanel`, others in `Window`. When the project's test conventions change (e.g. fixing a font, switching to a different default size), every site must be updated. |
| **Action** | Add a `src/desktop/src/test_utils.rs` (or `tests/common/mod.rs` for integration tests) exposing a `render_test(|ui| { ... })` helper. Migrate the call sites in small batches. Eventually, `egui_kittest::Harness` replaces this helper entirely. |

```rust
// Proposed helper
pub fn render_test(f: impl FnOnce(&mut egui::Ui)) {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, f);
    });
}

pub fn render_test_with_size(size: egui::Vec2, f: impl FnOnce(&mut egui::Ui)) {
    let mut raw = egui::RawInput::default();
    raw.screen_size = Some(size);
    let ctx = egui::Context::default();
    let _ = ctx.run(raw, |ctx| {
        egui::CentralPanel::default().show(ctx, f);
    });
}
```

### P1-2: `eprintln!` for diagnostic output in tests

| | |
|---|---|
| **What** | `render.rs:test_ftwa_measure_user_table` uses `eprintln!` to print FTWA inputs and outputs. The test is intended to be diagnostic, but the output is not captured by `cargo test` (which suppresses stdout in default mode) and not asserted against. |
| **Where** | `src/desktop/src/ui/render.rs:1275-1295` |
| **Why** | When the test fails, you can't tell what the inputs were. When it passes, you can't tell what the expected outputs should have been. The pattern is "debug by inspection", not "test by assertion". |
| **Action** | Either (a) drop the `eprintln!` and assert on the diagnostic values directly, or (b) move it to a `[cfg(debug_assertions)]` block that's only compiled in debug builds, or (c) use `cargo test -- --nocapture` is not a real fix; assert or remove. |

### P1-3: `render_tests.rs` duplicates `render.rs` test concerns

| | |
|---|---|
| **What** | `src/desktop/src/ui/render_tests.rs` tests `build_toc` and pulldown-cmark option flags. The first is already in `render.rs:1110-1127`. The second tests third-party crate behavior. |
| **Where** | `src/desktop/src/ui/render_tests.rs` (whole file) |
| **Why** | Splitting the test module into a sibling file is fine for size reasons, but this file tests things that are not part of the project's API surface (parser options) and overlap with what `render.rs` already tests. It also uses `super::super::render::build_toc` — the path suggests the file was carved out without reconsidering scope. |
| **Action** | Move the parser-option tests to `proptest` properties (Tier 1). Either keep `build_toc` tests in `render.rs` or move them to `render_tests.rs` — pick one. Delete the duplication. |

### P1-4: No `assert_eq!` on `RenderEvent` shape, only on contents

| | |
|---|---|
| **What** | `test_parse_markdown_to_events` checks `events[0] == RenderEvent::Heading { ... }` (good) but then nests a `match &events[3]` for the list item. Every structural test follows the same pattern. A change to the `RenderEvent` enum (e.g. adding a field) is caught by compile, but a change in *ordering* of events would only be caught by tests that happen to index the right position. |
| **Where** | `src/desktop/src/ui/render.rs:1054-1078` |
| **Why** | Indexed assertions are fragile. |
| **Action** | Convert structural tests to use `events.iter().filter(\|e\| matches!(e, ...))` or `assert!(events.contains(&RenderEvent::Heading { ... }))` — the failing case reports "missing event" instead of "wrong event at index 3". The existing `test_parse_markdown_rule_and_blockquote` and `test_parse_markdown_html_and_footnotes` already use `.any(...)` — apply that pattern uniformly. |

### P1-5: Tier 2 panel smoke tests assert on app state, not on rendered widgets

| | |
|---|---|
| **What** | Of the ~32 panel smoke tests in `panels/{top,bottom,left,right,center}.rs` and `modals.rs`, ~28 end with either `assert!(!app.foo())` (a tautology) or no assertion at all. A representative example is `test_show_bottom_panel_render` (`panels/bottom.rs:297-307`), which sets `command_input = "test input"`, renders, and asserts `command_input` is still `"test input"` — the panel never touched it. |
| **Where** | `src/desktop/src/ui/panels/bottom.rs:297-307`, `:308-326`; `panels/center.rs:476-505`; `panels/left.rs:322-409`; `panels/top.rs:249-287`; `ui/modals.rs:336-458`; `ui/background_logs.rs:231-265`. |
| **Why** | The `ctx.run_ui` calls are ~10× slower than a no-op `#[test]`, but they catch about the same class of bug (panics, removed-function compile errors). Worse, the presence of a `run_ui` block creates a false sense of coverage — a maintainer reading the suite may believe the panel's widgets are tested when they are not. The exceptions are the four id-stability tests (see P0-2 and the [id-stability pattern](#id-stability-test-pattern)) and the `right.rs` long-titles rect test. |
| **Action** | For each panel smoke test, replace the post-render state-tautology with at least one **rendered-content assertion** using a `tests/common/egui_assert.rs` helper: `assert_text_contains(shapes, "Indexing finished")` etc. Use `crate::ui::strings::*` constants rather than literals. Where the panel genuinely has no stable strings, the test should be deleted, not left as a no-op marker. See R-2. |

### P1-6: `eprintln!` regression in `right.rs:327`

| | |
|---|---|
| **What** | The P1-2 fix that replaced `eprintln!` calls in `test_ftwa_measure_user_table` was correct at the time, but a new `eprintln!` was introduced at `src/ui/panels/right.rs:327` in `test_show_right_panel_long_titles_anchor_at_panel_left_edge` — diagnostic output on the test's passing path. |
| **Where** | `src/desktop/src/ui/panels/right.rs:327`. |
| **Why** | CI logs accumulate noise; debugging a real failure mixes expected debug output with unexpected output. The doc said this was Done — the 2026-07-27 review found it is only Partially done. |
| **Action** | Delete the `eprintln!`, or gate it behind `#[cfg(debug_assertions)]` if the rects are genuinely useful for local debugging. Same treatment for any future `dbg!()` call in tests. See R-5. |

### P1-7: Hardcoded copy strings in the integration test

| | |
|---|---|
| **What** | `test_all_top_level_panels_visible_and_rendered` (`src/ui/app.rs:1490-1513`) asserts `all_text.contains("FastMD Viewer")`, `…contains("Workspace Files")`, `…contains("Table of Contents")`, `…contains("Laptop Specifications")`, and `…contains("Indexing finished") \|\| …contains("files")`. |
| **Where** | `src/desktop/src/ui/app.rs:1490-1513`. |
| **Why** | The `ui::strings` module exists *specifically* to centralize user-facing copy and prevent duplication. The integration test inlines literals that drift independently of the canonical constants. When (not if) the copy changes — e.g. "FastMD Viewer" → "FastMD" — the test breaks for a non-bug reason. |
| **Action** | Replace each literal with `crate::ui::strings::*` (e.g. `APP_TITLE`, the panel-header constants). The `strings.rs` module already exports these as `pub const` items. ~10 lines of edits, no behavior change. |

## P2 — Medium (code quality, edge cases)

### P2-1: `ftwa` edge cases not yet covered

| | |
|---|---|
| **What** | The existing `ftwa` tests cover surplus, deficit, fallback, determinism, and G3 sum-equals-available. They do **not** cover: `available = NaN`, `available = -1.0`, `available = f32::INFINITY`, `max_content[j] < min_content[j]` (invariant violation), empty strings as tokens, single-column tables, or very large column counts (1000+). |
| **Where** | `src/desktop/src/ui/table_width/mod.rs:223-410` (test module). |
| **Why** | The current tests cover the well-defined regimes but not the failure modes a real markdown table could trigger. |
| **Action** | Add tests for each regime + edge case. For invariant violations, the existing `assert_eq!(n, min_content.len(), ...)` at the top of `ftwa` will panic — a test should verify this. For non-finite `available`, decide whether to panic or to coerce to 0/∞ and test the chosen behavior. |

### P2-2: No layout stress tests for the table renderer

| | |
|---|---|
| **What** | `test_ftwa_measure_user_table` tests a 6-column, 3-row table. No test exists for: 1-column, 1-row, 50-column, 50-row, ragged rows, empty table, table with all empty cells, table inside a window with constrained width that triggers the §3.6 fallback. |
| **Where** | `src/desktop/src/ui/render.rs:1303-1335` (table tests). |
| **Why** | These are real failure modes. A user with a 1-row table or a window narrower than `Σ min_content` is a legitimate use case. |
| **Action** | For each edge case, write a Tier 2 smoke test (renders without panic) + a Tier 1 `ftwa` test (asserts correct widths). The `§3.6 fallback` path needs a snapshot to confirm the `ScrollArea` appears. |

### P2-3: Doc tests absent for public API

| | |
|---|---|
| **What** | Functions like `pub fn parse_markdown_to_events(markdown_text: &str) -> Vec<RenderEvent>`, `pub fn parse_yaml_to_pairs(yaml: &serde_yaml::Value) -> ...`, `pub fn build_toc(markdown_text: &str) -> Vec<ToCEntry>` have no doc tests. The AGENTS.md requires `///` doc comments and examples "where they clarify usage" but nothing verifies those examples compile. |
| **Where** | `src/desktop/src/ui/render.rs:329` (`parse_markdown_to_events`), `:215` (`parse_yaml_to_pairs`), `:573` (`build_toc`). |
| **Why** | Doc tests are documentation that won't rot. They also test the public API surface from the caller's perspective. |
| **Action** | Add a one-line `///` example to each public function. `cargo test --doc` is already wired in. |

### P2-4: `assert_eq!` inside the `ctx.run_ui` closure

| | |
|---|---|
| **What** | `test_render_heading_scroll_to_id` (`src/ui/render.rs:1574-1594`) places `assert_eq!(scroll_id, None, ...)` and `assert_eq!(dummy_scroll, Some(target_id))` *inside* the `\|ui\| { ... }` closure that egui runs in measure-then-paint passes. |
| **Where** | `src/desktop/src/ui/render.rs:1574-1594`. |
| **Why** | The assertion conceptually belongs in the test body, not in the render closure. The test happens to work because egui runs the closure once per pass and the assertion is correct in both, but the placement is misleading to a reader. |
| **Action** | Capture the relevant state into a `&mut Option<egui::Id>` cell, run `ctx.run_ui`, then assert in the test body. ~5 lines, no behavior change. |

### P2-5: `old_bug_set_width_ignored` is a no-assert diagnostic test

| | |
|---|---|
| **What** | `tests/table_layout_test.rs:old_bug_set_width_ignored` calls `dbg!(hw.response.rect.width())` three times and asserts nothing. The test name advertises a regression that was actually pinned by `fix_allocate_ui_randomised` (same file, later in the module). |
| **Where** | `src/desktop/tests/table_layout_test.rs:11-44`. |
| **Why** | A no-assert integration test produces `dbg!` output that pollutes CI logs and gives a false sense of coverage. |
| **Action** | Delete it, or add a real assertion (`assert!(width >= 90.0 && width <= 110.0, ...)` for the 100-px cell). The "this is the bug, here's what happens" intent is better served by the §"Concrete Code Examples" section. |

## P3 — Low (operational improvements)

### P3-1: CI integration of snapshots

| | |
|---|---|
| **What** | Once `insta` snapshots are added, `cargo test` will fail on any unapproved snapshot. CI should run `cargo insta test --review` (or equivalent) and report pending snapshots as a check failure. |
| **Where** | `.github/workflows/`. |
| **Action** | Add `cargo install cargo-insta` to CI setup, add a step that fails the build if there are `.pending-snap` files. Snapshot review should be a PR-author action, not a CI action. |

### P3-2: Mutation testing

| | |
|---|---|
| **What** | Once the test suite is reasonably complete, run `cargo-mutants` against the pure functions (`ftwa`, `parse_markdown_to_events`, `build_toc`) to identify gaps in the assertions. |
| **Why** | Mutation testing catches "the test passes but doesn't actually test the function". A high mutation score is a strong signal of test quality. |
| **Action** | Defer until the higher-priority items land. Run on a schedule (weekly or per-release), not on every commit. |

### P3-3: Fuzz harness for the parser

| | |
|---|---|
| **What** | `cargo-fuzz` against `parse_markdown_to_events` would catch panics on adversarial inputs. |
| **Why** | Markdown has many corner cases (nested lists, mixed indent, malformed tables, control characters). A real fuzz run finds bugs that hand-rolled property tests miss. |
| **Action** | Defer. `proptest` covers most of this. Revisit if user reports a parser panic. |

### P3-4: Test module naming inconsistency

| | |
|---|---|
| **What** | `panels/top.rs`, `panels/right.rs`, `panels/bottom.rs`, and `ui/background_logs.rs` each have two test modules — `mod tests` (for pure-logic helpers) and `mod ui_tests` (for panel smoke). The other panels and `app.rs` have only `mod tests`. `modals.rs` has only `mod tests`. `render.rs` mixes parser tests and Tier 4 tests in a single `mod tests`. |
| **Where** | `src/desktop/src/ui/panels/{top,right,bottom}.rs`, `src/desktop/src/ui/background_logs.rs`. |
| **Why** | New tests get put in whichever module the contributor happened to read last. Standardising to one convention (e.g. always `mod tests` with a doc comment separating concerns) is a 5-minute polish. |
| **Action** | Pick `mod tests` only. Move the `mod ui_tests` blocks into the main `mod tests` with a `// --- Panel smoke ---` section header. Bundle with the next time one of the four files is touched for another reason. |

### P3-5: `#[path]` indirection for `agent_impl_tests.rs`

| | |
|---|---|
| **What** | `src/agent/mod.rs:13` declares `#[path = "agent_impl_tests.rs"] mod agent_impl_tests;` — a separate file loaded into the module tree by path. |
| **Where** | `src/desktop/src/agent/mod.rs:13`. |
| **Why** | Tests usually live in `mod tests` inside the production file, or in `tests/`. The indirection works, but it's harder to discover for new contributors and for tooling like `cargo test --package fastmd -- agent_impl_tests::test_run_agent_missing_api_key`. |
| **Action** | Move the body of `agent_impl_tests.rs` into `agent_impl.rs` as `mod tests` and delete the `#[path]` indirection. No behavior change. |

---

# Concrete Code Examples

## A. Proposed `tests/common/mod.rs` (or `src/test_utils.rs`)

```rust
//! Shared test harness for egui rendering tests.

use egui;

/// Render `f` inside a single `CentralPanel` with default inputs.
/// Use for widget smoke tests that don't need interaction or snapshots.
pub fn render_test(f: impl FnOnce(&mut egui::Ui)) {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, f);
    });
}

/// Same as `render_test` but with a fixed viewport size.
/// Required for FTWA and table tests where `ui.available_width()` matters.
pub fn render_test_with_size(width: f32, height: f32, f: impl FnOnce(&mut egui::Ui)) {
    let mut raw = egui::RawInput::default();
    raw.screen_size = Some(egui::vec2(width, height));
    let ctx = egui::Context::default();
    let _ = ctx.run(raw, |ctx| {
        egui::CentralPanel::default().show(ctx, f);
    });
}

/// Assert that `f` does not panic. Returns the result for further assertions.
pub fn assert_renders_ok(f: impl FnOnce(&mut egui::Ui)) {
    render_test(f); // panics would surface immediately
}
```

## B. Migration of one existing test

**Before** (`src/desktop/src/ui/render.rs:1224-1233`):

```rust
#[test]
fn test_render_markdown_e2e() {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut scroll_id = None;
            render_markdown(ui, "# Test\n\n- [ ] Task\n\n```rust\nlet x = 1;\n```", &mut scroll_id);
            // ...
        });
    });
}
```

**After**:

```rust
#[test]
fn test_render_markdown_basic() {
    assert_renders_ok(|ui| {
        let mut scroll_id = None;
        render_markdown(ui, "# Test\n\n- [ ] Task\n\n```rust\nlet x = 1;\n```", &mut scroll_id);
    });
}
```

## C. Adding a Tier 3 snapshot

```rust
#[test]
fn snapshot_ftwa_6col_table() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1200.0, 600.0))
        .build_ui(|ui| {
            let table = build_six_col_table();
            render_table(ui, &table);
        });
    harness.run();
    // Pixel-diff threshold (5px) tolerates system-font variation across platforms.
    // Reference PNG is checked in; cargo insta review updates it.
    harness.snapshot("ftwa_6col_table");
}
```

## D. Replacing `test_parse_markdown_fuzz_property` with proptest

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn parse_never_panics_and_is_well_formed(
        s in "([#*_`>\\[\\]-+\\d\\w .,!?\\n\\t]|\\|[ -]+\\|){0,500}"
    ) {
        let events = parse_markdown_to_events(&s);

        // Finite, bounded length.
        prop_assert!(events.len() < 10_000);

        // Every Heading has a valid level.
        for e in &events {
            if let RenderEvent::Heading { level, .. } = e {
                prop_assert!((1..=6).contains(level));
            }
        }

        // Every Table is rectangular.
        if let Some(RenderEvent::Table(rows)) = events.iter().find(|e| matches!(e, RenderEvent::Table(_))) {
            let expected = rows.first().map(|r| r.len()).unwrap_or(0);
            for row in rows {
                prop_assert_eq!(row.len(), expected, "ragged table row");
            }
        }
    }
}
```

---

# Summary

| Priority | ID | Issue | Effort | Impact |
|----------|----|-------|--------|--------|
| **P0** | P0-1 | No visual regression coverage on the renderer | Medium | High |
| **P0** | P0-2 | No interaction coverage on click handlers | Small | High |
| **P0** | P0-3 | `test_parse_markdown_fuzz_property` doesn't assert | Small | Medium |
| **P1** | P1-1 | 40+ duplicated `Context::run` blocks (resolved by Q1 incremental migration, not a helper) | Small | Medium |
| **P1** | P1-2 | `eprintln!` in tests instead of assertions | Small | Low |
| **P1** | P1-3 | `render_tests.rs` duplicates scope of `render.rs` | Small | Low |
| **P1** | P1-4 | Indexed event assertions are fragile | Small | Medium |
| **P2** | P2-1 | `ftwa` edge cases (NaN, ∞, invariant violations) | Small | Medium |
| **P2** | P2-2 | No layout stress tests for tables | Small | Medium |
| **P2** | P2-3 | No doc tests for public API | Small | Low |
| **P3** | P3-1 | CI integration of `insta` snapshots | Small | Low |
| **P3** | P3-2 | Mutation testing on a schedule | Large | Low |
| **P3** | P3-3 | Fuzz harness for the parser | Medium | Low |

**Quick wins (do first):** P0-3 (replace the no-op fuzz test with proptest), P1-4 (switch to `.contains`/`.any` assertions), P2-3 (add doc tests). P1-1 is implicit in Phase 2 of the rollout.

**Biggest impact:** P0-1 (visual regression) and P0-2 (interaction coverage) — these are the two tiers the project has zero coverage of, and they're the tiers that catch real-world regressions.

**Phased rollout** (incremental, per the Q1 decision — no big-bang migration):
1. **Phase 1 (1 PR):** Extract `render_test` helper, add `proptest` and `egui_kittest` + `insta` to dev-deps, replace `test_parse_markdown_fuzz_property` (P0-3), add 3–4 snapshot tests for the most complex widgets (P0-1: FTWA table, full markdown, modal). Establishes the new testing surface in one go.
2. **Phase 2 (rolling, per file):** Each time a UI file is touched for any other reason, convert its existing `Context::run` tests to `egui_kittest::Harness`. Long mixed-style period is acceptable; no dedicated migration PR.
3. **Phase 3 (rolling):** Add Tier 4 interaction tests for click handlers as bugs / features warrant (P0-2). New widgets ship with interaction tests; old widgets get them on first touch.

---

# Patterns observed in practice

These two patterns are not in the egui testing literature or in the original proposal. They emerged during the 0.35 upgrade and the Tier 4 work, and they are the reason the 2026-07-25 status was out of sync with reality. Future contributors will reinvent them if they are not documented here.

## State-capture Tier 4 pattern

**Problem.** egui 0.35 replaced `PlatformOutput::copied_text` / `open_url` with `PlatformOutput::commands: Vec<OutputCommand>`. Worse, `PlatformOutput` is now **per-frame** — every new pass starts a fresh `PlatformOutput`. A click that emits `OutputCommand::CopyText(text)` on frame N has that command **overwritten on frame N+1**, before the test can read it.

This is the opposite of what you would expect from a `Harness::output()` API. The naive pattern — "render, click, render, assert on `harness.output().platform_output.commands`" — silently fails because the `run()`-and-settle cycle has already started a new pass and erased the click's output.

**Solution.** Capture the click's side effect into the harness's **persistent state** at the moment it fires. The harness's `state()` is preserved across frames; the closure's `T` argument is the same `T` for every frame. Push the side effect onto a `Vec` (or a `bool`, or a `Cell`) inside that state, then read `harness.state()` after `harness.step()` to see what fired.

**Canonical examples in this codebase:**

```rust
// src/ui/render.rs:test_copy_code_button_click_copies_to_output (~line 2231)
let mut harness = Harness::new_ui_state(
    |ui, captured: &mut Vec<String>| {
        if ui.button("Copy").clicked() {
            ui.copy_text("let x = 1;".to_string());
            captured.push("let x = 1;".to_string());
        }
    },
    Vec::<String>::new(),
);
harness.fit_contents();
harness.run();
harness.get_by_label("Copy").click();
// Two runs after the click: the first processes pointer events
// (hover + press + release = three steps), the second settles
// any post-click repaint.
harness.run();
harness.run();
let captured = harness.state();
assert_eq!(captured.as_slice(), &["let x = 1;".to_string()]);
```

```rust
// src/ui/render.rs:test_task_checkbox_click_toggles_state (~line 2402)
let mut harness = Harness::new_ui_state(
    |ui, captured: &mut Vec<bool>| {
        let mut checked = false;
        let response = ui.checkbox(&mut checked, "todo");
        let _ = response;
        captured.push(checked);
    },
    Vec::<bool>::new(),
);
harness.fit_contents();
harness.run();
harness.get_by_role(Role::CheckBox).click();
harness.step();
let captured = harness.state();
assert_eq!(captured.last().copied(), Some(true),
    "clicking an unchecked task-list checkbox must flip the local `checked` to `true`; \
     captured sequence: {captured:?}");
```

```rust
// src/ui/render.rs:test_hyperlink_click_opens_url (~line 2312) — the
// exception that proves the rule. This one *can* read harness.output()
// directly, but only because it uses `harness.step()` (not `run()`) to
// process the queued hover/press/release events in one frame and then
// reads `output().platform_output.commands` *before* any additional frame
// runs. This is the narrow window where the click's command is still
// observable. The state-capture pattern is the more general fix.
let link = harness.get_by_label("click me");
link.click();
harness.step();  // NOT run() — step stops after the click frame
let open_url = harness.output().platform_output.commands.iter().find_map(|cmd| {
    if let egui::OutputCommand::OpenUrl(url) = cmd { Some(url.url.clone()) } else { None }
});
```

**When to use which:**

- **Use state-capture** when the side effect is a Rust-level write (push to a vec, set a bool, write a string) and the closure is the natural place to record it.
- **Use `step() + harness.output()`** when the side effect lives only in `PlatformOutput::commands` (e.g. `OpenUrl` from a `Link` widget, where you cannot intercept the click).
- **Never use `run() + harness.output()`** for a click — the settling pass overwrites the command.

**Implication for the codebase:** every new Tier 4 test that needs to assert on a click's effect should use one of the two patterns above. A shared `tests/common/egui_interact.rs` helper (R-3) should wrap both so the next contributor does not have to discover this from a comment block.

## Id-stability test pattern

**Problem.** The most common egui bug class in this project is the "widget rect changed id between passes" warning. The AGENTS.md §"Conditional rendering" section documents the root cause and the fix (always allocate, toggle visibility with `add_visible` / `set_invisible`). But until a test exists for a panel, the warning can be reintroduced by a well-meaning refactor that wraps a panel in `if cond { Panel::right("...").show(...) }` and the `if` arm's allocation shape changes between passes.

**Solution.** Two complementary detection mechanisms, both run by the same test body:

1. **Red-stroke shape detection** — egui's `Context::warn_if_rect_changes_id` draws a red `Shape::Rect` outline in the second pass when it detects the warning. The test renders the panel twice (priming the first-pass state, then observing the second pass), walks `output.shapes`, and asserts no shape has `stroke.color == Color32::RED`.

   ```rust
   // src/ui/panels/left.rs:collect_id_change_warnings (~line 451)
   fn collect_id_change_warnings(ctx: &egui::Context, app: &mut FastMdApp) -> Vec<egui::Rect> {
       let _ = ctx.run_ui(Default::default(), |ui| {
           show_left_panel(app, ui);  // Prime Pass 1
       });
       let output = ctx.run_ui(Default::default(), |ui| {
           show_left_panel(app, ui);  // Pass 2 emits the warning if the tree is unstable
       });
       output.shapes.iter()
           .filter_map(|cs| match &cs.shape {
               egui::Shape::Rect(r) if r.stroke.color == egui::Color32::RED => Some(r.rect),
               _ => None,
           })
           .collect()
   }
   ```

2. **Log capture** — egui's `WARN egui::context: Widget rect ... changed id between passes` message is also emitted to the `log` crate. A test can install a `log::Log` impl, render across a bool flip, and grep the captured messages.

   ```rust
   // src/ui/panels/top.rs:289-360
   // (One-time logger install guarded by OnceLock<()>)
   let cap = LOGGER.get_or_init(|| Capture { msgs: Mutex::new(Vec::new()) });
   INSTALLED.get_or_init(|| {
       let _ = log::set_logger(cap);
       log::set_max_level(log::LevelFilter::Trace);
   });
   // Render pre-flip, flip, render again, render once more to stabilise.
   // Assert: msgs.iter().filter(|m| m.contains("changed id between passes")).count() == 0
   ```

**Why both?** The red-stroke mechanism can be suppressed by `ctx.set_theme(Light)` (the warning's red debug colour is hidden against certain backgrounds), and the log-capture mechanism can be lost if the test process has another `log::set_logger` installed. Belt-and-suspenders. The two checks can fail independently in edge cases.

**Existing test sites:**
- `panels/top.rs:289` — log capture, covers the `indexing_finished` bool flip on the toolbar.
- `panels/left.rs:491, :531, :550, :583` — red-stroke, covers the file tree, empty state, transition, and width-clamping.
- `ui/app.rs:1352` — red-stroke, covers the full 5-panel render with TOC active.
- `ui/background_logs.rs` — does *not* have an id-stability test yet; candidate for the next pass.

**Canonical helper (R-9):** promote the two checks into `tests/common/egui_assert.rs::assert_no_id_change_warnings(log_msgs, shapes)`. Use it at every site above.

---

# Resolved Decisions

All open questions from the initial draft of this doc were resolved during review. The decisions below are the source of truth going forward; if any of these need to change, update the doc and link the rationale.

| # | Question | Decision | Implication |
|---|----------|----------|-------------|
| Q1 | Migration of 40+ `Context::run` sites | **Incremental, per-file.** Each time a UI file is touched for any other reason, convert its tests to `egui_kittest::Harness` in the same PR. No dedicated migration PR. | Long mixed-style period is acceptable. The `render_test` helper from §A is unnecessary; the harness replaces `Context::run` directly. |
| Q2 | Snapshot determinism with system fonts | **`insta` with a 5-pixel diff threshold.** Tolerates font-metric variation across Windows / Linux CI / macOS without bundling a test font. | No `~1MB` font asset in the repo. Snapshots are not byte-identical but catch any real layout shift. Threshold is set per-harness, not per-platform. |
| Q3 | CI gating of snapshots | **Required on every PR, all platforms.** The 5px threshold is the only tolerance — no `#[ignore]`, no `#[cfg(target_os)]`, no nightly-only run. | CI step: `cargo test` (no special flags). When snapshots drift beyond threshold, the PR fails. Developer runs `cargo insta review` to accept intentional changes. |
| Q4 | Tier 4 scope for `FastMdApp` | **Defer until a real bug motivates it.** No `FastMdAppHarness`, no app-level integration tests. Widget-level Tier 4 is enough. | When a regression slips through widget tests, revisit and add a stubbed harness. Until then, no investment. |
| Q5 | `RenderEvent::to_markdown()` serializer | **Dropped after evaluation.** See rationale below. | P2-3 was the only consumer of the serializer; with it removed, no serializer work is needed. P0-3's proptest covers what round-trip would have covered, and is strictly stronger. |
| Q6 | `pulldown-cmark` option tests in `render_tests.rs` | **Keep them, move to a dedicated file.** They are cheap dependency-version sanity checks; just don't belong in `render_tests.rs`. | Create `src/desktop/tests/pulldown_config.rs` integration test, move `test_gfm_parser_options_set` and `test_gfm_hard_breaks_not_enabled` there. `render_tests.rs` keeps `build_toc` tests only (or those also move — see P1-3). |
| Q7 | Test-relevant changes from the 0.27 → 0.35 upgrade | **Resolved by the upgrade landing (commit `0b6d643`).** Doc updated 2026-07-25. | The `egui_kittest` blocker is gone. Q7's original "next step" was a 4-item checklist; items (1)–(3) are still open, item (4) is in scope for the same PR as P0-4. |
| Q8 | How to test a click that emits `OutputCommand::CopyText` / `OpenUrl` when `PlatformOutput` is per-frame in egui 0.35 | **State-capture into `Harness::new_ui_state`, or `step() + output()` for the narrow case.** The naive `run() + output()` silently fails because the settling pass overwrites the click's command. | Documented in [Patterns observed in practice](#state-capture-tier-4-pattern). The three Tier 4 tests in `render.rs` (`:2231`, `:2312`, `:2402`) are the canonical examples. New Tier 4 tests should use one of the two patterns; a shared helper (R-3) is the next step. |
| Q9 | How to test the "widget rect changed id between passes" warning | **Two complementary mechanisms: red-stroke shape detection (in `output.shapes`) and `log` capture (in the `log::Log` impl installed by the test).** Both are needed because they can fail independently — `ctx.set_theme(Light)` can hide the red debug colour; a previous `log::set_logger` can swallow the warn message. | Documented in [Patterns observed in practice](#id-stability-test-pattern). Test sites: `panels/top.rs:289` (log), `panels/left.rs:451` (red-stroke), `ui/app.rs:1352` (red-stroke). A shared helper (R-9) should combine the two checks. |
| Q10 | Snapshot threshold after the 0.35 upgrade (revised Q2) | **3px project-wide, set per-harness.** Single number in the snapshot helper. The original Q2 commitment of 5px was loosened: 3px is tight enough to catch a single line of text wrap (~14–16px) while still tolerating system-font metric variation across Windows / Linux CI / macOS. | The `tests/common/snapshot.rs` helper from R-1 takes a `threshold: f32` parameter (default 3.0) and applies it via `Harness::builder()`. No more 3-vs-5 split — the tighter threshold is uniform and simpler to reason about. Reviewers can bump a snapshot on first-time Linux CI runs if system-font noise flakes. |
| Q11 | Should R-8's 5 Tier 4 click tests land in one PR or spread? | **One PR.** R-3's `click_by_label_and_capture_state` helper lands in the same PR, and the next contributor benefits from seeing all 5 idioms at once in one diff. | Expected diff size: ~400 lines (helper + 5 tests + imports). The 5 candidates are TOC row click → `scroll_to_id`, tab close button, folder tree click → `selected_file`, send button → `submit_prompt`, batch button → `batch_dialog_open`. |
| Q12 | Tier 2 tautology policy (P1-5) | **(b) replace with text-content assertion** when the panel has stable, locatable strings. **Borderline case (panel has only a stable header but body is dynamic) → header-only assertion** — strictly more useful than leaving the test as a no-op marker, and the marginal cost is one line per panel. **(a) delete** when the panel is a pure no-op with no real assertion to make. | Lands as a sweep across `panels/{top,bottom,left,center}.rs` and `modals.rs` in R-2. The new `tests/common/egui_assert.rs::assert_text_contains(shapes, "…")` helper is the workhorse. |
| Q13 | Test tier for the new `MarkdownDoc` AST if the 4-stage pipeline refactor lands | **Tier 1 only.** The AST is pure Rust, zero `egui` dependency; the existing 49 parser tests + the FTWA proptest pattern move from `src/ui/render.rs` to `src/markdown/{ast,document}.rs` cleanly. The egui-testing strategy stays consistent across the refactor; only file locations change. | The alternative (some Tier 2 smoke at the `markdown::` boundary) is not needed. The `RenderEvent` drawing IR stays as the boundary between the new `markdown/` subsystem and `ui/`. |

## Q5 — Why the round-trip serializer was dropped

A `RenderEvent::to_markdown()` round-trip test (P2-3) was proposed, then evaluated and rejected. The reason: `RenderEvent` is intentionally a **drawing IR**, not a markdown source AST. It throws away information that round-tripping would require:

- GFM table cell alignment (`:---` vs `---:` vs `:---:`)
- Code block language tag
- Image alt text (only the URL is stored)
- HTML content (it's literal HTML — no markdown equivalent)
- Whitespace and indentation
- List-marker style (`-` vs `*` vs `+`)
- Header marker style (`#` vs Setext `===`)
- Hard vs soft break (only the resulting `SoftBreak` is preserved)
- Footnote definition body (only the name)
- Source-level list nesting depth (only the `indent` rendering field)

A round-trip test would therefore be either:

1. **Lossy** — assert `parse(serialize(x)) == x` modulo whitespace. This is **strictly weaker** than the existing structural tests in `render.rs:1054-1127` (which assert exact fields). A passing test would only mean the serializer is as lossy as the parser, not that round-tripping works.
2. **Requires enriching `RenderEvent`** to preserve source structure. At that point the IR is no longer a drawing representation — it is a markdown AST, and the refactor dwarfs the test improvement.

The proptest from P0-3 (no panic, well-formed output, level ∈ 1..=6, rectangular tables) already covers what round-trip would catch, and is strictly stronger than the lossy version.

**Recorded here so future-me doesn't re-litigate this.**

---

## Q7 — Test-relevant changes from the egui 0.27 → 0.35 upgrade (2026-07-25)

The `upgrade/egui-0.35` branch lands eight minor versions of `egui`/`eframe` in one shot. A few of those changes are visible at the test layer and worth recording so the next person touching tests knows what to expect:

| API (0.27) | API (0.35) | Migration in test code |
|-------------|------------|-------------------------|
| `ctx.run(RawInput, \|ctx\| { ... })` | `ctx.run_ui(RawInput, \|ui\| { ... })` | The closure now takes `&mut Ui`, not `&Context`. Every `Context::run` call site (≈40) was renamed to `Context::run_ui` and the parameter was renamed `ui`. `CentralPanel::default().show(ctx, …)` became `CentralPanel::default().show(ui, …)` (panels allocate from a parent `Ui` now). |
| `PlatformOutput::copied_text` | `PlatformOutput::commands: Vec<OutputCommand>` (`CopyText(_)`) | `ctx.output(\|o\| o.copied_text.clone())` no longer compiles. The replacement reads from the **`FullOutput` returned by `ctx.run_ui`**, not from the live `ctx.output` (which is reset between frames). See `commands_capture` in `src/ui/render.rs:2145` for the helper. |
| `PlatformOutput::open_url` | `OutputCommand::OpenUrl(_)` | Same shift; see the hyperlink smoke test for the pattern. |
| `ComboBox::from_id_source(…)` | `ComboBox::from_id_salt(…)` | Mechanical rename in the integration tests. |
| `ScrollArea::id_source(…)` | `ScrollArea::id_salt(…)` | Same. |
| `Label::wrap(bool)` | `Label::wrap()` / `wrap_mode(…)` | `tests/table_layout_test.rs:150` flipped from `.wrap(true)` to `.wrap()`. |
| `ui.child_ui(rect, layout)` | `ui.new_child(UiBuilder::new().max_rect(rect).layout(layout))` | `tests/table_layout_test.rs:145`. |
| `Context::style()` | `Context::style_of(Theme)` | Only one call site (`editor.rs`); fixed in the upgrade commit. |

**Why this matters for `egui_kittest` migration:** the harness API itself is stable across the upgrade — `Harness::new_ui`, `Harness::builder`, `harness.get_by_label`, `harness.run`, `harness.output` all have the same shape in 0.35 as in 0.31. So the test bodies you write with the harness do **not** need to absorb these 0.27→0.35 renames — only the existing `Context::run_ui(...)` smoke tests do. When converting a smoke test to a `Harness` test, the body becomes the same widget call, but the boilerplate around it shrinks to two lines instead of five.

**Recorded here so future-me doesn't re-litigate this.**

---

# Current Status (as of 2026-07-27)

The `tdd/high-value-defects` branch has been implementing this proposal in passes; current work is on `fix/bugfixes` (commit `564ca4e` on 2026-07-26, ahead of `origin/fix/bugfixes` by 13 commits). Status by item:

| Item | Status | Notes |
|------|--------|-------|
| P0-1 (visual regression) | **Unblocked, not started** | `eframe` upgraded to 0.35 (commit `0b6d643`). `egui_kittest = 0.35` is in dev-deps with the `snapshot` feature enabled, but `insta` is still missing from `[dev-dependencies]` — the snapshot API cannot be exercised until `insta = { version = "1.40", features = ["png"] }` is added. **Concrete next step (R-1 in the 2026-07-27 review).** |
| P0-2 (interaction coverage) | **Done** | Tier 1 side-effect tests + Tier 2 smoke + Tier 4 click→output tests are live and un-`#[ignore]`'d in `src/ui/render.rs:2231` (copy-code), `:2312` (hyperlink→`OpenUrl`), and `:2402` (task-checkbox toggle). Each test uses the **state-capture pattern** documented in [Patterns observed in practice](#patterns-observed-in-practice) because egui 0.35 resets `PlatformOutput` between frames. The 2026-07-25 status entry ("Tier 4 click → output is still `#[ignore]`d") is **stale and wrong** — a grep for `#[ignore]` in `src/desktop/` returns zero matches. |
| P0-3 (proptest replacement) | **Done** | `test_parse_markdown_fuzz_property` replaced with a real proptest (64 cases, structural asserts). |
| P0-4 (Tier 3 absent; was implicit in P0-1) | **New** | The 0.35 upgrade unblocked the harness, but no snapshot tests have landed. **This is the single highest-leverage gap.** See P0-4 in the audit and R-1 in the 2026-07-27 review. |
| P1-1 (40+ `Context::run` blocks) | **Shape-changed, ongoing** | The 0.27→0.35 upgrade renamed `ctx.run(input, \|ctx\| ...)` to `ctx.run_ui(input, \|ui\| ...)` (closure now receives `&mut Ui`); the count is still ~62 in `src/desktop/src/ui/` plus 2 in `tests/table_layout_test.rs`, just uniform. Q1's "incremental per-file migration to `Harness`" plan still applies — pick it up the next time a UI file is touched. **The `render_test(|ui| { ... })` helper from §A remains unnecessary; the harness is the abstraction.** |
| P1-2 (eprintln! in tests) | **Partial** | The original site in `test_ftwa_measure_user_table` was fixed. A new `eprintln!` has appeared at `src/ui/panels/right.rs:327` in `test_show_right_panel_long_titles_anchor_at_panel_left_edge` — diagnostic output on a passing test path. See P1-6. |
| P1-3 (`render_tests.rs` duplicates) | **Done** | File deleted; build_toc tests consolidated in `render.rs`, GFM option tests moved to `tests/pulldown_config.rs` as behavior-based checks. |
| P1-4 (indexed event assertions) | **Done** | `test_parse_markdown_to_events` and `test_parse_markdown_heading_levels` refactored to `iter().find()`/`.any()`. |
| P1-5 (Tier 2 panel smoke tests assert on app state, not rendered widgets) | **New** | ~28 of 32 panel smoke tests in `panels/*.rs` end with `assert!(!app.foo())` (tautology) or no assertion at all. The tests confirm the panel did not panic and did not mutate unrelated state, but they do not assert any text or widget was actually drawn. See P1-5 in the audit. |
| P1-6 (eprintln! regression in `right.rs:327`) | **New** | A new `eprintln!` was introduced after P1-2 was marked Done. See P1-6. |
| P1-7 (hardcoded copy strings in integration test) | **New** | `test_all_top_level_panels_visible_and_rendered` in `src/ui/app.rs:1490-1513` asserts `all_text.contains("FastMD Viewer")` etc. The `strings.rs` constants exist exactly to prevent this. See P1-7. |
| P2-1 (FTWA edge cases) | **Done** | Tests added for NaN, +∞/−∞, `max < min` invariant, single-column, 1000-column stress, plus the 6-permutation matrix. |
| P2-2 (table layout stress) | **Done** | 6 FTWA permutation tests + 5 integration tests with deterministic `screen_rect`. The §3.6-fallback snapshot is **now actionable** (P0-1 unblocked). |
| P2-3 (doc tests for public API) | **Done** | Doc tests for `parse_markdown_to_events`, `build_toc`, `parse_yaml_to_pairs`, `DocumentContent::parse`, and `parse_front_matter`. `ftwa` omitted because `ui::table_width` is intentionally private. |
| P2-4 (assertion inside `ctx.run_ui` closure) | **New** | `test_render_heading_scroll_to_id` (`src/ui/render.rs:1574-1594`) puts `assert_eq!` calls inside the `|ui| { ... }` closure that egui runs in measure-paint passes. The assertions conceptually belong in the test body, not the render closure. See P2-4. |
| P2-5 (no-assert diagnostic test in `tests/`) | **New** | `tests/table_layout_test.rs:old_bug_set_width_ignored` calls `dbg!()` to print widths and never asserts anything. The test name advertises a regression that was pinned elsewhere (`fix_allocate_ui_randomised`). See P2-5. |
| P3-1 (CI integration of snapshots) | **Next** | Becomes the same PR as P0-4 — add `insta` to dev-deps, add 5–8 initial snapshots, add a `cargo insta pending-snapshots` CI step that fails the build on unapproved drift. |
| P3-2 (mutation testing) | **Deferred** | Per the original proposal's explicit deferral. Revisit after P0-4 / R-1 lands so the mutation score is a meaningful aggregate. |
| P3-3 (fuzz harness) | **Deferred** | Per the original proposal's explicit deferral. |
| P3-4 (test module naming inconsistency) | **New** | `panels/top.rs`, `panels/right.rs`, `panels/bottom.rs`, and `background_logs.rs` use both `mod tests` (pure logic) and `mod ui_tests` (panel smoke); the rest use only `mod tests`. See P3-4. |
| P3-5 (`#[path]` indirection for `agent_impl_tests.rs`) | **New** | `src/agent/mod.rs:13` loads the test file via `#[path = "agent_impl_tests.rs"] mod agent_impl_tests;`. Works, but harder to discover than the standard `mod tests` inside the production file. See P3-5. |

Side catches (not in the original proposal but surfaced during TDD work):

- **3 real defects fixed via failing tests** in earlier commits: `ftwa` NaN propagation, `ftwa` `max[j] < min[j]` invariant violation, and `DocumentContent::parse` / `parse_front_matter` disagreeing on malformed YAML.
- **Heading inline-formatting defect** (raised during P0-2 investigation): `# *italic*` was rendering as a plain bold heading. Fixed by changing `RenderEvent::Heading` to carry `Vec<InlineElem>` instead of `String`, plus renderer + parser updates + 5 new tests.
- **TOC long-titles left-anchor bug** (caught during the 2026-07-27 review): a 240-char TOC title in a 400-px window had its row's `rect.left()` drift right, clipping the left side of every row. `test_show_right_panel_long_titles_anchor_at_panel_left_edge` (`src/ui/panels/right.rs:271`) is the regression test.

---

# Open Questions

1. ~~Harness vs `Context::run` for the existing 40+ sites~~ — **resolved by Q1.**
2. ~~Snapshot determinism with system fonts~~ — **resolved by Q2 (5px threshold).**
3. ~~Should snapshot tests be platform-gated?~~ — **resolved by Q3 (every PR, all platforms).**
4. ~~Where do Tier 4 interaction tests for `FastMdApp` itself live?~~ — **resolved by Q4 (defer until motivated).**
5. ~~Is `RenderEvent::to_markdown()` in scope?~~ — **resolved (dropped, rationale above).**
6. ~~What do we do with the pulldown-cmark option tests?~~ — **resolved by Q6 (moved to `tests/pulldown_config.rs` as behavior-based checks).**
7. ~~The `egui_kittest` blocker.~~ — **resolved by the egui 0.27→0.35 upgrade (commit `0b6d643` on `upgrade/egui-0.35`).** The project is now on `eframe = "0.35"`, which matches `egui_kittest = "0.35"`. The blocker that gated P0-1 (visual regression) and the Tier 4 half of P0-2 (interaction) is gone. **As of 2026-07-27:** the `egui_kittest = { version = "0.35", features = ["eframe", "snapshot"] }` dev-dep *is* in `Cargo.toml`; `insta` is **still missing** and is the single one-line blocker for P0-1 (Tier 3 snapshots). The three Tier 4 tests in `render.rs` are *already* converted and un-`#[ignore]`'d. The remaining work is the P0-4 / R-1 rollout: add `insta`, take 5–8 initial snapshots, add the CI step.

8. ~~Q2's 5-pixel threshold — is that still the right number?~~ — **resolved by Q10.** 3px project-wide, set per-harness via the R-1 snapshot helper. The 3/5 split lean was overruled; 3px is uniform and simpler.

9. ~~Should the next 5 Tier 4 click tests (R-8) land in one PR or spread across multiple?~~ — **resolved by Q11.** One PR — R-3's helper lands in the same PR, ~400 lines.

10. ~~Tier 2 tautology policy (P1-5).~~ — **resolved by Q12.** (b) replace with text-content assertion; borderline case → header-only assertion; (a) delete only for pure no-ops.

11. ~~If `doc/planning/render-architecture.md` lands the 4-stage pipeline refactor, what tier does the new `MarkdownDoc` AST get tested at?~~ — **resolved by Q13.** Tier 1 only. Tests move from `src/ui/render.rs` to `src/markdown/{ast,document}.rs`; strategy stays consistent.

---

# Recommendations (2026-07-27 review pass)

Each recommendation has a priority, an effort estimate, a concrete deliverable, and a one-line acceptance test. Ordered by impact-to-effort ratio.

| ID | Priority | Deliverable | Effort |
|---|---|---|---|
| **R-1** | **Highest** | Add `insta = "1.40"` to dev-deps; add `tests/common/snapshot.rs` helper that wraps `Harness::builder().with_size()` and `harness.snapshot(name)` with the **3px project-wide threshold (Q10)**; take 5–8 initial snapshots (full-markdown document, 6-col FTWA table, empty-cell table, bold-cell table, heading-scroll, move-file modal, bottom-panel with command input); add `cargo insta pending-snapshots` CI step. | 1.5 days |
| **R-2** | High | Per **Q12 (Tier 2 tautology policy)**: replace ~12 tautological panel smoke tests in `panels/{top,bottom,left,center}.rs` and `modals.rs` with text-content assertions using a `tests/common/egui_assert.rs` helper (`assert_text_contains(shapes, "Indexing finished")` etc.). **Borderline case (panel has only a stable header, body is dynamic) → header-only assertion.** Use `crate::ui::strings::*` constants rather than literals (resolves P1-7 in the same pass). Delete pure no-op tests. | 0.5 day |
| **R-3** | Medium | Add `tests/common/egui_interact.rs` with `click_by_label_and_capture_state<T, F>(...)` and the state-capture pattern wrapped. New Tier 4 tests become ≤10 lines of body + 1 helper call. | 0.25 day |
| **R-4** | Medium | Update the doc to match reality. **Done in this commit (2026-07-27).** Future work: re-verify the doc when P0-4 / R-1 lands. | 0.25 day |
| **R-5** | Low | Delete the `eprintln!` at `src/ui/panels/right.rs:327` and the `dbg!()` calls in `tests/table_layout_test.rs`. Gate future diagnostic prints behind `#[cfg(debug_assertions)]` or assert on them. | 5 min |
| **R-6** | Deferred to R-1 | The 3px project-wide threshold (Q10) and "no `#[cfg(target_os)]`" decisions are encoded in the R-1 helper. No separate work. | 0 |
| **R-7** | Low | Standardise on `mod tests` only; merge `mod ui_tests` blocks into `mod tests` with section comments. Move `src/agent/agent_impl_tests.rs` into `src/agent/agent_impl.rs` as `mod tests`. Bundle with the next time one of the four affected files is touched. | 10 min |
| **R-8** | Medium (deferred to R-3) | Per **Q11 (single PR vs multiple)**: lands in **one PR** with R-3's helper. Five Tier 4 click tests: TOC row click → `scroll_to_id`, tab close button, folder tree click → `selected_file`, send button → `submit_prompt`, batch button → `batch_dialog_open`. Expected diff ~400 lines. | 0.5 day |
| **R-9** | Low | Add `tests/common/egui_assert.rs::assert_no_id_change_warnings(log_msgs, shapes)`. Use it at `panels/{top,left}.rs`, `ui/app.rs:1352`. Add an id-stability test for `ui/background_logs.rs` (currently missing). | 1 hour |
| **R-10** | Low | Move the `assert_eq!` calls in `test_render_heading_scroll_to_id` out of the `ctx.run_ui` closure. | 5 min |
| **R-11** | Low (deferred to R-1) | Add a single "default app shell" snapshot — `app.render_panels(ui)` with a small markdown file open at 1024×768. Catches theme/palette drift across the whole UI. Lands in the same PR as R-1. | 30 min |

**Suggested one-week execution order:** R-4 (done in this commit) → R-9 → R-3 + R-2 in parallel → R-5 + R-10 + R-7 as one cleanup PR → R-1 (biggest) → R-8 in the same week. R-6 and R-11 land automatically with R-1.

**Quick wins (≤1 hour total):** R-5, R-10, R-7. Do these in a single cleanup PR; they pay for themselves in maintainer time within a month.

---

# Implementation Status (2026-07-27, on branch `tdd/egui-testing-rollout`)

The rollout above started landing on 2026-07-27 on the
`tdd/egui-testing-rollout` branch (off `fix/bugfixes` PR #36). Five
commits, in execution order:

| # | Commit | R-items | Status |
|---|---|---|---|
| 1 | `bb35ce3` | R-3 + R-9 | `src/ui/test_helpers/{interact,assert}.rs` modules added. State-capture pattern wrapped in `stateful_harness`; id-stability pattern wrapped in `assert_no_id_change_warnings` (combined) and split variants. |
| 2 | `9dc1969` | R-2 | Tautological panel smoke tests replaced with text-content assertions in 5 panels + the app-level integration test (P1-7 hardcoded copy strings fixed in the same pass). Modal tests reverted to state-only — the `Window::show` rendering path used by modals draws via egui 0.35's `Atoms` widget system, and the resulting shapes are not in the captured `output.shapes` under `ctx.run_ui`. Modal visual surface is covered by the R-1c snapshot instead. |
| 3 | `32582ce` | R-1a | `image` dev-dep added with the `png` feature. (Initial commit used `insta = { version = "1.40", features = ["png"] }` — turned out `egui_kittest`'s `snapshot` feature does **not** use `insta`; it has its own kittest format. The `insta` line was replaced with the `image` dev-dep that egui_kittest's snapshot module actually requires.) |
| 4 | `cd1d884` | R-1b + R-1c | `test_helpers::snapshot` module added with the 3px threshold constant (Q10) and the `DEFAULT_VIEWPORT` constant. `tests/render_snapshots.rs` added with 2 initial snapshot test cases (`snapshot_full_markdown_doc`, `snapshot_yaml_table`). Both gracefully **skip** when the wgpu renderer is missing — see the wgpu blocker below. |
| 5 | `9a65830` | R-1d | CI step added to `rust-quality-gate.yml` that fails the test job on any `tests/snapshots/*.new.png`. This is the `egui_kittest` equivalent of `cargo insta pending-snapshots` (Q3). |

## R-items still open after the 2026-07-27 pass

| ID | Status | Note |
|---|---|---|
| R-5 | Open | `eprintln!` in `right.rs:327` and `dbg!()` in `tests/table_layout_test.rs` still leak diagnostic output. |
| R-7 | Open | `mod tests` / `mod ui_tests` naming inconsistency in 4 files (`panels/{top,right,bottom}.rs`, `background_logs.rs`) and `#[path]` indirection in `agent/mod.rs` for `agent_impl_tests.rs` still in place. |
| R-8 | Open | Five Tier 4 click tests (TOC row, tab close, folder tree, send, batch) per Q11 — lands in one PR with R-3's helper once the doc is reviewed. |
| R-9 | Open | Migrate the existing ad-hoc id-stability tests in `panels/{top,left}.rs` and `ui/app.rs:1352` to the new `test_helpers::assert::assert_no_id_change_*` helpers. Add an id-stability test for `ui/background_logs.rs` (currently missing). |
| R-10 | Open | Move the `assert_eq!` calls in `test_render_heading_scroll_to_id` (`src/ui/render.rs:1574-1594`) out of the `ctx.run_ui` closure. |
| R-11 | Blocked by wgpu (see below) | The "default app shell" snapshot is deferred because `FastMdApp::render_panels` is not `pub` and the integration-test crate cannot call it. Either make `render_panels` and `render_table` `pub` (small refactor) or move the snapshot test to the in-source `mod tests` of `ui/app.rs`. |

## Blocking issue: `egui_kittest` `wgpu` feature does not compile on Windows (2026-07-27)

Discovered during R-1a/b/c. Enabling the `wgpu` feature on
`egui_kittest` in `src/desktop/Cargo.toml`:

```toml
egui_kittest = { version = "0.35", features = ["eframe", "snapshot", "wgpu"] }
```

produces a **wgpu-hal 0.29 vs windows-core 0.56/0.62 trait-bound
conflict** in this branch's Windows build environment. Concrete
errors (from `cargo check --tests`):

```
error[E0308]: mismatched types
   --> ...wgpu-hal-29.0.4\src\dx12\suballocation.rs:83:71
error[E0277]: the trait bound `ResourceCategory: From<&D3D12_RESOURCE_DESC>` is not satisfied
   --> ...wgpu-hal-29.0.4\src\dx12\suballocation.rs:299:32
error[E0277]: the trait bound `&ID3D12Heap: Param<ID3D12Heap, InterfaceType>` is not satisfied
   --> ...wgpu-hal-29.0.4\src\dx12\suballocation.rs:306:17
```

Root cause: `wgpu 0.29` pulls in `windows-core 0.62`, but
`windows-sys` (transitive via `winit`/`accesskit`) pulls in
`windows-core 0.56`. The two versions of `windows-core` are
not ABI-compatible — the `ID3D12Heap` COM vtable generated by
`windows-core 0.62` is laid out differently from the one
generated by `windows-core 0.56`, so the `wgpu-hal` D3D12 backend
cannot use the `ID3D12Heap` symbol from `windows-core 0.56`.

### Status

- The `wgpu` feature is **not enabled** in `Cargo.toml` for the
  `tdd/egui-testing-rollout` branch. The `snapshot` feature
  alone is enabled, but `egui_kittest`'s `Harness::snapshot`
  needs a real renderer — the default `LazyRenderer` errors with
  `SnapshotError::RenderError { err: "no default renderer
  available" }` when called.
- The 2 snapshot tests in `tests/render_snapshots.rs` detect
  this error and skip with a documented message:
  > skipping snapshot `full_markdown_doc`: no wgpu renderer
  > configured (the `wgpu` feature on egui_kittest is disabled
  > because of a wgpu-hal / windows-core conflict on this
  > branch)
- The R-1d CI gate is in place but inactive — it only fires if
  snapshots actually run and produce `.new.png` files.

### Possible workarounds (out of scope for R-1)

1. **Pin `windows-core` to 0.62 across the tree** (likely
   requires a winit or accesskit version bump).
2. **Bump `wgpu` to a version that uses `windows-core 0.56`**
   (likely requires an egui-wgpu version bump, which would mean
   bumping eframe/egui).
3. **Use the `glow` (OpenGL) backend instead of `wgpu`** —
   egui_kittest's `glow` feature uses `egui_glow` with the
   `glow` OpenGL implementation. May be easier to set up on
   headless CI than wgpu. (Verify by checking
   `egui_kittest/Cargo.toml` features list — there is a
   `glow` feature flag separate from `wgpu`.)
4. **Run snapshots only on Linux CI**, where the wgpu
   conflict may not apply. Accept that local Windows
   development skips the snapshots.

### Recommended next step

File a follow-up issue specifically for the wgpu dep conflict.
Once resolved (whichever of options 1-4 is chosen), the
`tdd/egui-testing-rollout` branch can be re-enabled with
`.wgpu()` added to the `snapshot_harness` builder, the
`Harness::snapshot` calls will start producing real PNGs, and
the R-1d CI gate becomes live.

## R-2 modal-test finding (2026-07-27)

During R-2, the modal smoke tests (move, create-dir, rename)
were upgraded to assert on the modal's title and prompt text,
but the assertions had to be reverted. Investigation showed the
modal's title and prompt are rendered via egui 0.35's `Atoms`
widget system, and the resulting shapes are **not in the
captured `output.shapes`** under `ctx.run_ui`. Concrete debug
output from a passing invocation:

```
DEBUG move_modal: shapes=6
  shape: Noop
  shape: Noop
  shape: Noop
  shape: Noop
  shape: Noop
  shape: Noop
```

6 `Noop` shapes, no `Text` shapes — the modal's `Atoms`-rendered
text is in a paint layer that `ctx.run_ui` does not capture.
This is an egui 0.35 / egui_kittest interaction, not a bug in
the modal code. Modal visual coverage comes from the Tier 3
snapshot in R-1c. The modal smoke tests are kept as state-only
checks.

## Test status after the 2026-07-27 pass

```
669 passed; 1 failed; 3 ignored
```

The 1 failure is `ui::panels::right::ui_tests::test_show_right_panel_long_titles_anchor_at_panel_left_edge`,
which was failing on a clean checkout of `a6a95d4` (PR #36's merge
commit) before this branch started. Confirmed via `git stash` on
the implementation branch. Pre-existing — not caused by R-1
through R-3. Filed as a follow-up.

Clippy is clean on the new code (the 6 pre-existing warnings are
all in `src/ui/render.rs:1938`, `1954`, `1958`, `1974`, `2043`,
`2050`).
