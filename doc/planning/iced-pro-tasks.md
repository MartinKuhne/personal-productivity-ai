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
- [x] **INC-08b** `widget/text_input.rs` (`UIF-ATOM-002`) — resolver + widget for
      text entry, search field and dropdown trigger.
      **One matrix, three kinds.** `resolve_input_style` returns the identical
      `InputStyle` for all three at every reachable state — asserted directly.
      The kind selects **slot content only**, never colour.
      Derived cells where §3.2 is silent: Focused placeholder `text.muted`,
      Focused icon `text.primary`, Invalid placeholder `text.muted`, and
      `Pressed` folds to the Hovered base.
      Slot precedence is `invalid > clear > idle`, with reserved widths from
      tokens (24 for entry/dropdown, 28 for search).
      **Contrast audit: zero judged failures, empty accepted-failure ledger.**
      Tightest margin in the atom is the Selection Foreground Lock at
      `text.strong`/`accent.primary` = **4.5648** (+0.0648) — the G-7 retune
      earning its keep in a second atom. Two `border.subtle` rows are classified
      `RestingDivider` and reported rather than gated, with a test pinning that
      classification so it cannot go stale silently.
      **Zero-shift invariant is guaranteed by construction, not by convention:**
      `text_area_bounds(bounds, kind)` and `trailing_slot_bounds(bounds, kind)`
      are functions of `(outer bounds, kind)` **alone** — there is no parameter
      for what is in the slot. That absence *is* the invariant. Iced's own
      `text_input` icon slot cannot do this; its icon is glyph-measured, so
      `🔍 → ×` moves the text. Hence the wrapper. Asserted at three levels:
      pure (five slot contents, identical rect), partition (no overlap, no
      leftover), and **real iced layout** — the editor's registered bounds are
      read back through the headless harness for three glyphs of three different
      widths and come out byte-identical.
      **Invalid + focused: invalid wins the border, focus relocates to a 2px
      `accent.primary` ring drawn outside the control.** Rationale: `state::resolve`
      already ranks Invalid above every pointer state precisely so clicking at an
      error does not visually clear it — letting focus take the border would undo
      that ruling at the one place the two collide; and a user who cannot see a
      field is invalid cannot fix it, while a user who cannot tell which of two
      invalid fields is theirs can still tab. Neither signal is lost: the ring is
      resolved through `resolve_ring_color_for_background` (4.3585 over
      `surface.base`). When focused and *not* invalid no ring is drawn — the 2px
      accent border **is** the indicator.
      **Divergence from the INC-06 model worth naming: focus is NOT additive for
      inputs.** §3.2 writes its Focused cell as a *replacement* (background
      `code → base`, border `→ 2px accent`), unlike §3.1 where it is
      "Rest **+** ring". Pinned by a test so an input cannot be quietly "fixed"
      back into button semantics.
      Three real defects were found and fixed during the increment, not test
      noise: a pressed-and-focused input lost its focus cell; disabled-focus
      eviction ran after `operation.traverse` so a read-only `is_focused`
      recorded focus before the same pass removed it; and a fixture omitted
      `.invalid()` so a "passive slot" was actually the actionable clear.
      Tests: 71 new (744 total).
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

### M3c — Pay the INC-11 tech debt before it triples

- [x] **INC-11b** Extract the shared focus/render **test harness** and move the
      **Tab bridge** out of `button_widget.rs` into a shared module.
      Two copies of the harness exist today (`focusable_tests.rs`,
      `button_widget_tests.rs`); the menu increment needs a third, and three
      copies of a nontrivial async render/operate loop is where drift becomes a
      bug source rather than an annoyance.
      Deliverables:
      * a `#[cfg(test)]`-gated shared harness (or `test_helpers` module) exposing
        the headless-renderer + `UserInterface::build` + carried `Cache` +
        `Outcome::Chain` loop + focus read-back as one callable API;
      * the Tab/Shift-Tab bridge as a named public function in a non-test module
        (it is production code: model → `operation::focus(id)`, never
        `focus_next`), reachable by every widget and later by `fastmd`;
      * both existing test files rewritten against the shared API with **no
        reduction in coverage** — the test count must not drop.
      **Commit:** `refactor(iced-pro): share focus test harness and tab bridge`

---

### M4 — Geometry solvers (UIF-VER-002)

- [x] **INC-12** `geometry/anchor.rs` — pure placement solver covering all three
      anchor kinds the spec defines: `Pointer` (`UIF-CMENU-001`),
      `Control` (`UIF-FMENU-001/002`) and `CursorProximal` (`UIF-INP-001`).
      `Placement` carries the **reasons**, not just the origin — flip flags per
      axis, `scrollable`, per-axis clamp flags and `visible_height`. A card
      clamped to the left margin and a card deliberately flipped left of the
      cursor land at the same `x` and need different downstream handling;
      throwing that away would force the submenu cascade to re-derive it.
      Every spacing number is a token alias (`VIEWPORT_MARGIN = space::SM`,
      `POINTER_OFFSET = space::XXS`, `TRIGGER_GAP = space::XS`,
      `SUBMENU_GAP = space::XXS`) — no literals.
      Viewport is a `Rectangle`, not a `Size`, so a content area that does not
      start at `(0,0)` still solves under `UIF-WIN-002`.
      Verified boundaries match the spec formula exactly: card 200×150 in
      1000×700 flips at `x = 791` (last non-flip 790) and `y = 541`
      (last non-flip 540), with the flipped origin landing at `x − w − 2`.
      Notable decisions:
      * **Tie on roominess → preferred side, no flip**, honouring the spec's
        stated "Right and Below" preference.
      * `Center`/`Middle` alignments return **no mirror** — a centred card clamps
        rather than reporting a fake flip.
      * `scrollable` is derived from resolved geometry, not from the decision, so
        a card the clamp pulled fully inside correctly reports *not* scrollable.
      * **No `NaN` or infinity ever leaves the solver** — sanitised at the
        boundary and tested, including `f32::MAX` cards and zero/negative
        viewports.
      Tests: 50 new (356 total), including a 285-point grid sweep at `640×480`
      and a property sweep across 8 viewports × 5 cards × ~214 anchors × 8 anchor
      kinds asserting the inside-the-margin invariant never fails.
      **Commit:** `feat(iced-pro): add menu anchor and viewport flip solver`
- [x] **INC-13** `geometry/clamp.rs` (`UIF-MOV-002`) — pure reachability solver.
      **The clamp protects the title bar, not the card.** The title bar is the
      only grip the user has to bring a card back; a card whose 32px bar is
      off-screen cannot be grabbed, resized or closed and is functionally gone.
      The card *body* may legitimately hang off the right or bottom — content
      scrolls. A fit-the-card rule would also be arithmetically impossible (a
      680×440 card cannot exist in a 640×480 window) and backwards: it would
      let the card's bottom edge decide where its top edge goes.
      Boundaries verified to the pixel: `x ∈ [viewport.x + 120 − w, viewport.x +
      width − 120]`, `y ∈ [viewport.y + 36, viewport.y + height − 32]` — card
      height never enters the vertical bound.
      **The same `required_visible = min(120, card_width, viewport_width)` feeds
      both the clamp and `is_reachable`**, so the invariant degrades identically
      from both sides rather than the clamp and the assertion disagreeing.
      Inverted ranges pin to the **lower** bound — the maximal-visibility choice.
      **Overshoot is a contract on the caller, made un-avoidable:** clamp from
      the last clamped position, never from drag-start plus a running total.
      `drag_position(current, delta, …)` implements exactly that step and
      clamps its `current` input on the way in, so the guarantee holds even if
      the caller lost track. A blocked frame is detectable — `returned !=
      requested` means that axis absorbed the delta.
      Priority when constraints collide: **invariant > size-policy floor > held
      edge.** The held edge shifts only when the other two forbid holding it.
      `resize` applies the size policy only to the axis being dragged; the
      untouched axis is left alone — clamping it too would resize an edge nobody
      is holding.
      Token mapping: 36 → `size::CONTROL_LG`, 32 → `size::CONTROL_MD` (both
      existing); 120 / 420 / 260 are new named constants — `token::size`
      describes control and icon rungs, not a reachability floor or a whole
      surface size, and each constant's doc says so.
      Degenerate cases all defined: narrow card → clamped fully visible; tiny
      viewport → pinned to cover it; viewport shorter than navbar + title bar →
      pinned to the top bound ("never above the navbar" is the harder half, so
      it is the one kept); non-finite max treated as unbounded not zero; NaN/∞
      sanitised because iced would propagate it into collapsed layout.
      Tests: 68 new (1086 total), including an invariant sweep across card
      sizes × viewports down to 640×480 and all eight resize handles.
      **Commit:** `feat(iced-pro): add movable overlay clamp solver`

### M5 — Menu engine (UIF-FMENU-*, UIF-CMENU-*)

- [x] **INC-14** `surface/menu/model.rs` — pure menu tree data model.
      `RowKind { Action, Check{checked}, Radio{group,selected}, Submenu{child},
      Heading, Separator }`. Two spec vocabulary items are deliberately **not**
      kinds: *Disabled/placeholder* is `enabled = false` on an interactive kind
      (a greyed-out `Delete…` is still a destructive action — label, slot,
      command identity and close policy all survive), and *destructive* is an
      orthogonal flag so a destructive checkable is expressible.
      **Model/cascade boundary: the model owns structure and selection, never
      openness.** No `is_open` exists. Sibling-replacement (`UIF-FMENU-002`) is a
      rule *across* siblings that a child cannot see; opening a child must not
      mutate a value that is cloned and rebuilt every frame; dismissal is
      tree-wide. The cascade (INC-16) owns openness; the model hands it
      `child_id()` handles.
      `activate(index) -> Option<Activation { row, effect, closes_menu }>` with
      `ActivationEffect ∈ { Command, Toggled, Selected{cleared}, OpenSubmenu,
      Absorbed }` — the cascade never re-reads the world to learn what happened.
      **Three nested predicates, three names:** `interactive()` (kind can carry a
      command → pointer cursor), `activatable()` (interactive ∧ enabled → click /
      Enter), `navigable()` (Up/Down may stop here). They coincide today and are
      kept apart so the renderer never asks traversal's question.
      `focus_members()` **excludes** non-navigable rows rather than registering
      them disabled — `FocusState` keeps disabled members in the list and steps
      over them, which would count unfocusable furniture in the group length and
      corrupt wrap distance and slot-preserving fall-forward.
      `ClosePolicy` is **stored per row, not derived**, because `UIF-FMENU-003`
      itself makes the checkable configurable. Radio re-selection is a **no-op on
      the selection** — the `•` marks a setting that always has a value, so a
      click cannot leave the group empty.
      `Menu::new` **rebases** submenu child ids to `<parent path>/<row key>`
      recursively, so identity is positional and stable across rebuilds.
      `validate()` catches `MissingKey`, `DuplicateKey`, `DestructiveNotLast`,
      `MixedGroup`, `RadioOverSelected`.
      Tests: 95 new (451 total), including the four §4.9.1 fixtures with exact
      navigable index vectors and a `FocusState` walk-agreement pin so the model
      and the focus model cannot drift.
      **Commit:** `feat(iced-pro): add menu tree data model`
- [x] **INC-15** `surface/menu/view.rs` — row style resolver + rendering,
      focusable row widget.
      `RowBase` covers the eight printed rows of §4.9.2 **plus
      `DestructivePressed`**, the matrix's one blind spot, filled from §3.1's
      destructive-button rule. `RowState { base, focused }` mirrors
      `state::VisualState` so focus stays additive: 8 spec cells + 3 derived
      combinations = 12.
      `derive_row_state` precedence: `!interactive` → Heading · `!activatable` →
      Disabled · Pressed → (Destructive)Pressed · `!attended` → Rest ·
      destructive → DestructiveHovered · open submenu parent →
      ActiveSubmenuParent · else Hovered, where `attended = hovered || focused`.
      **A focused row is never `Rest`.**
      **All six G-11 traps defused and measured**, with a 36-pair contrast audit
      and an **empty** accepted-failure ledger — nothing waived:
      destructive fill is `status_danger_fill` (the literal `status_danger`
      would sit at 1.4240); the ring always goes through
      `resolve_ring_color_for_background` (5.5185 on danger, 3.2954 on a normal
      row); disabled rows paint **no layer at all**, so there is nothing to
      inherit a hovered ancestor's fill; destructive cells carry no accent
      glyph (`accent.text` on the fill would be 2.9557).
      `Layer` from `button_widget.rs` was **reusable as-is** — no move, no churn.
      Behaviour decisions:
      * **Activation moved to release** — press captures the event and paints the
        pressed skin; the release must still land inside the row to fire.
        Walking off cancels. Heading/separator/disabled release as `Absorbed`
        only, which is what makes `UIF-CMENU-002`'s "absorbed without dismissing"
        true.
      * `rest_leading` follows the **glyph, not the slot type** — an unchecked box
        draws no glyph so it resolves `text_muted`, matching the spec's
        "`accent.text` (`✓`/`•`) **or** `text.muted`".
      * `cursor_for` consults the base as well as `activatable()`, so a row
        resolved `Disabled`/`SectionHeading` can never light a pointer cursor.
      **Two token gaps surfaced (G-12):** `token::typography` has no 12px
      **SemiBold** rung (`CAPTION` is Regular/Medium), so the heading is
      `CAPTION.size` + `Weight::Semibold`; and there is no token for the spec's
      10px row padding or 9px separator height — both are expressed as token
      compositions (`space::SM + space::XXS`, `space::SM + shape::BORDER_HAIRLINE`).
      Tests: 64 new (515 total).
      **Commit:** `feat(iced-pro): add menu card and row renderer`
- [x] **INC-16** `surface/menu/cascade.rs` — the controller that owns
      **openness**, the one thing INC-14 deliberately refused.
      An ordered stack of `OpenCard`s, root first. Each card carries its `Menu`,
      the `SubmenuTrigger` (parent card + row index it hangs from — exactly what
      the anchor solver needs) and the row focus sits on. The stack **is** the
      open path, which buys return-focus for free: each card's `FocusState` trap
      recorded the focus held when it opened — the parent's submenu row — so
      `pop_trap` lands the keyboard back on exactly where the user came from at
      every level.
      Three spec rules cannot be stated from inside a row, which is why the model
      was right to refuse `is_open`: mutual exclusion spans trees, sibling
      replacement is a rule across siblings, and dismissal is tree-wide.
      **Trap scoping:** each open card registers its navigable rows as its own
      group and pushes one trap; only the topmost governs, so `Tab` cycles inside
      the deepest card and cannot reach the background. Teardown order is **pop
      trap, then unregister group**, so recorded focus is restored while the
      card's own members can no longer be landed on. `stabilise` re-asserts the
      invariant after every input.
      `Tab` is deliberately absent from `MenuKey` — the host drives it through
      `bridge::tab_task` and the trap confines it. Iced's `focus_next` is never
      used.
      **Deviation worth recording:** the cascade owns **cloned** models.
      `model.rs` exposes no `row_mut`/`kind_mut`, so no consumer can get
      `&mut Menu` for a nested card — and `Enter` on a checkable three levels
      down requires exactly that. Rather than edit the forbidden file or leave
      the rule unreachable, the cascade clones the opened tree in. Contract is
      documented: `sync()` re-adopts the caller's tree and **overwrites** local
      changes, so the reported `Activation` must be applied to caller state
      first. The report is the durable record; the cascade's copy is the display.
      Edge cases decided (each tested):
      * a card with **nothing focusable does not open** — its trap would confine an
        empty scope while focus sat outside on a live control, painting a menu
        over a focus ring that is no longer theirs;
      * `Right` on a non-submenu row does nothing; onto an all-disabled submenu
        is refused **before anything closes**, leaving the parent branch intact;
      * interacting with an ancestor card collapses the branch below it — the
        pointer left that branch and the trap stack cannot focus two cards at
        once. Exception: the row that is the open branch's own trigger is left
        alone rather than closed and reopened under the cursor;
      * a row disabled while focused moves focus forward **in place** via
        slot-preserving fall-forward; nothing focusable left closes the card to
        its trigger;
      * closing the whole tree restores the focus the menu took; if nothing was
        focused before, focus clears rather than being invented;
      * re-opening the same tree is refused (`AlreadyOpen`) so a caller that
        re-issues `open` every frame cannot make the menu flicker.
      **Seam to INC-17:** `Escape` on the deepest card closes that card; on the
      root it closes the tree and leaves `is_open() == false`. A second `Escape`
      reports `IgnoredReason::NotOpen` — the cascade has consumed the menu level
      and is telling the band model its turn has passed. The cascade never
      reaches into a modal; the refusal is how the key propagates upward.
      Tests: 76 new (591 crate / 3752 workspace, 6 skipped).
      **Commit:** `feat(iced-pro): add menu cascade and keyboard navigation`

### M6 — Layered surface hosts (UIF-ZORD-*)

- [x] **INC-17** `surface/zorder.rs` — the §5 band table turned into queryable
      data. Nothing renders, nothing knows what a widget is.
      `ZBand` answers the four §5 questions about itself so no host re-derives
      the table: `backdrop()`, `pointer_rule()`, `blocks_lower_bands()`,
      `promotes_on_click()`, `escape_rank()`.
      `SurfaceRegistry` keeps open surfaces sorted by band (highest last) and
      within a band bottom-first, which makes `topmost()` a last-element lookup.
      Re-registration refreshes geometry but **keeps stacking position** — a card
      that rebuilds every frame must not silently restack, and a Band 1 card the
      user promoted stays promoted.
      `route_pointer(point)` is pure and walks bands top-down, stopping at the
      first band with something open. The three bands genuinely differ on a
      **miss**: Band 3 returns `Scrim` and the walk stops (that is what "blocks
      completely" means operationally); Band 2 returns `DismissBand` — the
      workspace underneath does **not** get the click; Band 1 records
      `passed_through()` and keeps walking down.
      **Escape is delegation, not orchestration.** `escape(closure)` walks
      surfaces in `escape_rank` order and asks each through the caller's
      closure; the first that accepts ends the walk, refusals are recorded, and
      the model never reaches into a menu or a modal. The refusal vocabulary is
      the cascade's own, adapted not paralleled: `NotOpen` → "my level is
      spent, the key keeps travelling"; any other refusal → `CannotUnwind`.
      Registering **each open menu card as its own Band 2 surface** is what makes
      `UIF-WIN-003`'s "deepest submenu → root menu" fall out of the generic
      reverse walk rather than needing menu-specific code.
      **Escape rank is deliberately NOT band rank** — see **G-13**.
      Edge cases decided:
      * a surface with **no regions is never hit by the pointer** — it still
        registers, stacks and takes part in Escape, so a full-window modal must
        be given the viewport rect or every click lands on its own scrim;
      * hit testing follows iced's convention: right and bottom edges are
        *outside*, so two abutting cards do not overlap; NaN hits nothing;
      * **the Band 3 scrim covers Band 2 as well as Bands 0/1** — the spec names
        only 0/1, but the scrim is full-window, so a menu open under a modal is
        pointer-unreachable while Escape still unwinds it first;
      * Band 1 promotion is the **only** automatic reordering — clicking a lower
        modal does not raise it, because inventing stacked-modal promotion would
        change what "topmost dialog" means without the host choosing it;
      * Band 1's "Escape does not close unless focused" is not special-cased —
        the surface answers `Declined(NotFocused)` and the key moves on, with
        `key_disposition` telling the host the condition exists;
      * `SCRIM_ALPHA` is a named constant and `scrim_for()` is the only place it
        meets the token, asserted equal to `Palette::surface_scrim` exactly.
      Tests: 82 new (673 total), including three grid sweeps across click
      positions and a real `MenuCascade` drive through the `from_cascade_close`
      seam.
      **Commit:** `feat(iced-pro): add z-band model and escape unwinding`
- [x] **INC-18** `surface/modal.rs` — the modal **host**: scrim, trap,
      click-absorption boundary, dirty guard and error banner. Content is supplied
      by the caller; the host is what makes the three `UIF-MOD` invariants
      unbreakable rather than merely intended.
      **Each invariant is structural, not a check:**
      * `UIF-MOD-001` — `open()` calls `push_trap` on the card's own group
        **before** moving focus. Containment is a property of `active_scope()`,
        so `Tab`, `Shift+Tab`, click-to-focus and programmatic focus all pass the
        same gate. The trap names the **group, not its members**, so
        `sync_members` cannot widen it.
      * `UIF-MOD-002` — a private `attempt_dismiss()` is the **only** path from
        a user gesture to a close. Escape, scrim and the header Close control all
        funnel through it, so the dirty guard exists in exactly one place.
        `close()` is the programmatic bypass — the guard protects the user from
        themselves, not the app from its own intent.
      * `UIF-MOD-003` — the error slot's mutation set is `set_error`,
        `clear_error` and the banner's own `Acknowledge`. Nothing else writes it,
        so "clicking elsewhere must not dismiss the error" is a **non-event
        rather than a rule**.
      **Absorption mechanism:** `on_pointer` tests the card rectangle **first**
      and the scrim branch is the `else`, so no interior arm can reach it. Inside
      the card, `Padding`/`Disabled` → `Absorbed`; `Control(id)` is **validated
      rather than trusted** — group membership is checked and `focus()` must
      succeed, else it is downgraded to `Absorbed { Disabled }`. Geometry
      outranks the caller's report in both directions: a point outside the card
      takes the scrim path even if the caller claimed `Padding`, and a point
      inside can never be reported as scrim.
      **Dirty guard × z-order:** `unwind_answer()` returns `Dismissed` (clean) /
      `Declined(CannotUnwind)` (dirty) / `Declined(NotOpen)` (closed) and
      **never mutates the registry** — `SurfaceRegistry::escape` removes by index,
      so unregistering from inside the closure would shift the list under it.
      Teardown order is load-bearing: pop trap → unregister focus group, with the
      surface unregister outside the closure. A dirty modal therefore holds its z
      slot, its trap and its focus; draining the menu band above it cannot close
      it. **An error does not block dismissal — dirt does.**
      Sizing is a real policy model (`Desired`/`Cap`/`AxisPolicy`) with the five
      §4.6.2 presets (`editor`, `tools_and_integrations`, `batch_processing`,
      `about`, `attributions`) resolved against the viewport.
      Tests: 114 new (858 total) — sizing 17, registration 10, Tab containment
      11, Escape/dirty 13, scrim 6, card absorption 12, error banner 19, nested
      modals 8, teardown/determinism 18.
      **Commit:** `feat(iced-pro): add modal dialog host`
- [x] **INC-19** `surface/input_dialog.rs` — Band-3 surface with the §4.5.1
      anatomy and all four `UIF-INP` invariants enforced structurally.
      **Every dimension traced to a token**: card padding `space::LG`, title row
      `typography::TITLE_SM.line_height`, gaps `space::SM`, text entry
      `size::CONTROL_MD`, folder row `size::CONTROL_SM`, validation slot
      `typography::BODY.line_height` (20px), footer `size::CONTROL_MD`, border
      `shape::BORDER_HAIRLINE`. Card height is derived, not declared: text-entry
      = **192px** exactly, confirmation = 160px, folder list =
      `160 + clamp(rows×28, 28, 200)`. Row offsets asserted literally.
      **Zero-shift is structural, same trick as INC-08b:** the footer's bounds
      are computed from the card rect and the anatomy **without consulting
      whether a validation message exists**. Asserted at the whole-footer level,
      not per-button — with no error, with an error, and mid-clear, `Cancel` and
      `Ok` are byte-identical.
      **`Enter`-while-invalid keeps focus in the box:** the key is routed to
      *reveal*, not *submit*. It writes the specific validation message,
      leaves `Ok` disabled, and never calls the accept path nor moves focus —
      so the user can fix the input without reaching for the mouse.
      **`UIF-INP-001` is the shared solver, not a local clamp.**
      `Anchor::CursorProximal` gives `+2,+2` from the trigger, the 8px margin
      clamp, and viewport-centring when `keyboard_invoked`. The host *could not*
      write its own clamp even if it wanted to — same code path the menus use.
      **Escape composes with G-13 rather than knowing about menus.** The host
      answers `Dismissed` (an input dialog has no unsaved-work concept, so it
      is always clean enough to close); because `escape_rank` for the Menu band
      is `3` against this band's `2`, a menu open above is asked **first** and
      closes before the dialog is touched. The registry's walk order does it.
      **Destructive parity (`UIF-INP-004`)** is a first-class variant, not a
      string difference: single-delete names the item, multi-delete names the
      count, both primary-styled `Danger`, both require confirmation — so the
      plural routing §4.9.1 demands has somewhere to land.
      Validation is an **injected rule**, so the surface carries no filesystem
      policy; the `Rename` rule set the spec names (non-empty, legal basename,
      distinct from current) ships as a named validator.
      Tests: 160 new (1018 crate / 4179 workspace, 6 skipped).
      **Commit:** `feat(iced-pro): add input dialog surface`
- [x] **INC-20** `surface/movable.rs` — Band-1 movable, non-blocking overlay host.
      **Each `UIF-MOV` invariant is structural, not a check:**
      * `UIF-MOV-001` — `zone_at` returns `Zone::TitleDrag` **only** for the
        title bar *minus* the trailing action cluster, and that exclusion is
        exactly as wide as the controls that must not start a drag — no wider.
        A `TitleBar` report on the Close rectangle does **not** drag, and a
        `Close` report on the title text does **not** close: geometry outranks
        the caller's report in both directions, mirroring `modal.rs`. Every
        drag frame goes through `clamp::drag_position`, so no-accumulation
        lives in the solver — this module holds no clamp of its own and could
        not bypass it.
      * `UIF-MOV-002` — **unreachable geometry is unrepresentable.** Exactly
        one private route to a new geometry (`apply_geometry`), which clamps
        through `clamp::clamp_rect` and `debug_assert!(is_reachable)`. Every
        public mutator funnels through it, so an unreachable card cannot be
        constructed, only *requested* — and the request comes back adjusted as
        `Reclamped { adjusted }`. `CARD_MIN_*` are **read from**
        `clamp::SizePolicy::MOVABLE.min` rather than restated, so host and
        solver cannot disagree about the floor.
      * `UIF-MOV-003` — routing is decided **first** by `registry.route_pointer`;
        only a route naming this surface reaches zone classification, so a click
        outside the card can never be classified "inside" by a mistaken caller.
        Promotion is gated on `ZBand::promotes_on_click()`. Mid-gesture the
        routing check is deliberately skipped so a fast pointer leaving the card
        does not drop the drag — **capture, not containment**, owns the gesture.
      * `UIF-MOV-004` — **following is a state machine, not a checkbox.**
        `FollowState::resolve(setting, at_bottom)` is a pure `const` function of
        exactly the two inputs the invariant names; only `Enabled × at_bottom`
        follows. The host's stored state is only ever that function's output, so
        the pause/resume matrix is the whole behaviour. `AT_BOTTOM_TOLERANCE` =
        `space::XXS` so a view settling 0.4px short does not pause a live stream
        over sub-pixel rounding. Entries arriving while paused are counted in
        `behind` with a `HoldReason` — `Disabled` wants a checkbox, `Inspecting`
        wants a "jump to latest" button, and those are different affordances.
      **Persistence honours D-2 / RUST-024:** the host never names a filesystem
      path. A source scan for `std::fs`, `std::env`, `APPDATA`, `USERPROFILE`,
      `dirs::`, `home_dir`, `File::`, `PathBuf` returns **zero hits**; writes go
      exclusively through the injected `&mut dyn GeometryStorage`. Writes happen
      only at *settled* geometry — drag release, resize release, close,
      programmatic set, viewport re-clamp — never per frame, asserted by counting
      sink writes across a multi-frame drag.
      **API reduction found by test-writing:** `MovableNoOp::KeyConsumedElsewhere`
      was removed as unproducible — `escape_via` answers `Declined(NotOpen)` for
      every other surface, so nothing above the card can consume the key *on the
      card's behalf*. Replaced by a `debug_assert!` documenting the
      broken-contract case rather than shipping a variant no test can reach.
      Tests: 116 new (1202 total), grouped by invariant, including the full
      auto-scroll matrix, all eight resize handles, and the G-13 Escape ordering.
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
- **G-11 — §4.9.2's menu row matrix contains six more contrast traps.**
  Recomputed by INC-14 against the **retuned** palette, WCAG 2.1. Recorded here
  so INC-15 does not rediscover them one build at a time.
  1. **Destructive-hovered row** — `text.strong` on `status.danger` =
     **3.8753** vs 4.5. Use `status.danger_fill` → **5.5185** (D-5).
  2. **Focus ring on a destructive row is worse than G-9 was for buttons** —
     `accent.primary` on `status.danger_fill` = **1.2089**, and on
     `danger_fill@85%` over overlay = **1.4953**. The renderer must call
     `widget::focus::resolve_ring_color(resolved_fill, palette)` (D-6), never
     the literal `accent.primary` the matrix prints.
  3. **A disabled row must never inherit a hovered ancestor's fill.**
     `text.disabled` on `surface.overlay` = 5.2686 (pass), but on
     `surface.hover` = **4.4734** and on `surface.active` = **4.1743** (both
     fail). The spec's "Disabled → transparent background" is load-bearing, not
     cosmetic.
  4. **`accent.text` must not carry a `✓` on a destructive fill** —
     **2.9557**, fails. Use `text.strong`, as the matrix already says for the
     label.
  5. **The keyboard-focus ring on a normal row passes thinly** —
     `accent.primary` on `surface.hover` = **3.2954**, headroom 0.2954. Do not
     lighten the row background without re-checking this pairing.
  6. **Fill-vs-fill deltas are ungated and near-invisible** —
     `surface.hover` vs `surface.overlay` = 1.1778; `status.danger_fill` vs
     `surface.hover` = 2.7259. The row-lift affordance rests entirely on a
     delta no rule protects.
  The separator (`border.subtle` on `surface.overlay` = **1.2017**) is **not**
  a violation: rule 2 governs *active* indicators, and a resting divider is
  deliberately below the floor — already settled and asserted that way.
  Everything else in the matrix clears comfortably (Rest label 11.7180, trailing
  6.7751, check glyph 9.4891, hovered 15.0427, pressed 14.0368, submenu
  chevron 8.0568, card edge 4.0128, destructive pressed 6.8258).
- **G-12 — Two token gaps surfaced by the menu renderer.**
  1. `token::typography` has no **12px SemiBold** rung. `CAPTION` is
     Regular/Medium, but §4.9.2's section heading is `12px SemiBold`. The
     renderer composes `CAPTION.size` + `Weight::Semibold` rather than inventing
     a rung. If more surfaces need that pairing it should become a real rung
     rather than staying a composition.
  2. No token exists for the spec's **10px menu row padding** or **9px separator
     height**. Both are expressed as compositions — `space::SM + space::XXS` and
     `space::SM + shape::BORDER_HAIRLINE`. They are honest compositions of
     existing tokens, not magic numbers, but the spec names them as if they were
     primitives. Worth reconciling when the spec is next revised.
  3. **Extended by INC-20**, which needed four more things the type/space scales
     do not carry: a **22px** footer height (control rungs are 24/28/32/36, and
     `HIT_TARGET_MIN` is a *rule*, not a height to pick); a **16px drag
     affordance** (`size::ICON_MD` is also 16 but documents glyphs, so borrowing
     it would make a resize grip read as an icon); **13px Bold** for the overlay
     title — the same missing-Bold-rung problem as item 1, since `UI_COMPACT`
     rests at Regular and emphasises to Medium; and the **monospace family
     itself**, which the type scale does not model at all — it carries
     size/line-height/weight/tracking and stops short of families. The movable
     surface can vouch for the monospace *sizes* only.
     Taken together these say the token scale is complete for *controls* and
     incomplete for *surfaces and type families* — a real gap to close before a
     second app builds on this crate.
- **G-13 — §4.1 and §5 give two different orderings and both are correct.**
  `UIF-WIN-003` unwinds **menus before dialogs** (deepest submenu → root menu →
  topmost dialog → search/filter), while §5's band table elevates **Modal (3)
  above Menu (2)**. These are not in conflict — they answer different questions:
  *"what is painted in front"* versus *"what is the user unwinding"*. A menu
  opened from inside a dialog sits under the dialog's scrim in z, but the first
  `Escape` closes the menu, not the dialog.
  `iced-pro` models them apart rather than deriving one from the other and then
  contradicting it: `escape_rank` is `Menu(3) > Modal(2) > UtilityOverlay(1) >
  Workspace(0)`, independent of band rank. The spec should state both orderings
  explicitly when revised.
  A consequence worth naming: **the topmost-painted surface is not the first
  asked to handle `Escape`.** Any host wiring this must not assume
  "topmost = first to receive keys".

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
