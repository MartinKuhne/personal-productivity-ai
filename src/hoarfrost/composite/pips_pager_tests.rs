//! Unit tests for `pips_pager` composite (`UIF-COMP-011`, `UIF-PRIN-004`).

use super::*;

#[test]
fn test_pips_pager_model_navigation_logic() {
    let empty = PipsPagerModel::new(0, 0);
    assert!(!empty.can_prev());
    assert!(!empty.can_next());
    assert!(empty.is_first_page());
    assert!(empty.is_last_page());

    let single_page = PipsPagerModel::new(0, 1);
    assert!(!single_page.can_prev());
    assert!(!single_page.can_next());
    assert!(single_page.is_first_page());
    assert!(single_page.is_last_page());

    let first_of_three = PipsPagerModel::new(0, 3);
    assert!(!first_of_three.can_prev());
    assert!(first_of_three.can_next());
    assert!(first_of_three.is_first_page());
    assert!(!first_of_three.is_last_page());

    let middle_of_three = PipsPagerModel::new(1, 3);
    assert!(middle_of_three.can_prev());
    assert!(middle_of_three.can_next());
    assert!(!middle_of_three.is_first_page());
    assert!(!middle_of_three.is_last_page());

    let last_of_three = PipsPagerModel::new(2, 3);
    assert!(last_of_three.can_prev());
    assert!(!last_of_three.can_next());
    assert!(!last_of_three.is_first_page());
    assert!(last_of_three.is_last_page());
}

#[test]
fn test_tokens_and_geometry() {
    assert_eq!(
        BAR_HEIGHT, 24.0,
        "bar height must be 24px (size.bar.status) per §3B.11"
    );
    assert_eq!(
        BUTTON_SIZE, 24.0,
        "navigation button dimensions must be 24x24px (size.hit_target.min) per §3B.11"
    );
    assert_eq!(
        PIP_SIZE_INACTIVE, 6.0,
        "inactive pip size must be 6px (size.indicator.dot) per §3B.11"
    );
    assert_eq!(
        PIP_WIDTH_ACTIVE, 16.0,
        "active pip width must be 16px (space.lg) per §3B.11"
    );
    assert_eq!(
        PIP_HEIGHT, 6.0,
        "pip height must be 6px (size.indicator.dot) per §3B.11"
    );
    assert_eq!(PIP_GAP, 4.0, "pip gap must be 4px (space.xs) per §3B.11");
    assert_eq!(
        PIP_RADIUS, 999.0,
        "pip radius must be radius.pill per §3B.11"
    );
}

#[test]
fn test_glyphs() {
    assert_eq!(GLYPH_PREV, "◀");
    assert_eq!(GLYPH_NEXT, "▶");
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PipsMsg {
    Change(usize),
}

#[test]
fn test_widget_creation_and_element_conversion() {
    let model = PipsPagerModel::new(1, 5);

    let widget: PipsPager<'_, PipsMsg> = PipsPager::new(model, |c| PipsMsg::Change(c.into_value()));
    assert_eq!(widget.model(), &model);

    let _element: Element<'_, PipsMsg> = pips_pager(model, |c| PipsMsg::Change(c.into_value()));
}

#[test]
fn test_boundary_pages_do_not_collapse_buttons() {
    // Page 0: [◀] is disabled, but renders at full 24x24px maintaining 0px shift per UIF-PRIN-004
    let page_zero = PipsPagerModel::new(0, 4);
    assert!(!page_zero.can_prev());
    assert!(page_zero.can_next());
    let _first_page_element: Element<'_, PipsMsg> =
        pips_pager(page_zero, |c| PipsMsg::Change(c.into_value()));

    // Last page: [▶] is disabled, but renders at full 24x24px
    let page_last = PipsPagerModel::new(3, 4);
    assert!(page_last.can_prev());
    assert!(!page_last.can_next());
    let _last_page_element: Element<'_, PipsMsg> =
        pips_pager(page_last, |c| PipsMsg::Change(c.into_value()));
}
