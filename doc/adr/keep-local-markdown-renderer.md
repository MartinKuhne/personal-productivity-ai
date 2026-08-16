# Keep the local markdown renderer instead of adopting `egui_commonmark`

Status: accepted (retroactive)
Date: 2026-08-14

## Context

`fastmd` ships a hand-written markdown renderer for its egui UI. It is
composed of several bounded subsystems under `markdown/` and `ui/`:

- `markdown/parser.rs` — a custom `RenderEvent` / `InlineElem` AST built
  on top of `pulldown-cmark` (`parse_markdown_to_events`,
  `render_markdown_to_html`).
- `markdown/document.rs` — the `Document` model: YAML front-matter split,
  a revision counter, and interactive task toggling
  (`toggle_task` / `apply_task_toggle`).
- `markdown/model.rs` — `build_toc` / `ToCEntry` with duplicate-heading
  id disambiguation, plus the right-panel table of contents.
- `markdown/table_width` + `markdown/table_layout` + `ui/table_width` —
  the custom Fair-Table-Width-Algorithm (FTWA) for markdown table column
  layout, with a `DeficitStrategy`, an egui text measurer, and a
  dedicated `benches/table_width.rs` benchmark.
- `ui/render/*` — the egui renderer: `render_markdown`, `render_inline`,
  `render_heading` (scroll-to-me), `code.rs` (code-block copy-to-clipboard
  button), `table/`, and `yaml_table.rs`.

A candidate replacement was evaluated: the `egui_commonmark` crate
(version 0.25.0, published 2026-08-05). Its dependency surface matches
the project exactly (`egui` / `egui_extras` 0.36.0, `pulldown-cmark` 0.13,
edition 2024, MSRV 1.95), so there is no version drift to resolve. It
renders markdown to egui via `CommonMarkViewer::new().show(ui, &mut cache, text)`,
exposes clickable task checkboxes through `.show_mut(...)`, and supports
link-driven scroll-to-heading via `enable_scroll_to_heading(...)`.

Despite the clean version fit, `egui_commonmark` does not cover a number
of behaviours that the local implementation provides. Adopting it would
require retaining or reworking those pieces, and would delete a spec'd,
benchmarked subsystem. On the balance of effort and risk, the local
implementation was kept.

## Decision

Keep the local markdown renderer for now. Do not add `egui_commonmark`
as a dependency and do not remove the local `markdown/` / `ui/render/`
rendering pipeline.

Re-evaluate adoption of `egui_commonmark` if the missing capabilities
below are either provided upstream or explicitly accepted as losses.

### Gaps `egui_commonmark` does NOT cover (critical comparison)

| Capability (local impl) | What it does | `egui_commonmark` 0.25.0 | Impact of adopting |
|-------------------------|--------------|--------------------------|--------------------|
| `render_markdown_to_html` (`markdown/parser.rs`, used by `print.rs`) | Produces HTML for the print flow | Renders egui only; no HTML output | `pulldown-cmark`'s HTML path must stay for print; can't drop the dep |
| YAML front-matter split + `render_yaml_table` (`markdown/document.rs`, `ui/render/yaml_table.rs`) | Strips `---…---` front matter and renders it as a key/value table | Renders `---…---` as a thematic break; no front-matter awareness | Front-matter split + YAML table must stay, fed only `Document::body()` |
| FTWA table column-width algorithm (`markdown/table_width`, `markdown/table_layout`, `ui/table_width`, `benches/table_width.rs`) | Fair-Table-Width column layout with `DeficitStrategy` | Plain `egui_extras` tables (equal/auto widths) | Whole FTWA subsystem + benchmark becomes dead; loses the custom width algorithm |
| `build_toc` / `ToCEntry` + right-panel ToC (`markdown/model.rs`) | Auto heading ids with duplicate disambiguation; right-panel navigation | Link-driven scroll-to-heading using a different `{#id}` syntax, not panel-driven | Needs a wrapper to keep the right-panel ToC UX |
| Code-block copy-to-clipboard button (`ui/render/code.rs`) | Copy button on fenced code blocks | Not provided | Needs a wrapper around the viewer |
| Interactive task-to-source mapping (`Document::toggle_task`, `apply_task_toggle`) | Maps checkbox clicks through a task index, handles front-matter stitching + revision bump | `.show_mut` mutates the source string directly with different semantics | Needs a parity shim to preserve `Document` / front-matter behaviour |
| Event caching (`ui/render/mod.rs`) | Memoizes parsed events keyed by source hash | Has its own `CommonMarkCache` (image store) | Caching must be re-keyed against the viewer's cache |

### Test surface that would need rework if adopted

The following tests assert on the local renderer's shape-level and FTWA
behaviour and would have to be rewritten against viewer output:
`tests/commonmark_spec_test.rs` (608 CommonMark examples),
`tests/pulldown_config.rs`, `tests/table_width_algorithm_test.rs`,
`tests/table_visual_layout_test.rs`, `tests/table_layout_test.rs`, and
the `ui/render/e2e_tests/*` suite (`commonmark_render`,
`commonmark_snapshots`, `commonmark_parser`, `ftwa`, `interactions`,
`table_*`, `render_smoke`, `agent_restyle`).

### Alternatives considered

| Option | Outcome |
|--------|---------|
| **Adopt `egui_commonmark` and delete the local renderer (chosen NOT to pursue now)** | Removes the custom AST + renderer, but loses FTWA and ToC, and requires wrappers for print HTML, front matter, task-to-source mapping, and copy-to-clipboard. Large test rewrite. |
| Adopt `egui_commonmark` for core blocks, keep custom FTWA tables | Keeps the width algorithm but adds integration complexity; still loses ToC and needs the wrappers. |
| Vendor/patch `egui_commonmark` | Fragile; would need to maintain a fork and replicate the missing features. |

## Consequences

- The local markdown renderer, its FTWA subsystem, and the ToC remain
  the source of truth for `fastmd`'s markdown rendering.
- `egui_commonmark` is not added to `Cargo.toml`; no dependency-graph or
  binary-size change.
- The spec'd FTWA behaviour and its benchmark are retained.
- The gap analysis above is recorded so that a future re-evaluation is
  a delta review against this list rather than a fresh investigation.
- If `egui_commonmark` later covers these capabilities upstream (e.g.
  FTWA-style table widths, panel-driven ToC, front-matter handling, or
  HTML export), the decision should be revisited.