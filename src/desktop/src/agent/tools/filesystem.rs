//! Filesystem agent tools — grep/search, read file, list files by tag, create/update/delete files, and directory listing.

use crate::bus::events::file::FileEventProducer;
use crate::markdown::parse_front_matter;
use crate::utils::tags::extract_tags_from_file;
use std::path::Path;
use walkdir::WalkDir;

/// Default maximum number of match lines the `grep` tool returns in
/// a single response. Kept here (rather than inlined at the call site)
/// so the constant has one canonical home and tests can reference it.
pub const DEFAULT_GREP_MAX_RESULTS: usize = 200;

/// Grep a single content library for a query string, case-insensitively.
/// Returns every matching line as `virtual/path:line - content`, scoped
/// strictly to Markdown (`.md`) files under `root_path`. The caller
/// (the tool registry) is responsible for applying the result cap
/// across libraries, so this function returns all matches unfiltered.
pub fn tool_grep(
    root_path: &Path,
    virtual_prefix: &str,
    query: &str,
) -> Result<Vec<String>, String> {
    let mut results = Vec::new();
    let query_lower = query.to_lowercase();
    for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file()
            && let Some(ext) = entry.path().extension()
            && ext == "md"
            && let Some(rel_path) = entry.path().strip_prefix(root_path).ok()
            && let Ok(content) = std::fs::read_to_string(entry.path())
        {
            let virtual_path = Path::new(virtual_prefix).join(rel_path);
            for (idx, line) in content.lines().enumerate() {
                if line.to_lowercase().contains(&query_lower) {
                    results.push(format!("{}:{} - {}", virtual_path.display(), idx + 1, line));
                }
            }
        }
    }
    Ok(results)
}

pub fn tool_read_tags(
    root_path: &Path,
) -> Result<crate::agent::tools::dtos::ReadTagsResponse, String> {
    let mut all_tags = std::collections::BTreeSet::new();
    for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file()
            && let Some(ext) = entry.path().extension()
            && (ext == "md" || ext == "markdown")
        {
            let tags = extract_tags_from_file(entry.path());
            for tag in tags {
                all_tags.insert(tag);
            }
        }
    }
    Ok(crate::agent::tools::dtos::ReadTagsResponse {
        tags: all_tags.into_iter().collect(),
    })
}

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
        if entry.path().is_file()
            && let Some(ext) = entry.path().extension()
            && (ext == "md" || ext == "markdown")
        {
            let tags = extract_tags_from_file(entry.path());
            if tags.contains(&tag.to_string()) {
                let rel_path = entry.path().strip_prefix(root_path).unwrap_or(entry.path());
                let virtual_path = Path::new(virtual_prefix).join(rel_path);
                matching_files.push(virtual_path.to_string_lossy().into_owned());
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
            if let Ok(file_type) = entry.file_type()
                && file_type.is_file()
            {
                let path = entry.path();
                if let Some(ext) = path.extension()
                    && (ext == "md" || ext == "markdown")
                    && let Some(name) = path.file_name()
                {
                    let virtual_path = Path::new(virtual_prefix).join(name);
                    files.push(virtual_path.to_string_lossy().into_owned());
                }
            }
        }
    }
    // Sort for deterministic paging at the call site.
    files.sort();
    Ok(files)
}

pub fn tool_read_file(
    path_str: &str,
) -> Result<crate::agent::tools::dtos::ReadFileResponse, String> {
    match crate::utils::read_text_file(Path::new(path_str)) {
        Ok(content) => Ok(crate::agent::tools::dtos::ReadFileResponse { content }),
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

pub fn tool_read_file_lines(
    path_str: &str,
    start_line: usize,
    end_line: usize,
) -> Result<crate::agent::tools::dtos::ReadFileLinesResponse, String> {
    match crate::utils::read_text_file(Path::new(path_str)) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            if lines.is_empty() && start_line == 1 {
                return Ok(crate::agent::tools::dtos::ReadFileLinesResponse {
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
            Ok(crate::agent::tools::dtos::ReadFileLinesResponse {
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
) -> Result<crate::agent::tools::dtos::CreateFileResponse, String> {
    if !path_str.to_lowercase().ends_with(".md") {
        return Err("Only markdown files (.md) are allowed.".to_string());
    }

    if content.starts_with("---\n") && parse_front_matter(content).is_none() {
        return Err("Invalid YAML front-matter in markdown.".to_string());
    }

    // Validate the markdown by ensuring it parses successfully
    let _events = crate::markdown::parse_markdown_to_events(content);

    let path = Path::new(path_str);
    if path.exists() {
        return Err("File already exists. This tool can only create new files.".to_string());
    }

    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return Err(format!("Failed to create parent directories: {}", e));
    }
    match std::fs::write(path, content) {
        Ok(_) => {
            let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            // Tell the rest of the app this file now exists so the
            // directory tree, tag manager, etc. can pick it up without
            // waiting for an OS-level notify event.
            producer.publish_discovered(path);
            Ok(crate::agent::tools::dtos::CreateFileResponse {
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
) -> Result<crate::agent::tools::dtos::InsertLinesResponse, String> {
    match crate::utils::read_text_file(Path::new(path_str)) {
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
                    Ok(crate::agent::tools::dtos::InsertLinesResponse {
                        result: "Lines inserted successfully.".to_string(),
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
) -> Result<crate::agent::tools::dtos::ReplaceTextResponse, String> {
    match crate::utils::read_text_file(Path::new(path_str)) {
        Ok(content) => {
            if !content.contains(old_string) {
                return Err("The specified old_string was not found in the file.".to_string());
            }
            let count = content.matches(old_string).count();
            let new_content = content.replace(old_string, new_string);
            match std::fs::write(path_str, new_content) {
                Ok(_) => {
                    producer.publish_updated(Path::new(path_str));
                    Ok(crate::agent::tools::dtos::ReplaceTextResponse {
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
    use crate::bus::core::Bus;
    use crate::bus::events::file::FileEvent;

    /// A producer that publishes to a throwaway bus. Tests don't
    /// need to consume the events — they only care about the
    /// success/failure of the underlying file operation.
    fn noop_producer() -> FileEventProducer<'static> {
        // We can't return a reference tied to a local bus, so
        // instead use a leaked one. Tests run in a single thread
        // here so leaking is fine for the test lifetime.
        let bus: &'static Bus<FileEvent> = Box::leak(Box::new(Bus::new()));
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

        let result = tool_grep(dir.path(), "Workspace", "World").unwrap();
        assert!(result.iter().any(|m| m.contains("World content")));
        assert!(result.iter().any(|m| m.contains("Workspace")));
        assert!(result.iter().any(|m| m.contains("test.md")));
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
    fn test_tool_create_file_fails_if_exists() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("existing.md");
        fs::write(&file_path, "existing content").unwrap();

        let producer = noop_producer();
        let result = tool_create_file(
            file_path.to_str().unwrap(),
            "---\ntitle: Test\n---\n# Hello",
            &producer,
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

        let result = tool_grep(dir.path(), "Workspace", "search term").unwrap();
        // Only the .md file should be found
        assert!(result.iter().any(|m| m.contains("test.md")));
        assert!(result.iter().any(|m| m.contains("Contains search term")));
        // txt, pdf, and .markdown must NOT appear in results
        assert!(!result.iter().any(|m| m.contains("secret.txt")));
        assert!(!result.iter().any(|m| m.contains("notes.pdf")));
        assert!(!result.iter().any(|m| m.contains("doc.markdown")));
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
    fn test_tool_grep_case_insensitive() {
        // Grep is documented as case-insensitive
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Hello WORLD hello World HELLO").unwrap();

        let result = tool_grep(dir.path(), "Workspace", "hello").unwrap();
        let matches_text = result.join("\n");
        assert!(matches_text.contains("Hello"));
        assert!(matches_text.contains("WORLD"));
        assert!(matches_text.contains("hello"));
        assert!(matches_text.contains("World"));
        assert!(matches_text.contains("HELLO"));
    }

    #[test]
    fn test_tool_grep_no_matches_returns_empty_vec() {
        // NEGATIVE ASSERTION: A query with no matches yields an empty
        // Vec; the "No matches found." sentinel is added by the tool
        // registry call site, not the low-level scan.
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.md"), "# Project\nNothing here").unwrap();
        let result = tool_grep(dir.path(), "Workspace", "nonexistent").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_grep_default_max_results_constant_is_200() {
        // The documented result cap. A regression here would silently
        // change the number of matches the LLM sees by default.
        assert_eq!(DEFAULT_GREP_MAX_RESULTS, 200);
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
