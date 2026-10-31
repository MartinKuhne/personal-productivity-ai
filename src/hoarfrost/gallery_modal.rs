//! The modal and input-dialog cells of the `UIF-VER-004` matrix
//! (`UIF-MOD-001`..`UIF-MOD-003`, `UIF-INP-001`/`003`/`004`,
//! `UIF-ATOM-000b`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit. These cells are built through `gallery`'s own
//! vocabulary and [`gallery::matrix`] calls the producers back, so the
//! matrix is still one list in one fixed order — the split moved the
//! code, not the artifact.
//!
//! # Why one module
//!
//! The four families here are four views of the same two shells, so they
//! share the shell's own vocabulary:
//!
//! * [`modal_cells`] — the [`ModalHost`] frame in each sizing mode
//!   (§4.6).
//! * [`input_dialog_cells`] — the same frame carrying the
//!   [`InputDialogHost`]'s field, footer and validation (§4.5).
//! * [`modal_band_cells`] — the header / banner / footer bands of §4.6.1
//!   cut out of `ModalBands` and measured on their own.
//! * [`input_dialog_row_cells`] — the per-row bodies of §4.5.1.
//!
//! The `000b` tiers for both shells (`DIALOG_BUTTON_TIERS`, `MODAL_TIERS`)
//! are here too: a tier is a fact about one of these two hosts, and
//! putting them anywhere else would split a host from its own outcomes.
//!
//! The tests for these cells are in the gallery's own `gallery_tests.rs`.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix

use crate::binding::ActionPending;
use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, footer_height, gallery_error,
    ink_box, paint_frame, rect_text, surface_layer, tier_how, viewport,
};
use crate::state::BaseState;
use crate::state::VisualState;
use crate::surface::dialog_button::DialogButtonController;
use crate::surface::dialog_button::DialogButtonEmission;
use crate::surface::dialog_button::DialogButtonRole;
use crate::surface::input_dialog::{self, InputDialogHost};
use crate::surface::modal::{self, ModalError, ModalFrame, ModalHost, ModalSizing};
use crate::surface::modal_tier::ModalAction;
use crate::surface::modal_tier::modal_action_state;
use crate::surface::zorder::{SurfaceRegistry, ZBand};
use crate::token::color::Palette;
use crate::token::shape;
use crate::token::space;
use crate::token::typography;
use crate::widget::button::{self, ButtonVariant};
use crate::widget::button_controller::ActivationSource;
use crate::widget::button_widget::Layer;
use crate::widget::button_widget::background_layer as button_background_layer;
use crate::widget::text_input::{self, InputKind};
use iced::Color;
use iced::Point;
use iced::Rectangle;
use iced::Size;

/// The controls every gallery modal carries, in tab order.
fn modal_controls() -> Vec<FocusMember> {
    vec![
        FocusMember::new(FocusId::new("gallery.modal.cancel")),
        FocusMember::new(FocusId::new("gallery.modal.ok")),
    ]
}

/// The modal-host cell for one cleanliness/error combination
/// (`UIF-MOD-001`, `UIF-MOD-002`, `UIF-MOD-003`).
///
/// Opened through [`ModalHost::open`] so the card geometry, the focus
/// group and the Band 3 registration are the real ones. Only the frame's
/// resolved colours and the card rectangle are recorded: `ModalFrame`
/// deliberately carries no sub-rectangles, because the header/banner/footer
/// split needs measured heights and belongs to the renderer.
fn modal_cell(name: &str, dirty: bool, error: Option<ModalError>) -> Cell {
    let palette = Palette::dark();
    let frame = ModalFrame::resolve(&palette);
    let viewport = viewport();

    let mut spec = modal::ModalSpec::new(
        "gallery.modal",
        "Gallery Modal",
        ModalSizing::attributions(),
    )
    .with_controls(modal_controls())
    .with_primary(FocusId::new("gallery.modal.ok"));

    if dirty {
        spec = spec.with_dirty(true);
    }
    if let Some(error) = error.clone() {
        spec = spec.with_error(error);
    }

    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let host = ModalHost::open(spec, viewport, Size::ZERO, &mut focus, &mut registry);

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
    ];

    let mut resolved = vec![
        StyleEntry::colour("frame.surface", frame.surface),
        StyleEntry::colour("frame.border", frame.border),
        StyleEntry::number("frame.border-width", frame.border_width),
        StyleEntry::number("frame.radius", frame.radius),
        StyleEntry::colour("frame.scrim", frame.scrim),
        StyleEntry::colour("frame.divider", frame.divider),
        StyleEntry::number("frame.padding-content", frame.content_padding),
        StyleEntry::number("frame.padding-footer-x", frame.footer_padding_horizontal),
        StyleEntry::number("frame.padding-footer-y", frame.footer_padding_vertical),
        StyleEntry::number("frame.close-target", frame.close_target),
        StyleEntry::text("geometry.card", rect_text(host.card())),
        StyleEntry::text("geometry.scrim", rect_text(host.scrim_bounds())),
        StyleEntry::text("dirty", host.is_dirty().to_string()),
        StyleEntry::text("band", format!("{:?}", ZBand::Modal)),
    ];

    let mut audit = vec![AuditPair {
        channel: "card.border",
        foreground: frame.border,
        background: frame.surface,
        role: ContrastRole::Chrome,
    }];

    if let Some(error) = error.as_ref() {
        resolved.push(StyleEntry::text("error.code", error.code()));
        resolved.push(StyleEntry::text(
            "error.recovery",
            format!("{:?}", error.recovery()),
        ));
        audit.push(AuditPair {
            channel: "error.text",
            foreground: palette.status_danger_text,
            background: frame.surface,
            role: ContrastRole::Text,
        });
    }

    Cell {
        id: format!("modal/{name}"),
        kind: CellKind::Composite,
        subject: "modal",
        spec: "UIF-MOD-001",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport, &layers),
        audit,
        note: None,
    }
}

/// Every modal-host cell: clean, dirty, and dirty-with-error.
#[must_use]
pub fn modal_cells() -> Vec<Cell> {
    let error = ModalError::new("MOD-5001", "Save failed")
        .at(modal::ErrorLocation::Position {
            line: 12,
            column: 4,
        })
        .with_recovery(modal::Recovery::Retry);

    vec![
        modal_cell("clean", false, None),
        modal_cell("dirty", true, None),
        modal_cell("error", true, Some(error)),
    ]
}

/// The dialog footer's two buttons, painted as the atoms they are.
///
/// `UIF-INP-004` fixes the primary's *intent*, not a new button style: a
/// `Danger` intent is the [`ButtonVariant::Danger`] variant and a
/// `Standard` intent is [`ButtonVariant::Primary`]. Painting both through
/// [`button_background_layer`] rather than a dialog-local literal is what
/// keeps the two definitions from drifting apart.
fn dialog_footer_layers(
    layout: &input_dialog::InputLayout,
    intent: input_dialog::ButtonIntent,
    palette: &Palette,
) -> Vec<(&'static str, Layer)> {
    let rest = VisualState {
        base: BaseState::Rest,
        focused: false,
    };
    let primary_variant = if intent.is_destructive() {
        ButtonVariant::Danger
    } else {
        ButtonVariant::Primary
    };

    let mut layers = Vec::new();

    let cancel = button::resolve_style(ButtonVariant::Secondary, rest, palette);
    if let Some(layer) = button_background_layer(layout.cancel, &cancel) {
        layers.push(("footer-cancel", layer));
    }

    let primary = button::resolve_style(primary_variant, rest, palette);
    if let Some(layer) = button_background_layer(layout.ok, &primary) {
        layers.push(("footer-primary", layer));
    }

    layers
}

/// One input-dialog cell: the card, its control, and its footer.
///
/// Opened through [`InputDialogHost::open`] so the anchor solve, the focus
/// trap and the Band 3 registration are the real ones, then driven with
/// [`InputDialogHost::set_value`] to the verdict the cell is about. The
/// box model is recorded whole — including the validation slot — because
/// `UIF-INP-003`'s zero-shift guarantee is precisely that the footer
/// rectangle below does not move whether that slot is empty or full.
fn input_dialog_cell(name: &str, spec: input_dialog::InputDialogSpec, value: Option<&str>) -> Cell {
    let palette = Palette::dark();
    let frame = ModalFrame::resolve(&palette);
    let viewport = viewport();

    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let mut host = InputDialogHost::open(
        spec,
        Point::new(space::XL * 12.0, space::XL * 8.0),
        false,
        viewport,
        &mut focus,
        &mut registry,
    );
    if let Some(value) = value {
        let _ = host.set_value(value.to_string(), &mut focus);
    }

    let layout = *host.layout();
    let invalid = !host.verdict().is_accept();
    let intent = host.intent();
    let is_atom_control = host.control().is_text_entry();

    let mut layers = vec![
        (
            "scrim",
            surface_layer(
                host.scrim_bounds(),
                frame.scrim,
                Color::TRANSPARENT,
                0.0,
                0.0,
            ),
        ),
        (
            "card",
            surface_layer(
                layout.card,
                frame.surface,
                frame.border,
                input_dialog::card_border(),
                frame.radius,
            ),
        ),
    ];

    let mut audit = vec![AuditPair {
        channel: "card.border",
        foreground: frame.border,
        background: frame.surface,
        role: ContrastRole::Chrome,
    }];

    if is_atom_control {
        // The field is the §3.2 atom, painted through the atom's own
        // resolver at the verdict the dialog has reached.
        let state = if invalid {
            VisualState {
                base: BaseState::Invalid,
                focused: true,
            }
        } else {
            VisualState {
                base: BaseState::Rest,
                focused: true,
            }
        };
        let style = text_input::resolve_input_style(InputKind::TextEntry, state, &palette);
        if let Some(layer) = text_input::background_layer(layout.control, &style) {
            layers.push(("control", layer));
        }
        if let Some(layer) = text_input::focus_ring_layer(layout.control, &style) {
            layers.push(("control-focus-ring", layer));
        }

        audit.push(AuditPair {
            channel: "value",
            foreground: style.text,
            background: style.background,
            role: ContrastRole::Text,
        });
        audit.push(AuditPair {
            channel: "placeholder",
            foreground: style.placeholder,
            background: style.background,
            role: ContrastRole::Text,
        });
        if style.border_width > 0.0 {
            audit.push(AuditPair {
                channel: "border",
                foreground: style.border_color,
                background: style.background,
                role: ContrastRole::Chrome,
            });
        }
        if style.focus_ring_width > 0.0 {
            audit.push(AuditPair {
                channel: "ring",
                foreground: style.focus_ring_color,
                background: style.background,
                role: ContrastRole::Chrome,
            });
        }
    } else {
        // A folder list and a bare confirmation are regions, not atoms:
        // there is no per-variant resolver for them, so the cell records
        // the sunken surface they occupy.
        layers.push((
            "control",
            surface_layer(
                layout.control,
                palette.surface_sunken,
                palette.border_subtle,
                shape::BORDER_HAIRLINE,
                frame.radius,
            ),
        ));
        audit.push(AuditPair {
            channel: "control.border",
            foreground: palette.border_subtle,
            background: palette.surface_sunken,
            role: ContrastRole::Chrome,
        });
    }

    layers.extend(dialog_footer_layers(&layout, intent, &palette));

    let mut resolved = vec![
        StyleEntry::text("geometry.card", rect_text(layout.card)),
        StyleEntry::text("geometry.title", rect_text(layout.title)),
        StyleEntry::text("geometry.context", rect_text(layout.context)),
        StyleEntry::text("geometry.control", rect_text(layout.control)),
        StyleEntry::text("geometry.validation-slot", rect_text(layout.validation)),
        StyleEntry::text("geometry.footer", rect_text(layout.footer)),
        StyleEntry::text("geometry.cancel", rect_text(layout.cancel)),
        StyleEntry::text("geometry.ok", rect_text(layout.ok)),
        StyleEntry::number("card.border-width", input_dialog::card_border()),
        StyleEntry::text("control.kind", format!("{:?}", host.control())),
        StyleEntry::text("intent", format!("{intent:?}")),
        StyleEntry::text("verdict", format!("{:?}", host.verdict())),
        StyleEntry::text("ok-enabled", host.ok_is_enabled().to_string()),
        StyleEntry::text("revealed", format!("{:?}", host.revealed())),
        StyleEntry::text("primary-focus", format!("{:?}", host.primary_id())),
        StyleEntry::text("band", format!("{:?}", ZBand::Modal)),
    ];
    if invalid {
        resolved.push(StyleEntry::colour(
            "status.danger_text",
            palette.status_danger_text,
        ));
    }

    Cell {
        id: format!("input-dialog/{name}"),
        kind: CellKind::Composite,
        subject: "input-dialog",
        spec: "UIF-INP-001",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport, &layers),
        audit,
        note: None,
    }
}

/// Every input-dialog cell: the field under each verdict, the folder list,
/// and the destructive confirmation.
#[must_use]
pub fn input_dialog_cells() -> Vec<Cell> {
    vec![
        input_dialog_cell(
            "rename-valid",
            input_dialog::InputDialogSpec::rename("gallery.rename", "notes", "notes.md"),
            Some("renamed.md"),
        ),
        input_dialog_cell(
            "rename-invalid",
            input_dialog::InputDialogSpec::rename("gallery.rename", "notes", "notes.md"),
            Some(""),
        ),
        input_dialog_cell(
            "create-valid",
            input_dialog::InputDialogSpec::create("gallery.create", "New Document", "notes"),
            Some("fresh.md"),
        ),
        input_dialog_cell(
            "move-folder-list",
            input_dialog::InputDialogSpec::move_item(
                "gallery.move",
                "notes.md",
                vec!["inbox".into(), "archive".into()],
            ),
            None,
        ),
        input_dialog_cell(
            "delete-danger",
            input_dialog::InputDialogSpec::delete_confirmation(
                "gallery.delete",
                input_dialog::DeleteTargets::Many(3),
            ),
            None,
        ),
    ]
}

/// The header, banner, body and footer bands of a modal card, composed
/// from the frame's own paddings.
///
/// [`ModalFrame`] deliberately carries no sub-rectangles: the split needs
/// measured heights, and measured heights belong to the renderer. The
/// gallery does not need the renderer's numbers to assert the *regions*,
/// though. Every band here is derived only from paddings and control
/// sizes the frame already exposes, so the cell asserts the band model
/// without introducing a second source of truth:
///
/// * header = `content_padding` + the `close_target` square + `content_padding`
/// * banner = `content_padding` + one body line + `content_padding`
/// * footer = `footer_padding_vertical` × 2 + one standard control
/// * body   = whatever the card has left between the banner and the footer
struct ModalBands {
    header: Rectangle,
    banner: Rectangle,
    body: Rectangle,
    footer: Rectangle,
}

impl ModalBands {
    fn compose(frame: &ModalFrame, card: Rectangle, with_banner: bool) -> Self {
        let inset = frame.border_width;
        let x = card.x + inset;
        let width = (card.width - inset * 2.0).max(0.0);

        let header = Rectangle::new(
            Point::new(x, card.y + inset),
            Size::new(width, frame.content_padding * 2.0 + frame.close_target),
        );
        // A modal with no error has no banner band, and the body takes the
        // space back rather than leaving a gap where the band would be.
        let banner_height = if with_banner {
            frame.content_padding * 2.0 + typography::BODY.line_height
        } else {
            0.0
        };
        let banner = Rectangle::new(
            Point::new(x, header.y + header.height),
            Size::new(width, banner_height),
        );
        let footer = Rectangle::new(
            Point::new(x, card.y + card.height - inset - footer_height(frame)),
            Size::new(width, footer_height(frame)),
        );
        let body_top = banner.y + banner.height;
        let body = Rectangle::new(
            Point::new(x, body_top),
            Size::new(width, (footer.y - body_top).max(0.0)),
        );

        Self {
            header,
            banner,
            body,
            footer,
        }
    }
}

/// The modal band cell: the card's header, banner and footer as
/// separately asserted regions (`UIF-MOD-001`, `UIF-MOD-003`, §4.6.1).
///
/// `INC-21` recorded that the modal cells carried no band split. They do
/// now: each band is its own named quad with its own fill, the two
/// dividers are their own quads, and each band's foreground is audited
/// against the surface it actually sits on rather than against the card
/// as a whole.
fn modal_band_cell(name: &str, with_banner: bool) -> Cell {
    let palette = Palette::dark();
    let frame = ModalFrame::resolve(&palette);
    let viewport = viewport();

    let mut spec = modal::ModalSpec::new(
        "gallery.bands",
        "Gallery Modal Bands",
        ModalSizing::attributions(),
    )
    .with_controls(modal_controls())
    .with_primary(FocusId::new("gallery.modal.ok"));
    if with_banner {
        spec = spec.with_error(gallery_error("MOD-5001", "Save failed"));
    }

    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let host = ModalHost::open(spec, viewport, Size::ZERO, &mut focus, &mut registry);
    let bands = ModalBands::compose(&frame, host.card(), with_banner);

    let divider = Rectangle::new(
        Point::new(
            bands.header.x,
            bands.header.y + bands.header.height - shape::BORDER_HAIRLINE,
        ),
        Size::new(bands.header.width, shape::BORDER_HAIRLINE),
    );
    let footer_rule = Rectangle::new(
        Point::new(bands.footer.x, bands.footer.y),
        Size::new(bands.footer.width, shape::BORDER_HAIRLINE),
    );
    let close_target = Rectangle::new(
        Point::new(
            bands.header.x + bands.header.width - frame.content_padding - frame.close_target,
            bands.header.y + frame.content_padding,
        ),
        Size::new(frame.close_target, frame.close_target),
    );

    let mut layers = vec![
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
            "header-band",
            surface_layer(bands.header, frame.surface, Color::TRANSPARENT, 0.0, 0.0),
        ),
    ];
    if with_banner {
        layers.push((
            "banner-band",
            surface_layer(bands.banner, frame.surface, Color::TRANSPARENT, 0.0, 0.0),
        ));
    }
    layers.extend([
        (
            "body-band",
            surface_layer(bands.body, frame.surface, Color::TRANSPARENT, 0.0, 0.0),
        ),
        (
            "footer-band",
            surface_layer(bands.footer, frame.surface, Color::TRANSPARENT, 0.0, 0.0),
        ),
        (
            "header-divider",
            surface_layer(divider, frame.divider, Color::TRANSPARENT, 0.0, 0.0),
        ),
        (
            "footer-divider",
            surface_layer(footer_rule, frame.divider, Color::TRANSPARENT, 0.0, 0.0),
        ),
        ("close-target", ink_box(close_target, palette.text_muted)),
    ]);

    let mut resolved = vec![
        StyleEntry::text("geometry.card", rect_text(host.card())),
        StyleEntry::text("geometry.header", rect_text(bands.header)),
        StyleEntry::text("geometry.body", rect_text(bands.body)),
        StyleEntry::text("geometry.footer", rect_text(bands.footer)),
        StyleEntry::text("geometry.header-divider", rect_text(divider)),
        StyleEntry::text("geometry.footer-divider", rect_text(footer_rule)),
        StyleEntry::text("geometry.close-target", rect_text(close_target)),
        StyleEntry::text(
            "band.header.composition",
            "content_padding + close_target + content_padding",
        ),
        StyleEntry::text(
            "band.footer.composition",
            "footer_padding_vertical x2 + size.control.md",
        ),
        StyleEntry::text("banner-present", with_banner.to_string()),
    ];

    let mut audit = vec![
        AuditPair {
            channel: "card.border",
            foreground: frame.border,
            background: frame.surface,
            role: ContrastRole::Chrome,
        },
        AuditPair {
            channel: "header.title",
            foreground: palette.text_strong,
            background: frame.surface,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "header.close-glyph",
            foreground: palette.text_muted,
            background: frame.surface,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "footer.status",
            foreground: palette.text_muted,
            background: frame.surface,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "divider.header",
            foreground: frame.divider,
            background: frame.surface,
            role: ContrastRole::Chrome,
        },
        AuditPair {
            channel: "divider.footer",
            foreground: frame.divider,
            background: frame.surface,
            role: ContrastRole::Chrome,
        },
    ];

    if with_banner {
        resolved.push(StyleEntry::text("geometry.banner", rect_text(bands.banner)));
        audit.push(AuditPair {
            channel: "banner.text",
            foreground: palette.status_danger_text,
            background: frame.surface,
            role: ContrastRole::Text,
        });
    }

    Cell {
        id: format!("modal-bands/{name}"),
        kind: CellKind::Composite,
        subject: "modal-bands",
        spec: "UIF-MOD-001",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport, &layers),
        audit,
        note: None,
    }
}

/// Every modal band cell: the card with no banner, and the card showing
/// the `UIF-MOD-003` error banner.
#[must_use]
pub fn modal_band_cells() -> Vec<Cell> {
    vec![
        modal_band_cell("no-banner", false),
        modal_band_cell("banner", true),
    ]
}

/// Layer names for the folder list's rows.
///
/// Layer names are `&'static str` because they are assertion keys, and a
/// computed name would have to be leaked to reach that lifetime. A list
/// capped at [`input_dialog::FOLDER_LIST_MAX_HEIGHT`] shows at most seven
/// `size.control.sm` rows, so eight names cover the widest case with one
/// to spare.
const FOLDER_ROW_LAYERS: [&str; 8] = [
    "folder-row-0",
    "folder-row-1",
    "folder-row-2",
    "folder-row-3",
    "folder-row-4",
    "folder-row-5",
    "folder-row-6",
    "folder-row-7",
];

/// The input dialog cell that paints the box model row by row.
///
/// `INC-21` recorded the dialog's rectangles as resolved values but
/// painted only the card, the control and the two footer buttons. This
/// paints every band between the frame and the buttons as its own quad,
/// and for a folder list paints each `28px` row separately with its own
/// fill, border and label pairing — which is where `UIF-INP-003`'s
/// zero-shift guarantee is actually visible: the rows below the control
/// hold their rectangles whether the list is one row deep or capped.
fn input_dialog_rows_cell(
    name: &str,
    spec: input_dialog::InputDialogSpec,
    value: Option<&str>,
) -> Cell {
    let palette = Palette::dark();
    let frame = ModalFrame::resolve(&palette);
    let viewport = viewport();

    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let mut host = InputDialogHost::open(
        spec,
        Point::new(space::XL * 12.0, space::XL * 8.0),
        false,
        viewport,
        &mut focus,
        &mut registry,
    );
    if let Some(value) = value {
        let _ = host.set_value(value.to_string(), &mut focus);
    }

    let layout = *host.layout();
    let invalid = !host.verdict().is_accept();

    let mut layers = vec![
        (
            "scrim",
            surface_layer(
                host.scrim_bounds(),
                frame.scrim,
                Color::TRANSPARENT,
                0.0,
                0.0,
            ),
        ),
        (
            "card",
            surface_layer(
                layout.card,
                frame.surface,
                frame.border,
                input_dialog::card_border(),
                frame.radius,
            ),
        ),
        (
            "row-title",
            surface_layer(layout.title, frame.surface, Color::TRANSPARENT, 0.0, 0.0),
        ),
        (
            "row-context",
            surface_layer(layout.context, frame.surface, Color::TRANSPARENT, 0.0, 0.0),
        ),
    ];

    let mut audit = vec![
        AuditPair {
            channel: "card.border",
            foreground: frame.border,
            background: frame.surface,
            role: ContrastRole::Chrome,
        },
        AuditPair {
            channel: "row.title",
            foreground: palette.text_strong,
            background: frame.surface,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "row.context",
            foreground: palette.text_muted,
            background: frame.surface,
            role: ContrastRole::Text,
        },
    ];

    let mut resolved = vec![
        StyleEntry::text("control.kind", format!("{:?}", host.control())),
        StyleEntry::text("geometry.title", rect_text(layout.title)),
        StyleEntry::text("geometry.context", rect_text(layout.context)),
        StyleEntry::text("geometry.control", rect_text(layout.control)),
        StyleEntry::text("geometry.validation", rect_text(layout.validation)),
        StyleEntry::text("geometry.footer", rect_text(layout.footer)),
        StyleEntry::number("folder-row.height", input_dialog::FOLDER_ROW_HEIGHT),
        StyleEntry::number(
            "folder-list.max-height",
            input_dialog::FOLDER_LIST_MAX_HEIGHT,
        ),
    ];

    match host.control() {
        input_dialog::InputControl::FolderList { folders, .. } => {
            layers.push((
                "list-well",
                surface_layer(
                    layout.control,
                    palette.surface_sunken,
                    palette.border_subtle,
                    shape::BORDER_HAIRLINE,
                    frame.radius,
                ),
            ));

            let row_height = input_dialog::FOLDER_ROW_HEIGHT;
            let capacity = if row_height > 0.0 {
                (layout.control.height / row_height).floor() as usize
            } else {
                0
            };
            let visible = capacity.min(folders.len()).min(FOLDER_ROW_LAYERS.len());
            let selected = host.selected();

            resolved.push(StyleEntry::text(
                "folder-rows.total",
                folders.len().to_string(),
            ));
            resolved.push(StyleEntry::text("folder-rows.visible", visible.to_string()));
            resolved.push(StyleEntry::text(
                "folder-rows.selected",
                selected.to_string(),
            ));

            for (index, layer_name) in FOLDER_ROW_LAYERS.iter().take(visible).enumerate() {
                let row = Rectangle::new(
                    Point::new(
                        layout.control.x,
                        layout.control.y + index as f32 * row_height,
                    ),
                    Size::new(layout.control.width, row_height),
                );
                let is_selected = index == selected;
                let fill = if is_selected {
                    palette.accent_fill
                } else {
                    palette.surface_sunken
                };
                let border = if is_selected {
                    palette.accent_border
                } else {
                    palette.border_subtle
                };
                let label = if is_selected {
                    palette.text_strong
                } else {
                    palette.text_primary
                };

                layers.push((
                    *layer_name,
                    surface_layer(row, fill, border, shape::BORDER_HAIRLINE, 0.0),
                ));
                resolved.push(StyleEntry::text(
                    "folder-row.bounds",
                    format!("{index} = {}", rect_text(row)),
                ));
                audit.push(AuditPair {
                    channel: if is_selected {
                        "folder-row.selected"
                    } else {
                        "folder-row.label"
                    },
                    foreground: label,
                    background: fill,
                    role: ContrastRole::Text,
                });
            }
        }
        input_dialog::InputControl::TextEntry { .. } => {
            layers.push((
                "row-control",
                surface_layer(
                    layout.control,
                    palette.surface_sunken,
                    palette.border_subtle,
                    shape::BORDER_HAIRLINE,
                    frame.radius,
                ),
            ));
            audit.push(AuditPair {
                channel: "row.control.border",
                foreground: palette.border_subtle,
                background: palette.surface_sunken,
                role: ContrastRole::Chrome,
            });
        }
        input_dialog::InputControl::Confirmation { .. } => {}
    }

    layers.extend(dialog_footer_layers(&layout, host.intent(), &palette));
    resolved.push(StyleEntry::text("intent", format!("{:?}", host.intent())));
    resolved.push(StyleEntry::text("verdict", format!("{:?}", host.verdict())));

    if invalid {
        resolved.push(StyleEntry::colour(
            "validation.foreground",
            palette.status_danger_text,
        ));
        audit.push(AuditPair {
            channel: "row.validation",
            foreground: palette.status_danger_text,
            background: frame.surface,
            role: ContrastRole::Text,
        });
    }

    Cell {
        id: format!("input-dialog-rows/{name}"),
        kind: CellKind::Composite,
        subject: "input-dialog-rows",
        spec: "UIF-INP-001",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, viewport, &layers),
        audit,
        note: None,
    }
}

/// Every input-dialog row cell: the text entry, a short folder list, a
/// folder list driven to its `200px` cap, and a bare confirmation.
#[must_use]
pub fn input_dialog_row_cells() -> Vec<Cell> {
    vec![
        input_dialog_rows_cell(
            "text-entry",
            input_dialog::InputDialogSpec::rename("gallery.rows.rename", "notes", "notes.md"),
            Some("renamed.md"),
        ),
        input_dialog_rows_cell(
            "folder-list",
            input_dialog::InputDialogSpec::move_item(
                "gallery.rows.move",
                "notes.md",
                vec!["inbox".into(), "archive".into()],
            ),
            None,
        ),
        input_dialog_rows_cell(
            "folder-list-capped",
            input_dialog::InputDialogSpec::move_item(
                "gallery.rows.move-capped",
                "notes.md",
                (0..12).map(|index| format!("folder-{index}")).collect(),
            ),
            None,
        ),
        input_dialog_rows_cell(
            "confirmation",
            input_dialog::InputDialogSpec::delete_confirmation(
                "gallery.rows.delete",
                input_dialog::DeleteTargets::Single("notes.md".into()),
            ),
            None,
        ),
    ]
}

/// The dialog footer tier a cell records, and how a user produces it.
pub const DIALOG_BUTTON_TIERS: [(&str, &str); 4] = [
    (
        "primary-committed",
        "press and release `Ok` with a value that validates",
    ),
    ("secondary-cancelled", "press `Cancel`"),
    (
        "primary-cancel-requested",
        "press `Ok` while its action is busy and cancellable",
    ),
    (
        "primary-suppressed",
        "press `Ok` twice before the projection catches up",
    ),
];

/// The `UIF-ATOM-000b` tier cell for one dialog footer outcome.
///
/// Drives a real [`DialogButtonController`]. The paint underneath is the
/// rename dialog's own valid cell, unchanged: `Ok` and `Cancel` do not
/// change colour because a tier fired.
#[must_use]
pub fn dialog_button_tier_cell(tier: &'static str) -> Cell {
    let busy = ActionPending::indeterminate().with_cancellable(true);
    let tracked_idle = ActionPending::idle().with_cancellable(true);

    let mut controller = match tier {
        "secondary-cancelled" => DialogButtonController::new(DialogButtonRole::Secondary),
        _ => DialogButtonController::new(DialogButtonRole::Primary),
    };

    let emission: Option<DialogButtonEmission> = match tier {
        "primary-committed" => {
            controller.observe(ActionPending::idle());
            controller.begin(ActivationSource::Pointer);
            Some(
                controller
                    .commit(ActivationSource::Pointer)
                    .expect("a valid `Ok` commits"),
            )
        }
        "secondary-cancelled" => {
            controller.observe(ActionPending::idle());
            controller.begin(ActivationSource::Pointer);
            Some(
                controller
                    .commit(ActivationSource::Pointer)
                    .expect("`Cancel` always settles"),
            )
        }
        "primary-cancel-requested" => {
            controller.observe(busy);
            Some(
                controller
                    .begin(ActivationSource::Pointer)
                    .expect("a busy cancellable `Ok` requests"),
            )
        }
        "primary-suppressed" => {
            controller.observe(tracked_idle);
            controller
                .commit(ActivationSource::Pointer)
                .expect("the first press dispatches");
            controller.observe(tracked_idle);
            assert!(
                controller.commit(ActivationSource::Pointer).is_none(),
                "the double-activation window must withhold the second press"
            );
            None
        }
        other => panic!("unknown dialog tier {other}"),
    };

    let mut cell = input_dialog_cell(
        "rename-valid",
        input_dialog::InputDialogSpec::rename("gallery.rename", "notes", "notes.md"),
        Some("renamed.md"),
    );
    cell.id = format!("dialog-tier/{tier}");
    cell.subject = "dialog-tier";
    cell.spec = "UIF-ATOM-000b";
    cell.resolved.extend([
        StyleEntry::text("tier", tier),
        StyleEntry::text(
            "emitted",
            emission
                .as_ref()
                .map(DialogButtonEmission::tier)
                .unwrap_or("none"),
        ),
        StyleEntry::text("role", controller.role().slug()),
        StyleEntry::text("settle-tier-for-role", controller.role().settle_tier()),
        StyleEntry::text(
            "dispatched",
            emission
                .as_ref()
                .and_then(DialogButtonEmission::settlement)
                .map(|settlement| settlement.dispatched.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
        ),
        StyleEntry::text("fired-count", controller.fired().to_string()),
        StyleEntry::text("suppressed-count", controller.suppressed().to_string()),
    ]);
    cell.note = Some(format!(
        "produced by: {}",
        tier_how(&DIALOG_BUTTON_TIERS, tier)
    ));
    cell
}

/// Every dialog footer tier cell, in [`DIALOG_BUTTON_TIERS`] order.
#[must_use]
pub fn dialog_button_tier_cells() -> Vec<Cell> {
    DIALOG_BUTTON_TIERS
        .iter()
        .map(|(tier, _)| dialog_button_tier_cell(tier))
        .collect()
}

/// The modal pending-action tier a cell records, and how a user produces it.
pub const MODAL_TIERS: [(&str, &str); 4] = [
    ("idle", "no action under way in the card"),
    ("pending", "the primary call to action is running"),
    (
        "cancel-requested",
        "the running action has been asked to stop",
    ),
    (
        "dismiss-refused",
        "`Escape` against a card whose command is still running",
    ),
];

/// The `UIF-ATOM-000b` tier cell for one modal pending-action state.
///
/// Drives a real [`ModalAction`]. The paint underneath is the modal's own
/// clean card, unchanged: a running command does not repaint the frame, it
/// changes whether the frame may be dismissed.
#[must_use]
pub fn modal_tier_cell(tier: &'static str) -> Cell {
    let busy = ActionPending::indeterminate().with_cancellable(true);
    let mut action = ModalAction::new();

    match tier {
        "idle" => {
            action.observe(ActionPending::idle());
        }
        "pending" => {
            action.observe(ActionPending::indeterminate());
        }
        "cancel-requested" => {
            action.observe(busy);
            action
                .request_cancel()
                .expect("a cancellable action can be stopped");
        }
        "dismiss-refused" => {
            action.observe(busy);
            action.request_cancel();
        }
        other => panic!("unknown modal tier {tier}: {other}"),
    }

    let mut cell = modal_cell("clean", false, None);
    cell.id = format!("modal-tier/{tier}");
    cell.subject = "modal-tier";
    cell.spec = "UIF-ATOM-000b";
    cell.resolved.extend([
        StyleEntry::text("tier", tier),
        StyleEntry::text("state", modal_action_state(&action)),
        StyleEntry::text("busy", action.is_busy().to_string()),
        StyleEntry::text("blocks-dismiss", action.blocks_dismiss().to_string()),
        // The answer `ModalHost::unwind_answer` gives for this state. A
        // running command is the only thing the pending half can refuse
        // with, and it refuses with the same reason the dirty guard uses.
        StyleEntry::text(
            "unwind-answer",
            if action.blocks_dismiss() {
                "Declined(CannotUnwind)"
            } else {
                "Dismissed"
            },
        ),
        StyleEntry::text("cancel-requested", action.cancel_requested().to_string()),
    ]);
    cell.note = Some(format!("produced by: {}", tier_how(&MODAL_TIERS, tier)));
    cell
}

/// Every modal pending-action tier cell, in [`MODAL_TIERS`] order.
#[must_use]
pub fn modal_tier_cells() -> Vec<Cell> {
    MODAL_TIERS
        .iter()
        .map(|(tier, _)| modal_tier_cell(tier))
        .collect()
}
