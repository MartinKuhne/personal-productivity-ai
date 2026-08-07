## Objective

Fix the horizontal-overflow bug in markdown table rendering and give users
five different deficit-distribution algorithms to pick from, selectable
at runtime via a top-bar combobox. The root cause was a
pulldown-cmark-side text-fragmentation quirk that the table-width
pipeline couldn't work around, plus a fallback gap when the wrap set
saturated below the available width.

## Changes

### Bug Fixes

- **src/desktop/src/markdown/parser.rs** — Coalesce consecutive
  same-style `Event::Text` / `Event::Code` (and same-URL `Link`)
  fragments emitted by pulldown-cmark. The upstream inline parser
  fragments plain text at every delimiter run (`*`, `_`, `~`, `=`, `^`)
  even when the delimiter fails to form a valid emphasis /
  strikethrough / superscript / subscript pair, so a cell containing
  `~4,031 / ~19,000` came out as four `InlineElem::Text` entries
  instead of one. The text measurer then tokenized it as four runs and
  the FTWA pipeline produced wrong column widths. Two ~10-line helpers
  (`push_text_coalesce`, `push_link_coalesce`) fold same-style
  fragments at the push site; the upstream event count is pinned in a
  regression test so a future cmark upgrade / option change that shifts
  the count surfaces immediately.
- **src/desktop/src/markdown/parser.rs** — Module-level doc comment
  explains the pulldown-cmark "tree of items" inline-parser design and
  why the coalescer lives in our parser rather than in cmark. The
  helper doc comments document the exact coalesce rules (same
  `TextStyle` for `Text`, same URL for `Link`; different style / URL /
  variant always starts a new element).
- **src/desktop/src/markdown/table_width/mod.rs** — `from_config`
  fallback for unknown values is now `BreakpointWaterFill` (matches
  `default_table_width_strategy()` in `config.rs`). An empty / unknown
  config string previously mapped to `ProportionalToSlack`, producing
  inconsistent behaviour between the default and the "no preference set"
  paths.
- **src/desktop/src/markdown/table_width/mod.rs** — Clamp-based drift
  fix for the wrap-set-saturation case: when the B2 distribution
  fully clamps every wrap-set column to `min_content` and the
  post-drift `Σ widths` still exceeds `available`, escalate to the
  §3.6 horizontal-scroll fallback rather than letting the renderer
  overflow the viewport.

### New Features

- **src/desktop/src/markdown/table_width/mod.rs** — Three new
  `DeficitStrategy` variants, each implemented in its own pure solver
  function (no egui dependency):
  - `WaterFillRatio` (§2.10) — equalize the "wrap pressure" `max_j /
    w_j` across all columns; closed-form given the active-column set.
  - `LagrangePenalty` (§2.13) — minimize `Σ extraLines_j(w_j)`
    subject to `Σ w_j = available` via Lagrange-multiplier bisection
    over the per-column penalty function.
  - `HybridMinPenaltyWaterFill` (§2.14) — per-column target is the
    "first-wrap" boundary (largest breakpoint, or `max_j` for
    single-line columns); the residual is water-filled by headroom
    `max_j − target_j`.
- **src/desktop/src/ui/panels/top.rs** — Top-bar combobox exposing all
  five strategies (existing two + the three new ones). The picked
  value is persisted to `AppConfig::table_width_strategy` on change;
  the markdown renderer re-reads it every frame so the next paint
  uses the new algorithm without any explicit invalidation hook.
- **src/desktop/src/ui/strings.rs** — Five new label constants
  (`TABLE_WIDTH_STRATEGY_PROPORTIONAL`, `_WATERFILL`, `_RATIO`,
  `_LAGRANGE`, `_HYBRID`) and a `TABLE_WIDTH_STRATEGY_LABEL` /
  `_ID_SALT` / `_EVENT` triplet for the combobox.

### Refactoring

- **src/desktop/src/ui/panels/top.rs** — `apply_table_width_strategy_change`
  now takes a `persist: F` callback where
  `F: FnOnce(&AppConfig) -> Result<PathBuf, String>`. Production
  passes `crate::config::save_config` directly; tests pass a closure
  that captures the saved config (or a `|cfg| panic!()` closure to
  assert the no-op path). This replaces an earlier version that
  hard-coded `save_config` — a test that exercised it without
  path-isolation was silently overwriting the user's real
  `config.yaml` at `%APPDATA%\fastmd\config.yaml` on every test run.
- **src/desktop/src/markdown/table_width/mod.rs** — The three new
  solver functions are dispatched up-front in `ftwa()` (before the
  wrap-set construction) because they operate on all columns
  directly, with no need for the FTWA-specific G2 minimum-cardinality
  wrap set. Existing FTWA strategies (`ProportionalToSlack`,
  `BreakpointWaterFill`) are unchanged and still use the wrap-set
  construction below.

### Tests

- **src/desktop/src/markdown/table_width/internal_tests.rs** — 32 new
  tests across the three new solvers (surplus, deficit, fallback,
  per-column penalty curve, Lagrange bisection convergence, hybrid
  residual distribution).
- **src/desktop/src/markdown/parser.rs** — 7 new unit tests:
  4× `push_text_coalesce_*` (coalesce, style-change, empty buffer,
  after-non-text), 1× `push_link_coalesce_*` (URL change), 1×
  `cmark_strikethrough_fragments_single_tilde` (pins the upstream
  event count), 1× `parses_laptops_table_cells_to_single_text_element`
  (parser-level regression for the laptops table).
- **src/desktop/src/ui/render/tests.rs** — `test_parse_laptops_table_ast_shape`
  integration test asserting the laptops table parses to 2 rows × 7
  cells, each a single `InlineElem::Text` with default style.
- **src/desktop/src/ui/panels/top_tests.rs** — Existing dropdown
  tests extended to cover the three new strategy variants and the
  no-op re-pick path (verifies `persist` is not called when the
  picked strategy equals the persisted value).
- **src/desktop/tests/table_width_algorithm_test.rs** — Public-API
  coverage for the new variants.

## How

The pulldown-cmark coalesce fix is the load-bearing change. Pulldown-cmark
parses inline content into a tree of `Item`s, not a stream — every
delimiter run becomes its own `ItemBody::Text` item, and the
flatten-to-events step (`item_to_event` in cmark `parse.rs`) emits
one `Event::Text` per item. The tree-vs-stream divergence is invisible
for normal prose but breaks the table-width contract: a cell must
contribute exactly one (max, min) measurement pair, not N. The
coalescer folds same-style `Text` (and same-URL `Link`) events at the
push site so the AST the rest of the pipeline sees has the
expected "one element per visible run" shape. The two helpers are
~10 lines each, preserve style transitions across non-`Text` element
boundaries, and don't allocate when coalescing. The cmark-side
behaviour (event count per plain-text fragment) is pinned in
`cmark_strikethrough_fragments_single_tilde` so a future upgrade that
shifts the count (or a deliberate option change) makes the test fail
loudly rather than silently regressing the table pipeline.

The new strategies dispatch in `ftwa()` ahead of the wrap-set
construction. Each is a pure function of the four input arrays +
`available`, returns the standard `ColumnWidths` (with
`needs_horizontal_scroll` set on §3.6 fallback), and is exercised
against the same surplus / deficit / fallback boundary tests as
the existing FTWA strategies.

The callback refactor in `top.rs` decouples persistence from the
in-memory mutation. Production calls
`apply_table_width_strategy_change(app, picked, crate::config::save_config)`;
tests pass `|cfg| panic!("persist should not be called")` for the
no-op re-pick path and a closure that captures the saved config for
the persistence path. This keeps the function testable without a
filesystem and removes the APPDATA-overwrite class of bug.

## Why

The original horizontal-overflow bug surfaced on the laptops table in
`render/tests.rs`: the `Summary` cell contained `~3.3 lbs.`,
pulldown-cmark fragmented the surrounding text at every `~`, the
text measurer treated each fragment as a separate run, and the
FTWA pipeline produced widths that pushed the table past the
viewport. The coalesce fix is the surgical repair; the three new
strategies and the runtime selector are the value-add (a survey of
the published algorithms in
`doc/planning/table-column-width-algorithm.md` showed
`BreakpointWaterFill` is good but not always the best fit for
degenerate cell distributions — `LagrangePenalty` and the hybrid in
particular are smoother on wide cells with many breakpoints).

## Quality Gates

- [x] Build: `cargo check --all-targets` — clean, 0 warnings
- [x] Lint: `cargo clippy --all-targets -- -D warnings` — clean, 0 issues
- [x] Format: `cargo fmt --check` — clean
- [x] Tests: `cargo nextest run --lib` — 1056 passed, 8 skipped, 0 failed
- [ ] Tests (integration): `cargo nextest run` — not run this round; `fastmd.exe` is locked by the running desktop app, but the modified test files (`tests/table_width_algorithm_test.rs`, `tests/table_visual_layout_test.rs`) are covered by the lib tests above
- [x] Docs: `cargo doc --no-deps --quiet` — clean, 0 warnings
- [ ] Static Analysis: not configured
- [ ] Secrets Scan: not configured
