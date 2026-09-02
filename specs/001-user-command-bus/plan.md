# Implementation Plan: User Command Bus

**Branch**: `[001-user-command-bus]` | **Date**: 2026-09-01 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-user-command-bus/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Introduce a `Bus<UserCommand>` broadcast channel and a unified `UserCommand` enum to route all UI inputs into a centralized orchestrator-side executor. This pattern completely decouples UI rendering from direct app state mutation, eliminates deferred-action fields like `submit_prompt`, and preserves Tier-4 click-capture testability.

## Technical Context

**Language/Version**: Rust 2024 Edition

**Primary Dependencies**: `tokio` (for broadcast channels), `eframe`, `egui`

**Storage**: Local files (workspace tracking/sync)

**Testing**: `cargo test`, `cargo nextest`, `egui_kittest` (Tier-4 click-capture tests)

**Target Platform**: Desktop (Windows, Linux, macOS)

**Project Type**: Desktop Application

**Performance Goals**: N/A (Internal architecture refactoring; must maintain UI 60fps frame rate without noticeable lag)

**Constraints**: Must run synchronously within the `AppOrchestrator` frame update loop.

**Scale/Scope**: Refactoring all 8 primary UI interaction surfaces (Top toolbar, Bottom panel, Center panel, Right panel, Left file tree, Modals, Keyboard shortcuts, Deferred actions).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Testability**: PASS - The change inherently improves testability. By moving from side-effect-heavy `apply_*` helpers to pure functions returning `UserCommand`, Tier-4 click capture tests can assert on returned intents without needing a full orchestrator setup.
- **II. Security**: PASS - No security boundaries are crossed. Internal decoupling only.
- **III. Modularity**: PASS - The project mandates modularity. This refactoring achieves stronger modularity by explicitly separating UI (emitters) from logic (the command executor). The plan iterates through the codebase in 9 small, verifiable stages as mandated by "Refrain from sweeping, massive refactors in a single pass".
- **IV. Open Source Leverage**: PASS - Utilizes the existing `tokio::sync::broadcast` channel infrastructure.
- **V. SDLC Best Practices**: PASS - Validated via existing regression test suite (`cargo nextest` and `cargo check`).

## Project Structure

### Documentation (this feature)

```text
specs/001-user-command-bus/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
src/
├── app/
│   ├── bus/
│   │   ├── events/
│   │   │   └── user_command.rs    # New: UserCommand enum definition
│   │   └── core.rs                # Existing: Bus definitions
│   ├── command_executor.rs        # New: CommandExecutor module for dispatching commands
│   ├── orchestrator.rs            # Existing: Modified to integrate the bus and executor
│   └── ui/                        # Existing: Modified to publish commands instead of mutating state
```

**Structure Decision**: The files are integrated into the existing `fastmd` desktop architecture under `src/app/`, maintaining the boundary between the unified `bus` events and the `ui` layers.
