use fastmd::document::DocumentContent;

#[test]
fn test_parse_with_front_matter() {
    let raw = "---\ntitle: test\n---\n# Header\nBody text";
    let doc = DocumentContent::parse(raw);
    
    assert_eq!(doc.front_matter.as_deref(), Some("---\ntitle: test\n---"));
    assert_eq!(doc.body, "\n# Header\nBody text");
}

#[test]
fn test_parse_without_front_matter() {
    let raw = "# Header\nBody text";
    let doc = DocumentContent::parse(raw);
    
    assert_eq!(doc.front_matter, None);
    assert_eq!(doc.body, "# Header\nBody text");
}

#[test]
fn test_to_string_with_front_matter() {
    let doc = DocumentContent {
        front_matter: Some("---\ntitle: test\n---".to_string()),
        body: "\n# Header\nBody text".to_string(),
    };
    
    assert_eq!(doc.to_string(), "---\ntitle: test\n---\n# Header\nBody text");
}

#[test]
fn test_to_string_without_front_matter() {
    let doc = DocumentContent {
        front_matter: None,
        body: "# Header\nBody text".to_string(),
    };
    
    assert_eq!(doc.to_string(), "# Header\nBody text");
}
