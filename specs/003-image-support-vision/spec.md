# Feature Specification: Image Support (Vision)

**Feature Branch**: `[003-image-support-vision]`

**Created**: 2026-07-19

**Status**: Draft

**Input**: User description: "### Image Support (Vision) ..."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Initial Image Discovery & Analysis (Priority: P1)

When the system indexes the library, it should find all images and trigger vision analysis for them, creating markdown files that describe the images. This provides value by making images searchable via their text descriptions without exposing raw images to LLM tools.

**Why this priority**: Core functionality for enabling image processing and discovery.

**Independent Test**: Can be tested by adding a directory with an image, running initial indexing, and verifying a markdown file is created containing the vision analysis.

**Acceptance Scenarios**:

1. **Given** an image file without a corresponding markdown file, **When** indexing occurs, **Then** the image is queued for vision analysis.
2. **Given** an image file, **When** vision analysis completes successfully, **Then** a `.md` file is created with the description and picked up by the file watcher.

---

### User Story 2 - Image Update Detection (Priority: P1)

When an existing image is updated on disk, the system should re-analyze it if its modified timestamp is newer than its corresponding markdown file.

**Why this priority**: Ensures that descriptions remain accurate as files change.

**Independent Test**: Can be tested by modifying an existing image file and verifying that the vision analysis is re-triggered and the markdown file is updated.

**Acceptance Scenarios**:

1. **Given** an image file with an older corresponding markdown file, **When** the file system notifies of a change (or during re-index), **Then** the image is queued for vision analysis.

---

### User Story 3 - Hidden Images (Priority: P2)

Images should remain hidden from the UI and LLM file tools so that the system operates strictly on the text descriptions.

**Why this priority**: Maintains cleaner user experience and avoids unnecessary errors from LLMs trying to read binary files.

**Independent Test**: Can be tested by attempting to list files or view the directory tree, ensuring image files are not visible.

**Acceptance Scenarios**:

1. **Given** a configured image library, **When** viewing the directory tree or using LLM tools (grep, list_files, read_file), **Then** image files are NOT displayed or accessible.

### Edge Cases

- What happens when a vision model configuration is not available or correctly formatted?
- How does system handle failures during vision analysis (e.g., API timeout or rejection)?
- What happens if the generated markdown file is deleted but the image remains?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST scan all configured image libraries for image files (extensions: `.jpg`, `.jpeg`, `.png`, `.gif`, `.webp`, `.bmp`, `.tiff`, `.avif`) during initial indexing (REQ-301) and on file system change notifications (REQ-401).
- **FR-002**: System MUST hide image files from the directory tree, tab bar, and LLM file tools (grep, list_files, read_file, etc.).
- **FR-003**: System MUST check if a corresponding Markdown file exists for each discovered image file (same stem, `.md` extension).
- **FR-004**: System MUST queue the image for vision analysis if the Markdown file is missing or older than the image's last-modified timestamp.
- **FR-005**: System MUST support a `models` configuration section in `config.yaml` defining multiple models with `use_case` tags (`chat`, `embeddings`, `vision`) and an optional `cost` field.
- **FR-006**: System MUST invoke the model tagged with `vision` use_case, sending the image as a base64-encoded data URL, and request a detailed Markdown description.
- **FR-007**: System MUST write the generated Markdown description to the corresponding `.md` file on success (creating or overwriting), allowing the file watcher (REQ-403) to pick it up.
- **FR-008**: System MUST log errors to the Background Process Log (REQ-460) if vision analysis fails.
- **FR-009**: System MUST emit progress messages every 500 files or 5 seconds during initial indexing, reporting images found, and analyses queued/completed.
- **FR-010**: System MUST log file system events for image files to the Background Process Log with virtual path and event type.

### Key Entities

- **Image File**: A supported media file (`.jpg`, etc.) located in the configured libraries.
- **Vision Model Configuration**: Configuration determining which AI model performs the vision analysis based on `use_case: ["vision"]`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of discovered image files without up-to-date markdown descriptions are queued for vision analysis during indexing and file system change events.
- **SC-002**: Image files are completely invisible in the standard file UI and to LLM file tools.
- **SC-003**: Vision analysis successfully generates and saves markdown descriptions to the same directory for 100% of successful API calls.
- **SC-004**: Progress updates are reliably emitted every 500 files or 5 seconds during the initial scan.

## Assumptions

- Supported image extensions are `.jpg`, `.jpeg`, `.png`, `.gif`, `.webp`, `.bmp`, `.tiff`, `.avif`.
- The vision model configured is capable of parsing base64-encoded data URLs and outputting Markdown.
- File system change notifications correctly report modification timestamps.
- Network connectivity is available to reach the vision model API.
