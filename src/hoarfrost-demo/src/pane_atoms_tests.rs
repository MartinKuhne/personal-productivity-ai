//! Unit tests for [`crate::pane_atoms`].

use iced::Element;
use iced::Size;

use crate::app::{DemoApp, Message};
use crate::chrome;
use crate::layout_probe::{Entry, Probe};
use crate::pane_atoms::AtomsMessage;
use hoarfrost::token::space;
use hoarfrost::widget::text_input;
use hoarfrost::widget::toggle_model::CheckState;

use super::*;

/// Lays `element` out headless at `viewport`; panics if the frame does.
fn laid_out<'a>(viewport: Size, element: impl Into<Element<'a, Message>>) {
    let _ = iced_test::Simulator::with_size(
        iced_test::core::Settings::default(),
        viewport,
        element.into(),
    );
}

/// The atoms pane's own viewport (the left half of the demo window).
const PANE: Size = Size::new(480.0, 900.0);

/// The widget count each live section is expected to report, in display order.
const EXPECTED_COUNTS: [usize; 18] = [12, 4, 1, 3, 1, 1, 3, 5, 2, 3, 1, 3, 2, 4, 2, 2, 2, 2];

#[test]
fn pane_scroll_id_is_stable() {
    assert_eq!(PANE_SCROLL_ID, "demo.pane.atoms.scroll");
}

#[test]
fn live_sections_cover_every_declared_builder() {
    let state = AtomsState::new();
    assert_eq!(live_sections(&state).len(), live_builders().len());
}

#[test]
fn every_live_section_reports_its_widget_count() {
    let state = AtomsState::new();
    let counts: Vec<usize> = live_sections(&state).iter().map(|s| s.widgets).collect();
    assert_eq!(counts, EXPECTED_COUNTS);
}

#[test]
fn no_live_section_reports_zero_widgets() {
    let state = AtomsState::new();
    for (name, build) in live_builders() {
        assert!(
            build(&state).widgets > 0,
            "section {name} must report at least one widget"
        );
    }
}

#[test]
fn buttons_section_renders() {
    let state = AtomsState::new();
    laid_out(PANE, buttons_section(&state));
}

#[test]
fn checkbox_section_renders() {
    let state = AtomsState::new();
    laid_out(PANE, checkbox_section(&state));
}

#[test]
fn radio_section_renders() {
    let state = AtomsState::new();
    laid_out(PANE, radio_section(&state));
}

#[test]
fn switch_section_renders() {
    let state = AtomsState::new();
    laid_out(PANE, switch_section(&state));
}

#[test]
fn segmented_section_renders() {
    let state = AtomsState::new();
    laid_out(PANE, segmented_section(&state));
}

#[test]
fn split_button_section_renders() {
    let state = AtomsState::new();
    laid_out(PANE, split_button_section(&state));
}

#[test]
fn hyperlink_section_renders() {
    laid_out(PANE, hyperlink_section());
}

#[test]
fn text_input_section_renders() {
    let state = AtomsState::new();
    laid_out(PANE, text_input_section(&state));
}

#[test]
fn password_section_renders() {
    let state = AtomsState::new();
    laid_out(PANE, password_section(&state));
}

#[test]
fn combo_box_section_renders() {
    let state = AtomsState::new();
    laid_out(PANE, combo_box_section(&state));
}

#[test]
fn search_box_section_renders() {
    let state = AtomsState::new();
    laid_out(PANE, search_box_section(&state));
}

#[test]
fn whole_atoms_pane_renders() {
    let state = AtomsState::new();
    laid_out(PANE, atoms_pane(&state).sections);
}

/// A width far below the demo window's, chosen to squeeze every
/// fixed-width box a section lays down below its natural size.
const NARROW: f32 = 280.0;

/// A named live-section builder.
type SectionBuilder = for<'a> fn(&'a AtomsState) -> Section<'a>;

/// Every live section as a named builder, so one test can drive them all.
fn live_builders() -> Vec<(&'static str, SectionBuilder)> {
    vec![
        ("buttons", buttons_section),
        ("checkbox", checkbox_section),
        ("radio", radio_section),
        ("switch", switch_section),
        ("segmented", segmented_section),
        ("split", split_button_section),
        ("hyperlink", |_| hyperlink_section()),
        ("text", text_input_section),
        ("password", password_section),
        ("combo", combo_box_section),
        ("search", search_box_section),
        ("rating", rating_section),
        ("range", range_slider_section),
        ("avatar", |_| avatar_section()),
        ("date", date_picker_section),
        ("time", time_picker_section),
        ("colour", color_picker_section),
        ("tooltip", |_| tooltip_section()),
    ]
}

/// Every live section must still lay out when the pane is squeezed.
///
/// `Length::Fixed` is clamped to whatever the parent actually offers, so a
/// squeezed button label can drop under `Button::is_icon_only`'s
/// threshold Ã¢â‚¬â€ the point where the framework's `debug_assert!` fires
/// unless the button carries a tooltip. This is the guard that keeps a
/// section from shipping without one.
#[test]
fn every_live_section_lays_out_at_a_narrow_viewport() {
    let state = AtomsState::new();
    for (name, build) in live_builders() {
        let section = build(&state);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            laid_out(Size::new(NARROW, 900.0), section);
        }));
        assert!(
            outcome.is_ok(),
            "the {name} section must lay out at {NARROW}px"
        );
    }
}

/// The whole pane, not just one section at a time, survives the squeeze.
#[test]
fn whole_atoms_pane_lays_out_at_a_narrow_viewport() {
    let state = AtomsState::new();
    let sections = atoms_pane(&state).sections;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        laid_out(Size::new(NARROW, 900.0), sections);
    }));
    assert!(outcome.is_ok(), "the atoms pane must lay out at {NARROW}px");
}

#[test]
fn checkbox_message_updates_the_stored_state() {
    let mut state = AtomsState::new();
    let before = state.checkboxes.clone();
    assert_eq!(
        data::CHECKBOX_ROWS.len(),
        before.len(),
        "one stored slot per fixture row"
    );
    assert_eq!(
        CheckState::Unchecked,
        before[0],
        "the fixture's first row must start unchecked for this to be a change"
    );

    state.apply(&AtomsMessage::Checkbox(0, CheckState::Checked));
    assert_eq!(
        CheckState::Checked,
        state.checkboxes[0],
        "the message must land in the slot it names"
    );
    assert_ne!(
        before, state.checkboxes,
        "applying a check must actually move the stored state"
    );
    laid_out(PANE, checkbox_section(&state));

    // An out-of-range index must be ignored rather than panic.
    state.apply(&AtomsMessage::Checkbox(99, CheckState::Unchecked));
    assert_eq!(
        before.len(),
        state.checkboxes.len(),
        "an out-of-range index must not grow or shrink the stored state"
    );
    assert_eq!(
        CheckState::Checked,
        state.checkboxes[0],
        "an out-of-range index must leave the slots that do exist untouched"
    );
}

#[test]
fn radio_message_replaces_the_selection() {
    let mut state = AtomsState::new();
    state.apply(&AtomsMessage::RadioSelected("spacious".to_string()));
    assert_eq!(state.radio.selected_index(), Some(2));
    laid_out(PANE, radio_section(&state));
}

#[test]
fn switch_message_flips_the_position() {
    let mut state = AtomsState::new();
    state.apply(&AtomsMessage::SwitchToggled(false));
    assert!(!state.switch_on);
    laid_out(PANE, switch_section(&state));
}

#[test]
fn segmented_message_replaces_the_selection_set() {
    let mut state = AtomsState::new();
    let mut set = std::collections::BTreeSet::new();
    set.insert("month".to_string());
    state.apply(&AtomsMessage::SegmentedChanged(set));
    assert!(state.segmented.is_selected(&"month".to_string()));
    laid_out(PANE, segmented_section(&state));
}

#[test]
fn split_menu_toggle_is_recorded() {
    let mut state = AtomsState::new();
    state.apply(&AtomsMessage::SplitMenuToggled(true));
    assert!(state.split.menu_open);
    state.apply(&AtomsMessage::SplitMenuToggled(false));
    assert!(!state.split.menu_open);
}

#[test]
fn text_typed_message_replaces_the_live_value() {
    let mut state = AtomsState::new();
    state.apply(&AtomsMessage::TextTyped("typed in".to_string()));
    assert_eq!(state.text, "typed in");
    laid_out(PANE, text_input_section(&state));
}

#[test]
fn a_tier_writes_the_bound_model_and_keeps_its_name() {
    let mut state = AtomsState::new();
    state.apply(&AtomsMessage::TextInputTier(
        "commit",
        "committed".to_string(),
    ));
    assert_eq!(state.text_model.value, "committed");

    // A cancel tier carries the restored value, which is what the model
    // must show after Escape.
    state.apply(&AtomsMessage::TextInputTier(
        "cancel",
        "restored".to_string(),
    ));
    assert_eq!(state.text_model.value, "restored");
    laid_out(PANE, text_input_section(&state));
}

#[test]
fn the_invalid_field_is_touched_so_its_error_paints() {
    let state = AtomsState::new();
    assert!(
        state.text_invalid_model.punishment().painted(),
        "a withheld error would leave Ã‚Â§3.2's Invalid row undisplayed"
    );
    assert!(state.combo_invalid.is_invalid());
}

#[test]
fn the_live_bound_field_is_pristine_and_shows_no_error() {
    let state = AtomsState::new();
    assert!(state.text_model.is_pristine());
    assert!(!state.text_model.punishment().painted());
}

#[test]
fn reveal_toggle_flips_masking_and_renders() {
    let mut state = AtomsState::new();
    assert!(!state.revealed, "the password starts masked");

    state.apply(&AtomsMessage::RevealToggled(true));
    assert!(state.revealed);
    laid_out(PANE, password_section(&state));

    state.apply(&AtomsMessage::RevealToggled(false));
    assert!(!state.revealed);
    laid_out(PANE, password_section(&state));
}

#[test]
fn combo_flyout_opens_and_a_choice_closes_it() {
    let mut state = AtomsState::new();
    assert!(!state.combo.is_open(), "the flyout starts closed");

    state.apply(&AtomsMessage::ComboToggled(true));
    assert!(state.combo.is_open());
    laid_out(PANE, combo_box_section(&state));

    state.apply(&AtomsMessage::ComboChosen("pandoc".to_string()));
    assert_eq!(state.combo.selected(), Some(&"pandoc".to_string()));
    assert!(!state.combo.is_open(), "choosing must dismiss the flyout");
    laid_out(PANE, combo_box_section(&state));
}

#[test]
fn search_messages_drive_the_query() {
    let mut state = AtomsState::new();
    state.apply(&AtomsMessage::SearchChanged("md".to_string()));
    assert_eq!(state.search, "md");

    state.apply(&AtomsMessage::SearchSubmitted("md".to_string()));
    assert_eq!(state.search, "md");

    state.apply(&AtomsMessage::SearchCleared);
    assert!(state.search.is_empty());
    laid_out(PANE, search_box_section(&state));
}

#[test]
fn describe_mentions_every_message_shape() {
    let cases = [
        AtomsMessage::Checkbox(1, CheckState::Checked),
        AtomsMessage::RadioSelected("day".to_string()),
        AtomsMessage::SwitchToggled(true),
        AtomsMessage::SegmentedChanged(std::collections::BTreeSet::from(["week".to_string()])),
        AtomsMessage::SplitMenuToggled(true),
        AtomsMessage::SplitRowChosen("export-pdf".to_string()),
        AtomsMessage::LinkOpened("x".to_string()),
        AtomsMessage::TextTyped("hello".to_string()),
        AtomsMessage::TextInputTier("live", "hell".to_string()),
        AtomsMessage::PasswordTyped("s3cret".to_string()),
        AtomsMessage::RevealToggled(true),
        AtomsMessage::ComboToggled(true),
        AtomsMessage::ComboChosen("typst".to_string()),
        AtomsMessage::SearchChanged("q".to_string()),
        AtomsMessage::SearchSubmitted("q".to_string()),
        AtomsMessage::SearchCleared,
        AtomsMessage::RatingPicked(Some(3.0)),
        AtomsMessage::RatingPicked(None),
        AtomsMessage::RangeLive(10.0, 40.0),
        AtomsMessage::RangeCommitted(10.0, 40.0),
        AtomsMessage::DatePicked(DateSelection::SingleDate(Some(data::date((2026, 10, 5))))),
        AtomsMessage::DateRangePicked(DateSelection::DateRange {
            start: Some(data::date((2026, 10, 5))),
            end: Some(data::date((2026, 10, 8))),
        }),
        AtomsMessage::DateFlyoutToggled(true),
        AtomsMessage::TimePicked(data::time((7, 30, 0))),
        AtomsMessage::TimeFlyoutToggled(false),
        AtomsMessage::ColorPicked(data::rgb((1, 2, 3))),
        AtomsMessage::ColorFlyoutToggled(true),
        AtomsMessage::Note("hi".to_string()),
    ];
    for case in cases {
        assert!(
            !AtomsState::describe(&case).is_empty(),
            "{case:?} must produce status text"
        );
    }
}

#[test]
fn describe_never_echoes_the_secret() {
    let text = AtomsState::describe(&AtomsMessage::PasswordTyped(
        data::PASSWORD_SAMPLE.to_string(),
    ));
    assert!(
        !text.contains("correct"),
        "the status bar must not repeat a password: {text}"
    );
    let expected = data::PASSWORD_SAMPLE.chars().count().to_string();
    assert!(
        text.contains(&expected),
        "the status bar should report the length instead: {text}"
    );
}

#[test]
fn describe_names_the_tier_a_controlled_field_reported() {
    let text = AtomsState::describe(&AtomsMessage::TextInputTier("commit", "x".to_string()));
    assert!(text.contains("commit"), "tier name must be visible: {text}");
}

#[test]
fn rating_pick_updates_the_model_and_clear_is_a_none() {
    let mut state = AtomsState::new();
    state.apply(&AtomsMessage::RatingPicked(Some(2.0)));
    assert_eq!(state.rating.effective_value(), Some(2.0));
    laid_out(PANE, rating_section(&state));

    state.apply(&AtomsMessage::RatingPicked(None));
    assert_eq!(state.rating.effective_value(), None);
}

#[test]
fn range_scrub_and_commit_both_move_the_thumbs() {
    let mut state = AtomsState::new();
    state.apply(&AtomsMessage::RangeLive(20.0, 80.0));
    assert_eq!((state.range.lower(), state.range.upper()), (20.0, 80.0));

    state.apply(&AtomsMessage::RangeCommitted(30.0, 90.0));
    assert_eq!((state.range.lower(), state.range.upper()), (30.0, 90.0));
    laid_out(PANE, range_slider_section(&state));
}

#[test]
fn the_range_keeps_its_step_and_unit_when_rebuilt() {
    let state = AtomsState::new();
    assert_eq!(state.range.step(), Some(data::RANGE_STEP));
    assert_eq!(state.range.unit_suffix(), Some(data::RANGE_UNIT));
}

#[test]
fn date_selection_is_folded_back_into_the_model() {
    let mut state = AtomsState::new();
    let picked = data::date((2026, 10, 5));
    state.apply(&AtomsMessage::DatePicked(DateSelection::SingleDate(Some(
        picked,
    ))));
    laid_out(PANE, date_picker_section(&state));

    state.apply(&AtomsMessage::DateRangePicked(DateSelection::DateRange {
        start: Some(data::date((2026, 10, 1))),
        end: Some(data::date((2026, 10, 9))),
    }));
    laid_out(PANE, date_picker_section(&state));
}

#[test]
fn blackout_dates_are_refused_by_the_date_model() {
    let state = AtomsState::new();
    assert!(
        state.date.is_date_disabled(data::date((2026, 10, 16))),
        "the fixture's blackout day must be refused"
    );
    assert!(!state.date.is_date_disabled(data::date((2026, 10, 15))));
}

#[test]
fn time_pick_stores_the_value_and_closes_the_flyout() {
    let mut state = AtomsState::new();
    state.apply(&AtomsMessage::TimeFlyoutToggled(true));
    assert!(state.time.open);

    let picked = data::time((14, 45, 0));
    state.apply(&AtomsMessage::TimePicked(picked));
    assert_eq!(state.time.value, Some(picked));
    assert!(!state.time.open, "committing must dismiss the drum");
    laid_out(PANE, time_picker_section(&state));
}

#[test]
fn colour_pick_stores_the_value_and_the_flyout_latches() {
    let mut state = AtomsState::new();
    state.apply(&AtomsMessage::ColorFlyoutToggled(true));
    assert!(state.color.open);

    let picked = data::rgb((10, 200, 40));
    state.apply(&AtomsMessage::ColorPicked(picked));
    assert_eq!(state.color.value, picked);
    laid_out(PANE, color_picker_section(&state));
}

#[test]
fn app_routes_atoms_messages_into_the_atoms_state() {
    let (mut app, _) = DemoApp::boot();
    let _ = app.update(Message::Atoms(AtomsMessage::SwitchToggled(false)));
    assert!(!app.atoms().switch_on);
}

#[test]
fn app_routes_a_combo_choice_into_the_pane_model() {
    let (mut app, _) = DemoApp::boot();
    let _ = app.update(Message::Atoms(AtomsMessage::ComboChosen(
        "chromium".to_string(),
    )));
    assert_eq!(app.atoms().combo.selected(), Some(&"chromium".to_string()));
}

/// The app's status-bar total is chrome + the atoms pane + the composites pane.
///
/// Lives here because the atoms side is the largest contributor; the
/// composites side asserts its own share in `pane_composites_tests`.
#[test]
fn atoms_widgets_are_counted_by_the_app() {
    let (app, _) = DemoApp::boot();
    let atoms = live_sections(app.atoms())
        .iter()
        .map(|s| s.widgets)
        .sum::<usize>();
    let expected = chrome::WIDGET_COUNT + atoms + app.composites().widget_count();
    assert_eq!(app.widget_count(), expected);
}
// ---------------------------------------------------------------------------
// R1 regression — the Buttons-row control columns share one left guide
// ---------------------------------------------------------------------------

/// The atoms pane's content width inside the 1400x900 demo window.
///
/// A focused capture measured at this width sees the same `x` values the
/// audit measures in the real window, so a pass here means the same thing
/// a pass in the audit golden means.
const PANE_WIDTH: f32 = 695.0;

/// The `Primary` / `Secondary` / `Ghost` / `Danger` button id stems.
///
/// Button ids carry the title-cased variant name, not the slugged label
/// id, so the two id families cannot share one list.
const VARIANT_STEMS: [&str; 4] = ["Primary", "Secondary", "Ghost", "Danger"];

/// Captures `element` at the real pane width and hands back the probe.
fn probe_of<'a>(element: impl Into<Element<'a, Message>>) -> Probe {
    let root: Element<'_, Message> = iced::widget::container(element.into())
        .width(Length::Fixed(PANE_WIDTH))
        .into();
    Probe::capture(Size::new(PANE_WIDTH, 900.0), root)
}

/// The recorded left edge of `id`, failing loudly when it was never drawn.
fn left_of(probe: &Probe, id: &str) -> f32 {
    probe
        .find(id)
        .unwrap_or_else(|| panic!("`{id}` was not recorded by the probe"))
        .bounds
        .x
}

/// The distance between the smallest and largest of `values`.
fn spread(values: &[f32]) -> f32 {
    let min = values.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    max - min
}

/// The natural, *unpinned* width of a Buttons-row label's text box.
///
/// Rendered through [`crate::section::labeled`] rather than
/// [`crate::section::labeled_column`] so the probe reports the word's own
/// width instead of the column it is normally forced into.
fn natural_label_width(name: &str) -> f32 {
    let label = crate::section::labeled(
        name,
        typography::UI_COMPACT,
        Palette::dark().text_muted,
        "demo.scratch.natural-label",
    );
    probe_of(label)
        .find("demo.scratch.natural-label")
        .expect("the scratch label must be recorded")
        .bounds
        .width
}

#[test]
fn button_rows_share_one_left_guide_per_control_column() {
    let state = AtomsState::new();
    let probe = probe_of(buttons_section(&state));

    let col1: Vec<f32> = VARIANT_STEMS
        .iter()
        .map(|stem| left_of(&probe, &format!("demo.atoms.button.{stem}.live")))
        .collect();
    let col2: Vec<f32> = VARIANT_STEMS
        .iter()
        .map(|stem| left_of(&probe, &format!("demo.atoms.button.{stem}.busy")))
        .collect();

    assert_eq!(col1.len(), VARIANT_STEMS.len());
    assert_eq!(col2.len(), VARIANT_STEMS.len());
    assert!(
        spread(&col1) <= crate::alignment_audit::TOLERANCE,
        "column 1 of the Buttons rows is ragged: {col1:?} spreads {}px",
        spread(&col1)
    );
    assert!(
        spread(&col2) <= crate::alignment_audit::TOLERANCE,
        "column 2 of the Buttons rows is ragged: {col2:?} spreads {}px",
        spread(&col2)
    );
}

#[test]
fn button_rows_start_at_the_column_not_at_their_words() {
    let state = AtomsState::new();
    let probe = probe_of(buttons_section(&state));

    let expected_col1 = hoarfrost::composite::card::PADDING + LABEL_COLUMN + space::SM;
    for stem in VARIANT_STEMS {
        assert!(
            (left_of(&probe, &format!("demo.atoms.button.{stem}.live")) - expected_col1).abs()
                <= crate::alignment_audit::TOLERANCE,
            "{stem}'s first control must start at the column edge {expected_col1}, got {}",
            left_of(&probe, &format!("demo.atoms.button.{stem}.live"))
        );
    }
}

#[test]
fn label_column_is_the_smallest_grid_value_that_clears_the_longest_label() {
    let widths: Vec<f32> = BUTTON_VARIANTS
        .iter()
        .map(|(name, _)| natural_label_width(name))
        .collect();
    let longest = widths.iter().cloned().fold(0.0f32, f32::max);

    assert!(
        LABEL_COLUMN >= longest,
        "LABEL_COLUMN {LABEL_COLUMN} clips the longest variant label ({longest}px): {widths:?}"
    );
    assert!(
        (LABEL_COLUMN % space::XS).abs() < f32::EPSILON,
        "LABEL_COLUMN {LABEL_COLUMN} must sit on the base-4 spatial grid"
    );
    assert!(
        LABEL_COLUMN - space::XS < longest,
        "LABEL_COLUMN is not the smallest grid value that fits: {} would already clear {longest}px",
        LABEL_COLUMN - space::XS
    );
}

// ---------------------------------------------------------------------------
// The text fields' 12px inset is the framework's own padding, not a demo offset
// ---------------------------------------------------------------------------

#[test]
fn text_field_frames_are_flush_with_their_section_caption() {
    let state = AtomsState::new();
    let sections = vec![text_input_section(&state), password_section(&state)];

    for section in sections {
        let id = section.id.clone();
        let probe = probe_of(section);
        let caption_left = left_of(&probe, &format!("{id}.caption"));
        let card_width = probe
            .find(&id)
            .unwrap_or_else(|| panic!("section card `{id}` was not recorded"))
            .bounds
            .width;
        let content_width = card_width - 2.0 * hoarfrost::composite::card::PADDING;

        let fields: Vec<&Entry> = probe
            .with_prefix("demo.atoms.")
            .into_iter()
            .filter(|entry| entry.kind == "text_input")
            .collect();
        assert!(!fields.is_empty(), "{id} recorded no text_input at all");

        let right_inset = text_input::HORIZONTAL_PADDING
            + text_input::SLOT_GAP
            + hoarfrost::token::size::HIT_TARGET_MIN;

        for field in fields {
            let frame_left = field.bounds.x - text_input::HORIZONTAL_PADDING;
            assert!(
                (frame_left - caption_left).abs() <= crate::alignment_audit::TOLERANCE,
                "{id}: `{}`'s FRAME starts at {frame_left}, not flush with the caption at \
                 {caption_left} (record x={}, HORIZONTAL_PADDING={})",
                field.id,
                field.bounds.x,
                text_input::HORIZONTAL_PADDING
            );
            assert!(
                (field.bounds.width
                    - (content_width - text_input::HORIZONTAL_PADDING - right_inset))
                    .abs()
                    <= crate::alignment_audit::TOLERANCE,
                "{id}: `{}` is {} wide against a predicted text area of {}",
                field.id,
                field.bounds.width,
                content_width - text_input::HORIZONTAL_PADDING - right_inset
            );
        }
    }
}
