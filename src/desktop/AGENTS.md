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

### `egui::Id` Stability and Salting Rules
- **Purpose of `egui::Id`s:** `egui` tracks interactive widget state (hover, focus, animation, context menus, drag/drop) across frames and multi-pass layout renders (e.g., `SidePanel` / `Panel`, `ScrollArea::show_rows`, `Grid`) using `egui::Id`.
- **Preventing `WARN egui::context` pass-to-pass ID changes:** If `egui` sees a widget at the exact same physical coordinates (`rect`) assigned a different `Id` between layout passes (Pass 1 measurement vs. Pass 2 paint), it emits a `WARN egui::context: Widget rect [...] changed id between passes` warning and paints red debug outlines.
- **Rules for setting `egui::Id`s:**
  1. **Never re-use duplicate keys for sibling widgets:** If a loop renders multiple widgets per item (e.g., a tab label and a tab close button `×`), salt each key with a string label tuple:
     ```rust
     ui.push_id((tab_path, "tab_label"), |ui| ui.selectable_label(is_selected, &title));
     ui.push_id((tab_path, "tab_close"), |ui| ui.button("×"));
     ```
  2. **Wrap iterated list and tree rows in salted scopes:** When rendering dynamic lists (`ScrollArea::show_rows`, file trees, TOC lists), wrap each row in `ui.push_id((&item_key, item_type), |ui| { ... })` so internal `ui.horizontal` calls generate stable child auto-IDs.
  3. **Isolate structural blocks:** Assign explicit string keys to top-level containers (`ui.push_id("selected_file_header", ...)`, `ScrollArea::id_salt(...)`) to insulate child auto-ID counters from sibling layout passes.

### Conditional rendering: always allocate, toggle visibility

**Why the warning is sticky.** `egui` is immediate-mode but stateful: hover, focus, drag, scroll, animation, and `Response` flags are all keyed by `egui::Id` and stored between frames. During a single frame, every measure-then-place container (`Panel`, `SidePanel`, `Window`, `Grid`, `ScrollArea::show_rows`) invokes the inner closure multiple times — first to measure, then to place/paint. `egui` requires that each allocation produce the **same `Id` at the same `Rect`** in every pass.

Most widget ids in a `Ui` are *auto-ids* — an invisible per-`Ui` counter whose value at the time of allocation is folded into the parent id's hash. Auto-ids are **positional, not logical**: the Nth widget allocated is the same id in both passes only if both passes allocate the same Nth widget in the same shape. Any structural change between passes — a different `if` arm, a `for` loop that yielded a different number of items, a `Some`/`None` flip, a `match` arm that fell through differently, a `collapsing` that opened — shifts every downstream auto-id by the size of the change.

The disagreement is between two invocations of the same call in the same frame, so you can't see the other invocation when reading the code. The auto-id is invisible. The mismatch can be many layers above the rect that warned, which is why a single off-by-one allocation at the top of a panel can flood the log with hundreds of `changed id between passes` lines. Per-leaf `push_id` salting does not fix this — it narrows the parent id, but it does not fix a tree that allocates a different shape across passes.

**The fix pattern.** When a conditional cannot be eliminated, **both branches of the conditional must allocate the same number of widgets in the same order; only the visibility of the widgets differs.** This keeps the auto-id counter walking the same path in every pass.

Rules:

1. **Replace `if cond { A } else { B }` with "always allocate, branch visibility."** If the two branches allocate a different number of widgets, pad the shorter one with `ui.add_visible(visible, /* placeholder */)` or `ui.allocate_space(size)` so the auto-id counters stay aligned across both arms.
2. **Replace `if let Some(x) = maybe { ... }` with the same shape.** Always allocate the body; toggle its visibility on the `Some`/`None` boundary.
3. **Never wrap an entire `Panel::*::show(...)` in `if cond { ... }`.** When the panel disappears, the available rect for every sibling changes, so the sibling widgets' rects shift in the same frame and the auto-id tree reshuffles. Always allocate the panel, and call `ui.set_invisible()` inside its closure when the cond is false.
4. **`ui.collapsing(header, |ui| { ... })` is fine on its own** — the body is always allocated, only its visibility follows the open/closed state. But never put a *second* conditional inside the body that adds or removes a widget, or the open/close toggle itself becomes a pass-to-pass id mismatch.
5. **Primitives:** `egui::Ui::add_visible(visible, widget)` for a single widget; `ui.scope(|ui| { if !cond { ui.set_invisible(); } ... })` for a block.

Stable `push_id` salts (rules above) are still required for sibling loops — they coexist with this pattern, they do not replace it.

**Canonical example in this codebase** — `src/ui/panels/top.rs:79-100`, the toolbar row's indexing-finished transition. The previous revision rendered a `Spinner` while indexing and a `ComboBox` after indexing inside mutually-exclusive `if/else if` blocks keyed on `indexing_finished`. The moment indexing finished, the combobox replaced the spinner at the same rect on successive passes and `egui` logged `WARN egui::context: Widget rect ... changed id between passes` for the entire toolbar row on every frame. The fix: `ui.add_visible(!indexing_finished, Spinner::new())` and a `ui.scope(...)` that always allocates the separator and combobox, calling `ui.set_invisible()` while indexing. The regression test `test_show_top_panel_no_id_change_warnings_on_indexing_finished_transition` in the same file captures `log` output across the bool flip and asserts no `changed id` warning fires.

**Counter-example to avoid** — `src/ui/panels/right.rs:60-99`, the right panel wrapped in `if should_show_panel(...) { Panel::right("toc_panel").show(...) }`. The TOC items inside are correctly salted with `push_id((i, entry.id, "toc_item"))`, but the *panel itself* still appears and disappears. When the user selects their first file (or unselects), the panel's allocation toggles, the center panel's available rect changes, and every rect inside the center panel shifts in the same frame. The fix is to *always* allocate the `Panel::right` and call `ui.set_invisible()` inside its closure when `should_show_panel` is false.

**Triage heuristic.** To localise a residual warning, do one user action at a time and watch the log for the rect coordinates:

- Hitting Enter in the bottom panel's command input trips `agent.show_results`; warnings at the center panel's rects (`x≈287+`) point at `src/ui/panels/center.rs:318-328`.
- Selecting or deselecting a file in the left tree trips the right panel's existence; warnings at the right side of the screen (`x≈700+`) point at `src/ui/panels/right.rs:60-99`.
- Switching between two files where one has YAML front-matter and one does not trips `if let Some(yaml)`; warnings at the top of the center scroll (`y≈180-220`) point at `src/ui/panels/center.rs:213-218`.
- Watching an agent stream trips the `collapsing` and `if !is_empty()` chain; warnings in the center that grow frame by frame point at `src/ui/panels/center.rs:98-130`.

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
