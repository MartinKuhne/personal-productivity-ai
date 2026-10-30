//! The tooltip cells of the `UIF-VER-004` matrix (`UIF-ATOM-001`
//! icon-only cell, `UIF-NAV-003`, `UIF-TAB-003`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit. These cells are built through `gallery`'s own
//! vocabulary and [`gallery::matrix`] calls [`tooltip_cells`] back, so
//! the matrix is still one list in one fixed order — the split moved the
//! code, not the artifact.
//!
//! # What is here
//!
//! The card itself, the state column, and the two placement findings:
//!
//! * [`tooltip_card_cell`] — the card painted with the anchor it hangs
//!   from, so the cell shows the relationship rather than a floating
//!   swatch.
//! * [`tooltip_state_cells`] — the visibility column a
//!   [`HoverController`] produces.
//! * [`tooltip_flip_cell`] and [`tooltip_clamped_cell`] — the anchor
//!   resolution when there is no room above, and when there is none at all.
//!
//! # Shared with `gallery_warm`
//!
//! `TOOLTIP_CARD_SIZE`, `gallery_navbar` and `tooltip_card_layers` are
//! `pub(crate)` rather than private because [`crate::gallery_warm`]
//! builds its warm-handoff cells from the same card and the same navbar.
//! They belong to this subject, so they moved here rather than staying
//! behind in `gallery`.
//!
//! The tests for these cells are in the gallery's own `gallery_tests.rs`.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix

use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, paint_frame, surface_layer,
    viewport,
};
use crate::token::color::Palette;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::button_widget::Layer;
use iced::Point;
use iced::Rectangle;
use iced::Size;

use crate::surface::tooltip::{
    self, ArrivalSource, HoverController, TimeMs, TooltipAnchor, TooltipStyle, Visibility,
};

/// The tooltip card's painted size.
///
/// A stand-in for a measured label: the resolver takes the card size from the host, so the
/// gallery supplies one rather than inventing a text measurer. Width is `space.xl * 7`,
/// height the minimum hit target, which is roughly what a short label occupies at
/// `type.caption`.
pub(crate) const TOOLTIP_CARD_SIZE: Size = Size::new(space::XL * 7.0, size::HIT_TARGET_MIN);

/// The navbar the placement cells are anchored in: a `size.control.lg` bar with a
/// `size.control.sm` entry inset inside it — the exact geometry that makes the
/// bar-versus-button anchor choice matter.
pub(crate) fn gallery_navbar(width: f32) -> (Rectangle, Rectangle) {
    let band = Rectangle::new(Point::new(0.0, 0.0), Size::new(width, size::CONTROL_LG));
    let entry = Rectangle::new(
        Point::new(space::SM, 4.0),
        Size::new(space::XL * 4.0, size::CONTROL_SM),
    );
    (band, entry)
}

/// Paint the tooltip card plus the anchor it hangs from, so a cell shows the *relationship*
/// rather than a floating swatch.
pub(crate) fn tooltip_card_layers(
    style: &TooltipStyle,
    card: Rectangle,
    anchor: Option<Rectangle>,
) -> Vec<(&'static str, Layer)> {
    let mut layers = Vec::new();
    if let Some(anchor) = anchor {
        layers.push((
            "anchor-bar",
            surface_layer(
                anchor,
                Palette::dark().surface_raised,
                Palette::dark().border_subtle,
                shape::BORDER_HAIRLINE,
                0.0,
            ),
        ));
    }
    layers.push((
        "tooltip-card",
        surface_layer(
            card,
            style.background,
            style.border_color,
            style.border_width,
            style.radius,
        ),
    ));
    layers
}

/// The tooltip's resolved paint on its own, with the two contrast pairings that decide
/// whether the card is readable and whether it has a visible edge.
#[must_use]
pub fn tooltip_card_cell() -> Cell {
    let palette = Palette::dark();
    let style = TooltipStyle::resolve(&palette);
    let card = Rectangle::new(Point::new(space::LG, space::LG), TOOLTIP_CARD_SIZE);

    Cell {
        id: "tooltip/card".to_string(),
        kind: CellKind::Composite,
        subject: "tooltip",
        spec: "UIF-ATOM-007 + UIF-NAV-003",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::colour("card.background", style.background),
            StyleEntry::colour("card.text", style.foreground),
            StyleEntry::colour("card.border", style.border_color),
            StyleEntry::number("card.border_width", style.border_width),
            StyleEntry::number("card.radius", style.radius),
            StyleEntry::text("card.type", "type.caption"),
            StyleEntry::number("card.size.w", TOOLTIP_CARD_SIZE.width),
            StyleEntry::number("card.size.h", TOOLTIP_CARD_SIZE.height),
            StyleEntry::number("card.max_width", style.max_width),
            StyleEntry::number("card.padding_v", style.padding_vertical),
            StyleEntry::number("card.padding_h", style.padding_horizontal),
            StyleEntry::number("hover-delay-ms", tooltip::HOVER_DELAY as f32),
            StyleEntry::number("focus-delay-ms", tooltip::KEYBOARD_FOCUS_DELAY as f32),
        ],
        frame: paint_frame(
            Namer::Standard,
            viewport(),
            &tooltip_card_layers(&style, card, None),
        ),
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
        note: Some(
            "Every figure here is what §3.7 now specifies: `surface.overlay` fill, `1px \
             border.strong`, `text.primary` at `type.caption`, `radius.widget` (4px), \
             280px max width and `4px 8px` padding. The text colour is named by the spec, \
             not chosen: `text.primary` because the label is this surface's primary \
             content. The card fill is only 1.1230:1 from `surface.base`, so the \
             `border.strong` hairline is what makes the card read as a separate layer."
                .to_string(),
        ),
    }
}

/// One cell per hover-machine visibility, driven by the real controller so the gallery
/// cannot drift from the state machine it is depicting.
#[must_use]
fn tooltip_state_cell(name: &'static str, controller: &HoverController, now: TimeMs) -> Cell {
    let palette = Palette::dark();
    let style = TooltipStyle::resolve(&palette);
    let visibility = controller.tick(now);
    let (band, entry) = gallery_navbar(viewport().width);
    let placement = TooltipAnchor::band(band, entry).resolve(TOOLTIP_CARD_SIZE, viewport());

    // Nothing is painted for the card until the state is `Shown`; that is the whole
    // content of the two non-shown cells.
    let layers = if visibility.is_visible() {
        tooltip_card_layers(&style, placement.rect(), Some(band))
    } else {
        vec![(
            "anchor-bar",
            surface_layer(
                band,
                palette.surface_raised,
                palette.border_subtle,
                shape::BORDER_HAIRLINE,
                0.0,
            ),
        )]
    };

    let mut resolved = vec![
        StyleEntry::text("visibility", format!("{visibility:?}")),
        StyleEntry::text("paints", if visibility.is_visible() { "yes" } else { "no" }),
        StyleEntry::number("hover-delay-ms", tooltip::HOVER_DELAY as f32),
        StyleEntry::number("focus-delay-ms", tooltip::KEYBOARD_FOCUS_DELAY as f32),
        StyleEntry::number(
            "active-delay-ms",
            controller.active_delay().as_millis() as f32,
        ),
        StyleEntry::text(
            "arrival",
            match controller.arrival_source() {
                Some(ArrivalSource::Pointer) => "pointer",
                Some(ArrivalSource::Keyboard) => "keyboard",
                None => "none",
            },
        ),
        StyleEntry::number("card.max_width", style.max_width),
        StyleEntry::number("card.padding_v", style.padding_vertical),
        StyleEntry::number("card.padding_h", style.padding_horizontal),
        StyleEntry::text("armed", if controller.is_active() { "yes" } else { "no" }),
        StyleEntry::text(
            "suppressed",
            if controller.is_suppressed() {
                "yes"
            } else {
                "no"
            },
        ),
    ];
    if visibility.is_visible() {
        resolved.push(StyleEntry::text(
            "placement",
            format!("below bar, gap {}", tooltip::TOOLTIP_GAP),
        ));
    }

    Cell {
        id: format!("tooltip/state-{name}"),
        kind: CellKind::Composite,
        subject: "tooltip",
        spec: "UIF-ATOM-007 + UIF-NAV-003",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport(), &layers),
        // The style is resolved in every state; only the *paint* is suppressed. Auditing
        // the resolved pairing here rather than only in the `Shown` cell is deliberate:
        // the gate asks whether a cell resolved anything auditable, and a tooltip that is
        // waiting still knows exactly what it would paint.
        audit: vec![AuditPair {
            channel: "tooltip.text-on-card",
            foreground: style.foreground,
            background: style.background,
            role: ContrastRole::Text,
        }],
        note: Some(match (visibility, controller.arrival_source()) {
            (Visibility::Hidden, _) => {
                "No channel on the control, or dismissed and suppressed.".to_string()
            }
            (Visibility::Pending, _) => format!(
                "Armed but the {} ms this channel waits has not elapsed: nothing paints. This \
                 state is the reason the subsystem exists — without it a tooltip fires while \
                 the pointer is still crossing the window.",
                controller.active_delay().as_millis()
            )
            .to_string(),
            (Visibility::Shown, Some(ArrivalSource::Keyboard)) => format!(
                "Reached by keyboard, so the wait is {} ms and the card is painted on the \
                 first tick — no `Pending` frame at all. Same placement as the hover card: \
                 below the bar, not below the inset button.",
                tooltip::KEYBOARD_FOCUS_DELAY
            )
            .to_string(),
            (Visibility::Shown, _) => "Delay elapsed with the pointer still resting; the card \
             is painted below the bar, not below the inset button."
                .to_string(),
        }),
    }
}

/// The visibility states, each produced by driving a real controller — the three a pointer
/// walks, plus the keyboard arrival that must skip `Pending` entirely.
#[must_use]
pub fn tooltip_state_cells() -> Vec<Cell> {
    let t0 = TimeMs::millis(10_000);
    let delay = tooltip::HOVER_DELAY;

    let hidden = HoverController::new();

    let mut pending = HoverController::new();
    pending.pointer_enter(t0);

    let mut shown = HoverController::new();
    shown.pointer_enter(t0);

    // Ticked at the arrival instant itself: with a 0 ms keyboard delay this is `Shown`,
    // which is the whole conformance claim for `UIF-ATOM-007`'s keyboard-focus rule.
    let mut keyboard = HoverController::new();
    keyboard.keyboard_focus_enter(t0);

    vec![
        tooltip_state_cell("hidden", &hidden, t0),
        tooltip_state_cell(
            "pending",
            &pending,
            TimeMs::millis(t0.as_millis() + delay - 1),
        ),
        tooltip_state_cell("shown", &shown, TimeMs::millis(t0.as_millis() + delay)),
        tooltip_state_cell("keyboard", &keyboard, t0),
    ]
}

/// A tooltip that had to flip above its anchor because the anchor sits at the bottom of the
/// window — the tab-strip case — with the gap preserved across the flip.
#[must_use]
pub fn tooltip_flip_cell() -> Cell {
    let palette = Palette::dark();
    let style = TooltipStyle::resolve(&palette);
    let view = viewport();

    let band = Rectangle::new(
        Point::new(0.0, view.height - size::CONTROL_LG),
        Size::new(view.width, size::CONTROL_LG),
    );
    let entry = Rectangle::new(
        Point::new(space::SM, view.height - size::CONTROL_LG + 4.0),
        Size::new(space::XL * 4.0, size::CONTROL_SM),
    );
    let placement = TooltipAnchor::band(band, entry).resolve(TOOLTIP_CARD_SIZE, view);

    Cell {
        id: "tooltip/place-flip-above".to_string(),
        kind: CellKind::Composite,
        subject: "tooltip",
        spec: "UIF-NAV-003 + UIF-TAB-003",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::text("flipped-vertical", placement.flipped_vertical.to_string()),
            StyleEntry::text("clamped", placement.was_clamped().to_string()),
            StyleEntry::number("card.y", placement.origin.y),
            StyleEntry::number(
                "gap-above-bar",
                band.y - (placement.origin.y + placement.size.height),
            ),
            StyleEntry::number("gap-token", tooltip::TOOLTIP_GAP),
        ],
        frame: paint_frame(
            Namer::Standard,
            view,
            &tooltip_card_layers(&style, placement.rect(), Some(band)),
        ),
        audit: vec![AuditPair {
            channel: "tooltip.text-on-card",
            foreground: style.foreground,
            background: style.background,
            role: ContrastRole::Text,
        }],
        note: Some(
            "No room below a bottom-anchored strip, so the solver mirrored the axis. The \
             mirrored rule keeps the same space.xs gap, so the card does not land on the \
             bar's top edge."
                .to_string(),
        ),
    }
}

/// A tooltip whose anchor sits near the right edge, where the `Start` preference mirrors
/// to right-edge alignment and the viewport keeps the card on screen.
#[must_use]
pub fn tooltip_clamped_cell() -> Cell {
    let palette = Palette::dark();
    let style = TooltipStyle::resolve(&palette);
    let view = viewport();

    let band = Rectangle::new(
        Point::new(0.0, 0.0),
        Size::new(view.width, size::CONTROL_LG),
    );
    let entry = Rectangle::new(
        Point::new(view.width - space::XL * 4.0, 4.0),
        Size::new(space::XL * 4.0, size::CONTROL_SM),
    );
    let wide = Size::new(space::XL * 12.0, size::HIT_TARGET_MIN);
    let placement = TooltipAnchor::band(band, entry).resolve(wide, view);

    Cell {
        id: "tooltip/place-clamped".to_string(),
        kind: CellKind::Composite,
        subject: "tooltip",
        spec: "UIF-NAV-003 + UIF-TAB-003",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::text(
                "flipped-horizontal",
                placement.flipped_horizontal.to_string(),
            ),
            StyleEntry::text(
                "clamped-horizontal",
                placement.clamped_horizontal.to_string(),
            ),
            StyleEntry::number("card.x", placement.origin.x),
            StyleEntry::number("card.right", placement.origin.x + placement.size.width),
            StyleEntry::number("viewport.right-margin", view.width - space::SM),
        ],
        frame: paint_frame(
            Namer::Standard,
            view,
            &tooltip_card_layers(&style, placement.rect(), Some(band)),
        ),
        audit: vec![AuditPair {
            channel: "tooltip.border-on-card",
            foreground: style.border_color,
            background: style.background,
            role: ContrastRole::Chrome,
        }],
        note: Some(
            "A card too wide to hang from a right-edge entry: the solver mirrors what it \
             can and clamps the rest, so the card stays inside the viewport margin instead \
             of being pushed off-screen."
                .to_string(),
        ),
    }
}

/// Every tooltip cell in the `UIF-VER-004` matrix.
#[must_use]
pub fn tooltip_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    cells.push(tooltip_card_cell());
    cells.extend(tooltip_state_cells());
    cells.push(tooltip_flip_cell());
    cells.push(tooltip_clamped_cell());
    cells.extend(crate::gallery_warm::cells());
    cells
}
