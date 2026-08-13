//! CSV database tool implementations and provider for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::descriptor::ToolDescriptor;
use crate::agent::tools::provider::{RegisteredTool, ToolProvider};
use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use std::sync::{Arc, OnceLock};

use super::strings;

fn build_csv_descriptor<I>(
    name: &'static str,
    description: &'static str,
    safety: crate::agent::tools::Safety,
) -> ToolDescriptor
where
    I: schemars::JsonSchema + 'static,
{
    let group = ToolGroupId::Internal(InternalToolGroup::CsvDb);
    ToolDescriptor::new::<I>(
        name,
        description,
        safety,
        crate::app::batch::prompt_rules::csv_prompt_rule(),
        group,
    )
}

/// Tool that creates a new CSV file database.
pub(crate) struct CsvCreateTool;
impl Tool for CsvCreateTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_csv_descriptor::<crate::agent::tools::csv_db::schema::CreateCsvInput>(
                "create_csv",
                strings::CREATE_CSV_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
            )
        })
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
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_csv_descriptor::<crate::agent::tools::csv_db::schema::ListCsvInput>(
                "list_csv",
                strings::LIST_CSV_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
            )
        })
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
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_csv_descriptor::<crate::agent::tools::csv_db::schema::AddRowsInput>(
                "add_rows",
                strings::ADD_ROWS_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
            )
        })
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
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_csv_descriptor::<crate::agent::tools::csv_db::schema::DeleteRowsInput>(
                "delete_rows",
                strings::DELETE_ROWS_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
            )
        })
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
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_csv_descriptor::<crate::agent::tools::csv_db::schema::QueryRequest>(
                "query",
                strings::QUERY_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: crate::agent::tools::csv_db::schema::QueryRequest =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::csv_db::query::query_csv(&ctx.config, input).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
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
