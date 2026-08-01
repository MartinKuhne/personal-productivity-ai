# Performance Profiling Tooling

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
