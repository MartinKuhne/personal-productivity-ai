//! Sidecar unit tests for the top-level iced `Message` vocabulary
//! ([TASK-021], `doc/planning/iced.md`).
//!
//! Pure data tests only ([RUST-006]): every path is a synthetic relative
//! fixture literal that is never opened, and no user profile, live
//! configuration file or database is touched.

use super::{
    AboutMessage, AgentDebugMessage, AgentMessage, BatchDialogMessage, BootMessage, BusSource,
    DialogMessage, EditorMessage, InboundMessage, LayoutMessage, LogsMessage, Message,
    RenderMessage, SearchMessage, TabMessage, ThemeMessage, ToolsDialogMessage, TreeMessage,
    WindowMessage, WindowSnapshot,
};
use crate::agent::batch::types::BatchMode;
use crate::bus::events::messages::LogCategory;
use crate::bus::events::user_command::UserCommand;
use crate::ui::tree_search::SearchResultEntry;
use crate::ui_iced::layout::middle_band_state;
use iced::widget::text_editor::{Action, Edit};
use std::path::PathBuf;
use std::sync::Arc;

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
        Message::About(AboutMessage::AttributionsRequested),
        Message::AgentDebug(AgentDebugMessage::EntryToggled { id: 3 }),
        Message::Inbound(InboundMessage::EventsDropped {
            source: BusSource::Agent,
            count: 3,
        }),
    ]
}

#[test]
fn test_message_exposes_one_variant_per_logical_channel() {
    let messages = one_message_per_channel();
    assert_eq!(
        messages.len(),
        15,
        "TASK-021 fixes the channel set at window, layout, command, tree, tab, \
         editor, agent, search, logs, theme; TASK-015 adds render; TASK-017b1 \
         adds dialog; TASK-017b2 adds about; TASK-017b4 adds agent_debug; \
         TASK-019 adds inbound (background→UI notifications) \
         (batch and tools, added by TASK-017b3, are not represented here)"
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
    assert!(matches!(messages[12], Message::About(_)));
    assert!(matches!(messages[13], Message::AgentDebug(_)));
    assert!(matches!(messages[14], Message::Inbound(_)));
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
fn test_agent_channel_thinking_and_scroll_variants() {
    // TASK-018 added the thinking-disclosure and transcript-scroll-report
    // variants to the Agent channel.
    let toggled = Message::Agent(AgentMessage::ThinkingToggled { expanded: true });
    assert!(matches!(
        toggled,
        Message::Agent(AgentMessage::ThinkingToggled { expanded: true })
    ));
    assert_eq!(toggled, toggled.clone());

    let scrolled = Message::Agent(AgentMessage::ScrollReported {
        offset_y: 10.0,
        viewport_height: 20.0,
        content_height: 30.0,
    });
    match scrolled {
        Message::Agent(AgentMessage::ScrollReported {
            offset_y,
            viewport_height,
            content_height,
        }) => {
            assert_eq!(offset_y, 10.0);
            assert_eq!(viewport_height, 20.0);
            assert_eq!(content_height, 30.0);
        }
        other => panic!("expected ScrollReported, got {other:?}"),
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
fn test_logs_channel_covers_every_thelegacy_filter_control() {
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
    // [TASK-012]: the three the legacy UI menu rows that read files or fan out
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
        "Render", "Dialog", "About",
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

#[test]
fn test_about_channel_covers_every_identity_window_interaction() {
    // [TASK-017b2]: the About + attributions windows' view-local
    // transitions and deferred intents. Each stays separable so the
    // reducer can route it.
    let attributions = Message::About(AboutMessage::AttributionsRequested);
    let copy_hash = Message::About(AboutMessage::CopyCommitHashRequested);
    let repo = Message::About(AboutMessage::RepositoryOpened {
        url: "https://github.com/dtolnay/anyhow".into(),
    });
    let close = Message::About(AboutMessage::CloseRequested);

    let all = [
        attributions.clone(),
        copy_hash.clone(),
        repo.clone(),
        close.clone(),
    ];
    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b, "about interactions {i} and {j} must stay separable");
            }
        }
    }

    // The About channel is not the command channel: opening the dialog is
    // `UserCommand::OpenAboutDialog`, everything else here is view-local.
    for message in all {
        assert!(
            !matches!(message, Message::Command(_)),
            "an about-window interaction must not masquerade as a published command"
        );
    }
}

#[test]
fn test_about_channel_is_not_the_small_dialog_channel() {
    // [TASK-017b2] channel choice: About/attributions deliberately do NOT
    // ride `Message::Dialog`. A close on one channel must never be
    // mistakable for a dismiss on the other.
    assert_ne!(
        Message::About(AboutMessage::CloseRequested),
        Message::Dialog(DialogMessage::Dismissed)
    );
    assert_ne!(
        Message::About(AboutMessage::CloseRequested),
        Message::Dialog(DialogMessage::Cancelled)
    );
}

#[test]
fn test_batch_channel_covers_every_selection_transition() {
    // [TASK-017b3]: the batch dialog's composed selection. Each variant
    // stays separable so the reducer can route it to one field.
    let all = [
        Message::Batch(BatchDialogMessage::DirectorySelected(Some(1))),
        Message::Batch(BatchDialogMessage::DirectorySelected(None)),
        Message::Batch(BatchDialogMessage::PromptSelected(Some(2))),
        Message::Batch(BatchDialogMessage::PromptSelected(None)),
        Message::Batch(BatchDialogMessage::PatternChanged {
            text: "*.md".into(),
        }),
        Message::Batch(BatchDialogMessage::ModeChanged(BatchMode::File)),
        Message::Batch(BatchDialogMessage::ModeChanged(BatchMode::Directory)),
        Message::Batch(BatchDialogMessage::ConcurrencyChanged(4)),
    ];

    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b, "batch selection {i} and {j} must stay separable");
            }
        }
    }

    for message in all {
        assert!(
            !matches!(message, Message::Command(_)),
            "a batch selection transition must not masquerade as a published command"
        );
    }
}

#[test]
fn test_batch_open_start_and_cancel_stay_on_the_command_channel() {
    // [TASK-017b3] channel choice: only the *selection* needed a new
    // channel. Open / Start / Cancel already ride `Message::Command` as
    // exactly what the legacy UI dialog published, so no `BatchDialogMessage`
    // variant duplicates them.
    let commands = [
        Message::Command(UserCommand::OpenBatchDialog),
        Message::Command(UserCommand::CancelBatch),
    ];
    for command in commands {
        assert!(matches!(command, Message::Command(_)));
    }

    // No Batch-channel value equals a command-channel value.
    assert_ne!(
        Message::Batch(BatchDialogMessage::ConcurrencyChanged(4)),
        Message::Command(UserCommand::CancelBatch)
    );
}

#[test]
fn test_tools_channel_carries_only_the_dismiss() {
    // [TASK-017b3]: every tool-group action is already a `UserCommand`,
    // so the Tools channel holds exactly one variant.
    assert_eq!(
        Message::Tools(ToolsDialogMessage::CloseRequested),
        Message::Tools(ToolsDialogMessage::CloseRequested)
    );
    assert_ne!(
        Message::Tools(ToolsDialogMessage::CloseRequested),
        Message::Command(UserCommand::OpenToolsDialog)
    );
    assert_ne!(
        Message::Tools(ToolsDialogMessage::CloseRequested),
        Message::Dialog(DialogMessage::Dismissed)
    );
}

#[test]
fn test_tool_group_actions_ride_the_command_channel_not_the_tools_channel() {
    let actions = [
        Message::Command(UserCommand::SetToolGroupEnabled {
            id: crate::agent::tools::registry::ToolGroupId::Mcp("srv".into()),
            enabled: true,
        }),
        Message::Command(UserCommand::ClearToolGroupError(
            crate::agent::tools::registry::ToolGroupId::Mcp("srv".into()),
        )),
        Message::Command(UserCommand::StartMcpAuth("srv".into())),
        Message::Command(UserCommand::ForgetMcpAuth("srv".into())),
    ];
    for action in actions {
        assert!(
            matches!(action, Message::Command(_)),
            "a tool-group action must ride the command channel"
        );
        assert_ne!(action, Message::Tools(ToolsDialogMessage::CloseRequested));
    }
}

#[test]
fn test_agent_debug_channel_carries_only_what_the_command_channel_cannot() {
    // [TASK-017b4]: search text, auto-scroll, the JSON row count, Clear
    // and close are already `UserCommand`s, so the AgentDebug channel
    // holds only disclosure plus the two deferred/runtime payloads iced
    // will not let a view handle itself.
    let channel = [
        Message::AgentDebug(AgentDebugMessage::EntryToggled { id: 1 }),
        Message::AgentDebug(AgentDebugMessage::CopyJsonRequested {
            content: "{\"a\": 1}".to_string(),
        }),
        Message::AgentDebug(AgentDebugMessage::ScrollReported {
            offset_y: 1.0,
            viewport_height: 2.0,
            content_height: 3.0,
        }),
    ];

    for message in channel {
        assert!(
            matches!(message, Message::AgentDebug(_)),
            "a debug-window transition must arrive on its own channel"
        );
        assert!(
            !matches!(message, Message::Command(_)),
            "a debug-window transition must not masquerade as a published command"
        );
    }
}

#[test]
fn test_agent_debug_settings_stay_on_the_command_channel() {
    // No AgentDebug-channel variant duplicates these: the legacy UI window
    // published/mutated exactly these, and the iced card keeps doing so.
    let commands = [
        Message::Command(UserCommand::SetDebugSearchText("turn".into())),
        Message::Command(UserCommand::SetDebugAutoScroll(true)),
        Message::Command(UserCommand::SetDebugJsonRows(24)),
        Message::Command(UserCommand::ClearDebugEntries),
        Message::Command(UserCommand::ToggleDebugWindow(false)),
    ];

    for command in commands {
        assert!(
            matches!(command, Message::Command(_)),
            "a debug-window setting must ride the command channel"
        );
        assert_ne!(
            command,
            Message::AgentDebug(AgentDebugMessage::EntryToggled { id: 0 })
        );
    }
}

#[test]
fn test_search_channel_results_ready_carries_query_and_snapshot() {
    // TASK-019 added the async completion variant: the reducer runs the
    // reused `ui::tree_search` engine in a Task and reports back here.
    let results = Arc::new(vec![SearchResultEntry {
        path: sample_path(),
        file_name: "notes.md".into(),
        relative_path: "Docs: notes.md".into(),
        snippet: "…matched text…".into(),
        line_number: 4,
        match_count: 2,
    }]);
    let message = Message::Search(SearchMessage::ResultsReady {
        query: "match".into(),
        results: Arc::clone(&results),
    });

    assert_eq!(message.clone(), message, "clone preserves the snapshot");
    match message {
        Message::Search(SearchMessage::ResultsReady { query, results }) => {
            assert_eq!(query, "match");
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].match_count, 2);
        }
        other => panic!("expected Search::ResultsReady, got {other:?}"),
    }
}

#[test]
fn test_boot_channel_variants_are_distinct_and_clone_preserving() {
    // [TASK-022]: the boot-infrastructure channel carries the font-load
    // report and the DEC-009 publish confirmation; the two variants must
    // stay separable and clone exactly.
    let font_failed = Message::Boot(BootMessage::FontLoadFinished(Some("boom".to_string())));
    let font_ok = Message::Boot(BootMessage::FontLoadFinished(None));
    let published = Message::Boot(BootMessage::ConfigPublished);

    assert_eq!(font_failed.clone(), font_failed);
    assert_eq!(font_ok.clone(), font_ok);
    assert_eq!(published.clone(), published);
    assert_ne!(font_failed, font_ok, "failure vs success must be separable");
    assert_ne!(font_ok, published, "font report vs config publish report");
    assert!(!matches!(published, Message::Command(_)));
}

#[test]
fn test_search_results_ready_is_distinct_from_submitted() {
    assert_ne!(
        Message::Search(SearchMessage::ResultsReady {
            query: "same".into(),
            results: Arc::new(Vec::new()),
        }),
        Message::Search(SearchMessage::Submitted {
            query: "same".into(),
        })
    );
}

#[test]
fn test_inbound_channel_is_not_a_widget_channel() {
    // Every inbound variant is distinct from every panel channel that could
    // superficially look similar — background notifications never ride the
    // Agent/Logs/Tree channels ([TASK-019]).
    let inbound_logs = Message::Inbound(InboundMessage::EventsDropped {
        source: BusSource::Background,
        count: 1,
    });
    assert_ne!(
        inbound_logs,
        Message::Logs(LogsMessage::Cleared),
        "a dropped-event report is not a user clearing the log list"
    );
    assert_ne!(
        Message::Inbound(InboundMessage::EventsDropped {
            source: BusSource::Agent,
            count: 1,
        }),
        Message::Agent(AgentMessage::InputChanged {
            text: String::new()
        }),
        "an agent notification is not agent input"
    );
}

#[test]
fn test_bus_source_is_copy_comparable_and_hashable() {
    assert_copy::<BusSource>();
    let a = BusSource::Config;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(BusSource::Config, BusSource::File);
    assert_ne!(BusSource::File, BusSource::Agent);
    assert_ne!(BusSource::Agent, BusSource::Background);
}
