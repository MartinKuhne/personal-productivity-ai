# Implementation Plan: Chrome DevTools Integration for Web Fetch

**Branch**: `feature/chrome-devtools-web-fetch` | **Date**: 2026-09-07 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/003-chrome-devtools-web-fetch/spec.md`

## Summary

Enhance the `web_fetch` tool in `fastmd-agent` to automatically detect whether Google Chrome (or a compatible Chromium-based browser such as Microsoft Edge, Chromium, or Brave) is installed on the host machine. When detected, `web_fetch` executes headless Chrome (`--headless=new --dump-dom --virtual-time-budget=3000`) within an isolated ephemeral profile (`--incognito --user-data-dir=<temp>`), allowing client-side JavaScript to execute and hydrate the DOM before extracting the fully rendered HTML and converting it to Markdown via `fast_h2m`. When Chrome is absent or when browser execution fails or exceeds the 15-second safety timeout, the tool transparently and seamlessly falls back to standard HTTP GET retrieval via `reqwest`, ensuring zero user-facing regressions. Cached responses (30-minute TTL) and line-based cursor pagination (64 lines per page) remain identical and fully preserved across all fetch types.

## Technical Context

**Language/Version**: Rust 2024 edition, stable/nightly via `rust-toolchain`.

**Primary Dependencies**: `reqwest` 0.13 (blocking HTTP client with `rustls`), `fast_h2m` 0.4 (HTML-to-Markdown converter), `tokio` 1.53 (asynchronous runtime and process execution), `tempfile` 3.8 (isolated ephemeral browser profile directories), `tracing` 0.1 (structured logging). No external browser automation drivers or node daemons required.

**Storage**: Ephemeral `tempfile::TempDir` allocated per headless browser execution and automatically removed upon exit; caching managed by existing in-memory `ToolCache` (`web_documents` and `web_lines`). No persistent configuration writes or database mutations (complying with RUST-024).

**Testing**: `cargo nextest run -p fastmd-agent` for unit tests and `cargo nextest run --workspace` for workspace validation. Unit tests reside in `src/agent/tools/web_tests.rs` using dependency injection (`BrowserRunner` and `BrowserLocator` traits) to test Chrome happy path, fallback on missing browser, fallback on crash, timeout recovery, and pagination consistency without requiring a live Chrome installation in CI (complying with RUST-001 and RUST-006).

**Target Platform**: Desktop operating systems: Windows (primary, checking `%ProgramFiles%`, `%LocalAppData%`, and Microsoft Edge), macOS (`/Applications`), and Linux (`/usr/bin`).

**Project Type**: Desktop application & agent library (`fastmd-agent` crate).

**Performance Goals**: Cache hits return in <50ms; headless Chrome renders complete in <3s on average; total timeout capped at 15s before triggering immediate HTTP fallback; process startup memory footprint bounded to ~100MB per active fetch.

**Constraints**: Completely headless execution (no windows or dock/taskbar icons); zero leakage of user browsing sessions or cookies; non-blocking telemetry per NFR-002; structured error codes on failure per NFR-004.

**Scale/Scope**: 1 tool affected (`web_fetch`), ~3 new internal modules/types (`BrowserLocator`, `BrowserRunner`, `WebFetchPipeline`), ~12 new unit tests, zero modifications to external tool contracts or schema signatures.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Evidence / Notes |
| :--- | :---: | :--- |
| **I. Testability** | **PASS** | `BrowserLocator` and `BrowserRunner` are injected via traits; tests in `web_tests.rs` verify all code paths (happy path, missing browser, crash, timeout, cache hits) without invoking external browser processes or touching live filesystem state. |
| **II. Security** | **PASS** | Every browser invocation runs with `--incognito`, `--no-first-run`, and an isolated temporary `--user-data-dir`. Personal user cookies, passwords, and sessions are never accessed or altered. |
| **III. Modularity** | **PASS** | Browser detection and execution are encapsulated in focused, modular components under `src/agent/tools/` without sprawling cross-crate refactors. `fastmd-agent` remains self-contained. |
| **IV. Open Source Leverage** | **PASS** | Leverages existing workspace dependencies (`reqwest`, `fast_h2m`, `tempfile`, `tokio`) and the host system's existing Chromium binary instead of introducing heavy external frameworks. |
| **V. SDLC Best Practices** | **PASS** | Clear separation of concerns, test-driven implementation, strict clippy compliance (`-D warnings`), clean documentation comments (`//!` and `///`), and traceable requirements to `FR-001`–`FR-010`. |

**Gate Result**: PASS — 100% compliant with constitution principles.

## Project Structure

### Documentation (this feature)

```text
specs/003-chrome-devtools-web-fetch/
├── plan.md              # Implementation plan (this file)
├── research.md          # Technical research & architectural decisions (Phase 0)
├── data-model.md        # Entities, structs, state machine (Phase 1)
├── contracts/           # Tool interface contract (Phase 1)
│   └── web-fetch-contract.md
├── quickstart.md        # Validation scenarios & test instructions (Phase 1)
└── checklists/
    └── requirements.md  # Specification quality checklist
```

### Source Code (repository root)

```text
src/agent/
├── tools/
│   ├── web.rs                 # [MODIFY] Integrate BrowserLocator & BrowserRunner into tool_web_fetch
│   ├── web_tests.rs           # [MODIFY] Unit tests for browser detection, mock execution & HTTP fallback
│   ├── browser_locator.rs     # [NEW] Multi-platform discovery of installed Chromium binaries
│   ├── browser_runner.rs      # [NEW] Headless execution runner with timeout, isolation & stdout capture
│   └── registry/
│       └── builtin/
│           └── strings.rs     # [MODIFY] String constants for tool descriptions, log codes & error messages
src/app/                       # No changes required (pure agent tool enhancement)
```

**Structure Decision**: Single-component layout in `fastmd-agent` crate under `src/agent/tools/`. New files are placed by domain concern per RUST-050/051; module-level documentation and sidecar pointer conventions strictly followed per RUST-010/057.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
| :--- | :--- | :--- |
| — | — | — |

*No constitutional violations identified; no complexity exceptions required.*

---

## Phase Roadmap

### Phase 0: Research & Architecture (Complete)
- [x] Research browser discovery mechanisms across Windows, macOS, and Linux ([`research.md`](research.md)).
- [x] Select invocation strategy (`--headless=new --dump-dom` with `--virtual-time-budget=3000` + `reqwest` fallback).
- [x] Define isolation flags (`--incognito --user-data-dir=<temp>`).

### Phase 1: Design & Contracts (Complete)
- [x] Model data structures, traits, and state transitions ([`data-model.md`](data-model.md)).
- [x] Formalize tool contract and error/fallback semantics ([`contracts/web-fetch-contract.md`](contracts/web-fetch-contract.md)).
- [x] Provide runnable validation scenarios and test instructions ([`quickstart.md`](quickstart.md)).

### Phase 2: Tasks & Implementation (Next Command)
- [ ] Run `/speckit.tasks` to generate actionable, dependency-ordered tasks in `tasks.md`.
- [ ] Implement `BrowserLocator` with Windows/macOS/Linux path resolvers.
- [ ] Implement `BrowserRunner` with child process timeout and ephemeral directory management.
- [ ] Wire `tool_web_fetch` with pipeline fallback logic.
- [ ] Validate all test suites with `cargo nextest`.
