//! Unit tests for the hoarfrost user-facing text catalogue
//! ([`crate::strings`]).
//!
//! These pin the two contracts the catalogue introduces: the text itself, so a
//! change to shipped copy is a deliberate edit, and the re-export paths, so a
//! host that already references `widget::color_picker::LABEL_APPLY` keeps
//! compiling and keeps reading the same value.

use super::*;
use crate::composite::{
    command_palette, diff_viewer, keybind_recorder, navigation_rail, side_sheet, stepper,
};
use crate::widget::{color_picker, date_picker, time_picker};

#[test]
fn colour_picker_labels_keep_their_original_paths_and_values() {
    assert_eq!(
        color_picker::LABEL_APPLY,
        LABEL_APPLY,
        "widget::color_picker::LABEL_APPLY must read the catalogue value"
    );
    assert_eq!(
        color_picker::LABEL_CANCEL,
        LABEL_CANCEL,
        "widget::color_picker::LABEL_CANCEL must read the catalogue value"
    );
    assert_eq!(
        color_picker::LABEL_HEX,
        LABEL_HEX,
        "widget::color_picker::LABEL_HEX must read the catalogue value"
    );
    assert_eq!(
        color_picker::LABEL_PRESETS,
        LABEL_PRESETS,
        "widget::color_picker::LABEL_PRESETS must read the catalogue value"
    );
    assert_eq!(
        color_picker::ERR_INVALID_HEX,
        ERR_INVALID_HEX,
        "widget::color_picker::ERR_INVALID_HEX must read the catalogue value"
    );
}

#[test]
fn diff_viewer_labels_keep_their_original_paths_and_values() {
    assert_eq!(
        diff_viewer::LABEL_ACCEPT_HUNK,
        LABEL_ACCEPT_HUNK,
        "composite::diff_viewer::LABEL_ACCEPT_HUNK must read the catalogue value"
    );
    assert_eq!(
        diff_viewer::LABEL_REJECT_HUNK,
        LABEL_REJECT_HUNK,
        "composite::diff_viewer::LABEL_REJECT_HUNK must read the catalogue value"
    );
    assert_eq!(
        diff_viewer::LABEL_UNIFIED,
        LABEL_UNIFIED,
        "composite::diff_viewer::LABEL_UNIFIED must read the catalogue value"
    );
    assert_eq!(
        diff_viewer::LABEL_SIDE_BY_SIDE,
        LABEL_SIDE_BY_SIDE,
        "composite::diff_viewer::LABEL_SIDE_BY_SIDE must read the catalogue value"
    );
}

#[test]
fn composite_labels_keep_their_original_paths_and_values() {
    assert_eq!(
        command_palette::LABEL_ESC,
        LABEL_ESC,
        "composite::command_palette::LABEL_ESC must read the catalogue value"
    );
    assert_eq!(
        command_palette::PLACEHOLDER_FILES,
        PLACEHOLDER_FILES,
        "composite::command_palette::PLACEHOLDER_FILES must read the catalogue value"
    );
    assert_eq!(
        command_palette::PLACEHOLDER_COMMANDS,
        PLACEHOLDER_COMMANDS,
        "composite::command_palette::PLACEHOLDER_COMMANDS must read the catalogue value"
    );
    assert_eq!(
        command_palette::PLACEHOLDER_SYMBOLS,
        PLACEHOLDER_SYMBOLS,
        "composite::command_palette::PLACEHOLDER_SYMBOLS must read the catalogue value"
    );
    assert_eq!(
        command_palette::PLACEHOLDER_TAGS,
        PLACEHOLDER_TAGS,
        "composite::command_palette::PLACEHOLDER_TAGS must read the catalogue value"
    );
    assert_eq!(
        keybind_recorder::LABEL_RECORDING,
        LABEL_RECORDING,
        "composite::keybind_recorder::LABEL_RECORDING must read the catalogue value"
    );
    assert_eq!(
        keybind_recorder::LABEL_EMPTY,
        LABEL_EMPTY,
        "composite::keybind_recorder::LABEL_EMPTY must read the catalogue value"
    );
    assert_eq!(
        navigation_rail::LABEL_TOGGLE,
        LABEL_TOGGLE,
        "composite::navigation_rail::LABEL_TOGGLE must read the catalogue value"
    );
    assert_eq!(
        side_sheet::LABEL_PIN,
        LABEL_PIN,
        "composite::side_sheet::LABEL_PIN must read the catalogue value"
    );
    assert_eq!(
        side_sheet::LABEL_UNPIN,
        LABEL_UNPIN,
        "composite::side_sheet::LABEL_UNPIN must read the catalogue value"
    );
    assert_eq!(
        side_sheet::LABEL_CLOSE,
        LABEL_CLOSE,
        "composite::side_sheet::LABEL_CLOSE must read the catalogue value"
    );
    assert_eq!(
        stepper::LABEL_PREVIOUS,
        LABEL_PREVIOUS,
        "composite::stepper::LABEL_PREVIOUS must read the catalogue value"
    );
    assert_eq!(
        stepper::LABEL_NEXT,
        LABEL_NEXT,
        "composite::stepper::LABEL_NEXT must read the catalogue value"
    );
    assert_eq!(
        stepper::LABEL_FINISH,
        LABEL_FINISH,
        "composite::stepper::LABEL_FINISH must read the catalogue value"
    );
    assert_eq!(
        stepper::LABEL_CANCEL,
        LABEL_CANCEL,
        "composite::stepper::LABEL_CANCEL must read the catalogue value"
    );
}

#[test]
fn date_and_time_picker_labels_keep_their_original_paths_and_values() {
    assert_eq!(
        date_picker::PLACEHOLDER_DATE,
        PLACEHOLDER_DATE,
        "widget::date_picker::PLACEHOLDER_DATE must read the catalogue value"
    );
    assert_eq!(
        time_picker::LABEL_HR,
        LABEL_HR,
        "widget::time_picker::LABEL_HR must read the catalogue value"
    );
    assert_eq!(
        time_picker::LABEL_MIN,
        LABEL_MIN,
        "widget::time_picker::LABEL_MIN must read the catalogue value"
    );
    assert_eq!(
        time_picker::LABEL_SEC,
        LABEL_SEC,
        "widget::time_picker::LABEL_SEC must read the catalogue value"
    );
    assert_eq!(
        time_picker::LABEL_AM_PM,
        LABEL_AM_PM,
        "widget::time_picker::LABEL_AM_PM must read the catalogue value"
    );
    assert_eq!(
        time_picker::LABEL_AM,
        LABEL_AM,
        "widget::time_picker::LABEL_AM must read the catalogue value"
    );
    assert_eq!(
        time_picker::LABEL_PM,
        LABEL_PM,
        "widget::time_picker::LABEL_PM must read the catalogue value"
    );
    assert_eq!(
        time_picker::LABEL_APPLY,
        LABEL_APPLY,
        "widget::time_picker::LABEL_APPLY must read the catalogue value"
    );
    assert_eq!(
        time_picker::LABEL_CANCEL,
        LABEL_CANCEL,
        "widget::time_picker::LABEL_CANCEL must read the catalogue value"
    );
    assert_eq!(
        time_picker::PLACEHOLDER_24H,
        PLACEHOLDER_24H,
        "widget::time_picker::PLACEHOLDER_24H must read the catalogue value"
    );
    assert_eq!(
        time_picker::PLACEHOLDER_24H_SEC,
        PLACEHOLDER_24H_SEC,
        "widget::time_picker::PLACEHOLDER_24H_SEC must read the catalogue value"
    );
    assert_eq!(
        time_picker::PLACEHOLDER_12H,
        PLACEHOLDER_12H,
        "widget::time_picker::PLACEHOLDER_12H must read the catalogue value"
    );
    assert_eq!(
        time_picker::PLACEHOLDER_12H_SEC,
        PLACEHOLDER_12H_SEC,
        "widget::time_picker::PLACEHOLDER_12H_SEC must read the catalogue value"
    );
}

/// The colour picker, time picker and stepper all offer Apply and Cancel
/// controls. Collapsing them onto one definition each is the point of the
/// catalogue: a host cannot get a stale "Apply" from one widget and a fresh one
/// from another.
#[test]
fn shared_action_labels_are_a_single_definition() {
    assert_eq!(
        color_picker::LABEL_APPLY,
        time_picker::LABEL_APPLY,
        "Apply must be one string across the colour and time pickers"
    );
    assert_eq!(
        color_picker::LABEL_CANCEL,
        time_picker::LABEL_CANCEL,
        "Cancel must be one string across the colour and time pickers"
    );
    assert_eq!(
        color_picker::LABEL_CANCEL,
        stepper::LABEL_CANCEL,
        "Cancel must be one string across the colour picker and the stepper"
    );
}

/// The shipped copy, pinned so that changing what a user sees is a deliberate
/// edit rather than a side effect of a refactor.
#[test]
fn catalogue_text_is_the_shipped_copy() {
    assert_eq!(
        LABEL_APPLY, "\u{2713} Apply",
        "the apply label keeps its check mark and the space after it"
    );
    assert_eq!(LABEL_CANCEL, "Cancel");
    assert_eq!(LABEL_HEX, "Hex");
    assert_eq!(LABEL_PRESETS, "Presets");
    assert_eq!(ERR_INVALID_HEX, "Invalid hex color");
    assert_eq!(LABEL_ACCEPT_HUNK, "Accept hunk");
    assert_eq!(LABEL_REJECT_HUNK, "Reject hunk");
    assert_eq!(LABEL_UNIFIED, "Unified");
    assert_eq!(LABEL_SIDE_BY_SIDE, "Side-by-Side");
    assert_eq!(LABEL_ESC, "Esc");
    assert_eq!(PLACEHOLDER_FILES, "Search files by name...");
    assert_eq!(PLACEHOLDER_COMMANDS, "Type a command...");
    assert_eq!(PLACEHOLDER_SYMBOLS, "Search symbols...");
    assert_eq!(PLACEHOLDER_TAGS, "Search by tag...");
    assert_eq!(LABEL_RECORDING, "Press shortcut keys...");
    assert_eq!(LABEL_EMPTY, "Record shortcut");
    assert_eq!(LABEL_TOGGLE, "Toggle navigation rail");
    assert_eq!(LABEL_PIN, "Pin");
    assert_eq!(LABEL_UNPIN, "Unpin");
    assert_eq!(LABEL_CLOSE, "Close");
    assert_eq!(LABEL_PREVIOUS, "\u{2190} Previous");
    assert_eq!(LABEL_NEXT, "Next \u{2192}");
    assert_eq!(LABEL_FINISH, "Finish \u{2713}");
    assert_eq!(PLACEHOLDER_DATE, "YYYY-MM-DD");
    assert_eq!(LABEL_HR, "Hr");
    assert_eq!(LABEL_MIN, "Min");
    assert_eq!(LABEL_SEC, "Sec");
    assert_eq!(LABEL_AM_PM, "AM/PM");
    assert_eq!(LABEL_AM, "AM");
    assert_eq!(LABEL_PM, "PM");
    assert_eq!(PLACEHOLDER_24H, "--:--");
    assert_eq!(PLACEHOLDER_24H_SEC, "--:--:--");
    assert_eq!(PLACEHOLDER_12H, "--:-- --");
    assert_eq!(PLACEHOLDER_12H_SEC, "--:--:-- --");
}

#[test]
fn labels_are_presentable_single_line_text() {
    for (name, label) in [
        ("LABEL_APPLY", LABEL_APPLY),
        ("LABEL_CANCEL", LABEL_CANCEL),
        ("LABEL_HEX", LABEL_HEX),
        ("LABEL_PRESETS", LABEL_PRESETS),
        ("ERR_INVALID_HEX", ERR_INVALID_HEX),
        ("LABEL_ACCEPT_HUNK", LABEL_ACCEPT_HUNK),
        ("LABEL_REJECT_HUNK", LABEL_REJECT_HUNK),
        ("LABEL_UNIFIED", LABEL_UNIFIED),
        ("LABEL_SIDE_BY_SIDE", LABEL_SIDE_BY_SIDE),
        ("LABEL_ESC", LABEL_ESC),
        ("PLACEHOLDER_FILES", PLACEHOLDER_FILES),
        ("PLACEHOLDER_COMMANDS", PLACEHOLDER_COMMANDS),
        ("PLACEHOLDER_SYMBOLS", PLACEHOLDER_SYMBOLS),
        ("PLACEHOLDER_TAGS", PLACEHOLDER_TAGS),
        ("LABEL_RECORDING", LABEL_RECORDING),
        ("LABEL_EMPTY", LABEL_EMPTY),
        ("LABEL_TOGGLE", LABEL_TOGGLE),
        ("LABEL_PIN", LABEL_PIN),
        ("LABEL_UNPIN", LABEL_UNPIN),
        ("LABEL_CLOSE", LABEL_CLOSE),
        ("LABEL_PREVIOUS", LABEL_PREVIOUS),
        ("LABEL_NEXT", LABEL_NEXT),
        ("LABEL_FINISH", LABEL_FINISH),
        ("PLACEHOLDER_DATE", PLACEHOLDER_DATE),
        ("LABEL_HR", LABEL_HR),
        ("LABEL_MIN", LABEL_MIN),
        ("LABEL_SEC", LABEL_SEC),
        ("LABEL_AM_PM", LABEL_AM_PM),
        ("LABEL_AM", LABEL_AM),
        ("LABEL_PM", LABEL_PM),
        ("PLACEHOLDER_24H", PLACEHOLDER_24H),
        ("PLACEHOLDER_24H_SEC", PLACEHOLDER_24H_SEC),
        ("PLACEHOLDER_12H", PLACEHOLDER_12H),
        ("PLACEHOLDER_12H_SEC", PLACEHOLDER_12H_SEC),
    ] {
        assert!(
            !label.trim().is_empty(),
            "{name} is rendered on screen and must not be blank"
        );
        assert_eq!(label, label.trim(), "{name} must not be padded: {label:?}");
        assert!(
            !label.contains('\n') && !label.contains('\t'),
            "{name} is rendered in a single-line control: {label:?}"
        );
    }
}
