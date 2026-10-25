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
//! * [`tree_row`] supplies **every geometric number** a row owns: the
//!   indent step, both insets, the label rung, the disclosure column and
//!   the compact row rung. The panel reads those constants instead of
//!   keeping parallel app-side numbers, and [`row_width`] delegates to
//!   [`tree_row::content_width`] outright. See the constant docs for each
//!   movement.
//!
//! # The `tree_row` composite is adopted for geometry and paint, not rendering
//!
//! `iced_pro::composite::tree_row` is a **pure geometry-and-style
//! module**. It resolves [`tree_row::TreeRowSlots`], [`tree_row::TreeRowStyle`]
//! and [`iced_pro::widget::button_widget::Layer`] rectangles; the only
//! thing that turns them into pixels is the gallery's canvas renderer
//! ([`gallery_tree`](iced_pro::gallery_tree)). There is no
//! `tree_row::view(..)` returning an `iced::Element`, so "use the
//! composite for row rendering" cannot be done from an iced widget tree
//! without writing a new widget in the framework.
//!
//! What *is* adopted here: the geometry above, the rest / hover /
//! selected bands from [`tree_row::resolve_style`] ([`row_paint`]), and
//! the label's role-based contrast lock from [`tree_row::locked_over`]
//! ([`row_label_colour`]). What is **not**, and why:
//!
//! * **The `Pressed` tier is feasible and is not built.** It is *not*
//!   blocked, contrary to an earlier note here: `mouse_area` pairs
//!   `on_press` with `on_release`
//!   (`iced_widget-0.14.2/src/mouse_area.rs:38` / `:45`), so a pressed
//!   band can be set *and* cleared. It is left unbuilt because a
//!   press-reactive row is an interaction change to the panel's primary
//!   affordance rather than a token pass, and that is a design call, not
//!   a missing primitive. Hover has no such question and **is** wired
//!   (INT-10): the very `mouse_area` the context-menu trigger builds on
//!   also carries `on_enter` / `on_exit`
//!   (`iced_widget-0.14.2/src/mouse_area.rs:106` / `:120`), and those
//!   crossings drive [`BaseState::Hovered`] into [`row_paint`].
//!
//!   The claim this replaces — that `mouse_area` "never reports hover
//!   into a `container` style", therefore the hover tier was
//!   unreachable — was true only of *automatic* style propagation,
//!   which is not what happens here. Nothing waits for iced to rewrite
//!   a style. The panel captures the crossing itself, holds it in
//!   reducer state keyed by **path**, and hands it to `resolve_style`
//!   as a plain value. INT-9 made the identical correction for the
//!   menu row's `Hovered` / `Unhovered` reporting.
//! * **The 2px selection bar and the depth guide lines are built** (INT-11).
//!   The note this replaces called them a canvas-painter requirement — "both
//!   come out of the composite as `Layer` rectangles for a canvas painter;
//!   drawing them means a `stack` per row or a canvas row, i.e. a rendering
//!   rewrite rather than a token pass". The first half was accurate and the
//!   inference drawn from it was not. A `Stack` per row *is* what they need,
//!   and a `Stack` of styled `container`s is not a rewrite: the row's
//!   existing padded `container` stays the base layer and the bar and guides
//!   are painted over it. Both colours already came out of
//!   [`tree_row::resolve_style`] (`guide`, `selection_bar`) and were simply
//!   never drawn. What the change really costs is per-row widget count, which
//!   [`row_overlay`] states and the sidecar pins.
//! * **The `[PDF]` badge.** **Landed in INT-12 as a chip.** §3B.6:883
//!   requires the PDF-backed distinction to survive the selection lock,
//!   and forbids it riding on badge text colour. The app previously tinted
//!   the whole label with `error` instead; the lock correctly overrode that
//!   tint on a selected row, so the state vanished. [`pdf_badge_chip`]
//!   now builds a `status.danger.fill` pill behind `text.strong` so the
//!   **fill** carries the mark. The chip is assembled from framework
//!   resolvers rather than taken from the badge atom, and it carries a
//!   recorded contrast shortfall against the selected band that the
//!   function's own doc states and the sidecar pins.
//! * **Width-relative indent compression.** **Landed in INT-12.**
//!   [`row_indent_pixels`] and [`row_guide_offsets`] both resolve through
//!   [`tree_row::indent_geometry`], so the step compresses proportionally
//!   instead of the depth being capped and the label being pushed off-screen.
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
//! formula `depth × 16 + label_width + 32` — the framework's own
//! [`tree_row::content_width`] — clamped to
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

use iced::widget::{
    Column, Row, Space, Stack, button, container, mouse_area, scrollable, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding};

use iced_pro::composite::tree_row;
use iced_pro::state::{BaseState, VisualState};
use iced_pro::token::contrast;
use iced_pro::token::shape;
use iced_pro::token::space;
use iced_pro::token::typography::{self, FAMILY_SANS};
use iced_pro::widget::badge;
use iced_pro::widget::badge_model::BadgeTone;
use iced_pro::widget::text_input as pro_text_input;

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
use crate::ui_iced::context_menu::{
    ContextGroup, ContextRow, ContextTarget, KEY_COPY_PATH, KEY_CREATE_DIRECTORY, KEY_DELETE,
    KEY_EDIT, KEY_MERGE, KEY_MOVE, KEY_NEW_DOCUMENT, KEY_PRINT, KEY_RENAME, KEY_RUN_AS_PROMPT,
    KEY_SAVE_AS_PDF, KEY_SHOW_IN_EXPLORER,
};
use crate::ui_iced::layout::clamped_width;
use crate::ui_iced::message::{Message, SearchMessage, TreeMessage};

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

/// Logical point size of the panel heading — `type.title_sm`
/// (`UIF-TOK-004`), the rung a pane's section title takes. Value
/// unchanged at `16`.
pub const HEADER_SIZE: f32 = typography::TITLE_SM.size;

/// Logical point size of tree rows and search-result titles — the size
/// the width pass measures at, and the row label rung the framework's own
/// tree renderer paints at ([`tree_row::LABEL_RUNG`], `type.ui_compact`,
/// `UIF-PANE-003`).
///
/// **Movement: 14 → 13.** The row used `BODY` (14), one rung above what
/// §4.3.2 names for a left-pane tree row. Dropping to `ui_compact` puts
/// the tree on the same rung as the tab labels, menu items and table
/// cells it sits beside, which is the whole reason the framework fixed
/// the row's label rung instead of leaving it to the host.
pub const ROW_TEXT_SIZE: f32 = tree_row::LABEL_RUNG.size;

/// Logical point size of the secondary search-result lines —
/// `type.caption` (`UIF-TOK-004`), the smallest rung on the scale and
/// the one that sits exactly on the [`typography::MIN_SIZE`] floor.
///
/// **Movement: 11 → 12 — a correctness fix, not a cosmetic one.** The
/// retired `11.0` is below the interface-wide 12px floor, which the type
/// scale forbids outright (`UIF-TOK-004`: nothing anywhere may render
/// smaller than 12px). It is the same class of defect INT-7a removed
/// from the bottom band's `min_size = 8`. Three lines of the search
/// result card carried it — the match-count suffix, the library-relative
/// path and the matching snippet — so every search result was rendering
/// illegal text.
///
/// The raise does not break the card: the card's height is now *derived*
/// from the rungs it holds (see [`SEARCH_ROW_HEIGHT`]) rather than being
/// a fixed number that had to accommodate them by luck.
pub const SECONDARY_TEXT_SIZE: f32 = typography::CAPTION.size;

/// Horizontal indent added per tree depth level, in logical points —
/// [`tree_row::INDENT_STEP`], which is `space.lg` (§3B.6's nominal
/// `depth × 16px`).
///
/// **Movement: 18 → 2.** The app's 18 was a parallel number with no
/// token behind it; the framework's row reserves 16. Reading the step
/// from `tree_row` rather than restating it is the point — the app's tree
/// and the framework's row cannot drift apart while both name the same
/// constant.
pub const ROW_INDENT_PER_DEPTH: f32 = tree_row::INDENT_STEP;

/// Deepest depth the **auto-fit measurement** charges for indentation.
///
/// **App-owned.** It is *not* the same rule as
/// [`tree_view::MAX_TREE_DEPTH`](iced_pro::composite::tree_view::MAX_TREE_DEPTH)
/// (1000), which is a stack-overflow guard on the framework's recursive
/// flatten.
///
/// **INT-12 narrowed what this constant governs, and the narrowing is
/// worth reading.** It used to cap the *drawn* indent
/// (`row_indent_pixels = depth.min(50) × 16`). That cap is gone: the
/// drawn indent now resolves through [`tree_row::indent_geometry`], which
/// bounds the indent by the row's width rather than by a depth number, so
/// a `50`-level cap on what is painted is no longer part of the model —
/// and never needed to be, because the width-relative tiers cannot push a
/// label out of the pane in the first place.
///
/// What remains is one use: [`row_width`]'s auto-fit term clamps the depth
/// it *requests* for a row, so a tree 200 levels deep does not ask the
/// pane for `3200px` of indent. **This is a known, deliberate
/// inconsistency and it fails safe.** The drawn indent ignores the cap;
/// the width request honours it. The consequence is that past depth 50 a
/// deep tree asks for less width than its nominal indent would want, so it
/// compresses somewhat more than it strictly had to. The `96px` label
/// floor still holds at every depth — that is guaranteed by the geometry,
/// not by this constant — so the effect is cosmetic at depths no real
/// workspace reaches. Removing the clamp from the auto-fit term is the
/// clean follow-on; it changes pane-width behaviour, which is outside this
/// increment.
pub const MAX_ROW_INDENT_DEPTH: usize = 50;

/// Width added on top of a measured label — the row's **fixed horizontal
/// furniture**, read from the framework's own row constants rather than
/// estimated: the two [`tree_row::ROW_INSET_X`] edges plus the
/// [`tree_row::CHEVRON_COLUMN`] every row reserves for its disclosure
/// control.
///
/// **Movement: 40 → 32.** The retired 40 was "padding, selection frame
/// and scroll gutter" written as one guess. `tree_row::content_width`
/// decomposes the same idea into the pieces the framework actually
/// reserves, and [`row_width`] now delegates to it so the app's auto-fit
/// and the framework's content-width rule are the same rule.
///
/// The app bakes the caret glyph into the measured label (see
/// [`row_label`]), so the reserved chevron column overlaps glyph width
/// that is already counted. That is deliberate: it leaves ~16px of
/// headroom over the real content instead of clipping a long name to the
/// pixel.
pub const ROW_WIDTH_SLACK: f32 =
    tree_row::ROW_INSET_X + tree_row::ROW_INSET_X + tree_row::CHEVRON_COLUMN;

/// Padding inside the panel body — `space.xs` (`UIF-TOK-001`).
///
/// **Movement: 6 → 4.** `6` is not on the base-4 spacing grid; in the
/// token set that number exists only as `shape::RADIUS_CARD`, a corner
/// radius. The old value sat exactly midway between `XS` (4) and `SM`
/// (8), so the tie was broken on *role* exactly as INT-7a broke
/// `BAND_PADDING`: `SM` is a sibling-control **gap**, `XS` is the
/// compact **inner padding** rung — which is what a panel body's inset
/// is.
pub const PANEL_PADDING: f32 = space::XS;

/// Vertical padding of one tree row — [`tree_row::ROW_INSET_Y`], which
/// is `space.xxs`. Value unchanged at `2`.
pub const ROW_PADDING_Y: f32 = tree_row::ROW_INSET_Y;

/// Horizontal padding of one tree row, before the depth indent is added —
/// [`tree_row::ROW_INSET_X`], which is `space.xs` (§4.3.2's "4px
/// `space.xs` horizontal inset").
///
/// **Movement: 6 → 4.** Same grid argument as [`PANEL_PADDING`], but
/// here there is no tie to break: §4.3.2 prices the tree row's horizontal
/// inset at `space.xs` by name, and the row's own composite agrees.
pub const ROW_PADDING_X: f32 = tree_row::ROW_INSET_X;

/// Minimum height of one tree row — [`tree_row::ROW_HEIGHT_COMPACT`],
/// which is `size.hit_target.min` (§4.3.2: "Every row has minimum height
/// `24px`").
///
/// **Movement: unset → 24.** The row previously had no height floor at
/// all; its height was whatever the label plus [`ROW_PADDING_Y`] produced
/// — about `20px` at the old 14pt label and about `19px` at the new
/// 13pt one. Both are below the pointer-target minimum the same section
/// states, so every row in the tree was a sub-target click. The floor is
/// now the framework's own compact rung, and
/// [`tree_row::label_fits_row`] is asserted against it so a rung retune
/// that would clip a label fails a test instead of shipping.
pub const ROW_MIN_HEIGHT: f32 = tree_row::ROW_HEIGHT_COMPACT;

/// The `[PDF]` badge's type rung — the framework badge atom's own rung
/// ([`badge::TYPE_RUNG`], `type.caption`, `12px`).
///
/// Taken from the badge atom rather than chosen here so the mark is typed
/// the way every other badge in the app is typed. It is also the reason
/// the badge costs the row nothing vertically: the caption line box is
/// `16px` and the row's interior is `24 − 2 × 2 = 20px`, so the badge
/// sits inside the compact rung with `4px` to spare and
/// [`ROW_MIN_HEIGHT`] is untouched.
pub const PDF_BADGE_RUNG: typography::TypeRung = badge::TYPE_RUNG;

/// The horizontal room the `[PDF]` glyph run is budgeted at the badge
/// rung — `space.lg + space.xs` (`16 + 4 = 20`).
///
/// This is the text term of the chip's box, declared rather than measured:
/// the view cannot measure real glyphs (see the module header), so the
/// room the mark needs is a stated budget and the chip is built around it.
pub const PDF_BADGE_TEXT_ALLOWANCE: f32 = space::LG + space::XS;

/// The `[PDF]` chip's height — `size.badge.count.height` (`18px`), read
/// off the badge atom's own pill height.
///
/// **Why not the status chip's `24px`.** `badge::CHIP_HEIGHT` is the
/// *entire* compact row height. The row's interior is
/// `24 − 2 × ROW_PADDING_Y(2) = 20px`, so a 24px chip cannot be placed
/// without either overflowing into the neighbouring rows or shrinking
/// away from its own token. The count pill's height is the framework's
/// other pill rung, it is a real token, and it sits inside the 20px
/// interior with `2px` to spare.
pub const PDF_BADGE_CHIP_HEIGHT: f32 = badge::COUNT_HEIGHT;

/// The `[PDF]` chip's corner radius — `shape.radius.pill`, the same
/// radius every badge kind in §3.8 wears.
pub const PDF_BADGE_CHIP_RADIUS: f32 = shape::RADIUS_PILL;

/// The `[PDF]` chip's horizontal padding — `space.sm`, the badge atom's
/// own [`badge::CHIP_H_PADDING`].
pub const PDF_BADGE_CHIP_H_PADDING: f32 = badge::CHIP_H_PADDING;

/// Both side paddings of the chip, taken together — `space.sm` twice.
///
/// Named so [`PDF_BADGE_SLOT`] stays on one line: the token gate parses a
/// declaration's expression textually, and a `pub const … =` wrapped across
/// lines leaves it reading an empty expression, which classifies as
/// token-bound without ever looking at what the value is made of. That is a
/// blind spot, not a pass, so the slot is kept short enough to be seen.
pub const PDF_BADGE_CHIP_PADDING_TOTAL: f32 = badge::CHIP_H_PADDING + badge::CHIP_H_PADDING;

/// The width the `[PDF]` badge owns on a tree row — and, because the chip
/// is drawn to fill it, the width the chip actually renders at.
///
/// **A declared slot, not a text measurement.** The chip is rendered in a
/// container fixed to this width, so the room the row charges for the
/// badge and the room the badge actually occupies are the same number by
/// construction. A reservation *guessed* from glyph widths would be a
/// standing risk that the guess runs short and the label floor is
/// breached by a font change; a slot cannot.
///
/// The slot is **composed from the chip's own box** rather than picked
/// from the spacing scale:
///
/// ```text
/// slot = text allowance + both side paddings
///      = 20            + 8 + 8
///      = 36
/// ```
///
/// That is the same `36` the slot had when it was written as
/// `space.xl + space.md`, so no fit test, floor computation or charge in
/// this module moves — but the number is now the chip's width rather than
/// a figure that merely happens to be large enough for it.
pub const PDF_BADGE_SLOT: f32 = PDF_BADGE_TEXT_ALLOWANCE + PDF_BADGE_CHIP_PADDING_TOTAL;

/// The horizontal gap between a row's label and its `[PDF]` badge —
/// `space.xs`.
pub const PDF_BADGE_GAP: f32 = space::XS;

/// The total horizontal cost one `[PDF]` badge charges the row: the slot
/// plus the gap in front of it.
///
/// This is the term §3B.6's row-furniture model does not have. The
/// section budgets a row as `insets + chevron + indent + label` and
/// never contemplates a trailing badge, so the badge has to be charged
/// against what is left explicitly rather than assumed to be free.
pub const PDF_BADGE_CHARGE: f32 = PDF_BADGE_SLOT + PDF_BADGE_GAP;

/// Vertical padding inside one search-result card — `space.xs`.
/// Paired with [`SEARCH_ROW_HEIGHT`], which is derived from it.
pub const SEARCH_CARD_PADDING: f32 = space::XS;

/// Slot height of one search-result card, in logical pixels, **derived
/// from the three lines the card renders** rather than picked:
/// the title's `ui_compact` line box, two `caption` line boxes for the
/// path and snippet lines, and the card's own vertical padding.
///
/// `18 + 16 + 16 + 2 × 4 = 58`.
///
/// **Movement: 52 → 58.** The retired 52 was a guess that happened to
/// clear the card's content only because iced's default *relative* line
/// height (1.2×) was doing the arithmetic: at the old sizes the content
/// was ~51.2px inside a 52px slot. Once the card's lines are pinned to
/// the framework's rung line heights — which is what this increment does
/// — the content is 50px and the slot has to be 58 to hold it plus its
/// padding. Deriving the height means the card can never again be shorter
/// than the text it is fixed to.
pub const SEARCH_ROW_HEIGHT: f32 = tree_row::LABEL_RUNG.line_height
    + typography::CAPTION.line_height * 2.0
    + SEARCH_CARD_PADDING * 2.0;

/// Glyph size for the search row's trigger and clear icons —
/// [`pro_text_input::SLOT_GLYPH_SIZE`], which is `size.icon.md`
/// (`UIF-TOK-002`): the rung the framework's own search-input slot
/// paints its glyph at.
///
/// **Movement: 14 → 16.** Both glyphs were sized off `ROW_TEXT_SIZE`, a
/// *text* rung that this increment moves to 13. A 13pt icon is not on
/// the icon scale at all, and the two controls are icon slots, not text.
/// Binding them to the slot rung makes them the size the framework would
/// have drawn them and stops the icon size silently tracking the label
/// size.
pub const SEARCH_GLYPH_SIZE: f32 = pro_text_input::SLOT_GLYPH_SIZE;

/// Spacing between the search field and its two trailing controls —
/// `space.xs` (`UIF-TOK-001`), the compact gap. Value unchanged at `4`,
/// previously written as a bare literal.
pub const SEARCH_ROW_SPACING: f32 = space::XS;

/// Vertical rhythm between search-result cards, and between the lines
/// inside one card — `space.xxs` (`UIF-TOK-001`), the densest rung.
/// Value unchanged at `2`, previously written as a bare literal in both
/// places.
pub const RESULT_LINE_SPACING: f32 = space::XXS;

/// Every text size this panel renders, as `(render site, size)` pairs.
///
/// This exists so the `UIF-TOK-004` floor — *nothing renders below
/// 12px* — is asserted over **every** place the panel draws text rather
/// than over three constants that happen to be named in a test. The
/// entries read the constants, so a value cannot drift from what the
/// view uses; the sidecar pins the list's length, so adding another
/// render site forces the floor check to be extended rather than
/// silently skipped.
pub const PANEL_TEXT_SITES: &[(&str, f32)] = &[
    ("panel header", HEADER_SIZE),
    ("tree row label", ROW_TEXT_SIZE),
    ("search field value", ROW_TEXT_SIZE),
    ("search trigger glyph", SEARCH_GLYPH_SIZE),
    ("search clear glyph", SEARCH_GLYPH_SIZE),
    ("empty-state message", ROW_TEXT_SIZE),
    ("search result title", ROW_TEXT_SIZE),
    ("search result match count", SECONDARY_TEXT_SIZE),
    ("search result path", SECONDARY_TEXT_SIZE),
    ("search result snippet", SECONDARY_TEXT_SIZE),
];

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

/// The pixel width one tree row is laid out at, given the pane's
/// remembered width.
///
/// The panel draws its body inside [`PANEL_PADDING`] on each side, so a
/// row is `2 × PANEL_PADDING` narrower than the pane that holds it.
/// `None` means the splitter has never been dragged, and the pane's
/// default width is what the row gets.
///
/// **Why the floor is applied here rather than trusted upstream.** The
/// indent policy this feeds is width-relative: the same depth resolves to
/// a different indent at a different width. Handing it a width the pane
/// will not honour would let a row compute indent for a geometry it never
/// receives, so the width is floored to the pane's own minimum before
/// the policy sees it. The *upper* bound needs the viewport and is not
/// available here — but [`PanelLayout::set_width`] stores the width
/// already clamped to `viewport × LEFT_PANEL_MAX_WIDTH_FRACTION` at
/// drag time, so the remembered value cannot exceed the cap.
///
/// [`PanelLayout::set_width`]: crate::ui::panel_layout::PanelLayout::set_width
///
/// Purity: pure.
#[must_use]
pub fn row_width_for(remembered: Option<f32>) -> f32 {
    let pane = remembered.unwrap_or(LEFT_PANEL_DEFAULT_WIDTH);
    (pane.max(LEFT_PANEL_MIN_WIDTH) - PANEL_PADDING - PANEL_PADDING).max(0.0)
}

/// The indent one row of `depth` is drawn with, in logical points, at a
/// row `row_width` wide.
///
/// **INT-12 replaced this function's whole policy.** It used to be
/// `depth.min(MAX_ROW_INDENT_DEPTH) × INDENT_STEP` — a flat clamp that
/// capped *how deep* a row could indent while leaving the label's width
/// entirely unconsidered. At the cap the indent was `50 × 16 = 800px`,
/// which in a `280px` pane pushes the label `520px` past the right edge:
/// the label is not truncated, it is **gone**. §3B.6's guarantee that the
/// label "never narrows below `96px` at any tree depth (`20` or `200`)"
/// was therefore false at every depth past about eight in any normally
/// sized pane.
///
/// It now delegates to [`tree_row::indent_geometry`], the framework's
/// three-tier width-relative solver: nominal `16px` per level while the
/// budget lasts, proportional compression across all active levels once
/// `depth × 16px` exceeds it, and a `4px` (`space.xs`) floor step beyond
/// which deeper levels collapse onto the innermost drawn guide. The label
/// keeps its `96px` because the *indent* is what absorbs the shortfall.
///
/// Extracted so the drawn indent and the guide offsets read the *same*
/// resolution instead of each repeating the call, which is how the two
/// would otherwise drift.
///
/// Purity: pure.
#[must_use]
pub fn row_indent_pixels(depth: usize, row_width: f32) -> f32 {
    tree_row::indent_geometry(depth, row_width).width
}

/// Whether a `[PDF]` badge fits on a row of `depth` at `row_width`
/// without breaching the `TREE_LABEL_MIN_W = 96px` label floor.
///
/// §3B.6 budgets a row as `insets + chevron + indent + label` and has no
/// trailing-badge term at all, so the badge's cost ([`PDF_BADGE_CHARGE`])
/// has to be charged against whatever the indent left over and the fit
/// tested explicitly. Charging it silently is how the badge would eat the
/// floor it exists to protect.
///
/// **The check is conservative in the safe direction.** The app folds its
/// disclosure glyph into the label string rather than reserving
/// [`tree_row::CHEVRON_COLUMN`] separately, so `indent_allowance`
/// subtracts `24px` the app does not actually spend. The real label
/// region is therefore *wider* than this test assumes, and a row that
/// passes here clears the floor with margin rather than by luck.
///
/// **What this gate decides when it returns `false`.** At the pane's hard
/// minimum width the two requirements collide: §4.3.2 wants the badge and
/// §3B.6 forbids a label under `96px`, and there is not enough row for
/// both. The floor wins, because §3B.6 states it absolutely ("**never**
/// narrows below") where §4.3.2 states the badge as what special rows
/// "include". The pane's auto-fit (`UIF-PANE-002`) normally removes the
/// conflict by widening the pane to fit its longest row.
///
/// Purity: pure.
#[must_use]
pub fn pdf_badge_fits(depth: usize, row_width: f32) -> bool {
    let allowance = tree_row::indent_allowance(row_width);
    let drawn = tree_row::indent_geometry(depth, row_width).width;
    allowance - drawn >= PDF_BADGE_CHARGE
}

/// The `[PDF]` chip's resolved paint and box (`§3B.6:883`).
///
/// Every field is a value the renderer consumes as-is, mirroring the shape
/// of [`badge::BadgeStyle`]: no token lookup and no further resolution at
/// the paint site.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PdfBadgeChip {
    /// The chip's fill — `status.danger.fill`, via the badge tone resolver.
    pub fill: Color,
    /// The chip's foreground — `color.text.strong`, in **every** posture.
    pub foreground: Color,
    /// The chip's corner radius — [`PDF_BADGE_CHIP_RADIUS`].
    pub radius: f32,
    /// The chip's height — [`PDF_BADGE_CHIP_HEIGHT`].
    pub height: f32,
    /// The chip's horizontal padding — [`PDF_BADGE_CHIP_H_PADDING`].
    pub h_padding: f32,
}

/// The `[PDF]` chip's paint and box, resolved from framework resolvers.
///
/// # Why the chip is assembled here rather than taken from the badge atom
///
/// The rule asks for a text-bearing pill on `status.danger.fill`, and
/// `badge::resolve_style` cannot produce that combination:
///
/// * The solid `status.danger.fill` behind `text.strong` is reachable only
///   through `BadgeKind::Count(u32)`, whose text is a number —
///   `count.text()` is `Some("7")`, never a word.
/// * The only text-bearing kind, `BadgeKind::Status { icon, label }`, is
///   hardcoded to `fill: surface_raised` with the tone in the
///   **foreground and border**. Verified against the resolver's own
///   output: `resolve_style(&Status + Danger)` returns `status_danger_text`
///   for both `text` and `border` — exactly the danger-coloured text label
///   the rule forbids. It also mandates a leading glyph from a fixed
///   four-set (`✓ ⚠ ✕ ●`), none of which denotes "PDF-backed".
/// * `§3.8` defines exactly three badge kinds, so a text-bearing solid
///   kind cannot be added without contradicting the spec's own inventory.
///
/// The two halves the rule needs live in mutually exclusive variants, so the
/// chip is composed from the same resolvers the atom uses internally:
/// [`badge::resolve_tone`] supplies the fill, `Palette::text_strong` the
/// foreground and [`shape::RADIUS_PILL`] the corners. No colour is
/// hand-picked and no numeric literal is written (§4a).
///
/// # The recorded shortfall — read this before believing the chip works
///
/// The chip is built exactly as mandated. What it measures is another
/// matter, and the figures are pinned in the sidecar so a palette retune
/// surfaces them rather than hiding them:
///
/// * **The chip's own `FG`/`BG` pair passes.** `text.strong` on
///   `status.danger.fill` measures **`5.126966:1`** from raw sRGB — the
///   `5.13:1` the rule prints and the `5.1270:1` the atom header reports.
///   This is the pairing [`contrast::assert_fill_foreground_lock`] guards,
///   and it holds.
/// * **The chip's boundary against the selected band fails.** Against
///   `color.accent.fill` the fill measures **`1.067676:1`**, short of the
///   [`contrast::MIN_COINCIDENT_FILL_CONTRAST`] `3.00:1` that the atom's
///   own `ChromeClass::CoincidentWithFill` classification demands — by
///   `1.93` points. The rule's stated mechanism ("so the chip fill itself
///   carries the distinction and survives the lock") does not hold on the
///   band the rule is about.
/// * **At rest and on hover it is fine:** `3.880532:1` on `surface.base`,
///   `3.166606:1` on `surface.hover`. The chip is visible until the row
///   is selected, then collapses into the band.
///
/// **No framework assertion fires on that shortfall, and none was
/// bypassed.** [`badge::badge_chrome_class`] does classify the chip
/// `CoincidentWithFill`, and `ChromeClass::minimum_contrast()` does return
/// `Some(3.00)` — but the audit that consumes the class,
/// `gallery_audit::judge`, returns `true` unconditionally for every class
/// except `Judged` ("reported without a verdict because no threshold
/// describes them"). The coincident-fill floor is declarative only:
/// nothing in the enforcement path compares a badge fill against a
/// selection band. The shortfall is therefore pinned in the sidecar as a
/// measured value labelled a **recorded shortfall, not a pass**.
///
/// Purity: pure.
#[must_use]
pub fn pdf_badge_chip() -> PdfBadgeChip {
    let pro = Palette::pro();
    let fill = badge::resolve_tone(BadgeTone::Danger, &pro).solid_fill;
    let foreground = pro.text_strong;
    // The framework's own `UIF-TOK-006` rule 4 shield, run on the pair as
    // actually built rather than trusted. A debug build aborts here if the
    // pairing ever stops being compliant.
    contrast::assert_fill_foreground_lock(fill, foreground, &pro);
    PdfBadgeChip {
        fill,
        foreground,
        radius: PDF_BADGE_CHIP_RADIUS,
        height: PDF_BADGE_CHIP_HEIGHT,
        h_padding: PDF_BADGE_CHIP_H_PADDING,
    }
}

/// Width a single row needs: `indent + measured label + slack`.
///
/// The arithmetic is delegated to [`tree_row::content_width`] — the
/// framework's own "width this row would need if nothing were clamped"
/// rule — so the app's UI-016 auto-fit and the row composite it is
/// laying out for cannot disagree about what the row's furniture costs.
/// `ROW_WIDTH_SLACK` is exactly that function's furniture term.
///
/// `measure` is the injected label measurer — see the module header for why
/// measurement cannot come from inside the view.
///
/// Purity: pure for a pure `measure`.
#[must_use]
pub fn row_width(row: &FlatRow, measure: impl Fn(&str) -> f32) -> f32 {
    tree_row::content_width(
        row.depth.min(MAX_ROW_INDENT_DEPTH),
        measure(&row_label(row)),
    )
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

/// Context-menu rows for a **directory** row, in §4.9.1's group order
/// (UI-006, VFS-122).
///
/// The spec's three runs are **Creation** (New document, Create Directory),
/// **Folder Automation & Management** (folder skills, then Rename, Move,
/// Copy path, Show in File Explorer) and **Destructive** (Delete). A
/// separator is emitted between the runs by
/// [`menu_from_rows`](crate::ui_iced::context_menu::menu_from_rows); the
/// rows here only declare which run they belong to.
///
/// Purity: pure.
#[must_use]
pub fn directory_menu_items(path: &Path, folder_skills: &[SkillFile]) -> Vec<ContextRow> {
    let mut items = vec![
        ContextRow::new(
            KEY_NEW_DOCUMENT,
            strings::NEW_DOCUMENT_ACTION,
            ContextGroup::Primary,
            Message::Command(UserCommand::CreateDocument {
                parent: path.to_path_buf(),
            }),
        ),
        ContextRow::new(
            KEY_CREATE_DIRECTORY,
            strings::CREATE_DIRECTORY_ACTION,
            ContextGroup::Primary,
            Message::Command(UserCommand::CreateDirectory {
                parent: path.to_path_buf(),
            }),
        ),
    ];

    items.extend(
        folder_skills
            .iter()
            .map(|skill| ContextRow::skill(skill, Some(path.to_path_buf()), path)),
    );

    items.extend([
        ContextRow::new(
            KEY_RENAME,
            strings::RENAME_ACTION,
            ContextGroup::Automation,
            Message::Command(UserCommand::Rename(path.to_path_buf())),
        ),
        ContextRow::new(
            KEY_MOVE,
            strings::MOVE_ACTION,
            ContextGroup::Automation,
            Message::Command(UserCommand::Move(path.to_path_buf())),
        ),
        ContextRow::new(
            KEY_COPY_PATH,
            strings::COPY_PATH_ACTION,
            ContextGroup::Automation,
            Message::Command(UserCommand::CopyPath(path.to_path_buf())),
        ),
        ContextRow::new(
            KEY_SHOW_IN_EXPLORER,
            strings::SHOW_IN_EXPLORER_ACTION,
            ContextGroup::Automation,
            Message::Command(UserCommand::ShowInExplorer(path.to_path_buf())),
        ),
        ContextRow::new(
            KEY_DELETE,
            strings::DELETE_ACTION,
            ContextGroup::Destructive,
            Message::Command(UserCommand::Delete(path.to_path_buf())),
        ),
    ]);

    items
}

/// Context-menu rows for a **single-selected file**, in §4.9.1's group order
/// (UI-007, UI-008, UI-009..UI-014, UI-020).
///
/// The spec's four runs are **Open & Output** (Edit, Print, Save as PDF),
/// **AI & Automation** (Run as prompt, then the VFS-121 note skills),
/// **File Management** (Rename, Move, Copy path, Show in File Explorer) and
/// **Destructive** (Delete).
///
/// `typst_available` gates "Save as PDF…": the row is absent, not
/// disabled, when the `typst` CLI is missing.
///
/// §4.9.1 also lists `Open in Editor` in the file menu's first run. The app
/// has never had that row on a tree target — `Edit` **is** the
/// `UserCommand::OpenInEditor` row — and adding a second row that fires the
/// same command is new user-facing behaviour, so it is deliberately **not**
/// added here.
///
/// Purity: pure.
#[must_use]
pub fn file_menu_items(
    path: &Path,
    note_skills: &[SkillFile],
    typst_available: bool,
) -> Vec<ContextRow> {
    let mut items = vec![
        ContextRow::new(
            KEY_EDIT,
            strings::EDIT_BUTTON,
            ContextGroup::Primary,
            Message::Command(UserCommand::OpenInEditor(path.to_path_buf())),
        ),
        ContextRow::new(
            KEY_PRINT,
            strings::PRINT_ACTION,
            ContextGroup::Primary,
            Message::Tree(TreeMessage::PrintRequested {
                path: path.to_path_buf(),
            }),
        ),
    ];

    if typst_available {
        items.push(ContextRow::new(
            KEY_SAVE_AS_PDF,
            strings::SAVE_AS_PDF_ACTION,
            ContextGroup::Primary,
            Message::Command(UserCommand::SaveAsPdf(path.to_path_buf())),
        ));
    }

    items.push(ContextRow::new(
        KEY_RUN_AS_PROMPT,
        strings::RUN_AS_PROMPT_ACTION,
        ContextGroup::Automation,
        Message::Tree(TreeMessage::RunAsPromptRequested {
            path: path.to_path_buf(),
        }),
    ));
    items.extend(
        note_skills
            .iter()
            .map(|skill| ContextRow::skill(skill, None, path)),
    );

    items.extend([
        ContextRow::new(
            KEY_RENAME,
            strings::RENAME_ACTION,
            ContextGroup::Management,
            Message::Command(UserCommand::Rename(path.to_path_buf())),
        ),
        ContextRow::new(
            KEY_MOVE,
            strings::MOVE_ACTION,
            ContextGroup::Management,
            Message::Command(UserCommand::Move(path.to_path_buf())),
        ),
        ContextRow::new(
            KEY_COPY_PATH,
            strings::COPY_PATH_ACTION,
            ContextGroup::Management,
            Message::Command(UserCommand::CopyPath(path.to_path_buf())),
        ),
        ContextRow::new(
            KEY_SHOW_IN_EXPLORER,
            strings::SHOW_IN_EXPLORER_ACTION,
            ContextGroup::Management,
            Message::Command(UserCommand::ShowInExplorer(path.to_path_buf())),
        ),
        ContextRow::new(
            KEY_DELETE,
            strings::DELETE_ACTION,
            ContextGroup::Destructive,
            Message::Command(UserCommand::Delete(path.to_path_buf())),
        ),
    ]);

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
pub fn multi_select_menu_items(paths: &[PathBuf]) -> Vec<ContextRow> {
    vec![
        ContextRow::new(
            KEY_MERGE,
            strings::MERGE_ACTION,
            ContextGroup::Primary,
            Message::Command(UserCommand::MergePrompt(paths.to_vec())),
        ),
        ContextRow::new(
            KEY_DELETE,
            strings::DELETE_ACTION,
            ContextGroup::Destructive,
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

/// The §4.9.1 context target `row` represents under `selected_files`.
///
/// The **single** place the multi-select rule lives: [`context_menu_items`]
/// picks the menu through this, so the target the floating surface records and
/// the menu it actually renders cannot drift apart.
///
/// Purity: pure.
#[must_use]
pub fn context_target_for(row: &FlatRow, selected_files: &[PathBuf]) -> ContextTarget {
    if row.is_dir {
        return ContextTarget::TreeDirectory {
            path: row.path.clone(),
        };
    }
    let is_multi = selected_files.len() > 1 && selected_files.iter().any(|path| path == &row.path);
    if is_multi {
        ContextTarget::TreeMultiSelect {
            paths: selected_files.to_vec(),
        }
    } else {
        ContextTarget::TreeFile {
            path: row.path.clone(),
        }
    }
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
) -> Vec<ContextRow> {
    match context_target_for(row, selected_files) {
        ContextTarget::TreeDirectory { path } => directory_menu_items(&path, context.folder_skills),
        ContextTarget::TreeMultiSelect { paths } => multi_select_menu_items(&paths),
        ContextTarget::TreeFile { path } => {
            file_menu_items(&path, context.note_skills, context.typst_available)
        }
        // A tab target is served by
        // [`tab_context_menu_items`](super::center::tab_context_menu_items),
        // never here: [`context_target_for`] can only produce `Tab` if a
        // caller routed a tab through the tree builder. The single-file menu
        // is returned rather than panicking because a view-side builder must
        // not abort the frame over a routing mistake.
        ContextTarget::Tab { path, .. } => {
            file_menu_items(&path, context.note_skills, context.typst_available)
        }
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
        .workspace()
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
        for dir in app.workspace().all_dirs.iter() {
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
    /// The tree row the pointer is currently over, by **path** (INT-10).
    ///
    /// Keyed by path rather than by row index because the row list is
    /// re-flattened on every expand, collapse, filter and watcher-driven
    /// refresh. An index is not a stable address: the moment the list
    /// moves, a highlight keyed by index lands on a different file.
    /// Keyed by path, the highlight can only ever sit on the file the
    /// pointer actually entered.
    ///
    /// Like [`multi_select`](Self::multi_select), this is reported
    /// `false`-equivalent (`None`) by [`from_app`](Self::from_app),
    /// which has no pointer input, and overwritten by
    /// `ViewModels::from_app` from
    /// [`OverlayState::hovered_tree_row`](crate::ui_iced::app::OverlayState::hovered_tree_row).
    pub hovered_row: Option<PathBuf>,
    /// The pixel width one tree row is laid out at (INT-12).
    ///
    /// Both INT-12 pieces need it and neither can invent it. The
    /// width-relative indent policy (`tree_row::indent_geometry`) and
    /// the `[PDF]` badge's fit test are both functions of the *row's*
    /// width; a row that guesses its own width either compresses when it
    /// need not, or breaches the `TREE_LABEL_MIN_W = 96px` floor when it
    /// must not.
    ///
    /// Resolved by [`row_width_for`] from the pane's remembered width
    /// less the panel's own padding, so the number the policy reads is
    /// the number the row is drawn at.
    pub row_width: f32,
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
            hovered_row: None,
            row_width: row_width_for(app.layout().left_panel_width),
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

    /// The §4.9.1 context target the row whose menu is open represents.
    #[must_use]
    pub fn menu_target_for(&self, row: &FlatRow) -> ContextTarget {
        context_target_for(row, &self.selected_files)
    }

    /// The context-menu rows for the row whose menu is open.
    #[must_use]
    pub fn menu_items_for(&self, row: &FlatRow) -> Vec<ContextRow> {
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
        .line_height(typography::TITLE_SM.iced_line_height())
        .font(typography::TITLE_SM.font(FAMILY_SANS, typography::TITLE_SM.weight))
        .color(palette.text_strong);

    let search_field = text_input(strings::SEARCH_PLACEHOLDER, &view_model.query)
        .id(SEARCH_INPUT_ID)
        .size(ROW_TEXT_SIZE)
        .line_height(tree_row::LABEL_RUNG.iced_line_height())
        .padding(SEARCH_CARD_PADDING)
        .style(|_theme, status| search_field_style(status))
        .on_input(|text| Message::Search(SearchMessage::QueryChanged { text }))
        .on_submit(Message::Search(SearchMessage::Submitted {
            query: view_model.query.clone(),
        }));

    let search_trigger = button(
        text(strings::ICON_MAGNIFYING_GLASS)
            .font(phosphor_font())
            .size(SEARCH_GLYPH_SIZE),
    )
    .on_press(Message::Search(SearchMessage::Submitted {
        query: view_model.query.clone(),
    }));

    let clear_trigger = button(
        text(strings::ICON_X)
            .font(phosphor_font())
            .size(SEARCH_GLYPH_SIZE),
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
    .spacing(SEARCH_ROW_SPACING)
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
                .line_height(tree_row::LABEL_RUNG.iced_line_height())
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
///
/// The row's geometry is the framework's: the indent comes from
/// [`row_indent_pixels`] (`tree_row::INDENT_STEP`), the insets from
/// [`ROW_PADDING_X`] / [`ROW_PADDING_Y`] (`tree_row::ROW_INSET_X` /
/// `ROW_INSET_Y`), the line box from [`tree_row::LABEL_RUNG`] and the
/// floor from [`ROW_MIN_HEIGHT`] (`tree_row::ROW_HEIGHT_COMPACT`). The
/// band comes from [`tree_row::resolve_style`].
fn tree_row<'a>(row: &'a FlatRow, view_model: &'a LeftPanelViewModel) -> Element<'a, Message> {
    let selected = is_row_selected(
        row,
        view_model.selected_file.as_deref(),
        &view_model.selected_files,
    );
    let pdf_backed = view_model.pdf_backed.contains(&row.path);
    let hovered = row_is_hovered(row, view_model.hovered_row.as_deref());
    let paint = row_paint(selected, hovered);
    let row_box = row_box(row.depth, view_model.row_width, selected, hovered);

    let label = text(row_label(row))
        .size(ROW_TEXT_SIZE)
        .line_height(tree_row::LABEL_RUNG.iced_line_height())
        .color(row_label_colour(selected, hovered, pdf_backed));

    // The `[PDF]` chip rides **after** the label, never before it. The
    // label's left edge is the line every row in the tree aligns to; a
    // leading chip would push it by the chip's width on PDF-backed rows
    // alone and the column would jog. Trailing, the label starts exactly
    // where it always did and the chip occupies its own declared slot.
    //
    // The chip fills [`PDF_BADGE_SLOT`] edge to edge, so the width the row
    // charges for and the width that actually paints are the same number
    // by construction. Its fill is `status.danger.fill` and its foreground
    // is `text.strong` in **every** posture — the foreground is
    // deliberately *not* routed through the selection lock, because
    // `§3B.6:883` forbids the distinction riding on badge text colour.
    // See [`pdf_badge_chip`] for the measured shortfall against the
    // selected band that this rendering is nonetheless required to carry.
    let content: Element<'a, Message> =
        if pdf_backed && pdf_badge_fits(row.depth, view_model.row_width) {
            let chip = pdf_badge_chip();
            let mark = container(
                text(strings::TREE_PDF_BADGE)
                    .size(PDF_BADGE_RUNG.size)
                    .line_height(PDF_BADGE_RUNG.iced_line_height())
                    .align_x(Alignment::Center)
                    .color(chip.foreground),
            )
            .width(Length::Fixed(PDF_BADGE_SLOT))
            .height(Length::Fixed(chip.height))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            // iced 0.14 has no `border_radius` builder on a container; the
            // radius lives on the style's `Border`, and `draw_background`
            // hands that whole `Border` to the fill quad
            // (`iced_widget-0.14.2/src/container.rs:446`), so the fill is
            // rounded even at a zero border width.
            .style(move |_| container::Style {
                background: Some(Background::Color(chip.fill)),
                border: Border {
                    width: 0.0,
                    radius: chip.radius.into(),
                    ..Default::default()
                },
                ..container::Style::default()
            })
            .into();

            Row::with_children(vec![label.into(), mark])
                .spacing(PDF_BADGE_GAP)
                .align_y(Alignment::Center)
                .into()
        } else {
            label.into()
        };

    let row_surface = container(content)
        .width(Length::Fill)
        // iced 0.14's `Container` has no `min_height`, so the compact
        // rung is applied as a fixed height. That is not a downgrade
        // here: the row's content is one single-line label on an 18pt
        // line box plus 2pt insets = 22pt, always shorter than the 24pt
        // rung, so "fixed at the rung" and "at least the rung" are the
        // same set of layouts. `align_y` keeps the label centred in the
        // 2pt of slack instead of parked against the top edge.
        .height(Length::Fixed(row_box.height))
        .align_y(Alignment::Center)
        .padding(row_box.padding)
        // The band is painted whenever the resolved style actually has
        // one. A resting row's background is transparent and paints
        // nothing, so the test is about not emitting a pointless quad,
        // not about appearance.
        .style(move |_theme| container::Style {
            background: (paint.background.a > 0.0).then_some(Background::Color(paint.background)),
            ..container::Style::default()
        });

    // The context-menu trigger *is* a `mouse_area`, so the same area
    // carries the hover crossings. `on_enter` / `on_exit` fire only on
    // the transition (see [`row_paint`]'s note), which is what keeps a
    // per-frame tree rebuild from flooding the reducer.
    let trigger = crate::ui_iced::overlay::context_menu_trigger(
        row_surface,
        Message::Tree(TreeMessage::ContextMenuOpened {
            path: row.path.clone(),
        }),
    )
    .on_press(Message::Command(row_click_command(
        row,
        view_model.multi_select,
    )))
    .on_enter(Message::Tree(TreeMessage::RowEntered {
        path: row.path.clone(),
    }))
    .on_exit(Message::Tree(TreeMessage::RowLeft {
        path: row.path.clone(),
    }));

    let base: Element<'a, Message> = match row_double_click_command(row) {
        Some(command) => trigger.on_double_click(Message::Command(command)).into(),
        None => trigger.into(),
    };

    // The selection bar and the depth guides are painted **over** the row in
    // a `Stack`; the row's own box is left exactly as [`row_box`] resolves it,
    // so neither the bar nor a guide can shift the label or a neighbour. The
    // stripes are siblings of the row's `mouse_area`, not children of it, and
    // iced dispatches events without occlusion culling — a stripe carries no
    // handler, so hover, click, double-click and the right-click menu all still
    // land on the area underneath, and INT-10's path-keyed hover is untouched.
    //
    // A resting, unindented row has nothing to paint and returns the base
    // directly rather than carrying an empty `Stack`.
    let overlay = row_overlay(row.depth, view_model.row_width, selected, hovered);
    if overlay.is_empty() {
        return base;
    }

    let mut layers = vec![base];
    if let Some(bar) = overlay.selection_bar {
        layers.push(stripe_element(bar, row_box.height));
    }
    for guide in &overlay.guides {
        layers.push(stripe_element(*guide, row_box.height));
    }

    Stack::with_children(layers)
        .width(Length::Fill)
        .height(Length::Fixed(row_box.height))
        .into()
}

/// Whether the pointer is over `row`, given the reducer's hovered path.
///
/// The whole recycling-safety argument in one comparison. iced 0.14
/// reconciles widget `Tree`s by **position**, not by explicit `Id`
/// (`iced_core-0.14.0/src/widget/tree.rs:86-107`), so a row's own
/// widget state cannot be trusted to follow its file when the list is
/// re-flattened. This function refuses to consider position at all: a
/// row is hovered exactly when the recorded path names *its own* file,
/// which makes it impossible for a highlight to travel onto a different
/// file after a re-sort, a filter, an expand/collapse or a
/// watcher-driven refresh. A stale path matches nothing and simply
/// paints no row.
///
/// Purity: pure.
#[must_use]
pub fn row_is_hovered(row: &FlatRow, hovered: Option<&Path>) -> bool {
    hovered == Some(row.path.as_path())
}

/// The tree row's paint, resolved through the framework's tree-row
/// composite (`UIF-COMP-006`) against the left pane's own surface.
///
/// `hovered` reaches [`BaseState::Hovered`], which paints
/// `color.surface.hover` — the rung §2.5 prices for a "hovered
/// menu/table/tree item". Selection outranks the pointer *inside*
/// `resolve_style`, so a selected row keeps `accent.fill` under the
/// cursor and a hover can never wash the selection out.
///
/// # The claim this function used to make
///
/// This used to hardcode [`BaseState::Rest`] and justify it by saying
/// iced 0.14's `mouse_area` "does not report hover into a `container`
/// style", so the tier could not be reached without replacing the row's
/// mouse handling. **That reasoning was wrong.** What it describes is
/// *automatic* style propagation: `mouse_area` will not rewrite a
/// container's style on your behalf. It says nothing about
/// `MouseArea::on_enter` / `MouseArea::on_exit`
/// (`iced_widget-0.14.2/src/mouse_area.rs:106` / `:120`), which the
/// row's existing context-menu `mouse_area` already carries and which
/// publish a message on the crossing — so the reducer can set the very
/// state this call reads. No widget was replaced; the same `mouse_area`
/// gained two handlers.
///
/// `mouse_area` is also edge-triggered for free, which matters because
/// this tree is rebuilt every frame: it publishes `on_enter` only on
/// `is_hovered && !was_hovered` and `on_exit` only on the inverse, and
/// skips the whole check when neither the cursor nor its own bounds
/// moved (`mouse_area.rs:349-366`). A level-triggered flood of hover
/// reports into the reducer is therefore structurally impossible.
///
/// Purity: pure function of `(selected, hovered)`.
#[must_use]
pub fn row_paint(selected: bool, hovered: bool) -> tree_row::TreeRowStyle {
    let pro = Palette::pro();
    let base = if hovered {
        BaseState::Hovered
    } else {
        BaseState::Rest
    };
    tree_row::resolve_style(selected, false, false, base, pro.surface_base, &pro)
}

/// The tree row's box, resolved in one place so it can be *diffed*.
///
/// Hover and selection are inputs here rather than absent on purpose:
/// that is what lets the sidecar ask "does hovering move the row?" and
/// get a diffable answer instead of a promise. §2 forbids a hover from
/// changing an outer bounding box or shifting a neighbour, and the way
/// to hold that line is to route every geometry input the row uses
/// through this function and assert the output is invariant across all
/// four postures. If anyone later adds a hover border or a pressed inset
/// that grows the box, a test fails rather than the layout quietly
/// shifting by two pixels on every file in the list.
///
/// Purity: pure function of `(depth, selected, hovered)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowBox {
    /// The row's height — the compact rung, whatever the posture.
    pub height: f32,
    /// The row's padding, with the depth indent folded into the left edge.
    pub padding: Padding,
}

/// Resolve a row's box for a given depth and posture.
///
/// See [`RowBox`] for why the posture parameters exist at all.
#[must_use]
pub fn row_box(depth: usize, row_width: f32, selected: bool, hovered: bool) -> RowBox {
    // Neither posture contributes to the box, and that is the whole
    // point of taking them: the invariance is testable rather than
    // merely asserted.
    let _ = (selected, hovered);
    RowBox {
        height: ROW_MIN_HEIGHT,
        padding: Padding {
            top: ROW_PADDING_Y,
            right: ROW_PADDING_X,
            bottom: ROW_PADDING_Y,
            left: ROW_PADDING_X + row_indent_pixels(depth, row_width),
        },
    }
}

/// The x offset of each drawn depth-guide line, measured from the row's own
/// left edge, at a row `row_width` wide.
///
/// **INT-12 honoured the coupling note INT-11 left here.** The note said
/// the guides read the app's *clamped* depth, and that if compression
/// were ever introduced the offsets would have to switch to the
/// compressed geometry or the guides would drift from the labels. They
/// have switched: the offsets now come from
/// [`tree_row::indent_geometry`]'s own `guides` vector rather than from
/// `guide_positions(depth, INDENT_STEP)`, so a guide is drawn exactly
/// where that tier of the indent actually landed. Under the old flat clamp
/// the two agreed only because nothing compressed; the moment the indent
/// became width-relative, `guide_positions(depth, 16.0)` would have drawn
/// a line for every level at the *nominal* step while the labels sat at
/// the compressed one — guides marching off to the right of the text
/// they belong to.
///
/// Delegated rather than recomputed for the same reason the indent is:
/// while both name the same geometry the app's tree and the framework's
/// row cannot drift apart.
///
/// Each line is centred in its step by the framework, which puts it under
/// its ancestor's disclosure column rather than on the boundary between
/// two levels.
///
/// Purity: pure.
#[must_use]
pub fn row_guide_offsets(depth: usize, row_width: f32) -> Vec<f32> {
    tree_row::indent_geometry(depth, row_width).guides
}

/// One painted stripe over a tree row: a full-height, fixed-width bar at a
/// given offset from the row's left edge.
///
/// Held as data so the bar's width can be asserted to come from
/// [`tree_row::SELECTION_BAR_WIDTH`] and the guides' positions to land on
/// [`tree_row::INDENT_STEP`] multiples, without a renderer in sight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowStripe {
    /// Offset from the row's left edge.
    pub x: f32,
    /// Stripe width — `SELECTION_BAR_WIDTH` for the bar, [`tree_row::GUIDE_WIDTH`]
    /// for a guide.
    pub width: f32,
    /// Stripe fill, already resolved through the §3B.6 role lock.
    pub color: Color,
}

/// Everything painted over a tree row's band.
///
/// Separate from [`RowBox`] because the box is what the row *occupies* and the
/// overlay is what is painted on top of it: the two must be independently
/// invariant, and folding them together is how a hover or a selection would
/// get to move a row again.
#[derive(Clone, Debug, PartialEq)]
pub struct RowOverlay {
    /// The `2px` left selection bar — `None` on an unselected row, so an
    /// unselected row cannot show a leftover bar.
    pub selection_bar: Option<RowStripe>,
    /// One guide per drawn indent level, empty at depth 0.
    pub guides: Vec<RowStripe>,
}

impl RowOverlay {
    /// Whether there is anything to paint, which lets the row skip the `Stack`
    /// entirely rather than carry an empty one.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selection_bar.is_none() && self.guides.is_empty()
    }

    /// The widget cost of drawing this overlay over one row, in widgets.
    ///
    /// One `Stack`, one widget for the selection bar (it needs no wrapper at
    /// `x = 0`), and two per guide — the bar plus the transparent full-width
    /// wrapper that offsets it. Exposed so the sidecar can assert the cost
    /// rather than let it be discovered in a profile.
    #[must_use]
    pub fn widget_cost(&self) -> usize {
        if self.is_empty() {
            return 0;
        }
        1 + usize::from(self.selection_bar.is_some()) + self.guides.len() * 2
    }
}

/// Resolve a row's overlay furniture for a given depth and posture.
///
/// Every colour here is the framework's, read off [`row_paint`]:
/// [`tree_row::TreeRowStyle::selection_bar`] and
/// [`tree_row::TreeRowStyle::guide`] are both already run through
/// §3B.6's **role-based** lock, so on a selected row they come back as
/// `text.strong` whatever the resting tokens measure, and off it they are
/// `accent.border` and `border.strong`. Hover moves the band to
/// `surface.hover` and the guide's contrast moves with it; nothing here
/// re-decides that.
///
/// The bar's width is [`tree_row::SELECTION_BAR_WIDTH`] and the guides' is
/// [`tree_row::GUIDE_WIDTH`] — no new literals, and the same constants the
/// composite's own `row_slots` rectangles are built from.
///
/// Purity: pure function of `(depth, row_width, selected, hovered)`.
#[must_use]
pub fn row_overlay(depth: usize, row_width: f32, selected: bool, hovered: bool) -> RowOverlay {
    let paint = row_paint(selected, hovered);
    RowOverlay {
        selection_bar: selected.then_some(RowStripe {
            x: 0.0,
            width: tree_row::SELECTION_BAR_WIDTH,
            color: paint.selection_bar,
        }),
        guides: row_guide_offsets(depth, row_width)
            .into_iter()
            .map(|x| RowStripe {
                x,
                width: tree_row::GUIDE_WIDTH,
                color: paint.guide,
            })
            .collect(),
    }
}

/// Draw one [`RowStripe`] as a full-height, fixed-width bar at its offset.
///
/// The transparent full-width wrapper is not decoration: iced 0.14's `Stack`
/// lays every child at its own top-left origin and exposes no per-child
/// alignment (`iced_widget-0.14.2/src/stack.rs:193-205`), and `Container` has
/// no `margin` to offset itself with, so a stripe at `x > 0` has to be pushed
/// by padding inside something that spans the row. A stripe at `x = 0` — the
/// selection bar — skips the wrapper, which is why the bar costs one widget and
/// a guide costs two.
fn stripe_element(stripe: RowStripe, height: f32) -> Element<'static, Message> {
    let bar = container(Space::new())
        .width(Length::Fixed(stripe.width))
        .height(Length::Fixed(height))
        .style(move |_theme| container::Style {
            background: Some(Background::Color(stripe.color)),
            ..container::Style::default()
        });

    if stripe.x == 0.0 {
        return bar.into();
    }

    container(bar)
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .padding(Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: stripe.x,
        })
        .align_x(Alignment::Start)
        .into()
}

/// The tree row's label colour, with the framework's **role-based**
/// selection lock applied (`UIF-TOK-006` rule 4 / §3B.6).
///
/// Off the selection band the app's own rule stands: the `error` token on
/// PDF-backed files (REQ-450), the body token otherwise. On the band
/// the label is locked to `text.strong`, because
/// `status_danger_text` over `accent_fill` measures **1.59:1** — a
/// selected PDF-backed file used to render red-on-indigo and was
/// effectively unreadable, which is exactly the failure §3B.6's lock
/// exists to prevent.
///
/// # Hover and the contrast lock
///
/// `hovered` moves the band from the pane's resting surface to
/// `color.surface.hover`, so the lock is re-run against the *actual*
/// band rather than against a remembered answer. It resolves to a
/// **no-op** on hover, and that is a computed result, not an absence of
/// handling: `surface_hover` is not a locked fill under
/// [`fill_lock_role`](iced_pro::token::contrast::fill_lock_role), so
/// `locked_over` returns the candidate untouched and the resting label
/// rule — including the `error` tint on PDF-backed rows — survives the
/// pointer. Both pairings clear the text floor by a wide margin, and the
/// sidecar pins the measured ratios rather than transcribing them.
///
/// §4.3.2 asks for a `[PDF]` badge so the *meaning* survives selection,
/// and INT-12 added one — see [`pdf_badge_chip`]. Note that the badge is a
/// **separate mark**; this function still tints the whole label with
/// `error` at rest and on hover. Read literally, `§3B.6:883`'s closing
/// clause ("never in a danger-colored text label") reaches that tint as
/// well as the badge, which would mean the label should not be danger-tinted
/// at all. That is a wider change than the badge rework and is deliberately
/// **not** taken here; it is recorded as an open deviation.
///
/// Purity: pure function of `(selected, hovered, pdf_backed)`.
#[must_use]
pub fn row_label_colour(selected: bool, hovered: bool, pdf_backed: bool) -> Color {
    let pro = Palette::pro();
    let paint = row_paint(selected, hovered);
    tree_row::locked_over(
        row_text_color(&Palette::fastmd_dark(), pdf_backed),
        paint.band,
        &pro,
    )
}

/// Stable id for the tree's search field ([AGENTS §5]).
pub const SEARCH_INPUT_ID: &str = "left_panel_search_input";

/// The search field's paint, resolved through the framework's text-input
/// atom at [`InputKind::Search`](pro_text_input::InputKind::Search)
/// (`UIF-ATOM-002`).
///
/// The same crossing point as
/// [`bottom::command_field_style`](super::bottom::command_field_style):
/// iced styles a `text_input` from `(Theme, Status)`, the framework
/// resolves one from `(InputKind, VisualState, Palette)`. Every channel
/// — fill, perimeter, value, placeholder, icon, selection — comes from
/// [`pro_text_input::resolve_input_style`] rather than from iced's own
/// theme.
///
/// This is the framework's *paint* on iced's *widget*, and that is as
/// far as the search row can go today:
/// [`composite::search_input`](iced_pro::composite) **does not exist**
/// (the composite directory has breadcrumb, card, expander, form_field,
/// info_bar, list_tile, notification, overflow_toolbar, scroll_view,
/// settings_row, table_* , toast, tree_row, tree_view, virtual_list), and
/// [`TextInput`](pro_text_input::TextInput)'s uncontrolled constructor
/// takes `on_input` only — Enter reaches a host solely as the
/// `Committed` tier of the *controlled* path, which needs an app-owned
/// `TextInputModel`. Submitting a query is the field's whole job, so the
/// value stays in `app.search()` and the field stays iced's.
///
/// Purity: pure function of `status`.
#[must_use]
pub fn search_field_style(
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    use iced::widget::text_input::{Status, Style};

    let palette = Palette::pro();
    let base = match status {
        Status::Active => BaseState::Rest,
        Status::Hovered => BaseState::Hovered,
        Status::Focused { .. } => BaseState::Rest,
        Status::Disabled => BaseState::Disabled,
    };
    let focused = matches!(status, Status::Focused { .. });
    let resolved = pro_text_input::resolve_input_style(
        pro_text_input::InputKind::Search,
        VisualState { base, focused },
        &palette,
    );

    Style {
        background: Background::Color(resolved.background),
        border: Border {
            color: resolved.border_color,
            width: resolved.border_width,
            radius: pro_text_input::RADIUS.into(),
        },
        icon: resolved.icon,
        placeholder: resolved.placeholder,
        value: resolved.text,
        selection: resolved.selection_background,
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
                .line_height(tree_row::LABEL_RUNG.iced_line_height())
                .color(palette.text_muted),
        )
        .padding(PANEL_PADDING)
        .into();
    }

    let cards = view_model.results.iter().fold(
        Column::<Message>::new().spacing(RESULT_LINE_SPACING),
        |cards, entry| cards.push(search_result_card(entry)),
    );

    scrollable(cards).height(Length::Fill).into()
}

/// One search-result card: title + match count, the library-relative path
/// and the matching snippet.
///
/// Every line is pinned to the rung it renders at — the title to
/// [`tree_row::LABEL_RUNG`], the three secondary lines to
/// [`typography::CAPTION`] — and the card's fixed height is
/// [`SEARCH_ROW_HEIGHT`], which is *derived* from those line boxes.
/// That pairing is what keeps a 58px slot from ever being shorter than
/// the 50px of text it has to hold.
fn search_result_card<'a>(entry: &'a SearchResultEntry) -> Element<'a, Message> {
    let palette = Palette::fastmd_dark();
    let caption_line = typography::CAPTION.iced_line_height();

    let mut title = Row::with_children(vec![
        text(&entry.file_name)
            .size(ROW_TEXT_SIZE)
            .line_height(tree_row::LABEL_RUNG.iced_line_height())
            .color(palette.text_strong)
            .into(),
    ]);
    if entry.match_count > 1 {
        title = title.push(
            text(strings::format_match_count(entry.match_count))
                .size(SECONDARY_TEXT_SIZE)
                .line_height(caption_line)
                .color(palette.text_faint),
        );
    }

    let mut lines = Column::<Message>::new()
        .spacing(RESULT_LINE_SPACING)
        .push(title);

    if !entry.relative_path.is_empty() && entry.relative_path != entry.file_name {
        lines = lines.push(
            text(&entry.relative_path)
                .size(SECONDARY_TEXT_SIZE)
                .line_height(caption_line)
                .color(palette.text_faint),
        );
    }

    if !entry.snippet.is_empty() {
        lines = lines.push(
            text(&entry.snippet)
                .size(SECONDARY_TEXT_SIZE)
                .line_height(caption_line)
                .font(italic_font())
                .color(palette.accent_text_subtle),
        );
    }

    mouse_area(
        container(lines)
            .width(Length::Fill)
            .height(Length::Fixed(SEARCH_ROW_HEIGHT))
            .padding(Padding::from([SEARCH_CARD_PADDING, SEARCH_CARD_PADDING])),
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
