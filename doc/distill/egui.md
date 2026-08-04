# egui SDK Reference

## Overview

| Property | Value |
|---|---|
| Name | egui |
| Version | 0.35.0 |
| Language | Rust (edition 2024) |
| Minimum Rust | 1.95 |
| License | MIT OR Apache-2.0 |
| Architecture | Immediate Mode GUI |
| unsafe code | Forbidden |
| Documentation Date | 2026-07-29 |

egui is an easy-to-use, portable, immediate mode GUI library for Rust. It runs on the web (Wasm + WebGL), natively (Linux, Mac, Windows, Android), and inside game engines.

### Workspace Crates

| Crate | Purpose |
|---|---|
| `egui` | Core GUI library. Widgets, layout, containers, input handling, style. |
| `emath` | Minimal 2D math: `Vec2`, `Pos2`, `Rect`, `Align`, `lerp`, `remap`. |
| `ecolor` | Color types: `Color32`, `Rgba`, `Hsva`, `HsvaGamma`. |
| `epaint` | 2D shapes and text rendering. Tessellates shapes to textured triangles. |
| `epaint_default_fonts` | Embedded default font files (.ttf/.otf). |
| `eframe` | Official framework. Compile the same app to web or native. |
| `egui-winit` | Integration with the `winit` windowing library. |
| `egui_glow` | Rendering backend using `glow` (OpenGL). |
| `egui-wgpu` | Rendering backend using `wgpu` (WebGPU). |
| `egui_extras` | Additional widgets: `Table`, `Strip`, syntax highlighting, image loaders, date picker. |
| `egui_kittest` | Test harness based on `kittest` and `AccessKit`. |

---

## Architecture & Core Concepts

### Immediate Mode

egui uses **immediate mode**. Every frame, the application code rebuilds all widgets, lays them out, and paints them. The library does not retain widgets between frames. State is stored separately via `Context::data()` / `Context::memory()`, keyed by stable `Id` values.

### Frame Lifecycle

```
1. Gather input (mouse, keyboard, screen size, etc.) → RawInput
2. Call Context::run_ui(raw_input, |ui| { /* app code */ })
   a. Panels and containers create child Ui instances
   b. Widgets are built, laid out, and painted
   c. Interaction is detected against previous frame rects
3. Handle output (clipboard, open URL, cursor changes, texture updates)
4. Tessellate shapes → ClippedPrimitive list (triangles + paint callbacks)
5. Render triangles via backend (glow, wgpu, or custom)
```

### Id System

Every widget and container gets a unique `Id`. Ids are generated from a hierarchical chain of `IdSource` values. Widgets with the same label text on the same level get the same Id automatically. For dynamic content, use `ui.push_id()` to salt the Id chain.

### Layout

Layout is driven by `Layout` struct with main direction (`LeftToRight`, `RightToLeft`, `TopDown`, `BottomUp`), wrapping, alignment, and justification. The `Ui` tracks a cursor position within a `max_rect`. Widgets allocate space from left-to-right/top-to-bottom by default.

### Widget Trait

```rust
pub trait Widget {
    fn ui(self, ui: &mut Ui) -> Response;
}
```

Every widget implements this trait. Call `ui.add(widget)` or use convenience methods like `ui.button("text")`.

### Response Chaining

Every `Widget::ui()` call returns a `Response`. Responses support:
- Boolean queries: `.clicked()`, `.hovered()`, `.dragged()`, `.changed()`
- Tooltip chaining: `.on_hover_text("tooltip")`
- Scroll: `.scroll_to_me(align)`
- Focus: `.request_focus()`, `.surrender_focus()`

### Containers

Containers use the `show` pattern:

```rust
Window::new("Title").show(ctx, |ui| { /* child UI */ });
ScrollArea::vertical().show(ui, |ui| { /* child UI */ });
Frame::none().fill(color).show(ui, |ui| { /* child UI */ });
```

Containers return `InnerResponse<R>` with the closure return value and the container's own `Response`.

---

## API Reference

### `egui::Context`

#### Overview

Central handle to the egui UI system. Cloneable (`Arc`-based with `RwLock`). Provides access to input, memory, style, output, fonts, and graphics layers. All state-access methods use closures to prevent deadlocks.

#### Prerequisites & Requirements

- A `Context` MUST be created before any UI calls.
- The same `Context` MUST be used for the entire frame lifecycle.
- `Context` methods that lock state MUST NOT be called inside a closure passed to another state-access method (deadlock prevention).

#### Syntax / Method Signature

```rust
impl Context {
    // Lifecycle
    pub fn new() -> Self;
    pub fn run_ui(&self, new_input: RawInput, run_ui: impl FnMut(&mut Ui)) -> FullOutput;
    pub fn begin_pass(&self, new_input: RawInput);
    pub fn end_pass(&self) -> FullOutput;
    pub fn tessellate(&self, shapes: Vec<ClippedShape>, pixels_per_point: f32) -> Vec<ClippedPrimitive>;

    // State access (closure pattern)
    pub fn input<R>(&self, reader: impl FnOnce(&InputState) -> R) -> R;
    pub fn memory<R>(&self, reader: impl FnOnce(&Memory) -> R) -> R;
    pub fn data<R>(&self, reader: impl FnOnce(&IdTypeMap) -> R) -> R;
    pub fn options<R>(&self, reader: impl FnOnce(&Options) -> R) -> R;
    pub fn output<R>(&self, reader: impl FnOnce(&PlatformOutput) -> R) -> R;
    pub fn fonts<R>(&self, reader: impl FnOnce(&FontsView<'_>) -> R) -> R;

    // Mutable state access (closure pattern)
    pub fn input_mut<R>(&self, writer: impl FnOnce(&mut InputState) -> R) -> R;
    pub fn memory_mut<R>(&self, writer: impl FnOnce(&mut Memory) -> R) -> R;
    pub fn data_mut<R>(&self, writer: impl FnOnce(&mut IdTypeMap) -> R) -> R;
    pub fn options_mut<R>(&self, writer: impl FnOnce(&mut Options) -> R) -> R;
    pub fn output_mut<R>(&self, writer: impl FnOnce(&mut PlatformOutput) -> R) -> R;
    pub fn fonts_mut<R>(&self, reader: impl FnOnce(&mut FontsView<'_>) -> R) -> R;

    // Style
    pub fn set_visuals(&self, visuals: Visuals);
    pub fn set_fonts(&self, font_definitions: FontDefinitions);
    pub fn set_theme(&self, theme_preference: impl Into<ThemePreference>);
    pub fn global_style(&self) -> Arc<Style>;

    // Input queries
    pub fn is_pointer_over_egui(&self) -> bool;
    pub fn egui_wants_pointer_input(&self) -> bool;
    pub fn egui_wants_keyboard_input(&self) -> bool;

    // Coordinates
    pub fn viewport_rect(&self) -> Rect;
    pub fn content_rect(&self) -> Rect;

    // Repaint
    pub fn request_repaint(&self);
    pub fn request_repaint_after(&self, duration: Duration);
    pub fn request_discard(&self, reason: impl Into<Cow<'static, str>>);

    // Textures
    pub fn load_texture(&self, name: String, image: ImageData, options: TextureOptions) -> TextureHandle;

    // Plugins
    pub fn add_plugin(&self, plugin: impl Plugin + 'static);
}
```

#### Examples

```rust
let ctx = egui::Context::default();

// Set theme
ctx.set_theme(egui::ThemePreference::Dark);
ctx.set_fonts(my_font_definitions);

// Run one frame
let raw_input = egui::RawInput {
    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
    ..Default::default()
};
let output = ctx.run_ui(raw_input, |ui| {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.label("Hello world");
    });
});

// Tessellate and render
let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
```

#### Type References

- `RawInput` - Input state for one frame
- `FullOutput` - Shapes, textures delta, platform output
- `PlatformOutput` - Commands: open URL, copy text, cursor changes
- `Memory` - Persistent per-frame state (window positions, scroll, etc.)
- `IdTypeMap` - Arbitrary per-id data storage
- `FontsView` - Font layout service for a given pixels_per_point
- `Style` - Complete visual style (spacing, visuals, text_styles)

#### Return Values

- `run_ui` returns `FullOutput` containing shapes, textures delta, platform output.
- State accessor closures return whatever `R` the closure produces.

#### Side effects

- `run_ui` advances internal frame counters.
- `request_repaint` schedules the platform to call the UI loop again.
- `load_texture` allocates space in the internal texture atlas.

#### References

- Source: `crates/egui/src/context.rs`

---

### `egui::Ui`

#### Overview

Represents a rectangular screen region with a active layout. Deref's to `Context`, so all `Context` methods are available on `Ui`. Created by containers (`Window::show`, `CentralPanel::show`) or by scope methods (`ui.horizontal()`, `ui.scope()`).

#### Prerequisites & Requirements

- A `Ui` MUST NOT outlive its creating `Context`.
- `Ui` methods that change layout state MUST NOT be called from multiple threads without synchronization.

#### Syntax / Method Signature

```rust
impl Ui {
    // Adding widgets
    pub fn add(&mut self, widget: impl Widget) -> Response;
    pub fn add_sized(&mut self, max_size: impl Into<Vec2>, widget: impl Widget) -> Response;
    pub fn add_enabled(&mut self, enabled: bool, widget: impl Widget) -> Response;
    pub fn add_visible(&mut self, visible: bool, widget: impl Widget) -> Response;

    // Convenience widget shortcuts
    pub fn label(&mut self, text: impl Into<WidgetText>) -> Response;
    pub fn button(&mut self, atoms: impl IntoAtoms) -> Response;
    pub fn checkbox(&mut self, checked: &mut bool, atoms: impl IntoAtoms) -> Response;
    pub fn radio(&mut self, selected: bool, atoms: impl IntoAtoms) -> Response;
    pub fn text_edit_singleline<S: TextBuffer>(&mut self, text: &mut S) -> Response;
    pub fn text_edit_multiline<S: TextBuffer>(&mut self, text: &mut S) -> Response;
    pub fn code_editor<S: TextBuffer>(&mut self, text: &mut S) -> Response;
    pub fn hyperlink(&mut self, url: impl ToString) -> Response;
    pub fn separator(&mut self) -> Response;
    pub fn spinner(&mut self) -> Response;
    pub fn image(&mut self, source: impl Into<ImageSource>) -> Response;
    pub fn heading(&mut self, text: impl Into<RichText>) -> Response;
    pub fn colored_label(&mut self, color: impl Into<Color32>, text: impl Into<RichText>) -> Response;

    // Layout scopes
    pub fn horizontal<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
    pub fn vertical<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
    pub fn with_layout<R>(&mut self, layout: Layout, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
    pub fn scope<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
    pub fn push_id<R>(&mut self, id_salt: impl AsIdSalt, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
    pub fn indent<R>(&mut self, id_salt: impl AsIdSalt, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;

    // Sizing
    pub fn allocate_response(&mut self, desired_size: Vec2, sense: Sense) -> Response;
    pub fn allocate_exact_size(&mut self, size: Vec2, sense: Sense) -> (Rect, Response);
    pub fn allocate_painter(&mut self, size: Vec2, sense: Sense) -> (Response, Painter);
    pub fn available_size(&self) -> Vec2;
    pub fn cursor(&self) -> Rect;
    pub fn add_space(&mut self, amount: f32);

    // Interaction
    pub fn interact(&self, rect: Rect, id: Id, sense: Sense) -> Response;
    pub fn is_enabled(&self) -> bool;
    pub fn disable(&mut self);

    // Style access
    pub fn style(&self) -> &Arc<Style>;
    pub fn visuals(&self) -> &Visuals;
    pub fn spacing(&self) -> &Spacing;
}
```

#### Examples

```rust
// Basic layout
ui.horizontal(|ui| {
    ui.label("Name:");
    ui.text_edit_singleline(&mut name);
});

// Conditional widget
if ui.button("Save").clicked() {
    save_data();
}

// Sized widget
ui.add_sized([200.0, 30.0], egui::Button::new("Wide Button"));

// Disabled section
ui.add_enabled(false, |ui| {
    ui.label("This is disabled");
});
```

#### Type References

- `Widget` - Trait for all widgets
- `Response` - Returned by every widget add
- `InnerResponse<R>` - Container result with inner value and response
- `Sense` - Interaction flags: `click()`, `drag()`, `hover()`
- `Layout` - Direction, wrapping, alignment
- `WidgetText` / `RichText` - Text with formatting
- `TextBuffer` - Trait for text edit sources (`String`, `SmartString`, etc.)
- `ImageSource` - URI, texture, or bytes

#### Return Values

- `add()` / convenience methods return `Response`.
- Layout scopes return `InnerResponse<R>`.

#### Side effects

- Adds shapes to the internal painter for rendering.
- Allocates space in the current layout cursor.
- Tracks interaction state in `Context::memory()`.

#### References

- Source: `crates/egui/src/ui.rs`

---

### `egui::Window`

#### Overview

A movable, resizable window with a title bar. Windows can be minimized, closed, and dragged. They retain their position and size across frames.

#### Prerequisites & Requirements

- `Window::show()` MUST be called with a `&Context` or `&Ui` (if inside another container).
- Window title MUST be unique (or provide a custom `id` / `id_source`).

#### Syntax / Method Signature

```rust
impl Window {
    pub fn new(title: impl Into<WidgetText>) -> Self;

    // Builder methods
    pub fn id(self, id: Id) -> Self;
    pub fn id_source(self, id_source: impl AsIdSalt) -> Self;
    pub fn title_bar(self, show: bool) -> Self;
    pub fn movable(self, movable: bool) -> Self;
    pub fn resizable(self, resizable: bool) -> Self;
    pub fn collapsible(self, collapsible: bool) -> Self;
    pub fn closable(self, closable: bool) -> Self;
    pub fn minimizable(self, minimizable: bool) -> Self;
    pub fn open(self, open: &mut bool) -> Self;
    pub fn default_open(self, default_open: bool) -> Self;
    pub fn default_size(self, size: impl Into<Vec2>) -> Self;
    pub fn min_size(self, min_size: impl Into<Vec2>) -> Self;
    pub fn max_size(self, max_size: impl Into<Vec2>) -> Self;
    pub fn fixed_size(self, fixed_size: impl Into<Vec2>) -> Self;
    pub fn current_pos(self, pos: Pos2) -> Self;
    pub fn default_pos(self, pos: Pos2) -> Self;
    pub fn anchor(self, anchor: Align2, offset: Vec2) -> Self;
    pub fn constraints(self, constraints: Layout) -> Self;
    pub fn scroll2(self, scroll: Vec2) -> Self;
    pub fn auto_sized(self) -> Self;
    pub fn title_bar_style(self, style: WidgetType) -> Self;
    pub fn frame(self, frame: Option<Frame>) -> Self;
    pub fn resize_speed(self, speed: f32) -> Self;
    pub fn enabled(self, enabled: bool) -> Self;
    pub fn sense(self, sense: Sense) -> Self;
    pub fn pivot(self, pivot: Align2) -> Self;
    pub fn interactable(self, interactable: bool) -> Self;
    pub fn fade_in(self, fading: bool) -> Self;
    pub fn classes(self, classes: impl IntoClasses) -> Self;

    pub fn show<R>(self, ctx: &Context, add_contents: impl FnOnce(&mut Ui) -> R) -> Option<InnerResponse<R>>;
    pub fn show_ctx<R>(self, ctx: &Context, add_contents: impl FnOnce(&mut Ui) -> R) -> Option<InnerResponse<R>>;
}
```

#### Examples

```rust
egui::Window::new("Settings")
    .default_size([300.0, 200.0])
    .collapsible(true)
    .resizable(true)
    .show(ctx, |ui| {
        ui.label("Window content here");
        ui.checkbox(&mut flag, "Enable feature");
    });

// Closable window with open state
let mut is_open = true;
egui::Window::new("Temporary")
    .open(&mut is_open)
    .show(ctx, |ui| {
        ui.label("Close me");
    });
```

#### Type References

- `WidgetText` - Window title type
- `Align2` - Anchor position (e.g. `Align2::RIGHT_TOP`)
- `Id` - Unique identifier
- `InnerResponse` - Contains the closure return value and the window's response

#### Return Values

- `show()` returns `Option<InnerResponse<R>>`. Returns `None` if the window is closed or minimized.
- The `InnerResponse.response` provides `.clicked()`, `.hovered()`, etc.

#### Side effects

- Allocates space in the `Context` for window state (position, size, scroll).
- Adds shapes to the painter for the window frame and title bar.

#### References

- Source: `crates/egui/src/containers/window.rs`

---

### `egui::Button`

#### Overview

A clickable button widget. Supports text, images, or both. Follows the builder pattern.

#### Prerequisites & Requirements

- MUST be passed to `ui.add()` or used via `ui.button()` convenience method.

#### Syntax / Method Signature

```rust
impl<'a> Button<'a> {
    pub fn new(atoms: impl IntoAtoms<'a>) -> Self;
    pub fn selectable(selected: bool, atoms: impl IntoAtoms<'a>) -> Self;
    pub fn image(image: impl Into<Image<'a>>) -> Self;
    pub fn image_and_text(image: impl Into<Image<'a>>, text: impl IntoAtoms<'a>) -> Self;

    // Builder methods
    pub fn wrap_mode(self, wrap_mode: TextWrapMode) -> Self;
    pub fn wrap(self) -> Self;
    pub fn truncate(self) -> Self;
    pub fn fill(self, fill: impl Into<Color32>) -> Self;
    pub fn stroke(self, stroke: impl Into<Stroke>) -> Self;
    pub fn small(self) -> Self;
    pub fn frame(self, frame: bool) -> Self;
    pub fn frame_when_inactive(self, frame: bool) -> Self;
    pub fn sense(self, sense: Sense) -> Self;
    pub fn min_size(self, min_size: impl Into<Vec2>) -> Self;
    pub fn corner_radius(self, corner_radius: impl Into<CornerRadius>) -> Self;
    pub fn selected(self, selected: bool) -> Self;
    pub fn shortcut_text(self, shortcut_text: impl Into<WidgetText>) -> Self;
    pub fn left_text(self, text: impl Into<WidgetText>) -> Self;
    pub fn right_text(self, text: impl Into<WidgetText>) -> Self;
}
```

#### Examples

```rust
// Simple button
if ui.button("Click me").clicked() {
    println!("Clicked!");
}

// Builder pattern with styling
ui.add(egui::Button::new("Styled")
    .fill(egui::Color32::RED)
    .corner_radius(8.0)
    .min_size([100.0, 40.0]));

// With shortcut hint
ui.add(egui::Button::new("Save").shortcut_text("Ctrl+S"));
```

#### Type References

- `IntoAtoms` - Trait for button content (text, rich text, or image)
- `TextWrapMode` - `Wrap`, `Truncate`, `Extend`
- `Color32` - sRGBA color
- `Stroke` - Line width and color
- `Sense` - Interaction flags
- `CornerRadius` - Per-corner u8 radius values

#### Return Values

- Returns `Response`. Use `.clicked()`, `.secondary_clicked()`, `.double_clicked()`, `.hovered()`.

#### Side effects

- Allocates space in the current layout.
- Adds shapes to the painter for the button frame and text.

#### References

- Source: `crates/egui/src/widgets/button.rs`

---

### `egui::Label`

#### Overview

A text label widget. Supports wrapping, truncation, text selection, and hover tooltips.

#### Prerequisites & Requirements

- MUST be passed to `ui.add()` or used via `ui.label()` convenience method.

#### Syntax / Method Signature

```rust
impl Label {
    pub fn new(text: impl Into<WidgetText>) -> Self;

    pub fn wrap_mode(self, wrap_mode: TextWrapMode) -> Self;
    pub fn wrap(self) -> Self;
    pub fn truncate(self) -> Self;
    pub fn extend(self) -> Self;
    pub fn halign(self, halign: Align) -> Self;
    pub fn selectable(self, selectable: bool) -> Self;
    pub fn sense(self, sense: Sense) -> Self;
    pub fn show_tooltip_when_elided(self, show: bool) -> Self;
}
```

#### Examples

```rust
ui.label("Simple label");

ui.add(egui::Label::new("Rich text").text_color(egui::Color32::RED));

ui.add(egui::Label::new("Truncated text").truncate());
```

#### Type References

- `WidgetText` - Text with optional formatting
- `Align` - `Min`, `Center`, `Max`
- `TextWrapMode` - Wrapping behavior

#### Return Values

- Returns `Response`. Use `.hovered()`, `.clicked()` (if `sense` is set).

#### Side effects

- Allocates space in the current layout.
- Adds text shapes to the painter.

#### References

- Source: `crates/egui/src/widgets/label.rs`

---

### `egui::Slider`

#### Overview

A draggable slider for numeric values. Supports integers and floats with configurable range, step, orientation, and display formatting.

#### Prerequisites & Requirements

- The value type MUST implement `emath::Numeric` (implemented for `f32`, `f64`, `i32`, `u32`, `usize`, etc.).
- Range MUST be a valid `RangeInclusive`.

#### Syntax / Method Signature

```rust
impl<'a> Slider<'a> {
    pub fn new<Num: Numeric>(value: &'a mut Num, range: RangeInclusive<Num>) -> Self;
    pub fn from_get_set(range: RangeInclusive<f64>, get_set: impl FnMut(Option<f64>) -> f64) -> Self;

    pub fn clamping(self, clamping: SliderClamping) -> Self;
    pub fn show_value(self, show_value: bool) -> Self;
    pub fn orientation(self, orientation: SliderOrientation) -> Self;
    pub fn text(self, text: impl Into<WidgetText>) -> Self;
    pub fn prefix(self, prefix: impl ToString) -> Self;
    pub fn suffix(self, suffix: impl ToString) -> Self;
    pub fn step(self, step: f64) -> Self;
    pub fn drag_value_speed(self, speed: f64) -> Self;
    pub fn min_decimals(self, min_decimals: usize) -> Self;
    pub fn max_decimals(self, max_decimals: usize) -> Self;
    pub fn integer(self) -> Self;
    pub fn logarithmic(self, logarithmic: bool) -> Self;
    pub fn smart_aim(self, smart_aim: bool) -> Self;
    pub fn trailing_fill(self, trailing_fill: bool) -> Self;
    pub fn handle_shape(self, handle_shape: HandleShape) -> Self;
    pub fn custom_formatter(self, formatter: impl Fn(&NumericFormat, f64) -> String) -> Self;
    pub fn custom_parser(self, parser: impl Fn(&NumericFormat, &str) -> Option<f64>) -> Self;
}

pub enum SliderClamping { Never, Edits, Always }
pub enum SliderOrientation { Horizontal, Vertical }
pub enum HandleShape { Circle, Rect }
```

#### Examples

```rust
let mut value = 0.5;
ui.add(egui::Slider::new(&mut value, 0.0..=1.0).text("opacity"));

let mut count = 0i32;
ui.add(egui::Slider::new(&mut count, 0..=100).integer().text("count"));

// Vertical slider
ui.add(egui::Slider::new(&mut value, 0.0..=1.0)
    .orientation(egui::SliderOrientation::Vertical));
```

#### Type References

- `Numeric` - Trait for slider-compatible types
- `SliderClamping` - When to clamp the value
- `SliderOrientation` - `Horizontal` or `Vertical`
- `HandleShape` - `Circle` or `Rect`

#### Return Values

- Returns `Response`. The value is modified in-place through the `&mut` reference.
- Use `.changed()` to detect user modification.

#### Side effects

- Mutates the bound value when the user drags.
- Allocates space in the current layout.

#### References

- Source: `crates/egui/src/widgets/slider.rs`

---

### `egui::TextEdit`

#### Overview

A text editing field supporting single-line, multi-line, and code editor modes. Handles keyboard input, clipboard, undo, and IME.

#### Prerequisites & Requirements

- The text source MUST implement `TextBuffer` (implemented for `String`, `SmartString`, etc.).
- For `code_editor()`, monospace font MUST be configured.

#### Syntax / Method Signature

```rust
impl<'t, 'a> TextEdit<'t, 'a> {
    pub fn singleline(text: &'t mut impl TextBuffer) -> Self;
    pub fn multiline(text: &'t mut impl TextBuffer) -> Self;

    pub fn id(self, id: Id) -> Self;
    pub fn font(self, font: TextStyle) -> Self;
    pub fn text_color(self, color: Color32) -> Self;
    pub fn text_style(self, text_style: TextStyle) -> Self;
    pub fn wrap_mode(self, wrap_mode: TextWrapMode) -> Self;
    pub fn margin(self, margin: Margin) -> Self;
    pub fn min_size(self, min_size: impl Into<Vec2>) -> Self;
    pub fn desired_width(self, width: f32) -> Self;
    pub fn lock_focus(self, lock: bool) -> Self;
    pub fn return_key(self, action: ReturnKeyAction) -> Self;
    pub fn code_editor(self) -> Self;
    pub fn password(self, password: bool) -> Self;
    pub fn hint_text(self, hint: impl Into<WidgetText>) -> Self;
    pub fn interactive(self, interactive: bool) -> Self;
    pub fn frame(self, frame: bool) -> Self;
}

pub trait TextBuffer {
    fn is_empty(&self) -> bool;
    fn as_str(&self) -> &str;
    fn insert_text(&mut self, text: &str, char_range: CharRange) -> CharRange;
    fn delete_char_range(&mut self, char_range: CharRange);
    fn clear(&mut self);
    fn take(&mut self) -> String;
}
```

#### Examples

```rust
let mut text = String::new();

// Single line
ui.text_edit_singleline(&mut text);

// Multi-line with wrapping
ui.add(egui::TextEdit::multiline(&mut text)
    .desired_width(300.0));

// Code editor
ui.add(egui::TextEdit::multiline(&mut text)
    .code_editor()
    .font(egui::TextStyle::Monospace));

// Password field
ui.add(egui::TextEdit::singleline(&mut password)
    .password(true)
    .hint_text("Enter password"));
```

#### Type References

- `TextBuffer` - Trait for text storage
- `TextStyle` - `Heading`, `Body`, `Monospace`, `Small`, `Button`
- `ReturnKeyAction` - Action on pressing Return: `NewLine` or `Submit`
- `Margin` - Per-side spacing (i8 values)

#### Return Values

- Returns `Response`. Use `.changed()` to detect text changes, `.has_focus()` for focus state.

#### Side effects

- Mutates the bound text when the user types.
- Manages focus, cursor, and selection state in `Context::memory()`.

#### References

- Source: `crates/egui/src/widgets/text_edit/`

---

### `egui::Image`

#### Overview

Displays an image from a URI, texture handle, or raw bytes. Supports fit modes, tinting, rotation, and corner rounding.

#### Prerequisites & Requirements

- For URI-based images, image loaders MUST be registered (use `egui_extras::install_image_loaders()`).
- For texture-based images, the texture MUST be loaded via `Context::load_texture()`.

#### Syntax / Method Signature

```rust
impl<'a> Image<'a> {
    pub fn new(source: impl Into<ImageSource<'a>>) -> Self;
    pub fn from_uri(uri: impl Into<Cow<'a, str>>) -> Self;
    pub fn from_texture(texture: impl Into<SizedTexture>) -> Self;
    pub fn from_bytes(uri: impl Into<Cow<'a, str>>, bytes: impl Into<Cow<'a, [u8]>>) -> Self;

    pub fn texture_options(self, texture_options: TextureOptions) -> Self;
    pub fn max_width(self, max_width: f32) -> Self;
    pub fn max_height(self, max_height: f32) -> Self;
    pub fn max_size(self, max_size: impl Into<Vec2>) -> Self;
    pub fn maintain_aspect_ratio(self, maintain: bool) -> Self;
    pub fn fit(self, fit: ImageFit) -> Self;
    pub fn sense(self, sense: Sense) -> Self;
    pub fn corner_radius(self, corner_radius: impl Into<CornerRadius>) -> Self;
    pub fn tint(self, tint: impl Into<Color32>) -> Self;
    pub fn rotate(self, angle: f32, rotation_center: Option<Vec2>) -> Self;
    pub fn show_loading_spinner(self, show: bool) -> Self;
    pub fn alt_text(self, alt_text: impl Into<WidgetText>) -> Self;
    pub fn bg_fill(self, bg_fill: impl Into<Color32>) -> Self;
}

pub enum ImageSource<'a> {
    Uri(Cow<'a, str>),
    Texture(SizedTexture),
    Bytes { uri: Cow<'a, str>, bytes: Cow<'a, [u8]> },
}

pub enum ImageFit {
    Original,
    Fill,
    Contain,
    Cover,
    Exact(Vec2),
    ScaleDown,
}
```

#### Examples

```rust
// From URI
ui.image("https://example.com/image.png");

// From texture
let texture: egui::TextureHandle = ctx.load_texture("my_image", image_data, Default::default());
ui.add(egui::Image::from_texture(texture).max_width(200.0));

// With fit and rounding
ui.add(egui::Image::from_uri("photo.jpg")
    .fit(egui::ImageFit::Cover)
    .max_size([200.0, 200.0])
    .corner_radius(10.0)
    .sense(egui::Sense::click()));
```

#### Type References

- `ImageSource` - URI, texture, or bytes
- `ImageFit` - Sizing behavior
- `TextureOptions` - Filtering and wrapping
- `SizedTexture` - Texture ID with explicit size
- `TextureHandle` - RAII texture reference

#### Return Values

- Returns `Response`. Use `.clicked()`, `.hovered()` (if `sense` is set).

#### Side effects

- Loads and caches the image data.
- Allocates space in the current layout.

#### References

- Source: `crates/egui/src/widgets/image.rs`

---

### `egui::ScrollArea`

#### Overview

A scrollable region. Supports vertical, horizontal, or both-axis scrolling with optional scrollbar visibility control.

#### Prerequisites & Requirements

- MUST be used inside an existing `Ui`.
- Content inside ScrollArea MUST NOT be infinite (use virtualization for large datasets via `egui_extras::Table`).

#### Syntax / Method Signature

```rust
impl ScrollArea {
    pub fn vertical() -> Self;
    pub fn horizontal() -> Self;
    pub fn both() -> Self;
    pub fn auto_sized() -> Self;  // Only scroll when content overflows

    pub fn id_source(self, id_source: impl AsIdSalt) -> Self;
    pub fn id(self, id: Id) -> Self;
    pub fn max_width(self, max_width: f32) -> Self;
    pub fn max_height(self, max_height: f32) -> Self;
    pub fn min_scrolled_width(self, min_scrolled_width: f32) -> Self;
    pub fn min_scrolled_height(self, min_scrolled_height: f32) -> Self;
    pub fn scroll_offset(self, offset: Vec2) -> Self;
    pub fn stick_to_bottom(self, stick_to_bottom: bool) -> Self;
    pub fn stick_to_right(self, stick_to_right: bool) -> Self;
    pub fn enable_scrolling(self, enable: bool) -> Self;
    pub fn show_scrollbars(self, show: bool) -> Self;
    pub fn always_show_scrollbars(self, always: bool) -> Self;
    pub fn drag_to_scroll(self, drag_to_scroll: bool) -> Self;
    pub fn use_fixed_buffer(self, use_fixed_buffer: bool) -> Self;
    pub fn animated(self, animated: bool) -> Self;
    pub fn sense(self, sense: Sense) -> Self;
    pub fn enabled(self, enabled: bool) -> Self;
    pub fn classes(self, classes: impl IntoClasses) -> Self;

    pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
    pub fn show_viewport<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui, &Rect) -> R) -> InnerResponse<R>;
}
```

#### Examples

```rust
// Vertical scroll
egui::ScrollArea::vertical()
    .max_height(300.0)
    .show(ui, |ui| {
        for i in 0..1000 {
            ui.label(format!("Item {i}"));
        }
    });

// Auto-sized (scroll only when needed)
egui::ScrollArea::auto_sized()
    .show(ui, |ui| {
        ui.label("Content that may overflow");
    });

// Stick to bottom (chat-like)
egui::ScrollArea::vertical()
    .stick_to_bottom(true)
    .show(ui, |ui| {
        for msg in &messages {
            ui.label(msg);
        }
    });
```

#### Type References

- `InnerResponse` - Contains the inner value and scroll area response

#### Return Values

- `show()` returns `InnerResponse<R>`.

#### Side effects

- Manages scroll state (offset, stickiness) in `Context::memory()`.
- Clips child shapes to the scroll region.

#### References

- Source: `crates/egui/src/containers/scroll_area.rs`

---

### `egui::Frame`

#### Overview

A decorative container with fill, stroke, corner rounding, and shadow. Can wrap child UI content.

#### Prerequisites & Requirements

- MUST be used inside an existing `Ui`.

#### Syntax / Method Signature

```rust
impl Frame {
    pub fn none() -> Self;
    pub fn canvas(style: &Style) -> Self;   // Default canvas style
    pub fn group(style: &Style) -> Self;    // Group box style
    pub fn central_panel(style: &Style) -> Self;

    pub fn fill(self, fill: impl Into<Color32>) -> Self;
    pub fn stroke(self, stroke: impl Into<Stroke>) -> Self;
    pub fn corner_radius(self, corner_radius: impl Into<CornerRadius>) -> Self;
    pub fn shadow(self, shadow: Shadow) -> Self;
    pub fn inner_margin(self, margin: impl Into<Margin>) -> Self;
    pub fn outer_margin(self, margin: impl Into<Margin>) -> Self;
    pub fn margin(self, margin: impl Into<Margin>) -> Self;
    pub fn min_size(self, min_size: impl Into<Vec2>) -> Self;
    pub fn enabled(self, enabled: bool) -> Self;
    pub fn sense(self, sense: Sense) -> Self;
    pub fn classes(self, classes: impl IntoClasses) -> Self;

    pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
    pub fn begin(self, ui: &mut Ui) -> FrameState;
    pub fn end(state: FrameState, ui: &mut Ui);
}
```

#### Examples

```rust
egui::Frame::group(ui.style())
    .fill(egui::Color32::DARK_GRAY)
    .corner_radius(5.0)
    .show(ui, |ui| {
        ui.label("Inside a group frame");
    });
```

#### Type References

- `Margin` - Per-side spacing
- `Stroke` - Line width and color
- `Shadow` - Box shadow (offset, blur, spread, color)
- `FrameState` - Intermediate state for `begin`/`end` pattern

#### Return Values

- `show()` returns `InnerResponse<R>`.

#### Side effects

- Paints the frame background, stroke, and shadow.
- Creates a child `Ui` with reduced available space.

#### References

- Source: `crates/egui/src/containers/frame.rs`

---

### `egui::CollapsingHeader`

#### Overview

A clickable header that expands/collapses to show or hide child content.

#### Prerequisites & Requirements

- MUST be used inside an existing `Ui`.

#### Syntax / Method Signature

```rust
impl CollapsingHeader {
    pub fn new(heading: impl Into<WidgetText>) -> Self;

    pub fn id_source(self, id_source: impl AsIdSalt) -> Self;
    pub fn id(self, id: Id) -> Self;
    pub fn default_open(self, default_open: bool) -> Self;
    pub fn open(self, open: Option<&mut bool>) -> Self;
    pub fn enabled(self, enabled: bool) -> Self;
    pub fn sense(self, sense: Sense) -> Self;
    pub fn selectable(self, selectable: bool) -> Self;
    pub fn selected(self, selected: bool) -> Self;
    pub fn show_tooltip(self, show: bool) -> Self;
    pub fn icon(self, icon: Option<CollapsingIcon>) -> Self;
    pub fn classes(self, classes: impl IntoClasses) -> Self;

    pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> CollapsingResponse<R>;
}
```

#### Examples

```rust
egui::CollapsingHeader::new("Advanced Settings")
    .default_open(false)
    .show(ui, |ui| {
        ui.label("These are the advanced settings.");
        ui.checkbox(&mut verbose, "Verbose mode");
    });
```

#### Type References

- `CollapsingResponse<R>` - Contains `header_response`, `body_response`, and `open` state
- `CollapsingIcon` - `TriangleDown`, `TriangleRight`, or `None`

#### Return Values

- `show()` returns `CollapsingResponse<R>`.

#### Side effects

- Toggles open/closed state on click.

#### References

- Source: `crates/egui/src/containers/collapsing_header.rs`

---

### `egui::ComboBox`

#### Overview

A drop-down selection widget. Shows a label and a selected value; clicking opens a popup list of options.

#### Prerequisites & Requirements

- MUST be provided with a unique `Id` or `id_source`.

#### Syntax / Method Signature

```rust
impl ComboBox {
    pub fn new(id_source: impl AsIdSalt, label: impl Into<WidgetText>) -> Self;

    pub fn id(self, id: Id) -> Self;
    pub fn selected_text(self, selected_text: impl Into<WidgetText>) -> Self;
    pub fn width(self, width: f32) -> Self;
    pub fn wrap_mode(self, wrap_mode: TextWrapMode) -> Self;
    pub fn icon(self, icon: Option<CollapsingIcon>) -> Self;
    pub fn enabled(self, enabled: bool) -> Self;
    pub fn classes(self, classes: impl IntoClasses) -> Self;

    pub fn show_ui(self, ui: &mut Ui, add_options: impl FnOnce(&mut Ui)) -> Response;
}
```

#### Examples

```rust
egui::ComboBox::from_id_source("language")
    .selected_text(selected_language)
    .show_ui(ui, |ui| {
        ui.selectable_value(&mut language, "Rust", "Rust");
        ui.selectable_value(&mut language, "Python", "Python");
        ui.selectable_value(&mut language, "JS", "JavaScript");
    });
```

#### Return Values

- Returns `Response`.

#### Side effects

- Opens a popup area below the combo box on click.

#### References

- Source: `crates/egui/src/containers/combo_box.rs`

---

### `egui::Area`

#### Overview

A free-floating area positioned at absolute coordinates. Unlike `Window`, it has no title bar or frame. Used for popups, tooltips, and custom overlays.

#### Prerequisites & Requirements

- MUST be shown with a `&Context` (not a `&Ui`).

#### Syntax / Method Signature

```rust
impl Area {
    pub fn new(id: impl AsIdSalt) -> Self;

    pub fn id(self, id: Id) -> Self;
    pub fn fixed_pos(self, pos: Pos2) -> Self;
    pub fn anchor(self, anchor: Align2, offset: Vec2) -> Self;
    pub fn movable(self, movable: bool) -> Self;
    pub fn resizable(self, resizable: bool) -> Self;
    pub fn enabled(self, enabled: bool) -> Self;
    pub fn sense(self, sense: Sense) -> Self;
    pub fn order(self, order: Order) -> Self;
    pub fn default_width(self, width: f32) -> Self;
    pub fn min_width(self, min_width: f32) -> Self;
    pub fn max_width(self, max_width: f32) -> Self;
    pub fn interactable(self, interactable: bool) -> Self;
    pub fn fade_in(self, fading: bool) -> Self;
    pub fn classes(self, classes: impl IntoClasses) -> Self;

    pub fn show<R>(self, ctx: &Context, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
}
```

#### Examples

```rust
egui::Area::new("tooltip_area")
    .anchor(egui::Align2::LEFT_TOP, [10.0, 10.0])
    .show(ctx, |ui| {
        ui.label("This floats at (10, 10)");
    });
```

#### Return Values

- Returns `InnerResponse<R>`.

#### Side effects

- Creates a new layer for rendering above other content.

#### References

- Source: `crates/egui/src/containers/area.rs`

---

### `egui::Panel` (CentralPanel, TopBottomPanel, LeftRightPanel)

#### Overview

Fixed-position panels that anchor to the sides or center of the screen. CentralPanel fills remaining space after side panels.

#### Prerequisites & Requirements

- Each panel type MUST be shown at most once per frame.
- TopBottomPanel and LeftRightPanel MUST be shown before CentralPanel.

#### Syntax / Method Signature

```rust
impl CentralPanel {
    pub fn default() -> Self;
    pub fn show(self, ctx: &Context, add_contents: impl FnOnce(&mut Ui));
}

impl TopBottomPanel {
    pub fn new(side: TopBottomSide) -> Self;  // Top or Bottom
    pub fn show(self, ctx: &Context, add_contents: impl FnOnce(&mut Ui));
}

impl LeftRightPanel {
    pub fn new(side: LeftRightSide) -> Self;  // Left or Right
    pub fn show(self, ctx: &Context, add_contents: impl FnOnce(&mut Ui));
}
```

#### Examples

```rust
egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
    ui.horizontal(|ui| {
        if ui.button("New").clicked() { /* ... */ }
        if ui.button("Open").clicked() { /* ... */ }
    });
});

egui::CentralPanel::default().show(ctx, |ui| {
    ui.label("Main content area");
});
```

#### Type References

- `TopBottomSide` - `Top`, `Bottom`
- `LeftRightSide` - `Left`, `Right`

#### Return Values

- Panels return no value (`()`).

#### Side effects

- Reserves space on the screen edges, reducing available space for other panels.

#### References

- Source: `crates/egui/src/containers/panel.rs`

---

### `egui::Grid`

#### Overview

A simple grid layout. Cells are laid out left to right, top-down. Each cell content is aligned to the left and center vertically. Column widths and row heights auto-size based on content and are remembered across frames.

#### Prerequisites & Requirements

- MUST be called inside an existing `Ui`.
- Call `ui.end_row()` to advance to the next row.
- To place multiple widgets in one cell, group them with `ui.horizontal()` or `ui.vertical()`.

#### Syntax / Method Signature

```rust
impl Grid {
    pub fn new(id_salt: impl AsIdSalt) -> Self;

    pub fn num_columns(self, num_columns: usize) -> Self;
    pub fn min_col_width(self, min_col_width: f32) -> Self;
    pub fn min_row_height(self, min_row_height: f32) -> Self;
    pub fn max_col_width(self, max_col_width: f32) -> Self;
    pub fn spacing(self, spacing: impl Into<Vec2>) -> Self;
    pub fn start_row(self, start_row: usize) -> Self;
    pub fn striped(self, striped: bool) -> Self;
    pub fn with_row_color<F>(self, color_picker: F) -> Self
    where
        F: Send + Sync + Fn(usize, &Style) -> Option<Color32> + 'static;

    pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
}
```

#### Examples

```rust
egui::Grid::new("my_grid")
    .num_columns(3)
    .striped(true)
    .min_col_width(100.0)
    .show(ui, |ui| {
        ui.label("First row, first column");
        ui.label("First row, second column");
        ui.end_row();

        ui.label("Second row, first column");
        ui.label("Second row, second column");
        ui.label("Second row, third column");
        ui.end_row();

        // Multiple widgets in one cell:
        ui.horizontal(|ui| {
            ui.label("Same");
            ui.label("cell");
        });
        ui.label("Next column");
        ui.end_row();
    });
```

#### Key Behaviors

- Column widths are auto-sized to the widest content in that column.
- Row heights are auto-sized to the tallest content in that row.
- Sizes are remembered across frames, so the grid does not flicker.
- The first frame is rendered invisibly (sizing pass) to measure content.
- Use `num_columns()` to let the last column fill remaining space.
- Use `max_col_width()` to enable text wrapping within cells.
- Use `striped(true)` for alternating row background colors.
- Use `start_row()` with `ScrollArea::show_rows()` for virtualized grids.

#### Type References

- `IdSalt` - Unique identifier source
- `InnerResponse` - Contains the inner value and grid area response

#### Return Values

- `show()` returns `InnerResponse<R>`.

#### Side Effects

- Stores column/row size state in `Context::data()`.
- May call `request_discard` on the first frame for a sizing pass.

#### References

- Source: `crates/egui/src/grid.rs`

---

### `egui::Response`

#### Overview

Returned by every widget. Provides interaction state and supports tooltip chaining.

#### Prerequisites & Requirements

- Created by `Ui::add()` or `Ui::allocate_response()`.
- Interaction queries rely on comparing current frame input against previous frame widget rects.

#### Syntax / Method Signature

```rust
impl Response {
    // Fields
    pub ctx: Context;
    pub layer_id: LayerId;
    pub id: Id;
    pub rect: Rect;
    pub interact_rect: Rect;
    pub sense: Sense;

    // Interaction queries
    pub fn clicked(&self) -> bool;
    pub fn clicked_by(&self, button: PointerButton) -> bool;
    pub fn secondary_clicked(&self) -> bool;
    pub fn middle_clicked(&self) -> bool;
    pub fn double_clicked(&self) -> bool;
    pub fn triple_clicked(&self) -> bool;
    pub fn drag_started(&self) -> bool;
    pub fn dragged(&self) -> bool;
    pub fn drag_stopped(&self) -> bool;
    pub fn hovered(&self) -> bool;
    pub fn contains_pointer(&self) -> bool;
    pub fn changed(&self) -> bool;
    pub fn has_focus(&self) -> bool;
    pub fn gained_focus(&self) -> bool;
    pub fn lost_focus(&self) -> bool;

    // Drag info
    pub fn drag_delta(&self) -> Vec2;
    pub fn total_drag_delta(&self) -> Option<Vec2>;

    // Chaining
    pub fn on_hover_text(self, text: impl Into<WidgetText>) -> Self;
    pub fn on_hover_ui(self, add_contents: impl FnOnce(&mut Ui)) -> Self;
    pub fn on_disabled_hover_text(self, text: impl Into<WidgetText>) -> Self;
    pub fn on_hover_cursor(self, cursor: CursorIcon) -> Self;
    pub fn on_hover_and_drag_cursor(self, cursor: CursorIcon) -> Self;
    pub fn highlight(self) -> Self;
    pub fn scroll_to_me(self, align: Option<Align>) -> Self;
    pub fn context_menu(self, add_contents: impl FnOnce(&mut Ui)) -> Option<InnerResponse<()>>;
    pub fn request_focus(&self);
    pub fn surrender_focus(&self);
}
```

#### Examples

```rust
let response = ui.button("Click");
if response.clicked() { /* ... */ }

// Chaining tooltip
ui.button("Hover me")
    .on_hover_text("This is a tooltip")
    .on_hover_cursor(egui::CursorIcon::PointingHand);
```

#### Return Values

- Interaction methods return `bool`.
- Chaining methods return `Self` for further chaining.

#### References

- Source: `crates/egui/src/response.rs`

---

### `egui::Painter`

#### Overview

Allows custom painting of shapes on a layer. Obtained from `Ui::allocate_painter()`, `Ui::painter()`, or `Context::debug_painter()`.

#### Prerequisites & Requirements

- MUST be obtained from a valid `Ui` or `Context`.

#### Syntax / Method Signature

```rust
impl Painter {
    pub fn line_segment(&self, points: [Pos2; 2], stroke: impl Into<Stroke>);
    pub fn circle(&self, center: Pos2, radius: f32, fill: impl Into<Color32>, stroke: impl Into<Stroke>);
    pub fn circle_filled(&self, center: Pos2, radius: f32, fill: impl Into<Color32>);
    pub fn rect(&self, rect: Rect, corner_radius: impl Into<CornerRadius>, fill: impl Into<Color32>, stroke: impl Into<Stroke>);
    pub fn rect_filled(&self, rect: Rect, corner_radius: impl Into<CornerRadius>, fill: impl Into<Color32>);
    pub fn text(&self, pos: Pos2, anchor: Align2, text: impl Into<WidgetText>, font_id: FontId, color: Color32);
    pub fn image(&self, texture_id: TextureId, rect: Rect, uv: Rect, tint: Color32);
    pub fn add(&self, shape: Shape);
    pub fn add_with_clip_rect(&self, shape: Shape, clip_rect: Rect);
    pub fn set(&self, shapes: Vec<Shape>);
    pub fn extend(&self, shapes: Vec<Shape>);
}
```

#### Examples

```rust
let (response, painter) = ui.allocate_painter([200.0, 100.0], Sense::hover());

// Draw a filled circle
painter.circle_filled(response.rect.center(), 30.0, egui::Color32::RED);

// Draw text
painter.text(response.rect.center(), Align2::CENTER_CENTER, "Custom", FontId::proportional(20.0), Color32::WHITE);
```

#### Type References

- `Shape` - Enum of paintable shapes
- `Align2` - Anchor point for text

#### References

- Source: `crates/egui/src/painter.rs`

---

### `emath` (2D Math Library)

#### Overview

Minimal 2D math types used throughout egui. Provides `Vec2`, `Pos2`, `Rect`, `Rangef`, `Align`, `Align2`, `Rot2`, `TSTransform`, and numeric utilities.

#### Syntax / Method Signature

```rust
// Vec2 - 2D vector
pub struct Vec2 { pub x: f32, pub y: f32 }
impl Vec2 {
    pub const ZERO: Self;
    pub const ONE: Self;
    pub const X: Self;
    pub const Y: Self;
    pub const RIGHT: Self;
    pub const LEFT: Self;
    pub const UP: Self;
    pub const DOWN: Self;
    pub fn new(x: f32, y: f32) -> Self;
    pub fn splat(v: f32) -> Self;
    pub fn length(&self) -> f32;
    pub fn length_sq(&self) -> f32;
    pub fn normalized(&self) -> Self;
    pub fn rot90(&self) -> Self;
    pub fn floor(&self) -> Self;
    pub fn ceil(&self) -> Self;
    pub fn round(&self) -> Self;
    pub fn min(&self, other: Self) -> Self;
    pub fn max(&self, other: Self) -> Self;
    pub fn clamp(&self, min: Self, max: Self) -> Self;
    pub fn to_pos2(&self) -> Pos2;
}

// Pos2 - 2D position
pub struct Pos2 { pub x: f32, pub y: f32 }
impl Pos2 {
    pub const ZERO: Self;
    pub fn new(x: f32, y: f32) -> Self;
    pub fn splat(v: f32) -> Self;
    pub fn distance(&self, other: Self) -> f32;
    pub fn distance_sq(&self, other: Self) -> f32;
    pub fn floor(&self) -> Self;
    pub fn ceil(&self) -> Self;
    pub fn round(&self) -> Self;
    pub fn min(&self, other: Self) -> Self;
    pub fn max(&self, other: Self) -> Self;
    pub fn to_vec2(&self) -> Vec2;
}

// Rect - Axis-aligned rectangle
pub struct Rect { pub min: Pos2, pub max: Pos2 }
impl Rect {
    pub const NOTHING: Self;
    pub const EVERYTHING: Self;
    pub fn from_min_size(min: Pos2, size: Vec2) -> Self;
    pub fn from_min_max(min: Pos2, max: Pos2) -> Self;
    pub fn from_center_size(center: Pos2, size: Vec2) -> Self;
    pub fn from_two_pos(a: Pos2, b: Pos2) -> Self;
    pub fn from_x_y_ranges(x_range: Rangef, y_range: Rangef) -> Self;
    pub fn nothing() -> Self;
    pub fn everything() -> Self;
    pub fn center(&self) -> Pos2;
    pub fn size(&self) -> Vec2;
    pub fn width(&self) -> f32;
    pub fn height(&self) -> f32;
    pub fn area(&self) -> f32;
    pub fn intersects(&self, other: Self) -> bool;
    pub fn contains(&self, pos: Pos2) -> bool;
    pub fn contains_rect(&self, other: Self) -> bool;
    pub fn expand(&self, amount: f32) -> Self;
    pub fn translate(&self, delta: Vec2) -> Self;
    pub fn union(&self, other: Self) -> Self;
    pub fn intersect(&self, other: Self) -> Self;
    pub fn shrink(&self, amount: f32) -> Self;
    pub fn clamp(&self, pos: Pos2) -> Pos2;
    pub fn lerp(&self, t: Vec2) -> Pos2;
}

// Align
pub enum Align { Min, Center, Max }
pub enum Align2 { LEFT_TOP, LEFT_CENTER, LEFT_BOTTOM, CENTER_TOP, CENTER_CENTER, CENTER_BOTTOM, RIGHT_TOP, RIGHT_CENTER, RIGHT_BOTTOM }

// Rangef
pub struct Rangef { pub min: f32, pub max: f32 }

// Utility functions
pub fn lerp<T: Lerp>(from: T, to: T, t: f32) -> T;
pub fn remap(x: f32, from: Rangef, to: Rangef) -> f32;
pub fn remap_clamp(x: f32, from: Rangef, to: Rangef) -> f32;
```

#### Examples

```rust
use emath::*;

let pos = pos2(10.0, 20.0);
let vec = vec2(5.0, -3.0);
let rect = Rect::from_min_size(pos, vec2(100.0, 50.0));
let center = rect.center();
let t = remap_clamp(0.5, Rangef::new(0.0, 1.0), Rangef::new(0.0, 100.0));
```

#### References

- Source: `crates/emath/src/`

---

### `ecolor` (Color Types)

#### Overview

Color types with premultiplied alpha. Provides `Color32`, `Rgba`, `Hsva`, and `HsvaGamma`.

#### Syntax / Method Signature

```rust
// Color32 - sRGBA with premultiplied alpha
pub struct Color32(pub [u8; 4]);
impl Color32 {
    pub const TRANSPARENT: Self;
    pub const BLACK: Self;
    pub const WHITE: Self;
    pub const RED: Self;
    pub const GREEN: Self;
    pub const BLUE: Self;
    pub const YELLOW: Self;
    pub const ORANGE: Self;
    pub const DARK_RED: Self;
    pub const DARK_GREEN: Self;
    pub const DARK_BLUE: Self;
    pub const GOLD: Self;
    pub const PLACEHOLDER: Self; // #808080
    pub const DEBUG_COLOR: Self;
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self;
    pub fn from_rgba_premultiplied(r: u8, g: u8, b: u8, a: u8) -> Self;
    pub fn from_rgba_unmultiplied(r: u8, g: u8, b: u8, a: u8) -> Self;
    pub fn from_additive_luminance(l: u64) -> Self;
    pub fn from_hex(hex: &str) -> Result<Self, ParseColorError>;
    pub fn to_array(&self) -> [u8; 4];
    pub fn to_tuple(&self) -> (u8, u8, u8, u8);
    pub fn r(&self) -> u8;
    pub fn g(&self) -> u8;
    pub fn b(&self) -> u8;
    pub fn a(&self) -> u8;
    pub fn is_opaque(&self) -> bool;
    pub fn gamma_multiply(&self, gamma: f32) -> Self;
    pub fn linear_multiply(&self, alpha: f32) -> Self;
    pub fn to_opaque(self) -> Self;
    pub fn blend_onto(self, target: Self) -> Self;
}

// Rgba - Linear space RGBA (f32 components)
pub struct Rgba(pub [f32; 4]);

// Hsva
pub struct Hsva { pub h: f32, pub s: f32, pub v: f32, pub a: f32 }

// HsvaGamma - Like Hsva but with gamma-expanded v
pub struct HsvaGamma { pub h: f32, pub s: f32, pub v: f32, pub a: f32 }
```

#### Examples

```rust
let red = egui::Color32::RED;
let custom = egui::Color32::from_rgb(100, 200, 50);
let hex_color = egui::Color32::from_hex("#FF8800").unwrap();
let blended = red.blend_onto(egui::Color32::WHITE);
```

#### References

- Source: `crates/ecolor/src/`

---

### `epaint` (2D Shapes and Text)

#### Overview

2D graphics library providing shapes, text layout, tessellation, and texture management. Shapes are tessellated into textured triangles for rendering.

#### Key Types

```rust
// Shape enum
pub enum Shape {
    Noop,
    Vec(Vec<Self>),
    Circle(CircleShape),
    Ellipse(EllipseShape),
    LineSegment { points: [Pos2; 2], stroke: Stroke },
    Path(PathShape),
    Rect(RectShape),
    Text(TextShape),
    Mesh(Arc<Mesh>),
    QuadraticBezier(QuadraticBezierShape),
    CubicBezier(CubicBezierShape),
    Callback(PaintCallback),
}

// Stroke
pub struct Stroke { pub width: f32, pub color: Color32 }

// Mesh
pub struct Mesh {
    pub indices: Vec<u32>,
    pub vertices: Vec<Vertex>,
    pub texture_id: TextureId,
}

// Vertex
pub struct Vertex { pub pos: Pos2, pub uv: Pos2, pub color: Color32 }

// TextureId
pub enum TextureId { Managed(u64), User(u64) }

// TextureOptions
pub struct TextureOptions {
    pub magnification: TextureFilter,
    pub minification: TextureFilter,
    pub wrap_mode: TextureWrapMode,
    pub mipmap_mode: Option<TextureFilter>,
}

// FontId
pub struct FontId { pub size: f32, pub family: FontFamily }

// FontFamily
pub enum FontFamily { Proportional, Monospace, Name(Arc<str>) }

// Galley (laid-out text)
pub struct Galley {
    pub job: Arc<LayoutJob>,
    pub rows: Vec<PlacedRow>,
    pub elided: bool,
    pub rect: Rect,
    // ...
}
```

#### Examples

```rust
use epaint::*;

// Create a shape
let shape = Shape::circle_filled(pos2(100.0, 100.0), 50.0, Color32::RED);
let mesh = Mesh::with_texture(TextureId::default());

// Layout text
let galley = ctx.fonts(|f| {
    f.layout_job(LayoutJob::simple("Hello".into(), FontId::proportional(20.0), Color32::WHITE, 200.0))
});
```

#### References

- Source: `crates/epaint/src/`

---

### `eframe` (Application Framework)

#### Overview

Official egui framework for compiling the same app to web (Wasm) and native (desktop). Provides the `App` trait, window management, input handling, and rendering.

#### The `App` Trait

```rust
pub trait App {
    // REQUIRED - Called every frame
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut Frame);

    // OPTIONAL
    fn logic(&mut self, ctx: &egui::Context, frame: &mut Frame) {}
    fn save(&mut self, storage: &mut dyn Storage) {}
    fn on_exit(&mut self) {}
    fn auto_save_interval(&self) -> Duration { Duration::from_secs(30) }
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4];
    fn persist_egui_memory(&self) -> bool { true }
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {}
}
```

#### Running a Native App

```rust
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native("My App", options, Box::new(|cc| {
        Ok(Box::new(MyApp::new(cc)))
    }))
}
```

#### `NativeOptions`

```rust
pub struct NativeOptions {
    pub viewport: egui::ViewportBuilder,
    pub renderer: Renderer,          // Glow or Wgpu
    pub multisampling: u16,
    pub depth_buffer: u8,
    pub stencil_buffer: u8,
    pub centered: bool,
    pub persist_window: bool,
    pub dithering: bool,
    pub event_loop_builder: Option<EventLoopBuilderHook>,
    pub window_builder: Option<WindowBuilderHook>,
    // ...
}
```

#### `Frame`

```rust
impl Frame {
    pub fn is_web(&self) -> bool;
    pub fn storage(&self) -> Option<&dyn Storage>;
    pub fn storage_mut(&self) -> Option<&mut dyn Storage>;
    pub fn info(&self) -> &IntegrationInfo;
    pub fn winit_window(&self) -> Option<&Arc<Window>>;
    pub fn gl(&self) -> Option<&Arc<glow::Context>>;
    pub fn wgpu_render_state(&self) -> Option<&RenderState>;
}
```

#### `CreationContext`

```rust
pub struct CreationContext<'s> {
    pub egui_ctx: egui::Context,
    pub integration_info: IntegrationInfo,
    pub storage: Option<&'s dyn Storage>,
    pub gl: Option<Arc<glow::Context>>,
    pub wgpu_render_state: Option<egui_wgpu::RenderState>,
}
```

#### Web App

```rust
#[wasm_bindgen]
pub struct WebHandle { runner: eframe::WebRunner }

#[wasm_bindgen]
impl WebHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self { /* ... */ }

    pub async fn start(&self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
        self.runner.start(canvas, eframe::WebOptions::default(), Box::new(|cc| {
            Ok(Box::new(MyApp::new(cc)))
        })).await
    }
}
```

#### References

- Source: `crates/eframe/src/epi.rs`

---

### `egui_extras::Table`

#### Overview

A scrollable table widget with sticky headers, resizable columns, striped rows, and virtualized rendering for large datasets.

#### Prerequisites & Requirements

- The `egui_extras` crate MUST be added as a dependency.
- `TableBuilder` MUST specify at least one `Column`.

#### Syntax / Method Signature

```rust
impl TableBuilder {
    pub fn new(ui: &mut Ui) -> Self;
    pub fn columns(self, column: Column, count: usize) -> Self;
    pub fn column(self, column: Column) -> Self;
    pub fn resizable(self, resizable: bool) -> Self;
    pub fn cell_layout(self, layout: Layout) -> Self;
    pub fn min_scrolled_height(self, min_height: f32) -> Self;
    pub fn max_scroll_height(self, max_height: f32) -> Self;
    pub fn drag_to_scroll(self, drag_to_scroll: bool) -> Self;
    pub fn stick_to_bottom(self, stick_to_bottom: bool) -> Self;
    pub fn auto_shrink(self, auto_shrink: bool) -> Self;
    pub fn striped(self, striped: bool) -> Self;
    pub fn sense(self, sense: Sense) -> Self;
    pub fn vertical_scroll_offset(self, offset: f32) -> Self;
    pub fn scroll_to_row(self, row: usize, align: Option<Align>) -> Self;

    pub fn header(self, height: f32, setup: impl FnOnce(&mut TableRow)) -> Self;
    pub fn body(self, setup: impl FnOnce(&mut TableBody));
}

impl TableBody {
    pub fn rows(self, height: f32, num_rows: usize, setup: impl FnOnce(&mut TableRow, row_index: usize));
    pub fn heterogeneous_rows(self, num_rows: usize, setup: impl FnOnce(&mut TableRow, row_index: usize));
    pub fn set_row_height(&mut self, height: f32);
    pub fn max_height(self, max_height: f32) -> Self;
    pub fn min_height(self, min_height: f32) -> Self;
}

impl TableRow {
    pub fn col(|self|, add_contents: impl FnOnce(&mut Ui));
    pub fn set_row_height(&mut self, height: f32);
}

impl Column {
    pub fn auto() -> Self;
    pub fn exact(width: f32) -> Self;
    pub fn initial(width: f32) -> Self;
    pub fn remainder() -> Self;
    pub fn relative(fraction: f32) -> Self;
    pub fn at_least(self, min: f32) -> Self;
    pub fn at_most(self, max: f32) -> Self;
    pub fn resizable(self, resizable: bool) -> Self;
    pub fn clip(self, clip: bool) -> Self;
    pub fn range(self, range: Rangef) -> Self;
}
```

#### Examples

```rust
egui_extras::TableBuilder::new(ui)
    .striped(true)
    .resizable(true)
    .column(egui_extras::Column::auto())
    .column(egui_extras::Column::remainder())
    .column(egui_extras::Column::exact(80.0))
    .header(20.0, |mut header| {
        header.col(|ui| { ui.label("Name"); });
        header.col(|ui| { ui.label("Description"); });
        header.col(|ui| { ui.label("Actions"); });
    })
    .body(|body| {
        body.rows(20.0, items.len(), |mut row, i| {
            row.col(|ui| { ui.label(&items[i].name); });
            row.col(|ui| { ui.label(&items[i].desc); });
            row.col(|ui| { if ui.button("Edit").clicked() { /* ... */ } });
        });
    });
```

#### References

- Source: `crates/egui_extras/src/table.rs`

---

### `egui_extras::install_image_loaders`

#### Overview

Registers all enabled image loaders (file, HTTP, image crate, SVG, GIF, WebP) with an `egui::Context`.

#### Syntax / Method Signature

```rust
pub fn install_image_loaders(ctx: &egui::Context);
```

#### Features

| Feature | Loader | Supported Formats |
|---|---|---|
| `file` | FileLoader | `file://` URIs |
| `http` | EhttpLoader | `http://`, `https://` URIs |
| `image` | ImageCrateLoader | PNG, JPEG, BMP, etc. |
| `svg` | SvgLoader | `.svg` via resvg/tiny_skia |
| `gif` | GifLoader | Animated GIFs |
| `webp` | WebPLoader | Static and animated WebP |

#### Examples

```rust
// In app setup:
egui_extras::install_image_loaders(&cc.egui_ctx);

// Then use:
ui.image("https://example.com/image.png");
ui.image("file:///path/to/local/image.png");
ui.image("ferris.svg");
```

#### References

- Source: `crates/egui_extras/src/loaders.rs`

---

## Configuration Reference

### `egui::Style`

```rust
pub struct Style {
    pub spacing: Spacing,
    pub interaction: Interaction,
    pub visuals: Visuals,
    pub text_styles: BTreeMap<TextStyle, FontId>,
    pub wrap_mode: Option<TextWrapMode>,
    pub override_font_id: Option<FontId>,
    pub animation_time: f32,
    pub explanation: String,
}
```

### `egui::Visuals`

```rust
pub struct Visuals {
    pub dark: bool,
    pub override_text_color: Option<Color32>,
    pub window_rounding: f32,
    pub window_shadow: Shadow,
    pub window_fill: Color32,
    pub window_stroke: Stroke,
    pub window_highlight_topmost: bool,
    pub panel_fill: Color32,
    pub faint_bg_color: Color32,
    pub extreme_bg_color: Color32,
    pub code_bg_color: Color32,
    pub warn_fg_color: Color32,
    pub error_fg_color: Color32,
    pub hyperlink_color: Color32,
    pub selection: Selection,
    pub widgets: Widgets,
    pub collapsing_header_frame: bool,
    pub slider_trailing_fill: bool,
    pub image_loading_spinners: bool,
    pub clip_rect_margin: f32,
    pub striped: bool,
    pub margin: Margin,
    pub indent_guides: bool,
    pub buttons: ButtonVisuals,
}
```

### `egui::Spacing`

```rust
pub struct Spacing {
    pub item_spacing: Vec2,
    pub indent: f32,
    pub indent_unit: f32,
    pub interact_size: Vec2,
    pub interact_size_fudge: Vec2,
    pub slider_width: f32,
    pub button_padding: Vec2,
    pub icon_width: f32,
    pub icon_width_inner: f32,
    pub icon_spacing: f32,
    pub tooltip_width: f32,
    pub combo_width: f32,
    pub text_edit_width: f32,
    pub indent_guides_indent_width: f32,
    pub scroll_bar_inner_margin: f32,
    pub scroll_bar_width: f32,
    pub scroll_bar_outer_margin: f32,
    pub item_spacing_resize: Vec2,
    pub item_spacing_scroll: Vec2,
    pub line_height: Option<f32>,
}
```

### `egui::FontDefinitions`

```rust
pub struct FontDefinitions {
    pub font_data: BTreeMap<String, Arc<FontData>>,
    pub families: BTreeMap<FontFamily, Vec<String>>,
}
```

### Theme Configuration

```rust
pub enum ThemePreference { Dark, Light, HighContrast, System }
pub enum Theme { Dark, Light, HighContrast }

// Usage:
ctx.set_theme(egui::ThemePreference::Dark);
ctx.set_visuals(egui::Visuals::light());
```

### Modifying Style at Runtime

```rust
// Per-frame
let mut style = (*ui.style()).clone();
style.spacing.item_spacing = egui::vec2(10.0, 5.0);
ui.set_style(style);

// Global
ctx.set_visuals(egui::Visuals::light());
ctx.set_fonts(my_font_definitions);
```

---

## Error Handling

### egui Core

egui avoids panics in normal operation. The core library uses fallible operations sparingly:

| Area | Behavior |
|---|---|
| Widget interaction | Returns `Response` with boolean flags; no errors |
| Image loading | Fails silently (shows nothing or spinner); error logged via `log` crate |
| Font loading | Missing glyphs render as tofu (`☐`) |
| Texture allocation | Managed internally; overflow causes atlas resize |
| ID clashes | Logged as warning; may cause visual glitches |

### eframe (Application Framework)

```rust
pub enum eframe::Error {
    AppCreation(Box<dyn Error + Send + Sync>),     // App creator returned error
    Winit(OsError),                                  // Window creation failed
    WinitEventLoop(EventLoopError),                  // Event loop failed
    Glutin(glutin::error::Error),                    // OpenGL context creation failed (glow)
    NoGlutinConfigs,                                 // No suitable OpenGL config found
    OpenGL(PainterError),                            // GL painter initialization failed
    Wgpu(WgpuError),                                 // WGPU initialization failed
}

pub type eframe::Result<T = (), E = Error> = std::result::Result<T, E>;
```

- `run_native()` returns `Result<(), Error>`.
- `WebRunner::start()` returns `Result<(), JsValue>` (Wasm).
- App creation errors propagate as `Error::AppCreation`.
- Backend-specific errors (OpenGL, WGPU, window) are surfaced as specific variants.
- Persistence failures are logged but do not crash.

### General Pattern

- Use `Response::changed()` to detect user modifications.
- Use `Response::clicked()` / `hovered()` to check interaction success.
- Invalid state (e.g., out-of-range slider values) is clamped silently.
- Invalid widget sizes or negative values are handled gracefully (clamped to zero or minimum).
- Panics only occur on programmer error (e.g., double-locking `Context`, using `Ui` after its container is dropped).