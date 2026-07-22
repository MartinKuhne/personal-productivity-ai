//! CSV-database CRUD operations — create a new CSV database, add rows, list databases, and path resolution.

use super::schema::{AddRowsInput, CreateCsvInput, CsvDatabase, ListCsvInput};
use crate::config::AppConfig;
use std::path::PathBuf;

pub fn get_db_dir(config: &AppConfig) -> PathBuf {
    let path = if let Some(ref override_path) = config.csv_db_path {
        PathBuf::from(override_path)
    } else {
        let app_data = std::env::var("APPDATA").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            format!("{}/.config", home)
        });
        let mut p = PathBuf::from(app_data);
        p.push("fastmd");
        p.push("db");
        p
    };

    let _ = std::fs::create_dir_all(&path);
    path
}

pub fn get_db_path(config: &AppConfig, db_name: &str) -> PathBuf {
    let mut path = get_db_dir(config);
    path.push(format!("{}.csv", db_name));
    path
}

pub fn create_csv(config: &AppConfig, input: CreateCsvInput) -> Result<CsvDatabase, String> {
    let db_path = get_db_path(config, &input.db_name);
    if db_path.exists() {
        return Err(format!("Database '{}' already exists", input.db_name));
    }

    let mut wtr =
        csv::Writer::from_path(&db_path).map_err(|e| format!("Failed to create csv: {}", e))?;
    wtr.write_record(&input.headers)
        .map_err(|e| format!("Failed to write headers: {}", e))?;
    wtr.flush().map_err(|e| format!("Failed to flush: {}", e))?;

    Ok(CsvDatabase {
        name: input.db_name,
        path: db_path,
        headers: input.headers,
    })
}

pub fn list_csv(config: &AppConfig, _input: ListCsvInput) -> Result<Vec<CsvDatabase>, String> {
    let db_dir = get_db_dir(config);
    let mut dbs = Vec::new();

    if let Ok(entries) = std::fs::read_dir(db_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("csv") {
                let name = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                if let Ok(mut rdr) = csv::ReaderBuilder::new()
                    .has_headers(false)
                    .from_path(&path)
                {
                    if let Some(Ok(header_record)) = rdr.records().next() {
                        let headers: Vec<String> =
                            header_record.iter().map(|s| s.to_string()).collect();
                        dbs.push(CsvDatabase {
                            name,
                            path,
                            headers,
                        });
                    }
                }
            }
        }
    }

    Ok(dbs)
}

pub fn add_rows(config: &AppConfig, input: AddRowsInput) -> Result<String, String> {
    let db_path = get_db_path(config, &input.db_name);
    if !db_path.exists() {
        return Err(format!("Database '{}' does not exist", input.db_name));
    }

    let mut rdr =
        csv::Reader::from_path(&db_path).map_err(|e| format!("Failed to read csv: {}", e))?;
    let headers_record = rdr
        .headers()
        .map_err(|e| format!("Failed to read headers: {}", e))?
        .clone();
    let headers: Vec<String> = headers_record.iter().map(|s| s.to_string()).collect();

    for (i, row) in input.rows.iter().enumerate() {
        for key in row.keys() {
            if !headers.contains(key) {
                return Err(format!(
                    "Row {} contains invalid header: '{}'. Valid headers are: {:?}",
                    i, key, headers
                ));
            }
        }
        for header in &headers {
            if !row.contains_key(header) {
                return Err(format!(
                    "Row {} is missing required header: '{}'",
                    i, header
                ));
            }
        }
    }

    let file = std::fs::OpenOptions::new()
        .write(true)
        .append(true)
        .open(&db_path)
        .map_err(|e| format!("Failed to open file: {}", e))?;
    let mut wtr = csv::Writer::from_writer(file);

    for row in &input.rows {
        let mut record = Vec::new();
        for header in &headers {
            record.push(row.get(header).cloned().unwrap_or_default());
        }
        wtr.write_record(&record)
            .map_err(|e| format!("Failed to write row: {}", e))?;
    }
    wtr.flush().map_err(|e| format!("Failed to flush: {}", e))?;

    Ok(format!("Added {} rows", input.rows.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn test_create_and_list_and_add_rows() {
        let dir = tempdir().unwrap();
        let mut config = AppConfig::default();
        config.csv_db_path = Some(dir.path().to_string_lossy().to_string());

        let create_input = CreateCsvInput {
            db_name: "test_db".to_string(),
            headers: vec!["id".to_string(), "name".to_string(), "age".to_string()],
        };
        let db = create_csv(&config, create_input.clone()).unwrap();
        assert_eq!(db.name, "test_db");
        assert_eq!(db.headers.len(), 3);

        // Test creating again should fail
        let err_create = create_csv(&config, create_input).unwrap_err();
        assert!(err_create.contains("already exists"));

        let list = list_csv(&config, ListCsvInput {}).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "test_db");

        let mut row1 = HashMap::new();
        row1.insert("id".to_string(), "1".to_string());
        row1.insert("name".to_string(), "Alice".to_string());
        row1.insert("age".to_string(), "25".to_string());

        let mut row2 = HashMap::new();
        row2.insert("id".to_string(), "2".to_string());
        row2.insert("name".to_string(), "Bob".to_string());
        row2.insert("age".to_string(), "30".to_string());

        let add_input = AddRowsInput {
            db_name: "test_db".to_string(),
            rows: vec![row1, row2],
        };
        let add_res = add_rows(&config, add_input).unwrap();
        assert!(add_res.contains("Added 2 rows"));

        // Test invalid header
        let mut bad_row = HashMap::new();
        bad_row.insert("invalid_header".to_string(), "1".to_string());
        let bad_input = AddRowsInput {
            db_name: "test_db".to_string(),
            rows: vec![bad_row],
        };
        let err = add_rows(&config, bad_input).unwrap_err();
        assert!(err.contains("invalid header"));

        // Test missing required header
        let mut missing_row = HashMap::new();
        missing_row.insert("id".to_string(), "3".to_string());
        missing_row.insert("name".to_string(), "Charlie".to_string());
        let missing_input = AddRowsInput {
            db_name: "test_db".to_string(),
            rows: vec![missing_row],
        };
        let err_missing = add_rows(&config, missing_input).unwrap_err();
        assert!(err_missing.contains("missing required header"));

        // Test add to non-existent db
        let err_not_exist = add_rows(
            &config,
            AddRowsInput {
                db_name: "missing_db".to_string(),
                rows: vec![],
            },
        )
        .unwrap_err();
        assert!(err_not_exist.contains("does not exist"));
    }
}
