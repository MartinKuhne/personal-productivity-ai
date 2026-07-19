# Implementation Plan: Tab Context Menu

**Branch**: `004-tab-context-menu` | **Date**: 2026-07-19 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/004-tab-context-menu/spec.md`

## Summary

Implement a right-click context menu on document tabs with file management options (Close, Close Others, Close All) and system integration options (Copy Path, Show in Explorer, Open in Editor, Format Markdown).

## Technical Context

**Language/Version**: Rust 2021

**Primary Dependencies**: `eframe` (egui), `tokio`

**Storage**: N/A

**Testing**: `cargo test`

**Target Platform**: Desktop (Windows)

**Project Type**: Desktop app

**Performance Goals**: Responsive UI rendering via egui for context menu interactions.

**Constraints**: Consistent behavior with the existing file tree context menu; ensure clipboard access, file explorer, and default editor functionality use cross-platform safe or Windows-targeted approaches as currently implemented in `ui/tree.rs`.

**Scale/Scope**: Tab management for the `fastmd` text editor.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Testability**: The context menu logic should be contained and trigger easily mockable UI callbacks.
- **Security**: No security-sensitive context.
- **Modularity**: The tab context menu should reuse the file operations code existing in the file tree or place them in a shared utility function.
- **Open Source Leverage**: Utilizing built-in `std::process::Command` and `egui`'s native output for clipboard handling to avoid unnecessary external dependencies.
- **SDLC Best Practices**: Requirements are clear, and implementation will use small iterations starting with standard file close commands before integrating OS-level features.

## Project Structure

### Documentation (this feature)

```text
specs/004-tab-context-menu/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
src/
└── desktop/
    └── src/
        └── ui/
            ├── panels/
            │   ├── center.rs       # Where tabs are rendered and where context menu logic goes
            │   └── bottom.rs
            ├── tree.rs             # Source of existing file system actions
            └── ...
```

**Structure Decision**: The Tab Context Menu will be implemented inside the center panel where tabs are drawn (`src/desktop/src/ui/panels/center.rs`), likely mapping a right-click interaction over tab headers to `ui.interact(...).context_menu(...)`.
