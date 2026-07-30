# Render Architecture Proposal

* **Status:** proposal
* **Date:** 2026-07-26
* **Deciders:** Architecture Review Board
* **Replaces:** Monolithic `ui::render` design ([`src/desktop/src/ui/render.rs`](file:///C:/Users/mkuhn/src/ppai/src/desktop/src/ui/render.rs))
* **Requirements Traceability:** [REQ-101], [REQ-102], [REQ-103], [MD-001], [MD-002], [MD-003], [MD-004], [MD-005]

---

## 1. Context

The desktop viewer (`fastmd`) renders Markdown files, YAML front-matter, tables, blockquotes, code blocks, and agent responses using `egui` and `pulldown-cmark`. Currently, the rendering implementation is centralized in [`src/desktop/src/ui/render.rs`](file:///C:/Users/mkuhn/src/ppai/src/desktop/src/ui/render.rs).

While functioning, the current architecture suffers from three structural limitations:

1. **Tight Coupling of AST Parsing and GUI Painting:**
   `parse_markdown_to_events` runs synchronously inside `render_markdown` on **every single frame repaint** (60+ FPS). Parsing raw string bytes into event vectors during repaints introduces unnecessary CPU overhead and heap churn.
2. **GUI Context Dependency in Layout Mathematics:**
   Table column width math (FTWA, [`src/desktop/src/ui/table_width/mod.rs`](file:///C:/Users/mkuhn/src/ppai/src/desktop/src/ui/table_width/mod.rs)) and block height calculations are executed directly inside `egui::Ui` layout closures. Testing layout geometry and table wrapping math currently requires initializing mock `egui::Context` instances.
3. **State Mutation vs. Render Desynchronization:**
   Task list checkbox toggles ([MD-001]) were historically applied via line-by-line string regex searches ([`apply_task_toggle`](file:///C:/Users/mkuhn/src/ppai/src/desktop/src/ui/render.rs#L1180)), creating risks of desynchronization between parsed AST task indices and string offsets.

---

## 2. Decision

We propose refactoring the rendering subsystem into a **4-stage decoupled pipeline**:

```
[Markdown String] ---> (Stage 1: AST Parser) ---> [MarkdownDoc AST]
                                                         |
                                                         v
                                              (Stage 2: Document Model & Cache)
                                                         |
                                                         v
[Viewport Rect]  ---> (Stage 3: Layout Engine) ---> [LayoutTree]
                                                         |
                                                         v
                                              (Stage 4: egui View Painter) ---> [egui::Ui Output]
```

### Stage 1: Pure AST Subsystem (`markdown::ast`)
* **Location:** `src/desktop/src/markdown/ast.rs`
* **Purity:** 100% Pure Rust, Zero `egui` or GUI dependencies.
* **Responsibility:** Wraps `pulldown_cmark` with GFM options ([MD-001]). Converts markdown source into a strongly typed, immutable AST (`MarkdownDoc`) with exact source byte ranges (`Range<usize>`) for every node.

### Stage 2: Document State & Reactive Cache (`markdown::document`)
* **Location:** `src/desktop/src/markdown/document.rs` & `src/desktop/src/ui/cache.rs`
* **Responsibility:** Owns document revisioning, table of contents extraction ([`build_toc`](file:///C:/Users/mkuhn/src/ppai/src/desktop/src/ui/render.rs#L1256)), and byte-range mutation.
* **Mutation Rule:** Task checkbox toggling operates on exact byte ranges (`task.marker_range`), guaranteeing verbatim preservation of un-edited content and line endings (`\r\n` / `\n`).
* **Cache Strategy:** `MarkdownDoc` is cached on `TabManager` keyed by `(source_hash, revision)`. Repaints of unchanged documents bypass Stage 1 entirely.

### Stage 3: Headless Layout & Measurement Engine (`markdown::layout`)
* **Location:** `src/desktop/src/markdown/layout.rs`
* **Responsibility:** Computes bounding rectangles, line wrap points, blockquote bar heights, and FTWA column widths given an available viewport width.
* **Testability:** Can be run in headless unit tests to verify column allocations, token-break invariants, and spacing bounds without initializing an `egui::Context`.

### Stage 4: Thin egui View Painter (`ui::render::painter`)
* **Location:** `src/desktop/src/ui/render/painter.rs`
* **Responsibility:** Draws pre-computed `LayoutTree` nodes to `egui::Ui`.
* **Behavior:** Side-effect-free view layer. Reads calculated rects and emits `egui` labels, links, and grid lines.

---

## 3. Detailed Data Contracts

### 3.1 Document AST (`MarkdownDoc`)

```rust
pub struct MarkdownDoc {
    pub blocks: Vec<BlockNode>,
    pub source_hash: u64,
}

pub enum BlockNode {
    Heading {
        level: u32,
        elems: Vec<InlineNode>,
        heading_id: String,
    },
    Paragraph {
        elems: Vec<InlineNode>,
    },
    ListItem {
        indent: usize,
        ordinal: Option<u64>,
        task: Option<TaskMarker>,
        elems: Vec<InlineNode>,
    },
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    Table {
        headers: Vec<Vec<InlineNode>>,
        rows: Vec<Vec<Vec<InlineNode>>>,
        ordinal: usize,
    },
    BlockQuote {
        depth: usize,
        blocks: Vec<BlockNode>,
    },
    Separator,
    Space(f32),
}

pub struct TaskMarker {
    pub index: usize,
    pub checked: bool,
    pub byte_range: std::ops::Range<usize>,
}
```

### 3.2 Layout Tree (`LayoutTree`)

```rust
pub struct LayoutTree {
    pub nodes: Vec<LayoutNode>,
    pub size: egui::Vec2,
}

pub struct LayoutNode {
    pub bounds: egui::Rect,
    pub payload: LayoutPayload,
}

pub enum LayoutPayload {
    InlineSpans(Vec<InlineSpanLayout>),
    CodeBlock { text: String },
    Table { widths: Vec<f32>, needs_scroll: bool },
    BlockQuoteBar { depth: usize, height: f32 },
}
```

---

## 4. Consequences

### Positive
* **Decoupled Architecture:** Clean separation between Markdown syntax rules (`markdown/`), spatial math (`markdown/layout`), and GUI rendering (`ui/render`).
* **Deterministic Testability:** AST parsing, task toggles, TOC extraction, and FTWA table layout math can be tested 100% in pure unit tests without GUI context overhead.
* **Zero-Allocation Repaints:** Frame updates for static open documents perform zero string tokenization passes and zero heap re-allocations.
* **Verifiable Correctness:** Mathematical invariants (e.g. FTWA token break bounds, blockquote bar heights, task offset mutation) can be verified automatically via property tests (`proptest`).

### Risks & Mitigations
* **Risk:** Temporary memory footprint for caching `LayoutTree` alongside `MarkdownDoc`.
  * *Mitigation:* `LayoutTree` is lightweight (a few KB per document) and evicted when document tabs are closed.
* **Risk:** Migration complexity across existing `ui/render.rs` call sites.
  * *Mitigation:* Maintain facade functions (`render_markdown`, `render_yaml_table`, `build_toc`) while incrementally migrating internal stages.

---

## 5. Formal Quality & Verification Strategy

| Verification Tier | Focus Area | Harness / Tools | Invariant Asserted |
| :--- | :--- | :--- | :--- |
| **Tier 1: AST & Mutation** | Parsing, task toggling, TOC | Pure Rust (`cargo test`) | AST range mutation preserves source verbatim & CRLF |
| **Tier 2: Layout Geometry** | FTWA column math, block bounds | Headless Layout Engine | `width >= min_content` OR `needs_horizontal_scroll` |
| **Tier 3: View Snapshot** | Checkbox click, copy button, link | `egui_kittest` | Widget clicks emit correct platform output & state toggle |

---

## 6. Implementation Roadmap

1. **Phase 1: AST Extraction (`markdown::ast`)**
   Move AST types and `pulldown_cmark` parser into `src/desktop/src/markdown/ast.rs`.
2. **Phase 2: Document Model & Caching (`markdown::document`)**
   Wrap open document state in `DocumentModel` and cache AST on `TabManager`.
3. **Phase 3: Headless Layout Engine (`markdown::layout`)**
   Extract spatial layout math into pure layout structures.
4. **Phase 4: egui Painter & Facade Clean-up (`ui::render::painter`)**
   Refactor `ui/render.rs` to delegate to painter adapter.
