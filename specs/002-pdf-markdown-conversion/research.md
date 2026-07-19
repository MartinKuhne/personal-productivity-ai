# Research: PDF to Markdown Conversion

## Decisions & Alternatives

### 1. Asynchronous External Process Execution
- **Decision**: Use `tokio::process::Command` to execute the PDF converter command asynchronously.
- **Rationale**: `tokio` is already in the project dependencies and provides non-blocking process execution, allowing us to wait for exit status and capture stdout/stderr lines concurrently.
- **Alternatives considered**: `std::process::Command` (rejected because it blocks the thread, which would either require a dedicated OS thread per conversion or block the UI/async runtime).

### 2. UI Updates from Background Tasks
- **Decision**: Use `std::sync::mpsc` or `tokio::sync::mpsc` to send log entries from the background tokio tasks to the eframe UI thread, and call `ctx.request_repaint()` whenever a new log arrives.
- **Rationale**: `eframe` is immediate mode. It doesn't update unless repainted. Background tasks must notify the UI to repaint when they emit a new log line.
- **Alternatives considered**: Polling a shared `Arc<Mutex<VecDeque>>` on every frame (inefficient, requires constant repainting or missing updates).

### 3. Log Persistence on Exit
- **Decision**: Implement the `on_exit` trait method in `eframe::App` (or equivalent graceful shutdown hook) to flush the in-memory `VecDeque` of logs to `logs/background-process.log` using standard blocking file I/O.
- **Rationale**: Since the app is exiting, doing a quick synchronous write of up to 10,000 log lines is fast and ensures data is written before process termination.
- **Alternatives considered**: Streaming logs to disk continuously (rejected as it causes unnecessary disk I/O, requirement says "On application exit").

### 4. PDF Exclusion Strategy
- **Decision**: Update the existing file discovery/walking logic (using `walkdir` or internal file indexer) to explicitly ignore files with `.pdf` extension for the purposes of the UI tree and LLM context.
- **Rationale**: Safest way to ensure it never leaks to the UI or LLMs.
- **Alternatives considered**: Filter them out at the rendering level (rejected because LLMs might still see them in the data model).
