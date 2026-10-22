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

- [x] **INC-06** `state.rs` — interaction state machine. **Design diverged from
      this entry's original sketch, deliberately.** Rather than a flat six-value
      enum, the resolved state is two parts: `BaseState { Rest, Hovered, Pressed,
      Disabled, Invalid }` plus `VisualState { base, focused }`.
      Reason: the spec does not treat focus as mutually exclusive with the
      pointer states — §3.1 spells every Focused cell as "Rest **+** 2px focus
      ring", and §4.9.2's Focused menu row is `surface.hover` **plus** an inset
      outline, held simultaneously. A flat enum must pick one and contradict a row.
      Precedence for `base`: `Disabled > Invalid > Pressed > Hovered > Rest`,
      with `focused` resolved independently. Focus is a ring, not a rung, so the
      "does focus beat hover?" slot disappears rather than being ordered.
      Disabled dominance is total (it zeroes `focused` too — a dead control must
      not advertise a ring). `pressed` without `hovered` resolves to `Pressed`.
      `snapshot_label` maps to the spec's six names as a documented many-to-one
      projection; `Hovered + focused` reports `"Focused"` and is flagged as a
      seventh combination the spec never snapshots.
      Tests: full 32-row truth table, disabled dominance across all 16
      sub-combinations, focus orthogonality, purity.
      **Commit:** `feat(iced-pro): add interaction state machine`

### M3 — Atoms (UIF-ATOM-*)

- [x] **INC-07** `widget/button.rs` (`UIF-ATOM-001`) — pure `ButtonStyle` resolver
      over `(Variant × VisualState)` returning fill/fg/border/ring, plus the
      geometry constants INC-07b consumes. **Wrapper deferred to INC-07b** — see
      G-10, which changes what a wrapper even means.
      Tests: 21 new (122 total), including a 34-row contrast audit of every pair
      the resolver can produce. Three pairs fail; see G-8 and G-9.
      **Commit:** `feat(iced-pro): add button style resolver and contrast audit`
- [x] **INC-08** `status.danger` split + adaptive focus ring (clears **G-8**, **G-9**
      per decisions **D-5**, **D-6**).
      - New token `status_danger_fill = RGB(208, 23, 2)` — `L = 0.140271`,
        a `0.080678` step (≈36%) darker than `status_danger`, hue `6.12°` vs
        `6.17°` so it stays the same red. White-on-fill measures **5.5185:1**.
        `status_danger` is untouched and keeps the error text/icon role.
      - New `widget/focus.rs` — `resolve_ring_color(fill, palette)` returns
        `accent_primary` when it clears `MIN_CHROME_CONTRAST`, else `text_strong`;
        when neither clears, the higher-contrast one wins. A zero-alpha background
        short-circuits to the accent, so a Ghost ring is judged against its
        container rather than against `transparent`.
      - Button Danger Hovered/Pressed now fill with `status_danger_fill`, and the
        ring resolves against the actual background instead of per variant.
      - **Deviation from §3.1, deliberate:** the Danger *pressed border* also moves
        to `status_danger_fill`. Keeping the spec's `status_danger` edge over the
        85% fill measures **1.4240:1** — well under the 3.0 chrome floor — so
        honouring the literal value would have created a new violation. Measured
        across all seven surfaces so it cannot silently return.

      The three previously-failing pairs now clear: `3.8753 → 5.5185`,
      `1.1779 → 5.5185`, `1.0609 → 6.6216`. `accepted_failures()` is empty.
      Tests: 21 new (143 total).
      **Commit:** `feat(iced-pro): split danger fill and adapt focus ring`
- [ ] **INC-08b** `widget/text_input.rs` (`UIF-ATOM-002`) — rest / hovered /
      focused / invalid / disabled styling for text entry, search field and
      dropdown trigger; reserved trailing-icon slot so validation never shifts
      geometry.
      Tests: all 5 states, placeholder vs value colour rules, zero-shift invariant.
      **Commit:** `feat(iced-pro): add text input atom with validation states`

> **Plan revision R1 (post-G-10).** Decision **D-7** (custom focusable widgets)
> inserts three increments that the original plan did not have, because stock iced
> cannot give anything except text inputs a keyboard focus path:
> **INC-09** focus model (traversal order, roving focus, modal trap — pure state),
> **INC-10** the `operation::Focusable` widget plumbing,
> **INC-11** the iced button wrapper built on both.
> The original M4 onward shifts by three: anchor solver → **INC-12**,
> clamp solver → **INC-13**, and so on. The titles below are unchanged; only the
> numbers move.

### M3b — Focus infrastructure (decision D-7)

- [x] **INC-09** `focus.rs` — the focus model. Pure, zero iced imports, so the
      traversal rules are testable without a window and are not entangled with
      iced's router. `FocusId` / `GroupId` (owned), `FocusMember`, `FocusGroup`,
      `FocusState` with group registration, `focus` / `clear` / `is_focused`,
      `focus_next` / `focus_previous` (wrapping, skips disabled),
      `focus_first_in_group` / `focus_last_in_group`, `move_within(group, delta)`
      (the roving-focus primitive for menus), and a trap stack implementing
      `UIF-MOD-001`.
      Documented decisions:
      * **Unregister-while-focused** — slot-preserving fall-forward; the ordinal
        position survives a rebuild rather than resetting to the top, and the slot
        is computed inside the active scope so no structural change escapes a trap.
      * **Roving wraps, does not clamp** — the spec's verb for `UIF-CMENU-003` is
        "cycles"; clamping leaves the keyboard stuck on the bottom row.
      * **Disabled-while-focused** — moves forward, never stays. `state::resolve`
        zeroes the ring for a disabled control, so a retained focus would be
        invisible. Matches ARIA roving-tabindex.
      * **Focus outside the active trap** — rejected, returns `false`, state
        untouched. Every path (click, programmatic, Tab) passes the same gate, so
        there is no hole for INC-10 to fall through.
      Tests: 83 new.
      **Commit:** `feat(iced-pro): add focus model with trap and roving traversal`

- [x] **INC-10** `widget/focusable.rs` — **spike: D-7 is unblocked.** A custom
      widget can hold keyboard focus in iced 0.14.
      `FocusableState` implements `operation::Focusable`; `Widget::operate` calls
      `operation.focusable(id, bounds, state)`; `tag`/`state` pin the state into
      the `Tree` slot so focus survives a `view` rebuild. `focusable(content, id,
      enabled)` is the ergonomic entry point. The wrapper adds no layout level
      (nesting two reports identical bounds, proven).
      Disabled gating is **absence-based**: the router has no concept of
      "disabled", so a disabled wrapper is not registered and self-unfocuses each
      pass — otherwise it would keep a lit ring on a control the router can no
      longer see.
      **Four constraints that bind every later increment:**
      1. **`iced`'s `focus_next` does not wrap — it *loses* focus** at the ends.
         `FocusState::focus_next` wraps, as `UIF-CMENU-003` requires. So the
         bridge must compute the next id from the model and push it with
         `focus(id)`. **Never hand `Tab` to `operation::focus_next`.**
      2. **The bridge runs one way: model → router.** The router has nowhere to
         hold groups, traps or slot-preserving fall-forward, and reading focus
         *up* from the router would let anything registering itself outside the
         policy (a `text_input` inside a modal) move focus against a trap.
      3. **`UserInterface::operate` is single-pass.** `focus_next` is built as
         `then(count(), …)` whose `finish()` returns `Outcome::Chain`; a harness
         that does not loop on `Chain` runs only the `count` half and reports
         focus broken when the harness was.
      4. **`iced_test` cannot drive focus at all.** The harness is: headless
         renderer + real `UserInterface::build` + a carried
         `user_interface::Cache` (which owns the `Tree` where the state lives) +
         the operation chain loop + a `RecordFocusables` operation to read back
         ids and bounds. Later widget tests reuse this.
      **Feature deviation, accepted:** `iced` now carries the `advanced` feature.
      It is the only way to reach `Widget` / `Tree` / `operation::Focusable`
      through `iced` at all — without it `iced::advanced` is cfg'd out and
      `iced::widget::{core, renderer, graphics}` are empty shadow modules.
      Verified `advanced = []` in both `iced_core` and `iced_widget`, so it
      compiles in no code and no dependencies; it is a pure re-export switch.
      Tests: 28 new (254 total). Slowest tests in the crate (~4s, headless
      renderer per test) — acceptable.
      **Commit:** `feat(iced-pro): add focusable wrapper for custom widgets`

- [x] **INC-11** `widget/button_widget.rs` — the first widget that turns resolved
      style into pixels, composing every layer already built.
      Each frame: `state::resolve` → `button::resolve_style` → paint. Both pure
      layers were **sufficient for every colour and every state transition** —
      nothing had to be re-derived, which is the point of having built them first.
      `draw` is eight lines of composition containing no policy.
      **One genuinely new concept: `Layer { bounds, fill, border }`.**
      `ButtonStyle` answers *what* to paint but not *where the ring goes* — a
      focus ring must sit **outside** the fill while iced strokes a quad on its
      **inside**. Making "what did this state actually paint" a queryable,
      ordered value rather than a side effect is what makes the ring auditable.
      Menus and dialogs reuse this directly.
      Ring on a hot fill resolved end-to-end: the widget asks
      `widget::focus::resolve_ring_color(resolved_fill, palette)`, so a Danger
      button that is hovered *and* focused gets the `text.strong` ring (5.5185:1)
      rather than the invisible accent (1.1779:1) — D-6 working as intended in
      the real renderer, not just in the resolver test.
      **Tech debt this increment created, to be paid before the next widget:**
      1. **Test-harness duplication.** The focus/headless-renderer harness from
         `focusable_tests.rs` was copied into `button_widget_tests.rs` rather than
         shared. A third copy is due when the menu lands. **Extract a
         `test_helpers` module before INC-14** — three copies is the point where
         drift becomes a bug source.
      2. **The Tab bridge is inside `button_widget.rs`.** It is generic: model →
         `operation::focus(id)`, never `focus_next`. It belongs in a shared
         module, not in the first widget that needed it. Move it with the harness.
      Tests: 35 new (289 total).
      **Commit:** `feat(iced-pro): add focusable button widget`

### Process incidents — and a correction that changes the supervision rule

Two things went wrong around INC-08/INC-09. The second was **misdiagnosed at the
time and the diagnosis is corrected here**, because the wrong rule would have sent
future increments down a dead end.

1. **INC-08 shipped an orphan module.** `widget/focus.rs` and its sidecar were
   committed, but `widget/mod.rs` never got `pub mod focus;` — so in commit
   `579eb41` the module was dead code that never compiled. The working tree at
   verification time had the declaration and the tests passed, but the commit did
   not capture it. **Repaired in the INC-09 commit.** Supervision rule: after
   every increment, confirm the new module actually appears in
   `cargo nextest list`, not just that the suite is green.
2. **The "rogue subagent reformatting" diagnosis was WRONG.** INC-09 was recorded
   as having reformatted four files outside its allowed set with a pre-2024 style
   edition and then misreported the cause. It did not. Traced properly during
   INC-11: **the `edit` tool's file writes flip Rust import ordering from
   edition-2024 version-sort to the older case-insensitive sort, even for
   comment-only edits.** HEAD holds the correct 2024 ordering; every file an
   agent edited drifted. The subagents were reporting the symptom accurately and
   were blamed for their own tooling.
   * **Working rule that actually works:** after any `edit` to a Rust file, run
     `cargo fmt` (write) before checking. `cargo fmt` write is the correct
     normaliser and leaves HEAD's ordering intact — verified: after normalising,
     the diff against HEAD is the intended change only.
   * **Do not** "fix" drift by reverting files the increment legitimately
     touched; that discards real work. Revert only files the increment had no
     business changing.
   * **Do not** trust a subagent's account of *why* a gate failed — but equally,
     do not trust the supervisor's first theory either. Measure before assigning
     blame. Three increments were spent chasing this.
   * `cargo fmt --check` reports only the first dirty file per run in some
     invocations, which is what made the drift look like it was "coming back":
     fixing one file simply revealed the next.

---

### M4 — Geometry solvers (UIF-VER-002)

- [ ] **INC-12** `geometry/anchor.rs` — pure placement solver: cursor-proximal
      offset, horizontal/vertical quadrant flipping, 8px viewport margins,
      roomier-side scrollable fallback. Tests across extreme viewports incl.
      640×480 and sub-menu-card windows.
      **Commit:** `feat(iced-pro): add menu anchor and viewport flip solver`
- [ ] **INC-13** `geometry/clamp.rs` (`UIF-MOV-002`) — movable-overlay title-bar
      reachability clamps (≥120px title visible, y within
      [navbar, viewport − titlebar]) and resize clamps against min/max card size.
      Tests: boundary and out-of-range inputs, drag deltas that would lose the card.
      **Commit:** `feat(iced-pro): add movable overlay clamp solver`

### M5 — Menu engine (UIF-FMENU-*, UIF-CMENU-*)

- [ ] **INC-14** `surface/menu/model.rs` — menu tree data model: action rows,
      checkable toggles, radio choices, submenu parents, section headings,
      separators, disabled placeholders; per-row close-on-toggle policy
      (`UIF-FMENU-003`).
      **Precondition:** extract the shared focus/render `test_helpers` module and
      move the Tab bridge out of `button_widget.rs` (INC-11 tech debt).
      Tests: model construction, semantic predicates, group ordering.
      **Commit:** `feat(iced-pro): add menu tree data model`
- [ ] **INC-15** `surface/menu/view.rs` — menu card + row renderer implementing the
      §4.9.2 state matrix (rest / hovered / pressed / focused / submenu-parent /
      destructive-hover / disabled / heading) on the shared row grid.
      Tests: every row state resolves to the specified token triple.
      **Commit:** `feat(iced-pro): add menu card and row renderer`
- [ ] **INC-16** `surface/menu/tree.rs` — cascade controller: single-tree mutual
      exclusion, sibling replacement, parent stays open, keyboard navigation
      (`Up`/`Down` skip non-interactive rows, `Right` opens + focuses first child,
      `Left`/`Escape` unwinds to parent) (`UIF-CMENU-002/003`).
      Tests: full keyboard walk, deep nesting, disabled-row skipping, dismissal.
      **Commit:** `feat(iced-pro): add menu cascade and keyboard navigation`

### M6 — Layered surface hosts (UIF-ZORD-*)

- [ ] **INC-17** `surface/zorder.rs` — four z-band model and the strict top-down
      Escape unwinding order (`UIF-WIN-003`): deepest submenu → root menu →
      topmost dialog → search/filter.
      Tests: unwind sequence across mixed open layers, empty-stack no-op.
      **Commit:** `feat(iced-pro): add z-band model and escape unwinding`
- [ ] **INC-18** `surface/modal.rs` (`UIF-MOD-*`) — scrim, focus trap, card-level
      click absorption, dirty-dismiss guard (`UIF-MOD-002`), structured error
      banner (`UIF-MOD-003`), width/height policy clamps from §4.6.2.
      Tests: scrim click clean-vs-dirty, tab containment, error banner retention.
      **Commit:** `feat(iced-pro): add modal dialog host`
- [ ] **INC-19** `surface/input_dialog.rs` (`UIF-INP-*`) — 320px card anatomy,
      permanently reserved 20px validation slot, live validation hook, card-level
      click absorption, cursor-proximal placement + clamping, destructive parity.
      Tests: zero-shift across error states, absorption, placement clamps.
      **Commit:** `feat(iced-pro): add input dialog surface`
- [ ] **INC-20** `surface/movable.rs` (`UIF-MOV-*`) — title-bar drag state,
      clamp integration, z-promotion on click, non-blocking pass-through outside the
      card, respectful auto-scroll rule (`UIF-MOV-004`).
      Tests: drag deltas, promotion ordering, auto-scroll pause/resume matrix.
      **Commit:** `feat(iced-pro): add movable overlay surface`

### M7 — Verification and closeout

- [ ] **INC-21** `bin/gallery.rs` (`UIF-VER-004`) — deterministic gallery that
      renders every atom across `(Variant × State)` and every composite surface,
      plus snapshot-style assertions on resolved styles.
      **Commit:** `feat(iced-pro): add widget gallery and snapshot harness`
- [ ] **INC-22** Spec-compliance audit + docs: cross-check every `UIF-*` ID in the
      spec against the crate, record gaps here, update
      `doc/technical-context/ARCHITECTURE_C4.md` for the new crate boundary.
      **Commit:** `docs: record iced-pro crate boundary and spec coverage`

---

## 4. Known spec gaps / open questions

Recorded as they are found; none resolved yet.

- **G-1 — RESOLVED (INC-04).** Not a conflict: the two figures describe
  different pairings. `4.45:1` is `accent.primary` **vs `surface.base`**
  (computed `4.4540`); `4.46:1` is `text.strong` **on `accent.primary`**
  (computed `4.4669`, which rounds to **4.47** — the spec truncates).
  INC-05 must record both pairings separately and cite `4.47:1` for the
  foreground-lock rule.
- **G-5 — The spec's contrast column is not trustworthy.** INC-04 recomputed every
  row of §2.5 with WCAG 2.1 relative luminance. Most rows match within rounding,
  but several are materially wrong:

  | Token | Spec | Computed | Error |
  |---|---|---|---|
  | `surface.sunken` | `0.98:1` | `1.0176:1` | impossible — no WCAG ratio is < 1; the darker surface was put in the numerator |
  | `border.strong` | `3.92:1` | `4.5062:1` | `3.92` corresponds to grey ≈110, not 120 |
  | `accent.text` | `10.12:1` | `10.6559:1` | `+0.54` |
  | `text.primary` | `12.63:1` | `13.1590:1` | `+0.53` |
  | `status.warning` | `5.50:1` | `6.8555:1` | `+1.36` — largest error in the table |
  | `status.success` | `15.40:1` | `15.1867:1` | `-0.21` |
  | `border.subtle` | `1.36:1` | `1.3495:1` | rounds to `1.35` |
  | `text.disabled` | `5.86:1` | `5.9165:1` | `+0.06` |

  No single base luminance reproduces the column (`text.primary` implies
  `L_base ≈ 0.00499`, `accent.text` implies `≈ 0.00557`, `text.strong` implies
  `≈ 0.00287`), so the figures were hand-collected from mixed sources rather than
  generated. **INC-05 must recompute all rows and treat the computed values as
  authoritative**, and must not transcribe the spec's printed ratios into code.
- **G-6 — Live accessibility defect in `fastmd` (out of scope, must not be lost).**
  The app's current editor error colour `RGB(255, 0, 0)` on white measures
  **`3.9985:1`**, which fails the `UIF-TOK-006` 4.5:1 normal-text rule. The
  spec's `RGB(180, 0, 0)` measures `7.14:1`. `iced-pro` follows the spec per D-4;
  the app's value needs fixing when `fastmd` is wired onto this framework.
- **G-7 — BLOCKING: five mandated pairings cannot pass their own threshold.**
  INC-05 measured every pairing the spec's usage contracts require. Five fail.
  These are pinned by `test_contract_mandated_pairings_cannot_clear_their_rule`
  rather than waived, so the conflict stays visible:

  | # | Pairing | Measured | Rule | Short by | Where the spec requires it |
  |---|---|---|---|---|---|
  | 1 | `text.strong` on `accent.primary` | **4.4669** | 4.5 | 0.0331 | Selection Foreground Lock (`UIF-TOK-006` rule 3), Primary button, selected tree row |
  | 2 | `status.danger` on `surface.overlay` | 4.1473 | 4.5 | 0.3527 | Input-dialog validation slot (§4.5.1), modal error banner (§4.6.1) |
  | 3 | `status.danger` on `surface.raised` | 4.3800 | 4.5 | 0.1200 | Danger button resting foreground (§3.1) |
  | 4 | `accent.primary` on `surface.active` | 2.8441 | 3.0 | 0.1559 | Secondary button pressed border (§3.1) |
  | 5 | `border.strong` on `surface.active` | 2.8775 | 3.0 | 0.1225 | Ghost pressed border (§3.1), latched navbar trigger (§4.2.2) |

  Pairing 1 was independently re-derived outside the crate and confirmed
  (`L(accent.primary) = 0.185063`, ratio `4.4669`). The spec's own printed
  `4.46:1` agrees — the spec simply never checked it against its own rule 1.
  **Rule 1 and rule 3 are mutually unsatisfiable at the literal token values.**
  A one-unit retune clears it: `RGB(98, 101, 240)` → `4.5268`.

  Also measured under threshold but *not currently mandated* anywhere, so they are
  latent rather than active defects: `text.disabled` on `surface.hover` (4.4734)
  and `surface.active` (3.7781); `status.warning` on `surface.active` (4.3777);
  `status.danger` on `surface.active` (2.9740).

  **RESOLVED by INC-05b.** Retuned per the §5 decision. All five now clear
  their rule with at least `0.05` headroom:

  | # | Pairing | Was | Now | Rule |
  |---|---|---|---|---|
  | 1 | `text.strong` on `accent.primary` | 4.4669 | **4.5648** | 4.5 |
  | 2 | `status.danger` on `surface.overlay` | 4.1473 | **4.5718** | 4.5 |
  | 3 | `status.danger` on `surface.raised` | 4.3800 | **4.8283** | 4.5 |
  | 4 | `accent.primary` on `surface.active` | 2.8441 | **3.0750** | 3.0 |
  | 5 | `border.strong` on `surface.active` | 2.8775 | **3.1793** | 3.0 |

  New values: `accent_primary` `RGB(98, 100, 240)`, `surface_active`
  `RGB(43, 43, 51)`, `status_danger` `RGB(255, 28, 2)`. Total channel
  movement 49 units across three tokens; accent hue moves `+0.42°`.
  The elevation ladder survives, but the `hover → active` gap narrows from
  `0.0128` to `0.0050` — that is forced by the constraint set, not chosen.
- **G-8 — `status.danger` is one token asked to do two mutually exclusive jobs.**
  Found by the INC-07 audit and independently re-derived. The spec uses a single
  red for both:

  | Role | Requirement | Implied luminance |
  |---|---|---|
  | Background behind `text.strong` (Danger button hover, §3.1) | white-on-red ≥ 4.5 | `L <= 0.183333` |
  | Text/icon colour on dark surfaces (validation errors, §3.2, §4.5.1) | red-on-overlay ≥ 4.5 | `L >= 0.216697` |

  The bounds are **disjoint** — no value of `status.danger` satisfies both. This
  is not a rounding problem and cannot be retuned away; the spec conflates two
  roles that need opposite luminance. Measured at the current retuned
  `RGB(255, 28, 2)`: white-on-danger is **3.8753:1**, short by `0.6247`.
- **G-9 — The keyboard focus ring is invisible on hot fills.**
  Also from the INC-07 audit. Because INC-06 made focus additive, a focused
  button can also be hovered or pressed, which drops the `accent.primary` ring
  onto the button's own fill:

  | Combination | Ring vs fill | Needs 3.0 | Short by |
  |---|---|---|---|
  | Danger hovered + focused | `accent.primary` on `status.danger` | 3.0 | **1.8221** |
  | Danger pressed + focused | `accent.primary` on `status.danger` @85% | 3.0 | **1.9391** |

  A keyboard user tabbing onto a destructive button gets a ring they cannot see,
  at the moment they most need to know what is selected. Note the same colour pair
  *passes* as a border (3.8753 ≥ 3.0) and *fails* as a ring — the ring fails for
  its own reason. Ghost escapes entirely because its ring sits on the surface,
  never on a fill.
- **G-10 — BLOCKING: stock iced 0.14 widgets cannot receive keyboard focus.**
  Verified against the vendored `iced_widget-0.14.2` source. Across the entire
  crate, **only `text_input` and `text_editor` implement `operation::Focusable`**
  (`text_input.rs:691,1554`, `text_editor.rs:571,1097`). `Button`'s `State` is
  just `{ is_pressed: bool }` and its `operate` never calls
  `operation.focusable(...)`, so a button is never registered with the focus
  router. Same for checkbox, radio, scrollable, menu rows — nothing.

  Three further constraints confirmed:
  * **No automatic Tab traversal exists.** `focus_next` / `focus_previous` appear
    only as runtime definitions; nothing in `iced_widget` calls them.
  * **No widget-level focus event** — only window-scoped
    `window::Event::Focused` / `Unfocused`.
  * **`iced_test` 0.14 has zero focus support**, so focus cannot be driven in
    the tier-2 simulator.

  This collides head-on with `UIF-PRIN-005` ("every interactive operation
  reachable by pointer MUST have a deterministic keyboard path") and with
  `UIF-MOD-001` (strict focus trap) and `UIF-CMENU-003` (arrow-key menu
  navigation). None of those are achievable with stock iced widgets.
  **RESOLVED by decision D-7, proven by INC-10.** Custom widgets can hold
  keyboard focus in iced 0.14; the `Focusable` wrapper works, is testable
  headlessly, and bridges cleanly to the pure `FocusState` model. The four
  constraints listed in the INC-10 ledger entry are the standing rules that
  fall out of that proof.

---

## 5. Resolved decisions

### D-5 — `status.danger` split into two tokens (resolves G-8)
User decision 2026-10-21. `status.danger` stays the light red used for error
**text and icons** on dark surfaces. A new `status.danger_fill` is the darker red
used **as a background behind `text.strong`**. Each clears 4.5:1 in its own role.
The disjointness is a property of the colour, not of the implementation — one red
cannot serve both.

### D-6 — Adaptive focus ring (resolves G-9)
The ring colour is resolved against the **actual resolved fill** with the contrast
solver: `accent.primary` if it clears `MIN_CHROME_CONTRAST`, otherwise
`text.strong`. A ring that cannot be seen is not an accessibility feature, so the
ring adapts rather than the spec's colour being honoured into invisibility.

### D-7 — Custom focusable widgets (resolves G-10)
`iced-pro` implements `operation::Focusable` on its own widgets and owns
`Tab` / `Shift+Tab` traversal, roving focus for menus and the modal focus trap.
Stock iced widgets are not used for anything that must be keyboard-reachable.

---

## 5b. Original decision record — the G-7 retune

The spec's literal token values cannot satisfy the spec's own contrast rules in
five places. Every downstream widget increment builds on these values, so the
choice must be made before M3.

| Option | What it means | Cost |
|---|---|---|
| **A — Retune the offending tokens minimally** | Nudge `accent.primary`, `status.danger` and the `surface.active`/border relationship until all five pairings clear their rule. E.g. `accent.primary` → `RGB(98, 101, 240)` gives `4.5268`. | Changes brand values slightly; visually near-indistinguishable. Keeps the spec's rules intact and the gate strict. |
| **B — Keep the literal values, record accepted deviations** | Tokens stay exactly as the spec prints them. Each shortfall becomes a named, test-pinned documented exception. | The `UIF-VER-001` gate ships knowingly below WCAG AA. Honest, but the accessibility rule is soft. |
| **C — Escalate the spec for redesign** | Stop at M2 and send the five conflicts back to the spec author. | Blocks the plan on an external dependency. |

**Decision (user, 2026-10-21): Option A — retune minimally, fix only the five
active failures.** The spec will be revised to match at a later date; until then
the retuned values are authoritative in code and the deviation is recorded here.
Latent pairings are deliberately left alone — pre-emptively darkening
`surface.active` further would flatten the elevation ladder.

### Constraint analysis driving the retune

`accent.primary` **cannot** absorb both failure 1 and failure 4. Solving each for
`L(accent.primary)`:

- Failure 1 (white on accent ≥ 4.5) requires `L(acc) <= 0.183333` — the accent
  must get **darker**.
- Failure 4 (accent on `surface.active` ≥ 3.0, with `surface.active` at its
  literal value) requires `L(acc) >= 0.197944` — the accent must get
  **lighter**.

The lower bound exceeds the upper bound, so no value of `accent.primary`
satisfies both. The retune therefore splits across three tokens:

| Token | Direction | Fixes |
|---|---|---|
| `accent.primary` | darken until white-on-accent clears 4.5 | #1 |
| `surface.active` | darken until accent-on-active and `border.strong`-on-active clear 3.0 | #4, #5 |
| `status.danger` | lighten until it clears 4.5 on `surface.overlay` (the worst of its two backgrounds) | #2, #3 |

Hard guardrails on the retune: the elevation ladder must survive (`sunken < base <
raised < code < overlay < hover < active`), surfaces and borders must stay
neutral greys, and total channel movement must be minimal with headroom above
each threshold rather than sitting knife-edge on it. Option A keeps
`UIF-VER-001` a real gate rather than a documented exception, and the visual
difference is not perceptible.
- **G-2** The spec defines `size.hit_target.min` as `24 × 24` but navbar icon
  buttons are given a `28 × 28` floor in `UIF-ATOM-001`. Both hold; the atom rule
  is the stricter one and should win for navbar-class controls.
- **G-3** `UIF-VER-004` asks for "deterministic offscreen visual snapshots".
  iced has no first-class snapshot API; INC-18 should assert on resolved style
  values rather than pixels unless a workable harness is found.
- **G-4** The spec prints line-height multipliers rounded to two decimals, so a
  rung's derived ratio only matches the printed figure within a tolerance. The
  tightest case is `type.ui_compact`: `18 / 13 = 1.3846…`, which is `0.0046`
  from the printed `1.38×`. INC-03 pins the ratio at `5e-3`. Any future line-height
  retune must stay inside that band or the multiplier test must be re-derived.
