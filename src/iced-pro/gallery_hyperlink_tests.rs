//! Tests for the hyperlink gallery cells — `UIF-ATOM-001d`, §3.1d.
//!
//! Four groups, in order:
//!
//! 1. **The shape of the block.** Ten cells, unique ids, the fixed
//!    kind/subject/spec triple, and both mounts under every column.
//! 2. **The gates.** No emitted colour is unmapped, and every audit pair
//!    the cells declare actually passes its threshold.
//! 3. **What each shape records.** The label is judged against whatever
//!    it actually sits on, the ring is the control's own, and a disabled
//!    link drops its underline.
//! 4. **Byte stability.** Two calls produce identical snapshots.

use super::*;
use crate::gallery::{Cell, StyleValue, judge, unmapped_colours};
use crate::token::contrast::contrast_ratio;

/// The expected cell count: the five §3.3 columns x the two mounts
/// §3.1d names.
const EXPECTED_CELLS: usize = 10;

/// Look up a resolved channel on a cell.
fn value<'a>(cell: &'a Cell, channel: &str) -> Option<&'a StyleValue> {
    cell.resolved
        .iter()
        .find(|entry| entry.channel == channel)
        .map(|entry| &entry.value)
}

/// The text a resolved channel carries, if it is a text entry.
fn text<'a>(cell: &'a Cell, channel: &str) -> Option<&'a str> {
    match value(cell, channel) {
        Some(StyleValue::Text(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// The number a resolved channel carries, if it is a numeric entry.
fn number(cell: &Cell, channel: &str) -> Option<f32> {
    match value(cell, channel) {
        Some(StyleValue::Number(n)) => Some(*n),
        _ => None,
    }
}

/// The colour a resolved channel carries, if it is a colour entry.
fn colour(cell: &Cell, channel: &str) -> Option<Color> {
    value(cell, channel).and_then(StyleValue::colour)
}

/// Does this cell declare an audit pair on `channel`?
fn has_channel(cell: &Cell, channel: &str) -> bool {
    cell.audit.iter().any(|pair| pair.channel == channel)
}

/// The audit pair on `channel`.
fn pair<'a>(cell: &'a Cell, channel: &str) -> Option<&'a crate::gallery::AuditPair> {
    cell.audit.iter().find(|p| p.channel == channel)
}

/// Is this the disabled column?
fn is_disabled(cell: &Cell) -> bool {
    cell.id.contains("disabled")
}

/// Is this the focused column?
fn is_focused(cell: &Cell) -> bool {
    cell.id.contains("+focused")
}

#[test]
fn the_hyperlink_block_is_ten_cells() {
    assert_eq!(hyperlink_cells().len(), EXPECTED_CELLS);
}

#[test]
fn every_cell_answers_to_the_hyperlink_requirement() {
    for cell in hyperlink_cells() {
        assert_eq!(cell.spec, "UIF-ATOM-001d", "{}", cell.id);
        assert_eq!(cell.subject, "hyperlink", "{}", cell.id);
        assert_eq!(cell.kind, CellKind::Atom, "{}", cell.id);
    }
}

#[test]
fn the_cell_ids_are_unique() {
    let cells = hyperlink_cells();
    let mut ids: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique, "duplicate hyperlink cell ids: {ids:?}");
}

#[test]
fn every_state_is_shown_against_both_mounts() {
    let cells = hyperlink_cells();
    for state in COLUMNS {
        let slug = state_slug(state);
        let for_state: Vec<&Cell> = cells
            .iter()
            .filter(|cell| cell.id.ends_with(&slug))
            .collect();
        assert_eq!(
            for_state.len(),
            2,
            "every mount must be shown for the {slug} column"
        );
        let mut mounts: Vec<&str> = for_state.iter().map(|c| c.id.as_str()).collect();
        mounts.sort_unstable();
        assert!(mounts.windows(2).all(|w| w[0] != w[1]));
    }
}

#[test]
fn no_colour_is_unmapped() {
    let unmapped = unmapped_colours(&hyperlink_cells());
    assert!(
        unmapped.is_empty(),
        "unmapped hyperlink colours: {unmapped:?}"
    );
}

#[test]
fn every_audit_pair_passes_its_threshold() {
    let failing: Vec<String> = hyperlink_cells()
        .iter()
        .flat_map(|cell| cell.audit.iter().map(move |pair| (cell, pair)))
        .filter(|(_, pair)| !judge(pair))
        .map(|(cell, pair)| format!("{} {}", cell.id, pair.channel))
        .collect();
    assert!(failing.is_empty(), "pairs failing the gate: {failing:?}");
}

#[test]
fn every_cell_judges_its_label() {
    for cell in hyperlink_cells() {
        assert!(
            has_channel(&cell, "link.text"),
            "{}: a link must audit its label",
            cell.id
        );
    }
}

#[test]
fn the_label_is_judged_against_whatever_it_actually_sits_on() {
    for cell in hyperlink_cells() {
        let audited = pair(&cell, "link.text").expect("a link.text pair");
        let fill = colour(&cell, "fill").expect("every cell records its fill");
        let mount = match text(&cell, "mount") {
            Some("base") => Palette::dark().surface_base,
            Some("overlay") => Palette::dark().surface_overlay,
            other => panic!("{}: unknown mount {other:?}", cell.id),
        };
        let expected_backing = if text(&cell, "transparent-fill") == Some("true") {
            mount
        } else {
            fill
        };
        assert_eq!(
            audited.background, expected_backing,
            "{}: a transparent fill must fall back to the mount so the \
             exemption cannot silently drop the label from the judged set",
            cell.id
        );
    }
}

#[test]
fn a_disabled_link_is_not_underlined() {
    let cells = hyperlink_cells();
    for cell in cells.iter().filter(|c| is_disabled(c)) {
        assert_eq!(
            text(cell, "underlined"),
            Some("false"),
            "{}: §3.1d removes the underline when the link is dead",
            cell.id
        );
    }
}

#[test]
fn every_live_link_is_underlined() {
    for cell in hyperlink_cells() {
        if is_disabled(&cell) {
            continue;
        }
        assert_eq!(
            text(&cell, "underlined"),
            Some("true"),
            "{}: a live link carries its underline",
            cell.id
        );
    }
}

#[test]
fn the_focus_ring_is_present_only_on_the_focused_column() {
    for cell in hyperlink_cells() {
        let width = number(&cell, "focus-ring.width").unwrap_or(0.0);
        if is_focused(&cell) {
            assert!(width > 0.0, "{}: a focused link carries a ring", cell.id);
            assert!(
                has_channel(&cell, "link.focus-ring"),
                "{}: a painted ring must be audited",
                cell.id
            );
        } else {
            assert_eq!(width, 0.0, "{}: an unfocused link carries no ring", cell.id);
            assert!(
                !has_channel(&cell, "link.focus-ring"),
                "{}: an unpainted ring must not be audited",
                cell.id
            );
        }
    }
}

#[test]
fn the_focus_ring_is_audited_against_the_mount_not_the_pill() {
    let palette = Palette::dark();
    for cell in hyperlink_cells() {
        let Some(audited) = pair(&cell, "link.focus-ring") else {
            continue;
        };
        let mount = match text(&cell, "mount") {
            Some("base") => palette.surface_base,
            Some("overlay") => palette.surface_overlay,
            other => panic!("{}: unknown mount {other:?}", cell.id),
        };
        assert_eq!(
            audited.background, mount,
            "{}: the ring is drawn over the mount, because the control \
             paints nothing at rest",
            cell.id
        );
    }
}

#[test]
fn the_resting_label_ratios_match_the_ones_the_spec_prints() {
    let palette = Palette::dark();
    let cells = hyperlink_cells();
    for (mount_name, mount, printed) in [
        ("base", palette.surface_base, 10.66_f32),
        ("overlay", palette.surface_overlay, 9.49_f32),
    ] {
        let cell = cells
            .iter()
            .find(|c| c.id.ends_with(&format!("{mount_name}-rest")))
            .unwrap_or_else(|| panic!("no resting {mount_name} cell"));
        let measured = contrast_ratio(palette.accent_text, mount);
        assert!(
            (measured - printed).abs() < 0.005,
            "{mount_name}: recomputed {measured:.4} vs the spec's {printed}"
        );
        assert_eq!(
            colour(cell, "text"),
            Some(palette.accent_text),
            "{mount_name}: the resting label must be accent.text"
        );
    }
}

#[test]
fn the_disabled_label_ratios_match_the_ones_the_spec_prints() {
    let palette = Palette::dark();
    let cells = hyperlink_cells();
    for (mount_name, mount, printed) in [
        ("base", palette.surface_base, 5.92_f32),
        ("overlay", palette.surface_overlay, 5.27_f32),
    ] {
        let cell = cells
            .iter()
            .find(|c| c.id.ends_with(&format!("{mount_name}-disabled")))
            .unwrap_or_else(|| panic!("no disabled {mount_name} cell"));
        let measured = contrast_ratio(palette.text_disabled, mount);
        assert!(
            (measured - printed).abs() < 0.005,
            "{mount_name}: recomputed {measured:.4} vs the spec's {printed}"
        );
        assert_eq!(
            colour(cell, "text"),
            Some(palette.text_disabled),
            "{mount_name}: the disabled label must be text.disabled"
        );
    }
}

#[test]
fn the_hover_pill_is_painted_only_where_the_spec_paints_one() {
    let cells = hyperlink_cells();
    let painted: Vec<&str> = cells
        .iter()
        .filter(|c| text(c, "transparent-fill") == Some("false"))
        .map(|c| {
            // The state slug is the tail of the id.
            c.id.rsplit('-').next().unwrap_or_default()
        })
        .collect();
    assert!(
        !painted.is_empty(),
        "at least one column must paint the hover/press pill"
    );
    for slug in &painted {
        assert!(
            matches!(*slug, "hovered" | "pressed"),
            "only hover and press paint a pill, found {slug}"
        );
    }
}

#[test]
fn the_hit_box_never_collapses_below_the_spec_minimum() {
    for cell in hyperlink_cells() {
        let height = number(&cell, "hit.height").expect("every cell records its hit height");
        assert!(
            height >= hyperlink::MIN_HEIGHT,
            "{}: the hit box must never fall below the {}px floor",
            cell.id,
            hyperlink::MIN_HEIGHT
        );
    }
}

#[test]
fn two_calls_produce_identical_snapshots() {
    let a = hyperlink_cells();
    let b = hyperlink_cells();
    assert_eq!(
        a.iter().map(|cell| cell.snapshot()).collect::<Vec<_>>(),
        b.iter().map(|cell| cell.snapshot()).collect::<Vec<_>>()
    );
}
