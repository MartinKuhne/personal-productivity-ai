use crate::file_events::FileEventProducer;
use crate::utils::markdown::parse_front_matter;
use serde_yaml::{Mapping, Value};
use std::path::Path;

pub fn tool_read_yaml_header(
    path_str: &str,
) -> Result<crate::tools::dtos::ReadYamlHeaderResponse, String> {
    match std::fs::read_to_string(path_str) {
        Ok(content) => {
            if let Some((yaml_val, _)) = parse_front_matter(&content) {
                Ok(crate::tools::dtos::ReadYamlHeaderResponse {
                    content: format!("{:#?}", yaml_val),
                })
            } else {
                tracing::warn!(name = "tool.yaml.read_no_header", path = %path_str, "No YAML header found in this file. Operator should check if the file is expected to have one.");
                Err("No YAML header found in this file.".to_string())
            }
        }
        Err(e) => {
            tracing::error!(name = "tool.yaml.read_failed", error = %e, path = %path_str, "Failed to read file for YAML header processing. Likely cause: file missing or permission denied.");
            Err(format!("Failed to read file: {}", e))
        }
    }
}

pub fn tool_write_yaml_header(
    path_str: &str,
    title: Option<&str>,
    summary: Option<&str>,
    tags: Option<Vec<String>>,
    header_date: Option<&str>,
    producer: &FileEventProducer,
) -> Result<crate::tools::dtos::WriteYamlHeaderResponse, String> {
    let existed = Path::new(path_str).exists();
    let current_content = std::fs::read_to_string(path_str).unwrap_or_else(|_| "".to_string());

    let markdown_body = if let Some((_, md)) = parse_front_matter(&current_content) {
        md.to_string()
    } else {
        current_content
    };

    let mut map = Mapping::new();
    if let Some(t) = title {
        map.insert(
            Value::String("title".to_string()),
            Value::String(t.to_string()),
        );
    }
    if let Some(s) = summary {
        map.insert(
            Value::String("summary".to_string()),
            Value::String(s.to_string()),
        );
    }
    if let Some(tg) = tags {
        let seq: Vec<Value> = tg.into_iter().map(Value::String).collect();
        map.insert(Value::String("tags".to_string()), Value::Sequence(seq));
    }
    if let Some(hd) = header_date {
        map.insert(
            Value::String("header-date".to_string()),
            Value::String(hd.to_string()),
        );
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
                Ok(_) => {
                    // Was the file created or updated? Publish the
                    // matching event so consumers (directory tree,
                    // tag manager) refresh.
                    if existed {
                        producer.publish_updated(path);
                    } else {
                        producer.publish_discovered(path);
                    }
                    Ok(crate::tools::dtos::WriteYamlHeaderResponse {
                        result: "YAML header written successfully.".to_string(),
                    })
                }
                Err(e) => {
                    tracing::error!(name = "tool.yaml.write_failed", error = %e, path = %path_str, "Failed to write file after YAML header update. Likely cause: disk full or permission denied.");
                    Err(format!("Failed to write file: {}", e))
                }
            }
        }
        Err(e) => {
            tracing::error!(name = "tool.yaml.serialize_failed", error = %e, path = %path_str, "Failed to serialize value to YAML. Operator should check the provided YAML parameters.");
            Err(format!("Failed to serialize value to YAML: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_events::Bus;
    use std::fs;
    use tempfile::tempdir;

    /// A producer that publishes to a throwaway bus. Tests don't
    /// need to consume the events — they only care about the
    /// success/failure of the underlying file operation.
    fn noop_producer() -> FileEventProducer<'static> {
        let bus: &'static Bus<crate::file_events::FileEvent> = Box::leak(Box::new(Bus::new()));
        FileEventProducer::new(bus)
    }

    #[test]
    fn test_tool_read_yaml_header() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "---\ntitle: Test\ntags: [tag1]\n---\nContent").unwrap();

        let result = tool_read_yaml_header(file_path.to_str().unwrap())
            .unwrap()
            .content;
        assert!(result.contains("title"));
        assert!(result.contains("Test"));
    }

    #[test]
    fn test_tool_read_yaml_header_no_header() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "No header here").unwrap();

        let result = tool_read_yaml_header(file_path.to_str().unwrap());
        assert_eq!(result.unwrap_err(), "No YAML header found in this file.");
    }

    #[test]
    fn test_tool_write_yaml_header_new_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new.md");

        let producer = noop_producer();
        let result = tool_write_yaml_header(
            file_path.to_str().unwrap(),
            Some("Test Title"),
            Some("Test summary"),
            Some(vec!["tag1".to_string(), "tag2".to_string()]),
            Some("2024-01-01T00:00:00Z"),
            &producer,
        )
        .unwrap()
        .result;

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

        let producer = noop_producer();
        let result = tool_write_yaml_header(
            file_path.to_str().unwrap(),
            Some("New Title"),
            None,
            None,
            None,
            &producer,
        )
        .unwrap()
        .result;

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

        let producer = noop_producer();
        let result = tool_write_yaml_header(
            file_path.to_str().unwrap(),
            Some("Title"),
            None,
            None,
            None,
            &producer,
        )
        .unwrap()
        .result;

        assert_eq!(result, "YAML header written successfully.");
        assert!(file_path.exists());
    }

    #[test]
    fn test_tool_write_yaml_header_publishes_discovered_for_new_file() {
        // A brand new file must publish a Discovered event so the
        // directory tree and tag manager pick it up.
        let bus: Bus<crate::file_events::FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("brand_new.md");

        tool_write_yaml_header(
            file_path.to_str().unwrap(),
            Some("Title"),
            None,
            None,
            None,
            &producer,
        )
        .unwrap();

        let event = reader
            .recv_timeout(std::time::Duration::from_millis(100))
            .unwrap();
        assert_eq!(event.kind, crate::file_events::FileEventKind::Discovered);
        assert_eq!(event.path, file_path);
    }

    #[test]
    fn test_tool_write_yaml_header_publishes_updated_for_existing_file() {
        // An existing file getting its header rewritten must
        // publish an Updated event.
        let bus: Bus<crate::file_events::FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("existing.md");
        fs::write(&file_path, "# Body").unwrap();

        tool_write_yaml_header(
            file_path.to_str().unwrap(),
            Some("New Title"),
            None,
            None,
            None,
            &producer,
        )
        .unwrap();

        let event = reader
            .recv_timeout(std::time::Duration::from_millis(100))
            .unwrap();
        assert_eq!(event.kind, crate::file_events::FileEventKind::Updated);
        assert_eq!(event.path, file_path);
    }
}
