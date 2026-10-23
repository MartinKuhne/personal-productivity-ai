//! The composite cells of the `UIF-VER-004` matrix (§3B,
//! `UIF-COMP-001`, `UIF-COMP-005`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit. These cells are built through `gallery`'s own
//! vocabulary and [`gallery::matrix`] calls the producers back, so the
//! matrix is still one list in one fixed order — the split moved the code,
//! not the artifact.
//!
//! # What a composite cell proves that an atom cell cannot
//!
//! An atom cell asks "what colour is this control in this state". A
//! composite cell asks "where is the band, and does it move". That is why
//! every cell here records its geometry channels alongside its colours, and
//! why the form field's three caption postures are emitted as three cells
//! whose `geometry.*` entries are byte-identical: the zero-shift
//! requirement is checked by diffing the cells against each other rather
//! than by trusting that a layout happened to hold.
//!
//! # The frame paints the reservation, not a picture of one
//!
//! A composite owns layout, and mostly owns no fill of its own — a form
//! field's control brings its own chrome, and a caption is text rather
//! than a quad. So the frame draws each reserved band as a
//! transparent-filled, hairline-stroked marker layer. Every quad in these
//! frames is a rectangle the resolver actually computed and a stroke in a
//! colour the resolver actually resolved; nothing is painted to make the
//! picture look complete. Where a composite *does* own a fill — the card's
//! surface, the settings row's card, a list tile's selected background —
//! the real layer builder's output is what goes in.
//!
//! # The audit mount is stated, not assumed
//!
//! A caption on a transparent band has no contrast against "nothing", so
//! each cell names the surface it is mounted on and measures against that:
//! `surface.raised`, the lightest of the three surfaces §4 mounts these
//! composites on and therefore the one that gives the *lowest* ratio for a
//! light foreground. A selected tile is measured against its own
//! `accent.fill`, which is where the subtitle promotion is caught.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix
//!
//! Unit tests live in the sibling `gallery_composite_tests.rs` sidecar.

use crate::composite::card::{self, CardElevation};
use crate::composite::form_field::{self, CaptionKind};
use crate::composite::list_tile::{self, Leading, TileLines};
use crate::composite::settings_row::{self, DEFAULT_CONTROL_HEIGHT};
use crate::gallery::{AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, paint_frame};
use crate::state::BaseState;
use crate::token::color::Palette;
use crate::token::{shape, size, space};
use crate::widget::button_widget::Layer;
use iced::{Color, Point, Rectangle};

/// A sample width for the composite cells, composed from tokens so the
/// harness carries no bare visual magnitude.
const SAMPLE_WIDTH: f32 = space::XL * 12.0;

/// A sample height for the card cell.
const CARD_SAMPLE_HEIGHT: f32 = space::XL * 6.0 + space::LG;

/// The surface every composite here is mounted on.
///
/// Stated because it decides every text audit below: `surface.raised` is
/// the lightest of the surfaces §4 mounts forms, settings rows and lists
/// on, so it is the conservative choice for a light foreground.
const MOUNT: &str = "surface.raised";

/// A transparent, hairline-stroked marker for a band the resolver
/// reserved but does not paint.
fn marker(bounds: Rectangle, colour: Color) -> Layer {
    Layer {
        bounds,
        fill: Color::TRANSPARENT,
        border: iced::Border {
            color: colour,
            width: shape::BORDER_HAIRLINE,
            radius: 0.0_f32.into(),
        },
    }
}

fn sample(height: f32) -> Rectangle {
    Rectangle::new(Point::ORIGIN, iced::Size::new(SAMPLE_WIDTH, height))
}

// ---------------------------------------------------------------------------
// FormField — UIF-COMP-001
// ---------------------------------------------------------------------------

/// The stacked form field in one caption posture (`UIF-COMP-001`).
///
/// The three caption postures are three cells, and their `geometry.*`
/// channels are the same three numbers in all three. That equality is the
/// zero-shift evidence: it holds because
/// [`form_field::bands`] cannot see the caption, not because a layout
/// happened to come out level.
#[must_use]
pub fn form_field_cell(state: &'static str) -> Cell {
    let palette = Palette::dark();
    let bounds = sample(form_field::stacked_height());
    let bands = form_field::bands(bounds);
    let mount = palette.surface_raised;

    let caption = match state {
        "error" => {
            form_field::resolve_caption(false, Some("helper"), Some("Name is taken"), &palette)
        }
        "helper" => form_field::resolve_caption(true, Some("Shown at the top"), None, &palette),
        _ => form_field::resolve_caption(true, None, None, &palette),
    };
    let header = form_field::resolve_header("Display name", true, None, &palette);

    let layers: Vec<(&'static str, Layer)> = vec![
        ("band.header", marker(bands.header, palette.border_subtle)),
        ("band.control", marker(bands.control, palette.border_strong)),
        ("band.caption", marker(bands.caption, caption.colour)),
    ];

    Cell {
        id: format!("form-field/{state}"),
        kind: CellKind::Composite,
        subject: "form-field",
        spec: "UIF-COMP-001",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::number("geometry.total-height", form_field::stacked_height()),
            StyleEntry::number("geometry.header.height", bands.header.height),
            StyleEntry::number("geometry.control.height", bands.control.height),
            StyleEntry::number("geometry.caption.height", bands.caption.height),
            StyleEntry::number("geometry.gap", form_field::BAND_GAP),
            StyleEntry::text("caption.kind", caption_kind_slug(caption.kind)),
            StyleEntry::colour("caption.colour", caption.colour),
            StyleEntry::text(
                "caption.glyph",
                caption.glyph.map_or("none", |_| "warning").to_string(),
            ),
            StyleEntry::colour("header.label", header.label_colour),
            StyleEntry::colour("header.marker", header.marker_colour),
            StyleEntry::text("overflow.policy", "clip-single-line"),
            StyleEntry::text("mount", MOUNT),
        ],
        frame: paint_frame(Namer::Standard, bounds, &layers),
        audit: vec![
            AuditPair {
                channel: "header.label",
                foreground: header.label_colour,
                background: mount,
                role: ContrastRole::Text,
            },
            AuditPair {
                channel: "header.marker",
                foreground: header.marker_colour,
                background: mount,
                role: ContrastRole::Text,
            },
            AuditPair {
                channel: "caption",
                foreground: caption.colour,
                background: mount,
                role: ContrastRole::Text,
            },
        ],
        note: None,
    }
}

fn caption_kind_slug(kind: CaptionKind) -> &'static str {
    match kind {
        CaptionKind::Empty => "empty",
        CaptionKind::Helper => "helper",
        CaptionKind::Error => "error",
    }
}

/// The same control slot hosting three different atom kinds
/// (`UIF-COMP-001`).
///
/// The slot is one rectangle whatever goes into it, and the three atoms'
/// own natural heights differ — `32`, `28` and `20` — so the cell records
/// that the field did not resize to fit any of them.
#[must_use]
pub fn form_field_host_cell(kind: &'static str) -> Cell {
    let palette = Palette::dark();
    let bounds = sample(form_field::stacked_height());
    let bands = form_field::bands(bounds);
    let mount = palette.surface_raised;

    let (hosted, hosted_height) = match kind {
        "input" => ("text-input", size::CONTROL_SM),
        "row" => ("focusable-row", size::HIT_TARGET_MIN),
        _ => ("button", size::CONTROL_MD),
    };

    let layers: Vec<(&'static str, Layer)> = vec![
        ("band.header", marker(bands.header, palette.border_subtle)),
        ("band.control", marker(bands.control, palette.border_strong)),
        ("band.caption", marker(bands.caption, palette.border_subtle)),
    ];

    Cell {
        id: format!("form-field/host/{kind}"),
        kind: CellKind::Composite,
        subject: "form-field",
        spec: "UIF-COMP-001",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::text("host.kind", hosted.to_string()),
            StyleEntry::number("host.natural-height", hosted_height),
            StyleEntry::number("geometry.control.height", bands.control.height),
            StyleEntry::number("geometry.total-height", form_field::stacked_height()),
            StyleEntry::text("host.slot-depends-on-control", "no"),
            StyleEntry::text("mount", MOUNT),
        ],
        frame: paint_frame(Namer::Standard, bounds, &layers),
        audit: vec![AuditPair {
            channel: "slot.border",
            foreground: palette.border_strong,
            background: mount,
            role: ContrastRole::Chrome,
        }],
        note: None,
    }
}

/// Every form-field cell: the three caption postures, then the three hosts.
#[must_use]
pub fn form_field_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for state in ["empty", "helper", "error"] {
        cells.push(form_field_cell(state));
    }
    for kind in ["button", "input", "row"] {
        cells.push(form_field_host_cell(kind));
    }
    cells
}

// ---------------------------------------------------------------------------
// SettingsRow — UIF-COMP-001
// ---------------------------------------------------------------------------

/// The horizontal settings row in one content shape (`UIF-COMP-001`).
#[must_use]
pub fn settings_row_cell(state: &'static str) -> Cell {
    let palette = Palette::dark();
    let disabled = state == "disabled";
    let has_description = state == "with-description";
    let height = settings_row::row_height(has_description, DEFAULT_CONTROL_HEIGHT);
    let bounds = sample(height);
    let split = settings_row::bands(bounds, size::CONTROL_LG);
    let style = settings_row::resolve_style(disabled, &palette);

    let layers: Vec<(&'static str, Layer)> = vec![
        ("card", settings_row::card_layer(bounds, &style)),
        ("slot.control", marker(split.control, palette.border_strong)),
    ];

    Cell {
        id: format!("settings-row/{state}"),
        kind: CellKind::Composite,
        subject: "settings-row",
        spec: "UIF-COMP-001",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::number("geometry.height", height),
            StyleEntry::number("geometry.min-height", settings_row::MIN_HEIGHT),
            StyleEntry::number("geometry.padding.vertical", settings_row::PADDING_VERTICAL),
            StyleEntry::number(
                "geometry.padding.horizontal",
                settings_row::PADDING_HORIZONTAL,
            ),
            StyleEntry::number("geometry.column-gap", settings_row::COLUMN_GAP),
            StyleEntry::number("geometry.text.width", split.text.width),
            StyleEntry::number("geometry.control.width", split.control.width),
            StyleEntry::colour("background", style.background),
            StyleEntry::colour("border.color", style.border_color),
            StyleEntry::number("border.width", style.border_width),
            StyleEntry::number("radius", style.radius),
            StyleEntry::colour("title", style.title_colour),
            StyleEntry::colour("description", style.description_colour),
            StyleEntry::text("mount", MOUNT),
        ],
        frame: paint_frame(Namer::Standard, bounds, &layers),
        audit: vec![
            AuditPair {
                channel: "title",
                foreground: style.title_colour,
                background: style.background,
                role: ContrastRole::Text,
            },
            AuditPair {
                channel: "description",
                foreground: style.description_colour,
                background: style.background,
                role: ContrastRole::Text,
            },
            AuditPair {
                channel: "border",
                foreground: style.border_color,
                background: style.background,
                role: ContrastRole::Chrome,
            },
        ],
        note: None,
    }
}

/// Every settings-row cell.
#[must_use]
pub fn settings_row_cells() -> Vec<Cell> {
    ["title-only", "with-description", "disabled"]
        .iter()
        .map(|state| settings_row_cell(state))
        .collect()
}

// ---------------------------------------------------------------------------
// Card — UIF-COMP-005
// ---------------------------------------------------------------------------

/// The card container at one elevation (`UIF-COMP-005`).
#[must_use]
pub fn card_cell(elevation: CardElevation) -> Cell {
    let palette = Palette::dark();
    let style = card::resolve_style(elevation, &palette);
    let bounds = Rectangle::new(
        Point::ORIGIN,
        iced::Size::new(SAMPLE_WIDTH + space::LG, CARD_SAMPLE_HEIGHT),
    );
    let content = card::content_bounds(bounds, &style);

    let layers: Vec<(&'static str, Layer)> = vec![
        ("card", card::card_layer(bounds, &style)),
        ("content", marker(content, palette.border_subtle)),
    ];

    Cell {
        id: format!("card/{}", elevation_slug(elevation)),
        kind: CellKind::Composite,
        subject: "card",
        spec: "UIF-COMP-005",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::colour("background", style.background),
            StyleEntry::colour("border.color", style.border_color),
            StyleEntry::number("border.width", style.border_width),
            StyleEntry::number("radius", style.radius),
            StyleEntry::number("padding", style.padding),
            StyleEntry::number("content.width", content.width),
            StyleEntry::number("content.height", content.height),
            StyleEntry::text(
                "border.role",
                if elevation.is_active_indicator() {
                    "active-indicator"
                } else {
                    "resting-separation"
                }
                .to_string(),
            ),
        ],
        frame: paint_frame(Namer::Standard, bounds, &layers),
        audit: vec![AuditPair {
            channel: "border",
            foreground: style.border_color,
            background: style.background,
            role: ContrastRole::Chrome,
        }],
        note: None,
    }
}

fn elevation_slug(elevation: CardElevation) -> &'static str {
    match elevation {
        CardElevation::Rest => "rest",
        CardElevation::Elevated => "elevated",
    }
}

/// Every card cell.
#[must_use]
pub fn card_cells() -> Vec<Cell> {
    [CardElevation::Rest, CardElevation::Elevated]
        .iter()
        .map(|elevation| card_cell(*elevation))
        .collect()
}

// ---------------------------------------------------------------------------
// ListTile — UIF-COMP-005
// ---------------------------------------------------------------------------

/// One list-tile cell: a rung, a selection posture and a pointer state
/// (`UIF-COMP-005`, decision **D-8**).
#[must_use]
pub fn list_tile_cell(state: &'static str) -> Cell {
    let palette = Palette::dark();
    let lines = if state.contains("two-line") {
        TileLines::Two
    } else {
        TileLines::Single
    };
    let selected = state.contains("selected");
    let disabled = state == "disabled";
    let interactive = state != "non-interactive";
    let base = match state {
        "hovered" => BaseState::Hovered,
        "pressed" => BaseState::Pressed,
        _ => BaseState::Rest,
    };

    let height = list_tile::tile_height(lines);
    let bounds = sample(height);
    let trailing_width = list_tile::TRAILING_TARGET * 2.0 + list_tile::SLOT_GAP;
    let grid = list_tile::slots(bounds, Leading::Checkbox, interactive, trailing_width);
    let targets = list_tile::trailing_targets(bounds, 2);
    let hit = list_tile::hit_target(bounds, interactive);
    let style = list_tile::resolve_style(selected, disabled, base, &palette);

    let audit_background = if style.background.a == 0.0 {
        palette.surface_raised
    } else {
        style.background
    };

    let mut layers: Vec<(&'static str, Layer)> = Vec::new();
    if let Some(row) = list_tile::row_layer(bounds, &style) {
        layers.push(("row", row));
    }
    layers.push(("slot.leading", marker(grid.leading, palette.border_strong)));
    layers.push(("slot.center", marker(grid.center, palette.border_subtle)));
    layers.push((
        "slot.trailing",
        marker(grid.trailing, palette.border_subtle),
    ));
    for (index, target) in targets.iter().enumerate() {
        let _ = index;
        layers.push((
            "slot.trailing-target",
            marker(*target, palette.border_strong),
        ));
    }

    Cell {
        id: format!("list-tile/{state}"),
        kind: CellKind::Composite,
        subject: "list-tile",
        spec: "UIF-COMP-005",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::number("geometry.height", height),
            StyleEntry::number("geometry.padding.vertical", list_tile::PADDING_VERTICAL),
            StyleEntry::number("geometry.padding.horizontal", list_tile::PADDING_HORIZONTAL),
            StyleEntry::number("geometry.slot.gap", list_tile::SLOT_GAP),
            StyleEntry::number("geometry.slot.leading.width", grid.leading.width),
            StyleEntry::number("geometry.slot.center.width", grid.center.width),
            StyleEntry::number("geometry.slot.trailing.width", grid.trailing.width),
            StyleEntry::number("geometry.trailing.target", list_tile::TRAILING_TARGET),
            StyleEntry::number("geometry.hit.height", hit.map_or(0.0, |rect| rect.height)),
            StyleEntry::text("interactive", interactive.to_string()),
            StyleEntry::colour("background", style.background),
            StyleEntry::colour("title", style.title_colour),
            StyleEntry::colour("subtitle", style.subtitle_colour),
            StyleEntry::colour("leading", style.leading_colour),
            StyleEntry::colour("trailing", style.trailing_colour),
            StyleEntry::text("audit.background", token_label(audit_background)),
        ],
        frame: paint_frame(Namer::Standard, bounds, &layers),
        audit: vec![
            AuditPair {
                channel: "title",
                foreground: style.title_colour,
                background: audit_background,
                role: ContrastRole::Text,
            },
            AuditPair {
                channel: "subtitle",
                foreground: style.subtitle_colour,
                background: audit_background,
                role: ContrastRole::Text,
            },
            AuditPair {
                channel: "leading",
                foreground: style.leading_colour,
                background: audit_background,
                role: ContrastRole::Chrome,
            },
        ],
        note: None,
    }
}

fn token_label(colour: Color) -> String {
    Namer::Standard.name(colour)
}

/// Every list-tile cell.
#[must_use]
pub fn list_tile_cells() -> Vec<Cell> {
    [
        "single",
        "two-line",
        "single-selected",
        "two-line-selected",
        "hovered",
        "pressed",
        "disabled",
        "non-interactive",
    ]
    .iter()
    .map(|state| list_tile_cell(state))
    .collect()
}

/// Every `UIF-COMP-001` and `UIF-COMP-005` cell, in spec order.
#[must_use]
pub fn composite_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    cells.extend(form_field_cells());
    cells.extend(settings_row_cells());
    cells.extend(card_cells());
    cells.extend(list_tile_cells());
    cells
}

#[cfg(test)]
#[path = "gallery_composite_tests.rs"]
mod gallery_composite_tests;
