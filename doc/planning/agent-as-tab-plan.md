# Implementation Plan: Agent Session as a Regular Tab

## Goal

Replace the current agent-as-overlay pattern (where `show_agent_results` replaces the center panel content) with a proper tab-based architecture. The agent session becomes a `TabItem::Agent` entry in the main tab bar, alongside file tabs. Only one agent tab is allowed at any time.

---

## Current Architecture (Baseline)

```
FastMdApp state:
  tabs: Vec<PathBuf>           ← file paths only
  selected_file: Option<PathBuf>  ← which tab is selected / file highlighted in tree
  show_agent_results: bool     ← when true, center panel shows agent instead of tabs
  agent_running: bool          ← agent state flags
  agent_status: String
  agent_thinking: String
  agent_response: String
  ...more agent fields

Center panel dispatch (center.rs:260-269):
  if show_agent_results → render_agent_session()
  else if tabs not empty → render_tabs_and_content()
  else → render_empty_state()
```

**Problem:** The agent session is not a tab — it's a global modal-like override. The user cannot switch between a file tab and the agent session. Closing the agent (Back to Document) destroys all state.

---

## Phase 1: Data Model (`app.rs`)

### 1a. Add `TabItem` enum

Create a new type to represent either a file tab or the agent tab:

```rust
// In app.rs or a new tab_types.rs module
#[derive(Debug, Clone, PartialEq)]
pub enum TabItem {
    File(PathBuf),
    Agent,
}

impl TabItem {
    pub fn label(&self) -> String {
        match self {
            TabItem::File(p) => p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            TabItem::Agent => "🤖 Agent".to_string(),
        }
    }

    pub fn is_agent(&self) -> bool {
        matches!(self, TabItem::Agent)
    }

    pub fn file_path(&self) -> Option<&PathBuf> {
        match self {
            TabItem::File(p) => Some(p),
            TabItem::Agent => None,
        }
    }
}
```

**Location:** New file `src/ui/tab_item.rs` — keeps the type focused and avoids circular dependencies.

### 1b. Update `FastMdApp` struct fields

| Current Field | Replacement | Notes |
|---|---|---|
| `tabs: Vec<PathBuf>` | `tabs: Vec<TabItem>` | Mixed file + agent tabs |
| `selected_file: Option<PathBuf>` | `active_tab_index: Option<usize>` | Index into `tabs` for the active tab |
| `selected_file: Option<PathBuf>` | Keep for tree highlighting | Rename to `tree_selected_file` to disambiguate |
| `selected_files: HashSet<PathBuf>` | Keep | Multi-select in tree, unchanged |
| `show_agent_results: bool` | **Remove** | No longer needed — presence of `TabItem::Agent` in `tabs` implies it |
| `loaded_path: Option<PathBuf>` | Keep | Tracks which file is loaded into `current_markdown` |

**New/kept fields:**

```rust
pub tabs: Vec<TabItem>,                      // replaces Vec<PathBuf>
pub active_tab_index: Option<usize>,          // replaces tab role of selected_file

pub tree_selected_file: Option<PathBuf>,      // file highlighted in tree (was selected_file)
pub selected_files: HashSet<PathBuf>,         // multi-select (unchanged)
pub tree_selected_dir: Option<PathBuf>,       // (unchanged)

// Agent state — kept as is, now tied to the Agent tab's lifecycle
pub agent_running: bool,
pub agent_status: String,
pub agent_thinking: String,
pub agent_response: String,
pub agent_scroll_to_id: Option<egui::Id>,
pub agent_cancel_flag: Option<Arc<AtomicBool>>,
pub agent_history: Option<Vec<serde_json::Value>>,
pub agent_token_usage: Option<TokenUsageInfo>,
pub agent_total_usage: TokenUsageInfo,
pub submit_prompt: Option<String>,
```

### 1c. Add helper methods on `FastMdApp`

```rust
impl FastMdApp {
    /// Returns the path of the active file tab, if the active tab is a file tab.
    pub fn active_file_path(&self) -> Option<&PathBuf> {
        self.active_tab_index
            .and_then(|i| self.tabs.get(i))
            .and_then(|t| t.file_path())
    }

    /// Returns true if the active tab is the agent tab.
    pub fn is_agent_tab_active(&self) -> bool {
        self.active_tab_index
            .and_then(|i| self.tabs.get(i))
            .map_or(false, |t| t.is_agent())
    }

    /// Returns the index of the agent tab, if one exists.
    pub fn agent_tab_index(&self) -> Option<usize> {
        self.tabs.iter().position(|t| t.is_agent())
    }

    /// Opens or focuses the agent tab. Creates it if it doesn't exist.
    pub fn focus_agent_tab(&mut self) -> usize {
        if let Some(idx) = self.agent_tab_index() {
            self.active_tab_index = Some(idx);
            idx
        } else {
            self.tabs.push(TabItem::Agent);
            let idx = self.tabs.len() - 1;
            self.active_tab_index = Some(idx);
            idx
        }
    }

    /// Closes the agent tab and cancels any running agent.
    pub fn close_agent_tab(&mut self) {
        if let Some(flag) = &self.agent_cancel_flag {
            flag.store(true, Ordering::SeqCst);
        }
        self.tabs.retain(|t| !t.is_agent());
        if self.is_agent_tab_active() {
            self.active_tab_index = self.tabs.last().map(|_| self.tabs.len() - 1);
        }
        self.agent_running = false;
        self.agent_history = None;
        self.agent_response.clear();
        self.agent_thinking.clear();
        self.agent_status.clear();
        self.agent_token_usage = None;
        self.agent_total_usage = TokenUsageInfo::default();
    }

    /// Navigate to a file tab (create or switch).
    pub fn focus_file_tab(&mut self, path: PathBuf) -> usize {
        if let Some(idx) = self.tabs.iter().position(|t| t.file_path() == Some(&path)) {
            self.active_tab_index = Some(idx);
            idx
        } else {
            self.tabs.push(TabItem::File(path));
            let idx = self.tabs.len() - 1;
            self.active_tab_index = Some(idx);
            idx
        }
    }

    /// Loads the content for `path` into `current_markdown` / `current_yaml`.
    /// Already exists as inline logic in `update_ui` — extract into a method.
    pub fn load_file_content(&mut self, path: &PathBuf) { /* move existing logic here */ }
}
```

### 1d. Update `empty_state()` constructor

- `tabs` starts as `vec![]`
- `active_tab_index` starts as `None`
- `tree_selected_file` starts as `None`
- `show_agent_results` removed

---

## Phase 2: Tab Bar Rendering (`center.rs`)

### 2a. Rewrite `render_tabs_and_content`

The current function signature takes `&mut FastMdApp`. The rewritten version handles both `TabItem` variants:

```rust
fn render_tabs_and_content(ui: &mut egui::Ui, app: &mut FastMdApp) {
    // ── Tab bar ──
    ui.horizontal(|ui| {
        let mut tab_action: Option<(usize, TabAction)> = None;

        for (i, tab) in app.tabs.iter().enumerate() {
            let is_active = app.active_tab_index == Some(i);
            let label = tab.label();  // from TabItem::label()

            let response = ui.selectable_label(is_active, &label);
            if response.clicked() {
                app.active_tab_index = Some(i);
                // If switching to a file tab, load its content
                if let Some(path) = tab.file_path() {
                    if app.tree_selected_file.as_ref() != Some(path) {
                        app.tree_selected_file = Some(path.clone());
                        app.load_file_content(path);
                    }
                }
            }
            if response.middle_clicked() {
                tab_action = Some((i, TabAction::Close));
            }

            // Context menu (only for file tabs; agent tab gets its own)
            if !tab.is_agent() {
                response.context_menu(|ui| {
                    // ... existing context menu for files ...
                });
            }

            // Close button
            if ui.button("❌").clicked() {
                tab_action = Some((i, TabAction::Close));
            }
            ui.separator();
        }

        // Handle tab actions
        if let Some((idx, action)) = tab_action {
            if app.tabs[idx].is_agent() {
                app.close_agent_tab();  // cancels agent, removes tab
            } else {
                apply_tab_action(&mut app.tabs, &mut app.active_tab_index, idx, action);
            }
        }
    });
    ui.separator();

    // ── Content area ──
    if let Some(idx) = app.active_tab_index {
        match &app.tabs[idx] {
            TabItem::Agent => {
                render_agent_session(ui, app);
            }
            TabItem::File(path) => {
                render_file_content(ui, app, path);
            }
        }
    }
}
```

### 2b. Refactor `apply_tab_action`

Current signature: `(tabs: &mut Vec<PathBuf>, selected: &mut Option<PathBuf>, action: TabAction)`

New signature: `(tabs: &mut Vec<TabItem>, active_tab: &mut Option<usize>, close_idx: usize, action: TabAction)`

```rust
pub fn apply_tab_action(
    tabs: &mut Vec<TabItem>,
    active_tab: &mut Option<usize>,
    close_idx: usize,
    action: TabAction,
) {
    match action {
        TabAction::Close(i) => {
            if i < tabs.len() {
                tabs.remove(i);
                // Adjust active_tab if needed
                if let Some(at) = active_tab {
                    if *at == i {
                        *active_tab = if tabs.is_empty() { None } else { Some(tabs.len() - 1) };
                    } else if *at > i {
                        *at -= 1;
                    }
                }
            }
        }
        TabAction::CloseOthers(i) => {
            if i < tabs.len() {
                let keep = tabs[i].clone();
                tabs.clear();
                tabs.push(keep);
                *active_tab = Some(0);
            }
        }
        TabAction::CloseAll => {
            tabs.clear();
            *active_tab = None;
        }
    }

    // Clean up agent state if agent tab was removed
    if !tabs.iter().any(|t| t.is_agent()) {
        // Cancel agent if running (caller should do this)
    }
}
```

### 2c. Extract `render_file_content`

Move the current file-content rendering logic from `render_tabs_and_content` into its own function:

```rust
fn render_file_content(ui: &mut egui::Ui, app: &mut FastMdApp, path: &PathBuf) {
    ui.horizontal(|ui| {
        ui.heading(RichText::new(
            path.file_name().unwrap_or_default().to_string_lossy()
        ).size(18.0).strong());
        ui.label(RichText::new(format!("({})", path.to_string_lossy()))
            .size(11.0).italics().color(egui::Color32::GRAY));
    });
    ui.separator();

    egui::ScrollArea::vertical()
        .id_source("main_markdown_scroll")
        .show(ui, |ui| {
            if let Some(yaml) = &app.current_yaml {
                render_yaml_table(ui, yaml);
            }
            render_markdown(ui, &app.current_markdown, &mut app.scroll_to_header_id);
        });
}
```

### 2d. Update `render_agent_session`

Remove the "Back to Document" button — the user switches tabs to go back to a file. Rename header to just "🤖 Agent". Add a close button if desired (already handled by tab bar close button).

```rust
fn render_agent_session(ui: &mut egui::Ui, app: &mut FastMdApp) {
    ui.horizontal(|ui| {
        ui.heading(RichText::new("🤖 Agent").size(18.0).strong()
            .color(egui::Color32::from_rgb(100, 200, 255)));
        ui.separator();
        // Token usage display (moved from nowhere to here)
        if let Some(usage) = &app.agent_token_usage {
            ui.label(RichText::new(format!(
                "Tokens: {}↑{}↑{}↓{}",
                usage.prompt_tokens, usage.completion_tokens,
                usage.total_tokens,
                app.agent_total_usage.cached_tokens.map_or(0, |c| c)
            )).size(10.0).color(egui::Color32::GRAY));
        }
    });
    ui.separator();

    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Status: {}", app.agent_status)).strong());
        if app.agent_running { ui.spinner(); }
    });
    ui.add_space(8.0);

    egui::ScrollArea::vertical()
        .id_source("agent_thinking_scroll")
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if !app.agent_thinking.is_empty() {
                ui.collapsing("Thinking Process", |ui| {
                    ui.label(RichText::new(&app.agent_thinking)
                        .italics().color(egui::Color32::from_rgb(160, 160, 160)));
                });
                ui.add_space(8.0);
            }
            if !app.agent_response.is_empty() {
                ui.heading("Response");
                ui.separator();
                render_markdown(ui, &app.agent_response, &mut app.agent_scroll_to_id);
                if app.agent_running {
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                }
            }
        });
}
```

### 2e. Update `show_center_panel`

Simplify to just render tabs (which now includes the agent tab):

```rust
pub fn show_center_panel(app: &mut FastMdApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        if !app.tabs.is_empty() {
            render_tabs_and_content(ui, app);
        } else {
            render_empty_state(ui);
        }
    });
}
```

---

## Phase 3: Tree Integration (`tree.rs`, `left.rs`)

### 3a. Update `TreeNodeContext`

Replace `selected_file: &mut Option<PathBuf>` with `tree_selected_file: &mut Option<PathBuf>`.

Add `active_tab_index: &mut Option<usize>` and `tabs: &mut Vec<TabItem>`:

```rust
pub struct TreeNodeContext<'a> {
    pub tabs: &'a mut Vec<TabItem>,            // was Vec<PathBuf>
    pub active_tab_index: &'a mut Option<usize>, // NEW
    pub tree_selected_file: &'a mut Option<PathBuf>,  // was selected_file
    pub selected_files: &'a mut HashSet<PathBuf>,
    // ... rest unchanged
}
```

### 3b. Update file click handler

When a file is clicked in the tree:

```rust
if response.clicked() {
    if ctx.modifiers.shift || ctx.modifiers.ctrl || ctx.modifiers.command {
        // Multi-select (unchanged)
        if ctx.selected_files.contains(&node.path) {
            ctx.selected_files.remove(&node.path);
            // ...
        } else {
            ctx.selected_files.insert(node.path.clone());
            *ctx.tree_selected_file = Some(node.path.clone());
        }
    } else {
        ctx.selected_files.clear();
        ctx.selected_files.insert(node.path.clone());
        *ctx.tree_selected_file = Some(node.path.clone());
        // Open file tab or switch to existing one
        *ctx.active_tab_index = Some(
            if let Some(idx) = ctx.tabs.iter().position(|t| t.file_path() == Some(&node.path)) {
                idx
            } else {
                ctx.tabs.push(TabItem::File(node.path.clone()));
                ctx.tabs.len() - 1
            }
        );
    }
}
```

### 3c. Update directory click handler

```rust
if response.clicked() {
    // Toggle expand, clear file selection
    *ctx.tree_selected_file = None;
    ctx.selected_files.clear();
    *ctx.selected_dir = Some(node.path.clone());
    // NOTE: do NOT add directories to tabs — only files go in tabs
}
```

### 3d. Update tree selection highlighting

```rust
let is_selected = ctx.selected_files.contains(&node.path)
    || ctx.tree_selected_file.as_ref() == Some(&node.path);
```

---

## Phase 4: Bottom Panel (`bottom.rs`)

### 4a. Command submission routing

When the user submits a command:

```rust
if submit {
    let prompt = app.command_input.trim_end().to_string();
    app.command_input.clear();

    match parse_command_intent(&prompt) {
        CommandIntent::ShowModels => {
            app.focus_agent_tab();
            app.agent_status = "Done".to_string();
            app.agent_response = format_models_list(&app.config.models);
        }
        CommandIntent::ShowDeprecatedModelMessage => {
            app.focus_agent_tab();
            app.agent_status = "Error".to_string();
            app.agent_response = "The /model command is deprecated...".to_string();
        }
        CommandIntent::RunAgent(agent_prompt) => {
            app.focus_agent_tab();  // opens or switches to agent tab
            setup_and_launch_agent(app, agent_prompt);
        }
        CommandIntent::Empty => {}
    }
}
```

### 4b. Stop button

The stop button only shows when the agent tab is active AND the agent is running:

```rust
if app.is_agent_tab_active() && app.agent_running {
    if ui.button(RichText::new("⏹ Stop").color(egui::Color32::RED)).clicked() {
        if let Some(flag) = &app.agent_cancel_flag {
            flag.store(true, Ordering::SeqCst);
        }
        app.agent_running = false;
        app.agent_status = "Aborted by user.".to_string();
    }
}
```

---

## Phase 5: Modal Updates (`modals.rs`)

### 5a. Rename modal

When a file is renamed, update paths in `tabs`:

```rust
// Instead of:
//   for i in 0..app.tabs.len() {
//       if app.tabs[i] == *file {
//           app.tabs[i] = new_path.clone();
//       }
//   }

// Now:
for tab in &mut app.tabs {
    if let TabItem::File(p) = tab {
        if *p == *file {
            *p = new_path.clone();
        }
    }
}
```

### 5b. Move modal

File move operations should also update `tabs` — same pattern as rename.

### 5c. `create_dir_modal`, `move_modal`

No changes needed — these don't interact with tabs.

---

## Phase 6: Right Panel (`right.rs`)

### 6a. Conditional TOC display

Only show the Table of Contents when a file tab is active (not the agent tab):

```rust
pub fn show_right_panel(app: &mut FastMdApp, ctx: &egui::Context) {
    let show_toc = !app.toc.is_empty()
        && app.active_file_path().is_some();  // only for file tabs

    if show_toc {
        egui::SidePanel::right("toc_panel")
            // ... existing render ...
    }
}
```

### 6b. Update `should_show_panel`

No longer needs `has_selected_file` parameter — derives from `active_file_path()`:

```rust
pub fn should_show_panel(has_toc: bool, has_active_file: bool) -> bool {
    has_toc && has_active_file
}
```

---

## Phase 7: App State Management (`app.rs`)

### 7a. Remove `show_agent_results`

- Remove from struct, `empty_state()`, and all references
- Remove `clear_agent_session_state` (replaced by `close_agent_tab`)
- Remove `Back to Document` button logic

### 7b. Update `start_agent_session`

Replace `self.show_agent_results = true` with `self.focus_agent_tab()`:

```rust
pub fn start_agent_session(&mut self, prompt: String) {
    self.command_input = prompt;
    self.agent_status = "Initializing agent...".to_string();
    self.agent_thinking.clear();

    self.focus_agent_tab();  // creates or switches to agent tab

    if self.agent_history.is_none() {
        self.agent_response.clear();
        self.agent_history = None;
        self.agent_token_usage = None;
        self.agent_total_usage = TokenUsageInfo::default();
    } else {
        self.agent_response
            .push_str(&format!("> **User:** {}\n\n", self.command_input));
    }

    self.agent_running = true;

    let cancel_flag = Arc::new(AtomicBool::new(false));
    self.agent_cancel_flag = Some(cancel_flag.clone());

    crate::agent::run_agent(
        self.config.clone(),
        self.tx.clone(),
        self.tree_selected_file.clone(),  // was selected_file
        self.selected_dir.clone(),
        self.selected_files.clone(),
        self.command_input.clone(),
        cancel_flag,
        self.agent_history.clone(),
        self.agent_response.clone(),
        self.file_event_bus.clone(),
    );
    self.command_input.clear();
}
```

**Important:** `start_agent_session` is called from two places: the bottom panel submit handler, and the "Format Markdown" context menu action. Both should continue to work — they just need to reference `tree_selected_file` instead of `selected_file` where applicable.

Actually, looking at the Format Markdown path more carefully: in `tree.rs:199-203`, `selected_file` is set via `*ctx.selected_file = Some(tab_path.clone())` before submitting the format prompt. With the refactor, this would be:
```rust
*ctx.submit_prompt = Some(crate::ui::generate_format_prompt(&date_str));
```
And the format prompt submission handler sets `app.selected_file = Some(tab_path.clone())` — this needs to become `app.tree_selected_file = Some(...)`.

### 7c. Update `update_ui` main loop

The file loading logic at `app.rs:536-551` currently checks `self.selected_file`:

```rust
// Before:
if let Some(selected_path) = &self.selected_file {
    if self.loaded_path.as_ref() != Some(selected_path) {
        // load content
    }
}

// After:
if let Some(active_path) = self.active_file_path() {
    if self.loaded_path.as_ref() != Some(active_path) {
        // load content
    }
} else if self.is_agent_tab_active() {
    // Don't try to load file content — agent tab has no file
    // Possibly clear loaded content to free memory
    self.loaded_path = None;
    self.current_yaml = None;
    self.current_markdown.clear();
}
```

### 7d. Update file event handlers

File `Removed` events that reference `selected_file`:

```rust
// Before:
if self.selected_file.as_ref() == Some(&event.path) {
    self.selected_file = None;
    self.current_yaml = None;
    self.current_markdown.clear();
    self.toc.clear();
}

// After:
if self.tree_selected_file.as_ref() == Some(&event.path) {
    self.tree_selected_file = None;
}
// Also close any file tab pointing to the removed path
self.tabs.retain(|t| t.file_path() != Some(&event.path));
if let Some(at) = self.active_tab_index {
    if at >= self.tabs.len() {
        self.active_tab_index = self.tabs.last().map(|_| self.tabs.len() - 1);
    }
}
// Clear loaded content if the active tab is gone
if self.active_file_path().is_none() && !self.is_agent_tab_active() {
    self.current_yaml = None;
    self.current_markdown.clear();
    self.toc.clear();
    self.loaded_path = None;
}
```

### 7e. Handle `AgentFinished` message

When the agent finishes, update state but do NOT switch away from the agent tab (the user decides when to close it):

```rust
BackgroundMessage::AgentFinished(history) => {
    self.agent_running = false;
    self.agent_history = Some(history);
}
```

### 7f. Handle `submit_prompt`

The `submit_prompt` mechanism is used by "Format Markdown" and "Merge" context actions. These should still work — they trigger `start_agent_session` which now uses `focus_agent_tab()`:

```rust
if let Some(prompt) = self.submit_prompt.take() {
    self.start_agent_session(prompt);
}
```

---

## Phase 8: Update Tests

### 8a. Update `center.rs` tests

- `test_show_center_panel_render_modes`: remove the agent-results mode test, replace with agent-tab-in-tabs test
- `test_apply_tab_action_close`, `test_apply_tab_action_close_others`, etc.: update signatures to use `TabItem` and `active_tab_index`
- `test_clear_agent_session_state`: replace with `test_close_agent_tab`
- Add tests for `TabItem::label()`, `TabItem::is_agent()`, `TabItem::file_path()`

### 8b. Update `app.rs` tests

- `test_background_messages_handling`: update `selected_file` references
- `test_background_message_file_modified_and_deleted`: update to use `TabItem` tabs
- `test_agent_failure_and_finish_messages`: no structural change needed
- `test_agent_token_usage_message_accumulates`: no structural change needed

### 8c. Update `tree.rs` tests

- `test_tree_node_selection_state_modifiers`: update `TreeNodeContext` construction with new fields
- Update field references (`selected_file` → `tree_selected_file`)

### 8d. Update `bottom.rs` tests

- `test_show_bottom_panel_render`: no change
- `test_show_bottom_panel_stop_agent`: verify stop button only shows when agent tab is active

### 8e. Update `right.rs` tests

- `test_show_right_panel_hidden_when_no_file`: update to test with no active file tab
- `test_show_right_panel_shown_with_toc`: update to verify agent tab does NOT show TOC

### 8f. Update `modals.rs` tests

- `test_rename_modal`: update to check `TabItem::File` path updates
- `test_rename_preserves_extension`: same

### 8g. Add new tests

```rust
#[test]
fn test_tab_item_agent_helpers() {
    let agent = TabItem::Agent;
    assert!(agent.is_agent());
    assert!(agent.file_path().is_none());
    assert_eq!(agent.label(), "🤖 Agent");
}

#[test]
fn test_tab_item_file_helpers() {
    let path = PathBuf::from("/test/doc.md");
    let file = TabItem::File(path.clone());
    assert!(!file.is_agent());
    assert_eq!(file.file_path(), Some(&path));
    assert_eq!(file.label(), "doc.md");
}

#[test]
fn test_focus_agent_tab_creates_and_switches() {
    let mut app = create_test_app();
    app.tabs.push(TabItem::File(PathBuf::from("a.md")));
    app.active_tab_index = Some(0);

    let idx = app.focus_agent_tab();
    assert_eq!(app.tabs.len(), 2);
    assert!(app.tabs[1].is_agent());
    assert_eq!(app.active_tab_index, Some(1));
}

#[test]
fn test_focus_agent_tab_reuses_existing() {
    let mut app = create_test_app();
    app.tabs.push(TabItem::File(PathBuf::from("a.md")));
    app.tabs.push(TabItem::Agent);
    app.active_tab_index = Some(0);

    let idx = app.focus_agent_tab();
    assert_eq!(app.tabs.len(), 2);  // no duplicate
    assert_eq!(app.active_tab_index, Some(1));
}

#[test]
fn test_close_agent_tab_cancels_agent() {
    let mut app = create_test_app();
    let cancel = Arc::new(AtomicBool::new(false));
    app.agent_cancel_flag = Some(cancel.clone());
    app.agent_running = true;
    app.tabs.push(TabItem::Agent);
    app.active_tab_index = Some(0);

    app.close_agent_tab();
    assert!(cancel.load(Ordering::SeqCst));
    assert!(!app.agent_running);
    assert!(app.tabs.iter().all(|t| !t.is_agent()));
}
```

---

## Migration: `selected_file` → `tree_selected_file`

The following files reference `selected_file` on `FastMdApp` and must be updated:

| File | Lines | Change |
|---|---|---|
| `app.rs` (struct) | 64 | Rename field + update all internal references |
| `app.rs` (methods) | 157-158, 378, 465-466, 536 | Use `active_file_path()` for tab, keep `tree_selected_file` for tree |
| `left.rs` | 132 | Pass `&mut app.tree_selected_file` to tree context |
| `tree.rs` | 11, 50, 138, 147, 151, 155-156 | Rename field in context struct |
| `bottom.rs` | 172 | Pass `app.tree_selected_file.clone()` to agent |
| `modals.rs` | 198-199 | Use `tree_selected_file` for tree selection; check `tabs` for tab paths |
| `top.rs` | 97-98 | Tag filter — keep using `tree_selected_file` for tree highlighting |
| `center.rs` | 137, 142, 188, 200, 205, 412 | Replace with `active_tab_index` + `active_file_path()` |
| `right.rs` | 10, 35, 143 | Replace with `active_file_path()` |

---

## Dependency Graph & Execution Order

```
Phase 1 (Data Model)
    │
    ▼
Phase 2 (Tab Rendering) ──── depends on: TabItem enum, helper methods
    │
    ├──► Phase 3 (Tree) ──── depends on: TabItem in tabs, TreeNodeContext updates
    ├──► Phase 4 (Bottom Panel) ──── depends on: focus_agent_tab(), is_agent_tab_active()
    ├──► Phase 5 (Modals) ──── depends on: TabItem enum
    ├──► Phase 6 (Right Panel) ──── depends on: active_file_path()
    │
    ▼
Phase 7 (App State) ──── integrates all the above
    │
    ▼
Phase 8 (Tests) ──── depends on: all code changes complete
```

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `selected_file` has many references; missing one causes compile error | Medium | Medium | Rust's type system catches all naming mismatches at compile time. The rename is safe. |
| Tab index management (after close, insert, etc.) off-by-one errors | Medium | High | Add property-based tests for tab index invariants after every operation. |
| Agent state cleared when switching tabs | Low | Medium | `close_agent_tab()` only runs on explicit close, not on tab switch — ensure this is not accidentally called. |
| "Format Markdown" action changes tab context | Low | Low | Format prompt still works via `submit_prompt` → `start_agent_session` which now opens agent tab. The file context is still passed correctly. |
| Agent tab persist across agent finish | Low | Medium | Agent tab stays open after agent finishes (intentional — user decides when to close it). No automatic close added. |

---

## Key Design Decisions

1. **Agent tab is singleton**: At most one `TabItem::Agent` in `tabs`. `focus_agent_tab()` reuses the existing one if present.

2. **No automatic tab switching on agent finish**: When the agent finishes (`AgentFinished` message), the agent tab remains active and the user decides when to close it.

3. **`tree_selected_file` != active file tab**: The tree selection and the active tab are decoupled. A file can be highlighted in the tree while the agent tab is active. This matches user expectation (you can refer to what's highlighted while chatting with the agent).

4. **`loaded_path` tracks the last loaded file**: Used for cache invalidation. Cleared when switching to agent tab or when the active file tab changes.

5. **TOC panel only for file tabs**: The right panel hides when the agent tab is active, since there are no markdown headers to navigate.

6. **Close button on agent tab = cancel agent**: Middle-click or ❌ button closes the agent tab, which cancels any running agent and clears agent state. This is explicit and intentional.
