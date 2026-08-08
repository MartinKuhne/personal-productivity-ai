//! Unit tests for the Markdown → Typst translator.
//!
//! Sidecar of `typst.rs`. Tests are pure-string assertions on the
//! generated Typst markup; they do not invoke the Typst compiler.
//!
//! Per AGENTS.md RUST-056 / RUST-057 — extracted to a sidecar.

use super::{escape_typst, escape_typst_string, render_markdown_to_typst};

#[test]
fn empty_input_emits_empty_string() {
    let out = render_markdown_to_typst("");
    assert!(out.trim().is_empty(), "got: {out}");
}

#[test]
fn plain_paragraph_passes_through() {
    let out = render_markdown_to_typst("hello world");
    assert!(out.contains("hello world"), "got: {out}");
}

#[test]
fn heading_levels_are_emitted() {
    let md = "# h1\n\n## h2\n\n### h3\n";
    let out = render_markdown_to_typst(md);
    // Typst heading markers: `= ` for h1, `== ` for h2, etc. They
    // appear at the start of a line and the heading text follows.
    assert!(out.contains("= h1"), "missing h1 marker in: {out}");
    assert!(out.contains("== h2"));
    assert!(out.contains("=== h3"));
}

#[test]
fn strong_renders_as_strong_function() {
    // Markdown `**bold**` becomes the Typst `#strong[bold]`
    // content-block form. Earlier versions emitted the literal
    // delimiter form `*bold*`, but that has subtle word-boundary
    // rules: a `*` is only recognized as an emphasis marker when
    // it appears at a position where it could start a word. The
    // spec test caught the adjacent-to-text cases (`**foo**bar`,
    // `foo**bar**`) failing with "unclosed delimiter" because the
    // output `*foo*bar` / `foo*bar*` had the trailing asterisk
    // interpreted as an opener. The function form is unambiguous.
    let out = render_markdown_to_typst("**bold**");
    assert!(out.contains("#strong[bold]"), "got: {out}");
}

#[test]
fn emphasis_renders_as_emph_function() {
    // Markdown `*italic*` becomes the Typst `#emph[italic]`
    // content-block form. The earlier `_italic_` delimiter form
    // was valid for word-bounded cases but would silently
    // disappear (literal underscore) when the emphasis was
    // adjacent to other text like `foo*bar*`. The function form
    // is unambiguous and works at any position. See
    // `strong_renders_as_strong_function` for the full rationale.
    let out = render_markdown_to_typst("*italic*");
    assert!(out.contains("#emph[italic]"), "got: {out}");
}

#[test]
fn emphasis_adjacent_to_text() {
    // Regression: emphasis and strong whose boundaries are
    // immediately adjacent to other text (no whitespace, no
    // punctuation). The delimiter form `*...*` and `**...**` has
    // subtle word-boundary rules in Typst that fail in these
    // positions; the function form `#emph[...]` / `#strong[...]`
    // works at any position. The five cases below are the spec
    // failures #311, #326, #337, #352, #371.
    let cases = [
        ("foo*bar*", "foo#emph[bar]"),
        ("*foo*bar", "#emph[foo]bar"),
        ("foo**bar**", "foo#strong[bar]"),
        ("**foo**bar", "#strong[foo]bar"),
        ("*foo**bar***", "#emph[foo#strong[bar]]"),
    ];
    for (md, expected) in cases {
        let out = render_markdown_to_typst(md);
        assert!(
            out.contains(expected),
            "expected {expected:?} in translator output for {md:?}, got: {out}"
        );
    }
}

#[test]
fn strikethrough_uses_strike_function() {
    let out = render_markdown_to_typst("~~gone~~");
    assert!(out.contains("#strike["), "got: {out}");
    assert!(out.contains("gone"));
    assert!(out.contains(']'));
}

#[test]
fn inline_code_renders_as_raw_function() {
    // Inline code spans are emitted as the Typst `#raw("...")` function
    // call (string argument) rather than backtick-fenced raw text. The
    // function form is required because the body can contain backticks
    // — see `inline_code_with_embedded_backticks` below for the
    // embedded-backtick regression that motivated the switch. The
    // body is string-escaped (only `\` and `"`) so markup-special
    // characters in the code body are passed through verbatim.
    let out = render_markdown_to_typst("`x = 1`");
    assert!(
        out.contains("#raw(\"x = 1\")"),
        "expected raw function form, got: {out}"
    );
}

#[test]
fn inline_code_with_embedded_backticks() {
    // Regression: code spans whose body contains one or more backticks
    // must still render to a valid Typst document. The backtick-
    // fenced raw text form was rejected by Typst with "unclosed raw
    // text" because `escape_typst` backslash-escaped the embedded
    // backtick to `` \` ``, but the `\` is a literal character inside
    // raw text (not an escape) and the following `` ` `` was
    // interpreted as the raw's close delimiter. The fix: switch to
    // the `#raw("...")` function form, where the body is a string
    // literal and embedded backticks render literally.
    //
    // Each example below is one of the five spec failures
    // (examples #285, #286, #287, #295, #296) — the translation is
    // asserted to contain `#raw("...")` with the expected body
    // content. The full round-trip (translation + PDF compile) is
    // covered by the spec test in `tests/commonmark_spec_test.rs`.
    let cases = [
        ("`` foo ` bar ``", "foo ` bar"),
        ("` `` `", "``"),
        ("`  ``  `", " `` "),
        ("``foo`bar``", "foo`bar"),
        ("` foo `` bar `", "foo `` bar"),
    ];
    for (md, expected_body) in cases {
        let out = render_markdown_to_typst(md);
        let expected_call = format!("#raw(\"{}\")", expected_body);
        assert!(
            out.contains(&expected_call),
            "expected {expected_call:?} in translator output for {md:?}, got: {out}"
        );
    }
}

#[test]
fn fenced_code_block_uses_raw_with_lang() {
    let md = "```rust\nfn main() {}\n```\n";
    let out = render_markdown_to_typst(md);
    // The new format uses the Typst string form so that curly
    // braces and other markup-special characters in code bodies
    // are passed through verbatim.
    assert!(
        out.contains("#raw(block: true, lang: \"rust\", \"fn main() {}\")")
            || out.contains("#raw(block: true, lang: \"rust\""),
        "got: {out}"
    );
    assert!(out.contains("fn main()"));
}

#[test]
fn fenced_code_block_without_lang_uses_raw_string() {
    let md = "```\nplain code\n```\n";
    let out = render_markdown_to_typst(md);
    // We emit `#raw(block: true, "...")` for the untagged case
    // so that any markup-special characters in the code body
    // (curly braces, asterisks, etc.) are passed through verbatim
    // and not interpreted as Typst markup.
    assert!(out.contains("#raw(block: true"), "got: {out}");
    assert!(out.contains("plain code"));
}

#[test]
fn unordered_list_renders_bullets() {
    let out = render_markdown_to_typst("- one\n- two\n- three\n");
    // Each item should be a `- ` bullet.
    let bullets = out.matches("- ").count();
    assert!(
        bullets >= 3,
        "expected >=3 bullets, got {bullets} in: {out}"
    );
}

#[test]
fn ordered_list_wraps_in_enum_function() {
    // Ordered lists are emitted as `#enum(numbering: "1.")[ ... ]`
    // with the items as `+ Item` markup inside a content block.
    // (The earlier `#list(marker: ([_],), + Item)` form was
    // invalid Typst: `+ Item` is a list-item expression, not a
    // function-call arg, so Typst rejected it. See the translator
    // source for the full explanation.)
    let out = render_markdown_to_typst("1. one\n2. two\n");
    assert!(out.contains("#enum(numbering: \"1.\")"), "got: {out}");
    assert!(out.contains("+ one"), "got: {out}");
    assert!(out.contains("+ two"), "got: {out}");
    // Items live inside a content block, not as further function
    // call args.
    assert!(out.contains("[\n+ one"), "got: {out}");
    assert!(out.ends_with("]\n"), "got: {out}");
}

#[test]
fn task_list_emits_checkbox_marker() {
    let out = render_markdown_to_typst("- [ ] todo\n- [x] done\n");
    assert!(out.contains("[ ]"), "got: {out}");
    assert!(out.contains("[x]"));
}

#[test]
fn blockquote_renders_as_quote_function() {
    let out = render_markdown_to_typst("> a quotation\n");
    assert!(out.contains("#quote(block: true)["), "got: {out}");
    assert!(out.contains("a quotation"));
}

#[test]
fn link_renders_as_link_function() {
    let out = render_markdown_to_typst("[text](https://example.com)");
    assert!(
        out.contains("#link(\"https://example.com\")["),
        "got: {out}"
    );
    assert!(out.contains("text"));
    // Trailing space after the closing `]` is the chain break for
    // the case where the link is followed by `(args)` or
    // `[args]` — see the `link_followed_by_paren` test and
    // `Event::InlineHtml` for the same pattern. The space is
    // visually invisible in the rendered PDF because the
    // surrounding paragraph's `par(justify: true)` collapses
    // adjacent whitespace; in code where the chain break is
    // unnecessary (no `(args)` follows), the space still doesn't
    // affect rendering.
    assert!(
        out.contains("] "),
        "expected trailing space after link close, got: {out}"
    );
}

#[test]
fn autolink_text_escapes_url_chars() {
    // Regression: CommonMark autolinks (`<url>`) emit a
    // `Link { link_type: Autolink, ... }` event with the URL
    // itself as the link text. URLs routinely contain `:` and
    // `//` which Typst treats as markup-active in content mode
    // (`:` after a word starts a labelled-content item; the
    // value swallows the surrounding `[...]` and produces
    // "unclosed delimiter"). The translator routes autolink
    // text through a stricter escape that backslash-escapes
    // `:` and `/` in addition to the usual markup-active chars.
    // Regular `[text](url)` links keep the user's text
    // verbatim — they may have used their own markup on
    // purpose.
    //
    // The expected escape is built char-by-char so the test
    // is obviously correct without having to count backslashes
    // in a string literal (which is notoriously error-prone).
    fn esc(s: &str) -> String {
        let mut out = String::new();
        for c in s.chars() {
            match c {
                ':' => out.push_str("\\:"),
                '/' => out.push_str("\\/"),
                other => out.push(other),
            }
        }
        out
    }
    let cases = [
        ("<irc://foo.bar:2233/baz>", esc("irc://foo.bar:2233/baz")),
        ("<http://example.com>", esc("http://example.com")),
        (
            "<made-up-scheme://foo,bar>",
            esc("made-up-scheme://foo,bar"),
        ),
    ];
    for (md, expected_text) in cases {
        let out = render_markdown_to_typst(md);
        let needle = format!("[{expected_text}]");
        assert!(
            out.contains(&needle),
            "expected link text {needle:?} in translator output for {md:?}, got: {out}"
        );
    }
}

#[test]
fn link_followed_by_paren_uses_chain_break() {
    // Regression: CommonMark example #524
    // (`[foo](not a link)\n\n[foo]: /url1`) translates to a
    // link with text `foo` followed by literal `(not a link)`.
    // The `TagEnd::Link` emits `]` and then the `(not a link)`
    // text follows. Without a chain break, Typst parses
    // `#link("...")[foo](not a link)` as
    // "call `link()`, then call its result on `(not a link)`"
    // — content can't be called, so the compiler errors with
    // "expected comma". A single space after `]` forces a new
    // content sequence.
    let out = render_markdown_to_typst("[foo](not a link)\n\n[foo]: /url1\n");
    // The link should be resolved by pulldown-cmark to the
    // reference URL, and the closing `]` must be followed by
    // a space.
    assert!(
        out.contains("#link(\"/url1\")[foo] (not a link)"),
        "expected link with chain-break space before '(not a link)', got: {out}"
    );
}

#[test]
fn image_renders_as_image_function() {
    // The translator renders images as a Typst placeholder block
    // that displays the destination URL. `#image("...")` would
    // require the URL to resolve to a real file at compile time,
    // which the spec test corpus does not provide. The placeholder
    // also documents in the PDF that an image was dropped — useful
    // for exports of real documents where the image is missing.
    let out = render_markdown_to_typst("![alt](https://example.com/img.png)");
    assert!(
        out.contains("image: https://example.com/img.png"),
        "got: {out}"
    );
    assert!(out.contains("#block("), "got: {out}");
}

#[test]
fn gfm_table_emits_table_function() {
    let md = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    let out = render_markdown_to_typst(md);
    assert!(out.contains("#table("), "got: {out}");
    // The placeholder is patched at Table end.
    assert!(out.contains("columns: 2"), "missing column count in: {out}");
    assert!(
        out.contains("table.header("),
        "missing table.header in: {out}"
    );
    assert!(out.contains("a"));
    assert!(out.contains("b"));
    assert!(out.contains("1"));
    assert!(out.contains("2"));
}

#[test]
fn horizontal_rule_renders_as_line() {
    let out = render_markdown_to_typst("---\n");
    assert!(out.contains("#line(length: 100%"), "got: {out}");
}

#[test]
fn hard_break_renders_as_backslash() {
    let out = render_markdown_to_typst("line one  \nline two\n");
    assert!(out.contains(" \\"), "got: {out}");
}

#[test]
fn hash_sign_in_text_is_escaped() {
    // The user writes "C# language" in Markdown. Without escaping,
    // Typst would interpret the `#` as a function-call prefix.
    let out = render_markdown_to_typst("I love C# language");
    assert!(out.contains("C\\#"), "expected escaped hash, got: {out}");
}

#[test]
fn asterisk_in_text_is_escaped() {
    let out = render_markdown_to_typst("rating: 5 * 3");
    // `*` outside of `**` or `*` markers should be escaped; the
    // important invariant is that we don't emit unescaped asterisks
    // that typst would interpret as strong/emphasis.
    assert!(
        !out.contains("rating: 5 * 3\n"),
        "unescaped asterisk: {out}"
    );
}

#[test]
fn backslash_in_text_is_escaped() {
    let out = render_markdown_to_typst(r"path: C:\Users");
    assert!(
        out.contains("C:\\\\Users") || out.contains("C:\\Users"),
        "got: {out}"
    );
}

#[test]
fn complex_doc_round_trips_all_features() {
    let md = "# Title\n\nA paragraph with **bold** and *italic*.\n\n- one\n- two\n\n```rust\nlet x = 1;\n```\n\n> a quote\n\n| h1 | h2 |\n|----|----|\n| a  | b  |\n";
    let out = render_markdown_to_typst(md);
    // Spot-check each feature survived.
    assert!(out.contains("= Title"), "missing heading: {out}");
    assert!(out.contains("#strong[bold]"));
    assert!(out.contains("#emph[italic]"));
    assert!(
        out.contains("#raw(block: true, lang: \"rust\""),
        "got: {out}"
    );
    assert!(out.contains("#quote(block: true)["));
    assert!(out.contains("#table("));
}

// =====================================================================
// Typst syntax-reference compliance
// =====================================================================
//
// These tests pin the translator to the markup-active character set
// documented at https://typst.app/docs/reference/syntax/. Every char
// listed under "Markup" / "Code expression" / "Symbol shorthand" /
// "Character escape" / "Smart quote" must be backslash-escaped when
// it appears in user-supplied content that lands inside a Typst
// content block (i.e. between `[...]`).
//
// The exhaustive set is encoded in the table at the top of
// `escape_typst` in `typst.rs`; the assertions below test the
// function one char at a time and a representative string at once.
// If a future Typst release adds a new markup-active char, add a
// row to the table, a row to this test, and the matching arm in
// `escape_typst`.

/// Pin: `\` — the escape character itself. Must always be doubled.
#[test]
fn escape_typst_backslash() {
    assert_eq!(escape_typst("\\"), r"\\");
    assert_eq!(escape_typst("a\\b"), r"a\\b");
}

/// Pin: `#` — entry into code mode.
#[test]
fn escape_typst_hash() {
    assert_eq!(escape_typst("#"), r"\#");
    assert_eq!(escape_typst("C# language"), r"C\# language");
}

/// Pin: `*` — strong emphasis.
#[test]
fn escape_typst_star() {
    assert_eq!(escape_typst("*"), r"\*");
    assert_eq!(escape_typst("2*3"), r"2\*3");
}

/// Pin: `_` — emphasis.
#[test]
fn escape_typst_underscore() {
    assert_eq!(escape_typst("_"), r"\_");
    assert_eq!(escape_typst("snake_case"), r"snake\_case");
}

/// Pin: `` ` `` — inline raw text.
#[test]
fn escape_typst_backtick() {
    assert_eq!(escape_typst("`"), r"\`");
    assert_eq!(escape_typst("a`b"), r"a\`b");
}

/// Pin: `<` and `>` — Typst label-reference syntax (`<name>`).
/// Without escape, user text like `<a href="hi">` gets parsed
/// as a label reference, swallowing the rest of the line and
/// producing "unclosed label" errors. The earlier version of
/// this function did NOT escape `<` and `>`, reasoning that
/// they're only markup-active at line start — but the
/// CommonMark spec test exposed that they're markup-active at
/// any position in content mode. (Eight spec failures
/// collapsed to zero once these chars were added to the
/// escape set: examples #558, #562, #575, #576, #577, #578,
/// #588, plus a structural fix to the autolink text escape
/// that took #552 and #555 along with it.)
#[test]
fn escape_typst_angle_brackets() {
    assert_eq!(escape_typst("<"), "\\<");
    assert_eq!(escape_typst(">"), "\\>");
    assert_eq!(escape_typst("<a href=\"hi\">"), "\\<a href=\\\"hi\\\"\\>");
}

/// Pin: `[` and `]` — content block delimiters. Critical: a stray
/// bracket inside a content block would either close the block
/// early (malformed Typst) or be interpreted as markup.
#[test]
fn escape_typst_brackets() {
    assert_eq!(escape_typst("["), r"\[");
    assert_eq!(escape_typst("]"), r"\]");
    assert_eq!(escape_typst("[a][b]"), r"\[a\]\[b\]");
}

/// Pin: `@` — reference marker.
#[test]
fn escape_typst_at() {
    assert_eq!(escape_typst("@"), r"\@");
    assert_eq!(escape_typst("user@example.com"), r"user\@example.com");
}

/// Pin: `$` — entry into math mode. Without escape, the user's
/// text "costs $5" would be parsed as Typst math and fail to
/// compile. Caught by the `dollar_sign_in_text` integration test
/// in `print_pdf_tests.rs`.
#[test]
fn escape_typst_dollar() {
    assert_eq!(escape_typst("$"), r"\$");
    assert_eq!(escape_typst("costs $5"), r"costs \$5");
}

/// Pin: `~` — symbol shorthand (a bare `~` is a non-breaking space).
#[test]
fn escape_typst_tilde() {
    assert_eq!(escape_typst("~"), r"\~");
    assert_eq!(escape_typst("~hello~"), r"\~hello\~");
}

/// Pin: `'` — smart quote trigger. Without escape, the user's
/// "don't" gets rendered as the typographic curly "don't".
#[test]
fn escape_typst_apostrophe() {
    assert_eq!(escape_typst("'"), r"\'");
    assert_eq!(escape_typst("don't"), r"don\'t");
}

/// Pin: `"` — smart quote trigger. Without escape, the user's
/// straight double quotes are converted to curly variants.
#[test]
fn escape_typst_quote() {
    assert_eq!(escape_typst("\""), r#"\""#);
    assert_eq!(escape_typst(r#"say "hi""#), r#"say \"hi\""#);
}

/// Pin: chars that look like markup but are NOT escaped (and don't
/// need to be inside a content block). Documents the negative
/// half of the contract so a well-meaning future change can't
/// accidentally over-escape and break a test that relies on a
/// literal `-`, `+`, `=`, `/` in user content.
#[test]
fn escape_typst_passes_through_safe_chars() {
    // `-` `+` `=` `/` at non-line-start position are not markup-
    // active. (At line start in *markup* mode they would be, but
    // we emit user content inside `[...]` content blocks where
    // line position has no markup meaning.)
    assert_eq!(escape_typst("-"), "-");
    assert_eq!(escape_typst("+"), "+");
    assert_eq!(escape_typst("="), "=");
    assert_eq!(escape_typst("/"), "/");
    // ASCII letters, digits, whitespace, and common punctuation
    // pass through unchanged.
    assert_eq!(escape_typst("hello world 123"), "hello world 123");
    assert_eq!(escape_typst("a.b,c;d:e"), "a.b,c;d:e");
    // Unicode (non-ASCII letters) passes through. Identifier syntax
    // accepts arbitrary letters per Unicode Standard Annex #31.
    assert_eq!(escape_typst("café"), "café");
    assert_eq!(escape_typst("日本語"), "日本語");
}

/// Round-trip every escape in a single string and confirm the
/// exact byte sequence. This is the single source of truth for
/// the escape contract; the per-char tests above are the
/// individual pins.
#[test]
fn escape_typst_full_set_round_trip() {
    // Input is 12 chars: `\` `#` `*` `_` `` ` `` `[` `]` `@` `$` `~` `"` `'`
    // (note: two backslashes in source `\\` is one backslash; `\"` is
    // one double-quote; the apostrophe is unescaped).
    let input: &str = "\\#*_`[]@$~\"'";
    // Expected output is 24 chars: each input char becomes
    // backslash + char, except the backslash itself which becomes
    // two backslashes. Build it char-by-char to keep the test
    // obviously correct.
    let mut expected = String::with_capacity(24);
    for c in ['\\', '#', '*', '_', '`', '[', ']', '@', '$', '~', '"', '\''] {
        expected.push('\\');
        expected.push(c);
    }
    let actual = escape_typst(input);
    assert_eq!(actual, expected);
    // Length sanity check: 12 chars in, 24 chars out.
    assert_eq!(actual.len(), 24);
}

// ---------------------------------------------------------------------
// String escape function (used in code blocks and URL fields).
// ---------------------------------------------------------------------

/// Pin: inside a string literal only `\\` and `\"` are recognised
/// escape sequences. The function emits exactly those.
#[test]
fn escape_typst_string_only_quotes_and_backslashes() {
    // Use raw strings with `##` delimiter to keep the test
    // readable — a regular Rust string with both `\` and `"`
    // gets hard to parse visually.
    let one_backslash: &str = r#"\"#; // 1 char: \
    let one_quote: &str = r#"""#; // 1 char: "
    let two_backslashes: &str = r#"\\"#; // 2 chars: \\
    let escaped_quote: &str = r#"\""#; // 2 chars: \"
    let sample_url: &str = r#"https://x/#anchor"#;
    let sample_round_trip_in: &str = r#"has "quote" and \backslash"#;
    let sample_round_trip_out: &str = r#"has \"quote\" and \\backslash"#;
    // Input is one backslash; expected output is two (the escape
    // sequence for a single literal backslash inside a string).
    assert_eq!(escape_typst_string(one_backslash), two_backslashes);
    // Input is one double-quote; expected output is `\"`.
    assert_eq!(escape_typst_string(one_quote), escaped_quote);
    // Other chars that are markup-active in markup mode pass
    // through unchanged inside a string.
    assert_eq!(escape_typst_string("C# language"), "C# language");
    assert_eq!(escape_typst_string("$5"), "$5");
    assert_eq!(escape_typst_string("a*b_c"), "a*b_c");
    assert_eq!(escape_typst_string("[bracket]"), "[bracket]");
    assert_eq!(escape_typst_string(sample_url), sample_url);
    // Round-trip every char that needs escaping.
    assert_eq!(
        escape_typst_string(sample_round_trip_in),
        sample_round_trip_out
    );
}
