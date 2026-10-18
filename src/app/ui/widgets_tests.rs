//! Tests for `ui/widgets.rs`.

use super::*;
use crate::ui::test_helpers::assert::assert_no_id_change_in_shapes;
use crate::ui::test_helpers::run_ui_test;

/// A 1024x768 viewport so windows and menus have a non-degenerate rect to
/// lay out into (matches `modals_tests.rs::raw_input`).
fn raw_input() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1024.0, 768.0),
        )),
        ..egui::RawInput::default()
    }
}

/// Extract the painted shapes from a run output for the id-stability check.
fn shapes_of(output: &egui::FullOutput) -> Vec<egui::Shape> {
    output.shapes.iter().map(|cs| cs.shape.clone()).collect()
}

#[test]
fn test_muted_text_is_italic_gray() {
    assert_eq!(
        muted_text("no files"),
        RichText::new("no files")
            .italics()
            .color(egui::Color32::GRAY)
    );
}

#[test]
fn test_muted_text_allows_chained_size() {
    assert_eq!(
        muted_text("no file selected").size(15.0),
        RichText::new("no file selected")
            .italics()
            .color(egui::Color32::GRAY)
            .size(15.0)
    );
}

#[test]
fn test_right_aligned_row_returns_inner_value() {
    let ctx = egui::Context::default();
    let mut captured = None;
    let _ = run_ui_test(&ctx, raw_input(), |ui| {
        let inner = right_aligned_row(ui, |ui| {
            ui.label("ok");
            7u32
        });
        captured = Some(inner.inner);
    });
    assert_eq!(captured, Some(7));
}

#[test]
fn test_icon_button_renders() {
    let ctx = egui::Context::default();
    let output = run_ui_test(&ctx, raw_input(), |ui| {
        icon_button(ui, "x", "Close");
    });
    assert_no_id_change_in_shapes(&shapes_of(&output));
}

#[test]
fn test_panel_heading_renders() {
    let ctx = egui::Context::default();
    let output = run_ui_test(&ctx, raw_input(), |ui| {
        panel_heading(ui, "Table of Contents", 14.0);
    });
    assert_no_id_change_in_shapes(&shapes_of(&output));
}

#[test]
fn test_modal_window_renders_without_id_change() {
    let ctx = egui::Context::default();
    let mut open = true;
    let output = run_ui_test(&ctx, raw_input(), |ui| {
        modal_window("Widgets Test Modal", &mut open).show(ui.ctx(), |ui| {
            ui.label("body");
        });
    });
    assert!(open, "a modal left untouched must stay open");
    assert_no_id_change_in_shapes(&shapes_of(&output));
}

#[test]
fn test_menu_item_click_returns_true() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        ui.menu_button("Menu", |ui| {
            if menu_item(ui, "Open") {
                captured.push("open");
            }
            if menu_item(ui, "Close") {
                captured.push("close");
            }
        });
    });
    harness.set_size(egui::vec2(800.0, 600.0));
    harness.run_steps(2);

    harness.get_by_label("Menu").click();
    harness.run_steps(2);
    harness.run_steps(2);
    harness.get_by_label("Open").click();
    harness.run_steps(2);
    harness.run_steps(2);

    assert_eq!(
        harness.state(),
        &vec!["open"],
        "clicking a menu item twice must only fire the clicked item once"
    );
}

#[test]
fn test_menu_check_returns_post_click_value() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(Option::<bool>::None, |ui, toggled| {
        ui.menu_button("Menu", |ui| {
            if let Some(value) = menu_check(ui, "Wrap", false) {
                *toggled = Some(value);
            }
        });
    });
    harness.set_size(egui::vec2(800.0, 600.0));
    harness.run_steps(2);

    harness.get_by_label("Menu").click();
    harness.run_steps(2);
    harness.run_steps(2);
    harness.get_by_label("Wrap").click();
    harness.run_steps(2);
    harness.run_steps(2);

    assert_eq!(
        *harness.state(),
        Some(true),
        "toggling an unchecked item must report the post-click value"
    );
}

#[test]
fn test_bordered_panel_uses_fill_and_palette_border() {
    use crate::ui::theme::Palette;

    let ctx = egui::Context::default();
    let mut captured = None;
    let _ = run_ui_test(&ctx, raw_input(), |ui| {
        let fill = egui::Color32::from_rgb(1, 2, 3);
        let frame = bordered_panel(ui, fill);
        captured = Some((frame.fill, frame.stroke, frame.inner_margin));
    });
    let (fill, stroke, margin) = captured.expect("render closure ran");
    assert_eq!(fill, egui::Color32::from_rgb(1, 2, 3));
    assert_eq!(stroke.width, 1.0_f32);
    assert_eq!(stroke.color, Palette::fastmd_dark().border);
    assert_eq!(margin, egui::Margin::same(8));
}

#[test]
fn test_bordered_panel_renders_without_id_change() {
    let ctx = egui::Context::default();
    let output = run_ui_test(&ctx, raw_input(), |ui| {
        bordered_panel(ui, ui.palette().code_surface).show(ui, |ui| {
            ui.label("body");
        });
    });
    assert_no_id_change_in_shapes(&shapes_of(&output));
}

#[test]
fn test_search_field_accepts_typed_text() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use accesskit::Role;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(String::new(), |ui, text| {
        search_field(ui, text);
    });
    harness.set_size(egui::vec2(600.0, 200.0));
    harness.run_steps(2);

    harness.get_by_role(Role::TextInput).click();
    harness.run_steps(2);
    harness.get_by_role(Role::TextInput).type_text("note");
    harness.run_steps(2);

    assert_eq!(harness.state().as_str(), "note");
}

#[test]
fn test_no_op_sink_is_an_interaction_sink() {
    use crate::ui::test_helpers::interact::stateful_harness;

    // `no_op_sink` must satisfy the `InteractionSink` bound used by every
    // `*_capture` render seam.
    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        ui.menu_button("Menu", |ui| {
            if menu_item(ui, "Open") {
                no_op_sink("open");
            }
            if menu_item(ui, "Close") {
                captured.push("close");
            }
        });
    });
    use egui_kittest::kittest::Queryable;
    harness.set_size(egui::vec2(800.0, 600.0));
    harness.run_steps(2);

    harness.get_by_label("Menu").click();
    harness.run_steps(2);
    harness.run_steps(2);
    harness.get_by_label("Close").click();
    harness.run_steps(2);
    harness.run_steps(2);

    assert_eq!(harness.state(), &vec!["close"]);
}

#[test]
fn test_selectable_options_writes_back_and_marks_changed() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness((0usize, false), |ui, (current, changed)| {
        *changed |= selectable_options(
            ui,
            current,
            [(0usize, "Zero".to_string()), (1usize, "One".to_string())],
        );
    });
    harness.set_size(egui::vec2(800.0, 600.0));
    harness.run_steps(2);

    harness.get_by_label("One").click();
    harness.run_steps(2);
    harness.run_steps(2);

    let (current, changed) = harness.state();
    assert_eq!(*current, 1, "the picked option must be written back");
    assert!(*changed, "changing the selection must report `changed`");
}

#[test]
fn test_selectable_options_reports_unchanged_for_current_option() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness((0usize, false), |ui, (current, changed)| {
        *changed |= selectable_options(
            ui,
            current,
            [(0usize, "Zero".to_string()), (1usize, "One".to_string())],
        );
    });
    harness.set_size(egui::vec2(800.0, 600.0));
    harness.run_steps(2);

    harness.get_by_label("Zero").click();
    harness.run_steps(2);
    harness.run_steps(2);

    let (current, changed) = harness.state();
    assert_eq!(*current, 0);
    assert!(
        !*changed,
        "re-picking the already-selected option must not report `changed`"
    );
}

#[test]
fn test_selectable_combo_renders_selected_text_without_id_change() {
    let ctx = egui::Context::default();
    let mut current = 1usize;
    let mut changed = true;
    let output = run_ui_test(&ctx, raw_input(), |ui| {
        changed = selectable_combo(
            ui,
            "widgets_test_combo",
            "One",
            &mut current,
            [(0usize, "Zero".to_string()), (1usize, "One".to_string())],
        );
    });
    assert_eq!(current, 1);
    assert!(!changed, "a collapsed combo must not change the selection");
    assert_no_id_change_in_shapes(&shapes_of(&output));
}

#[test]
fn test_selectable_combo_empty_options_renders() {
    let ctx = egui::Context::default();
    let mut current = 3usize;
    let mut changed = true;
    let output = run_ui_test(&ctx, raw_input(), |ui| {
        changed = selectable_combo(
            ui,
            "widgets_test_empty_combo",
            "None",
            &mut current,
            std::iter::empty::<(usize, String)>(),
        );
    });
    assert_eq!(current, 3);
    assert!(!changed);
    assert_no_id_change_in_shapes(&shapes_of(&output));
}

#[test]
fn test_confirm_cancel_buttons_reports_confirm() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(DialogAction::None, |ui, action| {
        let now = confirm_cancel_buttons(ui, crate::ui::strings::OK_BUTTON, true);
        if now != DialogAction::None {
            *action = now;
        }
    });
    harness.set_size(egui::vec2(400.0, 200.0));
    harness.run_steps(2);

    harness.get_by_label(crate::ui::strings::OK_BUTTON).click();
    harness.run_steps(2);
    harness.run_steps(2);

    assert_eq!(*harness.state(), DialogAction::Confirm);
}

#[test]
fn test_confirm_cancel_buttons_reports_cancel() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(DialogAction::None, |ui, action| {
        let now = confirm_cancel_buttons(ui, crate::ui::strings::OK_BUTTON, true);
        if now != DialogAction::None {
            *action = now;
        }
    });
    harness.set_size(egui::vec2(400.0, 200.0));
    harness.run_steps(2);

    harness
        .get_by_label(crate::ui::strings::CANCEL_BUTTON)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    assert_eq!(*harness.state(), DialogAction::Cancel);
}

#[test]
fn test_confirm_cancel_buttons_disabled_confirm_is_inert() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(DialogAction::None, |ui, action| {
        let now = confirm_cancel_buttons(ui, crate::ui::strings::OK_BUTTON, false);
        if now != DialogAction::None {
            *action = now;
        }
    });
    harness.set_size(egui::vec2(400.0, 200.0));
    harness.run_steps(2);

    harness.get_by_label(crate::ui::strings::OK_BUTTON).click();
    harness.run_steps(2);
    harness.run_steps(2);

    assert_eq!(
        *harness.state(),
        DialogAction::None,
        "a disabled confirm button must not report a confirm"
    );
}

#[test]
fn test_confirm_cancel_buttons_renders_without_id_change() {
    let ctx = egui::Context::default();
    let output = run_ui_test(&ctx, raw_input(), |ui| {
        let _ = confirm_cancel_buttons(ui, crate::ui::strings::OK_BUTTON, true);
    });
    assert_no_id_change_in_shapes(&shapes_of(&output));
}
