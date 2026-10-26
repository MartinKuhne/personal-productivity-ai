//! Unit tests for `pagination` bar composite (`UIF-COMP-014`, `UIF-PRIN-003`).

use super::*;

#[test]
fn test_pagination_model_pages_and_ranges() {
    let empty = PaginationModel::new(0, 50, 0, vec![25, 50, 100]);
    assert_eq!(empty.total_pages(), 0);
    assert_eq!(empty.start_item(), 0);
    assert_eq!(empty.end_item(), 0);
    assert!(!empty.can_first());
    assert!(!empty.can_prev());
    assert!(!empty.can_next());
    assert!(!empty.can_last());
    assert_eq!(empty.range_summary(), "Showing 0 of 0");
    assert_eq!(empty.page_indicator(), "Page 0 of 0");

    let first_page = PaginationModel::new(0, 50, 1240, vec![25, 50, 100]);
    assert_eq!(first_page.total_pages(), 25);
    assert_eq!(first_page.start_item(), 1);
    assert_eq!(first_page.end_item(), 50);
    assert!(!first_page.can_first());
    assert!(!first_page.can_prev());
    assert!(first_page.can_next());
    assert!(first_page.can_last());
    assert_eq!(first_page.range_summary(), "Showing 1–50 of 1,240");
    assert_eq!(first_page.page_indicator(), "Page 1 of 25");

    let mid_page = PaginationModel::new(1, 50, 1240, vec![25, 50, 100]);
    assert_eq!(mid_page.start_item(), 51);
    assert_eq!(mid_page.end_item(), 100);
    assert!(mid_page.can_first());
    assert!(mid_page.can_prev());
    assert!(mid_page.can_next());
    assert!(mid_page.can_last());
    assert_eq!(mid_page.range_summary(), "Showing 51–100 of 1,240");
    assert_eq!(mid_page.page_indicator(), "Page 2 of 25");

    let last_page = PaginationModel::new(24, 50, 1240, vec![25, 50, 100]);
    assert_eq!(last_page.start_item(), 1201);
    assert_eq!(last_page.end_item(), 1240);
    assert!(last_page.can_first());
    assert!(last_page.can_prev());
    assert!(!last_page.can_next());
    assert!(!last_page.can_last());
    assert_eq!(last_page.range_summary(), "Showing 1,201–1,240 of 1,240");
    assert_eq!(last_page.page_indicator(), "Page 25 of 25");
}

#[test]
fn test_format_thousands() {
    assert_eq!(format_thousands(0), "0");
    assert_eq!(format_thousands(9), "9");
    assert_eq!(format_thousands(99), "99");
    assert_eq!(format_thousands(999), "999");
    assert_eq!(format_thousands(1000), "1,000");
    assert_eq!(format_thousands(1240), "1,240");
    assert_eq!(format_thousands(10000), "10,000");
    assert_eq!(format_thousands(100000), "100,000");
    assert_eq!(format_thousands(1000000), "1,000,000");
    assert_eq!(format_thousands(12345678), "12,345,678");
}

#[test]
fn test_tokens_and_geometry() {
    assert_eq!(
        BAR_HEIGHT, 32.0,
        "bar height must be 32px (size.control.md) per §3B.14"
    );
    assert_eq!(
        BORDER_WIDTH, 1.0,
        "border width must be 1px hairline per §3B.14"
    );
    assert_eq!(
        BUTTON_SIZE, 28.0,
        "navigation button size must be 28x28px (size.control.sm) per §3B.14"
    );
    assert_eq!(
        PADDING_X, 12.0,
        "horizontal padding must be 12px (space.md) per §3B.14"
    );
    assert_eq!(
        NAV_SPACING, 8.0,
        "navigation spacing must be 8px (space.sm) per §3B.14"
    );
    assert_eq!(
        OPTION_SPACING, 4.0,
        "option spacing must be 4px (space.xs) per §3B.14"
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestMsg {
    Page(usize),
    PageSize(usize),
}

#[test]
fn test_widget_creation_and_element_conversion() {
    let model = PaginationModel::new(0, 50, 100, vec![25, 50, 100]);

    let widget: PaginationBar<'_, TestMsg> = PaginationBar::new(
        model.clone(),
        |commit| TestMsg::Page(commit.into_value()),
        |commit| TestMsg::PageSize(commit.into_value()),
    );
    assert_eq!(widget.model(), &model);

    let _element: Element<'_, TestMsg> = pagination_bar(
        model,
        |commit| TestMsg::Page(commit.into_value()),
        |commit| TestMsg::PageSize(commit.into_value()),
    );
}
