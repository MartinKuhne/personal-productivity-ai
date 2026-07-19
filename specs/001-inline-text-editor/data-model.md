# Data Model: Inline Text Editor

## Entities

### `DocumentContent`
Represents the content of the file being edited.

**Fields**:
- `front_matter`: `Option<String>` - The raw YAML front-matter block, including the `---` delimiters.
- `body`: `String` - The Markdown content following the front-matter.

**Behaviors**:
- `parse(raw: &str) -> DocumentContent`: Splits the raw file content into front-matter and body.
- `to_string(&self) -> String`: Combines front-matter and body back into a single string for saving.

### `EditorState`
Represents the UI state of the inline editor overlay/modal.

**Fields**:
- `is_open`: `bool` - Whether the editor is currently visible.
- `content`: `String` - The current text in the editor (the Markdown body).
- `original_front_matter`: `Option<String>` - The saved front-matter to prepend on save.
- `file_path`: `PathBuf` - The path to the file being edited.
- `error_message`: `Option<String>` - Any parse error to display in the UI.

**Transitions**:
- `open(file_path, raw_content)`: Sets `is_open = true`, parses `raw_content` to set `content` and `original_front_matter`.
- `close()`: Sets `is_open = false`, clears state.
- `save()`: Validates `content`, if valid saves to `file_path` and `close()`, if invalid sets `error_message`.
