//! Unit tests for the shared framework-menu card chrome and solved geometry.
//!
//! The render path itself (`card_element`, `overlay`) is exercised against a
//! live widget tree in
//! [`interaction_tests`](crate::ui_iced::interaction_tests); what is
//! unit-testable here is the **grid** every menu card is painted from, and
//! the open-card rectangle walk that feeds the `SurfaceRegistry`.

use super::*;
use crate::error_codes::menu as menu_codes;
use crate::ui_iced::menu_surface::log_capture::{Capture, with_capture};
use hoarfrost::focus::FocusState;
use hoarfrost::surface::menu::cascade::{MenuCascade, MenuTreeKind};
use hoarfrost::surface::menu::model::{MenuId, Row, submenu_id};

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

// ---------------------------------------------------------------------------
// Reproduction — the whole open chain, driven from a real trigger click
// ---------------------------------------------------------------------------
//
// Everything above this line tests a piece of the chain in isolation. What the
// bug report ("menus do not open") needs is the chain end to end: a real
// pointer press on a real trigger inside the real `view()`, the real reducer,
// and then a measurement of whether a card actually landed on screen and can
// be clicked. Each of those links can pass on its own while the chain is
// broken, so they are asserted here in order.

use std::path::PathBuf;

use crate::config::ContentLibrary;
use crate::ui::strings;
use crate::ui_iced::app::AppState;
use crate::ui_iced::context_menu::{self};
use crate::ui_iced::hamburger::{self, hamburger_menu};
use crate::ui_iced::test_helpers::test_app_state;
use crate::ui_iced::update::update;
use crate::ui_iced::view::view;
use iced_test::selector::Candidate;

/// A headless iced driver over the app's `Message`.
type Sim<'a> = iced_test::Simulator<'a, Message, iced::Theme, iced::Renderer>;

/// Builds a driver over a rendered element.
fn sim(element: Element<'_, Message>) -> Sim<'_> {
    iced_test::simulator(element)
}

/// The text candidate whose content equals `label`.
fn text_bounds(label: &str) -> impl Fn(Candidate<'_>) -> Option<iced::Rectangle> {
    move |candidate: Candidate<'_>| match candidate {
        Candidate::Text {
            content, bounds, ..
        } if content == label => Some(bounds),
        _ => None,
    }
}

/// Points the cursor at `position` and left-clicks it.
fn click_at(ui: &mut Sim<'_>, position: iced::Point) {
    ui.point_at(position);
    let _ = ui.simulate([
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }),
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ]);
}

/// The viewport the reproduction renders at.
const VIEWPORT: f32 = 1024.0;

/// Clicking the ☰ trigger inside the real `view()` must open a menu that is
/// mounted, on-screen, and hit-testable.
///
/// Every link is asserted separately so a failure names the link that broke
/// rather than the symptom:
///
/// 1. the trigger publishes `MenuMessage::Opened`;
/// 2. the reducer drives the cascade to open and registers a solved rect;
/// 3. the next frame mounts a card whose bounds are non-zero and inside the
///    viewport;
/// 4. a second click on a row of that card produces that row's message.
#[test]
fn reproduction_hamburger_trigger_click_mounts_a_hit_testable_card() {
    let mut state = test_app_state();

    // ── Link 1: the trigger publishes the open request ──────────────────
    let opened = {
        let mut ui = sim(view(&state));
        let trigger = ui
            .find(text_bounds(strings::HAMBURGER_MENU_BUTTON))
            .expect("the ☰ hamburger trigger in the top bar");
        click_at(&mut ui, trigger.center());
        ui.into_messages().collect::<Vec<_>>()
    };
    assert!(
        opened.contains(&Message::Menu(MenuMessage::Opened)),
        "link 1 broken: clicking ☰ must publish Menu(Opened), got {opened:?}"
    );

    // ── Link 2: the reducer opens the cascade and solves a rect ─────────
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    state.rebuild_view_models();
    assert!(
        state.overlay.hamburger_open,
        "link 2 broken: Opened did not leave the hamburger cascade open"
    );
    assert_eq!(
        state.overlay.hamburger_rects.len(),
        1,
        "link 2 broken: an open tree must register exactly one root rect"
    );
    let root_rect = state.overlay.hamburger_rects[0];
    assert!(
        root_rect.width > 0.0 && root_rect.height > 0.0,
        "link 2 broken: the solved root rect is degenerate: {root_rect:?}"
    );

    // ── Link 3: the next frame mounts the card on screen ────────────────
    let mut ui = sim(view(&state));
    let row = ui
        .find(text_bounds(strings::MENU_BATCH))
        .expect("link 3 broken: the mounted card paints no row text");
    assert!(
        row.width > 0.0 && row.height > 0.0,
        "link 3 broken: the row's bounds are zero: {row:?}"
    );
    assert!(
        row.x >= 0.0
            && row.y >= 0.0
            && row.x + row.width <= VIEWPORT
            && row.y + row.height <= VIEWPORT,
        "link 3 broken: the row landed outside the viewport: {row:?}"
    );

    // ── Link 4: that row is hit-testable ────────────────────────────────
    click_at(&mut ui, row.center());
    let emitted = ui.into_messages().collect::<Vec<_>>();
    assert!(
        emitted.contains(&Message::Menu(MenuMessage::RowPicked {
            card: hoarfrost::surface::menu::model::MenuId::new(
                crate::ui_iced::hamburger::HAMBURGER_MENU_ID
            ),
            row: 0,
        })),
        "link 4 broken: clicking the mounted row produced no RowPicked: {emitted:?}"
    );
}

/// Hovering a submenu parent inside the real `view()` must open the branch.
///
/// The hamburger's submenu parents open on **hover**, not on click, so the
/// chain under test here is the one no existing test drives end to end:
/// the row widget reporting the crossing, the reducer feeding the aiming
/// corridor, the cascade mounting the child card, and the next frame
/// painting that child where the pointer can reach it.
#[test]
fn reproduction_hovering_a_submenu_parent_opens_the_branch() {
    let mut state = test_app_state();
    let _ = update(&mut state, Message::Menu(MenuMessage::Opened));
    state.rebuild_view_models();

    let root =
        hoarfrost::surface::menu::model::MenuId::new(crate::ui_iced::hamburger::HAMBURGER_MENU_ID);

    // ── Link A: the row reports the hover crossing ──────────────────────
    let entered = {
        let mut ui = sim(view(&state));
        let parent = ui
            .find(text_bounds(strings::MENU_WINDOWS))
            .expect("the Windows submenu row on the root card");
        let position = parent.center();
        ui.point_at(position);
        let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::CursorMoved {
            position,
        })]);
        ui.into_messages().collect::<Vec<_>>()
    };
    assert!(
        entered.iter().any(|message| matches!(
            message,
            Message::Menu(MenuMessage::RowEntered { card, row: _ }) if *card == root
        )),
        "link A broken: hovering the submenu parent reported no RowEntered: {entered:?}"
    );

    // ── Link B: the reducer mounts the child card ───────────────────────
    for message in entered {
        let _ = update(&mut state, message);
    }
    state.rebuild_view_models();
    assert_eq!(
        state.overlay.hamburger_cascade.depth(),
        2,
        "link B broken: the hover did not open the submenu branch (depth {})",
        state.overlay.hamburger_cascade.depth()
    );
    assert_eq!(
        state.overlay.hamburger_rects.len(),
        2,
        "link B broken: the child card registered no solved rect"
    );

    // ── Link C: the child card is painted where the pointer can reach it ─
    let mut ui = sim(view(&state));
    let child = ui
        .find(text_bounds(strings::MENU_BACKGROUND_OPERATIONS))
        .expect("link C broken: the submenu card paints no rows");
    assert!(
        child.width > 0.0 && child.height > 0.0,
        "link C broken: the submenu row's bounds are zero: {child:?}"
    );
    assert!(
        child.x >= 0.0
            && child.y >= 0.0
            && child.x + child.width <= VIEWPORT
            && child.y + child.height <= VIEWPORT,
        "link C broken: the submenu row landed outside the viewport: {child:?}"
    );
}

/// The opening click must not also feed the click-away guard.
///
/// The reproduction above hand-feeds `Menu(Opened)` to the reducer. That
/// hides the self-close class of bug: if the very same gesture also emits
/// `Menu(Closed)` — because the release lands on the full-window blocker
/// the open just mounted — the tree is open and shut again before it is
/// ever painted, and the user sees a ☰ that does nothing.
///
/// So: drive the real trigger, feed **every** message the click produced
/// into the real reducer, in order, and assert the menu survived the whole
/// batch.
#[test]
fn reproduction_the_opening_click_does_not_also_close_the_menu() {
    let mut state = test_app_state();

    let emitted = {
        let mut ui = sim(view(&state));
        let trigger = ui
            .find(text_bounds(strings::HAMBURGER_MENU_BUTTON))
            .expect("the ☰ hamburger trigger in the top bar");
        click_at(&mut ui, trigger.center());
        ui.into_messages().collect::<Vec<_>>()
    };

    for message in emitted.clone() {
        let _ = update(&mut state, message);
    }
    state.rebuild_view_models();

    assert!(
        state.overlay.hamburger_open,
        "self-close broken: the opening gesture emitted {emitted:?} and left \
         the hamburger closed"
    );
    assert_eq!(
        state.overlay.hamburger_rects.len(),
        1,
        "self-close broken: the menu is open but registered no card to paint"
    );
}

/// The text candidate whose content contains `needle`.
fn text_containing(needle: &str) -> impl Fn(Candidate<'_>) -> Option<iced::Rectangle> + '_ {
    move |candidate: Candidate<'_>| match candidate {
        Candidate::Text {
            content, bounds, ..
        } if content.contains(needle) => Some(bounds),
        _ => None,
    }
}

/// A `AppState` whose content library is a temp dir holding one markdown
/// file, with the library root expanded so the file renders as a tree row
/// ([RUST-006]).
fn state_with_one_file() -> (AppState, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().to_string_lossy().to_string();

    let mut state = test_app_state();
    state.app.content_libraries_mut().push(ContentLibrary {
        root_folder: root.clone(),
        name: "System".to_string(),
        kind: "system".to_string(),
        readonly: false,
        priority: 0,
    });
    state
        .app
        .workspace_mut()
        .all_files
        .push(PathBuf::from(format!("{root}/alpha.md")));
    state
        .app
        .selection_mut()
        .expanded_dirs
        .insert(PathBuf::from(&root));
    state.rebuild_view_models();
    (state, dir)
}

/// The same self-close check on the right-click context-menu lane.
#[test]
fn reproduction_the_right_click_that_opens_a_context_menu_does_not_also_close_it() {
    let (mut state, _dir) = state_with_one_file();
    let path = state.models.left.rows[1].path.clone();

    let emitted = {
        let mut ui = sim(view(&state));
        let row = ui.find(text_containing("alpha.md")).expect("a tree row");
        let position = row.center();
        ui.point_at(position);
        let _ = ui.simulate([
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }),
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                iced::mouse::Button::Right,
            )),
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                iced::mouse::Button::Right,
            )),
        ]);
        ui.into_messages().collect::<Vec<_>>()
    };

    for message in emitted.clone() {
        let _ = update(&mut state, message);
    }
    state.rebuild_view_models();

    assert!(
        state.overlay.context_menu.is_some(),
        "self-close broken: right-clicking {path:?} emitted {emitted:?} and \
         left no context menu mounted"
    );
    assert!(
        !state
            .overlay
            .context_menu
            .as_ref()
            .unwrap()
            .rects
            .is_empty(),
        "self-close broken: the context menu is open but registered no card \
         to paint"
    );
}

/// A context menu anchored anywhere the user can right-click must solve its
/// card fully inside the viewport.
///
/// The anchor is the raw cursor point, so the solve has to flip and clamp on
/// both axes: right-clicking near the right or bottom edge is the common
/// case, and a card that lands off-screen there is another way the user ends
/// up seeing "the menu just doesn't open".
#[test]
fn test_the_context_menu_card_stays_inside_the_viewport_at_every_anchor() {
    let (state, _dir) = state_with_one_file();
    let rows = state.models.left.menu_items_for(&state.models.left.rows[1]);
    assert!(
        !rows.is_empty(),
        "the fixture file row must produce context-menu rows"
    );
    let menu = context_menu::menu_from_rows(MenuId::new("context.file.alpha"), &rows);
    let cascade = opened(&menu);

    let viewport = Size::new(1280.0, 800.0);
    let view = Rectangle::new(Point::ORIGIN, viewport);

    // Every corner, every edge midpoint, and the centre.
    let anchors = [
        Point::new(0.0, 0.0),
        Point::new(1279.0, 0.0),
        Point::new(0.0, 799.0),
        Point::new(1279.0, 799.0),
        Point::new(640.0, 0.0),
        Point::new(0.0, 400.0),
        Point::new(1279.0, 400.0),
        Point::new(640.0, 799.0),
        Point::new(640.0, 400.0),
    ];

    for anchor in anchors {
        let rects = context_menu::open_card_rects(&cascade, anchor, viewport);
        assert_eq!(
            rects.len(),
            1,
            "an open context card must solve exactly one rect at {anchor:?}"
        );
        let rect = rects[0];
        assert!(
            rect.width.is_finite() && rect.height.is_finite(),
            "the context solve produced a non-finite rect at {anchor:?}: {rect:?}"
        );
        assert!(
            rect.x >= 0.0 && rect.y >= 0.0,
            "the context card solved to a negative origin at {anchor:?}: {rect:?}"
        );
        assert!(
            rect.x + rect.width <= view.width + 0.001
                && rect.y + rect.height <= view.height + 0.001,
            "the context card solved outside the viewport at {anchor:?}: {rect:?}"
        );
    }
}

/// The hamburger's solved root card must stay fully inside the viewport at
/// every window size the app can actually open at.
///
/// The card is right-anchored to a **synthesised** navbar rectangle
/// ([`hamburger::hamburger_trigger_rect`]) derived from `state.viewport`,
/// not from the measured button. If that solve ever leaves the card off the
/// right or bottom edge the user sees the ☰ do nothing at all, which is
/// indistinguishable from "the menu is broken".
#[test]
fn test_the_root_card_stays_inside_the_viewport_at_every_window_size() {
    let menu = hamburger_menu(&test_app_state().models.top);
    let cascade = opened(&menu);

    // Includes the real persisted window from `%APPDATA%\fastmd\ui_state.json`
    // (2188.8 x 1200.8) alongside sizes that squeeze the solve from
    // both axes.
    let viewports = [
        (2188.8, 1200.8),
        (1920.0, 1080.0),
        (1280.0, 800.0),
        (1024.0, 768.0),
        (800.0, 600.0),
        (640.0, 480.0),
        (420.0, 900.0),
        (900.0, 260.0),
        (300.0, 300.0),
    ];

    for (width, height) in viewports {
        let viewport = Size::new(width, height);
        let rects = hamburger::open_card_rects(&cascade, viewport);
        assert_eq!(
            rects.len(),
            1,
            "an open root card must solve exactly one rect at {viewport:?}"
        );
        let rect = rects[0];
        assert!(
            rect.width.is_finite() && rect.height.is_finite(),
            "the solve produced a non-finite rect at {viewport:?}: {rect:?}"
        );
        assert!(
            rect.x >= 0.0 && rect.y >= 0.0,
            "the card solved to a negative origin at {viewport:?}: {rect:?}"
        );
        assert!(
            rect.x + rect.width <= width + 0.001 && rect.y + rect.height <= height + 0.001,
            "the card solved outside the viewport at {viewport:?}: {rect:?}"
        );
    }
}

// ------------------------- observability (NFR-001 / NFR-004) -----------------
//
// Every route below is a failure that used to be silent. The tests force the
// failure and assert the [NFR-004] code and the [NFR-001] level arrive, with
// enough context on the line to identify the cause without a debugger.

/// The viewport every observability test pins its context against.
const OBS_VIEWPORT: (f32, f32) = (1200.0, 800.0);

fn obs_viewport() -> Size {
    Size::new(OBS_VIEWPORT.0, OBS_VIEWPORT.1)
}

#[test]
fn test_card_drop_reasons_carry_distinct_error_codes() {
    let codes = [
        CardDropReason::ParentRectMissing.error_code(),
        CardDropReason::ParentMenuMissing.error_code(),
        CardDropReason::RowRectMissing.error_code(),
    ];
    let mut unique = codes.to_vec();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        codes.len(),
        "each card-drop reason must have its own code, got {codes:?}"
    );
    assert_eq!(codes[0], menu_codes::CARD_PARENT_RECT_MISSING);
    assert_eq!(codes[1], menu_codes::CARD_PARENT_MENU_MISSING);
    assert_eq!(codes[2], menu_codes::CARD_ROW_RECT_MISSING);
    for code in codes {
        assert!(
            code.starts_with("MENU-3"),
            "card-geometry routes live in the MENU-3xxx block, got {code}"
        );
    }
}

#[test]
fn test_log_card_dropped_reports_error_with_full_context() {
    let capture = Capture::new();
    with_capture(&capture, || {
        log_card_dropped(
            &MenuId::new("surface.child"),
            &MenuId::new("surface.root"),
            7,
            CardDropReason::RowRectMissing,
            obs_viewport(),
            2,
        );
    });

    let event = capture
        .with_code(menu_codes::CARD_ROW_RECT_MISSING)
        .expect("the dropped card must report its code");
    assert_eq!(
        event.level,
        tracing::Level::ERROR,
        "a card that opened but cannot paint is a failure ([NFR-001])"
    );
    assert_eq!(event.field("menu"), Some("surface.child"));
    assert_eq!(event.field("parent"), Some("surface.root"));
    assert_eq!(event.field("row"), Some("7"));
    assert_eq!(event.field("open_cards"), Some("2"));
    assert_eq!(event.field("viewport_w"), Some("1200.0"));
    assert_eq!(event.field("viewport_h"), Some("800.0"));
    assert_eq!(
        event.field("detail"),
        Some("trigger row index is past the end of the parent menu")
    );
    assert_eq!(event.field("trigger"), Some("menu.open_card_rects"));
}

#[test]
fn test_report_rect_alignment_is_silent_and_true_when_lists_match() {
    let menu = menu_with_submenu(&["a", "b"]);
    let cascade = opened(&menu);
    let capture = Capture::new();
    let aligned = with_capture(&capture, || {
        report_rect_alignment(
            cascade.open_cards(),
            &[Rectangle::new(Point::ORIGIN, Size::new(224.0, 90.0))],
        )
    });
    assert!(aligned, "equal-length lists are aligned");
    assert!(
        capture.codes().is_empty(),
        "an aligned rect list must not log, got {:?}",
        capture.codes()
    );
}

#[test]
fn test_report_rect_alignment_reports_error_on_shortfall() {
    let menu = menu_with_submenu(&["a", "b"]);
    let cascade = opened(&menu);
    let capture = Capture::new();
    let aligned = with_capture(&capture, || {
        report_rect_alignment(cascade.open_cards(), &[])
    });
    assert!(!aligned, "an empty rect list cannot cover an open card");
    let event = capture
        .with_code(menu_codes::RECT_COUNT_MISMATCH)
        .expect("a rect shortfall must report its code");
    assert_eq!(
        event.level,
        tracing::Level::ERROR,
        "misaligned rects paint cards at the wrong place: a failure, not a warning"
    );
    assert_eq!(event.field("rects"), Some("0"));
    assert!(
        event.field("open_cards").is_some_and(|count| count != "0"),
        "the line must report how many cards were left unpainted"
    );
}

#[test]
fn test_open_card_rects_is_silent_on_the_happy_path() {
    // Guards against the diagnostics becoming spam: a cascade that solves
    // cleanly must not emit a single MENU-3xxx code.
    let menu = menu_with_submenu(&["a", "b"]);
    let cascade = opened(&menu);
    let capture = Capture::new();
    let rects = with_capture(&capture, || {
        open_card_rects(&cascade, obs_viewport(), |_| {
            placement(Rectangle::new(Point::ORIGIN, Size::new(224.0, 90.0)))
        })
    });
    assert_eq!(rects.len(), cascade.open_cards().len());
    assert!(
        capture.codes().is_empty(),
        "a clean solve must log no error codes, got {:?}",
        capture.codes()
    );
}

#[test]
fn test_safe_menu_id_redacts_the_path_bearing_tail_of_a_context_id() {
    // `context_menu::root_id` builds ids as `context.<kind>.<path>`, so the
    // id itself is the PII carrier [NFR-009].
    let marker = "secret-person";
    for kind in ["dir", "file", "multi", "tab"] {
        let id = MenuId::new(format!(
            "{}.{kind}.C:\\Users\\{marker}\\notes",
            crate::ui_iced::context_menu::CONTEXT_MENU_ID_PREFIX
        ));
        let safe = safe_menu_id(&id);
        assert!(
            !safe.contains(marker),
            "a context-menu id must never render the user's path, got {safe}"
        );
        assert!(
            safe.contains(&format!(".{kind}.")),
            "the kind segment must survive so the builder is still identifiable, got {safe}"
        );
        assert!(
            safe.ends_with("<redacted>"),
            "the redaction must be explicit rather than a silent truncation, got {safe}"
        );
    }
}

#[test]
fn test_safe_menu_id_passes_non_path_ids_through_untouched() {
    // Hamburger ids are static and *are* the diagnostic; redacting them would
    // cost the log its only identifying information.
    for raw in ["hamburger", "hamburger.windows", "surface.test"] {
        assert_eq!(safe_menu_id(&MenuId::new(raw)), raw);
    }
}

#[test]
fn test_log_card_dropped_never_prints_a_context_path() {
    let marker = "secret-person";
    let prefix = crate::ui_iced::context_menu::CONTEXT_MENU_ID_PREFIX;
    let capture = Capture::new();
    with_capture(&capture, || {
        log_card_dropped(
            &MenuId::new(format!("{prefix}.file.C:\\Users\\{marker}\\notes.md")),
            &MenuId::new(format!("{prefix}.dir.C:\\Users\\{marker}")),
            3,
            CardDropReason::RowRectMissing,
            obs_viewport(),
            2,
        );
    });
    let event = capture
        .with_code(menu_codes::CARD_ROW_RECT_MISSING)
        .expect("the drop must report its code");
    let rendered = format!(
        "{} {:?}",
        event.message,
        capture
            .events()
            .iter()
            .flat_map(|e| e.fields.iter())
            .collect::<Vec<_>>()
    );
    assert!(
        !rendered.contains(marker),
        "the card-drop line must not carry the user's path, got {rendered}"
    );
    assert_eq!(
        event.field("menu"),
        Some(format!("{prefix}.file.<redacted>").as_str())
    );
    assert_eq!(
        event.field("parent"),
        Some(format!("{prefix}.dir.<redacted>").as_str())
    );
}

#[test]
fn test_menu_codes_come_from_the_menu_surface_module() {
    // Proves the emitter is the module under test, so a code cannot be
    // satisfied by an unrelated log line that happens to reuse it.
    let capture = Capture::new();
    with_capture(&capture, || {
        report_rect_alignment(&[], &[Rectangle::new(Point::ORIGIN, Size::new(1.0, 1.0))]);
    });
    assert_eq!(
        capture.targets(),
        vec!["fastmd::ui_iced::menu_surface"],
        "the rect-invariant line must be emitted by menu_surface"
    );
}
