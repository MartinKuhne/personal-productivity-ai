//! Gallery cells for the trail and the strip (`UIF-COMP-008`, `INC-50`).
//!
//! These live outside `gallery.rs` because the matrix file is already past
//! the module size limit `RUST-053` sets. They also are not the same shape
//! as the cells beside them: neither of these two surfaces *is* a picture.
//! A breadcrumb is a set-membership decision and an overflow toolbar is an
//! eviction order, and both are pure functions of their inputs, so every
//! cell here reports what the function returned rather than a rendering of
//! how it looked.
//!
//! Four claims the cells carry:
//!
//! * **The emphasis ladder.** Ancestor crumbs are `text.muted` and the
//!   terminal crumb is `text.strong`, and the separator is `text.muted`
//!   as well — so the trail gets its emphasis from exactly *one* step, not
//!   two. Every cell prints the measured ratio beside the token that
//!   produced it, including the separator's, which is audited as a text
//!   pairing because a `/` is a glyph rather than a hairline.
//! * **The elision set.** Collapsing hides a prefix of the middle run and
//!   nothing else; the root and the terminal survive at every width. The
//!   cell prints the hidden indices so the *shape* is checkable rather than
//!   merely plausible.
//! * **The eviction order.** Which item leaves first is declared by
//!   [`OverflowRank`], not by where the item happens to sit. The cell
//!   prints the eviction order beside the placement the solver chose, so a
//!   regression to positional eviction shows up as a mismatch between the
//!   two lists.
//! * **The zero shift.** [`strip_height`](overflow_toolbar::strip_height)
//!   takes no argument, so every strip cell reports the same height whether
//!   zero items overflowed or four did. That equality across cells *is*
//!   the structural proof; the matrix only makes it visible.
//!
//! The overflow-row cells are resolved through the real menu engine
//! ([`resolve_row_style`]) rather than a stand-in, because the requirement
//! under test is that an overflowed item keeps its identity, its enabled
//! state and its busy posture *inside the menu* — a hand-picked colour for
//! that row would test the gallery, not the toolbar.
//!
//! Unit tests live in the sibling `gallery_breadcrumb_tests.rs` sidecar.

use crate::binding::ActionPending;
use crate::composite::breadcrumb::{self, BreadcrumbItem, CrumbRole};
use crate::composite::overflow_toolbar::{self, OverflowItem, OverflowRank};
use crate::gallery::{AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, paint_frame};
use crate::state::{BaseState, VisualState};
use crate::surface::menu::view::{RowBase, RowState, resolve_row_style};
use crate::token::color::Palette;
use crate::token::{shape, space};
use crate::widget::button_widget::Layer;
use iced::{Color, Rectangle};

/// The width a crumb-posture cell is drawn at.
const CRUMB_CELL_WIDTH: f32 = 132.0;

/// The width a strip-regime cell is drawn at.
const STRIP_CELL_WIDTH: f32 = 340.0;

/// The width of the swatch standing in for a crumb's label run.
const LABEL_SWATCH_WIDTH: f32 = 56.0;

/// A transparent, hairline-stroked marker.
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

/// A filled layer.
fn fill(bounds: Rectangle, colour: Color) -> Layer {
    Layer {
        bounds,
        fill: colour,
        border: iced::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0_f32.into(),
        },
    }
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
    Rectangle {
        x,
        y,
        width,
        height,
    }
}

/// The five-crumb trail every breadcrumb cell here is built from.
///
/// Wide enough that the middle run has three crumbs to elide, which is the
/// only shape that can distinguish "a prefix went away" from "some
/// arbitrary subset went away".
#[must_use]
pub fn trail() -> Vec<BreadcrumbItem<&'static str>> {
    vec![
        BreadcrumbItem::new("root", "Root", 48.0),
        BreadcrumbItem::new("alpha", "Alpha", 56.0),
        BreadcrumbItem::new("bravo", "Bravo", 64.0),
        BreadcrumbItem::new("charlie", "Charlie", 56.0),
        BreadcrumbItem::new("leaf", "Report", 72.0),
    ]
}

/// The five-item strip every toolbar cell here is built from.
///
/// The ranks are deliberately *not* in positional order: the two
/// [`OverflowRank::DISPOSABLE`] items sit at indices 1 and 3, so the
/// eviction order starts `3, 1` while the declaration order starts `0, 1`.
/// If the solver ever falls back to laying out and evicting left-to-right,
/// the two lists agree and the cell stops being evidence — which is exactly
/// the failure the sidecar pins.
#[must_use]
pub fn strip() -> Vec<OverflowItem<&'static str>> {
    vec![
        OverflowItem::new("cut", "Cut", 56.0, OverflowRank::SECONDARY),
        OverflowItem::new("copy", "Copy", 64.0, OverflowRank::DISPOSABLE),
        OverflowItem::new("paste", "Paste", 60.0, OverflowRank::PRIMARY),
        OverflowItem::new("share", "Share", 52.0, OverflowRank::DISPOSABLE),
        OverflowItem::new("export", "Export", 68.0, OverflowRank::LAST),
    ]
}

/// The `⋯` trigger's busy projection: in flight, and cancellable.
fn busy() -> ActionPending {
    ActionPending::indeterminate().with_cancellable(true)
}

/// A boolean rendered the way the manifest reads one.
fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

// ---------------------------------------------------------------------------
// The emphasis ladder - UIF-COMP-008, the crumb colour ramp
// ---------------------------------------------------------------------------

/// The resting state a posture cell names.
fn visual(base: BaseState, focused: bool) -> VisualState {
    VisualState { base, focused }
}

fn state_slug(base: BaseState, focused: bool) -> &'static str {
    match (base, focused) {
        (BaseState::Rest, false) => "rest",
        (BaseState::Rest, true) => "focused",
        (BaseState::Hovered, _) => "hovered",
        (BaseState::Pressed, _) => "pressed",
        (BaseState::Disabled, _) => "disabled",
        (BaseState::Invalid, _) => "invalid",
    }
}

fn role_slug(role: CrumbRole) -> &'static str {
    match role {
        CrumbRole::Root => "root",
        CrumbRole::Middle => "middle",
        CrumbRole::Terminal => "terminal",
    }
}

/// One crumb posture (`UIF-COMP-008`).
///
/// The claim is the ladder: `Root` and `Middle` resolve to the *same*
/// resting colour, and only `Terminal` steps up. A cell that showed the
/// root brighter than a middle crumb would contradict §3B.8, which draws
/// the ancestor/terminal distinction and no ancestor/ancestor one.
#[must_use]
pub fn crumb_posture_cell(role: CrumbRole, base: BaseState, focused: bool) -> Cell {
    let palette = Palette::dark();
    let state = visual(base, focused);
    let slug = state_slug(base, focused);

    let viewport = rect(0.0, 0.0, CRUMB_CELL_WIDTH, breadcrumb::BAR_HEIGHT);
    let segment = rect(
        space::XS,
        (breadcrumb::BAR_HEIGHT - breadcrumb::SEGMENT_HEIGHT) / 2.0,
        CRUMB_CELL_WIDTH - space::XS * 2.0,
        breadcrumb::SEGMENT_HEIGHT,
    );
    let label = rect(
        segment.x + breadcrumb::SEGMENT_PADDING_X,
        segment.y + 6.0,
        LABEL_SWATCH_WIDTH,
        breadcrumb::SEGMENT_HEIGHT - 12.0,
    );

    let backdrop = breadcrumb::bar_backdrop(&palette, state);
    let crumb = breadcrumb::crumb_colour(&palette, role, state);
    let fill_colour = breadcrumb::crumb_fill(&palette, state);
    // The label sits on the segment, not on the bar. A Ghost segment is
    // transparent at rest, so the bar shows through and the bar *is* the
    // effective background; the moment the ghost paints a fill, that fill
    // is what the text has to clear.
    let effective = if fill_colour.a > 0.0 {
        fill_colour
    } else {
        backdrop
    };

    let layers = vec![
        ("bar", fill(viewport, backdrop)),
        ("segment", fill(segment, fill_colour)),
        ("segment-edge", marker(segment, palette.border_subtle)),
        ("label", fill(label, crumb)),
    ];

    let resolved = vec![
        StyleEntry::text("audit.role", role_slug(role)),
        StyleEntry::text("audit.state", slug),
        StyleEntry::colour("crumb", crumb),
        StyleEntry::colour("crumb_fill", fill_colour),
        StyleEntry::colour("bar_backdrop", backdrop),
        StyleEntry::colour("effective_backdrop", effective),
        StyleEntry::number(
            "crumb.contrast",
            breadcrumb::crumb_contrast(&palette, role, state),
        ),
        StyleEntry::number("bar.height", breadcrumb::BAR_HEIGHT),
        StyleEntry::number("segment.height", breadcrumb::SEGMENT_HEIGHT),
        StyleEntry::number("segment.padding_x", breadcrumb::SEGMENT_PADDING_X),
        StyleEntry::number("segment.gap", breadcrumb::SEGMENT_GAP),
    ];

    let audit = vec![AuditPair {
        channel: "crumb",
        foreground: crumb,
        background: effective,
        role: ContrastRole::Text,
    }];

    Cell {
        id: format!("breadcrumb-crumb/{}/{}", role_slug(role), slug),
        kind: CellKind::Composite,
        subject: "breadcrumb-crumb",
        spec: "UIF-COMP-008",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport, &layers),
        audit,
        note: None,
    }
}

/// The crumb postures the gallery renders.
///
/// Three roles across the four states a crumb can actually be in. `Invalid`
/// is absent because a crumb carries no validation: there is no way for a
/// position in a path to be wrong.
#[must_use]
pub fn crumb_posture_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for role in [CrumbRole::Root, CrumbRole::Middle, CrumbRole::Terminal] {
        for (base, focused) in [
            (BaseState::Rest, false),
            (BaseState::Hovered, false),
            (BaseState::Pressed, false),
            (BaseState::Disabled, false),
        ] {
            cells.push(crumb_posture_cell(role, base, focused));
        }
    }
    cells
}

/// The separator pairing (`UIF-COMP-008`).
///
/// Audited as **text**, not chrome: the separator is a `/` glyph, and a
/// glyph the user has to read answers to rule 1's `4.5:1` rather than rule
/// 2's `3.0:1`. The measured figure is printed against
/// [`breadcrumb::SEPARATOR_CONTRAST_SPEC`], the value §2.5 prints for
/// `text.muted` on `surface.base`, so a palette change that moved the two
/// apart would show as a delta rather than a silent pass.
#[must_use]
pub fn separator_cell() -> Cell {
    let palette = Palette::dark();
    let viewport = rect(0.0, 0.0, CRUMB_CELL_WIDTH, breadcrumb::BAR_HEIGHT);
    let left = rect(
        space::XS,
        (breadcrumb::BAR_HEIGHT - breadcrumb::SEGMENT_HEIGHT) / 2.0,
        44.0,
        breadcrumb::SEGMENT_HEIGHT,
    );
    let right = rect(
        left.x + left.width + breadcrumb::SEGMENT_GAP,
        left.y,
        44.0,
        breadcrumb::SEGMENT_HEIGHT,
    );
    let glyph = rect(
        left.x + left.width,
        left.y,
        breadcrumb::SEGMENT_GAP,
        breadcrumb::SEGMENT_HEIGHT,
    );

    let backdrop = breadcrumb::bar_backdrop(&palette, visual(BaseState::Rest, false));
    let separator = breadcrumb::separator_colour(&palette);
    let measured = breadcrumb::separator_contrast(&palette);

    let layers = vec![
        ("bar", fill(viewport, backdrop)),
        ("segment-left", marker(left, palette.border_subtle)),
        ("segment-right", marker(right, palette.border_subtle)),
        ("separator", fill(glyph, separator)),
    ];

    let resolved = vec![
        StyleEntry::colour("separator", separator),
        StyleEntry::colour("bar_backdrop", backdrop),
        StyleEntry::number("separator.contrast", measured),
        StyleEntry::number(
            "separator.contrast.spec",
            breadcrumb::SEPARATOR_CONTRAST_SPEC,
        ),
        StyleEntry::number("separator.footprint", breadcrumb::SEGMENT_GAP),
    ];

    let audit = vec![AuditPair {
        channel: "separator",
        foreground: separator,
        background: backdrop,
        role: ContrastRole::Text,
    }];

    Cell {
        id: "breadcrumb-separator/rest".to_string(),
        kind: CellKind::Composite,
        subject: "breadcrumb-separator",
        spec: "UIF-COMP-008",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport, &layers),
        audit,
        note: Some(breadcrumb::NOTE_SEPARATOR_TOKEN.to_string()),
    }
}

// ---------------------------------------------------------------------------
// The elision set - UIF-COMP-008, the pure middle-crumb collapse
// ---------------------------------------------------------------------------

/// One collapse regime (`UIF-COMP-008`).
///
/// The evidence is `hidden`, printed as indices. The requirement is not
/// merely "some crumbs got hidden" but that the hidden set is a prefix of
/// the middle run and that the root and the terminal survive; both are
/// readable directly off the printed list.
#[must_use]
pub fn collapse_regime_cell(label: &'static str, available: f32) -> Cell {
    let palette = Palette::dark();
    let items = trail();
    let layout = breadcrumb::collapse(&items, available);

    let viewport = rect(0.0, 0.0, STRIP_CELL_WIDTH, breadcrumb::BAR_HEIGHT);
    let mut layers = vec![("viewport", marker(viewport, palette.border_subtle))];
    for (index, bounds) in breadcrumb::segment_rects(&layout, space::XS, 2.0) {
        let _ = index;
        layers.push(("crumb", fill(bounds, palette.surface_hover)));
        layers.push(("crumb-edge", marker(bounds, palette.border_strong)));
    }
    for bounds in breadcrumb::separator_rects(&layout, space::XS, 2.0) {
        layers.push(("separator", fill(bounds, palette.text_muted)));
    }
    if let Some(bounds) = breadcrumb::trigger_rect(&layout, space::XS, 2.0) {
        layers.push(("trigger", fill(bounds, palette.surface_active)));
        layers.push(("trigger-edge", marker(bounds, palette.border_strong)));
    }

    let hidden: Vec<String> = layout
        .hidden_indices()
        .into_iter()
        .map(|i| i.to_string())
        .collect();
    let inline: Vec<String> = layout
        .plans()
        .iter()
        .filter(|plan| plan.is_inline())
        .map(|plan| plan.index.to_string())
        .collect();

    let resolved = vec![
        StyleEntry::text("audit.regime", label),
        StyleEntry::number("available", available),
        StyleEntry::text("inline", inline.join(" ")),
        StyleEntry::text(
            "hidden",
            if hidden.is_empty() {
                "none".to_string()
            } else {
                hidden.join(" ")
            },
        ),
        StyleEntry::number("inline.count", layout.inline_count() as f32),
        StyleEntry::number("hidden.count", layout.hidden_count() as f32),
        StyleEntry::number("inline.width", layout.inline_width()),
        StyleEntry::number("trigger.width", layout.trigger_width()),
        StyleEntry::number("total.width", layout.total_width()),
        StyleEntry::text("elided", yes_no(layout.is_elided())),
    ];

    Cell {
        id: format!("breadcrumb-collapse/{label}"),
        kind: CellKind::Composite,
        subject: "breadcrumb-collapse",
        spec: "UIF-COMP-008",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport, &layers),
        audit: vec![AuditPair {
            channel: "crumb_edge",
            foreground: palette.border_strong,
            background: palette.surface_base,
            role: ContrastRole::Chrome,
        }],
        note: Some(breadcrumb::GAP_COLLAPSE_ORDER.to_string()),
    }
}

/// The collapse regimes the gallery renders.
///
/// `fits` needs no trigger at all; `elide-1` through `elide-3` each hide
/// one more crumb from the front of the middle run; `below-floor` is the
/// width the solver cannot satisfy and must report rather than loop on.
#[must_use]
pub fn collapse_regime_cells() -> Vec<Cell> {
    vec![
        collapse_regime_cell("fits", 320.0),
        collapse_regime_cell("elide-1", 290.0),
        collapse_regime_cell("elide-2", 230.0),
        collapse_regime_cell("elide-3", 160.0),
        collapse_regime_cell("below-floor", 120.0),
    ]
}

// ---------------------------------------------------------------------------
// The trigger - UIF-COMP-008, the `...` affordance
// ---------------------------------------------------------------------------

/// One `⋯` trigger posture (`UIF-COMP-008`).
///
/// The glyph is chrome: it is an affordance the user aims at rather than
/// copy to read, so it answers to rule 2's `3.0:1`. The measured ratio is
/// printed against the ghost column that produced it.
#[must_use]
pub fn trigger_posture_cell(base: BaseState) -> Cell {
    let palette = Palette::dark();
    let state = visual(base, false);
    let slug = state_slug(base, false);

    let viewport = rect(0.0, 0.0, CRUMB_CELL_WIDTH, breadcrumb::BAR_HEIGHT);
    let button = rect(
        (CRUMB_CELL_WIDTH - overflow_toolbar::TRIGGER_SIZE) / 2.0,
        (breadcrumb::BAR_HEIGHT - overflow_toolbar::TRIGGER_SIZE) / 2.0,
        overflow_toolbar::TRIGGER_SIZE,
        overflow_toolbar::TRIGGER_SIZE,
    );
    let glyph = rect(
        button.x + 8.0,
        button.y + button.height / 2.0 - 3.0,
        button.width - 16.0,
        6.0,
    );

    let backdrop = overflow_toolbar::trigger_backdrop(&palette, state);
    let glyph_colour = overflow_toolbar::trigger_glyph_colour(&palette, state);

    let layers = vec![
        ("bar", fill(viewport, palette.surface_base)),
        (
            "trigger",
            fill(button, overflow_toolbar::trigger_fill(&palette, state)),
        ),
        ("trigger-edge", marker(button, palette.border_strong)),
        ("glyph", fill(glyph, glyph_colour)),
    ];

    let resolved = vec![
        StyleEntry::text("audit.state", slug),
        StyleEntry::colour("glyph", glyph_colour),
        StyleEntry::colour(
            "trigger_fill",
            overflow_toolbar::trigger_fill(&palette, state),
        ),
        StyleEntry::colour("backdrop", backdrop),
        StyleEntry::number(
            "glyph.contrast",
            overflow_toolbar::trigger_contrast(&palette, state),
        ),
        StyleEntry::number("trigger.size", overflow_toolbar::TRIGGER_SIZE),
        StyleEntry::number("trigger.gap", overflow_toolbar::TRIGGER_GAP),
        StyleEntry::number("trigger.reserve", overflow_toolbar::TRIGGER_RESERVE),
    ];

    let audit = vec![AuditPair {
        channel: "glyph",
        foreground: glyph_colour,
        background: backdrop,
        role: ContrastRole::Chrome,
    }];

    Cell {
        id: format!("overflow-trigger/{slug}"),
        kind: CellKind::Composite,
        subject: "overflow-trigger",
        spec: "UIF-COMP-008",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport, &layers),
        audit,
        note: None,
    }
}

/// The trigger postures the gallery renders.
#[must_use]
pub fn trigger_posture_cells() -> Vec<Cell> {
    [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Disabled,
    ]
    .iter()
    .map(|&base| trigger_posture_cell(base))
    .collect()
}

// ---------------------------------------------------------------------------
// The eviction order - UIF-COMP-008, declared priority
// ---------------------------------------------------------------------------

/// One strip regime (`UIF-COMP-008`).
///
/// Two numbers carry the whole requirement. `evicts` is the order the
/// *declaration* produces; `overflow` is the set the solver actually put
/// behind the trigger. The second must always be a prefix of the first —
/// that is what makes widening monotone — and neither may be derived from
/// layout position.
///
/// `strip.height` is the same constant in every cell regardless of
/// `inline.count` or `overflow.count`, which is the 0px-shift claim made
/// visible: the solver cannot trade vertical space for horizontal relief
/// because the height is not a function of anything it decides.
#[must_use]
pub fn strip_regime_cell(label: &'static str, available: f32) -> Cell {
    let palette = Palette::dark();
    let items = strip();
    let layout = overflow_toolbar::fit(&items, available);
    let order: Vec<String> = overflow_toolbar::eviction_order(&items)
        .into_iter()
        .map(|index| index.to_string())
        .collect();
    let overflow: Vec<String> = layout
        .overflow_indices()
        .into_iter()
        .map(|index| index.to_string())
        .collect();
    let inline: Vec<String> = layout
        .plans()
        .iter()
        .filter(|plan| plan.is_inline())
        .map(|plan| plan.index.to_string())
        .collect();

    let viewport = rect(0.0, 0.0, STRIP_CELL_WIDTH, overflow_toolbar::STRIP_HEIGHT);
    let mut layers = vec![("viewport", marker(viewport, palette.border_subtle))];
    for (index, bounds) in overflow_toolbar::item_rects(&layout, space::XS, 2.0) {
        let _ = index;
        layers.push(("item", fill(bounds, palette.surface_hover)));
        layers.push(("item-edge", marker(bounds, palette.border_strong)));
    }
    if let Some(bounds) = overflow_toolbar::trigger_rect(&layout, space::XS, 2.0) {
        layers.push(("trigger", fill(bounds, palette.surface_active)));
        layers.push(("trigger-edge", marker(bounds, palette.border_strong)));
    }

    let resolved = vec![
        StyleEntry::text("audit.regime", label),
        StyleEntry::number("available", available),
        StyleEntry::text("evicts", order.join(" ")),
        StyleEntry::text("inline", inline.join(" ")),
        StyleEntry::text(
            "overflow",
            if overflow.is_empty() {
                "none".to_string()
            } else {
                overflow.join(" ")
            },
        ),
        StyleEntry::number("inline.count", layout.inline_count() as f32),
        StyleEntry::number("overflow.count", layout.overflow_count() as f32),
        StyleEntry::number("inline.width", layout.inline_width()),
        StyleEntry::number("trigger.width", layout.trigger_width()),
        StyleEntry::number("strip.height", overflow_toolbar::strip_height()),
    ];

    Cell {
        id: format!("overflow-strip/{label}"),
        kind: CellKind::Composite,
        subject: "overflow-strip",
        spec: "UIF-COMP-008",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport, &layers),
        audit: vec![AuditPair {
            channel: "item_edge",
            foreground: palette.border_strong,
            background: palette.surface_base,
            role: ContrastRole::Chrome,
        }],
        note: Some(overflow_toolbar::TIE_BREAK.to_string()),
    }
}

/// The strip regimes the gallery renders.
///
/// `fits` takes the whole strip inline with no trigger; each later regime
/// evicts exactly one more item down the declared order, down to the last
/// surviving inline item beside the trigger.
#[must_use]
pub fn strip_regime_cells() -> Vec<Cell> {
    vec![
        strip_regime_cell("fits", 320.0),
        strip_regime_cell("evict-1", 300.0),
        strip_regime_cell("evict-2", 235.0),
        strip_regime_cell("evict-3", 160.0),
        strip_regime_cell("last-standing", 95.0),
    ]
}

// ---------------------------------------------------------------------------
// The overflow row - UIF-COMP-008, state survives eviction
// ---------------------------------------------------------------------------

/// One overflowed item's row inside the menu (`UIF-COMP-008`).
///
/// The row is resolved by the real menu engine over the real row the
/// toolbar's own [`overflow_menu`](overflow_toolbar::overflow_menu) built,
/// so what is measured here is the pairing the user would actually see.
///
/// The busy row is the interesting one. It is **not** a disabled row: a
/// disabled row is dropped from the menu's focus roster entirely, which
/// would make an in-flight action unreachable by keyboard. A busy row
/// keeps the enabled row's colour and its focus stop, and the refusal to
/// re-fire it belongs to the action engine. The cell prints that
/// distinction rather than letting it be inferred from a shade of grey.
#[must_use]
pub fn overflow_row_cell(label: &'static str, enabled: bool, pending: ActionPending) -> Cell {
    let palette = Palette::dark();
    let mut items = strip();
    items[1] = items[1].clone().with_enabled(enabled).with_pending(pending);
    let layout = overflow_toolbar::fit(&items, 235.0);
    let menu = overflow_toolbar::overflow_menu(&items, &layout);
    let row = menu
        .rows()
        .first()
        .expect("the regime must overflow at least one item");

    let base = if enabled {
        RowBase::Hovered
    } else {
        RowBase::Disabled
    };
    let state = RowState {
        base,
        focused: enabled,
    };
    let style = resolve_row_style(row, state, &palette);

    let viewport = rect(0.0, 0.0, STRIP_CELL_WIDTH, 28.0);
    let row_bounds = rect(0.0, 0.0, STRIP_CELL_WIDTH, 28.0);
    let label_swatch = rect(space::MD, 10.0, 96.0, 8.0);

    let menu_fill = if enabled {
        style.background
    } else {
        palette.surface_overlay
    };

    let mut layers = vec![
        ("menu", fill(viewport, menu_fill)),
        ("row", fill(row_bounds, style.background)),
        ("label", fill(label_swatch, style.label)),
    ];
    if let Some(outline) = &style.focus_outline {
        layers.push(("focus", marker(row_bounds, outline.color)));
    }

    let resolved = vec![
        StyleEntry::text("audit.posture", label),
        StyleEntry::text("enabled", yes_no(enabled)),
        StyleEntry::text(
            "busy",
            yes_no(overflow_toolbar::item_gates_action(&items[1])),
        ),
        StyleEntry::text("navigable", yes_no(enabled)),
        StyleEntry::colour("label", style.label),
        StyleEntry::colour("row_fill", style.background),
        StyleEntry::colour("menu_fill", menu_fill),
        StyleEntry::number(
            "label.contrast",
            overflow_toolbar::overflow_row_contrast(style.label, menu_fill),
        ),
        StyleEntry::number("row.height", 28.0),
    ];

    let audit = vec![AuditPair {
        channel: "overflow_row_label",
        foreground: style.label,
        background: menu_fill,
        role: ContrastRole::Text,
    }];

    Cell {
        id: format!("overflow-row/{label}"),
        kind: CellKind::Composite,
        subject: "overflow-row",
        spec: "UIF-COMP-008",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport, &layers),
        audit,
        note: None,
    }
}

/// The overflow-row postures the gallery renders.
///
/// Enabled at rest, disabled, and busy-but-still-reachable. The third is
/// the one that would be silently wrong if busy were modelled as disabled.
#[must_use]
pub fn overflow_row_cells() -> Vec<Cell> {
    vec![
        overflow_row_cell("enabled", true, ActionPending::idle()),
        overflow_row_cell("disabled", false, ActionPending::idle()),
        overflow_row_cell("busy", true, busy()),
    ]
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

/// Every trail-and-strip cell.
#[must_use]
pub fn breadcrumb_cells() -> Vec<Cell> {
    let mut cells = crumb_posture_cells();
    cells.push(separator_cell());
    cells.extend(collapse_regime_cells());
    cells.extend(trigger_posture_cells());
    cells.extend(strip_regime_cells());
    cells.extend(overflow_row_cells());
    cells
}

#[cfg(test)]
#[path = "gallery_breadcrumb_tests.rs"]
mod gallery_breadcrumb_tests;
