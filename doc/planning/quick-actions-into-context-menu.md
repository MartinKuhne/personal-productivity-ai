# Plan: Move Quick Actions into the File Context Menu

Status: proposal
Date: 2026-08-01
Owner: TBD
Related: `UI-019`, `UI-033`, `UI-034` (`src/desktop/src/ui/SPEC.md`); `AGENT-023` (`src/desktop/src/agent/SPEC.md`)

## 1. Goal

Today the bottom panel surfaces a `⚡ Quick Tasks` menu button whose only item is `Format Markdown`. The file context menu already has `Format Markdown` and `Run as prompt` inlined, sandwiched between `Copy path` and `Print`, with no visual separator. The two trigger sites call the same handlers — the bottom-panel button is effectively a duplicate entry point.

This plan relocates the *Quick Actions* concept out of the bottom panel and into the file context menu as a dedicated section:

- A `ui.separator()` is drawn inside the file context menu.
- Every quick action becomes its own line below the separator.
- The context menu reads top-to-bottom as: navigation/editing → file operations → quick actions (separator) → destructive actions.
- The bottom panel no longer carries the `Quick Tasks` `menu_button`. The freed horizontal space is reclaimed by the command input (or left as slack — recommend a measured change in width, not a hard resize).
- The same effect helpers drive both trigger sites where applicable, so the behaviour stays in one place.

## 2. Current state (with file references)

### 2.1 The bottom-panel "Quick Tasks" menu
- `src/desktop/src/ui/panels/bottom.rs:177-202` — `ui.menu_button(QUICK_TASKS_MENU, ...)` wraps a single `ui.button(FORMAT_MARKDOWN_ACTION)`.
- The click handler pre-fills `app.agent.command_input` with `generate_format_prompt(date_str)` and sets `submit = true`, which dispatches via `apply_send_click` (`bottom.rs:96-125`).
- `src/desktop/src/ui/strings.rs:71` defines `QUICK_TASKS_MENU = "⚡ Quick Tasks"`.
- `src/desktop/src/ui/strings.rs:74` defines `FORMAT_MARKDOWN_ACTION = "Format Markdown"` (re-used by the context menu too).

### 2.2 The file context menu (two parallel implementations)
There are *two* context-menu call sites for files because the codebase still has the legacy recursive tree plus the flat-row renderer:

- `src/desktop/src/ui/tree/render.rs:136-246` — `render_flat_row`'s `response.context_menu(...)` for a file row (the active path). Single-select branch (`:161-244`) has the order: `Edit` → `Show in File Explorer` → `Copy path` → `Format Markdown` → `Run as prompt` → `Print` → `Rename` → `Move` → `Delete`.
- `src/desktop/src/ui/tree/render.rs:368-470` — `draw_tree_node`'s legacy `response.context_menu(...)` for a file row. Single-select branch (`:394-468`) has the same item order.

Both branches already include `FORMAT_MARKDOWN_ACTION` and `RUN_AS_PROMPT_ACTION`, but neither draws a separator and the quick-action items are visually mixed with the other actions.

### 2.3 The click handler
Both call sites set `*ctx.submit_prompt() = Some(generate_format_prompt(&date_str))` for `Format Markdown`, and read the file with `std::fs::read_to_string` for `Run as prompt`. The effect is "submit a prompt to the agent through the same `submit_prompt` channel that the bottom panel uses" — i.e. the context-menu items already go through the unified prompt-dispatch path. The bottom-panel "Format Markdown" goes via `command_input` + `apply_send_click` instead of `submit_prompt`; that difference is incidental and not part of the user contract.

## 3. Proposed change

### 3.1 Bottom panel: drop the `menu_button` wrapper
- Remove the `ui.menu_button(QUICK_TASKS_MENU, |ui| { ... })` block in `ui/panels/bottom.rs`. Keep the surrounding `ui.vertical(|ui| { ... })` so the layout still has room for the (now-simpler) Stop button.
- Mark `QUICK_TASKS_MENU` in `ui/strings.rs` as `@deprecated` in its `///` doc comment and keep the constant for one release so older snapshot tests do not break; or delete it outright if the test suite can be updated in the same change.
- Update the Tier 4 render test `test_show_bottom_panel_render` in `ui/panels/bottom.rs:484-502`. Replace the `assert_text_contains(QUICK_TASKS_MENU)` assertion with a stronger one: the panel must still contain the command input (`COMMAND_INPUT_HINT` or a stable marker) and must NOT contain `QUICK_TASKS_MENU`. The existing comment in that test already says the Quick Tasks menu was the "stable header for the bottom panel" — that rationale goes away.

### 3.2 File context menu: add a separator and a quick-actions section
- In both context-menu closures (`render_flat_row` at `ui/tree/render.rs:161-244` and `draw_tree_node` at `:394-468`):
  1. Keep the navigation/editing items at the top: `Edit`, `Show in File Explorer`, `Copy path`, `Print`.
  2. After `Print`, call `ui.separator()`.
  3. Draw the quick-actions section in declaration order. For the initial release, the section contains `Format Markdown` and `Run as prompt` — i.e. the same two items currently inlined.
  4. After the quick actions, call `ui.separator()`.
  5. Draw the file-management items: `Rename`, `Move`, `Delete`.
- The visual outcome, top to bottom:
  ```
  Edit
  Show in File Explorer
  Copy path
  Print
  ─────────────
  Format Markdown       ← quick action
  Run as prompt         ← quick action
  ─────────────
  Rename
  Move
  Delete
  ```
  Open question (see §6): the second separator may be considered overkill for one user. The minimal proposal is a single separator before the quick-actions block; a second separator before destructive actions is a UX-style choice the reviewer can flip.

### 3.3 Centralise the quick-action list
- New private helper in `src/desktop/src/ui/tree/render.rs` (or in a new `src/desktop/src/ui/tree/quick_actions.rs` if the file trends > 1024 lines per `AGENTS.md §5`):
  - `fn quick_action_items(ui: &mut egui::Ui, ctx: &mut TreeNodeContext<'_>, row: &FlatRow) -> bool` returning `true` if a quick action was clicked (so the caller can `ui.close()`).
  - The helper iterates a `const QUICK_ACTIONS: &[QuickAction]` table. Each `QuickAction` carries a label string and a closure `fn(&mut FastMdApp, &mut TreeNodeContext<'_>, &FlatRow)`.
  - The closure body is exactly the existing inline code (format prompt + `submit_prompt`, read file + `submit_prompt`, future actions plug in here).
- The two context-menu call sites invoke the helper instead of repeating the items. This kills the duplication flagged in `ui/tree/render.rs` between `render_flat_row` and `draw_tree_node` (the whole `else` block of the single-select branch is currently copy-pasted with `row` vs. `node` field renames).
- `QUICK_ACTIONS` lives at the top of the file as a `const` slice so the order is data, not control flow. New actions become one new entry, not new branches in two places.

### 3.4 Keep the bottom-panel path alive for keyboard-only flows
The current bottom-panel trigger fires `apply_send_click` from the Enter handler. The plan does NOT change the Enter-handler path. The only removal is the `menu_button` shell. The agent-dispatch pipeline (`apply_send_click` → `start_session`) is untouched.

## 4. Spec impact

- `UI-019` ("When the user selects [Format Markdown] from the context menu, the system executes the Format Markdown quick task as described elsewhere.") — unchanged in spirit, the trigger is still the file context menu.
- `UI-033` ("[Format Markdown] - Executes the Format Markdown quick task on the tab's file.") — also unchanged. The tab context menu (`ui/panels/center.rs`) gets the *same* treatment: a `ui.separator()` before its quick-actions block, with `Format Markdown` moved into that block. UI-034 already requires the file/tab menus to be consistent.
- `AGENT-023` (Quick Tasks Menu) — propose a revision: drop the "bottom panel menu button" wording, re-state the requirement as "Quick Actions are exposed as a dedicated section in the file/tab context menus, separated from the file-operations section by a visual separator." The capability (predefined prompts that inject a structured prompt with YAML front-matter template) stays the same.
- `UI-007` … `UI-018` — unchanged.

## 5. Test plan

Per `src/desktop/AGENTS.md §6` (quality gate) and §2 (test-driven changes):

### 5.1 Tier 1 (unit, no egui harness)
- New helper unit test: `quick_action_items` iterates `QUICK_ACTIONS` in declared order and dispatches the right `submit_prompt` for each entry. Mock `TreeNodeContext` per the existing `test_helpers` pattern.
- Keep the existing `test_apply_send_click_*` tests in `ui/panels/bottom.rs` — they exercise the Enter-key dispatch and are independent of the menu shell.

### 5.2 Tier 4 (egui_kittest click tests)
- New snapshot test in `ui/tree/render.rs::tests`:
  - Open the file row's context menu.
  - Assert the rendered shape tree contains the labels `Edit`, `Format Markdown`, `Run as prompt`, `Delete` and at least one `Separator` widget between `Print` and `Format Markdown` and another between `Run as prompt` and `Rename`.
  - Assert the order matches the proposal in §3.2.
- New interaction test:
  - Open the context menu on a file row, click `Format Markdown`, verify `ctx.submit_prompt()` ends up with `Some(generate_format_prompt(...))`.
  - Same for `Run as prompt`: click it, verify `submit_prompt` contains the file's body.
- Update `test_show_bottom_panel_render` in `ui/panels/bottom.rs:484-502`:
  - Assert the panel still renders the command input.
  - Assert the panel does NOT render `QUICK_TASKS_MENU`.
  - The current `assert_text_contains(QUICK_TASKS_MENU)` becomes `assert_text_does_not_contain(QUICK_TASKS_MENU)` (mirror helper to add in `ui/test_helpers/text.rs` if not present).
- Add a regression test for the tab context menu (`ui/panels/center.rs`): clicking `Format Markdown` on a tab fires the same `submit_prompt`. This locks in UI-034 consistency.

### 5.3 Quality gate
- `cargo check --quiet`, `cargo nextest run`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps` — all clean before merge (per `src/desktop/AGENTS.md §6`).

## 6. Open questions for the reviewer

1. **One separator or two?** Proposal uses two (one before the quick-actions block, one before Rename/Move/Delete). A single separator before quick actions is the minimal change. Pick the variant that matches the project's UX convention.
2. **Order inside the quick-actions block.** Today the items appear in source order (`Format Markdown` first, then `Run as prompt`). I propose keeping that order. If `Run as prompt` should come first (it is the more frequent power-user action), flip it.
3. **Tab context menu scope.** The plan extends the same change to the tab context menu in `ui/panels/center.rs` because UI-034 requires consistency. If the reviewer wants the file-menu change only, the tab menu stays as-is and the plan shrinks to a one-file change. Recommend the consistent approach.
4. **Future quick actions.** The `QUICK_ACTIONS` table makes adding new entries a one-line change. Should the table be sourced from `config.yaml` (user-configurable quick actions) or stay code-defined? The proposal keeps it code-defined for v1 to match today's "Format Markdown is the only one" scope; a follow-up can add config-backed actions.
5. **Bottom-panel width.** Removing the `menu_button` shrinks the bottom panel's right-side content. Either widen the `TextEdit` by ~32 px or leave the slack. Recommend the `TextEdit` width change, but flag it as a follow-up UI polish (not required for correctness).

## 7. Out of scope

- The agent-loop / LLM dispatch path (`start_session`, `apply_send_click`, `parse_command_intent`).
- The `Run as prompt` semantics (it remains a single-line context menu item, not a generalised "run any file as a prompt" feature).
- The keyboard shortcut surface — no new shortcuts are proposed; the bottom-panel Enter handler is unchanged.
- Mobile / non-desktop targets (per `src/desktop/AGENTS.md`, the crate is Windows/Linux/macOS via eframe).

## 8. Rollback plan

Each step is independently revertible:
- §3.3 (helper extraction) is pure refactor; the inlined items in §3.2 still work without the helper.
- §3.2 (separators + reorder) is a UI change; reverting restores the previous order with no behavioural impact.
- §3.1 (drop the bottom-panel `menu_button`) is the user-visible change. Reverting re-adds the `menu_button` shell; the `QUICK_TASKS_MENU` constant is kept in `ui/strings.rs` with a `@deprecated` doc to make the rollback trivial.

The whole change is the union of three small diffs, each testable in isolation. No DB migration, no config-file format change, no public API change.
