Guardrail: AI agents may not edit, modify, change or delete this file

# SPEC.md: FastMD Technical Specification

## RFC Metadata Block
- **Authors**: FastMD Contributors
- **Date**: July 15, 2026
- **Version**: 1.0.0
- **Category**: Technical Specification

---

## Summary
This document specifies the technical requirements and architecture for **FastMD**, a hardware-accelerated, native Windows Markdown viewer. FastMD delivers high-performance filesystem navigation, GFM table layout, hierarchical Table of Contents (ToC) scrolling, real-time filesystem synchronization, and concurrent metadata indexing.

---

## Background / Context
Markdown document viewers often rely on web engines or Electron shell instances, which impose significant memory and CPU overhead. FastMD addresses this by providing a single native binary using the Rust programming language and the GPU-accelerated `egui` framework, resulting in instant startup, minimal memory footprint, and fluid rendering.

---

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
* [REQ-102] Color System: The FastMD Viewer shall render a premium dark mode layout with a dark gray background, bright off-white body text, and amber-accented inline code snippets.
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

* [REQ-201] GFM Parsing: The Markdown parser shall support the GitHub Flavored Markdown (GFM) tables extension. The parser shall enable the `ENABLE_TABLES`, `ENABLE_FOOTNOTES`, `ENABLE_STRIKETHROUGH`, and `ENABLE_TASKLISTS` options. The `ENABLE_HARD_BREAKS` option is NOT enabled by default.
* [REQ-202] Heading Sizing: The FastMD Viewer shall render H1, H2, and H3 headings at 24px, 20px, and 16px respectively.
* [REQ-203] Break Semantics:
    * [REQ-204]: When a single carriage return (newline) is encountered, the Markdown parser shall render it as a single space character (soft break).
    * [REQ-205]: Hard breaks (two trailing spaces followed by a newline, or backslash-newline) require the `ENABLE_HARD_BREAKS` parser option which is disabled by default. With the option enabled, the parser shall force a line split.
* [REQ-206] Bullet Lists: The FastMD Viewer shall render list items with indentation and bullet points (`•`).
    * [REQ-207]: If a list item contains hard breaks, then the FastMD Viewer shall render the bullet only on the first line; subsequent wrapped lines shall be indented without a bullet. [Note: Current implementation renders bullet on each Item start; this is a known gap.]
* [REQ-208] Table Layout:
    * [REQ-209]: The FastMD Viewer shall render tables inside a horizontally scrollable container with striped rows and column spacing.
    * [REQ-210]: The FastMD Viewer shall arrange cells in an `egui::Grid` with striped rows and column spacing.
    * [REQ-211]: The FastMD Viewer shall render header row cells with a bold font weight.
    * [REQ-211b]: Tables shall be rendered with a distinct visual frame (rounded corners, background color) to separate from body text. [Gap: Not yet implemented.]
* [REQ-212] YAML Front-Matter: Where a document contains a YAML front-matter header, the FastMD Viewer shall parse the metadata into key-value pairs and render them inside a dedicated container table.
* [REQ-213] Table of Contents (ToC) Navigation:
    * [REQ-214]: The ToC panel shall display H1–H3 headers indented by header depth.
    * [REQ-215]: When a ToC element is clicked, the FastMD Viewer shall invoke a viewport scroll event to the selected heading.

### 3. Concurrent Workspace Indexer Pipeline
* [REQ-301] Parallel Startup Indexer: When the application starts, the FastMD Viewer shall initialize a background directory crawler thread to recursively scan the workspace directory and populate a shared work queue with Markdown file paths.
* [REQ-302] Worker Pool: The FastMD Viewer shall maintain a worker pool of up to four threads to read paths from the queue, parse YAML tags, and notify the GUI thread of results.
* [REQ-303] GUI Progress Reporting: While background indexing is active, the FastMD Viewer shall display a loading spinner and file count in the top menu bar.
* [REQ-304] Progress Completion: When index workers complete, the FastMD Viewer shall replace the spinner with the total file count and populate the tag combobox filter.
* [REQ-305] Progress Completion: When index workers complete, the system shall increase the width of the left column to accomodate the maximum possible file/directory combination found.

### 4. Live Workspace File System Watcher
* [REQ-401] File System Watcher: When the initial index completes, the FastMD Viewer shall schedule a background file system watcher utilizing Windows directory change notifications.
* [REQ-402] Hot Reloading:
    * [REQ-403]: When a file is created or modified, the FastMD Viewer shall re-scan the document's YAML tags and update the tag list and directory tree.
    * [REQ-404]: When a file is deleted or renamed, the FastMD Viewer shall remove the document from the tree and tag lists. Rename is detected as a delete + create event pair.
    * [REQ-405]: When the active document is modified, the FastMD Viewer shall hot-reload and redraw it immediately.
    * [REQ-406]: If the active document is deleted, then the FastMD Viewer shall reset the viewer pane.
* [REQ-407] New Directory Watch: When a new directory is created, the file watcher shall automatically begin watching it recursively.

### PDF to Markdown Conversion

* [REQ-450] PDF Discovery: The system shall scan all configured text libraries for PDF files (extension `.pdf`) during initial indexing (REQ-301) and on file system change notifications (REQ-401).
* [REQ-451] PDF Visibility: PDF files shall NOT be displayed in the directory tree, tab bar, or exposed to any LLM tools (grep, list_files, read_file, etc.). They remain hidden from the user interface.
* [REQ-452] Corresponding Markdown Check: For each discovered PDF file, the system shall check if a Markdown file with the same name (same stem, `.md` extension) exists in the same directory.
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
* [REQ-474] Vision Model Configuration: The system shall support a `models` configuration section in `config.yaml` defining multiple models with `use_case` tags: `chat` (default), `embeddings`, `vision`, and an optional `cost` field (integer, default 0, lower = cheaper) used for auto-model selection (REQ-613). Example:
```yaml
models:
  - model: "gpt-4o-mini"
    api_url: "https://api.openai.com/v1"
    use_case: ["chat", "vision"]
    cost: 5
  - model: "text-embedding-3-small"
    api_url: "https://api.openai.com/v1"
    use_case: ["embeddings"]
    cost: 1
  - model: "google/gemini-2.5-flash:free"
    api_url: "https://openrouter.ai/api/v1"
    use_case: ["chat"]
    cost: 0
```
* [REQ-475] Vision Analysis Execution: The system shall invoke the model tagged with `vision` use_case, sending the image as base64-encoded data URL in the message content. The prompt shall request a detailed Markdown description of the image contents (text, objects, scenes, charts, diagrams, UI elements, etc.).
* [REQ-476] Vision Result Handling: On success, the generated Markdown description shall be written to the corresponding `.md` file (creating or overwriting). The file watcher (REQ-403) shall pick it up and index it. On failure, the error shall be logged to the Background Process Log (REQ-460).
* [REQ-477] Periodic Image Scan Progress: During initial indexing (REQ-301), the system shall emit progress messages every 500 files or 5 seconds, reporting images found, analyses queued/completed.
* [REQ-478] Image Watcher Event Progress: File system events for image files shall be logged to the Background Process Log with virtual path and event type.

### 5. CLI & Deployment
* [REQ-501] CLI Directory Input: The FastMD Viewer shall accept a workspace directory path as its first command-line argument.
    * [REQ-502]: If the provided workspace path does not exist, then the FastMD Viewer shall fallback to the current working directory.
* [REQ-503] UNC Path Normalization: The FastMD Viewer shall normalize and strip UNC prefixes (`\\?\`) from Windows paths.
* [REQ-504] Deployment Binary: The build system shall include a release-deployment binary target (`deploy`) that compiles and deploys the optimized binary to `C:\tools\fastmd.exe` when invoked via `cargo run --bin deploy`.

### 6. Local LLM Interface & Tool Call Agent
* [REQ-601] OpenAI Compatible Endpoint: The FastMD Viewer shall support connections to an OpenAI compatible chat completions API.
* [REQ-602] Default Settings: The FastMD Viewer shall default to the OpenRouter endpoint using the free model `google/gemini-2.5-flash:free`.
* [REQ-603] Configuration File: The FastMD Viewer shall parse a YAML configuration file (`config.yaml`) from the standard user configuration path to retrieve the API key, model, endpoint URL, and multi-use model configuration.
    * [REQ-604]: If the configuration file does not exist, then the FastMD Viewer shall create a default template configuration file.
    * [REQ-604a] Multi-Use Model Configuration: The configuration shall support a `models` list where each entry defines `model`, `api_url`, `api_key` (optional, inherits global), `use_case` (array: `chat`, `embeddings`, `vision`), and `cost` (optional integer, default 0, lower = cheaper). The system shall route requests to the appropriate model based on use_case. When multiple models match a use_case, the system shall prefer the model with the lowest `cost`.
    * [REQ-604b] PDF Converter Configuration: The configuration shall support `pdf_converter_command` as an array of command and arguments with `{input}` and `{output}` placeholders.
* [REQ-605] Monospace Command Prompt: When the bottom panel command entry field is submitted, the FastMD Viewer shall execute the command through the Local LLM completions thread.
* [REQ-606] LLM Tools Library: The LLM Agent shall utilize functional tools as per the [LLM Tools] section below.
* [REQ-607] Real-time Stream Output: The FastMD Viewer shall display the LLM's active thinking sequence and render the final Markdown response in real-time inside the Central Panel.
* [REQ-608] Tool Invocation Logging: The system shall print tool call invocations with their significant parameters to the response window. Tool arguments shall be formatted as pretty-printed JSON.
* [REQ-609] Agent Loop: The agent shall execute a tool-use loop: (1) call LLM with tools, (2) execute safe tools in parallel, (3) execute unsafe tools sequentially, (4) append results to conversation, (5) repeat until LLM returns no tool calls or max 10 iterations. Safe tools: grep, read_tags, list_files_by_tag, list_files, read_file, read_file_lines, web_fetch, web_search, read_yaml_header, search_calendar, get_calendar, get_calendar_item, search_email, get_email_by_id, get_email, search_contact, get_contact, list_csv, query. Unsafe tools: create_file, insert_lines, delete_lines, replace_text, write_yaml_header, add_calendar_item, update_calendar_item, delete_calendar_item, send_email, add_contact, web_delegate, add_rows, delete_rows, create_csv.
* [REQ-610] Active File Context: When the user sends an AI prompt and there is a file being displayed in the middle pane, the system shall send the full virtual path of that file with the system prompt.
* [REQ-611] Active Directory Context: When the user selects a directory from the left pane, it becomes the directory context for the AI prompt. When the user sends an AI prompt and there is NO file being displayed in the middle pane, the system shall send the full virtual path of the directory context with the system prompt.
* [REQ-612] Active Directory Context Display: The AI prompt shall display the directory context, relative to the base directory, with the prompt. Example: 'Users\Martin >'
* [REQ-620] When displaying tool call arguments, format the JSON
* [REQ-699] Cancel AI Prompt: While an AI prompt is being executed, the system shall display a stop button. When the user clicks the stop button, the system shall abort the prompt processing.

### Agent Behavior & UI

* [REQ-613] Auto-Model Selection: On application startup, if multiple models are configured with the `chat` use_case, the system shall automatically select the model with the lowest `cost` value and persist the selection to the configuration file.
* [REQ-614] USER.md Context Injection: For each configured content library, if a USER.md file exists at the library root, its contents shall be appended to the system prompt as user context.
* [REQ-615] Agent Conversation History: The agent shall maintain conversation history across prompts within a session. History is reset when the user clicks "Back to Document" or starts a new session.
* [REQ-616] Thinking Delimiter: Model reasoning/thinking content wrapped in `🤔...🤔` delimiters shall be extracted and displayed in a collapsible "Thinking Process" section separate from the main response.
* [REQ-617] Model Management Commands: The command prompt shall support `/models` to list available models with their cost and use_cases, and `/model <name>` to switch the active model and persist to config.
* [REQ-618] Quick Tasks Menu: The bottom panel shall provide a "Quick Tasks" menu with predefined prompts (e.g., "Format Markdown") that inject a structured prompt with YAML front-matter template.
* [REQ-619] Tabbed Document Interface: The center panel shall support multiple open documents as tabs. Clicking a file opens it in a new tab; middle-click or close button closes tabs.

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

### LLM tools

| Tool | Description |
|---|---|
| `grep` | Search for a specific pattern within files across all libraries. |
| `read_tags` | Read all unique tags from markdown front-matter across all libraries. |
| `list_files_by_tag` | List files that contain a specific tag in their front-matter. |
| `list_files` | List markdown files in a directory (non-recursive). With "/" or "." returns library names. |
| `read_file` | Read the entire text contents of a file. |
| `read_file_lines` | Read specific line numbers or ranges from a file (1-indexed). |
| `create_file` | Create a new markdown file with the specified content. |
| `insert_lines` | Insert new lines of text into an existing file at a specific 1-indexed position. |
| `delete_lines` | Delete specific lines from a file (1-indexed, inclusive). |
| `replace_text` | Replace exact occurrences of old_string with new_string in a file. |
| `web_fetch` | Fetch content from a URL and convert HTML to Markdown. |
| `web_search` | Search the web using SearXNG. Requires searxng_url config. |
| `web_delegate` | Delegate complex web research to a sub-agent with web_fetch/web_search tools. |
| `read_yaml_header` | Parse a YAML header from a markdown file and return its content. |
| `write_yaml_header` | Write or update data in a YAML header to a markdown file. |
| `search_calendar` | Search calendar events by keyword. Requires CalDAV config. |
| `get_calendar` | Get calendar items by date range. Requires CalDAV config. |
| `get_calendar_item` | Get a specific calendar item by its full href. Requires CalDAV config. |
| `add_calendar_item` | Add a new calendar item. Requires CalDAV config. |
| `update_calendar_item` | Update a calendar item. Requires CalDAV config. |
| `delete_calendar_item` | Delete a calendar item. Requires CalDAV config. |
| `search_email` | Search email by keyword, folder, date range, sender, recipient, unread, or flagged status. Results are paginated (default page size 10); every response includes total for follow-up page requests. Requires JMAP config. |
| `get_email_by_id` | Get email by id. Requires JMAP config. |
| `get_email` | Get email by date range, sender, recipient, unread, or flagged status. Requires JMAP config. |
| `send_email` | Send an email. Requires JMAP config. |
| `search_contact` | Search contacts by keyword. Requires JMAP config. |
| `get_contact` | Get contact by id. Requires JMAP config. |
| `add_contact` | Add a new contact. Requires JMAP config. |
| `add_rows` | Add rows to a CSV file database. |
| `delete_rows` | Delete rows from a CSV file database based on a predicate. |
| `create_csv` | Create a new CSV file database with specified headers. |
| `list_csv` | List all CSV file databases. |
| `query` | Query a CSV file database using an evalexpr predicate, supporting sum and average aggregates. |

### CSV Database Tools

* [REQ-650] Tool Availability: The CSV database tools (`add_rows`, `delete_rows`, `create_csv`, `list_csv`, `query`) shall only be offered to the LLM if the user's query contains any of the tool names, "table", "csv", or "database".
* [REQ-651] Query Evaluation: The `query` tool shall use the `evalexpr` crate to parse and execute query predicates as dynamic expressions against CSV rows.
* [REQ-652] Aggregate Functions: The query system shall allow `sum` and `average` as aggregate functions over a specified column.
* [REQ-653] The system shall store all csv databases in a user specified location. Default to %APPDATA%\fastmd\db\ if not configured.


### YAML frontmatter template

```yaml
---
title: A brief title
summary: A three sentence summary of the contents
tags: ["tag1","tag2"]
header-date: RFC 3339 timestamp
---
```

## Key Findings / Recommendations
- **Thread Pool Coordination**: Wrapping a receiver channel in an `Arc<Mutex<Receiver<PathBuf>>>` is the recommended method to implement a shared work queue in Rust standard library.
- **Scroll Alignment**: Using `response.scroll_to_me(Some(egui::Align::TOP))` provides optimal navigation within `egui::ScrollArea` components.
- **UNC Stripping**: Windows standard canonicalization returns path prefixes starting with `\\?\`, which should be removed before visual tree rendering to maintain clean layout headers.

---

## Sources
- RFC 2119 Key Words Reference: [ietf.org/rfc/rfc2119.txt](https://www.ietf.org/rfc/rfc2119.txt)
- Egui Documentation: [github.com/ocornut/egui](https://github.com/ocornut/egui)
- Notify Crate API Documentation: [docs.rs/notify](https://docs.rs/notify)
- Rust Standard Library Threading Models: [doc.rust-lang.org/std/thread/](https://doc.rust-lang.org/std/thread/)
