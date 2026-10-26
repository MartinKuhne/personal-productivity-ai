//! Tests for the `UIF-PRIN-001` source scan.
//!
//! The scan is a gate that can fail in two directions: it can let a magic
//! number through, and — more dangerously — it can *report* that nothing
//! got through because its patterns quietly stopped matching. Every test
//! here is aimed at one of those two failure modes. The pinned counts are
//! the anti-vacuity half: a scanner whose regex had broken would report
//! zero violations *and* zero findings, and the pinned counts are what
//! makes the first impossible without the second also going red.
//!
//! The injected-failure tests follow the house pattern from
//! [`spec_coverage`](crate::spec_coverage): the real code path, one
//! doctored input, the guard observed firing on every run rather than
//! trusted.

use std::path::{Path, PathBuf};

use crate::token_lint::{
    ALLOW_LIST, Audit, COLOUR_CONSTRUCTORS, COLOUR_DEFINITION_FILE, Rule,
    SURFACE_GEOMETRY_CONSTANTS, ScalarClass, SourceScan, VISUAL_NAME_MARKERS, VISUAL_SCOPE_DIRS,
    Violation, audit_crate, crate_root, in_visual_scope, is_test_file, is_visual_name,
    join_continuation, redact_comments_and_strings, render, scan_source, source_files,
    surface_geometry,
};

// ---------------------------------------------------------------------------
// Pinned counts — the anti-vacuity half
//
// These are not aspirations. They are what the scan reports against the
// tree as it stands, and they move only when the tree moves deliberately.
// If a pattern stops matching, the scan goes quiet and these go red.
// ---------------------------------------------------------------------------

/// Production `.rs` files the scan reads (test fixtures excluded).
///
/// 96 as of the three `INC-44` modules joining the tree for
/// `UIF-ATOM-002d` — `widget/auto_suggest_model.rs`,
/// `widget/auto_suggest_host.rs` and `gallery_suggest.rs` — on top of the
/// 93 `INC-43` left behind.
///
/// 102 as of `INC-45`: the five additions of the `composite/` subsystem
/// — `mod.rs` and the four §3B composite modules — plus
/// `gallery_composite.rs`, the composite block of the verification
/// matrix. The gallery cell file is scanned but not visual-scoped, so it
/// joins the count without joining Rule B.
///
/// 104 as of `INC-46`, which added the disclosure and the scroll
/// contract: `composite/expander.rs` and `composite/scroll_view.rs`.
///
/// 107 as of `INC-47`, which added the notification subsystem:
/// `composite/notification.rs` (the shared severity and model),
/// `composite/info_bar.rs` and `composite/toast.rs`.
///
/// 118 as of `INC-50`, whose two additions are the trail and the strip —
/// `composite/breadcrumb.rs` and `composite/overflow_toolbar.rs`.
///
/// 119 as of the same increment's gallery split module,
/// `gallery_breadcrumb.rs`, which sits outside `composite/` and so joins
/// the file count without joining Rule B.
///
/// 120 as of `INC-57`, whose one addition is `binding/pre_rank.rs` — the
/// Pre-Rank pointer-capture ledger. Like the rest of `binding/` it is
/// scanned but sits outside Rule B's visual scope, so the file count moves
/// and `PIN_RULE_B_FILES` does not.
///
/// 121 as of `INC-58`, whose one addition is `token/time.rs` — the
/// `UIF-TOK-007` duration and easing scale. It lives under `token/`,
/// which is the layer the rules exist to protect rather than police, so
/// like every other token module it is scanned but never visual-scoped:
/// the file count moves and `PIN_RULE_B_FILES` does not.
///
/// 126 as of `B-1`, whose five additions are the `layout/` subsystem —
/// `layout/mod.rs`, `density.rs`, `emphasis.rs`, `footer.rs` and
/// `optical.rs` (`UIF-PRIN-006`). It defines no visual literal of its
/// own: every measurement it uses is a read-through of an existing
/// `token::` value, so widening Rule B to cover it would add no reach.
/// The file count moves and `PIN_RULE_B_FILES` does not.
///
/// 132 as of `B-2`, whose six additions are the `microcopy/` subsystem
/// — `microcopy/mod.rs`, `pins.rs`, `case_rule.rs`, `action_label.rs`,
/// `truncate.rs` and `tabular.rs` (`UIF-PRIN-007`). It carries string
/// and character rules rather than visual measurements, so like
/// `layout/` it is scanned but never visual-scoped.
///
/// 137 as of `B-3`, whose five additions are the `a11y/` subsystem —
/// `a11y/mod.rs`, `contract.rs`, `project.rs` and `tab_stop.rs` — plus
/// `surface/menu/aim.rs` (`UIF-PRIN-008`). The four `a11y/` files are
/// semantic projections that define no visual literal, so they join the
/// scanned set without joining Rule B's scope. `aim.rs` is different: it
/// sits under `surface/`, which is scoped by directory rather than by
/// content, so it is counted by Rule B even though its only number is a
/// read-through of `token::time::DELAY_SUBMENU`. That is why
/// `PIN_RULE_B_FILES` moves for the first time since `INC-50`.
///
/// 142 as of `B-4`, whose five additions are the `validation/` subsystem
/// — `validation/mod.rs`, `stance.rs`, `disallowed.rs`, `punish.rs` and
/// `recovery.rs` (`UIF-PRIN-009`). Like `layout/`, `microcopy/` and the
/// four `a11y/` files before it, it defines no visual literal: the
/// subsystem's only quantities are a stance, a character class and a
/// three-field snapshot, none of which a token could have supplied. The
/// file count moves and `PIN_RULE_B_FILES` does not.
/// 144 as of `B-5`, whose two additions are the tooltip warm-handoff
/// session — `surface/tooltip/warm.rs` — and its gallery block,
/// `gallery_warm.rs` (`UIF-TOK-007`). Only one of them joins Rule B:
/// `warm.rs` sits under `surface/`, which is scoped by directory rather
/// than by content, even though its only numbers are read-throughs of
/// `token::time`. `gallery_warm.rs` sits outside the visual scope, like
/// every other gallery cell file.
///
/// 152 as of the `RUST-053` split of `gallery.rs`, whose eight
/// additions are the subject-cohesive modules that file was cut into —
/// `gallery_button.rs`, `gallery_input.rs`, `gallery_menu.rs`,
/// `gallery_modal.rs`, `gallery_movable.rs`, `gallery_editor.rs`,
/// `gallery_tooltip.rs` and `gallery_audit.rs`. Every one of them is a
/// gallery cell file, which means it is scanned but not visual-scoped:
/// Rule B's scope is `widget/`, `surface/` and `composite/`, and the
/// gallery deliberately lives beside those rather than inside them,
/// because a cell records what a resolver produced instead of being the
/// code that paints it. The file count moves by eight and
/// `PIN_RULE_B_FILES` does not move at all — the split added no visual
/// literal to any scoped directory, and changed no literal in one either.
///
/// 155 as of the live tooltip widget — `surface/tooltip/widget.rs`
/// (`UIF-ATOM-007`). It sits under `surface/`, which is visual-scoped, so
/// `PIN_RULE_B_FILES` moves with it.
///
/// 157 as of `UIF-ATOM-005b`: `widget/range_slider.rs` and
/// `widget/range_slider_model.rs`. Both live under `widget/`, so both join
/// Rule B's visual scope.
///
/// 160 as of `UIF-COMP-014`: `composite/empty_state.rs`, `composite/skeleton.rs`
/// and `composite/pagination.rs`. All three live under `composite/`, joining
/// Rule B's visual scope.
///
/// 162 as of `UIF-ATOM-013`: `widget/avatar.rs` and `widget/rating.rs`. Both
/// live under `widget/`, joining Rule B's visual scope.
///
/// 164 as of `UIF-COMP-011`: `composite/stepper.rs` and `composite/pips_pager.rs`. Both
/// live under `composite/`, joining Rule B's visual scope.
///
/// 165 as of `UIF-COMP-012`: `composite/side_sheet.rs`, joining Rule B's visual scope.
///
/// 166 as of `UIF-COMP-009`: `composite/navigation_rail.rs`, joining Rule B's visual scope.
///
/// 167 as of `UIF-COMP-010`: `composite/command_palette.rs`, joining Rule B's visual scope.
///
/// 168 as of `UIF-COMP-013`: `composite/grid_view.rs`, joining Rule B's visual scope.
///
/// 171 as of `UIF-COMP-015`: `composite/code_block.rs`, `composite/diff_viewer.rs`,
/// and `composite/keybind_recorder.rs`, joining Rule B's visual scope.
///
/// 172 as of Increment 18 (`UIF-ATOM-010`): `widget/date_picker.rs`, joining Rule B's visual scope.
const PIN_FILES_SCANNED: usize = 172;
/// Files Rule A treats as the colour-definition layer.
const PIN_RULE_A_FILES: usize = 1;
/// Files Rule B treats as widget/surface code.
///
/// 60 as of the two `INC-44` autosuggest modules joining `widget/`. Rule
/// B counts scope membership, so a file with no colour literals still
/// counts; the gallery cells sit outside `widget/`, `surface/` and
/// `composite/` and so are not among them.
///
/// 65 as of `INC-45`, which brought `composite/` into Rule B's scope and
/// added its four composite modules plus the subsystem `mod.rs`.
///
/// 67 as of `INC-46`, whose two additions are `composite/expander.rs`
/// and `composite/scroll_view.rs`.
///
/// 70 as of `INC-47`, whose three additions are the notification
/// subsystem's modules — `composite/notification.rs`,
/// `composite/info_bar.rs` and `composite/toast.rs`.
///
/// 77 as of `INC-49`, whose four additions are the structured grid's
/// resolvers — `composite/table_column.rs`, `composite/table_cell.rs`,
/// `composite/table_header.rs` and `composite/table_view.rs`.
///
/// 79 as of `INC-50`, whose two additions are `composite/breadcrumb.rs`
/// and `composite/overflow_toolbar.rs`.
///
/// 80 as of `B-3`, the one addition being `surface/menu/aim.rs`. Rule B
/// scopes by directory, not by content, so a file under `surface/` is
/// counted whether or not it carries a colour literal — and `aim.rs`
/// carries none, taking its only number from
/// `token::time::DELAY_SUBMENU`.
/// 81 as of `B-5`, the one addition being `surface/tooltip/warm.rs`.
/// Like `aim.rs` before it, it is counted because of where it lives, not
/// because of what it contains: the module defines no literal of its own
/// and takes both of its numbers from `token::time`.
/// 82 as of `UIF-ATOM-003`, whose one addition is `widget/toggle_widget.rs`.
/// 83 as of `UIF-ATOM-007`, whose one addition is `surface/tooltip/widget.rs`.
/// 85 as of `UIF-ATOM-005b`, whose two additions are `widget/range_slider.rs`
/// and `widget/range_slider_model.rs`.
/// 88 as of `UIF-COMP-014`, whose three additions are `composite/empty_state.rs`,
/// `composite/skeleton.rs` and `composite/pagination.rs`.
/// 90 as of `UIF-ATOM-013`, whose two additions are `widget/avatar.rs` and
/// `widget/rating.rs`.
/// 92 as of `UIF-COMP-011`, whose two additions are `composite/stepper.rs` and
/// `composite/pips_pager.rs`.
/// 93 as of `UIF-COMP-012`, whose one addition is `composite/side_sheet.rs`.
/// 94 as of `UIF-COMP-009`, whose one addition is `composite/navigation_rail.rs`.
/// 95 as of `UIF-COMP-010`, whose one addition is `composite/command_palette.rs`.
/// 96 as of `UIF-COMP-013`, whose one addition is `composite/grid_view.rs`.
/// 99 as of `UIF-COMP-015`, whose three additions are `composite/code_block.rs`,
/// `composite/diff_viewer.rs`, and `composite/keybind_recorder.rs`.
/// 100 as of Increment 18 (`UIF-ATOM-010`): `widget/date_picker.rs`.
const PIN_RULE_B_FILES: usize = 100;
/// Colour constructors found inside `token/color.rs`.
const PIN_COLOUR_DEFINITIONS: usize = 26;
/// Visual scalars reading a token path.
///
/// 124 as of `INC-42`, whose fifteen additions are the nine `slider::`
/// geometry consts (`TRACK_HEIGHT`, `TRACK_RADIUS`, `THUMB_SIZE`,
/// `THUMB_HIT_SIZE`, `THUMB_RING_WIDTH`, `RAIL_BORDER_WIDTH`,
/// `TICK_WIDTH`, `TICK_HEIGHT`, `FOCUS_RING_WIDTH`) and the six
/// `number_box::` ones (`ENTRY_HEIGHT`, `STEPPER_WIDTH`,
/// `STEP_BUTTON_SIZE`, `GLYPH_SIZE`, `RADIUS`, `BORDER_WIDTH`). Every one
/// is a `pub const` naming a `token::*` path. The one literal this
/// increment introduces — [`slider::FILLED_ON_UNFILLED_RATIO`] — is a
/// recorded measurement rather than a visual primitive, carries none of
/// the name markers Rule B scans for, and so is not classified at all.
///
/// [`VISUAL_NAME_MARKERS`]: crate::token_lint::VISUAL_NAME_MARKERS
///
/// 127 as of `INC-44`, whose three additions are the autosuggest
/// flyout's token-bound figures — `FLYOUT_ROW_HEIGHT` (`size.control.md`),
/// `FLYOUT_GAP` (`space.xs`) and `FLYOUT_RADIUS` (`shape.radius.widget`).
/// The fourth figure that increment introduces, `FLYOUT_MAX_HEIGHT`, is a
/// bare `240.0` and is registered as a Surface Geometry Constant rather
/// than token-bound, because §3.2 fixes the number outright.
///
/// 146 as of `INC-45`, whose nineteen additions are the token-bound
/// figures the four §3B composites read: the field's `BAND_GAP`,
/// `CONTROL_SLOT_HEIGHT` and `CAPTION_SLOT_HEIGHT`; the settings row's
/// paddings, column gap, border, radius and two rungs; the card's radius,
/// padding and border; and the tile's paddings, slot gap, leading and
/// trailing extents, border and rungs.
///
/// 157 as of `INC-46`, whose eleven additions are the figures the
/// disclosure and the scroll contract read: the expander's two header
/// rungs, its horizontal padding, its chevron-to-title gap, its body
/// padding and its body border; and the scroll view's resting, active and
/// reserved track widths, its tick width and its thumb radius.
///
/// The rest of `INC-46`'s numbers deliberately do *not* join this count,
/// because none of them carries a name marker Rule B scans for — and that
/// is the honest classification rather than an oversight. The chevron's
/// two settled angles and its sweep are rotations, not lengths. The
/// thumb's minimum length, the arrow step, the page-keep, the at-bottom
/// tolerance and the reach-end distance are behavioural measures: they
/// decide *when* something happens, not how large a painted box is, and
/// `REACH_END_DISTANCE` in particular paints nothing at all.
///
/// 167 as of `INC-47`, whose ten additions are the figures the banner
/// and the transient read: the bar's minimum height, its two paddings,
/// its stripe width, its icon-to-title gap, its action-to-dismiss gap,
/// its border weight and its radius; plus the toast's stack gap and its
/// width clamp. What deliberately does *not* join is equally honest — the
/// bar's icon box and dismiss target carry no name marker, its two rungs
/// are `&TypeRung` rather than `f32`, the toast's two gutters are named
/// `*_GUTTER` (no marker), `MAX_VISIBLE` is a `usize`, and `WIDTH` is a
/// bare `360.0` registered as a Surface Geometry Constant because §2.3
/// has no `size.toast.width` rung for it to read.
///
/// 186 as of `INC-49`, whose eleven additions are the structured grid's
/// token reads: `table_cell::{CELL_PADDING_X, CELL_PADDING_Y,
/// BODY_ROW_MIN_HEIGHT, GRID_LINE_WIDTH}`, `table_header::{HEADER_HEIGHT,
/// RESIZE_GRAB, HEADER_BORDER_WIDTH, CELL_TITLE_PADDING_X,
/// SORT_INDICATOR_SIZE, SORT_INDICATOR_GAP}` and
/// `table_view::ROW_HEIGHT` (which reads `BODY_ROW_MIN_HEIGHT` rather
/// than restating 32px, so the row height has exactly one source).
///
/// 188 as of `INC-50`, whose two additions are the two minimums that
/// read a sibling rather than a number — `breadcrumb::MIN_SEGMENT_WIDTH`
/// and `overflow_toolbar::MIN_ITEM_WIDTH`, both aliased to their own
/// 24px segment height so a crumb or a strip item can never be narrower
/// than it is tall. They are token-bound in Rule B's sense because the
/// expression carries no literal at all; the ten figures that *are*
/// literals join the §1.1 registry below instead.
/// 189 as of `INC-53`, whose one addition is
/// `list_tile::FOCUS_OUTLINE_WIDTH` — the `1px` inset keyboard-focus
/// outline §3B.5 prescribes for a focused tile, read from
/// `shape::BORDER_HAIRLINE` rather than written as a literal. Note that
/// `tree_row::SELECTION_BAR_WIDTH` did *not* move the count when §3B.6
/// re-priced it from `border.focus + border.hairline` to a single
/// `border.focus`: Rule B counts the declaration, not the number of
/// token references inside it.
/// 192 as of `INC-54`, whose three additions are figures the corrected
/// spec makes *summable* rather than merely printed:
/// `settings_row::BORDER_INSET_RESERVE` — the `4px` of border/inset the
/// §3B.1 floor carries on top of `12 + 20 + 12`, read from `space::XS`;
/// `list_tile::TITLE_SUBTITLE_GAP` — the `2px` title-to-subtitle gap that
/// makes §3B.5's two-line total close, read from `space::XXS`; and
/// `settings_row::MIN_HEIGHT` itself, which left the registry below the
/// moment it became that sum instead of a bare `48.0`. Two figures that
/// changed in the same pass did *not* move the count, and are named here
/// so the gap is on the record: `scroll_view::REACH_END_DISTANCE` was a
/// literal `64.0` and is now `space::XXL * 2.0`, but "DISTANCE" carries
/// none of Rule B's name markers, so the rule never inspected it either
/// way; and `slider::TICK_WIDTH` / `slider::TICK_HEIGHT` were already
/// token-bound, so re-pointing them from `shape::BORDER_FOCUS` and
/// `size::SLIDER_TRACK_HEIGHT` at the §2.1 spacing rungs is invisible to
/// a counter that counts declarations rather than provenance.
/// 193 as of `INT-11`, whose one addition is
/// `tree_row::GUIDE_WIDTH` — the `1px` depth-guide stroke, read from
/// `shape::BORDER_HAIRLINE`. It was previously an inline literal inside
/// `row_slots`; lifting it to a named constant is what lets a host painting
/// its own guides (a `Stack` of styled `container`s rather than a canvas
/// painter) and the composite's own `guides` rectangles share one source
/// for §3B.6's "vertical `1px` tree depth guides". The count moves by one
/// because a declaration was added, not because a number changed — the same
/// asymmetry `SELECTION_BAR_WIDTH` demonstrated above.
///
/// 194 as of `UIF-ATOM-007`: `surface/tooltip/mod.rs`'s `SHORTCUT_GAP` bound
/// to `token::space::SM`.
///
/// 207 as of `UIF-ATOM-005b`: the 13 `range_slider::` geometry and readout
/// constants (`TRACK_HEIGHT`, `TRACK_RADIUS`, `THUMB_SIZE`, `THUMB_HIT_SIZE`,
/// `THUMB_RING_WIDTH`, `RAIL_BORDER_WIDTH`, `READOUT_GAP`, `READOUT_RADIUS`,
/// `READOUT_BORDER_WIDTH`, `READOUT_FONT_SIZE`, `READOUT_PADDING_X`,
/// `READOUT_PADDING_Y`, `FOCUS_RING_WIDTH`).
///
/// 225 as of `UIF-COMP-014`: the 18 token-bound visual constants in
/// `empty_state::` (`MAX_WIDTH`, `PADDING`, `BADGE_SIZE`, `BADGE_RADIUS`,
/// `BADGE_BORDER_WIDTH`, `ICON_SIZE`, `BADGE_GAP`, `TEXT_GAP`, `ACTION_GAP`),
/// `skeleton::` (`DEFAULT_RADIUS`, `BORDER_WIDTH`, `TREE_ROW_HEIGHT`,
/// `TABLE_ROW_HEIGHT`, `TABLE_ROW_COMPACT_HEIGHT`), and
/// `pagination::` (`BAR_HEIGHT`, `BORDER_WIDTH`, `BUTTON_SIZE`, `PADDING_X`).
///
/// 240 as of `UIF-COMP-011`: the 15 token-bound visual constants in
/// `stepper::` (`BAR_HEIGHT`, `NODE_SIZE`, `NODE_RADIUS`, `CONNECTOR_STROKE`,
/// `CURRENT_BORDER_WIDTH`, `LABEL_GAP`, `STEP_GAP`, `FOOTER_BUTTON_HEIGHT`) and
/// `pips_pager::` (`BAR_HEIGHT`, `BUTTON_SIZE`, `PIP_SIZE_INACTIVE`,
/// `PIP_WIDTH_ACTIVE`, `PIP_HEIGHT`, `PIP_GAP`, `PIP_RADIUS`).
///
/// 248 as of `UIF-COMP-012`: the 8 token-bound visual constants in
/// `side_sheet::` (`HEADER_HEIGHT`, `GRAB_WIDTH`, `GRIP_PILL_WIDTH`,
/// `GRIP_PILL_HEIGHT`, `GRIP_PILL_RADIUS`, `DIVIDER_STROKE`,
/// `HEADER_BUTTON_SIZE`, `BODY_PADDING`).
///
/// 254 as of `UIF-COMP-009`: the 6 token-bound visual constants in
/// `navigation_rail::` (`ROW_HEIGHT`, `TOGGLE_SIZE`, `ICON_SIZE`,
/// `INDICATOR_WIDTH`, `GRAB_WIDTH`, `DIVIDER_STROKE`).
///
/// 260 as of `UIF-COMP-010`: the 6 token-bound visual constants in
/// `command_palette::` (`ROW_SINGLE_HEIGHT`, `FOOTER_HEIGHT`, `ICON_SIZE`,
/// `RADIUS`, `GROUP_HEADER_HEIGHT`, `DIVIDER_STROKE`).
///
/// 269 as of `UIF-COMP-013`: the 9 token-bound visual constants in
/// `grid_view::` (`GAP`, `GUTTER_WIDTH`, `CHECKBOX_SIZE`, `RADIUS`,
/// `BORDER_SELECTED_WIDTH`, `BORDER_REST_WIDTH`, `CHECKBOX_INSET`,
/// `TILE_PADDING`, `ICON_SIZE`).
///
/// 294 as of `UIF-COMP-015`: the 25 token-bound visual constants in `code_block::`
/// (`HEADER_HEIGHT`, `COPY_BUTTON_SIZE`, `RADIUS`, `BORDER_WIDTH`, `CODE_PADDING`,
/// `GUTTER_PADDING_X`), `diff_viewer::` (`HEADER_HEIGHT`, `INDICATOR_BAR_WIDTH`,
/// `RADIUS`, `BORDER_WIDTH`, `DIVIDER_WIDTH`, `BUTTON_HEIGHT`, `BUTTON_PADDING_X`,
/// `BUTTON_GAP`, `LINE_HEIGHT`), and `keybind_recorder::` (`HEIGHT`, `KEYCAP_HEIGHT`,
/// `CLEAR_BUTTON_SIZE`, `RADIUS`, `BORDER_REST_WIDTH`, `BORDER_FOCUS_WIDTH`,
/// `KEYCAP_RADIUS`, `KEYCAP_PADDING_X`, `KEYCAP_GAP`, `PADDING_X`).
///
/// 302 as of Increment 18 (`UIF-ATOM-010`): the 8 token-bound visual constants in `date_picker::`
/// (`TRIGGER_HEIGHT`, `TRAILING_BUTTON_SIZE`, `HEADER_HEIGHT`, `NAV_BUTTON_SIZE`,
/// `WEEKDAY_ROW_HEIGHT`, `CELL_SIZE`, `RADIUS`, `FLYOUT_PADDING`).
const PIN_TOKEN_BOUND: usize = 302;
/// Visual scalars bound to `0.0`.
const PIN_ZERO: usize = 3;
/// Visual scalars bound to a sub-unit proportion.
const PIN_RATIO: usize = 1;
/// Visual scalars covered by the §1.1 registry.
///
/// 21 as of the three figures §3.2 and §3.4 fix outright for a
/// completion flyout and a Select trigger:
/// `auto_suggest_model::FLYOUT_MAX_HEIGHT` (240px),
/// `combo_model::FLYOUT_MAX_HEIGHT` (280px) and `combo_box::MIN_WIDTH`
/// (120px), which therefore need a registered authority rather than a
/// token.
///
/// 24 as of `INC-45`, which registered the three §3B figures no rung
/// carries: `form_field::HEADER_HEIGHT` (20px), `settings_row::MIN_HEIGHT`
/// (48px) and `list_tile::TWO_LINE_HEIGHT` (44px).
///
/// 25 as of `INC-47`, which registered `toast::WIDTH` (360px) against
/// §3B.2 — the same treatment §3.7's popover width range gets.
///
/// 26 as of `INC-49`, which registered `table_header::RESIZE_REACH` —
/// half of §3.7's 10px column-resize grab, the reach split evenly across
/// the divider so the sort target and the handle cannot overlap. No rung
/// carries a half-grab, so the figure needs the registry rather than a
/// token read.
///
/// 36 as of `INC-50`, which registers the ten §3B.8 figures no rung
/// carries: the trail's `BAR_HEIGHT` (28px), `SEGMENT_HEIGHT` (24px),
/// `SEGMENT_PADDING_X` (6px), `SEGMENT_GAP` (4px) and
/// `ELLIPSIS_TRIGGER_WIDTH` (28px), plus the strip's `STRIP_HEIGHT`
/// (28px), `ITEM_HEIGHT` (24px), `ITEM_GAP` (4px), `TRIGGER_SIZE`
/// (28px) and `TRIGGER_GAP` (8px). §3B.8 fixes each of these outright
/// as part of the two surfaces' geometry, and §2.3 has no rung for a
/// 28px bar or a 24px Ghost-button row, so the registry is the only
/// authority available. `TRIGGER_RESERVE` deliberately does *not* join,
/// and neither does it land anywhere else: it is `TRIGGER_SIZE +
/// TRIGGER_GAP`, a derived sum whose name carries none of the markers
/// Rule B scans for, so the rule never inspects it.
/// 35 as of `INC-54`, which removes an entry rather than adding one:
/// `settings_row::MIN_HEIGHT` is now §3B.1's own `12 + 20 + 12 + 4`
/// composition rather than a literal `48.0`, so Rule B reads a token on
/// every term and passes it with no carve-out. The registry is the
/// authority for figures no token carries; a derived total is not one,
/// and leaving the entry would have claimed a §-backed literal that no
/// longer exists in the file.
/// 36 as of `UIF-MOD-004`, which adds `modal::ATTRIBUTIONS_LIST_MAX_HEIGHT`
/// (200px). The value is a stated bound in the §2.2 closed table, not a
/// multiple of any rung — the same standing `input_dialog::FOLDER_LIST_MAX_HEIGHT`
/// has, and it is registered for the same reason. Nothing was re-tuned and
/// no existing entry changed class.
/// 40 as of `UIF-COMP-012`, which registers the four §3B.12 figures:
/// `side_sheet::DEFAULT_WIDTH` (360px), `side_sheet::MIN_WIDTH` (280px),
/// `side_sheet::MAX_WIDTH` (520px), and `side_sheet::FOOTER_HEIGHT` (48px).
/// 44 as of `UIF-COMP-009`, which registers the four §3B.9 figures:
/// `navigation_rail::COMPACT_WIDTH` (48px), `navigation_rail::EXPANDED_DEFAULT_WIDTH` (220px),
/// `navigation_rail::EXPANDED_MIN_WIDTH` (180px), and `navigation_rail::EXPANDED_MAX_WIDTH` (320px).
/// 48 as of `UIF-COMP-010`, which registers the four §3B.10 figures:
/// `command_palette::CARD_WIDTH` (560px), `command_palette::CARD_HEIGHT` (380px),
/// `command_palette::SEARCH_HEIGHT` (40px), and `command_palette::ROW_DOUBLE_HEIGHT` (44px).
/// 50 as of `UIF-COMP-013`, which registers the two §3B.13 figures:
/// `grid_view::DEFAULT_MIN_TILE_WIDTH` (160px) and `grid_view::DEFAULT_TILE_HEIGHT` (120px).
/// 52 as of `UIF-COMP-015`, which registers the two §3B.15 figures:
/// `code_block::GUTTER_WIDTH` (40px) and `diff_viewer::GUTTER_WIDTH` (40px).
/// 53 as of Increment 18 (`UIF-ATOM-010`), which registers `date_picker::CALENDAR_FLYOUT_WIDTH` (256px).
const PIN_SURFACE_BOUND: usize = 53;
/// Exceptions the crate currently takes.
const PIN_ALLOW_ENTRIES: usize = 1;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// An independent recursive `.rs` walk, so the walker itself is checked
/// against something that is not the walker.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn violations_of(scan: &SourceScan) -> Vec<Violation> {
    scan.findings.clone()
}

fn only(scan: &SourceScan) -> Violation {
    assert_eq!(
        scan.findings.len(),
        1,
        "expected exactly one finding, got:\n  {:?}",
        scan.findings
    );
    scan.findings[0].clone()
}

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

#[test]
fn the_crate_passes_the_token_scan() {
    let audit = audit_crate();
    assert!(
        audit.is_clean(),
        "UIF-PRIN-001 source scan failed:\n{}",
        audit.describe()
    );
}

#[test]
fn the_allow_list_is_small_and_every_entry_actually_fires() {
    let audit = audit_crate();

    // Widening the allow-list is the easy way to make this file go green,
    // so the size is pinned: adding an entry is a deliberate act that
    // shows up here, not a quiet fix.
    assert_eq!(
        ALLOW_LIST.len(),
        PIN_ALLOW_ENTRIES,
        "the allow-list has {} entries; the pinned count is {PIN_ALLOW_ENTRIES}. \
         Fix the finding rather than excuse it.",
        ALLOW_LIST.len(),
    );

    // Every entry must suppress something real. A stale entry is a lie
    // about the state of the tree and fails the build on its own.
    assert_eq!(
        audit.suppressed.len(),
        ALLOW_LIST.len(),
        "every allow entry must fire; stale: {:?}",
        audit.stale_allow_entries,
    );
    assert!(
        audit.stale_allow_entries.is_empty(),
        "stale allow entries suppress nothing: {:?}",
        audit.stale_allow_entries,
    );

    // Each entry carries a reason, and the reasons are distinct — a
    // blanket "it's fine" repeated down the list is not a justification.
    let mut reasons: Vec<&str> = ALLOW_LIST.iter().map(|e| e.reason).collect();
    for entry in ALLOW_LIST {
        assert!(
            entry.reason.trim().len() > 40,
            "allow entry `{}` needs a real reason, got {:?}",
            entry.key,
            entry.reason,
        );
        assert!(
            entry.key.contains('/') && entry.key.contains(':'),
            "allow key must be `file:identity`, got {}",
            entry.key,
        );
    }
    reasons.sort_unstable();
    reasons.dedup();
    assert_eq!(reasons.len(), ALLOW_LIST.len(), "reasons must be distinct");
}

// ---------------------------------------------------------------------------
// Anti-vacuity: the scan is reading what it claims to read
// ---------------------------------------------------------------------------

#[test]
fn the_scan_reads_every_production_file_and_agrees_with_an_independent_walk() {
    let audit = audit_crate();
    assert_eq!(audit.stats.files_scanned, PIN_FILES_SCANNED);

    let root = crate_root();
    let mut all = Vec::new();
    walk(&root, &mut all);
    let expected: Vec<String> = all
        .iter()
        .map(|p| rel(&root, p))
        .filter(|r| !is_test_file(r))
        .collect();

    assert_eq!(
        audit.stats.files_scanned,
        expected.len(),
        "the scan read {} files but an independent walk finds {} non-test .rs files",
        audit.stats.files_scanned,
        expected.len(),
    );
    assert!(
        !expected.is_empty(),
        "the independent walk found nothing — the fixture is broken, not clean"
    );
}

#[test]
fn the_rule_scopes_are_what_they_claim_to_be() {
    let audit = audit_crate();
    assert_eq!(audit.stats.rule_a_files, PIN_RULE_A_FILES);
    assert_eq!(audit.stats.rule_b_files, PIN_RULE_B_FILES);
    assert_eq!(VISUAL_SCOPE_DIRS, ["widget/", "surface/", "composite/"]);
    assert_eq!(COLOUR_DEFINITION_FILE, "token/color.rs");
    assert!(in_visual_scope("widget/button.rs"));
    assert!(in_visual_scope("surface/movable.rs"));
    assert!(!in_visual_scope("token/color.rs"));
    assert!(!in_visual_scope("geometry/clamp.rs"));
    assert!(!in_visual_scope("gallery.rs"));
}

#[test]
fn the_scanner_actually_finds_colour_definitions_in_the_token_layer() {
    // This is the single most important assertion in the file. Rule A
    // passes by finding nothing; this is the only thing that distinguishes
    // "no colours outside the token layer" from "the pattern is broken".
    let audit = audit_crate();
    assert_eq!(
        audit.stats.token_layer_colour_definitions, PIN_COLOUR_DEFINITIONS,
        "the scan must still find the colour definitions it is calibrated against",
    );
    assert!(audit.stats.token_layer_colour_definitions > 0);
}

#[test]
fn the_scanner_actually_finds_visual_scalars_and_classifies_them() {
    let audit = audit_crate();
    assert_eq!(audit.stats.token_bound, PIN_TOKEN_BOUND);
    assert_eq!(audit.stats.zero, PIN_ZERO);
    assert_eq!(audit.stats.ratio, PIN_RATIO);
    assert_eq!(audit.stats.surface_bound, PIN_SURFACE_BOUND);

    let total =
        audit.stats.token_bound + audit.stats.zero + audit.stats.ratio + audit.stats.surface_bound;
    assert!(
        total >= 60,
        "Rule B classified only {total} visual scalars; the parser has likely stopped matching"
    );
    // Token-bound must dominate: if it collapsed to zero while the others
    // held, the token-path pattern is what broke.
    assert!(audit.stats.token_bound > audit.stats.surface_bound);
}

// ---------------------------------------------------------------------------
// Rule A — injection tests
// ---------------------------------------------------------------------------

#[test]
fn rule_a_fires_on_a_colour_injected_into_a_widget() {
    let src = "pub fn paint() {\n    let c = Color::from_rgb(255, 128, 0);\n}\n";
    let scan = scan_source("widget/injected.rs", src);
    let v = only(&scan);
    assert_eq!(v.rule, Rule::ColourLiteral);
    assert_eq!(v.file, "widget/injected.rs");
    assert_eq!(v.line, 2);
    assert_eq!(v.detail, "Color::from_rgb");
    assert_eq!(v.key(), "widget/injected.rs:Color::from_rgb");
}

#[test]
fn rule_a_fires_on_every_banned_constructor() {
    for ctor in COLOUR_CONSTRUCTORS {
        let src = format!("fn f() {{ let c = {ctor}(1, 2, 3, 1.0); }}\n");
        let scan = scan_source("widget/injected.rs", &src);
        let v = only(&scan);
        assert_eq!(
            v.detail, ctor,
            "`{ctor}` must be reported as itself, not mis-attributed"
        );
        assert_eq!(v.rule, Rule::ColourLiteral);
    }
    assert_eq!(
        COLOUR_CONSTRUCTORS,
        [
            "Color::from_rgb",
            "Color::from_rgba",
            "Color::from_rgb8",
            "Color::from_rgba8"
        ],
        "the banned set is closed; adding to it is a deliberate act"
    );
}

#[test]
fn rule_a_fires_on_hex_colour_literals_outside_a_string() {
    // The scanner is text-based, so the `0x` form is the live one in Rust
    // source. The `#` form is asserted at the text level because the
    // module doc claims it: a bare `#rrggbb` is not valid Rust, but the
    // rule is stated over text and this proves the branch is wired.
    for src in [
        "let c = 0x1A2B3C;\n",
        "let c = 0x1A2B3C4D;\n",
        "let c = #ff8800;\n",
    ] {
        let scan = scan_source("widget/injected.rs", src);
        let v = only(&scan);
        assert_eq!(v.rule, Rule::ColourLiteral, "`{src}` must be caught");
        assert_eq!(v.detail, "hex colour literal");
    }
}

#[test]
fn rule_a_does_not_fire_inside_the_token_layer_but_still_counts_it() {
    let src = "pub const RED: Color = Color::from_rgb(255, 0, 0);\n";
    let scan = scan_source(COLOUR_DEFINITION_FILE, src);
    assert!(
        scan.findings.is_empty(),
        "the token layer is the one place a colour may be defined: {:?}",
        scan.findings
    );
    assert_eq!(scan.token_layer_colour_definitions, 1);

    // Same text, different file: the definition count must NOT accrue
    // outside the token layer, or the count proves nothing.
    let elsewhere = scan_source("widget/red.rs", src);
    assert_eq!(elsewhere.token_layer_colour_definitions, 0);
    assert_eq!(elsewhere.findings.len(), 1);
}

#[test]
fn rule_a_ignores_a_colour_written_in_a_comment_or_a_string() {
    let src = concat!(
        "// Color::from_rgb(1, 2, 3) is only prose\n",
        "/* block: Color::from_rgba8 */\n",
        "let s = \"Color::from_rgb(0, 0, 0)\";\n",
        "let r = r#\"#ff0000 and Color::from_rgb\"#;\n",
    );
    let scan = scan_source("widget/prose.rs", src);
    assert!(
        scan.findings.is_empty(),
        "documentation is not code: {:?}",
        scan.findings
    );
}

// ---------------------------------------------------------------------------
// Rule B — injection tests
// ---------------------------------------------------------------------------

#[test]
fn rule_b_fires_on_an_injected_bare_measurement() {
    let src = "pub const INJECTED_GAP: f32 = 13.0;\n";
    let scan = scan_source("widget/injected.rs", src);
    let v = only(&scan);
    assert_eq!(v.rule, Rule::UnboundVisualScalar);
    assert_eq!(v.constant_name(), Some("INJECTED_GAP"));
    assert_eq!(v.line, 1);
    assert!(
        v.detail.contains("13.0"),
        "the report must name the literal: {}",
        v.detail
    );
    assert_eq!(v.key(), "widget/injected.rs:INJECTED_GAP");
}

#[test]
fn rule_b_accepts_the_classes_the_spec_itself_draws() {
    let src = concat!(
        "pub const TOKEN_GAP: f32 = space::MD;\n",
        "pub const COMPOSITE_PADDING: f32 = space::SM + space::XXS;\n",
        "pub const ZERO_PADDING: f32 = 0.0;\n",
        "pub const SLOT_RATIO: f32 = 0.25;\n",
    );
    let scan = scan_source("widget/ok.rs", src);
    assert!(scan.findings.is_empty(), "{:?}", scan.findings);
    assert_eq!(scan.token_bound, 2, "a sum of tokens is still token-bound");
    assert_eq!(scan.zero, 1);
    assert_eq!(scan.ratio, 1);
    assert_eq!(scan.surface_bound, 0);
}

#[test]
fn rule_b_grants_the_carve_out_only_to_a_registered_constant() {
    // Registered: §4.7.1 fixes the movable card at 680x440.
    let registered = "pub const CARD_DEFAULT_WIDTH: f32 = 680.0;\n";
    let scan = scan_source("surface/movable.rs", registered);
    assert!(scan.findings.is_empty(), "{:?}", scan.findings);
    assert_eq!(scan.surface_bound, 1);

    // Same name, same literal, a file that never registered it. The
    // exemption is the registry, not the name — so this is a violation.
    let unregistered = scan_source("surface/input_dialog.rs", registered);
    let v = only(&unregistered);
    assert_eq!(v.rule, Rule::UnboundVisualScalar);
    assert_eq!(v.constant_name(), Some("CARD_DEFAULT_WIDTH"));
    assert_eq!(unregistered.surface_bound, 0);
}

#[test]
fn rule_b_ignores_constants_whose_name_is_not_a_visual_measurement() {
    let src = "pub const TIMEOUT_SECONDS: f32 = 13.0;\npub const MAX_RETRIES: f32 = 4.0;\n";
    let scan = scan_source("widget/notvisual.rs", src);
    assert!(scan.findings.is_empty(), "{:?}", scan.findings);
    assert_eq!(
        scan.token_bound + scan.zero + scan.ratio + scan.surface_bound,
        0
    );
}

#[test]
fn rule_b_ignores_non_pub_const_fn_and_non_f32() {
    let src = concat!(
        "const PRIVATE_GAP: f32 = 13.0;\n",
        "pub const fn gap_fn() -> f32 { 13.0 }\n",
        "pub const WIDTH_U32: u32 = 13;\n",
    );
    let scan = scan_source("widget/other.rs", src);
    assert!(
        scan.findings.is_empty(),
        "only `pub const … : f32` is in scope: {:?}",
        scan.findings
    );
}

#[test]
fn rule_b_does_not_reach_outside_the_visual_scope() {
    let src = "pub const GAP: f32 = 13.0;\n";
    for file in [
        "geometry/clamp.rs",
        "token/space.rs",
        "gallery.rs",
        "state.rs",
    ] {
        let scan = scan_source(file, src);
        assert!(
            scan.findings.is_empty(),
            "{file} is outside widget/ and surface/ and must not be scanned by Rule B"
        );
    }
}

// ---------------------------------------------------------------------------
// Real-tree injection — the house pattern
// ---------------------------------------------------------------------------

#[test]
fn the_scan_fires_on_a_scalar_injected_into_a_real_production_file() {
    let root = crate_root();
    let path = root.join("surface").join("movable.rs");
    let text = std::fs::read_to_string(&path).expect("movable.rs must be readable");

    let baseline = scan_source("surface/movable.rs", &text);
    assert!(
        baseline.findings.is_empty(),
        "the real file must be clean before injection: {:?}",
        baseline.findings
    );

    let injected = format!("{text}\npub const INJECTED_STROKE: f32 = 7.0;\n");
    let after = scan_source("surface/movable.rs", &injected);
    assert_eq!(
        after.findings.len(),
        1,
        "exactly the injected constant must be new: {:?}",
        after.findings
    );
    assert_eq!(after.findings[0].constant_name(), Some("INJECTED_STROKE"));
    assert_eq!(after.token_bound, baseline.token_bound);
    assert_eq!(after.zero, baseline.zero);
    assert_eq!(after.ratio, baseline.ratio);
    assert_eq!(after.surface_bound, baseline.surface_bound);
}

#[test]
fn the_scan_fires_on_a_colour_injected_into_a_real_production_file() {
    let root = crate_root();
    let path = root.join("widget").join("button.rs");
    let text = std::fs::read_to_string(&path).expect("button.rs must be readable");

    let baseline = scan_source("widget/button.rs", &text);
    assert!(
        baseline.findings.is_empty(),
        "the real file must be clean before injection: {:?}",
        baseline.findings
    );

    let injected = format!("{text}\nfn injected() {{ let c = Color::from_rgb8(9, 9, 9, 1.0); }}\n");
    let after = scan_source("widget/button.rs", &injected);
    let v = only(&after);
    assert_eq!(v.rule, Rule::ColourLiteral);
    assert_eq!(v.detail, "Color::from_rgb8");
}

// ---------------------------------------------------------------------------
// Registry integrity
// ---------------------------------------------------------------------------

#[test]
fn every_surface_geometry_entry_is_registered_against_a_file_in_scope_and_a_visual_name() {
    assert_eq!(
        SURFACE_GEOMETRY_CONSTANTS.len(),
        PIN_SURFACE_BOUND,
        "the registry and the scan's surface-bound count must agree",
    );
    let mut keys: Vec<String> = Vec::new();
    for entry in SURFACE_GEOMETRY_CONSTANTS {
        assert!(
            in_visual_scope(entry.file),
            "{} is not a widget/surface file",
            entry.file
        );
        assert!(
            is_visual_name(entry.name),
            "{} does not read as a visual measurement, so it needed no carve-out",
            entry.name
        );
        assert!(
            entry.spec.contains('§'),
            "{} must cite the § that authorises it",
            entry.name
        );
        let key = format!("{}::{}", entry.file, entry.name);
        assert!(!keys.contains(&key), "duplicate registry entry {key}");
        keys.push(key);
    }
}

#[test]
fn surface_geometry_lookup_is_exact_on_both_file_and_name() {
    assert!(surface_geometry("surface/movable.rs", "CARD_DEFAULT_WIDTH").is_some());
    assert!(surface_geometry("surface/movable.rs", "CARD_DEFAULT_HEIGHT").is_some());
    assert!(surface_geometry("surface/input_dialog.rs", "CARD_WIDTH").is_some());
    assert!(surface_geometry("surface/movable.rs", "CARD_WIDTH").is_none());
    assert!(surface_geometry("surface/input_dialog.rs", "CARD_DEFAULT_WIDTH").is_none());
    assert!(surface_geometry("surface/movable.rs", "NOT_A_BOUND").is_none());
}

// ---------------------------------------------------------------------------
// Helpers and reporting
// ---------------------------------------------------------------------------

#[test]
fn test_files_are_out_of_scope_for_both_rules() {
    assert!(is_test_file("widget/button_tests.rs"));
    assert!(is_test_file("test_helpers.rs"));
    assert!(is_test_file("ui/test_helpers.rs"));
    assert!(is_test_file("tests/integration.rs"));
    assert!(!is_test_file("widget/button.rs"));
    assert!(!is_test_file("token/color.rs"));

    // A raw colour in a test fixture must not fail the build — a contrast
    // solver cannot be tested without one — and must not be counted.
    let scan = scan_source(
        "token/contrast_tests.rs",
        "let c = Color::from_rgb(255, 0, 0);\n",
    );
    assert!(scan.findings.is_empty());
    assert_eq!(scan.token_layer_colour_definitions, 0);
}

#[test]
fn visual_name_matching_is_case_insensitive_substring() {
    for marker in VISUAL_NAME_MARKERS {
        assert!(
            is_visual_name(&format!("SOMETHING_{marker}_ELSE")),
            "{marker} must mark a visual name"
        );
        assert!(
            is_visual_name(&marker.to_lowercase()),
            "matching must be case-insensitive"
        );
    }
    assert!(!is_visual_name("LABEL"));
    assert!(!is_visual_name("MAX_ITEMS"));
}

#[test]
fn scalar_classes_report_their_own_compliance() {
    assert!(ScalarClass::TokenBound.is_compliant());
    assert!(ScalarClass::Zero.is_compliant());
    assert!(ScalarClass::Ratio.is_compliant());
    assert!(ScalarClass::SurfaceBound.is_compliant());
    assert!(!ScalarClass::Violation.is_compliant());

    assert_eq!(Rule::ColourLiteral.label(), "colour-literal");
    assert_eq!(Rule::UnboundVisualScalar.label(), "unbound-scalar");
    assert_eq!(ScalarClass::SurfaceBound.label(), "surface-bound");
}

#[test]
fn the_rendered_report_names_the_counts_and_the_findings() {
    let audit = audit_crate();
    let text = render(&audit);
    assert!(text.contains("UIF-PRIN-001"));
    assert!(text.contains(&format!("files scanned: {}", PIN_FILES_SCANNED)));
    assert!(text.contains(&format!(
        "colour definitions in token/color.rs: {PIN_COLOUR_DEFINITIONS}"
    )));
    assert!(text.contains("violations: 0"));
    assert!(text.contains(&format!("suppressed: {}", PIN_ALLOW_ENTRIES)));
    assert!(text.contains("BUTTON_MIN_WIDTH"));

    // And when it is not clean, the report says so and names the file.
    let dirty = Audit {
        violations: vec![Violation {
            rule: Rule::ColourLiteral,
            file: "widget/x.rs".to_string(),
            line: 7,
            code: "let c = Color::from_rgb(1, 2, 3);".to_string(),
            detail: "Color::from_rgb".to_string(),
        }],
        ..Audit::default()
    };
    let dirty_text = render(&dirty);
    assert!(dirty_text.contains("violations: 1"));
    assert!(dirty_text.contains("widget/x.rs:7"));
    assert!(!dirty.is_clean());
    assert!(dirty.describe().contains("colour-literal"));
}

#[test]
fn redaction_blanks_noise_without_moving_the_lines() {
    let src = concat!(
        "let a = 1; // Color::from_rgb\n",
        "let b = 2; /* outer /* nested */ still comment */ Color::from_rgb(1,2,3);\n",
        "let c = \"a \\\" escaped string\"; let d = 4;\n",
        "let e = r#\"raw \"# and let f = 6;\n",
        "let g = 'x'; let h = '&'; let life = &1;\n",
    );
    let red = redact_comments_and_strings(src);
    let lines: Vec<&str> = red.lines().collect();

    // Line-for-line alignment is what makes reported line numbers true.
    assert_eq!(lines.len(), src.lines().count());
    for (a, b) in src.lines().zip(lines.iter()) {
        assert_eq!(
            a.chars().count(),
            b.chars().count(),
            "columns must not shift"
        );
    }

    // Line 1 is nothing but a comment: the constructor in it is gone.
    assert!(!lines[0].contains("Color::from_rgb"), "{}", lines[0]);
    // Line 2's comment is gone, but the *live* call after it is not — the
    // nested block closed exactly where Rust would have closed it.
    assert!(!lines[1].contains("nested"), "{}", lines[1]);
    assert!(!lines[1].contains("still comment"), "{}", lines[1]);
    assert!(
        lines[1].contains("Color::from_rgb(1,2,3);"),
        "live code after a nested comment must survive: {}",
        lines[1]
    );
    // Strings, raw strings and char literals are blanked; the code around
    // them is not, and a lifetime quote never eats the line.
    assert!(!lines[2].contains("escaped string"), "{}", lines[2]);
    assert!(lines[2].contains("let d = 4;"), "{}", lines[2]);
    assert!(!lines[3].contains("raw "), "{}", lines[3]);
    assert!(lines[3].contains("let f = 6;"), "{}", lines[3]);
    assert!(!lines[4].contains("'x'"), "{}", lines[4]);
    assert!(lines[4].contains("let life = &1;"), "{}", lines[4]);

    // Rule A sees exactly the one live constructor, on the right line.
    let scan = scan_source("widget/redcheck.rs", src);
    assert_eq!(scan.findings.len(), 1, "{:?}", scan.findings);
    assert_eq!(scan.findings[0].line, 2);
}

#[test]
fn an_empty_file_is_clean_and_an_empty_audit_is_clean() {
    let scan = scan_source("widget/empty.rs", "");
    assert!(scan.findings.is_empty());
    assert_eq!(violations_of(&scan).len(), 0);
    assert!(Audit::default().is_clean());
}

// ---------------------------------------------------------------------------
// The `line_height` unit boundary — a source-level guard
//
// `TypeRung::iced_line_height` exists so no call site ever touches iced's
// ambiguous `From<f32> for LineHeight`, which reads a bare `f32` as a
// multiple of the font size rather than as pixels. That protection only
// holds while it is enforced: one reintroduced
// `.line_height(SOME_RUNG.line_height)` re-arms a defect that makes a
// menu row render no text at all.
//
// So the scan reads the crate's own source, comments and string literals
// redacted, finds every `.line_height(` call, and requires the argument
// to name its unit. Like the pinned counts above, it is written so that a
// scanner which quietly stopped matching goes red too: the call count is
// asserted, not assumed.
// ---------------------------------------------------------------------------

/// Argument shapes a `.line_height(...)` call may legally carry.
///
/// Each names its unit, so neither the reader nor the compiler can
/// mistake it for a multiplier:
///
/// * `iced_line_height()` — the rung's own projection to absolute pixels.
/// * `Pixels(` / `Absolute(` — an explicit absolute construction.
/// * `LEADING_LINE_HEIGHT` — the menu's `LineHeight`-typed constant.
/// * `resolved_line_height` — `form_field::text_part`'s parameter,
///   declared `iced::advanced::text::LineHeight`, so it can only have
///   arrived from one of the shapes above.
const LINE_HEIGHT_SAFE_SHAPES: [&str; 5] = [
    "iced_line_height()",
    "Pixels(",
    "Absolute(",
    "LEADING_LINE_HEIGHT",
    "resolved_line_height",
];

/// Every `.line_height(` call in `code` and its argument text.
///
/// The needle requires the `.` to sit directly on `line_height(`, so
/// `.iced_line_height(` — where `iced_` sits in between — is not
/// mistaken for a call being audited.
fn line_height_callsites(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let needle: Vec<char> = ".line_height(".chars().collect();
    let mut out = Vec::new();
    let mut i = 0;

    while i + needle.len() <= chars.len() {
        if chars[i..i + needle.len()] == needle[..] {
            let start = i + needle.len();
            let mut depth = 1usize;
            let mut j = start;
            while j < chars.len() && depth > 0 {
                match chars[j] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    j += 1;
                }
            }
            out.push(chars[start..j].iter().collect());
            i = j + 1;
        } else {
            i += 1;
        }
    }

    out
}

/// No `.line_height(` call anywhere in the crate hands iced a bare pixel
/// `f32`, and the scan is demonstrably still finding the calls.
#[test]
fn no_line_height_call_hands_iced_a_bare_f32() {
    let root = crate_root();
    let mut offenders: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for path in source_files(&root) {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let code = redact_comments_and_strings(&raw);
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path.as_path())
            .display()
            .to_string();

        for arg in line_height_callsites(&code) {
            checked += 1;
            if !LINE_HEIGHT_SAFE_SHAPES
                .iter()
                .any(|shape| arg.contains(shape))
            {
                offenders.push(format!("{rel}: .line_height({arg})"));
            }
        }
    }

    assert!(
        checked >= 8,
        "the scan found only {checked} `.line_height(` calls; the widget layer has \
         eight, so the scanner has gone quiet and cannot be trusted"
    );
    assert!(
        offenders.is_empty(),
        "these calls hand iced a value whose unit it cannot know, which iced reads as \
         a multiple of the font size: {offenders:?}"
    );
}

/// The scanner fires on the exact shape it exists to catch, so a green run
/// above means "none found" rather than "none could be found".
#[test]
fn the_line_height_scan_fires_on_a_doctored_bare_f32() {
    let doctored = "\
fn build() {
    text(\"x\")
        .size(typography::UI_COMPACT.size)
        .line_height(typography::UI_COMPACT.line_height)
        .into();
}
";
    let code = redact_comments_and_strings(doctored);
    let calls = line_height_callsites(&code);
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        !LINE_HEIGHT_SAFE_SHAPES
            .iter()
            .any(|shape| calls[0].contains(shape)),
        "the doctored bare f32 must not read as safe: {:?}",
        calls[0]
    );
}

/// `.iced_line_height()` is not itself flagged — the needle sits on the
/// dot, and `iced_` sits between it and `line_height(`.
#[test]
fn the_line_height_scan_does_not_flag_the_helper_itself() {
    let good = "\
fn build() {
    text(\"x\")
        .line_height(typography::UI_COMPACT.iced_line_height())
        .into();
}
";
    let code = redact_comments_and_strings(good);
    let calls = line_height_callsites(&code);
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        LINE_HEIGHT_SAFE_SHAPES
            .iter()
            .any(|shape| calls[0].contains(shape)),
        "the helper form must read as safe: {:?}",
        calls[0]
    );
}

/// A bare pixel constant used as a line height is caught, which is the
/// shape `LEADING_SLOT` used to be passed in.
#[test]
fn the_line_height_scan_fires_on_a_bare_pixel_constant() {
    let doctored = "text(g).size(LEADING_SLOT).line_height(LEADING_SLOT).into();";
    let code = redact_comments_and_strings(doctored);
    let calls = line_height_callsites(&code);
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        !LINE_HEIGHT_SAFE_SHAPES
            .iter()
            .any(|shape| calls[0].contains(shape)),
        "a bare LEADING_SLOT must not read as safe: {:?}",
        calls[0]
    );
}

// ---------------------------------------------------------------------------
// Wrapped declarations — the INT-14 blind spot
//
// A `pub const NAME: f32 =` whose RHS `rustfmt` wraps onto the next line
// used to leave `expr` empty. `numeric_literals("")` found nothing, and
// `classify_scalar` returned `TokenBound` — a *pass* — for a constant whose
// value had never been read. The parser's own header says a parser that
// quietly gives up is more dangerous than one that does not look; giving
// up and reporting approval is the worst of both.
//
// These tests hold both halves of the fix: a wrapped expression is now
// actually read, and an expression that still cannot be read is reported
// rather than approved.
// ---------------------------------------------------------------------------

/// The bug, reproduced in the shape it shipped. Same wrapping as the
/// compliant case below, and it must not share that case's verdict.
#[test]
fn a_wrapped_literal_is_not_reported_token_bound() {
    let src = concat!("pub const ROW_GAP: f32 =\n", "    space::MD + 13.0;\n");
    let scan = scan_source("widget/wrapped.rs", src);

    assert_eq!(
        scan.token_bound, 0,
        "a wrapped literal must NOT be counted token-bound — that was the hole"
    );
    assert_eq!(
        scan.findings.len(),
        1,
        "it must surface as exactly one finding: {:?}",
        scan.findings
    );
    assert_eq!(scan.findings[0].rule, Rule::UnboundVisualScalar);
    assert_eq!(
        scan.findings[0].line, 1,
        "the finding is reported at the declaration's opening line, where the \
         author can act on it"
    );
    assert_eq!(
        scan.findings[0].constant_name(),
        Some("ROW_GAP"),
        "and it names the constant"
    );
    assert!(
        scan.findings[0].detail.contains("13.0"),
        "the finding must name the literal it read, proving the continuation \
         was examined: {}",
        scan.findings[0].detail
    );
}

/// The preferred outcome: a wrapped, token-only expression is read as
/// token-bound because the gate actually saw the tokens — not because it
/// saw nothing.
#[test]
fn a_wrapped_token_only_expression_is_read_rather_than_assumed() {
    let src = concat!("pub const ROW_GAP: f32 =\n", "    space::MD + space::SM;\n");
    let scan = scan_source("widget/wrapped.rs", src);

    assert!(scan.findings.is_empty(), "{:?}", scan.findings);
    assert_eq!(scan.token_bound, 1);
    assert_eq!(
        scan.unreadable, 0,
        "the expression was recovered, so this is not an unreadable declaration"
    );
}

/// **Non-vacuity.** All four fixtures declare the same constant with the
/// same name in the same file; they differ only in how the RHS is wrapped
/// and what it contains. A gate that still could not see would call every
/// one of them `token-bound`.
#[test]
fn wrapping_is_transparent_to_the_value_and_never_to_the_verdict() {
    let fixtures = [
        (
            "wrapped literal",
            "pub const ROW_GAP: f32 =\n    space::MD + 13.0;\n",
        ),
        (
            "inline literal",
            "pub const ROW_GAP: f32 = space::MD + 13.0;\n",
        ),
        (
            "wrapped token",
            "pub const ROW_GAP: f32 =\n    space::MD + space::SM;\n",
        ),
        (
            "inline token",
            "pub const ROW_GAP: f32 = space::MD + space::SM;\n",
        ),
    ];
    let scanned: Vec<(&str, SourceScan)> = fixtures
        .iter()
        .map(|(name, src)| (*name, scan_source("widget/wrapped.rs", src)))
        .collect();

    let wrapped_literal = &scanned[0].1;
    let inline_literal = &scanned[1].1;
    let wrapped_token = &scanned[2].1;
    let inline_token = &scanned[3].1;

    // Wrapping changes nothing about the verdict the *value* earns.
    assert_eq!(
        wrapped_literal.findings.len(),
        inline_literal.findings.len(),
        "literal RHS: wrapped and inline must agree"
    );
    assert_eq!(
        wrapped_literal.token_bound, inline_literal.token_bound,
        "literal RHS: wrapped and inline must agree on token_bound"
    );
    assert_eq!(
        wrapped_token.findings.len(),
        inline_token.findings.len(),
        "token RHS: wrapped and inline must agree"
    );
    assert_eq!(
        wrapped_token.token_bound, inline_token.token_bound,
        "token RHS: wrapped and inline must agree on token_bound"
    );

    // And the two families genuinely disagree, which is what proves the
    // classifier read the continuation rather than the line break.
    assert_eq!(
        inline_literal.findings.len(),
        1,
        "the inline literal violates"
    );
    assert_eq!(inline_token.token_bound, 1, "the inline token binds");
    assert_eq!(
        wrapped_literal.findings.len(),
        1,
        "the wrapped literal violates"
    );
    assert_eq!(wrapped_token.token_bound, 1, "the wrapped token binds");
}

/// A declaration whose terminating `;` can never be reached is reported
/// `unreadable`. It is never counted compliant, because nothing about it
/// was established.
#[test]
fn an_unreachable_expression_is_reported_unreadable_not_approved() {
    // Three ways the `;` can fail to arrive: the next line opens a new
    // item, the next line is blank, or the file simply ends.
    let cases = [
        (
            "next line opens a new item",
            concat!(
                "pub const ROW_GAP: f32 =\n",
                "pub const OTHER_GAP: f32 = space::MD;\n"
            ),
        ),
        (
            "blank line breaks the continuation",
            concat!(
                "pub const ROW_GAP: f32 =\n",
                "\n",
                "pub const OTHER_GAP: f32 = space::MD;\n"
            ),
        ),
        ("file ends mid-declaration", "pub const ROW_GAP: f32 =\n"),
    ];

    for (label, src) in cases {
        let scan = scan_source("widget/broken.rs", src);
        assert_eq!(
            scan.unreadable, 1,
            "{label}: the unreadable declaration must be counted"
        );
        assert!(
            !scan.findings.is_empty(),
            "{label}: unreadable must produce a finding, not silence"
        );
        let v = &scan.findings[0];
        assert_eq!(v.rule, Rule::UnboundVisualScalar, "{label}");
        assert_eq!(
            v.constant_name(),
            Some("ROW_GAP"),
            "{label}: the unreadable constant is the one named"
        );
        assert!(
            v.detail.contains("could not read"),
            "{label}: the detail must say the expression was unreadable rather \
             than claim an unbound measurement: {}",
            v.detail
        );
    }
}

/// The unreadable declaration must never be counted `token_bound`, and the
/// class must report itself non-compliant. This is the assertion whose
/// absence created the hole.
#[test]
fn unreadable_is_never_compliant_and_never_token_bound() {
    assert!(
        !ScalarClass::Unreadable.is_compliant(),
        "an unreadable declaration has not been shown to satisfy the rule"
    );
    assert_eq!(ScalarClass::Unreadable.label(), "unreadable-expr");

    let src = "pub const ROW_GAP: f32 =\n";
    let scan = scan_source("widget/broken.rs", src);
    assert_eq!(
        scan.token_bound, 0,
        "unreadable must not be counted token-bound under any path"
    );
    assert_eq!(scan.unreadable, 1);
}

/// The happy path is unchanged: a single-line literal constant violates
/// exactly as it always did, with the same message shape.
#[test]
fn a_single_line_literal_still_violates_exactly_as_before() {
    let src = "pub const ROW_GAP: f32 = 13.0;\n";
    let scan = scan_source("widget/inline.rs", src);
    assert_eq!(scan.findings.len(), 1, "{:?}", scan.findings);
    let v = &scan.findings[0];
    assert_eq!(v.rule, Rule::UnboundVisualScalar);
    assert_eq!(v.line, 1);
    assert!(
        v.detail.contains("13.0"),
        "the ordinary violation message still names the literal: {}",
        v.detail
    );
    assert!(
        !v.detail.contains("could not read"),
        "a readable declaration must not be described as unreadable: {}",
        v.detail
    );
    assert_eq!(scan.unreadable, 0);
}

/// A multi-line continuation spanning more than one extra line is joined in
/// full, so a long derived expression is read rather than truncated.
#[test]
fn a_continuation_spanning_several_lines_is_joined_in_full() {
    let src = concat!(
        "pub const STACKED_HEIGHT: f32 =\n",
        "    HEADER_HEIGHT\n",
        "    + BAND_GAP\n",
        "    + CONTROL_SLOT_HEIGHT\n",
        "    + BAND_GAP\n",
        "    + CAPTION_SLOT_HEIGHT;\n",
    );
    let scan = scan_source("widget/tall.rs", src);
    assert!(
        scan.findings.is_empty(),
        "a token-only multi-line RHS must be read as token-bound: {:?}",
        scan.findings
    );
    assert_eq!(scan.token_bound, 1);
    assert_eq!(scan.unreadable, 0);

    // Same shape, but the last line carries a literal — the join must have
    // reached it, not stopped early.
    let with_literal = src.replace("+ CAPTION_SLOT_HEIGHT;", "+ CAPTION_SLOT_HEIGHT + 7.0;");
    let after = scan_source("widget/tall.rs", &with_literal);
    assert_eq!(
        after.findings.len(),
        1,
        "the literal on the final continuation line must be seen: {:?}",
        after.findings
    );
    assert!(
        after.findings[0].detail.contains("7.0"),
        "the finding names the literal from the last line: {}",
        after.findings[0].detail
    );
}

// ---------------------------------------------------------------------------
// The second half of the continuation hole — a PARTIAL first-line RHS
//
// INT-14 closed the shape where `=` ends the line and the whole value sits
// below. This group closes the shape where line 1 carries the *opening term*
// and the rest continues underneath:
//
//     pub const X: f32 = space::SM
//         + 500.0;
//
// The old condition was `!first.trim().is_empty()`, so line 1 was returned
// alone as `"space::SM"`. `numeric_literals` found nothing and the constant
// was reported `TokenBound` — a pass granted without the value ever being
// read. `500.0` never reached the classifier.
//
// This shape is worse than INT-14's because it is what a developer writes
// by hand when breaking a long sum across lines, and once written `rustfmt`
// keeps it (each line is inside `max_width`). Two constants in this tree
// were living in it when the hole was found.
// ---------------------------------------------------------------------------

/// **The bug, in the shape it shipped.** A literal on a continuation line
/// below a non-empty first line must now be seen and must fail.
#[test]
fn a_literal_below_a_partial_first_line_is_seen_and_fails() {
    let src = concat!("pub const ROW_GAP: f32 = space::MD\n", "    + 13.0;\n");
    let scan = scan_source("widget/partial.rs", src);

    assert_eq!(
        scan.token_bound, 0,
        "a partial-first-line RHS with a literal must NOT be token-bound — that \
         was the hole: the first term alone looked clean"
    );
    assert_eq!(
        scan.findings.len(),
        1,
        "it must surface as exactly one finding: {:?}",
        scan.findings
    );
    assert_eq!(scan.findings[0].rule, Rule::UnboundVisualScalar);
    assert_eq!(scan.findings[0].constant_name(), Some("ROW_GAP"));
    assert!(
        scan.findings[0].detail.contains("13.0"),
        "the finding must name the literal from the continuation line, proving \
         the join reached past line 1: {}",
        scan.findings[0].detail
    );
    assert_eq!(
        scan.unreadable, 0,
        "the expression WAS recovered — this is a read-and-failed case, not an \
         unreadable one"
    );
}

/// The same shape with a token-only continuation binds, because the whole
/// expression was read — not because line 1 looked clean.
#[test]
fn a_token_only_continuation_below_a_partial_first_line_binds() {
    let src = concat!("pub const ROW_GAP: f32 = space::MD\n", "    + space::SM;\n");
    let scan = scan_source("widget/partial.rs", src);

    assert!(scan.findings.is_empty(), "{:?}", scan.findings);
    assert_eq!(scan.token_bound, 1);
    assert_eq!(scan.unreadable, 0);
}

/// **Non-vacuity.** Six fixtures, same constant name, same file, differing
/// only in where the wrap falls and what the continuation holds. A gate that
/// still stopped at line 1 would call the two partial-with-literal shapes
/// `token-bound`.
#[test]
fn where_the_wrap_falls_never_changes_the_verdict_the_value_earns() {
    let fixtures = [
        (
            "inline literal",
            "pub const ROW_GAP: f32 = space::MD + 13.0;\n",
        ),
        (
            "empty-first literal",
            "pub const ROW_GAP: f32 =\n    space::MD + 13.0;\n",
        ),
        (
            "partial-first literal",
            "pub const ROW_GAP: f32 = space::MD\n    + 13.0;\n",
        ),
        (
            "inline token",
            "pub const ROW_GAP: f32 = space::MD + space::SM;\n",
        ),
        (
            "empty-first token",
            "pub const ROW_GAP: f32 =\n    space::MD + space::SM;\n",
        ),
        (
            "partial-first token",
            "pub const ROW_GAP: f32 = space::MD\n    + space::SM;\n",
        ),
    ];

    let mut literal_shapes = Vec::new();
    let mut token_shapes = Vec::new();
    for (label, src) in fixtures {
        let scan = scan_source("widget/wrap.rs", src);
        if label.contains("literal") {
            literal_shapes.push((label, scan));
        } else {
            token_shapes.push((label, scan));
        }
    }

    for (label, scan) in &literal_shapes {
        assert_eq!(
            scan.findings.len(),
            1,
            "{label}: a literal must fail regardless of where the wrap falls: {:?}",
            scan.findings
        );
        assert_eq!(scan.token_bound, 0, "{label}: a literal never binds");
    }
    for (label, scan) in &token_shapes {
        assert_eq!(
            scan.token_bound, 1,
            "{label}: a token-only RHS binds regardless of the wrap"
        );
        assert!(scan.findings.is_empty(), "{label}: {:?}", scan.findings);
    }
}

/// **INT-14 regression guard.** The empty-first-line shape must still join
/// exactly as it did. The fix changed the guard from emptiness to the
/// terminator; `""` carries no terminator, so it must still fall through
/// to the join loop.
#[test]
fn the_empty_first_line_shape_still_joins_after_the_fix() {
    let literal = concat!("pub const ROW_GAP: f32 =\n", "    space::MD + 13.0;\n");
    let scan = scan_source("widget/int14.rs", literal);
    assert_eq!(
        scan.findings.len(),
        1,
        "INT-14's caught case must still be caught: {:?}",
        scan.findings
    );
    assert!(scan.findings[0].detail.contains("13.0"));
    assert_eq!(scan.token_bound, 0);

    let token = concat!("pub const ROW_GAP: f32 =\n", "    space::MD + space::SM;\n");
    let ok = scan_source("widget/int14.rs", token);
    assert_eq!(ok.token_bound, 1);
    assert!(ok.findings.is_empty(), "{:?}", ok.findings);
}

/// **No behaviour change on the happy path.** A declaration that ends on
/// its opening line is returned untouched, exactly as before.
#[test]
fn a_declaration_ending_on_its_opening_line_is_returned_untouched() {
    let lines: Vec<&str> = vec!["pub const A: f32 = space::MD + 13.0;", "fn later() {}"];
    assert_eq!(
        join_continuation(&lines, 0, " space::MD + 13.0;"),
        Some(" space::MD + 13.0;".to_string()),
        "the raw first-line RHS is returned verbatim when it carries the \
         terminator — no join, no trim, no reordering"
    );
}

/// The decision rule itself, asserted directly: the **terminator** decides,
/// never emptiness.
#[test]
fn the_terminator_decides_the_join_and_emptiness_does_not() {
    let lines: Vec<&str> = vec![
        "pub const A: f32 = space::MD",
        "    + 13.0;",
        "pub const B: f32 = space::MD;",
        "pub const C: f32 =",
        "    space::SM;",
    ];

    // Partial first line → joined through to the terminator.
    assert_eq!(
        join_continuation(&lines, 0, " space::MD"),
        Some("space::MD + 13.0;".to_string()),
        "a non-empty first line WITHOUT a terminator must continue joining — \
         this is the case the old `!is_empty()` guard returned early on"
    );
    // Complete first line → returned as-is.
    assert_eq!(
        join_continuation(&lines, 2, " space::MD;"),
        Some(" space::MD;".to_string())
    );
    // Empty first line (INT-14) → joins.
    assert_eq!(
        join_continuation(&lines, 3, ""),
        Some("space::SM;".to_string())
    );
}

/// The `opens_new_item` / blank-line guards still bite on the partial shape,
/// so a malformed declaration cannot swallow the code below it.
#[test]
fn a_partial_first_line_that_cannot_reach_a_terminator_is_unreadable() {
    let cases = [
        (
            "continuation swallowed by a new item",
            concat!(
                "pub const ROW_GAP: f32 = space::MD\n",
                "pub const OTHER: f32 = space::SM;\n"
            ),
        ),
        (
            "blank line before the terminator",
            concat!(
                "pub const ROW_GAP: f32 = space::MD\n",
                "\n",
                "    + 13.0;\n"
            ),
        ),
        (
            "file ends mid-expression",
            "pub const ROW_GAP: f32 = space::MD\n",
        ),
    ];

    for (label, src) in cases {
        let scan = scan_source("widget/broken_partial.rs", src);
        assert_eq!(
            scan.unreadable, 1,
            "{label}: must be reported unreadable, not silently token-bound"
        );
        assert_eq!(
            scan.token_bound, 0,
            "{label}: an expression that was never fully read cannot bind"
        );
        assert!(
            scan.findings[0].detail.contains("could not read"),
            "{label}: reported as unreadable, not as an unbound measurement: {}",
            scan.findings[0].detail
        );
    }
}

/// A coefficient on a continuation line is a literal and must be caught,
/// not mistaken for part of a token path. This is the exact shape that made
/// the hole live in real code.
#[test]
fn a_multiplier_on_a_continuation_line_is_caught_not_glossed() {
    let src = concat!(
        "pub const CARD_HEIGHT: f32 = TITLE_RUNG.line_height\n",
        "    + CAPTION_RUNG.line_height * 2.0\n",
        "    + CARD_PADDING * 2.0;\n",
    );
    let scan = scan_source("widget/mult.rs", src);
    assert_eq!(
        scan.token_bound, 0,
        "`* 2.0` is a literal on a continuation line; the join must reach it"
    );
    assert_eq!(scan.findings.len(), 1, "{:?}", scan.findings);
    assert!(
        scan.findings[0].detail.contains("2.0"),
        "the finding names the coefficient: {}",
        scan.findings[0].detail
    );

    // The same sum written with the doubled terms spelled out binds cleanly,
    // which is how the one exposed constant in the tree was fixed.
    let fixed = concat!(
        "pub const CARD_HEIGHT: f32 = TITLE_RUNG.line_height\n",
        "    + CAPTION_RUNG.line_height\n",
        "    + CAPTION_RUNG.line_height\n",
        "    + CARD_PADDING\n",
        "    + CARD_PADDING;\n",
    );
    let ok = scan_source("widget/mult.rs", fixed);
    assert_eq!(
        ok.token_bound, 1,
        "literal-free composition binds: {:?}",
        ok.findings
    );
    assert!(ok.findings.is_empty(), "{:?}", ok.findings);
}
