//! Sidecar unit tests for the top-level iced `Message` vocabulary
//! ([TASK-021], `doc/planning/iced.md`).
//!
//! Pure data tests only ([RUST-006]): every path is a synthetic relative
//! fixture literal that is never opened, and no user profile, live
//! configuration file or database is touched.

use super::{
    AgentMessage, DialogMessage, EditorMessage, LayoutMessage, LogsMessage, Message, RenderMessage,
    SearchMessage, TabMessage, ThemeMessage, TreeMessage, WindowMessage, WindowSnapshot,
};
use crate::bus::events::messages::LogCategory;
use crate::bus::events::user_command::UserCommand;
use crate::ui_iced::layout::middle_band_state;
use iced::widget::text_editor::{Action, Edit};
use std::path::PathBuf;

/// Synthetic path used by the payload fixtures — never opened ([RUST-006]).
fn sample_path() -> PathBuf {
    PathBuf::from("fixture-workspace").join("notes.md")
}

/// Compile-time assertion that a type is `Clone`.
fn assert_clone<T: Clone>() {}

/// Compile-time assertion that a type is `Copy`.
fn assert_copy<T: Copy>() {}

/// One representative message per logical channel, in declaration order.
fn one_message_per_channel() -> Vec<Message> {
    let id = iced::window::Id::unique();
    let (_panes, band) = middle_band_state();
    vec![
        Message::Window(WindowMessage::CloseRequested(id)),
        Message::Layout(LayoutMessage::PaneResized(
            iced::widget::pane_grid::ResizeEvent {
                split: band.left_split,
                ratio: 0.4,
            },
        )),
        Message::Command(UserCommand::RunAgent("summarise my notes".into())),
        Message::Tree(TreeMessage::ContextMenuOpened {
            path: sample_path(),
        }),
        Message::Tab(TabMessage::ContextMenuOpened { index: 2 }),
        Message::Editor(EditorMessage::Action(Action::Edit(Edit::Insert('x')))),
        Message::Agent(AgentMessage::InputChanged {
            text: "/models".into(),
        }),
        Message::Search(SearchMessage::Submitted {
            query: "kickoff".into(),
        }),
        Message::Logs(LogsMessage::CategoryFilterChanged(Some(
            LogCategory::Indexer,
        ))),
        Message::Theme(ThemeMessage::ScaleChanged { multiplier: 1.25 }),
        Message::Render(RenderMessage::LinkActivated {
            url: "https://example.com".into(),
            source_file: Some(sample_path()),
        }),
        Message::Dialog(DialogMessage::NameChanged {
            text: "my note".into(),
        }),
    ]
}

#[test]
fn test_message_exposes_one_variant_per_logical_channel() {
    let messages = one_message_per_channel();
    assert_eq!(
        messages.len(),
        12,
        "TASK-021 fixes the channel set at window, layout, command, tree, tab, \
         editor, agent, search, logs, theme; TASK-015 adds render; TASK-017b1 \
         adds dialog"
    );

    assert!(matches!(messages[0], Message::Window(_)));
    assert!(matches!(messages[1], Message::Layout(_)));
    assert!(matches!(messages[2], Message::Command(_)));
    assert!(matches!(messages[3], Message::Tree(_)));
    assert!(matches!(messages[4], Message::Tab(_)));
    assert!(matches!(messages[5], Message::Editor(_)));
    assert!(matches!(messages[6], Message::Agent(_)));
    assert!(matches!(messages[7], Message::Search(_)));
    assert!(matches!(messages[8], Message::Logs(_)));
    assert!(matches!(messages[9], Message::Theme(_)));
    assert!(matches!(messages[10], Message::Render(_)));
    assert!(matches!(messages[11], Message::Dialog(_)));
}

#[test]
fn test_every_channel_message_clones_to_an_equal_value() {
    for message in one_message_per_channel() {
        let clone = message.clone();
        assert_eq!(
            message, clone,
            "clone must preserve the channel and its payload exactly"
        );
    }
}

#[test]
fn test_message_is_clone_but_channels_that_can_be_are_copy() {
    assert_clone::<Message>();
    assert_clone::<WindowMessage>();
    assert_clone::<LayoutMessage>();
    assert_clone::<TreeMessage>();
    assert_clone::<TabMessage>();
    assert_clone::<EditorMessage>();
    assert_clone::<AgentMessage>();
    assert_clone::<SearchMessage>();
    assert_clone::<LogsMessage>();
    assert_clone::<ThemeMessage>();

    // Every payload field of these four channels is `Copy`, so the channel
    // itself must be — a missing `Copy` derive fails this at compile time.
    assert_copy::<WindowMessage>();
    assert_copy::<LayoutMessage>();
    assert_copy::<TabMessage>();
    assert_copy::<ThemeMessage>();
    assert_copy::<WindowSnapshot>();
}

#[test]
fn test_copy_channel_value_survives_a_by_value_move() {
    // Runtime half of the `Copy` proof: `message` stays usable after `copy`
    // is bound by value, which only compiles for `Copy` types.
    let message = ThemeMessage::ScaleChanged { multiplier: 1.5 };
    let copy = message;
    assert_eq!(message, copy);

    let tab = TabMessage::ContextMenuClosed;
    let tab_copy = tab;
    assert_eq!(tab, tab_copy);
}

#[test]
fn test_from_user_command_wraps_into_the_command_channel() {
    let command = UserCommand::MergePrompt(vec![sample_path()]);
    let message = Message::from(command.clone());

    assert_eq!(message, Message::Command(command.clone()));
    match message {
        Message::Command(carried) => assert_eq!(carried, command),
        other => panic!("From<UserCommand> must produce the Command channel, got {other:?}"),
    }
}

#[test]
fn test_window_snapshot_carries_every_close_metric() {
    let id = iced::window::Id::unique();
    let snapshot = WindowSnapshot {
        id,
        size: iced::Size::new(1280.0, 800.0),
        position: Some(iced::Point::new(40.0, 24.0)),
    };

    assert_eq!(snapshot.id, id);
    assert_eq!(snapshot.size, iced::Size::new(1280.0, 800.0));
    assert_eq!(snapshot.position, Some(iced::Point::new(40.0, 24.0)));
    assert_eq!(snapshot, snapshot.clone());

    // A position-less window is representable (the runtime reports `None`).
    let unplaced = WindowSnapshot {
        position: None,
        ..snapshot
    };
    assert_eq!(unplaced.position, None);
    assert_ne!(unplaced, snapshot);
}

#[test]
fn test_pilot_messages_fold_into_the_window_and_layout_channels() {
    // The mapping documented in the module header: each pilot `Message`
    // variant from `src/app/bin/fastmd_iced.rs:239-267` has a home here.
    let id = iced::window::Id::unique();
    let (_panes, band) = middle_band_state();

    let folded = [
        Message::Window(WindowMessage::CloseRequested(id)),
        Message::Window(WindowMessage::CloseSnapshotCaptured(WindowSnapshot {
            id,
            size: iced::Size::new(1000.0, 700.0),
            position: Some(iced::Point::new(120.0, 90.0)),
        })),
        Message::Window(WindowMessage::ScaleBaseline(1.5)),
        Message::Window(WindowMessage::Resized {
            id,
            size: iced::Size::new(1024.0, 768.0),
        }),
        Message::Layout(LayoutMessage::PaneResized(
            iced::widget::pane_grid::ResizeEvent {
                split: band.right_split,
                ratio: 0.75,
            },
        )),
    ];

    // Distinctness: no two folded messages collapse onto each other, so the
    // pilot's reduction branches stay separable after the fold.
    for (i, a) in folded.iter().enumerate() {
        for (j, b) in folded.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b, "pilot variants {i} and {j} must stay distinguishable");
            }
        }
    }
}

#[test]
fn test_editor_channel_distinguishes_edit_actions_from_button_intents() {
    let edit = Message::Editor(EditorMessage::Action(Action::Edit(Edit::Insert('a'))));
    let motion = Message::Editor(EditorMessage::Action(Action::SelectAll));
    let save = Message::Editor(EditorMessage::SaveRequested {
        path: sample_path(),
    });
    let cancel = Message::Editor(EditorMessage::CancelRequested);
    let undo = Message::Editor(EditorMessage::Undo {
        path: sample_path(),
    });
    let redo = Message::Editor(EditorMessage::Redo {
        path: sample_path(),
    });

    assert_ne!(
        edit, motion,
        "edit vs caret motion must be separable (DEC-007)"
    );
    assert_ne!(save, cancel);
    assert_ne!(undo, redo);
    assert_eq!(undo.clone(), undo);
    assert_eq!(
        save,
        Message::Editor(EditorMessage::SaveRequested {
            path: sample_path(),
        })
    );
}

#[test]
fn test_search_channel_keeps_typed_text_separate_from_committed_query() {
    let typed = Message::Search(SearchMessage::QueryChanged {
        text: "kick".into(),
    });
    let committed = Message::Search(SearchMessage::Submitted {
        query: "kick".into(),
    });
    let cleared = Message::Search(SearchMessage::Cleared);

    // UI-050: typing must not be confused with committing, and clearing is
    // its own intent rather than an empty query.
    assert_ne!(typed, committed);
    assert_ne!(cleared, committed);
    assert_ne!(cleared, typed);
}

#[test]
fn test_logs_channel_covers_every_egui_filter_control() {
    let search = Message::Logs(LogsMessage::SearchChanged {
        text: "warn".into(),
    });
    let category = Message::Logs(LogsMessage::CategoryFilterChanged(Some(
        LogCategory::PdfConverter,
    )));
    let all_categories = Message::Logs(LogsMessage::CategoryFilterChanged(None));
    let autoscroll = Message::Logs(LogsMessage::AutoScrollToggled(true));
    let cleared = Message::Logs(LogsMessage::Cleared);

    assert_ne!(category, all_categories, "`None` means every category");
    assert_ne!(
        autoscroll,
        Message::Logs(LogsMessage::AutoScrollToggled(false))
    );
    assert_ne!(search, cleared);
    assert_eq!(category.clone(), category);
}

#[test]
fn test_tree_and_tab_context_menu_channels_are_independent() {
    let tree_open = Message::Tree(TreeMessage::ContextMenuOpened {
        path: sample_path(),
    });
    let tree_closed = Message::Tree(TreeMessage::ContextMenuClosed);
    let tab_open = Message::Tab(TabMessage::ContextMenuOpened { index: 0 });
    let tab_closed = Message::Tab(TabMessage::ContextMenuClosed);

    assert_ne!(tree_open, tree_closed);
    assert_ne!(tab_open, tab_closed);
    assert_ne!(tree_closed, tab_closed, "channels must not collapse");
    assert_eq!(tree_open.clone(), tree_open);
}

#[test]
fn test_debug_window_controls_stay_on_the_command_channel() {
    // Grounding guard: the agent debug window's controls are already
    // `UserCommand`s, so the Agent panel channel must not duplicate them.
    let message = Message::from(UserCommand::SetDebugAutoScroll(true));
    assert!(matches!(message, Message::Command(_)));
    assert!(matches!(
        Message::from(UserCommand::ClearDebugEntries),
        Message::Command(_)
    ));
    assert!(matches!(
        Message::from(UserCommand::ToggleBackgroundLogs(true)),
        Message::Command(_)
    ));
}

#[test]
fn test_cursor_moved_is_a_copy_layout_report_that_stays_distinct() {
    // [TASK-012]: the context-menu anchor. `MouseArea::on_right_press`
    // cannot carry a position, so the reducer has to remember this one.
    let moved = LayoutMessage::CursorMoved {
        point: iced::Point::new(12.0, 34.0),
    };
    let copy = moved;
    assert_eq!(moved, copy, "CursorMoved must keep the channel `Copy`");

    assert_eq!(
        Message::Layout(moved),
        Message::Layout(LayoutMessage::CursorMoved {
            point: iced::Point::new(12.0, 34.0),
        })
    );
    assert_ne!(
        Message::Layout(moved),
        Message::Layout(LayoutMessage::CursorMoved {
            point: iced::Point::new(12.0, 35.0),
        }),
        "a different point must not compare equal"
    );
    assert_ne!(
        Message::Layout(moved),
        Message::Layout(LayoutMessage::TreeAutoFit),
        "cursor motion must not collapse into the auto-fit request"
    );
}

#[test]
fn test_tree_deferred_intent_variants_are_all_distinct() {
    // [TASK-012]: the three egui menu rows that read files or fan out
    // inside the click have no single `UserCommand`, so each needs its own
    // separable message.
    let deferred = vec![
        Message::Tree(TreeMessage::RunAsPromptRequested {
            path: sample_path(),
        }),
        Message::Tree(TreeMessage::PrintRequested {
            path: sample_path(),
        }),
        Message::Tree(TreeMessage::DeleteSelectedRequested {
            paths: vec![sample_path()],
        }),
        Message::Tree(TreeMessage::SkillRunRequested {
            skill: PathBuf::from("fixture-workspace/Skills/Note/summarize.md"),
            target_dir: None,
            target_file: Some(sample_path()),
        }),
        Message::Tree(TreeMessage::SkillRunRequested {
            skill: PathBuf::from("fixture-workspace/Skills/Folder/tidy.md"),
            target_dir: Some(sample_path()),
            target_file: Some(sample_path()),
        }),
    ];

    for (i, a) in deferred.iter().enumerate() {
        for (j, b) in deferred.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b, "deferred intents {i} and {j} must stay separable");
            }
        }
    }
}

#[test]
fn test_deferred_tree_intent_stays_off_the_command_channel() {
    // Guard: these are NOT `UserCommand`s — the view may not read files,
    // so the reducer must be able to tell "deferred read" from "publish
    // this now".
    for message in [
        Message::Tree(TreeMessage::RunAsPromptRequested {
            path: sample_path(),
        }),
        Message::Tree(TreeMessage::PrintRequested {
            path: sample_path(),
        }),
        Message::Tree(TreeMessage::DeleteSelectedRequested {
            paths: vec![sample_path()],
        }),
    ] {
        assert!(
            !matches!(message, Message::Command(_)),
            "deferred intent must not masquerade as a ready-to-publish command"
        );
    }
}

#[test]
fn test_debug_messages_are_debuggable() {
    // `Debug` is required for tracing every cross-subsystem interaction
    // ([FMD-071]); spot-check that the rendering names the channels.
    let rendered = format!("{:?}", one_message_per_channel());
    for channel in [
        "Window", "Layout", "Command", "Tree", "Tab", "Editor", "Agent", "Search", "Logs", "Theme",
        "Render", "Dialog",
    ] {
        assert!(
            rendered.contains(channel),
            "Debug output must name the {channel} channel for [FMD-071] tracing"
        );
    }
}

#[test]
fn test_dialog_channel_covers_every_small_dialog_interaction() {
    // [TASK-017b1]: the name-entry / move / delete dialogs' view-local
    // transitions. Each is a distinct message so the reducer can route it.
    let name_changed = Message::Dialog(DialogMessage::NameChanged {
        text: "draft".into(),
    });
    let submitted = Message::Dialog(DialogMessage::Submitted);
    let cancelled = Message::Dialog(DialogMessage::Cancelled);
    let dismissed = Message::Dialog(DialogMessage::Dismissed);
    let destination = Message::Dialog(DialogMessage::DestinationSelected {
        path: sample_path(),
    });

    let all = [
        name_changed.clone(),
        submitted.clone(),
        cancelled.clone(),
        dismissed.clone(),
        destination.clone(),
    ];
    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b, "dialog interactions {i} and {j} must stay separable");
            }
        }
    }

    // Cancel and Dismiss are distinct: an explicit cancel differs from a
    // click-away, the same distinction OverlaySlot::close reports.
    assert_ne!(cancelled, dismissed);

    // None of these is a ready-to-publish command: validation runs first.
    for message in all {
        assert!(
            !matches!(message, Message::Command(_)),
            "a dialog interaction must not masquerade as a published command"
        );
    }
}
