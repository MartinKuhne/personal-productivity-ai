# iced — Distilled Developer Guide (SDK Reference)

> **Purpose:** A working reference for writing and reviewing UI code using iced 0.14. Derived from the official SDK documentation, iced 0.14 crate sources, and common patterns. Read this before touching any UI code in this codebase.
>
> **Locked versions:** `iced 0.14.0`, `iced_widget 0.14.2`, `winit 0.30.13`, `wgpu 27.0.1`, `tiny-skia 0.11.4`, `cosmic-text 0.15.0`, `iced_test 0.14.0`. Do not read `master`/`0.15-dev` material and expect it to compile against this tree.

---

## 1. The Model

### 1.1 The Elm Architecture

Four application-specific parts; everything else is the runtime's job.

| Elm term | iced term | What it is |
|---|---|---|
| Model | State | Application-owned data |
| Messages | `Message` | Pure data describing something that happened |
| Update logic | `update` | How messages change state |
| View logic | `view` | How state dictates widgets |

Widgets produce interactions; interactions become messages; messages change state; state rebuilds the widgets. The loop is:

```text
boot  -> (State, Task<Message>)      once, then re-callable
view  -> Element<Message>           from current state
   <... user input / subscription event ...>
update-> Task<Message>              state mutated here, nowhere else
subscription -> Subscription<Message>  which streams are live right now
```

`view` and `subscription` run after every batch of messages and `update` calls. **Do not rely on that cadence** — treat both as stateless, declarative functions. Since 0.14 rendering is reactive: the screen repaints only when a widget requests it.

### 1.2 The Runtime Stack

| Layer | Crate | Notes |
|---|---|---|
| Windowing / OS events | `winit` | window, DPI, close request, input |
| GPU renderer | `wgpu` | the `wgpu` cargo feature |
| Software renderer | `tiny-skia` | CPU fallback when no GPU backend works |
| Text shaping | `cosmic-text` | fonts, glyph shaping, hit-testing |
| Elm runtime + widgets | `iced` / `iced_widget` | loop, widget kit, theme plumbing |

The default feature set is `["wgpu", "tiny-skia", "crisp", "web-colors", "thread-pool", "linux-theme-detection", "x11", "wayland", "wayland-dlopen", "wayland-decorations"]`.

| Feature | Role |
|---|---|
| `thread-pool` | The `futures` thread-pool executor as `executor::Default` on native platforms. |

### 1.3 Design Rules That Shape the API

- **Plain Rust, no DSL.** `row!` / `column!` / `text!` behave like `vec!` / `format!`. No templates, no heavy proc-macros.
- **Widget construction is pure.** Building a widget has no side effects and needs no global context; you may move widget code freely.
- **Type-level message contract.** Every widget carries the `Message` type it can emit. Mismatched children will not compile.

---

## 2. Application Structure (Generic Patterns)

### 2.1 The Launch Chain

```rust
iced::application(boot, update, view)
    .title("Application Title")
    .window(window_settings)
    .theme(|_| custom_theme())
    .scale_factor(|state| state.scale_factor())
    .subscription(|state| subscription_builder(&state.receivers))
    .run()
```

`boot` returns `(State, Task<Message>)` — the initial state plus any startup tasks (font registration, config loads, etc.). The `boot` closure is re-callable (Elm requirement).

### 2.2 State Organization

Keep state as a thin wrapper around domain data plus runtime bits:

| Field Category | Purpose |
|---|---|
| Domain state | All business logic state, accessed through accessors |
| Bus/receiver handles | Live handles for inbound subscription bridge |
| Persistence | Injected storage handler (trait object) |
| UI scale | OS baseline composed with user multiplier |
| Viewport | Last known logical window size |
| Pane grid | `pane_grid::State` for split layouts |
| View-models | Pre-computed snapshots for each panel/dialog |
| Overlay state | Floating window state (modals, menus, dialogs) |

**Rule:** Rebuild view-models at the end of every `update` entry point, or the view renders a stale frame. View-models cannot be built inside `view` — widgets such as `pick_list` borrow their option vectors out of the snapshot and the snapshot would not outlive the call.

### 2.3 Pane Grid Layout

Use `pane_grid` for split layouts:

- Build a `Configuration` tree → `State`
- Pane/split IDs are **discovered from the built `pane_grid::State`**, never hardcoded
- `on_resize(leeway, f)` feeds `ResizeEvent { split, ratio }` to the reducer
- Grid runs with zero intra-pane spacing; `ratio = width / viewport_width`

### 2.4 Overlay Host

Mount floating surfaces through a `Stack` with a fixed z-order:

- A full-viewport scrim (dimmed background) at base layer
- Context menus, dialogs, editors, log viewers mounted above
- Top-down event routing with early-return-on-capture gives the card priority over the scrim
- `modal(content, close_msg)` / `modal_at(...)` for centred or anchored blocking cards

**Why `Stack` not `overlay` layer:** A `Float` at its natural origin stops floating; `Stack` has no such condition.

### 2.5 Message Channel Design

`Message` should have **one variant per logical interaction channel**, each a small sub-enum.

Two routing rules keep channels from overlapping:

1. **User intent rides a single `Command` variant.** Anything the user asked the application to do is a self-contained command; the panel emits it, the reducer publishes and executes it. Panels never touch the event bus themselves.
2. **Panel channels carry view-local state transitions only** — which row's context menu is open, what the search box holds, window-layer reports.

Inbound background events (config, file, agent, background, bus lag reports) should use a dedicated variant (e.g., `Inbound`) because bus payload enums typically don't implement `PartialEq` and panel channels are widget-only by rule 2.

---

## 3. Widgets Reference

| Widget | What to Know |
|---|---|
| `pane_grid` | Middle-band split. `Configuration` tree → `State`; ids discovered, not constructed. `on_resize(leeway, f)` feeds `ResizeEvent { split, ratio }`. |
| `text_editor` | **Has no native undo/redo** — the app owns the history stack and replays `Action`s. `Content` *is* the text buffer. |
| `table` | Column widths are pinned `Length::Fixed` from a solve. `spacing = 2*padding + separator`. |
| `scrollable` | Give it an explicit `Id` when the reducer must scroll it (`operation::snap_to_end(id)`). Report geometry with `on_scroll` and decide stick-to-bottom in the reducer. |
| `overlay::menu` | Built-in dropdowns are single-level, `ToString`-only rows with no per-row handler. For nested menus, auto-flip, or per-span tooltips, build custom overlays. |
| `mouse_area` | `on_right_press` takes a fixed `Message` (no closure), so the click point must be tracked with `on_move` and read back. Positions are **widget-relative**, not window-absolute. |
| `markdown` | iced's built-in Markdown widget (`markdown` cargo feature). `parse(&str) -> Iterator<Item>`, `view(items, settings)`. Emits link clicks as its `Uri` message type; `.map` them onto your message enum. |
| `pick_list` | Single-level dropdown. Opaque to test selectors — the closed label and open rows are painted with `renderer.fill_text`, and the menu overlay does not override `Overlay::operate`. |
| `button` | `.on_press` / `.on_press_maybe(Option<..>)` — the "maybe" form is how a disabled control is expressed. |
| `container`, `column!`, `row!`, `rule`, `space` | Layout primitives. Alignment only takes effect when the sizing strategy yields bounds larger than the content — pass an explicit `Fill`/size. |
| `stack` | Layered draw, top-down event routing, early-return on capture. Use for scrim + card composition. |
| `text_input`, `checkbox` | Simple input state lives in the view-model, not the widget. |

---

## 4. Theme, Palette, Fonts

### 4.1 Colour

A single `Palette` module is the **single source of truth** for every numeric colour. Token type is `iced::Color`.

```text
Palette  ──►  iced_theme_palette()  ──►  iced::theme::palette::Palette
   · surface  ─────────────────────────────────── background
   · text     ─────────────────────────────────── text
   · accent   ─────────────────────────────────── primary
   · success / warning / error ────────────────── success / warning / danger
```

Rules:
- Never write a numeric colour literal in a panel, modal, tree, renderer, or editor module. Read a token: `Palette::<theme>().<token>`.
- The theme module adds no literals; it only maps roles.
- Named iced constants (`Color::WHITE`, `Color::BLACK`, `Color::TRANSPARENT`) are acceptable for non-brand, self-describing uses.
- Self-contained brand artwork (e.g., logo) may use local tints not part of the UI palette.

### 4.2 Fonts

`iced::font::load(bytes)` registers a font with the cosmic-text runtime. Load embedded fonts plus platform symbol-fallback in the boot task, not in `view`. The family becomes addressable via `iced::Font::with_name(...)`. Per-widget selection (`Font::MONOSPACE`, family names) happens in the render code.

### 4.3 Scale

Text sizes are logical pixels. The effective scale is the OS/winit DPI factor composed with the user multiplier. The OS baseline is captured at boot through a task, so it is `None` (→ `1.0`) for the very first frame. The `.scale_factor(...)` builder reads the composed factor from state.

---

## 5. Testing

Four tiers. Tier 1 is the default; tiers 3 and 4 need a human recording pass.

| Tier | What it asserts | Tool |
|---|---|---|
| 1 | Pure model: view-model snapshots, message mappers, geometry, reducers | plain `#[test]` in `*_tests.rs` sidecars |
| 2 | Real `view` + real `update`, asserting the **messages** produced | `iced_test::Simulator` |
| 3 | Pixels do not drift | `simulator(..).snapshot(&theme)` → `Snapshot::matches_hash` |
| 4 | A recorded user session still replays | `iced_test::run(program, ice_dir)` |

### 5.1 Tier 1 — Pure

The bulk of the suite. Because `update` and `view` are plain functions over plain data, most behaviour is testable with no runtime at all: build a view model, call the reducer, assert on the data (sizes, colours, solved widths, the exact message a row carries). Prefer adding a pure function and testing that over testing a widget tree.

Fixtures over `InMemoryConfigStorage` + `InMemoryUiStateStorage`, so no user path is touched.

### 5.2 Tier 2 — Headless Interaction

```rust
let mut state = test_app_state();
let mut ui = simulator(view(&state));
let _ = ui.click("some exact label");
for message in ui.into_messages() {
    update(&mut state, message);
}
```

Selector reality check (verified against `iced_selector` 0.14): a `&str` selector is an **exact** match against the rendered text, not a substring. For widgets with no selectable text, use the closure `Selector` impl to match a `Candidate` variant directly (`Candidate::TextInput`, `Candidate::Focusable`, or a counted match for the *n*-th identical label).

Known limitation: **`pick_list` menus are opaque to every selector**. Workaround: drive the same row models as real buttons so the row → `Message::Command` contract is still covered.

### 5.3 Tiers 3 and 4 — Recorded Artifacts

Baselines are **sha256 of the rendered RGBA frame** (small, diff-friendly). `Snapshot::matches_hash` *creates* a missing baseline and returns `true`, so the harness gates recording behind an env var and asserts the file exists before comparing. The hash is renderer-specific — pin the test backend (e.g., `tiny-skia`) for reproducible, GPU-free baselines.

`iced_test::run` / `Emulator` execute the **real program**, so side effects really happen; the harness uses a null executor so no subscription runs during a recording — no network, no watcher, no real user config.

### 5.4 Test-Safety Rules

- No test reads or writes a real user path, platform-default config dirs. Use in-memory storage or `tempfile::TempDir`.
- Do not feed an executing message (`Command`, `Dialog(Submitted)`) back into the reducer in a test unless the fixture isolates the effect — assertions stop at the emitted message.
- Recording suites replay real program behaviour; keep them deterministic with injected preset state, never ambient user config.

---

## 6. Conventions Checklist

Before writing or changing UI code:

1. **Panels are pure functions.** Take an immutable view-model snapshot, return `Element<Message>`. Never read domain state inside the widget tree, never publish to a bus, never touch the filesystem.
2. **All mutation happens in `update`.** Then rebuild view-models.
3. **User intent = single `Command` variant.** Do not invent a parallel intent vocabulary per panel.
4. **Background → UI only via dedicated inbound variant.** The subscription bridge is the single inbound path.
5. **Stable widget IDs.** Give every stateful widget (scrollable, editor) an explicit `iced::widget::Id` from a `&'static str` const so the reducer can address it and the widget keeps its state across rebuilds. `Id::unique()` produces a different id every call — never use it for something you must address later.
6. **Strings live in a dedicated constants module** as documented `pub const` values or formatting helpers. No UI literals in panels, modals, trees or editors.
7. **Colours live in `Palette`** as tokens. No numeric literals anywhere else.
8. **Keep pane layout owned** by a dedicated layout module. Do not add ad-hoc side panels.
9. **Cross-cutting UI state** goes on `AppState` (domain on `app`, snapshots in `models`, floating windows in `overlay`). Split a new concern into its own module when it grows.
10. **Never block the UI thread.** Long work goes on a `Task` or a background worker; the loop must stay responsive.

### 6.1 Conditional Rendering

iced rebuilds the widget tree from state every frame, so the immediate-mode "keep the shape allocated" concern does not apply here. What does apply:

- Branch on the view-model, not on live widget state.
- Prefer `on_press_maybe(Some/None)` over removing a control when the action is merely unavailable.
- Keep stateful widgets' **ids** stable across the branch — a scrollable that disappears and returns under a different id loses its scroll position.
- Mount floating surfaces through the overlay host's z-order, never ad hoc.

### 6.2 Gotchas That Cost Time

| Gotcha | Consequence |
|---|---|
| `view` builds a view-model locally | E0515 — build it in `update` and store it |
| `Subscription::run` with captured state | Won't compile; use `run_with(data, builder)` with a hashable handle |
| `impl Stream` returned from a builder | Won't coerce to iced's `fn(&D) -> S`; box the stream |
| `tokio::task::*` inside iced | Panics — the executor is the futures thread-pool (unless `tokio` feature is enabled) |
| `text` `.center()` with default sizing | No-op; give it bounds larger than the content |
| `Float` at its natural origin | Stops floating; use `Stack` for full-area layers |
| `pick_list` row under a test selector | Invisible; drive the row model instead |
| Reading `master` docs against 0.14 | API differs; pin the release you build |

---

## 7. Where to Look

| Question | Go to |
|---|---|
| Full iced SDK reference | Official iced documentation / project `doc/iced.md` |
| Architecture picture | Project architecture docs |
| Event/message payloads | Project messaging docs |
| App root, boot, launch | Application entry point (e.g., `main.rs`, `app.rs`) |
| Reducer | `update` function module |
| Top-level view + overlay host | `view` function module |
| Bus → subscription bridge | `subscription` builder module |
| Message enum | Message type definition module |
| Pane geometry | Layout/pane grid module |
| Overlay/menu/modal primitives | Overlay primitives module |
| Markdown render path | Markdown render module |
| Tier-2 interaction tests | `iced_test::Simulator` test module |
| Tier-3/4 harness | Visual test harness directory |