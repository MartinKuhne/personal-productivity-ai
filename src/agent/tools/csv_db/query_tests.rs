//! Tests for the CSV query engine (`agent::tools::csv_db::query`): predicate filtering, aggregates and row deletion.

use super::*;
use crate::tools::csv_db::schema::{AddRowsInput, CreateCsvInput};
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

#[test]
fn test_query_and_delete() {
    let tmp_dir = tempdir().unwrap();
    let config = crate::config::AgentConfigBuilder::new()
        .with_csv_db_path(Some(tmp_dir.path().to_string_lossy().to_string()))
        .build();

    let _ = super::super::operations::create_csv(
        &test_ctx(&config),
        CreateCsvInput {
            db_name: "sales".to_string(),
            headers: vec!["item".to_string(), "price".to_string(), "qty".to_string()],
        },
    );

    let mut row1 = HashMap::new();
    row1.insert("item".to_string(), "apple".to_string());
    row1.insert("price".to_string(), "1.5".to_string());
    row1.insert("qty".to_string(), "10".to_string());

    let mut row2 = HashMap::new();
    row2.insert("item".to_string(), "banana".to_string());
    row2.insert("price".to_string(), "0.5".to_string());
    row2.insert("qty".to_string(), "20".to_string());

    let _ = super::super::operations::add_rows(
        &test_ctx(&config),
        AddRowsInput {
            db_name: "sales".to_string(),
            rows: vec![row1, row2],
        },
    );

    let q_res = query_csv(
        &test_ctx(&config),
        QueryRequest {
            db_name: "sales".to_string(),
            predicate: Some("price < 1.0".to_string()),
            aggregate_col: None,
            aggregate_func: None,
        },
    )
    .unwrap();
    assert_eq!(q_res.rows.len(), 1);
    assert_eq!(q_res.rows[0].get("item").unwrap(), "banana");

    // Test aggregate sum
    let q_res2 = query_csv(
        &test_ctx(&config),
        QueryRequest {
            db_name: "sales".to_string(),
            predicate: None,
            aggregate_col: Some("qty".to_string()),
            aggregate_func: Some("sum".to_string()),
        },
    )
    .unwrap();
    assert_eq!(q_res2.aggregate_result, Some(30.0));

    // Test aggregate average
    let q_res_avg = query_csv(
        &test_ctx(&config),
        QueryRequest {
            db_name: "sales".to_string(),
            predicate: None,
            aggregate_col: Some("price".to_string()),
            aggregate_func: Some("avg".to_string()),
        },
    )
    .unwrap();
    assert_eq!(q_res_avg.aggregate_result, Some(1.0));

    // Test unsupported aggregate
    let err_agg = query_csv(
        &test_ctx(&config),
        QueryRequest {
            db_name: "sales".to_string(),
            predicate: None,
            aggregate_col: Some("qty".to_string()),
            aggregate_func: Some("max".to_string()),
        },
    )
    .unwrap_err();
    assert!(err_agg.contains("Unsupported aggregate function"));

    // Test query invalid database
    let err_not_exist = query_csv(
        &test_ctx(&config),
        QueryRequest {
            db_name: "missing".to_string(),
            predicate: None,
            aggregate_col: None,
            aggregate_func: None,
        },
    )
    .unwrap_err();
    assert!(err_not_exist.contains("does not exist"));

    // Test delete
    let d_res = delete_rows(
        &test_ctx(&config),
        DeleteRowsInput {
            db_name: "sales".to_string(),
            predicate: "item == \"apple\"".to_string(),
        },
    )
    .unwrap();
    assert!(d_res.contains("Deleted 1 rows"));

    let q_res3 = query_csv(
        &test_ctx(&config),
        QueryRequest {
            db_name: "sales".to_string(),
            predicate: None,
            aggregate_col: None,
            aggregate_func: None,
        },
    )
    .unwrap();
    assert_eq!(q_res3.rows.len(), 1);
    assert_eq!(q_res3.rows[0].get("item").unwrap(), "banana");

    // Test delete invalid predicate
    let err_pred = delete_rows(
        &test_ctx(&config),
        DeleteRowsInput {
            db_name: "sales".to_string(),
            predicate: "invalid syntax ++".to_string(),
        },
    )
    .unwrap_err();
    assert!(
        err_pred.contains("Invalid predicate") || err_pred.contains("Evaluation error"),
        "Actual error: {}",
        err_pred
    );
}

#[test]
fn test_query_type_mismatch_predicate_references_missing_column() {
    let tmp_dir = tempdir().unwrap();
    let config = crate::config::AgentConfigBuilder::new()
        .with_csv_db_path(Some(tmp_dir.path().to_string_lossy().to_string()))
        .build();

    let _ = super::super::operations::create_csv(
        &test_ctx(&config),
        CreateCsvInput {
            db_name: "people".to_string(),
            headers: vec!["name".to_string(), "age".to_string()],
        },
    );
    let mut row = HashMap::new();
    row.insert("name".to_string(), "alice".to_string());
    row.insert("age".to_string(), "30".to_string());
    let _ = super::super::operations::add_rows(
        &test_ctx(&config),
        AddRowsInput {
            db_name: "people".to_string(),
            rows: vec![row],
        },
    );

    // Predicate references a column that does not exist in the schema:
    // The predicate evaluator treats the identifier as unbound and raises an
    // Evaluation error rather than silently matching zero rows.
    let err = query_csv(
        &test_ctx(&config),
        QueryRequest {
            db_name: "people".to_string(),
            predicate: Some("missing_col == 1".to_string()),
            aggregate_col: None,
            aggregate_func: None,
        },
    )
    .unwrap_err();
    assert!(err.contains("Evaluation error"), "Actual error: {}", err);
}

#[test]
fn test_query_sum_non_numeric_column_and_missing_column() {
    let tmp_dir = tempdir().unwrap();
    let config = crate::config::AgentConfigBuilder::new()
        .with_csv_db_path(Some(tmp_dir.path().to_string_lossy().to_string()))
        .build();

    let _ = super::super::operations::create_csv(
        &test_ctx(&config),
        CreateCsvInput {
            db_name: "mix".to_string(),
            headers: vec!["label".to_string(), "num".to_string()],
        },
    );
    let mut row = HashMap::new();
    row.insert("label".to_string(), "text-value".to_string());
    row.insert("num".to_string(), "5".to_string());
    let _ = super::super::operations::add_rows(
        &test_ctx(&config),
        AddRowsInput {
            db_name: "mix".to_string(),
            rows: vec![row],
        },
    );

    // sum over a non-numeric column: numeric values skipped -> 0.0
    let res = query_csv(
        &test_ctx(&config),
        QueryRequest {
            db_name: "mix".to_string(),
            predicate: None,
            aggregate_col: Some("label".to_string()),
            aggregate_func: Some("sum".to_string()),
        },
    )
    .unwrap();
    assert_eq!(res.aggregate_result, Some(0.0));

    // sum over a missing column -> Some(0.0)
    let res = query_csv(
        &test_ctx(&config),
        QueryRequest {
            db_name: "mix".to_string(),
            predicate: None,
            aggregate_col: Some("nope".to_string()),
            aggregate_func: Some("sum".to_string()),
        },
    )
    .unwrap();
    assert_eq!(res.aggregate_result, Some(0.0));
}

#[test]
fn test_query_avg_over_empty_rows_returns_zero() {
    let tmp_dir = tempdir().unwrap();
    let config = crate::config::AgentConfigBuilder::new()
        .with_csv_db_path(Some(tmp_dir.path().to_string_lossy().to_string()))
        .build();

    let _ = super::super::operations::create_csv(
        &test_ctx(&config),
        CreateCsvInput {
            db_name: "empty".to_string(),
            headers: vec!["x".to_string()],
        },
    );

    // No rows: predicate filters everything out, avg over empty -> Some(0.0)
    let res = query_csv(
        &test_ctx(&config),
        QueryRequest {
            db_name: "empty".to_string(),
            predicate: Some("x == 99".to_string()),
            aggregate_col: Some("x".to_string()),
            aggregate_func: Some("avg".to_string()),
        },
    )
    .unwrap();
    assert_eq!(res.aggregate_result, Some(0.0));
}

#[test]
fn test_delete_rows_invalid_record_fails() {
    let tmp_dir = tempdir().unwrap();
    let config = crate::config::AgentConfigBuilder::new()
        .with_csv_db_path(Some(tmp_dir.path().to_string_lossy().to_string()))
        .build();

    let db_path = super::super::operations::get_db_path(&test_ctx(&config), "bad");
    // Write a CSV whose rows have an inconsistent field count with the header
    // so that csv parsing of a record yields an error.
    std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
    std::fs::write(&db_path, "a,b,c\n1,2\n1,2,3\n").unwrap();

    let err = delete_rows(
        &test_ctx(&config),
        DeleteRowsInput {
            db_name: "bad".to_string(),
            predicate: "a == 1".to_string(),
        },
    )
    .unwrap_err();
    assert!(err.contains("Invalid record"), "Actual error: {}", err);
}
