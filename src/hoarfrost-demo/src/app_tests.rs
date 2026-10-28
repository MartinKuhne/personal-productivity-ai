//! Unit tests for [`crate::app`].

use iced::Size;

use crate::menu_surface;

use super::*;

/// A realistic desktop viewport for the headless render smoke test.
const VIEWPORT: Size = Size::new(1400.0, 900.0);

/// Lays `element` out headless at `viewport`; panics if the frame does.
fn laid_out<'a>(viewport: Size, element: impl Into<Element<'a, Message>>) {
    let _ = iced_test::Simulator::with_size(
        iced_test::core::Settings::default(),
        viewport,
        element.into(),
    );
}

/// Applies one message for its state effect only, discarding the task.
fn send(app: &mut DemoApp, message: Message) {
    let _ = app.update(message);
}

/// Renders the app and captures the laid-out widget tree.
fn render(app: &DemoApp) -> crate::layout_probe::Probe {
    crate::layout_probe::Probe::capture(VIEWPORT, app.view())
}

/// Whether any menu card is present in the **rendered** tree.
fn card_is_rendered(app: &DemoApp) -> bool {
    !render(app)
        .with_prefix(crate::menu_surface::CARD_ID_PREFIX)
        .is_empty()
}

#[test]
fn boot_seeds_the_ready_state_and_the_default_split() {
    let (app, task) = DemoApp::boot();
    assert_eq!(task.units(), 0, "boot must not schedule any work");
    assert_eq!(app.last_event, data::STATUS_READY);
    assert_eq!(app.split_ratio, data::SPLIT_RATIO_DEFAULT);
    assert!(app.navbar.active_menu_id.is_none());
    assert!(
        !card_is_rendered(&app),
        "boot must render no menu card at all"
    );
}

#[test]
fn close_maps_to_a_live_exit_task() {
    let (mut app, _) = DemoApp::boot();
    let task = app.update(Message::Close);
    assert!(
        task.units() > 0,
        "Close must schedule the iced exit, not return a none-task"
    );
}

#[test]
fn every_other_message_schedules_no_work() {
    let (mut app, _) = DemoApp::boot();
    for message in [
        Message::NavbarToggled("file".to_string()),
        Message::SplitLive(0.4),
        Message::SplitCommitted(0.6),
        Message::SplitCancelled,
    ] {
        assert_eq!(
            app.update(message.clone()).units(),
            0,
            "{message:?} must be a pure state mutation"
        );
    }
}

#[test]
fn navbar_toggle_latches_and_unlatches() {
    let (mut app, _) = DemoApp::boot();

    send(&mut app, Message::NavbarToggled("file".to_string()));
    assert_eq!(app.navbar.active_menu_id.as_deref(), Some("file"));
    assert!(app.last_event.contains("file"));
    // State alone is not render: assert the card is actually drawn.
    assert!(
        card_is_rendered(&app),
        "the latch is set but no card was rendered"
    );

    send(&mut app, Message::NavbarToggled("file".to_string()));
    assert_eq!(app.navbar.active_menu_id, None);
    assert_eq!(app.last_event, strings::NAVBAR_CLOSED);
    assert!(
        !card_is_rendered(&app),
        "the latch cleared but the card is still on screen"
    );
}

#[test]
fn switching_from_one_trigger_to_another_moves_the_latch() {
    let (mut app, _) = DemoApp::boot();
    send(&mut app, Message::NavbarToggled("file".to_string()));
    send(&mut app, Message::NavbarToggled("view".to_string()));
    assert_eq!(app.navbar.active_menu_id.as_deref(), Some("view"));

    // …and the render follows the latch to the new trigger, leaving only
    // one card on screen.
    let probe = render(&app);
    assert!(
        probe.find(&menu_surface::card_id("view")).is_some(),
        "the newly latched trigger rendered no card"
    );
    assert!(
        probe.find(&menu_surface::card_id("file")).is_none(),
        "the previous trigger's card is still on screen"
    );
    assert_eq!(
        1,
        probe.with_prefix(menu_surface::CARD_ID_PREFIX).len(),
        "exactly one card may be mounted at a time"
    );
}

#[test]
fn split_messages_move_the_ratio_and_report() {
    let (mut app, _) = DemoApp::boot();

    send(&mut app, Message::SplitLive(0.4));
    assert_eq!(app.split_ratio, 0.4);
    assert_eq!(app.last_event, strings::SPLIT_DRAGGING);

    send(&mut app, Message::SplitCommitted(0.6));
    assert_eq!(app.split_ratio, 0.6);
    assert!(app.last_event.starts_with(strings::SPLIT_COMMITTED));

    send(&mut app, Message::SplitCancelled);
    assert_eq!(app.split_ratio, data::SPLIT_RATIO_DEFAULT);
    assert_eq!(app.last_event, strings::SPLIT_CANCELLED);
}

#[test]
fn widget_count_includes_the_chrome() {
    let (app, _) = DemoApp::boot();
    assert!(
        app.widget_count() >= chrome::WIDGET_COUNT,
        "the total must always cover the chrome"
    );
}

/// The whole root view must lay out and paint one frame without panicking.
///
/// This is the demo's real validation: every widget in both panes is
/// constructed, measured and drawn by a headless renderer.
#[test]
fn root_view_renders_one_frame_without_panic() {
    let (app, _) = DemoApp::boot();
    laid_out(VIEWPORT, app.view());
}

/// A second frame over the same state renders too — nothing about the first
/// frame left the widget tree in a state the second cannot handle.
#[test]
fn root_view_renders_repeatedly() {
    let (app, _) = DemoApp::boot();
    for _ in 0..3 {
        laid_out(VIEWPORT, app.view());
    }
}

/// The view survives a split drag, which changes the geometry it lays out.
#[test]
fn root_view_renders_after_a_split_drag() {
    let (mut app, _) = DemoApp::boot();
    send(&mut app, Message::SplitCommitted(data::SPLIT_RATIO_MAX));
    laid_out(VIEWPORT, app.view());
    send(&mut app, Message::SplitCommitted(data::SPLIT_RATIO_MIN));
    laid_out(VIEWPORT, app.view());
}

/// A narrow viewport must not break the layout either.
#[test]
fn root_view_renders_at_a_narrow_viewport() {
    let (app, _) = DemoApp::boot();
    laid_out(Size::new(640.0, 480.0), app.view());
}

// ---------------------------------------------------------------------------
// Composites routing (S5)
// ---------------------------------------------------------------------------

use crate::pane_composites::CompositesMessage;

/// A composites message reaches the composites state and the status bar.
#[test]
fn composites_messages_route_to_the_composites_state() {
    let (mut app, _) = DemoApp::boot();
    send(
        &mut app,
        Message::Composites(CompositesMessage::RailSelected("rail-tasks".into())),
    );
    assert_eq!("rail-tasks", app.composites().rail.selected);
    assert!(app.last_event.contains("Tasks"), "{}", app.last_event);
}

/// The tab close handshake survives the round trip through the app.
#[test]
fn the_tab_close_handshake_survives_the_app_round_trip() {
    let (mut app, _) = DemoApp::boot();
    let before = app.composites().tabs.tabs.len();

    send(
        &mut app,
        Message::Composites(CompositesMessage::TabCloseRequested("tab-notes".into())),
    );
    assert_eq!(
        Some("tab-notes".to_string()),
        app.composites().pending_close,
        "a dirty tab must park in pending_close"
    );
    assert_eq!(before, app.composites().tabs.tabs.len());

    send(
        &mut app,
        Message::Composites(CompositesMessage::TabCloseRequested("tab-notes".into())),
    );
    assert_eq!(None, app.composites().pending_close);
    assert_eq!(before - 1, app.composites().tabs.tabs.len());
}

/// No composites message schedules real work — only `Close` does.
#[test]
fn composites_messages_schedule_no_work() {
    let (mut app, _) = DemoApp::boot();
    for message in [
        CompositesMessage::TabSelected("tab-todo".into()),
        CompositesMessage::TabCloseRequested("tab-report".into()),
        CompositesMessage::RailExpanded(false),
        CompositesMessage::StepNext,
        CompositesMessage::StepPrevious,
        CompositesMessage::PageChanged(4),
        CompositesMessage::PageSizeChanged(50),
        CompositesMessage::PipsPageChanged(6),
        CompositesMessage::Note("x".into()),
    ] {
        let task = app.update(Message::Composites(message));
        assert_eq!(0, task.units(), "only Close may schedule work");
    }
}

/// The view renders after every kind of composites state change.
#[test]
fn root_view_renders_after_composites_changes() {
    let (mut app, _) = DemoApp::boot();
    for message in [
        CompositesMessage::TabSelected("tab-notes".into()),
        CompositesMessage::RailExpanded(false),
        CompositesMessage::StepNext,
        CompositesMessage::PageSizeChanged(100),
        CompositesMessage::PipsPageChanged(7),
    ] {
        send(&mut app, Message::Composites(message));
        laid_out(VIEWPORT, app.view());
    }
}

/// Collapsing the rail and shrinking the window still lays out.
#[test]
fn root_view_renders_a_collapsed_rail_at_a_narrow_viewport() {
    let (mut app, _) = DemoApp::boot();
    send(
        &mut app,
        Message::Composites(CompositesMessage::RailExpanded(false)),
    );
    laid_out(Size::new(640.0, 480.0), app.view());
}

// ---------------------------------------------------------------------------
// fill_portions
// ---------------------------------------------------------------------------

/// The halves are equal at the default ratio.
#[test]
fn the_default_ratio_splits_the_panes_evenly() {
    let (left, right) = fill_portions(data::SPLIT_RATIO_DEFAULT);
    assert_eq!(500, left);
    assert_eq!(500, right);
}

/// The portions always sum to the whole, whatever the ratio.
#[test]
fn the_portions_always_sum_to_the_total() {
    for ratio in [0.0, 0.1, 0.25, 0.5, 0.62, 0.75, 1.0, -3.0, 42.0] {
        let (left, right) = fill_portions(ratio);
        assert_eq!(
            PORTION_TOTAL,
            left + right,
            "ratio {ratio} must split the whole between the panes"
        );
    }
}

/// A ratio outside the allowed band is clamped, not trusted.
#[test]
fn a_ratio_outside_the_allowed_band_is_clamped_to_the_extremes() {
    let (floor_left, floor_right) = fill_portions(0.0);
    let (at_min_left, at_min_right) = fill_portions(data::SPLIT_RATIO_MIN);
    assert_eq!((floor_left, floor_right), (at_min_left, at_min_right));

    let (ceil_left, ceil_right) = fill_portions(1.0);
    let (at_max_left, at_max_right) = fill_portions(data::SPLIT_RATIO_MAX);
    assert_eq!((ceil_left, ceil_right), (at_max_left, at_max_right));
}

/// A NaN ratio cannot leak a NaN portion, which iced would carry into layout.
#[test]
fn a_nan_ratio_falls_back_to_the_floor() {
    let (left, right) = fill_portions(f32::NAN);
    assert!(left >= 1 && right >= 1, "({left}, {right})");
    assert_eq!(PORTION_TOTAL, left + right);
}

/// Neither pane may be starved of every portion.
#[test]
fn neither_pane_is_ever_given_zero_portions() {
    for ratio in [0.0, 0.001, 0.25, 0.5, 0.75, 0.999, 1.0] {
        let (left, right) = fill_portions(ratio);
        assert!(left >= 1, "ratio {ratio} starved the left pane");
        assert!(right >= 1, "ratio {ratio} starved the right pane");
    }
}

/// A wider ratio really does mean a wider left pane, so the handle is not
/// decorative once its reported ratio is carried back into the widths.
#[test]
fn a_larger_ratio_widens_the_left_pane_monotonically() {
    let ratios = [0.25, 0.4, 0.5, 0.6, 0.75];
    let mut previous = 0u16;
    for ratio in ratios {
        let (left, _) = fill_portions(ratio);
        assert!(left > previous, "ratio {ratio} did not widen the left pane");
        previous = left;
    }
}

/// Committing a new split ratio moves the composites pane, which is the
/// observable proof that the divider drives the layout.
#[test]
fn committing_a_split_ratio_moves_the_composites_pane() {
    let (mut app, _) = DemoApp::boot();
    laid_out(VIEWPORT, app.view());

    send(&mut app, Message::SplitCommitted(0.7));
    laid_out(VIEWPORT, app.view());

    let before = fill_portions(data::SPLIT_RATIO_DEFAULT).0;
    let after = fill_portions(app.split_ratio).0;
    assert!(
        after > before,
        "the committed ratio should have widened the left pane: {before} -> {after}"
    );
}

/// The count the status bar *shows* is the count the builders reported.
///
/// `widget_count()` being internally consistent is not enough — the number
/// has to reach the screen. This closes the loop between the sum of what
/// the section builders laid down and the text in the status bar.
#[test]
fn the_status_bar_renders_the_widget_count() {
    let (app, _) = DemoApp::boot();
    let rendered = crate::layout_probe::text_of(app.view());
    let shown = format!("{} widgets rendered", app.widget_count());

    assert!(
        rendered.contains(&shown),
        "the status bar must show the live count ({shown}); rendered text was: {rendered}"
    );
}

/// Toggling an existing control adds no widgets to the frame.
#[test]
fn the_widget_count_survives_a_state_change() {
    let (mut app, _) = DemoApp::boot();
    let before = app.widget_count();

    send(
        &mut app,
        Message::Atoms(crate::pane_atoms::AtomsMessage::SwitchToggled(true)),
    );

    assert_eq!(
        before,
        app.widget_count(),
        "toggling existing controls must not change how many widgets exist"
    );
}

/// Latching a navbar trigger is *not* a no-op for the frame: it mounts a
/// card, and the count has to say so.
///
/// The old version of this test asserted the opposite, which was only true
/// because nothing was ever mounted — the very gap the render tests exist
/// to close.
#[test]
fn latching_a_navbar_trigger_adds_the_mounted_card_to_the_count() {
    let (mut app, _) = DemoApp::boot();
    let before = app.widget_count();

    send(&mut app, Message::NavbarToggled(String::from("file")));

    assert_eq!(
        before + menu_surface::mounted_row_count(&data::menu_model()),
        app.widget_count(),
        "opening the menu must add exactly the card's rows"
    );

    send(&mut app, Message::NavbarToggled(String::from("file")));
    assert_eq!(
        before,
        app.widget_count(),
        "closing the card must take its rows back out of the count"
    );
}
