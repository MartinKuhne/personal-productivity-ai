//! Markdown subsystem — parsing, rendering AST models, table layout algorithms, and document state.
//!
//! Per `src/desktop/AGENTS.md`, this subsystem is the single import point for `pulldown-cmark`,
//! `serde_yml` front-matter/table parsing, and Markdown AST types. `ui/`, `tools/`, `print.rs`,
//! and `editor.rs` call into `markdown::` rather than handling Markdown directly.

pub mod ast;
pub mod document;
pub mod parser;
pub mod toc;

pub use ast::{InlineElem, RenderEvent, TextStyle, heading_plain_text};
pub use document::{DocumentModel, FrontMatter, apply_task_toggle, parse_front_matter};
pub use parser::{parse_markdown_to_events, parse_yaml_to_pairs, render_markdown_to_html};
pub use toc::build_toc;
