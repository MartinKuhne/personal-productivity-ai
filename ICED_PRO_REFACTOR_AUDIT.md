# Hoarfrost Refactor Audit — Remaining `iced::widget::` Usage

**Date**: 2026-10-28 (Re-audited)  
**Status**: Button and RadioGroup migrations complete; Checkbox partially migrated (Logs pending); ComboBox blocked by framework in-flow layout; Tooltips partially wired; SearchBox/Hyperlink available in Hoarfrost but bypassed in app  
**Purpose**: Authoritative audit of Hoarfrost adoption by FastMD (`src/app`), cataloging migrated controls, remaining native usages, framework blockers, and architectural non-fits

---

## Executive Summary

| Category | Count | Action |
|----------|-------|--------|
| ✅ **Fully Adopted in App** | `Button` (atoms), `RadioGroup` (controlled), Menu Cascade engine, Movable Host overlay, Design Tokens | Done |
| ⚠️ **Partially Adopted (Gaps)** | `Checkbox` (`logs.rs` unmigrated), `Tooltip` (only 3 controls wired), `Hyperlink` (resolver only, widget bypassed), `TextInput` (custom search row in `left.rs`), `ProgressBar` (static container only) | App migration / wiring pending |
| 🚫 **Blocked by Framework** | `ComboBox` (flyout is in-flow `Column`, shifts layout; no overlay host adapter) | Framework fix needed before app adoption |
| 🚫 **Architectural Non-Fits** | `SplitDivider` (conflicts with `pane_grid`), `TabStrip` (hover false, 32px vs 36px, menu origin), `NavBar` (row-only, lacks right flex/logo/status), `TreeView`/`VirtualList` (arithmetic only, ephemeral vs persisted) | Keep app implementations |
| 🔄 **Core iced widgets (keep iced)** | `pane_grid`, `text_editor`, `table`, `operation`, `rich_text` | Keep — out of Hoarfrost's scope or intentional domain fits |
| 🔄 **Unadopted Hoarfrost widgets** | `Chip`, `Segmented`, `SplitButton`, `DatePicker`, `TimePicker`, `ColorPicker`, `RangeSlider`, `ToggleSwitch`, `SearchBox`, `CommandPalette`, `SideSheet`, `EmptyState`, `Skeleton`, `PaginationBar`, `CodeBlock`, `DiffViewer` | Candidate backlog |

---

## Detailed Audit by File

### 1. Layout Primitives — KEEP (iced core, not replaced by Hoarfrost)

| Widget | Files | Notes |
|--------|-------|-------|
| `Column`, `Row`, `Space` | 30+ files | Basic layout — Hoarfrost uses these too |
| `container` | 25+ files | Basic layout |
| `text` | 30+ files | Basic text rendering |
| `scrollable` | 15+ files | Basic scrolling (TOC, viewer, tree, search, logs have stable IDs per INT-21a) |
| `stack` | `view.rs`, `menu_surface.rs`, `overlay.rs` | Layering |
| `mouse_area` | 10+ files | Hover/press detection — Hoarfrost uses this internally |
| `rule` | `panels/top.rs`, `render/mod.rs` | Dividers |
| `rich_text` / `text::{Rich, Span}` | `render/inline.rs` | Rich text rendering |

---

### 2. Core iced Widgets — KEEP (out of Hoarfrost scope or intentional fits)

| Widget | Files | Reason |
|--------|-------|--------|
| `pane_grid` | `app.rs`, `layout.rs`, `message.rs`, `update_tests.rs` | Core app 3-pane layout; Hoarfrost's `SplitDivider` is pairwise-only and cannot replace the N-ary split tree |
| `text_editor` | `editor.rs`, `editor_tests.rs`, `message.rs`, `message_tests.rs` | **Markdown editor with undo/redo** — Hoarfrost has `text_area_model`/`host` for plain text only |
| `table` | `tools_dialog.rs`, `render/table.rs` | Data table driven by pure Fair Table Width Algorithm (FTWA); Hoarfrost `table_*` composites are unused |
| `operation` | `update.rs` | Core iced focus/accessibility ops |

---

### 3. Interactive Widgets — ADOPTION STATUS

#### A. `button` → `hoarfrost::widget::Button` ✅ **ADOPTED (with tooltip gap)**

All native `iced::widget::button` instances have been replaced by `hoarfrost::widget::button_widget::Button`:

| File | Status | Notes |
|------|--------|-------|
| `render/code.rs` | ✅ | Copy code button |
| `agent/transcript.rs` | ✅ | Transcript action buttons |
| `panels/center.rs` | ✅ | Tab close button (`Button::icon_only`) |
| `view.rs` | ✅ | View mode buttons |
| `movable_shell.rs` | ✅ | Movable card title bar buttons |
| `overlay.rs` | ✅ | Overlay dismiss buttons |
| `agent/prompt_input.rs` | ✅ | Prompt submit / action buttons |
| `about.rs` | ✅ | Modal action buttons |
| `batch_dialog.rs` | ✅ | Batch run / action buttons |
| `tools_dialog.rs` | ✅ | Tool enable / action buttons |
| `attributions.rs` | ✅ | Close button |
| `logs.rs` | ✅ | Clear logs button |
| `agent_debug.rs` | ✅ | Inspect / clear buttons |

> [!NOTE]
> **Button Gaps**:
> 1. `ButtonModel` and `ButtonController` (`UIF-ATOM-001a` gesture slots, busy/double-activation gates, `Live`/`Commit`/`Cancel` tiers) are not used by `src/app`.
> 2. `impl From<Button> for Element` calls `Element::new(button)` directly, bypassing `Button::into_element()`, which silently drops automatic tooltip wrapping when `.into()` is called.

#### B. `checkbox` → `hoarfrost::widget::Checkbox` ⚠️ **PARTIAL (Logs Pending)**

| File | Line | Usage | Status |
|------|------|-------|--------|
| `tools_dialog.rs` | 91 | Tool enable/disable | ✅ Migrated |
| `render/mod.rs` | 67 | Task list checkboxes | ✅ Migrated |
| `logs.rs` | 725 | Auto-scroll checkbox | ❌ **UNMIGRATED (still uses native `iced::widget::checkbox`)** |

#### C. `radio` → `hoarfrost::widget::RadioGroup` ✅ **COMPLETE**

| File | Line | Usage | Status |
|------|------|-------|--------|
| `batch_dialog.rs` | 833-840 | Strategy radio group | ✅ Migrated via `Controlled<RadioGroupModel>` |

#### D. `tooltip` → `hoarfrost::surface::tooltip` ⚠️ **PARTIAL (3 Controls Only)**

| Control | Location | Status |
|---|---|---|
| Search Trigger (magnifier) | `panels/left.rs` | ✅ Wired to `TooltipState` |
| Search Clear (×) | `panels/left.rs` | ✅ Wired to `TooltipState` |
| Tools Error Marker (⚠) | `tools_dialog.rs` | ✅ Wired to `TooltipState` |
| All Other Icon Buttons | `top.rs`, `center.rs`, `dialogs.rs`, etc. | ❌ **Inert / Unrendered tooltips** |

---

### 4. Interactive Widgets — AVAILABLE IN HOARFROST BUT BYPASSED IN APP

| Hoarfrost Widget | Spec ID | App Location | Current App State & Reason |
|------------------|---------|--------------|----------------------------|
| `widget::SearchBox` | `UIF-PANE-003` | `panels/left.rs:1587-1630` | Hand-rolls separate `TextInput`, `search_trigger`, and `clear_trigger` with comments claiming composite does not exist |
| `widget::Hyperlink` | `UIF-ATOM-001d` | `attributions.rs:360` | Uses `hyperlink::resolve_style` with manual container + mouse area; claims widget does not exist |
| `widget::Chip` / `ChipModel` | `UIF-ATOM-006` | `panels/left.rs:2050`, `tools_dialog.rs:1145` | Hand-rolls `PdfBadgeChip` and `parallel_chip` as ad-hoc iced containers |
| `widget::ProgressBar` / `ProgressRing` | `UIF-ATOM-009` | `batch_dialog.rs:1095`, `panels/top.rs:505` | `batch_dialog` renders static container; `top.rs` uses italic text ellipsis |
| `composite::CodeBlock` | `UIF-COMP-015` | `render/code.rs` | Hand-rolls bespoke code block model without 28px header, language badge, copy button success hold, or line gutters |

---

### 5. Interactive Widgets — BLOCKED BY FRAMEWORK

#### `pick_list` → Hoarfrost `ComboBox` widget 🚫 **BLOCKED**

| File | Usage | Status |
|------|-------|-------|
| `panels/top.rs:954` | Tag filter dropdown | Blocked by in-flow layout & 120px floor |
| `batch_dialog.rs:817` | Directory dropdown | Blocked by in-flow layout |
| `batch_dialog.rs:876` | Concurrency dropdown | Blocked by in-flow layout |
| `logs.rs:716` | Category filter dropdown | Blocked by in-flow layout |
| `agent/prompt_input.rs:297` | Chat model selector | Blocked by in-flow layout |
| `agent_debug.rs:949` | JSON row selector | Blocked by in-flow layout |

**Why `ComboBox` cannot be adopted today:**
1. **In-Flow Expansion vs Floating Overlay:** `combo_box.rs:595` pushes an in-flow `Column` child when `model.is_open()` is true. In `top.rs` (36px fixed navbar), opening the dropdown expands the navbar to 240px tall, distorting the entire application window layout.
2. **Headless `combo_host.rs`:** Emits no `Element`. Wiring an overlay popup requires custom integration with `MenuCascade`, `FocusState`, `SurfaceRegistry`, and `OverlayState` in `view.rs`.
3. **Model Ceremony:** Native `pick_list` takes a plain slice `&[T]`. `ComboBoxModel` requires persistent state, `SelectOption<Id>` adapters, and hashable IDs that risk collisions (e.g., tag named "All").
4. **Sizing:** `MIN_WIDTH = 120.0px` breaks compact `Length::Shrink` usage sites.

---

## Recommended Next Steps

### Quick Wins (Clean Migrations in `src/app`)
1. **Migrate Logs auto-scroll checkbox** in `src/app/ui_iced/logs.rs:725` to `hoarfrost::widget::toggle_widget::Checkbox`.
2. **Adopt `SearchBox`** in `src/app/ui_iced/panels/left.rs:1587`, replacing the hand-rolled text input and trigger layout.
3. **Adopt `Hyperlink` widget** in `src/app/ui_iced/attributions.rs:360`, replacing the manual container + mouse area.
4. **Remove dead `tab_close_style`** in `src/app/ui_iced/panels/center.rs:988`.

### Framework Enhancements Required in `src/hoarfrost`
1. **Overlay ComboBox:** Refactor `hoarfrost::widget::combo_box` or build an overlay container so the flyout floats in an overlay layer rather than expanding in-flow.
2. **TabStrip Fixes:** Allow dynamic hover input (`is_hovered`) in `tab_strip.rs`, support pointer-anchored context menus, and update icon styling to match FastMD's 36px token contracts.


---

## Quality Gates Status

All gates pass after migrations:
- ✅ `cargo check --quiet`
- ✅ `cargo nextest run --status-level fail` (2411 tests)
- ✅ `cargo clippy -- -D warnings`
- ✅ `cargo fmt --check`
- ✅ `cargo doc --no-deps --quiet`
