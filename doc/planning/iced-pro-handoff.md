# iced-pro — D-2 Boundary Review Hand-off (Composite Widget-Binding Pass)

> **Status:** Hand-off — verified work queue for the next coding agent.
> Nothing in the queue below is implemented yet: this document is the
> review's output, not the pass. Every citation was re-verified against the
> tree at commit `9318c26` on 2026-10-26.
> **Spec:** [`src/iced-pro/spec.md`](../../src/iced-pro/spec.md) — amended
> this session (`82c7cde`); needs **no further edits** for this pass. Never
> modify a `SPEC.md` without explicit instruction (`RUST-043`).
> **Ledger:** [`iced-pro-tasks.md`](iced-pro-tasks.md) — decisions D-1..D-8
> (D-8 at `### D-8`). This pass records **D-9** (§1 below) when it starts.
> **Architecture:** [`ARCHITECTURE_C4.md`](../technical-context/ARCHITECTURE_C4.md)
> — corrected this session (`9318c26`).
> **Read first:** root `AGENTS.md`, `doc/distill/iced.md` (iced widget
> conventions), spec §5 `spec.md:1578` ("Presentation vs. Lifecycle Ownership
> Split"), `surface/movable.rs:1337` (the pattern to copy) and
> `surface/modal.rs:1064` (the pattern **not** to copy).

---

## 1. The decision this hand-off executes (record as D-9)

The D-2 split — framework owns tokens, atoms, menu engine, dialog hosts,
z-order, geometry; the app owns its content — is right in principle. The
review found the boundary failing in three places, and the seven `NotOwned`
rows in `spec_coverage.rs` are now half factually wrong:

1. **Host-facing APIs self-register and self-trap.** `ModalHost::open`
   (`surface/modal.rs:1064`) registers into the shared registry *and*
   `push_trap`s focus (`:1075`) from inside the open call — two writers to
   the surface index where spec §5 (`spec.md:1578`) allows one.
   **D-9 guardrail:** absorbed widgets follow the `MovableHost::open` shape
   (`surface/movable.rs:1337`) — the caller injects `&mut SurfaceRegistry`;
   the framework never self-registers. Applies to every queue item.
2. **D-8 decided row rendering, but the composites stayed resolvers.**
   `composite/tab_strip.rs`, `composite/tree_view.rs` and
   `composite/navbar.rs` exist and ship models — `TabStripModel` with MRU
   history and `TabCloseVerdict` dirty guards, id-anchored tree selection,
   `NavBarMenuTrigger` — none produces an `Element`. The remaining work is
   exactly the widget-binding pass D-8's consequence #3 already mandates.
3. **Framework gaps forced app inventions.** Three verified examples:
   `MENU_CARD_WIDTH = 224..320` sits in the spec's §2.2 closed table
   (`spec.md:113`, `spec.md:1472`) but is missing from the
   `SURFACE_GEOMETRY_CONSTANTS` registry (`token_lint.rs:325`), and the app
   invented its own `CARD_WIDTH` (`src/app/ui_iced/about.rs` et al.);
   `ValidationRule::Name` via `check_basename`
   (`surface/input_dialog.rs:464`) accepts `..`; a `Length::Fixed(0.0)`-sized
   region voids its children, so composites mounted under one need the
   mount-safety clause §3B.16 now states.

**Verdict, adopted:** composite-enhancement pass (Option A) with the
MovableHost guardrail — absorb the widget shells of the `NotOwned` surfaces
into `iced-pro` as composites that bind the existing models. The app keeps
its content, its policy decisions and all disk I/O.

### Where the seven `NotOwned` rows land (target end-state)

| Spec ID | § | Today's `NotOwned` reason | Fact check | Target |
|---|---|---|---|---|
| `UIF-NAV-002` | 4.2.2 | "renders no navbar label … contributes no text-formatting helper" | **false** — `microcopy/action_label.rs` ships `ASCII_ACTION_ELLIPSIS` / `check_action_ellipsis` (:52); `NavBarMenuTrigger` renders labels | **Implemented** (Q5) |
| `UIF-TAB-001` | 4.4.3 | "no tab-strip module exists" | **false** — `composite/tab_strip.rs` exists; the `Element` binding is what is missing | **Implemented** (Q4) |
| `UIF-TAB-002` | 4.4.3 | "never given … no framework geometry to satisfy or test" | **false once Q4 lands** — the overflow container is the queue item | **Implemented** (Q4) |
| `UIF-PANE-001` | 4.3.2 | "no splitter or divider module exists" | true *today*; §3B.16 (`UIF-COMP-016`) now specifies it | **Implemented** (Q1) |
| `UIF-WIN-001` | 4.1.2 | geometry + persistence belong to the host | right, incomplete — spec now assigns the schema and `solve_window_layout` to the framework | **Partial**, host residual: disk I/O, engine resize commands (Q3) |
| `UIF-PANE-002` | 4.3.2 | "owns no pane width policy at all" | right in code, countermanded — spec §4.3.2 now makes the pure solvers framework-owned | **Partial**, host residual: TOC mount decision, persistence (Q2) |
| `UIF-PANE-003` | 4.3.2 | tree content app-owned | right — content stays app-side; the framework gains the pinned-footer mount option | **Partial**, host residual: tree content, ordering, filtering (Q6) |

`EXPECTED_SPEC_ID_COUNT` stays **92**. `Partial` rows must name exactly what
is missing (`spec_coverage_tests.rs` enforces a gap marker); `NotOwned` rows
must cite `D-2` — none of the seven should remain `NotOwned` once the pass
lands.

---

## 2. Work queue (dependency-ordered; failing test first per `RUST-005`)

**Q0 — clippy is red before you start.** Two pre-existing
`unnecessary_map_or` lints in `token_lint_tests.rs:1321` and `:1350` —
`.map_or(false, |c| c.is_ascii_digit() && c != '0')` becomes
`.is_some_and(...)`. Behavior-identical, one line each. Fix first so
`-D warnings` can gate every increment after it.

**Q1 — `SplitDivider` composite (`UIF-COMP-016`, spec §3B.16).** New module
`composite/split_divider.rs` + sidecar tests. Model per §3B.16: `SplitAxis`,
`SplitSnapshot`, `SplitDividerModel { axis, ratio, min_ratio, max_ratio,
disabled }`; callbacks `on_resize_change(Live<f32>)` per move (coalesced),
`on_resize_commit(Commit<f32>)` exactly once on release-in-zone,
`on_resize_cancel(Cancel<SplitSnapshot>)` on release-outside and on
`Escape` via `binding/pre_rank.rs` (Pre-Rank, `UIF-WIN-003`); clamp from the
last clamped position per the `surface::movable` drag contract; an
`ActivationGate` on press. Paint from tokens only: resting `border.subtle`
`1px` stroke, `2px × 24px` grip pill (`space.2xs × size.hit_target.min`,
`radius.pill`) in `border.strong`, promotion to `accent.border` through
`VisualState`, `10px` grab zone (`size.splitter.grab`), resize cursor as a
property of the grab zone. Extend `gallery.rs` — §6 (`spec.md:1594`)
already demands `SplitDivider` snapshot quads. **MovableHost guardrail
applies.** Coverage: `UIF-COMP-016` Partial → Implemented; `UIF-PANE-001`
→ Implemented.

**Q2 — pane-width solvers (`UIF-PANE-002`, spec §4.3.2).** Pure functions in
`geometry/`: `solve_clamped_pane_width(viewport_w, requested_w, min_px,
max_px)` and `auto_fit_pane_width(labels, font_size, icon_w, padding, min_w,
max_w)`. Constants from §2.2 (`LEFT_PANE_MIN_W = 150`,
`RIGHT_PANE_MIN_W = 120`); the `20% viewport` cap arrives pre-multiplied
from the host. Coverage: `UIF-PANE-002` → Partial naming the host residual.

**Q3 — window geometry core (`UIF-WIN-001`, spec §4.1.2).** `WindowGeometry`,
`UiScale` (`0.75×..=2.00×` in `0.10×` steps, clamped construction, OS-DPI
composition at the render boundary only), `solve_window_layout` enforcing
`WINDOW_MIN = 640 × 480` + `CENTER_PANE_MIN_W = 240`, and a `WindowStorage`
trait the host implements — the framework never names a storage location
(`RUST-024`). Coverage: `UIF-WIN-001` → Partial naming the host residual.

**Q4 — tab-strip widget binding (`UIF-TAB-001`/`002`, spec §4.4.3).** Bind
the existing `composite/tab_strip.rs` `TabStripModel` to an `Element`: tab
row in a horizontal `scrollable` honoring `TAB_WIDTH = 96..220` (§2.2) with
the `24 × 24px` close target preserved; per-tab `mouse_area` middle-click;
a `TabContextMenuModel` export mounting through the §4.9 menu engine.
Coverage: `UIF-TAB-001`, `UIF-TAB-002` → Implemented.

**Q5 — navbar label + ellipsis gate (`UIF-NAV-002`, spec §4.2.2).** Route
`NavBarMenuTrigger` label text (including the dialog-launching ellipsis
suffix) through `microcopy/action_label.rs::check_action_ellipsis`, and
extend the `UIF-VER-005` gate so the convention is enforced, not merely
available. Coverage: `UIF-NAV-002` → Implemented.

**Q6 — tree search-footer mount option (`UIF-PANE-003`, spec §4.3.2).**
`TreeView::with_search_footer` composite mount option on
`composite/tree_view.rs`: the `40px` band (`32px` input + `2 × space.xs`)
from tokens, pinned so tree rows end at a clear boundary. Coverage:
`UIF-PANE-003` → Partial naming the app residual.

**Q7 — ledger + docs close-out.** Append a `### D-9` record (§1 above,
house style of the D-5..D-8 sections, after `### D-8`) to
`iced-pro-tasks.md`; retire the four stale `NotOwned` reasons as their items
land; keep `ARCHITECTURE_C4.md` ledger prose pointing at the manifest — the
84 / 1 / 7 tallies live in `spec_coverage.rs`, not the prose.

---

## 3. Carried gaps (verified this session; fold into the pass)

- **`MENU_CARD_WIDTH` unpublished** — add to `SURFACE_GEOMETRY_CONSTANTS`
  (`token_lint.rs:325`) with the §2.2 range `224..320`; the app-side
  `CARD_WIDTH` (`src/app/ui_iced/about.rs` et al.) migrates onto it in a
  later, app-side increment.
- **`check_basename` accepts `..`** (`surface/input_dialog.rs:464`) — write
  the failing test first (`RUST-005`), then reject `..` path components.
- **`Length::Fixed(0.0)` mount-safety** — §3B.16 promises the divider is safe
  inside a zero-sized region; Q1's tests must prove it, not assert it.

---

## 4. Gates (every increment ends green)

`cargo fmt --check` · `cargo check --workspace --all-targets` ·
`cargo nextest run --workspace` (8463 passing on this tree; the
`spec_coverage_tests.rs` audit is the tripwire — it reads
`CARGO_MANIFEST_DIR/spec.md` at test time and fails loudly on any spec /
ledger drift) · `cargo clippy --workspace --all-targets -- -D warnings` (red
*only* on the two Q0 lints today) · `cargo doc --workspace --no-deps`. New
code under `src/iced-pro/` only; tests in `<file>_tests.rs` sidecars;
`//!`/`///` on every module and `pub` item, citing the `UIF-*` ID each one
implements (`RUST-010`/`RUST-011`, `RUST-040`).