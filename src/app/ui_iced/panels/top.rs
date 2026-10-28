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
//!   [`TagFilterOption`]; emits [`UserCommand::SelectTagFilter`]. Its
//!   trigger is painted through the framework combobox atom —
//!   [`tag_trigger_style`] routes iced's statuses into
//!   [`combo_box::resolve_trigger`] — at the compact rung
//!   [`TAG_TRIGGER_RUNG`], not through iced's own theme.
//! * **Hamburger** — the framework's own
//!   [`Button::icon_only`](hoarfrost::widget::button_widget::Button::icon_only)
//!   in the `Ghost` variant at the compact rung, carrying the ☰ from
//!   [`strings::HAMBURGER_MENU_BUTTON`] plus the label
//!   [`strings::HAMBURGER_MENU_TOOLTIP`] that `UIF-NAV-003` requires of
//!   an icon-only navbar control. Emits
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
//! # Framework adoption is partial for the tag filter, with a reason
//!
//! `hoarfrost::widget::combo_box` is a **resolver and geometry module**, not
//! a widget: it publishes [`combo_box::trigger_bounds`],
//! [`combo_box::chevron_bounds`] and [`combo_box::resolve_trigger`] for the
//! gallery's contrast audit, and no `ComboBox` type that renders an
//! `Element`. The state half, [`ComboHost`](hoarfrost::widget::combo_host::ComboHost),
//! drives open/filter/commit against a `FocusState` the app does not keep
//! for this control. So the trigger's paint comes from the framework today
//! while the flyout stays iced's. Adopting `ComboHost` here means giving the
//! navbar a focus roster — a state-ownership change, not a token alignment,
//! and it is left as a stated follow-on.
//!
//! # Nested submenus (UI-062/UI-064/UI-066)
//!
//! iced 0.14's built-in dropdowns are **single-level**:
//! `overlay::menu::Menu` renders each option as one plain
//! [`ToString::to_string`] row, has no separators, no disabled rows and no
//! child menus, and `iced_aw` is not a dependency. The full hierarchy is
//! therefore kept in the *pure model* — [`build_menu_entries`] returns
//! every row with its [`MenuSection`], [`MenuEntryKind`] and checked state.
//! The **render path** is separate: [`hamburger_menu`](crate::ui_iced::hamburger::hamburger_menu)
//! builds the `hoarfrost` [`Menu`](hoarfrost::surface::menu::model::Menu) tree
//! the framework cascade drives, so the top bar itself only emits
//! [`MenuMessage::Opened`]).
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

use iced::widget::{Row, container, pick_list, rule, text};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Point, Size};

use hoarfrost::state::{BaseState, VisualState};
use hoarfrost::token::typography;
use hoarfrost::token::{shape, size, space};
use hoarfrost::widget::button::ButtonVariant;
use hoarfrost::widget::button_widget::Button;
use hoarfrost::widget::combo_box;

use crate::bus::events::user_command::UserCommand;
use crate::markdown::table_width::DeficitStrategy;
use crate::ui::app::FastMdApp;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::fonts::PHOSPHOR_FONT_NAME;
use crate::ui_iced::logo::logo_mark;
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

/// Logical point size of the app-title heading — `type.title_sm`
/// (`UIF-TOK-004`).
///
/// **Movement: 18 → 16.** The bar used to paint the brand title at a bare
/// `18`, which is not a rung on the type scale — `TITLE_SM` is 16 and
/// `TITLE_MD` is 20, so nothing on the scale produced that number. §4.2.1
/// names the navbar brand title as `type.title_sm` outright, so the title
/// drops 2 pt onto the rung the spec actually assigns rather than staying
/// off-scale. `TITLE_MD` was rejected: it is the document-H2 and
/// About-header rung, and a 20 pt brand line would be the loudest thing
/// inside a 36 pt band whose controls are 28.
pub const TITLE_SIZE: f32 = typography::TITLE_SM.size;

/// Horizontal spacing between top-bar items — `space.sm` (`UIF-TOK-001`),
/// the standard sibling-control gap §4.2.1 prices for every navbar zone.
/// Value unchanged at `8`.
pub const BAR_SPACING: f32 = space::SM;

/// Inner padding of the top bar — `[4px, 12px]`, the pair §4.2.1 prints.
///
/// **Movement: 2 → `[4, 12]`.** The old single scalar `2` was chosen to
/// keep the pick lists inside the then-28 pt band, not to sit on the
/// spacing grid. With the band now `CONTROL_LG` the grid pair fits with
/// room to spare, and the bar closes arithmetically:
/// `CONTROL_LG (36) = XS (4) + CONTROL_SM (28) + XS (4)`.
pub const BAR_PADDING: Padding = Padding {
    top: space::XS,
    bottom: space::XS,
    left: space::MD,
    right: space::MD,
};

/// Full-on-screen height of the **top bar band**, in logical points —
/// `size.control.lg` (`UIF-TOK-002`).
///
/// **Movement: 28 → 36.** The band was `CONTROL_SM`, but `CONTROL_LG`'s
/// own token doc names it as the Top Navbar height, and §4.1.1 and §4.2.1
/// both print 36px for the band. `CONTROL_SM` is still the rung the bar's
/// *controls* take (§4.2.1 "Right: Workspace Controls — Height 28px"),
/// which is why the band grew while the trigger and button did not.
///
/// The bar must stay a fixed height either way: without it the trigger's
/// own content height drives the band and the bar renders taller than the
/// layout contract allows.
pub const BAR_HEIGHT_POINTS: f32 = size::CONTROL_LG;

/// The rung the tag-filter trigger takes — the compact select rung
/// (`size.control.sm`), per §4.2.1's 28px navbar control band.
///
/// Replaces the retired `PICK_LIST_PADDING` scalar pair, which existed
/// only to fight iced's default 5pt button padding down into the old
/// 28 pt band.
pub const TAG_TRIGGER_RUNG: combo_box::ComboRung = combo_box::ComboRung::Compact;

/// Vertical padding inside the tag-filter trigger, **derived from the
/// rung** rather than picked: `(rung − BODY line height) / 2`.
///
/// `(CONTROL_SM 28 − 20) / 2 = 4`, which lands on `space.xs` — so the
/// trigger's height is a consequence of the rung it is declared against,
/// not a number that has to be kept in step with it by hand.
pub const TAG_TRIGGER_VERTICAL_PADDING: f32 =
    (TAG_TRIGGER_RUNG.height() - typography::BODY.line_height) / 2.0;

/// Padding inside the tag-filter trigger.
///
/// Horizontal comes from the framework's own select padding
/// ([`combo_box::HORIZONTAL_PADDING`], `space.md`); vertical is
/// [`TAG_TRIGGER_VERTICAL_PADDING`], which puts the trigger's laid-out
/// height on [`TAG_TRIGGER_RUNG`].
///
/// iced's `pick_list` inherits `button::DEFAULT_PADDING` (5 pt vertical)
/// when unset, which pushed the bar's natural height past its band
/// (pick_list layout contract: `iced_widget-0.14.2/src/pick_list.rs:421-432`
/// — height = padding + text line height).
pub const TAG_TRIGGER_PADDING: Padding = Padding {
    top: TAG_TRIGGER_VERTICAL_PADDING,
    bottom: TAG_TRIGGER_VERTICAL_PADDING,
    left: combo_box::HORIZONTAL_PADDING,
    right: combo_box::HORIZONTAL_PADDING,
};

/// Width of the vertical rule drawn between top-bar groups —
/// `border.hairline` (`UIF-TOK-003`). Value unchanged at `1`.
pub const SEPARATOR_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Minimum width of the hamburger menu surface, in logical points.
///
/// **App-owned.** No framework token covers it: `combo_box::MIN_WIDTH`
/// (120) is the *select* trigger's floor, not a menu card's, and the
/// hamburger's real width comes from
/// [`menu_surface::CARD_WIDTH`](crate::ui_iced::menu_surface::CARD_WIDTH),
/// which exceeds this bound. Same standing as `CARD_WIDTH` and
/// `LICENSE_MAX_HEIGHT`, which are app-owned for the same reason.
pub const MENU_MIN_WIDTH: f32 = 180.0;

/// Height of the ☰ glyph the hamburger button renders —
/// `size.icon.md` (`UIF-TOK-002`), the standard button/menu leading slot
/// rung. Value unchanged at `16`.
pub const MENU_BUTTON_GLYPH_SIZE: f32 = size::ICON_MD;

/// Right inset between the hamburger menu surface and the window's right
/// edge — `space.md` (`UIF-TOK-001`). Value unchanged at `12`.
pub const MENU_ANCHOR_RIGHT_MARGIN: f32 = space::MD;

/// Gap between the top bar's bottom edge and the hamburger menu surface —
/// `space.xs` (`UIF-TOK-001`), the §4.8.1 menu-anchor gap. Value
/// unchanged at `4`.
pub const MENU_ANCHOR_BELOW_BAR: f32 = space::XS;

/// Stable id for the hamburger trigger button ([AGENTS §5]).
///
/// The tag-filter trigger deliberately has **no** counterpart: iced 0.14's
/// `PickList` exposes no `.id()` setter (its builder surface is
/// `placeholder`/`width`/`menu_height`/`padding`/`text_*`/`font`/`handle`/
/// `on_open`/`on_close`/`style`/`menu_style` —
/// `iced_widget-0.14.2/src/pick_list.rs:218-323`), and its open state is
/// never addressed from the reducer, so nothing needs a stable handle on it.
pub const HAMBURGER_BUTTON_ID: &str = "topbar_hamburger_button";

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
    /// `workspace()`, `tags()`, `config()` and
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
            indexing_finished: app.workspace().indexing_finished,
            file_count: app.workspace().all_files.len(),
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

/// The tag-filter trigger's paint, resolved through the framework's
/// combobox atom (`UIF-ATOM-004`).
///
/// iced's `pick_list` is styled by a `(Theme, Status) -> Style` closure
/// and the framework resolves a trigger from `(VisualState, Palette)`.
/// This is the crossing point: iced's three statuses are translated to the
/// framework's `(base, open)` pair and the trigger's fill, perimeter,
/// value and chevron colours all come from
/// [`combo_box::resolve_trigger`] / [`combo_box::chevron_color`] rather
/// than from iced's own theme.
///
/// Two resolved channels have nowhere to go and are deliberately dropped:
/// `focus_ring_color` / `focus_ring_width`. iced's `pick_list::Style` has
/// no ring field — the widget draws no focus ring of its own — so a
/// keyboard-focus ring on this trigger is framework work, not app work.
///
/// Purity: pure function of `status`.
#[must_use]
pub fn tag_trigger_style(status: pick_list::Status) -> pick_list::Style {
    let palette = Palette::pro();
    let (base, open) = match status {
        pick_list::Status::Active => (BaseState::Rest, false),
        pick_list::Status::Hovered => (BaseState::Hovered, false),
        pick_list::Status::Opened { is_hovered } => (
            if is_hovered {
                BaseState::Hovered
            } else {
                BaseState::Rest
            },
            true,
        ),
    };
    let visual = VisualState {
        base,
        focused: false,
    };
    let resolved = combo_box::resolve_trigger(visual, &palette);

    pick_list::Style {
        text_color: resolved.text,
        placeholder_color: resolved.placeholder,
        handle_color: combo_box::chevron_color(visual, open, &palette),
        background: Background::Color(resolved.background),
        border: Border {
            color: resolved.border_color,
            width: resolved.border_width,
            radius: combo_box::RADIUS.into(),
        },
    }
}

/// The navbar brand title, painted on its own rung.
///
/// §4.2.1 asks for `type.title_sm` at `Bold`, and §2.4 names the navbar
/// brand title as one of the two `title_sm` usages that take the rung's
/// **emphasis** weight rather than its resting SemiBold. The rung permits
/// it (`TypeRung::allows`), so the weight comes from the rung instead of
/// a hand-picked number.
fn brand_title<'a>() -> text::Text<'a> {
    let rung = typography::TITLE_SM;
    text(strings::APP_TITLE)
        .size(rung.size)
        .line_height(rung.iced_line_height())
        .font(iced::Font {
            weight: rung.emphasis_weight.iced(),
            ..iced::Font::with_name("Fira Sans")
        })
        .color(Palette::fastmd_dark().accent_text)
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
/// [`BAR_HEIGHT_POINTS`] height (content vertically centred), with the
/// tag trigger padded to [`TAG_TRIGGER_PADDING`] so it lands on
/// [`TAG_TRIGGER_RUNG`].
pub fn top_bar(view_model: &TopBarViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let logo = logo_mark(LOGO_MARK_SIZE);

    let title = brand_title();

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
    .padding(TAG_TRIGGER_PADDING)
    .style(|_theme, status| tag_trigger_style(status))
    .width(Length::Shrink)
    .into();

    let hamburger: Element<'_, Message> = Button::icon_only(
        ButtonVariant::Ghost,
        text(strings::HAMBURGER_MENU_BUTTON)
            .size(MENU_BUTTON_GLYPH_SIZE)
            .color(palette.text_strong),
        Message::Menu(MenuMessage::Opened),
        HAMBURGER_BUTTON_ID,
        strings::HAMBURGER_MENU_TOOLTIP,
    )
    .palette(Palette::pro())
    .compact()
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
    // Without the fixed length the band grows to the trigger's content
    // height and renders taller than the band contract allows.
    .width(Length::Fill)
    .center_y(BAR_HEIGHT_POINTS)
    .into()
}
