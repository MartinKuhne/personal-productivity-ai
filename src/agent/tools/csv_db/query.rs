//! CSV-database query and delete operations — expression-based predicate evaluation against row values.
//!
//! Unit tests live in the sibling `query_tests.rs` sidecar.

use super::predicate::{Predicate, Value};
use super::schema::{DeleteRowsInput, QueryRequest, QueryResponse};

use std::collections::HashMap;

fn create_context(row: &csv::StringRecord, headers: &csv::StringRecord) -> HashMap<String, Value> {
    let mut context = HashMap::new();
    for (i, header) in headers.iter().enumerate() {
        if let Some(val) = row.get(i) {
            if let Ok(num) = val.parse::<i64>() {
                context.insert(header.to_string(), Value::Int(num));
            } else if let Ok(num) = val.parse::<f64>() {
                context.insert(header.to_string(), Value::Float(num));
            } else {
                context.insert(header.to_string(), Value::String(val.to_string()));
            }
        }
    }
    context
}

pub fn delete_rows(
    ctx: &crate::tools::context::ToolContext,
    input: DeleteRowsInput,
) -> Result<String, String> {
    let db_path = super::operations::get_db_path(ctx, &input.db_name);
    if !db_path.exists() {
        return Err(format!("Database '{}' does not exist", input.db_name));
    }

    let mut rdr =
        csv::Reader::from_path(&db_path).map_err(|e| format!("Failed to read csv: {}", e))?;
    let headers = rdr
        .headers()
        .map_err(|e| format!("Failed to read headers: {}", e))?
        .clone();

    let mut kept_rows = Vec::new();
    let mut deleted_count = 0;

    let predicate = Predicate::parse(&input.predicate)?;

    for result in rdr.records() {
        let record = result.map_err(|e| format!("Invalid record: {}", e))?;
        let context = create_context(&record, &headers);
        let eval_res = predicate.eval_boolean(&context)?;

        if eval_res {
            deleted_count += 1;
        } else {
            kept_rows.push(record);
        }
    }

    let mut wtr =
        csv::Writer::from_path(&db_path).map_err(|e| format!("Failed to open for write: {}", e))?;
    wtr.write_record(&headers)
        .map_err(|e| format!("Failed to write headers: {}", e))?;
    for record in kept_rows {
        wtr.write_record(&record)
            .map_err(|e| format!("Failed to write record: {}", e))?;
    }
    wtr.flush().map_err(|e| format!("Failed to flush: {}", e))?;

    Ok(format!("Deleted {} rows", deleted_count))
}

pub fn query_csv(
    ctx: &crate::tools::context::ToolContext,
    input: QueryRequest,
) -> Result<QueryResponse, String> {
    let db_path = super::operations::get_db_path(ctx, &input.db_name);
    if !db_path.exists() {
        return Err(format!("Database '{}' does not exist", input.db_name));
    }

    let mut rdr =
        csv::Reader::from_path(&db_path).map_err(|e| format!("Failed to read csv: {}", e))?;
    let headers = rdr
        .headers()
        .map_err(|e| format!("Failed to read headers: {}", e))?
        .clone();

    let mut matched_rows = Vec::new();

    let predicate = match &input.predicate {
        Some(p) => Some(Predicate::parse(p)?),
        None => None,
    };

    for result in rdr.records() {
        let record = result.map_err(|e| format!("Invalid record: {}", e))?;
        if let Some(ref pred) = predicate {
            let context = create_context(&record, &headers);
            let eval_res = pred.eval_boolean(&context)?;
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
            if let Some(val_str) = row.get(col)
                && let Ok(num) = val_str.parse::<f64>()
            {
                sum += num;
                count += 1;
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
            }
            _ => return Err(format!("Unsupported aggregate function: {}", func)),
        }
    }

    Ok(QueryResponse {
        rows: matched_rows,
        aggregate_result,
    })
}

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;

// Property tests for the csv_db query builder. The proptest
// exercises the seven core properties of the query / delete
// pipeline: panic-freedom, tautology / contradiction handling,
// sum / average aggregates, and the all-rows delete. Sidecar of
// query.rs per AGENTS.md RUST-056 / RUST-057.
#[cfg(test)]
#[path = "query_proptests.rs"]
mod proptests;
