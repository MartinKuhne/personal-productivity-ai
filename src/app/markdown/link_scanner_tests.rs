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
    let style = TextStyle {
        code: true,
        ..Default::default()
    };
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

#[test]
fn scan_empty_text_returns_single_empty_text() {
    let style = TextStyle::default();
    let elems = scan_text_for_links("", &style);
    assert_eq!(
        elems,
        vec![InlineElem::Text(String::new(), style)],
        "the empty-input fast path still yields one (empty) Text element"
    );
}

/// An unterminated `[[` is not a link: the scanner emits the `[[` as literal
/// text and continues *after* it, so the run stays a single coalesced Text.
#[test]
fn unclosed_wikilink_is_kept_as_text() {
    let style = TextStyle::default();
    let elems = scan_text_for_links("see [[foo", &style);
    assert_eq!(
        elems,
        vec![InlineElem::Text("see [[foo".to_string(), style)],
        "an unterminated wikilink must survive verbatim"
    );
}

/// A valid-looking `[[ ... ]]` whose closing bracket is on a later line is not
/// a wikilink (wikilinks never span line breaks).
#[test]
fn wikilink_spanning_newline_is_not_converted() {
    let style = TextStyle::default();
    let input = "[[foo\nbar]]";
    let elems = scan_text_for_links(input, &style);
    assert_eq!(
        elems,
        vec![InlineElem::Text(input.to_string(), style)],
        "a wikilink must not cross a newline"
    );
}

/// `[[|label]]` has an empty target, so it is not a link even though the label
/// is present.
#[test]
fn wikilink_with_empty_target_is_not_converted() {
    let style = TextStyle::default();
    let input = "[[|only a label]]";
    let elems = scan_text_for_links(input, &style);
    assert_eq!(
        elems,
        vec![InlineElem::Text(input.to_string(), style)],
        "an empty wikilink target is not a link"
    );
}

/// `[[Target|]]` has an empty label, so the label falls back to the target.
#[test]
fn wikilink_with_empty_label_falls_back_to_target() {
    let style = TextStyle::default();
    let elems = scan_text_for_links("[[Target|]]", &style);
    assert_eq!(
        elems,
        vec![InlineElem::Link(
            "wikilink:Target".to_string(),
            "Target".to_string()
        )],
        "an empty label must fall back to the target text"
    );
}

/// Target and label are trimmed, so padded wikilinks resolve to the same note.
#[test]
fn wikilink_target_and_label_are_trimmed() {
    let style = TextStyle::default();
    let elems = scan_text_for_links("[[  Target  |  Label  ]]", &style);
    assert_eq!(
        elems,
        vec![InlineElem::Link(
            "wikilink:Target".to_string(),
            "Label".to_string()
        )],
        "padding around target and label must be trimmed"
    );
}

/// An invalid wikilink consumes only the `[[`, so the following text is still
/// scanned (here: nothing left to find) and a valid link earlier in the run is
/// unaffected.
#[test]
fn invalid_wikilink_advances_past_the_double_bracket_only() {
    let style = TextStyle::default();
    let elems = scan_text_for_links("[[a]] [[b", &style);
    assert_eq!(
        elems,
        vec![
            InlineElem::Link("wikilink:a".to_string(), "a".to_string()),
            InlineElem::Text(" [[b".to_string(), style),
        ],
        "the valid wikilink must still be recognised before the broken one"
    );
}

/// `www.` needs something that looks like a domain after it: a dot in the
/// remainder and a valid boundary before it.
#[test]
fn www_without_a_domain_shape_is_not_converted() {
    let style = TextStyle::default();
    let cases = [
        ("bare host label: www.x", "no dot after www."),
        ("bare prefix: www.", "nothing after www."),
        (
            "glued: xwww.example.com",
            "a letter before www. is not a boundary",
        ),
        (
            "glued: /www.example.com",
            "a slash before www. is not a boundary",
        ),
    ];
    for (input, case) in cases {
        let elems = scan_text_for_links(input, &style);
        assert_eq!(
            elems,
            vec![InlineElem::Text(input.to_string(), style.clone())],
            "{case}: input {input:?} must stay plain text"
        );
    }
}

/// The character before a URL candidate decides whether it is a real boundary.
#[test]
fn url_boundary_before_candidate_is_enforced() {
    let style = TextStyle::default();
    let url = "https://example.com";

    let allowed = ['(', '[', '{', '<', '"', '\'', '`', ':', ';', ',', '.', ' '];
    for prefix in allowed {
        let input = format!("{prefix}{url}");
        let elems = scan_text_for_links(&input, &style);
        assert_eq!(
            elems,
            vec![
                InlineElem::Text(prefix.to_string(), style.clone()),
                InlineElem::Link(url.to_string(), url.to_string()),
            ],
            "prefix {prefix:?} is a valid boundary and must expose the link"
        );
    }

    let rejected = ['a', 'z', '1', '9', '-', '_', '/'];
    for prefix in rejected {
        let input = format!("{prefix}{url}");
        let elems = scan_text_for_links(&input, &style);
        assert_eq!(
            elems,
            vec![InlineElem::Text(input.clone(), style.clone())],
            "prefix {prefix:?} is not a boundary: {input:?} must stay plain text"
        );
    }
}

/// A scheme with nothing after it is not a URL.
#[test]
fn bare_scheme_without_host_is_not_converted() {
    let style = TextStyle::default();
    let input = "see https:// end of line";
    let elems = scan_text_for_links(input, &style);
    assert_eq!(
        elems,
        vec![InlineElem::Text(input.to_string(), style)],
        "a scheme with an empty remainder is not a link"
    );
}

/// A scheme occurrence that is glued into a longer path segment is not a new
/// URL (the boundary check runs per scheme candidate, not just for the first).
#[test]
fn scheme_inside_a_path_is_not_a_second_url() {
    let style = TextStyle::default();
    let input = "mirror at /https://x.com";
    let elems = scan_text_for_links(input, &style);
    assert_eq!(
        elems,
        vec![InlineElem::Text(input.to_string(), style)],
        "the '/' before the scheme is not a boundary"
    );
}

/// Trailing `]` / `)` are only trimmed when unbalanced.
#[test]
fn trailing_brackets_are_trimmed_only_when_unbalanced() {
    let style = TextStyle::default();

    let unbalanced = "https://en.wikipedia.org/wiki/X]";
    let elems = scan_text_for_links(unbalanced, &style);
    assert_eq!(
        elems,
        vec![
            InlineElem::Link(
                "https://en.wikipedia.org/wiki/X".to_string(),
                "https://en.wikipedia.org/wiki/X".to_string()
            ),
            InlineElem::Text("]".to_string(), style.clone()),
        ],
        "an unmatched trailing ']' must be trimmed off the URL"
    );

    let balanced = "https://example.com/a[b]";
    let elems = scan_text_for_links(balanced, &style);
    assert_eq!(
        elems,
        vec![InlineElem::Link(balanced.to_string(), balanced.to_string())],
        "a balanced trailing ']' must stay part of the URL"
    );

    // Two closing parens, one opening: exactly one ')' is trimmed, then the
    // remaining pair is balanced and kept.
    let unbalanced_paren = "https://example.com/(a))";
    let elems = scan_text_for_links(unbalanced_paren, &style);
    assert_eq!(
        elems,
        vec![
            InlineElem::Link(
                "https://example.com/(a)".to_string(),
                "https://example.com/(a)".to_string()
            ),
            InlineElem::Text(")".to_string(), style),
        ],
        "only the surplus ')' may be trimmed"
    );
}

/// Every scheme is searched; the leftmost candidate wins regardless of the
/// order the schemes are tried in.
#[test]
fn leftmost_url_wins_across_schemes() {
    let style = TextStyle::default();
    let input = "ftp://files.example.com and https://web.example.com";
    let elems = scan_text_for_links(input, &style);
    assert_eq!(
        elems,
        vec![
            InlineElem::Link(
                "ftp://files.example.com".to_string(),
                "ftp://files.example.com".to_string()
            ),
            InlineElem::Text(" and ".to_string(), style.clone()),
            InlineElem::Link(
                "https://web.example.com".to_string(),
                "https://web.example.com".to_string()
            ),
        ],
        "the ftp:// URL starts earlier and must be emitted first"
    );
}

/// Wikilinks and URLs interleave in source order, whichever comes first.
#[test]
fn wikilink_and_url_interleave_in_source_order() {
    let style = TextStyle::default();

    let wiki_first = scan_text_for_links("[[A]] https://x.com", &style);
    assert_eq!(
        wiki_first,
        vec![
            InlineElem::Link("wikilink:A".to_string(), "A".to_string()),
            InlineElem::Text(" ".to_string(), style.clone()),
            InlineElem::Link("https://x.com".to_string(), "https://x.com".to_string()),
        ],
        "wikilink first in source order"
    );

    let url_first = scan_text_for_links("https://x.com [[A]]", &style);
    assert_eq!(
        url_first,
        vec![
            InlineElem::Link("https://x.com".to_string(), "https://x.com".to_string()),
            InlineElem::Text(" ".to_string(), style.clone()),
            InlineElem::Link("wikilink:A".to_string(), "A".to_string()),
        ],
        "url first in source order"
    );
}

/// Non-ASCII characters are counted by their UTF-8 length, so a URL with
/// non-ASCII bytes keeps its full text instead of being cut mid-codepoint.
#[test]
fn unicode_url_is_preserved() {
    let style = TextStyle::default();
    let url = "https://exämple.com/über?q=straße";
    let input = format!("siehe {url}!");
    let elems = scan_text_for_links(&input, &style);
    assert_eq!(
        elems,
        vec![
            InlineElem::Text("siehe ".to_string(), style.clone()),
            InlineElem::Link(url.to_string(), url.to_string()),
            InlineElem::Text("!".to_string(), style),
        ],
        "multi-byte characters must not truncate the URL"
    );
}
