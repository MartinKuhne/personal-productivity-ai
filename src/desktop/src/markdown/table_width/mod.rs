//! Fair Table Width Algorithm (FTWA) — assigns per-column pixel widths to a markdown/GFM table.
//!
//! Reconciles three lexicographically-ordered goals (see
//! `doc/planning/table-column-width-algorithm.md`):
//!   **G1** minimize total word-wrap (extra wrapped lines),
//!   **G2** minimize the number of columns that wrap, and
//!   **G3** use all available horizontal space.
//!
//! The pure `ftwa` core is independent of egui and is unit-tested in isolation;
//! `measure` bridges to egui text shaping to derive the input widths.

use crate::markdown::ast::InlineElem;
use eframe::egui;

/// Outcome of an FTWA computation: per-column pixel widths plus a flag telling
/// the caller that the available width is below the sum of min-content widths,
/// in which case the table physically cannot fit and horizontal scrolling must
/// be enabled (doc §3.6 fallback).
///
/// `widths.len()` matches the input column count. In the fallback case the
/// widths equal the min-content widths so any wrapping layout still respects
/// the never-break-a-token invariant.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnWidths {
    /// Per-column assigned pixel width, in input order.
    pub widths: Vec<f32>,
    /// `true` when `available < Σ min_content` — caller must enable horizontal scroll.
    pub needs_horizontal_scroll: bool,
}

/// Pure FTWA core. Solves the deficit regime exactly for G2 (fewest wrapping
/// columns) and approximately for G1 (within the chosen wrap set).
pub fn ftwa(max_content: &[f32], min_content: &[f32], available: f32) -> ColumnWidths {
    let n = max_content.len();
    assert_eq!(
        n,
        min_content.len(),
        "ftwa: max_content and min_content must have equal length"
    );

    if n == 0 {
        return ColumnWidths {
            widths: Vec::new(),
            needs_horizontal_scroll: false,
        };
    }

    assert!(
        available.is_finite(),
        "ftwa: available must be finite, got {available}"
    );
    assert!(
        max_content.iter().all(|x| x.is_finite()),
        "ftwa: max_content must be finite, got {max_content:?}"
    );
    assert!(
        min_content.iter().all(|x| x.is_finite()),
        "ftwa: min_content must be finite, got {min_content:?}"
    );

    assert!(
        available >= 0.0,
        "ftwa: available must be non-negative, got {available}"
    );
    assert!(
        max_content.iter().all(|&x| x >= 0.0),
        "ftwa: max_content must be non-negative, got {max_content:?}"
    );
    assert!(
        min_content.iter().all(|&x| x >= 0.0),
        "ftwa: min_content must be non-negative, got {min_content:?}"
    );

    for (j, (&mx, &mn)) in max_content.iter().zip(min_content.iter()).enumerate() {
        assert!(
            mx >= mn,
            "ftwa: max_content[{j}] = {mx} < min_content[{j}] = {mn} (invariant violation)"
        );
    }

    let sum_max: f32 = max_content.iter().copied().sum();
    let sum_min: f32 = min_content.iter().copied().sum();

    if available < sum_min {
        return ColumnWidths {
            widths: min_content.to_vec(),
            needs_horizontal_scroll: true,
        };
    }

    if available >= sum_max {
        let spare = available - sum_max;
        let n_f = n as f32;
        let mut widths: Vec<f32> = max_content
            .iter()
            .map(|&m| {
                let share = if sum_max > 0.0 {
                    spare * (m / sum_max)
                } else {
                    spare / n_f
                };
                m + share
            })
            .collect();
        let drift = available - widths.iter().copied().sum::<f32>();
        if drift.abs() > 0.0 {
            let target = (0..n)
                .max_by(|&a, &b| {
                    let ma = max_content[a];
                    let mb = max_content[b];
                    ma.partial_cmp(&mb)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(b.cmp(&a))
                })
                .expect("n > 0 (early-return above guarantees this)");
            widths[target] += drift;
        }
        return ColumnWidths {
            widths,
            needs_horizontal_scroll: false,
        };
    }

    let deficit = sum_max - available;

    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        let slack_a = max_content[a] - min_content[a];
        let slack_b = max_content[b] - min_content[b];
        slack_b
            .partial_cmp(&slack_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });

    let mut wrap_set: Vec<usize> = Vec::new();
    let mut acc = 0.0_f32;
    for &j in &order {
        if acc >= deficit {
            break;
        }
        let slack = max_content[j] - min_content[j];
        if slack <= 0.0 {
            continue;
        }
        wrap_set.push(j);
        acc += slack;
    }

    let mut widths = max_content.to_vec();
    for &j in &wrap_set {
        let slack_j = max_content[j] - min_content[j];
        let share = if acc > 0.0 {
            deficit * (slack_j / acc)
        } else {
            0.0
        };
        widths[j] = (max_content[j] - share).max(min_content[j]);
    }

    let drift = available - widths.iter().copied().sum::<f32>();
    if drift.abs() > 0.0 && !wrap_set.is_empty() {
        let target = *wrap_set
            .iter()
            .max_by(|&&a, &&b| {
                let sa = max_content[a] - min_content[a];
                let sb = max_content[b] - min_content[b];
                sa.partial_cmp(&sb)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.cmp(&a))
            })
            .expect("wrap_set is non-empty (checked above)");
        widths[target] = (widths[target] + drift).max(min_content[target]);
    }

    ColumnWidths {
        widths,
        needs_horizontal_scroll: false,
    }
}

pub fn measure(cells: &[Vec<Vec<InlineElem>>], ui: &egui::Ui) -> (Vec<f32>, Vec<f32>) {
    let n = cells.iter().map(|row| row.len()).max().unwrap_or(0);
    if n == 0 {
        return (Vec::new(), Vec::new());
    }

    let body_font = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .cloned()
        .unwrap_or_else(|| egui::FontId::proportional(14.0));
    let mono_font = egui::FontId::new(body_font.size, egui::FontFamily::Monospace);
    let color = egui::Color32::WHITE;

    let mut max_w = vec![0.0_f32; n];
    let mut min_w = vec![0.0_f32; n];

    for row in cells {
        for (j, cell) in row.iter().enumerate() {
            let (cell_max, cell_min) = measure_cell(cell, ui, &body_font, &mono_font, color);
            if cell_max > max_w[j] {
                max_w[j] = cell_max;
            }
            if cell_min > min_w[j] {
                min_w[j] = cell_min;
            }
        }
    }

    for w in &mut max_w {
        if *w <= 0.0 {
            *w = 1.0;
        }
    }
    for w in &mut min_w {
        if *w <= 0.0 {
            *w = 1.0;
        }
    }

    (max_w, min_w)
}

fn measure_cell(
    cell: &[InlineElem],
    ui: &egui::Ui,
    body_font: &egui::FontId,
    mono_font: &egui::FontId,
    color: egui::Color32,
) -> (f32, f32) {
    let mut max_w = 0.0_f32;
    let mut min_w = 0.0_f32;

    let mut current_token = String::new();
    let mut current_font = body_font;

    let measure_token = |tok: &str, font: &egui::FontId, min_w: &mut f32| {
        if !tok.is_empty() {
            let g = ui.fonts_mut(|f| f.layout_no_wrap(tok.to_string(), font.clone(), color));
            let w = g.size().x;
            if w > *min_w {
                *min_w = w;
            }
        }
    };

    for elem in cell {
        let (displayed, font) = match elem {
            InlineElem::Text(t, style) => {
                let f = if style.code { mono_font } else { body_font };
                (t.clone(), f)
            }
            InlineElem::Link(_, display) => (display.clone(), body_font),
            InlineElem::Image(url) => (format!("[Image: {}]", url), body_font),
            InlineElem::Html(h) => (h.clone(), body_font),
            InlineElem::SoftBreak => (" ".to_string(), body_font),
        };

        if displayed.is_empty() {
            continue;
        }

        let g = ui.fonts_mut(|f| f.layout_no_wrap(displayed.clone(), font.clone(), color));
        max_w += g.size().x;

        let parts: Vec<&str> = displayed.split_whitespace().collect();
        if parts.is_empty() {
            if !current_token.is_empty() {
                let tok = std::mem::take(&mut current_token);
                measure_token(&tok, current_font, &mut min_w);
            }
        } else {
            let starts_ws = displayed.chars().next().is_some_and(char::is_whitespace);
            let ends_ws = displayed.chars().last().is_some_and(char::is_whitespace);

            if starts_ws {
                if !current_token.is_empty() {
                    let tok = std::mem::take(&mut current_token);
                    measure_token(&tok, current_font, &mut min_w);
                }
                for &p in &parts[..parts.len() - 1] {
                    measure_token(p, font, &mut min_w);
                }
                if ends_ws {
                    measure_token(parts.last().unwrap(), font, &mut min_w);
                } else {
                    current_token.push_str(parts.last().unwrap());
                    current_font = font;
                }
            } else {
                current_token.push_str(parts[0]);
                if parts.len() > 1 {
                    let tok = std::mem::take(&mut current_token);
                    measure_token(&tok, current_font, &mut min_w);
                    for &p in &parts[1..parts.len() - 1] {
                        measure_token(p, font, &mut min_w);
                    }
                    if ends_ws {
                        measure_token(parts.last().unwrap(), font, &mut min_w);
                    } else {
                        current_token.push_str(parts.last().unwrap());
                        current_font = font;
                    }
                } else if ends_ws {
                    let tok = std::mem::take(&mut current_token);
                    measure_token(&tok, current_font, &mut min_w);
                } else {
                    current_font = font;
                }
            }
        }
    }

    if !current_token.is_empty() {
        measure_token(&current_token, current_font, &mut min_w);
    }

    (max_w, min_w)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(v: f32) -> f32 {
        (v * 1000.0).round() / 1000.0
    }
    fn round_vec(v: &[f32]) -> Vec<f32> {
        v.iter().map(|x| r(*x)).collect()
    }

    #[test]
    fn empty_input_returns_empty_widths() {
        let d = ftwa(&[], &[], 100.0);
        assert!(d.widths.is_empty());
        assert!(!d.needs_horizontal_scroll);
    }

    #[test]
    fn length_mismatch_panics() {
        let result = std::panic::catch_unwind(|| ftwa(&[10.0], &[10.0, 5.0], 20.0));
        assert!(result.is_err());
    }

    #[test]
    fn surplus_regime_distributes_proportional_to_max() {
        let max = [20.0, 80.0];
        let min = [10.0, 40.0];
        let d = ftwa(&max, &min, 150.0);
        assert!(!d.needs_horizontal_scroll);
        assert_eq!(round_vec(&d.widths), vec![30.0, 120.0]);
    }

    #[test]
    fn test_measure_cell_fragmented_tokens() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let cell_fragmented = vec![
                    InlineElem::Text("super".to_string(), crate::markdown::ast::TextStyle { bold: true, ..Default::default() }),
                    InlineElem::Text("califragilistic".to_string(), crate::markdown::ast::TextStyle::default()),
                ];
                let cell_single = vec![
                    InlineElem::Text("supercalifragilistic".to_string(), crate::markdown::ast::TextStyle::default()),
                ];
                let body_font = egui::FontId::proportional(14.0);
                let mono_font = egui::FontId::monospace(14.0);
                let color = egui::Color32::WHITE;

                let (_, min_frag) = measure_cell(&cell_fragmented, ui, &body_font, &mono_font, color);
                let (_, min_sing) = measure_cell(&cell_single, ui, &body_font, &mono_font, color);

                assert!(
                    (min_frag - min_sing).abs() < 1.0,
                    "fragmented token min_content {min_frag} should match single token min_content {min_sing}"
                );
            });
        });
    }
}
