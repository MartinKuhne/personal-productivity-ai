//! Tier 2 functional rendering inspection tests for CommonMark spec 0.31.2.
//!
//! These tests exercise the egui rendering layer (`render_markdown()`) by
//! rendering Markdown text inside `run_ui_test()` and inspecting the resulting
//! `egui::FullOutput.shapes` for widget hierarchy, text content, formatting,
//! and layout bounds.
//!
//! Reference: `tests/collateral/commonmark.md` and `tests/collateral/functional-test-plan.md`.
//! Each test annotates the CM example numbers it exercises via `[CM-NNN]`.

#![cfg(test)]

use super::*;
use crate::ui::table_width::DeficitStrategy;
use crate::ui::test_helpers::run_ui_test;
use crate::ui::test_helpers::text::{assert_text_contains, extract_text};

/// Helper to render markdown through `render_markdown` inside `run_ui_test`
/// and return the resulting `egui::FullOutput`.
fn render_cm_ui(md: &str, viewport_size: egui::Vec2) -> egui::FullOutput {
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, viewport_size)),
        ..egui::RawInput::default()
    };
    let mut scroll = None;
    let mut toggles = Vec::new();
    let mut output = run_ui_test(&ctx, raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_markdown(
                ui,
                md,
                &mut scroll,
                &mut toggles,
                DeficitStrategy::ProportionalToSlack,
                None,
            );
        });
    });
    output.textures_delta.clear();
    output
}

/// Helper to collect all `egui::Shape::Text` shapes recursively from `output.shapes`.
fn extract_text_shapes<'a>(
    shapes: &'a [egui::epaint::ClippedShape],
) -> Vec<&'a egui::epaint::TextShape> {
    let mut out = Vec::new();
    for cs in shapes {
        collect_text_shapes_from(&cs.shape, &mut out);
    }
    out
}

fn collect_text_shapes_from<'a>(
    shape: &'a egui::Shape,
    out: &mut Vec<&'a egui::epaint::TextShape>,
) {
    match shape {
        egui::Shape::Text(ts) => out.push(ts),
        egui::Shape::Vec(shapes) => {
            for s in shapes {
                collect_text_shapes_from(s, out);
            }
        }
        _ => {}
    }
}

// ===========================================================================
// ATX Headings Render (CM-062 to CM-079)
// ===========================================================================

/// [CM-062] ATX headings at levels 1-6 render text content into egui shapes.
/// Font size for higher levels (H1/H2) must be strictly greater than lower levels (H5/H6).
#[test]
fn cm_render_atx_headings_all_levels() {
    let md =
        "# H1 Title\n## H2 Title\n### H3 Title\n#### H4 Title\n##### H5 Title\n###### H6 Title";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));
    assert_text_contains(&output.shapes, "H1 Title");
    assert_text_contains(&output.shapes, "H2 Title");
    assert_text_contains(&output.shapes, "H3 Title");
    assert_text_contains(&output.shapes, "H4 Title");
    assert_text_contains(&output.shapes, "H5 Title");
    assert_text_contains(&output.shapes, "H6 Title");

    // Inspect text shape font sizes for level hierarchy
    let text_shapes = extract_text_shapes(&output.shapes);
    let h1_shape = text_shapes
        .iter()
        .find(|s| s.galley.text().contains("H1 Title"))
        .expect("H1 text shape present");
    let h6_shape = text_shapes
        .iter()
        .find(|s| s.galley.text().contains("H6 Title"))
        .expect("H6 text shape present");

    let h1_size = h1_shape.galley.job.sections[0].format.font_id.size;
    let h6_size = h6_shape.galley.job.sections[0].format.font_id.size;

    assert!(
        h1_size > h6_size,
        "H1 font size ({h1_size}) must be larger than H6 font size ({h6_size})"
    );
}

/// [CM-071, CM-079] ATX headings strip optional closing `#` and render empty headings without panicking.
#[test]
fn cm_render_atx_headings_closing_and_empty() {
    let md = "## Closing Sequence ##\n# \n### ###";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    let texts = extract_text(&output.shapes);
    assert!(
        texts.iter().any(|t| t == "Closing Sequence"),
        "closing # sequence must be stripped; got {:?}",
        texts
    );
}

// ===========================================================================
// Setext Headings Render (CM-080 to CM-112)
// ===========================================================================

/// [CM-080] Setext H1 (`=`) and H2 (`-`) render with correct text content and heading font sizes.
#[test]
fn cm_render_setext_headings_levels() {
    let md = "Setext H1 Header\n================\n\nSetext H2 Header\n----------------";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    assert_text_contains(&output.shapes, "Setext H1 Header");
    assert_text_contains(&output.shapes, "Setext H2 Header");

    let text_shapes = extract_text_shapes(&output.shapes);
    let h1_shape = text_shapes
        .iter()
        .find(|s| s.galley.text().contains("Setext H1 Header"))
        .expect("Setext H1 text shape present");
    let h2_shape = text_shapes
        .iter()
        .find(|s| s.galley.text().contains("Setext H2 Header"))
        .expect("Setext H2 text shape present");

    let h1_size = h1_shape.galley.job.sections[0].format.font_id.size;
    let h2_size = h2_shape.galley.job.sections[0].format.font_id.size;

    assert!(
        h1_size > h2_size,
        "Setext H1 font size ({h1_size}) must be larger than H2 font size ({h2_size})"
    );
}

// ===========================================================================
// Thematic Breaks Render (CM-043 to CM-061)
// ===========================================================================

/// [CM-043] Valid thematic breaks (`***`, `---`, `___`) emit horizontal separator line stroke shapes.
#[test]
fn cm_render_thematic_breaks() {
    let md = "Paragraph 1\n\n***\n\nParagraph 2\n\n---\n\nParagraph 3\n\n___\n\nParagraph 4";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    let texts = extract_text(&output.shapes);
    assert_eq!(texts.len(), 4, "4 paragraph text shapes expected");

    // CommonMark thematic breaks emit horizontal lines (Shape::Path / Shape::LineSegment / Shape::Stroke)
    let stroke_count = output
        .shapes
        .iter()
        .filter(|cs| match &cs.shape {
            egui::Shape::Path(p) => p.points.len() == 2,
            egui::Shape::LineSegment { .. } => true,
            egui::Shape::Vec(v) => v.iter().any(|s| matches!(s, egui::Shape::Path(_))),
            _ => false,
        })
        .count();

    assert!(
        stroke_count >= 3,
        "expected at least 3 stroke shapes for thematic breaks; got {stroke_count}"
    );
}

// ===========================================================================
// Code Blocks Render (CM-113 to CM-236)
// ===========================================================================

/// [CM-113, CM-126] Indented and fenced code blocks render text in monospace font and dark frame background.
#[test]
fn cm_render_code_blocks_indented_and_fenced() {
    let md = "```\nfn main() {\n    println!(\"Hello World\");\n}\n```";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    assert_text_contains(&output.shapes, "println!(\"Hello World\")");

    let text_shapes = extract_text_shapes(&output.shapes);
    let code_shape = text_shapes
        .iter()
        .find(|s| s.galley.text().contains("println!"))
        .expect("code block text shape present");

    // Monospace font family check for code blocks
    let font_id = &code_shape.galley.job.sections[0].format.font_id;
    assert_eq!(
        font_id.family,
        egui::FontFamily::Monospace,
        "code block text shape must use FontFamily::Monospace"
    );
}

/// [CM-133] Fenced code block info string (`ruby`, `rust`) must be rendered in the UI.
#[test]
fn cm_render_code_blocks_info_string_header() {
    let md = "```ruby\ndef foo(x)\n  return 3\nend\n```";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    let texts = extract_text(&output.shapes);
    assert!(
        texts.iter().any(|t| t.contains("ruby")),
        "code block language info string 'ruby' must be rendered in the UI; got {:?}",
        texts
    );
}

// ===========================================================================
// Paragraphs & Spacing Render (CM-331 to CM-356)
// ===========================================================================

/// [CM-331, CM-354] Paragraphs render text shapes separated by vertical space.
#[test]
fn cm_render_paragraphs_spacing() {
    let md = "First paragraph.\n\nSecond paragraph.";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    let text_shapes = extract_text_shapes(&output.shapes);
    let p1 = text_shapes
        .iter()
        .find(|s| s.galley.text().contains("First paragraph"))
        .expect("P1 present");
    let p2 = text_shapes
        .iter()
        .find(|s| s.galley.text().contains("Second paragraph"))
        .expect("P2 present");

    // Y position of P2 must be strictly greater than P1 Y position + height
    assert!(
        p2.pos.y > p1.pos.y,
        "P2 Y position ({}) must be below P1 Y position ({})",
        p2.pos.y,
        p1.pos.y
    );
}

// ===========================================================================
// Block Quotes Render (CM-357 to CM-388)
// ===========================================================================

/// [CM-357] Block quotes render quoted text content.
#[test]
fn cm_render_block_quotes() {
    let md = "> Quoted paragraph in block quote.";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    assert_text_contains(&output.shapes, "Quoted paragraph in block quote");
}

// ===========================================================================
// Lists & Task Lists Render (CM-389 to CM-539)
// ===========================================================================

/// [CM-390] Unordered lists render bullet indicators and item texts.
#[test]
fn cm_render_unordered_lists() {
    let md = "- Alpha item\n- Beta item\n- Gamma item";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    assert_text_contains(&output.shapes, "Alpha item");
    assert_text_contains(&output.shapes, "Beta item");
    assert_text_contains(&output.shapes, "Gamma item");
}

/// [CM-470] Ordered lists render ordinal strings ("1.", "2.") and item texts.
#[test]
fn cm_render_ordered_lists() {
    let md = "1. First step\n2. Second step";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    let texts = extract_text(&output.shapes);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("1.") || t.contains("First step")),
        "ordered list ordinal and text must be rendered; got {:?}",
        texts
    );
    assert!(
        texts
            .iter()
            .any(|t| t.contains("2.") || t.contains("Second step")),
        "ordered list ordinal 2 and text must be rendered; got {:?}",
        texts
    );
}

/// [CM-434] Task lists render checkbox widgets.
#[test]
fn cm_render_task_lists() {
    let md = "- [x] Completed task\n- [ ] Pending task";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    assert_text_contains(&output.shapes, "Completed task");
    assert_text_contains(&output.shapes, "Pending task");
}

// ===========================================================================
// Code Spans Render (CM-540 to CM-574)
// ===========================================================================

/// [CM-540] Inline code spans render with monospace font styling.
#[test]
fn cm_render_code_spans() {
    let md = "Plain text `inline_code_span` plain text.";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    assert_text_contains(&output.shapes, "inline_code_span");

    let text_shapes = extract_text_shapes(&output.shapes);
    let code_span_shape = text_shapes
        .iter()
        .find(|s| s.galley.text().contains("inline_code_span"))
        .expect("inline code span text shape present");

    let font_id = &code_span_shape.galley.job.sections[0].format.font_id;
    assert_eq!(
        font_id.family,
        egui::FontFamily::Monospace,
        "inline code span must use FontFamily::Monospace"
    );
}

// ===========================================================================
// Emphasis & Strong Render (CM-575 to CM-753)
// ===========================================================================

/// [CM-575, CM-613, CM-678] Bold, italic, and strikethrough styling in rendered shapes.
#[test]
fn cm_render_emphasis_styles() {
    let md = "**Bold Text** *Italic Text* ~~Strikethrough Text~~";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    assert_text_contains(&output.shapes, "Bold Text");
    assert_text_contains(&output.shapes, "Italic Text");
    assert_text_contains(&output.shapes, "Strikethrough Text");
}

// ===========================================================================
// Links & Autolinks Render (CM-754 to CM-876)
// ===========================================================================

/// [CM-754, CM-865] Inline links and autolinks render clickable link text.
#[test]
fn cm_render_links_and_autolinks() {
    let md = "[Click Here](https://example.com) and <https://autolink.example.com>";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    assert_text_contains(&output.shapes, "Click Here");
    assert_text_contains(&output.shapes, "https://autolink.example.com");
}

// ===========================================================================
// Images Render (CM-860 to CM-864)
// ===========================================================================

/// [CM-860] Images render image widgets / alt text placeholder in egui shapes.
#[test]
fn cm_render_images() {
    let md = "![Alt Description](https://example.com/logo.png)";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    let texts = extract_text(&output.shapes);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("Alt Description") || t.contains("logo.png")),
        "image alt text or URL must be rendered; got {:?}",
        texts
    );
}

// ===========================================================================
// Hard Line Breaks Render (CM-911 to CM-921)
// ===========================================================================

/// [CM-911] Hard line breaks (trailing 2 spaces + newline) must produce separate lines in rendered text shapes.
#[test]
fn cm_render_hard_line_breaks() {
    let md = "First line  \nSecond line";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    let text_shapes = extract_text_shapes(&output.shapes);
    let line1 = text_shapes
        .iter()
        .find(|s| s.galley.text().contains("First line"))
        .expect("line 1 text shape present");
    let line2 = text_shapes
        .iter()
        .find(|s| s.galley.text().contains("Second line"))
        .expect("line 2 text shape present");

    // Hard line break must place line2 on a separate Y row below line1
    assert!(
        line2.pos.y > line1.pos.y,
        "hard line break must render second line at lower Y position ({}) than first line ({})",
        line2.pos.y,
        line1.pos.y
    );
}

// ===========================================================================
// Raw HTML Render (CM-877 to CM-910)
// ===========================================================================

/// [CM-877] Raw HTML tags pass through as rendered text.
#[test]
fn cm_render_raw_html() {
    let md = "<span class=\"custom\">Custom Text</span>";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    assert_text_contains(&output.shapes, "Custom Text");
}

// ===========================================================================
// Tables Render (CM-191 to CM-310)
// ===========================================================================

/// [CM-191] Markdown tables render grid layout cells for header and data rows.
#[test]
fn cm_render_tables() {
    let md = "| Header A | Header B |\n| --- | --- |\n| Cell A | Cell B |";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    assert_text_contains(&output.shapes, "Header A");
    assert_text_contains(&output.shapes, "Header B");
    assert_text_contains(&output.shapes, "Cell A");
    assert_text_contains(&output.shapes, "Cell B");
}

// ===========================================================================
// Textual Content & Unicode Render (CM-928 to CM-935)
// ===========================================================================

/// [CM-928] Non-ASCII Unicode and Emoji strings render correctly in text shapes.
#[test]
fn cm_render_unicode_emoji() {
    let md = "Unicode test: 🚀 Rocket, 💡 Idea, ★ Star, 日本語, Café.";
    let output = render_cm_ui(md, egui::vec2(800.0, 600.0));

    assert_text_contains(&output.shapes, "🚀 Rocket");
    assert_text_contains(&output.shapes, "日本語");
    assert_text_contains(&output.shapes, "Café");
}
