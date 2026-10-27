//! The composite widgets of the framework (`UIF-COMP-*`).
//!
//! A composite is not an atom with more parts. An atom
//! ([`crate::widget`]) owns an interaction state machine for one control;
//! a composite owns the *relationship between* controls and the text that
//! explains them — the vertical rhythm of a labelled field, the horizontal
//! split of a settings row, the three slots of a list tile. §3B of
//! `spec.md` at this crate's root specifies those relationships, and this
//! subsystem is where they become code.
//!
//! The same split that runs through [`crate::widget`] runs through here, and
//! for the same reason. The **resolver** is a pure function from a small
//! declarative input to rectangles and resolved colours; the **widget** is
//! the iced `Element` that lays out to those rectangles and paints them.
//! Keeping the geometry pure is what makes the requirements checkable at
//! all, because the two things §3B actually demands of a composite are
//! facts about geometry rather than facts about pixels:
//!
//! * **A band that is reserved is a band that is measured.** `UIF-PRIN-003`
//!   forbids a state change from moving content. In a form field that means
//!   the caption slot must exist at its full height whether or not anything
//!   is written in it, and the only way to *prove* that rather than observe
//!   it is a geometry function that cannot see the caption's content. Every
//!   reserved band in this subsystem is computed by such a function — the
//!   absence of the parameter is the proof.
//! * **A slot the spec fixes is a slot the host does not get to choose.**
//!   Decision **D-8** requires the `ListTile`'s slot geometry to be
//!   computed here rather than left to whatever lays the row out, because a
//!   host that invents its own leading-box width invents a different list on
//!   every surface.
//!
//! Like [`crate::token`] and [`crate::widget`], this module does not
//! glob-re-export its children; call sites stay qualified.
//!
//! * [`form_field`] — the stacked label / control / reserved-caption field
//!   whose total rhythm is fixed so a validation transition moves nothing
//!   (`UIF-COMP-001`, `UIF-PRIN-003`)
//! * [`settings_row`] — the horizontal split: a filling text column and a
//!   shrinking control column on one raised card row (`UIF-COMP-001`)
//! * [`breadcrumb`] - the trail's elision solver: a pure function of
//!   `(items, available_width)` that keeps the root and the current-location
//!   crumb at every width, drops middle crumbs from the root end so the
//!   hidden set can only shrink monotonically as the bar widens, and leaves
//!   the separator in the gap between targets rather than as a target that
//!   ignores clicks (`UIF-COMP-008`)
//! * [`card`] — the grouping container: `surface.raised`, `radius.card`,
//!   `space.lg` padding, and the resting-vs-elevated border (`UIF-COMP-005`)
//! * [`list_tile`] — the three-slot row: leading mark, filling text,
//!   shrinking trailing targets, with the hit target resolved separately
//!   for the interactive and the inert tile (`UIF-COMP-005`)
//! * [`virtual_list`] - the recycling primitive: fixed-height row metrics that
//!   bound realized rows independently of the total row count, so a
//!   1 000 000-row list realises exactly what a 100 000-row one does, and
//!   never a prefix sum (`UIF-COMP-006`)
//! * [`tree_row`] - the framework-owned tree row: the depth-indent policy, the
//!   row slot layout, and the D-6 contrast adaptation behind every mark a row
//!   paints, including the ones `fastmd` never drew (decision **D-8**)
//! * [`tree_view`] - the tree model: visible-row flattening, single and multi
//!   selection keyed by node id, the keyboard contract, and the
//!   scroll-into-view handoff to [`virtual_list`]
//! * [`table_column`] - the column model and the pure width resolver: every
//!   column declares a floor, a target and a ceiling, and the layout is a
//!   function of the specs plus the available width alone. The
//!   over-subscribed-minimums case is reported as `needs_horizontal_scroll`
//!   rather than silently violated (`UIF-COMP-007`)
//! * [`table_cell`] - framework-owned cell rendering (decision **D-8**): the
//!   band a row sits on, the cell and content boxes inside a resolved column,
//!   and the grid-line colour promoted where `border.subtle` would fall below
//!   the legibility floor once it sits on a tinted band
//! * [`table_header`] - the header cell: a sort target and a resize handle
//!   that are disjoint by construction, so a small accidental drag never
//!   sorts and a click on the title never starts a resize
//! * [`table_view`] - the grid model: row selection on the same anchor
//!   semantics [`tree_view`] defines, cell-level keyboard traversal, a stable
//!   sort that cannot change which rows are selected, the resize gesture, and
//!   the sticky header kept out of the row-recycling cycle
//! * [`overflow_toolbar`] - the priority-declared overflow strip: each item
//!   declares its own overflow rank rather than learning it from its position,
//!   eviction follows that declaration, and the strip's height function
//!   cannot see how many items went inline versus into the menu, which is what
//!   makes the `0px` vertical-shift requirement structural instead of
//!   observed (`UIF-COMP-008`, `UIF-PRIN-003`)
//! * [`mod@empty_state`] — the non-collapsing placeholder distinguishing empty,
//!   zero-match and load-error states with a 40x40px badge and recovery CTA (`UIF-COMP-014`, `UIF-PRIN-004`)
//! * [`mod@skeleton`] — the loading placeholder reserving exact target widget dimensions
//!   with a 1200ms shimmer pulse or static subtle border under reduced motion (`UIF-COMP-014`, `UIF-PRIN-003`)
//! * [`mod@pagination`] — the 32px footer bar with tabular mono range summary, page size
//!   options, and boundary-clamped navigation buttons (`UIF-COMP-014`)
//! * [`mod@stepper`] — the multi-step workflow header and vertical wizard sequence with
//!   redundant glyph/colour status cues and non-shifting footer (`UIF-COMP-011`, `UIF-PRIN-005`, `UIF-PRIN-004`)
//! * [`mod@pips_pager`] — the compact 24px carousel and pagination indicator strip with
//!   boundary-clamped navigation buttons (`UIF-COMP-011`, `UIF-PRIN-004`)
//! * [`mod@side_sheet`] — the contextual inspector drawer with Docked, ModelessOverlay and ModalScrim modes,
//!   10px resize divider, and 36px header (`UIF-COMP-012`, `UIF-PRIN-006`, `UIF-MOD-002`)
//! * [`mod@navigation_rail`] — the collapsible navigation rail composite supporting compact (48px)
//!   and resizable expanded (180–320px) modes with roving keyboard focus (`UIF-COMP-009`, `UIF-PRIN-005`, `UIF-PRIN-006`)
//! * [`mod@command_palette`] — the quick-open and command palette modal card with prefix modes,
//!   two-stage Escape, and Rule 4 text.strong subtitle promotion (`UIF-COMP-010`, `UIF-PRIN-003`, `UIF-TOK-006`)
//! * [`mod@grid_view`] — the 2D virtualized tile and card grid with deterministic column solving
//!   and 2D boundary clamping (`UIF-COMP-013`, `UIF-VER-002`, `UIF-PRIN-006`)
//! * [`mod@code_block`] — the fenced code block container with 28px header, language badge,
//!   fixed 24x24px copy button with 1500ms success hold, 13px mono body and 40px line-number gutter (`UIF-COMP-015`, `UIF-PRIN-003`)
//! * [`mod@diff_viewer`] — the visual diff viewer pairing + and - glyphs with 2px indicator bars
//!   and distinct text roles in unified and side-by-side modes with per-hunk actions (`UIF-COMP-015`, `UIF-PRIN-005`, `UIF-PRIN-006`)
//! * [`mod@keybind_recorder`] — the 32px shortcut capture input with mono keycap pills,
//!   trailing clear button, and raw chord interception (`UIF-COMP-015`)
//!
//! Every number comes from [`crate::token`] (`UIF-PRIN-001`). The three
//! figures §3B prints that no rung carries — the `20px` field header, the
//! `48px` settings-row floor and the `44px` two-line tile — are registered
//! Surface Geometry Constants in [`crate::token_lint`] rather than passed
//! off as token reads.

pub mod breadcrumb;
pub mod card;
pub mod code_block;
pub mod command_palette;
pub mod diff_viewer;
pub mod empty_state;
pub mod expander;
pub mod form_field;
pub mod grid_view;
pub mod info_bar;
pub mod keybind_recorder;
pub mod list_tile;
pub mod navbar;
pub mod navigation_rail;
pub mod notification;
pub mod overflow_toolbar;
pub mod pagination;
pub mod pips_pager;
pub mod scroll_view;
pub mod settings_row;
pub mod side_sheet;
pub mod skeleton;
pub mod split_divider;
pub mod stepper;
pub mod tab_strip;
pub mod table_cell;
pub mod table_column;
pub mod table_header;
pub mod table_view;
pub mod toast;
pub mod tree_row;
pub mod tree_view;
pub mod virtual_list;

pub use code_block::{CodeBlock, CodeBlockModel, code_block};
pub use command_palette::{
    CARD_HEIGHT, CARD_WIDTH, CommandPalette, CommandPaletteModel, CommandPaletteState,
    DIVIDER_STROKE, EscapeDisposition, FOOTER_HEIGHT, GLYPH_SEARCH, GROUP_HEADER_HEIGHT, LABEL_ESC,
    LEGEND_KEYBOARD, NO_RESULTS, PLACEHOLDER_COMMANDS, PLACEHOLDER_FILES, PLACEHOLDER_SYMBOLS,
    PLACEHOLDER_TAGS, PaletteGroup, PaletteItem, PaletteMode, RADIUS, ROW_DOUBLE_HEIGHT,
    ROW_SINGLE_HEIGHT, SEARCH_HEIGHT, SEARCHING, TOP_OFFSET, card_bounds, command_palette,
    highlight_spans,
};
pub use diff_viewer::{
    DiffDisplayMode, DiffHunk, DiffLine, DiffLineKind, DiffViewer, DiffViewerModel, diff_viewer,
};
pub use empty_state::{
    EmptyState, EmptyStateKind, EmptyStateModel, EmptyStateStyle, Icon, empty_state,
};
pub use grid_view::{
    BORDER_REST_WIDTH, BORDER_SELECTED_WIDTH, CHECKBOX_INSET, CHECKBOX_SIZE,
    DEFAULT_MIN_TILE_WIDTH, DEFAULT_TILE_HEIGHT, GAP, GLYPH_CHECK, GUTTER_WIDTH, GridNavKey,
    GridTile, GridView, GridViewModel, GridViewState, grid_view, solve_columns, tile_rect,
};
pub use keybind_recorder::{KeyChord, KeybindRecorder, KeybindRecorderModel, keybind_recorder};
pub use navbar::{
    NAVBAR_HEIGHT_COMPACT, NAVBAR_HEIGHT_STANDARD, NavBar, NavBarMenuTrigger, NavBarModel,
    TRIGGER_HEIGHT_COMPACT, TRIGGER_HEIGHT_STANDARD, TRIGGER_PADDING_HORIZONTAL, TriggerStyle,
    navbar_trigger_element, resolve_trigger_style,
};
pub use navigation_rail::{
    COMPACT_WIDTH, EXPANDED_DEFAULT_WIDTH, EXPANDED_MAX_WIDTH, EXPANDED_MIN_WIDTH, GLYPH_TOGGLE,
    ICON_SIZE, INDICATOR_WIDTH, LABEL_TOGGLE, NavDestination, NavRailResize, NavigationRail,
    NavigationRailModel, NavigationRailState, ROW_HEIGHT, TOGGLE_SIZE, navigation_rail,
};
pub use pagination::{PaginationBar, PaginationModel, pagination_bar};
pub use pips_pager::{PipsPager, PipsPagerModel, pips_pager};
pub use side_sheet::{
    SheetMode, SheetPlacement, SideSheet, SideSheetModel, SideSheetResize, SideSheetState,
    side_sheet,
};
pub use skeleton::{Skeleton, SkeletonStyle, skeleton};
pub use split_divider::{
    SplitAxis, SplitDivider, SplitDividerModel, SplitDividerResize, SplitDividerState,
    SplitSnapshot, compute_grab_rect, split_divider, split_divider_geometry, split_divider_layers,
};
pub use stepper::{
    StepItem, StepStatus, Stepper, StepperModel, StepperOrientation, WizardFooterConfig, stepper,
    wizard_footer,
};
pub use tab_strip::{
    CLOSE_BUTTON_SIZE, DIRTY_DOT_DIAMETER, TAB_MAX_WIDTH, TAB_MIN_WIDTH, TAB_STRIP_HEIGHT,
    TabCloseVerdict, TabItem, TabItemStyle, TabStrip, TabStripModel, resolve_tab_style,
};
