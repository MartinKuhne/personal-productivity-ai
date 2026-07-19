use crate::utils::markdown::parse_front_matter;
use serde_yaml::{Mapping, Value};
use std::path::Path;

pub fn tool_read_yaml_header(path_str: &str) -> String {
    match std::fs::read_to_string(path_str) {
        Ok(content) => {
            if let Some((yaml_val, _)) = parse_front_matter(&content) {
                format!("{:#?}", yaml_val)
            } else {
                "No YAML header found in this file.".to_string()
            }
        }
        Err(e) => format!("Error reading file: {}", e),
    }
}

pub fn tool_write_yaml_header(
    path_str: &str,
    title: Option<&str>,
    summary: Option<&str>,
    tags: Option<Vec<String>>,
    header_date: Option<&str>,
) -> String {
    let current_content = std::fs::read_to_string(path_str).unwrap_or_else(|_| "".to_string());

    let markdown_body = if let Some((_, md)) = parse_front_matter(&current_content) {
        md.to_string()
    } else {
        current_content
    };

    let mut map = Mapping::new();
    if let Some(t) = title {
        map.insert(Value::String("title".to_string()), Value::String(t.to_string()));
    }
    if let Some(s) = summary {
        map.insert(Value::String("summary".to_string()), Value::String(s.to_string()));
    }
    if let Some(tg) = tags {
        let seq: Vec<Value> = tg.into_iter().map(Value::String).collect();
        map.insert(Value::String("tags".to_string()), Value::Sequence(seq));
    }
    if let Some(hd) = header_date {
        map.insert(Value::String("header-date".to_string()), Value::String(hd.to_string()));
    }

    let yaml_val = Value::Mapping(map);
    match serde_yaml::to_string(&yaml_val) {
        Ok(yaml_str) => {
            let yaml_inner = yaml_str.trim_start_matches("---\n");
            let mut yaml_final = yaml_inner.to_string();
            if !yaml_final.ends_with('\n') {
                yaml_final.push('\n');
            }
            let new_content = format!("---\n{}---\n{}", yaml_final, markdown_body.trim_start());
            let path = Path::new(path_str);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(path_str, new_content) {
                Ok(_) => "YAML header written successfully.".to_string(),
                Err(e) => format!("Error writing file: {}", e),
            }
        }
        Err(e) => format!("Error serializing value to YAML: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_tool_read_yaml_header() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "---\ntitle: Test\ntags: [tag1]\n---\nContent").unwrap();
        
        let result = tool_read_yaml_header(file_path.to_str().unwrap());
        assert!(result.contains("title"));
        assert!(result.contains("Test"));
    }

    #[test]
    fn test_tool_read_yaml_header_no_header() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "No header here").unwrap();
        
        let result = tool_read_yaml_header(file_path.to_str().unwrap());
        assert_eq!(result, "No YAML header found in this file.");
    }

    #[test]
    fn test_tool_write_yaml_header_new_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new.md");
        
        let result = tool_write_yaml_header(
            file_path.to_str().unwrap(),
            Some("Test Title"),
            Some("Test summary"),
            Some(vec!["tag1".to_string(), "tag2".to_string()]),
            Some("2024-01-01T00:00:00Z"),
        );
        
        assert_eq!(result, "YAML header written successfully.");
        
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("title: Test Title"));
        assert!(content.contains("summary: Test summary"));
        assert!(content.contains("tags:"));
        assert!(content.contains("tag1"));
        assert!(content.contains("tag2"));
        assert!(content.contains("header-date: 2024-01-01T00:00:00Z"));
    }

    #[test]
    fn test_tool_write_yaml_header_preserves_body() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "---\ntitle: Old\n---\n# Body Content").unwrap();
        
        let result = tool_write_yaml_header(
            file_path.to_str().unwrap(),
            Some("New Title"),
            None,
            None,
            None,
        );
        
        assert_eq!(result, "YAML header written successfully.");
        
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("title: New Title"));
        assert!(content.contains("# Body Content"));
        assert!(!content.contains("Old"));
    }

    #[test]
    fn test_tool_write_yaml_header_creates_dirs() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("subdir").join("test.md");
        
        let result = tool_write_yaml_header(
            file_path.to_str().unwrap(),
            Some("Title"),
            None,
            None,
            None,
        );
        
        assert_eq!(result, "YAML header written successfully.");
        assert!(file_path.exists());
    }
}