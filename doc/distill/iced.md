# iced — Distilled Developer Guide (FastMD)

> **Purpose:** the working reference for writing and reviewing UI code in this
> repository. Derived from the full SDK doc (`doc/iced.md`), the iced 0.14
> crate sources, and the conventions actually enforced in
> `src/app/ui_iced/`. Read this before touching any UI code.
>
> **Locked versions** (`Cargo.lock`): `iced 0.14.0`, `iced_widget 0.14.2`,
> `winit 0.30.13`, `wgpu 27.0.1`, `tiny-skia 0.11.4`, `cosmic-text 0.15.0`,
> `iced_test 0.14.0`. Do not read `master`/`0.15-dev` material and expect it to
> compile against this tree.

---

## 1. The model

### 1.1 The Elm architecture

Four application-specific parts; everything else is the runtime's job.

| Elm term | iced term | What it is |
|---|---|---|
| Model | State | Application-owned data |
| Messages | `Message` | Pure data describing something that happened |
| Update logic | `update` | How messages change state |
| View logic | `view` | How state dictates widgets |

Widgets produce interactions; interactions become messages; messages change
state; state rebuilds the widgets. The loop is:

```text
boot  -> (State, Task<Message>)      once, then re-callable
view  -> Element<Message>           from current state
  <... user input / subscription event ...>
update-> Task<Message>              state mutated here, nowhere else
subscription -> Subscription<Message>  which streams are live right now
```

`view` and `subscription` run after every batch of messages and `update`
calls. **Do not rely on that cadence** — treat both as stateless,
declarative functions. Since 0.14 rendering is reactive: the screen repaints
only when a widget requests it.

### 1.2 The runtime stack

| Layer | Crate | Notes |
|---|---|---|
| Windowing / OS events | `winit` | window, DPI, close request, input |
| GPU renderer | `wgpu` | the `wgpu` cargo feature |
| Software renderer | `tiny-skia` | CPU fallback when no GPU backend works |
| Text shaping | `cosmic-text` | fonts, glyph shaping, hit-testing |
| Elm runtime + widgets | `iced` / `iced_widget` | loop, widget kit, theme plumbing |

FastMD enables: `wgpu`, `tiny-skia`, `fira-sans`, `advanced-shaping`,
`markdown`. iced's own futures executor in this build is the default
**`futures` thread-pool, not tokio**. `tokio::sync::*` primitives are
reactor-free and poll fine on it; `tokio::task::*` would panic. That is why
the `std::sync::mpsc` background channel is bridged by a dedicated
forwarding thread.

### 1.3 Design rules that shape the API

- **Plain Rust, no DSL.** `row!` / `column!` / `text!` behave like `vec!` /
  `format!`. No templates, no heavy proc-macros.
- **Widget construction is pure.** Building a widget has no side effects and
  needs no global context; you may move widget code freely.
- **Type-level message contract.** Every widget carries the `Message` type it
  can emit. Mismatched children will not compile.

---

## 2. FastMD's shape

### 2.1 The launch chain

`src/app/main.rs` → `ui_iced::app::launch(LaunchInputs)`:

```rust
iced::application(boot, update, view)
    .title(strings::WINDOW_TITLE)          // "FastMD AI" (DEC-010)
    .window(window_settings)               // persisted size/pos, icon,
                                         // exit_on_close_request: false
    .theme(|_| theme::fastmd_dark_theme())
    .scale_factor(|s| s.composed_scale_factor())   // DEC-008
    .subscription(|s| subscription::subscription(&s.receivers))
    .run()
```

`boot` returns the batched startup task chain: font registration, the
scale-baseline capture (`window::latest()` → `window::scale_factor(id)`),
and the DEC-009 `ConfigArrived` publish. The publish runs **as a task**, so
every subscriber exists before the event is sent.

### 2.2 State: `AppState`

A **thin wrapper** — the framework-free `FastMdApp` domain object plus the
iced-specific runtime bits:

| Field | Why it exists |
|---|---|
| `app: FastMdApp` | all domain state; every domain accessor routes through it |
| `receivers: BusReceivers` | live bus handles for the inbound subscription bridge |
| `storage: Arc<dyn UiStateStorage>` | injected persistence handler ([RUST-024]) |
| `os_scale: Option<f32>` | DEC-008 OS scale baseline, `None` → `1.0` |
| `viewport: Size` | last known logical window size; the pane-ratio input |
| `panes: pane_grid::State<PaneKind>` | the middle-band pane grid |
| `band: MiddleBandPanes` | pane/split ids discovered at construction |
| `models: ViewModels` | every panel/dialog view-model snapshot |
| `overlay: OverlayState` | reducer-held floating-window state |

**Hard rule:** `AppState::rebuild_view_models()` MUST run at the end of
every `update` entry point, or the view renders a stale frame. The view-models
cannot be built inside `view` — widgets such as `pick_list` borrow their
option vectors out of the snapshot and the snapshot would not outlive the
call (E0515).

### 2.3 The three-band layout

```text
┌───────────────────────────────────────────────────────────┐
│ top band: [logo] FastMD Viewer | status | tag filter | ☰   │
├───────────┬───────────────────────────────┬───────────────┤
│  left     │        center                 │  right (ToC)  │
│  tree +   │   tabs / document / agent     │   gated by    │
│  search   │                               │  has_toc &&   │
│           │                               │  selected     │
├───────────┴───────────────────────────────┴───────────────┤
│ bottom band: prompt prefix ▸ [ input ] [Stop]              │
└───────────────────────────────────────────────────────────┘
```

The middle band is an `iced::widget::pane_grid` with the tree shape
`left | (center | right)` and `Axis::Vertical` split walls. Pane ids are
**discovered from the built `pane_grid::State`**, never hardcoded — the
wrapper ints on `Pane`/`Split` are `pub(super)` and cannot be constructed
from outside. Pane-grid resize events reduce 1:1 onto `PanelLayout` widths,
clamped by the UI-016 min/max constants. The grid runs with zero intra-pane
spacing, so `ratio = width / viewport_width` with no spacing term.

Above the bands sits the **overlay host**: a `Stack` that mounts the context
menu, editor, dialogs, log viewer, agent debug window, tools, About,
attributions and batch dialog in a fixed z-order (`OverlayLayer`), each over
a scrim that swallows input below the top card.

### 2.4 Message channels

`Message` has **one variant per logical interaction channel**, each a small
sub-enum:

`Window`, `Layout`, `Command`, `Tree`, `Tab`, `Editor`, `Agent`, `Search`,
`Logs`, `Theme`, `Render`, `Dialog`, `About`, `Batch`, `Tools`,
`AgentDebug`, `Inbound`, `Boot`.

Two routing rules keep the channels from overlapping ([FMD-068]):

1. **User intent rides `Message::Command(UserCommand)`.** Anything the user
   asked the *application* to do is already a self-contained `UserCommand`;
   the panel emits it, the reducer publishes and executes it. Panels never
   touch the bus themselves.
2. **Panel channels carry view-local state transitions only** — which row's
   context menu is open, what the search box holds, the window-layer reports.

`Message::Inbound` is the *background → UI* channel (config, file, agent,
background events, plus the bus lag report). It is a dedicated channel
because the bus payload enums do not implement `PartialEq` and the panel
channels are widget-only by rule 2.

---

## 3. Widgets used here

| Widget | Where | What to know |
|---|---|---|
| `pane_grid` | `ui_iced/layout.rs` | Middle-band split. `Configuration` tree → `State`; ids discovered, not constructed. `on_resize(leeway, f)` feeds `ResizeEvent { split, ratio }`. |
| `text_editor` | `ui_iced/editor.rs` | The inline editor (DEC-007). **Has no native undo/redo** — the app owns the history stack and replays `Action`s. `Content` *is* the text buffer. |
| `table` | `render/table.rs`, `tools_dialog.rs` | Column widths are pinned `Length::Fixed` from the FTWA solve. `spacing = 2*padding + separator`. |
| `scrollable` | logs, transcript, ToC, tree results | Give it an explicit `Id` when the reducer must scroll it (`operation::snap_to_end(id)`). Report geometry with `on_scroll` and decide stick-to-bottom in the reducer. |
| `overlay::menu` | referenced; **not used directly** | Built-in dropdowns are single-level, `ToString`-only rows with no per-row handler. FastMD's context menus are hand-built instead (below). |
| `mouse_area` | `ui_iced/overlay.rs` | Right-press + move. `on_right_press` takes a fixed `Message` (no closure), so the click point must be tracked with `on_move` and read back. Positions are **widget-relative**, not window-absolute. |
| `markdown` | `agent/transcript.rs` | iced's built-in Markdown widget (`markdown` cargo feature). `parse(&str) -> Iterator<Item>`, `view(items, settings)`. Emits link clicks as its `Uri` message type; `.map` them onto `RenderMessage::LinkActivated`. |
| `pick_list` | top bar, prompt row | Single-level dropdown for tag filter / chat model / hamburger. |
| `button` | everywhere | `.on_press` / `.on_press_maybe(Option<..>)` — the "maybe" form is how a disabled control is expressed. |
| `container`, `column!`, `row!`, `rule`, `space` | everywhere | Layout primitives. Alignment only takes effect when the sizing strategy yields bounds larger than the content — pass an explicit `Fill`/size. |
| `stack` | `view.rs`, `overlay.rs` | Layered draw, top-down event routing, early-return on capture. This is how the scrim + card composition works. |
| `text_input`, `checkbox` | search row, tools dialog | Simple input state lives in the view-model, not the widget. |

### 3.1 Context menus and modals (DEC-005: built-ins only)

`iced::widget::overlay::menu` is **not** reusable for FastMD's menus: it is
built *as* the overlay, forcing `&mut menu::State` into every view function,
renders each row as one plain `ToString` row with no per-row handler, and
never closes itself. Instead `ui_iced/overlay.rs` supplies:

- `with_context_menu(base, anchor, items, viewport, close_msg)` — a `Stack`
  layer with a click-away blocker plus the anchored card;
- `clamp_anchor(anchor, size, viewport)` — deterministic clamping (menu
  size is computable because rows are uniform-height buttons);
- `modal(content, close_msg)` / `modal_at(...)` — centred or anchored
  blocking cards over a dimmed scrim (`theme::scrim_color()`);
- `confirm_cancel_buttons(...)` — the shared button pair.

Why a `Stack` and not the `overlay` layer: a full-viewport scrim sits at its
natural origin, so a `Float`'s translation is `Vector::ZERO` and it would
silently stop floating. `Stack` has no such condition, and its top-down
routing with early-return-on-capture already gives the card priority over the
scrim.

**Not achievable with built-ins:** nested submenus (flatten the hierarchy
into the label, as the hamburger does), auto-flip above the anchor (clamp
only), per-span hover tooltips inside rich text (one widget per paragraph).

---

## 4. Theme, palette, fonts

### 4.1 Colour

`ui/theme.rs` is the **single source of truth** for every numeric colour.
Its token type is `iced::Color` ([GUD-003]).

```text
Palette (ui/theme.rs)  ──►  iced_theme_palette()  ──►  iced::theme::palette::Palette
   · surface  ─────────────────────────────────── background
   · text     ─────────────────────────────────── text
   · accent   ─────────────────────────────────── primary
   · success / warning / error ────────────────── success / warning / danger
                                        │
                                        └─► iced::Theme::custom("FastMD Dark", palette)
```

Rules:

- Never write a numeric colour literal in a panel, modal, tree, renderer or
  editor module. Read a token: `Palette::fastmd_dark().<token>`.
- `ui_iced/theme.rs` adds no literals; it only maps roles.
- `scrim_color()` derives the modal scrim from `surface` at `SCRIM_ALPHA`.
- The UI is dark-only (UI-002). There is no light/dark toggle, so the only
  runtime-mutable theme state is the DEC-008 user scale multiplier.
- Named iced constants (`Color::WHITE`, `Color::BLACK`, `Color::TRANSPARENT`)
  are acceptable for non-brand, self-describing uses.
- The one exception is self-contained brand artwork in `ui_iced/logo.rs`.

### 4.2 Fonts

`iced::font::load(bytes)` registers a font with the cosmic-text runtime.
`ui_iced/fonts.rs` loads the embedded icon font plus the platform
symbol-fallback, and the family becomes addressable via
`iced::Font::with_name(...)`. Registration happens in the boot task, not in
`view`. Per-widget selection (`Font::MONOSPACE`, family names) happens in
the render code.

### 4.3 Scale (DEC-008)

Text sizes are logical pixels. The effective scale is the OS/winit DPI
factor composed with the user multiplier; `AppState::composed_scale_factor()`
is what the `.scale_factor(...)` builder reads. The OS baseline is captured
at boot through a task, so it is `None` (→ `1.0`) for the very first frame.

---

## 5. Testing

Four tiers. Tier 1 is the default; tiers 3 and 4 need a human recording
pass.

| Tier | What it asserts | Tool | Lives in |
|---|---|---|---|
| 1 | Pure model: view-model snapshots, message mappers, geometry, reducers | plain `#[test]` in `*_tests.rs` sidecars | `src/app/ui_iced/**/*_tests.rs` (39 files) |
| 2 | Real `view` + real `update`, asserting the **messages** produced | `iced_test::Simulator` | `src/app/ui_iced/interaction_tests.rs` |
| 3 | Pixels do not drift | `simulator(..).snapshot(&theme)` → `Snapshot::matches_hash` | `tests/iced_visual_harness.rs` + `tests/iced_visual/baselines/` |
| 4 | A recorded user session still replays | `iced_test::run(program, ice_dir)` | `tests/iced_visual/ice/*.ice` |

### 5.1 Tier 1 — pure

The bulk of the suite. Because `update` and `view` are plain functions over
plain data, most behaviour is testable with no runtime at all: build a view
model, call the reducer, assert on the data (sizes, colours, solved widths,
the exact message a row carries). Prefer adding a pure function and testing
that over testing a widget tree.

Fixtures: `ui_iced::test_helpers::{test_app, test_app_with_config,
test_app_state, test_app_state_with}` — all over `InMemoryConfigStorage` +
`InMemoryUiStateStorage`, so no user path is touched ([RUST-006]).

### 5.2 Tier 2 — headless interaction

```rust
let mut state = test_app_state();
let mut ui = simulator(view(&state));
let _ = ui.click("some exact label");
for message in ui.into_messages() {
    update(&mut state, message);
}
```

Selector reality check (verified against `iced_selector` 0.14): a `&str`
selector is an **exact** match against the rendered text, not a substring.
For widgets with no selectable text, use the closure `Selector` impl to match
a `Candidate` variant directly (`Candidate::TextInput`, `Candidate::Focusable`,
or a counted match for the *n*-th identical label).

Known limitation: **`pick_list` menus are opaque to every selector** — the
closed label and the open rows are painted with `renderer.fill_text`, and the
menu overlay does not override `Overlay::operate`. Combined with
`Simulator::point_at` emitting no `CursorMoved`, a menu row click cannot be
driven at all. Workaround in use: drive the *same* row models as real
buttons so the row → `Message::Command` contract is still covered.

### 5.3 Tiers 3 and 4 — recorded artifacts

Baselines are **sha256 of the rendered RGBA frame** (small, diff-friendly).
`Snapshot::matches_hash` *creates* a missing baseline and returns `true`, so
the harness gates recording behind `FASTMD_RECORD_ICED_BASELINES=1` and
asserts the file exists before comparing. The hash is renderer-specific —
pin `$env:ICED_TEST_BACKEND = "tiny-skia"` for reproducible, GPU-free
baselines.

`iced_test::run` / `Emulator` execute the **real program**, so side effects
really happen; the harness uses `backend::null::Executor` so no subscription
runs during a recording — no network, no watcher, no real user config.

### 5.4 Test-safety rules

- No test reads or writes a real user path, `%APPDATA%`, or `~/.fastmd*`.
  Use `InMemory*Storage` or `tempfile::TempDir` ([RUST-006]).
- Do not feed an executing message (`Command`, `Dialog(Submitted)`) back
  into the reducer in a test unless the fixture isolates the effect —
  assertions stop at the emitted message.
- `.ice` suites replay real program behaviour; keep them deterministic with
  injected preset state, never ambient user config.

---

## 6. Conventions checklist

Before you write or change UI code:

1. **Panels are pure functions.** Take an immutable view-model snapshot,
   return `Element<Message>`. Never read `FastMdApp` inside the widget tree,
   never publish to a bus, never touch the filesystem.
2. **All mutation happens in `update`.** Then `rebuild_view_models()`.
3. **User intent = `Message::Command(UserCommand)`.** Do not invent a
   parallel intent vocabulary per panel.
4. **Background → UI only via `Message::Inbound`.** The subscription bridge
   is the single inbound path ([FMD-065]).
5. **Stable widget ids.** Give every stateful widget (scrollable, editor) an
   explicit `iced::widget::Id` from a `&'static str` const so the reducer
   can address it and the widget keeps its state across rebuilds.
   `Id::unique()` produces a different id every call — never use it for
   something you must address later.
6. **Strings live in `ui/strings.rs`** as documented `pub const` values or
   formatting helpers. No UI literals in panels, modals, trees or editors.
7. **Colours live in `ui/theme.rs`** as `Palette` tokens. No numeric
   literals anywhere else.
8. **Keep the pane layout owned** by `PanelLayout` + `ui_iced/layout.rs`.
   Do not add ad-hoc side panels.
9. **Cross-cutting UI state** goes on `AppState` (domain on `app`,
   snapshots in `models`, floating windows in `overlay`). Split a new
   concern into its own module when it grows ([RUST-051]).
10. **Never block the UI thread.** Long work goes on a `Task` or a background
    worker; the loop must stay responsive ([FMD-060]..[FMD-064]).

### 6.1 Conditional rendering

iced rebuilds the widget tree from state every frame, so the immediate-mode
"keep the shape allocated" concern does not apply here. What does apply:

- Branch on the view-model, not on live widget state.
- Prefer `on_press_maybe(Some/None)` over removing a control when the action
  is merely unavailable.
- Keep stateful widgets' **ids** stable across the branch — a scrollable that
  disappears and returns under a different id loses its scroll position.
- Mount floating surfaces through the overlay host's `OverlayLayer` order,
  never ad hoc.

### 6.2 Gotchas that cost time

| Gotcha | Consequence |
|---|---|
| `view` builds a view-model locally | E0515 — build it in `update` and store it |
| `Subscription::run` with captured state | Won't compile; use `run_with(data, builder)` with a hashable handle |
| `impl Stream` returned from a builder | Won't coerce to iced's `fn(&D) -> S`; box the stream |
| `tokio::task::*` inside iced | Panics — the executor is the futures thread-pool |
| `text` `.center()` with default sizing | No-op; give it bounds larger than the content |
| `Float` at its natural origin | Stops floating; use `Stack` for full-area layers |
| `pick_list` row under a test selector | Invisible; drive the row model instead |
| Reading `master` docs against 0.14 | API differs; pin the release you build |

---

## 7. Where to look

| Question | Go to |
|---|---|
| Full iced SDK reference | `doc/iced.md` |
| Migration plan, decisions (DEC-xxx), requirement matrix | `doc/planning/iced.md`, `doc/planning/iced-migration-map.md` |
| Architecture picture | `doc/technical-context/ARCHITECTURE_C4.md` |
| Event/message payloads | `doc/technical-context/Messaging.md` |
| App root, boot, launch | `src/app/ui_iced/app.rs` |
| Reducer | `src/app/ui_iced/update.rs` |
| Top-level view + overlay host | `src/app/ui_iced/view.rs` |
| Bus → subscription bridge | `src/app/ui_iced/subscription.rs` |
| Message enum | `src/app/ui_iced/message.rs` |
| Pane geometry | `src/app/ui_iced/layout.rs` |
| Overlay/menu/modal primitives | `src/app/ui_iced/overlay.rs` |
| Markdown render path | `src/app/ui_iced/render/` |
| Tier-2 interaction tests | `src/app/ui_iced/interaction_tests.rs` |
| Tier-3/4 harness | `tests/iced_visual_harness.rs`, `tests/iced_visual/README.md` |
