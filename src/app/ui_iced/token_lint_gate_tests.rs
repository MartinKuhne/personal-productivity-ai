//! Tests for the app-side token gate in
//! [`token_lint_gate`](crate::ui_iced::token_lint_gate).
//!
//! Same two failure modes the iced-pro sidecar defends against: the gate
//! can let a magic number through, and — more dangerously — it can
//! *report* that nothing got through because a pattern quietly stopped
//! matching. The pinned counts are the anti-vacuity half; the injected
//! fixtures are the proof it still bites.
//!
//! The injected-failure tests follow the house pattern from
//! [`token_lint_tests`](iced_pro::token_lint_tests): the real code path,
//! one doctored input, the guard observed firing on every run rather than
//! trusted.
//!
//! `assertions_on_constants` is allowed throughout: pinning token-bound
//! constants and scope literals is exactly what that lint fires on, and
//! pinning them is the point of this file.
#![allow(clippy::assertions_on_constants)]

use std::path::{Path, PathBuf};

use iced_pro::token_lint::{self, Rule, is_test_file};

use crate::ui_iced::token_lint_gate::{
    APP_ALLOW_LIST, APP_COLOUR_DEFINITION_FILE, APP_GEOMETRY_CONSTANTS, APP_VISUAL_SCOPE_PREFIXES,
    app_scope, app_source_root, render_app, scan_app,
};

// ---------------------------------------------------------------------------
// Pinned counts
//
// Not aspirations. These are what the gate reports against the tree as it
// stands, and they move only when the tree moves deliberately.
// ---------------------------------------------------------------------------

/// Production `.rs` files the gate reads under `src/app` (fixtures excluded).
///
/// 157 through INT-8, 158 after INT-9 added
/// `ui_iced/menu_aim.rs` — the app-side driver for the submenu aiming
/// corridor. The file declares no visual scalar of its own (its only
/// number is the `u64` millisecond count it hands to the sleeper, which
/// is a duration read from `time.delay.submenu`, not a measurement), so
/// `PIN_APP_TOKEN_BOUND` and `PIN_APP_SURFACE_BOUND` do not move with it.
const PIN_APP_FILES_SCANNED: usize = 158;
/// Files treated as the app's colour-definition layer.
const PIN_APP_RULE_A_FILES: usize = 1;
/// Files under the app's visual scope, i.e. everything in `ui_iced/`.
///
/// 44 through INT-8, 45 after INT-9's `ui_iced/menu_aim.rs` landed in
/// the scope. The count is files-in-scope, not files-with-findings: the
/// new module contributes a scanned file and no violation.
const PIN_APP_RULE_B_FILES: usize = 45;
/// Colour constructors found inside `ui/theme.rs`.
///
/// The anti-vacuity half of Rule A: without this, "zero colour literals
/// outside the theme layer" is indistinguishable from "the pattern is
/// broken and found nothing".
const PIN_APP_COLOUR_DEFINITIONS: usize = 8;
/// App visual scalars reading a token path.
///
/// 73 before INT-8 landed the gate, 89 after: the sixteen constants that
/// carried a bare literal at a value a token already supplies
/// (`14.0` → `typography::BODY.size`, `1.0` → `shape::BORDER_HAIRLINE`,
/// `12.0` → `space::MD`, `16.0` → `space::LG`, `4.0` → `space::XS`, the
/// two derived tab-strip heights, the off-scale `18.0` title and the
/// hand-derived `× 1.3` JSON line height) now read a rung instead.
/// 91 after INT-11's two additions in `panels/center.rs`:
/// `UNSAVED_DOT_RADIUS` (`shape::RADIUS_PILL`, the round-clip on the
/// `6px` marker) and `TAB_INDICATOR_HEIGHT` (`shape::BORDER_FOCUS`, the
/// `2px` top accent bar). `UNSAVED_DOT_SIZE` also changed — `12.0` →
/// `size::INDICATOR_DOT` — but Rule B counts declarations, so moving a
/// constant from the type floor onto the indicator token does not move
/// this number.
/// 93 after INT-12's badge geometry in `panels/left.rs`:
/// `PDF_BADGE_SLOT` (`space.xl + space.md`, the width the `[PDF]` mark
/// owns) and `PDF_BADGE_GAP` (`space.xs`, the gap in front of it).
/// `PDF_BADGE_CHARGE` is derived from those two rather than naming a token
/// of its own, and `PDF_BADGE_RUNG` re-exports the badge atom's rung, so
/// neither adds to this count.
/// 97 after the same increment's badge became a chip per `§3B.6:883`,
/// which gave the mark a box of its own: `PDF_BADGE_TEXT_ALLOWANCE`
/// (`space.lg + space.xs`), `PDF_BADGE_CHIP_HEIGHT` (`badge.COUNT_HEIGHT`,
/// the pill rung that fits the 20px row interior), `PDF_BADGE_CHIP_RADIUS`
/// (`shape.radius.pill`), `PDF_BADGE_CHIP_H_PADDING` (`badge.CHIP_H_PADDING`)
/// and `PDF_BADGE_CHIP_PADDING_TOTAL` (both side paddings named together).
/// `PDF_BADGE_SLOT` stays in the count and keeps its value at `36` — it is
/// now composed from those chip metrics rather than from the spacing scale,
/// so the fit tests, the label-floor arithmetic and `PDF_BADGE_CHARGE` are
/// all unmoved.
/// 96 after INT-14 taught the parser to read a wrapped declaration's
/// expression instead of approving the empty string it used to read.
/// `TAG_TRIGGER_VERTICAL_PADDING` (`panels/top.rs`) wraps onto a second
/// line and its RHS carries `/ 2.0`. Under the old parser the expression
/// read empty, `numeric_literals` found nothing, and the constant was
/// counted `token-bound` without its value ever being looked at. Read
/// properly it is an unbound-scalar finding, so it leaves this count and
/// is carried on `APP_ALLOW_LIST` with the reason. Nothing was re-tuned:
/// the constant's value is byte-for-byte what it was.
/// 98 as of `UIF-MOD-004`, which adds two genuinely token-bound visual
/// constants to `ui_iced/attributions.rs`: `HEADER_BAND_HEIGHT` and
/// `FOOTER_BAND_HEIGHT`. Both are pure sums of `typography::` / `shape::` /
/// `size::` / `space::` tokens with **no numeric literal anywhere** — the
/// gap counts are written out as repeated `space::SM` terms rather than as
/// `3.0 *` / `2.0 *`, because a coefficient is a literal and would make
/// the constant a finding. Neither is registered, neither is allow-listed,
/// and neither was re-tuned: they pass because every term is a token.
const PIN_APP_TOKEN_BOUND: usize = 98;
/// App visual scalars covered by the app's own geometry registry.
const PIN_APP_SURFACE_BOUND: usize = 3;
/// Exceptions the app currently takes.
///
/// 12 through INT-13. 13 after INT-14: `panels/top.rs:TAG_TRIGGER_VERTICAL_PADDING`
/// became visible once wrapped declarations were actually read, and its
/// `/ 2.0` is a centring divisor rather than a measurement. It is excused
/// rather than fixed because every alternative is worse — binding the
/// divisor to a spacing rung would call a non-spacing value a spacing
/// value, and collapsing the expression to the `space.xs` it yields today
/// would throw away the rung-derivation the constant exists to carry. The
/// entry's own text states that, and the non-vacuity assertion below proves
/// the entry is suppressing a real finding rather than padding the list.
const PIN_APP_ALLOW_ENTRIES: usize = 13;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// An independent recursive `.rs` walk, so the walker is checked against
/// something that is not the walker.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    for d in dirs {
        walk(&d, out);
    }
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

#[test]
fn the_app_passes_the_token_gate() {
    let audit = scan_app();
    assert!(
        audit.is_clean(),
        "app token gate failed over src/app:\n{}",
        audit.describe()
    );
}

#[test]
fn the_allow_list_is_small_and_every_entry_actually_fires() {
    let audit = scan_app();

    // Widening the allow-list is the easy way to make this file go green,
    // so the size is pinned: adding an entry is a deliberate act that
    // shows up here, not a quiet fix.
    assert_eq!(
        APP_ALLOW_LIST.len(),
        PIN_APP_ALLOW_ENTRIES,
        "the app allow-list has {} entries; the pinned count is \
         {PIN_APP_ALLOW_ENTRIES}. Fix the finding rather than excuse it.",
        APP_ALLOW_LIST.len(),
    );

    // Every entry must suppress something real. A stale entry is a lie
    // about the state of the tree and fails on its own.
    assert_eq!(
        audit.suppressed.len(),
        APP_ALLOW_LIST.len(),
        "every allow entry must fire; stale: {:?}",
        audit.stale_allow_entries,
    );
    assert!(
        audit.stale_allow_entries.is_empty(),
        "stale allow entries suppress nothing: {:?}",
        audit.stale_allow_entries,
    );

    // Each entry carries a distinct, substantive reason — a blanket "it's
    // fine" repeated down the list is not a justification.
    let mut reasons: Vec<&str> = APP_ALLOW_LIST.iter().map(|e| e.reason).collect();
    for entry in APP_ALLOW_LIST {
        assert!(
            entry.reason.trim().len() > 80,
            "allow entry `{}` needs a real reason, got {:?}",
            entry.key,
            entry.reason,
        );
        assert!(
            entry.key.contains('/') && entry.key.contains(':'),
            "allow key must be `file:identity`, got {}",
            entry.key,
        );
        assert!(
            entry.key.starts_with("ui_iced/"),
            "every app allow entry must live inside the gated tree, got {}",
            entry.key,
        );
    }
    reasons.sort_unstable();
    reasons.dedup();
    assert_eq!(
        reasons.len(),
        APP_ALLOW_LIST.len(),
        "reasons must be distinct"
    );
}

/// No allow entry may cover a whole directory or a whole file.
///
/// The brief for this gate is explicit that an enumerated list is fine and
/// a blanket exclusion is not. A key is `file:CONST_NAME`, so an entry
/// that ends at a `/` or names no constant would be a directory pardon.
#[test]
fn no_allow_entry_is_a_blanket_exclusion() {
    for entry in APP_ALLOW_LIST {
        let (_, identity) = entry
            .key
            .split_once(':')
            .unwrap_or_else(|| panic!("allow key must be `file:identity`, got {}", entry.key));
        assert!(
            !identity.is_empty() && !identity.contains('/'),
            "allow entry `{}` excuses a path rather than a named constant",
            entry.key,
        );
        assert!(
            identity
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
            "allow identity must name a constant, got {identity:?} in {}",
            entry.key,
        );
    }
}

// ---------------------------------------------------------------------------
// Anti-vacuity: the gate reads what it claims to read
// ---------------------------------------------------------------------------

#[test]
fn the_gate_reads_every_production_file_and_agrees_with_an_independent_walk() {
    let audit = scan_app();
    assert_eq!(audit.stats.files_scanned, PIN_APP_FILES_SCANNED);

    let root = app_source_root();
    let mut all = Vec::new();
    walk(&root, &mut all);
    let expected: Vec<String> = all
        .iter()
        .map(|p| rel(&root, p))
        .filter(|r| !is_test_file(r))
        .collect();

    assert_eq!(
        audit.stats.files_scanned,
        expected.len(),
        "the gate read {} files but an independent walk finds {} non-test .rs files under {root:?}",
        audit.stats.files_scanned,
        expected.len(),
    );
    assert!(
        !expected.is_empty(),
        "the independent walk found nothing — the fixture is broken, not clean"
    );
}

#[test]
fn the_gate_scopes_the_app_the_way_it_claims() {
    let audit = scan_app();
    assert_eq!(audit.stats.rule_a_files, PIN_APP_RULE_A_FILES);
    assert_eq!(audit.stats.rule_b_files, PIN_APP_RULE_B_FILES);
    assert_eq!(audit.stats.token_bound, PIN_APP_TOKEN_BOUND);
    assert_eq!(audit.stats.surface_bound, PIN_APP_SURFACE_BOUND);
    assert_eq!(
        audit.stats.token_layer_colour_definitions,
        PIN_APP_COLOUR_DEFINITIONS
    );

    assert_eq!(APP_COLOUR_DEFINITION_FILE, "ui/theme.rs");
    assert_eq!(APP_VISUAL_SCOPE_PREFIXES, ["ui_iced/"]);

    let scope = app_scope();
    assert!(token_lint::in_scope(
        "ui_iced/panels/center.rs",
        scope.visual_scope_prefixes
    ));
    assert!(token_lint::in_scope(
        "ui_iced/render/yaml.rs",
        scope.visual_scope_prefixes
    ));
    assert!(!token_lint::in_scope(
        "ui/theme.rs",
        scope.visual_scope_prefixes
    ));
    assert!(!token_lint::in_scope(
        "markdown/parser.rs",
        scope.visual_scope_prefixes
    ));
    assert!(!token_lint::in_scope(
        "bus/router/mod.rs",
        scope.visual_scope_prefixes
    ));
}

/// The single most important assertion for Rule A on this side of the
/// boundary: the scanner really does find colours *inside* the theme layer.
/// Without it, "zero colour literals outside `ui/theme.rs`" cannot be
/// told apart from "the pattern stopped matching".
#[test]
fn the_gate_actually_counts_colours_in_the_theme_layer() {
    let audit = scan_app();
    assert!(
        audit.stats.token_layer_colour_definitions > 0,
        "Rule A found no colour constructors in {APP_COLOUR_DEFINITION_FILE} at all — the \
         pattern is broken, not the tree clean"
    );
    assert_eq!(
        audit.stats.token_layer_colour_definitions, PIN_APP_COLOUR_DEFINITIONS,
        "the theme layer's colour-definition count moved; if colours left `ui/theme.rs`, they \
         did not go to a token"
    );
}

/// The gate must not be a second scanner that has drifted from the first.
///
/// The same fixture text, run under iced-pro's scope and under the app's,
/// must produce the *same* finding for the same shape. If the two scopes
/// ever disagree about what a violation is, the shared implementation has
/// been forked in practice even though the code is shared.
#[test]
fn the_app_scope_and_the_framework_scope_agree_on_the_same_shape() {
    let fixture = "pub const INJECTED_STROKE: f32 = 7.0;\n";

    let pro = token_lint::scan_source_in(&token_lint::crate_scope(), "widget/injected.rs", fixture);
    let app = token_lint::scan_source_in(&app_scope(), "ui_iced/injected.rs", fixture);

    assert_eq!(pro.findings.len(), 1, "framework scope must fire: {pro:?}");
    assert_eq!(app.findings.len(), 1, "app scope must fire: {app:?}");
    assert_eq!(
        pro.findings[0].rule, app.findings[0].rule,
        "the two scopes disagree about what a violation is"
    );
    assert_eq!(
        pro.findings[0].constant_name(),
        app.findings[0].constant_name(),
    );
    assert_eq!(
        pro.findings[0].detail, app.findings[0].detail,
        "the violation descriptions have drifted"
    );
}

// ---------------------------------------------------------------------------
// Real-tree injection — the house pattern
// ---------------------------------------------------------------------------

#[test]
fn the_gate_fires_on_a_scalar_injected_into_a_real_app_file() {
    let root = app_source_root();
    let path = root.join("ui_iced").join("panels").join("center.rs");
    let text = std::fs::read_to_string(&path).expect("center.rs must be readable");

    let baseline = token_lint::scan_source_in(&app_scope(), "ui_iced/panels/center.rs", &text);
    assert!(
        baseline.findings.is_empty(),
        "the real file must be clean before injection: {:?}",
        baseline.findings
    );

    let injected = format!("{text}\npub const INJECTED_STROKE: f32 = 7.0;\n");
    let after = token_lint::scan_source_in(&app_scope(), "ui_iced/panels/center.rs", &injected);
    assert_eq!(
        after.findings.len(),
        1,
        "exactly the injected constant must be new: {:?}",
        after.findings
    );
    assert_eq!(after.findings[0].rule, Rule::UnboundVisualScalar);
    assert_eq!(after.findings[0].constant_name(), Some("INJECTED_STROKE"));
    assert_eq!(after.token_bound, baseline.token_bound);
    assert_eq!(after.surface_bound, baseline.surface_bound);
}

#[test]
fn the_gate_fires_on_a_colour_injected_into_a_real_app_file() {
    let root = app_source_root();
    let path = root.join("ui_iced").join("panels").join("center.rs");
    let text = std::fs::read_to_string(&path).expect("center.rs must be readable");

    let baseline = token_lint::scan_source_in(&app_scope(), "ui_iced/panels/center.rs", &text);
    assert!(
        baseline.findings.is_empty(),
        "the real file must be clean before injection: {:?}",
        baseline.findings
    );

    let injected =
        format!("{text}\nfn injected() {{ let c = iced::Color::from_rgb8(9, 9, 9, 1.0); }}\n");
    let after = token_lint::scan_source_in(&app_scope(), "ui_iced/panels/center.rs", &injected);
    let found = match after.findings.as_slice() {
        [one] => one,
        other => panic!("expected exactly one finding, got {other:?}"),
    };
    assert_eq!(found.rule, Rule::ColourLiteral);
    assert_eq!(found.detail, "Color::from_rgb8");
}

/// A colour written into the theme layer is not a violation, and is
/// counted instead — the same asymmetry iced-pro relies on.
#[test]
fn the_gate_does_not_fire_inside_the_theme_layer() {
    let fixture = "fn f() { let c = iced::Color::from_rgb8(1, 2, 3, 1.0); }\n";
    let scan = token_lint::scan_source_in(&app_scope(), APP_COLOUR_DEFINITION_FILE, fixture);
    assert!(
        scan.findings.is_empty(),
        "the theme layer is the colour owner: {:?}",
        scan.findings
    );
    assert_eq!(scan.token_layer_colour_definitions, 1);
}

/// A file outside `ui_iced/` is still Rule-A scanned but never Rule-B
/// scoped, so a stray visual literal there is not this gate's finding.
#[test]
fn the_gate_does_not_reach_outside_its_visual_scope() {
    let fixture = "pub const ROW_HEIGHT: f32 = 21.0;\n";
    let scan = token_lint::scan_source_in(&app_scope(), "workspace/watcher.rs", fixture);
    assert!(
        scan.findings.is_empty(),
        "Rule B must not reach outside `ui_iced/`: {:?}",
        scan.findings
    );
    assert_eq!(scan.token_bound, 0);
}

/// Test fixtures are out of scope for both rules, exactly as they are on
/// the framework side — a contrast test cannot be written without raw
/// reference colours.
#[test]
fn app_test_fixtures_are_out_of_scope() {
    assert!(is_test_file("ui_iced/panels/center_tests.rs"));
    assert!(is_test_file("ui_iced/test_helpers.rs"));
    assert!(!is_test_file("ui_iced/panels/center.rs"));
    assert!(!is_test_file("ui/theme.rs"));

    let fixture = "fn f() { let c = iced::Color::from_rgb8(255, 0, 0, 1.0); }\n";
    let scan = token_lint::scan_source_in(&app_scope(), "ui_iced/panels/center_tests.rs", fixture);
    assert!(
        scan.findings.is_empty(),
        "fixtures must be exempt: {:?}",
        scan.findings
    );
}

// ---------------------------------------------------------------------------
// Registry integrity
// ---------------------------------------------------------------------------

#[test]
fn every_geometry_entry_is_registered_against_a_scoped_file_and_a_visual_name() {
    assert_eq!(
        APP_GEOMETRY_CONSTANTS.len(),
        PIN_APP_SURFACE_BOUND,
        "the app registry and the scan's surface-bound count must agree",
    );
    let scope = app_scope();
    let mut keys: Vec<String> = Vec::new();
    for entry in APP_GEOMETRY_CONSTANTS {
        assert!(
            token_lint::in_scope(entry.file, scope.visual_scope_prefixes),
            "{} is not inside the app's visual scope",
            entry.file
        );
        assert!(
            token_lint::is_visual_name(entry.name),
            "{} names no visual marker, so Rule B would never inspect it and the \
             registry entry is dead weight",
            entry.name
        );
        assert!(
            entry.spec.contains('§'),
            "registry entry {}.{} must cite the spec section that fixes the value",
            entry.file,
            entry.name
        );
        let key = format!("{}:{}", entry.file, entry.name);
        assert!(!keys.contains(&key), "duplicate registry entry for {key}");
        keys.push(key);
    }
}

#[test]
fn geometry_lookup_is_exact_on_both_file_and_name() {
    let scope = app_scope();
    assert!(
        token_lint::lookup_geometry(
            scope.surface_geometry,
            "ui_iced/app.rs",
            "WINDOW_WIDTH_POINTS"
        )
        .is_some(),
        "the registered window width must resolve"
    );
    assert!(
        token_lint::lookup_geometry(
            scope.surface_geometry,
            "ui_iced/app.rs",
            "WINDOW_HEIGHT_POINTS"
        )
        .is_some(),
        "the registered window height must resolve"
    );
    // Right name, wrong file — must not resolve.
    assert!(
        token_lint::lookup_geometry(
            scope.surface_geometry,
            "ui_iced/menu_surface.rs",
            "WINDOW_WIDTH_POINTS"
        )
        .is_none(),
        "a registry entry must not transfer between files"
    );
    // Right file, wrong name.
    assert!(
        token_lint::lookup_geometry(
            scope.surface_geometry,
            "ui_iced/app.rs",
            "WINDOW_DEPTH_POINTS"
        )
        .is_none(),
        "an unregistered name must not resolve"
    );
}

// ---------------------------------------------------------------------------
// What this gate cannot see — and why the other guard still matters
// ---------------------------------------------------------------------------

/// A token-bound constant is clean here **whatever its value is**, because
/// Rule B checks provenance, not legality.
///
/// This is the boundary between the two gates, pinned so nobody reads a
/// green gate as "the sizes are all legal". `TITLE_MD` is a real rung, so
/// this scan passes — and whether 20pt is the *right* size for that
/// constant is a question this scan cannot answer.
/// [`token_guard_tests`](crate::ui_iced::token_guard_tests) answers it, by
/// evaluating the render inputs at runtime against the floor and the rung
/// set.
#[test]
fn this_gate_checks_provenance_not_legality() {
    let fixture = "pub const SOMETHING_SIZE: f32 = typography::TITLE_MD.size;\n";
    let scan = token_lint::scan_source_in(&app_scope(), "ui_iced/provenance.rs", fixture);
    assert!(
        scan.findings.is_empty(),
        "a token-bound scalar must pass Rule B regardless of value: {:?}",
        scan.findings
    );
    assert_eq!(
        scan.token_bound, 1,
        "and it must be counted as token-bound, i.e. invisible to any value check here"
    );
}

/// The report names the counts and the tree it scanned, so a human reading
/// CI output can tell which gate spoke.
#[test]
fn the_report_names_the_gate_and_its_counts() {
    let audit = scan_app();
    let report = render_app(&audit);
    assert!(report.contains("fastmd/ui_iced"), "{report}");
    assert!(report.contains(APP_COLOUR_DEFINITION_FILE), "{report}");
    assert!(report.contains("violations: 0"), "{report}");
    assert!(report.contains("suppressed: 13"), "{report}");
    // The unreadable count is in the report so a clean-looking tree can be
    // told apart from an unread one. Zero here means the parser actually
    // read every declaration it counted.
    assert!(report.contains("0 unreadable"), "{report}");
}

/// **The INT-13 regression.** `PDF_BADGE_SLOT` is the constant that
/// exposed the hole: while its RHS wrapped onto a second line the parser
/// read an empty expression, `numeric_literals` found nothing, and the
/// gate counted it `token-bound` without ever looking at the value. It is
/// single-line today, so the shape that hid it is gone — but the point of
/// INT-14 is that re-wrapping it must no longer be a way to smuggle a
/// literal past Rule B.
#[test]
fn a_rewrapped_pdf_badge_slot_cannot_smuggle_a_literal_past_rule_b() {
    let path = app_source_root()
        .join("ui_iced")
        .join("panels")
        .join("left.rs");
    let text = std::fs::read_to_string(&path).expect("left.rs must be readable");

    let baseline = token_lint::scan_source_in(&app_scope(), "ui_iced/panels/left.rs", &text);
    assert!(
        baseline.findings.is_empty(),
        "the real file must be clean before injection: {:?}",
        baseline.findings
    );
    assert_eq!(
        baseline.unreadable, 0,
        "and nothing in it may be unreadable — every counted constant was read"
    );

    // Re-wrap the declaration exactly the way INT-13 had it, and put a
    // bare literal on the continuation line.
    let rewrapped = text.replace(
        "pub const PDF_BADGE_SLOT: f32 = PDF_BADGE_TEXT_ALLOWANCE + PDF_BADGE_CHIP_PADDING_TOTAL;",
        "pub const PDF_BADGE_SLOT: f32 =\n    PDF_BADGE_TEXT_ALLOWANCE + PDF_BADGE_CHIP_PADDING_TOTAL + 13.0;",
    );
    assert_ne!(
        rewrapped, text,
        "the substitution must have applied — if the real declaration changed \
         shape, update this fixture so the regression keeps testing something"
    );

    let after = token_lint::scan_source_in(&app_scope(), "ui_iced/panels/left.rs", &rewrapped);
    assert_eq!(
        after.findings.len(),
        1,
        "a literal smuggled into a re-wrapped slot must be caught, not waved \
         through as token-bound: {:?}",
        after.findings
    );
    assert_eq!(
        after.findings[0].constant_name(),
        Some("PDF_BADGE_SLOT"),
        "and it must name the constant that smuggled it"
    );
    assert!(
        after.findings[0].detail.contains("13.0"),
        "the finding must name the literal it read from the continuation line: {}",
        after.findings[0].detail
    );
    assert_eq!(
        after.unreadable, 0,
        "the expression was read, so this is a real verdict rather than an \
         admission of blindness"
    );
}
