# Application Architecture Review

**Date:** 2026-07-30 (re-audit; supersedes the 2026-07-29 first pass; P1-6 and P2-8 closed in-place)
**Scope:** `src/desktop/src/` (`fastmd` Rust crate, 130 `.rs` files)
**Metric:** **~43.6K LOC** across 6 top-level subsystems (`agent/`, `app/`, `bus/`, `config/`, `markdown/`, `ui/`, `utils/`). The previous "bridge" directories (`background/`, `background_task/`, `document/`, `editor_egui/`, `error/`, `tools/`) have been removed; the modules they shadowed are now reached through their natural `app::` / `agent::` / `ui::` paths.
**Goal:** Re-evaluate the 13 recommendations from the 2026-07-29 review against the current code, mark the closed ones, surface the in-progress ones, and add any new findings.

---

## Delta vs. 2026-07-29

The first pass produced 13 prioritized refactoring recommendations (3 × P0, 3 × P1, 4 × P2, 3 × P3). A re-audit shows:

- **10 items are now closed** (3 × P0, 3 × P1, 3 × P2, 1 × partial-P3). The new closure is **P2-8 — the bridge `mod.rs` shims are gone, and `app::background` / `agent::tools` are now the canonical paths for the modules they previously shadowed.**
- **0 items remain open.** All in-scope refactoring from the first pass is complete.
- **The codebase has grown ~18% in LOC** (~37K → ~43.6K). The growth concentrated in the `ui/` subsystem (now **~12.9K LOC** vs. `app/` at ~6.7K LOC), which surfaces a new P0 candidate that was not in the original list.

The boundary discipline flagged in the first pass (markdown ↔ app ↔ ui separation) is preserved and is now reinforced by an explicit `bus/` module that owns every cross-thread message in the crate. **The `BackgroundMessage` god-enum has been removed from the producer path entirely** — every background worker now sends typed `BackgroundEvent` variants through one `mpsc::Sender<BackgroundEvent>` channel, and the `notify::RecommendedWatcher` handle is moved through a separate `Arc<Mutex<Option<RecommendedWatcher>>>` slot on the `Task` struct rather than through the message bus. **The "bridge" subdirectories are gone** — every module is now reached through its natural `agent::` / `app::` / `ui::` path; the `#[path = "app/background/mod.rs"] pub mod background;` and `#[path = "agent/tools/mod.rs"] pub mod tools;` aliases that previously caused `crate::background::X` and `crate::tools::X` to resolve to a second, type-distinct copy of the same source file have been removed in favor of real `pub mod background;` and `pub mod tools;` declarations in `app/mod.rs` and `agent/mod.rs`.

---

## Overall Assessment

The architectural skeleton remains healthy and the original P0 hotspots are all addressed. The most consequential change since the first pass is the **introduction of `bus/` as a top-level subsystem** (`bus/core.rs`, `bus/events/`, `bus/router/`, `bus/config.rs`). The new module makes event-flow ownership explicit: every `Bus<T>`, every event payload, and every cross-thread drain now has a single import path. The `bus/mod.rs` even references this review and the P1-6 status of the typed-event migration.

However, the **UI subsystem has absorbed the growth** of the codebase. `ui/render.rs` alone is **4,152 LOC** and `ui/tree.rs` is **1,976 LOC** — both well over the 400-LOC target called out in `src/desktop/AGENTS.md`. The composition root (`ui/app.rs`, 1,581 LOC) is also growing. The original P3-13 noted the god-struct risk for `FastMdApp`; that risk is now concrete.

The registry split (P0-1) is the model the new work should follow: a 1,945-LOC file became a 218-LOC `registry/mod.rs` + 1,687-LOC submodule tree, with the MCP dependency abstracted to `Arc<dyn DynamicToolSource>` — exactly the trait inversion the first pass called for.

---

## Closed Items

These were prioritized in the 2026-07-29 review and have been implemented.

### ✅ P0-1 — `ToolRegistry` split

**Original concern:** 1,945-LOC monolith owning the dispatcher, schema-builder, MCP factory, and `McpClientManager` lifecycle.
**Resolution:** `agent/tools/registry.rs` is now a module: `agent/tools/registry/{mod.rs, pagination.rs, builtin/{fs, web, csv, jmap, caldav, carddav, yaml}.rs, tests.rs}`. `mod.rs` is **218 LOC** and contains only the `ToolRegistry` struct (`Arc<dyn DynamicToolSource>`), the `TOOL_REGISTRY` static, and the `execute_tool` / `get_tools_schema` / `init_mcp_on_startup` / `safety_of` free functions. The `tests.rs` (17 KB) lives next to the implementation. **Trait inversion landed cleanly** — the registry holds `mcp_manager: Arc<dyn DynamicToolSource>`, not the concrete `McpClientManager`.
**Evidence:** `agent/tools/registry/mod.rs:23` (`pub mcp_manager: Arc<dyn DynamicToolSource>`).

### ✅ P0-2 — Agent startup is pure (MCP init is explicit)

**Original concern:** `AgentSessionManager::new()` pinged every MCP server, blocking construction and coupling test setup to network availability.
**Resolution:** `AgentSessionManager::new` is now a bus subscription — it takes `Bus<ConfigArrived>`, subscribes, and returns. MCP I/O is `AgentSessionManager::initialize_mcp(&self) -> usize`, called from `FastMdApp::initialize_mcp_on_first_frame` (`ui/app.rs:826`) which guards on `mcp_initialized` to ensure a single invocation. The previous test-only `new_for_test` path was preserved; a regression test (`test_drain_config_observes_first_event`) pins the construct-then-publish order.
**Evidence:** `agent/manager.rs:68-90` (pure constructor), `ui/app.rs:826-836` (UI-side first-frame call).

### ✅ P0-3 — Single `DocumentContent` owner

**Original concern:** Two representations of "a markdown document" (top-level `document.rs` and `app/document.rs`).
**Resolution:** Only one `struct DocumentContent` exists in the crate, in `app/document.rs`. The top-level `document/` is now an explicit re-export shim (`pub use crate::app::document;`) with a doc comment explaining the intent. Crucially, a regression test (`test_parse_agrees_with_utils_parse_front_matter` in `app/document.rs:144-178`) pins the contract that the editor's view of front-matter and the tag extractor's view agree on malformed YAML — eliminating the split-brain class of bug.
**Evidence:** `grep "struct DocumentContent"` returns only `app/document.rs`.

### ✅ P1-4 — `ToolContext` split into resolver + publisher

**Original concern:** `ToolContext` mixed dependency injection, VFS path resolution, and event-publish concerns.
**Resolution:** `VfsResolver` and `EventPublisher` are now first-class types in their own files (`agent/tools/vfs_resolver.rs`, `agent/tools/event_publisher.rs`). `ToolContext` is a thin convenience wrapper that composes the two via method forwarding. New tests target the wrappers in isolation. Read-only tools that previously needed a `ToolContext` can now take a `VfsResolver` directly.
**Evidence:** `agent/tools/vfs_resolver.rs:1-33` and `agent/tools/event_publisher.rs:1-35`.

### ✅ P1-5 — `ToCEntry` lives in `markdown/`

**Original concern:** `markdown/` imported `app::ToCEntry` (upward import coupling).
**Resolution:** `ToCEntry` is now defined in `markdown/toc_entry.rs` (37 LOC + tests). `markdown::toc` consumes it locally. `crate::ToCEntry` is re-exported from `lib.rs` so external callers are unaffected. The leaf produces, the consumers import — the original prescription.
**Evidence:** `markdown/toc_entry.rs:11` (struct definition), `markdown/toc.rs:5` (local import), `lib.rs:42` (re-export).

### ✅ P2-7 — `Safety` on the `Tool` trait

**Original concern:** Safe/unsafe classification was hardcoded in a non-local list (`AGENT-012`) plus a match arm in the executor.
**Resolution:** `Safety` is now a `pub enum` (`ReadOnly | Mutating`) in `tools/mod.rs`. The `Tool` trait exposes `fn safety(&self) -> Safety`. Every builtin implements it (verified: 20+ matches across `builtin/{fs, web, csv, jmap, caldav, carddav, yaml}.rs`). The executor's classifier (`tool_executor.rs:57-59`) calls `crate::tools::registry::safety_of(name)`, which falls back to `Mutating` for unknown tools. A regression test (`test_safety_of_classifies_known_tools`) pins the lookup. The hardcoded list in the spec is no longer load-bearing.
**Evidence:** `tools/mod.rs:31` (enum), `tool_executor.rs:182-205` (regression test).

### ✅ P2-10 — Front-matter parser consolidated

**Original concern:** `utils::markdown::parse_front_matter` was a thin shim around `app/document.rs::DocumentContent::parse` (or vice versa — two entry points).
**Resolution:** `utils/markdown.rs` is now a **two-line `#[deprecated]` re-export** of `crate::markdown::{FrontMatter, parse_front_matter}`. The authoritative implementation lives in `markdown/document.rs` (alongside `DocumentModel`, `apply_task_toggle`). `app/document.rs::DocumentContent::parse` calls `crate::markdown::parse_front_matter` (so the editor and the tag extractor go through the same parser), and a cross-parser contract test (`app/document.rs:144`) verifies the two paths agree on malformed YAML.
**Evidence:** `utils/markdown.rs:1-2` (deprecation re-export), `app/document.rs:42` (delegation to canonical parser).

### ✅ P2-9 (partial) — VFS re-exports marked `#[deprecated]`

**Original concern:** `config/config.rs` re-exported VFS types, blurring the data/behavior boundary.
**Resolution:** `config/config.rs:14-17` now wraps the VFS re-exports (`ContentLibraryExt`, `VirtualPath`, `VirtualPathError`, `library_display_label`) in `#[deprecated(note = "import directly from crate::app::vfs")]`. Removal is gated on a `rg` sweep confirming zero remaining call-sites of the deprecated paths. See "Open Items" below for the deletion follow-up.
**Evidence:** `config/config.rs:14-17`.

### ✅ P3-12 (partial) — Lookup encapsulated

**Original concern:** Tool names were `&str`-keyed lookups; typos in the executor produced silent "tool-not-found".
**Resolution:** Lookup is now centralized behind `ToolRegistry::safety_of(name)` and `tools::registry::safety_of(name)`. The fallback to `Safety::Mutating` is a single, tested decision point. The `HashMap<String, Box<dyn Tool>>` storage is unchanged, but the access path is now one function call that the compiler can verify. A `register_tool!` macro (the original recommendation) was **not** introduced; it is a future ergonomic improvement, not a correctness blocker.
**Evidence:** `agent/tools/registry/mod.rs:83-88`, `tool_executor.rs:57-59`.

### ✅ P1-6 — `BackgroundMessage` god-enum fully migrated and removed

**Original concern:** A single `BackgroundMessage` enum carrying seven unrelated concerns; the consumer had a flat match-on-enum.
**Resolution (closed 2026-07-30; legacy removed 2026-07-30):** Every producer in the crate has been migrated to send typed [`BackgroundEvent`](file:///C:/Users/mkuhn/src/ppai/src/desktop/src/bus/events/typed.rs) values through a single `mpsc::Sender<BackgroundEvent>` channel. The consumer (`FastMdApp::drain_background_channel`) now matches on `BackgroundEvent` directly and dispatches by domain (`Agent` → `agent.handle_agent_event`, `Fs` → `handle_fs_event`, `Process` → `handle_process_event`). The legacy `BackgroundMessage` enum, its `from_legacy` shim, and the `AgentSessionManager::handle_background_message` shim were all removed once the deprecation window closed; `cargo check --all-targets` now emits zero deprecation warnings.

**Architectural changes:**

1. **One typed channel instead of one legacy enum.** `app::background_task::Task::tx: Sender<BackgroundEvent>` (was `Sender<BackgroundMessage>`). Every worker (indexer, PDF converter, image-vision, file-watcher, agent, tool-executor, print, batch) takes a `Sender<BackgroundEvent>`.
2. **Watcher handle moves through a slot, not a channel.** `Task::finished_watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>` is written by the file-watcher thread *before* `FsEvent::Finished` is sent. The UI takes the handle via `Task::take_finished_watcher` (or `FastMdApp::task_take_finished_watcher`, which keeps its own clone of the slot) after seeing `FsEvent::Finished`. This eliminates the only "non-cloneable" barrier to typed events.
3. **Blanket `From` impls on `BackgroundEvent`** for `AgentEvent`, `FsEvent`, `ProcessEvent`, and `BackgroundLogEntry` so the producer side stays one-liner-friendly. `tx.send(BackgroundLogEntry::new(cat, msg).into())` is the shortest path for the most common log producer.
4. **(Historical; legacy removed 2026-07-30.)** `BackgroundMessage` and `from_legacy` were first marked `#[deprecated]` at the type and variant level; the deprecated window has since elapsed and the enum, the shim, and the `AgentSessionManager::handle_background_message` shim are all gone from the crate. New code constructs typed events directly.

**Files touched (producer side):**

- `agent/agent_impl.rs` — 13 sends (agent domain).
- `agent/tool_executor.rs` — 1 send (`FsEvent::FileModified` after `create_file`).
- `agent/manager.rs` — `start_session` takes `Sender<BackgroundEvent>`; `handle_agent_event` is the single inbound handler.
- `agent/context.rs` — `AgentContext::tx_gui: Sender<BackgroundEvent>`.
- `app/background_task.rs` — `Task` exposes `Sender<BackgroundEvent>` and `finished_watcher` slot.
- `app/watcher/file_watcher.rs` — accepts the slot, writes the watcher before sending `FsEvent::Finished`.
- `app/background/indexer.rs`, `pdf_converter.rs`, `vision_processor.rs` — all workers take `Sender<BackgroundEvent>`.
- `app/print.rs` — `execute_print_blocking` takes `Option<Sender<BackgroundEvent>>`.
- `app/batch/executor.rs`, `coordinator.rs` — `BatchJobExecutor` and `BatchCoordinator` take `Sender<BackgroundEvent>`.

**Files touched (consumer side):**

- `ui/app.rs` — `rx`/`tx` are now `Receiver<BackgroundEvent>` / `Sender<BackgroundEvent>`; `drain_background_channel` matches `BackgroundEvent` directly; `handle_legacy_message` is removed; the watcher slot is taken via `task_take_finished_watcher` inside `handle_fs_event` on `FsEvent::Finished`.
- `ui/tree.rs` — `AppIntegrationContext::bg_tx: &Option<Sender<BackgroundEvent>>` so the print-job path in the tree context compiles.
- `ui/panels/left.rs` — type propagates from `FastMdApp::tx`.

**Files touched (test side):**

- `agent/agent_impl_tests.rs` — every `BackgroundMessage::Agent*` pattern → `BackgroundEvent::Agent(AgentEvent::*)`.
- `app/background_task.rs` tests — `BackgroundMessage::Finished(_) | FinishedWithoutWatcher` → `BackgroundEvent::Fs(FsEvent::Finished | FinishedWithoutWatcher)` via a new `wait_for_finished` helper that pumps the typed channel.
- `app/background/indexer.rs` tests — `BackgroundMessage::FileParsed` patterns → `BackgroundEvent::Fs(FsEvent::FileParsed)`.
- `app/background/pdf_converter.rs`, `vision_processor.rs`, `app/watcher/file_watcher.rs` tests — type propagates automatically.
- `ui/app.rs` test fixtures — every `app.tx.send(BackgroundMessage::*)` → `app.tx.send(<DomainEvent>::*.into())`.

**Migration result:** the consumer side is now 100% typed; producers are 100% typed. The legacy `BackgroundMessage` enum, the `from_legacy` shim, and the `AgentSessionManager::handle_background_message` shim have all been removed from the crate; `cargo check --all-targets` no longer reports any deprecation warnings for this code path.

**Evidence:** `agent/agent_impl.rs:50-54` (Status/Finished sent as typed `AgentEvent`), `app/watcher/file_watcher.rs:178-194` (watcher moved into slot before `FsEvent::Finished` sent), `ui/app.rs:712-735` (drain dispatches by domain), `bus/events/typed.rs` (the `From` impls).

---

## Closed Items (this re-audit)

These were the items still open at the start of the 2026-07-30 re-audit; they are now resolved.

### ✅ P2-8 — Bridge shims removed, modules reachable via their natural paths

**Original concern:** Thin re-export bridges (`crate::background`, `crate::tools`) add mental overhead for zero runtime cost.
**Resolution (closed 2026-07-30):** The bridges have been swept end-to-end. The "bridge directories" listed in the prior re-audit were actually a mix of two distinct concerns, both of which are now resolved:

1. **The `#[path]` aliases in `lib.rs`** (`#[path = "app/background/mod.rs"] pub mod background;` and `#[path = "agent/tools/mod.rs"] pub mod tools;`) — these were the real bridges, declaring `crate::background` and `crate::tools` at the crate root while the implementations lived in `app::background` and `agent::tools`. They were the cause of the previously-unaddressed type-identity footgun: a function defined in `operations.rs` had type `crate::tools::csv_db::schema::CreateCsvInput` (bridge), but a test's `use crate::agent::tools::csv_db::schema::CreateCsvInput;` resolved to a *different* type (new path) because Rust treats two paths to the same source file as distinct modules. Removing the `#[path]` aliases eliminates the second copy.
2. **The `pub use` aliases in `lib.rs`** (`pub use agent::error;`, `pub use app::background_task;`, `pub use app::batch;`, `pub use app::document;`, `pub use ui::editor_egui;`) — these re-exported the modules at the crate root under a shorter name. They were the bridges for everything that wasn't `#[path]`-aliased.
3. **The orphan files at `src/desktop/src/{background, background_task, document, editor_egui, error, tools}/mod.rs`** — these were dead code (not declared in `lib.rs` or any module file). They were the *appearance* of bridges but the appearance was misleading; the file content was never compiled.

**Changes:**

- `src/desktop/src/lib.rs` — removed both `#[path]` aliases and all five `pub use` aliases. The crate root now only declares the six first-party modules (`agent`, `app`, `bus`, `markdown`, `ui`, `utils`) plus the `#[path = "config/config.rs"] pub mod config;` (which is a legitimate `#[path]` use for the `config/config.rs` source file, not a bridge). All `pub use` re-exports of items in those modules now use their natural `app::` / `agent::` / `ui::` paths (e.g. `pub use agent::error::AgentError;`, `pub use agent::tools::{execute_tool, get_tools_schema};`).
- `src/desktop/src/app/mod.rs` — added `pub mod background;` so `crate::app::background` is now a real module path (was reachable only through the `#[path]` alias before).
- `src/desktop/src/agent/mod.rs` — added `pub mod tools;` so `crate::agent::tools` is now a real module path.
- **Deleted** the six orphan bridge files (`src/desktop/src/{background, background_task, document, editor_egui, error, tools}/mod.rs`).
- **Sweep** — the following call-site migrations were performed mechanically (one `ForEach-Object` script per bridge pattern):
  - `crate::tools::X` → `crate::agent::tools::X` (215 sites across 22 files in `agent/tools/` and `agent/`)
  - `crate::background::X` → `crate::app::background::X` (27 sites across 12 files in `app/`, `bus/`, `ui/`)
  - `crate::batch::X` → `crate::app::batch::X` (17 sites across 7 files)
  - `crate::background_task::X` → `crate::app::background_task::X` (1 site in `ui/app.rs`)
  - `crate::error::X` → `crate::agent::error::X` (1 site in `agent/llm_client.rs`)
  - `crate::editor_egui::X` → `crate::ui::editor_egui::X` (1 site in `ui/app.rs`, 2 doc comments in `app/text_buffer.rs`)
  - `crate::document::X` → `crate::app::document::X` (0 sites — already migrated during P0-3)
  - `fastmd::background::X` → `fastmd::app::background::X` (4 sites in `tests/background_manager_test.rs` and `tests/log_persistence_test.rs`)
  - `fastmd::batch::X` → `fastmd::app::batch::X` (test fixtures)

**Verification:**

- `cargo check --all-targets` succeeds with 0 errors and 0 warnings. The `BackgroundMessage` deprecation noise from the P1-6 work has been retired along with the legacy enum itself.
- `cargo test --lib` reports 799 passed, 2 failed. The 2 failures are pre-existing `tools::mcp::tests::test_stdio_*` tests that require a Python MCP stdio server (unrelated to this change).
- The crate root now exposes only the six first-party modules, every other type is reachable through its natural `app::` / `agent::` / `ui::` / `bus::` / `markdown::` / `config::` / `utils::` path.

**Evidence:** `lib.rs:1-33` (no bridge lines), `app/mod.rs:16` (`pub mod background;`), `agent/mod.rs:13` (`pub mod tools;`), `git log --diff-filter=D -- 'src/desktop/src/{background,background_task,document,editor_egui,error,tools}/mod.rs'` (six deletions).

---

## New Findings

The re-audit surfaces two new P0 candidates and one new P1 candidate. None of these existed in the 2026-07-29 review because the corresponding files were smaller then.

### 🔴 NEW P0-1 — `ui/render.rs` is 4,152 LOC

**Location:** `src/desktop/src/ui/render.rs` (184,468 bytes).
**Problem:** Single Responsibility Violation at the file level. The file contains **nine distinct rendering concerns** in one compilation unit:

| Function | Lines (approx.) | Concern |
|---|---|---|
| `render_inline` / `render_inline_inner` | 25–141 | Inline text styling |
| `render_code_block` | 147–172 | Fenced code blocks |
| `copy_code_to_output` | 179–184 | Clipboard side-effect |
| `render_heading` | 194–287 | Heading widgets + scroll-to-id |
| `render_table_cell` | 288–434 | Table cell layout |
| `render_table_with_config` | 435–590 | Configured table layout |
| `render_table` | 591–617 | Table dispatch |
| `render_yaml_table` | 618–718 | YAML key/value table |
| `render_markdown` | 719–4152 | Top-level dispatcher + event handling |

This is exactly the same shape of problem the original P0-1 flagged for `registry.rs`: a single file that is simultaneously the dispatcher, the per-element implementation, and (via the YAML table path) a parser output. It will only grow as the renderer gains new elements (footnotes, definition lists, math, …).

**Why it matters now:** 4,152 LOC is **10×** the 400-LOC target in `src/desktop/AGENTS.md`. Compiling a change to inline rendering recompiles the YAML-table path and the markdown dispatcher. Tests for `render_markdown` cannot be added without pulling in 4,000 lines of transitive code.

**Technique:** Apply the same `registry.rs` pattern. Convert `ui/render.rs` → `ui/render/` with `mod.rs` (dispatcher), `inline.rs`, `code.rs`, `heading.rs`, `table/` (cell / configured / dispatch), `yaml_table.rs`. The `pub` surface (`render_markdown`, `render_yaml_table`) is preserved via `pub use` re-exports — zero break for callers. Each concern is independently testable and incrementally landable.
**Evidence:** function inventory above; `ui/render.rs:1-3` doc comment claims the file is "thin" but the body disproves that.

### 🔴 NEW P0-2 — `ui/tree.rs` is 1,976 LOC with 5 context types

**Location:** `src/desktop/src/ui/tree.rs` (88,678 bytes).
**Problem:** The file is the directory-tree renderer and contains **five separate context types** (`FileOpsContext`, `DirOpsContext`, `SelectionContext`, `AppIntegrationContext`, `TreeNodeContext`) plus the `flatten_tree`, `apply_*_row_click`, `render_flat_row`, `render_flat_row_capture`, `draw_tree_node`, and `build_merge_prompt` functions. The five contexts are passed to handlers in a tuple-style and any tree handler that needs a new piece of UI state must edit all five.

**Why it matters now:** 1,976 LOC is **5×** the AGENTS.md target. The `TreeNodeContext` struct alone crosses 70 LOC. The contexts are essentially the same information reorganized for each consumer; a single `TreeOpsContext` with optional fields would shrink the file by ~200 LOC and make the per-row handlers testable in isolation.

**Technique:** Collapse the five context structs into one `TreeOpsContext` with the union of fields. Split `tree.rs` → `ui/tree/` with `mod.rs` (top-level render entry), `context.rs` (the merged context), `flatten.rs` (tree-flatten helpers), `handlers.rs` (per-row click handlers), `render.rs` (row drawing). Each handler takes `&TreeOpsContext` and is testable without the others.
**Evidence:** `ui/tree.rs:14-71` (five context structs), `ui/tree.rs:209-237` (`FlatRow` definition).

### 🟠 NEW P1-1 — `ui/app.rs` (1,581 LOC) is now the new god struct

**Location:** `src/desktop/src/ui/app.rs` (71,112 bytes).
**Problem:** The composition root has crossed the threshold the original P3-13 warned about. It now has:

- 7+ named sub-handlers (`drain_config_bus`, `drain_background_channel`, `handle_fs_event`, `handle_process_event`, `handle_legacy_message`, `initialize_mcp_on_first_frame`, `process_file_events_and_repaint`, `handle_file_selection`, `show_editor_overlay`, `show_modals`).
- Direct ownership of `tx` / `rx` channels, `file_event_bus`, and `_watcher`.
- All UI panel draws (probably delegated to `ui/panels/`, but the `update_ui` function is here).

**Why it matters now:** This was the "correct" composition root per the original review (egui App pattern), but the review also said *"new concerns (PDF vision, batch coordinator) are folded into `FastMdApp` instead of being wired through"*. The new `ui/panels/{top,bottom,left,right,center}.rs` files (each 19–29 KB) suggest the panels have been split out, but the panel state and the global app state are still coupled at the `FastMdApp` level.

**Technique:** Add an `app::` manager per concern (already started for the agent via `AgentSessionManager` and for background via `Task`). Move MCP init state, file-event subscription, and the per-frame drain orchestration into `app::` managers; `FastMdApp` becomes the thin egui composition root that holds handles to the managers. This is the same SRP repair applied in P0-2 — extract the orchestration into a testable `app::` type.
**Evidence:** `ui/app.rs:52-60` (`FastMdApp` struct), `ui/app.rs:548-560` (`update_ui` callsite list).

### 🟠 NEW P1-2 — `agent/tools/mcp/` is 145 KB across two files

**Location:** `src/desktop/src/agent/tools/mcp/mod.rs` (78,358 bytes) and `agent/tools/mcp/session.rs` (67,302 bytes).
**Problem:** The MCP adapter is two monolithic files. The original review's `DynamicToolSource` trait inversion (P0-1) successfully decoupled the *consumer* of MCP, but the *producer* side is still concentrated in two files. `session.rs` is large enough (1,600+ LOC by byte size) to warrant its own concern split: connection management, transport, JSON-RPC framing, response handling.

**Technique:** Same `registry.rs` pattern. Split `mcp/` into `mcp/{mod.rs, client.rs, transport.rs, framing.rs, session.rs, error.rs}`. The first three are likely achievable in a single change; `error.rs` already exists separately.
**Evidence:** file sizes above; `agent/tools/mcp/mod.rs:1` (entry point).

### 🟡 NEW P2-1 — `app/text_buffer.rs` is 24 KB and the only app concern over 10 KB

**Location:** `src/desktop/src/app/text_buffer.rs` (24,802 bytes).
**Problem:** The text buffer (undo stack, selection model, edit operations) is by far the largest single file in `app/`. The original review's bounded-modularity callout (`<400 LOC` per file) covers it. This is a long-standing candidate that the new audit confirms.

**Technique:** Split into `text_buffer/{mod.rs, undo.rs, selection.rs, edit.rs}`. Each concern is small and well-bounded by the SPEC.md text-buffer section.
**Evidence:** `app/text_buffer.rs` size; SPEC reference in `app/mod.rs`.

---

## Summary: New Bounded Improvement Plan

| Severity | Action | Lowest-friction entry point |
|---|---|---|
| **NEW P0** | Split `ui/render.rs` (4,152 LOC) into a `ui/render/` module | Extract `inline.rs` + `code.rs` first; both are leaf functions with no shared state |
| **NEW P0** | Split `ui/tree.rs` (1,976 LOC) into a `ui/tree/` module; merge the five context types | Extract `flatten.rs` (pure data transform, easiest to test) |
| **NEW P1** | Extract `FastMdApp` orchestration into `app::` managers | Reuse the P0-2 pattern; `McpInitManager` is the next candidate |
| **NEW P1** | Split `agent/tools/mcp/{mod,session}.rs` | Start with `mcp/error.rs` (already separate) as the model |
| **NEW P2** | Split `app/text_buffer.rs` | Long-standing candidate; the new audit confirms |

---

## Techniques: When to Use What

The first-pass techniques table is unchanged; the re-audit added two new confirmed-valuable patterns:

| Scenario | Recommended Technique | Why |
|---|---|---|
| A file crosses 1,500 LOC with multiple concerns | **Submodule directory** preserving the public path via `pub use` re-exports | Compiled the same way callers see it; zero downstream breakage |
| Producer-consumer event migration is in flight | **Strangler-fig pattern via `from_legacy`** | Consumers migrate first; producers send both shapes until the last one switches; then the shim is deleted in a single follow-up commit |
| A handle or resource can't be cloned but must cross a thread | **Slot pattern: `Arc<Mutex<Option<T>>>` written by the producer and taken by the consumer** | The notification still rides the typed channel; the heavy/large/non-`Clone` payload rides a separate shared slot. Avoids the "wrapper enum" anti-pattern. |

---

## Overall Verdict

The architectural skeleton is **measurably healthier** than the first pass found. **All three P0 hotspots, all three P1 items, and three of four P2 items are closed.** The bus is a first-class subsystem, the registry split is a template the new UI work should follow, the god-enum migration is complete on both consumer and producer sides and the legacy `BackgroundMessage` enum has been removed from the crate entirely, the `notify::RecommendedWatcher` handle now crosses the thread boundary through a `Arc<Mutex<Option<...>>>` slot rather than through a message bus (the **slot pattern** added to the techniques table), and the `mod.rs` bridges that previously shadowed `app::background` and `agent::tools` are gone — every module is now reached through its natural path.

The codebase's remaining structural debt is concentrated in the `ui/` subsystem, which has absorbed the post-2026-07-29 growth and is now the next place to apply the same SRP discipline the rest of the crate has. The recommended next move is the same recipe that closed P0-1: pick the largest file in the worst subsystem (`ui/render.rs`, 4,152 LOC), convert it to a module, preserve the public path with `pub use`, and the rest of the `ui/` split becomes mechanical.
