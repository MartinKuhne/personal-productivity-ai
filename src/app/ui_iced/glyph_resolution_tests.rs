//! Tests for [`crate::ui_iced::glyph_resolution`]: the before/after
//! evidence for the first-frame font-registration race.
//!
//! # Process isolation
//!
//! The font system is one process-wide `OnceLock`. The race test measures
//! the *pre-registration* state, so it needs a process where nothing has
//! registered yet â€” which `cargo nextest` provides by running every test in
//! its own process ([RUST-002]). The test asserts that precondition rather
//! than assuming it, and says so if it does not hold.

use crate::ui_iced::fonts::{register_fonts, registration_attempted};
use crate::ui_iced::glyph_resolution::{
    PROBED_FONTS, Resolution, SYMBOL_CODEPOINTS, face_registered, handle_resolves_to_family,
    report, resolve, resolve_all,
};

/// The family name the embedded Phosphor-Regular.ttf actually declares
/// (its `name` records 1 / 4 / 6 are all `Phosphor`).
const PHOSPHOR_FAMILY: &str = "Phosphor";

/// A codepoint assigned to no script and in no shipped font.
const UNASSIGNED: char = '\u{0530}';

/// The two handles that must render real symbols (Phosphor is an icon font
/// and is asserted separately).
const TEXT_HANDLES: [(iced::Font, &str); 2] = [
    (iced::Font::DEFAULT, "default"),
    (iced::Font::MONOSPACE, "mono"),
];

/// Registration must have happened before the "after" probes mean anything.
fn with_fonts_registered() {
    let failure = register_fonts();
    assert!(
        registration_attempted(),
        "register_fonts() must latch so the probe reads the same font system the window does"
    );
    assert_eq!(
        failure, None,
        "every FastMD font collection must contribute faces"
    );
}

/// Every codepoint that failed to resolve, as report lines.
fn tofu(resolutions: &[Resolution]) -> Vec<String> {
    resolutions
        .iter()
        .filter(|r| !r.resolved)
        .map(|r| r.line())
        .collect()
}

#[test]
fn the_process_starts_with_registration_not_attempted() {
    // Precondition for `the_before_and_after_glyph_tables_...`. nextest runs
    // one test per process, so nothing has called `register_fonts` yet; if
    // this ever fails the before-state is unmeasurable in this process.
    assert!(
        !registration_attempted(),
        "this test needs a pristine font system; run it under `cargo nextest` \
         (one process per test) rather than a threaded libtest run"
    );
}

#[test]
fn the_before_and_after_glyph_tables_show_whether_the_first_frame_race_changes_resolution() {
    // --- BEFORE: nothing registered, exactly what the first frame would see
    // if registration were still deferred behind the iced scheduler.
    assert!(
        !registration_attempted(),
        "the before-state needs a pristine font system; run under `cargo nextest`"
    );
    let before = resolve_all();
    let before_text = report();

    // --- AFTER: the synchronous registration `build_state` performs.
    with_fonts_registered();
    let after = resolve_all();
    let after_text = report();

    // Diff the two tables on every (codepoint, handle) pair. `resolve_all`
    // is deterministic in order, so index `i` of `before` and of `after`
    // describe the same codepoint under the same handle.
    let mut differences: Vec<String> = Vec::new();
    for (b, a) in before.iter().zip(after.iter()) {
        assert_eq!(
            b.ch, a.ch,
            "before/after tables must describe the same codepoint"
        );
        assert_eq!(
            b.font, a.font,
            "before/after tables must use the same handle"
        );
        if (b.face.as_str(), b.glyph_id) != (a.face.as_str(), a.glyph_id) {
            differences.push(format!(
                "U+{:04X} {} {:<8} BEFORE {} [{}] gid={} -> AFTER {} [{}] gid={}",
                u32::from(a.ch),
                a.ch,
                a.font,
                b.face,
                b.post_script,
                b.glyph_id,
                a.face,
                a.post_script,
                a.glyph_id,
            ));
        }
    }
    assert_eq!(
        after.len(),
        SYMBOL_CODEPOINTS.len() * PROBED_FONTS.len(),
        "the tables must cover the full codepoint x handle matrix"
    );

    println!("=== BEFORE registration (first-frame state) ===\n{before_text}");
    println!("=== AFTER registration ===\n{after_text}");
    if differences.is_empty() {
        println!("=== DIFF ===\nno codepoint resolves to a different face after registration");
    } else {
        println!("=== DIFF ===\n{}", differences.join("\n"));
    }

    // The verdict this test exists to record: registration must not make
    // any symbol *worse*, and the after-state must be fully resolved.
    let after_text_only: Vec<Resolution> = after
        .iter()
        .filter(|r| TEXT_HANDLES.iter().any(|(_, l)| *l == r.font))
        .cloned()
        .collect();
    assert!(
        tofu(&after_text_only).is_empty(),
        "after registration every symbol must resolve under the text handles:\n{}",
        tofu(&after_text_only).join("\n")
    );
    let before_resolved = before
        .iter()
        .filter(|r| TEXT_HANDLES.iter().any(|(_, l)| *l == r.font))
        .filter(|r| r.resolved)
        .count();
    let after_resolved = after_text_only.iter().filter(|r| r.resolved).count();
    assert!(
        after_resolved >= before_resolved,
        "registration must not reduce coverage: before {before_resolved}, after {after_resolved}"
    );
}

#[test]
fn every_symbol_resolves_in_the_default_family_once_registered() {
    with_fonts_registered();

    let unresolved: Vec<String> = SYMBOL_CODEPOINTS
        .iter()
        .map(|ch| resolve(iced::Font::DEFAULT, "default", *ch))
        .filter(|r| !r.resolved)
        .map(|r| r.line())
        .collect();

    assert!(
        unresolved.is_empty(),
        "unresolved under Font::DEFAULT (tofu on screen):\n{}",
        unresolved.join("\n")
    );
}

#[test]
fn every_symbol_resolves_in_the_mono_family_once_registered() {
    with_fonts_registered();

    let unresolved: Vec<String> = SYMBOL_CODEPOINTS
        .iter()
        .map(|ch| resolve(iced::Font::MONOSPACE, "mono", *ch))
        .filter(|r| !r.resolved)
        .map(|r| r.line())
        .collect();

    assert!(
        unresolved.is_empty(),
        "unresolved under Font::MONOSPACE (tofu on screen):\n{}",
        unresolved.join("\n")
    );
}

#[test]
fn the_probe_actually_detects_an_unresolvable_codepoint() {
    // Without this the green tests above could be green because the probe
    // never reports a failure.
    with_fonts_registered();

    for (font, label) in PROBED_FONTS {
        let resolution = resolve(font, label, UNASSIGNED);
        assert!(
            !resolution.resolved,
            "the probe claimed {} renders U+{:04X} ({UNASSIGNED}) with a real \
             glyph â€” it resolved to {} gid={}, so the tofu check is not proving anything",
            label,
            u32::from(UNASSIGNED),
            resolution.face,
            resolution.glyph_id
        );
    }
}

#[test]
fn phosphor_supplies_none_of_the_symbol_codepoints() {
    // Phosphor is registered, but it is a Private-Use-Area icon font: it
    // must not be the face supplying any of the 14 user-visible symbols.
    with_fonts_registered();

    // Positive control first: the face must be in the database, and under
    // its real family name it must draw its own PUA bolt. Without this,
    // "supplies none of these" would pass for the wrong reason.
    assert!(
        face_registered(PHOSPHOR_FAMILY),
        "the Phosphor face must be in the database before this test means anything"
    );
    let bolt = crate::ui::strings::ICON_LIGHTNING
        .chars()
        .next()
        .expect("ICON_LIGHTNING is one codepoint");
    assert!(
        handle_resolves_to_family(PHOSPHOR_FAMILY, bolt),
        "Phosphor must draw its own PUA bolt U+{:04X}, or the probe is blind",
        u32::from(bolt)
    );

    let supplied: Vec<String> = SYMBOL_CODEPOINTS
        .iter()
        .filter(|ch| handle_resolves_to_family(PHOSPHOR_FAMILY, **ch))
        .map(|ch| format!("U+{:04X} {ch}", u32::from(*ch)))
        .collect();

    assert!(
        supplied.is_empty(),
        "Phosphor is not doing the symbol work; these codepoints must not come \
         from the Phosphor face: {}",
        supplied.join(", ")
    );
}

#[test]
fn the_phosphor_registration_key_matches_the_fonts_own_declared_family() {
    // The registration key MUST equal the family name the TTF declares
    // (name records 1/4/6 all read "Phosphor"). cosmic-text matches
    // case-sensitively, so any other spelling misses the embedded face and
    // silently falls through to the platform fallback — the icon still
    // draws, which is why the mismatch is invisible without comparing glyphs.
    with_fonts_registered();
    let bolt = crate::ui::strings::ICON_LIGHTNING
        .chars()
        .next()
        .expect("ICON_LIGHTNING is one codepoint");

    assert_eq!(
        crate::ui_iced::fonts::PHOSPHOR_FONT_NAME,
        PHOSPHOR_FAMILY,
        "the registration key must equal the family the font declares"
    );
    assert!(
        face_registered(crate::ui_iced::fonts::PHOSPHOR_FONT_NAME),
        "the face is registered under the key the app asks for"
    );
    assert!(
        handle_resolves_to_family(crate::ui_iced::fonts::PHOSPHOR_FONT_NAME, bolt),
        "the app's own handle must actually reach the Phosphor face"
    );
}

#[test]
fn the_report_is_byte_stable_across_calls() {
    with_fonts_registered();

    assert_eq!(
        report(),
        report(),
        "the resolution report must not depend on call order or caching"
    );
}

#[test]
fn the_report_is_printable_for_a_human_to_read() {
    with_fonts_registered();

    let text = report();
    println!("{text}");
    assert!(
        text.contains("glyph resolution:") && text.contains("U+2713"),
        "the report looks wrong:\n{text}"
    );
}
