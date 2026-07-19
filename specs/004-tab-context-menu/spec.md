# Feature Specification: Tab Context Menu

**Feature Branch**: `[004-tab-context-menu]`

**Created**: 2026-07-19

**Status**: Draft

**Input**: User description: "* [REQ-190] When the user right-clicks on a document tab in the center panel tab bar, a tab context menu shall appear with the following options: * [REQ-191] [Close] - Closes the selected tab. If the tab has unsaved changes, prompt for confirmation. * [REQ-192] [Close Others] - Closes all other tabs except the selected one. * [REQ-193] [Close All] - Closes all open tabs. If any have unsaved changes, prompt for confirmation. * [REQ-194] [Copy Path] - Copies the full virtual path of the tab's file to the clipboard. * [REQ-195] [Show in File Explorer] - Opens the system file explorer with the tab's file selected. * [REQ-196] [Open in Editor] - Opens the tab's file in the system default editor (same behavior as double-click in directory tree). * [REQ-197] [Format Markdown] - Executes the Format Markdown quick task on the tab's file. * [REQ-198] The tab context menu items [Copy Path], [Show in File Explorer], [Open in Editor], [Format Markdown] shall also be available when right-clicking on a file in the directory tree (see REQ-152 through REQ-173), providing consistent behavior across both UI locations."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Tab Management (Priority: P1)

As a user, I want to quickly manage open tabs using a context menu, so I can keep my workspace organized without needing keyboard shortcuts or navigating through standard menus.

**Why this priority**: Essential for basic workspace hygiene and usability. Allows users to quickly close irrelevant files and focus.

**Independent Test**: Can be fully tested by opening multiple files and verifying that the context menu appears on right-click, and that the close operations (Close, Close Others, Close All) function correctly.

**Acceptance Scenarios**:

1. **Given** multiple open tabs, **When** I right-click on a tab and select "Close", **Then** the selected tab closes and prompts if there are unsaved changes.
2. **Given** multiple open tabs, **When** I right-click on a tab and select "Close Others", **Then** all other tabs are closed and the selected tab remains open.
3. **Given** multiple open tabs, **When** I right-click on a tab and select "Close All", **Then** all open tabs are closed, prompting for confirmation for any tabs with unsaved changes.

---

### User Story 2 - File Operations from Tabs and Tree (Priority: P2)

As a user, I want to access file operations (Copy Path, Show in Explorer, Open in Editor, Format) directly from the tab context menu and directory tree, so I can easily interact with the underlying files.

**Why this priority**: Provides consistent quick access to common file operations from multiple UI locations.

**Independent Test**: Can be tested by right-clicking tabs and directory tree files, verifying the file operations trigger system-level actions (clipboard, explorer, default editor) and the format task.

**Acceptance Scenarios**:

1. **Given** an open tab, **When** I right-click and select "Copy Path", **Then** the file's full virtual path is copied to the clipboard.
2. **Given** an open tab, **When** I right-click and select "Show in File Explorer", **Then** the system file explorer opens with the file selected.
3. **Given** an open tab, **When** I right-click and select "Open in Editor", **Then** the file opens in the system default editor.
4. **Given** an open tab, **When** I right-click and select "Format Markdown", **Then** the format quick task is executed.
5. **Given** a file in the directory tree, **When** I right-click it, **Then** the same file operations are available and function identically.

### Edge Cases

- What happens when a user attempts to "Close All" but one of the files fails to save when prompted?
- What happens when "Show in File Explorer" or "Open in Editor" is triggered on a file that has been deleted or moved externally?
- What happens when "Format Markdown" is executed on a non-markdown file via the tab context menu?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST display a context menu when a user right-clicks on a document tab in the center panel tab bar.
- **FR-002**: The context menu MUST contain a "Close" option that closes the selected tab, prompting for confirmation if it has unsaved changes.
- **FR-003**: The context menu MUST contain a "Close Others" option that closes all tabs except the selected one.
- **FR-004**: The context menu MUST contain a "Close All" option that closes all tabs, prompting for confirmation for any with unsaved changes.
- **FR-005**: The context menu MUST contain a "Copy Path" option that copies the tab's full virtual file path to the clipboard.
- **FR-006**: The context menu MUST contain a "Show in File Explorer" option that opens the OS file explorer with the file selected.
- **FR-007**: The context menu MUST contain an "Open in Editor" option that opens the file in the OS default editor.
- **FR-008**: The context menu MUST contain a "Format Markdown" option that executes the Format Markdown quick task on the file.
- **FR-009**: The directory tree file context menu MUST include "Copy Path", "Show in File Explorer", "Open in Editor", and "Format Markdown" with identical behavior to the tab context menu.

### Key Entities

- **Document Tab**: Represents an open file in the center panel, tracking its unsaved state and underlying file path.
- **Tab Context Menu**: The UI element displaying the available operations for a specific tab.
- **Directory Tree File Node**: Represents a file in the workspace directory tree.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can successfully access and trigger tab context menu options with a right-click.
- **SC-002**: All "Close" operations (Close, Close Others, Close All) accurately respect the unsaved state of files and prevent accidental data loss.
- **SC-003**: System file integration options (Copy Path, Show in Explorer, Open in Editor) correctly bridge the application with the host operating system.
- **SC-004**: Context menu file operations are consistent between the tab bar and the directory tree.

## Assumptions

- The workspace environment allows interaction with the system clipboard and file explorer.
- "Format Markdown" quick task is already defined and executable by the application.
- Prompting for unsaved changes uses the application's existing confirmation dialog system.
