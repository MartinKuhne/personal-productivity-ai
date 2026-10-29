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
use crate::markdown::ToCEntry;
use crate::markdown::parse_markdown_to_events;
use crate::ui::panel_layout::{LEFT_PANEL_DEFAULT_WIDTH, RIGHT_PANEL_DEFAULT_WIDTH};
use crate::ui::theme::Palette;
use crate::ui_iced::about::about_card;
use crate::ui_iced::agent::agent_panel;
use crate::ui_iced::app::{AppState, OverlayLayer, TooltipId};
use crate::ui_iced::attributions::attributions_view;
use crate::ui_iced::batch_dialog::{BatchDialogViewModel, batch_modal};
use crate::ui_iced::dialogs::dialog_card;
use crate::ui_iced::editor::editor_modal;
use crate::ui_iced::layout;
use crate::ui_iced::message::{LayoutMessage, Message, MovableCard};
use crate::ui_iced::overlay::modal_centered;
use crate::ui_iced::panels::bottom::bottom_panel;
use crate::ui_iced::panels::center::center_panel_rendered;
use crate::ui_iced::panels::left::left_panel;
use crate::ui_iced::panels::right::toc_indent_pixels;
use crate::ui_iced::panels::top::top_bar;
use crate::ui_iced::render::{RenderContext, render_document};
use hoarfrost::surface::tooltip::{TooltipAnchor, TooltipStyle};
use hoarfrost::token::color::Palette as ProPalette;
use hoarfrost::token::typography::{self, TypeRung};
use hoarfrost::widget::button::ButtonVariant;
use hoarfrost::widget::button_widget::Button;
use iced::Length;
use iced::Size;
use iced::widget::{column, container, mouse_area, scrollable, stack, text};

/// Vertical gap between the three bands, in logical pixels.
const BAND_SPACING_PIXELS: f32 = 8.0;

/// Floor for the document render width so a collapsed-viewport solve never
/// gets a non-positive width.
const MIN_DOCUMENT_WIDTH_PIXELS: f32 = 240.0;

/// Stable iced widget id of the TOC-pane `scrollable`.
///
/// The reducer needs a stable id to aim a scroll operation at the pane, and
/// a widget without an explicit id is re-keyed on every rebuild and loses
/// its scroll position. Named once so the view and the reducer cannot drift
/// (§5 id-stability rule, [RUST-021]).
pub const TOC_SCROLLABLE_ID: &str = "toc_scroll";

/// The type rung a Table-of-Contents entry label sits on — `type.ui_compact`
/// (`UIF-TOK-004`), the same 13px / 18px-line rung the left-hand file tree
/// paints its row labels at
/// ([`crate::ui_iced::panels::left::ROW_TEXT_SIZE`]).
///
/// The ToC and the tree are read side by side, so they are stated as one
/// rung rather than two numbers that can drift ([RUST-021]). §2.4 names
/// `type.ui_compact` for exactly this usage — "dense UI lists: directory
/// tree rows, table of contents".
///
/// Held as a whole [`TypeRung`] rather than a bare point size because the
/// row needs the line box as well, and that box must cross into iced through
/// [`TypeRung::iced_line_height`]: `impl From<f32> for iced::LineHeight`
/// produces `LineHeight::Relative(factor)`, so handing `.line_height(...)`
/// the rung's raw `18.0` asks iced for 13 × 18 = **234px per line**.
pub const TOC_TEXT_RUNG: TypeRung = typography::UI_COMPACT;

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

    let bands = column![
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
    .spacing(BAND_SPACING_PIXELS);

    // MouseArea positions are relative to their own bounds. Tracking here,
    // around the full-window content, gives context menus the same root-space
    // coordinates that menu_overlay uses, regardless of pane or scroll offsets.
    mouse_area(bands)
        .on_move(|point| Message::Layout(LayoutMessage::CursorMoved { point }))
        .into()
}

/// The center pane body, in routing order: agent session first, then the open
/// document, then the empty state.
fn center_body(state: &AppState) -> iced::Element<'_, Message> {
    if state.models.center.show_agent_session {
        agent_panel(&state.models.agent)
    } else if state.models.center.has_tabs() {
        center_panel_rendered(&state.models.center, Some(document_body(state)))
    } else {
        center_panel_rendered(&state.models.center, None)
    }
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
            container(toc_entry_button(entry))
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
        .id(TOC_SCROLLABLE_ID)
        .height(Length::Fill)
        .into()
}

/// One ToC entry row's label, at the rung the file tree paints at.
///
/// The single place this pane turns [`TOC_TEXT_RUNG`] into a `text` widget,
/// so no call site restates a point size ([RUST-021], `UIF-TOK-004`).
/// Before this, the label carried no `.size()` at all and iced painted it at
/// its own 16px default — a rung above the tree it sits beside.
///
/// The line height goes through [`TypeRung::iced_line_height`], never the
/// rung's raw `f32`: `From<f32> for iced::LineHeight` is `Relative`, so a
/// bare `18.0` on a 13px font means 234px per line.
fn toc_entry_label(title: String) -> iced::widget::Text<'static> {
    text(title)
        .size(TOC_TEXT_RUNG.size)
        .line_height(TOC_TEXT_RUNG.iced_line_height())
}

/// The ToC entry's button, tooltiped against the icon-only rule.
///
/// `UIF-ATOM-001`'s icon-only cell makes a tooltip mandatory on any button
/// whose content measures at or below `size::CONTROL_SM` (28×28), and
/// `Button::outer_size` (`button_widget.rs:799`) `debug_assert!`s that rule
/// the moment the box is computed. A markdown heading of one or two
/// characters (`# A`) makes a row label that small, so scrolling a document
/// that opens with a short heading crashed the app.
///
/// The tooltip is the entry's own heading, so no new user-facing copy is
/// invented; the shape matches the movable shell's `close_action`, which was
/// fixed the same way.
fn toc_entry_button(entry: &ToCEntry) -> Button<'static, Message> {
    Button::new(
        ButtonVariant::Ghost,
        toc_entry_label(entry.title.clone()),
        Message::Command(UserCommand::ScrollToHeader(entry.id.clone())),
        format!("toc_entry_{}", entry.id),
    )
    .palette(Palette::pro())
    .with_tooltip(entry.title.clone())
}

/// The overlay host: the bands with every mounted floating window stacked
/// above them, bottom-most first.
///
/// The tree/tab context menu and the top-bar hamburger are both rendered by
/// the `hoarfrost` menu engine and folded into the base here, each with its own
/// click-away blocker and solved card geometry. Higher windows are stacked
/// above them as full-screen overlays. The reducer keeps the two menus
/// mutually exclusive, so at most one menu surface is live per frame.
///
/// The tooltip subsystem (`UIF-ATOM-007`) mounts as a presentation layer above
/// all z-bands — it is not a z-band itself and does not participate in escape
/// handling. Dismissal is a pre-pass in the reducer.
fn overlay_host(state: &AppState) -> iced::Element<'_, Message> {
    let mut base = base_bands(state);

    if let Some(menu) = state.overlay.context_menu.as_ref() {
        base = crate::ui_iced::context_menu::overlay(base, &menu.cascade, &menu.rects);
    }

    if state.overlay.hamburger_open {
        base = crate::ui_iced::hamburger::overlay(
            base,
            &state.overlay.hamburger_cascade,
            &state.overlay.hamburger_rects,
        );
    }

    let layers: Vec<iced::Element<'_, Message>> = state
        .mounted_layers()
        .into_iter()
        .filter(|layer| *layer != OverlayLayer::ContextMenu)
        .map(|layer| overlay_element(layer, state))
        .collect();

    let mut all = vec![base];
    all.extend(layers);

    // Tooltip presentation layer — mounted above all z-bands (`UIF-ATOM-007`).
    if let Some(tooltip_element) = tooltip_layer(state) {
        all.push(tooltip_element);
    }

    stack(all).into()
}

/// Tooltip presentation layer — mounted above all z-bands (`UIF-ATOM-007`).
///
/// This is a pure presentation layer, not a z-band. It renders the currently
/// active tooltip (if any) using the Hoarfrost tooltip subsystem's styling and
/// placement. The tooltip subsystem handles:
/// - Hover delay (400ms pointer, 0ms keyboard focus)
/// - Warm handoff (100ms window for cross-control immediate show)
/// - Placement (below, left-aligned with flip/clamp)
/// - Styling (surface.overlay fill, border.strong border, text.primary text)
fn tooltip_layer(state: &AppState) -> Option<iced::Element<'_, Message>> {
    let tooltip = &state.overlay.tooltip;
    let active_tooltip = tooltip.active_tooltip?;
    let now = hoarfrost::surface::tooltip::TimeMs::now();

    // Check visibility for the active tooltip
    let is_visible = match active_tooltip {
        TooltipId::SearchTrigger => tooltip.search_trigger_controller.tick(now).is_visible(),
        TooltipId::SearchClear => tooltip.search_clear_controller.tick(now).is_visible(),
        TooltipId::ToolsError => tooltip.tools_error_controller.tick(now).is_visible(),
    };

    if !is_visible {
        return None;
    }

    // Get tooltip text and anchor for the active tooltip
    let (tooltip_text, anchor) = match active_tooltip {
        TooltipId::SearchTrigger => (crate::ui::strings::SEARCH_TRIGGER_TOOLTIP.to_string(), None),
        TooltipId::SearchClear => (crate::ui::strings::SEARCH_CLEAR_TOOLTIP.to_string(), None),
        TooltipId::ToolsError => {
            let text = tooltip.tools_error_tooltip.clone().unwrap_or_default();
            let anchor = tooltip.tools_error_anchor;
            (text, anchor)
        }
    };

    // Resolve tooltip style from palette
    let palette = ProPalette::dark();
    let style = TooltipStyle::resolve(&palette);

    // For now, use a simple anchored tooltip at cursor position
    // TODO: Proper anchor resolution from control layout
    let cursor = state.overlay.cursor.unwrap_or(iced::Point::ORIGIN);
    let anchor = anchor.unwrap_or(TooltipAnchor::Pointer(cursor));

    // Measure card size (approximate)
    let card_size = iced::Size::new(style.max_width.min(280.0), 40.0);

    // Resolve placement
    let viewport = iced::Rectangle::new(iced::Point::ORIGIN, state.viewport);
    let placement = anchor.resolve(card_size, viewport);

    // Build tooltip card
    let tooltip_element = container(
        text(tooltip_text)
            .size(style.font.size)
            .line_height(style.font.iced_line_height())
            .color(style.foreground),
    )
    .padding(iced::Padding {
        top: style.padding_vertical,
        bottom: style.padding_vertical,
        left: style.padding_horizontal,
        right: style.padding_horizontal,
    })
    .style(move |_theme: &iced::Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(style.background)),
        border: iced::Border {
            color: style.border_color,
            width: style.border_width,
            radius: style.radius.into(),
        },
        ..Default::default()
    })
    .width(Length::Shrink)
    .height(Length::Shrink);

    // Position as overlay at the resolved placement
    Some(
        container(tooltip_element)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Left)
            .align_y(iced::alignment::Vertical::Top)
            .padding(iced::Padding {
                top: placement.rect().y,
                left: placement.rect().x,
                ..Default::default()
            })
            .into(),
    )
}

/// Builds one mounted overlay layer as a full-screen blocking element.
fn overlay_element(layer: OverlayLayer, state: &AppState) -> iced::Element<'_, Message> {
    match layer {
        OverlayLayer::ContextMenu => unreachable!(
            "the context menu is folded into the base by overlay_host, never stacked here"
        ),
        OverlayLayer::Editor => match state.overlay.editor.as_ref() {
            Some(buffer) => editor_modal(buffer, state.viewport),
            None => empty_layer(),
        },
        OverlayLayer::Dialog => dialog_card(
            &state.overlay.dialog,
            state.overlay.dialog_anchor,
            state.viewport,
        ),
        OverlayLayer::Logs => movable_overlay_layer(MovableCard::Logs, state, |host| {
            crate::ui_iced::logs::background_logs_movable_card(host, &state.models.logs)
        }),
        OverlayLayer::AgentDebug => movable_overlay_layer(MovableCard::AgentDebug, state, |host| {
            crate::ui_iced::agent_debug::agent_debug_movable_card(host, &state.models.debug)
        }),
        OverlayLayer::Tools => {
            crate::ui_iced::tools_dialog::tools_modal(&state.models.tools, state.viewport)
        }
        OverlayLayer::About => modal_centered(
            about_card(&state.models.about, state.viewport),
            Message::About(crate::ui_iced::message::AboutMessage::Absorbed),
            Message::About(crate::ui_iced::message::AboutMessage::CloseRequested),
        ),
        OverlayLayer::Attributions => modal_centered(
            attributions_view(&state.models.attributions, state.viewport),
            Message::About(crate::ui_iced::message::AboutMessage::Absorbed),
            Message::About(crate::ui_iced::message::AboutMessage::CloseRequested),
        ),
        OverlayLayer::Batch => batch_layer(&state.models.batch, state.viewport),
    }
}

/// The batch dialog layer: the framed card centred over a scrim, with the
/// card-boundary absorption and `CancelBatch` as the dismiss.
fn batch_layer(view_model: &BatchDialogViewModel, viewport: Size) -> iced::Element<'_, Message> {
    batch_modal(view_model, viewport)
}

/// One Band-1 movable utility layer — **no scrim** (`UIF-MOV-003`, INT-6).
///
/// Replaces the `modal(...)` mount these two cards used before. The old
/// mount laid a full-area `scrim_blocker` under the card, which swallowed
/// every click outside it and made the log viewer a blocking dialog — the
/// exact thing §4.7 forbids, since the user watches these cards *while*
/// working in the document behind them.
///
/// Here the card is positioned at its host's own geometry over a
/// scrim-free full-screen container, so a press outside the card falls
/// through to Band 0. When the host is holding a drag or a resize, the
/// gesture capture lane is stacked on top so moves keep arriving after the
/// card has run out from under the pointer.
///
/// `build` receives the card's host so the caller can size its §4.7.1
/// bands from the host's resolved rectangles rather than a locally-computed
/// box. Falls back to an empty layer when no host is synced — a state the
/// reducer should never leave, but one that must not panic inside a frame.
fn movable_overlay_layer<'a>(
    card: MovableCard,
    state: &'a AppState,
    build: impl FnOnce(&'a hoarfrost::surface::movable::MovableHost) -> iced::Element<'a, Message>,
) -> iced::Element<'a, Message> {
    let host = match card {
        MovableCard::Logs => state.overlay.logs_movable.as_ref(),
        MovableCard::AgentDebug => state.overlay.debug_movable.as_ref(),
    };

    let Some(host) = host else {
        return empty_layer();
    };

    let framed = crate::ui_iced::overlay::movable_layer(build(host), host.geometry());

    if host.is_capturing() {
        iced::widget::stack(vec![
            framed,
            crate::ui_iced::movable_shell::gesture_capture_layer(card),
        ])
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
    } else {
        framed
    }
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
