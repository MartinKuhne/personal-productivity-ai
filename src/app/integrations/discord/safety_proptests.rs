//! Property-based tests for the Discord safety filter in
//! `integrations::discord::safety::SafetyFilter`.
//!
//! The `SafetyFilter` is the last line of defense between
//! attacker-controlled content (a fetched markdown page, an
//! `@everyone` mention, a bidi-override string) and the
//! Discord channel output. A panic here would crash every
//! message; a regression that lets a known-bad pattern slip
//! through would let prompt-injection payloads reach the
//! chat.
//!
//! # Properties under test
//!
//! All four properties are sourced from the C11 corner-case
//! row in `doc/planning/fuzzing.md` §2.2 "Phase 2".
//!
//! 1. **`is_safe` never panics on any input.** Any byte string
//!    is checked without unwinding. The filter's pattern
//!    matcher must be total over the content alphabet.
//! 2. **An empty filter accepts everything.** A `SafetyFilter`
//!    with no patterns returns `Safe` for any content.
//! 3. **A registered pattern blocks matching content.** A
//!    filter with `patterns = ["bad"]` returns `Unsafe` for
//!    content containing `bad` and `Safe` for content
//!    without it (case-insensitive per the implementation).
//! 4. **Bidirectional override (U+202E) is matched as a
//!    control char.** The Discord security guidance
//!    explicitly calls out `U+202E` as a hostile character;
//!    a filter that misses it is a regression.
//!
//! `cases = 512` per property. The filter uses async
//! `read().await` so the tests use a single shared tokio
//! runtime.

use crate::integrations::discord::safety::SafetyFilter;
use proptest::prelude::*;

/// One proptest case count for every property in this sidecar.
const CASES: u32 = 512;

/// Single-threaded tokio runtime. The filter's `is_safe`
/// uses `Arc<RwLock<Vec<String>>>` internally with
/// `read().await`, so each test must run inside a tokio
/// runtime.
static RUNTIME: std::sync::LazyLock<tokio::runtime::Runtime> = std::sync::LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime for safety proptests")
});

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// `is_safe` must never panic on any input. The filter is
    /// async; we run it inside a tokio runtime.
    #[test]
    fn is_safe_never_panics_on_any_input(
        content in prop::string::string_regex(r"[\x00-\x7F\s]{0,1024}").unwrap()
    ) {
        let filter = SafetyFilter::new();
        let result = RUNTIME.block_on(async { filter.is_safe(&content).await });
        // The function returns a `SafetyResult` enum;
        // any content is either Safe or Unsafe, never
        // panics.
        let _ = result;
    }

    /// An empty filter accepts every content. The filter
    /// has no patterns to match against, so every check
    /// returns `Safe`.
    #[test]
    fn empty_filter_accepts_everything(
        content in prop::string::string_regex(r"[\x00-\x7F\s]{1,256}").unwrap()
    ) {
        let filter = SafetyFilter::new();
        let result = RUNTIME.block_on(async { filter.is_safe(&content).await });
        prop_assert!(result.is_safe(), "empty filter must accept any content");
    }

    /// A filter with a registered pattern blocks matching
    /// content. The check is case-insensitive (per the
    /// implementation's `to_lowercase()`), so the
    /// case of the pattern and the content do not matter.
    #[test]
    fn registered_pattern_blocks_matching_content(
        pattern in "[a-z]{4,8}",
        content in prop::string::string_regex(r"[A-Za-z0-9 ]{1,128}").unwrap()
    ) {
        let filter = SafetyFilter::with_patterns(vec![pattern.clone()]);
        // Build a content string that includes the pattern
        // (mixed case) somewhere in the middle.
        let mut with_pattern = String::from("prefix ");
        with_pattern.push_str(&content.to_uppercase());
        with_pattern.push(' ');
        with_pattern.push_str(&pattern);
        with_pattern.push_str(" suffix");
        let result = RUNTIME.block_on(async { filter.is_safe(&with_pattern).await });
        prop_assert!(!result.is_safe(), "content containing the pattern must be blocked");
    }

    /// A filter with a registered pattern does NOT block
    /// content that doesn't contain the pattern.
    #[test]
    fn registered_pattern_does_not_block_unrelated_content(
        pattern in "[a-z]{4,8}",
        content in prop::string::string_regex(r"[0-9]{1,32}").unwrap()
    ) {
        // Pure digits — guaranteed not to contain the
        // alphabetic pattern.
        let filter = SafetyFilter::with_patterns(vec![pattern]);
        let result = RUNTIME.block_on(async { filter.is_safe(&content).await });
        prop_assert!(result.is_safe(), "unrelated content must not be blocked");
    }

    /// The Discord security guidance calls out the
    /// bidirectional override character `U+202E` as a
    /// hostile marker. A filter that misses it is a
    /// regression. We register `U+202E` as a pattern and
    /// feed it content that contains the character.
    #[test]
    fn bidi_override_is_detected_when_registered(
        prefix in "[A-Za-z]{1,16}",
        suffix in "[A-Za-z]{1,16}",
    ) {
        let bidi = "\u{202e}"; // U+202E RIGHT-TO-LEFT OVERRIDE
        let filter = SafetyFilter::with_patterns(vec![bidi.to_string()]);
        let content = format!("{prefix}{bidi}{suffix}");
        let result = RUNTIME.block_on(async { filter.is_safe(&content).await });
        prop_assert!(!result.is_safe(), "U+202E must be detected when registered as a pattern");
    }

    /// A control character (NUL, BEL, etc.) does not crash
    /// the filter. The filter must accept arbitrary control
    /// characters in the content; a regression that treats
    /// them as a regex anchor or special character would
    /// panic.
    #[test]
    fn control_characters_do_not_crash_filter(
        control in prop::sample::select(vec![
            '\x00', '\x01', '\x07', '\x08', '\x0B', '\x0C',
            '\x1B', '\x7F',
        ]),
        prefix in "[A-Za-z]{1,16}",
        suffix in "[A-Za-z]{1,16}",
    ) {
        let filter = SafetyFilter::new();
        let mut content = prefix;
        content.push(control);
        content.push_str(&suffix);
        let result = RUNTIME.block_on(async { filter.is_safe(&content).await });
        // The filter has no patterns, so any input is Safe.
        prop_assert!(result.is_safe());
    }

    /// A `discord://` mention (`@everyone`, `@here`) does
    /// not crash the filter. The filter is a generic
    /// pattern matcher; the production code registers the
    /// actual `@everyone` pattern at startup. A regression
    /// that treats `@` as a regex metacharacter would
    /// panic.
    #[test]
    fn discord_mentions_do_not_crash_filter(
        mention in prop::sample::select(vec!["@everyone", "@here", "<@123456789012345678>"]),
    ) {
        let filter = SafetyFilter::new();
        let result = RUNTIME.block_on(async { filter.is_safe(mention).await });
        prop_assert!(result.is_safe());
    }
}
