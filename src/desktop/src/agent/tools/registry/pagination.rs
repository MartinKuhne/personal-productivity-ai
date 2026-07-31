//! Paginator helper for tool results.

pub fn paginate_in_range<T: Clone>(
    items: &[T],
    page: usize,
    page_size: usize,
    total: usize,
    plural: &str,
) -> (Vec<T>, Option<String>) {
    if total == 0 {
        return (Vec::new(), Some(format!("No matching {plural} found.")));
    }
    let start = (page - 1).saturating_mul(page_size);
    if start >= total {
        return (
            Vec::new(),
            Some(format!(
                "No {plural} on page {page} (showing 0 of {total} total, page_size: {page_size})."
            )),
        );
    }
    let end = (start + page_size).min(total);
    (items[start..end].to_vec(), None)
}
