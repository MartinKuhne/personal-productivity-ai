# UI Defect Ledger — 18 reported defects (branch `more-iced`)

> **Status:** In progress — 13 of 18 landed, 5 open. This ledger is the single
> record of the reported defect list, the owner's rulings on the ambiguous
> ones, what has actually landed, and what remains.
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
| 2 | Batch vertical spacing | **Open** | see §4 |
| 3 | 'mode' before 'pattern' | **Landed** `06e8f9c` | `batch_dialog.rs:850` `config_body_order(mode)`; tests `batch_dialog_tests.rs:308,324,338` |
| 4 | Tools card at 80% of viewport | **Landed** (this branch, §5) | `tools_dialog.rs:124` `VIEWPORT_FILL`; `:151` `TOOLS_SIZING`; tests `tools_dialog_tests.rs:838-1130` |
| 5 | "10 groups" | **Landed** `06e8f9c` | `ui/strings.rs:160` `tools_group_count()`; test `tools_dialog_tests.rs:735` |
| 6 | Tools card resizable | **Open** | see §4 |
| 7 | Tools title-bar 'x' | **Landed** `06e8f9c` | `tools_dialog.rs:915` `close_button()`; tests `:1563,1602,1683` |
| 8 | Tools footer 'Close' | **Landed** `06e8f9c` | `tools_dialog.rs:953` `footer_close_button()`; tests `:1622,1654,1665` |
| 9 | Background Ops opaque | **Landed** `06e8f9c` | `logs.rs:895` routed through `movable_shell`; opaque fill `movable_shell.rs:160`; test `logs_tests.rs:994` |
| 10 | Background Ops 'x' | **Landed** `06e8f9c` | `movable_shell.rs:221` `close_action`; test `logs_tests.rs:1037` |
| 11 | Background Ops 'Close' | **Landed** `06e8f9c` | `logs.rs:848` `logs_close_button_widget()`; tests `logs_tests.rs:1063,1088,1125,1172` |
| 12 | Pane separation | **Open** | see §4 |
| 13 | ToC font rung | **Landed** `06e8f9c` | `view.rs:93` `TOC_TEXT_RUNG` = `UI_COMPACT` (13px/18px); tests `view_tests.rs:406,421,452` |
| 14 | Content-sized ToC panel | **Open** | see §4 |
| 15 | Selected-tab fill conforms | **Landed** `06e8f9c` | `center.rs:1541` slot is `Length::Shrink`, fill painted on the content-sized `surface` (`:1468`) — box and fill are one rectangle by construction; tests `center_tests.rs:1707,1739` |
| 16 | Tab overflow overlap | **Landed** `06e8f9c` | `center.rs:1391` strip rides a horizontal scrollable; tests `center_tests.rs:1791,1822,1871,1908` |
| 17 | Table renders empty | **Open** | see §4 |
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

---

## 4. Open work

Each item states the measured cause and the change sites, so the work is
actionable without re-deriving it.

### #2 — Batch dialog vertical spacing

The card reserves a fixed band per form row whether or not that row has a
caption: hoarfrost `form_field` reserves `HEADER_HEIGHT` 20 + `BAND_GAP` 4 +
`CONTROL_SLOT_HEIGHT` 32 + `CAPTION_SLOT_HEIGHT` 16 = **72px of content per
row**, and the body column adds `space::MD` (12px) between rows — about 438px
for five rows in File mode, most of it empty reserved space.

Change sites: `ui_iced/batch_dialog.rs:988` (body spacing), `:1003` (card
spacing), and the reserved empty caption band. No test pins the current
spacing, so the negative test comes first.

### #6 — Tools card resize (D-3)

Nothing exists to extend: `modal_card` (`overlay.rs:556`) is a styled
`Container` and `modal_centered` (`overlay.rs:618`) is scrim + centred card.
Needs a corner grip, per-surface user geometry on the overlay slot, a gesture
lane in the reducer, and clamping through hoarfrost `clamp::resize`. The
movable-card stack (`hoarfrost/surface/movable.rs`, `movable_shell.rs:259`,
`update.rs:216`) is the reference implementation for the gesture, not a thing
to reuse directly.

### #12 — Pane separation (D-2)

`layout.rs:302` `middle_band()` builds the `PaneGrid` with no `.spacing(..)`,
so iced's default `0.0` applies, and all three panes paint the same
`surface.base` — nothing separates them. The module doc at `layout.rs:25-27`
states the zero-spacing premise the decision removes.

Change sites: `.spacing()` on the grid; `apply_pane_resize` (`layout.rs:230`)
left ratio `width / viewport` and right `available = viewport - left`;
`auto_fit_tree` (`update.rs:1075`); `RESIZE_LEEWAY_PIXELS` (`layout.rs:51`),
because iced's grab area is `spacing + leeway` and `UIF-PANE-001` mandates a
10px grab; the ratio expectations pinned in `layout_tests.rs:133-300`; and
F-2's startup replay.

### #14 — Content-sized ToC panel

The right pane's width is a pure ratio: `middle_band_configuration()`
(`layout.rs:136`) gives the centre|right split `ratio: 0.5`, so the ToC opens
at roughly 37.5% of the viewport regardless of its contents.

The pattern to copy already exists for the left pane: `left::auto_fit_width`
(`panels/left.rs:837`) driven by `update.rs:1064` `auto_fit_tree`, measured
with `table_width::fira_sans_width` (real cosmic-text metrics). Needs the
ToC equivalent — max over entries of indent + measured title at
`TOC_TEXT_RUNG` + row padding + scrollbar allowance — clamped through
`layout::clamped_width` and replayed onto `band.right_split`.

### #17 — Table renders empty

Parsing is not the cause: pulldown-cmark 0.13.4 accepts the all-empty header
as 4 columns and reads `|||||` as an ordinary 4-empty-cell body row, and the
Fair Table Width Algorithm cannot emit 0/NaN widths (it asserts on
non-finite input and clamps a zero-measured column to 1px at
`markdown/table_layout.rs:99`).

The collapse is downstream, in iced's `Table` layout:

1. `render/table.rs:305` wraps every cell in `container(..).width(Length::Fill)`.
2. `render/table.rs:252` pins every column to `Length::Fixed(w)`.
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

Fix direction: stop declaring `Fill` on cells whose column is fixed, so the
first pass measures them. Per D-4, the layout-level render tests are restored
alongside it, covering empty header cells, an empty mid-table row, all-empty
columns, and narrow viewports.

---

## 5. Commit log

Appended as increments land. Each entry is one defect (or a tightly coupled
set), test-first, with the quality gate passing.

| Commit | Defects | Summary |
|--------|---------|---------|
| `06e8f9c` | 1, 3, 5, 7, 8, 9, 10, 11, 13, 15, 16, 18 | Dialog, overlay, ToC and tab-strip repairs (pre-ledger baseline) |
| `a49eaf5` | 4 | Tools card sized to 80% of the viewport on both axes (D-1) |
| — | — | *(ledger opened)* |

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
