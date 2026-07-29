# Correctness Analysis — egui Render Paths (`src/desktop`)

**Scope:** `src/ui/render.rs`, `src/ui/table_width/mod.rs`, `src/ui/panels/{center,left,right,top,bottom}.rs`, `src/ui/tree.rs`, `src/ui/app.rs`.

**Stack:** `eframe`/`egui` 0.35 (`Cargo.toml:20`), `pulldown-cmark` 0.10 (`Cargo.toml:28`), `egui_kittest` 0.35 dev-dependency with `snapshot` feature (`Cargo.toml:52`).

**Method:** static read-only review of the render paths. No code was changed.

All line references are relative to `src/desktop/`.

---

## P0 — Visible incorrect output or dead interaction

### P0-1. Pinned table cells never report their height → table rows overlap

`src/ui/render.rs:314-323`

```rust
let (rect, _) = ui.allocate_at_least(egui::vec2(w, 0.0), egui::Sense::hover());
let layout = egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true);
let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(layout));
content(&mut child_ui);
```

Three compounding defects:

1. The parent allocation is `vec2(w, **0.0**)` — zero height.
2. `Ui::new_child` does **not** allocate parent space. In egui 0.35 the child's `min_rect` must be fed back (`ui.advance_cursor_after_rect(child_ui.min_rect())`, or use `allocate_new_ui`). It is not.
3. `max_rect` has zero height, so `with_main_wrap(true)` wraps horizontally but grows *downward past the rect*, and no clip rect is set.

Net effect: `egui::Grid` computes row height from the other cells' allocations, which are also zero-height. Any cell that wraps to 2+ lines draws outside its row and overlaps the following row. This is the primary payload of the whole FTWA feature, and it is broken at the egui boundary.

The doc comment at `src/ui/render.rs:257-262` describes behaviour the code does not implement — it claims `ui.set_width` and `Label::wrap(true)`; `set_width` is never called. The header comment on `render_table` (`src/ui/render.rs:329-330`) claims `ui.allocate_ui_at_least`, which is also not the function used.

**Why tests miss it:** `e2e_tests` in `src/ui/render.rs` drive `Context::default().run_ui(...)` and assert only "does not panic". There is no geometry or row-rect assertion anywhere in the table render path.

---

### P0-2. Task-list checkboxes are inert

`src/ui/render.rs:90-95`

```rust
if let Some(checked) = task_checked {
    let mut c = checked;
    ui.checkbox(&mut c, "");   // `c` is dropped
}
```

The mutation is written into a frame-local temporary. Since `render_markdown` re-parses the source on every frame (`src/ui/render.rs:914`), the visual state snaps back immediately. There is no write-back path to `current_markdown` or to the file.

`test_render_task_checkbox_initial_state` only proves the *parser* sets `task_checked` correctly, so the missing round-trip is untested.

---

### P0-3. Ordered lists render as bullets; numbering is lost

`src/ui/render.rs:630` and `src/ui/render.rs:654-666`

```rust
Event::Start(Tag::List(_)) => { ... list_depth += 1; }    // start number discarded
Event::Start(Tag::Item)    => { ... needs_bullet = true; } // unconditional
```

`pulldown_cmark::Tag::List(Option<u64>)` carries the ordered-list start index; it is matched with `_` and thrown away. Every item then gets `"• "` (`src/ui/render.rs:88`). `1. / 2. / 3.` markdown renders as an unordered list.

`RenderEvent::FlushInline` has no field capable of carrying an ordinal, so this is a data-model gap, not a one-line fix.

---

### P0-4. ToC navigation silently fails — two divergent id derivations

`src/ui/render.rs:968-1020` (`build_toc`) vs `src/ui/render.rs:185-190` (`render_heading`)

Both sides key the scroll target on `egui::Id::new(<plain heading text>)`, but compute that text by **different algorithms**:

| | `build_toc` | `render_heading` |
|---|---|---|
| Parser options | `ENABLE_TABLES` only (`render.rs:971-972`) | `TABLES\|FOOTNOTES\|STRIKETHROUGH\|TASKLISTS` (`render.rs:504-508`) |
| Source of text | raw `Event::Text` + `Event::Code` accumulation (`render.rs:993-1002`) | `heading_plain_text(elems)` (`render.rs:49-63`) |
| Images | ignored | `"[Image: {url}]"` (`render.rs:55-57`) |

Concrete divergences, each producing a ToC entry whose click does nothing:

- `# ~~old~~ name` — strikethrough is disabled in `build_toc`, so it yields the literal `"~~old~~ name"`; `render_heading` yields `"old name"`. Ids differ → no scroll.
- `# Logo ![alt](x.png)` — `build_toc` yields `"Logo alt"`; `heading_plain_text` yields `"Logo [Image: x.png]"`. Ids differ → no scroll.
- Headings containing footnote references diverge for the same reason.

Failure is silent: `render_heading` only clears `*scroll_to_id` when it matches (`src/ui/render.rs:248-251`), so a non-matching id stays set forever and every subsequent heading render re-checks it.

---

### P0-5. Duplicate heading text collapses to one navigable target and clashes egui ids

`src/ui/render.rs:190`, `src/ui/render.rs:1007`, `src/ui/panels/right.rs:67`

`egui::Id::new(&trimmed)` is **content-derived**, with no ordinal or position disambiguator. A document with two `## Notes` sections produces:

- Two `ToCEntry` values with the *same* `id`.
- `src/ui/panels/right.rs:67`: `ui.push_id(entry.id, |ui| ...)` → two identical id scopes in the ToC panel. This is a genuine egui id clash — the exact class of bug the codebase has fought elsewhere (see the id-stability regression tests at `src/ui/panels/top.rs:289-331` and `src/ui/panels/left.rs:451`).
- Clicking the *second* entry scrolls to the *first* heading and clears `scroll_to_id` (`src/ui/render.rs:250`), making later duplicates permanently unreachable.

---

### P0-6. Blockquotes are visually indistinguishable from paragraphs

`src/ui/render.rs:677-698`

```rust
Event::Start(Tag::BlockQuote) => { /* flush only */ }
Event::End(TagEnd::BlockQuote) => { /* flush only */ }
```

No indent increment, no left quote bar, no italics or color change. `RenderEvent` has no blockquote variant.

`test_parse_markdown_rule_and_blockquote` (`src/ui/render.rs:1303-1317`) asserts only that the *text* survives, so it passes while the semantic block is lost.

---

## P1 — Layout/state corruption, non-deterministic ids, panic risk

### P1-1. `Grid`/`ScrollArea` ids derived from `ui.next_auto_id()`

`src/ui/render.rs:356`, `src/ui/render.rs:358`, `src/ui/render.rs:374`

`next_auto_id()` is a *positional peek* at the Ui's auto-id counter. Consequences:

- `egui::Grid` persists per-column minimum widths in `Memory` keyed by its `Id`. Since the id is positional, inserting or removing *any* widget above a table (a heading, a paragraph, a streaming agent token) changes the Grid's id and discards its cached layout → visible one-frame column jump.
- The `needs_horizontal_scroll` branch (`src/ui/render.rs:352`) emits a **structurally different widget tree** (an extra `ScrollArea`) at the same position. Crossing that threshold — e.g. by dragging the window edge — changes the ids of everything after it in the same Ui. That is precisely the `Widget rect ... changed id between passes` warning source documented at `src/ui/panels/left.rs:194-206` and worked around in `src/ui/panels/top.rs` (always-allocated spinner and ComboBox).

Stable ids for these containers should be derived from content — e.g. a table ordinal threaded through `RenderEvent::Table` — as `render_yaml_table` already does with fixed salts (`src/ui/render.rs:453,456`).

### P1-2. Mixed pinned/unpinned cells within one row break column alignment

`src/ui/render.rs:380-385`

```rust
let w = decision.widths.get(j).copied()
    .filter(|w| w.is_finite() && *w > 0.0);
render_table_cell(ui, cell, w);
```

`None` from either `get(j)` (row longer than the widths vector) or the `filter` (non-finite / ≤ 0) silently switches that single cell to the *unpinned* `ui.horizontal` branch (`src/ui/render.rs:322`), which reports its full intrinsic single-line width. One such cell mid-row destroys alignment for the whole column and defeats the FTWA guarantee. This should be an invariant violation, not a silent mode flip.

Related: ragged rows emit fewer cells before `ui.end_row()` (`src/ui/render.rs:378-388`), so short rows misalign against `striped(true)` and the column boundaries. `table_width::measure` documents that missing cells contribute nothing, so short rows are also unmeasured.

### P1-3. `return` instead of `continue` aborts the entire left panel

`src/ui/panels/left.rs:63-65`

```rust
let Some(current_node_ref) = root_node.children.get_mut(&lib_node_name) else {
    return;   // <-- exits show_left_panel entirely
};
```

This is inside the *file* loop and returns from `show_left_panel` **before `Panel::left(...).show(parent_ui, ...)` is ever reached** (`src/ui/panels/left.rs:183-187`). The structurally identical *directory* loop at `src/ui/panels/left.rs:105-107` correctly uses `continue`.

On the triggering condition the whole left panel disappears for that frame → full layout reflow of every other panel plus panel/id state churn. Latent (needs a file whose library node is absent) but a clear asymmetry with a severe blast radius.

### P1-4. Expanding a folder destroys the user's manual panel resize

`src/ui/panels/left.rs:129-133` plus `src/ui/tree.rs:276`, `src/ui/tree.rs:285`

```rust
if (indexing_finished && !indexing_finished_handled) || app.layout().left_panel_dirty {
    ctx.data_mut(|d| d.remove::<PanelState>(left_panel_id()));   // discards user resize
```

`ctx.data_mut(|d| d.remove::<PanelState>(...))` deletes egui's persisted panel state, which is where the user's drag-resize lives. `mark_dirty()` is called on **every directory click and double-click** (`src/ui/tree.rs:276`, `src/ui/tree.rs:285`), so ordinary tree navigation resets the panel width to the recomputed default.

`test_show_left_panel_dirty_flag_triggers_recalc` (`src/ui/panels/left.rs:355`) asserts the flag is cleared but never asserts that a user-set width survives.

Also in this block: `calc_max_width` (`src/ui/panels/left.rs:136-161`) is a full recursive text-shaping pass (`ctx.fonts_mut(...layout_no_wrap...)` per node) over the entire tree, run on every dirty frame.

### P1-5. Virtual-scroll row height assumption vs. actual row height

`src/ui/panels/left.rs:227` plus `src/ui/tree.rs:262`, `src/ui/tree.rs:352`

`show_rows(ui, TREE_ROW_HEIGHT /* 22.0 */, rows.len(), ...)` positions row *i* at `i * 22.0` and clips accordingly, but each row is drawn as `ui.horizontal { add_space(depth*18.0); selectable_label(...) }`, whose natural height is a function of the body font size and `item_spacing.y` — not pinned to 22.0.

Any mismatch accumulates linearly down the list: rows drift out of their assigned slots, hit-testing and clipping desynchronise, and rect-vs-id stability degrades — the same failure mode the comment at `src/ui/panels/left.rs:194-206` describes fixing at a different layer. Nothing in the code or tests enforces `row height == TREE_ROW_HEIGHT`.

### P1-6. Unclamped negative widget width

`src/ui/panels/bottom.rs:92-93`

```rust
let text_width = ui.available_width() - 130.0;
let response = ui.add_sized(egui::vec2(text_width, ui.available_height()), TextEdit::multiline(...));
```

No `.max(0.0)`. On a narrow window this is negative and produces an inverted/degenerate rect. Compare `src/ui/render.rs:349`, which *does* clamp (`.max(0.0)`) for the same class of computation.

### P1-7. Copy-code button is squeezed to zero width

`src/ui/render.rs:151-158`

```rust
ui.horizontal_top(|ui| {
    ui.add(egui::Label::new(RichText::new(content).monospace()).wrap());  // consumes all width
    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| { ...button... });
});
```

The wrapping `Label` is added *first* into a `horizontal_top`, so it takes the full available width. The subsequent right-to-left layout receives ~0 remaining width and the copy button is clipped or overlaps the code text. The button must be allocated before the flexible label, or placed in the frame's top-right via a separate allocation.

### P1-8. Double `Mutex` lock with `unwrap()` on the render path

`src/ui/panels/top.rs:74-76`

```rust
let mut show_bg = app.background_manager.lock().unwrap().show_background_logs;
...
app.background_manager.lock().unwrap().show_background_logs = show_bg;
```

Two panics-on-poison per frame in the UI thread, plus a read-modify-write split across two lock acquisitions (a lost-update window if any background thread writes the same field).

### P1-9. Directory double-click toggles expansion twice → net no-op

`src/ui/tree.rs:267-286`

`response.clicked()` toggles expansion, and `response.double_clicked()` toggles it again. egui fires both for a double-click, so the second toggle undoes the first: double-clicking a folder appears to do nothing — while still calling `mark_dirty()` twice (see P1-4).

### P1-10. YAML front-matter table always shows a horizontal scrollbar

`src/ui/render.rs:445-455`

`available_width` is captured **before** entering `Frame::NONE.inner_margin(8.0)` (`src/ui/render.rs:445` vs `:449`), then applied inside as `ui.set_min_width(available_width)`. The min width therefore exceeds the frame's content width by the 2 × 8 px margin, so `ScrollArea::horizontal` is permanently overflowed and the scrollbar is always drawn.

---

## P2 — Fidelity, robustness, performance, maintainability

### Heading rendering (`src/ui/render.rs:204-244`)

- Uses `ui.horizontal` (not `horizontal_wrapped`), so a long heading overflows horizontally instead of wrapping — inconsistent with `render_inline` (`src/ui/render.rs:134`).
- Does not zero `item_spacing.x`, unlike `render_inline` (`:82`) and `render_table_cell` (`:278`), so a heading split into multiple styled spans shows spurious gaps between them.
- `style.bold` is ignored: `RichText::new(t).size(size).strong()` (`:208`) is unconditional, so `**bold**` inside a heading is indistinguishable and the `TextStyle.bold` bit is dead on this path.

### Parser fidelity (`src/ui/render.rs:501-903`)

- `Event::End(TagEnd::CodeBlock)` pushes a `CodeBlock` **unconditionally** (`:571-574`) with no `if in_code_block` guard — an unmatched End emits a stale/duplicate block.
- Emphasis is tracked as booleans, not counters (`:753-758`), so nested same-type emphasis clears early.
- `InlineElem::Link(url, text)` (`:767-769`) **drops the entire `TextStyle`** — bold/italic/code/strikethrough inside a link is lost. `in_link` being a `bool` also breaks on nesting.
- An image inside a link becomes a detached `Image` elem plus a `Link` carrying only the alt text (`:749-752`).
- `HardBreak` inside a table cell silently degrades to `SoftBreak`, i.e. a space (`:809-811`).
- Block-level `Event::Html` is emitted as an *inline* `InlineElem::Html` (`:829-830`), identical to `InlineHtml` — block structure lost.
- `End(TagEnd::Table)` pushes `Table(cells)` **and** `Space(4.0)` even when the table is empty (`:712-715`), while `render_table` early-returns on `n == 0` (`:342-345`) → a stray 4 px gap.
- `Start(Tag::TableCell)` calls `buffered_inline.clear()` (`:733-735`), silently discarding any pending inline content rather than flushing it.

### YAML front matter (`src/ui/render.rs:414-437`)

- `Sequence` items use `v.as_str().unwrap_or("")` (`:424`) → numbers, bools, and nested structures become **empty strings**. `a: [1, 2]` renders as `", "`. The `_ =>` arm two lines below already handles non-strings correctly via `serde_yml::to_string`; the sequence arm does not reuse it.
- Non-string keys are silently dropped (`:418`).

### ToC panel (`src/ui/panels/right.rs`)

- `calculate_indent`: `_ => 0.0` (`:28`) means level 0 or > 6 indents *less* than level 1 — asserted as intended by `test_calculate_indent` (`:103`), but it is a discontinuity.
- `calculate_font_size = 13.0 - level*0.5` (`:37`) is unbounded: level 26 → 0.0 px, beyond → negative. `level` is a `u32` from the parser and clamped to 1..=6 today, but the function is `pub` and unguarded.

### FTWA measurement (`src/ui/table_width/mod.rs`)

- The pure `ftwa` core (line 68) is **sound**: length/finiteness/`max >= min` asserts, three well-separated regimes, drift fix-up, `Σwidths == available` invariant, proptest plus 10k-column stress. The defects are all at the egui boundary.
- `measure` (line 291) is called from `render_table` (`src/ui/render.rs:347`) **unconditionally, every frame, with no memoization**, and `accumulate` (line 384) performs one `layout_no_wrap` for the whole fragment *plus one per whitespace token*. That is O(cells × tokens) full text shapings per frame per visible table.
- Semantic mismatch: max-content is the **sum of independently laid-out fragment widths**, whereas the renderer lays fragments out sequentially in one wrapping flow. Kerning/shaping differences mean the pinned width is only approximately the true content width, so cells can wrap when they shouldn't (or overflow when they should wrap).

### Tree (`src/ui/tree.rs`)

- `render_flat_row` (line 257) and `draw_tree_node` (line 492) are ~230 lines of duplicated interaction logic (click/double-click, expand/collapse, identical 11-item context menu). Only `render_flat_row` is used by `left.rs`; `draw_tree_node` remains `pub` and tested. They have **already diverged** — only `render_flat_row` clamps depth (`row.depth.min(50)`, line 264). Guaranteed future drift.
- Blocking filesystem I/O on the UI thread inside context menus: `std::fs::write` (line 321), `trash::delete` (line 335, plus a loop over all multi-selected files), `std::fs::read_to_string`, and `execute_print_blocking` — the last stalls the render thread for the duration of a print job.
- `build_merge_prompt` iterates a `HashSet<PathBuf>` → **non-deterministic file ordering** in the generated prompt, making agent output irreproducible.

### Center panel (`src/ui/panels/center.rs`)

- The `×` close button (`:205`) is **not** inside the `ui.push_id(tab_path, ...)` scope used for the tab label (`:150`). Its id is purely positional, so it shifts whenever tabs are added, closed, or reordered — inconsistent with the deliberate id-scoping one line above.
- `tab_action` is a single `Option` (`:139`) overwritten by later widgets in the same frame; two actions in one frame silently drop the first.
- `apply_tab_action` `Close` moves selection to `tabs.last()` rather than a neighbour — closing a middle tab jumps to the end (documented by `test_apply_tab_action_close`).
- `ScrollArea::...stick_to_bottom(true)` (`:105`) plus an inner `ui.scroll_to_cursor(Some(Align::BOTTOM))` (`:125`) are two competing scroll mechanisms on the same area.
- `agent.state().response.clone()` (`:122`) clones the entire streaming response **and re-parses the full markdown every frame** while the agent streams — O(n) clone plus O(n) parse per frame with n growing.

### App and left panel (`src/ui/app.rs`, `src/ui/panels/left.rs`)

- `src/ui/panels/left.rs:20-127` rebuilds the entire `TreeNode` hierarchy from scratch every frame, with a linear scan over content libraries per path → O(files × libraries) per frame; `flatten_tree` (`:221`) then re-walks and clones name and path per row. `app.rs` already carries regression tests asserting that file events must *not* set `left_panel_dirty` and must rebuild tags only on removal — the same performance concern, unaddressed here.
- `PersistedUiState`: `save()` writes `expanded_dirs` into the field literally named `collapsed_dirs`, and `new()` reads it back into `expanded_dirs`. It round-trips correctly, but the name inverts the meaning — a trap for anyone reading the persisted blob or adding a second consumer.
- `handle_deferred_actions` (`src/ui/app.rs:771`) uses `eprintln!("Batch completed: {:?}", result)` while the rest of the codebase uses `tracing`.

### Dead code

- `render_inline`'s `wrap` parameter: every `push_inline` call site passes `wrap: true`, so the `ui.horizontal` branch (`src/ui/render.rs:136`) is unreachable.
- `render_code_block`'s `_idx: &mut usize` and `render_markdown`'s `code_block_idx` (`src/ui/render.rs:915`) are created, passed, and never read or incremented.

---

## Cross-cutting: the test suite cannot see the layout defects

`src/ui/render.rs`'s `e2e_tests` render through `egui::Context::default().run_ui(...)` and assert only *"does not panic"*. There is not a single assertion on a widget rect, row height, column boundary, or overlap anywhere in the table, heading, or tree render paths.

`egui_kittest` 0.35 is already a dev-dependency **with the `snapshot` feature enabled** (`Cargo.toml:52`) and is already used for click-behaviour tests (copy-code, hyperlink → `OutputCommand::OpenUrl`, checkbox). P0-1 (row overlap), P0-2 (inert checkbox), P1-5 (row drift), P1-7 (squeezed button), and P1-10 (spurious scrollbar) are all directly expressible as kittest snapshot or rect assertions.

The three existing "no id change warnings" tests (`src/ui/panels/left.rs:355` and `:451`, `src/ui/panels/top.rs:289-331`) use a `log::Log` capture / red-stroke-shape heuristic, and their own doc comments concede they do not reproduce the production trigger — so P1-1 (positional `next_auto_id` salts) and P0-5 (duplicate `push_id`) also pass unnoticed.

---

## Suggested fix order

1. **P0-1** — feed the child's `min_rect` back to the parent (`advance_cursor_after_rect`) and set a clip rect; add a kittest row-overlap assertion. This unblocks the entire FTWA feature.
2. **P0-4, P0-5** — unify heading-id derivation into one function shared by `build_toc` and `render_heading`, and disambiguate duplicates with an occurrence ordinal.
3. **P0-2, P0-3, P0-6** — extend `RenderEvent` / `InlineElem` to carry list ordinals, blockquote depth, and a task-toggle write-back channel.
4. **P1-1, P1-2** — thread a stable table ordinal through `RenderEvent::Table` for Grid/ScrollArea ids; make a missing or invalid column width an assertion rather than a silent mode flip.
5. **P1-3, P1-4, P1-6, P1-8** — small, local, low-risk: `return` → `continue`, stop discarding `PanelState` on tree clicks, clamp the width, single lock acquisition.
