# Implementation Plan: Agent Loop / UI Seam Refactor

**Branch**: `003-agent-ui-seam-refactor` | **Date**: 2026-08-10 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/003-agent-ui-seam-refactor/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Introduce a clean seam between the agent LLM/tool-call loop and the UI rendering
layer of the `fastmd` desktop app. Today the agent thread holds a direct handle
to the UI's mpsc channel (`AgentContext::tx_gui: Sender<BackgroundEvent>`),
formats user-facing markdown strings inside the loop, ships a running
`full_response: String` buffer as `AgentEvent::Response`, and seeds the next
session from a UI-edited copy of that buffer. The `AgentSessionManager` is a god
object spanning agent lifecycle, agent domain state, and pure UI widget state.
The tool executor reaches the UI through a second back-channel
(`ToolExecutor::execute_all` takes `tx_gui` and sends `FsEvent::FileModified`
directly).

The refactor replaces this with: (1) a long-lived `AgentPrompt` mpsc channel
(UI → agent input, carrying a `Uuid` session identity), (2) a `Bus<AgentEvent>`
broadcast channel (agent → UI output, structured session-tagged deltas), and
(3) a `Vec<ToolSideEffect>` return from `execute_all` (no `tx_gui` in tools).
`AgentSessionManager` is split into agent lifecycle state and a UI-owned
`AgentPanelState`. The agent layer becomes UI-free and unit-testable in
isolation. Delivered in 10 small, independently testable iterations.

## Technical Context

**Language/Version**: Rust, edition 2024 (`src/desktop/Cargo.toml:4`).

**Primary Dependencies**: `eframe`/`egui` 0.36 (wgpu, wayland, x11, accesskit,
persistence); `tokio` 1.53 (rt-multi-thread, macros, sync, process — provides
`broadcast::Sender`/`Receiver` behind `bus::core::Bus<T>`); `serde`/`serde_json`
1.0; `uuid` 1 (v4 + serde features — already a direct dep); `reqwest` 0.13
(rustls, blocking); `notify` 8 (file watcher); `playwright-rs` 0.15.1 (optional,
`browser` feature); `jmap-client` 0.4.1 (rustls, blocking); `pulldown-cmark`
0.13. Dev: `egui_kittest` 0.36 (snapshot, wgpu), `wiremock` 0.6, `proptest`
1.4, `accesskit` 0.24, `image` 0.25 (png).

**Storage**: N/A — local-first desktop app. Notes are Markdown files on disk,
accessed through `app::vfs` (parser/library/resolver) and `app::watcher`
(notify-based `FileWatcher`). No database. The agent's conversation history is
an in-memory `Option<Vec<Value>>` on `AgentState` (`manager.rs:35`).

**Testing**: `cargo nextest run --status-level fail --show-progress none`
(primary); `cargo test` (fallback). Snapshot/e2e render tests via
`egui_kittest` (snapshots as PNGs in `src/desktop/tests/snapshots/`).
Integration tests in `src/desktop/tests/` using `wiremock` for fake HTTP
services. Unit tests in sidecar `<file>_tests.rs` files (RUST-001/056). Quality
gate: `cargo check --quiet`, `cargo clippy -- -D warnings`, `cargo fmt --check`,
`cargo doc --no-deps --quiet` (RUST quality gate, `src/desktop/AGENTS.md:78-86`).

**Target Platform**: Desktop — Windows (primary, `cfg(windows)` deps for shell),
Linux (Wayland/X11 via eframe wgpu). Single-user, local-first, offline-capable
(no server component for the agent; LLM calls go to a remote API over HTTPS).

**Project Type**: desktop-app (egui immediate-mode GUI with background worker
threads). Crate name `fastmd` (`Cargo.toml:2`), two binaries: `fastmd` and
`deploy` (`Cargo.toml:138-143`).

**Performance Goals**: 60 fps UI (egui default repaint); agent runs on a
background thread and MUST never block the UI thread (FR-001, AGENT-008).
Agent→UI channel traffic should grow O(n) not O(n²) with output length (SC-003).
**NEEDS CLARIFICATION**: the proposed `Bus<AgentEvent>` is backed by
`tokio::sync::broadcast` (capacity 8192, `bus/core.rs:17`). Unlike the current
`std::sync::mpsc::Sender` (which never drops — `send` only fails if the
receiver is gone), tokio broadcast **silently drops old messages for lagging
subscribers**. If the UI drains once per frame at 60 fps and the agent emits
faster than the UI can process for several frames, agent events could be lost.
Resolved in [research.md](research.md) §1.

**Constraints**:
- RUST-052: background work MUST reach the UI via event-driven fan-out on
  `Bus<T>` broadcast buses; UI subscribes as `BusReader` and drains each frame.
- RUST-058: `app/` is egui-free (no `eframe::egui`, `egui`, or UI-crate
  imports). The agent layer (`agent/`) is already egui-free in practice — the
  only leak is `AgentState::scroll_to_id: Option<String>` (a UI concern parked
  on the agent struct, mutated by `ui/panels/center.rs:205`).
- RUST-050: bounded subsystems; each directory exposes its public API through a
  `mod.rs` re-export.
- RUST-053: no `.rs` file > 4096 lines.
- RUST-054: facade-only `lib.rs`.
- RUST-040: every user-facing behaviour maps to a requirement in `SPEC.md`;
  drift must be flagged (this refactor moves AGENT-011/016/022 satisfaction to a
  different layer — flagged in spec traceability, not a requirement change).
- `ARCHITECTURE_C4.md` (in `doc/technical-context/`) MUST be updated when module
  boundaries or contracts change (RUST-042).
- clippy: `-D warnings` (deny all); `cargo doc` must build without warnings
  (RUST-011: every `pub` item needs a `///` doc comment).

**Scale/Scope**: Single-user desktop app. Agent module: ~12 `.rs` files in
`src/desktop/src/agent/` (agent_impl 493 lines, context 87, manager 436,
tool_executor 272, response_formatter 321, tools/web 578, tools/dtos 619, plus
tool families and sidecar tests). Bus module: ~8 files. UI module: ~40 files.
The refactor touches ~14 files (per spec File Impact Summary) plus the new
`agent/events.rs`. Migration is 10 incremental steps, each independently
testable (FR-017, SC-008). No new runtime dependencies — `uuid` (v4+serde) and
`tokio` (sync broadcast) are already direct deps.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Constitution: `.specify/memory/constitution.md` v1.0.0 (ratified 2026-07-19).
Core principles: I. Testability, II. Security, III. Modularity, IV. Open Source
Leverage, V. SDLC Best Practices. Plus `src/desktop/AGENTS.md` RUST rules.

| Principle | Status | Evidence |
|-----------|--------|----------|
| **I. Testability** | ✅ Pass — primary motivation | The refactor's explicit goal is making `agent/` unit-testable in isolation (FR-005, SC-001, SC-004, User Story 2). Today the agent loop cannot be tested without a UI mpsc channel handle (`AgentContext::tx_gui`). The seam removes that: a test harness feeds `AgentPrompt`s and reads `Bus<AgentEvent>` output. RUST-001 (test sidecars) followed — new `agent/events_tests.rs` etc. |
| **II. Security** | ✅ Pass — neutral-to-positive | No input-validation or sanitization surface changes. Structuring the `web_delegate` trace as `Vec<DelegateToolCall>` (FR-014) removes a minor string-injection vector (the trace was appended verbatim to `full_response` and rendered as markdown). Session identity via `Uuid::new_v4()` (CSPRNG) is standard. No secrets cross the new channel that didn't cross the old one. |
| **III. Modularity** | ✅ Pass — core driver | Bounded subsystem boundary restored: `agent/` owns agent lifecycle + structured events; `ui/` owns presentation + transcript view model + panel state (FR-005/006/010/013). Delivered in 10 small, independently testable iterations (FR-017, SC-008) — not a single sweeping pass. RUST-050 (bounded subsystems), RUST-052 (Bus<T> fan-out), RUST-054 (facade lib.rs) all respected. |
| **IV. Open Source Leverage** | ✅ Pass — reuses existing | Reuses the existing `Bus<T>` (`bus/core.rs`, tokio broadcast, capacity 8192) rather than inventing a transport. Reuses the existing `uuid` crate (v1, v4+serde features, `Cargo.toml:83`). No hand-rolled channel or ID generator. |
| **V. SDLC Best Practices** | ✅ Pass | TDD: failing test first for each migration step (per AGENTS.md development process). Quality gate enforced (cargo check/nextest/clippy/fmt/doc). `ARCHITECTURE_C4.md` will be updated when module boundaries change (RUST-042). Drift flagged per RUST-040. Each iteration must compile + pass tests before the next (FR-017). |

**RUST rule compliance**:
- RUST-052 (Bus<T> fan-out): ✅ — the refactor moves agent→UI onto `Bus<AgentEvent>`, which is the mandated pattern. Today's `tx_gui: mpsc::Sender<BackgroundEvent>` is actually a *violation* of RUST-052 (agent uses mpsc, not `Bus<T>`); this refactor *fixes* the drift.
- RUST-058 (app/ egui-free): ✅ — `app/` stays egui-free. `AgentPanelState` moves to `ui/` or `app/` (egui-free parts); widget state that needs egui stays in `ui/`.
- RUST-050/051/054/055: ✅ — new `agent/events.rs` placed by concern; `mod.rs` re-exports; `lib.rs` stays facade.
- RUST-010/011 (doc comments): ✅ — every new `pub` item gets `///`; every module gets `//!`.

**Gate result**: PASS. No violations. No Complexity Tracking table required.

## Project Structure

### Documentation (this feature)

```text
specs/003-agent-ui-seam-refactor/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output — resolves broadcast-drop question + decisions
├── data-model.md        # Phase 1 output — entity definitions + state transitions
├── quickstart.md        # Phase 1 output — runnable validation scenarios
├── contracts/
│   └── agent-seam.md    # Phase 1 output — AgentPrompt input + AgentEvent output contract
└── tasks.md             # Phase 2 output (/speckit-tasks command — NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
src/desktop/src/
├── agent/                       # LLM tool-loop — becomes UI-free after refactor
│   ├── events.rs                # NEW: AgentEvent, AgentStatus, ToolSideEffect, AgentPrompt
│   ├── agent_impl.rs            # process_turn — remove tx_gui, publish on Bus, emit deltas
│   ├── context.rs               # AgentContext — drop tx_gui/current_response; session_id: Uuid
│   ├── manager.rs               # AgentSessionManager — lifecycle only; own Receiver<AgentPrompt> + Bus<AgentEvent>
│   ├── tool_executor.rs         # execute_all returns (results, Vec<ToolSideEffect>); drop tx_gui
│   ├── response_formatter.rs    # MOVED to ui/render/agent_render.rs (or ui/agent/)
│   ├── tools/
│   │   ├── web.rs               # drop format_delegate_tool_call_message; structured trace
│   │   └── dtos.rs              # WebDelegateResponse: tool_call_trace: String → Vec<DelegateToolCall>
│   └── ... (llm_client, prompt_builder, error, datamark, mod.rs unchanged in shape)
├── bus/
│   ├── core.rs                  # Bus<T> / BusReader<T> — unchanged (reused)
│   └── events/
│       ├── typed.rs             # BackgroundEvent: remove Agent variant; keep Fs/Process/McpAuth
│       └── debug.rs             # AgentDebugEntry: drop session: usize (session_id on enclosing event)
├── app/
│   └── orchestrator.rs          # own Sender<AgentPrompt> + BusReader<AgentEvent>; route by session_id
├── ui/
│   ├── agent/                   # NEW (or ui/render/agent_render.rs): transcript view model + formatting
│   ├── panels/center.rs         # read structured transcript from AgentPanelState; apply_task_toggle on view model
│   ├── agent_debug_window.rs    # read AgentState.debug_entries (session-tagged) + AgentPanelState
│   └── app/{mod.rs,update.rs}   # own AgentPanelState; submit via Sender<AgentPrompt>
└── ... (markdown, config, utils, integrations unchanged)
```

**Structure Decision**: Single-crate desktop app (`fastmd`), organized by bounded
subsystems per RUST-050. The refactor adds one new file (`agent/events.rs`),
moves one (`response_formatter.rs` → `ui/`), and modifies ~12 existing files in
place. No new crates, no new directories beyond `ui/agent/` (optional — could be
a single `ui/render/agent_render.rs` file if the transcript view model is small).
The module tree otherwise matches the existing `src/desktop/AGENTS.md` layout.
