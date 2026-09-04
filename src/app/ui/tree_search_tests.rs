//! Unit tests for `tree_search.rs`.

use super::*;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn write_file(path: &Path, content: &str) {
    let mut file = fs::File::create(path).expect("test file should be created");
    file.write_all(content.as_bytes())
        .expect("test file should be written");
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fastmd_tree_search_{name}"));
    fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

#[test]
fn test_apply_filters_files_by_content() {
    let dir = temp_dir("apply");
    let matching = dir.join("match.md");
    let non_matching = dir.join("other.md");
    write_file(&matching, "This document mentions blueberries.");
    write_file(&non_matching, "Nothing here matches.");

    let mut search = TreeSearch::new();
    *search.query_mut() = "blueberries".to_string();
    search.apply(&[matching.clone(), non_matching.clone()], &[]);

    assert!(search.is_searching());
    assert_eq!(search.active_filter(), Some("blueberries"));
    assert!(search.matching_files().contains(&matching));
    assert!(!search.matching_files().contains(&non_matching));
    assert_eq!(search.results().len(), 1);
    assert_eq!(search.results()[0].path, matching);
    assert_eq!(search.results()[0].file_name, "match.md");
    assert_eq!(search.results()[0].line_number, 1);
    assert!(search.results()[0].snippet.contains("blueberries"));
}

#[test]
fn test_apply_is_case_insensitive() {
    let dir = temp_dir("case");
    let file = dir.join("notes.md");
    write_file(&file, "Hello World");

    let mut search = TreeSearch::new();
    *search.query_mut() = "HELLO".to_string();
    search.apply(std::slice::from_ref(&file), &[]);

    assert!(search.matching_files().contains(&file));
    assert_eq!(search.results().len(), 1);
}

#[test]
fn test_apply_trims_query() {
    let dir = temp_dir("trim");
    let file = dir.join("notes.md");
    write_file(&file, "needle");

    let mut search = TreeSearch::new();
    *search.query_mut() = "  needle  ".to_string();
    search.apply(std::slice::from_ref(&file), &[]);

    assert_eq!(search.active_filter(), Some("needle"));
}

#[test]
fn test_apply_with_empty_query_clears_filter() {
    let mut search = TreeSearch::new();
    *search.query_mut() = "needle".to_string();
    search.apply(&[], &[]);
    assert!(search.is_searching());

    *search.query_mut() = "   ".to_string();
    search.apply(&[], &[]);
    assert!(!search.is_searching());
    assert!(search.active_filter().is_none());
    assert!(search.matching_files().is_empty());
    assert!(search.results().is_empty());
}

#[test]
fn test_clear_resets_all_state() {
    let dir = temp_dir("clear");
    let file = dir.join("notes.md");
    write_file(&file, "needle");

    let mut search = TreeSearch::new();
    *search.query_mut() = "needle".to_string();
    search.apply(std::slice::from_ref(&file), &[]);
    assert!(search.is_searching());

    search.clear();
    assert!(search.query().is_empty());
    assert!(!search.is_searching());
    assert!(search.active_filter().is_none());
    assert!(search.matching_files().is_empty());
    assert!(search.results().is_empty());
}

#[test]
fn test_missing_file_never_matches() {
    let dir = temp_dir("missing");
    let ghost = dir.join("does_not_exist.md");
    let mut search = TreeSearch::new();
    *search.query_mut() = "anything".to_string();
    search.apply(&[ghost], &[]);
    assert!(search.matching_files().is_empty());
    assert!(search.results().is_empty());
    assert!(search.is_searching());
}

#[test]
fn test_one_entry_per_file_with_multiple_matches() {
    let dir = temp_dir("multi_matches");
    let file = dir.join("multi.md");
    write_file(
        &file,
        "Line 1: first target\nLine 2: other text\nLine 3: second target\nLine 4: target again",
    );

    let mut search = TreeSearch::new();
    *search.query_mut() = "target".to_string();
    search.apply(std::slice::from_ref(&file), &[]);

    assert_eq!(
        search.results().len(),
        1,
        "Must have exactly one entry per file"
    );
    let res = &search.results()[0];
    assert_eq!(res.line_number, 1);
    assert_eq!(res.match_count, 3);
    assert_eq!(res.snippet, "Line 1: first target");
}

#[test]
fn test_compute_relative_path() {
    let lib_dir = temp_dir("rel_lib");
    let file = lib_dir.join("subdir").join("note.md");
    let libraries = vec![ContentLibrary {
        root_folder: lib_dir.to_string_lossy().to_string(),
        name: "Docs".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];

    let rel = compute_relative_path(&file, &libraries);
    assert_eq!(rel, "Docs: subdir/note.md");
}

#[test]
fn test_search_skips_directories_without_markdown_files() {
    let dir_no_md = temp_dir("no_md");
    let txt_in_no_md = dir_no_md.join("doc.txt");
    write_file(&txt_in_no_md, "contains target_word");

    let dir_with_md = temp_dir("with_md");
    let md_file = dir_with_md.join("guide.md");
    write_file(&md_file, "also contains target_word");

    let mut search = TreeSearch::new();
    *search.query_mut() = "target_word".to_string();
    search.apply(&[txt_in_no_md.clone(), md_file.clone()], &[]);

    assert_eq!(
        search.results().len(),
        1,
        "Only markdown files in directories containing markdown files should be returned"
    );
    assert_eq!(search.results()[0].path, md_file);
    assert!(!search.matching_files().contains(&txt_in_no_md));
}

#[test]
fn test_search_skips_non_markdown_files() {
    let dir = temp_dir("skip_non_md");
    let md_file = dir.join("article.markdown");
    let txt_file = dir.join("notes.txt");
    let log_file = dir.join("build.log");
    write_file(&md_file, "keyword here");
    write_file(&txt_file, "keyword here");
    write_file(&log_file, "keyword here");

    let mut search = TreeSearch::new();
    *search.query_mut() = "keyword".to_string();
    search.apply(&[md_file.clone(), txt_file.clone(), log_file.clone()], &[]);

    assert_eq!(
        search.results().len(),
        1,
        "Only .md/.markdown files should be searched and returned"
    );
    assert_eq!(search.results()[0].path, md_file);
}

#[test]
fn test_search_skips_directories_outside_content_libraries() {
    let lib_dir = temp_dir("in_library");
    let md_in_lib = lib_dir.join("inside.md");
    write_file(&md_in_lib, "shared secret phrase");

    let outside_dir = temp_dir("outside_library");
    let md_outside_lib = outside_dir.join("outside.md");
    write_file(&md_outside_lib, "shared secret phrase");

    let libraries = vec![ContentLibrary {
        root_folder: lib_dir.to_string_lossy().to_string(),
        name: "MyLib".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];

    let mut search = TreeSearch::new();
    *search.query_mut() = "secret".to_string();
    search.apply(&[md_in_lib.clone(), md_outside_lib.clone()], &libraries);

    assert_eq!(
        search.results().len(),
        1,
        "Only files in directories within content libraries should be searched"
    );
    assert_eq!(search.results()[0].path, md_in_lib);
    assert!(!search.matching_files().contains(&md_outside_lib));
}
