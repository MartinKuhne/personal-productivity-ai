# SPEC.md: Application Logic Technical Specification

> **GUARDRAIL**: This specification file is managed by the spec-split workflow. Do not edit
> this file directly unless explicitly instructed. Any changes to requirements must be
> reflected in the corresponding implementation code. If drift is detected between
> this spec and the actual code behavior, notify the user immediately.

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

