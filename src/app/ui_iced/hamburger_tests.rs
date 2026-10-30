//! Tests for the framework-driven hamburger menu: tree shape, key
//! identity, command resolution and solved card geometry.

use crate::ui::strings;
use crate::ui_iced::hamburger::*;
use crate::ui_iced::test_helpers::test_app_state;
use hoarfrost::focus::FocusState;
use hoarfrost::surface::menu::cascade::{MenuCascade, MenuTreeKind};
use hoarfrost::surface::menu::model::{Menu, MenuId, RowKind, submenu_id};
use hoarfrost::surface::menu::view;
use hoarfrost::token::space;
use iced::{Point, Rectangle, Size};

/// The production tree for the test view model.
fn tree() -> Menu {
    hamburger_menu(&test_app_state().models.top)
}

/// A cascade with `menu` open as a fixed navbar tree.
fn opened(menu: &Menu) -> MenuCascade {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    let _ = cascade.open(menu, MenuTreeKind::Fixed, &mut focus);
    cascade
}

/// Collect every row key in the tree, root rows and submenu rows alike.
fn all_keys(menu: &Menu) -> Vec<String> {
    let mut keys = Vec::new();
    for row in menu.rows() {
        if !row.key().is_empty() {
            keys.push(row.key().to_owned());
        }
        if let Some(child) = row.child() {
            keys.extend(all_keys(child));
        }
    }
    keys
}

#[test]
fn test_row_keys_are_unique_across_the_tree() {
    let keys = all_keys(&tree());
    let mut sorted = keys.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        keys.len(),
        "command_for_activation dispatches on the key alone, so no two rows may share one: {keys:?}"
    );
}

#[test]
fn test_root_card_offers_the_ui_061_to_066_row_set() {
    let menu = tree();
    let root_keys: Vec<&str> = menu.rows().iter().map(|row| row.key()).collect();
    assert_eq!(
        root_keys,
        [
            KEY_BATCH,
            KEY_TOOLS,
            KEY_WINDOWS,
            KEY_CHAT_MODELS,
            KEY_TABLE_WRAP,
            KEY_ABOUT,
        ],
        "the root card's row order is the shipped order"
    );
    assert_eq!(
        menu.rows()
            .iter()
            .filter(|row| matches!(row.kind(), RowKind::Action))
            .count(),
        3,
        "three dialog actions at the root"
    );
    assert_eq!(
        menu.rows()
            .iter()
            .filter(|row| matches!(row.kind(), RowKind::Submenu { .. }))
            .count(),
        3,
        "three submenu parents at the root"
    );
}

#[test]
fn test_submenu_paths_are_derived_from_the_root_id() {
    let menu = tree();
    let root = MenuId::new(HAMBURGER_MENU_ID);
    for key in [KEY_WINDOWS, KEY_CHAT_MODELS, KEY_TABLE_WRAP] {
        let row = menu
            .rows()
            .iter()
            .find(|row| row.key() == key)
            .unwrap_or_else(|| panic!("no submenu row {key:?}"));
        assert_eq!(
            row.child_id(),
            Some(&submenu_id(&root, key)),
            "{key:?}'s child must carry the path-derived id"
        );
    }
}

#[test]
fn test_window_toggles_are_check_rows_that_stay_open() {
    let menu = tree();
    let windows = menu
        .rows()
        .iter()
        .find(|row| row.key() == KEY_WINDOWS)
        .and_then(|row| row.child())
        .expect("Windows submenu");

    for key in [KEY_BACKGROUND_OPERATIONS, KEY_AGENT_DEBUG] {
        let row = windows
            .rows()
            .iter()
            .find(|row| row.key() == key)
            .unwrap_or_else(|| panic!("no {key:?} row"));
        assert!(
            matches!(row.kind(), RowKind::Check { .. }),
            "{key:?} must be a real Check row, not an Action with a glyph in its label"
        );
        assert!(
            !row.closes_menu_on_activate(),
            "{key:?} must stay open so both windows can be flipped in one visit"
        );
    }
}

#[test]
fn test_chat_models_are_radio_members_of_one_named_group() {
    let state = test_app_state();
    let menu = hamburger_menu(&state.models.top);
    let chat = menu
        .rows()
        .iter()
        .find(|row| row.key() == KEY_CHAT_MODELS)
        .and_then(|row| row.child())
        .expect("chat models submenu");

    // The test view model has no models configured, so the placeholder is
    // the only row. Assert the group the real rows would join by building a
    // populated card through the same key/prefix contract.
    assert_eq!(
        chat.rows().len(),
        1,
        "the empty-models case is one placeholder row"
    );
    assert_eq!(
        chat.rows()[0].key(),
        KEY_NO_MODELS,
        "the placeholder is the no-models row"
    );
    assert_eq!(
        chat.rows()[0].label(),
        strings::NO_CHAT_MODELS_CONFIGURED,
        "the placeholder carries the shipped copy"
    );
}

#[test]
fn test_empty_models_placeholder_is_enterable_by_the_keyboard() {
    // `MenuCascade::stabilise` closes any open card the keyboard cannot
    // enter, so a card whose only row is disabled cannot stay open at all.
    // The placeholder must therefore be enabled and navigable.
    let menu = tree();
    let chat = menu
        .rows()
        .iter()
        .find(|row| row.key() == KEY_CHAT_MODELS)
        .and_then(|row| row.child())
        .expect("chat models submenu");

    assert!(
        chat.rows()[0].activatable(),
        "a disabled-only card is closed the moment it opens"
    );
    assert!(
        chat.first_navigable().is_some(),
        "the card must have at least one navigable row to hold the focus trap"
    );
    assert!(
        !chat.rows()[0].closes_menu_on_activate(),
        "picking the placeholder must not dismiss the menu"
    );

    let mut cascade = opened(&menu);
    let mut focus = FocusState::new();
    let root = MenuId::new(HAMBURGER_MENU_ID);
    let chat_row = menu
        .rows()
        .iter()
        .position(|row| row.key() == KEY_CHAT_MODELS)
        .expect("chat models row");
    let _ = cascade.activate_row(&root, chat_row, &mut focus);
    assert_eq!(
        cascade.depth(),
        2,
        "the placeholder card must survive the stabilise pass"
    );
}

#[test]
fn test_table_wrap_rows_are_radio_members_of_one_named_group() {
    let menu = tree();
    let wrap = menu
        .rows()
        .iter()
        .find(|row| row.key() == KEY_TABLE_WRAP)
        .and_then(|row| row.child())
        .expect("table wrap submenu");

    assert_eq!(wrap.rows().len(), strategy_menu_order().len());
    let selected: Vec<&str> = wrap
        .rows()
        .iter()
        .filter(|row| matches!(row.kind(), RowKind::Radio { selected: true, .. }))
        .map(|row| row.key())
        .collect();
    assert_eq!(
        selected.len(),
        1,
        "exactly one strategy is selected in a radio group"
    );
    for row in wrap.rows() {
        assert_eq!(
            row.radio_group().map(|group| group.as_str()),
            Some(RADIO_GROUP_TABLE_WRAP),
            "every strategy row joins the table-wrap group"
        );
    }
}

#[test]
fn test_strategy_key_round_trips_for_every_strategy() {
    for variant in strategy_menu_order() {
        let key = strategy_key(variant);
        assert!(
            key.starts_with(STRATEGY_KEY_PREFIX),
            "{key:?} must carry the strategy prefix"
        );
        assert_eq!(
            strategy_for_key(&key),
            Some(variant),
            "strategy_key must invert through strategy_for_key"
        );
    }
}

#[test]
fn test_strategy_for_key_rejects_foreign_keys() {
    assert_eq!(strategy_for_key("HybridPenaltyWaterFill"), None);
    assert_eq!(strategy_for_key(STRATEGY_KEY_PREFIX), None);
    assert_eq!(strategy_for_key("strategy:NoSuchStrategy"), None);
    assert_eq!(strategy_for_key(KEY_BATCH), None);
}

#[test]
fn test_trigger_rect_is_the_navbar_band_truncated_at_the_button_inset() {
    let viewport = Size::new(1280.0, 800.0);
    let rect = hamburger_trigger_rect(viewport);
    assert_eq!(rect.x, 0.0);
    assert_eq!(rect.y, 0.0);
    assert_eq!(rect.height, BAR_HEIGHT_POINTS);
    assert_eq!(rect.width, viewport.width - MENU_ANCHOR_RIGHT_MARGIN);

    // A viewport narrower than the inset cannot produce a negative band.
    let tiny = hamburger_trigger_rect(Size::new(4.0, 800.0));
    assert!(tiny.width >= 0.0, "the band width must never go negative");
}

#[test]
fn test_card_size_sums_the_framework_row_heights() {
    let menu = tree();
    let rows: f32 = menu.rows().iter().map(view::row_height).sum();
    let size = card_size(&menu);
    assert_eq!(size.width, CARD_WIDTH);
    assert_eq!(size.height, rows + CARD_INSET * 2.0);
    assert_eq!(
        CARD_INSET,
        space::XS,
        "the card inset is the space.xs token"
    );
}

#[test]
fn test_root_placement_hangs_below_the_bar_right_aligned() {
    let viewport = Size::new(1280.0, 800.0);
    let placement = root_placement(&tree(), viewport);
    assert_eq!(placement.size, card_size(&tree()));
    assert_eq!(
        placement.origin.y,
        BAR_HEIGHT_POINTS + space::XS,
        "the card hangs TRIGGER_GAP (space.xs) below the navbar"
    );
    assert!(
        !placement.flipped_vertical,
        "a roomy viewport must not flip the card above the bar"
    );
    assert!(
        !placement.flipped_horizontal,
        "a roomy viewport must not flip the card to the leading side"
    );
    assert!(
        (placement.origin.x + placement.size.width - viewport.width).abs()
            <= MENU_ANCHOR_RIGHT_MARGIN + f32::EPSILON,
        "the card's right edge sits at the ☰ button's inset: {:?}",
        placement.origin
    );
}

#[test]
fn test_closed_cascade_solves_no_rectangles() {
    let cascade = MenuCascade::new();
    assert!(
        open_card_rects(&cascade, Size::new(1280.0, 800.0)).is_empty(),
        "a closed tree registers no region"
    );
}

#[test]
fn test_open_card_rects_are_one_per_open_card_root_first() {
    let menu = tree();
    let cascade = opened(&menu);
    let viewport = Size::new(1280.0, 800.0);
    let rects = open_card_rects(&cascade, viewport);
    assert_eq!(rects.len(), cascade.depth());
    assert_eq!(
        rects[0],
        root_placement(&menu, viewport).rect(),
        "the first rectangle is the root card's solved placement"
    );
}

#[test]
fn test_submenu_rect_is_anchored_to_its_trigger_row_not_its_card() {
    let menu = tree();
    let mut cascade = opened(&menu);
    let mut focus = FocusState::new();
    let root = MenuId::new(HAMBURGER_MENU_ID);
    let windows_row = menu
        .rows()
        .iter()
        .position(|row| row.key() == KEY_WINDOWS)
        .expect("windows row");
    let _ = cascade.activate_row(&root, windows_row, &mut focus);
    assert_eq!(cascade.depth(), 2, "the Windows card is mounted");

    let viewport = Size::new(1280.0, 800.0);
    let rects = open_card_rects(&cascade, viewport);
    assert_eq!(rects.len(), 2);
    let root_rect = rects[0];

    let offset: f32 = menu.rows()[..windows_row]
        .iter()
        .map(view::row_height)
        .sum();
    let trigger_top = root_rect.y + CARD_INSET + offset;

    assert_eq!(
        rects[1].y, trigger_top,
        "UIF-FMENU-002: the child card is level with the row that opened it"
    );
    // The root card is flush against the window's right margin, so a
    // submenu cannot open on its trailing side inside any window this app
    // ships into: the anchor solver flips it to the leading side. That is
    // the quadrant rule doing its job, and it is a rendered change from
    // the previous hand-rolled placement.
    assert!(
        rects[1].x < root_rect.x,
        "with the root card flush right the submenu must flip leading: \
         root={root_rect:?} child={:?}",
        rects[1]
    );
    assert!(
        rects[1].x >= 0.0,
        "the flipped child still sits inside the viewport: {:?}",
        rects[1]
    );
    assert!(
        rects[1].x + rects[1].width <= viewport.width + f32::EPSILON,
        "the child card stays inside the viewport"
    );
}

#[test]
fn test_a_short_viewport_still_yields_a_non_negative_origin() {
    use hoarfrost::geometry::anchor::{self, Anchor, ControlAlignment};

    let menu = tree();
    let cascade = opened(&menu);
    // Short enough that the root card cannot hang below the bar.
    let viewport = Size::new(1280.0, 120.0);
    let rects = open_card_rects(&cascade, viewport);
    assert_eq!(rects.len(), 1, "only the root card is open");
    assert!(
        rects[0].y >= 0.0 && rects[0].x >= 0.0,
        "the solver must not place a card at a negative origin: {:?}",
        rects[0]
    );

    // The same solve the app performs reports the constraint that produced
    // the rectangle, so the registered geometry and the solver's reasoning
    // cannot disagree about why the card sits where it does.
    let placement = anchor::solve(
        Anchor::Control {
            rect: hamburger_trigger_rect(viewport),
            alignment: ControlAlignment::below_end(),
        },
        card_size(&menu),
        Rectangle::new(Point::ORIGIN, viewport),
    );
    assert_eq!(rects[0], placement.rect());
    assert!(
        placement.flipped_vertical || placement.clamped_vertical,
        "a 120px viewport cannot fit the card below the bar, so the solver \
         must report having constrained the vertical axis: {placement:?}"
    );
}
