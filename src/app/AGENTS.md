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
- You MUST consult [doc/distill/egui.md](../../../doc/distill/egui.md) before you write or review UI code.
- You MUST consult [doc/distill/egui-kittest.md](../../../doc/distill/egui-kittest.md) before you write or review snapshot tests.

- Keep `update` methods side-effect free where possible.
- Keep pane layout ownership with `ui::panel_layout::PanelLayout` and do not add ad-hoc side panels in `FastMdApp::update`.
- Keep cross-cutting UI state on `FastMdApp` in [app/mod.rs](app/mod.rs). Split new UI concerns into a dedicated manager when the state grows.

## 4. User Interface Text String Rules
- Isolate all user-facing text into [strings.rs](strings.rs) as `pub const` values or formatting helpers.
- Do not hardcode UI literals in panel, modal, tree, or editor modules.
- Reference strings through `crate::ui::strings::<CONST_NAME>`.
- Add a `///` doc comment to every `pub const` or helper in [strings.rs](strings.rs).

## 5. `egui::Id` Stability Rules
- Keep widget IDs stable across layout passes.
- Do not reuse the same key for sibling widgets. Salt each key with a label tuple when a loop renders multiple widgets for one item.
- Wrap dynamic rows, lists, and tree rows in a salted scope so child auto-IDs stay stable.
- Assign explicit string keys to top-level containers when a block has structural variation.

Use this pattern for sibling widgets:
```rust
ui.push_id((tab_path, "tab_label"), |ui| ui.selectable_label(is_selected, &title));
ui.push_id((tab_path, "tab_close"), |ui| ui.button("×"));
```
Use this pattern for dynamic rows:
```rust
ui.push_id((&item_key, item_type), |ui| {
    // render row contents
});
```

## 6. Conditional Rendering Rules
- Always allocate the widget tree shape. Toggle visibility instead of removing the whole branch.
- Replace `if` / `else` branches with an always-allocated structure when the branches differ in widget count.
- Pad the shorter branch with `ui.add_visible(visible, ...)` or `ui.allocate_space(...)` when needed.
- Replace `if let Some(x) = maybe` with the same shape. Allocate the body and toggle visibility at the `Some` / `None` boundary.
- Do not wrap an entire `Panel::*::show(...)` block in a conditional. Keep the panel allocated and call `ui.set_invisible()` inside the closure when the panel is hidden.
- Keep `ui.collapsing(...)` blocks valid. The body may remain allocated. Do not add a second conditional that changes widget count inside the body.
- Use `egui::Ui::add_visible(visible, widget)` for a single widget and `ui.scope(...)` with `ui.set_invisible()` for a block.

