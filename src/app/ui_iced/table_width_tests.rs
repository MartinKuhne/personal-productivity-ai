//! Unit tests for the iced FTWA adapter ([TASK-015]).
//!
//! Covers the three stages of the adapter contract: available-width →
//! solver-input mapping, the [`IcedTextMeasurer`] cell measurements, and
//! solver-output → iced column-width mapping. Measurement is driven by a
//! fixed-width fake ([`fixed_width`]: 5 px per body character, 10 px per
//! monospace character) so every assertion is exact.
//!
//! The [GAP-06] section exercises the real measurer ([`fira_sans_width`])
//! against iced's own Fira Sans bytes through `cosmic-text`. Those
//! assertions are deliberately *relational* — narrow < wide, monotone in
//! length, always finite, mono equal to the heuristic — rather than pinned
//! to literal pixel values, so they hold across Fira Sans releases while
//! still failing if the real path degrades into the heuristic.

use super::*;
use crate::markdown::model::{InlineElem, TextStyle};
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
    // Mirrors the legacy UI `test_measure_cell_fragmented_tokens`: two adjacent
    // runs with no whitespace between them are ONE unbreakable token.
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
fn test_trailing_whitespace_single_part_run_closes_the_token_like_the_legacy_renderer() {
    // Parity lock for the walker's `else if ends_ws` branch: a single-part
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

// --- real font metrics via cosmic-text ([GAP-06]) ----------------------

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
    // The whole point of GAP-06: the flat model cannot tell "iiii" from
    // "MMMM", real metrics can, and narrow comes out genuinely narrow.
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
    // width, real metrics solve them apart. This is the visible behaviour
    // change GAP-06 delivers.
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
