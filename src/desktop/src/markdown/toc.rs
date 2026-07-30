//! Table of Contents (ToC) generation from Markdown AST events.

use crate::app::ToCEntry;
use crate::markdown::ast::heading_plain_text;
use crate::markdown::parser::parse_markdown_to_events;

/// Builds a Table of Contents from markdown.
pub fn build_toc(markdown_text: &str) -> Vec<ToCEntry> {
    let events = parse_markdown_to_events(markdown_text);
    let mut toc = Vec::new();
    use std::collections::HashMap;
    let mut seen: HashMap<String, usize> = HashMap::new();

    for event in events {
        if let crate::markdown::ast::RenderEvent::Heading { level, elems } = event {
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
