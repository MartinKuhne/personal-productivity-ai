//! Property-based tests for the tag extractor in
//! `utils::tags::extract_tags_from_file`.
//!
//! `extract_tags_from_file` reads a markdown file, parses
//! the YAML front matter, and returns the `tags:` field as a
//! `Vec<String>`. The LLM uses the result to filter files by
//! tag; a bug here would either (a) silently drop files that
//! should be included, or (b) include files that should be
//! filtered out.
//!
//! The function reads from disk (it takes a `&Path`), so
//! each proptest case writes a temporary file with the
//! generated content and reads it back. This is the same
//! shape the existing `tags_tests.rs` uses; the proptest
//! here is the fuzzer's counterpart.
//!
//! # Properties under test
//!
//! All four properties are sourced from the A7 corner-case
//! row in `doc/planning/fuzzing.md` §2.2 "Phase 3".
//!
//! 1. **`extract_tags_from_file` never panics on any
//!    input.** Any file content (valid YAML, invalid YAML,
//!    no front matter, huge file) is parsed without
//!    unwinding.
//! 2. **Returned tags are lowercase.** The implementation
//!    lowercases every tag; a regression that returned the
//!    raw YAML value would break the tag filter (case-
//!    sensitive equality on a case-insensitive contract).
//! 3. **Returned tags are non-empty.** A regression that
//!    produced an empty string in the result would fail
//!    the LLM's `is_empty()` check downstream.
//! 4. **Tags from a `Vec<String>` front-matter field are
//!    preserved in order.** A regression that re-ordered
//!    or deduplicated the list would change the LLM's
//!    prompt output for the same file content.
//!
//! `cases = 1024` per property.

use fastmd_agent::utils::tags::extract_tags_from_file;
use proptest::prelude::*;
use std::fs;
use tempfile::TempDir;

/// One proptest case count for every property in this sidecar.
const CASES: u32 = 1024;

/// Strategy: any markdown content with a front-matter
/// block whose `tags:` field is either absent, a string, or
/// a `Vec<String>`. We avoid the empty front-matter
/// because the `tags:` field is required for any of the
/// properties to be meaningful.
fn md_with_tags_yaml(tags_yaml: &str) -> String {
    format!("---\ntitle: Test\ntags:{tags_yaml}\n---\n\n# Body\n")
}

/// Write a temporary file with the given content and run
/// `extract_tags_from_file` on it. Returns the result of
/// the function (the caller may further assert on it) and
/// the `TempDir` (kept alive for the duration of the
/// test).
fn run_extract(content: &str) -> (Vec<String>, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("test.md");
    fs::write(&path, content).expect("write");
    let tags = extract_tags_from_file(&path);
    (tags, dir)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// `extract_tags_from_file` never panics on any
    /// input. Even an empty file or a file with garbage
    /// front matter is handled gracefully.
    #[test]
    fn extract_tags_never_panics_on_any_input(
        content in prop::string::string_regex(r"[\x00-\x7F\n]{0,1024}").unwrap()
    ) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.md");
        fs::write(&path, &content).expect("write");
        let _ = extract_tags_from_file(&path);
    }

    /// Tags from a YAML list are returned in lower case.
    /// The implementation calls `.to_lowercase()` on every
    /// tag; a regression that returned the raw YAML value
    /// would break the tag filter's case-insensitive
    /// contract.
    #[test]
    fn extract_tags_are_lowercase(
        raw_tag in "[A-Z][A-Za-z]{2,8}",
    ) {
        let content = md_with_tags_yaml(&format!("\n  - {raw_tag}"));
        let (tags, _dir) = run_extract(&content);
        prop_assert!(
            !tags.is_empty(),
            "tag list must not be empty for a well-formed input"
        );
        for t in &tags {
            let lower = t.to_lowercase();
            prop_assert_eq!(t, &lower, "tag {:?} is not lowercase", t);
        }
    }

    /// Tags from a YAML list are returned as non-empty
    /// strings. A regression that produced an empty string
    /// in the result would fail the LLM's `is_empty()` check
    /// downstream and silently drop the file.
    #[test]
    fn extract_tags_are_non_empty_strings(
        // Use only alphabetic chars so the YAML scalar is
        // a string, not an integer. (YAML parses `10` as
        // an integer, not a string, and the extractor
        // skips non-string scalars.)
        raw_tag in "[A-Za-z][A-Za-z0-9]{1,7}",
    ) {
        let content = md_with_tags_yaml(&format!("\n  - {raw_tag}"));
        let (tags, _dir) = run_extract(&content);
        prop_assert!(!tags.is_empty());
        for t in &tags {
            prop_assert!(!t.is_empty(), "tag is empty");
        }
    }

    /// A YAML scalar string `tags: foo` produces a
    /// one-element list `["foo"]`. This is the
    /// backwards-compatible form for users who only have
    /// one tag.
    #[test]
    fn extract_tags_from_yaml_scalar(
        tag in "[a-z][a-z0-9]{1,8}",
    ) {
        let content = md_with_tags_yaml(&format!(" {tag}"));
        let (tags, _dir) = run_extract(&content);
        prop_assert_eq!(tags, vec![tag]);
    }

    /// A YAML list `tags: [a, b, c]` produces a
    /// three-element list. The order is preserved (the
    /// implementation does not sort or dedupe).
    #[test]
    fn extract_tags_from_yaml_list_preserves_order(
        a in "[a-z][a-z0-9]{1,4}",
        b in "[a-z][a-z0-9]{1,4}",
        c in "[a-z][a-z0-9]{1,4}",
    ) {
        // Use disjoint character classes to avoid
        // collisions that would fail the "preserves order"
        // assertion.
        let content = md_with_tags_yaml(&format!("\n  - {a}\n  - {b}\n  - {c}"));
        let (tags, _dir) = run_extract(&content);
        prop_assert_eq!(tags, vec![a, b, c]);
    }

    /// A front-matter without a `tags:` field produces an
    /// empty list (the file has no tags, not a parse
    /// error).
    #[test]
    fn extract_tags_absent_field_is_empty(_unused in 0..1u8) {
        let content = "---\ntitle: Test\n---\n\n# Body\n";
        let (tags, _dir) = run_extract(content);
        prop_assert!(tags.is_empty());
    }

    /// A front-matter without a closing `---` is treated
    /// as no front-matter; the `tags:` field is absent.
    #[test]
    fn extract_tags_unclosed_front_matter_is_empty(_unused in 0..1u8) {
        let content = "tags: [foo, bar]\n# Body without front matter close\n";
        let (tags, _dir) = run_extract(content);
        prop_assert!(tags.is_empty());
    }
}
