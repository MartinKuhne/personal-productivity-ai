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
  when working on the MCP client (protocol layer in
  `integrations/mcp/`, tool-loop adapter in `agent/tools/mcp/`).

## 3. Egui inspection and egui_mcp

[RUST-030] You MAY inspect a live running instance of the fastmd app by setting the EGUI_INSPECTION=1 environment variable then running the application. You can then use the egui_mcp tool to interact with the application.

## 3. Egui inspection and egui_mcp

This release includes a new inspection protocol for egui. It allows reading the accesskit tree of a running app, as well as sending events to control it. It's implemented via a new InspectionPlugin in the egui_inspection crate.
Eframe includes a new inspection feature. When enabled, you can enable inspection by launching the app with `EGUI_INSPECTION=1`. This will cause the app to listen on port 5719.

The first inspection protocol consumer is egui_mcp.
It's a mcp server that allows your agent to see and use egui apps. It can be used to have the agent use the app, reproduce bugs and verify its changes.
Install it via `cargo install --git https://github.com/rerun-io/kittest_inspector egui_mcp` and then add it to your agent via `claude mcp add egui egui-mcp`.

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
├── bin/                    # Binary targets (deploy, etc.)
├── bus/                    # Messaging subsystem — transports, event payloads,
│                           # bus-side routing
│   ├── events/             # Event payloads that flow over a bus or channel
│   └── router/             # Bus-side plumbing
├── config/                 # AppConfig + client structs, loader, secrets (data shapes only)
├── app/                    # egui-free application domain (managers, watcher, vfs)
│   ├── vfs/                # Virtual File System — parser, library behaviour, resolver
│   ├── watcher/            # FileWatcher (notify), FileEventProcessor, DirectoryTracker
│   ├── session/            # Shared runtime — BrowserSession (Playwright) and PdfBackingTracker
│   ├── browser/            # Playwright wrapper
│   ├── background/         # Worker pool — Indexer, PdfConverterWorker,
│   │                       # ImageVisionWorker, ProcessManager
│   └── batch/              # Batch prompt processing (coordinator, discoverer,
│                           # executor, file_matcher, prompts, types)
├── agent/                  # LLM tool-loop: manager, llm_client, prompt_builder,
│                           # response_formatter, tool_executor, context, agent_impl
│   └── tools/              # Tool trait, ToolContext, ToolRegistry, and every tool
│       ├── csv_db/         # CSV database tool family
│       ├── jmap/           # JMAP email client tool family
│       ├── manager/        # Tool manager and registry
│       └── mcp/            # MCP tool-loop glue (McpToolAdapter)
├── integrations/           # External service integrations
│   └── mcp/                # MCP client protocol (transports, sessions, OAuth 2.1)
├── markdown/               # THE Markdown subsystem: parsing, rendering, document
│                           # model, front-matter R/W, table-width algorithm
│   └── table_width/        # Table width calculation algorithm
├── ui/                     # egui layer only: FastMdApp, PanelLayout, panels/,
│                           # tree, tab/selection/dialog managers, modals,
│                           # background_logs, os_shell
│   ├── app/                # Main app UI logic
│   ├── panels/             # UI panels
│   ├── render/             # Rendering utilities
│   ├── table_width/        # Table width UI components
│   ├── test_helpers/       # UI test helpers
│   └── tree/               # Tree view components
└── utils/                  # Generic helpers only (path, tags) — NO Markdown knowledge
```

- [RUST-051] When adding or moving code, place files by **concern**, not by type
- [RUST-052] **Event-driven fan-out.** Background work MUST reach the UI through event-driven fan-out on `Bus<T>` broadcast buses (`bus::core`). Long-running work MUST run on its own thread or worker and publish results as events onto a `Bus<T>` bus. The UI MUST NOT await a background future directly. The UI MUST subscribe as a `BusReader` and drain events each frame with `try_recv()` (see `ui/app.rs`), calling `ctx.request_repaint()` when a frame needs to be drawn. One-off operations MAY use a dedicated channel plus `ctx.request_repaint()` after sending, but MUST still produce on a background thread and consume on the UI thread. The bus contract MUST remain the single path for results flowing into the UI. Do not add a parallel per-widget async-binding mechanism.
- [RUST-053] **Module size limit.** Each `.rs` file MUST NOT exceed 4096 lines. When a file exceeds this limit, split by concern into a submodule directory (cf. `config/`, `tools/registry/`, `ui/app/`). Keep the original file as a `mod.rs` that re-exports the pieces so public paths are unchanged.
- [RUST-054] **Facade-only `lib.rs`.** `lib.rs` MUST be a facade only — no logic, only `pub use` of subsystem public APIs. Do not grow `lib.rs` when adding features; add to the relevant subsystem and let `lib.rs` re-export.
- [RUST-055] **Submodule extraction.** When extracting a submodule, you MUST refactor and update all external callers.
- [RUST-056] **Test sidecar extraction.** When a source file's `#[cfg(test)] mod tests { ... }` block exceeds ~150 lines or more than half the file, extract the test body into a sibling sidecar file. Declare it from the source file with `#[cfg(test)] mod tests;`. The sidecar file MUST be named `tests.rs` or `<foo>_tests.rs`. Use `tests/<name>.rs` (integration test) instead of a sidecar when the test should exercise only the public API.
- [RUST-057] **Sidecar header note.** When an implementation file has a test sidecar, the implementation file's `//!` module doc comment MUST end with a one-line pointer: `//! Unit tests live in the sibling \`tests.rs\` sidecar.` (substitute actual filename if different).
- [RUST-058] **`app/` is egui-free.** No `.rs` file under `app/` MUST import `eframe::egui`, `egui`, or any other UI crate. Doc comments MAY mention `egui::Id` but `use` statements MUST NOT. Stable identifiers that the UI layer addresses by id MUST be stored as `String` (or `Option<String>`). The UI layer converts to `egui::Id::new(&s)` at the boundary. New application-domain types MUST go in `app/`; new rendering concerns MUST go in `ui/`. When adding code under `app/`, run `rg -n '^\s*use .*egui|^\s*use eframe' src/desktop/src/app` and confirm it returns nothing.

## 6. Quality Gate (Rust)

Before marking any task as complete, run the following from `src/desktop/` and ensure they all pass cleanly:
- `cargo check --quiet` — no errors or warnings
- `cargo nextest run --status-level fail --show-progress none` — all tests pass (the `default` profile in `.config/nextest.toml` retries flaky tier-4 click tests twice; CI uses the `ci` profile which is strict)
- `cargo clippy -- -D warnings` — no lint warnings (deny all)
- `cargo fmt --check` — code is properly formatted
- `cargo doc --no-deps --quiet` — documentation builds without warnings

You MUST use [`cargo-nextest`](https://nexte.st/) for the test runner