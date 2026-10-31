//! Markdown data model and the small operations that operate on it.
//!
//! Three things live here, all data-only or close to it:
//!
//! - [`InlineElem`], [`TextStyle`], [`RenderEvent`] — the strong-typed
//!   AST surface produced by [`crate::markdown::parser`].
//! - [`heading_plain_text`] — the small helper that derives the
//!   plain-text title and ToC scroll-id from a styled heading.
//! - [`ToCEntry`] and [`build_toc`] — the table-of-contents data type
//!   and its builder. The builder walks the parser output and emits a
//!   disambiguated list of `ToCEntry`s.
//!
//! Spec: [`markdown/SPEC.md`](../markdown/SPEC.md) (MD-001..MD-018).
//!
//! Unit tests live in the sibling `model_tests.rs` sidecar.
use crate::markdown::parser::parse_markdown_to_events;
use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InlineElem {
    Text(String, TextStyle),
    Link(String, String),
    Image(String),
    Html(String),
    SoftBreak,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Hash)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strikethrough: bool,
    pub muted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RenderEvent {
    FlushInline {
        elems: Vec<InlineElem>,
        needs_bullet: bool,
        task_checked: Option<bool>,
        indent: usize,
        /// Ordinal for ordered list items. `None` → bullet, `Some(n)` → `"n. "`.
        list_ordinal: Option<u64>,
    },
    CodeBlock {
        language: Option<String>,
        content: String,
    },
    Heading {
        level: u32,
        /// Styled inline elements that make up the heading text.
        elems: Vec<InlineElem>,
    },
    Table(Vec<Vec<Vec<InlineElem>>>),
    Space(f32),
    Separator,
}

/// Scheme prefix marking an in-app wikilink target inside an
/// [`InlineElem::Link`] URL (`wikilink:<target>` or
/// `wikilink:<target>#<anchor>`).
pub const WIKILINK_SCHEME: &str = "wikilink:";

/// Prefix of the placeholder text produced by [`image_placeholder`].
pub const IMAGE_PLACEHOLDER_PREFIX: &str = "[Image: ";

/// The placeholder text rendered in place of an [`InlineElem::Image`] whose
/// bitmap is not painted: `"[Image: <url>]"` (RUST-021 — the single source
/// shared by the plain-text derivation, the parser and every UI renderer).
pub fn image_placeholder(url: &str) -> String {
    format!("{IMAGE_PLACEHOLDER_PREFIX}{url}]")
}

/// Concatenate the plain-text content of inline elements. Used to derive
/// the scroll-id key and the ToC title from a heading's styled elements.
pub fn heading_plain_text(elems: &[InlineElem]) -> String {
    let mut out = String::new();
    for e in elems {
        match e {
            InlineElem::Text(t, _) => out.push_str(t),
            InlineElem::Link(_, t) => out.push_str(t),
            InlineElem::Image(url) => out.push_str(&image_placeholder(url)),
            InlineElem::Html(h) => out.push_str(h),
            InlineElem::SoftBreak => out.push(' '),
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToCEntry {
    /// Heading text, trimmed of markdown decoration.
    pub title: String,
    /// Heading depth (1 == H1, 2 == H2, …).
    pub level: u32,
    /// Stable string identifier for the heading.
    /// The first `"Intro"` has `id == "Intro"`; the second has
    /// `id == "Intro#1"`; the third `"Intro#2"`, etc.
    pub id: String,
}

impl ToCEntry {
    /// Construct a new entry.
    pub fn new(title: impl Into<String>, level: u32, id: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            level,
            id: id.into(),
        }
    }
}

impl fmt::Display for ToCEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (H{})", self.title, self.level)
    }
}

/// Builds a Table of Contents from markdown.
pub fn build_toc(markdown_text: &str) -> Vec<ToCEntry> {
    let events = parse_markdown_to_events(markdown_text);
    let mut toc = Vec::new();
    use std::collections::HashMap;
    let mut seen: HashMap<String, usize> = HashMap::new();

    for event in events {
        if let RenderEvent::Heading { level, elems } = event {
            let text = heading_plain_text(&elems);
            let trimmed = text.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            let occurrence = seen.entry(trimmed.clone()).or_insert(0);
            // Stable string identifier — the UI layer converts
            // this to an `Id` at render time.
            let id = if *occurrence == 0 {
                trimmed.clone()
            } else {
                format!("{}#{}", trimmed, *occurrence)
            };
            *occurrence += 1;
            toc.push(ToCEntry {
                title: trimmed,
                level,
                id,
            });
        }
    }
    toc
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
