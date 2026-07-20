use crate::config::AppConfig;
use super::schema::{DeleteRowsInput, QueryRequest, QueryResponse};
use evalexpr::{HashMapContext, ContextWithMutableVariables, Value};
use std::collections::HashMap;

fn create_context(row: &csv::StringRecord, headers: &csv::StringRecord) -> HashMapContext {
    let mut context = HashMapContext::new();
    for (i, header) in headers.iter().enumerate() {
        if let Some(val) = row.get(i) {
            if let Ok(num) = val.parse::<i64>() {
                context.set_value(header.into(), Value::Int(num)).unwrap();
            } else if let Ok(num) = val.parse::<f64>() {
                context.set_value(header.into(), Value::Float(num)).unwrap();
            } else {
                context.set_value(header.into(), Value::String(val.to_string())).unwrap();
            }
        }
    }
    context
}

pub fn delete_rows(config: &AppConfig, input: DeleteRowsInput) -> Result<String, String> {
    let db_path = super::operations::get_db_path(config, &input.db_name);
    if !db_path.exists() {
        return Err(format!("Database '{}' does not exist", input.db_name));
    }

    let mut rdr = csv::Reader::from_path(&db_path).map_err(|e| format!("Failed to read csv: {}", e))?;
    let headers = rdr.headers().map_err(|e| format!("Failed to read headers: {}", e))?.clone();
    
    let mut kept_rows = Vec::new();
    let mut deleted_count = 0;
    
    let predicate = evalexpr::build_operator_tree(&input.predicate).map_err(|e| format!("Invalid predicate: {}", e))?;

    for result in rdr.records() {
        let record = result.map_err(|e| format!("Invalid record: {}", e))?;
        let context = create_context(&record, &headers);
        let eval_res = predicate.eval_boolean_with_context(&context).map_err(|e| format!("Evaluation error: {}", e))?;
        
        if eval_res {
            deleted_count += 1;
        } else {
            kept_rows.push(record);
        }
    }
    
    let mut wtr = csv::Writer::from_path(&db_path).map_err(|e| format!("Failed to open for write: {}", e))?;
    wtr.write_record(&headers).map_err(|e| format!("Failed to write headers: {}", e))?;
    for record in kept_rows {
        wtr.write_record(&record).map_err(|e| format!("Failed to write record: {}", e))?;
    }
    wtr.flush().map_err(|e| format!("Failed to flush: {}", e))?;

    Ok(format!("Deleted {} rows", deleted_count))
}

pub fn query_csv(config: &AppConfig, input: QueryRequest) -> Result<QueryResponse, String> {
    let db_path = super::operations::get_db_path(config, &input.db_name);
    if !db_path.exists() {
        return Err(format!("Database '{}' does not exist", input.db_name));
    }

    let mut rdr = csv::Reader::from_path(&db_path).map_err(|e| format!("Failed to read csv: {}", e))?;
    let headers = rdr.headers().map_err(|e| format!("Failed to read headers: {}", e))?.clone();
    
    let mut matched_rows = Vec::new();
    
    let predicate = if let Some(p) = &input.predicate {
        Some(evalexpr::build_operator_tree(p).map_err(|e| format!("Invalid predicate: {}", e))?)
    } else {
        None
    };

    for result in rdr.records() {
        let record = result.map_err(|e| format!("Invalid record: {}", e))?;
        if let Some(ref pred) = predicate {
            let context = create_context(&record, &headers);
            let eval_res = pred.eval_boolean_with_context(&context).map_err(|e| format!("Evaluation error: {}", e))?;
            if !eval_res {
                continue;
            }
        }
        
        let mut row_map = HashMap::new();
        for (i, header) in headers.iter().enumerate() {
            row_map.insert(header.to_string(), record.get(i).unwrap_or("").to_string());
        }
        matched_rows.push(row_map);
    }
    
    let mut aggregate_result = None;
    if let (Some(col), Some(func)) = (&input.aggregate_col, &input.aggregate_func) {
        let mut sum = 0.0;
        let mut count = 0;
        for row in &matched_rows {
            if let Some(val_str) = row.get(col) {
                if let Ok(num) = val_str.parse::<f64>() {
                    sum += num;
                    count += 1;
                }
            }
        }
        
        match func.to_lowercase().as_str() {
            "sum" => aggregate_result = Some(sum),
            "average" | "avg" => {
                if count > 0 {
                    aggregate_result = Some(sum / (count as f64));
                } else {
                    aggregate_result = Some(0.0);
                }
            },
            _ => return Err(format!("Unsupported aggregate function: {}", func)),
        }
    }

    Ok(QueryResponse {
        rows: matched_rows,
        aggregate_result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::tools::csv_db::schema::{CreateCsvInput, AddRowsInput};

    #[test]
    fn test_query_and_delete() {
        let dir = tempdir().unwrap();
        let mut config = AppConfig::default();
        config.csv_db_path = Some(dir.path().to_string_lossy().to_string());

        let _ = super::super::operations::create_csv(&config, CreateCsvInput {
            db_name: "sales".to_string(),
            headers: vec!["item".to_string(), "price".to_string(), "qty".to_string()],
        });

        let mut row1 = HashMap::new();
        row1.insert("item".to_string(), "apple".to_string());
        row1.insert("price".to_string(), "1.5".to_string());
        row1.insert("qty".to_string(), "10".to_string());

        let mut row2 = HashMap::new();
        row2.insert("item".to_string(), "banana".to_string());
        row2.insert("price".to_string(), "0.5".to_string());
        row2.insert("qty".to_string(), "20".to_string());

        let _ = super::super::operations::add_rows(&config, AddRowsInput {
            db_name: "sales".to_string(),
            rows: vec![row1, row2],
        });

        let q_res = query_csv(&config, QueryRequest {
            db_name: "sales".to_string(),
            predicate: Some("price < 1.0".to_string()),
            aggregate_col: None,
            aggregate_func: None,
        }).unwrap();
        assert_eq!(q_res.rows.len(), 1);
        assert_eq!(q_res.rows[0].get("item").unwrap(), "banana");

        // Test aggregate sum
        let q_res2 = query_csv(&config, QueryRequest {
            db_name: "sales".to_string(),
            predicate: None,
            aggregate_col: Some("qty".to_string()),
            aggregate_func: Some("sum".to_string()),
        }).unwrap();
        assert_eq!(q_res2.aggregate_result, Some(30.0));

        // Test aggregate average
        let q_res_avg = query_csv(&config, QueryRequest {
            db_name: "sales".to_string(),
            predicate: None,
            aggregate_col: Some("price".to_string()),
            aggregate_func: Some("avg".to_string()),
        }).unwrap();
        assert_eq!(q_res_avg.aggregate_result, Some(1.0));

        // Test unsupported aggregate
        let err_agg = query_csv(&config, QueryRequest {
            db_name: "sales".to_string(),
            predicate: None,
            aggregate_col: Some("qty".to_string()),
            aggregate_func: Some("max".to_string()),
        }).unwrap_err();
        assert!(err_agg.contains("Unsupported aggregate function"));

        // Test query invalid database
        let err_not_exist = query_csv(&config, QueryRequest {
            db_name: "missing".to_string(),
            predicate: None,
            aggregate_col: None,
            aggregate_func: None,
        }).unwrap_err();
        assert!(err_not_exist.contains("does not exist"));

        // Test delete
        let d_res = delete_rows(&config, DeleteRowsInput {
            db_name: "sales".to_string(),
            predicate: "item == \"apple\"".to_string(),
        }).unwrap();
        assert!(d_res.contains("Deleted 1 rows"));

        let q_res3 = query_csv(&config, QueryRequest {
            db_name: "sales".to_string(),
            predicate: None,
            aggregate_col: None,
            aggregate_func: None,
        }).unwrap();
        assert_eq!(q_res3.rows.len(), 1);
        assert_eq!(q_res3.rows[0].get("item").unwrap(), "banana");

        // Test delete invalid predicate
        let err_pred = delete_rows(&config, DeleteRowsInput {
            db_name: "sales".to_string(),
            predicate: "invalid syntax ++".to_string(),
        }).unwrap_err();
        assert!(err_pred.contains("Invalid predicate") || err_pred.contains("Evaluation error"), "Actual error: {}", err_pred);
    }
}
