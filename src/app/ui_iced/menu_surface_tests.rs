//! Unit tests for the shared framework-menu card chrome and solved geometry.
//!
//! The render path itself (`card_element`, `overlay`) is exercised against a
//! live widget tree in
//! [`interaction_tests`](crate::ui_iced::interaction_tests); what is
//! unit-testable here is the **grid** every menu card is painted from, and
//! the open-card rectangle walk that feeds the `SurfaceRegistry`.

use super::*;
use hoarfrost::focus::FocusState;
use hoarfrost::surface::menu::cascade::{MenuCascade, MenuTreeKind};
use hoarfrost::surface::menu::model::{Row, submenu_id};

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

// ---------------------------------------------------------------------------
// End-to-end pixel check — the card the application actually mounts
// ---------------------------------------------------------------------------

/// The scale this file's pixel checks render at: two physical pixels per
/// logical one, so a glyph's landing point resolves to half a logical
/// pixel rather than being rounded away.
const PIXEL_SCALE: f32 = 2.0;

/// A channel value above which a pixel counts as ink — well above the
/// darkest card fill this crate paints (`surface_variant` resolves to
/// `pro.surface_raised`), well below any text colour it resolves, so a
/// background never reads as a glyph.
const INK_THRESHOLD: u32 = 90;

/// Draw `element` once, headless, and return the RGBA framebuffer.
///
/// Re-implemented here rather than shared from `hoarfrost::test_helpers`
/// because that module is `#[cfg(test)]` to its own crate and cannot be
/// reached from this one. Deliberately minimal: one draw, one screenshot.
fn shoot(size: Size, element: Element<'static, Message>) -> Vec<u8> {
    use iced::advanced::renderer::Headless as _;
    use iced_test::futures::futures::executor::block_on;
    use iced_test::renderer::Renderer;
    use iced_test::runtime::user_interface::{self, UserInterface};

    let mut renderer = block_on(<Renderer as iced::advanced::renderer::Headless>::new(
        iced::Font::with_name("Fira Sans"),
        iced::Pixels(16.0),
        None,
    ))
    .expect("a headless renderer should be available for menu card tests");

    let mut ui: UserInterface<'static, Message, iced::Theme, Renderer> = UserInterface::build(
        element,
        size,
        user_interface::Cache::default(),
        &mut renderer,
    );

    ui.draw(
        &mut renderer,
        &iced::Theme::Dark,
        &iced::advanced::renderer::Style {
            text_color: iced::Color::WHITE,
        },
        iced::mouse::Cursor::Unavailable,
    );

    renderer.screenshot(
        Size::new(
            (size.width * PIXEL_SCALE).round() as u32,
            (size.height * PIXEL_SCALE).round() as u32,
        ),
        PIXEL_SCALE,
        iced::Color::BLACK,
    )
}

/// The vertical band of ink inside the logical rectangle `(x0,y0)..(x1,y1)`,
/// in logical pixels, or `None` when that region carries none.
///
/// Bounded in **both** axes on purpose. A y-only scan is satisfied by the
/// card's own bright perimeter stroke (`border_strong` is (120,120,120),
/// well over the ink threshold), which runs down the left and right edges
/// through every row band — so a y-only check reports "painted" for a
/// card whose labels are entirely missing.
fn ink_in_rect(rgba: &[u8], width: u32, x0: f32, y0: f32, x1: f32, y1: f32) -> Option<(f32, f32)> {
    let height = (rgba.len() as u64 / 4 / u64::from(width)) as u32;
    let top = ((y0 * PIXEL_SCALE).floor() as i64).clamp(0, height as i64) as u32;
    let bottom = ((y1 * PIXEL_SCALE).ceil() as i64).clamp(0, height as i64) as u32;
    let left = ((x0 * PIXEL_SCALE).floor() as i64).clamp(0, width as i64) as u32;
    let right = ((x1 * PIXEL_SCALE).ceil() as i64).clamp(0, width as i64) as u32;

    let mut first = u32::MAX;
    let mut last = 0u32;

    for y in top..bottom {
        for x in left..right {
            let i = ((y * width + x) * 4) as usize;
            if i + 3 >= rgba.len() {
                continue;
            }
            let peak = u32::from(rgba[i].max(rgba[i + 1]).max(rgba[i + 2]));
            if peak > INK_THRESHOLD {
                first = first.min(y);
                last = last.max(y);
            }
        }
    }

    if first == u32::MAX || last < first {
        return None;
    }
    Some((first as f32 / PIXEL_SCALE, (last + 1) as f32 / PIXEL_SCALE))
}

/// The application's own menu card paints a label inside every row rect
/// the geometry solver reports for it.
///
/// The framework-side tests prove the row helper paints; this proves the
/// card this crate actually mounts does, through [`card_element`] with the
/// real palette and chrome. It is the difference between "the widget is
/// correct" and "the menu the user opens shows its items".
///
/// Before `TypeRung::iced_line_height` existed this frame was blank. Each
/// row's paragraph was clipped to the row's own height while its glyphs
/// were being laid out `line_height × size` pixels down the box, so no
/// label reached the framebuffer at all.
#[test]
fn test_the_app_menu_card_paints_every_row_inside_its_own_rect() {
    let menu = menu_with(&["alpha", "beta", "gamma"]);
    let cascade = opened(&menu);
    let card = cascade.open_cards().first().expect("one open card");
    let size = card_size(&menu);
    let rect = Rectangle::new(Point::ORIGIN, size);
    let element: Element<'static, Message> = card_element(card, &cascade, rect);

    let rgba = shoot(size, element);
    let width = (size.width * PIXEL_SCALE).round() as u32;

    for row in 0..3 {
        let r = row_rect(&menu, rect, row).expect("every row has a solved rect");
        let band = ink_in_rect(&rgba, width, r.x, r.y, r.x + r.width, r.y + r.height)
            .unwrap_or_else(|| {
                panic!(
                    "row {row} painted nothing inside its rect {r:?} — \
                     an empty band means the row's label was clipped away"
                )
            });

        assert!(
            band.0 >= r.y && band.1 <= r.y + r.height,
            "row {row} ink landed at {band:?}, outside its rect {r:?}"
        );
    }
}

#[test]
fn test_card_inset_is_the_framework_token_not_a_restated_number() {
    assert_eq!(
        CARD_INSET,
        hoarfrost::token::space::XS,
        "the card inset comes from the token, so it cannot drift from §4.8.1"
    );
}
