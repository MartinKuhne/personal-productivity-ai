# AI Agent Instructions — `src/desktop/` (`fastmd` crate)

## Tests
- [RUST-001] Unit tests SHOULD be kept in a separate file. The file MUST be named <file>_tests.rs.

## 1. Documentation
- [RUST-010] Every module must have a `//!` module-level doc comment containing a concise one-sentence summary of the module's purpose
- [RUST-011] Every `pub` item (struct, enum, function, trait, type alias, const) must have a `///` doc comment.

## 2. Distilled SDK reference docs

[RUST-020] You MUST consult this distilled reference documentation before writing or reviewing code 

- `doc/distill/egui.md` — egui/eframe reference (immediate-mode core,
  `Context`/`Ui`/`Response` API, widgets, containers, `emath`/`ecolor`/`epaint`).
  Consult when writing or modifying egui UI code (`ui/`, `editor.rs`, `main.rs`).
- `doc/distill/egui-kittest.md` — egui_kittest / kittest reference
  (AccessKit-based `Harness`, `Queryable`, `By` filters, `NodeT`). Consult when
  writing or maintaining egui UI tests.
- `doc/distill/mcp.md` — Model Context Protocol client-side spec (lifecycle,
  transports, OAuth 2.1 authorization, cancellation, ping, progress). Consult
  when working on the MCP client (`agent/tools/mcp/`).

## 3. Egui inspection and egui_mcp

[RUST-030] You MAY inspect a live running instance of the fastmd app by setting the EGUI_INSPECTION=1 environment variable then running the application. You can then use the egui_mcp tool to interact with the application.

## 4. Spec traceability
- [RUST-040] Every user-facing behaviour maps to a requirement in `SPEC.md`. When adding or changing a feature, you MUST point out any drift between implemented behaviour and code. You MUST NOT update the requirement unless asked to do so.
- [RUST-041] Requirements MUST be high level, goal oriented and user facing. Avoid leaking implementation specifics.
- [RUST-042] You SHOULD cite `REQ-xxx` in `//!`/`///` comments when making changes to the code.
- `ARCHITECTURE_C4.md` (in `doc/technical-context/`) is the authoritative architecture picture; You MUST update it when module boundaries or contracts change.

## 5. Folder structure
- [RUST-050] The crate is organised by **bounded subsystems**. Each directory SHOULD fully contain a cohesive
concern and expose its public API through a `mod.rs` that re-exports symbols.

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
│   ├── session/            # Shared runtime — BrowserSession (Playwright) and PdfBackingTracker,
│   │                       # the long-lived handles the orchestrator hands to the agent
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

- [RUST-051] When adding or moving code, place files by **concern**, not by type

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

You MUST use [`cargo-nextest`](https://nexte.st/) for the test runner