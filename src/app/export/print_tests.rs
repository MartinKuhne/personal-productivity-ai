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

/// A tracked file that was already removed (browser kept it, the OS cleaned the
/// temp dir, …) must not abort the sweep: the entry still leaves the list.
#[test]
fn test_cleanup_temp_files_tolerates_an_already_deleted_file() {
    let dir = tempdir().unwrap();
    let gone = dir.path().join("already_gone.html");
    std::fs::write(&gone, "<html></html>").unwrap();
    std::fs::remove_file(&gone).unwrap();

    {
        let mut files = temp_print_files().lock().unwrap();
        files.push(gone.clone());
    }

    cleanup_temp_files();

    assert!(
        !temp_print_files().lock().unwrap().contains(&gone),
        "the entry must be drained even when the file is already gone, otherwise every later sweep retries it"
    );
}

/// `PrintJob` is cloned into the background task and printed in diagnostics, so
/// the derived `Clone`/`Debug` must carry every field.
#[test]
fn test_print_job_clone_and_debug_carry_every_field() {
    let job = PrintJob::from_content("# Body".to_string(), "Quarterly Report".to_string());
    let clone = job.clone();

    assert_eq!(
        clone.markdown_path, job.markdown_path,
        "a cloned job must point at the same source"
    );
    assert_eq!(
        clone.markdown_content, job.markdown_content,
        "a cloned job must carry the full markdown"
    );
    assert_eq!(clone.title, job.title, "a cloned job must keep the title");

    let rendered = format!("{job:?}");
    assert!(
        rendered.contains("PrintJob") && rendered.contains("Quarterly Report"),
        "Debug must name the job for diagnostics, got: {rendered}"
    );
}

/// A job built from in-memory content has no file on disk, so the title doubles
/// as the placeholder path — pinned because the print pipeline never reads it.
#[test]
fn test_print_job_from_content_uses_the_title_as_a_placeholder_path() {
    let job = PrintJob::from_content("body".to_string(), "Weekly Note".to_string());

    assert_eq!(
        job.markdown_path,
        PathBuf::from("Weekly Note"),
        "drift: `markdown_path` is synthesised from the title, so it is not a real file path"
    );
    assert!(
        !job.markdown_path.exists(),
        "a content job must not claim to reference an existing file"
    );
}

/// Empty input must still yield a complete, self-contained printable page.
#[test]
fn test_empty_markdown_still_produces_a_complete_printable_document() {
    let body = markdown_to_html("");
    assert!(
        !body.contains("<p>"),
        "empty input must not invent a paragraph, got: {body}"
    );

    let doc = build_html_document("Empty", &body);
    assert!(
        doc.starts_with("<!DOCTYPE html>") && doc.contains("</html>"),
        "the wrapper must always emit a full HTML document"
    );
    assert!(
        doc.contains("@media print"),
        "the print stylesheet must be present even for an empty document"
    );
}

/// The print pipeline renders with the same GFM feature set as the editor, so
/// tables and fenced code must survive into the HTML handed to the browser.
#[test]
fn test_markdown_to_html_renders_tables_and_fenced_code_for_printing() {
    let md = "| Name | Qty |\n|------|-----|\n| Rope | 1   |\n\n```rust\nlet x = 1;\n```\n";

    let html = markdown_to_html(md);

    assert!(
        html.contains("<table>") && html.contains("<th>Name</th>"),
        "GFM tables must render as a real table for printing, got: {html}"
    );
    assert!(
        html.contains("<td>Rope</td>"),
        "table cells must be present, got: {html}"
    );
    assert!(
        html.contains("<pre><code") && html.contains("let x = 1;"),
        "fenced code must be preserved verbatim inside a pre block, got: {html}"
    );
}

/// The rendered body must be embedded verbatim — the wrapper may not escape or
/// drop it, otherwise the printed page loses content.
#[test]
fn test_build_html_document_embeds_the_rendered_body_verbatim() {
    let body = "<p>Alpha &amp; Beta</p>";

    let doc = build_html_document("Ignored", body);

    assert!(
        doc.contains(body),
        "the body must appear unchanged inside <body>, got: {doc}"
    );
    let normalised = doc.replace("\r\n", "\n");
    assert!(
        normalised.contains("<body>\n    <p>Alpha &amp; Beta</p>\n</body>"),
        "the body must sit directly inside the <body> element"
    );
}
