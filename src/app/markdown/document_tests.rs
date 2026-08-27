use super::*;

#[test]
fn test_document_caching_and_revision() {
    let mut doc = Document::new("- [ ] Task 1".to_string());
    assert_eq!(doc.revision(), 1);
    assert_eq!(doc.events().len(), 1);

    // Update with identical source — revision stays unchanged
    doc.update_source("- [ ] Task 1".to_string());
    assert_eq!(doc.revision(), 1);

    // Toggle task — mutates source and increments revision
    doc.toggle_task(0, true);
    assert_eq!(doc.revision(), 2);
    assert_eq!(doc.source(), "- [x] Task 1");
}

#[test]
fn test_document_body_is_source_without_front_matter() {
    let doc = Document::new("# Hello".to_string());
    assert!(doc.front_matter().is_none());
    assert_eq!(doc.body(), "# Hello");
    assert_eq!(doc.source(), "# Hello");
    assert_eq!(doc.yaml(), None);
}

#[test]
fn test_document_body_is_post_delimiter_with_front_matter() {
    let doc = Document::new("---\ntitle: T\n---\n# Body".to_string());
    let fm = doc.front_matter().expect("front matter should parse");
    assert_eq!(fm.yaml["title"].as_str(), Some("T"));
    assert_eq!(doc.body(), "\n# Body");
    // Source preserves the delimiters verbatim.
    assert_eq!(doc.source(), "---\ntitle: T\n---\n# Body");
}

#[test]
fn test_document_invalid_yaml_front_matter_is_treated_as_no_front_matter() {
    // Malformed YAML is not a valid front-matter block; the
    // whole source becomes the body.
    let doc = Document::new("---\ninvalid: [unclosed\n---\nBody".to_string());
    assert!(doc.front_matter().is_none());
    assert_eq!(doc.body(), "---\ninvalid: [unclosed\n---\nBody");
}

#[test]
fn test_document_update_source_reparses_front_matter() {
    let mut doc = Document::new("body".to_string());
    assert!(doc.front_matter().is_none());
    doc.update_source("---\ntitle: T2\n---\nbody2".to_string());
    assert!(doc.front_matter().is_some());
    assert_eq!(
        doc.front_matter().unwrap().yaml["title"].as_str(),
        Some("T2")
    );
    assert_eq!(doc.body(), "\nbody2");
    assert_eq!(doc.revision(), 2);
}

#[test]
fn test_document_toggle_task_in_body_with_front_matter() {
    // When the front matter is present, the task list lives in
    // the body, not the source. `toggle_task` must still flip
    // the marker and re-parse the events.
    let mut doc = Document::new("---\ntitle: T\n---\n- [ ] todo".to_string());
    assert_eq!(doc.body(), "\n- [ ] todo");
    doc.toggle_task(0, true);
    // Body now contains the toggled marker.
    assert_eq!(doc.body(), "\n- [x] todo");
    // Source is the verbatim text with delimiters.
    assert_eq!(doc.source(), "---\ntitle: T\n---\n- [x] todo");
    assert_eq!(doc.revision(), 2);
}

#[test]
fn test_parse_front_matter_basic() {
    let content = "---\ntitle: Test Document\nauthor: John Doe\n---\n# Hello World";
    let fm = parse_front_matter(content).unwrap();
    assert_eq!(fm.yaml["title"].as_str(), Some("Test Document"));
    assert_eq!(fm.yaml["author"].as_str(), Some("John Doe"));
    assert_eq!(fm.body.trim(), "# Hello World");
}

#[test]
fn test_document_content_parse_with_front_matter() {
    let raw = "---\ntitle: Test\n---\nBody text";
    let doc = DocumentContent::parse(raw);
    assert_eq!(doc.front_matter, Some("---\ntitle: Test\n---".to_string()));
    assert_eq!(doc.body, "\nBody text");
}

#[test]
fn test_document_content_parse_without_front_matter() {
    let raw = "Just body\ncontent";
    let doc = DocumentContent::parse(raw);
    assert!(doc.front_matter.is_none());
    assert_eq!(doc.body, "Just body\ncontent");
}

#[test]
fn test_document_content_to_string_with_front_matter() {
    let doc = DocumentContent {
        front_matter: Some("---\ntitle: Test\n---".to_string()),
        body: "\nBody".to_string(),
    };
    let result = doc.to_string();
    assert_eq!(result, "---\ntitle: Test\n---\nBody");
}

#[test]
fn test_document_content_to_string_without_front_matter() {
    let doc = DocumentContent {
        front_matter: None,
        body: "Just body".to_string(),
    };
    assert_eq!(doc.to_string(), "Just body");
}

#[test]
fn test_document_content_parse_strips_leading_bom() {
    // A UTF-8 BOM before the front matter must be stripped so the
    // block still parses (DocumentContent::parse, line 45).
    let raw = "\u{feff}---\ntitle: Test\n---\nBody text";
    let doc = DocumentContent::parse(raw);
    assert_eq!(doc.front_matter, Some("---\ntitle: Test\n---".to_string()));
    assert_eq!(doc.body, "\nBody text");
}

#[test]
fn test_parse_front_matter_strips_leading_bom() {
    // The BOM is also stripped inside parse_front_matter itself.
    let content = "\u{feff}---\ntitle: T\n---\nBody";
    let fm = parse_front_matter(content).expect("BOM front matter should parse");
    assert_eq!(fm.yaml["title"].as_str(), Some("T"));
}

#[test]
fn test_document_toggle_task_out_of_range_still_bumps_revision() {
    // `apply_task_toggle` silently no-ops when `task_index` matches
    // nothing, but `toggle_task` still re-parses and bumps the
    // revision. Pin that contract so a future "don't bump on no-op"
    // change is deliberate.
    let mut doc = Document::new("- [ ] Task 1".to_string());
    let before = doc.revision();
    doc.toggle_task(99, true);
    assert_eq!(doc.source(), "- [ ] Task 1");
    assert_eq!(doc.revision(), before + 1);
}

#[test]
fn test_document_toggle_task_on_document_without_tasks_still_bumps_revision() {
    // A body with zero task markers is a no-op for `apply_task_toggle`
    // but still bumps the revision.
    let mut doc = Document::new("# Just a heading".to_string());
    let before = doc.revision();
    doc.toggle_task(0, true);
    assert_eq!(doc.source(), "# Just a heading");
    assert_eq!(doc.revision(), before + 1);
}

#[test]
fn test_document_toggle_task_toggles_second_marker() {
    // Toggling the second task must leave the first untouched.
    let mut doc = Document::new("- [ ] A\n- [x] B\n- [ ] C".to_string());
    doc.toggle_task(2, true);
    assert_eq!(doc.source(), "- [ ] A\n- [x] B\n- [x] C");
}
