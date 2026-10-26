//! A reusable iced widget library implementing the FastMD UI framework
//! specification.
//!
//! This crate realises the design specification in
//! `spec.md` at this crate's root. Requirement IDs of the form `UIF-*` in that
//! document map to the code in this crate; each module cites the IDs it
//! implements in its doc comments.
//!
//! The core rule is Perceptual Harmony Through Tokenization (`UIF-PRIN-001`):
//! every visual property — colour, elevation, spacing, sizing, border radius,
//! stroke weight and typography — is selected from a closed set of semantic
//! design tokens. Raw numeric literals in individual widget definitions are
//! forbidden; widgets resolve their style through the token modules only.
//!
//! The crate is standalone: the `fastmd` application does not consume it yet.
//! Wiring the app onto this framework is a separate, later project (decision
//! D-3 in `doc/planning/iced-pro-tasks.md`).

pub mod a11y;
pub mod binding;
pub mod bridge;
pub mod composite;
pub mod focus;
pub mod gallery;
pub mod gallery_audit;
pub mod gallery_badge;
pub mod gallery_breadcrumb;
pub mod gallery_button;
pub mod gallery_chip;
pub mod gallery_combo;
pub mod gallery_composite;
pub mod gallery_editor;
pub mod gallery_hyperlink;
pub mod gallery_input;
pub mod gallery_menu;
pub mod gallery_modal;
pub mod gallery_movable;
pub mod gallery_popover;
pub mod gallery_progress;
pub mod gallery_range;
pub mod gallery_secret;
pub mod gallery_segmented;
pub mod gallery_split_button;
pub mod gallery_suggest;
pub mod gallery_table;
pub mod gallery_toggle;
pub mod gallery_tooltip;
pub mod gallery_tree;
pub mod gallery_warm;
pub mod geometry;
pub mod invariant_gate;
pub mod layout;
pub mod microcopy;
pub mod spec_coverage;
pub mod state;
pub mod surface;
pub mod token;
pub mod token_lint;
pub mod validation;
pub mod widget;

pub use composite::{
    CodeBlock, CodeBlockModel, CommandPalette, CommandPaletteModel, DiffDisplayMode, DiffHunk,
    DiffLine, DiffLineKind, DiffViewer, DiffViewerModel, EmptyState, EmptyStateKind,
    EmptyStateModel, EmptyStateStyle, EscapeDisposition, GridNavKey, GridTile, GridView,
    GridViewModel, Icon, KeyChord, KeybindRecorder, KeybindRecorderModel, NavDestination,
    NavigationRail, NavigationRailModel, PaginationBar, PaginationModel, PaletteGroup, PaletteItem,
    PaletteMode, PipsPager, PipsPagerModel, SheetMode, SheetPlacement, SideSheet, SideSheetModel,
    SideSheetResize, SideSheetState, Skeleton, SkeletonStyle, StepItem, StepStatus, Stepper,
    StepperModel, StepperOrientation, WizardFooterConfig, code_block, command_palette, diff_viewer,
    empty_state, grid_view, keybind_recorder, navigation_rail, pagination_bar, pips_pager,
    side_sheet, skeleton, stepper, wizard_footer,
};
pub use widget::{
    AmPm, Avatar, AvatarModel, AvatarShape, AvatarSize, CalendarView, CalendarViewMode, CivilDate,
    CivilTime, ColorPicker, ColorPickerModel, ColorSwatchButton, DatePicker, DatePickerModel,
    DateRangePicker, DateSelection, Hsva, RatingControl, RatingModel, RatingStep, Rgba8,
    TimePicker, TimePickerModel, TimeSegment, Weekday, YearMonth, avatar, calendar_view,
    color_picker, color_swatch_button, date_picker, date_range_picker, rating_control, time_picker,
};

#[cfg(test)]
pub mod test_helpers;
