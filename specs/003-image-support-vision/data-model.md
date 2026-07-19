# Data Model: Image Support (Vision)

## Entities

### `LlmConfig` (Existing, Update)
- **Modifications**: The `use_case` field must be checked for `"vision"`. (It is already supported by the struct, but validation logic must be added to ensure the system gracefully handles the lack of a vision model).
- **Validation**: Ensure that when vision processing is triggered, it searches for a model tagged with `"vision"`.

### `ImageJob` (New)
Represents a queued image for vision analysis in the background task.
- `image_path`: `PathBuf` - Absolute path to the image file.
- `md_path`: `PathBuf` - Absolute path to the destination markdown file.

## State Transitions (Image Analysis)
1. **Discovered**: Image file found by initial scan or file watcher.
2. **Queued**: Added to the `vision_processor` channel for processing.
3. **Processing**: Sending request to API.
4. **Completed**: Markdown file successfully written.
5. **Failed**: Error logged to `BackgroundProcessLog`.
