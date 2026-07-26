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

## 2. egui Best Practices
- Follow egui's recommended patterns and idioms when building UI.
- Follow egui testing best practices (e.g. using `egui::__run_test_ctx` / `State::test_ctx` style harnesses as appropriate) for deterministic UI tests. `egui_kittest` (already a dev-dep) is the preferred harness for snapshot tests.
- Keep `update` methods side-effect free where possible and avoid storing unnecessary state in `egui::Id`s.
- The 5-pane layout is owned by `ui::panel_layout::PanelLayout` (REQ-101); do not ad-hoc side panels in `FastMdApp::update`.
- All cross-cutting state lives on `FastMdApp` (`ui/app.rs`); split new UI concerns into a dedicated manager struct (cf. `DialogManager`, `SelectionManager`, `TabManager`) rather than growing `app.rs`.

## 3. Tool trait contract
- Every tool implements `tools::Tool` and is registered in `tools/registry.rs::ToolRegistry::register_all`.
- `Tool::execute` takes a **single** `ToolContext<'a>` (config + `&Bus<FileEvent>`) — do not add extra parameters (REQ-609).
- Filesystem tools must resolve paths through `ToolContext::resolve_virtual_path(vpath, allow_write)`; never accept raw filesystem paths from LLM input (REQ-700..708).
- Tools surface conditional availability via `Tool::is_enabled(config, prompt)`; do not branch inside `execute` on prompt keywords (CSV DB gating, REQ-650, is the canonical pattern).
- Safe tools may be dispatched in parallel; unsafe (mutating) tools must be sequential. `agent::tool_executor::ToolExecutor` already enforces this — do not bypass it.
- When adding a tool, also update `Tools.md` and the tool table in `SPEC.md`.

## 4. Spec traceability
- Every user-facing behaviour maps to a `REQ-xxx` in `SPEC.md`. When adding a feature, add the requirement first; when changing behaviour, update the REQ in place.
- Code may cite `REQ-xxx` in `//!`/`///` comments where it aids review (e.g. `(REQ-609)` next to the safe/unsafe split). Do not sprinkle citations inline in business logic.
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
├── config/                 # AppConfig + client structs, loader, secrets, VirtualPath
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
├── background/             # Indexer, FileWatcher, PdfConverterWorker,
│                           #   ImageVisionWorker, BusRouter, ProcessManager,
│                           #   Task orchestrator
├── batch/                  # Batch prompt processing (coordinator, discoverer,
│                           #   executor, file_matcher, prompts, types)
├── events/                 # Bus<FileEvent>, FileEventProcessor, DirectoryTracker
├── editor.rs               # Inline egui text editor widget (calls markdown::)
├── tag_manager.rs          # TagManager (file_tags, all_tags, prompt_paths)
├── messages.rs             # BackgroundMessage + TokenUsageInfo
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
- **Event bus + its consumers** (`Bus<FileEvent>`, `FileEventProcessor`,
  `DirectoryTracker`) go in `events/`. Producers (indexer, watcher) live in
  `background/`; consumers that drive UI state live in `events/` or `ui/`.
- **Configuration** (data types, loader, secret-redacting Debug impls,
  `VirtualPath`) goes in `config/`. Keep `AppConfig` data-only; behaviour lives
  in the subsystem that uses it.
- **Generic, domain-free helpers** (path utilities, file-walk tag extraction)
  go in `utils/`. If a helper knows about Markdown, it belongs in `markdown/`,
  not `utils/`.
- **Cross-cutting value types** with no single home (e.g. `BackgroundMessage`,
  `TokenUsageInfo`) may live as top-level modules (`messages.rs`); prefer a
  subsystem home when one exists.

### Module size and splitting

- Target **≤ 400 lines** per `.rs` file. When a file exceeds this, split by
  concern into a submodule directory (cf. `config/`, `tools/registry/`,
  `ui/app/`). Keep the original file as a `mod.rs` that re-exports the pieces
  so public paths are unchanged.
- `lib.rs` is a **facade only** — no logic, only `pub use` of subsystem
  public APIs. Do not grow it when adding features; add to the relevant
  subsystem and let `lib.rs` re-export.
- When extracting a submodule, preserve the existing public import path via
  `pub use` in the parent `mod.rs`. External callers (and `main.rs`) must not
  see path changes.

## 6. Quality Gate (Rust)

Before marking any task as complete, run the following from `src/desktop/` and ensure they all pass cleanly:
- `cargo check` — no errors or warnings
- `cargo test` — all tests pass
- `cargo clippy -- -D warnings` — no lint warnings (deny all)
- `cargo fmt --check` — code is properly formatted
- `cargo doc --no-deps --quiet` — documentation builds without warnings
