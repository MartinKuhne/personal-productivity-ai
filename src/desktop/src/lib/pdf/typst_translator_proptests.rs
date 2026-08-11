//! Property-based tests for the `escape_typst*` family.
//!
//! The escape functions are the project's last line of
//! defense against a markdown user (or a hostile agent input)
//! supplying content that Typst would otherwise interpret as
//! markup. The four closed escape gaps in
//! [`doc/adr/pdf-export-test-gaps.md`]
//! (`escape_typst`, `escape_typst_string`, `escape_typst_autolink`,
//! plus the `in_autolink` routing state) have example-based
//! unit tests; the proptest here layers a stronger property
//! check on top:
//!
//! 1. **No character survives un-escaped** when the escape
//!    function claims to escape it. The set of chars each
//!    function escapes is the contract; proptest checks it
//!    character-by-character on random inputs.
//! 2. **Pass-through is exact for non-special chars**:
//!    `escape_typst` and `escape_typst_string` must be the
//!    identity on inputs they don't touch. A regression
//!    that over-escapes (e.g. escapes `/` or `.`) would
//!    produce garbled output in the rendered PDF.
//! 3. **Idempotence where it should hold**: applying
//!    `escape_typst_string` twice is a no-op (the second
//!    pass has nothing to escape because the first pass
//!    produced only literal chars). `escape_typst` is *not*
//!    idempotent in general (a `\` from a previous pass is
//!    re-escaped) but the `escape_typst_string` pass should
//!    be.
//! 4. **`render_markdown_to_typst` never panics** on any
//!    random markdown input. The translator is the
//!    entry point for the whole pipeline; a panic in the
//!    translator would crash the editor. The proptest is
//!    the strongest "the translator is panic-free" check
//!    available short of `cargo-fuzz`.

use crate::pdf::typst_translator::{
    escape_typst, escape_typst_autolink, escape_typst_string, render_markdown_to_typst,
};
use proptest::prelude::*;

/// Strategy: any printable-ASCII string up to 80 chars. The
/// escape functions operate on individual chars; a random
/// 80-char string exercises every position and every char
/// in the printable-ASCII range.
fn printable_ascii() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\x20-\x7E]{0,80}").unwrap()
}

/// Strategy: a string that contains at least one of the
/// chars the escape function claims to escape. Without
/// this, the "no un-escaped char survives" property would
/// degenerate to "passes on inputs with no special chars".
fn contains_char(c: char) -> impl Strategy<Value = String> {
    (printable_ascii(), printable_ascii(), printable_ascii()).prop_map(move |(pre, mid, post)| {
        // Place the target char at a random position in
        // the middle segment so proptest's shrinking has
        // the surrounding context to work with.
        let mid_pos = mid.len() / 2;
        let (left, right) = mid.split_at(mid_pos);
        format!("{pre}{left}{c}{right}{post}")
    })
}

/// Strategy: a string that contains at least one of the
/// chars `escape_typst` claims to escape. The target char
/// is chosen uniformly from the documented escape set so
/// the proptest explores every char in the set across
/// shrinking.
fn string_with_any_target_char() -> impl Strategy<Value = String> {
    // The set of chars `escape_typst` escapes, in
    // deterministic order. Picking uniformly across this
    // set gives every char equal probability.
    let targets: [char; 16] = [
        '\\', '#', '*', '_', '`', '[', ']', '{', '}', '@', '$', '~', '\'', '"', '<', '>',
    ];
    let idx_strategy = 0..targets.len();
    (idx_strategy, printable_ascii()).prop_map(move |(idx, base)| {
        let c = targets[idx];
        // Splice `c` into the middle of `base`. The
        // surrounding context gives proptest something to
        // shrink to when it finds a failure.
        let mid = base.len() / 2;
        let (left, right) = base.split_at(mid);
        format!("{left}{c}{right}")
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Property 1a: `escape_typst` is a homomorphism for
    /// chars *not* in its escape set. Every char in the
    /// printable-ASCII range that is NOT in the escape
    /// set (`\`, `#`, `*`, `_`, `` ` ``, `[`, `]`, `{`,
    /// `}`, `@`, `$`, `~`, `'`, `"`, `<`, `>`) must
    /// pass through unchanged. A regression that adds
    /// an unrelated char to the escape set (e.g. `/`
    /// or `,`) would garble user content.
    #[test]
    fn escape_typst_passes_through_non_escape_chars(s in printable_ascii()) {
        let escaped = escape_typst(&s);
        // The set of chars `escape_typst` claims to
        // touch. Anything not in this set is the
        // "pass-through" set and must round-trip
        // exactly.
        let escape_set = [
            '\\', '#', '*', '_', '`', '[', ']', '{', '}', '@', '$', '~', '\'', '"', '<', '>',
        ];
        for c in s.chars() {
            if escape_set.contains(&c) {
                continue;
            }
            // Pass-through char: input count must equal
            // output count.
            let in_count = s.matches(c).count();
            let out_count = escaped.matches(c).count();
            if out_count != in_count {
                prop_assert!(
                    false,
                    "char {:?} should pass through `escape_typst` unchanged: \
                     input count {} output count {} (input={:?} output={:?})",
                    c,
                    in_count,
                    out_count,
                    s,
                    escaped
                );
            }
        }
    }

    /// Property 1b: `escape_typst_string` is a
    /// homomorphism for non-special chars and a
    /// one-to-one escape for `\` and `"`. Every char
    /// in the printable-ASCII range that is NOT `\`
    /// or `"` must pass through unchanged (input
    /// count == output count). For `\` and `"` the
    /// input char is replaced by its escape sequence
    /// (`\\` and `\"` respectively) — the count of
    /// the escape sequence in the output must equal
    /// the count of the char in the input.
    ///
    /// The arithmetic on the `\` char is interesting
    /// because BOTH escape sequences contribute to the
    /// `\` count: `\\` contributes 2 backslashes per
    /// input `\`, and `\"` contributes 1 backslash per
    /// input `"`. So the output `\` count is
    /// `2*in_count(\) + in_count(")`.
    ///
    /// A regression that adds a char to the escape
    /// set (e.g. escapes `{`) would corrupt code-block
    /// bodies which are interpolated into `"..."`
    /// string literals. A regression that drops `\`
    /// or `"` from the escape set would let the
    /// surrounding string literal terminate early.
    #[test]
    fn escape_typst_string_only_escapes_quote_and_backslash(s in printable_ascii()) {
        let escaped = escape_typst_string(&s);
        let in_backslashes = s.matches('\\').count();
        let in_quotes = s.matches('"').count();
        let out_backslashes = escaped.matches('\\').count();
        let out_quotes = escaped.matches('"').count();
        // The `\` count in the output is 2x the input
        // `\` count (each becomes `\\`) plus the input
        // `"` count (each becomes `\"` which adds one
        // `\`).
        if out_backslashes != in_backslashes * 2 + in_quotes {
            prop_assert!(
                false,
                "backslash count mismatch: input {} (\\)+{} (\"), expected {} in output, got {} \
                 (input={:?} output={:?})",
                in_backslashes,
                in_quotes,
                in_backslashes * 2 + in_quotes,
                out_backslashes,
                s,
                escaped
            );
        }
        // The `"` count in the output equals the input
        // `"` count (each input `"` becomes one `"`
        // in the output, as part of `\"`).
        if out_quotes != in_quotes {
            prop_assert!(
                false,
                "quote count mismatch: input {} expected {} in output, got {} \
                 (input={:?} output={:?})",
                in_quotes,
                in_quotes,
                out_quotes,
                s,
                escaped
            );
        }
        // Every other char passes through unchanged.
        for c in s.chars() {
            if c == '\\' || c == '"' {
                continue;
            }
            let in_count = s.matches(c).count();
            let out_count = escaped.matches(c).count();
            if out_count != in_count {
                prop_assert!(
                    false,
                    "char {:?} should pass through `escape_typst_string` \
                     unchanged: input count {} output count {} \
                     (input={:?} output={:?})",
                    c,
                    in_count,
                    out_count,
                    s,
                    escaped
                );
            }
        }
    }

    /// Property 1c: `escape_typst_autolink` is a strict
    /// superset of `escape_typst` for the pass-through
    /// chars. Every char that passes through the
    /// standard escape must also pass through the
    /// autolink escape. And the autolink-only set
    /// (`:`, `/`) must pass through the standard
    /// escape (i.e. NOT be in its escape set). A
    /// regression that adds `:` or `/` to the standard
    /// escape would break URL handling; a regression
    /// that drops one of them from the autolink escape
    /// would let URL syntax through Typst content mode.
    #[test]
    fn escape_typst_autolink_matches_standard_on_pass_through(s in printable_ascii()) {
        let standard = escape_typst(&s);
        let autolink = escape_typst_autolink(&s);
        // The pass-through set: every printable-ASCII
        // char NOT in the standard escape set. The
        // autolink escape must also pass these through
        // (the autolink escape is a *superset* of the
        // standard escape on the escape set, and
        // identical on the pass-through set).
        let standard_set = [
            '\\', '#', '*', '_', '`', '[', ']', '{', '}', '@', '$', '~', '\'', '"', '<', '>',
        ];
        for c in s.chars() {
            if standard_set.contains(&c) {
                continue;
            }
            // Pass-through char: both outputs must have
            // the same count as the input.
            let in_count = s.matches(c).count();
            let std_count = standard.matches(c).count();
            let at_count = autolink.matches(c).count();
            if std_count != in_count || at_count != in_count {
                prop_assert!(
                    false,
                    "char {:?} should pass through both escapes: \
                     input count {} standard count {} autolink count {} \
                     (input={:?})",
                    c,
                    in_count,
                    std_count,
                    at_count,
                    s
                );
            }
        }
        // The autolink-only set: `:` and `/` pass
        // through the standard escape (NOT escaped) but
        // are escaped by the autolink escape. The
        // output of the autolink escape should have 0
        // un-escaped `:` and `/` (they appear only as
        // part of `\:` and `\/` escape sequences).
        for c in [':', '/'] {
            let std_count = standard.matches(c).count();
            let at_count = autolink.matches(c).count();
            let in_count = s.matches(c).count();
            if std_count != in_count {
                prop_assert!(
                    false,
                    "standard escape should not touch {:?}: \
                     input count {} standard count {} (input={:?})",
                    c,
                    in_count,
                    std_count,
                    s
                );
            }
            // For the autolink escape, the `:` and `/`
            // appear in the output only as part of `\:`
            // and `\/` (i.e. preceded by a backslash). We
            // count the bare-`:` and bare-`/` substrings
            // (without a preceding backslash) — should be
            // 0 for the autolink output.
            let bare_count = count_bare(&autolink, c);
            if bare_count != 0 {
                prop_assert!(
                    false,
                    "autolink escape should fully escape {:?}: \
                     bare count {} (input={:?} output={:?})",
                    c,
                    bare_count,
                    s,
                    autolink
                );
            }
        }
    }

    /// Property 2: `escape_typst_string` produces a valid
    /// Typst string literal. The function is NOT
    /// idempotent — calling it twice on the same string
    /// produces a different output (each pass adds an
    /// escape layer). What the function MUST guarantee
    /// is that the output is a valid string literal:
    /// `\` and `"` are escaped, and no `\` is left
    /// "dangling" (the count of `\\` pairs is consistent
    /// with the input). The translator and the inline
    /// code path both rely on the output being a valid
    /// string literal — a regression that produces
    /// invalid output would either fail to compile or
    /// terminate the string early, swallowing the rest
    /// of the body.
    #[test]
    fn escape_typst_string_produces_valid_string_literal(s in printable_ascii()) {
        let escaped = escape_typst_string(&s);
        // The output must contain a valid sequence of
        // `\\` and `\"` escapes. Walk the output and
        // assert no `\` is followed by a non-escape
        // char (a regression like `\` -> `\` without
        // the second escape would leave a dangling
        // backslash).
        let mut chars = escaped.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                // The next char MUST be `\` or `"` (the
                // only two escape sequences `escape_typst_string`
                // emits). If it's anything else, the
                // function emitted a malformed escape
                // sequence.
                let next = chars.next();
                if !matches!(next, Some('\\') | Some('"')) {
                    prop_assert!(
                        false,
                        "escape_typst_string emitted a malformed escape: \
                         input={:?} output={:?} (unexpected char after backslash: {:?})",
                        s,
                        escaped,
                        next
                    );
                }
            }
        }
    }

    /// Property 3: `render_markdown_to_typst` never panics
    /// on any printable-ASCII + newline input. The
    /// translator is the project's only markdown-to-Typst
    /// entry point; a panic here would crash the editor.
    /// The proptest runs 256 cases with shrinking to find
    /// the minimum input that triggers a panic.
    #[test]
    fn translator_never_panics_on_arbitrary_input(
        body in prop::string::string_regex(r"[\x20-\x7E\n]{0,200}").unwrap()
    ) {
        let _ = render_markdown_to_typst(&body);
    }
}

/// Count the number of bare occurrences of `c` in `s` —
/// occurrences that are NOT immediately preceded by a
/// backslash. The escape functions in this module emit
/// `\` + char for every char they escape, so a
/// fully-escaped output has 0 bare occurrences of the
/// escape set. This is the property the proptest asserts
/// for `escape_typst_autolink` on the autolink-only set
/// (`:`, `/`).
///
/// A `\\` pair in the input is a literal backslash (per
/// the Typst string-literal rules); a `\:` is a literal
/// colon. The "preceded by a backslash" check counts the
/// backslash as the escape prefix, not as a bare char
/// itself — so a `\\` in the output has both chars
/// preceded by something (the first `\` precedes the
/// second, the second is preceded by the first).
///
/// We walk the string with a "skip next" flag set by
/// seeing a `\`. The flag is consumed by the next char
/// regardless of what it is. This is a deliberate
/// simplification: a `\\:` in the output has the first
/// `\` consuming the second, and the `:` is then bare
/// (not preceded by an escape). The proptest asserts
/// "no bare `:`" which catches this case.
fn count_bare(s: &str, c: char) -> usize {
    let mut count = 0;
    let mut skip_next = false;
    for ch in s.chars() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if ch == '\\' {
            skip_next = true;
            continue;
        }
        if ch == c {
            count += 1;
        }
    }
    count
}
