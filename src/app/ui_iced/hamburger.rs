//! The top-bar hamburger menu rendered by the `iced-pro` menu engine.
//!
//! This module replaces the app-side rendering path for the ☰ menu
//! (integration increment INT-3a). The menu is now an
//! [`iced_pro::surface::menu::model::Menu`] value, its openness is owned by
//! [`iced_pro::surface::menu::cascade::MenuCascade`], its placement comes
//! from [`iced_pro::geometry::anchor::solve`], and each row is an
//! [`iced_pro::surface::menu::view::MenuRow`] — the first framework
//! *widget* this application renders rather than a framework token it reads.
//!
//! # What moved where
//!
//! | Concern | Owner |
//! |---|---|
//! | Row structure, kinds, selection | [`hamburger_menu`](crate::ui_iced::hamburger::hamburger_menu) building the `Menu` |
//! | Which card is open, keyboard position | `MenuCascade` on `OverlayState` |
//! | Where each card lands | `anchor::solve` in [`open_card_rects`](crate::ui_iced::hamburger::open_card_rects) |
//! | Row painting + hit testing | `MenuRow` |
//! | Row intent | [`command_for_activation`](crate::ui_iced::hamburger::command_for_activation) |
//!
//! The tree/tab **context** menu is deliberately untouched: this increment
//! converts one surface to prove the pattern, so
//! [`crate::ui_iced::menu`] still serves it and its hand-rolled constants
//! stay alive for that reason.
//!
//! # Row kinds are now real, not painted
//!
//! The previous path drew the checkmark *inside the label string*
//! (`"✓ Model name"`) for the chat-model and table-wrap choices, so a
//! single-choice group looked identical to an independent toggle. Here the
//! chat models and the table-wrap strategies are
//! [`RowKind::Radio`](iced_pro::surface::menu::model::RowKind::Radio)
//! members of a named
//! [`RadioGroupId`](iced_pro::surface::menu::model::RadioGroupId), and
//! Background operations / Agent debug are
//! [`RowKind::Check`](iced_pro::surface::menu::model::RowKind::Check) rows.
//! The leading slot glyph (`•` vs `✓`) is the framework's, and mutual
//! exclusion is enforced by
//! [`Menu::activate`](iced_pro::surface::menu::model::Menu::activate)
//! instead of by each row's command.
//!
//! # The trigger rectangle is synthesised, not measured
//!
//! The app has no widget-geometry reporting for the ☰ button, so
//! [`hamburger_trigger_rect`](crate::ui_iced::hamburger::hamburger_trigger_rect)
//! builds the navbar band truncated at the button's right inset. That is
//! the same approximation the previous synthetic anchor point encoded,
//! expressed as a real
//! [`Anchor::Control`](iced_pro::geometry::anchor::Anchor::Control)
//! rectangle so the framework's quadrant-flip and viewport-clamp rules
//! now apply to it.
//!
//! Unit tests live in the sibling `hamburger_tests.rs` sidecar.

#[cfg(test)]
#[path = "hamburger_tests.rs"]
mod tests;

use iced::widget::{Column, container, stack};
use iced::{Element, Length, Padding, Point, Rectangle, Size};
use std::collections::HashMap;

use iced_pro::geometry::anchor::{self, Anchor, ControlAlignment, Placement};
use iced_pro::surface::menu::cascade::{MenuCascade, OpenCard};
use iced_pro::surface::menu::model::{
    ActivationEffect, Menu, MenuId, RadioGroupId, Row, submenu_id,
};
use iced_pro::surface::menu::view::{self, MenuRow, RowEvent};
use iced_pro::token::space;

use crate::bus::events::user_command::UserCommand;
use crate::markdown::table_width::DeficitStrategy;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::menu::MENU_SURFACE_WIDTH;
use crate::ui_iced::message::{MenuMessage, Message};
use crate::ui_iced::overlay;
use crate::ui_iced::panels::top::{
    BAR_HEIGHT_POINTS, MENU_ANCHOR_RIGHT_MARGIN, TopBarViewModel, apply_about_button_click,
    apply_agent_debug_toggle, apply_background_logs_toggle, apply_batch_button_click,
    apply_table_width_strategy_change, apply_tools_button_click, strategy_menu_order,
};

/// Root [`MenuId`] of the hamburger tree.
///
/// Every submenu id below is derived from this path by
/// [`submenu_id`], so the
/// whole tree's identity — and therefore every focus id inside it — is
/// fixed by this one string plus the row keys.
pub const HAMBURGER_MENU_ID: &str = "hamburger";

/// Radio group holding the chat-model choices (UI-065).
pub const RADIO_GROUP_CHAT_MODEL: &str = "chat_model";

/// Radio group holding the table-wrap strategy choices (UI-066).
pub const RADIO_GROUP_TABLE_WRAP: &str = "table_wrap";

/// Key prefix carrying a chat model's name on its radio row.
pub const CHAT_MODEL_KEY_PREFIX: &str = "model:";

/// Key prefix carrying a [`DeficitStrategy`] on its radio row.
pub const STRATEGY_KEY_PREFIX: &str = "strategy:";

/// Row key: open the batch prompt-processing dialog.
pub const KEY_BATCH: &str = "batch";
/// Row key: open the tools and integrations dialog.
pub const KEY_TOOLS: &str = "tools";
/// Row key: the "Windows..." submenu parent.
pub const KEY_WINDOWS: &str = "windows";
/// Row key: the "Chat models" submenu parent.
pub const KEY_CHAT_MODELS: &str = "chat_models";
/// Row key: the "Table wrap algorithm" submenu parent.
pub const KEY_TABLE_WRAP: &str = "table_wrap";
/// Row key: open the About dialog.
pub const KEY_ABOUT: &str = "about";
/// Row key: the Background operations checkbox.
pub const KEY_BACKGROUND_OPERATIONS: &str = "background_operations";
/// Row key: the Agent debug checkbox.
pub const KEY_AGENT_DEBUG: &str = "agent_debug";
/// Row key: the "no models configured" placeholder.
pub const KEY_NO_MODELS: &str = "no_models";

/// Card inset between the card edge and its rows (`space.xs`, §4.8.1).
///
/// This replaces the previous hand-rolled `MENU_SURFACE_PADDING` of 6 px.
/// The framework states the inset as a token, so the card is 2 px tighter
/// on every side than the surface it replaced.
pub const CARD_INSET: f32 = space::XS;

/// Card width, reusing the app's existing menu width.
///
/// §4.8.1 gives the menu card a **range** (`224px..320px`) rather than a
/// value, and `iced-pro` publishes no menu-card width token — the only
/// card-width constant in the framework is the Input Dialog's 320. So the
/// app's existing 224 (the low end of the range, and what it rendered
/// before) is carried over unchanged rather than restated here.
pub const CARD_WIDTH: f32 = MENU_SURFACE_WIDTH;

/// Builds the hamburger menu as a framework [`Menu`] tree.
///
/// Top-level rows are the three dialog actions and three submenu parents;
/// each submenu carries its own rows. The chat-model and table-wrap
/// children are radio rows in named groups, the two window toggles are
/// checkable rows, and the empty-chat-models case is a disabled
/// placeholder so the card never renders as a blank surface.
///
/// Purity: pure function of the view model.
#[must_use]
pub fn hamburger_menu(view_model: &TopBarViewModel) -> Menu {
    Menu::new(
        HAMBURGER_MENU_ID,
        vec![
            Row::action(KEY_BATCH, strings::MENU_BATCH),
            Row::action(KEY_TOOLS, strings::MENU_TOOLS),
            Row::submenu(KEY_WINDOWS, strings::MENU_WINDOWS, windows_menu()),
            Row::submenu(
                KEY_CHAT_MODELS,
                strings::MENU_CHAT_MODELS,
                chat_models_menu(view_model),
            ),
            Row::submenu(
                KEY_TABLE_WRAP,
                strings::MENU_TABLE_WRAP_ALGORITHM,
                table_wrap_menu(view_model),
            ),
            Row::action(KEY_ABOUT, strings::MENU_ABOUT),
        ],
    )
}

/// The "Windows" submenu: two independent on/off windows.
///
/// [`ClosePolicy`](iced_pro::surface::menu::model::ClosePolicy) for a
/// [`RowKind::Check`] is `StayOpen`, which is what both of these want —
/// the user flips one and then the other in a single visit.
fn windows_menu() -> Menu {
    Menu::new(
        submenu_path(KEY_WINDOWS),
        vec![
            Row::checkable(
                KEY_BACKGROUND_OPERATIONS,
                strings::MENU_BACKGROUND_OPERATIONS,
                false,
            ),
            Row::checkable(KEY_AGENT_DEBUG, strings::MENU_AGENT_DEBUG, false),
        ],
    )
}

/// The "Chat models" submenu (UI-064, UI-065).
///
/// With no models configured the card holds a single placeholder row so
/// the surface still paints. The placeholder is **enabled** and
/// [`stays_open`](Row::stays_open), which is forced by the cascade rather
/// than chosen:
/// [`MenuCascade::stabilise`](MenuCascade::stabilise) closes any open card
/// the keyboard cannot enter, so a card whose only row is disabled cannot
/// stay open at all — the submenu would appear to do nothing when clicked.
/// Being enabled makes it navigable; resolving to no
/// [`UserCommand`](crate::bus::events::user_command::UserCommand) makes it
/// inert, which is what
/// `test_hamburger_rows_emit_their_commands` pins for
/// [`KEY_NO_MODELS`].
fn chat_models_menu(view_model: &TopBarViewModel) -> Menu {
    let rows = if view_model.chat_models.is_empty() {
        vec![Row::action(KEY_NO_MODELS, strings::NO_CHAT_MODELS_CONFIGURED).stays_open()]
    } else {
        view_model
            .chat_models
            .iter()
            .map(|model| {
                let selected =
                    view_model.current_chat_model.as_deref() == Some(model.name.as_str());
                Row::radio(
                    format!("{CHAT_MODEL_KEY_PREFIX}{}", model.name),
                    model.label(),
                    RadioGroupId::new(RADIO_GROUP_CHAT_MODEL),
                    selected,
                )
            })
            .collect()
    };
    Menu::new(submenu_path(KEY_CHAT_MODELS), rows)
}

/// The "Table wrap algorithm" submenu (UI-066).
fn table_wrap_menu(view_model: &TopBarViewModel) -> Menu {
    let rows = strategy_menu_order()
        .iter()
        .map(|variant| {
            let selected = *variant == view_model.current_strategy;
            Row::radio(
                strategy_key(*variant),
                crate::ui_iced::panels::top::strategy_label(*variant),
                RadioGroupId::new(RADIO_GROUP_TABLE_WRAP),
                selected,
            )
        })
        .collect();
    Menu::new(submenu_path(KEY_TABLE_WRAP), rows)
}

/// The path-derived id of the hamburger submenu opened by row `key`.
///
/// Spelled through `submenu_id` rather than written out as a literal so
/// the id a card carries and the key that opened it cannot drift apart.
/// `Menu::new` on the parent re-derives the same value on attachment, so
/// this is the identity the tree actually holds.
fn submenu_path(key: &str) -> MenuId {
    submenu_id(&MenuId::new(HAMBURGER_MENU_ID), key)
}

/// The stable radio-row key for one strategy.
///
/// Derived from the variant's own name so a new
/// [`DeficitStrategy`] needs no key table, and so
/// [`strategy_for_key`] can invert it without a second mapping.
#[must_use]
pub fn strategy_key(strategy: DeficitStrategy) -> String {
    format!("{STRATEGY_KEY_PREFIX}{}", strategy_name(strategy))
}

/// Resolves a [`strategy_key`] back to its variant.
///
/// Scans [`strategy_menu_order`] rather than matching names by hand, so
/// the two directions cannot drift apart from each other — only from a
/// strategy that was never added to the menu order at all.
#[must_use]
pub fn strategy_for_key(key: &str) -> Option<DeficitStrategy> {
    let name = key.strip_prefix(STRATEGY_KEY_PREFIX)?;
    strategy_menu_order()
        .into_iter()
        .find(|variant| strategy_name(*variant) == name)
}

/// The variant name carried inside a [`STRATEGY_KEY_PREFIX`] key.
fn strategy_name(strategy: DeficitStrategy) -> &'static str {
    match strategy {
        DeficitStrategy::HybridMinPenaltyWaterFill => "HybridMinPenaltyWaterFill",
        DeficitStrategy::ProportionalToSlack => "ProportionalToSlack",
        DeficitStrategy::BreakpointWaterFill => "BreakpointWaterFill",
        DeficitStrategy::WaterFillRatio => "WaterFillRatio",
        DeficitStrategy::LagrangePenalty => "LagrangePenalty",
    }
}

/// Resolves an activation to the self-contained [`UserCommand`] it publishes.
///
/// Dispatch is on the row **key** alone. Keys are unique across the whole
/// hamburger tree by construction (asserted by
/// `test_row_keys_are_unique_across_the_tree`), which is what lets the
/// resolver ignore which card the row sat in — the card says where the row
/// was, not what it does.
///
/// A [`ActivationEffect::Toggled`] carries the new value the cascade
/// already committed to its model copy, so the command is built from that
/// rather than re-reading the view model, which is still stale at this
/// point in the reduction.
#[must_use]
pub fn command_for_activation(key: &str, effect: &ActivationEffect) -> Option<UserCommand> {
    match key {
        KEY_BATCH => Some(apply_batch_button_click()),
        KEY_TOOLS => Some(apply_tools_button_click()),
        KEY_ABOUT => Some(apply_about_button_click()),
        KEY_BACKGROUND_OPERATIONS => toggled(effect).map(apply_background_logs_toggle),
        KEY_AGENT_DEBUG => toggled(effect).map(apply_agent_debug_toggle),
        other => {
            if let Some(name) = other.strip_prefix(CHAT_MODEL_KEY_PREFIX) {
                Some(UserCommand::SelectChatModel(name.to_owned()))
            } else {
                strategy_for_key(other).map(apply_table_width_strategy_change)
            }
        }
    }
}

/// The new value of a [`ActivationEffect::Toggled`], `None` for any other effect.
fn toggled(effect: &ActivationEffect) -> Option<bool> {
    match effect {
        ActivationEffect::Toggled { checked } => Some(*checked),
        _ => None,
    }
}

/// The synthesised navbar rectangle the hamburger card anchors to.
///
/// The ☰ button's right edge sits `MENU_ANCHOR_RIGHT_MARGIN` inside the
/// window's right edge and the bar is [`BAR_HEIGHT_POINTS`] tall, so the
/// card's `below_end()` placement lands exactly where the previous
/// hand-computed anchor point put it. See the
/// [module docs](self#the-trigger-rectangle-is-synthesised-not-measured)
/// for why this is a synthesised rectangle rather than a measured one.
///
/// Purity: pure function of `viewport`.
#[must_use]
pub fn hamburger_trigger_rect(viewport: Size) -> Rectangle {
    let width = (viewport.width - MENU_ANCHOR_RIGHT_MARGIN).max(0.0);
    Rectangle::new(Point::ORIGIN, Size::new(width, BAR_HEIGHT_POINTS))
}

/// The card size §4.8.1 produces for `menu`: fixed width, height summed
/// from the framework's own per-kind row heights plus the card inset.
///
/// There is **no inter-row spacing** here. The previous surface added 1 px
/// between every row; the framework's row grid is contiguous, with a
/// separator row carrying its own 9 px.
///
/// Purity: pure function of `menu`.
#[must_use]
pub fn card_size(menu: &Menu) -> Size {
    let rows: f32 = menu.rows().iter().map(view::row_height).sum();
    Size::new(CARD_WIDTH, rows + CARD_INSET * 2.0)
}

/// Solves the root card's placement under the navbar.
///
/// Purity: pure function of `(menu, viewport)`.
#[must_use]
pub fn root_placement(menu: &Menu, viewport: Size) -> Placement {
    anchor::solve(
        Anchor::Control {
            rect: hamburger_trigger_rect(viewport),
            alignment: ControlAlignment::below_end(),
        },
        card_size(menu),
        Rectangle::new(Point::ORIGIN, viewport),
    )
}

/// Solves the viewport rectangle of every open card, root first.
///
/// A child card is anchored to the **row** that opened it, not to its
/// parent card (§4.8.1 / `UIF-FMENU-002`), so the parent's already-solved
/// rectangle is used to derive the trigger row's rectangle inside it.
/// Cards whose parent did not get a rectangle — which cannot happen for a
/// cascade walked in open order — are skipped rather than guessed at.
///
/// These are the rectangles registered with the
/// [`SurfaceRegistry`](iced_pro::surface::zorder::SurfaceRegistry), which
/// is what makes the hamburger's registered region the card the user
/// actually sees instead of an empty list.
///
/// Purity: pure function of `(cascade, viewport)`.
#[must_use]
pub fn open_card_rects(cascade: &MenuCascade, viewport: Size) -> Vec<Rectangle> {
    let view = Rectangle::new(Point::ORIGIN, viewport);
    let mut placed: HashMap<MenuId, Rectangle> = HashMap::new();
    let mut rects = Vec::new();

    for card in cascade.open_cards() {
        let rect = match card.trigger() {
            None => root_placement(card.menu(), viewport).rect(),
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
                    Anchor::Control {
                        rect: row_rect,
                        alignment: ControlAlignment::submenu(),
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

/// The rectangle one row occupies inside an already-placed card.
///
/// Offset from the card's top-left by the inset plus the heights of every
/// row above it — the same contiguous grid [`card_size`] sums, which is
/// what makes the submenu land against the row rather than the card.
fn row_rect(menu: &Menu, card: Rectangle, row: usize) -> Option<Rectangle> {
    let target = menu.row(row)?;
    let offset: f32 = menu.rows()[..row].iter().map(view::row_height).sum();
    Some(Rectangle::new(
        Point::new(card.x + CARD_INSET, card.y + CARD_INSET + offset),
        Size::new(card.width - CARD_INSET * 2.0, view::row_height(target)),
    ))
}

/// Renders every open card of `cascade` above `base`, plus the click-away
/// blocker that dismisses the whole tree.
///
/// `rects` is the output of [`open_card_rects`] for the same cascade and
/// viewport, index-aligned with
/// [`MenuCascade::open_cards`](MenuCascade::open_cards) — the view does
/// not re-solve geometry, it paints the rectangles the reducer already
/// registered. Cards past the end of `rects` are not mounted.
///
/// Each card is a [`MenuRow`] per model row inside the §4.8.1 card chrome.
/// A submenu parent row is told whether its child is open so it paints the
/// Active-Submenu-Parent cell rather than a plain Hovered one.
///
/// The rows publish [`MenuMessage::RowPicked`] carrying the card path and
/// row index; the reducer runs
/// [`MenuCascade::activate_row`](MenuCascade::activate_row) and lets the
/// row's own close policy decide whether the menu survives the click.
pub fn overlay<'a>(
    base: Element<'a, Message>,
    cascade: &MenuCascade,
    rects: &[Rectangle],
) -> Element<'a, Message> {
    if !cascade.is_open() {
        return base;
    }

    let mut layers: Vec<Element<'a, Message>> = vec![
        base,
        overlay::input_blocker(Message::Menu(MenuMessage::Closed)).into(),
    ];
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

/// Positions one card at its solved rectangle inside the full-window stack.
///
/// The overlay host mounts floating surfaces by padding a full-size
/// container out to the card's origin, which keeps the card in root-space
/// coordinates regardless of pane or scroll offsets.
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

/// Builds one open card: the §4.8.1 chrome around a column of
/// [`MenuRow`] widgets, mounted at the card's solved `rect`.
fn card_element<'a>(
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
        let address = MenuMessage::RowPicked {
            card: menu.id().clone(),
            row: index,
        };

        rows = rows.push(
            MenuRow::new(
                row.clone(),
                menu.row_focus_id(row),
                move |event| match event {
                    RowEvent::Activated => Message::Menu(address.clone()),
                    RowEvent::Absorbed => Message::Menu(MenuMessage::InteractionIgnored),
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
