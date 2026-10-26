//! Unit tests for `empty_state` (`UIF-COMP-014`, `UIF-PRIN-004`).

use super::*;
use crate::token::color::Palette;
use crate::token::typography::{FAMILY_SANS, TITLE_SM, UI_COMPACT, Weight};

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
