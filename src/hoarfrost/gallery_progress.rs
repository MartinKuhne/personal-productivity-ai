//! The progress-indicator cells of the `UIF-VER-004` matrix (§3.9,
//! `UIF-ATOM-009`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit. These cells are built through `gallery`'s own
//! vocabulary and [`gallery::matrix`] calls [`progress_cells`] back, so
//! the matrix is still one list in one fixed order — the split moved the
//! code, not the artifact.
//!
//! # Twenty-two cells
//!
//! * the bar in each of the three statuses at five fill conditions —
//!   a quarter, a half, full, indeterminate in motion, and indeterminate
//!   under injected reduced motion — 15 cells;
//! * the spinner at both rungs, outside a solid fill and inside one —
//!   4 cells;
//! * the trailing stop button in each status — 3 cells.
//!
//! The five fill conditions are the interesting axis. A determinate bar
//! is one resolver question (`fraction`), but the indeterminate half is
//! two: whether the segment is travelling and whether the host has said
//! motion is reduced. Sampling both is the only way the gallery can show
//! that the reduced-motion form is a *different picture* rather than the
//! same picture frozen mid-slide.
//!
//! # The phase is injected, and the cell says so
//!
//! Every indeterminate cell is resolved at a fixed phase of `0.5`. The
//! gallery never reads a clock, so the recorded snapshot is the same on
//! every run — which is the entire reason the no-clock rule exists. The
//! phase is written into `resolved.motion.phase` so a reader can see the
//! cell was sampled at a stated instant rather than at an unstated one.
//!
//! # The track is *not* audited against the mount
//!
//! `surface.active` on `surface.raised` is `1.0314:1`. Auditing the
//! track as chrome would fail every bar cell and would be measuring the
//! wrong thing: §3.9 makes the **fill** the indicator, and the track is
//! the groove it runs in. The one chrome pair a bar cell reports is
//! `bar.fill` against `bar.track`, which is the separation the widget
//! exists to make.
//!
//! # The stop button's colour is a gap, filled by convention
//!
//! §3.9 names a trailing `24 × 24px` `[×]` stop button and gives it no
//! colour. Rather than invent one, the cell takes the same resting glyph
//! colour the text input's trailing slot uses — [`Palette::text_muted`] —
//! which is the framework's existing answer to "an actionable glyph in
//! a trailing slot". Recorded as a spec gap in the cell's note.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix
//!
//! Unit tests live in the sibling `gallery_progress_tests.rs` sidecar.

use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, atom_sample_at, paint_frame,
};
use crate::token::color::Palette;
use crate::token::size;
use crate::token::space;
use crate::widget::button_widget::Layer;
use crate::widget::progress::{self, ProgressStyle, SegmentStyle};
use crate::widget::progress_model::{ProgressModel, ProgressStatus};
use iced::{Border, Point, Rectangle};

/// Every status, in §3.9 order.
const STATUSES: [ProgressStatus; 3] = [
    ProgressStatus::Active,
    ProgressStatus::Paused,
    ProgressStatus::Error,
];

/// The phase every indeterminate cell is sampled at.
const SAMPLE_PHASE: f32 = 0.5;

/// A [`ProgressStatus`] as a cell-id slug.
fn status_slug(status: ProgressStatus) -> &'static str {
    match status {
        ProgressStatus::Active => "active",
        ProgressStatus::Paused => "paused",
        ProgressStatus::Error => "error",
    }
}

/// The bar's own box, centred in the sample.
fn bar_box(sample: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(
            sample.x + space::LG / 2.0,
            sample.y + (sample.height - progress::TRACK_HEIGHT) / 2.0,
        ),
        iced::Size::new(sample.width - space::LG, progress::TRACK_HEIGHT),
    )
}

/// Centre an `n × n` box inside `outer`.
fn centred_square(outer: Rectangle, n: f32) -> Rectangle {
    Rectangle::new(
        Point::new(
            outer.x + (outer.width - n) / 2.0,
            outer.y + (outer.height - n) / 2.0,
        ),
        iced::Size::new(n, n),
    )
}

/// A transparent quad carrying only an outline — how a ring or a hit
/// target is recorded without claiming to have painted a glyph.
fn outline_layer(bounds: Rectangle, colour: iced::Color, width: f32, radius: f32) -> Layer {
    Layer {
        bounds,
        fill: iced::Color::TRANSPARENT,
        border: Border {
            color: colour,
            width,
            radius: radius.into(),
        },
    }
}

/// The resolved style entries a bar cell carries.
///
/// `fraction` is recorded only for a determinate bar. An indeterminate
/// cell that reported `0.0` would be answering a question the model
/// refuses, and a reader could not tell that from a bar that has not
/// started.
fn bar_entries(style: &ProgressStyle, model: &ProgressModel) -> Vec<StyleEntry> {
    let mut entries = vec![
        StyleEntry::colour("track", style.track),
        StyleEntry::colour("fill", style.fill),
        StyleEntry::number("track.height", style.height),
        StyleEntry::number("radius", style.radius),
        StyleEntry::text("status", status_slug(model.status())),
        StyleEntry::text(
            "mode",
            if model.is_indeterminate() {
                "indeterminate"
            } else {
                "determinate"
            },
        ),
    ];
    if let Some(fraction) = model.fraction() {
        entries.push(StyleEntry::number("fraction", fraction));
    }
    entries
}

/// The bar cell for one model and one fill condition.
#[must_use]
pub fn bar_cell(model: &ProgressModel, condition: &str, phase: f32, reduced: bool) -> Cell {
    let palette = Palette::dark();
    let style = progress::resolve_style(model, &palette);
    let sample = atom_sample_at(size::HIT_TARGET_MIN);
    let bounds = bar_box(sample);

    let mut layers: Vec<(&'static str, Layer)> =
        vec![("track", progress::track_layer(bounds, &style))];
    let mut resolved = bar_entries(&style, model);
    resolved.push(StyleEntry::text("condition", condition.to_string()));

    if model.is_indeterminate() {
        let segment = progress::indeterminate_segment(model.status(), phase, reduced, &palette);
        layers.push(("segment", progress::segment_layer(bounds, &segment)));
        push_segment_entries(&mut resolved, &segment, phase, reduced);
    } else if let Some(fill) = progress::fill_layer(bounds, &style, model.fraction().unwrap_or(0.0))
    {
        layers.push(("fill", fill));
    }

    Cell {
        id: format!("progress/bar/{}/{}", status_slug(model.status()), condition),
        kind: CellKind::Atom,
        subject: "progress",
        spec: "UIF-ATOM-009",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit: vec![AuditPair {
            channel: "bar.fill",
            foreground: style.fill,
            background: style.track,
            role: ContrastRole::Chrome,
        }],
        note: None,
    }
}

/// The segment-specific entries an indeterminate bar cell carries.
fn push_segment_entries(
    resolved: &mut Vec<StyleEntry>,
    segment: &SegmentStyle,
    phase: f32,
    reduced: bool,
) {
    resolved.push(StyleEntry::text(
        "motion",
        if reduced { "reduced-static" } else { "running" },
    ));
    resolved.push(StyleEntry::number("motion.phase", phase));
    resolved.push(StyleEntry::number(
        "motion.segment.width-fraction",
        segment.width_fraction,
    ));
    resolved.push(StyleEntry::number(
        "motion.segment.offset-fraction",
        segment.offset_fraction,
    ));
    resolved.push(StyleEntry::text(
        "motion.striped",
        segment.striped.to_string(),
    ));
    resolved.push(StyleEntry::number(
        "motion.stripe-period",
        progress::stripe_period(),
    ));
}

/// The spinner cell for one rung and one mount.
#[must_use]
pub fn spinner_cell(large: bool, inside_solid: bool) -> Cell {
    let palette = Palette::dark();
    let style = progress::spinner_style(large, inside_solid, &palette);
    let sample = atom_sample_at(size::HIT_TARGET_MIN);
    let ring = centred_square(sample, style.diameter);

    let background = if inside_solid {
        palette.accent_fill
    } else {
        palette.surface_overlay
    };

    let layers = vec![
        (
            "track-ring",
            outline_layer(ring, style.track, style.stroke, style.diameter / 2.0),
        ),
        (
            "arc",
            outline_layer(ring, style.arc, style.stroke, style.diameter / 2.0),
        ),
    ];

    Cell {
        id: format!(
            "progress/spinner/{}/{}",
            if large { "md" } else { "sm" },
            if inside_solid { "solid" } else { "plain" }
        ),
        kind: CellKind::Atom,
        subject: "progress",
        spec: "UIF-ATOM-009",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::number("diameter", style.diameter),
            StyleEntry::number("stroke", style.stroke),
            StyleEntry::colour("track", style.track),
            StyleEntry::colour("arc", style.arc),
            StyleEntry::number("arc.sweep-fraction", progress::SPINNER_ARC_FRACTION),
            StyleEntry::text(
                "mount",
                if inside_solid {
                    "solid-fill"
                } else {
                    "overlay"
                },
            ),
        ],
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit: vec![AuditPair {
            channel: "spinner.arc",
            foreground: style.arc,
            background,
            role: ContrastRole::Chrome,
        }],
        note: None,
    }
}

/// The trailing stop-button cell for one status.
#[must_use]
pub fn stop_button_cell(status: ProgressStatus) -> Cell {
    let palette = Palette::dark();
    let style = progress::resolve_style(
        &ProgressModel::determinate(1, 2).with_status(status),
        &palette,
    );
    let sample = atom_sample_at(size::HIT_TARGET_MIN);
    let target = centred_square(sample, progress::STOP_BUTTON_SIZE);

    let layers = vec![
        ("bar-track", progress::track_layer(bar_box(sample), &style)),
        (
            "stop-target",
            outline_layer(target, palette.text_muted, 0.0, 0.0),
        ),
    ];

    Cell {
        id: format!("progress/stop/{}", status_slug(status)),
        kind: CellKind::Atom,
        subject: "progress",
        spec: "UIF-ATOM-009",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::number("stop.size", progress::STOP_BUTTON_SIZE),
            StyleEntry::colour("stop.glyph", palette.text_muted),
            StyleEntry::text("status", status_slug(status)),
            StyleEntry::text("cancellable", "true".to_string()),
        ],
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit: vec![AuditPair {
            channel: "stop.glyph",
            foreground: palette.text_muted,
            background: palette.surface_raised,
            role: ContrastRole::Text,
        }],
        note: Some(
            "§3.9 names the 24x24 [x] stop button but assigns it no colour; the \
             cell uses the text input's resting trailing-glyph convention \
             (text.muted) and records the gap"
                .to_string(),
        ),
    }
}

/// Every progress cell: fifteen bar cells, four spinner cells and three
/// stop-button cells, in that order.
#[must_use]
pub fn progress_cells() -> Vec<Cell> {
    let mut cells = Vec::new();

    for status in STATUSES {
        for completed in [1_u64, 2, 4] {
            let model = ProgressModel::determinate(completed, 4).with_status(status);
            let condition = match completed {
                1 => "quarter",
                2 => "half",
                _ => "full",
            };
            cells.push(bar_cell(&model, condition, 0.0, false));
        }

        let indeterminate = ProgressModel::indeterminate().with_status(status);
        cells.push(bar_cell(
            &indeterminate,
            "indeterminate",
            SAMPLE_PHASE,
            false,
        ));
        cells.push(bar_cell(
            &indeterminate,
            "reduced-static",
            SAMPLE_PHASE,
            true,
        ));
    }

    for large in [false, true] {
        for inside_solid in [false, true] {
            cells.push(spinner_cell(large, inside_solid));
        }
    }

    for status in STATUSES {
        cells.push(stop_button_cell(status));
    }

    cells
}

#[cfg(test)]
#[path = "gallery_progress_tests.rs"]
mod gallery_progress_tests;
