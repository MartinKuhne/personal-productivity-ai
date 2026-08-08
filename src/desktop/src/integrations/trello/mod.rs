//! Trello integration — REST client over `https://api.trello.com/1`.
//!
//! This is the protocol layer. The LLM-tool-loop adapters that expose
//! it as a family of `Tool` impls (`trello_get_boards`,
//! `trello_create_card`, …) live in
//! crate::agent::tools::manager::builtin::trello.

mod client;

pub use client::trello_request;
