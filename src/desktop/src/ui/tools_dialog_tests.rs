//! Tier 1 render tests for the Tools dialog body. These drive the
//! dialog in a fresh `egui::Context` and assert that the table
//! headers and at least one known internal group row appear in
//! the rendered output.

use crate::ui::test_helpers::text::{assert_text_contains, extract_text};
use crate::ui::tools_dialog::{compute_dialog_size, render_contents, show_tools_dialog};
use eframe::egui;

fn render_dialog_once(app: &mut crate::ui::FastMdApp) -> egui::FullOutput {
    let ctx = egui::Context::default();
    ctx.run_ui(egui::RawInput::default(), |ui| {
        render_contents(ui, app);
    })
}

fn show_dialog_once(app: &mut crate::ui::FastMdApp) -> egui::FullOutput {
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        )),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    ctx.run_ui(raw, |ctx| {
        show_tools_dialog(ctx, app);
    })
}

#[test]
fn test_tools_dialog_renders_table_headers() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let output = render_dialog_once(&mut app);
    // The six visible column headers.
    assert_text_contains(&output.shapes, "Group");
    assert_text_contains(&output.shapes, "Kind");
    assert_text_contains(&output.shapes, "Tools");
    assert_text_contains(&output.shapes, "Prompt");
    assert_text_contains(&output.shapes, "Actions");
}

#[test]
fn test_tools_dialog_renders_internal_groups() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let output = render_dialog_once(&mut app);
    // The seven internal groups are registered on every manager
    // startup; "Filesystem" should appear in the rendered output.
    assert_text_contains(&output.shapes, "Filesystem");
    assert_text_contains(&output.shapes, "Internal");
}

#[test]
fn test_tools_dialog_renders_char_count() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let output = render_dialog_once(&mut app);
    // Every row's char count is rendered as "N". We don't assert
    // on the exact value because it depends on tool descriptions
    // that may evolve.
    let texts = extract_text(&output.shapes);
    let any_digits = texts.iter().any(|t| t.chars().all(|c| c.is_ascii_digit()));
    assert!(
        any_digits,
        "dialog must render at least one numeric char count; got {} text shape(s): {:?}",
        texts.len(),
        texts
    );
}

/// `show_tools_dialog` must clear `tools_dialog_just_opened` on
/// the first frame so the (potentially expensive) MCP discovery
/// refresh runs exactly once per dialog open.
#[test]
fn test_show_tools_dialog_clears_just_opened_on_first_render() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    app.orchestrator.dialogs.tools_dialog_open = true;
    app.orchestrator.dialogs.tools_dialog_just_opened = true;
    let _ = show_dialog_once(&mut app);
    assert!(
        !app.orchestrator.dialogs.tools_dialog_just_opened,
        "show_tools_dialog must clear tools_dialog_just_opened on first frame"
    );
    // The second frame must not re-trigger MCP discovery.
    let _ = show_dialog_once(&mut app);
    assert!(!app.orchestrator.dialogs.tools_dialog_just_opened);
}

/// The dialog must NOT render a bottom Close button anymore —
/// the title-bar X is the only close affordance (UI-056).
#[test]
fn test_tools_dialog_does_not_render_close_button() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let output = render_dialog_once(&mut app);
    let texts = extract_text(&output.shapes);
    // The header row contains "Actions" but the bottom Close
    // button label would be a plain "Close" rendered as a
    // button. The title-bar X is a frame widget, not a text
    // shape, so it doesn't appear in `output.shapes`.
    let has_close_button = texts.iter().any(|t| t.trim() == "Close");
    assert!(
        !has_close_button,
        "Tools dialog must not render a bottom Close button; rely on the title-bar X instead"
    );
}

/// The dialog must NOT render the "✓ parallel" chip (per the
/// user's request to remove the parallel display for now).
/// `parallel_safe` is still tracked in `ToolGroupState` for
/// future use; only the visual chip is gone.
#[test]
fn test_tools_dialog_does_not_render_parallel_chip() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let output = render_dialog_once(&mut app);
    let texts = extract_text(&output.shapes);
    let has_parallel_chip = texts.iter().any(|t| t.contains("✓ parallel"));
    assert!(
        !has_parallel_chip,
        "Tools dialog must not render the ✓ parallel chip"
    );
}

/// When a group has a recorded error, the dialog must render a
/// "Restart" link (labelled per the user's request; it clears the
/// error and allows the group to retry).
#[test]
fn test_tools_dialog_renders_restart_button_on_error() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    // Populate the global manager's group state so `record_error`
    // has a row to attach the error to, then record it.
    crate::agent::tools::manager::groups_snapshot(app.config());
    let id = crate::agent::tools::manager::ToolGroupId::Internal(
        crate::agent::tools::manager::InternalToolGroup::Filesystem,
    );
    crate::agent::tools::manager::record_mcp_error(
        &id,
        crate::agent::tools::manager::ToolGroupError::now(
            crate::agent::tools::manager::ToolErrorKind::Execution,
            "boom".to_owned(),
        ),
    );
    let output = render_dialog_once(&mut app);
    let texts = extract_text(&output.shapes);
    let has_restart_button = texts.iter().any(|t| t.trim() == "Restart");
    assert!(
        has_restart_button,
        "Tools dialog must render a Restart button on error"
    );
}

/// `compute_dialog_size` should grow the window to fit the row
/// count when the screen has room, and cap it at the screen
/// height otherwise.
#[test]
fn test_compute_dialog_size_fits_few_rows_on_large_screen() {
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1920.0, 1080.0));
    // 7 internal groups; preferred ~ 56 + 20 + 7*24 + 8 = 252.
    let (default_size, min_size, max_height) = compute_dialog_size(viewport, 7);
    // The default should fit all 7 rows.
    assert!(
        default_size[1] >= 250.0,
        "default height must be large enough to fit 7 rows; got {}",
        default_size[1]
    );
    // Capped at screen height * 0.85 = 918; the hard cap (1200)
    // no longer truncates on a 1080-tall screen. So max_height
    // should be 918.
    assert!(
        (max_height - 918.0).abs() < 1.0,
        "max_height should be 918 on a 1080-tall screen; got {max_height}"
    );
    // min_size height <= default_size height.
    assert!(min_size[1] <= default_size[1]);
}

/// On a large screen the dialog must grow to fit many rows when
/// the screen has room, rather than truncating at the hard cap.
/// Regression test for: "the tools window does not show all the
/// content although there is room on the screen."
#[test]
fn test_compute_dialog_size_fits_many_rows_on_large_screen() {
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1920.0, 1080.0));
    // 30 rows; preferred = 56 + 20 + 30*24 + 8 = 804, which fits
    // within the 85% screen cap (918) on this viewport.
    let (default_size, _min_size, max_height) = compute_dialog_size(viewport, 30);
    assert!(
        default_size[1] >= 804.0,
        "default height must fit all 30 rows when the screen has room; got {}",
        default_size[1]
    );
    assert!(
        max_height >= default_size[1],
        "max_height must not truncate the preferred height; got {max_height} vs default {}",
        default_size[1]
    );
}

/// On a small screen, the dialog must cap the height at the
/// available space so it fits without going off-screen.
#[test]
fn test_compute_dialog_size_caps_on_small_screen() {
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 400.0));
    // 100 rows; preferred would be 2484, but the screen caps it.
    let (_default_size, _min_size, max_height) = compute_dialog_size(viewport, 100);
    // 400 * 0.85 = 340. The dialog caps at 340 even though 100
    // rows would prefer 2484.
    assert!(
        max_height < 400.0,
        "max_height must be < screen height; got {max_height}"
    );
    assert!(
        max_height > 200.0,
        "max_height must be a usable size; got {max_height}"
    );
}
