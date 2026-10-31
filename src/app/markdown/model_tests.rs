//! Tests for the Markdown data model helpers (`markdown::model`).

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
