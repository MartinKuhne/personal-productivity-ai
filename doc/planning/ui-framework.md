# UI Framework & Widget Library — Design Specification

> **Status:** Final — authoritative design specification (Aspirational / Target Architecture per `doc/planning/AGENTS.md`).
> **Date:** 2026-10-23
> **Scope:** Visual design language, design token hierarchy (spatial, sizing, surface geometry constants, radii, typography, semantic colors, and temporal/motion budgets), UI/style-guide best practices, ergonomic geometry, interaction state machines, controlled data models, responsive callback contracts, atomic widgets (`UIF-ATOM-*`), composite widget patterns (`UIF-COMP-*`), unified z-order/escape-rank layering (`UIF-ZORD-*`), and the nine core application surfaces: Main Window, Navbar, Multiple Panes, Tabbed Windows, Input Dialogs, Modal Dialogs, Movable Overlay Dialogs, Fixed Menus, and Context Menus.

Statements in this specification that express pattern-based conclusions or broad design principles are prefixed with `[INFERENCE]` or `[GENERALIZATION]` in accordance with the Anti-Hallucination Protocol. All design questions, geometry formulas, contrast thresholds, data models, and interaction contracts in this specification are finalized and normative.

---

## 1. Design Philosophy & Usability Principles

[GENERALIZATION] High-usability desktop interfaces achieve visual harmony and speed of use by combining mathematical consistency (strict spatial, typographic, temporal, and perceptual color scales) with unambiguous interaction affordances (explicit state feedback, predictable focus choreography, and forgiving hit geometry).

### 1.1 Core Design Pillars

1. **Perceptual Harmony Through Tokenization (`UIF-PRIN-001`):**
   * Every visual and temporal property—color, elevation, spacing, sizing, border radius, stroke weight, typography, and motion/debounce duration—is selected from a closed set of semantic design tokens (`§2`), deterministic arithmetic compositions of those tokens (`space.sm + space.2xs = 10px` menu-row inset; `2 × space.xs + border.hairline = 9px` menu separator height), or the closed table of named **Surface Geometry Constants** (`§2.2`). Raw numeric literals in individual widget or surface definitions are forbidden.
2. **High-Density Desktop Ergonomics (`UIF-PRIN-002`):**
   * [INFERENCE] Productivity applications require compact vertical rhythm to maximize visible workspace content without sacrificing pointer accuracy. Controls balance compact visual height (`28px–36px`) with a strict minimum interactive hit target (`24px × 24px`).
3. **Zero Layout Shift Across State Transitions (`UIF-PRIN-003`):**
   * Hovering, focusing, selecting, validating, loading (`busy`), or disabling a widget MUST NOT alter its outer bounding box or shift adjacent elements. Validation caption slots, trailing input icon/action slots, leading selection indicators (`✓`), `14px` busy spinner slots, `10px` scrollbar gutters, and status bars reserve their geometry at rest.
4. **Visibility of System Status & Non-Collapsing Chrome (`UIF-PRIN-004`):**
   * Unavailable actions remain visible in a clearly distinguishable `Disabled` state (retaining their ordinal focus slot, label, command identity, and close policy) rather than disappearing. Empty panels or filtered views display structured placeholder states (`UIF-COMP-014`) rather than collapsing to zero dimensions.
5. **Dual-Modality Access (Pointer + Keyboard) & Redundant Cueing (`UIF-PRIN-005`):**
   * Every interactive operation reachable by pointer MUST have a deterministic keyboard path (`Tab`, arrow keys, `Home`/`End`, `PageUp`/`PageDown`, `Enter`, `Space`, `Escape`).
   * Meaning is never communicated by color alone: selection, validation errors, dirty state, diff lines, stepper states, and special item types pair color tokens with shape, weight, border, or badge cues.
6. **Emphasis Hierarchy, Density Discipline & Optical Alignment (`UIF-PRIN-006`):**
   * **Single High-Emphasis Focal Point:** At most **one** control per surface container (modal footer, toolbar cluster, card, or form) uses the high-emphasis `Primary` (`color.accent.fill`) or `Danger` (`color.status.danger.fill`) solid fill. Sibling actions step down to `Secondary` (`surface.raised` + `border.strong`) or `Ghost` (`transparent`) so visual priority is unambiguous.
   * **Action Row Button Ordering:** Dialog and form footers right-align their button group with a `space.sm` (`8px`) gap in the order **`[Secondary / Cancel] [Primary / Confirm]`**, with utility or help actions pinned far-left (`<── Fill ──>`). In destructive confirmations, initial keyboard focus lands on `[Cancel]`.
   * **Row Density Homogeneity:** Every control in a single horizontal row (`Navbar`, `FormRow`, `OverflowToolbar`, `FilterBar`, or dialog footer) MUST share the same `ControlDensity` (`Compact = 28px` or `Standard = 32px`). Mixing `28px` and `32px` controls in the same horizontal band is forbidden.
   * **Baseline & Optical Centering:** Adjacent inline text and controls align on their typographic baseline; directional glyphs (`▸`, `▾`, `◀`, `▶`) and circular/pill badges center on their optical bounding weight inside `16×16px` / `24×24px` slots.
7. **Microcopy, Capitalization & Truncation Style Guide (`UIF-PRIN-007`):**
   * **Sentence-Case Mandate & Host `SPEC.md` Literal Pin Exception:** All UI labels—buttons, menu items, tabs, form labels, table column headers, tooltips, and dialog titles—use **Sentence case** (capitalizing only the first word and proper nouns/acronyms such as `Markdown`, `PDF`, `MCP`, `JSON`). ALL-CAPS and Title Case on multi-word chrome labels are prohibited, **except** for the closed **Host `SPEC.md` Literal Pin Table** (`'Ok'`, `'Cancel'`, `'Open in Editor'`, `'Show in File Explorer'`, `'Create Directory ...'`, `'Close Others'`, `'Close All'`, `'Copy Path'`, and `'Windows...'`), which are pinned verbatim by `UI-009`..`UI-034` and `UI-062` in `src/app/ui/SPEC.md` (`[RUST-043]`).
   * **Verb-First Action Labels:** Dialog and form CTAs use specific imperative verbs reflecting the consequence (`Save changes`, `Rename file`, `Delete 3 items`, `Run batch`, `Apply`) rather than generic `OK` or `Yes`/`No` (with Input Dialog `'Ok'` / `'Cancel'` exempted per `UI-009`).
   * **ASCII Action Ellipsis (`...`) vs. Unicode Truncation Ellipsis (`…`, `U+2026`):**
     * **ASCII Three-Dot Suffix (`...`):** Appended unspaced (`Rename...`, `Move...`, `Delete...`, `Batch...`, `Tools...`, `Actions...`; `'Create Directory ...'` exempted per `UI-015`) **if and only if** activating the control opens a secondary dialog or prompt requiring additional user input before executing. Immediate commands (`Save`, `Format Markdown`) and view toggles (`System logs`) never carry `...`.
     * **Unicode Horizontal Ellipsis (`…`, `U+2026`):** Reserved exclusively for layout-time text truncation (`Title…`, `src/app/…/mod.rs`), input field placeholders (`"Search content…"`), and breadcrumb elision triggers (`[…]`).
   * **Tabular Numerals (`TabularNums`) & Alignment:** All numeric values (`NumberBox`, `Slider`/`RangeSlider` readouts, `DatePicker`/`TimePicker` segments, `Badge` counts, `ProgressBar` percentages, timestamps, line/column counters, `PaginationBar` ranges, and numeric `DataTable` columns) MUST render with fixed-width tabular numerals (`FontFamily::Mono` or `font-variant-numeric: tabular-nums`) so live updates never cause horizontal text jitter. Numeric table columns right-align both header and cell values.
   * **Context-Aware Truncation & Automatic Tooltip:**
     * **End-Truncation (`Title…`):** Mandatory for prose titles, tree nodes, menu labels, form captions, and table cells.
     * **Middle-Truncation (`src/app/…/mod.rs`):** Mandatory for filesystem paths, URIs, and hierarchical/git identifiers to preserve both root context and basename/extension.
     * Any single-line label or caption truncated at layout time MUST automatically register a `Tooltip` (`UIF-ATOM-007`) exposing the full untruncated string on hover or keyboard focus.
8. **Single Tab-Stop Composite Navigation, Disjoint Hit Zones & `A11yContract` (`UIF-PRIN-008`):**
   * **WAI-ARIA Roving Focus Pattern:** Composite collections (`SegmentedButton`, `RadioGroup`, `OverflowToolbar`, `TabStrip`, `Menu`, `TreeView`, `DataTable`, `GridView`, `CalendarView`, `NavigationRail`, `Stepper`) act as a **single `Tab` stop**. Pressing `Tab` enters the composite on its active/last-focused item, arrow keys (`Up`/`Down`/`Left`/`Right`, `Home`/`End`, `PageUp`/`PageDown`) rove focus internally, and the next `Tab` exits to the next control in the surface's focus ring.
   * **Bounded Wrap vs. Virtualized Clamp:** Small bounded groups (`Menu`, `RadioGroup`, `SegmentedButton`, `NavigationRail`, `CommandPalette`) **wrap** arrow navigation from last to first; spatial and large virtualized collections (`Slider`, `RangeSlider`, `TreeView`, `DataTable`, `GridView`) **clamp** at the first and last item so holding `Down` at the bottom never teleports a `100,000`-row scroll viewport back to row `0`.
   * **Disjoint Hit Zones & Submenu Aiming Corridors:** Nested or adjacent interactive targets (`Tab` body vs. `[×]` close button; `Chip` body vs. `[×]`; `ListTile` row vs. trailing action button; `TreeView` row vs. `▸` disclosure chevron; `DataTable` sort header vs. `10px` resize grip; `Expander` header vs. trailing action button) MUST maintain strictly disjoint hit rectangles separated by at least `space.2xs` (`2px`). Cascading submenus implement a `150ms` (`time.delay.submenu`) diagonal aiming grace window so moving the pointer diagonally from a parent row into an open child submenu card does not collapse the submenu.
   * **Pure Accessibility Projection (`A11yContract`):** Every widget model exposes a pure semantic descriptor (`role`, `name`, `value`, `expanded`, `selected`, `checked`, `busy`, `invalid`, `disabled`) so assistive technologies and headless test selectors never rely on pixel scraping.
9. **"Reward Early, Punish Late" Validation Choreography (`UIF-PRIN-009`):**
   * **Punish Late (Pristine Entry):** Untouched (`pristine`) form fields MUST NOT paint `Invalid` red borders or error captions before the user commits (`on_commit` / `blur`) or submits the form—except when typing a disallowed character in real time (e.g., `/` or `\0` in a filesystem basename under `UIF-INP-003`).
   * **Reward Early (Immediate Recovery):** Once a control is in `BaseState::Invalid`, live edits (`Live<T>`) re-validate on every keystroke and clear the `Invalid` visual state **immediately on the exact frame** the value becomes valid.
   * **Non-Destructive Error Recovery:** Validation failures or rejected background operations MUST preserve the user's uncommitted text buffer, scroll offset, and selection range intact.

---

## 2. Foundation Design Tokens (`UIF-TOK-*`)

### 2.1 Spatial Grid & Spacing Scale (`UIF-TOK-001`)

All padding, gaps, margins, and alignment offsets adhere to a base-4px spatial grid (with a 2px half-step for dense inline micro-gaps):

| Token | Value (Logical px) | Semantic Role |
|---|---|---|
| `space.2xs` | `2` | Micro gap between anchor point and floating menu, sub-pixel scroll bottom tolerance, disjoint hit-zone buffer, tight two-line subtitle gap, slider tick width |
| `space.xs` | `4` | Compact inner padding (tags, badges, inline code, menu card inset, tree row horizontal inset, separator vertical margin, tab close inset, slider tick height) |
| `space.sm` | `8` | Standard sibling control gap, toolbar button spacing, form row spacing, viewport clamp margin, text area vertical padding |
| `space.md` | `12` | Standard horizontal padding inside buttons, inputs, tabs, table cells, and menu rows |
| `space.lg` | `16` | Card and dialog inner padding, tree depth nominal indentation step, section separation |
| `space.xl` | `24` | Major layout section spacing, empty-state container padding |
| `space.2xl` | `32` | Minimum viewport margin around large modal cards |

### 2.2 Sizing, Hit Targets, Geometry Tokens & Surface Geometry Constants (`UIF-TOK-002`)

| Token | Value (Logical px) | Semantic Role |
|---|---|---|
| `size.hit_target.min` | `24 × 24` | Mandatory minimum pointer hit area for any interactive control (close `×`, clear `×`, resize grip, tree/TOC rows, scroll step unit) |
| `size.bar.status` | `24` | Compact status and footer bar height (movable overlay footer, menu section heading row, command palette legend, compact chip height) |
| `size.control.sm` | `28` | Compact controls: navbar buttons, dropdown triggers, inline search action buttons, folder list rows, bar chip height |
| `size.control.md` | `32` | Standard controls: dialog CTA buttons, single-line text inputs, selectable menu rows, table sticky header, movable overlay title bar |
| `size.control.lg` | `36` | Primary structural bars: Top Navbar height, Tab Strip height, movable overlay filter bar, navigation rail item height, side sheet header |
| `size.indicator.dot` | `6` | Unsaved state dot (`●`), status badge dot, and inactive pager pip diameter |
| `size.splitter.grab` | `10` | Interactive pointer grab width centered over a `1px` pane/table/sheet split divider (`5px` each side) and permanently reserved scrollbar gutter |
| `size.checkbox.box` | `16 × 16` | Visual glyph box for `Checkbox` and `Radio` controls (centered inside a `24 × 24` minimum hit target; radio inner dot is `size.checkbox.box / 2 = 8 × 8px`) |
| `size.switch.track` | `36 × 20` | Outer pill track dimensions for `ToggleSwitch` controls |
| `size.switch.thumb` | `14 × 14` | Inner sliding thumb diameter for `ToggleSwitch` controls (`(20 - 14) / 2 = 3px` derived inset from track edges) |
| `size.slider.track_height` | `4` | Horizontal rail thickness for `Slider` and `RangeSlider` controls |
| `size.slider.thumb` | `16 × 16` | Visual thumb diameter for `Slider`, `RangeSlider`, and `ColorPicker` 2D crosshair controls (`24 × 24` pointer hit target) |
| `size.progress.bar_height` | `4` | Linear `ProgressBar` track thickness |
| `size.spinner.sm` | `14` | Inline button/input busy `Spinner` diameter (`SPINNER_SLOT` reservation) |
| `size.spinner.md` | `20` | Panel or dialog loading `Spinner` diameter |
| `size.icon.sm` | `14` | Inline status, badge, sort arrow (`▲`/`▼`), leading segment `✓` (`CHECK_SLOT`), and close `×` glyph optical size |
| `size.icon.md` | `16` | Standard button, menu leading slot, resize grip glyph, and tree item icon optical size |
| `size.icon.lg` | `20` | Brand mark and empty-state illustration icon size |
| `size.avatar.sm` | `24 × 24` | Compact inline identity `Avatar` / `PersonPicture` diameter |
| `size.avatar.md` | `32 × 32` | Standard navbar, comment, and list tile `Avatar` diameter |
| `size.avatar.lg` | `36 × 36` | Profile and card header `Avatar` diameter |
| `size.nav_rail.compact` | `48` | Collapsed vertical `NavigationRail` width (icon-only `36×36px` destinations with warm-handoff tooltips) |
| `size.nav_rail.expanded` | `220` | Expanded `NavigationView` drawer default width (`[180px, 320px]` clamp) |
| `size.calendar.cell` | `32 × 32` | Individual day cell dimensions inside `CalendarView` / `DatePicker` (`7 cols × 6 rows = 224 × 192px` grid) |
| `size.color.well` | `180 × 140` | 2D saturation-value (`SV`) picker canvas inside `ColorPicker` flyout |
| `size.palette.card` | `560 × 380` | Default floating `CommandPalette` (`Ctrl+K`) card dimensions (`top: 64px` in `Z-Band 3`) |
| `size.sheet.default` | `360` | Default docked/overlay `SideSheet` inspector width (clamped `[280px, 520px]`) |
| `size.toast.width` | `360` | Floating `Toast` / `SnackBar` notification card width in `Z-Band 1` |

* **Closed Table of Surface Geometry Constants (Carved Out by `UIF-PRIN-001` & Checked by `token_lint`):**
  * **Main Window (`§4.1`):** `WINDOW_DEFAULT = 1000 × 700`, `WINDOW_MIN = 640 × 480`, `BOTTOM_BAR_HEIGHT = 44..120`.
  * **Panes & Tree (`§4.3`, `§3B.6`):** `LEFT_PANE_MIN_W = 150`, `CENTER_PANE_MIN_W = 240`, `RIGHT_PANE_MIN_W = 120`, `TREE_LABEL_MIN_W = 96`.
  * **Tabs & Navbar (`§4.2`, `§4.4`):** `NAV_FILTER_W = 140..220`, `TAB_WIDTH = 96..220`.
  * **Input Dialog (`§4.5`):** `INPUT_DIALOG_WIDTH = 320`, `FOLDER_LIST_MAX_H = 200`.
  * **Modal Dialogs (`§4.6`):** `EDITOR_MODAL_MIN = 520 × 380`, `EDITOR_MODAL_MAX = 960 × 760`, `TOOLS_MODAL_W = 720`, `TOOLS_MODAL_MIN_H = 320`, `BATCH_MODAL_MAX = 560 × 480`, `ABOUT_MODAL_W = 440`, `ATTRIBUTIONS_MODAL_SIZE = 640 × 480`.
  * **Movable Overlay (`§4.7`):** `MOVABLE_DEFAULT = 680 × 440`, `MOVABLE_MIN = 420 × 260`, `TITLE_REACHABLE_MIN_W = 120`.
  * **Menus, Popovers & Pickers (`§3.4`, `§3.7`, `§3.10`, `§4.8`):** `MENU_CARD_WIDTH = 224..320`, `TOOLTIP_MAX_W = 280`, `POPOVER_WIDTH = 280..360`, `COMBO_FLYOUT_MAX_H = 280`, `AUTOSUGGEST_FLYOUT_MAX_H = 240`, `CALENDAR_FLYOUT_W = 256`, `TIME_FLYOUT_W = 200..240`, `COLOR_FLYOUT_W = 240`.

### 2.3 Corner Radii & Stroke Weights (`UIF-TOK-003`)

| Token | Value (Logical px) | Semantic Role |
|---|---|---|
| `radius.window` | `8` | Outer application window frame radius |
| `radius.card` | `6` | Floating modal dialogs, input dialogs, movable overlay windows, command palette, calendar/time/color flyouts, teaching popovers, surface cards, and menu cards |
| `radius.widget` | `4` | Interactive controls: buttons, text inputs, dropdowns, checkboxes, filter/input chips, tooltips, code blocks, expander headers, settings rows, menu rows |
| `radius.pill` | `999` | Status chips, `Assist` chips, avatars, switches, scrollbar thumbs, pager pips, and numeric count badges |
| `border.hairline` | `1` | Default structural dividers, card perimeters, resting input borders, inset menu focus outline, table borders |
| `border.focus` | `2` | Outer keyboard focus ring, active tab accent bar, active splitter bar, tree/rail selection bar, focused/invalid input border, `Spinner` arc/track stroke (`SPINNER_STROKE`) |

### 2.4 Typography Scale, Font Families & Weight Composition (`UIF-TOK-004`)

No text anywhere in the interface may render smaller than `12px` (`FONT_SIZE_FLOOR = 12`).
* **First-Class Font Family Token (`FontFamily`):** Every typographic style resolves a `FontFamily` alongside its size rung and weight:
  * `FontFamily::Sans` (`font.family.sans`): Default proportional family for UI chrome, labels, menus, tables, and prose.
  * `FontFamily::Mono` (`font.family.mono`, `.mono()` on any rung): Mandatory fixed-width family for code blocks, inline code, file paths, timestamps, numeric/date/time inputs (`NumberBox`, `DatePicker`, `TimePicker`, `Slider` readouts), `ColorPicker` hex strings, `KeybindRecorder` keycap pills, editor buffers, and structured log/debug views.
* **Font Weight Tokens (`Weight`):** `Weight::Regular` (`400`), `Weight::Medium` (`500`), `Weight::SemiBold` (`600`), and `Weight::Bold` (`700`).
* **Canonical Rung + Weight Compositions:** Each of the six rungs defines its size, authoritative integer `Line Height (px)`, resting weight, emphasis weight, tracking, and `allowed_weights` set. In particular, `CAPTION` (`12px`) with `Weight::SemiBold` is the canonical style for menu section headings (`§4.8.1`) and `DataTable` column headers (`§3B.7`), and `UI_COMPACT` (`13px`) with `Weight::Bold` is the canonical style for `MovableOverlay` title bars (`§4.7.1`).
* **Authoritative Line Height:** The integer `Line Height (px)` column is authoritative for all box-model and row-height arithmetic; the parenthetical multiplier (`1.33×`, `1.38×`, etc.) is an informational 2-decimal approximation.

| Token | Size (px) | Line Height (px) | Resting / Emphasis Weight | Allowed Weights | Tracking | Primary Usage |
|---|---|---|---|---|---|---|
| `type.caption` | `12` | `16` (`1.33×`) | Regular / Medium | Regular, Medium, SemiBold | `+0.01em` | Shortcut hints, validation error captions, status chips, menu section headings (`SemiBold`), table headers (`SemiBold`), timestamps (`Mono`) |
| `type.ui_compact` | `13` | `18` (`1.38×`) | Regular / SemiBold | Regular, Medium, SemiBold, Bold | `0` | Dense UI lists: tree rows, TOC, tab labels (`SemiBold` when active), menu items, movable overlay title (`Bold`), code/diff/log lines (`Mono`) |
| `type.body` | `14` | `20` (`1.43×`) | Regular / Medium | Regular, Medium, SemiBold | `0` | Document prose, dialog prompt text, text input values, settings row titles (`Medium`), inline editor buffer (`Mono`) |
| `type.title_sm` | `16` | `24` (`1.50×`) | SemiBold / Bold | SemiBold, Bold | `-0.01em` | Dialog titles (`Bold`), navbar brand title (`Bold`), side sheet / empty-state headlines (`SemiBold`) |
| `type.title_md` | `20` | `28` (`1.40×`) | Bold / Bold | SemiBold, Bold | `-0.015em` | Document H2 headings, About dialog header |
| `type.title_lg` | `24` | `32` (`1.33×`) | Bold / Bold | Bold | `-0.02em` | Document H1 headings |

### 2.5 Semantic Color System & Contrast Invariants (`UIF-TOK-005`)

The color system uses a dark-first elevation model where higher z-layers use progressively lighter neutral surfaces, reinforced by explicit perimeter borders.

[INFERENCE] Because WCAG 2.1 relative luminance requires background fills under white text (`L = 1.0000`) to have `L <= 0.1833` (for `>= 4.50:1` contrast) while foreground text and active borders on elevated dark surfaces (`surface.overlay` `L = 0.0093`, `surface.active` `L = 0.0235`) require `L >= 0.2167` (for `>= 4.50:1` text) or `L >= 0.1705` (for `>= 3.00:1` borders), **accent and danger tokens are split by polarity role** into background fills (`*.fill`, `*.fill_active`) and foreground/border tokens (`*.text`, `*.border`). All contrast figures below are exact WCAG 2.1 ratios computed from raw sRGB relative luminance (`(L_lighter + 0.05) / (L_darker + 0.05)`):

| Semantic Token | sRGB Value | Rel. Luminance (`L`) | Contrast vs `surface.base` | Contrast vs `surface.raised` | Contrast vs `surface.overlay` | Contrast vs `surface.active` | Contrast vs `text.strong` | Role & Usage Contract |
|---|---|---|---|---|---|---|---|---|
| `color.surface.sunken` | `RGB(6, 6, 8)` | `0.0019` | `1.02:1` | `1.08:1` | `1.14:1` | `1.42:1` | `20.24:1` | Deep input wells and scrollable log/code viewport backgrounds |
| `color.surface.base` | `RGB(9, 9, 11)` | `0.0028` | `1.00:1` | `1.06:1` | `1.12:1` | `1.39:1` | `19.90:1` | Root window canvas, focused input background, and primary document/tree pane background |
| `color.surface.raised` | `RGB(18, 18, 20)` | `0.0061` | `1.06:1` | `1.00:1` | `1.06:1` | `1.31:1` | `18.71:1` | Navbar, bottom bar, navigation rail, table headers, cards, inactive tabs, resting `Secondary`/`Danger` buttons |
| `color.surface.code` | `RGB(20, 20, 22)` | `0.0071` | `1.08:1` | `1.02:1` | `1.04:1` | `1.29:1` | `18.40:1` | Fenced code blocks and resting text input / unchecked toggle background |
| `color.surface.overlay` | `RGB(24, 24, 27)` | `0.0093` | `1.12:1` | `1.06:1` | `1.00:1` | `1.24:1` | `17.72:1` | Floating surfaces: menus, toasts, input dialogs, modal dialogs, movable overlays, command palette, active tab |
| `color.surface.hover` | `RGB(32, 32, 36)` | `0.0147` | `1.23:1` | `1.15:1` | `1.09:1` | `1.14:1` | `16.24:1` | Interactive control hover fill, hovered menu/table/tree item, active submenu parent |
| `color.surface.active` | `RGB(42, 42, 46)` | `0.0235` | `1.39:1` | `1.31:1` | `1.24:1` | `1.00:1` | `14.29:1` | Pressed quiet/secondary control fill, selected segment/table/rail/calendar-range fill, movable overlay title bar |
| `color.surface.scrim` | `RGBA(9, 9, 11, 0.55)` | — | — | — | — | — | — | Dimmed backdrop behind blocking modal dialogs, input dialogs, modal side sheets, and command palette |
| `color.border.subtle` | `RGB(40, 40, 40)` | `0.0212` | `1.35:1` | `1.27:1` | `1.20:1` | `1.03:1` | `14.74:1` | Passive structural dividers (`RestingDivider`): pane split lines at rest, menu separators, resting inputs |
| `color.border.strong` | `RGB(120, 120, 120)` | `0.1878` | `4.51:1` | `4.24:1` | `4.01:1` | `3.24:1` | `4.42:1` | Active/floating perimeters (`Judged`): cards, modals, menus, active buttons, tree guides, scrollbar thumbs (`3.68:1` on `hover`) |
| `color.accent.fill` | `RGB(93, 96, 238)` | `0.1687` | `4.14:1` | `3.90:1` | `3.69:1` | `2.98:1` | **`4.80:1`** | Solid primary CTA button fill and selected tree/list/calendar/chip background under `text.strong` |
| `color.accent.fill_active` | `RGB(82, 85, 222)` | `0.1356` | `3.52:1` | `3.31:1` | `3.13:1` | `2.52:1` | **`5.66:1`** | Pressed primary CTA button fill under `text.strong` |
| `color.accent.border` | `RGB(129, 140, 248)` | `0.3020` | `6.67:1` | `6.27:1` | **`5.94:1`** | **`4.79:1`** | `2.98:1` | Keyboard focus rings (`6.17:1` on `code`, `5.44:1` on `hover`), active tab top bar, rail bar, pressed/focused accent borders |
| `color.accent.text` | `RGB(100, 200, 255)` | `0.5124` | `10.66:1` | `10.05:1` | **`9.49:1`** | **`7.66:1`** | `1.87:1` | Hyperlinks, fuzzy match spans, menu/segment checkmarks (`✓`), sort arrows, submenu indicators (`>`) |
| `color.text.primary` | `RGB(210, 210, 210)` | `0.6445` | `13.16:1` | `12.38:1` | **`11.72:1`** | **`9.45:1`** | `1.51:1` | Default body and resting control label text (`12.17:1` on `code`, `10.72:1` on `hover`) |
| `color.text.strong` | `RGB(255, 255, 255)` | `1.0000` | `19.90:1` | `18.71:1` | **`17.72:1`** | **`14.29:1`** | `1.00:1` | Emphasized text, hovered/focused/selected labels, all foreground elements on `*.fill` tokens |
| `color.text.muted` | `RGB(160, 160, 160)` | `0.3515` | `7.61:1` | `7.16:1` | **`6.78:1`** | **`5.47:1`** | `2.61:1` | Secondary descriptions (`7.04:1` on `code`), breadcrumb separators, inactive tab labels, shortcut hints, section headings |
| `color.text.disabled` | `RGB(140, 140, 140)` | `0.2623` | `5.92:1` | `5.57:1` | `5.27:1` | `4.25:1` | `3.36:1` | Disabled button/menu/input text and empty placeholders (`6.02:1` on `sunken`; WCAG 1.4.3 exempt) |
| `color.status.success` | `RGB(100, 255, 100)` | `0.7515` | `15.19:1` | `14.28:1` | **`13.52:1`** | **`10.91:1`** | `1.31:1` | Completion states, positive status chips (`✓ parallel`), diff additions (`+`), copy confirmation |
| `color.status.warning` | `RGB(220, 130, 0)` | `0.3118` | `6.86:1` | `6.45:1` | **`6.10:1`** | **`4.93:1`** | `2.90:1` | Unsaved indicator dot (`●`), warning icons (`⚠`), filled rating stars (`★`), paused progress bar |
| `color.status.danger.text` | `RGB(255, 92, 92)` | `0.2969` | `6.57:1` | `6.18:1` | **`5.85:1`** | **`4.72:1`** | `3.03:1` | Error captions, error icons, invalid input borders, diff deletions (`-`), resting `Danger` button label/border (`5.36:1` on `hover`) |
| `color.status.danger.fill` | `RGB(216, 28, 20)` | `0.1548` | `3.88:1` | `3.65:1` | `3.46:1` | `2.79:1` | **`5.13:1`** | Hovered `Danger` button fill, error step/badge fill, and hovered destructive menu row fill under `text.strong` |
| `color.status.danger.fill_active` | `RGB(202, 24, 16)` | `0.1325` | `3.46:1` | `3.25:1` | `3.08:1` | `2.48:1` | **`5.71:1`** | Pressed `Danger` button fill and pressed destructive menu row fill under `text.strong` |
| `color.editor.inverted.*` | BG: `RGB(255,255,255)`<br>FG: `RGB(0,0,0)`<br>Err: `RGB(180,0,0)` | `1.0000`<br>`0.0000`<br>`0.0999` | — | — | — | — | `21.00:1` (FG)<br>`7.14:1` (Err) | Dedicated high-contrast light surface (`EditorPalette`) for the inline document editor modal |

**Mandatory Contrast Rules (`UIF-TOK-006`):**
1. **Active Foreground Text (`< 18px`):** Every non-disabled text token (`text.primary`, `text.strong`, `text.muted`, `accent.text`, `status.success`, `status.warning`, `status.danger.text`) MUST maintain $\ge 4.50:1$ contrast against every neutral surface it appears on (`sunken` through `active`).
2. **Disabled Text Scope:** Per WCAG 2.1 SC 1.4.3, `color.text.disabled` (`RGB(140, 140, 140)`) is exempt from the `4.50:1` active-text floor and sits in the `[4.00:1, 6.00:1]` muted legibility band across standard control surfaces (`base` `5.92:1` through `active` `4.25:1`, and `6.0205:1` on deep `surface.sunken`) so disabled controls remain readable while visually distinct from active controls. Disabled rows and controls on floating surfaces MUST never paint an opaque hover/active fill beneath `text.disabled`, and `text.disabled` MUST NEVER render on `accent.fill` (`1.43:1`) or `status.danger.fill`.
3. **Active UI Chrome, Borders & Focus Indicators:** Active control borders (`border.strong`, `accent.border`, `status.danger.text`) and solid control fills (`accent.fill`, `accent.fill_active`, `status.danger.fill`, `status.danger.fill_active`) MUST maintain $\ge 3.00:1$ contrast against their adjacent surface (`base` through `active` for borders; `base` through `overlay` for solid fills — unbordered solid fills MUST NOT mount directly on `surface.active` where `accent.fill` is `2.98:1`). Automated border/chrome evaluation classifies every stroke into one of four categories:
   * **`Judged`:** Active perimeter borders, tree guides, scrollbar thumbs, and focus rings — MUST measure $\ge 3.00:1$.
   * **`CoincidentWithFill`:** A `1px` perimeter stroke set to the exact same token as its solid button/toggle fill (`accent.fill`, `accent.fill_active`, `status.danger.fill_active`) — boundary contrast is carried by the solid fill against the surrounding surface `base` through `overlay` ($\ge 3.00:1$; `status.danger.fill_active` vs `surface.overlay` measures `3.0789:1`).
   * **`UnpaintedBackground`:** A `transparent` stroke on a rest-state `Ghost` button or unselected menu row — foreground is audited directly against the parent mount surface.
   * **`RestingDivider`:** Passive `1px` structural hairlines (`border.subtle`) — decorative and exempt from the `3.00:1` active-chrome threshold, promoting to `border.strong` on hovered/selected table rows where `border.subtle` falls below `1.15:1` (`1.10:1` on `hover`, `1.03:1` on `active`).
4. **Selection & Solid-Fill Foreground Lock (Role-Based):** Whenever `color.accent.fill`, `color.accent.fill_active`, `color.status.danger.fill`, or `color.status.danger.fill_active` is used as a background fill, **all** foreground labels, secondary subtitles (`ListTile`), disclosure chevrons (`TreeView`), tree guide lines, leading selection bars, split-button seams/chevrons, and icons inside that fill MUST lock/promote to `color.text.strong` (`RGB(255, 255, 255)`, yielding `4.80:1` to `5.71:1`). This lock is strictly **role-based**, not threshold-based: all other tokens (`text.primary` `2.96:1`, `text.muted` `1.84:1`, `text.disabled` `1.43:1`, `border.strong` `1.09:1`, `accent.border` `1.61:1`, `status.danger.text`, `status.warning`, and `accent.text` even where `accent.text` clears `3.00:1` on active fills) are forbidden on `*.fill` and `*.fill_active`.

### 2.6 Temporal, Motion & Choreography Tokens (`UIF-TOK-007`)

[INFERENCE] All interaction timers, hover delays, debounce windows, and transition durations are selected from a closed set of `TimeMs` tokens. To guarantee deterministic headless testing, `TimeMs` timestamps and `reduced_motion: bool` are always **injected as pure parameters** into resolvers and state controllers—never read from an OS wall clock or platform global inside a widget resolver:

| Token | Value (`ms`) | Easing / Curve | Semantic Role & Reduced-Motion Contract |
|---|---|---|---|
| `time.instant` | `0` | `Linear` | Immediate press/focus ring response; mandatory fallback duration for all motion transitions when injected `reduced_motion == true`; `Expander` body mount/unmount (`0px` per-frame layout shift) |
| `time.fast` | `100` | `EaseOutCubic` | Micro-interactions: hover/press fill transitions, `ToggleSwitch` thumb travel, `Checkbox` glyph appear |
| `time.normal` | `180` | `EaseOutCubic` | Flyout, `ComboBox`, `DatePicker`, `TimePicker`, `ColorPicker`, and `SideSheet` open/close transitions |
| `time.slow` | `240` | `EaseInOutCubic` | Modal scrim fade and `NavigationRail` compact $\leftrightarrow$ expanded width transition |
| `time.debounce.live` | `200` | — | Trailing-edge input debounce (`DEBOUNCE_MS = 200`, restarted by every keystroke) before dispatching async `AutoSuggestBox` or `CommandPalette` queries |
| `time.delay.submenu` | `150` | — | Diagonal pointer safe-corridor grace timer (`time.menu.aim_grace`) before switching open submenu branches (`UIF-PRIN-008`, `UIF-FMENU-002`) |
| `time.delay.tooltip_open` | `400` | — | Pointer hover delay (`time.tooltip.hover_delay`) before opening a `Tooltip` (`0ms` on keyboard focus arrival) |
| `time.delay.tooltip_warm` | `100` | — | Warm-handoff window: moving between adjacent icon-only controls (`Navbar`, `NavigationRail`) within `100ms` opens the next tooltip immediately (`0ms` wait) |
| `time.hold.copied` | `1500` | — | Duration a `CodeBlock` `[Copy]` button displays `✓` (`status.success`) before returning to `Rest` |
| `time.toast.auto_dismiss` | `5000` | — | Default auto-dismiss countdown for `Info` and `Success` toasts (paused and banked while `hovered \|\| focused`; forced to `None` for `Warning`/`Danger`) |

---

## 3. Atomic Widget State Matrices (`UIF-ATOM-*`)

### 3.0 Interaction State Model & Focus Orthogonality (`UIF-ATOM-000`)

1. **Two-Part Orthogonal State Resolution:**
   * Interactive state is resolved as `VisualState = { base: BaseState, focused: bool }` rather than a mutually exclusive flat enum, because keyboard focus can coexist with pointer hover, press, and validation error states.
   * **Base State Precedence:** `Disabled > Invalid > Pressed > Hovered > Rest`.
   * **Total Disabled Dominance:** When `Disabled` is active, `focused` is forced to `false` (disabled controls cannot hold focus or display a focus ring, and if an already-focused control becomes disabled, focus advances immediately via slot-preserving fall-forward to the next enabled control in scope).
   * Combining the 5 base states with `focused` yields **9 reachable states**: `Rest`, `Hovered`, `Pressed`, `Invalid`, `Rest + Focused`, `Hovered + Focused`, `Pressed + Focused`, `Invalid + Focused`, and `Disabled`.
2. **Adaptive Focus Ring & Outline Placement:**
   * **Outer Focus Ring (`2px` / `border.focus`, Buttons & Inputs):** Rendered `2px` *outside* the control's outer border so it never clips internal content or overwrites an `Invalid` border.
   * **Inset Focus Outline (`1px` / `border.hairline`, Menu, Tree & Table Rows):** Rendered `1px` *inside* the row perimeter so adjacent rows in dense lists do not clip the ring.
   * **Dynamic Ring Contrast Resolution:** Every focus ring or inset outline is resolved against the adjacent surface/fill: it uses `color.accent.border` whenever `accent.border` achieves $\ge 3.00:1$ contrast against that surface/fill, and automatically promotes to `color.text.strong` (`RGB(255, 255, 255)`) on saturated fills (`Primary` button, hovered/pressed `Danger` button, destructive menu row, checked toggle, selected `accent.fill` tree/list row).
3. **Press-and-Release Activation:**
   * Pointer press inside an enabled control enters the `Pressed` visual state; the command fires **only** when the pointer is released while still inside the control's hit bounds. Releasing outside cancels activation.

#### 3.0.4 Data Binding, Controlled State & Responsive Callback Architecture (`UIF-ATOM-000b`)

[GENERALIZATION] Declarative desktop UI frameworks prevent main-thread stalls and state desynchronization by strictly separating **Controlled Domain Data Models**, **Ephemeral Interaction State**, and **Typed Lifecycle Callbacks**.

1. **Controlled vs. Ephemeral State Separation:**
   * **Controlled Data Model (`Controlled<'a, M>`):** Owned by application state and borrowed read-only by the widget each frame (`value`, `checked`, `selected_ids`, `validation`, `pending`, `busy`, `disabled`).
   * **Ephemeral Interaction State (`Ephemeral<S>`):** Owned by the widget/framework layer (`VisualState`, hover timers, active `GestureSlot`, pre-edit `Snapshot<T>`, `ActivationGate`, IME composition buffer, scroll velocity, open flyout state) so pointer and keyboard feedback renders within `< 1 frame` without round-tripping through background subsystems.
2. **Three-Tier Callback Lifecycle (`Live<T>` vs. `Commit<T>` vs. `Cancel<T>`):**
   * `Live<T>`, `Commit<T>`, and `Cancel<T>` are distinct nominal wrapper types that **share no common conversion trait**, and multi-frame gestures settle through a linear `Gesture<T>` guard whose consuming `.commit(self)` and `.cancel(self)` methods make double-settlement a compile-time error.
   * **`on_change(Live<T>)` (Live / Frame-Immediate):** Emitted on every keystroke (coalesced to at most one emission per pass when the value actually changed), thumb scrub pixel, or hover highlight. Used for live validation or previewing; MUST never block or trigger synchronous disk/network I/O.
   * **`on_commit(Commit<T>)` (Settled / Intentional):** Emitted **exactly once** when an interaction settles—`Enter` key, focus loss (`blur`), pointer release at the end of a slider/splitter/column/overlay drag, or dropdown option selection, superseding any pending `Live<T>` tick from the same pass. Drives persistence, command dispatch, and expensive queries.
   * **`on_cancel(Cancel<T>)` (Revert):** Emitted on `Escape` or pointer release outside hit bounds, restoring the pre-gesture `Snapshot<T>`. Pressing `Escape` while any pointer-captured drag (`Slider`, `RangeSlider`, `SplitDivider`, `DataTable` column resize, `SideSheet` resize, `MovableOverlay` drag/resize) is active cancels that drag first (`Pre-Rank`), even if keyboard focus resides elsewhere.
   * **Dismissal Tier Rule (`Cancel` vs. `Commit` for `on_dismiss`):** Closing a transient overlay without executing its target action emits `Cancel` (`Cancel<()>` on `PopoverModel` and `CommandPaletteModel`; `Cancel<String>` on `AutoSuggestModel`, carrying the dismissed query snapshot so the suggestion flyout remains suppressed for that string until `query` changes). Conversely, dismissing a `NotificationModel` (`§3B.2`) emits `Commit<(NotificationId, DismissReason)>` because notification dismissal is a settled lifecycle transition that permanently removes the item from the active notification queue and records acknowledgment state rather than reverting an uncommitted gesture.
3. **Async & Long-Running Action State (`ActionPending` & `ActivationGate`):**
   * Command action controls (`Button`, `SplitButton` primary zone, `ToolbarItem`, `MenuRow` actions, `Modal` primary CTA) carry `pending: ActionPending { busy: bool, progress: Option<f32>, cancellable: bool }` in their data model. Binary state switches (`ToggleSwitch` / `SwitchModel`) carry `busy: bool` only—locking out re-toggles while an async state flip settles, without determinate progress or cancel sub-affordances on the `36×20px` switch track.
   * To close the double-click race window before the host's `busy = true` projection arrives on a subsequent frame, action and switch controllers maintain an ephemeral **`ActivationGate`** (`Armed -> AwaitingProjection -> Busy`) that suppresses duplicate `on_activate` / `on_toggle` emissions immediately upon first commit.
   * When `pending.busy == true`, a command action control locks its outer bounding box (`UIF-PRIN-003`), paints a `14px` (`size.spinner.sm`) indeterminate arc (`progress: None`) or determinate progress ring (`progress: Some(0.0..=1.0)`) inside the reserved `SPINNER_SLOT`, suppresses `on_activate`, and emits `Cancel<CancelRequest>` (`on_cancel_request`) on press or `Escape` if and only if `pending.cancellable == true`.
4. **Container-Query Responsive Adaptation (`ResponsiveBreakpoint`):**
   * Widgets adapt layout based on **allocated parent container width** via a pure solver taking no global window-width parameter:
     * `Expanded` $\rightarrow$ `Compact`: Secondary labels collapse to icon-only while enforcing the tooltip obligation (`UIF-ATOM-007`) as the same atomic transition, segmented controls compress horizontal padding from `space.md` (`12px`) to `space.sm` (`8px`), and secondary toolbar items move into an overflow `⋯` menu (`UIF-COMP-008`) with `0px` vertical layout shift.
5. **Shared Foundation Primitives & `iced 0.14` Elm Architecture Bridge:**
   * **`iced 0.14` `view` / `update` Mapping:** In `iced 0.14`, `view(&AppState) -> Element<'_, Message>` borrows a `Controlled<'a, M>` model projection from `&AppState`, while `Ephemeral<S>` interaction state resides in the widget's `iced::advanced::widget::Tree` state (or host surface controller). Widget interactions emit `WidgetEvent<L, C, X>` (`Live(Live<L>)`, `Commit(Commit<C>)`, `Cancel(Cancel<X>)`) mapped into the application's `Message` enum via `.map(Message::from)` and processed in `update(&mut AppState, Message) -> Task<Message>`.
   * **Canonical Shared Value Types:**
     ```rust
     pub struct InteractionInput { pub hovered: bool, pub pressed: bool, pub focused: bool, pub invalid: bool, pub disabled: bool }
     pub enum A11yRole { Button, ToggleButton, TextInput, SearchBox, PasswordInput, ComboBox, Slider, Checkbox, Radio, Switch, Tab, MenuItem, TreeItem, GridCell, Dialog, Alert, Status, Region }
     pub struct A11yContract { pub role: A11yRole, pub name: String, pub value: Option<String>, pub expanded: Option<bool>, pub selected: Option<bool>, pub checked: Option<CheckState>, pub busy: bool, pub invalid: bool, pub disabled: bool }
     pub struct Layer { pub bounds: Rectangle, pub fill: Color, pub border: BorderSpec }
     pub struct Icon(pub &'static str);
     pub enum ControlDensity { Compact, Standard } // 28px (size.control.sm) | 32px (size.control.md)
     pub enum PreferredSide { Above, Below, Left, Right }
     pub struct TextRange { pub start: usize, pub end: usize }
     pub struct LineCol { pub line: u32, pub col: u32 } // 1-based line and column
     pub struct ParseDiagnostic { pub location: LineCol, pub message: String }
     pub struct SecretInput(String); // Debug & Display always output "[REDACTED]" per NFR-009
     pub enum ActivationSource { Pointer, Keyboard }
     pub struct ButtonSettlement { pub source: ActivationSource }
     pub struct CancelRequest { pub reason: &'static str }
     pub struct ActionPending { pub busy: bool, pub progress: Option<f32>, pub cancellable: bool }
     ```

### 3.1 Action Controls: Buttons, Segmented Controls, Split Buttons & Hyperlinks (`UIF-ATOM-001`)

Geometry: Height `size.control.sm` (`28px`, compact/navbar) or `size.control.md` (`32px`, dialogs/forms); horizontal padding `space.md` (`12px`); icon-to-label gap `space.sm` (`8px`); corner radius `radius.widget` (`4px`). Icon-only buttons enforce `min_width: 28px`, `min_height: 28px` and MUST carry a non-empty tooltip label (enforced by constructor/lint shield).

| Variant | Rest (`base = Rest`) | Hovered (`base = Hovered`) | Pressed (`base = Pressed`) | Keyboard Focused (`focused = true` Overlay) | Disabled (`base = Disabled`) |
|---|---|---|---|---|---|
| **Primary** | BG: `accent.fill`<br>FG: `text.strong` (`4.80:1`)<br>Border: `1px accent.fill` (`CoincidentWithFill`) | BG: `accent.fill`<br>FG: `text.strong` (`4.80:1`)<br>Border: `1px text.strong` (`4.80:1`) | BG: `accent.fill_active`<br>FG: `text.strong` (`5.66:1`)<br>Border: `1px accent.fill_active` (`CoincidentWithFill`) | Base (`Rest`/`Hovered`/`Pressed`) + `2px text.strong` outer focus ring (`4.80:1`–`5.66:1` vs fill) | BG: `surface.raised`<br>FG: `text.disabled`<br>Border: `1px border.subtle` |
| **Secondary** | BG: `surface.raised`<br>FG: `text.primary` (`12.38:1`)<br>Border: `1px border.strong` (`4.24:1`) | BG: `surface.hover`<br>FG: `text.strong` (`16.24:1`)<br>Border: `1px border.strong` (`3.68:1`) | BG: `surface.active`<br>FG: `text.strong` (`14.29:1`)<br>Border: `1px accent.border` (`4.79:1`) | Base (`Rest`/`Hovered`/`Pressed`) + `2px accent.border` outer focus ring (`4.79:1`–`6.27:1`) | BG: `surface.base`<br>FG: `text.disabled`<br>Border: `1px border.subtle` |
| **Ghost / Quiet** | BG: `transparent`<br>FG: `text.primary`<br>Border: `1px transparent` (`UnpaintedBackground`) | BG: `surface.hover`<br>FG: `text.strong` (`16.24:1`)<br>Border: `1px border.subtle` | BG: `surface.active`<br>FG: `text.strong` (`14.29:1`)<br>Border: `1px border.strong` (`3.24:1`) | Base (`Rest`/`Hovered`/`Pressed`) + `2px accent.border` outer focus ring | BG: `transparent`<br>FG: `text.disabled`<br>Border: `1px transparent` |
| **Danger** | BG: `surface.raised`<br>FG: `status.danger.text` (`6.18:1`)<br>Border: `1px status.danger.text` (`6.18:1`) | BG: `status.danger.fill`<br>FG: `text.strong` (`5.13:1`)<br>Border: `1px text.strong` (`5.13:1`) | BG: `status.danger.fill_active`<br>FG: `text.strong` (`5.71:1`)<br>Border: `1px status.danger.fill_active` (`CoincidentWithFill`) | At `Rest`: + `2px accent.border` ring (`6.27:1`);<br>When `Hovered`/`Pressed`: + `2px text.strong` ring (`5.13:1`–`5.71:1`) | BG: `surface.base`<br>FG: `text.disabled`<br>Border: `1px border.subtle` |

* **`UIF-ATOM-001a` — Standard Button Data Model & Callbacks:**
  * **Data Model:**
    ```rust
    pub struct ButtonModel {
        pub label: Option<String>,
        pub leading_icon: Option<Icon>,
        pub trailing_icon: Option<Icon>,
        pub variant: ButtonVariant, // Primary | Secondary | Ghost | Danger
        pub density: ControlDensity, // Compact (28px) | Standard (32px)
        pub pending: ActionPending,
        pub disabled: bool,
        pub tooltip: Option<String>,
    }
    ```
  * **Callbacks:** `on_activate(Commit<ButtonSettlement>)` (emitted on pointer press-and-release inside hit bounds or `Enter`/`Space` when focused and `!disabled && !pending.busy`, gated by `ActivationGate`), `on_cancel(Cancel<ButtonSettlement>)` (pointer release outside bounds or `Escape` while pointer-pressed, carrying pre-gesture snapshot), and `on_cancel_request(Cancel<CancelRequest>)` (press or `Escape` while `pending.busy == true && pending.cancellable == true`).
* **`UIF-ATOM-001b` — `ToggleButton` & `SegmentedButton` (Single / Multi-Select):**
  * **Geometry & Visuals:** Joined horizontal strip (`height: 28px` or `32px`, outer radius `radius.widget = 4px`, `1px border.strong` perimeter). Unselected segments resolve through `ButtonVariant::Secondary`; selected segments paint as `Secondary (Pressed)` (`BG: surface.active`, `FG: text.strong` `14.29:1`, `Border: 1px accent.border` `4.79:1`). Because a pressed unselected segment and a resting selected segment share `BG: surface.active`, **every segment—selected or unselected—permanently reserves the `14px` (`size.icon.sm`) `CHECK_SLOT` leading tick box** (`0px` label shift per `UIF-PRIN-003`), painting a `14px` `✓` glyph in `accent.text` (`7.66:1`, or `text.disabled` when disabled) when selected (`UIF-PRIN-005`). A standalone 1-segment `ToggleButton` uses `MultiSelect` mode so re-activating the segment toggles it off (whereas `SingleSelect` treats re-picking the active segment as a no-op).
  * **Data Model & Callbacks:**
    ```rust
    pub struct SegmentItem<Id> { pub id: Id, pub label: String, pub icon: Option<Icon>, pub disabled: bool }
    pub enum SelectionPolarity { SingleSelect, MultiSelect }

    pub struct SegmentedModel<Id> {
        pub segments: Vec<SegmentItem<Id>>,
        pub selected: BTreeSet<Id>,
        pub mode: SelectionPolarity, // SingleSelect | MultiSelect
        pub density: ControlDensity,
        pub disabled: bool,
    }
    ```
    Callbacks: `on_selection_change(Commit<BTreeSet<Id>>)` (`Left`/`Right` arrow keys rove and wrap focus across enabled segments; `Space`/`Enter` toggles in `MultiSelect` or selects in `SingleSelect`).
* **`UIF-ATOM-001c` — `SplitButton` & `DropDownButton`:**
  * **Geometry & Per-Variant Seam Contrast:** Two-zone control (`height: 28px` or `32px`) with independent hit, hover, and press tracking for the primary action half (`padding: [0, 12px]`) and the `28px`-wide trailing menu chevron half (`▾`). Hovering or pressing the primary half never highlights the chevron half or opens the menu. The `1px` internal vertical divider seam and `▾` chevron resolve per variant:
    * `Secondary`: seam uses `1px border.strong` (`4.24:1` on `surface.raised`); chevron uses `text.primary` at rest and `text.strong` on hover/open.
    * `Primary`: because `border.strong` (`1.09:1`) and `text.muted` (`1.84:1`) fail contrast on `accent.fill`, the seam uses `1px text.strong` (`4.80:1`) and the chevron uses `text.strong` (`4.80:1`) per `UIF-TOK-006` Rule 4.
  * **Data Model & Callbacks:**
    ```rust
    pub struct MenuRow<ActionId> { pub id: ActionId, pub label: String, pub shortcut: Option<String>, pub destructive: bool, pub disabled: bool }

    pub struct SplitButtonModel<ActionId> {
        pub primary: ButtonModel,
        pub menu_rows: Vec<MenuRow<ActionId>>,
        pub menu_open: bool,
    }
    ```
    Callbacks: `on_primary_activate(Commit<ButtonSettlement>)` (`Enter`/`Space` or click release on primary zone), `on_menu_toggle(Live<bool>)` (chevron press-and-release or `Alt+Down`/`Down`), `on_menu_select(Commit<ActionId>)` (mounts `MenuCascade` flyout in `Z-Band 2`; `Escape` closes the menu via `FocusState::pop_trap()` and restores focus to the split button without firing the primary action).
* **`UIF-ATOM-001d` — `Hyperlink` Control:**
  * **Geometry & Visuals:** Inline or standalone interactive text atom (`min_height: 24px` hit target, `radius.widget = 4px` focus ring resolved against the mount surface). Rest: `FG: accent.text` (`10.66:1` on `base`, `9.49:1` on `overlay`) with `1px` underline; Hovered: `FG: text.strong` on `surface.hover` pill fill; Pressed: `FG: accent.text` on `surface.active` pill fill; Disabled: `FG: text.disabled` with underline removed (emits neither commit nor hover URI).
  * **Data Model & Callbacks:** `HyperlinkModel { label: String, uri: String, external: bool, disabled: bool }` $\rightarrow$ `on_open(Commit<HyperlinkTarget>)` (`HyperlinkTarget { uri, external }`), `on_hover_uri(Live<Option<String>>)`.

### 3.2 Text Inputs, Search Fields, AutoSuggest & Multi-Line Editors (`UIF-ATOM-002`)

Geometry: Height `size.control.md` (`32px`, or `28px` in compact toolbars); horizontal padding `space.md` (`12px`); radius `radius.widget` (`4px`).
* **Zero-Shift Trailing Slot Partition (`UIF-ATOM-002b`):** Every input kind (`Entry`, `Search`, `DropdownTrigger`, `Password`, `TextArea`) partitions its inner width into `text_area_bounds` and `trailing_slot_bounds` (`24×24px` for entry/dropdown/password; `28×28px` for search) as pure functions of the outer control rectangle alone. Trailing slot content resolves by strict priority: **`reveal (Password U+1F441) > invalid (⚠) > clear (×) > idle (🔍 / ▾)`**. Changing slot content or growing text length MUST NOT shift `text_area_bounds` by even `1px`.

| State | Background | Text / Placeholder / Icon | Border & Focus Ring |
|---|---|---|---|
| **Rest** | `surface.code` | Text: `text.primary` (`12.17:1`) · Placeholder: `text.disabled` (`5.50:1`) · Icon: `text.muted` (`7.04:1`) | Border: `1px solid border.subtle` (`RestingDivider`) |
| **Hovered / Pressed** | `surface.code` | Text: `text.primary` (`12.17:1`) · Placeholder: `text.muted` (`7.04:1`) · Icon: `text.primary` (`12.17:1`) | Border: `1px solid border.strong` (`4.17:1`) |
| **Focused (Valid)** | `surface.base` | Text: `text.strong` (`19.90:1`) · Placeholder: `text.muted` (`7.61:1`) · Icon: `text.primary` (`13.16:1`) · Selection: `accent.fill` | Border: `2px solid accent.border` (`6.67:1`) |
| **Invalid (Unfocused)** | `surface.base` | Text: `text.strong` (`19.90:1`) · Placeholder: `text.muted` (`7.61:1`) · Trailing icon: `status.danger.text` (`6.57:1`) | Border: `2px solid status.danger.text` (`6.57:1`) |
| **Invalid + Focused** | `surface.base` | Text: `text.strong` (`19.90:1`) · Placeholder: `text.muted` (`7.61:1`) · Trailing icon: `status.danger.text` (`6.57:1`) | Border: `2px solid status.danger.text` (`6.57:1`) + Outer Ring: `2px accent.border` (`6.67:1`) |
| **Disabled** | `surface.raised` | Text: `text.disabled` (`5.57:1`) · Placeholder: `text.disabled` · Icon: `text.disabled` | Border: `1px solid border.subtle` |

* **`UIF-ATOM-002a` — Single-Line Text Input & Search Field Data Model & Callbacks:**
  * **Data Model:**
    ```rust
    pub struct TextInputModel {
        pub value: String,
        pub placeholder: String,
        pub selection: Option<TextRange>,
        pub validation: Option<String>,
        pub read_only: bool,
        pub disabled: bool,
        pub clearable: bool,
    }
    ```
  * **Snapshot, Read-Only & `Escape` Contract:** The pre-edit `Snapshot<String>` is captured lazily on the **first edit after focus** (so `Tab` traversal or host syncs while focused never overwrite the snapshot). Bound inputs intercept `Escape` before inner widget blur so reverting via `Escape` keeps keyboard focus inside the field. With no edit session open, `Escape` emits `on_clear(Commit<String>)` **only** when `clear_affordance(kind)` holds (`Search` or `clearable == true`); a plain non-clearable entry ignores `Escape` when unedited. When `read_only == true`, edit callbacks are withheld, the control paints as enabled `Rest`/`Hovered`, and the focus caret cell is dropped.
  * **Callbacks:** `on_change(Live<String>)` (coalesced per pass), `on_commit(Commit<String>)` (`Enter` or focus loss with an open edit session, superseding pending `Live`), `on_cancel(Cancel<String>)` (`Escape` with an open edit session, restoring snapshot), `on_clear(Commit<String>)` (trailing `×` click or unedited `Escape` when clearable), `on_focus_change(bool)`.
* **`UIF-ATOM-002c` — `PasswordInput` (Secret Entry):**
  * **Anatomy & Security Contract:** Built on `InputKind::Password`. Holds the secret in `value: SecretInput` whose `Debug` and `Display` implementations always redact the plaintext (`NFR-009`), deriving `masked_len` (`U+2022` `•` per grapheme) and `revealed_value` from `value` without storing a duplicate plaintext copy. Suppresses `Ctrl+C`/`Ctrl+X` while masked. The trailing `24×24px` reveal toggle uses single-scalar glyph `👁` (`U+1F441`) and outranks the `Invalid` mark in `slot_content` so a rejected password can still be inspected. Enforces `revealed => focused && !disabled && !read_only`: calling `toggle_revealed` while unfocused, disabled, or read-only is refused with zero emission, and `note_focus(false)` automatically resets `revealed` to `false`.
  * **Data Model & Callbacks:**
    ```rust
    pub struct PasswordInputModel {
        pub value: SecretInput,
        pub placeholder: String,
        pub revealed: bool,
        pub validation: Option<String>,
        pub read_only: bool,
        pub disabled: bool,
    }
    ```
    Callbacks: `on_change(Live<SecretInput>)`, `on_commit(Commit<SecretInput>)`, `on_cancel(Cancel<SecretInput>)`, `on_reveal_toggle(Live<bool>)`.
* **`UIF-ATOM-002d` — `AutoSuggestBox` (Search & Command Completion):**
  * **Anatomy & Stable Flyout Identity:** Joins `TextInputController` with `MenuCascade` (`Z-Band 2`, `Escape Rank 3`, `AUTOSUGGEST_FLYOUT_MAX_H = 240px`, `32px` rows, `4px` gap). The flyout's `MenuId` is minted once from the **control identity** (`menu_id_for`), never from the query text, so typing never orphans the open flyout. Query changes are trailing-edge debounced by `DEBOUNCE_MS = 200` (`time.debounce.live`, restarted on every keystroke) and tagged with a monotonic `generation: u64` counter (`apply_suggestions` rejects out-of-order responses and increments `stale_dropped`). When `suggestions` is empty, no `Z-Band 2` card is mounted (`NoFocusableRows`). A dismissed query latches against its own text so an in-flight response for the already-dismissed string cannot reopen the flyout.
  * **Two-Stage `Escape` & Roving Highlight:** Roving `Down`/`Up` updates `highlighted_index` without moving the iced text caret out of the input. First `Escape` closes the open suggestion flyout; second `Escape` clears the disposable search `query` to `""`; third `Escape` is unconsumed and bubbles up the `Escape Rank` stack.
  * **Data Model & Callbacks:**
    ```rust
    pub enum CommitSource { Enter, Click, Tab }
    pub struct SuggestionItem<Id> { pub id: Id, pub label: String, pub detail: Option<String>, pub disabled: bool }
    pub struct SuggestionCommit<Id> {
        pub id: Id,
        pub label: String,
        pub source: CommitSource, // Enter | Click | Tab
    }

    pub struct AutoSuggestModel<Id> {
        pub query: String,
        pub suggestions: Vec<SuggestionItem<Id>>,
        pub highlighted_index: Option<usize>,
        pub busy: bool,
        pub disabled: bool,
    }
    ```
    Callbacks: `on_query_change(Live<String>)`, `on_suggestion_highlight(Live<usize>)`, `on_suggestion_commit(Commit<SuggestionCommit<Id>>)`, `on_dismiss(Cancel<String>)` (emitted on Stage-1 `Escape` flyout close, carrying the dismissed `query` snapshot so the flyout stays suppressed for that string until `query` changes).
* **`UIF-ATOM-002e` — `MultiLineTextArea`:**
  * **Anatomy & Exact Token Box-Model Arithmetic:** Built on `InputKind::TextArea` with `EditHistory` undo/redo stack. Vertical padding is `2 × space.sm = 16px` (`8px` top + `8px` bottom), horizontal padding is `space.md = 12px`, and `line_height` is `20px` (`type.body`) or `18px` (`type.ui_compact`). Outer height is solved from tokens as `height = clamp(line_count, min_rows, max_rows) × line_height + 2 × space.sm` (so `3` lines of `type.body` = `3 × 20px + 16px = 76px`).
  * **Data Model & Callbacks:**
    ```rust
    pub struct TextAreaModel {
        pub text: String,
        pub cursor: LineCol,
        pub selection: Option<TextRange>,
        pub min_rows: u16,
        pub max_rows: Option<u16>,
        pub dirty: bool,
        pub validation: Option<ParseDiagnostic>,
        pub read_only: bool,
        pub disabled: bool,
    }
    ```
    Callbacks: `on_change(Live<String>)` (`Enter` inserts `\n` and never settles), `on_cursor_change(Live<LineCol>)`, `on_submit(Commit<String>)` (`Ctrl+Enter` / `Cmd+Enter` emits `Submitted` *without* closing the open edit/revert session, so a subsequent `Escape` still reverts what was typed), `on_commit(Commit<String>)` (focus loss with an open edit session), `on_cancel(Cancel<String>)` (`Escape` reverts to focus-entry snapshot), `on_undo()`, `on_redo()`.

### 3.3 Boolean & Choice Controls: Checkbox, Radio & ToggleSwitch (`UIF-ATOM-003`)

Geometry: All three controls share one `TogglePolarity` (`Unchecked/Off` vs `Checked/Indeterminate/On`) style resolver and enforce a minimum `24px × 24px` (`size.hit_target.min`) pointer hit target extending across their inline label (`space.sm = 8px` gap between indicator and `13px`/`14px` label).
* **`Checkbox`:** `16×16px` (`size.checkbox.box`) visual box, `radius.widget` (`4px`), supporting `Unchecked`, `Checked` (`✓`), and `Indeterminate` (`−` dash).
* **`Radio`:** `16×16px` (`size.checkbox.box`) circular indicator (`radius.pill = 999px`) with a centered `size.checkbox.box / 2 = 8×8px` inner dot (`color.text.strong`) when selected.
* **`ToggleSwitch`:** `36×20px` (`size.switch.track`) pill track (`radius.pill`) with a `14×14px` (`size.switch.thumb`) sliding thumb at a derived `(20 - 14) / 2 = 3px` inset.

| Control & Polarity | Rest (`base = Rest`) | Hovered (`base = Hovered`) | Pressed (`base = Pressed`) | Keyboard Focused (`focused = true`) | Disabled (`base = Disabled`) |
|---|---|---|---|---|---|
| **Unchecked / Off** (`Checkbox`, `Radio`, `Switch`) | BG: `surface.code`<br>Border: `1px border.strong` (`4.17:1`)<br>Switch thumb: `text.muted` (`7.04:1`)<br>Label: `text.primary` | BG: `surface.hover`<br>Border: `1px accent.border` (`5.44:1`)<br>Switch thumb: `text.strong` (`16.24:1`)<br>Label: `text.strong` | BG: `surface.active`<br>Border: `1px accent.border` (`4.79:1`)<br>Switch thumb: `text.strong` (`14.29:1`)<br>Label: `text.strong` | Base + `2px accent.border` outer focus ring (`4.79:1` on `active` to `6.17:1` on `code` / `6.67:1` on `base`) | BG: `surface.raised`<br>Border: `1px border.subtle`<br>Thumb/Label: `text.disabled` |
| **Checked / Indeterminate / On** (`Checkbox`, `Radio`, `Switch`) | BG: `accent.fill`<br>Border: `1px accent.fill` (`CoincidentWithFill`)<br>Glyph/Thumb: `text.strong` (`4.80:1`)<br>Label: `text.primary` | BG: `accent.fill`<br>Border: `1px text.strong` (`4.80:1`)<br>Glyph/Thumb: `text.strong` (`4.80:1`)<br>Label: `text.strong` | BG: `accent.fill_active`<br>Border: `1px accent.fill_active` (`CoincidentWithFill`)<br>Glyph/Thumb: `text.strong` (`5.66:1`)<br>Label: `text.strong` | Base + `2px text.strong` outer focus ring (`4.80:1`–`5.66:1` vs fill) | BG: `surface.raised`<br>Border: `1px border.subtle`<br>Glyph/Thumb/Label: `text.disabled` |

* **Data Models & Callbacks:**
  ```rust
  pub enum CheckState { Unchecked, Checked, Indeterminate }
  pub struct CheckboxModel {
      pub state: CheckState,
      pub all_enabled_children_checked: bool, // True iff every enabled child under a parent tri-state checkbox is Checked
      pub label: String,
      pub description: Option<String>,
      pub disabled: bool,
  }
  pub enum RadioOrientation { Vertical, HorizontalWrap }
  pub struct RadioOption<Id> { pub id: Id, pub label: String, pub description: Option<String>, pub disabled: bool }
  pub struct RadioGroupModel<Id> { pub options: Vec<RadioOption<Id>>, pub selected: Option<Id>, pub orientation: RadioOrientation, pub disabled: bool }
  pub struct SwitchModel { pub on: bool, pub label: String, pub on_text: Option<String>, pub off_text: Option<String>, pub busy: bool, pub disabled: bool }
  ```
  * **`CheckboxModel`:** **`on_toggle(Commit<CheckState>)`** (`Space` or click; clicking an `Indeterminate` checkbox transitions to `Unchecked` if and only if `all_enabled_children_checked == true`, and to `Checked` otherwise; a leaf `Indeterminate` with `all_enabled_children_checked == false` always transitions to `Checked`).
  * **`RadioGroupModel<Id>`:** **`on_select(Commit<Id>)`** (`Up`/`Down`/`Left`/`Right` roves and wraps focus across enabled options and selects; re-clicking the already-selected option is a no-op).
  * **`SwitchModel`:** **`on_toggle(Commit<bool>)`** (`Space` or click; refused with zero emission while `busy == true || disabled == true`. Carries `busy: bool` only—no `progress` or `cancellable`—as a binary `36×20px` switch settles to `on: bool` without a secondary cancel sub-gesture).

### 3.4 Selection Pickers: `ComboBox` / `Select` (`UIF-ATOM-004`)

* **Geometry & Visuals:** Trigger resolves through `InputKind::DropdownTrigger` (`height: 28px` toolbars or `32px` forms/dialogs; horizontal padding `space.md = 12px`; reserved `24×24px` trailing slot with `14px` `▾` chevron in `text.muted` at rest and `text.strong` on hover/open).
* **Flyout Engine, Filter Policy & Keyboard Contract:**
  * `ComboHost` reuses `MenuCascade` (`MenuTreeKind::Fixed`, `Z-Band 2`, `Escape Rank 3`, `min_width = trigger.width`, `COMBO_FLYOUT_MAX_H = 280px`) with `menu_id` derived from the **control identity** (never the filter string) and a `✓` (`accent.text`) in the `16×16px` leading slot of the selected row.
  * **Filter Matching (`FilterPolicy`):** Defaults to `FilterPolicy::ByWidgetSubstring` (case-insensitive substring match that keeps the currently selected option row visible in the list) or `FilterPolicy::ByCaller`. When `0` enabled rows match, the flyout card is refused (`IgnoredReason::NoFocusableRows`) while `on_filter_change(Live<String>)` continues firing.
  * **Open/Close & Snapshot Rules:** From a closed trigger, `Down` opens onto the first enabled row and `Up` opens onto the last enabled row (`Home`/`End` jump to first/last enabled row). Non-editable (`editable == false`) selects own no text buffer and ignore printable keys. Clicking the trigger chevron while open closes the flyout without reverting (`on_open_change(Live(false))`). Pressing `Escape` while open closes the flyout and restores the pre-open snapshot of **both** `selected` and `filter_query` (`on_cancel`). When `read_only == true`, options remain browsable but never commit.
* **Data Model & Callbacks:**
  ```rust
  pub struct SelectOption<Id> { pub id: Id, pub label: String, pub disabled: bool }
  pub enum FilterPolicy { ByWidgetSubstring, ByCaller }

  pub struct ComboBoxModel<Id> {
      pub items: Vec<SelectOption<Id>>,
      pub selected: Option<Id>,
      pub placeholder: String,
      pub filter_query: Option<String>,
      pub filter_policy: FilterPolicy, // ByWidgetSubstring | ByCaller
      pub editable: bool,              // Default false (closed Select)
      pub read_only: bool,
      pub open: bool,
      pub validation: Option<String>,
      pub disabled: bool,
  }
  ```
  Callbacks: `on_open_change(Live<bool>)`, `on_filter_change(Live<String>)`, `on_commit(Commit<Id>)`, `on_cancel(Cancel<Option<Id>>)`.

### 3.5 Continuous, Stepped & Dual-Thumb Range Controls: `Slider`, `RangeSlider` & `NumberBox` (`UIF-ATOM-005`)

* **Shared Model & Cross-Control Arbitration:** A single `RangeModel` backs both `Slider` and `NumberBox`. When both controls are bound to the same `RangeModel` simultaneously, whichever control holds an open `GestureSlot` (active thumb drag or active text-entry session) owns the live value projection; settling one immediately updates the other through the shared controlled `RangeModel`.
* **`Slider` Anatomy & Visual States:**
  * Track height `size.slider.track_height` (`4px`, `radius.pill`); unfilled rail `surface.active` with `1px border.subtle` (`RestingDivider`) perimeter; filled active rail `accent.fill` (`3.69:1` vs `overlay`, `4.14:1` vs `base`; interior filled-to-unfilled seam is `2.98:1` reinforced by the thumb boundary and reported without failing perimeter judgment).
  * Thumb `16×16px` (`size.slider.thumb`, `24×24px` hit target) in `text.strong` with `2px accent.border` border at rest (`Hovered`/`Pressed` thumb uses `2px text.strong` on `accent.fill`; when `validation.is_some()`, thumb border uses `2px status.danger.text`; thumb contrast is evaluated as the union of face + border). Stepped tick marks measure `space.2xs × space.xs` (`2×4px`) in `border.strong`.
* **`NumberBox` Anatomy:**
  * `32px`-high numeric text input (`FontFamily::Mono`, `14px`, tabular numerals) with either a **Stacked Stepper** (`28×32px` total trailing zone = `size.control.sm × size.control.md`, containing two `28×16px` `▲`/`▼` buttons) or a **Side-by-Side Stepper** (`48×32px` total trailing zone = `2 × size.hit_target.min` width, containing two disjoint `24×24px` `[−]`/`[+]` buttons).
* **Precision Step Arithmetic:** Step alignment re-derives the grid index from `min` in `f64` on every step (`min + (idx as f64) * step`) so repeated stepping never accumulates float drift. Non-finite (`NaN`/`±∞`) inputs are rejected before clamping, inverted bounds (`min > max`) are normalized at read time, and `step <= 0.0` is treated as continuous.
* **Data Model & Callbacks:**
  ```rust
  pub struct RangeModel {
      pub value: f64,
      pub min: f64,
      pub max: f64,
      pub step: Option<f64>,
      pub precision: u8,
      pub unit_suffix: Option<String>,
      pub validation: Option<String>,
      pub disabled: bool,
  }
  ```
  * **`on_change(Live<f64>)` (Live Scrub):** Emitted continuously during pointer thumb drag or stepper hold.
  * **`on_commit(Commit<f64>)` (Settled):** Emitted **once** via `GestureSlot` on pointer release (even if released outside track bounds), stepper click release, `Enter`, focus loss, or clamped keyboard step (`Left`/`Down` = `-step`, `Right`/`Up` = `+step`, `PageDown`/`PageUp` = `±10 × step`, `Home` = `min`, `End` = `max`).
  * **`on_cancel(Cancel<f64>)`:** Emitted on `Escape` during an active pointer-captured drag, restoring the pre-drag value.
* **`UIF-ATOM-005b` — `RangeSlider` (Dual-Thumb Interval Control):**
  * **Style & Ergonomic Rules:**
    * Shares the `4px` (`size.slider.track_height`) rail and two independent `16×16px` (`size.slider.thumb`, `24×24px` hit target) thumbs (`Lower`, `Upper`). Only the rail segment between `[lower, upper]` paints `color.accent.fill`; outer segments paint `color.surface.active` with `1px border.subtle`.
    * **Non-Crossing & Endpoint Disambiguation:** Dragging or stepping `lower` clamps to `[min, upper - min_span]`; dragging or stepping `upper` clamps to `[lower + min_span, max]` (never swapping thumb identity mid-drag). When `lower == upper` (or separated by `min_span`) and clicked, the thumb with room to move in the pointer's direction captures the gesture.
    * **Value Readout Pill:** While either thumb is `Hovered`, `Pressed`, or `Focused`, a tabular-numeral readout pill (`FontFamily::Mono`, `12px`, `surface.overlay` + `1px border.strong`) renders `4px` above the active thumb without shifting surrounding layout (`UIF-PRIN-003`).
  * **Controlled Data Model & Callbacks:**
    ```rust
    pub struct RangeSliderModel {
        pub lower: f64,
        pub upper: f64,
        pub min: f64,
        pub max: f64,
        pub min_span: f64,
        pub step: Option<f64>,
        pub precision: u8,
        pub unit_suffix: Option<String>,
        pub validation: Option<String>,
        pub disabled: bool,
    }
    ```
    Callbacks: `on_change(Live<(f64, f64)>)`, `on_commit(Commit<(f64, f64)>)` (`Tab` traverses `Lower` thumb $\rightarrow$ `Upper` thumb $\rightarrow$ next control), `on_cancel(Cancel<(f64, f64)>)` (`Escape` during active drag reverts to pre-drag interval).

### 3.6 Interactive Chips & Filter Pills (`UIF-ATOM-006`)

* **Geometry & Radius Assignment:** Height is selected by `ChipHeight::{Compact (24px / size.bar.status), Bar (28px / size.control.sm)}`; horizontal padding `space.sm` (`8px`); gap `space.xs` (`4px`). Corner radius is `radius.pill` (`999px`) for `Assist` chips and status/count pills, and `radius.widget` (`4px`) for `Filter` and `Input` chips. Removable `Input` chips reserve a disjoint `24×24px` (`size.hit_target.min`) trailing `×` hit target (`UIF-PRIN-008`).
* **Visual States & Mounting Surface (`ChipPolarity`):**
  * **Unselected (`Assist` / `Filter` / `Input`):** `BG: surface.raised`, `FG: text.primary` (`12.38:1`), `Border: 1px border.strong` (`4.24:1`). Hover: `BG: surface.hover`, `FG: text.strong`.
  * **Selected (`Filter`, mounted on `base`, `raised`, `overlay`, or `hover` — never `surface.active` where `accent.fill` is `2.98:1`):** `BG: accent.fill`, `FG: text.strong` (`4.80:1`), `Border: 1px accent.border`, plus a leading `14px` (`size.icon.sm`) `✓` glyph (`text.strong`).
  * **Selected + Disabled (`Filter`):** Drops background fill from `accent.fill` to `BG: surface.raised`, `Border: 1px border.subtle`, `FG: text.disabled`, while keeping the leading `14px` `✓` glyph in `text.strong` (`18.71:1` on `surface.raised`) so the selected state remains unmistakable and `text.disabled` never renders on `accent.fill` (`UIF-TOK-006`).
* **Data Model & Callbacks:**
  ```rust
  pub enum ChipVariant { Assist, Filter { selected: bool }, Input { removable: bool } }
  pub enum ChipHeight { Compact, Bar }

  pub struct ChipModel<Id> {
      pub id: Id,
      pub label: String,
      pub variant: ChipVariant, // Assist | Filter { selected: bool } | Input { removable: bool }
      pub height: ChipHeight,   // Compact (24px) | Bar (28px)
      pub leading_icon: Option<Icon>,
      pub disabled: bool,
  }
  ```
  Callbacks: `on_activate(Commit<Id>)` (`Assist` and `Input` body), `on_toggle(Commit<(Id, bool)>)` (`Filter` via `Space`/`Enter`/click), `on_remove(Commit<Id>)` (trailing `×` click or `Delete`/`Backspace` when focused).

### 3.7 Tooltips & Teaching Popovers (`UIF-ATOM-007`)

* **`Tooltip` (Non-Interactive Hover/Focus Hint):**
  * **Geometry & Visuals:** Max width `TOOLTIP_MAX_W = 280px`; padding `4px 8px` (`space.xs`, `space.sm`); `BG: surface.overlay`; `Border: 1px border.strong` (`4.01:1`); `Radius: radius.widget` (`4px`); text `12px Regular` (`type.caption`, `text.primary` `11.72:1`) with optional trailing shortcut pill (`FontFamily::Mono`, `text.muted` `6.78:1`).
  * **Behavioral Contract:** Opens after `400ms` (`time.delay.tooltip_open`, or `0ms` within the `100ms` `time.delay.tooltip_warm` handoff window) pointer hover delay or `0ms` on keyboard focus arrival, offset `4px` (`space.xs`) from the trigger rect (flipped/clamped into viewport via `geometry::anchor::solve`). Sits visually above all bands (`HitTest::Ignore`), stays outside `SurfaceRegistry`, dismisses on pointer leave or any key press, and **never consumes `Escape`** (passing `Escape` through to the active `Escape Rank` stack).
* **`TeachingPopover` (Interactive Callout / Flyout):**
  * **Geometry & Visuals:** Width clamped to `POPOVER_WIDTH = 280px..360px`; padding `12px` (`space.md`); `BG: surface.overlay`; `Border: 1px accent.border` (`5.94:1`); `Radius: radius.card` (`6px`). Contains title (`13px SemiBold` on `type.ui_compact`, `text.strong` `17.72:1`), body (`13px Regular`, `text.primary` `11.72:1`), an **always-present** focusable `[Dismiss ×]` button (`24×24px`, `DISMISS_LABEL = "Dismiss"`, guaranteeing the focus trap is never empty even when `primary_action` is `None`), and optional primary action button.
  * **Layering & Dismissal:** Registers in `SurfaceRegistry` as `SurfaceKind::TeachingPopover` in `Z-Band 2` (`ZBand::Menu`, `Escape Rank 3`), pushes a nested focus trap (`FocusState::push_trap`), absorbs interior clicks, **never dismisses on pointer leave** (so the cursor can travel across the `4px` gap into the card), and **consumes `Escape`** at Rank 3 (keeping any underlying `Z-Band 3` modal open).
* **Data Model & Callbacks:**
  ```rust
  pub struct PopoverModel<ActionId> {
      pub title: String,
      pub body: String,
      pub primary_action: Option<(String, ActionId)>,
      pub step_indicator: Option<(u16, u16)>,
      pub placement: PreferredSide,
      pub open: bool,
  }
  ```
  Callbacks: `on_action(Commit<ActionId>)`, `on_dismiss(Cancel<()>)`.

### 3.8 Badges & Status Indicators (`UIF-ATOM-008`)

* **Variants & Exact Measured Contrast Polarity (per `UIF-TOK-006`):**
  1. **Dot Badge (`size.indicator.dot = 6×6px`, `radius.pill`):** `accent.border` (unread/active), `status.warning` (unsaved/modified `●`), `status.success` (online/ready), `status.danger.text` (error).
  2. **Numeric Count Pill (`height: 18px`, `min_width: 18px`, padding `[0, 4px]`, `radius.pill`, `12px SemiBold`, `FontFamily::Mono` tabular numerals):** Counts $> 99$ format as `99+`. Solid emphasis uses `BG: accent.fill` or `status.danger.fill` with `FG: text.strong` (`4.80:1` / `5.13:1` per Rule 4 lock); subtle count uses `BG: surface.active` with `FG: text.strong` (`14.29:1`) and `1px border.subtle`.
  3. **Semantic Status Chip (`height: 24px`, padding `[0, 8px]`, `radius.pill`, `12px Medium`):** Pairs a mandatory leading glyph (`✓`, `⚠`, `✕`, `●`) with text on `BG: surface.raised` + `1px` border:
     * `Neutral`: `FG: text.primary` (`12.38:1` on `surface.raised`, `11.72:1` on `surface.overlay`), `Border: 1px border.strong`.
     * `Accent`: `FG/Border: accent.text` (`10.05:1` on `surface.raised`, `9.49:1` on `surface.overlay`).
     * `Success`: `FG/Border: status.success` (`14.28:1` on `surface.raised`, `13.52:1` on `surface.overlay`).
     * `Warning`: `FG/Border: status.warning` (`6.45:1` on `surface.raised`, `6.10:1` on `surface.overlay`).
     * `Danger`: `FG/Border: status.danger.text` (`6.18:1` on `surface.raised`, `5.85:1` on `surface.overlay`).
* **Data Model:** `pub enum BadgeKind { Dot, Count(u32), Status { icon: Icon, label: String } }`, `pub enum BadgeTone { Neutral, Accent, Success, Warning, Danger }`, `pub struct BadgeModel { pub kind: BadgeKind, pub tone: BadgeTone }`.

### 3.9 Progress Indicators: `ProgressBar` & `Spinner` (`UIF-ATOM-009`)

* **`ProgressBar` (Linear):**
  * **Geometry & Pure Injected Motion:** Track height `size.progress.bar_height` (`4px`, `radius.pill`), `BG: surface.active`. Fill bar uses `accent.border` (`4.79:1` vs `surface.active`) when `Active`, `status.warning` (`4.93:1`) when `Paused`, and `status.danger.text` (`4.72:1`) when `Error`. Indeterminate mode takes an injected normalized `phase: f32` and `reduced_motion: bool` parameter (never reading OS globals inside the resolver), animating a `35%`-width sliding segment when `reduced_motion == false` and rendering a static centered/striped fill when `reduced_motion == true`.
* **`Spinner` (Circular Indeterminate / Determinate Ring):**
  * **Geometry & Stroke Token:** Diameter `size.spinner.sm` (`14px`, inline inside buttons/inputs/navbar via `SPINNER_SLOT`) or `size.spinner.md` (`20px`, panel/dialog loading); stroke thickness is pinned to `border.focus` (`SPINNER_STROKE = 2px`); track `border.subtle`, active arc `accent.text` (`9.49:1` on `overlay`) or `text.strong` when inside a solid `Primary`/`Danger` button.
* **Data Model & Callbacks:**
  ```rust
  pub enum ProgressMode { Indeterminate, Determinate { completed: u64, total: u64 } }
  pub enum ProgressStatus { Active, Paused, Error }

  pub struct ProgressModel {
      pub mode: ProgressMode,     // Indeterminate | Determinate { completed: u64, total: u64 }
      pub status: ProgressStatus, // Active | Paused | Error
      pub label: Option<String>,
      pub cancellable: bool,
  }
  ```
  Callbacks: `on_cancel(Cancel<()>)` (via trailing `24×24px` `[×]` stop button when `cancellable == true`).

### 3.10 Date & Calendar Pickers: `DatePicker`, `DateRangePicker` & `CalendarView` (`UIF-ATOM-010`)

* **Style & Ergonomic Rules:**
  * **Segmented Trigger Input (`32px` height):** Displays ISO-8601 `YYYY-MM-DD` (or `YYYY-MM-DD – YYYY-MM-DD` for range) in `FontFamily::Mono` (`14px`, tabular numerals) with individually focusable numeric segments (`Year`, `Month`, `Day`) and a reserved `24×24px` trailing calendar button (`📅`) obeying `UIF-ATOM-002b` (`0px` shift). Typing digits auto-advances to the next segment; `Up`/`Down` increments/decrements the focused segment in place without opening the flyout.
  * **Fixed-Grid Calendar Flyout (`Z-Band 2`, `Escape Rank 3`):** Anchored `4px` below the trigger (`CALENDAR_FLYOUT_W = 256px`, `padding: 12px`, `BG: surface.overlay`, `Border: 1px border.strong`, `Radius: radius.card = 6px`) with a `SurfaceId` derived from the control identity.
    * **Header (`32px`):** Left-aligned `[Month YYYY ▾]` drill-down button (`Ghost` style, toggles `Days -> Months -> Years` grid) and right-aligned `[◀]` `[▶]` (`28×28px`) navigation buttons.
    * **Zero-Shift 6×7 Day Grid (`UIF-PRIN-003`):** Always renders **6 rows × 7 columns** of `32×32px` (`size.calendar.cell`) cells (`224×192px` grid body) plus a `24px` weekday header row (`Mo Tu We Th Fr Sa Su` in `12px SemiBold text.muted`). Leading/trailing days from adjacent months render in `text.disabled` so switching between 28-day and 31-day months never changes card height by even `1px`.
    * **Redundant Visual Cueing (`UIF-PRIN-005`):**
      * **Today:** `1px solid color.accent.border` perimeter outline + bottom `4×2px` accent bar.
      * **Selected Date / Range Endpoints:** `BG: color.accent.fill`, `FG: color.text.strong` (`4.80:1`), `Weight::SemiBold`, adaptive focus ring per `UIF-ATOM-000`.
      * **In-Range Interior Days (`DateRangePicker`):** `BG: color.surface.active`, `FG: color.text.strong` (`14.29:1`), top/bottom `1px color.border.strong`.
      * **Blackout / Out-of-Bounds Dates (`< min_date` or `> max_date`):** `FG: color.text.disabled` with a `1px` diagonal strikethrough line (`non-interactive`, skipped by selection).
* **Controlled Data Model (Reusing `chrono` per `[RUST-022]`):**
  ```rust
  pub struct YearMonth { pub year: i32, pub month: u8 }
  pub enum CalendarViewMode { Days, Months, Years }
  pub enum DateSelection {
      SingleDate(Option<chrono::NaiveDate>),
      DateRange { start: Option<chrono::NaiveDate>, end: Option<chrono::NaiveDate> },
  }

  pub struct DatePickerModel {
      pub selection: DateSelection,
      pub hover_preview_date: Option<chrono::NaiveDate>,
      pub displayed_month: YearMonth,
      pub view_mode: CalendarViewMode, // Days | Months | Years
      pub min_date: Option<chrono::NaiveDate>,
      pub max_date: Option<chrono::NaiveDate>,
      pub blackout_dates: BTreeSet<chrono::NaiveDate>,
      pub first_day_of_week: chrono::Weekday,
      pub open: bool,
      pub validation: Option<String>,
      pub disabled: bool,
  }
  ```
* **Responsive Callbacks & Keyboard Choreography:**
  * `on_open_change(Live<bool>)`, `on_view_mode_change(Live<CalendarViewMode>)`, `on_month_navigate(Live<YearMonth>)`.
  * `on_change(Live<DateSelection>)`: Emitted on inline segment edit or when hovering candidate end-dates during a two-click range selection.
  * `on_commit(Commit<DateSelection>)`: Emitted **once** when a valid day cell (or second range endpoint) is clicked, `Enter` is pressed on a focused cell, or inline segment editing settles on `blur`/`Enter`.
  * `on_cancel(Cancel<DateSelection>)`: `Escape` closes the flyout (`Escape Rank 3`), restores focus to the trigger, and reverts any half-selected range or uncommitted edit.
  * **2D Calendar Keyboard Navigation:** `Left`/`Right` (`±1 day`), `Up`/`Down` (`±7 days`), `PageUp`/`PageDown` (`±1 month`; `Shift+PageUp`/`PageDown` `±1 year`), `Home`/`End` (first/last day of month), `Enter`/`Space` (select focused date).

### 3.11 Time Input: `TimePicker` (`UIF-ATOM-011`)

* **Style & Ergonomic Rules:**
  * **Trigger (`32px` height):** Monospace segmented input (`HH : MM [: SS] [AM/PM]`, `FontFamily::Mono`, `14px`, tabular numerals) with `Up`/`Down` arrow spinning per segment and a trailing `24×24px` clock trigger button (`🕒`).
  * **Drum Flyout (`Z-Band 2`, `Escape Rank 3`):** `TIME_FLYOUT_W = 200px..240px`-wide card (`max_height: 260px`, `BG: surface.overlay`, `Border: 1px border.strong`, `Radius: radius.card = 6px`) containing parallel scrollable `32px`-row columns (`Hour`, `Minute` stepped by `minute_increment`, optional `Second`, optional `AM/PM`) separated by `1px border.subtle` vertical hairlines, with a central `32px` selection band (`BG: surface.active`, `1px accent.border`) and a pinned `36px` `[Cancel]` / `[✓ Apply]` footer bar so the focus trap is never empty.
* **Controlled Data Model & Callbacks (Reusing `chrono` per `[RUST-022]`):**
  ```rust
  pub struct TimePickerModel {
      pub value: Option<chrono::NaiveTime>,
      pub use_24_hour: bool,
      pub show_seconds: bool,
      pub minute_increment: u8, // 1, 5, 10, 15, 30
      pub open: bool,
      pub validation: Option<String>,
      pub disabled: bool,
  }
  ```
  * **Callbacks:** `on_open_change(Live<bool>)`, `on_change(Live<chrono::NaiveTime>)` (column scroll or keyboard spin), `on_commit(Commit<chrono::NaiveTime>)` (`Enter` or `[✓ Apply]`), `on_cancel(Cancel<Option<chrono::NaiveTime>>)` (`Escape` closes flyout and restores pre-open snapshot).

### 3.12 Color Selection: `ColorPicker` & `ColorSwatchButton` (`UIF-ATOM-012`)

* **Style & Ergonomic Rules:**
  * **`ColorSwatchButton` Trigger (`28px` or `32px` height):** Displays a `16×16px` color preview well + hex label (`#RRGGBB` in `FontFamily::Mono`).
  * **Alpha Checkerboard & Border Guarantee:** Every color swatch and preview well renders over a `4×4px` neutral checkerboard (`surface.base` / `surface.active`) enclosed by a mandatory `1px border.strong` inner frame so very dark colors (near `surface.base`) remain visually bounded (`UIF-TOK-006`).
  * **`ColorPicker` Surface (`Z-Band 2`, `Escape Rank 3`, `COLOR_FLYOUT_W = 240px`, padding `12px`):** Contains a `180×140px` (`size.color.well`) 2D Saturation/Value (`SV`) canvas with a `16×16px` (`size.slider.thumb`) crosshair thumb (`2px text.strong` + `1px surface.base` dual ring for contrast over any hue), a `4px`-tall Hue `Slider` and optional Alpha `Slider` (`UIF-ATOM-005`), a validated `#RRGGBB[AA]` `TextInput` (`UIF-ATOM-002`), and a grid of `24×24px` preset theme swatches.
  * **Built-In Accessibility Contrast Readout:** When `contrast_reference: Some(bg)` is provided in the model, the picker footer renders a live `Semantic Status Chip` (`UIF-ATOM-008`) showing the computed WCAG 2.1 contrast ratio (`✓ 4.80:1 AA` in `status.success` or `⚠ 2.40:1 Fail` in `status.warning`/`status.danger.text`).
* **Controlled Data Model & Callbacks:**
  ```rust
  pub struct Rgba8 { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }
  pub struct Hsva { pub h: f32, pub s: f32, pub v: f32, pub a: f32 }

  pub struct ColorPickerModel {
      pub value: Rgba8,
      pub hsv: Hsva,
      pub hex_input: String,
      pub alpha_enabled: bool,
      pub preset_swatches: Vec<Rgba8>,
      pub contrast_reference: Option<Rgba8>,
      pub open: bool,
      pub disabled: bool,
  }
  ```
  * **Callbacks:** `on_change(Live<Rgba8>)` (2D SV plane or Hue/Alpha slider drag), `on_commit(Commit<Rgba8>)` (pointer release, swatch click, or valid `#RRGGBB[AA]` `Enter`), `on_cancel(Cancel<Rgba8>)` (`Escape` during scrub or flyout open reverts to pre-open color).

### 3.13 Identity & Feedback Atoms: `Avatar` / `PersonPicture` & `RatingControl` (`UIF-ATOM-013`)

* **`Avatar` / `PersonPicture`:**
  * **Geometry & Visuals:** Circular (`radius.pill`) or rounded-square (`radius.widget`) container in `24×24px` (`size.avatar.sm`, dense rows), `32×32px` (`size.avatar.md`, navbar/comments), or `36×36px` (`size.avatar.lg`, profile/agent headers) with `1px border.strong` perimeter.
  * **Deterministic Fallback Cascade:** `Image` bitmap $\rightarrow$ `1–2 Uppercase Initials` (`12px`/`13px SemiBold`, `text.strong` on `accent.fill` or `surface.active` clearing `4.50:1`) $\rightarrow$ generic `Person` icon (`size.icon.md`, `text.primary`).
  * **Presence Cut-Out:** Optional bottom-right `6×6px` (`size.indicator.dot`) status badge (`UIF-ATOM-008`: `status.success` online, `status.warning` busy, `status.danger.text` error, `text.disabled` offline) with a `1px surface.base` cutout ring.
  * **Data Model:**
    ```rust
    pub enum AvatarSize { Sm, Md, Lg }

    pub struct AvatarModel {
        pub display_name: String,
        pub initials: String,
        pub image_uri: Option<String>,
        pub presence: Option<BadgeTone>,
        pub size: AvatarSize, // Sm (24px) | Md (32px) | Lg (36px)
    }
    ```
* **`RatingControl` (1..5 Star / Score Selector):**
  * **Geometry & Visuals:** Row of `max` (`5` default) `24×24px` (`size.hit_target.min`) star slots (`space.xs = 4px` gap). Filled stars (`★`) use `color.status.warning` (`6.86:1` on `base`, `6.10:1` on `overlay`) + `text.strong` outline on hover; empty stars (`☆`) use `color.border.strong` (`4.01:1`–`4.51:1`).
  * **Controlled Data Model & Callbacks:**
    ```rust
    pub enum RatingStep { Half, Whole }

    pub struct RatingModel {
        pub value: Option<f32>,
        pub hover_preview: Option<f32>,
        pub max: u8,
        pub step: RatingStep, // Half | Whole
        pub allow_clear: bool,
        pub read_only: bool,
        pub disabled: bool,
    }
    ```
  * **Callbacks:** `on_hover_preview(Live<Option<f32>>)` (pointer hover across stars shows temporary preview fill), `on_commit(Commit<Option<f32>>)` (click or `Left`/`Right` arrow keys stepping by `0.5` or `1.0`; clicking the current rating when `allow_clear == true` or pressing `Delete`/`0` commits `None`), `on_cancel(Cancel<Option<f32>>)`.

---

## 3B. Reusable Composite Widget Patterns (`UIF-COMP-*`)

Composite widgets combine atomic controls (`§3`) into standardized layout, data-binding, and keyboard-navigation patterns shared across the nine application surfaces (`§4`).

### 3B.1 `FormField` & Settings Row Pattern (`UIF-COMP-001`)

* **Stacked Form Layout (Dialogs & Forms):**
  * **Pure Structural Band Solver (`bands(bounds)`):** The vertical layout of a `FormField` is a pure function of its outer `bounds` alone—taking no `helper_text`, `error_text`, or `valid` parameter—so validation transitions can never shift the control or adjacent rows (`0px` layout shift per `UIF-PRIN-003`):
    * Header band (`height: 20px`, `type.body.line_height`): Field label (`13px Medium` on `type.ui_compact`, `text.primary`, sentence case) + required indicator (`*` in `status.danger.text` with tooltip `"Required"` when `required == true`) + optional right-aligned helper hint (`12px Regular`, `text.muted`).
    * Gap `space.xs` (`4px`) $\rightarrow$ Control slot (`height: 32px`, `size.control.md`) $\rightarrow$ Gap `space.xs` (`4px`) $\rightarrow$ Permanently reserved caption band (`height: 16px`, `type.caption.line_height`: renders `helper_text` in `text.muted` when valid, or `error_text` in `status.danger.text` with leading `14px` `⚠` glyph when invalid). Total stacked height is `20 + 4 + 32 + 4 + 16 = 76px`.
    * **Single-Line Caption Truncation:** The `16px` caption band is strictly single-line: overflow end-truncates with `…` and automatically registers a hover/focus `Tooltip` (`UIF-PRIN-007`) exposing the full message, rather than wrapping to a second line.
* **Horizontal Settings Card Row (`SettingsRow`):**
  * Single-line floor height is `48px` (`space.md = 12px` top padding + `20px` `type.body` title + `12px` bottom padding + `4px` border/inset reserve); when `description.is_some()`, the two-line height is `60px` (`12px` top + `20px` title + `16px` `type.caption` description + `12px` bottom + `0px` slack). Horizontal padding is `space.lg` (`16px`), `BG: surface.raised`, `Border: 1px border.subtle`, `Radius: radius.widget` (`4px`). Left column (`Fill`): title (`14px Medium`, `text.strong`) + optional description (`12px Regular`, `text.muted`). Right column (`Shrink`, min horizontal gap `space.lg = 16px`): interactive control (`ToggleSwitch`, `ComboBox`, `NumberBox`, or `Button`).
* **Controlled Data Model:**
  ```rust
  pub struct FieldId(pub &'static str);
  pub enum FormLayout { Stacked, HorizontalSplit }

  pub struct FormFieldModel<ControlModel> {
      pub id: FieldId,
      pub label: String,
      pub required: bool,
      pub helper_text: Option<String>,
      pub error_text: Option<String>,
      pub layout: FormLayout, // Stacked | HorizontalSplit
      pub control: ControlModel,
  }
  ```

### 3B.2 `InfoBar` (Inline Banner) & `Toast` / `SnackBar` (Transient Notification) (`UIF-COMP-002`)

* **Geometry, Surface Selection & Visuals:**
  * **`InfoBar` (Docked Inline inside Modals or Panes):** Full container width, `min_height: 36px` (`size.control.lg`), padding `8px 12px` (`space.sm`, `space.md`), `Radius: radius.widget` (`4px`), `1px` perimeter border in `border.strong`, plus a `4px` (`space.xs`) solid left accent stripe and `16px` (`size.icon.md`) leading icon colored by severity (`accent.text` for `Info`, `status.success` for `Success`, `status.warning` for `Warning`, `status.danger.text` for `Danger`). Background fill is explicitly selected by `BarSurface`: `BarSurface::Raised` (`surface.raised`) when docked inside a `Z-Band 0` workspace pane, and `BarSurface::Overlay` (`surface.overlay`) when docked inside a `Z-Band 3` modal dialog.
  * **`Toast` / `SnackBar` (Floating in `Z-Band 1`):** Width `size.toast.width = 360px` (clamped to `viewport.width - 32px`), `BG: surface.overlay`, `Border: 1px border.strong`, `Radius: radius.card` (`6px`), anchored bottom-right (`16px` from right edge, `16px` above Bottom Command Bar), stacking upward with `8px` (`space.sm`) gap (max `3` visible). Dismissing a toast re-anchors surviving toasts within `Z-Band 1` without shifting any `Z-Band 0` workspace layout (`0px` layout shift).
* **Usability, Banking Timer & Overflow Eviction Contract:**
  * **Severity-Enforced Persistence:** `effective_auto_dismiss_ms` forces `None` for `Warning` and `Danger` notifications regardless of caller input—they require explicit user acknowledgment (`[Dismiss ×]` or action button). `Info` and `Success` toasts default to `Some(5000)` (`time.toast.auto_dismiss`).
  * **Dual-Hold Timer Banking:** Countdown advancement is driven purely by injected `TimeMs`. While `hovered || focused` holds, the countdown pauses and **banks** its remaining `TimeMs` balance (so hovering after `4000ms` of a `5000ms` toast leaves `1000ms` upon pointer leave/blur rather than resetting to `5000ms` or expiring while read).
  * **Deterministic Overflow Eviction (`Max 3 Visible`):** When a 4th toast arrives while 3 are visible, the queue evicts the oldest visible auto-dismissible (`Info`/`Success`) toast so the newcomer takes the anchor immediately; if all 3 visible slots are held by persistent `Warning`/`Danger` toasts, none is evicted and the newcomer queues FIFO, starting its `auto_dismiss_ms` countdown only on the frame it becomes visible.
  * **Keyboard & Pointer Routing (`Z-Band 1`, `Escape Rank 1`):** Clicks outside toast bounds pass through to `Z-Band 0`; `Escape` dismisses the active toast **if and only if** keyboard focus currently resides inside the toast stack.
* **Controlled Data Model & Callbacks:**
  ```rust
  pub struct NotificationId(pub u64);
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum NotificationSeverity { Info, Success, Warning, Danger }
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum BarSurface { Raised, Overlay }
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum DismissReason { UserClick, EscapeKey, Timeout, OverflowEvicted }

  pub struct NotificationModel<ActionId> {
      pub id: NotificationId,
      pub severity: NotificationSeverity,
      pub title: String,
      pub message: String,
      pub action: Option<(String, ActionId)>,
      pub dismissible: bool,
      pub auto_dismiss_ms: Option<u32>,
  }
  ```
  Callbacks: `on_action(Commit<(NotificationId, ActionId)>)`, `on_dismiss(Commit<(NotificationId, DismissReason)>)` (uses `Commit` per `§3.0.4` because notification dismissal is a settled lifecycle transition that removes the item from the active queue and records acknowledgment state).

### 3B.3 `Expander` / Collapsible Disclosure Container (`UIF-COMP-003`)

* **Anatomy & Zero-Jitter Disclosure:**
  * **Header Bar:** Height `32px` (`size.control.md`, standard) or `28px` (`size.control.sm`, compact inside tree/JSON views), padding `[0, 12px]` (`space.md`), `Radius: radius.widget` (`4px` on all four corners when collapsed; `4px` top corners when expanded), `Border: 1px border.subtle`. Background is `surface.raised` at `Rest`, `surface.hover` when `Hovered`, and `surface.active` when `Pressed`. Contains a `16×16px` (`size.icon.md`) leading chevron (`▸` collapsed / `▾` expanded), title (`13px SemiBold` on `type.ui_compact`, `text.primary` at rest / `text.strong` on hover), optional summary `BadgeModel`, and optional disjoint trailing action button (`24×24px` hit target separated by $\ge 2\text{px}$ from the disclosure header hit zone).
  * **Invariant Header Geometry & Instant Body Mount:** Header bounds resolve from `(row_width, density_rung, has_trailing_action)` alone and never shift when the body mounts. Toggling `expanded` settles through a `GestureSlot<bool>` (`time.instant = 0ms`, zero per-frame height interpolation; only the chevron angle is interpolable).
  * **Body Surface & Focus Exclusion:** When `expanded == true`, the body container (`ExpanderBodySurface::Base` = `surface.base` by default for standard forms/settings, or `ExpanderBodySurface::Sunken` = `surface.sunken` for recessed code/JSON/log wells and nested inspector trees; padding `space.md = 12px`; `1px border.subtle` horizontal top divider shared with the header plus `1px border.subtle` left/right/bottom perimeter with `4px` bottom corners) mounts immediately below the header. When `expanded == false`, all controls inside the body register as disabled focus members so `FocusCycle` (`Tab`) never lands inside a collapsed expander.
  * **Sibling Coordination (`AccordionPolicy`):** A standalone `ExpanderModel` manages a single section without requiring a redundant section ID. Groups of sibling expanders coordinate through `AccordionModel<Id>` with `AccordionPolicy::Independent` (any number open simultaneously) or `AccordionPolicy::SingleOpen` (opening section `id` automatically collapses any other open sibling in the group).
* **Controlled Data Model & Callbacks:**
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum AccordionPolicy { Independent, SingleOpen }
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
  pub enum ExpanderBodySurface { #[default] Base, Sunken }

  pub struct ExpanderModel {
      pub title: String,
      pub badge: Option<BadgeModel>,
      pub body_surface: ExpanderBodySurface,
      pub expanded: bool,
      pub disabled: bool,
  }

  pub struct AccordionSection<Id> {
      pub id: Id,
      pub expander: ExpanderModel,
  }

  pub struct AccordionModel<Id> {
      pub sections: Vec<AccordionSection<Id>>,
      pub policy: AccordionPolicy,
  }
  ```
  Callbacks:
  * **Standalone `ExpanderModel`:** `on_toggle(Commit<bool>)` (`Enter`/`Space` or pointer click release on header toggles; `Right` arrow expands; `Left` arrow collapses).
  * **Coordinated `AccordionModel<Id>`:** `on_section_toggle(Commit<(Id, bool)>)`.

### 3B.4 `ScrollView` & Annotated Scrollbar Contract (`UIF-COMP-004`)

* **Geometry, Zero-Shift Gutter & Contrast:**
  * **Permanently Reserved `10px` Gutter (`0px` Content Reflow):** On every scrollable axis (vertical and/or horizontal), `content_bounds` permanently deducts the full `10px` (`size.splitter.grab`) scrollbar gutter—taking no track-phase parameter—so transitioning between `Rest` (`8px` visual thumb width inset by `2px`) and `Expanded`/`Dragging` (`10px` visual thumb width expanding inward) causes **`0px` content reflow** (`UIF-PRIN-003`). When both vertical and horizontal scrollbars are active, the bottom-right `10×10px` intersection is reserved as an unpainted corner box.
  * **Pure Phase & Thumb Contrast:** Scrollbar phase (`Hidden`, `Rest`, `Expanded`, `Dragging`) is a pure function of `(is_scrollable, pointer_in_gutter, is_dragging)` with no implicit wall-clock timer inside the solver. The track itself paints no fill (`transparent`), while the thumb (`radius.pill`, minimum length `size.hit_target.min = 24px` even at extreme content ratios) is graded `Judged`: `border.strong` at `Rest`/`Expanded` (`4.51:1` on `base`, `4.24:1` on `raised`, `4.01:1` on `overlay`, `3.68:1` on `hover`, `3.24:1` on `active`) and `accent.border` (`4.79:1`–`6.67:1`) while `Dragging`.
  * **Track Tick Annotations:** Optional `2px` (`space.2xs`) tick marks along the vertical gutter mark search hits (`accent.border`), headings (`text.muted`), or diagnostic errors (`status.danger.text`).
  * **Keyboard Step Distances & Nested Scroll Bubbling:** When the scroll viewport holds keyboard focus, `Up`/`Down` (and `Left`/`Right` horizontally) step by `size.hit_target.min = 24px` (1 compact tree row); `PageUp`/`PageDown` step by `(viewport_length - 24.0).max(24.0)`; `Home` jumps to `0.0`; `End` jumps to `max_offset`. In nested `ScrollView` hierarchies, wheel/pan deltas are consumed by the innermost hovered `ScrollView` that has remaining travel in the scroll direction, and bubble to the parent `ScrollView` only when the inner view is pinned at its boundary.
* **Controlled Data Model & Callbacks:**
  ```rust
  pub enum TickTone { SearchHit, Heading, DiagnosticError }
  pub struct ScrollTick { pub ratio: f32, pub tone: TickTone }
  pub struct ScrollMetrics { pub offset: Point, pub content_size: Size, pub viewport_size: Size, pub at_bottom_within_2px: bool }

  pub struct ScrollViewModel {
      pub content_size: Size,
      pub viewport_size: Size,
      pub offset: Point,
      pub follow_tail: bool,
      pub annotations: Vec<ScrollTick>,
  }
  ```
  * **`on_scroll(Live<ScrollMetrics>)`:** Emitted on wheel, pan, thumb scrub, or keyboard scroll, carrying `at_bottom_within_2px: bool` (`(content_size.height - viewport_size.height - offset.y) <= 2.0`) for `UIF-MOV-004` stream auto-scroll.
  * **`on_scroll_settled(Commit<ScrollMetrics>)` / `on_scroll_cancel(Cancel<ScrollMetrics>)`:** Thumb drag settles through `GestureSlot` (`Escape` during thumb drag restores pre-drag offset).
  * **`on_reach_end(Commit<()>)`:** Emitted once per approach when remaining bottom scroll distance enters the final `64px` (`2 × space.2xl`) of content.

### 3B.5 `Card` & `ListTile` (`UIF-COMP-005`)

* **`Card` Container:** Grouping surface (`BG: surface.raised`, `Border: 1px border.subtle` at rest or `1px border.strong` when interactive/elevated, `Radius: radius.card = 6px`, inner padding `space.lg = 16px`).
* **`ListTile` (3-Slot Row & Exact Two-Line Arithmetic):**
  * **Single-Line Row (`height: 32px`, `size.control.md`)** or **Two-Line Row (`height: 44px` authoritative total = `4px` `space.xs` top padding + `18px` `type.ui_compact` title + `2px` `space.2xs` inter-line gap + `16px` `type.caption` subtitle + `4px` `space.xs` bottom padding = `44px`)**, horizontal padding `space.md = 12px`, inter-slot gap `space.sm = 8px`:
    1. **Leading Slot (`16×16px` or `24×24px`, expanded to `24×24px` hit extent on interactive tiles):** Icon, `Avatar`, `Checkbox`, or `Radio`.
    2. **Center Slot (`Fill`):** Primary title (`13px Regular`/`Medium`, `text.primary`, single-line truncated with `…`) + optional subtitle (`12px Regular`, `text.muted` at rest).
    3. **Trailing Slot (`Shrink`):** Metadata badge, shortcut hint, or disjoint secondary action icon buttons (`24×24px` hit targets separated by $\ge 2\text{px}$ from the primary row hit zone).
  * **Selected State Contrast & Border Omission (`UIF-TOK-006` Rule 4):** When `selected == true`, the tile paints `BG: accent.fill`, promotes **both** title and subtitle to `FG: text.strong` (`4.80:1`, since `text.muted` on `accent.fill` is `1.84:1`), and wears **no perimeter border stroke** (since `accent.border` on `accent.fill` is `1.61:1`; keyboard focus uses the `1px text.strong` inset outline at `4.80:1`).
* **Controlled Data Model & Callbacks:**
  ```rust
  pub enum LeadingSlot { Icon(Icon), Avatar(AvatarModel), Checkbox(CheckState), Radio(bool) }
  pub enum TrailingSlot { Badge(BadgeModel), Shortcut(String), Actions(Vec<Icon>) }

  pub struct ListTileModel<Id> {
      pub id: Id,
      pub title: String,
      pub subtitle: Option<String>,
      pub leading: Option<LeadingSlot>,
      pub trailing: Option<TrailingSlot>,
      pub selected: bool,
      pub disabled: bool,
  }
  ```
  Callbacks: `on_select(Commit<Id>)`, `on_activate(Commit<Id>)`, `on_trailing_action(Commit<Id>)`, `on_context_menu(Commit<(Id, Point)>)`.

### 3B.6 `VirtualList` & `TreeView` (`UIF-COMP-006`)

* **Geometry, Width-Relative Indentation Compression & Contrast:**
  * **Fixed Row Rungs & `f64` Closed-Form Virtualization:** Row height is selected from two fixed rungs—`24px` (`Compact`, `size.hit_target.min`) or `28px` (`Standard`, `size.control.sm`)—with horizontal row inset `space.xs = 4px`. Every row's vertical offset is computed as `(index as f64) * (row_height as f64)` narrowed once to `f32`, eliminating float prefix-sum drift across `100,000+` rows.
  * **Width-Relative Indentation & Mandatory `96px` Label Floor (`TREE_LABEL_MIN_W = 96`):** Deducting `32px` of fixed horizontal row furniture (`2 × 4px` `space.xs` row insets + `24px` disclosure chevron slot) and the mandatory `96px` (`TREE_LABEL_MIN_W`) label floor leaves a maximum total indentation budget of `indent_budget = (row_width - 32px - 96px).max(0px) = (row_width - 128px).max(0px)`. Nominal indentation is `16px` (`space.lg`) per depth level. When `depth × 16px > indent_budget` (equivalently, `row_width - 32px - depth × 16px < 96px`), per-level guide spacing compresses proportionally across all active levels down to a minimum step of `4px` (`space.xs`). Once the `4px` floor step is reached and `depth × 4px > indent_budget`, deeper levels collapse onto the innermost drawn guide line at `indent_budget` (reporting `collapsed_levels: u16` on the layout result) so the label box **never narrows below `96px`** at any tree depth (`20` or `200`).
  * **Disjoint Chevron & `Judged` Guide Contrast:** Expandable parent rows reserve a disjoint `24×24px` leading disclosure chevron (`▸` / `▾`). Vertical `1px` tree depth guides are graded `Judged` and use `border.strong` on unselected rows (`4.51:1` on `base`, `4.24:1` on `raised`, `3.68:1` on `hover`). On selected rows (`BG: accent.fill`), the label, disclosure chevron, depth guides, and `2px` (`border.focus`) left selection bar all lock to `color.text.strong` (`4.80:1` per `UIF-TOK-006` Rule 4).
* **Clamped Keyboard Navigation & ID-Keyed Anchor Semantics:**
  * Arrow navigation (`Up`/`Down`, `PageUp`/`PageDown`, `Home`/`End`) **clamps** at index `0` and `visible_nodes.len() - 1` (never wrapping). `Ctrl+Up` / `Ctrl+Down` moves `focused_id` without altering `selected_ids`; `Space` toggles the focused row in `ExtendedMulti`.
  * The range-selection anchor (`anchor_id: Option<Id>`) is a stable node `Id`, never a row index: if the anchor node is collapsed out of view, range selection resolves from its nearest visible ancestor; if the anchor node was deleted, `Shift` selection degenerates cleanly to single selection on the target row; if `focused_id` is collapsed out of view, focus falls back slot-preservingly to the collapsed ancestor. Auto-scroll to `focused_id` returns `None` while a scrollbar thumb drag is open.
* **Controlled Data Model & Callbacks:**
  ```rust
  pub enum TreeSelectionMode { Single, ExtendedMulti }

  pub struct TreeNodeModel<Id> {
      pub id: Id,
      pub parent_id: Option<Id>,
      pub depth: u16,
      pub label: String,
      pub icon: Icon,
      pub expandable: bool,
      pub expanded: bool,
      pub special_warning: bool,
      pub disabled: bool,
  }

  pub struct TreeViewModel<Id> {
      pub visible_nodes: Vec<TreeNodeModel<Id>>,
      pub selected_ids: BTreeSet<Id>,
      pub anchor_id: Option<Id>,
      pub focused_id: Option<Id>,
      pub selection_mode: TreeSelectionMode, // Single | ExtendedMulti
      pub filter_query: String,
  }
  ```
  * **`on_selection_change(Commit<BTreeSet<Id>>)`:** Click selects single; `Ctrl+Click` or `Space` toggles in `ExtendedMulti`; `Shift+Click` or `Shift+Up`/`Shift+Down` selects contiguous range from resolved `anchor_id`.
  * **`on_activate(Commit<Id>)`:** Double-click or `Enter` on focused row.
  * **`on_expand_toggle(Commit<(Id, bool)>)`:** Chevron click, `Right` arrow (expands collapsed parent, or moves focus to first child if already expanded), `Left` arrow (collapses expanded parent, or moves focus to parent node if already collapsed/leaf).
  * **`on_context_menu(Commit<(BTreeSet<Id>, Point)>)`:** Right-click or `Shift+F10` / `ContextMenu` key (resets selection to the target row first if outside `selected_ids`; preserves `selected_ids` if already inside).

### 3B.7 `DataTable` / Structured Grid (`UIF-COMP-007`)

* **Geometry, Disjoint Header Targets & Grid-Line Promotion:**
  * **Sticky Header Band (`height: 32px` / `size.control.md`, `BG: surface.raised`, bottom border `1px border.strong`):** Never enters virtual row recycling and remains column-aligned across horizontal scroll offsets. Header titles render in `CAPTION` (`12px`) + `Weight::SemiBold`, `FG: text.strong` (`18.71:1`).
  * **Disjoint Sort Target vs. `10px` Resize Handle (`UIF-PRIN-008`):** Each resizable internal column border exposes a `10px` (`size.splitter.grab`, `±5px` half-grab) resize handle. A header cell's sort click target is inset by `5px` on every edge adjacent to a live resize handle, and its `14px` (`size.icon.sm`, `space.xs = 4px` gap) sort arrow (`▲` / `▼` in `accent.text`) is pinned inside the sort target's trailing edge so it never paints inside the `5px` grab band. `ColumnSizing::Fixed` columns and the terminal rightmost column edge expose no resize handle.
  * **Three-Regime Pure Column Width Solver (`ColumnSizing`):** Columns declare `Fixed(f32)`, `Content { min, preferred }` (where `max == preferred` so it shrinks under deficit but never grows past content), or `Flex { min, preferred, max, weight }`. Given `available_width`:
    1. *Surplus (`available_width >= sum(preferred)`):* Water-fills surplus across `Flex` columns by `weight` clamped by `max` (terminating in at most one pass per column).
    2. *Compressible Deficit (`sum(min) <= available_width < sum(preferred)`):* Scales each column in closed form between `preferred` and `min` proportionally to its slack `(preferred - min)`.
    3. *Overflow Floor (`available_width < sum(min)`):* Pins every column at `min` and sets `needs_horizontal_scroll = true`.
  * **Body Rows (`height: 28px` compact or `32px` standard, cell padding `[space.xs (4px), space.md (12px)]`):** Band precedence is `Selected > Hovered > Zebra/Base`. Resting rows use `surface.base` / `surface.raised` with `FG: text.primary`; `Hovered` rows use `surface.hover` with `FG: text.strong`; `Selected` rows use `BG: surface.active`, `FG: text.strong` (`14.29:1`), and `Border: 1px accent.border` (`4.79:1`). Internal `1px` cell grid lines use `border.subtle` (`RestingDivider`, `1.35:1` on `base`, `1.27:1` on `raised`) and automatically promote to `border.strong` (`3.68:1` on `hover`, `3.24:1` on `active`) whenever the resting hairline contrast against the row band drops below the `1.15:1` visibility floor (`1.10:1` on `hover`, `1.03:1` on `active`).
  * **Stable Sort Cycle & Anchor Selection:** Header activation cycles `None -> Ascending -> Descending -> None`. `Descending` reverses the primary comparator while tie-breaking by ascending original row index (`index_a.cmp(&index_b)`). Selection and `anchor_id: Option<RowId>` reuse `TreeView`'s ID-anchored semantics (`§3B.6`), and arrow navigation clamps at table boundaries.
* **Controlled Data Model & Callbacks:**
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum SortDirection { Ascending, Descending }
  pub enum ColumnSizing { Fixed(f32), Content { min: f32, preferred: f32 }, Flex { min: f32, preferred: f32, max: f32, weight: f32 } }
  pub struct ColumnDef<ColId> { pub id: ColId, pub title: String, pub sizing: ColumnSizing, pub sortable: bool, pub numeric: bool }
  pub struct TableRow<RowId> { pub id: RowId, pub cells: Vec<String>, pub disabled: bool }
  pub enum CellAction { Activate, Edit(String), Custom(&'static str) }

  pub struct DataTableModel<RowId, ColId> {
      pub columns: Vec<ColumnDef<ColId>>,
      pub rows: Vec<TableRow<RowId>>,
      pub sort: Option<(ColId, SortDirection)>,
      pub selected_rows: BTreeSet<RowId>,
      pub anchor_id: Option<RowId>,
      pub focused_cell: Option<(RowId, ColId)>,
  }
  ```
  Callbacks: `on_sort_change(Commit<(ColId, Option<SortDirection>)>)`, `on_column_resize_change(Live<(ColId, f32)>)`, `on_column_resize_commit(Commit<(ColId, f32)>)`, `on_column_resize_cancel(Cancel<(ColId, f32)>)`, `on_row_select(Commit<BTreeSet<RowId>>)`, `on_cell_action(Commit<(RowId, ColId, CellAction)>)`.

### 3B.8 `BreadcrumbBar` & Responsive Overflow Toolbar (`UIF-COMP-008`)

* **`BreadcrumbBar`:**
  * **Geometry & Monotone Nearest-Root Elision:** Horizontal trail (`height: 28px` / `size.control.sm`, gap `space.xs = 4px`, separator `/` or `›` rendered in `text.muted` at `7.61:1` on `surface.base` and audited as a text glyph). Each segment is a `Ghost` button (`height: 24px` / `size.bar.status`, padding `[0, 6px]`, `13px Regular text.muted` for ancestors, `13px SemiBold text.strong` for the terminal current page, and `13px Regular text.primary` on the `[…]` trigger). Both ancestor segments and the terminal segment are focusable and clickable, emitting `BreadcrumbRole::{Ancestor, CurrentPage}` so the host controls ARIA `aria-current="page"` vs refresh behavior.
  * **Deterministic Collapse Order:** `collapse(items, available_width)` always keeps the root (`index 0`) and terminal (`index len - 1`) inline while eliding middle ancestors **nearest-root first** (`index 1`, then `1..=2`, etc.) into a single `[…]` (`28×24px`) trigger button (`Z-Band 2`, `Escape Rank 3`). If even `root + […] + terminal` exceeds `available_width`, `root` joins the `[…]` menu leaving `[…] + terminal`. Because the hidden set is a contiguous prefix of the middle run, widening the container un-hides items monotonically.
  * **Data Model & Callbacks:**
    ```rust
    pub enum BreadcrumbRole { Ancestor, CurrentPage }
    pub struct BreadcrumbSegment<Id> { pub id: Id, pub label: String, pub measure: f32 }

    pub struct BreadcrumbModel<Id> {
        pub segments: Vec<BreadcrumbSegment<Id>>,
        pub elided_popup_open: bool,
    }
    ```
    Callbacks: `on_segment_activate(Commit<(Id, BreadcrumbRole)>)`, `on_overflow_toggle(Live<bool>)`.
* **`OverflowToolbar` (`CommandBar` Pattern):**
  * **Geometry & Zero-Shift Height:** Single-row strip (`strip_height() = 36px` / `size.control.lg` constant, `0px` vertical shift across any eviction count).
  * **Monotone Priority Eviction (`OverflowRank`):** Each toolbar item declares an explicit `OverflowRank(u8)` (`0` = pinned primary action; lower rank numbers evict first, with ties broken **rightmost-first** by layout index). The `36px` (`28×28px` `[⋯]` trigger + `8px` `space.sm` gap) trigger reserve is charged **only** once the full strip fails to fit in `available_width`.
  * **Focus & Busy Semantics in Overflow:** The `[⋯]` trigger is a normal `Tab` stop (`Z-Band 2`, `Escape Rank 3`) whose `Escape` closes the overflow menu and returns focus to the `[⋯]` trigger itself. An overflowed item with `busy == true` remains enabled and focusable in the overflow menu (retaining enabled text color and focus stop so in-flight actions stay keyboard-reachable, while `ActivationGate` prevents re-firing).
  * **Data Model & Callbacks:**
    ```rust
    pub struct OverflowRank(pub u8);
    pub struct ToolbarItem<ActionId> { pub id: ActionId, pub label: String, pub measure: f32, pub rank: OverflowRank, pub enabled: bool, pub pending: ActionPending }

    pub struct OverflowToolbarModel<ActionId> {
        pub items: Vec<ToolbarItem<ActionId>>,
        pub overflow_open: bool,
    }
    ```
    Callbacks: `on_action(Commit<ActionId>)`, `on_overflow_toggle(Live<bool>)`.

### 3B.9 `NavigationRail` / Collapsible `NavigationView` (`UIF-COMP-009`)

* **Anatomy & Responsive Modes:** Vertical primary destination switcher (`BG: surface.raised`, right border `1px border.subtle`) supporting `Compact` (`width: 48px` / `size.nav_rail.compact`, icon-only `36×36px` cells with mandatory right-aligned hover `Tooltip` using the `100ms` `time.delay.tooltip_warm` handoff window) and `Expanded` (`width: 220px` / `size.nav_rail.expanded`, resizable `[180px, 320px]`, `36px`-high rows with `16px` leading icon, `13px Medium` label, and optional trailing `BadgeModel` count pill). Selected destination renders `BG: surface.active`, `FG: text.strong` (`14.29:1`), and a `2px accent.border` (`4.79:1`) leading vertical indicator bar. Includes a top collapse/expand toggle (`28×28px`), scrollable middle destination list, and a `1px border.subtle` divider above pinned bottom utility destinations (`Settings`, `Help`).
* **Controlled Data Model & Callbacks:**
  ```rust
  pub struct NavDestination<DestId> { pub id: DestId, pub label: String, pub icon: Icon, pub badge: Option<BadgeModel>, pub disabled: bool }

  pub struct NavigationRailModel<DestId> {
      pub top_items: Vec<NavDestination<DestId>>,
      pub bottom_items: Vec<NavDestination<DestId>>,
      pub selected: DestId,
      pub expanded: bool,
      pub expanded_width: f32,
  }
  ```
  Callbacks: `on_select(Commit<DestId>)` (`Up`/`Down`/`Home`/`End` roves and wraps focus across enabled destinations as a single `Tab` stop; `Enter`/`Space` activates), `on_expand_toggle(Commit<bool>)`, `on_resize_change(Live<f32>)`, `on_resize_commit(Commit<f32>)`, `on_resize_cancel(Cancel<f32>)`.

### 3B.10 `CommandPalette` / Modal `SearchView` (`UIF-COMP-010`)

* **Anatomy:** Keyboard-first quick-launcher and omnibox (`Ctrl+K` / `Ctrl+P`) mounted in `Z-Band 3` (`Escape Rank 2`) over `surface.scrim`, top-centered at a fixed `top: 64px` offset below the window top (`size.palette.card = 560×380px`, clamped to `viewport.width - 32px`, `BG: surface.overlay`, `Border: 1px border.strong`, `Radius: radius.card = 6px`) so changing result counts never shifts the search input vertically (`0px` input shift).
* **Internal Slots & Prefix-Mode Contract:** Top `40px` search input (`14px` text, leading `16px` `🔍` icon, removable mode prefix chip `>` for `Commands` / `#` for `Tags` / `@` for `Symbols` where `Backspace` at caret `0` resets to `Files`, trailing `Esc` hint chip) + `1px border.subtle` divider + virtualized grouped result list (`32px` single-line or `44px` two-line rows with matched-character spans in `accent.text` `9.49:1` + `Weight::SemiBold`, secondary path/subtitle promoted to `text.strong` on the active `accent.fill` row per `UIF-TOK-006` Rule 4, and right-aligned `FontFamily::Mono` shortcut pills) + `24px` (`size.bar.status`) keyboard legend footer (`↑↓ Navigate · ↵ Run · Esc Close`).
* **Two-Stage `Escape` & Generation Guard:** First `Escape` when `query` is non-empty clears `query` to `""`; `Escape` when `query` is empty (or scrim click) emits `on_dismiss(Cancel<()>)`. Async queries debounce at `time.debounce.live = 200ms` with a monotonic `generation: u64` counter.
* **Controlled Data Model & Callbacks:**
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum PaletteMode { Files, Commands, Symbols, Tags }
  pub struct PaletteItem<ItemId> { pub id: ItemId, pub title: String, pub subtitle: Option<String>, pub match_spans: Vec<TextRange>, pub shortcut: Option<String> }
  pub struct PaletteGroup<ItemId> { pub heading: String, pub items: Vec<PaletteItem<ItemId>> }

  pub struct CommandPaletteModel<ItemId> {
      pub query: String,
      pub mode: PaletteMode,
      pub groups: Vec<PaletteGroup<ItemId>>,
      pub highlighted_id: Option<ItemId>,
      pub generation: u64,
      pub busy: bool,
      pub open: bool,
  }
  ```
  Callbacks: `on_query_change(Live<(PaletteMode, String, u64)>)`, `on_mode_change(Commit<PaletteMode>)`, `on_highlight_change(Live<ItemId>)` (`Up`/`Down`/`PageUp`/`PageDown` wraps while keeping text caret in search input), `on_execute(Commit<ItemId>)` (`Enter` or click), `on_dismiss(Cancel<()>)`.

### 3B.11 `Stepper` & `PipsPager` (`UIF-COMP-011`)

* **`Stepper` (Multi-Step Workflow Header / Vertical Wizard):**
  * **Anatomy & Redundant Cues (`UIF-PRIN-005`):** Horizontal (`height: 36px` / `size.control.lg`) or vertical step sequence connected by `1px border.subtle` (`Pending`/`Error`) or `1px accent.border` (`Completed`) connector lines. Each step node renders a `24×24px` (`size.hit_target.min`, `radius.pill`) indicator encoding status by both glyph and color:
    * `Completed`: `BG: accent.fill`, `FG: text.strong` (`4.80:1`), glyph `✓`.
    * `Current`: `BG: surface.active`, `Border: 2px accent.border` (`4.79:1`), `FG: text.strong` (`14.29:1`), step number `1..N`.
    * `Pending`: `BG: surface.raised`, `Border: 1px border.strong` (`4.24:1`), `FG: text.muted` (`7.16:1`), step number `1..N`.
    * `Error`: `BG: status.danger.fill`, `FG: text.strong` (`5.13:1`), glyph `!`.
  * **Non-Shifting Wizard Footer:** On step `0`, `[← Previous]` renders as `Disabled` rather than disappearing so footer button coordinates never shift across steps (`0px` shift per `UIF-PRIN-004`).
  * **Controlled Data Model & Callbacks:**
    ```rust
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum StepStatus { Pending, Current, Completed, Error }
    pub struct StepItem<StepId> { pub id: StepId, pub label: String, pub status: StepStatus, pub disabled: bool }

    pub struct StepperModel<StepId> {
        pub steps: Vec<StepItem<StepId>>,
        pub current_step: StepId,
        pub linear: bool,
    }
    ```
    Callbacks: `on_step_select(Commit<StepId>)`, `on_next(Commit<StepId>)`, `on_previous(Commit<StepId>)`, `on_finish(Commit<()>)`, `on_cancel(Cancel<()>)`.
* **`PipsPager` (Compact Carousel / Onboarding Page Dots):**
  * **Anatomy & Callbacks:** `24px`-high (`size.bar.status`) strip with `[◀]` (`24×24px`), `N` pip indicators (`6×6px` `size.indicator.dot` in `border.strong` when inactive; `16×6px` `radius.pill` in `accent.border` when active), and `[▶]` (`24×24px`).
  * **Data Model & Callbacks:** `PipsPagerModel { page: usize, total_pages: usize }` $\rightarrow$ `on_page_change(Commit<usize>)`.

### 3B.12 `SideSheet` / Docked Inspector Drawer (`UIF-COMP-012`)

* **Surface Selection Rule:** Use `Input Dialog` (`§4.5`) for 1-field prompts, `Modal Dialog` (`§4.6`) for blocking decisions, `Movable Overlay` (`§4.7`) for floating log/debug monitors, and `SideSheet` (`§3B.12`) for contextual inspectors with 4+ fields or live-linked document properties.
* **Anatomy & Modes:** Contextual right-hand (or left-hand) drawer (`size.sheet.default = 360px`, resizable `[280px, 520px]` via a `10px` `size.splitter.grab` edge divider) with a `36px` (`size.control.lg`) header (`16px SemiBold` `type.title_sm` title + `24×24px` `[Pin/Unpin]` + `24×24px` `[Close ×]`), scrollable `16px`-padded body, and optional `48px` action footer.
  * **`SheetMode::Docked` (`Z-Band 0`, `Escape Rank 0`):** Shares horizontal workspace layout with Center Pane without covering content.
  * **`SheetMode::ModelessOverlay` (`Z-Band 1`, `Escape Rank 1` when focused):** Floats over the workspace edge without a scrim.
  * **`SheetMode::ModalScrim` (`Z-Band 3`, `Escape Rank 2`):** Slides over `surface.scrim` when viewport width `< 800px` or when invoked as a blocking detail sheet, trapping `Tab` focus and honoring `UIF-MOD-002` when `dirty || busy`.
* **Controlled Data Model & Callbacks:**
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum SheetMode { Docked, ModelessOverlay, ModalScrim }

  pub struct SideSheetModel {
      pub title: String,
      pub mode: SheetMode,
      pub width: f32,
      pub open: bool,
      pub dirty: bool,
      pub busy: bool,
  }
  ```
  Callbacks: `on_resize_change(Live<f32>)`, `on_resize_commit(Commit<f32>)`, `on_resize_cancel(Cancel<f32>)`, `on_mode_toggle(Commit<SheetMode>)`, `on_close_request(Cancel<()>)`.

### 3B.13 `GridView` (2D Virtualized Tile / Card Grid) (`UIF-COMP-013`)

* **Anatomy & Responsive Column Math:** Virtualized 2D grid for media thumbnails, template cards, and skill catalogs. Computes column count and tile width deterministically from container width (after deducting the reserved `10px` `ScrollView` gutter):
  $$\text{cols} = \max\!\left(1, \left\lfloor \frac{\text{available\_width} + \text{gap}}{\text{min\_tile\_width} + \text{gap}} \right\rfloor\right), \quad \text{tile\_width} = \frac{\text{available\_width} - (\text{cols} - 1) \cdot \text{gap}}{\text{cols}}$$
  with `gap = space.md (12px)` and `min_tile_width: 160px`. Tile selection visuals use `BG: surface.active`, `FG: text.strong` (`14.29:1`), `Border: 2px accent.border` (`4.79:1`), plus a top-left `16×16px` `Checkbox` overlay in `ExtendedMulti` mode.
* **Controlled Data Model & Callbacks:**
  ```rust
  pub struct GridTile<Id> { pub id: Id, pub title: String, pub subtitle: Option<String>, pub preview_icon: Option<Icon>, pub disabled: bool }

  pub struct GridViewModel<Id> {
      pub items: Vec<GridTile<Id>>,
      pub selected_ids: BTreeSet<Id>,
      pub anchor_id: Option<Id>,
      pub focused_id: Option<Id>,
      pub min_tile_width: f32,
      pub tile_height: f32,
      pub selection_mode: TreeSelectionMode, // Single | ExtendedMulti
  }
  ```
  Callbacks: `on_selection_change(Commit<BTreeSet<Id>>)` (clamped 2D arrow navigation: `Left`/`Right` = `±1`, `Up`/`Down` = `±cols`, `Home`/`End` = first/last, with `Shift`/`Ctrl` ID-anchored range/toggle modifiers matching `§3B.6`), `on_activate(Commit<Id>)`, `on_context_menu(Commit<(BTreeSet<Id>, Point)>)`.

### 3B.14 `EmptyState`, `Skeleton` Shimmer & `PaginationBar` (`UIF-COMP-014`)

* **`EmptyState` (Non-Collapsing Placeholder per `UIF-PRIN-004`):**
  * Centered container (`padding: space.xl = 24px`, `max_width: 360px`) distinguishing `EmptyStateKind::{NoItems, NoMatches { query }, LoadError { code }}` with a `20×20px` (`size.icon.lg`, `text.muted` or `status.danger.text` on error) icon inside a `40×40px` `surface.raised` badge (`1px border.subtle`), `16px SemiBold` (`type.title_sm`, `text.strong`) headline, `13px Regular` (`text.muted`) explanation, and optional `Primary`/`Secondary` recovery CTA button (`Clear filter`, `Retry`, `New document...`).
  * **Data Model & Callbacks:**
    ```rust
    pub enum EmptyStateKind { NoItems, NoMatches { query: String }, LoadError { code: String } }

    pub struct EmptyStateModel<ActionId> {
        pub kind: EmptyStateKind,
        pub icon: Icon,
        pub title: String,
        pub description: String,
        pub primary_action: Option<(String, ActionId)>,
    }
    ```
    Callbacks: `on_action(Commit<ActionId>)`.
* **`Skeleton` Placeholder:**
  * Reserves exact target widget dimensions (`24px` tree rows, `28px`/`32px` table rows, `radius.widget = 4px` or `radius.pill`, `BG: surface.raised` pulsing to `surface.hover` via injected phase over `1200ms`, or static `surface.raised` with `1px border.subtle` when injected `reduced_motion == true`) while initial data loads so content arrival causes `0px` layout shift (`UIF-PRIN-003`).
* **`PaginationBar`:**
  * `32px`-high (`size.control.md`) footer bar (`BG: surface.raised`, top border `1px border.subtle`, `FontFamily::Mono` tabular numerals): left range summary (`Showing 1–50 of 1,240`, `12px text.muted`), center page-size `ComboBox` (`25 / 50 / 100`), right navigation buttons (`[⏮ First]`, `[◀ Prev]`, page indicator `Page 1 of 25`, `[Next ▶]`, `[Last ⏭]`, each `28×28px`). Boundary buttons on page `0` or `total_pages - 1` render as `Disabled` rather than disappearing (`0px` shift).
  * **Data Model & Callbacks:**
    ```rust
    pub struct PaginationModel {
        pub page: usize,
        pub page_size: usize,
        pub total_items: usize,
        pub page_size_options: Vec<usize>,
    }
    ```
    Callbacks: `on_page_change(Commit<usize>)`, `on_page_size_change(Commit<usize>)`.

### 3B.15 Developer & AI Workspace Composites: `CodeBlock`, `DiffViewer` & `KeybindRecorder` (`UIF-COMP-015`)

* **`CodeBlock` (Fenced Code & Tool-Call Snippet Container):**
  * **Anatomy:** Outer container (`BG: surface.code`, `Border: 1px border.subtle`, `Radius: radius.widget = 4px`). Top header bar (`height: 28px` / `size.control.sm`, `BG: surface.raised`, bottom border `1px border.subtle`, padding `[0, 12px]`) with language badge (`12px FontFamily::Mono`, `text.muted`) on the left and a `[Copy]` action button (`24×24px` hit target, fixed width slot switching icon `📋 -> ✓` in `status.success` for `time.hold.copied = 1500ms` on click with `0px` shift) on the right. Body renders `13px` (`type.ui_compact`) `FontFamily::Mono` text with optional `40px`-wide right-aligned tabular line-number gutter (`text.disabled`, `1px border.subtle` divider).
  * **Controlled Data Model & Callbacks:**
    ```rust
    pub struct CodeBlockModel {
        pub language: Option<String>,
        pub code: String,
        pub show_line_numbers: bool,
        pub copied_hold_remaining_ms: u32,
    }
    ```
    Callbacks: `on_copy(Commit<String>)`.
* **`DiffViewer` (Unified / Split Hunk Comparison):**
  * **Anatomy & Redundant Cueing (`UIF-PRIN-005`):** Renders line-by-line additions, deletions, and context lines in `13px FontFamily::Mono`. Added lines pair a leading `+` glyph and `2px status.success` left bar with `text.strong`; deleted lines pair a leading `-` glyph and `2px status.danger.text` left bar with `text.primary` (never relying on background tint alone). Includes per-hunk `[Accept hunk]` / `[Reject hunk]` (`28px` height) action buttons in the hunk header bar.
  * **Controlled Data Model & Callbacks:**
    ```rust
    pub enum DiffLineKind { Context, Added, Deleted }
    pub struct DiffLine { pub kind: DiffLineKind, pub old_line: Option<u32>, pub new_line: Option<u32>, pub text: String }
    pub struct DiffHunk<HunkId> { pub id: HunkId, pub header: String, pub lines: Vec<DiffLine> }
    pub enum DiffDisplayMode { Unified, SideBySide }

    pub struct DiffViewerModel<HunkId> {
        pub hunks: Vec<DiffHunk<HunkId>>,
        pub mode: DiffDisplayMode, // Unified | SideBySide
    }
    ```
    Callbacks: `on_accept_hunk(Commit<HunkId>)`, `on_reject_hunk(Commit<HunkId>)`, `on_mode_change(Commit<DiffDisplayMode>)`.
* **`KeybindRecorder` (Shortcut Capture Input):**
  * **Anatomy & Callbacks:** `32px`-high (`size.control.md`) specialized input (`BG: surface.code`, `Border: 1px border.strong`, `2px accent.border` when `listening == true`) rendering captured key chords as `24px`-high `FontFamily::Mono` keycap pills (`[Ctrl] + [Shift] + [P]`) with a trailing `24×24px` `[× Clear]` button. When focused and in `Listening` mode, intercepts raw chord presses (`Escape` cancels capture via `on_cancel` without clearing; `Backspace`/`Delete` clears binding to `None`) and surfaces chord conflicts via `FormField` validation (`status.danger.text`).
  * **Controlled Data Model & Callbacks:**
    ```rust
    pub struct KeyChord { pub ctrl: bool, pub alt: bool, pub shift: bool, pub super_key: bool, pub key: String }

    pub struct KeybindRecorderModel {
        pub chord: Option<KeyChord>,
        pub listening: bool,
        pub conflict_with: Option<String>,
        pub disabled: bool,
    }
    ```
    Callbacks: `on_chord_preview(Live<KeyChord>)`, `on_chord_commit(Commit<Option<KeyChord>>)`, `on_cancel(Cancel<()>)`.

---

## 4. Core Surface Specifications

### 4.1 Main Window (`UIF-WIN-*`)

#### 4.1.1 Spatial Architecture & Band Hierarchy
```text
┌─ Main Window (Default: 1000×700px, Minimum: 640×480px, BG: color.surface.base) ──┐
│ ┌─ Band 1: Top Navbar (Width: Fill, Height: 36px, BG: color.surface.raised) ───┐ │
│ └────────────────────────────────────────────────────── (1px border.subtle) ───┘ │
│ ┌─ Band 2: Middle Multi-Pane Workspace (Width: Fill, Height: Fill) ────────────┐ │
│ │  Left Pane (Tree + Search) │ Center Pane (Tabs + Content) │ Right Pane (TOC) │ │
│ └────────────────────────────────────────────────────── (1px border.subtle) ───┘ │
│ ┌─ Band 3: Bottom Command Bar (Width: Fill, Height: Shrink 44px..120px) ───────┐ │
│ └──────────────────────────────────────────────────────────────────────────────┘ │
│ [Overlay Stack Host (Z-Layers 1..3 mounted above Base Bands)]                    │
└──────────────────────────────────────────────────────────────────────────────────┘
```
* **Bottom Command Bar Arithmetic:** The `BOTTOM_BAR_HEIGHT = 44px..120px` outer band wraps the `MultiLineTextArea` (`UIF-ATOM-002e`) plus outer bar chrome and vertical padding.

#### 4.1.2 Usability & Behavioral Contract
* **`UIF-WIN-001` — Responsive Viewport & Persisted Geometry:** Window dimensions, screen position, split pane ratios, and user UI scale multiplier (`0.75×` to `2.00×` in `0.10×` steps, composed with OS DPI scale) persist across sessions via an injected storage handler (`[RUST-024]`). Geometry persistence writes occur only when a gesture settles (on drag/resize release or window close, never per motion frame). Resizing below `640 × 480` logical pixels (`WINDOW_MIN`) clamps internal pane minimums so the central workspace never collapses below `240px` (`CENTER_PANE_MIN_W`) width.
* **`UIF-WIN-002` — Unified Root Coordinate Space:** All floating overlays (context menus, fixed menus, cursor-anchored input dialogs, and movable utility windows) resolve coordinates within a single root viewport `Rectangle { x, y, width, height }` (supporting non-zero `(x, y)` origins as well as `(0, 0)`).
* **`UIF-WIN-003` — Global Escape Unwinding (`Escape Rank`):** Pressing `Escape` dismisses active drags, ephemeral popups, and floating layers in strict **`Escape Rank` priority** (intentionally unwinding pointer-captured drags and transient Band 2 menus before Band 3 blocking dialogs):
  1. **Active Pointer-Captured Drag (`Pre-Rank`):** If a pointer-captured drag (`Slider`, `RangeSlider`, `SplitDivider`, `DataTable` column resize, `SideSheet` resize, `MovableOverlay` drag/resize) is in flight, `Escape` cancels that drag (`on_cancel`) and restores the pre-drag snapshot first.
  2. **Rank 3 — Menu & Flyout Cascade (`Z-Band 2`):** Deepest open submenu branch $\rightarrow$ parent menu level $\rightarrow$ open root menu, `ComboBox` flyout, `SplitButton` menu, `AutoSuggestBox` list, `DatePicker` / `TimePicker` / `ColorPicker` flyout, `BreadcrumbBar` / `OverflowToolbar` menu, or `TeachingPopover`. (Non-interactive `Tooltip` overlays never consume `Escape`.)
  3. **Rank 2 — Blocking Dialogs & Command Palette (`Z-Band 3`):** Topmost open Input Dialog, Modal Dialog, `CommandPalette`, or `ModalScrim` `SideSheet` (subject to the dirty-state guard `UIF-MOD-002` and `busy` guard `UIF-ATOM-000b`).
  4. **Rank 1 — Focused Utility Overlay (`Z-Band 1`):** Topmost Movable Overlay Dialog, `ModelessOverlay` `SideSheet`, or `Toast` stack, **only** if keyboard focus currently resides inside that overlay/toast.
  5. **Rank 0 — Workspace Filter / Prompt (`Z-Band 0`):** Active search filter or in-flight prompt focus when no higher-rank surface consumes the key (returning `EscapeOutcome::Ignored` when the stack is empty).
  * **Workspace Flyout Auto-Close on Modal Entry:** Opening any `Z-Band 3` blocking surface (Modal Dialog, Input Dialog, `CommandPalette`, `ModalScrim` `SideSheet`) MUST first close every open workspace-anchored `Z-Band 2` menu or flyout (one whose trigger lies outside the new blocking surface), restoring no focus to it. A `Z-Band 2` surface therefore never remains open beneath a scrim, and the first `Escape` after a modal opens is always delivered to that modal (or to a flyout the modal itself spawned).
  * **Modal-Hosted Child Flyout Sub-Layering:** A `Z-Band 2` flyout (`ComboBox`, `AutoSuggestBox`, `SplitButton` menu, `DatePicker` / `TimePicker` / `ColorPicker`, `TeachingPopover`) spawned by a trigger control **inside** the topmost `Z-Band 3` surface keeps `ZBand::Menu` and `Escape Rank 3` (so `Escape` unwinds the flyout before its host modal) and remains subject to single-flyout mutual exclusion, but is painted **above** its host modal card and receives pointer hit-testing **ahead of** the host's interior absorption and scrim blocker. Its transparent click-away catcher covers only the host modal's bounds-plus-scrim, so an outside click closes the flyout without dismissing the host modal.

---

### 4.2 Top Navbar (`UIF-NAV-*`)

#### 4.2.1 Anatomy & Slot Layout
```text
┌─ Top Navbar (Height: 36px, Padding: [4px, 12px], BG: surface.raised, Bottom Border: 1px border.subtle) ─┐
│ [Brand Icon 20px] [App Title 16px Bold] │ [Status Pill] │ <── Flex Fill ──> │ [Filter Dropdown] [Actions...] [☰] │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

| Zone | Alignment | Sizing & Spacing | Usability & Visual Rules |
|---|---|---|---|
| **Left: Brand & Identity** | Left, Center-Y | `Shrink`, gap `8px` | `20×20px` (`size.icon.lg`) brand mark + `16px` bold (`type.title_sm`) application title (`text.strong`), followed by a `1px × 20px` vertical `border.subtle` divider. |
| **Left-Center: Live System Status** | Left, Center-Y | `Shrink`, gap `6px` | Compact status indicator: shows animated/ellipsis progress in `accent.text` while background indexing or tasks run, transitioning to a `status.success` (`✓`) summary when idle. |
| **Center: Elastic Spacer** | Fill | Min width `16px` | Separates status information from interactive workspace controls. |
| **Right: Workspace Controls** | Right, Center-Y | Height `28px`, gap `8px` | Filter dropdown (`NAV_FILTER_W = 140..220px`, truncated with `…`), followed by secondary dialog trigger buttons (`Batch...`, `Tools...`) and the far-right Fixed Menu button (`☰`, `28×28px`). |

#### 4.2.2 Interaction Details
* **`UIF-NAV-001` — Open-Menu Latch State:** When the navbar's fixed menu (`☰`) is open, the trigger button locks into its `Pressed/Active` visual state (`surface.active` background, `border.strong` outline), and `MenuCascade::toggle` closes the already-open root menu when clicked a second time.
* **`UIF-NAV-002` — Ellipsis Convention:** Any navbar button that launches a modal or overlay dialog appends `...` to its label (or its hover tooltip for icon-only buttons).
* **`UIF-NAV-003` — Icon-Only Tooltip Requirement:** Every icon-only control in the navbar displays a `Tooltip` (`UIF-ATOM-007`) after `400ms` hover (`0ms` on focus or within the `100ms` warm-handoff window), positioned `4px` below the navbar on `surface.overlay` with `1px border.strong` and `type.caption` text.

---

### 4.3 Multiple Panes (`UIF-PANE-*`)

#### 4.3.1 Split Topology & Proportions
```text
┌──────────────────────┬───────────────────────────────────────────┬────────────────────┐
│ Left Pane            │ Center Pane                               │ Right Pane         │
│ (Navigation & Filter)│ (Tabbed Document / Agent Workspace)       │ (Table of Contents)│
│                      │                                           │                    │
│ Min: 150px           │ Min: 240px (absorbs remaining width)      │ Min: 120px         │
│ Max: 20% Viewport    │                                           │ Max: 25% Viewport  │
│ Auto-fits longest    │                                           │ Auto-hides when no │
│ visible label        │                                           │ headings exist     │
└──────────────────────┴───────────────────────────────────────────┴────────────────────┘
                       ▲                                           ▲
                Split Divider 1                             Split Divider 2
            (1px border, 10px grab)                     (1px border, 10px grab)
```

#### 4.3.2 Split Resizing & Pane Ergonomics
* **`UIF-PANE-001` — Split Divider Affordance:**
  * Visual stroke at rest is a `1px` `color.border.subtle` vertical line.
  * The interactive pointer hit zone spans `10px` (`size.splitter.grab`, `5px` to each side of the divider line) with a horizontal resize cursor (`↔`).
  * On hover or active drag, the divider stroke widens/brightens to `2px color.accent.border` (`6.67:1` on `surface.base`).
* **`UIF-PANE-002` — Clamped Widths & Auto-Fit:**
  * Navigating to a directory automatically expands the Left Pane width to fit the longest visible item without wrapping, capped at `20%` of the viewport width (and never narrower than `LEFT_PANE_MIN_W = 150px`). Manual user drag overrides persist within `[150px, 20% viewport]`.
  * The Right Pane (Table of Contents, `RIGHT_PANE_MIN_W = 120px`) is conditionally mounted only when a document is active and contains at least one heading; when hidden, its width allocation returns seamlessly to the Center Pane.
* **`UIF-PANE-003` — Left Pane Tree & Search Layout:**
  * **Directory Ordering & Filtering:** Folders sort above files at every hierarchy level (`16px` nominal indentation per depth level, with proportional compression above deep nesting so at least `TREE_LABEL_MIN_W = 96px` of label width remains visible per `UIF-COMP-006`). Empty folders (or folders with zero matches under an active tag or content search filter) are automatically hidden.
  * **Tree Row Ergonomics:** Every row has minimum height `24px` (`size.hit_target.min`), `4px` (`space.xs`) horizontal inset, `type.ui_compact` (`13px`) label, and full keyboard focusability.
  * **Single & Range Selection:** Left-click selects one item; `Shift + Left-click` selects a contiguous range; double-click opens the item in the system editor. Selected rows render with `color.accent.fill` background (`RGB(93, 96, 238)`), `color.text.strong` (`RGB(255, 255, 255)`) text/chevron/guides, and a `2px` (`border.focus`) leading bar in `color.text.strong`. Special item states (such as PDF-backed notes) include a text/icon badge (`[PDF]`) so meaning and contrast (`4.80:1`) are preserved when selected.
  * **Pinned Search Footer:** A `32px` search bar is pinned at the bottom of the Left Pane with a text input (`placeholder: "Search content…"`), submitting on `Enter` or clicking the trailing `28×28px` action button (`🔍` when idle; switches to clear `×` while a filter is active).

---

### 4.4 Tabbed Windows (`UIF-TAB-*`)

#### 4.4.1 Tab Strip & Tab Item Anatomy
```text
┌─ Tab Strip (Height: 36px, Width: Fill, BG: surface.base, Bottom Border: 1px border.subtle) ──┐
│ [Horizontal Scroll Container (Min Tab Width: 96px, Max Tab Width: 220px)]                    │
│  ┌────────────────────────────┐ ┌────────────────────────────┐ ┌───────────────────────────┐ │
│  │ ━━━ (2px accent.border)    │ │                            │ │                           │ │
│  │ [●] [Icon] Title…      [×] │ │     [Icon] Other.md    [×] │ │     [⚙] System logs   [×] │ │
│  └────────────────────────────┘ └────────────────────────────┘ └───────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

* **Internal Tab Slots (Left to Right):**
  1. **Top Accent Indicator:** `2px` full-width top border (`color.accent.border` when active, `transparent` when inactive, `color.border.strong` on hover).
  2. **Leading Status Slot (`12px`):** Displays a `6px` (`size.indicator.dot`) `color.status.warning` dot (`●`) when the tab has unsaved changes; reserved/collapsed cleanly when saved.
  3. **Title Label (`Fill`, `13px`):** Single-line truncated with `…` when the tab reaches `220px` (`TAB_WIDTH = 96..220`); hovering the tab displays a tooltip with the full document path.
  4. **Trailing Close Button `[×]` (`24px × 24px` disjoint hit target):** Dedicated quiet button with a `14px` (`size.icon.sm`) `×` glyph, `4px` corner radius, `>= 2px` separation from the tab activation zone (`UIF-PRIN-008`), and independent hover highlight (`color.surface.hover` background, `color.status.danger.text` glyph at `5.36:1`).

#### 4.4.2 Tab Visual State Matrix
| Tab State | Surface Fill | Top Bar (`2px`) | Label Typography & Color | Perimeter Border | Close `[×]` State |
|---|---|---|---|---|---|
| **Active** | `surface.overlay` (`RGB(24,24,27)`) | `accent.border` (`5.94:1`) | `13px SemiBold`, `text.strong` (`17.72:1`) | `1px border.subtle` left/right; bottom open into viewer | Visible (`text.muted`; hover: `status.danger.text` on `surface.hover`) |
| **Inactive (Rest)** | `surface.raised` (`RGB(18,18,20)`) | `transparent` | `13px Regular`, `text.muted` (`7.16:1`) | `1px border.subtle` all sides | Visible (`text.disabled`; hover: `text.strong` on `surface.hover`) |
| **Inactive (Hovered)** | `surface.hover` (`RGB(32,32,36)`) | `border.strong` (`3.68:1`) | `13px Regular`, `text.primary` (`10.72:1`) | `1px border.strong` | Visible (`text.primary`; hover: `status.danger.text`) |
| **Dirty (`unsaved`)** | Matches Active/Inactive | Matches state | Leading `●` (`status.warning`) + `text.strong` | Matches state | Clicking `[×]` triggers Unsaved Confirmation Dialog |

#### 4.4.3 Tab Interaction & Lifecycle Rules
* **`UIF-TAB-001` — Pointer & Keyboard Bindings:**
  * Left-clicking the tab body activates the tab.
  * Middle-clicking anywhere on the tab OR left-clicking `[×]` requests tab closure (without first switching active selection if an inactive tab's `[×]` was clicked).
  * Right-clicking anywhere on the tab opens the Tab Context Menu (`§4.9`).
* **`UIF-TAB-002` — Non-Breaking Overflow Scrolling:** When open tabs exceed the Center Pane width, the tab strip scrolls horizontally while maintaining a minimum tab width of `96px` and keeping the `[×]` hit target at `24 × 24px`.
* **`UIF-TAB-003` — Dirty-Close Guard & External Deletion Sync:**
  * Attempting to close a tab with `unsaved == true` (via `[×]`, middle-click, `Close`, or `Close All`) opens a confirmation dialog (`DeleteTargets::{Single, Multiple}`) before discarding edits.
  * If an open tab's underlying file is deleted externally or via the tree, its tab closes automatically and focus moves to the most recently active remaining tab (or the structured empty state if none remain).

---

### 4.5 Input Dialogs (`UIF-INP-*`)

Input dialogs are compact, single-purpose cards (`Create directory`, `New document`, `Rename`, `Move`, and `Delete confirmation` for single or multiple items) designed for rapid completion without losing spatial context.

> **Host `SPEC.md` Literal Pin Exception (`UI-009`..`UI-034`, `UI-062`):** The footer labels `'Ok'` and `'Cancel'` are pinned verbatim by `src/app/ui/SPEC.md` and are exempt from the `UIF-PRIN-007` verb-first CTA rule and the `UIF-VER-005` lint gate. All other Input Dialog strings follow `UIF-PRIN-007`.

#### 4.5.1 Anatomy & Deterministic Box Model
```text
┌─ Input Dialog Card (Width: 320px, Padding: 16px, BG: surface.overlay, Border: 1px border.strong) ─┐
│ [Card Hit-Capture Boundary: absorbs all clicks inside card perimeter]                              │
│  ├─ Title Row (Height: 24px, 16px Bold, text.strong)                                               │
│  ├─ Gap (8px)                                                                                      │
│  ├─ Context / Target Label (Height: 20px, 14px Regular, text.muted, middle-truncated path/count)   │
│  ├─ Gap (8px)                                                                                      │
│  ├─ Primary Input Control:                                                                         │
│  │    • Text Entry (Height: 32px, auto-focused) OR                                                 │
│  │    • Destination Folder List (Scrollable, Max Height: 200px, 28px rows)                         │
│  ├─ Gap (8px)                                                                                      │
│  ├─ Validation Message Slot (Height: 20px, ALWAYS reserved; 12px status.danger.text when invalid)  │
│  ├─ Gap (8px)                                                                                      │
│  └─ Footer Action Row (Height: 32px, Right-Aligned, Gap: 8px: [Cancel (Secondary)] [Ok (Primary)]) │
└────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### 4.5.2 Positioning, Validation & Ergonomic Contract
* **`UIF-INP-001` — Cursor-Proximal Placement with Viewport Clamping:**
  * When invoked from a pointer action (such as a context menu item), the dialog opens via `Anchor::CursorProximal` offset `+2px, +2px` (`space.2xs`) from the trigger point so the pointer rests near the input and action buttons.
  * If `anchor + card_size` exceeds the viewport bounds, the top-left origin is clamped to maintain a minimum `8px` (`space.sm`) margin from all window edges. When invoked via keyboard shortcut (`keyboard_invoked == true`), the dialog centers in the viewport.
* **`UIF-INP-002` — Card-Level Click Absorption:**
  * The card rectangle is tested first in pointer routing: clicking anywhere inside the Input Dialog card—including on a `Disabled` `Ok` button or empty card whitespace—MUST be absorbed by the card boundary and MUST NEVER fall through to the backdrop scrim or dismiss the dialog.
* **`UIF-INP-003` — Live Validation & Zero-Shift Error Slot:**
  * Input values are validated continuously on keystroke (`ValidationRule::{Name, Rename}`: non-empty, valid filesystem basename characters, and distinct from the current name for `Rename`), honoring `UIF-PRIN-009` (initial pristine empty state does not show a red error banner until edited or submitted, while illegal characters flag immediately and recovery clears on the exact valid keystroke).
  * `InputLayout::solve` takes no error-visibility parameter: the `20px` (`type.body.line_height`) validation error slot is permanently reserved in the card layout so displaying or clearing an error never shifts the `Cancel` / `Ok` buttons.
  * While the input is invalid, `Ok` renders in the `Disabled` state and pressing `Enter` reveals the specific validation error in `color.status.danger.text` (`5.85:1` on `surface.overlay`) while keeping focus inside the text box (`Invalid + Focused` state: `2px status.danger.text` border + `2px accent.border` outer focus ring).
* **`UIF-INP-004` — Immediate Focus & Destructive Parity:**
  * Opening a text input dialog automatically places the text caret inside the input field (`Selection::rename_stem` selects the filename stem on `Rename`, keeping the whole name on a dotfile such as `.gitignore`).
  * Both single-item and multi-item deletions (`DeleteTargets::{Single, Multiple}`) require confirmation on the same `InputDialogHost` (displaying the item name for single delete, or the item count for multi-select delete, with the primary button styled as `ButtonIntent::Danger` and initial focus on `Cancel`).

---

### 4.6 Modal Dialogs (`UIF-MOD-*`)

Modal dialogs are centered, application-blocking surfaces mounted over a full-viewport dimmed scrim (`color.surface.scrim`: `RGBA(9, 9, 11, 0.55)`) for complex workflows (`Inline Document Editor`, `Tools & Integrations Configuration`, `Batch Processing`, `About`, and `License Attributions`).

#### 4.6.1 Standard Modal Anatomy
```text
┌─ Full-Viewport Dimmed Scrim (BG: RGBA(9,9,11,0.55), blocks background pointer input) ──────┐
│     ┌─ Modal Card (BG: surface.overlay, Border: 1px border.strong, Radius: 6px) ─────────┐ │
│     │ [Header Bar (Padding: 16px): Title (16px Bold)   <── Fill ──>   Close [×] (24×24)] │ │
│     │ ────────────────────────────────────────────────────────────── (1px border.subtle) │ │
│     │ [Body Region (Padding: 16px, Scrollable / Table / Editor)]                         │ │
│     │ ────────────────────────────────────────────────────────────── (1px border.subtle) │ │
│     │ [Inline Status / Error Banner (Rendered when validation, auth, or save fails)]     │ │
│     │ [Footer Bar (Padding: 12px 16px): Status Info   <── Fill ──>   [Cancel] [Primary]] │ │
│     └────────────────────────────────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### 4.6.2 Modal Sizing & Surface Variants
| Modal Type | Width Policy | Height Policy | Specific Design & Usability Requirements |
|---|---|---|---|
| **Inline Document Editor** | `80%` viewport (`EDITOR_MODAL_MIN = 520×380px`, `EDITOR_MODAL_MAX = 960×760px`) | `80%` viewport (`min: 380px`, `max: 760px`) | Uses high-contrast **Inverted Palette** (`color.editor.inverted.*`: black monospace text on white background, `1px` black border) to distinguish raw editing from dark-mode reading. Includes undo/redo (`100+` steps), `Line X, Col Y` status bar, pre-save Markdown validation, and `[Cancel]` / `[Save]` buttons. |
| **Tools & Integrations** | `TOOLS_MODAL_W = 720px` (clamped to `viewport.width - 32px`) | `TOOLS_MODAL_MIN_H = 320px`, `max: 80%` viewport | Structured 6-column table: `[☑ Enable]`, `[Group name]`, `[Kind: Internal/MCP]`, `[Tools list]`, `[Prompt char count + ✓ parallel chip]`, and `[Actions: ⚠ Warning tooltip / Authenticate / Restart]`. |
| **Batch Processing** | `BATCH_MODAL_MAX = 560×480px` (`560px` clamped to `viewport.width - 32px`) | Content-fit (`max: 480px`) | Directory/prompt selectors, live batch progress indicator, and `[Cancel]` / `[Run batch]` controls. |
| **About & Attributions** | `ABOUT_MODAL_W = 440px` (`About`) / `ATTRIBUTIONS_MODAL_SIZE = 640×480px` (`Attributions`) | Content-fit / `480px` scrollable | Application version, branding, scrollable third-party license table, and `[Close]` action. |

#### 4.6.3 Focus Containment & Data-Safety Invariants
* **`UIF-MOD-001` — Strict Keyboard Focus Trap & Focus Restoration:**
  * When a modal dialog opens, `FocusState::push_trap` records the previously focused control and moves keyboard focus automatically to its primary interactive control.
  * `Tab` and `Shift + Tab` wrap focus strictly within the active modal card's enabled focusable elements and MUST NEVER traverse to background panes underneath the scrim. Clicking or programmatically requesting focus outside the active trap is rejected.
  * When the modal closes (`pop_trap`), focus returns to the recorded prior control (or advances via slot-preserving fall-forward if that control became disabled or unmounted).
* **`UIF-MOD-002` — Unsaved Edit & Busy Protection on Dismiss:**
  * Every user-initiated close gesture (`Escape`, scrim click, and header `Close [×]`) routes through `attempt_dismiss`: if `busy == true`, dismissal is blocked (`DeclineReason::CannotUnwind`) unless `on_cancel_request()` is wired; if `dirty == true` (unsaved edits exist), dismissal returns `DeclineReason::CannotUnwind` unless confirmed or explicitly cancelled via `[Cancel]`.
* **`UIF-MOD-003` — Explicit Error Surfacing & Recovery:**
  * If a save, validation, or background authentication action fails inside a modal, the modal MUST remain open and display a structured `ModalError` banner (`color.status.danger.text` on `surface.overlay`) carrying 1-based line/column coordinates where applicable and a `Recovery::Retry` affordance. Only explicit `set_error` / `clear_error` calls mutate the banner—clicking elsewhere inside the modal body never clears it—and a displayed error banner alone (when `dirty == false && busy == false`) does not block explicit modal dismissal.

---

### 4.7 Movable Overlay Dialogs (`UIF-MOV-*`)

[INFERENCE] Utility and diagnostic surfaces (`Background Operations Log` and `Agent Debug Inspector`) are frequently monitored while reading documents or running prompts. Rendering them as **movable, non-blocking floating overlays** allows side-by-side inspection without locking the main workspace behind a modal scrim.

#### 4.7.1 Anatomy & Draggable Header Box Model
```text
┌─ Movable Overlay Card (Default: 680×440px, Min: 420×260px, BG: surface.overlay, Border: 1px border.strong) ─┐
│ ┌─ Draggable Title Bar (Height: 32px, Padding: [0, 12px], BG: surface.active, Cursor: Grab / Grabbing) ───┐ │
│ │ [∷ Grip] [Window Title (13px Bold, text.strong)]  <── Fill ──>  [Token/Cost Summary] [Dock] [Close ×]   │ │
│ └─────────────────────────────────────────────────────────────────────────────────────────────────────────┘ │
│ ├─ Filter & Control Bar (Height: 36px, Padding: [4px, 12px], BG: surface.raised, Gap: 8px):                 │
│ │   [Category Dropdown] [Search Filter Input] [☑ Auto-scroll] <── Fill ──> [Copy / Clear]                   │
│ ├─ Scrollable Content Viewport (Height: Fill, BG: surface.sunken, Monospace 12px/13px):                     │
│ │   • Structured timestamp (`text.muted`) + `[Category]` tag (`accent.text`) + payload / collapsible JSON   │
│ └─ Footer Status & Resize Bar (Height: 24px, Padding: [0, 12px], BG: surface.raised):                       │
│     [Showing N of M entries (K filtered out)]        <── Fill ──>        [◢ Resize Handle 24×24 (16px icon)] │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### 4.7.2 Dragging, Clamping & Pass-Through Contract
* **`UIF-MOV-001` — Direct-Manipulation Title Bar Dragging & Gesture Capture:**
  * Pressing and holding the left mouse button inside the `32px` (`size.control.md`) Title Bar *outside* the trailing action button cluster (`[Dock]`, `[Close ×]`) enters the `Dragging` state (`cursor: Grabbing`) and translates the card in real time (`MOVABLE_DEFAULT = 680 × 440`, `MOVABLE_MIN = 420 × 260`).
  * Active drag and resize gestures capture pointer tracking until mouse-button release so fast pointer motion outside the card perimeter never drops the gesture. Pressing `Escape` during an active drag or resize cancels the gesture (`on_cancel`) and restores the pre-gesture card rect.
  * Position and dimensions are preserved across open/close toggles (`reopen`) and persisted via injected `GeometryStorage` (`[RUST-024]`) only when the gesture settles.
* **`UIF-MOV-002` — Title-Bar Reachability Clamping & Zero-Overshoot Dragging:**
  * Clamping protects the `32px` **Title Bar** (the user's recovery grip), while allowing the card body to extend past the right or bottom viewport edge when the window is smaller than the card:
    * Horizontal `x` is clamped to `[viewport.x + 120 - card_width, viewport.x + viewport.width - 120]` so at least `min(TITLE_REACHABLE_MIN_W = 120px, card_width, viewport.width)` of the title bar remains reachable.
    * Vertical `y` is clamped to `[viewport.y + navbar_height (36px), viewport.y + viewport.height - title_bar_height (32px)]`.
  * **Zero-Overshoot Incremental Clamping:** Each drag frame applies the pointer delta to the **last clamped position** (never to `drag_start + cumulative_delta`), ensuring that reversing direction after hitting a viewport edge moves the card on the very first reverse pixel.
* **`UIF-MOV-003` — Non-Blocking Workspace Pass-Through & Z-Promotion:**
  * Movable overlays do **not** render a backdrop scrim. Pointer clicks, scroll events, and text selection outside the floating card's bounds pass directly through to the underlying Main Window panes (`Z-Band 0`).
  * When multiple movable overlays are open simultaneously, `pointer_down` anywhere inside a background overlay card immediately promotes it to the top of `Z-Band 1`.
* **`UIF-MOV-004` — Respectful Stream Auto-Scroll:**
  * `FollowState::resolve(setting, at_bottom)` follows incoming log/debug entries to the bottom **only** when `[☑ Auto-scroll]` is enabled **and** the scroll viewport is within `2px` (`space.2xs` sub-pixel tolerance) of the bottom.
  * If the user scrolls up while `[☑ Auto-scroll]` is checked, the view enters `Inspecting (Paused)`, tracks the count of new behind-the-scenes entries for a jump-to-latest affordance, and resumes automatic following as soon as the user scrolls back within `2px` of the bottom.

---

### 4.8 Fixed Menus (`UIF-FMENU-*`)

Fixed menus are temporary command surfaces anchored to a permanent UI trigger button (such as the top-right Navbar `☰` menu).

#### 4.8.1 Menu Card & Row Slot Anatomy
Fixed menus and Context menus share a single unified surface and row grid (`MENU_CARD_WIDTH = 224px..320px`, card inset `space.xs = 4px`, row horizontal padding `space.md = 12px`, vertical row padding composition `space.sm + space.2xs = 10px` where applicable):

```text
┌─ Menu Surface Card (Width: 224px..320px, Padding: 4px, BG: surface.overlay, Border: 1px border.strong) ─┐
│ ┌─ Menu Row (Height: 32px, Padding: [0, 12px], Gap: 8px, Radius: 4px) ────────────────────────────────┐ │
│ │ [Leading Slot: 16×16px] │ [Primary Label: Fill, 13px Regular] │ [Trailing Slot: Shrink, 12px Muted] │ │
│ │  (✓ Check / • Radio /   │  (Single-line text; `...` suffix    │  (Keyboard shortcut hint OR         │ │
│ │   Action Icon)          │   when row opens a dialog)          │   Submenu chevron `>`)              │ │
│ └─────────────────────────────────────────────────────────────────────────────────────────────────────┘ │
│ ───────────────── Separator Row (Height: 9px = 4px top + 1px border.subtle + 4px bottom) ────────────── │
│ [Section Heading Row (Height: 24px, Padding: [4px, 12px], 12px SemiBold, text.muted)]                   │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### 4.8.2 Anchor Placement, Cascade & Dismissal Rules
* **`UIF-FMENU-001` — Control-Rectangle Anchoring:**
  * A fixed menu anchors via `Anchor::Control` to its trigger button's bounding rectangle (`AnchorRect`), defaulting to opening **below** the trigger (`4px` / `space.xs` vertical gap) and aligning its right edge with the trigger's right edge (`ControlAlignment`) when positioned on the right side of the navbar.
* **`UIF-FMENU-002` — Persistent Parent Surfaces, Diagonal Safe-Triangle Aiming & Submenu Cascade Discipline:**
  * Activating or hovering an enabled submenu parent row (`Windows... >`, `Chat models >`, `Table wrap algorithm >`) with at least one enabled child item opens the child submenu card adjacent to the parent card (`2px` / `space.2xs` horizontal gap, flipping to the left when anchored near the right viewport edge, aligned to the parent row's top edge) while keeping the parent menu card visible and interactive.
  * Moving to a sibling row on an ancestor card retires the deeper child branch after the `150ms` (`time.delay.submenu`) diagonal aiming corridor check (`UIF-PRIN-008`), except when hovering the open child branch's own trigger row, which preserves the open child card without flicker.
* **`UIF-FMENU-003` — Controlled Selection & Dismissal Policy (`ClosePolicy`):**
  * **Action Rows (`ClosePolicy::Close`):** Emit their command on pointer release (or `Enter`/`Space`) and close the entire menu tree immediately.
  * **Radio-Choice Rows (`•` mutually exclusive choice, `ClosePolicy::Close`):** Select the option (re-activating an already-selected radio row is a selection no-op so a radio group can never be deselected to empty) and close the menu by default.
  * **Checkable Toggle Rows (`✓` independent window/feature toggles, `ClosePolicy::StayOpen`):** Toggle the checkmark state in place and keep the menu open by default so the user can toggle multiple options in one visit (with per-row caller override to `ClosePolicy::Close` where required).

---

### 4.9 Context Menus (`UIF-CMENU-*`)

Context menus are pointer-anchored command surfaces opened by right-clicking a workspace target (a single file, a folder, a multi-file selection in the directory tree, or a document tab in the tab strip).

> **Host `SPEC.md` Literal Pin Exception (`UI-009`..`UI-034`, `UI-062`):** The row labels `'Open in Editor'`, `'Show in File Explorer'`, `'Create Directory ...'`, `'Close Others'`, `'Close All'`, `'Copy Path'` (§4.9) and `'Windows...'` (§4.8) are pinned verbatim by `src/app/ui/SPEC.md` and are exempt from the `UIF-PRIN-007` sentence-case and ellipsis rules and from the `UIF-VER-005` lint gate. Unpinned rows (`Copy path` on tree targets, `New document...`, `Run as prompt`) follow `UIF-PRIN-007`.

#### 4.9.1 Semantic Grouping & Row Hierarchy
To prevent cognitive overload in long action lists, context menus organize items into functional groups separated by `1px` `color.border.subtle` dividers (`9px` total separator row height), placing destructive actions in the final group:

| Context Target | Structured Group Order |
|---|---|
| **Single File (Tree)** | **1. Open & Output:** `Edit`, `Open in Editor`, `Print`<br>— *Separator* —<br>**2. AI & Automation:** `Run as prompt`, `Format Markdown`, Contextual Note Skills<br>— *Separator* —<br>**3. File Management:** `Rename...`, `Move...`, `Copy path`, `Show in File Explorer`<br>— *Separator* —<br>**4. Destructive:** `Delete...` |
| **Directory (Tree)** | **1. Creation:** `New document...`, `Create Directory ...`<br>— *Separator* —<br>**2. Folder Automation & Management:** Folder Skills, `Rename...`, `Move...`, `Copy path`, `Show in File Explorer`<br>— *Separator* —<br>**3. Destructive:** `Delete...` |
| **Multi-File Selection (Tree)** | **1. Batch AI:** `Merge`<br>— *Separator* —<br>**2. Destructive:** `Delete...` *(routes to plural confirmation)* |
| **Document Tab (Tab Strip)** | **1. Tab Lifecycle:** `Close`, `Close Others`, `Close All`<br>— *Separator* —<br>**2. Edit & Automation:** `Edit`, `Open in Editor`, `Format Markdown`, Contextual Note Skills<br>— *Separator* —<br>**3. Location:** `Copy Path`, `Show in File Explorer` |

#### 4.9.2 Shared Menu Row Visual State Matrix (Fixed & Context Menus)
| Row State | Row Background | Leading Slot (`16px`) | Primary Label (`13px`) | Trailing Slot (`12px` / `>`) | Pointer Cursor |
|---|---|---|---|---|---|
| **Rest (Enabled)** | `transparent` (`UnpaintedBackground`) | `accent.text` (`✓` / `•`, `9.49:1`) or `text.muted` | `text.primary` (`11.72:1`) | `text.muted` (`6.78:1`) | `Pointer` |
| **Hovered** | `surface.hover` (`RGB(32,32,36)`) | `text.strong` (`16.24:1`) | `text.strong` (`16.24:1`) | `text.strong` (`16.24:1`) | `Pointer` |
| **Pressed** | `surface.active` (`RGB(42,42,46)`) | `text.strong` (`14.29:1`) | `text.strong` (`14.29:1`) | `text.strong` (`14.29:1`) | `Pointer` |
| **Keyboard Focused** | `surface.hover` + `1px accent.border` (`5.44:1`) inset outline | `text.strong` (`16.24:1`) | `text.strong` (`16.24:1`) | `text.strong` (`16.24:1`) | `Pointer` |
| **Active Submenu Parent** | `surface.hover` (`RGB(32,32,36)`) | `text.strong` (`16.24:1`) | `text.strong` (`16.24:1`) | `accent.text` (`>`, `8.72:1`) | `Pointer` |
| **Destructive Row (Rest)** | `transparent` | `status.danger.text` (`5.85:1`) | `status.danger.text` (`5.85:1`) | `text.muted` (`6.78:1`) | `Pointer` |
| **Destructive Row (Hovered)** | `status.danger.fill` (`RGB(216,28,20)`) | `text.strong` (`5.13:1`) | `text.strong` (`5.13:1`) | `text.strong` (`5.13:1`) | `Pointer` |
| **Destructive Row (Pressed)** | `status.danger.fill_active` (`RGB(202,24,16)`) | `text.strong` (`5.71:1`) | `text.strong` (`5.71:1`) | `text.strong` (`5.71:1`) | `Pointer` |
| **Destructive Row (Focused)** | `status.danger.fill` + `1px text.strong` (`5.13:1`) inset outline | `text.strong` (`5.13:1`) | `text.strong` (`5.13:1`) | `text.strong` (`5.13:1`) | `Pointer` |
| **Disabled / Placeholder** | `transparent` (never inherits hover fill) | `text.disabled` (`5.27:1`) | `text.disabled` (`5.27:1`) | `text.disabled` (`5.27:1`) | `Default` (absorbs click; does not close menu) |
| **Section Heading** | `transparent` | None | `12px SemiBold` (`CAPTION + SemiBold`), `text.muted` (`6.78:1`) | None | `Default` (non-interactive) |

#### 4.9.3 Pointer Placement, Quadrant Flipping & Keyboard Navigation
* **`UIF-CMENU-001` — Automatic Quadrant Flipping & Viewport Clamping:**
  * Context menus open via `Anchor::Pointer` anchored to the root-space pointer coordinate `(x, y)` with a `+2px, +2px` (`space.2xs`) offset (`Right` and `Below` preference).
  * **Horizontal Flip:** If `x + 2 + menu_width > viewport.width - 8`, the menu flips to open to the **left** of the cursor (`x - menu_width - 2`).
  * **Vertical Flip:** If `y + 2 + menu_height > viewport.height - 8`, the menu flips to open **above** the cursor (`y - menu_height - 2`).
  * **Scrollable Fallback & Tie-Breaking:** If neither side fits in a small window, the solver chooses the roomier side (breaking ties in favor of the default `Right` / `Below` preference), clamps to `8px` (`space.sm`) viewport margins, and sets `scrollable = true` so the menu scrolls vertically inside the card.
* **`UIF-CMENU-002` — Single-Menu Mutual Exclusion & Click-Away Dismissal:**
  * At most one menu tree (either a Context Menu or a Fixed Navbar Menu) may be open at a time; opening one automatically closes the other.
  * Clicking anywhere outside the open menu surface(s) on the transparent click-away layer closes the entire menu hierarchy immediately and restores prior focus. Releasing a click on a `Disabled` row, `Heading`, or `Separator` inside the card is absorbed without dismissing the menu.
* **`UIF-CMENU-003` — Complete Roving Keyboard Navigation:**
  * `Down` / `Up`: Cycles (wraps) focus through enabled interactive rows within the active menu card, skipping headings, separators, and disabled items.
  * `Right`: Opens the focused submenu row (if it contains at least one enabled item) and focuses its first enabled child item.
  * `Left` / `Escape`: Closes the deepest open submenu and pops focus back to its parent trigger row (or closes the root menu and restores prior workspace focus when at the top level).
  * `Enter` / `Space`: Activates the currently focused row.

---

## 5. Unified Z-Order & Layering Architecture (`UIF-ZORD-*`)

To guarantee predictable hit-testing, visual elevation, and keyboard routing across all atomic popups, composite patterns, and the nine core surfaces, the window separates **Visual Elevation (`Z-Band`, 0..3)** from **Keyboard `Escape` Unwind Priority (`Escape Rank`, Pre-Rank + 3..0)**:

```text
Top Non-InteractiveOverlay : Hover/Focus Tooltips (HitTest::Ignore, outside SurfaceRegistry, never consumes Escape)
Z-Band 3 (Top Visual)      : Blocking Modal & Input Dialogs, CommandPalette, Modal SideSheet ──► Dimmed Scrim (0.55 alpha) + Focus Trap (Escape Rank 2)
Z-Band 2                   : Menus, ComboBox, AutoSuggest, Date/Time/Color Pickers, Popovers ──► Transparent Click-Away Blocker + Roving Focus (Escape Rank 3 — unwinds first!)
Z-Band 1                   : Movable Utility Overlays, Modeless SideSheet & Toast Stack      ──► Modeless Pass-Through Outside Card + Z-Promotion (Escape Rank 1 when focused)
Z-Band 0 (Base)            : Main Window 3-Band Workspace + Docked SideSheet + NavigationRail──► Top Navbar + Middle Multi-Panes (with Tabs) + Bottom Bar (Escape Rank 0)
```

| Z-Band | Escape Rank | Surface & Widget Types | Backdrop Behaviour | Pointer Routing | Keyboard Routing |
|---|---|---|---|---|---|
| **Band 0: Workspace** | `Rank 0` (Last) | Main Window, Navbar, Multiple Panes, Tabbed Windows, `NavigationRail`, Docked `SideSheet`, `TreeView`, `DataTable`, `GridView`, `FormField`, `InfoBar`, `Expander` | None (`surface.base`) | Direct interaction with panes, splitters, tabs, trees, grids, and tables | Standard wrapping `Tab` / `Shift+Tab` (single stop per composite collection) and roving arrow traversal across enabled workspace controls |
| **Band 1: Utility Overlays & Toasts** | `Rank 1` (If focused) | Movable Overlay Dialogs (`Logs`, `Agent Debug`), Modeless `SideSheet`, `Toast` / `SnackBar` notification stack | None (pass-through) | Captures events inside card/toast bounds; promotes clicked overlay card to top of Band 1; passes outside clicks to Band 0 | Focuses card/toast controls when clicked; hovering or focusing a Toast pauses and banks auto-dismiss countdown; `Escape` unwinds **only** when focus is inside the overlay |
| **Band 2: Menus & Flyouts** | `Rank 3` (First after active drag) | Fixed Menus (`☰`), Context Menus + Nested Submenus, `ComboBox` flyouts, `SplitButton` menus, `AutoSuggestBox` completion lists, `DatePicker` / `TimePicker` / `ColorPicker` flyouts, `BreadcrumbBar` / `OverflowToolbar` menus, `TeachingPopover` | Full-window transparent click-away catcher (workspace-anchored) or host-modal-bounded catcher (modal-hosted) | Single-flyout mutual exclusion; captures clicks inside open card(s); outside clicks dismiss Band 2 flyouts; **workspace-anchored flyouts are auto-closed when any Band 3 surface opens**; **modal-hosted flyouts paint above and hit-test ahead of their host Band 3 card and scrim** | Captures `Up`/`Down`/`Left`/`Right`/`Enter`/`Space`/`Tab` and **always consumes `Escape` before Band 3 modals** (deepest submenu/flyout $\rightarrow$ root menu); zero-match `AutoSuggest`/`ComboBox` queries do not mount an empty Band 2 focus trap |
| **Band 3: Modals & Command Palette** | `Rank 2` (Second) | Input Dialogs, Modal Dialogs (`Editor`, `Tools`, `Batch`, `About`), `CommandPalette`, Modal `SideSheet`, plus non-interactive hover `Tooltip` overlay (`HitTest::Ignore`) | Full-window dimmed scrim (`RGBA(9,9,11,0.55)`) for modals | Modals block Bands 0–1 and workspace-anchored Band 2 completely and absorb interior clicks; flyouts spawned from inside the topmost modal sit above it (see `UIF-WIN-003` sub-layering rule); scrim click dismisses only if `dirty == false && busy == false`; `Tooltip` never intercepts pointer events | Traps `Tab`/`Shift+Tab`, `Enter`, and `Escape` strictly inside topmost modal (after any open modal-hosted Band 2 flyout is unwound); `Tooltip` never consumes `Escape` |

* **Normative Layering Rules (cross-referenced from `UIF-WIN-003`):**
  1. **Workspace Flyout Auto-Close on Modal Entry:** Registering any `Z-Band 3` surface closes every open `Z-Band 2` surface whose owner is not inside that Band 3 surface, before the new focus trap is pushed.
  2. **Modal-Hosted Child Flyout Sub-Layering:** A `Z-Band 2` surface whose owner is inside the topmost `Z-Band 3` surface is ordered immediately above its host for paint and pointer routing, while keeping `Escape Rank 3`. The `ZBand` enum (`Workspace = 0, UtilityOverlay = 1, Menu = 2, Modal = 3`) and `Escape Rank 0..3` are unchanged; sub-layering is a registry ordering rule, not a fifth band.
* **Surface Controller Contracts (compact signatures):**
  ```rust
  pub enum ZBand { Workspace = 0, UtilityOverlay = 1, Menu = 2, Modal = 3 }
  pub struct SurfaceId(pub u64);
  pub enum DeclineReason { NotOpen, NotFocused, CannotUnwind }
  pub enum UnwindAnswer { Dismissed, Declined(DeclineReason) }
  pub enum EscapeOutcome { Consumed(SurfaceId), Ignored }
  pub struct SurfaceRegistry { /* ordered Vec<(SurfaceId, ZBand, Option<SurfaceId> /* host */, Vec<Rectangle>)> */ }
  impl SurfaceRegistry {
      pub fn register(&mut self, id: SurfaceId, band: ZBand, host: Option<SurfaceId>, regions: Vec<Rectangle>);
      pub fn unregister(&mut self, id: SurfaceId);
      pub fn route_pointer(&self, point: Point) -> Option<SurfaceId>;
      pub fn escape(&mut self, answer: impl FnMut(SurfaceId) -> UnwindAnswer) -> EscapeOutcome;
  }
  pub struct FocusId(pub u64);
  impl FocusState {
      pub fn focus(&mut self, id: FocusId) -> bool;
      pub fn push_trap(&mut self, group: FocusGroupId);
      pub fn pop_trap(&mut self) -> Option<FocusId>; // returns restored focus target
  }
  pub enum Anchor { Control(Rectangle), Pointer(Point), CursorProximal { point: Point, keyboard_invoked: bool } }
  pub fn solve(anchor: Anchor, card: Size, viewport: Rectangle, preferred: PreferredSide) -> Placement; // Placement { origin: Point, scrollable: bool }
  impl InputLayout { pub fn solve(card: Rectangle, kind: InputDialogKind) -> InputLayout; } // no error-visibility parameter
  impl FollowState { pub const fn resolve(setting: FollowSetting, at_bottom: bool) -> FollowState; }
  pub struct ModalError { pub message: String, pub location: Option<LineCol>, pub recovery: Option<Recovery> }
  pub trait GeometryStorage { fn load(&self, key: &str) -> Option<Rectangle>; fn save(&mut self, key: &str, rect: Rectangle); }
  ```

---

## 6. Design Quality Gates & Verification Criteria (`UIF-VER-*`)

An AI coding agent implementing or auditing widgets against this specification MUST verify compliance across five objective quality gates:

1. **Token & Contrast Gate (`UIF-VER-001`):**
   * Zero raw RGB/hex color literals, inline string literals, magic spacing numbers, or raw timer literals outside the central token definitions (`§2`), deterministic token compositions (`§1.1`), and the closed table of named Surface Geometry Constants (`§2.2`).
   * Automated WCAG 2.1 luminance contrast assertions verify every reachable `(Variant × VisualState)` pairing across the four stroke/fill categories (`Judged`, `CoincidentWithFill`, `UnpaintedBackground`, `RestingDivider`), confirming $\ge 4.50:1$ for all active text pairings (including role-based `text.strong` promotion on `accent.fill` and `status.danger.fill`), `[4.00:1, 6.00:1]` for `text.disabled` on standard control surfaces (`base` through `active`, `6.02:1` on `sunken`, and never on `accent.fill`), and $\ge 3.00:1$ for all `Judged` active borders, solid fills on `base` through `overlay`, `Checkbox`/`Radio`/`Switch` states, `Slider`/`RangeSlider` tracks/thumbs, `ScrollView` thumbs, `TreeView` guides, and adaptive focus rings.
2. **Geometry, Hit-Target & Responsive Container Gate (`UIF-VER-002`):**
   * Every interactive element measures at least `24px × 24px` in hit bounds; nested interactive zones (`Tab [×]`, `Chip [×]`, `ListTile` trailing action, `TreeView` chevron, `DataTable` resize handle, `Expander` header action) are strictly disjoint by $\ge 2\text{px}$; all text measures at least `12px`; splitters, side sheets, and table column dividers provide a `10px` grab zone; text inputs, `SegmentedButton` `CHECK_SLOT`s, `Button` `SPINNER_SLOT`s, `ScrollView` `10px` gutters, `FormField` single-line captions, `Expander` headers, `Skeleton` placeholders, and input dialogs guarantee `0px` layout shift across trailing-slot, `ActionPending` (`busy`), hover, and validation-error state changes.
   * Pure boundary solver property tests verify menu/flyout quadrant flipping, tie-breaking, input dialog cursor clamping, tab strip overflow scrolling, `TreeView` `96px` label floor compression and `f64` virtual index multiplication, `DataTable` 3-regime column width solving, `GridView` column count derivation, `BreadcrumbBar` nearest-root middle-ancestor elision (`[…]`), `OverflowToolbar` (`[⋯]`) `OverflowRank` + rightmost-first container-width collapse, and movable overlay incremental title-bar clamping (with zero drag-overshoot dead zone) across extreme viewport sizes (down to `640 × 480px`) and non-finite inputs.
3. **Interaction, State-Machine & Callback Lifecycle Gate (`UIF-VER-003`):**
   * Headless state and interaction tests verify:
     * The full 32-combination `InteractionInput` truth table (resolving to the 9 reachable `VisualState`s with total `Disabled` dominance) and press-and-release activation.
     * Strict nominal type separation and lifecycle behavior for **`on_change(Live<T>)` vs. `on_commit(Commit<T>)` vs. `on_cancel(Cancel<T>)`** across `Button`, `TextInput`, `PasswordInput`, `AutoSuggestBox`, `MultiLineTextArea`, `ComboBox`, `Slider`, `RangeSlider`, `NumberBox`, `DatePicker`, `TimePicker`, `ColorPicker`, `KeybindRecorder`, `ScrollView` thumb drag, `Expander` toggle, `DataTable` column resize, `SideSheet` resize, and `MovableOverlay` drag/resize (verifying `on_commit` fires **exactly once** on settle/submit and `ActivationGate` suppresses double-click races).
     * Keyboard navigation contracts for single-`Tab`-stop roving collections (`SegmentedButton`, `RadioGroup`, `NavigationRail`, `TreeView`, `DataTable`, `GridView`, `CalendarView`, `CommandPalette`, `Stepper`), `Toast` dual-hold (`hovered || focused`) timer banking and overflow eviction rules, card-interior click absorption, dirty/busy close guards, focus trap containment/restoration, and `Escape Rank` unwinding (`Active Drag [Pre-Rank] -> Menu/Flyout [Rank 3] -> Modal/Palette [Rank 2] -> Focused Movable Overlay/Toast [Rank 1] -> Workspace [Rank 0]`).
4. **Visual & Layer Snapshot Gate (`UIF-VER-004`):**
   * Deterministic headless snapshots verify both **resolved semantic token assignments** and **ordered draw-layer quad lists (`Layer { bounds, fill, border }`)** across all 9 reachable states for every atomic widget (`§3.1–§3.13`), every composite widget pattern (`§3B.1–§3B.15`), and every core surface (`§4.1–§4.9`).
5. **Style Guide, Microcopy & Ergonomic Invariant Gate (`UIF-VER-005`):**
   * Automated checks verify sentence-case chrome labels, ASCII `...` suffix consistency on dialog-launching actions versus immediate actions (and Unicode `…` only for truncation, placeholders, and `[…]` elision), at most one `Primary` CTA per dialog/form/toolbar scope, homogeneous `ControlDensity` (`28px` vs `32px`) within every horizontal row, automatic `Tooltip` registration on all truncated (`…`) labels and icon-only controls, `SecretInput` debug/log redaction (`NFR-009`), and "Reward Early, Punish Late" validation state transitions (`UIF-PRIN-009`).
   * **Host `SPEC.md` Literal Pin Exception:** The label checks MUST skip the closed Host `SPEC.md` Literal Pin Table (`'Ok'`, `'Cancel'`, `'Open in Editor'`, `'Show in File Explorer'`, `'Create Directory ...'`, `'Close Others'`, `'Close All'`, `'Copy Path'`, `'Windows...'`) pinned by `UI-009`..`UI-034` and `UI-062` in `src/app/ui/SPEC.md`; the exception list is closed and any addition requires a corresponding host `SPEC.md` requirement.
