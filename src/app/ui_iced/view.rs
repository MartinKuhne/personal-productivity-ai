//! The iced `view` — the render path for the whole application window.
//!
//! Renders the three-band layout with the real panels mounted, plus the
//! overlay host stacked above them. The window chrome (title, icon,
//! persisted geometry, scale) is supplied by the builder chain in
//! [`crate::ui_iced::app::launch`], not here.
//!
//! # Purity
//!
//! `view` reads [`AppState`] and builds widgets. It never publishes to a
//! bus, never touches the filesystem and never mutates anything — every
//! decision it makes (which overlays are mounted, what the document body
//! is, whether the TOC pane shows) is already encoded in the state the
//! reducer produced.
//!
//! # Why the view-models come from `AppState`
//!
//! Every panel function returns `Element<'_, Message>` that **borrows** its
//! view-model (the `pick_list` widgets hold references into the snapshot's
//! option vectors), so a view-model built inside `view` cannot outlive the
//! call (E0515). [`crate::ui_iced::app::AppState::rebuild_view_models`]
//! therefore runs at the end of every `update`, and `view` is a pure borrow
//! over the stored [`ViewModels`](crate::ui_iced::app::ViewModels).
//!
//! # Overlay z-order
//!
//! [`AppState::mounted_layers`] is the single selector that decides what
//! floats above the bands; the host mounts them in that order, bottom-most
//! first.
//!
//! Unit tests live in the sibling `view_tests.rs` sidecar.

use crate::bus::events::user_command::UserCommand;
use crate::markdown::parse_markdown_to_events;
use crate::ui::panel_layout::{LEFT_PANEL_DEFAULT_WIDTH, RIGHT_PANEL_DEFAULT_WIDTH};
use crate::ui_iced::about::about_card;
use crate::ui_iced::agent::panel_state::AgentPanelViewModel;
use crate::ui_iced::agent::transcript::agent_transcript_view;
use crate::ui_iced::agent_debug::agent_debug_card;
use crate::ui_iced::app::{AppState, OverlayLayer};
use crate::ui_iced::attributions::attributions_view;
use crate::ui_iced::batch_dialog::{BatchDialogViewModel, batch_dialog_card};
use crate::ui_iced::dialogs::dialog_card;
use crate::ui_iced::editor::editor_modal;
use crate::ui_iced::layout;
use crate::ui_iced::logs::background_logs_view;
use crate::ui_iced::message::{LayoutMessage, MenuMessage, Message, TreeMessage};
use crate::ui_iced::overlay::{menu_size, modal, with_card_at, with_context_menu};
use crate::ui_iced::panels::bottom::bottom_panel;
use crate::ui_iced::panels::center::center_panel_rendered;
use crate::ui_iced::panels::left::left_panel;
use crate::ui_iced::panels::right::toc_indent_pixels;
use crate::ui_iced::panels::top::{
    build_menu_entries, hamburger_menu_anchor, hamburger_menu_card, hamburger_menu_row_count,
    top_bar,
};
use crate::ui_iced::render::{RenderContext, render_document};
use iced::Length;
use iced::widget::{button, column, container, scrollable, stack, text};

/// Vertical gap between the three bands, in logical pixels.
const BAND_SPACING_PIXELS: f32 = 8.0;

/// Floor for the document render width so a collapsed-viewport solve never
/// gets a non-positive width.
const MIN_DOCUMENT_WIDTH_PIXELS: f32 = 240.0;

/// Renders the three-band layout plus the overlay host (`ViewFn`,
/// `iced-0.14.0/src/application.rs:628-638`).
pub fn view(state: &AppState) -> iced::Element<'_, Message> {
    overlay_host(state)
}

/// The three bands with the real panels mounted.
///
/// The right (TOC) pane is gated by the pure
/// [`layout::should_show_panel`] rule — `has_toc && has_selected_file`
/// ([RUST-051]).
fn base_bands(state: &AppState) -> iced::Element<'_, Message> {
    let has_selected_file = state.app.selection().selected_file.is_some();
    let has_toc = !state.app.orchestrator.tabs.toc.is_empty();

    column![
        container(top_bar(&state.models.top)).width(Length::Fill),
        container(layout::middle_band(
            &state.panes,
            || left_panel(&state.models.left),
            || center_body(state),
            || toc_body(state),
            has_toc,
            has_selected_file,
            |event| Message::Layout(LayoutMessage::PaneResized(event)),
        ))
        .width(Length::Fill)
        .height(Length::Fill),
        container(bottom_panel(&state.models.bottom)).width(Length::Fill),
    ]
    .spacing(BAND_SPACING_PIXELS)
    .into()
}

/// The center pane body, in routing order: agent session first, then the open
/// document, then the empty state.
fn center_body(state: &AppState) -> iced::Element<'_, Message> {
    if state.models.center.show_agent_session {
        agent_session_body(&state.models.agent)
    } else if state.models.center.has_tabs() {
        center_panel_rendered(&state.models.center, Some(document_body(state)))
    } else {
        center_panel_rendered(&state.models.center, None)
    }
}

/// The agent session view — the streaming transcript, shown while the center
/// pane's `show_agent_session` flag is on.
fn agent_session_body(view_model: &AgentPanelViewModel) -> iced::Element<'_, Message> {
    agent_transcript_view(view_model)
}

/// The active document rendered through the markdown path
/// (front-matter table above the body, FTWA table solve, task checkboxes).
fn document_body(state: &AppState) -> iced::Element<'static, Message> {
    let tabs = &state.app.orchestrator.tabs;
    let strategy = state.app.orchestrator.config.deficit_strategy();
    let events = parse_markdown_to_events(&tabs.current_markdown);
    let ctx = RenderContext::new(strategy, tabs.loaded_path.clone());
    render_document(
        &events,
        tabs.current_yaml.as_ref(),
        document_width(state),
        &ctx,
    )
}

/// The width the document render solve gets: the viewport minus the two
/// side panels.
///
/// The pane grid reports its real geometry only during layout, so the
/// reducer-held [`PanelLayout`](crate::ui::PanelLayout) widths (with the
/// UI-016 defaults) are the honest approximation available at build time.
fn document_width(state: &AppState) -> f32 {
    let left = state
        .app
        .layout
        .left_panel_width
        .unwrap_or(LEFT_PANEL_DEFAULT_WIDTH);
    let toc_visible = layout::should_show_panel(
        !state.app.orchestrator.tabs.toc.is_empty(),
        state.app.selection().selected_file.is_some(),
    );
    let right = if toc_visible {
        state
            .app
            .layout
            .right_panel_width
            .unwrap_or(RIGHT_PANEL_DEFAULT_WIDTH)
    } else {
        0.0
    };
    (state.viewport.width - left - right).max(MIN_DOCUMENT_WIDTH_PIXELS)
}

/// The right (TOC) pane body: one button per heading, each publishing a
/// [`UserCommand::ScrollToHeader`]. The per-level left pad comes from
/// [`toc_indent_pixels`].
fn toc_body(state: &AppState) -> iced::Element<'_, Message> {
    let rows: Vec<iced::Element<'_, Message>> = state
        .app
        .orchestrator
        .tabs
        .toc
        .iter()
        .map(|entry| {
            let indent = toc_indent_pixels(entry.level);
            container(button(text(entry.title.clone())).on_press(Message::Command(
                UserCommand::ScrollToHeader(entry.id.clone()),
            )))
            .padding(iced::Padding {
                top: 2.0,
                right: 4.0,
                bottom: 2.0,
                left: indent,
            })
            .width(Length::Fill)
            .into()
        })
        .collect();

    scrollable(column(rows).width(Length::Fill))
        .height(Length::Fill)
        .into()
}

/// The overlay host: the bands with every mounted floating window stacked
/// above them, bottom-most first.
///
/// The context menu and the hamburger menu are both folded into the base
/// with [`with_context_menu`] / [`with_card_at`] rather than stacked as
/// their own layer because those helpers own the click-away blocker +
/// anchored-card composition the tree, tab and top-bar menus need; every
/// higher window is a full-screen modal. The reducer keeps the two menus
/// mutually exclusive, so at most one of the two folds is live per frame.
fn overlay_host(state: &AppState) -> iced::Element<'_, Message> {
    let mut base = base_bands(state);

    if let Some(menu) = state.overlay.context_menu.as_ref()
        && !menu.items.is_empty()
    {
        base = with_context_menu(
            base,
            menu.anchor,
            &menu.items,
            state.viewport,
            Message::Tree(TreeMessage::ContextMenuClosed),
        );
    }

    if state.overlay.hamburger_open {
        let entries = build_menu_entries(&state.models.top);
        base = with_card_at(
            base,
            hamburger_menu_anchor(state.viewport),
            hamburger_menu_card(&entries, state.overlay.hamburger_submenu),
            menu_size(hamburger_menu_row_count(
                &entries,
                state.overlay.hamburger_submenu,
            )),
            state.viewport,
            Message::Menu(MenuMessage::Closed),
        );
    }

    let layers: Vec<iced::Element<'_, Message>> = state
        .mounted_layers()
        .into_iter()
        .filter(|layer| *layer != OverlayLayer::ContextMenu)
        .map(|layer| overlay_element(layer, state))
        .collect();

    if layers.is_empty() {
        base
    } else {
        let mut all = vec![base];
        all.extend(layers);
        stack(all).into()
    }
}

/// Builds one mounted overlay layer as a full-screen blocking element.
fn overlay_element(layer: OverlayLayer, state: &AppState) -> iced::Element<'_, Message> {
    match layer {
        OverlayLayer::ContextMenu => unreachable!(
            "the context menu is folded into the base by overlay_host, never stacked here"
        ),
        OverlayLayer::Editor => match state.overlay.editor.as_ref() {
            Some(buffer) => editor_modal(buffer),
            None => empty_layer(),
        },
        OverlayLayer::Dialog => dialog_card(
            &state.overlay.dialog,
            state.overlay.dialog_anchor,
            state.viewport,
        ),
        OverlayLayer::Logs => modal(
            background_logs_view(&state.models.logs),
            Message::Command(UserCommand::ToggleBackgroundLogs(false)),
        ),
        OverlayLayer::AgentDebug => modal(
            agent_debug_card(&state.models.debug),
            Message::Command(UserCommand::ToggleDebugWindow(false)),
        ),
        OverlayLayer::Tools => modal(
            crate::ui_iced::tools_dialog::tools_dialog_card(&state.models.tools),
            Message::Tools(crate::ui_iced::message::ToolsDialogMessage::CloseRequested),
        ),
        OverlayLayer::About => modal(
            about_card(&state.models.about),
            Message::About(crate::ui_iced::message::AboutMessage::CloseRequested),
        ),
        OverlayLayer::Attributions => modal(
            attributions_view(&state.models.attributions),
            Message::About(crate::ui_iced::message::AboutMessage::CloseRequested),
        ),
        OverlayLayer::Batch => batch_layer(&state.models.batch),
    }
}

/// The batch dialog layer: the card over a scrim whose dismiss publishes the
/// `CancelBatch` command.
fn batch_layer(view_model: &BatchDialogViewModel) -> iced::Element<'_, Message> {
    modal(
        batch_dialog_card(view_model),
        Message::Command(UserCommand::CancelBatch),
    )
}

/// An empty full-screen layer, standing in for a state the selector said was
/// mounted but the reducer has since cleared.
fn empty_layer() -> iced::Element<'static, Message> {
    container(text(""))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
