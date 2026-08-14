//! Tests for `tools/filesystem.rs`.

use super::*;

fn test_ctx() -> crate::tools::context::ToolContext {
    let config = crate::config::AgentConfig::default();
    let mut builder = crate::tools::context::ToolContextBuilder::new(
        std::sync::Arc::new(config.clone()),
        std::sync::Arc::new(crate::tools::observer::DefaultFileObserver),
    );
    builder = builder.with_extension(std::sync::Arc::new(
        crate::tools::vfs::VirtualFileSystemExt(std::sync::Arc::new(
            crate::tools::vfs::VfsResolver::new(std::sync::Arc::new(config.clone())),
        )),
    ));
    builder.build()
}

/// A producer that publishes to a throwaway bus. Tests don't
/// need to consume the events — they only care about the
/// success/failure of the underlying file operation.
fn noop_producer() -> std::sync::Arc<dyn crate::tools::observer::OnFileChanged> {
    std::sync::Arc::new(crate::tools::observer::DefaultFileObserver)
}

#[test]
fn test_tool_patch_note() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "Line 1\nOld Text\nLine 3").unwrap();

    let producer = noop_producer();
    let result = tool_patch_note(
        &test_ctx(),
        file_path.to_str().unwrap(),
        "Old Text",
        "New Text",
        &*producer,
    )
    .unwrap()
    .result;
    assert_eq!(result, "Successfully replaced 1 occurrence(s).");

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "Line 1\nNew Text\nLine 3");
}

#[test]
fn test_tool_patch_note_not_found() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "Line 1\nOld Text\nLine 3").unwrap();

    let producer = noop_producer();
    let result = tool_patch_note(
        &test_ctx(),
        file_path.to_str().unwrap(),
        "Missing Text",
        "New Text",
        &*producer,
    );
    assert_eq!(
        result.unwrap_err(),
        "The specified old_string was not found in the file body."
    );
}

use std::fs;
use tempfile::tempdir;

#[test]
fn test_tool_search_notes() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "# Hello\nWorld content\nAnother line").unwrap();

    let result = tool_search_notes(&test_ctx(), dir.path(), "Workspace", "World").unwrap();
    assert!(result.iter().any(|m| m.contains("World content")));
    assert!(result.iter().any(|m| m.contains("Workspace")));
    assert!(result.iter().any(|m| m.contains("test.md")));
}

#[test]
fn test_tool_list_notes() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "content").unwrap();
    fs::write(dir.path().join("b.txt"), "content").unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub").join("c.md"), "content").unwrap();

    // The low-level tool now returns a `Vec<String>` of every
    // match (no paging, no newline joining). Paging is applied at
    // the registry call site.
    let result = tool_list_notes(&test_ctx(), dir.path(), "Workspace").unwrap();
    assert_eq!(result.len(), 1, "non-recursive scan must return just a.md");
    assert!(result[0].ends_with("a.md"));
    assert!(result[0].starts_with("Workspace"));
}

#[test]
fn test_tool_read_note() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Hello World").unwrap();

    let result = tool_read_note(&test_ctx(), file_path.to_str().unwrap())
        .unwrap()
        .content;
    assert_eq!(result, "Hello World");
}

#[test]
fn test_tool_read_note_not_found() {
    let result = tool_read_note(&test_ctx(), "/nonexistent/path.md");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Failed to read file"));
}

#[test]
fn test_tool_window_note() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Line 1\nLine 2\nLine 3\nLine 4").unwrap();

    // 0-indexed: skip "Line 1", return the next 2 lines.
    let result = tool_window_note(&test_ctx(), file_path.to_str().unwrap(), 1, 2)
        .unwrap()
        .content;
    assert_eq!(result, "Line 2\nLine 3");
}

#[test]
fn test_tool_window_note_offset_zero() {
    // BOUNDARY: offset=0 is the first line (was an error pre-migration).
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Line 1\nLine 2").unwrap();

    let result = tool_window_note(&test_ctx(), file_path.to_str().unwrap(), 0, 1)
        .unwrap()
        .content;
    assert_eq!(result, "Line 1");
}

#[test]
fn test_tool_window_note_empty_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("empty.md");
    fs::write(&file_path, "").unwrap();

    let result = tool_window_note(&test_ctx(), file_path.to_str().unwrap(), 0, 50)
        .unwrap()
        .content;
    assert_eq!(result, "");
}

#[test]
fn test_tool_create_note() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("new.md");

    let producer = noop_producer();
    let result = tool_create_note(
        &test_ctx(),
        file_path.to_str().unwrap(),
        "---\ntitle: Test\n---\n# Hello",
        &*producer,
    )
    .unwrap()
    .result;
    assert_eq!(result, "File created successfully.");
    assert!(file_path.exists());

    let content = fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("title: Test"));
    assert!(content.contains("# Hello"));
}

#[test]
fn test_tool_create_note_invalid_extension() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("new.txt");

    let producer = noop_producer();
    let result = tool_create_note(
        &test_ctx(),
        file_path.to_str().unwrap(),
        "content",
        &*producer,
    );
    assert_eq!(
        result.unwrap_err(),
        "Only markdown files (.md) are allowed."
    );
}

#[test]
fn test_tool_create_note_invalid_yaml() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("new.md");

    let producer = noop_producer();
    let result = tool_create_note(
        &test_ctx(),
        file_path.to_str().unwrap(),
        "---\ninvalid: [unclosed\n---\nContent",
        &*producer,
    );
    assert_eq!(
        result.unwrap_err(),
        "Invalid YAML front-matter in markdown."
    );
}

#[test]
fn test_tool_create_note_fails_if_exists() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("existing.md");
    fs::write(&file_path, "existing content").unwrap();

    let producer = noop_producer();
    let result = tool_create_note(
        &test_ctx(),
        file_path.to_str().unwrap(),
        "---\ntitle: Test\n---\n# Hello",
        &*producer,
    );
    assert_eq!(
        result.unwrap_err(),
        "File already exists. This tool can only create new files."
    );
    // Original content should be unchanged
    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "existing content");
}

#[test]
fn test_tool_insert_into_note() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Line 1\nLine 2\nLine 3").unwrap();

    let producer = noop_producer();
    // 0-indexed: insert at position 1 (between "Line 1" and "Line 2").
    let result = tool_insert_into_note(
        &test_ctx(),
        file_path.to_str().unwrap(),
        1,
        &["New Line".to_string()],
        &*producer,
    )
    .unwrap()
    .result;
    assert_eq!(result, "Lines inserted successfully.");

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "Line 1\nNew Line\nLine 2\nLine 3");
}

#[test]
fn test_tool_insert_into_note_at_top() {
    // BOUNDARY: offset=0 inserts at the top of the file.
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Line 1\nLine 2").unwrap();

    let producer = noop_producer();
    let result = tool_insert_into_note(
        &test_ctx(),
        file_path.to_str().unwrap(),
        0,
        &["New".to_string()],
        &*producer,
    );
    assert!(result.is_ok());

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "New\nLine 1\nLine 2");
}

#[test]
fn test_tool_insert_into_note_at_end() {
    // BOUNDARY: offset == lines.len() appends to the end.
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Line 1\nLine 2").unwrap();

    let producer = noop_producer();
    let result = tool_insert_into_note(
        &test_ctx(),
        file_path.to_str().unwrap(),
        2,
        &["New".to_string()],
        &*producer,
    );
    assert!(result.is_ok());

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "Line 1\nLine 2\nNew");
}

#[test]
fn test_tool_insert_into_note_out_of_range() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Line 1\nLine 2").unwrap();

    let producer = noop_producer();
    // 2-line file accepts offset in [0, 2]; offset 5 is out of range.
    let result = tool_insert_into_note(
        &test_ctx(),
        file_path.to_str().unwrap(),
        5,
        &["New".to_string()],
        &*producer,
    );
    assert_eq!(result.unwrap_err(), "Offset out of range.");
}

#[test]
fn test_tool_read_tags() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "---\ntags: [tag1, tag2]\n---\n# Hello").unwrap();

    let result = tool_read_tags(&test_ctx(), dir.path()).unwrap().tags;
    assert_eq!(result, vec!["tag1", "tag2"]);
}

// -- list_files_by_tag (paging support) -------------------------------

/// Helper: build a temp library with `n` Markdown files whose
/// front-matter all carry the given tag.
fn build_tagged_library(n: usize, tag: &str) -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    for i in 0..n {
        // Zero-pad so the lexicographic order matches numeric
        // order — paging tests need a stable, predictable order.
        let name = format!("file_{:03}.md", i);
        let body = format!("---\ntags: [{}]\n---\n# Doc {}\n", tag, i);
        fs::write(dir.path().join(name), body).unwrap();
    }
    dir
}

#[test]
fn test_list_files_by_tag_returns_all_sorted_when_no_paging_in_tool() {
    // The low-level tool returns every match (sorted) without
    // slicing — paging lives at the call site so it can be
    // applied to the cross-library result.
    let dir = build_tagged_library(5, "meeting");
    let res = tool_list_notes_by_tag(&test_ctx(), dir.path(), "Workspace", "meeting").unwrap();
    assert_eq!(res.len(), 5);
    // Use ends_with because Path::join uses the platform
    // separator (backslash on Windows, forward slash elsewhere).
    assert!(res[0].ends_with("file_000.md"));
    assert!(res[0].starts_with("Workspace"));
    assert!(res[4].ends_with("file_004.md"));
}

#[test]
fn test_list_files_by_tag_no_matches_returns_empty() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("solo.md"), "---\ntags: [other]\n---\n# x\n").unwrap();
    let res = tool_list_notes_by_tag(&test_ctx(), dir.path(), "Workspace", "meeting").unwrap();
    assert!(res.is_empty());
}

#[test]
fn test_list_files_by_tag_ignores_non_markdown_files() {
    // A .txt with the same tag in its body must not be matched —
    // only .md / .markdown files are scanned.
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("note.md"),
        "---\ntags: [meeting]\n---\n# md\n",
    )
    .unwrap();
    fs::write(dir.path().join("note.txt"), "tags: [meeting]").unwrap();
    let res = tool_list_notes_by_tag(&test_ctx(), dir.path(), "Workspace", "meeting").unwrap();
    assert_eq!(res.len(), 1);
    assert!(res[0].ends_with("note.md"));
    assert!(res[0].starts_with("Workspace"));
}

// =====================================================================
// Additional edge case tests
// =====================================================================

#[test]
fn test_tool_search_notes_ignores_non_markdown_files() {
    // NEGATIVE ASSERTION: Files without a `.md` extension must NOT
    // be searched, even if they contain matching text. This includes
    // `.markdown` files, which the grep tool does not match.
    let dir = tempdir().unwrap();
    let md_file = dir.path().join("test.md");
    let md2_file = dir.path().join("doc.markdown");
    let txt_file = dir.path().join("secret.txt");
    let pdf_file = dir.path().join("notes.pdf");

    fs::write(&md_file, "# Project\nContains search term here").unwrap();
    fs::write(&md2_file, "Also contains search term").unwrap();
    fs::write(&txt_file, "This also contains search term").unwrap();
    fs::write(&pdf_file, "Search term in PDF").unwrap();

    let result = tool_search_notes(&test_ctx(), dir.path(), "Workspace", "search term").unwrap();
    // Only the .md file should be found
    assert!(result.iter().any(|m| m.contains("test.md")));
    assert!(result.iter().any(|m| m.contains("Contains search term")));
    // txt, pdf, and .markdown must NOT appear in results
    assert!(!result.iter().any(|m| m.contains("secret.txt")));
    assert!(!result.iter().any(|m| m.contains("notes.pdf")));
    assert!(!result.iter().any(|m| m.contains("doc.markdown")));
}

#[test]
fn test_tool_search_notes_multiple_matches_same_file() {
    // ORDERING ASSERTION: When a query matches multiple lines in the
    // same file, they should appear in line number order.
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(
        &file_path,
        "Line 1: foo\nLine 2: bar\nLine 3: foo\nLine 4: baz\nLine 5: foo",
    )
    .unwrap();

    let result = tool_search_notes(&test_ctx(), dir.path(), "Workspace", "foo").unwrap();

    // Should find 3 matches at lines 1, 3, 5
    assert_eq!(result.len(), 3, "Expected 3 matches, got: {:?}", result);

    // Verify line numbers are in ascending order by extracting from the format "path:line - content"
    // The format is: "path:line_number - content"
    let line_nums: Vec<usize> = result
        .iter()
        .filter_map(|l| {
            // Find the first colon that's followed by digits (the line number)
            let colon_pos = l.find(':')?;
            let after_colon = &l[colon_pos + 1..];
            // Line number is before the next ' - '
            if let Some(dash_pos) = after_colon.find(" - ") {
                after_colon[..dash_pos].trim().parse().ok()
            } else {
                None
            }
        })
        .collect();
    assert_eq!(line_nums, vec![1, 3, 5], "Line numbers should be in order");
}

#[test]
fn test_tool_search_notes_case_insensitive() {
    // Grep is documented as case-insensitive
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Hello WORLD hello World HELLO").unwrap();

    let result = tool_search_notes(&test_ctx(), dir.path(), "Workspace", "hello").unwrap();
    let matches_text = result.join("\n");
    assert!(matches_text.contains("Hello"));
    assert!(matches_text.contains("WORLD"));
    assert!(matches_text.contains("hello"));
    assert!(matches_text.contains("World"));
    assert!(matches_text.contains("HELLO"));
}

#[test]
fn test_tool_search_notes_no_matches_returns_empty_vec() {
    // NEGATIVE ASSERTION: A query with no matches yields an empty
    // Vec; the "No matches found." sentinel is added by the tool
    // registry call site, not the low-level scan.
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("test.md"), "# Project\nNothing here").unwrap();
    let result = tool_search_notes(&test_ctx(), dir.path(), "Workspace", "nonexistent").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_grep_default_max_results_constant_is_200() {
    // The documented result cap. A regression here would silently
    // change the number of matches the LLM sees by default.
    assert_eq!(DEFAULT_SEARCH_NOTES_MAX_RESULTS, 200);
}

#[test]
fn test_tool_window_note_offset_past_end() {
    // BOUNDARY: offset past the end of the file returns empty content
    // (was an error in the 1-indexed variant; the offset/limit model is
    // forgiving so the LLM can walk forward without computing lengths).
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Line 1\nLine 2\nLine 3").unwrap();

    let result = tool_window_note(&test_ctx(), file_path.to_str().unwrap(), 999, 100)
        .unwrap()
        .content;
    assert_eq!(result, "");
}

#[test]
fn test_tool_window_note_limit_zero() {
    // BOUNDARY: limit=0 returns empty content without erroring.
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Line 1\nLine 2").unwrap();

    let result = tool_window_note(&test_ctx(), file_path.to_str().unwrap(), 0, 0)
        .unwrap()
        .content;
    assert_eq!(result, "");
}

#[test]
fn test_tool_window_note_limit_beyond_file() {
    // BOUNDARY: limit that would overflow the file is clamped to the
    // remainder. Mirrors the pre-migration "end beyond file" behavior.
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "Line 1\nLine 2\nLine 3").unwrap();

    let content = tool_window_note(&test_ctx(), file_path.to_str().unwrap(), 0, 100)
        .unwrap()
        .content;
    assert!(content.contains("Line 1"));
    assert!(content.contains("Line 2"));
    assert!(content.contains("Line 3"));
}

#[test]
fn test_tool_create_note_rejects_markdown_extension() {
    // NEGATIVE: Only .md extension is allowed, not .markdown
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("new.markdown");

    let producer = noop_producer();
    let result = tool_create_note(
        &test_ctx(),
        file_path.to_str().unwrap(),
        "# Hello",
        &*producer,
    );
    // Should reject .markdown extension
    assert_eq!(
        result.unwrap_err(),
        "Only markdown files (.md) are allowed."
    );
    assert!(!file_path.exists());
}

#[test]
fn test_tool_list_notes_excludes_subdirectories() {
    // POSITIVE: tool_list_notes is documented as non-recursive
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("root.md"), "content").unwrap();
    fs::create_dir(dir.path().join("subdir")).unwrap();
    fs::write(dir.path().join("subdir").join("nested.md"), "content").unwrap();

    let result = tool_list_notes(&test_ctx(), dir.path(), "Workspace").unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].ends_with("root.md"));
    // Nested files should NOT be included
    assert!(!result.iter().any(|p| p.contains("nested.md")));
}

#[test]
fn test_tool_list_notes_by_tag_with_markdown_extension() {
    // POSITIVE: files with .markdown extension should also be found
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("doc.markdown"),
        "---\ntags: [test-tag]\n---\n# Doc\n",
    )
    .unwrap();

    let res = tool_list_notes_by_tag(&test_ctx(), dir.path(), "Workspace", "test-tag").unwrap();
    assert_eq!(res.len(), 1);
    assert!(res[0].ends_with("doc.markdown"));
}
