//! Property-based tests for the PKCE / `state` generators in
//! `integrations::mcp::oauth::pkce`.
//!
//! The PKCE code-verifier and OAuth `state` parameter are the
//! two pieces of CSPRNG-driven entropy that an attacker must
//! guess to forge a successful authorization-code-flow callback.
//! A bug in the generator — wrong alphabet, wrong length,
//! non-random output, even just an off-by-one in the entropy
//! pool — is a complete bypass of the security guarantees PKCE
//! and CSRF protection are supposed to provide.
//!
//! # Properties under test
//!
//! All four properties are sourced from the C3 corner-case
//! row in `doc/planning/fuzzing.md` §2.2 "Phase 2".
//!
//! 1. **Output is URL-safe.** Every byte of the verifier and
//!    the challenge is in the `URL_SAFE_NO_PAD` base64
//!    alphabet: `[A-Za-z0-9_-]`. No `+`, `/`, `=`, or any
//!    other byte leaks into the output.
//! 2. **Output length is exact.** A 32-byte input encodes to
//!    exactly 43 base64-no-pad characters. Both the verifier
//!    and the challenge MUST be 43 chars; the `state`
//!    parameter MUST also be 43 chars.
//! 3. **Method is "S256".** PKCE is required to be `S256`
//!    per MCP spec §4.9 — `plain` is forbidden. A regression
//!    that lets the method change would weaken PKCE to a
//!    dictionary-attack-vulnerable form.
//! 4. **Uniqueness across calls.** Two `generate()` calls in
//!    the same process must return different values; two
//!    `State::generate()` calls must also return different
//!    values. The probability of an accidental collision on
//!    256 bits of entropy is ~10^-77; even at 512 proptest
//!    cases the property holds.
//!
//! `cases = 512` per property. We use the production
//! `SystemRandom` CSPRNG; this is fast and the high entropy
//! guarantees the invariants hold on every case.

use crate::integrations::mcp::oauth::pkce::{PkcePair, State, s256};
use proptest::prelude::*;

/// One proptest case count for every property in this sidecar.
const CASES: u32 = 512;

/// Per RFC 4648 §5, the URL-safe-no-pad base64 alphabet.
/// `+` and `/` from standard base64 are replaced with `-`
/// and `_`; padding `=` is omitted.
const URL_SAFE_NO_PAD_ALPHABET: &str =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// The PKCE verifier is always 43 chars of URL-safe-no-pad
    /// base64. The 32-byte CSPRNG output encodes to exactly 43
    /// characters in that alphabet.
    #[test]
    fn pkce_verifier_is_43_url_safe_chars(_unused in 0..1u8) {
        let pair = PkcePair::generate();
        prop_assert_eq!(pair.verifier.len(), 43, "verifier must be 43 chars");
        for c in pair.verifier.chars() {
            prop_assert!(
                URL_SAFE_NO_PAD_ALPHABET.contains(c),
                "verifier char {c:?} is outside URL-safe-no-pad alphabet"
            );
        }
    }

    /// The PKCE challenge is always 43 chars of URL-safe-no-pad
    /// base64 (the SHA-256 digest is 32 bytes; same encoding
    /// rules as the verifier).
    #[test]
    fn pkce_challenge_is_43_url_safe_chars(_unused in 0..1u8) {
        let pair = PkcePair::generate();
        prop_assert_eq!(pair.challenge.len(), 43, "challenge must be 43 chars");
        for c in pair.challenge.chars() {
            prop_assert!(
                URL_SAFE_NO_PAD_ALPHABET.contains(c),
                "challenge char {c:?} is outside URL-safe-no-pad alphabet"
            );
        }
    }

    /// The PKCE method is always `"S256"`. MCP spec §4.9
    /// forbids `plain`.
    #[test]
    fn pkce_method_is_s256(_unused in 0..1u8) {
        let pair = PkcePair::generate();
        prop_assert_eq!(pair.method, "S256");
    }

    /// The challenge is the SHA-256 of the verifier, encoded
    /// the same way. This is the fundamental PKCE invariant;
    /// a regression that decoupled them would break the
    /// authorization flow.
    #[test]
    fn pkce_challenge_equals_s256_of_verifier(_unused in 0..1u8) {
        let pair = PkcePair::generate();
        let expected_challenge = s256(&pair.verifier);
        prop_assert_eq!(&pair.challenge, &expected_challenge);
    }

    /// Two `generate()` calls produce distinct verifiers. The
    /// property is statistical, but with 256 bits of entropy
    /// the probability of an accidental collision is
    /// negligible even across 512 cases.
    #[test]
    fn pkce_pair_uniqueness(_unused in 0..1u8) {
        let p1 = PkcePair::generate();
        let p2 = PkcePair::generate();
        prop_assert_ne!(&p1.verifier, &p2.verifier);
        prop_assert_ne!(&p1.challenge, &p2.challenge);
    }

    /// The `state` parameter is 43 chars of URL-safe-no-pad
    /// base64. Same entropy and alphabet guarantees as the
    /// PKCE verifier.
    #[test]
    fn state_is_43_url_safe_chars(_unused in 0..1u8) {
        let s = State::generate();
        prop_assert_eq!(s.as_str().len(), 43, "state must be 43 chars");
        for c in s.as_str().chars() {
            prop_assert!(
                URL_SAFE_NO_PAD_ALPHABET.contains(c),
                "state char {c:?} is outside URL-safe-no-pad alphabet"
            );
        }
    }

    /// Two `State::generate()` calls produce distinct values.
    /// The CSRF protection on the redirect callback depends
    /// on this; a regression that re-used state would let an
    /// attacker replay a previous successful callback.
    #[test]
    fn state_uniqueness(_unused in 0..1u8) {
        let s1 = State::generate();
        let s2 = State::generate();
        prop_assert_ne!(s1.as_str(), s2.as_str());
    }

    /// `s256` is a pure function: the same input always
    /// produces the same output. A regression that introduced
    /// non-determinism here would break both PKCE and the
    /// test vector in `pkce.rs`.
    #[test]
    fn s256_is_deterministic(
        input in prop::string::string_regex(r"[A-Za-z0-9_\-]{1,128}").unwrap()
    ) {
        let h1 = s256(&input);
        let h2 = s256(&input);
        prop_assert_eq!(&h1, &h2);
    }

    /// `s256` output is always 43 chars of URL-safe-no-pad
    /// base64 (a SHA-256 hash is 32 bytes; same encoding).
    #[test]
    fn s256_output_is_43_url_safe_chars(
        input in prop::string::string_regex(r"[A-Za-z0-9_\-]{1,128}").unwrap()
    ) {
        let h = s256(&input);
        prop_assert_eq!(h.len(), 43);
        for c in h.chars() {
            prop_assert!(
                URL_SAFE_NO_PAD_ALPHABET.contains(c),
                "s256 char {c:?} is outside URL-safe-no-pad alphabet"
            );
        }
    }
}
