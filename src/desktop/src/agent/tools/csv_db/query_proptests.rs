//! Property-based tests for the csv_db query builder.
//!
//! The csv_db query builder evaluates a user-supplied
//! `evalexpr` predicate against CSV row values. The
//! builder has two contract surfaces:
//!
//! - **No panics** on any input. A panic in the predicate
//!   evaluator or the row-iteration loop would crash the
//!   editor.
//! - **Deterministic results for tautologies and
//!   contradictions**. The predicate `1 == 1` (or
//!   `true`) must match every row; `1 == 0` (or `false`)
//!   must match no row. The aggregate `sum` of a column of
//!   zeros must be 0; `sum` of a column with no numeric
//!   values must be 0 (the count is 0, sum is 0).
//!
//! Per AGENTS.md RUST-058 the csv_db is egui-free, so the
//! proptest can live as a sidecar of `query.rs`.
//!
//! # Properties under test
//!
//! 1. `query_csv` with no predicate returns every row.
//! 2. `query_csv` with `1 == 1` (or `true`) returns every
//!    row.
//! 3. `query_csv` with `1 == 0` (or `false`) returns no
//!    row.
//! 4. `query_csv` aggregate `sum` is the sum of the
//!    column's numeric values (non-numeric values are
//!    skipped).
//! 5. `query_csv` aggregate `average`/`avg` is the mean
//!    of the column's numeric values (or 0 if no numeric
//!    values).
//! 6. `delete_rows` with `1 == 1` deletes every row.
//! 7. The query builder does not panic on a random
//!    predicate (predicate-freedom is the
//!    panic-freedom property — the builder must handle
//!    any syntactically valid `evalexpr` expression
//!    without panicking).
//!
//! The proptest creates a fresh CSV per test run (via
//! `tempfile::tempdir`) and runs the query builder
//! against it. The CSV content is small (a handful of
//! rows) because the property under test is the *logic*
//! of the builder, not its throughput.

use super::super::operations::{add_rows, create_csv};
use super::super::query::{delete_rows, query_csv};
use super::super::schema::{AddRowsInput, CreateCsvInput, DeleteRowsInput, QueryRequest};
use crate::agent::config::AgentConfig;

fn test_ctx(config: &crate::agent::config::AgentConfig) -> crate::agent::tools::context::ToolContext {
    let mut builder = crate::agent::tools::context::ToolContextBuilder::new(
        std::sync::Arc::new(config.clone()),
        std::sync::Arc::new(crate::agent::tools::observer::DefaultFileObserver)
    );
    builder = builder.with_extension(std::sync::Arc::new(crate::agent::tools::vfs::VirtualFileSystemExt(std::sync::Arc::new(crate::agent::tools::vfs::VfsResolver::new(std::sync::Arc::new(config.clone()))))));
    builder.build()
}

use proptest::prelude::*;
use std::collections::HashMap;
use tempfile::tempdir;

/// Build a small CSV with a fixed schema and a fixed set
/// of rows. The schema is the same for every test run so
/// the proptest can use a fixed predicate corpus. The
/// values are deterministic (not proptest-generated)
/// because the property under test is the *builder's*
/// behaviour on a known input, not the behaviour of
/// `evalexpr` on random inputs.
fn build_fixture_csv(config: &AgentConfig) -> (String, Vec<HashMap<String, String>>) {
    let db_name = "proptest".to_string();
    create_csv(&test_ctx(&config),
        CreateCsvInput {
            db_name: db_name.clone(),
            headers: vec!["item".to_string(), "price".to_string(), "qty".to_string()],
        },
    )
    .expect("create_csv");
    let rows = vec![
        vec![("item", "apple"), ("price", "1.5"), ("qty", "10")],
        vec![("item", "banana"), ("price", "0.5"), ("qty", "20")],
        vec![("item", "cherry"), ("price", "2.0"), ("qty", "5")],
    ];
    let rows: Vec<HashMap<String, String>> = rows
        .into_iter()
        .map(|r| {
            r.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        })
        .collect();
    add_rows(&test_ctx(&config),
        AddRowsInput {
            db_name: db_name.clone(),
            rows: rows.clone(),
        },
    )
    .expect("add_rows");
    (db_name, rows)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Property 1: no predicate returns every row.
    #[test]
    fn no_predicate_returns_all_rows(_dummy in 0u8..1) {
        let dir = tempdir().unwrap();
        let config = AgentConfig {
            csv_db_path: Some(dir.path().to_string_lossy().to_string()),
            ..AgentConfig::default()
        };
        let (db_name, rows) = build_fixture_csv(&config);
        let res = query_csv(
            &test_ctx(&config),
            QueryRequest {
                db_name,
                predicate: None,
                aggregate_col: None,
                aggregate_func: None,
            },
        )
        .expect("query_csv");
        prop_assert_eq!(res.rows.len(), rows.len(), "all rows should be returned");
    }

    /// Property 2: `1 == 1` matches every row.
    #[test]
    fn tautology_predicate_matches_all_rows(_dummy in 0u8..1) {
        let dir = tempdir().unwrap();
        let config = AgentConfig {
            csv_db_path: Some(dir.path().to_string_lossy().to_string()),
            ..AgentConfig::default()
        };
        let (db_name, rows) = build_fixture_csv(&config);
        for pred in &["1 == 1", "true", "1.0 == 1.0"] {
            let res = query_csv(
                &test_ctx(&config),
                QueryRequest {
                    db_name: db_name.clone(),
                    predicate: Some(pred.to_string()),
                    aggregate_col: None,
                    aggregate_func: None,
                },
            )
            .expect("query_csv");
            prop_assert_eq!(
                res.rows.len(),
                rows.len(),
                "tautology {:?} should match all rows, got {}/{}",
                pred,
                res.rows.len(),
                rows.len()
            );
        }
    }

    /// Property 3: `1 == 0` matches no row.
    #[test]
    fn contradiction_predicate_matches_no_rows(_dummy in 0u8..1) {
        let dir = tempdir().unwrap();
        let config = AgentConfig {
            csv_db_path: Some(dir.path().to_string_lossy().to_string()),
            ..AgentConfig::default()
        };
        let (db_name, _rows) = build_fixture_csv(&config);
        for pred in &["1 == 0", "false", "1.0 == 0.0"] {
            let res = query_csv(
                &test_ctx(&config),
                QueryRequest {
                    db_name: db_name.clone(),
                    predicate: Some(pred.to_string()),
                    aggregate_col: None,
                    aggregate_func: None,
                },
            )
            .expect("query_csv");
            prop_assert_eq!(
                res.rows.len(),
                0,
                "contradiction {:?} should match no rows, got {}",
                pred,
                res.rows.len()
            );
        }
    }

    /// Property 4: `sum` aggregate on a numeric column is
    /// the sum of the values.
    #[test]
    fn sum_aggregate_is_correct(_dummy in 0u8..1) {
        let dir = tempdir().unwrap();
        let config = AgentConfig {
            csv_db_path: Some(dir.path().to_string_lossy().to_string()),
            ..AgentConfig::default()
        };
        let (db_name, _) = build_fixture_csv(&config);
        // qty: 10 + 20 + 5 = 35
        let res = query_csv(
            &test_ctx(&config),
            QueryRequest {
                db_name,
                predicate: None,
                aggregate_col: Some("qty".to_string()),
                aggregate_func: Some("sum".to_string()),
            },
        )
        .expect("query_csv");
        prop_assert_eq!(res.aggregate_result, Some(35.0), "qty sum should be 35");
    }

    /// Property 5: `average` aggregate on a numeric column
    /// is the mean of the values.
    #[test]
    fn average_aggregate_is_correct(_dummy in 0u8..1) {
        let dir = tempdir().unwrap();
        let config = AgentConfig {
            csv_db_path: Some(dir.path().to_string_lossy().to_string()),
            ..AgentConfig::default()
        };
        let (db_name, _) = build_fixture_csv(&config);
        // price: (1.5 + 0.5 + 2.0) / 3 = 1.333...
        let res = query_csv(
            &test_ctx(&config),
            QueryRequest {
                db_name,
                predicate: None,
                aggregate_col: Some("price".to_string()),
                aggregate_func: Some("avg".to_string()),
            },
        )
        .expect("query_csv");
        let avg = res.aggregate_result.expect("avg should be Some");
        prop_assert!(
            (avg - 4.0 / 3.0).abs() < 1e-9,
            "price average should be ~1.333, got {}",
            avg
        );
    }

    /// Property 6: `delete_rows` with `1 == 1` deletes every
    /// row (the resulting CSV is empty when re-queried).
    #[test]
    fn delete_tautology_removes_all_rows(_dummy in 0u8..1) {
        let dir = tempdir().unwrap();
        let config = AgentConfig {
            csv_db_path: Some(dir.path().to_string_lossy().to_string()),
            ..AgentConfig::default()
        };
        let (db_name, rows) = build_fixture_csv(&config);
        let result = delete_rows(
            &test_ctx(&config),
            DeleteRowsInput {
                db_name: db_name.clone(),
                predicate: "1 == 1".to_string(),
            },
        )
        .expect("delete_rows");
        prop_assert!(
            result.contains(&format!("Deleted {} rows", rows.len())),
            "delete_rows should report all rows deleted: {}",
            result
        );
        // Re-query to confirm the CSV is empty.
        let res = query_csv(
            &test_ctx(&config),
            QueryRequest {
                db_name,
                predicate: None,
                aggregate_col: None,
                aggregate_func: None,
            },
        )
        .expect("query_csv");
        prop_assert_eq!(res.rows.len(), 0, "CSV should be empty after delete_tautology");
    }

    /// Property 7: the query builder does not panic on a
    /// random predicate. The predicate is a syntactically
    /// valid `evalexpr` expression that compares a random
    /// column to a random literal; the proptest explores the
    /// space of "real predicates" the LLM might emit.
    ///
    /// A panic here would be the worst-case outcome — the
    /// editor would crash on a bad query. The proptest
    /// asserts the *predicate evaluation itself* does not
    /// panic; an invalid predicate that produces an
    /// `Err` from `evalexpr::build_operator_tree` is
    /// expected and the test skips it (the `evalexpr`
    /// surface itself is not what we're testing).
    #[test]
    fn query_does_not_panic_on_random_predicate(
        column in prop::string::string_regex(r"[a-z_]{1,10}").unwrap(),
        op in prop::sample::select(&["==", "!=", "<", ">", "<=", ">="]),
        value in prop::string::string_regex(r"[a-zA-Z0-9_]{1,10}").unwrap(),
    ) {
        let dir = tempdir().unwrap();
        let config = AgentConfig {
            csv_db_path: Some(dir.path().to_string_lossy().to_string()),
            ..AgentConfig::default()
        };
        let (db_name, _) = build_fixture_csv(&config);
        // Quote the value to handle non-numeric predicates
        // (evalexpr requires string operands to be quoted).
        let predicate = format!("{column} {op} \"{value}\"");
        // The query MUST NOT panic. The result may be Ok
        // (any number of rows) or Err (predicate or
        // column doesn't exist); both are acceptable.
        let _ = query_csv(
            &test_ctx(&config),
            QueryRequest {
                db_name,
                predicate: Some(predicate),
                aggregate_col: None,
                aggregate_func: None,
            },
        );
    }
}
