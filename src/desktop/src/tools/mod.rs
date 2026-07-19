pub mod caldav;
pub mod filesystem;
pub mod web;
pub mod yaml_header;
pub mod jmap;
pub mod registry;
pub mod dtos;
pub mod weather;

pub use filesystem::{tool_grep, tool_list_files, tool_list_files_by_tag, tool_read_file, tool_read_file_lines, tool_create_file, tool_insert_lines, tool_delete_lines, tool_read_tags};
pub use web::{tool_web_fetch, tool_web_search};
pub use yaml_header::{tool_read_yaml_header, tool_write_yaml_header};
pub use jmap::{tool_search_calendar, tool_get_calendar, tool_get_calendar_item, tool_add_calendar_item, tool_update_calendar_item, tool_delete_calendar_item, tool_search_email, tool_get_email, tool_send_email, tool_search_contact, tool_get_contact, tool_add_contact};
pub use registry::{execute_tool, get_tools_schema};