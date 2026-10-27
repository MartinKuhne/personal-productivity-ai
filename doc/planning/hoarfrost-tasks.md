# hoarfrost — Implementation Task Ledger

*(Formerly `iced-pro`; renamed to `hoarfrost`)*

> **Status:** In progress — increment ledger for implementing
> [`src/hoarfrost/spec.md`](../../src/hoarfrost/spec.md) as a new workspace crate.
> **Date:** 2026-10-26 (ledger last updated; opened 2026-10-21)
> **Spec:** [`src/hoarfrost/spec.md`](../../src/hoarfrost/spec.md) (UIF-*
> requirement IDs). The spec used to live in *this* directory as
> `ui-framework.md`; commit `edbe04a` moved it inside the crate it governs.
> Read §3c's opening record before treating any older path in this file as
> current.
> **Supervisor:** OpenCode agent session (orchestrates one subagent per increment)

---

## 1. Confirmed decisions

| # | Decision | Outcome |
|---|---|---|
| D-1 | Crate location & name | `src/hoarfrost/`, package `iced-pro`, lib name `iced_pro` |
| D-2 | Scope | **Framework layer only.** Design tokens, atoms, menu engine, dialog/overlay hosts, z-order bands, geometry + contrast solvers. App-specific content (navbar slot contents, tree rows, tab strip contents, Batch/Tools dialogs) stays in `fastmd`. |
| D-3 | App integration | **Standalone first.** `fastmd`'s `src/app/ui_iced/` and `src/app/ui/` are NOT modified in this effort. Wiring is a later, separate project. |
| D-4 | Token source of truth | `iced-pro` owns the `UIF-TOK-005` palette exactly as specified. `src/app/ui/theme.rs` is left untouched. |

### Hard constraints for every increment

- **Do not touch** `src/app/**`, `src/agent/**`, `src/md2pdf/**`, or any `SPEC.md`.
  New code lives under `src/hoarfrost/` only. (Exception: the root `Cargo.toml`
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
- **Read before writing:** `src/hoarfrost/spec.md` (the spec),
  `AGENTS.md` (root), `doc/distill/iced.md` (iced conventions).

---

## 2. Target crate layout

```
src/hoarfrost/
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

- [x] **INC-01** Crate skeleton. `src/hoarfrost/Cargo.toml` (package `iced-pro`,
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

- [x] **INC-21** `gallery.rs` + `bin/gallery.rs` (`UIF-VER-004`) — the full
      `(Variant × State)` matrix, enumerable and asserted. **91 cells.**
      | Subject | Cells | Shape |
      |---|---|---|
      | button | 36 | 4 variants × 9 reachable states |
      | input | 27 | 3 kinds × the same 9 states |
      | menu | 12 | `RowState::ALL` |
      | modal | 3 | clean / dirty / error |
      | input-dialog | 5 | rename-valid / rename-invalid / create-valid / move-folder-list / delete-danger |
      | movable | 4 | idle / dragging / resizing / auto-scroll-paused |
      | z-band | 1 | overlay-menu-modal (G-13 visible in one frame) |
      | NotOwned | 3 | navbar / panes / tabs |
      The 9 states are **derived**, not listed — all 32 `InteractionInput`
      combinations swept through `state::resolve`; `disabled+focused` is absent
      because `resolve` zeroes the ring on a dead control.
      **`NotOwned` is the honest answer to D-2.** Navbar, panes and tabs are
      app-owned and have no framework-owned frame. Each cell carries its
      justification in a `note`, draws **zero** quads and resolves **zero**
      channels, so the audit can read "out of scope under D-2" straight off the
      manifest instead of inferring it from a missing row. Notably: **a tab is
      not a variant of the §3.1 button** — snapshotting a button there would
      misrepresent coverage rather than provide it.
      **Snapshots assert resolved style values plus the ordered quad list, not
      pixels.** The resolved values are what compliance is made of — every
      `UIF-TOK-006` rule and every matrix cell is a statement about a resolved
      colour or rectangle. They diff **by token name** (`background:
      accent.primary → accent.primary @ 85%`) rather than as an undifferentiated
      pixel delta, and they don't pin every glyph, antialiasing kernel and
      rasteriser version to the repo. The frame is still a real draw pass:
      `Layer`s go through `renderer::Renderer::fill_quad` on `FrameRecorder`,
      the same call a widget's `draw` issues, so the recorded quads are the ones
      a renderer actually received and in the order it received them.
      `FrameRecorder` is library code, not `cfg(test)`, so the binary and the
      asserted tests draw the same frames.
      **The contrast gate needed the audit's classification, not a threshold.**
      A naive `3.0:1` on every non-zero border produced 32 "failures" that were
      all gate bugs, not design bugs — a Primary border deliberately sharing its
      fill's channels, a Ghost stroke over transparency, `border.subtle`
      hairlines. Adopted INC-08's four-class model (`Judged` /
      `UnpaintedBackground` / `CoincidentWithFill` / `RestingDivider`) with
      worst-case compositing of translucent fills over the surface ladder.
      `RestingDivider` matches on the token's **channels** rather than the
      channel name, because the gallery meets `border.subtle` as `border`,
      `control.border` and `divider`; every `border_subtle` use in the crate
      was verified to be a resting border, so the exemption covers the token
      completely and cannot hide a real failure.
      **Binary guard verified by execution:** `cargo run -p iced-pro --bin
      gallery` exits **0** with 91 cells and both gates PASS; pointed at
      `%APPDATA%` it exits **2** (likewise `%USERPROFILE%` and any `.fastmd*`
      component) — RUST-024 enforced, not just documented.
      **Scope deviation, accepted:** `Cargo.toml` gained an explicit `[[bin]]`
      target. The crate's lib path is `mod.rs`, not `src/lib.rs`, so cargo's
      default `src/bin/` discovery does not apply and the gallery would be
      invisible to the build otherwise.
      **Known uncovered cells (recorded, not silent):** only `Palette::dark()`
      is snapshotted (no inverted palette); only the standard control height (no
      compact rung); modal internals carry no header/banner/footer sub-rects by
      design; menu separator/glyph placement, input-dialog per-row bodies,
      movable trailing actions and the iced caret are not cells.
      Tests: 47 new (1249 total).
      **Commit:** `feat(iced-pro): add widget gallery and snapshot harness`
- [x] **INC-22** `spec_coverage.rs` + `ARCHITECTURE_C4.md` — the compliance audit,
      made **self-maintaining rather than a document that decays.**
      **Inventory: 46 unique numbered `UIF-*` IDs across 13 families**
      (TOK 6, PRIN 5, VER 4, INP 4, MOV 4, CMENU 3, FMENU 3, MOD 3, WIN 3,
      ATOM 2, NAV 3, PANE 3, TAB 3). 47 occurrences, 46 unique
      (`UIF-MOD-002` is defined once and cross-referenced once in `UIF-WIN-003`).
      **Disposition: 25 Implemented · 14 Partial · 7 NotOwned = 46**, asserted to
      partition the set exactly.
      **The audit bites.** A test reads `src/hoarfrost/spec.md` at test
      time, extracts every numbered ID by regex, and asserts both directions:
      every spec ID is classified, and every manifest entry exists in the spec.
      Verified by injection — adding a fake `UIF-FAKE-999` to the spec produced
      four failures naming exactly what was wrong, and the spec was restored
      byte-exact (SHA-256 verified, `git diff` empty). Two permanent tests keep
      firing on doctored input every run, so the guard is always observed alive.
      **Status measures spec-conformance, not effort** — a taxonomy decision made
      deliberately. `TOK-005` is `Partial` even though the token set is complete,
      because `Implemented` would tell an integrator the palette matches the
      spec and it does not (G-7 retune + D-5 split). Likewise `TOK-006`: every
      resolver obeys the foreground lock today, but nothing *prevents* a future
      violation, so the missing enforcement is on the record rather than hidden
      behind a green tick.
      **The 14 `Partial`s are the real integration backlog**, each naming what is
      missing. The substantive ones: **no tooltip surface exists at all**
      (NAV-003, ATOM-001 — no hover-delay, no floating card, no placement rule);
      **no font family in the type scale** and no 12px SemiBold / 13px Bold rung
      (TOK-004, G-12); **no source-level ban on raw literals**, so PRIN-001 and
      half of VER-001 are enforced only where the gallery can see emitted colours;
      the **Selection Foreground Lock is convention not code** (TOK-006);
      tab-strip overflow and the 10px splitter grab zone are untested because no
      framework geometry exists for them (VER-002); dirty **tab** closure is
      uncovered (VER-003); the gallery covers dark palette and standard height
      only (VER-004).
      **Audit caught real citation drift — see G-14:** `gallery.rs` cited
      `UIF-ZORD-001`, an ID the spec never assigned (§5 carries only the family
      wildcard). Corrected to `UIF-ZORD-* + UIF-WIN-003`, matching every other
      module. The deeper gap: the four-band layering rule is among the most
      load-bearing requirements in the document and has **no numbered ID of its
      own**, so it cannot be traced or gated by ID.
      **`ARCHITECTURE_C4.md` updated (RUST-040):** `iced-pro` added as a third
      workspace crate at Level 2 with **no `Rel` edge drawn to `fastmd`**, because
      none exists (D-3); a new Level-3 component section mapping the twelve
      subsystems to real paths; and the three integrator-critical decisions
      recorded where a future integrator will hit them — **G-13** (escape rank ≠
      band rank: topmost-painted is not first to receive keys), the **one-way
      focus bridge** (model → router, read-back confined to `cfg(test)`), and
      **D-7** (stock iced widgets are not keyboard-reachable; dropping a stock
      `button` silently breaks `UIF-PRIN-005`).
      **Aspiration kept out of technical context** as that directory's rules
      require: the section opens by stating plainly that this is not live
      application architecture and no module under `src/app/` imports
      `iced_pro`; every node maps to a real file; every claim is present tense
      about code that exists; the not-owned list points at the ledger instead of
      describing a planned port.
      Tests: 21 new (1270 total).
      **Commit:** `docs: record iced-pro crate boundary and spec coverage`

---

### M8 — Spec revision 2026-10-22: re-baseline and gap closure

The spec was revised mid-project (308 lines). The revision **adopted
decisions D-5 and D-6 outright** and re-baselined the colour vocabulary,
which turned several recorded "deviations" into conformance and created new
work. Increments INC-23…INC-30 plus one defect fix.

**Every contrast figure in this milestone was independently recomputed from
raw sRGB. Nothing was transcribed from the spec's printed columns.** That
discipline paid for itself repeatedly — see the spec-error list below.

#### INC-23 — type scale families and allowed-weight sets (`UIF-TOK-004`, G-12)

Two defects. First, no font-family tokens existed at all. Second, the
`TypeRung` model was the wrong *shape*: it carried a resting/emphasis
**pair** where the spec's "Allowed Weights" column describes a **set**.
The pair cannot express that `caption` may legally carry SemiBold, or that
`ui_compact` may carry all four — which is exactly why `menu/view.rs` and
`movable.rs` were hand-composing weights.

Added `Family::{Sans, Mono}` mapped onto iced's *generic* families. Naming
`"Fira Sans"` would have claimed a four-weight family the crate does not
ship: iced's `fira-sans` feature bundles **only** `FiraSans-Regular.ttf`
(plus `Iced-Icons.ttf`). `Font::MONOSPACE` resolves through cosmic-text's
system fallback, so mono needs no bundled TTF — but the four-weight model's
glyph availability depends on the **host machine**, not this crate. That
limitation is documented rather than papered over.

#### INC-24 — palette re-baseline (`UIF-TOK-005`, `UIF-TOK-006`)

Rename `accent_primary` → `accent_fill`, `status_danger` →
`status_danger_text`; add `accent_fill_active`, `accent_border`,
`status_danger_fill_active`; revalue `surface_hover` (38,38,44)→(32,32,36)
and `surface_active` (43,43,51)→(42,42,46). ~400 references across 18
files. The new model is a six-token polarity split that validates D-5 and
D-6. `TOK-005` → **Implemented**: every token now matches the spec's sRGB
exactly.

Rules 1, 3a and 3b pass the full matrix; rule 4's strong-on-fill span is
4.8020–5.7543 as claimed.

**Two spec-internal conflicts pinned rather than patched** — neither is
fixable in code without breaking surfaces that currently pass:

| Conflict | Measured | Spec says |
|---|---|---|
| rule 2 `[4.00, 6.00]` band, `text.disabled` on `surface.sunken` | **6.0205** | must be ≤ 6.00 |
| rule 3 `CoincidentWithFill` floor, `status.danger.fill_active` on `overlay` | **3.0789** | must be ≥ 3.08 |

The second is self-referential: the spec derived its own floor from that
cell and then rounded the floor above the value.

#### INC-25 — real pressed-fill tokens in the button resolver

`PRESSED_FILL_ALPHA = 0.85` synthesised what §3.1 now supplies as tokens.
Deleted, along with `with_pressed_alpha`. Primary pressed →
`accent_fill_active`; Danger hovered → `status_danger_fill`, pressed →
`status_danger_fill_active`. Primary hover repaints the **border only**,
never the fill.

The conditional Danger ring (accent.border at Rest, text.strong when
Hovered/Pressed) is **computed from the resolved fill** by the existing
D-6 resolver, not hard-coded. Two tests lock the mechanism rather than the
outcome, so a hard-coded ring that happened to match the colours would still
fail.

**Correction to my own brief:** I supplied `accent_border/surface_raised =
6.2745` and `status_danger_text/surface_raised = 6.1836`, derived from
the spec's 4dp-rounded luminances. Raw sRGB gives **6.2727** and **6.1812**.
The subagent kept the crate's raw-RGB solver and flagged mine. Lesson:
rounded intermediate values are not a source of truth.

#### INC-26 — new size tokens; last opacity hack removed

Added `size.bar.status` (24), `size.indicator.dot` (6),
`size.splitter.grab` (10). The splitter grab band is declared with **no
consumer** — D-2 leaves splitters app-owned, so the alternative is the app
inventing its own number for the same measurement, which holes through
`PRIN-001`.

Movable footer: raw `22.0` → `size::BAR_STATUS` (24). Derived values
recomputed: `CHROME_HEIGHT` 90→92, `CONTENT_VIEWPORT_MIN` 170→168.
Destructive menu row pressed → `status_danger_fill_active`;
`PRESSED_FILL_ALPHA` and `with_alpha` deleted. A crate-wide gate walks 75
state fills and asserts each is alpha 0.0 or 1.0, with the walk count
itself asserted so the gate cannot go vacuous.

#### INC-27 — 1px inset focus outline (`UIF-ATOM-000` → Implemented)

Colour, stroke width and placement travel as one `FocusIndicator`, because
a bare `Color` pushes width and placement back onto the call site — which
is how an inset outline drifts back to 2px and outward.

**Iced strokes a quad inward from its bounds**
(`iced_wgpu-0.14.0/src/shader/quad/solid.wgsl:82`). So `Outer` expands
the control by the ring width and grows the radius to stay concentric;
`Inset` keeps bounds *and* radius unchanged, putting the whole 1px inside
with its outer edge on the row's own edge. An earlier draft used
`shrink(width)`, which left a 1px band of fill at the perimeter — a third
placement the spec does not describe.

Both placements share one adaptive colour rule. The two focused menu cells
fall out of arithmetic rather than naming a colour: normal focused resolves
to `accent.border` (5.4427:1 vs `surface.hover`); destructive focused
resolves to `text.strong` because accent measures 1.7188:1 against the red
fill and loses.

#### INC-28 — source-level token enforcement (`PRIN-001`, `VER-001`)

`token_lint` makes the "no raw literals" rule machine-enforced. Previously
the only automated gate was the gallery's unmapped-colours check, which
sees a hard-coded colour only if the gallery renders that cell, and sees
hard-coded padding, radius, stroke and size **not at all**.

Rule A bans colour constructors and hex literals outside `token/color.rs`.
Comments, strings and char literals are redacted to spaces with line/column
preserved, so documentation *quoting* a constructor is never reported.
Rule B bans a bare numeric literal bound to a visually-named `pub const` in
`widget/`/`surface/`, excepting `TokenBound`, `Zero`, `Ratio`, and
`SurfaceBound` — the §1.1 carve-out being an enumerated registry of 15
entries each carrying its authorising section, not a regex.

Three violations found before any allow-listing; two fixed rather than
excused (resize grip → `size.icon.md`, which the spec's own table names for
the resize grip glyph, value-identical; scrim → struct update instead of a
constructor). One allow-list entry, pinned at length one, with a written
reason.

#### INC-29 — solid-fill foreground lock enforced (`TOK-006` → Implemented)

A pure predicate plus a guard the resolvers call. A newtype would ripple
through every `Color` consumer; a `Result` constructor cannot thread the
pure/`const fn` paint paths. The guard checks and never rewrites, so **zero
rendered colour changed** — the hard constraint, and it held.

**The lock is role-based, not threshold-based, and that is load-bearing.**
Of twelve foreground-only × locked-fill pairings, ten fall below the 3.00
chrome floor but **two clear it** — `accent.text` on `accent.fill_active`
at **3.0293** and on `status.danger.fill_active` at **3.0820**. Rule 4
forbids both regardless. A threshold-based implementation would have
permitted them.

#### INC-30 — gallery coverage (`VER-004`)

91 → **175 cells**. Added a palette-world dimension (Standard /
InvertedEditor) because value-based naming is genuinely ambiguous — the
editor canvas `RGB(255,255,255)` *is* `text.strong`'s channels. Added a
control-rung geometry dimension (32 / 28), modal header/banner/footer
sub-rectangles, menu separator and glyph placement as ink boxes (glyphs
come from a text pass, not `fill_quad`), input-dialog row bodies, and
movable trailing actions.

Two behaviours corrected rather than assumed: a resting menu row paints no
fill, so its glyph is judged against `surface.overlay` rather than waved
through as `UnpaintedBackground`; the submenu chevron is `text.muted`
closed and `accent.text` only while the branch is open.

#### Defect fix — the gallery path guard only refused the exact root

**Found by running the binary rather than trusting a report.** The guard
compared the candidate path to each forbidden root for *equality*.
Refusing `%APPDATA%` while accepting `%APPDATA%\fastmd-snapshot` protects
nothing. Combined with the `.fastmd` marker requiring a leading dot, a full
175-cell snapshot landed in `%APPDATA%` while the guard was believed to be
working. The stray output was removed.

Now refuses containment, component-wise rather than string-wise. **The
first containment fix broke the default**, because on Windows the temp tree
lives *inside* `%LOCALAPPDATA%` — the carve-out has to be the temp tree
specifically, not its parent. Verified end to end: APPDATA / USERPROFILE /
LOCALAPPDATA subdirs all exit 2; default temp exits 0; nothing left behind.

The guard had **no tests at all**, which is how the hole survived while a
working guard was being reported. Eight added, testing the pure predicate
only so no test can touch a real user path.

#### Spec errors found by recomputation

| Cell | Spec prints | True |
|---|---|---|
| `status.danger.fill_active` luminance | `0.1309` | **0.1325** |
| `status.danger.fill_active` vs `base` | `3.43` | 3.4575 |
| `status.danger.fill_active` vs `active` | `2.46` | 2.4842 |
| `accent.fill_active` vs `active` | `2.52` | 2.5274 |
| `Secondary` hover border vs hover | `3.66` | 3.6772 |
| normal focused menu outline | `5.35` | 5.4427 |
| `editor.inverted.err` on white | `7.00` | 7.1413 |

None of these break a floor. They confirm the standing rule: never
transcribe the spec's contrast column.

#### Milestone result

- **1406** crate tests, **4567** workspace tests (6 skipped). All six gates
  green at every step.
- Coverage: **28 Implemented · 12 Partial · 7 NotOwned = 47** (was
  25/14/7 across 46).
- `TOK-005`, `TOK-006` and `ATOM-000` moved to Implemented.
- Zero opacity-synthesised state fills remain in the crate.

---

### M9 — Second revision `e6ecbfb`: the audit was lying

The spec was revised a **second** time mid-project, by `e6ecbfb`
("doc update", 16:44). It went from 435 lines / 47 ids to **603 lines /
62 numbered ids**, adding seven new atoms (§3.3–§3.9) and an entire new
composite-components chapter (§3B.1–§3B.8), plus a data-binding chapter
(§3.0.4) and a container-query responsive model.

The visible symptom was four failing `spec_coverage_tests`: the manifest
expected 47, the spec carried 62. **The real problem was underneath.**

#### The extraction was blind to suffixed requirements

The extractor used `UIF-[A-Z]+-\d+`, which silently truncates every
**letter-suffixed** id onto its parent. The new revision uses ten of them:

```
UIF-ATOM-000b  001a  001b  001c  001d
UIF-ATOM-002a  002b  002c  002d  002e
```

Consequences, verified before any fix:

| Parent | Manifest said | Actually unbuilt beneath it |
|---|---|---|
| `UIF-ATOM-002` | **Implemented** | `002c` PasswordInput (bullet masking, clipboard Copy/Cut suppression, reveal toggle); `002d` AutoSuggestBox (completion flyout, highlight roving, two-stage Escape); `002e` MultiLineTextArea (auto-grow, cursor/selection model, undo/redo, Ctrl+Enter submit) |
| `UIF-ATOM-000` | **Implemented** | `000b` controlled-vs-ephemeral state split, three-tier `on_change`/`on_commit`/`on_cancel` lifecycle, `ActionPending`, container-query `ResponsiveBreakpoint` |

**This is worse than a wrong number.** The count test would catch 47 vs 62.
The extraction blindness meant *every* `Implemented` in the manifest was
untrustworthy — a requirement could gain an entire unbuilt widget beneath
it and still read done. That is the decorative-gate failure mode this
project has been fighting, arriving from a direction the gate was not
built to see.

#### The fix

A suffixed id is now a **distinct, independently classified requirement**.
The rule is deliberately narrow — exactly one trailing ASCII lowercase
letter — so `UIF-TOK-001X` and `UIF-ATOM-002ab` stay rejected, and
lowercase prefixes still cannot forge an id.

True count: **72** (62 base + 10 suffixed), cross-checked three ways —
the Rust extractor, an independent Python reimplementation, and a
set-difference proving the fix only *added* visibility and lost nothing.

Three guards against recurrence:

1. Every suffixed id must have its own entry, **and if its parent says
   `Implemented` the child must independently say `Implemented`** —
   otherwise the parent is covering for unbuilt work.
2. Injection test using `UIF-ATOM-002z`, mirroring the `UIF-FAKE-999` pattern.
3. Base and suffixed ids must be two separate ledger entries.

**Proved live, not vacuous:** flipping `ATOM-002` back to `Implemented`
makes the guard fail with *"UIF-ATOM-002 is `Implemented` while its own
sub-requirement UIF-ATOM-002a is `Partial` — the parent is covering for
unbuilt work"*. Flip reverted.

`ATOM-000` and `ATOM-002` both dropped to `Partial`. `002b` is the one
suffixed requirement genuinely `Implemented` — the zero-shift trailing
slot partition really is built.

#### Tooltip conformance (the revision resolved invented values)

INC-31 built the tooltip when §3.7 specified almost nothing and had to
invent. The revision pinned them, and **three guesses were wrong**:

| | invented | spec now says |
|---|---|---|
| hover delay | 500 ms | **400 ms** |
| keyboard-focus delay | absent | **0 ms** (new second arrival channel) |
| corner radius | `RADIUS_CARD` (6) | **`radius.widget` (4)** |
| max width | "host's problem" | **280 px** |

Keyboard focus is a second arrival channel, not a retimed hover: first
arrival owns the channel and is idempotent, so a pointer wandering onto a
keyboard-focused control cannot restart the timer.

Recomputed rather than transcribed — `border.strong`/`overlay` = **4.0128**
and `text.primary`/`overlay` = **11.718**, both agreeing with the spec's
printed 4.01 / 11.72.

**Palette finding:** `surface.overlay` vs `surface.base` = **1.1230:1**.
The tooltip card is essentially indistinguishable from the base surface by
fill alone; the `border.strong` hairline is the only thing making it read
as floating. The spec assumes its overlay is visually distinct from its
base and is wrong about its own palette.

Z-order: deliberately **not** a new `ZBand`. The band model's
`UnwindAnswer` has no vocabulary for *dismiss-but-do-not-consume*;
registering a band would move `topmost()`/`keyboard_owner()` for existing
hosts, and a tooltip answers "none" to everything a band is asked. It is a
presentation layer above all bands, outside `SurfaceRegistry`, dismissed
as a pre-pass so the owner underneath still sees `Escape`.

#### Manifest after M9

**72 entries, 72 distinct, no duplicates: 27 Implemented / 38 Partial /
7 NotOwned.** 1501 crate tests, 4662 workspace tests, 182 gallery cells.

#### What the second revision implies for scope

The new chapters are not small. Framework-owned and unbuilt:

- **Atoms** — Checkbox/Radio/ToggleSwitch (tri-state), ComboBox/Select,
  Slider/NumberBox, Chips/Filter Pills, TeachingPopover, Badges,
  ProgressBar/Spinner
- **Composites** — FormField, InfoBar/Toast, Expander, ScrollView +
  scrollbar contract, Card/ListTile, VirtualList/TreeView, DataTable,
  BreadcrumbBar/overflow toolbar
- **Cross-cutting** — `000b`'s controlled/ephemeral split, three-tier
  callbacks and `ActionPending` cut across *every* existing widget, so they
  are a refactor of the whole atom layer, not an addition to it
- **New size tokens** — `checkbox.box`, `switch.track`, `switch.thumb`,
  `slider.track_height`, `slider.thumb`, `progress.bar_height`,
  `spinner.sm`, `spinner.md`

`COMP-006` and `COMP-007` sit on the **D-2 boundary** and were called
`Partial`, not `NotOwned`: D-2 excludes what the app draws *inside* these
surfaces, not the surfaces themselves. The framework owes the virtualised
viewport, row height, indent step, disclosure chevron and selection
choreography for the tree; the sticky header, row height, 10px resize
grab and cell traversal for the table. The app owns the node model,
filtering, column definitions and cell rendering.

---

## 3c. App integration increments (INT-*)

D-3 said "standalone first — wiring is a later, separate project". That
later project started here. These increments **do** touch `src/app/**`;
they are the exception to the M-series constraint, taken under a separate
supervisor instruction on branch `feature/iced-pro`.

### The spec move — `doc/planning/ui-framework.md` → `src/hoarfrost/spec.md`

**This record is here because the spec is no longer where every other record in
this file says it is.** Commit `edbe04a` ("doc update") moved the spec out of
`doc/planning/` and into `src/hoarfrost/`, the crate it governs, and rewrote it
in the same step: 1548 lines to 1597, 138 insertions across 89 deletions. It
also added a ninety-first requirement. Every reference to the
old path in this ledger is now wrong; the ones that matter have been repointed,
and the ones that are deliberate history have been left alone and labelled.

**The spec is now co-located with the crate it governs, and the coverage reader
resolves it with no traversal at all.** `spec_coverage_tests.rs:39`'s
`spec_path()` used to walk two directories up —
`CARGO_MANIFEST_DIR/../../doc/planning/ui-framework.md`. It is now
`CARGO_MANIFEST_DIR/spec.md`. That is a better invariant rather than merely a
different path: the reader no longer reaches outside the crate boundary to find
the thing it audits, so the crate is self-contained for conformance checking.

**The break was one function and it was total, and that is the lesson.**
`edbe04a` repointed nothing that reads the spec, so the crate's own conformance
gate went red through no fault of the code. Seven coverage tests failed — with
**one** panic message, `cannot read the spec … (os error 2)`. Not seven
findings: one dead path, because every one of those tests opens that file before
it can assert anything. A coverage gate that cannot read the spec it covers
reports nothing at all, which is worse than reporting failures, because a tree
that reports nothing looks like a quiet tree. The failure mode of a moved
conformance input is not "fewer assertions pass"; it is "no assertions run".

**What moving a gated file requires, stated for the next contributor:** a spec
move is not a docs-only change. The move is observable only through the gate that
reads it, and that gate fails at one place no matter how many assertions hang off
it. Repoint every reader in the same commit as the move, then confirm the reader
still reads. A green coverage suite immediately after a spec move means nothing
until someone has seen it open the new file — and a *red* one, as here, is the
only reason the move was noticed at all. `0076e8d` did the repoint and the
classification together, which is the shape the next one should take.

**`UIF-MOD-004` — Scrollable Sub-Region Height Solving (`spec.md:1379`), added
by the same commit; classified Partial.** The requirement: solve a scrollable
sub-region as
`subregion_max_h = modal_height - (header_band + 2 × body_padding + footer_band)`
with every term taken from tokens, never borrow the card's outer height constant,
and add a named constant to the closed §2.2 table (`spec.md:73`) when a
sub-region needs a stable cap — `ATTRIBUTIONS_LIST_MAX_H = 200` for the
Attributions credits list.

*What is genuinely missing:* there is no sub-region solver. `ModalSizing::solve`
produces the **card's** bounds from the viewport and stops there; nothing
derives a sub-region from the solved card, and `ATTRIBUTIONS_LIST_MAX_H` is not
defined in the token set.

*What is not broken, said plainly so it is not re-fixed:* the specific failure
the requirement names does not exist in this app, and it does not exist by a
**better mechanism than the spec asks for**. `credits_list`
(`attributions.rs:306`) sizes the list `.height(Length::Fill)` (`:309`) — a
remainder rather than a chosen number — so it structurally cannot borrow the
card's outer 480px constant and cannot overflow past the header and footer
bands. The hazard is unreachable rather than defended against. Claiming
`Implemented` would hide that the named cap is absent; claiming the app
overflows would be false. `Partial`, with both halves stated, is the only
honest row.

*The cap is not cosmetic, so this is live work:* a `Fill` remainder in a 480px
card leaves the credits list roughly 330px against the spec's 200px cap.

**The ID total moves 90 → 91.** `EXPECTED_SPEC_ID_COUNT` (`spec_coverage.rs:84`)
is pinned at 91 with a comment naming the commit that added the ninety-first, and
the two-directional audit fails the build if the spec and the manifest disagree
either way.

### INT-1 — Token bridge (done; recorded in `ARCHITECTURE_C4.md`)

`fastmd` depends on `iced-pro` and reads `UIF-TOK-005` from it. See
`doc/technical-context/ARCHITECTURE_C4.md` §"Consumption status (INT-1)".

### INT-2 — Escape / z-order: the overlay host walks the band model

**Scope: the z-order half only.** `OverlayState::close_topmost` no longer
decides "which layer is on top" from a hardcoded eight-branch `if`-chain;
it drives `iced_pro::surface::zorder::escape_with_pre_rank` over a
derived `SurfaceRegistry`.

**The registry is a DERIVED INDEX, not a second source of truth.** The
individual window-state fields (`batch`, `about`, `tools`, `debug`,
`dialog`, `editor`, `context_menu`, `hamburger_open`) remain the single
authority for *whether* a surface is open. `OverlayState::sync_surfaces`
mirrors them into the band model — open → `register`, closed →
`unregister` — and is called at the top of `close_topmost`, the only
moment the index must be exact. Hooking every update path was deliberately
not attempted in this increment. Idempotency comes from `register`
keeping an existing id's band position, so a card rebuilt every frame
cannot reshuffle the stack.

#### The escape-rank bug this fixed

The old chain contradicted `UIF-WIN-003` in two places. `ZBand::escape_rank`
is **not** band index (`Menu` is band index 2 but rank 3), and the chain
had been written as if it were.

| Situation | Old (wrong) first `Esc` | New (correct) first `Esc` | Rule |
|---|---|---|---|
| `Editor` + `ContextMenu` | `editor` — destroyed the modal the menu was opened over, orphaning the menu | `context_menu` | `Menu` rank 3 unwinds above `Modal` rank 2 |
| `About` (Modal) + `AgentDebug` (MovableCard) | `agent_debug` | `about` | a modeless utility card must not unwind ahead of a blocking modal |
| `Dialog` + `AgentDebug` | `agent_debug` | `dialog` | `InputDialog` is rank 2, above rank 1 |
| `Batch` + non-menu surfaces | `batch` | `batch` | unchanged — correct behaviour preserved |
| `Batch` + `ContextMenu` | `batch` | `context_menu` | rank 3 outranks rank 2 |

Full-stack unwind order, same seven-plus surfaces open:

| # | Old chain | New rank walk |
|---|---|---|
| 1 | batch | **hamburger** (rank 3) |
| 2 | attributions | **context_menu** (rank 3) |
| 3 | about | batch (rank 2) |
| 4 | tools | attributions (rank 2) |
| 5 | **agent_debug** (rank 1 — too early) | tools (rank 2) |
| 6 | dialog | dialog (rank 2) |
| 7 | editor | editor (rank 2) |
| 8 | context_menu (rank 3 — too late) | **agent_debug** (rank 1) |
| 9 | hamburger | — |

`test_close_topmost_walks_the_reverse_z_order` was **pinning the old wrong
order**. It is replaced by `test_close_topmost_walks_the_escape_rank_order`
with the corrected expectation, plus targeted tests for each row of the
table above.

#### Surface mapping

| App surface | `SurfaceKind` | Band / escape rank |
|---|---|---|
| `ContextMenu`, `Hamburger` | `MenuCard` | 2 / **3** |
| `Editor`, `Tools`, `About`, `Attributions`, `Batch` | `Modal` | 3 / **2** |
| `Dialog` | `InputDialog` | 3 / **2** |
| `AgentDebug` | `MovableCard` | 1 / **1** |
| `Logs` (docked background viewer) | *not registered* | — |

`Logs` stays out of the registry for the reason already recorded on
`close_topmost`: its visibility is application state owned by
`UserCommand::ToggleBackgroundLogs`, not reducer-held overlay state, and
it is not `Esc`-closable.

**Areas that did not map cleanly, recorded in code rather than forced:**

* **Regions are empty on every registration.** The app has no card
  geometry in the reducer. Per `SurfaceRegistry`'s contract a regionless
  surface still stacks and still takes part in `Escape`, which is the half
  wired here — but `route_pointer` can never answer
  `PointerTarget::Surface` against this index; it falls through to the
  band rule (`DismissBand(Menu)` / `Scrim(..)`). Recorded on
  `sync_surfaces` as a **review area**: real viewport rectangles arrive
  with the widget increments (INT-3..7).
* **`About` and `Attributions` share one `AboutWindowState`** (a single
  `Option`), so they are two `SurfaceId`s over one authoritative field and
  are mutually exclusive by construction.
* **The two menus are mutually exclusive in production**
  (`update.rs`: opening one dismisses the other), so their relative
  within-band order is an ordering *rule* pinned by test, not a reachable
  state. It follows the overlay host's draw order (context menu mounted
  first, hamburger on top), so the card painted on top unwinds first.

#### Decision: Tab / `FocusState` projection is deliberately deferred

Recorded in the `OverlayState` struct doc comment
(`src/app/ui_iced/app.rs`). The app captures **only** `Escape` off the
runtime keyboard stream (`ui_iced/subscription.rs:442`); `Tab` traversal
is owned exclusively by iced's built-in router today. Introducing
`iced_pro::focus::FocusState` + `bridge::tab_task` now would run a second
focus model in parallel with iced's with nothing keeping its member set in
sync against the live widget tree — the app never enumerates its
focusables, so the framework's member set would be fiction. The sync
problem disappears once the framework widgets of INT-3..7 self-register
their focusables as they are built, so the projection belongs there.

Also recorded there: the original integration plan named
`bridge::advance` / `bridge::sync` / `bridge::apply_transition`. **Those
do not exist.** The real bridge API is
`bridge::tab_task(model, direction)` / `bridge::focus_task(model)` /
`bridge::focus_operation` / `bridge::widget_id`.

#### Decision: the Pre-Rank step is wired with a no-op cancel

`close_topmost` runs `escape_with_pre_rank` on every `Esc`, but the
`cancel_capture` closure (`cancel_drag_capture`, `app.rs`) always answers
`false`: no framework drag widget is mounted, so no capture can exist.
The step is a **provably clean pass-through today**, asserted by
`test_close_topmost_pre_rank_is_a_clean_pass_through_with_no_capture` so
it is not dead code a later reader has to trust, and
`test_close_topmost_consults_the_pre_rank_ledger_before_the_walk` proves
the ledger is drained by the same call that closes a surface.
`test_a_live_drag_cancel_short_circuits_the_rank_walk` pins the ordering
INT-6 depends on — a `Cancel` spends the key and no surface is asked — at
the same composition the app uses. **INT-6 must replace the body of
`cancel_drag_capture`** with a dispatch to the capturing control's own
`GestureSlot::cancel`, keyed on `DragCapture::id`.

#### Gates (INT-2)

`cargo check --workspace --all-targets` clean · `cargo nextest run
--workspace` **7250 passed, 6 skipped** (was 7238; +12 new) ·
`cargo clippy --workspace --all-targets -- -D warnings` clean ·
`cargo fmt --check` clean · `cargo doc --workspace --no-deps` after
`cargo clean --doc`: all 5 crates re-documented, **0 warning lines**.

### INT-3a — The hamburger menu renders on the framework menu engine

First framework *widget* the app actually draws. `ui_iced/hamburger.rs`
builds a `Menu` tree, drives the cascade, projects focus, and settles
activations through `command_for_activation`.

Two framework behaviours had to be worked around rather than used:

- `MenuCascade::stabilise` closes any card the keyboard cannot enter, so
  a card whose only row is disabled cannot stay open. Every ported menu
  therefore needs an enabled, stays-open placeholder row.
- The empty-`regions` hole INT-2 left is closed **for this surface only**:
  INT-3a registers real `Placement` rectangles, so `route_pointer` can
  resolve `PointerTarget::Surface` against the hamburger card. Other
  surfaces still register empty regions.

Reconciled with the INT-2 deferral note in `app.rs`: `MenuCascade::on_key`
requires `&mut FocusState`, so the exit condition INT-2 named was met by
the framework enumerating its own members. App-wide `Tab` traversal stays
iced-owned.

Gates: 7749 workspace tests, all five green.

### INT-3b — Context menus on the framework menu engine

`cbef6a4`. File-tree, directory, multi-select and tab context menus now
render through `surface::menu`, pointer-anchored via `anchor::solve`.

**This retired the app's second menu engine outright.** The app carried
two: `menu.rs` (983 ln, rich `Action/Toggle/Radio/Submenu/Heading/
Separator` + `MenuState`) and `overlay.rs`'s simple
`MenuItem{label,message}`. Producers returned the simple type, `app.rs`
widened it, `menu_overlay` drew it. Two engines for one surface meant the
simple type's limits were the surface's limits — which is exactly why
separators were impossible.

**The separator gap is closed for real.** `center.rs` documented that the
overlay card was a uniform column of buttons whose rows had no separator
kind, so grouping was carried by row order alone. Each row now declares a
`ContextGroup` and the engine emits a real `1px border.subtle` divider at
every group boundary. The tab menu carries three groups — Primary,
Automation, Management — so it draws two dividers where it previously
drew none.

New: `context_menu.rs` (the `ContextRow`/`ContextGroup` model, row-key
routing, stable root ids) and `menu_surface.rs` (card chrome and solved
geometry shared with the hamburger, so the two cannot drift on inset,
width, row grid or focus-id scheme). An unsolved card is not mounted, so a
card past the end of the rect list can never appear at a guessed position.

`CARD_WIDTH = 224` is the app's own choice pinned at the low end of
§4.8.1's `224..320` range because iced-pro publishes **no** menu-card
width token — documented as such rather than passed off as a framework
number.

Row height (32), heading height (24), separator height (9) and card width
(224) unchanged; card inset 6 → 4 (`space.xs`), inter-row spacing 1 → 0
(contiguous grid, separators carry their own 9px). Quadrant flip pinned in
three directions.

Tests: net −25 (7749 → 7724). Shedding, not loss — 59 removed with the
code they tested, 34 denser tests added covering group-run sequences
against the spec for all four targets, separator placement including never
leading or trailing, destructive-group isolation, row-key uniqueness and
route coverage, root-id stability, anchor flips, and row-grid contiguity.

**Framework gap found — FALSE, and it kept the aiming corridor unwired
until INT-9.** What this record asserted was that `MenuRow` reports only
`Activated`/`Absorbed`, cannot drive hover or aim, and that B-3's
`AimCorridor` is therefore **not consumable from the row widget**. INT-9
disproved it two ways. First, iced 0.14.2's `mouse_area` has `on_enter`
(`iced_widget-0.14.2/src/mouse_area.rs:106`), `on_move(Point)` (`:113`)
and `on_exit` (`:120`) — hover entry/exit and pointer coordinates are
plainly available. Second, and decisively, `MenuRow` **already recorded
hover**: `surface/menu/view.rs:1652-1667` sets `state.hovered =
cursor.is_over(layout.bounds())` on every event, under a comment saying
the widget reports raw observations and `resolve` judges them. The row
knew where the pointer was the whole time; `RowEvent` simply had no
variant to say so. The gap was one missing enum pair, not a missing
capability. Consumed in INT-9.

### INT-4 — Input dialogs on the framework dialog surface

`ae408dd`. CreateDirectory, CreateDocument, Rename, Move, DeleteConfirm.

**The constant mapping was EXACT.** All eight — width 320, title 24,
prompt 20, input 32, error 20, buttons 32, gap 8, list max 200 — already
equalled their framework counterparts, as did the three type rungs
16/14/12. The app's dialog geometry was spec-conformant by coincidence,
so the rebinding is provenance with no pixel movement.

**Punish Late is now real on this surface.** B-4 built `EditStance` and
`punish()` with no renderer calling them. `painted_error()` is now the
only thing the card may draw, gated on `punishment().painted()`. The
subtle part is correct: the confirm gate reads `blocks_submit`, **not**
`painted` — a pristine empty field is invalid and unsubmittable even
though nothing is drawn. Withholding the visual is not forgetting the
fact. Closes the B-4 renderer residual **for the input-dialog surface**;
other app text inputs still never call `punish()`.

**Two real defects fixed.**

1. Clicking a disabled `Ok` used to **dismiss** the dialog — the press
   fell through the card to the scrim. A pre-existing test asserted
   exactly that: *"the inert Ok click must fall through to the modal
   scrim, which dismisses"*. That is the behaviour `UIF-INP-002` forbids
   verbatim. The spec was re-read at L1309 to confirm before the assertion
   was rewritten — the test was wrong, not the gate.
2. A silently-blocking `Ok`. `DisallowedClass::FilenameBasename` refuses
   characters `is_safe_basename` does not, so `"a:b"` gave `painted ==
   true` with `validation == None` — a disabled button with no
   explanation. Now falls back to `DIALOG_NAME_UNSAFE_ERROR`.

**`ValidationRule::Name` deliberately NOT adopted — framework gap
recorded in `spec_coverage.rs`.** The spec names it as the injected rule,
but `check_basename` (`surface/input_dialog.rs:464`) rejects only empty,
edge-spacing, control characters and the illegal character class. It does
**not** reject `.` or `..` as whole names, so the rule accepts a
directory-traversal name. It is a character-class check, not a traversal
guard. The app keeps its stricter `is_safe_basename`, which separately
rejects `Component::ParentDir`.

`InputDialogHost` not adopted — its focus trap and `SurfaceRegistry`
self-registration collide with `OverlayState` ownership. `keyboard_invoked`
cannot be honoured: the app cannot distinguish pointer- from
shortcut-invoked dialogs, so `None` means "no pointer ever reported", not
"keyboard". §4.5.1 wants Cancel-first right-aligned; the app renders
`Ok, Cancel` left. Report-only.

Tests: 7724 → 7746 (+22).

### INT-5a — About and Attributions on the framework modal frame

`eddb3a2`. Width change is spec-mandated: §4 sizing gives About 440 and
Attributions 640×480; the framework presets match exactly; **the app used
620 for both** and was non-conformant.

About 620 → 440 (−180px). Measured 440.0 exactly, 268.0 at a 300px
viewport where the gutter clamp reaches the box, 440×624 at 1024×768 with
144px headroom. Licence body lays out at 24 lines × 20 = 480px and
scrolls inside its 200px window with every word reachable.

**The `line_height` footgun, discovered here** — see the dedicated entry
below. `From<f32> for LineHeight` is a *multiplier*, not pixels.

Contrast recomputed from raw sRGB: the old `accent_fill` on
`surface_raised` that users actually saw measures **3.90:1** — a WCAG
failure. Moving to `accent_text` on `surface_overlay` gives **9.49:1**.
The fix is the token move, not a fill retune.

`LICENSE_MAX_HEIGHT = 200` stays app-owned: no framework token covers how
tall one scroll region inside a card may get, and `CONTENT_HEIGHT_MAX`
(480) is the ceiling for a whole content-fit card body — and the licence
body measures exactly 480px, so using it would mean no scroll and a
~764px card.

`UIF-MOD-001` focus trap **not wired and not claimable**: only Escape
reaches the app (`subscription.rs:442`); iced's router owns `Tab`, so a
pushed trap would be fiction. `UIF-MOD-002` not wired (no editable field,
no in-flight action). `UIF-MOD-003` not wired (no failing action). What
*is* wired: dismiss via Escape and scrim, and card-boundary absorption.

Tests: 7746 → 7779 (+33).

### The `line_height` footgun — menus rendered **no ink at all**

`bb18c2d`. The most serious defect found in the whole series.

`TypeRung::line_height` is a bare `f32` holding **pixels** (20.0 for
`BODY`, 18.0 for `UI_COMPACT`, 16.0 for `CAPTION`). But iced maps
`From<f32> for LineHeight` → `LineHeight::Relative(factor)`
(`iced_core-0.14.0/src/text.rs:221`), and
`Relative.to_absolute` returns `Pixels(factor * text_size)` (same file,
line 209). Passing a rung's `line_height` into `.line_height(...)` asked
iced for line_height × font_size pixels per line.

Measured: `CAPTION` 192px, `UI_COMPACT` 234px, `BODY` 280px, `TITLE_SM`
384px — exactly `size × line_height`.

**What the user saw: nothing.** INT-5a guessed the fixed 32px row slot
masked this. It did not mask it — it caused total disappearance. The
glyph is drawn at the vertical centre of the inflated line box (~112px
below the box top for `UI_COMPACT`), and cosmic_text clips the paragraph
to the buffer height it was handed, which is the 32px slot. A real
`MenuRow` before the fix: **no ink anywhere in a 600×6000 buffer.**
After: ink at 1000.5..1020.5 across 1876px. The app's own mounted card
failed *"row 0 painted nothing inside its rect {x:4, y:4, w:216, h:32}"*.
**Every menu in the app was blank**, and had been since INT-3a shipped.

Fix: `TypeRung::iced_line_height()` returns
`LineHeight::Absolute(Pixels(self.line_height))`. No token **values**
changed — the bug was at the boundary. All eight sites routed through it.

Footgun survey: `iced_core` has `From<f32>` for `Degrees`, `Radians`,
`Radius`, `Length`, `Padding`, `Pixels`, `Rotation`, `LineHeight`. Only
`LineHeight`'s is a multiplier rather than a magnitude; the others are
unused or never cross to iced here. `LineHeight` was the only realized
instance.

Guards: 14 tests, each verified to FAIL on a reintroduced bug.
Independently confirmed here — reintroducing the bare `f32` at
`settings_row.rs:272` makes `no_line_height_call_hands_iced_a_bare_f32`
fail, and restoring returns the tree to green. **The guard is not
vacuous.**

Near-miss worth recording: the first app-level ink test **passed with the
bug present**, because it bounded only `y` and the card's `border_strong`
(120,120,120) sits above the ink threshold and runs down the left and
right edges through every row band. Caught by ASCII-arting the frame. The
helper now bounds `x` too, and the permanent guard is per-slot rather than
per-row for the same reason.

### INT-5b — Editor modal sizing and the unsaved-edit guard

`96f1b4e`. **The gap: `EditorViewModel::is_dirty()` existed all along and
nothing used it to guard dismissal.** A search found only `tree_dirty` in
`orchestrator.rs`, unrelated. Escape or a scrim click over an editor with
unsaved work discarded it without a word. `UIF-MOD-002` is exactly this,
and it was simply unimplemented.

One rule, `guard_leave`, routed through `guard_editor_leave`, reached from
both dismissal paths — the Escape unwind on `SURFACE_ID_EDITOR` and the
scrim click in `reduce_editor`. Escape over a dirty editor is refused and
raises a discard confirmation; a scrim click over a **clean** editor still
dismisses normally, which matters because that path must not regress. The
`Cancel` button closes a dirty editor on purpose — that is the explicit
discard the spec names.

**Escape-rank correctness was the subtle part.** A refusal returns
`UnwindAnswer::Unwound`, consuming the key, **not** `Declined`. Returning
`Declined` would let the Escape travel on down the rank walk and close
`AgentDebug` or `About` sitting *underneath* the editor — so guarding the
editor would have started closing other surfaces.
`test_repeated_escape_never_discards_unsaved_edits` drives three rounds
and asserts the edits survive every one.

The confirmation surface is reused, not invented — `DialogKind::
DiscardEditorChanges` on INT-4's `InputDialog` with
`InputControl::Confirmation` + `ButtonVariant::Danger`.

Fixed 800×600 becomes viewport-responsive: 800×600 → 640×480, 1024×768
→ 819.2×614.4, 1280×800 → 960×640 (width capped), 1920×1080 →
960×760 (both capped), 600×420 → 520×380 (both floored), 320×240 →
288×208 (gutter only). Usable canvas height at the 520×380 floor is ≥
200pt, ten body lines, clean and with the error banner up. The banner
costs exactly 48pt, pinned to its tokens.

`EDITOR_TITLE_SIZE` 18 → `TITLE_SM` 16 because 18 was never a rung and
§4.6.1 says 16. Error slot `EDITOR-4001`/`EDITOR-5001` with live
`Recovery::Retry` and `FocusControl`. `EditorPalette` under `UIF-TOK-005`
carries only background/foreground/error — a real `text_editor` also needs
placeholder and selection, both added with contrast measured through the
crate's own WCAG solver.

Not claimed: `UIF-MOD-001` trap (Tab is iced's). A save that fails
**during** background execution is still not surfaced — only a queue
refusal is, because `UserFileOperationCompleted` failure results are not
routed back to the editor error slot.

Tests: 7793 → 7832 (+39).

### INT-5c — Batch and Tools dialogs on the framework modal stack

`8756315`, both halves.

**A real bug fixed on the way:** both dialogs previously mounted via
`overlay::modal`, which had **no absorb boundary at all**. A press on dead
space inside the card fell straight through and cancelled or closed the
dialog.

**Three components the increment plan named do not exist as assumed.** Each
was verified before wiring:

| Named | Reality | Used instead |
|---|---|---|
| `widget::chip` + `ChipVariant::Success` | `ChipVariant` has exactly `Assist`/`Filter`/`Input`; `ChipStyle` has **no status-colour axis** | `widget::badge` (`UIF-ATOM-008`) — a `✓` meaning "parallel-safe" is a *judgement*, and §3.8 is the framework's component for a judgement |
| `widget::toggle` | Resolver-only — ships `ToggleStyle` as `Layer`s, no `Element`-producing builder | iced `checkbox` **painted from `toggle::resolve_style`**, including its contrast lock |
| `form_field::stacked` for multi-row | Fixes its control band at `CONTROL_SLOT_HEIGHT = 32px` | Single-control fields use `stacked()`; the prompt section reuses `resolve_header`/`resolve_caption` with a filling middle band |

Also `ProgressStatus` has no `Completed` position and `BatchRunStatus`
carries no completed/total counts, so the bar is indeterminate — nothing
invents a fraction the subsystem never reported.

Batch 500 → 560 per `BATCH_WIDTH`, guarded by `assert_ne!` against the
retired literal. Batch mounts at the §4.6.2 **ceiling** rather than
content-fit, and that is load-bearing: five `stacked` `form_field`s at
`STACKED_HEIGHT` 76 consume 380px before the prompt list gets a pixel.

Tools width was already the spec's 720 — only the owner moved. The height
**policy** changed, which matters more than the number. The app's invented
`MIN_HEIGHT 200` / `MAX_HEIGHT_HARD_CAP 1200` became the framework's 320
floor, 80%-of-viewport cap and gutter-last rule. The case that mattered:
0 rows at a 100px window previously clamped to **200 — taller than its
own window**; now 68.

Error surfacing is genuinely used here, unlike About:
`ToolErrorKind::Authentication` → `ModalError` `TOOLS-5001` with
`ErrorLocation::Region` and a live `Recovery::Retry` that re-issues
`StartMcpAuth`. `error_kind` is carried onto `ToolGroupRow` from
`group.last_error` so the banner never parses the rendered
"Kind: message" line back apart. `Forget` is the card's only
`ButtonVariant::Danger` because it discards a credential the user cannot
re-derive by pressing a button.

Two second sources of truth retired: `parallel_chip_color` and
`parallel_chip_surface_color`.

Tests: 7832 → 7866 (+34).

### INT-6 — Movable Band-1 utility overlays

`bd8456e`. Agent Debug fully converted; Logs gets the blocking half
removed but keeps its pre-INT-6 interior.

**The design question: registration is not closability.** The plan wanted
both cards registered as Band-1 `MovableCard`s, and two existing tests pin
that the docked Logs panel must **never** be Esc-closable
(`app_tests.rs:924`, `update_tests.rs:2813`). Not actually in conflict:
`SurfaceRegistry` registration gives a surface its z-order and promotion;
closability is a separate decision made by the app's `ClosableSurfaces`
match in `close_topmost`. `sync_surfaces` now registers both cards as
`MovableCard` with their **real** card rects rather than empty ones, so
`route_pointer` answers Surface inside and Workspace outside, while
`SURFACE_ID_LOGS` returns an explicit `Declined(CannotUnwind)`.

Both pin tests pass with their bodies **untouched** — `update_tests.rs` has
no diff at all.

**Geometry persistence** `[RUST-024]`: `UiStateGeometryStorage`
implements `GeometryStorage` over the live `persisted_ui_state` plus the
injected `UiStateStorage`. It borrows the live model rather than doing a
handler-side read-modify-write on purpose: the close-time save writes the
whole model it is handed, so a handler-side RMW would have silently
reverted every position settled mid-session. `load` is pure memory with no
I/O on card open `[NFR-003]`; `save` fires only at settle, never per
frame `[NFR-002]`. Backed by an additive `overlay_geometry:
BTreeMap<String,String>`, `serde(default)`, so **no schema bump**.

**A contract almost broken:** the first cut keyed registration off
`host.is_some()` and broke five existing pins. The real contract is that
the index derives from the authoritative `OverlayState` field, so
`agent_debug` keys off `debug.is_open()` and Logs off a new explicit
`logs_visible` mirror. Logs' visibility has always lived on
`BackgroundLogsViewModel`, which is exactly why only Agent Debug was
mappable before this increment.

**`AT_BOTTOM_TOLERANCE` deliberately NOT adopted.** The framework value is
`space::XXS` = 2.0; the app's `BOTTOM_STICK_TOLERANCE` stays 1.0.
`logs_tests` pins a viewport 2.0pt short as NOT at bottom, so switching to
2.0 flips that cell and would keep yanking the view down 1.5pt into
someone's reading history. The decision matrix was unified **without**
moving the position threshold.

**Plan components that do not exist:** `composite::search_input` is not in
`iced_pro::composite` at all. Also iced 0.14 `mouse_area` has no
`on_press_with` — `on_press` carries a bare `Message` and only
`on_move(Point)` yields a coordinate — so a press is paired with the last
reported position. Sound in practice (the pointer must be over the bar to
press it) but an inference, not a press-time coordinate. It is also why
the full-screen capture lane is required: without it a drag that outruns
the title bar freezes mid-gesture.

**Correction to the brief:** `MovableHost` pushes **no** focus trap, which
is precisely why it *is* adopted and `sync_surfaces` converges with it.
Only `ModalHost` (`modal.rs:926`) and `InputDialogHost`
(`input_dialog.rs:1142`) are refused.

Tests: 7866 → 7892 (+26).

### INT-7a — Navbar and command band token alignment

`179b029`. `top.rs`, `bottom.rs`, `layout.rs` all fully converted.

**Navbar band 28 → 36 pt, spec-correct not preference.**
`size.control.lg`'s own token doc names it "Primary structural bars: Top
Navbar height" and the layout sketch prints Band 1 at 36px. What did
**not** grow is the controls inside it — the trigger and buttons stay at
`CONTROL_SM` 28 because §4.2.1 prints "Right: Workspace Controls — Height
28px" *inside* that 36px band. The band grew around unchanged controls.

Brand title 18 → 16 and Normal → Bold (`TITLE_SM`, the rung §4.2.1 names;
18 was never a rung). Navbar padding 2 → [4, 12]; the bar now closes
arithmetically: `CONTROL_LG (36) = XS (4) + CONTROL_SM (28) + XS (4)`.

**File-context floor 8 → 12 pt.** `compute_file_context` used `min_size =
8`, a size the type scale forbids. A long directory path could shrink the
label below anything the framework considers legible.

`stop_button_variant(running)` returns `Danger` while a turn is actually
cancellable, `Secondary` when idle — and the colour claim is *checked*:
`stop_button_label_colour_matches_the_danger_rest_lock` pins that the idle
label's muted colour and the Danger rest cell's resolved foreground are
the same token resolution, so the button cannot drift to a colour that
contradicts its own variant.

`band_scalars_resolve_from_tokens_and_share_one_density_rung` pins
PRIN-006's row-density homogeneity for the command band.

Tests: 7892 → 7903 (+11).

### INT-7b — File tree and ToC panel

`df053e3`. Row geometry, sizes and paint derive from `tree_row`'s own
constants rather than parallel app-side numbers.

**Three real defects.**

1. **Selected PDF-backed label: 1.59:1 → 4.80:1.** The app tinted the
   whole tree label with `status_danger_text` to mark a PDF-backed file.
   On a **selected** row that put red on the accent fill —
   `RGB(255,92,92)` on `RGB(93,96,238)`. Recomputed from raw sRGB:
   **1.5863:1** before, **4.802:1** after. WCAG AA needs 4.5. Not
   marginal — unreadable, and it only appeared on rows that were both
   selected *and* PDF-backed, which is why it survived.
2. **Tree rows had no height floor at all** — ~20px against the 24px
   pointer minimum §4.3.2 states. Now `ROW_HEIGHT_COMPACT`
   (`HIT_TARGET_MIN` = 24).
3. **Sub-floor text in search results.** `SECONDARY_TEXT_SIZE` was 11.0,
   below `MIN_SIZE` 12. Three lines carried it — match-count suffix,
   library-relative path, snippet — so every search result rendered below
   the `UIF-TOK-004` floor. Same class as INT-7a's `min_size = 8`.
4. Header weight was illegal: 16px **Regular** is not a legal `TITLE_SM`
   rendering — that rung forbids Regular outright.

Rendered: row text 14 → 13; indent 18 → 16/level (depth 4: 72px → 64px);
row inset-x 6 → 4; panel padding 6 → 4; auto-fit slack 40 → 32 (panel
auto-fits 8px narrower); search cards 52 → 58 (the old 52 only cleared
its content because iced's relative line height was doing the arithmetic
at ~51.2px inside); search glyphs 14 → 16 (they tracked `ROW_TEXT_SIZE`,
which moved to 13, and a 13pt icon is not on the icon scale).

**`tree_row` adopted for geometry and paint, blocked for rendering** — of
the four gaps named here, **two were real and two were false**, and both of
the false ones have since been built.

* **Still true:** `tree_row` produces no `Element` — it resolves
  `TreeRowSlots`/`TreeRowStyle`/`Layer` rectangles and only the gallery's
  canvas turns those into pixels. And the `[PDF]` badge §4.3.2 uses to
  carry meaning through the selection lock still does not exist.
* **FALSE — the hover tier (and with it the pressed tier).** As written,
  "hover/pressed tiers unreachable because iced 0.14's `mouse_area` never
  reports hover into a container style, so only `Rest` and `Selected`
  resolve." That is true only of *automatic* style propagation, which is
  not the mechanism. INT-10 captures `on_enter`/`on_exit` on the row's
  existing context-menu `mouse_area` and hands `BaseState::Hovered` to
  `resolve_style` itself; tree rows highlight on hover as of INT-10. The
  `Pressed` tier is feasible on the same area — `on_press` (`:38`) pairs
  with `on_release` (`:45`) — and is deliberately unbuilt as a design call,
  not a primitive gap. See INT-10.
* **FALSE — the selection bar and depth guides.** As written, "the 2px
  selection bar and depth guide lines are `Layer` rectangles needing a
  stack per row", carried as if needing a stack made them impossible. A
  stack per row is exactly what INT-11 built, and it is skipped entirely
  at depth 0. Built.

**The increment plan's description of `right.rs` was wrong** — it contains
no tab strip, only `should_show_panel`, `resolve_anchor` and two ToC
indent constants, both binding **exactly** (4 = `space::XS`, 12 =
`space::MD`). The tab strip lives in `center.rs`.

Tests: 7903 → 7923 (+20).

### INT-7c — Center panel, agent panel, render layer

`f7e5ed1`, all three areas. Closes INT-7.

**Two accessibility defects, every figure independently recomputed from
raw sRGB and matched to two decimals.**

1. **The unsaved-changes dot was invisible on the active tab.**
   `status_warning` on `accent_fill` measures **1.65:1** — the marker sat
   on the very tab the user was looking at. `tab_unsaved_dot_color()` now
   applies the role lock, giving **4.80:1**.
2. **Transcript links failed WCAG AA on every surface they can appear on.**
   The link colour was `Palette::accent`, i.e. `color.accent.fill` — a
   **background-only** token — painted as a foreground. Measured 4.14:1
   on `surface.base`, 3.90:1 on `surface.raised`, 3.83:1 on
   `surface.code`; the floor is 4.5, so it passed nowhere. Now
   `accent_text` at 10.66 / 10.02 / 9.85. The fix is the role change,
   not a fill retune.

**Three off-scale type sizes corrected:** 18, 18 and 15 were not on the
type scale at all.

**The tab strip height was emergent and is now explicit.** No strip-height
constant existed; height fell out of text size plus padding at ~30. Now
`STRIP_HEIGHT = CONTROL_LG` 36 with `TAB_HEIGHT = STRIP_HEIGHT −
2×STRIP_PADDING = CONTROL_SM` 28 — the same band closure INT-7a found for
the navbar. `TAB_PADDING_Y` had to move 4 → 2 as a **consequence, not a
choice**: it is derived as `(TAB_HEIGHT − HIT_TARGET_MIN)/2`, because the
old 4 could not coexist with the spec's 24×24 close target (24 + 2×4 = 32
overflows a 28pt tab). The close hit target moves from glyph-sized and
sub-24 to a real 24×24.
`the_tab_strip_height_is_not_the_sum_of_its_contents` pins 36 > 30 so the
band cannot silently revert to emergent.

**Iced's derived markdown ladder was illegal, five sizes at once.**
`Settings::with_text_size(14)` yields `code = 10.50` (below the 12
floor), `h1` 28, `h2` 24.5, `h3` 21, `h4` 17.5 (all off-scale) and
`spacing` 12.25 (off the base-4 grid). Nobody chose those — they are the
side effect of a multiplication table. `markdown_settings()` now sets
every field by hand to the same ladder `render::heading` produces, so the
transcript and the document viewer render one Markdown at one legal set of
sizes.

Render layer: heading ladder 32/24/18/14/12 → 24/20/16/14/12;
`CODE_LANGUAGE_SIZE` 11 → 12 (was below `MIN_SIZE`); `EMPTY_CELL_HEIGHT`
18 → 20 = `BODY.line_height`; `INDENT_UNIT` 20 → 16 so document and tree
nest the same distance as `tree_row::INDENT_STEP`.

**Spec conflict "resolved" here — the premise was false and INT-11 closed
it for real.** What this record asserted: §4.4.1's 6px
`size.indicator.dot` **cannot be honoured**, because the marker is painted
with a font glyph and §2.4 is absolute that no text renders below 12px, so
the dot is pinned at `MIN_SIZE` 12 with `assert_ne!` against
`INDICATOR_DOT`. The conflict was real only while a 6px *shape* was
believed unavailable. It is available — `Space` with a styled border, 6×6,
radius clamped to a circle. `UNSAVED_DOT_SIZE` now equals
`size::INDICATOR_DOT` (`center.rs:338`), the `MIN_SIZE` workaround is
retired, and so is the `assert_ne!`, which existed only to record a
conflict that no longer exists. Critically, the floor guard keeps its teeth
**by scope rather than by exemption**: `UNSAVED_DOT_SIZE` was removed from
`converted_runtime_sizes()` (`token_guard_tests.rs:362`), the list of
sizes the interface can render *as type*, so
`nothing_in_the_converted_surfaces_renders_below_the_type_floor` and
`every_converted_type_size_sits_on_a_rung` still mean exactly what they
said — a 6pt font still fails them. No call site was exempted. See INT-11.

Also: framework `Button` has **no `.width()` builder** (verified across its
21 `pub fn`s), so the thinking-block disclosure header, which must
stretch the full pane so the whole row is the click target, stays iced's
button. And `agent_panel`/`agent_prompt_row` are **not mounted in the live
view** — `view.rs::agent_session_body` calls `agent_transcript_view`
directly, so the prompt row is currently dead surface.

Tests: 7923 → 7941 (+18).

### INT-8 — Token-lint gate over the app's iced layer + C4 sync

`c2111a9`. Closes the INT series.

**The honest starting count**, measured by landing the gate with an empty
allow-list and letting the real linter dump the audit rather than
estimating with a regex: 157 production files under `src/app`, **31 Rule
B violations** (unbound visual-measurement `pub const f32`), and **zero
Rule A violations** — §4a was already clean by hand across the entire app
crate, zero hex colour literals in app production code, with all 8 colour
constructors inside `ui/theme.rs`. The gate adds enforcement where none
existed and needed no colour exceptions at all.

Resolved as **16 fixed · 3 spec-registered · 12 allow-listed**
(`token_bound` 73 → 89).

Two of the sixteen were real defects, not rebindings: `agent_debug`'s
`TITLE_SIZE` 18 → `TITLE_SM` 16 (18 is on no rung), and `JSON_LINE_HEIGHT`
18.2 → 20.0 — it was `JSON_TEXT_SIZE * 1.3`, which is iced's
undocumented `Relative(1.3)` re-derived by hand **and** a bare `f32` line
height, i.e. the `bb18c2d` class reintroduced by arithmetic instead of by
conversion.

**One scope mechanism, not two.** `VISUAL_SCOPE_DIRS` was *not* widened —
that would change iced-pro's semantics and still do nothing for the app,
whose scan root is iced-pro's own manifest dir. `token_lint` gained a
`Scope` descriptor plus `scan_source_in`, `scan_tree`, `in_scope`,
`lookup_geometry`, `render_scoped`, `crate_scope`; `scan_source` is now a
thin wrapper over `scan_source_in(&crate_scope(), ..)`. A parallel scanner
starts identical and then drifts: fix a false negative in one and the
other keeps reporting what it always reported, and two numbers disagreeing
about the same tree is worse than one wrong in a known way.
`the_app_scope_and_the_framework_scope_agree_on_the_same_shape` asserts
both scopes produce an identical finding for identical input, so a fork in
practice fails the build.

**The allow-list is enumerated, not a grandfather clause.** Twelve
entries, each with its own reason, and `scan_tree` reports any entry that
suppresses nothing, so an entry cannot outlive its reason;
`no_allow_entry_is_a_blanket_exclusion` pins the shape. Each is a number
no token carries at its current value, where binding it to the nearest
rung would be a **rendering change** rather than a token binding —
including two `table_width` FTWA solver inputs where changing 5.0
re-solves every column of every table. Landing the gate was licence to
*name* every place still needing a decision, not licence to re-tune the
app.

`PIN_FILES_SCANNED` (144) and `PIN_RULE_B_FILES` (81) **did not move** —
the machinery was generalised rather than iced-pro's scope changed, so all
31 existing iced-pro `token_lint` tests pass untouched. New app-side pins:
157 files, rule_a 1, rule_b 44, colour_definitions 8, token_bound 89,
surface_bound 3, allow_entries 12.
`this_gate_checks_provenance_not_legality` pins that a token-bound constant
passes whatever its value, so nobody reads a green gate as "the sizes are
legal". INT-7c's `token_guard_tests.rs` is **not** subsumed: that file
proves *legality* of the specific converted surfaces, this gate proves
*provenance* across the whole layer.

**C4 sync: three stale claims were live in the authoritative architecture
picture.** The Level-3 diagram title said *"widgets not yet"* and the prose
said iced-pro is consumed *"only as the design token source"* — both true
at INT-1, both false since INT-3. A third, not previously flagged, said
*"nothing checks for raw literals outside `token/` at source level"* — now
false as of this increment. All three verified at zero occurrences. New
sections record the integration contract, that `SurfaceRegistry` is a
derived index and registration ≠ closability, and seven known integration
gaps.

**Still open after INT-8:** `SPACING` was not added to
`VISUAL_NAME_MARKERS` — it would surface `CONTROL_SPACING`,
`CHEVRON_SPACING` and `FILTER_SPACING` at 6.0 across both trees
mid-increment, so it is recorded as a gap rather than silently widened.
Rule B still reads *declarations*, not expressions, so inline literals,
non-`pub` constants and indirection stay invisible — inherited from the
scanner and restated in the gate's module doc so it cannot be
over-trusted.

Tests: 7941 → 7957 (+16).

### INT-9 — The submenu aiming corridor is consumed

`c1f89fb`. Delivers `UIF-PRIN-008` / `UIF-FMENU-002` — the 150 ms
diagonal aiming grace window — in the running application. The framework
built the corridor back in B-3 and nothing could consume it, because this
ledger's INT-3b record said the row widget could not report hover. It
could; see the correction recorded there.

**Framework: `RowEvent` gains `Hovered` and `Unhovered`.** `MenuRow` reads
the previous `state.hovered` *before* the overwrite and publishes only on a
`false → true` or `true → false` edge — never level-triggered, because the
widget tree rebuilds constantly and a level-triggered report would flood
the reducer every frame. Reported uniformly for all six row kinds, matching
the philosophy already written into that code. Paint, press, capture,
`Activated`, `Absorbed` and `mouse_interaction` are untouched.

**Three premise corrections found by reading rather than assuming.**

1. **iced 0.14 has no `Task::later`** — zero `later`/`delayed`/`after`
   constructors in iced's `task.rs`. The replacement timer,
   `iced::time::every`, lives in
   `iced_futures::backend::native::{tokio,smol}::time`, and this app runs
   the default thread-pool backend, whose `time` module is *literally*
   empty: `iced_futures-0.14.0/src/backend/native/thread_pool.rs:20` is a
   module declaration with nothing in it. So the wake-up sleeps on the
   worker the task already runs on — at most one sleep per hover crossing,
   each bounded by `grace_ms` (150 ms), on a pool sized to the CPU count,
   and never on the UI thread, which is what `FMD-060`/`FMD-061` actually
   require. A wake that arrives after its reason has gone is inert because
   `on_deadline` re-reads the corridor.
2. **`hover_submenu_row` already calls `aim.branch_opened` /
   `branch_closed` itself** (`cascade.rs:802-808`). The app does not,
   because that would be a second owner of the same fact.
3. **Three existing framework tests could not pass literally unchanged.**
   A press event carries the cursor into the row, so the hover edge
   necessarily lands on that frame. Expected vectors updated only;
   activation semantics unchanged and still pinned.

**The app driver is `menu_aim.rs`, pure with respect to time:** every entry
point takes `now: TimeMs`, so the 150 ms window is verified by arithmetic
rather than by sleeping and cannot be flaky on a loaded machine.
`TimeMs::now()` is called in the reducer, never in a resolver — the exit
§2.6 was written for. The app had no time source at all before this: no
`TimeMs`, no timer, no clock.

**Three rules.** *Only a submenu parent opens anything.* *A pointer leaving
never dismisses* — the corridor answers `Collapse` when the pointer is
outside everything, and that answer is dropped, because dismissal belongs
to the click-away blocker `menu_surface::overlay` mounts. `pointer_left_card`
says the same about itself in the same words: a corridor that closed
surfaces on its own would give one menu two owners. Without this rule every
menu in the app would shut when the mouse swept past it. *A held switch
must mature without another event* — iced is event-driven, the corridor is
not, so `wake_task` is what stops a `Hold` waiting on a mouse jiggle that
may never come.

**A spec drift was flagged rather than fixed, and the spec was taken.**
§4.8.2 (`spec.md:1452`) requires that moving to a sibling row on
an ancestor card retires the deeper child branch after the corridor check.
The first cut left it unimplemented and flagged it as a judgement call. The
distinction that makes the rule safe — and that the first cut's hesitation
was conflating — is that **retiring a BRANCH is not dismissing the MENU**.
Rule two still forbids the hover path from dismissing: the root card stays
up and click-away still owns closing it. What goes is the child card the
pointer walked away from, which is precisely what the corridor was built to
time.

Two things make the rule precise that a naive reading gets wrong, both
found because existing tests caught them. **The ancestor-card qualifier is
load-bearing:** a row *below* the root is inside the branch the user is
aiming at, so retiring on that makes arriving at the target be what closes
it — the exact opposite of the diagonal manoeuvre the corridor exists to
serve. The first cut of the fix did exactly that and
`test_the_pointer_inside_the_open_child_branch_preserves_the_branch`
failed; the retire now keys off `target_for`, which already draws that
line, rather than re-deriving depth. And **the retire drains to the root**
rather than closing one level, so the rule stays honest if a deeper menu
ever appears, and it clears the corridor unconditionally on a real retire —
a guard left pointing at a row whose branch is gone is the stale-guard
failure `guard_is_live` exists to catch, and here it would have been caught
one hover late.

**Still open: one corridor, one level.** The corridor tracks a single parent
card's trigger rows by index, so the root card is the parent and everything
below is `ChildBranch`. Correct for the hamburger, which nests one level
deep. A two-level menu needs one corridor per parent card — a shape change,
not a constant. Documented in the module header.

Tests: 7957 → 7992 (+35). `view_tests` +6 — hover edge fires exactly
once, repeats are silent, re-entry re-reports, all six row kinds, hover
does not disturb activation, `Cursor::Unavailable` reads as leaving.
`menu_aim` sidecar +21 — holds before 150 ms, holds at grace − 1, matures
at grace, an early wake re-arms rather than being lost, `Preserve` on the
trigger's own row, `ChildBranch` preserves, stale guard cleared,
non-submenu opens nothing, pointer-out closes nothing, a closed tree owes no
wake; plus five new retire tests — crossing to a plain sibling holds before
retiring, the crossing retires once the window elapses and stops at the
root, retiring with nothing open is a no-op, a retire leaves the menu open
so dismissal stays with click-away, and a retire frees the corridor so the
next crossing is not held. `update_tests` +8 — hover opens without a click
and publishes no command, a held switch owes a wake proved by a `+1`
task-unit delta against a `Preserve` control, the held switch completes on
the real clock, pointer-out never dismisses, a late wake cannot resurrect a
closed menu, the hamburger button is still click-to-open and hover cannot
open it from closed, reopening clears a stale guard.

Gates: `cargo check --workspace --all-targets` clean · `cargo nextest run
--workspace` **7992 passed, 6 skipped** · `cargo clippy --workspace
--all-targets -- -D warnings` clean · `cargo fmt --check` clean ·
`cargo doc --workspace --no-deps` after `cargo clean --doc`: **0 warning
lines**. Pins: iced-pro `PIN_FILES_SCANNED` 152 / `PIN_RULE_B_FILES` 81
unchanged — no new files under `widget/`, `surface/` or `composite/`; the
app-crate gate needed bumping, `PIN_APP_FILES_SCANNED` 157 → 158 and
`PIN_APP_RULE_B_FILES` 44 → 45, from `menu_aim.rs`. Token-bound and
surface-bound counts did not move; the module declares no visual scalar.

### INT-10 — Tree rows highlight on pointer hover

`f1e442b`. The file tree's hover tier existed in code and was
unreachable. Now it is reached, and the reason it was believed impossible
is corrected.

**Verified broken before building, not assumed.** Traced end to end first,
with four independent confirmations: `row_paint` hardcoded
`BaseState::Rest`, the only `BaseState` the panel ever passed to
`tree_row::resolve_style`; the style closure painted a background only when
selected, so even a hover-resolved style would not have been drawn;
`context_menu_trigger` built `mouse_area(content).on_right_press(..)` with
no `on_enter`/`on_exit`; and a grep for `on_enter`/`on_exit`/`on_hover`
across `src/app` returned zero hits in the entire app crate. The
`Status::Hovered => BaseState::Hovered` mapping cited as evidence that a
hover tier existed is real but belongs to the wrong component — it is
`search_field_style`, the search `text_input`, driven by iced's own
`text_input::Status`. A hover tier existed; it just was not the tree's.

**Mechanism.** `mouse_area` `on_enter`/`on_exit` on the row's existing
context-menu trigger (`overlay.rs:624`) → `TreeMessage::RowEntered{path}` /
`RowLeft{path}` → `OverlayState::hovered_tree_row` →
`LeftPanelViewModel::hovered_row` → `row_paint(selected, hovered)` →
`BaseState::Hovered` → `color.surface.hover`. Edge-triggering is provided
by iced rather than hand-rolled, which makes this simpler than INT-9:
`mouse_area` publishes `on_enter` only on `is_hovered && !was_hovered` and
`on_exit` only on the inverse, and skips the check entirely when neither
the cursor nor its own bounds moved (`mouse_area.rs:349-366`). INT-9
needed a manual previous-value capture because `MenuRow` drives its own
state machine; here the app receives already-clean transitions.

**Two details that turned out load-bearing.** `RowLeft` carries the path so
the clear is conditional: both crossings of a row-to-row move publish in
one iced pass in tree order, so moving the pointer *upward* delivers the
lower row's `RowEntered` before the upper row's `RowLeft` — a bare clear
would wipe the new highlight the tick it appeared
(`leaving_a_different_row_cannot_wipe_the_highlight_that_just_arrived`).
And a prune for rows that stop existing: a deleted or filtered row's
`mouse_area` is gone before it can say goodbye, so `prune_hovered_row`
runs in the frame tail after the tree cache refresh.

**A contradiction with the brief, and it mattered: the live tree is NOT
virtualized.** `left.rs:1266-1273` folds every `FlatRow` into a plain
`Column` inside a `scrollable`; every row is constructed every frame, and
`virtual_list` appears nowhere in the app crate outside a doc-comment
inventory. Classic slot recycling does not occur. The real risk is
re-ordering under expand/collapse, filter and watcher refresh, and keying on
`path` handles it — which matters precisely because iced 0.14 reconciles
widget `Tree`s by position, not by explicit `Id`
(`iced_core-0.14.0/src/widget/tree.rs:86-107`), so an explicit `.id()`
cannot make hover follow a file. `row_is_hovered` refuses to consider
position at all.

**One honest residual, and it fails the right way.** If a *different* file
lands at byte-identical bounds under a stationary pointer, iced skips
re-evaluation and never fires `RowEntered` for the new file. The old file's
own `mouse_area` does re-evaluate, because its bounds moved, and fires
`RowLeft`, so state self-corrects to nothing highlighted. The worst case is
a *missing* highlight until the pointer moves — never a highlight on the
wrong file.

**Contrast computed from raw sRGB, not transcribed.** `text_primary` on
`surface_hover` `RGB(32,32,36)` measures **10.7380:1** against the 4.5
floor; the worst case, `status_danger_text` on `surface_hover`,
**5.3633:1**; `text_strong` on `surface_hover` **16.2351:1**; resting
baseline `text_primary` on `surface_base` **13.1590:1**. The §3B.6
contrast lock is a *computed no-op* on hover rather than an absence of
handling: `fill_lock_role(surface_hover)` is `Unlocked` — it is not one of
the four locked solid fills — so `locked_over` returns the candidate
untouched and the PDF error tint survives the pointer. That role is
asserted directly rather than inferred. Selection still outranks the
pointer: a selected-and-hovered row keeps `accent_fill` and locks to
`text_strong`.

**The `Pressed` tier: a false claim corrected before commit.** The first
cut recorded it as "genuinely blocked" because `mouse_area` supposedly
"offers no matching release callback, so a pressed band would be set and
never cleared". That is false — `mouse_area` pairs `on_press`
(`mouse_area.rs:38`) with `on_release` (`:45`). The tier is **feasible and
deliberately unbuilt**, because a press-reactive row changes the panel's
primary affordance and that is a design call rather than a missing
primitive. Committing the original wording would have left a false gap in
the code for the next person to inherit.

**Part C — the other "impossible because iced can't" claims audited, none
built in this increment.** The tab-strip 2px top accent indicator: wrong,
same fallacy — needs neither a hover-reporting row nor a canvas. The 6px
unsaved dot: half right — the glyph reasoning is sound, but "needs a
canvas" is not. The tree 2px selection bar and depth guides: overstated —
the stack-per-row is feasible. The strip's 1px `border.subtle` bottom rule:
**largely correct, and it is not a hover claim** — the blocker is the 36px
height budget, a real arithmetic constraint. `widget::toggle`
"resolver-only" is not an iced limitation at all; the module itself says it
was out of scope, so a real `Element` producer is feasible by this same
pattern — a gap in framework coverage, not capability. Canvas specifically
*is* genuinely unavailable (`iced/canvas` is not in the app's feature
list), but none of the three pending indicators need it. All three were
built in INT-11 except the bottom rule.

Tests: 7992 → 8009 (+17) — 8 in `left_tests`, 9 in `update_tests`; the
baseline was measured in a throwaway worktree rather than taken on trust.
Both Logs `Esc` pins re-verified unchanged.

Gates: `cargo check --workspace --all-targets` clean · `cargo nextest run
--workspace` **8009 passed, 6 skipped** · `cargo clippy --workspace
--all-targets -- -D warnings` clean · `cargo fmt --check` clean ·
`cargo doc --workspace --no-deps` after `cargo clean --doc`: **0 warning
lines**. No new files under iced-pro or `src/app`, so 152/81 and 158/45
are untouched.

**Process incident.** The first attempt to append test blocks used
PowerShell `Add-Content -Encoding utf8`, which double-encoded whole files
— em-dashes became mojibake and a reverse-transform then destroyed
box-drawing characters in `update_tests.rs`. Both files were restored from
HEAD and re-applied with the `edit` tool, then verified free of `U+FFFD`
with clean, distinct non-ASCII. Appending code with a shell encoding
switch is not a shortcut; it corrupts at a distance.

### INT-11 — The three indicators previously declared impossible

`18b5d16`. All three pieces landed. INT-7b and INT-7c had written these
off as needing a canvas painter that does not exist; INT-10's Part C audit
proved that wrong and this builds on it.

**Piece 1 — the 6px unsaved dot: the §4.4.1 conflict is closed, not
re-worked-around.** INT-7c had to *violate* §4.4.1 to land this marker,
pinning it at `MIN_SIZE` 12 with an `assert_ne!` against `INDICATOR_DOT`
on the reasoning that taking the token literally meant either breaking the
12px floor or exempting one call site and gutting the guard. That was a
real conflict only because a 6px *shape* was believed unavailable.
`UNSAVED_DOT_SIZE` moves `MIN_SIZE` (12) → `size::INDICATOR_DOT` (6)
(`center.rs:338`); the floor workaround is retired and so is the
`assert_ne!`, which existed only to record a conflict that no longer
exists. The floor guard keeps its teeth **by scope rather than by
exemption** — `UNSAVED_DOT_SIZE` is removed from
`converted_runtime_sizes()` (`token_guard_tests.rs:362`), the list of
sizes the interface can render *as type*, so a 6pt font still fails
`nothing_in_the_converted_surfaces_renders_below_the_type_floor`. No call
site was exempted.

**Piece 2 — the 2px top accent indicator fits as a zero-layout overlay.**
`TAB_INDICATOR_HEIGHT = shape::BORDER_FOCUS` (2) (`center.rs:361`), a
`Stack` per tab: base is the existing mouse-area-wrapped tab, overlay is a
full-width 2px `container`. The bar consumes no layout, so the pinned band
arithmetic is untouched — `STRIP_HEIGHT 36 = STRIP_PADDING 4 + TAB_HEIGHT
28 + STRIP_PADDING 4`, unchanged; `TAB_INDICATOR_HEIGHT 2 <= TAB_PADDING_Y
2`, an exact fit; the bar occupies `y ∈ [0,2]` and the tab content box
starts at `y = 2`, so it cannot reach the label. The `36 > 30`
content-floor pin holds. Inactive tabs resolve to `None` rather than a
transparent bar, so no partial or leftover indicator is reachable.

**Piece 3 — tree selection bar and depth guides.** `row_guide_offsets`
(`left.rs:1495`), `RowStripe` (`:1506`), `RowOverlay` (`:1523`),
`row_overlay` (`:1571`), `stripe_element` (`:1599`). Colours come straight
off `row_paint()`'s already-resolved `TreeRowStyle::selection_bar` and
`::guide` — nothing re-decides them. Bar width is
`tree_row::SELECTION_BAR_WIDTH` as-is (`tree_row.rs:165`) rather than a
minted parallel constant; guide width is a new `tree_row::GUIDE_WIDTH`
(`:174`) that `row_slots` now reads too (`:354`), so app and composite
share one source. `guide_positions` (`:287`) was made `pub` for this.

**Threshold determination: `MIN_CHROME_CONTRAST` 3.00:1 applies, not
4.5.** The dot is a graphical object whose shape carries the state, not
readable text — `UIF-TOK-006` rule 3 / WCAG 1.4.11. §2.4's floor is
explicitly about text and never reaches a box. Both live pairings clear
3.00 by a wide margin *and* clear 4.5, so the choice of floor changes no
verdict — which is what should be true of a genuinely resolved conflict. On
the active tab the question is moot anyway: rule four's role lock forces
`text.strong` regardless of measurement. The INT-7c win over the old
1.65:1 survives the shape change.

**Contrast recomputed from raw sRGB — all seven figures re-derived
independently, each matching the reported value to six decimal places.**
`text_strong`/`accent_fill` **4.8020:1** (active dot, selected bar and
guide — role-locked); `status_warning`/`surface_base` **6.8555:1**
(inactive dot); the retired `status_warning`/`accent_fill` pairing
**1.6547:1**, now pinned illegal; guide `border_strong`/`surface_base`
**4.5062:1**; the worst case, guide `border_strong`/`surface_hover`,
**3.6772:1**; and the two figures that explain why the stripes lock by
role rather than by measurement — `border_strong`/`accent_fill`
**1.0876:1** and `accent_border`/`accent_fill` **1.6098:1**. Every live
pairing clears the 3.00:1 chrome floor, and the §3B.6 lock holds at every
depth × posture.

**Per-frame cost reported honestly and pinned** (`RowOverlay::widget_cost`,
`left.rs:1546`): depth 0 unselected costs **0** — the `Stack` is skipped
entirely. Depth `d` unselected costs `1 + 2d`; depth `d` selected costs
`2 + 2d`. Against a ~3-widget baseline row, a depth-2 selected row goes 3 →
9. The tree is not virtualized, so this is paid per visible row per frame.
The two-per-guide cost is forced: iced's `Stack` lays every child at its own
top-left origin with no per-child alignment
(`iced_widget-0.14.2/src/stack.rs:193-205`) and `Container` has no
`margin`. A uniform-pitch `Row` would save N − 1 widgets per row; the
obviously-correct per-stripe wrapper was chosen over a layout that could
drift from the geometry.

**How INT-10's hover survived a stack.** The stripes are *siblings* of the
row's `mouse_area` in the `Stack`, not children of it. iced dispatches
events without occlusion culling and a plain `container` has no event
override, so enter/exit/press/double-click/right-press all still land on
the area underneath. Every INT-10 hover test passes unchanged — hover
highlight, the path-keyed recycling matrix, the conditional `RowLeft`
clear, the prune, idempotent re-entry. Both Logs `Esc` pins re-verified.

**Corrections to the brief.** `container().width(6).height(6)
.border_radius(3)` is not literally valid in iced 0.14 — there is no
`iced::widget::empty` and no `Container::border_radius`. Radius lives on
`container::Style.border.radius`, and `draw_background` passes that whole
`Border` to the fill quad (`iced_widget-0.14.2/src/container.rs:446`), so
the fill rounds even at `width: 0.0`; `Space` is the empty child.
`border_radius(3)` would also have been a bare literal that Rule B reads as
a `*_RADIUS` declaration, so `shape::RADIUS_PILL` (999) was used instead
— iced clamps every corner to `min(w,h)/2`
(`iced_graphics-0.14.0/src/geometry/path/builder.rs:173`), which over a
6px box is 3. Identical render, fully token-driven, no violation. And
**§4.4.1 slot 1 has three states, not active-only**: the spec also requires
`border.strong` on a hovered *inactive* tab. Built active-only per brief;
the hovered state is flagged as unbuilt rather than silently added or
silently dropped, because it needs the tab strip to report hover into the
reducer — new interaction plumbing outside this piece.

**Pins resisted.** `STRIP_HEIGHT` 36 and `TAB_HEIGHT` 28 were not
re-derived to make room, because the overlay made room unnecessary.
`MIN_SIZE` 12 was neither lowered nor exempted. `MIN_CHROME_CONTRAST` 3.0
and `MIN_TEXT_CONTRAST` 4.5 untouched. `SELECTION_BAR_WIDTH` used as-is.
Guides clamp to the existing `MAX_ROW_INDENT_DEPTH` (`left.rs:272`) rather
than a new cap. The only pin removed is the `assert_ne!` that recorded the
now-closed conflict. Pin *counts* moved as counts, not values: iced-pro
`PIN_TOKEN_BOUND` 192 → 193 (`tree_row::GUIDE_WIDTH`), app
`PIN_APP_TOKEN_BOUND` 89 → 91 (`UNSAVED_DOT_RADIUS`,
`TAB_INDICATOR_HEIGHT`); `PIN_APP_FILES_SCANNED` 158 and
`PIN_RULE_B_FILES` 81 unchanged — no new files.

**Still open.** Tab-strip hover reporting for the indicator's third state.
The strip's 1px `border.subtle` bottom rule, genuinely blocked by the 36px
budget rather than a missing layer — a bottom-only hairline needs a
`rule::horizontal` sibling inside the band. The `[PDF]` badge on tree rows,
still the missing piece behind the selection lock erasing the `error` tint.
Width-relative indent compression via `tree_row::indent_geometry`
(`tree_row.rs:234`) — the app still draws a flat clamped `padding-left`,
and the new guides read the app's clamped depth, so if compression is
adopted `row_guide_offsets` must switch to
`indent_geometry(depth, row_width)`. Tree `Pressed` tier, unbuilt by
INT-10's stated design decision rather than a primitive gap.

Tests: 8009 → 8022 (+13) — `center_tests` +6 / −1 (the removed test was
`unsaved_marker_is_the_dot_only_for_dirty_documents`, superseded by the
shape-based pins), `left_tests` +8, `token_guard` renamed only.

Gates: `cargo check --workspace --all-targets` clean · `cargo nextest run
--workspace` **8022 passed, 6 skipped** · `cargo clippy --workspace
--all-targets -- -D warnings` clean · `cargo fmt --check` clean ·
`cargo doc --workspace --no-deps` after `cargo clean --doc`: **0 warning
lines**.

### The INT series — what the integration work established

Twenty-two records spanning INT-1 through INT-15 plus `UIF-MOD-004` and
the token-lint continuation fix; 7724 → 8083 workspace tests (+359),
every gate independently re-run at every step with `--workspace` and
doc-warning lines counted explicitly. INT-9 through INT-11 alone are
7957 → 8022 (+65); INT-12 through INT-14 are 8022 → 8048 (+26);
INT-15, MOD-004 and the lint continuation fix are 8048 → 8083 (+35).

**Two headlines from the last round, and they are the same failure in
different clothes.** First: a spec rewrite landed mid-stream and the
conformance gate caught the move only by going **fully red** — seven tests,
one panic message, because every coverage test opens the spec file before it
can assert anything. That is the gate working, but only after the fact, and
only because a red gate is loud. The spec now lives inside the crate it
governs (`src/hoarfrost/spec.md`); see the record at the head of §3c.
Second: a lint gate was **passing code it could not read** — a wrapped `f32`
const was classified `TokenBound` without its value ever being examined, and
the count of "compliant" constants went up as though nothing had happened.
The lesson generalises past this parser: a compliance check must distinguish
"compliant" from "unread", or its green means nothing. That is the same
class of failure as the "impossible because iced can't" claims of
INT-9..11 — a confident verdict delivered without looking. The difference
is that those were written by a person and this one was written into the tool,
which makes it quieter and longer-lived.

**One ownership contract emerged and held across every surface.**
`ModalHost` and `InputDialogHost` self-register into `SurfaceRegistry` and
push a focus trap, which collides with `OverlayState::sync_surfaces` /
`close_topmost` ownership; both are refused. `MovableHost` registers its
control roster **without** a trap, so it *is* adopted and the sync
converges with it. The rule for the next contributor: take framework
sizing, frame and semantics; keep app-side open/close/dismiss ownership;
never create a second writer to the surface index.

**`UIF-MOD-001`'s focus trap is unwired everywhere and is not claimable
anywhere.** Only `Escape` reaches the app (`subscription.rs:442`); iced's
router owns `Tab`. Every increment refused to claim it rather than writing
a trap that iced would ignore.

**The plan was wrong about six named components.** `ChipVariant::Success`,
`widget::toggle` as an `Element` producer, `form_field::stacked` for
multi-row, `composite::search_input`, `bridge::apply_transition` /
`advance` / `sync`, and `right.rs` as the tab strip. Each was caught by
verifying against the files before wiring rather than by trusting the
brief. INT-11 added two more of the same kind, this time aimed at iced
itself rather than at this repo: `iced::widget::empty` and
`Container::border_radius` do not exist in 0.14, so a snippet written with
both in it cannot compile.

**Ten user-visible defects were found by the integration work that the
framework's own test suite did not catch** — because the framework tests
test resolvers, and the defects only appear when something actually paints:
blank menus (`bb18c2d`), disabled-`Ok` dismissing a dialog, silent loss
of unsaved editor work, PDF-label 1.59:1, transcript links failing AA on
every surface, the dirty dot invisible on the active tab, submenu parents
that needed a click because the aiming corridor never ran (INT-9), tree
rows that never highlighted on hover (INT-10), and the unsaved marker
rendered at 12px against the spec's 6px with neither the tab-strip accent
bar nor the tree selection bar and depth guides present at all (INT-11).
The tenth is different in kind from the other nine: it was **ruled to be
accepted rather than fixed**. The `[PDF]` chip mounts only where there is
room, and the room condition reduces to `pane >= 176 + 16 × depth`, so at
the app's own minimum pane width every *nested* PDF-backed file is
visually identical to an ordinary file, and at the spec's minimum even the
top-level ones are (INT-15). It is counted here rather than dropped because
a user at a narrow pane still cannot tell a PDF-backed file from any other
whether or not anyone decided that was acceptable; what the ruling changed
is the response, not the consequence.

**The last six increments found seven more defects — and every one of them
is in the spec or in the checking machinery rather than in the app.**
(1) `spec.md:883`'s mandated chip fill measures `1.067676:1` against the
selection band it exists to survive, and no danger or warning token in the
palette reaches `3.00` there — the rule asks for something the palette
cannot express (INT-12, ruled and built in INT-13). (2) The framework's
`3.00` coincident-fill obligation is **declarative only**:
`gallery_audit::judge()` (`gallery_audit.rs:152`) returns `true`
unconditionally for every class except `Judged`, so nothing in the
enforcement path ever compares a fill against its mount (INT-13). (3) The
token-lint gate blessed declarations it could not read as compliant (found
INT-13, fixed INT-14). (4) That fix surfaced a literal that had shipped
invisibly inside `TAG_TRIGGER_VERTICAL_PADDING` (`top.rs:309`) (INT-14).
(5) The spec's own two numbers for the attributions card contradict each
other: the card is pinned at `480px` and the list capped at `200px`, the
furniture takes 150px, so 130px of a fixed-size dialog is dead space and
about 4.4 visible rows are given back for nothing (MOD-004). (6) The
token-lint gate had a **second** unread-expression hole one level deeper
than (3), and the fix that closed (3) did not find it — `join_continuation`
returned the first line whenever it was non-empty, so a partial first line
hid everything below it (lint fix, `e09a94d`). (7) That second hole had
already let a literal ship unread: `SEARCH_ROW_HEIGHT`
(`ui_iced/panels/left.rs:481`) carried two `2.0` multipliers that no
check had ever seen.

A related finding that is neither gate nor spec but belongs with them:
`pdf_badge_fits` carried a doc comment asserting `UIF-PANE-002` *"normally
removes the conflict by widening the pane"*. It does not — `auto_fit_width`
charges `tree_row::content_width` with no badge term at all. A false claim
inside the code is worse than a missing one, because the next contributor
reads it as settled (INT-15; corrected in place and now disproved by test).

**The class, generalised — and it has now appeared three times in three
different subsystems.** A verdict delivered without looking. In INT-9..11
it was what we *asserted about iced*: four capabilities written off as
unavailable, all four present, each claim cheaper to repeat than to test.
In INT-14 and its second half it was written **into the tool**: a
compliance check that cannot distinguish "compliant" from "unread" reports
the same number for both, so its green is not evidence of anything — and
the second half is the damning one, because it survived the fix that was
supposed to close the first. And in the spec itself it is a number that was
never checked against the thing it governs: a chip fill the palette cannot
produce, and a card height and a list cap that were each written down
without anyone adding them up. The three are the same mistake wearing
different uniforms, and the second is the one that should worry a reviewer
most: a person's false claim gets argued with, but a gate's false pass is
downloaded as fact by everyone who trusts the count. The practical rule
this ledger now carries is that **a passing check must be shown to be able
to fail** — every fix in this series is accompanied by a non-vacuity run
against the reverted code, because a test that only ever sees fixed code
proves nothing.

**The headline of the last three increments: the "impossible" claims were
the blocking defect, not the missing code.** Four separate capabilities
were written off as unavailable in iced 0.14 and all four turned out to be
there — `MenuRow` driving hover and aim (asserted in INT-3b, disproved by
INT-9), the tree hover tier (asserted in INT-7b, disproved by INT-10), the
tree `Pressed` tier (asserted in INT-10's first cut, corrected before that
same commit), and a 6px shape indicator plus a 2px overlay bar said to
need a canvas painter (asserted in INT-7b and INT-7c, disproved by
INT-11). The pattern is the same in every case: the claim confused
*automatic style propagation*, or the absence of a convenience API, with a
missing primitive. iced does not push hover into a container style on its
own — nothing stopped the app from capturing the crossing itself and
passing it in. iced has no `Task::later` — nothing stopped a sleep on the
worker thread the task already occupied. `Container` has no
`border_radius` builder — the radius was sitting on the style the whole
time, and `draw_background` hands that whole `Border` to the fill quad.
Each false claim was cheaper to repeat than to test, which is exactly why
one survived months of "blocked" status. That is why the corrections are
recorded here at the point of original assertion and not only in the
increment that disproved them: a gap that is stated in the ledger becomes a
premise the next contributor inherits.

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
- **G-14 — the spec's §5 numbers no requirement, but the crate cited one.**
  §5 is headed with the family wildcard `UIF-ZORD-*` and assigns **no numbered
  IDs**, yet `gallery.rs` cited `UIF-ZORD-001` — an ID the spec never created.
  Caught by the INC-22 audit (the extractor requires digits, so the phantom was
  invisible to the coverage count but visible to a reader). **Corrected:** the
  gallery now cites `UIF-ZORD-* + UIF-WIN-003`, matching how every other module
  in the crate refers to the band family.
  The underlying spec gap is real: the four-band layering rule is one of the most
  load-bearing requirements in the document and has **no numbered requirement ID
  of its own**, so it cannot be cited, traced or gated by ID. `UIF-WIN-003`
  covers only the Escape-unwinding consequence. The spec should assign numbered
  IDs to the band definitions themselves when revised.

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

### D-8 — Framework owns list/table **row and cell rendering** (revises D-2)

**Made 2026-10-22 after the second revision.** D-2 originally left the
tree, tab strip and Batch/Tools dialogs to the app. The `e6ecbfb` revision
added `COMP-006` (`VirtualList`/`TreeView`) and `COMP-007` (`DataTable`),
which sit directly on that boundary, and the decision taken is that the
framework owns **more** than D-2 allowed:

| Layer | Owner |
|---|---|
| Virtualised viewport, row height, indent step, disclosure chevron, selection model and keyboard choreography | **Framework** |
| **Tree row rendering** | **Framework** |
| Sticky header, row height, 10px column-resize grab, cell focus and traversal, live-then-commit resize | **Framework** |
| **Table cell rendering** | **Framework** |
| Node model, filtering, `[PDF]` badge semantics, column definitions, sort comparators | App |

**Consequences to carry:**

1. `COMP-006` and `COMP-007` are `Partial`, **not** `NotOwned`. The
   framework owes real, reusable code for both.
2. **This implies eventually migrating `fastmd`'s existing directory
   tree** out of `src/app/ui/tree/` onto the framework's row renderer.
   That is a cross-crate change and is *not* part of the current
   standalone-first constraint (D-3) — it is a follow-on project that
   D-8 creates.
3. The row/cell renderer must be generic enough to serve the navbar filter
   dropdown, the tree, the Tools dialog's six-column table and the
   autosuggest flyout, because those are four places the same shape recurs.
   Designing it for one of them alone will force a second renderer later.
4. Row rendering in the framework still draws **only** token-resolved
   paint. It takes content as data (label, icon, badge state) and never
   reaches into application domain types.

### D-9 — Composite widget-binding pass & framework boundary absorption (revises D-2)

**Made 2026-10-27 after framework boundary convergence.** D-2 originally classified
higher-level composite bindings, split dividers, tab strip integrations, geometry solvers,
and microcopy validations as app-owned or deferred. Following the systematic
implementation of Phase B and Phase C composites and the comprehensive 92-item spec ledger
review, the framework directly absorbs the remaining presentation, layout, and ergonomic
responsibilities:

| Subsystem / Feature | Absorption Details | Owner |
|---|---|---|
| **SplitDivider** (`UIF-COMP-016`) | Draggable split handle with 10px grab zone, pure geometry solvers (`split_divider_geometry`), live-vs-commit drag state, and collapse thresholds | **Framework** |
| **TabStrip binding** (`UIF-TAB-001`–`003`) | Framework-owned tab strip model with dynamic width clamping (64–240px), close button hover hitboxes, unsaved indicator dot, and tab context menu | **Framework** |
| **TreeView search footer** (`UIF-PANE-003`) | Framework-owned search filter bar, integrated 28px search input, next/prev navigation buttons, and match count badge | **Framework** |
| **Geometry & Window Solvers** (`UIF-WIN-001`, `UIF-PANE-001`–`003`) | Pure clamped window layout solvers, three-pane column width allocation, DPI scaling, and headless storage traits (`WindowStorage`) | **Framework** |
| **Navbar Label & Microcopy Gate** (`UIF-NAV-002`) | Framework-enforced ellipsis convention (`...` iff prompt) on navbar menu triggers and icon-only tooltips via invariant gate | **Framework** |
| Shell window lifecycle, real OS window handles, and persistence | OS window handle management, actual config file persistence, and backend sync | App |

**Consequences to carry:**

1. **Zero `NotOwned` items**: With `UIF-NAV-002` absorbed into the framework's invariant
   gate and navbar composite model, all 92 spec items are now owned by `iced-pro` (89
   `Implemented`, 3 `Partial` for shell window lifecycle and host pane composition,
   and 0 `NotOwned`).
2. **Framework boundary complete**: The framework provides complete visual, geometric,
   and microcopy primitives for all desktop surfaces; host applications retain only
   surface persistence, real OS window handles, and business domain logic.

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

### INT-12 — Tree `[PDF]` badge, width-relative indent compression, tab-strip hover

Two of the three signals INT-11's Part C audit identified as landable are in.
The third is blocked, and the block is in the spec rather than in the code.

**Piece 2 — width-relative indent compression: landed, and the rewritten spec
mandates exactly this.** `row_indent_pixels(depth, row_width)` and
`row_guide_offsets(depth, row_width)` both resolve through
`tree_row::indent_geometry` rather than the retired flat clamp
(`depth.min(MAX_ROW_INDENT_DEPTH) × INDENT_STEP`). `spec.md:882` requires the
shared solver — *"Depth guides and row furniture MUST resolve from the same
pure indent solver that lays out the row label … never a fixed nominal
`16px x d`"* — and `spec.md:880` gives the rule the solver implements:
`indent_budget = (row_width - 128px).max(0)`, nominal step `16px`,
proportional compression to a `4px` floor, then collapse onto the innermost
drawn guide reporting `collapsed_levels: u16`. INT-11's coupling note is
honoured: guides read `indent_geometry().guides`, not
`guide_positions(depth, INDENT_STEP)`, so they cannot drift from the labels.

The old clamp was not merely suboptimal — at `50 × 16 = 800px` it pushed
labels off-screen and made the `96px` floor false past roughly eight levels.

**Worst-case floor arithmetic (`at_least_ninety_six_pixels_of_label_survive_the_worst_case`).**
Pinned at the spec's own `LEFT_PANE_MIN_W = 150` (§4.3.1), not the app's
wider `LEFT_PANEL_MIN_WIDTH = 180`, so the spec's worst case is checked and
the app's own minimum is strictly safer:

```
row_width        = 150 − PANEL_PADDING(4) − PANEL_PADDING(4) = 142
indent_allowance = (142 − 4 − 4 − 24 − 96).max(0)           =  14
drawn indent     = allowance (fully compressed at depth >= 8) =  14
label            = 142 − ROW_PADDING_X(4) − ROW_PADDING_X(4) − 14 = 120
120 >= MIN_LABEL_WIDTH(96)                                   24px spare
```

Eight nominal levels would want `8 × 16 = 128px` against a `14px` allowance,
so the compression is doing real work at this width rather than merely
happening not to overflow. Checked at depths `0, 1, 8, 50, 200, 10_000`.

**Piece 3 — tab-strip hover: landed.** `TabMessage::Entered { path }` /
`Left { path }` → `OverlayState::hovered_tab` → `CenterPanelViewModel`, with
conditional-clear arms in `reduce_tab` mirroring `TreeMessage::RowLeft`. The
hover is **path-keyed, not index-keyed**: index-keying would reproduce
INT-10's stale-address bug under `UIF-TAB-003` external closes, where a tab
removed at index 2 silently moves the highlight to whatever slides into that
slot. State is held in the reducer rather than read from iced `button` status
because the tab also needs middle-press and right-press, which `button`
cannot provide, and §6 requires branching on the view-model. `TabMessage` is
consequently `Clone`, not `Copy`, to carry the `PathBuf` — its former `Copy`
classification rested on a premise ("every payload field of these four
channels is `Copy`") that this change made false. `STRIP_HEIGHT 36` /
`TAB_HEIGHT 28` and the band-closure arithmetic are untouched.

Scoped to the top-bar third state. §4.4.2 additionally mandates a
`surface.hover` fill, `text.primary` label and `border.strong` perimeter on a
hovered *inactive* tab; those are deliberately not wired, and the plumbing now
makes them a small follow-on rather than a redesign.

**Piece 1 — the `[PDF]` badge: landed against the old spec, superseded by the
rewritten one, and blocked.** The badge was built as a trailing foreground
mark coloured via `row_label_colour(selected, hovered, true)` —
`status.danger.text` at rest, `text.strong` when selected — on the reasoning
that a chip would make the section's `4.80:1` figure wrong. `spec.md:883` now
forbids that mechanism outright: the distinction *"MUST NOT ride on badge text
color"* and must instead be a chip on `status.danger.fill` behind
`text.strong`. The landed badge is therefore non-compliant in **both**
postures: at rest it is a danger-coloured text label, and on selection it
resolves to the identical ink as the label beside it, so it carries no
distinction where the rule is about.

**Superseded by INT-13.** The block recorded below stood until the spec author
ruled on it. The measurement here is what that ruling was made from and is
unchanged; the chip INT-13 builds is the answer to it. Read the rest of this
piece as the analysis, and INT-13 as the outcome.

Two independent blockers, both verified against the resolver's own output:

1. **The badge atom cannot produce the mandated chip.** `badge::resolve_style`
   reaches a solid `status.danger.fill` only through `BadgeKind::Count(u32)`,
   which renders a number (`count.text()` → `Some("7")`). The only
   text-bearing kind, `BadgeKind::Status { icon, label }`, is hardcoded to
   `fill: surface_raised` with the tone in the **foreground and border** —
   `resolve_style(&Status + Danger)` returns `status_danger_text` for both,
   i.e. exactly the danger-coloured text label the rule forbids. `Status`
   also mandates a leading glyph from a fixed four-set (`✓ ⚠ ✕ ●`), none of
   which denotes "PDF-backed". The two halves the rule needs live in mutually
   exclusive variants, and `§3.8`'s inventory contains no text-bearing solid
   chip to add without contradicting that section's own three-kind list.
2. **The mandated fill does not survive the lock either.** The rule's stated
   mechanism — *"so the chip fill itself carries the distinction and survives
   the lock"* — is arithmetically false on the band the rule is about.

**Contrast recomputed from raw sRGB via `token::contrast::contrast_ratio`;
none of these figures is transcribed from the spec.**

| Pair | Measured | Against |
|---|---|---|
| `text.strong` / `status.danger.fill` | **5.126966** | spec `5.13:1`, atom header `5.1270:1` — both accurate |
| `status.danger.fill` / `accent.fill` (selected band) | **1.067676** | the rule's premise; fails |
| `status.danger.fill` / `surface.base` | 3.880532 | clears the 3.0 coincident-fill floor |
| `status.danger.fill` / `surface.raised` | 3.649497 | header prints `3.6495:1` — accurate |
| `status.danger.fill` / `surface.hover` | 3.166606 | clears 3.0 |
| `text.strong` / `accent.fill` | 4.801987 | spec `4.80:1` — accurate |
| `status.danger.text` / `accent.fill` | 1.586348 | spec `1.59:1`; the INT-7b defect, pinned illegal |

The chip would be visible at rest (`3.88:1`, `3.17:1`) and collapse into the
selection band at `1.0677:1` — not a perceptible boundary. The atom
classifies a solid pill as `ChromeClass::CoincidentWithFill`, which owes
`MIN_COINCIDENT_FILL_CONTRAST` `3.00:1` against its mount; on the selection
band the spec-exact chip delivers `1.0677:1` and fails the framework's own
obligation by `1.93` points. So requirement #1 — that a selected PDF-backed
row still read as PDF-backed via the chip fill — is not achievable by the
mandated means, and resolving it needs a decision about the rule rather than
about the code.

**Rationale retired, not left to be re-believed.** The argument that the badge
is safe *"because it is a word rather than a colour"* is removed from
`strings::TREE_PDF_BADGE` and `left::pdf_badge_colour`. To be precise about
what was wrong with it: the string `[PDF]` does pass through the lock
unchanged, so the word survives — what the lock takes is its **salience**,
leaving a content difference rather than a visual-role one. `§3B.6:883`
declines a word-only cue for exactly that reason. The INT-7b protection the
old rationale described is real and is retained: routing through the lock is
what keeps `1.5863:1` unreachable.

**Pins deliberately not moved.** `MIN_LABEL_WIDTH` (96), `LEFT_PANE_MIN_W`
(150), `STRIP_HEIGHT` (36), `TAB_HEIGHT` (28), `TAB_INDICATOR_HEIGHT` (2) and
every pinned tolerance are unchanged. `MAX_ROW_INDENT_DEPTH` was narrowed to
govern only the auto-fit width request, not the drawn indent — a deliberate
inconsistency that fails safe (deep trees compress slightly more than
strictly needed; the floor still holds). `PIN_APP_TOKEN_BOUND` moves 91 → 93
for `PDF_BADGE_SLOT` and `PDF_BADGE_GAP`; `files_scanned` 158 and
`rule_b_files` 45 are unchanged because no new app files were added.

**Stale spec-path doc comments repointed: 11.** The move left 12 literal
`doc/planning/ui-framework.md` hits, all in `src/hoarfrost/`. One of them —
`spec_coverage_tests.rs:32` — is deliberate history (*"used to sit at …
commit `edbe04a` moved it here"*) and was left alone. The other 11 are
repointed to `spec.md` with no assertion changed: `mod.rs:5`,
`spec_coverage.rs:2`, `spec_coverage.rs:253`, `spec_coverage_tests.rs:8`,
`composite/mod.rs:8`, `token/color.rs:23`, `token/color.rs:90`,
`token/contrast.rs:182`, `token/contrast_tests.rs:1622`,
`token/typography_tests.rs:46`, `widget/mod.rs:6`. `composite/mod.rs:8` was
not in the original site list. Six further `ui-framework` matches in
`src/agent/`, `src/app/` and `src/md2pdf/` are the phrase "UI-framework" used
generically in framework-free assertions and must not be repointed.

**Gate state: 8038 workspace tests pass, 6 skipped.** `cargo check` 0 errors /
0 warnings; `cargo clippy -D warnings` 0; `cargo fmt --check` 0 diffs;
`cargo doc --no-deps` 0 warning lines.

---

### INT-13 — The `[PDF]` chip built to the mandate, and the mandate recorded as broken

INT-12 left the badge blocked: `spec.md:883` mandates a chip on
`status.danger.fill` behind `text.strong`, and the measurement said that fill is
nearly invisible against the very band the rule exists to protect. The spec
author ruled: **"if the spec mandates it, then follow the spec."** So this builds
it exactly as written and records the measurement rather than quietly fixing the
mandate or quietly hiding the number.

**The chip as built — every colour from the atom's own resolvers.**

| Property | Value | Source |
|---|---|---|
| fill | `status.danger.fill` | `badge::resolve_tone(BadgeTone::Danger, &pro).solid_fill` (`badge.rs:437`) |
| foreground | `text.strong` | constant in **all four** postures |
| radius | `shape::RADIUS_PILL` (999; iced clamps to 9px on an 18px chip) | `shape` |
| height | 18px | `badge::COUNT_HEIGHT` = `size::BADGE_COUNT_HEIGHT` (`size.rs:209`) |
| h-padding | 8px | `badge::CHIP_H_PADDING` = `space.sm` (`badge.rs:157`) |
| text | `[PDF]` | `strings::TREE_PDF_BADGE` |

No colour is hand-picked, so the "use the framework atom" obligation holds even
though the atom cannot produce this shape itself. `contrast::assert_fill_foreground_lock`
runs on the pair as actually built rather than on a claim about it.

**The atom still cannot produce it — re-verified against `resolve_style`'s three
arms, not assumed.** Solid `status.danger.fill` is reachable only through
`BadgeKind::Count(u32)`, whose text is a number. `BadgeKind::Status` is
hardcoded to `surface_raised` with the tone in the **foreground and border** —
which is the danger-coloured text label the rule forbids — plus a mandatory
leading glyph from a fixed four-set, none of which means "PDF-backed". §3.8
(`spec.md:559`) defines exactly three kinds, so adding a text-bearing solid kind
would contradict the spec's own inventory. Assembling from the atom's resolvers
is the only way to follow `:883` without breaking §3.8.

**The forbidden mechanism is now structurally absent, not merely avoided.**
`pdf_badge_colour` is **deleted**, not repurposed. That function routed the badge
through the selection lock, which is the thing `:883` forbids. With it gone there
is no code path that could produce a lock-varying badge colour; the chip's
foreground is a constant. Deleting the wrong function is a stronger guarantee
than leaving it in and not calling it.

**Contrast recomputed from raw sRGB via `token::contrast::contrast_ratio`; none
of these figures is transcribed from the spec.**

| Pair | Measured | Verdict |
|---|---|---|
| `text.strong` / `status.danger.fill` | **5.126966** | passes — spec `5.13`, atom header `5.1270`, both accurate |
| `status.danger.fill` / `accent.fill` (selected band) | **1.067676** | fails `MIN_COINCIDENT_FILL_CONTRAST` (`contrast.rs:271`) `3.00` by **1.932324** |
| `status.danger.fill` / `surface.base` | 3.880532 | visible at rest |
| `status.danger.fill` / `surface.hover` | 3.166606 | visible on hover |
| `status.danger.text` / `accent.fill` | 1.586348 | the forbidden pairing, pinned illegal |

The chip's own FG/BG pair is genuinely good — 5.13:1, comfortably AA, and it is
the pair the spec asks for. What fails is the chip's *boundary* against the band
it sits on once the row is selected. Visible at rest, visible on hover,
collapsed on selection: the mandated mechanism fails precisely in the one
posture the rule exists to address.

**How the shortfall is kept from reading as a pass.**
`the_chip_fill_collapses_on_the_selected_band_recorded_shortfall_not_a_pass`
opens with "THIS TEST RECORDS A FAILURE. IT IS NOT A PASSING RESULT." It asserts
the value is BELOW the floor **on purpose** and pins `1.067676` and the
`1.932324` gap to `1e-5`, so a palette retune that made the chip visible would
fail the test rather than silently pass it. The stated correct response, written
into the test, is to **delete it, not flip the comparison** — flipping would turn
a recorded failure into a claim that the problem was solved. The ruling that put
this here is recorded in the comment so the decision trail lives in the code and
not only in a commit message.

**The obligation that "should" catch this catches nothing — and that is a finding
about the framework's enforcement surface, not about this chip.**
`badge::badge_chrome_class` (`badge.rs:420`) does classify a solid pill
`ChromeClass::CoincidentWithFill`, and `ChromeClass::minimum_contrast()` does
return `Some(3.00)` (`contrast.rs:340`). But `gallery_audit::judge()`
(`gallery_audit.rs:152`) returns `true` **unconditionally** for every class
except `Judged`. Nothing in the enforcement path ever compares a badge fill
against a selection band. The 3.00 coincident-fill obligation is **declarative
only**. Nothing was bypassed here because there was nothing to bypass — and a test
now asserts that fact directly, so nobody later reasons "the framework would have
caught it."

**Selection lock unmoved, as `:883` requires.** No colour path for label,
chevron, depth guides or selection bar was touched. All four guard tests pass
unchanged: `the_selection_lock_still_holds_on_a_selected_and_hovered_pdf_row`,
`hover_moves_guide_geometry_never_colour_only_the_band`,
`selection_outranks_the_pointer_so_hover_cannot_wash_it_out`,
`the_depth_guides_land_on_indent_step_multiples`.

**Geometry: the chip fills the declared slot edge-to-edge, so charged width equals
painted width by construction.** `PDF_BADGE_SLOT` (`left.rs:437`) =
`PDF_BADGE_TEXT_ALLOWANCE` (20 = `space.lg + space.xs`) +
`PDF_BADGE_CHIP_PADDING_TOTAL` (8 + 8) = **36** — the same 36 as before, so
`pdf_badge_fits`, `PDF_BADGE_CHARGE` and every downstream floor computation are
unmoved. Row height stays at `HIT_TARGET_MIN` 24; the 18px chip sits in a 20px
interior with 2px spare and its 16px line box inside the chip. Worst case at
`LEFT_PANE_MIN_W = 150`: row_width 142, allowance 14, drawn 14, label 120 ≥ 96.
The fit gate is **load-bearing** — allowance minus drawn is 0, below
`PDF_BADGE_CHARGE`, so the chip is suppressed; forcing it in would leave 80px and
breach the floor. Both halves are asserted, which is what proves the suppression
is what protects the 96px floor rather than the layout happening to be roomy.
The label never shifts: `row_box` takes no PDF-backed input.

**A gate blind spot found, reported, and not exploited.** The first cut of this
work hit a wrapped `PDF_BADGE_SLOT` that the token-lint gate classified without
reading. It was restructured onto one line so it stayed gated — a workaround
around a real hole rather than a fix to it. The hole itself, the survey that sized
it, and the fix are INT-14's record. What belongs here is only the fact that the
chip work is what surfaced it, and that the workaround was reported instead of
left as the answer.

**Open deviation, deliberately left — since CLOSED by INT-15.** The row
**label** was still danger-tinted at rest and on hover via
`row_label_colour`'s error tint. Read literally, `:883`'s closing clause —
"never in a danger-colored text label" — reaches that too, and more broadly
than the badge. Changing it is a wider behavioural change to every
PDF-backed row than the badge rework that was scoped, so it was recorded
here as an open deviation rather than folded in. **INT-15 closed it**: the
tint is gone, the function that produced it (`row_text_color`) is deleted
rather than neutered, and `row_label_colour` (`left.rs:2080`) takes no
PDF-backed input at all. That function's doc had also falsely claimed the
app has no badge at all; corrected during INT-13.

**Correction to this increment's own brief.** `PDF_BADGE_SLOT` was described in
the brief as `space::XL + space::MD = 28px`. `XL` is 24 and `MD` is 12, so it
is **36**. The original doc comment was right and the brief's arithmetic was
wrong.

Tests: 8038 → 8040 (+2) —
`the_chip_fill_collapses_on_the_selected_band_recorded_shortfall_not_a_pass` and
`the_badge_charge_cannot_breach_the_label_floor_at_the_minimum_pane`, both
confirmed discovered by `cargo nextest list`.

Gates: `cargo check --workspace --all-targets` clean · `cargo nextest run
--workspace` **8040 passed, 6 skipped** · `cargo clippy --workspace
--all-targets -- -D warnings` clean · `cargo fmt --check` clean ·
`cargo doc --workspace --no-deps` after `cargo clean --doc`: **0 warning
lines**. `src/hoarfrost/spec.md` and all `SPEC.md` files untouched.

---

### INT-14 — The token-lint gate was passing code it could not read

**The hole.** `classify_scalar` (`token_lint.rs:1201`) called
`numeric_literals(&c.expr)` and returned `TokenBound` — a **pass** — when the
result was empty. The parser is line-oriented by design, so a `pub const X: f32`
whose right-hand side wrapped onto a second line yielded an empty expression, and
"I could not read this" and "this is bound to tokens" produced the identical
verdict. The parser's own doc comment named the correct principle — *"a parser
that quietly gives up on a construct is more dangerous than one that simply does
not look"* — and then did not honour it. It gave up, and the giving-up was
indistinguishable from compliance.

**Survey before fixing, run against the gate's real parser rather than a regex.**
A scratch harness called the actual `f32_consts` / `classify_scalar` /
`is_visual_name` / `in_scope` over both trees, so these are ground truth and not
a pattern-match guess:

| Tree | Files in visual scope | Gated `f32` consts | Wrapped |
|---|---|---|---|
| `iced-pro` | 81 | 233 | **2** |
| `app` | 46 | 112 | **2** |

The four wraps: `composite/form_field.rs:106` `STACKED_HEIGHT` (token-only),
`composite/settings_row.rs:80` `MIN_HEIGHT` (token-only),
`ui_iced/panels/left.rs:325` `ROW_WIDTH_SLACK` (token-only), and
`ui_iced/panels/top.rs:309` `TAG_TRIGGER_VERTICAL_PADDING`
(**literal-bearing**).

**All four are rustfmt-produced** — each right-hand side exceeds the repo's
100-column limit. That is what makes this a live trap rather than a historical
curiosity: any gated constant that grows by one token in a future edit gets
wrapped automatically and silently leaves the gate. Nobody has to choose to
bypass it. Formatting does.

**Treatment: parse the continuation, with `Unreadable` as the backstop.**
`join_continuation()` (`token_lint.rs:1123`) appends following lines until the
terminating semicolon — the same boundary the compiler uses — so the expression
is actually read. It stops and returns `None` on a blank line or on a line that
opens a new item (`pub` / `const` / `fn` / `impl` / `struct` / `enum` / `trait` /
`#`), via `opens_new_item()` (`token_lint.rs:1155`), so a malformed declaration
cannot swallow unrelated code below it. `redact_comments_and_strings` runs first,
so a semicolon inside a string on a continuation line cannot mis-bind the
expression boundary.

**The ordering inside `classify_scalar` is the whole point.** It now checks for an
empty expression **before** the empty-literals branch. An empty expression has no
literals either, so reading the two as one case is exactly how the hole worked.
"No literals" is a verdict about what was seen. "No expression" means nothing was
seen.

**New `ScalarClass::Unreadable` (`token_lint.rs:183`) is deliberately not
compliant** — `is_compliant()` returns `false` for it (`token_lint.rs:193`), it
carries its own label `unreadable-expr`, it emits a distinct Rule B finding, and
it adds a `ScanStats.unreadable` counter to the report line
(`token_lint.rs:872`: `…, N unreadable`) so a clean tree can be told apart from
an unread one. Without that counter, "nothing to report" and "nothing readable"
print the same way.

**The three token-only wraps are now token-bound because the gate actually saw the
tokens, not because it saw nothing.** That is the difference between a gate that
works and a gate that waves things through.

**It exposed a real violation on shipped code.** `TAG_TRIGGER_VERTICAL_PADDING`
carries a literal `/ 2.0`. Under the old gate that was invisible. It is now a
finding, and it is on the app allow-list **with a reason** rather than re-tuned:
the `2.0` is a centring divisor, not a measurement, and binding it to a spacing
rung would call a non-spacing value a spacing value. Collapsing the expression to
the `space.xs` it happens to yield today would discard the rung-derivation the
constant exists to carry. The non-vacuity assertion proves the allow-list entry
suppresses a real finding rather than padding the list.

**Non-vacuity proven by reproducing the old bug, not by asserting the new
behaviour.** The join and the unreadable check were temporarily disabled and the
new tests were run against them. Both failed exactly as they should — a wrapped
literal reported `token_bound = 1` with **zero** findings, i.e. reported
compliant. With the fix both pass. The simulation was reverted and verified clean;
no scratch harness or marker remains in the tree. A test that only ever sees the
fixed code proves nothing; this one was shown to catch the bug it exists for.

**Nothing is exempted.** No wrapped declaration is skipped, which would have been
the same hole with better documentation.

**Pins moved, each with an explanatory comment.** `PIN_APP_TOKEN_BOUND` **97 →
96** — `TAG_TRIGGER_VERTICAL_PADDING` left the compliant count and became a real
finding. `PIN_APP_ALLOW_ENTRIES` **12 → 13** — the new allow-list entry, and the
report assertion with it, plus a new `0 unreadable` assertion. Not moved:
`PIN_FILES_SCANNED` 152, `PIN_RULE_B_FILES` 81, `PIN_TOKEN_BOUND` 193,
`PIN_APP_FILES_SCANNED` 158, `PIN_APP_RULE_B_FILES` 45,
`PIN_APP_SURFACE_BOUND` 3 — `rule_b_files` counts files in scope rather than
const content, so it is immune to this class of change. No rendered value changed
anywhere: `TAG_TRIGGER_VERTICAL_PADDING`'s expression is byte-for-byte what it
was.

**Correction to INT-13's own report.** INT-13 said the wrapped `PDF_BADGE_SLOT`
"silently dropped out of the gate — the token-bound count read 96 instead of 97."
That is wrong. **It did not drop out.** The old code counted it, as
`token_bound`. That *is* the defect: it was counted as compliant without its
value being read. The count was never evidence of the hole; the hole was invisible
precisely **because** the constant was being counted as a pass. The 96-to-97
delta came from a different constant added in the same edit. The blind spot itself
was exactly as described; the diagnosis of the symptom was not.

Tests: 8040 → 8048 (+8) — 7 in `token_lint_tests`
(`a_wrapped_literal_is_not_reported_token_bound`,
`a_wrapped_token_only_expression_is_read_rather_than_assumed`,
`wrapping_is_transparent_to_the_value_and_never_to_the_verdict`,
`an_unreachable_expression_is_reported_unreadable_not_approved`,
`unreadable_is_never_compliant_and_never_token_bound`,
`a_single_line_literal_still_violates_exactly_as_before`,
`a_continuation_spanning_several_lines_is_joined_in_full`) and 1 in the app gate
(`a_rewrapped_pdf_badge_slot_cannot_smuggle_a_literal_past_rule_b`), all
confirmed discovered via `cargo nextest list`.

Gates: `cargo check --workspace --all-targets` clean · `cargo nextest run
--workspace` **8048 passed, 6 skipped** · `cargo clippy --workspace
--all-targets -- -D warnings` clean · `cargo fmt --check` clean ·
`cargo doc --workspace --no-deps` after `cargo clean --doc`: **0 warning
lines**. `src/hoarfrost/spec.md` and all `SPEC.md` files untouched.

---

### INT-15 — The danger tint comes off PDF labels, and a hole is accepted

**The token was literally the one the clause names.** `app.error` maps
directly to `status.danger.text` (`src/app/ui/theme.rs:202`), and the tree
painted every PDF-backed row label with `app.error` at rest and on hover.
`spec.md:883` ends with *"never in a danger-colored text label"*. This is
not a close analogue of what the rule prohibits — it is the exact token, in
a text label, carrying the exact distinction the rule exists to govern.
INT-13 recorded this as an open deviation because the badge rework did not
scope it. INT-15 closes it.

**`row_text_color` is deleted, not neutered.** The function returned
`palette.error` for PDF-backed files (the retired `REQ-450`) and the body
token otherwise. Leaving it in place behind a parameter that ignores it
would have kept the mechanism reachable: a later call site could pass
`true` again without anyone having to re-decide anything, and the doc
comment saying "this parameter is no longer used" is not a barrier anyone
hits. With the function gone and `pdf_backed` removed from
`row_label_colour`'s signature (`left.rs:2080`), there is no code path in
the crate that can put a danger token on a row label.
`no_posture_resolves_a_row_label_to_any_danger_token` asserts that against
three tokens — `app.error`, `status.danger.text`, `status.danger.fill` —
across all four selected/hovered postures, and pins as a fixture that
`app.error == status.danger.text` while `status_danger_text != status_danger_fill`,
so the sweep is not sweeping one value three times.

**The suppression hole, and its closed form: the chip shows iff
`pane >= 176 + 16 × depth`.** The chip mounts only where there is room,
and the room condition reduces exactly. Derived from the real
`pdf_badge_fits` (`left.rs:660`) and `tree_row::indent_geometry`, then
reproduced independently on both sides of the boundary at every depth
0..=12:

| Pane width | Depths that show a chip |
|---|---|
| **150** — spec `LEFT_PANE_MIN_W` | **none, at any depth** |
| **180** — app `LEFT_PANEL_MIN_WIDTH` (`panel_layout.rs:12`) | 0 only |
| 192 | 0–1 |
| 224 | 0–3 |
| **280** — app default (`panel_layout.rs:9`) | 0–6 |
| 384 — widest legal at a 1920 viewport | 0–8+ |

So at the app's own minimum pane width **every nested PDF-backed file is
visually identical to an ordinary file**, and at the spec's minimum even
the top-level ones are. This is not a degenerate width nobody visits: 180
is a width a user can drag the panel to and leave there.

**Auto-fit cannot rescue it, and the comment that said otherwise was false.**
`auto_fit_width` (`left.rs:805`) → `longest_row_width` → `row_width` →
`tree_row::content_width` (`left.rs:777`) — indent, label, furniture — and
there is **no badge term anywhere in that chain**. A pane auto-fitted to a
deep PDF-backed row therefore still suppresses that row's own chip.
`pdf_badge_fits` carried a doc comment claiming `UIF-PANE-002` *"normally
removes the conflict by widening the pane"*. That claim is false. It has
been corrected in place, and `auto_fit_does_not_widen_the_pane_to_keep_the_chip`
disproves it rather than leaving the sentence in to be re-believed. A false
claim inside the code costs more than a missing one: the next contributor
reads it as settled.

**The ruling: accept the hole.** This was brought to the spec author with
the table above and three alternatives — revert the tint, make
`auto_fit_width` charge `PDF_BADGE_CHARGE` (`left.rs:450`), or build the
spec's own icon-shape fallback per `UIF-PRIN-005`. The ruling was
**accept the hole**. It is therefore recorded rather than fixed, and pinned
by `a_pdf_backed_row_can_end_up_with_no_visible_distinction_at_all`, which
asserts the boundary on both sides at every depth 0..=12 so it can neither
widen silently nor be quietly dropped. The same test proves the suppressed
posture is *total*, not merely weak: with the chip suppressed the label
resolves to the ordinary row's colour in all four postures, so a
suppressed-chip PDF-backed row and an ordinary row share every painted
pixel.

**How literally the rule requires this — the honesty caveat, kept because
the hole makes it matter.** The implementer pointed out that `:883`'s
explicit `MUST NOT` is scoped to *"badge text color"* and the section is
titled *"Badge on **Selected** Rows"*, while *"never in a danger-colored
text label"* is the tail of the **fallback** instruction — the branch that
applies when a host cannot mount a badge chip. On a strict literal reading
the *resting* tint is not prohibited at all. The change still rests on
solid ground: the clause's wording is categorical and describes exactly
what the app was doing; the INT-13 chip makes the tint redundant under any
reading; and a colour-only cue is precisely what `UIF-PRIN-005` redundant
cueing exists to prevent. But the case is made by the clause's intent plus
the redundancy, **not by an unambiguous prohibition** — and with the
suppression hole accepted, that distinction is not academic. A reader who
assumes the rule forced this change should know it did not, and should know
that the tint's removal is what makes the suppressed-chip posture
undistinct rather than merely differently-coloured.

**The selection lock is still load-bearing, and it would be easy to conclude
otherwise.** Removing the tint invites "the label no longer needs the
lock". It does: the body token on the selection band measures **3.176079**
raw, below the `4.5` text floor. Only the role-based lock lifts it to
`text.strong` at **4.801987**. Pinned by
`the_lock_is_load_bearing_for_the_body_token_not_only_the_retired_tint`,
because "the tint is gone so the lock can go" is exactly the mistake that
would reintroduce an unreadable label — this time for every selected row,
not just PDF-backed ones.

**Contrast, all recomputed from raw sRGB via `token::contrast::contrast_ratio`
— none of it transcribed from the spec.**

| Pair | Measured | Note |
|---|---|---|
| `app.text` / `surface.base` | **13.158975** | the new resting label |
| `app.text` / `surface.hover` | **10.738035** | the new hovered label |
| `app.text` / `accent.fill` (raw) | **3.176079** | below 4.5 — the lock is required |
| `text.strong` / `accent.fill` (locked) | **4.801987** | what actually paints when selected |
| `app.error` / `accent.fill` | **1.586348** | INT-7b's defect — now unreachable **by construction** |
| `app.error` / `surface.base` | 6.572478 | what the tint used to be |
| `app.error` / `surface.hover` | 5.363298 | what the tint used to be |
| chip fill / `accent.fill` | 1.067676 | INT-13's shortfall, unchanged |
| `text.strong` / chip fill | 5.126966 | the chip's own pair, unchanged |

INT-7b's original defect is now unreachable for a stronger reason than it
was: `error` is never handed to the label resolver at all, rather than
being corrected by the lock after the fact. The lock still runs, and still
has to.

**Tests changed because the rule changed, not to make them green.** Two were
rewritten outright because they asserted behaviour the clause now
prohibits — `pdf_backed_rows_use_the_error_token_and_others_the_body_token`
became `a_pdf_backed_row_label_never_uses_the_error_token`, and
`an_unselected_row_keeps_the_apps_own_label_rule` became
`an_unselected_pdf_backed_row_is_labelled_exactly_like_an_ordinary_row`.
Neither was kept and flipped: the old assertions described the prohibited
behaviour, so both were rebuilt around the new rule. Five new tests cover
the never-danger property across four postures and three danger tokens, the
lock's continued necessity, the chip as sole carrier, the suppression
hole, and the auto-fit finding.

**Correction to this increment's own commit message.** It records
"+6 new, 1 removed, 1 renamed" and "four sites were mechanically updated
for the dropped parameter". The tree says otherwise. `left_tests.rs` went
from 84 to 89 `#[test]` items — **two rewritten, five new, net +5**, with
nothing purely removed. And in production there is **one** call site of
`row_label_colour` (`left.rs:1635`) plus the signature itself; the other
six de-parameterised sites are in tests. The substance of the increment is
unchanged; the counts in the commit message are not what the diff contains.

**No pin moved, and three tempting moves were refused.** `iced-pro`
152 / 81 / 193 and app 158 / 45 / 96 / 13 all stand — no `f32` constant
was added or removed. Refused: keeping `row_text_color` behind a dead
parameter for a smaller diff (deleted instead); lowering `PDF_BADGE_CHARGE`
or raising the pane minimum to make the suppression hole disappear (both
are pins that encode decisions, and moving one to hide a defect is exactly
what the no-lowering rule forbids); and leaving the false `UIF-PANE-002`
claim in place because correcting it was out of scope.

**The chip and the lock are untouched.**
`the_chip_is_the_sole_pdf_carrier_and_is_unchanged` pins every INT-13 chip
field — fill `status.danger.fill`, foreground `text.strong`,
`badge::COUNT_HEIGHT` 18, `shape::RADIUS_PILL`, `badge::CHIP_H_PADDING` 8
— and the slot identity. All four INT-11/INT-13 selection-lock guard tests
pass unchanged, and `row_box` still takes no PDF-backed input, so the label
still cannot shift.

Tests: 8048 → 8053 (+5 net: 2 rewritten, 5 new), all confirmed discovered
via `cargo nextest list`.

Gates: `cargo check --workspace --all-targets` clean · `cargo nextest run
--workspace` **8053 passed, 6 skipped** · `cargo clippy --workspace
--all-targets -- -D warnings` clean · `cargo fmt --check` clean ·
`cargo doc --workspace --no-deps`: **0 warning lines**. `src/hoarfrost/spec.md`
and all `SPEC.md` files untouched.

---

### MOD-004 — Sub-region height solving lands, and the spec's own two numbers conflict

**The requirement.** `UIF-MOD-004` (`spec.md:1379`) asks that scrollable
sub-region heights be solved as a pure function of the card's solved
bounds — `subregion_max_h = modal_height − (header_band + 2 × body_padding
+ footer_band)` — with every term resolved from tokens, and that a sub-region
never borrow the card's outer height constant. It was the one requirement
the spec rewrite added that still stood at `Partial`. It is now
**`Implemented`** (`spec_coverage.rs:3000`), with the deviations recorded
alongside the upgrade so the status cannot be mistaken for a clean finish.

**The solver shape: a struct, not three floats.** `SubRegionBands`
(`surface/modal.rs:510`) carries `header`, `body_padding` and `footer`.
Three interchangeable `f32` arguments at a call site is a silent
transposition bug — `solve(card, footer, header, pad)` compiles, runs and
is wrong — and `ModalFrame` deliberately carries no header or footer
sub-rectangles because those need measured content, so the frame cannot
supply them itself. `from_frame` (`modal.rs:540`) seeds the one term the
frame genuinely owns, `content_padding`, and leaves the two content bands
empty; `with_header` / `with_footer` set one band each; `total()`
(`modal.rs:558`) is **the only place in the codebase that writes the
`2 ×` term**, so the doubling cannot be restated inconsistently somewhere
else.

**`subregion_max_height` (`modal.rs:593`) is pure**: card minus the bands,
clamped at zero, non-finite inputs collapsed through the module's existing
`finite()` helper (`modal.rs:1648`) rather than propagated downstream.
`subregion_height` (`modal.rs:604`) applies the cap **after** the
remainder, not instead of it — a card too short to reach the cap still
gets everything it has rather than being over-capped.
`the_cap_is_applied_after_the_remainder_not_instead_of_it` and
`a_card_smaller_than_its_own_furniture_clamps_to_zero_rather_than_going_negative`
hold both edges; `non_finite_inputs_cannot_poison_the_solved_height` and
`the_solver_is_pure_and_deterministic` hold the purity claim.

**The named cap, and two deliberate naming decisions.**
`ATTRIBUTIONS_LIST_MAX_HEIGHT = 200` (`modal.rs:274`), registered in
`token_lint::SURFACE_GEOMETRY_CONSTANTS`.

* It is `_HEIGHT`, **not** the spec's `_H`. `VISUAL_NAME_MARKERS`
  (`token_lint.rs:295`) matches `HEIGHT`, not a bare `H`, so a constant
  named `ATTRIBUTIONS_LIST_MAX_H` would be invisible to the very gate it is
  supposed to be visible to — the cap would have been unenforceable by
  construction. The sibling §2.2 entry already made exactly this call: the
  spec's `FOLDER_LIST_MAX_H` is `FOLDER_LIST_MAX_HEIGHT` at
  `input_dialog.rs:185`.
* It lives in `surface/modal.rs`, not `token/size.rs`. `size.rs` holds
  reusable `size.*` rungs; this is a §2.2 **surface geometry** constant,
  and this codebase mirrors that table in the owning surface module.

**Measured headless before any change — none of these figures is
transcribed.**

| Quantity | Value |
|---|---|
| catalog | **58 crates** (an old test comment said 57; wrong) |
| header band | 69 |
| body padding × 2 | 32 |
| footer band | 49 |
| band total | **150** |
| remainder in a 480px card | **330.0** — matches the retired `Fill` pin exactly |
| now rendered | **200.0** — the cap binds |
| row height | 29.3793px |
| visible rows | **11.23 → 6.81** |

**THE SPEC'S OWN TWO NUMBERS CONFLICT FOR THIS CARD, AND THE COST IS REAL.**
The spec pins the card at `480px` (`ATTRIBUTIONS_MODAL_SIZE`, `spec.md:111`
and the §4.6.2 sizing table at `:1376`) **and** caps the list at `200px`.
The furniture consumes 150px, so the card has 330px to give and the cap
allows only 200 of it to be used. That leaves **130px of dead card** at
the bottom of a fixed-size dialog with nothing laid out in it, and costs
about **4.4 visible rows** — a reader scrolls roughly four more times to
reach the same place.

The spec was shipped as written rather than quietly picking a side, and the
cost is pinned by a test named
`the_cap_leaves_130px_of_dead_card_and_costs_about_four_and_a_half_rows`
rather than buried in prose. Resolving it means changing a pinned number —
the card height or the cap — which is a **decision, not an implementation
detail**, and neither number was touched. The requirement's phrasing is
conditional (*"when a sub-region needs a stable cap"*), so a reading
exists in which `200` is illustrative rather than mandatory for this card;
the §2.2 table names it concretely, so the cap was applied. **This is
open for the spec author.**

**Overflow safety moved from structural to conditional, which is a new
risk and is named as one.** `Length::Fill` made overflow structurally
impossible. A solved `Length::Fixed` is safe only because the band terms
it was computed from are the same terms the `Column` actually consumes, so
`solved + bands` equals the card's inner height and the list lands exactly
where `Fill` put it — never further. **An understated band now over-solves
through the footer**, which `Fill` could not do. The property went from
structural to argumentative, so
`the_credits_list_never_crosses_the_footer_band_at_any_viewport` measures
list-bottom against footer-top at five viewports instead of trusting the
argument. This is the cost of replacing a mechanism with a computation, and
it is the reason the solver's band terms must be updated in lockstep with
the card's furniture.

**`Length::Fixed(0.0)` is void in iced 0.14 — found by measurement, not
by reading.** `Size::is_void()` is true if **either** axis is
`Fixed(0.0)`, and void children are silently dropped from the widget tree.
At a sub-182px viewport the solved height is 0, the scrollable became
`SelectorNotFound`, and `ATTRIBUTIONS_LIST_ID` and its scroll position
were lost — a direct breach of `src/app/AGENTS.md` §5, which requires a
stateful widget's id to survive rebuilds. Verified against `Fill`, which
keeps the widget present at 0.0. `credits_list_length`
(`attributions.rs:231`) therefore falls back to `Fill` when there is no
room; the cap cannot be breached there because both mechanisms agree the
height is zero. A solved height of zero is not "nothing to draw", it is
"nothing exists", and iced means it literally.

**Wiring.** `HEADER_BAND_HEIGHT` (`attributions.rs:170`) = 69 and
`FOOTER_BAND_HEIGHT` (`attributions.rs:184`) = 49 are composed from tokens
with the gaps written as repeated `space::SM` terms rather than `3.0 ×`,
so they are genuinely `TokenBound` under any parser reading rather than
merely passing one. `credits_list_height` (`:200`) calls `subregion_height`
with the cap; `credits_list_length` (`:231`) turns the solved height into a
`Length`; the call site is `attributions.rs:352`.

**Pins moved, all genuine.** `PIN_SURFACE_BOUND` **35 → 36** — the registry
gained one entry. `PIN_APP_TOKEN_BOUND` **96 → 98** — two new literal-free,
token-bound band constants. Unchanged: `PIN_TOKEN_BOUND` 193,
`PIN_FILES_SCANNED` 152, `PIN_RULE_B_FILES` 81 — the new cap classifies
as `SurfaceBound` rather than `TokenBound`, and the scope counts are
scope-based rather than content-based.

**A second hole in the token gate was found here, one level deeper than
INT-14, and deliberately not fixed in this increment** — the record is
immediately below. The new constants were made literal-free precisely so
they would pass under either reading while that hole stayed open.

Tests: 8053 → 8073 (+20) — 13 in `surface/modal_tests.rs`
(`subregion_max_height_is_the_formula_the_requirement_states`,
`the_body_padding_is_charged_twice_and_only_inside_total`,
`from_frame_seeds_the_body_padding_and_leaves_the_content_bands_empty`,
`the_builders_set_one_band_without_touching_the_others`,
`the_attributions_band_composition_is_every_token_that_is_on_it`,
`the_attributions_subregion_is_the_measured_remainder_not_the_card_constant`,
`the_cap_is_applied_after_the_remainder_not_instead_of_it`,
`the_solved_height_never_exceeds_either_the_remainder_or_the_cap`,
`a_card_smaller_than_its_own_furniture_clamps_to_zero_rather_than_going_negative`,
`non_finite_inputs_cannot_poison_the_solved_height`,
`the_solver_is_pure_and_deterministic`,
`attributions_list_max_height_is_the_number_the_closed_table_names`,
`attributions_list_max_height_is_registered_as_a_surface_geometry_constant`)
and 8 in `attributions_tests.rs`
(`the_credits_list_is_solved_and_capped_at_the_named_maximum`,
`the_cap_leaves_130px_of_dead_card_and_costs_about_four_and_a_half_rows`,
`the_credits_list_never_crosses_the_footer_band_at_any_viewport`,
`a_viewport_too_short_for_the_cards_own_furniture_clamps_the_list_to_zero`,
`a_short_card_gives_the_list_its_remainder_not_the_cap`,
`an_empty_catalog_still_gets_the_capped_list`,
`a_very_large_catalog_changes_the_scroll_extent_not_the_region`,
`the_about_card_is_unaffected_and_keeps_its_own_cap`), replacing the
retired `the_credits_list_fills_the_card_and_scrolls`. All confirmed
discovered via `cargo nextest list`.

Gates: `cargo check --workspace --all-targets` clean · `cargo nextest run
--workspace` **8073 passed, 6 skipped** · `cargo clippy --workspace
--all-targets -- -D warnings` clean · `cargo fmt --check` clean ·
`cargo doc --workspace --no-deps`: **0 warning lines**. `src/hoarfrost/spec.md`
and all `SPEC.md` files untouched.

---

### The token-lint continuation hole, second half — a gate that cannot tell "compliant" from "unread"

**One class of failure, two halves.** INT-14 (`a02137a`) is the first half
and its record above stands; it is not repeated here. This is the second
half, found while doing MOD-004 and fixed in `e09a94d`. Both halves are
the same sentence: **the gate reported compliance for an expression it had
not finished reading.**

**Half one, only as far as half two needs it.** `classify_scalar` returned
`TokenBound` for an empty expression, so a `pub const X: f32` whose
right-hand side wrapped onto a second line passed unread. Closed by
`join_continuation` plus `ScalarClass::Unreadable`.

**Half two: the early return was one condition too broad.**
`join_continuation` opened with

```rust
if !first.trim().is_empty() { return Some(first.to_string()); }
```

so continuation lines were read **only when the first line's right-hand
side was entirely empty**. A constant written as

```rust
pub const X: f32 = tree_row::LABEL_RUNG.line_height
    + typography::CAPTION.line_height * 2.0 + PAD * 2.0;
```

returned just the first line. No literals in that line → `TokenBound` →
pass. The `2.0` multipliers underneath were never read by anything.

This is worse than half one. Half one needed the RHS to be *absent* from
line 1, which only a formatter produces. This fires on the ordinary way a
developer breaks a long sum across lines — put some of it on line 1,
continue underneath.

**Survey first, run through the gate's real parser before anything was
changed** — a scratch harness calling the actual `parse_f32_const` /
`f32_consts` / `join_continuation` / `classify_scalar` / `is_visual_name` /
`in_scope`, so the population is measured rather than guessed:

| | iced-pro | app |
|---|---|---|
| gated visual `f32` consts | 234 | 114 |
| single-line (first line carries `;`) | 231 | 109 |
| empty first-line RHS (half one's case) | 2 | 3 |
| **partial first-line RHS (newly affected)** | **1** | **2** |
| **`TokenBound` → `Violation`** | **0** | **1** |

**Exactly one constant was exposed across both trees.** `SEARCH_ROW_HEIGHT`
(`ui_iced/panels/left.rs:481`) read before the fix as
`" tree_row::LABEL_RUNG.line_height"` — no literals, `TokenBound`,
passing. Its real right-hand side carried two `2.0` multipliers that no
check had ever seen. The other two partial-first constants are token-only
continuations and stay `TokenBound`, but are now genuinely read rather than
assumed: `composite/list_tile.rs:130` `TWO_LINE_PARTS_HEIGHT`, and
`ui_iced/attributions.rs` `HEADER_BAND_HEIGHT` — the latter clean only
because MOD-004 wrote its gaps as repeated `space::SM` terms.

*Caveat on the app column:* the survey harness could not reach
`app_scope()` from iced-pro, so it used iced-pro's registry and the
absolute violation counts are inflated. The `SEARCH_ROW_HEIGHT` flip is
registry-independent — it is in neither registry — and was cross-checked
against the real `scan_app()`, which passes at `PIN_APP_TOKEN_BOUND` 98
after the fix.

*Reconciling the two survey tables.* INT-14's table reads 233 gated consts
in iced-pro and 112 in the app, with 2 and 2 wrapped; this one reads 234
and 114, with 2 and 3 empty-first-line. The trees are different, not the
counts. MOD-004 added `HEADER_BAND_HEIGHT` and `FOOTER_BAND_HEIGHT` to
`attributions.rs` and `ATTRIBUTIONS_LIST_MAX_HEIGHT` to `modal.rs`, which
accounts for the extra gated constants and for the app's third wrapped
declaration. Both tables were produced by the same harness against the tree
each increment started from.

**Tokenised, not excused.** The doubled terms are written out —
`CAPTION + CAPTION`, `PADDING + PADDING` — value byte-identical, zero
literals. Registering it as a Surface Geometry Constant would have claimed
a §-backed fixed bound that does not exist: it is a derived sum, not a
spec-pinned measurement. Allow-listing it would have excused exactly the
thing the rule exists to catch. The repetition also makes the two caption
lines and the two padding sides countable by eye.

**The fix is terminator-based, not emptiness-based.** A first line carrying
the terminating `;` is complete and is returned verbatim; otherwise the
join continues until the `;` arrives, keeping the blank-line and
`opens_new_item` guards so a malformed declaration still cannot swallow
unrelated code below it. INT-14's empty-first-line case still joins,
because `""` carries no terminator. No multi-line declaration is exempted
— that would have been the same hole with better documentation.

**Correction to the brief's own premise: `rustfmt` does not produce this
shape.** A probe constant written as one long line was wrapped by
`cargo fmt` as `pub const X: f32 =` with the terms on the following line
— i.e. **half one's** empty-first-line shape, not the partial one. What is
true is that once a developer hand-writes the partial shape, `rustfmt`
**preserves** it, because every line sits inside `max_width`. That is how
two constants were living in it at a fmt-clean HEAD. The hole is reachable
by ordinary hand-wrapping and survives formatting; it is simply not
generated by it. The premise was wrong in a direction that makes the hole
slightly narrower than claimed and no less real.

**Non-vacuity proven by reverting the fix, not by asserting the new
behaviour.** With the shipped `!first.trim().is_empty()` restored, exactly
five tests fail — `a_literal_below_a_partial_first_line_is_seen_and_fails`,
`a_multiplier_on_a_continuation_line_is_caught_not_glossed`,
`a_partial_first_line_that_cannot_reach_a_terminator_is_unreadable`,
`the_terminator_decides_the_join_and_emptiness_does_not`,
`where_the_wrap_falls_never_changes_the_verdict_the_value_earns` — and the
other three new tests pass under **both** versions, which is the correct
result: they guard behaviour that must *not* change (INT-14's regression
case, the single-line-untouched case, the token-only-continuation case).
Fix restored → 48/48 in `token_lint`. The scratch harness was deleted and
verified gone.

**No pins moved, and that is itself the lesson.** `SEARCH_ROW_HEIGHT` was
counted `token_bound` falsely before and is counted `token_bound` truly
after, so `PIN_APP_TOKEN_BOUND` 98 holds either way. **The count was never
the evidence.** INT-14's record made the same point about `PDF_BADGE_SLOT`
and it holds again here, on a different constant, for a different reason: a
compliance counter that cannot distinguish "read and clean" from "unread
and assumed clean" reports exactly the same number for both states. The
only way to tell them apart is to make the check demonstrably capable of
failing.

Tests: 8073 → 8083 (+10) — 8 in `token_lint_tests`
(`a_literal_below_a_partial_first_line_is_seen_and_fails`,
`a_token_only_continuation_below_a_partial_first_line_binds`,
`where_the_wrap_falls_never_changes_the_verdict_the_value_earns`,
`the_empty_first_line_shape_still_joins_after_the_fix`,
`a_declaration_ending_on_its_opening_line_is_returned_untouched`,
`the_terminator_decides_the_join_and_emptiness_does_not`,
`a_partial_first_line_that_cannot_reach_a_terminator_is_unreadable`,
`a_multiplier_on_a_continuation_line_is_caught_not_glossed`) and 2 in the
app gate (`a_partial_first_line_rewrap_cannot_smuggle_a_literal_either`,
`the_search_row_height_exposure_is_closed_and_reopening_it_trips_the_gate`),
all confirmed discovered via `cargo nextest list`.

Gates: `cargo check --workspace --all-targets` clean · `cargo nextest run
--workspace` **8083 passed, 6 skipped** · `cargo clippy --workspace
--all-targets -- -D warnings` clean · `cargo fmt --check` clean ·
`cargo doc --workspace --no-deps`: **0 warning lines**. `src/hoarfrost/spec.md`
and all `SPEC.md` files untouched.

---

## 6. Proposed FastMD adoption follow-on (INT-16+)

> **Status: Proposed, with three sub-increments now implemented.** This
> sequence records the 2026-10-26 source-level app audit. The items below
> remain a plan except where marked done: **INT-21a** (stable scrollable ids),
> **INT-22a** (named focus-id literals) and **INT-22b** (named close glyph)
> are implemented and committed this session — see "Completed this session"
> below. Everything else is aspirational, not a claim that the item is already
> adopted.

### Audit baseline and boundaries

The current framework ledger classifies **92 requirements: 89 Implemented and
3 Partial** (`UIF-WIN-001`, `UIF-PANE-002`, `UIF-PANE-003`). That is a
framework-coverage result, not evidence that FastMD uses every available
composite. FastMD has adopted framework tokens and several controls and
surfaces, including buttons, menus, modal geometry, movable hosts, tooltip
support, and tree-row styling. Application data, commands, lifecycle, and
persistence remain host-owned.

The previous app migration inventory is historical: Hoarfrost now has
`ComboBox`, checkbox/radio widgets, and tooltip support. The remaining
ComboBox work is app-side compatibility review, not implementation of another
framework wrapper. Likewise, keep iced layout primitives and core widgets
(`pane_grid`, markdown `text_editor`, and `table`) unless a specific reusable
Hoarfrost component is demonstrated to preserve the app's behavior.

Integration must keep one authoritative owner for app state and surface
open/close decisions. Do not add a second writer to `OverlayState` or
`SurfaceRegistry`, and do not move FastMD domain types into Hoarfrost. Where a
composite is not a fit, record the reason and keep the existing implementation;
the goal is correct use, not migration by count.

### Ordered proposed increments

- [ ] **INT-16 — Reconcile the adoption matrix and API documentation.**
      Record per-surface current usage, Hoarfrost API, behavior parity,
      ownership boundary, and decision (adopt, defer, or keep iced). Include
      `TabStrip`, `TreeView`/`VirtualList`, `SplitDivider`, `NavBar`, the native
      `pick_list` sites, and Logs. Correct app-facing integration notes that
      still describe available APIs as missing. Sources include `src/app/ui_iced/`,
      `src/hoarfrost/composite/tab_strip.rs`,
      `src/hoarfrost/composite/tree_view.rs`,
      `src/hoarfrost/composite/split_divider.rs`, and
      `src/hoarfrost/composite/navbar.rs`.
      **Gate:** every proposed migration names its app-owned state and its
      behavior-preservation tests before implementation starts.

- [ ] **INT-17 — Review native dropdowns against Hoarfrost `ComboBox`.**
      *(Analysis 2026-10-26: **not a drop-in** — the flyout has no host
      (`combo_box.rs:595` pushes an in-flow `Column` gated by
      `model.is_open()` at `:458`; the `MenuCascade` engine in
      `combo_host.rs` emits no `Element`). Scoped rewrite at best. See
      "Adoption-fit analysis" below.)*
      Assess the tag filter, batch selectors, chat-model selector, Logs
      category filter, and agent-debug JSON-row selector individually. Adopt
      only where controlled selection, filtering, disabled/read-only behavior,
      keyboard navigation, menu dismissal, and current labels match; preserve
      caller-specific matching and do not imply closed-select type-ahead where
      the model does not provide it. Add focused integration tests for each
      migrated site and retain native `pick_list` for any incompatible case.
      **Gate:** no selection or Escape behavior changes without an explicit
      decision and regression test.

- [ ] **INT-18 — Evaluate and, if compatible, adopt `TabStrip`.**
      *(Analysis 2026-10-26: **non-fit** — the composite hardcodes
      `is_hovered=false` (`tab_strip.rs:379`) and emits no enter/exit, so the
      app's hover-driven tab visuals are unreachable without framework
      changes. See "Adoption-fit analysis" below.)*
      Compare `src/hoarfrost/composite/tab_strip.rs` with the app strip in
      `src/app/ui_iced/panels/center.rs`. Preserve path-based tab identity,
      close/context-menu commands, hover and middle-click behavior, the
      unsaved indicator, and current layout constraints. Separately resolve
      the app-side dirty-tab tracking/close-guard gap identified by the audit;
      an indicator alone is not a dirty-close workflow. **Gate:** cover open,
      switch, close, external tab removal, dirty state, and pointer/keyboard
      interaction before removing existing strip logic.

- [ ] **INT-19 — Evaluate `TreeView`/`VirtualList` for the file tree.**
      *(Analysis 2026-10-26: **scoped rewrite, low ROI** — the tree subsystem
      ships no renderable iced widget (`virtual_list.rs:13` "resolver and not
      a widget"); expansion ownership is adversarial (framework ephemeral vs
      app-persisted). Row-level `tree_row` adoption is already done. See
      "Adoption-fit analysis" below.)*
      Compare the framework composite in
      `src/hoarfrost/composite/tree_view.rs` with
      `src/app/ui_iced/panels/left.rs`. Preserve app-owned filesystem
      filtering and commands, path-keyed selection/hover, context menus,
      search, PDF badge semantics and the accepted narrow-pane suppression
      behavior. Measure construction/scroll performance before and after any
      virtualization. **Gate:** prove stable row identity across filter,
      expand/collapse, and watcher refresh; preserve scroll state and all
      existing tree interaction tests. Do not claim virtualization is present
      in the current app tree until the implementation actually uses it.

- [ ] **INT-20 — Review `SplitDivider` and `NavBar` as separate adoptions.**
      *(Analysis 2026-10-26: **both non-fits** — `SplitDivider` is a single
      pairwise handle vs the app's `pane_grid` tree (a whole-layout-engine
      rewrite); `NavBar` has no region/slot model (one left-aligned `Row`,
      `navbar.rs:322`). See "Adoption-fit analysis" below.)*
      For `SplitDivider`, compare pane-grid resizing, collapse thresholds,
      keyboard/pointer behavior, and persisted pane sizes with
      `src/app/ui_iced/layout.rs`. For `NavBar`, compare the 36px app band,
      existing menu ownership, trigger behavior, and tooltip placement with
      `src/app/ui_iced/panels/top.rs`. Adopt each independently so a mismatch
      in one does not block the other. **Gate:** no changes to pane geometry,
      menu ownership, or shortcut behavior without explicit regression pins.

- [~] **INT-21 — Finish Logs shell review and stabilize stateful scrollables.**
      *(scrollable-stabilization clause done as INT-21a, commit `7905bbc`; the
      Logs shell-review clause remains open — see "Completed this session".)*
      Logs already participates in framework movable geometry/registration;
      review the remaining app-owned interior and shell behavior rather than
      treating it as wholly unmigrated. Preserve its modeless, non-Escape-close
      policy and background-log state ownership. Audit stable IDs and scroll
      restoration for the TOC, file tree, document viewer, and agent transcript
      in `src/app/ui_iced/`; add IDs only where the widget's state must survive
      rebuilds, with tests for identity and retained position. **Gate:** Logs
      remains non-Escape-closable and each stateful scrollable retains its
      intended state across rebuilds.

- [ ] **INT-22 — Resolve integration discrepancies without weakening guards.**
      *(the naming-consistency siblings INT-22a/22b are done — see "Completed
      this session" — but the basename-validation and tooltip-path work below
      remains open.)*
      Keep FastMD's stricter basename validation: Hoarfrost's
      `check_basename` does not reject `.` and `..` as whole names, despite the
      `UIF-INP-003` wording. Track the framework discrepancy for a later
      framework/coverage correction; do not remove the app guard or edit
      `src/hoarfrost/spec.md` as part of this plan. Document the distinct
      framework tooltip and app-managed tooltip paths so call sites and
      ownership are unambiguous. **Gate:** tests continue to reject traversal
      basenames, and docs describe the APIs that source actually exposes.

### Completed this session (2026-10-26)

Three small, behaviour-preserving increments landed under the supervisor
instruction on `feature/iced-pro`, plus one unrelated chore. All gates green
at each step (`cargo fmt --check` · `cargo nextest run --workspace` ·
`cargo clippy --workspace --all-targets -- -D warnings` ·
`cargo doc --workspace --no-deps`).

- [x] **INT-21a — Stable ids for the four single-instance scrollables.**
      Commit `7905bbc`. The TOC, document viewer, file tree and
      search-results scrollables had no explicit `iced::widget::Id`, so the
      reducer could not address them and their scroll position was not stable
      across tree rebuilds (`src/app/AGENTS.md` §5, `doc/distill/iced.md`
      rule 5). Each now carries a named `pub const` id and a pin test,
      following the existing `AGENT_TRANSCRIPT_SCROLLABLE_ID` pattern. The
      agent transcript already had a stable id — the audit's "missing" note
      was stale. The repeated markdown-table horizontal scrollable is
      intentionally left out: a single static id would collide across tables
      and needs a per-table scheme. **This satisfies the scrollable clause of
      INT-21; the Logs shell-review clause is not addressed here.**

- [x] **INT-22a — Named the two remaining inline focus-id literals.**
      Commit `052dbcf`. Every other static focus id in the app is a named
      `pub const &str` ([RUST-021]); the Batch mode radio group
      (`"batch.mode"`) and the agent-session close button
      (`"agent_session_close"`) still passed bare literals to the Hoarfrost
      focus router. Promoted to `BATCH_MODE_RADIO_FOCUS_ID` and
      `AGENT_SESSION_CLOSE_BUTTON_ID`, value byte-identical, each with a pin
      test. This is naming-consistency work and is **distinct from the
      ledger's INT-22** (basename validation + tooltip-path documentation),
      which stays open.

- [x] **INT-22b — Named the movable title-bar close glyph.**
      Commit `458afc3`. `movable_shell.rs` declares its grip glyphs as named
      consts (`TITLE_GRIP_GLYPH`, `RESIZE_GRIP_GLYPH`) but the Close button
      rendered a bare `"×"`. Now `CLOSE_GLYPH`, value byte-identical
      (U+00D7), same font and size — only the literal gains a name.

- [x] **chore — cleared a pre-existing clippy lint.** Commit `eef41d9`.
      Dropped a needless borrow in the batch-prompt UTF-8 test
      (`src/app/agent/batch/prompts.rs:448`) that was blocking the workspace
      `-D warnings` gate; unrelated to the adoption work, isolated as its own
      commit.

**Remaining unambiguous surface is exhausted.** The other proposed items
(INT-16..INT-20, the Logs shell-review half of INT-21, and the
basename/tooltip half of INT-22) each require a product/compatibility or
visual decision and are not safe to implement without explicit scoping.

### Adoption-fit analysis — remaining widgets (2026-10-26)

Read-only investigation (four parallel `explore` subagents; every
load-bearing claim re-verified against source by the supervisor). This
records *why* the remaining adoption items are not autonomous work, so a
future pass starts from evidence rather than re-deriving it. No files were
changed by the investigation.

**Summary**

| Widget | Framework status | Verdict | Biggest blocker (verified) |
|---|---|---|---|
| ComboBox | `UIF-ATOM-004` Implemented | Scoped rewrite (near non-fit for navbar) | Flyout has no host: `combo_box.rs:595` pushes an *in-flow* `Column` gated by `model.is_open()` (`:458`); the real `MenuCascade` engine in `combo_host.rs` emits no `Element` |
| TabStrip | `UIF-TAB-001/2/3` Implemented | Non-fit | Cannot render the app's hover contract — `is_hovered` hardcoded `false` at `tab_strip.rs:379`, no enter/exit emission |
| TreeView / VirtualList | `UIF-PANE-003` **Partial**, `UIF-COMP-006` Implemented | Scoped rewrite, low ROI | Ships no renderable iced widget — `virtual_list.rs:13` "resolver and not a widget"; `TreeView` is a state model |
| NavBar | `UIF-NAV-001/2/3` Implemented | Non-fit | No region/slot model — one left-aligned `Row` (`navbar.rs:322`), only item type is a text `NavBarMenuTrigger` (no icon/badge/selected/slot) |

**ComboBox (INT-17).** `pick_list` is self-contained (iced owns open +
overlay). The framework splits the control into three disconnected parts: a
style resolver (`combo_box.rs`), a non-rendering `MenuCascade` state machine
(`combo_host.rs`), and a widget that paints an in-flow flyout nobody wants
(`combo_box.rs:499/:595`). A real popup needs `ComboHost` + `MenuCascade` +
`FocusState` + the overlay host + per-frame `sync` — and then the `ComboBox`
widget is *not used at all* (essentially what `top.rs` already does via
`resolve_trigger`/`chevron_color`, `top.rs:890-901`). Additional cost: five
option types need `SelectOption<Id>` adapters; `menu_id`/`id_by_key` collide
on `Display` (a tag literally named "All" is already guarded at
`top.rs:442-444`); a 120px min-width floor (`combo_box.rs:276-279`) breaks
three `Length::Shrink` sites; `Escape`-revert semantics do not exist in the
app's message channels. Simplest pilot: `batch_dialog.rs:876` (concurrency),
but only after a `ComboHost`→overlay bridge exists, else it eats modal body
height.

**TabStrip (INT-18).** The app's §4.4.2 hover matrix (fill / label /
perimeter / top-bar third state) is reducer-driven (`center.rs:1414-1419`,
`update.rs:1116-1127`). The composite hardcodes `is_hovered=false`
(`tab_strip.rs:379`) and emits no enter/exit, so that contract is
unreachable — no host-side adapter recovers it. Secondary mismatches: fixed
32px band + text `"×"`/`"•"` glyphs vs the app's token-pinned 36px band +
Phosphor `ICON_X` + 6px shape dot; `Point::ORIGIN` context-menu anchor
(`tab_strip.rs:471`) vs the app's cursor-anchored menus; a static 10-row menu
vs the app's dynamic note-skill rows (`center.rs:1083-1087`). The
framework's headline value (dirty guard, MRU, close-confirm) targets state the
app does not track (`center.rs:95-100`). Adoption requires *framework*
changes first.

**TreeView / VirtualList (INT-19).** The framework tree subsystem is pure
model + arithmetic; the only code that turns its rectangles into pixels is
the gallery canvas painter. "Adopt the tree" still leaves the app writing the
entire windowed render host — the code adoption was meant to replace
(`left.rs:38-47` says this in the app's own words). Expansion ownership is
adversarial: the framework mandates *ephemeral, never-read-back* expansion
(`tree_view.rs:14-19`), but the app persists and re-points `expanded_dirs`
across restarts and renames (`persisted.rs:57`,
`command_executor.rs:332-333`). Selection semantics diverge (framework
ranges/anchor vs the app's dirs-never-selected / Shift-toggle-only,
`command_executor.rs:115-155`). The keyboard value is stranded — no tree
focus lane exists (`subscription.rs:417-426`). Note: the **row-level**
adoption (`tree_row` geometry/paint) is already done and was the part worth
having.

**NavBar (INT-20).** The composite renders brand + triggers into one
left-aligned `Row` (`navbar.rs:322`); its only item type is a text
`NavBarMenuTrigger` with no icon/badge/selected/slot, and `brand` is
`Option<String>`. The app's bar needs a right-aligned flex region
(`top.rs:982`), a status readout, a `pick_list`, and a logo `Element` — none
representable. It also deviates from the spec the app already satisfies:
padding `[0,8]` vs `[4,12]`, gap 4 vs 8, brand 14px vs 16px Bold,
four-sided border vs bottom-only. Its latch (`active_menu_id`) would be a
second writer against the app's documented single authority
`OverlayState::hamburger_open` (`app.rs:521-523`). Every piece worth owning
is already owned by a finer atom the app consumes directly
(`Button::icon_only`, `MenuCascade`, `resolve_trigger`).

**Cross-cutting framework-side drift surfaced (flagged, not touched).** These
are in `spec_coverage.rs` / `gallery.rs`, **not** `SPEC.md`:

1. `gallery.rs:955-963` still asserts "No navbar module exists in hoarfrost"
   — contradicted by `composite/navbar.rs` being cited as `UIF-NAV-001`
   evidence (`spec_coverage.rs:2112`).
2. `UIF-COMP-006` note calls the app's "15px tree rows" a migration gap —
   **stale**: the app already moved to 24px (`left.rs:370`); only the
   vestigial `flatten.rs:56` constant keeps 15 alive.
3. `UIF-VER-002` note says `TAB_MIN_WIDTH=64 / MAX=200` — actual constants
   are 96 / 220 (`tab_strip.rs:24,27`).
4. `navbar.rs` `TRIGGER_HEIGHT_STANDARD=32` is declared/re-exported but never
   applied in the render path (draws 28 via padding) — latent drift.

**Spec-revision needs (flagged only — RUST-043, `SPEC.md` untouched).**

- ComboBox: `UIF-ATOM-004` bakes in case-insensitive *substring* filtering,
  no type-ahead on the closed select, no `ActionPending`/busy state, and a
  `read_only` mode "imported, not specified." App adoption would require
  reconciling the app's dropdown spec to these — a product decision.
- TreeView: `UIF-PANE-003` is Partial *by design* (D-2/D-9 leave
  content/ordering/filter to the host); no spec change makes the widget a
  drop-in.
- No drift found in the committed work (INT-21a/22a/22b remain
  behavior-preserving).

**Clean-pilot options (each carries a caveat — none are "no-ambiguity").**

1. `VirtualMetrics` for the search-results list — reuse the windowing
   *arithmetic* (`virtual_list.rs:268`) with zero ownership conflict; the app
   still writes the windowed `Column` + `on_scroll`, and results are small,
   so ROI is low.
2. `render_tree_with_footer` — one call (`tree_view.rs:1142`) that also
   closes the UI-050 drift (search field under header vs spec "bottom"); a
   visible layout change needing sign-off.
3. ComboBox concurrency pilot (`batch_dialog.rs:876`) — cleanest combo site,
   but needs the `ComboHost`→overlay bridge (framework work) first.

**Conclusion.** None of the four is a drop-in. NavBar and TabStrip are
non-fits (keep the app implementations). ComboBox and TreeView/VirtualList
are scoped rewrites whose only clean slices each carry a layout/behavior/
framework-change caveat. The unambiguous, no-framework-change surface remains
what was shipped as INT-21a/22a/22b.

### Completion evidence for the follow-on

Each adopted component needs narrow app integration tests for its relevant
pointer and keyboard paths, plus the applicable Hoarfrost tests. Re-run the
app token-lint gate and the repository quality gates from the root before
marking an increment complete. Update
[`ARCHITECTURE_C4.md`](../technical-context/ARCHITECTURE_C4.md) only if an
actual module boundary or integration contract changes; keep this section as
the plan until that work is implemented. No `SPEC.md` is to be changed under
this plan.
