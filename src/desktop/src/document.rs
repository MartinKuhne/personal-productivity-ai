//! Markdown document model — splits raw text into YAML front matter and body.

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentContent {
    pub front_matter: Option<String>,
    pub body: String,
}

impl DocumentContent {
    /// Parses a raw document string into front-matter and body.
    ///
    /// Delegates the front-matter detection to
    /// [`crate::utils::markdown::parse_front_matter`] so the editor's
    /// view of a file stays consistent with the rest of the crate
    /// (tag extraction, YAML header tools, batch prompts). Previously
    /// this function split on `---` without validating the YAML
    /// payload, which meant the editor would treat malformed YAML as
    /// valid front matter while the rest of the crate rejected it.
    pub fn parse(raw: &str) -> Self {
        let content = raw.strip_prefix('\u{feff}').unwrap_or(raw);

        if let Some((_yaml, body)) = crate::utils::markdown::parse_front_matter(content) {
            // Reconstruct the exact front-matter block so the editor
            // can round-trip it on save (including the original
            // whitespace and any comments the YAML parser ignored).
            let parts: Vec<&str> = content.splitn(3, "---").collect();
            if parts.len() == 3 && parts[0].trim().is_empty() {
                let original_fm = format!("---{}---", parts[1]);
                return Self {
                    front_matter: Some(original_fm),
                    body: body.to_string(),
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

    #[test]
    fn test_parse_agrees_with_utils_parse_front_matter() {
        // Two parsers exist for front matter in this crate:
        //   1. `crate::utils::markdown::parse_front_matter` — validates YAML,
        //      returns `None` if the YAML is malformed.
        //   2. `DocumentContent::parse` (this fn) — splits on `---` without
        //      validation; treats anything between delimiters as front matter.
        //
        // The editor's `open()` uses (2) to populate `original_front_matter`,
        // while `crate::utils::tags::extract_tags_from_file` and other tools
        // use (1) to extract structured data. If the two disagree, the editor
        // and the tag extractor see different things in the same file.
        //
        // The expected contract: BOTH parsers either accept the input as
        // front matter or reject it — and they agree on which.

        let invalid_yaml = "---\ninvalid: [unclosed\n---\nBody";

        // utils::markdown::parse_front_matter rejects this.
        let utils_result = crate::utils::markdown::parse_front_matter(invalid_yaml);
        assert!(
            utils_result.is_none(),
            "utils parser should reject malformed YAML; got {utils_result:?}"
        );

        // DocumentContent::parse should agree — also reject (front_matter: None,
        // body contains the entire input).
        let doc = DocumentContent::parse(invalid_yaml);
        assert!(
            doc.front_matter.is_none(),
            "DocumentContent::parse disagrees with utils::parse_front_matter: \
             editor sees front matter ({:?}) but tag extractor does not",
            doc.front_matter
        );
        assert_eq!(doc.body, invalid_yaml);
    }
}
