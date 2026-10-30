use super::build_merge_prompt;
use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn test_build_merge_prompt() {
    let libs = vec![crate::config::ContentLibrary {
        name: "Notes".to_string(),
        root_folder: "C:/notes".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];
    let mut files = HashSet::new();
    files.insert(PathBuf::from("C:/notes/a.md"));
    files.insert(PathBuf::from("C:/notes/b.md"));
    let prompt = build_merge_prompt(&libs, &files);
    assert!(prompt.contains("Consolidate"));
    assert!(prompt.contains("a.md"));
    assert!(prompt.contains("b.md"));
}

#[test]
fn test_build_merge_prompt_sorts_files() {
    let libs: Vec<crate::config::ContentLibrary> = Vec::new();
    let mut files = HashSet::new();
    files.insert(PathBuf::from("z.md"));
    files.insert(PathBuf::from("a.md"));
    let prompt = build_merge_prompt(&libs, &files);
    let a = prompt.find('a').unwrap();
    let z = prompt.find('z').unwrap();
    assert!(a < z, "files must be listed in sorted order");
}

#[test]
fn test_build_merge_prompt_empty_selection_still_has_header() {
    let libs: Vec<crate::config::ContentLibrary> = Vec::new();
    let files = HashSet::new();
    let prompt = build_merge_prompt(&libs, &files);
    assert!(prompt.contains("read_note"));
    assert!(!prompt.contains("- \n"));
}
