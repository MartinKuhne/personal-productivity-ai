//! JMAP subsystem — email tool functions over the JMAP protocol (RFC 8620/8621).
//!
//! Email operations use typed methods from the `jmap_client` crate:
//! - `email_query()` — filter by text, mailbox, dates, keywords
//! - `email_get()` — fetch full email details by ID
//! - `email_import()` — import raw email as draft

pub mod client;
pub mod email;

#[cfg(test)]
mod mock_server;

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use mock_server::{spawn_mock_server, spawn_recording_mock_server};

pub use email::{
    SearchEmailFilters, new_search_email_cursor, tool_get_email_by_id, tool_search_email,
    tool_send_email,
};
