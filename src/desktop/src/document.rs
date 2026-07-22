//! Markdown document model — splits raw text into YAML front matter and body.

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentContent {
    pub front_matter: Option<String>,
    pub body: String,
}

impl DocumentContent {
    /// Parses a raw document string into front-matter and body.
    pub fn parse(raw: &str) -> Self {
        let content = raw.strip_prefix('\u{feff}').unwrap_or(raw);

        // Match behavior similar to parse_front_matter where it must start with ---
        if content.starts_with("---") || content.trim_start().starts_with("---") {
            let parts: Vec<&str> = content.splitn(3, "---").collect();
            if parts.len() == 3 && parts[0].trim().is_empty() {
                // Return the exact front matter block, including delimiters
                let original_fm = format!("---{}---", parts[1]);
                let body = parts[2].to_string();

                return Self {
                    front_matter: Some(original_fm),
                    body,
                };
            }
        }

        Self {
            front_matter: None,
            body: raw.to_string(),
        }
    }

    /// Combines the front-matter and body into a single string to save.
    pub fn to_string(&self) -> String {
        if let Some(fm) = &self.front_matter {
            format!("{}{}", fm, self.body)
        } else {
            self.body.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_with_front_matter() {
        let raw = "---\ntitle: Test\n---\nBody text";
        let doc = DocumentContent::parse(raw);
        assert_eq!(doc.front_matter, Some("---\ntitle: Test\n---".to_string()));
        assert_eq!(doc.body, "\nBody text");
    }

    #[test]
    fn test_parse_without_front_matter() {
        let raw = "Just body\ncontent";
        let doc = DocumentContent::parse(raw);
        assert!(doc.front_matter.is_none());
        assert_eq!(doc.body, "Just body\ncontent");
    }

    #[test]
    fn test_parse_bom_stripped() {
        let raw = "\u{feff}---\ntitle: Test\n---\nBody";
        let doc = DocumentContent::parse(raw);
        assert!(doc.front_matter.is_some());
        assert_eq!(doc.body, "\nBody");
    }

    #[test]
    fn test_to_string_with_front_matter() {
        let doc = DocumentContent {
            front_matter: Some("---\ntitle: Test\n---".to_string()),
            body: "\nBody".to_string(),
        };
        let result = doc.to_string();
        assert_eq!(result, "---\ntitle: Test\n---\nBody");
    }

    #[test]
    fn test_to_string_without_front_matter() {
        let doc = DocumentContent {
            front_matter: None,
            body: "Just body".to_string(),
        };
        assert_eq!(doc.to_string(), "Just body");
    }

    #[test]
    fn test_parse_incomplete_front_matter() {
        // Only one --- delimiter, no front matter
        let raw = "---\nincomplete";
        let doc = DocumentContent::parse(raw);
        assert!(doc.front_matter.is_none());
        assert_eq!(doc.body, "---\nincomplete");
    }

    #[test]
    fn test_parse_front_matter_with_body_containing_dashes() {
        let raw = "---\ntitle: Test\n---\nBody with --- inside";
        let doc = DocumentContent::parse(raw);
        assert_eq!(doc.front_matter, Some("---\ntitle: Test\n---".to_string()));
        assert_eq!(doc.body, "\nBody with --- inside");
    }

    #[test]
    fn test_to_string_empty_body() {
        let doc = DocumentContent {
            front_matter: Some("---\ntitle: Test\n---".to_string()),
            body: String::new(),
        };
        assert_eq!(doc.to_string(), "---\ntitle: Test\n---");
    }
}
