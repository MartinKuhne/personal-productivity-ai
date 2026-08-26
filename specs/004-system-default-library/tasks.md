---
description: "Task list for Default System Library & Conversation Logging"
---

# Tasks: Default System Library & Conversation Logging

**Input**: Design documents from `/specs/004-system-default-library/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/system-library.md, quickstart.md

**Tests**: Test-driven development with unit tests in sidecars and quality gate checks between steps.

**Organization**: Tasks are ordered sequentially by user story (US1 -> US2 -> US3).

## Path Conventions
- Single crate / workspace root: `C:\Users\mkuhn\src\ppai`
- Quality gate command:
  ```powershell
  cargo check --quiet
  cargo nextest run --status-level fail --show-progress none
  cargo clippy -- -D warnings
  cargo fmt --check
  cargo doc --no-deps --quiet
  ```

---

## Phase 1: User Story 1 — Automatic System Library Provisioning & Custom Naming (Priority: P1)

- [x] T001 [US1] Add `system_library_name: Option<String>` to `AppConfig` and `AgentConfig`, implement `system_library_display_name()`, `get_system_library_path()`, `ensure_system_library_dir()`, `ensure_conversations_dir()`, and `get_or_create_system_library()` in `src/app/config/config.rs` and `src/agent/config.rs` (VFS-100, VFS-101, VFS-102, VFS-103, VFS-104, VFS-110).
- [x] T002 [US1] Add unit tests for system library config, path resolution, directory auto-creation, and custom naming in `src/app/config/config_tests.rs` and `src/agent/config_tests.rs`.
- [x] T003 [US1] Update `src/app/orchestrator.rs` (in `drain_config_bus` and initial config setup) to ensure the system library is registered in `content_libraries` as a writable `text` library with the configured/default display name.

---

## Phase 2: User Story 2 — Automated Conversation Logging for Agent Prompts (Priority: P2)

- [x] T004 [US2] Implement `ConversationLogger` in `src/agent/conversation_logger.rs` and export it in `src/agent/lib.rs` / `src/agent/mod.rs` (or `src/app/agent/`) to manage logging of prompts and assistant responses to `%APPDATA%/fastmd/system/Conversations/YYYY-MM-DD HH-MM-SS.md` with headers `## Prompt (nnn)` and `## Response (nnn)` (VFS-111, VFS-112, VFS-113).
- [x] T005 [US2] Add unit tests in `src/agent/conversation_logger_tests.rs` for timestamped filename generation, header formatting (`## Prompt (1)`, `## Response (1)`, `## Prompt (2)`), and multi-turn continuation logging.
- [x] T006 [US2] Integrate `ConversationLogger` into the agent turn execution in `src/agent/agent_impl.rs` (or `src/agent/manager.rs` / orchestrator) so that every submitted prompt and final model response are logged to the session's conversation file.

---

## Phase 3: User Story 3 — Logging Mutating Tool Calls in Conversation Log (Priority: P3)

- [x] T007 [US3] Extend `ConversationLogger` and `agent_impl.rs` to record executed mutating/write tools (`Safety::Mutating` such as `create_note`, `patch_note`, `insert_into_note`, `move_note`) and format them at the end of the `## Response (nnn)` section in the conversation log (VFS-114).
- [x] T008 [US3] Add unit tests in `src/agent/conversation_logger_tests.rs` verifying that write tool calls are included at the end of `## Response (nnn)` and read-only tool calls are not included.
- [x] T009 [US3] Update `src/app/workspace/vfs/SPEC.md` / `src/SPEC.md` to document the VFS-100..104 and VFS-110..114 requirements.

---

## Phase 4: Quality Gate & Validation

- [x] T010 Run full quality gate (`cargo check`, `cargo nextest run`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps --quiet`) and verify all tests pass.
