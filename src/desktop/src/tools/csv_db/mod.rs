use crate::config::AppConfig;
use serde_json::Value;

pub mod operations;
pub mod query;
pub mod schema;

fn should_enable_tools(prompt: &str) -> bool {
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

pub fn get_csv_tools(_config: &AppConfig, prompt: &str) -> Vec<Value> {
    if !should_enable_tools(prompt) {
        return vec![];
    }

    vec![
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "create_csv",
                "description": "Create a new CSV file database with specified headers.",
                "parameters": schemars::schema_for!(schema::CreateCsvInput)
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "list_csv",
                "description": "List all CSV file databases.",
                "parameters": schemars::schema_for!(schema::ListCsvInput)
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "add_rows",
                "description": "Add rows to a CSV file database.",
                "parameters": schemars::schema_for!(schema::AddRowsInput)
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "delete_rows",
                "description": "Delete rows from a CSV file database based on a predicate.",
                "parameters": schemars::schema_for!(schema::DeleteRowsInput)
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "query",
                "description": "Query a CSV file database using an evalexpr predicate, supporting sum and average aggregates.",
                "parameters": schemars::schema_for!(schema::QueryRequest)
            }
        }),
    ]
}

pub fn execute_csv_tool(config: &AppConfig, name: &str, args_str: &str) -> Option<Result<Value, String>> {
    match name {
        "create_csv" => {
            let input: schema::CreateCsvInput = match serde_json::from_str(args_str) {
                Ok(v) => v,
                Err(e) => return Some(Err(format!("Invalid args: {}", e))),
            };
            Some(operations::create_csv(config, input).map(|r| serde_json::to_value(r).unwrap()))
        },
        "list_csv" => {
            let input: schema::ListCsvInput = match serde_json::from_str(args_str) {
                Ok(v) => v,
                Err(e) => return Some(Err(format!("Invalid args: {}", e))),
            };
            Some(operations::list_csv(config, input).map(|r| serde_json::to_value(r).unwrap()))
        },
        "add_rows" => {
            let input: schema::AddRowsInput = match serde_json::from_str(args_str) {
                Ok(v) => v,
                Err(e) => return Some(Err(format!("Invalid args: {}", e))),
            };
            Some(operations::add_rows(config, input).map(|r| serde_json::to_value(r).unwrap()))
        },
        "delete_rows" => {
            let input: schema::DeleteRowsInput = match serde_json::from_str(args_str) {
                Ok(v) => v,
                Err(e) => return Some(Err(format!("Invalid args: {}", e))),
            };
            Some(query::delete_rows(config, input).map(|r| serde_json::to_value(r).unwrap()))
        },
        "query" => {
            let input: schema::QueryRequest = match serde_json::from_str(args_str) {
                Ok(v) => v,
                Err(e) => return Some(Err(format!("Invalid args: {}", e))),
            };
            Some(query::query_csv(config, input).map(|r| serde_json::to_value(r).unwrap()))
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_enable_tools() {
        assert!(should_enable_tools("I want to create a csv file"));
        assert!(should_enable_tools("let's query the database"));
        assert!(should_enable_tools("make a table for me"));
        assert!(should_enable_tools("add_rows to it"));
        assert!(should_enable_tools("use list_csv"));
        
        assert!(!should_enable_tools("just a normal message"));
        assert!(!should_enable_tools("hello world"));
    }

    #[test]
    fn test_get_csv_tools() {
        let config = AppConfig::default();
        let tools = get_csv_tools(&config, "create a csv");
        assert_eq!(tools.len(), 5);

        let empty_tools = get_csv_tools(&config, "normal prompt");
        assert!(empty_tools.is_empty());
    }
}
