//! CSV database tool implementations and provider for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::provider::{RegisteredTool, ToolProvider};
use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use fastmd_tool_macros::ToolDescriptor;
use std::sync::Arc;

use super::strings;

/// Tool that creates a new CSV file database.
#[derive(ToolDescriptor)]
#[tool(
    name = "create_csv",
    desc = strings::CREATE_CSV_DESCRIPTION,
    input = crate::agent::tools::csv_db::schema::CreateCsvInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = CsvDb,
    config = crate::app::batch::prompt_rules::csv_prompt_rule(),
    execute_with = execute_create_csv,
)]
pub(crate) struct CsvCreateTool;
fn execute_create_csv(
    _self: &CsvCreateTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: crate::agent::tools::csv_db::schema::CreateCsvInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::agent::tools::csv_db::operations::create_csv(ctx, input).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that lists all CSV file databases.
#[derive(ToolDescriptor)]
#[tool(
    name = "list_csv",
    desc = strings::LIST_CSV_DESCRIPTION,
    input = crate::agent::tools::csv_db::schema::ListCsvInput,
    safety = crate::agent::tools::Safety::ReadOnly,
    group = CsvDb,
    config = crate::app::batch::prompt_rules::csv_prompt_rule(),
    execute_with = execute_list_csv,
)]
pub(crate) struct CsvListTool;
fn execute_list_csv(
    _self: &CsvListTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: crate::agent::tools::csv_db::schema::ListCsvInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::agent::tools::csv_db::operations::list_csv(ctx, input).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that adds rows to a CSV file database.
#[derive(ToolDescriptor)]
#[tool(
    name = "add_rows",
    desc = strings::ADD_ROWS_DESCRIPTION,
    input = crate::agent::tools::csv_db::schema::AddRowsInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = CsvDb,
    config = crate::app::batch::prompt_rules::csv_prompt_rule(),
    execute_with = execute_add_rows,
)]
pub(crate) struct CsvAddRowsTool;
fn execute_add_rows(
    _self: &CsvAddRowsTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: crate::agent::tools::csv_db::schema::AddRowsInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::agent::tools::csv_db::operations::add_rows(ctx, input).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that deletes rows from a CSV file database based on a predicate.
#[derive(ToolDescriptor)]
#[tool(
    name = "delete_rows",
    desc = strings::DELETE_ROWS_DESCRIPTION,
    input = crate::agent::tools::csv_db::schema::DeleteRowsInput,
    safety = crate::agent::tools::Safety::Mutating,
    group = CsvDb,
    config = crate::app::batch::prompt_rules::csv_prompt_rule(),
    execute_with = execute_delete_rows,
)]
pub(crate) struct CsvDeleteRowsTool;
fn execute_delete_rows(
    _self: &CsvDeleteRowsTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: crate::agent::tools::csv_db::schema::DeleteRowsInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::agent::tools::csv_db::query::delete_rows(ctx, input).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that queries a CSV file database using an evalexpr predicate.
#[derive(ToolDescriptor)]
#[tool(
    name = "query",
    desc = strings::QUERY_DESCRIPTION,
    input = crate::agent::tools::csv_db::schema::QueryRequest,
    safety = crate::agent::tools::Safety::ReadOnly,
    group = CsvDb,
    config = crate::app::batch::prompt_rules::csv_prompt_rule(),
    execute_with = execute_query,
)]
pub(crate) struct CsvQueryTool;
fn execute_query(
    _self: &CsvQueryTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: crate::agent::tools::csv_db::schema::QueryRequest =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::agent::tools::csv_db::query::query_csv(ctx, input).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Self-registering provider for the CSV family.
pub(crate) struct CsvProvider;
impl ToolProvider for CsvProvider {
    fn id(&self) -> &'static str {
        "csv"
    }
    fn group(&self) -> ToolGroupId {
        ToolGroupId::Internal(InternalToolGroup::CsvDb)
    }
    fn tools(&self) -> Vec<RegisteredTool> {
        vec![
            registered(CsvCreateTool),
            registered(CsvListTool),
            registered(CsvAddRowsTool),
            registered(CsvDeleteRowsTool),
            registered(CsvQueryTool),
        ]
    }
}

fn registered<T: Tool + 'static>(tool: T) -> RegisteredTool {
    RegisteredTool {
        descriptor: Arc::new(tool.descriptor().clone()),
        executor: Arc::new(tool),
    }
}
