//! DAV integration — CalDAV (RFC 4791) and CardDAV (RFC 6352) over
//! HTTP, backed by the `fast_dav_rs` SDK.
//!
//! `cal` covers the calendar (VEVENT) wire format and the
//! `search_calendar` / `get_calendar` / `add_calendar_item` / etc.
//! tool entry points. `card` covers the address book (vCard) wire
//! format and the matching contact entry points.
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
