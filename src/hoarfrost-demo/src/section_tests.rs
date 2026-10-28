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

/// Lays one labelled rung out on its own and returns its recorded entry.
///
/// The wrapper carries no padding, no background and no width policy, so
/// what the probe records is the rung's own line box.
fn rung_box(rung_rung: typography::TypeRung, id: &str) -> crate::layout_probe::Entry {
    let probe = crate::layout_probe::Probe::capture(
        Size::new(400.0, 400.0),
        labeled("Hg", rung_rung, Palette::dark().text_primary, id),
    );
    probe
        .find(id)
        .unwrap_or_else(|| panic!("`{id}` must be addressable"))
        .clone()
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
    // `Text` exposes no size getter, so the rung's declared geometry is
    // read back off the layout tree instead: `labeled` wraps the rung in
    // a padding-free container whose recorded box is the text's own, so
    // the height the probe reports *is* the line box `rung` asked iced
    // for.
    let body = rung_box(typography::BODY, "test.rung.body");

    assert!(
        (body.bounds.height - typography::BODY.line_height).abs() <= 0.5,
        "a BODY rung must occupy its declared {}px line box, got {}",
        typography::BODY.line_height,
        body.bounds.height
    );

    // A larger rung has to actually render larger. This is the guard
    // against the `LineHeight::Relative` mistake: handing iced a bare
    // pixel `f32` means "that many times the font size", so every rung
    // lands in a line many times its declared height.
    let title = rung_box(typography::TITLE_SM, "test.rung.title");

    assert!(
        (title.bounds.height - typography::TITLE_SM.line_height).abs() <= 0.5,
        "a TITLE_SM rung must occupy its declared {}px line box, got {}",
        typography::TITLE_SM.line_height,
        title.bounds.height
    );
    assert!(
        title.bounds.height > body.bounds.height,
        "the larger rung must render taller: {} vs {}",
        title.bounds.height,
        body.bounds.height
    );
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
