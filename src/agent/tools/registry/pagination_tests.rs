//! Unit tests for the paginator helper (REQ: paginated tool results).
//!
//! Covers the corner cases the LLM-facing `list_notes` tool can hit:
//! empty result sets, out-of-range offsets, zero limits, exact-boundary
//! pages, and the usize-overflow guard.

use super::pagination::{paginate_in_range, DEFAULT_LIST_NOTES_BY_TAG_LIMIT};

#[test]
fn default_limit_is_100() {
    assert_eq!(DEFAULT_LIST_NOTES_BY_TAG_LIMIT, 100);
}

#[test]
fn total_zero_returns_empty_with_plural_hint() {
    let (items, hint) = paginate_in_range(&["a".to_string()], 0, 10, 0, "files");
    assert!(items.is_empty());
    let hint = hint.expect("hint should be set when total is zero");
    assert!(
        hint.contains("files"),
        "hint should mention the plural: {hint}"
    );
}

#[test]
fn offset_zero_limit_untouched_returns_full_page() {
    let data = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let (items, hint) = paginate_in_range(&data, 0, 10, data.len(), "files");
    assert_eq!(items, data);
    assert!(hint.is_none());
}

#[test]
fn offset_mid_slice_is_honoured() {
    let data = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let (items, hint) = paginate_in_range(&data, 1, 1, data.len(), "files");
    assert_eq!(items, vec!["b".to_string()]);
    assert!(hint.is_none());
}

#[test]
fn limit_zero_returns_empty_slice_with_no_hint() {
    let data = vec!["a".to_string(), "b".to_string()];
    let (items, hint) = paginate_in_range(&data, 0, 0, data.len(), "files");
    assert!(items.is_empty());
    assert!(hint.is_none());
}

#[test]
fn offset_past_end_returns_empty_with_verbatim_offset() {
    let data = vec!["a".to_string()];
    let (items, hint) = paginate_in_range(&data, 999, 10, data.len(), "files");
    assert!(items.is_empty());
    let hint = hint.expect("hint must be set for out-of-range offset");
    assert!(hint.contains("999"), "offset reported verbatim: {hint}");
}

#[test]
fn offset_equal_total_is_out_of_range() {
    let data = vec!["a".to_string(), "b".to_string()];
    let (items, hint) = paginate_in_range(&data, 2, 10, data.len(), "files");
    assert!(items.is_empty());
    assert!(hint.is_some());
}

#[test]
fn last_item_page_at_boundary() {
    let data = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let (items, hint) = paginate_in_range(&data, 2, 10, data.len(), "files");
    assert_eq!(items, vec!["c".to_string()]);
    assert!(hint.is_none());
}

#[test]
fn huge_limit_clamps_to_end() {
    let data = vec!["a".to_string(), "b".to_string()];
    let (items, hint) = paginate_in_range(&data, 1, 100_000, data.len(), "files");
    assert_eq!(items, vec!["b".to_string()]);
    assert!(hint.is_none());
}

#[test]
fn offset_plus_limit_overflow_is_clamped_not_panicked() {
    let data = vec!["a".to_string(), "b".to_string()];
    let (items, hint) = paginate_in_range(&data, 1, usize::MAX, data.len(), "files");
    assert_eq!(items, vec!["b".to_string()]);
    assert!(hint.is_none());
}

#[test]
fn plural_interpolation_uses_passed_word() {
    let data: Vec<String> = Vec::new();
    let (_items, hint) = paginate_in_range(&data, 0, 10, 0, "libraries");
    assert_eq!(hint.unwrap(), "No matching libraries found.");
}
