//! Every test fixture the demo renders.
//!
//! Nothing in this crate reads a file, a database or a network socket
//! (PLAN C4/C5). Every string, list, model and record the widgets display
//! is hardcoded here, so the whole program is a closed, deterministic
//! function of this module plus the user's clicks.
//!
//! The module is organised by the pane that consumes the fixture: navbar
//! fixtures first, then the atoms pane, then the composites pane.
//!
//! Unit tests live in the sibling `data_tests.rs` sidecar.

use hoarfrost::composite::command_palette::{CommandPaletteModel, PaletteGroup, PaletteItem};
use hoarfrost::composite::diff_viewer::{DiffHunk, DiffLine, DiffViewerModel};
use hoarfrost::composite::empty_state::{EmptyStateKind, EmptyStateModel, Icon};
use hoarfrost::composite::grid_view::{GridTile, GridViewModel};
use hoarfrost::composite::keybind_recorder::{KeyChord, KeybindRecorderModel};
use hoarfrost::composite::navbar::{NavBarMenuTrigger, NavBarModel};
use hoarfrost::composite::navigation_rail::{NavDestination, NavigationRailModel};
use hoarfrost::composite::pagination::PaginationModel;
use hoarfrost::composite::pips_pager::PipsPagerModel;
use hoarfrost::composite::side_sheet::{SheetMode, SheetPlacement, SideSheetModel};
use hoarfrost::composite::stepper::{StepItem, StepStatus, StepperModel};
use hoarfrost::composite::tab_strip::{TabItem, TabStripModel};
use hoarfrost::surface::menu::model::{Menu, Row};
use hoarfrost::widget::badge_model::BadgeTone;
use hoarfrost::widget::color_picker::Rgba8;
use hoarfrost::widget::combo_model::SelectOption;
use hoarfrost::widget::date_picker::CivilDate;
use hoarfrost::widget::time_picker::CivilTime;
use hoarfrost::widget::toggle_model::{CheckState, ChildSummary};

/// The brand string the navbar carries.
pub const BRAND: &str = "hoarfrost demo";

/// The navbar menu triggers, as `(stable id, label)` pairs.
pub const NAVBAR_TRIGGERS: [(&str, &str); 4] = [
    ("file", "File"),
    ("edit", "Edit"),
    ("view", "View"),
    ("help", "Help"),
];

/// Builds the navbar model: a brand plus four inert menu triggers.
///
/// Pressing a trigger only latches it open/closed in the model, which is
/// exactly the `UIF-NAV-001` latch being on display.
#[must_use]
pub fn navbar_model() -> NavBarModel {
    let mut model = NavBarModel::new().with_brand(BRAND);
    for (id, label) in NAVBAR_TRIGGERS {
        model = model.with_trigger(NavBarMenuTrigger::new(id, label).with_tooltip(label));
    }
    model
}

/// The status text shown before the user touches anything.
pub const STATUS_READY: &str = "ready";

/// Default split ratio between the two panes.
pub const SPLIT_RATIO_DEFAULT: f32 = 0.5;

/// Smallest ratio the split divider may take.
pub const SPLIT_RATIO_MIN: f32 = 0.25;

/// Largest ratio the split divider may take.
pub const SPLIT_RATIO_MAX: f32 = 0.75;

// ---------------------------------------------------------------------------
// Atoms pane fixtures
// ---------------------------------------------------------------------------

/// The checkbox demo rows: `(label, state, children, disabled)`.
pub const CHECKBOX_ROWS: [(&str, CheckState, ChildSummary, bool); 4] = [
    (
        "Unchecked",
        CheckState::Unchecked,
        ChildSummary::LEAF,
        false,
    ),
    ("Checked", CheckState::Checked, ChildSummary::LEAF, false),
    (
        "Indeterminate",
        CheckState::Indeterminate,
        ChildSummary {
            enabled: 3,
            checked: 1,
        },
        false,
    ),
    ("Disabled", CheckState::Checked, ChildSummary::LEAF, true),
];

/// The radio options shown in the Radio group section, as `(id, label)`.
pub const RADIO_OPTIONS: [(&str, &str); 3] = [
    ("compact", "Compact"),
    ("comfortable", "Comfortable"),
    ("spacious", "Spacious"),
];

/// The radio option selected at boot.
pub const RADIO_SELECTED: &str = "comfortable";

/// The segmented strip's segments, as `(id, label)`.
pub const SEGMENT_ITEMS: [(&str, &str); 3] = [("day", "Day"), ("week", "Week"), ("month", "Month")];

/// The segment selected at boot.
pub const SEGMENT_SELECTED: &str = "week";

/// The split button's primary label.
pub const SPLIT_PRIMARY_LABEL: &str = "Split Action";

/// The split button's flyout rows, as `(id, label)`.
pub const SPLIT_MENU_ROWS: [(&str, &str); 3] = [
    ("export-pdf", "Export as PDF"),
    ("export-md", "Export as Markdown"),
    ("copy-link", "Copy link"),
];

/// The combo box options shown in the atoms pane, as `(id, label)`.
pub const COMBO_OPTIONS: [(&str, &str); 5] = [
    ("typst", "Typst"),
    ("weasyprint", "WeasyPrint"),
    ("pandoc", "Pandoc"),
    ("chromium", "Chromium print"),
    ("plain", "Plain text"),
];

/// The combo box option selected at boot.
pub const COMBO_SELECTED: &str = "typst";

/// The placeholder an unselected combo box shows.
pub const COMBO_PLACEHOLDER: &str = "Choose a renderer";

/// The validation message the invalid combo box carries.
pub const COMBO_ERROR: &str = "A renderer is required";

/// The hint an empty text field shows.
pub const TEXT_PLACEHOLDER: &str = "Type something…";

/// The value the live text field starts with.
pub const TEXT_SAMPLE: &str = "hoarfrost";

/// The value the read-only text field shows.
pub const TEXT_READONLY_SAMPLE: &str = "report-2026-10.md";

/// The value the invalid text field shows.
pub const TEXT_INVALID_SAMPLE: &str = "no-spaces-allowed";

/// The validation message the invalid text field carries.
pub const TEXT_ERROR: &str = "Value rejected by the host validator";

/// The hint an empty password field shows.
pub const PASSWORD_PLACEHOLDER: &str = "Secret";

/// The value the password field starts with.
pub const PASSWORD_SAMPLE: &str = "correct horse battery staple";

/// The query the live search box starts with.
pub const SEARCH_SAMPLE: &str = "renderer";

/// The rating's maximum star count.
pub const RATING_MAX: u8 = 5;

/// The rating the live control starts on.
pub const RATING_VALUE: f32 = 4.0;

/// The range slider's bounds and initial selection, as `(min, max, lower, upper)`.
pub const RANGE_BOUNDS: (f64, f64, f64, f64) = (0.0, 240.0, 60.0, 120.0);

/// The range slider's step.
pub const RANGE_STEP: f64 = 5.0;

/// The unit the range slider prints after each value.
pub const RANGE_UNIT: &str = "mm";

/// The avatar roster, as `(display name, presence tone)`.
pub const AVATAR_ROSTER: [(&str, Option<BadgeTone>); 4] = [
    ("Kathrin Müller", Some(BadgeTone::Success)),
    ("Ada Lovelace", Some(BadgeTone::Accent)),
    ("Gerhard von Kant", Some(BadgeTone::Warning)),
    ("Unnamed Contact", None),
];

/// The fixed "today" the calendar is anchored to.
///
/// Hardcoded rather than read from a clock: PLAN C4 forbids this program
/// reading the time, and a fixed today keeps every calendar screenshot
/// and every test assertion deterministic.
pub const TODAY: (i32, u8, u8) = (2026, 10, 26);

/// The date the single-date picker starts on.
pub const DATE_SELECTED: (i32, u8, u8) = (2026, 10, 14);

/// The date range the range picker starts on.
pub const DATE_RANGE: ((i32, u8, u8), (i32, u8, u8)) = ((2026, 10, 12), (2026, 10, 19));

/// Dates the calendar refuses to offer.
pub const DATE_BLACKOUTS: [(i32, u8, u8); 2] = [(2026, 10, 16), (2026, 10, 17)];

/// The time the time picker starts on, as `(hour, minute, second)`.
pub const TIME_SELECTED: (u8, u8, u8) = (9, 30, 0);

/// The colour the colour picker starts on, as `(r, g, b)`.
pub const COLOR_SELECTED: (u8, u8, u8) = (56, 132, 234);

/// The surface the colour picker measures its contrast against.
pub const COLOR_CONTRAST_REF: (u8, u8, u8) = (248, 248, 250);

// ---------------------------------------------------------------------------
// Composites pane fixtures
// ---------------------------------------------------------------------------

/// The open editor tabs, as `(id, title)`.
pub const TABS: [(&str, &str); 4] = [
    ("tab-report", "report.md"),
    ("tab-notes", "notes.md"),
    ("tab-todo", "todo.md"),
    ("tab-archive", "archive.md"),
];

/// The tabs carrying unsaved changes.
pub const TABS_DIRTY: [&str; 1] = ["tab-notes"];

/// The tabs pinned against closing.
pub const TABS_PINNED: [&str; 1] = ["tab-archive"];

/// The active tab.
pub const TAB_SELECTED: &str = "tab-report";

/// The navigation rail's primary destinations, as `(id, label, glyph)`.
pub const RAIL_TOP: [(&str, &str, &str); 3] = [
    ("rail-docs", "Documents", "🗎"),
    ("rail-tasks", "Tasks", "☑"),
    ("rail-calendar", "Calendar", "🗓"),
];

/// The navigation rail's footer destinations, as `(id, label, glyph)`.
pub const RAIL_BOTTOM: [(&str, &str, &str); 2] = [
    ("rail-settings", "Settings", "⚙"),
    ("rail-help", "Help", "❓"),
];

/// The selected rail destination.
pub const RAIL_SELECTED: &str = "rail-docs";

/// The wizard's steps, as `(id, label)`.
pub const WIZARD_STEPS: [(&str, &str); 4] = [
    ("step-source", "Source"),
    ("step-theme", "Theme"),
    ("step-layout", "Layout"),
    ("step-export", "Export"),
];

/// The wizard's current step.
pub const WIZARD_CURRENT: &str = "step-theme";

/// The pagination fixture: `(page, page_size, total_items)`.
pub const PAGINATION: (usize, usize, usize) = (3, 25, 480);

/// The page sizes the pagination bar offers.
pub const PAGE_SIZE_OPTIONS: [usize; 4] = [10, 25, 50, 100];

/// The pips pager fixture: `(page, total_pages)`.
pub const PIPS: (usize, usize) = (2, 7);

/// Builds the combo box option list as framework [`SelectOption`]s.
///
/// `Id` is `String` so the same fixture drives both the live and the
/// disabled combo box without a second copy of the list.
#[must_use]
pub fn combo_options() -> Vec<SelectOption<String>> {
    COMBO_OPTIONS
        .iter()
        .map(|(id, label)| SelectOption::new((*id).to_string(), *label))
        .collect()
}

/// Turns a `(year, month, day)` fixture into a framework [`CivilDate`].
///
/// Every fixture date in this module is a real calendar day, so the
/// unwrap is a checked invariant rather than a runtime risk — and
/// `data_tests` asserts each one parses.
#[must_use]
pub fn date(ymd: (i32, u8, u8)) -> CivilDate {
    CivilDate::new(ymd.0, ymd.1, ymd.2).expect("fixture date must be a real calendar day")
}

/// Turns a `(hour, minute, second)` fixture into a framework [`CivilTime`].
#[must_use]
pub fn time(hms: (u8, u8, u8)) -> CivilTime {
    CivilTime::new(hms.0, hms.1, hms.2).expect("fixture time must be a real time of day")
}

/// Turns an `(r, g, b)` fixture into an opaque framework [`Rgba8`] colour.
#[must_use]
pub fn rgb(triple: (u8, u8, u8)) -> Rgba8 {
    Rgba8::from_rgb(triple.0, triple.1, triple.2)
}

// ---------------------------------------------------------------------------
// Composite model fixtures (right pane, S5)
// ---------------------------------------------------------------------------

/// Builds the tab strip model the right pane renders.
///
/// The dirty and pinned flags come from [`TABS_DIRTY`] and [`TABS_PINNED`]
/// rather than being restated per tab, and the active tab is set explicitly
/// because [`TabStripModel::open_tab`] always selects what it appends.
#[must_use]
pub fn tab_strip_model() -> TabStripModel {
    let mut model = TabStripModel::new();
    for (id, title) in TABS {
        let item = TabItem::new(id, title)
            .with_tooltip(format!("{title} - demo tab"))
            .with_dirty(TABS_DIRTY.contains(&id))
            .with_pinned(TABS_PINNED.contains(&id));
        model.open_tab(item);
    }
    model.select_tab(TAB_SELECTED);
    model
}

/// Builds the navigation rail model from [`RAIL_TOP`] / [`RAIL_BOTTOM`].
#[must_use]
pub fn rail_model() -> NavigationRailModel<String> {
    NavigationRailModel::new(
        rail_destinations(&RAIL_TOP),
        rail_destinations(&RAIL_BOTTOM),
        RAIL_SELECTED.to_string(),
    )
    .with_expanded(true)
}

/// Turns `(id, label, glyph)` fixture rows into rail destinations.
#[must_use]
pub fn rail_destinations(rows: &[(&str, &str, &str)]) -> Vec<NavDestination<String>> {
    rows.iter()
        .map(|(id, label, glyph)| {
            NavDestination::new((*id).to_string(), *label, Icon((*glyph).to_string()))
        })
        .collect()
}

/// Builds the wizard model.
///
/// Step status is *derived* from [`WIZARD_CURRENT`] — everything before the
/// current step is `Completed`, the current step is `Current`, everything
/// after is `Pending` — so the indicator can never contradict the fixture.
#[must_use]
pub fn stepper_model() -> StepperModel<String> {
    let current = WIZARD_CURRENT;
    let at = WIZARD_STEPS
        .iter()
        .position(|(id, _)| *id == current)
        .unwrap_or(0);

    let steps = WIZARD_STEPS
        .iter()
        .enumerate()
        .map(|(index, (id, label))| {
            let status = match index.cmp(&at) {
                std::cmp::Ordering::Less => StepStatus::Completed,
                std::cmp::Ordering::Equal => StepStatus::Current,
                std::cmp::Ordering::Greater => StepStatus::Pending,
            };
            StepItem::new((*id).to_string(), *label, status)
        })
        .collect();

    StepperModel::new(steps, current.to_string(), true)
}

/// Builds the pagination model from [`PAGINATION`] / [`PAGE_SIZE_OPTIONS`].
#[must_use]
pub fn pagination_model() -> PaginationModel {
    PaginationModel::new(
        PAGINATION.0,
        PAGINATION.1,
        PAGINATION.2,
        PAGE_SIZE_OPTIONS.to_vec(),
    )
}

/// Builds the pips pager model from [`PIPS`].
#[must_use]
pub fn pips_model() -> PipsPagerModel {
    PipsPagerModel::new(PIPS.0, PIPS.1)
}

// ---------------------------------------------------------------------------
// Composite model fixtures (right pane, S6)
// ---------------------------------------------------------------------------

/// The language badge the code block carries.
pub const CODE_LANGUAGE: &str = "rust";

/// The source the code block displays.
///
/// Deliberately short: the block is rendered inside a scrolling pane, so a
/// long listing would push every section below it off screen.
pub const CODE_SAMPLE: &str =
    "// hoarfrost demo\npub fn greet(name: &str) -> String {\n    format!(\"hello, {name}\")\n}\n";

/// The diff's hunk headers, as `(id, header)`.
pub const DIFF_HUNKS: [(&str, &str); 2] = [
    ("hunk-1", "@@ -1,4 +1,5 @@ fn greet"),
    ("hunk-2", "@@ -20,3 +21,6 @@ mod tests"),
];

/// The diff's lines, grouped by hunk index.
///
/// Each row is `(kind, old_line, new_line, text)` where a `0` line number
/// means "absent on that side" — mapped to `Option` by [`diff_model`].
pub const DIFF_LINES: [[(&str, u32, u32, &str); 4]; 2] = [
    [
        ("context", 1, 1, "pub fn greet(name: &str) -> String {"),
        ("deleted", 2, 0, "    String::from(\"hello\")"),
        ("added", 0, 2, "    format!(\"hello, {name}\")"),
        ("context", 3, 3, "}"),
    ],
    [
        ("context", 20, 21, "#[test]"),
        ("added", 0, 22, "fn greet_names_the_caller() {"),
        (
            "added",
            0,
            23,
            "    assert_eq!(greet(\"ada\"), \"hello, ada\");",
        ),
        ("added", 0, 24, "}"),
    ],
];

/// Builds the diff viewer model from [`DIFF_HUNKS`] / [`DIFF_LINES`].
#[must_use]
pub fn diff_model() -> DiffViewerModel<String> {
    let hunks = DIFF_HUNKS
        .iter()
        .enumerate()
        .map(|(index, (id, header))| {
            let lines = DIFF_LINES[index]
                .iter()
                .map(|(kind, old_line, new_line, text)| match *kind {
                    "added" => DiffLine::added(*new_line, *text),
                    "deleted" => DiffLine::deleted(*old_line, *text),
                    _ => DiffLine::context(*old_line, *new_line, *text),
                })
                .collect();
            DiffHunk::new((*id).to_string(), *header, lines)
        })
        .collect();
    DiffViewerModel::new(hunks)
}

/// The empty-state fixtures: `(kind_tag, title, description, action_label)`.
///
/// `kind_tag` is `"items"`, `"matches"` or `"error"`; [`empty_state_models`]
/// turns it into the framework's [`EmptyStateKind`], attaching the query or
/// error code the variant needs.
pub const EMPTY_STATES: [(&str, &str, &str, &str); 3] = [
    (
        "items",
        "No documents yet",
        "Create one with Ctrl+N, or drop a folder to open it.",
        "New document",
    ),
    (
        "matches",
        "Nothing matched",
        "Try a shorter phrase, or clear the filter.",
        "Clear filter",
    ),
    (
        "error",
        "Could not load the workspace",
        "The folder is on a disconnected drive.",
        "Retry",
    ),
];

/// The query an `EmptyStateKind::NoMatches` reports.
pub const EMPTY_QUERY: &str = "zzzqqq";

/// The error code an `EmptyStateKind::LoadError` reports.
pub const EMPTY_ERROR_CODE: &str = "WS-4001";

/// Builds the three empty-state models, each with its recovery action.
#[must_use]
pub fn empty_state_models() -> Vec<EmptyStateModel<String>> {
    EMPTY_STATES
        .iter()
        .map(|(tag, title, description, action)| {
            let kind = match *tag {
                "matches" => EmptyStateKind::NoMatches {
                    query: EMPTY_QUERY.to_string(),
                },
                "error" => EmptyStateKind::LoadError {
                    code: EMPTY_ERROR_CODE.to_string(),
                },
                _ => EmptyStateKind::NoItems,
            };
            let icon = Icon(
                match *tag {
                    "matches" => "⌕",
                    "error" => "⚠",
                    _ => "▢",
                }
                .to_string(),
            );
            EmptyStateModel::new(kind, icon, *title, *description)
                .with_action(*action, format!("empty-{tag}"))
        })
        .collect()
}

/// The grid tiles, as `(id, title, subtitle, disabled)`.
pub const GRID_TILES: [(&str, &str, &str, bool); 6] = [
    ("tile-1", "report.md", "Edited 2 minutes ago", false),
    ("tile-2", "notes.md", "Edited today", false),
    ("tile-3", "archive.zip", "Read only", true),
    ("tile-4", "plan.md", "Draft", false),
    ("tile-5", "ideas.md", "Edited yesterday", false),
    ("tile-6", "inbox.md", "3 unread", false),
];

/// Builds the grid view model from [`GRID_TILES`].
#[must_use]
pub fn grid_model() -> GridViewModel<String> {
    let tiles = GRID_TILES
        .iter()
        .map(|(id, title, subtitle, disabled)| {
            let tile = GridTile::new((*id).to_string(), *title);
            let tile = if subtitle.is_empty() {
                tile
            } else {
                tile.subtitle(*subtitle)
            };
            tile.preview_icon(Icon("▤".to_string())).disabled(*disabled)
        })
        .collect();
    GridViewModel::new(tiles)
}

/// The keybind the recorder starts with, as `(ctrl, alt, shift, super, key)`.
pub const KEYBIND: (bool, bool, bool, bool, &str) = (true, false, true, false, "P");

/// The command name the recorded keybind conflicts with.
pub const KEYBIND_CONFLICT: &str = "Open command palette";

/// Builds the keybind recorder model from [`KEYBIND`].
///
/// The conflict is part of the fixture so the recorder's conflict banner is
/// visible without the user having to collide with a real command.
#[must_use]
pub fn keybind_model() -> KeybindRecorderModel {
    let chord = KeyChord::new(KEYBIND.0, KEYBIND.1, KEYBIND.2, KEYBIND.3, KEYBIND.4);
    KeybindRecorderModel::new()
        .with_chord(Some(chord))
        .with_conflict(Some(KEYBIND_CONFLICT.to_string()))
}

/// The side sheet's title.
pub const SHEET_TITLE: &str = "Document settings";

/// The side sheet's starting width in logical pixels.
pub const SHEET_WIDTH: f32 = 320.0;

/// The side sheet's body text.
pub const SHEET_BODY: &str = "Docked panels share the layout; overlay panels float above it.";

/// Builds the side sheet model.
#[must_use]
pub fn sheet_model() -> SideSheetModel {
    SideSheetModel::new(SHEET_TITLE)
        .with_width(SHEET_WIDTH)
        .with_placement(SheetPlacement::Right)
        .with_mode(SheetMode::Docked)
}

/// One command palette row fixture: `(id, title, shortcut)`.
pub type PaletteRow = (&'static str, &'static str, &'static str);

/// The command palette's groups, as `(heading, [PaletteRow])`.
pub const PALETTE_GROUPS: [(&str, &[PaletteRow]); 2] = [
    (
        "Document",
        &[
            ("cmd-new", "New document", "Ctrl+N"),
            ("cmd-save", "Save", "Ctrl+S"),
            ("cmd-close", "Close tab", "Ctrl+W"),
        ],
    ),
    ("View", &[("cmd-rail", "Toggle rail", "Ctrl+B")]),
];

/// The query the palette starts with.
pub const PALETTE_QUERY: &str = "sa";

/// Builds the command palette model from [`PALETTE_GROUPS`].
#[must_use]
pub fn palette_model() -> CommandPaletteModel<String> {
    let groups = PALETTE_GROUPS
        .iter()
        .map(|(heading, items)| {
            let items = items
                .iter()
                .map(|(id, title, shortcut)| {
                    PaletteItem::new((*id).to_string(), *title).with_shortcut(*shortcut)
                })
                .collect();
            PaletteGroup::new(*heading, items)
        })
        .collect();
    CommandPaletteModel::new(groups)
        .with_query(PALETTE_QUERY)
        .with_open(true)
}

/// The form field's label, helper and error text.
pub const FORM_LABEL: &str = "Workspace path";
/// Helper text shown while the field is valid.
pub const FORM_HELPER: &str = "Absolute path to the vault root.";
/// Error text shown while the field is invalid.
pub const FORM_ERROR: &str = "That folder does not exist.";

/// The settings rows, as `(title, description, disabled)`.
pub const SETTINGS_ROWS: [(&str, &str, bool); 3] = [
    ("Show line numbers", "Gutter digits in code blocks.", false),
    ("Auto-save", "Write every 30 seconds after a change.", false),
    ("Telemetry", "Disabled by this build policy.", true),
];

/// The menu's rows, as `(key, label, kind)` where `kind` is
/// `"action"`, `"check"`, `"radio"` or `"separator"`.
pub const MENU_ROWS: [(&str, &str, &str); 7] = [
    ("m-new", "New document", "action"),
    ("m-save", "Save", "action"),
    ("m-sep-1", "", "separator"),
    ("m-wrap", "Soft wrap", "check"),
    ("m-sep-2", "", "separator"),
    ("m-light", "Light theme", "radio"),
    ("m-dark", "Dark theme", "radio"),
];

/// The menu's own id.
pub const MENU_ID: &str = "demo.file.menu";

/// The radio group the two theme rows belong to.
pub const MENU_RADIO_GROUP: &str = "theme";

/// Builds the menu tree from [`MENU_ROWS`].
#[must_use]
pub fn menu_model() -> Menu {
    let rows = MENU_ROWS
        .iter()
        .map(|(key, label, kind)| match *kind {
            "separator" => Row::separator(),
            "check" => Row::checkable(*key, *label, true).with_shortcut("Ctrl+."),
            "radio" => Row::radio(*key, *label, MENU_RADIO_GROUP, *key == "m-dark"),
            _ => Row::action(*key, *label).with_shortcut(match *key {
                "m-new" => "Ctrl+N",
                "m-save" => "Ctrl+S",
                _ => "",
            }),
        })
        .collect();
    Menu::new(MENU_ID, rows)
}

#[cfg(test)]
#[path = "data_tests.rs"]
mod tests;
