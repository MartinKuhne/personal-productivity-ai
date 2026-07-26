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
egui_kittest = { version = "0.27", features = ["eframe", "snapshot"] }
insta = { version = "1.40", features = ["png"] }
proptest = "1.4"  # for property-based tests on the parser
```

- `egui_kittest` 0.27 is the matching version for `eframe = "0.27"`. The `eframe` feature pulls in the harness; the `snapshot` feature adds the `snapshot()` API.
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

# Current Status (as of 2026-07-25)

The `tdd/high-value-defects` branch has been implementing this proposal in passes. Status by item:

| Item | Status | Notes |
|------|--------|-------|
| P0-1 (visual regression) | **Blocked** | Requires `egui_kittest` (egui 0.31+); project is on eframe 0.27. See "Open Question 7" below. |
| P0-2 (interaction coverage) | **Partial** | Tier 2 smoke tests + Tier 1 side-effect tests added for copy-code, hyperlink, task-list checkbox. Tier 4 click → output is `#[ignore]`d pending `egui_kittest`. |
| P0-3 (proptest replacement) | **Done** | `test_parse_markdown_fuzz_property` replaced with a real proptest (64 cases, structural asserts). |
| P1-1 (40+ `Context::run` blocks) | **Resolved by Q1** | Incremental per-file migration; no helper introduced. |
| P1-2 (eprintln! in tests) | **Done** | Replaced with real assertions in `test_ftwa_measure_user_table`. |
| P1-3 (`render_tests.rs` duplicates) | **Done** | File deleted; build_toc tests consolidated in `render.rs`, GFM option tests moved to `tests/pulldown_config.rs` as behavior-based checks. |
| P1-4 (indexed event assertions) | **Done** | `test_parse_markdown_to_events` and `test_parse_markdown_heading_levels` refactored to `iter().find()`/`.any()`. |
| P2-1 (FTWA edge cases) | **Done** | Tests added for NaN, +∞/−∞, `max < min` invariant, single-column, 1000-column stress, plus the 6-permutation matrix. |
| P2-2 (table layout stress) | **Done** | 6 FTWA permutation tests + 5 integration tests with deterministic `screen_rect`. The §3.6-fallback snapshot is blocked by `egui_kittest`. |
| P2-3 (doc tests for public API) | **Done** | Doc tests for `parse_markdown_to_events`, `build_toc`, `parse_yaml_to_pairs`, `DocumentContent::parse`, and `parse_front_matter`. `ftwa` omitted because `ui::table_width` is intentionally private. |
| P3-1 (CI integration of snapshots) | **N/A** | No snapshots exist (P0-1 blocked). The CI workflow already runs `cargo test` which includes doc tests. |
| P3-2 (mutation testing) | **Deferred** | Per the original proposal's explicit deferral. |
| P3-3 (fuzz harness) | **Deferred** | Per the original proposal's explicit deferral. |

Side catches (not in the original proposal but surfaced during TDD work):

- **3 real defects fixed via failing tests** in earlier commits: `ftwa` NaN propagation, `ftwa` `max[j] < min[j]` invariant violation, and `DocumentContent::parse` / `parse_front_matter` disagreeing on malformed YAML.
- **Heading inline-formatting defect** (raised during P0-2 investigation): `# *italic*` was rendering as a plain bold heading. Fixed by changing `RenderEvent::Heading` to carry `Vec<InlineElem>` instead of `String`, plus renderer + parser updates + 5 new tests.

---

# Open Questions

1. ~~Harness vs `Context::run` for the existing 40+ sites~~ — **resolved by Q1.**
2. ~~Snapshot determinism with system fonts~~ — **resolved by Q2 (5px threshold).**
3. ~~Should snapshot tests be platform-gated?~~ — **resolved by Q3 (every PR, all platforms).**
4. ~~Where do Tier 4 interaction tests for `FastMdApp` itself live?~~ — **resolved by Q4 (defer until motivated).**
5. ~~Is `RenderEvent::to_markdown()` in scope?~~ — **resolved (dropped, rationale above).**
6. ~~What do we do with the pulldown-cmark option tests?~~ — **resolved by Q6 (moved to `tests/pulldown_config.rs` as behavior-based checks).**
7. **The `egui_kittest` blocker.** P0-1 (visual regression) and P0-2 (Tier 4 interaction) both require `egui_kittest::Harness`, which is published only for egui 0.31+. The project is on `eframe = "0.27"` (pulling in egui 0.27). Options:
   - **Upgrade eframe to 0.31+.** Touches every `eframe` / `egui` import across the desktop codebase; risk surface is large but the win is access to the full Tier 3 + Tier 4 toolkit. Estimated effort: 2–4 days of mechanical porting + testing.
   - **Stay on 0.27 and live with Tier 2 + Tier 1 coverage.** Acceptable for now: the 3 click handlers in `render.rs` have `#[ignore]`'d Tier 4 tests that document the expected migration path. When the upgrade lands, un-ignore them and they exercise the same code.
   - **Third-party headless harness for egui 0.27.** A quick survey didn't surface one; the egui 0.27 ecosystem predates the `egui_kittest` work. Not recommended.

   Recommendation: stay on 0.27, accept Tier 2 + Tier 1 coverage for now, and un-ignore the Tier 4 tests in a dedicated upgrade PR.
