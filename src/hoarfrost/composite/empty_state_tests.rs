//! Unit tests for `empty_state` (`UIF-COMP-014`, `UIF-PRIN-004`).

use super::*;
use crate::test_helpers::Harness;
use crate::token::color::Palette;
use crate::token::typography::{FAMILY_SANS, TITLE_SM, UI_COMPACT, Weight};
use iced::Point;

#[test]
fn test_empty_state_kind_variants_and_error_detection() {
    let no_items = EmptyStateKind::NoItems;
    assert!(!no_items.is_error());

    let no_matches = EmptyStateKind::NoMatches {
        query: "report".to_string(),
    };
    assert!(!no_matches.is_error());

    let load_error = EmptyStateKind::LoadError {
        code: "HTTP-503".to_string(),
    };
    assert!(load_error.is_error());
}

#[test]
fn test_icon_construction_and_deref() {
    let icon1 = Icon::new("folder-open");
    assert_eq!(&*icon1, "folder-open");

    let icon2: Icon = "search".into();
    assert_eq!(&*icon2, "search");

    let icon3: Icon = String::from("alert-triangle").into();
    assert_eq!(&*icon3, "alert-triangle");
}

#[test]
fn test_model_construction_with_and_without_action() {
    let model = EmptyStateModel::new(
        EmptyStateKind::NoItems,
        "inbox",
        "No notifications",
        "You're all caught up for today.",
    );
    assert_eq!(model.kind, EmptyStateKind::NoItems);
    assert_eq!(model.icon.0, "inbox");
    assert_eq!(model.title, "No notifications");
    assert_eq!(model.description, "You're all caught up for today.");
    assert!(model.primary_action.is_none());

    let model_with_action = model.with_action("Refresh", 42);
    assert_eq!(
        model_with_action.primary_action,
        Some(("Refresh".to_string(), 42))
    );
}

#[test]
fn test_geometry_and_token_alignment() {
    assert_eq!(MAX_WIDTH, 360.0, "max_width must be 360px per §3B.14");
    assert_eq!(PADDING, 24.0, "padding must be 24px (space.xl) per §3B.14");
    assert_eq!(BADGE_SIZE, 40.0, "badge size must be 40x40px per §3B.14");
    assert_eq!(
        BADGE_RADIUS, 6.0,
        "badge radius must be radius.card per §3B.14"
    );
    assert_eq!(
        BADGE_BORDER_WIDTH, 1.0,
        "badge border must be 1px hairline per §3B.14"
    );
    assert_eq!(
        ICON_SIZE, 20.0,
        "icon size must be size.icon.lg (20px) per §3B.14"
    );
}

#[test]
fn test_typography_rungs() {
    assert_eq!(TITLE_SM.size, 16.0, "headline must be 16px");
    assert_eq!(UI_COMPACT.size, 13.0, "explanation must be 13px");

    let title_font = TITLE_SM.font(FAMILY_SANS, Weight::SemiBold);
    assert_eq!(title_font.weight, iced::font::Weight::Semibold);

    let desc_font = UI_COMPACT.font(FAMILY_SANS, Weight::Regular);
    assert_eq!(desc_font.weight, iced::font::Weight::Normal);
}

#[test]
fn test_color_resolution_for_standard_vs_error() {
    let palette = Palette::dark();

    let normal_style = resolve_style(&EmptyStateKind::NoItems, &palette);
    assert_eq!(normal_style.badge_background, palette.surface_raised);
    assert_eq!(normal_style.badge_border_color, palette.border_subtle);
    assert_eq!(normal_style.icon_color, palette.text_muted);
    assert_eq!(normal_style.title_color, palette.text_strong);
    assert_eq!(normal_style.description_color, palette.text_muted);

    let match_style = resolve_style(
        &EmptyStateKind::NoMatches {
            query: "rust".to_string(),
        },
        &palette,
    );
    assert_eq!(match_style.icon_color, palette.text_muted);

    let error_style = resolve_style(
        &EmptyStateKind::LoadError {
            code: "NET-404".to_string(),
        },
        &palette,
    );
    assert_eq!(error_style.icon_color, palette.status_danger_text);
    assert_eq!(error_style.badge_background, palette.surface_raised);
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestMsg {
    TriggerAction(usize),
}

#[test]
fn test_widget_creation_and_helper() {
    let model = EmptyStateModel::new(
        EmptyStateKind::NoItems,
        "empty",
        "Nothing here",
        "Create an item to get started",
    )
    .with_action("Create", 99);

    let widget: EmptyState<'_, TestMsg, usize> = EmptyState::new(model.clone(), |commit| {
        TestMsg::TriggerAction(commit.into_value())
    });

    assert_eq!(widget.model(), &model);

    let _element: Element<'_, TestMsg> =
        empty_state(model, |commit| TestMsg::TriggerAction(commit.into_value()));
}

// ---------------------------------------------------------------------------
// Model shapes the composite has to survive
// ---------------------------------------------------------------------------

#[test]
fn test_a_no_matches_state_carries_the_query_that_produced_it() {
    let kind = EmptyStateKind::NoMatches {
        query: String::new(),
    };
    assert!(
        !kind.is_error(),
        "an empty query that matched nothing is still an empty result, not a failure"
    );
    assert_eq!(
        resolve_style(&kind, &Palette::dark()).icon_color,
        Palette::dark().text_muted,
        "a zero-match state keeps the muted icon rather than the danger one"
    );
}

#[test]
fn test_an_icon_may_be_empty_without_breaking_the_badge() {
    let blank = Icon::new(String::new());
    assert_eq!(blank.len(), 0, "an empty icon renders an empty badge glyph");
    assert_eq!(&*blank, "");
}

#[test]
fn test_an_action_can_be_replaced_but_not_cleared_by_the_builder() {
    let model = EmptyStateModel::new(EmptyStateKind::NoItems, "inbox", "Empty", "Nothing to show")
        .with_action("Retry", 1usize)
        .with_action("Retry later", 2usize);

    assert_eq!(
        model.primary_action,
        Some(("Retry later".to_string(), 2)),
        "the last action attached is the one the CTA commits"
    );
}

// ---------------------------------------------------------------------------
// Harness helpers
// ---------------------------------------------------------------------------

fn no_items_model() -> EmptyStateModel<usize> {
    EmptyStateModel::new(
        EmptyStateKind::NoItems,
        "inbox",
        "No notifications",
        "You are all caught up.",
    )
}

fn error_model() -> EmptyStateModel<usize> {
    EmptyStateModel::new(
        EmptyStateKind::LoadError {
            code: "NET-503".to_string(),
        },
        "alert",
        "Could not load",
        "The service did not answer.",
    )
}

fn matches_model() -> EmptyStateModel<usize> {
    EmptyStateModel::new(
        EmptyStateKind::NoMatches {
            query: "widget".to_string(),
        },
        "search",
        "No matches",
        "Nothing matched your filter.",
    )
}

fn with_cta(model: EmptyStateModel<usize>) -> EmptyStateModel<usize> {
    model.with_action("Retry", 7)
}

fn cta(model: EmptyStateModel<usize>) -> Element<'static, TestMsg> {
    empty_state(model, |commit| TestMsg::TriggerAction(commit.into_value()))
}

/// Every box the operation stream publishes as a container, in traversal order.
fn containers<Message: Send + 'static, Renderer: renderer::Renderer>(
    harness: &mut Harness<'_, Message, Renderer>,
) -> Vec<Rectangle> {
    harness
        .candidates()
        .into_iter()
        .filter(|candidate| candidate.kind == crate::test_helpers::KIND_CONTAINER)
        .map(|candidate| candidate.bounds)
        .collect()
}

fn click<Message: Send + 'static, Renderer: renderer::Renderer>(
    harness: &mut Harness<'_, Message, Renderer>,
    at: Point,
) -> Vec<Message> {
    harness.press(at);
    harness.release(at)
}

// ---------------------------------------------------------------------------
// Layout, paint and the recovery CTA
// ---------------------------------------------------------------------------

#[test]
fn test_the_badge_is_laid_out_at_its_fixed_40px_box() {
    for model in [
        no_items_model(),
        matches_model(),
        error_model(),
        with_cta(no_items_model()),
    ] {
        let mut harness = Harness::<TestMsg, _>::headless(move || cta(model.clone()));
        harness.render();

        let badge = containers(&mut harness)
            .into_iter()
            .filter(|box_| {
                (box_.width - BADGE_SIZE).abs() < 0.5 && (box_.height - BADGE_SIZE).abs() < 0.5
            })
            .count();
        assert_eq!(
            badge, 1,
            "every empty state paints exactly one 40x40 badge, whatever its kind"
        );
    }
}

#[test]
fn test_the_cta_adds_exactly_one_more_box_and_commits_its_action() {
    let mut plain = Harness::<TestMsg, _>::headless(|| cta(no_items_model()));
    let mut actioned = Harness::<TestMsg, _>::headless(|| cta(with_cta(no_items_model())));

    let plain_boxes = containers(&mut plain);
    let actioned_boxes = containers(&mut actioned);
    assert_eq!(
        actioned_boxes.len(),
        plain_boxes.len() + 1,
        "the recovery CTA contributes exactly one extra interactive box"
    );

    // The CTA is the last child pushed to the column, so it is the last box
    // the operation stream reports.
    let cta_box = *actioned_boxes
        .last()
        .expect("the actioned empty state must report its CTA box");
    assert_eq!(
        click(&mut actioned, cta_box.center()),
        vec![TestMsg::TriggerAction(7)],
        "pressing the CTA commits the action id it was built with"
    );
}

#[test]
fn test_an_empty_state_without_a_cta_commits_nothing_when_clicked() {
    let mut harness = Harness::<TestMsg, _>::headless(|| cta(no_items_model()));
    let boxes = containers(&mut harness);

    // With no CTA the last box reported is the badge, which is not a button.
    let badge = *boxes.last().expect("the badge is always laid out");
    assert_eq!(
        click(&mut harness, badge.center()),
        Vec::<TestMsg>::new(),
        "clicking the placeholder body of a CTA-less empty state commits nothing"
    );
}

#[test]
fn test_a_cta_press_released_off_the_button_commits_nothing() {
    let mut harness = Harness::<TestMsg, _>::headless(|| cta(with_cta(error_model())));
    let cta_box = *containers(&mut harness)
        .last()
        .expect("the actioned empty state must report its CTA box");

    harness.press(cta_box.center());
    assert_eq!(
        harness.release(Point::new(cta_box.center().x, cta_box.center().y + 120.0)),
        Vec::<TestMsg>::new(),
        "dragging off the CTA before releasing abandons the recovery action"
    );
}

#[test]
fn test_the_placeholder_fills_its_panel_rather_than_collapsing() {
    let widget: EmptyState<'_, TestMsg, usize> = EmptyState::new(error_model(), |commit| {
        TestMsg::TriggerAction(commit.into_value())
    });

    let size = Widget::size(&widget);
    assert_eq!(
        size.width,
        Length::Fill,
        "UIF-PRIN-004: the placeholder claims the panel's width so it cannot collapse"
    );
    assert_eq!(
        size.height,
        Length::Fill,
        "UIF-PRIN-004: the placeholder claims the panel's height so it cannot collapse"
    );
}

#[test]
fn test_every_kind_lays_out_and_paints_through_the_real_renderer() {
    for model in [
        no_items_model(),
        matches_model(),
        error_model(),
        with_cta(matches_model()),
        with_cta(error_model()),
    ] {
        let painted_model = model.clone();
        let mut harness = Harness::<TestMsg, _>::headless(move || cta(painted_model.clone()));
        harness.render();
        assert!(
            !containers(&mut harness).is_empty(),
            "{:?} must lay out and paint its placeholder box",
            model.kind
        );
    }
}

#[test]
fn test_a_long_description_still_paints_inside_the_max_width() {
    let model = EmptyStateModel::new(
        EmptyStateKind::NoMatches {
            query: "a-very-long-filter-expression-that-will-not-fit-on-one-line".to_string(),
        },
        "search",
        "No matches for this query",
        "Try a shorter search term, or clear the active filters to see everything again \
         in the panel, which may take a moment to reload from the workspace index.",
    )
    .with_action("Clear filters", 3);

    let mut harness = Harness::<TestMsg, _>::headless(move || cta(model.clone()));
    harness.render();

    let widest = containers(&mut harness)
        .into_iter()
        .fold(0.0f32, |widest, box_| widest.max(box_.width));
    assert!(
        widest <= MAX_WIDTH + 0.5,
        "the placeholder never grows past its {MAX_WIDTH}px cap, got {widest}px"
    );
}

#[test]
fn test_children_and_diff_delegate_to_the_one_content_tree() {
    let mut widget: EmptyState<'_, TestMsg, usize> =
        EmptyState::new(with_cta(no_items_model()), |commit| {
            TestMsg::TriggerAction(commit.into_value())
        });
    let mut tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    assert_eq!(
        tree.children.len(),
        1,
        "the placeholder wraps exactly one content child"
    );

    widget.diff(&mut tree);
    assert_eq!(
        tree.children.len(),
        1,
        "diffing against the same content keeps the child tree rather than rebuilding it"
    );

    // The layout has to come from a real `layout()` pass: the placeholder's
    // content is an iced `container`, and `container::overlay` unwraps its
    // child's layout node — a hand-made `Node::new` with no children panics
    // inside iced before the placeholder's own forwarding is ever reached.
    let renderer = crate::test_helpers::headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(360.0, 240.0));
    let node = widget.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);
    let viewport = Rectangle::new(Point::new(0.0, 0.0), node.size());

    assert!(
        widget
            .overlay(&mut tree, layout, &renderer, &viewport, Vector::ZERO)
            .is_none(),
        "the placeholder forwards overlay() down the real layout and nothing it \
         holds — headline, explanation or CTA — has a flyout to pop out"
    );
}
