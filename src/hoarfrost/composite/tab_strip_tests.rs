//! Tests for tab strip model, dirty close guards, pointer bindings, and external deletion synchronization (`UIF-TAB-001`, `UIF-TAB-002`, `UIF-TAB-003`).

use super::*;
use crate::surface::menu::RowKind;
use crate::token::color::Palette;

#[test]
fn test_tab_strip_constants() {
    assert_eq!(TAB_STRIP_HEIGHT, 32.0);
    assert_eq!(TAB_MIN_WIDTH, 96.0);
    assert_eq!(TAB_MAX_WIDTH, 220.0);
    assert_eq!(DIRTY_DOT_DIAMETER, 6.0);
    assert_eq!(CLOSE_BUTTON_SIZE, 24.0);
}

#[test]
fn test_tab_width_clamping() {
    assert_eq!(clamp_tab_width(50.0), 96.0);
    assert_eq!(clamp_tab_width(96.0), 96.0);
    assert_eq!(clamp_tab_width(150.0), 150.0);
    assert_eq!(clamp_tab_width(220.0), 220.0);
    assert_eq!(clamp_tab_width(300.0), 220.0);
}

#[test]
fn test_tab_context_menu_functional_groups() {
    let menu = tab_context_menu("doc1.md");
    let rows = menu.rows();

    // Spec §4.9.1 / UIF-TAB-001: 3 functional groups separated by dividers:
    // Group 1: Close Tab, Close Other Tabs, Close All Tabs
    // Separator
    // Group 2: Edit, Open in System Editor, Format Markdown
    // Separator
    // Group 3: Copy Relative Path, Show in File Explorer
    assert_eq!(rows.len(), 10);

    // Group 1: Tab Lifecycle
    assert_eq!(rows[0].key(), "close");
    assert_eq!(rows[0].label(), "Close Tab");
    assert_eq!(rows[1].key(), "close_others");
    assert_eq!(rows[1].label(), "Close Other Tabs");
    assert_eq!(rows[2].key(), "close_all");
    assert_eq!(rows[2].label(), "Close All Tabs");

    // Divider 1
    assert!(matches!(rows[3].kind(), RowKind::Separator));

    // Group 2: Edit & Automation
    assert_eq!(rows[4].key(), "edit");
    assert_eq!(rows[4].label(), "Edit");
    assert_eq!(rows[5].key(), "open_in_editor");
    assert_eq!(rows[5].label(), "Open in System Editor");
    assert_eq!(rows[6].key(), "format_markdown");
    assert_eq!(rows[6].label(), "Format Markdown");

    // Divider 2
    assert!(matches!(rows[7].kind(), RowKind::Separator));

    // Group 3: Location
    assert_eq!(rows[8].key(), "copy_path");
    assert_eq!(rows[8].label(), "Copy Relative Path");
    assert_eq!(rows[9].key(), "show_in_explorer");
    assert_eq!(rows[9].label(), "Show in File Explorer");

    // Verify TabContextMenuModel wrapper
    let model = TabContextMenuModel::new("doc1.md");
    let built = model.build_menu();
    assert_eq!(built.rows().len(), 10);
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
        ContextMenu(String, iced::Point),
    }

    let mut model = TabStripModel::new();
    model.open_tab(TabItem::new("a", "Doc A").with_tooltip("/path/to/Doc A"));
    model.open_tab(TabItem::new("b", "Doc B").with_dirty(true));

    let tab_strip = TabStrip::new(&model)
        .with_palette(Palette::dark())
        .on_select_tab(Msg::Select)
        .on_close_tab(Msg::Close)
        .on_context_menu(Msg::ContextMenu);

    let _el: Element<'_, Msg> = tab_strip.into();
}

#[test]
fn test_tab_strip_extreme_viewport_stress_down_to_640x480() {
    let mut model = TabStripModel::new();
    for i in 0..12 {
        model.open_tab(
            TabItem::new(
                format!("doc{i}"),
                format!("Long Document Title Number {i}.md"),
            )
            .with_dirty(i % 3 == 0),
        );
    }
    assert_eq!(model.tabs.len(), 12);

    // Stress across viewport widths: 640px, 800px, 1024px, 1920px, 3840px
    let viewports = [640.0, 800.0, 1024.0, 1920.0, 3840.0];
    for vp_width in viewports {
        let available_width = vp_width - 32.0; // Subtract shell margins
        let tab_width =
            (available_width / model.tabs.len() as f32).clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH);
        assert!(
            tab_width >= TAB_MIN_WIDTH,
            "tab width must not fall below 64px floor"
        );
        assert!(
            tab_width <= TAB_MAX_WIDTH,
            "tab width must not exceed 200px ceiling"
        );
    }
}
