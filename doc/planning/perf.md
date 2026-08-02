# Rendering Performance Improvement Plan

**Status:** accepted
**Date:** 2026-08-01
**Reviewer:** AI Agent

---

## Context

The `fastmd` crate (~43.6K LOC) is an egui-based markdown knowledge-base viewer. Current CPU load is 3.1% on a powerful system (AMD 7950X / Intel XPS 15), but frame-time profiling reveals redundant work every frame even when nothing changes. The immediate-mode egui architecture rebuilds the entire widget tree each frame; without caching, stable content incurs O(N) work per frame where N = files in workspace or lines in document.

### What Changes vs. Stays Stable Per Frame

| Component | Stability | Current Cache Status |
|-----------|-----------|---------------------|
| Markdown text content | Stable until file reload/edit | ✅ Parsed events cached via text hash |
| Markdown AST / RenderEvents | Stable | ✅ Cached in `ui.ctx().data_temp` |
| Table cell content & token widths | Stable | ✅ `measure_cached` with cell_hash + font_hash |
| Table column max/min/breakpoints | Stable | ✅ `measure_cached` |
| Table FTWA decision | Stable if available_width same | ✅ `ftwa_cached` with input_hash + avail + strategy |
| Table row heights | Stable | ✅ Cached per-row in egui temp data |
| YAML front matter | Stable until file reload | ❌ Re-rendered every frame |
| File tree structure | Stable until file add/remove | ❌ `build_workspace_tree` runs every frame |
| TOC entries | Stable until file reload | ✅ Built once on file load, re-rendered each frame |
| Panel sizes | Stable until user resizes | ✅ Cached in `PanelLayout` |
| Font metrics / text shaping | Stable | ✅ Cached via measure/ftwa caches |

### Redundant Work Every Frame (Performance Targets)

1. **`build_workspace_tree()`** (`left.rs:148`) — Rebuilds entire `TreeNode` hierarchy from `all_files` every frame — **expensive for large workspaces**
2. **`flatten_tree()`** — Converts tree to flat rows every frame
3. **`render_markdown` full iteration** — Walks all `RenderEvent`s even when only scroll position changed
4. **YAML table re-render** — Full layout every frame
4. **Tab strip rebuild** — Recreates all tab buttons every frame
5. **TOC rebuild** — Full re-layout each frame despite stable data

---

## Decision: Prioritized Optimization Plan

### 🔴 P0 — High Impact, Low Risk (Implement First)

| # | Optimization | Location | Expected Gain |
|---|--------------|----------|---------------|
| **P0-1** | **Cache `TreeNode` hierarchy** — Only rebuild when `all_files`/`all_dirs`/`tags`/library list changes. Add `tree_dirty` flag like `left_panel_dirty`. | `left.rs:build_workspace_tree` | **Eliminates O(files) tree rebuild every frame** — biggest win for large workspaces |
| **P0-2** | **Cache flattened rows** — Store `Vec<FlatRow>` in `SelectionManager` or `PanelLayout`, invalidate only on tree structure change. | `left.rs:243-246` | Avoids `flatten_tree` + `push_id` allocation every frame |
| **P0-3** | **Extend viewport culling to all event types** — Currently skips only `FlushInline` and `CodeBlock` off-screen. Extend to skip `Heading`, `Table`, `Space`, `Separator` when off-screen. | `render/mod.rs:123-150` | Reduces widget allocation for long documents; already has clip_rect logic |
| **P0-4** | **Cache YAML table render** — Store rendered row layout data; only re-render when YAML content changes. | `yaml_table.rs` | Avoids full table layout every frame for documents with front matter |

### 🟠 P1 — Medium Impact, Medium Effort

| # | Optimization | Location | Expected Gain |
|---|--------------|----------|---------------|
| **P1-1** | **Memoize tab strip** — Cache tab button widgets; only rebuild when tabs vector changes. | `center.rs:247-338` | Eliminates per-frame tab button allocation |
| **P1-2** | **Virtualize TOC rendering** — Use `ScrollArea::show_rows` for TOC like left panel does. | `right.rs:158-185` | Scales to documents with hundreds of headings |
| **P1-3** | **Pre-compute heading IDs once** — `render_markdown` builds `heading_seen` HashMap every frame. Move to `TabManager` with markdown content hash. | `render/mod.rs:110-121` | Avoids HashMap allocation + string ops every frame |

### 🟡 P2 — Lower Impact / Higher Risk

| # | Optimization | Location | Risk / Effort |
|---|--------------|----------|---------------|
| **P2-1** | Persistent egui widget IDs for static content | `render/mod.rs` | High: stable IDs across frames challenge |
| **P2-2** | Separate "layout" vs "paint" passes | Architecture | Very High: fundamental egui model change |
| **P2-3** | GPU-accelerated text rendering | Architecture | High: major refactor, diminishing returns at 3.1% CPU |

### 🟢 P3 — Profiling & Validation Infrastructure

| # | Task | Purpose |
|---|------|---------|
| **P3-1** | Add frame-time profiling — Integrate `puffin` scopes around each panel + `render_markdown` + `build_workspace_tree` | Quantify actual bottlenecks |
| **P3-2** | Benchmark harness — Render large markdown file (10k lines) + large workspace (10k files) in headless mode | Regression detection |
| **P3-3** | Cache hit/miss metrics — Instrument `measure_cached` / `ftwa_cached` / markdown parse cache | Validate caching effectiveness |

---

## Implementation Order (P0 Focus)

```
Week 1:  P0-1 → P0-2  (Tree caching - highest ROI)
Week 2:  P0-3 → P0-4  (Markdown culling + YAML cache)
Week 3:  P1-1 → P1-2  (Tab strip + TOC virtualization)
Week 4:  P1-3 → P3-1  (Heading IDs + profiling)
```

---

## Expected Results

| Metric | Current | Target (after P0+P1) |
|--------|---------|---------------------|
| Frame time (10k file workspace) | ~8-12ms | **<2ms** |
| Frame time (10k line markdown) | ~5-8ms | **<1ms** |
| Tree rebuilds per second | 60 (every frame) | **0 (only on change)** |
| Markdown events processed/frame | All | **~10-20% (visible only)** |

---

## Key Implementation Patterns (Already Established in Codebase)

### Tree caching pattern (`left_panel_dirty` precedent)
```rust
// In SelectionManager or PanelLayout - add dirty flag
if app.selection.tree_dirty {
    root_node = build_workspace_tree(app);
    app.selection.flattened_rows = flatten_tree(&root_node, ...);
    app.selection.tree_dirty = false;
}
```

### Viewport culling extension (minimal change)
```rust
// In render/mod.rs - extend the clip check to ALL event types
if clip.is_positive() && top_y > clip.max.y + viewport_margin {
    // Skip rendering this event entirely, just add estimated space
    ui.add_space(estimated_height_for_event(event));
    continue;
}
```

### Tab strip memoization
The tab strip is stable except when tabs added/removed/closed. Cache the `Vec<PathBuf>` snapshot and only rebuild on change.

---

## Related

- Profiling tooling ADR: [`doc/planning/perf.md`](../planning/perf.md) (this document — profiling tooling section preserved below)
- egui documentation: [`doc/distill/egui.md`](../distill/egui.md)
- Architecture: [`doc/technical-context/ARCHITECTURE_C4.md`](../technical-context/ARCHITECTURE_C4.md)

---

# Profiling Tooling (Preserved from Original Proposal)

**Status:** proposal
**Date:** 2026-08-01

---

## Context

The `fastmd` crate (~43.6K LOC) has no built-in performance profiling or instrumentation tooling. When performance issues arise — slow renders, stuttering egui frames, sluggish markdown parsing — there is no way to see where time is spent inside the application. The project uses `egui` / `eframe 0.35` which has deep integration with two profiling ecosystems:

1. **puffin** — the lightweight profiler created by Emil Ernerfeldt (egui author), providing in-app flamegraph visualization.
2. **Tracy** — the high-precision system profiler supporting frame-by-frame nano-second tracking, memory allocation profiling, and GPU timeline integration.

The project's egui dependency already includes the `accesskit` feature. Adding profiling crates introduces no structural architecture changes — they are purely instrumentation additions with zero runtime overhead when disabled.

## Decision

Adopt **both** profiling tools in a feature-gated configuration:

### Layer 1: In-app flamegraphs with `puffin_egui` (quick win)

Add `puffin` + `puffin_egui` as conditional dependencies gated behind a `profiling` cargo feature.

**Changes:**

1. **`Cargo.toml`** — Add puffin dependencies under a `profiling` feature:

    ```toml
    [features]
    default = []
    profiling = ["puffin", "puffin_egui"]
    puffin = { version = "0.19", optional = true }
    puffin_egui = { version = "0.30", optional = true, default-features = false }
    ```

2. **`src/main.rs`** — Initialize `puffin` profiler at startup:

    ```rust
    #[cfg(feature = "profiling")]
    static PROFILER: puffin::Profiler = puffin::Profiler {};
    ```

3. **`ui/app.rs` — `FastMdApp::update`** — Instrument the egui frame loop:

    ```rust
    #[cfg(feature = "profiling")]
    puffin::GlobalProfiler::lock().new_frame();

    #[cfg(feature = "profiling")]
    puffin::profile_function!();
    ```

4. **`ui/app.rs`** — Add a conditional "Profiler" window (toggleable via a menu bar item or keyboard shortcut):

    ```rust
    #[cfg(feature = "profiling")]
    puffin_egui::profiler_ui(ctx);
    ```

5. **Hot-path instrumentation** — Add `puffin::profile_scope!` to the most expensive known paths:
    - `ui/render.rs` — `render_markdown` (4,152 LOC, known hot path)
    - `markdown/document.rs` — front-matter parsing and markdown parsing
    - `agent/tool_executor.rs` — tool execution loops
    - `ui/tree.rs` — tree rendering and flattening

**Why puffin first:**
- Zero external dependencies (no Tracy server to install).
- In-app UI means any user can profile without tools.
- Emil Ernerfeldt is the egui author — first-class ecosystem support.
- Minimal code changes (~20 LOC across 3-4 files).
- Can be merged and tested immediately as a quick win.

### Layer 2: Tracy for deep system-level profiling

Add `tracing-tracy` (or `tracy-client`) as an optional dependency gated behind the same `profiling` feature.

**Changes:**

1. **`Cargo.toml`** — Add tracy under the `profiling` feature:

    ```toml
    tracy-client = { version = "0.17", optional = true }
    # or: tracing-tracy = { version = "0.11", optional = true }
    ```

2. **`src/main.rs`** — Initialize Tracy subscriber:

    ```rust
    #[cfg(all(feature = "profiling", feature = "tracy"))]
    fn init_tracy() {
        tracing_subscriber::registry()
            .with(tracing_tracy::TracyLayer::default())
            .init();
    }
    ```

3. **Existing `tracing` usage** — The crate already uses `tracing` and `tracing-subscriber`. Tracy integration piggybacks on this — no `tracing` import changes needed. All existing `tracing::info!`, `tracing::debug!` calls automatically route through Tracy when the layer is active.

4. **Manual spans** — Add `tracy_client::span!{}` around known expensive operations where `puffin::profile_scope!` is insufficient (e.g., GPU rendering, network I/O).

**Why Tracy second:**
- Requires installing the Tracy Profiler GUI separately (external dependency).
- Higher precision for frame-by-frame analysis and memory tracking.
- Better for investigating issues that puffin identifies.
- Some overhead even when not connected (though negligible).

### Configuration summary

```
[features]
default = []
profiling = ["puffin", "puffin_egui"]

[dependencies]
# Conditional on "profiling" feature
puffin = { version = "0.19", optional = true }
puffin_egui = { version = "0.30", optional = true, default-features = false }
tracy-client = { version = "0.17", optional = true }
```

Build with in-app profiler: `cargo run --features profiling`
Build with Tracy: `cargo run --all-features`

---

## Consequences

### Positive

- **Immediate visibility** — Developers can identify hot paths without external tools.
- **No runtime cost when disabled** — Both puffin and Tracy have `cfg(feature = "...")` guards; zero-cost when the `profiling` feature is off.
- **Complementary coverage** — Puffin for quick in-app profiling; Tracy for deep system-level analysis.
- **No architectural changes** — Both are pure instrumentation; no module boundaries shift, no API changes.
- **User-facing option** — Power users can enable profiling via `--features profiling` without recompiling a separate debug build.

### Negative

- **Dependency growth** — Two additional crates (puffin, puffin_egui) plus an optional Tracy crate.
- **Build complexity** — The `profiling` feature adds conditional compilation paths that must be kept in sync.
- **Tracy overhead** — Tracy has a small but non-zero runtime overhead even when not connected; puffin has near-zero overhead.
- **No automated perf regression gates** — Profiling tools identify problems but don't prevent regressions. This is a future enhancement opportunity.

### Risks and mitigations

| Risk | Mitigation |
|------|-----------|
| Puffin API changes between versions | Pin versions in Cargo.toml; update with other egui ecosystem deps |
| Tracy feature conflicts with puffin | Gate both behind the same `profiling` feature; mutual exclusion not required (they are independent) |
| In-app profiler window clutters UI | Only visible when user explicitly opens it; hidden by default |
| Feature-gated code diverges from main | CI runs `cargo check --features profiling` to catch compilation errors |

---

## Implementation Order

1. **Phase 1** — Add puffin + puffin_egui dependencies and frame loop instrumentation (quick win, ~30 min).
2. **Phase 2** — Add `profile_scope!` annotations to known hot paths: `render_markdown`, tree rendering, markdown parsing (~1-2 hours).
3. **Phase 3** — Add Tracy integration as an optional sub-feature (1-2 hours).
4. **Phase 4** — Document profiling workflow in `src/desktop/README.md` or `src/desktop/Tools.md` (30 min).

---

## Related

- egui documentation: [`doc/distill/egui.md`](../distill/egui.md)
- Puffin upstream: <https://github.com/puffin-api/puffin>
- Tracy upstream: <https://github.com/wolfpld/tracy>
- Existing `tracing` usage: `src/desktop/src/agent/tool_executor.rs`, `src/desktop/src/main.rs`
