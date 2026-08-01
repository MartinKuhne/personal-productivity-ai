# UI Specification

> **GUARDRAIL**: This specification file is managed by the spec-split workflow. Do not edit
> this file directly unless explicitly instructed. Any changes to requirements must be
> reflected in the corresponding implementation code. If drift is detected between
> this spec and the actual code behavior, notify the user immediately.
>
> Part of [`SPEC.md`](../../SPEC.md) (FastMD crate). See the [Requirements Index](../../SPEC.md#requirements-index) for the full REQ-xxx → file map.
>
> Owns UI-001..UI-050, TBL-001..TBL-051. Cross-cutting requirements that also touch this module are listed at the bottom of this file.

## Scope

This module owns the user interface layer: pane layout and styling, directory tree, file viewer area (tabbed document interface), inline text editor, background process log tab, and thinking process section. The code lives in `src/desktop/src/ui/` and `src/desktop/src/editor_egui.rs`.

## Requirements

### User Interface Layout & Styling

* [UI-001] Pane Structure: The FastMD Viewer shall display a multi-pane layout consisting of a Left Pane (directory tree and tag filter), Central Pane (Markdown document), Right Pane (Table of Contents), and Bottom Pane (command prompt).
* [UI-002] Dark Color Scheme: The FastMD Viewer shall pin egui to its `Theme::Dark` at startup and apply the FastMD brand palette to the dark theme's visuals so the dark surface is the source of truth regardless of the host system's reported theme preference. The brand palette is:
    * Window and panel surface: `RGB(9, 9, 11)` (a near-black neutral with a 1-unit cool bias so the surface reads as a black panel, not a brown one).
    * Selection background: `RGB(99, 102, 241)` (indigo-500; the FastMD primary accent).
    * Window corner radius: 8 px. Widget (noninteractive / inactive / hovered / active) corner radius: 4 px.
    * Body text: `RGB(210, 210, 210)` (off-white) for non-interactive and inactive widgets; pure white for hovered and active widgets.
    * The egui "default dark" palette (`RGB(27, 27, 27)` panel fill) shall NOT be used; the FastMD palette above is the only acceptable dark surface.
    * The palette is applied via `FastMdApp::configure_dark_theme`, which is the single point of truth for the dark color scheme.
* [UI-003] UI Responsiveness: While executing disk I/O, compilation, or file system crawls, the FastMD Viewer shall maintain an unblocked, responsive UI thread.

### Left Column / Directory Tree

* [UI-004] When the user clicks on a file, it opens as a new tab in the file viewer area.
* [UI-005] When the user double-clicks on a file, it opens in the system default editor.
* [UI-006] When the user right-clicks on a file or folder in the directory tree, the context menu appears.
* [UI-007] When the user selects [Edit] from the context menu, and the object under the mouse cursor is a file, it opens in the system default editor.
* [UI-008] When the user selects [Delete] from the context menu, the file or folder gets moved to the recycle bin.
* [UI-009] When the user selects [Show in File Explorer] from the context menu, the system opens the system file explorer with the directory that contains the file.
* [UI-010] When the user selects [Move] from the context menu, the system shows a modal dialog containing all the known folders as well as 'Ok' and 'Cancel' buttons. When the user selects a folder and then 'Ok' the system moves the file to that folder, then closes the dialog. When the user selects 'Cancel' the dialog closes and the file is not moved or changed.
* [UI-011] When the user selects [Create Directory ...] from the context menu, the system opens a modal dialog for the user to enter a directory name, as well as 'Ok' and 'Cancel' buttons. The dialog appears next to the mouse cursor when it fits within the window; otherwise it is clamped onto the viewport. When the user enters a valid folder name and then clicks 'Ok' the system creates the directory, then closes the dialog. When the user selects 'Cancel' the dialog closes and no side effects occur.
* [UI-012] When the user selects [Rename] from the context menu, the system shows a modal dialog containing the current file name as well as 'Ok' and 'Cancel' buttons. When the user makes changes to the file name and then clicks 'Ok' or presses the enter key, the system renames the file or folder, then closes the dialog. When the user selects 'Cancel' the dialog closes and the file is not moved or changed.
* [UI-013] When the user selects [Copy path] from the context menu, the system copies the fully qualified file or directory name to the clipboard.
* [UI-014] When the user selects [Print] from the context menu, and the item under the mouse cursor is a file, the system prints the page using the windows system print dialog (implemented via ShellExecute "print" verb).
* [UI-015] When the user selects [New document] from the context menu, and the item under the mouse cursor is a directory, the system opens a modal dialog for the user to enter a document name, as well as 'Ok' and 'Cancel' buttons. The dialog appears next to the mouse cursor when it fits within the window; otherwise it is clamped onto the viewport. When the user enters a valid document name and then clicks 'Ok' or presses Enter, the system creates a document with a yaml markdown header (titled with the entered name) and the file name as entered, appending `.md` when the entered name has no extension. If a file with that name exists, the system adds the current date and time to the document name until a unique file name is generated, then closes the dialog. When the user selects 'Cancel' the dialog closes and no side effects occur.
* [UI-016] The left column shall increase in size to display any one item without line breaks, to use up to 20% of the available width. The system shall re-evaluate the width needed when the user navigates to a new directory.
* [UI-017] On every level of the directory tree, directories appear before files.
* [UI-018] The directory tree should not display folders that contain no markdown files.
* [UI-019] When the user selects [Format Markdown] from the context menu, the system executes the Format Markdown quick task as described elsewhere.
* [UI-020] When the user selects [Run as prompt] from the context menu, and the object under the mouse cursor is a file, the system shall execute the content of the file as an agent prompt.
* [UI-021] Tag Filter Directory Hiding: When filtering by tag, the directory tree shall not display directories that do not contain any files matching the active tag.
* [UI-022] When the user holds the shift key, the system shall allow the user to select multiple documents.
* [UI-023] When the user has selected multiple documents, and they right click on one of the selected documents, the [multi select context menu] is shown.
* [UI-024] When the user selects [Merge] from the [multi select context menu], the system shall run a new LLM prompt instructing the LLM to merge the content into a new document and consolidate the content.
* [UI-025] When the user selects [Delete] from the [multi select context menu], the system shall move all the selected files to the recycle bin.
* [UI-050] Content Search: The left column shall provide a search box at the bottom of the directory tree pane. When the user enters a term and presses the Enter key or clicks the magnifier icon, the system shall filter the directory tree to show only files whose content contains the search term (case-insensitive). While a filter is active, the system shall replace the magnifier icon with a clear (×) icon. When the user clicks the clear icon, the system shall restore the directory tree to show all files. Directories that do not contain any matching files shall not be displayed while the filter is active.

### Middle Column / File Viewer Area

* [UI-026] When the user right-clicks on a document tab in the center panel tab bar, a tab context menu shall appear with the following options:
    * [UI-027] [Close] - Closes the selected tab. If the tab has unsaved changes, prompt for confirmation.
    * [UI-028] [Close Others] - Closes all other tabs except the selected one.
    * [UI-029] [Close All] - Closes all open tabs. If any have unsaved changes, prompt for confirmation.
    * [UI-030] [Copy Path] - Copies the full virtual path of the tab's file to the clipboard.
    * [UI-031] [Show in File Explorer] - Opens the system file explorer with the tab's file selected.
    * [UI-032] [Open in Editor] - Opens the tab's file in the system default editor (same behavior as double-click in directory tree).
    * [UI-033] [Format Markdown] - Executes the Format Markdown quick task on the tab's file.
* [UI-034] The tab context menu items [Copy Path], [Show in File Explorer], [Open in Editor], [Format Markdown] shall also be available when right-clicking on a file in the directory tree (see UI-007 through UI-019), providing consistent behavior across both UI locations.

### Inline Text Editor

* [UI-035] Inline Editor Toggle: The system shall provide a configuration option `inline_editor_enabled` (default: `false`) in `config.yaml` to enable the built-in inline text editor.
* [UI-036] Edit Behavior Override: When `inline_editor_enabled` is `true`, selecting [Edit] from the file context menu (directory tree or tab bar) shall open the inline editor instead of launching the system default editor.
* [UI-037] Editor Content: The inline editor shall display only the raw Markdown body content of the file, excluding the YAML front-matter header. The front-matter shall remain unchanged on save.
* [UI-038] Editor UI: The inline editor shall appear as a modal dialog or panel overlay with a monospace text editing area, a status bar showing line/column position, and [Save] and [Cancel] buttons.
* [UI-039] Text Selection: The editor shall support standard text selection via mouse drag, double-click to select word, triple-click to select line, and Shift+arrow keys.
* [UI-040] Clipboard Operations: The editor shall support Copy (Ctrl+C), Cut (Ctrl+X), and Paste (Ctrl+V) via keyboard shortcuts and context menu.
* [UI-041] Cursor Navigation: The editor shall support cursor movement by character (←/→), word (Ctrl+←/→), line (↑/↓), line start (Home), line end (End), document start (Ctrl+Home), and document end (Ctrl+End).
* [UI-042] Undo/Redo: The editor shall support Undo (Ctrl+Z) and Redo (Ctrl+Y) with a minimum of 100 history entries.
* [UI-043] Markdown Validation: Before saving, the system shall validate the edited Markdown by parsing it with the same GFM parser used for rendering (pulldown-cmark with ENABLE_TABLES, ENABLE_FOOTNOTES, ENABLE_STRIKETHROUGH, ENABLE_TASKLISTS). If parsing fails, the save shall be aborted and an error message displayed with the parse error location.
* [UI-044] Save Behavior: On successful validation, the editor shall write the new Markdown body combined with the original YAML front-matter back to the file, then close the editor. The file watcher (REQ-403) shall detect the change and hot-reload the view.
* [UI-045] Cancel Behavior: Selecting [Cancel] shall discard all unsaved changes and close the editor without modifying the file.
* [UI-046] The inline text editor shall have an inverted, black text on white background color scheme, to help it stand out from other content.

### Background Process Log — UI

* [UI-047] Tab Behavior: The Background Processes tab shall open automatically when the first background task starts (e.g., initial indexing on startup). The user may close the tab. A menu item [View] → [Background Processes] in the top frame menu shall re-open the tab (or focus it if already open).

### Thinking Process Section

* [UI-048] Thinking Delimiter: Model reasoning/thinking content wrapped in `🤔...🤔` delimiters shall be extracted and displayed in a collapsible "Thinking Process" section separate from the main response.

### Tabbed Document Interface

* [UI-049] Tabbed Document Interface: The center panel shall support multiple open documents as tabs. Clicking a file opens it in a new tab; middle-click or close button closes tabs.

### Tools Dialog

* [UI-051] Dialog Trigger: When the user clicks the "Tools..." button on the top toolbar (AGENT-024), the system shall open the Tools dialog as a modal window centered over the main window.
* [UI-052] Row Layout: For every tool group (built-in and MCP server) currently known to the system, the dialog shall display a row containing: an enable checkbox, the group's display name, a kind label ("Internal" or "MCP"), the group's prompt char count (TOOL-015/016), a parallel-safe chip when the group is fully `ReadOnly` (TOOL-020), and an `Authenticate` button when MCP-020 returns `true` for that group.
* [UI-053] Toggle Behaviour: Toggling a row's checkbox shall update `AppConfig` per TOOL-017 / TOOL-018 and persist the change to `config.yaml` immediately via `config::save_config`.
* [UI-054] Authenticate Action: When the user clicks the `Authenticate` button for an eligible MCP server, the system shall invoke `McpClientManager::authenticate(server_name)` on a background thread. The button shall display a disabled "Authenticating..." state until the task completes or fails. A status line shall report success or failure; failure shall not close the dialog.
* [UI-055] Character Count Recomputation: The dialog shall recompute the char count for every row on every open and whenever the underlying tool set changes (e.g. after a successful MCP discovery). Char counts are informational; their exact value is not part of the user contract.
* [UI-056] Close: The dialog shall provide a `Close` action (button and `Esc` keypress via the `Window::open` flag) that hides the dialog without persisting further state.
* [UI-057] Error Indicator: When a group's `last_error` is `Some`, the row shall display a warning indicator (⚠ icon) whose tooltip shows the error kind and message.
* [UI-058] Parallel-Safe Chip: When a group's `parallel_safe` is `true`, the row shall display a "✓ parallel" chip alongside the char count. The chip is informational; its absence does not imply the group is unsafe.
* [UI-059] Authenticate Error State: When an `Authenticate` call fails, the row's status line shall show the error message until the next successful authentication attempt or until the user closes the dialog.
* [UI-060] Clear Error: Each row with a `last_error` shall provide a "Restart" link that calls `ToolManager::clear_error(group)`.

---

## Existing Table Layout Renderer Requirements (TBL-xxx)

### 2. Data Model & Input Specification

* [TBL-001] The System MUST accept input comprising tabular data organized into explicit rows and columns.
* [TBL-002] The System MUST support cell content consisting of arbitrary plain text.
* [TBL-003] The System MUST support formatted text as per the markdown specification.

### 3. Layout and Geometry Calculation

#### 3.1. Sizing Constraints

* [TBL-010] The System MUST accept a maximum available target width ($W_{max}$) from the parent context.
* [TBL-011] The System MUST calculate the intrinsic minimum content width ($W_{min}$) and maximum preferred width ($W_{pref}$) for every column prior to final layout rendering.
* [TBL-012] If $W_{pref, total} \le W_{max}$, the System MUST allocate each column its preferred width $W_{pref}$.
* [TBL-013] If $W_{pref, total} > W_{max}$, the System MUST shrink column widths down to fit $W_{max}$, without reducing any column below its $W_{min}$, unless $W_{max} < W_{min, total}$.

#### 3.2. Overflow and Wrapping

* [TBL-020] When column width allocation is less than cell content length, the System MUST wrap text content onto subsequent lines.
* [TBL-021] The System SHOULD break lines preferentially at whitespace characters.
* [TBL-022] If a single continuous word exceeds the allocated column width, the System MAY fallback to horizontal scrolling.

---

### 4. Alignment and Padding

* [TBL-030] The System MUST align horizontal content LEFT.
* [TBL-031] The System MUST align vertical content alignment within cells: TOP.
* [TBL-032] The System MAY have inner cell padding (top, bottom, left, right) on a per-cell, per-column, or global table level.
* [TBL-033] If present, Padding MUST be factored into all column width and row height calculations.

---

### 5. Rendering & Decoration

* [TBL-045] The System MUST render a medium-gray border around the outer perimeter of every markdown table — Width 1 px, color ≈ `Color32::from_gray(120)`.
* [TBL-044] The Renderer SHOULD NOT perform redundant re-layout passes if neither table data nor target viewport dimensions have changed.

---

### 6. Performance & Error Handling

* [TBL-50] Malformed input (e.g., inconsistent row lengths, negative padding values) MUST NOT result in undefined behavior or memory corruption. The System SHOULD return a descriptive error or normalize the input gracefully.
* [TBL-51] The system SHOULD use available resources (memory, threads, parallelism etc) to speed up processing.

---

## Cross-cutting references

- UI-004 / UI-034 — File open behaviour coordinates with [`src/app/tab_manager.rs`](../../app/tab_manager.rs) and [`src/app/selection_manager.rs`](../../app/selection_manager.rs).
- UI-016 — Left column width logic lives in [`src/app/panel_layout.rs`](../../app/panel_layout.rs).
- UI-017 / UI-018 — Directory tree rendering and filtering in [`src/ui/tree.rs`](../ui/tree.rs).
- UI-022 / UI-023 / UI-024 / UI-025 — Multi-select handling in [`src/ui/panels/left.rs`](../ui/panels/left.rs) and [`src/app/selection_manager.rs`](../../app/selection_manager.rs).
- UI-035..UI-046 — Inline editor implementation in [`src/editor_egui.rs`](../../editor_egui.rs); validation uses [`src/markdown/parser.rs`](../markdown/parser.rs).
- UI-047 — Background log tab UI in [`src/ui/background_logs.rs`](../ui/background_logs.rs); log buffer owned by [`src/background/`](../background/) (see REQ-460..465 in `src/background/SPEC.md`).
- UI-048 — Thinking delimiter rendering in [`src/ui/panels/center.rs`](../ui/panels/center.rs); delimiter extraction in [`src/agent/response_formatter.rs`](../agent/response_formatter.rs) (AGENT-022).
- UI-049 — Tab management in [`src/app/tab_manager.rs`](../../app/tab_manager.rs).
- UI-050 — Tree content-search state in [`src/app/tree_search.rs`](../../app/tree_search.rs); search box UI in [`src/ui/panels/left.rs`](../ui/panels/left.rs).
- UI-004 / UI-026..UI-034 — Tab context menu and file actions in [`src/ui/panels/center.rs`](../ui/panels/center.rs).