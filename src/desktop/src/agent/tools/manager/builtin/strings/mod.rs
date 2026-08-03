//! User-visible description strings for every built-in tool — the single source of truth for the LLM-facing tool description and per-field schema description.
//!
//! Each submodule corresponds to one tool family (filesystem, web, calendar,
//! contacts, email, CSV, weather, YAML). The `Tool::description()` impls in
//! the sibling `builtin/*.rs` files and the `#[schemars(description = ...)]`
//! attributes on the DTO fields in `crate::agent::tools::dtos` and
//! `crate::agent::tools::csv_db::schema` both reference the consts defined
//! here. Editing a string in this module is the only place to change what
//! the LLM sees for that tool.

pub(crate) mod browser;
pub(crate) mod caldav;
pub(crate) mod carddav;
pub(crate) mod csv;
pub(crate) mod cursor;
pub(crate) mod fs;
pub(crate) mod jmap;
pub(crate) mod paging;
pub mod trello;
pub(crate) mod weather;
pub(crate) mod web;
pub(crate) mod yaml;
