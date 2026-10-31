# hoarfrost demo

A standalone, runnable desktop app that puts **every** widget in the
`hoarfrost` UI framework on screen at once, plus a layout test suite that
proves the thing actually lays out correctly rather than merely compiling.

The demo has exactly one piece of real behaviour: **Close** quits. No
filesystem, no network, no persistence, no clocks. Every string, model and
fixture is hardcoded in `src/data.rs`.

```
cargo run
```

---

## Running it

All commands run from `D:\ppai2\src\hoarfrost-demo`. It is its own cargo
workspace (empty `[workspace]` table), so it does not join the parent
build.

```powershell
cd D:\ppai2\src\hoarfrost-demo

cargo run                 # open the window
cargo nextest run         # the whole suite
```

The iced feature set is pinned to match hoarfrost's own exactly — `wgpu`,
`tiny-skia`, `fira-sans`, `advanced-shaping`, `advanced` — because a
mismatched feature changes which renderer and font backend get linked, and
the layout numbers would stop meaning anything.

---

## The window

```
┌──────────────────────────────────────────────────────────────────────┐
│  hoarfrost   [File] [Edit] [View] [Help]                    [ Close ]│  navbar
├─────────────────────────────────┬─┬──────────────────────────────────┤
│                                 │ │                                  │
│   LEFT PANE — atoms             │ │   RIGHT PANE — composites        │
│   (scrollable)                  │ │   (scrollable)                   │
│                                 │ │                                  │
│   every control atom            │ │   every composite                │
│   18 live sections + 5 previews │ │   16 live sections + 6 previews  │
│                                 │ │                                  │
├─────────────────────────────────┴─┴──────────────────────────────────┤
│  last event: …                                    83 widgets         │  status bar
└──────────────────────────────────────────────────────────────────────┘
```

The divider is live: dragging it re-proportions the two panes.

---

## What is in each pane

Every section is a titled card that reports how many interactive widgets
it lays down. The status bar total is the sum of what the builders
actually built, not a number typed in by hand.

### Left pane — atoms

| # | Section | Widgets | What it shows |
|---|---|---|---|
| 1 | Buttons | 12 | Primary / Secondary / Ghost / Danger × live / busy (disabled variants rendered too) |
| 2 | Checkbox | 4 | unchecked / checked / indeterminate / disabled |
| 3 | Radio group | 1 | one controlled group |
| 4 | Toggle switch | 3 | live / busy / disabled |
| 5 | Segmented | 1 | multi-select strip |
| 6 | Split button | 1 | primary action + flyout |
| 7 | Hyperlink | 3 | internal / external / disabled |
| 8 | Text input | 5 | live / read-only / invalid / disabled / controlled |
| 9 | Password | 2 | live / disabled |
| 10 | Combo box | 3 | single / multi / disabled |
| 11 | Search box | 1 | controlled query |
| 12 | Rating | 3 | live / read-only / disabled |
| 13 | Range slider | 2 | live / disabled |
| 14 | Avatar | 4 | shape × size grid |
| 15 | Date picker | 2 | single / range |
| 16 | Time picker | 2 | live / disabled |
| 17 | Colour picker | 2 | live / disabled |
| 18 | Tooltip | 2 | hover targets |
| — | *5 resolver previews* | 0 | chip, badge, progress, slider, number box |

**53 interactive widgets.**

### Right pane — composites

| # | Section | Widgets | What it shows |
|---|---|---|---|
| 1 | Tab strip | 1 | dirty-tab close handshake |
| 2 | Navigation rail | 1 | collapsible, top + pinned destinations |
| 3 | Stepper | 2 | horizontal stepper + wizard footer |
| 4 | Pagination | 1 | page and page-size changes |
| 5 | Pips pager | 1 | dot pager |
| 6 | Code block | 1 | rust listing with copy affordance |
| 7 | Diff viewer | 1 | unified + side-by-side |
| 8 | Empty state | 3 | empty / no-match / error |
| 9 | Grid view | 1 | tile grid at a fixed available width |
| 10 | Keybind recorder | 1 | chord capture |
| 11 | Skeleton | 0 | loading placeholder (deliberately not counted) |
| 12 | Side sheet | 1 | inline docked sheet |
| 13 | Command palette | 1 | inline card, filtered query |
| 14 | Form field | 2 | stacked label / control / caption |
| 15 | Settings rows | 3 | horizontal label + control rows |
| 16 | Menu | 5 | action / checkable / radio / submenu / separator |
| — | *6 resolver previews* | 0 | card, list tile, table, notification, breadcrumb, overflow toolbar |

**25 interactive widgets.** Plus 5 in the chrome (4 navbar triggers +
Close). **83 total.**

### Resolver previews are paint, not widgets

The preview sections call the framework's own `resolve_style` /
`resolve_*` functions and paint the returned values through iced
containers. They report **0** widgets because they are not interactive —
counting them would inflate the status bar with things a user cannot
operate. What they prove is different and stronger: the value the resolver
returns is the value that ends up on screen, asserted per state in
`previews_tests.rs`.

---

## The layout gate

`src/layout_gate.rs` builds the real view at an explicit viewport with
the headless renderer and records every widget's geometry through a custom
`iced_selector::Selector` whose `select()` captures each candidate and
returns `None`, so `find_all()` walks the whole tree.

Each line of a manifest is:

```
id [kind] | x y w h | visible x y w h
```

The `kind` column is part of the key, not decoration: one widget registers
several candidates under the same `Id` (a focusable node plus the text
node inside it), and without the kind that reads as a duplicated widget.

Every section card also carries a stable id derived from its title
(`demo.section.side-sheet-…`), so all 45 sections are pinned with their
full geometry without 45 builders each inventing a unique constant.

### The nine assertions

| # | Assertion | What it rules out |
|---|---|---|
| 1 | Determinism | two captures at one viewport differ |
| 2 | Goldens | the tree drifted from the reviewed reference |
| 3 | Nothing collapsed | a widget resolved to ~zero size |
| 4 | Nothing escapes sideways | horizontal overflow / a stray scrollbar |
| 5 | Nothing clipped off-fold | a clip the scroll fold does not explain |
| 6 | Band ordering | navbar / panes / status bar out of order or overlapping |
| 7 | Registry completeness | a dead id, or a widget no test watches |
| 8 | Responsive sweep | breakage at any of 12 viewport sizes |
| 9 | No rebuild drift | a state change moving the chrome, or nothing ever moving |

Assertion 9 has a deliberate **mirror**: one test asserts a state change
leaves the chrome alone, another asserts a layout-affecting change *does*
move something. Without the mirror, every "nothing moved" assertion would
be vacuous. That mirror is what caught the decorative split divider — see
`BUGS.md`.

### Goldens

```
tests/golden/root-1400x900.txt            91 named / 1831 anonymous
tests/golden/atoms-pane-600x900.txt       58 named /  963 anonymous
tests/golden/composites-pane-600x900.txt  31 named /  848 anonymous
```

Goldens are **compare-only by default and a missing golden is a hard
failure.** This is deliberate: `iced_test`'s own
`Snapshot::matches_image` / `matches_hash` *create* the reference file and
return `true` when it is absent, which is exactly the failure mode where a
suite reports green forever against a reference nobody ever reviewed.
`check_golden` refuses that behaviour.

To rewrite a golden deliberately and review the diff:

```powershell
$env:HOARFROST_DEMO_GOLDEN='1'
cargo nextest run matches_its_golden
Remove-Item Env:\HOARFROST_DEMO_GOLDEN
git diff tests/golden
```

### Human review, not gating

```powershell
cargo nextest run --run-ignored only render_a_png_for_human_review
```

Renders the window to a PNG in the temp dir and prints its SHA-256.
Deliberately `#[ignore]`d: a raster golden pins the renderer, the font
files and the hinting settings of whichever machine drew it, so it can
only ever be *reviewed*, never gated. The text manifest is what the suite
asserts on.

Also ignored: `repro_bug01_infinite_height_in_unbounded_axis`, a canary
for the framework finding in `BUGS.md`.

---

## Quality gate

From `D:\ppai2\src\hoarfrost-demo`:

```
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo nextest run --status-level fail --show-progress none
cargo doc --no-deps --quiet
```

---

## KNOWN GAPS

Things this demo does **not** show, with the exact reason.

### Functionality

- **Nothing beyond `Close`.** By design. Every other control updates
  in-memory state and the status line and stops there. No filesystem,
  network, persistence or clock.

### Widgets not instantiated as widgets

- **`Toast`** (`composite::toast`) is shown as resolver paint only, never
  as a live widget. `ToastTimer::armed(total_ms, now: TimeMs)` and
  `ToastStack::advance(now)` require a live clock to arm and expire the
  dismissal timer, and the demo forbids clocks. A toast rendered at a
  frozen `TimeMs` would be a toast that never dismisses — a lie about the
  widget. The resolved bar paint is shown instead via
  `info_bar::resolve_style(severity, surface, palette)`, which is the
  same paint path a real toast uses for its surface.
- **`SplitDivider`'s own candidate is not addressable.** The divider is
  not focusable and exposes no candidate kind; only
  `demo.atoms.split.control`, the button inside it, can be located by
  bounds.

### Test-addressability constraints

These are framework/iced behaviours, not demo defects, but they bound what
the layout gate can see. Each is asserted **absent** by
`the_unaddressable_ids_are_still_absent`, so if a hoarfrost change makes
one addressable the test fails and the registry gets updated rather than
drifting quietly.

- A **disabled** `Button`, `ToggleSwitch`, `Hyperlink` or `RangeSlider`
  registers no candidate at all — a disabled control is not focusable, so
  the operation tree never carries its id. (A disabled **text input**
  *does* register, so "disabled means invisible" is not a general rule.)
- `RangeSlider` registers its thumbs as `<id>.lower` / `<id>.upper`, not
  under the bare id.

### Structure

- **The layout tests live in-crate (`src/layout_gate.rs`), not in
  `tests/layout.rs`.** The demo is a binary crate, and an integration test
  under `tests/` cannot reach `main.rs`'s modules. Making them reachable
  would mean splitting the crate into lib + bin and rewriting every
  `crate::` path in 236 tests for no behavioural gain, so the assertions
  live in the binary's own test build. Goldens are still under
  `tests/golden/` and resolved via `CARGO_MANIFEST_DIR`.

### Deviations from `PLAN.md` §3

- No `strings.rs` — user-facing text is a nested `pub mod strings` inside
  `app.rs`, which keeps each string next to the module that owns it.
- `PaneBody::empty()` and `placeholder_section` were dropped as unused.
- Previews are wired into both panes from S1 rather than arriving at S7,
  so the section list never had a placeholder to be replaced later.

---

## Bugs found

See **[BUGS.md](BUGS.md)** for the full triage: one framework robustness
gap with a runnable canary, one demo mis-composition the layout gate
caught, and five framework behaviours that looked like gaps and were not.
