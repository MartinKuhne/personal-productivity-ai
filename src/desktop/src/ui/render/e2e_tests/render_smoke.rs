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

#[test]
fn test_render_markdown_e2e() {
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut scroll_id = None;
            render_markdown(
                ui,
                "# Test\n\n- [ ] Task\n\n```rust\nlet x = 1;\n```",
                &mut scroll_id,
                &mut Vec::new(),
                crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
            );

            let yaml_str = "a: 1\nb: 2";
            let val: serde_yml::Value = serde_yml::from_str(yaml_str).unwrap();
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

            // Render non-mapping YAML table
            let non_map = serde_yml::Value::String("test".to_string());
            render_yaml_table(ui, &non_map);
        });
    });
}

#[test]
fn test_render_table_with_empty_cells_e2e() {
    let ctx = egui::Context::default();
    let md = "| A | | C |\n|---|---|---|\n| | B | |";
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

/// Regression: a long YAML value must word-wrap inside the YAML
/// metadata table rather than overflow the panel's content rect
/// and get clipped by the inner horizontal `ScrollArea`.
///
/// Before the fix, `render_yaml_table` rendered both columns with
/// `ui.label(...)` inside an unconstrained `Grid`, so the value
/// column expanded to the natural width of the longest text. The
/// value text therefore ran off the right edge of the viewport
/// and the user saw the value truncated mid-word (e.g.
/// "Microsoft in Re…").
///
/// The test pins the symptom by:
/// 1. Rendering the YAML table in a deliberately narrow 320px
///    viewport so a long value cannot fit on a single line.
/// 2. Locating the `Shape::Text` that carries the long summary
///    text (uniquely identified by its leading "Heise Invoice"
///    substring).
/// 3. Asserting the underlying `Galley` has more than one row
///    (the text wrapped) and that the rendered rect's width fits
///    within the available content area (no horizontal overflow).
#[test]
fn test_render_yaml_table_wraps_long_values_within_viewport() {
    use crate::ui::test_helpers::text::extract_text;

    let ctx = egui::Context::default();
    let viewport_width: f32 = 320.0;
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, 800.0),
        )),
        ..egui::RawInput::default()
    };
    // The exact string the user reported in the screenshot.
    let long_summary = "January 2005 invoice from Heise Zeitschriften Verlag for \
        Microsoft half-year archive CD-ROMs, shipped tax-free to Martin Kühne at \
        Microsoft in Redmond, WA, USA, for archive and product-evaluation purposes \
        under Microsoft product license terms.";
    let yaml_str = format!(
        "title: Heise Invoice for Microsoft Product — Tax-Free Export Delivery\n\
         summary: \"{long_summary}\"\n\
         tags: [invoice, receipt, technology, documents]\n\
         header-date: 2026-07-22T19:32:47Z\n"
    );
    let yaml: serde_yml::Value = serde_yml::from_str(&yaml_str).unwrap();

    let output = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_yaml_table(ui, &yaml);
        });
    });

    // Sanity: the long value must actually have been rendered.
    let texts = extract_text(&output.shapes);
    let needle = "Heise Zeitschriften Verlag";
    assert!(
        texts.iter().any(|t| t.contains(needle)),
        "expected the long summary to be rendered; got {} text shape(s)",
        texts.len()
    );

    // Locate the `Shape::Text` whose galley carries the long
    // summary (matched by an unambiguous substring that cannot
    // appear in any other YAML key in the fixture).
    let summary_shape = output.shapes.iter().find_map(|cs| match &cs.shape {
        egui::Shape::Text(t) if t.galley.text().contains(needle) => Some(t),
        _ => None,
    });
    let shape = summary_shape.expect("expected a Text shape for the long summary");
    let galley = &shape.galley;
    let rendered_width = galley.rect.width();

    // 1. The long value must wrap: more than one row in the galley.
    assert!(
        galley.rows.len() > 1,
        "expected the long summary to word-wrap; got galley with {} row(s) and rect width={:.1}px (viewport={viewport_width:.0}px)",
        galley.rows.len(),
        rendered_width,
    );

    // 2. The wrapped text must fit inside the viewport — the
    //    rect width should never exceed the viewport. Use a small
    //    tolerance for the CentralPanel's outer margins.
    let max_allowed = viewport_width;
    assert!(
        rendered_width <= max_allowed + 1.0,
        "expected wrapped text width <= {max_allowed:.0}px; got {rendered_width:.1}px \
         (the value is overflowing the panel — the horizontal ScrollArea is clipping it)",
    );
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
    let yaml: serde_yml::Value = serde_yml::from_str(&yaml_str).unwrap();

    let output = ctx.run_ui(raw, |ui| {
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

    let output = ctx.run_ui(raw, |ui| {
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

/// Regression: long body paragraphs must word-wrap inside the preview.
///
/// `render_inline_inner` renders each `InlineElem::Text` via
/// `ui.add(egui::Label::new(rt).wrap())`. In egui 0.35, `Label::new`
/// defaults `wrap_mode` to `None` and only wraps if the parent
/// layout is vertical or horizontal+main_wrap AND the available
/// width is finite — a fragile contract that already broke for
/// `render_yaml_table` (see
/// `test_render_yaml_table_wraps_long_values_within_viewport`),
/// `render_code_block`, and `render_table_cell`, each of which
/// had to be patched with an explicit `.wrap()`. This test pins
/// the same invariant for the paragraph path so a future
/// refactor (e.g. swapping the `horizontal_wrapped` parent for a
/// `Grid` or removing the explicit `.wrap()`) cannot silently
/// regress long-paragraph wrapping.
///
/// Mirrors the shape of
/// `test_render_yaml_table_wraps_long_values_within_viewport` above:
/// render in a deliberately narrow 320px viewport, locate the
/// `Shape::Text` that carries the long paragraph (matched by an
/// unambiguous substring that cannot appear in the table or header
/// text), and assert the underlying `Galley` wraps to multiple
/// rows and stays within the viewport.
#[test]
fn test_render_markdown_long_paragraph_wraps_in_preview() {
    use crate::ui::test_helpers::text::extract_text;

    let ctx = egui::Context::default();
    let viewport_width: f32 = 320.0;
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, 800.0),
        )),
        ..egui::RawInput::default()
    };
    // Same shape as the user's `Mythical man-month.md` body: one
    // long German sentence with normal whitespace, ~570 chars.
    let long_paragraph = "Es ist ein Mix an Methoden im Einsatz. \
        Einerseits soll im traditionellen Projektmanagement in Voraus \
        der Funktionsumfang und die Projektdauer feststehen. Die zur \
        Planung notwendige Dokumentation der Anforderungen, Technologien \
        und Risken findet aber nicht statt. Daraufhin trägt das \
        ausführende Team ein erhebliches Risiko, wenn sich die \
        Anforderungen ändern oder die Arbeit komplexer ist als erwartet.";
    let md = format!("\n{long_paragraph}\n");

    let output = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut scroll_id = None;
            render_markdown(
                ui,
                &md,
                &mut scroll_id,
                &mut Vec::new(),
                crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
            );
        });
    });

    // Sanity: the long paragraph must have been rendered.
    let texts = extract_text(&output.shapes);
    let needle = "erhebliches Risiko, wenn sich";
    assert!(
        texts.iter().any(|t| t.contains(needle)),
        "expected the long paragraph to be rendered; got {} text shape(s): {:?}",
        texts.len(),
        texts,
    );

    // Locate the Text shape whose galley carries the long paragraph.
    let paragraph_shape = output.shapes.iter().find_map(|cs| match &cs.shape {
        egui::Shape::Text(t) if t.galley.text().contains(needle) => Some(t),
        _ => None,
    });
    let shape = paragraph_shape.expect("expected a Text shape for the long paragraph");
    let galley = &shape.galley;

    // 1. The paragraph must wrap: more than one row.
    assert!(
        galley.rows.len() > 1,
        "expected the long paragraph to word-wrap; got galley with {} row(s) \
         and rect width={:.1}px (viewport={viewport_width:.0}px) — \
         the text is overflowing instead of wrapping",
        galley.rows.len(),
        galley.rect.width(),
    );

    // 2. The wrapped text must fit inside the viewport.
    let max_allowed = viewport_width;
    assert!(
        galley.rect.width() <= max_allowed + 1.0,
        "expected wrapped paragraph width <= {max_allowed:.0}px; got {:.1}px \
         (the paragraph is overflowing the panel — the preview will \
         horizontal-scroll the long line instead of wrapping it)",
        galley.rect.width(),
    );
}

#[test]
fn test_render_table_with_bold_and_special_chars_e2e() {
    let ctx = egui::Context::default();
    let md = "| Name | Account | Amount | Type |\n|---|---|---|---|\n| **Vanguard** | #12345678 | $1 | Taxable (investment) |";
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
