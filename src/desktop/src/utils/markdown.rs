use serde_yaml::Value;

/// Parse YAML front matter from markdown content.
///
/// Expects content to start with `---` followed by YAML and another `---`.
/// Returns the parsed YAML value and the remaining markdown content.
///
/// # Examples
///
/// ```
/// use fastmd::utils::markdown::parse_front_matter;
/// let content = "---\ntitle: Test\ntags: [tag1, tag2]\n---\n# Hello World";
/// let result = parse_front_matter(content);
/// assert!(result.is_some());
/// let (yaml, md) = result.unwrap();
/// assert_eq!(yaml["title"], "Test");
/// assert_eq!(md.trim(), "# Hello World");
/// ```
pub fn parse_front_matter(content: &str) -> Option<(Value, &str)> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() == 3 && parts[0].trim().is_empty() {
        let yaml_str = parts[1];
        let markdown_content = parts[2];
        if let Ok(value) = serde_yaml::from_str::<Value>(yaml_str) {
            return Some((value, markdown_content));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_front_matter_basic() {
        let content = "---\ntitle: Test Document\nauthor: John Doe\n---\n# Hello World";
        let result = parse_front_matter(content);
        assert!(result.is_some());
        let (yaml, md) = result.unwrap();
        assert_eq!(yaml["title"].as_str(), Some("Test Document"));
        assert_eq!(yaml["author"].as_str(), Some("John Doe"));
        assert_eq!(md.trim(), "# Hello World");
    }

    #[test]
    fn test_parse_front_matter_with_tags_array() {
        let content = "---\ntags: [tag1, tag2, tag3]\n---\nContent here";
        let result = parse_front_matter(content);
        assert!(result.is_some());
        let (yaml, _) = result.unwrap();
        let tags = yaml["tags"].as_sequence().unwrap();
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0].as_str(), Some("tag1"));
        assert_eq!(tags[1].as_str(), Some("tag2"));
        assert_eq!(tags[2].as_str(), Some("tag3"));
    }

    #[test]
    fn test_parse_front_matter_with_tags_string() {
        let content = "---\ntags: single-tag\n---\nContent";
        let result = parse_front_matter(content);
        assert!(result.is_some());
        let (yaml, _) = result.unwrap();
        assert_eq!(yaml["tags"].as_str(), Some("single-tag"));
    }

    #[test]
    fn test_parse_front_matter_missing_delimiters() {
        let content = "No front matter here";
        let result = parse_front_matter(content);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_front_matter_invalid_yaml() {
        let content = "---\ninvalid: [unclosed\n---\nContent";
        let result = parse_front_matter(content);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_front_matter_empty() {
        let content = "---\n---\nContent";
        let result = parse_front_matter(content);
        assert!(result.is_some());
        let (yaml, md) = result.unwrap();
        assert!(yaml.is_null());
        assert_eq!(md.trim(), "Content");
    }

    #[test]
    fn test_parse_front_matter_nested_objects() {
        let content = "---\nconfig:\n  key: value\n  nested:\n    deep: 123\n---\nBody";
        let result = parse_front_matter(content);
        assert!(result.is_some());
        let (yaml, _) = result.unwrap();
        assert_eq!(yaml["config"]["key"].as_str(), Some("value"));
        assert_eq!(yaml["config"]["nested"]["deep"].as_i64(), Some(123));
    }

    #[test]
    fn test_parse_front_matter_inside_body() {
        let content = "---\ntitle: test\n---\nBody with\n---\ninside";
        let result = parse_front_matter(content);
        assert!(result.is_some());
        let (yaml, md) = result.unwrap();
        assert_eq!(yaml["title"].as_str(), Some("test"));
        assert_eq!(md.trim(), "Body with\n---\ninside");
    }

    #[test]
    fn test_parse_front_matter_leading_whitespace() {
        let content = " ---\ntitle: test\n---\nBody";
        let result = parse_front_matter(content);
        assert!(result.is_some());
        let (yaml, md) = result.unwrap();
        assert_eq!(yaml["title"].as_str(), Some("test"));
        assert_eq!(md.trim(), "Body");
    }

    #[test]
    fn test_parse_front_matter_special_chars() {
        let content = "---\ntitle: \"Tést 🚀\"\n---\nBody";
        let result = parse_front_matter(content);
        assert!(result.is_some());
        let (yaml, md) = result.unwrap();
        assert_eq!(yaml["title"].as_str(), Some("Tést 🚀"));
        assert_eq!(md.trim(), "Body");
    }

    #[test]
    fn test_parse_front_matter_no_closing_delimiter() {
        let content = "---\ntitle: test\nBody";
        let result = parse_front_matter(content);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_front_matter_yaml_list() {
        let content = "---\ntags:\n  - tag1\n  - tag2\n---\nContent";
        let result = parse_front_matter(content);
        assert!(result.is_some());
        let (yaml, _) = result.unwrap();
        let tags = yaml["tags"].as_sequence().unwrap();
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn test_parse_front_matter_empty_array() {
        let content = "---\ntags: []\n---\nContent";
        let result = parse_front_matter(content);
        assert!(result.is_some());
        let (yaml, _) = result.unwrap();
        let tags = yaml["tags"].as_sequence().unwrap();
        assert_eq!(tags.len(), 0);
    }
}