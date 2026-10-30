//! Text primitives the audit report renders with.
//!
//! Every user-facing string in the audit is a `const` template that these
//! functions fill in, so no report wording is ever assembled from inline
//! literals at a call site and the phrasing of a finding has exactly one
//! place it can be wrong.

/// A coordinate rendered the way the probe renders it.
#[must_use]
pub fn px(value: f32) -> String {
    format!("{value:.2}")
}

/// A boolean as the report says it.
#[must_use]
pub fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

/// Substitutes `{key}` placeholders in `template`.
#[must_use]
pub fn fill(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in pairs {
        out = out.replace(&format!("{{{key}}}"), value);
    }
    out
}

/// Renders `name=count` pairs for the header totals.
#[must_use]
pub fn totals<T>(items: &[T], name: fn(T) -> &'static str, count: impl Fn(T) -> usize) -> String
where
    T: Copy,
{
    items
        .iter()
        .map(|item| format!("{}={}", name(*item), count(*item)))
        .collect::<Vec<_>>()
        .join(" ")
}
