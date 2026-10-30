//! The file-tree and document-tab context menus rendered by the `hoarfrost`
//! menu engine (integration increment INT-3b, `UIF-CMENU-001`/`-002`).
//!
//! Both context menus are now [`Menu`](hoarfrost::surface::menu::model::Menu)
//! values whose openness is owned by a
//! [`MenuCascade`](hoarfrost::surface::menu::cascade::MenuCascade), whose card
//! comes from [`anchor::solve`](hoarfrost::geometry::anchor::solve) and whose
//! rows are [`MenuRow`](hoarfrost::surface::menu::view::MenuRow) widgets. The
//! card chrome, the row grid and the open-card rectangle walk are shared with
//! the hamburger through [`menu_surface`]; only the anchor and the row
//! vocabulary are specific to this module.
//!
//! # What this closes over the previous path
//!
//! | Gap | How it is closed |
//! |---|---|
//! | No separators — the card was a uniform column of buttons, so §4.9.1's functional groups were carried by row order alone | Rows carry a [`ContextGroup`](crate::ui_iced::context_menu::ContextGroup) and [`menu_from_rows`](crate::ui_iced::context_menu::menu_from_rows) emits a real `Row::separator()` at every group boundary, so [`Menu::groups`](hoarfrost::surface::menu::model::Menu::groups) reports the groups the spec names |
//! | Destructive rows were visually indistinguishable | [`ContextGroup::Destructive`](crate::ui_iced::context_menu::ContextGroup::Destructive) marks them, which is what [`Menu::validate`](hoarfrost::surface::menu::model::Menu::validate) requires to be the final group |
//! | Placement was a hand-clamped point | [`Anchor::Pointer`](hoarfrost::geometry::anchor::Anchor::Pointer) runs the framework's quadrant-flip and viewport-clamp rules (`UIF-CMENU-001`) |
//! | The registered surface was an empty region | [`open_card_rects`](crate::ui_iced::context_menu::open_card_rects) solves the real card rectangle and the reducer registers it, so `route_pointer` answers `PointerTarget::Surface` inside the card |
//!
//! # Row intent is data, not a resolver
//!
//! A context-menu row's intent depends on the *target* it was opened on, so it
//! cannot be resolved from the row key alone the way the hamburger's can. Rather
//! than duplicate every command builder into a `match` on key, the row carries
//! its own self-contained [`Message`](crate::ui_iced::message::Message)
//! ([FMD-068]) and the surface keeps a
//! key → message routing table built from the same rows the card was built
//! from. The table and the card therefore cannot disagree: they are two views of
//! one list.
//!
//! # Labels are not touched
//!
//! Every label comes verbatim from [`crate::ui::strings`]. The Host `SPEC.md`
//! Literal Pin Table (`UIF-PRIN-007`) pins `'Open in Editor'`,
//! `'Show in File Explorer'`, `'Create Directory ...'`, `'Close Others'`,
//! `'Close All'` and `'Copy Path'`; nothing here re-cases, re-punctuates or
//! renames a label, pinned or otherwise.
//!
//! Unit tests live in the sibling `context_menu_tests.rs` sidecar.

#[cfg(test)]
#[path = "context_menu_tests.rs"]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use iced::{Element, Point, Rectangle, Size};

use hoarfrost::geometry::anchor::{self, Anchor, Placement};
use hoarfrost::surface::menu::cascade::MenuCascade;
use hoarfrost::surface::menu::model::{Menu, MenuId, Row};

use crate::config::SkillFile;
use crate::ui_iced::menu_surface;
use crate::ui_iced::message::{Message, TreeMessage};

/// Prefix of every context-menu root [`MenuId`].
///
/// The root id encodes the menu's **target** (see [`root_id`]) rather than
/// being one constant, because
/// [`MenuCascade::open`] refuses a re-open of the same
/// tree kind over the same root id with `AlreadyOpen`. A single shared id
/// would therefore leave the card anchored at the *previous* target's pointer
/// when the user right-clicks a second file.
pub const CONTEXT_MENU_ID_PREFIX: &str = "context";

/// Key prefix carrying a skill's own path on its row.
///
/// Skill rows are generated from a directory listing, so their identity is
/// the skill file rather than a name that could collide with a fixed row.
pub const SKILL_KEY_PREFIX: &str = "skill:";

/// Row key: open the target in the external editor.
pub const KEY_EDIT: &str = "edit";
/// Row key: reveal the target in the OS file manager.
pub const KEY_SHOW_IN_EXPLORER: &str = "show_in_explorer";
/// Row key: copy the target's full path.
pub const KEY_COPY_PATH: &str = "copy_path";
/// Row key: rename the target.
pub const KEY_RENAME: &str = "rename";
/// Row key: move the target.
pub const KEY_MOVE: &str = "move";
/// Row key: create a directory inside the target folder.
pub const KEY_CREATE_DIRECTORY: &str = "create_directory";
/// Row key: create a new document inside the target folder.
pub const KEY_NEW_DOCUMENT: &str = "new_document";
/// Row key: delete the target (or the whole selection).
pub const KEY_DELETE: &str = "delete";
/// Row key: feed the target file to the agent as a prompt.
pub const KEY_RUN_AS_PROMPT: &str = "run_as_prompt";
/// Row key: print the target file.
pub const KEY_PRINT: &str = "print";
/// Row key: render the target file to PDF via Typst.
pub const KEY_SAVE_AS_PDF: &str = "save_as_pdf";
/// Row key: merge the whole selection into one prompt.
pub const KEY_MERGE: &str = "merge";
/// Row key: close this tab only.
pub const KEY_CLOSE_TAB: &str = "close_tab";
/// Row key: close every tab but this one.
pub const KEY_CLOSE_OTHERS: &str = "close_others";
/// Row key: close every open tab.
pub const KEY_CLOSE_ALL_TABS: &str = "close_all_tabs";
/// Row key: open the tab's document in the external editor.
pub const KEY_OPEN_IN_EDITOR: &str = "open_in_editor";

/// One §4.9.1 functional group of a context menu.
///
/// The group is **structural**, not decorative: [`menu_from_rows`] puts a
/// separator between two consecutive rows whose groups differ, and
/// [`ContextGroup::Destructive`] is the only group
/// [`Menu::validate`] requires to come last. The spec names the groups
/// differently per target; the variants below are the shared shape of those
/// four runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextGroup {
    /// The target's primary actions — "Open & Output" for a file,
    /// "Creation" for a folder, "Tab Lifecycle" for a tab, "Batch AI" for a
    /// multi-selection.
    Primary,
    /// AI and automation rows — "Run as prompt" and the VFS-121/122 skill
    /// rows (§4.9.1 "AI & Automation").
    Automation,
    /// Location and file-management rows — Rename, Move, Copy path, Show in
    /// File Explorer (§4.9.1 "File Management" / "Location").
    Management,
    /// Destructive actions, kept in the final group and never mixed with
    /// non-destructive ones (§4.9.1).
    Destructive,
}

impl ContextGroup {
    /// Whether activating a row in this group destroys user data, which is
    /// what makes the row render with the destructive treatment and forces it
    /// to be the menu's last group.
    #[must_use]
    pub const fn is_destructive(self) -> bool {
        matches!(self, Self::Destructive)
    }
}

/// One row of a file-tree or tab context menu, before it becomes a framework
/// [`Row`].
///
/// The row carries its own self-contained intent ([FMD-068]) so the panel
/// builders that produce it stay pure functions of the view-model, and so the
/// routing table can be derived from the same list the card is built from.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextRow {
    /// Stable row key, unique within the card.
    pub key: String,
    /// The row label, verbatim from [`crate::ui::strings`] or a skill name.
    pub label: String,
    /// The §4.9.1 functional group this row belongs to.
    pub group: ContextGroup,
    /// The self-contained intent the row publishes when activated.
    pub message: Message,
}

impl ContextRow {
    /// A row with a fixed key and label in group `group`.
    #[must_use]
    pub fn new(key: &str, label: &str, group: ContextGroup, message: Message) -> Self {
        Self {
            key: key.to_owned(),
            label: label.to_owned(),
            group,
            message,
        }
    }

    /// A VFS-121/122 skill row, keyed on the skill file's own path.
    ///
    /// Always lands in [`ContextGroup::Automation`], which is where §4.9.1
    /// puts contextual skills in every one of the four target menus.
    #[must_use]
    pub fn skill(skill: &SkillFile, target_dir: Option<PathBuf>, target_file: &Path) -> Self {
        Self {
            key: format!("{SKILL_KEY_PREFIX}{}", skill.path.display()),
            label: skill.name.clone(),
            group: ContextGroup::Automation,
            message: Message::Tree(TreeMessage::SkillRunRequested {
                skill: skill.path.clone(),
                target_dir,
                target_file: Some(target_file.to_path_buf()),
            }),
        }
    }

    /// Whether this row destroys user data.
    #[must_use]
    pub fn is_destructive(&self) -> bool {
        self.group.is_destructive()
    }
}

/// The workspace target a context menu was opened on.
///
/// These are exactly §4.9.1's four context targets. The variant is what
/// [`root_id`] folds into the menu's identity so that right-clicking a
/// different target replaces the live card instead of being refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextTarget {
    /// A folder row in the file tree.
    TreeDirectory {
        /// The folder's path.
        path: PathBuf,
    },
    /// A single-selected file row in the file tree.
    TreeFile {
        /// The file's path.
        path: PathBuf,
    },
    /// A file inside a multi-selection; `paths` is the whole selection,
    /// because one menu row can only emit one message.
    TreeMultiSelect {
        /// Every selected path, in selection order.
        paths: Vec<PathBuf>,
    },
    /// A document tab in the tab strip.
    Tab {
        /// The tab's index in the strip, which every tab command carries.
        index: usize,
        /// The document's path.
        path: PathBuf,
    },
}

impl ContextTarget {
    /// The target's own path — the folder for a directory target, the file
    /// for every other single-target menu, and the first selected path for a
    /// multi-selection.
    ///
    /// Used only for identity and diagnostics; the row builders take the paths
    /// they need explicitly.
    #[must_use]
    pub fn primary_path(&self) -> &Path {
        match self {
            Self::TreeDirectory { path } | Self::TreeFile { path } | Self::Tab { path, .. } => path,
            Self::TreeMultiSelect { paths } => paths
                .first()
                .map_or_else(|| Path::new(""), |path| path.as_path()),
        }
    }
}

/// The root [`MenuId`] identifying this target's menu tree.
///
/// Encodes the target so a re-open over a **different** target is not refused
/// as `AlreadyOpen`, while a re-open over the *same* target is. See
/// [`CONTEXT_MENU_ID_PREFIX`].
///
/// Purity: pure function of `target`.
#[must_use]
pub fn root_id(target: &ContextTarget) -> MenuId {
    let suffix = match target {
        ContextTarget::TreeDirectory { path } => format!("dir.{}", path.display()),
        ContextTarget::TreeFile { path } => format!("file.{}", path.display()),
        ContextTarget::TreeMultiSelect { paths } => {
            let joined = paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(";");
            format!("multi.{joined}")
        }
        ContextTarget::Tab { index, path } => format!("tab.{index}.{}", path.display()),
    };
    MenuId::new(format!("{CONTEXT_MENU_ID_PREFIX}.{suffix}"))
}

/// Builds the framework [`Menu`] for `rows`, inserting a separator at every
/// group boundary and marking the destructive group's rows destructive.
///
/// A leading or trailing separator is never emitted, and two rows of the same
/// group never get one between them, so [`Menu::groups`] returns exactly the
/// runs the builders declared.
///
/// Purity: pure function of `(id, rows)`.
#[must_use]
pub fn menu_from_rows(id: MenuId, rows: &[ContextRow]) -> Menu {
    let mut out: Vec<Row> = Vec::with_capacity(rows.len());
    let mut previous: Option<ContextGroup> = None;

    for row in rows {
        if previous.is_some_and(|previous| previous != row.group) && !out.is_empty() {
            out.push(Row::separator());
        }
        let model = Row::action(row.key.clone(), row.label.clone());
        out.push(if row.is_destructive() {
            model.destructive()
        } else {
            model
        });
        previous = Some(row.group);
    }

    Menu::new(id, out)
}

/// The row key → intent-message table for `rows`.
///
/// A later row claiming a key already present **overwrites** the earlier
/// entry, which is why the sidecar's `test_row_keys_are_unique` pins
/// uniqueness: two rows sharing a key would also share a focus id, and
/// [`Menu::validate`](hoarfrost::surface::menu::model::Menu::validate) would
/// report `DuplicateKey` long before this table silently mis-routed anything.
///
/// Purity: pure function of `rows`.
#[must_use]
pub fn routes_from_rows(rows: &[ContextRow]) -> HashMap<String, Message> {
    rows.iter()
        .map(|row| (row.key.clone(), row.message.clone()))
        .collect()
}

/// The card size for a context-menu card — the shared §4.8.1 grid.
///
/// Purity: pure function of `menu`.
#[must_use]
pub fn card_size(menu: &Menu) -> Size {
    menu_surface::card_size(menu)
}

/// Solves the root card's placement under the pointer (`UIF-CMENU-001`).
///
/// `anchor` is the root-space cursor position reported by
/// [`LayoutMessage::CursorMoved`](crate::ui_iced::message::LayoutMessage::CursorMoved)
/// just before the open request. The framework opens the card `+2, +2` from
/// it and flips to the other side of the cursor on either axis when the
/// preferred side cannot hold the card.
///
/// Purity: pure function of `(menu, anchor, viewport)`.
#[must_use]
pub fn root_placement(menu: &Menu, anchor: Point, viewport: Size) -> Placement {
    anchor::solve(
        Anchor::Pointer { point: anchor },
        card_size(menu),
        Rectangle::new(Point::ORIGIN, viewport),
    )
}

/// Solves the viewport rectangle of every open card, root first.
///
/// Context menus are flat today, so this returns one rectangle; the walk is
/// shared with the hamburger so a future submenu needs no new geometry code.
///
/// Purity: pure function of `(cascade, anchor, viewport)`.
#[must_use]
pub fn open_card_rects(cascade: &MenuCascade, anchor: Point, viewport: Size) -> Vec<Rectangle> {
    menu_surface::open_card_rects(cascade, viewport, |menu| {
        root_placement(menu, anchor, viewport)
    })
}

/// The message the click-away blocker publishes to dismiss a context menu.
///
/// Kept as a function rather than inlined at the mount site so the dismiss
/// path is a greppable symbol and the tree panel's existing
/// [`TreeMessage::ContextMenuClosed`] handling stays the single close route.
#[must_use]
pub fn dismiss_message() -> Message {
    Message::Tree(TreeMessage::ContextMenuClosed)
}

/// Renders the open context-menu cards above `base`, plus the click-away
/// blocker.
///
/// Delegates to [`menu_surface::overlay`] with [`dismiss_message`].
pub fn overlay<'a>(
    base: Element<'a, Message>,
    cascade: &MenuCascade,
    rects: &[Rectangle],
) -> Element<'a, Message> {
    menu_surface::overlay(base, cascade, rects, dismiss_message())
}
