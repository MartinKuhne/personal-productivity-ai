# Agent Instructions

## Anti-Hallucination protocol

### MANDATORY LABELS (use at START of any unverified statement)
- [SPECULATION] - For logical guesses
- [INFERENCE] - For pattern-based conclusions  
- [UNVERIFIED] - For anything you cannot confirm
- [GENERALIZATION] - For broad statements about groups/categories

- Any statement that is not backed by factual evidence is considered unverified
- If you realize you made an unverified claim, immediately state:
> "Correction: My previous statement was unverified. I should have labeled it as [appropriate label]"

## Development process
- Break tasks into logical, contained and testable steps. Confirm tests pass between steps.
- When - and only when - the current branch is 'main' you MUST create a branch before changing ANY files.
- You MUST name the branch feature/<change>, bugfix/<change> or chore/<change>.
- Compile and unit test MUST succeed before starting any work.
- Quality gates MUST pass to call a task complete.
- You SHOULD assess if the existing codebase is suitable for the modification you are being asked to make. You SHOULD suggest to the user any refactorings needed before implementation can start.
- You MUST refuse a task if the requirements or context are unclear. You MUST ask clarifying questions.

## Test-Driven development
- All changes MUST be covered by unit tests. Happy path, corner cases, failure modes, all code paths MUST be covered.
- All changes SHOULD be covered by narrow integration tests.
- When asked to fix a bug, create a failing test first. The test MUST reproduce the issue. Then make the code change. Then prove the code change works because the test passes.

## Code
- You MUST Write modular code with minimal side effects. Functions SHOULD be pure and honest.
- You MUST use string constants for repeat strings or user-facing literals.
- You SHOULD use open source and well-maintained libraries over hand-coding equivalent functions.
- Prefer splitting large functions, extracting helpers, and reducing nesting over introducing additional branches into already-complex code.

## Tests
- [RUST-001] Unit tests SHOULD be kept in a separate file. The file MUST be named <file>_tests.rs.

## Documentation
- [RUST-010] Every module must have a `//!` module-level doc comment containing a concise one-sentence summary of the module's purpose
- [RUST-011] Every `pub` item (struct, enum, function, trait, type alias, const) must have a `///` doc comment.

## Spec traceability
- [RUST-040] Every user-facing behaviour maps to a requirement in `SPEC.md`. When adding or changing a feature, You SHOULD cite `REQ-xxx` in `//!`/`///` comments when making changes to the code.
- [RUST-041] You MUST point out any drift between implemented behaviour and code.
- [RUST-042] Requirements MUST be high level, goal oriented and user facing. Avoid leaking implementation specifics.
- `ARCHITECTURE_C4.md` (in `doc/technical-context/`) is the authoritative architecture picture; You MUST update it when module boundaries or contracts change.
- [RUST-043] YOU MUST NOT CHANGE, UPDATE, COMMIT, REVERT, OR OTHERWISE MODIFY ANY SPEC.MD file UNLESS EXPLICITLY INSTRUCTED TO DO SO.

## Folder structure
- [RUST-050] The crate is organised by **bounded subsystems**. Each directory SHOULD fully contain a cohesive
concern and expose its public API through a `mod.rs` that re-exports symbols.

```
    src/
    ├── agent/                  # fastmd-agent crate — core agent engine & tools
    │   ├── agent_impl.rs       # Agent turn-loop driver (run_agent / run_agent_inner)
    │   ├── context.rs          # AgentContext execution state
    │   ├── datamark.rs         # Prompt-injection defense envelope & delimiters
    │   ├── llm_client.rs       # OpenAI-compatible streaming LLM client
    │   ├── session.rs          # AgentSession lifecycle & thread management
    │   ├── tool_context.rs     # AgentToolContext execution environment
    │   ├── tool_executor.rs    # ToolExecutor (parallel-safe dispatch & side-effects)
    │   ├── lib/                # External service clients & protocols
    │   │   ├── dav/            # CalDAV / CardDAV client & parsers
    │   │   ├── mcp/            # MCP client protocol (SSE/transports, sessions, OAuth 2.1)
    │   │   ├── trello/         # Trello API integration
    │   │   └── weather/        # Geocoding & Open-Meteo weather client
    │   ├── tools/              # Tool trait, descriptor, policies, DTOs
    │   │   ├── registry/       # Tool registry, groups, pagination & builtin/ catalog
    │   │   ├── csv_db/         # CSV database tool family (query, operations, schema)
    │   │   ├── jmap/           # JMAP email client tools & mock server
    │   │   └── mcp/            # McpToolAdapter dynamic tool bridge
    │   ├── utils/              # Agent-level helpers (encoding, markdown, path, tags, uuid)
    │   └── vfs/                # Virtual File System behaviour & path handling
    ├── app/                    # fastmd crate — desktop application & domain
    │   ├── lib.rs              # Application facade & root re-exports
    │   ├── main.rs             # Desktop binary entry point
    │   ├── orchestrator.rs     # AppOrchestrator lifecycle & subsystem wiring
    │   ├── agent/              # App-side agent orchestration
    │   │   ├── prompts.rs      # System prompt construction & library injection
    │   │   ├── batch/          # Batch prompt processing (coordinator, discoverer, executor)
    │   │   └── session/        # BrowserSession (Playwright), PdfBackingTracker, config sync
    │   ├── background/         # Worker pool (embeddings, indexer, logs, task, vector search)
    │   ├── bin/                # Binary targets (deploy.rs, check.rs)
    │   ├── bus/                # Messaging subsystem — broadcast bus & typed channels
    │   │   ├── events/         # Event payloads (agent, config, debug, file, messages, typed)
    │   │   └── router/         # BusRouter file routing & worker plumbing
    │   ├── config/             # AppConfig, client configs, loader, secrets management
    │   ├── export/             # Export subsystem (print, pdf/ via Typst)
    │   │   └── pdf/            # Typst translator & PDF save pipeline
    │   ├── integrations/       # Application external service integrations
    │   │   └── discord/        # Discord bot gateway, commands & safety guardrails
    │   ├── markdown/           # Markdown parsing, document model, table layout
    │   │   └── table_width/    # Fair Table Width Algorithm (pure f32 column solver)
    │   ├── ui/                 # egui layer only: FastMdApp, PanelLayout, modals, tabs
    │   │   ├── agent/          # Agent panel UI, conversation logger, transcript
    │   │   ├── app/            # Main app UI lifecycle (init, render, update frame drain)
    │   │   ├── panels/         # 5-pane layout (top, bottom, left, right, center)
    │   │   ├── render/         # Markdown renderers (code, heading, inline, table, YAML)
    │   │   ├── table_width/    # egui table-width layout adapter
    │   │   ├── test_helpers/   # UI test harness, interaction helpers, offscreen snapshots
    │   │   └── tree/           # Directory tree view, flattening, context & render
    │   ├── utils/              # Generic desktop helpers (clock, path, recycle_bin, tags)
    │   └── workspace/          # Workspace tracking & file synchronization
    │       ├── vfs/            # Desktop VFS integration
    │       └── watcher/        # FileWatcher (notify), FileProcessor, DirectoryTracker
    └── fastmd-tool-macros/     # Proc-macro crate — #[derive(ToolDescriptor)]
        └── src/lib.rs          # ToolDescriptor derive implementation
```

- [RUST-051] When adding or moving code, place files by **concern** or **domain**, not by type
- [RUST-052] **Event-driven fan-out.** Background work MUST reach the UI through event-driven fan-out on `Bus<T>` broadcast buses (`bus::core`). Long-running work MUST run on its own thread or worker and publish results as events onto a `Bus<T>` bus. The UI MUST subscribe as a `BusReader` and drain events each frame.
- [RUST-053] **Module size limit.** Each `.rs` file SHOULD NOT exceed 4096 lines.
- [RUST-053b] When a file exceeds this limit, propose to the user a plan to split by concern into a submodule directory.
- [RUST-054] **Facade-only `lib.rs`.** `lib.rs` MUST be a facade only — no logic, only `pub use` of subsystem public APIs. Do not grow `lib.rs` when adding features; add to the relevant subsystem and let `lib.rs` re-export.
- [RUST-055] **Submodule extraction.** When extracting a submodule, you MUST refactor and update all external callers.
- [RUST-056] **Test sidecar extraction.** When a source file's `#[cfg(test)] mod tests { ... }` block exceeds ~150 lines or more than half the file, extract the test body into a sibling sidecar file. Declare it from the source file with `#[cfg(test)] mod tests;`. The sidecar file MUST be named `<codefile>_tests.rs`.
- [RUST-056b] Use `tests/<name>.rs` (integration test) instead of a sidecar when the test should exercise only the public API.
- [RUST-057] **Sidecar header note.** When an implementation file has a test sidecar, the implementation file's `//!` module doc comment MUST end with a one-line pointer: `//! Unit tests live in the sibling \`<filename>.rs\` sidecar.`
- [RUST-058] **`app/` is egui-free.** No `.rs` file under `app/` MAY import `eframe::egui`, `egui`, or any other UI crate. Rendering concerns MUST go in `ui/`.

## Quality Gate

Before marking any task as complete, run the following from `src/desktop/` and ensure they all pass cleanly:
- `cargo check --quiet` — no errors or warnings
- `cargo nextest run --status-level fail --show-progress none` — all tests pass (the `default` profile in `.config/nextest.toml` retries flaky tier-4 click tests twice; CI uses the `ci` profile which is strict)
- `cargo clippy -- -D warnings` — no lint warnings (deny all)
- `cargo fmt --check` — code is properly formatted
- `cargo doc --no-deps --quiet` — documentation builds without warnings

## Component-specific rules

**CRITICAL ROUTING RULE**: If your task involves editing, analyzing, or testing code in a specific component directory, you **MUST** immediately read its component-specific `AGENTS.md` file using the `view_file` tool *before* taking any action.

| Directory                    | Scope                                                                                  |
|------------------------------|----------------------------------------------------------------------------------------|
| [`src/AGENTS.md`](src/AGENTS.md)                               | Rust `fastmd` application and crates: documentation, egui, quality gate, tool/UI contracts. |
| [`doc/technical-context/AGENTS.md`](doc/technical-context/AGENTS.md) | Maintenance of architecture documentation.                |
| [`doc/planning/AGENTS.md`](doc/planning/AGENTS.md)             | Planning / design-record documents.                                                   |
| [`test/wiki/AGENTS.md`](test/wiki/AGENTS.md)                   | Test wiki fixtures.                                                                   |

### Observabsility

* [NFR-001] All application failures and external protocol failures must produce an `ERROR` log.
* [NFR-002] Telemetry emission (logs, spans, metrics) must be completely non-blocking. The application must buffer telemetry data asynchronously to prevent I/O bottlenecks from degrading the main execution thread.
* [NFR-003] If the centralized observability backend becomes unavailable, the application must not crash or hang.
* [NFR-004] All `ERROR` and `FATAL` level logs must include a standardized, globally unique error code (e.g., `AUTH-4001`) to facilitate automated grouping, filtering, and alerting.
* [NFR-005] Unhandled exceptions and explicit error logs must automatically capture the full execution stack trace and attach it to the structured log payload without truncating the root cause.
* [NFR-006] Application logs must be emitted as structured JSON objects adhering to a centrally defined schema (requiring fields for `timestamp`, `level`, `service_name`, and `correlation_id`).
* [NFR-007] Distributed trace context (W3C Trace Context) must be successfully propagated across 100% of inter-service boundaries, including HTTP/gRPC calls, message queues, and asynchronous task runners.
* [NFR-008] Spans must be automatically generated for all external dependencies, including database queries, external API calls, and cache interactions, capturing exact latency and response status.
* [NFR-009] The logging and tracing pipeline must automatically mask or redact Personally Identifiable Information (PII), authentication tokens, and passwords before the data leaves the application boundary.

<!-- CODEGRAPH_START -->
## CodeGraph

In repositories indexed by CodeGraph (a `.codegraph/` directory exists at the repo root), reach for it BEFORE grep/find or reading files when you need to understand or locate code:

- **MCP tool** (when available): `codegraph_explore` answers most code questions in one call — the relevant symbols' verbatim source plus the call paths between them, including dynamic-dispatch hops grep can't follow. Name a file or symbol in the query to read its current line-numbered source. If it's listed but deferred, load it by name via tool search.
- **Shell** (always works): `codegraph explore "<symbol names or question>"` prints the same output.

If there is no `.codegraph/` directory, skip CodeGraph entirely — indexing is the user's decision.
<!-- CODEGRAPH_END -->
