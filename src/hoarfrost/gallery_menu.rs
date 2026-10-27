//! The menu cells of the `UIF-VER-004` matrix (`UIF-CMENU-001`,
//! `UIF-FMENU-001`, `UIF-ATOM-009`, `UIF-ATOM-000b`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit. These cells are built through `gallery`'s own
//! vocabulary and [`gallery::matrix`] calls [`menu_cells`],
//! [`menu_glyph_cells`] and [`menu_row_tier_cells`] back, so the matrix is
//! still one list in one fixed order — the split moved the code, not the
//! artifact.
//!
//! # Three families
//!
//! * [`menu_cells`] — the composite card in every row state the resolver
//!   defines (§4.9.2), one cell per [`menu_view::RowState`].
//! * [`menu_glyph_cells`] — separators and glyph placement (§4.8.1),
//!   including the `UIF-TOK-006` question of whether a resting divider
//!   and an indicator stroke can be told apart by where they are painted.
//!   `leading_ink_box` and `trailing_ink_box` moved with this family
//!   because only the glyph cells measure ink at an end.
//! * [`menu_row_tier_cells`] — the `UIF-ATOM-000b` binding tiers for a
//!   menu row, driven through a real [`RowActionEngine`] so the cell
//!   records the settlement the engine actually produced.
//!
//! The three were separate sections of `gallery.rs` and stay separate
//! sections here; what they share is the subject, not the code path.
//!
//! The tests for these cells are in the gallery's own `gallery_tests.rs`.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix

use crate::binding::ActionPending;
use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, ink_box, paint_frame, rect_text,
    tier_how,
};
use crate::surface::menu::model::ActivationEffect;
use crate::surface::menu::model::Menu;
use crate::surface::menu::model::RadioGroupId;
use crate::surface::menu::model::Row;
use crate::surface::menu::row_action::RowActionEngine;
use crate::surface::menu::row_action::RowEmission;
use crate::surface::menu::view as menu_view;
use crate::token::color::Palette;
use crate::token::space;
use crate::widget::button_controller::ActivationSource;
use crate::widget::button_widget::Layer;
use iced::Color;
use iced::Point;
use iced::Rectangle;
use iced::Size;

/// A row that legitimately reaches [`RowState`] `state`.
///
/// The row state matrix is over `(base, focused)`, but a row's *kind* has
/// to agree with the base for the cell to be one a real menu produces: a
/// `SectionHeading` base only arrives on a heading row, a
/// `DestructivePressed` base only on a destructive one. Pairing them
/// arbitrarily would snapshot cells no menu can reach.
fn sample_row(state: menu_view::RowState) -> Row {
    match state.base {
        menu_view::RowBase::SectionHeading => Row::heading("Section heading"),
        menu_view::RowBase::Disabled => Row::action("off", "Unavailable action").disabled(),
        menu_view::RowBase::DestructiveHovered | menu_view::RowBase::DestructivePressed => {
            Row::action("delete", "Delete item").destructive()
        }
        menu_view::RowBase::ActiveSubmenuParent => {
            Row::submenu("submenu", "Open submenu", Menu::empty("gallery.menu.child"))
        }
        menu_view::RowBase::Rest | menu_view::RowBase::Hovered | menu_view::RowBase::Pressed => {
            Row::action("item", "Ordinary action")
        }
    }
}

/// A [`RowState`] as a cell-id slug.
fn row_slug(state: menu_view::RowState) -> String {
    let base = match state.base {
        menu_view::RowBase::Rest => "rest",
        menu_view::RowBase::Hovered => "hovered",
        menu_view::RowBase::Pressed => "pressed",
        menu_view::RowBase::ActiveSubmenuParent => "submenu-parent",
        menu_view::RowBase::DestructiveHovered => "destructive",
        menu_view::RowBase::DestructivePressed => "destructive-pressed",
        menu_view::RowBase::Disabled => "disabled",
        menu_view::RowBase::SectionHeading => "heading",
    };

    if state.focused {
        format!("{base}+focused")
    } else {
        base.to_string()
    }
}

/// The menu-card cell for one row state (`UIF-CMENU-001`, §4.9.2).
///
/// One cell per entry of [`menu_view::RowState::ALL`] — the eight rows the
/// matrix prints plus the three derived combinations and the hand-built
/// `rest+focused` cell. Resolved through
/// [`menu_view::resolve_row_style`], so the ring on a destructive row is
/// the adaptive one that keeps it visible rather than the matrix's printed
/// accent.
#[must_use]
pub fn menu_row_cell(state: menu_view::RowState) -> Cell {
    let palette = Palette::dark();
    let row = sample_row(state);
    let style = menu_view::resolve_row_style(&row, state, &palette);
    let bounds = Rectangle::new(
        Point::ORIGIN,
        Size::new(space::XL * 12.0, menu_view::row_height(&row)),
    );

    let mut layers: Vec<(&'static str, Layer)> = Vec::new();
    if let Some(layer) = menu_view::background_layer(bounds, &style) {
        layers.push(("row-background", layer));
    }
    if let Some(layer) = menu_view::focus_outline_layer(bounds, &style) {
        layers.push(("focus-outline", layer));
    }

    let mut audit = vec![
        AuditPair {
            channel: "leading",
            foreground: style.leading,
            background: style.background,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "label",
            foreground: style.label,
            background: style.background,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "trailing",
            foreground: style.trailing,
            background: style.background,
            role: ContrastRole::Text,
        },
    ];

    if let Some(outline) = style.focus_outline {
        audit.push(AuditPair {
            channel: "focus-outline",
            foreground: outline.color,
            background: style.background,
            role: ContrastRole::Chrome,
        });
    }

    let focus_entries = match style.focus_outline {
        Some(outline) => vec![
            StyleEntry::colour("focus-outline.color", outline.color),
            StyleEntry::number("focus-outline.width", outline.width),
            StyleEntry::text(
                "focus-outline.placement",
                format!("{:?}", outline.placement),
            ),
        ],
        None => vec![StyleEntry::colour(
            "focus-outline.color",
            Color::TRANSPARENT,
        )],
    };

    let mut resolved = vec![
        StyleEntry::colour("background", style.background),
        StyleEntry::colour("leading", style.leading),
        StyleEntry::colour("label", style.label),
        StyleEntry::colour("trailing", style.trailing),
        StyleEntry::text("cursor", format!("{:?}", style.cursor)),
        StyleEntry::text("matrix-row", state.matrix_row()),
        StyleEntry::text("row-kind", format!("{:?}", row.kind())),
        StyleEntry::number("geometry.height", bounds.height),
        StyleEntry::number("geometry.padding-x", menu_view::ROW_PADDING_X),
        StyleEntry::number("geometry.radius", menu_view::ROW_RADIUS),
    ];
    resolved.extend(focus_entries);

    Cell {
        id: format!("menu/row/{row_slug}", row_slug = row_slug(state)),
        kind: CellKind::Composite,
        subject: "menu",
        spec: "UIF-CMENU-001",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, bounds, &layers),
        audit,
        note: None,
    }
}

/// Every menu-card cell — one per row state the resolver defines.
#[must_use]
pub fn menu_cells() -> Vec<Cell> {
    menu_view::RowState::ALL
        .iter()
        .copied()
        .map(menu_row_cell)
        .collect()
}

/// The leading slot's reserved ink box inside a menu row.
fn leading_ink_box(bounds: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(
            bounds.x + menu_view::ROW_PADDING_X,
            bounds.y + (bounds.height - menu_view::LEADING_SLOT) / 2.0,
        ),
        Size::new(menu_view::LEADING_SLOT, menu_view::LEADING_SLOT),
    )
}

/// The trailing slot's reserved ink box, mirrored from the leading one.
fn trailing_ink_box(bounds: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(
            bounds.x + bounds.width - menu_view::ROW_PADDING_X - menu_view::LEADING_SLOT,
            bounds.y + (bounds.height - menu_view::LEADING_SLOT) / 2.0,
        ),
        Size::new(menu_view::LEADING_SLOT, menu_view::LEADING_SLOT),
    )
}

/// One menu glyph or separator cell (§4.8.1 slot anatomy, §4.9.2 matrix).
///
/// The row is resolved through [`menu_view::resolve_row_style`] like every
/// other menu cell, but the assertion here is about the *slots*: which
/// glyph the model asks for, where its box sits inside the row's padding,
/// and whether the colour it is drawn in is legible on the surface
/// actually painted behind it.
///
/// A resting row paints no fill of its own, so the glyph's contrast is
/// measured against the menu card's `surface.overlay` rather than being
/// waved through as unpainted. That is the pairing a user sees, and it is
/// the one `UIF-TOK-006` rule 1 is about.
fn menu_glyph_cell(
    name: &str,
    row: Row,
    state: menu_view::RowState,
    leading_glyph: Option<&'static str>,
    trailing_glyph: Option<&'static str>,
) -> Cell {
    let palette = Palette::dark();
    let style = menu_view::resolve_row_style(&row, state, &palette);
    let bounds = Rectangle::new(
        Point::ORIGIN,
        Size::new(space::XL * 12.0, menu_view::row_height(&row)),
    );
    let behind = if style.background.a == 0.0 {
        palette.surface_overlay
    } else {
        style.background
    };

    let mut layers = Vec::new();
    let mut resolved = vec![
        StyleEntry::text("matrix-row", state.matrix_row()),
        StyleEntry::text("row-kind", format!("{:?}", row.kind())),
        StyleEntry::text("leading-slot", format!("{:?}", row.leading_slot())),
        StyleEntry::text("trailing-slot", format!("{:?}", row.trailing_slot())),
        StyleEntry::text("checked", row.is_checked().to_string()),
        StyleEntry::text("selected", row.is_selected().to_string()),
        StyleEntry::number("row.height", bounds.height),
        StyleEntry::number("row.padding-x", menu_view::ROW_PADDING_X),
        StyleEntry::number("leading-slot.width", menu_view::LEADING_SLOT),
    ];
    let mut audit = Vec::new();

    if row.is_separator() {
        let rule = menu_view::separator_layer(bounds, &palette);
        layers.push(("separator-rule", rule));
        resolved.push(StyleEntry::number(
            "separator.band",
            menu_view::SEPARATOR_HEIGHT,
        ));
        resolved.push(StyleEntry::number(
            "separator.thickness",
            menu_view::SEPARATOR_THICKNESS,
        ));
        resolved.push(StyleEntry::text("geometry.rule", rect_text(rule.bounds)));
        audit.push(AuditPair {
            channel: "separator",
            foreground: palette.border_subtle,
            background: behind,
            role: ContrastRole::Chrome,
        });
    } else {
        resolved.push(StyleEntry::colour("row.background", style.background));
        resolved.push(StyleEntry::colour("row.label", style.label));
        audit.push(AuditPair {
            channel: "row.label",
            foreground: style.label,
            background: behind,
            role: ContrastRole::Text,
        });
        if let Some(layer) = menu_view::background_layer(bounds, &style) {
            layers.push(("row-background", layer));
        }
        if let Some(layer) = menu_view::focus_outline_layer(bounds, &style) {
            layers.push(("focus-outline", layer));
        }
        if let Some(outline) = style.focus_outline {
            resolved.push(StyleEntry::colour("focus-outline.color", outline.color));
            resolved.push(StyleEntry::number("focus-outline.width", outline.width));
            audit.push(AuditPair {
                channel: "focus-outline",
                foreground: outline.color,
                background: behind,
                role: ContrastRole::Chrome,
            });
        }

        if let Some(glyph) = leading_glyph {
            let box_bounds = leading_ink_box(bounds);
            layers.push(("leading-glyph", ink_box(box_bounds, style.leading)));
            resolved.push(StyleEntry::text("leading-glyph.char", glyph));
            resolved.push(StyleEntry::colour("leading-glyph.color", style.leading));
            resolved.push(StyleEntry::text(
                "geometry.leading-glyph",
                rect_text(box_bounds),
            ));
            audit.push(AuditPair {
                channel: "leading-glyph",
                foreground: style.leading,
                background: behind,
                role: ContrastRole::Text,
            });
        }
        if let Some(glyph) = trailing_glyph {
            let box_bounds = trailing_ink_box(bounds);
            layers.push(("trailing-glyph", ink_box(box_bounds, style.trailing)));
            resolved.push(StyleEntry::text("trailing-glyph.char", glyph));
            resolved.push(StyleEntry::colour("trailing-glyph.color", style.trailing));
            resolved.push(StyleEntry::text(
                "geometry.trailing-glyph",
                rect_text(box_bounds),
            ));
            audit.push(AuditPair {
                channel: "trailing-glyph",
                foreground: style.trailing,
                background: behind,
                role: ContrastRole::Text,
            });
        }
    }

    Cell {
        id: format!("menu-glyph/{name}"),
        kind: CellKind::Composite,
        subject: "menu-glyph",
        spec: "UIF-FMENU-001",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, bounds, &layers),
        audit,
        note: None,
    }
}

/// Every menu glyph and separator cell.
///
/// One per placement the model can ask for, plus the two states the spec
/// calls out by name: the submenu parent's chevron while its branch is
/// open, and a checked row under the pointer.
#[must_use]
pub fn menu_glyph_cells() -> Vec<Cell> {
    let rest = menu_view::RowState::REST;
    let hovered = menu_view::RowState::HOVERED;
    let submenu_open = menu_view::RowState::ACTIVE_SUBMENU_PARENT;
    let child = Menu::empty("gallery.glyph.child");
    let group = RadioGroupId::new("gallery.glyph.indent");

    vec![
        menu_glyph_cell("separator", Row::separator(), rest, None, None),
        menu_glyph_cell(
            "check-on",
            Row::checkable("wrap", "Wrap Lines", true),
            rest,
            Some(menu_view::GLYPH_CHECK),
            None,
        ),
        menu_glyph_cell(
            "check-off",
            Row::checkable("wrap", "Wrap Lines", false),
            rest,
            None,
            None,
        ),
        menu_glyph_cell(
            "radio-selected",
            Row::radio("indent-2", "Two Spaces", group.clone(), true),
            rest,
            Some(menu_view::GLYPH_RADIO),
            None,
        ),
        menu_glyph_cell(
            "radio-unselected",
            Row::radio("indent-4", "Four Spaces", group, false),
            rest,
            None,
            None,
        ),
        menu_glyph_cell(
            "submenu-chevron",
            Row::submenu("more", "More Actions", child.clone()),
            rest,
            None,
            Some(menu_view::GLYPH_CHEVRON),
        ),
        menu_glyph_cell(
            "submenu-chevron-open",
            Row::submenu("more", "More Actions", child),
            submenu_open,
            None,
            Some(menu_view::GLYPH_CHEVRON),
        ),
        menu_glyph_cell(
            "check-on-hovered",
            Row::checkable("wrap", "Wrap Lines", true),
            hovered,
            Some(menu_view::GLYPH_CHECK),
            None,
        ),
        menu_glyph_cell(
            "shortcut",
            Row::action("save", "Save Document").with_shortcut("Ctrl+S"),
            rest,
            None,
            Some("Ctrl+S"),
        ),
        menu_glyph_cell(
            "icon",
            Row::action("new", "New Document").with_icon("+"),
            rest,
            Some("+"),
            None,
        ),
    ]
}

// ---------------------------------------------------------------------------
// Cells for the input dialog's per-row bodies (§4.5.1)
// ---------------------------------------------------------------------------

/// The menu-row tier a cell records, and how a user produces it.
pub const MENU_ROW_TIERS: [(&str, &str); 5] = [
    ("committed", "release inside a stay-open check row"),
    ("cancelled", "press a row and release outside its bounds"),
    (
        "cancel-requested",
        "press a row whose action is busy and cancellable",
    ),
    (
        "suppressed",
        "activate the same row twice before the projection catches up",
    ),
    (
        "fire-and-settle",
        "activate an Action row that closes the tree",
    ),
];

/// The `UIF-ATOM-000b` tier cell for one menu-row outcome.
///
/// Drives a real [`RowActionEngine`] and records the tier it actually
/// emitted plus the [`RevertKind`](crate::surface::menu::row_action::RevertKind)
/// the engine classified the row as, so the
/// cell is a transcript rather than a caption — the discipline
/// [`crate::gallery_button::button_tier_cell`] sets. The paint underneath
/// is the menu row's own
/// `rest` cell, deliberately unchanged: the engine answers whether the
/// spinner *slot* is occupied, it does not paint one (`UIF-ATOM-009`
/// owns that), and nothing here may suggest a colour that §4.9.2 does not
/// name.
#[must_use]
pub fn menu_row_tier_cell(tier: &'static str) -> Cell {
    let stay_open = Row::checkable("wrap", "Wrap lines", false).stays_open();
    let closing = Row::action("save", "Save");
    let busy = ActionPending::indeterminate().with_cancellable(true);
    let tracked_idle = ActionPending::idle().with_cancellable(true);

    let mut engine = RowActionEngine::new();
    let emission: Option<RowEmission> = match tier {
        "committed" => {
            engine.observe(ActionPending::idle());
            engine.begin(&stay_open, ActivationSource::Pointer);
            Some(
                engine
                    .commit(
                        &stay_open,
                        &ActivationEffect::Toggled { checked: true },
                        ActivationSource::Pointer,
                    )
                    .expect("a stay-open check dispatches"),
            )
        }
        "cancelled" => {
            engine.observe(ActionPending::idle());
            engine.begin(&closing, ActivationSource::Pointer);
            Some(engine.cancel().expect("an open gesture cancels"))
        }
        "cancel-requested" => {
            engine.observe(busy);
            Some(
                engine
                    .begin(&closing, ActivationSource::Pointer)
                    .expect("a busy cancellable row requests"),
            )
        }
        "suppressed" => {
            engine.observe(tracked_idle);
            engine
                .commit(
                    &closing,
                    &ActivationEffect::Command,
                    ActivationSource::Pointer,
                )
                .expect("the first press dispatches");
            engine.observe(tracked_idle);
            assert!(
                engine
                    .commit(
                        &closing,
                        &ActivationEffect::Command,
                        ActivationSource::Pointer
                    )
                    .is_none(),
                "the double-activation window must withhold the second press"
            );
            // A withheld press emits nothing at all: that silence *is* the
            // tier behaviour, so the cell records the gate's counter and
            // reports the emitted tier as `none`.
            None
        }
        "fire-and-settle" => {
            engine.observe(ActionPending::idle());
            Some(
                engine
                    .commit(
                        &closing,
                        &ActivationEffect::Command,
                        ActivationSource::Pointer,
                    )
                    .expect("an Action row dispatches"),
            )
        }
        other => panic!("unknown menu row tier {other}"),
    };

    let mut cell = menu_row_cell(menu_view::RowState::REST);
    cell.id = format!("menu-tier/{tier}");
    cell.subject = "menu-tier";
    cell.spec = "UIF-ATOM-000b";
    cell.resolved.extend([
        StyleEntry::text("tier", tier),
        StyleEntry::text(
            "emitted",
            emission.as_ref().map(RowEmission::tier).unwrap_or("none"),
        ),
        StyleEntry::text(
            "revert",
            emission
                .as_ref()
                .and_then(RowEmission::settlement)
                .map(|settlement| settlement.revert.slug())
                .unwrap_or("n/a"),
        ),
        StyleEntry::text(
            "dispatched",
            emission
                .as_ref()
                .and_then(RowEmission::settlement)
                .map(|settlement| settlement.dispatched.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
        ),
        StyleEntry::text("fired-count", engine.fired().to_string()),
        StyleEntry::text("suppressed-count", engine.suppressed().to_string()),
    ]);
    cell.note = Some(format!("produced by: {}", tier_how(&MENU_ROW_TIERS, tier)));
    cell
}

/// Every menu-row tier cell, in [`MENU_ROW_TIERS`] order.
#[must_use]
pub fn menu_row_tier_cells() -> Vec<Cell> {
    MENU_ROW_TIERS
        .iter()
        .map(|(tier, _)| menu_row_tier_cell(tier))
        .collect()
}
