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

/// Concatenate the plain-text content of inline elements. Used to derive
/// the scroll-id key and the ToC title from a heading's styled elements.
pub fn heading_plain_text(elems: &[InlineElem]) -> String {
    let mut out = String::new();
    for e in elems {
        match e {
            InlineElem::Text(t, _) => out.push_str(t),
            InlineElem::Link(_, t) => out.push_str(t),
            InlineElem::Image(url) => {
                out.push_str(&format!("[Image: {}]", url));
            }
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
            // this to an `egui::Id` at render time.
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
mod tests {
    use super::*;

    #[test]
    fn test_heading_plain_text_all_variants() {
        let elems = vec![
            InlineElem::Text(
                "Heading ".to_string(),
                TextStyle {
                    bold: true,
                    ..Default::default()
                },
            ),
            InlineElem::Link("https://example.com".to_string(), "Link".to_string()),
            InlineElem::SoftBreak,
            InlineElem::Image("pic.png".to_string()),
            InlineElem::SoftBreak,
            InlineElem::Html("<span>Tag</span>".to_string()),
        ];
        let plain = heading_plain_text(&elems);
        assert_eq!(plain, "Heading Link [Image: pic.png] <span>Tag</span>");
    }

    #[test]
    fn test_toc_entry_new() {
        let e = ToCEntry::new("Intro", 1, "Intro");
        assert_eq!(e.title, "Intro");
        assert_eq!(e.level, 1);
        assert_eq!(e.id, "Intro");
    }

    #[test]
    fn test_toc_entry_display() {
        let e = ToCEntry::new("Intro", 1, "Intro");
        assert_eq!(format!("{e}"), "Intro (H1)");
    }

    #[test]
    fn test_build_toc_basic() {
        let md = "# Title\nSome text\n## Subtitle";
        let toc = build_toc(md);
        assert_eq!(toc.len(), 2);
        assert_eq!(toc[0].title, "Title");
        assert_eq!(toc[0].level, 1);
        assert_eq!(toc[0].id, "Title");
        assert_eq!(toc[1].title, "Subtitle");
        assert_eq!(toc[1].level, 2);
        assert_eq!(toc[1].id, "Subtitle");
    }

    #[test]
    fn test_build_toc_disambiguates_duplicate_headings() {
        // Two headings with the same text must get unique ids
        // so the right panel can scroll to the second one.
        let md = "# Intro\nfoo\n# Intro\nbar\n# Intro\nbaz";
        let toc = build_toc(md);
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0].id, "Intro");
        assert_eq!(toc[1].id, "Intro#1");
        assert_eq!(toc[2].id, "Intro#2");
    }
}
