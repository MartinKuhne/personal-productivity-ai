//! CSV-database data types — `CsvDatabase` descriptor and serde/JsonSchema input/output structs for CRUD and query operations.
//!
//! Per-field description strings are sourced from the `csv` strings submodule
//! under `registry/builtin/strings/` so the LLM-facing JSON schema is
//! generated from a single source of truth.

use crate::agent::tools::registry::builtin::strings;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvDatabase {
    pub name: String,
    pub path: std::path::PathBuf,
    pub headers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateCsvInput {
    #[schemars(description = strings::FIELD_CREATE_CSV_INPUT_DB_NAME)]
    pub db_name: String,
    #[schemars(description = strings::FIELD_CREATE_CSV_INPUT_HEADERS)]
    pub headers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddRowsInput {
    #[schemars(description = strings::FIELD_ADD_ROWS_INPUT_DB_NAME)]
    pub db_name: String,
    #[schemars(description = strings::FIELD_ADD_ROWS_INPUT_ROWS)]
    pub rows: Vec<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteRowsInput {
    #[schemars(description = strings::FIELD_DELETE_ROWS_INPUT_DB_NAME)]
    pub db_name: String,
    #[schemars(description = strings::FIELD_DELETE_ROWS_INPUT_PREDICATE)]
    pub predicate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListCsvInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryRequest {
    #[schemars(description = strings::FIELD_QUERY_REQUEST_DB_NAME)]
    pub db_name: String,
    #[schemars(description = strings::FIELD_QUERY_REQUEST_PREDICATE)]
    pub predicate: Option<String>,
    #[schemars(description = strings::FIELD_QUERY_REQUEST_AGGREGATE_COL)]
    pub aggregate_col: Option<String>,
    #[schemars(description = strings::FIELD_QUERY_REQUEST_AGGREGATE_FUNC)]
    pub aggregate_func: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    pub rows: Vec<HashMap<String, String>>,
    pub aggregate_result: Option<f64>,
}
