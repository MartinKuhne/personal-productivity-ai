//! Tests for `ui_iced/panels/top.rs`.
//!
//! Widget construction cannot be driven headlessly without the iced
//! simulator, so these tests cover the state/mapping layer the widget is
//! built from: the pure helpers, the view-model constructor, and the
//! menu-row → `UserCommand` mapping.

use super::*;
use crate::config::{AppConfig, LlmConfig};
use crate::ui_iced::test_helpers::{test_app, test_app_with_config};
use std::collections::HashMap;

/// Chat models fixture mirroring `ui/panels/top_tests.rs`: two
/// chat-capable models with distinct costs.
fn app_with_models() -> FastMdApp {
    let mut models = HashMap::new();
    models.insert(
        "model-a".to_string(),
        LlmConfig {
            model: "model-a".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "key".to_string(),
            cost: Some(10),
            use_case: vec!["chat".to_string()],
        },
    );
    models.insert(
        "model-b".to_string(),
        LlmConfig {
            model: "model-b".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "key".to_string(),
            cost: Some(20),
            use_case: vec!["chat".to_string()],
        },
    );
    test_app_with_config(AppConfig {
        models,
        ..AppConfig::default()
    })
}

/// A view model with two chat models and nothing else interesting.
fn view_model_with_models() -> TopBarViewModel {
    TopBarViewModel::from_app(&app_with_models())
}

// --- ported pure helpers (mirrored from `ui/panels/top_tests.rs`) ---------

#[test]
fn indexing_status_text_finished() {
    assert_eq!(
        indexing_status_text(true, 42),
        "Indexing finished (42 files)"
    );
    assert_eq!(
        indexing_status_text(true, 42),
        strings::build_indexing_finished_text(42)
    );
}

#[test]
fn indexing_status_text_unfinished() {
    assert_eq!(
        indexing_status_text(false, 10),
        "Indexing workspace (found 10 files)..."
    );
    assert_eq!(
        indexing_status_text(false, 10),
        strings::build_indexing_progress_text(10)
    );
}

#[test]
fn indexing_status_color_uses_success_token_when_finished() {
    let expected = Palette::fastmd_dark().success;
    assert_eq!(indexing_status_color(true), expected);
    // While indexing the default body-text token is used instead.
    assert_eq!(indexing_status_color(false), Palette::fastmd_dark().text);
    assert_ne!(indexing_status_color(true), indexing_status_color(false));
}

#[test]
fn indexing_status_font_is_italic_only_while_indexing() {
    assert_eq!(indexing_status_font(true), Font::DEFAULT);
    assert_eq!(
        indexing_status_font(false),
        Font {
            style: iced::font::Style::Italic,
            ..Font::DEFAULT
        }
    );
}

#[test]
fn get_tag_filter_text_default_and_selected() {
    assert_eq!(get_tag_filter_text(None), "Filter by Tag: All");
    let tag = "Rust".to_string();
    assert_eq!(get_tag_filter_text(Some(&tag)), "Rust");
}

#[test]
fn compute_next_selected_file_no_selected_file() {
    let file_tags = BTreeMap::new();
    assert_eq!(compute_next_selected_file(None, None, &file_tags), None);
}

#[test]
fn compute_next_selected_file_no_tag() {
    let mut file_tags = BTreeMap::new();
    let path = PathBuf::from("test.md");
    file_tags.insert(path.clone(), vec!["Rust".to_string()]);

    assert_eq!(
        compute_next_selected_file(Some(&path), None, &file_tags),
        Some(path)
    );
}

#[test]
fn compute_next_selected_file_tag_matches() {
    let mut file_tags = BTreeMap::new();
    let path = PathBuf::from("test.md");
    file_tags.insert(path.clone(), vec!["Rust".to_string()]);
    let tag = "Rust".to_string();

    assert_eq!(
        compute_next_selected_file(Some(&path), Some(&tag), &file_tags),
        Some(path)
    );
}

#[test]
fn compute_next_selected_file_tag_missing() {
    let mut file_tags = BTreeMap::new();
    let path = PathBuf::from("test.md");
    file_tags.insert(path.clone(), vec!["Rust".to_string()]);
    let tag = "Go".to_string();

    assert_eq!(
        compute_next_selected_file(Some(&path), Some(&tag), &file_tags),
        None
    );
}

#[test]
fn compute_next_selected_file_file_not_in_tags() {
    let file_tags = BTreeMap::new();
    let path = PathBuf::from("test.md");
    let tag = "Rust".to_string();

    assert_eq!(
        compute_next_selected_file(Some(&path), Some(&tag), &file_tags),
        None
    );
}

#[test]
fn view_model_next_selected_file_delegates_to_pure_helper() {
    let vm = view_model_with_models();
    let mut file_tags = BTreeMap::new();
    let path = PathBuf::from("test.md");
    file_tags.insert(path.clone(), vec!["Rust".to_string()]);
    let tag = "Go".to_string();

    assert_eq!(
        vm.next_selected_file_for(Some(&path), Some(&tag), &file_tags),
        compute_next_selected_file(Some(&path), Some(&tag), &file_tags)
    );
    assert_eq!(
        vm.next_selected_file_for(Some(&path), Some(&tag), &file_tags),
        None
    );
}

#[test]
fn strategy_label_maps_every_variant() {
    let proportional = strategy_label(DeficitStrategy::ProportionalToSlack);
    let waterfill = strategy_label(DeficitStrategy::BreakpointWaterFill);
    let ratio = strategy_label(DeficitStrategy::WaterFillRatio);
    let lagrange = strategy_label(DeficitStrategy::LagrangePenalty);
    let hybrid = strategy_label(DeficitStrategy::HybridMinPenaltyWaterFill);

    let labels = [proportional, waterfill, ratio, lagrange, hybrid];
    for label in labels {
        assert!(!label.is_empty(), "strategy labels must not be empty");
    }
    for (i, a) in labels.iter().enumerate() {
        for (j, b) in labels.iter().enumerate() {
            if i != j {
                assert_ne!(
                    a, b,
                    "labels for distinct variants must differ ({i} vs {j})"
                );
            }
        }
    }
    assert_eq!(proportional, strings::TABLE_WIDTH_STRATEGY_PROPORTIONAL);
    assert_eq!(waterfill, strings::TABLE_WIDTH_STRATEGY_WATERFILL);
    assert_eq!(ratio, strings::TABLE_WIDTH_STRATEGY_RATIO);
    assert_eq!(lagrange, strings::TABLE_WIDTH_STRATEGY_LAGRANGE);
    assert_eq!(hybrid, strings::TABLE_WIDTH_STRATEGY_HYBRID);
}

#[test]
fn strategy_menu_order_covers_every_variant_exactly_once() {
    let order = strategy_menu_order();
    let all = [
        DeficitStrategy::ProportionalToSlack,
        DeficitStrategy::BreakpointWaterFill,
        DeficitStrategy::WaterFillRatio,
        DeficitStrategy::LagrangePenalty,
        DeficitStrategy::HybridMinPenaltyWaterFill,
    ];
    assert_eq!(order.len(), all.len());
    for variant in all {
        assert_eq!(
            order.iter().filter(|v| **v == variant).count(),
            1,
            "{variant:?} must appear exactly once in the menu order"
        );
    }
    // The production default leads the menu.
    assert_eq!(
        order[0],
        DeficitStrategy::HybridMinPenaltyWaterFill,
        "the production default strategy must be listed first"
    );
}

// --- tag filter options ---------------------------------------------------

#[test]
fn tag_filter_option_all_carries_no_tag() {
    let all = TagFilterOption::all();
    assert_eq!(all.label, strings::TAG_FILTER_ALL);
    assert_eq!(all.tag, None);
    assert_eq!(all.command(), UserCommand::SelectTagFilter(None));
    assert_eq!(all.to_string(), strings::TAG_FILTER_ALL);
}

#[test]
fn tag_filter_option_for_tag_carries_the_tag() {
    let option = TagFilterOption::for_tag("Rust");
    assert_eq!(option.label, "Rust");
    assert_eq!(
        option.command(),
        UserCommand::SelectTagFilter(Some("Rust".to_string()))
    );
}

#[test]
fn tag_filter_option_named_all_is_still_a_real_tag_filter() {
    // A tag literally spelled "All" must never collapse into the "show
    // everything" entry: the command is driven by `tag`, not the label.
    let option = TagFilterOption::for_tag(strings::TAG_FILTER_ALL);
    assert_eq!(
        option.command(),
        UserCommand::SelectTagFilter(Some(strings::TAG_FILTER_ALL.to_string()))
    );
    assert_ne!(option, TagFilterOption::all());
}

// --- chat model entries ---------------------------------------------------

#[test]
fn chat_model_entry_label_matches_ui_064_format() {
    let model = ChatModelEntry {
        name: "gpt-x".to_string(),
        cost: 7,
    };
    assert_eq!(model.label(), "gpt-x (Cost: 7)");
    assert_eq!(
        model.command(),
        UserCommand::SelectChatModel("gpt-x".to_string())
    );
}

// --- menu sections --------------------------------------------------------

#[test]
fn menu_section_titles_use_the_submenu_strings() {
    assert_eq!(MenuSection::Direct.title(), None);
    assert_eq!(
        MenuSection::Windows.title(),
        Some(strings::MENU_WINDOWS_TITLE)
    );
    assert_eq!(
        MenuSection::ChatModels.title(),
        Some(strings::MENU_CHAT_MODELS)
    );
    assert_eq!(
        MenuSection::TableWrap.title(),
        Some(strings::MENU_TABLE_WRAP_ALGORITHM)
    );
}

// --- hamburger menu structure (UI-061..UI-066) ---------------------------

#[test]
fn menu_entries_are_ordered_per_ui_061_to_066() {
    let entries = build_menu_entries(&view_model_with_models());
    let kinds: Vec<(&str, &MenuEntryKind)> = entries
        .iter()
        .map(|e| (e.label.as_str(), &e.kind))
        .collect();

    let expected_order: Vec<&str> = vec![
        strings::MENU_BATCH,
        strings::MENU_TOOLS,
        strings::MENU_WINDOWS,
        strings::MENU_BACKGROUND_OPERATIONS,
        strings::MENU_AGENT_DEBUG,
        strings::MENU_CHAT_MODELS,
        "model-a (Cost: 10)",
        "model-b (Cost: 20)",
        strings::MENU_TABLE_WRAP_ALGORITHM,
        strings::TABLE_WIDTH_STRATEGY_HYBRID,
        strings::TABLE_WIDTH_STRATEGY_PROPORTIONAL,
        strings::TABLE_WIDTH_STRATEGY_WATERFILL,
        strings::TABLE_WIDTH_STRATEGY_RATIO,
        strings::TABLE_WIDTH_STRATEGY_LAGRANGE,
        strings::MENU_ABOUT,
    ];
    let actual: Vec<&str> = kinds.iter().map(|(l, _)| *l).collect();
    assert_eq!(actual, expected_order);
}

#[test]
fn submenu_entries_are_clickable_and_name_their_destination() {
    let entries = build_menu_entries(&view_model_with_models());
    let submenus: Vec<&MenuEntry> = entries
        .iter()
        .filter(|entry| matches!(entry.kind, MenuEntryKind::Submenu(_)))
        .collect();

    assert_eq!(
        submenus.len(),
        3,
        "Windows / Chat models / Table wrap must be clickable submenus"
    );
    let destinations: Vec<MenuSection> = submenus
        .iter()
        .map(|entry| match &entry.kind {
            MenuEntryKind::Submenu(section) => *section,
            _ => unreachable!("filtered to submenu rows"),
        })
        .collect();
    assert_eq!(
        destinations,
        vec![
            MenuSection::Windows,
            MenuSection::ChatModels,
            MenuSection::TableWrap
        ]
    );
    assert!(
        submenus
            .iter()
            .all(|entry| entry.section == MenuSection::Direct)
    );
    assert!(submenus.iter().all(|entry| entry.kind.is_selectable()));
    assert!(submenus.iter().all(|entry| entry.kind.command().is_none()));
}

#[test]
fn empty_chat_models_yields_a_non_selectable_placeholder() {
    let app = test_app();
    let vm = TopBarViewModel::from_app(&app);
    assert!(vm.chat_models.is_empty());

    let entries = build_menu_entries(&vm);
    let placeholders: Vec<&MenuEntry> = entries
        .iter()
        .filter(|e| e.kind == MenuEntryKind::Placeholder)
        .collect();

    assert_eq!(placeholders.len(), 1, "UI-064 placeholder must appear once");
    assert_eq!(placeholders[0].label, strings::NO_CHAT_MODELS_CONFIGURED);
    assert_eq!(placeholders[0].section, MenuSection::ChatModels);
    assert!(!placeholders[0].kind.is_selectable());

    // ...and it never reaches the widget as a pickable row.
    assert!(
        hamburger_items(&vm)
            .iter()
            .all(|item| !item.label.contains(strings::NO_CHAT_MODELS_CONFIGURED)),
        "the placeholder must not be a selectable hamburger row"
    );
}

#[test]
fn hamburger_items_are_all_selectable_and_drop_headers_and_placeholders() {
    let items = hamburger_items(&view_model_with_models());
    let entries = build_menu_entries(&view_model_with_models());

    let selectable = entries
        .iter()
        .filter(|entry| matches!(entry.kind, MenuEntryKind::Action(_)))
        .count();
    assert_eq!(items.len(), selectable);
    assert_eq!(
        items.len(),
        entries.len() - 3,
        "submenu rows do not produce UserCommands"
    );
}

#[test]
fn hamburger_item_labels_carry_submenu_prefix_and_check_slot() {
    let items = hamburger_items(&view_model_with_models());

    let batch = items
        .iter()
        .find(|i| i.command == UserCommand::OpenBatchDialog)
        .expect("Batch row");
    // Top-level rows have no submenu prefix; unchecked -> blank slot.
    assert_eq!(batch.label, format!("  {}", strings::MENU_BATCH));

    let agent_debug = items
        .iter()
        .find(|i| matches!(i.command, UserCommand::ToggleAgentDebugWindow(true)))
        .expect("Agent debug row");
    assert!(
        agent_debug.label.contains(strings::MENU_AGENT_DEBUG),
        "label must keep the item string: {agent_debug:?}"
    );
    assert!(
        agent_debug.label.contains(strings::MENU_WINDOWS_TITLE),
        "label must carry the owning submenu name: {agent_debug:?}"
    );
    assert!(
        !agent_debug.label.starts_with('\u{2713}'),
        "an unchecked row must not carry the check glyph"
    );

    let current_model = items
        .iter()
        .find(|i| i.command == UserCommand::SelectChatModel("model-a".to_string()))
        .expect("model-a row");
    assert!(
        current_model.label.starts_with('\u{2713}'),
        "the active chat model must carry the check glyph (UI-065): {:?}",
        current_model.label
    );
    // UI-064's `<name> (Cost: <cost>)` survives verbatim inside the row.
    assert!(current_model.label.ends_with("model-a (Cost: 10)"));
}

#[test]
fn checked_rows_match_current_state() {
    let mut vm = view_model_with_models();
    vm.show_background_logs = true;
    vm.show_agent_debug = false;
    vm.current_strategy = DeficitStrategy::LagrangePenalty;

    let entries = build_menu_entries(&vm);
    let by_label = |label: &str| -> bool {
        entries
            .iter()
            .find(|e| e.label == label)
            .unwrap_or_else(|| panic!("missing row {label}"))
            .checked
    };

    assert!(by_label(strings::MENU_BACKGROUND_OPERATIONS));
    assert!(!by_label(strings::MENU_AGENT_DEBUG));
    assert!(by_label(strings::TABLE_WIDTH_STRATEGY_LAGRANGE));
    assert!(!by_label(strings::TABLE_WIDTH_STRATEGY_HYBRID));
    assert!(by_label("model-a (Cost: 10)"));
    assert!(!by_label("model-b (Cost: 20)"));
}

// --- command mapping ------------------------------------------------------

#[test]
fn every_hamburger_row_maps_to_the_expected_user_command() {
    let vm = view_model_with_models();
    let items = hamburger_items(&vm);
    let commands: Vec<UserCommand> = items.iter().map(|i| i.command.clone()).collect();

    let expected: Vec<UserCommand> = vec![
        apply_batch_button_click(),
        apply_tools_button_click(),
        apply_background_logs_toggle(true),
        apply_agent_debug_toggle(true),
        apply_chat_model_selection("model-a".to_string()),
        apply_chat_model_selection("model-b".to_string()),
        apply_table_width_strategy_change(DeficitStrategy::HybridMinPenaltyWaterFill),
        apply_table_width_strategy_change(DeficitStrategy::ProportionalToSlack),
        apply_table_width_strategy_change(DeficitStrategy::BreakpointWaterFill),
        apply_table_width_strategy_change(DeficitStrategy::WaterFillRatio),
        apply_table_width_strategy_change(DeficitStrategy::LagrangePenalty),
        apply_about_button_click(),
    ];
    assert_eq!(commands, expected);
}

#[test]
fn window_toggle_rows_carry_the_switched_to_value() {
    // Shown -> the row publishes `false` (hide it); hidden -> `true`.
    let mut vm = view_model_with_models();
    vm.show_background_logs = true;
    vm.show_agent_debug = false;

    let items = hamburger_items(&vm);
    assert!(
        items
            .iter()
            .any(|i| { i.command == UserCommand::ToggleBackgroundLogs(false) })
    );
    assert!(
        items
            .iter()
            .any(|i| i.command == UserCommand::ToggleAgentDebugWindow(true))
    );

    vm.show_background_logs = false;
    vm.show_agent_debug = true;
    let items = hamburger_items(&vm);
    assert!(
        items
            .iter()
            .any(|i| { i.command == UserCommand::ToggleBackgroundLogs(true) })
    );
    assert!(
        items
            .iter()
            .any(|i| { i.command == UserCommand::ToggleAgentDebugWindow(false) })
    );
}

#[test]
fn hamburger_item_from_entry_rejects_non_actionable_rows() {
    let submenu = submenu_entry(MenuSection::Windows);
    assert_eq!(HamburgerItem::from_entry(&submenu), None);

    let placeholder = MenuEntry {
        label: strings::NO_CHAT_MODELS_CONFIGURED.to_string(),
        section: MenuSection::ChatModels,
        kind: MenuEntryKind::Placeholder,
        checked: false,
    };
    assert_eq!(HamburgerItem::from_entry(&placeholder), None);

    let action = MenuEntry {
        label: strings::MENU_ABOUT.to_string(),
        section: MenuSection::Direct,
        kind: MenuEntryKind::Action(apply_about_button_click()),
        checked: false,
    };
    assert_eq!(
        HamburgerItem::from_entry(&action).map(|i| i.command),
        Some(UserCommand::OpenAboutDialog)
    );
}

#[test]
fn tag_filter_option_commands_cover_all_and_each_tag() {
    let mut vm = view_model_with_models();
    vm.tag_options = vec![
        TagFilterOption::all(),
        TagFilterOption::for_tag("Rust"),
        TagFilterOption::for_tag("Docs"),
    ];

    let commands: Vec<UserCommand> = vm.tag_options.iter().map(|o| o.command()).collect();
    assert_eq!(
        commands,
        vec![
            UserCommand::SelectTagFilter(None),
            UserCommand::SelectTagFilter(Some("Rust".to_string())),
            UserCommand::SelectTagFilter(Some("Docs".to_string())),
        ]
    );
}

// --- view-model construction ---------------------------------------------

#[test]
fn view_model_snapshots_indexing_state() {
    let mut app = test_app();
    app.workspace_mut().indexing_finished = false;
    let vm = TopBarViewModel::from_app(&app);
    assert!(!vm.indexing_finished);
    assert_eq!(vm.file_count, app.workspace().all_files.len());

    app.workspace_mut().indexing_finished = true;
    let vm = TopBarViewModel::from_app(&app);
    assert!(vm.indexing_finished);
}

#[test]
fn view_model_snapshots_tags_with_all_first() {
    let mut app = test_app();
    app.tags_mut().add_tags(
        PathBuf::from("dummy.md"),
        vec!["Rust".to_string(), "Docs".to_string()],
    );

    let vm = TopBarViewModel::from_app(&app);
    assert_eq!(vm.tag_options.len(), 3);
    assert_eq!(vm.tag_options[0], TagFilterOption::all());
    assert!(vm.tag_options.contains(&TagFilterOption::for_tag("Rust")));
    assert!(vm.tag_options.contains(&TagFilterOption::for_tag("Docs")));
    assert_eq!(vm.selected_tag, None);
    assert_eq!(vm.selected_tag_option(), Some(&TagFilterOption::all()));

    app.tags_mut().selected_tag = Some("Rust".to_string());
    let vm = TopBarViewModel::from_app(&app);
    assert_eq!(
        vm.selected_tag_option(),
        Some(&TagFilterOption::for_tag("Rust"))
    );
}

#[test]
fn selected_tag_option_is_none_for_an_unknown_tag() {
    let mut vm = view_model_with_models();
    vm.selected_tag = Some("gone".to_string());
    assert_eq!(vm.selected_tag_option(), None);
}

#[test]
fn view_model_lists_chat_models_sorted_by_name() {
    let vm = view_model_with_models();
    let names: Vec<&str> = vm.chat_models.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, vec!["model-a", "model-b"]);
    assert_eq!(vm.chat_models[0].cost, 10);
    assert_eq!(vm.chat_models[1].cost, 20);
    assert_eq!(vm.current_chat_model.as_deref(), Some("model-a"));
}

#[test]
fn view_model_falls_back_to_all_models_when_none_declare_chat() {
    let mut models = HashMap::new();
    models.insert(
        "embed-1".to_string(),
        LlmConfig {
            model: "embed-1".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "key".to_string(),
            cost: Some(3),
            use_case: vec!["embedding".to_string()],
        },
    );
    let app = test_app_with_config(AppConfig {
        models,
        ..AppConfig::default()
    });

    let vm = TopBarViewModel::from_app(&app);
    // No chat-capable model -> every configured model is listed.
    assert_eq!(vm.chat_models.len(), 1);
    assert_eq!(vm.chat_models[0].name, "embed-1");
}

#[test]
fn view_model_snapshots_window_visibility_flags() {
    let mut app = test_app();
    let vm = TopBarViewModel::from_app(&app);
    assert!(!vm.show_background_logs);
    assert!(!vm.show_agent_debug);

    app.orchestrator
        .apply_user_command(apply_background_logs_toggle(true));
    app.orchestrator
        .apply_user_command(apply_agent_debug_toggle(true));

    let vm = TopBarViewModel::from_app(&app);
    assert!(vm.show_background_logs);
    assert!(vm.show_agent_debug);
}

#[test]
fn view_model_snapshots_current_strategy() {
    let mut app = test_app();
    app.orchestrator
        .apply_user_command(apply_table_width_strategy_change(
            DeficitStrategy::WaterFillRatio,
        ));

    let vm = TopBarViewModel::from_app(&app);
    assert_eq!(vm.current_strategy, DeficitStrategy::WaterFillRatio);

    let entries = build_menu_entries(&vm);
    let checked: Vec<&str> = entries
        .iter()
        .filter(|e| e.section == MenuSection::TableWrap && e.checked)
        .map(|e| e.label.as_str())
        .collect();
    assert_eq!(checked, vec![strings::TABLE_WIDTH_STRATEGY_RATIO]);
}

// --- widget smoke construction -------------------------------------------

#[test]
fn top_bar_builds_for_both_indexing_states() {
    // The element cannot be rendered headlessly, but building the widget
    // tree must not panic for either branch of the indexing flag.
    let mut app = test_app();
    app.workspace_mut().indexing_finished = false;
    let _ = top_bar(&TopBarViewModel::from_app(&app));

    app.workspace_mut().indexing_finished = true;
    let _ = top_bar(&TopBarViewModel::from_app(&app));
}

// --- band contract (INT-7a: resolved from tokens, not literals) ---------

/// The top bar band is the `size.control.lg` rung, which `UIF-TOK-002`
/// names as the Top Navbar height (§4.1.1, §4.2.1).
///
/// The bar used to be `CONTROL_SM` (28). The `assert_ne!` guards below
/// pin that the retired literal is gone rather than merely shadowed.
#[test]
fn top_band_height_is_the_control_lg_rung() {
    assert_eq!(BAR_HEIGHT_POINTS, size::CONTROL_LG);
    assert_eq!(BAR_HEIGHT_POINTS, 36.0);
    assert_ne!(
        BAR_HEIGHT_POINTS, 28.0,
        "the retired CONTROL_SM band must not come back"
    );
}

/// The band closes arithmetically around one compact control rung:
/// `CONTROL_LG = XS + CONTROL_SM + XS`. That is what makes the navbar's
/// 36pt height and its 28pt controls the same fact rather than two
/// numbers that have to be kept in step by hand.
#[test]
fn band_padding_wraps_one_compact_control_rung_exactly() {
    assert_eq!(BAR_PADDING.top, space::XS);
    assert_eq!(BAR_PADDING.bottom, space::XS);
    assert_eq!(BAR_PADDING.left, space::MD);
    assert_eq!(BAR_PADDING.right, space::MD);
    assert_eq!(
        BAR_PADDING.top + size::CONTROL_SM + BAR_PADDING.bottom,
        BAR_HEIGHT_POINTS,
        "vertical chrome plus a CONTROL_SM control must fill the band exactly"
    );
}

/// The tag trigger's laid-out height (line height + vertical padding, the
/// pick_list layout contract at
/// `iced_widget-0.14.2/src/pick_list.rs:421-432`) lands on the compact
/// rung it is declared against, and fills the band's content box exactly.
/// The trigger's `border.hairline` perimeter is painted inside its own
/// quad, so it costs no extra layout room.
#[test]
fn tag_trigger_lands_on_its_declared_rung_and_fits_the_band() {
    let trigger_content =
        typography::BODY.line_height + TAG_TRIGGER_PADDING.top + TAG_TRIGGER_PADDING.bottom;
    assert_eq!(
        trigger_content,
        TAG_TRIGGER_RUNG.height(),
        "BODY line height plus the trigger's vertical padding must land on the rung"
    );
    assert_eq!(TAG_TRIGGER_RUNG.height(), size::CONTROL_SM);

    let content_box = BAR_HEIGHT_POINTS - BAR_PADDING.top - BAR_PADDING.bottom;
    assert_eq!(
        trigger_content,
        content_box,
        "the trigger fills the band's content box exactly: rung {} vs box {}",
        TAG_TRIGGER_RUNG.height(),
        content_box
    );
}

/// The brand title sits on `type.title_sm`, the rung §4.2.1 assigns it —
/// not the off-scale 18 the bar used to paint.
#[test]
fn brand_title_sits_on_the_title_sm_rung() {
    assert_eq!(TITLE_SIZE, typography::TITLE_SM.size);
    assert_eq!(TITLE_SIZE, 16.0);
    assert_ne!(
        TITLE_SIZE, 18.0,
        "18 was never a rung on the type scale; it must not return"
    );
    // §2.4 names the navbar brand title as a Bold usage, which is the
    // rung's emphasis weight, and the rung permits it.
    assert!(typography::TITLE_SM.allows(typography::Weight::Bold));
    assert_eq!(
        typography::TITLE_SM.emphasis_weight,
        typography::Weight::Bold
    );
}

/// Every remaining top-bar scalar resolves to a named token rather than a
/// literal, so a retune of the scale moves the bar with it.
#[test]
fn top_bar_scalars_resolve_from_tokens() {
    assert_eq!(BAR_SPACING, space::SM);
    assert_eq!(SEPARATOR_WIDTH, shape::BORDER_HAIRLINE);
    assert_eq!(MENU_BUTTON_GLYPH_SIZE, size::ICON_MD);
    assert_eq!(MENU_ANCHOR_RIGHT_MARGIN, space::MD);
    assert_eq!(MENU_ANCHOR_BELOW_BAR, space::XS);
    assert_eq!(
        TAG_TRIGGER_PADDING.left,
        combo_box::HORIZONTAL_PADDING,
        "the trigger's horizontal inset comes from the combobox atom, not a local number"
    );
    assert_eq!(
        TAG_TRIGGER_VERTICAL_PADDING,
        (TAG_TRIGGER_RUNG.height() - typography::BODY.line_height) / 2.0,
        "the vertical inset is derived from the rung, not picked"
    );
    assert_eq!(TAG_TRIGGER_VERTICAL_PADDING, space::XS);
}

/// The tag trigger's paint comes from the framework combobox resolver for
/// every iced status, not from iced's own theme — and the chevron tracks
/// the *open* flag the resolver takes separately from hover.
#[test]
fn tag_trigger_paint_resolves_through_the_combo_atom() {
    let pro = Palette::pro();
    let rest = VisualState {
        base: BaseState::Rest,
        focused: false,
    };
    let hovered = VisualState {
        base: BaseState::Hovered,
        focused: false,
    };

    let active = tag_trigger_style(pick_list::Status::Active);
    let resolved_rest = combo_box::resolve_trigger(rest, &pro);
    assert_eq!(
        active.background,
        Background::Color(resolved_rest.background)
    );
    assert_eq!(active.border.color, resolved_rest.border_color);
    assert_eq!(active.border.width, resolved_rest.border_width);
    assert_eq!(active.text_color, resolved_rest.text);
    assert_eq!(active.placeholder_color, resolved_rest.placeholder);
    assert_eq!(
        active.border.radius,
        combo_box::RADIUS.into(),
        "the trigger wears the widget radius, not iced's default"
    );

    let hover = tag_trigger_style(pick_list::Status::Hovered);
    let resolved_hover = combo_box::resolve_trigger(hovered, &pro);
    assert_eq!(
        hover.background,
        Background::Color(resolved_hover.background)
    );
    assert_eq!(hover.border.color, resolved_hover.border_color);
    assert_ne!(
        hover.border.color, active.border.color,
        "hover must be distinguishable from rest through the atom"
    );

    // §3.4: the chevron goes to text.strong while the flyout is open,
    // whether or not the pointer is still over the trigger.
    let open = tag_trigger_style(pick_list::Status::Opened { is_hovered: false });
    assert_eq!(
        open.handle_color,
        combo_box::chevron_color(rest, true, &pro),
        "an open flyout drives the chevron, not hover"
    );
    assert_eq!(open.handle_color, pro.text_strong);
    assert_ne!(
        open.handle_color, active.handle_color,
        "the open chevron must differ from the resting one"
    );

    let open_hovered = tag_trigger_style(pick_list::Status::Opened { is_hovered: true });
    assert_eq!(open_hovered.handle_color, pro.text_strong);
}

/// The hamburger trigger carries an app-chosen stable `Id` rather than
/// `Id::unique()`, so it keeps its focus state across tree rebuilds
/// ([AGENTS §5]).
#[test]
fn hamburger_trigger_has_a_stable_id() {
    assert_eq!(HAMBURGER_BUTTON_ID, "topbar_hamburger_button");
    assert_ne!(HAMBURGER_BUTTON_ID, strings::HAMBURGER_MENU_ID_SALT);
}

// --- hamburger menu widget model -------------------------------------------

/// The root-space anchor sits at the right edge of the hamburger button.
#[test]
fn hamburger_anchor_hangs_below_the_bar_at_the_right_edge() {
    let wide = Size::new(1600.0, 900.0);
    let anchor = hamburger_menu_anchor(wide);
    assert_eq!(anchor.y, BAR_HEIGHT_POINTS);
    assert_eq!(
        anchor.x,
        1600.0 - MENU_ANCHOR_RIGHT_MARGIN + MENU_ANCHOR_BELOW_BAR,
        "the widget gap preserves the configured menu-to-window inset"
    );
}

/// A narrow viewport keeps the anchor non-negative; the shared widget clamps
/// the surface itself to the available viewport.
#[test]
fn hamburger_anchor_clamps_to_non_negative_x_on_a_narrow_viewport() {
    let narrow = Size::new(MENU_ANCHOR_RIGHT_MARGIN * 0.5, 400.0);
    assert_eq!(hamburger_menu_anchor(narrow).x, 0.0);
}

/// A selectable row's message is the row's own command riding the menu
/// channel, so the reducer can close the menu in the same reduction.
#[test]
fn submenu_label_uses_the_required_right_arrow_cue() {
    assert_eq!(
        strings::format_menu_submenu_label(strings::MENU_WINDOWS),
        "Windows... >"
    );
}
