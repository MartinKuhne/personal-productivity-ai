# Hoarfrost Refactor Audit — Remaining `iced::widget::` Usage

**Date**: 2026-10-26  
**Status**: Hoarfrost rename and ComboBox widget are merged; Button/Checkbox/RadioGroup migrations are complete; app-side ComboBox review remains  
**Purpose**: Handoff document for spec review & framework coding agent

---

## Executive Summary

| Category | Count | Action |
|----------|-------|--------|
| ✅ **Already using Hoarfrost** | Migrated Button, Checkbox, RadioGroup, tooltip, and existing framework controls | Done |
| 🔄 **Layout primitives (keep iced)** | Column, Row, container, text, scrollable, Space, Stack, rule, mouse_area | Keep — Hoarfrost doesn't replace these |
| 🔄 **Core iced widgets (keep iced)** | pane_grid, text_editor, table, operation, rich_text | Keep — out of Hoarfrost's scope |
| 🔄 **Available in Hoarfrost** | `Button`, `Checkbox`, `RadioGroup`, `ToggleSwitch`, `Chip`, `Segmented`, `SplitButton`, `Hyperlink`, `DatePicker`, `RangeSlider`, `ComboBox` | App migration/use review pending |
| ✅ **Framework gaps in this handoff** | None identified for the audited controls | No ComboBox wrapper work remains |

---

## Detailed Audit by File

### 1. Layout Primitives — KEEP (iced core, not replaced by Hoarfrost)

| Widget | Files | Notes |
|--------|-------|-------|
| `Column`, `Row`, `Space` | 30+ files | Basic layout — Hoarfrost uses these too |
| `container` | 25+ files | Basic layout |
| `text` | 30+ files | Basic text rendering |
| `scrollable` | 15+ files | Basic scrolling |
| `stack` | `view.rs`, `menu_surface.rs`, `overlay.rs` | Layering |
| `mouse_area` | 10+ files | Hover/press detection — Hoarfrost uses this internally |
| `rule` | `panels/top.rs`, `render/mod.rs` | Dividers |
| `rich_text` / `text::{Rich, Span}` | `render/inline.rs` | Rich text rendering |

---

### 2. Core iced Widgets — KEEP (out of Hoarfrost scope)

| Widget | Files | Reason |
|--------|-------|--------|
| `pane_grid` | `app.rs`, `layout.rs`, `message.rs`, `update_tests.rs` | Core app layout, not a "widget" in Hoarfrost's scope |
| `text_editor` | `editor.rs`, `editor_tests.rs`, `message.rs`, `message_tests.rs` | **Markdown editor with undo/redo** — Hoarfrost has `text_area_model`/`host` for plain text only |
| `table` | `tools_dialog.rs`, `render/table.rs` | Data table — no Hoarfrost equivalent |
| `operation` | `update.rs` | Core iced focus/accessibility ops |
| `tooltip` | **REPLACED** — now uses Hoarfrost `surface::tooltip` | ✅ Done |

---

### 3. Interactive Widgets — MIGRATED TO HOARFROST

#### A. `button` → `hoarfrost::widget::Button` ✅ **COMPLETE**

| File | Lines | Status |
|------|-------|--------|
| `render/code.rs` | 22, 160 | ✅ |
| `agent/transcript.rs` | 53, 323 | ✅ |
| `panels/center.rs` | 170, 1351, 1505 | ✅ |
| `view.rs` | 54, 172 | ✅ |
| `movable_shell.rs` | 190, 197 | ✅ |
| `overlay.rs` | 342, 345 | ✅ |
| `agent/prompt_input.rs` | 114, 264, 274, 284, 308 | ✅ (via `gated_button`) |
| `about.rs` | 504, 530, 557, 650 | ✅ (via `footer_button`) |
| `batch_dialog.rs` | 1005, 1013, 1087, 1258 | ✅ (via `footer_button`) |
| `tools_dialog.rs` | 844, 1104, 1115, 1122, 1161 | ✅ (via helpers) |
| `attributions.rs` | 357 | ✅ (via `close_button`) |
| `logs.rs` | 727 | ✅ |
| `agent_debug.rs` | 949, 1140 | ✅ |

**Pattern**:
```rust
use hoarfrost::widget::button_widget::Button;
use hoarfrost::widget::button::ButtonVariant;

Button::new(ButtonVariant::Primary, text("Label"), Message::Action, "stable_id")
    .palette(Palette::pro())
```

#### B. `checkbox` → `hoarfrost::widget::Checkbox` ✅ **COMPLETE**

| File | Line | Usage | Status |
|------|------|-------|--------|
| `tools_dialog.rs` | 91 | Tool enable/disable | ✅ |
| `render/mod.rs` | 67 | Task list checkboxes | ✅ |

**Pattern**:
```rust
use hoarfrost::widget::toggle_widget::Checkbox;
use hoarfrost::widget::toggle_model::CheckState;

Checkbox::standalone(label, CheckState::Checked, focus_id, on_toggle)
    .with_palette(Palette::pro())
```

#### C. `radio` → `hoarfrost::widget::RadioGroup` ✅ **COMPLETE**

| File | Line | Usage | Status |
|------|------|-------|--------|
| `batch_dialog.rs` | 833-840 | Strategy radio group | ✅ |

**Pattern**:
```rust
use hoarfrost::widget::toggle_widget::RadioGroup;
use hoarfrost::widget::toggle_model::{RadioGroupModel, RadioOption};
use hoarfrost::binding::Controlled;

let model: Controlled<RadioGroupModel<String>> = host.radio_model(id);
RadioGroup::controlled(model, focus_id, |commit| Some(Message::Action(commit)))
    .with_palette(Palette::pro())
```

---

### 4. Interactive Widgets — NOW AVAILABLE IN HOARFROST (Migration Candidates)

| iced-native | Hoarfrost replacement | Spec | App usage |
|-------------|---------------------|------|-----------|
| *none direct* | `widget::Chip` / `ChipModel` | UIF-ATOM-006 | Filter chips, tags |
| *none direct* | `widget::Segmented` / `SegmentedModel` | UIF-ATOM-001b | Toggle groups |
| *none direct* | `widget::SplitButton` / `SplitButtonModel` / `SplitButtonHost` | UIF-ATOM-001c | Primary + menu actions |
| *none direct* | `widget::Hyperlink` / `HyperlinkModel` | UIF-ATOM-001d | Inline links |
| *none direct* | `widget::DatePicker` / `DateRangePicker` / `CalendarView` | UIF-ATOM-010 | Date selection |
| *none direct* | `widget::RangeSlider` | UIF-ATOM-005b | Range selection |
| *none direct* | `widget::ToggleSwitch` / `SwitchModel` | UIF-ATOM-003 | On/off toggles |
| `pick_list` | `widget::ComboBox` | UIF-ATOM-004 | Dropdown usages; app-side fit review pending |

---

### 5. Interactive Widgets — APP-SIDE REVIEW PENDING

#### `pick_list` → Hoarfrost `ComboBox` widget (AVAILABLE)

| File | Usage | Status |
|------|-------|-------|
| `panels/top.rs` | Tag filter dropdown | Review compatibility |
| `batch_dialog.rs` | Directory and concurrency dropdowns | Review compatibility |
| `agent/prompt_input.rs` | Chat model selector | Review compatibility |
| `logs.rs` | Category filter dropdown | Review compatibility |
| `agent_debug.rs` | JSON row selector | Review compatibility |

The incoming Hoarfrost updates provide the `widget::ComboBox` wrapper along with its model/host and menu-cascade integration. The remaining work is app-side compatibility review and migration, not framework implementation.

---

## Framework Handoff Status

No additional framework work is identified for the audited ComboBox gap: the wrapper is now available in Hoarfrost. The remaining work is app-side review and migration of the listed native `pick_list` sites.

---

## Spec Review Notes for Framework Agent

### UIF-ATOM-004 (ComboBox) — Implementation Notes
1. **Type-ahead on closed select**: Explicitly NOT implemented per `combo_model` docs
2. **Read-only mode**: Browsable but never commits — spec covers, widget must enforce
3. **Filter matching**: `ByWidgetSubstring` vs `ByCaller` — spec covers
4. **Trigger geometry**: Already in `combo_box` module — use `trigger_bounds`, `trailing_slot_bounds`, `value_bounds`
5. **Flyout**: Uses `MenuCascade` (Z-Band 2, Escape Rank 3) — already implemented

### UIF-ATOM-003 (Toggle/Checkbox/Radio/Switch) — ✅ WIDGETS EXIST
- `widget::toggle_widget::Checkbox` — tri-state, controlled/standalone, label support
- `widget::toggle_widget::RadioGroup` — roving focus, vertical/horizontal, controlled
- `widget::toggle_widget::ToggleSwitch` — busy refusal, on/off text, controlled/standalone
- All follow `button_widget` pattern: `state::resolve` → `toggle::resolve_style` → paint

### UIF-ATOM-007 (Tooltip) — ✅ COMPLETE
- Integrated using Hoarfrost APIs
- No additional framework changes needed
- Presentation layer above z-bands (not a band)
- Warm handoff via `WarmSession` (100ms)
- Keyboard focus = 0ms delay

---

## Recommended Next Steps

### For App Team (Clear Path Forward)
1. **Evaluate** new widgets (`Chip`, `Segmented`, `SplitButton`, `Hyperlink`, `DatePicker`, `RangeSlider`, `ToggleSwitch`) for adoption
2. **Keep** `text_editor`, `table`, `pane_grid`, and `rich_text` on iced native; evaluate `pick_list` sites against the available Hoarfrost `ComboBox`

### For Framework Agent
No additional framework work is identified by this audit. The app team can evaluate the available ComboBox wrapper for the listed dropdown sites.

---

## Quality Gates Status

All gates pass after migrations:
- ✅ `cargo check --quiet`
- ✅ `cargo nextest run --status-level fail` (2411 tests)
- ✅ `cargo clippy -- -D warnings`
- ✅ `cargo fmt --check`
- ✅ `cargo doc --no-deps --quiet`
