# Phase 0: Research & Technical Decisions

## Decision 1: Text Editor Component
- **Decision**: Use `egui::TextEdit` for the core text editing area.
- **Rationale**: The project already uses `eframe` (egui) for its UI. `egui::TextEdit` natively supports monospace fonts, text selection, clipboard operations, and basic cursor navigation, fulfilling REQ-254, REQ-255, and REQ-256 out of the box.
- **Alternatives considered**: Embedding an external editor or writing a custom text rendering widget. Both are unnecessarily complex and would violate the "keep it simple" and "use existing ecosystem" principles.

## Decision 2: Undo/Redo Implementation
- **Decision**: Leverage `egui`'s built-in undo/redo capabilities if they meet the 100-entry requirement, otherwise wrap `TextEdit` state with a custom history stack.
- **Rationale**: `egui` provides native undo/redo. If we need strict compliance with exactly 100 entries, we can configure `egui`'s state or keep a simple diff stack.
- **Alternatives considered**: None, using built-in is standard.

## Decision 3: Line/Column Status Bar
- **Decision**: Extract cursor position from `egui::TextEdit` state via `cursor_range()`.
- **Rationale**: We can query the `TextEdit`'s state after it renders to find the cursor's character index, then compute line and column by scanning the current text up to that index.
- **Alternatives considered**: Maintaining a separate cursor state, but that could get out of sync with `egui`'s internal state.

## Decision 4: Markdown Validation
- **Decision**: Use the existing `pulldown-cmark` parser configured with the required extensions to test-parse the editor content.
- **Rationale**: Ensures the preview and the saved document use the exact same logic, preventing saving corrupt documents.
- **Alternatives considered**: Writing a custom regex validator (too fragile).

## Decision 5: Front-matter Preservation
- **Decision**: Parse the file on load into `front_matter` (string) and `body` (string). When saving, simply concatenate `front_matter + new_body`.
- **Rationale**: Safest way to preserve YAML without modifying its structure, comments, or formatting.
- **Alternatives considered**: Deserializing YAML and re-serializing it, which would destroy comments and formatting.
