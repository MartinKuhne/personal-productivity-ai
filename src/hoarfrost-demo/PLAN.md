# hoarfrost-demo — implementation plan

A standalone runnable demo app that shows off the `hoarfrost` design-system crate:
every live widget it exposes, plus token-resolved previews for the atoms that are
resolver-only. Two panes, a navbar, one close button. No real functionality,
test data only.

---

## 1. Hard constraints

| # | Constraint |
|---|------------|
| C1 | **Every file lives under `D:\ppai2\src\hoarfrost-demo\`.** Never create, edit, delete or move anything outside that folder. In particular `src/hoarfrost/**` and `src/app/**` are read-only reference. |
| C2 | **Fully standalone cargo workspace.** `src/hoarfrost-demo/Cargo.toml` carries its own empty `[workspace]` table so cargo does not walk up to `D:\ppai2`. The root `Cargo.toml` is **not** modified. Run all cargo commands from `D:\ppai2\src\hoarfrost-demo`. |
| C3 | The iced feature set must match `hoarfrost`'s exactly or the re-exported types will not line up: `wgpu`, `tiny-skia`, `fira-sans`, `advanced-shaping`, `advanced`. |
| C4 | No functionality. The only real behaviour is **Close → exit**. Every other callback mutates local demo state so the control still feels alive; nothing persists, nothing reads the filesystem, nothing hits the network. |
| C5 | Test data only, hardcoded in `data.rs`. No user paths, no `%APPDATA%`, no `~/.fastmd*` (`RUST-024`, `RUST-006`). |
| C6 | Repo rules apply: `//!` module doc on every module (`RUST-010`), `///` on every `pub` item (`RUST-011`), repeated/user-facing literals as `const` (`RUST-021`), tests in sibling `<file>_tests.rs` sidecars (`RUST-001`, `RUST-056`), `cargo nextest` only (`RUST-002`). |
| C7 | If a widget cannot be built through hoarfrost's **public** API, **do not modify hoarfrost**. Skip it and record it under `KNOWN GAPS` in `README.md` with the reason. |

### Dependencies

```toml
[dependencies]
hoarfrost = { path = "../hoarfrost" }
iced = { version = "0.14.0", features = [
    "wgpu", "tiny-skia", "fira-sans", "advanced-shaping", "advanced",
] }

[dev-dependencies]
iced_test = "0.14.0"
```

---

## 2. Application shape

```
┌──────────────────────────────────────────────────────────────────┐
│ NavBar  ❄ Hoarfrost Demo   File Edit View Help …        [ Close ] │  36px
├───────────────────────────┬──────────────────────────────────────┤
│  LEFT PANE · Atoms        │  RIGHT PANE · Composites             │
│  ┌──────────────────────┐ │ ┌──────────────────────────────────┐ │
│  │ section: Buttons     │ │ │ section: Tab strip               │ │
│  │ section: Toggles     │ │ │ section: Navigation rail         │ │
│  │ …                    │ │ │ …                                │ │
│  └──────────────────────┘ │ └──────────────────────────────────┘ │
│      (scrollable)         │      (scrollable)                      │
├───────────────────────────┴──────────────────────────────────────┤
│ status bar: last event · widget counter                          │  24px
└──────────────────────────────────────────────────────────────────┘
```

* The two panes are separated by a hoarfrost `SplitDivider` so the splitter is
  itself on display.
* Each pane is a `scrollable` over a column of **sections**. A section is a titled
  card: heading + the widget(s) under test, all spacing/radius/colour from tokens.
* The navbar is `hoarfrost::composite::navbar::NavBar` over a `NavBarModel`.
* The close button is a hoarfrost `Button` (`ButtonVariant::Danger`) wired to
  `iced::exit`.

---

## 3. File layout

```
src/hoarfrost-demo/
├── Cargo.toml
├── README.md                 # how to run + KNOWN GAPS
├── PLAN.md                   # this file
└── src/
    ├── main.rs               # iced::application(boot, update, view) + .theme()
    ├── app.rs                # DemoApp state, Message enum, update(), root view
    ├── app_tests.rs
    ├── chrome.rs               # navbar row, close button, status bar
    ├── chrome_tests.rs
    ├── section.rs              # the titled-card section primitive + section() helper
    ├── section_tests.rs
    ├── pane_atoms.rs           # LEFT pane  — every atom
    ├── pane_atoms_tests.rs
    ├── pane_composites.rs      # RIGHT pane — every composite
    ├── pane_composites_tests.rs
    ├── previews.rs             # resolver-driven static previews (non-interactive)
    ├── previews_tests.rs
    ├── data.rs                 # all test fixtures
    ├── data_tests.rs
    ├── theme.rs                # hoarfrost Palette -> iced::Theme::custom
    └── theme_tests.rs
```

`main.rs` stays a thin entry point; state, view and data stay separated so no file
approaches the 4096-line limit (`RUST-053`).

---

## 4. Widget distribution

> The table is the **target list**, not a promise. Verify each constructor against
> the hoarfrost source before using it. Anything that cannot be built publicly is
> skipped + recorded (C7).

### 4.1 Left pane — atoms

| Section | Widget / API | Kind |
|---|---|---|
| Buttons | `widget::button_widget::Button::new(variant, content, msg, focus_id)` — Primary / Secondary / Ghost / Danger × enabled / disabled / busy | live |
| Checkbox | `widget::toggle_widget::Checkbox` + `toggle_model::CheckboxModel` — off / on / indeterminate / disabled | live |
| Radio group | `widget::toggle_widget::RadioGroup` + `RadioGroupModel` / `RadioOption` / `RadioOrientation` | live |
| Toggle switch | `widget::toggle_widget::ToggleSwitch` — on / off / busy / disabled | live |
| Segmented | `widget::segmented::SegmentedButton` + `segmented_model::SegmentedModel` | live |
| Split button | `widget::split_button::SplitButton` + `split_button_model::SplitButtonModel` | live |
| Hyperlink | `widget::hyperlink::Hyperlink` + `hyperlink_model::HyperlinkModel` | live |
| Text input | `widget::text_input::TextInput::new(kind, placeholder, value, on_input, focus_id)` — normal / search / invalid / disabled / read-only | live |
| Password | `widget::password_model` + `password_host` (or the `TextInput` secret kind) | live |
| Combo box | `widget::combo_box(&model)` + `combo_model::ComboBoxModel` / `SelectOption` | live |
| Search box | `widget::search_box::SearchBox` + `SearchBoxModel` | live |
| Rating | `widget::rating::rating_control(model)` + `RatingModel` / `RatingStep` | live |
| Range slider | `widget::range_slider::range_slider(model)` + `RangeSliderModel` | live |
| Avatar | `widget::avatar::avatar(model)` — shape × size grid | live |
| Date | `widget::date_picker::{date_picker, date_range_picker, calendar_view}` | live |
| Time | `widget::time_picker::time_picker(model)` + `TimePickerModel` | live |
| Colour | `widget::color_picker::{color_picker, color_swatch_button}` | live |
| Tooltip | `surface::tooltip::tooltip(element, text)` | live |
| Chip | `widget::chip::resolve_style` — Assist / Filter(off) / Filter(on) / Input | preview |
| Badge | `widget::badge::{resolve_tone, …}` — Dot / Count / Status | preview |
| Progress | `widget::progress::*` + `progress_model::ProgressModel` — determinate / indeterminate / statuses | preview |
| Slider | `widget::slider::*` — single-thumb rail | preview |
| Number box | `widget::number_box::*` — stacked + side-by-side steppers | preview |

### 4.2 Right pane — composites

| Section | Widget / API | Kind |
|---|---|---|
| Tab strip | `composite::tab_strip::TabStrip` + `TabStripModel` / `TabItem` | live |
| Navigation rail | `composite::navigation_rail::navigation_rail(model)` + `NavigationRailModel` / `NavDestination` | live |
| Stepper | `composite::stepper::{stepper, wizard_footer}` + `StepperModel` / `StepItem` / `StepStatus` | live |
| Pagination | `composite::pagination::pagination_bar(model, on_change, on_size)` | live |
| Pips pager | `composite::pips_pager::pips_pager(model, on_change)` | live |
| Code block | `composite::code_block::code_block(model)` + `CodeBlockModel` | live |
| Diff viewer | `composite::diff_viewer::diff_viewer(model)` — unified + side-by-side | live |
| Empty state | `composite::empty_state::empty_state(model, on_action)` — empty / no-match / error | live |
| Grid view | `composite::grid_view::grid_view(model, width)` + `GridTile` | live |
| Keybind recorder | `composite::keybind_recorder::keybind_recorder(model)` + `KeyChord` | live |
| Skeleton | `composite::skeleton::skeleton(w, h)` | live |
| Split divider | `composite::split_divider::split_divider(model)` | live |
| Command palette | `composite::command_palette::command_palette(model)` — inline card | live |
| Side sheet | `composite::side_sheet::side_sheet(...)` — inline docked | live |
| Form field | `composite::form_field::stacked(header, control, caption)` | live |
| Settings row | `composite::settings_row::horizontal(...)` | live |
| Menu | `surface::menu::view` rows over a `menu::model::Menu` | live |
| Card | `composite::card::resolve_style` — resting / elevated | preview |
| List tile | `composite::list_tile::*` — single-line / two-line, selected | preview |
| Table | `composite::{table_column, table_cell, table_header}` resolvers | preview |
| Toast / notification | `composite::{toast, notification}` resolvers | preview |
| Breadcrumb | `composite::breadcrumb::*` — elided trail | preview |
| Overflow toolbar | `composite::overflow_toolbar::fit` — inline vs overflow plan | preview |

---

## 5. Incremental steps

Every step ends with the **gate**. A step is not done until its gate is green.
All commands run from `D:\ppai2\src\hoarfrost-demo`.

**Gate (G)** =
```
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo nextest run --status-level fail --show-progress none
```

| Step | Deliverable | Extra validation |
|---|---|---|
| **S0** | `Cargo.toml` (standalone workspace) + `main.rs` that opens a window with a "Hello" text and a working Close button. | `cargo check` green; app builds. |
| **S1** | `theme.rs` (hoarfrost `Palette::dark()` → `iced::Theme::custom`), `chrome.rs` (real `NavBar` + Close), `section.rs`, two-pane shell with `SplitDivider` + scrollables, `data.rs` skeleton. | Headless smoke test: `iced_test::simulator(root_view)` renders one frame without panic. |
| **S2** | Left pane, group 1: buttons, checkbox, radio group, toggle switch, segmented, split button, hyperlink. | Smoke test renders the pane at 480×900. |
| **S3** | Left pane, group 2: text input, password, combo box, search box. | same |
| **S4** | Left pane, group 3: rating, range slider, avatar, date picker, time picker, colour picker, tooltip. | same |
| **S5** | Right pane, group 1: tab strip, navigation rail, stepper + wizard footer, pagination, pips pager. | same |
| **S6** | Right pane, group 2: code block, diff viewer, empty state, grid view, keybind recorder, skeleton, side sheet, command palette, form field, settings row, menu. | same |
| **S7** | `previews.rs`: resolver-driven static previews for the resolver-only atoms/composites, wired into both panes. | Preview helpers unit-tested against their resolvers. |
| **S8** | `README.md` (run instructions, screenshot-free, + `KNOWN GAPS`), doc pass, final full gate. | `cargo doc --no-deps --quiet` builds clean. |

If a step is blocked by a hoarfrost API that is not publicly reachable, drop that
one widget, note it, and continue — never widen the blast radius.

---

## 6. Definition of done

* `cargo run -p` equivalent: `cargo run` from `src/hoarfrost-demo` opens a window
  with navbar, two panes and a working Close button.
* Every live widget in §4 renders; every preview in §4 renders.
* All four gate commands green, plus `cargo doc --no-deps --quiet`.
* `git status` shows changes **only** under `src/hoarfrost-demo/`.
* `README.md` lists anything skipped and why.
