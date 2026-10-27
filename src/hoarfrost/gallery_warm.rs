//! The `time.delay.tooltip_warm` handoff, driven through the real session and
//! the real hover controller (`UIF-TOK-007`, `UIF-ATOM-007`).
//!
//! The tooltip cells in `gallery` already show *when one control's* tooltip
//! paints. These three show the thing a per-control view cannot: the same
//! arriving control, at the same kind of instant, charged a different wait
//! because of what the **user** did a moment before. That difference is the
//! whole content of `time.delay.tooltip_warm`, and it only exists across
//! controls — so each cell drives one [`WarmSession`] and two real
//! [`HoverController`]s, and the numbers in the manifest are read back out of
//! them rather than written down beside them.
//!
//! Every cell is painted at the **arrival instant itself**, one frame before any
//! wait could have elapsed. That is the honest probe point: a cell that showed
//! the state 400 ms later would look identical whichever verdict applied.

use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, paint_frame, surface_layer,
    viewport,
};
use crate::gallery_tooltip::{TOOLTIP_CARD_SIZE, gallery_navbar, tooltip_card_layers};
use crate::surface::tooltip::{
    ArrivalWarmth, HoverController, TimeMs, WarmSession, cold_wait_ms, handoff_window_ms,
    warm_wait_ms,
};
use crate::token::color::Palette;
use crate::token::{shape, space};
use iced::{Point, Rectangle, Size};

/// The instant the scripts are anchored at, in host-clock milliseconds.
const T0: u64 = 10_000;

/// A tooltip card wide enough to read as a label under the arriving entry.
const CARD_SIZE: Size = TOOLTIP_CARD_SIZE;

/// The second navbar entry, sitting immediately after the one
/// [`gallery_navbar`] returns — the icon the pointer steps to.
fn next_entry(entry: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(entry.x + entry.width + space::XS, entry.y),
        entry.size(),
    )
}

/// Build one verdict cell by driving a real session and a real controller.
///
/// `warm_from` is the instant of the last visible tooltip, or `None` for a
/// session that has never shown one. The arriving control is entered through
/// [`HoverController::pointer_enter_warm`] with the session's own verdict, and
/// the cell is probed at the arrival instant.
fn handoff_cell(
    id: &'static str,
    note: &'static str,
    warm_from: Option<u64>,
    arrival: u64,
) -> Cell {
    let palette = Palette::dark();
    let style = crate::surface::tooltip::TooltipStyle::resolve(&palette);

    let mut session = WarmSession::new();
    if let Some(visible_at) = warm_from {
        session.note_visible(TimeMs::millis(visible_at));
    }

    let now = TimeMs::millis(arrival);
    let verdict = session.warmth_for(now);
    let mut controller = HoverController::new();
    controller.pointer_enter_warm(now, verdict);
    let visibility = controller.tick(now);

    let (band, first_entry) = gallery_navbar(viewport().width);
    let arriving = next_entry(first_entry);
    let placement =
        crate::surface::tooltip::TooltipAnchor::band(band, arriving).resolve(CARD_SIZE, viewport());

    let mut layers = vec![(
        "anchor-bar",
        surface_layer(
            band,
            palette.surface_raised,
            palette.border_subtle,
            shape::BORDER_HAIRLINE,
            0.0,
        ),
    )];
    layers.push((
        "arriving-entry",
        surface_layer(
            arriving,
            palette.surface_raised,
            palette.border_strong,
            shape::BORDER_HAIRLINE,
            shape::RADIUS_WIDGET,
        ),
    ));
    if visibility.is_visible() {
        layers.extend(tooltip_card_layers(&style, placement.rect(), None));
    }

    let gap_ms = warm_from.map_or(0, |visible_at| arrival.saturating_sub(visible_at));

    Cell {
        id: id.to_string(),
        kind: CellKind::Composite,
        subject: "tooltip",
        spec: "UIF-TOK-007 + UIF-ATOM-007",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::text(
                "session.memory",
                if session.has_memory() { "yes" } else { "no" },
            ),
            StyleEntry::number("warm-window-ms", handoff_window_ms() as f32),
            StyleEntry::number("gap-since-visible-ms", gap_ms as f32),
            StyleEntry::text(
                "verdict",
                match verdict {
                    ArrivalWarmth::Cold => "cold",
                    ArrivalWarmth::Warm => "warm",
                },
            ),
            StyleEntry::number("wait-ms", controller.active_delay().as_millis() as f32),
            StyleEntry::number("cold-wait-ms", cold_wait_ms() as f32),
            StyleEntry::number("warm-wait-ms", warm_wait_ms() as f32),
            StyleEntry::text(
                "paints-at-arrival",
                if visibility.is_visible() { "yes" } else { "no" },
            ),
            StyleEntry::text("visibility", format!("{visibility:?}")),
            StyleEntry::number("card.radius", style.radius),
            StyleEntry::number("card.max_width", style.max_width),
        ],
        frame: paint_frame(Namer::Standard, viewport(), &layers),
        audit: vec![
            AuditPair {
                channel: "tooltip.text-on-card",
                foreground: style.foreground,
                background: style.background,
                role: ContrastRole::Text,
            },
            AuditPair {
                channel: "tooltip.border-on-card",
                foreground: style.border_color,
                background: style.background,
                role: ContrastRole::Chrome,
            },
        ],
        note: Some(note.to_string()),
    }
}

/// The three verdicts the warm-handoff rule produces, each at the arrival
/// instant.
#[must_use]
pub fn cells() -> Vec<Cell> {
    vec![
        handoff_cell(
            "tooltip/warm-cold-first-hover",
            "No tooltip has been shown in this group, so the session has no \
             memory and the arrival is cold: the full `time.delay.tooltip_open` \
             of 400 ms must elapse before the card paints. Probed at the \
             arrival frame, so nothing paints here — that is the point.",
            None,
            T0,
        ),
        handoff_cell(
            "tooltip/warm-handoff",
            "A tooltip was visible 40 ms ago on the neighbouring icon, so this \
             arrival is inside the `time.delay.tooltip_warm` window and is \
             handed off at `0 ms`: the card paints on the arrival frame \
             itself. The wait was removed by what the user already saw, not by \
             reconfiguring the control.",
            Some(T0),
            T0 + 40,
        ),
        handoff_cell(
            "tooltip/warm-cooled",
            "The last visible tooltip was 101 ms ago — one millisecond past the \
             100 ms window — so the session cooled on its own, with no event \
             and no timer to cancel. The arrival pays the full \
             `time.delay.tooltip_open` again and nothing paints at the \
             arrival frame.",
            Some(T0),
            T0 + handoff_window_ms() + 1,
        ),
    ]
}

#[cfg(test)]
#[path = "gallery_warm_tests.rs"]
mod gallery_warm_tests;
