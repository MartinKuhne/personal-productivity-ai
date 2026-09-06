use super::*;
use tempfile::tempdir;

#[test]
fn test_print_job_new() {
    let dir = tempdir().unwrap();
    let md_path = dir.path().join("test.md");
    std::fs::write(&md_path, "# Test\n\nContent").unwrap();

    let job = PrintJob::new(md_path.clone());
    assert_eq!(job.markdown_path, md_path);
    assert_eq!(job.title, "test");
    assert_eq!(job.markdown_content, "# Test\n\nContent");
}

#[test]
fn test_print_job_new_missing_file_falls_back_to_empty() {
    let missing = PathBuf::from("definitely_missing_file_12345.md");
    let job = PrintJob::new(missing.clone());
    // Missing file -> empty content, but the stem still provides a title.
    assert_eq!(job.markdown_content, "");
    assert_eq!(job.title, "definitely_missing_file_12345");
}

#[test]
fn test_print_job_new_without_stem_uses_document_title() {
    // A path with no file name/stem falls back to the default title.
    let job = PrintJob::new(PathBuf::from(""));
    assert_eq!(job.title, "Document");
    assert_eq!(job.markdown_content, "");
}

#[test]
fn test_print_job_from_content() {
    let job = PrintJob::from_content("# Test\n\nContent".to_string(), "My Doc".to_string());
    assert_eq!(job.title, "My Doc");
    assert_eq!(job.markdown_content, "# Test\n\nContent");
}

#[test]
fn test_markdown_to_html() {
    let html = markdown_to_html("# Hello\n\n**Bold** text");
    assert!(html.contains("<h1>Hello</h1>"));
    assert!(html.contains("<strong>Bold</strong>"));
}

#[test]
fn test_build_html_document() {
    let html = markdown_to_html("# Hi");
    let doc = build_html_document("Test", &html);
    assert!(doc.contains("<!DOCTYPE html>"));
}

#[test]
fn test_markdown_to_html_strips_yaml_header() {
    let md = "---\ntitle: Packing List\nauthor: Alice\n---\n# My Heading\n\nBody content.";
    let html = markdown_to_html(md);
    assert!(
        !html.contains("Packing List"),
        "YAML header title should not appear in HTML print output"
    );
    assert!(
        !html.contains("author: Alice"),
        "YAML header author should not appear in HTML print output"
    );
    assert!(html.contains("<h1>My Heading</h1>"));
    assert!(html.contains("<p>Body content.</p>"));
}

#[test]
fn test_build_html_document_removes_file_name_from_header() {
    let html = "<p>Content</p>";
    let doc = build_html_document("MySecretFileName", html);
    assert!(
        !doc.contains("MySecretFileName"),
        "File name must not appear in the printed header"
    );
    assert!(
        doc.contains("<title></title>"),
        "Title tag must be empty to avoid browser header print"
    );
    assert!(
        doc.contains("@page"),
        "Must include @page CSS rule to suppress browser print headers"
    );
}

#[test]
fn test_markdown_to_html_preserves_internal_rules() {
    let md = "# Top\n\n---\n\nBottom content after divider";
    let html = markdown_to_html(md);
    assert!(html.contains("<hr />") || html.contains("<hr>"));
    assert!(html.contains("Bottom content after divider"));
}

#[test]
fn test_cleanup_temp_files_clears_tracked_files() {
    let dir = tempdir().unwrap();
    let temp_file = dir.path().join("dummy_print.html");
    std::fs::write(&temp_file, "<html></html>").unwrap();
    assert!(temp_file.exists());

    if let Ok(mut files) = temp_print_files().lock() {
        files.push(temp_file.clone());
    }

    cleanup_temp_files();
    assert!(
        !temp_file.exists(),
        "cleanup_temp_files should delete tracked files"
    );
}
