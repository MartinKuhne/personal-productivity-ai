use std::path::Path;

/// Extract tags from a markdown file's YAML front matter.
/// Tags are normalized to lowercase.
pub fn extract_tags_from_file(path: &Path) -> Vec<String> {
    let mut tags = Vec::new();
    if let Ok(content) = std::fs::read_to_string(path)
        && let Some(yaml_str) = extract_yaml_front_matter(&content)
        && let Ok(yaml_val) = serde_norway::from_str::<serde_norway::Value>(yaml_str)
        && let Some(mapping) = yaml_val.as_mapping()
        && let Some(tags_val) = mapping.get("tags")
    {
        if let Some(arr) = tags_val.as_sequence() {
            for item in arr {
                if let Some(s) = item.as_str() {
                    tags.push(s.to_lowercase());
                }
            }
        } else if let Some(s) = tags_val.as_str() {
            tags.push(s.to_lowercase());
        }
    }
    tags
}

fn extract_yaml_front_matter(content: &str) -> Option<&str> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let rest = &trimmed[3..];
    let end_idx = rest.find("\n---")?;
    Some(&rest[..end_idx])
}
