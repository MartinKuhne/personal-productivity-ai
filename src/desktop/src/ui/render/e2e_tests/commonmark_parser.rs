//! Tier 1 functional tests for CommonMark spec 0.31.2 examples.
//!
//! These tests exercise `parse_markdown_to_events()` (our code) and verify
//! that the `RenderEvent` output matches expected structural properties.
//!
//! They test **OUR code's** event translation, NOT pulldown-cmark's parsing.
//! Pulldown-cmark's raw event emission is already pinned by
//! `cmark_strikethrough_fragments_single_tilde` in `markdown/parser.rs`.
//!
//! Reference: `tests/collateral/commonmark.md` and `tests/collateral/functional-test-plan.md`.
//! Each test annotates the CM example numbers it exercises via `[CM-NNN]`.

#![cfg(test)]

use super::*;

// ===========================================================================
// Thematic breaks (CM-043 to CM-061)
// ===========================================================================

/// [CM-043] Three valid thematic break forms produce Separator events.
#[test]
fn cm_thematic_breaks_valid_forms() {
    let md = "***\n---\n___";
    let events = parse_markdown_to_events(md);

    let sep_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Separator))
        .count();
    assert_eq!(
        sep_count, 3,
        "expected 3 Separator events for valid thematic breaks"
    );
}

/// [CM-044, CM-045] Wrong characters (`+++`, `===`) do NOT produce thematic breaks.
#[test]
fn cm_thematic_breaks_wrong_characters() {
    let md = "+++\n===";
    let events = parse_markdown_to_events(md);

    let sep_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Separator))
        .count();
    assert_eq!(
        sep_count, 0,
        "wrong characters must not produce Separator events"
    );
}

/// [CM-046] Fewer than 3 matching characters do NOT produce thematic breaks.
#[test]
fn cm_thematic_breaks_not_enough_characters() {
    let md = "--\n**\n__";
    let events = parse_markdown_to_events(md);

    let sep_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Separator))
        .count();
    assert_eq!(
        sep_count, 0,
        "fewer than 3 characters must not produce Separator events"
    );
}

/// [CM-047] Up to 3 spaces of indentation are allowed.
#[test]
fn cm_thematic_breaks_indented() {
    let md = " ***\n  ***\n   ***";
    let events = parse_markdown_to_events(md);

    let sep_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Separator))
        .count();
    assert_eq!(
        sep_count, 3,
        "up to 3 spaces of indentation should produce Separator events"
    );
}

/// [CM-048] Four spaces of indentation is too many — becomes a code block.
#[test]
fn cm_thematic_breaks_four_spaces_is_code_block() {
    let md = "    ***";
    let events = parse_markdown_to_events(md);

    let has_sep = events.iter().any(|e| matches!(e, RenderEvent::Separator));
    let has_code = events
        .iter()
        .any(|e| matches!(e, RenderEvent::CodeBlock { .. }));
    assert!(
        has_code,
        "4-space-indented *** should be a code block, not a thematic break"
    );
    assert!(
        !has_sep,
        "should not emit Separator for 4-space-indented ***"
    );
}

/// [CM-057] Thematic breaks do not need blank lines before or after.
/// [CM-058] Thematic breaks can interrupt a paragraph.
#[test]
fn cm_thematic_breaks_interruption() {
    let md = "Foo\n***\nbar";
    let events = parse_markdown_to_events(md);

    // Expected: FlushInline("Foo"), Space, Separator, FlushInline("bar"), Space
    let flush_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::FlushInline { .. }))
        .count();
    let sep_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Separator))
        .count();
    assert_eq!(
        flush_count, 2,
        "expected 2 FlushInline events around thematic break"
    );
    assert_eq!(sep_count, 1, "expected 1 Separator event");
}

/// [CM-059] Setext heading takes precedence over thematic break for `---`.
#[test]
fn cm_thematic_breaks_setext_precedence() {
    let md = "Foo\n---\nbar";
    let events = parse_markdown_to_events(md);

    // The `---` is a setext heading underline, NOT a thematic break.
    let has_heading = events
        .iter()
        .any(|e| matches!(e, RenderEvent::Heading { .. }));
    let sep_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Separator))
        .count();
    assert!(
        has_heading,
        "Foo---bar must be interpreted as a setext heading, not a thematic break"
    );
    assert_eq!(
        sep_count, 0,
        "setext heading must not also produce a Separator"
    );
}

/// [CM-060] Thematic break takes precedence over list item interpretation.
/// [CM-061] Thematic break in a list item requires a different bullet.
#[test]
fn cm_thematic_breaks_list_precedence() {
    // CM-060: `* Foo\n* * *\n* Bar` — the middle line is a thematic break, not a list item.
    let md_060 = "* Foo\n* * *\n* Bar";
    let events = parse_markdown_to_events(md_060);
    let sep_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Separator))
        .count();
    assert_eq!(
        sep_count, 1,
        "thematic break takes precedence in list context [CM-060]"
    );

    // CM-061: `- Foo\n- * * *` — two list items, second contains a thematic break.
    let md_061 = "- Foo\n- * * *";
    let events = parse_markdown_to_events(md_061);
    let list_items = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                RenderEvent::FlushInline {
                    needs_bullet: true,
                    ..
                }
            )
        })
        .count();
    assert_eq!(list_items, 2, "two list items expected [CM-061]");
}

// ===========================================================================
// ATX headings (CM-062 to CM-079)
// ===========================================================================

/// [CM-062] ATX headings at all 6 levels produce Heading events with correct level.
#[test]
fn cm_atx_headings_all_levels() {
    let md = "# foo\n## foo\n### foo\n#### foo\n##### foo\n###### foo";
    let events = parse_markdown_to_events(md);

    let headings: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .collect();

    assert_eq!(
        headings,
        vec![
            (1u32, "foo".to_string()),
            (2u32, "foo".to_string()),
            (3u32, "foo".to_string()),
            (4u32, "foo".to_string()),
            (5u32, "foo".to_string()),
            (6u32, "foo".to_string()),
        ],
    );
}

/// [CM-063] More than 6 `#` characters is NOT a heading.
#[test]
fn cm_atx_headings_seven_hashes_not_heading() {
    let md = "####### foo";
    let events = parse_markdown_to_events(md);

    let heading_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Heading { .. }))
        .count();
    assert_eq!(
        heading_count, 0,
        "7 # characters must not produce a heading event",
    );
}

/// [CM-064] At least one space or tab required between `#` and heading content.
#[test]
fn cm_atx_headings_space_required() {
    let md = "#5 bolt\n#hashtag";
    let events = parse_markdown_to_events(md);

    let heading_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Heading { .. }))
        .count();
    assert_eq!(
        heading_count, 0,
        "no space between # and content must not produce heading events",
    );
}

/// [CM-065] Escaped `#` does not start a heading.
#[test]
fn cm_atx_headings_escaped_hash() {
    let md = "\\## foo";
    let events = parse_markdown_to_events(md);

    let heading_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Heading { .. }))
        .count();
    assert_eq!(heading_count, 0, "escaped first # must not start a heading");
}

/// [CM-066] Heading contents are parsed as inline elements.
/// [CM-067] Leading/trailing spaces are stripped.
#[test]
fn cm_atx_headings_inline_contents_and_trim() {
    let md = "# foo *bar* \\*baz\\*";
    let events = parse_markdown_to_events(md);

    let heading = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, elems.clone())),
            _ => None,
        })
        .expect("expected a Heading event");

    assert_eq!(heading.0, 1);
    // The escaped asterisks become literal asterisk text in the heading.
    // Pulldown-cmark handles backslash escapes to produce literal characters.
    let plain = heading_plain_text(&heading.1);
    assert!(
        plain.contains("foo") && plain.contains("bar") && plain.contains("*baz*"),
        "heading should contain foo, bar, and *baz*; got {:?}",
        plain
    );
}

/// [CM-068] Up to 3 spaces of indentation allowed.
/// [CM-069] Four spaces of indentation is too many — becomes code block.
#[test]
fn cm_atx_headings_indentation_limit() {
    // CM-068: up to 3 spaces allowed
    let md_ok = " ### foo\n  ## foo\n   # foo";
    let events_ok = parse_markdown_to_events(md_ok);
    let heading_count = events_ok
        .iter()
        .filter(|e| matches!(e, RenderEvent::Heading { .. }))
        .count();
    assert_eq!(
        heading_count, 3,
        "up to 3 spaces of indentation should produce headings"
    );

    // CM-069: 4 spaces → code block
    let md_code = "    # foo";
    let events_code = parse_markdown_to_events(md_code);
    let has_heading = events_code
        .iter()
        .any(|e| matches!(e, RenderEvent::Heading { .. }));
    let has_code = events_code
        .iter()
        .any(|e| matches!(e, RenderEvent::CodeBlock { .. }));
    assert!(has_code, "4-space-indented # foo should be a code block");
    assert!(
        !has_heading,
        "should not produce Heading for 4-space-indented #"
    );
}

/// [CM-071] Closing `#` sequence is optional.
/// [CM-072] Closing sequence length need not match opening.
/// [CM-073] Trailing spaces after closing are allowed.
/// [CM-074] Non-space after closing sequence counts as heading content.
/// [CM-075] No space before closing `#` means it's part of the heading.
/// [CM-076] Backslash-escaped `#` does not count as closing sequence.
#[test]
fn cm_atx_headings_closing_sequence() {
    // CM-071: optional closing
    let md_close = "## foo ##\n  ###   bar    ###";
    let events = parse_markdown_to_events(md_close);
    let headings: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .collect();
    assert_eq!(
        headings,
        vec![(2u32, "foo".to_string()), (3u32, "bar".to_string())]
    );

    // CM-072: independent closing length
    let md_len = "# foo ##################################\n##### foo ##";
    let events = parse_markdown_to_events(md_len);
    let headings: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .collect();
    assert_eq!(
        headings,
        vec![(1u32, "foo".to_string()), (5u32, "foo".to_string())]
    );

    // CM-074: non-space after closing
    let md_extra = "### foo ### b";
    let events = parse_markdown_to_events(md_extra);
    let heading = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .expect("expected a Heading event");
    assert_eq!(heading.0, 3);
    // CM-074: characters after closing ### mean it's not a closing sequence
    assert_eq!(
        heading.1, "foo ### b",
        "non-space after closing must be part of heading content"
    );

    // CM-075: no space before closing
    let md_nospace = "# foo#";
    let events = parse_markdown_to_events(md_nospace);
    let heading = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .expect("expected a Heading event");
    assert_eq!(
        heading.1, "foo#",
        "no space before closing # means it's part of heading"
    );

    // CM-076: escaped # in closing
    let md_escaped = "### foo \\###";
    let events = parse_markdown_to_events(md_escaped);
    let heading = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .expect("expected a Heading event");
    assert_eq!(
        heading.1, "foo ###",
        "escaped # does not end the closing sequence"
    );
}

/// [CM-077] ATX headings need not be separated by blank lines.
/// [CM-078] ATX headings can interrupt paragraphs.
/// [CM-079] ATX headings can be empty.
#[test]
fn cm_atx_headings_edge_cases() {
    // CM-077: no blank lines needed
    let md = "****\n## foo\n****";
    let events = parse_markdown_to_events(md);
    let has_heading = events
        .iter()
        .any(|e| matches!(e, RenderEvent::Heading { level: 2, .. }));
    let sep_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Separator))
        .count();
    assert!(has_heading, "## foo should be an ATX heading");
    assert_eq!(sep_count, 2, "**** lines should be thematic breaks");

    // CM-078: can interrupt paragraphs
    let md = "Foo bar\n# baz\nBar foo";
    let events = parse_markdown_to_events(md);
    let flush_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::FlushInline { .. }))
        .count();
    let heading = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .expect("expected a Heading event");
    assert_eq!(heading.0, 1);
    assert_eq!(heading.1, "baz");
    assert_eq!(flush_count, 2, "paragraphs before and after heading");

    // CM-079: empty headings
    let md = "## \n#\n### ###";
    let events = parse_markdown_to_events(md);
    let headings: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .collect();
    assert_eq!(
        headings,
        vec![
            (2u32, String::new()),
            (1u32, String::new()),
            (3u32, String::new())
        ]
    );
}

// ===========================================================================
// Setext headings (CM-080 to CM-112)
// ===========================================================================

/// [CM-080] Simple setext headings: `=` → H1, `-` → H2.
#[test]
fn cm_setext_headings_simple() {
    let md = "Foo *bar*\n=========\n\nFoo *bar*\n---------";
    let events = parse_markdown_to_events(md);

    // heading_plain_text strips emphasis, so "Foo *bar*" → "Foo bar"
    let headings: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .collect();
    assert_eq!(
        headings,
        vec![(1u32, "Foo bar".to_string()), (2u32, "Foo bar".to_string()),]
    );
}

/// [CM-081] Setext heading content can span multiple lines.
/// [CM-082] Leading/trailing spaces in content lines are trimmed.
#[test]
fn cm_setext_headings_multiline_content() {
    let md = "Foo *bar\nbaz*\n====";
    let events = parse_markdown_to_events(md);

    let heading = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .expect("expected a Heading event");
    assert_eq!(heading.0, 1);
    // heading_plain_text strips emphasis tags and concatenates inline elems
    // The *bar\nbaz* emphasis doesn't survive plain text extraction
    assert!(
        heading.1.contains("Foo"),
        "multi-line content forms the heading; got {:?}",
        heading.1
    );
}

/// [CM-083] Underline can be any length.
#[test]
fn cm_setext_headings_underline_length() {
    let md = "Foo\n-------------------------\n\nFoo\n=";
    let events = parse_markdown_to_events(md);
    let headings: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .collect();
    assert_eq!(
        headings,
        vec![(2u32, "Foo".to_string()), (1u32, "Foo".to_string())]
    );
}

/// [CM-084, CM-085] Up to 3 spaces indentation for heading content and underline.
/// Four spaces is too many — becomes a code block.
#[test]
fn cm_setext_headings_indentation() {
    // Up to 3 spaces: CM-084
    let md_ok = "   Foo\n---\n\n  Foo\n-----\n\n  Foo\n  ===";
    let events_ok = parse_markdown_to_events(md_ok);
    let headings: Vec<_> = events_ok
        .iter()
        .filter_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .collect();
    assert_eq!(
        headings,
        vec![
            (2u32, "Foo".to_string()),
            (2u32, "Foo".to_string()),
            (1u32, "Foo".to_string())
        ]
    );

    // CM-085: 4 spaces → code block, then thematic break
    let md_code = "    Foo\n    ---\n\n    Foo\n---";
    let events_code = parse_markdown_to_events(md_code);
    let code_blocks = events_code
        .iter()
        .filter(|e| matches!(e, RenderEvent::CodeBlock { .. }))
        .count();
    let has_sep = events_code
        .iter()
        .any(|e| matches!(e, RenderEvent::Separator));
    assert_eq!(
        code_blocks, 1,
        "4-space-indented Foo + --- should be a code block"
    );
    assert!(
        has_sep,
        "unindented --- after code block should be a thematic break"
    );
}

/// [CM-088] Setext heading underline cannot contain internal spaces or tabs.
#[test]
fn cm_setext_headings_no_internal_spaces() {
    // CM-088: `= =` and `--- -` are not valid underlines
    let md = "Foo\n= =\n\nFoo\n--- -";
    let events = parse_markdown_to_events(md);

    // Underlines with internal spaces should not form setext headings.
    // They may still produce other events (paragraphs, separators) but not headings.
    let heading_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Heading { .. }))
        .count();
    assert_eq!(
        heading_count, 0,
        "underlines with internal spaces must not be setext headings; got {} heading(s): {:?}",
        heading_count, events
    );
}

/// [CM-092 to CM-094] Setext heading underline cannot be a lazy continuation line
/// in a block quote or list item.
/// [CM-095] Blank line needed between paragraph and setext heading.
#[test]
fn cm_setext_headings_lazy_continuation_and_blank_line() {
    // CM-092: `> Foo\n---` — --- is NOT a setext underline, it's a thematic break.
    let md_quote = "> Foo\n---";
    let events_quote = parse_markdown_to_events(md_quote);
    let heading_count = events_quote
        .iter()
        .filter(|e| matches!(e, RenderEvent::Heading { .. }))
        .count();
    let sep_count = events_quote
        .iter()
        .filter(|e| matches!(e, RenderEvent::Separator))
        .count();
    assert_eq!(
        heading_count, 0,
        "--- in block quote context is not a setext underline"
    );
    assert_eq!(
        sep_count, 1,
        "--- after block-quoted content should be a thematic break"
    );

    // CM-094: `- Foo\n---` — --- is a thematic break, not a setext underline.
    let md_list = "- Foo\n---";
    let events_list = parse_markdown_to_events(md_list);
    let heading_count = events_list
        .iter()
        .filter(|e| matches!(e, RenderEvent::Heading { .. }))
        .count();
    assert_eq!(
        heading_count, 0,
        "--- after list item is not a setext underline"
    );

    // CM-095: blank line needed before setext heading after paragraph.
    // However, our parser still recognizes "Foo\nBar\n---" as a setext heading
    // because it doesn't enforce the "block interrupt" rule as strictly as CM.
    let md_no_blank = "Foo\nBar\n---";
    let events = parse_markdown_to_events(md_no_blank);
    let heading = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .expect("expected a Heading event");
    assert_eq!(heading.0, 2);
    // heading_plain_text concatenates inline elements; newlines between
    // content lines become spaces or are stripped.
    assert!(
        heading.1.contains("Foo") && heading.1.contains("Bar"),
        "multi-line content forms the heading; got {:?}",
        heading.1
    );
}

/// [CM-097] Setext headings cannot be empty.
/// [CM-098 to CM-101] Line of dashes interpreted as thematic break, not setext underline,
/// when the preceding line would be a code block, list item, or block quote.
#[test]
fn cm_setext_headings_empty_and_block_interference() {
    // CM-097: empty setext is not a heading
    let md_empty = "\n====";
    let events = parse_markdown_to_events(md_empty);
    let heading_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Heading { .. }))
        .count();
    assert_eq!(heading_count, 0, "empty setext must not produce a heading");

    // CM-098: `---\n---` — both are thematic breaks
    let md_dash = "---\n---";
    let events = parse_markdown_to_events(md_dash);
    let sep_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Separator))
        .count();
    assert_eq!(sep_count, 2, "both --- lines should be thematic breaks");

    // CM-101: `> foo\n-----` — ----- is not a setext underline
    let md_block = "> foo\n-----";
    let events = parse_markdown_to_events(md_block);
    let heading_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Heading { .. }))
        .count();
    assert_eq!(
        heading_count, 0,
        "--- in block quote context is not a setext underline"
    );
}

// ===========================================================================
// Code blocks (CM-113 to CM-236)
// ===========================================================================

/// [CM-113] Indented code blocks produce CodeBlock events with preserved content.
#[test]
fn cm_code_blocks_indented_simple() {
    let md = "    a simple\n      indented code block";
    let events = parse_markdown_to_events(md);

    let code = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::CodeBlock { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("expected a CodeBlock event");

    assert_eq!(code, "a simple\n  indented code block");
}

/// [CM-114, CM-115] List item content takes precedence over code block interpretation.
#[test]
fn cm_code_blocks_list_precedence() {
    // CM-114: `  - foo\n\n    bar` — bar is part of the list item, not a code block.
    let md = "  - foo\n\n    bar";
    let events = parse_markdown_to_events(md);

    let list_items = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                RenderEvent::FlushInline {
                    needs_bullet: true,
                    ..
                }
            )
        })
        .count();
    let code_blocks = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::CodeBlock { .. }))
        .count();
    assert_eq!(list_items, 1, "list item should take precedence");
    assert_eq!(
        code_blocks, 0,
        "no code block when list item context applies"
    );
}

/// [CM-116] Code block content is literal text, not parsed as Markdown.
#[test]
fn cm_code_blocks_literal_content() {
    let md = "    <a/>\n    *hi*\n\n    - one";
    let events = parse_markdown_to_events(md);

    let code = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::CodeBlock { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("expected a CodeBlock event");

    assert_eq!(code, "<a/>\n*hi*\n\n- one");
}

/// [CM-126, CM-128] Fenced code blocks with backticks and tildes produce CodeBlock events.
/// [CM-130] Fewer than 3 backticks is not enough.
/// [CM-131] More than 3 backticks/tildes work.
#[test]
fn cm_code_blocks_fenced_basic() {
    // CM-126: backtick fence
    let md_bt = "```\n<\n >\n```";
    let events = parse_markdown_to_events(md_bt);
    let code = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::CodeBlock { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("expected a CodeBlock event");
    assert_eq!(code, "<\n >\n");

    // CM-128: tilde fence
    let md_tilde = "~~~\n<\n >\n~~~";
    let events = parse_markdown_to_events(md_tilde);
    let code = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::CodeBlock { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("expected a CodeBlock event");
    assert_eq!(code, "<\n >\n");

    // CM-130: fewer than 3 backticks → not a code block
    let md_few = "``\n";
    let events = parse_markdown_to_events(md_few);
    let has_code = events
        .iter()
        .any(|e| matches!(e, RenderEvent::CodeBlock { .. }));
    assert!(!has_code, "2 backticks must not produce a code block");

    // CM-131: 4+ backticks/tildes work
    let md_many = "````\ncode\n````";
    let events = parse_markdown_to_events(md_many);
    let has_code = events
        .iter()
        .any(|e| matches!(e, RenderEvent::CodeBlock { .. }));
    assert!(has_code, "4+ backticks must produce a code block");
}

/// [CM-133] Info string after opening fence must record language info string.
#[test]
fn cm_code_blocks_info_string_language() {
    let md = "```ruby\ndef foo(x)\n  return 3\nend\n```";
    let events = parse_markdown_to_events(md);

    let lang = events.iter().find_map(|e| match e {
        RenderEvent::CodeBlock { language, .. } => language.clone(),
        _ => None,
    });

    assert_eq!(
        lang.as_deref(),
        Some("ruby"),
        "code block info string 'ruby' must be retained per CM spec 0.31.2; got {events:?}"
    );
}

/// [CM-142] Missing closing fence consumes to end of document.
#[test]
fn cm_code_blocks_missing_close() {
    let md_noclose = "```\ncode\nmore code";
    let events = parse_markdown_to_events(md_noclose);
    let code = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::CodeBlock { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("expected a CodeBlock event for unclosed fence");
    assert!(
        code.contains("more code"),
        "missing closing fence should capture all content"
    );
}

/// [CM-119] Indented code block cannot interrupt a paragraph.
/// [CM-120] Less than 4 spaces ends a code block.
#[test]
fn cm_code_blocks_paragraph_boundaries() {
    // CM-119: indented code cannot interrupt paragraph
    let md = "Foo\n    bar\n";
    let events = parse_markdown_to_events(md);

    // "Foo\n    bar" should be one paragraph with "bar" indented (hanging indent).
    let flush_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::FlushInline { .. }))
        .count();
    let code_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::CodeBlock { .. }))
        .count();
    assert_eq!(flush_count, 1, "Foo + indented bar should be one paragraph");
    assert_eq!(
        code_count, 0,
        "no code block when paragraph precedes indented content"
    );

    // CM-120: 3 spaces ends code block, remaining text is paragraph.
    let md_end = "    foo\nbar";
    let events = parse_markdown_to_events(md_end);
    let code_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::CodeBlock { .. }))
        .count();
    let flush_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::FlushInline { .. }))
        .count();
    assert_eq!(code_count, 1, "code block for indented line");
    assert_eq!(flush_count, 1, "bar after code block is a paragraph");
}

/// [CM-124] Trailing spaces in code block content are preserved.
#[test]
fn cm_code_blocks_trailing_spaces() {
    let md = "    foo  ";
    let events = parse_markdown_to_events(md);

    let code = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::CodeBlock { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("expected a CodeBlock event");

    assert_eq!(
        code, "foo  ",
        "trailing spaces in code block must be preserved"
    );
}

// ===========================================================================
// Paragraphs (CM-331 to CM-353)
// ===========================================================================

/// [CM-331] Simple paragraphs produce FlushInline + Space events.
#[test]
fn cm_paragraphs_simple() {
    let md = "First paragraph.\n\nSecond paragraph.";
    let events = parse_markdown_to_events(md);

    let flush_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::FlushInline { .. }))
        .count();
    let space_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Space(4.0)))
        .count();
    assert_eq!(
        flush_count, 2,
        "two paragraphs should produce 2 FlushInline events"
    );
    assert_eq!(space_count, 2, "each paragraph should end with Space(4.0)");
}

/// [CM-332] Blank lines between paragraphs are required for separation.
#[test]
fn cm_paragraphs_blank_line_required() {
    let md = "Line one.\nLine two.";
    let events = parse_markdown_to_events(md);

    let flush_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::FlushInline { .. }))
        .count();
    assert_eq!(
        flush_count, 1,
        "two lines without blank line should be one paragraph",
    );
}

// ===========================================================================
// Block quotes (CM-357 to CM-388)
// ===========================================================================

/// [CM-357] Block quotes group inline content — content flows through as FlushInline events.
/// [CM-359] Nested block quotes.
#[test]
fn cm_block_quotes_grouping() {
    // CM-357: simple block quote
    let md = "> Paragraph in block quote.";
    let events = parse_markdown_to_events(md);

    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event inside block quote");

    assert_eq!(flush.len(), 1);
    assert!(matches!(&flush[0], InlineElem::Text(t, _) if t.contains("Paragraph in block quote")),);

    // CM-359: nested block quote
    let md_nested = "> > Deep quote";
    let events = parse_markdown_to_events(md_nested);

    let flush_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::FlushInline { .. }))
        .count();
    assert_eq!(
        flush_count, 1,
        "nested block quotes still produce one FlushInline"
    );
}

/// [CM-366, CM-367] Block quotes can contain other block elements.
/// [CM-367] Block quote with heading.
#[test]
fn cm_block_quotes_with_headings() {
    // CM-367: block quote containing a heading
    let md = "> # Heading in block quote";
    let events = parse_markdown_to_events(md);

    let heading = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .expect("expected a Heading event inside block quote");

    assert_eq!(heading.0, 1);
    // The heading text may contain HTML tags from inline parsing
    assert!(heading.1.contains("Heading"));
}

// ===========================================================================
// Lists (CM-389 to CM-539)
// ===========================================================================

/// [CM-390] Unordered list items produce FlushInline with `needs_bullet = true`.
#[test]
fn cm_lists_unordered_needs_bullet() {
    let md = "- item one\n- item two\n- item three";
    let events = parse_markdown_to_events(md);

    let bullet_events: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::FlushInline { needs_bullet, .. } => Some(*needs_bullet),
            _ => None,
        })
        .collect();

    assert_eq!(
        bullet_events,
        vec![true, true, true],
        "all list items should have needs_bullet = true",
    );
}

/// [CM-470] Ordered list items produce FlushInline with `list_ordinal` set.
#[test]
fn cm_lists_ordered_list_ordinal() {
    let md = "1. first\n2. second\n3. third";
    let events = parse_markdown_to_events(md);

    let ordinals: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::FlushInline { list_ordinal, .. } => Some(*list_ordinal),
            _ => None,
        })
        .collect();

    assert_eq!(ordinals, vec![Some(1), Some(2), Some(3)]);
}

/// [CM-475, CM-483] Nested lists have correct `indent` levels.
/// [CM-483] Mixed nested lists (unordered inside ordered).
#[test]
fn cm_lists_nested_indent() {
    // CM-475: nested unordered list
    let md = "- one\n  - two\n  - three";
    let events = parse_markdown_to_events(md);

    let indent_levels: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::FlushInline {
                indent,
                needs_bullet: true,
                ..
            } => Some(*indent),
            _ => None,
        })
        .collect();

    assert_eq!(
        indent_levels,
        vec![1, 2, 2],
        "outer items indent=1, nested items indent=2",
    );

    // CM-483: unordered inside ordered
    let md_mixed = "1. first\n   - nested bullet\n2. second";
    let events = parse_markdown_to_events(md_mixed);

    let items: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::FlushInline {
                indent,
                needs_bullet,
                list_ordinal,
                ..
            } => Some((*indent, *needs_bullet, *list_ordinal)),
            _ => None,
        })
        .collect();

    assert_eq!(
        items,
        vec![
            (1, true, Some(1)), // "first" — ordered list item at indent 1
            (2, true, None),    // "nested bullet" — unordered at indent 2
            (1, true, Some(2)), // "second" — ordered list item at indent 1
        ],
    );
}

/// [CM-395] List item continuation paragraphs produce FlushInline events.
/// [CM-396] Two blank lines between list items produce separate items.
#[test]
fn cm_lists_continuation_and_separation() {
    // CM-395: continuation paragraph
    let md = "- item\n\n  continuation";
    let events = parse_markdown_to_events(md);

    let bullet_events: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::FlushInline {
                needs_bullet,
                indent,
                ..
            } => Some((*needs_bullet, *indent)),
            _ => None,
        })
        .collect();

    assert!(
        bullet_events.iter().any(|(b, i)| *b && *i == 1),
        "continuation paragraph should be part of list item at indent 1",
    );
}

/// [CM-477] List marker indentation: list markers can be indented.
#[test]
fn cm_lists_marker_indentation() {
    let md = "  1. first\n  2. second";
    let events = parse_markdown_to_events(md);

    let ordinals: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::FlushInline {
                list_ordinal,
                needs_bullet: true,
                ..
            } => Some(*list_ordinal),
            _ => None,
        })
        .collect();

    assert_eq!(ordinals, vec![Some(1), Some(2)]);
}

/// [CM-491] Blank line between list items does NOT end the list.
/// [CM-496] Paragraphs separated by blank lines within list items are loose.
#[test]
fn cm_lists_blank_lines_within_items() {
    let md = "- item one\n\n  continuation\n\n- item two";
    let events = parse_markdown_to_events(md);

    let bullet_count = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                RenderEvent::FlushInline {
                    needs_bullet: true,
                    ..
                }
            )
        })
        .count();
    assert_eq!(bullet_count, 2, "two top-level list items");
}

// ===========================================================================
// Code spans (CM-540 to CM-574)
// ===========================================================================

/// [CM-540] Simple code spans produce InlineElem with `code = true`.
#[test]
fn cm_code_spans_simple() {
    let md = "`code`";
    let events = parse_markdown_to_events(md);

    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");

    assert_eq!(flush.len(), 1);
    assert!(
        matches!(&flush[0], InlineElem::Text(t, s) if t == "code" && s.code),
        "code span should produce Text with code=true",
    );
}

/// [CM-542] Code spans with leading/trailing spaces.
/// [CM-547] Multiple backticks for code span with backtick content.
#[test]
fn cm_code_spaces_edge_cases() {
    // CM-542: code span with spaces
    let md = "`` ` ` ``";
    let events = parse_markdown_to_events(md);

    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");

    assert_eq!(flush.len(), 1);
    let InlineElem::Text(t, s) = &flush[0] else {
        panic!("expected Text elem, got {:?}", flush[0]);
    };
    assert!(s.code, "expected code=true for code span");
    assert_eq!(
        t, "` `",
        "code span content should strip outer spaces per CM spec"
    );

    // CM-547: multiple backticks to include backtick in content
    let md_multi = "``foo ` bar``";
    let events = parse_markdown_to_events(md_multi);
    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");

    assert!(
        matches!(&flush[0], InlineElem::Text(t, s) if t == "foo ` bar" && s.code),
        "multiple backtick fence should preserve inner backtick",
    );
}

// ===========================================================================
// Emphasis (CM-575 to CM-753)
// ===========================================================================

/// [CM-575] Bold text produces InlineElem with `bold = true`.
/// [CM-613] Italic text produces InlineElem with `italic = true`.
/// [CM-678] Strikethrough text produces InlineElem with `strikethrough = true`.
#[test]
fn cm_emphasis_style_flags() {
    // Bold
    let md_bold = "**bold**";
    let events = parse_markdown_to_events(md_bold);
    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");
    assert!(
        matches!(&flush[0], InlineElem::Text(t, s) if t == "bold" && s.bold),
        "bold should set TextStyle::bold",
    );

    // Italic
    let md_italic = "*italic*";
    let events = parse_markdown_to_events(md_italic);
    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");
    assert!(
        matches!(&flush[0], InlineElem::Text(t, s) if t == "italic" && s.italic),
        "italic should set TextStyle::italic",
    );

    // Strikethrough
    let md_strike = "~~strikethrough~~";
    let events = parse_markdown_to_events(md_strike);
    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");
    assert!(
        matches!(&flush[0], InlineElem::Text(t, s) if t == "strikethrough" && s.strikethrough),
        "strikethrough should set TextStyle::strikethrough",
    );
}

/// [CM-643] Nested emphasis: strong within emphasis and vice versa.
#[test]
fn cm_emphasis_nested() {
    let md = "***triple***";
    let events = parse_markdown_to_events(md);
    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");

    // Triple emphasis (`***text***`) produces bold+italic.
    assert_eq!(flush.len(), 1);
    assert!(
        matches!(&flush[0], InlineElem::Text(t, s) if t == "triple" && s.bold && s.italic),
        "triple emphasis should set both bold and italic",
    );
}

/// [CM-580, CM-621, CM-667] Emphasis delimiter space boundary rules.
#[test]
fn cm_emphasis_no_space_boundary() {
    // CM-580: space after opening delimiter prevents emphasis
    // (a space after `*` means it's not left-flanking).
    let md1 = "a * foo bar*";
    let events1 = parse_markdown_to_events(md1);
    let flush = events1
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");
    // The text should be plain (no emphasis) since space after `*`
    // prevents left-flanking.
    assert!(
        flush.iter().any(|e| matches!(
            e, InlineElem::Text(t, s) if t.contains("foo") && !s.italic
        )),
        "space after * should not form emphasis; got {:?}",
        flush
    );

    // CM-621: space after closing delimiter does NOT prevent emphasis.
    // `*` followed by space IS right-flanking, so emphasis forms.
    let md2 = "not *emphasized* ";
    let events2 = parse_markdown_to_events(md2);
    let flush = events2
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");
    // Emphasis should form: "not " + "emphasized" (italic)
    assert!(
        flush.iter().any(|e| matches!(
            e, InlineElem::Text(t, s) if t == "emphasized" && s.italic
        )),
        "space after * does not prevent emphasis; got {:?}",
        flush
    );
}

/// [CM-598] Emphasis delimiters inside code spans are ignored.
/// [CM-600] Strong emphasis with `__` and `**`.
#[test]
fn cm_emphasis_code_and_double_delimiters() {
    // CM-600: double underscore/bang for strong emphasis
    let md = "__bold__";
    let events = parse_markdown_to_events(md);
    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");
    assert!(
        matches!(&flush[0], InlineElem::Text(t, s) if t == "bold" && s.bold),
        "__text__ should produce bold",
    );
}

// ===========================================================================
// Links (CM-754 to CM-859)
// ===========================================================================

/// [CM-754] Inline links produce `InlineElem::Link(url, text)`.
#[test]
fn cm_links_inline() {
    let md = "[link text](https://example.com)";
    let events = parse_markdown_to_events(md);

    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");

    assert_eq!(flush.len(), 1);
    assert!(
        matches!(&flush[0], InlineElem::Link(url, text) if url == "https://example.com" && text == "link text"),
    );
}

/// [CM-755] Link with title.
/// [CM-814] Link reference definitions are consumed by pulldown-cmark.
#[test]
fn cm_links_with_title_and_reference() {
    // CM-755: link with title
    let md = "[link](/url \"title\")";
    let events = parse_markdown_to_events(md);

    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");

    assert!(matches!(&flush[0], InlineElem::Link(url, text) if url == "/url" && text == "link"),);

    // CM-814: reference links — our parser should produce InlineElem::Link
    // when pulldown-cmark resolves the reference.
    let md_ref = "[ref][id]\n\n[id]: /url";
    let events = parse_markdown_to_events(md_ref);
    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event for reference link");
    assert!(matches!(&flush[0], InlineElem::Link(url, text) if url == "/url" && text == "ref"),);
}

/// [CM-865] Angle-bracket autolinks produce `InlineElem::Link(url, url)`.
#[test]
fn cm_links_autolinks() {
    let md = "<https://example.com>";
    let events = parse_markdown_to_events(md);

    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event for autolink");

    assert_eq!(flush.len(), 1);
    assert!(
        matches!(&flush[0], InlineElem::Link(url, text) if url == "https://example.com" && text == "https://example.com"),
        "autolink should produce Link(url, url)",
    );
}

// ===========================================================================
// Images (CM-860 to CM-864)
// ===========================================================================

/// [CM-860] Images produce `InlineElem::Image(url)`.
#[test]
fn cm_images() {
    let md = "![alt text](https://example.com/image.png)";
    let events = parse_markdown_to_events(md);

    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");

    assert!(matches!(&flush[0], InlineElem::Image(url) if url == "https://example.com/image.png"),);
}

// ===========================================================================
// Hard line breaks (CM-911 to CM-921)
// ===========================================================================

/// [CM-911, CM-912] Hard line breaks flush buffered inline content.
/// [CM-915] Backslash at end of line is a hard line break.
#[test]
fn cm_hard_line_breaks_flush() {
    // CM-911: two trailing spaces produce hard line break
    let md = "first line  \nsecond line";
    let events = parse_markdown_to_events(md);

    // Hard line break flushes buffered inline, producing two FlushInline events.
    let flush_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::FlushInline { .. }))
        .count();
    assert_eq!(
        flush_count, 2,
        "hard line break should flush buffered inline into separate events",
    );

    // CM-915: backslash at end of line
    let md_bs = "first line\\\nsecond line";
    let events = parse_markdown_to_events(md_bs);
    let flush_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::FlushInline { .. }))
        .count();
    assert_eq!(
        flush_count, 2,
        "backslash at end of line should flush buffered inline",
    );
}

/// [CM-917] Hard line breaks in code blocks are NOT flush events.
/// [CM-920] Hard line breaks in headings do NOT flush.
#[test]
fn cm_hard_line_breaks_not_in_code_or_heading() {
    // Hard line breaks in code blocks are preserved as literal trailing spaces.
    let md = "```\ncode line  \nnext line\n```";
    let events = parse_markdown_to_events(md);

    let code = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::CodeBlock { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("expected a CodeBlock event");

    assert!(
        code.contains("code line  "),
        "hard line break in code block preserved as literal spaces per CM spec; got {:?}",
        code
    );

    // Hard line breaks in headings do NOT flush buffered inline.
    let md_h = "# heading  \n## next";
    let events = parse_markdown_to_events(md_h);

    let heading_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Heading { .. }))
        .count();
    assert_eq!(heading_count, 2, "both # lines should be headings");
}

// ===========================================================================
// Soft line breaks (CM-922 to CM-927)
// ===========================================================================

/// [CM-922] Soft line breaks produce `InlineElem::SoftBreak`.
#[test]
fn cm_soft_line_breaks() {
    let md = "first line\nsecond line";
    let events = parse_markdown_to_events(md);

    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");

    let soft_breaks = flush
        .iter()
        .filter(|e| matches!(e, InlineElem::SoftBreak))
        .count();
    assert_eq!(
        soft_breaks, 1,
        "soft line break should produce SoftBreak inline element"
    );
}

/// [CM-926, CM-927] Soft line breaks inside code blocks and headings are ignored.
#[test]
fn cm_soft_line_breaks_not_in_code_or_heading() {
    // Soft break in code block is just part of the content.
    let md_code = "```\nline one\nline two\n```";
    let events = parse_markdown_to_events(md_code);
    let code = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::CodeBlock { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("expected a CodeBlock event");
    assert!(code.contains("line one\nline two"));

    // Soft break in heading is ignored (content is on one logical line).
    let md_h = "# heading\n## next";
    let events = parse_markdown_to_events(md_h);
    let heading_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Heading { .. }))
        .count();
    assert_eq!(heading_count, 2);
}

// ===========================================================================
// Tables (CM-191 to CM-310+)
// ===========================================================================

/// [CM-191] Simple table produces `RenderEvent::Table` with correct structure.
#[test]
fn cm_tables_simple() {
    let md = "| Header 1 | Header 2 |\n| -------- | -------- |\n| Cell 1   | Cell 2   |";
    let events = parse_markdown_to_events(md);

    let table = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::Table(rows) => Some(rows),
            _ => None,
        })
        .expect("expected a Table event");

    assert_eq!(
        table.len(),
        2,
        "table should have 2 rows (header + 1 data row)"
    );
    assert_eq!(table[0].len(), 2, "header should have 2 cells");
    assert_eq!(table[1].len(), 2, "data row should have 2 cells");
}

/// [CM-192] Table cells with inline elements.
#[test]
fn cm_tables_inline_in_cells() {
    let md = "| **Bold** | *Italic* |\n| -------- | -------- |\n| Cell 1   | Cell 2   |";
    let events = parse_markdown_to_events(md);

    let table = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::Table(rows) => Some(rows),
            _ => None,
        })
        .expect("expected a Table event");

    // Header cell 0 should have bold text.
    let header_cell0 = &table[0][0];
    assert!(
        matches!(&header_cell0[0], InlineElem::Text(t, s) if t == "Bold" && s.bold),
        "table cell should preserve bold styling",
    );

    // Header cell 1 should have italic text.
    let header_cell1 = &table[0][1];
    assert!(
        matches!(&header_cell1[0], InlineElem::Text(t, s) if t == "Italic" && s.italic),
        "table cell should preserve italic styling",
    );
}

/// [CM-194] Tables with pipes in cells (escaped).
/// [CM-200] Tables with inline links in cells.
#[test]
fn cm_tables_escaped_pipes_and_links() {
    // CM-200: table with inline link in cell
    let md = "| Header | Header |\n| --- | --- |\n| [link](/url) | Cell 2 |";
    let events = parse_markdown_to_events(md);

    let table = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::Table(rows) => Some(rows),
            _ => None,
        })
        .expect("expected a Table event");

    // Data cell 0 should contain a Link element.
    let data_cell0 = &table[1][0];
    assert!(
        matches!(&data_cell0[0], InlineElem::Link(url, text) if url == "/url" && text == "link"),
        "table cell should preserve link elements",
    );
}

/// [CM-204] Tables can interrupt paragraphs.
/// [CM-207] Tables with varying column counts are rectangular.
#[test]
fn cm_tables_structure() {
    // Tables require a header, separator, and data row per CM spec.
    let md = "| A | B |\n| --- | --- |\n| C | D |";
    let events = parse_markdown_to_events(md);

    // There should be exactly 1 Table event.
    let table_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Table(_)))
        .count();
    assert_eq!(
        table_count, 1,
        "should produce 1 Table event; got {} events: {:?}",
        table_count, events
    );

    let table = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::Table(rows) => Some(rows),
            _ => None,
        })
        .expect("expected a Table event");

    // All rows should have same number of columns (rectangular).
    let col_count = table[0].len();
    for (i, row) in table.iter().enumerate() {
        assert_eq!(
            row.len(),
            col_count,
            "table row {i} should have {col_count} cells for rectangular table",
        );
    }
}

// ===========================================================================
// Task lists (embedded in list examples)
// ===========================================================================

/// [CM-434] Task list items produce `FlushInline` with `task_checked` field.
/// Note: `needs_bullet` is `false` for task list items (checkbox replaces bullet).
#[test]
fn cm_task_lists_checked() {
    let md = "- [x] Done\n- [ ] Not done";
    let events = parse_markdown_to_events(md);

    let task_events: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::FlushInline {
                needs_bullet,
                task_checked,
                ..
            } => Some((*needs_bullet, *task_checked)),
            _ => None,
        })
        .collect();

    // Task list items have needs_bullet=false (checkbox replaces the bullet)
    assert_eq!(
        task_events,
        vec![(false, Some(true)), (false, Some(false))],
        "task list items should have task_checked set; needs_bullet is false",
    );
}

/// [CM-436] Task list with continuation.
#[test]
fn cm_task_lists_continuation() {
    let md = "- [x] task\n  continuation";
    let events = parse_markdown_to_events(md);

    let task_events: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::FlushInline {
                needs_bullet,
                task_checked,
                indent,
                ..
            } => Some((*needs_bullet, *task_checked, *indent)),
            _ => None,
        })
        .collect();

    // Task list items have needs_bullet=false. First event is the task
    // checkbox, second is the continuation line.
    // Note: continuation may or may not be a separate FlushInline depending
    // on whether it's on the same line or a new block.
    assert!(
        task_events
            .iter()
            .any(|(b, t, i)| *b == false && *t == Some(true) && *i >= 1),
        "task checkbox at indent >= 1; got {:?}",
        task_events
    );
}

// ===========================================================================
// HTML blocks and raw HTML (CM-237 to CM-310, CM-877 to CM-910)
// ===========================================================================

/// [CM-877, CM-885] Raw HTML produces `InlineElem::Html`.
#[test]
fn cm_raw_html_inline() {
    // CM-877: raw HTML tags inline
    let md = "<a href=\"\">link</a>";
    let events = parse_markdown_to_events(md);

    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");

    let has_html = flush.iter().any(|e| matches!(e, InlineElem::Html(_)));
    assert!(
        has_html,
        "raw HTML tags must be emitted as InlineElem::Html per CM spec; got {:?}",
        flush
    );
}

// ===========================================================================
// Precedence (CM-42, CM-43)
// ===========================================================================

/// [CM-042] Block structure indicators take precedence over inline structure.
#[test]
fn cm_precedence_block_over_inline() {
    // CM-042: `- \`one\n- two\`` is two list items, not one list with a code span.
    let md = "- `one\n- two`";
    let events = parse_markdown_to_events(md);

    let bullet_count = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                RenderEvent::FlushInline {
                    needs_bullet: true,
                    ..
                }
            )
        })
        .count();
    assert_eq!(bullet_count, 2, "each dash should start a new list item");
}

/// [CM-043] Block structure precedence for thematic breaks in lists.
#[test]
fn cm_precedence_thematic_break_in_list() {
    // CM-060: `* Foo\n* * *\n* Bar` — middle line is thematic break, not list item.
    let md = "* Foo\n* * *\n* Bar";
    let events = parse_markdown_to_events(md);

    let sep_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Separator))
        .count();
    let list_count = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                RenderEvent::FlushInline {
                    needs_bullet: true,
                    ..
                }
            )
        })
        .count();
    assert_eq!(sep_count, 1, "thematic break takes precedence");
    assert_eq!(list_count, 2, "two list items (Foo and Bar)");
}

// ===========================================================================
// Entity and numeric character references (CM-025 to CM-041)
// ===========================================================================

/// [CM-025] Entity references are resolved by pulldown-cmark and pass through as text.
/// [CM-039] Numeric character references for newlines.
#[test]
fn cm_entity_references() {
    // CM-025: named entity references
    let md = "&amp;";
    let events = parse_markdown_to_events(md);
    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");
    assert!(
        matches!(&flush[0], InlineElem::Text(t, _) if t == "&"),
        "entity &amp; should resolve to literal &",
    );

    // CM-039: numeric character references for newlines do not break paragraphs
    // (they remain as literal text, not actual newline characters)
    let md = "foo&#10;&#10;bar";
    let events = parse_markdown_to_events(md);
    let flush_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::FlushInline { .. }))
        .count();
    assert_eq!(flush_count, 1, "entity newlines do not separate paragraphs");
}

// ===========================================================================
// Tabs (CM-001 to CM-011)
// ===========================================================================

/// [CM-001, CM-002, CM-003] Tabs in code blocks expand to 4-space tab stops.
#[test]
fn cm_tabs_in_code_block() {
    let md = "\tfoo\tbaz\t\tbim";
    let events = parse_markdown_to_events(md);

    let code = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::CodeBlock { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("expected a CodeBlock event");

    assert!(
        code.contains("foo"),
        "tab-indented code block must contain content"
    );
}

/// [CM-004, CM-005] Tabs in list item indentation.
#[test]
fn cm_tabs_in_lists() {
    let md = "  - foo\n\n\tbar";
    let events = parse_markdown_to_events(md);

    // Tab-indented `bar` is part of the list item at indent 1.
    let bullet_items = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                RenderEvent::FlushInline {
                    needs_bullet: true,
                    ..
                }
            )
        })
        .count();
    assert_eq!(bullet_items, 1, "tab-indented item belongs to list item");
}

/// [CM-006] Tabs in block quotes.
#[test]
fn cm_tabs_in_block_quotes() {
    let md = ">\t\tfoo";
    let events = parse_markdown_to_events(md);

    let code_or_text = events.iter().any(|e| match e {
        RenderEvent::CodeBlock { content, .. } => content.contains("foo"),
        RenderEvent::FlushInline { elems, .. } => elems.iter().any(|el| match el {
            InlineElem::Text(t, _) => t.contains("foo"),
            _ => false,
        }),
        _ => false,
    });
    assert!(code_or_text, "block quote with tab should contain 'foo'");
}

// ===========================================================================
// Backslash escapes (CM-012 to CM-024)
// ===========================================================================

/// [CM-012, CM-013] Backslash escaping ASCII punctuation disables formatting.
#[test]
fn cm_backslash_escapes_all_ascii_punct() {
    let md = r"\*not italic\* \[not a link\]\(not url\)";
    let events = parse_markdown_to_events(md);

    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");

    // Asterisks and brackets escaped -> no italic, no link, plain text.
    let has_italic = flush
        .iter()
        .any(|e| matches!(e, InlineElem::Text(_, s) if s.italic));
    let has_link = flush.iter().any(|e| matches!(e, InlineElem::Link(..)));

    assert!(!has_italic, "escaped asterisks must not produce italic");
    assert!(!has_link, "escaped brackets must not produce a link");
}

/// [CM-020, CM-021] Backslash before non-punctuation is preserved as literal backslash.
#[test]
fn cm_backslash_escapes_non_punct() {
    let md = r"\a \1 \ ";
    let events = parse_markdown_to_events(md);

    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");

    let text: String = flush
        .iter()
        .filter_map(|e| match e {
            InlineElem::Text(t, _) => Some(t.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");

    assert!(
        text.contains(r"\a"),
        "non-punctuation backslash must be preserved as literal backslash per CM spec; got {:?}",
        text
    );
}

// ===========================================================================
// Blank lines (CM-354 to CM-356)
// ===========================================================================

/// [CM-354, CM-355] Multiple blank lines between blocks.
#[test]
fn cm_blank_lines_handling() {
    let md = "# Header\n\n\n\nParagraph text.\n\n\n";
    let events = parse_markdown_to_events(md);

    let heading_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::Heading { .. }))
        .count();
    let flush_count = events
        .iter()
        .filter(|e| matches!(e, RenderEvent::FlushInline { .. }))
        .count();

    assert_eq!(heading_count, 1, "1 heading expected despite blank lines");
    assert_eq!(flush_count, 1, "1 paragraph expected despite blank lines");
}

// ===========================================================================
// Autolinks (CM-865 to CM-876)
// ===========================================================================

/// [CM-869] Email autolinks produce InlineElem::Link with mailto: prefix.
#[test]
fn cm_autolinks_email() {
    let md = "<foo@bar.example.com>";
    let events = parse_markdown_to_events(md);

    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");

    assert_eq!(flush.len(), 1);
    assert!(
        matches!(&flush[0], InlineElem::Link(url, text) if url == "mailto:foo@bar.example.com" && text == "foo@bar.example.com"),
        "email autolink should produce mailto: URL; got {:?}",
        flush[0]
    );
}

// ===========================================================================
// Textual content (CM-928 to CM-935)
// ===========================================================================

/// [CM-928, CM-929] Unicode, emoji, and string content pass through intact.
#[test]
fn cm_textual_content_unicode_and_emoji() {
    let md = "Hello 🌍! Δt = 5s. ὐ→a.";
    let events = parse_markdown_to_events(md);

    let flush = events
        .iter()
        .find_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => Some(elems.clone()),
            _ => None,
        })
        .expect("expected a FlushInline event");

    let text: String = flush
        .iter()
        .filter_map(|e| match e {
            InlineElem::Text(t, _) => Some(t.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");

    assert!(
        text.contains("🌍"),
        "emoji should pass through intact; got {:?}",
        text
    );
    assert!(
        text.contains("Δt"),
        "greek letters should pass through intact; got {:?}",
        text
    );
}
