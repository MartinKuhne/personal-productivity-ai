//! Parses YAML front matter from a markdown string, returning the parsed value, the original source, and the remaining body.

use serde_yaml::Value;

/// The result of successfully parsing a YAML front matter block.
///
/// All three fields are exposed so callers that need to round-trip the
/// original source text (e.g. `DocumentContent::parse` preserving the
/// front matter verbatim for save) can do so without re-parsing or
/// re-splitting the markdown.
#[derive(Debug)]
pub struct FrontMatter {
    /// The parsed YAML value.
    pub yaml: Value,
    /// The original YAML source text between the `---` delimiters,
    /// preserved verbatim (no whitespace trimming). Suitable for
    /// round-trip serialization back to disk.
    pub source: String,
    /// The body content: everything after the closing `---` delimiter.
    pub body: String,
}

/// Parse YAML front matter from markdown content.
///
/// Expects content to start with `---` followed by YAML and another `---`.
/// Returns the parsed YAML value, the original source text, and the
/// remaining body. Returns `None` if the delimiters are missing or
/// unclosed, or if the YAML payload fails to parse.
///
/// This is the single source of truth for front-matter detection in the
/// crate. `DocumentContent::parse` and the various tools (`tags`,
/// `yaml_header`, `filesystem`, `batch::prompts`, `ui::app`) all
/// delegate to this function so the editor and the structured-data
/// consumers never disagree on what counts as "front matter".
///
/// # Examples
///
/// ```
/// use fastmd::utils::markdown::parse_front_matter;
/// let content = "---\ntitle: Test\ntags: [tag1, tag2]\n---\n# Hello World";
/// let fm = parse_front_matter(content).unwrap();
/// assert_eq!(fm.yaml["title"], "Test");
/// assert_eq!(fm.body.trim(), "# Hello World");
/// ```
pub fn parse_front_matter(content: &str) -> Option<FrontMatter> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() == 3 && parts[0].trim().is_empty() {
        let yaml_source = parts[1];
        let body = parts[2];
        if let Ok(yaml) = serde_yaml::from_str::<Value>(yaml_source) {
            return Some(FrontMatter {
                yaml,
                source: yaml_source.to_string(),
                body: body.to_string(),
            });
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
        let fm = parse_front_matter(content).unwrap();
        assert_eq!(fm.yaml["title"].as_str(), Some("Test Document"));
        assert_eq!(fm.yaml["author"].as_str(), Some("John Doe"));
        assert_eq!(fm.body.trim(), "# Hello World");
    }

    #[test]
    fn test_parse_front_matter_with_tags_array() {
        let content = "---\ntags: [tag1, tag2, tag3]\n---\nContent here";
        let fm = parse_front_matter(content).unwrap();
        let tags = fm.yaml["tags"].as_sequence().unwrap();
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0].as_str(), Some("tag1"));
        assert_eq!(tags[1].as_str(), Some("tag2"));
        assert_eq!(tags[2].as_str(), Some("tag3"));
    }

    #[test]
    fn test_parse_front_matter_with_tags_string() {
        let content = "---\ntags: single-tag\n---\nContent";
        let fm = parse_front_matter(content).unwrap();
        assert_eq!(fm.yaml["tags"].as_str(), Some("single-tag"));
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
        let fm = parse_front_matter(content).unwrap();
        assert!(fm.yaml.is_null());
        assert_eq!(fm.body.trim(), "Content");
        assert_eq!(fm.source, "\n");
    }

    #[test]
    fn test_parse_front_matter_nested_objects() {
        let content = "---\nconfig:\n  key: value\n  nested:\n    deep: 123\n---\nBody";
        let fm = parse_front_matter(content).unwrap();
        assert_eq!(fm.yaml["config"]["key"].as_str(), Some("value"));
        assert_eq!(fm.yaml["config"]["nested"]["deep"].as_i64(), Some(123));
    }

    #[test]
    fn test_parse_front_matter_inside_body() {
        let content = "---\ntitle: test\n---\nBody with\n---\ninside";
        let fm = parse_front_matter(content).unwrap();
        assert_eq!(fm.yaml["title"].as_str(), Some("test"));
        assert_eq!(fm.body.trim(), "Body with\n---\ninside");
    }

    #[test]
    fn test_parse_front_matter_leading_whitespace() {
        let content = " ---\ntitle: test\n---\nBody";
        let fm = parse_front_matter(content).unwrap();
        assert_eq!(fm.yaml["title"].as_str(), Some("test"));
        assert_eq!(fm.body.trim(), "Body");
    }

    #[test]
    fn test_parse_front_matter_special_chars() {
        let content = "---\ntitle: \"Tést 🚀\"\n---\nBody";
        let fm = parse_front_matter(content).unwrap();
        assert_eq!(fm.yaml["title"].as_str(), Some("Tést 🚀"));
        assert_eq!(fm.body.trim(), "Body");
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
        let fm = parse_front_matter(content).unwrap();
        let tags = fm.yaml["tags"].as_sequence().unwrap();
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn test_parse_front_matter_empty_array() {
        let content = "---\ntags: []\n---\nContent";
        let fm = parse_front_matter(content).unwrap();
        let tags = fm.yaml["tags"].as_sequence().unwrap();
        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn test_source_preserves_verbatim_yaml() {
        let content = "---\n  title:   Test  \n  tags: [a, b]\n---\nbody";
        let fm = parse_front_matter(content).unwrap();
        assert_eq!(fm.source, "\n  title:   Test  \n  tags: [a, b]\n");
    }
}
