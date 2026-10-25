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
//! The tree/tab **context** menu was converted onto the same engine in the
//! following increment (INT-3b), so both menu surfaces in the app now share
//! [`the shared menu-surface module`](crate::ui_iced::menu_surface)'s chrome
//! and geometry and
//! the hand-rolled flat-menu constants that used to back them are gone.
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

use iced::{Element, Point, Rectangle, Size};

use iced_pro::geometry::anchor::{self, Anchor, ControlAlignment, Placement};
use iced_pro::surface::menu::cascade::MenuCascade;
use iced_pro::surface::menu::model::{
    ActivationEffect, Menu, MenuId, RadioGroupId, Row, submenu_id,
};

use crate::bus::events::user_command::UserCommand;
use crate::markdown::table_width::DeficitStrategy;
use crate::ui::strings;
use crate::ui_iced::menu_surface;
use crate::ui_iced::message::{MenuMessage, Message};
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

/// Card inset, card width and the card-size sum are shared with the
/// context-menu surface through [`menu_surface`], and re-exported here so the
/// INT-3a public path stays intact.
pub use crate::ui_iced::menu_surface::{CARD_INSET, CARD_WIDTH, card_size};

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
/// Delegates the walk to
/// [`menu_surface::open_card_rects`],
/// supplying [`root_placement`] as the root anchor — the one thing the
/// hamburger does differently from the context-menu surface.
///
/// These are the rectangles registered with the
/// [`SurfaceRegistry`](iced_pro::surface::zorder::SurfaceRegistry), which
/// is what makes the hamburger's registered region the card the user
/// actually sees instead of an empty list.
///
/// Purity: pure function of `(cascade, viewport)`.
#[must_use]
pub fn open_card_rects(cascade: &MenuCascade, viewport: Size) -> Vec<Rectangle> {
    menu_surface::open_card_rects(cascade, viewport, |menu| root_placement(menu, viewport))
}

/// Renders every open card of `cascade` above `base`, plus the click-away
/// blocker that dismisses the whole tree.
///
/// Delegates to [`menu_surface::overlay`]
/// with the hamburger's own dismiss message. `rects` is the output of
/// [`open_card_rects`] for the same cascade and viewport, index-aligned with
/// [`MenuCascade::open_cards`](MenuCascade::open_cards) — the view does not
/// re-solve geometry, it paints the rectangles the reducer already
/// registered. Cards past the end of `rects` are not mounted.
pub fn overlay<'a>(
    base: Element<'a, Message>,
    cascade: &MenuCascade,
    rects: &[Rectangle],
) -> Element<'a, Message> {
    menu_surface::overlay(base, cascade, rects, Message::Menu(MenuMessage::Closed))
}
