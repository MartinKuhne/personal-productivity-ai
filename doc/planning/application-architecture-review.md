# Application Architecture Review

**Date:** 2026-07-29
**Scope:** `src/desktop/src/` (`fastmd` Rust crate, ~37K LOC)
**Metric:** ~37K LOC across 6 bounded subsystems (`app/`, `agent/`, `tools/`, `markdown/`, `ui/`, `background/`), plus `config/`, `utils/`.
**Goal:** Evaluate module boundaries, coupling, and modularity against the project's hexagonal-flavor conventions (see `src/desktop/AGENTS.md`).

---

## Overall Assessment

The subdivision by concern is **sound**, and the egui/domain boundary is **strictly enforced**. The `markdown/` subsystem is the cleanest module in the crate — it imports nothing from `ui/`, `agent/`, or `app/` (save `ToCEntry` for TOC). The `app/` domain is testable without a UI harness. These are mature architectural decisions.

However, **coupling is accumulating at the seams between subsystems**, and **file-level modularity has degraded** in two of the largest shared types (`registry.rs`, the agent-manager startup path). Some single-responsibility violations are creating hard-to-test code paths, particularly around the agent ↔ MCP ↔ tool-registry lifecycle.

---

## Prioritized Refactoring Recommendations

### P0 — High Impact, Low Disruption

| # | Location | Problem | Techniques |
|---|----------|---------|------------|
| 1 | `agent/tools/registry.rs` (1,945 LOC) | **Single Responsibility Violation.** The `ToolRegistry` is simultaneously a tool dispatcher, a schema-builder for the LLM, the MCP tool adapter factory, *and* owns the `McpClientManager` lifecycle. It is the largest file in the crate and will only grow as tool families are added. | **Extract `mcp_manager` into a dedicated `McpToolSource` trait** or a separate `McpToolBridge` type. Split the registry into: `(a) Registry::register_all` (static tool set), `(b) Registry::execute`, `(c) a `DynamicToolSource` trait implemented by `McpClientManager` so the registry holds only `Box<dyn DynamicToolSource>`. This drops coupling to the concrete MCP module and shrinks the registry to <400 LOC. |
| 2 | `agent/manager.rs:49-60` → `registry::init_mcp_on_startup` | **Agent lifecycle performs network I/O at construction time.** `AgentSessionManager::new()` immediately pings every configured MCP server. If any server is slow or unreachable, `new()` blocks. This makes the agent hard to unit-test (no "offline" constructor) and couples agent startup to MCP availability. | **Introduce a `BackgroundInitializer` trait / `init` step parameter.** `AgentSessionManager::new(config)` should be pure. MCP ping → tool discovery should be an explicit `manager.initialize().await` step triggered by the UI on app startup. Use `app::background::Task` machinery to drive it. Tests construct managers without network. |
| 3 | Top-level `document.rs` + `app/document.rs` | **Duplicate document model.** There is a `DocumentContent` in `app/document.rs` and a top-level shim referenced by `agent_impl.rs` ("top-level `DocumentContent` shims" per exploration). Two representations of "a markdown document" invites split-brain bugs (front-matter parsed twice, body normalization inconsistent). | **Pick one owner.** `app/document.rs` already handles the YAML/body split. Remove the top-level `document.rs` (or make it a thin re-export of `app::document::DocumentContent` if existing upstream callers exist). Update imports: `agent/`, `editor.rs`, and any caller to use `crate::app::document`. |

### P1 — Medium-High Impact

| # | Location | Problem | Techniques |
|---|----------|---------|------------|
| 4 | `agent/tools/context.rs` | **ToolContext violates SRP.** It is: *(a)* a dependency injector for `AppConfig` + `Bus`, *(b)* a VFS path resolver, *(c)* a `FileEventProducer` factory. It knows about three separate subsystems. This mixes read-resolve concerns (no side-effects) with write-publish concerns. | **Split into `VfsResolver` and `EventPublisher`.** Tools that only need path resolution take `&VfsResolver`; tools that publish events take `&EventPublisher` (or embed `FileEventProducer` directly). `ToolContext` becomes a convenience tuple struct only, with method forwarding. In the static `ToolRegistry::new()` case, pass injectable trait objects so tests can supply stubs. |
| 5 | `markdown::toc.rs` → `app::ToCEntry` | **Upward import.** `markdown/` is the only leaf-architecture module that imports from `app/`. If `ToCEntry` ever needs new fields (e.g. an icon enum, a dirty flag set by the watcher), the markdown parser must recompile, coupling two otherwise independent pipelines. | **Invert the dependency via a type alias / trait.** Either move `ToCEntry` into `markdown/` (it *is* a parse product — heading ID + level + text) and let `app/tab_manager.rs` import from `markdown::`, or introduce a `toc::Entry` trait that `app::ToCEntry` implements. Given `ToCEntry` derives from parsed headings, the first option is cleaner: `ToCEntry` lives where it is produced. |
| 6 | `crate::BackgroundMessage` enum | **Event bus as god enum.** `BackgroundMessage` carries LLM status, token usage, indexer progress, PDF conversion, image vision, watcher events — seven unrelated concerns in one enum. Any handler must match `_ => unreachable!()` or bloated arms. Every new background source greps this file. | **Replace the message enum with broadcast channels by topic.** Introduce `bus_router`-style typed channels (`LLMStatusRx`, `IndexerProgressRx`, …) or a `typed-broadcast` / `tokio::sync::broadcast` per domain. `BackgroundMessage` is a symptom of the `bus_router` module (`app/background/bus_router.rs`) not being used aggressively enough — the router should be the intermediary, not a monolithic enum. |

### P2 — Medium Impact, Incremental Improvement

| # | Location | Problem | Techniques |
|---|----------|---------|------------|
| 7 | `agent/tool_executor.rs` | **Safe/unsafe split is hardcoded in spec (`AGENT-012`), not self-describing.** Any new tool requires updating a non-local static list in the spec and the executor's `match` block. The executor violates Open/Closed. | **Add `Safety` to the `Tool` trait itself.** `trait Tool { fn safety(&self) -> Safety; execute(...); }` — each `Box<dyn Tool>` carries its classification. The executor iterates the registry, splits into two vectors, and dispatches without a match arm. Remove the hardcoded list from `AGENT-012` (kept only for backward compat). |
| 8 | Bridge `mod.rs` files (e.g., `background/mod.rs`, `tools/mod.rs`) | **Structural indirection without value.** Thin re-exports that re-route `crate::background` → `crate::app::background` add mental overhead for zero runtime cost. They exist to preserve `pub` paths during a prior move, but now they are permanent scaffolding. | **Deprecate then remove.** If no external crate consumes `crate::background::Foo` directly, collapse the bridge. Run `rg` across the repo to confirm call sites use the new paths, then delete the bridge `mod.rs`. If some callers remain, mark the re-export `#[deprecated(since = "...", note = "use crate::app::background::...")]` and gate removal behind a prompt-major bump. |
| 9 | `config/config.rs` re-exporting `Vfs` types | **Config module owns VFS behavior types.** `AppConfig` is pure data (good), but `ContentLibraryExt` lives in `app/vfs/` and is re-exported through `config/` for backward compat. This blurs the data/behavior separation the AGENTS.md explicitly calls out. | **Same deprecation path as (8).** Push a `cargo fix --edition`-style lint (custom `#[deprecated]`) in `config/config.rs` for the re-exported VFS symbols, updating call sites to import from `crate::app::vfs` directly. Then remove the re-export. |
| 10 | `utils/markdown.rs` | **Front-matter parser also lives in `app/document.rs`.** `utils::markdown::parse_front_matter` is a thin shim *and* `app/document.rs::DocumentContent::parse` delegates to it. Two entry points to the same parse logic. | **Consolidate.** `utils::markdown::parse_front_matter` should become `markdown::front_matter::parse(...)` (it *is* markdown format knowledge — the rule in AGENTS.md says format-aware helpers live in `markdown/`). Update `app/document.rs` to call `crate::markdown::front_matter::parse`. Remove or inline the utils shim. |

### P3 — Lower Priority / Good Practice

| # | Location | Problem | Techniques |
|---|----------|---------|------------|
| 11 | Error type proliferation | **`agent/error.rs` (`AgentError`), `tools/mcp/error.rs` (`McpError`), app-level `error.rs`.** Three error types for conceptually overlapping failure modes (LLM call, tool call, transport). Consumers do many `match e { … }` blocks because the types don't share a trait beyond `std::error::Error`. | **Define a sealed `FastmdError` trait with `source()` / `suggest_recovery()` and let each subsystem error implement it.** Alternatively, use `thiserror` (if not already present) with error codes so the UI can display actionable "retry / check config" suggestions from a single enum, using `#[from]` conversions for internal details. Goal: fewer `match` arms in `ui/app.rs`. |
| 12 | Stringly-typed tool names (`HashMap<String, Box<dyn Tool>>`) | **Tool lookup is `&str`-keyed and constructed at runtime.** Typos in `executor.rs` produce silent tool-not-found, not compile errors. Adding tools requires manual registration order. | **Consider a macro (`register_tool!`)** or a static `HashMap` built via `once_cell::sync::Lazy` for the static tool set. MCP-discovered tools remain dynamic, but built-in tools become a compile-checked set. |
| 13 | `ui/app.rs` as composition root | **Noted as a "god struct" in exploration.** This is *correct* for an egui App — it is the ports-and-adapters composition root. The risk is that new concerns (PDF vision, batch coordinator) are folded into `FastMdApp` instead of being wired through. | **Add a lint in code-review guidance**: any new field in `FastMdApp` requires a corresponding `app::` manager. If the concern can live in `app::`, it should. `FastMdApp.state.pdf_converter` belongs in a `BackgroundTaskHandle` inside `app::`, not directly in the UI struct. |

---

## Summary: Suggested Bounded Improvement Plan

| Severity | Action | Lowest-friction entry point |
|----------|--------|------------------------------|
| P0 | Split `ToolRegistry` so it no longer owns `McpClientManager` | Refactor `registry.rs` → `registry/` + `mcp_bridge.rs` |
| P0 | Remove agent-construction-time MCP ping | Move `init_mcp_on_startup` into an explicit `initialize_mcp()` call in `ui/app.rs` startup sequence |
| P0 | Merge duplicate `DocumentContent` models | Delete top-level `document.rs` re-exports, update 3–4 call sites |
| P1 | Decouple `ToolContext` into resolver + publisher | Split `context.rs`; update ~20 tool impl call-sites |
| P1 | Rehome `ToCEntry` to `markdown/` | Move struct + tests; update 3–4 consumers |
| P1 | Replace `BackgroundMessage` monolith with typed channels | Pilot in `app/background/bus_router.rs` with `LLMStatus` channel first |

---

## Techniques: When to Use What

| Scenario | Recommended Technique | Why |
|----------|----------------------|-----|
| Two subsystems share a mutable service instance (MCP client) | **Inversion via trait object / Arc-shared handle** — the consumer holds an interface, the producer owns the implementation | Removes transitive dependency chains (A owns B owns C) |
| A module needs network I/O at construction | **Lazy `Arc<Mutex>` + explicit `initialize()` async step**; do not perform side-effects in `new()` | Keeps unit test construction offline, obeys Single Responsibility |
| A God-Enum carrying many message shapes | **Typed broadcast channels** (`tokio::sync::broadcast`) or **domain-specific events** | Match-arms scale by domain, not by cross-cutting enum size |
| Upward import in leaf subsystem | **Move the upstream type to the leaf** (the leaf *produces* it) or use **type erasure** | Prevents the leaf from recompiling on internal changes to its consumer |
| File exceeds 400 LOC with ≥2 concerns | **Submodule directory** with `pub use` re-exports preserving the original path | Comply with the AGENTS.md target; zero breakage for downstream callers |
| Hardcoded classification lists in spec | **Add the classification to the sealed trait / enum itself** (Strategy pattern via `trait Safety { ... }`) | Open/Closed — adding a tool only requires implementing the trait, not editing a registry |

---

## Overall Verdict

The architectural skeleton is healthy. The most urgent refactoring is the `ToolRegistry` / `McpClientManager` lifecycle entanglement, because it simultaneously hurts testability, startup performance, and the Open/Closed Principle for every new tool family.

The project's conventions — bounded subsystems, thin-root facades, egui-free `app/`, spec-traceable requirements — provide a strong foundation for incremental improvement. The P0 items can land without breaking the public API (using `pub use` re-exports to preserve paths), and each delivers measurable gains in modularity, testability, or compile-time decoupling.
