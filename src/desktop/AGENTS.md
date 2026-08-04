# AI Agent Instructions — `src/desktop/` (`fastmd` crate)

These rules apply **only** to the `fastmd` Rust crate (the desktop app and its
`deploy` binary). The repo-root `AGENTS.md` provides the shared principles;
this file adds Rust/egui-specific conventions and the quality gate.

## 1. Documentation
- Every module must have a `//!` module-level doc comment.
- Start with a concise one-sentence summary, then add detail if needed.
- The first line (before any blank line) is used in search results and overviews — keep it short and descriptive.
- Every `pub` item (struct, enum, function, trait, type alias, const) must have a `///` doc comment.
- Include examples in doc comments where they clarify usage.
- Run `cargo doc --no-deps` to verify documentation builds without warnings.

## 2. Distilled SDK reference docs

`doc/distill/` holds distilled reference docs for the third-party SDKs this
crate depends on. Consult them before writing or reviewing code that touches
these areas; they are the in-repo source of truth for the pinned versions.

- `doc/distill/egui.md` — egui/eframe 0.35 reference (immediate-mode core,
  `Context`/`Ui`/`Response` API, widgets, containers, `emath`/`ecolor`/`epaint`).
  Consult when writing or modifying egui UI code (`ui/`, `editor.rs`, `main.rs`).
- `doc/distill/egui-kittest.md` — egui_kittest / kittest reference
  (AccessKit-based `Harness`, `Queryable`, `By` filters, `NodeT`). Consult when
  writing or maintaining egui UI tests.
- `doc/distill/mcp.md` — Model Context Protocol client-side spec (lifecycle,
  transports, OAuth 2.1 authorization, cancellation, ping, progress). Consult
  when working on the MCP client (`agent/tools/mcp/`).
- https://docs.discord.com/llms.txt for discord changes. You MUST retrieve the current copy of that document when making discord related changes.

## 4. Spec traceability
- Every user-facing behaviour maps to requirement in `SPEC.md`. When adding a feature, add the requirement first; when changing behaviour, prompt to update the requirement.
- Keep requirements goal oriented and user facing. Avoid leaking implementation specifics.
- Code may cite `REQ-xxx` / `MD-xxx` / `TOOL-xxx` / `AGENT-xxx` in `//!`/`///` comments where it aids review (e.g. `(AGENT-012)` next to the safe/unsafe split). Do not sprinkle citations inline in business logic.
- `ARCHITECTURE_C4.md` (in `doc/technical-context/`) is the authoritative architecture picture; update it when module boundaries change.

## 5. Folder structure

The crate is organised by **bounded subsystem**. Each directory owns a cohesive
concern and exposes its public API through a `mod.rs` that re-exports symbols,
so `lib.rs` stays a thin facade. The layout (files elided — see
`doc/technical-context/ARCHITECTURE_C4.md` for the full tree):

```
src/
├── lib.rs                  # facade: re-exports public API, no logic
├── main.rs                 # fastmd binary entry; eframe::run_native
├── error.rs                # AgentError
├── bin/deploy.rs           # deploy binary target
│
├── bus/                    # Messaging subsystem — transports, event payloads,
│   │                       # bus-side routing (see `doc/planning/.../bus-folder-consolidation.md`)
│   ├── core.rs             # Bus<T>, BusReader<T>  (tokio::sync::broadcast wrapper)
│   ├── events/             # every event payload that flows over a bus or channel
│   ├── router/             # bus-side plumbing
│   └── config.rs           # config_bus() constructor, CONFIG_ARRIVAL_TIMEOUT
├── config/                 # AppConfig + client structs, loader, secrets (data shapes only)
├── app/                    # egui-free application domain (managers, watcher, vfs)
│   ├── vfs/                # Virtual File System — parser, library behaviour, resolver (app/vfs/SPEC.md)
│   ├── watcher/            # FileWatcher (notify), FileEventProcessor, DirectoryTracker
│   └── (managers)          # TabManager, SelectionManager, DialogManager, PanelLayout, TagManager, TextBuffer
├── agent/                  # LLM tool-loop: manager, llm_client, prompt_builder,
│                           #   response_formatter, tool_executor, context, agent_impl
├── tools/                  # Tool trait, ToolContext, ToolRegistry, and every tool
│                           #   family (filesystem, web, jmap, csv_db, caldav,
│                           #   carddav, weather, yaml_header)
├── markdown/               # THE Markdown subsystem: parsing, rendering, document
│                           #   model, front-matter R/W, table-width algorithm
├── ui/                     # egui layer only: FastMdApp, PanelLayout, panels/,
│                           #   tree, tab/selection/dialog managers, modals,
│                           #   background_logs, os_shell
├── background/             # Worker pool — Indexer, PdfConverterWorker,
│                           #   ImageVisionWorker, ProcessManager
├── batch/                  # Batch prompt processing (coordinator, discoverer,
│                           #   executor, file_matcher, prompts, types)
├── editor.rs               # Inline egui text editor widget (calls markdown::)
├── tag_manager.rs          # TagManager (file_tags, all_tags, prompt_paths)
├── print.rs                # PrintJob (calls markdown::render for HTML)
├── browser.rs              # Playwright wrapper
└── utils/                  # Generic helpers only (path, tags) — NO Markdown knowledge
```

### Placement guidance for coding agents

When adding or moving code, place files by **concern**, not by type:

- **Anything that knows about Markdown as a format** (parsing, rendering,
  front-matter, document model, table layout) goes in `markdown/`. That
  subsystem is the single import point for `pulldown-cmark`, `gray_matter`,
  and Markdown AST types. `ui/`, `tools/`, `print.rs`, and `editor.rs` call
  into `markdown::` rather than handling Markdown themselves.
- **egui-dependent code** goes in `ui/` (or `editor.rs` for the text widget).
  `ui/` must not import `pulldown-cmark` directly — use `markdown::render`.
  Pure layout math with no egui dependency lives in `markdown/table_width/`,
  not `ui/`.
- **Tools** (anything implementing the `Tool` trait) go in `tools/` and are
  registered in `tools/registry.rs::register_all`. Group tool families into
  submodules (`tools/filesystem/`, `tools/jmap/`, `tools/csv_db/`, ...). A tool
  that wraps a Markdown concern (e.g. `read_yaml_header`) lives as a thin shim
  in `tools/` that delegates to `markdown/`; the Markdown logic itself does
  not live in `tools/`.
- **Agent loop concerns** (LLM client, prompt builder, response formatter,
  tool executor, session manager) go in `agent/`. Do not put tool
  implementations or UI code here.
- **Background workers** and the `Task` orchestrator go in `background/`.
  Channel/bus plumbing between workers and the UI belongs here, not in `ui/`.
- **Messaging primitives** (buses, event payload types, bus-side routing,
  generic channel-drain workers) go in `bus/`. Producers (indexer,
  watcher) live in `background/`; consumers that drive UI state live in
  `app/` or `ui/`. The `Bus<T>` transport is in `bus::core`; per-event
  payload types are in `bus::events::*`; the bus-side router and
  channel-drain helpers are in `bus::router::*`.
- **Configuration** (data types, loader, secret-redacting Debug impls)
  goes in `config/`. Keep `AppConfig` data-only; the `ContentLibrary`
  data type lives here but its behaviour lives in `app/vfs/`. The VFS
  parser, errors, and resolver live in `app/vfs/` (see
  `src/app/vfs/SPEC.md`); `config.rs` re-exports the public types for
  backwards compatibility, but new code should import from
  `crate::app::vfs`.
- **Generic, domain-free helpers** (path utilities, file-walk tag extraction)
  go in `utils/`. If a helper knows about Markdown, it belongs in `markdown/`,
  not `utils/`.
- **Cross-cutting value types** with no single home (e.g.
  `TokenUsageInfo`) live in `bus::events::messages` (value-type home)
  or `bus::events::typed` (per-domain
  replacement). The `app/` module no longer hosts these.

### Event-driven fan-out

Background work reaches the UI through **event-driven fan-out** on
`Bus<T>` broadcast buses (`bus::core`), never through per-widget
request/response binding:

- Long-running or background work runs on its own thread or worker and
  publishes results as events onto a `Bus<T>` bus; the UI never awaits a
  background future directly.
- The UI subscribes as a `BusReader` and drains events each frame with
  `try_recv()` (see `ui/app.rs`), calling `ctx.request_repaint()` when a
  frame needs to be drawn.
- One-off operations (e.g. an OAuth flow) may use a dedicated channel plus
  `ctx.request_repaint()` after sending, but still produce on a background
  thread and consume on the UI thread.
- Keep the bus contract as the single path for results flowing into the UI;
  do not add a parallel per-widget async-binding mechanism alongside it.

### Module size and splitting

- Target **≤ 4096 lines** per `.rs` file. When a file exceeds this, split by
  concern into a submodule directory (cf. `config/`, `tools/registry/`,
  `ui/app/`). Keep the original file as a `mod.rs` that re-exports the pieces
  so public paths are unchanged.
- `lib.rs` is a **facade only** — no logic, only `pub use` of subsystem
  public APIs. Do not grow it when adding features; add to the relevant
  subsystem and let `lib.rs` re-export.
- When extracting a submodule, refactor and update all external callers.

### Test sidecar files

When a source file's `#[cfg(test)] mod tests { ... }` block grows large enough
to materially affect cognitive load (rough rule: more than ~150 test lines, or
when the test block is more than half the file), extract the test body into a
sibling sidecar file and declare it from the source file:

```rust
// In <area>/foo.rs:
#[cfg(test)]
mod tests;  // sibling file: <area>/tests.rs (or <area>/foo_tests.rs)

// In <area>/tests.rs (or <area>/foo_tests.rs):
//! Tests for [`crate::area::foo`].
//! Lives in a sidecar so the implementation file stays focused.

use super::*;
```

The `mod tests;` declaration (no `#[path]` needed) makes the test file a child
of the implementation module, so `use super::*;` keeps working and private
items stay in scope. This is the **unit-test sidecar** pattern. It is
mechanical, preserves visibility, and is preferred over the alternatives below
whenever tests need to touch internal state.

Use **`tests/<name>.rs` (integration test)** instead of a sidecar when the test
should be exercised against the public API only — typically for algorithm
tests, public-contract regression tests, and black-box behaviour checks. The
sibling pattern is wrong in that case because it is too forgiving: a test that
only uses public items should be proven to do so.

**Header note requirement.** Whenever an implementation file has a test
sidecar, the implementation file's `//!` module doc comment must end with a
one-line pointer to the sidecar, in this form:

```rust
//! ...existing module doc...
//!
//! Unit tests live in the sibling `tests.rs` sidecar.
```

(If the sidecar is named `<foo>_tests.rs` rather than `tests.rs`, substitute
the actual filename.) This keeps the sidecar discoverable from the
implementation file and from any `cargo doc` output.

Existing examples to follow:

- `agent/agent_impl.rs` ↔ `agent/agent_impl_tests.rs`
- `agent/tools/manager/mod.rs` ↔ `agent/tools/manager/tests.rs` and
  `agent/tools/manager/group_tests.rs`
- `ui/tools_dialog.rs` ↔ `ui/tools_dialog_tests.rs`
- `agent/tools/browser.rs` ↔ `agent/tools/browser_tests.rs`
- `markdown/table_width/mod.rs` ↔ `tests/table_layout_test.rs` and
  `tests/table_visual_layout_test.rs` (integration-test variant)

### `app/` is egui-free

The `app/` module owns the application-domain types: managers
(`TabManager`, `SelectionManager`, `DialogManager`, `PanelLayout`,
`TagManager`), the file-watcher plumbing (`FileEventProcessor`,
`DirectoryTracker`, `FileWatcher`), the `ToCEntry`
data type, and the persisted UI struct (`PersistedUiState`).

The `Bus<T>` transport and all event payload types live in
[`crate::bus`]; `app/` does not define or re-export them.

- **No `egui` references.** No `.rs` file under `app/` may import
  `eframe::egui`, `egui`, or any other UI crate. Doc comments may
  mention `egui::Id` (e.g. to document how a stable string id
  maps to one at render time), but `use` statements must not.
  The module is unit-tested without driving the UI harness, so an
  `egui` dependency would couple those tests to the framework.
- **Stable identifiers are `String`s.** Anything the UI layer needs
  to address by id (`ToCEntry::id`, `TabManager::scroll_to_header_id`,
  `AgentState::scroll_to_id`) is stored as a `String` (or
  `Option<String>`). The UI layer converts to `egui::Id::new(&s)`
  at the boundary.
- **The UI layer adapts, the app layer doesn't.** `app/` exposes
  plain Rust types and behaviour; `ui/` is the only consumer that
  knows about `eframe::egui`. New application-domain types belong in
  `app/`; new rendering concerns belong in `ui/`.
- **Verification.** `cargo clippy --all-targets` does not catch
  accidental `egui` imports in `app/` (clippy is framework-agnostic).
  When adding code under `app/`, run
  `rg -n '^\s*use .*egui|^\s*use eframe' src/desktop/src/app` and
  confirm it returns nothing.

## 6. Quality Gate (Rust)

Before marking any task as complete, run the following from `src/desktop/` and ensure they all pass cleanly:
- `cargo check --quiet` — no errors or warnings
- `cargo nextest run --status-level fail --show-progress none` — all tests pass (the `default` profile in `.config/nextest.toml` retries flaky tier-4 click tests twice; CI uses the `ci` profile which is strict)
- `cargo clippy -- -D warnings` — no lint warnings (deny all)
- `cargo fmt --check` — code is properly formatted
- `cargo doc --no-deps --quiet` — documentation builds without warnings

The project uses [`cargo-nextest`](https://nexte.st/) for the test runner instead of the built-in `cargo test`. nextest runs each test in its own process, surfaces per-test timing, and gives flaky tier-4 click tests a chance to retry. Install with `cargo install cargo-nextest --locked`; the configuration lives in `src/desktop/.config/nextest.toml`.
