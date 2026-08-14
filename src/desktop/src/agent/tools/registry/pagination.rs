//! Paginator helper for tool results.

/// Default `limit` for `list_notes` and `list_notes_by_tag`.
pub const DEFAULT_LIST_NOTES_BY_TAG_LIMIT: usize = 100;

/// Slice `items` by the 0-indexed `offset` and `limit`. Returns the
/// slice and a human-readable hint (set when the result is empty
/// for a known reason; `None` otherwise).
///
/// The helper does NOT cap `limit` — the LLM owns the page-size
/// choice. If `limit: 100_000` is requested and the result has
/// 100_000 items, all of them are returned. The end-of-slice clamp
/// `(offset + limit).min(total)` is a bound check, not a cap.
///
/// `offset` is reported verbatim in the past-end hint (i.e. not
/// clamped) so the LLM can see what it asked for. The end-of-slice
/// calculation uses the clamped value.
pub fn paginate_in_range<T: Clone>(
    items: &[T],
    offset: usize,
    limit: usize,
    total: usize,
    plural: &str,
) -> (Vec<T>, Option<String>) {
    if total == 0 {
        return (Vec::new(), Some(format!("No matching {plural} found.")));
    }
    if offset >= total {
        return (
            Vec::new(),
            Some(format!(
                "No {plural} at offset {offset} (showing 0 of {total} total, limit: {limit})."
            )),
        );
    }
    let end = (offset + limit).min(total);
    (items[offset..end].to_vec(), None)
}
