//! Sidecar unit tests for the top-level iced `Message` vocabulary.
//!
//! Pure data tests only ([RUST-006]): every path is a synthetic relative
//! fixture literal that is never opened, and no user profile, live
//! configuration file or database is touched.

use super::{
    AboutMessage, AgentDebugMessage, AgentMessage, BatchDialogMessage, BootMessage, BusSource,
    DialogMessage, EditorMessage, InboundMessage, LayoutMessage, LogsMessage, Message, MovableCard,
    MovableMessage, RenderMessage, SearchMessage, TabMessage, ThemeMessage, ToolsDialogMessage,
    TooltipId, TreeMessage, WindowMessage, WindowSnapshot,
};
use crate::agent::batch::types::BatchMode;
use crate::bus::events::messages::LogCategory;
use crate::bus::events::user_command::UserCommand;
use crate::bus::events::{
    AgentEvent, BackgroundEvent, DirectoryChanged, DirectoryChangedKind, FileChanged, FsEvent,
};
use crate::config::AppConfig;
use crate::ui::tree_search::SearchResultEntry;
use crate::ui_iced::layout::middle_band_state;
use hoarfrost::geometry::clamp::ResizeHandle;
use iced::widget::text_editor::{Action, Edit};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

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
    // The Agent channel carries the thinking-disclosure and
    // transcript-scroll-report variants.
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

    // Every payload field of these three channels is `Copy`, so the channel
    // itself must be — a missing `Copy` derive fails this at compile time.
    assert_copy::<WindowMessage>();
    assert_clone::<LayoutMessage>();
    assert_copy::<ThemeMessage>();
    assert_copy::<WindowSnapshot>();

    // `TabMessage` left this list in INT-12. It carries a `PathBuf` on the
    // hover variants now, so it cannot be `Copy`; the pin is doing its job
    // by re-classifying the channel rather than by staying satisfied.
    // `LayoutMessage` left this list when tooltip hover/focus variants added
    // `String` payloads (tooltip text for dynamic tooltips), making it
    // `Clone` rather than `Copy`.
    assert_clone::<TabMessage>();
}

#[test]
fn test_tab_channel_is_clone_rather_than_copy_now_that_it_carries_a_path() {
    // The by-value-move proof, inverted: `TabMessage` no longer compiles
    // as `Copy`, so the clone is explicit. Asserted through `Clone` so a
    // future change that re-added `Copy` here (by dropping the path
    // payload back out) is a deliberate edit rather than a silent drift.
    let entered = TabMessage::Entered {
        path: sample_path(),
    };
    let cloned = entered.clone();
    assert_eq!(entered, cloned);

    let left = TabMessage::Left {
        path: sample_path(),
    };
    assert_ne!(entered, left);
    assert_ne!(
        TabMessage::ContextMenuClosed,
        left,
        "the hover variants must be distinct from the menu variants"
    );
}

#[test]
fn test_copy_channel_value_survives_a_by_value_move() {
    // Runtime half of the `Copy` proof: `message` stays usable after `copy`
    // is bound by value, which only compiles for `Copy` types.
    let message = ThemeMessage::ScaleChanged { multiplier: 1.5 };
    let copy = message;
    assert_eq!(message, copy);
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
    // The window and layout channels carry every runtime report the window
    // layer and the pane grid produce.
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

    // Distinctness: no two of these messages collapse onto each other, so
    // the reducer's branches stay separable.
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

    assert_ne!(edit, motion, "edit vs caret motion must be separable");
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
fn test_logs_channel_covers_every_filter_control() {
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
fn test_cursor_moved_is_a_clone_layout_report_that_stays_distinct() {
    // The context-menu anchor. `MouseArea::on_right_press`
    // cannot carry a position, so the reducer has to remember this one.
    let moved = LayoutMessage::CursorMoved {
        point: iced::Point::new(12.0, 34.0),
    };
    let cloned = moved.clone();
    assert_eq!(moved, cloned, "CursorMoved must keep the channel `Clone`");

    // Use cloned for Message::Layout since moved is consumed
    assert_eq!(
        Message::Layout(cloned.clone()),
        Message::Layout(LayoutMessage::CursorMoved {
            point: iced::Point::new(12.0, 34.0),
        })
    );
    assert_ne!(
        Message::Layout(cloned.clone()),
        Message::Layout(LayoutMessage::CursorMoved {
            point: iced::Point::new(12.0, 35.0),
        }),
        "a different point must not compare equal"
    );
    assert_ne!(
        Message::Layout(cloned),
        Message::Layout(LayoutMessage::TreeAutoFit),
        "cursor motion must not collapse into the auto-fit request"
    );
}

/// The two auto-fit requests are distinct transitions: the tree fits the LEFT
/// pane and the ToC fits the RIGHT one, so a replay must not be able to
/// substitute one for the other (defect #14).
#[test]
fn test_toc_auto_fit_is_its_own_request() {
    assert_eq!(
        LayoutMessage::TocAutoFit,
        LayoutMessage::TocAutoFit,
        "the channel cannot derive `PartialEq`, so the hand-written impl must cover \
         the new variant"
    );
    assert_ne!(
        LayoutMessage::TocAutoFit,
        LayoutMessage::TreeAutoFit,
        "fitting the ToC pane is not the same transition as fitting the tree pane"
    );
}

#[test]
fn test_tooltip_hover_enter_can_carry_dynamic_text() {
    // ToolsError tooltip needs dynamic text from the row's error
    let enter = LayoutMessage::TooltipHoverEnter {
        id: TooltipId::ToolsError,
        tooltip_text: Some("Authentication: invalid credentials".to_string()),
    };
    let cloned = enter.clone();
    assert_eq!(enter, cloned);

    // SearchTrigger tooltip doesn't need dynamic text (uses static string)
    let enter_search = LayoutMessage::TooltipHoverEnter {
        id: TooltipId::SearchTrigger,
        tooltip_text: None,
    };
    // Verify via pattern matching
    if let LayoutMessage::TooltipHoverEnter { id, tooltip_text } = enter_search {
        assert_eq!(id, TooltipId::SearchTrigger);
        assert!(tooltip_text.is_none());
    } else {
        panic!("expected TooltipHoverEnter variant");
    }
}

#[test]
fn test_tooltip_focus_enter_shows_immediately() {
    // Keyboard focus should show tooltip immediately (0ms delay)
    let enter = LayoutMessage::TooltipFocusEnter {
        id: TooltipId::SearchTrigger,
        tooltip_text: None,
    };
    let cloned = enter.clone();
    assert_eq!(enter, cloned);
}

#[test]
fn test_tree_deferred_intent_variants_are_all_distinct() {
    // The menu rows that read files or fan out
    // inside the click have no single `UserCommand`, so each needs its own
    // separable message.
    let deferred = [
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
    // `Debug` is required for tracing every cross-subsystem interaction;
    // spot-check that the rendering names the channels.
    let rendered = format!("{:?}", one_message_per_channel());
    for channel in [
        "Window", "Layout", "Command", "Tree", "Tab", "Editor", "Agent", "Search", "Logs", "Theme",
        "Render", "Dialog", "About",
    ] {
        assert!(
            rendered.contains(channel),
            "Debug output must name the {channel} channel for tracing"
        );
    }
}

#[test]
fn test_dialog_channel_covers_every_small_dialog_interaction() {
    // The name-entry / move / delete dialogs' view-local
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
    // The About + attributions windows' view-local
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
    // Channel choice: About/attributions deliberately do NOT
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
    // The batch dialog's composed selection. Each variant
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
    // Channel choice: only the *selection* needed a new
    // channel. Open / Start / Cancel already ride `Message::Command`, so no
    // `BatchDialogMessage` variant duplicates them.
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
    // Every tool-group action is already a `UserCommand`,
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
    // Search text, auto-scroll, the JSON row count, Clear
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
    // No AgentDebug-channel variant duplicates these: the card emits
    // exactly these as commands.
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
    // The async completion variant: the reducer runs the
    // `ui::tree_search` engine in a Task and reports back here.
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
    // The boot-infrastructure channel carries the font-load
    // report (the config publish is synchronous inside
    // `build_state`, so no boot task reports it); variants must stay
    // separable and clone exactly.
    let font_failed = Message::Boot(BootMessage::FontLoadFinished(Some("boom".to_string())));
    let font_ok = Message::Boot(BootMessage::FontLoadFinished(None));

    assert_eq!(font_failed.clone(), font_failed);
    assert_eq!(font_ok.clone(), font_ok);
    assert_ne!(font_failed, font_ok, "failure vs success must be separable");
    assert!(matches!(font_ok, Message::Boot(_)));
    assert!(!matches!(font_ok, Message::Command(_)));
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
    // Agent/Logs/Tree channels.
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

// ---------------------------------------------------------------------------
// Hand-written `PartialEq` arms
// ---------------------------------------------------------------------------
//
// `LayoutMessage` and `InboundMessage` cannot derive `PartialEq` (see the impl
// docs in `message.rs`), so every arm of the two comparisons is behaviour no
// compiler check protects — including the `_ => false` fall-through that keeps
// two structurally similar variants apart. Each arm is pinned below with its
// equal case and its field-mismatch case, and the fall-through is swept
// pairwise across the whole channel.

/// One arm of a hand-written `PartialEq`: the case label, the two messages and
/// whether the impl is expected to call them equal.
type EqualityCase<M> = (&'static str, M, M, bool);

/// The middle band's two splits. `pane_grid::Split` is not publicly
/// constructable, so a `PaneResized` fixture has to take real ids off an
/// in-memory pane-grid state ([RUST-006]: built in memory, nothing read).
fn band_splits() -> (
    iced::widget::pane_grid::Split,
    iced::widget::pane_grid::Split,
) {
    let (_panes, band) = middle_band_state();
    (band.left_split, band.right_split)
}

/// A pane-resize report on `split` at `ratio`.
fn pane_resized(split: iced::widget::pane_grid::Split, ratio: f32) -> LayoutMessage {
    LayoutMessage::PaneResized(iced::widget::pane_grid::ResizeEvent { split, ratio })
}

/// A cursor report at `(x, y)`.
fn cursor(x: f32, y: f32) -> LayoutMessage {
    LayoutMessage::CursorMoved {
        point: iced::Point::new(x, y),
    }
}

/// A tooltip pointer-enter for `id` carrying optional dynamic `text`.
fn hover_enter(id: TooltipId, text: Option<&str>) -> LayoutMessage {
    LayoutMessage::TooltipHoverEnter {
        id,
        tooltip_text: text.map(str::to_string),
    }
}

/// A tooltip keyboard-focus-enter for `id` carrying optional dynamic `text`.
fn focus_enter(id: TooltipId, text: Option<&str>) -> LayoutMessage {
    LayoutMessage::TooltipFocusEnter {
        id,
        tooltip_text: text.map(str::to_string),
    }
}

/// `(case, left, right, are_equal)` for every arm of
/// `PartialEq for LayoutMessage`.
fn layout_equality_cases() -> Vec<EqualityCase<LayoutMessage>> {
    let (left, right) = band_splits();
    vec![
        (
            "PaneResized on the same split at the same ratio",
            pane_resized(left, 0.4),
            pane_resized(left, 0.4),
            true,
        ),
        (
            "PaneResized on the same split at another ratio",
            pane_resized(left, 0.4),
            pane_resized(left, 0.45),
            false,
        ),
        (
            "PaneResized on the other split at the same ratio",
            pane_resized(left, 0.4),
            pane_resized(right, 0.4),
            false,
        ),
        (
            "TreeAutoFit against itself",
            LayoutMessage::TreeAutoFit,
            LayoutMessage::TreeAutoFit,
            true,
        ),
        (
            "TocAutoFit against itself",
            LayoutMessage::TocAutoFit,
            LayoutMessage::TocAutoFit,
            true,
        ),
        (
            "CursorMoved to the same point",
            cursor(12.0, 34.0),
            cursor(12.0, 34.0),
            true,
        ),
        (
            "CursorMoved to another point",
            cursor(12.0, 34.0),
            cursor(12.0, 35.0),
            false,
        ),
        (
            "TooltipHoverEnter same id, no dynamic text",
            hover_enter(TooltipId::SearchTrigger, None),
            hover_enter(TooltipId::SearchTrigger, None),
            true,
        ),
        (
            "TooltipHoverEnter same id, same dynamic text",
            hover_enter(TooltipId::ToolsError, Some("auth failed")),
            hover_enter(TooltipId::ToolsError, Some("auth failed")),
            true,
        ),
        (
            "TooltipHoverEnter same id, another dynamic text",
            hover_enter(TooltipId::ToolsError, Some("auth failed")),
            hover_enter(TooltipId::ToolsError, Some("timeout")),
            false,
        ),
        (
            "TooltipHoverEnter same text, another id",
            hover_enter(TooltipId::ToolsError, Some("auth failed")),
            hover_enter(TooltipId::SearchTrigger, Some("auth failed")),
            false,
        ),
        (
            "TooltipHoverEnter one side carries text",
            hover_enter(TooltipId::ToolsError, Some("auth failed")),
            hover_enter(TooltipId::ToolsError, None),
            false,
        ),
        (
            "TooltipHoverExit same id",
            LayoutMessage::TooltipHoverExit {
                id: TooltipId::SearchClear,
            },
            LayoutMessage::TooltipHoverExit {
                id: TooltipId::SearchClear,
            },
            true,
        ),
        (
            "TooltipHoverExit another id",
            LayoutMessage::TooltipHoverExit {
                id: TooltipId::SearchClear,
            },
            LayoutMessage::TooltipHoverExit {
                id: TooltipId::SearchTrigger,
            },
            false,
        ),
        (
            "TooltipFocusEnter same id and text",
            focus_enter(TooltipId::ToolsError, Some("auth failed")),
            focus_enter(TooltipId::ToolsError, Some("auth failed")),
            true,
        ),
        (
            "TooltipFocusEnter another text",
            focus_enter(TooltipId::ToolsError, Some("auth failed")),
            focus_enter(TooltipId::ToolsError, Some("timeout")),
            false,
        ),
        (
            "TooltipFocusExit same id",
            LayoutMessage::TooltipFocusExit {
                id: TooltipId::ToolsError,
            },
            LayoutMessage::TooltipFocusExit {
                id: TooltipId::ToolsError,
            },
            true,
        ),
        (
            "TooltipFocusExit another id",
            LayoutMessage::TooltipFocusExit {
                id: TooltipId::ToolsError,
            },
            LayoutMessage::TooltipFocusExit {
                id: TooltipId::SearchTrigger,
            },
            false,
        ),
        (
            "TooltipHoverExit against TooltipFocusExit on the same id",
            LayoutMessage::TooltipHoverExit {
                id: TooltipId::SearchTrigger,
            },
            LayoutMessage::TooltipFocusExit {
                id: TooltipId::SearchTrigger,
            },
            false,
        ),
        (
            "TooltipHoverEnter against TooltipFocusEnter on the same id",
            hover_enter(TooltipId::SearchTrigger, None),
            focus_enter(TooltipId::SearchTrigger, None),
            false,
        ),
        (
            "TreeAutoFit against TocAutoFit",
            LayoutMessage::TreeAutoFit,
            LayoutMessage::TocAutoFit,
            false,
        ),
    ]
}

#[test]
fn test_layout_channel_equality_is_field_wise_in_every_arm() {
    for (case, left, right, expected) in layout_equality_cases() {
        let forward = left.clone() == right;
        // The impl matches on the tuple, so the answer must not depend on
        // which side each message is handed in from.
        let backward = right == left;
        assert_eq!(
            forward, expected,
            "LayoutMessage equality, {case}: forward comparison disagreed"
        );
        assert_eq!(
            backward, expected,
            "LayoutMessage equality, {case}: swapping the operands changed the answer"
        );
    }
}

#[test]
fn test_layout_channel_variants_are_pairwise_distinct() {
    // One representative per variant, all three tooltip ids deliberately the
    // same so the sweep cannot pass by accident: the hover and focus lanes
    // differ only in the variant, and the impl keeps them apart through its
    // `_ => false` arm.
    let (first_split, _second_split) = band_splits();
    let representatives = [
        pane_resized(first_split, 0.4),
        LayoutMessage::TreeAutoFit,
        LayoutMessage::TocAutoFit,
        cursor(1.0, 2.0),
        hover_enter(TooltipId::SearchTrigger, None),
        LayoutMessage::TooltipHoverExit {
            id: TooltipId::SearchTrigger,
        },
        focus_enter(TooltipId::SearchTrigger, None),
        LayoutMessage::TooltipFocusExit {
            id: TooltipId::SearchTrigger,
        },
    ];

    for (index, left) in representatives.iter().enumerate() {
        let replay = (*left).clone();
        assert_eq!(
            replay, *left,
            "a cloned LayoutMessage must compare equal to its original: {left:?}"
        );
        for (other, right) in representatives.iter().enumerate() {
            assert_eq!(
                left == right,
                index == other,
                "two different LayoutMessage variants must never compare equal: \
                 {left:?} against {right:?}"
            );
        }
    }
}

/// A synthetic file-change notification for `name`; the path is never opened
/// ([RUST-006]).
fn file_changed(name: &str) -> FileChanged {
    FileChanged::discovered_one(PathBuf::from("fixture-workspace").join(name))
}

/// A synthetic directory-change notification for `name` ([RUST-006]).
fn directory_changed(name: &str) -> DirectoryChanged {
    DirectoryChanged {
        path: PathBuf::from("fixture-workspace").join(name),
        kind: DirectoryChangedKind::Added,
    }
}

/// A synthetic agent notification with a fixed session id, so two of them can
/// carry byte-identical payloads.
fn agent_event(session: Uuid, text: &str) -> AgentEvent {
    AgentEvent::Thinking {
        session_id: session,
        text: text.to_string(),
    }
}

/// A synthetic typed background notification.
fn background_event() -> BackgroundEvent {
    BackgroundEvent::Fs(FsEvent::Finished)
}

#[test]
fn test_inbound_value_bearing_variants_compare_by_value() {
    let cases: Vec<EqualityCase<InboundMessage>> = vec![
        (
            "FileChanged same path and kind",
            InboundMessage::FileChanged(file_changed("a.md")),
            InboundMessage::FileChanged(file_changed("a.md")),
            true,
        ),
        (
            "FileChanged another path",
            InboundMessage::FileChanged(file_changed("a.md")),
            InboundMessage::FileChanged(file_changed("b.md")),
            false,
        ),
        (
            "FileChanged same path, another kind",
            InboundMessage::FileChanged(FileChanged::discovered_one(sample_path())),
            InboundMessage::FileChanged(FileChanged::updated_one(sample_path())),
            false,
        ),
        (
            "FileChangedBatch same list in the same order",
            InboundMessage::FileChangedBatch(vec![file_changed("a.md"), file_changed("b.md")]),
            InboundMessage::FileChangedBatch(vec![file_changed("a.md"), file_changed("b.md")]),
            true,
        ),
        (
            "FileChangedBatch reordered",
            InboundMessage::FileChangedBatch(vec![file_changed("a.md"), file_changed("b.md")]),
            InboundMessage::FileChangedBatch(vec![file_changed("b.md"), file_changed("a.md")]),
            false,
        ),
        (
            "FileChangedBatch one event short",
            InboundMessage::FileChangedBatch(vec![file_changed("a.md")]),
            InboundMessage::FileChangedBatch(vec![file_changed("a.md"), file_changed("b.md")]),
            false,
        ),
        (
            "DirectoryChanged same path and kind",
            InboundMessage::DirectoryChanged(directory_changed("docs")),
            InboundMessage::DirectoryChanged(directory_changed("docs")),
            true,
        ),
        (
            "DirectoryChanged same path, another kind",
            InboundMessage::DirectoryChanged(directory_changed("docs")),
            InboundMessage::DirectoryChanged(DirectoryChanged {
                path: PathBuf::from("fixture-workspace").join("docs"),
                kind: DirectoryChangedKind::Deleted,
            }),
            false,
        ),
        (
            "DirectoryChangedBatch same list",
            InboundMessage::DirectoryChangedBatch(vec![directory_changed("docs")]),
            InboundMessage::DirectoryChangedBatch(vec![directory_changed("docs")]),
            true,
        ),
        (
            "DirectoryChangedBatch another path",
            InboundMessage::DirectoryChangedBatch(vec![directory_changed("docs")]),
            InboundMessage::DirectoryChangedBatch(vec![directory_changed("notes")]),
            false,
        ),
        (
            "EventsDropped same source and count",
            InboundMessage::EventsDropped {
                source: BusSource::File,
                count: 2,
            },
            InboundMessage::EventsDropped {
                source: BusSource::File,
                count: 2,
            },
            true,
        ),
        (
            "EventsDropped another count",
            InboundMessage::EventsDropped {
                source: BusSource::File,
                count: 2,
            },
            InboundMessage::EventsDropped {
                source: BusSource::File,
                count: 3,
            },
            false,
        ),
        (
            "EventsDropped another source",
            InboundMessage::EventsDropped {
                source: BusSource::File,
                count: 2,
            },
            InboundMessage::EventsDropped {
                source: BusSource::Agent,
                count: 2,
            },
            false,
        ),
    ];

    for (case, left, right, expected) in cases {
        let equal = left == right;
        assert_eq!(
            equal, expected,
            "InboundMessage equality, {case}: expected {expected}, got {equal}"
        );
        assert_eq!(
            right == left,
            expected,
            "InboundMessage equality, {case}: swapping the operands changed the answer"
        );
    }
}

#[test]
fn test_inbound_opaque_payloads_compare_by_delivery_identity() {
    // `AppConfig`, `AgentEvent` and `BackgroundEvent` do not support `PartialEq`,
    // so the impl compares the `Arc` instead: two messages are equal iff they
    // carry the same shared delivery.
    let config = Arc::new(AppConfig::default());
    let session = Uuid::new_v4();
    let thinking = Arc::new(agent_event(session, "planning"));
    let fs = Arc::new(background_event());

    let pairs: Vec<EqualityCase<InboundMessage>> = vec![
        (
            "ConfigArrived: the same delivery",
            InboundMessage::ConfigArrived(Arc::clone(&config)),
            InboundMessage::ConfigArrived(Arc::clone(&config)),
            true,
        ),
        (
            "ConfigArrived: two deliveries of an identical config",
            InboundMessage::ConfigArrived(Arc::new(AppConfig::default())),
            InboundMessage::ConfigArrived(Arc::new(AppConfig::default())),
            false,
        ),
        (
            "Agent: the same delivery",
            InboundMessage::Agent(Arc::clone(&thinking)),
            InboundMessage::Agent(Arc::clone(&thinking)),
            true,
        ),
        (
            "Agent: two byte-identical deliveries",
            InboundMessage::Agent(Arc::new(agent_event(session, "planning"))),
            InboundMessage::Agent(Arc::new(agent_event(session, "planning"))),
            false,
        ),
        (
            "Background: the same delivery",
            InboundMessage::Background(Arc::clone(&fs)),
            InboundMessage::Background(Arc::clone(&fs)),
            true,
        ),
        (
            "Background: two byte-identical deliveries",
            InboundMessage::Background(Arc::new(background_event())),
            InboundMessage::Background(Arc::new(background_event())),
            false,
        ),
    ];

    for (case, left, right, expected) in pairs {
        if expected {
            assert_eq!(
                left, right,
                "InboundMessage equality, {case}: a clone of the same Arc must compare equal"
            );
            assert_eq!(
                left.clone(),
                right.clone(),
                "InboundMessage equality, {case}: cloning must preserve delivery identity"
            );
        } else {
            assert_ne!(
                left, right,
                "InboundMessage equality, {case}: two independent deliveries are two deliveries"
            );
        }
    }
}

#[test]
fn test_inbound_background_batch_is_compared_arc_by_arc() {
    let first = Arc::new(background_event());
    let second = Arc::new(background_event());
    let batch = InboundMessage::BackgroundBatch(vec![Arc::clone(&first), Arc::clone(&second)]);

    assert_eq!(
        batch,
        InboundMessage::BackgroundBatch(vec![Arc::clone(&first), Arc::clone(&second)]),
        "the same two deliveries in the same order are the same batch"
    );
    assert_ne!(
        batch,
        InboundMessage::BackgroundBatch(vec![Arc::clone(&second), Arc::clone(&first)]),
        "the batch is compared position by position, so a reorder is a different batch"
    );
    assert_ne!(
        batch,
        InboundMessage::BackgroundBatch(vec![Arc::clone(&first)]),
        "a batch of two is not a batch of one"
    );
    assert_ne!(
        batch,
        InboundMessage::BackgroundBatch(vec![
            Arc::new(background_event()),
            Arc::new(background_event())
        ]),
        "look-alike payloads are not the same deliveries"
    );
    assert_eq!(
        batch.clone(),
        batch,
        "cloning a batch must preserve every delivery identity"
    );
}

/// **Regression test for the missing `AgentBatch` equality arm (defect D3).**
///
/// `PartialEq for InboundMessage` had an arm for `BackgroundBatch` but none for
/// `AgentBatch`, so every pair of agent batches fell through to `_ => false` —
/// the comparison was not even reflexive, and a clone of a batch did not equal
/// its original. The arm now mirrors `BackgroundBatch`: same length, each pair
/// the same shared delivery.
#[test]
fn defect_03_inbound_agent_batch_compares_by_delivery_identity() {
    let event = Arc::new(agent_event(Uuid::new_v4(), "planning"));
    let batch = InboundMessage::AgentBatch(vec![Arc::clone(&event)]);
    let replay = InboundMessage::AgentBatch(vec![Arc::clone(&event)]);

    assert_eq!(
        batch, replay,
        "two batches carrying the same deliveries must compare equal"
    );
    assert_eq!(replay, batch, "the arm must be symmetric");
    assert!(
        batch.eq(&batch),
        "an AgentBatch must compare equal to itself (the missing arm broke reflexivity)"
    );
    assert_eq!(
        batch,
        batch.clone(),
        "cloning an AgentBatch must preserve every delivery pointer and stay equal"
    );

    // A different delivery is a different batch, even when the payloads are
    // byte-identical: agent events are compared by delivery identity.
    let other = InboundMessage::AgentBatch(vec![Arc::new(agent_event(Uuid::nil(), "planning"))]);
    assert_ne!(
        batch, other,
        "an independently delivered event is a second delivery, not the same one"
    );

    // Same deliveries, different length.
    let longer = InboundMessage::AgentBatch(vec![Arc::clone(&event), Arc::clone(&event)]);
    assert_ne!(
        batch, longer,
        "a batch with one more delivery is not the same batch"
    );
    assert_ne!(
        longer, batch,
        "the length check must hold in both directions"
    );

    // Two batches that are both empty are equal — the arm must not special-case
    // an empty burst.
    assert_eq!(
        InboundMessage::AgentBatch(Vec::new()),
        InboundMessage::AgentBatch(Vec::new()),
        "two empty bursts are the same (empty) burst"
    );

    // Cross-variant: an AgentBatch is never equal to a single Agent carrying the
    // very same delivery, nor to any other variant.
    assert_ne!(
        batch,
        InboundMessage::Agent(Arc::clone(&event)),
        "a burst of one is not the same message as the single delivery"
    );
    assert_ne!(
        batch,
        InboundMessage::BackgroundBatch(Vec::new()),
        "an agent burst must never compare equal to a background burst"
    );
    assert_ne!(
        batch,
        InboundMessage::EventsDropped {
            source: BusSource::Agent,
            count: 1,
        },
        "an agent burst must never compare equal to a lag report"
    );
}

#[test]
fn test_inbound_variants_are_pairwise_distinct() {
    // `AgentBatch` now has its equality arm (see
    // [`defect_03_inbound_agent_batch_compares_by_delivery_identity`]) and takes
    // part in the sweep like every other variant.
    let representatives = [
        InboundMessage::ConfigArrived(Arc::new(AppConfig::default())),
        InboundMessage::FileChanged(file_changed("a.md")),
        InboundMessage::FileChangedBatch(vec![file_changed("a.md")]),
        InboundMessage::DirectoryChanged(directory_changed("docs")),
        InboundMessage::DirectoryChangedBatch(vec![directory_changed("docs")]),
        InboundMessage::Agent(Arc::new(agent_event(Uuid::new_v4(), "planning"))),
        InboundMessage::AgentBatch(vec![Arc::new(agent_event(Uuid::new_v4(), "planning"))]),
        InboundMessage::Background(Arc::new(background_event())),
        InboundMessage::BackgroundBatch(vec![Arc::new(background_event())]),
        InboundMessage::EventsDropped {
            source: BusSource::Background,
            count: 1,
        },
    ];

    for (index, left) in representatives.iter().enumerate() {
        let replay = (*left).clone();
        assert_eq!(
            replay, *left,
            "a cloned InboundMessage must compare equal to its original: {left:?}"
        );
        for (other, right) in representatives.iter().enumerate() {
            assert_eq!(
                left == right,
                index == other,
                "two different InboundMessage variants must never compare equal: \
                 {left:?} against {right:?}"
            );
        }
    }
}

// --- the movable-card pointer channel -------------------------------------

#[test]
fn test_movable_channel_names_its_card_in_every_variant() {
    assert_copy::<MovableCard>();
    assert_clone::<MovableMessage>();

    for card in [MovableCard::Logs, MovableCard::AgentDebug] {
        let events: [(&str, MovableMessage); 7] = [
            (
                "Moved",
                MovableMessage::Moved {
                    card,
                    at: iced::Point::new(4.0, 8.0),
                },
            ),
            ("TitlePressed", MovableMessage::TitlePressed { card }),
            ("BodyPressed", MovableMessage::BodyPressed { card }),
            (
                "GripPressed",
                MovableMessage::GripPressed {
                    card,
                    handle: ResizeHandle::BottomRight,
                },
            ),
            ("DockRequested", MovableMessage::DockRequested { card }),
            ("CloseRequested", MovableMessage::CloseRequested { card }),
            ("Released", MovableMessage::Released { card }),
        ];

        for (name, event) in events {
            assert_eq!(
                event.card_kind(),
                card,
                "{name} must report the card it was produced for, or the reducer \
                 drives the wrong host"
            );
            assert_eq!(event.clone(), event, "{name} must survive a clone exactly");
        }
    }
}

#[test]
fn test_movable_events_for_different_cards_are_never_equal() {
    // The reducer picks the host from the message alone, so a Logs gesture must
    // never be mistaken for the same gesture on the Agent Debug card.
    let pairs: [(&str, MovableMessage, MovableMessage); 7] = [
        (
            "Moved",
            MovableMessage::Moved {
                card: MovableCard::Logs,
                at: iced::Point::new(4.0, 8.0),
            },
            MovableMessage::Moved {
                card: MovableCard::AgentDebug,
                at: iced::Point::new(4.0, 8.0),
            },
        ),
        (
            "TitlePressed",
            MovableMessage::TitlePressed {
                card: MovableCard::Logs,
            },
            MovableMessage::TitlePressed {
                card: MovableCard::AgentDebug,
            },
        ),
        (
            "BodyPressed",
            MovableMessage::BodyPressed {
                card: MovableCard::Logs,
            },
            MovableMessage::BodyPressed {
                card: MovableCard::AgentDebug,
            },
        ),
        (
            "GripPressed",
            MovableMessage::GripPressed {
                card: MovableCard::Logs,
                handle: ResizeHandle::Right,
            },
            MovableMessage::GripPressed {
                card: MovableCard::AgentDebug,
                handle: ResizeHandle::Right,
            },
        ),
        (
            "DockRequested",
            MovableMessage::DockRequested {
                card: MovableCard::Logs,
            },
            MovableMessage::DockRequested {
                card: MovableCard::AgentDebug,
            },
        ),
        (
            "CloseRequested",
            MovableMessage::CloseRequested {
                card: MovableCard::Logs,
            },
            MovableMessage::CloseRequested {
                card: MovableCard::AgentDebug,
            },
        ),
        (
            "Released",
            MovableMessage::Released {
                card: MovableCard::Logs,
            },
            MovableMessage::Released {
                card: MovableCard::AgentDebug,
            },
        ),
    ];

    for (name, logs, debug) in pairs {
        assert_ne!(
            logs, debug,
            "{name} on the Logs card must not equal the same {name} on the \
             Agent Debug card"
        );
    }
}

#[test]
fn test_movable_grip_presses_are_distinguished_by_handle() {
    // Every grip drives a different edge of the host, so two presses on two
    // grips of one card are two different gestures.
    let grips = [
        ResizeHandle::TopLeft,
        ResizeHandle::Top,
        ResizeHandle::TopRight,
        ResizeHandle::Right,
        ResizeHandle::BottomRight,
        ResizeHandle::Bottom,
        ResizeHandle::BottomLeft,
        ResizeHandle::Left,
    ];
    let events: Vec<MovableMessage> = grips
        .iter()
        .map(|handle| MovableMessage::GripPressed {
            card: MovableCard::Logs,
            handle: *handle,
        })
        .collect();

    for (index, left) in events.iter().enumerate() {
        for (other, right) in events.iter().enumerate() {
            assert_eq!(
                left == right,
                index == other,
                "a grip press must be distinguishable per handle: {:?} against {:?}",
                grips[index],
                grips[other]
            );
        }
    }
}
