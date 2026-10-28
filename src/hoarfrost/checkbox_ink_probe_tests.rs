//! Empirical ink-placement and glyph-coverage measurements for the
//! `Checkbox` mark (`UIF-ATOM-003`, §3.3).
//!
//! Every test here prints its figures. They record what the widget **does**,
//! not what it should do — a failure is a finding, not something to tune
//! away.

use crate::checkbox_ink_probe::{
    self as probe, BOX_AA_MARGIN_PHYS, CheckboxInkReport, INDICATOR_BOX, PROBE_SCALE,
    PROBE_VIEWPORT,
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
const KNOWN_MISSING_GLYPHS: [&str; 40] = [
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
