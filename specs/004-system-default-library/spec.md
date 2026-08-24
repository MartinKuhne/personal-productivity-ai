# Feature Specification: Default System Library & Conversation Logging

**Feature Branch**: `004-system-default-library`

**Created**: 2026-08-24

**Status**: Draft

**Input**: User description: "implement all of ### Default library: [VFS-100] The system SHALL support a system library. [VFS-101] The default name of the system library SHALL be System. [VFS-102] The system SHALL support a permanent configuration option for the user to specify the display name of the system library. [VFS-103] The system SHALL store the files for the system library under %APPDATA%/fastmd/system on the Windows operating system. [VFS-104] When the %APPDATA%/fastmd/system on the Windows operating system does not exist, the system SHALL create it. [VFS-110] The system library SHALL support a Conversations folder. [VFS-111] When the user performs an agent prompt, the system SHALL log the prompt, the chat model response, and any further prompts and responses to a file in the Conversations folder. [VFS-112] When the system creates a file to log a prompt, the file name SHALL be YYYY-MM-DD HH-MM-SS.md. [VFS-113] When the system writes to a file to log a prompt, it shall use headings ## Prompt (nnn) and ## Response (nnn) with nnn representing a one based, incrementing number. [VFS-114] When the system logs to a prompt log file, it shall include any write tool calls at the end of the ## Response (nnn) section."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Automatic System Library Provisioning & Custom Naming (Priority: P1)

An end user launches FastMD. The system automatically provisions a default system library mapped to `%APPDATA%/fastmd/system` (creating the folder on disk if it does not yet exist). In the Left Pane's library/directory tree, the library appears with the default display name `System`. If the user specifies a custom display name in `config.yaml` (e.g. `system_library_name: "Personal Knowledge"`), FastMD displays the custom name as the root library node while continuing to map to `%APPDATA%/fastmd/system`.

**Why this priority**: Core prerequisite for storing system assets, conversation logs, and providing a permanent default library across sessions.

**Independent Test**: Can be tested by starting FastMD on a fresh machine/environment without `%APPDATA%/fastmd/system` present, verifying the directory is created and mounted as a text content library with display name `System` (or configured display name), and that notes inside it can be read/written via virtual paths.

**Acceptance Scenarios**:

1. **Given** no `%APPDATA%/fastmd/system` directory on Windows, **When** FastMD initializes configuration and libraries, **Then** `%APPDATA%/fastmd/system` is created on disk and added to content libraries as a writable `text` library with display name `System`.
2. **Given** a configuration file with `system_library_name: "My System"`, **When** FastMD starts, **Then** the system library is displayed with name `My System` pointing to `%APPDATA%/fastmd/system`.
3. **Given** existing content libraries in configuration, **When** FastMD loads, **Then** the system library is present alongside user-configured workspace libraries.

---

### User Story 2 - Automated Conversation Logging for Agent Prompts (Priority: P2)

An end user submits an agent prompt in the chat/prompt interface. FastMD automatically ensures a `Conversations` folder exists inside the system library (`%APPDATA%/fastmd/system/Conversations`). FastMD creates a new markdown file named `YYYY-MM-DD HH-MM-SS.md` based on the local session start timestamp. When the prompt and response are processed, FastMD logs `## Prompt (1)` followed by the user prompt, and `## Response (1)` followed by the assistant response. When subsequent prompts are submitted within the same conversation session, FastMD appends `## Prompt (2)`, `## Response (2)`, etc., into the same log file.

**Why this priority**: Ensures user conversations are permanently recorded and searchable in markdown without manual saving.

**Independent Test**: Can be tested by submitting multi-turn prompts through the agent orchestrator and verifying that a file with timestamp format `YYYY-MM-DD HH-MM-SS.md` is created in `Conversations/` containing incrementing `## Prompt (n)` and `## Response (n)` headers.

**Acceptance Scenarios**:

1. **Given** a new agent session, **When** the user submits a prompt and the LLM responds, **Then** a log file `YYYY-MM-DD HH-MM-SS.md` is created in `%APPDATA%/fastmd/system/Conversations/` containing `## Prompt (1)` with the prompt text and `## Response (1)` with the response text.
2. **Given** an ongoing session with an existing log file, **When** the user sends a follow-up prompt, **Then** `## Prompt (2)` and `## Response (2)` are appended to the existing file.

---

### User Story 3 - Logging Mutating Tool Calls in Conversation Log (Priority: P3)

During an agent turn, the model executes write/mutating tools (e.g. `create_note`, `patch_note`, `insert_into_note`, `move_note`). FastMD logs the write tool calls at the end of the `## Response (nnn)` section in the corresponding conversation log file, ensuring a complete audit trail of filesystem alterations executed by the agent.

**Why this priority**: Provides transparency and reproducibility for actions taken by the agent during turns.

**Independent Test**: Can be tested by running an agent turn that executes write tools, and asserting that the `## Response (nnn)` block in the log file ends with the formatted write tool invocations and details.

**Acceptance Scenarios**:

1. **Given** an agent turn where `create_note` or `patch_note` is executed, **When** the turn finishes and the log is updated, **Then** the end of `## Response (nnn)` contains the write tool call records.
2. **Given** an agent turn with only read-only tools (e.g. `search_notes`, `read_note`), **When** the turn finishes, **Then** no write tool block is appended to the response section.

---

### Edge Cases

- When `%APPDATA%/fastmd/system` does not exist, FastMD creates it along with the `Conversations` subfolder without raising I/O errors.
- If multiple turns occur within the same second, the existing log file for the session is retained and appended to.
- If a write tool execution fails or returns an error, the write tool call and its error result are still recorded in the response section.

## Requirements *(mandatory)*

### System Library Model & Storage (VFS-100..104)

- **[VFS-100]** The system SHALL support a system library.
- **[VFS-101]** The default name of the system library SHALL be `System`.
- **[VFS-102]** The system SHALL support a permanent configuration option for the user to specify the display name of the system library (`system_library_name` in `AppConfig` / `config.yaml`).
- **[VFS-103]** The system SHALL store the files for the system library under `%APPDATA%/fastmd/system` on the Windows operating system.
- **[VFS-104]** When `%APPDATA%/fastmd/system` on the Windows operating system does not exist, the system SHALL create it.

### Conversations Folder & Agent Logging (VFS-110..114)

- **[VFS-110]** The system library SHALL support a `Conversations` folder (`%APPDATA%/fastmd/system/Conversations`).
- **[VFS-111]** When the user performs an agent prompt, the system SHALL log the prompt, the chat model response, and any further prompts and responses to a file in the `Conversations` folder.
- **[VFS-112]** When the system creates a file to log a prompt, the file name SHALL be `YYYY-MM-DD HH-MM-SS.md` based on the local start time of the conversation session.
- **[VFS-113]** When the system writes to a file to log a prompt, it SHALL use headings `## Prompt (nnn)` and `## Response (nnn)` with `nnn` representing a 1-based incrementing integer (e.g. `## Prompt (1)`, `## Response (1)`, `## Prompt (2)`).
- **[VFS-114]** When the system logs to a prompt log file, it SHALL include any write tool calls at the end of the `## Response (nnn)` section.

## Key Entities & Data Model

- **`SystemLibraryConfig`**: Configuration options for the system library, including optional custom `system_library_name`.
- **`ConversationLogger` / `ConversationLogSession`**: Domain entity managing the lifecycle of an active session's conversation log file in `%APPDATA%/fastmd/system/Conversations/<YYYY-MM-DD HH-MM-SS>.md`, tracking turn count (`nnn`), formatting headers, responses, and write tool call blocks.
- **`WriteToolCallRecord`**: Representation of mutating tool calls executed during a turn (tool name, target path/args, status) to format into the response section.

## Success Criteria *(mandatory)*

- **SC-001**: Launching FastMD ensures `%APPDATA%/fastmd/system` and `%APPDATA%/fastmd/system/Conversations` exist and are accessible in the VFS as a text content library.
- **SC-002**: Setting `system_library_name` in `config.yaml` renames the display label of the system library across the UI and VFS while retaining the same root path.
- **SC-003**: 100% of agent sessions create or append to `Conversations/YYYY-MM-DD HH-MM-SS.md` with exact header syntax `## Prompt (n)` and `## Response (n)`.
- **SC-004**: Mutating tool calls executed during a turn appear at the end of the corresponding `## Response (n)` section in the log file.

## Assumptions

- `%APPDATA%/fastmd/system` is the root on Windows; on non-Windows platforms or test environments without APPDATA, standard fallbacks (`USERPROFILE` or test directories) are respected.
- The `Conversations` folder is automatically created inside the system library if it does not already exist when logging or initializing.
- The one-based incrementing number `nnn` format in headings is `1`, `2`, `3`, etc. (e.g. `## Prompt (1)`, `## Response (1)`).

