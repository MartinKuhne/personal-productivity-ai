# iced-pro — Implementation Task Ledger

> **Status:** In progress — increment ledger for implementing
> [`ui-framework.md`](ui-framework.md) as a new workspace crate.
> **Date:** 2026-10-21
> **Spec:** [`doc/planning/ui-framework.md`](ui-framework.md) (UIF-* requirement IDs)
> **Supervisor:** OpenCode agent session (orchestrates one subagent per increment)

---

## 1. Confirmed decisions

| # | Decision | Outcome |
|---|---|---|
| D-1 | Crate location & name | `src/iced-pro/`, package `iced-pro`, lib name `iced_pro` |
| D-2 | Scope | **Framework layer only.** Design tokens, atoms, menu engine, dialog/overlay hosts, z-order bands, geometry + contrast solvers. App-specific content (navbar slot contents, tree rows, tab strip contents, Batch/Tools dialogs) stays in `fastmd`. |
| D-3 | App integration | **Standalone first.** `fastmd`'s `src/app/ui_iced/` and `src/app/ui/` are NOT modified in this effort. Wiring is a later, separate project. |
| D-4 | Token source of truth | `iced-pro` owns the `UIF-TOK-005` palette exactly as specified. `src/app/ui/theme.rs` is left untouched. |

### Hard constraints for every increment

- **Do not touch** `src/app/**`, `src/agent/**`, `src/md2pdf/**`, or any `SPEC.md`.
  New code lives under `src/iced-pro/` only. (Exception: the root `Cargo.toml`
  `[workspace] members` list, touched once in INC-01.)
- **No raw literals outside the token module.** Per `UIF-PRIN-001` / `UIF-VER-001`,
  widgets read spacing, sizing, radius, stroke, type and colour from `token::*`.
  A numeric literal in a widget file is a spec violation.
- **Pure where possible.** Style resolvers, geometry solvers and state machines are
  pure functions returning values. Rendering wrappers stay thin. This is what makes
  the `UIF-VER-*` gates testable without a window.
- **Every increment ends green** on the crate: `cargo check -p iced-pro`,
  `cargo nextest run -p iced-pro`, `cargo clippy -p iced-pro -- -D warnings`,
  `cargo fmt --check`.
- **Tests are mandatory and exhaustive** (`RUST-003`): happy path, corner cases,
  failure modes, every branch. Sidecar convention: `<file>_tests.rs`, declared with
  `#[cfg(test)] #[path = "<file>_tests.rs"] mod <name>_tests;`.
- **Docs mandatory** (`RUST-010`, `RUST-011`): `//!` on every module, `///` on
  every `pub` item. Cite the spec ID (`UIF-TOK-001`, `UIF-CMENU-001`, …) in the
  doc comment of the item implementing it.
- **Read before writing:** `doc/planning/ui-framework.md` (the spec),
  `AGENTS.md` (root), `doc/distill/iced.md` (iced conventions).

---

## 2. Target crate layout

```
src/iced-pro/
├── Cargo.toml                 # package iced-pro, iced 0.14
├── mod.rs                     # facade only — pub use of subsystem APIs (RUST-054)
├── token/                     # UIF-TOK-001..006 — the closed token set
│   ├── mod.rs
│   ├── space.rs               # spacing scale (base-4 grid)
│   ├── size.rs                # control heights, hit targets, icon sizes
│   ├── shape.rs               # corner radii, stroke weights
│   ├── typography.rs          # type scale: size / line-height / weight / tracking
│   ├── color.rs               # semantic colour tokens + scrim + editor-inverted set
│   └── contrast.rs            # luminance + WCAG ratio solver, mandated pairings
├── state.rs                   # InteractionState machine + priority resolution
├── geometry/                  # pure boundary solvers (UIF-VER-002)
│   ├── mod.rs
│   ├── anchor.rs              # quadrant flipping, cursor-proximal placement, clamping
│   └── clamp.rs               # movable-overlay reachability clamps
├── widget/                    # atoms (UIF-ATOM-*)
│   ├── mod.rs
│   ├── button.rs              # 4 variants × 6 states
│   └── text_input.rs          # input / search / dropdown trigger, 5 states
├── surface/                   # composite framework surfaces
│   ├── mod.rs
│   ├── zorder.rs              # UIF-ZORD bands + Escape unwinding (UIF-WIN-003)
│   ├── menu/                  # unified fixed + context menu engine
│   │   ├── mod.rs
│   │   ├── model.rs           # rows, sections, separators, submenus, toggle semantics
│   │   ├── view.rs            # menu card + row state matrix (§4.9.2)
│   │   └── tree.rs            # cascade, mutual exclusion, keyboard navigation
│   ├── modal.rs               # scrim, focus trap, dirty guard, error banner
│   ├── input_dialog.rs        # zero-shift validation slot, click absorption
│   └── movable.rs             # drag, clamp, z-promotion, pass-through, auto-scroll
└── bin/
    └── gallery.rs             # UIF-VER-004 visual snapshot harness
```

---

## 3. Increment ledger

Status legend: `[ ]` queued · `[~]` in flight · `[x]` committed · `[!]` blocked

### M0 — Scaffold

- [x] **INC-01** Crate skeleton. `src/iced-pro/Cargo.toml` (package `iced-pro`,
      edition 2024, `iced` 0.14 with the same feature set `fastmd` uses), `mod.rs`
      facade with a module doc comment, added to root `[workspace] members`.
      Gate: `cargo check -p iced-pro` green.
      **Commit:** `chore(iced-pro): scaffold iced-pro workspace crate`

### M1 — Design tokens (UIF-TOK-*)

- [x] **INC-02** Spatial + geometry tokens: `token/space.rs` (`UIF-TOK-001`),
      `token/size.rs` (`UIF-TOK-002`), `token/shape.rs` (`UIF-TOK-003`).
      Tests assert every value against the spec tables and the 24×24 hit-target floor.
      **Commit:** `feat(iced-pro): add spatial, sizing and shape tokens`
- [x] **INC-03** Typography scale: `token/typography.rs` (`UIF-TOK-004`) —
      size, line height, weight, tracking per rung; 12px absolute floor.
      **Commit:** `feat(iced-pro): add typography scale tokens`
- [x] **INC-04** Semantic colour tokens: `token/color.rs` (`UIF-TOK-005`) —
      surfaces incl. `scrim` alpha, borders, accents, text roles, status colours,
      and the `editor.inverted.*` set.
      **Commit:** `feat(iced-pro): add semantic colour tokens`
- [x] **INC-05** Contrast solver: `token/contrast.rs` (`UIF-TOK-006`, `UIF-VER-001`)
      — relative luminance, contrast ratio, and a checked table of every mandated
      text/surface and border/surface pairing.
      **Commit:** `feat(iced-pro): add WCAG contrast solver and token invariants`

### M2 — Interaction state machine

- [x] **INC-06** `state.rs` — `InteractionState { Rest, Hovered, Pressed, Focused,
      Disabled, Invalid }` plus a pure priority resolver (`Disabled` > `Invalid` >
      `Pressed` > `Focused` > `Hovered` > `Rest`) and transition helpers.
      Tests: every precedence combination, no-implicit-transition rules.
      **Commit:** `feat(iced-pro): add interaction state machine`

### M3 — Atoms (UIF-ATOM-*)

- [ ] **INC-07** `widget/button.rs` (`UIF-ATOM-001`) — pure `ButtonStyle` resolver
      over `(Variant × InteractionState)` returning fill/fg/border/ring, plus the
      iced widget wrapper. Tests: all 24 cells of the matrix, icon-only 28×28 floor,
      selection foreground lock on `accent.primary`.
      **Commit:** `feat(iced-pro): add button atom with full state matrix`
- [ ] **INC-08** `widget/text_input.rs` (`UIF-ATOM-002`) — rest / hovered / focused /
      invalid / disabled styling for text entry, search field and dropdown trigger;
      reserved trailing-icon slot so validation never shifts geometry.
      Tests: all 5 states, placeholder vs value colour rules, zero-shift invariant.
      **Commit:** `feat(iced-pro): add text input atom with validation states`

### M4 — Geometry solvers (UIF-VER-002)

- [ ] **INC-09** `geometry/anchor.rs` — pure placement solver: cursor-proximal
      offset, horizontal/vertical quadrant flipping, 8px viewport margins,
      roomier-side scrollable fallback. Tests across extreme viewports incl.
      640×480 and sub-menu-card windows.
      **Commit:** `feat(iced-pro): add menu anchor and viewport flip solver`
- [ ] **INC-10** `geometry/clamp.rs` (`UIF-MOV-002`) — movable-overlay title-bar
      reachability clamps (≥120px title visible, y within
      [navbar, viewport − titlebar]) and resize clamps against min/max card size.
      Tests: boundary and out-of-range inputs, drag deltas that would lose the card.
      **Commit:** `feat(iced-pro): add movable overlay clamp solver`

### M5 — Menu engine (UIF-FMENU-*, UIF-CMENU-*)

- [ ] **INC-11** `surface/menu/model.rs` — menu tree data model: action rows,
      checkable toggles, radio choices, submenu parents, section headings,
      separators, disabled placeholders; per-row close-on-toggle policy
      (`UIF-FMENU-003`).
      Tests: model construction, semantic predicates, group ordering.
      **Commit:** `feat(iced-pro): add menu tree data model`
- [ ] **INC-12** `surface/menu/view.rs` — menu card + row renderer implementing the
      §4.9.2 state matrix (rest / hovered / pressed / focused / submenu-parent /
      destructive-hover / disabled / heading) on the shared row grid.
      Tests: every row state resolves to the specified token triple.
      **Commit:** `feat(iced-pro): add menu card and row renderer`
- [ ] **INC-13** `surface/menu/tree.rs` — cascade controller: single-tree mutual
      exclusion, sibling replacement, parent stays open, keyboard navigation
      (`Up`/`Down` skip non-interactive rows, `Right` opens + focuses first child,
      `Left`/`Escape` unwinds to parent) (`UIF-CMENU-002/003`).
      Tests: full keyboard walk, deep nesting, disabled-row skipping, dismissal.
      **Commit:** `feat(iced-pro): add menu cascade and keyboard navigation`

### M6 — Layered surface hosts (UIF-ZORD-*)

- [ ] **INC-14** `surface/zorder.rs` — four z-band model and the strict top-down
      Escape unwinding order (`UIF-WIN-003`): deepest submenu → root menu →
      topmost dialog → search/filter.
      Tests: unwind sequence across mixed open layers, empty-stack no-op.
      **Commit:** `feat(iced-pro): add z-band model and escape unwinding`
- [ ] **INC-15** `surface/modal.rs` (`UIF-MOD-*`) — scrim, focus trap, card-level
      click absorption, dirty-dismiss guard (`UIF-MOD-002`), structured error
      banner (`UIF-MOD-003`), width/height policy clamps from §4.6.2.
      Tests: scrim click clean-vs-dirty, tab containment, error banner retention.
      **Commit:** `feat(iced-pro): add modal dialog host`
- [ ] **INC-16** `surface/input_dialog.rs` (`UIF-INP-*`) — 320px card anatomy,
      permanently reserved 20px validation slot, live validation hook, card-level
      click absorption, cursor-proximal placement + clamping, destructive parity.
      Tests: zero-shift across error states, absorption, placement clamps.
      **Commit:** `feat(iced-pro): add input dialog surface`
- [ ] **INC-17** `surface/movable.rs` (`UIF-MOV-*`) — title-bar drag state,
      clamp integration, z-promotion on click, non-blocking pass-through outside the
      card, respectful auto-scroll rule (`UIF-MOV-004`).
      Tests: drag deltas, promotion ordering, auto-scroll pause/resume matrix.
      **Commit:** `feat(iced-pro): add movable overlay surface`

### M7 — Verification and closeout

- [ ] **INC-18** `bin/gallery.rs` (`UIF-VER-004`) — deterministic gallery that
      renders every atom across `(Variant × State)` and every composite surface,
      plus snapshot-style assertions on resolved styles.
      **Commit:** `feat(iced-pro): add widget gallery and snapshot harness`
- [ ] **INC-19** Spec-compliance audit + docs: cross-check every `UIF-*` ID in the
      spec against the crate, record gaps here, update
      `doc/technical-context/ARCHITECTURE_C4.md` for the new crate boundary.
      **Commit:** `docs: record iced-pro crate boundary and spec coverage`

---

## 4. Known spec gaps / open questions

Recorded as they are found; none resolved yet.

- **G-1** `UIF-TOK-005` lists `color.accent.primary` contrast as `4.45:1` in the
  table but the `color.text.strong` row cites `4.46:1` for the same pairing.
  INC-05 must compute the actual ratio and record which figure is authoritative.
- **G-2** The spec defines `size.hit_target.min` as `24 × 24` but navbar icon
  buttons are given a `28 × 28` floor in `UIF-ATOM-001`. Both hold; the atom rule
  is the stricter one and should win for navbar-class controls.
- **G-3** `UIF-VER-004` asks for "deterministic offscreen visual snapshots".
  iced has no first-class snapshot API; INC-18 should assert on resolved style
  values rather than pixels unless a workable harness is found.
