use super::*;
use crate::binding::{Cancel, Commit, Live};
use crate::token::{shape, size};

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
