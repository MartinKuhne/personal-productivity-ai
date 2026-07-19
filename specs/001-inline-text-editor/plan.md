# Implementation Plan: Inline Text Editor

**Branch**: `[001-inline-text-editor]` | **Date**: 2026-07-19 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/001-inline-text-editor/spec.md`

## Summary

Implement an inline text editor overlay for editing Markdown files directly within the application, featuring full text editing capabilities (undo/redo, selection, clipboard), Markdown validation before saving, and strict preservation of YAML front-matter. This functionality is gated by a `inline_editor_enabled` configuration option.

## Technical Context

**Language/Version**: Rust 2021
**Primary Dependencies**: eframe (egui), pulldown-cmark
**Storage**: File system (Markdown files)
**Testing**: cargo test
**Target Platform**: Desktop OS (Windows, macOS, Linux)
**Project Type**: desktop-app
**Performance Goals**: Save operations complete in under 500ms
**Constraints**: Maintain 60fps UI, no blocking I/O on UI thread
**Scale/Scope**: Handling typical Markdown files (< 1MB)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Testability**: The `DocumentContent` parsing and serialization logic must be unit tested independently of the UI.
- **Security**: The editor validates input before writing to disk, ensuring data integrity.
- **Modularity**: The `EditorState` will be self-contained and not deeply coupled with the rest of the application state.
- **Open Source Leverage**: We are leveraging `egui::TextEdit` rather than building a custom text editor widget.
- **SDLC Best Practices**: Plan defines clear validation and test strategies.

## Project Structure

### Documentation (this feature)

```text
specs/001-inline-text-editor/
├── plan.md              
├── research.md          
├── data-model.md        
└── quickstart.md        
```

### Source Code (repository root)

```text
src/desktop/
├── src/
│   ├── app.rs           # Modified to handle config and context menu
│   ├── editor.rs        # New file for EditorState and UI logic
│   └── document.rs      # New/Modified file for front-matter separation
└── tests/
    └── editor_test.rs   # New tests for document parsing/saving
```

**Structure Decision**: The project is a single application (`src/desktop`). We will add `editor.rs` for the UI component and modify `document.rs` (or similar existing module) to handle the front-matter string splitting.

## Complexity Tracking

*(No complexity violations. The approach aligns perfectly with the current architecture.)*
