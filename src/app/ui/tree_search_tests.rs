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

/// An isolated fixture directory that removes itself when the returned guard
/// is dropped.
///
/// `name` becomes the directory prefix so a path still identifies the test that
/// created it, while `tempfile` guarantees a unique directory per call: the old
/// `std::env::temp_dir()/fastmd_tree_search_<name>` fixtures collided between
/// parallel test processes and leaked on failure (RUST-006).
fn temp_dir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("fastmd_tree_search_{name}_"))
        .tempdir()
        .expect("temp dir should be created")
}

#[test]
fn test_apply_filters_files_by_content() {
    let fixture = temp_dir("apply");
    let dir = fixture.path();
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
    let fixture = temp_dir("case");
    let dir = fixture.path();
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
    let fixture = temp_dir("trim");
    let dir = fixture.path();
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
    let fixture = temp_dir("clear");
    let dir = fixture.path();
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
    let fixture = temp_dir("missing");
    let dir = fixture.path();
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
    let fixture = temp_dir("multi_matches");
    let dir = fixture.path();
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
    let fixture = temp_dir("rel_lib");
    let lib_dir = fixture.path();
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
    let no_md_fixture = temp_dir("no_md");
    let dir_no_md = no_md_fixture.path();
    let txt_in_no_md = dir_no_md.join("doc.txt");
    write_file(&txt_in_no_md, "contains target_word");

    let with_md_fixture = temp_dir("with_md");
    let dir_with_md = with_md_fixture.path();
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
    let fixture = temp_dir("skip_non_md");
    let dir = fixture.path();
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
    let lib_fixture = temp_dir("in_library");
    let lib_dir = lib_fixture.path();
    let md_in_lib = lib_dir.join("inside.md");
    write_file(&md_in_lib, "shared secret phrase");

    let outside_fixture = temp_dir("outside_library");
    let outside_dir = outside_fixture.path();
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

#[test]
fn test_search_scoped_to_allowed_directories() {
    let dir1 = temp_dir("allowed_dir1");
    let md1 = dir1.path().join("note1.md");
    write_file(&md1, "same keyword everywhere");

    let dir2 = temp_dir("disallowed_dir2");
    let md2 = dir2.path().join("note2.md");
    write_file(&md2, "same keyword everywhere");

    let mut allowed_dirs = HashSet::new();
    allowed_dirs.insert(dir1.path().to_path_buf());

    let mut search = TreeSearch::new();
    *search.query_mut() = "keyword".to_string();
    search.apply_with_allowed_dirs(&[md1.clone(), md2.clone()], &[], Some(&allowed_dirs));

    assert_eq!(
        search.results().len(),
        1,
        "Only directories in allowed_dirs should be searched"
    );
    assert_eq!(search.results()[0].path, md1);
    assert!(!search.matching_files().contains(&md2));
}

#[test]
fn test_tree_node_collect_dirs() {
    let mut root = crate::ui::TreeNode::new("Workspace".to_string(), PathBuf::new(), true);
    let dir_a = PathBuf::from("/lib/dir_a");
    let dir_b = PathBuf::from("/lib/dir_b");
    let file_c = PathBuf::from("/lib/dir_a/file.md");

    let mut node_a = crate::ui::TreeNode::new("dir_a".to_string(), dir_a.clone(), true);
    node_a.children.insert(
        "file.md".to_string(),
        crate::ui::TreeNode::new("file.md".to_string(), file_c, false),
    );
    let node_b = crate::ui::TreeNode::new("dir_b".to_string(), dir_b.clone(), true);

    root.children.insert("dir_a".to_string(), node_a);
    root.children.insert("dir_b".to_string(), node_b);

    let mut dirs = HashSet::new();
    root.collect_dirs(&mut dirs);

    assert_eq!(dirs.len(), 2);
    assert!(dirs.contains(&dir_a));
    assert!(dirs.contains(&dir_b));
}

#[test]
fn test_results_arc_reuses_allocation_and_clears() {
    let fixture = temp_dir("results_arc");
    let dir = fixture.path();
    let file = dir.join("notes.md");
    write_file(&file, "keyword here");

    let mut search = TreeSearch::new();
    *search.query_mut() = "keyword".to_string();
    search.apply(std::slice::from_ref(&file), &[]);

    let first = search.results_arc();
    assert_eq!(first.len(), 1);

    // Same content: the Arc allocation is reused (pointer bump).
    let second = search.results_arc();
    assert!(Arc::ptr_eq(&first, &second));

    // Clearing swaps in a new (empty) allocation.
    search.clear();
    let third = search.results_arc();
    assert!(!Arc::ptr_eq(&first, &third));
    assert!(third.is_empty());
}

// ---------------------------------------------------------------------------
// Additional coverage: isolated fixtures (tempfile) for every new test below.
// ---------------------------------------------------------------------------

/// Build a content library rooted at `root`.
fn library(name: &str, root: &Path) -> ContentLibrary {
    ContentLibrary {
        root_folder: root.to_string_lossy().to_string(),
        name: name.to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }
}

/// Write `<dir>/<relative>` (creating parent directories) and return the path.
fn write_in(dir: &Path, relative: &str, content: &str) -> PathBuf {
    let path = dir.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create fixture directories");
    }
    write_file(&path, content);
    path
}

#[test]
fn test_default_state_is_empty() {
    let search = TreeSearch::default();

    assert!(search.query().is_empty());
    assert!(!search.is_searching());
    assert!(!search.is_filtering());
    assert!(search.active_filter().is_none());
    assert!(search.results().is_empty());
    assert!(search.results_arc().is_empty());
    assert!(search.matching_files().is_empty());
}

#[test]
fn test_is_filtering_mirrors_is_searching() {
    let mut search = TreeSearch::new();
    assert!(!search.is_filtering());

    *search.query_mut() = "term".to_string();
    search.apply(&[], &[]);
    assert!(
        search.is_filtering(),
        "a committed filter is also a content filter"
    );

    search.clear();
    assert!(!search.is_filtering());
}

#[test]
fn test_results_arc_shares_the_committed_vector() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = write_in(dir.path(), "docs/a.md", "alpha");

    let mut search = TreeSearch::new();
    *search.query_mut() = "alpha".to_string();
    search.apply(std::slice::from_ref(&file), &[]);

    let arc = search.results_arc();
    assert_eq!(arc.len(), 1);
    assert_eq!(
        arc.as_slice(),
        search.results(),
        "the Arc handle must expose the same entries as `results()`"
    );
}

#[test]
fn test_compute_relative_path_variants() {
    let dir = tempfile::tempdir().expect("tempdir");
    let libs = vec![library("Docs", dir.path())];

    assert_eq!(
        compute_relative_path(dir.path(), &libs),
        "Docs",
        "a path equal to the library root is labelled with the library name"
    );
    assert_eq!(
        compute_relative_path(&dir.path().join("sub").join("note.md"), &libs),
        "Docs: sub/note.md",
        "a nested path is prefixed with the library name"
    );
    assert_eq!(
        compute_relative_path(Path::new("/somewhere/else/note.md"), &libs),
        "note.md",
        "a path outside every library falls back to the file name"
    );
    assert_eq!(
        compute_relative_path(Path::new("/somewhere/else/note.md"), &[]),
        "note.md",
        "with no libraries the file name is all there is"
    );
    assert_eq!(
        compute_relative_path(Path::new(""), &libs),
        "",
        "a path without a file name renders as the empty string"
    );
    assert_eq!(
        compute_relative_path(Path::new("/lib/oth/x.md"), &libs),
        "x.md",
        "the library root is matched component-wise, not as a string prefix"
    );
}

#[test]
fn test_compute_relative_path_prefers_the_first_matching_library() {
    let outer = tempfile::tempdir().expect("tempdir");
    let inner = outer.path().join("inner");
    let libs = vec![library("Outer", outer.path()), library("Inner", &inner)];

    assert_eq!(
        compute_relative_path(&inner.join("note.md"), &libs),
        "Outer: inner/note.md",
        "the first library whose root matches wins"
    );
    assert_eq!(
        compute_relative_path(&inner.join("note.md"), &libs[1..]),
        "Inner: note.md",
        "dropping the outer library lets the inner one match"
    );
}

#[test]
fn test_path_starts_with_root_is_component_wise() {
    let root = Path::new("/lib/root");

    assert!(
        path_starts_with_root(root, root),
        "the root contains itself"
    );
    assert!(path_starts_with_root(Path::new("/lib/root/sub/a.md"), root));
    assert!(!path_starts_with_root(Path::new("/lib/other/a.md"), root));
    assert!(
        !path_starts_with_root(Path::new("/lib"), root),
        "a parent of the root is not inside it"
    );
    assert!(
        !path_starts_with_root(Path::new("/lib/rootx/a.md"), root),
        "matching is component-wise, so a sibling with a similar name never matches"
    );
    assert!(
        path_starts_with_root(Path::new("a/b"), Path::new("")),
        "every path starts with the empty root"
    );
}

#[cfg(windows)]
#[test]
fn test_path_starts_with_root_ignores_case_on_windows() {
    assert!(path_starts_with_root(
        Path::new("C:\\Lib\\Sub\\a.md"),
        Path::new("c:\\lib")
    ));
    assert!(!path_starts_with_root(
        Path::new("D:\\Lib\\a.md"),
        Path::new("c:\\lib")
    ));
    assert!(!path_starts_with_root(
        Path::new("C:\\Lib"),
        Path::new("c:\\lib\\sub")
    ));
}

#[cfg(not(windows))]
#[test]
fn test_path_starts_with_root_is_case_sensitive_off_windows() {
    assert!(
        !path_starts_with_root(Path::new("/Lib/Sub/a.md"), Path::new("/lib")),
        "off Windows the comparison is byte-for-byte"
    );
}

#[test]
fn test_is_in_content_library() {
    let dir = tempfile::tempdir().expect("tempdir");
    let libs = vec![library("Docs", dir.path())];
    let inside = dir.path().join("sub").join("a.md");

    assert!(is_in_content_library(&inside, &libs));
    assert!(
        is_in_content_library(dir.path(), &libs),
        "the library root itself counts as inside"
    );
    assert!(!is_in_content_library(
        Path::new("/somewhere/else.md"),
        &libs
    ));
    assert!(
        !is_in_content_library(&inside, &[]),
        "with no libraries nothing is inside"
    );
}

#[test]
fn test_is_markdown_file_recognises_both_extensions_case_insensitively() {
    let cases = [
        ("notes.md", true),
        ("notes.MD", true),
        ("notes.markdown", true),
        ("notes.Markdown", true),
        ("notes.txt", false),
        ("notes", false),
        (".md", false),
        ("archive.tar.gz", false),
    ];

    for (name, expected) in cases {
        assert_eq!(
            is_markdown_file(Path::new(name)),
            expected,
            "is_markdown_file({name:?})"
        );
    }
}

#[test]
fn test_extract_snippet_trims_windows_and_truncates() {
    assert_eq!(
        extract_snippet("  hello world  ", "hello"),
        "hello world",
        "a short line is trimmed and returned whole"
    );

    let exactly_100 = "y".repeat(100);
    assert_eq!(
        extract_snippet(&exactly_100, "y"),
        exactly_100,
        "exactly 100 characters is still short enough to pass through"
    );

    let exactly_101 = "y".repeat(101);
    let head = extract_snippet(&exactly_101, "y");
    assert_eq!(head.chars().count(), 91, "90 characters plus the ellipsis");
    assert!(head.ends_with('…'), "the tail is elided: {head}");
    assert!(
        head.starts_with('y'),
        "the window starts at the match: {head}"
    );

    let no_term = "x".repeat(200);
    let tail = extract_snippet(&no_term, "needle");
    assert_eq!(tail.chars().count(), 91, "90 characters plus the ellipsis");
    assert!(tail.ends_with('…'), "an elided line is marked: {tail}");

    let windowed = format!("{}needle{}", "a".repeat(100), "b".repeat(100));
    let middle = extract_snippet(&windowed, "needle");
    assert!(middle.starts_with('…'), "the head is elided: {middle}");
    assert!(middle.ends_with('…'), "the tail is elided: {middle}");
    assert!(middle.contains("needle"), "the match stays visible");
}

#[test]
fn test_extract_snippet_counts_characters_not_bytes() {
    let unicode = "ä".repeat(120);
    let snippet = extract_snippet(&unicode, "ä");

    assert_eq!(
        snippet.chars().count(),
        91,
        "90 characters plus the ellipsis"
    );
    assert!(
        snippet.chars().all(|c| c == 'ä' || c == '…'),
        "the window must not split a multi-byte character: {snippet}"
    );
}

#[test]
fn test_find_search_results_sorts_by_file_name_then_relative_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let files = vec![
        write_in(dir.path(), "z_first.md", "alpha"),
        write_in(dir.path(), "a_second.md", "alpha"),
        write_in(dir.path(), "m_third.md", "alpha"),
    ];

    let results = find_search_results(&files, "alpha", &[]);
    let names: Vec<&str> = results.iter().map(|r| r.file_name.as_str()).collect();
    assert_eq!(
        names,
        vec!["a_second.md", "m_third.md", "z_first.md"],
        "results are sorted by file name"
    );

    let lib_dir = tempfile::tempdir().expect("tempdir");
    let libs = vec![library("Docs", lib_dir.path())];
    let dupes = vec![
        write_in(lib_dir.path(), "sub2/dup.md", "alpha"),
        write_in(lib_dir.path(), "sub1/dup.md", "alpha"),
    ];

    let results = find_search_results(&dupes, "alpha", &libs);
    let relatives: Vec<&str> = results.iter().map(|r| r.relative_path.as_str()).collect();
    assert_eq!(
        relatives,
        vec!["Docs: sub1/dup.md", "Docs: sub2/dup.md"],
        "identical file names are ordered by relative path"
    );
}

#[test]
fn test_find_search_results_reports_first_line_and_total_occurrences() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = write_in(
        dir.path(),
        "docs/counted.md",
        "nothing here\ntarget once\nnothing here\ntarget twice target",
    );

    let results = find_search_results(std::slice::from_ref(&file), "target", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].line_number, 2, "the first matching line wins");
    assert_eq!(results[0].match_count, 3, "every occurrence is counted");
    assert_eq!(results[0].path, file);
    assert_eq!(results[0].file_name, "counted.md");
    assert_eq!(
        results[0].relative_path, "counted.md",
        "no library means the file name"
    );
}

#[test]
fn test_find_search_results_is_empty_for_a_blank_term() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = write_in(dir.path(), "docs/a.md", "alpha");

    assert!(
        find_search_results(std::slice::from_ref(&file), "", &[]).is_empty(),
        "a blank term never matches"
    );
    assert!(
        find_search_results(&[], "alpha", &[]).is_empty(),
        "an empty file list has nothing to search"
    );
}

#[test]
fn test_find_search_results_handles_unicode_terms() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = write_in(dir.path(), "docs/uni.md", "Grüße aus München");

    let results = find_search_results(std::slice::from_ref(&file), "grüße", &[]);
    assert_eq!(results.len(), 1, "the term matches case-insensitively");
    assert_eq!(results[0].match_count, 1);
    assert_eq!(results[0].snippet, "Grüße aus München");
}

#[test]
fn test_find_search_results_scopes_to_an_empty_allow_list() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = write_in(dir.path(), "docs/a.md", "alpha");
    let allowed: HashSet<PathBuf> = HashSet::new();

    let results =
        find_search_results_scoped(std::slice::from_ref(&file), "alpha", &[], Some(&allowed));
    assert!(
        results.is_empty(),
        "an empty allow list scopes the search to nothing"
    );
}

#[test]
fn test_find_search_results_relative_path_uses_the_library() {
    let dir = tempfile::tempdir().expect("tempdir");
    let libs = vec![library("Docs", dir.path())];
    let file = write_in(dir.path(), "sub/note.md", "alpha");

    let results = find_search_results(std::slice::from_ref(&file), "alpha", &libs);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].relative_path, "Docs: sub/note.md");
}

#[test]
fn test_filter_files_by_content_returns_matching_paths_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let hit = write_in(dir.path(), "docs/a.md", "alpha beta");
    let miss = write_in(dir.path(), "docs/b.md", "nothing here");
    let text = write_in(dir.path(), "docs/c.txt", "alpha");
    let lonely = write_in(dir.path(), "other/d.txt", "alpha");

    let found = filter_files_by_content(
        &[hit.clone(), miss.clone(), text.clone(), lonely.clone()],
        "alpha",
    );
    assert_eq!(found, HashSet::from([hit]), "only matching markdown files");

    assert!(
        filter_files_by_content(&[], "alpha").is_empty(),
        "an empty file list yields no matches"
    );
}
