//! Tests for the composite gallery subjects (`UIF-COMP-001`,
//! `UIF-COMP-002`, `UIF-COMP-005`).

use crate::composite::card::{self, CardElevation};
use crate::composite::form_field::{self, CaptionKind, OverflowPolicy};
use crate::composite::list_tile::{self, Leading, LeadingExtent, TileLines};
use crate::composite::settings_row::{self, DEFAULT_CONTROL_HEIGHT};
use crate::gallery::{
    AuditClass, Cell, CellKind, ContrastRole, StyleEntry, StyleValue, judge, measure, rgb8,
    threshold,
};
use crate::gallery_composite::*;
use crate::state::BaseState;
use crate::token::color::Palette;
use crate::token::{shape, size, space};
use iced::{Point, Rectangle};

/// Tolerance for geometry read back out of a painted frame, which is
/// rounded to sub-pixel precision by the recorder.
const EPS: f32 = 1e-3;

/// Frame layers that carry a real painted fill rather than a reservation
/// marker.
///
/// A composite painting one of these owns the surface its own text is
/// read on, so it is not required to name a mount. Every other composite
/// band is drawn as a transparent marker and must say what it sits on.
const PAINTED_SURFACE_LAYERS: [&str; 4] = ["card", "row", "bar", "toast"];

fn by_id(id: &str) -> Cell {
    composite_cells()
        .into_iter()
        .find(|cell| cell.id == id)
        .unwrap_or_else(|| panic!("no composite cell {id}"))
}

fn entry<'a>(cell: &'a Cell, channel: &str) -> &'a StyleEntry {
    cell.resolved
        .iter()
        .find(|entry| entry.channel == channel)
        .unwrap_or_else(|| panic!("{} has no channel {channel}", cell.id))
}

fn number(cell: &Cell, channel: &str) -> f32 {
    match entry(cell, channel).value {
        StyleValue::Number(value) => value,
        ref other => panic!("{} {channel} is not a number: {other:?}", cell.id),
    }
}

fn text_of(cell: &Cell, channel: &str) -> String {
    match &entry(cell, channel).value {
        StyleValue::Text(text) => text.clone(),
        ref other => panic!("{} {channel} is not text: {other:?}", cell.id),
    }
}

fn colour_token(cell: &Cell, channel: &str) -> String {
    match &entry(cell, channel).value {
        StyleValue::Colour { token, .. } => token.clone(),
        ref other => panic!("{} {channel} is not a colour: {other:?}", cell.id),
    }
}

fn quad(cell: &Cell, layer: &str) -> crate::gallery::FrameQuad {
    cell.frame
        .quads
        .iter()
        .find(|q| q.layer == layer)
        .unwrap_or_else(|| panic!("{} drew no layer {layer}", cell.id))
        .clone()
}

fn geometry(cell: &Cell) -> Vec<(String, f32)> {
    cell.resolved
        .iter()
        .filter(|entry| entry.channel.starts_with("geometry."))
        .map(|entry| (entry.channel.to_string(), number(cell, entry.channel)))
        .collect()
}

fn sample(width: f32, height: f32) -> Rectangle {
    Rectangle::new(Point::ORIGIN, iced::Size::new(width, height))
}

// ---------------------------------------------------------------------------
// Matrix shape
// ---------------------------------------------------------------------------

#[test]
fn the_composite_cells_are_in_spec_order_and_tagged() {
    let expected = [
        ("form-field/empty", "UIF-COMP-001", "form-field"),
        ("form-field/helper", "UIF-COMP-001", "form-field"),
        ("form-field/error", "UIF-COMP-001", "form-field"),
        ("form-field/host/button", "UIF-COMP-001", "form-field"),
        ("form-field/host/input", "UIF-COMP-001", "form-field"),
        ("form-field/host/row", "UIF-COMP-001", "form-field"),
        ("settings-row/title-only", "UIF-COMP-001", "settings-row"),
        (
            "settings-row/with-description",
            "UIF-COMP-001",
            "settings-row",
        ),
        ("settings-row/disabled", "UIF-COMP-001", "settings-row"),
        ("info-bar/info/raised/bare", "UIF-COMP-002", "info-bar"),
        ("info-bar/success/raised/bare", "UIF-COMP-002", "info-bar"),
        ("info-bar/warning/raised/bare", "UIF-COMP-002", "info-bar"),
        ("info-bar/danger/raised/bare", "UIF-COMP-002", "info-bar"),
        ("info-bar/info/raised/action", "UIF-COMP-002", "info-bar"),
        ("info-bar/success/raised/action", "UIF-COMP-002", "info-bar"),
        ("info-bar/warning/raised/action", "UIF-COMP-002", "info-bar"),
        ("info-bar/danger/raised/action", "UIF-COMP-002", "info-bar"),
        (
            "info-bar/info/raised/action-dismiss",
            "UIF-COMP-002",
            "info-bar",
        ),
        (
            "info-bar/success/raised/action-dismiss",
            "UIF-COMP-002",
            "info-bar",
        ),
        (
            "info-bar/warning/raised/action-dismiss",
            "UIF-COMP-002",
            "info-bar",
        ),
        (
            "info-bar/danger/raised/action-dismiss",
            "UIF-COMP-002",
            "info-bar",
        ),
        ("info-bar/info/overlay/bare", "UIF-COMP-002", "info-bar"),
        ("info-bar/success/overlay/bare", "UIF-COMP-002", "info-bar"),
        ("info-bar/warning/overlay/bare", "UIF-COMP-002", "info-bar"),
        ("info-bar/danger/overlay/bare", "UIF-COMP-002", "info-bar"),
        ("info-bar/info/overlay/action", "UIF-COMP-002", "info-bar"),
        (
            "info-bar/success/overlay/action",
            "UIF-COMP-002",
            "info-bar",
        ),
        (
            "info-bar/warning/overlay/action",
            "UIF-COMP-002",
            "info-bar",
        ),
        ("info-bar/danger/overlay/action", "UIF-COMP-002", "info-bar"),
        (
            "info-bar/info/overlay/action-dismiss",
            "UIF-COMP-002",
            "info-bar",
        ),
        (
            "info-bar/success/overlay/action-dismiss",
            "UIF-COMP-002",
            "info-bar",
        ),
        (
            "info-bar/warning/overlay/action-dismiss",
            "UIF-COMP-002",
            "info-bar",
        ),
        (
            "info-bar/danger/overlay/action-dismiss",
            "UIF-COMP-002",
            "info-bar",
        ),
        ("toast/armed", "UIF-COMP-002", "toast"),
        ("toast/hover-held", "UIF-COMP-002", "toast"),
        ("toast/focus-held", "UIF-COMP-002", "toast"),
        ("toast/persistent-warning", "UIF-COMP-002", "toast"),
        ("toast/persistent-danger", "UIF-COMP-002", "toast"),
        ("toast/stack-three", "UIF-COMP-002", "toast"),
        ("toast/queued", "UIF-COMP-002", "toast"),
        ("toast/narrow-viewport", "UIF-COMP-002", "toast"),
        ("expander/collapsed", "UIF-COMP-003", "expander"),
        ("expander/open", "UIF-COMP-003", "expander"),
        ("expander/hovered", "UIF-COMP-003", "expander"),
        ("expander/pressed", "UIF-COMP-003", "expander"),
        ("expander/disabled", "UIF-COMP-003", "expander"),
        ("expander/compact-collapsed", "UIF-COMP-003", "expander"),
        ("expander/compact-open", "UIF-COMP-003", "expander"),
        ("scroll-view/overlay-rest", "UIF-COMP-004", "scroll-view"),
        ("scroll-view/overlay-hovered", "UIF-COMP-004", "scroll-view"),
        ("scroll-view/overlay-dragged", "UIF-COMP-004", "scroll-view"),
        ("scroll-view/reserved-rest", "UIF-COMP-004", "scroll-view"),
        (
            "scroll-view/reserved-hovered",
            "UIF-COMP-004",
            "scroll-view",
        ),
        ("scroll-view/extreme-ratio", "UIF-COMP-004", "scroll-view"),
        (
            "scroll-view/hidden-unscrollable",
            "UIF-COMP-004",
            "scroll-view",
        ),
        ("scroll-view/annotated", "UIF-COMP-004", "scroll-view"),
        ("card/rest", "UIF-COMP-005", "card"),
        ("card/elevated", "UIF-COMP-005", "card"),
        ("list-tile/single", "UIF-COMP-005", "list-tile"),
        ("list-tile/two-line", "UIF-COMP-005", "list-tile"),
        ("list-tile/single-selected", "UIF-COMP-005", "list-tile"),
        ("list-tile/two-line-selected", "UIF-COMP-005", "list-tile"),
        ("list-tile/hovered", "UIF-COMP-005", "list-tile"),
        ("list-tile/pressed", "UIF-COMP-005", "list-tile"),
        ("list-tile/disabled", "UIF-COMP-005", "list-tile"),
        ("list-tile/non-interactive", "UIF-COMP-005", "list-tile"),
    ];
    let cells = composite_cells();
    assert_eq!(
        cells.len(),
        expected.len(),
        "the composite block emitted {} cells, the plan says {}",
        cells.len(),
        expected.len()
    );
    for (cell, (id, spec, subject)) in cells.iter().zip(expected) {
        assert_eq!(cell.id, id);
        assert_eq!(cell.spec, spec);
        assert_eq!(cell.subject, subject);
        assert_eq!(cell.kind, CellKind::Composite, "{} is not a composite", id);
    }
}

#[test]
fn every_composite_cell_declares_the_surface_its_text_is_read_on() {
    for cell in composite_cells() {
        assert!(!cell.resolved.is_empty(), "{} resolved nothing", cell.id);
        assert!(
            !cell.audit.is_empty(),
            "{} audits nothing, so its colours are unverified",
            cell.id
        );

        // A composite that paints no surface of its own has to say what it
        // is mounted on, or its text contrast is measured against nothing.
        let declares_its_surface = cell
            .resolved
            .iter()
            .any(|entry| matches!(entry.channel, "mount" | "audit.background"));
        let paints_its_own_surface = cell.frame.quads.iter().any(|quad| {
            PAINTED_SURFACE_LAYERS.contains(&quad.layer.as_str()) && quad.fill != "transparent"
        });

        assert!(
            declares_its_surface || paints_its_own_surface,
            "{} neither names the surface its text is read on nor paints one",
            cell.id
        );

        for pair in &cell.audit {
            assert!(
                pair.background.a > 0.0,
                "{} {} is measured against a background that paints nothing",
                cell.id,
                pair.channel
            );
        }
    }
}

#[test]
fn every_composite_audit_pair_clears_its_gate() {
    let mut judged = 0usize;
    for cell in composite_cells() {
        for pair in &cell.audit {
            let (ratio, _, class) = measure(pair);
            assert!(
                judge(pair),
                "{} {} {} on {} measures {ratio} against a floor of {}",
                cell.id,
                pair.channel,
                rgb8(pair.foreground),
                rgb8(pair.background),
                threshold(pair.role)
            );
            if class == AuditClass::Judged {
                judged += 1;
            }
        }
    }
    assert!(judged > 10, "only {judged} composite pairs were judged");
}

#[test]
fn the_resting_settings_row_border_is_the_exempt_class_not_a_pass() {
    let palette = Palette::dark();
    let cell = by_id("settings-row/title-only");
    let pair = cell
        .audit
        .iter()
        .find(|pair| pair.channel == "border")
        .expect("the settings row audits its border");

    assert_eq!(pair.foreground, palette.border_subtle);
    assert_eq!(
        measure(pair).2,
        AuditClass::RestingDivider,
        "a resting hairline must be classified, not waved through as a pass"
    );
}

// ---------------------------------------------------------------------------
// FormField — UIF-COMP-001
// ---------------------------------------------------------------------------

#[test]
fn the_three_caption_postures_differ_in_voice_not_in_geometry() {
    let empty = by_id("form-field/empty");
    let helper = by_id("form-field/helper");
    let error = by_id("form-field/error");

    assert_eq!(
        geometry(&empty),
        geometry(&helper),
        "a helper text moved the field"
    );
    assert_eq!(
        geometry(&empty),
        geometry(&error),
        "a validation error moved the field"
    );
    assert_eq!(text_of(&empty, "caption.kind"), "empty");
    assert_eq!(text_of(&helper, "caption.kind"), "helper");
    assert_eq!(text_of(&error, "caption.kind"), "error");
}

#[test]
fn the_reserved_bands_are_the_spec_rhythm() {
    let bands = form_field::bands(sample(280.0, form_field::stacked_height()));
    assert_eq!(bands.header.height, form_field::HEADER_HEIGHT);
    assert_eq!(bands.control.height, form_field::CONTROL_SLOT_HEIGHT);
    assert_eq!(bands.caption.height, form_field::CAPTION_SLOT_HEIGHT);
    assert_eq!(form_field::BAND_GAP, space::XS);
    assert_eq!(
        form_field::stacked_height(),
        form_field::HEADER_HEIGHT
            + form_field::BAND_GAP
            + form_field::CONTROL_SLOT_HEIGHT
            + form_field::BAND_GAP
            + form_field::CAPTION_SLOT_HEIGHT
    );
}

#[test]
fn the_caption_band_is_reserved_whatever_it_says() {
    let palette = Palette::dark();
    let postures = [
        (true, Some("helper"), None),
        (true, None, None),
        (false, None, Some("Name is taken")),
        (false, None, None),
    ];
    for (valid, helper, error) in postures {
        let caption = form_field::resolve_caption(valid, helper, error, &palette);
        let bands = form_field::bands(sample(280.0, form_field::stacked_height()));
        assert_eq!(
            bands.caption.height,
            form_field::reserved_band_height(
                form_field::CAPTION_SLOT_HEIGHT,
                form_field::RESERVED_BAND_OVERFLOW
            ),
            "{caption:?} changed the reserved height"
        );
    }
}

#[test]
fn the_reserved_height_depends_on_the_reservation_not_on_the_overflow() {
    for reserved in [8.0f32, 16.0, 24.0] {
        assert_eq!(
            form_field::reserved_band_height(reserved, OverflowPolicy::Clip),
            reserved
        );
        assert_eq!(
            form_field::reserved_band_height(reserved, OverflowPolicy::Wrap),
            reserved
        );
    }
    assert_eq!(
        form_field::RESERVED_BAND_OVERFLOW,
        OverflowPolicy::Clip,
        "wrapping is the one mode that would grow the band, so it must not be the policy"
    );
}

#[test]
fn the_error_voice_carries_the_glyph_and_the_danger_colour() {
    let palette = Palette::dark();
    let caption =
        form_field::resolve_caption(false, Some("helper"), Some("Name is taken"), &palette);
    assert_eq!(caption.kind, CaptionKind::Error);
    assert_eq!(caption.text, Some("Name is taken"));
    assert_eq!(caption.glyph, Some(form_field::WARNING_GLYPH));
    assert_eq!(caption.colour, palette.status_danger_text);
}

#[test]
fn the_helper_voice_is_muted_and_unmarked() {
    let palette = Palette::dark();
    let caption = form_field::resolve_caption(true, Some("Shown at the top"), None, &palette);
    assert_eq!(caption.kind, CaptionKind::Helper);
    assert_eq!(caption.glyph, None);
    assert_eq!(caption.colour, palette.text_muted);
}

#[test]
fn an_invalid_field_with_no_message_says_nothing() {
    let palette = Palette::dark();
    let caption = form_field::resolve_caption(false, Some("helper"), None, &palette);
    assert_eq!(
        caption.kind,
        CaptionKind::Empty,
        "the framework must not invent a fault the host never reported"
    );
    assert_eq!(caption.text, None);
    assert_eq!(caption.glyph, None);
}

#[test]
fn clipping_is_the_overflow_policy_and_costs_no_height() {
    assert_eq!(
        form_field::reserved_band_height(16.0, form_field::RESERVED_BAND_OVERFLOW),
        16.0
    );
    assert_eq!(
        form_field::RESERVED_BAND_OVERFLOW,
        form_field::OverflowPolicy::Clip
    );
    assert_eq!(
        text_of(&by_id("form-field/error"), "overflow.policy"),
        "clip-single-line"
    );
}

#[test]
fn three_different_atom_kinds_share_one_control_slot() {
    let button = by_id("form-field/host/button");
    let input = by_id("form-field/host/input");
    let row = by_id("form-field/host/row");

    for cell in [&button, &input, &row] {
        assert_eq!(
            number(cell, "geometry.control.height"),
            form_field::CONTROL_SLOT_HEIGHT,
            "{} resized the slot to its host",
            cell.id
        );
        assert_eq!(
            number(cell, "geometry.total-height"),
            form_field::stacked_height(),
            "{} resized the field to its host",
            cell.id
        );
        assert_eq!(
            text_of(cell, "host.slot-depends-on-control"),
            "no",
            "{} claims the slot depends on the control",
            cell.id
        );
    }

    let hosts = [
        text_of(&button, "host.kind"),
        text_of(&input, "host.kind"),
        text_of(&row, "host.kind"),
    ];
    for host in ["button", "text-input", "focusable-row"] {
        assert!(hosts.contains(&host.to_string()), "{host} was never hosted");
    }

    let natural = [
        number(&button, "host.natural-height"),
        number(&input, "host.natural-height"),
        number(&row, "host.natural-height"),
    ];
    assert_ne!(natural[0], natural[1]);
    assert_ne!(natural[1], natural[2]);
    assert!(
        natural
            .iter()
            .any(|h| *h != form_field::CONTROL_SLOT_HEIGHT),
        "every host is exactly the slot height, so nothing here proves the \
         slot does not shrink-wrap: {natural:?}"
    );
}

// ---------------------------------------------------------------------------
// SettingsRow — UIF-COMP-001
// ---------------------------------------------------------------------------

#[test]
fn the_printed_minimum_is_a_floor_not_a_height() {
    let one_line = settings_row::row_height(false, DEFAULT_CONTROL_HEIGHT);
    let two_line = settings_row::row_height(true, DEFAULT_CONTROL_HEIGHT);

    assert_eq!(settings_row::MIN_HEIGHT, 48.0);
    assert!(
        two_line > settings_row::MIN_HEIGHT,
        "a two-line row is {} and the floor is {}, so the printed 48 \
         cannot be the row's height",
        two_line,
        settings_row::MIN_HEIGHT
    );
    assert!(
        one_line >= settings_row::MIN_HEIGHT,
        "a one-line row fell below the floor at {one_line}"
    );
}

#[test]
fn a_tall_control_grows_the_row_and_a_short_one_cannot_pull_it_under_the_floor() {
    let tall = settings_row::row_height(false, size::CONTROL_LG);
    let tiny = settings_row::row_height(false, 8.0);

    assert!(
        tall > settings_row::MIN_HEIGHT,
        "a {tall}px control did not grow the row past the floor"
    );
    assert!(
        tiny >= settings_row::MIN_HEIGHT,
        "an 8px control pulled the row to {tiny}, under the floor"
    );
    assert!(tall > tiny, "the row ignored the taller control");
}

#[test]
fn the_control_column_is_reserved_before_the_text_takes_the_rest() {
    let bounds = sample(300.0, 60.0);
    let split = settings_row::bands(bounds, size::CONTROL_LG);
    let interior = bounds.width - 2.0 * settings_row::PADDING_HORIZONTAL;

    let right_edge = bounds.x + bounds.width - settings_row::PADDING_HORIZONTAL;

    assert_eq!(split.control.width, size::CONTROL_LG);
    assert_eq!(
        split.text.width + split.control.width + settings_row::COLUMN_GAP,
        interior,
        "the two columns and the gap must exactly fill the interior"
    );
    assert_eq!(
        split.control.x + split.control.width,
        right_edge,
        "the control column ends flush against the row's padding"
    );
    assert_eq!(split.text.x, bounds.x + settings_row::PADDING_HORIZONTAL);
}

#[test]
fn a_pathological_control_clamps_instead_of_going_negative() {
    let bounds = sample(100.0, 48.0);
    let split = settings_row::bands(bounds, 10_000.0);
    assert!(
        split.text.width >= 0.0 && split.control.width >= 0.0,
        "an oversized control produced negative columns: {split:?}"
    );
    assert_eq!(split.text.width, 0.0);
}

#[test]
fn a_disabled_row_dims_both_lines_together() {
    let palette = Palette::dark();
    let live = settings_row::resolve_style(false, &palette);
    let dead = settings_row::resolve_style(true, &palette);

    assert_eq!(live.title_colour, palette.text_strong);
    assert_eq!(live.description_colour, palette.text_muted);
    assert_eq!(dead.title_colour, palette.text_disabled);
    assert_eq!(dead.description_colour, palette.text_disabled);
    assert_eq!(live.background, dead.background);
    assert_eq!(live.border_color, dead.border_color);
}

// ---------------------------------------------------------------------------
// Card — UIF-COMP-005
// ---------------------------------------------------------------------------

#[test]
fn the_card_insets_its_content_by_its_padding_and_its_stroke() {
    let palette = Palette::dark();
    let style = card::resolve_style(CardElevation::Rest, &palette);
    let outer = sample(320.0, 160.0);
    let inner = card::content_bounds(outer, &style);
    let inset = card::PADDING + style.border_width;

    assert_eq!(inner.width, outer.width - 2.0 * inset);
    assert_eq!(inner.height, outer.height - 2.0 * inset);
    assert_eq!(inner.x, outer.x + inset);
    assert_eq!(inner.y, outer.y + inset);
}

#[test]
fn elevation_changes_the_borders_role_not_the_cards_shape() {
    let rest = by_id("card/rest");
    let raised = by_id("card/elevated");

    assert_eq!(text_of(&rest, "border.role"), "resting-separation");
    assert_eq!(text_of(&raised, "border.role"), "active-indicator");
    assert_eq!(
        number(&rest, "content.width"),
        number(&raised, "content.width")
    );
    assert_eq!(
        number(&rest, "content.height"),
        number(&raised, "content.height")
    );
    assert_eq!(number(&rest, "radius"), number(&raised, "radius"));
    assert_eq!(
        number(&rest, "border.width"),
        number(&raised, "border.width"),
        "both elevations wear the same hairline; the elevation is the colour"
    );
    assert_ne!(
        colour_token(&rest, "border.color"),
        colour_token(&raised, "border.color"),
        "the two elevations must not be indistinguishable"
    );
    assert_eq!(colour_token(&rest, "border.color"), "border.subtle");
    assert_eq!(colour_token(&raised, "border.color"), "border.strong");
}

#[test]
fn the_elevated_card_border_is_judged_chrome_and_the_resting_one_is_not() {
    let palette = Palette::dark();
    let rest = card::resolve_style(CardElevation::Rest, &palette);
    let raised = card::resolve_style(CardElevation::Elevated, &palette);

    assert!(!CardElevation::Rest.is_active_indicator());
    assert!(CardElevation::Elevated.is_active_indicator());
    assert_eq!(rest.border_color, palette.border_subtle);
    assert_eq!(raised.border_color, palette.border_strong);
    assert_eq!(
        measure(&crate::gallery::AuditPair {
            channel: "border",
            foreground: raised.border_color,
            background: raised.background,
            role: ContrastRole::Chrome,
        })
        .2,
        AuditClass::Judged
    );
}

// ---------------------------------------------------------------------------
// ListTile — UIF-COMP-005, D-8
// ---------------------------------------------------------------------------

/// Both figures are constants, so the gap between the printed two-line
/// height and the parts that compose it is checked at compile time: a
/// retune that closes or widens the gap fails the build instead of
/// quietly changing what the gallery reports.
const _: () = assert!(list_tile::TWO_LINE_PARTS_HEIGHT < list_tile::TWO_LINE_HEIGHT);
const _: () = assert!(list_tile::TWO_LINE_UNACCOUNTED > 0.0);

#[test]
fn the_two_line_arithmetic_gap_is_recorded_not_rounded_away() {
    assert_eq!(list_tile::TWO_LINE_HEIGHT, 44.0);
    assert_eq!(
        list_tile::TWO_LINE_UNACCOUNTED,
        list_tile::TWO_LINE_HEIGHT - list_tile::TWO_LINE_PARTS_HEIGHT
    );
    assert!(
        (list_tile::TWO_LINE_UNACCOUNTED - 2.0).abs() < 1e-6,
        "the printed 44 and the composed parts differ by {}, which is not \
         the 2px recorded as unaccounted",
        list_tile::TWO_LINE_UNACCOUNTED
    );
}

#[test]
fn the_slot_grid_is_invariant_to_selection_and_pointer_state() {
    let bounds = sample(300.0, list_tile::tile_height(TileLines::Two));
    let reference = list_tile::slots(bounds, Leading::Checkbox, true, 56.0);
    for selected in [false, true] {
        for base in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
            let style = list_tile::resolve_style(selected, false, base, &Palette::dark());
            let grid = list_tile::slots(bounds, Leading::Checkbox, true, 56.0);
            assert_eq!(grid, reference, "{base:?}/{selected} moved the slots");
            assert_eq!(
                grid.center.width
                    + grid.leading.width
                    + grid.trailing.width
                    + 2.0 * list_tile::SLOT_GAP,
                bounds.width - 2.0 * list_tile::PADDING_HORIZONTAL
            );
            let _ = style;
        }
    }
}

#[test]
fn an_empty_leading_collapses_rather_than_indenting_the_title() {
    let bounds = sample(300.0, list_tile::tile_height(TileLines::Single));
    let bare = list_tile::slots(bounds, Leading::None, true, 56.0);
    let marked = list_tile::slots(bounds, Leading::Checkbox, true, 56.0);

    assert_eq!(bare.leading.width, 0.0);
    assert_eq!(bare.center.x, bounds.x + list_tile::PADDING_HORIZONTAL);
    assert!(
        marked.center.x > bare.center.x,
        "a marked row must start further right than an unmarked one"
    );
    assert!(
        bare.center.width > marked.center.width,
        "the collapsed leading must return its width to the title"
    );
}

#[test]
fn a_leading_mark_inflates_to_the_hit_floor_only_when_it_is_a_target() {
    let icon = Leading::Icon(LeadingExtent::Small);
    assert_eq!(icon.visual_extent(), list_tile::LEADING_SMALL);
    assert_eq!(icon.hit_extent(false), list_tile::LEADING_SMALL);
    assert_eq!(icon.hit_extent(true), size::HIT_TARGET_MIN);
    assert_eq!(Leading::None.hit_extent(true), 0.0);
    assert_eq!(Leading::None.hit_extent(false), 0.0);
    assert_eq!(
        Leading::Icon(LeadingExtent::Large).hit_extent(true),
        list_tile::LEADING_LARGE
    );
}

#[test]
fn the_whole_row_is_the_target_only_when_the_tile_is_interactive() {
    let bounds = sample(300.0, list_tile::tile_height(TileLines::Single));
    assert_eq!(list_tile::hit_target(bounds, true), Some(bounds));
    assert_eq!(list_tile::hit_target(bounds, false), None);
    assert_eq!(
        number(&by_id("list-tile/non-interactive"), "geometry.hit.height"),
        0.0
    );
    assert_eq!(
        number(&by_id("list-tile/single"), "geometry.hit.height"),
        list_tile::SINGLE_LINE_HEIGHT
    );
}

#[test]
fn both_rungs_clear_the_hit_target_floor() {
    assert!(list_tile::meets_hit_target_floor(TileLines::Single));
    assert!(list_tile::meets_hit_target_floor(TileLines::Two));
    for lines in [TileLines::Single, TileLines::Two] {
        assert!(list_tile::tile_height(lines) >= size::HIT_TARGET_MIN);
    }
}

#[test]
fn trailing_targets_are_square_contained_and_right_aligned() {
    let bounds = sample(300.0, list_tile::tile_height(TileLines::Single));
    let targets = list_tile::trailing_targets(bounds, 2);
    assert_eq!(targets.len(), 2);
    let right = bounds.x + bounds.width - list_tile::PADDING_HORIZONTAL;
    for target in &targets {
        assert_eq!(target.width, list_tile::TRAILING_TARGET);
        assert_eq!(target.height, list_tile::TRAILING_TARGET);
        assert!(
            target.x + target.width <= right + EPS,
            "target overruns the row"
        );
        assert!(target.y >= bounds.y - EPS);
        assert!(target.y + target.height <= bounds.y + bounds.height + EPS);
    }
    assert!(
        targets[0].x < targets[1].x,
        "targets are not laid out in order"
    );
    assert_eq!(
        targets[1].x + targets[1].width,
        right,
        "the last target must sit flush against the padding"
    );
}

#[test]
fn a_selected_tile_promotes_its_subtitle_because_muted_cannot_read_on_accent() {
    let palette = Palette::dark();
    let ratio = crate::token::contrast::contrast_ratio(palette.text_muted, palette.accent_fill);
    let style = list_tile::resolve_style(true, false, BaseState::Rest, &palette);

    assert!(
        ratio < crate::token::contrast::MIN_TEXT_CONTRAST,
        "the promotion is unnecessary: muted reads at {ratio} on accent.fill"
    );
    assert_eq!(style.subtitle_colour, palette.text_strong);
    assert_eq!(
        list_tile::subtitle_on(palette.accent_fill, &palette),
        palette.text_strong
    );
    assert_eq!(
        list_tile::subtitle_on(palette.surface_raised, &palette),
        palette.text_muted,
        "an unselected tile keeps the muted subtitle"
    );
}

#[test]
fn a_selected_tile_wears_no_perimeter_because_accent_border_cannot_be_seen_on_accent() {
    let palette = Palette::dark();
    let ratio = crate::token::contrast::contrast_ratio(palette.accent_border, palette.accent_fill);
    let style = list_tile::resolve_style(true, false, BaseState::Rest, &palette);

    assert!(
        ratio < crate::token::contrast::MIN_CHROME_CONTRAST,
        "a perimeter stroke would be visible: {ratio}"
    );
    assert_eq!(style.border_width, 0.0);
    assert_eq!(
        number(&by_id("list-tile/single-selected"), "geometry.hit.height"),
        list_tile::SINGLE_LINE_HEIGHT
    );
}

#[test]
fn a_non_interactive_tile_paints_no_row_fill() {
    let cell = by_id("list-tile/non-interactive");
    assert!(
        cell.frame
            .quads
            .iter()
            .all(|q| q.layer != "row" && q.layer != "card"),
        "an inert tile painted a row fill: {:?}",
        cell.frame
            .quads
            .iter()
            .map(|q| &q.layer)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        text_of(&cell, "interactive"),
        "false",
        "the cell must say it is inert"
    );
}

#[test]
fn a_selected_tile_paints_its_accent_fill() {
    let palette = Palette::dark();
    let cell = by_id("list-tile/two-line-selected");
    let row = quad(&cell, "row");

    assert_eq!(row.fill, "accent.fill");
    assert_eq!(colour_token(&cell, "background"), "accent.fill");

    let subtitle = cell
        .audit
        .iter()
        .find(|pair| pair.channel == "subtitle")
        .expect("the selected tile audits its subtitle");
    assert_eq!(
        subtitle.background, palette.accent_fill,
        "the subtitle must be measured against the fill it is actually painted on"
    );
}

#[test]
fn the_hairline_layers_are_one_pixel_thick() {
    for cell in composite_cells() {
        for q in &cell.frame.quads {
            if q.border != "none" && q.border_width > 0.0 {
                assert!(
                    (q.border_width - shape::BORDER_HAIRLINE).abs() < EPS
                        || (q.border_width - shape::BORDER_FOCUS).abs() < EPS,
                    "{} {} strokes at {}px",
                    cell.id,
                    q.layer,
                    q.border_width
                );
            }
        }
    }
}

#[test]
fn no_composite_cell_paints_an_unmapped_colour() {
    for cell in composite_cells() {
        for q in &cell.frame.quads {
            for value in [&q.fill, &q.border] {
                assert!(
                    !value.starts_with("UNMAPPED"),
                    "{} {} painted an unmapped colour: {value}",
                    cell.id,
                    q.layer
                );
            }
        }
        for entry in &cell.resolved {
            if let StyleValue::Colour { token, .. } = &entry.value {
                assert!(
                    !token.starts_with("UNMAPPED"),
                    "{} {} resolved an unmapped colour: {token}",
                    cell.id,
                    entry.channel
                );
            }
        }
    }
}
