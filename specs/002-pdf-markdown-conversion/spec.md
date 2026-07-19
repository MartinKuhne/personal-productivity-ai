# Feature Specification: PDF to Markdown Conversion

**Feature Branch**: `[###-pdf-markdown-conversion]`

**Created**: 2026-07-19

**Status**: Draft

**Input**: User description: PDF to Markdown Conversion requirements...

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Automatic PDF to Markdown Conversion (Priority: P1)

As a user, I want any PDF files I place in the configured text libraries to be automatically converted to Markdown, so I can seamlessly analyze their text content without dealing with PDF tools.

**Why this priority**: Core value of the feature, ensuring all new and existing PDFs become analyzable without manual intervention.

**Independent Test**: Can be fully tested by placing a dummy PDF in a watched folder, ensuring a corresponding Markdown file is generated, and verifying no PDF files appear in the UI.

**Acceptance Scenarios**:

1. **Given** the app is running and monitoring text libraries, **When** a new PDF is added, **Then** a background process queues it and generates a corresponding `.md` file, which is then indexed.
2. **Given** a PDF is already in the folder, **When** the app runs initial indexing, **Then** the PDF is discovered, queued, and converted if no matching `.md` file exists or if the `.md` is older than the `.pdf`.
3. **Given** a user is viewing the directory tree or using LLM tools, **When** they look for files, **Then** `.pdf` files are completely hidden from the UI and tool results.

---

### User Story 2 - Background Process Visibility (Priority: P2)

As a user, I want to see the progress and output of background tasks like initial indexing, file watcher events, and PDF conversion, so I can understand what the system is doing and debug any failures.

**Why this priority**: Essential for observing the background conversion processes and system state.

**Independent Test**: Can be tested by verifying the new "Background Processes" tab appears on startup, auto-scrolls with new log entries, and correctly persists to `logs/background-process.log`.

**Acceptance Scenarios**:

1. **Given** the app starts and begins indexing, **When** the first background task fires, **Then** the Background Processes tab opens automatically.
2. **Given** the Background Processes tab is open with existing logs, **When** a user manually scrolls up, **Then** auto-scroll pauses.
3. **Given** the user interacts with the filter controls in the Background Processes tab, **When** they select a process category or type a search string, **Then** the log entries are filtered accordingly.

### Edge Cases

- What happens when PDF conversion fails? (The system logs the error to the Background Process Log and does not create the `.md` file).
- What happens when a user modifies a PDF? (The file watcher detects the modification and queues a re-conversion since the `.pdf` will be newer than the `.md`).
- What happens if the `pdf_converter_command` in `config.yaml` is invalid or the executable is missing? (The background conversion process fails and logs the execution error).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST scan all configured text libraries for `.pdf` files during initial indexing and on file system change notifications.
- **FR-002**: The system MUST NOT display `.pdf` files in the directory tree, tab bar, or expose them to LLM tools (they must remain hidden).
- **FR-003**: The system MUST check if a corresponding Markdown file exists for each discovered PDF (same stem, `.md` extension, same directory).
- **FR-004**: The system MUST queue a PDF for conversion if the Markdown file is missing or its last-modified timestamp is older than the PDF.
- **FR-005**: The system MUST provide a `pdf_converter_command` in `config.yaml` to specify the converter executable and arguments.
- **FR-006**: The system MUST execute the converter as a background process and capture stdout/stderr to the Background Process Log.
- **FR-007**: The system MUST ensure that successfully generated `.md` files are picked up by the normal file watcher and indexed.
- **FR-008**: The system MUST emit progress messages every 500 files or 5 seconds during initial indexing (files processed, PDFs found, conversions queued/completed).
- **FR-009**: The system MUST log file watcher events (create, modify, delete, rename) with virtual path and event type to the Background Process Log.
- **FR-010**: The system MUST provide a "Background Processes" tab displaying real-time output from background tasks (Initial Indexing, File Watcher, PDF Converter, Image Vision, LLM Tools).
- **FR-011**: The system MUST open the Background Processes tab automatically on the first background task and allow reopening via [View] → [Background Processes].
- **FR-012**: The system MUST format each log entry with timestamp (HH:MM:SS.mmm), process category, and message.
- **FR-013**: The system MUST provide filter controls for process category and text search in the log tab.
- **FR-014**: The system MUST retain the last 10,000 log entries in memory and write them to `logs/background-process.log` on application exit.
- **FR-015**: The system MUST auto-scroll the log tab to the newest entry unless manually scrolled up by the user.

### Key Entities

- **PDF Converter Process**: Represents the background execution of the external command, tracking exit codes, standard output, and standard error.
- **Background Log Entry**: Represents a single log line in the UI, containing timestamp, category, and message string.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of PDF files placed in monitored directories are successfully converted to Markdown (assuming valid PDFs and valid converter).
- **SC-002**: The Background Processes tab handles streaming up to 1,000 logs per second without blocking the main UI thread.
- **SC-003**: Background log persistence writes to disk upon exit consistently, retaining exactly the last 10,000 entries.
- **SC-004**: PDF files are completely excluded from 100% of LLM tools and directory views.

## Assumptions

- Users have a compatible external PDF converter (e.g., `pandoc`) installed and correctly specified in `config.yaml`.
- Markdown output generated by the external command does not require further post-processing before indexing.
- The external PDF to Markdown converter supports accepting input file path and output file path as arguments.
