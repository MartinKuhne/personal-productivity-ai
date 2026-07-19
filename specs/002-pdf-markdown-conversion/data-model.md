# Data Model: PDF to Markdown Conversion

## Entities

### `LogCategory` (Enum)
Represents the source of a background log entry.
- `Indexer`
- `Watcher`
- `PdfConverter`
- `ImageVision`
- `LlmTools`

### `BackgroundLogEntry` (Struct)
Represents a single log line.
- `timestamp`: `chrono::DateTime<chrono::Local>` or `String` (formatted HH:MM:SS.mmm)
- `category`: `LogCategory`
- `message`: `String`

### `BackgroundProcessManager` (Struct/State)
Maintains the state of background processes and their logs.
- `logs`: `std::collections::VecDeque<BackgroundLogEntry>` (Cap at 10,000)
- `filter_category`: `Option<LogCategory>` (For UI filtering)
- `search_text`: `String` (For UI filtering)
- `auto_scroll`: `bool` (State of user scrolling)

### `PdfConversionJob` (Struct)
Represents an ongoing conversion task.
- `input_pdf`: `std::path::PathBuf`
- `output_md`: `std::path::PathBuf`
- `queued_at`: `chrono::DateTime<chrono::Utc>`

### Config Updates
- The main `AppConfig` struct needs a new field: `pdf_converter_command: Option<Vec<String>>` (e.g. `["pandoc", "-f", "pdf", "-t", "markdown", "-o", "{output}", "{input}"]`).
