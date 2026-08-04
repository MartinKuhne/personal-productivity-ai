//! Markdown subsystem — parsing, rendering AST models, table layout algorithms, and document state.
//!
//! Requirements: see [`SPEC.md`](SPEC.md) (MD-001..MD-018) for the full specification.
//!
//! Per `src/desktop/AGENTS.md`, this subsystem is the single import point for `pulldown-cmark`,
//! `serde_norway` front-matter/table parsing, and Markdown AST types. `ui/`, `tools/`, `print.rs`,
//! and `editor.rs` call into `markdown::` rather than handling Markdown directly.
//!
//! Submodules:
//! - `model` — the Markdown data model (inline AST, table-of-contents
//!   entry, ToC builder).
//! - `document` — `Document` parsed-document newtype, front-matter read/write, task-toggle.
//! - `parser` — `pulldown-cmark`-backed parsing and HTML rendering.
//! - `table_width` — pure Fair Table Width Algorithm (no egui, no Markdown types).

pub mod document;
pub mod model;
pub mod parser;
pub mod table_width;

pub use document::{Document, FrontMatter, apply_task_toggle, parse_front_matter};
pub use model::{InlineElem, RenderEvent, TextStyle, ToCEntry, build_toc, heading_plain_text};
pub use parser::{parse_markdown_to_events, parse_yaml_to_pairs, render_markdown_to_html};
pub use table_width::{
    Breakpoint, CellTokens, ColumnWidths, DeficitStrategy, compute_column_breakpoints, ftwa,
};
