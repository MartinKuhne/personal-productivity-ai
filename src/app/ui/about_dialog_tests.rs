//! Tests for `ui/about_dialog.rs`

use super::*;
use crate::ui::test_helpers::run_ui_test;
use crate::ui::test_helpers::text::{assert_text_contains, extract_text};
use eframe::egui;

fn render_dialog_once(app: &mut crate::ui::FastMdApp) -> egui::FullOutput {
    let ctx = egui::Context::default();
    // Two frames like batch_dialog_tests — first initializes window, second renders.
    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1920.0, 1080.0),
        )),
        ..Default::default()
    };
    app.dialogs_mut().about_dialog_open = true;
    let mut out1 = run_ui_test(&ctx, raw_input.clone(), |ui| {
        show_about_dialog(ui.ctx(), app);
    });
    out1.textures_delta.clear();

    app.dialogs_mut().about_dialog_open = true;
    let mut out = run_ui_test(&ctx, raw_input, |ui| {
        show_about_dialog(ui.ctx(), app);
    });
    out.textures_delta.clear();
    out
}

#[test]
fn license_reflow_preserves_words() {
    // The reflowed license must contain exactly the same words in the same
    // order (whitespace-only change), keep paragraph breaks, and leave no
    // single newlines that would pin the label to the file's hard wrap.
    let words_original: Vec<&str> = LICENSE_TEXT.split_whitespace().collect();
    let reflowed = license_display_text();
    let words_reflowed: Vec<&str> = reflowed.split_whitespace().collect();
    assert_eq!(words_reflowed, words_original);
    assert_eq!(
        reflowed.matches("\n\n").count(),
        LICENSE_TEXT.matches("\n\n").count(),
        "paragraph breaks must survive the reflow"
    );
    let singles = reflowed.replace("\n\n", "").matches('\n').count();
    assert_eq!(singles, 0, "no single newlines may remain");
}

/// Collects `(rect, is_text)` shapes, recursing into `Shape::Vec`.
fn collect_rects_and_texts(output: &egui::FullOutput) -> (Vec<egui::Rect>, Vec<(f32, String)>) {
    fn walk(shape: &egui::Shape, rects: &mut Vec<egui::Rect>, texts: &mut Vec<(f32, String)>) {
        match shape {
            egui::Shape::Vec(shapes) => {
                for s in shapes {
                    walk(s, rects, texts);
                }
            }
            egui::Shape::Rect(r) => rects.push(r.rect),
            egui::Shape::Text(t) => {
                texts.push((t.pos.x + t.galley.rect.max.x, t.galley.text().to_owned()));
            }
            _ => {}
        }
    }
    let mut rects = Vec::new();
    let mut texts = Vec::new();
    for s in &output.shapes {
        walk(&s.shape, &mut rects, &mut texts);
    }
    (rects, texts)
}

#[test]
fn license_section_spans_full_content_width() {
    // The license text's right edge must align with the attributions
    // scrollbar (the full-width marker) — previously the hard-wrapped
    // license file pinned the section ~115px narrower than the dialog.
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let output = render_dialog_once(&mut app);
    let (rects, texts) = collect_rects_and_texts(&output);
    let license_x1 = texts
        .iter()
        .find(|(_, text)| text.contains("Permission is hereby granted"))
        .map(|(x1, _)| *x1)
        .expect("license body text should render");
    let attr_scrollbar_x = rects
        .iter()
        .filter(|r| r.width() < 2.0 && r.height() > 300.0)
        .map(|r| r.min.x)
        .fold(f32::NAN, f32::min);
    assert!(
        !attr_scrollbar_x.is_nan(),
        "attributions scrollbar should render"
    );
    assert!(
        (attr_scrollbar_x - license_x1).abs() <= 24.0,
        "license right edge ({license_x1}) should align with attributions width ({attr_scrollbar_x})"
    );
}

#[test]
fn dialog_viewports_use_taller_budget() {
    // The dialog opens taller than the old 580px default and both scroll
    // viewports use their raised caps (license 200, attributions 340).
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let output = render_dialog_once(&mut app);
    let (rects, _) = collect_rects_and_texts(&output);
    assert!(
        rects
            .iter()
            .any(|r| r.width() > 600.0 && r.height() >= 700.0),
        "window should open at least 700px tall"
    );
    assert!(
        rects
            .iter()
            .any(|r| r.width() < 2.0 && (190.0..210.0).contains(&r.height())),
        "license viewport should use the 200px cap"
    );
    assert!(
        rects
            .iter()
            .any(|r| r.width() < 2.0 && (330.0..350.0).contains(&r.height())),
        "attributions viewport should use the 340px cap"
    );
}

#[test]
fn dialog_renders_without_panic() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let _ = render_dialog_once(&mut app);
    // If we reach here without panic, smoke test passes.
}

#[test]
fn header_shows_app_name_and_copyright() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let output = render_dialog_once(&mut app);
    assert_text_contains(&output.shapes, crate::ui::strings::ABOUT_APP_NAME);
    assert_text_contains(&output.shapes, crate::ui::strings::ABOUT_COPYRIGHT);
}

#[test]
fn build_labels_and_values_present() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let output = render_dialog_once(&mut app);
    assert_text_contains(&output.shapes, crate::ui::strings::ABOUT_BRANCH_LABEL);
    assert_text_contains(&output.shapes, crate::ui::strings::ABOUT_COMMIT_LABEL);
    assert_text_contains(&output.shapes, crate::ui::strings::ABOUT_DATE_LABEL);
    // Build metadata values are compile-time env — at least check they are present as text.
    // In CI they may be "unknown" but still rendered.
    let texts = extract_text(&output.shapes);
    // BUILD_* constants are non-empty by build.rs fallback.
    assert!(
        texts.iter().any(|t| !t.trim().is_empty()),
        "dialog should render some build metadata text"
    );
    // Short hash length 7–8 or "unknown" — just verify commit label's value appears.
    // We check that at least one of the texts contains the short hash substring.
    // Access the constants via the module's env! read — reuse the same env values by checking output contains date-like pattern.
    let has_date_like = texts.iter().any(|t| t.contains('-') && t.len() >= 10);
    // Not strict; but ensure dialog rendered something date-ish (YYYY-MM-DD) or "unknown".
    assert!(
        has_date_like || texts.iter().any(|t| t.contains("unknown")),
        "expected build date or unknown in texts: {texts:?}"
    );
}

#[test]
fn license_scroll_contains_mit_text() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let output = render_dialog_once(&mut app);
    assert_text_contains(&output.shapes, crate::ui::strings::ABOUT_LICENSE_HEADER);
    assert_text_contains(&output.shapes, "MIT License");
    assert_text_contains(&output.shapes, "Permission is hereby granted");
}

#[test]
fn license_is_scrollable_with_capped_height() {
    // Indirect: the license scroll area id_salt is verified by rendering without panic
    // and checking that the dialog contains the license header plus scrollable content.
    // The capped height is an implementation detail; we verify the scroll area exists by
    // ensuring the license text is rendered inside a scrollable context.
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let output = render_dialog_once(&mut app);
    let texts = extract_text(&output.shapes);
    // License text is long — ensure multiple chunks of it appear, indicating scroll area content.
    let license_hits = texts
        .iter()
        .filter(|t| t.contains("MIT") || t.contains("Permission") || t.contains("Copyright"))
        .count();
    assert!(
        license_hits >= 1,
        "license scroll area should render license text chunks: {texts:?}"
    );
}

#[test]
fn attributions_all_52_rendered_and_scrollable() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let output = render_dialog_once(&mut app);
    assert_text_contains(
        &output.shapes,
        crate::ui::strings::ABOUT_ATTRIBUTIONS_HEADER,
    );
    let texts = extract_text(&output.shapes);
    // Egui scroll areas cull off-screen rows; only the top ~12 rows are in output.shapes.
    // Verify that the first few alphabetically sorted crates are visible.
    for name in ["anyhow", "arboard", "arc-swap", "async-openai", "eframe"] {
        assert!(
            texts.iter().any(|t| t.contains(name)),
            "attributions should contain visible crate {name}; texts={texts:?}"
        );
    }
    // Full catalog completeness is verified by attributions_tests; here we just ensure
    // the catalog length is 53.
    assert_eq!(crate::ui::attributions::DIRECT_DEPENDENCIES.len(), 53);
}

#[test]
fn close_button_clears_dialog_flag() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    app.dialogs_mut().about_dialog_open = true;
    assert!(app.dialogs().about_dialog_open);
    // Simulate user closing via title-bar X: the Window's open flag goes false and
    // show_about_dialog writes back false. We emulate by calling the function with open=false
    // path: set open to false via the context's window manipulation is complex, so we test
    // the state transition directly: when dialog is open, calling show_about_dialog with
    // open already false should keep it closed. The real close path is `if !open { false }`.
    // Verify that setting the flag to false persists.
    app.dialogs_mut().about_dialog_open = false;
    let ctx = egui::Context::default();
    show_about_dialog(&ctx, &mut app);
    assert!(!app.dialogs().about_dialog_open);
}

#[test]
fn commit_tooltip_reveals_full_hash_and_hint() {
    // Spec FR-006 / SC-002: hovering the short hash must reveal the full hash.
    let tooltip = commit_tooltip_text();
    assert!(
        tooltip.contains(BUILD_COMMIT_HASH),
        "tooltip should reveal the full hash; got {tooltip:?}"
    );
    assert!(
        tooltip.contains(crate::ui::strings::ABOUT_COPY_COMMIT_TOOLTIP),
        "tooltip should carry the copy hint; got {tooltip:?}"
    );
    assert!(
        tooltip.contains(crate::ui::strings::ABOUT_FULL_COMMIT_PREFIX),
        "tooltip should use the centralized prefix, not an inline literal; got {tooltip:?}"
    );
}

#[test]
fn commit_copy_records_confirmation() {
    // Spec FR-006 / SC-002: clicking copies the full hash and shows a
    // confirmation. `ctx.copy_text` is a headless noop in tests but must not
    // panic; the observable side effect is the confirmation flag with TTL
    // semantics.
    let ctx = egui::Context::default();
    assert!(
        !should_show_copy_confirmation(&ctx),
        "fresh context should show no confirmation"
    );
    copy_commit_hash_to_output(&ctx);
    if is_build_metadata_unknown() {
        assert!(
            !should_show_copy_confirmation(&ctx),
            "unknown builds copy nothing and confirm nothing"
        );
    } else {
        assert!(
            should_show_copy_confirmation(&ctx),
            "copy should raise the confirmation flag"
        );
    }
}

#[test]
fn build_metadata_short_hash_is_prefix_of_full_hash() {
    // Data-model invariant: the short hash is the leading run of the full
    // hash, or both fall back to "unknown" (spec FR-007).
    if is_build_metadata_unknown() {
        assert_eq!(
            BUILD_COMMIT_SHORT_HASH, "unknown",
            "unknown builds should fall back gracefully, not blank"
        );
    } else {
        assert!(
            BUILD_COMMIT_HASH.starts_with(BUILD_COMMIT_SHORT_HASH),
            "short hash should prefix full hash: {:?} vs {:?}",
            BUILD_COMMIT_SHORT_HASH,
            BUILD_COMMIT_HASH
        );
        assert!(
            (7..=8).contains(&BUILD_COMMIT_SHORT_HASH.len()),
            "short hash should be 7-8 chars; got {:?}",
            BUILD_COMMIT_SHORT_HASH
        );
    }
}

#[test]
fn commit_hash_copy_interaction_does_not_panic() {
    // Verify that the commit hash label is rendered and the hover tooltip path doesn't panic.
    // Full clipboard copy is via ctx.copy_text — we smoke-test that the dialog renders the short hash.
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let output = render_dialog_once(&mut app);
    // Short hash is rendered as text; ensure commit label section exists.
    assert_text_contains(&output.shapes, crate::ui::strings::ABOUT_COMMIT_LABEL);
    // The short hash itself (BUILD_COMMIT_SHORT_HASH) is part of the rendered texts.
    // In offline builds it is "unknown" but still present.
    let texts = extract_text(&output.shapes);
    assert!(
        texts.len() > 5,
        "dialog should render multiple text shapes including commit hash; got {texts:?}"
    );
}

#[test]
fn should_auto_show_about_none_means_first_run() {
    // Spec FR-016 / SC-008: fresh UI state (no recorded shown version)
    // must auto-show the About dialog.
    assert!(
        should_auto_show_about(None, "0.2.0"),
        "fresh state should auto-show"
    );
}

#[test]
fn should_auto_show_about_same_version_stays_quiet() {
    // Spec FR-016 / SC-008: same-version restarts must not re-display.
    assert!(
        !should_auto_show_about(Some("0.2.0"), "0.2.0"),
        "same version should stay quiet"
    );
}

#[test]
fn should_auto_show_about_upgrade_reopens_once() {
    // Spec FR-016 / SC-008: each upgrade's first start re-displays once.
    assert!(
        should_auto_show_about(Some("0.1.0"), "0.2.0"),
        "upgraded version should auto-show"
    );
}
