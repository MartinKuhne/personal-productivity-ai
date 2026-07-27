# Research: Fair Table Column Width Determination

> Status: **Research / options document** (non-destructive). No code changes yet.
> Scope: Algorithm design for assigning pixel widths to markdown/GFM table columns
> rendered in the egui center panel (see `src/desktop/src/ui/render.rs:215` `render_table`).
> Author intent: reconcile three conflicting goals:
>   **G1** – minimize total word-wrap (extra wrapped lines),
>   **G2** – minimize the *number* of columns that wrap,
>   **G3** – use all available horizontal space.

---

## 1. Problem framing & notation

A table has `N` columns and available container width `W` (content rect minus
padding/borders/gutters). For column `j` we measure, up front, *intrinsic*
widths from the scanned cells:

| Symbol | Meaning |
|---|---|
| `max_j` | **max-content** width of column `j` = width of its widest cell laid out on a single line (the "ideal" no-wrap width). |
| `min_j` | **min-content** width of column `j` = width of its longest *unbreakable* token (a hard floor; a single-token column has `min_j == max_j`). |
| `slack_j` | `max_j − min_j` = how much column `j` can shrink before it *must* wrap a word. |
| `cellLines_j(w)` | greedy line-break count of column `j`'s tallest cell at width `w` (`w` ≥ `min_j`). Monotone non-increasing step function. |
| `extraLines_j(w)` | `Σ_cell (cellLines_cell(w) − 1)` or, for the count-objective variant, `max_cell (cellLines_cell(w) − 1)`. |

A column **wraps** iff some cell in it yields > 1 line, i.e. `extraLines_j(w) > 0`
(equivalently `w < max_j`, *provided* the tallest cell has ≥ 2 words; a single-word
cell never wraps and its column has `slack_j == 0`).

Decision variables: `w_j ∈ [min_j, max_j]` (optionally allowing `w_j > max_j` to
absorb spare width when `W` is abundant). Subject to `Σ w_j = W` (G3).

### 1.1 Why the goals conflict

* If `Σ max_j ≤ W`, all three are simultaneously satisfiable: give every column
  `max_j` and split the leftover fairly (G1 = G2 = 0, only G3 distribution left).
* The interesting regime is the **deficit** case `Σ max_j > W`. We must shrink some
  columns below `max_j`, forcing wraps. *Which* columns take the hit, and by how
  much, is where G1 and G2 diverge:
  * **G2-first**: concentrate the deficit on the fewest columns → fewest "victims."
  * **G1-first**: spread the deficit so each column wraps as little as possible →
    more columns wrap but each minimally.
* G3 ("use all space") is a hard equality constraint; in the surplus regime it is
  pure spare distribution and competes only with aesthetics.

### 1.2 What "fair" means here

We adopt a **lexicographic / Rawlsian** reading of fairness, mirroring how a fair
allocator minimizes the number of harmed parties first, then the severity of harm:

```
Priority A  minimize  C = |{ j : column j wraps }|            (G2)
Priority B  minimize  T = Σ_j extraLines_j(w_j)               (G1)
Priority C  use all space  (Σ w_j = W)                         (G3)
Priority D  distribute any remaining slack/spare evenly        (fairness tiebreak)
```

Rationale: a solution that wraps 1 column by 4 lines is "fairer" than one that
wraps 4 columns by 1 line each when judged by G2, even though both have `T = 4`.
The lexicographic order makes this preference explicit and unambiguous.

---

## 2. Survey of existing / prior-art algorithms

### 2.1 CSS Intrinsic & Extrinsic Sizing (W3C) — `table-layout: auto`
The browser algorithm computes per-column `max-content` and `min-content`,
then distributes available width by shrinking columns in proportion to how much
slack they have, iteratively, until the table fits. This is the ancestor of most
"two-pass shrink" algorithms.
* Handles G3 well (uses all space).
* Does **not** explicitly minimize wrap count (G2) — it tends to spread shrinkage,
  i.e. it optimizes G1-style proportional resizing rather than G2.
* No notion of line-count cost; it minimizes overflow, not wrap lines.

### 2.2 CSS Grid `minmax(auto, max-content)` / `auto-fit`
Grid auto-track sizing is a refinement of 2.1: tracks grow to `max-content`
first, then shrink toward `min-content` when overflowing, weighted by their
`max-content`. `auto-fit` collapses empty tracks and redistributes. Same
character trade-offs as 2.1; no lexicographic wrap-count minimization.

### 2.3 The `table-layout: fixed` model
Widths are dictated by the first row / explicit CSS only; content is then
clipped/wrapped into those widths. Solves sizing in O(1) but ignores content
shape entirely; useful as a *fallback* when available width is tiny but makes
no attempt at G1/G2. Not a candidate for the "fair content-aware" goal, but
relevant as a degenerate branch when `W < Σ min_j` (content cannot fit at all
→ fall back to horizontal scroll, which is what the current code does).

### 2.4 HTML4 / RFC-1945-era column width algorithm (Bos, Lie et al.)
The classic "used width" computation: each column gets a preferred width from
its widest cell; the table's preferred width is their sum; if that exceeds the
container, columns are reduced in proportion to `(max − min)` until each reaches
`min`. Identical in spirit to 2.1 (a single proportional-shrink pass); predecessor
of the W3C intrinsic sizing spec.

### 2.5 Spreadsheet "auto-fit" (Excel / Sheets) — longest-cell heuristic
Sets each column width to its widest cell with no wrapping (pure `max-content`).
If the table overflows the viewport, the user scrolls. This is *exactly* the
current `render_table` behavior (`ScrollArea::horizontal` + `Grid`). It
trivially achieves G1 = G2 = 0 *within the table* at the cost of abandoning G3
and forcing horizontal scrolling. The present task is to do better than this.

### 2.6 Qt `QHeaderView` resize modes
* `ResizeToContents` = §2.5.
* `Stretch` = proportional fill, ignoring content (wraps aggressively in narrow
  columns). Optimizes G3, ignores G1/G2.
* `Interactive` = user-driven; irrelevant algorithmically.
* `Stretch` + `ResizeToContents` mix: content-sized columns get a floor, the
  remainder is stretched. No wrap-count minimization.

### 2.7 Knuth–Plass / TeX paragraph breaking (per-column optimal wrapping)
Each column is laid out as a min-cost paragraph break that minimizes line badness
for a *fixed* `w_j`. This optimizes G1 *for a given width assignment* but does
not decide widths across columns. Composing it with width selection turns the
whole problem into a combinatorial optimization (a per-column subproblem with a
global budget); expensive and overkill for a small markdown table UI. Useful as
the inner `cellLines_j(w)` oracle (greedy first-fit is usually sufficient).

### 2.8 LP / convex-optimization formulation
Treat `w_j` as continuous, define a piecewise-linear convex surrogate of
`extraLines_j` (the per-column wrap-line count as a function of `w`), and solve
a constrained program:
```
minimize   λ·(|wrapping columns|) + Σ_j extraLines_j(w_j)
s.t.       Σ_j w_j = W
           min_j ≤ w_j ≤ max_j
```
Strictly minimizing the *count* of wrapping columns is non-convex (cardinality
penalty); the standard workaround is an `L1`-style surrogate (shrinkage
`max_j − w_j`), which implicitly pushes shrinkage onto few columns — a soft
proxy for G2. With `λ` tuned to prefer G2 first, this approximates the
lexicographic goal. General-purpose LP for an inner loop is too heavy for a UI
hot path, but the *structure* of its solution is what motivates §3.

### 2.9 Greedy two-pass shrink (WebKit / Gecko `nsTableOuterFrame`)
Pass 1: assign `max_j` to every column. Pass 2: while `Σ w_j > W`, pick the
column with the largest `slack_j` (or largest `max−w` ratio) and decrement it;
stop when the table fits or all columns are at `min_j`.
* Greedily favors taking the deficit from the columns that *can* give it up —
  a heuristic proxy for G2.
* Naive versions shrink in equal tiny steps (O(pixel·N)); optimized versions
  jump to the next "knee" of each `cellLines` step function.
* Does **not** guarantee minimum wrap count; can be shown to be a 2-approx for
  slack-constrained packing in the worst case.

### 2.10 Water-filling / iterative equalization
Allocate width so that all columns reach an equal *marginal* cost. In the surplus
regime this is "give everyone a fair share of the extra"; in the deficit regime
it equalizes the extra-line count across the shrinking columns. Equalizes G1
across victims but tends to *maximize* the number of victims (opposite of G2) —
so pure water-filling is great as the **intra-set distributor** (Priority D) but
bad as the **set selector** (Priority A).

### 2.11 Hardness note
Selecting the minimum-cardinality subset S of columns to shrink such that
`Σ_{j∈S} slack_j ≥ D` (deficit) is a **minimum-cardinality subset-sum / knapsack**
variant. Fortunately it admits an exact **greedy** solution when we only require
the *sum* of slacks to reach D (proof below): sort by `slack_j` descending and
take a prefix. Exactness holds because all columns contribute the *same kind* of
capacity (width units) and we only bound the cardinality — exchange argument:
any optimal k-subset can be replaced by the top-k slacks without reducing total
slack. So **Priority A is solvable in `O(N log N)`**.

---

## 3. Proposed algorithm — "Fair Table Width Algorithm" (FTWA)

Combines §2.11 (exact wrap-count minimization) for G2, §2.10 (water-filling) for
G1/fairness inside the chosen wrap set, and §2.1 surplus distribution for G3.

### 3.1 Inputs / precompute (the measurement pass)
For each column `j`, walk every cell once and record:
* `max_j`  — the per-cell one-line `Galley`/text-size width, take the column max.
* `min_j`  — the longest unbreakable token width in any cell of the column.
* `breakpoints_j` — a sorted list of width thresholds at which the tallest cell
  (or, for the sum-cost variant, the Σ-cell) line count increases. Concretely,
  the set of widths `b` where `cellLines_j(b) > cellLines_j(b+ε)`. Between two
  breakpoints the line count is constant. (egui's text shaping exposes the
  glyph advances needed; `LayoutJob` can produce these cheaply per token.)

Define the per-column discrete cost function on these breakpoints:
`cost_j(b) = extraLines_j(b)` (number of extra lines at width `b`), with
`cost_j(max_j) = 0` and `cost_j(min_j) =` the worst-case wrap count.

### 3.2 Phase A — surplus regime (`W ≥ Σ max_j`)
No column *needs* to wrap. Set `w_j = max_j` for every column. Goals
G1 = G2 = 0; G3 is intentionally **relaxed** (the table may not fill the
full available width). **Done.**

> **Revision note.** An earlier revision of this algorithm distributed
> the spare `W − Σ max_j` proportionally to `max_j` (Decision 7 below).
> That produced the "infinite-width column" visual defect: a column
> whose content is narrow (e.g. a `Cost` column with `$42,000`) was
> stretched hundreds of pixels wide with empty trailing whitespace
> whenever the viewport was much wider than the table's content.
> Browser/spreadsheet auto-fit tables size to content and leave spare
> space unused; FTWA now does the same in the surplus regime. The
> deficit and fallback regimes are unchanged (G3 still holds exactly
> in the deficit regime, `Σ w_j == W`).

### 3.3 Phase B — deficit regime (`W < Σ max_j`)
Let `D = Σ max_j − W` (total shrinkage required from the ideal widths).

**Step B1 — pick the wrap set S (minimize G2).** Sort columns by `slack_j`
descending; let `S` be the smallest prefix with `Σ_{j∈S} slack_j ≥ D`. Columns
not in `S` are pinned at `max_j` (they will not wrap). This is the exact
minimum-cardinality wrap set (§2.11).

*Edge case*: if `Σ_j slack_j < D` (even shrinking *every* column to its min
cannot reach `W`), fall back to §3.6 (give every column `min_j` and enable
horizontal scroll — done; G1/G2 are as good as physically possible).

**Step B2 — distribute the deficit across S to minimize G1 (water-filling on
extra lines).**
1. Initialize `w_j = max_j` for all `j ∈ S`.
2. Compute current `extraLines_j(w_j) = 0` for all.
3. Repeatedly "spend" a unit of deficit on the column in `S` that offers the
   smallest *marginal* increase in `extraLines` per unit of width removed — i.e.
   step down to the next breakpoint of the column whose `cost_j` at the next lower
   breakpoint introduces zero or the fewest new lines. This is greedy marginal
   cost minimization over the piecewise-constant `cost_j` curves (equivalently,
   a discrete water-filling that equalizes marginal cost).
4. Stop when `Σ_{j∈S} (max_j − w_j) = D`.

Because `cost_j` is monotone and piecewise-constant, the greedy "always take the
cheapest next breakpoint" rule is optimal for minimizing `Σ extraLines` subject
to a total-shrink budget over `S` (the breakpoints form a matroid-like greedy
structure: at each step the cheapest marginal step dominates). Implementation
uses a min-heap keyed by `Δcost / Δwidth`, O((B) log N) where `B` is the total
number of breakpoints.

**Step B3 — push leftover width (Priority D).** If after reaching exactly `D`
the algorithm holds at `Σ w_j = W`, nothing is left. If rounding/integer
arithmetic leaves a residue `r` (0 ≤ r < N), distribute it to `S` first (raise
the columns with the most extra lines, reducing G1 further), then to non-`S`.

### 3.4 Why this satisfies the lexicographic goal
* **G2** is minimized exactly by §3.3 Step B1 (§2.11 proof).
* **G1** is minimized *within* the lock-in of G2 by Step B3 greedy marginal
  allocation (optimal over the fixed wrap set `S`).
* **G3** holds by construction (`Σ w_j = W`).
* Remaining ties (same `{w_j}` from cost perspective) resolved by even surplus.

### 3.5 Surplus distribution rules (options for the fair tiebreak)
When spare pixels exist, candidates to assign `share`:
| Option | Rule | Pros / cons |
|---|---|---|
| Equal additive | `w_j += spare / N` | Maximally "fair" by count; can over-widen already-wide columns. |
| Proportional to `max_j` | `w_j += spare · max_j / Σmax` | Mirrors content weight; preserves whitespace proportions. |
| Proportional to `min_j` | safeguards narrow-token columns | Helps the *typographically* tight columns. |
| Proportional to `slack_j` | give spare where shrinkage flexibility exists | Hygiene; rarely best for readers. |

Recommended default: **proportional to `max_j`** so the visible whitespace
grows in proportion to the column's information density.

### 3.6 Degenerate fallback (`W < Σ min_j`)
Content is physically wider than the viewport even with every column at its
min-content floor. No algorithm can avoid scrolling; we:
1. Set `w_j = min_j` for all `j` (preserve `min-content` so words never break
   mid-token — the strongest invariant).
2. Wrap the grid in `ScrollArea::horizontal` exactly as the current code does.
G3 is intentionally violated (impossible); G1/G2 minimized given the physical
constraint.

### 3.7 Pseudocode

```
fn fair_column_widths(
    N: usize, W: f32,                 // columns, available width
    max: &[f32], min: &[f32],          // per-column max-content / min-content
    breakpoints: &[Vec<(f32, cost_i32)>], // per-column (width, extraLines) knees
    pad: f32,                         // per-cell padding (subtracted from W)
) -> Vec<f32> {
    let avail = W - N * pad;
    let total_max: f32 = max.iter().sum();

    // 3.2 surplus
    if avail >= total_max {
        return max.to_vec();  // pin at max_content; spare unused (see §3.2 revision note)
    }

    // 3.3 B1: choose wrap set S = smallest top-slack prefix with sum >= D
    let D = total_max - avail;
    let slack: Vec<f32> = (0..N).map(|j| max[j]-min[j]).collect();
    let mut order = (0..N).collect::<Vec<_>>();
    order.sort_by(|&a,&b| slack[b].partial_cmp(&slack[a]).unwrap());
    let mut S: Vec<usize> = Vec::new();
    let mut acc = 0.0;
    for &j in &order {                 // largest-slack first
        if acc >= D { break; }
        S.push(j); acc += slack[j];
    }
    if acc < D {                       // 3.6 fallback: cannot fit
        return min.to_vec();           // (caller enables horizontal scroll)
    }

    // 3.3 B2: water-fill the deficit across S using per-column breakpoints
    let mut w = max.to_vec();
    let mut rem = D;
    // min-heap of (marginal Δcost per Δwidth, col, next_bp_index)
    let mut heap = build_heap(S, breakpoints, &w, max, min);
    while rem > 0.0 {
        let step = heap.pop_min();
        if step.width_delta > rem { // clamp last step to exactly the remaining need
            w[step.col] -= rem;
            rem = 0.0;
        } else {
            w[step.col] = step.width_delta.abs();
            rem -= step.width_delta.abs();
            if has_next_bp(step.col) { heap.push(next_step(step.col)); }
        }
    }
    // 3.3 B3: ensure Σ w == W exactly (absorb FP/rounding residue into highest-Δcost col)
    normalize_sum(&mut w, avail);
    w
}
```

The inner `build_heap` / `next_step` walk each column's breakpoint list; a step
carries `{ width_delta = cur_w − next_bp_width, Δcost = next_bp_cost − cur_cost }`.
The heap is ordered by `Δcost` (cheapest line gained first); ties broken by
smaller `Δcost / Δwidth` to get the most shrinkage for the least wrap.

### 3.8 Complexity
* Precompute: `O(M)` where `M` is total cells, plus text shaping per token ≈
  `O(T)` tokens.
* Phase A: `O(N)`.
* Phase B1 (subset sum via greedy): `O(N log N)`.
* Phase B2 (marginal water-fill): `O(K log N)` where `K` is the number of
  breakpoint crossings touched (bounded by total breakpoints `B`, typically
  small for markdown tables).
* Overall: `O(N log N + B log N)`. Well within budget for a per-text-block UI
  pass; can be cached keyed on `(text, W)`.

---

## 4. Comparison matrix

| Algorithm | G1 total wrap | G2 wrap-cols | G3 uses space | Cost | Notes |
|---|---|---|---|---|---|
| Current code (`ScrollArea`+`Grid`, §2.5) | 0 (in-table) | 0 | **no** (scrolls) | O(N) | baseline we beat on G3 |
| `table-layout:auto` (§2.1/2.9) | heuristic | spread (bad for G2) | yes | O(N·px) | no wrap-col minimization |
| `table-layout:fixed` (§2.3) | unbounded | all | yes | O(N) | content-blind |
| Qt `Stretch` (§2.6) | large | all | yes | O(N) | ignores content |
| Knuth–Plass per-col (§2.7) | optimal (fixed w) | external | external | heavy | widths chosen elsewhere |
| LP surrogate (§2.8) | soft via `L1` | soft (λ-tuned) | yes | solver | overkill for UI |
| Pure water-filling (§2.10) | equalized | **maximized** | yes | O(B log N) | good distributor, bad selector |
| **FTWA (§3)** | optimal over S | **exact minimum** | yes | O(N log N + B log N) | recommended |

---

## 5. Resolved decisions (from review interview)

1. **Cost aggregation per column — Σ across all cells.** `extraLines_j =
   Σ_cell (cellLines_cell − 1)`; a column "wraps" iff *any* of its cells exceeds
   one line. Reflects total vertical space and aligns G2's binary wrap test with
   the same breakpoint tables used for G1.
2. **Word-break policy — never break tokens.** Strictest invariant: `min_j` =
   longest unbreakable token, period. Long URLs/code spans preserve at full
   width even if that triggers the §3.6 fallback (`W < Σ min_j` → pin `min_j`
   and horizontal-scroll). No soft-wrap classifier needed at measurement time;
   simpler and safer for code/URL legibility. Accepted cost: horizontal scroll
   will appear more often on tables with pathological long tokens.
3. **Padding/gutter accounting — probe-layout measurement.** Emit one
   throwaway grid row to read egui's used cell `Rect`, derive exact
   pad + gutter once, and subtract from `W` before sizing. Cacheable across
   frames until font/scale/theme changes. Most accurate; trades one probe pass
   for correctness.
4. **Interactivity — fully automatic, no override (v1).** FTWA decides all
   widths; no drag handles, no per-column persistence. Matches the current
   `ScrollArea` UX (no manual resize affordances already). User-driven override
   can be layered on later as a non-breaking enhancement (Q4 option B/C in the
   original survey) without touching the sizing core.
5. **egui enforcement — RESOLVED via API spike (egui 0.27.2).** Three candidate
   paths were probed against the actual `egui-0.27.2` source in the cargo
   registry. Findings:
   - **(b) `Grid::min_col_width`/`max_col_width` — REJECTED.** These setters
     (`egui-0.27.2/src/grid.rs:356,371`) write a single **scalar** field
     (`min_col_width: Option<f32>`, `max_cell_size: Vec2`) shared by all
     columns. The public `Grid` API exposes **no per-column width setter**.
     `State::set_min_col_width(col, width)` (`grid.rs:21`) is `pub(crate)`,
     reachable only from inside egui. Path (b) cannot pin distinct `w_j`.
   - **(a) wrap each cell in `allocate_ui_with_layout(width=w_j)` — ADOPTED.**
     `GridLayout::advance` (`grid.rs:215-218`) calls
     `curr_state.set_min_col_width(self.col, widget_rect.width().max(min_cell_size.x))`.
     Whatever width a cell actually allocates **becomes** the column's recorded
     width, and the cursor advances by `prev_col_width` (last frame's stored
     value) on subsequent frames. So calling `ui.allocate_ui_with_layout` (or
     `ui.set_width(w_j)` before rendering the cell content — see `ui.rs:577` and
     the precedent at `ui.rs:2155` where `Ui::columns` pins columns via
     `column_ui.set_width(column_width)`) forces the column to `w_j`.
   - **(c) hand-built `ScrollArea`+`ui::columns` — VIABLE FALLBACK.**
     `Ui::columns` (`ui.rs:2127-2155`) already constructs per-column child Uis
     pinned via `set_width(column_width)`. Path (c) works but discards
     `Grid::striped` / `Grid::end_row` / `with_row_color`; stripe logic would
     need re-implementation (trivial). Keep only as a fallback for
     pathological tables.
   **First-frame convergence caveat.** egui `Grid` reads `prev_state` to advance
   the cursor and sets `ui.set_visible(prev_state.is_some())` (`grid.rs:429`) —
   on the very first frame there is no `prev_state`, so the grid is rendered
   invisibly at default `min_cell_size` widths; from frame 2 onward the
   pinned `w_j` drives layout. This matches the current code's behavior and is
   not a regression. To make even frame-2 render exact, call FTWA in a pure
   measurement pass inside `render_table` before issuing any cells, and feed
   the resulting `w_j` into per-cell `set_width` calls on the first real
   (visible) frame.
   **Padding/gutter accounting tie-in (Q3).** `GridLayout::spacing`
   (`grid.rs:105`) defaults to `ui.spacing().item_spacing`; cell padding comes
   from any `Frame` we wrap cells in. The Q3 probe pass reads
   `ui.max_rect()` / `ui.cursor()` after one throwaway row and derives
   `pad+gutter` exactly; FTWA receives `W − N·(pad+gutter)` as available width.
6. **Frame stability — stable tie-break + cache by (text, W).** Heap keys
   break ties by column index (lexicographically stable); results are keyed on
   (content hash, W rounded to e.g. 0.1px) and reused across frames until text
   or width meaningfully changes. Repro exact widths frame-to-frame; avoids
   shimmer during resize.
7. **Surplus policy — pin at `max_content` (no stretching).
   SUPERSEDED**: ~~proportional to `max_content`, `spare_j = spare ·
   max_j / Σmax`, so visible whitespace grows in proportion to the
   column's information density.~~ The proportional rule produced the
   "infinite-width column" defect (see §3.2 revision note): a narrow
   content column stretched far beyond its content width in a wide
   viewport. The surplus regime now pins every column at its
   `max_content` and leaves the spare space unused to the right of
   the table, matching browser/spreadsheet auto-fit behaviour. G3
   ("use all space") is intentionally relaxed in the surplus regime;
   G3 still holds exactly in the deficit regime (`Σ w_j == W`).

### 5.1 No remaining open items
All seven decisions are resolved; the egui spike confirmed Path (a) as the
integration path. FTWA can be implemented against the parameters above. §6
reflects the chosen path.

---

## 6. Recommended next step

Implement FTWA in a pure module `src/desktop/src/ui/table_width/mod.rs` (per
AGENTS.md §6 modular + §7 documented), exposing:

```rust
/// Per-column width decision for a markdown/GFM table given available width.
pub fn column_widths(
    cells: &[Vec<&str>],            // row-major; rows may be ragged
    available_width: f32,
    font: &egui::FontDefinitions,
) -> Vec<f32>;
```

with unit tests (AGENTS.md §2/§3) covering: surplus regime, deficit/min-1-wrap,
deficit/multi-wrap water-fill, single-token column protection, `W < Σmin`
fallback, and deterministic stability.

**Wiring layer (Path (a) — egui 0.27.2 verified).** Modify `render_table`
(`src/desktop/src/ui/render.rs:215`) as follows:

1. Run FTWA's measurement pass up front (it needs `max_j`, `min_j`,
   `breakpoints_j` for every column — derived from the same
   `Vec<Vec<Vec<InlineElem>>>` already assembled
   at `render.rs:497-536`; measurement shapes glyphs through egui's
   `LayoutJob`/`Galley`).
2. Keep the existing `egui::ScrollArea::horizontal().show(...)` so the §3.6
   fallback (`W < Σ min_j`) still scrolls automatically when FTWA returns
   `min_j`-pinned widths that exceed `W`.
3. Inside `Grid::new(...).striped(true).spacing([10.0, 4.0])`, for each cell
   call `ui.allocate_ui_with_layout(Rect::from_min_size(cursor.min, vec2(w_j, desired_height)), Layout::top_down(Align::LEFT), |cell_ui| render_table_cell(cell_ui, ...))` — or
   equivalently `cell_ui.set_width(w_j)` before delegating to
   `render_table_cell` (`render.rs:167`). `GridLayout::advance`
   (`egui grid.rs:215-218`) records the allocated width as the column's min
   width, so on the next frame the cursor advances by exactly `w_j`,
   pinning the column to FTWA's decision.
4. Run the AGENTS.md §8 quality gate (`cargo check`, `cargo test`,
   `cargo clippy -- -D warnings`, `cargo fmt --check`,
   `cargo doc --no-deps --quiet`).

**First-frame notes.** egui's grid hides itself on the very first frame
(`grid.rs:429`, `ui.set_visible(prev_state.is_some())`); from frame 2 onward
the pinned `w_j` drives layout. This matches today's behavior (no regression).
If desired, an optional caching layer keyed on `(content_hash, available_W)`
(Decision 6) can feed frame-1 widths from a prior measurement pass, eliminating
even one-frame delay under reflow.

This document is research only — no production code is modified.