//! Tests for search box widget and model (`UIF-PANE-003`, `UIF-PRIN-006`).

use super::*;
use iced::Element;

#[test]
fn test_search_box_constants() {
    assert_eq!(SEARCH_INPUT_HEIGHT, 32.0);
    assert_eq!(SEARCH_ACTION_SIZE, 28.0);
    assert_eq!(SEARCH_PLACEHOLDER, "Search content…");
    assert_eq!(GLYPH_SEARCH, '🔍');
    assert_eq!(GLYPH_CLEAR, '×');
}

#[test]
fn test_search_box_model() {
    let empty = SearchBoxModel::new("");
    assert_eq!(empty.query, "");
    assert!(!empty.is_active());
    assert_eq!(empty.trailing_glyph(), GLYPH_SEARCH);

    let with_text = SearchBoxModel::new("find something");
    assert_eq!(with_text.query, "find something");
    assert!(with_text.is_active());
    assert_eq!(with_text.trailing_glyph(), GLYPH_CLEAR);

    let active_empty = SearchBoxModel::new("").with_active(true);
    assert!(active_empty.is_active());
    assert_eq!(active_empty.trailing_glyph(), GLYPH_CLEAR);
}

#[test]
fn test_search_box_renders_cleanly() {
    #[allow(dead_code)]
    #[derive(Clone, Debug)]
    enum Msg {
        Input(String),
        Submit,
        Clear,
    }

    let search_box = SearchBox::new("query")
        .with_placeholder("Search notes…")
        .on_input(Msg::Input)
        .on_submit(Msg::Submit)
        .on_clear(Msg::Clear);

    let _el: Element<'_, Msg> = search_box.into();
}
