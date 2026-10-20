//! Tests for `markdown/validation.rs` (UI-043).
//!
//! Covers the happy path (valid GFM bodies of every flavour UI-043 names),
//! every fence-detection branch, and the failure path's location data.

use super::{
    MAX_FENCE_INDENT, MIN_FENCE_WIDTH, MarkdownErrorKind, MarkdownParseError, fence_run,
    unterminated_code_fence, validate_markdown,
};
use crate::markdown::gfm_options;
use pulldown_cmark::Options;

/// A body exercising every GFM extension UI-043 lists.
fn valid_gfm_body() -> &'static str {
    "# Title\n\n\
     A paragraph with **bold**, *italic* and ~~struck~~ text.\n\n\
     - [x] done task\n\
     - [ ] open task\n\n\
     | a | b |\n\
     | - | - |\n\
     | 1 | 2 |\n\n\
     A footnote reference[^note].\n\n\
     [^note]: The footnote body.\n"
}

// --- the shared option set -------------------------------------------------

#[test]
fn gfm_options_enables_exactly_the_ui_043_feature_set() {
    let expected = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;

    assert_eq!(gfm_options(), expected);
}

#[test]
fn gfm_options_leaves_unrequested_extensions_off() {
    let options = gfm_options();

    assert!(!options.contains(Options::ENABLE_WIKILINKS));
    assert!(!options.contains(Options::ENABLE_SMART_PUNCTUATION));
    assert!(!options.contains(Options::ENABLE_MATH));
    assert!(!options.contains(Options::ENABLE_GFM));
    assert!(!options.contains(Options::ENABLE_HEADING_ATTRIBUTES));
}

// --- happy path ----------------------------------------------------------

#[test]
fn empty_body_is_valid() {
    assert_eq!(validate_markdown(""), Ok(()));
}

#[test]
fn full_gfm_body_is_valid() {
    assert_eq!(validate_markdown(valid_gfm_body()), Ok(()));
}

#[test]
fn closed_backtick_fence_is_valid() {
    assert_eq!(validate_markdown("```rust\nlet x = 1;\n```\n"), Ok(()));
}

#[test]
fn closed_backtick_fence_without_trailing_newline_is_valid() {
    assert_eq!(validate_markdown("```rust\nlet x = 1;\n```"), Ok(()));
}

#[test]
fn closed_tilde_fence_is_valid() {
    assert_eq!(validate_markdown("~~~python\nprint(1)\n~~~\n"), Ok(()));
}

#[test]
fn fence_closed_by_longer_fence_is_valid() {
    // CommonMark: the closer must be *at least* as long as the opener.
    assert_eq!(validate_markdown("```\ncode\n````\n"), Ok(()));
}

#[test]
fn fence_closer_with_trailing_text_still_closes_when_only_spaces() {
    assert_eq!(validate_markdown("```\ncode\n```   \n"), Ok(()));
}

#[test]
fn short_backtick_runs_are_not_fences() {
    assert_eq!(validate_markdown("`inline code` and ``double``"), Ok(()));
}

#[test]
fn double_tilde_is_not_a_fence() {
    assert_eq!(validate_markdown("~~struck~~ not a fence"), Ok(()));
}

#[test]
fn four_space_indent_is_indented_code_not_a_fence() {
    assert_eq!(validate_markdown("    ```\n    not a fence\n"), Ok(()));
}

#[test]
fn three_space_indent_is_a_fence() {
    assert_eq!(
        validate_markdown("   ```\n   unclosed\n"),
        Err(MarkdownParseError {
            kind: MarkdownErrorKind::UnterminatedCodeFence,
            byte_offset: 3,
            line: 1,
            column: 4,
        })
    );
}

// --- failure path --------------------------------------------------------

#[test]
fn unclosed_backtick_fence_reports_the_opening_fence_location() {
    let body = "```\ncode\nmore";

    assert_eq!(
        validate_markdown(body),
        Err(MarkdownParseError {
            kind: MarkdownErrorKind::UnterminatedCodeFence,
            byte_offset: 0,
            line: 1,
            column: 1,
        })
    );
}

#[test]
fn unclosed_tilde_fence_is_a_failure() {
    assert_eq!(
        validate_markdown("intro\n~~~\nstill open"),
        Err(MarkdownParseError {
            kind: MarkdownErrorKind::UnterminatedCodeFence,
            byte_offset: 6,
            line: 2,
            column: 1,
        })
    );
}

#[test]
fn unclosed_indented_fence_reports_its_column() {
    let body = "line one\nline two\n  ```\ncode";

    assert_eq!(
        validate_markdown(body),
        Err(MarkdownParseError {
            kind: MarkdownErrorKind::UnterminatedCodeFence,
            byte_offset: body.find("```").expect("fence present"),
            line: 3,
            column: 3,
        })
    );
}

#[test]
fn a_shorter_fence_cannot_close_a_longer_opener() {
    // ```` opens with 4; ``` is content, so the block never closes.
    let error = validate_markdown("````\n```\ncontent").expect_err("must fail");

    assert_eq!(error.kind, MarkdownErrorKind::UnterminatedCodeFence);
    assert_eq!(error.line, 1);
    assert_eq!(error.column, 1);
}

#[test]
fn a_closer_with_trailing_text_does_not_close() {
    let error = validate_markdown("```\ncode\n``` tail").expect_err("must fail");

    assert_eq!(error.line, 1);
}

#[test]
fn a_tilde_fence_cannot_close_a_backtick_fence() {
    let error = validate_markdown("```\ncode\n~~~\nmore").expect_err("must fail");

    assert_eq!(error.line, 1);
}

#[test]
fn crlf_line_endings_are_handled() {
    assert_eq!(validate_markdown("```rust\r\ncode\r\n```\r\n"), Ok(()));
    assert_eq!(
        validate_markdown("a\r\n```\r\nb\r\n"),
        Err(MarkdownParseError {
            kind: MarkdownErrorKind::UnterminatedCodeFence,
            byte_offset: 3,
            line: 2,
            column: 1,
        })
    );
}

#[test]
fn the_first_unterminated_fence_is_the_one_reported() {
    // The ``` block closes cleanly; the ~~~ block is the first unterminated
    // one, so that is the location reported.
    let error = validate_markdown("```\nfine\n```\n\n~~~\nalso open\n\n```\nfine\n```\n")
        .expect_err("must fail");

    assert_eq!(error.kind, MarkdownErrorKind::UnterminatedCodeFence);
    assert_eq!(error.line, 5);
    assert_eq!(error.column, 1);
}

#[test]
fn multibyte_text_before_a_fence_offsets_in_bytes() {
    let body = "héllo\n```\ncode";

    let error = validate_markdown(body).expect_err("must fail");

    assert_eq!(error.byte_offset, body.find("```").expect("fence present"));
    assert_eq!(error.line, 2);
    assert_eq!(error.column, 1);
}

// --- helper-level edge cases ---------------------------------------------

#[test]
fn fence_run_requires_the_minimum_width() {
    assert_eq!(fence_run("``"), None);
    assert_eq!(
        fence_run(&"`".repeat(MIN_FENCE_WIDTH)),
        Some(('`', MIN_FENCE_WIDTH))
    );
    assert_eq!(
        fence_run(&"~".repeat(MIN_FENCE_WIDTH)),
        Some(('~', MIN_FENCE_WIDTH))
    );
    assert_eq!(fence_run("nope"), None);
}

#[test]
fn fence_indent_limit_matches_commonmark() {
    assert_eq!(MAX_FENCE_INDENT, 3);
    assert_eq!(MIN_FENCE_WIDTH, 3);
}

#[test]
fn unterminated_fence_helper_is_none_for_closed_blocks() {
    assert!(unterminated_code_fence("```\nx\n```\n").is_none());
}

#[test]
fn validate_markdown_does_not_reject_dangling_footnotes() {
    // Pins the documented limitation: pulldown-cmark degrades `[^missing]`
    // to literal text, so there is nothing to detect here yet.
    assert_eq!(validate_markdown("text[^missing] more"), Ok(()));
}

#[test]
fn validate_markdown_does_not_reject_mismatched_table_rows() {
    // Pins the documented limitation: the parser pads short rows and drops
    // extra cells, so neither is observable as a defect.
    assert_eq!(
        validate_markdown("| a | b |\n| - | - |\n| 1 |\n| 2 | 3 | 4 |\n"),
        Ok(())
    );
}
