# Agent instructions

## 3. Required Review and Implementation Rules
- Consult the distilled reference in [doc/distill/egui.md](../../../doc/distill/egui.md) before you write or review UI code.
- Consult the distilled reference in [doc/distill/egui-kittest.md](../../../doc/distill/egui-kittest.md) before you write or review snapshot tests.

- Keep `update` methods side-effect free where possible.
- Keep pane layout ownership with `ui::panel_layout::PanelLayout` and do not add ad-hoc side panels in `FastMdApp::update`.
- Keep cross-cutting UI state on `FastMdApp` in [app/mod.rs](app/mod.rs). Split new UI concerns into a dedicated manager when the state grows.

## 4. User Interface Text String Rules
- Isolate all user-facing text into [strings.rs](strings.rs) as `pub const` values or formatting helpers.
- Do not hardcode UI literals in panel, modal, tree, or editor modules.
- Reference strings through `crate::ui::strings::<CONST_NAME>`.
- Add a `///` doc comment to every `pub const` or helper in [strings.rs](strings.rs).
- Add unit tests in [strings.rs](strings.rs) for constant values and formatting logic.

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

## 7. Examples to Follow
- Follow the toolbar transition pattern in [panels/top.rs](panels/top.rs) when a state change swaps one widget for another.
- Keep the panel allocated and hidden in [panels/right.rs](panels/right.rs) instead of removing the panel entirely.
- Trace remaining warnings by changing one user action at a time and checking the rect coordinates in the log.

