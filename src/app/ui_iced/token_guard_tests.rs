//! INT-7c token guards -- the converted center panel, agent panel and
//! Markdown render layer carry no off-scale type size, no sub-floor text, and
//! a tab strip whose geometry is derived rather than emergent.
//!
//! Three kinds of check live here, and each is written so that a **scanner
//! which quietly stopped matching goes red too** -- the found-count is
//! asserted, never assumed. That shape is copied from
//! `hoarfrost/token_lint_tests.rs::no_line_height_call_hands_iced_a_bare_f32`,
//! the only precedent in this repo for auditing source rather than values.
//!
//! * **Source scan** -- reads the converted files with comments and string
//!   literals redacted and rejects a bare numeric literal reaching `.size(..)`
//!   or a `*_SIZE` constant. A value that is legal by accident is still a
//!   value nobody bound to a token, and it drifts.
//! * **Floor and scale** -- every size the converted surfaces can actually
//!   render, evaluated at runtime, is at or above `type.min` (12) *and* sits
//!   on one of the six rungs.
//! * **Derivation** -- the tab-strip band closes on tokens, so retuning a
//!   rung moves the band instead of leaving a height that no longer fits.
//!
//! `assertions_on_constants` is allowed throughout: pinning token-bound
//! constants is exactly what that lint fires on, and pinning them is the
//! point of this file.
#![allow(clippy::assertions_on_constants)]

use hoarfrost::token::typography::{self, MIN_SIZE};
use hoarfrost::token::{shape, size, space};
use hoarfrost::token_lint::{redact_comments_and_strings, source_files};

use std::path::{Path, PathBuf};

use crate::ui_iced::agent::{self, prompt_input, transcript};
use crate::ui_iced::panels::center;
use crate::ui_iced::render::{code, heading, inline, yaml};

// ---------------------------------------------------------------------------
// Scope -- which files are "converted"
// ---------------------------------------------------------------------------

/// Every file INT-7c converted, relative to `src/app/ui_iced/`.
///
/// The list is the guard's contract. A converted file that is not listed is
/// unguarded, so the count is pinned below and each path is asserted to
/// exist -- a renamed or moved file fails loudly instead of being skipped.
const CONVERTED_FILES: [&str; 10] = [
    "panels/center.rs",
    "agent/mod.rs",
    "agent/prompt_input.rs",
    "agent/transcript.rs",
    "render/mod.rs",
    "render/heading.rs",
    "render/inline.rs",
    "render/code.rs",
    "render/table.rs",
    "render/yaml.rs",
];

/// `src/app/ui_iced`, resolved from this crate's manifest directory.
///
/// The `fastmd` package's manifest is the workspace root (`[lib] path =
/// "src/app/lib.rs"`), so `CARGO_MANIFEST_DIR` is the repo root and the
/// source tree hangs off `src/app/`.
fn ui_iced_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("app")
        .join("ui_iced")
}

/// Read a converted file with comments and string literals redacted.
///
/// Panics when the file is missing: the guard must not silently shrink its
/// own scope.
fn redacted(rel: &str) -> String {
    let path = ui_iced_root().join(rel);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("converted file {rel} is unreadable at {path:?}: {e}"));
    redact_comments_and_strings(&raw)
}

// ---------------------------------------------------------------------------
// Scanners
// ---------------------------------------------------------------------------

/// Paren-matched argument text for every `needle` call in `code`.
///
/// The needle is expected to end in `(`, and the leading `.` is part of it so
/// `.size(` cannot be confused with a bare `size(` or with `.with_text_size(`.
/// Bracket depth is tracked so a nested call inside an argument is captured
/// with it.
fn call_args(code: &str, needle: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let needle: Vec<char> = needle.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;

    while i + needle.len() <= chars.len() {
        if chars[i..i + needle.len()] == needle[..] {
            let start = i + needle.len();
            let mut depth = 1usize;
            let mut j = start;
            while j < chars.len() && depth > 0 {
                match chars[j] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    j += 1;
                }
            }
            out.push(chars[start..j.min(chars.len())].iter().collect());
            i = j + 1;
        } else {
            i += 1;
        }
    }

    out
}

/// Whether `arg` carries a numeric literal anywhere in it.
fn has_digit(arg: &str) -> bool {
    arg.chars().any(|c| c.is_ascii_digit())
}

/// Every `pub const NAME: f32 = RHS;` in `code`, as `(NAME, RHS)` pairs.
///
/// Single-line declarations only. A multi-line or non-`f32` declaration is
/// not a scalar size and is out of scope for this scan.
fn f32_consts(code: &str) -> Vec<(String, String)> {
    code.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("pub const ")?;
            let (name, tail) = rest.split_once(": f32 = ")?;
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            let rhs = tail.trim().trim_end_matches(';').trim().to_string();
            Some((name.to_string(), rhs))
        })
        .collect()
}

/// Every `*_SIZE` scalar constant across the converted files, keyed
/// `file:NAME`.
fn converted_size_consts() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for rel in CONVERTED_FILES {
        let code = redacted(rel);
        for (name, rhs) in f32_consts(&code) {
            if name.ends_with("_SIZE") {
                out.push((format!("{rel}:{name}"), rhs));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Guard 1 -- no bare number reaches `.size(..)`
// ---------------------------------------------------------------------------

/// No `.size(..)` call in a converted surface hands iced a bare number.
///
/// Every argument must name a constant or read `.size` off a rung. A literal
/// that happens to equal a rung is not bound to it: nothing moves when the
/// rung moves, and nothing fails when the rung is retuned. That is how
/// `DOCUMENT_TITLE_SIZE = 18` and `EMPTY_STATE_SIZE = 15` got into this
/// codebase in the first place -- numbers with no owner.
#[test]
fn no_size_call_in_a_converted_surface_hands_iced_a_bare_number() {
    let mut offenders: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for rel in CONVERTED_FILES {
        let code = redacted(rel);
        for arg in call_args(&code, ".size(") {
            checked += 1;
            if has_digit(&arg) {
                offenders.push(format!("{rel}: .size({arg})"));
            }
        }
    }

    assert!(
        checked >= 25,
        "the scan found only {checked} `.size(` calls across the {} converted files; \
         they carry thirty, so the scanner has gone quiet and cannot be trusted",
        CONVERTED_FILES.len()
    );
    assert!(
        offenders.is_empty(),
        "these calls render a size that no token owns: {offenders:?}"
    );
}

/// The scanner fires on the exact shape it exists to catch, so a green run
/// above means "none found" rather than "none could be found".
#[test]
fn the_size_scan_fires_on_a_doctored_bare_number() {
    let doctored = "\
fn build() {
    text(\"x\")
        .size(18.0)
        .line_height(typography::TITLE_SM.iced_line_height())
        .into();
}
";
    let code = redact_comments_and_strings(doctored);
    let calls = call_args(&code, ".size(");
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        has_digit(&calls[0]),
        "the doctored bare number must read as an offender: {:?}",
        calls[0]
    );

    // And the same scan must NOT flag the legal shapes.
    let legal = "\
fn build() {
    text(\"x\").size(center::EMPTY_STATE_SIZE).into();
    text(\"y\").size(TAB_RUNG.size).into();
    text(\"z\").size(typography::BODY.size).into();
}
";
    let code = redact_comments_and_strings(legal);
    let calls = call_args(&code, ".size(");
    assert_eq!(calls.len(), 3, "{calls:?}");
    for arg in &calls {
        assert!(
            !has_digit(arg),
            "a token-bound size argument must not read as an offender: {arg}"
        );
    }
}

// ---------------------------------------------------------------------------
// Guard 2 -- every `*_SIZE` constant is bound to a token
// ---------------------------------------------------------------------------

/// Every `pub const *_SIZE: f32` in a converted file is bound to a token or
/// a rung, not written as a number.
///
/// The right-hand side must name a token module (`typography::`, `space::`,
/// `size::`, `shape::`) or another constant. A bare literal is rejected even
/// when its value is legal, because the binding is the thing that keeps it
/// legal.
#[test]
fn every_converted_size_constant_is_bound_to_a_token() {
    const TOKEN_MODULES: [&str; 4] = ["typography::", "space::", "size::", "shape::"];

    let consts = converted_size_consts();
    let offenders: Vec<String> = consts
        .iter()
        .filter(|(_, rhs)| has_digit(rhs))
        .filter(|(_, rhs)| !TOKEN_MODULES.iter().any(|m| rhs.contains(m)))
        .map(|(name, rhs)| format!("{name} = {rhs}"))
        .collect();

    assert!(
        consts.len() >= 20,
        "the scan found {} `*_SIZE` constants across the converted files; they carry \
         twenty-three, so the scanner has gone quiet and cannot be trusted",
        consts.len()
    );
    assert!(
        offenders.is_empty(),
        "these size constants are a bare number with no token behind them: {offenders:?}"
    );
}

/// The retired off-scale and sub-floor numbers stay retired.
///
/// Each was a real defect INT-7c removed. Pinning them with `assert_ne!`
/// means a future edit cannot slide one back in under a new constant name.
#[test]
fn no_converted_size_reassembles_a_retired_number() {
    // `18` -- DOCUMENT_TITLE_SIZE, AGENT_HEADER_SIZE and heading.rs's H3.
    // `15` -- EMPTY_STATE_SIZE. `11` -- CODE_LANGUAGE_SIZE, under the floor.
    // `32` -- H1_SIZE, above the top of the scale. `10.5` -- the transcript's
    // code_size as iced derived it (base * 0.75), under the floor.
    const RETIRED: [f32; 5] = [18.0, 15.0, 11.0, 32.0, 10.5];

    for (name, value) in converted_runtime_sizes() {
        for retired in RETIRED {
            assert_ne!(
                value, retired,
                "{name} has slid back to the retired off-scale value {retired}pt"
            );
        }
    }

    // The retired set really is off-scale, so the guard above is not pinning
    // a legal value by mistake.
    let scale = legal_type_sizes();
    for retired in RETIRED {
        assert!(
            !scale.contains(&retired.to_bits()),
            "{retired} should not be on the type scale"
        );
    }
}

// ---------------------------------------------------------------------------
// Guard 3 -- nothing renders below the type floor, and everything is on a rung
// ---------------------------------------------------------------------------

/// Every size the converted surfaces can actually put on screen, evaluated
/// here rather than parsed, is at or above `type.min` (12).
///
/// This is the guard in its strongest form: it walks the real render inputs --
/// the constants, the heading ladder over every level including out-of-range
/// ones, and the Markdown widget's own `Settings` -- so a sub-floor size
/// cannot hide in a code path the source scan does not read.
#[test]
fn nothing_in_the_converted_surfaces_renders_below_the_type_floor() {
    for (name, value) in converted_runtime_sizes() {
        assert!(
            value >= MIN_SIZE,
            "{name} renders {value}pt, below the {MIN_SIZE}pt floor (UIF-TOK-004)"
        );
    }
}

/// Every converted type size is on the scale, not merely above the floor.
///
/// "at least 12" alone would let `17.0` through. This requires the value to
/// be one of the six rungs.
#[test]
fn every_converted_type_size_sits_on_a_rung() {
    let scale = legal_type_sizes();
    for (name, value) in converted_runtime_sizes() {
        assert!(
            scale.contains(&value.to_bits()),
            "{name} = {value}pt is off the type scale; the rungs are \
             [12, 13, 14, 16, 20, 24]"
        );
    }
}

/// The six rungs' sizes, keyed on `to_bits` because `f32` is not `Ord`.
fn legal_type_sizes() -> Vec<u32> {
    [
        typography::CAPTION.size,
        typography::UI_COMPACT.size,
        typography::BODY.size,
        typography::TITLE_SM.size,
        typography::TITLE_MD.size,
        typography::TITLE_LG.size,
    ]
    .into_iter()
    .map(f32::to_bits)
    .collect()
}

/// Every size value a converted surface can render, by name.
///
/// Includes the Markdown widget's `Settings` ladder and
/// [`heading::heading_size`] over `0..=8` -- the out-of-range levels are
/// deliberate, because that is where a collapse-to-floor bug would show up.
fn converted_runtime_sizes() -> Vec<(String, f32)> {
    const NAMED: &[(&str, f32)] = &[
        // Center panel -- tab strip.
        //
        // The removed unsaved-marker constants are absent from this list and
        // always were: it holds sizes the interface can put on screen *as type*,
        // and the marker was a 6px box, never a point size. §2.4's 12px floor
        // therefore never reached it, and its removal costs this guard nothing --
        // a 6pt *font* still cannot appear here.
        ("center::TAB_TEXT_SIZE", center::TAB_TEXT_SIZE),
        ("center::TAB_CLOSE_GLYPH_SIZE", center::TAB_CLOSE_GLYPH_SIZE),
        // Center panel -- viewers.
        ("center::DOCUMENT_TITLE_SIZE", center::DOCUMENT_TITLE_SIZE),
        ("center::AGENT_HEADER_SIZE", center::AGENT_HEADER_SIZE),
        ("center::AGENT_BODY_SIZE", center::AGENT_BODY_SIZE),
        ("center::EMPTY_STATE_SIZE", center::EMPTY_STATE_SIZE),
        // Agent panel.
        ("agent::HEADER_TEXT_SIZE", agent::HEADER_TEXT_SIZE),
        ("agent::STATUS_TEXT_SIZE", agent::STATUS_TEXT_SIZE),
        (
            "prompt_input::CONTROL_TEXT_SIZE",
            prompt_input::CONTROL_TEXT_SIZE,
        ),
        (
            "transcript::TRANSCRIPT_TEXT_SIZE",
            transcript::TRANSCRIPT_TEXT_SIZE,
        ),
        // Render layer.
        (
            "render::LIST_MARKER_SIZE",
            crate::ui_iced::render::LIST_MARKER_SIZE,
        ),
        ("inline::BODY_SIZE", inline::BODY_SIZE),
        ("inline::LIST_MARKER_SIZE", inline::LIST_MARKER_SIZE),
        ("code::CODE_TEXT_SIZE", code::CODE_TEXT_SIZE),
        ("code::CODE_LANGUAGE_SIZE", code::CODE_LANGUAGE_SIZE),
        ("code::CODE_COPY_GLYPH_SIZE", code::CODE_COPY_GLYPH_SIZE),
        ("yaml::YAML_ROW_SIZE", yaml::YAML_ROW_SIZE),
    ];

    let mut out: Vec<(String, f32)> = NAMED
        .iter()
        .map(|(name, value)| ((*name).to_string(), *value))
        .collect();

    for level in 0..=8u32 {
        out.push((
            format!("heading::heading_size({level})"),
            heading::heading_size(level),
        ));
    }

    let settings = transcript::markdown_settings();
    for (field, value) in [
        ("markdown::text_size", f32::from(settings.text_size)),
        ("markdown::h1_size", f32::from(settings.h1_size)),
        ("markdown::h2_size", f32::from(settings.h2_size)),
        ("markdown::h3_size", f32::from(settings.h3_size)),
        ("markdown::h4_size", f32::from(settings.h4_size)),
        ("markdown::h5_size", f32::from(settings.h5_size)),
        ("markdown::h6_size", f32::from(settings.h6_size)),
        ("markdown::code_size", f32::from(settings.code_size)),
    ] {
        out.push((field.to_string(), value));
    }

    out
}

// ---------------------------------------------------------------------------
// Guard 4 -- spacing is on the base-4 grid
// ---------------------------------------------------------------------------

/// Every converted spacing constant is a named `space` rung.
///
/// `UIF-TOK-001` is explicit that a spacing value which is not one of the
/// rungs "must not appear in layout code". Six of these were off-grid before
/// INT-7c (`10`, `6` four times over, `20`); none may come back.
#[test]
fn every_converted_spacing_constant_is_a_rung() {
    let spacings: &[(&str, f32)] = &[
        ("center::TAB_PADDING_X", center::TAB_PADDING_X),
        ("center::TAB_PADDING_Y", center::TAB_PADDING_Y),
        ("center::TAB_SPACING", center::TAB_SPACING),
        ("center::TAB_CLOSE_SPACING", center::TAB_CLOSE_SPACING),
        ("center::STRIP_PADDING", center::STRIP_PADDING),
        ("center::VIEWER_PADDING", center::VIEWER_PADDING),
        ("center::VIEWER_SPACING", center::VIEWER_SPACING),
        ("agent::PANEL_SPACING", agent::PANEL_SPACING),
        ("agent::PANEL_PADDING", agent::PANEL_PADDING),
        ("prompt_input::ROW_SPACING", prompt_input::ROW_SPACING),
        ("prompt_input::ROW_PADDING", prompt_input::ROW_PADDING),
        (
            "transcript::TRANSCRIPT_SPACING",
            transcript::TRANSCRIPT_SPACING,
        ),
        ("transcript::THINKING_SPACING", transcript::THINKING_SPACING),
        (
            "transcript::TRANSCRIPT_PADDING",
            transcript::TRANSCRIPT_PADDING,
        ),
        ("transcript::TURN_PADDING", transcript::TURN_PADDING),
        (
            "render::BLOCK_SPACING",
            crate::ui_iced::render::BLOCK_SPACING,
        ),
        (
            "render::DOCUMENT_SPACING",
            crate::ui_iced::render::DOCUMENT_SPACING,
        ),
        ("render::INDENT_UNIT", crate::ui_iced::render::INDENT_UNIT),
        (
            "render::TASK_CHECKBOX_SPACING",
            crate::ui_iced::render::TASK_CHECKBOX_SPACING,
        ),
        (
            "heading::HEADING_BOTTOM_SPACING",
            heading::HEADING_BOTTOM_SPACING,
        ),
        ("code::CODE_PADDING", code::CODE_PADDING),
        ("code::CODE_SPACING", code::CODE_SPACING),
        ("yaml::YAML_COLUMN_GAP", yaml::YAML_COLUMN_GAP),
        ("yaml::YAML_ROW_SPACING", yaml::YAML_ROW_SPACING),
        ("yaml::YAML_BOTTOM_SPACING", yaml::YAML_BOTTOM_SPACING),
    ];

    let legal: Vec<u32> = [
        space::XXS,
        space::XS,
        space::SM,
        space::MD,
        space::LG,
        space::XL,
        space::XXL,
    ]
    .into_iter()
    .map(f32::to_bits)
    .collect();

    assert!(
        spacings.len() >= 25,
        "only {} spacing constants are listed; the converted surfaces carry more, so \
         the guard's list has gone stale",
        spacings.len()
    );

    for (name, value) in spacings {
        assert!(
            legal.contains(&value.to_bits()),
            "{name} = {value} is not a `space` rung; the rungs are \
             [2, 4, 8, 12, 16, 24, 32]"
        );
    }
}

/// The retired off-grid spacing values stay retired.
#[test]
fn no_converted_spacing_reuses_a_retired_off_grid_value() {
    // `6` was the single most common off-grid number in these files -- it
    // appeared as VIEWER_SPACING, PANEL_SPACING, ROW_SPACING and
    // BLOCK_SPACING. `10` was TAB_PADDING_X, `20` was INDENT_UNIT.
    const RETIRED: [f32; 3] = [6.0, 10.0, 20.0];

    let values = [
        center::VIEWER_SPACING,
        center::TAB_PADDING_X,
        crate::ui_iced::render::INDENT_UNIT,
        crate::ui_iced::render::BLOCK_SPACING,
        agent::PANEL_SPACING,
        prompt_input::ROW_SPACING,
    ];
    for value in values {
        for retired in RETIRED {
            assert_ne!(
                value, retired,
                "a converted spacing has slid back to the retired off-grid {retired}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Guard 5 -- shape and stroke bindings
// ---------------------------------------------------------------------------

/// Border widths, radii and the blank-cell reservation come from `shape` and
/// the type scale rather than bare strokes.
#[test]
fn strokes_and_reservations_are_token_bound() {
    assert_eq!(code::CODE_BORDER_WIDTH, shape::BORDER_HAIRLINE);
    assert_eq!(code::CODE_BORDER_RADIUS, shape::RADIUS_WIDGET);
    assert_eq!(
        transcript::TURN_BORDER_WIDTH,
        shape::BORDER_HAIRLINE,
        "the user-turn bubble's stroke must be the hairline token"
    );
    assert_eq!(
        transcript::TURN_BORDER_RADIUS,
        shape::RADIUS_WIDGET,
        "the user-turn bubble's radius must be the widget-radius token"
    );
    assert_eq!(
        crate::ui_iced::render::SEPARATOR_WIDTH,
        shape::BORDER_HAIRLINE
    );
    assert_eq!(
        crate::ui_iced::render::table::EMPTY_CELL_HEIGHT,
        typography::BODY.line_height,
        "a blank table cell must reserve the line box its neighbours paint"
    );
    assert_eq!(
        code::CODE_COPY_COLUMN_WIDTH,
        size::CONTROL_SM + space::XXS,
        "the copy column is the compact control rung plus the micro-gap rung"
    );
}

// ---------------------------------------------------------------------------
// Guard 6 -- the tab-strip band is derived, not emergent
// ---------------------------------------------------------------------------

/// The tab-strip band closes on tokens.
///
/// 4.4.1 pins the strip box at `Height: 36px` and `size.control.lg` names
/// itself as the Tab Strip height. Before INT-7c there was **no strip-height
/// constant at all** -- the height was whatever the label, its padding and the
/// band inset added up to, which landed near 30 pt. The band is now explicit
/// and every part of it is derived from a rung, so retuning a rung moves the
/// band instead of leaving a height that no longer fits.
#[test]
fn the_tab_strip_band_closes_on_tokens() {
    // The band is the large structural rung, same as INT-7a's navbar.
    assert_eq!(center::STRIP_HEIGHT, size::CONTROL_LG);
    assert_eq!(center::STRIP_PADDING, space::XS);

    // The tab's height is derived from the band, not chosen.
    assert_eq!(
        center::TAB_HEIGHT,
        center::STRIP_HEIGHT - 2.0 * center::STRIP_PADDING
    );
    assert_eq!(
        center::TAB_HEIGHT,
        size::CONTROL_SM,
        "36 - 2x4 must land on the compact control rung"
    );

    // The band closes arithmetically: inset + tab + inset.
    assert_eq!(
        center::STRIP_PADDING + center::TAB_HEIGHT + center::STRIP_PADDING,
        center::STRIP_HEIGHT
    );

    // The vertical inset inside the tab is what the close target leaves over.
    assert_eq!(
        center::TAB_PADDING_Y,
        (center::TAB_HEIGHT - size::HIT_TARGET_MIN) / 2.0
    );
    assert_eq!(center::TAB_PADDING_Y, space::XXS);
    assert_eq!(
        center::TAB_CLOSE_HIT_TARGET + 2.0 * center::TAB_PADDING_Y,
        center::TAB_HEIGHT,
        "the 24x24 close target plus the tab's vertical inset must fill the tab"
    );

    // The label's line box fits inside the close target, so the label never
    // touches the tab's edges.
    assert!(
        typography::UI_COMPACT.line_height <= size::HIT_TARGET_MIN,
        "the ui_compact line box must fit inside the minimum hit target"
    );
}

/// The band is **taller than the sum of its contents**, which is what makes
/// it an explicit height rather than an emergent one.
#[test]
fn the_tab_strip_height_is_not_the_sum_of_its_contents() {
    let content_stack = typography::UI_COMPACT.line_height
        + 2.0 * center::TAB_PADDING_Y
        + 2.0 * center::STRIP_PADDING;

    assert!(
        center::STRIP_HEIGHT > content_stack,
        "the band ({}) must be an explicit height, not the emergent stack \
         ({content_stack}) it used to be; 4.4.1 pins 36",
        center::STRIP_HEIGHT
    );
    assert_ne!(
        center::STRIP_HEIGHT,
        content_stack,
        "an emergent band silently resizes when a type rung moves"
    );
}

/// The close affordance clears the hit-target floor.
///
/// Before INT-7c the close mark was only as large as its glyph plus iced's
/// default padding, i.e. below the 24 px floor every interactive control in
/// this interface is measured against.
#[test]
fn the_tab_close_button_clears_the_hit_target_floor() {
    assert_eq!(center::TAB_CLOSE_HIT_TARGET, size::HIT_TARGET_MIN);
    assert!(
        center::TAB_CLOSE_HIT_TARGET >= size::HIT_TARGET_MIN,
        "the close target must clear HIT_TARGET_MIN"
    );
    assert_eq!(center::TAB_CLOSE_GLYPH_SIZE, size::ICON_SM);
    assert_ne!(
        center::TAB_CLOSE_GLYPH_SIZE,
        center::TAB_TEXT_SIZE,
        "the close glyph must come from the icon scale, not the label's type rung"
    );
}

// The unsaved-marker guards (`the_unsaved_marker_is_a_shape_outside_the_type_floor
//` and `the_active_tab_dirty_dot_clears_the_fill_lock`) went with the marker
// itself. The type floor keeps its teeth the same way it did before: nothing in
// `converted_runtime_sizes` may render below `MIN_SIZE`, and none of the removed
// constants was ever a point size. The fill-lock rule those tests exercised is
// still pinned for the tab label in `center_tests.rs`

/// The active tab label takes the rung's emphasis weight, the inactive its
/// resting weight (4.4.2).
#[test]
fn the_active_tab_label_takes_the_emphasis_weight() {
    use hoarfrost::token::typography::Weight;

    // Compared through `Weight::iced()`, the framework's own mapping, because
    // `iced::font::Weight` has no reverse conversion into the token enum.
    assert_eq!(
        center::tab_label_font(true).weight,
        Weight::SemiBold.iced(),
        "4.4.2 gives the active tab a SemiBold label"
    );
    assert_eq!(
        center::tab_label_font(false).weight,
        Weight::Regular.iced(),
        "an inactive tab rests at Regular"
    );
    assert_ne!(
        center::tab_label_font(true).weight,
        center::tab_label_font(false).weight,
        "the active tab must be distinguishable by weight, not colour alone"
    );
}

// ---------------------------------------------------------------------------
// Guard 7 -- the scan's own file list is honest
// ---------------------------------------------------------------------------

/// The converted-file list resolves against the real tree, and the shared
/// scanner still sees the whole `ui_iced` directory.
#[test]
fn the_guard_scope_resolves_against_the_real_tree() {
    let root = ui_iced_root();
    assert!(root.is_dir(), "ui_iced root not found at {root:?}");

    for rel in CONVERTED_FILES {
        let path = root.join(rel);
        assert!(
            path.is_file(),
            "converted file {rel} is missing at {path:?}"
        );
    }

    // The shared scanner is not returning an empty list, which would make
    // every source-scan guard above vacuously true.
    let all = source_files(&root);
    assert!(
        all.len() >= CONVERTED_FILES.len(),
        "source_files found {} files under {root:?}; the converted set alone is {}",
        all.len(),
        CONVERTED_FILES.len()
    );
}
