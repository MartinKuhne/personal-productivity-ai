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
