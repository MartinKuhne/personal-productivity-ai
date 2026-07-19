# Quickstart & Validation

## Prerequisites
1. Ensure the Rust project is built: `cargo build`
2. Install `pandoc` locally on the system for testing PDF conversion (or use any dummy script that accepts `input` and `output`).
3. Set the configuration file `config.yaml` to have:
   ```yaml
   pdf_converter_command: ["pandoc", "-f", "pdf", "-t", "markdown", "-o", "{output}", "{input}"]
   ```

## Validation Scenarios

### Scenario 1: Initial Discovery and Conversion
1. Place a sample PDF file `test.pdf` into a monitored text library directory while the app is closed.
2. Ensure there is no `test.md` in that directory.
3. Start the application: `cargo run`.
4. Observe that the **Background Processes** tab opens automatically.
5. You should see log lines categorized as `PdfConverter` indicating that `test.pdf` is being converted.
6. Once conversion finishes, verify `test.md` appears in the folder.
7. Verify `test.pdf` is completely hidden from the directory tree in the UI.

### Scenario 2: File Watcher Queuing
1. While the app is running, copy a new PDF file `report.pdf` into the text library directory.
2. Look at the **Background Processes** tab.
3. Observe a `Watcher` event log, followed by `PdfConverter` starting a conversion.
4. Verify `report.md` is generated.

### Scenario 3: Log Persistence
1. Generate some background logs by performing file operations or adding PDFs.
2. Close the application gracefully.
3. Check the user config directory (or wherever `logs/background-process.log` is designated).
4. Verify the file exists and contains the latest log entries in plaintext.
