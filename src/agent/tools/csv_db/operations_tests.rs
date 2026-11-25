//! Tests for the CSV-database CRUD operations (`agent::tools::csv_db::operations`): create, list, add rows and atomic write buffering.

use super::*;
use std::collections::HashMap;
use tempfile::tempdir;

fn test_ctx(config: &crate::config::AgentConfig) -> crate::tools::context::ToolContext {
    let mut builder = crate::tools::context::ToolContextBuilder::new(
        std::sync::Arc::new(config.clone()),
        std::sync::Arc::new(crate::tools::observer::DefaultFileObserver),
    );
    builder = builder.with_extension(std::sync::Arc::new(
        crate::tools::vfs::VirtualFileSystemExt(std::sync::Arc::new(
            crate::tools::vfs::VfsResolver::new(std::sync::Arc::new(config.clone())),
        )),
    ));
    builder.build()
}

/// A `Write` that succeeds for the first `fail_after` bytes, then
/// returns an error on every subsequent call. Used to simulate
/// mid-write failures without relying on OS-level disk-full quirks.
struct FailingWriter {
    written: usize,
    fail_after: usize,
}
impl FailingWriter {
    fn new(fail_after: usize) -> Self {
        Self {
            written: 0,
            fail_after,
        }
    }
}
impl std::io::Write for FailingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.written >= self.fail_after {
            return Err(std::io::Error::other("simulated mid-write failure"));
        }
        let take = (self.fail_after - self.written).min(buf.len());
        self.written += take;
        Ok(take)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_create_and_list_and_add_rows() {
    let dir = tempdir().unwrap();
    let config = crate::config::AgentConfigBuilder::new()
        .with_csv_db_path(Some(dir.path().to_string_lossy().to_string()))
        .build();

    let create_input = CreateCsvInput {
        db_name: "test_db".to_string(),
        headers: vec!["id".to_string(), "name".to_string(), "age".to_string()],
    };
    let db = create_csv(&test_ctx(&config), create_input.clone()).unwrap();
    assert_eq!(db.name, "test_db");
    assert_eq!(db.headers.len(), 3);

    // Test creating again should fail
    let err_create = create_csv(&test_ctx(&config), create_input).unwrap_err();
    assert!(err_create.contains("already exists"));

    let list = list_csv(&test_ctx(&config), ListCsvInput {}).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "test_db");

    let mut row1 = HashMap::new();
    row1.insert("id".to_string(), "1".to_string());
    row1.insert("name".to_string(), "Alice".to_string());
    row1.insert("age".to_string(), "25".to_string());

    let mut row2 = HashMap::new();
    row2.insert("id".to_string(), "2".to_string());
    row2.insert("name".to_string(), "Bob".to_string());
    row2.insert("age".to_string(), "30".to_string());

    let add_input = AddRowsInput {
        db_name: "test_db".to_string(),
        rows: vec![row1, row2],
    };
    let add_res = add_rows(&test_ctx(&config), add_input).unwrap();
    assert!(add_res.contains("Added 2 rows"));

    // Test invalid header
    let mut bad_row = HashMap::new();
    bad_row.insert("invalid_header".to_string(), "1".to_string());
    let bad_input = AddRowsInput {
        db_name: "test_db".to_string(),
        rows: vec![bad_row],
    };
    let err = add_rows(&test_ctx(&config), bad_input).unwrap_err();
    assert!(err.contains("invalid header"));

    // Test missing required header
    let mut missing_row = HashMap::new();
    missing_row.insert("id".to_string(), "3".to_string());
    missing_row.insert("name".to_string(), "Charlie".to_string());
    let missing_input = AddRowsInput {
        db_name: "test_db".to_string(),
        rows: vec![missing_row],
    };
    let err_missing = add_rows(&test_ctx(&config), missing_input).unwrap_err();
    assert!(err_missing.contains("missing required header"));

    // Test add to non-existent db
    let err_not_exist = add_rows(
        &test_ctx(&config),
        AddRowsInput {
            db_name: "missing_db".to_string(),
            rows: vec![],
        },
    )
    .unwrap_err();
    assert!(err_not_exist.contains("does not exist"));
}

#[test]
fn test_add_rows_write_helper_propagates_errors() {
    // The CSV writer previously streamed records directly into
    // the live file as each row was produced; a mid-write failure
    // (disk full, broken pipe, etc.) would leave the database with
    // the first N rows appended and the rest missing — the caller
    // had no way to know the file was in a torn state.
    //
    // The fix is to serialize the new rows through an in-memory
    // buffer first, so a single `write_all` to the file is
    // atomic. This test exercises the buffer-writing path with a
    // `FailingWriter` to prove it propagates errors instead of
    // silently returning Ok.
    let mut row_a = HashMap::new();
    row_a.insert("id".to_string(), "1".to_string());
    row_a.insert("name".to_string(), "Alice".to_string());
    let mut row_b = HashMap::new();
    row_b.insert("id".to_string(), "2".to_string());
    row_b.insert("name".to_string(), "Bob".to_string());
    let rows = vec![row_a, row_b];
    let headers = vec!["id".to_string(), "name".to_string()];

    // fail_after = 0 means every write fails immediately.
    let mut failing = FailingWriter::new(0);
    let result = write_rows_to_writer(&rows, &headers, &mut failing);
    assert!(
        result.is_err(),
        "expected write failure to propagate, got {:?}",
        result
    );
}
