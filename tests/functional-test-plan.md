# Plan: Functional Tests for Markdown → egui Rendering Pipeline

## 1. Objective

Create functional tests that exercise the full markdown → egui rendering pipeline against the CommonMark spec 0.31.2. Each of the 652 examples in `tests/collateral/commonmark.md` will have a corresponding test that validates rendering correctness.

## 2. Pipeline Inspection

The pipeline has two layers, each testable independently:

```
Markdown text
    │
    ▼
┌─────────────────────────┐
│ markdown::parser        │  parse_markdown_to_events() → Vec<RenderEvent>
│ (pulldown-cmark backend)│
└─────────────────────────┘
    │
    ▼
┌─────────────────────────┐
│ ui::render              │  render_markdown() → egui widgets
│ (egui output shapes)    │  FlushInline, CodeBlock, Heading, Table, Space, Separator
└─────────────────────────┘
    │
    ▼
egui::FullOutput { shapes, text_input, commands, ... }
```

### 2.1 What's Available for Testing

**The `RenderEvent` AST is available** — `parse_markdown_to_events()` returns a `Vec<RenderEvent>` that can be inspected structurally in unit tests. This is already exercised by `ui/render/tests.rs`.

**The egui output shapes are available** — `egui::Context::default()` + `ctx.run_ui()` + `run_ui_test()` produces `egui::FullOutput` with `shapes: Vec<Shape>`. Text shapes contain `galley.rows` for layout inspection. This is already exercised by `ui/render/e2e_tests/`.

**egui_kittest snapshots are blocked** — `egui_kittest` is a dev-dependency but the `wgpu` feature is **not enabled** on Windows due to a `wgpu-hal 0.29 / windows-core` trait-bound conflict (`doc/distill/egui-kittest.md`, `ui/test_helpers/snapshot.rs`). Until resolved, `harness.snapshot()` returns `SnapshotError::RenderError`. The snapshot helper exists and exercises the full API surface.

### 2.2 Recommended Approach: Three-Tier Testing

| Tier | What it tests | Mechanism | Example location |
|------|--------------|-----------|-----------------|
| **Tier 1 — Parser AST** | `RenderEvent` structural correctness | Unit test, `parse_markdown_to_events()` + structural assertions on `RenderEvent` enum | `ui/render/tests.rs`, `markdown/parser.rs` |
| **Tier 2 — egui shape inspection** | Widget hierarchy, text wrapping, layout bounds | `run_ui_test()` + `egui::FullOutput.shapes` + `extract_text()` | `ui/render/e2e_tests/render_smoke.rs` |
| **Tier 3 — egui_kittest snapshots** | Visual regression (pixel-level) | `egui_kittest::Harness` + `harness.snapshot()` | TBD (blocked on wgpu) |

**Tier 1 and Tier 2 are immediately available.** Tier 3 is planned for when wgpu support is enabled.

## 3. Test Categorization by CommonMark Section

The 652 examples are grouped into these sections. Each section needs a different test strategy based on what "correct rendering" means:

| Section | Examples | Test Strategy |
|---------|----------|---------------|
| **2.1 Tabs** | 1-11 | Tier 1: verify tab → space conversion in code blocks; Tier 2: verify code block text content |
| **2.4 Backslash escapes** | 12-24 | Tier 1: verify escaped characters appear as plain text; Tier 2: verify text shapes |
| **2.5 Entity refs** | 25-41 | Tier 1: verify entity resolution; Tier 2: verify text shapes |
| **3.1 Precedence** | 42-43 | Tier 1: verify event structure (list vs code span) |
| **4.1 Thematic breaks** | 43-61 | Tier 2: verify `egui::Separator` widget is emitted; Tier 1: verify `Separator` event |
| **4.2 ATX headings** | 62-79 | Tier 2: verify heading widget emits correct font size; Tier 1: verify `Heading` events with correct level |
| **4.3 Setext headings** | 80-112 | Tier 2: same as ATX; Tier 1: verify `Heading` events |
| **4.4 Indented code blocks** | 113-125 | Tier 2: verify monospace text in code block widget; Tier 1: verify `CodeBlock` events |
| **4.5 Fenced code blocks** | 126-236 | Tier 2: same as indented; Tier 1: verify `CodeBlock` events; verify info string parsing |
| **4.6 HTML blocks** | 237-295 | Tier 2: verify raw HTML rendered as `InlineElem::Html` text; Tier 1: verify no inline elements |
| **4.7 Link ref defs** | 296-330 | Tier 1: verify link references are registered; verify resolved links render |
| **4.8 Paragraphs** | 331-353 | Tier 2: verify text shapes; verify spacing between paragraphs |
| **4.9 Blank lines** | 354-356 | Tier 2: verify vertical spacing |
| **5.1 Block quotes** | 357-388 | Tier 2: verify nested panel/border widget; Tier 1: verify `FlushInline` events are grouped |
| **5.2 List items** | 389-469 | Tier 2: verify bullet indicator widget + text; Tier 1: verify `needs_bullet`, `indent`, `list_ordinal` fields |
| **5.3 Lists** | 470-539 | Tier 2: verify list nesting via `indent` field; Tier 1: verify `list_depth` tracking |
| **6.1 Code spans** | 540-574 | Tier 2: verify monospace text with background; Tier 1: verify `InlineElem::Text` with `code: true` |
| **6.2 Emphasis** | 575-753 | Tier 2: verify font weight/italic in text shapes; Tier 1: verify `TextStyle` flags |
| **6.3 Links** | 754-859 | Tier 2: verify `egui::Link` widget with clickable response; Tier 1: verify `InlineElem::Link` |
| **6.4 Images** | 860-864 | Tier 2: verify image placeholder renders; Tier 1: verify `InlineElem::Image` |
| **6.5 Autolinks** | 865-876 | Tier 2: verify clickable link; Tier 1: verify `InlineElem::Link` with URL as text |
| **6.6 Raw HTML** | 877-910 | Tier 2: verify HTML text renders as plain text in egui; Tier 1: verify `InlineElem::Html` |
| **6.7 Hard line breaks** | 911-921 | Tier 2: verify separate text rows in galley; Tier 1: verify `HardBreak` → `FlushInline` flush |
| **6.8 Soft line breaks** | 922-927 | Tier 2: verify single row wrapping; Tier 1: verify `SoftBreak` → space in text |
| **6.9 Textual content** | 928-935 | Tier 2: verify text shapes for emoji, unicode, whitespace |

## 4. Implementation Plan

### Phase 1: Parser-level tests (Tier 1) — immediately actionable

**Location**: `tests/render/e2e_tests/commonmark_parser.rs` (new file in `ui/render/e2e_tests/`)

For each CommonMark example `[CM-XXX]`, create a test that:

1. Reads the markdown input
2. Calls `parse_markdown_to_events(md)`
3. Asserts structural properties of the `RenderEvent` output

**Example pattern:**
```rust
#[test]
fn cm_062_atx_headings() {
    let md = "# foo\n## foo\n### foo\n#### foo\n##### foo\n###### foo";
    let events = parse_markdown_to_events(md);

    let headings: Vec<_> = events.iter()
        .filter_map(|e| match e {
            RenderEvent::Heading { level, elems } => {
                Some((*level, heading_plain_text(elems)))
            }
            _ => None,
        })
        .collect();

    assert_eq!(headings, vec![
        (1, "foo"), (2, "foo"), (3, "foo"),
        (4, "foo"), (5, "foo"), (6, "foo"),
    ]);
}

#[test]
fn cm_043_thematic_breaks() {
    let md = "***\n---\n___";
    let events = parse_markdown_to_events(md);

    let separators: Vec<_> = events.iter()
        .filter_map(|e| matches!(e, RenderEvent::Separator).then_some(()))
        .collect();
    assert_eq!(separators.len(), 3);
}
```

**Scope**: 652 examples. Prioritize by section complexity. Start with:
- Thematic breaks (19 examples)
- ATX headings (18 examples)
- Setext headings (33 examples)
- Code blocks (111 examples)
- Emphasis (179 examples)

### Phase 2: egui shape inspection tests (Tier 2) — immediately actionable

**Location**: `tests/render/e2e_tests/commonmark_render.rs` (new file in `ui/render/e2e_tests/`)

For each CommonMark example, create a test that:

1. Calls `render_markdown(ui, md, &mut scroll, &mut toggles, strategy, None)` inside `run_ui_test()`
2. Inspects `egui::FullOutput.shapes`
3. Uses `extract_text(&output.shapes)` for text verification

**Example pattern:**
```rust
use crate::ui::test_helpers::text::extract_text;

#[test]
fn cm_062_atx_headings_render() {
    let md = "# foo\n## bar";
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO, egui::vec2(800.0, 600.0),
        )),
        ..egui::RawInput::default()
    };
    let mut output = run_ui_test(&ctx, raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_markdown(ui, md, &mut None, &mut Vec::new(),
                DeficitStrategy::ProportionalToSlack, None);
        });
    });
    output.textures_delta.clear();

    let texts = extract_text(&output.shapes);
    assert!(texts.iter().any(|t| t == "foo"), "H1 'foo' not rendered");
    assert!(texts.iter().any(|t| t == "bar"), "H2 'bar' not rendered");
}
```

**Key verifications per section:**
- **Headings**: text content present; font size matches level (inspect `egui::TextFormat` in text shapes)
- **Code blocks**: monospace font flag on text shapes; text content preserved
- **Thematic breaks**: `egui::Shape::Stroke` horizontal line present
- **Lists**: bullet indicator widget + text; correct `indent` in `FlushInline` events
- **Emphasis**: `TextStyle::bold` / `TextStyle::italic` reflected in text shape `TextFormat`
- **Links**: `egui::OutputCommand::OpenUrl` on click (inspect `output.commands`)
- **Tables**: grid shapes with correct row/column structure

### Phase 3: Snapshot tests (Tier 3) — blocked on wgpu

**Location**: `tests/render/e2e_tests/commonmark_snapshots.rs` (new file)

Once wgpu feature is enabled on `egui_kittest`:
```rust
use crate::ui::test_helpers::snapshot::snapshot_harness;

#[test]
#[ignore = "requires wgpu feature on egui_kittest"]
fn cm_062_atx_headings_snapshot() {
    let mut harness = snapshot_harness("cm_062_atx_headings", DEFAULT_VIEWPORT, |ui| {
        render_markdown(ui, "# foo\n## bar", &mut None, &mut Vec::new(),
            DeficitStrategy::ProportionalToSlack, None);
    });
    harness.run();
    harness.snapshot("cm_062_atx_headings");
}
```

**Snapshot strategy:**
- Take initial snapshots for a representative subset (~50 examples covering all sections)
- Run full suite (652) in CI as regression guard
- Use `3.0` threshold (`SNAPSHOT_THRESHOLD`) for cross-platform font tolerance

### Phase 4: Property-based fuzzing (bonus)

Extend the existing `test_parse_markdown_fuzz_property` in `ui/render/tests.rs:809` to cover CommonMark edge cases:
- Delimiter runs in unexpected contexts
- Nested lists at arbitrary depth
- Mixed block quote + list + heading combinations
- Entity references in URLs and link titles

## 5. Test Data Generation Strategy

Rather than hardcoding 652 markdown inputs, generate test cases from `tests/collateral/commonmark.md`:

```rust
// tests/render/e2e_tests/commonmark_examples.rs (generated)
// This file is auto-generated from tests/collateral/commonmark.md
// DO NOT EDIT MANUALLY

/// Returns all CommonMark examples as (id, markdown_input) tuples.
pub fn all_examples() -> Vec<(&'static str, &'static str)> {
    vec![
        ("CM-001", "\u{2192}foo\u{2192}baz\u{2192}\u{2192}bim"),
        ("CM-002", "  \u{2192}foo\u{2192}baz\u{2192}\u{2192}bim"),
        // ... all 652 examples
    ]
}
```

**Generator script**: A one-off `scripts/gen_commonmark_tests.py` (or Rust `build.rs`) reads `tests/collateral/commonmark.md` and emits `commonmark_examples.rs`. This keeps test cases DRY and easy to update when the spec changes.

## 6. File Structure

```
src/desktop/src/ui/render/e2e_tests/
├── mod.rs              (existing — re-exports, submodule declarations)
├── helpers.rs          (existing — shared test helpers)
├── ftwa.rs             (existing)
├── interactions.rs     (existing)
├── render_smoke.rs     (existing)
├── table_alignment.rs  (existing)
├── table_regressions.rs (existing)
├── commonmark_parser.rs   (NEW — Tier 1 parser AST tests)
├── commonmark_render.rs   (NEW — Tier 2 egui shape inspection)
└── commonmark_snapshots.rs (NEW — Tier 3 snapshots, #[ignore] until wgpu)
```

## 7. Prioritization

| Priority | What | Why |
|----------|------|-----|
| **P0** | Tier 1: Thematic breaks, ATX headings, setext headings | Simple events, high coverage with few examples |
| **P0** | Tier 1: Code blocks (indented + fenced) | Most examples (~111), complex edge cases |
| **P1** | Tier 1: Emphasis (bold, italic, strikethrough) | 179 examples, most complex inline parsing |
| **P1** | Tier 2: Headings, code blocks, thematic breaks | Verifies rendering matches parser output |
| **P2** | Tier 1: Lists, block quotes, paragraphs | Medium complexity |
| **P2** | Tier 2: Links, code spans, autolinks | Verifies interactive widget behavior |
| **P3** | Tier 1: All remaining examples | Complete CommonMark coverage |
| **P3** | Tier 2: All remaining examples | Full rendering verification |
| **TBD** | Tier 3: Snapshot tests | Blocked on wgpu feature |

## 8. Quality Gates

Per `src/desktop/AGENTS.md` §6, all new test files must pass:
- `cargo check --quiet` — no errors or warnings
- `cargo nextest run --status-level fail --show-progress none` — all tests pass
- `cargo clippy -- -D warnings` — no lint warnings
- `cargo fmt --check` — code is properly formatted
- `cargo doc --no-deps --quiet` — documentation builds without warnings

## 9. Risk Assessment

| Risk | Mitigation |
|------|-----------|
| wgpu feature never enabled on Windows | Tier 3 tests stay `#[ignore]`; Tier 1 + Tier 2 provide full coverage |
| 652 tests slow down CI | Tier 1 tests are pure Rust, very fast; Tier 2 uses `Context::default()` which is also fast; CI profile in `.config/nextest.toml` already handles this |
| Font metric variation across platforms | Tier 1 tests are platform-independent; Tier 2 tests assert text content, not pixel positions |
| pulldown-cmark upgrade changes parser behavior | Existing `cmark_strikethrough_fragments_single_tilde` pattern — pin upstream behavior |
| Test file exceeds 4096 lines | Split by section (e.g., `commonmark_headings.rs`, `commonmark_lists.rs`, etc.) |
