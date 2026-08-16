//! End-to-end smoke tests for the top-level render entry points.
//!
//! These tests verify that `render_markdown` and `render_yaml_table`
//! run end-to-end through egui (`ctx.run_ui`) without panicking, and
//! pin a few targeted regressions for long-body wrapping and the
//! YAML metadata table that motivated the long-value wrap fix.
//!
//! The wider FTWA / column / cell / interaction tests live in the
//! sibling submodules.

use super::*;
use crate::ui::test_helpers::run_ui_test;

/// Render the given closure in a 320px-wide viewport and assert that
/// a text shape containing `needle` has a galley with more than one
/// row (i.e. it wrapped) and a width that does not exceed the
/// viewport + 1 px tolerance. This is the canonical "long text must
/// wrap inside the panel, not overflow" assertion used by the YAML
/// table and the long-paragraph regression tests below.
fn assert_long_text_wraps_in_viewport(render: impl FnMut(&mut egui::Ui), needle: &str) {
    use crate::ui::test_helpers::text::extract_text;

    let viewport_width: f32 = 320.0;
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, 800.0),
        )),
        ..egui::RawInput::default()
    };
    let ctx = egui::Context::default();
    let mut render = render;
    let output = run_ui_test(&ctx, raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render(ui);
        });
    });

    // Sanity: the long text must actually have been rendered.
    let texts = extract_text(&output.shapes);
    assert!(
        texts.iter().any(|t| t.contains(needle)),
        "expected text containing {needle:?} to be rendered; got {} text shape(s): {:?}",
        texts.len(),
        texts,
    );

    // Locate the Text shape whose galley carries the long text.
    let shape = output
        .shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text().contains(needle) => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected a Text shape for needle {needle:?}"));
    let galley = &shape.galley;

    // 1. The text must wrap: more than one row in the galley.
    assert!(
        galley.rows.len() > 1,
        "expected text containing {needle:?} to word-wrap; got galley with {} row(s) and \
         rect width={:.1}px (viewport={viewport_width:.0}px) — the text is overflowing \
         instead of wrapping",
        galley.rows.len(),
        galley.rect.width(),
    );

    // 2. The wrapped text must fit inside the viewport.
    let max_allowed = viewport_width;
    assert!(
        galley.rect.width() <= max_allowed + 1.0,
        "expected wrapped text width <= {max_allowed:.0}px; got {:.1}px \
         (the value is overflowing the panel — the inner horizontal ScrollArea \
         is clipping it)",
        galley.rect.width(),
    );
}

#[test]
fn test_render_markdown_e2e() {
    let ctx = egui::Context::default();
    let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut scroll_id = None;
            render_markdown(
                ui,
                "# Test\n\n- [ ] Task\n\n```rust\nlet x = 1;\n```",
                &mut scroll_id,
                &mut Vec::new(),
                crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                None,
            );

            let yaml_str = "a: 1\nb: 2";
            let val: serde_norway::Value = serde_norway::from_str(yaml_str).unwrap();
            render_yaml_table(ui, &val);
        });
    });
}

#[test]
fn test_render_markdown_all_elements_e2e() {
    let ctx = egui::Context::default();
    let md = r#"# Heading 1
## Heading 2
### Heading 3

Paragraph with **bold**, *italic*, ~~strikethrough~~, `inline code`, [link](https://example.com), and ![img](https://example.com/img.png).

- [ ] Unchecked Task
- [x] Checked Task
- Regular list item

| Header 1 | Header 2 |
| --- | --- |
| Cell 1 | Cell 2 |

---

> Blockquote text

```python
def foo():
    return 42
```

<div>Html block</div>
"#;
    let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut scroll_id = None;
            render_markdown(
                ui,
                md,
                &mut scroll_id,
                &mut Vec::new(),
                crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                None,
            );

            // Render non-mapping YAML table
            let non_map = serde_norway::Value::String("test".to_string());
            render_yaml_table(ui, &non_map);
        });
    });
}

#[test]
fn test_render_table_with_empty_cells_e2e() {
    let ctx = egui::Context::default();
    let md = "| A | | C |\n|---|---|---|\n| | B | |";
    let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut scroll_id = None;
            render_markdown(
                ui,
                md,
                &mut scroll_id,
                &mut Vec::new(),
                crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                None,
            );
        });
    });
}

/// Regression: long text surfaces in the renderer must word-wrap
/// inside the panel rather than overflow and get clipped by the
/// inner horizontal `ScrollArea`.
///
/// The two formerly-separate tests
/// (`test_render_yaml_table_wraps_long_values_within_viewport` and
/// `test_render_markdown_long_paragraph_wraps_in_preview`) were
/// 95% identical — same viewport, same needle-based shape lookup,
/// same `galley.rows.len() > 1` + `width <= viewport + 1 px`
/// assertions. The only real difference was the markdown source:
/// a long YAML summary value vs. a long German paragraph. Both
/// regressions were caused by the same egui 0.35 wrap-mode
/// default change, so they belong together.
#[test]
fn test_long_text_surfaces_wrap_within_viewport() {
    // YAML case: render_yaml_table with a long summary value.
    // The exact string the user reported in the screenshot.
    let long_yaml_summary = "January 2005 invoice from Example Publishing GmbH for \
        ExampleCorp half-year archive CD-ROMs, shipped tax-free to J. Doe at \
        ExampleCorp in Springfield, IL, USA, for archive and product-evaluation purposes \
        under ExampleCorp product license terms.";
    let yaml_str = format!(
        "title: Example Publishing Invoice for ExampleCorp Product — Tax-Free Export Delivery\n\
         summary: \"{long_yaml_summary}\"\n\
         tags: [invoice, receipt, technology, documents]\n\
         header-date: 2026-07-22T19:32:47Z\n"
    );
    let yaml: serde_norway::Value = serde_norway::from_str(&yaml_str).unwrap();

    // Markdown case: long German paragraph mirroring the user's
    // `Mythical man-month.md` body (~570 chars).
    let long_paragraph = "Es ist ein Mix an Methoden im Einsatz. \
        Einerseits soll im traditionellen Projektmanagement in Voraus \
        der Funktionsumfang und die Projektdauer feststehen. Die zur \
        Planung notwendige Dokumentation der Anforderungen, Technologien \
        und Risken findet aber nicht statt. Daraufhin trägt das \
        ausführende Team ein erhebliches Risiko, wenn sich die \
        Anforderungen ändern oder die Arbeit komplexer ist als erwartet.";
    let md = format!("\n{long_paragraph}\n");

    type WrapCase = (&'static str, &'static str, Box<dyn FnMut(&mut egui::Ui)>);
    let cases: Vec<WrapCase> = vec![
        (
            "YAML table long summary",
            "Example Publishing GmbH",
            Box::new(move |ui| {
                render_yaml_table(ui, &yaml);
            }),
        ),
        (
            "Markdown long paragraph",
            "erhebliches Risiko, wenn sich",
            Box::new(move |ui| {
                let mut scroll_id = None;
                render_markdown(
                    ui,
                    &md,
                    &mut scroll_id,
                    &mut Vec::new(),
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                    None,
                );
            }),
        ),
    ];

    for (label, needle, render) in cases {
        assert_long_text_wraps_in_viewport(render, needle);
        let _ = label; // surfaces via the per-assertion diagnostic
    }
}

/// Regression: multi-line YAML front-matter values must expand grid row height
/// so subsequent rows do not print over the wrapped text lines.
#[test]
fn test_render_yaml_table_row_height_prevents_text_overlap() {
    let ctx = egui::Context::default();
    let viewport_width: f32 = 320.0;
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, 800.0),
        )),
        ..egui::RawInput::default()
    };
    let long_summary = "Cloud native software architect profile focused on scalable, \
        performant, and resilient services. Highlights skills in legacy application \
        migration to microservices and building developer communities.";
    let yaml_str = format!(
        "title: 2020 LinkedIn Profile - Cloud Native Architect\n\
         summary: \"{long_summary}\"\n\
         tags: [professional, career, linkedin, architect]\n"
    );
    let yaml: serde_norway::Value = serde_norway::from_str(&yaml_str).unwrap();

    let output = run_ui_test(&ctx, raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_yaml_table(ui, &yaml);
        });
    });

    // Find the summary value text shape and the tags key/value text shape.
    let summary_shape = output
        .shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text().contains("microservices") => Some(t),
            _ => None,
        })
        .expect("expected summary text shape");

    let tags_shape = output
        .shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text() == "tags" => Some(t),
            _ => None,
        })
        .expect("expected tags key text shape");

    let summary_bottom_y = summary_shape.pos.y + summary_shape.galley.rect.height();
    let tags_top_y = tags_shape.pos.y;

    assert!(
        tags_top_y >= summary_bottom_y,
        "expected tags row (y={tags_top_y:.1}) to start below summary wrapped content (bottom y={summary_bottom_y:.1}), but it overlapped!",
    );
}

/// Regression: multi-line markdown table cells must respect FTWA column widths (fit viewport)
/// or enable horizontal scrolling when min_w exceeds viewport, and expand grid row height
/// so subsequent rows do not overlap.
#[test]
fn test_render_table_multiline_cells_fit_panel_and_expand_row_height() {
    let ctx = egui::Context::default();

    // 1. Table that fits within viewport (3 columns, 600px viewport)
    let fit_table = vec![
        vec![
            vec![InlineElem::Text("Make".into(), Default::default())],
            vec![InlineElem::Text("Model".into(), Default::default())],
            vec![InlineElem::Text("Summary".into(), Default::default())],
        ],
        vec![
            vec![InlineElem::Text("Acer".into(), Default::default())],
            vec![InlineElem::Text("Swift 16 AI".into(), Default::default())],
            vec![InlineElem::Text("Excellent value. Vibrant OLED touch screen, lightweight (~1.5kg), everyday performance.".into(), Default::default())],
        ],
        vec![
            vec![InlineElem::Text("Dell".into(), Default::default())],
            vec![InlineElem::Text("XPS 16".into(), Default::default())],
            vec![InlineElem::Text("Dell's flagship premium laptop.".into(), Default::default())],
        ],
    ];

    let viewport_width: f32 = 600.0;
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, 800.0),
        )),
        ..egui::RawInput::default()
    };

    let output = run_ui_test(&ctx, raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_table(
                ui,
                &fit_table,
                0,
                crate::ui::table_width::DeficitStrategy::BreakpointWaterFill,
            );
        });
    });

    // Summary text in Row 1 must fit inside viewport
    let summary_shape = output
        .shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text().contains("Vibrant OLED") => Some(t),
            _ => None,
        })
        .expect("expected row 1 summary text shape");

    let summary_right_x = summary_shape.pos.x + summary_shape.galley.rect.width();
    assert!(
        summary_right_x <= viewport_width + 1.0,
        "expected summary text right x ({summary_right_x:.1}) to fit inside viewport ({viewport_width:.1})",
    );

    // Row 2 (Dell) must start below Row 1 Summary text bottom Y (no text overlap)
    let dell_shape = output
        .shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text() == "Dell" => Some(t),
            _ => None,
        })
        .expect("expected dell text shape");

    let summary_bottom_y = summary_shape.pos.y + summary_shape.galley.rect.height();
    let dell_top_y = dell_shape.pos.y;

    assert!(
        dell_top_y >= summary_bottom_y,
        "expected dell row (y={dell_top_y:.1}) to start below summary wrapped content (bottom y={summary_bottom_y:.1}), but it overlapped!",
    );
}

#[test]
fn test_render_table_with_bold_and_special_chars_e2e() {
    let ctx = egui::Context::default();
    let md = "| Name | Account | Amount | Type |\n|---|---|---|---|\n| **Vanguard** | #12345678 | $1 | Taxable (investment) |";
    let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut scroll_id = None;
            render_markdown(
                ui,
                md,
                &mut scroll_id,
                &mut Vec::new(),
                crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                None,
            );
        });
    });
}

/// Regression: in the agent response window, long lines that wrap had
/// their continuation lines centered within the line width because
/// `ui.horizontal_wrapped` defaults `main_align` to `Align::Center`.
/// For a wrapped continuation row narrower than the line width, the
/// centered placement landed the row at a positive x offset, causing
/// the leftmost characters to fall outside the visible left edge of
/// the scroll viewport — the "text cut off on the left on subsequent
/// lines" symptom.
///
/// The fix in `render_inline` pins `main_align: Min` via
/// `Layout::left_to_right(egui::Align::Min).with_main_wrap(true)`, so
/// every wrapped row starts at the galley's left edge (offset = 0 px
/// from the first row's `x`).
///
/// Mirrors the production render path: the agent response is rendered
/// inside a vertical `ScrollArea` by
/// `src/ui/panels/center.rs:180-224`, so the test must do the same.
#[test]
fn test_render_inline_wrapped_rows_left_aligned() {
    use eframe::epaint::Shape;

    // 50 short space-separated tokens — wide enough to force several
    // wraps at a 200 px viewport, with clear word-boundary break
    // points so cosmic_text doesn't overflow a single token.
    let long_text: String = "alpha ".repeat(50);
    let elems = vec![InlineElem::Text(long_text.clone(), TextStyle::default())];

    let viewport_width: f32 = 200.0;
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, 600.0),
        )),
        ..egui::RawInput::default()
    };
    let mut pending_toggles = Vec::new();
    let ctx = egui::Context::default();
    let output = run_ui_test(&ctx, raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            // Mirror the production render path in
            // `src/ui/panels/center.rs:200-209`: response is rendered
            // inside a vertical ScrollArea.
            egui::ScrollArea::vertical()
                .id_salt("test_inline_wrap_scroll")
                .show(ui, |ui| {
                    render_inline(ui, &InlineRenderItem::simple(&elems), &mut pending_toggles);
                });
        });
    });

    // Locate the text shape for our long text.
    let shape = output
        .shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            Shape::Text(t) if t.galley.text().trim_end() == long_text.trim_end() => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected a Text shape whose galley carries the long text"));
    let galley = &shape.galley;

    // 1. Sanity: the text must actually have wrapped to multiple rows.
    //    If it didn't, the rest of the assertions are meaningless.
    assert!(
        galley.rows.len() > 1,
        "expected long text to wrap to multiple rows; got {} row(s) — \
         the wrap layout isn't engaging at viewport_width={viewport_width:.0}px",
        galley.rows.len(),
    );

    // 2. Every wrapped row must start at (or within sub-pixel
    //    tolerance of) the first row's x position. A positive offset
    //    on continuation rows means the row was centered, which is
    //    exactly the "left clip" regression.
    let first_row_x = galley.rows[0].rect().min.x;
    let tolerance = 1.0; // 1 px tolerance for sub-pixel rounding
    for (i, row) in galley.rows.iter().enumerate() {
        let offset = row.rect().min.x - first_row_x;
        assert!(
            offset.abs() <= tolerance,
            "row {i} starts at x={:.2} but first row starts at x={first_row_x:.2} \
             (offset={offset:.2}px) — wrapped continuation rows must be left-aligned \
             (offset=0), not centered. This is the 'text cut off on the left on \
             subsequent lines' regression. Ensure render_inline uses \
             Layout::left_to_right(Align::Min).with_main_wrap(true), not \
             ui.horizontal_wrapped (which defaults main_align to Center).",
            row.rect().min.x,
        );
    }
}

/// Regression: same "left clip on subsequent wrapped lines" symptom, but
/// exercised through the full `render_markdown` path (the production
/// entry point used by the agent response window). This catches the
/// bug at the level the user actually sees it, rather than at the
/// `render_inline` unit level. Pinned to the
/// `Layout::left_to_right(Align::Min).with_main_wrap(true)` fix in
/// `render_inline` — if anyone regresses that fix, this test fails
/// because the continuation row's `x` is no longer pinned to the first
/// row's `x`.
#[test]
fn test_render_markdown_wrapped_paragraph_left_aligned() {
    use eframe::epaint::Shape;

    // A long, plain paragraph that will wrap to multiple lines at a
    // narrow viewport. The final line is intentionally shorter than
    // the line width — that's exactly the case where
    // `horizontal_wrapped`'s default `main_align: Center` would shift
    // the text right.
    let md = "\
This is a long paragraph that will wrap to multiple lines at a \
narrow viewport width and the final line will be shorter than the \
line width which is the case that triggers the left alignment \
regression when the placer centers children along the main axis.
";
    assert!(
        md.split_whitespace().count() > 20,
        "test fixture should be long enough to force multiple wraps",
    );

    let viewport_width: f32 = 240.0;
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, 800.0),
        )),
        ..egui::RawInput::default()
    };
    let mut scroll_id = None;
    let mut pending_toggles = Vec::new();
    let ctx = egui::Context::default();
    let output = run_ui_test(&ctx, raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("test_md_wrap_scroll")
                .show(ui, |ui| {
                    render_markdown(
                        ui,
                        md,
                        &mut scroll_id,
                        &mut pending_toggles,
                        crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                        None,
                    );
                });
        });
    });

    // Find the text shape that carries the long paragraph.
    let shape = output
        .shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            Shape::Text(t) if t.galley.text().trim() == md.trim() => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected a Text shape whose galley carries the long paragraph"));
    let galley = &shape.galley;

    // 1. Sanity: the paragraph must actually have wrapped.
    assert!(
        galley.rows.len() > 1,
        "expected paragraph to wrap to multiple rows; got {} row(s) — \
         the wrap layout isn't engaging at viewport_width={viewport_width:.0}px",
        galley.rows.len(),
    );

    // 2. Every wrapped row must start at the first row's x position
    //    (within 1 px tolerance). The "left clip" regression shifts
    //    continuation rows right by the centered-placement offset.
    let first_row_x = galley.rows[0].rect().min.x;
    let tolerance = 1.0;
    for (i, row) in galley.rows.iter().enumerate() {
        let offset = row.rect().min.x - first_row_x;
        assert!(
            offset.abs() <= tolerance,
            "row {i} starts at x={:.2} but first row starts at x={first_row_x:.2} \
             (offset={offset:.2}px) — wrapped continuation rows must be left-aligned \
             (offset=0), not centered. This is the 'text cut off on the left on \
             subsequent lines' regression in the agent response window. Ensure \
             render_inline uses Layout::left_to_right(Align::Min).with_main_wrap(true), \
             not ui.horizontal_wrapped (which defaults main_align to Center).",
            row.rect().min.x,
        );
    }
}
