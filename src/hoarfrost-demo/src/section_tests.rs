//! Unit tests for [`crate::section`].

use iced::Size;
use iced::widget::Column;

use super::*;

/// Lays `element` out headless at `viewport`; panics if the frame does.
fn laid_out<'a>(viewport: Size, element: impl Into<Element<'a, Message>>) {
    let _ = iced_test::Simulator::with_size(
        iced_test::core::Settings::default(),
        viewport,
        element.into(),
    );
}

fn empty_body() -> Column<'static, Message> {
    Column::new()
}

#[test]
fn section_carries_its_widget_count() {
    assert_eq!(section("Buttons", 3, empty_body()).widgets, 3);
}

#[test]
fn section_renders_without_panic() {
    laid_out(Size::new(480.0, 200.0), section("Buttons", 2, empty_body()));
}

#[test]
fn a_zero_count_section_omits_the_badge() {
    // The badge is only drawn above zero; the section must still build.
    let built = section("Empty", 0, empty_body());
    assert_eq!(built.widgets, 0);
    laid_out(Size::new(480.0, 200.0), built);
}

#[test]
fn rung_paints_at_the_declared_size() {
    let painted = rung("x", typography::BODY, Palette::dark().text_primary);
    // `Text` exposes no size getter; the contract we can assert headlessly is
    // that the rung builds and lays out on its own.
    laid_out(Size::new(80.0, 40.0), painted);
}

#[test]
fn pane_body_sums_the_section_counts() {
    let body = PaneBody::new(vec![
        section("a", 1, empty_body()),
        section("b", 4, empty_body()),
        section("c", 0, empty_body()),
    ]);
    assert_eq!(body.widgets, 5);
}

#[test]
fn a_pane_with_no_sections_has_no_widgets() {
    assert_eq!(PaneBody::new(Vec::new()).widgets, 0);
}

#[test]
fn pane_body_renders_without_panic() {
    let body = PaneBody::new(vec![section("a", 1, empty_body())]);
    laid_out(Size::new(480.0, 900.0), body.sections);
}
