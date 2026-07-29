# Feature Specification: Table Layout and Renderer Subsystem

**Feature Branch**: `002-table-layout-renderer`

**Created**: 2026-07-28

**Status**: Draft

**Input**: User description: "implement requirements in /src/desktop/src/ui/SPEC.md" — i.e., the Table Layout and Renderer Subsystem specified in `src/desktop/src/ui/SPEC.md`, which defines the functional, algorithmic, and architectural requirements for computing table layout geometry, sizing constraints, and visual representation of structured tabular data.

**Authoritative Source**: The existing `src/desktop/src/ui/SPEC.md` (sections 2–6, requirement ids `TBL-001`…`TBL-51`) is the requirements source of record. This feature spec re-expresses those requirements as user-valuable capabilities for planning and acceptance. Where the two documents disagree, the `SPEC.md` requirement ids govern the behaviour; this spec governs scope, prioritisation, and acceptance.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - View a Markdown table rendered to fit the available width (Priority: P1)

A reader opens a Markdown document containing one or more tables. The System computes the available width from the surrounding view, measures each column's minimum and preferred width, and renders the table so that it fits within the available width without truncating text. When the column's preferred widths fit, the table uses those preferred widths; when they do not fit, columns shrink down to (but never below) their minimum widths so the reader can still see every cell's content, with text wrapping onto subsequent lines as needed.

**Why this priority**: This is the foundational, MVP-grade capability of the System. Without correct width-fitting and text wrapping, no other layout, decoration, or performance work is meaningful — a reader cannot view any table at all. It directly satisfies `TBL-001`, `TBL-010`, `TBL-011`, `TBL-012`, `TBL-013`, `TBL-020`.

**Independent Test**: A document containing a single Markdown table can be opened and the rendered table must appear fully within the available width, with the column widths matching the fitting rule (preferred widths when they fit, otherwise shrunk-to-fit above the minimum), and with cell text wrapping to additional lines rather than being clipped.

**Acceptance Scenarios**:

1. **Given** a Markdown table whose total preferred column width is less than or equal to the available width, **When** the reader views the document, **Then** every column is rendered at its preferred width and no text is wrapped or truncated.
2. **Given** a Markdown table whose total preferred column width exceeds the available width but whose total minimum width is less than or equal to the available width, **When** the reader views the document, **Then** columns are shrunk so the table fits the available width and no column is reduced below its minimum content width.
3. **Given** a Markdown table whose total minimum column width exceeds the available width, **When** the reader views the document, **Then** no column is reduced below its own minimum content width and the overflow is surfaced to the reader (see User Story 4 for the horizontal-scroll fallback).
4. **Given** a cell whose content is longer than the allocated column width, **When** the table is rendered, **Then** the text wraps onto subsequent lines within the column rather than being clipped or obscured.
5. **Given** a cell containing a single continuous word longer than the allocated column width, **When** the table is rendered, **Then** the line break falls at whitespace where possible (`TBL-021`) and, only when no whitespace break is available, the fallback behaviour of User Story 4 applies (`TBL-022`).

---

### User Story 2 - Render formatted (markdown) cell content, not just plain text (Priority: P2)

A reader opens a Markdown document whose table cells contain inline markdown formatting (emphasis, code, links, etc.). The System renders the cell content using the Markdown specification, not merely as opaque plain text, so the reader sees the intended formatting inside each cell.

**Why this priority**: Above story 1 this is the capability that distinguishes a Markdown table renderer from a generic table renderer. It satisfies `TBL-002` (plain text) and `TBL-003` (markdown content), but is lower priority than story 1 because a table with plain-text cells is still readable, whereas a table that does not fit cannot be read at all.

**Independent Test**: A document containing a Markdown table with formatted cell content is rendered with the formatting visible (e.g., emphasised text appears emphasised), and the layout still honours the width-fitting rule from User Story 1.

**Acceptance Scenarios**:

1. **Given** a Markdown table whose cells contain plain text only, **When** the reader views the document, **Then** the plain text is rendered correctly (`TBL-002`).
2. **Given** a Markdown table whose cells contain inline markdown formatting, **When** the reader views the document, **Then** the formatting is rendered according to the Markdown specification so the reader perceives the intended emphasis, code spans, links, etc. (`TBL-003`), and the cell's measured widths and wrapping still honour User Story 1.

---

### User Story 3 - Render tables with consistent alignment, padding, and borders (Priority: P3)

A reader views a rendered table and sees consistent horizontal alignment (LEFT) and vertical alignment (TOP) within every cell, configurable inner cell padding that does not break width or height calculations, and visually clean border decoration — a medium-gray border around the table perimeter, dark-gray borders between adjacent cells, and clean junctions at border intersections (no double-drawn or ragged grid lines). The reader can change padding at global, per-column, or per-cell level and the layout adjusts correctly.

**Why this priority**: Decoration and alignment polish the readable output produced by stories 1 and 2; they are visible to the reader but do not block reading. They satisfy `TBL-030`…`TBL-033` and `TBL-040`…`TBL-042`.

**Independent Test**: A Markdown table is rendered with the required alignment, configurable padding that is correctly factored into width and height calculations, and the required border styling; switching padding at the three configurable levels visibly changes the rendered output without breaking layout.

**Acceptance Scenarios**:

1. **Given** any rendered table, **When** the reader views a cell, **Then** the cell content is horizontally aligned LEFT (`TBL-030`) and vertically aligned to the TOP (`TBL-031`).
2. **Given** inner cell padding is configured at global, per-column, or per-cell level, **When** the table is rendered, **Then** the configured padding is applied (`TBL-032`) and every column width and row height calculation accounts for the configured padding (`TBL-033`).
3. **Given** a rendered table, **When** the reader views it, **Then** the table perimeter is drawn with medium-gray border styling (`TBL-040`) and adjacent cells are separated by dark-gray border styling (`TBL-041`).
4. **Given** a rendered table where two or more cell borders intersect, **When** the reader views the intersection, **Then** the borders are collapsed so the junction renders cleanly without ragged or doubled grid lines (`TBL-042`).

---

### User Story 4 - Handle overflow gracefully and avoid masking content unless clipping is explicitly requested (Priority: P3)

A reader encounters a table that cannot fit the available width even at minimum column widths, or a cell containing a single continuous word that cannot wrap. Rather than silently clipping or truncating the text, the System falls back to horizontal scrolling so the reader can reach the obscured content; it never visually truncates or obscures text unless the reader has explicitly configured clip-overflow behaviour.

**Why this priority**: Robustness/edge-case handling. It satisfies `TBL-013`'s overflow case, `TBL-022`'s unbreakable-word fallback, and `TBL-043`'s no-masking rule. It is P3 because it only triggers when stories 1 and 2 have already done their work and an overflow condition exists.

**Independent Test**: A Markdown table whose total minimum width exceeds the available width (or a cell containing a single word longer than its column width) is rendered so all content remains reachable — the reader can scroll horizontally to see the obscured part — and no text is visually truncated unless clip-overflow has been explicitly configured.

**Acceptance Scenarios**:

1. **Given** a table whose total minimum column width exceeds the available width, **When** the reader views the table, **Then** no column is shrunk below its minimum content width (`TBL-013`), the system surfaces the overflow through horizontal scrolling (`TBL-022`), and no text is visually truncated unless clipping overflow is explicitly configured (`TBL-043`).
2. **Given** a cell containing a single continuous word longer than the allocated column width, **When** the table is rendered, **Then** the system falls back to horizontal scrolling so the word remains reachable (`TBL-022`), rather than clipping the word.
3. **Given** clipping overflow has been explicitly configured for the table, **When** a cell's content exceeds the allocated width, **Then** the content is clipped as configured and the reader is made aware that clipping is active.

---

### Edge Cases

- A cell whose content is empty (zero-length plain text). The cell still occupies its row and column with consistent padding and alignment; an empty cell does not collapse the row to zero width or height.
- A single-row table (header-only or one data row) and a single-column table. Layout rules apply uniformly; a single column is allocated the full available width (subject to its own preferred/minimum widths).
- A table whose cells contain content of widely varying lengths. The shrink-to-fit rule distributes the available width among columns without reducing any column below its minimum width when the table's total minimum width still fits.
- A table whose total minimum width exceeds the available width (User Story 4 fallback). No column is reduced below its minimum; horizontal scrolling exposes the overflow; no content is masked unless clipping is explicit.
- Malformed input: inconsistent row lengths (some rows have more or fewer cells than the header), negative padding values, or other malformed inputs. The System does not exhibit undefined behaviour or memory corruption and either returns a descriptive error or normalises the input gracefully (`TBL-50`).
- A table whose content does not change and whose available width does not change between renders. The System does not perform redundant re-layout passes (`TBL-044`).
- Configuring per-cell, per-column, and global padding simultaneously. The most-specific level (per-cell overrides per-column overrides global) takes precedence and the chosen padding is consistently factored into width and height calculations.

## Requirements *(mandatory)*

The functional requirements below are grouped to mirror the structure of `src/desktop/src/ui/SPEC.md`. Each requirement is testable and cross-references the source `TBL-xxx` requirement id(s) it derives from.

### Functional Requirements

#### Data Model & Input

- **FR-001** (`TBL-001`): The System MUST accept input comprising tabular data organised into explicit rows and columns.
- **FR-002** (`TBL-002`): The System MUST support cell content consisting of arbitrary plain text.
- **FR-003** (`TBL-003`): The System MUST support cell content consisting of formatted text as defined by the Markdown specification.

#### Layout and Geometry Calculation

- **FR-010** (`TBL-010`): The System MUST accept a maximum available target width (`W_max`) supplied by the parent context.
- **FR-011** (`TBL-011`): The System MUST calculate, for every column and prior to final layout rendering, the intrinsic minimum content width (`W_min`) and the maximum preferred width (`W_pref`).
- **FR-012** (`TBL-012`): When the total of the columns' preferred widths (`W_pref, total`) is less than or equal to `W_max`, the System MUST allocate each column its preferred width `W_pref`.
- **FR-013** (`TBL-013`): When `W_pref, total` exceeds `W_max`, the System MUST shrink column widths down to fit `W_max` without reducing any column below its `W_min`, except when `W_max` itself is smaller than the total of the minimum widths (`W_min, total`); in that case `TBL-022` overflow handling applies (see FR-022).

#### Overflow and Wrapping

- **FR-020** (`TBL-020`): When a column's allocated width is less than the length of its cell content, the System MUST wrap the cell's text content onto subsequent lines.
- **FR-021** (`TBL-021`): The System SHOULD break wrapped lines preferentially at whitespace characters.
- **FR-022** (`TBL-022`): When a single continuous word exceeds the allocated column width, the System MAY fall back to horizontal scrolling to keep the word reachable.

#### Alignment and Padding

- **FR-030** (`TBL-030`): The System MUST align cell content horizontally LEFT.
- **FR-031** (`TBL-031`): The System MUST align cell content vertically TOP within cells.
- **FR-032** (`TBL-032`): The System SHOULD support inner cell padding (top, bottom, left, right) configurable at the per-cell, per-column, or global table level.
- **FR-033** (`TBL-033`): Padding configured per FR-032 MUST be factored into all column width and row height calculations.

#### Rendering & Decoration

- **FR-040** (`TBL-040`): The System MUST render a medium-gray border around the table perimeter.
- **FR-041** (`TBL-041`): The System MUST render a dark-gray border between adjacent cells.
- **FR-042** (`TBL-042`): When cell borders intersect, the System SHOULD perform border collapsing so junctions render cleanly (e.g., proper box-drawing or grid-line intersection behaviour, no doubled or ragged lines).
- **FR-043** (`TBL-043`): The Renderer MUST output the final table without visually truncating or obscuring text unless the table has been explicitly configured to clip overflow.
- **FR-044** (`TBL-044`): The Renderer SHOULD NOT perform redundant re-layout passes when neither the table data nor the target viewport dimensions have changed since the last layout.

#### Performance & Error Handling

- **FR-050** (`TBL-50`): Malformed input (for example, inconsistent row lengths or negative padding values) MUST NOT result in undefined behaviour or memory corruption; the System SHOULD either return a descriptive error or normalise the input gracefully.
- **FR-051** (`TBL-51`): The System SHOULD use available resources (memory, threads, parallelism) to speed up processing.

### Key Entities *(include if feature involves data)*

- **Table**: The top-level input and output unit. Comprises an ordered list of rows and the metadata needed to render it (available width `W_max`, padding configuration at global level, border styling, overflow mode). Relationships: contains many Rows and many Columns (columnar metadata derived from row cells).
- **Row**: An ordered sequence of Cells whose index corresponds to column position. Relationship: belongs to exactly one Table; contains one or more Cells.
- **Column**: The columnar metadata shared by every Cell at a given index across rows. Key attributes: intrinsic minimum content width `W_min`, maximum preferred width `W_pref`, final allocated width, per-column padding override (if any). Relationship: belongs to a Table; aligned with one Cell per Row.
- **Cell**: A single content unit at the intersection of a Row and a Column. Key attributes: content (plain text or markdown), per-cell padding override (if any), wrapping result. Relationship: belongs to one Row and one Column.
- **Padding configuration**: A value set (top, bottom, left, right) resolvable at three levels — global table, per-column, per-cell — with the more specific level overriding the less specific one. All non-negative.
- **Computed Layout**: The result of the geometry calculation pass: per-column allocated widths satisfying `W_max`, per-row heights derived from wrapped cell content plus padding, and the decoration plan (perimeter + inter-cell borders + junction collapsing). Consumed by the Renderer.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of Markdown tables in the test corpus render fully within the available width when their columns' preferred widths fit, with no column widened or narrowed beyond the fitting rule (User Story 1 / FR-012).
- **SC-002**: 100% of Markdown tables whose preferred widths do not fit but whose minimum widths do fit render within the available width with no column reduced below its minimum content width (User Story 1 / FR-013).
- **SC-003**: 100% of tables whose total minimum width exceeds the available width keep every column at-or-above its minimum content width and surface the overflow through horizontal scrolling, with no text visually masked unless clipping is explicitly configured (User Story 4 / FR-013, FR-022, FR-043).
- **SC-004**: 100% of cells whose content exceeds their allocated column width wrap their text onto subsequent lines, preferring whitespace break points where available (User Story 1 / FR-020, FR-021).
- **SC-005**: 100% of tested cell content containing inline markdown formatting renders with the formatting applied per the Markdown specification (User Story 2 / FR-003).
- **SC-006**: All rendered tables display LEFT horizontal alignment and TOP vertical alignment in every cell, and all configured padding (global, per-column, per-cell) is correctly reflected in width and height calculations (User Story 3 / FR-030, FR-031, FR-033).
- **SC-007**: All rendered tables display the required medium-gray perimeter border, dark-gray inter-cell borders, and cleanly collapsed junctions (User Story 3 / FR-040, FR-041, FR-042).
- **SC-008**: 100% of tested malformed inputs (inconsistent row lengths, negative padding) do not produce undefined behaviour, crashes, or memory corruption, and the System either reports a descriptive error or renders a normalised result (Edge Cases / FR-050).
- **SC-009**: For tables that have not changed in content or available width, the System performs zero redundant re-layout passes between consecutive renders (User Story 1 / Edge Cases / FR-044).
- **SC-010**: Users report they can read rendered Markdown tables including formatted cell content and overflow scenarios without manually inspecting source Markdown (qualitative outcome, verifiable via a user review of a rendered sample set).

## Assumptions

- The subsystem being specified is the Table Layout Engine and Renderer; it consumes already-parsed tabular data and emits rendered geometry and decoration. Parsing Markdown source text into the row/column input is the responsibility of the existing Markdown subsystem (`markdown/` per the crate's `AGENTS.md`); this feature does not re-specify the parser.
- The "available width" (`W_max`) is supplied by the parent rendering context (the UI's view of the viewport); the System does not invent its own viewport.
- A reasonable default for "padding" exists when none is configured: a small, uniform inner padding on all four sides is assumed, consistent with common table-rendering defaults in the project's domain. The exact default value (in display units) is an implementation decision for the planning phase.
- Border colour tokens "medium gray" and "dark gray" reference the crate's existing colour tokens / theme; resolving concrete colour values is an implementation decision for the planning phase.
- "Horizontal scrolling" fallback (`TBL-022`) is the mechanism for keeping overflow content reachable; the exact scroll affordance is provided by the surrounding view/scroll container, not by the layout engine itself.
- Malformed-input handling (`TBL-50`) is satisfied either by a descriptive error or by graceful normalisation; the choice per-error is an implementation decision, but the choice MUST be deterministic and documented.
- Inline markdown formatting in cells (`TBL-003`) is limited to inline constructs that fit inside a cell — block-level constructs (headings, nested tables, fenced code blocks) inside a table cell are out of scope for this feature beyond what the existing Markdown subsystem already supports.
- Performance target is "use available resources sensibly" (`TBL-51`); no explicit latency budget is required because the visible marker of success is "no redundant re-layout" (`SC-009`) and the visible marker of failure is observable lag on a realistically sized table.
- Mobile support is out of scope for this iteration; the subsystem targets the desktop (`fastmd`) crate.