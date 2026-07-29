//! Strongly typed Markdown AST representations and render events.

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
    CodeBlock(String),
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
}
