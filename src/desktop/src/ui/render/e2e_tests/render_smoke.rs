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
    let output = ctx.run_ui(raw, |ui| {
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
    let long_yaml_summary = "January 2005 invoice from Heise Zeitschriften Verlag for \
        Microsoft half-year archive CD-ROMs, shipped tax-free to Martin Kühne at \
        Microsoft in Redmond, WA, USA, for archive and product-evaluation purposes \
        under Microsoft product license terms.";
    let yaml_str = format!(
        "title: Heise Invoice for Microsoft Product — Tax-Free Export Delivery\n\
         summary: \"{long_yaml_summary}\"\n\
         tags: [invoice, receipt, technology, documents]\n\
         header-date: 2026-07-22T19:32:47Z\n"
    );
    let yaml: serde_yml::Value = serde_yml::from_str(&yaml_str).unwrap();

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
            "Heise Zeitschriften Verlag",
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
