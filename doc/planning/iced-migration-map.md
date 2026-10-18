# Iced Migration Map — egui→iced Widget Mapping & UI Traceability Matrix

> **Artifact contract:** Created by `TASK-001` (Part A) and `TASK-002` (Part B) of
> [`doc/planning/iced.md`](iced.md). This file is the **contract of record** for all later
> migration phases; widget choices recorded here MUST NOT be silently changed — open a plan
> revision instead. Test-tier citations in Part B are completed by `TASK-028` (coverage matrix).
>
> Part of [`doc/planning/iced.md`](iced.md). Requirements traceability source:
> [`src/app/ui/SPEC.md`](../../src/app/ui/SPEC.md) (`UI-001`..`UI-066`; owned set per header is
> UI-001..UI-050 + TBL-001..TBL-051, with the Tools-dialog/hamburger requirements UI-051..066
> defined inside this SPEC file). **Do not edit SPEC.md** ([RUST-043]).

## Part A — egui → iced 0.14 widget mapping (`TASK-001`)

All egui usage-site counts verified 2026-10-17 by recursive sweep of `src/app/ui/**`
(shipping code; `*_tests.rs`, `test_helpers/**`, `render/e2e_tests/**` excluded unless noted).
iced equivalents are verified against the local
`iced-0.14.0` crate sources (see `doc/iced.md` for citations) — no API in this table is guessed.

| # | egui API (sites) | iced 0.14 equivalent | FastMD call sites | Gap flag |
|---|------------------|----------------------|-------------------|----------|
| A1 | `CentralPanel` (8): `panels/center.rs` (3), `panels/right.rs` (1), `render/mod.rs` (4) | Nested composition: `column![]` of three full-width bands (top/middle/bottom); middle band is `iced::widget::pane_grid` split horizontally left/center/right; `Task` from `PaneGrid::on_resize` → `PanelLayout` widths | `ui/panels/*`, `ui/render/mod.rs`, `ui/panel_layout.rs` | LOW — geometry solver carried over; see `A7`, `TASK-012` |
| A2 | `TopBottomPanel` (4): `panels/{top,bottom,left,right}.rs` each 1 | Full-width `row![]` bands inside the root `column![]`; the *conditional* side-panel branch (`SidePanel` vs `TopBottomPanel` for left/right/bottom) becomes pane-visibility + `pane_grid` placement | `ui/panels/top.rs`, `ui/panels/bottom.rs`, `ui/panels/left.rs`, `ui/panels/right.rs` | LOW |
| A3 | `SidePanel` (3): `panels/{left,bottom,right}.rs` each 1 | `pane_grid` pane + show/hide gate (`should_show_panel(has_toc, has_selected_file)` in `panels/right.rs`) | same as A2 | LOW |
| A4 | `ScrollArea` (24, incl. tests): shipping sites `about_dialog.rs`(2), `agent_debug_window.rs`(2), `background_logs.rs`(2), `editor_egui.rs`(1), `modals.rs`(1), `tools_dialog.rs`(3), `panels/center.rs`(3), `panels/left.rs`(4), `panels/right.rs`(1), `table_width/mod.rs` via `ScrollArea` sizing inputs, `tree/flatten.rs`(1), `render/table/{cell.rs,configured.rs}` | `iced::widget::scrollable` (vertical/horizontal), `scrollable::Properties` where autoscroll is needed (log tails) | see list | MEDIUM — autoscroll-to-bottom behavior for log/chat panes needs `scrollable::Id` + programmatic scroll offset; covered by `TASK-016` |
| A5 | `TextEdit::multiline` (21 incl. single-line `TextEdit`): `editor_egui.rs`, `agent_debug_window.rs`, search boxes (`panels/left.rs`, `tree_search.rs`), prompt fields, modal text inputs | `iced::widget::text_editor` (multi-line; DEC-007 with custom history), `iced::widget::text_input` (single-line) | `ui/editor_egui.rs`, `ui/text_buffer.rs`, `ui/panels/left.rs`, `ui/tree_search.rs`, `ui/modals.rs` | MEDIUM — DEC-007 undo/redo replays `Action::Edit` (NO native undo/redo in iced 0.14, verified) |
| A6 | `context_menu` (11): `panels/center.rs`(1), `tree/render.rs`(8), `render_tests.rs`(2 test) | `iced::widget::MouseArea::on_right_press` → overlay menu widget — built-in **`iced::widget::overlay::menu`** (dropdown with `State`/`Style`); on `Stack`/`overlay` for blocking dialogs (DEC-005; `iced_aw` NOT adopted) | `ui/tree/*`, `ui/panels/center.rs` tab strip, `ui/selection.rs` | MEDIUM — submenu positioning at cursor requires the overlay-anchored pattern from `TASK-017` |
| A7 | `egui::Table`/`TableBuilder` (2): `tools_dialog.rs`(2) — the only real `egui_extras` consumer; `egui::Table` grid in `background_logs.rs` | `iced::widget::table` | `ui/tools_dialog.rs`, `ui/background_logs.rs`, markdown tables in `ui/render/**` via the iced table-width adapter `ui_iced/table_width.rs` (`GUD-003`, `PAT-010`) | LOW — column-striping/frozen columns not needed by any `UI-xxx` row |
| A8 | Closeable/floatable `Window` (3): `modals.rs`, `dialogs.rs`, `about_dialog.rs`/`agent_debug_window.rs` applets | `iced::widget::overlay` + `Stack` shell (modal: blocking; decorator window: closable layer) | `ui/modals.rs`, `ui/dialogs.rs`, `ui/about_dialog.rs`, `ui/agent_debug_window.rs` | LOW — `TASK-017` |
| A9 | `CollapsingHeader` (1): `agent_debug_window.rs:170` | `iced::widget::toggler`-driven branch + `lazy` block, or `widget::container` with a disclose-arrow `Row` | `ui/agent_debug_window.rs` | LOW — single site |
| A10 | Tooltip helpers (`widgets.rs` tooltip(14) e.g. `on_hover_text`) | `iced::widget::tooltip` | `ui/widgets.rs`, `ui/tools_dialog.rs` (warnings, `UI-057`), `ui/about_dialog.rs` | LOW |
| A11 | Theme tokens (`Palette`, 67 refs in `ui/theme.rs`) | `iced::Theme` built-in + custom stylesheet closures mapped from `crate::ui::theme::Palette` (values identical, `Color32`→`iced::core::Color`, `GUD-003`) | `ui/theme.rs` → `ui_iced/theme.rs` | LOW |
| A12 | `ui/logo.rs` raw pixel drawing + `Image::` (Logo, 41 refs) | `iced::widget::svg` for vector logo; `iced::widget::image` for raster fallback; window icon via `window::Settings{icon}`/`iced::window::icon::Icon::from_rgba` | `ui/logo.rs`, `main.rs:40-57` icon wiring | LOW |
| A13 | Frame-driven app loop (`eframe::run_native`, `App::update`) | `iced::application(boot, update, view).run()` — Elm architecture; reactive rendering by default since 0.14 | `main.rs:140`, `ui/app/init.rs`:384 lines, `ui/app/update.rs`, `ui/app/render.rs` | LOW — bootstrap differs structurally; implemented by `TASK-022` |
| A14 | `rfd` native dialogs (`command_executor.rs:237`, `ui/tree/render.rs:206`) | unchanged (egui-independent) | `src/app/command_executor.rs`, `src/app/ui/tree/render.rs` | NONE — carry-over |

## Part B — Requirements traceability matrix (`TASK-002`)

One row per `UI-001`..`UI-066` from [`src/app/ui/SPEC.md`](../../src/app/ui/SPEC.md).
Rows MUST NOT be created for requirements covered by non-UI subsystems — subsystem co-owners
(tab manager, selection manager, background, agent) are noted because their *behavioral surface*
flows through UI widgets, but the matrix never documents subsystem internals.
The **Tier** column is the REQ-010 four-tier test citation (1=unit sidecar, 2=`Simulator`,
3=snapshot, 4=`.ice` flow), completed by `TASK-028`; `—` means "not yet assigned".

| Req | Requirement (abridged) | iced 0.14 widgets implementing it (contract) | Co-owned subsystem note | Tier |
|-----|------------------------|-----------------------------------------------|-------------------------|------|
| UI-001 | Pane structure: Left(tree+tags), Central(doc), Right(TOC), Bottom(prompt); top/bottom full-width bands, middle split left/center/right | `pane_grid` (middle) inside `column![top, middle, bottom]`; `row![]` bands | `tab_manager.rs` supplies center content | — |
| UI-002 | Dark scheme + FastMD brand palette, host-theme independent | `Theme::custom` + `ui_iced/theme.rs` stylesheet closures from `Palette` (`GUD-003`); no OS theme listener | config `theme` arrival (`DEC-009`) | — |
| UI-003 | UI thread never blocked during I/O/crawl | Elm architecture: all I/O via `Task::perform`/`Subscription`/bus workers; no sync work in `view` | `background/` worker pool | — |
| UI-004 | Click file → new tab | `MouseArea` (single press) + `Button`/`hover` on tree rows → `Message::OpenFile` | `tab_manager.rs` | — |
| UI-005 | Double-click file → system default editor | `MouseArea::on_press`-counted double-click (or `iced::widget::button` + press timing) → shell open | selection_manager | — |
| UI-006 | Right-click file/folder → context menu | `MouseArea::on_right_press` → `overlay::menu` anchored at cursor (`DEC-005`) | selection_manager | — |
| UI-007 | [Edit] on file → default editor (or inline, UI-036) | `overlay::menu` item → same open path as UI-005 | selection_manager | — |
| UI-008 | [Delete] → recycle bin | `overlay::menu` item → action; no egui confirm helper | `recycle_bin` util | — |
| UI-009 | [Show in File Explorer] | `overlay::menu` item → shell verb | — | — |
| UI-010 | [Move] → modal folder chooser with Ok/Cancel | `overlay`/`Stack` blocking modal + `scrollable` folder list + `Button` pair | selection_manager | — |
| UI-011 | [Create Directory…] modal, cursor-adjacent, unique-name | modal overlay anchored to cursor; `text_input` + `Button` pair | workspace layer creates dir | — |
| UI-012 | [Rename] modal, current name prefilled, Enter commits | modal + `text_input` (`on_submit` → confirm) | — | — |
| UI-013 | [Copy path] full path to clipboard | `overlay::menu` item → `iced::clipboard::write` Task | — | — |
| UI-014 | [Print] on file, Windows print verb | `overlay::menu` item → ShellExecute print (egui-free carry-over) | command_executor | — |
| UI-015 | [New document] modal; yaml header; unique-name fallback with timestamp | modal + `text_input` | workspace layer creates doc | — |
| UI-016 | Left column auto-fit ≤ 20% width, re-evaluated per directory | `pane_grid::PaneState` sizing driven by `TASK-012` measurements; constants from `panel_layout.rs` (`LEFT_PANEL_MAX_WIDTH_FRACTION` etc., `DEC-004`) — new measurement solution required | `panel_layout.rs` | — |
| UI-017 | Directories before files on every level | pure `tree/flatten.rs` ordering (carried over) rendered with `column!`/`mouse_area` rows | — | — |
| UI-018 | Hide folders with no markdown files | pure iterate/filter (carried over) → same rows as UI-017 | workspace crawl | — |
| UI-019 | [Format Markdown] quick task | `overlay::menu` item → format action | `markdown/` formatter | — |
| UI-020 | [Run as prompt] on file | `overlay::menu` item → agent prompt exec | agent batch executor | — |
| UI-021 | Tag filter hides non-matching directories | same tree rows; filter state in `PersistedUiState`-adjacent model | tag subsystem | — |
| UI-022 | Shift held → multi-select documents | `Modifers`/keyboard `Subscription` + selection state in tree rows | selection_manager | — |
| UI-023 | Right-click on multi-selection → multi-select context menu | `MouseArea::on_right_press` → `overlay::menu` (multi variant) | selection_manager | — |
| UI-024 | [Merge] runs consolidate LLM prompt | `overlay::menu` item → agent merge prompt | agent subsystem | — |
| UI-025 | [Delete] (multi) → recycle all | `overlay::menu` item → batch recycle | selection_manager, recycle_bin | — |
| UI-026 | Right-click document tab → tab context menu | tab strip `Row` of named `button`/`mouse_area` + right-press menu (`DEC-005`) | `tab_manager.rs` | — |
| UI-027 | [Close] tab w/ unsaved confirmation | `overlay::menu` item; confirmation via modal overlay | tab_manager | — |
| UI-028 | [Close Others] | same tab menu | tab_manager | — |
| UI-029 | [Close All] w/ unsaved confirmation | same tab menu + modal | tab_manager | — |
| UI-030 | [Copy Path] (tab) | same menu → `clipboard::write` | — | — |
| UI-031 | [Show in File Explorer] (tab) | same menu → shell verb | — | — |
| UI-032 | [Open in Editor] (tab) | same menu → system editor | — | — |
| UI-033 | [Format Markdown] (tab) | same menu → format action | markdown formatter | — |
| UI-034 | Tab-menu items also available on tree files (consistency) | shared menu-definition function reused by both menus (`GUD-001`) | tab_manager, selection_manager | — |
| UI-035 | `inline_editor_enabled` config gate (default false) | config arrival (`DEC-009`); `Message::Edit(paragraph)` gating | config subsystem | — |
| UI-036 | When enabled, [Edit] opens inline editor instead | same menu; variant branch | config subsystem | — |
| UI-037 | Editor shows raw Markdown body only; front-matter untouched | `iced::widget::text_editor` content split in `ui_iced/editor.rs` | `markdown/` parser | — |
| UI-038 | Editor modal: monospace area, line/col status bar, [Save]/[Cancel] | modal overlay + `text_editor` (mono font) + `Row` status `text` + `Button` pair | — | — |
| UI-039 | Selection: drag, word/line/section, Shift+arrows | `text_editor` built-in selection (verified in 0.14 `Action` set) | — | — |
| UI-040 | Copy/Cut/Paste via keyboard + context menu | `text_editor` built-ins + menu items → `clipboard` Task | — | — |
| UI-041 | Cursor navigation by char/word/line/Home/End/Ctrl-Home/End | `text_editor` built-ins | — | — |
| UI-042 | Undo/Redo ≥ 100 entries | **DEC-007**: no native undo in 0.14; custom history stack intercepting `Action::Edit` via `on_edit`, replayed by `Message::Undo`/`Redo`; depth constant ≥ 100 | — | — |
| UI-043 | GFM validation before save; abort + parse-error message | existing pulldown-cmark validation carried over; error `text` in modal | `markdown/parser.rs` | — |
| UI-044 | Save writes body + original front-matter; watcher hot-reloads | save via workspace API; reload via existing watcher → file event | `workspace/watcher` | — |
| UI-045 | [Cancel] discards changes | close modal without write | — | — |
| UI-046 | (Dropped) inverted editor color scheme | NOT implemented — dropped from scope by `DEC-002` (SPEC still owns the text; flagged, not edited) | — | n/a |
| UI-047 | Background Processes tab: auto-open on first task; [View] menu reopens/focuses | tab entry + iced `widget::table` for log rows + `scrollable` tail (`TASK-016`) | `background/` log buffer (REQ-460..465) | — |
| UI-048 | Thinking sections collapsible | `toggler`-driven collapsible block (same disclosure pattern as A9) with the reasoning `markdown` content | agent response_formatter (AGENT-022) | — |
| UI-049 | Center panel = tabbed documents; click opens; middle-click/close closes | tab strip `Row`, `MouseArea::on_middle_press`, close `button` | tab_manager | — |
| UI-051a | Tab auto-closes when its file is deleted (in-app or via watcher); selects last tab or empty state | file-removed event (Subscription/bus) closes tab | `workspace/watcher`, tab_manager | — |
| UI-051b | Tools… toolbar button opens Tools dialog as centered modal | `Button` → modal `overlay` (blocking) | agent tool registry (AGENT-024) | — |
| UI-052 | Tools row layout: checkbox, name, kind, char count, parallel-safe chip, Authenticate button | `iced::widget::table` row: `toggler`+`text`×3+`button` (chip as styled `container`) | tool registry (TOOL-015/016/020, MCP-020) | — |
| UI-053 | Toggle persists config immediately | `toggler` toggle → `Message::ToolToggle` → config write (async `Task`, non-blocking) | config subsystem (TOOL-017/018) | — |
| UI-054 | Authenticate runs background; disabled state; status line; failure keeps dialog | `button` w/ disabled state via `Style`, busy text "Authenticating…" | MCP client auth (MCP-020..028) | — |
| UI-055 | Char counts recomputed on open/tool-set change | model update path; display via `text` | TOOL-015/016 | — |
| UI-056 | Close action (button + Esc) hides dialog | `Button` + `keyboard::on_key_press` Esc; modal close | — | — |
| UI-057 | Error indicator with tooltip | `text` (warn glyph) inside `iced::widget::tooltip` | tool registry last_error | — |
| UI-058 | Parallel-safe chip | styled `container` chip beside count | TOOL-020 | — |
| UI-059 | Auth failure status persists until next success | row status `text` bound to error state | MCP auth | — |
| UI-060 | "Restart" action clears error and retries | `button` in error row | MCP auth | — |
| UI-061 | Hamburger button far right; dropdown with submenus | `iced::widget::icon`/`button` + `overlay::menu` nested dropdown (DEC-005) | — | — |
| UI-062 | "Windows" submenu with toggles | `overlay::menu` submenu | — | — |
| UI-063 | "Background operations" + "Agent debug" toggles | same submenu → window-visibility state | agent debug window state | — |
| UI-064 | "Chat models" submenu; `<name> (Cost: <cost>)`; placeholder when empty | `overlay::menu` submenu fed from config models | config chat models | — |
| UI-065 | Checkmark on active model; click selects + persists | same submenu; `config` persist Task | config subsystem | — |
| UI-066 | "Table wrap algorithm" submenu with checkmark; persists choice | same submenu; strategy state → `ui_iced/table_width.rs` | `markdown/table_width` solver | — |

### Notes & flags

- **SPEC anomaly (flagged per [RUST-041], not edited):** `UI-051` is defined twice in
  `src/app/ui/SPEC.md` — *tab auto-close on file deletion* (line 66, tab-behavior section) and
  *Tools-dialog trigger* (line 97, Tools-dialog section). The matrix disambiguates them as
  `UI-051a` / `UI-051b`. Renumbering requires a SPEC edit and is left for explicit user instruction.
- `UI-046` remains in the SPEC but is out of migration scope (`DEC-002`); the iced editor keeps the
  standard palette colors (`GUD-003`).
- `UI-042`'s "minimum 100 history entries" fixes the DEC-007 history-stack depth constant at ≥ 100.
- Rows marked `—` in **Tier** receive their four-tier citations when `TASK-028` completes REQ-010;
  every row MUST cite ≥ 1 tier artifact at that point or spawn new gaps for `TASK-029`-adjacent work.

## Phase 2 success criteria (TASK-005)

Verified 2026-10-17 against the local crate sources in the cargo registry checkout
`C:\Users\mkuhn\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\` (`iced-0.14.0` plus its
resolved `iced_widget-0.14.2` dependency). All citations are `file:line` within that checkout.

### Verified `iced::application(boot, update, view).run()` signature

- Constructor: `pub fn application<State, Message, Theme, Renderer>(boot: impl BootFn<State,
  Message>, update: impl UpdateFn<State, Message>, view: impl for<'a> ViewFn<'a, State, Message,
  Theme, Renderer>) -> Application<impl Program<State = State, Message = Message, Theme = Theme>>`
  — `iced-0.14.0/src/application.rs:79-89` (bounds: `State: 'static`, `Message: Send + 'static`,
  `Theme: theme::Base`, `Renderer: program::Renderer`).
- **BootFn**: `trait BootFn<State, Message> { fn boot(&self) -> (State, Task<Message>); }` —
  `application.rs:535-538`; blanket-impl'd for `T: Fn() -> C where C: IntoBoot<State, Message>` —
  `application.rs:540-548`. **IntoBoot** is implemented for the bare `State` (wrapped with
  `Task::none()`) and for `(State, Task<Message>)` — `application.rs:551-566`, confirming DEC-009
  (boot may return an initial Task). `Program::boot` returns the tuple at `application.rs:124-126`.
- **UpdateFn**: `trait UpdateFn<State, Message> { fn update(&self, state: &mut State, message:
  Message) -> Task<Message>; }` — `application.rs:598-601`; blanket-impl'd for `T: Fn(&mut State,
  Message) -> C where C: Into<Task<Message>>` — `application.rs:609-617` (the unit type converts
  into `Task::none()`, so a no-op passthrough handler needs only a unit-returning closure).
- **ViewFn**: `trait ViewFn<'a, State, Message, Theme, Renderer> { fn view(&self, state: &'a
  State) -> Element<'a, Message, Theme, Renderer>; }` — `application.rs:623-626`; blanket-impl'd
  (HRTB) for `T: Fn(&'a State) -> W where W: Into<Element<'a, ...>>` — `application.rs:628-638`.
- Final call shape: `pub fn run(self) -> Result where Self: 'static, P::Message:
  message::MaybeDebug + message::MaybeClone` — `application.rs:185-190`; `shell::run(program)?` at
  `application.rs:214`. `iced::Result = Result<(), Error>` — `iced-0.14.0/src/lib.rs:679`.
- Builder methods of `Application<P>` (window/window_size/position/centered/title/theme/
  subscription/scale_factor/presets/..., per DEP-007) — `application.rs:184-457`.

### Logo widget existence check (TASK-004 verification duty)

`iced::widget::iced(text_size)` **exists** in 0.14 — no discrepancy, no stub fallback needed:
"Creates an [`Element`] that displays the iced logo with the given `text_size`." —
`iced_widget-0.14.2/src/helpers.rs:1848-1894` (takes `impl Into<core::Pixels>`). Re-exported
through `iced_widget-0.14.2/src/lib.rs:48` (`pub use helpers::*;`) and the `iced::widget`
 façade `iced-0.14.0/src/lib.rs:634-637`. Feature gates required by the where-clauses
(`Renderer: core::Renderer + core::text::Renderer<Font = core::Font>`,
`Theme: text::Catalog + container::Catalog`) are satisfied by the default `iced::Theme` /
`iced::Renderer` built from the `wgpu`/`tiny-skia`/`fira-sans` features (`Cargo.toml` TASK-003).

### Dependency/feature verification (TASK-003)

All four requested features exist in `iced-0.14.0/Cargo.toml`: `wgpu` (:116), `tiny-skia` (:107),
`fira-sans` (:74), `advanced-shaping` (:53). The dependency is declared REGULAR (non-optional,
DEC-011) in the root `Cargo.toml`; no Cargo feature may gate it.

### Lockfile resolution note (found & fixed during the TASK-003 gate, 2026-10-17)

Adding iced exposed a **`windows` crate version split inside the iced/wgpu dependency tail**, not
an iced-version mismatch (all iced crates resolve at 0.14.x: `iced`/`iced_core`/`iced_futures`/
`iced_graphics`/`iced_program`/`iced_renderer`/`iced_runtime`/`iced_wgpu` = **0.14.0**,
`iced_tiny_skia`/`iced_winit` = **0.14.1**, `iced_widget` = **0.14.2**):

- `iced → iced_wgpu 0.14 → cryoglyph 0.1 → wgpu 27.0.1 → wgpu-hal 27.0.4` requires `windows 0.58`
  (`wgpu-hal-27.0.4/Cargo.toml`, `[target."cfg(windows)".dependencies.windows] version = "0.58"`).
- `wgpu-hal 27.0.4 → gpu-allocator 0.27.0`, which declares a *wide* range
  `windows = ">=0.53,<=0.58"` (`gpu-allocator-0.27.0/Cargo.toml:135`), so it can unify with 0.58.
- The pre-existing `trash = 5.2.6` dependency pinned a `windows 0.56.0` vertex into the
  lockfile's target-union (trash's own `windows` dep is `cfg(windows)`-gated while our
  edge is `cfg(not(windows))` — a combination that never compiles on any real target), and the
  stale lock resolved gpu-allocator **down** to that 0.56 copy → type-mismatch compile errors in
  `wgpu-hal/src/dx12/suballocation.rs` (E0308/E0277, two incompatible `windows-core` majors).
- **Fix (lock-only, no manifest/API change):** `cargo update -p trash` → `trash 5.2.9`
  (patch bump; `trash = "5.2.6"` in `Cargo.toml` means `^5.2.6` and already allowed it). 5.2.9
  requires `windows ^0.62`, which deletes the `windows|windows-core 0.56.0` vertices from the
  graph, after which gpu-allocator 0.27.0 unifies on `windows 0.58.0` — exactly what wgpu-hal 27
  needs. Workspace `windows 0.62` edge and the egui/eframe stack (wgpu 30 / gpu-allocator 0.28)
  are unaffected.


### Success criteria (window run — executed later by the user/orchestrator, not in the implementation session)

1. `cargo run --bin fastmd_iced` launches a native OS window (wgpu path initializes without panic).
2. The window shows the built-in `iced::widget::iced(48.0)` logo widget and the label
   text `fastmd iced pilot`, vertically centered (`fork: src/app/bin/fastmd_iced.rs`).
3. Closing the window exits the process with exit code 0 (`iced::application(...).run()` returns
   `Ok(())`; the no-op passthrough `update` and empty-state `boot` produce no error path).


### TASK-006/007/008 verification log (Phase 2 window bootstrap, fonts, theme — 2026-10-17)

All APIs verified against the local iced 0.14 registry sources before use
(`C:\Users\mkuhn\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f`):

- **Window bootstrap (TASK-006)**
  - `.window(window::Settings)` — `iced-0.14.0/src/application.rs:250-255`;
    `Settings{ size: Size, icon: Option<Icon>, exit_on_close_request: bool }`
    — `iced_core-0.14.0/src/window/settings.rs:34-101` (`size`:35, `icon`:87,
    `exit_on_close_request`:92-100, default `true` at :121). The egui 1000x700
    viewport (`main.rs:49`) is ported as `Settings.size`; default 1024x768
    unused.
  - `.title(...)` — `iced-0.14.0/src/application.rs:346-360`; `TitleFn` accepts
    `&'static str` directly (:579-583), so `strings::WINDOW_TITLE` ("FastMD AI",
    DEC-010) is passed verbatim.
  - `iced::window::icon::from_rgba(rgba, width, height) -> Result<Icon, Error>`
    — `iced_core-0.14.0/src/window/icon.rs:7-11`, re-exported via
    `iced-0.14.0/src/window/icon.rs:1-2`. The iced `image` feature
    (`from_file_data`) is NOT enabled in our dependency line, so the icon is
    decoded through the new shared loader `fastmd::utils::app_icon::
    decode_app_icon_rgba()` (same `assets/icon-32.png` the egui path embeds;
    `ui::logo::load_app_icon` now delegates to it — ONE decode function,
    [RUST-023]).
  - **Close-requests simplification (deviation-free, DEC-006 groundwork):**
    with `exit_on_close_request: false` the runtime takes NO action on a close
    request. `iced::window::close_requests() -> Subscription<Id>` — verified
    `iced_runtime-0.14.0/src/window.rs:259-268` (it filters
    `Event::CloseRequested` and yields the requesting window `Id` directly;
    there is no payload-bearing `CloseRequested` message type to map) — so the
    pilot maps `Id -> Message::CloseRequested(Id)` with
    `Subscription::map` (`iced_futures-0.14.0/src/subscription.rs:282`) and
    `update` issues `iced::window::close(id) -> Task<T>`
    (`iced_runtime-0.14.0/src/window.rs:283-286`). TASK-009 extends `update`
    with the persistence save BEFORE the `close(id)` call.
- **Fonts (TASK-007)**
  - `iced::font::load(bytes) -> Task<Result<(), Error>>` — verified
    `iced_runtime-0.14.0/src/font.rs:11`, re-exported as `iced::font::load`
    through `iced-0.14.0/src/lib.rs:576-580`. `Error` is a variant-less enum
    (`font.rs:7-8`): Debug-only, no `Display` — error descriptions are
    formatted with `{error:?}`.
  - `Task::perform` takes a FUTURE in 0.14 (`iced_runtime-0.14.0/src/task.rs:
    48-56`), NOT a `Task`; the per-collection outcome mapping uses
    `Task::map` (`task.rs:111-117`) and the loads are combined with
    `Task::batch` (`task.rs:88-109`; the batch streams ONE output per child
    task, so `boot` maps each outcome into `Message::FontLoadFinished(
    Option<String>)` — `Some(description)` = failed collection ([NFR-001]
    ERROR log in `update`), `None` = loaded.
  - `boot` returns `(PilotState, Task<Message>)` — the `(State, Task)` boot
    contract (`iced-0.14.0/src/application.rs:551-566`, DEC-009).
  - Byte inventory stays SINGLE-SOURCED with the egui path:
    `crate::ui::fonts::PHOSPHOR_REGULAR_FONT` (made `pub`) +
    `load_fallback_font()`/`candidate_fallback_font_paths()` (already
    egui-free). `ui_iced::fonts::font_collections()` registers Phosphor
    ("phosphor", mirroring the egui registration key) then the platform
    fallback; per-widget selection (`Font::MONOSPACE` etc.) is Phase 3.
  - `iced::window::Id` is `Copy` — `iced_core-0.14.0/src/window/id.rs:6` — so
    `Message: Clone` holds.
- **Theme (TASK-008)**
  - `iced::Theme::custom(name, palette)` / `custom_with_fn` — verified
    `iced_core-0.14.0/src/theme.rs:91-106`; `iced::theme::palette::Palette`
    fields (`background/text/primary/success/warning/danger`) —
    `iced_core-0.14.0/src/theme/palette.rs:7-22`; `Color::from_rgba8(r, g, b,
    a: f32)` — `iced_core-0.14.0/src/color.rs:89` (alpha is f32, NOT u8).
  - **egui `Color32` is stored PREMULTIPLIED** (`ecolor-0.36.1/src/color32.rs:
    31`; `r()`/`g()`/`b()` return premultiplied bytes). `ui_iced::theme::
    to_iced_color` therefore converts through `to_srgba_unmultiplied`
    (`ecolor-0.36.1/src/color32.rs:273-286`) — identity for opaque colors,
    which every `crate::ui::theme::Palette` token is, so the mapping
    reproduces the egui values bit-exactly ([GUD-003]; caught by the new
    sidecar test `test_to_iced_color_includes_alpha`, which failed against the
    naive `r()`-based conversion and drove the fix).
  - Role mapping: `background<-surface`, `text<-text`, `primary<-accent`,
    `success<-success`, `warning<-warning`, `danger<-error`. No numeric color
    literal exists in `ui_iced/theme.rs`. **Where it takes effect:** wired into
    the pilot via `.theme(|_state| ui_iced::theme::fastmd_dark_theme())`
    (`iced-0.14.0/src/application.rs:377-392`; `ThemeFn` accepts
    `Fn(&State) -> impl Into<Option<Theme>>`, :648-670).
- **Pre-existing gate breakage found & fixed (not caused by this chunk):**
  `ui::attributions_tests::completeness_against_cargo_manifests` was failing
  on HEAD since TASK-003 added `iced` to `Cargo.toml` without adding it to
  `ui::attributions::DIRECT_DEPENDENCIES`. Fixed by adding the iced entry
  (authors/repository from `iced-0.14.0/Cargo.toml`) and bumping the two
  hardcoded count assertions 56 -> 57
  (`attributions_tests.rs:9`, `about_dialog_tests.rs:200,218`).
- Gate results: `cargo check --workspace` PASS; `cargo clippy -p fastmd
  --bins --lib -- -D warnings` PASS; `cargo nextest run -p fastmd` 1212 run /
  1212 passed / 1 skipped PASS; `cargo fmt --check` PASS; `cargo doc --no-deps
  -p fastmd` PASS. [UNVERIFIED] The runtime behaviors (window opens with
  title/icon, close button exits, fonts render) are NOT machine-verified in
  this session — success criteria 1-3 above still need the user/orchestrator
  run of `cargo run --bin fastmd_iced`.

### Suspicious / for orchestrator attention (TASK-006..008)

1. Pre-existing rustdoc warning (unrelated, `fastmd-agent`): unresolved link
   `super::ToolRegistry::refresh_state` at
   `src/agent/tools/registry/groups.rs:58` (introduced by commits #101/#104,
   long before the migration). `cargo doc -p fastmd` is clean; the warning
   only appears with `--workspace`.
2. `strings.rs` was found with a pre-existing uncommitted formatting-only
   drift in `PDF_BACKED_ERROR` relative to HEAD; `cargo fmt` normalized it
   back to the HEAD shape (identical string value, no semantic change).
3. The `iced` feature list does NOT include `image`, so iced's own
   `window::icon::from_file_data` shortcut is unavailable; the shared
   `image`-crate decode in `utils::app_icon` is the load-bearing path. If a
   future task wants iced's `from_file_data`, revisit the feature list.

### TASK-009/010 verification log (UI-state persistence + three-band layout — 2026-10-17)

Note: this chunk was resumed from an interrupted prior run whose files were
present but gate-RED; the fixes made in this pass are listed under "Found &
fixed" below.

**Egui save path as found (TASK-009 preamble):**
- Save: `FastMdApp::save` (`src/app/ui/app/mod.rs:250-269`) copies
  `layout.left/right_panel_width` into `persisted_ui_state`, collects
  `expanded_dirs` from the orchestrator selection, and writes the JSON via
  `storage.set_string("ppai_ui_state", json)` (key `mod.rs:44`).
- Restore: `ui/app/init.rs:162-189` — `get_string` → serde parse with
  default fallback → v0→v1 migration clears `font_size_scale` and bumps
  the version (`init.rs:181-189`); panel widths seed `PanelLayout`
  (`init.rs:191-197`).
- **FINDING:** the egui build NEVER writes `window_width/height/x/y` in
  `save()` — window geometry rides eframe's built-in `persistence`
  feature (`mod.rs:254-256` comment). The fields exist on
  `PersistedUiState` but are dead in the egui path. No schema bump was
  needed: the iced path populates the SAME existing fields explicitly via
  `apply_close_metrics` (DEC-006), since iced has no equivalent automatic
  geometry persistence.

**Verified APIs (iced 0.14 local registry sources):**
- `BootFn` accepts `(State, Task<Message>)` — `iced-0.14.0/src/application.rs:540-566` (`IntoBoot` tuple impl :562-566).
- `.scale_factor(f: impl Fn(&P::State) -> f32)` — `application.rs:410-424`.
- `iced::window::latest() -> Task<Option<Id>>` — `iced_runtime-0.14.0/src/window.rs:294`; `scale_factor(id) -> Task<f32>` :376; `size(id) -> Task<Size>` :338; `position(id) -> Task<Option<Point>>` :369; `close<T>(id) -> Task<T>` :284; `close_requests() -> Subscription<Id>` :260; `resize_events() -> Subscription<(Id, Size)>` :249.
- `Task::then` — `iced_runtime-0.14.0/src/task.rs:128`; `Task::chain` :159.
- `window::Settings`: `size` `iced_core-0.14.0/src/window/settings.rs:36`, `position` :45, `icon` :87, `exit_on_close_request` :100; `Position::{Default, Specific(Point)}` — `position.rs:5-11`.
- pane_grid (`iced_widget-0.14.2`): `PaneGrid::new(&'a State<T>, view)` — `pane_grid.rs:182-185` (IMMUTABLE borrow in 0.14); `on_resize(leeway, f)` :267-275 (grab area = `spacing + leeway`, :264-266); default `spacing: 0.0` :202; `ResizeEvent { split, ratio }` :1226-1237.
- `State::with_configuration` — `pane_grid/state.rs:50`; `Internal::from_configuration` :365-401 assigns ids recursing a→b→split; `split_node` allocates the new PANE before the SPLIT :191-201; `resize(split, ratio)` :308; `layout()` :93; `Node::splits()` stack-DFS yields the OUTER split before the nested one — `pane_grid/node.rs:61-76`.
- `Pane(pub(super) usize)` / `Split(pub(super) usize)` — `pane.rs:5` / `split.rs:5`: ids CANNOT be constructed externally; the pilot discovers them from the built state. For the band config (left | (center | right)): panes Left=0, Center=1, Right=2 (BTreeMap ascending, `state.rs:29`), inner split=3, outer split=4; `splits()` order [4, 3] → `left_split`=4, `right_split`=3 (pinned by `layout_tests.rs`).
- `Axis::Vertical` divides left/right — `pane_grid/axis.rs:48-71`.

**Design choices / deviations:**
- `UiStateStorage` trait (`ui_iced/persisted.rs`) MIRRORS the
  `ConfigStorageHandler` conventions (`Send + Sync`, `Result<PathBuf,
  String>`, Noop/InMemory/File impls — `config/config.rs:686-724`)
  instead of reusing it: `save_config(&AppConfig)` is AppConfig-specific
  with NO load method and is shared with the orchestrator
  (`orchestrator.rs:50`), so it cannot carry a keyed UI-state payload.
  Deviation from the task's literal `&dyn ConfigStorageHandler` signature,
  documented in the module header.
- `PersistedUiState` stays in `crate::ui::persisted` UNCHANGED (rule 1);
  iced consumers import it. `migrate_legacy_state` ports the v0→v1
  semantics verbatim (log level `warn` vs egui's `info` — cosmetic).
- Pilot storage wiring (rule 4): `FASTMD_ICED_UI_STATE_PATH` env →
  `FileUiStateStorage`, else `InMemoryUiStateStorage`; the pilot NEVER
  resolves the platform-default user location ([RUST-024]/[RUST-006]).
  `FileUiStateStorage::platform_default()` exists for TASK-022 behind a
  test-context panic shield.
- Close flow (DEC-006): `CloseRequested` → chained `window::size` +
  `window::position` report tasks → `CloseSnapshotCaptured` → sync
  `layout`→`persisted` (faithful port of egui `save()` `mod.rs:251-252`)
  + `apply_close_metrics` + `save_ui_state` → THEN `window::close(id)`.
- DEC-008: boot chains `latest()` → `scale_factor(id)` → `ScaleBaseline`;
  `.scale_factor(|s| s.composed_scale_factor())` composes OS baseline ×
  persisted multiplier on the UI thread only ([FMD-069]).
- TASK-010: `should_show_panel` reused via `pub use
  crate::ui::panels::right::should_show_panel` — single source of truth
  ([RUST-051]), no verbatim duplication.
- Resize reduction replays the CLAMPED ratio into `pane_grid::State::resize`
  so widget geometry matches the persisted width; degenerate viewports
  (20%/30% cap below the panel minimum) clamp to the MINIMUM so the side
  panels stay draggable.

**Found & fixed in this pass (prior-run gate breakage):**
1. `clamped_width` returned `desired` (500) instead of `min` (180) in the
   degenerate `max < min` case — fixed (min wins).
2. `test_left_resize_grow_from_existing_width` expected 750px for a 30%
   drag on a 2500px viewport — violates the 20% cap (500); expectation
   corrected to 500 / replay 0.2.
3. f32 exact equality in `test_compose_scale_factor_multiplies_baseline_and_multiplier`
   (`1.5 × 1.2 = 1.8000001`) — fixed with 1e-6 tolerance.
4. Pilot close path did not sync `layout`→`persisted` before saving (only
   `PaneResized` synced) — added the save-time sync mirroring egui `save()`.
5. `test_split_side_unknown_split_returns_none` probe collided with the
   band's inner split (`split_node` allocates pane before split → probe's
   FIRST split is `Split(3)`); now uses the probe's second split
   (`Split(5)`, outside the band's {3, 4}).
6. Removed leftover debug scaffolding test `zzz_temp_print_ids` (println
   probe from the interrupted run).
7. rustdoc: 6 broken/private intra-doc links in `layout.rs` fixed
   (`Axis::Vertical`/`PaneGrid` → `pane_grid::` paths; UI-016 constants →
   plain code spans since `ui::panel_layout` is `pub(crate)` with only
   `PanelLayout` re-exported, `ui/mod.rs:18,43`).
8. clippy `redundant_closure`: `Ok::<String, String>` in
   `InMemoryUiStateStorage::load_ui_state`; the `|| center_pane_element()`
   closure in `view()` is REQUIRED (the bare fn item forces the builder's
   `'a = 'static` → E0521 on `&_state.panes`) — kept with a documented
   `#[allow(clippy::redundant_closure)]`.

**Gate results (this pass):** `cargo check --workspace` PASS;
`cargo clippy -p fastmd --bins --lib -- -D warnings` PASS;
`cargo fmt --check` PASS; `cargo nextest run -p fastmd` 1266 run /
1266 passed / 1 skipped PASS; `cargo doc --no-deps -p fastmd` PASS.

**[UNVERIFIED] runtime items for the user/orchestrator run:**
- Restore-across-runs: run `cargo run --bin fastmd_iced` twice with
  `FASTMD_ICED_UI_STATE_PATH` pointed at a scratch file — second launch
  must open at the saved size/position.
- The actual window close AFTER the save (the returned `window::close`
  task needs a live runtime; only the save-before-close reduction is
  unit-tested).
- Scale-baseline delivery through the `latest()` → `scale_factor` chain
  on a real display (esp. >100% DPI), and the composed factor applying
  to the window.

### TASK-021 verification log (top-level iced `Message` enum - 2026-10-17)

**Deliverables:** `src/app/ui_iced/message.rs` + `message_tests.rs` sidecar
(12 tests), `src/app/ui_iced/mod.rs` (`pub mod message;` + module-doc
bullet). No pilot rewiring (that is TASK-022/023).

**Final `Message` shape** - one variant per logical channel, each channel a
small sub-enum so the top level stays flat (PAT-001).
`Message` itself: `#[derive(Debug, Clone, PartialEq)]` - NOT `Copy`, because
the `UserCommand` payload carries `String`/`PathBuf`/`Vec`.

| Channel | Payload | Derives | Grounding |
| --- | --- | --- | --- |
| `Window` | `WindowMessage`: `CloseRequested(Id)`, `CloseSnapshotCaptured(WindowSnapshot)`, `ScaleBaseline(f32)`, `Resized{id,size}` | `Debug Clone Copy PartialEq` | pilot `bin/fastmd_iced.rs:239-267`; DEC-006/DEC-008 |
| `Layout` | `LayoutMessage`: `PaneResized(pane_grid::ResizeEvent)`, `TreeAutoFit` | `Debug Clone Copy` + hand-written `PartialEq` | TASK-010; UI-016 / TASK-012 (`left.rs:139-186`) |
| `Command` | `UserCommand` (existing bus enum, **49** variants) | `Debug Clone` (as `UserCommand` derives) | `bus/events/user_command.rs`; FMD-068 |
| `Tree` | `TreeMessage`: `ContextMenuOpened{path}`, `ContextMenuClosed` | `Debug Clone PartialEq` | `ui/tree/handlers.rs:51-69`, `ui/tree/render.rs:368-427`; DEC-005 |
| `Tab` | `TabMessage`: `ContextMenuOpened{index}`, `ContextMenuClosed` | `Debug Clone Copy PartialEq` | `ui/panels/center.rs:208-315` |
| `Editor` | `EditorMessage`: `Action(text_editor::Action)`, `SaveRequested{path}`, `CancelRequested`, `Undo{path}`, `Redo{path}` | `Debug Clone PartialEq` | `ui/editor_egui.rs:173-190`; DEC-007 (UI-042/043/045) |
| `Agent` | `AgentMessage`: `InputChanged{text}`, `Submit{text}` | `Debug Clone PartialEq` | `ui/panels/bottom.rs:226-294`; `AgentPanelState::command_input` |
| `Search` | `SearchMessage`: `QueryChanged{text}`, `Submitted{query}`, `Cleared` | `Debug Clone PartialEq` | `ui/panels/left.rs:230-265`; `tree_search.rs:37-146` (UI-050) |
| `Logs` | `LogsMessage`: `SearchChanged{text}`, `CategoryFilterChanged(Option<LogCategory>)`, `AutoScrollToggled(bool)`, `Cleared` | `Debug Clone PartialEq` | `ui/background_logs.rs:70-123` (UI-047) |
| `Theme` | `ThemeMessage`: `ScaleChanged{multiplier}` | `Debug Clone Copy PartialEq` | DEC-008 + `PersistedUiState::font_size_scale` |

**Routing rule (what keeps the channels from overlapping):**

1. **User intent rides `Message::Command(UserCommand)`.** The egui panels
   already publish a `UserCommand` for every application-level action
   (select, expand, close tab, run agent, rename, toggle logs window,
   debug-window controls, ...), so the iced panels keep that vocabulary
   instead of inventing a parallel per-panel one. `impl From<UserCommand>
   for Message` wraps it.
2. **Panel channels carry view-local state transitions only** - the state
   egui kept implicitly in immediate-mode widget memory (which row's
   context menu is open, what the search box currently holds, the editor's
   undo request) plus the runtime reports the window layer hands us.

Consequence: `Tree` and `Tab` are thin *by design* (context-menu overlay
state only), because selection, expansion and closing are `UserCommand`s.

**Grounded vs minimal:**

- Grounded in shipped egui behaviour: `Window`, `Layout`, `Command`,
  `Editor`, `Search`, `Logs`, `Agent`, `Tab`, `Tree`.
- Minimal on purpose: `Theme`. UI-002 pins the dark scheme
  (`ui/theme.rs:190 ctx.set_theme(egui::Theme::Dark)`; mirrored by
  `ui_iced::theme::fastmd_dark_theme`) and the egui build exposes NO
  light/dark toggle, so no toggle variant was invented. The only
  runtime-mutable theme state is the DEC-008 user scale multiplier.

**Not modelled here (open item, flagged for TASK-019):** inbound
background -> UI notifications (`ConfigArrived`, `FileEvent`,
`AgentEvent`, `BackgroundEvent`). Those arrive over the subscription
bridge rather than from a widget, so TASK-019 will either extend
`Logs`/`Agent`/`Tree` with inbound variants or add a dedicated inbound
channel. Nothing was guessed for them now.

**Pilot fold-in map** (the pilot's private `Message` in
`src/app/bin/fastmd_iced.rs`):

| Pilot variant | New home |
| --- | --- |
| `CloseRequested(Id)` | `Message::Window(WindowMessage::CloseRequested(id))` |
| `CloseSnapshotCaptured{id,size,position}` | `Message::Window(WindowMessage::CloseSnapshotCaptured(WindowSnapshot{..}))` |
| `ScaleBaseline(f32)` | `Message::Window(WindowMessage::ScaleBaseline(s))` |
| `WindowResized(Size)` | `Message::Window(WindowMessage::Resized{id,size})` - carries the window id `resize_events()` delivers and the single-window pilot dropped (the pilot header already anticipates TASK-022 carrying it) |
| `PaneResized(ResizeEvent)` | `Message::Layout(LayoutMessage::PaneResized(ev))` |
| `FontLoadFinished(Option<String>)` | NOT a window concern - left in the pilot. When TASK-022 owns the font bootstrap it needs its own channel (e.g. `Bootstrap`/`Font`). Flagged. |

**Design decisions for orchestrator attention:**

1. `PartialEq` is derived on every channel and on `Message` (the plan asked
   only for `Debug, Clone[, Copy]`). Rationale: message equality is what a
   replay/dedup check needs and what the sidecar tests assert; every
   payload supports it.
2. `LayoutMessage::PartialEq` is hand-written because
   `pane_grid::ResizeEvent` derives only `Debug, Clone, Copy`
   (`iced_widget-0.14.2/src/pane_grid.rs:1226-1237`). The comparison is
   exact on `ratio` (bit equality of the runtime-produced `f32`); a
   tolerance would break transitivity.
3. `EditorMessage::SaveRequested`/`Undo`/`Redo` carry `path` so the target
   travels with the intent (DEC-007 per-tab history stacks + FMD-068).
   `CancelRequested` does not - it discards whichever buffer is open.
4. `EditorMessage::Action` carries `iced::widget::text_editor::Action`
   (`Debug, Clone, PartialEq`, not `Copy`) as the DEC-007 interception
   point; `Action::is_edit()` separates content changes from caret moves.
5. `Layout` is a channel NOT named in the plan's nine. It was added to give
   the pilot's `PaneResized` and TASK-012's `Message::AutoFitTree` an
   honest home without calling pane geometry a "window" concern.
6. `UserCommand` has **49** variants (the task brief said ~36); it derives
   `Clone, Debug, PartialEq` only, which is why `Message` is `Clone` and
   not `Copy`.

**Gate results (this pass):** `cargo check -q` PASS (clean);
`cargo clippy -p fastmd --bins --lib -- -D warnings` PASS (exit 0, no
warnings); `cargo nextest run -p fastmd message` PASS (25 run, incl. the 12
new `ui_iced::message` tests, 0 failed);
`cargo nextest run --status-level fail --show-progress none` PASS
(1279 run / 1279 passed / 1 skipped); `cargo doc --no-deps --quiet` PASS.

`cargo fmt --check`: the TASK-021 files are clean (`rustfmt --check
--edition 2024` on `message.rs` and `message_tests.rs` -> exit 0). The
workspace-level check FAILS on three files outside this task's scope
(`ui_iced/layout.rs`, `ui_iced/persisted.rs`, `ui_iced/persisted_tests.rs`)
whose working-tree diffs are pure import-ordering / call-wrap reversals
against HEAD with zero semantic content, and which this task never touched.
Left untouched deliberately (not this task's work); the orchestrator should
run `cargo fmt` - or `git checkout --` those three files - before
committing.
- Pane-grid drag feel: leeway grab, clamped-ratio replay, and the
  right-pane gate visually (pilot gate inputs are `false` stubs).

### TASK-011 verification log (top panel ported to iced - 2026-10-17)

**Deliverables:** `src/app/ui_iced/panels/mod.rs`,
`src/app/ui_iced/panels/top.rs` + `top_tests.rs` sidecar (36 tests),
`src/app/ui_iced/mod.rs` (`pub mod panels;` + module-doc bullet), and one
additive formatting helper in `src/app/ui/strings.rs`
(`format_menu_row_label`, plus its two private glyph consts). No pilot or
update-loop wiring (that is TASK-022/023).

**Port shape - pure view, no `FastMdApp` inside the widget tree.**

`top_bar(&TopBarViewModel) -> Element<'_, Message>` renders the bar; every
user action becomes `Message::Command(UserCommand)`. The panel never
publishes to `user_command_bus` - the TASK-022/023 reducer does, which is
what keeps intent replayable (FMD-068). Because iced keeps widget state in
the widget `Tree` (verified: `PickList::new(options, selected, on_select)`
takes NO state argument - `iced_widget-0.14.2/src/pick_list.rs:191-215`,
state created via `Widget::state()` at `pick_list.rs:762-780`), the view
needs no `&mut` state parameter and stays a pure function.

`TopBarViewModel` (built by `TopBarViewModel::from_app(&FastMdApp)`,
reading exactly the accessors the egui panel used):

| Field | Source (egui panel parity) |
| --- | --- |
| `indexing_finished: bool` | `file_processor().indexing_finished` |
| `file_count: usize` | `file_processor().all_files.len()` |
| `tag_options: Vec<TagFilterOption>` | `tags().all_tags()`, `All` entry first |
| `selected_tag: Option<String>` | `tags().selected_tag` |
| `chat_models: Vec<ChatModelEntry{name,cost}>` | `config().models` filtered on `has_use_case("chat")`, falling back to ALL models when none declare it, name-sorted (egui rule preserved verbatim) |
| `current_chat_model: Option<String>` | `config().current_chat_model_key()` |
| `current_strategy: DeficitStrategy` | `config().deficit_strategy()` |
| `show_background_logs: bool` | `orchestrator.background_manager.lock().unwrap().show_background_logs` |
| `show_agent_debug: bool` | `orchestrator.agent_panel_state.show_debug_window` |

`TagFilterOption { label, tag: Option<String> }` carries the command in
`tag`, not the label, so a tag literally spelled `"All"` can never collapse
into the "show everything" entry (pinned by a test).

**Pure helpers - reused, not duplicated ([RUST-051]).**
`compute_next_selected_file`, `get_tag_filter_text`, `strategy_label` and
all six `apply_*` command constructors are `pub use`-re-exported from
`crate::ui::panels::top` (they are framework-free; `layout.rs:64` sets
the same precedent for `should_show_panel`). Only
`build_indexing_status_text` needed re-typing: `egui::RichText` is replaced
by three pure functions - `indexing_status_text` (String),
`indexing_status_color` (palette token) and `indexing_status_font`
(italic-while-indexing) - so no egui type crosses into the iced layer.

**Nested submenus (UI-062/UI-064/UI-066) - flattened, with the hierarchy
kept in the model.**

Investigated `overlay::menu::Menu` / `State` before choosing
(`iced_widget-0.14.2/src/overlay/menu.rs`):

* `Menu::new(state, options, hovered_option, on_selected,
  on_option_hovered, class)` - `menu.rs:57-79`; `State` is a bare `Tree`
  wrapper (`menu.rs:144-156`) with no open/close query.
* Options render as **one plain `ToString::to_string` row each**
  (`menu.rs:557-576`). There are **no separators, no disabled rows, no
  rich content and no child menus** - `Menu` is single-level only.
* `Menu` never closes itself; the consumer must (only `PickList` wires
  open/close/click-away internally - `pick_list.rs:450-478`).
* DEC-005 rules out `iced_aw` (and `iced_aw` is not in `Cargo.lock`).

Resolution: **two-level model, one-level widget.**

1. `build_menu_entries(&vm) -> Vec<MenuEntry>` is the authoritative,
   fully-ordered UI-061..066 structure: `MenuEntry { label, section,
   kind, checked }` with `MenuSection::{Direct, Windows, ChatModels,
   TableWrap}` and `MenuEntryKind::{Action(UserCommand), SectionHeader,
   Placeholder}`.
2. `hamburger_items(&vm) -> Vec<HamburgerItem>` projects only the
   **selectable** rows (`HamburgerItem { label, command }`) and feeds one
   `pick_list`.

Why the non-selectable rows are filtered rather than rendered: a iced
dropdown must answer a click on *any* row with a `Message`, and the panel
is only allowed to emit real user intent - `Message` (TASK-021) has no
no-op variant and adding one is out of this task's scope. So section
headers and UI-064's `NO_CHAT_MODELS_CONFIGURED` placeholder live in the
model and are dropped by `HamburgerItem::from_entry` instead of firing a
meaningless command. **Consequence: the placeholder is currently modelled
but not visible in the rendered menu** - flagged below for the
orchestrator.

Submenu membership is carried inside the row label via
`strings::format_menu_row_label(submenu, label, checked)`:
`"<check slot> <submenu> › <label>"` (`›` = U+203A), with the submenu
segment omitted for top-level rows and the check slot always present so
checked/unchecked rows stay column-aligned. UI-064's
`<name> (Cost: <cost>)` format survives verbatim as the item segment.

**Menu item -> `UserCommand` mapping** (all rows, in menu order):

| Menu row | `UserCommand` | checked when |
| --- | --- | --- |
| `MENU_BATCH` | `OpenBatchDialog` | - |
| `MENU_TOOLS` | `OpenToolsDialog` | - |
| *Windows* (header) | none (not selectable) | - |
| `MENU_BACKGROUND_OPERATIONS` | `ToggleBackgroundLogs(!show_background_logs)` | window shown |
| `MENU_AGENT_DEBUG` | `ToggleAgentDebugWindow(!show_agent_debug)` | window shown |
| *Chat models* (header) | none (not selectable) | - |
| `NO_CHAT_MODELS_CONFIGURED` | none (placeholder, not selectable) | - |
| `<name> (Cost: <cost>)` | `SelectChatModel(name)` | `current_chat_model == name` |
| *Table wrap algorithm* (header) | none (not selectable) | - |
| each of 5 `strategy_label(v)` | `ChangeTableWidthStrategy(v)` | `v == current_strategy` |
| `MENU_ABOUT` | `OpenAboutDialog` | - |

Toggle rows carry the value they *switch to* (egui's `menu_check` returned
the post-click value), so the reducer receives an absolute target and stays
idempotent-friendly. Strategy order is pinned by `strategy_menu_order()` -
production default `HybridMinPenaltyWaterFill` first, then the two FTWA
strategies, then the three survey algorithms, matching the egui panel.

**Feature-gated widgets stubbed, and why.**

* **Logo** - `iced::widget::image` requires the `image` cargo feature and
  `iced::widget::svg` requires `svg`; **neither is enabled**
  (`Cargo.toml:55` adds iced with `wgpu`, `tiny-skia`, `fira-sans`,
  `advanced-shaping` only), and DEC-011 says no feature flags. The
  egui panel vector-paints `ui::logo::paint_logo`; the port renders the
  existing Phosphor bolt `strings::ICON_LIGHTNING` in
  `phosphor_font()` (`PHOSPHOR_FONT_NAME`, registered by TASK-007) as
  the "Document + Bolt" stand-in. No cargo feature was added.
* **Spinner** - iced 0.14 has **no spinner widget at all**: there is no
  `iced::widget::spin`, and a search of `iced_widget-0.14.2/src` finds no
  spinner implementation. An indeterminate indicator would need
  `progress_bar` driven by a timer subscription, i.e. runtime wiring
  outside a view-only task. The panel renders the status **text only**;
  "still indexing" stays legible through the italic font
  (`indexing_status_font`) and the `(...found N files...)` ellipsis.
* **Icon font inside `pick_list`** - `pick_list` passes ONE font to both
  the button and its menu (`pick_list.rs:704, 725`), so the Phosphor
  hamburger glyph cannot be applied to the button without also applying it
  to every menu row. The hamburger therefore uses the **default UI font**
  and relies on cosmic-text per-glyph fallback to resolve
  `HAMBURGER_MENU_BUTTON` (U+E2F0) from the registered Phosphor font.
  Same fallback assumption applies to the U+2713 check glyph.

**Gate results (this pass):** `cargo check -q` PASS (exit 0, clean);
`cargo clippy -p fastmd --bins --lib -- -D warnings` PASS (exit 0);
`cargo fmt` run, `cargo fmt --check` PASS workspace-wide (exit 0 - the
three files TASK-021 flagged as dirty are clean in this working tree);
`cargo nextest run --status-level fail --show-progress none` PASS
(**1315 run / 1315 passed / 1 skipped**, +36 over TASK-021's 1279);
`cargo doc --no-deps --quiet` PASS (exit 0, no warnings).

**Needs the orchestrator's eyes:**

1. **`strings.rs` was extended** (not in the task's deliverable list).
   [GUD-002] forbids literals in panels, so the row format had to live in
   `strings.rs`: `pub fn format_menu_row_label(...)` plus private consts
   `MENU_CHECK_GLYPH` (U+2713) and `MENU_SUBMENU_ARROW` (U+203A).
   Additive only; no existing string changed.
2. **UI-064's empty-models placeholder is modelled but not rendered.**
   As explained above, `pick_list` cannot show a non-selectable row. If
   the placeholder must be *visible*, the options are (a) render it as a
   selectable row that fires a command we do not have, (b) render it
   outside the menu, or (c) replace the `pick_list` with the custom
   overlay work scheduled for TASK-017. Recommend deciding at TASK-017.
3. **UI-062/UI-064/UI-066 nesting is flattened into label prefixes.**
   True nested popups are not available from iced 0.14 built-ins under
   DEC-005. If the reviewer wants real submenus, that is a DEC-005
   revisit (custom overlay in TASK-017), not a TASK-011 fix.
4. **Glyph rendering is unverified at runtime.** Both U+E2F0 (hamburger)
   and U+2713 (check) depend on cosmic-text falling back to the
   Phosphor / system symbol font registered by TASK-007. [UNVERIFIED]
   headlessly - needs one visual pass in the pilot once TASK-022 mounts
   the panel. If either tofus, the consts in `strings.rs` make the swap a
   one-line change.
5. **`apply_*` / pure helpers are re-exported, not re-implemented.** If
   the plan expects the egui panel to be deleted in Phase 5, these
   `pub use` paths must be re-pointed at the time of that deletion; the
   iced panel currently depends on `crate::ui::panels::top` remaining
   present.
6. **Tag-filter gating dropped deliberately.** The egui panel hides the
   combobox while indexing purely to keep egui widget ids stable across
   the `indexing_finished` flip (`ui/panels/top.rs:154-211`). iced has
   no such constraint, so the tag `pick_list` is always allocated and
   visible. No behaviour requirement is affected.

### TASK-017a overlay-infrastructure verification log (2026-10-17)

**Scope delivered: the reusable primitives only.** No individual dialogs
(about / batch / tools / agent-debug / attributions) were ported - those
stay a later chunk. New files: `src/app/ui_iced/overlay.rs` +
`overlay_tests.rs` sidecar (40 tests), `pub mod overlay;` + module-doc
bullet in `src/app/ui_iced/mod.rs`, and two additive colour-token items in
`src/app/ui_iced/theme.rs` (`SCRIM_ALPHA`, `scrim_color()`) so the scrim
introduces no new numeric literal ([GUD-003]).

**Verified iced 0.14 APIs (local sources).**

| API | Citation |
| --- | --- |
| `mouse_area(w)` helper | `iced_widget-0.14.2/src/helpers.rs:2010-2017` |
| `MouseArea` struct (all nine event slots) | `mouse_area.rs:14-33` |
| `MouseArea::on_right_press` | `mouse_area.rs:68-71` - publishes **and captures** at `mouse_area.rs:407-412` |
| `MouseArea::on_right_release` | `mouse_area.rs:75-78` |
| `MouseArea::on_move(impl Fn(Point) -> Message)` | `mouse_area.rs:113-116` - the only position-carrying slot; position is **widget-relative** (`cursor.position_in(layout.bounds())`, `mouse_area.rs:358-360`) |
| `MouseArea::interaction(Interaction)` | `mouse_area.rs:127-131` |
| `Stack` - layered draw, top-down event routing, early return on capture | `stack.rs:23-30` (contract), `stack.rs:249-275` (`update`; capture check `stack.rs:262-264`) |
| `Float::translate(impl Fn(Rectangle, Rectangle) -> Vector)` | `float.rs:53-59` - receives content bounds + viewport, returns a **delta**, not an absolute position |
| `Float` overlay activation condition | `float.rs:79-84` (`is_floating`), `float.rs:226-250` - floats **only** if `scale > 1.0 \|\| translation != Vector::ZERO` |
| `overlay::menu::Menu::new(state, options, hovered_option, on_selected, on_option_hovered, class)` | `overlay/menu.rs:57-79` |
| `menu::State` | `overlay/menu.rs:144-162` - a bare `Tree` wrapper, no open/close query |
| `Menu::overlay(position, viewport, target_height, menu_height)` | `overlay/menu.rs:126-140`; above/below flip `menu.rs:244-270` |
| `Menu` rows are plain `ToString`, no per-row handler, never self-closing | `overlay/menu.rs:557-576`, `menu.rs:401-487` |
| Runtime updates overlays **before** the base tree | `iced_runtime-0.14.0/src/user_interface.rs:198-254` |
| Base tree is **skipped** for any event the overlay `Captured` | `iced_runtime-0.14.0/src/user_interface.rs:310-316` |
| `container` default alignment is Left/Top; `align_x`/`align_y` | `container.rs:100-101`, `container.rs:188-203` |
| `Padding { top, right, bottom, left }` public fields | `iced_core-0.14.0/src/padding.rs:36-45` |
| `Button::on_press_maybe(Option<Message>)` | `button.rs:170-173` |
| `mouse::Interaction` variants (no `Blocked` in 0.14) | `iced_core-0.14.0/src/mouse/interaction.rs:4-33` |

**Architecture decision: `Stack` in the base tree, not the `overlay` layer.**
Both primitives mount as extra `Stack` layers over the caller's content
rather than as `overlay::Element`s.

1. A scrim needs the full area. `Float` positions by *delta from where the
   content would naturally sit* and only becomes an overlay when that delta
   is non-zero (`float.rs:226`). A full-viewport scrim sits at its natural
   origin, so its translation is `Vector::ZERO` and it would silently stop
   floating. `Stack` has no such condition.
2. `overlay::menu::Menu` is built *as* the overlay, so it demands
   caller-held `&mut menu::State` **and** `&mut Option<usize>`
   (`menu.rs:57-79`). That forces `&mut` into every view function, against
   the pure-view pattern the panels already establish
   (`panels/top.rs:544-553`). It also renders each row as one plain
   `ToString` row with no per-row handler and never closes itself. A
   `Column` of `button`s gives per-row `Message`s, native hover styling,
   keyboard focus, and needs no external state.
3. Event precedence is already correct: `Stack::update` walks children
   top-down and returns as soon as one captures the event
   (`stack.rs:262-264`), so the dialog card gets the click and the scrim
   only sees clicks that miss it.

**Primitive signatures.**

```rust
// Overlay state model - generic over the caller's modal vocabulary.
pub enum OverlaySlot<Kind> { Closed, ContextMenu { anchor: Point }, Modal(Kind) }
impl<Kind> OverlaySlot<Kind> {
    pub fn closed() -> Self;
    pub fn is_closed(&self) -> bool;
    pub fn is_context_menu(&self) -> bool;
    pub fn is_modal(&self) -> bool;
    pub fn anchor(&self) -> Option<Point>;
    pub fn modal_kind(&self) -> Option<&Kind>;
    pub fn open_context_menu(&mut self, anchor: Point) -> bool; // refused while Modal is up
    pub fn open_modal(&mut self, kind: Kind);
    pub fn close(&mut self) -> bool;  // true = something was open (Escape consumed)
}
impl<Kind> Default for OverlaySlot<Kind> { /* Closed, hand-written */ }

// Menu item + projections.
pub struct MenuItem<Message> { pub label: String, pub message: Message }
impl<Message> MenuItem<Message> {
    pub fn new(label: impl Into<String>, message: Message) -> Self;
    pub fn map<Other>(self, f: impl FnOnce(Message) -> Other) -> MenuItem<Other>;
}
impl<Message> Display for MenuItem<Message>;          // renders the label
pub fn menu_labels<Message>(items: &[MenuItem<Message>]) -> Vec<String>;
pub fn menu_index_of<Message: PartialEq>(items: &[MenuItem<Message>], m: &Message) -> Option<usize>;

// Pure geometry (deterministic, so clamping cannot drift from the layout).
pub fn menu_size(item_count: usize) -> Size;
pub fn clamp_anchor(anchor: Point, size: Size, viewport: Size) -> Point;

// Widgets.
pub fn context_menu_card<'a, M: Clone + 'a>(items: &'a [MenuItem<M>]) -> Element<'a, M>;
pub fn with_context_menu<'a, M: Clone + 'a>(
    base: Element<'a, M>, anchor: Point, items: &'a [MenuItem<M>],
    viewport: Size, on_close: M,
) -> Element<'a, M>;
pub fn modal<'a, M: Clone + 'a>(content: Element<'a, M>, on_dismiss: M) -> Element<'a, M>;
pub fn context_menu_trigger<'a, M: 'a>(
    content: impl Into<Element<'a, M>>,
    on_move: impl Fn(Point) -> M + 'a,
    on_right_press: M,
) -> MouseArea<'a, M>;
pub fn input_blocker<'a, M: 'a>(on_click: M) -> MouseArea<'a, M>;   // transparent
pub fn scrim_blocker<'a, M: 'a>(on_click: M) -> MouseArea<'a, M>;   // dimmed

// Confirm/cancel, re-typed from `ui/widgets.rs:178-194`.
pub enum ConfirmCancel { Confirm, Cancel }
impl ConfirmCancel { pub fn to_dialog_action(self) -> crate::ui::widgets::DialogAction; }
pub fn confirm_cancel_pair<M>(on: impl Fn(ConfirmCancel) -> M) -> (M, M);
pub fn confirm_press<M>(confirm_enabled: bool, message: M) -> Option<M>;
pub fn confirm_cancel_buttons<'a, M: Clone + 'a>(
    confirm_label: &'a str, confirm_enabled: bool,
    on: impl Fn(ConfirmCancel) -> M + 'a,
) -> Row<'a, M>;
```

Layout consts: `MENU_WIDTH = 200.0`, `MENU_ROW_HEIGHT = 24.0`,
`MENU_ROW_SPACING = 2.0`, `MENU_PADDING = 4.0`, `MENU_ANCHOR_GAP = 2.0`,
`MODAL_PADDING = 16.0`, `CARD_BORDER_WIDTH = 1.0`, `CARD_RADIUS = 4.0`,
`CONFIRM_CANCEL_SPACING = 8.0`.

**What positioning IS achievable with built-ins.**

* Cursor-anchored menus anywhere in the mount element's coordinate space
  (`with_context_menu`), via `container` padding on a full-area layer
  (`container.rs:188-203` + `Padding` fields, `padding.rs:36-45`).
* **Exact** viewport clamping. The menu card's size is deterministic
  (`menu_size`) because rows are uniform-height buttons, so
  `clamp_anchor` can run *before* layout and still match the real box.
  The lower bound wins when the viewport is smaller than the card, so the
  result is never negative.
* Blocking modals: full-area dimmed scrim + centred card. Input blocking is
  real, not cosmetic - `mouse_area` captures the press
  (`mouse_area.rs:376-379`) and the runtime then skips the base tree for
  that event (`user_interface.rs:310-316`).
* Click-outside dismissal without activating what is underneath, for the
  same reason.
* Modality: `open_context_menu` is refused while a `Modal` is open, so a
  menu can never open behind a blocking dialog.

**What is NOT achievable with built-ins (and the workaround).**

1. **The right-press point cannot travel with the message.**
   `MouseArea::on_right_press` takes a fixed `Message` and no closure
   (`mouse_area.rs:68-71`), unlike `on_move`. The anchor must therefore be
   **state the caller holds**: track the cursor with `on_move`, then read
   it back when the right-press message is handled.
   `context_menu_trigger` wires exactly that pair.
2. **`on_move` positions are widget-relative** (`mouse_area.rs:358-360`),
   not window-absolute. Mount the menu against the same element the trigger
   wrapped and no conversion is needed; for a window-absolute anchor, wrap
   the full-window root (its bounds *are* the viewport) or add the wrapped
   widget's known offset.
3. **No submenu nesting.** `Menu` is single-level and DEC-005 rules out
   `iced_aw` (also absent from `Cargo.lock`). Context menus use the same
   label-prefix flattening the hamburger menu already uses
   (`panels/top.rs:584-626`).
4. **No auto-flip above the anchor.** `Menu::overlay` does flip
   (`menu.rs:244-270`) but this design does not use `Menu`.
   `clamp_anchor` clamps rather than flips; a caller wanting flip-above
   compares `menu_size` against the space below the anchor itself.
5. **`Float` is not usable for full-area scrims** - see the
   zero-translation condition above. It remains the right tool for a
   *small* floating card whose delta from its natural position is
   guaranteed non-zero.
6. **`mouse::Interaction::Blocked` does not exist in 0.14**
   (`mouse/interaction.rs:4-33`); `Interaction::Hidden` is used on the
   blockers instead.

**How the tree / tabs / editor are expected to call it.**

Tree ([TASK-012]) and tabs ([TASK-013]) - the panel builds the item list,
the app root mounts it:

```rust
// per row / per tab - capture the anchor and ask for the menu:
context_menu_trigger(
    row,
    Message::CursorMoved,                                  // keeps app cursor state current
    Message::Tree(TreeMessage::ContextMenuOpened { path }),
)

// app root view - `viewport` is the window's logical size:
match &slot {
    OverlaySlot::ContextMenu { anchor } => with_context_menu(
        root, *anchor, &tree_menu_items, viewport,
        Message::Tree(TreeMessage::ContextMenuClosed),
    ),
    _ => root,
}
```

`TreeMessage::ContextMenuOpened/ContextMenuClosed` and
`TabMessage::ContextMenuOpened/ContextMenuClosed` already exist
(`ui_iced/message.rs:211-241`), so no `Message` variants were invented.
The reducer calls `slot.open_context_menu(tracked_cursor)` on open and
`slot.close()` on close.

Editor ([TASK-014]) - the blocking save/cancel dialog:

```rust
// app root view:
stack(vec![root, modal(dialog_card, Message::Editor(EditorMessage::CancelRequested))])

// dialog card footer - `buffer_is_valid` gates the confirm button:
confirm_cancel_buttons(strings::SAVE_BUTTON, buffer_is_valid,
    |outcome| Message::Editor(match outcome {
        ConfirmCancel::Confirm => EditorMessage::SaveRequested { path: path.clone() },
        ConfirmCancel::Cancel  => EditorMessage::CancelRequested,
    }))
```

Escape: the caller maps its Escape key to the same dismiss `Message` (or
calls `OverlaySlot::close()` directly and uses the returned `bool` to
decide whether the key was consumed).

**Open items for the orchestrator.**

1. **A cursor-tracking message is still missing from `Message`.**
   `context_menu_trigger` needs an `on_move` target that stores the cursor
   (the `Message::CursorMoved(Point)` used in the snippets above does not
   exist yet - TASK-021 has no such variant, and adding one is outside this
   task). Until it lands, the anchor can only come from a widget-relative
   position the panel already knows. Flagging rather than inventing a
   variant.
2. **Menu chrome is hand-styled, not native.** Because the menu is a
   `button` column rather than `overlay::menu::Menu`, its look comes from
   `card_style` (`surface_variant` + `border_strong` tokens) plus the
   default `button` theme, not from iced's menu style. If the visual pass
   wants the native menu appearance, that means switching to `Menu` and
   accepting caller-held `menu::State` + `hovered_option` in the view
   signature.
3. **`menu_size` is a contract, not a measurement.** It assumes every row
   renders at exactly `MENU_ROW_HEIGHT`. If a future menu item wraps to two
   lines, `clamp_anchor` will under-estimate and the card can overflow the
   bottom edge. Keep labels single-line.
4. **The scrim token is new.** `SCRIM_ALPHA = 0.55` has no egui
   predecessor (egui `Window`s do not dim the viewport). It is a deliberate
   design addition derived from `Palette::surface`, not a ported value -
   worth a visual confirmation.
5. **Workspace-wide `cargo clippy --all-targets -D warnings` does NOT pass**
   and did not pass before this task either: pre-existing lints fire in
   `ui/batch_dialog_tests.rs`, `ui_iced/fonts_tests.rs`,
   `ui_iced/persisted_tests.rs`, `markdown/link_scanner_tests.rs`,
   `config/config_tests.rs` and across `fastmd-agent`'s test targets. The
   gate specified for this task (`cargo clippy -p fastmd --bins --lib --
   -D warnings`) is clean.

### TASK-012 verification log (left file-tree panel ported to iced - 2026-10-18)

**Deliverables.** New `src/app/ui_iced/panels/left.rs` + `left_tests.rs`
sidecar (37 tests); `pub mod left;` + module-doc bullet in
`ui_iced/panels/mod.rs`; one new `LayoutMessage::CursorMoved` and four new
`TreeMessage` variants in `ui_iced/message.rs` (+3 message tests, 16
there now); `clamped_width` promoted from private to `pub` in
`ui_iced/layout.rs`; `Debug, PartialEq, Eq` added to
`ui/tree/flatten.rs::FlatRow`; one additive helper in `ui/strings.rs`
(`format_match_count`). No pilot or update-loop wiring (that is
TASK-022/023).

**Port shape - pure view, no `FastMdApp` inside the widget tree.**

`left_panel(&LeftPanelViewModel) -> Element<'_, Message>` renders header +
search row + (tree rows | search-result cards). Every user action becomes
`Message::Command(UserCommand)` or a named panel-channel message; the panel
never publishes to `user_command_bus`, never reads a file and never mutates
`FastMdApp` - the TASK-022/023 reducer does (FMD-068).

**Reused, not reimplemented (RUST-051).**

| Concern | How the iced panel gets it |
| --- | --- |
| Tree hierarchy + tag filtering | calls `crate::ui::panels::left::build_workspace_tree` (crate-internal). `prune_empty_dirs`, `library_for_path` and `insert_path_into_tree` are NOT duplicated |
| Row ordering (**UI-017** dirs-before-files, **UI-018** hide no-markdown dirs) | calls `crate::ui::tree::flatten_tree` and renders the returned `FlatRow`s **in order**. The iced module adds no sorting of its own - pinned by `tree_rows_keep_directories_before_files_at_every_level` and `tag_filter_prunes_directories_without_matching_files` |
| Row cache | reads `FastMdApp::cached_tree_rows` while `tree_dirty == false`; **never writes it back** (pinned by `tree_rows_never_write_the_cache_back`) - populating/invalidating is reducer state |
| UI-016 clamping | `ui_iced::layout::clamped_width`, the same rule the TASK-010 pane-resize uses (promoted to `pub` for this) |
| Context-menu card, anchoring, dismissal | `ui_iced::overlay::{context_menu_trigger, with_context_menu, MenuItem}` (TASK-017a) |
| Search engine | `TreeSearch` results are read off the view model; the panel does not scan |

Only what egui bound to `egui::Ui`/`egui::Context` was re-typed: the width
measurement, the row labels, the click→command mapping (the egui
`apply_*_row_click` handlers take `&mut TreeNodeContext`, which carries
`egui::Modifiers`, so they are not framework-free), and the menu item list
(`ui/tree/render.rs` builds those with `menu_item(ui, ..)` + a bus publish).

`LeftPanelViewModel` (built by `from_app(&FastMdApp)`):

| Field | Source (egui parity) |
| --- | --- |
| `rows: Arc<Vec<FlatRow>>` | cached rows, else `build_workspace_tree` + `flatten_tree(expanded_dirs)` |
| `query: String` | `search().query()` |
| `is_searching: bool` | `search().is_searching()` |
| `results: Arc<Vec<SearchResultEntry>>` | `search().results_arc()` |
| `selected_file: Option<PathBuf>` | `selection().selected_file()` |
| `selected_files: Vec<PathBuf>` | `selection().selected_files()`, **sorted** (egui collected from a `HashSet` and was not deterministic) |
| `pdf_backed: HashSet<PathBuf>` | per-row `pdf_backing_tracker().is_pdf_backed()` over the visible file rows - the tracker exposes no enumerator, so a snapshot is the only way to get a `PartialEq` view model |
| `folder_skills` / `note_skills: Vec<SkillFile>` | `AppConfig::list_skill_files_in(skill_dir(...))` resolved from the "System" content library, **read-only** (see flag 5) |
| `typst_available: bool` | `fastmd_pdf::is_typst_available()` - gates "Save as PDF..." exactly as egui does |
| `multi_select: bool` | `false` - see flag 1 |

**UI-016 auto-fit is a pure label→width function, and real measurement is NOT achievable in a pure view.**

`auto_fit_width(rows, viewport_width, measure)` reproduces the egui
formula exactly - `depth × 18.0 + measured("icon name") + 40.0`, capped at
`viewport × LEFT_PANEL_MAX_WIDTH_FRACTION` and floored at
`LEFT_PANEL_MIN_WIDTH` - but takes the measurer as an injected
`Fn(&str) -> f32`, because a pure view has no font atlas to ask. iced
0.14 only produces text metrics from inside `Widget::layout`, i.e. during
runtime layout, not from a stateless view function. **TASK-022 must supply
the real measurer** (or drive the pane width off a post-layout
size-reporting pass). Until then the fit is only as good as the measurer
that is injected; the clamping band itself is exact and pinned by tests.

One deliberate divergence: egui's inline pass produced
`((longest.min(cap)).max(min)).min(cap)`, which yields the **cap** on a
viewport so tiny that the 20 % cap falls below the minimum. The iced layer
uses `clamped_width`, where the **minimum** wins - chosen so the left panel
behaves exactly like the TASK-010 pane-resize instead of introducing a
second degenerate rule.

**Context-menu item → command mapping** (all rows, in menu order).

Directory row (`directory_menu_items`, egui `ui/tree/render.rs:18-96`):

| Menu row | Emitted message |
| --- | --- |
| `SHOW_IN_EXPLORER_ACTION` | `Command(ShowInExplorer(path))` |
| `COPY_PATH_ACTION` | `Command(CopyPath(path))` |
| `RENAME_ACTION` | `Command(Rename(path))` |
| `MOVE_ACTION` | `Command(Move(path))` |
| `CREATE_DIRECTORY_ACTION` | `Command(CreateDirectory { parent: path })` |
| `NEW_DOCUMENT_ACTION` | `Command(CreateDocument { parent: path })` |
| `DELETE_ACTION` | `Command(Delete(path))` |
| each folder skill (VFS-122) | `Tree(SkillRunRequested { skill, target_dir: Some(path), target_file: Some(path) })` |

Single file row (`file_menu_items`, egui `ui/tree/render.rs:120-303`):

| Menu row | Emitted message |
| --- | --- |
| `EDIT_BUTTON` | `Command(OpenInEditor(path))` |
| `SHOW_IN_EXPLORER_ACTION` | `Command(ShowInExplorer(path))` |
| `COPY_PATH_ACTION` | `Command(CopyPath(path))` |
| `RUN_AS_PROMPT_ACTION` | `Tree(RunAsPromptRequested { path })` |
| `PRINT_ACTION` | `Tree(PrintRequested { path })` |
| `SAVE_AS_PDF_ACTION` *(only when `typst_available`)* | `Command(SaveAsPdf(path))` |
| `RENAME_ACTION` | `Command(Rename(path))` |
| `MOVE_ACTION` | `Command(Move(path))` |
| `DELETE_ACTION` | `Command(Delete(path))` |
| each note skill (VFS-121) | `Tree(SkillRunRequested { skill, target_dir: None, target_file: Some(path) })` |

Multi-selected file row (`multi_select_menu_items`, egui
`ui/tree/render.rs:100-116`), shown by the same gate egui uses - only when
`selected_files.len() > 1` **and** this path is one of them:

| Menu row | Emitted message |
| --- | --- |
| `MERGE_ACTION` | `Command(MergePrompt(paths))` |
| `DELETE_ACTION` | `Tree(DeleteSelectedRequested { paths })` |

`SaveAsPdf` is the existing `UserCommand` - its executor already runs the
`rfd` "Save as" dialog and the background export, so the panel does not
re-implement that flow.

**Deferred intent - four new `TreeMessage` variants.**

Three egui menu rows do their work *inside* the click: "Run as prompt"
reads the file (`render.rs:143-160`), each skill row reads the skill file
(`render.rs:311-337`), "Print" hands a job to the background channel
(`render.rs:161-173`), and the multi-select "Delete" fans out into N
`Delete` commands plus a selection clear (`render.rs:108-115`). A pure
view may not do I/O, and none of these is expressible as one
`UserCommand`. They therefore emit `RunAsPromptRequested`,
`SkillRunRequested`, `PrintRequested` and `DeleteSelectedRequested`, each
naming its target, and the reducer performs the read/dispatch and publishes
the real command(s). This is recorded as a deliberate exception to
message.rs's "panel channels carry view-local state only" rule, documented
in that module's header.

**Cursor tracking - `LayoutMessage::CursorMoved { point: iced::Point }`.**

This closes TASK-017a's open item 1. `MouseArea::on_right_press` takes a
fixed `Message` and cannot carry the press point
(`iced_widget-0.14.2/src/mouse_area.rs:68-71`), so the anchor must be
state the reducer reads back. `Layout` is the honest home: like a split
drag and the tree auto-fit, a cursor report is a statement about *where in
the pane geometry something is* - not user intent (so not `Command`) and
not any panel's private view state. `iced::Point` is `Copy`
(`iced_core-0.14.0/src/point.rs:7`), so the channel keeps `Copy`; the
hand-written `PartialEq` gained the matching arm. Coordinates are
**widget-relative** (`mouse_area.rs:358-360`), so the overlay must be
mounted against the same element the trigger wrapped (or the offset added) -
unchanged from the TASK-017a caveat.

**Gate results (this pass).** `cargo check -q` PASS (exit 0, clean);
`cargo clippy -p fastmd --bins --lib -- -D warnings` PASS (exit 0);
`cargo fmt` run, `cargo fmt --check` PASS workspace-wide (exit 0);
`cargo nextest run --status-level fail --show-progress none` PASS
(**1397 run / 1397 passed / 1 skipped**, +40 over the 1357 this task
started from: 37 left-panel + 3 message); `cargo doc --no-deps --quiet`
PASS (exit 0, no warnings).

**Needs the orchestrator's eyes.**

1. **Modifier-aware multi-select (UI-022) is NOT wired.** egui computed
   `multi` from `ui.input(|i| i.modifiers)` at click time. iced 0.14's
   `MouseArea::on_press` carries no modifier state
   (`mouse_area.rs:38-41`). The panel emits
   `LeftPanelViewModel::multi_select`, and `from_app` sets it to `false`
   because nothing on the iced side tracks modifiers yet. **TASK-022 must
   fold `iced::Event::Keyboard(KeyEvent { modifiers, .. })` into that
   field**, otherwise every tree click is a single-select and the
   multi-select menu can never open. The pure functions
   (`row_click_command(row, multi)`, `context_menu_items(..)`) already take
   the flag explicitly, so the wiring is a reducer-only change.
2. **`FlatRow` gained `Debug, PartialEq, Eq` derives** (not in the task's
   deliverable list). It derived only `Clone`, which blocked a derivable
   iced view model and blocked comparing flattened row lists in tests - the
   egui build never needed either. Purely additive; no existing behaviour
   touched.
3. **`ui_iced::layout::clamped_width` was made `pub`.** Shared with
   `auto_fit_width` so every UI-016 width in the iced layer obeys one
   clamping rule rather than two near-identical ones.
4. **`strings.rs` was extended again** (additive only): `format_match_count`
   hoists the `format!("({})", match_count)` the egui panel inlined at
   `ui/panels/left.rs:303`, per GUD-002.
5. **Skill lookup deliberately uses the read-only path.**
   `AppConfig::list_folder_skills()` / `list_note_skills()` call
   `create_dir_all` as a side effect, which a per-frame view must not do
   (RUST-006). The port mirrors the egui menu instead: resolve
   `Skills/{Folder,Note}` under the "System" content library and call
   `AppConfig::list_skill_files_in`, which returns empty for a missing
   directory. `skill_dir()` is pure and takes the fallback explicitly.
6. **UI-050 spec drift, mirrored not fixed.** UI-050 says the search box
   sits "at the bottom of the directory tree pane"; the egui build has
   always rendered it directly under the header. This port mirrors the
   shipped behaviour. Flagging per RUST-041; SPEC.md was not touched.
7. **Search submit relies on message ordering.** `text_input::on_submit`
   takes a fixed `Message`, so `Submitted { query }` is built from
   `view_model.query`. That is correct only because iced processes the
   keystroke's `QueryChanged` (and rebuilds the view model) before the
   Enter event - true for the sequential Elm loop, but worth knowing if
   message batching ever lands.
8. **Tooltips are not rendered.** iced 0.14 has no tooltip primitive under
   DEC-005 (`iced_aw` ruled out), so `SEARCH_TRIGGER_TOOLTIP` /
   `SEARCH_CLEAR_TOOLTIP` have no iced home yet.
9. **`PdfBackingTracker` still has no enumerator.** The view model snapshots
   PDF-backed paths by calling `is_pdf_backed()` once per visible file row.
   Correct but O(rows) read-lock acquisitions per frame; a `snapshot()` /
   `paths()` accessor on the tracker would make it one lock. Left alone as
   out of scope.

