use crate::utils::markdown::parse_front_matter;
use std::path::Path;

/// Extract tags from a markdown file's YAML front matter.
/// Tags are normalized to lowercase.
pub fn extract_tags_from_file(path: &Path) -> Vec<String> {
    let mut tags = Vec::new();
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Some((yaml_val, _)) = parse_front_matter(&content) {
            if let Some(mapping) = yaml_val.as_mapping() {
                if let Some(tags_val) = mapping.get(serde_yaml::Value::String("tags".to_string())) {
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
            }
        }
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_extract_tags_from_file_with_array() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "---\ntags: [tag1, tag2]\n---\nContent").unwrap();

        let tags = extract_tags_from_file(&file_path);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"tag1".to_string()));
        assert!(tags.contains(&"tag2".to_string()));
    }

    #[test]
    fn test_extract_tags_from_file_with_string() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "---\ntags: my-tag\n---\nContent").unwrap();

        let tags = extract_tags_from_file(&file_path);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], "my-tag");
    }

    #[test]
    fn test_extract_tags_from_file_no_tags() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "---\ntitle: No Tags\n---\nContent").unwrap();

        let tags = extract_tags_from_file(&file_path);
        assert!(tags.is_empty());
    }

    #[test]
    fn test_extract_tags_from_file_no_front_matter() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Just content, no front matter").unwrap();

        let tags = extract_tags_from_file(&file_path);
        assert!(tags.is_empty());
    }

    #[test]
    fn test_extract_tags_from_file_nonexistent() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("nonexistent.md");

        let tags = extract_tags_from_file(&file_path);
        assert!(tags.is_empty());
    }

    #[test]
    fn test_extract_tags_from_file_mixed_case() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "---\ntags: [TagOne, TAGTWO]\n---\nContent").unwrap();
        let tags = extract_tags_from_file(&file_path);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"tagone".to_string()));
        assert!(tags.contains(&"tagtwo".to_string()));
    }

    #[test]
    fn test_extract_tags_from_file_yaml_list() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "---\ntags:\n  - tag1\n  - tag2\n---\nContent").unwrap();
        let tags = extract_tags_from_file(&file_path);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"tag1".to_string()));
        assert!(tags.contains(&"tag2".to_string()));
    }

    #[test]
    fn test_extract_tags_from_file_empty_array() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "---\ntags: []\n---\nContent").unwrap();
        let tags = extract_tags_from_file(&file_path);
        assert!(tags.is_empty());
    }
}