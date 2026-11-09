# iced SDK Documentation

*Source: the [official iced book](https://github.com/iced-rs/book/tree/master/src) and the `iced` crate manifest (master branch).*

- [Overview](#overview)
- [Architecture & Core Concepts](#architecture--core-concepts)
  - [The GUI Trinity and The Elm Architecture](#the-gui-trinity-and-the-elm-architecture)
  - [The Runtime](#the-runtime)
- [API Reference](#api-reference)
  - [`iced::run`](#icedrun)
  - [`iced::Result`](#icedresult)
  - [`iced::widget::button`](#icedwidgetbutton--button)
  - [`iced::widget::text`](#icedwidgettext--text)
  - [`iced::widget::container`](#icedwidgetcontainer--container)
  - [Utility Functions](#utility-functions)
- [Testing & Developer Tools](#testing--developer-tools)
- [Community & Ecosystem](#community--ecosystem)
- [Configuration Reference](#configuration-reference)
  - [Installing iced](#installing-iced)
  - [Feature Flags](#feature-flags)
- [Error Handling](#error-handling)

---

## Overview

- **Name**: `iced` — a cross-platform GUI library for Rust, inspired by Elm.
- **Version**: `0.14.0` (latest published on crates.io; master is `0.15.0-dev`). This document reflects the `master` branch.
- **Language**: Rust only (edition 2024, MSRV `1.93`). JavaScript may provide the runtime on web targets; you MUST NOT write application code in it.
- **Platforms**: natively supports Windows, macOS, Linux (X11 and Wayland), and the browser via WebAssembly. Most widgets are portable across platforms.
- **Frameworks**: iced IS the framework. It ships a widget kit, a renderer stack (`wgpu`, `tiny-skia`), a windowing layer (`winit`), and a text shaper (`cosmic-text`).
- **License**: MIT.
- **Documentation generated**: 2026-10-17, from `iced-rs/book@master` (`fae769c`) and `iced-rs/iced@master`.

The documentation source is unfinished; its planned chapters (Layout, Styling, Concurrency, Scaling Applications, Extending the Runtime, Widgets, Subscriptions, Themes, Shells, Renderers) do not exist yet. This document covers only the material the source contains.

---

## Architecture & Core Concepts

### Design Philosophy

The project is opinionated. You SHOULD understand these principles before writing code, because the API design follows them:

- **The Elm Architecture is the model.** State, messages, update logic, and view logic are the four application-specific parts of an interface. Everything else is the runtime's job.
- **Rust is all you need.** No view DSL, no template language, no heavy proc-macros. Expect to use plain Rust: variables, `match` statements, nested scopes, iterators, functions, and generics. Small declarative macros exist (`row!`, `column!`, `text!`), but each has the same semantics as the ubiquitous `vec!` macro.
- **No HTML/CSS/JS split.** Layout, styling, and logic are written directly in Rust as a cohesive whole. You MUST NOT introduce a three-layer separation of structure, style, and logic.
- **Widget construction is pure.** Constructing and configuring widgets has no side effects and needs no global context. View code MAY move widgets around freely without breaking anything else.
- **Type safety first.** The library leverages the type system and compile-time guarantees to minimize runtime errors. Widget types carry the message type they produce.

### The GUI Trinity and The Elm Architecture

Every user interface has three foundational ideas, which form a feedback loop:

1. **Widgets** — the distinct visual elements of an interface (a button, some text).
2. **Interactions** — the actions widgets produce when a user interacts with them.
3. **State** — the underlying condition or information of the interface.

Widgets produce interactions; interactions change state; state dictates new widgets. Repeat.

Widgets are generic and reusable; state and interactions are application-specific. iced supplies the widgets; you supply the application-specific parts. The Elm Architecture names them:

| Elm term | iced term | Nature |
|---|---|---|
| Model | State | Application-specific ownership data |
| Messages | Messages | Pure data describing events that occurred |
| Update logic | `update` method | How messages change state |
| View logic | `view` method | How state dictates widgets |

Guidance from the source:

- Model messages as an `enum`. Interactions are mutually exclusive; an enum models "one value out of a set". Derive `Debug` and `Clone` (and `Copy` for small payloads). Message MUST always be easy to `Debug`/`Clone`.
- Think about the data model first, including states that must be impossible. Use the type system to make impossible states impossible.
- `update` and `view` are plain methods on the state. They remain UI-library-free until the view returns widgets, so you can unit-test update logic with no dependencies in sight.

### The Runtime

The runtime is the environment where the interface feedback loop takes place. It owns every part of the loop:

1. Initialize the **state** (once).
2. Run the **view logic** to obtain the interface; display it to the user.
3. Gather user **interactions** and turn them into **messages**.
4. Run the **update logic** for each message.
5. Go to step 2.

You never write this loop. `iced::run` implements it. It infers the state and message types from the signatures of your `update` and `view` functions, and initializes the state with `Default`.

Scheduling guarantees (per the FAQ):

- `view` and `subscription` run after every batch of messages and `update` calls.
- You MUST NOT rely on that cadence; treat `view` and `subscription` as declarative, stateless functions.
- iced does NOT redraw all the time. Since `0.14`, reactive rendering is enabled by default and redraws only when a widget requests it.

Under the hood, the default runtime stack is `winit` (windowing), `softbuffer`/`wgpu`/`tiny-skia` (rendering), and `cosmic-text` (shaping and layout).

---

## API Reference

### `iced::run`

#### Overview
Entry point that wires your update and view logic into the runtime and starts the event loop. One call per process.

#### Prerequisites & Requirements
- Your state type MUST implement `Default`. The runtime initializes state with it. The state is inferred from the signature of `update`.
- Your message type MUST derive `Debug` and `Clone` (and MAY derive `Copy`). Widgets producing it must be able to emit it repeatedly.
- You MUST call `iced::run` from `pub fn main`. It returns an `iced::Result`, which `main` can return directly.
- You MUST pass the update logic first, then the view logic: `iced::run(Counter::update, Counter::view)`. The runtime infers both state and message types from these signatures; do not add extra state to fill in.
- Your `view` logic MUST return a widget `Element<Message>`, and the widgets it builds MUST have the same message type, since the type parameter ties them together.

#### Syntax / Method Signature
```rust
pub fn main() -> iced::Result {
    iced::run(Counter::update, Counter::view)
}
```

#### Examples

Minimal counter application (the full "first steps" program):

```rust
use iced::widget::{button, column, text, Column};

pub fn main() -> iced::Result {
    iced::run(Counter::update, Counter::view)
}

#[derive(Default)]
struct Counter {
    value: i64,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Increment,
    Decrement,
}

impl Counter {
    fn update(&mut self, message: Message) {
        match message {
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
        }
    }

    fn view(&self) -> Column<'_, Message> {
        column![
            button("+").on_press(Message::Increment),
            text(self.value),
            button("-").on_press(Message::Decrement),
        ]
    }
}
```

Note the return type `Column<'_, Message>`: the `column!` macro produces a widget of the `Column` type, and its message type parameter is `Message` because the buttons inside produce `Message` values.

#### Type references
- [`iced::run`](https://docs.iced.rs/iced/fn.run.html)
- [`iced::Result`](https://docs.iced.rs/iced/type.Result.html)

#### Return values
An `iced::Result` — `Ok(())` after the runtime exits cleanly, otherwise an error (see [Error Handling](#error-handling)).

#### Side effects
Starts the OS event loop, opens windows, spawns the renderer, and manages application state. You MUST NOT call it more than once per process; you MUST NOT treat the state you pass to `view` as shared with other threads.

#### References
- Book: [First Steps](https://github.com/iced-rs/book/blob/master/src/first-steps.md), [The Runtime](https://github.com/iced-rs/book/blob/master/src/the-runtime.md)

---

### `iced::Result`

#### Overview
Result type returned by `iced::run` and by `main` in every example.

#### Prerequisites & Requirements
- You MUST propagate the result (return it from `main`); you MUST NOT discard it. Depending on `display`, it may be a unit type, but the API contract does not guarantee that.

#### Syntax / Method Signature
```rust
pub fn main() -> iced::Result {
    iced::run(Counter::update, Counter::view)
}
```

#### Type references
- [`iced::Result`](https://docs.iced.rs/iced/type.Result.html)

#### Return values
The result of the application. On the web, the result is likely a unit type; native platforms return an error if the runtime could not start (see [Error Handling](#error-handling)).

#### References
- Book: [The Runtime](https://github.com/iced-rs/book/blob/master/src/the-runtime.md)

---

### `iced::widget::button` / `Button`

#### Overview
A widget the user presses to trigger an interaction. One of the core interactive building blocks.

#### Prerequisites & Requirements
- You MUST set a message with `.on_press(...)`. A button with no message is a disabled, inert widget.
- The message passed to `.on_press` MUST be the same generic type the enclosing widget tree produces. This is a type-level contract.
- The message type MUST implement `Clone` because pressing a button yields a fresh copy each time; it SHOULD be `Copy` when the message is small and it MUST derive `Debug` for diagnostics.

#### Syntax / Method Signature
```rust
use iced::widget::button;

fn view(&self) -> Column<'_, Message> {
    column![
        button("+").on_press(Message::Increment),
        button("-").on_press(Message::Decrement),
    ]
}
```

#### Examples

Building and wiring a pair of buttons to a pair of messages:

```rust
use iced::widget::{button, column, text, Column};

fn view(&self) -> Column<'_, Message> {
    column![
        button("+").on_press(Message::Increment),
        text(self.value),
        button("-").on_press(Message::Decrement),
    ]
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Increment,
    Decrement,
}
```

The helper takes one argument that describes the button contents. It accepts an `Into<Element>`; see [`text`](#icedwidgettext--text) for how `'static` strings fit in.

#### Type references
- [`iced::widget::Button`](https://docs.rs/iced/latest/iced/widget/struct.Button.html)

#### Return values
A `Button<'_, Message>` — an owned widget value configured for the message type `Message`. Widget helpers return values; they have no side effects.

#### References
- Book: [First Steps](https://github.com/iced-rs/book/blob/master/src/first-steps.md)

---

### `iced::widget::text` / `Text`

#### Overview
The most essential widget. Displays a string, number, or any `Into<Element>` value.

#### Prerequisites & Requirements
- You MUST import `iced::widget::text` for the function helper, the `text` module (for built-in styles), and the `text!` macro; a single import gives you all three.
- You MUST use the `text` helper if you need to customize appearance; the `From<&str>` shortcut is for default appearance only.
- Text alignment only works if the sizing strategy yields bounds larger than the content. You MUST set `width` or `height` explicitly (for example, to `Fill`) when you want `.center()` to have an effect. With the default intrinsic sizing, alignment is a no-op.

#### Syntax / Method Signature
```rust
use iced::widget::text;
use iced::{Fill, Font, Theme};

text("- Hello there!\n- General Kenobi!")
    .font(Font::MONOSPACE)   // choose a font
    .size(30)               // logical pixels
    .line_height(1.5)       // relative to the size (= 45 px)
    .width(Fill)            // expand to fill the parent width
    .height(Fill)           // expand to fill the parent height
    .center()               // center once bounds exceed the content
```

#### Examples

Theming (the recommended way to color text from the current theme):

```rust
use iced::widget::text;
use iced::Theme;

text("This is the primary color of the current theme!")
    .style(|theme: &Theme| text::Style {
        color: Some(theme.palette().primary),
    })
```

Using a built-in color helper:

```rust
use iced::widget::text;

text("And this is the warning color!")
    .style(text::warning)
```

Centering inside fixed bounds:

```rust
use iced::widget::text;

text("This text will be centered inside a 150x150 square")
    .width(150)
    .height(150)
    .center()
```

The `text!` macro behaves like `format!` but returns a `Text` widget:

```rust
let name = "Héctor";

text!("Hello, {name}!")
```

String slices accepted directly, because `Text` provides `From<&str>` for `Element`:

```rust
use iced::widget::container;

container("Short and nice!") // same as container(text("Short and nice!"))
```

#### Type references
- [`iced::widget::Text`](https://docs.rs/iced/latest/iced/widget/type.Text.html)
- [`iced::widget::text` module](https://docs.rs/iced/latest/iced/widget/text/index.html)
- [`iced::Element`](https://docs.rs/iced/latest/iced/type.Element.html)

#### Return values
A `Text` widget configured with the given content and builder options. The `text` module types the styling closure (`text::Style`).

#### Side effects
None. Widget construction is side effect free.

#### References
- Book: [Text](https://github.com/iced-rs/book/blob/master/src/text.md)

---

### `iced::widget::container` / `Container`

#### Overview
One of the main building blocks of iced. Applies padding, alignment, and styling to another widget. Use it to position and decorate.

#### Prerequisites & Requirements
- Content sizing is inherited by default. All alignment methods therefore take a new `Length` (for example `Fill`) to override the inherited sizing and produce the bounds in which content is positioned. You MUST pass an explicit sizing value if you want alignment to take effect.
- `padding` accepts values from the `iced::padding` module for per-side control; you MAY pass a single number for uniform padding.

#### Syntax / Method Signature
```rust
use iced::widget::container;

container("This is a bordered box!")
    .padding(10)                    // uniform padding
    .style(container::bordered_box) // built-in theme-aware style
```

#### Examples

Independent padding per side:

```rust
use iced::padding;
use iced::widget::container;

container("Independent padding!")
    .padding(padding::vertical(30).left(20).right(80))
    .style(container::bordered_box)
```

Nested containers for layout and decoration; alignment is like framing a picture:

```rust
use iced::widget::container;
use iced::Fill;

let my_box = container("Bottom right!")
    .padding(10)
    .style(container::primary);

container(my_box)
    .padding(10)
    .align_bottom(Fill)
    .align_right(Fill)
    .style(container::bordered_box)
```

#### Type references
- [`iced::widget::Container`](https://docs.rs/iced/latest/iced/widget/struct.Container.html)
- [`iced::widget::container` module](https://docs.rs/iced/latest/iced/widget/container/index.html)
- [`iced::padding` module](https://docs.rs/iced/latest/iced/padding/index.html)

#### Return values
A `Container<'_, Message>` wrapping the given content and carrying the chosen builder configuration.

#### References
- Book: [Container](https://github.com/iced-rs/book/blob/master/src/container.md)

---

### Utility Functions

Conveniences from the book that are not part of a single widget type.

#### `column!` / `row!`

##### Overview
Declarative macros that build a `Column` or `Row` widget from a comma-separated list of children, in order. Semantics mirror `vec!`.

##### Syntax / Method Signature
```rust
use iced::widget::{button, column, text, Column};

impl Counter {
    fn view(&self) -> Column<'_, Message> {
        column![ // produces Column<'_, Message>
            button("+").on_press(Message::Increment),
            text(self.value),
            button("-").on_press(Message::Decrement),
        ]
    }
}
```

##### Prerequisites & Requirements
- All children MUST produce the same message type. The macro infers it; mismatched children will not compile.

##### Type references
- [`iced::widget::Column`](https://docs.rs/iced/latest/iced/widget/struct.Column.html)

#### `From<&str>` for `Element`

##### Overview
iced provides `From<&str>` for the `Element` type, so widget helpers that take `Into<Element>` accept a string slice directly.

##### Prerequisites & Requirements
- Only for default text appearance. You MUST use the `text` helper if you need to customize the appearance.

##### Examples
```rust
// Verbose:
container(text("Redundant"))
// Equivalent:
container("Short and nice!")
```

##### Type references
- [`iced::Element`](https://docs.rs/iced/latest/iced/type.Element.html)

#### Concurrency primitives

##### Overview
Two ways to receive asynchronous updates (see the FAQ):

| API | Use it when |
|---|---|
| [`Task::run`](https://docs.rs/iced/latest/iced/task/struct.Task.html#method.run) | You have an asynchronous [`Stream`](https://docs.rs/futures/latest/futures/stream/trait.Stream.html) and want to turn it into messages. |
| [`Subscription::run`](https://docs.rs/iced/latest/iced/struct.Subscription.html#method.run) | You control the creation of the source channel yourself. |

Return these from `update` or `subscription` respectively; the runtime drives them.

---

## Testing & Developer Tools

### `iced_test`

#### Overview
Headless testing library for iced applications, published as [`iced_test` 0.14.0 on crates.io](https://crates.io/crates/iced_test) (released alongside iced 0.14; MIT; MSRV 1.88). It tests widgets and whole applications without opening a window — including screenshots.

#### Prerequisites & Requirements
- `Simulator`-based tests MUST receive a fully built `Element` (the result of `view()`); program-level APIs (`run`, `screenshot`, `Emulator`) MUST receive a type implementing `program::Program`.
- A `Selector` queries the tree. `&str` selects a widget by the text it contains, so text used for selection MUST be stable and you MUST NOT rely on formatting-specific strings for assertion targets. Custom selectors MAY be implemented via the `Selector` trait on `iced::widget::Id` keys.
- `.ice` test suites run the real program — side effects WILL take place. Use `program::Preset` to inject reproducible state; you MUST NOT rely on ambient user config in tests.

#### Syntax / Method Signature
```rust
use iced_test::simulator; // or iced_test::{run, screenshot, Emulator, Ice, Instruction}
```

#### Examples

Widget-level: build a `Simulator` from `view()`, click by text, replay messages into `update`, and query the rebuilt view:

```rust,no_run
use iced_test::simulator;

let mut counter = Counter { value: 0 };
let mut ui = simulator(counter.view());

let _ = ui.click("+");
let _ = ui.click("+");
let _ = ui.click("-");

for message in ui.into_messages() {
    counter.update(message);
}

assert_eq!(counter.value, 1);
assert!(ui.find("1").is_ok(), "Counter should display 1!");
```

Additional `Simulator` operations: `tap_key`, `typewrite`, and `snapshot` (snapshot testing).

Program-level: run a directory of `.ice` test files through an emulator of the real program, or capture a single screenshot:

```rust
iced_test::run(&program, "tests/iced")?; // executes every *.ice file
iced_test::screenshot(&program, &theme, (800, 500), 1.0, duration); // -> window::Screenshot
```

#### Type references
- [`Simulator`](https://docs.rs/iced_test/0.14.0/iced_test/struct.Simulator.html) — headless interface driver (click/find/typewrite/snapshot).
- [`Selector`](https://docs.rs/iced_test/0.14.0/iced_test/trait.Selector.html) — widget query trait (`&str` selects by contained text).
- `Ice`, `Instruction`, `Emulator` — `.ice` file parsing and replay against a `Program`.
- `Error` — parse/execution failure types, including `IceTestingFailed`.

#### Return values
`run` returns `Result<(), Error>`; `screenshot` returns a `window::Screenshot`; `Simulator` calls return per-click results (`Result`-shaped) and `into_messages` yields the produced messages.

#### Side effects
`Simulator` is side-effect free apart from the messages it collects. `run`/`Emulator` execute the actual program: timers, subscriptions and I/O run for real.

#### References
- [Crate docs (`iced_test` 0.14.0)](https://docs.rs/iced_test/0.14.0/iced_test/)
- [Source](https://github.com/iced-rs/iced/tree/0.14.0/test)

### `iced_tester`

#### Overview
Official test **recorder, editor, and runner** for iced applications ([`iced_tester` 0.14.0](https://crates.io/crates/iced_tester)), enabled in the `iced` crate by the `tester` feature. Record interactions from a running app (press F12 to open the tool) into `.ice` files for later replay.

#### Prerequisites & Requirements
- You MUST enable `iced = { features = ["tester"] }`. [RUST: compiled out otherwise]
- Recorder output MUST be replayed with `iced_test::run` or manually via `Emulator`/`Instruction`.
- Recording captures real program behavior; recorded suites MUST be kept deterministic with `program::Preset` state injection.

#### Type references
- [`iced_tester` on crates.io](https://crates.io/crates/iced_tester) — the `tester` feature tool.
- [`iced_test::run`](https://docs.rs/iced_test/0.14.0/iced_test/fn.run.html) — `.ice` replay entry point.

#### References
- Feature definition: `tester = ["dep:iced_tester"]` in the [`iced` crate manifest](https://docs.rs/crate/iced/latest/features#tester).

---

## Community & Ecosystem

### `iced_aw`

#### Overview
Community widget collection for iced ([lib.rs](https://lib.rs/crates/iced_aw)); every widget is behind a Cargo feature gate so crates can cherry-pick. Relevant to this SDK: the **`ContextMenu`** widget, which provides a right-click menu wrapper.

#### Prerequisites & Requirements
- It is a third-party crate — it MAY lag iced releases. You MUST verify the published version targets iced 0.14 before adoption. [UNVERIFIED: no iced-0.14 release confirmed during research on 2026-10-17]
- Known open issues as of 2026-10-17: hover state is not rendered inside the ContextMenu overlay ([#378](https://github.com/iced-rs/iced_aw/issues/378)); a modified `ContextMenu` style can hide the entire window ([#363](https://github.com/iced-rs/iced_aw/issues/363)). Evaluate before production use.

#### Alternatives built into iced (preferred, verified in 0.14)
- **`iced::widget::overlay::menu`** — built-in "Build and show dropdown menus. A list of selectable options" module with its own `State` and `Style` types. Dropdowns are covered natively.
- **Hand-rolled overlay**: wrap the target widget in `iced::widget::MouseArea` (press events), then render the menu via the `overlay`/`Stack` widget when pressed. This is the pattern used by the official examples for popups not covered by `overlay::menu`.

#### References
- [iced::widget::overlay::menu docs](https://docs.rs/iced/latest/iced/widget/overlay/menu/index.html)
- [iced::widget::overlay module](https://docs.rs/iced/latest/iced/widget/overlay/index.html)

---

## Configuration Reference

### Installing iced

Quick start, directly from the book:

```rust
// Cargo.toml
[dependencies]
iced = "0.14.0"
```

You then use `iced::run`, `iced::widget::{button, column, text}`, and friends from the `iced` crate.

### Feature Flags

The default feature set is `["wgpu", "tiny-skia", "crisp", "web-colors", "thread-pool", "linux-theme-detection", "x11", "wayland", "wayland-dlopen", "wayland-decorations"]`.

The complete feature list, straight from the crate manifest, is:

| Feature | What it enables |
|---|---|
| `wgpu` | The `wgpu` GPU-accelerated renderer with all its default backends. |
| `wgpu-bare` | The `wgpu` renderer with the minimum required features (no backends). |
| `tiny-skia` | The `tiny-skia` software renderer. |
| `image` | The `image` widget and image clipboard support. |
| `image-without-codecs` | The `image` widget, without any built-in codecs from the `image` crate. |
| `svg` | The `svg` widget. |
| `canvas` | The `canvas` widget. |
| `qr_code` | The `qr_code` widget. |
| `markdown` | The `markdown` widget. |
| `debug` | Debug metrics on native platforms (press F12). |
| `time-travel` | Time-travel debugging. Very experimental. |
| `hot` | Hot reloading. Very experimental. |
| `tester` | The tester developer tool for recording and playing tests (press F12). |
| `thread-pool` | The `thread-pool` futures executor as `executor::Default` on native platforms. |
| `tokio` | The `tokio` executor as `executor::Default` on native platforms. |
| `smol` | The `smol` executor as `executor::Default` on native platforms. |
| `sysinfo` | Querying system information. |
| `web-colors` | Broken "sRGB linear" blending to reproduce color management of the web. |
| `crisp` | Pixel snapping for crisp edges by default. Can cause jitter. |
| `webgl` | The WebGL backend. |
| `highlighter` | Syntax highlighting. |
| `selector` | The `widget::selector` module. |
| `advanced` | The `advanced` module. |
| `fira-sans` | Embeds Fira Sans in the final application. Useful for testing and Wasm builds. |
| `basic-shaping` | Basic text shaping by default. |
| `advanced-shaping` | Advanced text shaping by default. |
| `strict-assertions` | Strict assertions for debugging, at the expense of performance. |
| `unconditional-rendering` | Redraws on every runtime event and not only when a widget requests it. |
| `sipper` | Support for the `sipper` library. |
| `linux-theme-detection` | Linux system theme detection. |
| `x11` | The Unix X11 backend. |
| `wayland` | The Unix Wayland backend. |
| `wayland-dlopen` | Dynamic linking of Wayland libraries. |
| `wayland-decorations` | Client-side decorations in Wayland. |

The `0.14` FAQ calls out one case where the default feature set is not enough — see [Error Handling](#error-handling) for the `tokio` example.

---

## Error Handling

The book surfaces one error pattern explicitly, reported by users as a runtime panic:

> **Symptom**
>
> ```text
> there is no reactor running, must be called from the context of a Tokio 1.x runtime
> ```
>
> **Why**
>
> You used a `Task` to execute a `Future` that requires the `tokio` executor, but you did not enable `tokio` on the `iced` crate. In particular, `Task::run` should be driven by a specific executor's reactor.
>
> **Fix**
>
> ```toml
> iced = { version = "0.14", features = ["tokio"] }
> ```

General guidance distilled from the book:

- There is no documented list of distinct error codes. Practically, failures surface as `Result` errors from `iced::run` or as panics. An agent SHOULD be prepared for both.
- You MUST NOT copy code from the `master` branch into a project pinned to an older crates.io release and expect it to compile (the FAQ calls this out as a common contributor mistake). Pin the release tag you actually use.
- The API may change wholesale between releases. Read the changelog for each upgrade step before you migrate.

[Official examples]: https://github.com/iced-rs/iced/tree/latest/examples
[Release tags]: https://github.com/iced-rs/iced/tags
