//! Tests for tab strip model, dirty close guards, and external deletion synchronization (`UIF-TAB-003`).

use super::*;
use crate::token::color::Palette;

#[test]
fn test_tab_strip_constants() {
    assert_eq!(TAB_STRIP_HEIGHT, 32.0);
    assert_eq!(TAB_MIN_WIDTH, 64.0);
    assert_eq!(TAB_MAX_WIDTH, 200.0);
    assert_eq!(DIRTY_DOT_DIAMETER, 6.0);
    assert_eq!(CLOSE_BUTTON_SIZE, 16.0);
}

#[test]
fn test_clean_tab_close_yields_immediate_close_and_mru_handoff() {
    let mut model = TabStripModel::new();
    model.open_tab(TabItem::new("a", "Doc A"));
    model.open_tab(TabItem::new("b", "Doc B"));
    model.open_tab(TabItem::new("c", "Doc C"));

    assert_eq!(model.active_id.as_deref(), Some("c"));
    assert_eq!(model.tabs.len(), 3);

    // Switch to Doc A, making MRU order: B, C, A
    model.select_tab("a");
    assert_eq!(model.active_id.as_deref(), Some("a"));

    // Close Doc A: clean close, should activate Doc C (previous in MRU)
    let verdict = model.request_close("a");
    assert_eq!(
        verdict,
        TabCloseVerdict::CloseImmediately {
            new_active_id: Some("c".to_string()),
        }
    );
    assert_eq!(model.active_id.as_deref(), Some("c"));
    assert_eq!(model.tabs.len(), 2);

    // Close Doc C: should activate Doc B
    let verdict = model.request_close("c");
    assert_eq!(
        verdict,
        TabCloseVerdict::CloseImmediately {
            new_active_id: Some("b".to_string()),
        }
    );
    assert_eq!(model.active_id.as_deref(), Some("b"));
    assert_eq!(model.tabs.len(), 1);
}

#[test]
fn test_dirty_tab_close_requests_confirmation() {
    let mut model = TabStripModel::new();
    model.open_tab(TabItem::new("a", "Doc A"));
    model.open_tab(TabItem::new("b", "Doc B").with_dirty(true));

    assert_eq!(model.active_id.as_deref(), Some("b"));

    // Closing dirty tab B requests confirmation without closing it
    let verdict = model.request_close("b");
    assert_eq!(
        verdict,
        TabCloseVerdict::RequiresConfirmation {
            tab_id: "b".to_string(),
            title: "Doc B".to_string(),
        }
    );
    assert_eq!(model.tabs.len(), 2);
    assert_eq!(model.active_id.as_deref(), Some("b"));

    // Confirm close forces close and hands focus to Doc A
    let next = model.confirm_close("b");
    assert_eq!(next.as_deref(), Some("a"));
    assert_eq!(model.tabs.len(), 1);
    assert_eq!(model.active_id.as_deref(), Some("a"));
}

#[test]
fn test_external_deletion_sync_closes_immediately_and_hands_off_mru() {
    let mut model = TabStripModel::new();
    model.open_tab(TabItem::new("path/a.md", "a.md"));
    model.open_tab(TabItem::new("path/b.md", "b.md").with_dirty(true));

    assert_eq!(model.active_id.as_deref(), Some("path/b.md"));

    // External file deletion bypasses confirmation dialog and hands off to remaining tab
    let next = model.external_deletion_sync("path/b.md");
    assert_eq!(next.as_deref(), Some("path/a.md"));
    assert_eq!(model.tabs.len(), 1);
    assert_eq!(model.active_id.as_deref(), Some("path/a.md"));
}

#[test]
fn test_request_close_all_returns_dirty_tabs() {
    let mut model = TabStripModel::new();
    model.open_tab(TabItem::new("a", "Doc A"));
    model.open_tab(TabItem::new("b", "Doc B").with_dirty(true));
    model.open_tab(TabItem::new("c", "Doc C").with_dirty(true));

    let dirty = model.request_close_all();
    assert_eq!(dirty, vec!["b".to_string(), "c".to_string()]);
}

#[test]
fn test_tab_strip_widget_into_element_renders_cleanly() {
    #[allow(dead_code)]
    #[derive(Clone, Debug)]
    enum Msg {
        Select(String),
        Close(String),
    }

    let mut model = TabStripModel::new();
    model.open_tab(TabItem::new("a", "Doc A").with_tooltip("/path/to/Doc A"));
    model.open_tab(TabItem::new("b", "Doc B").with_dirty(true));

    let tab_strip = TabStrip::new(&model)
        .with_palette(Palette::dark())
        .on_select_tab(Msg::Select)
        .on_close_tab(Msg::Close);

    let _el: Element<'_, Msg> = tab_strip.into();
}
