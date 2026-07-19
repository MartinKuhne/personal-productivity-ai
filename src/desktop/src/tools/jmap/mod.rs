pub mod client;
pub mod calendar;
pub mod email;
pub mod contacts;

pub use calendar::{tool_search_calendar, tool_get_calendar, tool_get_calendar_item, tool_add_calendar_item, tool_update_calendar_item, tool_delete_calendar_item};
pub use email::{tool_search_email, tool_get_email, tool_get_email_by_id, tool_send_email};
pub use contacts::{tool_search_contact, tool_get_contact, tool_add_contact};