//! Tests for [`crate::glyph_resolution`].

use hoarfrost::token::typography::Family;

use crate::font_fallback::{register, registration_attempted};
use crate::glyph_resolution::{PROBED_FAMILIES, SYMBOL_CODEPOINTS, report, resolve, resolve_all};

/// A codepoint assigned to no script and in no shipped font.
const UNASSIGNED: char = '\u{0530}';

/// Registration must have happened before the probe means anything.
fn with_fallback_registered() {
    register();
    assert!(
        registration_attempted(),
        "register() must latch so the probe reads the same font system the window does"
    );
}

#[test]
fn every_symbol_codepoint_resolves_in_the_sans_family() {
    with_fallback_registered();

    let tofu: Vec<String> = SYMBOL_CODEPOINTS
        .iter()
        .map(|ch| resolve(Family::Sans, "font.family.sans", *ch))
        .filter(|r| !r.resolved)
        .map(|r| r.line())
        .collect();

    assert!(
        tofu.is_empty(),
        "unresolved in font.family.sans (tofu on screen):\n{}",
        tofu.join("\n")
    );
}

#[test]
fn every_symbol_codepoint_resolves_in_the_mono_family() {
    with_fallback_registered();

    let tofu: Vec<String> = SYMBOL_CODEPOINTS
        .iter()
        .map(|ch| resolve(Family::Mono, "font.family.mono", *ch))
        .filter(|r| !r.resolved)
        .map(|r| r.line())
        .collect();

    assert!(
        tofu.is_empty(),
        "unresolved in font.family.mono (tofu on screen):\n{}",
        tofu.join("\n")
    );
}

#[test]
fn the_probe_actually_detects_an_unresolvable_codepoint() {
    // Without this the two green tests above could be green because the
    // probe never reports a failure.
    with_fallback_registered();

    for (family, token) in PROBED_FAMILIES {
        let resolution = resolve(family, token, UNASSIGNED);
        assert!(
            !resolution.resolved,
            "the probe claimed {} renders U+{:04X} ({UNASSIGNED}) with a real \
             glyph — it resolved to {} gid={}, so the tofu check is not proving anything",
            token,
            u32::from(UNASSIGNED),
            resolution.face,
            resolution.glyph_id
        );
    }
}

#[test]
fn the_report_covers_every_codepoint_under_both_families() {
    with_fallback_registered();

    let resolutions = resolve_all();
    assert_eq!(
        resolutions.len(),
        SYMBOL_CODEPOINTS.len() * PROBED_FAMILIES.len(),
        "the report must cover the full codepoint x family matrix"
    );

    for (family, token) in PROBED_FAMILIES {
        let under_token: Vec<char> = resolutions
            .iter()
            .filter(|r| r.token == token)
            .map(|r| r.ch)
            .collect();
        assert_eq!(
            under_token,
            SYMBOL_CODEPOINTS.to_vec(),
            "{token} must report every symbol codepoint, in order"
        );
        assert_eq!(
            under_token
                .iter()
                .filter(|&&ch| resolve(family, token, ch).resolved)
                .count(),
            SYMBOL_CODEPOINTS.len(),
            "{token} must resolve all {n} symbols",
            n = SYMBOL_CODEPOINTS.len()
        );
    }
}

#[test]
fn the_report_is_byte_stable_across_calls() {
    with_fallback_registered();

    assert_eq!(
        report(),
        report(),
        "the resolution report must not depend on call order or caching"
    );
}

#[test]
fn the_report_is_printable_for_a_human_to_read() {
    with_fallback_registered();

    let text = report();
    println!("{text}");
    assert!(
        text.contains("glyph resolution:") && text.contains("U+2713"),
        "the report looks wrong:\n{text}"
    );
}
