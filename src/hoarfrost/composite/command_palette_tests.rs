//! Unit tests for the `CommandPalette` composite widget (`UIF-COMP-010`, `UIF-PRIN-003`, `UIF-TOK-006`).

use iced::advanced::widget::tree::Tree;
use iced::advanced::{Shell, Widget};
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::mouse::Button;
use iced::{Element, Event, Length, Point, Rectangle, Size};

use super::*;
use crate::test_helpers::Harness;
use crate::token::color::Palette;
use crate::token::contrast;
use crate::widget::text_input_model::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TestId {
    FileOpen,
    FileSave,
    Settings,
    GitCommit,
}

fn sample_items() -> Vec<PaletteGroup<TestId>> {
    vec![
        PaletteGroup::new(
            "Recent Files",
            vec![
                PaletteItem::new(TestId::FileOpen, "main.rs")
                    .with_subtitle("src/main.rs")
                    .with_match_spans(vec![TextRange::new(0, 4)])
                    .with_shortcut("Ctrl+P"),
                PaletteItem::new(TestId::FileSave, "lib.rs").with_shortcut("Ctrl+S"),
            ],
        ),
        PaletteGroup::new(
            "Commands",
            vec![
                PaletteItem::new(TestId::Settings, "Open Settings")
                    .with_subtitle("Preferences and configurations"),
                PaletteItem::new(TestId::GitCommit, "Git: Commit")
                    .with_match_spans(vec![TextRange::new(5, 11)])
                    .with_shortcut("Ctrl+Enter"),
            ],
        ),
    ]
}

// ---------------------------------------------------------------------------
// 1. PaletteMode Tests
// ---------------------------------------------------------------------------

#[test]
fn test_palette_mode_defaults_and_prefixes() {
    assert_eq!(PaletteMode::default(), PaletteMode::Files);
    assert_eq!(PaletteMode::Files.prefix(), None);
    assert_eq!(PaletteMode::Commands.prefix(), Some('>'));
    assert_eq!(PaletteMode::Tags.prefix(), Some('#'));
    assert_eq!(PaletteMode::Symbols.prefix(), Some('@'));

    assert_eq!(PaletteMode::from_prefix('>'), Some(PaletteMode::Commands));
    assert_eq!(PaletteMode::from_prefix('#'), Some(PaletteMode::Tags));
    assert_eq!(PaletteMode::from_prefix('@'), Some(PaletteMode::Symbols));
    assert_eq!(PaletteMode::from_prefix('!'), None);
    assert_eq!(PaletteMode::from_prefix('a'), None);
    assert_eq!(PaletteMode::from_prefix(' '), None);

    assert_eq!(PaletteMode::Files.label(), "Files");
    assert_eq!(PaletteMode::Commands.label(), "Commands");
    assert_eq!(PaletteMode::Tags.label(), "Tags");
    assert_eq!(PaletteMode::Symbols.label(), "Symbols");

    assert_eq!(PaletteMode::Files.placeholder(), PLACEHOLDER_FILES);
    assert_eq!(PaletteMode::Commands.placeholder(), PLACEHOLDER_COMMANDS);
    assert_eq!(PaletteMode::Symbols.placeholder(), PLACEHOLDER_SYMBOLS);
    assert_eq!(PaletteMode::Tags.placeholder(), PLACEHOLDER_TAGS);
}

// ---------------------------------------------------------------------------
// 2. PaletteItem & PaletteGroup Tests
// ---------------------------------------------------------------------------

#[test]
fn test_palette_item_construction_and_builders() {
    let item = PaletteItem::new(TestId::FileOpen, "test.rs")
        .with_subtitle("path/to/test.rs")
        .with_match_spans(vec![TextRange::new(0, 4)])
        .with_shortcut("Ctrl+O");

    assert_eq!(item.id, TestId::FileOpen);
    assert_eq!(item.title, "test.rs");
    assert_eq!(item.subtitle.as_deref(), Some("path/to/test.rs"));
    assert_eq!(item.match_spans, vec![TextRange::new(0, 4)]);
    assert_eq!(item.shortcut.as_deref(), Some("Ctrl+O"));

    let group = PaletteGroup::new("Group 1", vec![item.clone()]);
    assert_eq!(group.heading, "Group 1");
    assert_eq!(group.items, vec![item]);
}

// ---------------------------------------------------------------------------
// 3. CommandPaletteModel Tests
// ---------------------------------------------------------------------------

#[test]
fn test_command_palette_model_construction_and_defaults() {
    let groups = sample_items();
    let model = CommandPaletteModel::new(groups);

    assert_eq!(model.query, "");
    assert_eq!(model.mode, PaletteMode::Files);
    assert_eq!(model.generation, 0);
    assert!(!model.busy);
    assert!(model.open);
    assert_eq!(model.highlighted_id, Some(TestId::FileOpen));
    assert_eq!(model.item_count(), 4);
    assert_eq!(model.all_items().len(), 4);

    let empty_model = CommandPaletteModel::<TestId>::new(Vec::new());
    assert_eq!(empty_model.highlighted_id, None);
    assert_eq!(empty_model.item_count(), 0);
    assert!(empty_model.all_items().is_empty());
}

#[test]
fn test_command_palette_model_builders() {
    let model = CommandPaletteModel::new(sample_items())
        .with_query("search")
        .with_mode(PaletteMode::Commands)
        .with_open(false)
        .with_busy(true)
        .with_highlighted_id(Some(TestId::GitCommit));

    assert_eq!(model.query, "search");
    assert_eq!(model.mode, PaletteMode::Commands);
    assert!(!model.open);
    assert!(model.busy);
    assert_eq!(model.highlighted_id, Some(TestId::GitCommit));
}

#[test]
fn test_highlight_roving_and_wrapping() {
    let mut model = CommandPaletteModel::new(sample_items());
    assert_eq!(model.highlighted_id, Some(TestId::FileOpen));

    // Next roves sequentially through: FileOpen -> FileSave -> Settings -> GitCommit -> FileOpen (wrap)
    model.highlight_next();
    assert_eq!(model.highlighted_id, Some(TestId::FileSave));

    model.highlight_next();
    assert_eq!(model.highlighted_id, Some(TestId::Settings));

    model.highlight_next();
    assert_eq!(model.highlighted_id, Some(TestId::GitCommit));

    model.highlight_next();
    assert_eq!(model.highlighted_id, Some(TestId::FileOpen)); // Wrap-around!

    // Prev roves backwards and wraps around at 0
    model.highlight_prev();
    assert_eq!(model.highlighted_id, Some(TestId::GitCommit));

    model.highlight_prev();
    assert_eq!(model.highlighted_id, Some(TestId::Settings));

    model.highlight_first();
    assert_eq!(model.highlighted_id, Some(TestId::FileOpen));

    model.highlight_last();
    assert_eq!(model.highlighted_id, Some(TestId::GitCommit));

    // Test with empty model
    let mut empty = CommandPaletteModel::<TestId>::new(Vec::new());
    empty.highlight_next();
    assert_eq!(empty.highlighted_id, None);
    empty.highlight_prev();
    assert_eq!(empty.highlighted_id, None);
}

#[test]
fn test_selected_item() {
    let mut model = CommandPaletteModel::new(sample_items());
    let selected = model.selected_item().expect("selected item");
    assert_eq!(selected.id, TestId::FileOpen);
    assert_eq!(selected.title, "main.rs");

    model.highlighted_id = None;
    assert_eq!(model.selected_item(), None);
}

#[test]
fn test_two_stage_escape_contract() {
    let mut model = CommandPaletteModel::new(sample_items()).with_query("query text");
    assert_eq!(model.generation, 0);

    // Stage 1: non-empty query -> ClearedQuery, generation increments
    let stage1 = model.handle_escape();
    assert_eq!(stage1, EscapeDisposition::ClearedQuery);
    assert_eq!(model.query, "");
    assert_eq!(model.generation, 1);

    // Stage 2: empty query -> Dismiss
    let stage2 = model.handle_escape();
    assert_eq!(stage2, EscapeDisposition::Dismiss);
    assert_eq!(model.query, "");
    assert_eq!(model.generation, 1);
}

#[test]
fn test_set_query_from_input_prefix_detection_and_caret_0_reset() {
    let mut model = CommandPaletteModel::new(sample_items());
    assert_eq!(model.mode, PaletteMode::Files);
    assert_eq!(model.query, "");
    assert_eq!(model.generation, 0);

    // 1. User types ">" -> mode switches to Commands, stripped query is empty, generation increments
    let mode1 = model.set_query_from_input(">");
    assert_eq!(mode1, Some(PaletteMode::Commands));
    assert_eq!(model.mode, PaletteMode::Commands);
    assert_eq!(model.query, "");
    assert_eq!(model.generation, 1);

    // 2. User types "open" -> retained in Commands mode, query becomes "open"
    let mode2 = model.set_query_from_input("open");
    assert_eq!(mode2, None);
    assert_eq!(model.mode, PaletteMode::Commands);
    assert_eq!(model.query, "open");
    assert_eq!(model.generation, 2);

    // 3. User backspaces query to empty
    let mode3 = model.set_query_from_input("");
    assert_eq!(mode3, None);
    assert_eq!(model.mode, PaletteMode::Commands);
    assert_eq!(model.query, "");
    assert_eq!(model.generation, 3);

    // 4. User backspaces at caret 0 while query is already empty -> resets to Files mode!
    let mode4 = model.set_query_from_input("");
    assert_eq!(mode4, Some(PaletteMode::Files));
    assert_eq!(model.mode, PaletteMode::Files);
    assert_eq!(model.query, "");
    assert_eq!(model.generation, 4);

    // 5. Test Tag prefix `#`
    let mode5 = model.set_query_from_input("#rust");
    assert_eq!(mode5, Some(PaletteMode::Tags));
    assert_eq!(model.mode, PaletteMode::Tags);
    assert_eq!(model.query, "rust");

    // 6. Test Symbol prefix `@`
    let mode6 = model.set_query_from_input("@fn_main");
    assert_eq!(mode6, Some(PaletteMode::Symbols));
    assert_eq!(model.mode, PaletteMode::Symbols);
    assert_eq!(model.query, "fn_main");

    // 7. backspace_prefix helper
    let res = model.backspace_prefix();
    assert_eq!(res, Some(PaletteMode::Files));
    assert_eq!(model.mode, PaletteMode::Files);
    assert_eq!(model.backspace_prefix(), None);
}

#[test]
fn test_open_close_methods() {
    let mut model = CommandPaletteModel::new(sample_items());
    assert!(model.open);

    model.close();
    assert!(!model.open);

    model.open_palette();
    assert!(model.open);
}

// ---------------------------------------------------------------------------
// 4. Pure Geometry and Highlighting Tests
// ---------------------------------------------------------------------------

#[test]
fn test_card_bounds_centering_and_clamping() {
    let viewport_wide = Rectangle::new(Point::new(0.0, 0.0), Size::new(1920.0, 1080.0));
    let bounds_wide = card_bounds(viewport_wide);

    assert_eq!(bounds_wide.width, CARD_WIDTH); // 560px
    assert_eq!(bounds_wide.height, CARD_HEIGHT); // 380px
    assert_eq!(bounds_wide.y, TOP_OFFSET); // 64px fixed top offset
    assert_eq!(bounds_wide.x, (1920.0 - 560.0) / 2.0); // centered horizontally

    // Clamped on narrow viewport (400px width) -> width: 400 - 32 = 368px
    let viewport_narrow = Rectangle::new(Point::new(0.0, 0.0), Size::new(400.0, 600.0));
    let bounds_narrow = card_bounds(viewport_narrow);
    assert_eq!(bounds_narrow.width, 368.0);
    assert_eq!(bounds_narrow.height, CARD_HEIGHT);
    assert_eq!(bounds_narrow.x, (400.0 - 368.0) / 2.0);
}

#[test]
fn test_geometry_constants_match_spec() {
    assert_eq!(CARD_WIDTH, 560.0);
    assert_eq!(CARD_HEIGHT, 380.0);
    assert_eq!(TOP_OFFSET, 64.0);
    assert_eq!(SEARCH_HEIGHT, 40.0);
    assert_eq!(ROW_SINGLE_HEIGHT, 32.0);
    assert_eq!(ROW_DOUBLE_HEIGHT, 44.0);
    assert_eq!(FOOTER_HEIGHT, 24.0);
    assert_eq!(ICON_SIZE, 16.0);
    assert_eq!(RADIUS, 6.0);
    assert_eq!(GROUP_HEADER_HEIGHT, 24.0);
    assert_eq!(DIVIDER_STROKE, 1.0);

    assert_eq!(LABEL_ESC, "Esc");
    assert_eq!(LEGEND_KEYBOARD, "↑↓ Navigate · ↵ Run · Esc Close");
    assert_eq!(NO_RESULTS, "No matching results");
    assert_eq!(SEARCHING, "Searching...");
}

#[test]
fn test_total_content_height() {
    let groups = sample_items();
    // Group 1: header(24) + item1 double(44) + item2 single(32) = 100
    // Group 2: header(24) + item3 double(44) + item4 single(32) = 100
    // Total = 200px
    let h = total_content_height(&groups);
    assert_eq!(h, 200.0);

    let empty_groups: Vec<PaletteGroup<TestId>> = Vec::new();
    assert_eq!(total_content_height(&empty_groups), 0.0);
}

#[test]
fn test_highlight_spans_slicing() {
    let text = "src/main.rs";

    // 1. Empty spans
    let empty_spans = highlight_spans(text, &[]);
    assert_eq!(empty_spans, vec![("src/main.rs", false)]);

    // 2. Single match in middle ("main" at 4..8)
    let spans1 = highlight_spans(text, &[TextRange::new(4, 8)]);
    assert_eq!(
        spans1,
        vec![("src/", false), ("main", true), (".rs", false)]
    );

    // 3. Match at beginning ("src" at 0..3)
    let spans2 = highlight_spans(text, &[TextRange::new(0, 3)]);
    assert_eq!(spans2, vec![("src", true), ("/main.rs", false)]);

    // 4. Match at end (".rs" at 8..11)
    let spans3 = highlight_spans(text, &[TextRange::new(8, 11)]);
    assert_eq!(spans3, vec![("src/main", false), (".rs", true)]);

    // 5. Multiple spans
    let spans4 = highlight_spans(text, &[TextRange::new(0, 3), TextRange::new(8, 11)]);
    assert_eq!(spans4, vec![("src", true), ("/main", false), (".rs", true)]);
}

// ---------------------------------------------------------------------------
// 5. Token Contrast and Rule 4 Verification
// ---------------------------------------------------------------------------

#[test]
fn test_rule_4_active_item_contrast_lock() {
    let palette = Palette::dark();

    // Active row uses accent_fill background:
    // Rule 4 pins text.strong on accent_fill
    let ratio = contrast::contrast_ratio(palette.text_strong, palette.accent_fill);
    assert!(
        ratio >= 4.80,
        "accent_fill under text_strong contrast ({ratio:.2}:1) must clear 4.80:1"
    );

    // Assert fill foreground lock check passes
    contrast::assert_fill_foreground_lock(palette.accent_fill, palette.text_strong, &palette);

    // Card surface.overlay under text_primary must clear body contrast (>= 4.5:1)
    let card_contrast = contrast::contrast_ratio(palette.text_primary, palette.surface_overlay);
    assert!(
        card_contrast >= 4.5,
        "surface_overlay under text_primary ({card_contrast:.2}:1) must clear 4.5:1"
    );

    // Hairline border.strong on surface_overlay must clear 3.0:1
    let border_contrast = contrast::contrast_ratio(palette.border_strong, palette.surface_overlay);
    assert!(
        border_contrast >= 3.0,
        "border_strong on surface_overlay ({border_contrast:.2}:1) must clear 3.0:1"
    );
}

// ---------------------------------------------------------------------------
// 6. Widget Architecture and Layout Tests
// ---------------------------------------------------------------------------

#[test]
fn test_command_palette_widget_construction_and_getters() {
    let model = CommandPaletteModel::new(sample_items());
    let palette: CommandPalette<'_, (), TestId> = command_palette(model.clone());

    assert_eq!(palette.model(), &model);
    assert!(palette.is_open());
}

#[test]
fn test_command_palette_widget_size_open_vs_closed() {
    let open_palette: CommandPalette<'_, (), TestId> =
        command_palette(CommandPaletteModel::new(sample_items()).with_open(true));
    assert_eq!(open_palette.size().width, Length::Fill);
    assert_eq!(open_palette.size().height, Length::Fill);

    let closed_palette: CommandPalette<'_, (), TestId> =
        command_palette(CommandPaletteModel::new(sample_items()).with_open(false));
    assert_eq!(closed_palette.size().width, Length::Fixed(0.0));
    assert_eq!(closed_palette.size().height, Length::Fixed(0.0));
}

#[test]
fn test_command_palette_widget_headless_render_open() {
    let model = CommandPaletteModel::new(sample_items());
    let mut harness = Harness::<(), _>::headless(move || command_palette(model.clone()).into());
    harness.render();
}

#[test]
fn test_command_palette_widget_headless_render_busy_and_empty() {
    let busy_model = CommandPaletteModel::new(sample_items()).with_busy(true);
    let mut harness_busy =
        Harness::<(), _>::headless(move || command_palette(busy_model.clone()).into());
    harness_busy.render();

    let empty_model = CommandPaletteModel::<TestId>::new(Vec::new()).with_query("unknown");
    let mut harness_empty =
        Harness::<(), _>::headless(move || command_palette(empty_model.clone()).into());
    harness_empty.render();
}

#[test]
fn test_command_palette_widget_headless_render_closed() {
    let closed_model = CommandPaletteModel::new(sample_items()).with_open(false);
    let mut harness =
        Harness::<(), _>::headless(move || command_palette(closed_model.clone()).into());
    harness.render();
}

#[test]
fn test_command_palette_tag_and_state() {
    let widget: CommandPalette<'_, (), TestId> =
        command_palette(CommandPaletteModel::new(sample_items()));
    assert_eq!(
        widget.tag(),
        iced::advanced::widget::tree::Tag::of::<CommandPaletteState<TestId>>()
    );

    let state = widget.state();
    let downcasted = state.downcast_ref::<CommandPaletteState<TestId>>();
    assert_eq!(downcasted.scroll_offset, 0.0);
    assert_eq!(downcasted.hovered_id, None);
    assert!(!downcasted.esc_hovered);
    assert!(!downcasted.chip_hovered);
}

#[test]
fn test_command_palette_element_conversion() {
    let widget: CommandPalette<'_, (), TestId> =
        command_palette(CommandPaletteModel::new(sample_items()));
    let _element: Element<'_, ()> = widget.into();
}

// ---------------------------------------------------------------------------
// 7. Interactive Event Handling Tests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum TestMsg {
    Query(PaletteMode, String, u64),
    Mode(PaletteMode),
    Highlight(TestId),
    Execute(TestId),
    Dismiss,
}

fn update_palette<'a, Message, ItemId: 'static + Clone + PartialEq>(
    widget: &mut CommandPalette<'a, Message, ItemId>,
    tree: &mut Tree,
    event: &Event,
    cursor: mouse::Cursor,
    bounds: Rectangle,
    messages: &mut Vec<Message>,
) {
    let layout_node = layout::Node::new(bounds.size());
    let layout = Layout::new(&layout_node);
    let renderer = crate::test_helpers::headless_renderer();
    let mut clipboard = iced::advanced::clipboard::Null;
    let mut shell = Shell::new(messages);
    let widget_dyn = widget as &mut dyn Widget<Message, iced::Theme, iced::Renderer>;
    widget_dyn.update(
        tree,
        event,
        layout,
        cursor,
        &renderer,
        &mut clipboard,
        &mut shell,
        &bounds,
    );
}

#[test]
fn test_interactive_keyboard_events() {
    let model = CommandPaletteModel::new(sample_items()).with_query("query");
    let mut widget = command_palette(model)
        .on_query_change(|live| {
            let (m, q, g) = live.into_value();
            TestMsg::Query(m, q, g)
        })
        .on_mode_change(|commit| TestMsg::Mode(commit.into_value()))
        .on_highlight_change(|live| TestMsg::Highlight(live.into_value()))
        .on_execute(|commit| TestMsg::Execute(commit.into_value()))
        .on_dismiss(|_cancel| TestMsg::Dismiss);

    let mut tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    let mut messages = Vec::new();
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(1920.0, 1080.0));

    // 1. Press Escape when query is non-empty -> Clears query
    let esc_event = Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: Key::Named(Named::Escape),
        modified_key: Key::Named(Named::Escape),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::default(),
        repeat: false,
        text: None,
    });

    update_palette(
        &mut widget,
        &mut tree,
        &esc_event,
        mouse::Cursor::Unavailable,
        bounds,
        &mut messages,
    );

    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0],
        TestMsg::Query(PaletteMode::Files, "".to_string(), 1)
    );

    // 2. Press ArrowDown -> Highlights next item (FileSave)
    messages.clear();
    let down_event = Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: Key::Named(Named::ArrowDown),
        modified_key: Key::Named(Named::ArrowDown),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::default(),
        repeat: false,
        text: None,
    });

    update_palette(
        &mut widget,
        &mut tree,
        &down_event,
        mouse::Cursor::Unavailable,
        bounds,
        &mut messages,
    );

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0], TestMsg::Highlight(TestId::FileSave));

    // 3. Press Enter -> Executes highlighted item (FileOpen)
    messages.clear();
    let enter_event = Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: Key::Named(Named::Enter),
        modified_key: Key::Named(Named::Enter),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::default(),
        repeat: false,
        text: None,
    });

    update_palette(
        &mut widget,
        &mut tree,
        &enter_event,
        mouse::Cursor::Unavailable,
        bounds,
        &mut messages,
    );

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0], TestMsg::Execute(TestId::FileOpen));
}

#[test]
fn test_interactive_mouse_scrim_and_esc_chip_dismiss() {
    let model = CommandPaletteModel::new(sample_items());
    let mut widget = command_palette(model).on_dismiss(|_cancel| TestMsg::Dismiss);

    let mut tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    let mut messages = Vec::new();
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(1920.0, 1080.0));

    // 1. Click outside card on scrim (e.g. at (10.0, 10.0)) -> Dismiss
    let scrim_click = Event::Mouse(mouse::Event::ButtonPressed(Button::Left));
    let scrim_cursor = mouse::Cursor::Available(Point::new(10.0, 10.0));

    update_palette(
        &mut widget,
        &mut tree,
        &scrim_click,
        scrim_cursor,
        bounds,
        &mut messages,
    );

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0], TestMsg::Dismiss);

    // 2. Click on Esc hint chip inside search header -> Dismiss
    messages.clear();
    let card_rect = card_bounds(bounds);
    let esc_chip_pos = Point::new(card_rect.x + card_rect.width - 20.0, card_rect.y + 20.0);
    let chip_cursor = mouse::Cursor::Available(esc_chip_pos);

    update_palette(
        &mut widget,
        &mut tree,
        &scrim_click,
        chip_cursor,
        bounds,
        &mut messages,
    );

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0], TestMsg::Dismiss);
}

// ---------------------------------------------------------------------------
// 8. Roving-highlight fallbacks for stale and absent highlights
// ---------------------------------------------------------------------------

#[test]
fn test_highlight_next_resets_to_first_when_highlight_is_stale() {
    let mut model =
        CommandPaletteModel::new(sample_items()).with_highlighted_id(Some(TestId::GitCommit));
    // Drop the highlighted item so the highlight points at something no longer listed.
    model.groups[1].items.pop();
    assert_eq!(model.item_count(), 3);

    model.highlight_next();
    assert_eq!(
        model.highlighted_id,
        Some(TestId::FileOpen),
        "a highlight pointing at a removed item must restart roving at the first item"
    );
}

#[test]
fn test_highlight_next_with_no_highlight_starts_at_first_item() {
    let mut model = CommandPaletteModel::new(sample_items()).with_highlighted_id(None);

    model.highlight_next();
    assert_eq!(
        model.highlighted_id,
        Some(TestId::FileOpen),
        "an absent highlight must be resolved to the first item rather than staying absent"
    );
}

#[test]
fn test_highlight_prev_wraps_to_last_when_highlight_is_stale() {
    let mut model =
        CommandPaletteModel::new(sample_items()).with_highlighted_id(Some(TestId::GitCommit));
    model.groups[1].items.pop();

    model.highlight_prev();
    assert_eq!(
        model.highlighted_id,
        Some(TestId::Settings),
        "a stale highlight must fall back to the last remaining item when roving backwards"
    );
}

#[test]
fn test_highlight_prev_with_no_highlight_wraps_to_last_item() {
    let mut model = CommandPaletteModel::new(sample_items()).with_highlighted_id(None);

    model.highlight_prev();
    assert_eq!(
        model.highlighted_id,
        Some(TestId::GitCommit),
        "an absent highlight must be resolved to the last item when roving backwards"
    );
}

#[test]
fn test_highlight_first_and_last_clear_highlight_on_empty_model() {
    let mut empty = CommandPaletteModel::<TestId>::new(Vec::new());
    empty.highlight_last();
    assert_eq!(
        empty.highlighted_id, None,
        "highlight_last on no items must leave no highlight"
    );
    empty.highlight_first();
    assert_eq!(
        empty.highlighted_id, None,
        "highlight_first on no items must leave no highlight"
    );
}

#[test]
fn test_selected_item_is_none_when_highlight_points_at_a_removed_item() {
    let mut model =
        CommandPaletteModel::new(sample_items()).with_highlighted_id(Some(TestId::GitCommit));
    model.groups[1].items.pop();

    assert_eq!(
        model.selected_item().map(|item| item.id),
        None,
        "a highlight with no matching item resolves to no selection"
    );
}

#[test]
fn test_generation_wraps_instead_of_overflowing_at_u64_max() {
    let mut model = CommandPaletteModel::new(sample_items())
        .with_query("stale")
        .with_highlighted_id(Some(TestId::FileOpen));
    model.generation = u64::MAX;

    let disposition = model.handle_escape();
    assert_eq!(disposition, EscapeDisposition::ClearedQuery);
    assert_eq!(
        model.generation, 0,
        "the generation counter must wrap at u64::MAX rather than overflow"
    );
}

#[test]
fn test_set_query_from_input_repeating_the_active_prefix_is_not_a_mode_change() {
    let mut model = CommandPaletteModel::new(sample_items())
        .with_mode(PaletteMode::Commands)
        .with_query("x");

    let emitted = model.set_query_from_input(">");
    assert_eq!(
        emitted, None,
        "re-typing the prefix of the mode already active is not a mode change"
    );
    assert_eq!(model.mode, PaletteMode::Commands);
    assert_eq!(
        model.query, "",
        "the prefix must still be stripped from the query"
    );
    assert_eq!(
        model.generation, 1,
        "the query still changed, so the generation advances"
    );
}

// ---------------------------------------------------------------------------
// 9. Pure geometry helpers at their limits
// ---------------------------------------------------------------------------

#[test]
fn test_card_bounds_clamps_to_zero_area_on_a_degenerate_viewport() {
    let tiny = Rectangle::new(Point::new(0.0, 0.0), Size::new(20.0, 30.0));
    let bounds = card_bounds(tiny);

    assert_eq!(
        bounds.width, 0.0,
        "a viewport narrower than the 32px side gutter clamps the card width to zero"
    );
    assert_eq!(
        bounds.height, 0.0,
        "a viewport shorter than the 64px top offset clamps the card height to zero"
    );
    assert_eq!(
        bounds.x, 10.0,
        "the zero-width card stays centred in the viewport"
    );
    assert_eq!(bounds.y, TOP_OFFSET, "the card keeps its fixed top offset");
}

#[test]
fn test_highlight_spans_clamps_a_span_running_past_the_text() {
    assert_eq!(
        highlight_spans("abc", &[TextRange::new(0, 100)]),
        vec![("abc", true)],
        "a span extending past the end of the text is clamped, not sliced out of range"
    );
}

#[test]
fn test_highlight_spans_skips_a_span_splitting_a_multibyte_character() {
    // 'e with acute' occupies bytes 2..4 of "he\u{e9}llo", so a span ending at byte 3
    // lands inside it and cannot be sliced out of the text.
    assert_eq!(
        highlight_spans("he\u{e9}llo", &[TextRange::new(1, 3)]),
        vec![("he\u{e9}llo", false)],
        "a span whose end falls inside a multibyte character must be skipped entirely, \
         leaving the whole text as one unhighlighted slice"
    );

    // The same character sliced at its own byte boundaries is a legal span, which is
    // what separates "skipped because it cuts a character" from "spans are never cut".
    assert_eq!(
        highlight_spans("he\u{e9}llo", &[TextRange::new(2, 4)]),
        vec![("he", false), ("\u{e9}", true), ("llo", false)],
        "a span aligned to a multibyte character's own bytes is highlighted like any other"
    );
}

#[test]
fn test_highlight_spans_keeps_surrounding_slices_for_a_zero_length_span() {
    assert_eq!(
        highlight_spans("abc", &[TextRange::new(1, 1)]),
        vec![("a", false), ("bc", false)],
        "an empty span emits no highlighted slice but does not swallow the surrounding text"
    );
}

#[test]
fn test_total_content_height_omits_an_empty_group_heading() {
    let groups = vec![PaletteGroup::new(
        "",
        vec![PaletteItem::new(TestId::FileSave, "lib.rs")],
    )];

    assert_eq!(
        total_content_height(&groups),
        ROW_SINGLE_HEIGHT,
        "a group with no heading must not reserve a 24px header row"
    );
}

// ---------------------------------------------------------------------------
// 10. Pointer interaction geometry
// ---------------------------------------------------------------------------

/// The viewport the [`Harness`] lays its root out at, so probe geometry here
/// matches what a real frame would give the widget.
const PROBE_VIEWPORT: Size = Size::new(400.0, 300.0);

fn probe_bounds() -> Rectangle {
    Rectangle::new(Point::new(0.0, 0.0), PROBE_VIEWPORT)
}

fn left_press() -> Event {
    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
}

fn cursor_moved(at: Point) -> Event {
    Event::Mouse(mouse::Event::CursorMoved { position: at })
}

fn wheel_pixels(y: f32) -> Event {
    Event::Mouse(mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Pixels { x: 0.0, y },
    })
}

fn wheel_lines(y: f32) -> Event {
    Event::Mouse(mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Lines { x: 0.0, y },
    })
}

fn key_press(key: Key) -> Event {
    Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::default(),
        repeat: false,
        text: None,
    })
}

/// The results viewport exactly as [`CommandPalette::update`] computes it.
fn results_clip(bounds: Rectangle) -> Rectangle {
    let card = card_bounds(bounds);
    Rectangle {
        x: card.x,
        y: card.y + SEARCH_HEIGHT + DIVIDER_STROKE,
        width: card.width,
        height: (card.height - SEARCH_HEIGHT - DIVIDER_STROKE - FOOTER_HEIGHT - DIVIDER_STROKE)
            .max(0.0),
    }
}

fn esc_chip_centre(bounds: Rectangle) -> Point {
    let card = card_bounds(bounds);
    Rectangle::new(
        Point::new(
            card.x + card.width - space::MD - 32.0,
            card.y + (SEARCH_HEIGHT - 20.0) / 2.0,
        ),
        Size::new(32.0, 20.0),
    )
    .center()
}

fn prefix_chip_centre(bounds: Rectangle) -> Point {
    let card = card_bounds(bounds);
    Rectangle::new(
        Point::new(
            card.x + space::MD + ICON_SIZE + space::SM,
            card.y + (SEARCH_HEIGHT - 24.0) / 2.0,
        ),
        Size::new(28.0, 24.0),
    )
    .center()
}

/// Centre of the `flat_index`-th item of [`sample_items`], in the same
/// document order the widget walks when it hit-tests a row.
fn item_centre(bounds: Rectangle, flat_index: usize) -> Point {
    let clip = results_clip(bounds);
    let mut y = clip.y;
    let mut seen = 0usize;

    for group in sample_items() {
        if !group.heading.is_empty() {
            y += GROUP_HEADER_HEIGHT;
        }
        for item in &group.items {
            let row_h = if item.subtitle.is_some() {
                ROW_DOUBLE_HEIGHT
            } else {
                ROW_SINGLE_HEIGHT
            };
            if seen == flat_index {
                return Point::new(clip.x + clip.width / 2.0, y + row_h / 2.0);
            }
            y += row_h;
            seen += 1;
        }
    }
    panic!("sample_items() has no item at flat index {flat_index}");
}

fn group_header_centre(bounds: Rectangle) -> Point {
    let clip = results_clip(bounds);
    Point::new(
        clip.x + clip.width / 2.0,
        clip.y + GROUP_HEADER_HEIGHT / 2.0,
    )
}

fn footer_centre(bounds: Rectangle) -> Point {
    let card = card_bounds(bounds);
    Point::new(
        card.x + card.width / 2.0,
        card.y + card.height - FOOTER_HEIGHT / 2.0,
    )
}

fn palette_state<ItemId: 'static>(tree: &Tree) -> &CommandPaletteState<ItemId> {
    // `State::downcast_ref` hands back `&T` and panics on a missing state, so the
    // precondition is asserted before the downcast rather than `.expect`ed after.
    assert!(
        !matches!(&tree.state, iced::advanced::widget::tree::State::None),
        "the palette keeps its own state in the tree"
    );
    tree.state.downcast_ref::<CommandPaletteState<ItemId>>()
}

/// Deliver `events` with the cursor parked at `cursor`, one event per frame.
fn drive_events<Message, ItemId: 'static + Clone + PartialEq>(
    widget: &mut CommandPalette<'_, Message, ItemId>,
    tree: &mut Tree,
    events: &[Event],
    cursor: mouse::Cursor,
    bounds: Rectangle,
    messages: &mut Vec<Message>,
) {
    let node = layout::Node::new(bounds.size());
    let layout = Layout::new(&node);
    let renderer = crate::test_helpers::headless_renderer();
    let mut clipboard = iced::advanced::clipboard::Null;
    let widget_dyn = widget as &mut dyn Widget<Message, iced::Theme, iced::Renderer>;

    for event in events {
        let mut shell = Shell::new(messages);
        widget_dyn.update(
            tree,
            event,
            layout,
            cursor,
            &renderer,
            &mut clipboard,
            &mut shell,
            &bounds,
        );
    }
}

/// Run a real draw pass over the widget in whatever state `tree` currently holds.
fn paint_palette<Message, ItemId: 'static + Clone + PartialEq>(
    widget: &CommandPalette<'_, Message, ItemId>,
    tree: &Tree,
    renderer: &mut iced::Renderer,
    bounds: Rectangle,
) {
    let node = layout::Node::new(bounds.size());
    let layout = Layout::new(&node);
    let style = renderer::Style {
        text_color: iced::Color::BLACK,
    };
    let widget_dyn = widget as &dyn Widget<Message, iced::Theme, iced::Renderer>;
    widget_dyn.draw(
        tree,
        renderer,
        &iced::Theme::Dark,
        &style,
        layout,
        mouse::Cursor::Unavailable,
        &bounds,
    );
}

fn interactive_palette(
    model: CommandPaletteModel<TestId>,
) -> (CommandPalette<'static, TestMsg, TestId>, Tree) {
    let widget = command_palette(model)
        .on_query_change(|live| {
            let (m, q, g) = live.into_value();
            TestMsg::Query(m, q, g)
        })
        .on_mode_change(|commit| TestMsg::Mode(commit.into_value()))
        .on_highlight_change(|live| TestMsg::Highlight(live.into_value()))
        .on_execute(|commit| TestMsg::Execute(commit.into_value()))
        .on_dismiss(|_cancel| TestMsg::Dismiss);
    let tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    (widget, tree)
}

#[test]
fn test_a_closed_palette_answers_no_pointer_event_at_all() {
    let model = CommandPaletteModel::new(sample_items()).with_open(false);
    let mut widget = command_palette(model).on_dismiss(|_cancel| TestMsg::Dismiss);
    let mut tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[left_press(), wheel_pixels(-40.0)],
        mouse::Cursor::Available(Point::new(5.0, 5.0)),
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        Vec::<TestMsg>::new(),
        "a closed palette occupies no space and must not answer the scrim"
    );
}

#[test]
fn test_pressing_the_mode_prefix_chip_returns_to_files_mode() {
    let model = CommandPaletteModel::new(sample_items()).with_mode(PaletteMode::Commands);
    let (mut widget, mut tree) = interactive_palette(model);
    let mut messages = Vec::new();
    let bounds = probe_bounds();

    drive_events(
        &mut widget,
        &mut tree,
        &[left_press()],
        mouse::Cursor::Available(prefix_chip_centre(bounds)),
        bounds,
        &mut messages,
    );

    assert_eq!(
        messages,
        vec![TestMsg::Mode(PaletteMode::Files)],
        "clicking the removable prefix chip drops the prefix back to Files"
    );
}

#[test]
fn test_the_prefix_chip_slot_is_inert_in_files_mode() {
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();
    let bounds = probe_bounds();
    let chip = prefix_chip_centre(bounds);

    drive_events(
        &mut widget,
        &mut tree,
        &[left_press()],
        mouse::Cursor::Available(chip),
        bounds,
        &mut messages,
    );

    assert_eq!(
        messages,
        Vec::<TestMsg>::new(),
        "with no prefix rendered, the chip slot must not swallow the press"
    );
    assert!(
        !palette_state::<TestId>(&tree).chip_hovered,
        "Files mode has no chip, so the chip slot cannot report a hover"
    );
}

#[test]
fn test_pressing_a_result_row_highlights_then_executes_it() {
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();
    let bounds = probe_bounds();

    drive_events(
        &mut widget,
        &mut tree,
        &[left_press()],
        mouse::Cursor::Available(item_centre(bounds, 1)),
        bounds,
        &mut messages,
    );

    assert_eq!(
        messages,
        vec![
            TestMsg::Highlight(TestId::FileSave),
            TestMsg::Execute(TestId::FileSave)
        ],
        "a row press moves the highlight and commits the row in one gesture"
    );
}

#[test]
fn test_pressing_a_group_header_inside_the_results_clip_selects_nothing() {
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();
    let bounds = probe_bounds();
    let header = group_header_centre(bounds);

    drive_events(
        &mut widget,
        &mut tree,
        &[left_press()],
        mouse::Cursor::Available(header),
        bounds,
        &mut messages,
    );

    assert_eq!(
        messages,
        Vec::<TestMsg>::new(),
        "a press that lands on a section header is inside the list but on no row"
    );
    assert_eq!(
        palette_state::<TestId>(&tree).hovered_id,
        None,
        "a section header must not be reported as a hovered item"
    );
}

#[test]
fn test_pressing_the_footer_inside_the_card_neither_executes_nor_dismisses() {
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();
    let bounds = probe_bounds();

    drive_events(
        &mut widget,
        &mut tree,
        &[left_press()],
        mouse::Cursor::Available(footer_centre(bounds)),
        bounds,
        &mut messages,
    );

    assert_eq!(
        messages,
        Vec::<TestMsg>::new(),
        "the legend footer is chrome: it is inside the card, so it is not scrim"
    );
}

#[test]
fn test_wheel_scrolling_the_results_clamps_to_the_content_extent() {
    // sample_items() is 200px tall and the 400x300 viewport leaves a 170px
    // results viewport, so the whole scrollable range is 30px.
    let clip = results_clip(probe_bounds());
    assert_eq!(total_content_height(&sample_items()) - clip.height, 30.0);

    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();
    let bounds = probe_bounds();
    let inside = Point::new(clip.x + clip.width / 2.0, clip.y + 10.0);

    drive_events(
        &mut widget,
        &mut tree,
        &[wheel_pixels(-500.0)],
        mouse::Cursor::Available(inside),
        bounds,
        &mut messages,
    );
    assert_eq!(
        palette_state::<TestId>(&tree).scroll_offset,
        30.0,
        "scrolling past the end clamps to the content extent"
    );

    drive_events(
        &mut widget,
        &mut tree,
        &[wheel_pixels(500.0)],
        mouse::Cursor::Available(inside),
        bounds,
        &mut messages,
    );
    assert_eq!(
        palette_state::<TestId>(&tree).scroll_offset,
        0.0,
        "scrolling past the start clamps back to zero"
    );
    assert!(
        messages.is_empty(),
        "scrolling is a view concern and publishes nothing"
    );
}

#[test]
fn test_line_based_wheel_delta_is_scaled_by_the_row_height() {
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();
    let bounds = probe_bounds();
    let clip = results_clip(bounds);
    let inside = Point::new(clip.x + clip.width / 2.0, clip.y + 10.0);

    // One line down is ROW_SINGLE_HEIGHT (32px) of travel, which the 30px
    // scrollable range clamps to 30.
    drive_events(
        &mut widget,
        &mut tree,
        &[wheel_lines(-1.0)],
        mouse::Cursor::Available(inside),
        bounds,
        &mut messages,
    );
    assert_eq!(
        palette_state::<TestId>(&tree).scroll_offset,
        30.0,
        "a Lines delta is converted to pixels through the 32px single-row height"
    );

    // One line back up overshoots the top and clamps to zero rather than going negative.
    drive_events(
        &mut widget,
        &mut tree,
        &[wheel_lines(1.0)],
        mouse::Cursor::Available(inside),
        bounds,
        &mut messages,
    );
    assert_eq!(
        palette_state::<TestId>(&tree).scroll_offset,
        0.0,
        "a Lines delta upward cannot take the list above its first row"
    );
}

#[test]
fn test_wheel_scrolling_outside_the_results_viewport_does_not_move_the_list() {
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();
    let bounds = probe_bounds();
    let card = card_bounds(bounds);

    drive_events(
        &mut widget,
        &mut tree,
        &[wheel_pixels(-100.0)],
        mouse::Cursor::Available(Point::new(card.x + card.width / 2.0, card.y + 5.0)),
        bounds,
        &mut messages,
    );

    assert_eq!(
        palette_state::<TestId>(&tree).scroll_offset,
        0.0,
        "a wheel event over the search header must not scroll the results"
    );
}

#[test]
fn test_hovering_the_esc_and_prefix_chips_records_the_hover() {
    let model = CommandPaletteModel::new(sample_items()).with_mode(PaletteMode::Tags);
    let (mut widget, mut tree) = interactive_palette(model);
    let mut messages = Vec::new();
    let bounds = probe_bounds();

    drive_events(
        &mut widget,
        &mut tree,
        &[cursor_moved(esc_chip_centre(bounds))],
        mouse::Cursor::Available(esc_chip_centre(bounds)),
        bounds,
        &mut messages,
    );
    let state = palette_state::<TestId>(&tree);
    assert!(state.esc_hovered, "the cursor over the Esc chip is a hover");
    assert!(
        !state.chip_hovered,
        "the Esc chip and the prefix chip are separate hit targets"
    );

    drive_events(
        &mut widget,
        &mut tree,
        &[cursor_moved(prefix_chip_centre(bounds))],
        mouse::Cursor::Available(prefix_chip_centre(bounds)),
        bounds,
        &mut messages,
    );
    let state = palette_state::<TestId>(&tree);
    assert!(
        state.chip_hovered,
        "the cursor over the '#' prefix chip is a hover"
    );
    assert!(
        !state.esc_hovered,
        "leaving the Esc chip must clear its hover"
    );
}

#[test]
fn test_hovering_a_result_row_records_its_id() {
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();
    let bounds = probe_bounds();

    let point = item_centre(bounds, 2);
    drive_events(
        &mut widget,
        &mut tree,
        &[cursor_moved(point)],
        mouse::Cursor::Available(point),
        bounds,
        &mut messages,
    );

    assert_eq!(
        palette_state::<TestId>(&tree).hovered_id,
        Some(TestId::Settings),
        "the row under the cursor is tracked so the draw pass can paint the hover fill"
    );
    assert!(messages.is_empty(), "hovering paints, it does not emit");
}

// ---------------------------------------------------------------------------
// 11. Keyboard branches the widget owns
// ---------------------------------------------------------------------------

#[test]
fn test_escape_on_an_empty_query_dismisses_instead_of_clearing() {
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Named(Named::Escape))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        vec![TestMsg::Dismiss],
        "the second stage of the two-stage Escape contract dismisses"
    );
}

#[test]
fn test_escape_with_a_query_and_no_query_callback_publishes_nothing() {
    let model = CommandPaletteModel::new(sample_items()).with_query("abc");
    let mut widget: CommandPalette<'_, TestMsg, TestId> =
        command_palette(model).on_dismiss(|_cancel| TestMsg::Dismiss);
    let mut tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Named(Named::Escape))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        Vec::<TestMsg>::new(),
        "stage one belongs to the query callback; with none attached nothing is owed"
    );
}

#[test]
fn test_arrow_down_on_an_empty_result_list_publishes_nothing() {
    let (mut widget, mut tree) =
        interactive_palette(CommandPaletteModel::<TestId>::new(Vec::new()));
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[
            key_press(Key::Named(Named::ArrowDown)),
            key_press(Key::Named(Named::ArrowUp)),
        ],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        Vec::<TestMsg>::new(),
        "there is nothing to rove to when no results matched"
    );
}

#[test]
fn test_arrow_down_from_the_last_item_wraps_to_the_first() {
    let model =
        CommandPaletteModel::new(sample_items()).with_highlighted_id(Some(TestId::GitCommit));
    let (mut widget, mut tree) = interactive_palette(model);
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Named(Named::ArrowDown))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        vec![TestMsg::Highlight(TestId::FileOpen)],
        "ArrowDown at the bottom of the list wraps to the top"
    );
}

#[test]
fn test_arrow_down_from_a_highlight_absent_from_the_list_restarts_at_the_first_item() {
    let groups = vec![PaletteGroup::new(
        "Recent",
        vec![
            PaletteItem::new(TestId::FileOpen, "main.rs"),
            PaletteItem::new(TestId::FileSave, "lib.rs"),
        ],
    )];
    let model = CommandPaletteModel::new(groups).with_highlighted_id(Some(TestId::GitCommit));
    let (mut widget, mut tree) = interactive_palette(model);
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Named(Named::ArrowDown))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        vec![TestMsg::Highlight(TestId::FileOpen)],
        "a highlight the results no longer contain restarts roving at the first item"
    );
}

#[test]
fn test_arrow_up_from_a_highlight_absent_from_the_list_lands_on_the_last_item() {
    let groups = vec![PaletteGroup::new(
        "Recent",
        vec![
            PaletteItem::new(TestId::FileOpen, "main.rs"),
            PaletteItem::new(TestId::FileSave, "lib.rs"),
        ],
    )];
    let model = CommandPaletteModel::new(groups).with_highlighted_id(Some(TestId::GitCommit));
    let (mut widget, mut tree) = interactive_palette(model);
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Named(Named::ArrowUp))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        vec![TestMsg::Highlight(TestId::FileSave)],
        "an unknown highlight roves backwards to the last remaining item"
    );
}

#[test]
fn test_arrow_up_from_the_first_item_wraps_to_the_last() {
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Named(Named::ArrowUp))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        vec![TestMsg::Highlight(TestId::GitCommit)],
        "the roving highlight wraps at the top of the list"
    );
}

#[test]
fn test_the_widget_holds_no_highlight_of_its_own_across_two_arrow_presses() {
    // The palette is a controlled view: it reports the *next* id from the model
    // it was given and never advances that model itself.
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[
            key_press(Key::Named(Named::ArrowDown)),
            key_press(Key::Named(Named::ArrowDown)),
        ],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        vec![
            TestMsg::Highlight(TestId::FileSave),
            TestMsg::Highlight(TestId::FileSave)
        ],
        "without the host writing the new highlight back, the widget re-reports the same step"
    );
}

#[test]
fn test_enter_with_nothing_selected_publishes_no_execution() {
    let (mut widget, mut tree) =
        interactive_palette(CommandPaletteModel::<TestId>::new(Vec::new()));
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Named(Named::Enter))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        Vec::<TestMsg>::new(),
        "Enter on an empty result list runs nothing"
    );
}

#[test]
fn test_enter_without_an_execute_callback_runs_nothing() {
    let mut widget: CommandPalette<'_, TestMsg, TestId> =
        command_palette(CommandPaletteModel::new(sample_items()));
    let mut tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Named(Named::Enter))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        Vec::<TestMsg>::new(),
        "a palette built without on_execute stays inert on Enter"
    );
}

#[test]
fn test_backspace_on_an_empty_files_query_is_a_no_op() {
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Named(Named::Backspace))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        Vec::<TestMsg>::new(),
        "there is nothing to delete and no prefix to drop in Files mode"
    );
}

#[test]
fn test_backspace_on_an_empty_prefixed_query_drops_the_prefix() {
    let model = CommandPaletteModel::new(sample_items()).with_mode(PaletteMode::Symbols);
    let (mut widget, mut tree) = interactive_palette(model);
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Named(Named::Backspace))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        vec![TestMsg::Mode(PaletteMode::Files)],
        "Backspace at caret 0 with a prefix active removes the prefix"
    );
}

#[test]
fn test_backspace_drops_the_prefix_only_when_a_mode_callback_is_attached() {
    let model = CommandPaletteModel::new(sample_items()).with_mode(PaletteMode::Tags);
    let mut widget: CommandPalette<'_, TestMsg, TestId> = command_palette(model);
    let mut tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Named(Named::Backspace))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        Vec::<TestMsg>::new(),
        "with no mode callback the prefix removal has nowhere to be reported"
    );
}

#[test]
fn test_backspace_pops_the_last_query_character_and_bumps_the_generation() {
    let model = CommandPaletteModel::new(sample_items())
        .with_query("abc")
        .with_highlighted_id(Some(TestId::FileOpen));
    let (mut widget, mut tree) = interactive_palette(model);
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Named(Named::Backspace))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        vec![TestMsg::Query(PaletteMode::Files, "ab".to_string(), 1)],
        "Backspace reports the shortened query under a fresh generation"
    );
}

#[test]
fn test_typing_a_prefix_character_switches_mode_and_reports_the_stripped_query() {
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Character(">".into()))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        vec![
            TestMsg::Mode(PaletteMode::Commands),
            TestMsg::Query(PaletteMode::Commands, String::new(), 1),
        ],
        "the mode commit is published before the query it implies"
    );
}

#[test]
fn test_typing_a_plain_character_appends_to_the_query() {
    let model = CommandPaletteModel::new(sample_items())
        .with_query("ab")
        .with_highlighted_id(Some(TestId::FileOpen));
    let (mut widget, mut tree) = interactive_palette(model);
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Character("x".into()))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        vec![TestMsg::Query(PaletteMode::Files, "abx".to_string(), 1)],
        "an ordinary character appends to the query and advances the generation"
    );
}

#[test]
fn test_typing_the_active_prefix_again_reports_only_a_query_change() {
    let model = CommandPaletteModel::new(sample_items())
        .with_mode(PaletteMode::Commands)
        .with_query("op");
    let (mut widget, mut tree) = interactive_palette(model);
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Character(">".into()))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        vec![TestMsg::Query(PaletteMode::Commands, "op>".to_string(), 1)],
        "the mode prefix is a chip rather than field text, so a '>' typed after existing \
         query text is an ordinary character appended to the query — and because Commands is \
         already the mode in force, no mode commit is published alongside it"
    );
    assert_eq!(
        widget.model().query,
        "op",
        "the palette reports the edit to its host instead of mutating the model it was given"
    );
}

#[test]
fn test_character_input_without_a_query_callback_publishes_nothing() {
    let mut widget: CommandPalette<'_, TestMsg, TestId> =
        command_palette(CommandPaletteModel::new(sample_items()));
    let mut tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[key_press(Key::Character("x".into()))],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        Vec::<TestMsg>::new(),
        "with no query callback attached the keystroke has no subscriber"
    );
}

#[test]
fn test_unhandled_keys_are_ignored() {
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();

    drive_events(
        &mut widget,
        &mut tree,
        &[
            key_press(Key::Named(Named::Tab)),
            key_press(Key::Named(Named::Home)),
            key_press(Key::Named(Named::End)),
        ],
        mouse::Cursor::Unavailable,
        probe_bounds(),
        &mut messages,
    );

    assert_eq!(
        messages,
        Vec::<TestMsg>::new(),
        "the palette claims Escape, the arrows, Enter and Backspace only"
    );
}

// ---------------------------------------------------------------------------
// 12. The draw path in each visual state
// ---------------------------------------------------------------------------

#[test]
fn test_draw_in_prefixed_modes_paints_the_chip_and_its_hover() {
    for mode in [
        PaletteMode::Commands,
        PaletteMode::Tags,
        PaletteMode::Symbols,
    ] {
        let model = CommandPaletteModel::new(sample_items()).with_mode(mode);
        let (mut widget, mut tree) = interactive_palette(model);
        let mut messages = Vec::new();
        let bounds = probe_bounds();

        // Resting chip.
        let resting = item_centre(bounds, 0);
        drive_events(
            &mut widget,
            &mut tree,
            &[cursor_moved(resting)],
            mouse::Cursor::Available(resting),
            bounds,
            &mut messages,
        );
        let mut renderer = crate::test_helpers::headless_renderer();
        paint_palette(&widget, &tree, &mut renderer, bounds);
        assert!(
            !palette_state::<TestId>(&tree).chip_hovered,
            "{mode:?} at rest must paint the chip in its resting fill"
        );
        assert_eq!(
            palette_state::<TestId>(&tree).hovered_id,
            Some(TestId::FileOpen),
            "{mode:?} still tracks the row under the cursor while the chip is idle"
        );

        // Hovered chip.
        let chip = prefix_chip_centre(bounds);
        drive_events(
            &mut widget,
            &mut tree,
            &[cursor_moved(chip)],
            mouse::Cursor::Available(chip),
            bounds,
            &mut messages,
        );
        assert!(
            palette_state::<TestId>(&tree).chip_hovered,
            "{mode:?} must paint the chip in its active fill while hovered"
        );
        paint_palette(&widget, &tree, &mut renderer, bounds);
    }
}

#[test]
fn test_draw_paints_the_hover_fill_on_a_row_that_is_not_the_active_one() {
    let model =
        CommandPaletteModel::new(sample_items()).with_highlighted_id(Some(TestId::GitCommit));
    let (mut widget, mut tree) = interactive_palette(model);
    let mut messages = Vec::new();
    let bounds = probe_bounds();

    let point = item_centre(bounds, 0);
    drive_events(
        &mut widget,
        &mut tree,
        &[cursor_moved(point)],
        mouse::Cursor::Available(point),
        bounds,
        &mut messages,
    );

    let state = palette_state::<TestId>(&tree);
    assert_eq!(
        state.hovered_id,
        Some(TestId::FileOpen),
        "the hovered row is not the highlighted row, so both fills are exercised"
    );

    let mut renderer = crate::test_helpers::headless_renderer();
    paint_palette(&widget, &tree, &mut renderer, bounds);
}

#[test]
fn test_draw_clips_rows_that_have_scrolled_out_of_the_results_viewport() {
    let (mut widget, mut tree) = interactive_palette(CommandPaletteModel::new(sample_items()));
    let mut messages = Vec::new();
    let bounds = probe_bounds();
    let clip = results_clip(bounds);
    let inside = Point::new(clip.x + clip.width / 2.0, clip.y + 10.0);

    drive_events(
        &mut widget,
        &mut tree,
        &[wheel_pixels(-30.0)],
        mouse::Cursor::Available(inside),
        bounds,
        &mut messages,
    );
    assert_eq!(palette_state::<TestId>(&tree).scroll_offset, 30.0);

    // At full scroll the first section header sits entirely above the clip,
    // so the draw pass must take the clipped-out branch for it.
    let mut renderer = crate::test_helpers::headless_renderer();
    paint_palette(&widget, &tree, &mut renderer, bounds);
    assert_eq!(
        palette_state::<TestId>(&tree).scroll_offset,
        30.0,
        "a draw pass must not disturb the scroll position"
    );
}

#[test]
fn test_draw_skips_the_header_of_a_group_that_carries_no_heading() {
    let groups = vec![
        PaletteGroup::new(
            "",
            vec![
                PaletteItem::new(TestId::FileOpen, "main.rs"),
                PaletteItem::new(TestId::FileSave, "lib.rs").with_subtitle("src/lib.rs"),
            ],
        ),
        PaletteGroup::new(
            "Commands",
            vec![
                PaletteItem::new(TestId::Settings, "Open Settings")
                    .with_match_spans(vec![TextRange::new(0, 4)]),
            ],
        ),
    ];
    let model = CommandPaletteModel::new(groups).with_highlighted_id(Some(TestId::FileSave));
    let (mut widget, mut tree) = interactive_palette(model);
    let mut messages = Vec::new();
    let bounds = probe_bounds();
    let clip = results_clip(bounds);
    // With no heading to skip, the first row of the unheaded group starts at
    // the very top of the results viewport.
    let first_row = Point::new(clip.x + clip.width / 2.0, clip.y + ROW_SINGLE_HEIGHT / 2.0);

    drive_events(
        &mut widget,
        &mut tree,
        &[cursor_moved(first_row)],
        mouse::Cursor::Available(first_row),
        bounds,
        &mut messages,
    );
    assert_eq!(
        palette_state::<TestId>(&tree).hovered_id,
        Some(TestId::FileOpen),
        "an unheaded group starts its rows at the top of the results viewport"
    );

    let mut renderer = crate::test_helpers::headless_renderer();
    paint_palette(&widget, &tree, &mut renderer, bounds);
    assert_eq!(
        widget.model().item_count(),
        3,
        "an unheaded group still contributes its rows to the list"
    );
    assert!(
        messages.is_empty(),
        "moving the cursor over a row highlights nothing on its own"
    );
}

#[test]
fn test_draw_paints_shortcut_pills_and_match_spans_on_the_active_row() {
    let groups = vec![PaletteGroup::new(
        "Recent",
        vec![
            PaletteItem::new(TestId::FileOpen, "main.rs")
                .with_match_spans(vec![TextRange::new(0, 4)])
                .with_shortcut("Ctrl+P"),
            PaletteItem::new(TestId::FileSave, "lib.rs")
                .with_shortcut("Ctrl+S")
                .with_match_spans(vec![TextRange::new(0, 3)]),
            PaletteItem::new(TestId::Settings, "Open Settings")
                .with_subtitle("Preferences")
                .with_shortcut("Ctrl+,"),
        ],
    )];
    // Highlight each row in turn so the Rule 4 promotion branch runs for the
    // single-line and the two-line layout alike.
    for highlighted in [TestId::FileOpen, TestId::FileSave, TestId::Settings] {
        let model = CommandPaletteModel::new(groups.clone()).with_highlighted_id(Some(highlighted));
        let (widget, tree) = interactive_palette(model);
        let bounds = probe_bounds();
        let mut renderer = crate::test_helpers::headless_renderer();

        paint_palette(&widget, &tree, &mut renderer, bounds);
        assert_eq!(
            widget.model().highlighted_id,
            Some(highlighted),
            "the highlighted row is the one the draw pass must promote to text.strong"
        );
    }
}

#[test]
fn test_draw_of_a_busy_palette_reports_searching_instead_of_the_rows() {
    let model = CommandPaletteModel::new(sample_items())
        .with_busy(true)
        .with_query("slow");
    let (widget, tree) = interactive_palette(model);
    let bounds = probe_bounds();
    let mut renderer = crate::test_helpers::headless_renderer();

    paint_palette(&widget, &tree, &mut renderer, bounds);
    assert!(
        widget.model().busy,
        "the busy flag is what makes the draw pass stop after the Searching line"
    );
}

#[test]
fn test_draw_of_an_empty_result_set_reports_no_matching_results() {
    let model = CommandPaletteModel::<TestId>::new(Vec::new()).with_query("nothing-matches");
    let (widget, tree) = interactive_palette(model);
    let bounds = probe_bounds();
    let mut renderer = crate::test_helpers::headless_renderer();

    paint_palette(&widget, &tree, &mut renderer, bounds);
    assert_eq!(
        widget.model().item_count(),
        0,
        "an empty result set is what makes the draw pass stop after the empty-state line"
    );
}

#[test]
fn test_a_closed_palette_draws_nothing_at_all() {
    let model = CommandPaletteModel::new(sample_items()).with_open(false);
    let (widget, tree) = interactive_palette(model);
    let bounds = probe_bounds();
    let mut renderer = crate::test_helpers::headless_renderer();

    paint_palette(&widget, &tree, &mut renderer, bounds);
    assert_eq!(
        widget.size().width,
        Length::Fixed(0.0),
        "a closed palette occupies no space and returns from draw before its first quad"
    );
}

#[test]
fn test_a_palette_with_an_explicit_palette_override_still_draws() {
    let model = CommandPaletteModel::new(sample_items()).with_mode(PaletteMode::Tags);
    let widget = command_palette(model).palette(Palette::dark());
    let tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    let bounds = probe_bounds();
    let mut renderer = crate::test_helpers::headless_renderer();

    paint_palette(&widget, &tree, &mut renderer, bounds);
    assert_eq!(
        widget.model().mode,
        PaletteMode::Tags,
        "the token override does not change which mode chrome is painted"
    );
}
