//! Unit and interaction tests for the reusable menu widget.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Run,
}

fn id(value: &'static str) -> MenuItemId {
    MenuItemId::new(value)
}

fn viewport() -> Size {
    Size::new(800.0, 600.0)
}

fn point_position(point: Point) -> MenuPosition {
    MenuPosition::Point {
        point,
        horizontal: HorizontalPreference::Right,
        vertical: VerticalPreference::Below,
    }
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
    Rectangle {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn constructors_keep_labels_and_default_rows_enabled() {
    let action = MenuItem::action("Run", Action::Run);
    let toggle = MenuItem::<Action>::toggle(id("toggle"), "Verbose", true);
    let radio = MenuItem::<Action>::radio(id("radio"), "Balanced", false);
    let submenu = MenuItem::<Action>::submenu(id("child"), "More", Vec::new());
    let disabled = MenuItem::<Action>::disabled("Unavailable");
    let heading = MenuItem::<Action>::heading("Group");
    let separator = MenuItem::<Action>::separator();

    assert_eq!(action.label(), Some("Run"));
    assert_eq!(toggle.label(), Some("Verbose"));
    assert_eq!(radio.label(), Some("Balanced"));
    assert_eq!(submenu.label(), Some("More"));
    assert_eq!(disabled.label(), Some("Unavailable"));
    assert_eq!(heading.label(), Some("Group"));
    assert_eq!(separator.label(), None);
    assert_eq!(id("stable-id").as_str(), "stable-id");
}

#[test]
fn explicit_disabled_action_has_no_activation() {
    let item = MenuItem::action_enabled("Disabled", Action::Run, false);

    assert!(matches!(item, MenuItem::Action { enabled: false, .. }));
}

#[test]
fn menu_size_accounts_for_padding_spacing_headings_and_separators() {
    let style = MenuStyle::default();
    let items = vec![
        MenuItem::<Action>::heading("Actions"),
        MenuItem::action("Run", Action::Run),
        MenuItem::separator(),
        MenuItem::disabled("Unavailable"),
    ];

    let size = menu_size(&items, style);
    assert_eq!(size.width, style.width);
    assert_eq!(
        size.height,
        style.padding * 2.0
            + style.heading_height
            + style.row_height
            + style.separator_height
            + style.row_height
            + style.row_spacing * 3.0
    );
}

#[test]
fn empty_menu_has_only_card_padding() {
    let style = MenuStyle::default();

    assert_eq!(
        menu_size::<Action>(&[], style),
        Size::new(style.width, style.padding * 2.0)
    );
}

#[test]
fn menu_size_normalizes_invalid_style_metrics() {
    let style = MenuStyle {
        width: -1.0,
        row_height: -2.0,
        heading_height: f32::NAN,
        separator_height: f32::INFINITY,
        padding: -3.0,
        row_spacing: -4.0,
        anchor_gap: -5.0,
    };
    let items = vec![
        MenuItem::<Action>::heading("Group"),
        MenuItem::separator(),
        MenuItem::action("Run", Action::Run),
    ];

    assert_eq!(menu_size(&items, style), Size::new(0.0, 0.0));
}

#[test]
fn default_point_placement_opens_right_and_below() {
    let resolved = resolve_menu_position(
        point_position(Point::new(20.0, 30.0)),
        Size::new(120.0, 60.0),
        viewport(),
        4.0,
    );

    assert_eq!(resolved, Point::new(24.0, 34.0));
}

#[test]
fn explicit_point_placement_opens_left_and_above() {
    let resolved = resolve_menu_position(
        MenuPosition::Point {
            point: Point::new(300.0, 250.0),
            horizontal: HorizontalPreference::Left,
            vertical: VerticalPreference::Above,
        },
        Size::new(120.0, 60.0),
        viewport(),
        4.0,
    );

    assert_eq!(resolved, Point::new(176.0, 186.0));
}

#[test]
fn point_auto_flips_to_left_and_above_when_preferred_sides_do_not_fit() {
    let resolved = resolve_menu_position(
        MenuPosition::Point {
            point: Point::new(790.0, 590.0),
            horizontal: HorizontalPreference::Auto,
            vertical: VerticalPreference::Auto,
        },
        Size::new(120.0, 60.0),
        viewport(),
        4.0,
    );

    assert_eq!(resolved, Point::new(666.0, 526.0));
}

#[test]
fn point_auto_chooses_roomier_side_if_neither_side_fits() {
    let resolved = resolve_menu_position(
        MenuPosition::Point {
            point: Point::new(260.0, 200.0),
            horizontal: HorizontalPreference::Auto,
            vertical: VerticalPreference::Below,
        },
        Size::new(240.0, 40.0),
        Size::new(400.0, 400.0),
        8.0,
    );

    assert_eq!(resolved.x, 12.0);
    assert_eq!(resolved.y, 208.0);
}

#[test]
fn rectangle_anchor_places_right_with_start_alignment() {
    let resolved = resolve_menu_position(
        MenuPosition::Anchor {
            bounds: rect(100.0, 80.0, 30.0, 24.0),
            side: MenuSide::Right,
            alignment: MenuAlignment::Start,
            auto_flip: false,
        },
        Size::new(150.0, 80.0),
        viewport(),
        6.0,
    );

    assert_eq!(resolved, Point::new(136.0, 80.0));
}

#[test]
fn rectangle_anchor_places_left_with_end_alignment() {
    let resolved = resolve_menu_position(
        MenuPosition::Anchor {
            bounds: rect(300.0, 80.0, 30.0, 40.0),
            side: MenuSide::Left,
            alignment: MenuAlignment::End,
            auto_flip: false,
        },
        Size::new(120.0, 80.0),
        viewport(),
        6.0,
    );

    assert_eq!(resolved, Point::new(174.0, 40.0));
}

#[test]
fn rectangle_anchor_places_above_with_center_alignment() {
    let resolved = resolve_menu_position(
        MenuPosition::Anchor {
            bounds: rect(100.0, 300.0, 100.0, 40.0),
            side: MenuSide::Above,
            alignment: MenuAlignment::Center,
            auto_flip: false,
        },
        Size::new(120.0, 80.0),
        viewport(),
        6.0,
    );

    assert_eq!(resolved, Point::new(90.0, 214.0));
}

#[test]
fn rectangle_anchor_places_below_with_end_alignment() {
    let resolved = resolve_menu_position(
        MenuPosition::Anchor {
            bounds: rect(100.0, 300.0, 100.0, 40.0),
            side: MenuSide::Below,
            alignment: MenuAlignment::End,
            auto_flip: false,
        },
        Size::new(120.0, 80.0),
        viewport(),
        6.0,
    );

    assert_eq!(resolved, Point::new(80.0, 346.0));
}

#[test]
fn rectangle_anchor_flips_horizontal_side_when_preferred_side_overflows() {
    let resolved = resolve_menu_position(
        MenuPosition::Anchor {
            bounds: rect(770.0, 100.0, 20.0, 24.0),
            side: MenuSide::Right,
            alignment: MenuAlignment::Start,
            auto_flip: true,
        },
        Size::new(150.0, 60.0),
        viewport(),
        4.0,
    );

    assert_eq!(resolved, Point::new(616.0, 100.0));
}

#[test]
fn rectangle_anchor_flips_vertical_side_when_preferred_side_overflows() {
    let resolved = resolve_menu_position(
        MenuPosition::Anchor {
            bounds: rect(100.0, 570.0, 80.0, 20.0),
            side: MenuSide::Below,
            alignment: MenuAlignment::Start,
            auto_flip: true,
        },
        Size::new(120.0, 100.0),
        viewport(),
        4.0,
    );

    assert_eq!(resolved, Point::new(100.0, 466.0));
}

#[test]
fn rectangle_anchor_flips_above_to_below_when_needed() {
    let resolved = resolve_menu_position(
        MenuPosition::Anchor {
            bounds: rect(100.0, 5.0, 80.0, 20.0),
            side: MenuSide::Above,
            alignment: MenuAlignment::Start,
            auto_flip: true,
        },
        Size::new(120.0, 100.0),
        viewport(),
        4.0,
    );

    assert_eq!(resolved, Point::new(100.0, 29.0));
}

#[test]
fn rectangle_anchor_chooses_roomier_side_when_neither_fits() {
    let resolved = resolve_menu_position(
        MenuPosition::Anchor {
            bounds: rect(250.0, 80.0, 20.0, 30.0),
            side: MenuSide::Right,
            alignment: MenuAlignment::Start,
            auto_flip: true,
        },
        Size::new(220.0, 50.0),
        Size::new(400.0, 300.0),
        8.0,
    );

    assert_eq!(resolved.x, 22.0);
    assert_eq!(resolved.y, 80.0);
}

#[test]
fn explicit_anchor_direction_is_clamped_without_flipping() {
    let resolved = resolve_menu_position(
        MenuPosition::Anchor {
            bounds: rect(790.0, 100.0, 20.0, 24.0),
            side: MenuSide::Right,
            alignment: MenuAlignment::Start,
            auto_flip: false,
        },
        Size::new(150.0, 60.0),
        viewport(),
        4.0,
    );

    assert_eq!(resolved, Point::new(650.0, 100.0));
}

#[test]
fn oversized_menu_and_tiny_viewport_never_produce_negative_origin() {
    let resolved = resolve_menu_position(
        point_position(Point::new(500.0, 500.0)),
        Size::new(300.0, 200.0),
        Size::new(40.0, 30.0),
        4.0,
    );

    assert_eq!(resolved, Point::ORIGIN);
}

#[test]
fn non_finite_coordinates_are_sanitized_to_the_origin() {
    let resolved = resolve_menu_position(
        point_position(Point::new(f32::NAN, f32::INFINITY)),
        Size::new(100.0, 50.0),
        viewport(),
        4.0,
    );

    assert_eq!(resolved, Point::new(4.0, 4.0));
}

#[test]
fn negative_anchor_coordinates_are_preserved_until_viewport_clamping() {
    let resolved = resolve_menu_position(
        MenuPosition::Anchor {
            bounds: rect(-10.0, 20.0, 15.0, 10.0),
            side: MenuSide::Right,
            alignment: MenuAlignment::Start,
            auto_flip: false,
        },
        Size::new(100.0, 50.0),
        viewport(),
        2.0,
    );

    assert_eq!(resolved, Point::new(7.0, 20.0));
}

#[test]
fn menu_state_opens_child_and_keeps_ancestor_path() {
    let root = id("root-child");
    let nested = id("nested-child");
    let mut state = MenuState::default();

    state.open_child(&[], root);
    state.open_child(&[root], nested);

    assert_eq!(state.open_path(), &[root, nested]);
    assert!(state.is_deepest(nested));
}

#[test]
fn opening_sibling_replaces_only_the_previous_child_branch() {
    let root = id("root-child");
    let sibling = id("sibling-child");
    let nested = id("nested-child");
    let mut state = MenuState::default();

    state.open_child(&[], root);
    state.open_child(&[root], nested);
    state.open_child(&[], sibling);

    assert_eq!(state.open_path(), &[sibling]);
}

#[test]
fn close_deepest_unwinds_one_submenu_at_a_time() {
    let root = id("root-child");
    let nested = id("nested-child");
    let mut state = MenuState::default();
    state.open_child(&[], root);
    state.open_child(&[root], nested);

    assert!(state.close_deepest());
    assert_eq!(state.open_path(), &[root]);
    assert!(state.close_deepest());
    assert!(state.open_path().is_empty());
    assert!(!state.close_deepest());
}

#[test]
fn close_all_clears_the_submenu_path() {
    let mut state = MenuState::default();
    state.open_child(&[], id("child"));

    state.close_all();

    assert!(state.open_path().is_empty());
}

#[test]
fn deepest_check_is_false_for_other_or_missing_ids() {
    let mut state = MenuState::default();
    state.open_child(&[], id("open"));

    assert!(state.is_deepest(id("open")));
    assert!(!state.is_deepest(id("other")));
}

#[test]
fn row_bounds_include_card_padding_and_inter_row_spacing() {
    let items = vec![
        MenuItem::<Action>::heading("Group"),
        MenuItem::action("Run", Action::Run),
        MenuItem::submenu(id("nested"), "Nested", Vec::new()),
    ];
    let style = MenuStyle::default();

    assert_eq!(
        row_bounds(&items, 2, Point::new(10.0, 10.0), style),
        Rectangle {
            x: 10.0,
            y: 10.0
                + style.padding
                + style.heading_height
                + style.row_spacing
                + style.row_height
                + style.row_spacing,
            width: style.width,
            height: style.row_height,
        }
    );
}

#[test]
fn empty_menu_overlay_returns_base_without_a_dismiss_layer() {
    let base: Element<'_, MenuEvent<Action>> = text("base").into();
    let result = menu_overlay(
        base,
        &[],
        &MenuState::default(),
        point_position(Point::ORIGIN),
        viewport(),
        MenuStyle::default(),
    );

    assert_eq!(
        result.as_widget().size(),
        Size::new(Length::Shrink, Length::Shrink)
    );
}

#[test]
fn nonempty_nested_menu_overlay_builds() {
    let items = vec![MenuItem::submenu(
        id("tools"),
        "Tools",
        vec![MenuItem::action("Run", Action::Run)],
    )];
    let mut state = MenuState::default();
    state.open_child(&[], id("tools"));
    let base: Element<'_, MenuEvent<Action>> = text("base").into();

    let result = menu_overlay(
        base,
        &items,
        &state,
        point_position(Point::new(10.0, 10.0)),
        viewport(),
        MenuStyle::default(),
    );

    assert_eq!(
        result.as_widget().size(),
        Size::new(Length::Fill, Length::Fill)
    );
}

#[test]
fn invalid_open_submenu_path_leaves_root_surface_renderable() {
    let items = vec![MenuItem::submenu(
        id("tools"),
        "Tools",
        vec![MenuItem::action("Run", Action::Run)],
    )];
    let mut state = MenuState::default();
    state.open_child(&[], id("missing"));
    let base: Element<'_, MenuEvent<Action>> = text("base").into();

    let result = menu_overlay(
        base,
        &items,
        &state,
        point_position(Point::new(10.0, 10.0)),
        viewport(),
        MenuStyle::default(),
    );

    assert_eq!(
        result.as_widget().size(),
        Size::new(Length::Fill, Length::Fill)
    );
}

#[test]
fn nested_menu_keeps_parent_and_child_rows_interactive() {
    let items = vec![MenuItem::submenu(
        id("tools"),
        "Tools",
        vec![MenuItem::action("Run", Action::Run)],
    )];
    let mut state = MenuState::default();
    state.open_child(&[], id("tools"));
    let base: Element<'_, MenuEvent<Action>> = text("base").into();
    let element = menu_overlay(
        base,
        &items,
        &state,
        point_position(Point::new(10.0, 10.0)),
        viewport(),
        MenuStyle::default(),
    );
    let mut ui = iced_test::simulator(element);

    let _ = ui.click("Tools >");
    let _ = ui.click("Run");

    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        vec![
            MenuEvent::OpenSubmenu {
                parent_path: Vec::new(),
                id: id("tools"),
            },
            MenuEvent::Action(Action::Run),
        ]
    );
}

#[test]
fn action_row_emits_its_caller_message() {
    let items = vec![MenuItem::action("Run", Action::Run)];
    let base: Element<'_, MenuEvent<Action>> = text("base").into();
    let element = menu_overlay(
        base,
        &items,
        &MenuState::default(),
        point_position(Point::new(10.0, 10.0)),
        viewport(),
        MenuStyle::default(),
    );
    let mut ui = iced_test::simulator(element);

    let _ = ui.click("Run");

    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        vec![MenuEvent::Action(Action::Run)]
    );
}

#[test]
fn toggle_row_emits_the_opposite_checked_value() {
    let items = vec![MenuItem::<Action>::toggle(id("verbose"), "Verbose", false)];
    let base: Element<'_, MenuEvent<Action>> = text("base").into();
    let element = menu_overlay(
        base,
        &items,
        &MenuState::default(),
        point_position(Point::new(10.0, 10.0)),
        viewport(),
        MenuStyle::default(),
    );
    let mut ui = iced_test::simulator(element);

    let _ = ui.click("Verbose");

    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        vec![MenuEvent::Toggle {
            id: id("verbose"),
            checked: true,
        }]
    );
}

#[test]
fn radio_row_emits_selection_message_when_not_selected() {
    let items = vec![MenuItem::<Action>::radio(id("balanced"), "Balanced", false)];
    let base: Element<'_, MenuEvent<Action>> = text("base").into();
    let element = menu_overlay(
        base,
        &items,
        &MenuState::default(),
        point_position(Point::new(10.0, 10.0)),
        viewport(),
        MenuStyle::default(),
    );
    let mut ui = iced_test::simulator(element);

    let _ = ui.click("Balanced");

    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        vec![MenuEvent::RadioSelected { id: id("balanced") }]
    );
}

#[test]
fn selected_radio_row_does_not_emit_a_duplicate_selection() {
    let items = vec![MenuItem::<Action>::radio(id("balanced"), "Balanced", true)];
    let base: Element<'_, MenuEvent<Action>> = text("base").into();
    let element = menu_overlay(
        base,
        &items,
        &MenuState::default(),
        point_position(Point::new(10.0, 10.0)),
        viewport(),
        MenuStyle::default(),
    );
    let mut ui = iced_test::simulator(element);

    let _ = ui.click("Balanced");

    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        vec![MenuEvent::Ignored]
    );
}

#[test]
fn click_away_emits_dismissed_for_the_complete_menu_tree() {
    let items = vec![MenuItem::<Action>::action("Run", Action::Run)];
    let base: Element<'_, MenuEvent<Action>> = text("base").into();
    let element = menu_overlay(
        base,
        &items,
        &MenuState::default(),
        point_position(Point::new(10.0, 10.0)),
        viewport(),
        MenuStyle::default(),
    );
    let mut ui = iced_test::simulator(element);
    let point = Point::new(500.0, 500.0);
    ui.point_at(point);
    let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::CursorMoved {
        position: point,
    })]);
    let _ = ui.simulate([
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ]);

    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        vec![MenuEvent::Dismissed]
    );
}

#[test]
fn disabled_menu_row_does_not_emit_an_action() {
    let items = vec![MenuItem::<Action>::disabled("Unavailable")];
    let base: Element<'_, MenuEvent<Action>> = text("base").into();
    let element = menu_overlay(
        base,
        &items,
        &MenuState::default(),
        point_position(Point::new(10.0, 10.0)),
        viewport(),
        MenuStyle::default(),
    );
    let mut ui = iced_test::simulator(element);

    let _ = ui.click("Unavailable");

    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        vec![MenuEvent::Ignored]
    );
}

#[test]
fn disabled_action_row_does_not_emit_its_message() {
    let items = vec![MenuItem::action_enabled("Unavailable", Action::Run, false)];
    let base: Element<'_, MenuEvent<Action>> = text("base").into();
    let element = menu_overlay(
        base,
        &items,
        &MenuState::default(),
        point_position(Point::new(10.0, 10.0)),
        viewport(),
        MenuStyle::default(),
    );
    let mut ui = iced_test::simulator(element);

    let _ = ui.click("Unavailable");

    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        vec![MenuEvent::Ignored]
    );
}

#[test]
fn disabled_and_informational_rows_consume_clicks_without_action_messages() {
    let items = vec![
        MenuItem::<Action>::heading("Group"),
        MenuItem::action_enabled("Unavailable", Action::Run, false),
        MenuItem::Toggle {
            id: id("toggle"),
            label: "Toggle unavailable".to_owned(),
            checked: false,
            enabled: false,
        },
        MenuItem::Radio {
            id: id("radio"),
            label: "Radio unavailable".to_owned(),
            selected: false,
            enabled: false,
        },
        MenuItem::Submenu {
            id: id("submenu"),
            label: "Submenu unavailable".to_owned(),
            children: vec![MenuItem::action("Child", Action::Run)],
            enabled: false,
        },
        MenuItem::disabled("Informational"),
        MenuItem::separator(),
    ];
    let base: Element<'_, MenuEvent<Action>> = text("base").into();
    let element = menu_overlay(
        base,
        &items,
        &MenuState::default(),
        point_position(Point::new(10.0, 10.0)),
        viewport(),
        MenuStyle::default(),
    );
    let mut ui = iced_test::simulator(element);

    for label in [
        "Group",
        "Unavailable",
        "Toggle unavailable",
        "Radio unavailable",
        "Submenu unavailable >",
        "Informational",
    ] {
        let _ = ui.click(label);
    }
    let separator_point = Point::new(30.0, 212.0);
    ui.point_at(separator_point);
    let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::CursorMoved {
        position: separator_point,
    })]);
    let _ = ui.simulate([
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ]);

    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        vec![MenuEvent::Ignored; 7]
    );
}

#[test]
fn submenu_row_emits_parent_path_and_child_id() {
    let items = vec![MenuItem::submenu(
        id("tools"),
        "Tools",
        vec![MenuItem::action("Run", Action::Run)],
    )];
    let base: Element<'_, MenuEvent<Action>> = text("base").into();
    let element = menu_overlay(
        base,
        &items,
        &MenuState::default(),
        point_position(Point::new(10.0, 10.0)),
        viewport(),
        MenuStyle::default(),
    );
    let mut ui = iced_test::simulator(element);

    let _ = ui.click("Tools >");

    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        vec![MenuEvent::OpenSubmenu {
            parent_path: Vec::new(),
            id: id("tools"),
        }]
    );
}
