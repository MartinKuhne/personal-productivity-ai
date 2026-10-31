//! Unit tests for the iced FTWA adapter.
//!
//! Covers the three stages of the adapter contract: available-width →
//! solver-input mapping, the [`IcedTextMeasurer`] cell measurements, and
//! solver-output → iced column-width mapping. Measurement is driven by a
//! fixed-width fake ([`fixed_width`]: 5 px per body character, 10 px per
//! monospace character) so every assertion is exact.
//!
//! The real-metrics section exercises the real measurer ([`fira_sans_width`])
//! against iced's own Fira Sans bytes through `cosmic-text`. Those
//! assertions are deliberately *relational* — narrow < wide, monotone in
//! length, always finite, mono equal to the heuristic — rather than pinned
//! to literal pixel values, so they hold across Fira Sans releases while
//! still failing if the real path degrades into the heuristic.

use super::*;
use crate::markdown::model::{InlineElem, TextStyle};
use crate::markdown::table_layout::LayoutCell;
use crate::markdown::table_width::CellTokens;
use iced::font::{Family, Font};

/// Deterministic fake measurer: 5 px per body char, 10 px per mono char.
fn fixed_width(text: &str, font: Font, _size: f32) -> f32 {
    if font.family == Family::Monospace {
        text.chars().count() as f32 * 10.0
    } else {
        text.chars().count() as f32 * 5.0
    }
}

/// Measurer over the fake, with the FastMD line height.
fn fake_measurer() -> IcedTextMeasurer<'static> {
    IcedTextMeasurer::new(
        &fixed_width,
        TextMetrics {
            body_size: 14.0,
            mono_size: 13.0,
            line_height: 18.0,
        },
    )
}

/// One-cell table shorthand.
fn cell(text: &str) -> Vec<InlineElem> {
    vec![InlineElem::Text(text.to_string(), TextStyle::default())]
}

/// A `rows x cols` table of `"c<r>c<c>"` labels.
fn grid(rows: usize, cols: usize) -> Vec<Vec<Vec<InlineElem>>> {
    (0..rows)
        .map(|r| {
            (0..cols)
                .map(|c| cell(&format!("r{r}c{c}")))
                .collect::<Vec<_>>()
        })
        .collect()
}

// --- heuristic width ---------------------------------------------------

#[test]
fn test_heuristic_width_scales_with_character_count() {
    assert_eq!(heuristic_width("", Font::DEFAULT, 10.0), 0.0);
    assert_eq!(
        heuristic_width("ab", Font::DEFAULT, 10.0),
        2.0 * PROPORTIONAL_AVG_ADVANCE * 10.0
    );
    assert_eq!(
        heuristic_width("abcd", Font::DEFAULT, 10.0),
        4.0 * PROPORTIONAL_AVG_ADVANCE * 10.0
    );
}

#[test]
fn test_heuristic_width_monospace_is_wider_than_proportional() {
    let body = heuristic_width("abc", Font::DEFAULT, 14.0);
    let mono = heuristic_width("abc", Font::MONOSPACE, 14.0);
    assert!(
        mono > body,
        "mono ({mono}) must advance more per character than proportional ({body})"
    );
}

#[test]
fn test_heuristic_width_counts_characters_not_bytes() {
    // 'é' is 2 UTF-8 bytes but one scalar value.
    assert_eq!(
        heuristic_width("é", Font::DEFAULT, 10.0),
        heuristic_width("a", Font::DEFAULT, 10.0)
    );
}

// --- available-width mapping -------------------------------------------

#[test]
fn test_solver_available_width_subtracts_outer_padding_only() {
    assert_eq!(solver_available_width(100.0), 100.0 - 2.0 * CELL_PADDING_X);
    assert_eq!(solver_available_width(20.0), 20.0 - 2.0 * CELL_PADDING_X);
}

#[test]
fn test_solver_available_width_clamps_at_zero() {
    assert_eq!(solver_available_width(0.0), 0.0);
    assert_eq!(solver_available_width(-500.0), 0.0);
    assert_eq!(solver_available_width(-2.0 * CELL_PADDING_X), 0.0);
}

#[test]
fn test_column_spacing_is_twice_cell_padding_plus_separator() {
    // The iced identity `spacing_x = 2 * padding_x + separator_x` with
    // separator_x = 0 must land on COLUMN_SPACING.
    assert_eq!(2.0 * CELL_PADDING_X, COLUMN_SPACING);
    assert_eq!(2.0 * CELL_PADDING_Y, ROW_SPACING);
}

// --- measurer: cell content --------------------------------------------

#[test]
fn test_measure_cell_single_token_max_equals_min() {
    let measurer = fake_measurer();
    let (max, min, tokens) = measurer.measure_cell(&cell("hello"));
    assert_eq!(max, 25.0);
    assert_eq!(min, 25.0, "a single token's min equals its max");
    assert_eq!(tokens.token_widths, vec![25.0]);
}

#[test]
fn test_measure_cell_multi_word_splits_into_tokens() {
    let measurer = fake_measurer();
    let (max, min, tokens) = measurer.measure_cell(&cell("hello world"));
    assert_eq!(max, 55.0, "max is the whole single-line string");
    assert_eq!(min, 25.0, "min is the longest token");
    assert_eq!(tokens.token_widths, vec![25.0, 25.0]);
}

#[test]
fn test_measure_cell_min_is_the_longest_token() {
    let measurer = fake_measurer();
    let (max, min, tokens) = measurer.measure_cell(&cell("a bb cccc"));
    assert_eq!(max, 9.0 * 5.0);
    assert_eq!(min, 20.0);
    assert_eq!(tokens.token_widths, vec![5.0, 10.0, 20.0]);
}

#[test]
fn test_measure_cell_fragmented_token_stays_one_token() {
    // Two adjacent runs with no whitespace between them are ONE
    // unbreakable token.
    let measurer = fake_measurer();
    let fragmented = vec![
        InlineElem::Text("super".to_string(), TextStyle::default()),
        InlineElem::Text("califragilistic".to_string(), TextStyle::default()),
    ];
    let single = cell("supercalifragilistic");

    let (_, min_frag, _) = measurer.measure_cell(&fragmented);
    let (_, min_single, _) = measurer.measure_cell(&single);
    assert_eq!(
        min_frag, min_single,
        "fragmented token min must equal the joined single-token min"
    );
    assert_eq!(min_frag, 20.0 * 5.0);
}

#[test]
fn test_measure_cell_code_run_measures_as_monospace() {
    let measurer = fake_measurer();
    let code_cell = vec![InlineElem::Text(
        "ab".to_string(),
        TextStyle {
            code: true,
            ..Default::default()
        },
    )];
    let (max, min, _) = measurer.measure_cell(&code_cell);
    assert_eq!(max, 20.0, "mono char = 10 px, so 2 chars = 20 px");
    assert_eq!(min, 20.0);
}

#[test]
fn test_measure_cell_image_uses_placeholder_text() {
    let measurer = fake_measurer();
    let (max, _, _) = measurer.measure_cell(&[InlineElem::Image("x.png".to_string())]);
    // "[Image: x.png]" is 14 characters.
    assert_eq!(max, 14.0 * 5.0);
}

#[test]
fn test_measure_cell_soft_break_counts_as_one_space() {
    let measurer = fake_measurer();
    let (max, _, _) = measurer.measure_cell(&[InlineElem::SoftBreak]);
    assert_eq!(max, 5.0);
}

#[test]
fn test_measure_cell_empty_cell_is_all_zero() {
    let measurer = fake_measurer();
    let (max, min, tokens) = measurer.measure_cell(&[]);
    assert_eq!((max, min), (0.0, 0.0));
    assert!(tokens.token_widths.is_empty());
}

#[test]
fn test_measure_cell_whitespace_only_closes_the_open_token() {
    let measurer = fake_measurer();
    let elems = vec![
        InlineElem::Text("hello".to_string(), TextStyle::default()),
        InlineElem::Text("   ".to_string(), TextStyle::default()),
        InlineElem::Text("world".to_string(), TextStyle::default()),
    ];
    let (max, min, tokens) = measurer.measure_cell(&elems);
    assert_eq!(max, 13.0 * 5.0);
    assert_eq!(min, 25.0);
    assert_eq!(tokens.token_widths, vec![25.0, 25.0]);
}

#[test]
fn test_trailing_whitespace_single_part_run_closes_the_token() {
    // The walker's `else if ends_ws` branch: a single-part
    // run that ends with whitespace closes the pending token immediately and
    // it is measured with the *previous* run's font, so the two runs stay two
    // separate tokens (they do NOT merge into one unbreakable token).
    let measurer = fake_measurer();
    let elems = vec![
        InlineElem::Text("ab ".to_string(), TextStyle::default()),
        InlineElem::Text("cd ".to_string(), TextStyle::default()),
    ];
    let (max, min, tokens) = measurer.measure_cell(&elems);
    assert_eq!(max, 30.0, "max counts every character, spaces included");
    assert_eq!(min, 10.0, "min is the longest single token");
    assert_eq!(tokens.token_widths, vec![10.0, 10.0]);
}

#[test]
fn test_space_width_and_line_height_come_from_the_metrics() {
    let measurer = fake_measurer();
    assert_eq!(measurer.space_width(), 5.0);
    assert_eq!(measurer.line_height(), 18.0);
}

#[test]
fn test_size_of_selects_by_font_family() {
    let measurer = fake_measurer();
    assert_eq!(measurer.size_of(Font::DEFAULT), 14.0);
    assert_eq!(measurer.size_of(Font::MONOSPACE), 13.0);
}

#[test]
fn test_font_handles_are_default_and_monospace() {
    assert_eq!(IcedTextMeasurer::body_font(), Font::DEFAULT);
    assert_eq!(IcedTextMeasurer::mono_font(), Font::MONOSPACE);
}

#[test]
fn test_heuristic_measurer_uses_the_heuristic_width_function() {
    let measurer = IcedTextMeasurer::heuristic(TextMetrics::FASTMD);
    let (max, _, _) = measurer.measure_cell(&cell("abcd"));
    assert_eq!(max, heuristic_width("abcd", Font::DEFAULT, 14.0));
}

// --- solver output → column widths -------------------------------------

#[test]
fn test_empty_table_solves_to_nothing() {
    let measurer = fake_measurer();
    let layout = solve_table_layout(
        &[],
        500.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    assert!(layout.rows.is_empty());
    assert!(column_widths(&layout).is_empty());
    assert!(!needs_horizontal_scroll(&layout));
}

#[test]
fn test_single_column_surplus_pins_to_max_content() {
    let measurer = fake_measurer();
    let layout = solve_table_layout(
        &[vec![cell("hello")]],
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    let widths = column_widths(&layout);
    assert_eq!(widths, vec![25.0], "surplus regime pins to max_content");
    assert!(!needs_horizontal_scroll(&layout));
}

#[test]
fn test_single_column_zero_width_forces_horizontal_scroll() {
    let measurer = fake_measurer();
    let layout = solve_table_layout(
        &[vec![cell("hello")]],
        0.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    assert!(
        needs_horizontal_scroll(&layout),
        "zero available width cannot hold even one column's min_content"
    );
    let widths = column_widths(&layout);
    assert_eq!(widths.len(), 1);
    assert!(
        widths[0] >= 25.0,
        "fallback keeps the never-break-a-token floor, got {}",
        widths[0]
    );
}

#[test]
fn test_very_narrow_viewport_falls_back_to_scroll() {
    let measurer = fake_measurer();
    let layout = solve_table_layout(
        &grid(2, 3),
        24.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    assert!(
        needs_horizontal_scroll(&layout),
        "24 px cannot fit three r#c# columns"
    );
    assert_eq!(column_widths(&layout).len(), 3);
}

#[test]
fn test_wide_viewport_fits_without_scroll() {
    let measurer = fake_measurer();
    let layout = solve_table_layout(
        &grid(2, 3),
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    assert!(!needs_horizontal_scroll(&layout));
    let widths = column_widths(&layout);
    assert_eq!(widths.len(), 3);
    let spent: f32 = widths.iter().sum::<f32>()
        + (widths.len() as f32 - 1.0) * COLUMN_SPACING
        + 2.0 * CELL_PADDING_X;
    assert!(
        spent <= 1000.0 + f32::EPSILON,
        "solved table must fit the viewport, wants {spent}"
    );
}

#[test]
fn test_many_columns_are_all_solved() {
    let measurer = fake_measurer();
    let layout = solve_table_layout(
        &grid(3, 12),
        2000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    let widths = column_widths(&layout);
    assert_eq!(widths.len(), 12);
    assert!(
        widths.iter().all(|w| *w > 0.0),
        "every column gets a positive width: {widths:?}"
    );
}

#[test]
fn test_horizontal_padding_is_folded_into_every_column() {
    let measurer = fake_measurer();
    let padded = TablePadding {
        top: 0.0,
        bottom: 0.0,
        left: 4.0,
        right: 4.0,
    };
    let layout = solve_table_layout(
        &[vec![cell("hello")]],
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        padded,
        &measurer,
    );
    // TBL-033: the assigned width covers content + horizontal padding.
    assert_eq!(column_widths(&layout), vec![25.0 + 8.0]);
}

#[test]
fn test_negative_padding_is_sanitised_before_solving() {
    let measurer = fake_measurer();
    let bad = TablePadding {
        top: -5.0,
        bottom: -5.0,
        left: -5.0,
        right: -5.0,
    };
    let layout = solve_table_layout(
        &[vec![cell("hello")]],
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        bad,
        &measurer,
    );
    assert_eq!(
        column_widths(&layout),
        vec![25.0],
        "negative padding clamps to zero and must not shrink the column"
    );
}

#[test]
fn test_column_widths_take_the_max_across_ragged_rows() {
    // A ragged table: row 0 has one column, row 1 has two.
    let measurer = fake_measurer();
    let cells = vec![vec![cell("aa")], vec![cell("bb"), cell("cccc")]];
    let layout = solve_table_layout(
        &cells,
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    let widths = column_widths(&layout);
    assert_eq!(widths.len(), 2, "the widest row sets the column count");
    assert!(
        widths.iter().all(|w| *w > 0.0),
        "no column collapses to zero: {widths:?}"
    );
}

#[test]
fn test_column_length_pins_a_fixed_width() {
    assert_eq!(column_length(120.0), Length::Fixed(120.0));
    assert_eq!(column_length(0.0), Length::Fixed(0.0));
    assert_eq!(
        column_length(-42.0),
        Length::Fixed(0.0),
        "a negative solved width must not produce a negative Length"
    );
}

#[test]
fn test_solver_input_uses_the_declared_strategy_unchanged() {
    // Every strategy must solve the same surplus case to the same pinned
    // widths — the adapter passes the strategy through without touching it.
    let measurer = fake_measurer();
    let strategies = [
        DeficitStrategy::ProportionalToSlack,
        DeficitStrategy::BreakpointWaterFill,
        DeficitStrategy::WaterFillRatio,
        DeficitStrategy::LagrangePenalty,
        DeficitStrategy::HybridMinPenaltyWaterFill,
    ];
    for strategy in strategies {
        let layout = solve_table_layout(
            &[vec![cell("hello")]],
            1000.0,
            strategy,
            TablePadding::ZERO,
            &measurer,
        );
        assert_eq!(
            column_widths(&layout),
            vec![25.0],
            "surplus must be strategy-independent for {strategy:?}"
        );
    }
}

#[test]
fn test_fira_sans_is_pinned_by_name_because_the_engine_also_holds_system_fonts() {
    // `FontSystem::new_with_fonts` calls `db.load_system_fonts()` before
    // adding the explicit sources
    // (`cosmic-text-0.15.0/src/font/system.rs:396-404`), so this engine
    // carries the host's whole installed-font collection alongside Fira
    // Sans — 517 faces on the machine this was measured on. Asking for the
    // face BY NAME is therefore not a nicety: it is the only thing that
    // pins the measurement to iced's Fira Sans instead of leaving it to
    // host-dependent generic-family resolution.
    let guard = match fira_sans_font_system().lock() {
        Ok(guard) => guard,
        Err(_) => panic!("shared Fira Sans engine lock was poisoned"),
    };
    let db = guard.db();

    let fira = db
        .query(&cosmic_text::fontdb::Query {
            families: &[cosmic_text::fontdb::Family::Name("Fira Sans")],
            ..Default::default()
        })
        .and_then(|id| db.face(id).map(|face| face.post_script_name.clone()));

    assert!(
        fira.as_deref().is_some_and(|n| n.starts_with("Fira")),
        "the by-name request must land on iced's own Fira Sans face, got {fira:?}"
    );
    assert!(
        db.faces().count() > 1,
        "the host font collection is expected to be loaded alongside Fira Sans; \
         if that ever stops being true the by-name pin is still correct but this \
         test should be revisited"
    );
}

#[test]
fn test_cell_tokens_type_is_reachable_from_the_adapter() {
    // Guards the re-export surface the render layer relies on.
    let measurer = fake_measurer();
    let (_, _, tokens): (f32, f32, CellTokens) = measurer.measure_cell(&cell("x y"));
    assert_eq!(tokens.token_widths.len(), 2);
}

// --- real font metrics via cosmic-text --------------------------------

/// Body size used across the real-metrics tests.
const REAL_BODY_SIZE: f32 = 14.0;

/// Corpus every real-metrics result must survive: ASCII, narrow and wide
/// glyphs, multi-byte UTF-8, emoji, a ZWJ family sequence, whitespace in
/// all its forms, a NUL byte, a zero-width space, and a 10 000-character
/// string.
fn real_metrics_corpus() -> Vec<String> {
    let mut corpus: Vec<String> = [
        "a",
        "ab",
        "iiii",
        "MMMM",
        "Hello, world!",
        "readme.md",
        "   ",
        "\t",
        " ",
        "\u{0}",
        "\u{200b}",
        "é",
        "ümlaut",
        "日本語",
        "Привет",
        "🎉",
        "👨‍👩‍👧‍👦",
        "mixed 日本語 é 🎉 tail",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    corpus.push("x".repeat(10_000));
    corpus
}

/// Measurer over the real Fira Sans metrics with the FastMD body metrics.
fn real_measurer() -> IcedTextMeasurer<'static> {
    IcedTextMeasurer::new(&fira_sans_width, TextMetrics::FASTMD)
}

#[test]
fn test_fira_sans_available_reports_the_loaded_face() {
    assert!(
        fira_sans_available(),
        "iced's FIRA_SANS_REGULAR must resolve as a face; if this is false the \
         measurer is silently answering with the heuristic"
    );
}

#[test]
fn test_fira_sans_width_empty_is_zero() {
    assert_eq!(fira_sans_width("", Font::DEFAULT, REAL_BODY_SIZE), 0.0);
    assert_eq!(fira_sans_width("", Font::MONOSPACE, REAL_BODY_SIZE), 0.0);
    assert_eq!(fira_sans_width("", Font::DEFAULT, 0.0), 0.0);
    assert_eq!(fira_sans_width("", Font::DEFAULT, f32::NAN), 0.0);
}

#[test]
fn test_fira_sans_width_separates_narrow_from_wide_glyphs() {
    // The flat model cannot tell "iiii" from "MMMM"; real metrics can, and
    // narrow comes out genuinely narrow.
    let narrow = fira_sans_width("iiii", Font::DEFAULT, REAL_BODY_SIZE);
    let wide = fira_sans_width("MMMM", Font::DEFAULT, REAL_BODY_SIZE);

    assert_eq!(
        heuristic_width("iiii", Font::DEFAULT, REAL_BODY_SIZE),
        heuristic_width("MMMM", Font::DEFAULT, REAL_BODY_SIZE),
        "the heuristic is glyph-blind: same char count, same width"
    );
    assert!(
        narrow < wide,
        "'iiii' ({narrow}) must measure narrower than 'MMMM' ({wide})"
    );
    assert!(
        wide > narrow * 1.5,
        "'MMMM' should be far wider than 'iiii', got {narrow} vs {wide}"
    );
    assert_ne!(
        narrow,
        heuristic_width("iiii", Font::DEFAULT, REAL_BODY_SIZE)
    );
    assert_ne!(wide, heuristic_width("MMMM", Font::DEFAULT, REAL_BODY_SIZE));
}

#[test]
fn test_fira_sans_width_is_monotonic_in_length() {
    assert!(fira_sans_width("a", Font::DEFAULT, REAL_BODY_SIZE) > 0.0);
    assert!(
        fira_sans_width("ab", Font::DEFAULT, REAL_BODY_SIZE)
            >= fira_sans_width("a", Font::DEFAULT, REAL_BODY_SIZE),
        "appending a glyph must not shrink the width"
    );
    assert!(
        fira_sans_width("abc", Font::DEFAULT, REAL_BODY_SIZE)
            > fira_sans_width("ab", Font::DEFAULT, REAL_BODY_SIZE)
    );
}

#[test]
fn test_fira_sans_width_is_monotonic_across_every_prefix() {
    // Walk every prefix of a mixed-script string; the running width must
    // never go backwards. This is the FTWA `max_content >= min_content`
    // invariant expressed over a single string.
    let base = "File.md 日本語 read me é 🎉 now";
    let chars: Vec<char> = base.chars().collect();
    let mut previous = 0.0_f32;
    for end in 0..=chars.len() {
        let prefix: String = chars[..end].iter().collect();
        let width = fira_sans_width(&prefix, Font::DEFAULT, REAL_BODY_SIZE);
        assert!(
            width >= previous,
            "prefix {prefix:?} measured {width}, below its predecessor {previous}"
        );
        previous = width;
    }
    assert!(previous > 0.0);
}

#[test]
fn test_fira_sans_width_grows_with_size() {
    let small = fira_sans_width("Hello World", Font::DEFAULT, 14.0);
    let large = fira_sans_width("Hello World", Font::DEFAULT, 28.0);
    assert!(
        large > small,
        "28pt ({large}) must be wider than 14pt ({small})"
    );
}

#[test]
fn test_fira_sans_width_multibyte_utf8_is_positive_and_finite() {
    for text in ["é", "ümlaut", "日本語", "Привет", "🎉", "👨‍👩‍👧‍👦"]
    {
        let width = fira_sans_width(text, Font::DEFAULT, REAL_BODY_SIZE);
        assert!(
            width.is_finite() && width > 0.0,
            "{text:?} measured {width}, want finite and positive"
        );
    }
}

#[test]
fn test_fira_sans_width_whitespace_only_does_not_panic_and_stays_positive() {
    for text in [" ", "   ", "\t", " \u{a0} "] {
        let width = fira_sans_width(text, Font::DEFAULT, REAL_BODY_SIZE);
        assert!(
            width.is_finite() && width > 0.0,
            "whitespace-only {text:?} measured {width}"
        );
    }
}

#[test]
fn test_fira_sans_width_very_long_string_is_finite() {
    let long = "x".repeat(10_000);
    let width = fira_sans_width(&long, Font::DEFAULT, REAL_BODY_SIZE);
    assert!(width.is_finite(), "10k chars produced {width}");
    assert!(width > 0.0);
    let short = "x".repeat(100);
    assert!(
        width > fira_sans_width(&short, Font::DEFAULT, REAL_BODY_SIZE),
        "10k glyphs must be wider than 100"
    );
}

#[test]
fn test_fira_sans_width_monospace_falls_back_to_the_heuristic_exactly() {
    // No monospace bytes are vendored, so mono must NOT be measured with
    // Fira Sans — it must be the heuristic, bit for bit.
    for text in ["a", "ab", "iiii", "MMMM", "hello world", "日本語", "\t"] {
        assert_eq!(
            fira_sans_width(text, Font::MONOSPACE, 13.0),
            heuristic_width(text, Font::MONOSPACE, 13.0),
            "mono {text:?} must equal the heuristic"
        );
    }
}

#[test]
fn test_fira_sans_width_proportional_samples_are_not_the_heuristic() {
    // Guards against the real path silently degrading into the fallback.
    let mut differed = 0;
    for text in ["iiii", "MMMM", "hello world", "readme.md", "é"] {
        if fira_sans_width(text, Font::DEFAULT, REAL_BODY_SIZE)
            != heuristic_width(text, Font::DEFAULT, REAL_BODY_SIZE)
        {
            differed += 1;
        }
    }
    assert_eq!(differed, 5, "every proportional sample should differ");
}

#[test]
fn test_fira_sans_width_degenerate_sizes_never_produce_nan_or_inf() {
    for size in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for font in [Font::DEFAULT, Font::MONOSPACE] {
            let width = fira_sans_width("hello", font, size);
            assert!(
                width.is_finite(),
                "size {size} with {font:?} produced {width}, which would panic the FTWA solver"
            );
            assert!(
                width >= 0.0,
                "size {size} produced a negative width {width}"
            );
        }
    }
}

#[test]
fn test_fira_sans_width_is_finite_and_non_negative_over_the_corpus() {
    for text in real_metrics_corpus() {
        for size in [1.0, 14.0, 64.0] {
            for font in [Font::DEFAULT, Font::MONOSPACE] {
                let width = fira_sans_width(&text, font, size);
                assert!(
                    width.is_finite() && width >= 0.0,
                    "{text:?} at {size} with {font:?} produced {width}"
                );
            }
        }
    }
}

#[test]
fn test_usable_width_accepts_only_finite_positive_measurements() {
    assert_eq!(usable_width(12.5, 99.0), 12.5);
    assert_eq!(usable_width(0.0001, 99.0), 0.0001);
    assert_eq!(usable_width(0.0, 99.0), 99.0, "zero is not a usable width");
    assert_eq!(usable_width(-1.0, 99.0), 99.0, "negative is not usable");
    assert_eq!(
        usable_width(f32::NAN, 99.0),
        99.0,
        "NaN must never reach the solver"
    );
    assert_eq!(usable_width(f32::INFINITY, 99.0), 99.0);
    assert_eq!(usable_width(f32::NEG_INFINITY, 99.0), 99.0);
}

#[test]
fn test_real_measurer_uses_the_real_metrics_function() {
    let measurer = real_measurer();
    let (max, _, _) = measurer.measure_cell(&cell("abcd"));
    assert_eq!(max, fira_sans_width("abcd", Font::DEFAULT, 14.0));
    assert_ne!(
        max,
        heuristic_width("abcd", Font::DEFAULT, 14.0),
        "the seam must be carrying the real measurer, not the heuristic"
    );
}

#[test]
fn test_real_measurer_cell_max_is_at_least_cell_min() {
    let measurer = real_measurer();
    for text in ["hello", "hello world", "a bb cccc", "日本語 text"] {
        let (max, min, tokens) = measurer.measure_cell(&cell(text));
        assert!(
            max.is_finite() && min.is_finite(),
            "{text:?} produced non-finite inputs"
        );
        assert!(min > 0.0, "{text:?} min must be positive, got {min}");
        assert!(
            max >= min,
            "{text:?}: max_content ({max}) must be >= min_content ({min})"
        );
        assert!(!tokens.token_widths.is_empty());
    }
}

#[test]
fn test_real_measurer_multi_word_cell_splits_into_tokens() {
    let measurer = real_measurer();
    let (max, min, tokens) = measurer.measure_cell(&cell("hello world"));
    assert_eq!(tokens.token_widths.len(), 2);
    assert!(
        max > min,
        "a two-word cell has slack to break at: {max} vs {min}"
    );
    assert_eq!(
        min,
        fira_sans_width("hello", Font::DEFAULT, 14.0).max(fira_sans_width(
            "world",
            Font::DEFAULT,
            14.0
        ))
    );
}

#[test]
fn test_real_measurer_code_run_still_falls_back_to_the_mono_heuristic() {
    let measurer = real_measurer();
    let code_cell = vec![InlineElem::Text(
        "ab".to_string(),
        TextStyle {
            code: true,
            ..Default::default()
        },
    )];
    let (max, min, _) = measurer.measure_cell(&code_cell);
    assert_eq!(max, heuristic_width("ab", Font::MONOSPACE, 13.0));
    assert_eq!(min, max);
}

#[test]
fn test_real_measurer_space_width_and_line_height_are_sane() {
    let measurer = real_measurer();
    let space = measurer.space_width();
    assert!(
        space.is_finite() && space > 0.0,
        "a real space advance must be positive, got {space}"
    );
    assert_eq!(measurer.line_height(), 18.0);
}

#[test]
fn test_real_measurer_solves_a_wide_table_without_scroll() {
    let measurer = real_measurer();
    let layout = solve_table_layout(
        &grid(2, 3),
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    assert!(!needs_horizontal_scroll(&layout));
    let widths = column_widths(&layout);
    assert_eq!(widths.len(), 3);
    assert!(
        widths.iter().all(|w| w.is_finite() && *w > 0.0),
        "real metrics must not collapse a column: {widths:?}"
    );
}

#[test]
fn test_real_measurer_zero_width_still_falls_back_to_scroll() {
    let measurer = real_measurer();
    let layout = solve_table_layout(
        &[vec![cell("hello")]],
        0.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    assert!(needs_horizontal_scroll(&layout));
    let widths = column_widths(&layout);
    assert_eq!(widths.len(), 1);
    assert!(widths[0].is_finite() && widths[0] > 0.0);
}

#[test]
fn test_real_metrics_change_solved_columns_the_heuristic_could_not() {
    // Narrow-vs-wide column pair: the heuristic solves both to the same
    // width, real metrics solve them apart — the visible behaviour change.
    let cells = vec![vec![cell("iiii"), cell("MMMM")]];
    let real = solve_table_layout(
        &cells,
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &real_measurer(),
    );
    let heuristic = solve_table_layout(
        &cells,
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &IcedTextMeasurer::heuristic(TextMetrics::FASTMD),
    );

    let real_widths = column_widths(&real);
    let heuristic_widths = column_widths(&heuristic);
    assert_eq!(real_widths.len(), 2);
    assert_eq!(heuristic_widths.len(), 2);
    assert!(
        real_widths[0] < heuristic_widths[0],
        "narrow column should shrink under real metrics: {real_widths:?} vs {heuristic_widths:?}"
    );
    assert!(
        real_widths[1] > heuristic_widths[1],
        "wide column should grow under real metrics: {real_widths:?} vs {heuristic_widths:?}"
    );
}
// --- `displayed`: the text and font a cell element actually paints -------

/// Inline code is the only run that renders monospace, and the measurer has to
/// pick the same font the renderer will use — otherwise the solver optimises a
/// width the table cannot hit.
#[test]
fn test_displayed_picks_the_monospace_font_only_for_code_runs() {
    let measurer = fake_measurer();

    let code_style = TextStyle {
        code: true,
        ..Default::default()
    };
    let (displayed, font) = measurer.displayed(&InlineElem::Text("git status".into(), code_style));
    assert_eq!(displayed, "git status", "a code run measures its own text");
    assert_eq!(
        font.family,
        Family::Monospace,
        "an inline-code run must be measured in the monospace font"
    );

    let (displayed, font) =
        measurer.displayed(&InlineElem::Text("plain".into(), TextStyle::default()));
    assert_eq!(displayed, "plain", "a plain run measures its own text");
    assert_ne!(
        font.family,
        Family::Monospace,
        "a plain run must be measured in the body font"
    );
}

/// Links, images and HTML all measure in the body font, and each measures the
/// text the renderer actually shows rather than its source markup.
#[test]
fn test_displayed_projects_every_non_text_element_into_the_body_font() {
    let measurer = fake_measurer();

    let (displayed, font) = measurer.displayed(&InlineElem::Link(
        "https://example.com".into(),
        "docs".into(),
    ));
    assert_eq!(
        displayed, "docs",
        "a link is measured by its display label, not its target URL"
    );
    assert_ne!(
        font.family,
        Family::Monospace,
        "links render in the body font"
    );

    let (displayed, font) = measurer.displayed(&InlineElem::Image("diagram.png".into()));
    assert_eq!(
        displayed,
        crate::markdown::model::image_placeholder("diagram.png"),
        "an image is measured by the placeholder text the renderer paints"
    );
    assert_ne!(
        font.family,
        Family::Monospace,
        "images measure in the body font"
    );

    let (displayed, font) = measurer.displayed(&InlineElem::Html("<b>hi</b>".into()));
    assert_eq!(
        displayed, "<b>hi</b>",
        "raw HTML is measured by its literal source"
    );
    assert_ne!(
        font.family,
        Family::Monospace,
        "HTML measures in the body font"
    );

    let (displayed, font) = measurer.displayed(&InlineElem::SoftBreak);
    assert_eq!(
        displayed, " ",
        "a soft break contributes exactly one space of width"
    );
    assert_ne!(
        font.family,
        Family::Monospace,
        "a soft break measures in the body font"
    );
}

// --- `record_token`: folding one token into the column's curve -----------

/// An empty token is not a token: it must not appear in the width list nor
/// move the min-content floor. Every real token is appended in call order,
/// and the floor only ever rises.
#[test]
fn test_record_token_skips_empty_tokens_and_records_widths_in_order() {
    let measurer = fake_measurer();
    let mut min_w = 0.0f32;
    let mut widths: Vec<f32> = Vec::new();

    measurer.record_token("", Font::DEFAULT, &mut min_w, &mut widths);
    assert!(
        widths.is_empty(),
        "an empty token has no width to contribute to the wrap curve"
    );
    assert_eq!(
        min_w, 0.0,
        "an empty token must not raise the min-content floor"
    );

    // `fixed_width` charges 5 px per body character.
    measurer.record_token("ab", Font::DEFAULT, &mut min_w, &mut widths);
    measurer.record_token("abcd", Font::DEFAULT, &mut min_w, &mut widths);
    assert_eq!(
        widths,
        vec![10.0, 20.0],
        "each token's width is recorded in the order it was seen"
    );
    assert_eq!(min_w, 20.0, "min-content tracks the widest token so far");

    measurer.record_token("a", Font::DEFAULT, &mut min_w, &mut widths);
    assert_eq!(
        min_w, 20.0,
        "a narrower token must never lower the min-content floor"
    );
    assert_eq!(
        widths.len(),
        3,
        "the narrower token still counts as a break opportunity"
    );
}

/// A monospace token is measured at the mono size, which is what keeps an
/// inline-code column from being sized as if it were body text.
#[test]
fn test_record_token_measures_monospace_tokens_with_the_mono_font() {
    let measurer = fake_measurer();
    let mut min_w = 0.0f32;
    let mut widths: Vec<f32> = Vec::new();

    measurer.record_token("abc", Font::MONOSPACE, &mut min_w, &mut widths);

    assert_eq!(
        widths,
        vec![30.0],
        "`fixed_width` charges 10 px per monospace character"
    );
    assert_eq!(min_w, 30.0, "the mono width sets the floor");
}

// --- `TablePadding`: the framework-free padding type --------------------

/// The `Default` impl is a separate item from `ZERO`; both must agree so a
/// caller that writes `TablePadding::default()` gets the same geometry as one
/// that writes `TablePadding::ZERO`.
#[test]
fn test_table_padding_default_agrees_with_the_zero_constant() {
    assert_eq!(
        TablePadding::default(),
        TablePadding::ZERO,
        "Default must not drift from the documented zero constant"
    );
    let pad = TablePadding::default();
    assert_eq!(
        (pad.top, pad.bottom, pad.left, pad.right),
        (0.0, 0.0, 0.0, 0.0),
        "every side must start at zero"
    );
    assert_eq!(pad.horizontal(), 0.0);
    assert_eq!(pad.vertical(), 0.0);
}

/// `sanitised()` clamps component-by-component, not as a whole: a padding with
/// mixed signs keeps its positive sides and zeroes only the negative ones.
#[test]
fn test_table_padding_sanitised_clamps_each_axis_independently() {
    let mixed = TablePadding {
        top: -3.0,
        bottom: 2.0,
        left: -1.0,
        right: 0.0,
    };
    let clean = mixed.sanitised();

    assert_eq!(
        (clean.top, clean.bottom, clean.left, clean.right),
        (0.0, 2.0, 0.0, 0.0),
        "only the negative components may be rewritten, got {clean:?}"
    );
    assert_eq!(
        clean.sanitised(),
        clean,
        "sanitising an already-clean padding must be a no-op"
    );
}

/// The two sums read their own axes only: horizontal never sees top/bottom and
/// vertical never sees left/right.
#[test]
fn test_table_padding_sums_only_their_own_axis() {
    let pad = TablePadding {
        top: 1.5,
        bottom: 2.5,
        left: 4.0,
        right: 6.0,
    };
    assert_eq!(pad.horizontal(), 10.0, "left + right");
    assert_eq!(pad.vertical(), 4.0, "top + bottom");

    // A padding that is purely vertical contributes nothing to width
    // accounting, and vice versa.
    let only_vertical = TablePadding {
        top: 7.0,
        bottom: 7.0,
        left: 0.0,
        right: 0.0,
    };
    assert_eq!(
        only_vertical.horizontal(),
        0.0,
        "vertical padding must not inflate TBL-033 width accounting"
    );
    let only_horizontal = TablePadding {
        top: 0.0,
        bottom: 0.0,
        left: 7.0,
        right: 7.0,
    };
    assert_eq!(
        only_horizontal.vertical(),
        0.0,
        "horizontal padding must not inflate row-height accounting"
    );
}

/// The render layer stores `TablePadding` in `RenderContext` and compares/clones
/// it there, so the derived `Copy`/`Clone`/`PartialEq`/`Debug` are live API.
#[test]
fn test_table_padding_derives_copy_clone_eq_and_debug() {
    let pad = TablePadding {
        top: 1.0,
        bottom: 2.0,
        left: 3.0,
        right: 4.0,
    };
    let copied = pad; // `Copy`: `pad` must still be usable afterwards
    assert_eq!(copied, pad, "a copy must compare equal to its source");

    // `Clone` is exercised through a container so the `Copy` type is not
    // `.clone()`d directly (that is a clippy lint).
    let boxed = vec![pad, TablePadding::ZERO];
    assert_eq!(
        boxed.clone(),
        boxed,
        "cloning a collection must preserve every padding"
    );

    assert_ne!(
        pad,
        TablePadding {
            top: 1.0,
            bottom: 2.0,
            left: 3.0,
            right: 4.5,
        },
        "a different right padding must compare unequal"
    );

    let rendered = format!("{pad:?}");
    for field in ["top", "bottom", "left", "right"] {
        assert!(
            rendered.contains(field),
            "`Debug` must name every side for diagnostics, got {rendered}"
        );
    }
}

// --- `TextMetrics` -------------------------------------------------------

/// The `FASTMD` constant is the single source of the body/mono/line sizes the
/// whole iced table path measures at.
#[test]
fn test_text_metrics_fastmd_pins_body_mono_and_line_height() {
    assert_eq!(TextMetrics::FASTMD.body_size, 14.0);
    assert_eq!(TextMetrics::FASTMD.mono_size, 13.0);
    assert_eq!(TextMetrics::FASTMD.line_height, 18.0);

    // The two sizes only reach the table through the measurer's `size_of`, so
    // the claim is checked on the answer it gives per font rather than on the
    // struct fields: inline code is rendered smaller than body text by design.
    let measurer = IcedTextMeasurer::heuristic(TextMetrics::FASTMD);
    assert!(
        measurer.size_of(IcedTextMeasurer::mono_font())
            < measurer.size_of(IcedTextMeasurer::body_font()),
        "inline code is rendered smaller than body text by design"
    );
}

/// `TextMetrics` is carried by value through the measurer constructors, so its
/// derived `Copy`/`Clone`/`PartialEq`/`Debug` are part of the adapter surface.
#[test]
fn test_text_metrics_derives_copy_clone_eq_and_debug() {
    let metrics = TextMetrics {
        body_size: 16.0,
        mono_size: 12.0,
        line_height: 20.0,
    };
    let copied = metrics; // `Copy`: `metrics` must still be usable afterwards
    assert_eq!(copied, metrics, "a copy must preserve every size");

    // `Clone` is exercised through a container so the `Copy` type is not
    // `.clone()`d directly (that is a clippy lint).
    let all = vec![metrics, TextMetrics::FASTMD];
    assert_eq!(
        all.clone(),
        all,
        "cloning a collection must preserve every metrics set"
    );

    assert_ne!(
        metrics,
        TextMetrics {
            body_size: 16.0,
            mono_size: 12.0,
            line_height: 21.0,
        },
        "a different line height must compare unequal"
    );
    assert!(
        format!("{metrics:?}").contains("body_size"),
        "`Debug` must name the fields for diagnostics"
    );
}

// --- `measure_cell`: element-boundary token walking ----------------------

/// An element that displays nothing (an empty text run, or a link with an empty
/// label) contributes no width and must not disturb the token being built.
#[test]
fn test_measure_cell_skips_elements_that_display_nothing() {
    let measurer = fake_measurer();
    let elems = vec![
        InlineElem::Text(String::new(), TextStyle::default()),
        InlineElem::Link("https://example.com".to_string(), String::new()),
        InlineElem::Text("ok".to_string(), TextStyle::default()),
    ];
    let (max, min, tokens) = measurer.measure_cell(&elems);
    assert_eq!(max, 10.0, "empty elements must add zero to max_content");
    assert_eq!(min, 10.0);
    assert_eq!(
        tokens.token_widths,
        vec![10.0],
        "an empty element must not inject a zero-width break opportunity"
    );
}

/// A run that *starts* with whitespace closes whatever token the previous run
/// left open, so "hello" + " world" are two breakable tokens, not one 11-char
/// unbreakable token.
#[test]
fn test_measure_cell_leading_whitespace_closes_the_previous_token() {
    let measurer = fake_measurer();
    let elems = vec![
        InlineElem::Text("hello".to_string(), TextStyle::default()),
        InlineElem::Text(" world".to_string(), TextStyle::default()),
    ];
    let (max, min, tokens) = measurer.measure_cell(&elems);
    assert_eq!(max, 55.0, "max_content still counts the whole line");
    assert_eq!(
        min, 25.0,
        "the leading space must break the runs apart, not join them"
    );
    assert_eq!(tokens.token_widths, vec![25.0, 25.0]);
}

/// A leading-whitespace run carrying several words records its interior tokens
/// immediately and, ending in whitespace, also records its last word — the
/// column gains every break opportunity the text offers.
#[test]
fn test_measure_cell_leading_whitespace_multiword_run_records_every_token() {
    let measurer = fake_measurer();
    let elems = vec![
        InlineElem::Text("a".to_string(), TextStyle::default()),
        InlineElem::Text(" bb ccc dd ".to_string(), TextStyle::default()),
    ];
    let (max, min, tokens) = measurer.measure_cell(&elems);
    assert_eq!(max, 60.0, "1 + 11 characters at 5 px");
    assert_eq!(min, 15.0, "'ccc' is the widest token");
    assert_eq!(
        tokens.token_widths,
        vec![5.0, 10.0, 15.0, 10.0],
        "order must follow the text: a, bb, ccc, dd"
    );
}

/// Leading whitespace with no token pending is simply a no-op flush — the run
/// must still contribute its own word rather than drop it.
#[test]
fn test_measure_cell_leading_whitespace_with_no_pending_token_still_records_its_word() {
    let measurer = fake_measurer();
    let (max, min, tokens) =
        measurer.measure_cell(&[InlineElem::Text(" hi".to_string(), TextStyle::default())]);
    assert_eq!(max, 15.0, "the leading space still costs its own advance");
    assert_eq!(min, 10.0);
    assert_eq!(
        tokens.token_widths,
        vec![10.0],
        "the leading space is trimmed away, 'hi' remains one token"
    );
}

/// A multi-word run that *ends* in whitespace closes its final word, so the
/// next run cannot silently glue itself onto the end of it.
#[test]
fn test_measure_cell_multiword_run_ending_in_whitespace_closes_the_last_token() {
    let measurer = fake_measurer();
    let (max, min, tokens) =
        measurer.measure_cell(&[InlineElem::Text("ab cd ".to_string(), TextStyle::default())]);
    assert_eq!(max, 30.0, "max_content counts the trailing space");
    assert_eq!(min, 10.0);
    assert_eq!(
        tokens.token_widths,
        vec![10.0, 10.0],
        "'ab cd ' must close both tokens, leaving nothing pending"
    );

    // The trailing space therefore also prevents a following run from merging.
    let joined = measurer.measure_cell(&[
        InlineElem::Text("ab cd ".to_string(), TextStyle::default()),
        InlineElem::Text("ef".to_string(), TextStyle::default()),
    ]);
    assert_eq!(
        joined.2.token_widths,
        vec![10.0, 10.0, 10.0],
        "'ef' starts a fresh token after the closed run"
    );
    assert_eq!(
        joined.1, 10.0,
        "no 4-character 'cdef' token may appear, min would be 20"
    );
}

/// The closed token is measured with the font of the run that *started* it, not
/// the font of the run that closed it — inline code stays measured as code even
/// when a following body run supplies the whitespace.
#[test]
fn test_measure_cell_closed_token_keeps_the_font_of_the_run_that_started_it() {
    let measurer = fake_measurer();
    let code = TextStyle {
        code: true,
        ..Default::default()
    };
    let elems = vec![
        InlineElem::Text("abc".to_string(), code),
        InlineElem::Text(" de".to_string(), TextStyle::default()),
    ];
    let (max, min, tokens) = measurer.measure_cell(&elems);
    assert_eq!(max, 45.0, "3 mono chars at 10 px + 3 body chars at 5 px");
    assert_eq!(
        tokens.token_widths,
        vec![30.0, 10.0],
        "'abc' must be recorded at the monospace width (30), not body (15)"
    );
    assert_eq!(min, 30.0);
}

/// A leading-whitespace run whose last word is not whitespace leaves that word
/// pending, so a following run can still glue onto it.
#[test]
fn test_measure_cell_leading_whitespace_run_leaves_its_tail_token_open() {
    let measurer = fake_measurer();
    let elems = vec![
        InlineElem::Text("x".to_string(), TextStyle::default()),
        InlineElem::Text(" yy zzz".to_string(), TextStyle::default()),
    ];
    let (max, min, tokens) = measurer.measure_cell(&elems);
    assert_eq!(max, 40.0, "1 + 7 characters at 5 px");
    assert_eq!(min, 15.0);
    assert_eq!(
        tokens.token_widths,
        vec![5.0, 10.0, 15.0],
        "'x' closes, 'yy' is interior, 'zzz' is flushed at the end"
    );

    // And the tail really was pending: a following run glues onto it.
    let glued = measurer.measure_cell(&[
        InlineElem::Text("x".to_string(), TextStyle::default()),
        InlineElem::Text(" yy zzz".to_string(), TextStyle::default()),
        InlineElem::Text("!".to_string(), TextStyle::default()),
    ]);
    assert_eq!(
        glued.2.token_widths,
        vec![5.0, 10.0, 20.0],
        "'zzz' + '!' have no whitespace between them and form one 20 px token"
    );
}
// --- `size_of` / font-family routing -------------------------------------

/// Only the `Monospace` family selects the mono size; every generic family and
/// every named face is body text, which is what keeps a `Family::Name` run from
/// silently being laid out at the code size.
#[test]
fn test_size_of_selects_mono_size_for_the_monospace_family_only() {
    let measurer = fake_measurer();
    for font in [
        Font::DEFAULT,
        Font::with_name("Fira Sans"),
        Font {
            family: Family::Serif,
            ..Font::DEFAULT
        },
        Font {
            family: Family::Cursive,
            ..Font::DEFAULT
        },
        Font {
            family: Family::Fantasy,
            ..Font::DEFAULT
        },
    ] {
        assert_eq!(
            measurer.size_of(font),
            TextMetrics::FASTMD.body_size,
            "{font:?} must be measured at the body size"
        );
    }
    assert_eq!(
        measurer.size_of(Font::MONOSPACE),
        TextMetrics::FASTMD.mono_size,
        "only Font::MONOSPACE selects the mono size"
    );
}

/// The heuristic's mono branch keys on the `Monospace` *variant*, not on the
/// family string: a face named "Monospace" is a `Family::Name` and advances at
/// the proportional rate.
#[test]
fn test_heuristic_width_only_treats_the_monospace_variant_as_monospace() {
    assert_eq!(
        heuristic_width("ab", Font::with_name("Monospace"), 10.0),
        2.0 * PROPORTIONAL_AVG_ADVANCE * 10.0,
        "a face merely *named* Monospace is not the monospace variant"
    );
    assert_eq!(
        heuristic_width("ab", Font::MONOSPACE, 10.0),
        2.0 * MONOSPACE_AVG_ADVANCE * 10.0
    );
}

// --- real-metrics routing -----------------------------------------------

/// Every non-monospace family is shaped as Fira Sans by name, so the answer is
/// machine-independent: a missing face and the generic families all land on the
/// same number as `Font::DEFAULT`.
#[test]
fn test_fira_sans_width_answers_identically_for_every_non_mono_family() {
    let expected = fira_sans_width("Hello, world!", Font::DEFAULT, REAL_BODY_SIZE);
    for font in [
        Font::with_name("Fira Sans"),
        Font::with_name("A Face This Build Does Not Ship"),
        Font {
            family: Family::Serif,
            ..Font::DEFAULT
        },
        Font {
            family: Family::Cursive,
            ..Font::DEFAULT
        },
        Font {
            family: Family::Fantasy,
            ..Font::DEFAULT
        },
    ] {
        assert_eq!(
            fira_sans_width("Hello, world!", font, REAL_BODY_SIZE),
            expected,
            "{font:?} must be measured as Fira Sans by name, not resolved \
             against whatever the host happens to have installed"
        );
    }
}

/// The public measurement is the raw shaping result whenever that result is
/// usable, and the heuristic's guaranteed-positive answer whenever it is not —
/// pinned per string rather than assumed, so a glyph that shapes to zero width
/// cannot collapse a column.
#[test]
fn test_fira_sans_width_uses_the_raw_shape_unless_it_is_unusable() {
    for text in [
        "MMMM",
        "iiii",
        "Hello, world!",
        "readme.md",
        "\u{200b}",
        "\u{0}",
    ] {
        let raw = measured_fira_sans_width(text, REAL_BODY_SIZE);
        let width = fira_sans_width(text, Font::DEFAULT, REAL_BODY_SIZE);
        if raw.is_finite() && raw > 0.0 {
            assert_eq!(
                width, raw,
                "{text:?}: a usable measurement ({raw}) must be passed through"
            );
        } else {
            assert_eq!(
                width,
                heuristic_width(text, Font::DEFAULT, REAL_BODY_SIZE),
                "{text:?}: an unusable measurement ({raw}) must fall back to \
                 the heuristic so the solver never sees 0/NaN/inf"
            );
        }
        assert!(
            width.is_finite() && width >= 0.0,
            "{text:?} ended at {width}, which the FTWA solver would reject"
        );
    }
}

/// The raw shaper is a pure function of its input: asking twice gives the same
/// number, so the shared `Mutex<FontSystem>` never leaks state between calls.
#[test]
fn test_measured_fira_sans_width_is_repeatable_across_calls() {
    let first = measured_fira_sans_width("repeatable", REAL_BODY_SIZE);
    let second = measured_fira_sans_width("repeatable", REAL_BODY_SIZE);
    assert_eq!(
        first, second,
        "the shared engine must not change an unchanged measurement"
    );
    assert!(
        first > 0.0 && first.is_finite(),
        "a real string must shape to a positive finite width, got {first}"
    );
}

// --- padding through the solver -----------------------------------------

/// Vertical padding is a row-height concern only: it raises the solved cell
/// height by `top + bottom` and leaves every column width untouched.
#[test]
fn test_vertical_padding_raises_row_height_without_touching_widths() {
    let measurer = fake_measurer();
    let solve = |padding: TablePadding| {
        solve_table_layout(
            &[vec![cell("hello")]],
            1000.0,
            DeficitStrategy::HybridMinPenaltyWaterFill,
            padding,
            &measurer,
        )
    };

    let plain = solve(TablePadding::ZERO);
    let padded = solve(TablePadding {
        top: 2.0,
        bottom: 3.0,
        left: 0.0,
        right: 0.0,
    });

    assert_eq!(
        plain.rows[0][0].height, 18.0,
        "one line at the FASTMD line height"
    );
    assert_eq!(
        padded.rows[0][0].height,
        18.0 + 5.0,
        "one line plus top + bottom padding"
    );
    assert_eq!(
        column_widths(&padded),
        column_widths(&plain),
        "vertical padding must not change any column width"
    );
}

/// A mixed-sign padding is sanitised per component before solving: the negative
/// left band is dropped while the positive right band still widens the column.
#[test]
fn test_mixed_sign_padding_widens_by_the_positive_band_only() {
    let measurer = fake_measurer();
    let layout = solve_table_layout(
        &[vec![cell("hello")]],
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding {
            top: -5.0,
            bottom: 0.0,
            left: -4.0,
            right: 6.0,
        },
        &measurer,
    );
    assert_eq!(
        column_widths(&layout),
        vec![25.0 + 6.0],
        "the -4 left band must be dropped and the +6 right band kept"
    );
    assert_eq!(
        layout.rows[0][0].height, 18.0,
        "a negative top band must not shrink the row below one line"
    );
}

// --- `column_widths` on hand-built layouts ------------------------------

/// A layout cell with a width and no content, for driving `column_widths`
/// directly instead of only through the solver.
fn layout_cell(width: f32) -> LayoutCell {
    LayoutCell {
        x: 0.0,
        y: 0.0,
        width,
        height: 18.0,
        content: Vec::new(),
    }
}

/// A column's width is the maximum over its cells, whichever row that cell
/// sits in — a later, narrower cell must not overwrite an earlier wide one.
#[test]
fn test_column_widths_take_the_maximum_cell_in_every_column() {
    let layout = TableLayout {
        rows: vec![
            vec![layout_cell(10.0), layout_cell(40.0)],
            vec![layout_cell(25.0), layout_cell(7.0)],
        ],
        total_width: 0.0,
        total_height: 0.0,
        needs_horizontal_scroll: false,
    };
    assert_eq!(
        column_widths(&layout),
        vec![25.0, 40.0],
        "column 0 takes the later, wider cell; column 1 keeps its earlier max"
    );
}

/// Ragged the other way: the *first* row is the longest, and the shorter rows
/// must not shift or drop the trailing columns.
#[test]
fn test_column_widths_survive_rows_shorter_than_the_widest() {
    let layout = TableLayout {
        rows: vec![
            vec![layout_cell(11.0), layout_cell(22.0), layout_cell(33.0)],
            vec![layout_cell(99.0)],
        ],
        total_width: 0.0,
        total_height: 0.0,
        needs_horizontal_scroll: false,
    };
    assert_eq!(
        column_widths(&layout),
        vec![99.0, 22.0, 33.0],
        "the short row only widens column 0; columns 1 and 2 keep their widths"
    );
}

/// A layout that has rows but no cells in them still has no columns.
#[test]
fn test_column_widths_of_a_layout_with_empty_rows_is_empty() {
    let layout = TableLayout {
        rows: vec![Vec::new(), Vec::new()],
        total_width: 0.0,
        total_height: 0.0,
        needs_horizontal_scroll: false,
    };
    assert!(
        column_widths(&layout).is_empty(),
        "the number of rows is not the number of columns"
    );
}

/// A fractional solved width reaches iced unchanged — the column is pinned to
/// exactly the number FTWA produced, not a rounded approximation.
#[test]
fn test_column_length_keeps_a_fractional_width_exact() {
    assert_eq!(column_length(0.25), Length::Fixed(0.25));
    assert_eq!(column_length(1.0 / 3.0), Length::Fixed(1.0 / 3.0));
    assert_eq!(
        column_length(12345.675),
        Length::Fixed(12345.675),
        "no rounding may happen on the way to the iced Length"
    );
}

/// The available-width clamp is exact at the boundary: a viewport exactly as
/// wide as the two outer bands leaves the solver nothing, and one pixel more
/// leaves exactly one pixel.
#[test]
fn test_solver_available_width_boundaries_around_the_outer_bands() {
    assert_eq!(
        solver_available_width(2.0 * CELL_PADDING_X),
        0.0,
        "a viewport of exactly the two outer bands leaves nothing to solve with"
    );
    assert_eq!(
        solver_available_width(2.0 * CELL_PADDING_X - 0.1),
        0.0,
        "just under the bands must clamp, not go negative"
    );
    assert_eq!(
        solver_available_width(2.0 * CELL_PADDING_X + 1.0),
        1.0,
        "one pixel past the bands must reach the solver as one pixel"
    );
}
