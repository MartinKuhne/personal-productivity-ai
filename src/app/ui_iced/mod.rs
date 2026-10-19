//! Iced-based UI layer (The Elm Architecture). Lives beside `ui/`; [TASK-029b]
//! removed the legacy renderer layer that used to sit next to it.
//!
//! The runtime bootstrap is exercised by the `fastmd` binary itself
//! (`src/app/main.rs`), which [TASK-024] flipped onto this launch chain; the
//! `fastmd_iced` pilot that first proved it was retired in [TASK-029b]. The
//! subsystem tree grows module by module in Phases 2..4 (`doc/planning/
//! iced.md`):
//!
//! - [`fonts`] — `iced::font::load` bootstrap for the FastMD font
//!   collections ([TASK-007]),
//! - [`theme`] — FastMD palette tokens mapped into a custom `iced::Theme`
//!   ([TASK-008], [GUD-003]),
//! - [`persisted`] — `PersistedUiState` save/load behind an injected
//!   storage handler, plus DEC-008 scale composition and window-field
//!   mapping ([TASK-009], DEC-006/[RUST-024]),
//! - [`layout`] — the three-band pane-grid geometry with pure
//!   `crate::ui::PanelLayout`-bound resize reductions ([TASK-010], UI-001),
//! - [`message`] — the top-level `Message` enum, one variant per logical
//!   interaction channel, with `UserCommand` as the user-intent channel
//!   ([TASK-021], [FMD-068]),
//! - [`panels`] — the pane view functions; each takes an immutable
//!   view-model snapshot and emits `Message::Command` instead of publishing
//!   to the command bus itself ([TASK-011]..[TASK-016]).
//! - [`overlay`] — the shared context-menu and blocking-modal primitives:
//!   the overlay slot model, cursor anchoring with viewport clamping, the
//!   scrim that swallows input, and the confirm/cancel button pair
//!   ([TASK-017a], DEC-005).
//! - [`editor`] — the inline Markdown editor modal with the DEC-007
//!   undo/redo history stack, pre-save GFM validation and the
//!   front-matter-preserving save model ([TASK-014], UI-037..UI-045).
//! - [`render`] — the Markdown **viewing** render path: the pure
//!   `RenderEvent` → `RenderBlock` mapping and its iced widgets for
//!   headings, paragraphs, lists, task checkboxes, fenced code and
//!   YAML front matter ([TASK-015]).
//! - [`table_width`] — the iced adapter for the reused pure Fair Table
//!   Width Algorithm: available width → solver inputs → pinned iced
//!   column widths ([TASK-015]).
//! - [`logs`] — the Background Processes viewer (UI-047): the log table
//!   over a read-only snapshot of the same `background::logs` ring buffer,
//!   the reused filter predicate, the category pick list and the pure
//!   auto-scroll intent rule ([TASK-016]).
//! - [`dialogs`] — the small individual dialogs: the shared name-entry
//!   dialog (create-directory UI-011 / create-document UI-015), rename,
//!   move and the delete confirmation, as a pure
//!   [`crate::ui_iced::dialogs::DialogState`] model with merged name
//!   validation and cursor-anchored, clamped modal cards ([TASK-017b1]).
//! - [`about`] — the About window: application identity, `build.rs`
//!   metadata, the MIT license body and the entry point into the credits
//!   list, as a pure [`crate::ui_iced::about::AboutViewModel`] snapshot
//!   with the reducer-held
//!   [`crate::ui_iced::about::AboutWindowState`] ([TASK-017b2]).
//! - [`attributions`] — the third-party credits window over the reused
//!   `crate::ui::attributions::DIRECT_DEPENDENCIES` catalog: a scrollable
//!   Crate / Authors / Repository table whose rows emit deferred
//!   open-in-browser intent ([TASK-017b2]).
//! - [`batch_dialog`] — the Batch prompt-processing dialog: directory /
//!   pattern / prompt / mode / concurrency composition over the reused
//!   `BatchConfig` + `validate_batch_params`, the running/completed
//!   views, and the reducer-held
//!   [`crate::ui_iced::batch_dialog::BatchDialogState`] ([TASK-017b3],
//!   BATCH-001..BATCH-014).
//! - [`tools_dialog`] — the Tools dialog: the six-column tool-group table
//!   that replaces the codebase's only `the legacy_extras::TableBuilder` with
//!   `iced::widget::table`, over the reused tool-group registry, with
//!   per-row enable / restart / OAuth actions riding the command channel
//!   ([TASK-017b3], UI-051..UI-060).
//! - [`agent_debug`] — the agent debug window: the streaming LLM-traffic
//!   list over the reused `AgentSession` debug-entry buffer, with the
//!   codebase's only `CollapsingHeader` replaced by an explicit
//!   expanded-id set, the [TASK-016] auto-scroll intent rule reused for
//!   its stick-to-bottom, and search / auto-scroll / json-rows / Clear
//!   riding the command channel ([TASK-017b4], UI-048).
//! - [`agent`] — the agent panel: the streaming conversation view over the
//!   reused `AgentTranscript` (user turns themed, assistant turns through
//!   iced's **built-in** `iced::widget::markdown`), the UI-048
//!   thinking-delimiter gate and disclosure, the prompt row with send /
//!   queue / cancel and the chat-model `pick_list`, and the reused
//!   follow-the-stream report ([TASK-018], UI-048).
//! - [`search`] — the content-search view model: query / committed filter /
//!   result snapshot / in-progress flag as pure transitions, with the
//!   filesystem scan left to the reused `ui::tree_search` engine and the
//!   the legacy UI result-card predicates extracted as pure functions ([TASK-019],
//!   UI-050).
//! - [`subscription`] — the bus → iced bridge: every inbound
//!   background→UI notification (`ConfigArrived`, `FileEvent`,
//!   `AgentEvent`, `BackgroundEvent`) plus the `Lagged(n)` dropped-event
//!   report, folded into one `Subscription<Message>` merged with
//!   `Subscription::batch`, over a pure `bus_event_to_message` mapper
//!   ([TASK-019], FMD-065..FMD-067).
//! - [`logo`] — the shared "Document + Bolt" mark: the single place the
//!   Phosphor-bolt stand-in is constructed, since the retired vector paint
//!   routine was not portable under DEC-011's
//!   feature set ([TASK-020]).
//! - [`app`] — the iced application root: the framework-free [`app::AppState`]
//!   (a thin wrapper over the domain `FastMdApp`), the parallel framework-free
//!   constructor, the DEC-008/DEC-009 `boot` task chain and the verified
//!   `launch()` builder chain ([TASK-022], DEC-006/008/009/010).
//! - [`update`] — the top-level reducer: the DEC-006 close/save flow, the
//!   [TASK-010] pane reduction, the `Message::Command` publish-and-execute
//!   half and the boot-report reductions ([TASK-022] minimal; [TASK-023]
//!   completes the one-to-one mirror).
//! - [`view`] - the top-level view: the three-band layout with the real top
//!   toolbar, the middle pane grid (left tree / center document-or-agent /
//!   gated ToC) and the bottom command band, under the overlay host that
//!   stacks the context menu, editor, dialogs, log viewer, debug window,
//!   tools, About, attributions and batch windows in the legacy UI draw order
//!   ([TASK-023]).
//! - [`os_shell`] — the framework-free platform wrappers the reducer calls:
//!   open-in-system-editor, reveal-in-file-explorer and open-URL, copied out
//!   of the legacy UI tree so `ui_iced` owns them outright ([TASK-029]).
//! - `test_helpers` — `#[cfg(test)]` framework-free fixtures that build a
//!   `FastMdApp` or a whole [`app::AppState`] over in-memory storage for
//!   the iced sidecar tests ([TASK-029], [RUST-006]).
//!
//! # Self-containment ([TASK-029])
//!
//! `ui_iced` no longer `pub use`s any helper out of an the legacy UI file that
//! [TASK-029b] deletes. Every framework-free helper the iced panes need is
//! **owned** by the iced module that consumes it — see the `//!` note in
//! [`layout`], [`panels::left`], [`panels::center`], [`panels::top`],
//! [`panels::bottom`], [`panels::right`], [`table_width`], [`overlay`] and
//! [`os_shell`]. The legacy UI originals keep their own copies until Phase 5;
//! the transient duplication is deliberate.
//!
//! Imports that DO remain point at files the migration **keeps** (they are
//! RELOCATE / RESIDUE / RE-TYPE, not DELETE): `crate::ui::strings`,
//! `crate::ui::theme`, [`crate::ui::app`] (`FastMdApp`),
//! `crate::ui::tree_search`, `crate::ui::panel_layout`, `crate::ui::agent`,
//! `crate::ui::tree`, `crate::ui::attributions`,
//! [`crate::ui::PersistedUiState`] and [`crate::ui::CURRENT_SCHEMA_VERSION`].
//!
//! The view borrows every panel view-model out of
//! [`AppState::models`](app::AppState::models); those snapshots are rebuilt
//! by [`AppState::rebuild_view_models`](app::AppState::rebuild_view_models)
//! at the end of every [`update`] call. Any new `update` entry point MUST
//! call it, or the view renders a stale frame.
//!
//! `tree` follows in Phase 4.

pub mod about;
pub mod agent;
pub mod agent_debug;
pub mod app;
pub mod attributions;
pub mod batch_dialog;
pub mod dialogs;
pub mod editor;
pub mod fonts;
#[cfg(test)]
pub mod interaction_tests;
pub mod layout;
pub mod logo;
pub mod logs;
pub mod message;
pub mod os_shell;
pub mod overlay;
pub mod panels;
pub mod persisted;
pub mod render;
pub mod search;
pub mod subscription;
pub mod table_width;
#[cfg(test)]
pub mod test_helpers;
pub mod theme;
pub mod tools_dialog;
pub mod update;
pub mod view;
