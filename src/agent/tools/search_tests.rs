//! Tests for `tools/search.rs`.

use super::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_is_markdown_file_accepts_md_and_markdown_case_insensitively() {
    assert!(is_markdown_file(Path::new("note.md")));
    assert!(is_markdown_file(Path::new("doc.markdown")));
    assert!(is_markdown_file(Path::new("NOTE.MD")));
    assert!(is_markdown_file(Path::new("Doc.MarkDown")));
    assert!(!is_markdown_file(Path::new("note.txt")));
    assert!(!is_markdown_file(Path::new("slides.pdf")));
    assert!(!is_markdown_file(Path::new("image.png")));
    assert!(!is_markdown_file(Path::new("no-extension")));
}

#[test]
fn test_search_content_is_case_insensitive_literal() {
    let content = "Hello WORLD hello World HELLO";
    let matches = search_content("hello", content, false);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].0, 1);
    assert!(matches[0].1.contains("Hello"));
}

#[test]
fn test_search_content_treats_query_as_literal_not_regex() {
    // A regex `.` would match "abc"; as a literal only "a.c" matches.
    let content = "abc\nline with a.c inside\n";
    let matches = search_content("a.c", content, false);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].0, 2);
}

#[test]
fn test_search_content_blank_query_matches_nothing() {
    let matches = search_content("   ", "some content\n", false);
    assert!(matches.is_empty());
    let matches = search_content("", "some content\n", false);
    assert!(matches.is_empty());
}

#[test]
fn test_search_content_reports_lines_in_order() {
    let content = "Line 1: foo\nLine 2: bar\nLine 3: foo\nLine 4: baz\nLine 5: foo";
    let matches = search_content("foo", content, false);
    let line_numbers: Vec<u64> = matches.iter().map(|(n, _)| *n).collect();
    assert_eq!(line_numbers, vec![1, 3, 5]);
}

#[test]
fn test_search_content_strips_front_matter_with_file_line_numbers() {
    let content = "---\ntags: [meeting]\n---\n# Hello\nWorld content\n";
    // Stripped: the header match must not appear, body lines keep file numbers.
    let stripped = search_content("meeting", content, true);
    assert!(stripped.is_empty());
    let body = search_content("world", content, true);
    assert_eq!(body.len(), 1);
    assert_eq!(body[0].0, 5);
    // Unstripped (tree-search contract): the header matches at file line 2.
    let whole = search_content("meeting", content, false);
    assert_eq!(whole.len(), 1);
    assert_eq!(whole[0].0, 2);
}

#[test]
fn test_search_content_keeps_invalid_front_matter_searchable() {
    // `---` without valid YAML is not front-matter, so it stays searchable,
    // mirroring `parse_front_matter` (only valid YAML is stripped).
    let content = "---\nnot: [valid yaml\n---\nneedle here\n";
    let matches = search_content("needle", content, true);
    assert_eq!(matches.len(), 1);
}

#[test]
fn test_collect_markdown_files_finds_nested_notes_only() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "x").unwrap();
    fs::write(dir.path().join("b.markdown"), "x").unwrap();
    fs::write(dir.path().join("c.txt"), "x").unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub").join("d.md"), "x").unwrap();

    let mut files = collect_markdown_files(dir.path());
    files.sort();
    assert_eq!(files.len(), 3);
    assert!(files.iter().all(|p| is_markdown_file(p)));
}

#[test]
fn test_search_files_parallel_skips_non_markdown_and_missing() {
    let dir = tempdir().unwrap();
    let hit = dir.path().join("hit.md");
    let miss = dir.path().join("miss.md");
    let txt = dir.path().join("notes.txt");
    let ghost = dir.path().join("ghost.md");
    fs::write(&hit, "needle in here\n").unwrap();
    fs::write(&miss, "nothing here\n").unwrap();
    fs::write(&txt, "needle but plain text\n").unwrap();

    let files = vec![hit.clone(), miss.clone(), txt.clone(), ghost.clone()];
    let matches = search_files_parallel(&files, "needle", false, |p| std::fs::read_to_string(p));
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].path, hit);
    assert_eq!(matches[0].lines.len(), 1);
}

#[test]
fn test_search_files_parallel_follows_input_order_deterministically() {
    let dir = tempdir().unwrap();
    let mut files = Vec::new();
    for i in (0..8).rev() {
        let path = dir.path().join(format!("note_{i:02}.md"));
        fs::write(&path, format!("shared term {i}\n")).unwrap();
        files.push(path);
    }
    let read = |p: &Path| std::fs::read_to_string(p);
    let first = search_files_parallel(&files, "shared term", false, read);
    let second = search_files_parallel(&files, "shared term", false, read);
    assert_eq!(first, second);
    let paths: Vec<PathBuf> = first.iter().map(|m| m.path.clone()).collect();
    assert_eq!(paths, files);
}

#[test]
fn test_search_files_parallel_strips_front_matter_for_agent_contract() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("note.md");
    fs::write(&path, "---\ntags: [meeting]\n---\nbody text\n").unwrap();

    let files = vec![path];
    let stripped = search_files_parallel(&files, "meeting", true, |p| std::fs::read_to_string(p));
    assert!(stripped.is_empty());
    let whole = search_files_parallel(&files, "meeting", false, |p| std::fs::read_to_string(p));
    assert_eq!(whole.len(), 1);
    assert_eq!(whole[0].lines[0].0, 2);
}

#[test]
fn test_count_occurrences_ascii_is_case_insensitive_and_non_overlapping() {
    assert_eq!(count_occurrences("Hello hello HELLO", "hello"), 3);
    // Non-overlapping: "aa" occurs twice in "aaaa", not three times.
    assert_eq!(count_occurrences("aaaa", "aa"), 2);
    assert_eq!(count_occurrences("nothing here", "zzz"), 0);
    assert_eq!(count_occurrences("anything", ""), 0);
}

#[test]
fn test_count_occurrences_supports_unicode() {
    assert_eq!(count_occurrences(" caf\u{e9} CAF\u{c9} ", "caf\u{e9}"), 2);
}

#[test]
fn test_extract_snippet_returns_short_lines_unchanged() {
    assert_eq!(extract_snippet("  short line  ", "short"), "short line");
}

#[test]
fn test_extract_snippet_windows_long_lines_around_term() {
    let line = format!("{} needle {}", "x".repeat(80), "y".repeat(80));
    let snippet = extract_snippet(&line, "needle");
    assert!(snippet.contains("needle"));
    assert!(snippet.starts_with("…"));
    assert!(snippet.ends_with("…"));
}

#[test]
fn test_extract_snippet_falls_back_without_term() {
    let line = "z".repeat(120);
    let snippet = extract_snippet(&line, "needle");
    assert!(snippet.ends_with("…"));
    assert_eq!(snippet.chars().count(), 91);
}
