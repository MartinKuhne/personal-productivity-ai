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

### TASK-013 verification log (center panel / tab strip ported to iced - 2026-10-18)

**Deliverables.** New `src/app/ui_iced/panels/center.rs` + `center_tests.rs`
sidecar (38 tests); `pub mod center;` + module-doc bullet in
`ui_iced/panels/mod.rs`; `NOTE_SKILLS_SUBDIR` promoted from private to `pub`
in `ui_iced/panels/left.rs`; one additive glyph const in `ui/strings.rs`
(`ICON_UNSAVED_DOT`, U+25CF). No pilot or update-loop wiring (that is
TASK-022/023).

**Port shape - pure view, no `FastMdApp` inside the widget tree.**

`center_panel(&CenterPanelViewModel) -> Element<'_, Message>` renders the
tab strip plus the viewer area below it (or the agent-session band while
`show_results` is on). Every user action becomes
`Message::Command(UserCommand)` or a named panel-channel message; the panel
never publishes to `user_command_bus`, never reads a file and never mutates
`FastMdApp` - the TASK-022/023 reducer does (FMD-068).

`CenterPanelViewModel` (built by `from_app(&FastMdApp)`):

| Field | Source (egui parity) |
| --- | --- |
| `tabs: Vec<TabEntry{path,title,unsaved,active}>` | `tabs().tabs` (the strip's source vector); `title` via the pure `tab_title()` = `file_name()` with the same `unwrap_or_default` fallback `Tabs::tab_strip` caches (`ui/tabs.rs:117-123`) |
| `active_tab: Option<usize>` | `active_tab_index(paths, selection().selected_file())` - the single expression of egui's per-tab `selected_file() == Some(tab_path)` test |
| `show_agent_session: bool` | `orchestrator.agent_panel_state.show_results` (egui `show_center_panel`'s first routing branch) |
| `agent_status: String` / `agent_running: bool` | `agent().state().status` / `.running` |
| `thinking: String` | `orchestrator.agent_transcript.thinking` (already delimiter-extracted upstream) |
| `active_pdf_backed: bool` | `pdf_backing_tracker().is_pdf_backed(selected)` - drives the `pdf_surface` frame egui painted behind PDF-backed documents |
| `note_skills: Vec<SkillFile>` | `AppConfig::list_skill_files_in(skill_dir(content_libraries(), NOTE_SKILLS_SUBDIR, get_skills_note_dir))` - the **read-only** path (RUST-006), same rule TASK-012 established; this is where "Format Markdown" comes from |

`TabEntry.unsaved` is reported `false` by `from_app` - see flag 2.

**Tab interactions - what each gesture emits (UI-049).**

| Gesture | Emitted message | egui source |
| --- | --- | --- |
| left-click tab label | `Command(SelectFile { path, multi: false })` | `ui/panels/center.rs:214-221` |
| middle-click tab | `Command(CloseTab(index))` | `:222-227` |
| `x` close button | `Command(CloseTab(index))` | `:304-313` |
| right-click tab | `Tab(ContextMenuOpened { index })` | `response.context_menu(..)` `:228` |
| pointer motion over tab | `Layout(CursorMoved { point })` | DEC-005 anchor tracking |

**Tab-select is modelled as a COMMAND, not view state.** egui never kept an
active-tab index of its own - the active tab *is*
`selection().selected_file()`, and clicking a label publishes
`UserCommand::SelectFile { multi: false }`. The iced port keeps exactly
that: the strip emits the intent and derives the `active` flag from the
snapshot. There is deliberately **no** `TabMessage::TabSelected` variant
(message.rs routing rule 1: user intent rides `Message::Command`).
`TabMessage` therefore stays exactly as TASK-021 defined it -
`ContextMenuOpened{index}` / `ContextMenuClosed` - the strip's only genuine
view-local state.

The `x` button sits **inside** the tab's `MouseArea`, which is safe and is
what preserves egui's semantics: `MouseArea::update` updates its child
first and returns without running its own handlers once the child has
captured the event (`iced_widget-0.14.2/src/mouse_area.rs:230-245`), and
`Button::update` calls `shell.capture_event()` on press (`button.rs:312`).
Clicking `x` closes the tab without also selecting it; middle-press is not
a button event, so it still reaches the tab's `on_middle_press`.

**Tab context-menu item -> message mapping** (UI-026..UI-034, egui order,
`tab_context_menu_items(index, path, note_skills)`):

| Menu row | Emitted message |
| --- | --- |
| `EDIT_BUTTON` | `Command(OpenInEditor(path))` |
| `CLOSE_TAB_MENU` | `Command(CloseTab(index))` |
| `CLOSE_OTHERS_MENU` | `Command(CloseOtherTabs(index))` |
| `CLOSE_ALL_TABS_MENU` | `Command(CloseAllTabs)` |
| `COPY_PATH_ACTION` | `Command(CopyPath(path))` |
| `SHOW_IN_EXPLORER_ACTION` | `Command(ShowInExplorer(path))` |
| `OPEN_IN_EDITOR_ACTION` | `Command(OpenInEditor(path))` |
| each note skill - VFS-121, incl. the seeded **FormatMarkdown** (UI-033) | `Tree(SkillRunRequested { skill, target_dir: None, target_file: Some(path) })` |

**UI-033 "Format Markdown" is not a `UserCommand`.** In the shipped egui
build it is a seeded VFS-121 *note skill*
(`Skills/Note/FormatMarkdown.md`) whose click reads the skill file inside
the menu closure and publishes `UserCommand::RunSkillPrompt { content,
target_dir: None, target_file }` (`ui/panels/center.rs:276-301`). A pure
view may not do I/O, so the row emits the **existing**
`TreeMessage::SkillRunRequested` deferred-intent variant - the same one the
file-tree's note-skill rows emit. Reusing it rather than adding a
`TabMessage` twin is what UI-034's "consistent behaviour across both UI
locations" asks for (GUD-001), and it keeps `TabMessage: Copy` intact (a
`PathBuf`-carrying tab variant would have cost it). TASK-022 can route both
variants through one handler. `strings::FORMAT_MARKDOWN_ACTION` exists but
is **not** used here: the row label is the skill's display name
(`file_stem`), exactly as egui rendered it.

Menu **separators** are not representable - the overlay card is a uniform
column of buttons and its `MenuItem` has no separator kind - so egui's
three separator groups survive as row *order* only.

**UI-048 - the thinking delimiter is NOT a center-panel extraction.** The
delimiter handling already lives upstream: `AgentTranscript::apply_event` /
`split_thinking_and_content` (`src/app/ui/agent/transcript.rs`) and the
agent's response formatter (AGENT-022). What the egui center panel owned
was only the *gate* - render the "Thinking Process" block when the
extracted buffer is non-empty - which is ported here as the pure
`thinking_section_visible(&str) -> bool` plus
`CenterPanelViewModel::show_thinking()`. The collapsible disclosure (A9's
`toggler` pattern) and the streaming response markdown are the
agent-session render path, **TASK-018**; this module renders the band's
header, status line and thinking text and stops there.

**What the document-viewer area defers to TASK-015.**
`viewer_area` / `document_preview` render the active tab's **title only**,
inside the always-allocated `scrollable` + `pdf_surface` frame. Deferred to
the TASK-015 render path, exactly as egui had them
(`ui/panels/center.rs:341-385`):

* the YAML front-matter table (`render_yaml_table`),
* the search-aware markdown renderer (`render_markdown_with_search`),
* the FTWA `DeficitStrategy` plumbing and `heading_ids_arc()`,
* task-checkbox toggles collected during render and applied to the
  markdown source (P0-2),
* `scroll_to_header_id` / `scroll_to_search` handling.

The `Column` in `document_preview` is the extension point those append to.
The empty state (`NO_FILE_SELECTED_PROMPT`) **is** ported.

**Reused, not reimplemented (RUST-051).**

| Concern | How the iced panel gets it |
| --- | --- |
| `CloseTab` / `CloseOtherTabs` / `CloseAllTabs` / `ClearAgentSession` construction | `pub use crate::ui::panels::center::{apply_tab_close_click, apply_tab_close_others_click, apply_tab_close_all_click, clear_agent_session_command}` - the egui panel's framework-free constructors, same as TASK-011 did for `top` |
| Note-skill folder resolution | `super::left::{skill_dir, NOTE_SKILLS_SUBDIR}` - one rule, not two (`NOTE_SKILLS_SUBDIR` made `pub` for this) |
| Context-menu trigger / card / dismissal | `ui_iced::overlay::{context_menu_trigger, with_context_menu, MenuItem}` (TASK-017a) |
| Cursor anchor | `LayoutMessage::CursorMoved` (TASK-012) |
| Deferred skill-run intent | `TreeMessage::SkillRunRequested` (TASK-012) |
| Tab data model | reads `Tabs.tabs` directly; no parallel tab store was introduced |

Only what egui bound to `egui::Ui`/`egui::Context` was re-typed: the
label/colour lookups, the click -> command mapping (the egui handlers
publish inside the closure), and the menu item list.

**Gate results (this pass).** `cargo check -q` PASS (exit 0, clean);
`cargo clippy -p fastmd --bins --lib -- -D warnings` PASS (exit 0);
`cargo fmt` run, `cargo fmt --check` PASS workspace-wide (exit 0);
`cargo nextest run --status-level fail --show-progress none` PASS
(**1435 run / 1435 passed / 1 skipped**, +38 over TASK-012's 1397 - all
38 are the new `ui_iced::panels::center` tests);
`cargo doc --no-deps --quiet` PASS (exit 0, no warnings from this task's
files).

**Needs the orchestrator's eyes.**

1. **`ui/strings.rs` was extended again** (additive only): `ICON_UNSAVED_DOT`
   (U+25CF BLACK CIRCLE). GUD-002 forbids the literal in the panel. It is
   a Unicode geometric shape rather than a Phosphor glyph, so it resolves
   through the system symbol font - [UNVERIFIED] at runtime, same caveat as
   TASK-011's check/hamburger glyphs.
2. **UI-027 / UI-029 unsaved-change confirmation is NOT implemented - in
   either build.** There is no per-tab dirty state anywhere in the shipped
   app: `Tabs` has no dirty flag, `TextBuffer` exposes no modified signal to
   the strip, and the egui tab menu closes without ever prompting. The view
   model carries `TabEntry::unsaved` and the view renders the marker slot
   (fixed-width, content toggled), so the surface is ready, but `from_app`
   reports `false` because there is nothing honest to read. Flagged as spec
   drift per RUST-041; SPEC.md was not touched. TASK-022 needs a
   dirty-tracking source before the confirmation dialogs mean anything.
3. **`ui_iced::panels::left::NOTE_SKILLS_SUBDIR` was made `pub`** so the
   tab menu resolves the same `Skills/Note` folder through the existing
   `skill_dir()` instead of repeating the literal (RUST-021). Purely
   additive.
4. **The tab menu's leading `EDIT_BUTTON` row is ported even though SPEC
   UI-027..UI-033 does not list it.** The shipped egui menu has it
   (`ui/panels/center.rs:229-235`), so parity wins; noting it because the
   task brief's seven-item list is the SPEC list, not the shipped list.
5. **The agent-session band is a partial port by design.** Header, status
   line, the UI-048 gate and the `ClearAgentSession` close intent are here
   so `center_panel`'s routing matches egui; the streaming response view,
   the collapsible disclosure and the response markdown are TASK-018. If
   the reviewer would rather the center panel not render that band at all
   until TASK-018, the gate is one `if` in `center_panel`.
6. **`TAB_TEXT_SIZE` (13.0) and the other layout consts are visual-pass
   values, not ported ones.** egui's `selectable_label` used the default
   body size (12.5 px) and no named constant existed to port, so each const
   documents its own role instead of implying an egui predecessor.
7. **`tab_title()` duplicates a three-line mapping** from
   `Tabs::tab_strip`'s cache builder. It cannot call `tab_strip()` because
   the cache needs `&mut Tabs` while a view model is built from
   `&FastMdApp`. If the orchestrator wants one source of truth, the clean
   refactor is a pure `Tabs::title_for(&Path) -> String` that both call -
   deliberately not done here to keep the change inside the iced layer.

### TASK-014 verification log (inline editor ported to iced `text_editor` - 2026-10-18)

**Scope.** DEC-007: iced 0.14 `text_editor` has no native undo/redo, so the
editing model egui kept in `ui/text_buffer.rs` + `ui/editor_egui.rs` was
re-hosted as a pure state module plus a pure view.

**Files.**

| File | Role |
| --- | --- |
| `src/app/ui_iced/editor.rs` | `EditorHistory`, `EditorViewModel`, `classify_action`, `apply_editor_message`, `editor_card`, `editor_modal` |
| `src/app/ui_iced/editor_tests.rs` | 49 sidecar tests |
| `src/app/markdown/validation.rs` | `validate_markdown`, `MarkdownParseError`, `MarkdownErrorKind` (UI-043) |
| `src/app/markdown/validation_tests.rs` | 27 sidecar tests |
| `src/app/markdown/parser.rs` | new `gfm_options()`; `parse_markdown_to_events` now calls it |
| `src/app/ui/strings.rs` | `build_editor_status_text`, `build_editor_validation_error`, `EDITOR_ERR_UNTERMINATED_FENCE`, `EDITOR_INPUT_PLACEHOLDER` |

Nothing is wired into the running app loop - that is TASK-022/023.

**`EditorHistory` design + cap (UI-042).** A `Vec<String>` of snapshots plus
a `cursor: usize` indexing the snapshot that *is* the buffer's current state,
so undo/redo are index moves rather than a second stack. `MIN_UNDO_STEPS =
100` is UI-042's floor; `UNDO_CAPACITY = MIN_UNDO_STEPS + 1 = 101` is the
stored-snapshot ceiling. The `+1` is load-bearing: `N` snapshots only yield
`N - 1` undo steps, so a cap of exactly 100 would ship 99 and miss the
requirement. Both are pinned at compile time (`const _: () = assert!(...)`)
and at runtime (push 150 -> `len() == 101`, exactly 100 undo steps
available, the 101st returns `None`, oldest retained is `s49`). Overflow
evicts the **oldest** snapshot. Pushing a snapshot equal to the current one
is **coalesced** - no entry added, and the redo tail survives - which is the
same rule egui's `UndoStack` used so a stream of no-op edits cannot burn the
budget. `with_capacity(0)` is raised to 1.

**Action interception (DEC-007).** iced 0.14 does not mutate the widget's
own buffer: `TextEditor::on_action` *reports* an `Action`
(`iced_widget-0.14.2/src/text_editor.rs:212-222`) and the application
applies it with `Content::perform` (`text_editor.rs:411-416`; the crate's
own example at `text_editor.rs:28-30`). That callback is the interception
point. `classify_action` delegates to `Action::is_edit()`, verified verbatim
at `iced_core-0.14.0/src/text/editor.rs:94-99`:

```rust
pub fn is_edit(&self) -> bool {
    matches!(self, Self::Edit(_))
}
```

`Action::Edit(..)` is the only content-changing variant. Its seven `Edit`
payloads (`Insert`, `Paste`, `Enter`, `Indent`, `Unindent`, `Backspace`,
`Delete` - `iced_core-0.14.0/src/text/editor.rs:101-118`) all push;
`Move`, `Select`, `SelectWord`, `SelectLine`, `SelectAll`, `Click`, `Drag`
and `Scroll` are `CaretOnly` and never touch history. Two tests cover this:
one enumerates all 28 `Action` values and asserts the exact partition
(7 content / 21 caret), one asserts `classify_action(a) == a.is_edit()` for
every value so the rule cannot drift from iced's own predicate.

**What iced `text_editor` provides natively (verified, not built here).**

| Req | Native support | Source |
| --- | --- | --- |
| UI-039 drag select | `Action::Drag(Point)` | `iced_core-0.14.0/src/text/editor.rs:85-86` |
| UI-039 double-click word | `Action::SelectWord` | `text/editor.rs:75-76` |
| UI-039 triple-click line | `Action::SelectLine` | `text/editor.rs:77-78` |
| UI-039 Shift+arrows | `Action::Select(Motion)` | `text/editor.rs:73-74` |
| UI-040 Copy/Cut/Paste | widget owns the OS clipboard binding; paste arrives as `Action::Edit(Edit::Paste(Arc<String>))`, cut as a delete edit over the selection | `text/editor.rs:101-118` |
| UI-041 char / word / line / Home / End / Ctrl-Home / Ctrl-End | `Motion::{Left, Right, Up, Down, WordLeft, WordRight, Home, End, PageUp, PageDown, DocumentStart, DocumentEnd}`, with `Motion::widen()` mapping Ctrl+Left -> WordLeft and Ctrl+Home -> DocumentStart | `text/editor.rs:119-159` |
| UI-038 monospace | `iced::Font::MONOSPACE` | `iced_core-0.14.0/src/font.rs:27` |
| UI-038 line/column | `Content::cursor() -> Cursor { position: Position { line, column } }`, 0-based; `display_line_column` makes it 1-based | `text/editor.rs:189-216`, `iced_graphics-0.14.0/src/text/editor.rs:277-303` |

Built here because iced has no equivalent: the undo/redo stack, the
front-matter-preserving save model, and pre-save GFM validation.

**Validation - written, not reused (there was nothing to reuse).** The egui
build **never validated before saving**: `TextBuffer::save`
(`ui/text_buffer.rs:329-355`) joins and writes with no parse step, so
UI-043 was unimplemented drift. A search of `crate::markdown` found no
validation helper. Per the subsystem's rule that `markdown/` is the single
`pulldown-cmark` import point, `validate_markdown` was added to
`crate::markdown` (new `validation.rs`) rather than to the view, and is
re-exported from `markdown/mod.rs`.

`pulldown_cmark::Parser` is **total** - `new_ext` returns an iterator,
never a `Result`, and malformed constructs degrade to literal text. Verified
against the 0.13.4 event stream: a dangling `[^missing]` emits literal
`Text("[")`/`Text("^missing")`/`Text("]")` with **no** `FootnoteReference`;
a short table row is **padded** with an empty `TableCell`; a long row has
its extra cell **dropped**; an unterminated fence is auto-closed at EOF. So
UI-043's "if parsing fails" cannot be read literally. `validate_markdown`
still drives the parser with the renderer's exact option set (that is the
step the requirement names) and then reports the structural defects the
parser tolerates silently that are real authoring errors with a determinate
location. One check ships: `MarkdownErrorKind::UnterminatedCodeFence`,
located at the **opening** fence (byte offset + 1-based line/column),
because that is the one tolerated defect that visibly destroys the rest of a
document. Dangling footnotes and table column mismatches are pinned as
*not-yet-detectable* by two tests so a future parser change surfaces.

`gfm_options()` was extracted from `parse_markdown_to_events` so UI-043's
"the same GFM parser used for rendering" is structural, not aspirational -
one definition, asserted to be exactly
`TABLES | FOOTNOTES | STRIKETHROUGH | TASKLISTS` and to leave
`WIKILINKS`/`SMART_PUNCTUATION`/`MATH`/`GFM`/`HEADING_ATTRIBUTES` off.
Validation runs on every content-changing action (not only at save) because
the Save button's disabled state has to be visible while the user types.

**Front matter (UI-037 / UI-044).** Reused, not reimplemented:
`EditorViewModel::open` splits with the existing `DocumentContent::parse`
so only the body reaches the text area and the YAML is cached verbatim;
`EditorViewModel::document()` re-joins through `DocumentContent`'s
`Display` impl. A test asserts the rejoined text re-parses to the identical
front-matter string. The actual `fs::write` stays in the reducer - the view
only emits `SaveRequested { path }`.

**Reducer contract for TASK-022/023.** `apply_editor_message(&mut vm,
&EditorMessage) -> bool` is the pure entry point (no I/O, no bus, no
overlay mutation; returns whether the buffer changed). `Action` is applied
through `apply_action`; `Undo`/`Redo` replay one snapshot **only when the
message's `path` matches the buffer's file** - a message aimed at another
tab is ignored rather than corrupting this buffer; `SaveRequested` /
`CancelRequested` never mutate the buffer. `EditorViewModel::open` seeds the
opened body into the history as the baseline, which is what makes the *first*
keystroke undoable.

**Gate results (this pass).** `cargo check -q` PASS (exit 0, clean);
`cargo clippy -p fastmd --bins --lib -- -D warnings` PASS (exit 0);
`cargo fmt` run, `cargo fmt --check` PASS workspace-wide (exit 0);
`cargo nextest run -p fastmd` PASS (**1512 run / 1512 passed / 1 skipped**,
+77 over TASK-013's 1435 - 49 `ui_iced::editor` + 27
`markdown::validation` + 1 `ui::strings` builder test);
`cargo doc --no-deps --quiet -p fastmd` PASS
(exit 0, no warnings from this task's files).

**Needs the orchestrator's eyes.**

1. **UI-043 was never implemented in the egui build** (see above), so this
   task *adds* behaviour rather than porting it. SPEC.md was not touched.
   The requirement's literal wording ("if parsing fails") is unsatisfiable
   with a total parser; the shipped realisation is documented in
   `markdown/validation.rs`'s module header. If the reviewer wants a
   stricter gate, the honest options are a different parser (comrak has a
   fallible API) or an explicit lint list - both out of scope here.
2. **Undo/redo resets the caret to the document start.** Per this task's
   contract `EditorHistory` snapshots the *text* only; egui's `UndoStack`
   stored `Cursor` + `Selection` per entry and restored them. Extending the
   snapshot to `(String, iced::widget::text_editor::Cursor)` would restore
   the caret - small change, but it changes the public `undo() -> Option<&str>`
   shape, so it was left as a follow-up rather than silently widening the API.
3. **The status-bar column is iced's glyph index, not a Unicode character
   count.** `Content::cursor()` maps cosmic-text's `cursor.index`
   (`iced_graphics-0.14.0/src/text/editor.rs:280-287`), which under
   advanced shaping can differ from the character count for composed
   scripts. ASCII columns are exact (asserted). The egui build computed a
   true character column, so this is a small fidelity regression for
   non-ASCII caret positions.
4. **`markdown/parser.rs` was modified** (not just read): `gfm_options()`
   extracted and `parse_markdown_to_events` switched to it. Behaviour-
   preserving, covered by the existing parser suite. `document.rs`'s
   `apply_task_toggle` still repeats the same four `insert` calls - a
   follow-up could fold it onto `gfm_options()` too; left alone to keep this
   task's blast radius small.
5. **`EditorViewModel` owns the iced `text_editor::Content`** rather than a
   `String`, because `Content` *is* the buffer and a parallel `String` would
   be a second source of truth. Consequence: undo/redo rebuild `Content`
   (`Content` has no in-place setter), which is also why the caret resets.
   The VM is therefore not iced-free, though it is side-effect-free.
6. **The dialog is a fixed 800x600** (`EDITOR_DIALOG_WIDTH`/`_HEIGHT`),
   matching egui's `default_size`. egui's window was resizable; iced has no
   resizable modal in the TASK-017a primitive, so resizing is not ported.
   If a resizable editor is wanted it needs overlay work, not editor work.
7. **`ui/strings.rs` extended (additive only)**: four items. The egui
   editor hardcoded `"Line: {} | Col: {}"` in the render code, which the
   new `build_editor_status_text` replaces for this layer.
8. **No keyboard bindings for Save/Cancel or Ctrl+Z/Ctrl+Y are wired.**
   `EditorMessage::Undo`/`Redo` exist and the reducer honours them, but
   nothing produces them from a key press yet - iced routes editor key
   events into `Action`s and the app-level shortcut map is TASK-022's. The
   buttons are the only Save/Cancel path in this commit.

### TASK-015 verification log (markdown render path ported to iced - 2026-10-18)

**Scope.** The iced counterpart of `src/app/ui/render/` - the read-only
markdown *viewing* path - plus a rewrite of the egui FTWA adapter. Both live
under `src/app/ui_iced/` and are pure: no file I/O, no bus access, no
application-state mutation. The parser and the pure solver
(`src/app/markdown/table_width/`) are **not modified** - the adapter is a
new consumer of them. Nothing is wired into the running app loop; that is
TASK-022/023.

**Files.**

| File | Role |
| --- | --- |
| `src/app/ui_iced/table_width.rs` | iced FTWA adapter: `IcedTextMeasurer` (impl of `markdown::table_layout::TextMeasurer`), `TextMetrics`, `heuristic_width`, `solver_available_width`, `solve_table_layout`, `column_widths`, `column_length`, `needs_horizontal_scroll`, spacing constants (392 lines) |
| `src/app/ui_iced/table_width_tests.rs` | 32 sidecar tests |
| `src/app/ui_iced/render/mod.rs` | `RenderContext`, `RenderBlock`, `BlockKind`, `ListMarker`, `TaskCheckbox`, `blocks_from_events`, `render_block`, `render_markdown`, `render_document`, `paragraph_block`, `task_toggle_message`, search matching (451 lines) |
| `src/app/ui_iced/render/mod_tests.rs` | 30 sidecar tests |
| `src/app/ui_iced/render/inline.rs` | `SpanModel`, `spans_from_inline`, `span_for_text`/`span_for_link`, `font_for_style`, `to_iced_span`, `rich_text_block`/`rich_text_element` (237 lines) + 22 tests |
| `src/app/ui_iced/render/heading.rs` | H1..H5 size ladder (`H1_SIZE`..`H5_SIZE`; H6 and out-of-range levels collapse to the `H5_SIZE` floor), blank-heading rule (70 lines) + 10 tests |
| `src/app/ui_iced/render/code.rs` | `CodeBlockModel`, fenced-block panel on `code_surface`, copy button (136 lines) + 12 tests |
| `src/app/ui_iced/render/table.rs` | `TableModel`/`TableCellModel`/`TableRowData`, `table_model`, `from_layout`, `table_widget`, `cell_element` (295 lines) + 24 tests |
| `src/app/ui_iced/render/yaml.rs` | `YamlRowModel`, front-matter key/value rows (161 lines) + 17 tests |
| `src/app/ui_iced/message.rs` | new `Message::Render(RenderMessage)` channel (`message.rs:120`, enum at `:481`) |
| `src/app/ui/strings.rs` | `LIST_BULLET` (U+2022), `ORDERED_LIST_MARKER_SUFFIX` (additive only) |
| `src/app/ui_iced/mod.rs` | `pub mod render;` / `pub mod table_width;` |

**Document model mapped from (untouched).** The view consumes exactly what
`crate::markdown` already produces:

* `RenderEvent::{FlushInline, Heading, CodeBlock, Table, Space, Separator}`
  - `FlushInline` carries `elems`, `needs_bullet`, `task_checked`, `indent`
  and `list_ordinal`, which become `BlockKind::Paragraph { spans, indent,
  marker, task }`.
* `InlineElem::{Text, Link, Image, Html, SoftBreak}` - each maps to one
  `SpanModel` (images via `markdown::image_placeholder`, soft breaks as one
  space), matching the egui renderer's element-by-element walk.
* `TextStyle::{bold, italic, code, strikethrough, muted}` - resolved
  once, at mapping time, into font/size/colour/underline/strikethrough on the
  `SpanModel`. No style is re-derived inside a widget builder.

**Pure descriptor layer.** Mirrors the top panel's `MenuEntry` pattern
(TASK-011) so the mapping is testable without an iced runtime:

```text
RenderEvent ──blocks_from_events──▶ RenderBlock { kind: BlockKind, scroll_to_me }
                                       │
              BlockKind::{Heading, Paragraph, Code, Table, Spacer, Separator}
                                       │
                            render_block ──▶ Element<'static, Message>
```

Every `BlockKind` carries fully resolved descriptors -
`SpanModel`, `CodeBlockModel`, `TableModel`, `YamlRowModel` - so tests
assert on data (sizes, colours, solved widths, the exact message a link or
copy button carries) rather than on a widget tree. All widget builders return
`Element<'static, Message>`; no builder takes a `&mut Ui`-style parameter and
none can fail.

**FTWA adapter contract.** The solver
(`markdown::table_width`) and the layout driver
(`markdown::table_layout::{TableLayoutBuilder, TextMeasurer}`) are reused
verbatim, as are `ui::table_width::{TablePadding, TableRenderConfig}`. What
the adapter owns is the *geometry contract*:

| Constant | Value | Why |
| --- | --- | --- |
| `CELL_PADDING_X` | `5.0` | per-side cell padding; iced's spacing identity makes the visible gap `2p + s` |
| `CELL_PADDING_Y` | `1.0` | row padding, same identity vertically |
| separator | `0.0` | the iced table draws no rule by default in this port |
| `COLUMN_SPACING` | `10.0` | `2*5 + 0` - equals egui's `item_spacing.x`, so column pitch is unchanged |
| `ROW_SPACING` | `2.0` | `2*1 + 0` |

The identity is not invented - it is iced's own: `let spacing_x =
self.padding_x * 2.0 + self.separator_x;`
(`iced_widget-0.14.2/src/table.rs:258`). `solver_available_width(a) =
(a - 2 * CELL_PADDING_X).max(0.0)` gives the solver the *content* width, so
the outer gutter cannot push the table into overflow.

**Injected text measurement.** iced exposes no renderer inside `view()`, so
measurement is injected: `IcedTextMeasurer::new(&'a dyn Fn(&str, Font, f32)
-> f32, TextMetrics)`. The default (`IcedTextMeasurer::heuristic`) uses
`heuristic_width` - `PROPORTIONAL_AVG_ADVANCE = 0.5` em,
`MONOSPACE_AVG_ADVANCE = 0.6` em per character - which is deterministic and
therefore testable. TASK-022/023 can inject a renderer-backed closure
(`text::Width::Some(w)` via a `Renderer::measure`) **without changing any
signature**. `TextMetrics { body_size, mono_size, line_height }` keeps the
font sizes the measurer reports and the row height the view reserves in one
place.

`measure_cell` is ported **verbatim** from the egui measurer
(`src/app/ui/table_width/mod.rs:415-510`), including its token-merge quirks
- a word split across two `Text` runs stays one unbreakable token, and a
single-part run ending in whitespace closes the pending token immediately
measured with the *previous* run's font. `test_trailing_whitespace_single_
part_run_closes_the_token_like_egui` locks that quirk so a future
"cleanup" cannot silently change solved widths.

**New message channel (why `UserCommand` was not enough).** A
`Message::Render(RenderMessage)` channel was added with three variants:

* `LinkActivated { url, source_file }` - resolving a relative link needs
  the workspace file list, which the pure view does not have.
* `TaskToggled { document, task_index, checked }` - `UserCommand::
  ApplyTaskToggles` exists but mutates the **agent transcript**
  (`src/app/command_executor.rs:568-576` calls
  `ui::render::apply_task_toggle(&mut self.agent_transcript.content, ...)`),
  not the open document. Carrying `document` keeps the command
  self-contained and replayable ([FMD-068]).
* `CopyCodeRequested { content }` - iced exposes the clipboard only to
  `update` (via `EventLoop::clipboard()` / a `Task`), never inside `view`.

**Verified iced 0.14 APIs (local registry sources).**

| API | Citation |
| --- | --- |
| `Font::MONOSPACE` / `Font::DEFAULT` | `iced_core-0.14.0/src/font.rs:27` |
| `text::Span` fields | `iced_core-0.14.0/src/text.rs:379-402` |
| `Span::{size, font, color, link, background, underline, strikethrough}` | `iced_core-0.14.0/src/text.rs:413-547` |
| `Rich::with_spans` | `iced_widget-0.14.2/src/text/rich.rs:68-75` |
| `Rich::on_link_click` | `iced_widget-0.14.2/src/text/rich.rs:140-146` |
| `table()` / `column()` | `iced_widget-0.14.2/src/table.rs:18-27` / `:34-49` |
| spacing identity `2*padding + separator` | `iced_widget-0.14.2/src/table.rs:258-259` |
| all-`Shrink` forces the first column to `Fill` | `iced_widget-0.14.2/src/table.rs:129-133` |
| `Column::width` | `iced_widget-0.14.2/src/table.rs:650-653` |
| `table::Style` carries **only** `separator_x`/`separator_y` | `iced_widget-0.14.2/src/table.rs:676-681` |
| `Scrollable::horizontal()` | `iced_widget-0.14.2/src/scrollable.rs:133-135` |
| `Wrapping::WordOrGlyph` (there is **no** `WordOrCharacter`) | `iced_core-0.14.0/src/text.rs:181-192` |
| `checkbox(is_checked)` takes only the bool | `iced_widget-0.14.2/src/helpers.rs:1281` |

Gotchas hit and settled during this task: `iced::Radius` does not exist (use
the `From<f32>` on `iced::border::Radius`); `iced::widget::Element` does not
exist (use `iced::Element`); `Space` uses `.width()`/`.height()`, not
`with_width`/`with_height`; there is no `iced::scrollable::Axis`; and
`Widget::type_name()` is gone in 0.14, so widget-identity assertions had to
go.

**Not ported, with reasons.**

1. **GFM column alignment** (`|:---|`, `|---:|`). The parser discards
   pulldown-cmark's alignment payload at `src/app/markdown/parser.rs:370`
   (`Event::Start(Tag::Table(_))`), and `RenderEvent::Table` carries only
   `Vec<Vec<Vec<InlineElem>>>`. Neither build has ever aligned columns; the
   iced port cannot invent alignment the model does not carry. Adding it means
   a model change, which this task is explicitly barred from making.
2. **Per-link hover tooltips** ("Note: ..." on hover). egui attached a
   tooltip per `Response`. iced's `Rich` is a *single* widget per paragraph,
   so per-span hover geometry is not available; a tooltip would need a
   separate overlay per link plus hit-testing. Deferred to the overlay work
   (TASK-017b), not the render port.
3. **Viewport culling / estimated-space insertion.** egui skipped
   off-screen events and inserted estimated height
   (`src/app/ui/render/mod.rs:174-227`). iced's `scrollable` virtualises
   nothing, and culling needs live scroll offsets the pure view deliberately
   does not own. The port renders the full document; if long-document cost
   shows up, culling belongs in the runtime layer.
4. **Scroll-to-target execution.** The pure layer computes *which* block
   matched (`RenderBlock::scroll_to_me`, first match only, matching egui's
   clear-after-first-hit behaviour) but cannot scroll: that needs a `Task`
   plus a stable `scrollable` `Id`, i.e. reducer work in TASK-022/023.
5. **egui's `ui.available_width()`-driven reflow of the code panel.** The
   iced code block uses `Length::Fill` plus `Wrapping::WordOrGlyph`; egui's
   horizontal-scroll-on-overflow for long code lines is not reproduced (iced
   `SelectableText`/`text` wrapping semantics differ). Long lines wrap rather
   than scroll.

**Gate results (this pass).** `cargo check -q` PASS (exit 0, clean,
workspace); `cargo clippy -p fastmd --bins --lib -- -D warnings` PASS
(exit 0); `cargo fmt` run, `cargo fmt --check` PASS workspace-wide (exit 0);
`cargo nextest run -p fastmd` PASS (**1660 run / 1660 passed / 1 skipped**),
of which **163** are this task's new sidecar tests (32 `table_width` + 30
`render::mod` + 24 `render::table` + 22 `render::inline` + 17
`render::yaml` + 12 `render::code` + 10 `render::heading` + 16
`ui_iced::message`); `cargo doc --no-deps --quiet` PASS (exit 0, no
warnings after fixing four intra-doc links in the new module headers).

**Needs the orchestrator's eyes.**

1. **`measure_cell` was reverted to the exact egui behaviour mid-task.** An
   earlier draft "fixed" the walker's `else if ends_ws` branch (a single-part
   run ending in whitespace closes the pending token and measures it with the
   *previous* run's font, so a lone code span is measured at proportional
   size). That is a latent bug in the egui adapter, but changing it shifts
   every solved column width. The port now matches egui exactly and the
   quirk is pinned by test. If the reviewer wants the bug fixed, it should be
   fixed in **both** adapters in one deliberate change, not smuggled in here.
2. **The heuristic measurer is not the real shaper.** Column widths in this
   pass are proportional estimates (0.5 / 0.6 em per char). They are
   deterministic and test-exact, but real iced shaping will produce different
   numbers. The injection point is designed so TASK-022/023 swaps in a
   renderer-backed closure with no code change here - but until then, do not
   read the pixel widths in tests as production widths.
3. **`RenderMessage` is a third command vocabulary.** `UserCommand` (bus),
   `EditorMessage`/`AgentMessage`/... (iced) and now `RenderMessage`. The
   task brief required a channel that carries document + index + value, and
   none existed. If the orchestrator prefers folding this into an existing
   channel, `UserCommand::ApplyTaskToggles` would need its target widened
   from the agent transcript to an arbitrary document - a bus-level change
   with its own blast radius.
4. **`strings.rs` regression caught and fixed by the gate.** An earlier edit
   had re-wrapped `PDF_BACKED_ERROR` into a form `rustfmt` rejected;
   `cargo fmt` restored it. The tracked diff for `strings.rs` is now the two
   new list-marker constants only (+14 lines).
5. **Table stripe parity depends on `TableRowData` carrying its own index.**
   `table::column`'s view closure receives only the row *value*, not its
   position, so the row index is stored in the data. Slightly awkward, but it
   is what keeps the alternating-row shading identical to egui's.
6. **No `SPEC.md` changes** were made ([RUST-043]); behaviour is mapped from
   existing requirements only.

### TASK-016 verification log (bottom command band + background-process logs - 2026-10-18)

**Scope.** Two egui modules ported to the iced pure-view pattern.

* `src/app/ui/panels/bottom.rs` (306 lines) - the **bottom command band**:
  prompt prefix, active-file context, the command input and the agent Stop
  button. Maps to UI-001's "Bottom Pane (command prompt)"; the band's
  message vocabulary is the existing `AgentMessage` channel
  (`message.rs:362-388`, grounded in `ui/panels/bottom.rs:226-294`).
* `src/app/ui/background_logs.rs` (336 lines) - the **Background Processes
  viewer** (UI-047): search field, category combo, auto-scroll checkbox,
  Clear button and the log list.

Both are pure: no file I/O, no bus access, no ring-buffer mutation, no
wiring into the pilot binary or the update loop (TASK-022/023). The
background subsystem was **not modified** - REQ-460..465 are untouched
(`git status` shows no change under `src/app/background/`).

**Ring-buffer source (read-only).** The viewer reads the same type the egui
build read: `BackgroundLogs` (`src/app/background/logs.rs:262`) - a
`VecDeque<BackgroundLogEntry>` (`logs.rs:263`) bounded by
`MAX_LOG_ENTRIES = 10_000` (`logs.rs:14`), shared as
`SharedBackgroundLogs = Arc<Mutex<BackgroundLogs>>` (`logs.rs:339`) and
reached through `AppOrchestrator::background_manager`
(`src/app/orchestrator.rs:48`). The entry and category types live in
`src/app/bus/events/messages.rs` (`BackgroundLogEntry` `:63-68`,
`LogCategory` `:30-39`, `Display` `:41-54`) after the layering-inversion
fix, re-exported by `background/models.rs:19`. The view-model calls only
`get_logs()` (`logs.rs:301-303`); `push_log` (`:293-298`) and `clear_logs`
(`:306-308`) are never called from the UI layer.

**Files.**

| File | Role |
| --- | --- |
| `src/app/ui_iced/logs.rs` | `CategoryFilterOption`/`category_filter_options`, `timestamp_text`/`category_tag`, `category_color`/`timestamp_color`/`message_color`, `ScrollSnapshot`/`scroll_snapshot`/`should_follow`, `BackgroundLogsViewModel` (`from_logs`/`from_app`/`unavailable`), `background_logs_view`, `log_table`, `LOG_SCROLLABLE_ID` (685 lines) |
| `src/app/ui_iced/logs_tests.rs` | 45 sidecar tests |
| `src/app/ui_iced/panels/bottom.rs` | `BottomPanelViewModel` (`from_app`), `submit_commands`, `stop_enabled`, `bottom_panel` + re-exports of the egui band's pure helpers (279 lines) |
| `src/app/ui_iced/panels/bottom_tests.rs` | 22 sidecar tests |
| `src/app/ui_iced/message.rs` | new `LogsMessage::ScrollReported { offset_y, viewport_height, content_height }` variant |
| `src/app/ui_iced/panels/mod.rs` | `pub mod bottom;` + module-doc entry |
| `src/app/ui_iced/mod.rs` | `pub mod logs;` + module-doc entry |

**Reused vs new.** Reused unchanged: `ui::background_logs::{is_log_visible,
filter_logs}` (re-exported, `ui/background_logs.rs:15-46`),
`ui::panels::bottom::{compute_prompt_prefix, compute_file_context,
format_models_list, parse_command_intent, clear_command_input_command}`
(`:44-142`), the `BackgroundLogs` ring buffer, `LogCategory::Display`,
`strings::{SEARCH_LABEL, CATEGORY_LABEL, LOG_CATEGORY_ALL, LOG_CATEGORY_INDEXER,
LOG_CATEGORY_WATCHER, LOG_CATEGORY_PDF_CONVERTER, LOG_CATEGORY_IMAGE_VISION,
LOG_CATEGORY_LLM_TOOLS, AUTO_SCROLL_CHECKBOX, CLEAR_BUTTON,
BACKGROUND_MGR_ACCESS_ERROR, COMMAND_INPUT_HINT, STOP_AGENT_BUTTON}` and the
`Palette` tokens `log_timestamp`, `log_category`, `text`, `text_strong`,
`text_muted`, `error` - no new colour literal ([GUD-003]). New: the two
view-models, the auto-scroll rule, the category pick-list model,
`submit_commands` (extracted from egui's inline submit branch) and one
message variant.

**View-model shapes.**

```text
BottomPanelViewModel {
    command_input: String,                // AgentPanelState::command_input
    agent_running: bool,                 // agent().state().running
    prompt_prefix: String,               // compute_prompt_prefix(prompt_dir, libraries)
    file_context: Option<(String, f32)>, // compute_file_context(active_file, prefix)
}

BackgroundLogsViewModel {
    visible: bool,                       // show_background_logs
    entries: Vec<BackgroundLogEntry>,    // filtered, ring order (old -> new)
    total_entries: usize,                // pre-filter count (FMD-067: report the loss)
    search_text: String,
    category_filter: Option<LogCategory>,
    category_options: Vec<CategoryFilterOption>,  // owned: the pick-list borrow lives here
    auto_scroll: bool,
    access_error: bool,                  // poisoned lock -> BACKGROUND_MGR_ACCESS_ERROR
}
```

`BackgroundLogsViewModel` is `Clone + Debug` only: `BackgroundLogEntry` is a
bus type that derives no `PartialEq` and REQ-460..465 keep it that way, so
the snapshot cannot derive it either - the sidecar asserts field by field.
`category_options` is a field rather than a call because `pick_list` needs
`V: Borrow<T> + 'a` (`pick_list.rs:179-195`): the selected option must be
borrowed out of something that outlives the element, which is the same
reason `TopBarViewModel::tag_options` exists ([TASK-011]).

**Verified iced 0.14 APIs (local registry sources).**

| API | Citation |
| --- | --- |
| `table(columns, rows)` / `column(header, view)` | `iced_widget-0.14.2/src/table.rs:18-28` / `:34-49` |
| all-`Shrink` forces the **first** column to `Fill` (why the message column is `Fill` explicitly) | `table.rs:129-133` |
| `Column::width` | `table.rs:650-653` |
| `table::Style` carries only `separator_x`/`separator_y` | `table.rs:676-681` |
| `scrollable` builder `.id` / `.width` / `.height` / `.on_scroll` | `scrollable.rs:144` / `:150` / `:156` / `:164-168` |
| `scrollable::Viewport::{absolute_offset, bounds, content_bounds}` | `scrollable.rs:1739-1748` / `:1775-1777` / `:1780-1782` |
| `pick_list` bounds: `T: ToString + PartialEq + Clone`, `L: Borrow<[T]>`, `V: Borrow<T> + 'a` | `pick_list.rs:179-215` |
| `checkbox(is_checked)` (label/toggle are builders) | `helpers.rs:1281-1289`, `checkbox.rs:145` (`.label`), `:155` (`.on_toggle`) |
| `text_input(...).on_submit` fires on Enter with **no modifier check** | `text_input.rs:194`, `:1050-1056` |
| `widget::Id: From<&'static str>` (the stable scroll target) | `iced_core-0.14.0/src/widget/id.rs:26-30` |
| reducer-side `operation::{snap_to, snap_to_end, scroll_to, scroll_by} -> Task<T>` | `iced_runtime-0.14.0/src/widget/operation.rs:12-47` |
| `AbsoluteOffset<T>` / `RelativeOffset::{START, END}` | `iced_core-0.14.0/src/widget/operation/scrollable.rs:120-153` |

**Auto-scroll intent model.** egui got the behaviour declaratively from
`ScrollArea::stick_to_bottom`
(`egui-0.36.1/src/containers/scroll_area.rs:648-658`), which in the frame
loop is:

```rust
if stick_to_end[d] && state.scroll_stuck_to_end[d] && !had_explicit_scroll_adjustment[d] {
    state.offset[d] = content_size[d] - inner_rect.size()[d];   // :1282-1284
}
// "stuck" is recomputed as:
// offset == available_offset  ||  (stick_to_end && available_offset < 0)   // :1543-1549
```

iced has no such flag, so the rule is split across the layers:

1. **Report** - the view wires `Scrollable::on_scroll` and emits
   `Message::Logs(ScrollReported { offset_y, viewport_height, content_height })`
   (the reason the `Logs` channel grew a variant).
2. **Decide** - `logs::should_follow(auto_scroll, Option<ScrollSnapshot>)`
   is `auto_scroll && viewport.is_at_bottom()`, where `is_at_bottom` is
   `fits() || offset_y >= max_offset() - BOTTOM_STICK_TOLERANCE`. The
   `fits()` arm is egui's `available_offset < 0` case; the 1 pt tolerance
   replaces egui's exact `f32` equality because iced clamps and rounds the
   offset before reporting it. `None` (no report yet) is **false** - the
   view must not invent geometry it was never given.
3. **Execute** - reducer work in TASK-022/023:
   `operation::scroll_to(LOG_SCROLLABLE_ID.into(), AbsoluteOffset { x: 0.0, y: snapshot.max_offset() })`,
   or `operation::snap_to_end(...)` for the same result without computing
   the offset (`operation.rs:23-39`). The pure layer marks the intent only;
   it never scrolls.

Consequences pinned by test: at-bottom + preference on follows; scrolled-up
never jumps back down; preference off never follows; content shorter than
the viewport counts as at the bottom; the tolerance absorbs a sub-point
shortfall and nothing more; `max_offset` never goes negative.

**Not ported, with reasons.**

1. **The IME Enter workaround** (`ui/panels/bottom.rs:13-40`,
   `is_enter_pressed`). egui needed it because `egui-winit` drops the raw
   `Key::Enter` when an IME claims the key (`VK_PROCESSKEY`) and only an
   `Event::Ime(Commit("\n"))` reaches the app. iced 0.14 never surfaces IME
   commits to the application - `text_input` publishes `on_submit` from its
   own key handling (`text_input.rs:1050-1056`) - so there is no second
   delivery path to reconcile and no equivalent helper to write.
2. **Shift+Enter = newline.** The egui band submitted on Enter *unless*
   Shift was held (`bottom.rs:238-243`). iced's `text_input` fires
   `on_submit` on Enter with no modifier check and the message is fixed, so
   the view cannot branch on Shift. `text_editor` is no better: its
   `Action::Edit(Edit::Enter)` carries no modifiers either. Flagged for
   TASK-018/022 - either accept Enter-always-submits or change
   `strings::COMMAND_INPUT_HINT`, which currently promises Shift+Enter.
3. **Multiline command field.** egui used `TextEdit::multiline`
   (`bottom.rs:228-235`); the port uses `text_input`, which is single-line.
   A multiline band needs `text_editor` plus an owned
   `text_editor::Content`/`State` in the runtime model (DEC-007,
   TASK-014) - the message vocabulary (`InputChanged`/`Submit`) is
   identical either way, so this is a widget swap, not a model change.
4. **The Tier-4 `on_click("send")` sink** (`bottom.rs:148-159`) is
   `egui_kittest` plumbing; CON-003 rules it out. The iced equivalent is the
   `iced_test::simulator` tier (TASK-025/026).
5. **egui's hand-computed right-alignment** (the 100 pt Stop-button rect and
   the `available_rect_before_wrap` bookkeeping, `bottom.rs:187-211`) is not
   ported: iced's `Row` plus a `Fill` container place it, so the magic
   numbers disappear rather than get translated.
6. **The egui window chrome** (resizable/collapsible 600x400 modal,
   `background_logs.rs:66-70`) is out of scope - the port is the body only,
   mounted by the overlay host (TASK-017) / the app view (TASK-023).
7. **UI-047's "open automatically when the first background task starts"**
   is **not implemented in the egui build either** - visibility is a plain
   `show_background_logs` flag toggled from the top menu
   (`UserCommand::ToggleBackgroundLogs`, ported in TASK-011). The port
   carries the flag through the view-model unchanged; the auto-open trigger
   would be reducer behaviour, and the gap is pre-existing rather than a
   migration regression ([RUST-041]).

**Gate results (this pass).** `cargo check -q` PASS (exit 0, clean,
workspace); `cargo clippy -p fastmd --bins --lib -- -D warnings` PASS
(exit 0); `cargo fmt` run, `cargo fmt --check` PASS (exit 0);
`cargo nextest run -p fastmd --status-level fail --show-progress none` PASS
(**1727 run / 1727 passed / 1 skipped**), of which **67** are this task's
new sidecar tests (45 `ui_iced::logs` + 22 `ui_iced::panels::bottom`);
`cargo doc --no-deps --quiet` PASS (exit 0, no warnings).

**Needs the orchestrator's eyes.**

1. **The task brief's "level colours" do not exist.** `BackgroundLogEntry`
   has no severity field (`messages.rs:63-68`) and the egui viewer painted
   every row with the same two tokens (`background_logs.rs:138-151`).
   `category_color` is therefore an exhaustive-but-uniform map to
   `palette.log_category`, kept exhaustive so a new `LogCategory` variant is
   a compile error rather than a silent fall-through. If per-severity colour
   is actually wanted, the ring-buffer entry needs a level field - a
   REQ-460..465 change this task was barred from making.
2. **`Print` and `Batch` have no filter chip.** The egui combo offered only
   All / Indexer / Watcher / PdfConverter / ImageVision / LlmTools
   (`background_logs.rs:84-106`) even though `parse_log_category` routes
   `print` and `batch` targets into two more variants (`logs.rs:51-54`), and
   `strings.rs` has no `LOG_CATEGORY_PRINT`/`LOG_CATEGORY_BATCH`. The port
   keeps parity and documents the gap (pinned by
   `category_filter_option_is_none_for_an_unoffered_category`). Widening it
   is a product decision needing two new string constants.
3. **`LogsMessage` grew a variant.** `ScrollReported` is a runtime geometry
   report, not user intent - the same class `LayoutMessage::CursorMoved`
   carries. It is required because iced exposes scroll position only through
   `on_scroll`; without it the auto-scroll rule can never be evaluated. If
   the orchestrator would rather keep [TASK-021]'s channel frozen, the
   alternative is a `Layout`-channel variant, which is a worse fit.
4. **Poisoned-lock handling is stricter than egui.** The egui viewer called
   `.lock().unwrap()` and would take the UI down
   (`background_logs.rs:49-64`, `:157-161`); `from_app` degrades to
   `unavailable()` instead ([FMD-064]). `unavailable()` reports
   `visible: false` deliberately - an unreadable buffer must not force a
   closed window open - so the reducer decides whether to surface the error
   banner. Flagged in case the reviewer prefers fail-open.
5. **`--all-targets` clippy is not clean, and was not before this task.** The
   gate command in the brief (`--bins --lib`) is green. `cargo clippy -p
   fastmd --all-targets -- -D warnings` reports **12 pre-existing** test-only
   lints in other tasks' files (`markdown/link_scanner_tests.rs`,
   `ui/batch_dialog_tests.rs`, `config/config_tests.rs`,
   `ui_iced/panels/left_tests.rs`, `ui_iced/render/code_tests.rs`,
   `ui_iced/message_tests.rs`: `field_reassign_with_default`,
   `let_unit_value`, `assertions_on_constants`, `redundant_closure`,
   `cloned_ref_to_slice_refs`, `useless_vec`). My own files are clean under
   the same lint set. Worth a separate chore sweep rather than folding into
   this task.
6. **No `SPEC.md` changes** were made ([RUST-043]); behaviour is mapped from
   existing requirements only.


### TASK-017b1 dialog verification log (small individual dialogs — first half — 2026-10-18)

**Scope delivered: the individual name-entry / move / delete-confirmation
dialogs only.** The big windows (about / batch / tools / agent-debug /
attributions) remain a later chunk. New files: `src/app/ui_iced/dialogs.rs`
+ `dialogs_tests.rs` sidecar (51 tests), `pub mod dialogs;` + module-doc
bullet in `src/app/ui_iced/mod.rs`, a new `Dialog` message channel in
`src/app/ui_iced/message.rs` (+1 `message_tests.rs` test), one new
primitive `modal_at` in `src/app/ui_iced/overlay.rs` (+1 `overlay_tests.rs`
test), and six new string constants / one formatting helper in
`src/app/ui/strings.rs`.

**`DialogKind` / `DialogState` shape.** The per-dialog `bool` fields of
`ui::dialogs::Dialogs` (`ui/dialogs.rs:25-63`) collapse into one
`Option<DialogKind>` carrying the reducer's execution targets
([FMD-068]):

```rust
pub enum DialogKind {
    CreateDirectory { parent: PathBuf },   // UI-011  (egui create_dir_parent)
    CreateDocument  { parent: PathBuf },   // UI-015  (egui create_document_parent)
    Rename          { path:   PathBuf },   // UI-012  (egui file_to_rename)
    Move            { path:   PathBuf },   // UI-013  (egui file_to_move)
    DeleteConfirm   { path:   PathBuf },   // NEW — see drift note below
}
pub struct DialogState {
    open: Option<DialogKind>,
    pending_name: String,            // name-entry field (egui *_name fields)
    error: Option<String>,           // NEW visible error line (egui had none)
    destinations: Vec<PathBuf>,      // move picker list, reducer-supplied
    selected_destination: Option<PathBuf>,  // egui selected_move_folder
}
// transitions: open / open_move / close->bool / cancel / set_name /
//              set_error / clear_error / select_destination / can_confirm /
//              submit -> SubmitOutcome
pub enum SubmitOutcome {
    Completed { kind: DialogKind, name: String, destination: Option<PathBuf> },
    Invalid   { message: String },
}
```

`open()` **replaces** whatever was open — one dialog at a time. egui kept
independent `bool` flags and did *not* enforce mutual exclusion
(`ui/dialogs.rs`); the iced port consolidates because the overlay contract
(`OverlaySlot`) allows exactly one blocking modal, matching
`OverlaySlot::open_modal`. `submit()` returns the self-contained payload
so the reducer can publish the `Confirm*` command without re-reading
state the close has already cleared.

**Validation rules — found in TWO egui layers, merged into one pure
`validate_name`.** Verified against the egui build:

| Layer | Location | Rule |
| --- | --- | --- |
| Dialog (silent) | `ui/modals.rs:176/218/306` | `!name.trim().is_empty()` — empty/whitespace-only just closes without publishing |
| Dialog (move) | `ui/modals.rs:138-148` | `selected_move_folder.is_some()` gates the publish |
| Executor (silent) | `command_executor.rs:309/338/365` → `utils::path::is_safe_basename` (`utils/path.rs:13-27`) | non-empty, no `/`, no `\`, no `..` component; bad name logs a warn and **skips** the operation |

`validate_name(kind, name)` merges the dialog's non-blank check with the
executor's `is_safe_basename` rule and surfaces the failure as a visible
error line (`DIALOG_NAME_EMPTY_ERROR` / `DIALOG_NAME_UNSAFE_ERROR`) — an
**improvement over egui's silent close**. Non-name kinds (Move,
DeleteConfirm) return `Ok` from `validate_name`; their gating lives in
`can_confirm` (move needs a picked destination, delete is always
confirmable). The trimmed name is what `submit` returns, matching egui's
`name.trim().to_string()` publish.

**Anchor model.** The egui name-entry window captures the pointer once per
open session and pins with `fixed_pos`, falling back to the viewport
centre (`ui/modals.rs:26-45`). The iced port keeps the shape: the reducer
captures the tracked `LayoutMessage::CursorMoved` point at open time and
passes it to `dialog_card(state, anchor: Option<Point>, viewport)`, which
resolves the top-left with `resolve_dialog_anchor` — the cursor point
clamped by `overlay::clamp_anchor`, or the viewport centre when `anchor`
is `None` (truly centred, not egui's top-left-at-centre quirk). The
card's size is deterministic (`dialog_size`) because every row is a
fixed-height widget and the error row is always allocated, so the clamp
cannot drift from the real layout — the same argument `menu_size` makes.
The card mounts via the new `overlay::modal_at` (same scrim + card chrome
as `modal`, explicit top-left placement instead of centring).

**Connection to the tree deferred-intents.** The left panel's context-menu
rows already emit the opening intents as `UserCommand`s
(`panels/left.rs:331-350` directory / `:416-427` file: `Rename`, `Move`,
`CreateDirectory { parent }`, `CreateDocument { parent }`, `Delete`).
The [TASK-022/023] reducer maps each to `DialogState::open(kind)` (or
`open_move` with the destination snapshot for Move). The dialog card then
emits `Message::Dialog(...)` for every interaction; the reducer runs
`submit()` and publishes the matching `UserCommand::Confirm*`
(`ConfirmCreateDirectory` / `ConfirmCreateDocument` / `ConfirmRename` /
`ConfirmMove`) or `UserCommand::Delete` only on
`SubmitOutcome::Completed`. The view never touches the bus ([FMD-065]).

**Message channel.** New `Message::Dialog(DialogMessage)` with
`NameChanged { text }` / `Submitted` / `Cancelled` / `Dismissed` /
`DestinationSelected { path }`. `Cancelled` (explicit) is kept distinct
from `Dismissed` (scrim/Escape) so the reducer can tell them apart, the
same distinction `OverlaySlot::close` reports. None of these is a
ready-to-publish `UserCommand` — validation runs first.

**Not ported / new behaviour, with reasons.**

1. **The delete confirmation is NEW.** The egui build has **no** delete
   confirmation — the tree's Delete row publishes `UserCommand::Delete`
   directly (`ui/tree/render.rs:63/112/270`); `modals.rs`'s module
   docstring names a "delete confirmation" that does not exist in the
   code. `DialogKind::DeleteConfirm` is added because the migration brief
   requires a confirm step; whether the reducer routes `Delete` through it
   is a [TASK-022/023] decision ([RUST-041] drift note).
2. **The move dialog is a destination picker, not a name-entry field** —
   egui parity (`ui/modals.rs:95-126`): a scrollable list of
   library-root + all-dir + file-parent folders with a `selectable_label`
   per row. The candidate list is data the pure view cannot build (it came
   from `ContentLibrary` + `FileEventProcessor` at render time), so the
   reducer snapshots it and hands it in via `open_move`. The egui
   `library_display_label` nicety is **not** reproduced — rows show the
   raw `to_string_lossy` path; adding library labels is a reducer-side
   concern flagged for the orchestrator.
3. **The visible error line is new** (egui failed silently). The error row
   is always allocated so the card's widget shape is stable across the
   error/no-error transition (egui conditional-rendering rule).
4. **`text_input` is single-line** (egui `text_edit_singleline` parity);
   no `.height()` is applied — `DIALOG_INPUT_HEIGHT` is reserved geometry
   for the size formula only.

**Gate results (this pass).** `cargo check --quiet` PASS (workspace, clean);
`cargo clippy -p fastmd --bins --lib -- -D warnings` PASS; `cargo fmt`
run, `cargo fmt --check` PASS; `cargo nextest run -p fastmd --status-level
fail --show-progress none` PASS (**1780 run / 1780 passed / 1 skipped**),
of which **53** are this task's new tests (51 `ui_iced::dialogs` + 1
`ui_iced::message` + 1 `ui_iced::overlay`); `cargo doc --no-deps --quiet
-p fastmd` PASS (no warnings).

**Needs the orchestrator's eyes.**

1. **`overlay.rs` grew a primitive (`modal_at`).** The brief said reuse
   `modal`; `modal` centres and cannot place a card at a clamped cursor
   point without duplicating the scrim + card chrome. `modal_at` is the
   anchored sibling (same `card_style`/`scrim_blocker`), so the dialog
   stays DRY. If the orchestrator prefers `overlay.rs` frozen, the
   alternative is duplicating the card chrome in `dialogs.rs` — worse.
2. **One-at-a-time is stricter than egui.** egui's independent flags could
   theoretically hold two dialogs open; the port forbids it (overlay
   contract). Confirm this is the intended consolidation.
3. **Delete-confirm routing is a reducer decision.** The dialog exists but
   nothing forces `UserCommand::Delete` through it. If the product wants
   the confirm step, [TASK-022/023] must intercept `Delete`; if not, the
   `DeleteConfirm` kind stays unused. Flagged so the gap is a decision,
   not an oversight.
4. **Move destination labels.** Raw path strings vs egui's
   `library_display_label` — a product-parity gap needing reducer-side
   label data if the friendlier labels are wanted.
5. **No `SPEC.md` changes** were made ([RUST-043]); behaviour is mapped
   from existing requirements only.


### TASK-017b2 about/attributions verification log (About window + credits window — 2026-10-18)

**Scope delivered: the About window and the third-party attributions
window only.** `batch_dialog` / `tools_dialog` / `agent_debug_window`
remain a later chunk. New files: `src/app/ui_iced/about.rs` +
`about_tests.rs` sidecar (24 tests), `src/app/ui_iced/attributions.rs` +
`attributions_tests.rs` sidecar (16 tests), two `pub mod` lines plus
module-doc bullets in `src/app/ui_iced/mod.rs`, a new `About` message
channel in `src/app/ui_iced/message.rs` (+2 `message_tests.rs` tests) and
five new string constants plus one formatting helper in
`src/app/ui/strings.rs`.

**Two modules, not one.** The brief allowed either. Two were chosen because
the egui originals are two files with two different jobs — a dialog
(`ui/about_dialog.rs`) and a data catalog (`ui/attributions.rs`) — and
the iced port keeps that boundary: `about.rs` owns identity, build
metadata, the license and the window-state model; `attributions.rs` owns
the credits table. Each stays well under the [RUST-053] size limit and
neither needs the other's internals (only `close_button` / spacing
constants are shared).

**Attribution / license data source — reused verbatim, unchanged.**
`crate::ui::attributions::DIRECT_DEPENDENCIES` (`src/app/ui/attributions.rs:24`),
the hand-curated `pub const &[Attribution]` of 57 direct third-party
crates across all four workspace members, alphabetically sorted and
duplicate-free. It is **not** `cargo-about` output, **not** a generated
file and there is **no** runtime shell-out in either build — the data is
compiled in, so the iced view reads it with zero I/O
(`AttributionsViewModel::from_catalog()` snapshots it with `to_vec()`).

Two honest findings about that catalog:

1. **`Attribution` has no license and no version field** — only
   `name`, `authors`, `github_url`. The brief asked for "crate name +
   license + (version/homepage if present)"; the "if present" is *not*
   present. The credits window therefore shows **Crate / Authors /
   Repository**, exactly the three columns the egui header already names
   (`strings::ABOUT_COL_CRATE` / `_AUTHORS` / `_REPO`,
   `src/app/ui/about_dialog.rs:202-206`). The only license text in the
   whole app is FastMD's own MIT license, embedded via
   `include_str!("../../../LICENSE")` and re-flowed by the **reused**
   `license_display_text()` helper in `src/app/ui/about_dialog.rs:58-64`
   (hard wraps collapsed into flowing paragraphs, blank-line paragraph
   breaks preserved). Adding real per-crate license data is a new
   compile-time pipeline (e.g. a generated `cargo-about` file) — a
   data-source decision for the orchestrator, not something this port
   invents.
2. **The catalog is curated, not derived.** `egui_extras` is listed at
   `src/app/ui/attributions.rs:76` purely to credit its authors even
   though it is not a real consumer. That curation is preserved exactly;
   `attributions_tests.rs` asserts snapshot-equality with the catalog
   rather than re-deriving it.

**Version source.** `crate::ui::about_dialog::APP_VERSION`
(`env!("CARGO_PKG_VERSION")`, currently `0.2.0`) — reused, not
re-declared. **Build metadata** (branch / full hash / short hash / build
date) is read with `env!` in the new module from the same root `build.rs`
`cargo:rustc-env` vars the egui dialog reads; the egui copies of those
consts are module-private, so re-reading the shared env vars is the
non-invasive reuse (no edit to the egui file).

**View-model shapes.**

```rust
// ui_iced/about.rs — identity + build metadata + license
pub struct AboutViewModel {
    pub app_name: &'static str,          // strings::ABOUT_APP_NAME
    pub version: &'static str,          // CARGO_PKG_VERSION
    pub description: &'static str,      // strings::ABOUT_DESCRIPTION (manifest desc)
    pub copyright: &'static str,        // strings::ABOUT_COPYRIGHT
    pub branch: &'static str,           // env!("BUILD_BRANCH")
    pub commit_hash: &'static str,      // env!("BUILD_COMMIT_HASH")
    pub commit_short_hash: &'static str,// env!("BUILD_COMMIT_SHORT_HASH")
    pub build_date: &'static str,       // env!("BUILD_DATE")
    pub license_text: String,           // reflowed MIT body
    pub attribution_count: usize,       // DIRECT_DEPENDENCIES.len()
}
// AboutViewModel::from_build() -> Self          — production snapshot
// .is_build_metadata_unknown() -> bool          — "unknown" sentinel (FR-007)
// .version_line() / .commit_tooltip() / .attributions_summary() -> String
// .view() -> Element<Message>                   — == about_card(&vm)

pub fn about_card(&AboutViewModel) -> Element<Message>   // banner, title,
    // version, description, Branch/Commit/Built row, license scroll
    // (200px cap, egui parity), attributions teaser + button, Close

// ui_iced/attributions.rs — the credits list
pub struct AttributionsViewModel { entries: Vec<Attribution> }
// .from_catalog() / .with_entries(Vec<Attribution>)
// .entries() -> &[Attribution]  .count()  .is_empty()  .summary() -> String
pub fn attributions_view(&AttributionsViewModel) -> Element<Message>  // title,
    // count line, scrollable 3-column table (340px cap, egui parity), Close
pub fn attribution_table(&[Attribution]) -> Element<Message>          // iced table
```

**Banner rendering decision — glyph, not the PNG.** Verified against
`iced-0.14.0/Cargo.toml`: the `image` feature exists (:83, pulls
`iced_widget/image`) and `svg` exists (:99, `iced_widget/svg`), but
**neither is enabled** — `Cargo.toml:55` lists only `wgpu`, `tiny-skia`,
`fira-sans`, `advanced-shaping` (the DEC-011 minimal-feature decision,
`doc/planning/iced-migration-map.md:169-173`). So `iced::widget::image`
and `iced::widget::svg` **do not exist in this build** and the
compiled-in `assets/icon-32.png`
(`crate::utils::app_icon::APP_ICON_PNG`) has no widget to live in. The
banner is the Phosphor bolt (`strings::ICON_LIGHTNING`) drawn with
`panels::top::phosphor_font()` — the identical stand-in [TASK-011] uses
for the top-bar "Document + Bolt" mark (`ui_iced/panels/top.rs:596`).
`banner_glyph()` is a tested constant accessor so swapping in a real image
later is a one-function change once the `image` feature decision is made.

**Message-channel choice — a new `Message::About` channel, deliberately
NOT the `Dialog` channel.**

```rust
pub enum AboutMessage {
    AttributionsRequested,        // About card's credits button
    CopyCommitHashRequested,     // commit cell click -> reducer writes clipboard
    RepositoryOpened { url: String }, // credits row -> reducer opens URL
    CloseRequested,              // Close button / scrim / Escape
}
```

`DialogMessage`'s vocabulary is name-entry shaped (`NameChanged`,
`Submitted`, `DestinationSelected`) and drives `DialogState`'s
validation; none of it applies to an About or credits window, and merging
them would put two unrelated state models behind one message type that
the reducer would have to discriminate on anyway. Opening the About
window is **not** in this channel: that is user intent and already rides
`Message::Command` as `UserCommand::OpenAboutDialog`
(`src/app/bus/events/user_command.rs:68`), which the iced hamburger
already emits (`ui_iced/panels/top.rs:439`). Only what the rendered
cards do to their own visibility, plus the two side effects egui
performed inside its click handler (clipboard, browser), live here — the
same deferred-intent shape [TASK-015] established for
`RenderMessage::LinkActivated`.

**Window state — pure, one window at a time.**

```rust
pub enum AboutWindow { About, Attributions }        // .title() -> &'static str
pub struct AboutWindowState { open: Option<AboutWindow> }
// .new() .is_open() .is_about() .is_attributions() .window()
// .open(AboutWindow) / .open_about() / .open_attributions() -> bool
// .close() -> bool                                 // "was anything consumed"
// .apply(AboutMessage) -> bool                     // did the VISIBLE WINDOW change
```

`open()` **replaces** whatever was showing — the same one-at-a-time rule
`DialogState` and `OverlaySlot::open_modal` apply, so a reducer that
wants the single-blocking-overlay invariant can mirror this state into
`OverlaySlot<AboutWindow>`. `apply()` returns whether the *visible
window* changed; `CopyCommitHashRequested` and `RepositoryOpened` return
`false` while still requiring the reducer's action, which is documented
on the method.

**Deviations from the egui dialog (deliberate, [RUST-041] notes).**

1. **Attributions moved out of the About dialog.** egui renders the whole
   57-row catalog inline (`src/app/ui/about_dialog.rs:200-220`); the
   brief asked for a dedicated credits window, so the About card keeps
   only the count line plus a `View third-party attributions...` button.
   This is a visible product-parity change, not a like-for-like port.
2. **A version row is new** — the egui dialog never displayed
   `CARGO_PKG_VERSION` even though it owns the constant.
3. **Copy-to-clipboard is deferred intent.** egui called `ctx.copy_text`
   inside the click handler (`src/app/ui/about_dialog.rs:68-77`); iced
   exposes the clipboard only to `update`
   (`iced_core-0.14.0/src/clipboard.rs`), so the commit cell emits
   `CopyCommitHashRequested` and the reducer writes it. `MouseArea` has
   no `on_press_maybe` (`iced_widget-0.14.2/src/mouse_area.rs:38`), so
   `commit_copy_press(unknown) -> Option<Message>` resolves the handler
   outside the widget and the caller attaches it only when `Some` — the
   widget count never changes (AGENTS §6).
4. **No hover tooltip.** iced 0.14 has no built-in tooltip widget and
   DEC-005 rules out `iced_aw`, so the full-hash reveal cannot follow the
   egui shape. `AboutViewModel::commit_tooltip()` still produces the same
   text for the reducer to surface another way.
5. **`iced::widget::table` instead of `ui.horizontal` rows.** egui laid
   the rows out as plain horizontal triples; the port uses the real table
   widget so the three columns the egui header already names actually
   align, per the migration map's `egui::Table` → `iced::widget::table`
   mapping (A5). Column widths follow the rule `logs::log_table` learned:
   the long URL column carries `Length::Fill` explicitly because iced
   promotes the first all-`Shrink` column to `Fill`
   (`iced_widget-0.14.2/src/table.rs:129-133`).
6. **Empty states never change the widget tree.** An empty catalog renders
   the same table (header only) with the empty-state text folded into the
   summary line; an unknown build renders the same commit cell with no
   press handler.

**Gate results (this pass).** `cargo check --quiet` PASS (workspace,
clean); `cargo clippy -p fastmd --bins --lib -- -D warnings` PASS;
`cargo fmt` run, `cargo fmt --check` PASS; `cargo nextest run -p fastmd
--status-level fail --show-progress none` PASS (**1825 run / 1825 passed
/ 1 skipped**), of which **45** are this task's new tests (24
`ui_iced::about` + 16 `ui_iced::attributions` + 2 `ui_iced::message` +
3 `ui::strings`); `cargo doc --no-deps --quiet -p fastmd` PASS (no
warnings). `cargo clippy -p fastmd --all-targets` still reports the same
**12 pre-existing** test-only lints in other tasks' files
(`config_tests.rs`, `panels/left_tests.rs`, `render/code_tests.rs`,
`ui/batch_dialog_tests.rs`, `message_tests.rs:350`,
`markdown/link_scanner_tests.rs`) — none in the new files.

**Needs the orchestrator's eyes.**

1. **Per-crate license data does not exist.** The reused catalog carries
   authors and a repo URL only. If the product wants a real License column
   per crate, that needs a new compile-time data source (a `cargo-about`
   generated file committed to the repo) — flagged, not invented here.
2. **`image` / `svg` feature decision.** A real About banner (the
   `assets/icon-32.png` mark) requires enabling iced's `image` feature,
   which DEC-011 currently keeps off. `banner_glyph()` / `banner_mark()`
   are the single seam to change.
3. **First-run auto-show was NOT ported.** `should_auto_show_about` /
   `apply_first_run_auto_show` (`src/app/ui/about_dialog.rs:100-123`,
   spec FR-016) live in the egui + persistence layer and are untouched;
   [TASK-022] must reuse them and map the decision onto
   `AboutWindowState::open_about()`. Nothing in this chunk calls them.
4. **Product-parity change: the catalog is no longer inline in About.**
   Users must click one more button to see credits. Confirm this is the
   intended shape.
5. **`strings::CLOSE_BUTTON` is a new generic const.** Existing modules
   keep their own `"Close"` constants (`BATCH_CLOSE_BUTTON`,
   `AGENT_SESSION_CLOSE_BUTTON`); consolidating them is a separate chore.
6. **No `SPEC.md` changes** were made ([RUST-043]); behaviour is mapped
   from existing requirements only.


### TASK-017b3 batch/tools dialog verification log (Batch dialog + Tools dialog — 2026-10-18)

**Scope delivered: the Batch prompt-processing dialog and the Tools
(tool-group) dialog only.** `agent_debug_window` remains a later chunk.
New files: `src/app/ui_iced/batch_dialog.rs` + `batch_dialog_tests.rs`
sidecar (50 tests), `src/app/ui_iced/tools_dialog.rs` +
`tools_dialog_tests.rs` sidecar (44 tests), two `pub mod` lines plus
module-doc bullets in `src/app/ui_iced/mod.rs`, two new message channels
in `src/app/ui_iced/message.rs` (+4 `message_tests.rs` tests) and six
new string constants plus one formatting helper in
`src/app/ui/strings.rs`. No egui file was touched.

**The brief's description of the Batch dialog did not match the code.**
It asked for "file selection for batch processing" and "merge-prompt
candidates". Verified against `src/app/ui/batch_dialog.rs`: the dialog
has **no file list and no merge-prompt handling**. It composes a
*directory* + a *glob pattern* + a *prompt* + a *mode* + a *concurrency*
and hands discovery to the batch subsystem
(`agent/batch/file_matcher.rs`). Merge prompts live on a different
surface entirely — the tree's multi-select context menu builds them
(`ui/tree/handlers.rs:71 build_merge_prompt`) and publishes
`UserCommand::MergePrompt` (`command_executor.rs:302`). The port
implements what exists; the scrollable, selectable rows the brief wanted
were given to the **prompt candidates** instead (see the deviation note
below). [RUST-041] brief-drift note, flagged rather than invented.

**Batch data sources — reused unchanged ([RUST-051]).**

| Field | Source |
| --- | --- |
| `BatchConfig`, `BatchMode`, `PromptInfo` | `crate::agent::batch::types`, re-exported (`pub use`) from `ui_iced/batch_dialog.rs`. `BatchConfig` is the exact `UserCommand::StartBatch` payload (`bus/events/user_command.rs:134`) — not re-declared, not wrapped |
| Validity gate | `validate_batch_params` (`agent/batch/types.rs:48-65`) — the *same* function the egui dialog calls (`ui/batch_dialog.rs:239`), so both builds gate Process identically |
| `BatchDialogConfig` | the egui dialog's own state type; `BatchDialogViewModel::from_config` copies it across so the reducer needs no field translation |
| Directory candidates | `DirectoryTracker::dirs_sorted()` / content-library roots — the egui refresh source (`ui/app/render.rs:105`), snapshotted by the reducer |
| Prompt candidates | `resolve_prompts(app.tags().prompt_paths(), &app.config().content_libraries)` — the egui dialog's own snapshot call (`ui/batch_dialog.rs:26-27`), moved out of render |
| Run status | `Dialogs::batch_handle`, `BatchHandle::thread.is_finished()`, `Dialogs::batch_cancel_flag` → [`BatchRunStatus { has_handle, finished, cancel_requested }`], because `BatchHandle` owns a `JoinHandle` and is neither `Clone` nor `Debug` |

**Tools data source — the tool-group registry, reused unchanged.**

| Field | Source |
| --- | --- |
| Rows | `ToolRegistry::groups(&AgentConfig)` (`src/agent/tools/registry/mod.rs:268-341`) — the same snapshot the egui dialog read (`ui/tools_dialog.rs:41-46`); internal families first, MCP servers sorted by name |
| Types | `ToolGroupState` / `ToolGroupId` / `ToolGroupKind` / `ToolGroupError` / `ToolErrorKind` / `InternalToolGroup` (`agent/tools/registry/groups.rs:9-90`), re-exported (`pub use`), never re-declared |
| Prompt char count | `ToolGroupState::prompt_char_count(&dyn Fn(&str) -> Option<usize>)` (`groups.rs:99-104`) — the subsystem's own summing helper, fed `ToolRegistry::tool_char_count` (`registry/mod.rs:255-263`) with the empty prompt context the egui row used (`ui/tools_dialog.rs:194`, exposed as `PROMPT_CONTEXT`) |
| OAuth eligibility | `McpClients::needs_authentication` (`agent/lib/mcp/mod.rs:320-327`) — the canonical rule. The egui dialog carried a **private duplicate** of the identical function (`ui/tools_dialog.rs:296-303`); the port calls the shared one rather than adding a third copy |
| OAuth in flight | `Dialogs::is_oauth_in_progress(server)` (`ui/dialogs.rs:112-117`) → `McpAuthFacts { eligible, in_progress }` |

`ToolsDialogViewModel::from_app(&FastMdApp)` performs the exact reads
the egui dialog did at render time, once, at snapshot time, so the card
stays I/O-free ([FMD-065]).

**`egui_extras::TableBuilder` → `iced::widget::table` (the migration
map's A5 mapping).** The Tools dialog was the codebase's only real
`egui_extras` consumer. The six columns map one-for-one:

| # | egui (`ui/tools_dialog.rs:143-148`) | iced cell | Width |
| --- | --- | --- | --- |
| 1 | `Column::auto().at_least(28.0)` — enable checkbox | `checkbox(enabled).on_toggle(..)` → `SetToolGroupEnabled` (`checkbox.rs:119`, `:155`) | `Shrink` |
| 2 | `Column::remainder().at_least(140.0).clip(true)` — group | strong `text` + always-allocated error line | `Fill` |
| 3 | `Column::auto().at_least(72.0)` — kind | `text(kind_label(kind))` | `Shrink` |
| 4 | `Column::remainder().at_least(120.0)` — tools | comma-joined `tool_names` | `Fill` |
| 5 | `Column::auto().at_least(80.0)` — prompt char count | `text(prompt_char_count)` | `Shrink` |
| 6 | `Column::remainder().at_least(140.0)` — actions | `Row` of `ToolAction` buttons via `on_press_maybe` | `Fill` |

Verified API (`iced_widget-0.14.2/src/table.rs`): `table(columns, rows)`
`:18-28`, `column(header, view)` `:34-49`, `Column::width` `:650-653`,
and the all-`Shrink` → first-column-`Fill` promotion rule `:129-133` —
three explicit `Fill`s keep the wide columns wide and the short ones
tight, the same lesson `logs::log_table` and `attributions` learned.

**Message-channel choices.**

```rust
// New: only the batch dialog's *composed selection* needed a channel.
pub enum BatchDialogMessage {
    DirectorySelected(Option<usize>),
    PromptSelected(Option<usize>),
    PatternChanged { text: String },
    ModeChanged(BatchMode),
    ConcurrencyChanged(u8),
}
// New: the tools dialog needed exactly ONE variant.
pub enum ToolsDialogMessage { CloseRequested }
```

* **Batch open / start / cancel stay on `Message::Command`.**
  `UserCommand::OpenBatchDialog`, `UserCommand::StartBatch(config)` and
  `UserCommand::CancelBatch` are exactly what the egui dialog published
  (`ui/batch_dialog.rs:169-178`, `:216-225`, `:54-61`), and the
  composed `BatchConfig` travels inside the command, so the intent is
  self-contained and replayable ([FMD-068]). No validation round-trip
  was added: the Process button is gated by
  `BatchDialogViewModel::can_start()` at render time, mirroring egui's
  `is_config_valid` gate.
* **Tools: every action already rides `Message::Command`.**
  `SetToolGroupEnabled` / `ClearToolGroupError` / `StartMcpAuth` /
  `ForgetMcpAuth` (`user_command.rs:80-88`) each carry their own
  `ToolGroupId` or server name — the egui dialog published precisely
  those (`ui/tools_dialog.rs:197-293`). The egui dialog also keeps **no
  view-local state at all** (no search box, no selection, no expansion),
  so nothing else needed a channel.
* **The one exception is dismissal.** egui closed both dialogs through
  the `egui::Window` title-bar `X` (`ui/tools_dialog.rs:61-63`), a
  window-frame widget an in-window iced modal does not have. The tools
  scrim/Escape therefore emits `ToolsDialogMessage::CloseRequested`.
  The batch dialog needs no such variant because its close path *is*
  `UserCommand::CancelBatch` in the egui build.

**Window state — pure, one dialog at a time.**

```rust
// ui_iced/batch_dialog.rs
pub struct BatchDialogState { open: bool, model: BatchDialogViewModel }
// .new() .with_model(vm) .is_open() .model()
// .open(vm) -> bool          — clamps stale selections, reports the open-flag change
// .close() -> bool           — "was anything consumed" (OverlaySlot::close contract)
// .toggle() -> bool
// .current_view() -> BatchDialogView { Config | Progress }
// .can_start() / .batch_config()
// .apply(BatchDialogMessage) -> bool   — did the model actually change

// ui_iced/tools_dialog.rs
pub struct ToolsDialogState { open: bool }
// .new() .is_open() .open() -> bool .close() -> bool .toggle() -> bool
// .apply(ToolsDialogMessage) -> bool
```

Both mirror `AboutWindowState` / `DialogState`: opening replaces,
`close()` reports consumption so an Escape handler can tell a consumed
key from a no-op, and a reducer wanting the single-blocking-overlay
invariant can mirror either into `OverlaySlot<K>`.

**Error highlighting — two tokens, deliberately.** The `⚠` marker
(`strings::TOOLS_ERROR_ICON`) uses `Palette::warning` — exact egui
parity (`ui/tools_dialog.rs:255`). The error **text** uses
`Palette::error`, not `warning`: egui never showed the text at all, it
hid `"{kind:?}: {message}"` in a hover tooltip (`:215`, `:256`). iced
0.14 has no built-in tooltip and DEC-005 rules out `iced_aw`, so the
message must be visible inline, and inline next to a warning marker a
failure reads correctly in the stronger token. Both come from the shared
palette; no numeric literal was introduced ([GUD-003]). Pinned by
`the_error_marker_uses_the_warning_token_and_the_text_the_error_token`.

**Not ported, with reasons.**

1. **Batch: a selected-file list.** Does not exist in the egui dialog —
   it selects a directory + glob and lets `file_matcher` discover the
   files. See the brief-drift note above.
2. **Batch: merge-prompt candidates.** Belong to the tree's
   `UserCommand::MergePrompt`, never to the batch dialog.
3. **Batch: select-all / select-none.** egui has neither (single-choice
   combos throughout), so no transition was invented. The two
   selection-state rules egui *does* run are ported as pure functions:
   `clamp_selection` (the stale-index drop at `ui/batch_dialog.rs:28-36`)
   and `index_of_selected` (the path-stable re-index at
   `ui/app/render.rs:102-108`).
4. **Batch: the spinner.** `egui::Spinner` has no iced 0.14 built-in and
   DEC-005 rules out `iced_aw`; the progress view carries
   `BATCH_PROCESSING_TEXT` plus the cancel hint instead.
5. **Batch: the prompt combo became a scrollable selectable row list.**
   Deliberate deviation. egui used `selectable_combo`
   (`ui/batch_dialog.rs:108-121`); a prompt catalog can outgrow a
   dropdown, and iced's `pick_list` has no search or per-row affordance.
   The rows use the same shape the move dialog's destination picker
   established (`ui_iced/dialogs.rs:644-661`). The directory and
   concurrency pickers *are* `pick_list`, egui parity.
6. **Batch: the pattern field is always allocated, inert in Directory
   mode.** egui hid the whole row (`:90`); AGENTS §6 wants the shape
   kept, so `on_input_maybe(None)` (`text_input.rs:184-191`) makes the
   field stop reporting edits instead of removing it.
7. **Tools: `striped`, `resizable` and the per-column `at_least`
   minimums.** iced 0.14's `table` has no row striping and no
   user-resizable columns — `table::Style` exposes only
   `separator_x`/`separator_y` (`table.rs:676-681`) — and `Column`
   takes one `Length` with no minimum. Proportions are expressed as
   `Fill`/`Shrink` ratios rather than pixel floors.
8. **Tools: the group-name hover tooltip.** Same no-tooltip constraint;
   the error line moved inline under the name (always allocated, empty
   when clean, so the cell shape never changes).
9. **Tools: the `✓ parallel` chip.** Already removed from the egui
   build by user request (`ui/tools_dialog_tests.rs:79-89`);
   `parallel_safe` is still carried in `ToolGroupState` and still not
   rendered.
10. **Both: the `egui::Window` chrome** (resizable/collapsible frames,
    title-bar `X`, `default_size`/`min_size`/`max_height` window
    negotiation). The cards are content only; the overlay host
    ([TASK-023]) mounts them through `ui_iced::overlay::modal`. The
    egui height rule is ported as a pure function
    (`tools_dialog::compute_dialog_size`, same constants and clamps as
    `ui/tools_dialog.rs:76-115`) so the reducer can size the mount.

**Gate results (this pass).** `cargo check --quiet` PASS (workspace,
clean); `cargo clippy -p fastmd --bins --lib -- -D warnings` PASS;
`cargo fmt` run, `cargo fmt --check` PASS; `cargo nextest run -p fastmd
--status-level fail --show-progress none` PASS (**1925 run / 1925
passed / 1 skipped**), of which **100** are this task's new tests (50
`ui_iced::batch_dialog` + 44 `ui_iced::tools_dialog` + 4
`ui_iced::message` + 2 `ui::strings`); `cargo doc --no-deps --quiet -p
fastmd` PASS (no warnings). `cargo clippy -p fastmd --all-targets` still
reports the same **12 pre-existing** test-only lints in other tasks'
files (`markdown/link_scanner_tests.rs`, `ui/batch_dialog_tests.rs`,
`ui_iced/panels/left_tests.rs`, `ui_iced/render/code_tests.rs`,
`config/config_tests.rs`, `ui_iced/message_tests.rs:351`) — none in the
new files.

**Needs the orchestrator's eyes.**

1. **The brief's Batch-dialog description is wrong** (no file list, no
   merge prompts — see the header). The port implements the real
   dialog. If a per-file batch selection is actually wanted, that is a
   new feature needing a new `BatchConfig` shape and a
   `UserCommand::StartBatch` payload change, not a migration.
2. **Product-parity change: the prompt picker is now a scrollable row
   list, not a combo.** One more visible difference between the builds;
   confirm it is wanted.
3. **`BatchDialogState::open(model)` clamps stale selections.** The
   egui build clamped only the *prompt* index inside the dialog
   (`ui/batch_dialog.rs:28-36`) and re-indexed the *directory* by path
   in the render loop (`ui/app/render.rs:102-108`). The port clamps
   both by index at open time and exposes `index_of_selected` for the
   reducer to do the path-stable re-index. If the reducer does not call
   it, a reordered directory list silently drops the selection instead
   of following the path. Flagged so [TASK-022/023] picks it up.
4. **`ToolsDialogMessage` exists only for dismissal.** If a
   `UserCommand::CloseToolsDialog` were added to the bus, this channel
   would become empty and could be dropped. It was not added here because
   the command enum is a shared contract and nothing else needs it.
5. **The egui `needs_authentication` duplicate is still there.**
   `ui/tools_dialog.rs:296-303` remains a private copy of
   `McpClients::needs_authentication`. The iced port uses the shared
   one; deleting the egui copy is a one-line change in a file this chunk
   was barred from touching.
6. **No `SPEC.md` changes** were made ([RUST-043]); behaviour is
   mapped from existing requirements only.

### TASK-017b4 agent-debug verification log (Agent debug window — 2026-10-18)

**Scope delivered: the agent debug window only.** New files:
`src/app/ui_iced/agent_debug.rs` + `agent_debug_tests.rs` sidecar
(62 tests), one `pub mod` line plus a module-doc bullet in
`src/app/ui_iced/mod.rs`, a new `AgentDebug` message channel in
`src/app/ui_iced/message.rs` (+2 `message_tests.rs` tests, channel
count 13 → 14) and one new string constant
(`DEBUG_OMITTED_TOOLS_PLACEHOLDER`) in `src/app/ui/strings.rs`. The
egui `ui/agent_debug_window.rs` was not touched.

**Debug-entry data source — reused unchanged ([RUST-051]).**

| Field | Source |
| --- | --- |
| Entries | `AgentSession::state().debug_entries` (`src/agent/session.rs:33`, appended by `push_debug_entry` at `:215`) — the exact read the egui window performed (`ui/agent_debug_window.rs:21`) |
| Types | `AgentDebugEntry` / `DebugEntryKind` / `DebugEntryRow` (`src/agent/events.rs:23-62`), reached through the existing bus re-export `crate::bus::events::debug` (`src/app/bus/events/debug.rs:3`); re-exported, never re-declared |
| Search / auto-scroll / json-rows / open flag | `AgentPanelState` (`src/app/ui/agent/panel_state.rs:13-31`) — the UI-owned panel state the egui window mutated in place, read through the same four fields |
| External-data markers | `crate::agent::datamark::{EXTERNAL_DATA_START, EXTERNAL_DATA_END}` (`src/agent/datamark.rs:27`, `:32`) — the envelope the agent itself writes, not re-typed literals ([RUST-021]) |

**Live streaming needed no new plumbing.** The agent appends to that same
`Vec` while a turn runs; the [TASK-022] subscription bridge turns each
inbound `AgentEvent::DebugEntry` into a redraw and the next snapshot
picks the new rows up. The view never pulls from a live stream at render
time ([FMD-065]) and never writes to the buffer ([FMD-069]).

**`CollapsingHeader` → an explicit expanded-id set.**
`ui/agent_debug_window.rs:170` was the only `CollapsingHeader` in the
codebase (row A9 of this map, `:30`) and iced 0.14 has no equivalent.
The mapping's second offered shape — a disclose-arrow `Row` — is what
shipped:

```rust
mouse_area(container(Row[chevron, "HH:MM:SS.fff  [Kind]  summary"]))
    .on_press(Message::AgentDebug(AgentDebugMessage::EntryToggled { id }))
```

The state egui kept inside the header widget's memory is a
`HashSet<usize>` held by `AgentDebugWindowState`, copied into the
view-model through `with_expanded`. A row id is the entry's index in the
**raw** debug-entry list — egui's `orig_idx` (`:106`), the very value
its `push_id(orig_idx)` / `id_salt(("entry_header", id_salt))` used —
so row identity is identical across both builds, and egui's
`.default_open(false)` maps to the empty set
`AgentDebugWindowState::new()` starts from. The chevron is
`chevron_glyph(expanded)` → `ICON_CARET_DOWN` / `ICON_CARET_RIGHT` in
the Phosphor font the top bar already loads. Session-boundary rows stay
**non-clickable**: egui's divider had no disclosure either.

Two reducer obligations follow from positional ids ([RUST-041] drift
note): `UserCommand::ClearDebugEntries` must be followed by
`AgentDebugWindowState::collapse_all()` (a stale id would silently
re-expand an unrelated new entry), and `close()` deliberately **keeps**
the set — hiding the window in egui left header states untouched, so
reopening shows what the user had open.

**Auto-scroll — the [TASK-016] intent model, reused not copied.**
`AgentDebugViewModel::follow_intent` delegates straight to
`crate::ui_iced::logs::should_follow`; the report type is the reused
`logs::ScrollSnapshot`, captured by the reused `logs::scroll_snapshot`.
`follow_intent_is_the_logs_rule_itself` asserts agreement with
`logs::should_follow` across five offsets, so the two stick-to-bottom
windows cannot drift. The list carries a stable `DEBUG_SCROLLABLE_ID` so
the reducer can aim `operation::scrollable::scroll_to` at it — a widget
without an explicit id cannot be scrolled programmatically at all.

**`json_rows` semantics — a viewport size, not a truncation.** egui
computed `max_h = text_style_height(Monospace) × json_rows` and capped
a `ScrollArea` with it (`:204-207`), so an expanded body shows
`min(lines, json_rows)` lines and scrolls to reach the rest. Ported as
`visible_json_lines` (the count) and `json_body_height` (the cap).
Because iced's default `LineHeight` is `Relative(1.3)`
(`iced_core-0.14.0/src/text.rs:215-218`), `JSON_LINE_HEIGHT =
JSON_TEXT_SIZE × 1.3` is the iced equivalent of egui's
`text_style_height(Monospace)`. The body is drawn `Wrapping::None`
(iced's default is `Word`) to reproduce egui's
`desired_width(f32::INFINITY)` horizontal scroll. The catalogue is
egui's `[8, 16, 24, 32, 64]` (default `8` from
`AgentPanelState::default`); `json_row_options(current)` additionally
keeps an off-catalogue value visible, because `pick_list` can only
display a value it can find among its options while
`UserCommand::SetDebugJsonRows` accepts any `usize`. Boundary coverage:
`json_rows == 0` ⇒ 0 visible lines and `0.0` height (egui's collapsed
area), `1`, and `usize::MAX`.

**Message-channel choices.**

```rust
// New: only disclosure + the two things iced will not let a view do.
pub enum AgentDebugMessage {
    EntryToggled { id: usize },
    CopyJsonRequested { content: String },
    ScrollReported { offset_y: f32, viewport_height: f32, content_height: f32 },
}
```

* **Settings and close stay on `Message::Command`.**
  `SetDebugSearchText` / `SetDebugAutoScroll` / `SetDebugJsonRows` /
  `ClearDebugEntries` / `ToggleDebugWindow(false)`
  (`bus/events/user_command.rs:152-160`) are exactly what the egui
  window published or mutated to the same effect
  (`ui/agent_debug_window.rs:41-53`, `:149-153`). Nothing duplicated.
* **`EntryToggled` is view-local disclosure, not application state**, so
  it is deliberately *not* a `UserCommand` — there is no command to
  reuse and inventing one would put widget state on a shared contract.
* **`CopyJsonRequested` is deferred intent**: egui copied with
  `Ui::copy_text` inside the click handler (`:177-184`) while iced
  exposes the clipboard to `update` only
  (`iced_core-0.14.0/src/clipboard.rs`), so the exact payload rides the
  message — the same shape `RenderMessage::CopyCodeRequested` and
  `AboutMessage::CopyCommitHashRequested` use.
* **`ScrollReported` is a runtime report**, the same class of payload
  `LogsMessage::ScrollReported` and `LayoutMessage::CursorMoved` carry.
* **No `CloseRequested` variant.** The egui window's own close path *is*
  `UserCommand::ToggleDebugWindow(false)`, so `dismiss_command()` reuses
  it instead of adding a fourth variant.

**Display pipeline — ported in the egui order.** `display_json` =
replace a duplicated `tools` array → `unescape_json_strings` →
`serde_json::to_string_pretty`. `duplicate_tool_indices` keeps the
**first** `tools`-bearing entry of each session whole and elides later
ones (the agent re-sends the same catalog every turn), with a
`SessionBoundary` resetting the flag. It runs on the **filtered** list,
so searching the first tools entry away promotes the second — pinned by
`the_dedup_runs_on_the_filtered_list_not_the_raw_buffer`. The
un-escaper turns string-encoded request/response bodies into real
structure and rewrites `<<<EXTERNAL_DATA>>> … <<<END_EXTERNAL_DATA>>>`
envelopes into `{"EXTERNAL_DATA": {provenance, data}}`; unparseable
bracketed strings and headerless envelopes are left exactly as they
arrived.

**Not ported, with reasons.**

1. **The `egui::Window` chrome** (resizable/collapsible frame,
   `default_size([600.0, 1600.0])`, title-bar `X`). The card is content
   only; `CARD_WIDTH = 600.0` keeps the egui width and the list is
   `Length::Fill`, so the overlay host ([TASK-023]) controls the height
   the way egui's monitor clamp did.
2. **The read-only `TextEdit` became a plain `text` label.** iced 0.14
   has no read-only text area, and a label is exactly what egui's
   `interactive(false)` multiline `TextEdit` rendered.
3. **No search placeholder.** egui's `search_field` is a `Search:` label
   plus a bare single-line edit (`ui/widgets.rs:37-40`); there was no
   hint text to carry over.
4. **No collapse-all control** — egui has none. `collapse_all()` exists
   on the state purely for the reducer's clear-entries path, not as a
   widget.
5. **The filter / dedup / un-escape helpers are re-implemented, not
   `pub use`d.** They are private to `ui/agent_debug_window.rs`, and
   [RUST-051] reuse would have required editing that file, which this
   chunk was barred from doing. The duplication disappears when Phase 5
   deletes the egui layer.

**Gate results (this pass).** `cargo check --quiet` PASS (workspace,
clean); `cargo clippy -p fastmd --bins --lib -- -D warnings` PASS;
`cargo fmt` run, `cargo fmt --check` PASS; `cargo nextest run -p fastmd
--status-level fail --show-progress none` PASS (**1989 run / 1989
passed / 1 skipped**), of which **64** are this task's new tests (62
`ui_iced::agent_debug` + 2 `ui_iced::message`); `cargo doc --no-deps
--quiet -p fastmd` PASS (no warnings). `cargo clippy -p fastmd
--all-targets` still reports the same **12 pre-existing** test-only
lints in other tasks' files (`markdown/link_scanner_tests.rs`,
`ui/batch_dialog_tests.rs`, `ui_iced/panels/left_tests.rs`,
`ui_iced/render/code_tests.rs`, `config/config_tests.rs`,
`ui_iced/message_tests.rs`) — none in the new files.

**Needs the orchestrator's eyes.**

1. **Three reducer obligations for [TASK-022/023]:** apply
   `collapse_all()` whenever the debug buffer is cleared; store the last
   `ScrollReported` and dispatch
   `operation::scrollable::scroll_to(DEBUG_SCROLLABLE_ID, …)` when
   `follow_intent` returns true; map `ToggleDebugWindow` /
   `ToggleAgentDebugWindowShortcut` onto `AgentDebugWindowState::toggle`.
   None of them can be enforced from the pure layer.
2. **Search now round-trips through the command channel** (egui mutated
   `AgentPanelState::debug_search_text` in place). If per-keystroke
   latency is visible, the fix is applying `SetDebugSearchText`
   synchronously inside `update`, not adding a second channel.
3. **`message_tests::one_message_per_channel` still omits `Batch` and
   `Tools`** — TASK-017b3 never added its two channels to that list. It
   now counts 14 with `AgentDebug`. Flagged rather than silently
   rebased; worth a follow-up to make the list exhaustive.
4. **The egui filter/dedup/un-escape duplication** (deviation 5) stays
   until the egui window is deleted. Consolidating now means editing a
   file this chunk could not touch.
5. **No `SPEC.md` changes** were made ([RUST-043]); behaviour is mapped
   from existing requirements only.

---

## TASK-018 agent-panel verification log

Ported the egui agent conversation panel to a pure iced 0.14 view in
`src/app/ui_iced/agent/` (`mod.rs`, `panel_state.rs`, `transcript.rs`,
`prompt_input.rs` + four `_tests.rs` sidecars). Pure view + pure state,
no bus / I/O at render time; **not** wired into the pilot binary or the
update loop (that is [TASK-022]/[TASK-023]).

**Render source: the flat `content` buffer, not `blocks`.** The egui
transcript renders `AgentTranscript::blocks`, but the bus path that
rewrites state in place — `ApplyTaskToggles` (re-renders task checkboxes)
and the bus-lag `truncate_to_last_complete_paragraph` — mutate
`AgentTranscript::content`, not `blocks`. Rendering `blocks` would make
those rewrites invisible on the next frame. The iced view therefore
re-parses `content` into turns every render via
`agent::panel_state::turns_from_content`, so both survive a re-render.
`blocks` is left as the egui-only incremental-build structure.

**Turn recovery.** `content` carries no per-turn role, so a user turn is
recognised by the injected `**User:** ` marker
(`strings::AGENT_USER_TURN_MARKER`, the same literal the orchestrator
prepends at `orchestrator.rs:268`, now sourced from the const rather than
duplicated). A user turn runs from that marker to the first blank line;
everything else is one assistant turn. `split_user_turn` returns the
prompt with the marker stripped. **Documented limitation:** a blank line
*inside* a pasted prompt splits that turn early — the egui incremental
builder never hit this because it recorded role at append time. Accepted
for the pure re-parse model.

**View-model shape.** `AgentPanelViewModel { turns, running, busy_thinking,
show_thinking, current_model, queued_count }`. `TranscriptTurn::Assistant`
carries **pre-parsed `Vec<markdown::Item>`** because
`iced_widget::markdown::view` borrows the items for the returned
`Element`'s whole lifetime (`markdown.rs:1197`), so the items must be
owned by something that outlives the element — here the view model. That
borrow requirement forced dropping `PartialEq` from `TranscriptTurn` and
`AgentPanelViewModel` (they are `Clone + Debug` only). `User` turns keep
the raw prompt string.

**Thinking-delimiter gate** (mirrors `panel_state.rs:214-233`):
`thinking_section_visible(busy_thinking, show_thinking, content)` hides
the section while streaming until the user opens it, and keeps it visible
after the stream ends only if it was opened. `thinking_content` is the
buffer after `**Assistant thinking:**` (`AGENT_THINKING_LABEL`). The
header text is `AGENT_THINKING_LABEL_RUNNING` while streaming,
`AGENT_THINKING_LABEL` otherwise, with the expand/collapse hint appended.

**`conversation_logger` disposition.** Kept **out** of the view. It is a
persistence / background observer (writes the transcript to disk), not a
render concern; [FMD-065] forbids the view being synchronously driven by
it. The iced view reads only the snapshot it is handed. Noted in
`agent/mod.rs` docs.

**Message-channel choices** (all on `Message::Agent`, `message.rs`):
`ThinkingToggled { expanded }` and
`ScrollReported { offset_y, viewport_height, content_height }` were
**added** to `AgentMessage`. Send and Queue both emit the existing
`AgentMessage::Submit { prompt }`; the reducer's reused
`bottom::submit_commands` (`bottom.rs:115-124`) picks `RunAgent` when idle
vs `QueueAgentPrompt` when running — the panel never decides that itself.
Close emits `ClearAgentSession` (via `clear_session_message`, which also
runs `clear_command_input_command`, `bottom.rs:126-133`). Markdown links
emit `RenderMessage::LinkActivated { source_file: None }` because iced's
`Viewer::link` hands back a bare `Uri(String)` with no source file
(`markdown.rs:1576`); the reducer resolves the source the same way the
egui link path does downstream. Scroll follows reuse `logs::should_follow`
/ `ScrollSnapshot` unchanged.

**Verified iced markdown API** (`iced_widget-0.14.2/src/markdown.rs`):
`parse` :426 · `view` :1197 · `view_with` :1213 · `Item` :192-235 ·
`Content` :76-182 · `Settings::with_text_size` :1067 · `Style`
:1102-1117 · `Viewer::image` :1576 · `Catalog for Theme` impl :1706-1722
(our theme satisfies it via `RendererTheme: Catalog`). `markdown_style`
maps `Palette` tokens onto the eight `Style` fields; `markdown_settings`
pins text size to `BODY_TEXT_SIZE`.

**Cargo feature correction.** The task brief said no extra iced features
were needed. That is wrong: `markdown` is a gated feature
(`iced-0.14.0/Cargo.toml:93` `markdown = ["iced_widget/markdown"]`;
`iced_widget-0.14.2/Cargo.toml:51` `markdown = ["dep:pulldown-cmark"]`).
Added `markdown` to the iced feature list in `Cargo.toml`.

**Not ported, with reasons.**

1. **Cancel button.** The egui agent panel has no cancel control (cancel
   lives on the command-input band). `cancel_enabled` is provided as a
   pure helper for [TASK-023] but is not rendered here.
2. **Images are placeholders.** `Viewer::image` returns a placeholder
   text, matching the other `ui_iced` renderers — there is no iced asset
   protocol yet.
3. **No incremental block building.** The egui `AgentTranscriptBuilder`
   appends blocks incrementally; the pure view re-parses `content` per
   render. Cheaper to reason about, and correct under in-place rewrites.

**Gate results (this pass).** `cargo check --quiet` PASS (clean);
`cargo clippy --quiet -- -D warnings` (canonical lib+bins) PASS;
`cargo fmt` run, `cargo fmt --check` PASS;
`cargo nextest run -p fastmd --status-level fail --show-progress none`
PASS (**2058 run / 2058 passed / 1 skipped**), of which **67** are this
task's new agent tests plus the `strings`/`message` additions;
`cargo doc --no-deps --quiet` PASS (no warnings). `cargo clippy
--all-targets` still reports the same **12 pre-existing** test-only lints
in other tasks' files (`markdown/link_scanner_tests.rs`,
`ui/batch_dialog_tests.rs`, `ui_iced/panels/left_tests.rs`,
`ui_iced/render/code_tests.rs`, `config/config_tests.rs`,
`ui_iced/message_tests.rs:384`) — **none in the new agent files**.

**Needs the orchestrator's eyes.**

1. **Reducer obligations for [TASK-022/023]** that the pure layer cannot
   enforce: apply `ThinkingToggled` → `AgentPanelState::show_thinking`;
   store the last `ScrollReported` and dispatch
   `operation::scrollable::scroll_to(AGENT_TRANSCRIPT_SCROLLABLE_ID, …)`
   when `logs::should_follow` returns true; route `AgentMessage::Submit`
   through `submit_commands`; map `ClearAgentSession` onto the agent-state
   reset plus `clear_command_input_command`.
2. **`orchestrator.rs:268` now uses `strings::AGENT_USER_TURN_MARKER`**
   instead of the inline literal — behaviour-identical, single source of
   truth, but it is a change outside `ui_iced/`.
3. **`markdown` feature added to `Cargo.toml`** (grows the binary
   slightly via `pulldown-cmark`) — confirm acceptable.
4. **Blank-line-in-prompt split limitation** (see Turn recovery) — a
   pasted multi-paragraph prompt becomes multiple user turns.
5. **No `SPEC.md` changes** were made ([RUST-043]); behaviour is mapped
   from existing requirements only.

### TASK-019 search+bridge verification log (content search + bus→subscription bridge — 2026-10-18)

**Scope delivered: the content-search view model and the bus → iced
subscription bridge.** New files: `src/app/ui_iced/search.rs` +
`search_tests.rs` (21 tests), `src/app/ui_iced/subscription.rs` +
`subscription_tests.rs` (25 tests), two `pub mod` lines plus two
module-doc bullets in `src/app/ui_iced/mod.rs`, and a message-layer
extension in `src/app/ui_iced/message.rs` (+4 `message_tests.rs` tests,
channel count 14 → 15). No egui file, no bus file and no background file
was touched.

**Verified iced 0.14 runtime primitives (read from the vendored sources,
not from docs).**

| Primitive | Verified signature | Source |
| --- | --- | --- |
| `Subscription::run` | `pub fn run<S>(builder: fn() -> S) -> Self where S: Stream<Item = T> + MaybeSend + 'static, T: 'static` | `iced_futures-0.14.0/src/subscription.rs:182-191` |
| `Subscription::run_with` | `pub fn run_with<D, S>(data: D, builder: fn(&D) -> S) -> Self where D: Hash + 'static, S: Stream<Item = T> + MaybeSend + 'static, T: 'static` | `iced_futures-0.14.0/src/subscription.rs:198-208` |
| `Subscription::batch` | `pub fn batch(subscriptions: impl IntoIterator<Item = Subscription<T>>) -> Self` | `iced_futures-0.14.0/src/subscription.rs:212-221` |
| `Subscription::none` | `pub fn none() -> Self` | `iced_futures-0.14.0/src/subscription.rs:106-110` |
| `Task::run` | `pub fn run<A>(stream: impl Stream<Item = A> + MaybeSend + 'static, f: impl Fn(A) -> T + MaybeSend + 'static) -> Self where T: 'static` | `iced_runtime-0.14.0/src/task.rs:61-69` |
| `Task::perform` | `pub fn perform<A>(future: impl Future<Output = A> + MaybeSend + 'static, f: impl FnOnce(A) -> T + MaybeSend + 'static) -> Self` | `iced_runtime-0.14.0/src/task.rs:48-57` |
| `iced::futures` re-export | `pub use iced_futures::futures;` (and `pub use iced_futures::stream;` on the next line) | `iced-0.14.0/src/lib.rs:484-485` |

Three findings that shaped the design and are worth carrying into
[TASK-022]/[TASK-023]:

1. **`Subscription::run` cannot carry a live receiver.** Its builder is a
   bare `fn() -> S` **pointer**, not a closure, so it cannot close over
   anything. `Subscription::run_with(data, builder)` is the only 0.14
   form that can, and its `D: Hash` bound is the subscription's identity:
   the runtime builds the stream once per distinct hash rather than
   respawning it per `subscription()` call. `SharedReceiver` therefore
   hashes its `Arc` address, so clones of one handle keep the identity
   stable across frames while two independent subscriptions never
   collapse into one.
2. **An `impl Stream` return type cannot be coerced to that fn pointer** —
   the opaque type is "more general than" the single `S` the call site
   infers (E0308). Every lane builder returns a concrete
   `futures::stream::BoxStream<'static, Message>` instead.
3. **This build's iced executor is `futures::executor::ThreadPool`, not
   tokio.** iced's `tokio` feature is off (`iced-0.14.0/Cargo.toml:64-73`
   default = wgpu/tiny-skia/crisp/web-colors/thread-pool/linux-theme-detection/x11/wayland;
   `tokio = ["iced_futures/tokio"]` at `:108`), and
   `backend::default` resolves to `backend::native::thread_pool` when
   neither `tokio` nor `smol` is on
   (`iced_futures-0.14.0/src/backend/default.rs:18-22`,
   `backend/native/thread_pool.rs:4`). The `tokio::sync::*` primitives
   used here (broadcast, mutex, unbounded mpsc) are reactor-free and poll
   correctly on that executor, but `tokio::task::*` would panic — which
   is why the blocking `std::sync::mpsc` background channel is bridged by
   one dedicated forwarding thread rather than `spawn_blocking`.

**No new crates.** `futures` is reachable through iced itself
(`iced::futures`, `iced-0.14.0/src/lib.rs:484`), so the streams are
built with `futures::stream::unfold` + `StreamExt::filter_map`/`boxed`.
`async-stream` and `tokio-stream` were **not** added — `tokio-stream` is
in `Cargo.lock` only behind the optional `vector-search` feature
(`async-openai`'s dependency, `Cargo.lock:386`, `:6590`), so taking it
as a direct dependency would have compiled it into the default binary for
no reason. `Cargo.toml` is unchanged.

**Search engine — reused unchanged ([RUST-051]).**

| Piece | Source |
| --- | --- |
| Result row type | `SearchResultEntry` (`src/app/ui/tree_search.rs:15-28`), re-exported (`pub use`) from `ui_iced/search.rs` — never re-declared |
| Scoped engine | `find_search_results_scoped` (`src/app/ui/tree_search.rs:227-301`) — the exact call the egui left panel made (`src/app/ui/panels/left.rs:252`), re-exported, not wrapped |
| Unscoped engine | `find_search_results` (`src/app/ui/tree_search.rs:308-314`) |
| Underlying matcher | `fastmd_agent::tools::search::search_files_parallel` / `count_occurrences` / `extract_snippet` — reached through the existing engine, untouched |
| Match-count label | `strings::format_match_count` (`src/app/ui/strings.rs:189-191`) |
| Header / empty state | `strings::SEARCH_RESULTS_HEADER` (`:181`), `strings::SEARCH_NO_RESULTS` (`:178`) |

`SearchViewModel` is the egui `TreeSearch` state model
(`src/app/ui/tree_search.rs:37-146`) minus the scan-on-commit: query text
stays separate from the committed filter, and results arrive from the
outside via `apply_results` rather than being produced in place. The
three presentation predicates the egui result card decided inline
(`src/app/ui/panels/left.rs:299-323`) are ported as pure functions —
`shows_match_count` (`match_count > 1`), `shows_relative_path` (non-empty
and not equal to the file name) and `shows_snippet` — so the already-iced
`ui_iced/panels/left.rs` card and any future surface share one rule. No
new user-facing string was needed ([GUD-002] satisfied with existing
constants; the in-progress flag is internal state and deliberately has no
label, because the egui build never showed one).

**Why the scan is no longer synchronous.** egui ran it inside the left
panel's paint pass (`ui/panels/left.rs:246-252`), so a large workspace
froze the UI — against [FMD-060]/[FMD-061]. The port keeps the same
engine but moves the call off the UI thread: `submit()` commits the
trimmed term, drops the stale list and raises the in-progress flag, the
reducer runs the engine in a `Task`, and `apply_results` snapshots the
outcome. `apply_results` is **staleness-guarded**: a result whose echoed
query is not the currently committed filter is dropped without touching
any state, including the in-progress flag the newer scan still owns
([FMD-062]). This is why `SearchMessage::ResultsReady { query, results }`
was added — see "Needs the orchestrator's eyes".

**Full bus-event → `Message` mapping.**

| Bus / channel | Producer | Event | `Message` |
| --- | --- | --- | --- |
| `Bus<ConfigArrived>` | `bus/config.rs`, one-shot at startup | `ConfigArrived { config }` | `Inbound(InboundMessage::ConfigArrived(Arc<AppConfig>))` |
| `Bus<FileEvent>` | watcher, initial scan, UI handlers, tools, agent | `FileEvent { kind: Discovered, paths }` | `Inbound(InboundMessage::FileChanged(event))` |
| ″ | ″ | `kind: Updated` | ″ |
| ″ | ″ | `kind: Removed` | ″ |
| ″ | ″ | `kind: DirDiscovered` | ″ |
| ″ | ″ | `kind: DirRemoved` | ″ |
| `Bus<AgentEvent>` | `BusAgentEventObserver` (`bus/events/agent.rs:100`) | `SessionStarted` | `Inbound(InboundMessage::Agent(Arc<AgentEvent>))` |
| ″ | ″ | `SessionFinished` | ″ |
| ″ | ″ | `Status` | ″ |
| ″ | ″ | `Thinking` | ″ |
| ″ | ″ | `ContentDelta` | ″ |
| ″ | ″ | `ToolCallStarted` | ″ |
| ″ | ″ | `ToolResult` | ″ |
| ″ | ″ | `ToolSideEffect` | ″ |
| ″ | ″ | `DebugEntry` | ″ |
| ″ | ″ | `TokenUsage` | ″ |
| ″ | ″ | `Failed` | ″ |
| `std::sync::mpsc<BackgroundEvent>` | `BackgroundEventSender` (`bus/events/typed.rs:28`) | `Fs(FileParsed)` | `Inbound(InboundMessage::Background(Arc<BackgroundEvent>))` |
| ″ | ″ | `Fs(DirParsed)` | ″ |
| ″ | ″ | `Fs(FileModified)` | ″ |
| ″ | ″ | `Fs(FileDeleted)` | ″ |
| ″ | ″ | `Fs(Finished)` | ″ |
| ″ | ″ | `Fs(FinishedWithoutWatcher)` | ″ |
| ″ | ″ | `Process(LogEntry)` | ″ |
| ″ | ″ | `Process(FileLoaded)` | ″ |
| ″ | ″ | `McpAuth(Completed)` | ″ |
| any bus, `Lagged(n)` | tokio broadcast channel | dropped-event report | `Inbound(InboundMessage::EventsDropped { source, count })` |

29 event shapes enumerated and covered by tests; every one maps to
`Some`. `bus_event_to_message` returns `Option<Message>` and no current
event produces `None` — every payload on these four buses is
UI-observable, which is what the egui drain loops demonstrated. The
`Option` is the deliberate seam where a future filter (throttle a chatty
producer, suppress another session's events) can drop a class of events
without touching the stream plumbing; the streams drop `None`s through
`filter_map`.

**Why one dedicated `Inbound` channel instead of new `Agent`/`Logs`/`Tree`
variants** (the [TASK-021] module doc left this open both ways). Two
independent reasons:

1. **Channel rule.** [TASK-021] rule 2 keeps panel channels for
   *view-local state* produced by a widget. A bus notification is neither.
2. **Type-level blocker.** `AgentEvent` (`bus/events/agent.rs:20`),
   `BackgroundEvent` (`bus/events/typed.rs:118`), `BackgroundLogEntry`
   (`bus/events/messages.rs:63`) and `AppConfig`
   (`src/app/config/config.rs:81`) derive `Clone` but **not**
   `PartialEq`, while every panel channel derives it. Riding them would
   have meant editing the bus, which this task is barred from doing.
   `InboundMessage` therefore carries the opaque payloads behind an
   `Arc` with a hand-written `PartialEq`: **delivery identity**
   (`Arc::ptr_eq`) for `AppConfig`/`AgentEvent`/`BackgroundEvent`, and
   real **value** equality for `FileEvent` and the lag report, which
   support `PartialEq` natively. Cloning a `Message` preserves the shared
   instance (so the [TASK-021] "clone must preserve the payload exactly"
   rule holds), while two independently delivered events are two
   deliveries and never compare equal — the correct answer for a dedup
   check on notifications.

**FMD-065..FMD-067 handling.**

* **[FMD-065] single direction.** The bridge only ever *reads*. It holds
  no `Bus::publish` path, no `BackgroundEventSender` and no
  `UserCommand` producer; the UI's outbound half is `Message::Command`
  and the [TASK-022] reducer. The four handles are receive-only wrappers
  around `Bus::subscribe_async` (`bus/core.rs:77-79`) and the std mpsc
  receiver.
* **[FMD-066] order per publisher.** Each lane is one FIFO consumer of
  one publisher, and tokio broadcast preserves per-sender order; the
  `test_stream_delivers_events_in_publish_order` test pins four publishes
  arriving in order. No merging that could reorder across buses — the four
  lanes stay separate inside `Subscription::batch`.
* **[FMD-067] loss notification + re-sync.** Ported from the egui drain
  loops, which read `BusReader::try_recv_exposing_lag() →
  BroadcastRecvError::Lagged(n)` (`bus/core.rs:149-151`) and handled it
  at `orchestrator.rs:96-103` (user commands) and `:435-449` (agent
  events). The async equivalent is `Receiver::recv() →
  Err(RecvError::Lagged(n))` — the same count off the same channel. On
  lag the bridge (a) emits `tracing::warn!` with
  `dropped_events = n` and the bus name, and (b) forwards
  `InboundMessage::EventsDropped { source, count }` so the reducer can
  surface the loss and re-synchronise on the next valid event, then
  **keeps streaming** — a lag is not a disconnect. Only `Closed` ends a
  stream. The bridge applies this to **all four** buses; the egui build
  only did it for the agent and user-command buses, silently mapping
  `Lagged` to `Empty` on the config bus (`orchestrator.rs:292-302`) and
  not exposing lag on the file bus at all (`:128`).
* **[FMD-072] count, not silence.** Both the warning and the message
  carry the count; no `Lagged` arm discards it.

**Subscription shape.** `subscription(&BusReceivers) ->
iced::Subscription<Message>` = `Subscription::batch([...])` of four
`Subscription::run_with(handle, lane_builder)` recipes, ready to pass
straight to `.subscription(...)` in [TASK-022]. `BusReceivers::new(...)`
subscribes **eagerly**, before the producers run — `Bus::publish`
delivers to nobody with no subscribers (`bus/core.rs:87-91`) and
`ConfigArrived` is published exactly once at startup, so a lazily
subscribed handle would lose DEC-009 entirely. Cloning the struct is four
`Arc` bumps and preserves every identity.

**Gate results (this pass).** `cargo check --quiet` PASS (workspace,
clean); `cargo clippy -p fastmd --bins --lib -- -D warnings` PASS;
`cargo fmt` run, `cargo fmt --check` PASS; `cargo nextest run -p fastmd
--status-level fail --show-progress none` PASS (**2108 run / 2108
passed / 1 skipped**), of which **50** are this task's new tests (21
`ui_iced::search` + 25 `ui_iced::subscription` + 4 `ui_iced::message`);
`cargo doc --no-deps --quiet` PASS (no warnings). `cargo clippy
--all-targets` still reports the same **12 pre-existing** test-only lints
in other tasks' files (`markdown/link_scanner_tests.rs`,
`ui/batch_dialog_tests.rs`, `ui_iced/panels/left_tests.rs`,
`ui_iced/render/code_tests.rs`, `config/config_tests.rs`,
`ui_iced/message_tests.rs:393` — the last shifted from `:384` by this
task's import additions) — **none in the new files**.

**Needs the orchestrator's eyes.**

1. **`SearchMessage::ResultsReady` is new-for-iced, not an egui port.**
   egui had no completion message because it scanned synchronously. It
   was added because the async port cannot report a background scan's
   outcome any other way, and it is what makes the staleness guard
   possible (the query rides back with the results). If [TASK-022] would
   rather keep the search fully synchronous for parity, this variant is
   unused and can be dropped — but that reintroduces the UI freeze.
2. **The brief's "map into the Agent/Logs/Tree channels" is not
   achievable** as written — see the `PartialEq` blocker above. The
   dedicated `Inbound` channel is the option [TASK-021]'s module doc
   explicitly left open; that paragraph has been updated to record the
   decision rather than leave it as an open question.
3. **Reducer obligations for [TASK-022]/[TASK-023]** that the pure
   layer cannot enforce:
   * `InboundMessage::ConfigArrived` → the full
     `AppOrchestrator::drain_config_bus` projection
     (`orchestrator.rs:309-330`): `to_agent_config()`,
     `content_libraries`, `inline_editor_enabled`,
     `dialogs.batch_dialog_config.available_dirs`, `tree_dirty`.
   * `InboundMessage::FileChanged` → the `process_file_events` fan-out
     (`orchestrator.rs:113-194`).
   * `InboundMessage::Agent` → the `drain_agent_event_bus` transcript
     and agent-state updates (`orchestrator.rs:365-458`), including the
     `LAG_TRUNCATION_MARKER` append on `EventsDropped { source: Agent }`.
   * `InboundMessage::Background` → `handle_fs_event` /
     `handle_process_event` / `handle_mcp_auth_event`
     (`orchestrator.rs:460-611`).
   * `InboundMessage::EventsDropped` → surface the loss per source; for
     `Agent` also set the lag flag and clear it on the next
     `SessionStarted`/`Status`, as `orchestrator.rs:377-385` does.
   * `SearchMessage::ResultsReady` → `SearchViewModel::apply_results`,
     and dispatch the engine call in a `Task` on `Submitted`.
4. **One OS thread per app run** for the `std::sync::mpsc` → async
   bridge, unavoidable given the non-tokio iced executor and the
   blocking `BackgroundEventSender` contract. It exits when the producer
   drops its sender. Flagged because it is a visible resource the egui
   build did not have.
5. **`InboundMessage` equality is deliberately coarse** (delivery
   identity for three of the five variants). Anything that wants to dedup
   inbound notifications by *content* must compare the payload fields
   itself; `Arc::ptr_eq` will not do it.
6. **No `SPEC.md` changes** were made ([RUST-043]); behaviour is mapped
   from existing requirements only.

### TASK-020 logo/misc verification log (logo port decision + egui-free module audit - 2026-10-18)

**Scope delivered: an audit plus one small consolidation.** Most of
TASK-020 turned out to need *no* porting — three of the four modules in
scope are already reusable by the iced layer exactly where they sit. The
only real work was removing the duplicated logo stand-in: new
`src/app/ui_iced/logo.rs` + `logo_tests.rs` (6 tests), with
`ui_iced/panels/top.rs` and `ui_iced/about.rs` refactored to route
through it, and `ui_iced/mod.rs` updated. Nothing was moved on disk, and
no cargo feature was added or removed.

**Per-module egui-dependency audit.**

| Module | egui / eframe dependency | Verdict | What the iced layer does |
| --- | --- | --- | --- |
| `ui/measure.rs` | `use eframe::egui;` — the *entire* API is egui-typed: `text_width(&egui::Context, &str, egui::FontId, egui::Color32) -> f32`, wrapping the 0.35 `fonts_mut` + `layout_no_wrap` quirk | **egui-bound by construction.** There is no pure part to extract — the context *is* the function | Not imported. iced cannot hand a `Context` to a pure view, so measurement is injected instead (`ui_iced/table_width.rs::IcedTextMeasurer`, [TASK-015]). No gap, no duplication |
| `ui/os_shell.rs` | `use eframe::egui;` — used by **one** of its four functions: `dispatch_platform_commands(&[egui::OutputCommand], F)` | **Mixed.** `open_in_system_editor`, `show_in_file_explorer` and `open_url` are egui-free `opener` wrappers; only the platform-output drainer is egui-typed | Importable as-is from `crate::ui::os_shell::{open_in_system_editor, show_in_file_explorer, open_url}` (`pub mod` + `pub(crate)` re-exports at `ui/mod.rs:17,42`) — **not moved, not copied**. `dispatch_platform_commands` is the egui `OutputCommand::OpenUrl` bridge and dies with the egui layer; the iced link path calls `open_url` directly instead |
| `ui/link_resolver.rs` | **zero** references to `egui`/`eframe` (grep-verified) | **egui-free by construction** — pure `&str` + `Option<&Path>` + slices → `LinkAction` | Importable as-is: `pub(crate) mod` at `ui/mod.rs:13` with `pub use link_resolver::{LinkAction, resolve_link}` at `:41`. This is the [TASK-015] Render-channel contract the reducer calls (`ui_iced/message.rs:594`), so the iced side already depends on it through that contract. **Not moved, not duplicated** |
| `ui/logo.rs` | `use eframe::egui;` — `paint_logo` draws the badge with `Painter::rect_filled`, `Shape::convex_polygon` and `line_segment`; `load_app_icon` returns `egui::IconData` | **The mark is vector paint, not a raster asset.** Not portable without a feature or a re-implementation | `load_app_icon` stays egui-only. The iced window icon already bypasses it via `crate::utils::app_icon::decode_app_icon_rgba` ([TASK-006]) |

**Logo decision: option (a) — keep the Phosphor-bolt stand-in, but stop
duplicating it.** `paint_logo` is ~140 lines of egui painter calls with
no iced equivalent under DEC-011's enabled features, so the stand-in
[TASK-011] introduced stays. What changed is *ownership*:
`ui_iced/logo.rs` now owns the mark (`logo_glyph`, `logo_color`,
`logo_font`, `logo_mark(size)`), and both consumers call it —

* `panels/top.rs` — `let logo = logo_mark(LOGO_MARK_SIZE);` (was an
  inline four-line `text(..).font(..).size(..).color(..)` build),
* `about.rs` — `banner_glyph()` delegates to `logo::logo_glyph()` and
  `banner_mark()` to `logo::logo_mark(ABOUT_BANNER_SIZE)` (was its own
  four-line build plus a `panels::top::phosphor_font` import).

`LOGO_MARK_SIZE` moved to `logo.rs` and is **re-exported** from
`panels::top`, so that module's public surface is unchanged. After the
refactor the literal `strings::ICON_LIGHTNING` appears in iced production
code exactly once — `ui_iced/logo.rs:65` ([RUST-051]).

**Feature trade-off (verified, not assumed).** Read from
`iced_widget-0.14.2`, `iced_renderer-0.14.0`, `iced_graphics-0.14.0`,
`iced_wgpu-0.14.0`, `iced_tiny_skia-0.14.1` feature tables and this
workspace's `Cargo.lock`:

| Option | Feature chain | Unlocks | Marginal cost |
| --- | --- | --- | --- |
| Raster badge | `iced/image` → `iced_renderer/image` → `iced_graphics/image` = `dep:image` + `kamadak-exif` | `iced::widget::image` over `crate::utils::app_icon::APP_ICON_PNG` (the real 32px mark, already embedded) | `image` 0.25 is **already** in the lock tree (fastmd's `utils::app_icon`); the only new crate is `kamadak-exif` (absent today). **Cheapest path to real artwork** |
| Vector SVG | `iced/svg` → `iced_wgpu/svg` = `resvg/text`, `iced_tiny_skia/svg` = `resvg` | `iced::widget::svg` over `assets/icon.svg` (present, 1.5 kB) | `resvg` + `usvg` + its text stack — **absent** from the lock. **Heaviest option** |
| Re-painted vectors | `iced/canvas` → `iced_renderer/geometry` = `iced_graphics/geometry` (`lyon_path`) + `iced_wgpu?/geometry` (`lyon`) | `iced::widget::canvas` — a `Program` reproducing `paint_logo`'s polygons at any size | `lyon` / `lyon_path` — **absent** from the lock, plus a hand port of the ~140-line paint routine. **Best visual parity** (crisp at every zoom, no raster asset) |

None of the three is enabled and **no cargo feature was touched** —
enabling one is a build-level decision for the orchestrator.

**Imported directly vs ported.**

*Directly, unchanged, in place:* `crate::ui::link_resolver::{resolve_link,
normalize_path, LinkAction}`; `crate::ui::os_shell::{open_in_system_editor,
show_in_file_explorer, open_url}`; `crate::utils::app_icon::{APP_ICON_PNG,
decode_app_icon_rgba}` ([TASK-006]).
*Ported / new:* `ui_iced/logo.rs` — the shared stand-in renderer.
*Not ported, with reasons:* `paint_logo` (vector art, needs a feature);
`load_app_icon` (egui `IconData` wrapper — iced has its own icon path);
`dispatch_platform_commands` (egui platform-output bridge, meaningless
without egui); `measure::text_width` (egui `Context` measurement — iced
injects its measurer instead).

**Tests.** 6 new in `logo_tests.rs`: glyph identity against
`strings::ICON_LIGHTNING`; colour resolves to the `accent_text` token
through `to_iced_color` ([GUD-003]); font handle is the registered
`phosphor` family ([TASK-007]); `LOGO_MARK_SIZE == 20.0` (parity with
the `egui::vec2(20.0, 20.0)` allocation at `ui/panels/top.rs:142`);
element-construction smoke at both caller sizes (top bar and About
banner); and `about::banner_glyph()` delegates to `logo::logo_glyph()` —
the routing check. The pre-existing
`about_tests::banner_glyph_is_the_shared_phosphor_bolt` and
`top_tests::top_bar_builds_for_both_indexing_states` still pass and now
cover the routed paths. No tests were written for modules this task did
not change; the `measure` / `os_shell` / `link_resolver` sidecars were
left running untouched.

**Gate results (this pass).** `cargo check -q` PASS (workspace, clean);
`cargo clippy -p fastmd --bins --lib -- -D warnings` PASS; `cargo fmt`
run, `cargo fmt --check` PASS; `cargo nextest run -p fastmd
--status-level fail --show-progress none` PASS (**2114 run / 2114 passed
/ 1 skipped**), of which **6** are this task's new tests;
`cargo doc --no-deps --quiet -p fastmd` PASS (no warnings).

**Needs the orchestrator's eyes.**

1. **Feature decision (the only open question in this task).** Cheapest
   real artwork = `image` (one new crate, `kamadak-exif`, and the PNG is
   already embedded); best fidelity = `canvas` (lyon + a hand port);
   `svg` is the heaviest and buys the least over `canvas`. If any is
   approved, `logo_mark` is the single call site to change — no panel
   edits.
2. **`LOGO_MARK_SIZE` is a 20pt *glyph*, not the 20×20pt *box* egui
   allocates**, so the drawn mark is slightly smaller than the egui
   badge. If exact visual parity matters, the fix is the `image`/`canvas`
   route, not inflating the point size.
3. **`ui/measure.rs` stays egui-only forever.** When Phase 5 deletes the
   egui layer it takes the tree-width and table-width egui measurers with
   it; the iced path already has its injected measurer ([TASK-015]), so
   there is no gap to close now.
4. **`dispatch_platform_commands` has no iced counterpart by design.**
   iced link activation calls `os_shell::open_url` from the reducer
   ([TASK-022/023]) rather than draining a platform-output list.
5. **`link_resolver` is `pub(crate)`** (re-exported at
   `crate::ui::{resolve_link, LinkAction}`). If a future iced-side
   integration wants it from outside the crate, that visibility — not the
   module — is what must change.
6. **No `SPEC.md` changes** were made ([RUST-043]); behaviour is mapped
   from existing requirements only.


### TASK-022 launch/state-decoupling verification log (launch() entry + egui-free state + DEC-009 config arrival - 2026-10-18)

**Scope delivered: the iced application root.** New files:
`src/app/ui_iced/app.rs` + `app_tests.rs` (19 tests),
`src/app/ui_iced/update.rs` + `update_tests.rs` (8 tests),
`src/app/ui_iced/view.rs` + `view_tests.rs` (5 tests); a `Boot` channel
(`BootMessage`) added to `message.rs` (+1 `message_tests.rs` test);
`ui_iced/mod.rs` updated with the three modules + doc bullets; a
`pub(crate) FastMdApp::from_parts` constructor added to
`src/app/ui/app/init.rs`; and `src/app/bin/fastmd_iced.rs` refactored to a
thin shim over `app::launch()` (its old `fastmd_iced_tests.rs` sidecar was
deleted — the still-relevant reductions were re-hosted in `update_tests.rs`
against the real `AppState`). `main.rs` was NOT flipped (that is
[TASK-024]).

**Verified iced 0.14 builder chain** (read from the local registry source
`iced-0.14.0/src/application.rs`, not docs):

| Builder element | Verified line |
| --- | --- |
| `iced::application(boot, update, view)` | helper (`application.rs`) |
| `.font(bytes)` | `:245` |
| `.window(window::Settings)` | `:253` |
| `.exit_on_close_request(bool)` | `:269` |
| `.window_size(Size)` | `:280` |
| `.position(window::Position)` | `:324` |
| `.title(...)` (`TitleFn`, `&'static str` impl `:579`) | `:346` |
| `.subscription(...)` | `:363` |
| `.theme(...)` (`ThemeFn`) | `:378` |
| `.scale_factor(impl Fn(&State) -> f32)` | `:410` |
| `.presets(...)` | `:448` |
| `.run() -> Result` | `:186` |
| `BootFn` / `IntoBoot for (State, Task<Message>)` | `:535-566` |
| `UpdateFn` (`Fn(&mut State, Message) -> C: Into<Task>`) | `:609-617` |
| `ViewFn` (`Fn(&'a State) -> Widget: Into<Element>`) | `:628-638` |

`launch()` wires `boot` + `update` + `view` through
`.title(strings::WINDOW_TITLE)` ("FastMD AI", DEC-010) →
`.window(window_settings)` (persisted size/position + icon +
`exit_on_close_request: false`, DEC-006) → `.theme(fastmd_dark_theme)`
([TASK-008]) → `.scale_factor(composed_scale_factor)` (DEC-008) →
`.subscription(subscription(&receivers))` ([TASK-019]) → `.run()`. Fonts
register through the [TASK-007] boot task (returned by `boot`), matching
the pilot's proven pattern rather than `.font(bytes)`.

**AppState shape — a thin iced wrapper, NOT `FastMdApp` itself.**
`AppState { app: FastMdApp, receivers: BusReceivers, storage:
Arc<dyn UiStateStorage>, os_scale: Option<f32>, viewport: Size, panes:
pane_grid::State<PaneKind>, band: MiddleBandPanes }`. The domain half is
the egui-free `FastMdApp` (its struct fields were confirmed egui-free —
the egui coupling lives only in the `impl eframe::App` block at
`ui/app/mod.rs:244-274` and the `cc.egui_ctx`/`cc.storage` use in
`init.rs::new`); the wrapper carries the iced-runtime bits the domain
struct never had. It is constructible with zero `egui`/`eframe` references
([DEC-011]). [TASK-023] grows the view-local half (cursor anchor, overlay
slot, dialog/about/debug window state, scroll reports) onto this struct.

**Egui-free construction approach: PARALLEL constructor, not a shared
`init.rs` core.** `build_state` (in `ui_iced/app.rs`) mirrors the plain
(non-egui) wiring of `init.rs::new` — background `Task`, MCP
config-subscriber, agent session + observer fan-out, directory tracker,
persisted-UI-state projection, first-run auto-show — but resolves the
snapshot through the injected `UiStateStorage` instead of `eframe::Storage`
and direct-injects the loaded `AppConfig`. The reason a single shared core
was rejected: the background-event channel `rx:
std::sync::mpsc::Receiver<BackgroundEvent>` is **single-owner**. The egui
path moves it into `AppOrchestrator.rx` and drains it per frame; the iced
path must move it into the [TASK-019] `BusReceivers` bridge so background
notifications arrive as `Message::Inbound`. Sharing one core would force a
fate-flag or an inert dummy channel into the shipping egui build. The
parallel constructor keeps `init.rs::new` byte-identical (zero regression
risk on the egui app that lives until [TASK-024]) at the cost of a
temporary duplication that dies with the egui layer in [TASK-029]. The
only change to `init.rs` is the additive `pub(crate) FastMdApp::from_parts`
(parts-in, struct-out) so the iced tree can build `FastMdApp` without
touching its three private egui first-frame font-tracking fields. In the
iced build the orchestrator's `rx` is given an inert, never-fed channel
(`try_recv` returns immediately — no blocking); the real events flow
through the bridge ([FMD-065] single inbound path).

**DEC-009 config-arrival wiring.** `boot` (a) constructs the state with
every subscriber already live — the background `Task` thread, the MCP
subscriber and the eager `BusReceivers` — and direct-injects the loaded
config into the orchestrator (`config` field, `content_libraries`,
`inline_editor_enabled`, batch-dialog dirs) and the agent
(`agent.set_agent_config(config.to_agent_config())`); (b) returns a
`Task::perform` that publishes `ConfigArrived` on the existing
`Bus<ConfigArrived>` for the background workers, mapped onto
`BootMessage::ConfigPublished`. Because the publish runs in a task, every
subscriber exists before the event is sent — the exact ordering guarantee
the retired `main.rs:147-160` workaround enforced, now expressed the iced
way. The UI itself never depends on the broadcast (it was direct-injected);
the subscription bridge still delivers the same publish to `update` as an
idempotent `Inbound(ConfigArrived)` for [TASK-023]'s projection.

**What moved from `main.rs`.** The CLI-argument workspace injection
(`main.rs:61-115`) moved into `boot` UNCHANGED, extracted as the pure
`apply_cli_workspace(&mut AppConfig, &[String])` so both branches
(explicit-dir and system-library-only→cwd) are unit-testable without a
process. The `Box::new`-then-publish workaround (`main.rs:147-160`) is
RETIRED — not replicated. `main.rs` itself is untouched (TASK-024 flips
it).

**Message-channel note.** A new top-level `Boot(BootMessage)` channel was
added (`FontLoadFinished(Option<String>)`, `ConfigPublished`). This is the
home the `message.rs` pilot-fold table promised for the font bootstrap
("until [TASK-022] gives the font bootstrap its own channel"), and the
terminal report the DEC-009 `Task::perform` needs (every iced task must map
to a `Message`; no existing variant fit a fire-and-forget publish). It is
deliberately separate from `Window` (not a window concern) and `Inbound`
(produced by the boot task chain, not a background bus). Channel count
15 → 16.

**Pilot binary.** `bin/fastmd_iced.rs` is now a ~70-line shim: panic hook +
non-blocking logging + rustls provider (mirroring `main.rs:16-44`), then
`app::launch(LaunchInputs { config: load_config(), config_bus,
config_storage: FileConfigStorage(get_config_path()), ui_state_storage:
ui_state_storage_from_env(FASTMD_ICED_UI_STATE_PATH), args })`. So
`cargo run --bin fastmd_iced` exercises the REAL launch chain. The UI-state
storage keeps the [TASK-009] pilot safety rule: env-override → file, else
in-process; the platform-default UI-state location is NOT resolved by the
pilot ([RUST-024]/[RUST-006]).

**Gate results (this pass).** `cargo check -q` PASS (workspace, clean);
`cargo clippy -p fastmd --bins --lib -- -D warnings` PASS; `cargo fmt`
run, `cargo fmt --check` PASS; `cargo nextest run -p fastmd --status-level
fail --show-progress none` PASS (**2133 run / 2133 passed / 1 skipped**),
of which **33** are this task's new tests (19 `app` + 8 `update` + 5
`view` + 1 `message` Boot-channel) and the 13 pilot sidecar tests were
re-hosted/retired with the binary gut; `cargo doc --no-deps --quiet -p
fastmd` PASS (no warnings).

**Needs the orchestrator's eyes.**

1. **`Boot` message channel added** (message.rs 15→16). If the channel
   design wants the font report elsewhere, `BootMessage` is the single
   place to change.
2. **`FastMdApp::from_parts` added to `init.rs`** — additive, egui-free,
   keeps the three font-tracking fields private; it is the iced construction
   seam. Flagged because it is a change outside `ui_iced/`.
3. **Inert `rx` in the iced orchestrator.** The real background channel
   goes to the bridge; the orchestrator's `rx` is a never-fed channel whose
   `try_recv` returns immediately. [TASK-023] must NOT rely on
   `orchestrator.rx` for background events in the iced build — use
   `Message::Inbound(Background)`.
4. **View uses band STUBS, not the real panels.** The real
   `panels::top::top_bar` returns an `Element<'_>` that borrows its
   `TopBarViewModel` (the tag `pick_list` holds `Option<&TagFilterOption>`
   from `selected_tag_option`), so it cannot escape `view` while the VM is
   a stack local (E0515). [TASK-023] must resolve the view-model lifetime
   (hold the VM in `AppState` and rebuild in `update`, or make panels
   return owned elements) before mounting the real top bar / left / center /
   bottom panels.
5. **What [TASK-023] must complete in `update`/`view`:** the full
   one-to-one mirror — `Inbound(ConfigArrived)` → the
   `drain_config_bus` projection; `Inbound(FileChanged)` →
   `process_file_events`; `Inbound(Agent)` → the transcript/agent-state
   drain incl. `LAG_TRUNCATION_MARKER` on lag; `Inbound(Background)` →
   `handle_fs/process/mcp_auth_event`; `Inbound(EventsDropped)` → surface
   per source; `Tree`/`Tab` deferred-intent reads + overlay open using the
   stored `CursorMoved` point; `Editor` DEC-007 undo/redo + save; `Agent`
   `Submit`→`submit_commands`, `ThinkingToggled`, `ScrollReported`→
   `should_follow`+`scroll_to`; `Search` `Submitted`→engine Task +
   `ResultsReady`→`apply_results`; `Logs`/`AgentDebug` scroll + filters;
   `Dialog`/`About`/`Batch`/`Tools` window-state transitions;
   `LayoutMessage::TreeAutoFit`/`CursorMoved` storage; and the real
   three-band render with the overlay/modal host (DEC-005).
6. **No `SPEC.md` changes** were made ([RUST-043]); behaviour is mapped
   from existing requirements only.

---

## TASK-023a view/lifetime verification log

**Scope:** the VIEW half of TASK-023 - `view()`, the view-model lifetime
resolution and the overlay z-order selector. The update reducer is a
separate chunk.

### Lifetime resolution (the E0515 problem from note 4 above)

RESOLVED as recommended: **the view-models are stored on `AppState` and
rebuilt at the end of `update`.**

- `AppState::models: ViewModels` holds every panel/dialog snapshot.
- `AppState::rebuild_view_models(&mut self)` re-snapshots them via
  `ViewModels::from_app(&app, &overlay)`.
- `view(&AppState) -> Element<'_, Message>` is a pure borrow over
  `&state.models` / `&state.overlay`, so every `Element<'_, _>` the panel
  functions return borrows something that outlives the call.
- `build_state` seeds `models` at construction so frame 0 renders.

No panel needed an owned-element parameter; the stored-VM approach covered
all of them. The three cases `from_app` cannot see on its own are merged
explicitly in `ViewModels::from_app`:

| Field | Merged from | Why |
|---|---|---|
| `left.query` / `is_searching` / `results` | `OverlayState::search` (`SearchViewModel`) | `LeftPanelViewModel::from_app` reads the egui `TreeSearch`, which stays empty in the iced build |
| `agent.thinking_expanded` | `OverlayState::thinking_expanded` | `AgentPanelViewModel::from_app` hardcodes `true` |
| `debug.expanded` | `OverlayState::debug` (`AgentDebugWindowState`) | `AgentDebugViewModel::from_app` returns an empty set |

### VM fields added to `AppState`

`models: ViewModels` with fields `top`, `left`, `center`, `bottom`,
`agent`, `logs`, `debug`, `tools`, `batch`, `about`, `attributions`; plus
the reducer-owned floating-window half `overlay: OverlayState` (small
dialog + anchor, context menu, editor buffer, debug-window disclosure,
tools state, about/attributions state, batch state, search state, cursor,
thinking disclosure and the three scroll snapshots).

### Overlay z-order selector

`mounted_layers(&OverlayState, &ViewModels) -> Vec<OverlayLayer>`
(surface: `AppState::mounted_layers()`) is the single pure selector. It
returns the showing layers **bottom-most first**, mirroring the egui draw
order recorded in `ui/app/render.rs`:

1. `ContextMenu` - only when it has at least one row (an empty menu must
   not mount a click blocker)
2. `Editor`
3. `Dialog` (the small name-entry / move / delete-confirm dialog)
4. `Logs`
5. `AgentDebug`
6. `Tools`
7. `About`
8. `Attributions`
9. `Batch` - topmost (egui drew it last)

`overlay_host` folds `ContextMenu` into the base with
`overlay::with_context_menu` (that helper owns the click-away blocker plus
the anchored-card composition the tree and tab menus need) and stacks every
higher layer with `iced::widget::stack`, bottom-most first. Nothing open ->
the bare three bands.

### Band assembly (one-to-one with `ui/app/render.rs`)

- top band: `panels::top::top_bar(&models.top)`
- middle band: `layout::middle_band` over the [TASK-010] pane grid -
  left = `panels::left::left_panel(&models.left)`, center = the routed
  center body, right = the ToC body
- bottom band: `panels::bottom::bottom_panel(&models.bottom)`
- center routing mirrors egui's `show_center_panel`: agent session first
  (`agent_panel_state.show_results` -> `agent_transcript_view`), then the
  open document through the [TASK-015] markdown render path, then the
  empty state.
- The right pane is gated by the reused `layout::should_show_panel`
  (`has_toc && has_selected_file`); a gated-out pane stays allocated as an
  empty container so the pane grid keeps ownership.
- DEC-008 scale and the close/save path are untouched (they live in
  `app.rs` and the update half).

### NOTE TO THE UPDATE-HALF SUBAGENT

**`update()` MUST call `state.rebuild_view_models()` at its end.** It is
already wired into `finish_frame` (`update.rs`), which runs after every
reducer branch. Any new entry point that mutates state and returns without
going through `finish_frame` will render a stale frame. The order inside
`finish_frame` matters and mirrors egui: `handle_file_selection` ->
`join_finished_batch` -> `rebuild_view_models` -> follow-the-stream scroll
tasks -> persisted panel-width sync.

### Gate results

| Gate | Result |
|---|---|
| `cargo check -q` | PASS, clean |
| `cargo clippy -p fastmd --bins --lib -- -D warnings` | PASS, clean |
| `cargo fmt --check` | PASS, clean |
| `cargo doc --no-deps --quiet` | PASS, no warnings |
| `cargo nextest run -p fastmd` | 2218 run: **2212 passed, 6 failed, 1 skipped** |
| `cargo nextest run -p fastmd ui_iced::view` | 15 run: **15 passed** |

The 6 remaining failures are all `ui_iced::update::tests` reducer gaps,
NOT view-half defects:

| Failing test | Diagnosis |
|---|---|
| `test_open_in_editor_lifts_the_text_buffer_into_an_editor_view_model` | `open_editor_from_buffer` rebuilds raw text through `DocumentContent: Display`, which emits a newline between front matter and body: `"\nBody text\n"` vs expected `"Body text\n"` |
| `test_run_as_prompt_reads_the_body_and_runs_it_against_the_file` | same leading-newline class in the run-as-prompt body extraction |
| `test_background_process_file_load_populates_the_document` | same class: `"\nLoaded body"` vs `"Loaded body"` |
| `test_file_change_inbound_drains_the_file_processor` | `Inbound(FileChanged)` does not run the file-processor drain the egui loop ran per frame |
| `test_batch_open_clamps_a_selection_that_no_longer_resolves` | `BatchDialogState::open` does not clamp a stale `selected_dir_idx` to a shrunken candidate list (`None` vs `Some(0)`) |
| `test_tree_context_menu_is_built_from_the_row_and_anchored_at_the_cursor` | tree context-menu construction, asserts at `update_tests.rs:512` |

### Incidental fixes made to get the gates green (update-half files)

The update-half's `update_tests.rs` did not compile, which blocked the
view tests from running at all. Mechanical fixes only - no reducer
semantics were changed:

- added the missing `BatchDialogViewModel` import;
- `state.app.rebuild_view_models()` -> `state.rebuild_view_models()`;
- filled the three missing `SearchResultEntry` fields (`file_name`,
  `relative_path`, `line_number`);
- `ThemeMessage::ScaleChanged { value }` -> `{ multiplier }`;
- `task.is_empty()` -> `task.units() > 0` (iced 0.14 `Task` has no
  `is_empty`);
- replaced the broken `unsafe_handle` helper with `finished_batch_handle`,
  which spins on `JoinHandle::is_finished` so the frame-tail join test is
  deterministic instead of racing its own thread;
- two tests that asserted the view-model snapshot contract incorrectly now
  rebuild first, or toggle `auto_scroll` explicitly (egui only ever
  changes `auto_scroll` through its checkbox,
  `ui/background_logs.rs:115-118`);
- clippy: `&PathBuf` -> `&Path` (x2), collapsed the `if let` chain in
  `view.rs`, merged the identical branches of `sanitise_scale`;
- rustdoc: de-linked private/unresolved items in the `update.rs` module
  doc and fixed the `ViewModels` intra-doc link in `view.rs`.

### Limitations

- The 6 update-half reducer failures above leave the workspace test gate
  red; fixing them means implementing reducer behaviour, which is outside
  this chunk's mandate.
- `document_width` uses the reducer-held `PanelLayout` widths, not live
  pane-grid geometry (the grid reports real geometry only during layout),
  so the FTWA solve is an approximation until a resize-reporting task
  exists.
- `view` is verified by construction plus smoke tests; iced elements are
  not inspectable without a renderer, so z-order is asserted through the
  pure `mounted_layers` selector rather than a rendered tree.
- `main.rs` is NOT flipped (TASK-024). No branches created, nothing
  committed.

## TASK-023b update-reducer verification log

**Scope:** the UPDATE half of TASK-023 — `update(&mut AppState, Message) ->
Task<Message>` as the one-to-one mirror of `FastMdApp::update_ui`
(`src/app/ui/app/update.rs`), plus the per-channel reducer tests. No branches
created, nothing committed, `main.rs` untouched.

### Frame shape

`update` is **channel reducer -> `finish_frame` -> `Task::batch`**. Every
branch falls through `finish_frame`, so the view never renders stale. The tail
order mirrors egui:

1. `refresh_tree_cache` — the egui tree-row cache transition
   (`ui/panels/left.rs:197-212`).
2. `AppOrchestrator::handle_file_selection` — kick the selection load off
   first so a change is in flight before the snapshot.
3. `join_finished_batch` — egui `handle_deferred_actions`.
4. `rebuild_view_models` — re-snapshot every panel/dialog VM.
5. `follow_scroll_tasks` — stick-to-bottom ops the `logs::should_follow` rule
   decided for the logs / agent-transcript / debug lists.
6. `sync_persisted_panel_widths` — egui `update_persisted_ui_state`.

### Per-channel update mapping

| Channel | Reduction | Runtime `Task`s |
|---|---|---|
| `Window` | `CloseRequested` -> chained `window::size` + `window::position` -> `CloseSnapshotCaptured`; `CloseSnapshotCaptured` -> width sync + `persisted::apply_close_metrics` + `persisted::save_ui_state(storage)`; `ScaleBaseline` -> store `os_scale` (DEC-008); `Resized` -> store `viewport` | `window::close` on the save-before-close phase |
| `Layout` | `PaneResized` -> `layout::apply_pane_resize` (UI-016 clamps) -> `panes.resize` + persisted width sync; `TreeAutoFit` -> `auto_fit_tree` (`heuristic_width` measurer, `set_width`, persisted sync, `panes.resize`); `CursorMoved` -> `overlay.cursor` (the menu anchor) | none |
| `Command` | `route_command` — see the routing table below | none (synchronous publish + drain) |
| `Tree` | `ContextMenuOpened` -> rows from `LeftPanelViewModel::menu_items_for`, anchored at the last cursor; `ContextMenuClosed` -> clear; `RunAsPromptRequested` / `SkillRunRequested` -> read + publish `RunSkillPrompt`; `PrintRequested` -> background print job; `DeleteSelectedRequested` -> one `Delete` per path + clear the selection (un-confirmed, egui parity) | none |
| `Tab` | `ContextMenuOpened` -> `CenterPanelViewModel::menu_items_for(index)`; `ContextMenuClosed` -> clear | none |
| `Editor` | `SaveRequested` -> write the re-joined document, publish `FileEvent::updated_one`, close; `CancelRequested` -> close + clear `text_buffer.is_open` AND `tabs.loaded_path`; everything else -> `apply_editor_message` onto the buffer | none |
| `Agent` | `InputChanged` -> `agent_panel_state.command_input`; `Submit` -> `submit_commands(running, text)` (RunAgent when idle, `QueueAgentPrompt` when running, always with `ClearCommandInput`); `ThinkingToggled` -> `overlay.thinking_expanded`; `ScrollReported` -> `overlay.agent_scroll` | none |
| `Search` | `QueryChanged` (never scans) / `Submitted` (marks in-progress, drops stale results) / `Cleared` / `ResultsReady` (applied only if the query still matches — a superseded scan is dropped) | `Task::perform` running `find_search_results_scoped` off the UI thread |
| `Logs` | filter / category / auto-scroll / clear written through to the shared `BackgroundLogs` ring buffer; `ScrollReported` -> `overlay.logs_scroll` | none |
| `Theme` | `ScaleChanged` -> `sanitise_scale` (near-unity and out-of-range collapse to `None`) -> `persisted_ui_state.font_size_scale` | none |
| `Render` | `LinkActivated` -> `resolve_link` -> `SelectFile` + anchor / `scroll_to_header_id` / external browser; `TaskToggled` -> in-memory document only (no write); `CopyCodeRequested` -> clipboard | clipboard write |
| `Dialog` | name / destination / submit / cancel / dismiss; a validated submit publishes the self-contained `confirmed_command` and closes, an invalid one keeps the dialog open with the error | none |
| `About` | copy hash -> clipboard; repository opened -> `os_shell::open_url`; else `overlay.about.apply` | clipboard write |
| `Batch` | `overlay.batch.apply` (the open transition is driven from `Command(OpenBatchDialog)`) | none |
| `Tools` | `overlay.tools.apply` | none |
| `AgentDebug` | `CopyJsonRequested` -> clipboard; `ScrollReported` -> `overlay.debug_scroll`; else `overlay.debug.apply` | clipboard write |
| `Inbound` | `ConfigArrived` -> the full `drain_config_bus` projection; `FileChanged` -> **apply the carried event** + the `DirectoryTracker` drain; `Agent` -> `drain_agent_event_bus`; `Background` -> `handle_fs_event` / `handle_process_event` / `handle_mcp_auth_event`; `EventsDropped` -> warn + `LAG_TRUNCATION_MARKER` on the agent stream | none |
| `Boot` | `FontLoadFinished(Some)` -> `ERROR` (`ui_iced.boot.font_load_failed`, non-fatal); `ConfigPublished` -> debug trace | none |

### UserCommand routing — bus publish vs. UI-side mutation

| `UserCommand` | Published to `user_command_bus` | Also mutates iced UI state |
|---|---|---|
| `Delete(path)` | **No** — intercepted before the executor | opens `DialogKind::DeleteConfirm`; the intent is published later by `reduce_dialog` once confirmed |
| `Rename(path)` | Yes (publish + drain) | opens the rename dialog pre-filled with `flatten::initial_rename_value` |
| `Move(path)` | Yes | opens the move dialog with `visible_directories()` (the visible tree rows) as candidates |
| `CreateDirectory { parent }` | Yes | opens the name-entry dialog |
| `CreateDocument { parent }` | Yes | opens the name-entry dialog |
| `OpenBatchDialog` | Yes | **re-indexes the previous directory / prompt picks by value** into the freshly built candidate lists (`reindex_batch_selection` + `index_of_selected`), then opens |
| `OpenToolsDialog` | Yes | opens the tools window |
| `OpenAboutDialog` | Yes | opens the About window |
| `CancelBatch` | Yes | dismisses the batch dialog |
| `ClearDebugEntries` | Yes | `overlay.debug.collapse_all()` — row ids are positional, a stale expansion would open an unrelated entry |
| `OpenInEditor(path)` | Yes | lifts the freshly populated `TextBuffer` into an `EditorViewModel` |
| everything else | Yes (publish + immediate drain/execute) | — |

Every publish goes through `publish_and_drain` ->
`AppOrchestrator::drain_user_command_bus` -> `apply_user_command`, i.e. the
same executor the egui frame loop drove ([FMD-066], [FMD-071]).

### The six reported gaps

**1. Leading-newline class (3 tests) — NOT a reducer gap; the tests were wrong.**

Verified against every egui original. `ui/app/update.rs` contains **no body
extraction at all** (read in full: 321 lines), so there is no egui "strip" to
mirror. What egui actually does:

* editor body — `TextBuffer::open` stores `DocumentContent::parse(raw).body`
  verbatim (`ui/text_buffer.rs:302-304`), leading newline kept;
* run-as-prompt — `ui/tree/render.rs:145` publishes
  `DocumentContent::parse(&raw).body` verbatim;
* `FileLoaded` -> viewer — `orchestrator.rs:537-539` stores
  `Document::new(content).body()`, which keeps the same newline.

Stripping would also have been actively harmful for the editor:
`DocumentContent`'s `Display` joins front matter and body with **no separator**
(`markdown/document.rs:64-72`), so a stripped body would write
`---\ntitle: Hi\n---Body text` back to disk and break the closing fence for
every other consumer. The old test additionally asserted
`vm.front_matter() == Some("title: Hi")`, which contradicts `EditorViewModel`'s
own pinned contract (`editor_tests::open_shows_only_the_body_and_caches_the_front_matter`
pins `Some("---\ntitle: Test\ntags: [a, b]\n---")`).

**Fix:** the three assertions now expect the egui-exact values
(`"\nBody text\n"`, `"\nSummarise this\n"`, `"\nLoaded body"`) and the fenced
front-matter form. A new test,
`test_editor_open_then_save_round_trips_the_file_byte_for_byte`, pins open ->
save byte identity — the property the strip would have destroyed. The tests were
corrected, not weakened: the contract under test (body extracted, front matter
preserved, file unchanged) is now asserted more strongly.

**2. `Inbound(FileChanged)` did not run the file-processor drain — REAL GAP, fixed.**

The arm called `process_file_events()` and **ignored the event the message
carried**. That only "worked" in production by accident: it relied on the
orchestrator holding a *second* live subscriber on `Bus<FileEvent>`, which
contradicts [FMD-065]'s single-inbound-path rule that already gave
`orchestrator.rx` an inert channel.

**Fix:** extracted the per-event match out of
`AppOrchestrator::process_file_events` into a new
`pub fn apply_file_event(&mut self, FileEvent)` (behaviour preserved
arm-for-arm), and the reducer now applies the carried event directly and then
calls `process_file_events()` to keep the `DirectoryTracker` drain the egui
frame ran alive. `build_state` sets `file_event_reader: None`, so exactly one
application path exists and no event is applied twice.

**3. `BatchDialogState::open` clamp — the test contradicted the module's own pinned contract.**

`update_tests` wanted a stale `Some(1)` against a one-element list to become
`Some(0)`; `batch_dialog_tests::open_clamps_selections_that_no_longer_resolve`
pins the opposite for the same function (`Some(5)` against one dir -> `None`,
"a directory index past the fresh list must be dropped"). Both cannot hold.

**Fix:** the update test now asserts the pinned `None` (and that
`can_start()` is disabled). Clamping to the last candidate would silently
select a directory the user never picked, which for a destructive batch run is
the wrong failure mode. The reducer-side obligation the task actually calls for
— re-index by path — was **missing** and is now implemented (see gap 3b).

**3b. Batch re-index-by-path — implemented.** `Command(OpenBatchDialog)` now
runs `reindex_batch_selection(previous, fresh)`, which re-finds the previous
directory pick with `index_of_selected` and the previous prompt pick by its
`path` (`PromptInfo` is not `PartialEq`), so a reopen follows the *value* the
user chose rather than a stale position (egui's rule,
`ui/app/render.rs:102-108`). A pick that no longer resolves is left alone so
`open` drops it. Covered by
`test_open_batch_dialog_reindexes_the_previous_pick_by_path` and
`test_open_batch_dialog_drops_a_previous_pick_that_is_gone`.

**4. Tree context-menu construction — the test seeded the wrong layer, and a real perf gap surfaced.**

The test seeded `models.left.rows`, then dispatched a `CursorMoved` — whose
`finish_frame` rebuild re-derived the rows from the real filesystem and wiped
the seed before the menu message was reduced. The sibling
`test_tree_context_menu_for_an_unknown_row_mounts_nothing` requires an unknown
path to produce an *empty* menu, so synthesising menu rows from the path alone
cannot satisfy both.

**Fix (test):** `seed_rows` now seeds the way the reducer does — into
`FastMdApp::cached_tree_rows` with `tree_dirty` cleared, exactly egui's cache
transition (`ui/panels/left.rs:209-210`) — so the seeded rows survive the
rebuild.

**Fix (production, found while diagnosing):** nothing in the iced build ever
populated `cached_tree_rows`, so `panels::left::tree_rows` fell through to
`build_workspace_tree` and re-walked the whole workspace on **every message**.
`finish_frame` now runs `refresh_tree_cache`, the exact egui transition: rebuild
only while dirty, publish the rows as the cache, clear the flag. Covered by
`test_frame_tail_populates_the_tree_cache_and_clears_the_dirty_flag`,
`test_frame_tail_does_not_rebuild_the_tree_while_it_is_clean` and
`test_a_dirty_tree_is_re_derived_rather_than_served_from_the_stale_cache`.

### Deferred-intent execution path

No file I/O happens in the view; the reducer owns it and reuses the existing
orchestrator machinery:

* **Tree run-as-prompt / skill run** — the reducer reads the file
  (`std::fs::read_to_string`), parses the body and publishes a *self-contained*
  `UserCommand::RunSkillPrompt { content, target_dir, target_file }`
  ([FMD-068]). The read is the command's **input**, not its execution — no
  `UserCommand` variant exists for "read this file and hand me the body", and
  the egui original read in the menu callback too. A read failure logs
  `ui_iced.file.read_failed` (NFR-001/NFR-004) and publishes nothing.
* **Print** — there is no `UserCommand::Print`, so the reducer owns the job:
  `export::print::execute_print_blocking(PrintJob::new(path), Some(tx))` hands
  it to the background channel; failure logs
  `ui_iced.file.print_no_channel`.
* **Editor save** — the reducer writes the re-joined document and publishes
  `FileEvent::updated_one` so the tree and tag manager refresh; the executor
  has no editor-save command.
* **Everything else destructive or creating** — publish + drain through
  `AppOrchestrator::drain_user_command_bus` -> `apply_user_command`, the same
  executor the egui frame drove.
* **Clipboard** (copy code, copy commit hash, copy debug JSON) rides iced
  `Task`s returned from the reducer, never a synchronous write.

### egui update behaviour that could NOT be mirrored

1. **`apply_persisted_font_scale` / `persist_font_scale`.** egui mutates
   `ctx.pixels_per_point()` against a first-frame OS baseline. iced 0.14 has no
   mutable ppp; DEC-008 replaces the whole pair with
   `.scale_factor(state.composed_scale_factor())` composed from the boot-
   captured `os_scale` baseline x the persisted multiplier. The near-unity and
   out-of-range collapse semantics are preserved verbatim in `sanitise_scale`.
2. **`should_request_immediate_repaint` and the repaint throttle.** iced is
   event-driven; there is no repaint request and no idle-CPU decider. The
   "file events arrived" half of that decision survives as the
   `Inbound(FileChanged)` trigger; the "raw input pending" half is the iced
   runtime's own event loop.
3. **`handle_platform_commands` / the `OutputCommand::OpenUrl` drain.** iced has
   no platform-output command list. The equivalent intent arrives as
   `RenderMessage::LinkActivated` and runs the identical `resolve_link`
   dispatch (workspace file / anchor / external).
4. **The four unconditional per-frame drains.** They are per-message triggers
   here, so a burst of messages can run a drain more often than once per frame.
   Each drain is idempotent and cheap; the alternative (a frame timer) would put
   a polling loop back into an event-driven runtime.
5. **`ctx.request_repaint()` calls inside `handle_open_link`.** No equivalent
   needed — returning from `update` re-renders.
6. **`WindowMessage::CloseRequested`'s phase-1 task chain** needs a live iced
   runtime (`window::size` / `window::position`), so unit tests drive
   `CloseSnapshotCaptured` directly, per [TASK-009]'s testing rule 3.

### Gate results

| Gate | Result |
|---|---|
| `cargo check -q` (workspace) | PASS, clean |
| `cargo clippy -p fastmd --bins --lib -- -D warnings` | PASS, clean |
| `cargo fmt --check` | PASS, clean |
| `cargo doc --no-deps --quiet` | PASS, no warnings |
| `cargo nextest run -p fastmd` | 2225 run: **2225 passed, 1 skipped** |
| `cargo nextest run --workspace` | 3362 run: **3362 passed, 4 skipped** |
| `cargo nextest run -p fastmd ui_iced::update` | 90 run: **90 passed** (was 83 with 6 failures; 7 tests added) |

### Needs the orchestrator's eyes

1. **`AppOrchestrator::process_file_events` was refactored.** The per-event
   match moved into a new `pub fn apply_file_event(&mut self, FileEvent)`. The
   arms are preserved, but two accumulators became per-event instead of
   per-batch: `close_tabs_for_removed_files` and `tags.rebuild()` now run once
   per `Removed` event rather than once after a whole drain. Both are idempotent
   and the egui tests pass unchanged, but if a bulk-removal path ever shows a
   rebuild cost this is the place to re-batch.
2. **`build_state` now sets `file_event_reader: None`** (mirrors the inert
   `rx`). The iced build's only file-event application path is
   `Inbound(FileChanged)` -> `apply_file_event`. Anything that later needs the
   orchestrator to observe file events independently of the [TASK-019] bridge
   must be re-wired deliberately — and then the reducer must stop applying the
   carried payload, or events double-apply.
3. **`apply_file_event` is new public API** on `AppOrchestrator` and is part of
   the iced reducer's contract.
4. **`DeleteConfirm` asymmetry is unchanged:** the single-path context-menu
   delete is confirmed, the multi-select `DeleteSelectedRequested` fan-out is
   not (egui parity; `DialogKind::DeleteConfirm` carries one path). A
   multi-select confirm dialog is a product decision, not a migration item.
5. **`main.rs` is NOT flipped (TASK-024).** No branches created, nothing
   committed.

---

## TASK-024 entry-flip verification log (default entry point -> iced - 2026-10-18)

**What `main.rs` now calls.** `fn main() -> iced::Result` ends in
`fastmd::ui_iced::app::launch(inputs)` — the same entry `bin/fastmd_iced.rs`
uses ([TASK-022]). `eframe::run_native` is gone, and with it the
`use eframe::egui`, `crate::ui::app::FastMdApp` and
`crate::bus::events::ConfigArrived` imports (the only remaining mentions of
`egui`/`eframe`/`ConfigArrived` in `main.rs` are prose in the `//!` header).
Plain `cargo run` / `cargo run --bin fastmd` yields the iced UI — no feature
flag (DEC-011).

**Entry chain (top to bottom).**

```
main() -> iced::Result
 ├─ #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
 ├─ #[global_allocator] MiMalloc
 ├─ std::panic::set_hook(...)                       (verbatim, inline)
 ├─ logs::init_logging(logs::get_log_dir())         (guard held for the whole run)
 ├─ rustls::crypto::ring::default_provider().install_default()
 ├─ config     = fastmd::config::load_config()
 ├─ config_bus = fastmd::bus::config::config_bus()
 ├─ #[cfg(feature = "discord")] tokio::spawn(run_discord_bot(..))
 └─ app::launch(LaunchInputs {
      config,
      config_bus,
      config_storage:   Arc<FileConfigStorage::new(config::get_config_path())>
                        as Arc<dyn ConfigStorageHandler>,
      ui_state_storage: ui_state_storage_for(resolve_ui_state_source(
                            env FASTMD_ICED_UI_STATE_PATH )),
      args:             std::env::args().collect(),
    })
```

**Startup steps PRESERVED in `main.rs`** (iced `boot`/`launch` do NOT do these):
the `windows_subsystem` attribute; the `MiMalloc` global allocator; the panic
hook (structured `tracing::error!` with location + `Backtrace::force_capture`,
[NFR-004]/[NFR-005]); non-blocking background logging init with the guard
held for the session ([NFR-002]); the `rustls` ring crypto-provider install;
the `discord`-feature bot spawn; and `load_config()` + `config_bus()` handle
creation.

**Startup steps MOVED (confirmed already in `boot`/`launch`, NOT duplicated in
`main.rs`):** CLI workspace-argument injection and the system-library-only ->
cwd fallback (`apply_cli_workspace`, `ui_iced/app.rs:805-835`); the DEC-009
`ConfigArrived` publish (`publish_config_task`, `ui_iced/app.rs:852-864`);
font registration ([TASK-007]) and the DEC-008 scale-baseline tasks; the
[TASK-019] subscription bridge; window title `strings::WINDOW_TITLE`
("FastMD AI", DEC-010), icon, persisted geometry and
`exit_on_close_request: false` (`window_settings`, DEC-006); and the exit
cleanup (`export::print::cleanup_temp_files`, `logs::unregister_ui_logs`)
that runs after `.run()` inside `launch`.

**ConfigArrived workaround REMOVED.** The old `main.rs:140-162`
"build `FastMdApp` inside the `eframe` creation callback, *then* publish"
block is deleted. The ordering guarantee it enforced now comes from `boot`:
the state — background `Task` thread, MCP config subscriber, eager
`BusReceivers` — is fully constructed before `launch` hands the
`Task::perform` to the iced runtime, so every subscriber exists when the
publish runs. Verified by reading `boot` / `boot_with_persisted` /
`publish_config_task` in `src/app/ui_iced/app.rs:805-864`.

**New production wiring: the persisted UI-state storage.** The pilot kept
snapshots in-process; the shipping entry needs real persistence for DEC-006
geometry restore. `main.rs` resolves it through two small pieces:

| Piece | Contract |
| --- | --- |
| `resolve_ui_state_source(Option<String>) -> UiStateSource` | pure decision: a non-empty `FASTMD_ICED_UI_STATE_PATH` wins (`Override(PathBuf)`); empty or absent -> `PlatformDefault` |
| `ui_state_storage_for(UiStateSource) -> Arc<dyn UiStateStorage>` | thin mapper: `FileUiStateStorage::new(path)` vs `FileUiStateStorage::platform_default()` (`%APPDATA%\fastmd\ui_state.json`, `~/.fastmd/ui_state.json`) |

The decision is kept apart from construction so the persistence-location
policy is unit-testable without tripping the platform-default guard in
`FileUiStateStorage::platform_default` ([RUST-006]); the platform-default
path is resolved only in production `main()` ([RUST-024]).

**Files changed.** `src/app/main.rs` (entry rewritten, `//!` header now
describes the iced launch) and `src/app/main_tests.rs` (new sidecar).
`bin/fastmd_iced.rs` is left in place (removed in [TASK-029]); the egui
`ui/` tree is untouched and still compiles; `Cargo.toml` is untouched
(`eframe`/`egui` stay until [TASK-029]); no `SPEC.md` changes ([RUST-043]).

**Tests added** (`src/app/main_tests.rs`, reported as
`fastmd::bin/fastmd tests::*`): `non_empty_override_selects_the_explicit_file`,
`empty_override_is_not_an_override`, `missing_override_selects_platform_default`,
`override_source_builds_a_file_storage_at_that_path` (`tempfile::TempDir`
round-trip, [RUST-006]) and `entry_target_is_the_iced_launch_path`
(compile-time fn-pointer pin `fn(LaunchInputs) -> iced::Result = app::launch`).
No GUI-launch test was invented — `main()`/`launch()` call `.run()` and are
not headlessly testable.

**Gate results.** `cargo check -q` (workspace) PASS (no output);
`cargo clippy -p fastmd --bins --lib -- -D warnings` PASS (exit 0);
`cargo fmt` run, `cargo fmt --check` PASS (exit 0);
`cargo nextest run -p fastmd --status-level fail --show-progress none` PASS
(**2230 run / 2230 passed / 1 skipped** — was 2133 at [TASK-022]; +5 are
this task's `bin/fastmd` sidecar tests, the remainder [TASK-023]'s);
`cargo build --bin fastmd` PASS (exit 0 — the default binary links the iced
runtime); `cargo doc --no-deps --quiet -p fastmd` PASS; and
`cargo check -p fastmd --features discord --bins` PASS (the cfg-gated Discord
block still compiles after the flip).

**[UNVERIFIED] runtime checklist — needs a human `cargo run`.** The GUI cannot
be exercised headlessly, so all of the following are **[UNVERIFIED]** and must
be confirmed by running `cargo run` (== `cargo run --bin fastmd`):

1. [UNVERIFIED] An iced window opens titled **"FastMD AI"** (DEC-010), and a
   release build shows no console window.
2. [UNVERIFIED] The FastMD app icon shows (title bar / taskbar).
3. [UNVERIFIED] Persisted window geometry restores: resize/move, close,
   relaunch -> same size and position (`%APPDATA%\fastmd\ui_state.json`
   written on close, read at launch).
4. [UNVERIFIED] The three-band layout renders with the top / left / center /
   bottom panels, and pane resizing works.
5. [UNVERIFIED] Close actually exits (DEC-006 `exit_on_close_request: false`
   + save-then-close) and the process terminates instead of hanging.
6. [UNVERIFIED] Background workers start scanning after the boot publish —
   look for `ui_iced.boot.config_published` in the log.
7. [UNVERIFIED] The CLI workspace argument still works:
   `cargo run -- <dir>` adds `<dir>` as a "Workspace" library.
8. [UNVERIFIED] `FASTMD_ICED_UI_STATE_PATH=<scratch file>` redirects the
   snapshot away from `%APPDATA%`.

**Needs the orchestrator's eyes.**

1. **Pre-existing `discord` defect preserved, NOT fixed.** The cfg-gated block
   calls `tokio::spawn` from `main` with no tokio runtime entered
   (`run_discord_bot` is an `async fn`; nothing here builds a `Runtime`).
   [INFERENCE] With `--features discord` and a bot token set this panics with
   "there is no reactor running". Identical in the old egui `main.rs`, so the
   flip introduces no regression — wrapping it in a runtime (as
   `bus/router/worker.rs:25` does) is a separate fix.
2. **`FASTMD_ICED_UI_STATE_PATH` now steers the shipping binary too.** Kept
   as an escape hatch so a manual run can dodge `%APPDATA%`; if production
   should ignore it, delete the `Override` branch of `resolve_ui_state_source`.
3. **`_log_guard`'s lifetime is load-bearing** — `launch()` blocks, so the
   guard lives for the whole session. A future refactor that moves `launch()`
   off the main thread must move the guard with it.
4. **`bin/fastmd_iced` still exists** and now duplicates `main.rs`'s boot
   preamble (panic hook + logging + rustls). Both build; the duplication dies
   with the pilot in [TASK-029].
5. **No branch created, nothing committed** — left for the orchestrator.


### TASK-029a self-containment verification log (ui_iced owns its pure helpers - 2026-10-18)

**Scope delivered: `ui_iced` no longer depends on any egui file [TASK-029b]
deletes.** Every framework-free helper the iced layer `pub use`d or called
across the boundary out of `src/app/ui/` has been **copied into the iced
module that uses it**. No file under `src/app/ui/` was modified — the egui
tree keeps its own copies and still builds, so the duplication is transient
and dies with the egui layer.

**Copy map (helper → new iced home).**

| # | Old location (DELETE in 029b) | Helper(s) | New iced home | egui-free? |
| --- | --- | --- | --- | --- |
| 1 | `ui/panels/right.rs:11-17` | `should_show_panel` | **new** `ui_iced/panels/right.rs`; re-exported from `ui_iced/layout.rs` so the [TASK-010] public path `layout::should_show_panel` is unchanged | Yes — `(bool, bool) -> bool` |
| 1 | `ui/panels/right.rs:78-110` | `resolve_anchor` | **new** `ui_iced/panels/right.rs`; `update.rs` now calls `ui_iced::panels::right::resolve_anchor` | Yes — `&[ToCEntry] + &[String] + &str -> String` |
| 2 | `ui/panels/left.rs:18-133` | `build_workspace_tree` + private `prune_empty_dirs`, `library_for_path`, `insert_path_into_tree` | `ui_iced/panels/left.rs` (called by `tree_rows`) | Yes — `TreeNode` / `ContentLibrary` / `Path` only. `TreeNode` itself lives in `ui/app/mod.rs` (KEPT) and is imported, not copied |
| 3 | `ui/panels/bottom.rs:44-142` | `compute_prompt_prefix`, `compute_file_context`, `format_models_list`, `parse_command_intent`, `clear_command_input_command` | `ui_iced/panels/bottom.rs` | Yes — `Path` / `String` / `HashMap<String, LlmConfig>` / `UserCommand` only |
| 4 | `ui/panels/center.rs:14-64` | `clear_agent_session_command`, `apply_tab_close_click`, `apply_tab_close_others_click`, `apply_tab_close_all_click` | `ui_iced/panels/center.rs` | Yes — pure `usize`/`() -> UserCommand` constructors |
| 5 | `ui/panels/top.rs:33-121` | `get_tag_filter_text`, `compute_next_selected_file`, `apply_batch_button_click`, `apply_tools_button_click`, `apply_about_button_click`, `apply_background_logs_toggle`, `apply_agent_debug_toggle`, `apply_chat_model_selection`, `apply_table_width_strategy_change`, `strategy_label` | `ui_iced/panels/top.rs` | Yes — pure `UserCommand` constructors and pure path/tag/label solvers |
| 6 | `ui/table_width/mod.rs:29-86` | `TablePadding` (+ `ZERO`, `sanitised`, `horizontal`, `vertical`, `Default`) | `ui_iced/table_width.rs`; `render/mod.rs` and `render/table.rs` re-pointed at it | Yes — four `f32` fields, four pure methods |
| 7 | `ui/widgets.rs:160-169` | `DialogAction` | `ui_iced/overlay.rs` (beside `ConfirmCancel`, whose `to_dialog_action` still maps onto it) | Yes — three unit variants |
| 8 | `ui/os_shell.rs:9-31` | `open_in_system_editor`, `show_in_file_explorer`, `open_url` | **new** `ui_iced/os_shell.rs`; `update.rs` now imports `ui_iced::os_shell` | Yes — `opener` wrappers over `&Path` / `&str` |
| 9 | `ui/test_helpers/app.rs:14-51` | `test_app`, `test_app_with_config` (+ `test_app_with_api_url` for parity) | **new** `ui_iced/test_helpers.rs` (`#[cfg(test)]`); the four panel sidecars re-pointed at it | Yes — route through `FastMdApp::empty_state`, which needs no `CreationContext` and applies no egui visuals |

**Placement decision the brief asked for (item 1).** `should_show_panel` and
`resolve_anchor` went into a **new `ui_iced/panels/right.rs`** rather than
`layout.rs`. Rationale: `resolve_anchor` is a ToC/heading-resolution rule,
not pane-grid geometry ([RUST-051]: place by concern); `panels/mod.rs`
already anticipated a `right` submodule; and `layout.rs` re-exports
`should_show_panel`, so `view.rs` and `layout_tests.rs` needed **zero**
call-site changes. The cost is one `layout → panels::right` module edge,
which is legal and unremarkable inside one crate.

**Symbols deliberately NOT copied.**

| Symbol | Why not |
| --- | --- |
| `ui/panels/bottom.rs::is_enter_pressed` | Takes `&egui::InputState` — egui-bound. Also already irrelevant: iced's `text_input` publishes `on_submit` from its own key handling, so there is no second IME delivery path to reconcile (the [TASK-016] gap note). |
| `ui/panels/top.rs::build_indexing_status_text` | Returns `egui::RichText`. Cannot be relocated egui-free. The iced panel already owns its own `indexing_status_text` re-typing. |
| `ui/panels/center.rs::apply_task_toggles_command` | egui-free, but **no `ui_iced` code reaches it** — the iced render path does not yet emit task-checkbox intents. Copying it would add unused public API. |
| `ui/table_width/mod.rs::TableRenderConfig` | egui-free, but **no `ui_iced` consumer**: the iced render path carries `RenderContext::table_padding` instead. Not copied. |
| `ui/table_width/mod.rs::resolve_padding`, `measure`, `measure_cached`, `table_layout_cached`, `EguiTextMeasurer`, `TableMeasureCache`, `TableDecisionCache` | egui-bound (`&egui::Ui` / `&egui::Context`), or already replaced iced-side by `IcedTextMeasurer` ([TASK-015]). |
| `ui/os_shell.rs::dispatch_platform_commands` | Takes `&[egui::OutputCommand]` — the egui platform-output bridge, meaningless without egui. The iced link path calls `os_shell::open_url` from the reducer instead ([TASK-020] note). |
| `ui/panels/right.rs::calculate_indent`, `calculate_font_size`, `apply_toc_row_click` | egui-free but **no `ui_iced` consumer** — `view::toc_body` uses its own `TOC_LEVEL_INDENT_PIXELS` rule and emits `ScrollToHeader` directly. |
| `ui/test_helpers/app.rs::noop_producer` | No `ui_iced` consumer. |

**Remaining `crate::ui::` imports — and why each is safe.**

*Explicitly KEPT by the brief (RELOCATE / RESIDUE / RE-TYPE, not DELETE):*

| Import | Used by | Why safe |
| --- | --- | --- |
| `crate::ui::strings` | ~every iced module | The single UI-text home (AGENTS §4). Survives the migration. |
| `crate::ui::theme` (`Palette`) | ~every iced module | The single colour-token home (AGENTS §4a); `ui_iced/theme.rs` maps it into iced. |
| `crate::ui::app::FastMdApp` (+ `TreeNode`) | panels, agent, logs, test_helpers | The domain state object itself. |
| `crate::ui::{Dialogs, FileSelection, PanelLayout, PersistedUiState, Tabs, TextBuffer}` | `app.rs`, `persisted.rs` | Domain/manager types re-exported from `ui/mod.rs`; `panel_layout` and `persisted` are KEPT. |
| `crate::ui::panel_layout::{LEFT/RIGHT_PANEL_*}` | `layout.rs`, `panels/left.rs`, `view.rs` | The UI-016 width constants. |
| `crate::ui::tree::{FlatRow, flatten_tree}`, `tree::flatten::FlatRow` | `panels/left.rs`, `update_tests.rs` | The UI-017/UI-018 ordering engine. |
| `crate::ui::tree_search::*` | `search.rs`, `message.rs`, `update.rs` | The UI-050 search engine. |
| `crate::ui::agent::*` (`AgentPanelState`, `AgentTranscript`, `TranscriptBlock`, `ConversationLoggerObserver`) | `app.rs`, `agent/*` | The reused agent session state. |
| `crate::ui::attributions::{Attribution, DIRECT_DEPENDENCIES}` | `about.rs`, `attributions.rs` | The curated credits catalog. |
| `crate::ui::PersistedUiState`, `crate::ui::CURRENT_SCHEMA_VERSION` | `persisted.rs`, `app.rs`, tests | The DEC-006 snapshot schema. |

*NOT named in the brief's KEPT list and NOT a copy target — flagged for the
orchestrator to classify before 029b runs:*

| Import | Used by | Note |
| --- | --- | --- |
| `crate::ui::{resolve_link, LinkAction}` (`ui/link_resolver.rs`) | `update.rs` | The map's TASK-020 audit already ruled this **KEPT**: "zero references to `egui`/`eframe` (grep-ified)… **Not moved, not duplicated**". Safe. |
| `crate::ui::fonts::{PHOSPHOR_REGULAR_FONT, load_fallback_font}` | `ui_iced/fonts.rs` (+ its sidecar) | Embedded font **bytes**, not egui types — `ui_iced/fonts.rs` deliberately reuses the same embedded assets rather than re-embedding them. If `ui/fonts.rs` is DELETE-marked, these two loaders must be relocated first. |
| `crate::ui::about_dialog::{APP_VERSION, license_display_text, apply_first_run_auto_show}` | `about.rs`, `app.rs` (+ sidecars) | Version string, license text and the first-run gate. All egui-free, but not in the copy inventory. If `ui/about_dialog.rs` is DELETE-marked these three need a home (natural fit: `ui_iced/about.rs`). |
| `crate::ui::background_logs::{filter_logs, is_log_visible}` (`pub use`d from `ui_iced/logs.rs`) | `logs.rs` | The UI-047 filter predicates, egui-free. Same situation — relocate before deleting `ui/background_logs.rs`. |

**Nothing in `ui_iced` now imports `ui/panels/*`, `ui/widgets.rs`,
`ui/os_shell.rs`, `ui/table_width/*` or `ui/test_helpers*`.** Verified by
grepping every `.rs` under `src/app/ui_iced/` for
`crate::ui::(panels|widgets|os_shell|table_width|test_helpers)`: the only
remaining hits are inside `//!`/`///` provenance comments, not code.

**Tests added (19, all new; [RUST-003]).**

| File | Tests | Covers |
| --- | --- | --- |
| `ui_iced/panels/right_tests.rs` | 9 | `should_show_panel` truth table; `resolve_anchor` exact / case-insensitive / hyphen⇄space (both directions) / title-vs-id / heading-id fallback / ToC-over-headings precedence / verbatim fallback / empty-input |
| `ui_iced/os_shell_tests.rs` | 4 | Each wrapper's signature pinned to `fn(&Path)` / `fn(&str)` — a compile-time egui-free guarantee; editor vs explorer wrappers proven distinct (`open` ≠ `reveal`). No OS process is launched ([RUST-006]), matching the egui sidecar's own precedent. |
| `ui_iced/test_helpers_tests.rs` | 6 | The egui-free fixture smoke tests: empty workspace, config forwarding, chat-model registration, a full `AppState` build (pane grid + viewport + no premature OS scale), persisted-geometry seeding with the DEC-008 composed scale, and `rebuild_view_models()` running egui-free. |

Existing coverage was **preserved, not duplicated**: `layout_tests.rs`'s
gate test still runs (now against the iced-owned copy), and the four panel
sidecars (`bottom`/`center`/`top`/`left`) exercise the relocated helpers and
the new `ui_iced::test_helpers` fixtures unchanged.

**Gate results.** `cargo check -q` (workspace) PASS (no output);
`cargo clippy -p fastmd --bins --lib -- -D warnings` PASS (exit 0);
`cargo fmt` run, `cargo fmt --check` PASS (exit 0);
`cargo nextest run -p fastmd --status-level fail --show-progress none` PASS
(**2249 run / 2249 passed / 1 skipped** — was 2230 before this chunk; the
+19 are exactly the three new sidecars above); `cargo doc --no-deps --quiet
-p fastmd` PASS (no warnings).

**Files changed.** 17 modified + 6 new, all under `src/app/ui_iced/`:

*New:* `panels/right.rs`, `panels/right_tests.rs`, `os_shell.rs`,
`os_shell_tests.rs`, `test_helpers.rs`, `test_helpers_tests.rs`.
*Modified:* `mod.rs`, `layout.rs`, `layout_tests.rs`, `update.rs`,
`overlay.rs`, `table_width.rs`, `panels/mod.rs`, `panels/left.rs`,
`panels/left_tests.rs`, `panels/center.rs`, `panels/center_tests.rs`,
`panels/top.rs`, `panels/top_tests.rs`, `panels/bottom.rs`,
`panels/bottom_tests.rs`, `render/mod.rs`, `render/table.rs`.
**Zero files under `src/app/ui/` touched.**

**Needs the orchestrator's eyes.**

1. **The four unlisted `crate::ui::` modules above** (`fonts`,
   `about_dialog`, `background_logs`, plus `link_resolver` which the map
   already keeps) are the only thing standing between `ui_iced` and full
   independence from the egui tree. They are all egui-free, so 029b either
   RELOCATEs them or they stay as KEPT residue — but that decision is the
   orchestrator's, not this chunk's.
2. **`TableRenderConfig` / `apply_task_toggles_command` /
   `calculate_indent` / `calculate_font_size` / `apply_toc_row_click` /
   `noop_producer` were NOT copied** because no `ui_iced` code reaches them.
   If a later task wants them iced-side, they are egui-free and trivially
   copyable — but adding them now would be unused public API.
3. **`ui_iced/test_helpers.rs` is `#[cfg(test)]`-gated**, unlike the egui
   `ui::test_helpers` which compiles into the production lib. Deliberate:
   test fixtures should not ship. Consequence — the stated
   `cargo clippy -p fastmd --bins --lib` gate does **not** lint it; it is
   compiled and run by `cargo nextest`.
4. **`cargo clippy -p fastmd --all-targets` is NOT clean**, and was not
   clean before this chunk either: 12 pre-existing lints in
   `config/config_tests.rs`, `markdown/link_scanner_tests.rs`,
   `ui/batch_dialog_tests.rs`, `ui_iced/message_tests.rs`,
   `ui_iced/panels/left_tests.rs` and `ui_iced/render/code_tests.rs`
   (`field_reassign_with_default`, `let_unit_value`, `redundant_closure`,
   `assertions_on_constants`, `cloned_ref_to_slice_refs`, `useless_vec`).
   None are in code this task wrote; the brief's stated gate
   (`--bins --lib`) passes. Flagging so the gate discrepancy is visible.
5. **The `ui_iced.left.build_workspace_tree` tracing span was renamed**
   (was `ui.left.build_workspace_tree`). Both copies now emit under
   distinct span names while they coexist; log dashboards keyed on the old
   name will see the egui span until Phase 5 and the iced span after.
6. **No branch created, nothing committed** — left for the orchestrator.

## TASK-029b egui-deletion verification log (the deletion half - 2026-10-18)

Closes the CON-007 zero-egui invariant inside `src/` and `Cargo.toml`. No
branch created, nothing committed - left for the orchestrator.

### Final sweep result (the invariant itself)

Command (case-insensitive, run from the repo root):

```powershell
Select-String -Pattern '\begui\b|\beframe\b|\bkittest\b' -CaseSensitive:$false
```

| Scope | Matches |
| --- | --- |
| `src/` (all `.rs`) | **0** |
| `Cargo.toml` | **0** |

Baseline before this task was 2267 matches in `src/`; after the code deletion
half it was 765 (comments, doc links and parity notes), and the doc sweep drove
it to zero. The one surviving literal containing the letters is the Windows font
file name `seguisym.ttf` in `ui/fonts.rs`, which `\begui\b` correctly does not
match.

Out of scope for this task and NOT edited (reported for TASK-030):

| Path | Matches |
| --- | --- |
| `doc/technical-context/ARCHITECTURE_C4.md` | 9 |
| `doc/technical-context/Messaging.md` | 2 |
| `src/app/AGENTS.md` ("MUST consult" rules, `egui::Id` section) | 5 |
| `doc/distill/` (`egui.md`, `egui-kittest.md`) | 164 |
| `SPEC.md` (file-map links at the deleted `src/app/ui/` tree) | 3 |

### Files deleted (72)

Legacy rendering layer, all egui- or kittest-owned:

* **Entry / frame loop** - `ui/app/render.rs`, `ui/app/update.rs`
  (`impl eframe::App`, `show_editor_overlay`, `show_modals`,
  `render_panels`).
* **Dialogs** - `ui/about_dialog.rs` (+tests), `ui/batch_dialog.rs` (+tests),
  `ui/tools_dialog.rs` (+tests), `ui/modals.rs` (+tests),
  `ui/agent_debug_window.rs`, `ui/background_logs.rs`.
* **Editor** - `ui/editor_egui.rs` (+tests).
* **Panels** - `ui/panels/{mod,bottom,center,left,right,top}.rs` (+tests).
* **Markdown renderers** - `ui/render/{mod,code,heading,inline,style,yaml_table}.rs`
  (+tests), `ui/render/table/{mod,cell,configured}.rs`,
  `ui/render/e2e_tests/**` (the kittest snapshot suite, 11 files).
* **Snapshot harness** - `ui/test_helpers.rs`, `ui/test_helpers/{app,assert,assert_tests,interact,offscreen,snapshot,text,text_tests}.rs`.
* **Widgets / layout** - `ui/widgets.rs` (+tests), `ui/table_width/mod.rs`,
  `ui/measure.rs` (+tests), `ui/logo.rs`.
* **Tree** - `ui/tree/context.rs` (`egui::Modifiers`), `ui/tree/render.rs`
  (+tests).
* **OS shell** - `ui/os_shell.rs` (+tests).
* **Pilot binary** - `src/app/bin/fastmd_iced.rs` (see below).

### RE-TYPE strategy (files kept, egui types replaced 1:1)

Three files under `ui/` were egui-typed but had live non-UI consumers, so they
were re-typed in place rather than deleted (GUD-003: values must not drift).

| File | Strategy |
| --- | --- |
| `ui/theme.rs` | `Palette` and `EditorColors` re-typed onto `iced::Color`. Every numeric value is identical to the retired `Color32` equivalent (`from_gray(N)` -> `from_rgb8(N,N,N)`; DARK_GRAY `(96,96,96)` -> `log_timestamp`; LIGHT_BLUE `(173,216,230)` -> `log_category`; RED `(255,0,0)` -> `EditorColors.error`). Derives reduced to `Debug/Clone/Copy/PartialEq/Default` because `iced::Color` is not `Eq`. |
| `ui/fonts.rs` | Kept the font byte inventory and the Windows/system font discovery. Dropped the egui-typed loaders (`FontDefinitions`, `FontData`, family insertion); the iced loader lives in `ui_iced/fonts.rs` and shares the same discovery and the same `'phosphor'` family key. |
| `ui/attributions.rs` | The `eframe` / `egui_extras` credit rows were deleted outright rather than renamed - the strings themselves would have matched the sweep. `DIRECT_DEPENDENCIES` is now 55 entries (was 57); `attributions_tests.rs` asserts 55. |

`to_iced_color` was removed entirely (its only job was
`egui::Color32 -> iced::Color`); its 196 call sites across 35 `ui_iced` files
were unwrapped to the bare colour token, which is now the same type on both
sides.

### `eframe::App` removal

* `impl eframe::App for FastMdApp` deleted from `ui/app/mod.rs`.
* `FastMdApp::new(cc: &eframe::CreationContext)`, `configure_dark_theme` and
  `configure_fonts` deleted from `ui/app/init.rs`; `from_parts` (pub(crate))
  and `empty_state` (pub) remain as the framework-free constructors.
* The font-tracking private fields the `App` impl needed are gone.
* `ui/app/render.rs` and `ui/app/update.rs` deleted; a slim framework-free
  `ui/app/tests.rs` replaces the frame-loop tests.
* `main.rs` already launched iced via `ui_iced::app::launch` ([TASK-024]), so
  no entry-point rewiring was needed.

### Pilot binary deletion

`src/app/bin/fastmd_iced.rs` and its `[[bin]]` stanza were removed. Rationale:
`main.rs` has launched the identical iced chain since [TASK-024], so the pilot
was a duplicate launch path, and keeping it would have kept a second
`Message`/`run` surface alive that [TASK-019]/[TASK-022] had already migrated.
The `FASTMD_ICED_UI_STATE_PATH` env override is retained (it is live wiring,
not pilot scaffolding).

### Cargo dependencies removed

| Entry | Kind |
| --- | --- |
| `eframe` | dependency |
| `egui_extras` | dependency |
| the platform-specific `eframe` feature line (linux target) | target dependency |
| `egui_kittest` | dev-dependency |
| the kittest-`image` snapshot comment | comment |

`cargo tree` confirms no `eframe`, `egui*` or `accesskit` package remains in
the graph. `iced` stays a normal dependency (DEC-011).

### Files kept because `ui_iced` needed them

Pure helpers that were egui-free but live-consumed were relocated, not deleted:

| Helper | From | To |
| --- | --- | --- |
| `APP_VERSION`, `license_display_text`, `should_auto_show_about`, `apply_first_run_auto_show` | deleted `ui/about_dialog.rs` | `ui_iced/about.rs` (tests ported to `about_tests.rs`) |
| `filter_logs`, `is_log_visible` | deleted `ui/background_logs.rs` | `ui_iced/logs.rs` |
| `agent_render` (+tests) | `ui/render/agent_render.rs` | `ui/agent/agent_render.rs` (git-mv; needed by the kept `ui/agent/transcript.rs`) |
| `write_new_document`, `unique_document_name`, `date_suffixed_name` | deleted `ui/modals.rs` | `utils/path.rs` (needed by `command_executor`) |
| `noop_producer` | deleted `ui/modals.rs` | `ui_iced/test_helpers.rs` (vision_processor / text_buffer tests) |
| `build_merge_prompt` | `ui/tree/handlers.rs` | kept in `ui/tree/handlers.rs`, trimmed to that one function (the click handlers are retired; the iced port is `ui_iced::panels::left::row_click_command`) |

### Gate results

| Gate | Result |
| --- | --- |
| `cargo check -q --all-targets` | pass - exit 0, no warnings |
| `cargo clippy -p fastmd --bins --lib -- -D warnings` | pass - exit 0 |
| `cargo fmt` then `cargo fmt --check` | pass - exit 0 |
| `cargo nextest run -p fastmd` | **1802 passed, 1 skipped**, 0 failures |
| `cargo build --bin fastmd` | pass - exit 0 |
| `cargo doc --no-deps --quiet -p fastmd` | pass - 0 warnings |

### Collateral damage from the automated sweep (fixed in-task)

The doc sweep was a scripted, ordered literal-replace. Two classes of fallout
were found and repaired:

1. **Snake_case identifiers.** The catch-all rule rewrote `egui` inside test
   function names, producing identifiers containing spaces. Repaired by
   stripping the whitespace and re-wording (e.g.
   `test_build_state_is_the_legacy_UI_free_...` ->
   `test_build_state_is_framework_free_at_the_crate_boundary`,
   `..._matches_the_egui_rule` -> `..._matches_the_legacy_rule`). 19 files
   touched; no test was renamed away from coverage - all still run.
2. **Clippy fallout from the `to_iced_color` unwrap.** With the conversion call
   gone, 9 sites became lintable: one `redundant_field_names`
   (`render/inline.rs`) and eight `unnecessary_lazy_evaluations`
   (`bool::then(|| Background::Color(..))` -> `then_some(..)`) across
   `batch_dialog.rs`, `dialogs.rs`, `panels/center.rs` (x2), `panels/left.rs`,
   `render/table.rs`, `render/yaml.rs` and `render/inline.rs`. All fixed.

### Needs the orchestrator's eyes

1. **Doc drift is untouched by design** ([RUST-043], TASK-030 scope): the
   counts in the table above (`ARCHITECTURE_C4.md` 9, `Messaging.md` 2,
   `src/app/AGENTS.md` 5, `doc/distill/` 164, `SPEC.md` 3). `SPEC.md`'s
   file-map still points at the deleted `src/app/ui/` tree - that is a
   spec-vs-code drift that needs an explicit instruction before anyone edits it.
2. **`src/app/AGENTS.md` still mandates consulting `doc/distill/egui.md` and
   `doc/distill/egui-kittest.md`** and still carries the `egui::Id` stability
   and conditional-rendering rules. Those instructions now point at a retired
   stack; TASK-030 must repoint them at the iced guide.
3. **`cargo clippy -p fastmd --all-targets` is still not clean** - 11 lint
   sites, all pre-existing (`config/config_tests.rs`,
   `markdown/link_scanner_tests.rs`, `ui_iced/panels/left_tests.rs`,
   `ui_iced/render/code_tests.rs`, `ui_iced/message_tests.rs`). This is one
   fewer than the 12 recorded at TASK-029a: the deleted
   `ui/batch_dialog_tests.rs` carried one of them. No new lint was introduced
   by this task. The brief's stated gate (`--bins --lib`) passes.
4. **`ui/` is now a shared domain-state module, not a UI module.** What is left
   under it (`theme.rs`, `fonts.rs`, `strings.rs`, `panel_layout.rs`,
   `persisted.rs`, `tree/flatten.rs`, `agent/*`) is framework-free state and
   assets consumed by `ui_iced`. If the orchestrator wants a final home for it
   (e.g. folding into `ui_iced/` or a `domain/` module), that is a naming
   decision this task deliberately did not make.
5. **No branch created, nothing committed** - left for the orchestrator.

---

## TASK-025 test-tier wiring log (iced_test tiers 2/3/4 - 2026-10-18)

### Dependency resolution

`iced_test = "0.14.0"` added to `[dev-dependencies]` in `Cargo.toml`.
Resolves cleanly against the existing `iced = "0.14.0"`:

```text
iced_test v0.14.0        [dev-dependencies] -> fastmd
iced_selector v0.14.0    (pulled in by iced_test)
iced v0.14.0            (unchanged)
```

`iced_tester` was **not** added. It is only needed to *record* `.ice`
sessions (the F12 overlay behind iced's `tester` feature); nothing in the
tier-2 or tier-3/4 harness needs it to compile or run, so the task left it
out rather than compile a recording feature into the shipped `iced`
dependency. Recording a `.ice` file therefore still requires a short-lived
manifest change - documented in `tests/iced_visual/README.md`.

### Feasibility gate: PASS - the simulator runs headlessly here

Verified first, before writing any real tests. A one-button
`iced_test::simulator` test (`button("OpenTools").on_press(...)`,
`ui.click("OpenTools")`, assert `into_messages()`) compiled and passed on
the first run on this Windows box with no window and no GPU setup step.
`iced_test::Simulator` builds an `iced_runtime::UserInterface` over the
`iced_renderer::Renderer` fallback chain (wgpu first, `tiny-skia`
second); `ICED_TEST_BACKEND=tiny-skia` forces the software path.

Two things the gate surfaced that shaped everything downstream:

1. **`Simulator::into_messages` consumes the simulator.** The example in
   `doc/iced.md` calls `ui.find(..)` *after* `into_messages()`, which
   cannot compile. Tests must query first, then drain.
2. **`&str` selectors are exact-match, not substring.** `iced_selector`
   0.14.0 matches `Candidate::Text.content == query` and
   `Candidate::TextInput.text() == query`. `doc/iced.md`'s "selects a
   widget by the text it contains" reads as substring; it is not. Every
   selector in the suite passes the full rendered string.

### Tier 2 - `src/app/ui_iced/interaction_tests.rs` (13 tests, all passing)

Declared from `ui_iced/mod.rs` as `#[cfg(test)] pub mod interaction_tests;`
rather than as a `<file>_tests.rs` sidecar ([RUST-056]) because it spans
every `ui_iced` view module instead of pairing with one implementation
file.

Each test renders a **production view function** (no stand-ins) over a
**`test_app_state()`** fixture (in-memory storage, [RUST-006]) and drives
it with `click` / `typewrite` / raw `simulate` events. Where a test needs
state to actually change between two clicks the emitted messages are
pushed through the **production reducer** `ui_iced::update::update`
before the view is rebuilt.

| Interaction | Test | Emitted & asserted |
|---|---|---|
| Tree row left click | `test_tree_row_left_click_emits_select_file` | `Command(SelectFile { path, multi: false })` |
| Tree row right click -> live menu | `test_tree_row_right_click_mounts_a_live_context_menu` | `Tree(ContextMenuOpened)`, then reducer-mounted menu rows rendered in `view(&state)`, then a row click publishes its own message |
| Tree pointer motion | `test_tree_pointer_motion_emits_the_cursor_anchor` | `Layout(CursorMoved)` in the scrollable's viewport frame |
| Tab strip `x` | `test_tab_strip_close_button_emits_close_tab` | `Command(CloseTab(0))` |
| Second tab `x` | `test_second_tab_close_button_emits_close_tab_one` | `Command(CloseTab(1))` (per-tab index, not always the first match) |
| Tab title click | `test_tab_title_click_selects_that_document` | `Command(tab_select_command(beta.md))` |
| Editor type + Save | `test_editor_typewrite_then_save_emits_save_requested` | `Editor(Action(_))` while typing, `Editor(SaveRequested { path })` on Save |
| Editor Cancel | `test_editor_cancel_never_requests_a_save` | `Editor(CancelRequested)` only |
| Name-entry dialog typing | `test_name_entry_dialog_streams_name_changed_messages` | one `Dialog(NameChanged)` per keystroke, accumulated |
| Name-entry dialog submit | `test_name_entry_dialog_submit_after_reducer_accepts_the_name` | inert while the reducer-held name is empty (click falls through to the scrim -> `Dismissed`), `Dialog(Submitted)` once `update` has accepted `subdir` |
| Hamburger open | `test_top_bar_hamburger_consumes_the_open_click` | the pick list captures the open click |
| Hamburger rows | `test_hamburger_rows_emit_their_commands` | every `hamburger_items()` row publishes its own `Message::Command` |
| Whole-app smoke | `test_full_app_view_is_simulatable_with_an_overlay` | `view(&state)` renders with a dialog mounted above the bands; its field is reachable |

Selector techniques the suite relies on beyond `&str`:
`Candidate::TextInput` to focus a text field, `Candidate::Focusable` to
focus a `text_editor` (which does **not** answer a `TextInput` query),
`Candidate::Container` filtered by width to reach the hamburger holder,
and a counting closure selector to reach the *n*-th of several identical
labels.

### Tier 2 gap: `pick_list` menus are unreachable

**The hamburger menu's rows cannot be driven under `iced_test` 0.14.**
Root cause, verified against the crate source and reproduced with a bare,
isolated `pick_list` (so it is not a FastMD composition problem):

* `iced_widget::pick_list` draws its closed label **and** its open menu
  rows with `renderer.fill_text`, not with child `Text` widgets, so no
  `Candidate::Text` is ever produced for a row;
* `iced_widget::overlay::menu::Overlay` does **not** override
  `Overlay::operate`, which is a no-op default in `iced_core` 0.14
  (`overlay.rs:41`), so the whole menu subtree is invisible to every
  `Selector`;
* `Simulator::point_at` deliberately emits no `CursorMoved` event, and
  the menu sets `hovered_option` only from `CursorMoved` - so even a
  coordinate-guessed click finds no hovered row. A 180-position scan over
  the opened menu produced zero messages.

`pick_list` also has no keyboard handling at all in 0.14.2, so `tap_key`
is not an escape hatch. Covered as far as the tool allows (open-click
consumed + the real `HamburgerItem` row set driven as buttons). If the
orchestrator wants the widget itself driven, the options are an upstream
`Overlay::operate` impl in iced, or replacing the hamburger `pick_list`
with the app's own `overlay::context_menu_card` (real buttons, already
fully testable) - a UI decision outside this task.

### Tier 3 / tier 4 harness

```text
tests/
  iced_visual_harness.rs    Program adapter + tier-3/4 entry points
  iced_visual/
    README.md               recording instructions
    baselines/.gitkeep      tier-3 baselines (EMPTY - none recorded)
    ice/.gitkeep            tier-4 recorded sessions (EMPTY - none recorded)
```

`FastMdProgram` implements `iced_test::program::Program` as a thin
adapter over the production `build_state` / `update` / `view` +
`window_settings`, so the harness renders exactly what `launch()`
renders - no parallel reimplementation. It uses
`iced_test::futures::backend::null::Executor`: no subscription runs, so
a recording touches no network, no watcher and no real user config
([RUST-006]).

Running tests (not ignored, both pass):

* `test_ice_replay_directory_is_wired` - `iced_test::run(program, ice_dir)`
  over the `.ice` directory. Passes empty today; executes every recorded
  session the moment one is committed, with no harness change.
* `test_visual_artifact_directories_exist` - guards the documented layout
  against drift.

Recording tests (`#[ignore]`d, need a human pass):

* `test_record_app_view_baseline` - whole `view(&AppState)`
* `test_record_top_band_baseline` - `top_bar(&state.models.top)`

How recording works: `Simulator::snapshot(&theme)` ->
`Snapshot::matches_hash(path)`. Because `matches_hash` *creates* a missing
baseline and returns `true` (a silent pass), recording is gated behind
`FASTMD_RECORD_ICED_BASELINES=1` and the compare path asserts the
baseline file exists first:

```powershell
$env:FASTMD_RECORD_ICED_BASELINES = "1"
cargo test -p fastmd --test iced_visual_harness -- --ignored   # record
Remove-Item Env:FASTMD_RECORD_ICED_BASELINES
cargo nextest run -p fastmd --test iced_visual_harness -- --ignored   # assert
```

Baselines are renderer-specific (`<name>-<renderer>.sha256`); pin
`$env:ICED_TEST_BACKEND = "tiny-skia"` for GPU-free reproducibility.

### Tiers that could NOT be fully wired

1. **Tier 2, hamburger `pick_list` row selection** - blocked upstream in
   iced 0.14.2 (see the gap section above). Partially covered.
2. **Tier 3, committed baselines** - not recorded. A baseline is only
   meaningful once a human has confirmed the rendered frame is correct;
   committing an unreviewed hash is worse than committing none. Harness
   is complete and waiting.
3. **Tier 4, committed `.ice` sessions** - cannot be produced headlessly:
   the recorder is an interactive F12 overlay in a real window, and
   enabling iced's `tester` feature was deliberately avoided. Harness and
   replay entry point are complete and waiting.

### Quality gate

| Gate | Result |
|---|---|
| `cargo check -q` | clean |
| `cargo nextest run --status-level fail --show-progress none` | **1817 passed, 3 skipped** (2 = the tier-3 recording tests, 1 = pre-existing `recycle_bin::tests::native_backend_live_delete`) |
| `cargo clippy -p fastmd --bins --lib -- -D warnings` | clean |
| `cargo clippy -p fastmd --all-targets` | no lint from the new files |
| `cargo fmt --check` | clean |
| `cargo doc --no-deps --quiet` | clean |

### Needs the orchestrator's eyes

1. **`doc/iced.md` is wrong in two places** and both cost time to rediscover:
   the `Simulator` example calls `find` after `into_messages` (cannot
   compile), and `&str` selectors are described as "contains" when they are
   exact-match. Worth correcting in the SDK doc.
2. **The `pick_list` gap is a real UI-testability hole**, not just a test
   gap: the shipped hamburger cannot be exercised by any automated tool.
   If closing it matters, the cheapest route is rendering the hamburger
   with the app's own `overlay::context_menu_card` (real buttons) instead
   of `pick_list`. That is a UI decision this task did not make.
3. **Tier 3/4 still need a recording pass.** The harness will not catch
   pixel drift or replay regressions until someone runs it and commits the
   artifacts.
4. **No branch created, nothing committed** - left for the orchestrator.
