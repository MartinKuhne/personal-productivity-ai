//! The inverted editor palette cells of the `UIF-VER-004` matrix
//! (`UIF-TOK-005`, §2.5 `color.editor.inverted.*`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit. These cells are built through `gallery`'s own
//! vocabulary and [`gallery::matrix`] calls [`editor_palette_cells`]
//! back, so the matrix is still one list in one fixed order — the split
//! moved the code, not the artifact.
//!
//! # Why this is its own concern
//!
//! The editor is the one surface in this crate outside the dark elevation
//! ladder, and its three tokens are not `surface.*` members at all. A
//! cell here therefore answers a question no other gallery cell asks:
//! what does a token *name* when the same channels mean the opposite
//! thing in another world? The canvas is the very same
//! `RGB(255,255,255)` the standard table calls `text.strong`, so these
//! cells are named through [`Namer::InvertedEditor`] and measured
//! against the editor canvas rather than against the ladder.
//!
//! Each cell is painted on a real editor modal sized by
//! [`ModalSizing::editor`], so the inverted surface is shown at the
//! geometry the host actually gives it.
//!
//! The tests for these cells are in the gallery's own `gallery_tests.rs`.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix

use crate::focus::FocusState;
use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, gallery_error, ink_box,
    paint_frame, rect_text, surface_layer, viewport,
};
use crate::surface::modal::{self, ModalFrame, ModalHost, ModalSizing};
use crate::surface::zorder::SurfaceRegistry;
use crate::token::color::{EditorPalette, Palette};
use crate::token::contrast;
use crate::token::shape;
use crate::token::space;
use crate::token::typography;
use crate::widget::button_widget::Layer;
use iced::Color;
use iced::Point;
use iced::Rectangle;
use iced::Size;

/// The inverted editor cell for one foreground token against its own
/// canvas (`UIF-TOK-005`).
///
/// The editor is the one surface in this crate outside the dark elevation
/// ladder: raw editing on white against reading in the dark. Its three
/// tokens only ever appear together, so the cell measures them against
/// the editor canvas rather than against the `surface.*` ladder, and
/// names them through [`Namer::InvertedEditor`] — the standard table
/// would call the canvas `text.strong`, because it is the very same
/// `RGB(255,255,255)`.
///
/// The canvas is the body region of a real editor modal, sized by
/// [`ModalSizing::editor`], so the cell shows the inverted surface at
/// the geometry the host actually gives it.
///
/// The ink quad is a text run's **ink box**, stroked in the foreground
/// token. A glyph is drawn by a text pass rather than a `fill_quad`, so
/// the box is the most the recorded quad list can honestly say about a
/// glyph: where its ink lands, and in which token. The legibility claim
/// is carried by the audit pair, which is measured against the canvas
/// the run sits on.
fn editor_ink_cell(
    name: &str,
    channel: &'static str,
    run: &str,
    foreground: Color,
    spec_figure: &'static str,
) -> Cell {
    let editor = EditorPalette::inverted();
    let standard = Palette::dark();
    let namer = Namer::InvertedEditor;
    let viewport = viewport();

    let mut spec = modal::ModalSpec::new(
        "gallery.editor",
        "Inline Document Editor",
        ModalSizing::editor(),
    );
    if channel == "error.text" {
        spec = spec.with_error(gallery_error("ED-5001", "Save failed"));
    }

    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let host = ModalHost::open(spec, viewport, Size::ZERO, &mut focus, &mut registry);

    let frame = ModalFrame::resolve(&standard);
    let inset = frame.border_width + frame.content_padding;
    let canvas = Rectangle::new(
        Point::new(host.card().x + inset, host.card().y + inset),
        Size::new(
            (host.card().width - inset * 2.0).max(0.0),
            (host.card().height - inset * 2.0).max(0.0),
        ),
    );
    let ink = Rectangle::new(
        Point::new(canvas.x + space::MD, canvas.y + space::MD),
        Size::new(
            (canvas.width - space::MD * 2.0).max(0.0),
            typography::BODY.line_height,
        ),
    );

    let layers: Vec<(&'static str, Layer)> = vec![
        (
            "scrim",
            surface_layer(viewport, frame.scrim, Color::TRANSPARENT, 0.0, 0.0),
        ),
        (
            "card",
            surface_layer(
                host.card(),
                frame.surface,
                frame.border,
                frame.border_width,
                frame.radius,
            ),
        ),
        (
            "editor-canvas",
            surface_layer(
                canvas,
                editor.background,
                standard.border_subtle,
                shape::BORDER_HAIRLINE,
                0.0,
            ),
        ),
        ("ink-run", ink_box(ink, foreground)),
    ];

    let computed = contrast::contrast_ratio(foreground, editor.background);

    let resolved = vec![
        StyleEntry::colour_as(namer, "canvas.fill", editor.background),
        StyleEntry::colour_as(namer, "ink.foreground", foreground),
        StyleEntry::text("ink.run", run),
        StyleEntry::text("geometry.card", rect_text(host.card())),
        StyleEntry::text("geometry.canvas", rect_text(canvas)),
        StyleEntry::text("geometry.ink-box", rect_text(ink)),
        StyleEntry::number("contrast.computed", computed),
        StyleEntry::text("contrast.spec-figure", spec_figure),
    ];

    let audit = vec![
        AuditPair {
            channel,
            foreground,
            background: editor.background,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "canvas.perimeter",
            foreground: standard.border_subtle,
            background: editor.background,
            role: ContrastRole::Chrome,
        },
    ];

    Cell {
        id: format!("editor-inverted/{name}"),
        kind: CellKind::Composite,
        subject: "editor",
        spec: "UIF-TOK-005",
        namer,
        resolved,
        frame: paint_frame(namer, viewport, &layers),
        audit,
        note: None,
    }
}

/// Every inverted-editor cell: the buffer run, and the error run on the
/// white canvas.
#[must_use]
pub fn editor_palette_cells() -> Vec<Cell> {
    let editor = EditorPalette::inverted();
    vec![
        editor_ink_cell(
            "rest",
            "buffer.text",
            "document buffer text",
            editor.text,
            "21.00:1 (spec 2.5)",
        ),
        editor_ink_cell(
            "error",
            "error.text",
            "validation and save-error text",
            editor.error,
            "7.00:1 (spec 2.5)",
        ),
    ]
}
