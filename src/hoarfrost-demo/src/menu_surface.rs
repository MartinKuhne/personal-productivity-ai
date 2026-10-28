//! The demo's menu surface host: the card the framework never mounts.
//!
//! `hoarfrost`'s `NavBar` reports a toggle and re-styles the trigger, and
//! *nothing else*: `composite/navbar.rs`'s `From<NavBar> for Element` builds
//! a `Row` of trigger buttons inside a `container` and never reads
//! `active_menu_id` to draw a menu. The framework's menu module is equally
//! explicit — `surface/menu/view.rs` lists "**The card.** No perimeter, no
//! 6px card padding, no 224–320px width policy" under *What is
//! deliberately not here*, and `MENU_CARD_WIDTH_MIN` is a constant no widget
//! in the crate consumes.
//!
//! Openness lives in `MenuCascade`, rows live in `MenuRow`, and the surface
//! that stacks them into a floating card is the **host's** job — which is
//! why the real desktop app has its own `ui_iced/menu_surface.rs`. This
//! module is the demo's half of that same contract: it wraps the framework's
//! `MenuRow` widgets in a card and overlays that card on the demo's base
//! view while a navbar trigger is latched open.
//!
//! Anchoring is deliberately simple. The card hangs below the navbar band at
//! the navbar's own leading inset. The demo has no live trigger geometry at
//! view time, and a fixed-width card at the top-left of a desktop viewport
//! cannot reach the flip/clamp cases `geometry::anchor` exists to solve, so
//! the demo places the card rather than solving for it.
//!
//! Unit tests live in the sibling `menu_surface_tests.rs` sidecar.

use iced::widget::{Column, container, stack};
use iced::{Alignment, Background, Border, Element, Length, Padding, Rectangle, Size};

use hoarfrost::composite::navbar::NAVBAR_HEIGHT_STANDARD;
use hoarfrost::surface::menu::MENU_CARD_WIDTH_MIN;
use hoarfrost::surface::menu::model::{Menu, Row};
use hoarfrost::surface::menu::view::{self, MenuRow};
use hoarfrost::token::color::Palette;
use hoarfrost::token::{shape, space};

use crate::app::Message;
use crate::data;

/// Prefix of the rendered menu card's stable widget id.
pub const CARD_ID_PREFIX: &str = "demo.menu.card.";

/// Prefix of a rendered menu row's stable widget id.
pub const ROW_ID_PREFIX: &str = "demo.menu.row.";

/// Width of the mounted card — the framework's own minimum menu-card width.
pub const CARD_WIDTH: f32 = MENU_CARD_WIDTH_MIN;

/// Padding between the card's edge and its rows (§4.8.1, `space.xs`).
pub const CARD_INSET: f32 = space::XS;

/// The band the card is anchored to: the navbar's own height.
pub const ANCHOR_BAND_HEIGHT: f32 = NAVBAR_HEIGHT_STANDARD;

/// Vertical gap between the navbar band and the card's top edge.
pub const CARD_GAP: f32 = space::XS;

/// Left inset of the card, matching the navbar's own horizontal padding.
pub const CARD_LEFT: f32 = space::SM;

/// The stable widget id of the card mounted for `trigger_id`.
#[must_use]
pub fn card_id(trigger_id: &str) -> String {
    format!("{CARD_ID_PREFIX}{trigger_id}")
}

/// The stable widget id of the row rendered for `row_key`.
#[must_use]
pub fn row_id(row_key: &str) -> String {
    format!("{ROW_ID_PREFIX}{row_key}")
}

/// The size the card occupies for `menu`: its rows' heights plus the inset.
#[must_use]
pub fn card_size(menu: &Menu) -> Size {
    let rows: f32 = menu.rows().iter().map(view::row_height).sum();
    Size::new(CARD_WIDTH, rows + CARD_INSET * 2.0)
}

/// Where the card for `menu` is mounted, in root-space coordinates.
#[must_use]
pub fn card_rect(menu: &Menu) -> Rectangle {
    let size = card_size(menu);
    Rectangle::new(
        iced::Point::new(CARD_LEFT, ANCHOR_BAND_HEIGHT + CARD_GAP),
        size,
    )
}

/// The card's own chrome: the `surface.overlay` perimeter the rows sit in.
///
/// `id` is the card's stable probe id, so a test can address the mounted
/// card by the trigger that opened it.
#[must_use]
pub fn card_container<'a>(rows: Column<'a, Message>, id: String) -> Element<'a, Message> {
    let palette = Palette::dark();

    container(rows)
        .id(iced::widget::Id::from(id))
        .width(Length::Fixed(CARD_WIDTH))
        .padding(CARD_INSET)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(palette.surface_overlay)),
            border: Border {
                color: palette.border_strong,
                width: shape::BORDER_HAIRLINE,
                radius: shape::RADIUS_CARD.into(),
            },
            shadow: iced::Shadow::default(),
            text_color: Some(palette.text_primary),
            snap: true,
        })
        .into()
}

/// One rendered row of the mounted card.
///
/// `key` is the row's **demo-declared** key rather than `Row::key()`:
/// the framework builds a separator keyless, so reading the `Row` alone
/// would publish `demo.menu.row.` — an id with an empty final segment —
/// for both separators. [`crate::data::menu_row_key`] is what carries
/// `m-sep-1` / `m-sep-2` through to the published id.
///
/// Every `RowEvent` is folded into the same demo message, which is how the
/// demo's existing menu preview (`pane_composites::menu_section`) already
/// treats its rows: the card is a showcase, not a live command surface.
#[must_use]
pub fn card_row<'a>(row: Row, key: &str) -> Element<'a, Message> {
    let key = key.to_string();

    MenuRow::new(row, row_id(&key), move |_event| {
        Message::MenuRowActivated(key.clone())
    })
    .palette(Palette::dark())
    .into()
}

/// The whole card mounted for `trigger_id` over `base`.
///
/// The card is pushed *after* `base` into the stack, so it draws on top of
/// the panes rather than underneath them.
#[must_use]
pub fn mount<'a>(
    base: Element<'a, Message>,
    trigger_id: &str,
    menu: &Menu,
) -> Element<'a, Message> {
    let rect = card_rect(menu);

    let mut rows = Column::<Message>::new().width(Length::Fill);
    for (index, row) in menu.rows().iter().enumerate() {
        let key = data::menu_row_key(index).unwrap_or_else(|| row.key());
        rows = rows.push(card_row(row.clone(), key));
    }

    let card = card_container(rows.width(Length::Fill), card_id(trigger_id));

    // A full-size container padded out to the card's origin keeps the card
    // in root-space coordinates regardless of pane or scroll offsets.
    let layer = container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Start)
        .align_y(Alignment::Start)
        .padding(Padding {
            top: rect.y,
            right: 0.0,
            bottom: 0.0,
            left: rect.x,
        });

    stack(vec![base, layer.into()])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// How many row widgets the card mounts for `menu`.
///
/// Every row kind is pushed, separators included, so this is the number of
/// row widgets the card adds to the frame.
#[must_use]
pub fn mounted_row_count(menu: &Menu) -> usize {
    menu.rows().len()
}

#[cfg(test)]
#[path = "menu_surface_tests.rs"]
mod tests;
