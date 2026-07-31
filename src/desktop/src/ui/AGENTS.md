## egui Best Practices
- Follow egui's recommended patterns and idioms when building UI.
- Follow egui testing best practices (e.g. using `egui::__run_test_ctx` / `State::test_ctx` style harnesses as appropriate) for deterministic UI tests. `egui_kittest` (already a dev-dep) is the preferred harness for snapshot tests.
- Keep `update` methods side-effect free where possible and avoid storing unnecessary state in `egui::Id`s.
- The 5-pane layout is owned by `ui::panel_layout::PanelLayout` (REQ-101); do not ad-hoc side panels in `FastMdApp::update`.
- All cross-cutting state lives on `FastMdApp` (`ui/app.rs`); split new UI concerns into a dedicated manager struct (cf. `DialogManager`, `SelectionManager`, `TabManager`) rather than growing `app.rs`.


## User Interface Text Strings Isolation
- **Centralized UI Strings:** All user-facing text strings (button labels, headers, dialog prompts, context menu items, tooltips, status messages) must be isolated into `src/desktop/src/ui/strings.rs` as `pub const` items or formatted string builder functions.
- **No Hardcoded UI Literals:** Avoid duplicating or embedding inline string literals in egui panel rendering modules (`ui/panels/*`, `ui/modals.rs`, `ui/tree.rs`, `editor.rs`, etc.). Always reference `crate::ui::strings::<CONST_NAME>`.
- **Documentation & Unit Tests:** Every `pub const` or helper function in `strings.rs` must include a `///` doc comment. Include unit tests in `strings.rs` verifying constant values and formatting logic.

## `egui::Id` Stability and Salting Rules
- **Purpose of `egui::Id`s:** `egui` tracks interactive widget state (hover, focus, animation, context menus, drag/drop) across frames and multi-pass layout renders (e.g., `SidePanel` / `Panel`, `ScrollArea::show_rows`, `Grid`) using `egui::Id`.
- **Preventing `WARN egui::context` pass-to-pass ID changes:** If `egui` sees a widget at the exact same physical coordinates (`rect`) assigned a different `Id` between layout passes (Pass 1 measurement vs. Pass 2 paint), it emits a `WARN egui::context: Widget rect [...] changed id between passes` warning and paints red debug outlines.
- **Rules for setting `egui::Id`s:**
  1. **Never re-use duplicate keys for sibling widgets:** If a loop renders multiple widgets per item (e.g., a tab label and a tab close button `×`), salt each key with a string label tuple:
     ```rust
     ui.push_id((tab_path, "tab_label"), |ui| ui.selectable_label(is_selected, &title));
     ui.push_id((tab_path, "tab_close"), |ui| ui.button("×"));
     ```
  2. **Wrap iterated list and tree rows in salted scopes:** When rendering dynamic lists (`ScrollArea::show_rows`, file trees, TOC lists), wrap each row in `ui.push_id((&item_key, item_type), |ui| { ... })` so internal `ui.horizontal` calls generate stable child auto-IDs.
  3. **Isolate structural blocks:** Assign explicit string keys to top-level containers (`ui.push_id("selected_file_header", ...)`, `ScrollArea::id_salt(...)`) to insulate child auto-ID counters from sibling layout passes.

### Conditional rendering: always allocate, toggle visibility

**Why the warning is sticky.** `egui` is immediate-mode but stateful: hover, focus, drag, scroll, animation, and `Response` flags are all keyed by `egui::Id` and stored between frames. During a single frame, every measure-then-place container (`Panel`, `SidePanel`, `Window`, `Grid`, `ScrollArea::show_rows`) invokes the inner closure multiple times — first to measure, then to place/paint. `egui` requires that each allocation produce the **same `Id` at the same `Rect`** in every pass.

Most widget ids in a `Ui` are *auto-ids* — an invisible per-`Ui` counter whose value at the time of allocation is folded into the parent id's hash. Auto-ids are **positional, not logical**: the Nth widget allocated is the same id in both passes only if both passes allocate the same Nth widget in the same shape. Any structural change between passes — a different `if` arm, a `for` loop that yielded a different number of items, a `Some`/`None` flip, a `match` arm that fell through differently, a `collapsing` that opened — shifts every downstream auto-id by the size of the change.

The disagreement is between two invocations of the same call in the same frame, so you can't see the other invocation when reading the code. The auto-id is invisible. The mismatch can be many layers above the rect that warned, which is why a single off-by-one allocation at the top of a panel can flood the log with hundreds of `changed id between passes` lines. Per-leaf `push_id` salting does not fix this — it narrows the parent id, but it does not fix a tree that allocates a different shape across passes.

**The fix pattern.** When a conditional cannot be eliminated, **both branches of the conditional must allocate the same number of widgets in the same order; only the visibility of the widgets differs.** This keeps the auto-id counter walking the same path in every pass.

Rules:

1. **Replace `if cond { A } else { B }` with "always allocate, branch visibility."** If the two branches allocate a different number of widgets, pad the shorter one with `ui.add_visible(visible, /* placeholder */)` or `ui.allocate_space(size)` so the auto-id counters stay aligned across both arms.
2. **Replace `if let Some(x) = maybe { ... }` with the same shape.** Always allocate the body; toggle its visibility on the `Some`/`None` boundary.
3. **Never wrap an entire `Panel::*::show(...)` in `if cond { ... }`.** When the panel disappears, the available rect for every sibling changes, so the sibling widgets' rects shift in the same frame and the auto-id tree reshuffles. Always allocate the panel, and call `ui.set_invisible()` inside its closure when the cond is false.
4. **`ui.collapsing(header, |ui| { ... })` is fine on its own** — the body is always allocated, only its visibility follows the open/closed state. But never put a *second* conditional inside the body that adds or removes a widget, or the open/close toggle itself becomes a pass-to-pass id mismatch.
5. **Primitives:** `egui::Ui::add_visible(visible, widget)` for a single widget; `ui.scope(|ui| { if !cond { ui.set_invisible(); } ... })` for a block.

Stable `push_id` salts (rules above) are still required for sibling loops — they coexist with this pattern, they do not replace it.

**Canonical example in this codebase** — `src/ui/panels/top.rs:79-100`, the toolbar row's indexing-finished transition. The previous revision rendered a `Spinner` while indexing and a `ComboBox` after indexing inside mutually-exclusive `if/else if` blocks keyed on `indexing_finished`. The moment indexing finished, the combobox replaced the spinner at the same rect on successive passes and `egui` logged `WARN egui::context: Widget rect ... changed id between passes` for the entire toolbar row on every frame. The fix: `ui.add_visible(!indexing_finished, Spinner::new())` and a `ui.scope(...)` that always allocates the separator and combobox, calling `ui.set_invisible()` while indexing. The regression test `test_show_top_panel_no_id_change_warnings_on_indexing_finished_transition` in the same file captures `log` output across the bool flip and asserts no `changed id` warning fires.

**Counter-example to avoid** — `src/ui/panels/right.rs:60-99`, the right panel wrapped in `if should_show_panel(...) { Panel::right("toc_panel").show(...) }`. The TOC items inside are correctly salted with `push_id((i, entry.id, "toc_item"))`, but the *panel itself* still appears and disappears. When the user selects their first file (or unselects), the panel's allocation toggles, the center panel's available rect changes, and every rect inside the center panel shifts in the same frame. The fix is to *always* allocate the `Panel::right` and call `ui.set_invisible()` inside its closure when `should_show_panel` is false.

**Triage heuristic.** To localise a residual warning, do one user action at a time and watch the log for the rect coordinates:

- Hitting Enter in the bottom panel's command input trips `agent.show_results`; warnings at the center panel's rects (`x≈287+`) point at `src/ui/panels/center.rs:318-328`.
- Selecting or deselecting a file in the left tree trips the right panel's existence; warnings at the right side of the screen (`x≈700+`) point at `src/ui/panels/right.rs:60-99`.
- Switching between two files where one has YAML front-matter and one does not trips `if let Some(yaml)`; warnings at the top of the center scroll (`y≈180-220`) point at `src/ui/panels/center.rs:213-218`.
- Watching an agent stream trips the `collapsing` and `if !is_empty()` chain; warnings in the center that grow frame by frame point at `src/ui/panels/center.rs:98-130`.

