//! The movable-overlay cells of the `UIF-VER-004` matrix
//! (`UIF-MOV-001`/`002`/`004`, `UIF-WIN-003`, `UIF-ATOM-000b`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit. These cells are built through `gallery`'s own
//! vocabulary and [`gallery::matrix`] calls [`movable_cells`],
//! [`zband_cell`], [`movable_action_cells`] and [`drag_tier_cells`] back,
//! so the matrix is still one list in one fixed order — the split moved
//! the code, not the artifact.
//!
//! # Four families, one shell
//!
//! * [`movable_cells`] — the shell idle, dragging, resizing, and with
//!   follow switched off by scrolling away from the bottom.
//! * [`zband_cell`] — the whole z-band stack (§5 `UIF-ZORD-*`,
//!   `UIF-WIN-003`), the one cell that shows every band at once rather
//!   than one band per cell.
//! * [`movable_action_cells`] — the trailing actions of §4.7.1.
//! * [`drag_tier_cells`] — the `000b` binding tiers for a drag, driven
//!   through a real [`DragGesture`] and a driven [`MovableHost`] so the
//!   cell records what the gesture actually settled.
//!
//! `MovableGesture` and `driven_movable_host` moved with them: both exist
//! only to put this shell through a gesture, and no other family drives
//! one.
//!
//! The tests for these cells are in the gallery's own `gallery_tests.rs`.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix

use crate::focus::FocusState;
use crate::gallery::{
    AuditPair, CHANNEL_EPS, Cell, CellKind, ContrastRole, Namer, StyleEntry, ink_box, paint_frame,
    rect_text, surface_layer, tier_how, viewport,
};
use crate::geometry::clamp::ResizeHandle;
use crate::surface::drag_tier::DragGesture;
use crate::surface::modal::ModalFrame;
use crate::surface::movable::{
    ACTION_CONTROL_SIZE, ACTION_GAP, AutoScrollSetting, MemoryGeometryStorage, MovableAnatomy,
    MovableGeometry, MovableHost, MovableSpec, PointerPhase, ScrollGeometry, SurfaceTarget,
};
use crate::surface::zorder::{SurfaceId, SurfaceKind, SurfaceRegistry, ZBand};
use crate::token::color::Palette;
use crate::token::shape;
use crate::token::space;
use iced::Color;
use iced::Point;
use iced::Rectangle;
use iced::Size;

/// The gesture a movable-overlay cell puts the host through before it is
/// recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MovableGesture {
    /// No gesture — the card as it opens.
    Idle,
    /// Press the title bar and drag it (`UIF-MOV-001`).
    Drag,
    /// Press the bottom-right grip and pull it (`UIF-MOV-002`).
    Resize,
    /// Scroll the viewport away from the bottom (`UIF-MOV-004`).
    ScrollAway,
}

/// Open a movable host against a fresh registry and drive it through one
/// gesture.
///
/// Shared by every movable cell so the card a cell reports is the card the
/// host reached, not the card the cell described. The host is opened
/// through [`MovableHost::open`] against a [`MemoryGeometryStorage`] so
/// the clamp, the Band 1 registration and the modeless focus registration
/// are the real ones, and driven through [`MovableHost::on_pointer`] —
/// press then move — rather than by asserting the drag flag directly.
fn driven_movable_host(
    gesture: MovableGesture,
    viewport: Rectangle,
    focus: &mut FocusState,
    registry: &mut SurfaceRegistry,
    storage: &mut MemoryGeometryStorage,
) -> MovableHost {
    let spec = MovableSpec::new("gallery.movable", "Background Operations Log")
        .with_geometry(MovableGeometry::new(
            Point::new(space::XL * 4.0, space::XL * 3.0),
            Size::new(space::XL * 16.0, space::XL * 11.0),
        ))
        .with_auto_scroll(AutoScrollSetting::Enabled);

    let mut host = MovableHost::open(spec, viewport, focus, registry, storage);

    let title = host.title_bar();
    let grip = host.resize_grip(ResizeHandle::BottomRight);

    match gesture {
        MovableGesture::Idle => {}
        MovableGesture::Drag => {
            let at = Point::new(title.x + space::SM, title.y + title.height / 2.0);
            let to = Point::new(at.x + space::LG, at.y + space::MD);
            let _ = host.on_pointer(
                PointerPhase::Pressed,
                at,
                SurfaceTarget::TitleBar,
                focus,
                registry,
                storage,
            );
            let _ = host.on_pointer(
                PointerPhase::Moved,
                to,
                SurfaceTarget::TitleBar,
                focus,
                registry,
                storage,
            );
        }
        MovableGesture::Resize => {
            if let Some(grip) = grip {
                let at = Point::new(
                    grip.x + grip.width - space::XS,
                    grip.y + grip.height - space::XS,
                );
                let to = Point::new(at.x + space::LG, at.y + space::MD);
                let target = SurfaceTarget::Resize(ResizeHandle::BottomRight);
                let _ = host.on_pointer(
                    PointerPhase::Pressed,
                    at,
                    target.clone(),
                    focus,
                    registry,
                    storage,
                );
                let _ = host.on_pointer(PointerPhase::Moved, to, target, focus, registry, storage);
            }
        }
        MovableGesture::ScrollAway => {
            let _ = host.sync_scroll(ScrollGeometry::new(
                space::XL * 4.0,
                space::XL * 40.0,
                space::XL * 8.0,
            ));
        }
    }

    host
}

/// The movable-overlay cell for one gesture (`UIF-MOV-001` … `UIF-MOV-004`).
///
/// Built on [`driven_movable_host`], so `dragging` and `resizing` are the
/// host's own answers after a real gesture rather than values this cell
/// wrote down.
fn movable_cell(name: &str, gesture: MovableGesture) -> Cell {
    let palette = Palette::dark();
    let anatomy = MovableAnatomy::resolve(&palette);
    let viewport = viewport();

    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let mut storage = MemoryGeometryStorage::new();
    let host = driven_movable_host(gesture, viewport, &mut focus, &mut registry, &mut storage);
    let grip = host.resize_grip(ResizeHandle::BottomRight);

    let layers = vec![
        (
            "card",
            surface_layer(
                host.card(),
                anatomy.card_surface,
                anatomy.card_border,
                anatomy.card_border_width,
                anatomy.card_radius,
            ),
        ),
        (
            "title-bar",
            surface_layer(
                host.title_bar(),
                anatomy.title_bar_surface,
                Color::TRANSPARENT,
                0.0,
                0.0,
            ),
        ),
        (
            "filter-bar",
            surface_layer(
                host.filter_bar(),
                anatomy.bar_surface,
                Color::TRANSPARENT,
                0.0,
                0.0,
            ),
        ),
        (
            "content-viewport",
            surface_layer(
                host.content_viewport(),
                anatomy.content_surface,
                Color::TRANSPARENT,
                0.0,
                0.0,
            ),
        ),
        (
            "footer",
            surface_layer(
                host.footer(),
                anatomy.bar_surface,
                Color::TRANSPARENT,
                0.0,
                0.0,
            ),
        ),
    ];

    let audit = vec![
        AuditPair {
            channel: "title",
            foreground: anatomy.title_text,
            background: anatomy.title_bar_surface,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "meta",
            foreground: anatomy.meta_text,
            background: anatomy.bar_surface,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "tag",
            foreground: anatomy.tag_text,
            background: anatomy.card_surface,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "card.border",
            foreground: anatomy.card_border,
            background: anatomy.card_surface,
            role: ContrastRole::Chrome,
        },
        AuditPair {
            channel: "divider",
            foreground: anatomy.divider,
            background: anatomy.bar_surface,
            role: ContrastRole::Chrome,
        },
    ];

    let mut resolved = vec![
        StyleEntry::text("geometry.card", rect_text(host.card())),
        StyleEntry::text("geometry.title-bar", rect_text(host.title_bar())),
        StyleEntry::text("geometry.filter-bar", rect_text(host.filter_bar())),
        StyleEntry::text(
            "geometry.content-viewport",
            rect_text(host.content_viewport()),
        ),
        StyleEntry::text("geometry.footer", rect_text(host.footer())),
        StyleEntry::text(
            "geometry.grip-bottom-right",
            grip.map_or_else(|| "none".to_string(), rect_text),
        ),
        StyleEntry::number("card.radius", anatomy.card_radius),
        StyleEntry::number("card.border-width", anatomy.card_border_width),
        StyleEntry::number("title-bar.height", anatomy.title_bar_height),
        StyleEntry::number("filter-bar.height", anatomy.filter_bar_height),
        StyleEntry::number("footer.height", anatomy.footer_height),
        StyleEntry::number("grip.size", anatomy.resize_handle_size),
        StyleEntry::text("dragging", host.is_dragging().to_string()),
        StyleEntry::text("resizing", format!("{:?}", host.resizing_handle())),
        StyleEntry::text("auto-scroll", format!("{:?}", host.auto_scroll())),
        StyleEntry::text("follow", format!("{:?}", host.follow_state())),
        StyleEntry::text("at-bottom", host.at_bottom().to_string()),
        StyleEntry::text("band", format!("{:?}", ZBand::UtilityOverlay)),
    ];
    if let Some(grip) = grip {
        resolved.push(StyleEntry::text("geometry.grip-hit", rect_text(grip)));
    }

    Cell {
        id: format!("movable/{name}"),
        kind: CellKind::Composite,
        subject: "movable",
        spec: "UIF-MOV-001",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport, &layers),
        audit,
        note: None,
    }
}

/// Every movable-overlay cell: idle, dragging, resizing, and following
/// switched off by scrolling away from the bottom.
#[must_use]
pub fn movable_cells() -> Vec<Cell> {
    vec![
        movable_cell("idle", MovableGesture::Idle),
        movable_cell("dragging", MovableGesture::Drag),
        movable_cell("resizing", MovableGesture::Resize),
        movable_cell("auto-scroll-paused", MovableGesture::ScrollAway),
    ]
}

/// The stacked-band cell: one surface in each of Bands 1, 2 and 3 at once.
///
/// This is the cell that carries finding **G-13** — paint order and escape
/// order are *different* orderings, and a gallery that recorded only one of
/// them would hide the whole finding. The paint stack is by band index, so
/// Band 3's scrim covers Band 2's menu card exactly as it covers Band 0,
/// while the unwind order the registry reports is
/// `Menu(3) → Modal(2) → UtilityOverlay(1) → Workspace(0)`.
///
/// All three surfaces are registered in one real [`SurfaceRegistry`], so
/// `highest_open_band`, `topmost` and the escape ranks are the registry's
/// own answers rather than values this cell wrote down. The Band 2 card's
/// own chrome is composed here from tokens because the menu module owns row
/// styling, not the card's perimeter.
#[must_use]
pub fn zband_cell() -> Cell {
    let palette = Palette::dark();
    let frame = ModalFrame::resolve(&palette);
    let anatomy = MovableAnatomy::resolve(&palette);
    let viewport = viewport();

    let overlay_rect = Rectangle::new(
        Point::new(space::SM, space::SM),
        Size::new(space::XL * 14.0, space::XL * 10.0),
    );
    let menu_rect = Rectangle::new(
        Point::new(space::XL * 6.0, space::XL * 4.0),
        Size::new(space::XL * 9.0, space::XL * 7.0),
    );
    let modal_rect = Rectangle::new(
        Point::new(space::XL * 10.0, space::XL * 6.0),
        Size::new(space::XL * 18.0, space::XL * 12.0),
    );

    let mut registry = SurfaceRegistry::new();
    registry.register(
        SurfaceId::new("zband/utility-overlay"),
        SurfaceKind::MovableCard,
        vec![overlay_rect],
    );
    registry.register(
        SurfaceId::new("zband/menu-card"),
        SurfaceKind::MenuCard,
        vec![menu_rect],
    );
    registry.register(
        SurfaceId::new("zband/modal"),
        SurfaceKind::Modal,
        vec![modal_rect],
    );

    // Paint bottom-up: Band 1 card, Band 2 card, then Band 3's backdrop
    // and card. The scrim is painted *after* the menu card because the
    // Modal band dims everything below it, menu included.
    let layers = vec![
        (
            "band1-overlay-card",
            surface_layer(
                overlay_rect,
                anatomy.card_surface,
                anatomy.card_border,
                anatomy.card_border_width,
                anatomy.card_radius,
            ),
        ),
        (
            "band2-menu-card",
            surface_layer(
                menu_rect,
                palette.surface_raised,
                palette.border_strong,
                shape::BORDER_HAIRLINE,
                shape::RADIUS_CARD,
            ),
        ),
        (
            "band3-scrim",
            surface_layer(viewport, frame.scrim, Color::TRANSPARENT, 0.0, 0.0),
        ),
        (
            "band3-modal-card",
            surface_layer(
                modal_rect,
                frame.surface,
                frame.border,
                frame.border_width,
                frame.radius,
            ),
        ),
    ];

    let resolved = vec![
        StyleEntry::text(
            "paint-order",
            "band1-overlay -> band2-menu -> band3-scrim -> band3-modal",
        ),
        StyleEntry::text(
            "escape-order",
            "Menu(3) -> Modal(2) -> UtilityOverlay(1) -> Workspace(0)",
        ),
        StyleEntry::text(
            "highest-open-band",
            format!("{:?}", registry.highest_open_band()),
        ),
        StyleEntry::text(
            "topmost",
            registry
                .topmost()
                .map(|surface| surface.id().as_str().to_string())
                .unwrap_or_else(|| "none".to_string()),
        ),
        StyleEntry::text(
            "escape-rank.workspace",
            ZBand::Workspace.escape_rank().to_string(),
        ),
        StyleEntry::text(
            "escape-rank.utility-overlay",
            ZBand::UtilityOverlay.escape_rank().to_string(),
        ),
        StyleEntry::text("escape-rank.menu", ZBand::Menu.escape_rank().to_string()),
        StyleEntry::text("escape-rank.modal", ZBand::Modal.escape_rank().to_string()),
    ];

    Cell {
        id: "zband/overlay-menu-modal".to_string(),
        kind: CellKind::Composite,
        subject: "zband",
        spec: "UIF-ZORD-* + UIF-WIN-003",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport, &layers),
        audit: vec![AuditPair {
            channel: "modal.border",
            foreground: frame.border,
            background: frame.surface,
            role: ContrastRole::Chrome,
        }],
        note: Some(
            "G-13: paint order is by band index, escape order is not. Band 3's \
             scrim covers the Band 2 menu card as well as Bands 0 and 1, but \
             Escape unwinds Menu before Modal."
                .to_string(),
        ),
    }
}

/// The movable overlay's trailing action cluster cell (`UIF-MOV-001`).
///
/// The cluster is the title bar's exclusion zone: the part of the bar a
/// drag must *not* start, because `Dock` and `Close ×` live there. The
/// cell asserts the zone, the two `24px` controls inside it, and the
/// `8px` gap between them, each as its own quad, and re-derives the
/// cluster's width from [`ACTION_CONTROL_SIZE`] and [`ACTION_GAP`] so a
/// change to either constant moves the assertion rather than drifting
/// away from it.
fn movable_action_cell(name: &str, gesture: MovableGesture) -> Cell {
    let palette = Palette::dark();
    let anatomy = MovableAnatomy::resolve(&palette);
    let viewport = viewport();

    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let mut storage = MemoryGeometryStorage::new();
    let host = driven_movable_host(gesture, viewport, &mut focus, &mut registry, &mut storage);

    let bar = host.title_bar();
    let cluster = host.trailing_actions();
    let centre = (cluster.height - ACTION_CONTROL_SIZE) / 2.0;
    let dock = Rectangle::new(
        Point::new(cluster.x, cluster.y + centre),
        Size::new(ACTION_CONTROL_SIZE, ACTION_CONTROL_SIZE),
    );
    let close = Rectangle::new(
        Point::new(dock.x + ACTION_CONTROL_SIZE + ACTION_GAP, dock.y),
        Size::new(ACTION_CONTROL_SIZE, ACTION_CONTROL_SIZE),
    );
    let drag_zone = Rectangle::new(
        Point::new(bar.x, bar.y),
        Size::new((cluster.x - bar.x).max(0.0), bar.height),
    );
    let derived_width = ACTION_CONTROL_SIZE * 2.0 + ACTION_GAP;

    let layers = vec![
        (
            "title-bar",
            surface_layer(bar, anatomy.title_bar_surface, Color::TRANSPARENT, 0.0, 0.0),
        ),
        ("drag-zone", ink_box(drag_zone, anatomy.divider)),
        ("action-cluster", ink_box(cluster, anatomy.divider)),
        ("action-dock", ink_box(dock, anatomy.meta_text)),
        ("action-close", ink_box(close, anatomy.meta_text)),
    ];

    let resolved = vec![
        StyleEntry::text("geometry.card", rect_text(host.card())),
        StyleEntry::text("geometry.title-bar", rect_text(bar)),
        StyleEntry::text("geometry.cluster", rect_text(cluster)),
        StyleEntry::text("geometry.dock", rect_text(dock)),
        StyleEntry::text("geometry.close", rect_text(close)),
        StyleEntry::text("geometry.drag-zone", rect_text(drag_zone)),
        StyleEntry::number("action.control-size", ACTION_CONTROL_SIZE),
        StyleEntry::number("action.gap", ACTION_GAP),
        StyleEntry::number("action.derived-width", derived_width),
        StyleEntry::text(
            "action.width-matches-host",
            ((cluster.width - derived_width).abs() < CHANNEL_EPS).to_string(),
        ),
        StyleEntry::text("drag-zone.ends-at-cluster", "true"),
    ];

    let audit = vec![
        AuditPair {
            channel: "title",
            foreground: anatomy.title_text,
            background: anatomy.title_bar_surface,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "action.glyph",
            foreground: anatomy.meta_text,
            background: anatomy.title_bar_surface,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "cluster.divider",
            foreground: anatomy.divider,
            background: anatomy.title_bar_surface,
            role: ContrastRole::Chrome,
        },
    ];

    Cell {
        id: format!("movable-actions/{name}"),
        kind: CellKind::Composite,
        subject: "movable-actions",
        spec: "UIF-MOV-001",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport, &layers),
        audit,
        note: None,
    }
}

/// Every trailing-action cell: the cluster as it opens, after a drag, and
/// after a resize.
#[must_use]
pub fn movable_action_cells() -> Vec<Cell> {
    vec![
        movable_action_cell("idle", MovableGesture::Idle),
        movable_action_cell("dragged", MovableGesture::Drag),
        movable_action_cell("resized", MovableGesture::Resize),
    ]
}

/// The movable-overlay drag tier a cell records, and how a user produces it.
///
/// §3.0.4.2 names this interaction `on_drag_settle`: many `Live` frames,
/// exactly one `Commit`, and a `Cancel` that puts the card back.
pub const DRAG_TIERS: [(&str, &str); 3] = [
    ("live", "one frame of the card being dragged"),
    ("committed", "the release that settles the drag"),
    (
        "cancelled",
        "`Escape` puts the card back where it was grabbed",
    ),
];

/// The `UIF-ATOM-000b` tier cell for one movable-overlay drag outcome.
///
/// Drives a real [`DragGesture`] over real [`MovableGeometry`]. The paint
/// underneath is the card as it opens, unchanged: the tiers are facts
/// about the wire, and the card's own resting rectangle is the honest
/// picture of what the drag started from.
#[must_use]
pub fn drag_tier_cell(tier: &'static str) -> Cell {
    let start = MovableGeometry::new(Point::new(120.0, 90.0), Size::new(680.0, 440.0));
    let moved = MovableGeometry::new(Point::new(260.0, 180.0), Size::new(680.0, 440.0));

    let mut drag = DragGesture::new();
    let mut restored = None;
    match tier {
        "live" => {
            drag.begin(start);
            drag.live(moved).expect("an open drag emits a live tick");
        }
        "committed" => {
            drag.begin(start);
            drag.live(moved).expect("an open drag emits a live tick");
            let commit = drag.commit(moved).expect("the release settles the drag");
            assert_eq!(
                commit.value(),
                &moved,
                "the commit carries where the card landed"
            );
            assert!(drag.commit(moved).is_none(), "exactly one commit per drag");
        }
        "cancelled" => {
            drag.begin(start);
            drag.live(moved).expect("an open drag emits a live tick");
            let cancel = drag.cancel().expect("an open drag cancels");
            restored = Some(*cancel.restored());
            assert_eq!(
                cancel.restored(),
                &start,
                "the revert target is the grab point, not the last live frame"
            );
        }
        other => panic!("unknown drag tier {tier}: {other}"),
    }

    let mut cell = movable_cell("idle", MovableGesture::Idle);
    cell.id = format!("drag-tier/{tier}");
    cell.subject = "drag-tier";
    cell.spec = "UIF-ATOM-000b";
    cell.resolved.extend([
        StyleEntry::text("tier", tier),
        StyleEntry::text("last-tier", drag.last_tier()),
        StyleEntry::text("live-ticks", drag.live_ticks().to_string()),
        StyleEntry::text("committed", drag.committed().to_string()),
        StyleEntry::text("cancelled", drag.cancelled().to_string()),
        StyleEntry::text(
            "restores",
            restored
                .map(|geometry| format!("{},{}", geometry.position.x, geometry.position.y))
                .unwrap_or_else(|| "n/a".to_string()),
        ),
    ]);
    cell.note = Some(format!("produced by: {}", tier_how(&DRAG_TIERS, tier)));
    cell
}

/// Every movable-overlay drag tier cell, in [`DRAG_TIERS`] order.
#[must_use]
pub fn drag_tier_cells() -> Vec<Cell> {
    DRAG_TIERS
        .iter()
        .map(|(tier, _)| drag_tier_cell(tier))
        .collect()
}
