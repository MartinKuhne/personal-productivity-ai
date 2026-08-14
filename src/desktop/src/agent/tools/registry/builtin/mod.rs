//! Built-in tool implementations and the default provider list.
//!
//! Each submodule under `builtin` defines a single family of
//! tools plus a `ToolProvider` struct that lists them. The
//! `default_providers` function returns the canonical provider
//! list, in registration order; the [`ToolRegistry`] constructor
//! iterates it and registers every tool each provider returns.
//!
//! MCP tools are registered separately via
//! [`ToolRegistry::register_mcp_tool`](crate::tools::registry::ToolRegistry::register_mcp_tool), which calls into the MCP
//! client manager rather than a provider.

#[cfg(feature = "browser")]
pub(crate) mod browser;
pub(crate) mod caldav;
pub(crate) mod carddav;
pub(crate) mod csv;
pub(crate) mod fs;
pub(crate) mod jmap;
pub(crate) mod strings;
pub(crate) mod trello;
pub(crate) mod weather;
#[cfg(feature = "vector-search")]
pub(crate) use crate::tools::vector_search;
pub(crate) mod web;
pub(crate) mod yaml;

use super::ToolRegistry;
use crate::tools::provider::ToolProvider;
use std::sync::Arc;

/// Default list of built-in tool providers, in the order the
/// registry should register them. Each provider contributes a
/// `Vec<RegisteredTool>` of tools from one family.
pub(crate) fn default_providers() -> Vec<Arc<dyn ToolProvider>> {
    vec![
        Arc::new(fs::FilesystemProvider),
        Arc::new(yaml::YamlProvider),
        #[cfg(feature = "vector-search")]
        Arc::new(vector_search::VectorSearchProvider),
        Arc::new(web::WebProvider),
        Arc::new(jmap::JmapProvider),
        Arc::new(caldav::CalDavProvider),
        Arc::new(carddav::CardDavProvider),
        Arc::new(csv::CsvProvider),
        Arc::new(weather::WeatherProvider),
        Arc::new(trello::TrelloProvider),
        #[cfg(feature = "browser")]
        Arc::new(browser::BrowserProvider),
    ]
}

/// Register every built-in tool into the given manager by
/// iterating the default provider list.
pub(crate) fn register_all_builtins(mgr: &mut ToolRegistry) {
    for provider in default_providers() {
        for tool in provider.tools() {
            mgr.register_registered_tool(tool);
        }
    }
}
