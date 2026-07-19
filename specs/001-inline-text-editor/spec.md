# Feature Specification: Inline Text Editor

**Feature Branch**: `[001-inline-text-editor]`

**Created**: 2026-07-19

**Status**: Draft

**Input**: User description: "### Inline Text Editor
* [REQ-250] Inline Editor Toggle: The system shall provide a configuration option `inline_editor_enabled` (default: `false`) in `config.yaml` to enable the built-in inline text editor.
* [REQ-251] Edit Behavior Override: When `inline_editor_enabled` is `true`, selecting [Edit] from the file context menu (directory tree or tab bar) shall open the inline editor instead of launching the system default editor.
* [REQ-252] Editor Content: The inline editor shall display only the raw Markdown body content of the file, excluding the YAML front-matter header. The front-matter shall remain unchanged on save.
* [REQ-253] Editor UI: The inline editor shall appear as a modal dialog or panel overlay with a monospace text editing area, a status bar showing line/column position, and [Save] and [Cancel] buttons.
* [REQ-254] Text Selection: The editor shall support standard text selection via mouse drag, double-click to select word, triple-click to select line, and Shift+arrow keys.
* [REQ-255] Clipboard Operations: The editor shall support Copy (Ctrl+C), Cut (Ctrl+X), and Paste (Ctrl+V) via keyboard shortcuts and context menu.
* [REQ-256] Cursor Navigation: The editor shall support cursor movement by character (←/→), word (Ctrl+←/→), line (↑/↓), line start (Home), line end (End), document start (Ctrl+Home), and document end (Ctrl+End).
* [REQ-257] Undo/Redo: The editor shall support Undo (Ctrl+Z) and Redo (Ctrl+Y) with a minimum of 100 history entries.
* [REQ-258] Markdown Validation: Before saving, the system shall validate the edited Markdown by parsing it with the same GFM parser used for rendering. If parsing fails, the save shall be aborted and an error message displayed with the parse error location.
* [REQ-259] Save Behavior: On successful validation, the editor shall write the new Markdown body combined with the original YAML front-matter back to the file, then close the editor. The file watcher shall detect the change and hot-reload the view.
* [REQ-260] Cancel Behavior: Selecting [Cancel] shall discard all unsaved changes and close the editor without modifying the file."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Edit a file successfully (Priority: P1)

Users need to quickly edit the Markdown content of files without leaving the application or switching contexts to an external editor, saving time and keeping focus.

**Why this priority**: Core workflow. Without the ability to successfully save edits, the editor is useless.

**Independent Test**: Can be fully tested by enabling the inline editor in config, clicking "Edit" on a valid Markdown file, typing changes, and saving. Validated by ensuring changes persist and front-matter is unharmed.

**Acceptance Scenarios**:

1. **Given** the `inline_editor_enabled` config is true and user opens a file for editing, **When** the user types changes and clicks [Save], **Then** the file is saved with changes, front-matter is preserved, the editor closes, and the view updates.

---

### User Story 2 - Discard unsaved changes (Priority: P2)

Users may start editing and decide they made a mistake or changed their minds, needing a safe way to abort without modifying the file.

**Why this priority**: Critical to prevent accidental or unwanted data overwrites.

**Independent Test**: Can be tested by typing text into the editor and clicking Cancel, ensuring the file remains completely unmodified on disk.

**Acceptance Scenarios**:

1. **Given** the inline editor is open with unsaved changes, **When** the user clicks [Cancel], **Then** the editor closes and the file content on disk remains completely unchanged.

---

### User Story 3 - Validate Markdown (Priority: P2)

To prevent rendering breakages or corrupting notes, the system must ensure users cannot save syntactically invalid Markdown.

**Why this priority**: Crucial for data integrity and consistent application rendering.

**Independent Test**: Can be tested by entering intentionally broken Markdown syntax and clicking Save, expecting an error instead of a successful write.

**Acceptance Scenarios**:

1. **Given** the inline editor is open, **When** the user attempts to save text that fails validation, **Then** the save operation is aborted, an error message shows the parse error location, and the editor remains open.

---

### Edge Cases

- What happens when a file has no front-matter header?
- How does system handle very large Markdown files (performance)?
- What happens if the file is modified on disk by an external process while the inline editor is open?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide a configuration option `inline_editor_enabled` in `config.yaml` with a default of `false`.
- **FR-002**: When enabled, the [Edit] context menu action MUST open the inline editor instead of the default system editor.
- **FR-003**: The inline editor MUST display only the raw Markdown body and hide the YAML front-matter.
- **FR-004**: The inline editor UI MUST feature a monospace text area, status bar (line/col), Save button, and Cancel button.
- **FR-005**: The editor MUST support text selection (drag, double/triple click, Shift+arrows) and cursor navigation (character, word, line, document boundaries).
- **FR-006**: The editor MUST support clipboard operations (Copy, Cut, Paste) via shortcuts and context menu.
- **FR-007**: The editor MUST retain an Undo/Redo history of at least 100 entries (Ctrl+Z/Y).
- **FR-008**: System MUST validate Markdown against the application's supported Markdown specification before saving, aborting and showing error locations on failure.
- **FR-009**: On successful save, System MUST persist the original YAML front-matter with the new Markdown body and close the editor.
- **FR-010**: System MUST discard changes and close the editor when [Cancel] is selected.
- **FR-011**: A file watcher MUST detect inline edits and hot-reload the view.

### Key Entities

- **File Configuration**: Settings object mapping to `config.yaml` tracking the `inline_editor_enabled` boolean.
- **Markdown Document**: The representation of the file being edited, split into a `front_matter` block and a `body` block.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of saved Markdown save operations result in structurally valid documents that render without parser errors.
- **SC-002**: Save operations complete in under 500ms on typical files (< 1MB) without blocking the UI.
- **SC-003**: Original YAML front-matter is preserved identically byte-for-byte upon any successful save.
- **SC-004**: Users can successfully navigate the document solely with the keyboard using all specified shortcuts.

## Assumptions

- Users have a keyboard/mouse available (standard desktop environment).
- Markdown files fit comfortably into memory during the editing session.
- The external file watcher is already implemented and handles external change events properly.
