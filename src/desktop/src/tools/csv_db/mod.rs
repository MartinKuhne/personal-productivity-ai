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

pub fn execute_csv_tool(
    config: &AppConfig,
    name: &str,
    args_str: &str,
) -> Option<Result<Value, String>> {
    match name {
        "create_csv" => {
            let input: schema::CreateCsvInput = match serde_json::from_str(args_str) {
                Ok(v) => v,
                Err(e) => return Some(Err(format!("Invalid args: {}", e))),
            };
            Some(operations::create_csv(config, input).map(|r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            }))
        }
        "list_csv" => {
            let input: schema::ListCsvInput = match serde_json::from_str(args_str) {
                Ok(v) => v,
                Err(e) => return Some(Err(format!("Invalid args: {}", e))),
            };
            Some(operations::list_csv(config, input).map(|r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            }))
        }
        "add_rows" => {
            let input: schema::AddRowsInput = match serde_json::from_str(args_str) {
                Ok(v) => v,
                Err(e) => return Some(Err(format!("Invalid args: {}", e))),
            };
            Some(operations::add_rows(config, input).map(|r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            }))
        }
        "delete_rows" => {
            let input: schema::DeleteRowsInput = match serde_json::from_str(args_str) {
                Ok(v) => v,
                Err(e) => return Some(Err(format!("Invalid args: {}", e))),
            };
            Some(query::delete_rows(config, input).map(|r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            }))
        }
        "query" => {
            let input: schema::QueryRequest = match serde_json::from_str(args_str) {
                Ok(v) => v,
                Err(e) => return Some(Err(format!("Invalid args: {}", e))),
            };
            Some(query::query_csv(config, input).map(|r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            }))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_should_enable_tools() {
        assert!(should_enable_tools("I want to create a csv file"));
        assert!(should_enable_tools("let's query the database"));
        assert!(should_enable_tools("make a table for me"));
        assert!(should_enable_tools("add_rows to it"));
        assert!(should_enable_tools("use list_csv"));
        assert!(should_enable_tools("delete_rows from table"));
        assert!(should_enable_tools("create_csv test"));
        assert!(should_enable_tools("query test"));

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

    #[test]
    fn test_execute_csv_tool_all_ops() {
        let dir = tempdir().unwrap();
        let mut config = AppConfig::default();
        config.csv_db_path = Some(dir.path().to_str().unwrap().to_string());

        // 1. Unknown tool
        assert!(execute_csv_tool(&config, "unknown_tool", "{}").is_none());

        // 2. create_csv
        let create_args = r#"{"db_name":"users","headers":["id","name","age"]}"#;
        let res = execute_csv_tool(&config, "create_csv", create_args);
        assert!(res.is_some());
        assert!(res.as_ref().unwrap().is_ok());

        // invalid args for create_csv
        let bad_res = execute_csv_tool(&config, "create_csv", "invalid json");
        assert!(bad_res.is_some());
        assert!(bad_res.as_ref().unwrap().is_err());

        // 3. list_csv
        let list_args = r#"{}"#;
        let res = execute_csv_tool(&config, "list_csv", list_args);
        assert!(res.is_some());
        let val = res.unwrap().unwrap();
        assert!(val.is_array());
        assert_eq!(val.as_array().unwrap().len(), 1);

        // invalid args for list_csv
        let bad_res = execute_csv_tool(&config, "list_csv", "{bad}");
        assert!(bad_res.is_some());
        assert!(bad_res.as_ref().unwrap().is_err());

        // 4. add_rows
        let add_args = r#"{"db_name":"users","rows":[{"id":"1","name":"Alice","age":"30"},{"id":"2","name":"Bob","age":"25"}]}"#;
        let res = execute_csv_tool(&config, "add_rows", add_args);
        assert!(res.is_some());
        assert!(res.as_ref().unwrap().is_ok());

        // invalid args for add_rows
        let bad_res = execute_csv_tool(&config, "add_rows", "not json");
        assert!(bad_res.is_some());
        assert!(bad_res.as_ref().unwrap().is_err());

        // 5. query
        let query_args = r#"{"db_name":"users","predicate":"age > 20"}"#;
        let res = execute_csv_tool(&config, "query", query_args);
        assert!(res.is_some());
        let val = res.unwrap().unwrap();
        assert_eq!(val["rows"].as_array().unwrap().len(), 2);

        // invalid args for query
        let bad_res = execute_csv_tool(&config, "query", "{invalid}");
        assert!(bad_res.is_some());
        assert!(bad_res.as_ref().unwrap().is_err());

        // 6. delete_rows
        let delete_args = r#"{"db_name":"users","predicate":"name == \"Bob\""}"#;
        let res = execute_csv_tool(&config, "delete_rows", delete_args);
        assert!(res.is_some());
        let val = res.unwrap().unwrap();
        assert!(val.as_str().unwrap().contains("Deleted 1 rows"));

        // invalid args for delete_rows
        let bad_res = execute_csv_tool(&config, "delete_rows", "abc");
        assert!(bad_res.is_some());
        assert!(bad_res.as_ref().unwrap().is_err());
    }
}
