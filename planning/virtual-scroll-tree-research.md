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

## Open Questions

1. **Multi-selection interaction:** How does shift-click range selection work when rows outside the viewport are not instantiated? Needs to operate on the flat row indices, not widget memory.

2. **Context menus:** With non-rendered rows, context menu logic must work on the flat row index (already available from the event path). No issue.

3. **Drag and drop:** If future feature adds DnD, virtualized items need stable identifiers. `PathBuf` works as a stable key.

4. **Expand/collapse animation:** egui has no built-in animation. A flat list with instant insert/remove of children rows may feel abrupt. Not a current concern.

5. **Filter performance:** Tag filtering currently rebuilds the tree O(N). With a flat list, filtering is O(N) scan plus O(N) list rebuild. Acceptable since filtering is a user action, not per-frame.
