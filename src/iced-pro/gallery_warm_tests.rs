//! Tests for the `time.delay.tooltip_warm` gallery cells (`UIF-TOK-007`).
//!
//! These do not re-test the session — [`warm_tests`](crate::surface::tooltip::warm::warm_tests)
//! does that. What they pin is that the gallery is showing the *real* verdicts:
//! that the manifest numbers come from the rungs, that the three cells really do
//! disagree with each other, and that the whole block is byte-stable across
//! calls.

use crate::gallery::{Cell, CellKind, FrameQuad, Namer};
use crate::surface::tooltip::{HOVER_DELAY, TooltipStyle};
use crate::token::time;

fn display(cell: &Cell, channel: &str) -> String {
    cell.resolved
        .iter()
        .find(|entry| entry.channel == channel)
        .unwrap_or_else(|| panic!("{} has no `{channel}` channel", cell.id))
        .value
        .display()
}

fn verdict(cell: &Cell) -> String {
    display(cell, "verdict")
}

#[test]
fn the_block_emits_three_cells() {
    assert_eq!(super::cells().len(), 3);
}

#[test]
fn the_three_cells_cover_the_three_verdicts() {
    let cells = super::cells();
    let verdicts: Vec<String> = cells.iter().map(verdict).collect();
    assert_eq!(verdicts, vec!["cold", "warm", "cold"]);
    assert_eq!(
        cells[0].id, "tooltip/warm-cold-first-hover",
        "the cold cell must come from a session with no memory"
    );
    assert_eq!(cells[1].id, "tooltip/warm-handoff");
    assert_eq!(cells[2].id, "tooltip/warm-cooled");
}

#[test]
fn only_the_warm_cell_paints_at_the_arrival_frame() {
    let cells = super::cells();
    assert_eq!(display(&cells[0], "paints-at-arrival"), "no");
    assert_eq!(display(&cells[1], "paints-at-arrival"), "yes");
    assert_eq!(display(&cells[2], "paints-at-arrival"), "no");
    assert_eq!(display(&cells[0], "visibility"), "Pending");
    assert_eq!(display(&cells[1], "visibility"), "Shown");
    assert_eq!(display(&cells[2], "visibility"), "Pending");
}

#[test]
fn the_wait_each_cell_reports_is_the_wait_its_verdict_owes() {
    let cells = super::cells();
    assert_eq!(display(&cells[0], "wait-ms"), format!("{HOVER_DELAY}.00"));
    assert_eq!(display(&cells[1], "wait-ms"), "0.00");
    assert_eq!(display(&cells[2], "wait-ms"), format!("{HOVER_DELAY}.00"));
}

#[test]
fn every_cell_reads_its_numbers_from_the_rungs() {
    for cell in super::cells() {
        assert_eq!(
            display(&cell, "warm-window-ms"),
            format!("{:.2}", time::DELAY_TOOLTIP_WARM.ms as f32),
            "{} must read the warm rung, not restate it",
            cell.id
        );
        assert_eq!(
            display(&cell, "cold-wait-ms"),
            format!("{:.2}", time::DELAY_TOOLTIP_OPEN.ms as f32),
            "{}",
            cell.id
        );
        assert_eq!(
            display(&cell, "warm-wait-ms"),
            format!("{:.2}", time::INSTANT.ms as f32),
            "{}",
            cell.id
        );
    }
}

#[test]
fn the_session_memory_channel_separates_the_first_cell_from_the_rest() {
    let cells = super::cells();
    assert_eq!(display(&cells[0], "session.memory"), "no");
    assert_eq!(display(&cells[1], "session.memory"), "yes");
    assert_eq!(display(&cells[2], "session.memory"), "yes");
    assert_eq!(display(&cells[0], "gap-since-visible-ms"), "0.00");
    assert_eq!(display(&cells[1], "gap-since-visible-ms"), "40.00");
    assert_eq!(
        display(&cells[2], "gap-since-visible-ms"),
        format!("{:.2}", (time::DELAY_TOOLTIP_WARM.ms + 1) as f32)
    );
}

#[test]
fn every_cell_is_a_composite_tooltip_cell_in_the_standard_world() {
    for cell in super::cells() {
        assert_eq!(cell.kind, CellKind::Composite);
        assert_eq!(cell.subject, "tooltip");
        assert_eq!(cell.spec, "UIF-TOK-007 + UIF-ATOM-007");
        assert_eq!(cell.namer, Namer::Standard);
        assert!(cell.note.is_some(), "{} carries no note", cell.id);
    }
}

#[test]
fn every_cell_audits_the_card_text_and_edge() {
    for cell in super::cells() {
        let channels: Vec<&str> = cell.audit.iter().map(|pair| pair.channel).collect();
        assert_eq!(
            channels,
            vec!["tooltip.text-on-card", "tooltip.border-on-card"]
        );
    }
}

#[test]
fn only_the_warm_cell_draws_a_card_quad() {
    let cells = super::cells();
    let palette = crate::token::color::Palette::dark();
    let style = TooltipStyle::resolve(&palette);

    let cards: Vec<&FrameQuad> = cells[1]
        .frame
        .quads
        .iter()
        .filter(|quad| quad.layer == "tooltip-card")
        .collect();
    assert_eq!(cards.len(), 1, "the warm cell must draw exactly one card");
    assert_eq!(cards[0].radius, style.radius);
    assert_eq!(cards[0].border_width, style.border_width);

    for cold in [&cells[0], &cells[2]] {
        assert!(
            !cold
                .frame
                .quads
                .iter()
                .any(|quad| quad.layer == "tooltip-card"),
            "{} must not paint a card at the arrival frame",
            cold.id
        );
        assert!(
            cold.frame
                .quads
                .iter()
                .any(|quad| quad.layer == "arriving-entry"),
            "{} must still show the control being arrived at",
            cold.id
        );
    }
}

#[test]
fn two_calls_produce_identical_snapshots() {
    let a: Vec<String> = super::cells().iter().map(Cell::snapshot).collect();
    let b: Vec<String> = super::cells().iter().map(Cell::snapshot).collect();
    assert_eq!(a, b);
}
