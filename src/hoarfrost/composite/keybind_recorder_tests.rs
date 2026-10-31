use super::*;
use crate::binding::{Cancel, Commit, Live};
use crate::test_helpers::Candidate;
use crate::test_helpers::{
    Capture, Harness, KIND_CONTAINER, RecordCandidates, assert_repainted, centre, headless_renderer,
};
use crate::token::{shape, size, space};
use iced::Point;
use iced::advanced::widget::operation::black_box;
use iced::advanced::widget::tree::State as TreeState;

#[test]
fn test_key_chord_construction_and_formatting() {
    let empty = KeyChord::default();
    assert_eq!(empty.format(), "");
    assert!(empty.parts().is_empty());

    let single = KeyChord::new(false, false, false, false, "F1");
    assert_eq!(single.parts(), vec!["F1"]);
    assert_eq!(single.format(), "F1");

    let combo = KeyChord::new(true, true, true, false, "P");
    assert_eq!(combo.parts(), vec!["Ctrl", "Alt", "Shift", "P"]);
    assert_eq!(combo.format(), "Ctrl + Alt + Shift + P");

    let super_combo = KeyChord::new(false, false, false, true, "Space");
    assert_eq!(super_combo.parts(), vec!["Super", "Space"]);
    assert_eq!(super_combo.format(), "Super + Space");
}

#[test]
fn test_keybind_recorder_model_defaults_and_builder() {
    let model = KeybindRecorderModel::new();
    assert_eq!(model.chord, None);
    assert!(!model.listening);
    assert_eq!(model.conflict_with, None);
    assert!(!model.disabled);

    let chord = KeyChord::new(true, false, false, false, "S");
    let configured = model
        .with_chord(Some(chord.clone()))
        .with_listening(true)
        .with_conflict(Some("Save File".to_string()))
        .with_disabled(true);

    assert_eq!(configured.chord, Some(chord));
    assert!(configured.listening);
    assert_eq!(configured.conflict_with.as_deref(), Some("Save File"));
    assert!(configured.disabled);
}

#[test]
fn test_geometry_constants_conform_to_spec() {
    assert_eq!(HEIGHT, size::CONTROL_MD);
    assert_eq!(HEIGHT, 32.0);

    assert_eq!(KEYCAP_HEIGHT, size::HIT_TARGET_MIN);
    assert_eq!(KEYCAP_HEIGHT, 24.0);

    assert_eq!(CLEAR_BUTTON_SIZE, size::HIT_TARGET_MIN);
    assert_eq!(CLEAR_BUTTON_SIZE, 24.0);

    assert_eq!(RADIUS, shape::RADIUS_WIDGET);
    assert_eq!(RADIUS, 4.0);

    assert_eq!(BORDER_REST_WIDTH, shape::BORDER_HAIRLINE);
    assert_eq!(BORDER_REST_WIDTH, 1.0);

    assert_eq!(BORDER_FOCUS_WIDTH, shape::BORDER_FOCUS);
    assert_eq!(BORDER_FOCUS_WIDTH, 2.0);

    assert_eq!(GLYPH_CLEAR, "×");
    assert_eq!(LABEL_RECORDING, "Press shortcut keys...");
    assert_eq!(LABEL_EMPTY, "Record shortcut");
    assert_eq!(PLUS_SEPARATOR, "+");
}

#[test]
fn test_build_content_resting_empty_and_with_chord() {
    let empty = KeybindRecorderModel::new();
    let elem_empty: Element<'_, ()> = build_keybind_recorder_content(&empty, None);
    assert_eq!(elem_empty.as_widget().size().width, Length::Fill);

    let with_chord =
        KeybindRecorderModel::new().with_chord(Some(KeyChord::new(true, false, true, false, "K")));
    let elem_chord: Element<'_, ()> = build_keybind_recorder_content(&with_chord, None);
    assert_eq!(elem_chord.as_widget().size().width, Length::Fill);
}

#[test]
fn test_build_content_listening() {
    let listening = KeybindRecorderModel::new().with_listening(true);
    let elem: Element<'_, ()> = build_keybind_recorder_content(&listening, None);
    assert_eq!(elem.as_widget().size().width, Length::Fill);
}

#[test]
fn test_build_content_with_conflict() {
    let conflict = KeybindRecorderModel::new()
        .with_chord(Some(KeyChord::new(true, false, false, false, "C")))
        .with_conflict(Some("Copy Selection".to_string()));
    let elem: Element<'_, ()> = build_keybind_recorder_content(&conflict, None);
    assert_eq!(elem.as_widget().size().width, Length::Fill);
}

#[test]
fn test_build_content_disabled() {
    let disabled = KeybindRecorderModel::new().with_disabled(true);
    let elem: Element<'_, ()> = build_keybind_recorder_content(&disabled, None);
    assert_eq!(elem.as_widget().size().width, Length::Fill);
}

#[test]
fn test_widget_creation_and_callbacks() {
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Msg {
        Preview(KeyChord),
        Commit(Option<KeyChord>),
        Cancel,
        Start,
    }

    let chord = KeyChord::new(true, false, false, false, "P");
    let model = KeybindRecorderModel::new().with_chord(Some(chord));

    let mut previewed = None;
    let mut committed = None;
    let mut cancelled = false;
    let mut started = false;

    let widget = keybind_recorder(model)
        .on_chord_preview(|live: Live<KeyChord>| Msg::Preview(live.into_value()))
        .on_chord_commit(|commit: Commit<Option<KeyChord>>| Msg::Commit(commit.into_value()))
        .on_cancel(|_cancel: Cancel<()>| Msg::Cancel)
        .on_start_listen(|| Msg::Start);

    if let Some(on_prev) = &widget.on_chord_preview {
        let msg = (on_prev)(Live::new(KeyChord::new(true, false, false, false, "P")));
        if let Msg::Preview(c) = msg {
            previewed = Some(c);
        }
    }
    if let Some(on_com) = &widget.on_chord_commit {
        let msg = (on_com)(Commit::new(None));
        if let Msg::Commit(c) = msg {
            committed = Some(c);
        }
    }
    if let Some(on_can) = &widget.on_cancel {
        let msg = (on_can)(Cancel::new(()));
        if let Msg::Cancel = msg {
            cancelled = true;
        }
    }
    if let Some(on_start) = &widget.on_start_listen {
        let msg = (on_start)();
        if let Msg::Start = msg {
            started = true;
        }
    }

    assert_eq!(previewed.unwrap().key, "P");
    assert_eq!(committed, Some(None));
    assert!(cancelled);
    assert!(started);
}

#[test]
fn test_from_keybind_recorder_into_element() {
    let model = KeybindRecorderModel::new();
    let widget = keybind_recorder(model);
    let _element: Element<'_, ()> = widget.into();
}

#[test]
fn test_interactive_keyboard_events() {
    use iced::{Event, Point};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum TestMsg {
        Preview(KeyChord),
        Commit(Option<KeyChord>),
        Cancel,
    }

    fn update_recorder<'a, Message>(
        widget: &mut KeybindRecorder<'a, Message>,
        tree: &mut Tree,
        event: &Event,
        cursor: mouse::Cursor,
        bounds: Rectangle,
        messages: &mut Vec<Message>,
    ) {
        let renderer = crate::test_helpers::headless_renderer();
        let limits = layout::Limits::new(Size::ZERO, bounds.size());
        let widget_dyn = widget as &mut dyn Widget<Message, iced::Theme, iced::Renderer>;
        let layout_node = widget_dyn.layout(tree, &renderer, &limits);
        let layout = Layout::new(&layout_node);
        let mut clipboard = iced::advanced::clipboard::Null;
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

    let model = KeybindRecorderModel::new().with_listening(true);
    let mut widget = keybind_recorder(model)
        .on_chord_preview(|live| TestMsg::Preview(live.into_value()))
        .on_chord_commit(|commit| TestMsg::Commit(commit.into_value()))
        .on_cancel(|_cancel| TestMsg::Cancel);

    let mut tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    let mut messages = Vec::new();
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 32.0));
    let cursor = mouse::Cursor::Available(Point::new(50.0, 16.0));

    // 1. Escape key cancels capture
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

    update_recorder(
        &mut widget,
        &mut tree,
        &esc_event,
        cursor,
        bounds,
        &mut messages,
    );
    assert_eq!(messages, vec![TestMsg::Cancel]);
    messages.clear();

    // 2. Backspace clears chord to None
    let backspace_event = Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: Key::Named(Named::Backspace),
        modified_key: Key::Named(Named::Backspace),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::default(),
        repeat: false,
        text: None,
    });

    update_recorder(
        &mut widget,
        &mut tree,
        &backspace_event,
        cursor,
        bounds,
        &mut messages,
    );
    assert_eq!(messages, vec![TestMsg::Commit(None)]);
    messages.clear();

    // 3. Delete clears chord to None
    let delete_event = Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: Key::Named(Named::Delete),
        modified_key: Key::Named(Named::Delete),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::default(),
        repeat: false,
        text: None,
    });

    update_recorder(
        &mut widget,
        &mut tree,
        &delete_event,
        cursor,
        bounds,
        &mut messages,
    );
    assert_eq!(messages, vec![TestMsg::Commit(None)]);
    messages.clear();

    // 4. Modifier alone (Control) previews modifier chord
    let ctrl_event = Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: Key::Named(Named::Control),
        modified_key: Key::Named(Named::Control),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::CTRL,
        repeat: false,
        text: None,
    });

    update_recorder(
        &mut widget,
        &mut tree,
        &ctrl_event,
        cursor,
        bounds,
        &mut messages,
    );
    assert_eq!(
        messages,
        vec![TestMsg::Preview(KeyChord {
            ctrl: true,
            alt: false,
            shift: false,
            super_key: false,
            key: String::new(),
        })]
    );
    messages.clear();

    // 5. Non-modifier key ('P') with Ctrl previews and commits chord
    let p_event = Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: Key::Character("p".into()),
        modified_key: Key::Character("p".into()),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::CTRL,
        repeat: false,
        text: Some("p".into()),
    });

    update_recorder(
        &mut widget,
        &mut tree,
        &p_event,
        cursor,
        bounds,
        &mut messages,
    );
    let expected_chord = KeyChord {
        ctrl: true,
        alt: false,
        shift: false,
        super_key: false,
        key: "P".to_string(),
    };
    assert_eq!(
        messages,
        vec![
            TestMsg::Preview(expected_chord.clone()),
            TestMsg::Commit(Some(expected_chord)),
        ]
    );
    messages.clear();

    // 6. When not listening, events are passed through without capturing
    let not_listening_model = KeybindRecorderModel::new().with_listening(false);
    let mut quiet_widget = keybind_recorder(not_listening_model).on_cancel(|_| TestMsg::Cancel);
    let mut quiet_tree =
        Tree::new(&quiet_widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
    update_recorder(
        &mut quiet_widget,
        &mut quiet_tree,
        &esc_event,
        cursor,
        bounds,
        &mut messages,
    );
    assert!(messages.is_empty());
}

// ---------------------------------------------------------------------------
// Helpers for the widget-level contract
// ---------------------------------------------------------------------------

/// Message the widget-level tests watch, one variant per callback channel.
#[derive(Debug, Clone, PartialEq)]
enum RecorderMsg {
    Preview(KeyChord),
    Commit(Option<KeyChord>),
    Cancel,
    Start,
}

/// Marker stashed in a child tree to see whether `diff` kept it or rebuilt it.
const SLOT_MARKER: u32 = 0xC0DE;

/// A recorder with every callback wired, so a test can tell which channel fired.
fn wired(model: KeybindRecorderModel) -> KeybindRecorder<'static, RecorderMsg> {
    keybind_recorder(model)
        .on_chord_preview(|live: Live<KeyChord>| RecorderMsg::Preview(live.into_value()))
        .on_chord_commit(|commit: Commit<Option<KeyChord>>| {
            RecorderMsg::Commit(commit.into_value())
        })
        .on_cancel(|_cancel: Cancel<()>| RecorderMsg::Cancel)
        .on_start_listen(|| RecorderMsg::Start)
}

/// A chord whose keycaps are too wide to be mistaken for the 24px clear button.
fn ctrl_p() -> KeyChord {
    KeyChord::new(true, false, false, false, "P")
}

/// A `KeyPressed` for `key` held with `modifiers`.
fn key_press(key: Key, modifiers: iced::keyboard::Modifiers) -> iced::Event {
    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers,
        repeat: false,
        text: None,
    })
}

/// Deliver one event through the widget's own `update`, reporting both the
/// messages it published and whether it captured the event.
///
/// `Harness::pointer` only carries mouse events, so the keyboard branches have
/// to be driven through `Widget::update` directly. The capture flag matters as
/// much as the messages: a recorder that publishes nothing but still lets the
/// host see the keystroke leaks shortcut keys into whatever else is focused.
fn drive<Message>(
    widget: &mut KeybindRecorder<'_, Message>,
    tree: &mut Tree,
    renderer: &iced::Renderer,
    event: &iced::Event,
    cursor: mouse::Cursor,
    bounds: Rectangle,
) -> (Vec<Message>, bool) {
    let limits = layout::Limits::new(Size::ZERO, bounds.size());
    let widget_dyn: &mut dyn Widget<Message, iced::Theme, iced::Renderer> = widget;
    let node = widget_dyn.layout(tree, renderer, &limits);
    let layout = Layout::new(&node);
    let mut clipboard = iced::advanced::clipboard::Null;
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);

    widget_dyn.update(
        tree,
        event,
        layout,
        cursor,
        renderer,
        &mut clipboard,
        &mut shell,
        &bounds,
    );

    let captured = shell.is_event_captured();
    (messages, captured)
}

/// The cursor the recorder asks for with `cursor` parked over an input laid out
/// in `model` state.
fn cursor_for(
    model: KeybindRecorderModel,
    cursor: mouse::Cursor,
    renderer: &iced::Renderer,
) -> mouse::Interaction {
    let twin: Element<'_, RecorderMsg> = wired(model.clone()).into();
    let mut tree = Tree::new(&twin);
    let mut widget = wired(model);
    let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 300.0));
    let node = widget.layout(&mut tree, renderer, &limits);
    let layout = Layout::new(&node);
    let viewport = Rectangle::with_size(node.size());

    widget.mouse_interaction(&tree, layout, cursor, &viewport, renderer)
}

/// Every box a laid-out widget publishes through the operation stream.
fn published_boxes<Message, Theme, Renderer: renderer::Renderer>(
    widget: &mut dyn Widget<Message, Theme, Renderer>,
    tree: &mut Tree,
    layout: Layout<'_>,
    renderer: &Renderer,
) -> Vec<Candidate> {
    let mut probe = RecordCandidates::default();
    {
        let probe: &mut dyn Operation<Vec<Candidate>> = &mut probe;
        let mut erased = black_box(probe);
        widget.operate(tree, layout, renderer, &mut erased);
    }
    probe.candidates()
}

/// The boxes of the trailing `[× Clear]` button, recovered from the tree rather
/// than hard-coded, so the hit test survives a padding change.
fn clear_boxes(published: &[Candidate]) -> Vec<Rectangle> {
    published
        .iter()
        .filter(|candidate| candidate.kind == KIND_CONTAINER)
        .map(|candidate| candidate.bounds)
        .filter(|bounds| bounds.width == CLEAR_BUTTON_SIZE && bounds.height == CLEAR_BUTTON_SIZE)
        .collect()
}

/// The recorder's own input band.
fn recorder_band(published: &[Candidate]) -> Rectangle {
    published
        .iter()
        .map(|candidate| candidate.bounds)
        .find(|bounds| bounds.height == HEIGHT && bounds.width > CLEAR_BUTTON_SIZE)
        .unwrap_or_else(|| {
            panic!(
                "the recorder must publish a {HEIGHT}px input band, found {:?}",
                published
                    .iter()
                    .map(|candidate| candidate.bounds)
                    .collect::<Vec<_>>()
            )
        })
}

/// Move, press and release at `at`, keeping every message the recorder answered.
fn click<Message: Send + 'static>(
    harness: &mut Harness<'_, Message, iced::Renderer>,
    at: Point,
) -> Vec<Message> {
    let mut emitted = harness.hover(at);
    emitted.extend(harness.press(at));
    emitted.extend(harness.release(at));
    emitted
}

// ---------------------------------------------------------------------------
// The widget's own contract with the host
// ---------------------------------------------------------------------------

#[test]
fn test_the_recorder_requests_the_spec_height_and_one_child_slot() {
    let widget = wired(KeybindRecorderModel::new().with_chord(Some(ctrl_p())));
    let requested = widget.size();

    assert_eq!(
        requested.width,
        Length::Fill,
        "the recorder spans the width it is given (UIF-PRIN-004)"
    );
    assert_eq!(
        requested.height,
        Length::Fixed(HEIGHT),
        "the input is exactly 32px tall and must not stretch (UIF-COMP-015)"
    );
    assert_eq!(
        widget.children().len(),
        1,
        "the composite wraps a single content element, so it publishes one child tree"
    );
}

#[test]
fn test_a_conflict_warning_stacks_a_row_below_the_input() {
    let widget = wired(
        KeybindRecorderModel::new()
            .with_chord(Some(ctrl_p()))
            .with_conflict(Some("Save File".to_string())),
    );

    assert_eq!(
        widget.size().height,
        Length::Shrink,
        "the warning is a second row, so the box shrinks to fit input and label together"
    );
}

#[test]
fn test_diff_rebuilds_a_missing_child_slot_and_keeps_an_unchanged_one() {
    let widget = wired(KeybindRecorderModel::new().with_chord(Some(ctrl_p())));

    let mut tree = Tree::empty();
    assert!(
        tree.children.is_empty(),
        "an empty tree carries no child slot to diff"
    );

    widget.diff(&mut tree);
    assert_eq!(
        tree.children.len(),
        1,
        "diffing against a tree with no children must create the content slot"
    );

    tree.children[0].state = TreeState::new(SLOT_MARKER);
    widget.diff(&mut tree);

    assert_eq!(
        tree.children.len(),
        1,
        "diffing an unchanged content element must neither add nor drop a slot"
    );
    assert_eq!(
        tree.children[0].state.downcast_ref::<u32>(),
        &SLOT_MARKER,
        "the content widget did not change, so its child state must survive the diff"
    );
}

#[test]
fn test_the_recorder_lays_out_at_the_spec_height_and_grows_for_a_conflict() {
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 300.0));

    let model = KeybindRecorderModel::new().with_chord(Some(ctrl_p()));
    let twin: Element<'_, RecorderMsg> = wired(model.clone()).into();
    let mut tree = Tree::new(&twin);
    let mut widget = wired(model);
    let node = widget.layout(&mut tree, &renderer, &limits);

    assert_eq!(
        node.size().height,
        HEIGHT,
        "a quiet recorder is exactly 32px tall (UIF-COMP-015)"
    );
    assert_eq!(
        node.size().width,
        400.0,
        "the recorder fills the width the host gives it"
    );

    let conflict_model = KeybindRecorderModel::new().with_conflict(Some("Save File".to_string()));
    let conflict_twin: Element<'_, RecorderMsg> = wired(conflict_model.clone()).into();
    let mut conflict_tree = Tree::new(&conflict_twin);
    let mut conflict_widget = wired(conflict_model);
    let conflict_node = conflict_widget.layout(&mut conflict_tree, &renderer, &limits);

    assert!(
        conflict_node.size().height > HEIGHT,
        "a conflict adds a readable line below the input, so the box must grow past {HEIGHT}px, got {}",
        conflict_node.size().height
    );
}

#[test]
fn test_the_laid_out_recorder_publishes_its_clear_button_and_no_overlay() {
    let renderer = headless_renderer();
    let model = KeybindRecorderModel::new().with_chord(Some(ctrl_p()));
    let twin: Element<'_, RecorderMsg> = wired(model.clone()).into();
    let mut tree = Tree::new(&twin);
    let mut widget = wired(model);
    let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 300.0));
    let node = widget.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);
    let viewport = Rectangle::with_size(node.size());

    let widget_dyn: &mut dyn Widget<RecorderMsg, iced::Theme, iced::Renderer> = &mut widget;
    let published = published_boxes(widget_dyn, &mut tree, layout, &renderer);

    let clear = clear_boxes(&published);
    assert_eq!(
        clear.len(),
        1,
        "the recorder must publish exactly one {CLEAR_BUTTON_SIZE}px clear button, found {clear:?}"
    );

    let band = recorder_band(&published);
    let clear_right = clear[0].x + clear[0].width;
    let band_right = band.x + band.width;
    assert!(
        (clear_right - (band_right - space::XS)).abs() <= 1.0,
        "the clear button sits flush inside the trailing padding: clear right {clear_right}, band right {band_right}"
    );

    assert!(
        widget
            .overlay(&mut tree, layout, &renderer, &viewport, Vector::ZERO)
            .is_none(),
        "the recorder has no popover to hand the host"
    );
}

#[test]
fn test_the_recorder_asks_for_a_pointer_only_when_it_can_start_listening() {
    let renderer = headless_renderer();
    let over = mouse::Cursor::Available(Point::new(200.0, 16.0));
    let outside = mouse::Cursor::Available(Point::new(200.0, 200.0));

    assert_eq!(
        cursor_for(KeybindRecorderModel::new(), over, &renderer),
        mouse::Interaction::Pointer,
        "a resting recorder invites the click that starts capture"
    );
    assert_eq!(
        cursor_for(
            KeybindRecorderModel::new().with_listening(true),
            over,
            &renderer
        ),
        mouse::Interaction::None,
        "a recorder already listening must not offer the start gesture again"
    );
    assert_eq!(
        cursor_for(
            KeybindRecorderModel::new().with_disabled(true),
            over,
            &renderer
        ),
        mouse::Interaction::None,
        "a disabled recorder must not invite a click it will refuse"
    );
    assert_eq!(
        cursor_for(KeybindRecorderModel::new(), outside, &renderer),
        mouse::Interaction::None,
        "the pointer is offered over the recorder's own box and nowhere else"
    );
}

// ---------------------------------------------------------------------------
// Keyboard capture guards
// ---------------------------------------------------------------------------

#[test]
fn test_a_disabled_recorder_ignores_keys_even_while_listening() {
    let renderer = headless_renderer();
    let model = KeybindRecorderModel::new()
        .with_listening(true)
        .with_disabled(true);
    let twin: Element<'_, RecorderMsg> = wired(model.clone()).into();
    let mut tree = Tree::new(&twin);
    let mut widget = wired(model);
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(400.0, HEIGHT));
    let cursor = mouse::Cursor::Available(Point::new(200.0, 16.0));

    let (messages, captured) = drive(
        &mut widget,
        &mut tree,
        &renderer,
        &key_press(Key::Character("p".into()), iced::keyboard::Modifiers::CTRL),
        cursor,
        bounds,
    );

    assert!(
        messages.is_empty(),
        "a disabled recorder must not commit a chord even while its listening flag is set, got {messages:?}"
    );
    assert!(
        !captured,
        "a disabled recorder must leave the keystroke to the rest of the interface"
    );
}

#[test]
fn test_a_listening_recorder_swallows_every_key_it_is_given() {
    let renderer = headless_renderer();
    let model = KeybindRecorderModel::new().with_listening(true);
    let twin: Element<'_, RecorderMsg> = keybind_recorder(model.clone()).into();
    let mut tree = Tree::new(&twin);
    let mut widget: KeybindRecorder<'_, RecorderMsg> = keybind_recorder(model);
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(400.0, HEIGHT));
    let cursor = mouse::Cursor::Available(Point::new(200.0, 16.0));

    let events = [
        key_press(
            Key::Named(Named::Escape),
            iced::keyboard::Modifiers::default(),
        ),
        key_press(
            Key::Named(Named::Backspace),
            iced::keyboard::Modifiers::default(),
        ),
        key_press(Key::Character("k".into()), iced::keyboard::Modifiers::SHIFT),
    ];

    for event in &events {
        let (messages, captured) = drive(&mut widget, &mut tree, &renderer, event, cursor, bounds);

        assert!(
            messages.is_empty(),
            "with no callbacks wired the recorder has nothing to publish, got {messages:?}"
        );
        assert!(
            captured,
            "a listening recorder owns every keystroke, {event:?}, so the host must not see it"
        );
    }
}

#[test]
fn test_a_named_function_key_commits_its_canonical_name() {
    let renderer = headless_renderer();
    let model = KeybindRecorderModel::new().with_listening(true);
    let twin: Element<'_, RecorderMsg> = wired(model.clone()).into();
    let mut tree = Tree::new(&twin);
    let mut widget = wired(model);
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(400.0, HEIGHT));
    let cursor = mouse::Cursor::Available(Point::new(200.0, 16.0));

    let (messages, captured) = drive(
        &mut widget,
        &mut tree,
        &renderer,
        &key_press(Key::Named(Named::Enter), iced::keyboard::Modifiers::SHIFT),
        cursor,
        bounds,
    );

    let expected = KeyChord::new(false, false, true, false, "Enter");
    assert_eq!(
        messages,
        vec![
            RecorderMsg::Preview(expected.clone()),
            RecorderMsg::Commit(Some(expected)),
        ],
        "a named key is previewed and then committed under its canonical name so the host can store it"
    );
    assert!(
        captured,
        "the keystroke that completed the chord belongs to the recorder"
    );
}

// ---------------------------------------------------------------------------
// Pointer: starting to listen, and the clear action
// ---------------------------------------------------------------------------

#[test]
fn test_a_press_starts_listening_only_for_a_resting_enabled_recorder() {
    let mut harness = Harness::headless(|| wired(KeybindRecorderModel::new()).into());
    let at = centre(recorder_band(&harness.candidates()));

    assert_eq!(
        harness.press(at),
        vec![RecorderMsg::Start],
        "a click on the input is the gesture that starts capture (UIF-COMP-015)"
    );

    harness.set_view(|| wired(KeybindRecorderModel::new().with_listening(true)).into());
    assert_eq!(
        harness.press(at),
        Vec::new(),
        "a recorder already listening must not ask the host to start again"
    );

    harness.set_view(|| wired(KeybindRecorderModel::new().with_disabled(true)).into());
    assert_eq!(
        harness.press(at),
        Vec::new(),
        "a disabled recorder ignores the click entirely"
    );

    harness.set_view(|| wired(KeybindRecorderModel::new()).into());
    assert_eq!(
        harness.press(Point::new(200.0, 200.0)),
        Vec::new(),
        "a click outside the input does not start capture"
    );
}

#[test]
fn test_the_clear_button_appears_only_for_a_live_recorded_chord() {
    let mut harness =
        Harness::headless(|| wired(KeybindRecorderModel::new().with_chord(Some(ctrl_p()))).into());

    assert_eq!(
        clear_boxes(&harness.candidates()).len(),
        1,
        "a recorded chord gets the [× Clear] action (UIF-COMP-015)"
    );

    harness.set_view(|| wired(KeybindRecorderModel::new()).into());
    assert!(
        clear_boxes(&harness.candidates()).is_empty(),
        "there is nothing to clear when no chord is recorded"
    );

    harness.set_view(|| {
        wired(
            KeybindRecorderModel::new()
                .with_chord(Some(ctrl_p()))
                .with_listening(true),
        )
        .into()
    });
    assert!(
        clear_boxes(&harness.candidates()).is_empty(),
        "while listening every keystroke is a chord, so the clear action must not compete for the click"
    );

    harness.set_view(|| {
        wired(
            KeybindRecorderModel::new()
                .with_chord(Some(ctrl_p()))
                .with_disabled(true),
        )
        .into()
    });
    assert!(
        clear_boxes(&harness.candidates()).is_empty(),
        "a disabled recorder offers no actions at all"
    );
}

#[test]
fn test_the_clear_button_clears_the_chord_when_no_start_callback_is_set() {
    let model = KeybindRecorderModel::new().with_chord(Some(ctrl_p()));
    let mut harness = Harness::headless(move || {
        keybind_recorder(model.clone())
            .on_chord_commit(|commit: Commit<Option<KeyChord>>| {
                RecorderMsg::Commit(commit.into_value())
            })
            .into()
    });

    let clear = clear_boxes(&harness.candidates());
    assert_eq!(
        clear.len(),
        1,
        "the clear button must be published before it can be clicked"
    );

    assert_eq!(
        click(&mut harness, centre(clear[0])),
        vec![RecorderMsg::Commit(None)],
        "the clear button commits an empty chord so the host can unbind the shortcut"
    );
}

#[test]
fn test_the_start_guard_swallows_a_press_on_the_clear_button() {
    let mut harness =
        Harness::headless(|| wired(KeybindRecorderModel::new().with_chord(Some(ctrl_p()))).into());

    let clear = clear_boxes(&harness.candidates());
    assert_eq!(
        clear.len(),
        1,
        "the clear button is painted, so the user can aim at it"
    );

    // NOTE(defect): the start-listening guard matches any press inside the
    // recorder's own bounds and returns before the content sees the event, so
    // the button never records the press and its release commits nothing.
    // Reported, not fixed: while `on_start_listen` is set, `[× Clear]` is
    // unreachable and the chord can only be cleared from the keyboard.
    assert_eq!(
        click(&mut harness, centre(clear[0])),
        vec![RecorderMsg::Start],
        "a press on the clear button is intercepted by the start guard, so only Start is published"
    );
}

// ---------------------------------------------------------------------------
// The paint path
// ---------------------------------------------------------------------------

#[test]
fn test_listening_and_conflict_states_repaint_the_input() {
    let mut capture = Capture::new();
    let viewport = Size::new(400.0, 90.0);
    let band = Rectangle::new(Point::ORIGIN, Size::new(400.0, HEIGHT));
    let below = Rectangle::new(Point::new(0.0, HEIGHT + 1.0), Size::new(400.0, 50.0));

    let rest = capture.frame(viewport, wired(KeybindRecorderModel::new()).into());
    assert!(
        rest.has_ink(band),
        "the resting recorder paints its `Record shortcut` prompt"
    );
    assert!(
        !rest.has_ink(below),
        "a quiet recorder paints nothing below its 32px input"
    );

    let listening = capture.frame(
        viewport,
        wired(KeybindRecorderModel::new().with_listening(true)).into(),
    );
    assert_repainted(
        &rest,
        &listening,
        band,
        "listening swaps the prompt for the recording label and the focus border",
    );

    let chorded = capture.frame(
        viewport,
        wired(KeybindRecorderModel::new().with_chord(Some(ctrl_p()))).into(),
    );
    assert_repainted(
        &rest,
        &chorded,
        band,
        "a recorded chord replaces the prompt with keycap pills",
    );

    let conflict = capture.frame(
        viewport,
        wired(
            KeybindRecorderModel::new()
                .with_chord(Some(ctrl_p()))
                .with_conflict(Some("Save File".to_string())),
        )
        .into(),
    );
    assert!(
        conflict.has_ink(below),
        "the conflict warning paints below the input where the user can read it"
    );
}
