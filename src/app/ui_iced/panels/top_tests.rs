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
    app.file_processor_mut().indexing_finished = false;
    let vm = TopBarViewModel::from_app(&app);
    assert!(!vm.indexing_finished);
    assert_eq!(vm.file_count, app.file_processor().all_files.len());

    app.file_processor_mut().indexing_finished = true;
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
    app.file_processor_mut().indexing_finished = false;
    let _ = top_bar(&TopBarViewModel::from_app(&app));

    app.file_processor_mut().indexing_finished = true;
    let _ = top_bar(&TopBarViewModel::from_app(&app));
}

// --- compact-bar band contract ---------------------------------------------

/// Regression: the top bar keeps its fixed 28 pt band. Without a fixed
/// height the pick lists' default 5 pt button padding grows the band to
/// ~40 logical points — the user-visible "navbar way too tall" report.
#[test]
fn top_band_height_matches_legacy_toolbar() {
    assert_eq!(BAR_HEIGHT_POINTS, 28.0);
}

/// Regression: both pick lists use the tightened [`PICK_LIST_PADDING`],
/// whose content height (padding + 16 pt text line) must fit inside the
/// [`BAR_HEIGHT_POINTS`] band minus the [`BAR_PADDING`] chrome so
/// nothing is clipped:
/// `(top + bottom) padding + 16 + 2 * BAR_PADDING <= BAR_HEIGHT_POINTS`.
#[test]
fn pick_list_content_fits_inside_the_fixed_band() {
    let pick_list_content = PICK_LIST_PADDING.top + PICK_LIST_PADDING.bottom + 16.0; // iced default text size
    let border_allowance = 2.0; // 1 pt style border above + below
    assert!(
        pick_list_content + border_allowance + 2.0 * BAR_PADDING <= BAR_HEIGHT_POINTS,
        "pick list ({pick_list_content} + {border_allowance}) does not fit \
         BAR_HEIGHT_POINTS = {BAR_HEIGHT_POINTS} with BAR_PADDING = {BAR_PADDING}"
    );
}

// --- hamburger menu card ----------------------------------------------------

/// The card anchor sits one bar-height plus the below-bar gap down, and one
/// right-margin in from the window's right edge.
#[test]
fn hamburger_anchor_hangs_below_the_bar_at_the_right_edge() {
    let wide = Size::new(1600.0, 900.0);
    let anchor = hamburger_menu_anchor(wide);
    assert_eq!(anchor.y, BAR_HEIGHT_POINTS + MENU_ANCHOR_BELOW_BAR);
    assert_eq!(
        anchor.x,
        1600.0 - MENU_WIDTH - MENU_ANCHOR_RIGHT_MARGIN,
        "the card must right-align under the ☰ button"
    );
}

/// A viewport narrower than the card must not push the anchor off-screen to
/// the left — the `max(0.0)` guard clamps it.
#[test]
fn hamburger_anchor_clamps_to_non_negative_x_on_a_narrow_viewport() {
    let narrow = Size::new(MENU_WIDTH * 0.5, 400.0);
    assert_eq!(hamburger_menu_anchor(narrow).x, 0.0);
}

/// A selectable row's message is the row's own command riding the menu
/// channel, so the reducer can close the menu in the same reduction.
#[test]
fn hamburger_row_message_wraps_the_row_command_on_the_menu_channel() {
    let command = apply_about_button_click();
    assert_eq!(
        hamburger_row_message(&command),
        Message::Menu(MenuMessage::RowActivated(command))
    );
}

/// Card rows render *below* their section header, so the label carries the
/// check slot but no submenu prefix.
#[test]
fn hamburger_row_label_has_check_slot_and_no_submenu_prefix() {
    let checked = hamburger_row_label("Hybrid", true);
    let unchecked = hamburger_row_label("Hybrid", false);
    assert!(
        checked.contains('✓'),
        "checked row must show the tick: {checked}"
    );
    assert!(checked.ends_with("Hybrid"));
    assert!(!unchecked.contains('✓'));
    assert!(
        !unchecked.contains('>'),
        "card rows carry no submenu prefix"
    );
    assert!(unchecked.ends_with("Hybrid"));
}

#[test]
fn submenu_label_uses_the_required_right_arrow_cue() {
    assert_eq!(
        strings::format_menu_submenu_label(strings::MENU_WINDOWS),
        "Windows... >"
    );
}

/// The card builds for the full production entry set — headers, the
/// no-models placeholder and every action row — without panicking, and
/// keeps the `'static` lifetime the overlay host needs.
#[test]
fn hamburger_menu_card_builds_for_the_full_entry_set() {
    let app = test_app();
    let view_model = TopBarViewModel::from_app(&app);
    let entries = build_menu_entries(&view_model);
    let card: Element<'static, Message> = hamburger_menu_card(&entries, None);
    let _ = card;
    let submenu: Element<'static, Message> =
        hamburger_menu_card(&entries, Some(MenuSection::ChatModels));
    let _ = submenu;
}

/// An empty entry list still builds a (header-less) card rather than
/// panicking — the overlay host clamps the size, never the shape.
#[test]
fn hamburger_menu_card_builds_for_an_empty_entry_list() {
    let card: Element<'static, Message> = hamburger_menu_card(&[], None);
    let _ = card;
}

#[test]
fn hamburger_menu_row_count_matches_the_visible_level() {
    let entries = build_menu_entries(&view_model_with_models());
    assert_eq!(hamburger_menu_row_count(&entries, None), 6);
    assert_eq!(
        hamburger_menu_row_count(&entries, Some(MenuSection::Windows)),
        4,
        "submenu rows include title and back navigation"
    );
    assert_eq!(
        hamburger_menu_row_count(&entries, Some(MenuSection::TableWrap)),
        7,
        "all five strategies remain visible in their submenu"
    );
}
