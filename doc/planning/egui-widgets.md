# egui Widget & Library Assessment

**Status:** Reference / Research  
**Last updated:** 2026-08-23

> Sources: [official egui wiki](https://github.com/emilk/egui/wiki/3rd-party-egui-crates),
> [hello_egui](https://github.com/lucasmerlin/hello_egui), [crates.io](https://crates.io).

**Rating key** — ⭐⭐⭐ high · ⭐⭐ medium · ⭐ low · ➖ unknown/not applicable

---

## 🏛️ Core / Official

| Crate | What it does | Utility | Popularity | Maturity | Support |
|-------|-------------|---------|------------|---------|---------|
| [egui](https://crates.io/crates/egui) | Immediate-mode GUI core — buttons, sliders, text, layouts | ⭐⭐⭐ | ⭐⭐⭐ (30k ★) | ⭐⭐⭐ | ⭐⭐⭐ Official |
| [egui_extras](https://crates.io/crates/egui_extras) | Official extras: images, `TableBuilder`, `RetainedImage`, date picker | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ Official |
| [egui_plot](https://github.com/emilk/egui_plot) | 2-D plots: lines, points, bars, histograms | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ Official |

---

## 📦 Widgets

### Tables & Data

| Crate | What it does | Utility | Popularity | Maturity | Support |
|-------|-------------|---------|------------|---------|---------|
| [egui_table](https://github.com/rerun-io/egui_table) | High-performance table with sorting, row virtualization (Rerun-backed) | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐⭐ Rerun org |
| [egui-data-table](https://crates.io/crates/egui-data-table) | Editable spreadsheet-style data table | ⭐⭐ | ⭐ | ⭐⭐ | ⭐ Community |
| [egui-table-filter](https://github.com/thomasnield/egui-table-filter) | Column filter overlay for tables | ⭐⭐ | ⭐ | ⭐ | ⭐ Community |
| [egui_json_tree](https://crates.io/crates/egui_json_tree) | Collapsible JSON tree viewer with search & highlighting | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ Active |

### Input & Forms

| Crate | What it does | Utility | Popularity | Maturity | Support |
|-------|-------------|---------|------------|---------|---------|
| [egui_form](https://crates.io/crates/egui_form) | Form validation (integrates `validator` crate), inline error messages | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ hello_egui |
| [egui_double_slider](https://crates.io/crates/egui_double_slider) | Range slider with two handles | ⭐⭐ | ⭐ | ⭐ | ⭐ Community |
| [egui-multiselect](https://crates.io/crates/egui-multiselect) | Tag/chip multi-select combo widget | ⭐⭐ | ⭐ | ⭐ | ⭐ Community |

### Rich Content & Display

| Crate | What it does | Utility | Popularity | Maturity | Support |
|-------|-------------|---------|------------|---------|---------|
| [egui_commonmark](https://crates.io/crates/egui_commonmark) | Render CommonMark Markdown inline in egui | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | ⭐⭐ Active |
| [egui-circular-progress-bar](https://crates.io/crates/egui-circular-progress-bar) | Circular/radial progress indicator | ⭐⭐ | ⭐ | ⭐ | ⭐ Community |
| [egui-jxl](https://crates.io/crates/egui-jxl) | JPEG XL image display widget | ⭐ | ⭐ | ⭐ | ⭐ Niche |
| [lumina-video](https://github.com/lumina-video/lumina-video) | Video playback widget | ⭐⭐ | ⭐ | ⭐ | ⭐ Early |
| [walkers](https://crates.io/crates/walkers) | Interactive slippy map (OpenStreetMap tiles) | ⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ Active |

### Interaction

| Crate | What it does | Utility | Popularity | Maturity | Support |
|-------|-------------|---------|------------|---------|---------|
| [egui_dnd](https://crates.io/crates/egui_dnd) | Drag-and-drop reordering of lists/items | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ hello_egui |
| [egui_pull_to_refresh](https://crates.io/crates/egui_pull_to_refresh) | Mobile-style pull-to-refresh gesture | ⭐ | ⭐ | ⭐ | ⭐ hello_egui |
| [egui_pie_menu](https://github.com/Deuracell/egui_pie_menu) | Radial/pie context menu | ⭐⭐ | ⭐ | ⭐ | ⭐ Community |

---

## 🗂️ Layout Containers

| Crate | What it does | Utility | Popularity | Maturity | Support |
|-------|-------------|---------|------------|---------|---------|
| [egui_dock](https://crates.io/crates/egui_dock) | Dockable/tabbed panel layout (IDE-style) | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ Active |
| [egui_tiles](https://github.com/rerun-io/egui_tiles) | Flexible tiling/splitting panels (Rerun-backed) | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ Rerun org |
| [egui_taffy](https://crates.io/crates/egui_taffy) | Full Flexbox + CSS grid layout via [Taffy](https://github.com/DioxusLabs/taffy) | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ Active |
| [egui_flex](https://crates.io/crates/egui_flex) | Lightweight flex-row/column layout helper | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ hello_egui |
| [egui_virtual_list](https://crates.io/crates/egui_virtual_list) | Virtualized scrolling list (only renders visible rows) | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ hello_egui |
| [egui_infinite_scroll](https://crates.io/crates/egui_infinite_scroll) | Lazy-loading infinite scroll container | ⭐⭐ | ⭐ | ⭐ | ⭐⭐ hello_egui |

---

## 🪟 Pre-Built Windows & Panels

| Crate | What it does | Utility | Popularity | Maturity | Support |
|-------|-------------|---------|------------|---------|---------|
| [egui-file-dialog](https://crates.io/crates/egui-file-dialog) | Cross-platform in-process file picker (no native dialog) | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ Active |
| [egui_file](https://crates.io/crates/egui_file) | Simpler cross-platform file dialog (older) | ⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐ Less active |
| [egui_logger](https://crates.io/crates/egui_logger) | `log`-crate backend that renders a scrollable log panel | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ Active |
| [egui_modal](https://github.com/n00kii/egui-modal) | Modal dialog builder with backdrop | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ Active |
| [egui_code_editor](https://github.com/p4ymak/egui_code_editor) | Syntax-highlighted code editor panel | ⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ Active |

---

## ✨ Other Visible Additions

| Crate | What it does | Utility | Popularity | Maturity | Support |
|-------|-------------|---------|------------|---------|---------|
| [egui-gizmo](https://crates.io/crates/egui-gizmo) | 3-D transform gizmo widget (translate/rotate/scale) | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ Active |
| [egui-notify](https://crates.io/crates/egui-notify) | Toast notification overlay system | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ Active |
| [egui-toast](https://crates.io/crates/egui-toast) | Alternative toast notification library | ⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ Active |
| [egui_node_graph](https://crates.io/crates/egui_node_graph) | Visual node-graph / flow editor | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ Active |
| [egui_cable](https://crates.io/crates/egui_cable) | Patch-cable connector for node/synth UIs | ⭐⭐ | ⭐ | ⭐ | ⭐ Niche |
| [iconflow](https://crates.io/crates/iconflow) | Icon rendering helper (various icon sets) | ⭐⭐ | ⭐ | ⭐ | ⭐ Community |
| [egui-shadcn](https://github.com/FerrisMind/shadcn-rs) | shadcn/ui-style component library for egui | ⭐⭐ | ⭐ | ⭐ | ⭐ Early |

---

## 🔧 Functionality Extensions

| Crate | What it does | Utility | Popularity | Maturity | Support |
|-------|-------------|---------|------------|---------|---------|
| [egui_router](https://crates.io/crates/egui_router) | SPA-style routing with history for multi-view apps | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ hello_egui |
| [egui_hooks](https://crates.io/crates/egui_hooks) | React-style hooks (`use_state`, `use_effect`) for egui | ⭐⭐ | ⭐ | ⭐ | ⭐ hello_egui |
| [egui_struct](https://crates.io/crates/egui_struct) | Derive macro to auto-generate inspector UI for structs | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ Active |
| [egui_inspect](https://crates.io/crates/egui_inspect) | Similar derive-based inspector (alternative to egui_struct) | ⭐⭐ | ⭐ | ⭐ | ⭐ Less active |
| [egui_hotkey](https://crates.io/crates/egui_hotkey) | Keyboard shortcut/hotkey management | ⭐⭐ | ⭐ | ⭐ | ⭐ Community |
| [egui-async](https://crates.io/crates/egui-async) | Async task integration utilities for egui | ⭐⭐⭐ | ⭐ | ⭐ | ⭐ Early |
| [egui_layout_job_macro](https://crates.io/crates/egui_layout_job_macro) | Macro to construct `LayoutJob` / `TextFormat` more ergonomically | ⭐⭐ | ⭐ | ⭐ | ⭐ Community |
| [egui_material_icons](https://crates.io/crates/egui_material_icons) | Material Design icon font integration | ⭐⭐ | ⭐⭐ | ⭐⭐ | ⭐⭐ hello_egui |

---

## 🎨 Theming

| Crate | What it does | Utility | Popularity | Maturity | Support |
|-------|-------------|---------|------------|---------|---------|
| [catppuccin-egui](https://crates.io/crates/catppuccin-egui) | Catppuccin color scheme (Latte, Frappe, Macchiato, Mocha) | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ Active |
| [egui_colors](https://crates.io/crates/egui_colors) | Color token system for building custom themes | ⭐⭐ | ⭐ | ⭐⭐ | ⭐⭐ Active |
| [egui-theme](https://crates.io/crates/egui-theme) | Theme management with persistence | ⭐⭐ | ⭐ | ⭐ | ⭐ Community |
| [egui-stylist](https://github.com/EmbarkStudios/egui-stylist) | Embark Studios fork — visual style editor | ⭐⭐ | ⭐ | ⭐ | ⭐⭐ Embark |
| [twill](https://github.com/FerrisMind/twill) | Design-token system to keep egui themes structured | ⭐⭐ | ⭐ | ⭐ | ⭐ Early |

---

## 🗃️ Meta-Libraries / Suites

| Crate | What it does | Utility | Popularity | Maturity | Support |
|-------|-------------|---------|------------|---------|---------|
| [hello_egui](https://crates.io/crates/hello_egui) | Monorepo suite: `egui_dnd`, `egui_flex`, `egui_form`, `egui_virtual_list`, `egui_router`, `egui_infinite_scroll`, `egui_pull_to_refresh`, `egui_material_icons`, `egui_hooks` | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ Active maintainer |

---

## 📊 Summary: Pick-by-use-case

| I need… | Reach for… |
|---------|-----------|
| Standard widgets | `egui` + `egui_extras` |
| Plots & charts | `egui_plot` |
| Dockable panels | `egui_dock` or `egui_tiles` |
| Flexbox / CSS grid layout | `egui_taffy` or `egui_flex` |
| Large lists (virtual scroll) | `egui_virtual_list` |
| Drag-and-drop | `egui_dnd` |
| File dialog | `egui-file-dialog` |
| Toast notifications | `egui-notify` |
| Node graph editor | `egui_node_graph` |
| Markdown rendering | `egui_commonmark` |
| Struct inspector | `egui_struct` |
| Log panel | `egui_logger` |
| Nice dark theme | `catppuccin-egui` |
| 3-D gizmos | `egui-gizmo` |
| Routing (multi-view app) | `egui_router` |

---

> **Note:** The egui ecosystem tracks closely with egui's SemVer releases. Always
> check crates.io for the last publish date and confirm the crate depends on the
> same `egui` major version you are using.
>
> Several community crates have no recent releases and may lag behind the current
> egui API. Libraries backed by organizations (Rerun, Embark) or the `hello_egui`
> suite tend to be more reliably maintained.
