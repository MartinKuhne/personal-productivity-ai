# FastMD crate

## Event-driven architecture

- [FMD-060] The user interface MUST remain responsive while any background work is in progress.
- [FMD-061] A user action MUST be accepted within one frame regardless of background load. The system MUST NOT block user input awaiting completion of background work.
- [FMD-062] Long-running work SHOULD be cancellable/interruptible by a subsequent user action.
- [FMD-063] Subsystems that produce work and subsystems that consume it MUST be decoupled. A producer MUST NOT require a consumer to be available in order to complete.
- [FMD-064] Failure of one background subsystem MUST NOT crash or hang the user interface or other subsystems.
- [FMD-065] The system MUST enforce a single direction of dependency: background work notifies the UI of results; the UI MUST NOT be synchronously invoked by background work.
- [FMD-066] Events published by any subsystem MUST be delivered to all interested subscribers. The system SHOULD preserve publication order per publisher.
- [FMD-067] If the system cannot keep up and must discard events, it SHOULD notify subscribers of the loss and MUST provide a means to re-synchronize to the current state on the next valid event.
- [FMD-068] User intent MUST be represented as a self-contained command carrying all information needed to execute it. Execution MUST be deterministic and replayable from the command alone.
- [FMD-069] Application state that is observable by the user MUST be updated only on the user-interface thread.
- [FMD-070] Configuration and workspace state MUST be consistently observable by background work without requiring direct access to user-interface state.
- [FMD-071] Every cross-subsystem interaction MUST be observable as a discrete event for logging and debugging.
- [FMD-072] Dropped or delayed events SHOULD be reported as warnings with count, not silently ignored.

## 3. Required Review and Implementation Rules
- You MUST consult [doc/distill/iced.md](../../doc/distill/iced.md) before you write or review UI code. It covers the Elm loop, the widget set this app actually uses, the four test tiers, the theme/palette mapping and the conventions below.
- The live UI lives in [`ui_iced/`](ui_iced/mod.rs). The [`ui/`](ui/mod.rs) tree is the framework-free domain the iced UI builds on (view-models, strings, palette tokens, tab/selection/tree/persisted state) — no rendering code goes there.
- Keep `update` side-effect free where possible: mutate state, return `iced::Task`s. Never publish to a bus, read the filesystem, or spawn work from `view`.
- `AppState::rebuild_view_models()` MUST run at the end of every `update` entry point, or the view renders a stale frame.
- Keep pane layout ownership with `ui::panel_layout::PanelLayout` plus `ui_iced::layout` and do not add ad-hoc side panels in the top-level view.
- Keep cross-cutting UI state on `AppState` in [ui_iced/app.rs](ui_iced/app.rs): domain state on `app`, view-model snapshots in `models`, floating-window state in `overlay`. Split new UI concerns into a dedicated module when the state grows.
- Panels are pure view functions: they take an immutable view-model snapshot and emit `Message::Command(UserCommand)`; the reducer performs the publish and execution ([FMD-068]).

## 4. User Interface Text String Rules
- Isolate all user-facing text into [ui/strings.rs](ui/strings.rs) as `pub const` values or formatting helpers.
- Do not hardcode UI literals in panel, modal, tree, or editor modules.
- Reference strings through `crate::ui::strings::<CONST_NAME>`.
- Add a `///` doc comment to every `pub const` or helper in [ui/strings.rs](ui/strings.rs).

## 4a. Colour Palette Rules
- All numeric colours (`iced::Color::from_rgb`, `from_rgb8`, `from_rgba*`) MUST live in [ui/theme.rs](ui/theme.rs) as [`Palette`](ui/theme.rs) tokens or [`EditorColors`](ui/theme.rs). The token type is `iced::Color` ([GUD-003]).
- Do not hardcode numeric colours in panel, modal, tree, renderer, or editor modules. Resolve them via `Palette::fastmd_dark().<token>`.
- The mapping from FastMD tokens to iced palette roles lives in [ui_iced/theme.rs](ui_iced/theme.rs) and adds no literals of its own.
- Add a `///` doc comment to every `Palette` field describing the role it serves.
- Named iced constants (`iced::Color::WHITE`, `BLACK`, `TRANSPARENT`) remain acceptable for non-brand, self-describing uses.
- The one exception is self-contained brand artwork in [ui_iced/logo.rs](ui_iced/logo.rs), whose tints are local to the mark and not part of the UI palette.

## 5. `iced::widget::Id` Stability Rules
- Give every stateful widget (scrollable, text editor, pane grid) an explicit `iced::widget::Id` so the reducer can address it and the widget keeps its state across tree rebuilds.
- Define the id as a `pub const &str` in the module that owns the widget and pass it through `.id(...)`; `iced::widget::Id` is `From<&'static str>`.
- Do not reuse the same id for two live widgets. One id, one live widget.
- `Id::unique()` produces a different id on every call — never use it for anything the reducer must address later, and never call it from `view` (the id would change every frame).
- Pane and split ids are **discovered** from the built `pane_grid::State`, never constructed: the wrapper ints on `pane_grid::{Pane, Split}` are not publicly constructible.

Use this pattern for an addressable scrollable:
```rust
/// Stable id for the background-log scrollable.
pub const LOG_SCROLLABLE_ID: &str = "background_logs_scroll";

scrollable(content).id(LOG_SCROLLABLE_ID).on_scroll(Reported)
```
And to drive it from the reducer:
```rust
operation::snap_to_end(LOG_SCROLLABLE_ID)
```

## 6. Conditional Rendering Rules
- Branch on the **view-model**, never on live widget state. The tree is rebuilt from state every frame, so a branch that changes widget count is fine — the state behind it is what must be stable.
- Prefer `button(...).on_press_maybe(option)` over removing a control when the action is merely unavailable; the control stays visible and disabled.
- Keep the **ids** of stateful widgets stable across a branch: a scrollable that disappears and returns under a different id loses its scroll position, and a `text_editor` whose `Content` is rebuilt loses the caret.
- Mount floating surfaces only through the overlay host's `OverlayLayer` z-order ([ui_iced/app.rs](ui_iced/app.rs)); never stack an ad-hoc layer inside a panel.
- Gate whole panes with the pure rule functions (e.g. `ui_iced::panels::right::should_show_panel`) rather than ad-hoc conditionals at the call site.
- For empty/loading states, render an explicit placeholder element rather than an empty container, so the layout does not collapse silently.
