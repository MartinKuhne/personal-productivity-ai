# SPEC.md: FastMD Technical Specification

## Summary
This document specifies the technical requirements and architecture for **FastMD**, a hardware-accelerated, native Windows Markdown viewer. FastMD delivers high-performance filesystem navigation, GFM table layout, hierarchical Table of Contents (ToC) scrolling, real-time filesystem synchronization, and concurrent metadata indexing.

## Background / Context
Markdown document viewers often rely on web engines or Electron shell instances, which impose significant memory and CPU overhead. FastMD addresses this by providing a single native binary using the Rust programming language and the GPU-accelerated `egui` framework, resulting in instant startup, minimal memory footprint, and fluid rendering.

## Requirements

The requirements below have been formatted using the **Easy Approach to Requirements Syntax (EARS)**, utilizing Ubiquitous, Event-Driven (When), State-Driven (While), Unwanted Behavior (If), and Optional Feature (Where) templates.

### 1. User Interface Layout & Styling

```text
+-----------------------------------------------------------------------+
| ⚡ FastMD Viewer     [ Spinner ] Indexing workspace...  [ Tag Filter ] |
+-------------------+-----------------------------------+---------------+
| Workspace Files   | # Document Title                  | Table of      |
|                   |                                   | Contents      |
| 📂 docs/          | YAML Front Matter                 |               |
|   📄 api.md       | +-------+-----------------------+ | H1 Document   |
|   📄 spec.md      | | Key   | Value                 | |   H2 Section  |
|                   | +-------+-----------------------+ |   H2 Section  |
| 📂 src/           |                                   |     H3 Sub    |
|   📄 main.rs      | Markdown Content...               |               |
|                   |                                   |               |
+-------------------+-----------------------------------+---------------+
| > LLM Command Prompt (Agent input...)                                 |
+-----------------------------------------------------------------------+
```

* [REQ-101] Pane Structure: The FastMD Viewer shall display a multi-pane layout consisting of a Left Pane (directory tree and tag filter), Central Pane (Markdown document), Right Pane (Table of Contents), and Bottom Pane (command prompt).
* [REQ-102] Dark Color Scheme: The FastMD Viewer shall pin egui to its `Theme::Dark` at startup and apply the FastMD brand palette to the dark theme's visuals so the dark surface is the source of truth regardless of the host system's reported theme preference. The brand palette is:
    * Window and panel surface: `RGB(9, 9, 11)` (a near-black neutral with a 1-unit cool bias so the surface reads as a black panel, not a brown one).
    * Selection background: `RGB(99, 102, 241)` (indigo-500; the FastMD primary accent).
    * Window corner radius: 8 px. Widget (noninteractive / inactive / hovered / active) corner radius: 4 px.
    * Body text: `RGB(210, 210, 210)` (off-white) for non-interactive and inactive widgets; pure white for hovered and active widgets.
    * The egui "default dark" palette (`RGB(27, 27, 27)` panel fill) shall NOT be used; the FastMD palette above is the only acceptable dark surface.
    * The palette is applied via `FastMdApp::configure_dark_theme`, which is the single point of truth for the dark color scheme.
* [REQ-103] UI Responsiveness: While executing disk I/O, compilation, or file system crawls, the FastMD Viewer shall maintain an unblocked, responsive UI thread.

### Left column / Directory tree

* [REQ-149] When the user clicks on a file, it opens as a new tab in the file viewer area
* [REQ-150] When the user double-clicks on a file, it opens in the system default editor
* [REQ-151] When the user right-clicks on a file or folder in the directory tree, the context menu appears
* [REQ-152] When the user selects [Edit] from the context menu, and the object under the mouse cursor is a file, it opens in the system default editor.
* [REQ-153] When the user selects [Delete] from the context menu, the file or folder gets moved to the recycle bin
* [REQ-154] When the user selects [Show in File Explorer] from the context menu, the system opens the system file exporer with the directory that contains the file
* [REQ-155] When the user selects [Move] from the context menu, the system shows a modal dialog containing all the known folders as well as 'Ok' and 'Cancel' buttons. When the user selects a folder and then 'Ok' the system moves the file to that folder, then closes the dialog. When the user selects 'Cancel' the dialog closes and the file is not moved or changed.
* [REQ-156] When the user selects [Create Directory ...] from the context menu, the system opens a modal dialog for the user to enter a directory name, as well as 'Ok' and 'Cancel' buttons. When the user enters a valid folder name and then clicks 'Ok' the system creates the directory, then closes the dialog. When the user selects 'Cancel' the dialog closes and no side effects occur
* [REQ-157] When the user selects [Rename] from the context menu, the system shows a modal dialog containing the current file name as well as 'Ok' and 'Cancel' buttons. When the user makes changes to the file name and then clicks 'Ok' or presses the enter key, the system renames the file or folder, then closes the dialog. When the user selects 'Cancel' the dialog closes and the file is not moved or changed.
* [REQ-158] When the user selects [Copy path] from the context menu, the system copies the fully qualified file or directory name to the clipboard
* [REQ-159] When the user selects [Print] from the context menu, and the item under the mouse cursor is a file, the system prints the page using the windows system print dialog (implemented via ShellExecute "print" verb).
* [REQ-160] When the user selects [New document] from the context menu, and the item under the mouse cursor is a directory, the system creates a document containing the yaml markdown header and the name 'New document.md'. If a file with that name exist, add the current date and time do the document name until a unique file name is generated.
* [REQ-170] The left column shall increase in size to display any one item without line breaks, to use up to 20% of the available width. The system shall re-evaluate the width needed when the user navigates to a new directory.
* [REQ-171] On every level of the directory tree, directories appear before files
* [REQ-172] The directory tree should not display folders that contain no markdown files
* [REQ-173] When the user selects [Format Markdown] from the context menu, the system executes the Format Markdown quick task as described elsewhere
* [REQ-174] When the user selects [Run as prompt] from the context menu, and the object under the mouse cursor is a file, the system shall execute the content of th file as an agent prompt
* [REQ-175] Tag Filter Directory Hiding: When filtering by tag, the directory tree shall not display directories that do not contain any files matching the active tag.
* [REQ-180] When the user holds the shift, the system shall allow the user to select multiple documents
* [REQ-181] When the user has selected multiple documents, and they right click on one of the selected documents, the [multi select context menu] is shown
* [REQ-182] When the user selects [Merge] from the [multi select context menu], the system shall run a new LLM prompt instructing the LLM to merge the content into a new document and consolidate the content. 
* [REQ-183] When the user selects [Delete] from the [multi select context menu], the system shall move all the selected files to the recycle bin

### Middle column / File viewer area

* [REQ-190] When the user right-clicks on a document tab in the center panel tab bar, a tab context menu shall appear with the following options:
    * [REQ-191] [Close] - Closes the selected tab. If the tab has unsaved changes, prompt for confirmation.
    * [REQ-192] [Close Others] - Closes all other tabs except the selected one.
    * [REQ-193] [Close All] - Closes all open tabs. If any have unsaved changes, prompt for confirmation.
    * [REQ-194] [Copy Path] - Copies the full virtual path of the tab's file to the clipboard.
    * [REQ-195] [Show in File Explorer] - Opens the system file explorer with the tab's file selected.
    * [REQ-196] [Open in Editor] - Opens the tab's file in the system default editor (same behavior as double-click in directory tree).
    * [REQ-197] [Format Markdown] - Executes the Format Markdown quick task on the tab's file.
* [REQ-198] The tab context menu items [Copy Path], [Show in File Explorer], [Open in Editor], [Format Markdown] shall also be available when right-clicking on a file in the directory tree (see REQ-152 through REQ-173), providing consistent behavior across both UI locations.

### Inline Text Editor

* [REQ-250] Inline Editor Toggle: The system shall provide a configuration option `inline_editor_enabled` (default: `false`) in `config.yaml` to enable the built-in inline text editor.
* [REQ-251] Edit Behavior Override: When `inline_editor_enabled` is `true`, selecting [Edit] from the file context menu (directory tree or tab bar) shall open the inline editor instead of launching the system default editor.
* [REQ-252] Editor Content: The inline editor shall display only the raw Markdown body content of the file, excluding the YAML front-matter header. The front-matter shall remain unchanged on save.
* [REQ-253] Editor UI: The inline editor shall appear as a modal dialog or panel overlay with a monospace text editing area, a status bar showing line/column position, and [Save] and [Cancel] buttons.
* [REQ-254] Text Selection: The editor shall support standard text selection via mouse drag, double-click to select word, triple-click to select line, and Shift+arrow keys.
* [REQ-255] Clipboard Operations: The editor shall support Copy (Ctrl+C), Cut (Ctrl+X), and Paste (Ctrl+V) via keyboard shortcuts and context menu.
* [REQ-256] Cursor Navigation: The editor shall support cursor movement by character (←/→), word (Ctrl+←/→), line (↑/↓), line start (Home), line end (End), document start (Ctrl+Home), and document end (Ctrl+End).
* [REQ-257] Undo/Redo: The editor shall support Undo (Ctrl+Z) and Redo (Ctrl+Y) with a minimum of 100 history entries.
* [REQ-258] Markdown Validation: Before saving, the system shall validate the edited Markdown by parsing it with the same GFM parser used for rendering (pulldown-cmark with ENABLE_TABLES, ENABLE_FOOTNOTES, ENABLE_STRIKETHROUGH, ENABLE_TASKLISTS). If parsing fails, the save shall be aborted and an error message displayed with the parse error location.
* [REQ-259] Save Behavior: On successful validation, the editor shall write the new Markdown body combined with the original YAML front-matter back to the file, then close the editor. The file watcher (REQ-403) shall detect the change and hot-reload the view.
* [REQ-260] Cancel Behavior: Selecting [Cancel] shall discard all unsaved changes and close the editor without modifying the file.
* [REQ-261] The inline text editor shall have an inverted, black text on white background color scheme, to help it stand out from other content.

### Markdown

> Markdown requirements moved to [`src/markdown/SPEC.md`](src/markdown/SPEC.md) (MD-001..MD-018). See that file for the full specification of GFM parsing, rendering, table layout, ToC navigation, and YAML front-matter template.

### 3. Concurrent Workspace Indexer Pipeline
* [REQ-301] Parallel Startup Indexer: When the application starts, the FastMD Viewer shall initialize a background directory crawler thread to recursively scan the workspace directory and populate a shared work queue with Markdown file paths.
* [REQ-302] Worker Pool: The FastMD Viewer shall maintain a worker pool of up to four threads to read paths from the queue, parse YAML tags, and notify the GUI thread of results.
* [REQ-303] GUI Progress Reporting: While background indexing is active, the FastMD Viewer shall display a loading spinner and file count in the top menu bar.
* [REQ-304] Progress Completion: When index workers complete, the FastMD Viewer shall replace the spinner with the total file count and populate the tag combobox filter.
* [REQ-305] Progress Completion: When index workers complete, the system shall increase the width of the left column to accomodate the maximum possible file/directory combination found.

### 4. Live Workspace File System Watcher
* [REQ-401] File System Watcher: When the initial index completes, the FastMD Viewer shall schedule a background file system watcher utilizing Windows file/directory change notifications.
* [REQ-402] Hot Reloading:
    * [REQ-403]: When a file is created or modified, the FastMD Viewer shall re-scan the document's YAML tags and update the tag list and directory tree.
    * [REQ-404]: When a file is deleted or renamed, the FastMD Viewer shall remove the document from the tree and tag lists. Rename is detected as a delete + create event pair.
    * [REQ-405]: When the active document is modified, the FastMD Viewer shall hot-reload and redraw it immediately.
    * [REQ-406]: If the active document is deleted, then the FastMD Viewer shall reset the viewer pane.
* [REQ-407] New Directory Watch: When a new directory is created, the file watcher shall automatically begin watching it recursively.

### PDF support

* [REQ-450] PDF Discovery: The system shall scan all configured text libraries for PDF files (extension `.pdf`) during initial indexing (REQ-301) and on file system change notifications (REQ-401).
* [REQ-451] PDF Visibility: PDF files shall NOT be displayed in the directory tree, tab bar, or exposed to any LLM tools (grep, list_files, read_file, etc.). They remain hidden from the user interface.
* [REQ-452] Corresponding Markdown Check: For each discovered PDF file, the system shall check if a Markdown file with the same name (same stem, `.md` extension) exists in the same directory. IF the backing PDF exists, the corresponding .MD file shall be rendered in a PDF-appropriate color in the tree
* [REQ-453] Conversion Trigger: If the corresponding Markdown file does not exist, OR if the Markdown file's last-modified timestamp is older than the PDF's last-modified timestamp, the system shall queue the PDF for conversion.
* [REQ-454] Converter Configuration: The system shall provide a configuration option `pdf_converter_command` in `config.yaml` specifying the executable and arguments to convert PDF to Markdown. The command shall receive the PDF file path as the first argument and the output Markdown file path as the second argument. Example: `["pandoc", "-f", "pdf", "-t", "markdown", "-o", "{output}", "{input}"]`.
* [REQ-455] Conversion Execution: The converter shall run as a background process. The system shall capture stdout/stderr and log to the Background Process Log (REQ-460).
* [REQ-456] Conversion Result Handling: On successful conversion (exit code 0), the generated Markdown file shall be picked up by the normal file watcher (REQ-403) and indexed. On failure, the error shall be logged and the Markdown file shall not be created.
* [REQ-457] Periodic Scan Progress: During initial indexing (REQ-301), the system shall emit progress messages every 500 files scanned or every 5 seconds (whichever comes first), reporting files processed, PDFs found, and conversions queued/completed.
* [REQ-458] Watcher Event Progress: For file system change notifications (REQ-401), the system shall log each event (create/modify/delete/rename) with the virtual path and event type to the Background Process Log.

### Background Process Log Tab

* [REQ-460] Background Process Log Tab: The system shall provide a "Background Processes" tab in the center panel tab bar that displays real-time output from background processes including: initial indexing, file watcher events, PDF conversions, image vision analyses, and LLM tool executions.
* [REQ-461] Tab Behavior: The Background Processes tab shall open automatically when the first background task starts (e.g., initial indexing on startup). The user may close the tab. A menu item [View] → [Background Processes] in the top frame menu shall re-open the tab (or focus it if already open).
* [REQ-462] Log Content: Each log entry shall include timestamp (HH:MM:SS.mmm), process category (Indexer, Watcher, PDF Converter, Image Vision, LLM Tools), and message.
* [REQ-463] Log Filtering: The log tab shall provide filter controls for process category and text search.
* [REQ-464] Log Persistence: The log shall retain the last 10,000 entries in memory. On application exit, the log shall be written to `logs/background-process.log` in the user config directory.
* [REQ-465] Log Auto-scroll: The log tab shall auto-scroll to the newest entry unless the user has manually scrolled up, in which case auto-scroll pauses until the user scrolls to the bottom.

### Image Support (Vision)

* [REQ-470] Image Discovery: The system shall scan all configured image libraries for image files (extensions: `.jpg`, `.jpeg`, `.png`, `.gif`, `.webp`, `.bmp`, `.tiff`, `.avif`) during initial indexing (REQ-301) and on file system change notifications (REQ-401).
* [REQ-471] Image Visibility: Image files shall NOT be displayed in the directory tree, tab bar, or exposed to LLM file tools (grep, list_files, read_file, etc.). They remain hidden from the standard file UI.
* [REQ-472] Corresponding Markdown Check: For each discovered image file, the system shall check if a Markdown file with the same name (same stem, `.md` extension) exists in the same directory.
* [REQ-473] Vision Analysis Trigger: If the corresponding Markdown file does not exist, OR if the Markdown file's last-modified timestamp is older than the image's last-modified timestamp, the system shall queue the image for vision analysis.
* [REQ-475] Vision Analysis Execution: The system shall invoke the model tagged with `vision` use_case, sending the image as base64-encoded data URL in the message content. The prompt shall request a detailed Markdown description of the image contents (text, objects, scenes, charts, diagrams, UI elements, etc.).
* [REQ-476] Vision Result Handling: On success, the generated Markdown description shall be written to the corresponding `.md` file (creating or overwriting). The file watcher (REQ-403) shall pick it up and index it. On failure, the error shall be logged to the Background Process Log (REQ-460).
* [REQ-477] Periodic Image Scan Progress: During initial indexing (REQ-301), the system shall emit progress messages every 500 files or 5 seconds, reporting images found, analyses queued/completed.
* [REQ-478] Image Watcher Event Progress: File system events for image files shall be logged to the Background Process Log with virtual path and event type.

### 5. CLI & Deployment
* [REQ-501] CLI Directory Input: The FastMD Viewer shall accept a workspace directory path as its first command-line argument.
    * [REQ-502]: If the provided workspace path does not exist, then the FastMD Viewer shall fallback to the current working directory.
* [REQ-503] UNC Path Normalization: The FastMD Viewer shall normalize and strip UNC prefixes (`\\?\`) from Windows paths.
* [REQ-504] Deployment Binary: The build system shall include a release-deployment binary target (`deploy`)

### 6. Local LLM Interface & Tool Call Agent

> LLM Agent requirements moved to [`src/agent/SPEC.md`](src/agent/SPEC.md) (AGENT-001..AGENT-023). See that file for the full specification of the OpenAI-compatible endpoint, agent loop, model routing, context injection, tool execution, thinking delimiters, and quick tasks.

### Libraries

* [REQ-700] The system shall support multiple content libraries. The libraries have [root_folder, name, kind, readonly (optional, default true), priority (optional, default 0)] attributes
* [REQ-701] The system shall support a content library 'text'. The behaviours throughout this document apply to this type. The tools are markdown focused.
* [REQ-701b] The system shall support a content library 'image'. The image library stores image files that are not directly exposed to the UI or tools. Instead, the system performs vision analysis on images (REQ-470 through REQ-478) and generates corresponding Markdown files that are indexed as text content.
* [REQ-702] The system shall support a virtual file system. The virtual paths are composed of the library name, then the files and directories present at the configured root_folder. Path traversal (.. components) shall be rejected.
* [REQ-703] The Directory tree pane shall display the content library name for each library as the top level node
* [REQ-704] The file based tools shall take virtual paths as arguments, and shall resolve these paths to fully qualified file names for the underlying operating system.
* [REQ-705] The [grep] tool shall search all libraries in priority order (highest first), and return a concatenated result
* [REQ-706] When the [list_files] tool is invoked with the '/' or '.' argument alone, it shall enumerate the list of libraries, enabling the LLM to continue the folder search for the virtual library subfolders
* [REQ-707] ContentLibrary priority field (default 0): grep searches libraries in descending priority order
* [REQ-708] Virtual path resolution shall reject paths containing parent directory (..) components and validate the library name exists

### Batch processing

* [REQ-800] The system shall display a 'Batch ...' button on the top navigation/menu bar bar
* [REQ-801] When the user clicks on the 'Batch ...' button, the [batch prompt processing dialog] opens
* [REQ-802] The [batch prompt processing dialog] shall let the user select a directory from the available directories to process files in
* [REQ-803] The [batch prompt processing dialog] shall let the user specify a wildcard patters of file names to process
* [REQ-804] The [batch prompt processing dialog] shall let the user select a prompt from a list of prompts. Prompts are markdown files with the 'prompt' tag
* [REQ-805] The [batch prompt processing dialog] shall let the user choose between [Batch modes]. Batch modes are [File] and [Directory].
* [REQ-806] The [batch prompt processing dialog] shall hide and ignore the contents of the wildcard pattern when the batch mode is [Directory], since it will not have control over which files are being processed.
* [REQ-807] The [batch prompt processing dialog] shall let the user select a processing concurrency number. This shall be a drop-down box with available numbers from 1 to 8. The system shall process that number of prompts concurrently.
* [REQ-808] When the user clicks the 'Cancel' button in the [batch prompt processing dialog], the system shall close the dialog with no action taken and no files modified
* [REQ-809] When the user clicks the 'Process' button in the [batch prompt processing dialog], and the batch mode is [File], the system shall add the file context to the system context and process the prompt once per file.
* [REQ-810] When the user clicks the 'Process' button in the [batch prompt processing dialog], and the batch mode is [Directory], the system shall add the directory context to the system context and process the prompt once per Directory.
* [REQ-811] The [batch prompt processing dialog] shall log the start and end of LLM processing for each file to the background log window.
* [REQ-812] While processing is underway, the [batch prompt processing dialog] shall disable the 'Process'
* [REQ-813] While processing is underway, the [batch prompt processing dialog] shall stop processing new prompts when the user clicks the 'Cancel' button

### LLM Tools

> Tool table and requirements moved to [`src/tools/SPEC.md`](src/tools/SPEC.md) (TOOL-001..TOOL-010). See that file for the full tool catalog, CSV Database tools (TOOL-001..TOOL-004), and Web Fetch pagination/caching (TOOL-005..TOOL-010).

### YAML frontmatter template

> Moved to [`src/markdown/SPEC.md`](src/markdown/SPEC.md#yaml-frontmatter-template).

## Sources
- RFC 2119 Key Words Reference: [ietf.org/rfc/rfc2119.txt](https://www.ietf.org/rfc/rfc2119.txt)
- Egui Documentation: [github.com/ocornut/egui](https://github.com/ocornut/egui)
- Notify Crate API Documentation: [docs.rs/notify](https://docs.rs/notify)
- Rust Standard Library Threading Models: [doc.rust-lang.org/std/thread/](https://doc.rust-lang.org/std/thread/)
