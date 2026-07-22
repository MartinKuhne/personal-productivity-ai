//! Filesystem agent tools — grep/search, read file, list files by tag, create/update/delete files, and directory listing.

use crate::file_events::FileEventProducer;
use crate::utils::markdown::parse_front_matter;
use crate::utils::tags::extract_tags_from_file;
use std::path::Path;
use walkdir::WalkDir;

pub fn tool_grep(
    root_path: &Path,
    virtual_prefix: &str,
    query: &str,
) -> Result<crate::tools::dtos::GrepResponse, String> {
    let mut results = Vec::new();
    let query_lower = query.to_lowercase();
    for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == "md" || ext == "markdown" {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        for (idx, line) in content.lines().enumerate() {
                            if line.to_lowercase().contains(&query_lower) {
                                let rel_path =
                                    entry.path().strip_prefix(root_path).unwrap_or(entry.path());
                                let virtual_path = Path::new(virtual_prefix).join(rel_path);
                                results.push(format!(
                                    "{}:{} - {}",
                                    virtual_path.display(),
                                    idx + 1,
                                    line
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    if results.is_empty() {
        Ok(crate::tools::dtos::GrepResponse {
            matches: "No matches found.".to_string(),
        })
    } else {
        Ok(crate::tools::dtos::GrepResponse {
            matches: results.join("\n"),
        })
    }
}

pub fn tool_read_tags(root_path: &Path) -> Result<crate::tools::dtos::ReadTagsResponse, String> {
    let mut all_tags = std::collections::BTreeSet::new();
    for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == "md" || ext == "markdown" {
                    let tags = extract_tags_from_file(entry.path());
                    for tag in tags {
                        all_tags.insert(tag);
                    }
                }
            }
        }
    }
    Ok(crate::tools::dtos::ReadTagsResponse {
        tags: all_tags.into_iter().collect(),
    })
}

/// Default page size for the paginated `list_files` family of tools
/// (`list_files`, `list_files_by_tag`). Kept here (rather than
/// inlined in the call site) so the constant has one canonical home
/// and tests can reference it.
pub const DEFAULT_LIST_FILES_BY_TAG_PAGE_SIZE: usize = 20;

/// Scan a single content library and return every Markdown file whose
/// front-matter contains the given tag, as a sorted list of virtual
/// paths.
///
/// Paging is intentionally **not** applied here — the call site
/// (`registry.rs`) is responsible for slicing the combined
/// cross-library result, so the page and total fields stay consistent
/// regardless of how many libraries the user has configured.
pub fn tool_list_files_by_tag(
    root_path: &Path,
    virtual_prefix: &str,
    tag: &str,
) -> Result<Vec<String>, String> {
    let mut matching_files = Vec::new();
    for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == "md" || ext == "markdown" {
                    let tags = extract_tags_from_file(entry.path());
                    if tags.contains(&tag.to_string()) {
                        let rel_path = entry.path().strip_prefix(root_path).unwrap_or(entry.path());
                        let virtual_path = Path::new(virtual_prefix).join(rel_path);
                        matching_files.push(virtual_path.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    // Sort for deterministic paging at the call site — without a
    // stable order the same page could return different files on each
    // call.
    matching_files.sort();
    Ok(matching_files)
}

/// Scan a single directory (non-recursive) and return every Markdown
/// file's virtual path, sorted. Paging is intentionally **not**
/// applied here — the call site (`registry.rs`) is responsible for
/// slicing the result so the page and total fields stay consistent
/// regardless of how the call is dispatched.
pub fn tool_list_files(target_dir: &Path, virtual_prefix: &str) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(target_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let path = entry.path();
                    if let Some(ext) = path.extension() {
                        if ext == "md" || ext == "markdown" {
                            if let Some(name) = path.file_name() {
                                let virtual_path = Path::new(virtual_prefix).join(name);
                                files.push(virtual_path.to_string_lossy().into_owned());
                            }
                        }
                    }
                }
            }
        }
    }
    // Sort for deterministic paging at the call site.
    files.sort();
    Ok(files)
}

pub fn tool_read_file(path_str: &str) -> Result<crate::tools::dtos::ReadFileResponse, String> {
    match std::fs::read_to_string(path_str) {
        Ok(content) => Ok(crate::tools::dtos::ReadFileResponse { content }),
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

pub fn tool_read_file_lines(
    path_str: &str,
    start_line: usize,
    end_line: usize,
) -> Result<crate::tools::dtos::ReadFileLinesResponse, String> {
    match std::fs::read_to_string(path_str) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            if lines.is_empty() && start_line == 1 {
                return Ok(crate::tools::dtos::ReadFileLinesResponse {
                    content: "".to_string(),
                });
            }
            if start_line == 0 || start_line > lines.len() {
                return Err("Start line out of range.".to_string());
            }
            let end = std::cmp::min(end_line, lines.len());
            if start_line > end {
                return Err("Start line greater than end line.".to_string());
            }
            let selected_lines = &lines[start_line - 1..end];
            Ok(crate::tools::dtos::ReadFileLinesResponse {
                content: selected_lines.join("\n"),
            })
        }
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

pub fn tool_create_file(
    path_str: &str,
    content: &str,
    producer: &FileEventProducer,
) -> Result<crate::tools::dtos::CreateFileResponse, String> {
    if !path_str.to_lowercase().ends_with(".md") {
        return Err("Only markdown files (.md) are allowed.".to_string());
    }

    if content.starts_with("---\n") && parse_front_matter(content).is_none() {
        return Err("Invalid YAML front-matter in markdown.".to_string());
    }

    // Validate the markdown by ensuring it parses successfully
    let parser = pulldown_cmark::Parser::new(content);
    for _ in parser {}

    let path = Path::new(path_str);
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return Err(format!("Failed to create parent directories: {}", e));
        }
    }
    match std::fs::write(path, content) {
        Ok(_) => {
            let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            // Tell the rest of the app this file now exists so the
            // directory tree, tag manager, etc. can pick it up without
            // waiting for an OS-level notify event.
            producer.publish_discovered(path);
            Ok(crate::tools::dtos::CreateFileResponse {
                result: "File created successfully.".to_string(),
                size_bytes,
            })
        }
        Err(e) => Err(format!("Failed to write file: {}", e)),
    }
}

pub fn tool_insert_lines(
    path_str: &str,
    line_index: usize,
    lines_to_insert: &[String],
    producer: &FileEventProducer,
) -> Result<crate::tools::dtos::InsertLinesResponse, String> {
    match std::fs::read_to_string(path_str) {
        Ok(content) => {
            let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
            if line_index == 0 || line_index > lines.len() + 1 {
                return Err("Line index out of range.".to_string());
            }
            let idx = line_index - 1;
            for (offset, line) in lines_to_insert.iter().enumerate() {
                lines.insert(idx + offset, line.clone());
            }
            let new_content = lines.join("\n");
            match std::fs::write(path_str, new_content) {
                Ok(_) => {
                    producer.publish_updated(Path::new(path_str));
                    Ok(crate::tools::dtos::InsertLinesResponse {
                        result: "Lines inserted successfully.".to_string(),
                    })
                }
                Err(e) => Err(format!("Failed to write file: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

pub fn tool_delete_lines(
    path_str: &str,
    start_line: usize,
    end_line: usize,
    producer: &FileEventProducer,
) -> Result<crate::tools::dtos::DeleteLinesResponse, String> {
    match std::fs::read_to_string(path_str) {
        Ok(content) => {
            let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
            if start_line == 0 || start_line > lines.len() {
                return Err("Start line out of range.".to_string());
            }
            let end = std::cmp::min(end_line, lines.len());
            if start_line > end {
                return Err("Start line greater than end line.".to_string());
            }
            lines.drain((start_line - 1)..end);
            let new_content = lines.join("\n");
            match std::fs::write(path_str, new_content) {
                Ok(_) => {
                    producer.publish_updated(Path::new(path_str));
                    Ok(crate::tools::dtos::DeleteLinesResponse {
                        result: "Lines deleted successfully.".to_string(),
                    })
                }
                Err(e) => Err(format!("Failed to write file: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

pub fn tool_replace_text(
    path_str: &str,
    old_string: &str,
    new_string: &str,
    producer: &FileEventProducer,
) -> Result<crate::tools::dtos::ReplaceTextResponse, String> {
    match std::fs::read_to_string(path_str) {
        Ok(content) => {
            if !content.contains(old_string) {
                return Err("The specified old_string was not found in the file.".to_string());
            }
            let count = content.matches(old_string).count();
            let new_content = content.replace(old_string, new_string);
            match std::fs::write(path_str, new_content) {
                Ok(_) => {
                    producer.publish_updated(Path::new(path_str));
                    Ok(crate::tools::dtos::ReplaceTextResponse {
                        result: format!("Successfully replaced {} occurrence(s).", count),
                    })
                }
                Err(e) => Err(format!("Failed to write file: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_events::Bus;

    /// A producer that publishes to a throwaway bus. Tests don't
    /// need to consume the events — they only care about the
    /// success/failure of the underlying file operation.
    fn noop_producer() -> FileEventProducer<'static> {
        // We can't return a reference tied to a local bus, so
        // instead use a leaked one. Tests run in a single thread
        // here so leaking is fine for the test lifetime.
        let bus: &'static Bus<crate::file_events::FileEvent> = Box::leak(Box::new(Bus::new()));
        FileEventProducer::new(bus)
    }

    #[test]
    fn test_tool_replace_text() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        std::fs::write(&file_path, "Line 1\nOld Text\nLine 3").unwrap();

        let producer = noop_producer();
        let result = tool_replace_text(
            file_path.to_str().unwrap(),
            "Old Text",
            "New Text",
            &producer,
        )
        .unwrap()
        .result;
        assert_eq!(result, "Successfully replaced 1 occurrence(s).");

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Line 1\nNew Text\nLine 3");
    }

    #[test]
    fn test_tool_replace_text_not_found() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        std::fs::write(&file_path, "Line 1\nOld Text\nLine 3").unwrap();

        let producer = noop_producer();
        let result = tool_replace_text(
            file_path.to_str().unwrap(),
            "Missing Text",
            "New Text",
            &producer,
        );
        assert_eq!(
            result.unwrap_err(),
            "The specified old_string was not found in the file."
        );
    }

    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_tool_grep() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "# Hello\nWorld content\nAnother line").unwrap();

        let result = tool_grep(dir.path(), "Workspace", "World").unwrap().matches;
        assert!(result.contains("World content"));
        assert!(result.contains("Workspace"));
        assert!(result.contains("test.md"));
    }

    #[test]
    fn test_tool_list_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.md"), "content").unwrap();
        fs::write(dir.path().join("b.txt"), "content").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("c.md"), "content").unwrap();

        // The low-level tool now returns a `Vec<String>` of every
        // match (no paging, no newline joining). Paging is applied at
        // the registry call site.
        let result = tool_list_files(dir.path(), "Workspace").unwrap();
        assert_eq!(result.len(), 1, "non-recursive scan must return just a.md");
        assert!(result[0].ends_with("a.md"));
        assert!(result[0].starts_with("Workspace"));
    }

    #[test]
    fn test_tool_read_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Hello World").unwrap();

        let result = tool_read_file(file_path.to_str().unwrap()).unwrap().content;
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_tool_read_file_not_found() {
        let result = tool_read_file("/nonexistent/path.md");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read file"));
    }

    #[test]
    fn test_tool_read_file_lines() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3\nLine 4").unwrap();

        let result = tool_read_file_lines(file_path.to_str().unwrap(), 2, 3)
            .unwrap()
            .content;
        assert_eq!(result, "Line 2\nLine 3");
    }

    #[test]
    fn test_tool_read_file_lines_empty_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("empty.md");
        fs::write(&file_path, "").unwrap();

        let result = tool_read_file_lines(file_path.to_str().unwrap(), 1, 50)
            .unwrap()
            .content;
        assert_eq!(result, "");
    }

    #[test]
    fn test_tool_create_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new.md");

        let producer = noop_producer();
        let result = tool_create_file(
            file_path.to_str().unwrap(),
            "---\ntitle: Test\n---\n# Hello",
            &producer,
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
    fn test_tool_create_file_invalid_extension() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new.txt");

        let producer = noop_producer();
        let result = tool_create_file(file_path.to_str().unwrap(), "content", &producer);
        assert_eq!(
            result.unwrap_err(),
            "Only markdown files (.md) are allowed."
        );
    }

    #[test]
    fn test_tool_create_file_invalid_yaml() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new.md");

        let producer = noop_producer();
        let result = tool_create_file(
            file_path.to_str().unwrap(),
            "---\ninvalid: [unclosed\n---\nContent",
            &producer,
        );
        assert_eq!(
            result.unwrap_err(),
            "Invalid YAML front-matter in markdown."
        );
    }

    #[test]
    fn test_tool_insert_lines() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3").unwrap();

        let producer = noop_producer();
        let result = tool_insert_lines(
            file_path.to_str().unwrap(),
            2,
            &["New Line".to_string()],
            &producer,
        )
        .unwrap()
        .result;
        assert_eq!(result, "Lines inserted successfully.");

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Line 1\nNew Line\nLine 2\nLine 3");
    }

    #[test]
    fn test_tool_insert_lines_out_of_range() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2").unwrap();

        let producer = noop_producer();
        let result = tool_insert_lines(
            file_path.to_str().unwrap(),
            5,
            &["New".to_string()],
            &producer,
        );
        assert_eq!(result.unwrap_err(), "Line index out of range.");
    }

    #[test]
    fn test_tool_delete_lines() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3\nLine 4").unwrap();

        let producer = noop_producer();
        let result = tool_delete_lines(file_path.to_str().unwrap(), 2, 3, &producer)
            .unwrap()
            .result;
        assert_eq!(result, "Lines deleted successfully.");

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Line 1\nLine 4");
    }

    #[test]
    fn test_tool_delete_lines_out_of_range() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2").unwrap();

        let producer = noop_producer();
        let result = tool_delete_lines(file_path.to_str().unwrap(), 5, 6, &producer);
        assert_eq!(result.unwrap_err(), "Start line out of range.");
    }

    #[test]
    fn test_tool_read_tags() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "---\ntags: [tag1, tag2]\n---\n# Hello").unwrap();

        let result = tool_read_tags(dir.path()).unwrap().tags;
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
    fn test_list_files_by_tag_default_page_size_constant_is_20() {
        // The documented default. A regression here would silently
        // change the page size the LLM sees by default.
        assert_eq!(DEFAULT_LIST_FILES_BY_TAG_PAGE_SIZE, 20);
    }

    #[test]
    fn test_list_files_by_tag_returns_all_sorted_when_no_paging_in_tool() {
        // The low-level tool returns every match (sorted) without
        // slicing — paging lives at the call site so it can be
        // applied to the cross-library result.
        let dir = build_tagged_library(5, "meeting");
        let res = tool_list_files_by_tag(dir.path(), "Workspace", "meeting").unwrap();
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
        let res = tool_list_files_by_tag(dir.path(), "Workspace", "meeting").unwrap();
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
        let res = tool_list_files_by_tag(dir.path(), "Workspace", "meeting").unwrap();
        assert_eq!(res.len(), 1);
        assert!(res[0].ends_with("note.md"));
        assert!(res[0].starts_with("Workspace"));
    }

    // =====================================================================
    // Additional edge case tests
    // =====================================================================

    #[test]
    fn test_tool_grep_ignores_non_markdown_files() {
        // NEGATIVE ASSERTION: Files without .md or .markdown extension
        // must NOT be searched, even if they contain matching text.
        let dir = tempdir().unwrap();
        let md_file = dir.path().join("test.md");
        let txt_file = dir.path().join("secret.txt");
        let pdf_file = dir.path().join("notes.pdf");

        fs::write(&md_file, "# Project\nContains search term here").unwrap();
        fs::write(&txt_file, "This also contains search term").unwrap();
        fs::write(&pdf_file, "Search term in PDF").unwrap();

        let result = tool_grep(dir.path(), "Workspace", "search term").unwrap();
        // Only the .md file should be found
        assert!(result.matches.contains("test.md"));
        assert!(result.matches.contains("Contains search term"));
        // txt and pdf must NOT appear in results
        assert!(!result.matches.contains("secret.txt"));
        assert!(!result.matches.contains("notes.pdf"));
    }

    #[test]
    fn test_tool_grep_multiple_matches_same_file() {
        // ORDERING ASSERTION: When a query matches multiple lines in the
        // same file, they should appear in line number order.
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(
            &file_path,
            "Line 1: foo\nLine 2: bar\nLine 3: foo\nLine 4: baz\nLine 5: foo",
        )
        .unwrap();

        let result = tool_grep(dir.path(), "Workspace", "foo").unwrap();
        let matches_text = result.matches;
        let lines: Vec<&str> = matches_text.lines().collect();

        // Should find 3 matches at lines 1, 3, 5
        assert_eq!(lines.len(), 3, "Expected 3 matches, got: {}", matches_text);

        // Verify line numbers are in ascending order by extracting from the format "path:line - content"
        // The format is: "path:line_number - content"
        let line_nums: Vec<usize> = lines
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

        // Verify the matches are in the correct positions
        assert!(lines[0].contains("Line 1"));
        assert!(lines[1].contains("Line 3"));
        assert!(lines[2].contains("Line 5"));
    }

    #[test]
    fn test_tool_grep_case_insensitive() {
        // Grep is documented as case-insensitive
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Hello WORLD hello World HELLO").unwrap();

        let result = tool_grep(dir.path(), "Workspace", "hello").unwrap();
        assert!(result.matches.contains("Hello"));
        assert!(result.matches.contains("WORLD"));
        assert!(result.matches.contains("hello"));
        assert!(result.matches.contains("World"));
        assert!(result.matches.contains("HELLO"));
    }

    #[test]
    fn test_tool_read_file_lines_start_greater_than_end() {
        // BOUNDARY: start_line > end_line should error
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3").unwrap();

        let result = tool_read_file_lines(file_path.to_str().unwrap(), 3, 1);
        assert_eq!(result.unwrap_err(), "Start line greater than end line.");
    }

    #[test]
    fn test_tool_read_file_lines_boundary_zero() {
        // BOUNDARY: start_line=0 should error (1-indexed)
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2").unwrap();

        let result = tool_read_file_lines(file_path.to_str().unwrap(), 0, 2);
        assert_eq!(result.unwrap_err(), "Start line out of range.");
    }

    #[test]
    fn test_tool_read_file_lines_end_beyond_file() {
        // BOUNDARY: end_line beyond file length should return available content
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3").unwrap();

        let result = tool_read_file_lines(file_path.to_str().unwrap(), 1, 100);
        // Should return all lines up to end of file
        let content = result.unwrap().content;
        assert!(content.contains("Line 1"));
        assert!(content.contains("Line 3"));
    }

    #[test]
    fn test_tool_create_file_rejects_markdown_extension() {
        // NEGATIVE: Only .md extension is allowed, not .markdown
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new.markdown");

        let producer = noop_producer();
        let result = tool_create_file(file_path.to_str().unwrap(), "# Hello", &producer);
        // Should reject .markdown extension
        assert_eq!(
            result.unwrap_err(),
            "Only markdown files (.md) are allowed."
        );
        assert!(!file_path.exists());
    }

    #[test]
    fn test_tool_delete_lines_boundary_start_equals_end() {
        // BOUNDARY: deleting single line (start == end) should work
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3").unwrap();

        let producer = noop_producer();
        let result = tool_delete_lines(file_path.to_str().unwrap(), 2, 2, &producer)
            .unwrap()
            .result;
        assert_eq!(result, "Lines deleted successfully.");

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Line 1\nLine 3");
    }

    #[test]
    fn test_tool_delete_lines_start_beyond_content() {
        // BOUNDARY: start_line beyond content should error
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2").unwrap();

        let producer = noop_producer();
        let result = tool_delete_lines(file_path.to_str().unwrap(), 5, 10, &producer);
        assert_eq!(result.unwrap_err(), "Start line out of range.");
    }

    #[test]
    fn test_tool_list_files_excludes_subdirectories() {
        // POSITIVE: tool_list_files is documented as non-recursive
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("root.md"), "content").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("subdir").join("nested.md"), "content").unwrap();

        let result = tool_list_files(dir.path(), "Workspace").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with("root.md"));
        // Nested files should NOT be included
        assert!(!result.iter().any(|p| p.contains("nested.md")));
    }

    #[test]
    fn test_tool_list_files_by_tag_with_markdown_extension() {
        // POSITIVE: files with .markdown extension should also be found
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("doc.markdown"),
            "---\ntags: [test-tag]\n---\n# Doc\n",
        )
        .unwrap();

        let res = tool_list_files_by_tag(dir.path(), "Workspace", "test-tag").unwrap();
        assert_eq!(res.len(), 1);
        assert!(res[0].ends_with("doc.markdown"));
    }
}
