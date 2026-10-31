//! User-facing text for the hoarfrost widget library: control labels and
//! validation messages a host renders verbatim, collected so a host or a
//! translation pass can see every string the library can put on screen.
//!
//! Scope is deliberately narrow — text a human reads. Glyphs used as layout
//! markers, token names and test fixtures are not user-facing and stay with the
//! code that uses them.
//!
//! Each constant is re-exported from the module that originally defined it, so
//! `widget::color_picker::LABEL_APPLY` and `strings::LABEL_APPLY` are the same
//! path; new text is defined here and imported where it is used.
//!
//! Unit tests live in the sibling `strings_tests.rs` sidecar.

/// Label for the colour picker's confirm control (§3.12, `UIF-ATOM-012`).
pub const LABEL_APPLY: &str = "✓ Apply";

/// Label for the colour picker's dismiss control (§3.12, `UIF-ATOM-012`).
pub const LABEL_CANCEL: &str = "Cancel";

/// Label for the colour picker's hex input (§3.12, `UIF-ATOM-012`).
pub const LABEL_HEX: &str = "Hex";

/// Label for the colour picker's preset swatch group (§3.12, `UIF-ATOM-012`).
pub const LABEL_PRESETS: &str = "Presets";

/// Validation message shown when the hex input cannot be parsed (§3.12,
/// `UIF-ATOM-012`).
pub const ERR_INVALID_HEX: &str = "Invalid hex color";

/// Label for the diff viewer's accept-hunk control (§3B.15).
pub const LABEL_ACCEPT_HUNK: &str = "Accept hunk";

/// Label for the diff viewer's reject-hunk control (§3B.15).
pub const LABEL_REJECT_HUNK: &str = "Reject hunk";

/// Label for the diff viewer's inline view mode (§3B.15).
pub const LABEL_UNIFIED: &str = "Unified";

/// Label for the diff viewer's two-pane view mode (§3B.15).
pub const LABEL_SIDE_BY_SIDE: &str = "Side-by-Side";

// ---------------------------------------------------------------------------
// Command palette (§3B.10)
// ---------------------------------------------------------------------------

/// Label on the trailing Escape hint chip (§3B.10).
pub const LABEL_ESC: &str = "Esc";

/// Placeholder in the palette's Files mode (§3B.10).
pub const PLACEHOLDER_FILES: &str = "Search files by name...";

/// Placeholder in the palette's Commands mode (§3B.10).
pub const PLACEHOLDER_COMMANDS: &str = "Type a command...";

/// Placeholder in the palette's Symbols mode (§3B.10).
pub const PLACEHOLDER_SYMBOLS: &str = "Search symbols...";

/// Placeholder in the palette's Tags mode (§3B.10).
pub const PLACEHOLDER_TAGS: &str = "Search by tag...";

// ---------------------------------------------------------------------------
// Keybind recorder (§3B.15)
// ---------------------------------------------------------------------------

/// Prompt shown while the recorder listens for a chord (§3B.15).
pub const LABEL_RECORDING: &str = "Press shortcut keys...";

/// Prompt shown when no chord is recorded and the recorder is idle (§3B.15).
pub const LABEL_EMPTY: &str = "Record shortcut";

// ---------------------------------------------------------------------------
// Navigation rail (§3B.9)
// ---------------------------------------------------------------------------

/// Accessibility label for the rail's expand/collapse toggle (§3B.9).
pub const LABEL_TOGGLE: &str = "Toggle navigation rail";

// ---------------------------------------------------------------------------
// Side sheet (§3B.12)
// ---------------------------------------------------------------------------

/// Label for the side sheet's pin control (§3B.12).
pub const LABEL_PIN: &str = "Pin";

/// Label for the side sheet's unpin control (§3B.12).
pub const LABEL_UNPIN: &str = "Unpin";

/// Label for the side sheet's close control (§3B.12).
pub const LABEL_CLOSE: &str = "Close";

// ---------------------------------------------------------------------------
// Stepper (§3B.11, `UIF-PRIN-004`)
// ---------------------------------------------------------------------------

/// Previous-step button label, arrow included (`UIF-PRIN-004`).
pub const LABEL_PREVIOUS: &str = "← Previous";

/// Next-step button label, arrow included.
pub const LABEL_NEXT: &str = "Next →";

/// Final-step button label, check mark included.
pub const LABEL_FINISH: &str = "Finish ✓";

// ---------------------------------------------------------------------------
// Date picker (§3.10)
// ---------------------------------------------------------------------------

/// Placeholder shown in an empty date field (§3.10).
pub const PLACEHOLDER_DATE: &str = "YYYY-MM-DD";

// ---------------------------------------------------------------------------
// Time picker (§3.11)
//
// `LABEL_APPLY` and `LABEL_CANCEL` are shared with the colour picker: the same
// action buttons, the same text, one definition.
// ---------------------------------------------------------------------------

/// Column header for the hours column (§3.11).
pub const LABEL_HR: &str = "Hr";

/// Column header for the minutes column (§3.11).
pub const LABEL_MIN: &str = "Min";

/// Column header for the seconds column (§3.11).
pub const LABEL_SEC: &str = "Sec";

/// Column header for the meridiem column (§3.11).
pub const LABEL_AM_PM: &str = "AM/PM";

/// Meridiem string for the morning half of the day (§3.11).
pub const LABEL_AM: &str = "AM";

/// Meridiem string for the afternoon half of the day (§3.11).
pub const LABEL_PM: &str = "PM";

/// Placeholder for a 24-hour time without seconds (§3.11).
pub const PLACEHOLDER_24H: &str = "--:--";

/// Placeholder for a 24-hour time with seconds (§3.11).
pub const PLACEHOLDER_24H_SEC: &str = "--:--:--";

/// Placeholder for a 12-hour time without seconds (§3.11).
pub const PLACEHOLDER_12H: &str = "--:-- --";

/// Placeholder for a 12-hour time with seconds (§3.11).
pub const PLACEHOLDER_12H_SEC: &str = "--:--:-- --";

#[cfg(test)]
#[path = "strings_tests.rs"]
mod strings_tests;
