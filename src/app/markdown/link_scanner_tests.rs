//! Unit tests for `link_scanner`.

use super::*;
use crate::markdown::{InlineElem, TextStyle};

#[test]
fn test_bare_https_url_recognized() {
    let style = TextStyle::default();
    let elems = scan_text_for_links("Visit https://example.com for info", &style);
    assert_eq!(elems.len(), 3);
    assert_eq!(
        elems[0],
        InlineElem::Text("Visit ".to_string(), style.clone())
    );
    assert_eq!(
        elems[1],
        InlineElem::Link(
            "https://example.com".to_string(),
            "https://example.com".to_string()
        )
    );
    assert_eq!(elems[2], InlineElem::Text(" for info".to_string(), style));
}

#[test]
fn test_bare_url_trailing_punctuation_trimmed() {
    let style = TextStyle::default();
    let cases = [
        ("https://example.com.", "https://example.com", "."),
        ("https://example.com,", "https://example.com", ","),
        ("https://example.com?", "https://example.com", "?"),
        ("https://example.com!", "https://example.com", "!"),
        ("https://example.com;", "https://example.com", ";"),
        ("https://example.com:", "https://example.com", ":"),
    ];

    for (input, expected_url, expected_punct) in cases {
        let elems = scan_text_for_links(input, &style);
        assert_eq!(elems.len(), 2, "failed for input: {input}");
        assert_eq!(
            elems[0],
            InlineElem::Link(expected_url.to_string(), expected_url.to_string())
        );
        assert_eq!(
            elems[1],
            InlineElem::Text(expected_punct.to_string(), style.clone())
        );
    }
}

#[test]
fn test_bare_url_balanced_parentheses_preserved() {
    let style = TextStyle::default();
    let input = "https://en.wikipedia.org/wiki/Rust_(programming_language)";
    let elems = scan_text_for_links(input, &style);
    assert_eq!(elems.len(), 1);
    assert_eq!(
        elems[0],
        InlineElem::Link(input.to_string(), input.to_string())
    );
}

#[test]
fn test_bare_url_enclosed_in_parentheses() {
    let style = TextStyle::default();
    let input = "(https://example.com/path)";
    let elems = scan_text_for_links(input, &style);
    assert_eq!(elems.len(), 3);
    assert_eq!(elems[0], InlineElem::Text("(".to_string(), style.clone()));
    assert_eq!(
        elems[1],
        InlineElem::Link(
            "https://example.com/path".to_string(),
            "https://example.com/path".to_string()
        )
    );
    assert_eq!(elems[2], InlineElem::Text(")".to_string(), style));
}

#[test]
fn test_www_url_normalized_to_https() {
    let style = TextStyle::default();
    let input = "Check out www.example.com/docs today";
    let elems = scan_text_for_links(input, &style);
    assert_eq!(elems.len(), 3);
    assert_eq!(
        elems[0],
        InlineElem::Text("Check out ".to_string(), style.clone())
    );
    assert_eq!(
        elems[1],
        InlineElem::Link(
            "https://www.example.com/docs".to_string(),
            "www.example.com/docs".to_string()
        )
    );
    assert_eq!(elems[2], InlineElem::Text(" today".to_string(), style));
}

#[test]
fn test_wikilink_simple_target() {
    let style = TextStyle::default();
    let input = "See [[My Note]] for details";
    let elems = scan_text_for_links(input, &style);
    assert_eq!(elems.len(), 3);
    assert_eq!(
        elems[0],
        InlineElem::Text("See ".to_string(), style.clone())
    );
    assert_eq!(
        elems[1],
        InlineElem::Link("wikilink:My Note".to_string(), "My Note".to_string())
    );
    assert_eq!(
        elems[2],
        InlineElem::Text(" for details".to_string(), style)
    );
}

#[test]
fn test_wikilink_with_custom_label() {
    let style = TextStyle::default();
    let input = "Refer to [[Goals-2024|2024 Goals]].";
    let elems = scan_text_for_links(input, &style);
    assert_eq!(elems.len(), 3);
    assert_eq!(
        elems[0],
        InlineElem::Text("Refer to ".to_string(), style.clone())
    );
    assert_eq!(
        elems[1],
        InlineElem::Link("wikilink:Goals-2024".to_string(), "2024 Goals".to_string())
    );
    assert_eq!(elems[2], InlineElem::Text(".".to_string(), style));
}

#[test]
fn test_mixed_wikilinks_and_bare_urls() {
    let style = TextStyle::default();
    let input = "Open [[Journal-2023-10-15]] or visit https://example.com/notes.";
    let elems = scan_text_for_links(input, &style);
    assert_eq!(elems.len(), 5);
    assert_eq!(
        elems[0],
        InlineElem::Text("Open ".to_string(), style.clone())
    );
    assert_eq!(
        elems[1],
        InlineElem::Link(
            "wikilink:Journal-2023-10-15".to_string(),
            "Journal-2023-10-15".to_string()
        )
    );
    assert_eq!(
        elems[2],
        InlineElem::Text(" or visit ".to_string(), style.clone())
    );
    assert_eq!(
        elems[3],
        InlineElem::Link(
            "https://example.com/notes".to_string(),
            "https://example.com/notes".to_string()
        )
    );
    assert_eq!(elems[4], InlineElem::Text(".".to_string(), style));
}

#[test]
fn test_code_style_bypasses_link_parsing() {
    let mut style = TextStyle::default();
    style.code = true;
    let input = "https://example.com and [[My Note]]";
    let elems = scan_text_for_links(input, &style);
    assert_eq!(elems.len(), 1);
    assert_eq!(elems[0], InlineElem::Text(input.to_string(), style));
}

#[test]
fn test_empty_wikilink_not_converted() {
    let style = TextStyle::default();
    let input = "Empty [[]] brackets";
    let elems = scan_text_for_links(input, &style);
    // Should preserve as text
    assert_eq!(elems.len(), 1);
    assert_eq!(elems[0], InlineElem::Text(input.to_string(), style));
}
