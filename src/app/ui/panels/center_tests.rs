//! Tests for `panels/center.rs`.

use super::*;
use crate::bus::events::user_command::UserCommand;

#[test]
fn test_apply_tab_close_click() {
    assert_eq!(apply_tab_close_click(1), UserCommand::CloseTab(1));
}

#[test]
fn test_apply_tab_close_others_click() {
    assert_eq!(
        apply_tab_close_others_click(2),
        UserCommand::CloseOtherTabs(2)
    );
}

#[test]
fn test_apply_tab_close_all_click() {
    assert_eq!(apply_tab_close_all_click(), UserCommand::CloseAllTabs);
}
