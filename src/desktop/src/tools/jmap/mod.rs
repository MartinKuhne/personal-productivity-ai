pub mod calendar;
pub mod client;
pub mod contacts;
pub mod email;

#[cfg(test)]
mod tests;

pub use calendar::{
    tool_add_calendar_item, tool_delete_calendar_item, tool_get_calendar, tool_get_calendar_item,
    tool_search_calendar, tool_update_calendar_item,
};
pub use contacts::{tool_add_contact, tool_get_contact, tool_search_contact};
pub use email::{tool_get_email, tool_get_email_by_id, tool_search_email, tool_send_email};
