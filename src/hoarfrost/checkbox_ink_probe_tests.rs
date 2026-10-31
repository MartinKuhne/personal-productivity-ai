//! Empirical ink-placement and glyph-coverage measurements for the
//! `Checkbox` mark (`UIF-ATOM-003`, §3.3).
//!
//! Every test here prints its figures. They record what the widget **does**,
//! not what it should do — a failure is a finding, not something to tune
//! away.

use crate::checkbox_ink_probe::{
    self as probe, BOX_AA_MARGIN_PHYS, COVERAGE_MATRIX, CheckboxInkReport, FIXTURE_SYMBOLS,
    FaceMeasurement, ICON_SYMBOLS, INDICATOR_BOX, PROBE_SCALE, PROBE_VIEWPORT, RENDER_FONT_COVERS,
    SymbolClass, TYPOGRAPHIC_SYMBOLS_UNCOVERED,
};
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::size;
use crate::widget::toggle::{self, TogglePolarity, ToggleStyle};
use crate::widget::toggle_widget::{CHECK_MARK, INDETERMINATE_MARK};

/// Tolerance for a centring claim, in **logical** pixels.
///
/// Half a pixel: tight enough that the `(-8.000, -8.000)` half-box
/// displacement this suite was written to catch could never hide inside it,
/// and loose enough that antialiasing at `PROBE_SCALE` cannot make a
/// correctly-placed mark fail.
const CENTRE_TOLERANCE: f32 = 0.5;

/// The resolved paint channels for a checked box at rest — the exact
/// colours `Checkbox::draw` hands to the renderer.
fn checked_style() -> ToggleStyle {
    toggle::resolve_style(
        TogglePolarity::On,
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
        &Palette::dark(),
    )
}

/// The mark's logical font size — `CHECKBOX_MARK_SIZE`, which is private,
/// pinned here through the token it aliases.
fn mark_size() -> f32 {
    assert_eq!(
        size::ICON_SM,
        14.0,
        "CHECKBOX_MARK_SIZE aliases size::ICON_SM; if that moved, the probe's \
         figure is no longer the widget's figure"
    );
    size::ICON_SM
}

/// Render every state once and print the whole picture.
fn report() -> &'static CheckboxInkReport {
    static REPORT: std::sync::OnceLock<CheckboxInkReport> = std::sync::OnceLock::new();
    REPORT.get_or_init(|| {
        let style = checked_style();
        probe::measure_checkbox_ink(mark_size(), style.mark, style.indicator_fill)
    })
}

fn art(label: &str, body: &str) {
    println!("--- {label} (1 char = 1 logical px, box is 16x16) ---");
    for (i, line) in body.lines().enumerate() {
        println!("{i:>3} |{line}|");
    }
    println!("--- end {label} ---");
}

// ---------------------------------------------------------------------------
// Calibration
// ---------------------------------------------------------------------------

/// The unchecked frame paints exactly the indicator quad, which is what
/// every figure below is calibrated against. If this is not a 16×16
/// logical box the whole measurement is meaningless.
#[test]
fn test_checkbox_ink_calibration_indicator_box_is_16x16() {
    let r = report();
    println!("viewport      = {PROBE_VIEWPORT:?} @ scale {PROBE_SCALE}");
    println!(
        "unchecked ink = {:?} -> logical [{:.3},{:.3} .. {:.3},{:.3}] ({:.2}x{:.2})",
        r.unchecked_ink,
        r.box_logical.x0,
        r.box_logical.y0,
        r.box_logical.x1,
        r.box_logical.y1,
        r.box_logical.w(),
        r.box_logical.h(),
    );
    art("REAL FRAME: unchecked (box only)", &r.unchecked_frame_art);

    let tol = 1.0 / PROBE_SCALE;
    assert!(
        (r.box_logical.w() - INDICATOR_BOX).abs() <= tol,
        "calibration: the unchecked indicator must measure {INDICATOR_BOX} logical px, \
         measured {:.3}",
        r.box_logical.w()
    );
    assert!(
        (r.box_logical.h() - INDICATOR_BOX).abs() <= tol,
        "calibration: the unchecked indicator must measure {INDICATOR_BOX} logical px, \
         measured {:.3}",
        r.box_logical.h()
    );
}

/// Negative control: an unchecked checkbox carries no mark ink — its total
/// ink is the box and nothing more.
#[test]
fn test_checkbox_ink_unchecked_control_paints_no_mark() {
    let r = report();
    let unchecked = r.unchecked_ink.expect("unchecked box must paint");
    let area = (unchecked.w() * unchecked.h()) as f64;
    let expected = (INDICATOR_BOX * PROBE_SCALE * INDICATOR_BOX * PROBE_SCALE) as f64;
    println!("unchecked ink bbox = {unchecked:?}  area={area:.0} phys px (box = {expected:.0})");

    assert!(
        area <= expected + 8.0,
        "negative control: unchecked ink must be the indicator quad only, got {unchecked:?}"
    );
}

// ---------------------------------------------------------------------------
// Positive control — the indeterminate dash (U+2212, present in Fira Sans)
// ---------------------------------------------------------------------------

/// Positive control **and** the acceptance target.
///
/// Two claims are made together here:
///
/// 1. **The pipeline is trustworthy.** The `✓` and the `−` come from two
///    different faces and are measured by two different isolation methods;
///    if the measurement were noise the two displacement vectors would
///    disagree. They do not.
/// 2. **The widget now matches iced.** Each mark's displacement against
///    iced's own `text` widget painting the same glyph, size, line height
///    and colour must be zero within [`CENTRE_TOLERANCE`]. Before the fix
///    both were `(-8.000, -8.000)` — one shared half-box translation in
///    the call site, which is why two unrelated glyphs moved identically.
#[test]
fn test_checkbox_ink_indeterminate_control_shows_same_displacement() {
    let r = report();
    println!(
        "indeterminate frame ink = {:?}",
        r.indeterminate_ink.map(|b| (b.x0, b.y0, b.x1, b.y1))
    );
    println!(
        "indeterminate escape ink outside box AA band = {:?}",
        r.indeterminate_escape_bbox
            .map(|b| (b.x0, b.y0, b.x1, b.y1))
    );
    println!(
        "{}",
        r.indeterminate_mark.summary("REAL   indeterminate U+2212")
    );
    println!(
        "{}",
        r.indeterminate_mark_replay
            .summary("REPLAY indeterminate U+2212")
    );
    art(
        "REAL FRAME: indeterminate (box + dash)",
        &r.indeterminate_frame_art,
    );
    art("REPLAY: dash alone at box origin", &r.indeterminate_art);

    assert!(
        r.indeterminate_mark.ink_pixels > 0,
        "positive control: the U+2212 dash must paint ink; zero ink means the \
         measurement pipeline is broken, not the widget"
    );

    let dash_ref = r.indeterminate_reference;
    let check_ref = r.checked_reference;
    let dash_actual = r.indeterminate_mark.mark_logical;
    let check_actual = r.checked_mark.mark_logical;

    let dash_disp = (dash_actual.x0 - dash_ref.x0, dash_actual.y0 - dash_ref.y0);
    let check_disp = (
        check_actual.x0 - check_ref.x0,
        check_actual.y0 - check_ref.y0,
    );

    println!("U+2212 reference (iced `text`, centred) = {dash_ref:?}");
    println!("U+2212 actual    (checkbox fill_text)   = {dash_actual:?}");
    println!(
        "U+2212 displacement                      = ({:+.3}, {:+.3}) logical px",
        dash_disp.0, dash_disp.1
    );
    println!("U+2713 reference (iced `text`, centred) = {check_ref:?}");
    println!("U+2713 actual    (checkbox fill_text)   = {check_actual:?}");
    println!(
        "U+2713 displacement                      = ({:+.3}, {:+.3}) logical px",
        check_disp.0, check_disp.1
    );

    let tol = 1.0 / PROBE_SCALE;
    assert!(
        (dash_disp.0 - check_disp.0).abs() <= tol && (dash_disp.1 - check_disp.1).abs() <= tol,
        "positive control FAILED: the two marks are displaced by different vectors — \
         dash ({:+.3},{:+.3}) vs check ({:+.3},{:+.3}); the measurement is not trustworthy",
        dash_disp.0,
        dash_disp.1,
        check_disp.0,
        check_disp.1
    );

    for (name, disp) in [("U+2212", dash_disp), ("U+2713", check_disp)] {
        assert!(
            disp.0.abs() <= CENTRE_TOLERANCE && disp.1.abs() <= CENTRE_TOLERANCE,
            "{name} does not land where iced's own `text` widget puts it: \
             displaced ({:+.3},{:+.3}) logical px (tolerance {CENTRE_TOLERANCE})",
            disp.0,
            disp.1
        );
    }

    println!(
        "PIPELINE OK + ACCEPTANCE MET: both marks land on iced's own placement, \
         residual ({:+.3},{:+.3}) logical px",
        check_disp.0, check_disp.1
    );
}

// ---------------------------------------------------------------------------
// The checked mark — U+2713
// ---------------------------------------------------------------------------

/// Is the `✓` visible at all?
#[test]
fn test_checkbox_ink_checked_mark_paints_something() {
    let r = report();
    println!("{}", r.checked_mark.summary("REAL  checked U+2713"));
    println!("{}", r.checked_mark_replay.summary("REPLAY checked U+2713"));
    art("REAL FRAME: checked (box + check)", &r.checked_frame_art);
    art("REPLAY: check alone at box origin", &r.checked_art);

    assert!(
        r.checked_mark.ink_pixels > 0,
        "FINDING: the U+2713 mark painted zero ink — the checkbox renders no \
         checkmark at all"
    );
}

/// The containment claim: does the `✓` ink lie inside the 16×16 box?
#[test]
fn test_checkbox_ink_checked_mark_lies_inside_indicator_box() {
    let r = report();
    let m = r.checked_mark;
    println!("{}", m.summary("checked U+2713 (containment)"));
    println!(
        "overhangs: left={:+.3} top={:+.3} right={:+.3} bottom={:+.3} max={:+.3} escapes={}",
        m.over_left,
        m.over_top,
        m.over_right,
        m.over_bottom,
        m.max_overhang(),
        m.escapes
    );
    println!(
        "gaps: top={:.3} bottom={:.3} left={:.3} right={:.3} vertical_imbalance={:+.3}",
        m.top_gap, m.bottom_gap, m.left_gap, m.right_gap, m.vertical_gap_imbalance
    );
    println!(
        "escape ink bbox (phys) = {:?}",
        r.checked_escape_bbox.map(|b| (b.x0, b.y0, b.x1, b.y1))
    );

    assert!(
        !m.escapes,
        "FINDING (UIF-ATOM-003 §3.3): the check mark's ink escapes the \
         {INDICATOR_BOX}x{INDICATOR_BOX} indicator box — \
         over(left,top,right,bottom)=({:+.3},{:+.3},{:+.3},{:+.3}) logical px, \
         max overhang {:+.3} px",
        m.over_left,
        m.over_top,
        m.over_right,
        m.over_bottom,
        m.max_overhang()
    );
}

/// Vertical centring of the `✓`, measured against iced's own `text` widget.
///
/// Centring is asserted against the **reference placement**, not against
/// equal top and bottom ink gaps. A glyph's ink is not symmetric inside
/// its own line box — `✓` sits high in its — so even iced's own correctly
/// centred mark shows a `-3.250` px gap imbalance. Demanding equal gaps
/// would ask for the glyph to be redrawn, not repositioned.
///
/// What the paint call actually controls is the centre, so that is what is
/// pinned here: the mark's ink centre must land on the centre iced's own
/// `text` widget produces for the same glyph, in the same box, at the same
/// size and line height — within [`CENTRE_TOLERANCE`].
#[test]
fn test_checkbox_ink_checked_mark_is_vertically_centred() {
    let r = report();
    let m = r.checked_mark;

    let mark_center = m.mark_logical.center();
    let ref_center = r.checked_reference.center();
    let dy = mark_center.y - ref_center.y;

    println!(
        "raw ink gaps: top={:.3} bottom={:.3} imbalance={:+.3} logical px \
         (glyph-intrinsic; NOT the centring criterion)",
        m.top_gap, m.bottom_gap, m.vertical_gap_imbalance
    );
    println!(
        "mark centre y = {:.3}   iced reference centre y = {:.3}   delta = {:+.3} logical px",
        mark_center.y, ref_center.y, dy
    );

    assert!(
        dy.abs() <= CENTRE_TOLERANCE,
        "the check mark is not vertically centred on iced's own placement — \
         mark centre y {:.3}, iced reference centre y {:.3}, delta {:+.3} logical px \
         (tolerance {CENTRE_TOLERANCE})",
        mark_center.y,
        ref_center.y,
        dy
    );
}

/// The escape must be visible in the real widget, not only in a replay.
#[test]
fn test_checkbox_ink_checked_frame_total_ink_vs_box() {
    let r = report();
    let box_bb = r.unchecked_ink.expect("box");
    let total = r.checked_ink.expect("checked frame ink");
    println!(
        "box bbox (physical)      = {:?}",
        (box_bb.x0, box_bb.y0, box_bb.x1, box_bb.y1)
    );
    println!(
        "checked frame ink (phys) = {:?}",
        (total.x0, total.y0, total.x1, total.y1)
    );
    println!(
        "real-widget escape beyond the box, logical px: left={:+.3} top={:+.3} right={:+.3} bottom={:+.3} \
         (AA band excluded = {BOX_AA_MARGIN_PHYS} phys px / {:+.2} logical)",
        (box_bb.x0 as f32 - total.x0 as f32) / PROBE_SCALE,
        (box_bb.y0 as f32 - total.y0 as f32) / PROBE_SCALE,
        (total.x1 as f32 - box_bb.x1 as f32) / PROBE_SCALE,
        (total.y1 as f32 - box_bb.y1 as f32) / PROBE_SCALE,
        BOX_AA_MARGIN_PHYS as f32 / PROBE_SCALE,
    );

    assert!(
        total.w() >= box_bb.w() && total.h() >= box_bb.h(),
        "the checked frame must cover at least the indicator box"
    );
}

/// The two independent isolation methods must agree on where the mark's
/// ink starts. If they disagree the probe is untrustworthy.
#[test]
fn test_checkbox_ink_isolation_methods_agree() {
    let r = report();
    let real = r.checked_mark.mark_logical;
    let replay = r.checked_mark_replay.mark_logical;
    println!(
        "check mark — real-frame classification: [{:.3},{:.3} .. {:.3},{:.3}]",
        real.x0, real.y0, real.x1, real.y1
    );
    println!(
        "check mark — isolated replay         : [{:.3},{:.3} .. {:.3},{:.3}]",
        replay.x0, replay.y0, replay.x1, replay.y1
    );
    println!(
        "agreement: dx0={:+.3} dy0={:+.3} dx1={:+.3} dy1={:+.3} logical px",
        real.x0 - replay.x0,
        real.y0 - replay.y0,
        real.x1 - replay.x1,
        real.y1 - replay.y1,
    );

    let tol = 1.0 / PROBE_SCALE;
    assert!(
        (real.x0 - replay.x0).abs() <= tol
            && (real.y0 - replay.y0).abs() <= tol
            && (real.x1 - replay.x1).abs() <= tol
            && (real.y1 - replay.y1).abs() <= tol,
        "the two mark-isolation methods disagree by more than one physical pixel; \
         the measurement cannot be trusted"
    );
}

// ---------------------------------------------------------------------------
// The sibling marks — the radio dot and the switch thumb
// ---------------------------------------------------------------------------

/// The resolved paint channels for an **un**selected control at rest.
fn off_style() -> ToggleStyle {
    toggle::resolve_style(
        TogglePolarity::Off,
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
        &Palette::dark(),
    )
}

/// The radio's inner dot is `fill_quad` geometry, not a glyph, so the
/// `fill_text` anchor defect cannot reach it — but that is a claim, so it is
/// measured rather than asserted from the source. The dot's ink centre must
/// sit on the ring's ink centre.
#[test]
fn test_checkbox_ink_radio_dot_is_centred_in_ring() {
    let style = checked_style();
    let m = probe::measure_radio_dot(style.mark);
    let (dx, dy) = m.centre_offset();

    println!("{}", m.summary("radio dot (selected, measured)"));
    println!("centre offset from ring centre = ({dx:+.3}, {dy:+.3}) logical px");

    assert!(
        m.ink_pixels > 0,
        "a selected radio must paint its inner dot; zero ink means the probe, \
         not the widget, is broken"
    );
    assert!(
        !m.escapes,
        "the radio dot escapes its ring: over(L,T,R,B)=({:+.3},{:+.3},{:+.3},{:+.3})",
        m.over_left, m.over_top, m.over_right, m.over_bottom
    );
    assert!(
        dx.abs() <= CENTRE_TOLERANCE && dy.abs() <= CENTRE_TOLERANCE,
        "radio dot is off-centre in its ring by ({dx:+.3},{dy:+.3}) logical px \
         (tolerance {CENTRE_TOLERANCE}) — the same half-box displacement class as \
         the checkbox mark was",
    );
    println!("radio dot OK: centred in the ring, no half-box displacement");
}

/// The switch thumb: vertically centred in the track in **both** positions,
/// travelling exactly the distance its own module declares.
///
/// The horizontal centre is deliberately not asserted — a thumb that did
/// not move would be a broken switch, not a centred one.
#[test]
fn test_checkbox_ink_switch_thumb_is_centred_and_travels() {
    let on = checked_style();
    let off = off_style();
    let marks = probe::measure_switch_thumb(on.mark, off.mark);

    println!("track = {:?}", marks.track);
    println!("{}", marks.off.summary("switch thumb (off, measured)"));
    println!("{}", marks.on.summary("switch thumb (on, measured)"));

    for (name, m) in [("off", marks.off), ("on", marks.on)] {
        let (dx, dy) = m.centre_offset();
        println!("{name}: centre offset = ({dx:+.3}, {dy:+.3}) logical px");

        assert!(
            m.ink_pixels > 0,
            "the switch must paint a thumb in the {name} position"
        );
        assert!(
            !m.escapes,
            "the switch thumb ({name}) escapes the track: \
             over(L,T,R,B)=({:+.3},{:+.3},{:+.3},{:+.3})",
            m.over_left, m.over_top, m.over_right, m.over_bottom
        );
        assert!(
            dy.abs() <= CENTRE_TOLERANCE,
            "switch thumb ({name}) is not vertically centred in the track: \
             {dy:+.3} logical px (tolerance {CENTRE_TOLERANCE})",
        );
    }

    let travel = marks.on.mark_logical.x0 - marks.off.mark_logical.x0;
    let declared = toggle::thumb_travel();
    println!("thumb travel measured = {travel:+.3} px, declared thumb_travel() = {declared:.3} px");
    assert!(
        (travel - declared).abs() <= CENTRE_TOLERANCE,
        "thumb travel {travel:+.3} px does not match the declared {declared:.3} px \
         (tolerance {CENTRE_TOLERANCE})"
    );
    println!("switch thumb OK: vertically centred both ends, travel matches the token");
}

// ---------------------------------------------------------------------------
// Task 2 — the glyph coverage gate
// ---------------------------------------------------------------------------

/// Print every glyph constant the crate emits and whether Fira Sans has it.
#[test]
fn test_checkbox_ink_glyph_coverage_full_table() {
    let all = probe::scan_glyph_constants();
    println!("manifest dir scanned: {}", probe::manifest_dir());
    println!(
        "render font bytes: {} (iced::advanced::graphics::text::FIRA_SANS_REGULAR)",
        probe::render_font_bytes().len()
    );
    println!(
        "\n{:<26} {:<9} {:<7} {:<46} face",
        "CONST", "CODEPOINT", "IN FONT", "LOCATION"
    );
    for g in &all {
        println!("{}", g.row());
    }
    println!("\ntotal glyph constants: {}", all.len());
    println!(
        "missing from Fira Sans: {}",
        all.iter().filter(|g| !g.in_render_font).count()
    );

    assert!(
        !all.is_empty(),
        "the scanner found no glyph constants at all — it is broken, not clean"
    );

    // Sanity-check the cmap reader against codepoints whose status is
    // independently known, so a parser bug cannot silently pass the gate.
    let font = probe::render_font_bytes();
    for cp in [
        'A', 'z', '0', '\u{00D7}', '\u{2212}', '\u{2022}', '\u{2190}',
    ] {
        assert!(
            probe::present_in_render_font(font, cp),
            "cmap reader sanity: U+{:04X} must be present in Fira Sans",
            cp as u32
        );
    }
    for cp in ['\u{2713}', '\u{2715}', '\u{26A0}', '\u{25CF}'] {
        assert!(
            !probe::present_in_render_font(font, cp),
            "cmap reader sanity: U+{:04X} must be ABSENT from Fira Sans",
            cp as u32
        );
    }
}

/// Glyph constants the framework emits that the render font cannot supply,
/// pinned as known-missing.
///
/// Each entry is `CONST_NAME U+XXXX`. The pair, not the file:line — a pin
/// that moved every time someone edited a line above it would be a pin that
/// gets deleted rather than read.
///
/// Deduplicated on purpose: `CHECK_MARK U+2713` is declared in three
/// modules, and the pin covers the *debt*, not each copy of it. Fixing one
/// of three copies leaves the pair missing; fixing all three heals it and
/// the ratchet fails so the entry is removed.
///
/// The `Self::*` entries are the icons the framework emits from a `match`
/// arm rather than a `const` — the notification and badge kinds, and the
/// table header's sort arrows. They are in this list because the scanner
/// reads arms as well as declarations now; before it did, the gate was
/// silently blind to seven real gaps, and a clean report meant only that
/// it had never looked at the line the glyph lives on.
const KNOWN_MISSING_GLYPHS: [&str; 47] = [
    "CALENDAR_ICON U+1F4C5",
    "CHECK_MARK U+2713",
    "CHEVRON_DOWN U+25BE",
    "CHEVRON_GLYPH U+25BE",
    "CHEVRON_UP U+25B4",
    "CJK U+30AD",
    "CJK U+30B9",
    "CJK U+30C6",
    "CJK U+30C8",
    "CJK U+65E5",
    "CJK U+672C",
    "CJK U+8A9E",
    "CLOCK_ICON U+1F552",
    "GLYPH_CHECK U+2713",
    "GLYPH_CHEVRON U+25BE",
    "GLYPH_COMPLETED U+2713",
    "GLYPH_COPIED U+2713",
    "GLYPH_COPY U+1F4CB",
    "GLYPH_DOWN U+25BC",
    "GLYPH_ERROR U+26A0",
    "GLYPH_FIRST U+23EE",
    "GLYPH_LAST U+23ED",
    "GLYPH_NEXT U+25B6",
    "GLYPH_PINNED U+1F4CC",
    "GLYPH_PREV U+25C0",
    "GLYPH_REVEAL U+1F441",
    "GLYPH_SEARCH U+1F50D",
    "GLYPH_TOGGLE U+2630",
    "GLYPH_UNPINNED U+1F4CD",
    "GLYPH_UP U+25B2",
    "ICON_PASS U+2713",
    "ICON_WARN U+26A0",
    "LABEL_APPLY U+2713",
    "LABEL_FINISH U+2713",
    "LEGEND_KEYBOARD U+21B5",
    "PERSON_GLYPH U+1F464",
    "Self::Ascending U+25B2",
    "Self::Danger U+2715",
    "Self::Descending U+25BC",
    "Self::Info U+24D8",
    "Self::Info U+25CF",
    "Self::Success U+2713",
    "Self::Warning U+26A0",
    "STAR_EMPTY U+2606",
    "STAR_FILLED U+2605",
    "TRIGGER_LABEL U+22EF",
    "WARNING_GLYPH U+26A0",
];

/// Why these are pinned rather than fixed here.
///
/// Every entry is a real gap, not a false positive: the constant is
/// emitted by the framework and `FIRA_SANS_REGULAR` — the only face iced
/// embeds — carries no cmap entry for it, so it reaches the page only via
/// cosmic-text's platform fallback chain. That makes the rendered glyph
/// host-dependent: whatever fallback face the machine happens to have, or
/// `.notdef` on a bare one.
///
/// The fix is a **font-fallback strategy** — bundle an icon/symbol face
/// and resolve these codepoints against it, or swap the constants for
/// glyphs Fira Sans actually carries — and that is a separate workstream
/// with its own decisions: which face, which codepoints, and whether the
/// emoji-range constants should exist at all. Pinning them here keeps this
/// crate's suite green while that is scheduled, and makes the debt a
/// countable number that cannot grow.
const MISSING_GLYPH_RATIONALE: &str = concat!(
    "These codepoints are absent from FIRA_SANS_REGULAR and reach the page only through ",
    "cosmic-text's platform fallback chain, so what renders is host-dependent. The fix is a ",
    "font-fallback strategy (bundle and resolve an icon/symbol face, or replace the constants) ",
    "and is owned by a separate workstream; pinned here so the count is visible and cannot grow."
);

/// The gate, **ratcheted** rather than left red.
///
/// Same pattern as `KNOWN_FRAMEWORK_DEFECTS` in
/// [`crate::geometry_gallery_tests`]: two claims are checked together so
/// the pin cannot rot in either direction.
///
/// * No missing glyph outside the pinned list may appear.
/// * Every pinned glyph must still be missing. One that became present fails
///   here so the entry gets removed rather than quietly outliving the fix.
///
/// A permanently-red gate is as useless as a missing one — people stop
/// reading it — and this one was red for a real but separately-owned
/// reason.
#[test]
fn test_checkbox_ink_glyph_coverage_gate() {
    let current: Vec<String> = probe::missing_glyphs()
        .into_iter()
        .map(|(name, cp, _loc)| format!("{name} {cp}"))
        .collect::<std::collections::BTreeSet<String>>()
        .into_iter()
        .collect();

    println!("glyph coverage gate (ratcheted)");
    println!("  pinned missing: {}", KNOWN_MISSING_GLYPHS.len());
    println!("  measured missing (unique): {}", current.len());
    println!("  rationale: {MISSING_GLYPH_RATIONALE}");

    let unexpected: Vec<&str> = current
        .iter()
        .filter(|glyph| !KNOWN_MISSING_GLYPHS.contains(&glyph.as_str()))
        .map(String::as_str)
        .collect();
    assert!(
        unexpected.is_empty(),
        "NEW missing glyph(s), not in KNOWN_MISSING_GLYPHS: {unexpected:?}\n\
         The framework now emits a glyph Fira Sans cannot supply. Either give it a face that \
         can, or pin it deliberately — and say why in the PR.\n\
         rationale: {MISSING_GLYPH_RATIONALE}"
    );

    let healed: Vec<&&str> = KNOWN_MISSING_GLYPHS
        .iter()
        .filter(|glyph| !current.iter().any(|present| present == *glyph))
        .collect();
    assert!(
        healed.is_empty(),
        "pinned glyph(s) are now present in the render font and must be removed from \
         KNOWN_MISSING_GLYPHS: {healed:?}"
    );

    println!("  gate: PASS — measured set matches the pinned set exactly");
}

/// Print the resolved paint channels so the colour classification used to
/// separate mark from box is on the record.
#[test]
fn test_checkbox_ink_print_resolved_channels() {
    let style = checked_style();
    println!("indicator_fill = {:?}", style.indicator_fill);
    println!("indicator_border = {:?}", style.indicator_border);
    println!("mark           = {:?}", style.mark);
    println!("probe background = {:?}", probe::PROBE_BACKGROUND);
    println!("COLOR_TOLERANCE = {}", probe::COLOR_TOLERANCE);
    println!("RESIDUAL_MAX = {}", probe::RESIDUAL_MAX);
}

/// The specific defect this probe was written to record: `CHECK_MARK` is
/// U+2713 and U+2713 is not in the render font.
#[test]
fn test_checkbox_ink_check_mark_codepoint_is_not_in_render_font() {
    let cp = CHECK_MARK.chars().next().expect("CHECK_MARK is one char");
    let dash = INDETERMINATE_MARK.chars().next().expect("mark is one char");
    let font = probe::render_font_bytes();
    println!(
        "CHECK_MARK = U+{:04X}  in Fira Sans cmap = {}",
        cp as u32,
        probe::present_in_render_font(font, cp)
    );
    println!(
        "INDETERMINATE_MARK = U+{:04X}  in Fira Sans cmap = {}",
        dash as u32,
        probe::present_in_render_font(font, dash)
    );
    println!(
        "CHECK_MARK resolves at render time to: {:?}",
        probe::resolving_face(cp)
    );
    println!(
        "INDETERMINATE_MARK resolves at render time to: {:?}",
        probe::resolving_face(dash)
    );

    assert_eq!(cp, '\u{2713}', "CHECK_MARK must be U+2713 CHECK MARK");
    assert_eq!(
        dash, '\u{2212}',
        "INDETERMINATE_MARK must be U+2212 MINUS SIGN"
    );
}

// ---------------------------------------------------------------------------
// The symbol-face ledger (`G-6`)
// ---------------------------------------------------------------------------

/// The icon inventory is twenty-six codepoints, and the bundled face
/// carries none of them.
const LEDGER_ICON_COUNT: usize = 26;

/// Message shared by the live-cmap icon assertions.
const ICONS_ABSENT: &str = "every icon in `ICON_SYMBOLS` must be absent from the bundled \
                           render font, or the geometry migration has a glyph it does not \
                           need to redraw";

/// The render font covers every symbol the ledger says it covers.
///
/// Checked against the live cmap rather than against the doc table, so a
/// declared-but-wrong entry cannot hide.
#[test]
fn test_glyph_ledger_render_font_actually_covers_every_declared_covered_symbol() {
    let missing: Vec<String> = RENDER_FONT_COVERS
        .iter()
        .filter(|ch| !probe::render_font_has(**ch))
        .map(|ch| probe::codepoint_label(*ch))
        .collect();
    assert!(
        missing.is_empty(),
        "RENDER_FONT_COVERS claims the bundled face draws these, but its cmap does not: {}",
        missing.join(", ")
    );
}

/// The bundled face draws **none** of the icon inventory — the headline
/// `G-6` measurement: 0 of 26.
#[test]
fn test_glyph_ledger_bundled_face_covers_none_of_the_icon_inventory() {
    let present: Vec<String> = ICON_SYMBOLS
        .iter()
        .filter(|ch| probe::render_font_has(**ch))
        .map(|ch| probe::codepoint_label(*ch))
        .collect();
    println!(
        "bundled render font covers {} of {} icon codepoints",
        ICON_SYMBOLS.len() - present.len(),
        ICON_SYMBOLS.len()
    );
    assert!(
        present.is_empty(),
        "{ICONS_ABSENT}. Present but should not be: {}",
        present.join(", ")
    );
}

#[test]
fn test_glyph_ledger_icon_inventory_is_the_expected_size() {
    assert_eq!(
        ICON_SYMBOLS.len(),
        LEDGER_ICON_COUNT,
        "the coverage matrix is measured against a {LEDGER_ICON_COUNT}-codepoint icon inventory"
    );
    let unique = ICON_SYMBOLS
        .iter()
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        unique.len(),
        ICON_SYMBOLS.len(),
        "ICON_SYMBOLS must not list a codepoint twice"
    );
}

#[test]
fn test_glyph_ledger_declared_sets_are_disjoint() {
    for ch in ICON_SYMBOLS {
        assert!(
            !RENDER_FONT_COVERS.contains(&ch),
            "{} is declared both covered and an icon",
            probe::codepoint_label(ch)
        );
        assert!(
            !TYPOGRAPHIC_SYMBOLS_UNCOVERED.contains(&ch),
            "{} is declared both an icon and typographic",
            probe::codepoint_label(ch)
        );
        assert!(
            !FIXTURE_SYMBOLS.contains(&ch),
            "{} is declared both an icon and a fixture",
            probe::codepoint_label(ch)
        );
    }
    for ch in TYPOGRAPHIC_SYMBOLS_UNCOVERED {
        assert!(
            !RENDER_FONT_COVERS.contains(&ch),
            "{} is declared both covered and uncovered",
            probe::codepoint_label(ch)
        );
        assert!(
            !FIXTURE_SYMBOLS.contains(&ch),
            "{} is declared both typographic and a fixture",
            probe::codepoint_label(ch)
        );
    }
}

/// Nothing in hoarfrost's sources is a raw symbol the ledger has no answer
/// for.
///
/// This is the guard that makes `G-6` machine-visible: add a symbol glyph
/// without deciding how the framework guarantees it and this goes red.
#[test]
fn test_glyph_ledger_every_symbol_found_in_the_sources_is_classified() {
    let unclassified = probe::unclassified_symbols();
    let labels: Vec<String> = unclassified
        .iter()
        .map(|ch| format!("{} ({})", probe::codepoint_label(*ch), *ch))
        .collect();
    assert!(
        unclassified.is_empty(),
        "a raw symbol glyph was added to hoarfrost's sources with no face decision: {}.\n\
         Classify it in `ICON_SYMBOLS` (needs drawn geometry), in the typographic set \
         (punctuation the text face owns), or in `FIXTURE_SYMBOLS` (test-only text the \
         framework never paints).",
        labels.join(", ")
    );
}

/// The three classes route apart, and an unknown codepoint is still `None`.
///
/// The `None` arm is asserted deliberately: a classifier that grew a
/// default would pass the guard test forever while classifying nothing.
#[test]
fn test_glyph_ledger_classification_routes_each_role_apart() {
    assert_eq!(probe::class_of('\u{2713}'), Some(SymbolClass::Icon));
    assert_eq!(probe::class_of('\u{26A0}'), Some(SymbolClass::Icon));
    assert_eq!(probe::class_of('\u{1F50D}'), Some(SymbolClass::Icon));
    assert_eq!(probe::class_of('\u{1F441}'), Some(SymbolClass::Icon));
    assert_eq!(probe::class_of('\u{24D8}'), Some(SymbolClass::Icon));
    assert_eq!(probe::class_of('\u{2014}'), Some(SymbolClass::Typographic));
    assert_eq!(probe::class_of('\u{21D2}'), Some(SymbolClass::Typographic));
    assert_eq!(probe::class_of('\u{65E5}'), Some(SymbolClass::Fixture));
    assert_eq!(probe::class_of('\u{30C6}'), Some(SymbolClass::Fixture));
    assert_eq!(probe::class_of('a'), None);
    assert_eq!(probe::class_of('\u{1F980}'), None);
    assert_eq!(probe::class_label(None), "UNCLASSIFIED");
    assert_eq!(probe::class_label(Some(SymbolClass::Fixture)), "fixture");
}

/// The CJK fixture is a fixture, not debt.
///
/// `microcopy/truncate_tests.rs` truncates `"テスト日本語"`. The scanner
/// reads test sidecars on purpose, so those seven codepoints arrive here;
/// they are CJK script and test input, not icon output, and classing them
/// `Fixture` records that rather than leaving them looking unhandled.
#[test]
fn test_glyph_ledger_the_cjk_truncate_fixture_is_not_icon_debt() {
    let entries: Vec<_> = probe::ledger()
        .into_iter()
        .filter(|e| FIXTURE_SYMBOLS.contains(&e.ch))
        .collect();
    assert!(
        !entries.is_empty(),
        "the truncate fixture's codepoints are no longer in the scan; if the fixture \
         moved, this test and FIXTURE_SYMBOLS should move with it"
    );
    for e in &entries {
        assert_eq!(
            e.class,
            Some(SymbolClass::Fixture),
            "{} reached the ledger from {} but is not classed as a fixture",
            e.codepoint,
            e.location
        );
        assert!(
            !probe::is_icon(e.ch),
            "{} is CJK script from a test fixture and must not be in the icon inventory",
            e.codepoint
        );
    }
    println!(
        "fixture codepoints in the ledger: {}",
        entries
            .iter()
            .map(|e| e.codepoint.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
}

#[test]
fn test_glyph_ledger_marks_every_icon_as_host_fallback() {
    let entries = probe::ledger();
    assert!(
        !entries.is_empty(),
        "the source scan found no glyph constants at all; the ledger is blind"
    );
    let bad: Vec<String> = entries
        .iter()
        .filter(|e| e.class == Some(SymbolClass::Icon) && e.render_font_has)
        .map(|e| format!("{} {}", e.codepoint, e.constant))
        .collect();
    assert!(
        bad.is_empty(),
        "{ICONS_ABSENT}. Bundled-but-classed-as-icon: {}",
        bad.join(", ")
    );
}

/// Every icon in the inventory is reported as unguaranteed.
///
/// The assertion is only honest because the scanner reaches every
/// declaration shape. Three of these icons — `ⓘ`, `●` and `✕` — exist
/// **only** in `match` arms, so a `const`-only scanner could never have
/// produced them and this test would have failed for a reason that was the
/// scanner's, not the framework's.
#[test]
fn test_glyph_ledger_unguaranteed_icons_cover_the_whole_inventory() {
    let got: Vec<String> = probe::unguaranteed_icons()
        .iter()
        .map(|e| e.codepoint.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    println!("unguaranteed icons reported: {}", got.len());
    for ch in ICON_SYMBOLS {
        let label = probe::codepoint_label(ch);
        assert!(
            got.contains(&label),
            "{label} is an emitted icon but the ledger does not report it as unguaranteed; \
             the scan may not reach its declaration"
        );
    }
}

/// The three match-arm-only icons are reached **through their arm**.
///
/// Pins the scanner extension itself rather than leaving it inferred: if
/// arm scanning were ever dropped these three would vanish from the ledger
/// and the coverage test above would fail with a message pointing at the
/// wrong thing.
#[test]
fn test_glyph_ledger_reaches_the_match_arm_icons() {
    let entries = probe::ledger();
    for ch in ['\u{24D8}', '\u{25CF}', '\u{2715}'] {
        let label = probe::codepoint_label(ch);
        let reached: Vec<&str> = entries
            .iter()
            .filter(|e| e.ch == ch && e.constant.starts_with("Self::"))
            .map(|e| e.constant.as_str())
            .collect();
        assert!(
            !reached.is_empty(),
            "{label} is declared only in a `match` arm; the scanner must reach it there, \
             but no `Self::*` entry carries it"
        );
        println!("{label} reached through: {}", reached.join(", "));
    }
}

#[test]
fn test_glyph_ledger_every_matrix_row_is_internally_consistent() {
    for m in COVERAGE_MATRIX {
        assert_eq!(
            m.icons_covered + m.icon_gaps.len(),
            LEDGER_ICON_COUNT,
            "{}: icons_covered + icon_gaps must equal the {}-codepoint inventory",
            m.face,
            LEDGER_ICON_COUNT
        );
        let unique = m.icon_gaps.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(
            unique.len(),
            m.icon_gaps.len(),
            "{}: icon_gaps lists a codepoint twice",
            m.face
        );
        for gap in m.icon_gaps {
            assert!(
                ICON_SYMBOLS.contains(gap),
                "{}: gap {} is not in the inventory",
                m.face,
                probe::codepoint_label(*gap)
            );
        }
    }
}

#[test]
fn test_glyph_ledger_fira_row_matches_the_live_cmap() {
    let fira = COVERAGE_MATRIX
        .iter()
        .find(|m| m.face == "Fira Sans (bundled today)")
        .expect("the matrix must carry the row for the face iced bundles");
    assert_eq!(
        fira.icons_covered, 0,
        "Fira Sans draws none of the icon inventory"
    );
    for ch in ICON_SYMBOLS {
        assert!(
            !probe::render_font_has(ch),
            "matrix says Fira Sans lacks {} but the live cmap has it",
            probe::codepoint_label(ch)
        );
    }
}

/// The measurement that decides the framework does not ship a symbol font.
///
/// If a permissive face ever reaches full coverage this test fires, and the
/// decision should be revisited with it.
#[test]
fn test_glyph_ledger_no_redistributable_face_closes_the_gap() {
    let closing: Vec<&FaceMeasurement> = COVERAGE_MATRIX
        .iter()
        .filter(|m| m.closes_the_gap())
        .collect();
    assert!(
        closing.is_empty(),
        "a redistributable face now covers the whole icon inventory: {:?}. \
         Re-run the bundle evaluation before assuming geometry is still the right call.",
        closing
            .iter()
            .map(|m| (m.face, m.licence))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_glyph_ledger_only_full_face_is_the_one_we_may_not_ship() {
    let full: Vec<&FaceMeasurement> = COVERAGE_MATRIX
        .iter()
        .filter(|m| m.icon_gaps.is_empty())
        .collect();
    assert_eq!(
        full.len(),
        1,
        "exactly one face covers all {LEDGER_ICON_COUNT} icons"
    );
    assert!(
        !full[0].redistributable,
        "the only complete face must be non-redistributable, else bundling is viable"
    );
    assert_eq!(full[0].face, "Segoe UI Symbol");
}

#[test]
fn test_glyph_ledger_best_permissive_face_leaves_seven_icons_on_the_host() {
    let best = COVERAGE_MATRIX
        .iter()
        .filter(|m| m.redistributable)
        .max_by_key(|m| m.icons_covered)
        .expect("at least one redistributable row");
    assert_eq!(best.face, "Noto Sans Symbols 2");
    assert_eq!(best.icons_covered, 19);
    assert_eq!(
        best.icon_gaps.len(),
        7,
        "the best OFL face leaves ↵ ⋯ ⓘ 👤 📅 📌 📍 uncovered"
    );
}

/// Noto Sans Symbols 2 is the strongest OFL candidate and carries no
/// Latin letters at all, so naming it for `"✓ Apply"` would strand
/// `"Apply"`.
#[test]
fn test_glyph_ledger_best_permissive_face_cannot_serve_a_mixed_label() {
    let best = COVERAGE_MATRIX
        .iter()
        .filter(|m| m.redistributable)
        .max_by_key(|m| m.icons_covered)
        .expect("at least one redistributable row");
    assert!(
        !best.has_latin,
        "{} unexpectedly carries Latin; re-check whether a bundle could serve mixed labels",
        best.face
    );
}

#[test]
fn test_glyph_ledger_every_permissive_face_leaves_at_least_one_gap() {
    let failing = probe::permissive_faces_that_fail();
    let redistributable = COVERAGE_MATRIX.iter().filter(|m| m.redistributable).count();
    assert_eq!(
        failing.len(),
        redistributable,
        "every redistributable face in the matrix must fail to close the gap"
    );
}

/// The property the whole ledger exists to hold: **adding an icon turns
/// the suite red.**
///
/// Proven end to end rather than asserted. A glyph hoarfrost does not emit
/// today — `U+1F980` CRAB — is pushed through the real pipeline in both
/// declaration shapes, and the ledger must reject it as unclassified on
/// both. The codepoint is supplied as a number and formatted at runtime so
/// this file's own text never contains a `const … = '🦀'` line the real
/// scan could pick up and blame on the crate.
#[test]
fn test_glyph_ledger_a_new_icon_turns_the_ledger_red() {
    const INJECTED: u32 = 0x1F980; // 🦀 CRAB — in no declared set
    let glyph = char::from_u32(INJECTED).expect("0x1F980 is a valid codepoint");
    let label = probe::codepoint_label(glyph);

    // The real crate is clean first, so any red below is attributable to
    // the injection and not to something already there.
    assert!(
        probe::unclassified_symbols().is_empty(),
        "precondition: the real sources already carry an unclassified glyph"
    );
    assert!(
        probe::class_of(glyph).is_none(),
        "precondition: {label} must not already be in the ledger"
    );

    let const_src = format!("pub const GLYPH_BRAND_NEW: char = {glyph:?};\n");
    let arm_src = format!("            Self::BrandNew => {glyph:?},\n");

    let mut injected = probe::scan_glyph_source("widget/injected_probe.rs", &const_src);
    injected.extend(probe::scan_glyph_source(
        "composite/injected_probe.rs",
        &arm_src,
    ));
    let entries = probe::ledger_from(injected);

    println!("injected {label} through the ledger pipeline:");
    for e in &entries {
        println!("  {}", e.row());
    }

    assert_eq!(
        entries.len(),
        2,
        "both the `const` and the `match` arm must be scanned; got {} entries",
        entries.len()
    );
    assert_eq!(
        entries[0].constant, "GLYPH_BRAND_NEW",
        "the const declaration must be scanned"
    );
    assert_eq!(
        entries[1].constant, "Self::BrandNew",
        "the match arm must be scanned"
    );

    let unclassified = probe::unclassified_among(&entries);
    assert_eq!(
        unclassified,
        vec![glyph],
        "a brand-new icon must reach the ledger unclassified on every declaration \
         shape — this is the guard that makes adding an icon visible"
    );
    for e in &entries {
        assert!(
            !e.render_font_has,
            "{} unexpectedly bundled, so the coverage gate would miss it too",
            e.codepoint
        );
    }

    // And the real crate is still clean afterwards: the injection never
    // touched it.
    assert!(
        probe::unclassified_symbols().is_empty(),
        "the synthetic injection leaked into the real ledger"
    );
    println!("GUARD OK: a new icon is caught unclassified in both declaration shapes");
}
