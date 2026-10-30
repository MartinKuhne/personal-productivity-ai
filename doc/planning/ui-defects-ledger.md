# UI Defect Ledger — 18 reported defects (branch `more-iced`)

> **Status:** **Complete — 18 of 18 landed.** Five defects were closed on this
> branch (`#2`, `#4`, `#6`, `#12`, `#14`, `#17`); the rest landed before the
> ledger opened. Six follow-ups are recorded in §3, of which **F-4 needs an
> owner decision**. This ledger is the single record of the reported defect
> list, the owner's rulings on the ambiguous ones, what has actually landed, and
> what remains.
> **Branch:** `more-iced`. Baseline commit at ledger creation: `06e8f9c`.
> **Specs:** [`src/app/ui/SPEC.md`](../../src/app/ui/SPEC.md) (`UI-xxx`) and
> [`src/hoarfrost/spec.md`](../../src/hoarfrost/spec.md) (`UIF-xxx`).
> Neither is modified by this work (`RUST-043`).
> **Method:** every fix is test-first (`RUST-005`) — a negative test is proven
> red against the unmodified behaviour before the fix is applied, and layout
> claims are settled by headless measurement (`iced_test::Simulator`), not by
> reading the code and reasoning about it.

---

## 1. The reported list

Reported by the owner as a single 18-item list of UI defects against the
iced UI. Item numbers are the owner's and are used verbatim in code comments
(`// defect #N`) and commit messages throughout.

| # | Defect | Owner's ruling |
|---|--------|----------------|
| 1 | Batch dialog directory list shows real directories; should show virtual ones | — |
| 2 | Batch dialog UI elements have too much vertical space between them | — |
| 3 | Batch dialog: 'mode' should appear before the 'pattern' input | — |
| 4 | Tools dialog should use more of the available screen real estate | **80% of the viewport** |
| 5 | Tools dialog says "10 group" instead of "10 groups" | — |
| 6 | Tools dialog cannot be resized | **resize the modal layer** (see D-3) |
| 7 | Tools dialog has no 'x' to close it | — |
| 8 | Tools dialog has no 'close' button | — |
| 9 | Background Operations window has a transparent background | — |
| 10 | Background Operations window has no 'x' to close it | — |
| 11 | Background Operations window has no 'close' button | — |
| 12 | No visual separation between the three window panes | **remove the zero intra-pane spacing condition — it is not in the spec — and take option (a): pane-grid spacing + adjusted ratio math** |
| 13 | Table of contents renders at a bigger font than the rest of the UI | — |
| 14 | Table of contents takes more horizontal space than its contents need | **content-sized right panel** |
| 15 | The blue selected-tab background does not conform to the size of the tab | *"if there is one tab open, the tab itself does not take the full width, but the blue band takes the full width"* |
| 16 | More tabs than fit on the tab bar render over each other | — |
| 17 | A markdown table renders as an empty table (contents off-screen) | *"did we lose the table render tests?"* |
| 18 | Windows → Agent Debug crashes the application | stack: `hoarfrost::widget::button_widget::Button::outer_size` debug_assert |

---

## 2. Owner decisions that bind this work

### D-1 — Defect #4: the Tools card is viewport-sized, not content-sized

80% of the window on **both** axes. This is an **app-side override** of
hoarfrost §4.6.2 (which states 720px wide, content-fit tall), not a
restatement of it. The framework's `ModalSizing::tools_and_integrations()`
keeps describing the spec verbatim; the card assembles its own policy from
the framework's `AxisPolicy` parts so the framework is not made to say
something the spec does not.

### D-2 — Defect #12: pane separation via grid spacing, not a painted divider

The zero intra-pane spacing assumption in `ui_iced::layout` was an
implementation convenience that never had a spec basis. It is removed: the
pane grid gets real spacing, and every pixel↔ratio conversion is adjusted to
account for the width the splits consume. Hoarfrost's `SplitDivider`
(`UIF-COMP-016`) exists and is *not* adopted by this decision.

### D-3 — Defect #6: resize lives in the modal layer

Hoarfrost modals are fixed-policy frames with no grip; the only resize
machinery in the codebase (`SurfaceTarget::Resize`, `clamp::resize`, gesture
capture, persisted geometry) belongs to Band-1 movable cards. The owner chose
to **extend the modal layer** rather than demote the Tools card to a movable
surface: the card stays a centred scrim modal, defect #4's 80% becomes the
*initial* size, and a user drag overrides it.

### D-4 — Defect #17: the lost table render tests come back

The egui deletion (`23cc35a`) removed the only tests that ever *laid out* a
table (`table_alignment.rs`, `table_visual_layout.rs`, `table_regressions.rs`,
`table_layout.rs`). Their iced replacements cover model solving and widget
*construction* only — nothing in the suite lays a table out, which is exactly
where the defect lives. Restoring layout-level coverage is part of the fix,
not an optional extra.

### D-5 — `probe_tmp.rs` is a diagnostic, not an artefact

`src/app/ui_iced/probe_tmp.rs` was left untracked and unwired by an earlier
session. It is used for empirical before/after measurement for #15/#17 and
then deleted; permanent coverage lands in the existing test sidecars instead.

---

## 3. Status

`Landed` = committed and test-pinned. `Open` = not implemented.

| # | Defect | Status | Evidence |
|---|--------|--------|----------|
| 1 | Batch dir list → virtual labels | **Landed** `06e8f9c` | `ui_iced/batch_dialog.rs:284` `available_dir_labels`; `:452` `directory_options()`; populated at `update.rs:2523`, `ui/app/init.rs:157`, `orchestrator.rs:368`, `ui_iced/app.rs:1960`. Tests `batch_dialog_tests.rs:204,232,277` |
| 2 | Batch vertical spacing | **Landed** `86a2343` | `batch_dialog.rs:988` body column `space::MD` → `space::SM`; tests `config_body_rows_are_spaced_on_the_form_row_rung`, `the_empty_run_between_two_rows_is_one_row_gap_wide`, `the_measured_row_gap_is_the_form_row_rung_not_a_wider_one` |
| 3 | 'mode' before 'pattern' | **Landed** `06e8f9c` | `batch_dialog.rs:850` `config_body_order(mode)`; tests `batch_dialog_tests.rs:308,324,338` |
| 4 | Tools card at 80% of viewport | **Landed** (this branch, §5) | `tools_dialog.rs:124` `VIEWPORT_FILL`; `:151` `TOOLS_SIZING`; tests `tools_dialog_tests.rs:838-1130` |
| 5 | "10 groups" | **Landed** `06e8f9c` | `ui/strings.rs:160` `tools_group_count()`; test `tools_dialog_tests.rs:735` |
| 6 | Tools card resizable | **Landed** (this branch, §5) | `hoarfrost/surface/modal.rs` `ResizeFloors::resolve`; `overlay.rs` `modal_resize_grip` / `modal_resize_capture_layer`; `tools_dialog.rs` `TOOLS_RESIZE_FLOORS`, `tools_card_size(viewport, user)`, `ToolsDialogState::apply_resize`; persisted under `modal:overlay.tools` via `UiStateGeometryStorage`. Tests `tools_dialog_tests.rs` (defect #6 group), `update_tests.rs` (`test_a_settled_tools_resize_*`, `test_a_reopened_tools_dialog_*`, `test_a_corrupt_stored_tools_*`), `modal_tests.rs` group 10 |
| 7 | Tools title-bar 'x' | **Landed** `06e8f9c` | `tools_dialog.rs:915` `close_button()`; tests `:1563,1602,1683` |
| 8 | Tools footer 'Close' | **Landed** `06e8f9c` | `tools_dialog.rs:953` `footer_close_button()`; tests `:1622,1654,1665` |
| 9 | Background Ops opaque | **Landed** `06e8f9c` | `logs.rs:895` routed through `movable_shell`; opaque fill `movable_shell.rs:160`; test `logs_tests.rs:994` |
| 10 | Background Ops 'x' | **Landed** `06e8f9c` | `movable_shell.rs:221` `close_action`; test `logs_tests.rs:1037` |
| 11 | Background Ops 'Close' | **Landed** `06e8f9c` | `logs.rs:848` `logs_close_button_widget()`; tests `logs_tests.rs:1063,1088,1125,1172` |
| 12 | Pane separation | **Landed** `47fb203`, regression fixed `03ecc96` | `layout.rs:75` `PANE_SPACING_PIXELS` = `space::XXS`; grid `.spacing()` at `layout.rs:511`; gutter painted `pane_gutter_fill()` **and** panes painted `pane_canvas_fill()` — the pair, see §4; ratio math via `pane_width_from_ratio` / `split_ratio_for_width` / `nested_region_width`; `RESIZE_LEEWAY_PIXELS` rebalanced to keep the `UIF-PANE-001` 10px grab; F-2 replay in `apply_pane_widths` (`layout.rs:414`) |
| 13 | ToC font rung | **Landed** `06e8f9c` | `view.rs:93` `TOC_TEXT_RUNG` = `UI_COMPACT` (13px/18px); tests `view_tests.rs:406,421,452` |
| 14 | Content-sized ToC panel | **Landed** `f962051` | `panels/right.rs:254` `auto_fit_width` (indent + `fira_sans_width` title + button padding + row pad + scrollbar gutter, clamped); fired on a landed `FileLoaded` from `update.rs:2554`; `LayoutMessage::TocAutoFit` |
| 15 | Selected-tab fill conforms | **Landed** `06e8f9c` | `center.rs:1541` slot is `Length::Shrink`, fill painted on the content-sized `surface` (`:1468`) — box and fill are one rectangle by construction; tests `center_tests.rs:1707,1739` |
| 16 | Tab overflow overlap | **Landed** `06e8f9c` | `center.rs:1391` strip rides a horizontal scrollable; tests `center_tests.rs:1791,1822,1871,1908` |
| 17 | Table renders empty | **Landed** `0c492e2` | `render/table.rs:370` `cell_element` declares its own column's `Length` instead of `Length::Fill`; `grid_element` refuses a zero-column model; `scroll_width` adds the outer padding bands; layout-level tests restored in `render/table_tests.rs` |
| 18 | Agent Debug crash | **Landed** `06e8f9c` | `movable_shell.rs:221` close carries `MOVABLE_CLOSE_TOOLTIP`; tests `movable_shell_tests.rs:206,245` |

### Follow-ups found while verifying landed items

- **F-1 (#18, open):** the shell's title-bar close pushes the `Button` into
  the `Row` directly instead of through `Button::into_element()`, which is the
  only path that wraps it in a visible `Tooltip` surface. The crash is gone
  because the tooltip *string* satisfies `UIF-ATOM-001`, but the tooltip may
  not actually paint on hover. Not in the original 18; recorded, not scheduled.
- **F-2 (#12, open):** persisted pane widths are restored into `PanelLayout`
  (`app.rs:1941`) but never replayed into the pane grid at startup — the grid
  always boots at the hardcoded `0.25` / `0.5` ratios of
  `middle_band_configuration()`. Folded into #12's ratio work.
- **F-3 (#12, spec drift):** hoarfrost §4.3.1 states `RIGHT_PANE_MIN_W = 120px`
  and a 25% max; the app uses 150px / 30% (`ui/panel_layout.rs:16,23`).
  Flagged under `RUST-041`; not silently changed.
- **F-4 (#2, open — needs an owner decision):** in **File** mode the Batch
  card's prompt scrollable measures **0px** and its candidate rows paint
  **over the footer buttons** (measured: candidate rows at y=418/450,
  Cancel/Process at y=438). Directory mode is healthy (62px viewport, rows
  inside). This is the opposite failure to #2 and was not in the reported 18.
  The arithmetic is spec-bound: §3B.1 fixes each field at
  `20+4+32+4+16 = 76px` and reserves the caption band *permanently* so a
  validation message cannot move the rows (`UIF-PRIN-003`) — 3 of 5 rows
  render an empty band and must keep it — while §4.6.2 caps the card at
  `560x480` and `batch_card_height` is pinned to `CONTENT_HEIGHT_MAX` by an
  existing test. Chrome + 4 fixed rows + the 44px minimum prompt section
  already exceed 480 before any row gap. Closing it needs one of: gaps of
  ~2px (off the spacing scale), dropping a reserved band (spec violation),
  raising `BATCH_MODAL_MAX` (spec change), or restructuring the card (e.g.
  folding Pattern into the Mode row). Recorded, **not** forced.
- **F-5 (ledger correction):** §4 of this ledger originally cited the
  form-field contract as `UIF-COMP-004`. That id is §3B.4 *ScrollView*. The
  form field is **`UIF-COMP-001`** (§3B.1, `src/hoarfrost/spec.md:734`).
- **F-6 (#6, spec drift):** hoarfrost §4.6.2 has no concept of a **user-set**
  modal size — it describes modal sizing as a policy the application declares
  per surface kind. `ResizeFloors` (added for #6) is therefore a framework
  primitive the spec does not name. Flagged under `RUST-041`; `SPEC.md` was not
  touched.

---

## 4. How the six defects closed on this branch

Each item records the measured cause as it was first written, then the
resolution — including where the original cause analysis turned out to be
wrong. Keeping both halves is the point: the gap between "what we thought" and
"what it was" is the useful part.

### #2 — Batch dialog vertical spacing — **CLOSED** `86a2343`

The card reserves a fixed band per form row whether or not that row has a
caption: hoarfrost `form_field` reserves `HEADER_HEIGHT` 20 + `BAND_GAP` 4 +
`CONTROL_SLOT_HEIGHT` 32 + `CAPTION_SLOT_HEIGHT` 16 = **72px of content per
row**, and the body column adds `space::MD` (12px) between rows — about 438px
for five rows in File mode, most of it empty reserved space.

**Resolution.** The reserved bands are not the bug and are not reducible:
§3B.1 (`UIF-COMP-001`) fixes them and `UIF-PRIN-003` requires the caption band
to stay reserved so a validation message cannot shove the rows below it. The
actual defect was that the body column stacked rows on `space::MD`, which the
spacing scale documents as the *horizontal* padding inside a control, when the
rung it documents as **form row spacing** is `space::SM`. One rung change:
row pitch 88px → 84px, inter-row gap 12px → 8px. Two contract guards were added
so the compaction cannot later be "paid for" by dropping a reserved band.
hoarfrost untouched. The residual overflow in File mode is tracked as **F-4**.

### #6 — Tools card resize (D-3) — **CLOSED**

Nothing existed to extend: `modal_card` (`overlay.rs:556`) is a styled
`Container` and `modal_centered` is scrim + centred card. Needs a corner grip,
per-surface user geometry on the overlay slot, a gesture lane in the reducer,
and clamping through hoarfrost `clamp::resize`. The movable-card stack
(`hoarfrost/surface/movable.rs`, `movable_shell.rs`, `update.rs`) is the
reference implementation for the gesture, not a thing to reuse directly.

**Resolution.** D-3 stands: the card stays a centred scrim modal and the
**modal layer** was extended, so position is never user-controlled — the grip
changes width and height only.

* **hoarfrost** gained one pure primitive, `surface::modal::ResizeFloors`, which
  resolves a wanted size against a viewport using the framework's own
  `AxisPolicy` machinery: floors from below, `Cap::None`, and `VIEWPORT_GUTTER`
  applied last so it always wins. It lives in `modal.rs` because the gutter order
  is §4.6.2 sizing machinery, not app policy.
* **The app** owns the widgets and the gesture, because `modal.rs` documents
  itself as a frame and not a widget — it builds no `Element`s.
  `overlay::modal_resize_grip` (the movable footer grip's shape: a
  `RESIZE_HANDLE_SIZE` square, `strings::ICON_RESIZE_GRIP`,
  `Interaction::ResizingDiagonallyDown`) and
  `overlay::modal_resize_capture_layer` (the full-area move/release lane, mounted
  only while the button is held) are the reusable capability; Tools is its first
  consumer.
* **Correction to the note above:** hoarfrost `clamp::resize` is **not** the right
  primitive here. It is the movable card's position+size clamp, keyed to
  `ResizeHandle` zones and a card the user is allowed to move; a centred modal has
  no position to clamp and no handle zones. Reusing it would have imported movable
  semantics the ruling explicitly excludes.
* The 80% fill from #4 is now the **initial** size, not a ceiling — a drag may
  exceed it, and only the gutter stops it. Floors are the framework's own
  `EDITOR_WIDTH_MIN` (width) and `TOOLS_HEIGHT_MIN` (height); the app invents no
  number of its own.
* A drag is measured absolutely, from the pointer to the viewport centre
  (`tools_dialog::centred_size`), because a centred card's corner sits at
  `centre + size / 2`. That makes the corner track the pointer one for one and
  removes any dependence on a press position, which `mouse_area::on_press` does
  not carry in iced 0.14.
* Transient drag state is never persisted; the settled size is, through the
  existing `UiStateGeometryStorage` seam under `modal:overlay.tools`, written
  **only** when a drag settles ([NFR-002]).

**Spec drift (F-6, flagged under `RUST-041`, not silently changed):** hoarfrost
§4.6.2 describes modal sizing as a policy the *application* declares per surface
kind. It has no concept of a user-set modal size at all, so `ResizeFloors` is a
framework addition the spec does not yet name. `SPEC.md` was not modified.

### #12 — Pane separation (D-2) — **CLOSED** `47fb203`

`layout.rs` `middle_band()` built the `PaneGrid` with no `.spacing(..)`, so
iced's default `0.0` applied, and all three panes paint the same
`surface.base` — nothing separates them. The module doc stated the
zero-spacing premise the decision removes.

**Resolution.** `PANE_SPACING_PIXELS = space::XXS` (2px) — the rung hoarfrost
itself uses for a visible separation between adjacent sibling surfaces. Three
named conversions carry the gutter through every pixel↔ratio conversion, and
because the replay ratio is the *exact inverse* of the forward one, an
unclamped drag round-trips. `RESIZE_LEEWAY_PIXELS` became
`size.splitter.grab − PANE_SPACING_PIXELS`, because iced's grab area is
`spacing + leeway`; the two together stay at exactly 10.0, so `UIF-PANE-001`'s
mandated band is preserved rather than widened by the fix.

Two judgement calls made during this work, both accepted on review:

1. **The gutter is painted.** iced's default container class is transparent and
   the canvas is `surface.base`, so an unpainted 2px gap is invisible against
   the panes it separates — spacing alone would not have fixed the reported
   defect. The container backing the grid now paints `color.border.subtle`,
   the framework's designated pane-split colour. `UIF-PRIN-006` is satisfied
   via its *visible divider* branch, not the differing-surface-token branch.
2. **Startup replay is verbatim, not re-clamped.** Re-clamping remembered widths
   against the restored viewport was tried first; it broke the pinned
   "project persisted widths verbatim" contract and meant a restore from a
   larger window silently lost its widths.

**Regression shipped and fixed — `03ecc96`.** Judgement call 1 grayed out the
middle band. `gutter_style` painted the grid's backing `color.border.subtle`
(`RGB(40,40,40)`) and its doc asserted that "panes are drawn on top of it, so
only the gutter is ever visible". That assertion was false: iced hands a pane no
background of its own, and `fill_pane` was an unstyled container, so the divider
colour read through every pane — the canvas went from `surface.base`
`RGB(9,9,11)` to `border.subtle` across the whole band, not just in the seams.

Fixed by making the panes opaque rather than by dropping the gutter fill:
`pane_canvas_style` paints each pane with `pane_canvas_fill`, which is asserted
to *be* iced's own `background` role, so an opaque pane is a visual no-op
against the window and the band looks exactly as it did before #12. Both
`fill_pane` and the TOC-hidden branch of `pane_body` apply it — the hidden-TOC
branch sits over the same backing. The two fills are now documented as a pair,
because the gutter only reads as a divider while the panes occlude it.

Why the landed test missed it: `test_pane_gutter_fill_differs_from_the_canvas_it_divides`
compares two *tokens*, and the gutter token was always correct. The bug was a
missing `.style(..)` call, which no token comparison can observe. The three
replacement tests make that explicit, and were verified to discriminate: with
`.style(pane_canvas_style)` removed from `fill_pane`, the pixel test **fails**
while both token tests still **pass**.

### #14 — Content-sized ToC panel — **CLOSED** `f962051`

The right pane's width was a pure ratio: `middle_band_configuration()` gives
the centre|right split `ratio: 0.5`, so the ToC opened at roughly 37.5% of the
viewport regardless of its contents.

**Resolution.** A ToC auto-fit modelled on the left pane's existing
`auto_fit_width` / `auto_fit_tree` pair, not a new mechanism:

```
entry = indent(level) + measured title + 2 x button h-pad
        + row right pad + reserved scrollbar gutter
width = clamped(max over entries, RIGHT_PANEL_MIN_WIDTH,
                viewport x RIGHT_PANEL_MAX_WIDTH_FRACTION)
```

Measured with `table_width::fira_sans_width` — real cosmic-text metrics against
`FIRA_SANS_REGULAR` — **not** hoarfrost `pane_solver::auto_fit_pane_width`, whose
`chars x 0.6` heuristic is not a measurement. Every furniture term is the same
token `view::toc_body` builds the row from (the view's own `2.0/4.0/2.0` padding
literals were replaced with those tokens for the same reason), so solver and view
cannot drift.

The fit fires on a **landed document load**, not at startup replay: at boot no
document is open, so a fit there could only measure zero entries and pin the pane
to its floor. A `FileLoaded` the orchestrator drops for a de-selected file must
not move the pane, and that is tested.

Measured pane width, before → after: `"# Intro"` at 1200px `448.5 → 150.0`
(floor); a 41-char heading at 1200px `448.5 → 298.6` (exact fit); a 90-char
heading at 2000px `748.5 → 539.8` (exact fit).

**The clamps genuinely fight long content, and this was reported rather than
worked around.** A 90-char heading needs 539.8px; at a 1200px window the 30% cap
allows 360px, so it is truncated, and at 800px even an ordinary 41-char heading
is truncated at the 240px cap. `RIGHT_PANEL_MIN_WIDTH` and
`RIGHT_PANEL_MAX_WIDTH_FRACTION` were left alone — that is decision **F-3**, not
a solver change. The test pins `min(needed, cap)` instead of pretending the
content fits.

**Follow-on found and fixed in the same commit.** While wiring the fit it emerged
that `apply_pane_resize`'s right arm still derived its nested region from
`LEFT_PANEL_DEFAULT_WIDTH` when no left width had been remembered, while the grid
actually sits at the seeded ratio's width — 249px vs 280px at a 1000px viewport.
The first right-drag therefore landed the pane **9.6px** away from the width the
reducer had just stored (measured: stored 214.4, laid out 224.0). That is the
same bug class #12 and #14 fix, and it undercut the round-trip claim committed in
`47fb203`. A drag replay is a ratio replay like any other, so the drag arm now
reads `current_left_width`, making it the single home of that rule for all three
callers (drag, startup replay, ToC auto-fit) rather than two of three.
`test_right_resize_without_persisted_left_width_uses_default` pinned the old
behaviour and was corrected, not deleted.

### #17 — Table renders empty — **CLOSED** `0c492e2`

Parsing is not the cause: pulldown-cmark 0.13.4 accepts the all-empty header
as 4 columns and reads `|||||` as an ordinary 4-empty-cell body row, and the
Fair Table Width Algorithm cannot emit 0/NaN widths (it asserts on
non-finite input and clamps a zero-measured column to 1px at
`markdown/table_layout.rs:99`).

The collapse is downstream, in iced's `Table` layout:

1. `render/table.rs` wrapped every cell in `container(..).width(Length::Fill)`.
2. Every column was pinned to `Length::Fixed(w)`.
3. iced's `Table::new` folds the column widths with `Length::enclose`; an
   all-`Fixed` table folds to `Shrink`, so iced **rewrites the first column
   to `Length::Fill`** (`iced_widget-0.14.2/src/table.rs:129`).
4. In the first layout pass a cell is skipped as "fluid" when its width is
   fill — so because *every* cell declares `Fill`, all cells are skipped and
   `metrics.columns` stays all-zero.
5. In the second pass each fixed column is laid out under
   `Limits::width(Fixed(w))`, and `Limits::width` **clamps down** to the
   recorded max of 0 — so columns 1..n get 0px and column 0 absorbs the
   viewport.

**Resolution.** A cell now declares its own column's `Length` rather than
`Fill`, so the first pass measures it; the `Length` is computed once per
column and shared with its cells so the two passes cannot disagree. Two
further corrections surfaced: a zero-column model must never reach the widget
(`Table::layout` divides by the column count and panics — proven by test), and
the scroll fallback must add back the two outer padding bands, because
`total_width` excludes them and column 0 was being laid out 10px narrower than
solved. The module doc's false premise was corrected (RUST-041).

Measured on the owner's table at 900×600, before → after:
`col0 860.0 → 20.0`, `col1 0.0 → 15.0`, `col2 0.0 → 100.0`, `col3 0.0 → 120.0`
— exactly the FTWA solve.

Per D-4 the layout-level tests came back: the owner's exact table, empty header
cells, an all-empty column, a 200→1200px viewport fuzz, the wide-table scroll
fallback, and the two `test/wiki` fixtures that had been referenced by no test
at all since the egui deletion.

---

## 5. Commit log

Appended as increments land. Each entry is one defect (or a tightly coupled
set), test-first, with the quality gate passing.

| Commit | Defects | Summary |
|--------|---------|---------|
| `06e8f9c` | 1, 3, 5, 7, 8, 9, 10, 11, 13, 15, 16, 18 | Dialog, overlay, ToC and tab-strip repairs (pre-ledger baseline) |
| `a49eaf5` | 4 | Tools card sized to 80% of the viewport on both axes (D-1) |
| `0c492e2` | 17 | Table cells declare their own column's `Length`; layout-level table tests restored (D-4) |
| `47fb203` | 12, F-2 | Pane grid gutter + gutter-aware ratio math; persisted widths replayed at startup (D-2) |
| `86a2343` | 2 | Batch card rows stacked on the form-row rung (`space::SM`) |
| `f962051` | 14 | ToC pane auto-fits to its content; nested-region derivation unified (drag arm follow-on) |
| `b913bef` | 6 | Centred modals are user-resizable: `ResizeFloors` + grip + gesture lane (D-3) |
| `03ecc96` | 12 (regression) | Panes painted opaque so `border.subtle` stays in the seams; baseline-free pixel guard |
| — | — | *(ledger closed)* |

### Gate at close

| Command | Result |
|---|---|
| `cargo check --quiet` | exit 0, no warnings |
| `cargo nextest run --workspace --status-level fail --show-progress none` | **8933 passed**, 7 skipped, 0 failed |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| `cargo fmt --check` | exit 0 |
| `cargo doc --no-deps --quiet` | exit 0 — 3 warnings, all pre-existing at `06e8f9c` (`fonts.rs` ×2, `app.rs` ×1) |

Test count moved `8847 → 8930 → 8933` (+86) across the six closed defects and
the #12 regression. Every layout claim is a measurement from
`iced_test::Simulator`, not an inference from reading the widget tree.

### Note on the `a49eaf5` baseline

Two defects in the tree at ledger-open were not in the reported 18 but had to
be settled first:

- A stray duplicate `pub mod token_lint_gate;` in `ui_iced/mod.rs`, added by
  an earlier session, broke the **test** build. `cargo check` passed because
  the duplicate only collides under `cfg(test)` — a reminder that the gate's
  first command is not sufficient on its own.
- The orphan `probe_tmp.rs` (D-5) broke `token_lint_gate_tests`, which walks
  the source tree independently of the module graph and counts every non-test
  `.rs` file. It is archived at
  `$TMP/opencode/probe_tmp.rs.bak` and deleted from the tree.
