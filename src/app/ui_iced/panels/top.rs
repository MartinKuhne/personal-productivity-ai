//! Top toolbar panel (UI-061..UI-066).
//!
//! The panel is a **pure view**: it
//! renders a [`TopBarViewModel`] snapshot into an
//! [`iced::Element`] of [`Message`] and emits
//! [`Message::Command`] for every user action. It never touches the
//! `UserCommand` bus itself — the reducer publishes
//! what the panel emits, and each intent is self-contained and replayable.
//!
//! # Shape
//!
//! ```text
//! [logo] FastMD Viewer | <indexing status> [tag filter]        [☰ menu]
//! ```
//!
//! * **Logo** — the shared stand-in built by
//!   [`crate::ui_iced::logo::logo_mark`] at [`LOGO_MARK_SIZE`]. See
//!   *Feature-gated widgets* below and the trade-off table in
//!   [`crate::ui_iced::logo`].
//! * **Indexing status** — [`indexing_status_text`] plus the `success`
//!   palette token once indexing has finished.
//! * **Tag filter** — an [`iced::widget::pick_list()`] over
//!   [`TagFilterOption`]; emits [`UserCommand::SelectTagFilter`].
//! * **Hamburger** — a literal ☰ [`iced::widget::button()`] labelled with
//!   [`strings::HAMBURGER_MENU_BUTTON`], emitting
//!   [`Message::Menu`] [`MenuMessage::Opened`];
//!   the reducer flips
//!   [`crate::ui_iced::app::OverlayState::hamburger_open`] and the
//!   overlay host mounts the shared menu widget anchored at
//!   [`hamburger_menu_anchor`]. A button that opens a real menu is what
//!   UI-061..UI-066 needs: a [`pick_list()`] would be a *dropdown*, not a
//!   menu — it has no open/closed state, its closed title bar leaks the
//!   last-picked row, and it can render only flattened selectable rows
//!   (no section headers, and no "no models configured" placeholder — see
//!   *Nested submenus* below).
//!
//! # Nested submenus (UI-062/UI-064/UI-066)
//!
//! iced 0.14's built-in dropdowns are **single-level**:
//! `overlay::menu::Menu` renders each option as one plain
//! [`ToString::to_string`] row, has no separators, no disabled rows and no
//! child menus, and `iced_aw` is not a dependency. The full hierarchy is
//! therefore kept in the *pure model* — [`build_menu_entries`] returns
//! every row with its [`MenuSection`], [`MenuEntryKind`] and checked state.
//! [`hamburger_menu_items`] projects that model into nested reusable menu
//! rows, keeping parent surfaces mounted as a child opens and preserving
//! each selectable row's [`hamburger_row_message`].
//!
//! # Feature-gated widgets
//!
//! * **Logo** — `iced::widget::image` needs the `image` feature and
//!   `iced::widget::svg` needs `svg`; neither is enabled in `Cargo.toml`
//!   (iced is built with `wgpu`, `tiny-skia`, `fira-sans`,
//!   `advanced-shaping` and `markdown` only). A `text` mark is used
//!   instead, single-sourced through [`crate::ui_iced::logo`], which also
//!   records the raster / SVG / `canvas` cost of each option.
//! * **Spinner** — iced 0.14 has **no** spinner widget at all (there is no
//!   `iced::widget::spin`, and `iced_widget-0.14.2` contains no spinner
//!   implementation). An indeterminate indicator would need `progress_bar`
//!   driven by a timer subscription, which is runtime wiring outside this
//!   panel. The panel renders the status text only; the "indexing in
//!   progress" state stays legible through the italic wording and the
//!   [`strings::build_indexing_progress_text`] ellipsis.
//!
//! # Relocated helpers
//!
//! The bar's ten framework-free helpers ([`get_tag_filter_text`],
//! [`compute_next_selected_file`], [`apply_batch_button_click`],
//! [`apply_tools_button_click`], [`apply_about_button_click`],
//! [`apply_background_logs_toggle`], [`apply_agent_debug_toggle`],
//! [`apply_chat_model_selection`], [`apply_table_width_strategy_change`],
//! [`strategy_label`]) are **owned by this module**. Each is a pure
//! `UserCommand` constructor or a pure path/tag/label solver with **no
//! framework type** in its signature or body, so the panel and the reducer
//! call the same code.
//!
//! `build_indexing_status_text` is **not** one of them — it returns an
//! `RichText` and cannot be relocated framework-free. This module's own
//! [`indexing_status_text`] is the iced equivalent.
//!
//! Unit tests live in the sibling `top_tests.rs` sidecar.

#[cfg(test)]
#[path = "top_tests.rs"]
mod tests;

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use iced::widget::{Row, button, container, pick_list, rule, text};
use iced::{Alignment, Color, Element, Font, Length, Padding, Point, Size};

use crate::bus::events::user_command::UserCommand;
use crate::markdown::table_width::DeficitStrategy;
use crate::ui::app::FastMdApp;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::fonts::PHOSPHOR_FONT_NAME;
use crate::ui_iced::logo::logo_mark;
use crate::ui_iced::menu::{MenuItem as WidgetMenuItem, MenuItemId};
use crate::ui_iced::message::{MenuMessage, Message};

// The framework-free helpers this panel owns, **relocated into this
// module**. Each is framework-free — pure `UserCommand` constructors and
// pure path/tag/label solvers with no framework type in any signature or
// body — so the panel and the reducer call the same code.
//
// `build_indexing_status_text` is deliberately **not** here: it returns an
// `RichText`, so it cannot be relocated framework-free. This module
// owns its own [`indexing_status_text`](crate::ui_iced::panels::top::indexing_status_text)
// instead.

/// Purpose: Determines the display text for the tag filter combobox.
/// Inputs: selected_tag (optional string reference)
/// Outputs: String slice representing the text to show.
/// Purity: Pure function.
/// Preconditions: None.
/// Postconditions: Returns the tag name if one is selected, otherwise "Filter by Tag: All".
#[must_use]
pub fn get_tag_filter_text(selected_tag: Option<&String>) -> &str {
    selected_tag
        .map(|s| s.as_str())
        .unwrap_or(strings::TAG_FILTER_DEFAULT)
}

/// Purpose: Determines the next selected file after the active tag filter changes.
/// Inputs: selected_file (current selected file path), selected_tag (currently active tag filter), file_tags (mapping of files to their tags)
/// Outputs: `Option<PathBuf>` representing the new selected file.
/// Purity: Pure function.
/// Preconditions: None.
/// Postconditions: Returns `None` if an active tag is selected and it is not associated with the selected file. Otherwise returns the original selected file.
#[must_use]
pub fn compute_next_selected_file(
    selected_file: Option<&PathBuf>,
    selected_tag: Option<&String>,
    file_tags: &BTreeMap<PathBuf, Vec<String>>,
) -> Option<PathBuf> {
    let selected = selected_file?;
    if let Some(active_tag) = selected_tag {
        {
            let tags = file_tags.get(selected)?;
            if !tags.contains(active_tag) {
                return None;
            }
        }
    }
    Some(selected.clone())
}

/// Returns the command for clicking the batch-processing button.
#[must_use]
pub fn apply_batch_button_click() -> UserCommand {
    UserCommand::OpenBatchDialog
}

/// Returns the command for clicking the "Tools..." button.
#[must_use]
pub fn apply_tools_button_click() -> UserCommand {
    UserCommand::OpenToolsDialog
}

/// Returns the command for clicking the "About FastMD..." button.
#[must_use]
pub fn apply_about_button_click() -> UserCommand {
    UserCommand::OpenAboutDialog
}

/// Returns the command for toggling the background logs window.
#[must_use]
pub fn apply_background_logs_toggle(show: bool) -> UserCommand {
    UserCommand::ToggleBackgroundLogs(show)
}

/// Returns the command for toggling the agent debug window.
#[must_use]
pub fn apply_agent_debug_toggle(show: bool) -> UserCommand {
    UserCommand::ToggleAgentDebugWindow(show)
}

/// Returns the command for picking a new chat model.
#[must_use]
pub fn apply_chat_model_selection(model_name: String) -> UserCommand {
    UserCommand::SelectChatModel(model_name)
}

/// Returns the command for changing the table-width deficit strategy.
#[must_use]
pub fn apply_table_width_strategy_change(strategy: DeficitStrategy) -> UserCommand {
    UserCommand::ChangeTableWidthStrategy(strategy)
}

/// Human-readable label for a [`DeficitStrategy`] variant, used as the
/// row label inside the top-bar Table wrap algorithm submenu. The match is
/// intentionally
/// exhaustive over every variant of `DeficitStrategy` so adding a new
/// strategy in the future is a compile error here until the label is
/// supplied.
#[must_use]
pub fn strategy_label(strategy: DeficitStrategy) -> &'static str {
    match strategy {
        DeficitStrategy::ProportionalToSlack => strings::TABLE_WIDTH_STRATEGY_PROPORTIONAL,
        DeficitStrategy::BreakpointWaterFill => strings::TABLE_WIDTH_STRATEGY_WATERFILL,
        DeficitStrategy::WaterFillRatio => strings::TABLE_WIDTH_STRATEGY_RATIO,
        DeficitStrategy::LagrangePenalty => strings::TABLE_WIDTH_STRATEGY_LAGRANGE,
        DeficitStrategy::HybridMinPenaltyWaterFill => strings::TABLE_WIDTH_STRATEGY_HYBRID,
    }
}

/// `LlmConfig::has_use_case` key selecting the chat-capable models
/// (UI-064). Not user-facing text, but named so the literal appears once
/// ([RUST-021]). Made `pub` so the agent panel's model selector
/// ([`crate::ui_iced::agent`]) filters on the exact same key.
pub const CHAT_USE_CASE: &str = "chat";

/// Logical point size of the logo mark — the shared constant owned by
/// [`crate::ui_iced::logo`], re-exported here so the top-bar public
/// surface includes it ([RUST-051]).
pub use crate::ui_iced::logo::LOGO_MARK_SIZE;

/// Logical point size of the app-title heading.
pub const TITLE_SIZE: f32 = 18.0;

/// Horizontal spacing between top-bar items.
pub const BAR_SPACING: f32 = 8.0;

/// Inner padding of the top bar; small so the fixed [`BAR_HEIGHT_POINTS`]
/// band still fits the pick lists' content height ([`PICK_LIST_PADDING`])
/// and the 20 pt logo mark.
pub const BAR_PADDING: f32 = 2.0;

/// Full-on-screen height of the **top bar band**, in logical points — the
/// 28 pt band contract this bar is laid out against.
///
/// The bar must stay this compact; without the fixed height
/// the two pick lists' default button padding (5 pt vertical each)
/// grew the band to ~40 pt and the bar rendered far taller than the
/// layout expects.
pub const BAR_HEIGHT_POINTS: f32 = 28.0;

/// Vertical padding of the two top-bar pick lists.
///
/// iced's `pick_list` inherits `button::DEFAULT_PADDING` (5 pt vertical)
/// when unset, which — combined with [`BAR_PADDING`] — pushed the bar's
/// natural height well past [`BAR_HEIGHT_POINTS`]. Tightening to 2 pt
/// vertical (horizontal hit-target unchanged at 10 pt) fits both
/// widgets inside the fixed band (pick_list layout contract:
/// `iced_widget-0.14.2/src/pick_list.rs:421-432` — height = padding +
/// text line height).
pub const PICK_LIST_PADDING: Padding = Padding {
    top: 2.0,
    bottom: 2.0,
    left: 10.0,
    right: 10.0,
};

/// Width of the vertical rule drawn between top-bar groups.
pub const SEPARATOR_WIDTH: f32 = 1.0;

/// Minimum width of the hamburger menu surface, in logical points. The
/// reusable menu's default width
/// ([`crate::ui_iced::menu::MENU_SURFACE_WIDTH`]) exceeds this bound.
pub const MENU_MIN_WIDTH: f32 = 180.0;

/// Height of the ☰ glyph the hamburger button renders.
///
/// The requested Unicode glyph uses the regular text font, at the same size
/// the tag-filter pick list's text renders; button padding keeps both controls
/// the same height inside the fixed bar band.
pub const MENU_BUTTON_GLYPH_SIZE: f32 = 16.0;

/// Right inset between the hamburger menu surface and the window's right edge;
/// the menu opens out from the right-aligned ☰ button.
pub const MENU_ANCHOR_RIGHT_MARGIN: f32 = 12.0;

/// Gap between the top bar's bottom edge and the hamburger menu surface.
pub const MENU_ANCHOR_BELOW_BAR: f32 = 4.0;

/// Root-space point beside the right-aligned hamburger button.
///
/// The shared menu widget opens to the left and below this point, using the
/// configured anchor gap and viewport clamping. Returning the button-side
/// edge rather than a precomputed card origin keeps placement independent of
/// the widget's surface width.
///
/// Purity: pure function of `viewport`.
#[must_use]
pub fn hamburger_menu_anchor(viewport: Size) -> Point {
    Point::new(
        (viewport.width - MENU_ANCHOR_RIGHT_MARGIN + MENU_ANCHOR_BELOW_BAR).max(0.0),
        BAR_HEIGHT_POINTS,
    )
}

/// The [`Message`] a selectable hamburger menu row publishes.
///
/// The row's own [`UserCommand`] is the self-contained intent; it rides the
/// menu channel so the reducer can close the menu in the same reduction
/// ([`MenuMessage::RowActivated`]).
///
/// Purity: pure function of `command`.
#[must_use]
pub fn hamburger_row_message(command: &UserCommand) -> Message {
    Message::Menu(MenuMessage::RowActivated(command.clone()))
}

/// The label a command-bearing hamburger row renders.
///
/// Menu-card rows sit *below* their submenu title, so they carry no submenu
/// prefix — the check slot (`✓`, blank when unchecked) plus the row label is
/// the full text.
///
/// Purity: pure function of `(label, checked)`.
#[must_use]
pub fn hamburger_row_label(label: &str, checked: bool) -> String {
    strings::format_menu_row_label(None, label, checked)
}

/// The Phosphor icon font handle used for the logo mark.
///
/// The family is registered by [`crate::ui_iced::fonts`] at boot;
/// `PHOSPHOR_FONT_NAME` is the shared registration key.
pub fn phosphor_font() -> Font {
    Font::with_name(PHOSPHOR_FONT_NAME)
}

/// Builds the indexing status label shown in the top bar.
///
/// The status is a plain `String` plus the
/// [`indexing_status_color`] / [`indexing_status_font`] token lookups, so no
/// framework-specific type crosses into this layer.
///
/// Purity: pure function of `(indexing_finished, file_count)`.
pub fn indexing_status_text(indexing_finished: bool, file_count: usize) -> String {
    if indexing_finished {
        strings::build_indexing_finished_text(file_count)
    } else {
        strings::build_indexing_progress_text(file_count)
    }
}

/// Palette token for the indexing status label.
///
/// The `success` token once indexing has finished, the default body `text`
/// token while it runs. No numeric colour is introduced here.
///
/// Purity: pure function of `indexing_finished`.
pub fn indexing_status_color(indexing_finished: bool) -> Color {
    let palette = Palette::fastmd_dark();
    if indexing_finished {
        palette.success
    } else {
        palette.text
    }
}

/// Font for the indexing status label.
///
/// Italic while indexing, upright once
/// finished. iced 0.14's `Text` has no `.italic()` helper, so the style is
/// carried on the [`Font`] struct.
///
/// Purity: pure function of `indexing_finished`.
pub fn indexing_status_font(indexing_finished: bool) -> Font {
    if indexing_finished {
        Font::DEFAULT
    } else {
        Font {
            style: iced::font::Style::Italic,
            ..Font::DEFAULT
        }
    }
}

/// One option in the tag-filter pick list (UI-006's tag filter).
///
/// The `tag` field — not the label — is what the command carries, so a tag
/// that happens to be spelled `"All"` can never be confused with the
/// "show everything" entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagFilterOption {
    /// Label rendered on the button and in the dropdown.
    pub label: String,
    /// The tag this option selects; `None` is the "All" entry.
    pub tag: Option<String>,
}

impl TagFilterOption {
    /// The "show every file" entry ([`strings::TAG_FILTER_ALL`]).
    pub fn all() -> Self {
        Self {
            label: strings::TAG_FILTER_ALL.to_string(),
            tag: None,
        }
    }

    /// A single-tag entry labelled with the tag itself.
    pub fn for_tag(tag: &str) -> Self {
        Self {
            label: tag.to_string(),
            tag: Some(tag.to_string()),
        }
    }

    /// The intent this option publishes when picked.
    pub fn command(&self) -> UserCommand {
        UserCommand::SelectTagFilter(self.tag.clone())
    }
}

impl fmt::Display for TagFilterOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// One configured chat model as listed in the Chat models submenu
/// (UI-064).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatModelEntry {
    /// Config key / display name of the model.
    pub name: String,
    /// Cost figure reported by `LlmConfig::get_cost`.
    pub cost: i32,
}

impl ChatModelEntry {
    /// The `<name> (Cost: <cost>)` label UI-064 pins.
    pub fn label(&self) -> String {
        strings::format_chat_model_menu_label(&self.name, self.cost)
    }

    /// The intent this model publishes when picked (UI-065).
    pub fn command(&self) -> UserCommand {
        UserCommand::SelectChatModel(self.name.clone())
    }
}

/// `ChatModelEntry` renders as its [`Self::label`]. Added so the entry can
/// drive an `iced::widget::pick_list` in the agent prompt row
/// ([`crate::ui_iced::agent::prompt_input`]), which requires
/// `T: ToString` (`iced_widget-0.14.2/src/helpers.rs:1662`).
impl std::fmt::Display for ChatModelEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label())
    }
}

/// The named submenu a hamburger row belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuSection {
    /// A top-level row that is not inside a named submenu.
    Direct,
    /// The "Windows" submenu (UI-062, UI-063).
    Windows,
    /// The "Chat models" submenu (UI-064, UI-065).
    ChatModels,
    /// The "Table wrap algorithm" submenu (UI-066).
    TableWrap,
}

const MENU_WINDOWS_ID: MenuItemId = MenuItemId::new("hamburger.windows");
const MENU_CHAT_MODELS_ID: MenuItemId = MenuItemId::new("hamburger.chat_models");
const MENU_TABLE_WRAP_ID: MenuItemId = MenuItemId::new("hamburger.table_wrap");
const MENU_BACKGROUND_OPERATIONS_ID: MenuItemId =
    MenuItemId::new("hamburger.background_operations");
const MENU_AGENT_DEBUG_ID: MenuItemId = MenuItemId::new("hamburger.agent_debug");

impl MenuSection {
    /// The submenu name rendered in front of the row label, `None` for
    /// top-level rows.
    pub fn title(self) -> Option<&'static str> {
        match self {
            MenuSection::Direct => None,
            MenuSection::Windows => Some(strings::MENU_WINDOWS_TITLE),
            MenuSection::ChatModels => Some(strings::MENU_CHAT_MODELS),
            MenuSection::TableWrap => Some(strings::MENU_TABLE_WRAP_ALGORITHM),
        }
    }

    /// Stable widget identifier for this section, or `None` for direct rows.
    #[must_use]
    pub const fn menu_item_id(self) -> Option<MenuItemId> {
        match self {
            MenuSection::Direct => None,
            MenuSection::Windows => Some(MENU_WINDOWS_ID),
            MenuSection::ChatModels => Some(MENU_CHAT_MODELS_ID),
            MenuSection::TableWrap => Some(MENU_TABLE_WRAP_ID),
        }
    }

    /// Resolves a stable widget identifier to its section.
    #[must_use]
    pub fn from_menu_item_id(id: MenuItemId) -> Option<Self> {
        if id == MENU_WINDOWS_ID {
            Some(Self::Windows)
        } else if id == MENU_CHAT_MODELS_ID {
            Some(Self::ChatModels)
        } else if id == MENU_TABLE_WRAP_ID {
            Some(Self::TableWrap)
        } else {
            None
        }
    }
}

/// What selecting a hamburger menu row does.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuEntryKind {
    /// Selecting the row publishes the wrapped intent.
    Action(UserCommand),
    /// Selecting the row opens the named submenu.
    Submenu(MenuSection),
    /// A non-selectable informational row — UI-064's "no models
    /// configured" placeholder.
    Placeholder,
}

impl MenuEntryKind {
    /// The intent this row publishes, or `None` for navigation/placeholder rows.
    pub fn command(&self) -> Option<&UserCommand> {
        match self {
            MenuEntryKind::Action(command) => Some(command),
            MenuEntryKind::Submenu(_) | MenuEntryKind::Placeholder => None,
        }
    }

    /// Whether the row can be picked.
    pub fn is_selectable(&self) -> bool {
        matches!(self, MenuEntryKind::Action(_) | MenuEntryKind::Submenu(_))
    }
}

/// One row of the hamburger menu in the full UI-061..UI-066 model.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuEntry {
    /// The row's own label (without the check slot or submenu prefix).
    pub label: String,
    /// The submenu owning this row.
    pub section: MenuSection,
    /// What selecting the row does.
    pub kind: MenuEntryKind,
    /// Whether this row is the currently active choice (UI-065, UI-066).
    pub checked: bool,
}

/// A selectable hamburger row: the rendered label plus the intent it
/// publishes.
///
/// This is the command-facing projection of [`MenuEntry`]: only rows with a
/// real [`UserCommand`] survive; submenu navigation is represented by
/// [`MenuMessage`] instead.
#[derive(Clone, Debug, PartialEq)]
pub struct HamburgerItem {
    /// Fully formatted label (check slot + submenu name + row label).
    pub label: String,
    /// The intent published when this row is picked.
    pub command: UserCommand,
}

impl HamburgerItem {
    /// Projects a [`MenuEntry`] onto a command row, returning `None` for
    /// submenu buttons and placeholders.
    pub fn from_entry(entry: &MenuEntry) -> Option<Self> {
        let command = entry.kind.command()?.clone();
        Some(Self {
            label: strings::format_menu_row_label(
                entry.section.title(),
                &entry.label,
                entry.checked,
            ),
            command,
        })
    }
}

impl fmt::Display for HamburgerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// The deficit strategies in hamburger-menu order (UI-066).
///
/// Order is load-bearing:
/// the production default first ([`DeficitStrategy::HybridMinPenaltyWaterFill`],
/// best G1/G2 trade-off), then the two original FTWA strategies, then the
/// three survey algorithms (§2.10 / §2.13 / §2.14 of the table-width
/// algorithm). A new strategy must be appended here as well as
/// to `DeficitStrategy` so [`strategy_label`] stays exhaustive.
pub fn strategy_menu_order() -> [DeficitStrategy; 5] {
    [
        DeficitStrategy::HybridMinPenaltyWaterFill,
        DeficitStrategy::ProportionalToSlack,
        DeficitStrategy::BreakpointWaterFill,
        DeficitStrategy::WaterFillRatio,
        DeficitStrategy::LagrangePenalty,
    ]
}

/// Builds the hamburger menu model: top-level actions and submenu buttons,
/// submenu contents, and the empty-chat-models placeholder, in UI-061..UI-066
/// order.
///
/// Purity: pure function of the view model.
pub fn build_menu_entries(view_model: &TopBarViewModel) -> Vec<MenuEntry> {
    // Direct action items: open a dialog immediately on click.
    let mut entries = vec![
        MenuEntry {
            label: strings::MENU_BATCH.to_string(),
            section: MenuSection::Direct,
            kind: MenuEntryKind::Action(apply_batch_button_click()),
            checked: false,
        },
        MenuEntry {
            label: strings::MENU_TOOLS.to_string(),
            section: MenuSection::Direct,
            kind: MenuEntryKind::Action(apply_tools_button_click()),
            checked: false,
        },
    ];

    // The submenu entry is on the root level; its toggles render only after
    // the user opens that submenu.
    entries.push(submenu_entry(MenuSection::Windows));
    entries.push(MenuEntry {
        label: strings::MENU_BACKGROUND_OPERATIONS.to_string(),
        section: MenuSection::Windows,
        kind: MenuEntryKind::Action(apply_background_logs_toggle(
            !view_model.show_background_logs,
        )),
        checked: view_model.show_background_logs,
    });
    entries.push(MenuEntry {
        label: strings::MENU_AGENT_DEBUG.to_string(),
        section: MenuSection::Windows,
        kind: MenuEntryKind::Action(apply_agent_debug_toggle(!view_model.show_agent_debug)),
        checked: view_model.show_agent_debug,
    });

    // Chat models submenu (UI-064, UI-065).
    entries.push(submenu_entry(MenuSection::ChatModels));
    if view_model.chat_models.is_empty() {
        entries.push(MenuEntry {
            label: strings::NO_CHAT_MODELS_CONFIGURED.to_string(),
            section: MenuSection::ChatModels,
            kind: MenuEntryKind::Placeholder,
            checked: false,
        });
    } else {
        for model in &view_model.chat_models {
            let is_current = view_model.current_chat_model.as_deref() == Some(model.name.as_str());
            entries.push(MenuEntry {
                label: model.label(),
                section: MenuSection::ChatModels,
                kind: MenuEntryKind::Action(model.command()),
                checked: is_current,
            });
        }
    }

    // Table wrap algorithm submenu (UI-066).
    entries.push(submenu_entry(MenuSection::TableWrap));
    for variant in strategy_menu_order() {
        let is_selected = variant == view_model.current_strategy;
        entries.push(MenuEntry {
            label: strategy_label(variant).to_string(),
            section: MenuSection::TableWrap,
            kind: MenuEntryKind::Action(apply_table_width_strategy_change(variant)),
            checked: is_selected,
        });
    }

    // About closes the menu.
    entries.push(MenuEntry {
        label: strings::MENU_ABOUT.to_string(),
        section: MenuSection::Direct,
        kind: MenuEntryKind::Action(apply_about_button_click()),
        checked: false,
    });

    entries
}

/// Projects the hamburger hierarchy into the reusable menu-widget model.
///
/// Direct entries remain at the root; section entries become child rows of
/// their matching submenu, preserving checked labels and the command carried
/// by each selectable item.
#[must_use]
pub fn hamburger_menu_items(entries: &[MenuEntry]) -> Vec<WidgetMenuItem<Message>> {
    entries
        .iter()
        .filter(|entry| entry.section == MenuSection::Direct)
        .filter_map(|entry| match &entry.kind {
            MenuEntryKind::Submenu(section) => {
                let id = section.menu_item_id()?;
                let children = entries
                    .iter()
                    .filter(|child| child.section == *section)
                    .filter_map(hamburger_child_item)
                    .collect();
                Some(WidgetMenuItem::submenu(id, entry.label.clone(), children))
            }
            _ => hamburger_child_item(entry),
        })
        .collect()
}

fn hamburger_child_item(entry: &MenuEntry) -> Option<WidgetMenuItem<Message>> {
    match &entry.kind {
        MenuEntryKind::Action(UserCommand::ToggleBackgroundLogs(_)) => {
            Some(WidgetMenuItem::toggle(
                MENU_BACKGROUND_OPERATIONS_ID,
                entry.label.clone(),
                entry.checked,
            ))
        }
        MenuEntryKind::Action(UserCommand::ToggleAgentDebugWindow(_)) => Some(
            WidgetMenuItem::toggle(MENU_AGENT_DEBUG_ID, entry.label.clone(), entry.checked),
        ),
        MenuEntryKind::Action(command) => Some(WidgetMenuItem::action(
            hamburger_row_label(&entry.label, entry.checked),
            hamburger_row_message(command),
        )),
        MenuEntryKind::Placeholder => Some(WidgetMenuItem::disabled(entry.label.clone())),
        MenuEntryKind::Submenu(_) => None,
    }
}

/// Resolves a hamburger toggle event to the self-contained application intent.
#[must_use]
pub fn hamburger_toggle_command(id: MenuItemId, checked: bool) -> Option<UserCommand> {
    if id == MENU_BACKGROUND_OPERATIONS_ID {
        Some(apply_background_logs_toggle(checked))
    } else if id == MENU_AGENT_DEBUG_ID {
        Some(apply_agent_debug_toggle(checked))
    } else {
        None
    }
}

/// Creates a top-level menu entry that opens `section`.
fn submenu_entry(section: MenuSection) -> MenuEntry {
    let label = match section {
        MenuSection::Windows => strings::MENU_WINDOWS,
        MenuSection::ChatModels => strings::MENU_CHAT_MODELS,
        MenuSection::TableWrap => strings::MENU_TABLE_WRAP_ALGORITHM,
        MenuSection::Direct => unreachable!("direct actions are not submenus"),
    };
    MenuEntry {
        label: label.to_string(),
        section: MenuSection::Direct,
        kind: MenuEntryKind::Submenu(section),
        checked: false,
    }
}

/// The command-bearing hamburger rows, in menu order.
///
/// Purity: pure function of the view model.
pub fn hamburger_items(view_model: &TopBarViewModel) -> Vec<HamburgerItem> {
    build_menu_entries(view_model)
        .iter()
        .filter_map(HamburgerItem::from_entry)
        .collect()
}

/// Everything the top bar needs to render, snapshotted off `FastMdApp`.
///
/// Holding the data in a plain struct keeps the widget tree free of
/// `FastMdApp` reads, which is what makes the panel unit-testable without a
/// live runtime and keeps the view decoupled from
/// where each field comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct TopBarViewModel {
    /// Whether the workspace index has finished building.
    pub indexing_finished: bool,
    /// Number of files discovered by the indexer.
    pub file_count: usize,
    /// Tag-filter options: "All" first, then every known tag.
    pub tag_options: Vec<TagFilterOption>,
    /// The tag filter currently applied, `None` = show all files.
    pub selected_tag: Option<String>,
    /// Chat-capable models listed in the Chat models submenu, name-sorted.
    pub chat_models: Vec<ChatModelEntry>,
    /// The active chat model key, `None` when nothing resolves.
    pub current_chat_model: Option<String>,
    /// The active table-width deficit strategy.
    pub current_strategy: DeficitStrategy,
    /// Whether the Background Processes window is currently shown.
    pub show_background_logs: bool,
    /// Whether the Agent Debug window is currently shown.
    pub show_agent_debug: bool,
}

impl TopBarViewModel {
    /// Snapshots the top bar's inputs off `app`.
    ///
    /// Reads only these accessors:
    /// `file_processor()`, `tags()`, `config()` and
    /// `orchestrator.{background_manager, agent_panel_state}`. Chat models
    /// follow this selection rule: prefer models declaring the `chat`
    /// use case, fall back to every configured model when none do, then
    /// sort by name.
    pub fn from_app(app: &FastMdApp) -> Self {
        let tag_options = std::iter::once(TagFilterOption::all())
            .chain(
                app.tags()
                    .all_tags()
                    .iter()
                    .map(|tag| TagFilterOption::for_tag(tag)),
            )
            .collect();

        let to_entry = |name: &str, cfg: &crate::config::LlmConfig| ChatModelEntry {
            name: name.to_string(),
            cost: cfg.get_cost(),
        };

        let mut chat_models: Vec<ChatModelEntry> = app
            .config()
            .models
            .iter()
            .filter(|(_, cfg)| cfg.has_use_case(CHAT_USE_CASE))
            .map(|(name, cfg)| to_entry(name, cfg))
            .collect();
        if chat_models.is_empty() {
            chat_models = app
                .config()
                .models
                .iter()
                .map(|(name, cfg)| to_entry(name, cfg))
                .collect();
        }
        chat_models.sort_by(|a, b| a.name.cmp(&b.name));

        Self {
            indexing_finished: app.file_processor().indexing_finished,
            file_count: app.file_processor().all_files.len(),
            tag_options,
            selected_tag: app.tags().selected_tag.clone(),
            chat_models,
            current_chat_model: app.config().current_chat_model_key(),
            current_strategy: app.config().deficit_strategy(),
            show_background_logs: app
                .orchestrator
                .background_manager
                .lock()
                .unwrap()
                .show_background_logs,
            show_agent_debug: app.orchestrator.agent_panel_state.show_debug_window,
        }
    }

    /// The [`TagFilterOption`] matching [`Self::selected_tag`], if the tag
    /// is still known.
    pub fn selected_tag_option(&self) -> Option<&TagFilterOption> {
        self.tag_options
            .iter()
            .find(|option| option.tag == self.selected_tag)
    }

    /// The file that should stay selected after the tag filter becomes
    /// `new_tag` — the post-filter selection rule, exposed so
    /// the reducer can apply it from the view model alone.
    ///
    /// Delegates to [`compute_next_selected_file`].
    pub fn next_selected_file_for(
        &self,
        selected_file: Option<&PathBuf>,
        new_tag: Option<&String>,
        file_tags: &BTreeMap<PathBuf, Vec<String>>,
    ) -> Option<PathBuf> {
        compute_next_selected_file(selected_file, new_tag, file_tags)
    }
}

/// Renders the top toolbar from a view-model snapshot.
///
/// The returned element emits [`Message::Command`] for the tag filter and
/// [`Message::Menu`] for hamburger navigation:
/// * tag filter pick → [`UserCommand::SelectTagFilter`]
/// * hamburger open / submenu navigation → [`MenuMessage`]
/// * hamburger row → the row's wrapped [`UserCommand`], routed by the reducer
///
/// The tag filter and the hamburger are always allocated (never removed
/// conditionally) so the widget tree shape does not change across the
/// indexing-finished transition.
///
/// The bar renders as a full-width band at the fixed
/// [`BAR_HEIGHT_POINTS`] height (content vertically centred), with compact
/// pick lists via [`PICK_LIST_PADDING`].
pub fn top_bar(view_model: &TopBarViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let logo = logo_mark(LOGO_MARK_SIZE);

    let title = text(strings::APP_TITLE)
        .size(TITLE_SIZE)
        .color(palette.accent_text);

    let status = text(indexing_status_text(
        view_model.indexing_finished,
        view_model.file_count,
    ))
    .color(indexing_status_color(view_model.indexing_finished))
    .font(indexing_status_font(view_model.indexing_finished));

    let tag_filter: Element<'_, Message> = pick_list(
        view_model.tag_options.clone(),
        view_model.selected_tag_option(),
        |option| Message::Command(option.command()),
    )
    .placeholder(strings::TAG_FILTER_DEFAULT)
    .padding(PICK_LIST_PADDING)
    .width(Length::Shrink)
    .into();

    let hamburger: Element<'_, Message> =
        button(text(strings::HAMBURGER_MENU_BUTTON).size(MENU_BUTTON_GLYPH_SIZE))
            .padding(PICK_LIST_PADDING)
            .on_press(Message::Menu(MenuMessage::Opened))
            .into();

    container(
        Row::with_children(vec![
            logo,
            title.into(),
            rule::vertical(SEPARATOR_WIDTH).into(),
            status.into(),
            tag_filter,
            container(hamburger).align_right(Length::Fill).into(),
        ])
        .spacing(BAR_SPACING)
        .padding(BAR_PADDING)
        .width(Length::Fill)
        .align_y(Alignment::Center),
    )
    // Full-width band at the fixed height, its content
    // vertically centred (`iced_widget-0.14.2/src/container.rs:150-159`).
    // Without the fixed length the band grows to the pick lists' content
    // height and renders far taller than the band contract allows.
    .width(Length::Fill)
    .center_y(BAR_HEIGHT_POINTS)
    .into()
}
