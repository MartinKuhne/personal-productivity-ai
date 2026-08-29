//! YAML front-matter tools — `read_yaml_header` and `write_yaml_header` for title, summary, tags, date, etc.

use crate::utils::markdown::parse_front_matter;
use serde_norway::{Mapping, Value};
use std::path::Path;

pub fn tool_read_yaml_header(
    ctx: &crate::tools::context::ToolContext,
    path_str: &str,
) -> Result<crate::tools::dtos::ReadYamlHeaderResponse, String> {
    match ctx.vfs().read_to_string(path_str.as_ref()) {
        Ok(content) => {
            if let Some(fm) = parse_front_matter(&content) {
                Ok(crate::tools::dtos::ReadYamlHeaderResponse {
                    content: format!("{:#?}", fm.yaml),
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
    ctx: &crate::tools::context::ToolContext,
    path_str: &str,
    title: Option<&str>,
    summary: Option<&str>,
    tags: Option<Vec<String>>,
    header_date: Option<&str>,
    producer: &dyn crate::tools::observer::OnFileChanged,
) -> Result<crate::tools::dtos::WriteYamlHeaderResponse, String> {
    let current_content = ctx
        .vfs()
        .read_to_string(path_str.as_ref())
        .unwrap_or_else(|_| "".to_string());

    let markdown_body = parse_front_matter(&current_content)
        .map(|fm| fm.body)
        .unwrap_or(current_content);

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
    match serde_norway::to_string(&yaml_val) {
        Ok(yaml_str) => {
            let yaml_inner = yaml_str.trim_start_matches("---\n");
            let mut yaml_final = yaml_inner.to_string();
            if !yaml_final.ends_with('\n') {
                yaml_final.push('\n');
            }
            let new_content = format!("---\n{}---\n{}", yaml_final, markdown_body.trim_start());
            let path = Path::new(path_str);
            if let Some(parent) = path.parent() {
                let _ = ctx.vfs().create_dir_all(parent);
            }
            match ctx.vfs().write(path_str.as_ref(), new_content.as_bytes()) {
                Ok(_) => {
                    producer.on_file_changed(path);
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

    fn test_ctx() -> crate::tools::context::ToolContext {
        let config = crate::config::AgentConfig::default();
        let mut builder = crate::tools::context::ToolContextBuilder::new(
            std::sync::Arc::new(config.clone()),
            std::sync::Arc::new(crate::tools::observer::DefaultFileObserver),
        );
        builder = builder.with_extension(std::sync::Arc::new(
            crate::tools::vfs::VirtualFileSystemExt(std::sync::Arc::new(
                crate::tools::vfs::VfsResolver::new(std::sync::Arc::new(config.clone())),
            )),
        ));
        builder.build()
    }

    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// A producer that publishes to a throwaway bus. Tests don't
    /// need to consume the events — they only care about the
    /// success/failure of the underlying file operation.
    fn noop_producer() -> std::sync::Arc<dyn crate::tools::observer::OnFileChanged> {
        std::sync::Arc::new(crate::tools::observer::DefaultFileObserver)
    }

    #[test]
    fn test_tool_read_yaml_header() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "---\ntitle: Test\ntags: [tag1]\n---\nContent").unwrap();

        let result = tool_read_yaml_header(&test_ctx(), file_path.to_str().unwrap())
            .unwrap()
            .content;
        assert!(result.contains("title"));
        assert!(result.contains("Test"));
    }

    #[test]
    fn test_tool_read_yaml_header_missing_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("missing.md");

        let result = tool_read_yaml_header(&test_ctx(), file_path.to_str().unwrap());
        let err = result.unwrap_err();
        assert!(
            err.starts_with("Failed to read file:"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn test_tool_write_yaml_header_new_file_preserves_no_leading_whitespace() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("fresh.md");

        let producer = noop_producer();
        tool_write_yaml_header(
            &test_ctx(),
            file_path.to_str().unwrap(),
            Some("Title"),
            Some("Summary"),
            None,
            None,
            &*producer,
        )
        .unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.starts_with("---\n"));
        assert!(content.contains("title: Title"));
        assert!(content.contains("summary: Summary"));
    }

    #[test]
    fn test_tool_read_yaml_header_no_header() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "No header here").unwrap();

        let result = tool_read_yaml_header(&test_ctx(), file_path.to_str().unwrap());
        assert_eq!(result.unwrap_err(), "No YAML header found in this file.");
    }

    #[test]
    fn test_tool_write_yaml_header_new_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new.md");

        let producer = noop_producer();
        let result = tool_write_yaml_header(
            &test_ctx(),
            file_path.to_str().unwrap(),
            Some("Test Title"),
            Some("Test summary"),
            Some(vec!["tag1".to_string(), "tag2".to_string()]),
            Some("2024-01-01T00:00:00Z"),
            &*producer,
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
        // `serde_norway` (YAML 1.2 strict) explicitly quotes strings that
        // match the timestamp regex to avoid silent re-typing on re-parse
        // — accept either form to keep this test stable across YAML
        // emitters, and assert the value round-trips.
        assert!(
            content.contains("header-date: 2024-01-01T00:00:00Z")
                || content.contains("header-date: \"2024-01-01T00:00:00Z\""),
            "header-date not present in expected form: {content}"
        );
        let fm = parse_front_matter(&content).expect("front matter should parse");
        assert_eq!(
            fm.yaml.get("header-date").and_then(|v| v.as_str()),
            Some("2024-01-01T00:00:00Z")
        );
    }

    #[test]
    fn test_tool_write_yaml_header_preserves_body() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "---\ntitle: Old\n---\n# Body Content").unwrap();

        let producer = noop_producer();
        let result = tool_write_yaml_header(
            &test_ctx(),
            file_path.to_str().unwrap(),
            Some("New Title"),
            None,
            None,
            None,
            &*producer,
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
            &test_ctx(),
            file_path.to_str().unwrap(),
            Some("Title"),
            None,
            None,
            None,
            &*producer,
        )
        .unwrap()
        .result;

        assert_eq!(result, "YAML header written successfully.");
        assert!(file_path.exists());
    }

    #[derive(Clone, Default)]
    struct RecordingFileObserver(std::sync::Arc<std::sync::Mutex<Vec<std::path::PathBuf>>>);
    impl crate::tools::observer::OnFileChanged for RecordingFileObserver {
        fn on_file_changed(&self, path: &std::path::Path) {
            self.0.lock().unwrap().push(path.to_path_buf());
        }
    }

    #[test]
    fn test_tool_write_yaml_header_publishes_event_on_write() {
        let recorded = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let producer = RecordingFileObserver(recorded.clone());

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("brand_new.md");

        tool_write_yaml_header(
            &test_ctx(),
            file_path.to_str().unwrap(),
            Some("Title"),
            None,
            None,
            None,
            &producer,
        )
        .unwrap();

        assert_eq!(*recorded.lock().unwrap(), vec![file_path]);
    }

    #[test]
    fn test_tool_write_yaml_header_publishes_updated_for_existing_file() {
        let recorded = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let producer = RecordingFileObserver(recorded.clone());

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("existing.md");
        fs::write(&file_path, "# Body").unwrap();

        tool_write_yaml_header(
            &test_ctx(),
            file_path.to_str().unwrap(),
            Some("New Title"),
            None,
            None,
            None,
            &producer,
        )
        .unwrap();

        assert_eq!(*recorded.lock().unwrap(), vec![file_path]);
    }
}

// Property tests for the YAML frontmatter parser. See the file for
// the surface under test (panic-freedom, key/value round-trip,
// unclosed-frontmatter rejection). Sidecar of yaml_header.rs per
// AGENTS.md RUST-056 / RUST-057.
#[cfg(test)]
#[path = "yaml_header_proptests.rs"]
mod proptests;
