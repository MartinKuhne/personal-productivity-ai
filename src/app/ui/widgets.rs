//! Reusable egui widget helpers shared across panels, dialogs, the file tree
//! and the markdown renderers (RUST-051).
//!
//! Each helper wraps an egui idiom that was previously hand-written at many
//! call sites, so the canonical form (including the `ui.close()` menu
//! semantics and the id-salt plumbing) lives in exactly one place. The helpers
//! are intentionally thin adapters over egui primitives and must keep
//! rendering the same widget tree as the inline code they replace, so widget
//! ids stay stable across layout passes (see `src/app/AGENTS.md` §"egui::Id
//! stability rules").
//!
//! Unit tests live in the sibling `widgets_tests.rs` sidecar.

use crate::ui::theme::PaletteExt;
use eframe::egui;
use egui::RichText;

/// Renders `add` inside a horizontal row aligned to the right edge
/// (`Align::Center`).
///
/// This is the dialog button-bar and window-toolbar idiom: `ui.with_layout(
/// egui::Layout::right_to_left(egui::Align::Center), |ui| { … })`.
pub fn right_aligned_row<R>(
    ui: &mut egui::Ui,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add)
}

/// An icon button with a hover tooltip. Returns the button's [`egui::Response`].
pub fn icon_button(ui: &mut egui::Ui, icon: &str, tooltip: &str) -> egui::Response {
    ui.button(icon).on_hover_text(tooltip)
}

/// The "Search" label plus single-line text field shared by the log/debug
/// viewer toolbars.
pub fn search_field(ui: &mut egui::Ui, text: &mut String) {
    ui.label(crate::ui::strings::SEARCH_LABEL);
    ui.text_edit_singleline(text);
}

/// A menu or context-menu item.
///
/// Returns `true` when the item was clicked and asks the enclosing menu to
/// close, matching every context-menu item in the application. Callers publish
/// their command inside the `if` body.
pub fn menu_item(ui: &mut egui::Ui, label: &str) -> bool {
    if ui.button(label).clicked() {
        ui.close();
        true
    } else {
        false
    }
}

/// A toggle row inside a menu.
///
/// Renders `label` as a checkbox initialised to `checked`. When the user
/// toggles it, the enclosing menu is asked to close and the post-click value
/// is returned as `Some(value)`; otherwise `None` is returned. Returning the
/// post-click value (rather than a plain `bool`) lets callers publish the new
/// state without re-reading the checkbox.
pub fn menu_check(ui: &mut egui::Ui, label: &str, checked: bool) -> Option<bool> {
    let mut value = checked;
    if ui.checkbox(&mut value, label).clicked() {
        ui.close();
        Some(value)
    } else {
        None
    }
}

/// Builds a muted, italic placeholder label (empty states, "no results").
///
/// Returned as [`RichText`] so callers can still chain a size, e.g.
/// `ui.label(muted_text(MSG).size(15.0))`.
pub fn muted_text(text: &str) -> RichText {
    RichText::new(text).italics().color(egui::Color32::GRAY)
}

/// Renders an accent-coloured, strong panel heading at `size`.
///
/// This is the shared "panel title" style used by the right TOC and the centre
/// panel (the top/left panels use a different, non-accent heading and are
/// deliberately not routed through here).
pub fn panel_heading(ui: &mut egui::Ui, text: &str, size: f32) -> egui::Response {
    ui.heading(
        RichText::new(text)
            .size(size)
            .strong()
            .color(ui.palette().accent_text),
    )
}

/// The bordered, rounded content-panel frame shared by code blocks and the
/// YAML front-matter table.
///
/// Callers pass the surface colour (`palette().code_surface` for code blocks,
/// `palette().table_surface` for the YAML table); the border, padding and
/// corner radius are the shared values.
pub fn bordered_panel(ui: &egui::Ui, fill: egui::Color32) -> egui::Frame {
    egui::Frame::NONE
        .fill(fill)
        .stroke(egui::Stroke::new(1.0_f32, ui.palette().border))
        .inner_margin(8.0)
        .corner_radius(4.0)
}

/// Renders `(value, label)` options as selectable rows, writing the picked
/// value back into `current`.
///
/// Returns `true` when the selection changed. This is the popup body of
/// [`selectable_combo`], and is also useful for embedding a selection list
/// directly in a container.
pub fn selectable_options<T>(
    ui: &mut egui::Ui,
    current: &mut T,
    options: impl IntoIterator<Item = (T, String)>,
) -> bool
where
    T: PartialEq,
{
    let mut changed = false;
    for (value, label) in options {
        if ui.selectable_value(current, value, label).changed() {
            changed = true;
        }
    }
    changed
}

/// A combo box that lists `(value, label)` options and writes the picked value
/// back into `current`.
///
/// `selected_text` is the value shown on the closed combo box. Returns `true`
/// when the selection changed, so callers can run follow-up side effects.
///
/// `id_salt` must be unique per combo box on a given `Ui` (egui's id rules);
/// every call site passes a stable string literal.
pub fn selectable_combo<T, S>(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    selected_text: S,
    current: &mut T,
    options: impl IntoIterator<Item = (T, String)>,
) -> bool
where
    T: PartialEq,
    S: Into<egui::WidgetText>,
{
    let mut changed = false;
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(selected_text)
        .show_ui(ui, |ui| {
            changed = selectable_options(ui, current, options);
        });
    changed
}

/// The standard application modal window.
///
/// Closable through the title-bar `X` (which clears `open`) and
/// non-collapsible. Callers add size / resizability with the returned builder,
/// e.g. `modal_window("About", &mut open).resizable(true).default_size(…)`.
pub fn modal_window<'a>(title: &'a str, open: &'a mut bool) -> egui::Window<'a> {
    egui::Window::new(title).open(open).collapsible(false)
}

/// Sink for the interaction events emitted by the `*_capture` render seams.
///
/// A blanket alias for `FnMut(&'static str)`. Production render paths pass
/// [`no_op_sink`]; tests pass a closure that records the event names into the
/// `egui_kittest` harness's persistent state (see `ui::test_helpers::interact`).
pub trait InteractionSink: FnMut(&'static str) {}

impl<T: FnMut(&'static str)> InteractionSink for T {}

/// The no-op [`InteractionSink`] passed by production render paths.
pub fn no_op_sink(_event: &'static str) {}

// ---------------------------------------------------------------------------
// Tests live in the sibling `widgets_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "widgets_tests.rs"]
mod widgets_tests;
