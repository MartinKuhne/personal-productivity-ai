//! Shared card chrome and solved geometry for framework-driven menus.
//!
//! Both menu surfaces this application renders on the `iced-pro` menu engine —
//! the top-bar hamburger ([the hamburger surface](crate::ui_iced::hamburger)) and the
//! file-tree / tab [context menu](crate::ui_iced::context_menu) — mount the
//! same §4.8.1 card around a column of
//! [`MenuRow`](iced_pro::surface::menu::view::MenuRow) widgets, and both solve
//! the same "one viewport rectangle per open card, root first" walk that the
//! [`SurfaceRegistry`](iced_pro::surface::zorder::SurfaceRegistry) is fed.
//!
//! Exactly **two** things differ between the two surfaces, and both are
//! parameters here rather than copies:
//!
//! * where the **root** card is anchored — a synthesised navbar band for the
//!   hamburger (`UIF-FMENU-001`), the pointer for a context menu
//!   (`UIF-CMENU-001`);
//! * which message the click-away blocker publishes on dismissal.
//!
//! Keeping the shared half in one module is what stops the two surfaces from
//! drifting apart on card inset, card width, the row grid and the focus-id
//! wiring — the four things that are invisible in a diff once duplicated.
//!
//! # Geometry is solved once, then painted
//!
//! Nothing in the render path calls the anchor solver.
//! [`open_card_rects`](crate::ui_iced::menu_surface::open_card_rects) runs
//! from the reducer (via
//! [`AppState::refresh_hamburger_rects`](crate::ui_iced::app::AppState::refresh_hamburger_rects)
//! and its context-menu counterpart), and
//! [`overlay`](crate::ui_iced::menu_surface::overlay) paints exactly the
//! rectangles it is handed. A card past the end of `rects` is not mounted, so
//! an unsolved card can never appear at a guessed position.
//!
//! Unit tests live in the sibling `menu_surface_tests.rs` sidecar.

#[cfg(test)]
#[path = "menu_surface_tests.rs"]
mod tests;

use std::collections::HashMap;

use iced::widget::{Column, container, stack};
use iced::{Element, Length, Padding, Point, Rectangle, Size};

use iced_pro::geometry::anchor::{self, Placement};
use iced_pro::surface::menu::cascade::{MenuCascade, OpenCard};
use iced_pro::surface::menu::model::{Menu, MenuId};
use iced_pro::surface::menu::view::{self, MenuRow, RowEvent};
use iced_pro::token::space;

use crate::ui::theme::Palette;
use crate::ui_iced::message::{MenuMessage, Message};
use crate::ui_iced::overlay;

/// Card inset between the card edge and its rows (`space.xs`, §4.8.1).
pub const CARD_INSET: f32 = space::XS;

/// Menu card width in logical pixels.
///
/// §4.8.1 gives a menu card a **range** (`224px..320px`) rather than a
/// value, and `iced-pro` publishes no menu-card width token — the only
/// card-width constant in the framework is the Input Dialog's 320. So this is
/// the application's own choice, pinned at the low end of the spec range and
/// shared by every menu card the app draws. It is deliberately **not** a
/// restatement of a framework number, because no framework number exists.
pub const CARD_WIDTH: f32 = 224.0;

/// The card size §4.8.1 produces for `menu`: fixed width, height summed from
/// the framework's own per-kind row heights plus the card inset.
///
/// There is **no inter-row spacing**. The framework's row grid is contiguous,
/// with a separator row carrying its own 9 px.
///
/// Purity: pure function of `menu`.
#[must_use]
pub fn card_size(menu: &Menu) -> Size {
    let rows: f32 = menu.rows().iter().map(view::row_height).sum();
    Size::new(CARD_WIDTH, rows + CARD_INSET * 2.0)
}

/// The rectangle one row occupies inside an already-placed card.
///
/// Offset from the card's top-left by the inset plus the heights of every row
/// above it — the same contiguous grid [`card_size`] sums, which is what makes
/// a submenu land against the row rather than against the card.
///
/// Purity: pure function of `(menu, card, row)`.
#[must_use]
pub fn row_rect(menu: &Menu, card: Rectangle, row: usize) -> Option<Rectangle> {
    let target = menu.row(row)?;
    let offset: f32 = menu.rows()[..row].iter().map(view::row_height).sum();
    Some(Rectangle::new(
        Point::new(card.x + CARD_INSET, card.y + CARD_INSET + offset),
        Size::new(card.width - CARD_INSET * 2.0, view::row_height(target)),
    ))
}

/// Positions one card at its solved rectangle inside the full-window stack.
///
/// The overlay host mounts floating surfaces by padding a full-size container
/// out to the card's origin, which keeps the card in root-space coordinates
/// regardless of pane or scroll offsets.
pub fn positioned<'a>(card: Element<'a, Message>, rect: Rectangle) -> Element<'a, Message> {
    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: rect.y,
            right: 0.0,
            bottom: 0.0,
            left: rect.x,
        })
        .into()
}

/// Builds one open card: the §4.8.1 chrome around a column of [`MenuRow`]
/// widgets, mounted at the card's solved `rect`.
///
/// A submenu parent row is told whether its child is open so it paints the
/// Active-Submenu-Parent cell rather than a plain Hovered one. The rows
/// publish [`MenuMessage::RowPicked`] carrying the card path and row index;
/// the reducer runs
/// [`MenuCascade::activate_row`] and lets the
/// row's own [`ClosePolicy`](iced_pro::surface::menu::model::ClosePolicy)
/// decide whether the menu survives the click.
///
/// The row's two hover reports are mapped alongside them onto
/// [`MenuMessage::RowEntered`] / [`MenuMessage::RowLeft`], which is the
/// only route the submenu aiming corridor (`UIF-PRIN-008`) has into the
/// reducer. Note what that mapping does **not** do: neither report can
/// reach the click-away `dismiss` message, so no pointer movement closes
/// a card from here. Dismissal stays the blocker's job alone.
pub fn card_element<'a>(
    card: &OpenCard,
    cascade: &MenuCascade,
    rect: Rectangle,
) -> Element<'a, Message> {
    let menu = card.menu();
    let mut rows = Column::<Message>::new();

    for (index, row) in menu.rows().iter().enumerate() {
        let child_open = row
            .child_id()
            .is_some_and(|child| cascade.is_card_open(child));
        let card = menu.id().clone();
        let address = MenuMessage::RowPicked {
            card: card.clone(),
            row: index,
        };

        rows = rows.push(
            MenuRow::new(
                row.clone(),
                menu.row_focus_id(row),
                move |event| match event {
                    RowEvent::Activated => Message::Menu(address.clone()),
                    RowEvent::Absorbed => Message::Menu(MenuMessage::InteractionIgnored),
                    RowEvent::Hovered => Message::Menu(MenuMessage::RowEntered {
                        card: card.clone(),
                        row: index,
                    }),
                    RowEvent::Unhovered => {
                        Message::Menu(MenuMessage::RowLeft { card: card.clone() })
                    }
                },
            )
            .palette(Palette::pro())
            .submenu_open(child_open),
        );
    }

    positioned(
        container(rows)
            .width(Length::Fixed(CARD_WIDTH))
            .padding(CARD_INSET)
            .style(overlay::card_style)
            .into(),
        rect,
    )
}

/// Renders every open card of `cascade` above `base`, plus the click-away
/// blocker that dismisses the whole tree.
///
/// `rects` is the output of [`open_card_rects`] for the same cascade and
/// viewport, index-aligned with [`MenuCascade::open_cards`]. `dismiss` is the
/// message the blocker publishes when the user clicks outside every card.
pub fn overlay<'a>(
    base: Element<'a, Message>,
    cascade: &MenuCascade,
    rects: &[Rectangle],
    dismiss: Message,
) -> Element<'a, Message> {
    if !cascade.is_open() {
        return base;
    }

    let mut layers: Vec<Element<'a, Message>> = vec![base, overlay::input_blocker(dismiss).into()];
    layers.extend(
        cascade
            .open_cards()
            .iter()
            .zip(rects)
            .map(|(card, rect)| card_element(card, cascade, *rect)),
    );

    stack(layers)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Solves the viewport rectangle of every open card, root first.
///
/// A child card is anchored to the **row** that opened it, not to its parent
/// card (§4.8.1 / `UIF-FMENU-002`), so the parent's already-solved rectangle
/// is used to derive the trigger row's rectangle inside it. Cards whose parent
/// did not get a rectangle — which cannot happen for a cascade walked in open
/// order — are skipped rather than guessed at.
///
/// `root_placement` supplies the root card's placement, which is the one piece
/// of anchoring the two menu surfaces do differently.
///
/// These are the rectangles registered with the
/// [`SurfaceRegistry`](iced_pro::surface::zorder::SurfaceRegistry), which is
/// what makes a registered menu surface the card the user actually sees rather
/// than an empty region.
///
/// Purity: pure function of `(cascade, viewport, root_placement)`.
#[must_use]
pub fn open_card_rects(
    cascade: &MenuCascade,
    viewport: Size,
    root_placement: impl Fn(&Menu) -> Placement,
) -> Vec<Rectangle> {
    let view = Rectangle::new(Point::ORIGIN, viewport);
    let mut placed: HashMap<MenuId, Rectangle> = HashMap::new();
    let mut rects = Vec::new();

    for card in cascade.open_cards() {
        let rect = match card.trigger() {
            None => root_placement(card.menu()).rect(),
            Some(trigger) => {
                let Some(parent_rect) = placed.get(&trigger.parent).copied() else {
                    continue;
                };
                let Some(parent_menu) = cascade.card_menu(&trigger.parent) else {
                    continue;
                };
                let Some(row_rect) = row_rect(parent_menu, parent_rect, trigger.row) else {
                    continue;
                };
                anchor::solve(
                    anchor::Anchor::Control {
                        rect: row_rect,
                        alignment: anchor::ControlAlignment::submenu(),
                    },
                    card_size(card.menu()),
                    view,
                )
                .rect()
            }
        };

        placed.insert(card.id().clone(), rect);
        rects.push(rect);
    }

    rects
}
