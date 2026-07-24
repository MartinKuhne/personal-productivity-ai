# Research: Virtual Scrolling for the Directory Tree

## Problem

`draw_tree_node()` recursively renders all visible nodes every frame. For a directory with 1000+ files, this creates thousands of egui widgets per frame. egui is an immediate-mode GUI — every frame, every visible widget is laid out, painted, and interaction-tested from scratch. With no virtualization, scrolling latency increases linearly with file count.

**Measured profile per frame:**
- `draw_tree_node()` traverses the full tree depth-first
- Each node creates at minimum: 1 `selectable_label` (with layout + painting), 1 potential `indent`, plus context menu setup
- For 10,000 files: ~10,000 label widgets created every frame, even those far outside the viewport
- On lower-end hardware this causes dropped frames and visible input lag during scrolling

## Approaches

### Approach A: Aggressive Default Collapse

**Idea:** Fold all subtrees by default. Only render what's expanded.

**Status:** Already partially implemented — `expanded_dirs` starts empty in `SelectionManager::new()`. Root-level library nodes appear collapsed.

**Limitations:**
- Does not help once a large directory is expanded (user clicks to expand a dir with 5000 files — same problem)
- Not true virtualization, just deferred rendering

**Verdict:** Keep as-is; insufficient for large expanded directories.

---

### Approach B: Viewport-Only Rendering with Fixed-Height Items

**Idea:** Approximate each tree row as a fixed height (e.g. 22px). Use the `ScrollArea` viewport to compute which rows are visible. Only call `draw_tree_node()` for visible rows.

**Algorithm:**
```
1. Flatten tree into ordered Vec<&TreeNode> (DFS pre-order)
2. Compute row count → total content height = N * ROW_HEIGHT
3. ScrollArea knows viewport offset + height
4. visible_range = (offset / ROW_HEIGHT) .. ((offset + viewport_height) / ROW_HEIGHT)
5. Only render tree[visible_range]
```

**Pros:**
- Scales to any number of files — only ~50 items rendered per frame regardless of total count
- Predictable performance

**Cons:**
- Variable expand/collapse changes row count every frame → scroll position jumps if not carefully tracked
- Indentation and icons vary by depth → fixed row height wastes space for shallow nodes or clips deep ones
- Requires flattening the tree (O(N)) on every change anyway — but not per frame
- egui `ScrollArea` has no built-in virtualization hooks; custom implementation needed
- Context menus, multi-select, drag operations must handle partially-visible items

**egui-specific considerations:**
- Use `egui::ScrollArea::show_viewport()` which provides `&egui::Rect` of the visible region
- Render only within that rect, using `ui.allocate_space()` for off-screen items
- Need to track `scroll_offset` and item positions manually

**Verdict:** High effort, fragile with variable-height items. Best suited for uniform lists.

---

### Approach C: Lazy Rendering with Measured Item Heights

**Idea:** Measure each tree item's actual height once (on first render), cache it. Use cached heights for viewport calculation on subsequent frames.

**Algorithm:**
```
type ItemId = Vec<usize>; // path in tree: [lib_index, child_index, ...]

struct LayoutCache {
    heights: HashMap<ItemId, f32>,
    total_height: f32,
    items: Vec<(ItemId, f32, f32)>, // (id, y_offset, height) sorted
}
```

1. On first render (or after tree mutation), traverse tree and measure each item
2. Cache item height and cumulative y-offset
3. On subsequent frames, use cached layout to skip non-visible items
4. Use `ui.put()` to place widgets at their correct y-position within the scroll area

**Pros:**
- Handles variable-height items (different indentation, bold filenames, etc.)
- Only measures once per item
- True virtualization — off-screen items don't generate widgets

**Cons:**
- Complex invalidation logic: file add/remove, rename, expand/collapse all invalidate layout
- Cache size = O(N) memory
- Variable height means no O(1) offset calculation — needs binary search in layout cache
- Implementing `ui.put()` with absolute positions inside egui's layout is error-prone

**Verdict:** Correct but complex. Worth it only if profiling shows Approach D is insufficient.

---

### Approach D: Rate-Limited Rendering with Dirty Flag (Hybrid)

**Idea:** Keep full tree rendering but only rebuild and re-render at a throttled rate. Between renders, the `ScrollArea` is frozen — user input is still processed but visual updates are batched.

**Implementation:**
```rust
struct TreeRenderState {
    last_render_time: Instant,
    render_interval: Duration, // e.g. 16ms (60fps target) or 33ms (30fps)
    frame_count: u32,
}
```

In `show_left_panel()`:
```rust
let now = Instant::now();
let elapsed = now - app.tree_render.last_render_time;
if elapsed >= app.tree_render.render_interval {
    // Full render
    app.tree_render.last_render_time = now;
    draw_tree_node(...);
} else {
    // Skip rendering, use egui::Ui::allocate_space to reserve height
    ui.allocate_space(previous_content_rect.size());
}
```

**Pros:**
- Minimal code change — no flattening, no viewport math
- Works with any tree structure
- Still produces correct layout at throttled rate

**Cons:**
- Does not reduce widget count per render — just reduces render frequency
- Skipped frames may cause visual jitter if scroll position changes during skip
- User-perceptible delay if scroll happens between renders

**Verdict:** Simple but incomplete. OK as a quick win but not a full solution.

---

### Approach E: Flattened Virtual List with Expand/Collapse Inline

**Idea:** Replace the recursive tree with a flat Vec of visible rows. Each row knows its depth, icon, selection state, and path. Only visible rows are rendered. Expand/collapse inserts/removes rows from the visible list inline without rebuilding from scratch.

**Data structure:**
```rust
struct FlatRow {
    depth: usize,
    node_path: PathBuf,
    name: String,
    is_dir: bool,
    is_expanded: bool,
    is_selected: bool,
    y_offset: f32, // computed from ROW_HEIGHT * row_index
}
```

**Reconciliation on expand/collapse:**
- On expand: insert children rows after the expanded row (shift subsequent rows down)
- On collapse: remove children rows (shift subsequent rows up)
- On file add/remove: atomic insert/remove at correct position

**Virtual rendering:**
```rust
let row_height = 22.0;
let total_height = flat_rows.len() as f32 * row_height;
let viewport = scroll_area.viewport();
let first_visible = (viewport.top() / row_height) as usize;
let last_visible = (viewport.bottom() / row_height) as usize;

for i in first_visible..=last_visible.min(flat_rows.len() - 1) {
    let row = &flat_rows[i];
    let y = i as f32 * row_height;
    // Use ui.put() or allocate_space + vertical offset
    render_row(ui, row, y - viewport.top());
}
```

**Pros:**
- True O(visible) rendering — scales to any file count
- Flat structure enables simple index-based viewport computation
- Expand/collapse is O(children) instead of O(full tree)
- File add/remove is O(path depth) instead of O(all files)

**Cons:**
- Requires maintaining a persistent flat list alongside the tree
- Flat list must be updated on every structure change (new file, deleted dir, etc.)
- Context menus / multi-select / drag-drop must index into flat list
- ROW_HEIGHT fixed — varying indentation depth wastes space
- More code than approaches A–D

**egui integration details:**
```rust
egui::ScrollArea::vertical()
    .id_source("virtual_tree")
    .show_viewport(ui, |ui, viewport| {
        let row_height = 22.0;
        let first = (viewport.min.y / row_height) as usize;
        let last = ((viewport.max.y / row_height) as usize + 1).min(flat_rows.len());

        // Reserve space for the full content height
        ui.set_height(flat_rows.len() as f32 * row_height);

        for i in first..last {
            let row = &flat_rows[i];
            let rect = egui::Rect::from_min_size(
                egui::pos2(0.0, i as f32 * row_height),
                egui::vec2(ui.available_width(), row_height),
            );
            let response = ui.allocate_rect(rect, egui::Sense::click());
            ui.put(rect, |ui: &mut egui::Ui| {
                // indent, icon, label
            });
        }
    });
```

**Verdict:** Most robust solution. Recommended for implementation if profiling confirms the tree is a bottleneck.

---

### Approach F: Hybrid — Sparse Rendering via ui.skip()

**Idea:** Use egui's `ui.skip()` mechanism to reserve space for non-visible items without creating widgets. Only render items in the viewport.

**Note:** egui does not have a built-in `ui.skip()` for jumping through a list. This would need to be emulated with `ui.allocate_space()`.

**Implementation sketch:**
```rust
fn draw_tree_virtual(ui: &mut egui::Ui, node: &TreeNode, ctx: &mut TreeNodeContext) {
    if !node.is_dir || ctx.expanded_dirs.contains(&node.path) {
        for child in &visible_children {
            // Check if this child is in the viewport
            let child_rect = ...; // approximate
            if viewport.intersects(child_rect) {
                draw_tree_virtual(ui, child, ctx);
            } else {
                // Reserve space but don't render
                ui.allocate_space(egui::vec2(0.0, ROW_HEIGHT));
            }
        }
    }
}
```

**Pros:**
- Minimal structural change from current recursive renderer
- No persistent flat list needed

**Cons:**
- Still traverses the full tree (O(N) per frame) just to skip items
- Viewport intersection test is approximate
- Recursion depth may overflow for very deep trees

**Verdict:** Middle ground. Still O(N) traversal so not a true fix.

---

## Recommendation

| Approach | Effort | Impact | Maintenance |
|----------|--------|--------|-------------|
| **A** (collapse) | None | Low | None |
| **B** (fixed-height viewport) | High | High | Medium |
| **C** (measured heights) | Very high | Very high | High |
| **D** (rate-limited) | Low | Medium | Low |
| **E** (flat virtual list) | High | Very high | Medium |
| **F** (sparse skip) | Medium | Medium | Low |

### Primary Recommendation: Approach E (Flattened Virtual List)

If profiling confirms the tree is the bottleneck (>5ms per frame in `draw_tree_node`), implement approach E:

1. Build a helper struct `VirtualTreeList` that:
   - Flattens the current `TreeNode` hierarchy into `Vec<FlatRow>` on tree mutations
   - Tracks which rows are visible via expand/collapse state
   - Exposes `visible_rows_in_viewport(viewport_rect) -> &[FlatRow]`
2. Replace the recursive `draw_tree_node()` call in `show_left_panel()` with:
   - A call to `virtual_tree.rows_in_viewport()`
   - A loop that renders only those rows using `ui.allocate_rect()`/`ui.put()`
3. On expand/collapse, update `expanded_dirs` and rebuild the flat list incrementally (not full flatten)

### Quick Win: Approach D (Rate-Limited) while E is built

As a stopgap, throttle the render frequency of the tree panel to 30fps. This is a 3-line change in `show_left_panel()` that halves the per-frame cost immediately.

---

---

# Egui Best Practices, Design Patterns & Guidelines

## 1. Architecture Patterns

### 1.1 Separate State from UI Logic

The single most important architectural decision: keep application state in a dedicated struct, separate from UI building functions.

```rust
// GOOD
struct AppState {
    documents: Vec<Document>,
    selected_id: Option<Id>,
    preferences: Preferences,
}

struct MyApp {
    state: AppState,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render_top_panel(ctx);
        self.render_side_panel(ctx);
        self.render_central_panel(ctx);
    }
}
```

### 1.2 The `update` Method as a Dispatcher

The main `eframe::App::update()` should be a thin dispatcher — never contain heavy logic directly. Each logical region gets its own `render_*` method. Keeping `update` under ~20 lines even in apps with dozens of widgets.

### 1.3 Panel Organization

Use egui's built-in panels to carve the screen into semantic zones:

| Panel | Purpose |
|-------|---------|
| `TopBottomPanel::top("menu_bar")` | Menus and toolbars |
| `TopBottomPanel::bottom("status_bar")` | Status bars |
| `SidePanel::left("nav_panel")` | Navigation or property lists |
| `SidePanel::right("inspector")` | Details / properties |
| `CentralPanel::default()` | Main content canvas |
| `Window::new("settings")` | Floating dialogs |

**Rules:**
- Each panel gets a **unique ID string** — reusing IDs causes layout flickering or misplaced widgets.
- Declare `TopBottomPanel` and `SidePanel` **before** `CentralPanel`. The central panel takes remaining space.
- Panels coordinate through shared state (`&mut self`), not an event bus.

### 1.4 Tabbed Interfaces Pattern

Use `enum Tab + match dispatch + per-tab helper`:

```rust
#[derive(PartialEq, Clone, Copy)]
enum Tab { Display, Audio, Editor }

struct MyApp {
    settings: Settings,
    active_tab: Tab,
}

// Sidebar with tab selection
egui::SidePanel::left("tabs")
    .default_width(120.0)
    .show(ctx, |ui| {
        ui.selectable_value(&mut self.active_tab, Tab::Display, "Display");
        ui.selectable_value(&mut self.active_tab, Tab::Audio, "Audio");
        ui.selectable_value(&mut self.active_tab, Tab::Editor, "Editor");
    });

// Central panel dispatches per-tab
egui::CentralPanel::default().show(ctx, |ui| {
    match self.active_tab {
        Tab::Display => self.display_settings(ui),
        Tab::Audio => self.audio_settings(ui),
        Tab::Editor => self.editor_settings(ui),
    }
});
```

### 1.5 Extracting Reusable Widget Functions

When a widget pattern repeats, extract it into a function that takes `&mut self` and `&mut egui::Ui`:

```rust
impl MyApp {
    fn render_file_list(&mut self, ui: &mut egui::Ui) {
        // owns no state itself — borrows from self
    }
}
```

Aim for functions <30 lines. They can be tested in isolation.

---

## 2. State Management

### 2.1 Three-Tier State System

egui uses a three-tier system because it rebuilds the UI from scratch every frame:

| Tier | Struct | Lifetime | Persistence | Purpose |
|------|--------|----------|-------------|---------|
| **1. Memory** | `Memory` | App lifetime | Optional (serde) | Global settings, widget state, window positions |
| **2. PassState** | `PassState` | Single pass | No | Shapes, accessibility, allocated space |
| **3. Local/Temp** | Various | Function scope | No | Transient UI state within closures |

### 2.2 Application State vs UI State

| State Type | Storage | Example |
|-----------|---------|---------|
| Application data | Your own structs | Documents, settings, business logic |
| UI state | `Memory.data` (via `IdTypeMap`) | Window positions, collapsed states, scroll positions |

### 2.3 Persistence via `eframe::Storage`

```rust
#[derive(Serialize, Deserialize, Clone)]
struct Settings {
    theme: Theme,
    font_size: f32,
    // ...other settings
}

impl MyApp {
    fn new(cc: &eframe::CreationContext) -> Self {
        let settings = cc.storage
            .and_then(|s| eframe::get_value(s, APP_KEY))
            .unwrap_or_default();
        Self { settings, active_tab: Tab::Display }
    }
}

impl eframe::App for MyApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, APP_KEY, &self.settings);
    }
}
```

**Persistence rules:**
- Use a **custom** `APP_KEY` string — not `eframe::APP_KEY` — when saving a subset of app state.
- Derive `Clone` on Settings (future-proofs undo, A/B previews).
- Add `#[serde(default)]` to new fields added after initial release.
- Persist user-facing settings only; transient UI state (active tab, open menus) is a design choice.

### 2.4 The IdTypeMap for Widget State

Use `ctx.data_mut()` / `ctx.data()` to store and retrieve type-safe widget state:

```rust
// Store
ctx.data_mut(|d| d.insert_persisted(id, my_state));

// Retrieve
let state: Option<MyState> = ctx.data(|d| d.get_persisted(id));
```

IDs are 64-bit hashes generated automatically from widget source location + parent context. Use `ui.id()` for stable IDs, `ui.next_auto_id()` for position-dependent ones.

---

## 3. Custom Widgets

### 3.1 The `Widget` Trait

```rust
impl Widget for MyWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        // 1. Decide space
        let desired_size = egui::vec2(100.0, 20.0);

        // 2. Allocate space
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        // 3. Paint (only if visible)
        if ui.is_rect_visible(rect) {
            ui.painter().rect_filled(rect, 0.0, egui::Color32::RED);
        }

        response
    }
}
```

**Patterns:**
- `ui.add(MyWidget::new(...))` for owned widgets.
- Implement `Widget` for `&mut YourType` when the widget needs to modify internal state.
- Always check `ui.is_rect_visible(rect)` before painting — free performance win for off-screen content.

### 3.2 Builder Pattern

egui uses the builder pattern for widget construction because Rust lacks named/default arguments:

```rust
ui.add(Label::new("Hello").text_color(Color32::RED));
```

Shortcut methods like `ui.label("Hello")` exist for common cases.

---

## 4. Performance Optimization

### 4.1 Frame Timing Budget

Target **16.6 ms** per frame for 60 FPS. A healthy egui app budget:

| Phase | Target |
|-------|--------|
| UI logic (your code) | <3 ms |
| egui layout + tessellation | <2 ms |
| GPU rendering | <11 ms |

If UI logic exceeds 5ms, profile immediately.

### 4.2 Profiling

Use [`puffin`](https://docs.rs/puffin) — egui integrates with it natively:

```rust
puffin::profile_scope!("my_section");
// ... code to profile
```

Enable the built-in profiler viewer with `egui::Window::new("Profiler")` for real-time flame graphs.

### 4.3 Widget Count Reduction

Each widget adds CPU overhead:

1. **`ui.add_visible_ui(visible, \|ui\| { ... })`** — egui skips layout for hidden content entirely.
2. **`CollapsingHeader`** — collapsed content costs almost zero CPU.
3. **Virtual scrolling** — only create widgets for visible rows. Manual calculation required with `ScrollArea::show_viewport()`.
4. Avoid deep nesting of `ui.horizontal()` / `ui.vertical()` — multiplied layout work.
5. Decorations (`ui.separator()`) in tight loops add up.

### 4.4 Virtual Scrolling with `show_viewport`

For large lists or trees:

```rust
egui::ScrollArea::vertical()
    .id_source("virtual_list")
    .show_viewport(ui, |ui, viewport| {
        let row_height = 22.0;
        let first = (viewport.min.y / row_height) as usize;
        let last = ((viewport.max.y / row_height) as usize + 1).min(items.len());

        ui.set_height(items.len() as f32 * row_height);

        for i in first..last {
            let rect = egui::Rect::from_min_size(
                egui::pos2(0.0, i as f32 * row_height),
                egui::vec2(ui.available_width(), row_height),
            );
            let response = ui.allocate_rect(rect, egui::Sense::click());
            ui.put(rect, |ui: &mut egui::Ui| {
                ui.label(&items[i]);
            });
        }
    });
```

### 4.5 Caching Expensive Operations

- **Textures:** Generate once, store `TextureHandle`, reuse across frames.
- **Formatted text:** Build `RichText` objects on data change, not every frame.
- **String buffers:** Reuse with `.clear()` instead of allocating new `String`s.
- **Computation cache:** Use `ctx.memory_mut(|mem| mem.caches.cache::<MyCache>())` for `FrameCache`.

### 4.6 Repaint Control

By default, egui repaints continuously. For battery/CPU efficiency:

```rust
// Request repaint only when needed
ctx.request_repaint(); // for immediate redraw
ctx.request_repaint_after(Duration::from_millis(16)); // throttled

// Check if continuous repaint is needed
if animation_active {
    ctx.request_repaint();
}
```

Reactive mode (repaint only on interaction) reduces CPU usage from ~100% to near 0% when idle.

---

## 5. Layout Patterns

### 5.1 Understanding Immediate Mode Layout

egui is **single-pass immediate mode** — the UI is fully rebuilt every frame. This creates a circular dependency: to position a window you need its size, but to know the size you must lay out the contents, which requires knowing the position.

**Solutions:**
- **Frame delay:** Store sizes from frame N-1, use on frame N. May cause first-frame jitter for new elements.
- **Multi-pass:** `ctx.request_discard()` discards the current frame's visual output and runs another pass. Used internally by `Grid`, `Table`, etc.
- **`Options::max_passes`** defaults to 2, limiting multi-pass to rare circumstances.

### 5.2 Auto-Sizing Behavior

Panels and windows auto-shrink to fit content. When combined with resizing, this can create a "rubber-band" effect where the panel shrinks after you release the drag.

**Workarounds:**
1. Disable resizing: `.resizable(false)`
2. Wrap in `ScrollArea`
3. Add `ui.allocate_space(ui.available_size())` **last** in the panel/window

### 5.3 Advanced Layout Crates

For flexbox-style layout, consider ecosystem crates:
- **[egui_flex](https://crates.io/crates/egui_flex)** — flexbox layout using frame-delay sizing
- **[taffy](https://crates.io/crates/taffy)** — full flexbox/grid layout engine (can be integrated with egui)

---

## 6. Asynchronous Operations

### 6.1 Thread/Channel Pattern

Never block the UI thread:

```rust
fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    // Check for background results
    if let Ok(result) = self.receiver.try_recv() {
        self.loaded_data = Some(result);
        ctx.request_repaint();
    }

    // Render UI
    // ...
}
```

**Flow:**
1. Spawn thread/async task outside `update()`
2. Send results via `std::sync::mpsc::Receiver`
3. Check receiver in `update()` and update state + `request_repaint()`

### 6.2 egui_mobius Pattern

For complex apps, [`egui_mobius`](https://github.com/saturn77/egui_mobius) provides:
- **`Dynamic<T>`** — thread-safe reactive cell (Arc-backed), synchronously readable/writable from any thread
- **`Derived<T>`** — auto-updating computed values
- **Signal/Slot** — type-safe message passing for decoupled communication
- **Panel-as-citizen** — dockable panel lifecycle with dispatcher coordination

```rust
// Thread-safe state shared across panels
let documents: Dynamic<Vec<Document>> = Dynamic::new(vec![]);

// Write from any thread
documents.set(new_list);

// Read from UI thread
let list = documents.get();
```

---

## 7. Cross-Panel Communication

### 7.1 Shared Mutable State

For simple apps, the app struct itself is the coordination mechanism — all panels read/write the same `&mut AppState`:

```rust
// Top panel writes
self.selected = i;

// Central panel reads
let doc = &self.documents[self.selected];
```

**Rules of thumb:**
- Compute booleans **before** taking immutable references to non-selected data
- No event bus is needed — the shared mutable struct IS the coordination
- Use `let selected = self.selected;` before `&self.documents[selected]` to satisfy the borrow checker

### 7.2 When to Use Stored vs Stateless Panels

| Panel type | Use case | Example |
|-----------|----------|---------|
| **Stored** (field on app) | Owns local state that must survive frames | Log buffer, terminal scrollback, image cache, filter text |
| **Stateless** (per-frame construction) | Pure view over shared data | DRC results, settings panel, file list |

Decision rule: "Does this panel own state that must survive between frames?" If yes → stored, else → stateless. Default to stored when unsure.

---

## 8. Common Mistakes & Anti-Patterns

### 8.1 ID Management

- ❌ Reusing panel/area IDs → flickering, layout corruption
- ✅ Use unique strings per panel: `SidePanel::left("unique_id")`
- ❌ Assuming widget IDs are stable across code changes
- ✅ Use explicit `Id::new("my_widget")` for persistent widget state

### 8.2 Panel Declaration Order

- ❌ Declaring `CentralPanel` before side/top panels → no room for other panels
- ✅ Correct order: `TopBottomPanel` → `SidePanel` → `CentralPanel`

### 8.3 Persistence

- ❌ Persisting transient UI state (active tab, open menus) without deliberate design
- ❌ Using `eframe::APP_KEY` for partial app saves — use a custom key
- ❌ Forgetting `#[serde(default)]` on new fields → broken loading of old data

### 8.4 Layout

- ❌ Deeply nested `horizontal` / `vertical` closures — multiplied layout cost
- ❌ Putting too much content in a `TopBottomPanel::bottom` — status bars should be dense
- ❌ Trying to nest `TopBottomPanel` or `SidePanel` inside another panel — they are top-level constructs

### 8.5 Performance

- ❌ Memory allocations (String, Vec) inside the UI loop
- ❌ Linear search over 10k+ items inside a widget loop — use HashMap lookups
- ❌ Decoding images every frame — upload texture once, reuse handle
- ❌ Many overlapping shapes with high tessellation cost (rounded rects, shadows)

### 8.6 Threading

- ❌ Blocking the UI thread with synchronous I/O
- ❌ Holding external locks while accessing `Context` (deadlock risk)
- ❌ Recursively locking the same `Context` within a closure

---

## 9. Recommended Ecosystem Crates

| Crate | Purpose |
|-------|---------|
| [`eframe`](https://crates.io/crates/eframe) | Application framework (native + web) |
| [`egui_extras`](https://crates.io/crates/egui_extras) | Tables, images, layout helpers |
| [`egui_plot`](https://crates.io/crates/egui_plot) | Plotting and data visualization |
| [`egui_dock`](https://crates.io/crates/egui_dock) | Dockable panel system |
| [`egui_mobius`](https://crates.io/crates/egui_mobius) | Reactive state + async + panel lifecycle |
| [`egui_flex`](https://crates.io/crates/egui_flex) | Flexbox layout with frame-delay sizing |
| [`egui_sauge`](https://crates.io/crates/egui_sauge) | Design system + component library |
| [`puffin`](https://crates.io/crates/puffin) | Profiler with egui viewer integration |

---

# Audit: Current Implementation vs Best Practices

This section compares `doc/planning/egui.md § Egui Best Practices, Design Patterns & Guidelines` against the actual codebase and produces prioritized recommendations.

## P0 — Critical (performance or correctness)

### P0-1: Virtual Scrolling for the File Tree

| | |
|---|---|
| **What** | The left-panel file tree (`src/desktop/src/ui/tree.rs:56-287`) renders **every visible node every frame**. For directories with many files, thousands of `selectable_label` widgets are created per frame, even for items far outside the viewport. |
| **Where** | `src/desktop/src/ui/tree.rs` — `draw_tree_node()` recursively renders all descendants of expanded directories. |
| **Why** | The file tree already limits rendering to expanded directories (short-circuits when `!is_expanded`), but a single expanded directory with 1000+ files still creates 1000+ widgets per frame. The codebase already proves the pattern works: `background_logs.rs:121` uses `ScrollArea::show_rows()` with virtual scrolling. The file tree should do the same. |
| **Existing research** | `doc/planning/egui.md` already documents **Approach E (Flattened Virtual List)** as the recommended solution with full implementation sketches in Rust (lines 138-215 of the original virtual-scrolling research). |
| **Action** | Add a `VirtualTreeList` struct that flattens `TreeNode` into `Vec<FlatRow>` on tree mutation, then use `show_rows()` (or `show_viewport()`) in `left.rs:170` to render only visible rows. |

### P0-2: `update_ui()` Is a Monolithic Dispatcher

| | |
|---|---|
| **What** | `app.rs:451-668` (`update_ui`) is **217 lines** and does: file-event processing, channel draining, file loading from disk, modal display, panel rendering, deferred prompt submission, and batch-thread join checking — all in one function. |
| **Where** | `src/desktop/src/ui/app.rs:451-668` |
| **Why** | Best practices say `update()` should be a ~20-line dispatcher that delegates to `render_*` methods. The current code mixes file I/O, state mutation, and UI rendering in one function. This makes it hard to test individual responsibilities, easy to accidentally reorder panel rendering, and violates the "single responsibility" principle. |
| **Action** | Extract into: `process_file_events()`, `drain_background_channel()`, `handle_deferred_actions()` (batch, submit_prompt), `show_modals()`, and `render_panels()`. Each extracted method gets its own doc comment and unit tests. The extracted `render_panels()` would be the ~5-line dispatcher. |

---

## P1 — High (architecture quality, maintainability)

### P1-1: No egui Persistence for Window/Widget State

| | |
|---|---|
| **What** | No use of `eframe::Storage`, `IdTypeMap`, or `ctx.data_mut()` for persisting UI state. Panel widths, window positions, collapsed directories, the TOC panel visibility, and scroll positions are all lost on restart. |
| **Where** | Throughout — no file uses `eframe::get_value`/`set_value`, `ctx.data()`, or `ctx.data_mut()`. |
| **Why** | The app currently reloads everything from filesystem config at startup. User customizations like panel resize, collapsed directories, and scroll position are gone each session. This is a significant UX deficit. |
| **Action** | Add `#[derive(Serialize, Deserialize)]` to a `PersistedUiState` struct holding `left_panel_width`, `collapsed_dirs` (subset), window size/position. Save via `eframe::App::save()`, load in `FastMdApp::new()`. Use a custom `APP_KEY`. |

### P1-2: Hack for SidePanel Width Reset

| | |
|---|---|
| **What** | `panel_layout.rs:6` stores `left_panel_reset_count: u32` that is appended to the SidePanel ID: `egui::Id::new("left_panel").with(app.layout().left_panel_reset_count)`. Every time the width is recalculated, the counter increments, creating a new widget ID and forcing egui to re-read `default_width`. |
| **Where** | `src/desktop/src/ui/panel_layout.rs:6`, `src/desktop/src/ui/panels/left.rs:161` |
| **Why** | This is a workaround for egui's SidePanel caching behavior. The proper fix is to use `ctx.data_mut()` with a stable ID to store/restore the panel's actual width, eliminating the need for the counter hack. The counter approach also leaks: each recalculation discards the user's manual resize. |
| **Action** | Store `left_panel_width` via `ctx.data_mut(|d| d.insert_persisted(panel_id, width))` using a stable `Id`. Remove `left_panel_reset_count` from `PanelLayout`. Use `ctx.data()` to read the persisted width on construction. |

### P1-3: `TreeNodeContext` Has 22 Fields — Split It

| | |
|---|---|
| **What** | `tree.rs:12-37` defines `TreeNodeContext` with 22 mutable references (`&'a mut`). Every caller in `left.rs:196-218` must destructure `app` into 22 individual borrows. Tests in `tree.rs:387-408` and `tree.rs:458-482` repeat this verbatim. |
| **Where** | `src/desktop/src/ui/tree.rs:12-37` (struct def), `src/desktop/src/ui/panels/left.rs:196-218` (construction), `src/desktop/src/ui/tree.rs:387-482` (test construction) |
| **Why** | 22 mutable borrows is fragile and verbose. Adding a new operation requires adding a field to the context, a parameter in every construction site, and a mutable borrow in every caller. This violates the "small interfaces" principle. |
| **Action** | Group into sub-contexts: `FileOpsContext` (file_to_move, move_dialog_open, rename_*), `DirOpsContext` (create_dir_*, selected_dir), `SelectionContext` (selected_file, selected_files, expanded_dirs), `AppIntegrationContext` (submit_prompt, layout, tabs, open_editor, etc.). Alternatively, pass `&mut FastMdApp` and let `draw_tree_node` access what it needs through methods. |

---

## P2 — Medium (code quality, idiomatic egui)

### P2-1: No `Widget` Trait Implementations

| | |
|---|---|
| **What** | `draw_tree_node()` is a free function taking `&mut Ui` and `&mut TreeNodeContext`. No custom type implements `egui::Widget`. |
| **Where** | `src/desktop/src/ui/tree.rs:56` |
| **Why** | While free functions are valid, implementing `Widget` for `TreeNode` would enable `ui.add(&mut tree_node)` syntax, composability within egui's widget system, and automatic `Response` handling. |
| **Action** | Create a `TreeWidget` struct that holds the root `TreeNode` and relevant state, then `impl Widget for TreeWidget`. Use `ui.add(tree_widget)` in `left.rs` instead of `crate::ui::tree::draw_tree_node(...)`. |

### P2-2: No `ui.add_visible_ui()` for Conditional Content

| | |
|---|---|
| **What** | The right TOC panel (`right.rs:39-74`) wraps its entire body in `if should_show_panel(...) { SidePanel::show(...) }`. The panel is simply not shown when conditions aren't met. |
| **Where** | `src/desktop/src/ui/panels/right.rs:39-74` |
| **Why** | The SidePanel itself is not created when conditions aren't met, which is correct. However, for conditional content _within_ a panel (e.g., collapsed sections, conditional metadata), `ui.add_visible_ui(visible, \|ui\| { ... })` skips layout of hidden content entirely — more efficient than `if visible { ... }`. |
| **Action** | Audit all `if condition { ui.* }` blocks in panels for potential replacement with `ui.add_visible_ui()`. Notable candidates: center panel's three-way routing (agent vs tabs vs empty state), and collapsing sections. |

### P2-3: File Content Loaded on UI Thread Each Frame

| | |
|---|---|
| **What** | `app.rs:528-543` reads a file from disk (`std::fs::read_to_string`) on the UI thread when `selected_file` changes. |
| **Where** | `src/desktop/src/ui/app.rs:528-543` |
| **Why** | Blocking the UI thread for file I/O is an anti-pattern per best practices §6.1. For small markdown files this is fast (<1ms), but it still blocks layout and creates a coupling between file I/O and frame rendering. |
| **Action** | Offload file reading to a background thread, send content back via the existing `rx` channel with a new `BackgroundMessage::FileLoaded { path, content }` variant. The UI thread checks once per frame and populates `tab_manager` non-blockingly. |

---

## P3 — Low (nice to have, future-proofing)

### P3-1: Missing egui Ecosystem Crates

| | |
|---|---|
| **What** | Only `eframe` is used. No `egui_extras`, `egui_dock`, `egui_plot`, or `egui_mobius`. |
| **Where** | `src/desktop/Cargo.toml` |
| **Why** | `egui_extras::Table` could simplify structured data views. `egui_extras::DatePicker` could help date inputs. `egui_dock` could provide drag-reorderable panels. Not urgent — current panels meet requirements — but worth noting. |
| **Action** | Evaluate `egui_extras::Table` for the YAML front-matter table (`render.rs:218`) as a replacement for the manual `Grid` + `Frame` construction. |

### P3-2: No `FrameCache` for Expensive Calculations

| | |
|---|---|
| **What** | `calc_max_width` in `left.rs:122-145` recursively measures every node's text width using `ctx.fonts(|f| f.layout_no_wrap(...))`. This runs every time the dirty flag is set or indexing finishes. |
| **Where** | `src/desktop/src/ui/panels/left.rs:122-145` |
| **Why** | `ctx.memory_mut(|mem| mem.caches.cache::<FrameCache<...>>())` could cache text-width calculations across frames. Currently the calculation runs fully each time the dirty flag is set. For large trees this could be costly node-by-node. |
| **Action** | Wrap the text-width calculation in a `FrameCache` keyed by node path. The cache auto-evicts after one frame, which is fine since the dirty flag recalculates only on structural changes. |

### P3-3: No Throttled Repaint Strategy

| | |
|---|---|
| **What** | `app.rs:523-525` repaints every frame during indexing. After indexing, only event-driven repaints occur. There is no throttling mechanism for animation or scroll-heavy scenarios. |
| **Where** | `src/desktop/src/ui/app.rs:523-525` |
| **Why** | During indexing, continuous repaint is correct (showing progress). For other scenarios (agent streaming response, background animation), `ctx.request_repaint_after(Duration)` could limit repaints to e.g. 30fps (33ms) instead of maxing out at display refresh rate. |
| **Action** | Add a configurable `repaint_interval` field; use `ctx.request_repaint_after(repaint_interval)` instead of `ctx.request_repaint()` where near-real-time updates suffice (agent thinking animation, spinner). |

---

## Summary

| Priority | ID | Issue | File(s) | Effort | Impact |
|----------|----|-------|---------|--------|--------|
| **P0** | P0-1 | No virtual scrolling for file tree | `tree.rs`, `left.rs` | Large | Very High |
| **P0** | P0-2 | `update_ui()` is 217-line monolith | `app.rs` | Medium | High |
| **P1** | P1-1 | No egui persistence (state lost on restart) | `app.rs` | Medium | High |
| **P1** | P1-2 | SidePanel width reset hack | `panel_layout.rs`, `left.rs` | Small | Medium |
| **P1** | P1-3 | `TreeNodeContext` has 22 fields | `tree.rs`, `left.rs` | Medium | Medium |
| **P2** | P2-1 | No `Widget` trait impl | `tree.rs` | Medium | Low |
| **P2** | P2-2 | No `add_visible_ui` for conditional content | `right.rs`, `center.rs` | Small | Low |
| **P2** | P2-3 | File I/O on UI thread | `app.rs` | Medium | Medium |
| **P3** | P3-1 | Missing `egui_extras` crates | `Cargo.toml` | Small | Low |
| **P3** | P3-2 | No `FrameCache` for text-width calc | `left.rs` | Small | Low |
| **P3** | P3-3 | No throttled repaint | `app.rs` | Small | Low |

**Quick wins** (do first): P1-2 (panel width hack), P2-2 (add_visible_ui), P3-2 (FrameCache), P3-3 (throttled repaint).

**Biggest impact**: P0-1 (virtual scrolling) and P1-1 (persistence) deliver the most user-visible improvement.

---

## Open Questions

1. **Multi-selection interaction:** How does shift-click range selection work when rows outside the viewport are not instantiated? Needs to operate on the flat row indices, not widget memory.

2. **Context menus:** With non-rendered rows, context menu logic must work on the flat row index (already available from the event path). No issue.

3. **Drag and drop:** If future feature adds DnD, virtualized items need stable identifiers. `PathBuf` works as a stable key.

4. **Expand/collapse animation:** egui has no built-in animation. A flat list with instant insert/remove of children rows may feel abrupt. Not a current concern.

5. **Filter performance:** Tag filtering currently rebuilds the tree O(N). With a flat list, filtering is O(N) scan plus O(N) list rebuild. Acceptable since filtering is a user action, not per-frame.
