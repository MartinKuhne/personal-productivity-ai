# Implementation Plan: Default System Library & Conversation Logging

**Branch**: `004-system-default-library` | **Date**: 2026-08-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/004-system-default-library/spec.md`

## Summary

Implement the Default System Library and Conversation Logging subsystem (requirements VFS-100 through VFS-104 and VFS-110 through VFS-114). The system library is automatically provisioned under `%APPDATA%/fastmd/system` on Windows (created if non-existent) with default display name `System` or a user-configurable display name `system_library_name`. Inside the system library, a `Conversations` folder is maintained where all agent prompts and responses are logged to timestamped markdown files (`YYYY-MM-DD HH-MM-SS.md`), organized with `## Prompt (nnn)` and `## Response (nnn)` headings, including write tool call details at the end of the response sections.

## Technical Context

**Language/Version**: Rust, edition 2024.

**Primary Dependencies**: `eframe`/`egui` 0.36; `tokio` 1.53; `serde`/`serde_json` 1.0; `chrono` 0.4; `uuid` 1.

**Storage**: Local filesystem under `%APPDATA%/fastmd/system` and `%APPDATA%/fastmd/system/Conversations/` on Windows. Virtual File System (`app::vfs` / `agent::vfs`) registers the system library as a text `ContentLibrary`.

**Testing**: `cargo nextest run --status-level fail --show-progress none`; unit test sidecars (`<file>_tests.rs`); integration tests under `tests/`.

**Target Platform**: Desktop (Windows primary; cross-platform directory fallbacks).

**Project Type**: Desktop application (`fastmd` crate).

**Constraints**:
- RUST-001 / RUST-056: Unit tests in sidecar `<file>_tests.rs`.
- RUST-010 / RUST-011: Documentation comments `//!` and `///` on all modules and pub items.
- RUST-058: `app/` is egui-free.
- Quality Gate: `cargo check --quiet`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps --quiet`, `cargo nextest run`.

## Project Structure

### Documentation (this feature)

```text
specs/004-system-default-library/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── system-library.md
├── checklists/
│   └── requirements.md
└── tasks.md
```

### Source Code

```text
src/
├── agent/
│   ├── config.rs              # AgentConfig support for system library name
│   ├── conversation_logger.rs # Conversation logging logic and formatters
│   ├── conversation_logger_tests.rs # Unit tests for conversation logging
│   └── agent_impl.rs          # Hook into turn processing for logging prompts/responses/write tools
├── app/
│   ├── config/
│   │   ├── config.rs          # AppConfig::system_library_name, path helpers, directory provisioning
│   │   └── config_tests.rs    # Tests for system library config & path helpers
│   ├── orchestrator.rs        # Ensuring system library in content_libraries on app start/config arrival
│   └── vfs/
│       └── SPEC.md            # Mirror VFS-100..114 requirements
```

## Structure Decision

The feature touches configuration (`src/app/config/config.rs` and `src/agent/config.rs`), domain virtual file system management (`src/app/orchestrator.rs`), and agent conversation logging (`src/agent/conversation_logger.rs` and `src/agent/agent_impl.rs`).

