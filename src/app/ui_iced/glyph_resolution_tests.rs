//! Tests for [`crate::ui_iced::glyph_resolution`]: what each shipped font
//! resolves, and what the first-frame registration race changes.
//!
//! # Process isolation
//!
//! Font registration is one process-wide `OnceLock` that cannot be un-latched,
//! and loaded faces cannot be unloaded, so the *pre*-registration state exists
//! only in a process that has never registered. The lib-test binary shares one
//! process across every `fastmd` test and `fonts_tests` registers in it, so the
//! race test cannot simply assert a pristine state — it spawns this binary
//! filtered to its own entry point, which yields a fresh process under every
//! runner (`cargo nextest`, `cargo test` and `cargo llvm-cov` all isolate by
//! process). Every other test here measures the post-registration state.

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

/// Printed by [`race_measurement_child_entry_point`] when it really did run
/// against a pristine font system. The parent test refuses to count a run
/// without it as evidence, so a child that quietly did nothing cannot pass.
const PRISTINE_MARKER: &str = "glyph-race:pristine";

/// libtest path of [`race_measurement_child_entry_point`], as the parent must
/// name it to select only that test.
const CHILD_TEST_PATH: &str =
    "ui_iced::glyph_resolution::tests::race_measurement_child_entry_point";

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

/// The text handles only; Phosphor is an icon font and is asserted separately.
fn text_handles(resolutions: &[Resolution]) -> Vec<Resolution> {
    resolutions
        .iter()
        .filter(|r| TEXT_HANDLES.iter().any(|(_, label)| *label == r.font))
        .cloned()
        .collect()
}

/// The before/after measurement, taken in whatever process calls this.
///
/// It is evidence only when nothing has registered fonts yet, so when the lock
/// is already latched it prints why it is abstaining and returns without
/// asserting anything. [`race_measurement_child_entry_point`] is reached from a
/// freshly spawned process, which is where that precondition holds.
fn measure_the_first_frame_race() {
    if registration_attempted() {
        println!("glyph-race:abstained (fonts were already registered in this process)");
        return;
    }

    // --- BEFORE: nothing registered, exactly what the first frame would see
    // if registration were still deferred behind the iced scheduler.
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
    let after_text_only = text_handles(&after);
    assert!(
        tofu(&after_text_only).is_empty(),
        "after registration every symbol must resolve under the text handles:\n{}",
        tofu(&after_text_only).join("\n")
    );
    let before_resolved = text_handles(&before).iter().filter(|r| r.resolved).count();
    let after_resolved = after_text_only.iter().filter(|r| r.resolved).count();
    assert!(
        after_resolved >= before_resolved,
        "registration must not reduce coverage: before {before_resolved}, after {after_resolved}"
    );

    println!("{PRISTINE_MARKER}");
}

/// Entry point for the spawned process in
/// [`the_before_and_after_glyph_tables_show_whether_the_first_frame_race_changes_resolution`].
///
/// Selected by name, it runs alone in a fresh process and so measures the
/// pristine state. When the whole suite reaches it instead, the lock may already
/// be latched and it abstains — the parent is what turns a real measurement into
/// an assertion.
#[test]
fn race_measurement_child_entry_point() {
    measure_the_first_frame_race();
}

/// The first-frame registration race, measured in a process of its own.
///
/// Font registration is a process-wide `OnceLock` and faces cannot be unloaded,
/// so the pre-registration table can only be read before anything registers.
/// Spawning this very binary filtered to [`CHILD_TEST_PATH`] guarantees that:
/// every runner — `cargo nextest`, `cargo test`, and `cargo llvm-cov`, which
/// shares one process among all lib tests — gives each process a font system
/// nothing has touched yet.
#[test]
fn the_before_and_after_glyph_tables_show_whether_the_first_frame_race_changes_resolution() {
    let this_binary = std::env::current_exe().expect("a test binary knows its own path");
    let child = std::process::Command::new(this_binary)
        .args(["--exact", CHILD_TEST_PATH, "--nocapture"])
        .output()
        .expect("spawning a fresh test process must be possible");

    let stdout = String::from_utf8_lossy(&child.stdout);
    let stderr = String::from_utf8_lossy(&child.stderr);
    assert!(
        child.status.success(),
        "the pristine measurement failed:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains(PRISTINE_MARKER),
        "the spawned process abstained instead of measuring, so nothing was proven. \
         It must have found fonts already registered, which cannot happen in a fresh \
         process.\nstdout:\n{stdout}\nstderr:\n{stderr}"
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
             glyph Ã¢â‚¬â€ it resolved to {} gid={}, so the tofu check is not proving anything",
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
fn tree_disclosure_glyphs_resolve_from_the_phosphor_face() {
    with_fonts_registered();

    for (icon, label) in [
        (crate::ui::strings::ICON_CARET_RIGHT, "collapsed"),
        (crate::ui::strings::ICON_CARET_DOWN, "expanded"),
    ] {
        let mut codepoints = icon.chars();
        let codepoint = codepoints.next().expect("caret icon is not empty");
        assert!(codepoints.next().is_none(), "caret icon is one codepoint");

        let resolution = resolve(
            iced::Font::with_name(PHOSPHOR_FAMILY),
            "phosphor",
            codepoint,
        );
        assert!(
            resolution.resolved && resolution.face == PHOSPHOR_FAMILY,
            "the {label} tree caret U+{:04X} must resolve through Phosphor, got {} [{}] gid={}",
            u32::from(codepoint),
            resolution.face,
            resolution.post_script,
            resolution.glyph_id
        );
    }
}

#[test]
fn the_phosphor_registration_key_matches_the_fonts_own_declared_family() {
    // The registration key MUST equal the family name the TTF declares
    // (name records 1/4/6 all read "Phosphor"). cosmic-text matches
    // case-sensitively, so any other spelling misses the embedded face and
    // silently falls through to the platform fallback â€” the icon still
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
