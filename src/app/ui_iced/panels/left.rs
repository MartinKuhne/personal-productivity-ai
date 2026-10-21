//! Left file-tree panel (UI-004..UI-018, UI-020..UI-025 and UI-050).
//!
//! Like [`top`](super::top), the panel is a **pure view**: it renders a
//! [`LeftPanelViewModel`] snapshot into an [`iced::Element`] of [`Message`]
//! and emits [`Message::Command`] for user intent. It never touches the
//! `UserCommand` bus, never reads a file and never mutates `FastMdApp` —
//! the reducer does all of that, so every intent is self-contained and
//! replayable.
//!
//! # Shape
//!
//! ```text
//! Workspace Files
//! [ search workspace… ] [🔎] [✕]
//! ├ ▸ Work                 <- tree rows (or search-result cards)
//! │   ▸ Projects
//! └ note.md
//! ```
//!
//! # Reused, not reimplemented ([RUST-051])
//!
//! * [`flatten_tree`] turns the `TreeNode` hierarchy into the visible
//!   [`FlatRow`] list, which is where **UI-017** (directories before files,
//!   then name-ascending) and **UI-018** (a tag filter prunes directories
//!   with no matching files) actually live. Rendering the reused rows *in
//!   order* is what preserves both requirements — this module adds no
//!   sorting of its own. `crate::ui::tree` is framework-free shared code, so
//!   that import stays.
//! * [`crate::ui_iced::layout::clamped_width`] is the single UI-016
//!   clamping rule shared with the pane-grid resize reduction.
//!
//! # Workspace-tree construction
//!
//! [`build_workspace_tree`] builds the `TreeNode` hierarchy inside this
//! module, together with its three private helpers (`prune_empty_dirs`,
//! `library_for_path`, `insert_path_into_tree`). All four are
//! **framework-free**: they touch only [`TreeNode`], [`ContentLibrary`] and
//! `Path`, never a UI framework type. The `TreeNode` type itself lives in
//! `crate::ui::app`, so it is imported rather than duplicated.
//!
//! The rest of the module is the panel's own typing: the width measurement,
//! the row labels, the click→command mapping and the context-menu item list.
//!
//! # UI-016 auto-fit is a pure label→width function
//!
//! A pure view has no font atlas to ask, so [`auto_fit_width`] takes an
//! **injected measurer** (`Fn(&str) -> f32`) and is otherwise the plain
//! formula `depth × 18 + label_width + 40`, clamped to
//! `[LEFT_PANEL_MIN_WIDTH, viewport × LEFT_PANEL_MAX_WIDTH_FRACTION]`.
//! **Real pixel measurement is not achievable in a pure view** — iced 0.14
//! only exposes text metrics after layout, from inside `Widget::layout`
//! (`iced_core-0.14.0/src/layout.rs`). The reducer therefore supplies the
//! measurer (an approximation is fine for a clamped fit) or keeps the
//! user's remembered width.
//!
//! # Deferred context-menu intent
//!
//! "Run as prompt…", "Print…" and the multi-select "Delete" read files or
//! fan out across many commands, so the panel emits the matching
//! [`TreeMessage`] variant instead and lets the reducer do the work — see
//! the [`message`](crate::ui_iced::message) module header.
//!
//! # Known gaps
//!
//! * **Modifier-aware multi-select (UI-022) — CLOSED.** iced's
//!   `MouseArea::on_press` carries no modifier state
//!   (`mouse_area.rs:38-41`) and neither does
//!   `mouse::Event::ButtonPressed` (`iced_core-0.14.0/src/mouse/event.rs:26`),
//!   so multi-select is carried as the [`LeftPanelViewModel::multi_select`]
//!   field and [`from_app`](LeftPanelViewModel::from_app) defaults it to
//!   `false` — it has no modifier input. The value is supplied one level up:
//!   the subscription bridge's keyboard lane mirrors Shift onto
//!   [`OverlayState::modifiers`](crate::ui_iced::app::OverlayState::modifiers),
//!   and `ViewModels::from_app` overrides the field from it alongside the
//!   other reducer-owned merges. The Shift keydown is delivered and reduced
//!   **before** the row-click message, so the bit is current when
//!   [`row_click_command`] runs.
//! * **Tooltips.** iced 0.14 has no tooltip primitive, so the search and
//!   clear buttons' tooltips
//!   ([`strings::SEARCH_TRIGGER_TOOLTIP`],
//!   [`strings::SEARCH_CLEAR_TOOLTIP`]) are not rendered.
//! * **Search box position.** UI-050 says "at the bottom of the directory
//!   tree pane"; the panel renders it directly under the header, which is
//!   the shipped behaviour rather than the spec wording ([RUST-041] drift
//!   note).
//!
//! Unit tests live in the sibling `left_tests.rs` sidecar.

#[cfg(test)]
#[path = "left_tests.rs"]
mod tests;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::widget::{Column, Row, button, container, mouse_area, scrollable, text, text_input};
use iced::{Alignment, Background, Element, Font, Length, Padding};

use crate::bus::events::user_command::UserCommand;
use crate::config::{AppConfig, ContentLibrary, SkillFile};
use crate::ui::TreeNode;
use crate::ui::app::FastMdApp;
use crate::ui::panel_layout::{
    LEFT_PANEL_DEFAULT_WIDTH, LEFT_PANEL_MAX_WIDTH_FRACTION, LEFT_PANEL_MIN_WIDTH,
};
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui::tree::{FlatRow, flatten_tree};
use crate::ui::tree_search::SearchResultEntry;
use crate::ui_iced::layout::clamped_width;
use crate::ui_iced::message::{Message, SearchMessage, TreeMessage};
use crate::ui_iced::overlay::MenuItem;

use super::top::phosphor_font;

/// Name of the content library that hosts the system skill folders
/// (VFS-121/VFS-122). Not user-facing; named once ([RUST-021]).
const SYSTEM_LIBRARY_NAME: &str = "System";

/// First path segment of the skill tree inside the system library.
const SKILLS_SUBDIR: &str = "Skills";

/// Skill subfolder offered on **directory** rows (VFS-122).
const FOLDER_SKILLS_SUBDIR: &str = "Folder";

/// Skill subfolder offered on **file** rows (VFS-121).
///
/// `pub` so the tab context menu ([`center`](super::center)) resolves the
/// same note-skill folder through [`skill_dir`] instead of repeating the
/// literal ([RUST-021]).
pub const NOTE_SKILLS_SUBDIR: &str = "Note";

/// Logical point size of the panel heading.
pub const HEADER_SIZE: f32 = 16.0;

/// Logical point size of tree rows and search-result titles — the size the
/// width pass measures at.
pub const ROW_TEXT_SIZE: f32 = 14.0;

/// Logical point size of the secondary search-result lines.
pub const SECONDARY_TEXT_SIZE: f32 = 11.0;

/// Horizontal indent added per tree depth level, in logical points.
pub const ROW_INDENT_PER_DEPTH: f32 = 18.0;

/// Deepest indent that is drawn; deeper rows clamp to it, guarding against
/// unbounded indentation.
pub const MAX_ROW_INDENT_DEPTH: usize = 50;

/// Width added on top of a measured label for row padding, the selection
/// frame and the scroll gutter.
pub const ROW_WIDTH_SLACK: f32 = 40.0;

/// Padding inside the panel body.
pub const PANEL_PADDING: f32 = 6.0;

/// Vertical padding of one tree row.
pub const ROW_PADDING_Y: f32 = 2.0;

/// Horizontal padding of one tree row, before the depth indent is added.
pub const ROW_PADDING_X: f32 = 6.0;

/// Slot height of one search-result card, in logical pixels.
pub const SEARCH_ROW_HEIGHT: f32 = 52.0;

/// Icon slot for a file row. Directories draw a caret; a file gets a blank
/// slot of the same width so the two label shapes stay aligned — the width
/// measurement uses this same string.
pub const FILE_ROW_ICON: &str = " ";

/// The glyph that opens a directory row's caret slot.
///
/// Purity: pure function of `(is_dir, is_expanded)`.
#[must_use]
pub fn row_icon(is_dir: bool, is_expanded: bool) -> &'static str {
    if is_dir {
        if is_expanded {
            strings::ICON_CARET_DOWN
        } else {
            strings::ICON_CARET_RIGHT
        }
    } else {
        FILE_ROW_ICON
    }
}

/// The label a row renders: `"<icon> <name>"`.
///
/// The auto-fit pass measures this exact string, so any measurer swapped in
/// produces the same width for the same rows.
///
/// Purity: pure function of `row`.
#[must_use]
pub fn row_label(row: &FlatRow) -> String {
    format!("{} {}", row_icon(row.is_dir, row.is_expanded), row.name)
}

/// Width a single row needs: `indent + measured label + slack`.
///
/// `measure` is the injected label measurer — see the module header for why
/// measurement cannot come from inside the view.
///
/// Purity: pure for a pure `measure`.
#[must_use]
pub fn row_width(row: &FlatRow, measure: impl Fn(&str) -> f32) -> f32 {
    let depth = row.depth.min(MAX_ROW_INDENT_DEPTH);
    depth as f32 * ROW_INDENT_PER_DEPTH + measure(&row_label(row)) + ROW_WIDTH_SLACK
}

/// Width of the widest row in `rows`; `0.0` for an empty list.
///
/// Purity: pure for a pure `measure`.
#[must_use]
pub fn longest_row_width<'a>(
    rows: impl IntoIterator<Item = &'a FlatRow>,
    measure: impl Fn(&str) -> f32,
) -> f32 {
    rows.into_iter()
        .map(|row| row_width(row, &measure))
        .fold(0.0_f32, f32::max)
}

/// UI-016 auto-fit: the width that fits the longest visible row, clamped
/// to `[LEFT_PANEL_MIN_WIDTH, viewport_width × LEFT_PANEL_MAX_WIDTH_FRACTION]`.
///
/// Shares [`clamped_width`] with the pane-grid resize reduction, so the
/// degenerate tiny-viewport case behaves identically in both paths: when the
/// 20 % cap falls *below* the minimum, the minimum wins.
///
/// Purity: pure for a pure `measure`.
#[must_use]
pub fn auto_fit_width(rows: &[FlatRow], viewport_width: f32, measure: impl Fn(&str) -> f32) -> f32 {
    clamped_width(
        longest_row_width(rows, &measure),
        LEFT_PANEL_MIN_WIDTH,
        viewport_width * LEFT_PANEL_MAX_WIDTH_FRACTION,
    )
}

/// The width the panel displays for a remembered width ([`None`] before the
/// first fit), clamped the same way as [`auto_fit_width`].
///
/// Purity: pure.
#[must_use]
pub fn panel_width(remembered: Option<f32>, viewport_width: f32) -> f32 {
    clamped_width(
        remembered.unwrap_or(LEFT_PANEL_DEFAULT_WIDTH),
        LEFT_PANEL_MIN_WIDTH,
        viewport_width * LEFT_PANEL_MAX_WIDTH_FRACTION,
    )
}

/// Whether a row renders as selected.
///
/// **Directory rows are never highlighted**; a file row is selected when it
/// is in the multi-select set or is the single selected file.
///
/// Purity: pure.
#[must_use]
pub fn is_row_selected(
    row: &FlatRow,
    selected_file: Option<&Path>,
    selected_files: &[PathBuf],
) -> bool {
    !row.is_dir && (selected_files.contains(&row.path) || selected_file == Some(row.path.as_path()))
}

/// The intent a left-click on a row produces: a directory row selects the
/// directory and toggles its expansion; a file row selects the file.
///
/// The `multi` flag is passed in explicitly because iced's press message
/// carries no modifier state.
///
/// Purity: pure.
#[must_use]
pub fn row_click_command(row: &FlatRow, multi: bool) -> UserCommand {
    if row.is_dir {
        UserCommand::SelectDirectory {
            path: row.path.clone(),
            toggle_expand: true,
        }
    } else {
        UserCommand::SelectFile {
            path: row.path.clone(),
            multi,
        }
    }
}

/// The intent a double-click on a row produces.
///
/// Double-click **opens a file** in the editor and is a deliberate **no-op
/// on a directory**: one double-click also fires the single-click handler,
/// so a second expand toggle would undo the first.
///
/// Purity: pure.
#[must_use]
pub fn row_double_click_command(row: &FlatRow) -> Option<UserCommand> {
    if row.is_dir {
        None
    } else {
        Some(UserCommand::OpenInEditor(row.path.clone()))
    }
}

/// Context-menu rows for a **directory** row, in menu order (UI-006),
/// followed by the VFS-122 folder skills.
///
/// Purity: pure.
#[must_use]
pub fn directory_menu_items(path: &Path, folder_skills: &[SkillFile]) -> Vec<MenuItem<Message>> {
    let mut items = vec![
        MenuItem::new(
            strings::SHOW_IN_EXPLORER_ACTION,
            Message::Command(UserCommand::ShowInExplorer(path.to_path_buf())),
        ),
        MenuItem::new(
            strings::COPY_PATH_ACTION,
            Message::Command(UserCommand::CopyPath(path.to_path_buf())),
        ),
        MenuItem::new(
            strings::RENAME_ACTION,
            Message::Command(UserCommand::Rename(path.to_path_buf())),
        ),
        MenuItem::new(
            strings::MOVE_ACTION,
            Message::Command(UserCommand::Move(path.to_path_buf())),
        ),
        MenuItem::new(
            strings::CREATE_DIRECTORY_ACTION,
            Message::Command(UserCommand::CreateDirectory {
                parent: path.to_path_buf(),
            }),
        ),
        MenuItem::new(
            strings::NEW_DOCUMENT_ACTION,
            Message::Command(UserCommand::CreateDocument {
                parent: path.to_path_buf(),
            }),
        ),
        MenuItem::new(
            strings::DELETE_ACTION,
            Message::Command(UserCommand::Delete(path.to_path_buf())),
        ),
    ];

    items.extend(folder_skills.iter().map(|skill| {
        MenuItem::new(
            skill.name.clone(),
            Message::Tree(TreeMessage::SkillRunRequested {
                skill: skill.path.clone(),
                target_dir: Some(path.to_path_buf()),
                target_file: Some(path.to_path_buf()),
            }),
        )
    }));

    items
}

/// Context-menu rows for a **single-selected file**, in menu order
/// (UI-007, UI-008, UI-009..UI-014, UI-020).
///
/// `typst_available` gates "Save as PDF…": the row is absent, not
/// disabled, when the `typst` CLI is missing.
///
/// Purity: pure.
#[must_use]
pub fn file_menu_items(
    path: &Path,
    note_skills: &[SkillFile],
    typst_available: bool,
) -> Vec<MenuItem<Message>> {
    let mut items = vec![
        MenuItem::new(
            strings::EDIT_BUTTON,
            Message::Command(UserCommand::OpenInEditor(path.to_path_buf())),
        ),
        MenuItem::new(
            strings::SHOW_IN_EXPLORER_ACTION,
            Message::Command(UserCommand::ShowInExplorer(path.to_path_buf())),
        ),
        MenuItem::new(
            strings::COPY_PATH_ACTION,
            Message::Command(UserCommand::CopyPath(path.to_path_buf())),
        ),
        MenuItem::new(
            strings::RUN_AS_PROMPT_ACTION,
            Message::Tree(TreeMessage::RunAsPromptRequested {
                path: path.to_path_buf(),
            }),
        ),
        MenuItem::new(
            strings::PRINT_ACTION,
            Message::Tree(TreeMessage::PrintRequested {
                path: path.to_path_buf(),
            }),
        ),
    ];

    if typst_available {
        items.push(MenuItem::new(
            strings::SAVE_AS_PDF_ACTION,
            Message::Command(UserCommand::SaveAsPdf(path.to_path_buf())),
        ));
    }

    items.push(MenuItem::new(
        strings::RENAME_ACTION,
        Message::Command(UserCommand::Rename(path.to_path_buf())),
    ));
    items.push(MenuItem::new(
        strings::MOVE_ACTION,
        Message::Command(UserCommand::Move(path.to_path_buf())),
    ));
    items.push(MenuItem::new(
        strings::DELETE_ACTION,
        Message::Command(UserCommand::Delete(path.to_path_buf())),
    ));

    items.extend(note_skills.iter().map(|skill| {
        MenuItem::new(
            skill.name.clone(),
            Message::Tree(TreeMessage::SkillRunRequested {
                skill: skill.path.clone(),
                target_dir: None,
                target_file: Some(path.to_path_buf()),
            }),
        )
    }));

    items
}

/// Context-menu rows for a file inside a **multi-selection**
/// (UI-023, UI-024, UI-025).
///
/// `paths` is the whole selection; the reducer receives it whole because
/// one menu row can only emit one message.
///
/// Purity: pure.
#[must_use]
pub fn multi_select_menu_items(paths: &[PathBuf]) -> Vec<MenuItem<Message>> {
    vec![
        MenuItem::new(
            strings::MERGE_ACTION,
            Message::Command(UserCommand::MergePrompt(paths.to_vec())),
        ),
        MenuItem::new(
            strings::DELETE_ACTION,
            Message::Tree(TreeMessage::DeleteSelectedRequested {
                paths: paths.to_vec(),
            }),
        ),
    ]
}

/// Everything the row context menus need beyond the row itself.
///
/// A tiny borrow-carrying parameter object so the menu builders stay pure
/// functions rather than methods reaching into a live app.
#[derive(Clone, Copy, Debug, Default)]
pub struct MenuContext<'a> {
    /// Skills offered on directory rows (VFS-122).
    pub folder_skills: &'a [SkillFile],
    /// Skills offered on file rows (VFS-121).
    pub note_skills: &'a [SkillFile],
    /// Whether the `typst` CLI is on PATH (gates "Save as PDF...").
    pub typst_available: bool,
}

/// The context-menu rows for `row`, choosing the directory / multi-select /
/// single-file menu: the multi-select menu applies only when more than one
/// file is selected *and* this path is one of them.
///
/// Purity: pure.
#[must_use]
pub fn context_menu_items(
    row: &FlatRow,
    selected_files: &[PathBuf],
    context: MenuContext<'_>,
) -> Vec<MenuItem<Message>> {
    if row.is_dir {
        return directory_menu_items(&row.path, context.folder_skills);
    }
    let is_multi = selected_files.len() > 1 && selected_files.iter().any(|path| path == &row.path);
    if is_multi {
        multi_select_menu_items(selected_files)
    } else {
        file_menu_items(&row.path, context.note_skills, context.typst_available)
    }
}

/// Resolves the skill directory for `sub` from the "System" content
/// library, falling back to `fallback` when that library is not configured.
///
/// This is the **read-only** path: `AppConfig::list_folder_skills()` /
/// `list_note_skills()` create their directories as a side effect, which a
/// per-frame view must not do ([RUST-006]).
///
/// Purity: pure.
#[must_use]
pub fn skill_dir(
    libraries: &[ContentLibrary],
    sub: &str,
    fallback: impl FnOnce() -> PathBuf,
) -> PathBuf {
    libraries
        .iter()
        .find(|lib| lib.name == SYSTEM_LIBRARY_NAME)
        .map(|lib| {
            PathBuf::from(&lib.root_folder)
                .join(SKILLS_SUBDIR)
                .join(sub)
        })
        .unwrap_or_else(fallback)
}

/// The flattened rows the panel renders.
///
/// Prefers `FastMdApp::cached_tree_rows` while the tree is clean but never
/// writes to it: populating and invalidating the cache is reducer state, and
/// a view that mutated it would break the pure-view rule.
///
/// Purity: read-only over `app`.
#[must_use]
pub fn tree_rows(app: &FastMdApp) -> Arc<Vec<FlatRow>> {
    if !app.selection().tree_dirty()
        && let Some(cached) = &app.cached_tree_rows
    {
        return Arc::clone(cached);
    }

    let root = build_workspace_tree(app);
    let mut rows = Vec::new();
    if !root.children.is_empty() {
        flatten_tree(&root, 0, app.selection().expanded_dirs(), &mut rows);
    }
    Arc::new(rows)
}

// ---------------------------------------------------------------------------
// Workspace-tree construction.
//
// All four functions are framework-free: their signatures and bodies mention
// only `TreeNode` (owned by `crate::ui::app`), `ContentLibrary`,
// `Path`/`PathBuf` and `String`.
// ---------------------------------------------------------------------------

/// Recursively removes directory nodes that do not contain any child files or subdirectories.
///
/// This is the UI-018 prune rule.
fn prune_empty_dirs(node: &mut TreeNode) {
    node.children.retain(|_, child| {
        if child.is_dir {
            prune_empty_dirs(child);
            !child.children.is_empty()
        } else {
            true
        }
    });
}

/// Finds the content library whose root is a prefix of `path`, returning the
/// library and the library-relative remainder of the path.
fn library_for_path<'a>(
    libraries: &'a [ContentLibrary],
    path: &'a std::path::Path,
) -> Option<(&'a ContentLibrary, &'a std::path::Path)> {
    libraries.iter().find_map(|lib| {
        let root = std::path::Path::new(&lib.root_folder);
        path.strip_prefix(root).ok().map(|rel| (lib, rel))
    })
}

/// Inserts `path` under the content library that contains it, creating the
/// intermediate directory nodes on the way down.
///
/// One walk serves both passes over the workspace (discovered files,
/// discovered directories); the only difference is whether the *final*
/// component is a file or a directory, selected by `leaf_is_dir`. Paths
/// outside every library root are ignored.
fn insert_path_into_tree(
    root_node: &mut TreeNode,
    libraries: &[ContentLibrary],
    path: &std::path::Path,
    leaf_is_dir: bool,
) {
    let Some((lib, rel_path)) = library_for_path(libraries, path) else {
        return;
    };
    let Some(current_node_ref) = root_node.children.get_mut(&lib.name) else {
        return;
    };
    let mut current_node = current_node_ref;
    let mut current_path = std::path::PathBuf::from(&lib.root_folder);

    let components: Vec<_> = rel_path.components().collect();
    for (i, comp) in components.iter().enumerate() {
        let name = comp.as_os_str().to_string_lossy().into_owned();
        current_path = current_path.join(&name);
        let is_dir = if i == components.len() - 1 {
            leaf_is_dir
        } else {
            true
        };

        current_node
            .children
            .entry(name.clone())
            .or_insert_with(|| TreeNode::new(name.clone(), current_path.clone(), is_dir));
        match current_node.children.get_mut(&name) {
            Some(n) => current_node = n,
            None => break,
        }
    }
}

/// Builds the `TreeNode` hierarchy from content libraries, discovered files, and tag filters.
///
/// UI-017 ordering and UI-018 tag pruning are applied by [`flatten_tree`]
/// over the result. The tracing span lives in the `ui_iced.*` namespace.
#[tracing::instrument(skip_all, name = "ui_iced.left.build_workspace_tree", level = "debug")]
#[must_use]
pub fn build_workspace_tree(app: &FastMdApp) -> TreeNode {
    let filtered_files: Vec<&std::path::PathBuf> = app
        .file_processor()
        .all_files
        .iter()
        .filter(|p| {
            if let Some(active_tag) = &app.tags().selected_tag {
                if let Some(tags) = app.tags().file_tags().get(*p) {
                    tags.contains(active_tag)
                } else {
                    false
                }
            } else {
                true
            }
        })
        .collect();

    let mut root_node = TreeNode::new(
        strings::DEFAULT_WORKSPACE_NAME.to_string(),
        std::path::PathBuf::new(),
        true,
    );

    for lib in app.content_libraries() {
        let lib_node_name = lib.name.clone();
        let lib_root_path = std::path::PathBuf::from(&lib.root_folder);
        root_node
            .children
            .entry(lib_node_name.clone())
            .or_insert_with(|| TreeNode::new(lib_node_name.clone(), lib_root_path, true));
    }

    for path in filtered_files {
        insert_path_into_tree(&mut root_node, app.content_libraries(), path, false);
    }

    if app.tags().selected_tag.is_none() {
        for dir in app.file_processor().all_dirs.iter() {
            insert_path_into_tree(&mut root_node, app.content_libraries(), dir, true);
        }
    } else {
        prune_empty_dirs(&mut root_node);
    }

    root_node
}

/// The note-skill listing the file menu and the tab-strip context menu both draw
/// from.
///
/// Read through the **read-only** path because `AppConfig::list_note_skills()`
/// `create_dir_all`s as a side effect, which a per-frame view must not do
/// ([RUST-006]) — the same rule [`LeftPanelViewModel::from_app`] follows.
///
/// Hoisted out of both panels so [`ViewModels::from_app`](crate::ui_iced::app::ViewModels)
/// can read the directory once per rebuild instead of once per panel.
///
/// Purity: reads the filesystem; otherwise pure.
#[must_use]
pub fn note_skill_files(app: &FastMdApp) -> Vec<SkillFile> {
    AppConfig::list_skill_files_in(&skill_dir(
        app.content_libraries(),
        NOTE_SKILLS_SUBDIR,
        AppConfig::get_skills_note_dir,
    ))
}

/// Everything the left panel needs to render, snapshotted off `FastMdApp`.
///
/// Mirrors [`super::top::TopBarViewModel`]: the widget tree reads only this
/// struct, which is what makes the panel unit-testable without a live iced
/// runtime and keeps the view decoupled from where each field comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct LeftPanelViewModel {
    /// The visible tree rows, in flatten order (UI-017/UI-018 preserved).
    pub rows: Arc<Vec<FlatRow>>,
    /// The search box's current text.
    pub query: String,
    /// Whether a content search is active — results replace the tree
    /// (UI-050).
    pub is_searching: bool,
    /// The committed search results.
    pub results: Arc<Vec<SearchResultEntry>>,
    /// The single selected file.
    pub selected_file: Option<PathBuf>,
    /// The multi-selected files, sorted so emitted commands are
    /// deterministic.
    pub selected_files: Vec<PathBuf>,
    /// The visible file rows that are PDF-backed — rendered with the
    /// `error` token (REQ-450).
    pub pdf_backed: HashSet<PathBuf>,
    /// Skills offered on directory rows (VFS-122).
    pub folder_skills: Vec<SkillFile>,
    /// Skills offered on file rows (VFS-121).
    pub note_skills: Vec<SkillFile>,
    /// Whether the `typst` CLI is available (gates "Save as PDF...").
    pub typst_available: bool,
    /// Whether the next row click extends the selection instead of
    /// replacing it (UI-022).
    ///
    /// The panel itself cannot know this: iced's `MouseArea::on_press` takes
    /// a fixed `Message` and carries no modifier state. The value is
    /// produced by the reducer — [`from_app`](LeftPanelViewModel::from_app)
    /// defaults it to `false` because it has no modifier input, and
    /// `ViewModels::from_app` overrides it from
    /// [`OverlayState::modifiers`](crate::ui_iced::app::OverlayState::modifiers),
    /// which the subscription bridge's keyboard lane keeps in step with the
    /// physical Shift key.
    pub multi_select: bool,
}

impl LeftPanelViewModel {
    /// Snapshots the left panel's inputs off `app`.
    ///
    /// [`multi_select`](Self::multi_select) is reported `false`: this
    /// function takes no modifier input, so the caller that does own it
    /// (`ViewModels::from_app`) overwrites the field after calling this.
    /// The signature is deliberately unchanged — existing call sites and
    /// tests construct the snapshot from `&FastMdApp` alone.
    pub fn from_app(app: &FastMdApp) -> Self {
        let rows = tree_rows(app);

        let pdf_backed = rows
            .iter()
            .filter(|row| !row.is_dir && app.pdf_backing_tracker().is_pdf_backed(&row.path))
            .map(|row| row.path.clone())
            .collect();

        let mut selected_files: Vec<PathBuf> =
            app.selection().selected_files().iter().cloned().collect();
        selected_files.sort();

        Self {
            rows,
            query: app.search().query().to_string(),
            is_searching: app.search().is_searching(),
            results: app.search().results_arc(),
            selected_file: app.selection().selected_file().cloned(),
            selected_files,
            pdf_backed,
            folder_skills: AppConfig::list_skill_files_in(&skill_dir(
                app.content_libraries(),
                FOLDER_SKILLS_SUBDIR,
                AppConfig::get_skills_folder_dir,
            )),
            note_skills: AppConfig::list_skill_files_in(&skill_dir(
                app.content_libraries(),
                NOTE_SKILLS_SUBDIR,
                AppConfig::get_skills_note_dir,
            )),
            typst_available: fastmd_pdf::is_typst_available(),
            multi_select: false,
        }
    }

    /// Whether `path` is part of a multi-selection (more than one file
    /// selected and this path among them) — the gate that swaps the file
    /// context menu for the multi-select one.
    #[must_use]
    pub fn is_multi_selected(&self, path: &Path) -> bool {
        self.selected_files.len() > 1
            && self
                .selected_files
                .iter()
                .any(|file| file.as_path() == path)
    }

    /// The context-menu rows for the row whose menu is open.
    #[must_use]
    pub fn menu_items_for(&self, row: &FlatRow) -> Vec<MenuItem<Message>> {
        context_menu_items(
            row,
            &self.selected_files,
            MenuContext {
                folder_skills: &self.folder_skills,
                note_skills: &self.note_skills,
                typst_available: self.typst_available,
            },
        )
    }

    /// The auto-fit width for this snapshot's rows (UI-016).
    #[must_use]
    pub fn auto_fit(&self, viewport_width: f32, measure: impl Fn(&str) -> f32) -> f32 {
        auto_fit_width(&self.rows, viewport_width, measure)
    }
}

/// Renders the left file-tree panel from a view-model snapshot.
///
/// Emitted messages:
/// * row click → [`Message::Command`] `SelectDirectory` / `SelectFile`
/// * file double-click → [`Message::Command`] `OpenInEditor`
/// * row right-click → [`Message::Tree`] `ContextMenuOpened`
/// * pointer motion → [`Message::Layout`] `CursorMoved` (the menu anchor)
/// * search box → [`Message::Search`] `QueryChanged` / `Submitted` / `Cleared`
///
/// The search row is always allocated; the clear button stays present and
/// merely inert when nothing is being searched, so the widget-tree shape does
/// not change across the search transition.
///
/// The search field takes `Length::Fill`: iced lays the input out inside the
/// `Row` itself, so no hand-computed width is needed.
pub fn left_panel(view_model: &LeftPanelViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let header = text(strings::WORKSPACE_HEADER)
        .size(HEADER_SIZE)
        .color(palette.text_strong);

    let search_field = text_input(strings::SEARCH_PLACEHOLDER, &view_model.query)
        .on_input(|text| Message::Search(SearchMessage::QueryChanged { text }))
        .on_submit(Message::Search(SearchMessage::Submitted {
            query: view_model.query.clone(),
        }));

    let search_trigger = button(
        text(strings::ICON_MAGNIFYING_GLASS)
            .font(phosphor_font())
            .size(ROW_TEXT_SIZE),
    )
    .on_press(Message::Search(SearchMessage::Submitted {
        query: view_model.query.clone(),
    }));

    let clear_trigger = button(
        text(strings::ICON_X)
            .font(phosphor_font())
            .size(ROW_TEXT_SIZE),
    )
    .on_press_maybe(
        view_model
            .is_searching
            .then_some(Message::Search(SearchMessage::Cleared)),
    );

    let search_row = Row::with_children(vec![
        search_field.width(Length::Fill).into(),
        search_trigger.into(),
        clear_trigger.into(),
    ])
    .spacing(4.0)
    .align_y(Alignment::Center);

    let body: Element<'_, Message> = if view_model.is_searching {
        search_results(view_model)
    } else {
        tree_body(view_model)
    };

    Column::with_children(vec![header.into(), search_row.into(), body])
        .spacing(PANEL_PADDING)
        .padding(PANEL_PADDING)
        .into()
}

/// The tree branch of the panel body: the virtual row list, or the
/// empty-workspace placeholder.
fn tree_body<'a>(view_model: &'a LeftPanelViewModel) -> Element<'a, Message> {
    let palette = Palette::fastmd_dark();

    if view_model.rows.is_empty() {
        return container(
            text(strings::NO_MARKDOWN_FILES)
                .size(ROW_TEXT_SIZE)
                .color(palette.text_muted),
        )
        .padding(PANEL_PADDING)
        .into();
    }

    let rows = view_model
        .rows
        .iter()
        .fold(Column::<Message>::new().spacing(0.0), |rows, row| {
            rows.push(tree_row(row, view_model))
        });

    scrollable(rows).height(Length::Fill).into()
}

/// One tree row: indented label plus the full click / double-click /
/// right-click / motion interaction.
fn tree_row<'a>(row: &'a FlatRow, view_model: &'a LeftPanelViewModel) -> Element<'a, Message> {
    let palette = Palette::fastmd_dark();
    let selected = is_row_selected(
        row,
        view_model.selected_file.as_deref(),
        &view_model.selected_files,
    );
    let pdf_backed = view_model.pdf_backed.contains(&row.path);

    let label = text(row_label(row))
        .size(ROW_TEXT_SIZE)
        .color(row_text_color(&palette, pdf_backed));

    let indent = row.depth.min(MAX_ROW_INDENT_DEPTH) as f32 * ROW_INDENT_PER_DEPTH;
    let row_surface = container(label)
        .width(Length::Fill)
        .padding(Padding {
            top: ROW_PADDING_Y,
            right: ROW_PADDING_X,
            bottom: ROW_PADDING_Y,
            left: ROW_PADDING_X + indent,
        })
        .style(move |_theme| container::Style {
            background: selected.then_some(Background::Color(palette.accent)),
            ..container::Style::default()
        });

    let trigger = crate::ui_iced::overlay::context_menu_trigger(
        row_surface,
        Message::Tree(TreeMessage::ContextMenuOpened {
            path: row.path.clone(),
        }),
    )
    .on_press(Message::Command(row_click_command(
        row,
        view_model.multi_select,
    )));

    match row_double_click_command(row) {
        Some(command) => trigger.on_double_click(Message::Command(command)).into(),
        None => trigger.into(),
    }
}

/// Label colour for a tree row: the `error` token on PDF-backed files
/// (REQ-450), the default body token otherwise.
///
/// Purity: pure.
#[must_use]
pub fn row_text_color(palette: &Palette, pdf_backed: bool) -> iced::Color {
    if pdf_backed {
        palette.error
    } else {
        palette.text
    }
}

/// The search-result branch of the panel body (UI-050).
fn search_results<'a>(view_model: &'a LeftPanelViewModel) -> Element<'a, Message> {
    let palette = Palette::fastmd_dark();

    if view_model.results.is_empty() {
        return container(
            text(strings::SEARCH_NO_RESULTS)
                .size(ROW_TEXT_SIZE)
                .color(palette.text_muted),
        )
        .padding(PANEL_PADDING)
        .into();
    }

    let cards = view_model
        .results
        .iter()
        .fold(Column::<Message>::new().spacing(2.0), |cards, entry| {
            cards.push(search_result_card(entry))
        });

    scrollable(cards).height(Length::Fill).into()
}

/// One search-result card: title + match count, the library-relative path
/// and the matching snippet.
fn search_result_card<'a>(entry: &'a SearchResultEntry) -> Element<'a, Message> {
    let palette = Palette::fastmd_dark();

    let mut title = Row::with_children(vec![
        text(&entry.file_name)
            .size(ROW_TEXT_SIZE)
            .color(palette.text_strong)
            .into(),
    ]);
    if entry.match_count > 1 {
        title = title.push(
            text(strings::format_match_count(entry.match_count))
                .size(SECONDARY_TEXT_SIZE)
                .color(palette.text_faint),
        );
    }

    let mut lines = Column::<Message>::new().spacing(2.0).push(title);

    if !entry.relative_path.is_empty() && entry.relative_path != entry.file_name {
        lines = lines.push(
            text(&entry.relative_path)
                .size(SECONDARY_TEXT_SIZE)
                .color(palette.text_faint),
        );
    }

    if !entry.snippet.is_empty() {
        lines = lines.push(
            text(&entry.snippet)
                .size(SECONDARY_TEXT_SIZE)
                .font(italic_font())
                .color(palette.accent_text_subtle),
        );
    }

    mouse_area(
        container(lines)
            .width(Length::Fill)
            .height(Length::Fixed(SEARCH_ROW_HEIGHT))
            .padding(Padding::from([4.0, 6.0])),
    )
    .on_press(Message::Command(UserCommand::SelectFile {
        path: entry.path.clone(),
        multi: false,
    }))
    .into()
}

/// Italic variant of the default UI font — iced's `Text` has no `.italic()`
/// helper, so the style rides on the [`Font`] (same trick as
/// [`super::top::indexing_status_font`]).
fn italic_font() -> Font {
    Font {
        style: iced::font::Style::Italic,
        ..Font::DEFAULT
    }
}
