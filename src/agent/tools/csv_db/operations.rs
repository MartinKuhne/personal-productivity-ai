//! CSV-database CRUD operations — create a new CSV database, add rows, list databases, and path resolution.
//!
//! Unit tests live in the sibling `operations_tests.rs` sidecar.

use super::schema::{AddRowsInput, CreateCsvInput, CsvDatabase, ListCsvInput};

use std::path::PathBuf;

pub fn get_db_dir(ctx: &crate::tools::context::ToolContext) -> PathBuf {
    let path = if let Some(ref override_path) = ctx.config.csv_db_path {
        PathBuf::from(override_path)
    } else {
        let app_data = std::env::var("APPDATA").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            format!("{}/.config", home)
        });
        let mut p = PathBuf::from(app_data);
        p.push("fastmd");
        p.push("db");
        p
    };

    let _ = ctx.vfs().create_dir_all(&path);
    path
}

pub fn get_db_path(ctx: &crate::tools::context::ToolContext, db_name: &str) -> PathBuf {
    let mut path = get_db_dir(ctx);
    path.push(format!("{}.csv", db_name));
    path
}

pub fn create_csv(
    ctx: &crate::tools::context::ToolContext,
    input: CreateCsvInput,
) -> Result<CsvDatabase, String> {
    let db_path = get_db_path(ctx, &input.db_name);
    if db_path.exists() {
        return Err(format!("Database '{}' already exists", input.db_name));
    }

    let mut wtr =
        csv::Writer::from_path(&db_path).map_err(|e| format!("Failed to create csv: {}", e))?;
    wtr.write_record(&input.headers)
        .map_err(|e| format!("Failed to write headers: {}", e))?;
    wtr.flush().map_err(|e| format!("Failed to flush: {}", e))?;

    Ok(CsvDatabase {
        name: input.db_name,
        path: db_path,
        headers: input.headers,
    })
}

pub fn list_csv(
    ctx: &crate::tools::context::ToolContext,
    _input: ListCsvInput,
) -> Result<Vec<CsvDatabase>, String> {
    let db_dir = get_db_dir(ctx);
    let mut dbs = Vec::new();

    if let Ok(entries) = ctx.vfs().read_dir(&db_dir) {
        for entry in entries {
            let path = entry.path;
            if path.extension().and_then(|e| e.to_str()) == Some("csv") {
                let name = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                if let Ok(mut rdr) = csv::ReaderBuilder::new()
                    .has_headers(false)
                    .from_path(&path)
                    && let Some(Ok(header_record)) = rdr.records().next()
                {
                    let headers: Vec<String> =
                        header_record.iter().map(|s| s.to_string()).collect();
                    dbs.push(CsvDatabase {
                        name,
                        path,
                        headers,
                    });
                }
            }
        }
    }

    Ok(dbs)
}

pub fn add_rows(
    ctx: &crate::tools::context::ToolContext,
    input: AddRowsInput,
) -> Result<String, String> {
    let db_path = get_db_path(ctx, &input.db_name);
    if !db_path.exists() {
        return Err(format!("Database '{}' does not exist", input.db_name));
    }

    let mut rdr =
        csv::Reader::from_path(&db_path).map_err(|e| format!("Failed to read csv: {}", e))?;
    let headers_record = rdr
        .headers()
        .map_err(|e| format!("Failed to read headers: {}", e))?
        .clone();
    let headers: Vec<String> = headers_record.iter().map(|s| s.to_string()).collect();

    for (i, row) in input.rows.iter().enumerate() {
        for key in row.keys() {
            if !headers.contains(key) {
                return Err(format!(
                    "Row {} contains invalid header: '{}'. Valid headers are: {:?}",
                    i, key, headers
                ));
            }
        }
        for header in &headers {
            if !row.contains_key(header) {
                return Err(format!(
                    "Row {} is missing required header: '{}'",
                    i, header
                ));
            }
        }
    }

    // Serialize the new rows through an in-memory buffer first, then
    // commit the buffer to the file in a single `write_all` call.
    //
    // The previous implementation streamed each `write_record` directly
    // into the live file, so a mid-write failure (disk full, broken
    // pipe, etc.) would leave the database with the first N rows
    // appended and the rest missing — the caller had no way to know
    // the file was in a torn state. Buffering first makes the file
    // update atomic: either every row lands or none of them do.
    let mut buffer: Vec<u8> = Vec::new();
    write_rows_to_writer(&input.rows, &headers, &mut buffer)?;

    ctx.vfs()
        .append(&db_path, &buffer)
        .map_err(|e| format!("Failed to write to file: {}", e))?;

    Ok(format!("Added {} rows", input.rows.len()))
}

/// Serialize `rows` into `writer` as CSV records ordered by `headers`.
///
/// Extracted from `add_rows` so the row-formatting path is unit-testable
/// with a mock `Write` (see `FailingWriter` in the test module). The
/// caller is responsible for atomically committing the resulting bytes
/// to durable storage — that is, this helper must not be pointed
/// directly at a live file, or a mid-write failure would tear the
/// destination just like the old streaming path did.
pub(crate) fn write_rows_to_writer<W: std::io::Write>(
    rows: &[std::collections::HashMap<String, String>],
    headers: &[String],
    writer: &mut W,
) -> Result<(), String> {
    let mut wtr = csv::Writer::from_writer(writer);
    for row in rows {
        let mut record = Vec::new();
        for header in headers {
            record.push(row.get(header).cloned().unwrap_or_default());
        }
        wtr.write_record(&record)
            .map_err(|e| format!("Failed to write row: {}", e))?;
    }
    wtr.flush().map_err(|e| format!("Failed to flush: {}", e))?;
    Ok(())
}

#[cfg(test)]
#[path = "operations_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "operations_proptests.rs"]
mod operations_proptests;
