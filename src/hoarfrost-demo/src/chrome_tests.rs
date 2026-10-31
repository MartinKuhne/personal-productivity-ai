//! Unit tests for [`crate::chrome`].

use iced::Size;

use super::*;

/// Lays `element` out headless at `viewport`; panics if the frame does.
fn laid_out<'a>(viewport: Size, element: impl Into<Element<'a, Message>>) {
    let _ = iced_test::Simulator::with_size(
        iced_test::core::Settings::default(),
        viewport,
        element.into(),
    );
}

#[test]
fn widget_count_is_the_triggers_plus_close() {
    assert_eq!(WIDGET_COUNT, data::NAVBAR_TRIGGERS.len() + 1);
}

#[test]
fn navbar_model_carries_every_declared_trigger() {
    let model = data::navbar_model();
    assert_eq!(model.triggers.len(), data::NAVBAR_TRIGGERS.len());
    for (id, label) in data::NAVBAR_TRIGGERS {
        let trigger = model
            .triggers
            .iter()
            .find(|trigger| trigger.id == id)
            .unwrap_or_else(|| panic!("trigger {id:?} must exist"));
        assert_eq!(trigger.label, label);
        assert_eq!(trigger.tooltip.as_deref(), Some(label));
        assert!(!trigger.disabled);
    }
}

#[test]
fn navbar_model_carries_the_brand() {
    assert_eq!(data::navbar_model().brand.as_deref(), Some(data::BRAND));
}

#[test]
fn navbar_row_renders_without_panic() {
    laid_out(Size::new(1400.0, 900.0), navbar_row(&data::navbar_model()));
}

#[test]
fn close_button_renders_without_panic() {
    laid_out(Size::new(400.0, 120.0), close_button());
}

#[test]
fn status_bar_renders_without_panic() {
    laid_out(Size::new(1400.0, 24.0), status_bar("ready", 42));
}

#[test]
fn status_bar_reports_the_count_in_its_text() {
    // Read the string the widget actually draws, not one this test wrote.
    let rendered = crate::layout_probe::text_of(status_bar("ready", 7));

    assert!(
        rendered.contains("7 widgets rendered"),
        "the status bar must render the count it was handed; rendered text was: {rendered}"
    );
    assert!(
        rendered.contains("ready"),
        "the status bar must still carry the event text alongside the count: {rendered}"
    );
    assert!(
        !rendered.contains("8 widgets rendered"),
        "the count must be the one passed in, not some other number: {rendered}"
    );
}

#[test]
fn chrome_ids_are_stable() {
    assert_eq!(CLOSE_FOCUS_ID, "demo.chrome.close");
    assert_eq!(STATUS_BAR_ID, "demo.chrome.status_bar");
    assert_ne!(CLOSE_FOCUS_ID, STATUS_BAR_ID);
}
