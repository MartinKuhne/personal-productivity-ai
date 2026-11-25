//! YAML front-matter tools — `read_yaml_header` and `write_yaml_header` for title, summary, tags, date, etc.
//!
//! Unit tests live in the sibling `yaml_header_tests.rs` sidecar.

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
#[path = "yaml_header_tests.rs"]
mod tests;

// Property tests for the YAML frontmatter parser. See the file for
// the surface under test (panic-freedom, key/value round-trip,
// unclosed-frontmatter rejection). Sidecar of yaml_header.rs per
// AGENTS.md RUST-056 / RUST-057.
#[cfg(test)]
#[path = "yaml_header_proptests.rs"]
mod proptests;
