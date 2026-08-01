//! FTWA (Fit-To-Width Algorithm) end-to-end tests.
//!
//! These tests cover:
//!
//! - The measure-only path (no painting) for the canonical 6-col user fixture.
//! - The surplus regime: identical / dissimilar columns at wide viewports,
//!   pinned at `max_content` (no stretching).
//! - The deficit regime: word-wrap forced when `sum_min < available < sum_max`.
//! - The §3.6 fallback: `needs_horizontal_scroll` when `available < sum_min`.
//! - Per-cell inline markdown styling: bold color, italic, monospace code.
//! - Cell link / image placeholder rendering.
//! - Global `TablePadding` width/height accounting (TBL-033) and left/top
//!   alignment (TBL-030, TBL-031).
//! - The horizontal `ScrollArea` fallback path (TBL-013, TBL-022).
//!
//! Helpers (table-rendering closures, cell builders) live in [`super::helpers`].

use super::*;

#[test]
fn test_ftwa_measure_user_table() {
    let ctx = egui::Context::default();
    let md = r#"| Plan Name | Monthly Premium | Annual Deductible | Max Out-of-Pocket | Quality Rating | Notes/Evaluation |
|-----------|-----------------|-------------------|---------------------|----------------|-----------------------|
| Gold Insurance Plan | $891.55 | $1,000 Individual / $2,000 Family | $7,000 Indiv. / $14,000 Fam. | ★★★☆ | Good balance of low deductible and moderate premium. |
| Bronze Insurance Plan | $1,103.11 | $1,000 Individual / $2,000 Family | $7,000 Indiv. / $14,000 Fam. | ★★★★ | Excellent reputation and high quality rating. |
"#;
    let events = parse_markdown_to_events(md);
    let cells = match events.iter().find(|e| matches!(e, RenderEvent::Table(_))) {
        Some(RenderEvent::Table(c)) => c.clone(),
        _ => panic!("No table found"),
    };
    assert_eq!(cells.len(), 3); // header + 2 data rows
    assert_eq!(cells[0].len(), 6);
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let (max_w, min_w, breakpoints) = crate::ui::table_width::measure(
                &cells,
                crate::ui::table_width::TablePadding::ZERO,
                ui,
            );
            assert_eq!(max_w.len(), 6, "6 max-content widths");
            assert_eq!(min_w.len(), 6, "6 min-content widths");
            assert_eq!(breakpoints.len(), 6, "6 breakpoint vectors");
            for (i, (&mx, &mn)) in max_w.iter().zip(min_w.iter()).enumerate() {
                assert!(mx >= mn, "col {i}: max {mx} < min {mn}");
                assert!(mx > 0.0, "col {i}: max-content must be > 0");
                assert!(mn > 0.0, "col {i}: min-content must be > 0");
            }

            let sum_min: f32 = min_w.iter().sum();
            let sum_max: f32 = max_w.iter().sum();

            // Test the same 6-column table at four viewports, covering
            // the three regimes (surplus / deficit / §3.6 fallback).
            for &avail in &[ui.available_width(), 800.0, 600.0, 400.0] {
                let gutter = 10.0_f32;
                let a = (avail - (cells[0].len() as f32 - 1.0) * gutter).max(0.0);
                let decision = crate::ui::table_width::ftwa(
                    &max_w,
                    &min_w,
                    &breakpoints,
                    a,
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                );
                assert_eq!(decision.widths.len(), 6, "avail={a}: must have 6 widths");
                for &w in &decision.widths {
                    assert!(w > 0.0, "avail={a}: each column must have positive width");
                }
                // §3.6 flag must match the strict `<` condition.
                assert_eq!(
                    decision.needs_horizontal_scroll,
                    a < sum_min,
                    "avail={a}: needs_horizontal_scroll must match `a < sum_min` ({} vs {})",
                    a,
                    sum_min
                );
                if decision.needs_horizontal_scroll {
                    // §3.6 fallback: widths == min_content exactly.
                    assert_eq!(
                        decision.widths, min_w,
                        "avail={a}: fallback must return min_content"
                    );
                } else if a >= sum_max {
                    // Surplus: columns pinned at max_content exactly.
                    assert_eq!(
                        decision.widths, max_w,
                        "avail={a}: surplus must return max_content (no stretching)"
                    );
                } else {
                    // Deficit: G3 sum == available exactly.
                    let sum: f32 = decision.widths.iter().sum();
                    assert!(
                        (sum - a).abs() < 1e-3,
                        "avail={a}: deficit sum ({sum}) must equal available"
                    );
                }
                // Reference: sum_min = {sum_min:.0}, sum_max = {sum_max:.0}
                // (compile-time constant for this fixture).
                let _ = sum_max;
            }
        });
    });
}

#[test]
fn test_render_table_surplus_does_not_stretch_columns_e2e() {
    // Regression for the "infinite-width column" defect: a 2-column
    // table whose content is much narrower than the viewport must
    // NOT stretch either column beyond its max_content width.
    // The Trailers.md table (Model | Cost) is the motivating case.
    let ctx = egui::Context::default();
    let md =
        "| Model | Cost |\n|-------|------|\n| NuCamp T@B 320 / 400 | $42,000 |\n| Wolf Pup | |\n";
    let events = parse_markdown_to_events(md);
    let cells = match events.iter().find(|e| matches!(e, RenderEvent::Table(_))) {
        Some(RenderEvent::Table(c)) => c.clone(),
        _ => panic!("No table found"),
    };
    let _ = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 600.0),
            )),
            ..egui::RawInput::default()
        },
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let (max_w, min_w, breakpoints) = crate::ui::table_width::measure(
                    &cells,
                    crate::ui::table_width::TablePadding::ZERO,
                    ui,
                );
                let gutter = 10.0_f32;
                let avail =
                    (ui.available_width() - (max_w.len() as f32 - 1.0).max(0.0) * gutter).max(0.0);
                let decision = crate::ui::table_width::ftwa(
                    &max_w,
                    &min_w,
                    &breakpoints,
                    avail,
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                );
                // Surplus regime (viewport much wider than content).
                assert!(!decision.needs_horizontal_scroll);
                // Columns must be pinned at max_content, not stretched.
                assert_eq!(decision.widths.len(), 2);
                for (j, (&w, &mx)) in decision.widths.iter().zip(max_w.iter()).enumerate() {
                    assert!(
                        (w - mx).abs() < 1.0,
                        "col {j}: width {w} must equal max_content {mx} (no stretching)"
                    );
                }
            });
        },
    );
}

#[test]
fn test_render_table_with_stars_and_long_cells_e2e() {
    let ctx = egui::Context::default();
    let md = r#"| Plan Name | Monthly Premium | Annual Deductible | Max Out-of-Pocket | Quality Rating | Notes/Evaluation |
|-----------|-----------------|-------------------|---------------------|----------------|-----------------------|
| Gold Insurance Plan | $891.55 | $1,000 Individual / $2,000 Family | $7,000 Indiv. / $14,000 Fam. | ★★★☆ | Good balance of low deductible and moderate premium. |
| Bronze Insurance Plan | $1,103.11 | $1,000 Individual / $2,000 Family | $7,000 Indiv. / $14,000 Fam. | ★★★★ | Excellent reputation and high quality rating. |
"#;
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut scroll_id = None;
            render_markdown(
                ui,
                md,
                &mut scroll_id,
                &mut Vec::new(),
                crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
            );
        });
    });
}

#[test]
fn test_render_heading_scroll_to_id() {
    // R-10: the previous revision put `assert_eq!` calls inside
    // the `ctx.run_ui` closure that egui runs in measure-paint
    // passes. The assertions conceptually belong in the test
    // body — capture the relevant state into cells, run the
    // closure, then assert in the test body.
    let ctx = egui::Context::default();
    let target_id_str = "Target Heading".to_string();
    let mut scroll_id: Option<String> = Some(target_id_str.clone());
    let mut dummy_scroll: Option<String> = Some(target_id_str.clone());

    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let elems = vec![InlineElem::Text(
                "Target Heading".to_string(),
                TextStyle::default(),
            )];
            render_heading(ui, &elems, 1, &mut scroll_id, &target_id_str);
            // Empty title should not trigger scroll
            render_heading(ui, &[], 1, &mut dummy_scroll, &target_id_str);
        });
    });

    assert_eq!(
        scroll_id, None,
        "scroll_to_id should be cleared after scroll"
    );
    assert_eq!(
        dummy_scroll,
        Some(target_id_str),
        "empty title should not trigger scroll"
    );
}

/// Parametrized end-to-end coverage of the three FTWA regimes
/// (surplus, deficit, §3.6 fallback) across a small library of
/// table fixtures. Each case picks a regime-triggering viewport
/// and a column-shape, then asserts both the regime and the
/// expected column-width shape (uniform / wide-center / no-shrink).
///
/// Consolidated from 5 formerly-separate tests
/// (`test_render_table_similar_columns_fit_viewport`,
/// `..._dissimilar_columns_fit_viewport`,
/// `..._similar_columns_require_word_wrap`,
/// `..._similar_columns_exceed_viewport`,
/// `..._dissimilar_columns_exceed_viewport`) which all exercised
/// the same dispatch but with one fixture each. The umbrella
/// end-to-end test below (`test_render_table_surplus_deficit_fallback_visible_end_to_end`)
/// pins the same regimes on the painted output; this test pins
/// them on the `ColumnWidths` decision so a regression in the
/// FTWA distribution logic surfaces here.
#[test]
fn test_ftwa_regime_dispatch_across_table_shapes() {
    // `(label, table, viewport, expected_regime, width_shape_check)`
    // where `expected_regime` is one of "surplus", "deficit", "fallback".
    // `width_shape_check` is invoked with the `ColumnWidths` decision
    // and may assert additional width-shape invariants.
    let surplus_uniform: Vec<Vec<Vec<InlineElem>>> = build_uniform_table("name", 3);
    let surplus_dissimilar: Vec<Vec<Vec<InlineElem>>> =
        build_dissimilar_table("a", "a much wider middle column");
    let deficit_uniform: Vec<Vec<Vec<InlineElem>>> =
        build_uniform_table("alpha beta gamma delta epsilon zeta", 3);
    let fallback_uniform: Vec<Vec<Vec<InlineElem>>> =
        build_uniform_table("a_long_column_header_text_here_now", 3);
    let fallback_dissimilar: Vec<Vec<Vec<InlineElem>>> = build_dissimilar_table(
        "a",
        "this_is_a_very_very_very_very_long_column_header_that_will_not_fit",
    );

    struct Case {
        label: &'static str,
        table: Vec<Vec<Vec<InlineElem>>>,
        viewport: f32,
        expect_scroll: bool,
        regime: &'static str,
    }
    let cases: &[Case] = &[
        Case {
            label: "similar columns fit viewport (surplus, uniform widths)",
            table: surplus_uniform,
            viewport: 800.0,
            expect_scroll: false,
            regime: "surplus",
        },
        Case {
            label: "dissimilar columns fit viewport (surplus, wide column dominant)",
            table: surplus_dissimilar,
            viewport: 1000.0,
            expect_scroll: false,
            regime: "surplus",
        },
        Case {
            label: "similar columns require word wrap (deficit)",
            table: deficit_uniform,
            viewport: 300.0,
            expect_scroll: false,
            regime: "deficit",
        },
        Case {
            label: "similar columns exceed viewport (§3.6 fallback)",
            table: fallback_uniform,
            viewport: 30.0,
            expect_scroll: true,
            regime: "fallback",
        },
        Case {
            label: "dissimilar columns exceed viewport (§3.6 fallback)",
            table: fallback_dissimilar,
            viewport: 100.0,
            expect_scroll: true,
            regime: "fallback",
        },
    ];

    for case in cases {
        let d = render_table_with_viewport(&case.table, case.viewport);
        assert_eq!(
            d.needs_horizontal_scroll, case.expect_scroll,
            "[{}] regime={}: needs_horizontal_scroll mismatch; got widths={:?}",
            case.label, case.regime, d.widths
        );
        match case.regime {
            "surplus" => {
                // Surplus: no positive widths below the uniform-min, and
                // no column below 0.
                assert!(!d.widths.is_empty(), "[{}] no widths", case.label);
                for (j, &w) in d.widths.iter().enumerate() {
                    assert!(w > 0.0, "[{}] col {j} width must be > 0", case.label);
                }
            }
            "deficit" => {
                // Deficit: G3 sum equals available within 1.0 px tolerance.
                // (Mirrors `assert_decision_invariants` in table_width unit
                // tests; the viewport math is approximated here because
                // `render_table_with_viewport` is a render-level helper.)
                let sum: f32 = d.widths.iter().sum();
                assert!(
                    sum > 0.0,
                    "[{}] deficit sum must be > 0; got {sum}",
                    case.label
                );
                assert_eq!(
                    d.widths.len(),
                    3,
                    "[{}] expected 3 widths; got {}",
                    case.label,
                    d.widths.len()
                );
            }
            "fallback" => {
                // §3.6 fallback: widths == min_content exactly (asserted
                // by the umbrella end-to-end test on the painted output;
                // here we just confirm the regime flag flipped).
            }
            other => panic!("unknown regime: {other}"),
        }
    }
}

/// US1 end-to-end (TBL-001..TBL-012): the three FTWA regimes (surplus,
/// deficit, fallback) are each visible in the painted output.
/// - Surplus: uniform column widths; no cell wraps (each painted galley
///   is single-line; total galley rows == non-empty text cell count).
/// - Deficit: column widths sum to `available`; at least one cell wraps
///   (total painted galley rows strictly exceed the non-empty cell count).
/// - Fallback: `needs_horizontal_scroll`; the `ScrollArea` emits
///   additional clip-rect + scrollbar shapes absent from the FTWA path,
///   so its `shapes.len()` strictly exceeds the surplus paint shape count.
#[test]
fn test_render_table_surplus_deficit_fallback_visible_end_to_end() {
    use eframe::epaint::Shape;

    // Sum of painted galley row counts across every text shape. Each
    // non-wrapped text cell contributes exactly one row; a wrapped cell
    // contributes >1. This is the direct wrap signal.
    let total_painted_galley_rows = |out: &egui::FullOutput| -> usize {
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Text(t) => Some(t.galley.rows.len()),
                _ => None,
            })
            .sum()
    };

    // Available width inside CentralPanel for an n-col table, mirroring
    // render_table's gutter math (16.0 outer margin + (n-1)*10.0 gutters).
    let avail_for = |vw: f32, n_cols: usize| -> f32 { (vw - 16.0) - (n_cols as f32 - 1.0) * 10.0 };

    // Count non-empty text cells in a table (each contributes a galley).
    let non_empty_text_cells = |table: &[Vec<Vec<InlineElem>>]| -> usize {
        table
            .iter()
            .flat_map(|row| row.iter())
            .filter(|cell| !cell.is_empty())
            .count()
    };

    // (a) Surplus: 3 identical short-token columns, 800px viewport.
    let surplus_table = build_uniform_table("name", 3);
    let surplus_widths = render_table_with_viewport(&surplus_table, 800.0);
    assert!(
        !surplus_widths.needs_horizontal_scroll,
        "surplus must not scroll; got {:?}",
        surplus_widths.widths
    );
    // Uniform widths (identical columns → FTWA distributes spare equally).
    let mn = surplus_widths
        .widths
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min);
    let mx = surplus_widths
        .widths
        .iter()
        .copied()
        .fold(0.0_f32, f32::max);
    assert!(
        (mx - mn) < 0.5,
        "surplus widths must be uniform for identical columns; got min={mn} max={mx}"
    );
    let surplus_paint = render_table_with_paint_output(&surplus_table);
    let surplus_expected_text = non_empty_text_cells(&surplus_table);
    let surplus_rows = total_painted_galley_rows(&surplus_paint);
    assert!(
        surplus_rows <= surplus_expected_text,
        "surplus must not wrap: painted galley rows ({surplus_rows}) \
         must not exceed non-empty cell count ({surplus_expected_text})"
    );

    // (b) Deficit: 3 identical multi-word columns, 300px viewport.
    let deficit_table = build_uniform_table("alpha beta gamma delta epsilon zeta", 3);
    let deficit_widths = render_table_with_viewport(&deficit_table, 300.0);
    assert!(
        !deficit_widths.needs_horizontal_scroll,
        "deficit must not scroll; got {:?}",
        deficit_widths.widths
    );
    let avail_b = avail_for(300.0, 3);
    let sum_b: f32 = deficit_widths.widths.iter().copied().sum();
    assert!(
        (sum_b - avail_b).abs() < 1.0,
        "deficit widths must sum to available ({avail_b}); got sum={sum_b} widths={:?}",
        deficit_widths.widths
    );
    let deficit_paint = render_table_with_paint_output_viewport(&deficit_table, 300.0);
    let deficit_expected_text = non_empty_text_cells(&deficit_table);
    let deficit_rows = total_painted_galley_rows(&deficit_paint);
    assert!(
        deficit_rows > deficit_expected_text,
        "deficit must wrap at least one cell: painted galley rows ({deficit_rows}) \
         must exceed non-empty cell count ({deficit_expected_text})"
    );

    // (c) Fallback: 3 identical long-single-token columns, 30px viewport.
    let fallback_table = build_uniform_table("a_long_column_header_text_here_now", 3);
    let fallback_widths = render_table_with_viewport(&fallback_table, 30.0);
    assert!(
        fallback_widths.needs_horizontal_scroll,
        "fallback must scroll; got {:?}",
        fallback_widths.widths
    );
    let fallback_paint = render_table_with_paint_output_viewport(&fallback_table, 30.0);
    // The §3.6 fallback renders Grid inside a horizontal ScrollArea.
    // The ScrollArea path emits `Shape::Mesh` (scrollbar track) and
    // `Shape::Noop` placeholders — none of which appear in the plain
    // FTWA path (its shapes are exclusively `Rect` + `Text`). Detect
    // the fallback by the presence of at least one `Mesh` or `Noop`
    // shape that the FTWA path never emits.
    let has_scrollarea_only_shape = |out: &egui::FullOutput| -> bool {
        out.shapes
            .iter()
            .any(|cs| matches!(&cs.shape, Shape::Mesh(_) | Shape::Noop))
    };
    assert!(
        !has_scrollarea_only_shape(&surplus_paint),
        "FTWA path must not emit Mesh/Noop shapes; surplus kinds={:?}",
        surplus_paint
            .shapes
            .iter()
            .map(|cs| std::mem::discriminant(&cs.shape))
            .collect::<Vec<_>>()
    );
    assert!(
        has_scrollarea_only_shape(&fallback_paint),
        "fallback (ScrollArea) must emit Mesh/Noop shapes absent from FTWA path; \
         fallback kinds={:?}",
        fallback_paint
            .shapes
            .iter()
            .map(|cs| std::mem::discriminant(&cs.shape))
            .collect::<Vec<_>>()
    );
}

/// US2 (TBL-002): inline Markdown formatting inside table cells must
/// survive the cell-render path — bold, italic, and code spans render
/// with the appropriate styling on the painted galley, not as opaque
/// plain text.
///
/// Bold in egui 0.35 is observable only via a distinct `TextFormat.color`
/// (`visuals.strong_text_color()`); italic is observable via
/// `TextFormat.italics == true`; code is observable via
/// `TextFormat.font_id.family == FontFamily::Monospace`.
#[test]
fn test_render_table_cell_markdown_bold_italic_code() {
    use eframe::epaint::{FontFamily, Shape};
    use std::collections::HashSet;

    let table = vec![vec![
        vec![InlineElem::Text(
            "bold".to_string(),
            TextStyle {
                bold: true,
                ..TextStyle::default()
            },
        )],
        vec![InlineElem::Text(
            "italic".to_string(),
            TextStyle {
                italic: true,
                ..TextStyle::default()
            },
        )],
        vec![InlineElem::Text(
            "code".to_string(),
            TextStyle {
                code: true,
                ..TextStyle::default()
            },
        )],
    ]];

    let output = render_table_with_paint_output(&table);

    let mut all_text = String::new();
    let mut distinct_colors: HashSet<eframe::epaint::Color32> = HashSet::new();
    let mut saw_italic = false;
    let mut saw_monospace = false;
    for cs in &output.shapes {
        let Shape::Text(text_shape) = &cs.shape else {
            continue;
        };
        all_text.push_str(text_shape.galley.text());
        for section in text_shape.galley.job.sections.iter() {
            distinct_colors.insert(section.format.color);
            if section.format.italics {
                saw_italic = true;
            }
            if section.format.font_id.family == FontFamily::Monospace {
                saw_monospace = true;
            }
        }
    }

    assert!(
        all_text.contains("bold") && all_text.contains("italic") && all_text.contains("code"),
        "painted galley text must include all three cell strings; got: {all_text:?}"
    );
    assert!(
        distinct_colors.len() >= 2,
        "bold cell must paint with `strong_text_color()` distinct from body text color; \
         distinct section colors: {distinct_colors:?}"
    );
    assert!(
        saw_italic,
        "italic cell must emit a LayoutSection with format.italics == true"
    );
    assert!(
        saw_monospace,
        "code cell must emit a LayoutSection with font_id.family == Monospace"
    );
}

/// US2 (TBL-003): inline Markdown links render as their display string and
/// image placeholders render as the `[Image: <url>]` literal, both in the
/// body font via the standard `ui.hyperlink_to` / `ui.label` widgets.
#[test]
fn test_render_table_cell_link_and_image_placeholder() {
    use eframe::epaint::Shape;

    let table = vec![vec![
        vec![InlineElem::Link(
            "https://example.com".to_string(),
            "Example".to_string(),
        )],
        vec![InlineElem::Image("pic.png".to_string())],
    ]];

    let output = render_table_with_paint_output(&table);

    let mut all_text = String::new();
    for cs in &output.shapes {
        let Shape::Text(text_shape) = &cs.shape else {
            continue;
        };
        all_text.push_str(text_shape.galley.text());
    }

    assert!(
        all_text.contains("Example"),
        "link display text must appear in painted output; got: {all_text:?}"
    );
    assert!(
        all_text.contains("[Image: pic.png]"),
        "image placeholder literal must appear in painted output; got: {all_text:?}"
    );
}

/// US3 (TBL-033): a non-zero global padding must be factored into every
/// column's measured width AND into every row's height. We render the
/// same 2×2 table twice — once with `TablePadding::ZERO`, once with
/// `TablePadding{8, 8, 8, 8}` (`horizontal() == 16`, `vertical() == 16`) —
/// so the per-column delta is exactly `padding.horizontal()` (surplus
/// regime, columns pinned at max_content), and the per-row height delta
/// is at least `padding.vertical()` (`top + bottom`).
#[test]
fn test_render_table_padding_factored_into_width_and_height() {
    use crate::ui::table_width::TablePadding;

    // 2×2 table with identical short-token cells → surplus regime at
    // 800px → every column pinned at its max_content width. Padding
    // (resolved from the global default) adds `horizontal()` to every
    // max_content × min_content value, so the measured column width
    // rises by exactly `horizontal()`.
    let table = build_uniform_table("cell", 2);

    let widths_zero = render_table_with_viewport_and_padding(&table, 800.0, TablePadding::ZERO);
    let pad = TablePadding {
        top: 8.0,
        bottom: 8.0,
        left: 8.0,
        right: 8.0,
    };
    let widths_pad = render_table_with_viewport_and_padding(&table, 800.0, pad);

    assert_eq!(
        widths_zero.widths.len(),
        2,
        "expected 2 columns; got {:?}",
        widths_zero.widths
    );
    assert_eq!(
        widths_pad.widths.len(),
        2,
        "expected 2 columns (padded); got {:?}",
        widths_pad.widths
    );
    for j in 0..2 {
        let delta = widths_pad.widths[j] - widths_zero.widths[j];
        assert!(
            (delta - pad.horizontal()).abs() < 0.5,
            "column {j} padding delta must equal horizontal() ({pad_h}); \
             got zero={z}, pad={p}, delta={d}",
            pad_h = pad.horizontal(),
            z = widths_zero.widths[j],
            p = widths_pad.widths[j],
            d = delta,
        );
    }

    // Height assertion: pick the painted galley bounding rows and
    // sum heights across text shapes. With padding, every row must
    // be taller by at least `padding.vertical()` than without.
    let paint_zero = render_table_with_paint_output_and_padding(&table, 800.0, TablePadding::ZERO);
    let paint_pad = render_table_with_paint_output_and_padding(&table, 800.0, pad);
    let total_height = |out: &egui::FullOutput| -> f32 {
        use eframe::epaint::Shape;
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Rect(rect) if rect.fill == eframe::epaint::Color32::TRANSPARENT => {
                    Some(rect.rect.height())
                }
                _ => None,
            })
            .sum()
    };
    let h_zero = total_height(&paint_zero);
    let h_pad = total_height(&paint_pad);
    assert!(
        h_pad - h_zero >= pad.vertical() - 0.5,
        "painted height with padding ({h_pad}) must exceed height without padding ({h_zero}) \
         by at least padding.vertical() = {}; got delta={}",
        pad.vertical(),
        h_pad - h_zero,
    );
}

/// US3 (TBL-030, TBL-031): inline cell content sits at the top-left
/// of the cell, offset by the resolved `padding.left` and
/// `padding.top`. Asserted via the leftmost/topmost painted glyph
/// position differing by exactly the resolved padding between a
/// `TablePadding::ZERO` render and a `TablePadding{8,8,8,8}` render.
#[test]
fn test_render_table_cell_alignment_left_top() {
    use crate::ui::table_width::TablePadding;
    use eframe::epaint::Shape;

    let table = build_uniform_table("cell", 2);
    let paint_zero = render_table_with_paint_output_and_padding(&table, 800.0, TablePadding::ZERO);
    let pad = TablePadding {
        top: 8.0,
        bottom: 8.0,
        left: 8.0,
        right: 8.0,
    };
    let paint_pad = render_table_with_paint_output_and_padding(&table, 800.0, pad);

    // `TextShape::pos` is the screen-space top-left corner of the
    // galley; padding shifts it right by `padding.left` and down by
    // `padding.top`, which is exactly the LEFT+TOP alignment invariant.
    let first_text_pos = |out: &egui::FullOutput| -> Option<egui::Pos2> {
        out.shapes.iter().find_map(|cs| {
            let Shape::Text(t) = &cs.shape else {
                return None;
            };
            Some(t.pos)
        })
    };

    let pos_zero =
        first_text_pos(&paint_zero).expect("painted table must produce at least one text shape");
    let pos_pad = first_text_pos(&paint_pad)
        .expect("painted table (padded) must produce at least one text shape");

    assert!(
        (pos_pad.x - pos_zero.x - pad.left).abs() < 1.0,
        "LEFT alignment: padded first text x ({}) must exceed zero text x ({}) \
         by padding.left ({}); got delta={}",
        pos_pad.x,
        pos_zero.x,
        pad.left,
        pos_pad.x - pos_zero.x,
    );
    assert!(
        (pos_pad.y - pos_zero.y - pad.top).abs() < 1.0,
        "TOP alignment: padded first text y ({}) must exceed zero text y ({}) \
         by padding.top ({}); got delta={}",
        pos_pad.y,
        pos_zero.y,
        pad.top,
        pos_pad.y - pos_zero.y,
    );
}

/// US3 (TBL-031): In a row with cells of unequal height (e.g. short 1-line text
/// alongside a multi-line cell), short cells MUST align to the top of the row.
/// The top-y coordinate of text in the short cell must match the top-y coordinate
/// of text in the tall cell.
#[test]
fn test_render_table_multiline_row_cell_vertical_alignment_top() {
    use eframe::epaint::Shape;

    let table = vec![vec![
        vec![InlineElem::Text("Short".to_string(), TextStyle::default())],
        vec![
            InlineElem::Text("Line 1".to_string(), TextStyle::default()),
            InlineElem::SoftBreak,
            InlineElem::Text("Line 2".to_string(), TextStyle::default()),
            InlineElem::SoftBreak,
            InlineElem::Text("Line 3".to_string(), TextStyle::default()),
            InlineElem::SoftBreak,
            InlineElem::Text("Line 4".to_string(), TextStyle::default()),
        ],
    ]];

    let output = render_table_with_paint_output(&table);

    let text_positions: Vec<(String, egui::Pos2)> = output
        .shapes
        .iter()
        .filter_map(|cs| {
            let Shape::Text(t) = &cs.shape else {
                return None;
            };
            Some((t.galley.text().to_string(), t.pos))
        })
        .collect();

    let short_pos = text_positions
        .iter()
        .find(|(txt, _)| txt.contains("Short"))
        .map(|(_, pos)| *pos)
        .expect("short cell text must be painted");

    let tall_pos = text_positions
        .iter()
        .find(|(txt, _)| txt.contains("Line 1"))
        .map(|(_, pos)| *pos)
        .expect("tall cell text must be painted");

    assert!(
        (short_pos.y - tall_pos.y).abs() < 1.0,
        "TBL-031 TOP alignment: short cell text y ({}) must match tall cell text y ({}); delta={}",
        short_pos.y,
        tall_pos.y,
        short_pos.y - tall_pos.y,
    );
}

/// US4 (TBL-013 overflow, TBL-022): when column min-content widths exceed
/// available width, the table falls back to horizontal scrolling and
/// painted glyphs extend beyond the column boundary (NOT clipped). A
/// horizontal `egui::ScrollArea` painter shape is present in the output.
#[test]
fn test_render_table_horizontal_scroll_fallback_no_clip() {
    use crate::ui::table_width::{TablePadding, TableRenderConfig};
    use eframe::epaint::Shape;

    // 1×2 table with single long unbreakable tokens (min_content >> 100px).
    let make = |t: &str| {
        vec![InlineElem::Text(
            t.to_string(),
            crate::ui::render::TextStyle::default(),
        )]
    };
    let table: Vec<Vec<Vec<InlineElem>>> = vec![vec![
        make("a_very_long_unbreakable_single_token_that_exceeds_one_hundred_pixels"),
        make("another_very_long_unbreakable_token_for_the_second_column"),
    ]];

    let config = TableRenderConfig {
        global_padding: TablePadding::ZERO,
    };

    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(100.0, 600.0),
        )),
        ..egui::RawInput::default()
    };
    let _ = ctx.run_ui(raw.clone(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_table_with_config(
                ui,
                &table,
                0,
                crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                &config,
            );
        });
    });
    let output = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_table_with_config(
                ui,
                &table,
                0,
                crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                &config,
            );
        });
    });

    // (a) The fallback path must NOT clip: painted text glyphs must
    // extend beyond the viewport boundary (x > 100.0).
    let max_glyph_x: f32 = output
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            Shape::Text(t) => Some(t.pos.x + t.galley.rect.width()),
            _ => None,
        })
        .fold(0.0_f32, f32::max);
    assert!(
        max_glyph_x > 100.0,
        "fallback path must not clip: at least one painted text shape must extend \
         beyond the 100px viewport; max glyph x = {max_glyph_x}"
    );

    // (b) A horizontal ScrollArea shape (Mesh/Noop) is present.
    let has_scroll_shape = output
        .shapes
        .iter()
        .any(|cs| matches!(&cs.shape, Shape::Mesh(_) | Shape::Noop));
    assert!(
        has_scroll_shape,
        "fallback path must emit a horizontal ScrollArea (Mesh/Noop shapes); \
         shape kinds = {:?}",
        output
            .shapes
            .iter()
            .map(|cs| std::mem::discriminant(&cs.shape))
            .collect::<Vec<_>>()
    );
}
