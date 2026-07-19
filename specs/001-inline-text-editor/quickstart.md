# Validation Guide: Inline Text Editor

## Prerequisites
- Rust toolchain installed (`cargo`).
- The repository cloned and built.

## Testing the Editor Toggle
1. Open the application configuration file (e.g., `config.yaml`).
2. Add or modify the setting: `inline_editor_enabled: true`.
3. Launch the app: `cargo run`.
4. Right-click or use the context menu on a `.md` file and select **Edit**.
5. The inline editor should appear instead of opening an external application.

## Testing Editing and Saving
1. Open the inline editor for a file containing front-matter and body text.
2. Verify only the Markdown body is visible in the editor.
3. Make a change to the text.
4. Click **Save**.
5. Open the file in an external text editor to verify that:
   - The changes to the body are present.
   - The front-matter remains exactly as it was.

## Testing Cancel
1. Open the inline editor and modify the text.
2. Click **Cancel**.
3. Verify the editor closes and the file on disk is unchanged.

## Testing Validation
1. Open the inline editor.
2. Enter intentionally broken syntax or a state that would fail validation (if any specific syntax is rejected by pulldown-cmark).
3. Click **Save**.
4. The save should fail, and an error message should display indicating the location of the error.
