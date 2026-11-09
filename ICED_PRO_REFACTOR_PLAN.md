# Hoarfrost Refactor Plan for src/app

> **Historical plan — superseded.** The iced-pro crate was renamed to Hoarfrost. Since this plan was written, tooltip, Button, Checkbox, and RadioGroup migrations have landed, and Hoarfrost now provides a ComboBox widget wrapper. See `ICED_PRO_REFACTOR_AUDIT.md` for the current app-side review status.

## Executive Summary

The `src/app/ui_iced` module currently uses a mix of:
- Direct `iced::widget::*` imports (pane_grid, pick_list, button, checkbox, radio, text_editor, table, tooltip, progress_bar, hyperlink, etc.)
- `iced_pro::widget::*` imports (button_widget, text_input, combo_box, progress, hyperlink, badge, toggle, etc.)

The goal is to fully migrate to `iced-pro` widgets where available, and identify gaps where iced-pro doesn't yet provide equivalents.

---

## Gap Analysis

### ✅ Already Migrated / In Progress

| Widget | iced-pro Module | Status | Files |
|--------|-----------------|--------|-------|
| Button | `button_widget::Button` | ✅ Adopted | top.rs, bottom.rs, batch_dialog.rs, about.rs, etc. |
| Text Input | `text_input::TextInput` | ✅ Adopted | bottom.rs, agent_debug.rs, batch_dialog.rs |
| Hyperlink | `hyperlink` | ✅ Adopted | attributions.rs |
| Progress | `progress` + `progress_model` | ✅ Adopted | batch_dialog.rs |
| Badge | `badge` + `badge_model` | ✅ Adopted | left.rs (PDF badge as chip) |
| Toggle/Checkbox/Radio | `toggle` | ⚠️ Partial | render/mod.rs, tools_dialog.rs, batch_dialog.rs still use iced |
| ComboBox/Select | `combo_box` + `combo_host` + `combo_model` | ⚠️ Partial | top.rs (tag filter), batch_dialog.rs, prompt_input.rs use iced pick_list |

---

### 🔴 High Priority Gaps (Need Implementation)

#### 1. **Tooltip (UIF-ATOM-007)**
- **App usage**: `iced::widget::tooltip` in:
  - `panels/left.rs:1572-1587` (search trigger & clear buttons)
  - `tools_dialog.rs:1082-1090` (error tooltips on ⚠ markers)
- **iced-pro provides**: Complete tooltip subsystem in `surface/tooltip/`:
  - `HoverController` - delay state machine
  - `TooltipStyle::resolve` - paint resolver
  - `TooltipAnchor` + `place::resolve` - placement solver
  - `WarmSession` - cross-control warm handoff
- **Work required**:
  1. Add `HoverController` per tooltip-bearing control to app state
  2. Add global `WarmSession` for warm handoff
  3. Call `tick(TimeMs::now())` each frame in update loop
  4. Mount tooltip as presentation layer above all z-bands in view
  5. Wire keyboard focus arrivals (`keyboard_focus_enter`)
  6. Handle dismissal (Escape, click, pointer exit)

#### 2. **ComboBox Widget (UIF-ATOM-004)**
- **App usage**: `iced::widget::pick_list` in:
  - `panels/top.rs:951-960` (tag filter)
  - `batch_dialog.rs` (pattern input, strategy selector)
  - `agent/prompt_input.rs` (chat model selector)
- **iced-pro provides**: 
  - `combo_box` - geometry/resolver only (NOT a widget)
  - `combo_host` - controller with MenuCascade
  - `combo_model` - data model
- **Gap**: No `ComboBox` widget that returns `Element<Message>`
- **Work required**: Build a `ComboBox` widget in iced-pro or app that composes `combo_host` + trigger rendering + flyout mounting

#### 3. **Checkbox/Radio (UIF-ATOM-003)**
- **App usage**: 
  - `render/mod.rs:67` - `iced::widget::checkbox` for task lists
  - `tools_dialog.rs:89` - `iced::widget::checkbox` for tool enable/disable
  - `batch_dialog.rs:833-840` - `iced::widget::radio` for strategy selection
- **iced-pro provides**: `toggle` module with `Checkbox`, `Radio`, `ToggleSwitch` models and resolvers
- **Gap**: No widget wrapper (`ToggleWidget`) like `Button` has `button_widget`
- **Work required**: Build widget wrappers for toggle atoms, or use gallery_toggle pattern

---

### 🟡 Medium Priority Gaps

#### 4. **Text Area / Multi-line Editor (UIF-ATOM-002e)**
- **App usage**: `iced::widget::text_editor` in `editor.rs` (full markdown editor with undo/redo)
- **iced-pro provides**: `text_area_model` + `text_area_host` (multi-line, but no markdown rendering)
- **Gap**: iced-pro's text_area is a plain text editor; app needs markdown editor with syntax highlighting
- **Decision**: Keep iced's `text_editor` for editor.rs; use text_area_host for simpler multi-line inputs

#### 5. **Table Widget**
- **App usage**: `iced::widget::table` in `tools_dialog.rs`, `render/table.rs`
- **iced-pro provides**: Nothing (gallery_table is demo only)
- **Decision**: Keep iced's table; consider adding to iced-pro later

---

### 🟢 Low Priority / Keep as-is

#### 6. **Pane Grid**
- **App usage**: `iced::widget::pane_grid` in `layout.rs`, `message.rs`, `app.rs`
- **Status**: iced core widget, not in iced-pro scope

---

## Implementation Order

1. **Tooltip** - Complete subsystem, high impact (used in 2+ panels)
2. **ComboBox widget** - Needed for 3 pick_list usages
3. **Toggle widgets (Checkbox/Radio)** - Needed for 3 checkbox/radio usages
4. **Document remaining gaps** - Table, Text Editor, Pane Grid

---

## Spec Gaps / Challenges Noted

### Tooltip (UIF-ATOM-007)
- Spec covers non-interactive Tooltip only; `TeachingPopover` (interactive) is **not built** (spec §3.7, gap noted in tooltip/mod.rs:76)
- No re-show delay specified in spec (framework decision)
- Tooltip is presentation layer **outside** z-band registry (not a band)

### ComboBox (UIF-ATOM-004)
- Spec §3.4 prices trigger as input atom's dropdown row
- Flyout uses MenuCascade (Z-Band 2, Escape Rank 3)
- Filter matching: `ByWidgetSubstring` or `ByCaller`
- **Type-ahead on closed select is NOT implemented** (combo_model docs)
- Read-only mode: browsable but never commits

### Toggle (UIF-ATOM-003)
- Three controls share one `TogglePolarity` resolver
- Checkbox: tri-state (Unchecked/Checked/Indeterminate)
- Radio: roving focus group
- Switch: busy refusal, no progress/cancel

---

## Subagent Tasks

Each gap will be addressed by a dedicated subagent with:
1. Clear scope
2. Test-first approach
3. Incremental commits
4. Spec gap documentation in commit messages
