# Implementation Plan: [FEATURE]

**Branch**: `[###-feature-name]` | **Date**: [DATE] | **Spec**: [link]

**Input**: Feature specification from `/specs/[###-feature-name]/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

The Image Support (Vision) feature enables the system to scan configured image libraries for image files and automatically queue them for vision analysis. The system uses a vision model to generate detailed Markdown descriptions of the images, which are saved in the same directory. Images remain hidden from the standard file UI and LLM tools, acting purely as inputs for the background analysis. The technical approach involves adding the `base64` crate to encode images and sending OpenAI-compatible vision payloads using the existing `ureq` HTTP client asynchronously via Tokio.

## Technical Context

**Language/Version**: Rust 2021 Edition

**Primary Dependencies**: `eframe`, `tokio`, `ureq`, `notify`, `base64` (to be added)

**Storage**: Local Filesystem (saving `.md` files)

**Testing**: `cargo test`

**Target Platform**: Desktop (Windows/macOS/Linux)

**Project Type**: Desktop application (`fastmd`)

**Performance Goals**: Background processing of images must not block the main UI thread or the Tokio event loop.

**Constraints**: API rate limits for vision models must be respected; graceful failure and logging to Background Process Log required.

**Scale/Scope**: Processing arbitrary numbers of images found in local directories during initial index or file system events.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Testability**: The vision processing logic will be modular and decoupled from the UI, enabling isolated testing of the API client and file watcher logic.
- **Security**: Base64 encoding and API payloads will be handled securely, ensuring valid JSON and proper MIME types. No arbitrary file execution.
- **Modularity**: Background vision processing will be its own module (`background::vision_processor`), similar to `pdf_converter`.
- **Open Source Leverage**: We are using standard, well-tested crates (`base64`, `ureq`, `notify`) instead of reinventing the wheel.
- **SDLC Best Practices**: Requirements are clear, and changes will be test-driven without introducing warnings.

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
src/desktop/src/
├── background/
│   ├── mod.rs               # Export new vision_processor
│   ├── models.rs            # Define ImageJob if necessary
│   └── vision_processor.rs  # New background processor for image analysis
├── background_task.rs       # File watcher updates to detect images
├── ui/
│   └── panels/
│       └── tree.rs          # Update to hide image files
└── config.rs                # Validation for vision models
```

**Structure Decision**: The logic will reside entirely within the `src/desktop` Rust crate, specifically adding a new background processor in `src/desktop/src/background/vision_processor.rs` to handle image analysis, and updating `background_task.rs` to queue these images.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
