//! YAML front-matter tools — `read_yaml_header` and `write_yaml_header` for title, summary, tags, date, etc.

use crate::markdown::Document;
use serde_norway::{Mapping, Value};
use std::path::Path;

pub fn tool_read_yaml_header(
    ctx: &crate::agent::tools::context::ToolContext,
    path_str: &str,
) -> Result<crate::agent::tools::dtos::ReadYamlHeaderResponse, String> {
    match ctx.vfs().read_to_string(path_str.as_ref()) {
        Ok(content) => {
            // `Document` parses the front matter (if any) in one
            // pass; reaching `front_matter()` is the same call the
            // editor and the orchestrator use.
            if let Some(fm) = Document::new(content).front_matter() {
                Ok(crate::agent::tools::dtos::ReadYamlHeaderResponse {
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
    ctx: &crate::agent::tools::context::ToolContext,
    path_str: &str,
    title: Option<&str>,
    summary: Option<&str>,
    tags: Option<Vec<String>>,
    header_date: Option<&str>,
    producer: &dyn crate::agent::tools::observer::OnFileChanged,
) -> Result<crate::agent::tools::dtos::WriteYamlHeaderResponse, String> {
    let existed = Path::new(path_str).exists();
    let current_content = ctx
        .vfs()
        .read_to_string(path_str.as_ref())
        .unwrap_or_else(|_| "".to_string());

    // `Document::body()` returns the source with the front-matter
    // block stripped when one is present, or the full source
    // otherwise — exactly the slice we want to preserve verbatim
    // when rewriting the header.
    let markdown_body = Document::new(current_content).body().to_string();

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
                    // Was the file created or updated? Publish the
                    // matching event so consumers (directory tree,
                    // tag manager) refresh.
                    if existed {
                        producer.on_file_changed(
                            path,
                            crate::bus::events::file::FileEventKind::Updated,
                        );
                    } else {
                        producer.on_file_changed(
                            path,
                            crate::bus::events::file::FileEventKind::Discovered,
                        );
                    }
                    Ok(crate::agent::tools::dtos::WriteYamlHeaderResponse {
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

    fn test_ctx() -> crate::agent::tools::context::ToolContext {
        let config = crate::agent::config::AgentConfig::default();
        let mut builder = crate::agent::tools::context::ToolContextBuilder::new(
            std::sync::Arc::new(config.clone()),
            std::sync::Arc::new(crate::agent::tools::observer::DefaultFileObserver),
        );
        builder = builder.with_extension(std::sync::Arc::new(
            crate::agent::tools::vfs::VirtualFileSystemExt(std::sync::Arc::new(
                crate::agent::tools::vfs::VfsResolver::new(std::sync::Arc::new(config.clone())),
            )),
        ));
        builder.build()
    }

    use super::*;
    use crate::bus::events::file::FileEventKind;
    use std::fs;
    use tempfile::tempdir;

    /// A producer that publishes to a throwaway bus. Tests don't
    /// need to consume the events — they only care about the
    /// success/failure of the underlying file operation.
    fn noop_producer() -> std::sync::Arc<dyn crate::agent::tools::observer::OnFileChanged> {
        std::sync::Arc::new(crate::agent::tools::observer::DefaultFileObserver)
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
        // `Document::front_matter()` extracts only the YAML block
        // between the `---` delimiters, mirroring how the app reads
        // headers. Parsing the full file (front matter + body)
        // would be multi-document YAML, which `serde_norway` rejects.
        let doc = crate::markdown::Document::new(content);
        let fm = doc.front_matter().expect("front matter should parse");
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

    #[test]
    fn test_tool_write_yaml_header_publishes_discovered_for_new_file() {
        // A brand new file must publish a Discovered event so the
        // directory tree and tag manager pick it up.
        let bus = crate::bus::core::Bus::new();
        let reader = bus.subscribe();
        let producer: std::sync::Arc<dyn crate::agent::tools::observer::OnFileChanged> =
            std::sync::Arc::new(crate::app::session::bus_observer::AppFileObserver::new(
                bus.clone(),
            ));

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("brand_new.md");

        tool_write_yaml_header(
            &test_ctx(),
            file_path.to_str().unwrap(),
            Some("Title"),
            None,
            None,
            None,
            &*producer,
        )
        .unwrap();

        let event = reader
            .recv_timeout(std::time::Duration::from_millis(100))
            .unwrap();
        assert_eq!(event.kind, FileEventKind::Discovered);
        assert_eq!(event.paths[0], file_path);
    }

    #[test]
    fn test_tool_write_yaml_header_publishes_updated_for_existing_file() {
        // An existing file getting its header rewritten must
        // publish an Updated event.
        let bus = crate::bus::core::Bus::new();
        let reader = bus.subscribe();
        let producer: std::sync::Arc<dyn crate::agent::tools::observer::OnFileChanged> =
            std::sync::Arc::new(crate::app::session::bus_observer::AppFileObserver::new(
                bus.clone(),
            ));

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
            &*producer,
        )
        .unwrap();

        let event = reader
            .recv_timeout(std::time::Duration::from_millis(100))
            .unwrap();
        assert_eq!(event.kind, FileEventKind::Updated);
        assert_eq!(event.paths[0], file_path);
    }
}

// Property tests for the YAML frontmatter parser. See the file for
// the surface under test (panic-freedom, key/value round-trip,
// unclosed-frontmatter rejection). Sidecar of yaml_header.rs per
// AGENTS.md RUST-056 / RUST-057.
#[cfg(test)]
#[path = "yaml_header_proptests.rs"]
mod proptests;
