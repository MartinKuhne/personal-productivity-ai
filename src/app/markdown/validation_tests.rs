//! Tests for `markdown/validation.rs` (UI-043).
//!
//! Covers the happy path (valid GFM bodies of every flavour UI-043 names),
//! every fence-detection branch, and the failure path's location data.
//!
//! The bodies are **table-driven**: every accepted body is one row of one
//! table and every unterminated-fence case is one row of another. Each row
//! names itself in its failure message, so a regression still reports which
//! body broke — the tables exist so the suite pays one process spawn per
//! shape of check instead of one per body.

use super::{
    MAX_FENCE_INDENT, MIN_FENCE_WIDTH, MarkdownErrorKind, fence_run, unterminated_code_fence,
    validate_markdown,
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

/// The option set is exactly UI-043's four extensions: the requested flags are
/// all on, and every unrequested extension the parser offers stays off.
#[test]
fn gfm_options_are_exactly_the_ui_043_feature_set() {
    let expected = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;

    assert_eq!(
        gfm_options(),
        expected,
        "the renderer's option set must be exactly the four UI-043 extensions"
    );

    let options = gfm_options();
    for (name, flag) in [
        ("ENABLE_WIKILINKS", Options::ENABLE_WIKILINKS),
        (
            "ENABLE_SMART_PUNCTUATION",
            Options::ENABLE_SMART_PUNCTUATION,
        ),
        ("ENABLE_MATH", Options::ENABLE_MATH),
        ("ENABLE_GFM", Options::ENABLE_GFM),
        (
            "ENABLE_HEADING_ATTRIBUTES",
            Options::ENABLE_HEADING_ATTRIBUTES,
        ),
    ] {
        assert!(
            !options.contains(flag),
            "{name} is not part of UI-043's set and must stay off"
        );
    }
}

// --- happy path ----------------------------------------------------------

/// Every body UI-043 considers well-formed validates cleanly, including the
/// two documented limitations the parser degrades to literal text instead of
/// reporting.
#[test]
fn every_well_formed_body_is_accepted() {
    let cases: &[(&str, &str)] = &[
        ("empty body", ""),
        ("full GFM body", valid_gfm_body()),
        ("closed backtick fence", "```rust\nlet x = 1;\n```\n"),
        (
            "closed backtick fence without a trailing newline",
            "```rust\nlet x = 1;\n```",
        ),
        ("closed tilde fence", "~~~python\nprint(1)\n~~~\n"),
        (
            // CommonMark: the closer must be *at least* as long as the opener.
            "fence closed by a longer fence",
            "```\ncode\n````\n",
        ),
        (
            "closer carrying only trailing spaces still closes",
            "```\ncode\n```   \n",
        ),
        (
            "short backtick runs are not fences",
            "`inline code` and ``double``",
        ),
        ("double tilde is not a fence", "~~struck~~ not a fence"),
        (
            "four-space indent is indented code, not a fence",
            "    ```\n    not a fence\n",
        ),
        ("CRLF line endings", "```rust\r\ncode\r\n```\r\n"),
        (
            // Pins the documented limitation: pulldown-cmark degrades
            // `[^missing]` to literal text, so there is nothing to detect yet.
            "dangling footnote reference",
            "text[^missing] more",
        ),
        (
            // Pins the documented limitation: the parser pads short rows and
            // drops extra cells, so neither is observable as a defect.
            "mismatched table rows",
            "| a | b |\n| - | - |\n| 1 |\n| 2 | 3 | 4 |\n",
        ),
    ];

    for (label, body) in cases {
        assert_eq!(
            validate_markdown(body),
            Ok(()),
            "{label}: must validate as well-formed"
        );
    }
}

// --- failure path --------------------------------------------------------

/// One unterminated-fence case and the location the sidecar pins for it.
///
/// `byte_offset` is `None` for the cases whose retired test pinned only the
/// line and column, so the table asserts exactly what those tests asserted —
/// no exactness is invented here.
struct FenceCase {
    label: &'static str,
    body: &'static str,
    byte_offset: Option<usize>,
    line: usize,
    column: usize,
}

/// Every unterminated fence is reported at its **opening** fence, and only the
/// first unterminated block of a document is reported.
#[test]
fn every_unterminated_fence_reports_its_opening_fence() {
    let indented = "line one\nline two\n  ```\ncode";
    let multibyte = "héllo\n```\ncode";

    let cases = [
        FenceCase {
            label: "three-space indent is still a fence",
            body: "   ```\n   unclosed\n",
            byte_offset: Some(3),
            line: 1,
            column: 4,
        },
        FenceCase {
            label: "unclosed backtick fence reports the opening fence",
            body: "```\ncode\nmore",
            byte_offset: Some(0),
            line: 1,
            column: 1,
        },
        FenceCase {
            label: "unclosed tilde fence",
            body: "intro\n~~~\nstill open",
            byte_offset: Some(6),
            line: 2,
            column: 1,
        },
        FenceCase {
            label: "unclosed indented fence reports its column",
            body: indented,
            byte_offset: Some(indented.find("```").expect("fence present")),
            line: 3,
            column: 3,
        },
        FenceCase {
            // ```` opens with 4; ``` is content, so the block never closes.
            label: "a shorter fence cannot close a longer opener",
            body: "````\n```\ncontent",
            byte_offset: None,
            line: 1,
            column: 1,
        },
        FenceCase {
            label: "a closer with trailing text does not close",
            body: "```\ncode\n``` tail",
            byte_offset: None,
            line: 1,
            column: 1,
        },
        FenceCase {
            label: "a tilde fence cannot close a backtick fence",
            body: "```\ncode\n~~~\nmore",
            byte_offset: None,
            line: 1,
            column: 1,
        },
        FenceCase {
            label: "CRLF line endings locate the fence",
            body: "a\r\n```\r\nb\r\n",
            byte_offset: Some(3),
            line: 2,
            column: 1,
        },
        FenceCase {
            // The ``` block closes cleanly; the ~~~ block is the first
            // unterminated one, so that is the location reported.
            label: "the first unterminated fence is the one reported",
            body: "```\nfine\n```\n\n~~~\nalso open\n\n```\nfine\n```\n",
            byte_offset: None,
            line: 5,
            column: 1,
        },
        FenceCase {
            label: "multibyte text before a fence offsets in bytes",
            body: multibyte,
            byte_offset: Some(multibyte.find("```").expect("fence present")),
            line: 2,
            column: 1,
        },
    ];

    for case in &cases {
        let error = validate_markdown(case.body)
            .expect_err("the body must fail validation with an unterminated-fence error");

        assert_eq!(
            error.kind,
            MarkdownErrorKind::UnterminatedCodeFence,
            "{}: the only defect this validator reports",
            case.label
        );
        assert_eq!(
            error.line, case.line,
            "{}: the error must carry the opening fence's 1-based line",
            case.label
        );
        assert_eq!(
            error.column, case.column,
            "{}: the error must carry the opening fence's 1-based column",
            case.label
        );
        if let Some(offset) = case.byte_offset {
            assert_eq!(
                error.byte_offset, offset,
                "{}: the error must carry the opening fence's byte offset",
                case.label
            );
        }
    }
}

// --- helper-level edge cases ---------------------------------------------

/// The fence helpers enforce the CommonMark limits the validator relies on:
/// the minimum run width, the maximum indent, and silence for closed blocks.
#[test]
fn the_fence_helpers_enforce_the_commonmark_limits() {
    assert_eq!(
        fence_run("``"),
        None,
        "fence_run: a two-character run is below the minimum width"
    );
    assert_eq!(
        fence_run(&"`".repeat(MIN_FENCE_WIDTH)),
        Some(('`', MIN_FENCE_WIDTH)),
        "fence_run: exactly the minimum width opens a backtick fence"
    );
    assert_eq!(
        fence_run(&"~".repeat(MIN_FENCE_WIDTH)),
        Some(('~', MIN_FENCE_WIDTH)),
        "fence_run: exactly the minimum width opens a tilde fence"
    );
    assert_eq!(
        fence_run("nope"),
        None,
        "fence_run: a line not starting with a fence character is not a fence"
    );

    assert_eq!(
        MAX_FENCE_INDENT, 3,
        "the indent limit must match CommonMark's three-space rule"
    );
    assert_eq!(
        MIN_FENCE_WIDTH, 3,
        "the minimum run width must match CommonMark's three-character rule"
    );

    assert!(
        unterminated_code_fence("```\nx\n```\n").is_none(),
        "unterminated_code_fence: a closed block has nothing to report"
    );
}
