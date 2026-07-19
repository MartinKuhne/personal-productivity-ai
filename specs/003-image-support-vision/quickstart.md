# Quickstart Validation: Image Support (Vision)

## Prerequisites
1. Ensure `config.yaml` contains a model with `use_case: ["vision"]` and a valid API key.
2. Ensure you have an image file (e.g., `test.png`) ready.

## Scenario 1: Initial Discovery
1. Copy `test.png` into a directory configured as a content library while the app is closed.
2. Start the application (`cargo run`).
3. **Verify**:
   - The UI does not show `test.png` in the file tree.
   - The Background Process Log shows the image was queued and processed.
   - `test.md` is generated in the same directory, containing a Markdown description of the image.

## Scenario 2: File Watcher Update
1. With the application running, copy `another.jpg` into the content library.
2. **Verify**:
   - `another.md` is automatically created shortly after.
3. Modify `another.jpg` (e.g., replace it with a different image file but keep the same name).
4. **Verify**:
   - The Background Process Log indicates `another.jpg` was queued again due to the newer timestamp.
   - `another.md` is updated with a new description.
