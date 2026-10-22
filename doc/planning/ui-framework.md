# UI Framework & Widget Library — Design Specification

> **Status:** Draft — design specification for review.
> **Date:** 2026-10-22
> **Scope:** Visual design language, design token hierarchy, ergonomic geometry, interaction state machines, controlled data models, responsive callback contracts, atomic widgets (`UIF-ATOM-*`), composite widget patterns (`UIF-COMP-*`), and the nine core application surfaces: Main Window, Navbar, Multiple Panes, Tabbed Windows, Input Dialogs, Modal Dialogs, Movable Overlay Dialogs, Fixed Menus, and Context Menus.

Statements in this specification that express pattern-based conclusions or broad design principles are prefixed with `[INFERENCE]` or `[GENERALIZATION]` in accordance with the Anti-Hallucination Protocol.

---

## 1. Design Philosophy & Usability Principles

[GENERALIZATION] High-usability desktop interfaces achieve visual harmony and speed of use by combining mathematical consistency (strict spatial, typographic, and perceptual color scales) with unambiguous interaction affordances (explicit state feedback, predictable focus choreography, and forgiving hit geometry).

### 1.1 Core Design Pillars

1. **Perceptual Harmony Through Tokenization (`UIF-PRIN-001`):**
   * Every visual property—color, elevation, spacing, sizing, border radius, stroke weight, and typography—is selected from a closed set of semantic design tokens (`§2`). Composite surface layout bounds (such as default/min/max card dimensions in `§4`) are defined as named **Surface Geometry Constants** per surface. Raw numeric literals in individual widget definitions are forbidden.
2. **High-Density Desktop Ergonomics (`UIF-PRIN-002`):**
   * [INFERENCE] Productivity applications require compact vertical rhythm to maximize visible workspace content without sacrificing pointer accuracy. Controls balance compact visual height (`28px–36px`) with a strict minimum interactive hit target (`24px × 24px`).
3. **Zero Layout Shift Across State Transitions (`UIF-PRIN-003`):**
   * Hovering, focusing, selecting, validating, or disabling a widget MUST NOT alter its outer bounding box or shift adjacent elements. Validation caption slots, trailing input icon/action slots, leading selection indicators, and status bars reserve their geometry at rest.
4. **Visibility of System Status & Non-Collapsing Chrome (`UIF-PRIN-004`):**
   * Unavailable actions remain visible in a clearly distinguishable `Disabled` state rather than disappearing. Empty panels or filtered views display structured placeholder states rather than collapsing to zero dimensions.
5. **Dual-Modality Access (Pointer + Keyboard) & Redundant Cueing (`UIF-PRIN-005`):**
   * Every interactive operation reachable by pointer MUST have a deterministic keyboard path (`Tab`, arrow keys, `Enter`, `Space`, `Escape`).
   * Meaning is never communicated by color alone: selection, validation errors, dirty state, and special item types pair color tokens with shape, weight, border, or badge cues.

---

## 2. Foundation Design Tokens (`UIF-TOK-*`)

### 2.1 Spatial Grid & Spacing Scale (`UIF-TOK-001`)

All padding, gaps, margins, and alignment offsets adhere to a base-4px spatial grid (with a 2px half-step for dense inline micro-gaps):

| Token | Value (Logical px) | Semantic Role |
|---|---|---|
| `space.2xs` | `2` | Micro gap between anchor point and floating menu, sub-pixel scroll bottom tolerance, tight row gap |
| `space.xs` | `4` | Compact inner padding (tags, badges, inline code, menu card inset, separator vertical margin, tab close inset) |
| `space.sm` | `8` | Standard sibling control gap, toolbar button spacing, form row spacing, viewport clamp margin |
| `space.md` | `12` | Standard horizontal padding inside buttons, inputs, tabs, and menu rows |
| `space.lg` | `16` | Card and dialog inner padding, tree depth indentation step, section separation |
| `space.xl` | `24` | Major layout section spacing, empty-state container padding |
| `space.2xl` | `32` | Minimum viewport margin around large modal cards |

### 2.2 Sizing, Hit Targets & Geometry Tokens (`UIF-TOK-002`)

| Token | Value (Logical px) | Semantic Role |
|---|---|---|
| `size.hit_target.min` | `24 × 24` | Mandatory minimum pointer hit area for any interactive control (close `×`, clear `×`, resize grip, tree/TOC rows) |
| `size.bar.status` | `24` | Compact status and footer bar height (movable overlay footer, menu section heading row) |
| `size.control.sm` | `28` | Compact controls: navbar buttons, dropdown triggers, inline search action buttons, folder list rows |
| `size.control.md` | `32` | Standard controls: dialog CTA buttons, single-line text inputs, selectable menu rows, movable overlay title bar |
| `size.control.lg` | `36` | Primary structural bars: Top Navbar height, Tab Strip height, movable overlay filter bar |
| `size.indicator.dot` | `6` | Unsaved state dot (`●`) and status badge dot diameter |
| `size.splitter.grab` | `10` | Interactive pointer grab width centered over a `1px` pane split divider (`5px` each side) |
| `size.checkbox.box` | `16 × 16` | Visual glyph box for `Checkbox` and `Radio` controls (centered inside a `24 × 24` minimum hit target) |
| `size.switch.track` | `36 × 20` | Outer pill track dimensions for `ToggleSwitch` controls |
| `size.switch.thumb` | `14 × 14` | Inner sliding thumb diameter for `ToggleSwitch` controls (`3px` inset from track edges) |
| `size.slider.track_height` | `4` | Horizontal rail thickness for `Slider` controls |
| `size.slider.thumb` | `16 × 16` | Visual thumb diameter for `Slider` controls (`24 × 24` pointer hit target) |
| `size.progress.bar_height` | `4` | Linear `ProgressBar` track thickness |
| `size.spinner.sm` | `14` | Inline button/input busy `Spinner` diameter |
| `size.spinner.md` | `20` | Panel or dialog loading `Spinner` diameter |
| `size.icon.sm` | `14` | Inline status, badge, and close `×` glyph optical size |
| `size.icon.md` | `16` | Standard button, menu leading slot, resize grip glyph, and tree item icon optical size |
| `size.icon.lg` | `20` | Brand mark and empty-state illustration icon size |

### 2.3 Corner Radii & Stroke Weights (`UIF-TOK-003`)

| Token | Value (Logical px) | Semantic Role |
|---|---|---|
| `radius.window` | `8` | Outer application window frame radius |
| `radius.card` | `6` | Floating modal dialogs, input dialogs, movable overlay windows, and menu cards |
| `radius.widget` | `4` | Interactive controls: buttons, text inputs, dropdowns, checkboxes, code blocks, menu rows |
| `radius.pill` | `999` | Status chips and numeric count badges |
| `border.hairline` | `1` | Default structural dividers, card perimeters, resting input borders, inset menu focus outline, table borders |
| `border.focus` | `2` | Outer keyboard focus ring, active tab accent bar, active splitter bar, focused/invalid input border |

### 2.4 Typography Scale & Hierarchy (`UIF-TOK-004`)

No text anywhere in the interface may render smaller than `12px`.
* **Font Family Tokens:** `font.family.sans` (proportional UI chrome and document prose) and `font.family.mono` (code blocks, inline code, file paths, timestamps, editor buffer, and structured logs).
* **Font Weight Tokens:** `weight.regular` (`400`), `weight.medium` (`500`), `weight.semibold` (`600`), and `weight.bold` (`700`).
* **Authoritative Line Height:** The integer `Line Height (px)` column is authoritative for layout and row-height derivation; the parenthetical multiplier (`1.33×`, `1.38×`, etc.) is an informational 2-decimal approximation.

| Token | Size (px) | Line Height (px) | Allowed Weights | Tracking | Primary Usage |
|---|---|---|---|---|---|
| `type.caption` | `12` | `16` (`1.33×`) | Regular / Medium / SemiBold | `+0.01em` | Shortcut hints, validation error captions, status chips, menu section headings (`SemiBold`), timestamps (`mono`) |
| `type.ui_compact` | `13` | `18` (`1.38×`) | Regular / Medium / SemiBold / Bold | `0` | Dense UI lists: tree rows, TOC, tab labels (`SemiBold` when active), menu items, movable overlay title (`Bold`), log entries (`mono`) |
| `type.body` | `14` | `20` (`1.43×`) | Regular / Medium | `0` | Document prose, dialog prompt text, text input values, inline editor buffer (`mono`) |
| `type.title_sm` | `16` | `24` (`1.50×`) | SemiBold / Bold | `-0.01em` | Dialog titles, navbar brand title, panel section titles |
| `type.title_md` | `20` | `28` (`1.40×`) | Bold | `-0.015em` | Document H2 headings, About dialog header |
| `type.title_lg` | `24` | `32` (`1.33×`) | Bold | `-0.02em` | Document H1 headings |

### 2.5 Semantic Color System & Contrast Invariants (`UIF-TOK-005`)

The color system uses a dark-first elevation model where higher z-layers use progressively lighter neutral surfaces, reinforced by explicit perimeter borders.

[INFERENCE] Because WCAG 2.1 relative luminance requires background fills under white text (`L = 1.0000`) to have `L <= 0.1833` (for `>= 4.50:1` contrast) while foreground text and active borders on elevated dark surfaces (`surface.overlay` `L = 0.0093`, `surface.active` `L = 0.0235`) require `L >= 0.2167` (for `>= 4.50:1` text) or `L >= 0.1705` (for `>= 3.00:1` borders), **accent and danger tokens are split by polarity role** into background fills (`*.fill`, `*.fill_active`) and foreground/border tokens (`*.text`, `*.border`).

| Semantic Token | sRGB Value | Rel. Luminance (`L`) | Contrast vs `surface.base` | Contrast vs `surface.overlay` | Contrast vs `surface.active` | Contrast vs `text.strong` | Role & Usage Contract |
|---|---|---|---|---|---|---|---|
| `color.surface.sunken` | `RGB(6, 6, 8)` | `0.0019` | `1.02:1` | `1.14:1` | `1.42:1` | `20.24:1` | Deep input wells and scrollable log/code viewport backgrounds |
| `color.surface.base` | `RGB(9, 9, 11)` | `0.0028` | `1.00:1` | `1.12:1` | `1.39:1` | `19.90:1` | Root window canvas and primary document/tree pane background |
| `color.surface.raised` | `RGB(18, 18, 20)` | `0.0061` | `1.06:1` | `1.06:1` | `1.31:1` | `18.71:1` | Navbar, bottom bar, inactive tabs, resting `Secondary`/`Danger` buttons |
| `color.surface.code` | `RGB(20, 20, 22)` | `0.0071` | `1.08:1` | `1.04:1` | `1.29:1` | `18.40:1` | Fenced code blocks and resting text input background |
| `color.surface.overlay` | `RGB(24, 24, 27)` | `0.0093` | `1.12:1` | `1.00:1` | `1.24:1` | `17.72:1` | Floating surfaces: menus, input dialogs, modal dialogs, movable overlays, active tab |
| `color.surface.hover` | `RGB(32, 32, 36)` | `0.0147` | `1.23:1` | `1.09:1` | `1.14:1` | `16.24:1` | Interactive control hover fill, hovered menu item, active submenu parent |
| `color.surface.active` | `RGB(42, 42, 46)` | `0.0235` | `1.39:1` | `1.24:1` | `1.00:1` | `14.29:1` | Pressed quiet/secondary control fill, movable overlay title bar header |
| `color.surface.scrim` | `RGBA(9, 9, 11, 0.55)` | — | — | — | — | — | Dimmed backdrop behind blocking modal dialogs |
| `color.border.subtle` | `RGB(40, 40, 40)` | `0.0212` | `1.35:1` | `1.20:1` | `1.03:1` | `14.74:1` | Passive structural dividers: pane split lines at rest, menu separators, resting inputs |
| `color.border.strong` | `RGB(120, 120, 120)` | `0.1878` | `4.51:1` | `4.01:1` | `3.24:1` | `4.42:1` | Active/floating perimeters: cards, modals, menus, active buttons, markdown tables |
| `color.accent.fill` | `RGB(93, 96, 238)` | `0.1687` | `4.14:1` | `3.69:1` | `2.98:1` | **`4.80:1`** | Solid primary CTA button fill and selected tree-row background under `text.strong` |
| `color.accent.fill_active` | `RGB(82, 85, 222)` | `0.1356` | `3.52:1` | `3.13:1` | `2.52:1` | **`5.66:1`** | Pressed primary CTA button fill under `text.strong` |
| `color.accent.border` | `RGB(129, 140, 248)` | `0.3020` | `6.67:1` | **`5.94:1`** | **`4.79:1`** | `2.98:1` | Keyboard focus rings, active tab top indicator, pressed/focused accent borders |
| `color.accent.text` | `RGB(100, 200, 255)` | `0.5124` | `10.66:1` | **`9.49:1`** | **`7.66:1`** | `1.87:1` | Headings, active links, menu checkmarks (`✓`), submenu indicators (`>`) |
| `color.text.primary` | `RGB(210, 210, 210)` | `0.6445` | `13.16:1` | **`11.72:1`** | **`9.45:1`** | `1.51:1` | Default body and resting control label text |
| `color.text.strong` | `RGB(255, 255, 255)` | `1.0000` | `19.90:1` | **`17.72:1`** | **`14.29:1`** | `1.00:1` | Emphasized text, hovered/focused control labels, text on `*.fill` tokens |
| `color.text.muted` | `RGB(160, 160, 160)` | `0.3515` | `7.61:1` | **`6.78:1`** | **`5.47:1`** | `2.61:1` | Secondary descriptions, inactive tab labels, keyboard shortcut hints, section headings |
| `color.text.disabled` | `RGB(140, 140, 140)` | `0.2623` | `5.92:1` | `5.27:1` | `4.25:1` | `3.36:1` | Disabled button/menu/input text and empty placeholders (WCAG 1.4.3 exempt) |
| `color.status.success` | `RGB(100, 255, 100)` | `0.7515` | `15.19:1` | **`13.52:1`** | **`10.91:1`** | `1.31:1` | Completion states, positive status chips (`✓ parallel`) |
| `color.status.warning` | `RGB(220, 130, 0)` | `0.3118` | `6.86:1` | **`6.10:1`** | **`4.93:1`** | `2.90:1` | Unsaved indicator dot (`●`), warning icons (`⚠`), non-selected special item tint |
| `color.status.danger.text` | `RGB(255, 92, 92)` | `0.2969` | `6.57:1` | **`5.85:1`** | **`4.72:1`** | `3.03:1` | Error captions, error icons, invalid input borders, resting `Danger` button label/border |
| `color.status.danger.fill` | `RGB(216, 28, 20)` | `0.1548` | `3.88:1` | `3.46:1` | `2.79:1` | **`5.13:1`** | Hovered `Danger` button fill and hovered destructive menu row fill under `text.strong` |
| `color.status.danger.fill_active` | `RGB(202, 24, 16)` | `0.1309` | `3.43:1` | `3.08:1` | `2.46:1` | **`5.75:1`** | Pressed `Danger` button fill under `text.strong` |
| `color.editor.inverted.*` | BG: `RGB(255,255,255)`<br>FG: `RGB(0,0,0)`<br>Err: `RGB(180,0,0)` | `1.0000`<br>`0.0000`<br>`0.0999` | — | — | — | `21.00:1` (FG)<br>`7.00:1` (Err) | Dedicated high-contrast light surface for the inline document editor modal |

**Mandatory Contrast Rules (`UIF-TOK-006`):**
1. **Active Foreground Text (`< 18px`):** Every non-disabled text token (`text.primary`, `text.strong`, `text.muted`, `accent.text`, `status.success`, `status.warning`, `status.danger.text`) MUST maintain $\ge 4.50:1$ contrast against every surface it may appear on (`sunken` through `active`).
2. **Disabled Text Scope:** Per WCAG 2.1 SC 1.4.3, `color.text.disabled` (`RGB(140, 140, 140)`) is exempt from the `4.50:1` active-text floor and is constrained to the `[4.00:1, 6.00:1]` muted legibility band (`5.92:1` on `surface.base`, `4.25:1` on `surface.active`) so disabled controls remain readable while visually distinct from active controls. Disabled rows and controls on floating surfaces MUST never paint an opaque hover/active fill beneath `text.disabled`.
3. **Active UI Chrome, Borders & Focus Indicators:** Active control borders (`border.strong`, `accent.border`, `status.danger.text`) and solid control fills (`accent.fill`, `accent.fill_active`, `status.danger.fill`, `status.danger.fill_active`) MUST maintain $\ge 3.00:1$ contrast against their adjacent surface (`base` through `active` for borders; `base` through `overlay` for solid fills). Automated border/chrome evaluation classifies every stroke into one of four categories:
   * **`Judged`:** Active perimeter borders and focus rings — MUST measure $\ge 3.00:1$.
   * **`CoincidentWithFill`:** A `1px` perimeter stroke set to the exact same token as its solid button fill (`accent.fill`, `accent.fill_active`, `status.danger.fill_active`) — boundary contrast is carried by the solid fill against the surrounding surface ($\ge 3.08:1$).
   * **`UnpaintedBackground`:** A `transparent` stroke on a rest-state `Ghost` button — evaluated against the parent container surface when focused/hovered.
   * **`RestingDivider`:** Passive structural hairlines (`border.subtle`) — decorative and exempt from the `3.00:1` active-chrome threshold.
4. **Selection & Solid-Fill Foreground Lock:** Whenever `color.accent.fill`, `color.accent.fill_active`, `color.status.danger.fill`, or `color.status.danger.fill_active` is used as a background fill, the foreground text and icon color MUST lock to `color.text.strong` (`RGB(255, 255, 255)`, yielding `4.80:1` to `5.75:1`). Foreground-only status/accent tokens (`status.danger.text`, `status.warning`, `accent.text`) MUST NEVER render directly on `accent.fill` or `status.danger.fill`.

---

## 3. Atomic Widget State Matrices (`UIF-ATOM-*`)

### 3.0 Interaction State Model & Focus Orthogonality (`UIF-ATOM-000`)

1. **Two-Part Orthogonal State Resolution:**
   * Interactive state is resolved as `VisualState = { base: BaseState, focused: bool }` rather than a mutually exclusive flat enum, because keyboard focus can coexist with pointer hover, press, and validation error states.
   * **Base State Precedence:** `Disabled > Invalid > Pressed > Hovered > Rest`.
   * **Total Disabled Dominance:** When `Disabled` is active, `focused` is forced to `false` (disabled controls cannot hold focus or display a focus ring, and if an already-focused control becomes disabled, focus advances immediately to the next enabled control in scope).
   * Combining the 5 base states with `focused` yields **9 reachable states**: `Rest`, `Hovered`, `Pressed`, `Invalid`, `Rest + Focused`, `Hovered + Focused`, `Pressed + Focused`, `Invalid + Focused`, and `Disabled`.
2. **Adaptive Focus Ring & Outline Placement:**
   * **Outer Focus Ring (`2px`, Buttons & Inputs):** Rendered `2px` *outside* the control's outer border so it never clips internal content or overwrites an `Invalid` border.
   * **Inset Focus Outline (`1px`, Menu & Tree Rows):** Rendered `1px` *inside* the row perimeter so adjacent rows in dense lists do not clip the ring.
   * **Dynamic Ring Contrast Resolution:** When a focus ring or inset outline is evaluated against a saturated fill (such as a `Primary` button or a hovered/pressed `Danger` button or destructive menu row), the ring color resolves to `color.accent.border` if it achieves $\ge 3.00:1$ contrast against the adjacent surface/fill, and automatically switches to `color.text.strong` (`RGB(255, 255, 255)`) otherwise.
3. **Press-and-Release Activation:**
   * Pointer press inside an enabled control enters the `Pressed` visual state; the command fires **only** when the pointer is released while still inside the control's hit bounds. Releasing outside cancels activation.

#### 3.0.4 Data Binding, Controlled State & Responsive Callback Architecture (`UIF-ATOM-000b`)

[GENERALIZATION] Declarative desktop UI frameworks prevent main-thread stalls and state desynchronization by strictly separating **Controlled Domain Data Models**, **Ephemeral Interaction State**, and **Typed Lifecycle Callbacks**.

1. **Controlled vs. Ephemeral State Separation:**
   * **Controlled Data Model (`Model`):** Owned by application state and projected into the widget each frame (`value`, `checked`, `selected_ids`, `validation`, `busy`, `disabled`).
   * **Ephemeral Interaction State (`TransientState`):** Owned by the widget/framework layer (`VisualState`, hover timers, active drag anchor, IME composition buffer, scroll velocity, open flyout state) so pointer and keyboard feedback renders within `< 1 frame` without round-tripping through background subsystems.
2. **Three-Tier Callback Lifecycle (`Live` vs. `Commit` vs. `Cancel`):**
   * **`on_change(T)` (Live / Frame-Immediate):** Emitted on every keystroke, thumb scrub pixel, or hover highlight. Used for live validation or previewing; MUST never block or trigger synchronous disk/network I/O.
   * **`on_commit(T)` (Settled / Intentional):** Emitted **exactly once** when an interaction settles—`Enter` key, focus loss (`blur`), pointer release at the end of a slider/splitter/overlay drag (`on_drag_settle`), or dropdown option selection. Drives persistence, command dispatch, and expensive queries.
   * **`on_cancel()` (Revert):** Emitted on `Escape` or pointer release outside hit bounds, restoring the pre-gesture snapshot.
3. **Async & Long-Running Action State (`ActionPending`):**
   * Action controls (`Button`, `ToggleSwitch`, `MenuRow`, `Modal Primary CTA`) accept an optional `busy: bool` / `progress: Option<f32>` field in their data model. When `busy == true`, the control locks its outer bounding box (`UIF-PRIN-003`), renders an inline `14px` (`size.spinner.sm`) indeterminate spinner, suppresses duplicate `on_activate` emissions, and optionally exposes `on_cancel_request()`.
4. **Container-Query Responsive Adaptation (`ResponsiveBreakpoint`):**
   * Widgets adapt layout based on **allocated parent container width** rather than global window dimensions alone:
     * `Expanded` $\rightarrow$ `Compact`: Secondary labels collapse to icon-only with mandatory hover tooltips (`UIF-ATOM-007`), segmented controls compress horizontal padding from `space.md` (`12px`) to `space.sm` (`8px`), and secondary toolbar items move into an overflow `⋯` menu (`UIF-COMP-008`) with `0px` vertical layout shift.

### 3.1 Action Controls: Buttons, Segmented Controls, Split Buttons & Hyperlinks (`UIF-ATOM-001`)

Geometry: Height `size.control.sm` (`28px`, compact/navbar) or `size.control.md` (`32px`, dialogs/forms); horizontal padding `space.md` (`12px`); icon-to-label gap `space.sm` (`8px`); corner radius `radius.widget` (`4px`). Icon-only buttons enforce `min_width: 28px`, `min_height: 28px` and MUST display a hover tooltip.

| Variant | Rest (`base = Rest`) | Hovered (`base = Hovered`) | Pressed (`base = Pressed`) | Keyboard Focused (`focused = true` Overlay) | Disabled (`base = Disabled`) |
|---|---|---|---|---|---|
| **Primary** | BG: `accent.fill`<br>FG: `text.strong` (`4.80:1`)<br>Border: `1px accent.fill` | BG: `accent.fill`<br>FG: `text.strong` (`4.80:1`)<br>Border: `1px text.strong` (`4.80:1`) | BG: `accent.fill_active`<br>FG: `text.strong` (`5.66:1`)<br>Border: `1px accent.fill_active` | Base (`Rest`/`Hovered`/`Pressed`) + `2px text.strong` outer focus ring (`4.80:1`–`5.66:1` vs fill) | BG: `surface.raised`<br>FG: `text.disabled`<br>Border: `1px border.subtle` |
| **Secondary** | BG: `surface.raised`<br>FG: `text.primary` (`12.38:1`)<br>Border: `1px border.strong` (`4.24:1`) | BG: `surface.hover`<br>FG: `text.strong` (`16.24:1`)<br>Border: `1px border.strong` (`3.66:1`) | BG: `surface.active`<br>FG: `text.strong` (`14.29:1`)<br>Border: `1px accent.border` (`4.79:1`) | Base (`Rest`/`Hovered`/`Pressed`) + `2px accent.border` outer focus ring (`4.79:1`–`6.26:1`) | BG: `surface.base`<br>FG: `text.disabled`<br>Border: `1px border.subtle` |
| **Ghost / Quiet** | BG: `transparent`<br>FG: `text.primary`<br>Border: `1px transparent` | BG: `surface.hover`<br>FG: `text.strong` (`16.24:1`)<br>Border: `1px border.subtle` | BG: `surface.active`<br>FG: `text.strong` (`14.29:1`)<br>Border: `1px border.strong` (`3.24:1`) | Base (`Rest`/`Hovered`/`Pressed`) + `2px accent.border` outer focus ring | BG: `transparent`<br>FG: `text.disabled`<br>Border: `1px transparent` |
| **Danger** | BG: `surface.raised`<br>FG: `status.danger.text` (`6.18:1`)<br>Border: `1px status.danger.text` (`6.18:1`) | BG: `status.danger.fill`<br>FG: `text.strong` (`5.13:1`)<br>Border: `1px text.strong` (`5.13:1`) | BG: `status.danger.fill_active`<br>FG: `text.strong` (`5.75:1`)<br>Border: `1px status.danger.fill_active` | At `Rest`: + `2px accent.border` ring (`6.26:1`);<br>When `Hovered`/`Pressed`: + `2px text.strong` ring (`5.13:1`–`5.75:1`) | BG: `surface.base`<br>FG: `text.disabled`<br>Border: `1px border.subtle` |

* **`UIF-ATOM-001a` — Standard Button Data Model & Callbacks:**
  * **Data Model:** `ButtonModel { label: Option<String>, leading_icon: Option<Icon>, trailing_icon: Option<Icon>, variant: Primary | Secondary | Ghost | Danger, density: Compact | Standard, busy: bool, disabled: bool, tooltip: Option<String> }`.
  * **Callbacks:** `on_activate()` (emitted on pointer press-and-release inside hit bounds or `Enter`/`Space` when `disabled == false && busy == false`), `on_cancel_request()` (when `busy == true`).
* **`UIF-ATOM-001b` — `ToggleButton` & `SegmentedButton` (Single / Multi-Select):**
  * **Geometry & Visuals:** Joined horizontal strip (`height: 28px` or `32px`, outer radius `radius.widget = 4px`, `1px border.strong` perimeter). Unselected segments render as `Secondary (Rest)`; selected segments render with `BG: surface.active`, `FG: text.strong` (`14.29:1`), `Border: 1px accent.border` (`4.79:1`), and a reserved `14px` leading `✓` icon (`accent.text`, `7.66:1`) for non-color cueing (`UIF-PRIN-005`).
  * **Data Model:** `SegmentedModel<Id> { segments: Vec<SegmentItem<Id>>, selected: BTreeSet<Id>, mode: SingleSelect | MultiSelect, density: Compact | Standard, disabled: bool }`.
  * **Callbacks:** `on_selection_change(BTreeSet<Id>)` (`Left`/`Right` arrow keys rove focus across segments; `Space`/`Enter` toggles or selects).
* **`UIF-ATOM-001c` — `SplitButton` & `DropDownButton`:**
  * **Geometry & Visuals:** Two-zone control (`height: 28px` or `32px`) combining a primary action zone (`padding: [0, 12px]`) and a `28px`-wide trailing menu chevron zone (`▾`) separated by a `1px border.strong` vertical divider.
  * **Data Model:** `SplitButtonModel<ActionId> { label: String, leading_icon: Option<Icon>, variant: Primary | Secondary, menu_rows: Vec<MenuRow<ActionId>>, menu_open: bool, busy: bool, disabled: bool }`.
  * **Callbacks:** `on_primary_activate()` (`Enter`/`Space` on primary zone), `on_menu_toggle(bool)` (chevron click or `Alt+Down`/`Down`), `on_menu_select(ActionId)` (mounts flyout in `Z-Band 2`).
* **`UIF-ATOM-001d` — `Hyperlink` Control:**
  * **Geometry & Visuals:** Inline or standalone interactive text control (`min_height: 24px` hit target, `radius.widget = 4px` focus ring). Rest: `FG: accent.text` (`10.66:1` on `base`, `9.49:1` on `overlay`) with `1px` underline; Hovered: `FG: text.strong` with `surface.hover` pill fill; Pressed: `FG: accent.text` on `surface.active`; Disabled: `FG: text.disabled` without underline.
  * **Data Model:** `HyperlinkModel { label: String, uri: String, external: bool, disabled: bool }`.
  * **Callbacks:** `on_open(String)`, `on_hover_uri(Option<String>)` (surfaces target URI preview in status bar).

### 3.2 Text Inputs, Search Fields, AutoSuggest & Multi-Line Editors (`UIF-ATOM-002`)

Geometry: Height `size.control.md` (`32px`, or `28px` in compact toolbars); horizontal padding `space.md` (`12px`); radius `radius.widget` (`4px`).
* **Zero-Shift Trailing Slot Partition (`UIF-ATOM-002b`):** Text inputs, dropdown triggers, and search fields partition their inner width into a text-area rectangle and a fixed trailing slot (`24×24px` for inputs/dropdowns; `28×28px` for search fields; slot priority `invalid > clear > idle`) derived purely from outer control bounds. Changing or hiding the trailing glyph (`🔍` $\leftrightarrow$ `×` $\leftrightarrow$ `⚠`) MUST NOT shift or resize the text layout rectangle by even `1px`.

| State | Background | Text / Placeholder / Icon | Border & Focus Ring |
|---|---|---|---|
| **Rest** | `surface.code` | Text: `text.primary` (`12.17:1`) · Placeholder: `text.disabled` · Icon: `text.muted` (`7.04:1`) | Border: `1px solid border.subtle` |
| **Hovered / Pressed** | `surface.code` | Text: `text.primary` · Placeholder: `text.muted` · Icon: `text.primary` | Border: `1px solid border.strong` (`4.17:1`) |
| **Focused (Valid)** | `surface.base` | Text: `text.strong` (`19.90:1`) · Placeholder: `text.muted` · Icon: `text.primary` · Selection: `accent.fill` | Border: `2px solid accent.border` (`6.67:1`) |
| **Invalid (Unfocused)** | `surface.base` | Text: `text.strong` (`19.90:1`) · Placeholder: `text.muted` · Trailing icon: `status.danger.text` (`6.57:1`) | Border: `2px solid status.danger.text` (`6.57:1`) |
| **Invalid + Focused** | `surface.base` | Text: `text.strong` (`19.90:1`) · Placeholder: `text.muted` · Trailing icon: `status.danger.text` (`6.57:1`) | Border: `2px solid status.danger.text` (`6.57:1`) + Outer Ring: `2px accent.border` (`6.67:1`) |
| **Disabled** | `surface.raised` | Text: `text.disabled` · Placeholder: `text.disabled` · Icon: `text.disabled` | Border: `1px solid border.subtle` |

* **`UIF-ATOM-002a` — Single-Line Text Input & Search Field Data Model & Callbacks:**
  * **Data Model:** `TextInputModel { value: String, placeholder: String, selection: Option<TextRange>, validation: Option<String>, read_only: bool, disabled: bool, clearable: bool }`.
  * **Callbacks:** `on_change(String)` (every edit/keystroke), `on_commit(String)` (`Enter` or focus loss), `on_clear()` (clicking trailing `×` or pressing `Escape` when non-empty), `on_focus_change(bool)`.
* **`UIF-ATOM-002c` — `PasswordInput` (Secret Entry):**
  * **Anatomy & Security Contract:** Masks characters with `•` (`U+2022`) by default, disables clipboard `Copy`/`Cut` while masked, and reserves a `24×24px` trailing reveal toggle button (`👁` / `👁‍🗨`).
  * **Data Model:** `PasswordInputModel { masked_len: usize, revealed_value: Option<String>, validation: Option<String>, disabled: bool }`.
  * **Callbacks:** `on_change(SecretInput)`, `on_commit()`, `on_reveal_toggle(bool)` (automatically resets `revealed_value` to masked on focus loss).
* **`UIF-ATOM-002d` — `AutoSuggestBox` (Search & Command Completion):**
  * **Anatomy:** Combines a `TextInput` with a `Z-Band 2` completion flyout (`max_height: 240px`, `32px` rows, anchored `4px` below input).
  * **Data Model:** `AutoSuggestModel<Id> { query: String, suggestions: Vec<SuggestionItem<Id>>, highlighted_index: Option<usize>, busy: bool, disabled: bool }`.
  * **Callbacks:** `on_query_change(String)`, `on_suggestion_highlight(usize)` (`Down`/`Up` wraps through suggestions without losing input caret), `on_suggestion_commit(Id, CommitSource::{Enter, Click, Tab})`, `on_dismiss()` (`Escape` closes flyout first; second `Escape` clears query per `Escape Rank`).
* **`UIF-ATOM-002e` — `MultiLineTextArea`:**
  * **Anatomy:** Scrollable multi-line text well (`min_height: 72px` / 3 lines of `type.body`, padding `space.sm = 8px` vertical, `space.md = 12px` horizontal, `font.family.sans` for prompts or `font.family.mono` for code/markdown). Supports auto-grow between `min_rows` and `max_rows` (such as the Bottom Command Bar `44px..120px` prompt box).
  * **Data Model:** `TextAreaModel { text: String, cursor: LineCol, selection: Option<TextRange>, min_rows: u16, max_rows: Option<u16>, dirty: bool, validation: Option<ParseDiagnostic>, disabled: bool }`.
  * **Callbacks:** `on_change(String)`, `on_cursor_change(LineCol)`, `on_submit(String)` (`Ctrl+Enter` / `Cmd+Enter`), `on_undo()`, `on_redo()`.

### 3.3 Boolean & Choice Controls: Checkbox, Radio & ToggleSwitch (`UIF-ATOM-003`)

Geometry: All boolean and choice controls enforce a minimum `24px × 24px` (`size.hit_target.min`) pointer hit target extending across their inline label (`space.sm = 8px` gap between indicator and `13px`/`14px` label).
* **`Checkbox`:** `16×16px` (`size.checkbox.box`) visual box, `radius.widget` (`4px`), supporting binary (`Unchecked`, `Checked`) and tri-state (`Indeterminate` `−` dash) selection.
* **`Radio`:** `16×16px` (`size.checkbox.box`) circular indicator (`radius.pill = 999px`) with an `8×8px` centered inner dot (`color.text.strong`) when selected.
* **`ToggleSwitch`:** `36×20px` (`size.switch.track`) pill track (`radius.pill`) with a `14×14px` (`size.switch.thumb`) sliding thumb (`3px` inset).

| Control & Polarity | Rest (`base = Rest`) | Hovered (`base = Hovered`) | Pressed (`base = Pressed`) | Keyboard Focused (`focused = true`) | Disabled (`base = Disabled`) |
|---|---|---|---|---|---|
| **Unchecked / Off** (`Checkbox`, `Radio`, `Switch`) | BG: `surface.code`<br>Border: `1px border.strong` (`4.17:1`)<br>Switch thumb: `text.muted` (`7.04:1`)<br>Label: `text.primary` | BG: `surface.hover`<br>Border: `1px accent.border` (`5.35:1`)<br>Switch thumb: `text.strong` (`16.24:1`)<br>Label: `text.strong` | BG: `surface.active`<br>Border: `1px accent.border` (`4.79:1`)<br>Switch thumb: `text.strong` (`14.29:1`)<br>Label: `text.strong` | Base + `2px accent.border` outer focus ring (`4.79:1`–`6.67:1`) | BG: `surface.raised`<br>Border: `1px border.subtle`<br>Thumb/Label: `text.disabled` |
| **Checked / Indeterminate / On** (`Checkbox`, `Radio`, `Switch`) | BG: `accent.fill`<br>Border: `1px accent.fill` (`CoincidentWithFill`)<br>Glyph/Thumb: `text.strong` (`4.80:1`)<br>Label: `text.primary` | BG: `accent.fill`<br>Border: `1px text.strong` (`4.80:1`)<br>Glyph/Thumb: `text.strong` (`4.80:1`)<br>Label: `text.strong` | BG: `accent.fill_active`<br>Border: `1px accent.fill_active` (`CoincidentWithFill`)<br>Glyph/Thumb: `text.strong` (`5.66:1`)<br>Label: `text.strong` | Base + `2px text.strong` outer focus ring (`4.80:1`–`5.66:1` vs fill) | BG: `surface.raised`<br>Border: `1px border.subtle`<br>Glyph/Thumb/Label: `text.disabled` |

* **Data Models & Callbacks:**
  * **`CheckboxModel`:** `{ state: Unchecked | Checked | Indeterminate, label: String, description: Option<String>, disabled: bool }` $\rightarrow$ **`on_toggle(CheckState)`** (`Space` or click; clicking an `Indeterminate` parent checkbox transitions deterministically to `Checked`, or `Unchecked` if all enabled children are already checked).
  * **`RadioGroupModel<Id>`:** `{ options: Vec<RadioOption<Id>>, selected: Option<Id>, orientation: Vertical | HorizontalWrap, disabled: bool }` $\rightarrow$ **`on_select(Id)`** (`Up`/`Down`/`Left`/`Right` roves focus and selects within the group; re-clicking the already-selected option is a no-op).
  * **`SwitchModel`:** `{ on: bool, label: String, on_text: Option<String>, off_text: Option<String>, busy: bool, disabled: bool }` $\rightarrow$ **`on_toggle(bool)`** (`Space` or click; immediate live toggle).

### 3.4 Selection Pickers: `ComboBox` / `Select` (`UIF-ATOM-004`)

* **Geometry & Visuals:** Trigger height `size.control.sm` (`28px`, toolbars) or `size.control.md` (`32px`, forms/dialogs); horizontal padding `space.md` (`12px`); trailing `24×24px` slot containing a `14px` `▾` chevron (`text.muted` at rest, `text.strong` on hover/open). Trigger states follow `UIF-ATOM-002`.
* **Flyout Surface:** Opening the `ComboBox` mounts a `Z-Band 2` (`Escape Rank 3`) menu card (`UIF-FMENU-001`) anchored `4px` (`space.xs`) below (or flipped above per `UIF-CMENU-001`) with `min_width = trigger.width`, `max_height = 280px`, and a `✓` (`accent.text`) in the `16×16px` leading slot of the selected row.
* **Data Model:** `ComboBoxModel<Id> { items: Vec<SelectOption<Id>>, selected: Option<Id>, placeholder: String, filter_query: Option<String>, open: bool, validation: Option<String>, disabled: bool }`.
* **Callbacks:** `on_open_change(bool)`, `on_filter_change(String)` (when searchable), `on_commit(Id)` (`Enter`, `Space`, or click on an option row closes the popup, commits `Id`, and restores focus to the trigger).

### 3.5 Continuous & Stepped Range Controls: `Slider` & `NumberBox` (`UIF-ATOM-005`)

* **`Slider` Anatomy & Visual States:**
  * Track height `size.slider.track_height` (`4px`, `radius.pill`); unfilled rail `surface.active` with `1px border.subtle`; filled active rail `accent.fill` (`3.69:1` vs `overlay`, `4.14:1` vs `base`); thumb `16×16px` (`size.slider.thumb`, `24×24px` hit target) in `text.strong` with `2px accent.border` border (`Hovered`/`Pressed` thumb scales optical ring to `2px text.strong` on `accent.fill`). Optional stepped tick marks (`2×4px` in `border.strong`).
* **`NumberBox` Anatomy:**
  * `32px`-high numeric text input (`font.family.mono`, `14px`) with a trailing `28×32px` dual stepper (`▲` / `▼`, each `28×16px` compact button or side-by-side `24×24px` `[−]` `[+]` buttons).
* **Data Model:** `RangeModel { value: f64, min: f64, max: f64, step: Option<f64>, precision: u8, unit_suffix: Option<String>, validation: Option<String>, disabled: bool }`.
* **Callbacks:**
  * **`on_change(f64)` (Live Scrub):** Emitted continuously during pointer thumb drag or stepper hold.
  * **`on_commit(f64)` (Settled):** Emitted **once** on pointer release, stepper click release, `Enter`, focus loss, or keyboard step (`Left`/`Down` = `-step`, `Right`/`Up` = `+step`, `PageDown`/`PageUp` = `±10 × step`, `Home` = `min`, `End` = `max`).
  * **`on_cancel()`:** Emitted on `Escape` during an active drag, reverting `value` to the pre-drag value.

### 3.6 Interactive Chips & Filter Pills (`UIF-ATOM-006`)

* **Geometry:** Height `24px` (`size.bar.status`, compact inline) or `28px` (`size.control.sm`, interactive filter bar); horizontal padding `space.sm` (`8px`); gap `space.xs` (`4px`); radius `radius.pill` (`999px`) for status/count pills or `radius.widget` (`4px`) for filter/input chips. Removable chips reserve a `24×24px` trailing `×` hit target.
* **Visual States:**
  * **Unselected (`Assist` / `Filter` / `Input`):** `BG: surface.raised`, `FG: text.primary` (`12.38:1`), `Border: 1px border.strong` (`4.24:1`). Hover: `BG: surface.hover`, `FG: text.strong`.
  * **Selected (`Filter`):** `BG: accent.fill`, `FG: text.strong` (`4.80:1`), `Border: 1px accent.border`, plus a leading `14px` `✓` glyph (`text.strong`).
* **Data Model:** `ChipModel<Id> { id: Id, label: String, variant: Assist | Filter { selected: bool } | Input { removable: bool }, leading_icon: Option<Icon>, disabled: bool }`.
* **Callbacks:** `on_activate(Id)`, `on_toggle(Id, bool)` (`Space`/`Enter`/click), `on_remove(Id)` (trailing `×` click or `Delete`/`Backspace` when focused).

### 3.7 Tooltips & Teaching Popovers (`UIF-ATOM-007`)

* **`Tooltip` (Non-Interactive Hover/Focus Hint):**
  * **Geometry & Visuals:** Max width `280px`; padding `4px 8px` (`space.xs`, `space.sm`); `BG: surface.overlay`; `Border: 1px border.strong` (`4.01:1`); `Radius: radius.widget` (`4px`); text `12px Regular` (`type.caption`, `text.primary` `11.72:1`) with optional trailing shortcut pill (`font.family.mono`, `text.muted` `6.78:1`).
  * **Behavioral Contract:** Opens after `400ms` hover delay or `0ms` keyboard focus delay, offset `4px` (`space.xs`) from the trigger rect (flipped into viewport via `UIF-CMENU-001`). Pointer-transparent (`HitTest::Ignore`); dismisses immediately on pointer leave, any key press, or `Escape`.
* **`TeachingPopover` (Interactive Callout / Flyout):**
  * **Geometry & Visuals:** Width `280px..360px`; padding `12px` (`space.md`); `BG: surface.overlay`; `Border: 1px accent.border` (`5.94:1`); `Radius: radius.card` (`6px`). Contains title (`13px SemiBold`, `text.strong`), body (`13px Regular`, `text.primary`), and optional footer action/dismiss buttons. Mounts in `Z-Band 2` (`Escape Rank 3`).
* **Data Model & Callbacks:** `PopoverModel<ActionId> { title: String, body: String, primary_action: Option<(String, ActionId)>, placement: PreferredSide, open: bool }` $\rightarrow$ `on_action(ActionId)`, `on_dismiss()`.

### 3.8 Badges & Status Indicators (`UIF-ATOM-008`)

* **Variants & Polarity Rules (per `UIF-TOK-006`):**
  1. **Dot Badge (`size.indicator.dot = 6×6px`, `radius.pill`):** `accent.border` (unread/active), `status.warning` (unsaved/modified `●`), `status.success` (online/ready), `status.danger.text` (error).
  2. **Numeric Count Pill (`height: 18px`, `min_width: 18px`, padding `[0, 4px]`, `radius.pill`, `12px SemiBold`):** Counts $> 99$ format as `99+`. Solid emphasis uses `BG: accent.fill` or `status.danger.fill` with `FG: text.strong` (`4.80:1` / `5.13:1`); subtle count uses `BG: surface.active` with `FG: text.strong` (`14.29:1`) and `1px border.subtle`.
  3. **Semantic Status Chip (`height: 24px`, padding `[0, 8px]`, `radius.pill`, `12px Medium`):** Pairs a mandatory leading glyph (`✓`, `⚠`, `✕`, `●`) with text on `BG: surface.raised` + `1px` border (`status.success` `13.52:1`, `status.warning` `6.10:1`, `status.danger.text` `6.18:1`, `accent.text` `9.96:1`).
* **Data Model:** `BadgeModel { kind: Dot | Count(u32) | Status { icon: Icon, label: String }, tone: Neutral | Accent | Success | Warning | Danger }`.

### 3.9 Progress Indicators: `ProgressBar` & `Spinner` (`UIF-ATOM-009`)

* **`ProgressBar` (Linear):**
  * **Geometry & Visuals:** Track height `size.progress.bar_height` (`4px`, `radius.pill`), `BG: surface.active`. Fill bar uses `accent.border` (`4.79:1` vs `surface.active`) when `Active`, `status.warning` (`4.93:1`) when `Paused`, and `status.danger.text` (`4.72:1`) when `Error`. Indeterminate mode animates a `35%`-width sliding segment (or renders a static striped segment when OS `reduced_motion` is enabled).
* **`Spinner` (Circular Indeterminate / Determinate Ring):**
  * **Geometry & Visuals:** Diameter `size.spinner.sm` (`14px`, `2px` stroke, inline inside buttons/inputs/navbar) or `size.spinner.md` (`20px`, `2px` stroke, panel/dialog loading); track `border.subtle`, active arc `accent.text` (`9.49:1` on `overlay`) or `text.strong` when inside a solid `Primary`/`Danger` button.
* **Data Model & Callbacks:** `ProgressModel { mode: Indeterminate | Determinate { completed: u64, total: u64 }, status: Active | Paused | Error, label: Option<String>, cancellable: bool }` $\rightarrow$ `on_cancel()` (via trailing `24×24px` `[×]` stop button when `cancellable == true`).

---

## 3B. Reusable Composite Widget Patterns (`UIF-COMP-*`)

Composite widgets combine atomic controls (`§3`) into standardized layout, data-binding, and keyboard-navigation patterns shared across the nine application surfaces (`§4`).

### 3B.1 `FormField` & Settings Row Pattern (`UIF-COMP-001`)

* **Stacked Form Layout (Dialogs & Forms):**
  * Header row (`height: 20px`): Field label (`13px Medium`, `text.primary`) + optional required indicator (`*` in `status.danger.text` + tooltip `"Required"`) + optional right-aligned helper hint (`12px Regular`, `text.muted`).
  * Gap `space.xs` (`4px`) $\rightarrow$ Control slot (`32px`) $\rightarrow$ Gap `space.xs` (`4px`) $\rightarrow$ Reserved caption/validation slot (`height: 16px`, `type.caption`: renders `helper_text` in `text.muted` when valid, or `validation_error` in `status.danger.text` with leading `⚠` glyph when invalid). Total vertical rhythm is fixed so validation transitions cause `0px` layout shift (`UIF-PRIN-003`).
* **Horizontal Settings Card Row (`SettingsRow`):**
  * Min height `48px`, padding `12px 16px`, `BG: surface.raised`, `Border: 1px border.subtle`, `Radius: radius.widget` (`4px`). Left column (`Fill`): title (`14px Medium`, `text.strong`) + description (`12px Regular`, `text.muted`). Right column (`Shrink`, min gap `16px`): interactive control (`ToggleSwitch`, `ComboBox`, `NumberBox`, or `Button`).
* **Data Model:** `FormFieldModel<ControlModel> { id: FieldId, label: String, required: bool, helper_text: Option<String>, error_text: Option<String>, layout: Stacked | HorizontalSplit, control: ControlModel }`.

### 3B.2 `InfoBar` (Inline Banner) & `Toast` / `SnackBar` (Transient Notification) (`UIF-COMP-002`)

* **Geometry & Visuals:**
  * **`InfoBar` (Docked Inline inside Modals or Panes):** Full container width, min height `36px`, padding `8px 12px`, `BG: surface.raised` (or `surface.overlay`), `Radius: radius.widget` (`4px`), `1px` perimeter border in `border.strong`, plus a `4px` solid left accent stripe and `16px` leading icon colored by severity (`accent.text` for `Info`, `status.success` for `Success`, `status.warning` for `Warning`, `status.danger.text` for `Danger`).
  * **`Toast` / `SnackBar` (Floating in `Z-Band 1`):** Width `360px` (`size.toast.width`, clamped to `viewport.width - 32px`), anchored bottom-right (`16px` from right edge, `16px` above Bottom Command Bar), stacking upward with `8px` (`space.sm`) gap (max 3 visible).
* **Usability & Data-Safety Contract:**
  * `Info` and `Success` toasts may auto-dismiss after `5000ms`; hovering or focusing inside a toast pauses its countdown timer.
  * `Warning` and `Danger` notifications MUST NEVER auto-dismiss (`auto_dismiss_ms = None`); they require explicit user acknowledgment (`[Dismiss ×]` or action button).
* **Data Model & Callbacks:** `NotificationModel<ActionId> { id: NotificationId, severity: Info | Success | Warning | Danger, title: String, message: String, action: Option<(String, ActionId)>, dismissible: bool, auto_dismiss_ms: Option<u32> }` $\rightarrow$ `on_action(NotificationId, ActionId)`, `on_dismiss(NotificationId, DismissReason)`.

### 3B.3 `Expander` / Collapsible Disclosure Container (`UIF-COMP-003`)

* **Anatomy:** Header bar (`height: 32px` standard or `28px` compact inside tree/JSON views, padding `[0, 12px]`, `BG: surface.raised`, `Border: 1px border.subtle`, `Radius: radius.widget = 4px`) with a `16×16px` leading chevron (`▸` collapsed / `▾` expanded), title (`13px SemiBold`, `text.primary`), optional summary badge (`BadgeModel`), and optional trailing action button. When `expanded == true`, the body container (`BG: surface.base` or `surface.sunken`, padding `12px`, `1px border.subtle` bottom/side border) mounts immediately below.
* **Data Model & Callbacks:** `ExpanderModel { id: SectionId, title: String, badge: Option<BadgeModel>, expanded: bool, disabled: bool }` $\rightarrow$ `on_toggle(SectionId, bool)` (`Enter`/`Space` or click toggles; `Right` arrow expands; `Left` arrow collapses).

### 3B.4 `ScrollView` & Annotated Scrollbar Contract (`UIF-COMP-004`)

* **Geometry & Visuals:** Scrollbar track width `8px` (`space.sm`) at rest, expanding to `10px` (`size.splitter.grab`) on track hover/drag; thumb `radius.pill`, min length `24px` (`size.hit_target.min`), colored `border.strong` at rest/hover (`4.01:1`–`4.51:1`) and `accent.border` while dragging. Optional `2px` (`space.2xs`) tick annotations along the vertical track mark search hits, headings, or diagnostic errors.
* **Data Model:** `ScrollViewModel { content_size: Size, viewport_size: Size, offset: Point, follow_tail: bool, annotations: Vec<ScrollTick> }`.
* **Callbacks:**
  * **`on_scroll(ScrollMetrics)`:** Emitted on wheel, trackpad pan, scrollbar drag, or keyboard scroll (`Up`/`Down`/`PageUp`/`PageDown`/`Home`/`End`), including `at_bottom_within_2px: bool` for `UIF-MOV-004` stream auto-scroll.
  * **`on_reach_end()`:** Emitted when scroll offset enters the final `64px` of content to trigger non-blocking lazy pagination.

### 3B.5 `Card` & `ListTile` (`UIF-COMP-005`)

* **`Card` Container:** Grouping surface (`BG: surface.raised`, `Border: 1px border.subtle` at rest or `1px border.strong` when interactive/elevated, `Radius: radius.card = 6px`, inner padding `space.lg = 16px`).
* **`ListTile` (3-Slot Row):** Single-line (`height: 32px`, `size.control.md`) or two-line (`height: 44px`, padding `[4px, 12px]`, gap `8px`):
  * **Leading Slot (`16×16px` or `24×24px`):** Icon, `Checkbox`, or `Radio`.
  * **Center Slot (`Fill`):** Primary title (`13px Regular`/`Medium`, `text.primary`, single-line truncated with `…`) + optional subtitle (`12px Regular`, `text.muted`).
  * **Trailing Slot (`Shrink`):** Metadata badge, shortcut hint, or action icon buttons (`24×24px` hit targets).
* **Data Model & Callbacks:** `ListTileModel<Id> { id: Id, title: String, subtitle: Option<String>, leading: Option<LeadingSlot>, trailing: Option<TrailingSlot>, selected: bool, disabled: bool }` $\rightarrow$ `on_select(Id)`, `on_activate(Id)`, `on_context_menu(Id, Point)`.

### 3B.6 `VirtualList` & `TreeView` (`UIF-COMP-006`)

* **Geometry & Visuals:** Virtualized row viewport rendering only visible rows (`height: 24px` compact or `28px` standard, `indentation: depth × 16px` / `space.lg`). Expandable folder/parent nodes reserve a `24×24px` leading disclosure chevron (`▸` / `▾`). Selection visual states obey `UIF-PANE-003` (`BG: accent.fill`, `FG: text.strong` `4.80:1`, `3px accent.border` left bar).
* **Data Model:**
  * `TreeNodeModel<Id> { id: Id, parent_id: Option<Id>, depth: u16, label: String, icon: Icon, expandable: bool, expanded: bool, special_warning: bool, disabled: bool }`.
  * `TreeViewModel<Id> { visible_nodes: Vec<TreeNodeModel<Id>>, selected_ids: BTreeSet<Id>, anchor_id: Option<Id>, focused_id: Option<Id>, selection_mode: Single | ExtendedMulti, filter_query: String }`.
* **Responsive Callbacks & Keyboard Choreography:**
  * **`on_selection_change(BTreeSet<Id>)`:** `Click` selects single; `Ctrl+Click` (or `Space`) toggles item in `ExtendedMulti`; `Shift+Click` (or `Shift+Up`/`Shift+Down`) selects contiguous range from `anchor_id`.
  * **`on_activate(Id)`:** Emitted on double-click or `Enter` on focused row.
  * **`on_expand_toggle(Id, bool)`:** Chevron click, `Right` arrow (expands collapsed node, or moves focus to first child if already expanded), `Left` arrow (collapses expanded node, or moves focus to parent node if already collapsed).
  * **`on_context_menu(BTreeSet<Id>, Point)`:** Right-click or `Shift+F10` / `ContextMenu` key (if right-clicked row is outside `selected_ids`, selection resets to that single row before opening the `Z-Band 2` menu; if inside `selected_ids`, preserves multi-selection).

### 3B.7 `DataTable` / Structured Grid (`UIF-COMP-007`)

* **Geometry & Visuals:**
  * **Sticky Header Row (`height: 32px`, `BG: surface.raised`, bottom border `1px border.strong`):** Column titles (`12px SemiBold`, `text.strong`) with optional sort indicator (`▲` / `▼` in `accent.text`) and a `10px` (`size.splitter.grab`) column-resize handle centered on each `1px border.subtle` vertical column divider.
  * **Body Rows (`min_height: 32px`, padding `[4px, 12px]`, bottom border `1px border.subtle`):** Alternating or uniform `surface.base` / `surface.raised` rows; `Hovered` row uses `surface.hover`; `Selected` row uses `surface.active` + `1px accent.border`.
* **Data Model:** `DataTableModel<RowId, ColId> { columns: Vec<ColumnDef<ColId>>, rows: Vec<TableRow<RowId>>, sort: Option<(ColId, SortDirection)>, selected_rows: BTreeSet<RowId>, focused_cell: Option<(RowId, ColId)> }`.
* **Callbacks:**
  * **`on_sort_change(ColId, SortDirection)`:** Clicking a sortable header or pressing `Enter`/`Space` when header cell is focused cycles `Ascending -> Descending -> None`.
  * **`on_column_resize_change(ColId, f32)`** (live drag) and **`on_column_resize_commit(ColId, f32)`** (settle on release, respecting `col.min_width`).
  * **`on_row_select(BTreeSet<RowId>)`**, **`on_cell_action(RowId, ColId, CellAction)`** (Arrow keys navigate across cells; `Tab` traverses interactive controls embedded inside cells).

### 3B.8 `BreadcrumbBar` & Responsive Overflow Toolbar (`UIF-COMP-008`)

* **`BreadcrumbBar`:**
  * **Geometry & Responsive Elision:** Horizontal path bar (`height: 28px`, gap `4px`, separator `/` or `›` in `text.muted`). Each segment is a `Ghost` button (`height: 24px`, padding `[0, 6px]`, `13px Regular text.muted` for ancestors, `13px SemiBold text.strong` for the terminal active item). When total segment width exceeds allocated container width, leading/middle ancestor segments collapse into a single `[…]` (`28×24px`) trigger button that opens a `Z-Band 2` dropdown menu of elided ancestors while keeping the root and terminal segments visible.
  * **Data Model & Callbacks:** `BreadcrumbModel<Id> { segments: Vec<BreadcrumbSegment<Id>>, elided_Popup_open: bool }` $\rightarrow$ `on_segment_activate(Id)`, `on_overflow_toggle(bool)`.
* **`OverflowToolbar` (`CommandBar` Pattern):**
  * **Container-Query Overflow:** Primary action controls measure against available toolbar width; any secondary control whose right edge would exceed `available_width - 36px` moves deterministically into a trailing `[⋯]` (`28×28px`) overflow menu button (`Z-Band 2`) without wrapping to a second line or shifting toolbar height.

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

#### 4.1.2 Usability & Behavioral Contract
* **`UIF-WIN-001` — Responsive Viewport & Persisted Geometry:** Window dimensions, screen position, split pane ratios, and user UI scale multiplier (`0.75×` to `2.00×` in `0.10×` steps, composed with OS DPI scale) persist across sessions. Geometry persistence writes occur only when a gesture settles (on drag/resize release or window close, never per motion frame). Resizing below `640 × 480` logical pixels clamps internal pane minimums so the central workspace never collapses below `240px` width.
* **`UIF-WIN-002` — Unified Root Coordinate Space:** All floating overlays (context menus, fixed menus, cursor-anchored input dialogs, and movable utility windows) resolve coordinates within a single root viewport coordinate system (`Rectangle { x, y, width, height }`) originating at `(0, 0)` top-left of the Main Window content area.
* **`UIF-WIN-003` — Global Escape Unwinding (`Escape Rank`):** Pressing `Escape` dismisses ephemeral and floating layers in strict **`Escape Rank` priority** (which intentionally unwinds transient menus before blocking dialogs, independent of visual z-band elevation):
  1. **Rank 3 — Menu Cascade (`Band 2`):** Deepest open submenu branch $\rightarrow$ parent menu level $\rightarrow$ open root menu (Context Menu or Fixed Navbar Menu).
  2. **Rank 2 — Blocking Dialogs (`Band 3`):** Topmost open Input Dialog or Modal Dialog (subject to the dirty-state guard `UIF-MOD-002`).
  3. **Rank 1 — Focused Utility Overlay (`Band 1`):** Topmost Movable Overlay Dialog, **only** if keyboard focus currently resides inside that overlay card.
  4. **Rank 0 — Workspace Filter / Prompt (`Band 0`):** Active search filter or in-flight prompt focus when no higher-rank surface consumes the key.

---

### 4.2 Top Navbar (`UIF-NAV-*`)

#### 4.2.1 Anatomy & Slot Layout
```text
┌─ Top Navbar (Height: 36px, Padding: [4px, 12px], BG: surface.raised, Bottom Border: 1px border.subtle) ─┐
│ [Brand Icon 20px] [App Title 16px Bold] │ [Status Pill] │ <── Flex Fill ──> │ [Filter Dropdown] [Actions…] [☰] │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

| Zone | Alignment | Sizing & Spacing | Usability & Visual Rules |
|---|---|---|---|
| **Left: Brand & Identity** | Left, Center-Y | `Shrink`, gap `8px` | `20×20px` brand mark + `16px` bold application title (`text.strong`), followed by a `1px × 20px` vertical `border.subtle` divider. |
| **Left-Center: Live System Status** | Left, Center-Y | `Shrink`, gap `6px` | Compact status indicator: shows animated/ellipsis progress in `accent.text` while background indexing or tasks run, transitioning to a `status.success` (`✓`) summary when idle. |
| **Center: Elastic Spacer** | Fill | Min width `16px` | Separates status information from interactive workspace controls. |
| **Right: Workspace Controls** | Right, Center-Y | Height `28px`, gap `8px` | Filter dropdown (`min-width: 140px`, `max-width: 220px`, truncated with `…`), followed by secondary dialog trigger buttons (`Batch...`, `Tools...`) and the far-right Fixed Menu button (`☰`, `28×28px`). |

#### 4.2.2 Interaction Details
* **`UIF-NAV-001` — Open-Menu Latch State:** When the navbar's fixed menu (`☰`) is open, the trigger button locks into its `Pressed/Active` visual state (`surface.active` background, `border.strong` outline). Clicking the trigger a second time closes the menu.
* **`UIF-NAV-002` — Ellipsis Convention:** Any navbar button that launches a modal or overlay dialog appends `...` to its label (or its hover tooltip for icon-only buttons).
* **`UIF-NAV-003` — Icon-Only Tooltip Requirement:** Every icon-only control in the navbar displays a tooltip after hover (positioned below the navbar with a `4px` gap, `surface.overlay` fill, `1px border.strong` border, `type.caption` text).

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
  * The interactive pointer hit zone spans `10px` (`5px` to each side of the divider line) with a horizontal resize cursor (`↔`).
  * On hover or active drag, the divider stroke widens/brightens to `2px color.accent.border` (`6.67:1` on `surface.base`).
* **`UIF-PANE-002` — Clamped Widths & Auto-Fit:**
  * Navigating to a directory automatically expands the Left Pane width to fit the longest visible item without wrapping, capped at `20%` of the viewport width (and never narrower than `150px`). Manual user drag overrides persist within `[150px, 20% viewport]`.
  * The Right Pane (Table of Contents) is conditionally mounted only when a document is active and contains at least one heading; when hidden, its width allocation returns seamlessly to the Center Pane.
* **`UIF-PANE-003` — Left Pane Tree & Search Layout:**
  * **Directory Ordering & Filtering:** Folders sort above files at every hierarchy level (`16px` indentation per depth level). Empty folders (or folders with zero matches under an active tag or content search filter) are automatically hidden.
  * **Tree Row Ergonomics:** Every row has minimum height `24px`, `4px` horizontal inset, `type.ui_compact` (`13px`) label, and full keyboard focusability.
  * **Single & Range Selection:** Left-click selects one item; `Shift + Left-click` selects a contiguous range; double-click opens the item in the system editor. Selected rows render with `color.accent.fill` background (`RGB(93, 96, 238)`), `color.text.strong` (`RGB(255, 255, 255)`) text, and a `2px color.accent.border` leading bar. Special item states (such as PDF-backed notes) include a text/icon badge (`[PDF]`) so meaning and contrast (`4.80:1`) are preserved when selected.
  * **Pinned Search Footer:** A `32px` search bar is pinned at the bottom of the Left Pane with a text input (`placeholder: "Search content…"`), submitting on `Enter` or clicking the trailing `28×28px` action button (`🔍` when idle; switches to clear `×` while a filter is active).

---

### 4.4 Tabbed Windows (`UIF-TAB-*`)

#### 4.4.1 Tab Strip & Tab Item Anatomy
```text
┌─ Tab Strip (Height: 36px, Width: Fill, BG: surface.base, Bottom Border: 1px border.subtle) ──┐
│ [Horizontal Scroll Container (Min Tab Width: 96px, Max Tab Width: 220px)]                    │
│  ┌────────────────────────────┐ ┌────────────────────────────┐ ┌───────────────────────────┐ │
│  │ ━━━ (2px accent.border)    │ │                            │ │                           │ │
│  │ [●] [Icon] Title…      [×] │ │     [Icon] Other.md    [×] │ │     [⚙] System Logs   [×] │ │
│  └────────────────────────────┘ └────────────────────────────┘ └───────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

* **Internal Tab Slots (Left to Right):**
  1. **Top Accent Indicator:** `2px` full-width top border (`color.accent.border` when active, `transparent` when inactive, `color.border.strong` on hover).
  2. **Leading Status Slot (`12px`):** Displays a `6px` `color.status.warning` dot (`●`) when the tab has unsaved changes; reserved/collapsed cleanly when saved.
  3. **Title Label (`Fill`, `13px`):** Single-line truncated with `…` when the tab reaches `220px`; hovering the tab displays a tooltip with the full document path.
  4. **Trailing Close Button `[×]` (`24px × 24px` hit target):** Dedicated quiet button with a `14px` `×` glyph, `4px` corner radius, and independent hover highlight (`color.surface.hover` background, `color.status.danger.text` glyph at `5.36:1`).

#### 4.4.2 Tab Visual State Matrix
| Tab State | Surface Fill | Top Bar (`2px`) | Label Typography & Color | Perimeter Border | Close `[×]` State |
|---|---|---|---|---|---|
| **Active** | `surface.overlay` (`RGB(24,24,27)`) | `accent.border` (`5.94:1`) | `13px SemiBold`, `text.strong` (`17.72:1`) | `1px border.subtle` left/right; bottom open into viewer | Visible (`text.muted`; hover: `status.danger.text` on `surface.hover`) |
| **Inactive (Rest)** | `surface.raised` (`RGB(18,18,20)`) | `transparent` | `13px Regular`, `text.muted` (`7.16:1`) | `1px border.subtle` all sides | Visible (`text.disabled`; hover: `text.strong` on `surface.hover`) |
| **Inactive (Hovered)** | `surface.hover` (`RGB(32,32,36)`) | `border.strong` (`3.66:1`) | `13px Regular`, `text.primary` (`10.72:1`) | `1px border.strong` | Visible (`text.primary`; hover: `status.danger.text`) |
| **Dirty (`unsaved`)** | Matches Active/Inactive | Matches state | Leading `●` (`status.warning`) + `text.strong` | Matches state | Clicking `[×]` triggers Unsaved Confirmation Dialog |

#### 4.4.3 Tab Interaction & Lifecycle Rules
* **`UIF-TAB-001` — Pointer & Keyboard Bindings:**
  * Left-clicking the tab body activates the tab.
  * Middle-clicking anywhere on the tab OR left-clicking `[×]` requests tab closure (without first switching active selection if an inactive tab's `[×]` was clicked).
  * Right-clicking anywhere on the tab opens the Tab Context Menu (`§4.9`).
* **`UIF-TAB-002` — Non-Breaking Overflow Scrolling:** When open tabs exceed the Center Pane width, the tab strip scrolls horizontally while maintaining a minimum tab width of `96px` and keeping the `[×]` hit target at `24 × 24px`.
* **`UIF-TAB-003` — Dirty-Close Guard & External Deletion Sync:**
  * Attempting to close a tab with `unsaved == true` (via `[×]`, middle-click, `Close`, or `Close All`) opens a confirmation dialog before discarding edits.
  * If an open tab's underlying file is deleted externally or via the tree, its tab closes automatically and focus moves to the most recently active remaining tab (or the structured empty state if none remain).

---

### 4.5 Input Dialogs (`UIF-INP-*`)

Input dialogs are compact, single-purpose cards (`Create Directory`, `New Document`, `Rename`, `Move`, and `Delete Confirmation` for single or multiple items) designed for rapid completion without losing spatial context.

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
  * When invoked from a pointer action (such as a context menu item), the dialog opens offset `+2px, +2px` from the trigger point so the pointer rests near the input and action buttons.
  * If `anchor + card_size` exceeds the viewport bounds, the top-left origin is clamped to maintain a minimum `8px` (`space.sm`) margin from all window edges. When invoked via keyboard shortcut, the dialog centers in the viewport.
* **`UIF-INP-002` — Card-Level Click Absorption:**
  * Clicking anywhere inside the Input Dialog card—including on a `Disabled` `Ok` button or empty card padding—MUST be absorbed by the card boundary and MUST NEVER fall through to the backdrop scrim or dismiss the dialog.
* **`UIF-INP-003` — Live Validation & Zero-Shift Error Slot:**
  * Input values are validated continuously on keystroke (non-empty, valid filesystem basename characters, and distinct from the current name for `Rename`).
  * The `20px` (`type.body.line_height`) validation error slot is permanently reserved in the card layout so displaying or clearing an error never shifts the `Cancel` / `Ok` buttons.
  * While the input is invalid, `Ok` renders in the `Disabled` state and pressing `Enter` reveals the specific validation error in `color.status.danger.text` (`5.85:1` on `surface.overlay`) while keeping focus inside the text box (`Invalid + Focused` state: `2px status.danger.text` border + `2px accent.border` outer focus ring).
* **`UIF-INP-004` — Immediate Focus & Destructive Parity:**
  * Opening a text input dialog automatically places the text caret inside the input field (selecting the filename stem on `Rename`).
  * Both single-item and multi-item deletions require confirmation via `Delete Confirmation` (displaying the item name for single delete, or the item count for multi-select delete, with the primary button styled as `Danger`).

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
| **Inline Document Editor** | `80%` viewport (`min: 520px`, `max: 960px`) | `80%` viewport (`min: 380px`, `max: 760px`) | Uses high-contrast **Inverted Palette** (`color.editor.inverted.*`: black monospace text on white background, `1px` black border) to distinguish raw editing from dark-mode reading. Includes undo/redo (`100+` steps), `Line X, Col Y` status bar, pre-save Markdown validation, and `[Cancel]` / `[Save]` buttons. |
| **Tools & Integrations** | `720px` (clamped to `viewport.width - 32px`) | `min: 320px`, `max: 80%` viewport | Structured 6-column table: `[☑ Enable]`, `[Group Name]`, `[Kind: Internal/MCP]`, `[Tools List]`, `[Prompt Char Count + ✓ parallel chip]`, and `[Actions: ⚠ Warning Tooltip / Authenticate / Restart]`. |
| **Batch Processing** | `560px` (clamped to `viewport.width - 32px`) | Content-fit (`max: 480px`) | Directory/prompt selectors, live batch progress indicator, and `[Cancel]` / `[Run Batch]` controls. |
| **About & Attributions** | `440px` (`About`) / `640px` (`Attributions`) | Content-fit / `480px` scrollable | Application version, branding, scrollable third-party license table, and `[Close]` action. |

#### 4.6.3 Focus Containment & Data-Safety Invariants
* **`UIF-MOD-001` — Strict Keyboard Focus Trap & Focus Restoration:**
  * When a modal dialog opens, it records the previously focused control and moves keyboard focus automatically to its primary interactive control.
  * `Tab` and `Shift + Tab` wrap focus strictly within the active modal card's enabled focusable elements and MUST NEVER traverse to background panes underneath the scrim. Clicking or programmatically requesting focus outside the active trap is rejected.
  * When the modal closes, focus returns to the recorded prior control (or advances via slot-preserving fall-forward if that control became disabled or unmounted).
* **`UIF-MOD-002` — Unsaved Edit Protection on Dismiss:**
  * Every user-initiated close gesture (`Escape`, scrim click, and header `Close [×]`) routes through a single dirty-state guard: if `dirty == true` (unsaved edits exist), dismissal is declined unless confirmed or explicitly cancelled via `[Cancel]`.
* **`UIF-MOD-003` — Explicit Error Surfacing & Recovery:**
  * If a save, validation, or background authentication action fails inside a modal, the modal MUST remain open and display a structured error banner (`color.status.danger.text` on `surface.overlay`) explaining what failed, where (e.g., line/column for Markdown parse errors), and providing a retry or acknowledge action. Clicking elsewhere inside the modal body does not clear the error banner, and a displayed error banner alone (when `dirty == false`) does not block explicit modal dismissal.

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
  * Pressing and holding the left mouse button inside the `32px` (`size.control.md`) Title Bar *outside* the trailing action button cluster (`[Dock]`, `[Close ×]`) enters the `Dragging` state (`cursor: Grabbing`) and translates the card in real time.
  * Active drag and resize gestures capture pointer tracking until mouse-button release so fast pointer motion outside the card perimeter never drops the gesture.
  * Position and dimensions are preserved across open/close toggles and persisted to storage only when the gesture settles (on drag/resize release or close).
* **`UIF-MOV-002` — Title-Bar Reachability Clamping & Zero-Overshoot Dragging:**
  * Clamping protects the `32px` **Title Bar** (the user's recovery grip), while allowing the card body to extend past the right or bottom viewport edge when the window is smaller than the card:
    * Horizontal `x` is clamped to `[viewport.x + 120 - card_width, viewport.x + viewport.width - 120]` so at least `min(120px, card_width, viewport.width)` of the title bar remains reachable.
    * Vertical `y` is clamped to `[viewport.y + navbar_height (36px), viewport.y + viewport.height - title_bar_height (32px)]`.
  * **Zero-Overshoot Incremental Clamping:** Each drag frame applies the pointer delta to the **last clamped position** (never to `drag_start + cumulative_delta`), ensuring that reversing direction after hitting a viewport edge moves the card on the very first reverse pixel.
* **`UIF-MOV-003` — Non-Blocking Workspace Pass-Through & Z-Promotion:**
  * Movable overlays do **not** render a backdrop scrim. Pointer clicks, scroll events, and text selection outside the floating card's bounds pass directly through to the underlying Main Window panes.
  * When multiple movable overlays are open simultaneously, clicking anywhere inside a background overlay card immediately promotes it to the top of the movable overlay z-layer (`Band 1`).
* **`UIF-MOV-004` — Respectful Stream Auto-Scroll:**
  * Live-updating log and debug views follow incoming entries to the bottom **only** when `[☑ Auto-scroll]` is enabled **and** the scroll viewport is within `2px` (`space.2xs` sub-pixel tolerance) of the bottom.
  * If the user scrolls up to inspect an earlier entry while `[☑ Auto-scroll]` is checked, the view enters an `Inspecting (Paused)` state, tracks the count of new behind-the-scenes entries for a jump-to-latest affordance, and resumes automatic following as soon as the user scrolls back within `2px` of the bottom.

---

### 4.8 Fixed Menus (`UIF-FMENU-*`)

Fixed menus are temporary command surfaces anchored to a permanent UI trigger button (such as the top-right Navbar `☰` menu).

#### 4.8.1 Menu Card & Row Slot Anatomy
Fixed menus and Context menus share a single unified surface and row grid so spacing, typography, and alignment are identical everywhere:

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
  * A fixed menu anchors to its trigger button's bounding rectangle (`AnchorRect`), defaulting to opening **below** the trigger (`4px` / `space.xs` vertical gap) and aligning its right edge with the trigger's right edge when positioned on the right side of the navbar.
* **`UIF-FMENU-002` — Persistent Parent Surfaces & Submenu Cascade Discipline:**
  * Activating or hovering an enabled submenu parent row (`Windows... >`, `Chat models >`, `Table wrap algorithm >`) with at least one enabled child item opens the child submenu card adjacent to the parent card (`2px` / `space.2xs` horizontal gap, opening to the left when anchored near the right viewport edge, aligned to the parent row's top edge) while keeping the parent menu card visible and interactive.
  * Moving to a sibling row on an ancestor card collapses the deeper child branch immediately (except when hovering the open child branch's own trigger row, which preserves the open child card without flicker).
* **`UIF-FMENU-003` — Controlled Selection & Dismissal Policy:**
  * **Action Rows:** Emit their command on pointer release (or `Enter`/`Space`) and close the entire menu tree immediately.
  * **Radio-Choice Rows (`•` mutually exclusive choice):** Select the option (re-activating an already-selected radio row is a selection no-op so a radio group can never be deselected to empty) and close the menu by default.
  * **Checkable Toggle Rows (`✓` independent window/feature toggles):** Toggle the checkmark state in place and keep the menu open by default so the user can toggle multiple windows in one visit (with per-row configurable close-on-toggle where required).

---

### 4.9 Context Menus (`UIF-CMENU-*`)

Context menus are pointer-anchored command surfaces opened by right-clicking a workspace target (a single file, a folder, a multi-file selection in the directory tree, or a document tab in the tab strip).

#### 4.9.1 Semantic Grouping & Row Hierarchy
To prevent cognitive overload in long action lists, context menus organize items into functional groups separated by `1px` `color.border.subtle` dividers, placing destructive actions in the final group:

| Context Target | Structured Group Order |
|---|---|
| **Single File (Tree)** | **1. Open & Output:** `Edit`, `Open in Editor`, `Print`<br>— *Separator* —<br>**2. AI & Automation:** `Run as prompt`, `Format Markdown`, Contextual Note Skills<br>— *Separator* —<br>**3. File Management:** `Rename...`, `Move...`, `Copy path`, `Show in File Explorer`<br>— *Separator* —<br>**4. Destructive:** `Delete...` |
| **Directory (Tree)** | **1. Creation:** `New document...`, `Create Directory ...`<br>— *Separator* —<br>**2. Folder Automation & Management:** Folder Skills, `Rename...`, `Move...`, `Copy path`, `Show in File Explorer`<br>— *Separator* —<br>**3. Destructive:** `Delete...` |
| **Multi-File Selection (Tree)** | **1. Batch AI:** `Merge`<br>— *Separator* —<br>**2. Destructive:** `Delete...` *(routes to plural confirmation)* |
| **Document Tab (Tab Strip)** | **1. Tab Lifecycle:** `Close`, `Close Others`, `Close All`<br>— *Separator* —<br>**2. Edit & Automation:** `Edit`, `Open in Editor`, `Format Markdown`, Contextual Note Skills<br>— *Separator* —<br>**3. Location:** `Copy Path`, `Show in File Explorer` |

#### 4.9.2 Shared Menu Row Visual State Matrix (Fixed & Context Menus)
| Row State | Row Background | Leading Slot (`16px`) | Primary Label (`13px`) | Trailing Slot (`12px` / `>`) | Pointer Cursor |
|---|---|---|---|---|---|
| **Rest (Enabled)** | `transparent` | `accent.text` (`✓` / `•`, `9.49:1`) or `text.muted` | `text.primary` (`11.72:1`) | `text.muted` (`6.78:1`) | `Pointer` |
| **Hovered** | `surface.hover` (`RGB(32,32,36)`) | `text.strong` (`16.24:1`) | `text.strong` (`16.24:1`) | `text.strong` (`16.24:1`) | `Pointer` |
| **Pressed** | `surface.active` (`RGB(42,42,46)`) | `text.strong` (`14.29:1`) | `text.strong` (`14.29:1`) | `text.strong` (`14.29:1`) | `Pointer` |
| **Keyboard Focused** | `surface.hover` + `1px accent.border` (`5.35:1`) inset outline | `text.strong` (`16.24:1`) | `text.strong` (`16.24:1`) | `text.strong` (`16.24:1`) | `Pointer` |
| **Active Submenu Parent** | `surface.hover` (`RGB(32,32,36)`) | `text.strong` (`16.24:1`) | `text.strong` (`16.24:1`) | `accent.text` (`>`, `8.72:1`) | `Pointer` |
| **Destructive Row (Rest)** | `transparent` | `status.danger.text` (`5.85:1`) | `status.danger.text` (`5.85:1`) | `text.muted` (`6.78:1`) | `Pointer` |
| **Destructive Row (Hovered)** | `status.danger.fill` (`RGB(216,28,20)`) | `text.strong` (`5.13:1`) | `text.strong` (`5.13:1`) | `text.strong` (`5.13:1`) | `Pointer` |
| **Destructive Row (Pressed)** | `status.danger.fill_active` (`RGB(202,24,16)`) | `text.strong` (`5.75:1`) | `text.strong` (`5.75:1`) | `text.strong` (`5.75:1`) | `Pointer` |
| **Destructive Row (Focused)** | `status.danger.fill` + `1px text.strong` (`5.13:1`) inset outline | `text.strong` (`5.13:1`) | `text.strong` (`5.13:1`) | `text.strong` (`5.13:1`) | `Pointer` |
| **Disabled / Placeholder** | `transparent` (never inherits hover fill) | `text.disabled` (`5.27:1`) | `text.disabled` (`5.27:1`) | `text.disabled` (`5.27:1`) | `Default` (absorbs click; does not close menu) |
| **Section Heading** | `transparent` | None | `12px SemiBold`, `text.muted` (`6.78:1`) | None | `Default` (non-interactive) |

#### 4.9.3 Pointer Placement, Quadrant Flipping & Keyboard Navigation
* **`UIF-CMENU-001` — Automatic Quadrant Flipping & Viewport Clamping:**
  * Context menus open anchored to the root-space pointer coordinate `(x, y)` with a `+2px, +2px` (`space.2xs`) offset (`Right` and `Below` preference).
  * **Horizontal Flip:** If `x + 2 + menu_width > viewport.width - 8`, the menu flips to open to the **left** of the cursor (`x - menu_width - 2`).
  * **Vertical Flip:** If `y + 2 + menu_height > viewport.height - 8`, the menu flips to open **above** the cursor (`y - menu_height - 2`).
  * **Scrollable Fallback & Tie-Breaking:** If neither side fits in a small window, the menu chooses the roomier side (breaking ties in favor of the default `Right` / `Below` preference), clamps to `8px` (`space.sm`) viewport margins, and scrolls vertically inside the card.
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

To guarantee predictable hit-testing, visual elevation, and keyboard routing across all atomic popups, composite patterns, and the nine core surfaces, the window separates **Visual Elevation (`Z-Band`, 0..3)** from **Keyboard `Escape` Unwind Priority (`Escape Rank`, 3..0)**:

```text
Z-Band 3 (Top Visual) : Blocking Modal & Input Dialogs + Non-Interactive Tooltips ──► Dimmed Scrim (0.55 alpha) + Focus Trap (Escape Rank 2)
Z-Band 2              : Menus, ComboBoxes, AutoSuggest & Teaching Popovers       ──► Transparent Click-Away Blocker + Roving Focus (Escape Rank 3 — unwinds first!)
Z-Band 1              : Movable Utility Overlays & Floating Toast Stack          ──► Modeless Pass-Through Outside Card + Z-Promotion (Escape Rank 1 when focused)
Z-Band 0 (Base)       : Main Window 3-Band Workspace                             ──► Top Navbar + Middle Multi-Panes (with Tabs) + Bottom Bar (Escape Rank 0)
```

| Z-Band | Escape Rank | Surface & Widget Types | Backdrop Behaviour | Pointer Routing | Keyboard Routing |
|---|---|---|---|---|---|
| **Band 0: Workspace** | `Rank 0` (Last) | Main Window, Navbar, Multiple Panes, Tabbed Windows, `TreeView`, `DataTable`, `FormField`, `InfoBar`, `Expander` | None (`surface.base`) | Direct interaction with panes, splitters, tabs, trees, and tables | Standard wrapping `Tab` / `Shift+Tab` and roving arrow traversal across enabled workspace controls |
| **Band 1: Utility Overlays & Toasts** | `Rank 1` (If focused) | Movable Overlay Dialogs (`Logs`, `Agent Debug`), `Toast` / `SnackBar` notification stack | None (pass-through) | Captures events inside card/toast bounds; promotes clicked overlay card to top of Band 1; passes outside clicks to Band 0 | Focuses card/toast controls when clicked; hovering/focusing a Toast pauses auto-dismiss; `Escape` unwinds **only** when focus is inside the overlay |
| **Band 2: Menus & Flyouts** | `Rank 3` (First) | Fixed Menus (`☰`), Context Menus + Nested Submenus, `ComboBox` flyouts, `SplitButton` menus, `AutoSuggestBox` completion lists, `BreadcrumbBar` / `OverflowToolbar` menus, `TeachingPopover` | Full-window transparent click-away catcher | Single-flyout mutual exclusion; captures clicks inside open card(s); outside clicks dismiss Band 2 flyouts | Captures `Up`/`Down`/`Left`/`Right`/`Enter`/`Space`/`Tab` and **always consumes `Escape` first** (deepest submenu/flyout $\rightarrow$ root menu) |
| **Band 3: Modals & Tooltips** | `Rank 2` (Second) | Input Dialogs, Modal Dialogs (`Editor`, `Tools`, `Batch`, `About`), plus non-interactive hover `Tooltip` overlay (`HitTest::Ignore`) | Full-window dimmed scrim (`RGBA(9,9,11,0.55)`) for modals | Modals block lower bands completely and absorb interior clicks; scrim click dismisses only if `dirty == false`; `Tooltip` never intercepts pointer events | Traps `Tab`/`Shift+Tab`, `Enter`, and `Escape` strictly inside topmost modal (after any open Band 2 menu/flyout mounted inside or above the modal is unwound) |

---

## 6. Design Quality Gates & Verification Criteria (`UIF-VER-*`)

An AI coding agent implementing or auditing widgets against this specification MUST verify compliance across four objective quality gates:

1. **Token & Contrast Gate (`UIF-VER-001`):**
   * Zero raw RGB/hex color literals, inline string literals, or magic spacing numbers outside the central token definitions (`§2`) and named surface geometry constants (`§4`).
   * Automated WCAG 2.1 luminance contrast assertions verify every reachable `(Variant × VisualState)` pairing across the four stroke/fill categories (`Judged`, `CoincidentWithFill`, `UnpaintedBackground`, `RestingDivider`), confirming $\ge 4.50:1$ for all active text pairings (including `text.strong` on `accent.fill` and `status.danger.fill`), `[4.00:1, 6.00:1]` for `text.disabled`, and $\ge 3.00:1$ for all `Judged` active borders, solid fills, `Checkbox`/`Radio`/`Switch` states, `Slider` tracks/thumbs, and adaptive focus rings.
2. **Geometry, Hit-Target & Responsive Container Gate (`UIF-VER-002`):**
   * Every interactive element measures at least `24px × 24px` in hit bounds; all text measures at least `12px`; splitters and table column dividers provide a `10px` grab zone; text inputs, `FormField` wrappers, and input dialogs guarantee `0px` layout shift across trailing-slot, `ActionPending` (`busy`), and validation-error state changes.
   * Pure boundary solver property tests verify menu/flyout quadrant flipping, tie-breaking, input dialog cursor clamping, tab strip overflow scrolling, `BreadcrumbBar` middle-ancestor elision (`[…]`), `OverflowToolbar` (`[⋯]`) container-width collapse, and movable overlay incremental title-bar clamping (with zero drag-overshoot dead zone) across extreme viewport sizes (down to `640 × 480px`) and non-finite inputs.
3. **Interaction, State-Machine & Callback Lifecycle Gate (`UIF-VER-003`):**
   * Headless state and interaction tests verify:
     * The full 32-combination `InteractionInput` truth table (resolving to the 9 reachable `VisualState`s with total `Disabled` dominance) and press-and-release activation.
     * Strict **`on_change` (live) vs. `on_commit` (settled) vs. `on_cancel` (revert)** separation across `TextInput`, `PasswordInput`, `AutoSuggestBox`, `ComboBox`, `Slider`, `NumberBox`, `DataTable` column resize, and `MovableOverlay` drag/resize (verifying `on_commit` fires **exactly once** on settle/submit).
     * Keyboard navigation contracts for `SegmentedButton`, `Checkbox` (including tri-state `Indeterminate`), roving `RadioGroup`, `TreeView` (`Up`/`Down`/`Left`/`Right` + `Ctrl`/`Shift` multi-select), `DataTable` sort/cell traversal, `Toast` hover-pause and non-auto-dismissing `Warning`/`Danger` severity rules, card-interior click absorption, dirty-state close guards (`dirty` vs. `error`), focus trap containment/restoration, and `Escape Rank` unwinding (`Menu/Flyout [Rank 3] -> Modal [Rank 2] -> Focused Movable Overlay [Rank 1] -> Workspace [Rank 0]`).
4. **Visual & Layer Snapshot Gate (`UIF-VER-004`):**
   * Deterministic headless snapshots verify both **resolved semantic token assignments** and **ordered draw-layer quad lists (`Layer { bounds, fill, border }`)** across all 9 reachable states for every atomic widget (`§3.1–§3.9`), every composite widget pattern (`§3B.1–§3B.8`), and every core surface (`§4.1–§4.9`).

