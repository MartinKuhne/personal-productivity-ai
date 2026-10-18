//! The iced `view` ([TASK-022] minimal, [TASK-023] completes the
//! one-to-one mirror of the egui render path).
//!
//! Renders the [TASK-010] three-band layout: the top band, the middle-band
//! pane grid (left tree / center document / right TOC) with a placeholder
//! center body, and the bottom command band. The window chrome (title, icon,
//! persisted geometry, scale) is supplied by the builder chain in
//! [`crate::ui_iced::app::launch`], not here.
//!
//! # Why band stubs, not the real panels, in this minimal pass
//!
//! The real [`crate::ui_iced::panels::top::top_bar`] returns an
//! `Element<'_, Message>` that **borrows** its `TopBarViewModel` (the tag
//! `pick_list` holds `Option<&TagFilterOption>` from
//! `TopBarViewModel::selected_tag_option`, `pick_list.rs:156,165`), so
//! the element cannot escape `view` while the view model is a stack local
//! (E0515). Mounting the real panels therefore requires the view-model
//! lifetime to be resolved first — see "What [TASK-023] completes". The
//! [TASK-010] band stubs (`layout::top_band` / `bottom_band`) are `'a`-free
//! and let the app launch now, exactly the "top band + layout + bottom band
//! stubs" the task calls for.
//!
//! # What [TASK-023] completes
//!
//! * mount the real top toolbar in the top band (resolve the view-model
//!   lifetime — e.g. hold the `TopBarViewModel` in `AppState` and rebuild it
//!   in `update`, or make the panel return owned elements);
//! * the center document viewer (the [TASK-015] markdown render path over
//!   the open tab), replacing `center_placeholder`;
//! * the left tree / right TOC pane bodies (currently the [TASK-010]
//!   stubs), driven by the real view models;
//! * the bottom command band (the [TASK-016] prompt row), replacing the
//!   band stub;
//! * the overlay / modal / floating-window host (DEC-005) mounted over the
//!   three bands.
//!
//! Unit tests live in the sibling `view_tests.rs` sidecar.

use crate::ui_iced::app::AppState;
use crate::ui_iced::layout;
use crate::ui_iced::message::{LayoutMessage, Message};
use iced::widget::{column, container, text};

/// Vertical gap between the three bands, in logical pixels ([TASK-010]).
const BAND_SPACING_PIXELS: f32 = 8.0;

/// Placeholder text for the center document body. This is [TASK-022]
/// scaffolding only — [TASK-023] replaces it with the markdown viewer — so
/// it is a local const rather than a permanent `strings.rs` entry.
const CENTER_PLACEHOLDER: &str = "TASK-023: document viewer";

/// Builds the center-pane placeholder. A free function — `iced::Element` is
/// not `Clone`, so the pane-grid view callback reconstructs it per call.
fn center_placeholder() -> iced::Element<'static, Message> {
    container(text(CENTER_PLACEHOLDER))
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .center_x(iced::Length::Fill)
        .center_y(iced::Length::Fill)
        .into()
}

/// Renders the three-band layout (`ViewFn`,
/// `iced-0.14.0/src/application.rs:628-638`).
///
/// The top and bottom bands are the [TASK-010] stubs; the middle band is
/// the pane grid whose right (TOC) pane is gated by the reused
/// `should_show_panel` rule (via [`layout::middle_band`]).
///
/// The `|| center_placeholder()` closure is NOT redundant: the fn item
/// `fn() -> Element<'static, _>` forces the builder's `'a` to `'static`
/// (E0521 on `&state.panes`), while a closure infers its signature from
/// the expected bound and coerces the `'static` return covariantly.
#[allow(clippy::redundant_closure)]
pub fn view(state: &AppState) -> iced::Element<'_, Message> {
    let has_selected_file = state.app.selection().selected_file.is_some();
    // [TASK-023]: the TOC model drives this gate; no document model yet.
    let has_toc = false;

    column![
        layout::top_band(),
        layout::middle_band(
            &state.panes,
            || center_placeholder(),
            has_toc,
            has_selected_file,
            |event| Message::Layout(LayoutMessage::PaneResized(event)),
        ),
        layout::bottom_band(),
    ]
    .spacing(BAND_SPACING_PIXELS)
    .into()
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
