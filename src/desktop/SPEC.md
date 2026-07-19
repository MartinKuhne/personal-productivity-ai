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
* [REQ-151] When the user right-clicks on file or folder, the context menu appears
* [REQ-152] When the user selects [Edit] from the context menu, and the object under the mouse cursor is a file, it opens in the system default editor
* [REQ-153] When the user selects [Delete] from the context menu, the file or folder gets moved to the recycle bin
* [REQ-154] When the user selects [Show in File Explorer] from the context menu, the system opens the system file exporer with the directory that contains the file
* [REQ-155] When the user selects [Move] from the context menu, the system shows a modal dialog containing all the known folders as well as 'Ok' and 'Cancel' buttons. When the user selects a folder and then 'Ok' the system moves the file to that folder, then closes the dialog. When the user selects 'Cancel' the dialog closes and the file is not moved or changed.
* [REQ-156] When the user selects [Create Directory ...] from the context menu, the system opens a modal dialog for the user to enter a directory name, as well as 'Ok' and 'Cancel' buttons. When the user enters a valid folder name and then clicks 'Ok' the system creates the directory, then closes the dialog. When the user selects 'Cancel' the dialog closes and no side effects occur
* [REQ-157] When the user selects [Rename] from the context menu, the system shows a modal dialog containing the current file name as well as 'Ok' and 'Cancel' buttons. When the user makes changes to the file name and then clicks 'Ok' or presses the enter key, the system renames the file or folder, then closes the dialog. When the user selects 'Cancel' the dialog closes and the file is not moved or changed.
* [REQ-158] When the user selects [Copy path] from the context menu, the system copies the fully qualified file or directory name to the clipboard
* [REQ-159] When the user selects [Print] from the context menu, and the item under the mouse cursor is a file, the system prints the page using the windows system print dialog
* [REQ-160] When the user selects [New document] from the context menu, and the item under the mouse cursor is a directory, the system creates a document containing the yaml markdown header and the name 'New document.md'. If a file with that name exist, add the current date and time do the document name until a unique file name is generated.
* [REQ-170] The left column shall increase in size to display any one item without line breaks, to use up to 20% of the available width
* [REQ-171] On every level of the directory tree, directories appear before files
* [REQ-172] The directory tree should not display folders that contain no markdown files
* [REQ-173] When the user selects [Format Markdown] from the context menu, the system executes the Format Markdown quick task as described elsewhere

* [REQ-180] When the user holds the shift key, the system shall allow the user to select multiple documents
* [REQ-181] When the user has selected multiple documents, and they right click on one of the selected documents, the [multi select context menu] is shown
* [REQ-182] When the user selects [Merge] from the [multi select context menu], the system shall run a new LLM prompt instructing the LLM to merge the content into a new document. 
* [REQ-183] When the user selects [Delete] from the [multi select context menu], the system shall move all the selected files to the recycle bin

### Middle column / File viewer area

### 2. Markdown Parser & Rendering Engine
* [REQ-201] GFM Parsing: The Markdown parser shall support the GitHub Flavored Markdown (GFM) tables extension.
* [REQ-202] Heading Sizing: The FastMD Viewer shall render H1, H2, and H3 headings at 24px, 20px, and 17px respectively.
* [REQ-203] Break Semantics:
    * [REQ-204]: When a single carriage return is encountered, the Markdown parser shall render it as a spacing character.
    * [REQ-205]: When two trailing spaces followed by a newline are encountered, the Markdown parser shall force a line split.
* [REQ-206] Bullet Lists: The FastMD Viewer shall render list items with indentation and bullet points (`•`).
    * [REQ-207]: If a list item contains hard breaks, then the FastMD Viewer shall render the bullet only on the first line.
* [REQ-208] Table Layout:
    * [REQ-209]: The FastMD Viewer shall render tables inside a rounded visual frame with a distinct background.
    * [REQ-210]: The FastMD Viewer shall arrange cells in an `egui::Grid` with striped rows and column spacing.
    * [REQ-211]: The FastMD Viewer shall render header row cells with a bold font weight.
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
    * [REQ-404]: When a file is deleted or renamed, the FastMD Viewer shall remove the document from the tree and tag lists.
    * [REQ-405]: When the active document is modified, the FastMD Viewer shall hot-reload and redraw it immediately.
    * [REQ-406]: If the active document is deleted, then the FastMD Viewer shall reset the viewer pane.

### 5. CLI & Deployment
* [REQ-501] CLI Directory Input: The FastMD Viewer shall accept a workspace directory path as its first command-line argument.
    * [REQ-502]: If the provided workspace path does not exist, then the FastMD Viewer shall fallback to the current working directory.
* [REQ-503] UNC Path Normalization: The FastMD Viewer shall normalize and strip UNC prefixes (`\\?\`) from Windows paths.
* [REQ-504] Deployment Binary: The build system shall include a release-deployment binary target (`deploy`) that compiles and deploys the optimized binary to `C:\tools\fastmd.exe` when invoked via `cargo run --bin deploy`.

### 6. Local LLM Interface & Tool Call Agent
* [REQ-601] OpenAI Compatible Endpoint: The FastMD Viewer shall support connections to an OpenAI compatible chat completions API.
* [REQ-602] Default Settings: The FastMD Viewer shall default to the OpenRouter endpoint using the free model `google/gemini-2.5-flash:free`.
* [REQ-603] Configuration File: The FastMD Viewer shall parse a YAML configuration file (`config.yaml`) from the standard user configuration path to retrieve the API key, model, and endpoint URL.
    * [REQ-604]: If the configuration file does not exist, then the FastMD Viewer shall create a default template configuration file.
* [REQ-605] Monospace Command Prompt: When the bottom panel command entry field is submitted, the FastMD Viewer shall execute the command through the Local LLM completions thread.
* [REQ-606] LLM Tools Library: The LLM Agent shall utilize functional tools as per the [LLM Tools] section below
* [REQ-607] Real-time Stream Output: The FastMD Viewer shall display the LLM's active thinking sequence and render the final Markdown response in real-time inside the Central Panel.
* [REQ-608] Tool Invocation Logging: The system shall print tool call invocations with their significant parameters to the response window.
* [REQ-610] Active File Context: When the user sends an AI prompt and there is a file being displayed in the middle pane, the system shall send the full path of that file with the system prompt.
* [REQ-611] Active Directory Context: When the user selects a directory from the left pane, it becomes the directory context for the AI prompt. When the user sends an AI prompt and there is NO file being displayed in the middle pane, the system shall send the full path of the directory context with the system prompt.
* [REQ-612] Active Directory Context: When the user selects a directory from the left pane, it becomes the directory context for the AI prompt. The AI prompt shall display the directory context, relative to the base directory, with the prompt. Example: 'Users\Martin >'
* [REQ-620] When displaying tool call arguments, format the JSON

* [REQ-699] Cancel AI Prompt: While an AI prompt is being executed, the system shall display a stop button. When the user clicks the stop button, the system shall abort the prompt processing.

### Libraries

* [REQ-700] The system shall support multiple content libraries. The libraries have [root_folder, name, kind, readonly (optional, default true)] attributes
* [REQ-701] The system shall support a content library 'text'. The behaviours throughout this document apply to this type. The tools are markdown focused.
* [REQ-702] The system shall support a virtual file system. The virtual paths are composed of the library name, then the files and directrories present at the configured root_folder.
* [REQ-703] The Directory tree pane shall display the content library name for each library as the top level node
* [REQ-704] The file based tools shall take virtual paths as arguments, and shall resolve these paths to fully qualified file names for the underlying operating system.
* [REQ-705] The [grep] tool shall search all libraries, and return a concatendated result
* [REQ-706] When the [list_files] tool is invoked with the '/' or '.' argument alone, it shall enumerate the list of libraries, enabling the LLM to continue the folder search for the virtual library subfolders

### LLM tools

| Tool | Description |
|---|---|
| `grep` | Search for a specific pattern within files. |
| `read_tags` | Read tags associated with specific files. |
| `list_files_by_tag` | List files that match a specific tag or set of tags. |
| `list_files` | List all files within a specified directory. |
| `read_file` | Read the entire contents of any file. |
| `read_file_lines` | Read specific line numbers or ranges from any file. |
| `create_markdown_file` | Create a new markdown with the specified content. |
| `insert_lines` | Insert new lines of text into an existing file. |
| `delete_lines` | Delete specific lines from an existing file. |
| `web_fetch` | Fetch and retrieve content from a specified URL. |
| `web_search` | Perform a web search |
| `read_header` | Read and parse data from a YAML header to a markdown file. |
| `write_header` | Write or update data in a YAML header tp a markdown file. |

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
