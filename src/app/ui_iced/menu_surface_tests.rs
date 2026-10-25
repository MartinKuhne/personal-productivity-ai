//! Unit tests for the shared framework-menu card chrome and solved geometry.
//!
//! The render path itself (`card_element`, `overlay`) is exercised against a
//! live widget tree in
//! [`interaction_tests`](crate::ui_iced::interaction_tests); what is
//! unit-testable here is the **grid** every menu card is painted from, and
//! the open-card rectangle walk that feeds the `SurfaceRegistry`.

use super::*;
use iced_pro::focus::FocusState;
use iced_pro::surface::menu::cascade::{MenuCascade, MenuTreeKind};
use iced_pro::surface::menu::model::{Row, submenu_id};

/// A root menu holding `keys` as plain action rows.
fn menu_with(keys: &[&str]) -> Menu {
    Menu::new(
        "surface.test",
        keys.iter().map(|key| Row::action(*key, *key)).collect(),
    )
}

/// A root menu whose row 1 is a submenu parent holding `child_keys`.
fn menu_with_submenu(child_keys: &[&str]) -> Menu {
    let root = MenuId::new("surface.test");
    let child = Menu::new(
        submenu_id(&root, "child"),
        child_keys
            .iter()
            .map(|key| Row::action(*key, *key))
            .collect::<Vec<_>>(),
    );
    Menu::new(
        root,
        vec![
            Row::action("first", "first"),
            Row::submenu("child", "child", child),
            Row::action("last", "last"),
        ],
    )
}

/// A cascade with `menu` open as a fixed navbar tree.
fn opened(menu: &Menu) -> MenuCascade {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    let _ = cascade.open(menu, MenuTreeKind::Fixed, &mut focus);
    cascade
}

/// A `Placement` standing in for "the root card was solved to this rectangle".
///
/// `Placement` has no public constructor outside `anchor::solve`, so the
/// struct literal is spelled once here rather than at every call site.
fn placement(rect: Rectangle) -> Placement {
    Placement {
        origin: Point::new(rect.x, rect.y),
        size: Size::new(rect.width, rect.height),
        flipped_horizontal: false,
        flipped_vertical: false,
        scrollable: false,
        clamped_horizontal: false,
        clamped_vertical: false,
    }
}

#[test]
fn test_card_size_is_fixed_width_over_the_row_sum_plus_inset() {
    let menu = menu_with(&["a", "b", "c"]);
    assert_eq!(
        card_size(&menu),
        Size::new(CARD_WIDTH, view::ROW_HEIGHT * 3.0 + CARD_INSET * 2.0),
        "the card is CARD_WIDTH wide and as tall as its rows plus one inset per side"
    );
}

#[test]
fn test_card_size_of_an_empty_menu_is_two_insets() {
    let menu = menu_with(&[]);
    assert_eq!(
        card_size(&menu),
        Size::new(CARD_WIDTH, CARD_INSET * 2.0),
        "with no rows the card collapses to its inset, it never goes negative"
    );
}

#[test]
fn test_row_grid_is_contiguous_and_exactly_fills_the_card() {
    let menu = Menu::new(
        "surface.test",
        vec![
            Row::heading("Head"),
            Row::separator(),
            Row::action("a", "A"),
        ],
    );
    let card = Rectangle::new(Point::new(10.0, 20.0), card_size(&menu));

    let heading = row_rect(&menu, card, 0).expect("heading row");
    let separator = row_rect(&menu, card, 1).expect("separator row");
    let action = row_rect(&menu, card, 2).expect("action row");

    assert_eq!(heading.y, card.y + CARD_INSET);
    assert_eq!(separator.y, heading.y + view::HEADING_HEIGHT);
    assert_eq!(action.y, separator.y + view::SEPARATOR_HEIGHT);
    assert_eq!(
        action.y + action.height,
        card.y + card.height - CARD_INSET,
        "there is no inter-row gap: the last row's bottom edge is the card's \
         *inner* bottom edge, which is what makes card_size and row_rect agree"
    );
    assert_eq!(
        action.width,
        card.width - CARD_INSET * 2.0,
        "rows span the card inside its inset on both sides"
    );
}

#[test]
fn test_row_rect_is_none_for_a_row_the_menu_does_not_have() {
    let menu = menu_with(&["a"]);
    let card = Rectangle::new(Point::ORIGIN, card_size(&menu));
    assert_eq!(row_rect(&menu, card, 1), None);
}

#[test]
fn test_open_card_rects_uses_the_supplied_root_placement_verbatim() {
    let menu = menu_with(&["a", "b"]);
    let cascade = opened(&menu);
    let viewport = Size::new(1200.0, 800.0);
    let want = Rectangle::new(Point::new(7.0, 9.0), card_size(&menu));

    let rects = open_card_rects(&cascade, viewport, |_| placement(want));

    assert_eq!(rects, vec![want]);
}

#[test]
fn test_open_card_rects_anchors_a_child_to_its_trigger_row_not_its_parent_card() {
    let menu = menu_with_submenu(&["c1", "c2"]);
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    let _ = cascade.open(&menu, MenuTreeKind::Fixed, &mut focus);
    let root = MenuId::new("surface.test");
    let _ = cascade.open_submenu(&root, 1, &mut focus);

    let viewport = Size::new(1200.0, 800.0);
    let root_rect = Rectangle::new(Point::new(100.0, 40.0), card_size(&menu));
    let rects = open_card_rects(&cascade, viewport, |_| placement(root_rect));

    assert_eq!(rects.len(), 2, "root card plus the open submenu");
    assert_eq!(
        rects[1].y,
        root_rect.y + CARD_INSET + view::ROW_HEIGHT,
        "the submenu's top edge lines up with row 1's top edge (§4.8.1 / \
         UIF-FMENU-002), not with the root card's top edge"
    );
}

#[test]
fn test_a_closed_cascade_solves_no_rectangles() {
    let cascade = MenuCascade::new();
    assert!(
        open_card_rects(&cascade, Size::new(800.0, 600.0), |_| {
            placement(Rectangle::new(Point::ORIGIN, Size::ZERO))
        })
        .is_empty(),
        "nothing open means nothing registered"
    );
}

#[test]
fn test_card_inset_is_the_framework_token_not_a_restated_number() {
    assert_eq!(
        CARD_INSET,
        iced_pro::token::space::XS,
        "the card inset comes from the token, so it cannot drift from §4.8.1"
    );
}
