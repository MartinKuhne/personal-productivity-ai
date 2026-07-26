pub use crate::markdown::{FrontMatter, parse_front_matter};

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
