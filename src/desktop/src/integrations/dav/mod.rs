//! DAV integration — CalDAV (RFC 4791) and CardDAV (RFC 6352) over
//! HTTP, backed by the `fast_dav_rs` SDK.
//!
//! `client` is the unified `DavClient` — one struct that owns
//! both a `CalDavClient` and a `CardDavClient` against the same
//! DAV server, exposing both protocols' tools.
//! `cal` and `card` are the protocol-specific helpers
//! (iCal/vCard parsing, JSON serialisation) and the per-protocol
//! `tool_*` LLM-adapter wrappers that aggregate per-server
//! results.
//!
//! Both submodules share the same crate::config::caldav_clients
//! configuration map (DAV servers are usually both CalDAV and
//! CardDAV at the same URL with the same credentials).
//!
//! This is the protocol layer. The LLM-tool-loop adapters that
//! expose these as `Tool` impls live in
//! crate::agent::tools::manager::builtin::caldav and
//! crate::agent::tools::manager::builtin::carddav.

pub mod cal;
pub mod card;
pub mod client;

pub use client::DavClient;
