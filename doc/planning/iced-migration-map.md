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
