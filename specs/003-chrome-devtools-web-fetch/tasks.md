---
description: "Task list for Chrome DevTools Integration for Web Fetch"
---

# Tasks: Chrome DevTools Integration for Web Fetch

**Input**: Design documents from `/specs/003-chrome-devtools-web-fetch/`  
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/web-fetch-contract.md, quickstart.md  

**Tests**: All tasks include unit test coverage per RUST-003, RUST-005, and Constitution Principle I. Tests are written and verified with `cargo nextest`.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

---

## Format: `- [ ] [TaskID] [P?] [Story?] Description with file path`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (`[US1]`, `[US2]`, `[US3]`, `[US4]`)
- Every task includes exact file paths

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify required dependencies and shared string constants in `fastmd-agent`.

- [X] T001 Verify `tokio` (with `process` feature), `tempfile`, `fast_h2m`, and `reqwest` dependencies in `src/agent/Cargo.toml`
- [X] T002 [P] Define string constants and error codes (`TOOL_W001_BROWSER_FETCH_FAILED`, `TOOL_E001_BROWSER_TIMEOUT`, log codes, CLI flags) with `///` doc comments in `src/agent/tools/registry/builtin/strings.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core browser locator and runner abstractions that all user stories depend on.

**⚠️ CRITICAL**: No user story work can begin until this foundational phase is complete.

- [X] T003 [P] Define `BrowserKind`, `BrowserInstallation`, and `BrowserLocator` trait in `src/agent/tools/browser_locator.rs`
- [X] T004 [P] Implement `SystemBrowserLocator` checking Windows paths/registry, macOS paths, Linux paths, and PATH search in `src/agent/tools/browser_locator.rs`
- [X] T005 [P] Create unit tests for `BrowserLocator` in `src/agent/tools/browser_locator_tests.rs`
- [X] T006 [P] Define `BrowserRunner` trait, `RenderedDocument`, and `BrowserError` types in `src/agent/tools/browser_runner.rs`
- [X] T007 Implement `SystemChromeRunner` supporting headless execution with `--headless=new --dump-dom --virtual-time-budget=3000` via `tokio::process::Command` in `src/agent/tools/browser_runner.rs`
- [X] T008 [P] Create unit tests and `MockBrowserRunner` test double in `src/agent/tools/browser_runner_tests.rs`
- [X] T009 Export `browser_locator` and `browser_runner` modules in `src/agent/tools/mod.rs`

**Checkpoint**: Foundation ready — `BrowserLocator` and `BrowserRunner` traits and implementations compile and pass unit tests with `cargo nextest run -p fastmd-agent`.

---

## Phase 3: User Story 1 - Client-Side Rendered Web Page Fetching via Headless Chrome (Priority: P1) 🎯 MVP

**Goal**: When a compatible Chromium browser is detected, `web_fetch` uses headless Chrome to render client-side JavaScript, extract the rendered DOM, and convert it to Markdown.

**Independent Test**: Can be fully tested by configuring a mock or system browser runner with a simulated client-rendered HTML page; verify that `tool_web_fetch` returns Markdown containing the hydrated elements rather than empty root containers.

### Tests for User Story 1

- [X] T010 [P] [US1] Create unit test `test_tool_web_fetch_chrome_happy_path` in `src/agent/tools/web_tests.rs` asserting that rendered DOM HTML is converted to Markdown and returned with correct line count

### Implementation for User Story 1

- [X] T011 [US1] Integrate `BrowserLocator` and `BrowserRunner` into `tool_web_fetch` in `src/agent/tools/web.rs` to attempt headless rendering when a browser is present
- [X] T012 [US1] Support response header retrieval when `input.headers == true` during browser fetch in `src/agent/tools/web.rs`
- [X] T013 [US1] Ensure browser execution runs headlessly with `--disable-gpu` and suppressed window creation flags in `src/agent/tools/browser_runner.rs`

**Checkpoint**: User Story 1 complete and independently testable — client-rendered pages are hydrated and converted to Markdown via headless Chrome.

---

## Phase 4: User Story 2 - Seamless Fallback to Standard HTTP Retrieval (Priority: P1)

**Goal**: When Chrome is not installed, or when browser execution crashes, fails, or times out, `web_fetch` automatically and transparently falls back to `reqwest` HTTP GET without surfacing an error to the user or agent.

**Independent Test**: Can be fully tested by simulating an absent browser or a crashing runner; verify that `tool_web_fetch` returns standard HTTP content and logs warning `TOOL-W001`.

### Tests for User Story 2

- [X] T014 [P] [US2] Create unit test `test_tool_web_fetch_fallback_when_browser_absent` in `src/agent/tools/web_tests.rs`
- [X] T015 [P] [US2] Create unit test `test_tool_web_fetch_fallback_on_browser_crash_or_error` in `src/agent/tools/web_tests.rs`
- [X] T016 [P] [US2] Create unit test `test_tool_web_fetch_fallback_on_timeout` in `src/agent/tools/web_tests.rs` verifying process termination and fallback within 15 seconds

### Implementation for User Story 2

- [X] T017 [US2] Implement two-tier fallback pipeline in `tool_web_fetch` in `src/agent/tools/web.rs` catching browser runner errors and falling back to `reqwest`
- [X] T018 [US2] Implement structured warning logging (`tracing::warn!`) with error code `TOOL-W001` upon fallback triggers in `src/agent/tools/web.rs`

**Checkpoint**: User Story 2 complete — zero regression on machines without Chrome or on web pages causing browser execution errors.

---

## Phase 5: User Story 3 - Execution Isolation and Privacy (Priority: P2)

**Goal**: Headless Chrome invocations execute in an ephemeral isolated profile (`--incognito`, `--user-data-dir` in `tempfile::TempDir`) that never accesses, alters, or retains user browsing history, cookies, or saved credentials.

**Independent Test**: Inspect child process command-line arguments to verify isolation flags; verify temporary profile directory is cleaned up upon process exit.

### Tests for User Story 3

- [X] T019 [P] [US3] Create unit test `test_chrome_process_arguments_isolation` in `src/agent/tools/browser_runner_tests.rs` asserting `--incognito`, `--no-first-run`, `--no-default-browser-check`, and ephemeral user data dir flags are present
- [X] T020 [P] [US3] Create unit test `test_chrome_ephemeral_tempdir_cleaned_on_exit` in `src/agent/tools/browser_runner_tests.rs`

### Implementation for User Story 3

- [X] T021 [US3] Enforce ephemeral temp directory creation via `tempfile::Builder` and pass `--user-data-dir` in `src/agent/tools/browser_runner.rs`

**Checkpoint**: User Story 3 complete — full privacy boundary enforced, 0% cookie/history leakage.

---

## Phase 6: User Story 4 - Cache Transparency and Pagination Continuity (Priority: P2)

**Goal**: Cached documents (30-minute TTL) and cursor pagination (64 lines per page) behave identically regardless of whether content was fetched via headless Chrome or standard HTTP.

**Independent Test**: Fetch a page via Chrome; verify initial 64 lines returned with cursor; retrieve subsequent pages via cursor without spawning browser processes; verify cache hit on repeat fetch.

### Tests for User Story 4

- [X] T022 [P] [US4] Create unit test `test_tool_web_fetch_chrome_cursor_pagination` in `src/agent/tools/web_tests.rs` ensuring 64-line slicing works on Chrome-rendered content
- [X] T023 [P] [US4] Create unit test `test_tool_web_fetch_chrome_cache_hit_bypasses_browser` in `src/agent/tools/web_tests.rs` verifying that subsequent calls return cached content without invoking the browser
- [X] T024 [P] [US4] Create unit test `test_tool_web_fetch_chrome_force_refetch_invalidates_cache` in `src/agent/tools/web_tests.rs`

### Implementation for User Story 4

- [X] T025 [US4] Wire cache insertion and line cursor pagination for browser-rendered documents into `src/agent/tools/web.rs`

**Checkpoint**: User Story 4 complete — caching and cursor pagination work seamlessly and identically across all fetch modes.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, linting, quality gate checks, and quickstart validation.

- [X] T026 [P] Add module-level doc comments (`//!`) and item doc comments (`///`) across new modules complying with RUST-010 and RUST-011 in `src/agent/tools/browser_locator.rs` and `src/agent/tools/browser_runner.rs`
- [X] T027 Update tool description constant in `src/agent/tools/registry/builtin/strings.rs` mentioning automatic headless Chrome rendering for client-side JavaScript when installed
- [X] T028 Run full workspace quality gate: `cargo check --quiet`, `cargo nextest run --workspace`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps --quiet`
- [X] T029 Execute validation scenarios in `specs/003-chrome-devtools-web-fetch/quickstart.md` and verify end-to-end functionality

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **Foundational (Phase 2)**: Depends on Setup completion — BLOCKS all user stories.
- **User Stories (Phases 3–6)**: Depend on Foundational phase completion.
  - US1 (P1): Can start immediately after Phase 2.
  - US2 (P1): Depends on US1 integration in `web.rs` to implement fallback.
  - US3 (P2): Depends on `browser_runner.rs` from Phase 2/3.
  - US4 (P2): Depends on US1 integration in `web.rs`.
- **Polish (Phase 7)**: Depends on all user stories being completed.

### Parallel Opportunities

- Within Phase 2: T003, T004, T005 (`browser_locator`) can run in parallel with T006, T007, T008 (`browser_runner`).
- Across tests: All test tasks marked `[P]` (T010, T014, T015, T016, T019, T020, T022, T023, T024) can run in parallel.
- Within stories: Unit test authoring can precede or parallel implementation tasks.

---

## Parallel Example: User Story 2 (Fallback Suite)

```bash
# Launch all fallback tests in parallel in web_tests.rs:
Task: "T014 [P] [US2] Create unit test test_tool_web_fetch_fallback_when_browser_absent in src/agent/tools/web_tests.rs"
Task: "T015 [P] [US2] Create unit test test_tool_web_fetch_fallback_on_browser_crash_or_error in src/agent/tools/web_tests.rs"
Task: "T016 [P] [US2] Create unit test test_tool_web_fetch_fallback_on_timeout in src/agent/tools/web_tests.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 & 2)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (`BrowserLocator` + `BrowserRunner`)
3. Complete Phase 3: User Story 1 (Headless Chrome render)
4. Complete Phase 4: User Story 2 (Seamless HTTP fallback)
5. **STOP and VALIDATE**: Run `cargo nextest run -p fastmd-agent` — MVP delivers rich JS rendering when Chrome is present and clean fallback when absent!

### Incremental Delivery

1. Phase 1 + 2: Core discovery and process engine ready.
2. Phase 3 + 4: Core functional MVP ready (hydration + fallback).
3. Phase 5: Privacy hardening (ephemeral isolation verified).
4. Phase 6: Performance hardening (caching + pagination verified).
5. Phase 7: Quality gates, clippy deny-all, and formatting.
