# Feature Specification: Batch Prompt Processing

**Feature Branch**: `001-batch-prompt-processing`

**Created**: 2026-07-20

**Status**: Draft

**Input**: User description: "### Batch processing

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
* [REQ-813] While processing is underway, the [batch prompt processing dialog] shall stop processing new prompts when the user clicks the 'Cancel' button "

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Open Batch Processing Dialog (Priority: P1)
User opens the batch prompt processing dialog from the main navigation to configure a batch job.

**Why this priority**: This is the entry point for the entire batch processing feature. Without access to the dialog, no batch processing can occur.

**Independent Test**: User clicks "Batch ..." button in top navigation → dialog opens with all configuration options visible → user can interact with all dialog controls.

**Acceptance Scenarios**:
1. **Given** the application is running with the main window visible, **When** the user clicks the "Batch ..." button in the top navigation/menu bar, **Then** the batch prompt processing dialog opens and displays all configuration options (directory selector, wildcard pattern, prompt selector, batch mode selector, concurrency dropdown, Cancel and Process buttons).

### User Story 2 - Configure and Execute File Mode Batch Processing (Priority: P1)
User configures and runs a batch job in File mode to process multiple files with a selected prompt.

**Why this priority**: File mode is the primary batch processing mode for processing individual files matching a pattern.

**Independent Test**: User selects directory, enters wildcard pattern, selects prompt, chooses File mode, sets concurrency, clicks Process → system processes each matching file with the selected prompt concurrently up to the limit → logs show start/end for each file → Process button disabled during processing → Cancel stops new processing.

**Acceptance Scenarios**:
1. **Given** the batch dialog is open, **When** user selects a directory, enters "*.md" as wildcard pattern, selects a prompt tagged "prompt", chooses "File" batch mode, sets concurrency to "3", **Then** all selections are retained and Process button is enabled.
2. **Given** a valid File mode configuration, **When** user clicks Process, **Then** system adds each matching file's context to system context and processes the prompt once per file, running up to 3 concurrent operations.
3. **Given** processing is underway, **When** user observes the background log window, **Then** log shows start and end entries for each file's LLM processing.
4. **Given** processing is underway, **When** user clicks Cancel, **Then** no new file processing starts, currently running operations complete, and dialog closes.

### User Story 3 - Configure and Execute Directory Mode Batch Processing (Priority: P1)
User configures and runs a batch job in Directory mode to process directories with a selected prompt.

**Why this priority**: Directory mode is the secondary batch processing mode for processing entire directories.

**Independent Test**: User selects directory, chooses Directory mode (wildcard hidden), selects prompt, sets concurrency, clicks Process → system processes each subdirectory with the selected prompt → logs show start/end for each directory.

**Acceptance Scenarios**:
1. **Given** the batch dialog is open, **When** user selects "Directory" batch mode, **Then** the wildcard pattern field is hidden and its value is ignored during processing.
2. **Given** a valid Directory mode configuration, **When** user clicks Process, **Then** system adds each subdirectory's context to system context and processes the prompt once per directory, running up to the configured concurrency limit.
3. **Given** Directory mode processing is underway, **When** user observes the background log window, **Then** log shows start and end entries for each directory's LLM processing.

### User Story 4 - Cancel Batch Processing Dialog (Priority: P2)
User cancels the dialog before or during processing without any side effects.

**Why this priority**: Cancel provides safe exit and abort capability, essential for user control.

**Independent Test**: User opens dialog, makes selections, clicks Cancel → dialog closes, no processing occurs, no files modified. During processing, user clicks Cancel → new processing stops, dialog closes.

**Acceptance Scenarios**:
1. **Given** the batch dialog is open with any configuration, **When** user clicks Cancel before processing starts, **Then** dialog closes immediately with no processing initiated and no files modified.
2. **Given** batch processing is actively running, **When** user clicks Cancel, **Then** no new prompt processing tasks are started, currently executing tasks complete, and dialog closes.

### User Story 5 - Concurrency Control (Priority: P2)
User controls how many prompts are processed simultaneously to manage system resources.

**Why this priority**: Concurrency control prevents resource exhaustion and allows users to balance speed vs. resource usage.

**Independent Test**: User sets concurrency to 1, 4, and 8 in separate runs → observes that at most N prompts process concurrently.

**Acceptance Scenarios**:
1. **Given** batch dialog is open, **When** user opens concurrency dropdown, **Then** options 1 through 8 are available.
2. **Given** concurrency is set to N, **When** batch processing runs, **Then** at most N prompts are processed concurrently at any time.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST display a "Batch ..." button on the top navigation/menu bar. (REQ-800)
- **FR-002**: System MUST open the batch prompt processing dialog when the user clicks the "Batch ..." button. (REQ-801)
- **FR-003**: The batch prompt processing dialog MUST allow the user to select a directory from the available directories to process files in. (REQ-802)
- **FR-004**: The batch prompt processing dialog MUST allow the user to specify a wildcard pattern of file names to process. (REQ-803)
- **FR-005**: The batch prompt processing dialog MUST allow the user to select a prompt from a list of available prompts. Prompts are markdown files tagged with "prompt". (REQ-804)
- **FR-006**: The batch prompt processing dialog MUST allow the user to choose between two batch modes: "File" and "Directory". (REQ-805)
- **FR-007**: When batch mode is "Directory", the batch prompt processing dialog MUST hide the wildcard pattern field and ignore its value during processing. (REQ-806)
- **FR-008**: The batch prompt processing dialog MUST allow the user to select a processing concurrency number via a dropdown with options 1 through 8. The system MUST process that number of prompts concurrently. (REQ-807)
- **FR-009**: When the user clicks "Cancel" in the batch prompt processing dialog before processing starts, the system MUST close the dialog with no action taken and no files modified. (REQ-808)
- **FR-010**: When the user clicks "Process" in File batch mode, the system MUST add each matching file's context to the system context and process the selected prompt once per file. (REQ-809)
- **FR-011**: When the user clicks "Process" in Directory batch mode, the system MUST add each directory's context to the system context and process the selected prompt once per directory. (REQ-810)
- **FR-012**: The batch prompt processing dialog MUST log the start and end of LLM processing for each file/directory to the background log window. (REQ-811)
- **FR-013**: While batch processing is underway, the batch prompt processing dialog MUST disable the "Process" button. (REQ-812)
- **FR-014**: While batch processing is underway, when the user clicks "Cancel", the system MUST stop submitting new prompts for processing (currently running prompts may complete). (REQ-813)

### Key Entities

- **Batch Prompt Processing Dialog**: The modal dialog that allows users to configure and execute batch prompt processing jobs. Contains controls for directory selection, wildcard pattern, prompt selection, batch mode, concurrency, and action buttons.
- **Available Directories**: The set of directories the user can choose from for batch processing. Source of these directories is an existing system concept (assumed to be project directories or workspace folders).
- **Prompt**: A markdown file tagged with "prompt" that contains the prompt template to be executed for each file or directory in the batch.
- **Batch Mode**: An enumeration with two values: "File" (process each matching file individually) and "Directory" (process each directory as a unit).
- **Concurrency Level**: An integer from 1 to 8 controlling how many prompt executions run in parallel.
- **Background Log Window**: An existing system component that displays logging output, including batch processing start/end events.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: User can open the batch prompt processing dialog from the top navigation in under 1 second.
- **SC-002**: User can configure a complete batch job (select directory, pattern, prompt, mode, concurrency) in under 30 seconds.
- **SC-003**: In File mode, system processes all matching files with the selected prompt, executing up to the configured concurrency limit in parallel.
- **SC-004**: In Directory mode, system processes all subdirectories with the selected prompt, executing up to the configured concurrency limit in parallel.
- **SC-005**: Background log window shows a start and end entry for each file/directory processed, with timestamps.
- **SC-006**: Process button is disabled within 100ms of processing start and remains disabled until processing completes or is cancelled.
- **SC-007**: Clicking Cancel before processing starts closes dialog immediately with zero file modifications.
- **SC-008**: Clicking Cancel during processing stops new task submission within 500ms; dialog closes after in-flight tasks complete.
- **SC-009**: Wildcard pattern field is hidden within 100ms of switching to Directory mode and shown within 100ms of switching to File mode.
- **SC-010**: Concurrency dropdown presents exactly 8 options (1 through 8) and defaults to a reasonable value (e.g., 4).

---

## Assumptions

- The application already has a concept of "available directories" (e.g., project folders, workspace roots) that can be enumerated for the directory selector.
- The application already has a mechanism to discover and list markdown files tagged with "prompt" for the prompt selector.
- The application already has a "background log window" component that can receive and display log entries with timestamps.
- The application already has a "system context" mechanism that can accept file or directory context for LLM processing.
- The application already has an LLM processing pipeline that can execute prompts with given context.
- The "Batch ..." button appears in the existing top navigation/menu bar structure.
- Batch processing operates on the local filesystem; no remote/cloud storage is in scope.
- Wildcard patterns use standard glob syntax (e.g., "*.md", "src/**/*.txt").
- Concurrency limit of 8 is a reasonable upper bound for local LLM processing.
- Cancel during processing allows in-flight operations to complete (graceful cancellation) rather than forcing immediate termination.

---

## Edge Cases

- What happens when no directories are available? → Directory selector shows empty state; Process button disabled until valid directory selected.
- What happens when no prompts with "prompt" tag exist? → Prompt selector shows empty state; Process button disabled until valid prompt selected.
- What happens when wildcard pattern matches no files in File mode? → Processing completes with zero files processed; log shows no file entries; user informed via UI.
- What happens when Directory mode is selected but selected directory has no subdirectories? → Processing completes with zero directories processed; log shows no directory entries.
- What happens if LLM processing fails for a file/directory? → Error logged to background log; processing continues for remaining items; user can review errors in log.
- What happens if user closes dialog via window close button (X) instead of Cancel? → Same behavior as Cancel (no action, no modifications).
- What happens if user changes configuration while processing? → Configuration controls disabled during processing; changes not allowed until processing completes or is cancelled.
- What happens if concurrency is set higher than available CPU cores? → System still respects the configured limit; OS handles thread scheduling.