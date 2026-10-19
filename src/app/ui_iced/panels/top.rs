//! Iced port of the top toolbar panel ([TASK-011], UI-061..UI-066).
//!
//! Replaces `src/app/ui/panels/top.rs`. The panel is a **pure view**: it
//! renders a [`TopBarViewModel`] snapshot into an
//! [`iced::Element`] of [`Message`] and emits
//! [`Message::Command`] for every user action. It never touches the
//! `UserCommand` bus itself — the [TASK-022]/[TASK-023] reducer publishes
//! what the panel emits ([FMD-068]).
//!
//! # Shape
//!
//! ```text
//! [logo] FastMD Viewer | <indexing status> [tag filter]        [☰ menu]
//! ```
//!
//! * **Logo** — the retired panel vector-painted its badge; this
//!   port renders the shared stand-in built by
//!   [`crate::ui_iced::logo::logo_mark`] at [`LOGO_MARK_SIZE`], because
//!   the legacy UI paint primitives do not exist in iced. See *Feature-gated
//!   widgets* below and the trade-off table in
//!   [`crate::ui_iced::logo`].
//! * **Indexing status** — [`indexing_status_text`] plus the `success`
//!   palette token once indexing has finished.
//! * **Tag filter** — an [`iced::widget::pick_list()`] over
//!   [`TagFilterOption`]; emits [`UserCommand::SelectTagFilter`].
//! * **Hamburger** — an [`iced::widget::pick_list()`] placeholdered with
//!   [`strings::HAMBURGER_MENU_BUTTON`], fed by [`hamburger_items`].
//!
//! # Nested submenus (UI-062/UI-064/UI-066)
//!
//! iced 0.14's built-in dropdowns are **single-level**:
//! `overlay::menu::Menu` renders each option as one plain
//! [`ToString::to_string`] row, has no separators, no disabled rows and no
//! child menus, and DEC-005 rules out `iced_aw`. The port therefore keeps
//! the full hierarchy in the *pure model* — [`build_menu_entries`] returns
//! every row with its [`MenuSection`], [`MenuEntryKind`] and checked state,
//! exactly mirroring the legacy UI menu — and renders the **selectable subset**
//! ([`hamburger_items`]) as one flat list whose labels carry the owning
//! submenu name (`"Windows › Agent debug"`), produced by
//! [`strings::format_menu_row_label`].
//!
//! Only selectable rows reach the widget because a iced dropdown must
//! produce a [`Message`] for *any* row the user clicks, and the panel is
//! allowed to emit only real user intent. Non-selectable rows — section
//! headers and UI-064's [`strings::NO_CHAT_MODELS_CONFIGURED`] placeholder
//! — stay in the model and are filtered out by [`HamburgerItem::from_entry`]
//! rather than being rendered as rows that would fire a meaningless command.
//!
//! # Feature-gated widgets
//!
//! * **Logo** — `iced::widget::image` needs the `image` feature and
//!   `iced::widget::svg` needs `svg`; neither is enabled in `Cargo.toml`
//!   (DEC-011 added iced with `wgpu`, `tiny-skia`, `fira-sans`,
//!   `advanced-shaping` and `markdown` only). A `text` mark is used
//!   instead, single-sourced through [`crate::ui_iced::logo`], which also
//!   records the raster / SVG / `canvas` cost of each option. Adding any of
//!   them is a build-level decision for the orchestrator, not this task
//!   ([TASK-020]).
//! * **Spinner** — iced 0.14 has **no** spinner widget at all (there is no
//!   `iced::widget::spin`, and `iced_widget-0.14.2` contains no spinner
//!   implementation). An indeterminate indicator would need `progress_bar`
//!   driven by a timer subscription, which is runtime wiring outside this
//!   task. The panel renders the status text only; the "indexing in
//!   progress" state stays legible through the italic wording and the
//!   [`strings::build_indexing_progress_text`] ellipsis.
//!
//! # Relocated helpers ([TASK-029])
//!
//! The bar's ten framework-free helpers ([`get_tag_filter_text`],
//! [`compute_next_selected_file`], [`apply_batch_button_click`],
//! [`apply_tools_button_click`], [`apply_about_button_click`],
//! [`apply_background_logs_toggle`], [`apply_agent_debug_toggle`],
//! [`apply_chat_model_selection`], [`apply_table_width_strategy_change`],
//! [`strategy_label`]) used to be `pub use`d from
//! `src/app/ui/panels/top.rs`, a file [TASK-029b] deletes. They are copied
//! into this module verbatim; every one is a pure `UserCommand` constructor
//! or a pure path/tag/label solver with **no framework type** in its signature
//! or body.
//!
//! `build_indexing_status_text` is **not** copied — it returns an
//! `RichText` and cannot be relocated framework-free. This module's own
//! [`indexing_status_text`] is the iced re-typing of it.
//!
//! Unit tests live in the sibling `top_tests.rs` sidecar.

#[cfg(test)]
#[path = "top_tests.rs"]
mod tests;

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use iced::widget::{Row, container, pick_list, rule, text};
use iced::{Alignment, Color, Element, Font, Length};

use crate::bus::events::user_command::UserCommand;
use crate::markdown::table_width::DeficitStrategy;
use crate::ui::app::FastMdApp;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::fonts::PHOSPHOR_FONT_NAME;
use crate::ui_iced::logo::logo_mark;
use crate::ui_iced::message::Message;

// The framework-free helpers the legacy UI panel owned, **relocated into this
// module** for the framework-free migration ([TASK-029]) from
// `src/app/ui/panels/top.rs:33-121`.
//
// They were previously `pub use`d from a file [TASK-029b] deletes. Each is
// copied verbatim and is framework-free — pure `UserCommand` constructors and
// pure path/tag/label solvers with no framework type in any signature or body
// — so the iced panel and the [TASK-022] reducer call the same code. The
// the legacy UI panel keeps its own copy until Phase 5.
//
// `build_indexing_status_text` is deliberately **not** copied: it returns an
// `RichText`, so it cannot be relocated framework-free. The iced panel
// owns its own [`indexing_status_text`](crate::ui_iced::panels::top::indexing_status_text)
// re-typing instead.

/// Purpose: Determines the display text for the tag filter combobox.
/// Inputs: selected_tag (optional string reference)
/// Outputs: String slice representing the text to show.
/// Purity: Pure function.
/// Preconditions: None.
/// Postconditions: Returns the tag name if one is selected, otherwise "Filter by Tag: All".
///
/// Relocated from the legacy UI top panel for the framework-free migration
/// ([TASK-029]).
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
///
/// Relocated from the legacy UI top panel for the framework-free migration
/// ([TASK-029]).
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
///
/// Relocated from the legacy UI top panel for the framework-free migration
/// ([TASK-029]).
#[must_use]
pub fn apply_batch_button_click() -> UserCommand {
    UserCommand::OpenBatchDialog
}

/// Returns the command for clicking the "Tools..." button.
///
/// Relocated from the legacy UI top panel for the framework-free migration
/// ([TASK-029]).
#[must_use]
pub fn apply_tools_button_click() -> UserCommand {
    UserCommand::OpenToolsDialog
}

/// Returns the command for clicking the "About FastMD..." button.
///
/// Relocated from the legacy UI top panel for the framework-free migration
/// ([TASK-029]).
#[must_use]
pub fn apply_about_button_click() -> UserCommand {
    UserCommand::OpenAboutDialog
}

/// Returns the command for toggling the background logs window.
///
/// Relocated from the legacy UI top panel for the framework-free migration
/// ([TASK-029]).
#[must_use]
pub fn apply_background_logs_toggle(show: bool) -> UserCommand {
    UserCommand::ToggleBackgroundLogs(show)
}

/// Returns the command for toggling the agent debug window.
///
/// Relocated from the legacy UI top panel for the framework-free migration
/// ([TASK-029]).
#[must_use]
pub fn apply_agent_debug_toggle(show: bool) -> UserCommand {
    UserCommand::ToggleAgentDebugWindow(show)
}

/// Returns the command for picking a new chat model.
///
/// Relocated from the legacy UI top panel for the framework-free migration
/// ([TASK-029]).
#[must_use]
pub fn apply_chat_model_selection(model_name: String) -> UserCommand {
    UserCommand::SelectChatModel(model_name)
}

/// Returns the command for changing the table-width deficit strategy.
///
/// Relocated from the legacy UI top panel for the framework-free migration
/// ([TASK-029]).
#[must_use]
pub fn apply_table_width_strategy_change(strategy: DeficitStrategy) -> UserCommand {
    UserCommand::ChangeTableWidthStrategy(strategy)
}

/// Human-readable label for a [`DeficitStrategy`] variant, used as the
/// `selected_text` of the top-bar table-width-strategy combobox and as
/// the row label inside the dropdown. The match is intentionally
/// exhaustive over every variant of `DeficitStrategy` so adding a new
/// strategy in the future is a compile error here until the label is
/// supplied.
///
/// Relocated from the legacy UI top panel for the framework-free migration
/// ([TASK-029]).
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
/// ([`crate::ui_iced::agent`]) filters on the exact same key
/// ([TASK-018]).
pub const CHAT_USE_CASE: &str = "chat";

/// Logical point size of the logo mark — the shared constant owned by
/// [`crate::ui_iced::logo`], re-exported here so the top-bar public
/// surface is unchanged ([TASK-020], [RUST-051]).
pub use crate::ui_iced::logo::LOGO_MARK_SIZE;

/// Logical point size of the app-title heading.
pub const TITLE_SIZE: f32 = 18.0;

/// Horizontal spacing between top-bar items.
pub const BAR_SPACING: f32 = 8.0;

/// Inner padding of the top bar.
pub const BAR_PADDING: f32 = 6.0;

/// Width of the vertical rule drawn between top-bar groups.
pub const SEPARATOR_WIDTH: f32 = 1.0;

/// Minimum width of the hamburger dropdown, mirroring the legacy UI panel's
/// `ui.set_min_width(180.0)`.
pub const MENU_MIN_WIDTH: f32 = 180.0;

/// The Phosphor icon font handle used for the logo mark.
///
/// The family is registered by [`crate::ui_iced::fonts`] at boot
/// ([TASK-007]); `PHOSPHOR_FONT_NAME` is the shared registration key.
pub fn phosphor_font() -> Font {
    Font::with_name(PHOSPHOR_FONT_NAME)
}

/// Builds the indexing status label shown in the top bar.
///
/// iced re-typing of the legacy UI panel's `build_indexing_status_text`: the
/// `RichText` return type is replaced by the plain `String` plus the
/// [`indexing_status_color`] / [`indexing_status_font`] token lookups, so no
/// the legacy UI type crosses into the iced layer.
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
/// token while it runs — the iced equivalent of the legacy UI panel's
/// `.color(ui.palette().success)`. No numeric colour is introduced
/// ([GUD-003]).
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
/// Italic while indexing (the legacy UI panel's `.italics()`), upright once
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
            MenuSection::Windows => Some(strings::MENU_WINDOWS),
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
    /// A non-selectable group heading.
    SectionHeader,
    /// A non-selectable informational row — UI-064's "no models
    /// configured" placeholder.
    Placeholder,
}

impl MenuEntryKind {
    /// The intent this row publishes, or `None` for non-selectable rows.
    pub fn command(&self) -> Option<&UserCommand> {
        match self {
            MenuEntryKind::Action(command) => Some(command),
            MenuEntryKind::SectionHeader | MenuEntryKind::Placeholder => None,
        }
    }

    /// Whether the row can be picked.
    pub fn is_selectable(&self) -> bool {
        self.command().is_some()
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
/// This is the widget-facing projection of [`MenuEntry`]: only rows with a
/// real [`UserCommand`] survive, because a iced dropdown has to answer a
/// click on *any* row with a `Message` and the panel must not invent intent.
#[derive(Clone, Debug, PartialEq)]
pub struct HamburgerItem {
    /// Fully formatted dropdown label (check slot + submenu prefix + label).
    pub label: String,
    /// The intent published when this row is picked.
    pub command: UserCommand,
}

impl HamburgerItem {
    /// Projects a [`MenuEntry`] onto a selectable row, returning `None`
    /// for section headers and placeholders.
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
/// Order is load-bearing and carried over verbatim from the legacy UI panel:
/// the production default first ([`DeficitStrategy::HybridMinPenaltyWaterFill`],
/// best G1/G2 trade-off), then the two original FTWA strategies, then the
/// three survey algorithms from `doc/planning/table-column-width-algorithm.md`
/// §2.10 / §2.13 / §2.14. A new strategy must be appended here as well as
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

/// Builds the complete hamburger menu — every row, including the
/// non-selectable section headers and the empty-chat-models placeholder —
/// in UI-061..UI-066 order.
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

    // Windows submenu — window-visibility toggles (UI-062, UI-063). A
    // toggle row carries the value it *switches to*, and is checked while
    // the window is currently shown.
    entries.push(section_header(MenuSection::Windows));
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
    entries.push(section_header(MenuSection::ChatModels));
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
    entries.push(section_header(MenuSection::TableWrap));
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

/// The non-selectable heading row for a submenu.
fn section_header(section: MenuSection) -> MenuEntry {
    MenuEntry {
        label: section.title().unwrap_or_default().to_string(),
        section,
        kind: MenuEntryKind::SectionHeader,
        checked: false,
    }
}

/// The selectable hamburger rows the pick list renders, in menu order.
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
/// live runtime ([TASK-011] testing rule) and keeps the view decoupled from
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
    /// Reads exactly the accessors the legacy UI panel used:
    /// `file_processor()`, `tags()`, `config()` and
    /// `orchestrator.{background_manager, agent_panel_state}`. Chat models
    /// keep the legacy UI selection rule: prefer models declaring the `chat`
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
    /// `new_tag` — the legacy UI panel's post-filter selection rule, exposed so
    /// the [TASK-022] reducer can apply it from the view model alone.
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

/// Renders the iced top toolbar from a view-model snapshot ([TASK-011]).
///
/// The returned element emits [`Message::Command`] only:
/// * tag filter pick → [`UserCommand::SelectTagFilter`]
/// * Batch → [`UserCommand::OpenBatchDialog`]
/// * Tools → [`UserCommand::OpenToolsDialog`]
/// * Background operations / Agent debug → [`UserCommand::ToggleBackgroundLogs`]
///   / [`UserCommand::ToggleAgentDebugWindow`]
/// * chat model → [`UserCommand::SelectChatModel`]
/// * strategy → [`UserCommand::ChangeTableWidthStrategy`]
/// * About → [`UserCommand::OpenAboutDialog`]
///
/// The tag filter and the hamburger are always allocated (never removed
/// conditionally) so the widget tree shape does not change across the
/// indexing-finished transition.
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
    .width(Length::Shrink)
    .into();

    let hamburger: Element<'_, Message> = pick_list(
        hamburger_items(view_model),
        None::<&HamburgerItem>,
        |item| Message::Command(item.command),
    )
    .placeholder(strings::HAMBURGER_MENU_BUTTON)
    .width(MENU_MIN_WIDTH)
    .into();

    Row::with_children(vec![
        logo,
        title.into(),
        rule::vertical(SEPARATOR_WIDTH).into(),
        status.into(),
        tag_filter,
        container(hamburger)
            .width(Length::Fill)
            .align_right(Length::Shrink)
            .into(),
    ])
    .spacing(BAR_SPACING)
    .padding(BAR_PADDING)
    .align_y(Alignment::Center)
    .into()
}
