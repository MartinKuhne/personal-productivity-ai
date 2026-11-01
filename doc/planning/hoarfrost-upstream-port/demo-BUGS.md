# Bug log — hoarfrost demo

Every failure the layout gate and the unit suite produced while building this
demo, triaged. Nothing here was waved through.

**Triage discipline**

| Class | Meaning | Action taken |
|---|---|---|
| `DEMO` | The demo composed a framework widget wrongly. The framework did what it documents. | Fixed in the demo. |
| `FRAMEWORK` | hoarfrost produced a result its own contract does not permit. | Recorded here with a minimal repro. Worked around in the demo only where the workaround does not hide the finding. |
| `TEST` | The assertion itself was wrong. | The assertion was corrected, and *why the original was wrong* is stated. |

No assertion was loosened into meaninglessness. Where an assertion was
dropped or narrowed, the reason is in the `TEST` entries below.

---

## BUG-01 — three full-height widgets propagate a non-finite height into a scrollable

**Class: `FRAMEWORK` (robustness gap, not a contract violation)**
**Found by:** `layout_gate::no_widget_has_infinite_geometry`
**Status:** worked around in the demo; framework note stands

### Symptom

With the navigation rail, the side sheet and the open command palette
composed directly into a `scrollable`, the probe reported non-finite
geometry, and the sibling below each one was placed at `y = inf`.

Measured at a 600×900 viewport, one widget above a `button("sibling")`
inside a `scrollable` (verbatim probe output):

```
### navigation_rail bare in scrollable: infinite=4
    #0 (scrollable) [scrollable] | 0.00 0.00 220.00 900.00 | 0.00 0.00 220.00 900.00
    #1 (container)  [container]  | 0.00 0.00 220.00 inf    | 0.00 0.00 220.00 900.00
    #2 (focusable)  [focusable]  | 0.00 0.00 220.00 inf    | 0.00 0.00 220.00 900.00
    #3 (container)  [container]  | 0.00 inf   120.00 0.00   | -
    #4 (text)       [text]       | 10.00 inf  100.00 0.00   | -

### side_sheet open in scrollable: infinite=3
    #1 (container)  [container]  | 0.00 0.00 320.00 inf    | 0.00 0.00 320.00 900.00
    #4 (container)  [container]  | 0.00 inf   120.00 0.00   | -

### command_palette open in scrollable: infinite=3
    #1 (container)  [container]  | 0.00 0.00 600.00 inf    | 0.00 0.00 600.00 900.00
    #2 (container)  [container]  | 0.00 inf   120.00 0.00   | -
```

Line `#1` is the widget claiming `inf` for its own height. Line `#3` is
the consequence: the next sibling in the column starts at `y = inf` and
collapses to zero height, unreachable by any scroll position.

### Evidence

`src/hoarfrost/composite/navigation_rail.rs:591-593`

```rust
let width = self.model.current_width();
let height = limits.max().height;
layout::Node::new(Size::new(width, height))
```

`src/hoarfrost/composite/side_sheet.rs:532-537`

```rust
fn size(&self) -> Size<Length> {
    if self.model.open {
        Size::new(Length::Fixed(self.model.width), Length::Fill)
    } else {
        Size::new(Length::Fixed(0.0), Length::Fixed(0.0))
    }
}
```

`src/hoarfrost/composite/command_palette.rs:741-745`

```rust
if !self.model.open {
    layout::Node::new(Size::ZERO)
} else {
    layout::Node::new(limits.max())
}
```

### Root cause

All three take the host's maximum height **verbatim**. Each is a documented
full-height or overlay-band widget — a rail spans its column, a sheet spans
its host, a palette covers its host — so reading `limits.max()` is the
intended way to size them.

The problem is what `limits.max().height` *is* inside a `scrollable`. A
scrollable gives its content an **unbounded main axis**, so the maximum is
`f32::INFINITY`, and iced resolves `Length::Fill` against an unbounded axis
to `inf` as well. The widget then hands `inf` back as its own height. That
`inf` is added to the running cursor by the parent `Column`, so every
sibling below it is placed at `inf`, and their own derived geometry becomes
`nan`.

### Why this is classified as a robustness gap rather than a hard bug

The three widgets are not violating their contracts. Given a bounded host
they lay out correctly, which is what the demo does after the fix and what
every real caller would do. Reading the host's max height for a full-height
widget is reasonable.

What is missing is a **guard**. A widget that returns a non-finite size
corrupts its entire siblings, and nothing in the layout path notices: no
`debug_assert`, no clamp, no diagnostic. The caller gets a silently broken
page instead of a loud failure at the point of the mistake. A
`debug_assert!(height.is_finite(), ...)` at the three sites above would
have turned this into a one-line panic naming the widget, instead of a
`nan` three screens down.

That is worth fixing in the framework, but it is a hardening request, not a
demonstration that these widgets are wrong.

### Minimal repro

Runnable and permanent inside this crate — a canary, not a throwaway:

```
cargo nextest run --run-ignored only repro_bug01
```

It lives at `src/layout_probe_tests.rs::repro_bug01_infinite_height_in_unbounded_axis`
and builds the tree above from `data::rail_model()`, `data::sheet_model()`
and `data::palette_model()`.

The same shape, reduced to something a hoarfrost maintainer can paste into
a scratch binary (no demo code required):

```rust
use iced::widget::{button, scrollable, Column};
use iced::{Element, Length};

use hoarfrost::composite::navigation_rail::NavigationRail;

fn main() -> iced::Result {
    // The scrollable's main axis is unbounded, so inside the rail's
    // `layout` (`navigation_rail.rs:592`) `limits.max().height` is +inf,
    // and the rail hands that back as its own height.
    let rail = NavigationRail::new(
        hoarfrost::composite::navigation_rail::NavigationRailModel::new(vec![], vec![], ""),
    );

    let root: Element<'_, ()> = scrollable(
        Column::new()
            .push(rail)
            .push(button("this sibling lands at y = inf").width(Length::Fixed(120.0))),
    )
    .height(Length::Fill)
    .into();

    iced::application("repro", || Ok(root)).run()
}
```

The same holds for `side_sheet` and `command_palette` in their open states
(`side_sheet.rs:534` declares `Length::Fill`; `command_palette.rs:744`
returns `layout::Node::new(limits.max())`).

The canary asserts the behaviour **still occurs**. If hoarfrost adds a
finite guard, the canary fails — which is the signal to retire this note
rather than let it go stale.

### Workaround applied in the demo

Each of the three is wrapped in a host `container` with an explicit fixed
height, so the widget receives a finite `limits.max()` and lays out
exactly as documented:

```rust
// src/pane_composites.rs
const RAIL_HOST_HEIGHT: f32 = 280.0;
const SHEET_HOST_HEIGHT: f32 = 260.0;
const PALETTE_HOST_HEIGHT: f32 = 420.0;
```

The workaround does **not** hide the finding: the gate test
`no_widget_has_infinite_geometry` still runs over the whole tree, so if the
demo ever drops a host wrapper the failure returns immediately. The
framework note above stands independently of the demo.

---

## BUG-02 — the split divider was decorative: dragging it moved nothing

**Class: `DEMO`**
**Found by:** `layout_gate::a_layout_affecting_change_actually_moves_the_pane`
**Status:** fixed in the demo

### Symptom

`DemoApp::update` recorded a new `split_ratio` on `SplitCommitted`, the
status bar reported `"split divider committed: 0.70"`, and the two panes
kept their original 50/50 widths. The handle dragged, the label changed,
the layout did not.

### Evidence

Before the fix, `src/app.rs::view` built both panes as `Length::Fill`
while `self.split_ratio` flowed only into the divider's own model:

```rust
let panes = iced::widget::Row::new()
    .push(scrollable(left.sections).width(iced::Length::Fill))   // 1 : 1
    .push(split_divider(divider))                              // fixed grab
    .push(scrollable(right.sections).width(iced::Length::Fill)); // 1 : 1
```

Measured: `demo.composites.scroll` sat at `x = 705.00` before *and* after
committing a ratio of `0.70` — identical to two decimal places, which is
what the failing assertion printed.

`src/hoarfrost/composite/split_divider.rs:489-494` shows why:

```rust
fn size(&self) -> Size<Length> {
    match self.model.axis {
        SplitAxis::Horizontal => Size::new(Length::Fixed(size::SPLITTER_GRAB), Length::Fill),
        SplitAxis::Vertical   => Size::new(Length::Fill, Length::Fixed(size::SPLITTER_GRAB)),
    }
}
```

`SplitDivider` is a **fixed-width grab handle**. It *reports* a ratio
through `on_resize_change` / `on_resize_commit`; it never resizes its
siblings. Applying the ratio is the caller's job.

### Root cause

Mis-composition in the demo. The framework behaved exactly as designed.

### Fix

The ratio is now carried back into the panes' widths as iced fill portions,
via `app::fill_portions`. Fill portions rather than fixed pixels because
the demo has no live viewport to measure, and portions keep the split
correct at every window size.

```rust
pub fn fill_portions(ratio: f32) -> (u16, u16) {
    let sane = if ratio.is_finite() { ratio } else { data::SPLIT_RATIO_DEFAULT };
    let clamped = sane.clamp(data::SPLIT_RATIO_MIN, data::SPLIT_RATIO_MAX);
    let scaled = (clamped * f32::from(PORTION_TOTAL)).round();
    let left = scaled.clamp(1.0, f32::from(PORTION_TOTAL - 1)) as u16;
    (left, PORTION_TOTAL - left)
}
```

Covered by 7 tests in `app_tests.rs` (even split at the default, portions
always sum to the total, out-of-band ratios clamped, `NaN` cannot leak a
zero portion, neither pane is ever starved, monotonicity, and the committed
ratio really moves the pane) plus the gate test that found it.

### Note on how it was found

This is why the gate has a *mirror* to its "nothing moved" assertion.
Asserting that a state change leaves the chrome alone proves stability.
Asserting that a layout-affecting change **does** move something proves
the assertions are not vacuous. The second one is what caught a splitter
that only looked connected.

---

## Not bugs — framework behaviours worth recording

These looked like gaps during the build and turned out to be intended. They
are recorded because they constrain how a test can address a widget, and
because the gate asserts each of them is still true so they cannot drift
quietly.

### 1. A disabled button, switch or hyperlink registers no candidate

`demo.atoms.button.*.disabled`, `demo.atoms.switch.disabled`,
`demo.atoms.link.disabled` and `demo.atoms.range.disabled` never appear in
the operation tree, so the selector cannot locate them by bounds.

This is iced's disabled handling: a disabled control is not focusable and
does not register a focusable candidate. It is the correct behaviour — the
alternative, a disabled control that still takes focus, would be worse.

Note the asymmetry: a disabled **text input** *does* register (`iced`'s
`TextInput` always registers so the operation can reach it and see that it
is disabled). So "disabled means invisible" is not a general rule, and a
test must not assume one.

The gate pins this with `the_unaddressable_ids_are_still_absent`: if a
future hoarfrost change makes one of these addressable, that test fails and
the id moves into `REGISTRY`.

### 2. `RangeSlider` registers its thumbs, not its own id

`demo.atoms.range.live` never appears. The widget registers
`demo.atoms.range.live.lower` and `demo.atoms.range.live.upper` — the bare
id gets a per-thumb suffix. Addressing a range slider means addressing one
of its thumbs.

### 3. `SplitDivider` registers no candidate of its own

`demo.atoms.split` is invisible to the selector; the divider is not
focusable. Only `demo.atoms.split.control`, the button inside it, is
addressable.

### 4. One widget can register several candidates under one `Id`

A text input registers a `focusable` node *and* the `text_input` node
inside it, at the same bounds. In a manifest keyed on id alone this reads
as a duplicated widget.

This is why the manifest line carries a `kind` column:

```
id [kind] | x y w h | visible x y w h
```

The kind is part of the key, not decoration.

### 5. A clip at a scroll fold is not a clip

The first cut of the clip assertion compared `visible` against `bounds` for
anything above the *window* fold and failed on
`demo.section.split-button | 0 800 695 96 | 0 800 695 72`.

That was **`TEST`**, not a layout bug: the clipping boundary is the
*scrollable's* viewport (bottom 872), not the window's (bottom 900). The
widget was scrolled, exactly as a scrollable is supposed to scroll.

The assertion was replaced with `assert_clipped_only_at_a_scroll_boundary`,
which is strictly stronger than what it replaced:

- `visible` may never be larger than `bounds`;
- nothing may be clipped **sideways** (no pane scrolls horizontally);
- any vertical clip must land exactly on a scrollable's bottom edge.

A clip the scroll fold does not explain is now a failure, which the original
form would have missed entirely.
