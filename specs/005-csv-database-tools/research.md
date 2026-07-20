# Research Notes: CSV Database Tools

## 1. Processing Architecture (In-Memory vs Streaming)
- **Decision**: In-Memory processing.
- **Rationale**: The performance requirement is evaluating 10,000 rows in <1 second. 10k rows of typical CSV data easily fits into memory (<10MB). Loading the entire CSV into memory simplifies `evalexpr` application, aggregation functions (`sum`, `average`), and schema validation.
- **Alternatives considered**: Streaming (reading row-by-row, evaluating, writing out to temp file). More complex and unnecessary for the targeted scale, though better for massive datasets.

## 2. Cross-Platform AppData Resolution
- **Decision**: Use the `directories` crate (or equivalent if already in project) to resolve user data directories, falling back to reading the `APPDATA` env var on Windows.
- **Rationale**: The spec specifies `%APPDATA%\fastmd\db\`. For true cross-platform behavior, we must map this to `~/.config/fastmd/db/` on Unix, which standard rust crates handle elegantly.
- **Alternatives considered**: Hardcoding `std::env::var("APPDATA")` which would break on macOS/Linux.
