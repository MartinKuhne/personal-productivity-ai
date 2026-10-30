//! The §4.7.1 four-band frame shared by the app's Band-1 movable utility
//! cards (`UIF-MOV-001` … `UIF-MOV-004`, INT-6).
//!
//! §4.7.1 draws the Background Operations Log and the Agent Debug
//! Inspector with the same silhouette: a draggable title bar, a filter /
//! control bar, a sunken monospace viewport, and a status footer carrying
//! a resize grip. Both cards render through here so the silhouette is
//! derived once from the framework's own band constants rather than being
//! restated per card ([RUST-021]) — and so the second card cannot drift
//! from the first.
//!
//! # The frame is geometry, not a widget
//!
//! Every band's height comes off
//! [`MovableHost`]'s resolved
//! rectangles, which are themselves the framework's
//! [`TITLE_BAR_HEIGHT`] / [`FILTER_BAR_HEIGHT`] / [`FOOTER_HEIGHT`] rungs.
//! The host is the single source of truth for where each band is, so the
//! painted card and the rectangle
//! [`SurfaceRegistry`](hoarfrost::surface::zorder::SurfaceRegistry) routes
//! pointer events against cannot disagree.
//!
//! # Pointer routing
//!
//! The title bar carries a [`mouse_area`] that reports moves and a press,
//! which the reducer drives through the host to start a drag
//! (`UIF-MOV-001`); the footer's ◢ carries one that starts a resize
//! (`UIF-MOV-002`). Nesting those inside the card-wide absorbing
//! [`mouse_area`] is safe: iced's `MouseArea::update` dispatches to its
//! content **first** and only claims the event if nothing below captured it
//! (`iced_widget-0.14.2/src/mouse_area.rs:230-245`), so the buttons and
//! fields inside a band keep working while the card still absorbs whatever
//! they leave behind.
//!
//! Unit tests live in the sibling `movable_shell_tests.rs` sidecar.

#[cfg(test)]
#[path = "movable_shell_tests.rs"]
mod tests;

use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{Message, MovableCard, MovableMessage};
use hoarfrost::geometry::clamp::ResizeHandle;
use hoarfrost::surface::movable::{
    FILTER_BAR_HEIGHT, FOOTER_HEIGHT, MovableHost, RESIZE_HANDLE_SIZE, TITLE_BAR_HEIGHT,
    TITLE_BAR_PADDING_HORIZONTAL,
};
use hoarfrost::widget::button::ButtonVariant;
use hoarfrost::widget::button_widget::Button;
use iced::widget::{Column, Row, container, mouse_area, space, text};
use iced::{Alignment, Background, Border, Element, Length, Padding, Point};

/// Id string of the log card's content viewport.
const LOGS_VIEWPORT_ID: &str = "movable.logs.viewport";

/// Id string of the agent-debug card's content viewport.
const DEBUG_VIEWPORT_ID: &str = "movable.agent_debug.viewport";

/// The grip glyph shown at the leading edge of a movable title bar.
const TITLE_GRIP_GLYPH: &str = "⣿";

// The footer's resize corner draws [`strings::ICON_RESIZE_GRIP`], the same
// mark the centred modals' grips draw (defect #6) — one constant, so a surface
// that can be resized always looks like every other one that can.

/// The × glyph painted in the title bar's Close button.
const CLOSE_GLYPH: &str = "×";

/// The recessed fill of the content viewport (§4.7.1 "sunken").
const VIEWPORT_PADDING: f32 = 4.0;

/// Gap between the title grip glyph and the title text.
const TITLE_GRIP_GAP: f32 = 6.0;

/// Focus id for the Dock button in the title bar.
const DOCK_BUTTON_FOCUS_ID: &str = "movable.title_bar.dock";

/// Focus id for the Close button in the title bar.
///
/// Public so a card's own test can address the shell-supplied `Close ×`
/// rather than restating the string ([RUST-021]).
pub const CLOSE_BUTTON_FOCUS_ID: &str = "movable.title_bar.close";

/// Stable iced widget id of one movable card's content viewport.
///
/// One id per card so the reducer can aim a scroll operation at it
/// (AGENTS §5), and so the two cards never share a live id.
#[must_use]
pub fn content_viewport_id(card: MovableCard) -> iced::widget::Id {
    match card {
        MovableCard::Logs => iced::widget::Id::new(LOGS_VIEWPORT_ID),
        MovableCard::AgentDebug => iced::widget::Id::new(DEBUG_VIEWPORT_ID),
    }
}

/// Render one Band-1 movable card in the §4.7.1 four-band silhouette.
///
/// * `host` — the card's [`MovableHost`], source of every band rectangle.
/// * `card` — which card this is, so pointer messages name the right host.
/// * `controls` — the filter / control bar's contents (search field,
///   auto-scroll checkbox, clear button …).
/// * `content` — the sunken viewport's contents; fills the remaining height.
/// * `status` — the footer's leading status line.
///
/// The returned element is the **card only**, sized to the host's geometry.
/// Mount it with
/// [`movable_layer`](crate::ui_iced::overlay::movable_layer) so it lands at
/// the host's position over a scrim-free full-screen container
/// (`UIF-MOV-003`).
#[must_use]
pub fn movable_shell<'a>(
    host: &MovableHost,
    card: MovableCard,
    controls: Row<'a, Message>,
    content: Element<'a, Message>,
    status: Element<'a, Message>,
) -> Element<'a, Message> {
    let palette = Palette::fastmd_dark();
    let geometry = host.geometry();

    let body: Column<'a, Message> = Column::new()
        .push(title_bar(host, card, palette))
        .push(filter_bar(controls, palette))
        .push(Element::from(
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(Padding::new(VIEWPORT_PADDING))
                .style(move |_| container::Style {
                    // §4.7.1's "sunken" viewport: the code-fence surface
                    // recessed below the card's own overlay fill, bounded
                    // by the subtle border rung.
                    background: Some(Background::Color(palette.code_surface)),
                    border: Border {
                        color: palette.border,
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    text_color: Some(palette.text),
                    ..Default::default()
                }),
        ))
        .push(footer(card, status, palette));

    // The card-wide absorber. A press anywhere on the card that no inner
    // control claimed stops here instead of falling through to Band 0 —
    // `UIF-MOV-003` says the card owns its inside and nothing outside it.
    // It reports the press so the host can raise this card above the other
    // Band-1 card: clicking a card's body is the promotion gesture, and
    // without this lane only grabbing the title bar or the grip would raise
    // it. `MouseArea::update` dispatches to content first, so buttons and
    // fields still take their own clicks and only the leftovers land here.
    let absorbed =
        mouse_area(body).on_press(Message::Movable(MovableMessage::BodyPressed { card }));

    container(absorbed)
        .id(content_viewport_id(card))
        .width(Length::Fixed(geometry.size.width))
        .height(Length::Fixed(geometry.size.height))
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.table_surface)),
            border: Border {
                color: palette.border_strong,
                width: 1.0,
                radius: 2.0.into(),
            },
            text_color: Some(palette.text),
            ..Default::default()
        })
        .into()
}

/// The §4.7.1 title bar — `[⣿] Title ……… [Dock] [Close ×]`,
/// [`TITLE_BAR_HEIGHT`] tall and draggable everywhere except the trailing
/// action cluster (`UIF-MOV-001`).
fn title_bar(host: &MovableHost, card: MovableCard, palette: Palette) -> Element<'static, Message> {
    let title = text(host.title().to_owned())
        .size(crate::ui_iced::logs::ROW_TEXT_SIZE)
        .color(palette.text_strong);

    let bar = Row::new()
        .push(text(TITLE_GRIP_GLYPH).color(palette.text_muted))
        .push(space::Space::new().width(Length::Fixed(TITLE_GRIP_GAP)))
        .push(title)
        .push(space::Space::new().width(Length::Fill))
        .push(dock_action(card))
        .push(close_action(card))
        .align_y(Alignment::Center)
        .height(Length::Fixed(TITLE_BAR_HEIGHT))
        .padding(Padding::from([0.0, TITLE_BAR_PADDING_HORIZONTAL]));

    // The drag lane. `on_move` is the only lane iced 0.14 gives a
    // coordinate on, and the reducer pairs the press with the last position
    // it reported — see `MovableMessage` for why.
    mouse_area(bar)
        .on_move(move |at: Point| Message::Movable(MovableMessage::Moved { card, at }))
        .on_press(Message::Movable(MovableMessage::TitlePressed { card }))
        .interaction(iced::mouse::Interaction::Grab)
        .into()
}

/// The title bar's `Dock` trailing action (`UIF-MOV-001`).
fn dock_action(card: MovableCard) -> Button<'static, Message> {
    Button::new(
        ButtonVariant::Ghost,
        text(strings::MOVABLE_DOCK_BUTTON).size(crate::ui_iced::logs::ROW_TEXT_SIZE),
        Message::Movable(MovableMessage::DockRequested { card }),
        DOCK_BUTTON_FOCUS_ID,
    )
    .palette(Palette::pro())
}

/// The title bar's `Close ×` trailing action (`UIF-MOV-001`).
///
/// The `×` glyph measures below §3.1's icon-only floor, so this control *is*
/// an icon-only button and `UIF-ATOM-001`'s icon-only cell makes the tooltip
/// mandatory — it is the only place the label exists. Built with
/// [`Button::with_tooltip`] carrying [`strings::MOVABLE_CLOSE_TOOLTIP`]; the
/// pinned test in the sidecar asserts the icon-only rule is both engaged and
/// satisfied.
fn close_action(card: MovableCard) -> Button<'static, Message> {
    Button::new(
        ButtonVariant::Ghost,
        text(CLOSE_GLYPH).size(crate::ui_iced::logs::ROW_TEXT_SIZE),
        Message::Movable(MovableMessage::CloseRequested { card }),
        CLOSE_BUTTON_FOCUS_ID,
    )
    .palette(Palette::pro())
    .with_tooltip(strings::MOVABLE_CLOSE_TOOLTIP)
}

/// The §4.7.1 filter / control bar, [`FILTER_BAR_HEIGHT`] tall.
fn filter_bar<'a>(controls: Row<'a, Message>, palette: Palette) -> Element<'a, Message> {
    container(controls)
        .width(Length::Fill)
        .height(Length::Fixed(FILTER_BAR_HEIGHT))
        .align_y(Alignment::Center)
        .padding(Padding::from([0.0, 12.0]))
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.surface_variant)),
            border: Border {
                color: palette.border,
                width: 1.0,
                radius: 0.0.into(),
            },
            text_color: Some(palette.text),
            ..Default::default()
        })
        .into()
}

/// The §4.7.1 status footer, [`FOOTER_HEIGHT`] tall, with the ◢ grip in the
/// bottom-right corner (`UIF-MOV-002`).
fn footer<'a>(
    card: MovableCard,
    status: Element<'a, Message>,
    palette: Palette,
) -> Element<'a, Message> {
    let grip = mouse_area(
        container(text(strings::ICON_RESIZE_GRIP).color(palette.text_muted))
            .width(Length::Fixed(RESIZE_HANDLE_SIZE))
            .height(Length::Fixed(RESIZE_HANDLE_SIZE))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    )
    .on_press(Message::Movable(MovableMessage::GripPressed {
        card,
        handle: ResizeHandle::BottomRight,
    }))
    .interaction(iced::mouse::Interaction::ResizingDiagonallyDown);

    let bar = Row::new()
        .push(status)
        .push(space::Space::new().width(Length::Fill))
        .push(grip)
        .align_y(Alignment::Center)
        .height(Length::Fixed(FOOTER_HEIGHT))
        .padding(Padding::from([0.0, 12.0]));

    container(bar)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(palette.surface_variant)),
            border: Border {
                color: palette.border,
                width: 1.0,
                radius: 0.0.into(),
            },
            text_color: Some(palette.text_muted),
            ..Default::default()
        })
        .into()
}

/// The full-screen capture lane a card mounts **while it holds a drag or a
/// resize** (`UIF-MOV-001`, `UIF-MOV-002`).
///
/// Without it the moves stop the moment the card runs out from under the
/// pointer: the title bar's own [`mouse_area`] only reports while the
/// cursor is over the title bar, and a drag that outruns the cursor would
/// freeze the card mid-gesture. Mounting a full-area `mouse_area` above
/// every layer for the duration of the gesture keeps `Moved` arriving, and
/// the release lands here too rather than being lost on whatever the card
/// just uncovered.
///
/// This is the one place a full-area blocker is legitimate: it exists only
/// while the user is holding the card, and the whole point of holding it is
/// that they are not clicking anything else.
#[must_use]
pub fn gesture_capture_layer(card: MovableCard) -> Element<'static, Message> {
    mouse_area(
        container(space::Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_move(move |at: Point| Message::Movable(MovableMessage::Moved { card, at }))
    .on_release(Message::Movable(MovableMessage::Released { card }))
    .into()
}
