//! CSV-database data types — `CsvDatabase` descriptor and serde/JsonSchema input/output structs for CRUD and query operations.

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
    pub db_name: String,
    pub headers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddRowsInput {
    pub db_name: String,
    pub rows: Vec<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteRowsInput {
    pub db_name: String,
    pub predicate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListCsvInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryRequest {
    pub db_name: String,
    pub predicate: Option<String>,
    pub aggregate_col: Option<String>,
    pub aggregate_func: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    pub rows: Vec<HashMap<String, String>>,
    pub aggregate_result: Option<f64>,
}
