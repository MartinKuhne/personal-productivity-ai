# Markdown Subsystem Architecture

This document describes the architecture of the Markdown rendering pipeline, with a specific focus on the Table Rendering Pipeline.

## Table Rendering Pipeline

The table layout pipeline is designed to be fully modular, decoupling pure layout math and text measurement from the `egui` UI context. This ensures that the layout logic is 100% testable without spinning up a UI framework, and the UI painter logic remains as simple as possible.

The pipeline follows a distinct 4-stage flow:

### 1. AST (Semantic Table)
The standard Markdown parsing pipeline (`pulldown-cmark`) generates a `RenderEvent::Table(Vec<Vec<Vec<InlineElem>>>)`. 
This event represents the raw parsed data. It is transformed into a domain-specific `SemanticTable` object that represents the table structure (headers, rows, cells).

### 2. Text Measurement (Dependency Injection)
A `TextMeasurer` trait defines the interface for measuring text widths:

```rust
pub trait TextMeasurer {
    fn measure_text(&self, elems: &[InlineElem]) -> f32;
}
```

By abstracting this operation behind a trait, we can inject a mock/dummy measurer for unit testing (e.g., assuming 1 char = 10 pixels), while the production environment injects an `egui`-backed measurer that calculates precise pixel widths using the loaded fonts.

### 3. Table Layout Builder
The `TableLayoutBuilder` takes the `SemanticTable`, the injected `TextMeasurer`, and the available `max_width`.
It computes the `min_content` and `max_content` constraints and token breakpoints for each column using the `TextMeasurer`.
Then, it invokes the Fair Table Width Algorithm (FTWA) to assign an exact pixel width to each column, accounting for wrapping, overflow, and horizontal scrolling.

### 4. Table Layout
The builder outputs a resolved `TableLayout` data structure.
```rust
pub struct TableLayout {
    pub cells: Vec<LayoutCell>,
    pub total_width: f32,
    pub needs_horizontal_scroll: bool,
    // ...
}

pub struct LayoutCell {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub content: Vec<InlineElem>,
}
```
This structure contains the exact pre-calculated `x, y, width, height` boundaries for every cell, as well as row/column boundaries.

### 5. UI Painter (`egui`)
The final stage happens in the `ui` module. The `egui` painter consumes the `TableLayout`. Because all complex logic, wrapping calculations, and size constraints have already been resolved, the painter only needs to execute simple drawing commands: setting up the scroll area (if required), drawing background rectangles for cells, drawing borders, and rendering the text into the pre-defined bounding boxes.
