//! CSV database tool implementations for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::config::AppConfig;
use std::any::TypeId;

use super::json_schema;
use super::strings;

fn csv_tools_enabled(prompt: &str) -> bool {
    let p = prompt.to_lowercase();
    p.contains("table")
        || p.contains("csv")
        || p.contains("database")
        || p.contains("add_rows")
        || p.contains("delete_rows")
        || p.contains("create_csv")
        || p.contains("list_csv")
        || p.contains("query")
}

/// Tool that creates a new CSV file database.
pub(crate) struct CsvCreateTool;
impl Tool for CsvCreateTool {
    fn name(&self) -> &'static str {
        "create_csv"
    }
    fn description(&self) -> &'static str {
        strings::CREATE_CSV_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<crate::agent::tools::csv_db::schema::CreateCsvInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<crate::agent::tools::csv_db::schema::CreateCsvInput>()
    }
    fn is_enabled(&self, config: &AppConfig, prompt: &str) -> bool {
        config.tool_groups.csv_db && csv_tools_enabled(prompt)
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: crate::agent::tools::csv_db::schema::CreateCsvInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::csv_db::operations::create_csv(&ctx.config, input).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that lists all CSV file databases.
pub(crate) struct CsvListTool;
impl Tool for CsvListTool {
    fn name(&self) -> &'static str {
        "list_csv"
    }
    fn description(&self) -> &'static str {
        strings::LIST_CSV_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<crate::agent::tools::csv_db::schema::ListCsvInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<crate::agent::tools::csv_db::schema::ListCsvInput>()
    }
    fn is_enabled(&self, config: &AppConfig, prompt: &str) -> bool {
        config.tool_groups.csv_db && csv_tools_enabled(prompt)
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: crate::agent::tools::csv_db::schema::ListCsvInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::csv_db::operations::list_csv(&ctx.config, input).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that adds rows to a CSV file database.
pub(crate) struct CsvAddRowsTool;
impl Tool for CsvAddRowsTool {
    fn name(&self) -> &'static str {
        "add_rows"
    }
    fn description(&self) -> &'static str {
        strings::ADD_ROWS_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<crate::agent::tools::csv_db::schema::AddRowsInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<crate::agent::tools::csv_db::schema::AddRowsInput>()
    }
    fn is_enabled(&self, config: &AppConfig, prompt: &str) -> bool {
        config.tool_groups.csv_db && csv_tools_enabled(prompt)
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: crate::agent::tools::csv_db::schema::AddRowsInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::csv_db::operations::add_rows(&ctx.config, input).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that deletes rows from a CSV file database based on a predicate.
pub(crate) struct CsvDeleteRowsTool;
impl Tool for CsvDeleteRowsTool {
    fn name(&self) -> &'static str {
        "delete_rows"
    }
    fn description(&self) -> &'static str {
        strings::DELETE_ROWS_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<crate::agent::tools::csv_db::schema::DeleteRowsInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<crate::agent::tools::csv_db::schema::DeleteRowsInput>()
    }
    fn is_enabled(&self, config: &AppConfig, prompt: &str) -> bool {
        config.tool_groups.csv_db && csv_tools_enabled(prompt)
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: crate::agent::tools::csv_db::schema::DeleteRowsInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::csv_db::query::delete_rows(&ctx.config, input).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that queries a CSV file database using an evalexpr predicate.
pub(crate) struct CsvQueryTool;
impl Tool for CsvQueryTool {
    fn name(&self) -> &'static str {
        "query"
    }
    fn description(&self) -> &'static str {
        strings::QUERY_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<crate::agent::tools::csv_db::schema::QueryRequest>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<crate::agent::tools::csv_db::schema::QueryRequest>()
    }
    fn is_enabled(&self, config: &AppConfig, prompt: &str) -> bool {
        config.tool_groups.csv_db && csv_tools_enabled(prompt)
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: crate::agent::tools::csv_db::schema::QueryRequest =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::csv_db::query::query_csv(&ctx.config, input).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}
