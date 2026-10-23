//! Tests for the `UIF-PRIN-001` source scan.
//!
//! The scan is a gate that can fail in two directions: it can let a magic
//! number through, and — more dangerously — it can *report* that nothing
//! got through because its patterns quietly stopped matching. Every test
//! here is aimed at one of those two failure modes. The pinned counts are
//! the anti-vacuity half: a scanner whose regex had broken would report
//! zero violations *and* zero findings, and the pinned counts are what
//! makes the first impossible without the second also going red.
//!
//! The injected-failure tests follow the house pattern from
//! [`spec_coverage`](crate::spec_coverage): the real code path, one
//! doctored input, the guard observed firing on every run rather than
//! trusted.

use std::path::{Path, PathBuf};

use crate::token_lint::{
    ALLOW_LIST, Audit, COLOUR_CONSTRUCTORS, COLOUR_DEFINITION_FILE, Rule,
    SURFACE_GEOMETRY_CONSTANTS, ScalarClass, SourceScan, VISUAL_NAME_MARKERS, VISUAL_SCOPE_DIRS,
    Violation, audit_crate, crate_root, in_visual_scope, is_test_file, is_visual_name,
    redact_comments_and_strings, render, scan_source, surface_geometry,
};

// ---------------------------------------------------------------------------
// Pinned counts — the anti-vacuity half
//
// These are not aspirations. They are what the scan reports against the
// tree as it stands, and they move only when the tree moves deliberately.
// If a pattern stops matching, the scan goes quiet and these go red.
// ---------------------------------------------------------------------------

/// Production `.rs` files the scan reads (test fixtures excluded).
///
/// 96 as of the three `INC-44` modules joining the tree for
/// `UIF-ATOM-002d` — `widget/auto_suggest_model.rs`,
/// `widget/auto_suggest_host.rs` and `gallery_suggest.rs` — on top of the
/// 93 `INC-43` left behind.
///
/// 102 as of `INC-45`: the five additions of the `composite/` subsystem
/// — `mod.rs` and the four §3B composite modules — plus
/// `gallery_composite.rs`, the composite block of the verification
/// matrix. The gallery cell file is scanned but not visual-scoped, so it
/// joins the count without joining Rule B.
///
/// 104 as of `INC-46`, which added the disclosure and the scroll
/// contract: `composite/expander.rs` and `composite/scroll_view.rs`.
const PIN_FILES_SCANNED: usize = 104;
/// Files Rule A treats as the colour-definition layer.
const PIN_RULE_A_FILES: usize = 1;
/// Files Rule B treats as widget/surface code.
///
/// 60 as of the two `INC-44` autosuggest modules joining `widget/`. Rule
/// B counts scope membership, so a file with no colour literals still
/// counts; the gallery cells sit outside `widget/`, `surface/` and
/// `composite/` and so are not among them.
///
/// 65 as of `INC-45`, which brought `composite/` into Rule B's scope and
/// added its four composite modules plus the subsystem `mod.rs`.
///
/// 67 as of `INC-46`, whose two additions are `composite/expander.rs`
/// and `composite/scroll_view.rs`.
const PIN_RULE_B_FILES: usize = 67;
/// Colour constructors found inside `token/color.rs`.
const PIN_COLOUR_DEFINITIONS: usize = 26;
/// Visual scalars reading a token path.
///
/// 124 as of `INC-42`, whose fifteen additions are the nine `slider::`
/// geometry consts (`TRACK_HEIGHT`, `TRACK_RADIUS`, `THUMB_SIZE`,
/// `THUMB_HIT_SIZE`, `THUMB_RING_WIDTH`, `RAIL_BORDER_WIDTH`,
/// `TICK_WIDTH`, `TICK_HEIGHT`, `FOCUS_RING_WIDTH`) and the six
/// `number_box::` ones (`ENTRY_HEIGHT`, `STEPPER_WIDTH`,
/// `STEP_BUTTON_SIZE`, `GLYPH_SIZE`, `RADIUS`, `BORDER_WIDTH`). Every one
/// is a `pub const` naming a `token::*` path. The one literal this
/// increment introduces — [`slider::FILLED_ON_UNFILLED_RATIO`] — is a
/// recorded measurement rather than a visual primitive, carries none of
/// the name markers Rule B scans for, and so is not classified at all.
///
/// [`VISUAL_NAME_MARKERS`]: crate::token_lint::VISUAL_NAME_MARKERS
///
/// 127 as of `INC-44`, whose three additions are the autosuggest
/// flyout's token-bound figures — `FLYOUT_ROW_HEIGHT` (`size.control.md`),
/// `FLYOUT_GAP` (`space.xs`) and `FLYOUT_RADIUS` (`shape.radius.widget`).
/// The fourth figure that increment introduces, `FLYOUT_MAX_HEIGHT`, is a
/// bare `240.0` and is registered as a Surface Geometry Constant rather
/// than token-bound, because §3.2 fixes the number outright.
///
/// 146 as of `INC-45`, whose nineteen additions are the token-bound
/// figures the four §3B composites read: the field's `BAND_GAP`,
/// `CONTROL_SLOT_HEIGHT` and `CAPTION_SLOT_HEIGHT`; the settings row's
/// paddings, column gap, border, radius and two rungs; the card's radius,
/// padding and border; and the tile's paddings, slot gap, leading and
/// trailing extents, border and rungs.
///
/// 157 as of `INC-46`, whose eleven additions are the figures the
/// disclosure and the scroll contract read: the expander's two header
/// rungs, its horizontal padding, its chevron-to-title gap, its body
/// padding and its body border; and the scroll view's resting, active and
/// reserved track widths, its tick width and its thumb radius.
///
/// The rest of `INC-46`'s numbers deliberately do *not* join this count,
/// because none of them carries a name marker Rule B scans for — and that
/// is the honest classification rather than an oversight. The chevron's
/// two settled angles and its sweep are rotations, not lengths. The
/// thumb's minimum length, the arrow step, the page-keep, the at-bottom
/// tolerance and the reach-end distance are behavioural measures: they
/// decide *when* something happens, not how large a painted box is, and
/// `REACH_END_DISTANCE` in particular paints nothing at all.
const PIN_TOKEN_BOUND: usize = 157;
/// Visual scalars bound to `0.0`.
const PIN_ZERO: usize = 3;
/// Visual scalars bound to a sub-unit proportion.
const PIN_RATIO: usize = 1;
/// Visual scalars covered by the §1.1 registry.
///
/// 21 as of the three figures §3.2 and §3.4 fix outright for a
/// completion flyout and a Select trigger:
/// `auto_suggest_model::FLYOUT_MAX_HEIGHT` (240px),
/// `combo_model::FLYOUT_MAX_HEIGHT` (280px) and `combo_box::MIN_WIDTH`
/// (120px), which therefore need a registered authority rather than a
/// token.
///
/// 24 as of `INC-45`, which registered the three §3B figures no rung
/// carries: `form_field::HEADER_HEIGHT` (20px), `settings_row::MIN_HEIGHT`
/// (48px) and `list_tile::TWO_LINE_HEIGHT` (44px).
const PIN_SURFACE_BOUND: usize = 24;
/// Exceptions the crate currently takes.
const PIN_ALLOW_ENTRIES: usize = 1;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// An independent recursive `.rs` walk, so the walker itself is checked
/// against something that is not the walker.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn violations_of(scan: &SourceScan) -> Vec<Violation> {
    scan.findings.clone()
}

fn only(scan: &SourceScan) -> Violation {
    assert_eq!(
        scan.findings.len(),
        1,
        "expected exactly one finding, got:\n  {:?}",
        scan.findings
    );
    scan.findings[0].clone()
}

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

#[test]
fn the_crate_passes_the_token_scan() {
    let audit = audit_crate();
    assert!(
        audit.is_clean(),
        "UIF-PRIN-001 source scan failed:\n{}",
        audit.describe()
    );
}

#[test]
fn the_allow_list_is_small_and_every_entry_actually_fires() {
    let audit = audit_crate();

    // Widening the allow-list is the easy way to make this file go green,
    // so the size is pinned: adding an entry is a deliberate act that
    // shows up here, not a quiet fix.
    assert_eq!(
        ALLOW_LIST.len(),
        PIN_ALLOW_ENTRIES,
        "the allow-list has {} entries; the pinned count is {PIN_ALLOW_ENTRIES}. \
         Fix the finding rather than excuse it.",
        ALLOW_LIST.len(),
    );

    // Every entry must suppress something real. A stale entry is a lie
    // about the state of the tree and fails the build on its own.
    assert_eq!(
        audit.suppressed.len(),
        ALLOW_LIST.len(),
        "every allow entry must fire; stale: {:?}",
        audit.stale_allow_entries,
    );
    assert!(
        audit.stale_allow_entries.is_empty(),
        "stale allow entries suppress nothing: {:?}",
        audit.stale_allow_entries,
    );

    // Each entry carries a reason, and the reasons are distinct — a
    // blanket "it's fine" repeated down the list is not a justification.
    let mut reasons: Vec<&str> = ALLOW_LIST.iter().map(|e| e.reason).collect();
    for entry in ALLOW_LIST {
        assert!(
            entry.reason.trim().len() > 40,
            "allow entry `{}` needs a real reason, got {:?}",
            entry.key,
            entry.reason,
        );
        assert!(
            entry.key.contains('/') && entry.key.contains(':'),
            "allow key must be `file:identity`, got {}",
            entry.key,
        );
    }
    reasons.sort_unstable();
    reasons.dedup();
    assert_eq!(reasons.len(), ALLOW_LIST.len(), "reasons must be distinct");
}

// ---------------------------------------------------------------------------
// Anti-vacuity: the scan is reading what it claims to read
// ---------------------------------------------------------------------------

#[test]
fn the_scan_reads_every_production_file_and_agrees_with_an_independent_walk() {
    let audit = audit_crate();
    assert_eq!(audit.stats.files_scanned, PIN_FILES_SCANNED);

    let root = crate_root();
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
        "the scan read {} files but an independent walk finds {} non-test .rs files",
        audit.stats.files_scanned,
        expected.len(),
    );
    assert!(
        !expected.is_empty(),
        "the independent walk found nothing — the fixture is broken, not clean"
    );
}

#[test]
fn the_rule_scopes_are_what_they_claim_to_be() {
    let audit = audit_crate();
    assert_eq!(audit.stats.rule_a_files, PIN_RULE_A_FILES);
    assert_eq!(audit.stats.rule_b_files, PIN_RULE_B_FILES);
    assert_eq!(VISUAL_SCOPE_DIRS, ["widget/", "surface/", "composite/"]);
    assert_eq!(COLOUR_DEFINITION_FILE, "token/color.rs");
    assert!(in_visual_scope("widget/button.rs"));
    assert!(in_visual_scope("surface/movable.rs"));
    assert!(!in_visual_scope("token/color.rs"));
    assert!(!in_visual_scope("geometry/clamp.rs"));
    assert!(!in_visual_scope("gallery.rs"));
}

#[test]
fn the_scanner_actually_finds_colour_definitions_in_the_token_layer() {
    // This is the single most important assertion in the file. Rule A
    // passes by finding nothing; this is the only thing that distinguishes
    // "no colours outside the token layer" from "the pattern is broken".
    let audit = audit_crate();
    assert_eq!(
        audit.stats.token_layer_colour_definitions, PIN_COLOUR_DEFINITIONS,
        "the scan must still find the colour definitions it is calibrated against",
    );
    assert!(audit.stats.token_layer_colour_definitions > 0);
}

#[test]
fn the_scanner_actually_finds_visual_scalars_and_classifies_them() {
    let audit = audit_crate();
    assert_eq!(audit.stats.token_bound, PIN_TOKEN_BOUND);
    assert_eq!(audit.stats.zero, PIN_ZERO);
    assert_eq!(audit.stats.ratio, PIN_RATIO);
    assert_eq!(audit.stats.surface_bound, PIN_SURFACE_BOUND);

    let total =
        audit.stats.token_bound + audit.stats.zero + audit.stats.ratio + audit.stats.surface_bound;
    assert!(
        total >= 60,
        "Rule B classified only {total} visual scalars; the parser has likely stopped matching"
    );
    // Token-bound must dominate: if it collapsed to zero while the others
    // held, the token-path pattern is what broke.
    assert!(audit.stats.token_bound > audit.stats.surface_bound);
}

// ---------------------------------------------------------------------------
// Rule A — injection tests
// ---------------------------------------------------------------------------

#[test]
fn rule_a_fires_on_a_colour_injected_into_a_widget() {
    let src = "pub fn paint() {\n    let c = Color::from_rgb(255, 128, 0);\n}\n";
    let scan = scan_source("widget/injected.rs", src);
    let v = only(&scan);
    assert_eq!(v.rule, Rule::ColourLiteral);
    assert_eq!(v.file, "widget/injected.rs");
    assert_eq!(v.line, 2);
    assert_eq!(v.detail, "Color::from_rgb");
    assert_eq!(v.key(), "widget/injected.rs:Color::from_rgb");
}

#[test]
fn rule_a_fires_on_every_banned_constructor() {
    for ctor in COLOUR_CONSTRUCTORS {
        let src = format!("fn f() {{ let c = {ctor}(1, 2, 3, 1.0); }}\n");
        let scan = scan_source("widget/injected.rs", &src);
        let v = only(&scan);
        assert_eq!(
            v.detail, ctor,
            "`{ctor}` must be reported as itself, not mis-attributed"
        );
        assert_eq!(v.rule, Rule::ColourLiteral);
    }
    assert_eq!(
        COLOUR_CONSTRUCTORS,
        [
            "Color::from_rgb",
            "Color::from_rgba",
            "Color::from_rgb8",
            "Color::from_rgba8"
        ],
        "the banned set is closed; adding to it is a deliberate act"
    );
}

#[test]
fn rule_a_fires_on_hex_colour_literals_outside_a_string() {
    // The scanner is text-based, so the `0x` form is the live one in Rust
    // source. The `#` form is asserted at the text level because the
    // module doc claims it: a bare `#rrggbb` is not valid Rust, but the
    // rule is stated over text and this proves the branch is wired.
    for src in [
        "let c = 0x1A2B3C;\n",
        "let c = 0x1A2B3C4D;\n",
        "let c = #ff8800;\n",
    ] {
        let scan = scan_source("widget/injected.rs", src);
        let v = only(&scan);
        assert_eq!(v.rule, Rule::ColourLiteral, "`{src}` must be caught");
        assert_eq!(v.detail, "hex colour literal");
    }
}

#[test]
fn rule_a_does_not_fire_inside_the_token_layer_but_still_counts_it() {
    let src = "pub const RED: Color = Color::from_rgb(255, 0, 0);\n";
    let scan = scan_source(COLOUR_DEFINITION_FILE, src);
    assert!(
        scan.findings.is_empty(),
        "the token layer is the one place a colour may be defined: {:?}",
        scan.findings
    );
    assert_eq!(scan.token_layer_colour_definitions, 1);

    // Same text, different file: the definition count must NOT accrue
    // outside the token layer, or the count proves nothing.
    let elsewhere = scan_source("widget/red.rs", src);
    assert_eq!(elsewhere.token_layer_colour_definitions, 0);
    assert_eq!(elsewhere.findings.len(), 1);
}

#[test]
fn rule_a_ignores_a_colour_written_in_a_comment_or_a_string() {
    let src = concat!(
        "// Color::from_rgb(1, 2, 3) is only prose\n",
        "/* block: Color::from_rgba8 */\n",
        "let s = \"Color::from_rgb(0, 0, 0)\";\n",
        "let r = r#\"#ff0000 and Color::from_rgb\"#;\n",
    );
    let scan = scan_source("widget/prose.rs", src);
    assert!(
        scan.findings.is_empty(),
        "documentation is not code: {:?}",
        scan.findings
    );
}

// ---------------------------------------------------------------------------
// Rule B — injection tests
// ---------------------------------------------------------------------------

#[test]
fn rule_b_fires_on_an_injected_bare_measurement() {
    let src = "pub const INJECTED_GAP: f32 = 13.0;\n";
    let scan = scan_source("widget/injected.rs", src);
    let v = only(&scan);
    assert_eq!(v.rule, Rule::UnboundVisualScalar);
    assert_eq!(v.constant_name(), Some("INJECTED_GAP"));
    assert_eq!(v.line, 1);
    assert!(
        v.detail.contains("13.0"),
        "the report must name the literal: {}",
        v.detail
    );
    assert_eq!(v.key(), "widget/injected.rs:INJECTED_GAP");
}

#[test]
fn rule_b_accepts_the_classes_the_spec_itself_draws() {
    let src = concat!(
        "pub const TOKEN_GAP: f32 = space::MD;\n",
        "pub const COMPOSITE_PADDING: f32 = space::SM + space::XXS;\n",
        "pub const ZERO_PADDING: f32 = 0.0;\n",
        "pub const SLOT_RATIO: f32 = 0.25;\n",
    );
    let scan = scan_source("widget/ok.rs", src);
    assert!(scan.findings.is_empty(), "{:?}", scan.findings);
    assert_eq!(scan.token_bound, 2, "a sum of tokens is still token-bound");
    assert_eq!(scan.zero, 1);
    assert_eq!(scan.ratio, 1);
    assert_eq!(scan.surface_bound, 0);
}

#[test]
fn rule_b_grants_the_carve_out_only_to_a_registered_constant() {
    // Registered: §4.7.1 fixes the movable card at 680x440.
    let registered = "pub const CARD_DEFAULT_WIDTH: f32 = 680.0;\n";
    let scan = scan_source("surface/movable.rs", registered);
    assert!(scan.findings.is_empty(), "{:?}", scan.findings);
    assert_eq!(scan.surface_bound, 1);

    // Same name, same literal, a file that never registered it. The
    // exemption is the registry, not the name — so this is a violation.
    let unregistered = scan_source("surface/input_dialog.rs", registered);
    let v = only(&unregistered);
    assert_eq!(v.rule, Rule::UnboundVisualScalar);
    assert_eq!(v.constant_name(), Some("CARD_DEFAULT_WIDTH"));
    assert_eq!(unregistered.surface_bound, 0);
}

#[test]
fn rule_b_ignores_constants_whose_name_is_not_a_visual_measurement() {
    let src = "pub const TIMEOUT_SECONDS: f32 = 13.0;\npub const MAX_RETRIES: f32 = 4.0;\n";
    let scan = scan_source("widget/notvisual.rs", src);
    assert!(scan.findings.is_empty(), "{:?}", scan.findings);
    assert_eq!(
        scan.token_bound + scan.zero + scan.ratio + scan.surface_bound,
        0
    );
}

#[test]
fn rule_b_ignores_non_pub_const_fn_and_non_f32() {
    let src = concat!(
        "const PRIVATE_GAP: f32 = 13.0;\n",
        "pub const fn gap_fn() -> f32 { 13.0 }\n",
        "pub const WIDTH_U32: u32 = 13;\n",
    );
    let scan = scan_source("widget/other.rs", src);
    assert!(
        scan.findings.is_empty(),
        "only `pub const … : f32` is in scope: {:?}",
        scan.findings
    );
}

#[test]
fn rule_b_does_not_reach_outside_the_visual_scope() {
    let src = "pub const GAP: f32 = 13.0;\n";
    for file in [
        "geometry/clamp.rs",
        "token/space.rs",
        "gallery.rs",
        "state.rs",
    ] {
        let scan = scan_source(file, src);
        assert!(
            scan.findings.is_empty(),
            "{file} is outside widget/ and surface/ and must not be scanned by Rule B"
        );
    }
}

// ---------------------------------------------------------------------------
// Real-tree injection — the house pattern
// ---------------------------------------------------------------------------

#[test]
fn the_scan_fires_on_a_scalar_injected_into_a_real_production_file() {
    let root = crate_root();
    let path = root.join("surface").join("movable.rs");
    let text = std::fs::read_to_string(&path).expect("movable.rs must be readable");

    let baseline = scan_source("surface/movable.rs", &text);
    assert!(
        baseline.findings.is_empty(),
        "the real file must be clean before injection: {:?}",
        baseline.findings
    );

    let injected = format!("{text}\npub const INJECTED_STROKE: f32 = 7.0;\n");
    let after = scan_source("surface/movable.rs", &injected);
    assert_eq!(
        after.findings.len(),
        1,
        "exactly the injected constant must be new: {:?}",
        after.findings
    );
    assert_eq!(after.findings[0].constant_name(), Some("INJECTED_STROKE"));
    assert_eq!(after.token_bound, baseline.token_bound);
    assert_eq!(after.zero, baseline.zero);
    assert_eq!(after.ratio, baseline.ratio);
    assert_eq!(after.surface_bound, baseline.surface_bound);
}

#[test]
fn the_scan_fires_on_a_colour_injected_into_a_real_production_file() {
    let root = crate_root();
    let path = root.join("widget").join("button.rs");
    let text = std::fs::read_to_string(&path).expect("button.rs must be readable");

    let baseline = scan_source("widget/button.rs", &text);
    assert!(
        baseline.findings.is_empty(),
        "the real file must be clean before injection: {:?}",
        baseline.findings
    );

    let injected = format!("{text}\nfn injected() {{ let c = Color::from_rgb8(9, 9, 9, 1.0); }}\n");
    let after = scan_source("widget/button.rs", &injected);
    let v = only(&after);
    assert_eq!(v.rule, Rule::ColourLiteral);
    assert_eq!(v.detail, "Color::from_rgb8");
}

// ---------------------------------------------------------------------------
// Registry integrity
// ---------------------------------------------------------------------------

#[test]
fn every_surface_geometry_entry_is_registered_against_a_file_in_scope_and_a_visual_name() {
    assert_eq!(
        SURFACE_GEOMETRY_CONSTANTS.len(),
        PIN_SURFACE_BOUND,
        "the registry and the scan's surface-bound count must agree",
    );
    let mut keys: Vec<String> = Vec::new();
    for entry in SURFACE_GEOMETRY_CONSTANTS {
        assert!(
            in_visual_scope(entry.file),
            "{} is not a widget/surface file",
            entry.file
        );
        assert!(
            is_visual_name(entry.name),
            "{} does not read as a visual measurement, so it needed no carve-out",
            entry.name
        );
        assert!(
            entry.spec.contains('§'),
            "{} must cite the § that authorises it",
            entry.name
        );
        let key = format!("{}::{}", entry.file, entry.name);
        assert!(!keys.contains(&key), "duplicate registry entry {key}");
        keys.push(key);
    }
}

#[test]
fn surface_geometry_lookup_is_exact_on_both_file_and_name() {
    assert!(surface_geometry("surface/movable.rs", "CARD_DEFAULT_WIDTH").is_some());
    assert!(surface_geometry("surface/movable.rs", "CARD_DEFAULT_HEIGHT").is_some());
    assert!(surface_geometry("surface/input_dialog.rs", "CARD_WIDTH").is_some());
    assert!(surface_geometry("surface/movable.rs", "CARD_WIDTH").is_none());
    assert!(surface_geometry("surface/input_dialog.rs", "CARD_DEFAULT_WIDTH").is_none());
    assert!(surface_geometry("surface/movable.rs", "NOT_A_BOUND").is_none());
}

// ---------------------------------------------------------------------------
// Helpers and reporting
// ---------------------------------------------------------------------------

#[test]
fn test_files_are_out_of_scope_for_both_rules() {
    assert!(is_test_file("widget/button_tests.rs"));
    assert!(is_test_file("test_helpers.rs"));
    assert!(is_test_file("ui/test_helpers.rs"));
    assert!(is_test_file("tests/integration.rs"));
    assert!(!is_test_file("widget/button.rs"));
    assert!(!is_test_file("token/color.rs"));

    // A raw colour in a test fixture must not fail the build — a contrast
    // solver cannot be tested without one — and must not be counted.
    let scan = scan_source(
        "token/contrast_tests.rs",
        "let c = Color::from_rgb(255, 0, 0);\n",
    );
    assert!(scan.findings.is_empty());
    assert_eq!(scan.token_layer_colour_definitions, 0);
}

#[test]
fn visual_name_matching_is_case_insensitive_substring() {
    for marker in VISUAL_NAME_MARKERS {
        assert!(
            is_visual_name(&format!("SOMETHING_{marker}_ELSE")),
            "{marker} must mark a visual name"
        );
        assert!(
            is_visual_name(&marker.to_lowercase()),
            "matching must be case-insensitive"
        );
    }
    assert!(!is_visual_name("LABEL"));
    assert!(!is_visual_name("MAX_ITEMS"));
}

#[test]
fn scalar_classes_report_their_own_compliance() {
    assert!(ScalarClass::TokenBound.is_compliant());
    assert!(ScalarClass::Zero.is_compliant());
    assert!(ScalarClass::Ratio.is_compliant());
    assert!(ScalarClass::SurfaceBound.is_compliant());
    assert!(!ScalarClass::Violation.is_compliant());

    assert_eq!(Rule::ColourLiteral.label(), "colour-literal");
    assert_eq!(Rule::UnboundVisualScalar.label(), "unbound-scalar");
    assert_eq!(ScalarClass::SurfaceBound.label(), "surface-bound");
}

#[test]
fn the_rendered_report_names_the_counts_and_the_findings() {
    let audit = audit_crate();
    let text = render(&audit);
    assert!(text.contains("UIF-PRIN-001"));
    assert!(text.contains(&format!("files scanned: {}", PIN_FILES_SCANNED)));
    assert!(text.contains(&format!(
        "colour definitions in token/color.rs: {PIN_COLOUR_DEFINITIONS}"
    )));
    assert!(text.contains("violations: 0"));
    assert!(text.contains(&format!("suppressed: {}", PIN_ALLOW_ENTRIES)));
    assert!(text.contains("BUTTON_MIN_WIDTH"));

    // And when it is not clean, the report says so and names the file.
    let dirty = Audit {
        violations: vec![Violation {
            rule: Rule::ColourLiteral,
            file: "widget/x.rs".to_string(),
            line: 7,
            code: "let c = Color::from_rgb(1, 2, 3);".to_string(),
            detail: "Color::from_rgb".to_string(),
        }],
        ..Audit::default()
    };
    let dirty_text = render(&dirty);
    assert!(dirty_text.contains("violations: 1"));
    assert!(dirty_text.contains("widget/x.rs:7"));
    assert!(!dirty.is_clean());
    assert!(dirty.describe().contains("colour-literal"));
}

#[test]
fn redaction_blanks_noise_without_moving_the_lines() {
    let src = concat!(
        "let a = 1; // Color::from_rgb\n",
        "let b = 2; /* outer /* nested */ still comment */ Color::from_rgb(1,2,3);\n",
        "let c = \"a \\\" escaped string\"; let d = 4;\n",
        "let e = r#\"raw \"# and let f = 6;\n",
        "let g = 'x'; let h = '&'; let life = &1;\n",
    );
    let red = redact_comments_and_strings(src);
    let lines: Vec<&str> = red.lines().collect();

    // Line-for-line alignment is what makes reported line numbers true.
    assert_eq!(lines.len(), src.lines().count());
    for (a, b) in src.lines().zip(lines.iter()) {
        assert_eq!(
            a.chars().count(),
            b.chars().count(),
            "columns must not shift"
        );
    }

    // Line 1 is nothing but a comment: the constructor in it is gone.
    assert!(!lines[0].contains("Color::from_rgb"), "{}", lines[0]);
    // Line 2's comment is gone, but the *live* call after it is not — the
    // nested block closed exactly where Rust would have closed it.
    assert!(!lines[1].contains("nested"), "{}", lines[1]);
    assert!(!lines[1].contains("still comment"), "{}", lines[1]);
    assert!(
        lines[1].contains("Color::from_rgb(1,2,3);"),
        "live code after a nested comment must survive: {}",
        lines[1]
    );
    // Strings, raw strings and char literals are blanked; the code around
    // them is not, and a lifetime quote never eats the line.
    assert!(!lines[2].contains("escaped string"), "{}", lines[2]);
    assert!(lines[2].contains("let d = 4;"), "{}", lines[2]);
    assert!(!lines[3].contains("raw "), "{}", lines[3]);
    assert!(lines[3].contains("let f = 6;"), "{}", lines[3]);
    assert!(!lines[4].contains("'x'"), "{}", lines[4]);
    assert!(lines[4].contains("let life = &1;"), "{}", lines[4]);

    // Rule A sees exactly the one live constructor, on the right line.
    let scan = scan_source("widget/redcheck.rs", src);
    assert_eq!(scan.findings.len(), 1, "{:?}", scan.findings);
    assert_eq!(scan.findings[0].line, 2);
}

#[test]
fn an_empty_file_is_clean_and_an_empty_audit_is_clean() {
    let scan = scan_source("widget/empty.rs", "");
    assert!(scan.findings.is_empty());
    assert_eq!(violations_of(&scan).len(), 0);
    assert!(Audit::default().is_clean());
}
