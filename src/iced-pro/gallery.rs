//! The deterministic `(Variant × State)` gallery (`UIF-VER-004`).
//!
//! §6's fourth gate asks for deterministic offscreen visual snapshots
//! verifying every atomic widget across `(Variant × State)` … and every
//! composite surface. This module is the **enumeration** half of that: it
//! walks every variant and every state the crate actually owns, resolves
//! each cell through the same pure resolvers the widgets paint from, and
//! records what each cell would draw.
//!
//! # What a snapshot asserts, and why it is not a picture
//!
//! A cell's snapshot is its **resolved style values** — every fill, border,
//! text colour, ring colour and geometry the cell paints, each named by the
//! token it came from — plus the ordered quad list its frame drew.
//!
//! That is the assertion target rather than a rasterised image for three
//! reasons. The resolved values are *what compliance is made of*: every
//! `UIF-TOK-006` rule, every `UIF-PRIN-003` zero-shift guarantee and every
//! §3.1 / §4.9.2 matrix cell is a statement about a resolved colour or a
//! resolved rectangle, and nothing a picture contributes can be asserted
//! without first being re-expressed as one of those. They are textual, so a
//! regression reads as a diff naming the token that moved rather than as a
//! pixel difference between two bitmaps. And they are cheap enough to keep
//! in version control, which a golden-image corpus is not: a frozen PNG pins
//! every glyph, antialiasing kernel and rasteriser version in the crate, so
//! it fails on unrelated upgrades and gets regenerated until it stops
//! meaning anything.
//!
//! The frame is still a **real draw pass**, not a table copy. Each cell's
//! [`Layer`]s are pushed through
//! [`renderer::Renderer::fill_quad`] on [`FrameRecorder`] — the same
//! protocol call a widget's `draw` issues — so a cell records the quads the
//! renderer was actually handed, in the order it was handed them.
//!
//! # Determinism is the contract
//!
//! Nothing here reads a clock, a random source, a filesystem path or a
//! hash-ordered collection. Every list is built by walking a fixed
//! `const` array in a fixed order, so two calls to [`matrix`] produce
//! byte-identical output — and the sidecar asserts that rather than
//! asserting the author's confidence that it holds.
//!
//! # Scope honesty (`D-2`)
//!
//! The spec's composite list names `Navbar`, `Panes` and `Tabs`. Their
//! *contents* are app-owned and are not in this crate, and there is no
//! framework-owned frame for any of them either — no navbar module, no
//! splitter, no tab strip. Rather than silently dropping a spec surface or
//! faking app content inside one, the gallery emits an explicit
//! [`CellKind::NotOwned`] cell that names what the framework *does*
//! contribute (tokens and atoms the host composes them from) and what it
//! does not. That is what lets the compliance audit tell "not applicable
//! under `D-2`" apart from "missing".
//!
//! Unit tests live in the sibling `gallery_tests.rs` sidecar.

use crate::binding::ActionPending;
use crate::binding::Live;
use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::geometry::clamp::ResizeHandle;
use crate::state::BaseState;
use crate::state::InteractionInput;
use crate::state::VisualState;
use crate::surface::dialog_button::DialogButtonController;
use crate::surface::dialog_button::DialogButtonEmission;
use crate::surface::dialog_button::DialogButtonRole;
use crate::surface::drag_tier::DragGesture;
use crate::surface::input_dialog::{self, InputDialogHost};
use crate::surface::menu::model::ActivationEffect;
use crate::surface::menu::model::Menu;
use crate::surface::menu::model::RadioGroupId;
use crate::surface::menu::model::Row;
use crate::surface::menu::row_action::RowActionEngine;
use crate::surface::menu::row_action::RowEmission;
use crate::surface::menu::view as menu_view;
use crate::surface::modal::{self, ModalError, ModalFrame, ModalHost, ModalSizing};
use crate::surface::modal_tier::ModalAction;
use crate::surface::modal_tier::modal_action_state;
use crate::surface::movable::{
    ACTION_CONTROL_SIZE, ACTION_GAP, AutoScrollSetting, MemoryGeometryStorage, MovableAnatomy,
    MovableGeometry, MovableHost, MovableSpec, PointerPhase, ScrollGeometry, SurfaceTarget,
};
use crate::surface::zorder::{SurfaceId, SurfaceKind, SurfaceRegistry, ZBand};
use crate::token::color::{EditorPalette, Palette};
use crate::token::contrast;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::token::typography;
use crate::widget::button::{self, ButtonVariant};
use crate::widget::button_controller::ActivationSource;
use crate::widget::button_controller::ButtonController;
use crate::widget::button_controller::ButtonEmission;
use crate::widget::button_model::ButtonDensity;
use crate::widget::button_model::ButtonModel;
use crate::widget::button_model::SPINNER_SLOT;
use crate::widget::button_widget::Layer;
use crate::widget::button_widget::background_layer as button_background_layer;
use crate::widget::button_widget::focus_ring_layer as button_focus_ring_layer;
use crate::widget::text_input::{self, InputKind, SlotContent};
use crate::widget::text_input_controller::TextInputController;
use crate::widget::text_input_controller::TextInputEmission;
use crate::widget::text_input_model::TextInputModel;

use iced::Background;
use iced::Border;
use iced::Color;
use iced::Point;
use iced::Rectangle;
use iced::Size;
use iced::Transformation;
use iced::advanced::image;
use iced::advanced::renderer;
use iced::advanced::renderer::Renderer as _;

use std::cell::RefCell;

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

/// Where a gallery cell comes from.
///
/// The three cases are deliberately distinct rather than collapsed into a
/// boolean. A `NotOwned` cell is a *finding*, not an absence: it says the
/// spec named this surface and `D-2` put its contents outside the crate.
/// Lumping it with the rendered cells would make the audit unable to tell a
/// deliberate scope call from a dropped requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CellKind {
    /// A single-control atom resolved across its state matrix.
    Atom,
    /// A whole composite surface resolved through its host.
    Composite,
    /// A spec-named surface this crate does not own under decision **D-2**.
    NotOwned,
}

/// The contrast role a foreground/background pairing is measured against.
///
/// Mirrors the two rules of `UIF-TOK-006` rather than inventing a third:
/// [`ContrastRole::Text`] is rule 1 (`4.5:1`), [`ContrastRole::Chrome`] is
/// rule 2 (`3.0:1`). The measurement itself belongs to
/// [`crate::token::contrast`]; this only says which rule a channel answers
/// to so the sidecar can gate it with the right threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContrastRole {
    /// Normal text — `UIF-TOK-006` rule 1.
    Text,
    /// Active chrome: borders, rings, indicators — rule 2.
    Chrome,
}

/// One resolved visual property of a cell.
///
/// Typed rather than a flat string so the audit can pull the [`Color`] out
/// of a [`StyleValue::Colour`] without parsing, while the manifest still
/// renders every kind through [`StyleValue::display`].
#[derive(Clone, Debug, PartialEq)]
pub enum StyleValue {
    /// A colour, named by the token it resolved from.
    Colour {
        /// The token name, e.g. `status.danger_fill @ 85%`.
        token: String,
        /// The resolved colour itself.
        colour: Color,
    },
    /// A length, weight or opacity.
    Number(f32),
    /// A non-numeric resolved fact: a slot's content, a cursor, a flag.
    Text(String),
}

impl StyleValue {
    /// The value as the manifest renders it.
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Colour { token, colour } => format!("{token} ({})", rgb8(*colour)),
            Self::Number(value) => fixed(*value),
            Self::Text(text) => text.clone(),
        }
    }

    /// The [`Color`] behind a colour entry.
    #[must_use]
    pub const fn colour(&self) -> Option<Color> {
        match self {
            Self::Colour { colour, .. } => Some(*colour),
            Self::Number(_) | Self::Text(_) => None,
        }
    }
}

/// A named resolved property inside a cell.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleEntry {
    /// The channel this entry resolves — `background`, `focus_ring`, …
    pub channel: &'static str,
    /// The resolved value.
    pub value: StyleValue,
}

impl StyleEntry {
    /// A colour channel in the standard dark palette.
    #[must_use]
    pub fn colour(channel: &'static str, colour: Color) -> Self {
        Self::colour_as(Namer::Standard, channel, colour)
    }

    /// A colour channel named against a specific palette world.
    ///
    /// The editor cells need this: their canvas is the same channels as
    /// `text.strong`, so only the cell's own [`Namer`] can say which of
    /// the two names the value actually carries.
    #[must_use]
    pub fn colour_as(namer: Namer, channel: &'static str, colour: Color) -> Self {
        Self {
            channel,
            value: StyleValue::Colour {
                token: namer.name(colour),
                colour,
            },
        }
    }

    /// A numeric channel.
    #[must_use]
    pub fn number(channel: &'static str, value: f32) -> Self {
        Self {
            channel,
            value: StyleValue::Number(value),
        }
    }

    /// A textual channel.
    #[must_use]
    pub fn text(channel: &'static str, value: impl Into<String>) -> Self {
        Self {
            channel,
            value: StyleValue::Text(value.into()),
        }
    }
}

/// One foreground/background pairing a cell actually paints, tagged with the
/// rule it must satisfy.
///
/// The gallery records these so the contrast gate can run over *exactly*
/// what the matrix emits — no more, so the gate cannot drift ahead of the
/// gallery, and no less, so a cell cannot hide an unreadable pairing.
#[derive(Clone, Debug, PartialEq)]
pub struct AuditPair {
    /// The channel being measured.
    pub channel: &'static str,
    /// The foreground it paints.
    pub foreground: Color,
    /// The background it paints onto.
    pub background: Color,
    /// Which `UIF-TOK-006` rule the channel answers to.
    pub role: ContrastRole,
}

/// One quad a cell's frame drew.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameQuad {
    /// The layer name the painter gave it.
    pub layer: String,
    /// The rectangle it covered — `x, y, width, height`.
    pub bounds: [f32; 4],
    /// What was painted inside it.
    pub fill: String,
    /// The stroke drawn on its inside edge.
    pub border: String,
    /// The stroke weight.
    pub border_width: f32,
    /// The corner radius.
    pub radius: f32,
}

/// One cell's rendered offscreen frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The viewport the frame was drawn into.
    pub viewport: [f32; 4],
    /// Every quad the renderer was handed, bottom of the stack first.
    pub quads: Vec<FrameQuad>,
}

/// One cell of the `UIF-VER-004` matrix.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// The stable, human-readable cell id, e.g. `button/danger/pressed+focused`.
    pub id: String,
    /// Whether this is an atom, a composite, or a surface the crate does not
    /// own.
    pub kind: CellKind,
    /// The subject slug — `button`, `input`, `menu`, `modal`, …
    pub subject: &'static str,
    /// The spec requirement the cell answers to.
    pub spec: &'static str,
    /// The palette world this cell's colours are named against.
    ///
    /// Part of the cell rather than a parameter of [`token_name`] because
    /// naming is by value, and two palettes in this crate share a channel
    /// set: the inverted editor's canvas is `RGB(255,255,255)`, the same
    /// channels the standard palette calls `text.strong`. A cell that did
    /// not say which world it was resolved in would have its editor
    /// background reported as a text token - a wrong assertion that still
    /// passes the unmapped-colour gate.
    pub namer: Namer,
    /// Every resolved style value, in a fixed order.
    pub resolved: Vec<StyleEntry>,
    /// The frame this cell drew. Empty for a [`CellKind::NotOwned`] cell,
    /// which has nothing framework-owned to paint.
    pub frame: Frame,
    /// The contrast pairings this cell paints.
    pub audit: Vec<AuditPair>,
    /// A note carried with the cell — mandatory for `NotOwned`.
    pub note: Option<String>,
}

impl Cell {
    /// The canonical snapshot text for this cell.
    ///
    /// This is the assertion target: a resolver change that alters a
    /// rendered result changes this text, and the diff names the channel and
    /// the token rather than a byte offset.
    #[must_use]
    pub fn snapshot(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("cell: {}\n", self.id));
        out.push_str(&format!("kind: {:?}\n", self.kind));
        out.push_str(&format!("subject: {}\n", self.subject));
        out.push_str(&format!("spec: {}\n", self.spec));

        out.push_str("resolved:\n");
        for entry in &self.resolved {
            out.push_str(&format!(
                "  {:<28} = {}\n",
                entry.channel,
                entry.value.display()
            ));
        }

        out.push_str("frame:\n");
        out.push_str(&format!(
            "  viewport = {} {} {} {}\n",
            fixed(self.frame.viewport[0]),
            fixed(self.frame.viewport[1]),
            fixed(self.frame.viewport[2]),
            fixed(self.frame.viewport[3])
        ));
        for (index, quad) in self.frame.quads.iter().enumerate() {
            out.push_str(&format!(
                "  quad[{index}] {:<18} bounds={} {} {} {} fill={} border={} width={} radius={}\n",
                quad.layer,
                fixed(quad.bounds[0]),
                fixed(quad.bounds[1]),
                fixed(quad.bounds[2]),
                fixed(quad.bounds[3]),
                quad.fill,
                quad.border,
                fixed(quad.border_width),
                fixed(quad.radius),
            ));
        }

        out.push_str("audit:\n");
        for pair in &self.audit {
            let (ratio, _, class) = measure(pair);
            out.push_str(&format!(
                "  {:<18} {:<26} on {:<26} role={:?} class={:?} ratio={} min={}\n",
                pair.channel,
                self.namer.name(pair.foreground),
                self.namer.name(pair.background),
                pair.role,
                class,
                if ratio.is_nan() {
                    "n/a".to_string()
                } else {
                    fixed(ratio)
                },
                fixed(threshold(pair.role)),
            ));
        }

        if let Some(note) = &self.note {
            out.push_str(&format!("note: {note}\n"));
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Formatting and token naming
// ---------------------------------------------------------------------------

/// Channel-match tolerance for [`token_name`].
///
/// Resolved colours are copied out of the palette, so a match is exact to
/// the bit in practice; the epsilon exists because a composited or
/// alpha-adjusted colour goes through arithmetic that can land a
/// representation away from the token.
const CHANNEL_EPS: f32 = 1.0e-4;

/// A fixed-precision rendering of a length, so the manifest does not
/// depend on `Display`'s shortest-round-trip form.
#[must_use]
pub fn fixed(value: f32) -> String {
    format!("{value:.2}")
}

/// A colour as `RGB(r, g, b)` / `RGBA(r, g, b, a)`.
#[must_use]
pub fn rgb8(color: Color) -> String {
    let r = (color.r * 255.0).round();
    let g = (color.g * 255.0).round();
    let b = (color.b * 255.0).round();
    if (color.a - 1.0).abs() < CHANNEL_EPS {
        format!("RGB({r}, {g}, {b})")
    } else {
        format!("RGBA({r}, {g}, {b}, {:#.2})", color.a)
    }
}

/// Do two colours share their channels, ignoring alpha?
fn same_channels(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < CHANNEL_EPS
        && (a.g - b.g).abs() < CHANNEL_EPS
        && (a.b - b.b).abs() < CHANNEL_EPS
}

/// The standard palette's naming table.
///
/// Every field of [`Palette`], in the order the spec's §2.5 prints them.
/// Naming by matching against the palette rather than carrying a name
/// through the resolvers means a renamed field cannot make a failing colour
/// anonymous, and a colour that came from nowhere cannot pass as a token.
fn palette_table() -> Vec<(&'static str, Color)> {
    let p = Palette::dark();
    vec![
        ("surface.base", p.surface_base),
        ("surface.sunken", p.surface_sunken),
        ("surface.raised", p.surface_raised),
        ("surface.code", p.surface_code),
        ("surface.overlay", p.surface_overlay),
        ("surface.hover", p.surface_hover),
        ("surface.active", p.surface_active),
        ("surface.scrim", p.surface_scrim),
        ("border.subtle", p.border_subtle),
        ("border.strong", p.border_strong),
        ("accent.fill", p.accent_fill),
        ("accent.fill_active", p.accent_fill_active),
        ("accent.border", p.accent_border),
        ("accent.text", p.accent_text),
        ("text.primary", p.text_primary),
        ("text.strong", p.text_strong),
        ("text.muted", p.text_muted),
        ("text.disabled", p.text_disabled),
        ("status.success", p.status_success),
        ("status.warning", p.status_warning),
        ("status.danger_text", p.status_danger_text),
        ("status.danger_fill", p.status_danger_fill),
        ("status.danger_fill_active", p.status_danger_fill_active),
    ]
}

/// The inverted editor palette's naming table (`color.editor.inverted.*`).
///
/// Kept separate from [`palette_table`] rather than appended to it, because
/// the two worlds disagree about white: the standard palette's
/// `text.strong` and the editor's `editor.inverted.bg` are the same
/// channels, and a single merged table would silently pick one. Which
/// table is consulted first is the cell's [`Namer`]'s job.
fn editor_table() -> Vec<(&'static str, Color)> {
    let editor = EditorPalette::inverted();
    vec![
        ("editor.inverted.bg", editor.background),
        ("editor.inverted.fg", editor.text),
        ("editor.inverted.err", editor.error),
    ]
}

/// The first channel match in a naming table.
///
/// A colour that shares a token's channels at a different alpha is named
/// `token @ NN%`, which is how the pressed fills and the dimmed
/// overlays read.
fn lookup(table: &[(&'static str, Color)], colour: Color) -> Option<String> {
    for (name, token) in table {
        if same_channels(colour, *token) {
            if (colour.a - token.a).abs() < CHANNEL_EPS {
                return Some((*name).to_string());
            }
            return Some(format!("{name} @ {:.0}%", colour.a * 100.0));
        }
    }
    None
}

/// The palette world a cell's colours are named against.
///
/// Two palettes in this crate describe the same pixel: the inverted
/// editor's canvas is `RGB(255,255,255)`, which the standard palette
/// calls `text.strong`. Value-based naming cannot tell those apart on its
/// own, so the cell declares its world and the report names from that
/// world first, falling back to the standard table so a cell that mixes
/// the two still names every colour it emits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Namer {
    /// The standard dark palette — the `color.*` tokens of §2.5.
    Standard,
    /// The inverted editor palette — `color.editor.inverted.*` (§2.5).
    InvertedEditor,
}

impl Namer {
    /// Name one colour against this palette.
    #[must_use]
    pub fn name(self, colour: Color) -> String {
        if colour.a == 0.0 {
            return "transparent".to_string();
        }
        if self == Self::InvertedEditor
            && let Some(name) = lookup(&editor_table(), colour)
        {
            return name;
        }
        if let Some(name) = lookup(&palette_table(), colour) {
            return name;
        }
        format!("UNMAPPED {}", rgb8(colour))
    }

    /// A [`Background`] as this palette world renders it.
    #[must_use]
    pub fn background(self, background: &Background) -> String {
        match background {
            Background::Color(colour) => self.name(*colour),
            Background::Gradient(_) => "gradient".to_string(),
        }
    }

    /// A [`Border`] as this palette world renders it — `none` when
    /// nothing is stroked.
    #[must_use]
    pub fn border(self, border: &Border) -> String {
        if border.width == 0.0 {
            "none".to_string()
        } else {
            self.name(border.color)
        }
    }
}

/// Name the token a resolved colour came from in the standard palette.
///
/// A fully transparent colour is `transparent` — the word the spec's own
/// matrices print for the Ghost resting fill, not a token lookup. A colour
/// whose channels match a token at a different alpha is named
/// `token @ NN%`, which is how the pressed fills read.
///
/// Anything else is `UNMAPPED`, and the gallery sidecar treats that string
/// as a failure: it means the matrix emitted a colour the token set does
/// not contain, which `UIF-VER-001` forbids outright.
///
/// Cells resolved against another palette name through
/// [`Namer::name`] instead — see [`Cell::namer`].
#[must_use]
pub fn token_name(color: Color) -> String {
    Namer::Standard.name(color)
}

// ---------------------------------------------------------------------------
// The recording renderer and the frame painter
// ---------------------------------------------------------------------------

/// A renderer that records the quads it is handed instead of rasterising.
///
/// Implements only the base [`renderer::Renderer`] protocol — the same
/// surface `test_helpers::Recorder` implements — because that is all a
/// framework widget's own layers ever call. Living in the library rather
/// than behind `cfg(test)` is what lets `bin/gallery` draw the same frames
/// the tests assert on: the shared harness is a dev-dependency target and a
/// binary cannot reach it, so the recorder has to be crate code for the
/// runnable gallery and the asserted gallery to be the same thing.
#[derive(Default)]
pub struct FrameRecorder {
    painted: RefCell<Vec<(Rectangle, Background, Border)>>,
}

impl FrameRecorder {
    /// The protocol calls recorded so far, bottom of the stack first.
    #[must_use]
    pub fn recorded(&self) -> Vec<(Rectangle, Background, Border)> {
        self.painted.borrow().clone()
    }
}

impl renderer::Renderer for FrameRecorder {
    fn start_layer(&mut self, _bounds: Rectangle) {}

    fn end_layer(&mut self) {}

    fn start_transformation(&mut self, _transformation: Transformation) {}

    fn end_transformation(&mut self) {}

    fn fill_quad(&mut self, quad: renderer::Quad, background: impl Into<Background>) {
        self.painted
            .borrow_mut()
            .push((quad.bounds, background.into(), quad.border));
    }

    fn reset(&mut self, _new_bounds: Rectangle) {}

    fn allocate_image(
        &mut self,
        _handle: &image::Handle,
        _callback: impl FnOnce(Result<image::Allocation, image::Error>) + Send + 'static,
    ) {
    }
}

/// The rectangle as the manifest records it: `x, y, width, height`.
#[must_use]
pub fn rect_array(rectangle: Rectangle) -> [f32; 4] {
    [rectangle.x, rectangle.y, rectangle.width, rectangle.height]
}

/// Paint a named stack of [`Layer`]s through [`FrameRecorder`] and report
/// the frame.
///
/// Each layer is handed to the renderer with [`Layer::quad`], exactly as a
/// widget's `draw` hands it, so the recorded quads are the ones a real
/// renderer received rather than a description of them. The names come from
/// the caller because the renderer protocol does not carry them; the zip
/// is exact because one layer is one `fill_quad`.
#[must_use]
pub fn paint_frame(namer: Namer, viewport: Rectangle, layers: &[(&'static str, Layer)]) -> Frame {
    let mut recorder = FrameRecorder::default();

    for (_, layer) in layers {
        recorder.fill_quad(layer.quad(), layer.fill);
    }

    let painted = recorder.recorded();
    let quads = layers
        .iter()
        .zip(painted)
        .map(|((name, _), (bounds, background, border))| FrameQuad {
            layer: (*name).to_string(),
            bounds: rect_array(bounds),
            fill: namer.background(&background),
            border: namer.border(&border),
            border_width: border.width,
            radius: border.radius.top_left,
        })
        .collect();

    Frame {
        viewport: rect_array(viewport),
        quads,
    }
}

/// A layer for a surface the gallery itself composes — a card, a scrim, a
/// bar — where no atom resolver produced the style.
///
/// Every argument still comes from a token or from a surface resolver; the
/// helper exists so the composite builders read as a stack of named
/// surfaces rather than as repeated struct literals.
fn surface_layer(
    bounds: Rectangle,
    fill: Color,
    border: Color,
    border_width: f32,
    radius: f32,
) -> Layer {
    Layer {
        bounds,
        fill,
        border: Border::default()
            .color(border)
            .width(border_width)
            .rounded(radius),
    }
}

// ---------------------------------------------------------------------------
// Viewport and sample geometry
// ---------------------------------------------------------------------------

/// The viewport every gallery frame is drawn into.
///
/// A harness viewport, not a visual property: no §3 or §4 cell fixes the
/// window a control is mounted in, so the gallery composes one from tokens
/// rather than writing a bare number (`UIF-VER-001`). It is large enough
/// that the widest composite in the crate — a `960`-capped editor modal, a
/// `680 × 440` movable card — fits without being clamped, so a cell shows
/// the surface rather than the clamp.
pub const VIEWPORT: Size = Size::new(space::XL * 40.0, space::XL * 30.0);

/// The full-window viewport the composite hosts are opened against.
#[must_use]
pub fn viewport() -> Rectangle {
    Rectangle::new(Point::ORIGIN, VIEWPORT)
}

/// The sample box an atom cell is rendered in, at the standard rung.
///
/// Wide enough that the widest atom chrome — a search field's reserved
/// `size.control.sm` slot plus its padding — still leaves real text area,
/// and exactly one control tall so the cell is the atom and nothing else.
#[must_use]
pub fn atom_sample() -> Rectangle {
    atom_sample_at(Rung::Standard.height())
}

/// The sample box at one control-height rung.
#[must_use]
pub fn atom_sample_at(height: f32) -> Rectangle {
    Rectangle::new(Point::ORIGIN, Size::new(space::XL * 10.0, height))
}

/// The control-height rung an atom cell is snapshotted at (§2.2).
///
/// The resolvers do not take a height: a button's fills, borders and ring
/// are the same pixels at `28` as at `32`. The rung is therefore a
/// *geometry* dimension of the cell rather than a style one, which is
/// exactly what §3.1 and §3.2 say — the rung changes the box, not the
/// palette. Snapshotting only the standard rung, as `INC-21` did, left
/// every compact-toolbar control in the spec unsampled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rung {
    /// `size.control.md` — `32`, dialogs and forms.
    Standard,
    /// `size.control.sm` — `28`, compact toolbars, navbar rows, folder lists.
    Compact,
}

impl Rung {
    /// The rung's height.
    #[must_use]
    pub const fn height(self) -> f32 {
        match self {
            Self::Standard => size::CONTROL_MD,
            Self::Compact => size::CONTROL_SM,
        }
    }

    /// The token the rung comes from.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Standard => "size.control.md",
            Self::Compact => "size.control.sm",
        }
    }

    /// The rung's cell-id suffix.
    ///
    /// Empty for the standard rung so the ids the gallery has emitted
    /// since `INC-21` do not move; only the new compact cells gain one.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Standard => "",
            Self::Compact => "@compact",
        }
    }
}

/// Every rung the gallery samples, standard first.
pub const RUNGS: [Rung; 2] = [Rung::Standard, Rung::Compact];

// ---------------------------------------------------------------------------
// State enumeration
// ---------------------------------------------------------------------------

/// The lowercase state slug a [`BaseState`] carries in a cell id.
#[must_use]
pub fn base_slug(base: BaseState) -> &'static str {
    match base {
        BaseState::Rest => "rest",
        BaseState::Hovered => "hovered",
        BaseState::Pressed => "pressed",
        BaseState::Invalid => "invalid",
        BaseState::Disabled => "disabled",
    }
}

/// A [`VisualState`]'s slug: its base, plus `+focused` when the ring is on.
#[must_use]
pub fn state_slug(state: VisualState) -> String {
    if state.focused {
        format!("{}+focused", base_slug(state.base))
    } else {
        base_slug(state.base).to_string()
    }
}

/// Every [`VisualState`] the state machine can actually produce.
///
/// Derived by running all 32 [`InteractionInput`] combinations through
/// [`state::resolve`](crate::state::resolve) rather than written out, so
/// the gallery covers exactly the reachable set and no unreachable one.
/// `Disabled + focused` is absent because `resolve` zeroes the ring on a
/// dead control — a gallery that invented it would snapshot a state no
/// widget can ever paint.
#[must_use]
pub fn reachable_visual_states() -> Vec<VisualState> {
    let mut out: Vec<VisualState> = Vec::new();

    for enabled in [false, true] {
        for hovered in [false, true] {
            for pressed in [false, true] {
                for focused in [false, true] {
                    for invalid in [false, true] {
                        let state = crate::state::resolve(InteractionInput {
                            enabled,
                            hovered,
                            pressed,
                            focused,
                            invalid,
                        });
                        if !out.contains(&state) {
                            out.push(state);
                        }
                    }
                }
            }
        }
    }

    out
}

/// The reachable states in the order the spec's snapshot vocabulary lists
/// them: base first, unfocused before focused, `disabled` last.
///
/// The derivation order of [`reachable_visual_states`] is an artefact of
/// the input sweep; this pins the *presentation* order so cell ids and the
/// manifest read in spec order. The sidecar asserts the two sets agree, so
/// the ordering cannot hide a missing state.
#[must_use]
pub fn snapshot_states() -> Vec<VisualState> {
    let reachable = reachable_visual_states();
    let mut ordered: Vec<VisualState> = Vec::with_capacity(reachable.len());

    for base in [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Invalid,
        BaseState::Disabled,
    ] {
        for focused in [false, true] {
            let candidate = VisualState { base, focused };
            if reachable.contains(&candidate) {
                ordered.push(candidate);
            }
        }
    }

    ordered
}

// ---------------------------------------------------------------------------
// Atoms
// ---------------------------------------------------------------------------

/// A [`SlotContent`] as the manifest names it.
fn slot_label(slot: SlotContent) -> String {
    match slot {
        SlotContent::Empty => "empty".to_string(),
        SlotContent::Passive(glyph) => format!("passive {glyph}"),
        SlotContent::Actionable(glyph) => format!("actionable {glyph}"),
        SlotContent::Toggle(glyph) => format!("toggle {glyph}"),
    }
}

/// The button cell for one `(variant, state)` pair (`UIF-ATOM-001`).
///
/// Resolved through [`button::resolve_style`] — the same pure function the
/// widget paints from — and drawn as the two layers `button_widget`
/// produces for that style: the fill-and-border layer, then the focus ring
/// outside it. A Ghost at rest resolves a fully transparent fill and
/// border, which is the cell the spec prints as `transparent`; the frame
/// records that by naming the quad `transparent` rather than by the
/// gallery deciding to leave it out.
#[must_use]
pub fn button_cell(variant: ButtonVariant, state: VisualState) -> Cell {
    button_cell_at(variant, state, Rung::Standard)
}

/// The button cell for one `(variant, state, rung)` triple
/// (`UIF-ATOM-001`, §3.1's two control heights).
///
/// Resolved through [`button::resolve_style`] — the same pure function the
/// widget paints from — and drawn as the two layers `button_widget`
/// produces for that style: the fill-and-border layer, then the focus ring
/// outside it. A Ghost at rest resolves a fully transparent fill and
/// border, which is the cell the spec prints as `transparent`; the frame
/// records that by naming the quad `transparent` rather than by the
/// gallery deciding to leave it out.
#[must_use]
pub fn button_cell_at(variant: ButtonVariant, state: VisualState, rung: Rung) -> Cell {
    let palette = Palette::dark();
    let style = button::resolve_style(variant, state, &palette);
    let bounds = atom_sample_at(rung.height());

    let mut layers: Vec<(&'static str, Layer)> = Vec::new();
    if let Some(layer) = button_background_layer(bounds, &style) {
        layers.push(("background", layer));
    }
    if let Some(layer) = button_focus_ring_layer(bounds, &style) {
        layers.push(("focus-ring", layer));
    }

    let mut audit = vec![AuditPair {
        channel: "label",
        foreground: style.foreground,
        background: style.background,
        role: ContrastRole::Text,
    }];

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

    Cell {
        id: format!(
            "button/{}/{}{}",
            slug(variant),
            state_slug(state),
            rung.slug()
        ),
        kind: CellKind::Atom,
        subject: "button",
        spec: "UIF-ATOM-001",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::colour("background", style.background),
            StyleEntry::colour("foreground", style.foreground),
            StyleEntry::colour("border.color", style.border_color),
            StyleEntry::number("border.width", style.border_width),
            StyleEntry::colour("focus-ring.color", style.focus_ring_color),
            StyleEntry::number("focus-ring.width", style.focus_ring_width),
            StyleEntry::number("geometry.height", bounds.height),
            StyleEntry::text("geometry.height-token", rung.token()),
            StyleEntry::number("geometry.radius", button::RADIUS),
            StyleEntry::number("geometry.padding-x", button::HORIZONTAL_PADDING),
            StyleEntry::text(
                "transparent-background",
                style.is_transparent_background().to_string(),
            ),
        ],
        frame: paint_frame(Namer::Standard, bounds, &layers),
        audit,
        note: None,
    }
}

/// The four §3.1 variants in the order the spec prints them.
pub const BUTTON_VARIANTS: [ButtonVariant; 4] = [
    ButtonVariant::Primary,
    ButtonVariant::Secondary,
    ButtonVariant::Ghost,
    ButtonVariant::Danger,
];

/// Every button cell: four variants across every reachable state, at both
/// control-height rungs (§3.1's `28px` / `32px` pair).
#[must_use]
pub fn button_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for variant in BUTTON_VARIANTS {
        for state in snapshot_states() {
            for rung in RUNGS {
                cells.push(button_cell_at(variant, state, rung));
            }
        }
    }
    cells
}

/// A [`ButtonVariant`] as a cell-id slug.
fn slug(variant: ButtonVariant) -> &'static str {
    match variant {
        ButtonVariant::Primary => "primary",
        ButtonVariant::Secondary => "secondary",
        ButtonVariant::Ghost => "ghost",
        ButtonVariant::Danger => "danger",
    }
}

// ---------------------------------------------------------------------------
// `UIF-ATOM-001a` — the controlled button's busy, collapse and tier cells
// ---------------------------------------------------------------------------
//
// The cells above snapshot what a button *looks like*. These snapshot what it
// *does*: the busy projection a host puts on the model, the rung a declared
// density collapses to and the tooltip that collapse then owes, and the three
// tiers the activation engine actually emits. The tier cells are produced by
// driving a real [`ButtonController`] rather than by writing the tier name
// down, so a gallery frame cannot claim a behaviour the engine does not have.

/// The gesture outcome a tier cell records, and how a user produces it.
pub const BUTTON_TIERS: [(&str, &str); 3] = [
    ("activated", "release inside the bounds"),
    ("cancelled", "release outside, or Escape"),
    ("cancel-requested", "press while the host reports busy"),
];

/// The `UIF-ATOM-001a` busy cell for one variant.
///
/// The paint is deliberately identical to that variant's `rest` cell.
/// §3.0.4.3 forbids a busy control from resizing or re-skinning itself, so
/// the only thing a busy button gains is the **reserved spinner slot** — and
/// the slot is empty, because the glyph is `UIF-ATOM-009`'s deliverable and
/// this increment reserves space rather than inventing a paint.
#[must_use]
pub fn button_busy_cell(variant: ButtonVariant) -> Cell {
    let model = ButtonModel::new("Saving").busy().cancellable();
    let mut cell = button_cell_at(variant, REST_STATE, Rung::Standard);

    cell.id = format!("button/busy/{}", slug(variant));
    cell.spec = "UIF-ATOM-001a";
    cell.resolved.extend([
        StyleEntry::text("busy", model.busy.to_string()),
        StyleEntry::text("cancellable", model.cancellable.to_string()),
        StyleEntry::text("pending", "indeterminate"),
        StyleEntry::text("spinner.reserved", "true"),
        StyleEntry::number("spinner.slot-width", SPINNER_SLOT),
        StyleEntry::text("spinner.slot-token", "size.icon.sm"),
        StyleEntry::number("bounds.height", cell_height(&cell)),
    ]);
    cell.note = Some(
        "busy holds the outer bounds and reserves the spinner slot; the glyph itself is \
         UIF-ATOM-009, so the slot renders empty"
            .to_string(),
    );
    cell
}

/// The `UIF-ATOM-001a` density-collapse cell for one variant.
///
/// Records the rung a declared `Compact` density resolves to, the label mode
/// that rung forces, the tooltip that mode therefore makes mandatory, and
/// the zero vertical shift §3.0.4.4 fixes.
#[must_use]
pub fn button_collapsed_cell(variant: ButtonVariant) -> Cell {
    let model = ButtonModel::new("Save")
        .with_variant(variant)
        .with_density(ButtonDensity::Compact)
        .with_tooltip("Save this document");
    let layout = model.responsive();
    let mut cell = button_cell_at(variant, REST_STATE, Rung::Compact);

    cell.id = format!("button/collapsed/{}", slug(variant));
    cell.spec = "UIF-ATOM-001a";
    cell.resolved.extend([
        StyleEntry::text("density", "compact"),
        StyleEntry::text("breakpoint", format!("{:?}", layout.breakpoint())),
        StyleEntry::text("label-mode", format!("{:?}", layout.label_mode())),
        StyleEntry::text(
            "icon-only",
            model.is_icon_only(layout.label_mode()).to_string(),
        ),
        StyleEntry::text("tooltip.required", "true"),
        StyleEntry::text(
            "tooltip.present",
            model
                .tooltip_for(layout.label_mode())
                .unwrap_or("<MISSING>")
                .to_string(),
        ),
        StyleEntry::number("padding-x", layout.horizontal_padding()),
        StyleEntry::number("vertical-shift", layout.vertical_shift()),
    ]);
    cell.note = Some(
        "the collapsed rung drops the label and therefore owes a tooltip (UIF-ATOM-007); \
         the rung shift is 0px by construction"
            .to_string(),
    );
    cell
}

/// The `UIF-ATOM-001a` tier cell for one gesture outcome.
///
/// Drives a real [`ButtonController`] and records the tier it actually
/// emitted, so the cell is a transcript rather than a caption.
#[must_use]
pub fn button_tier_cell(tier: &'static str) -> Cell {
    let idle = ButtonModel::new("Save").cancellable();
    let busy = ButtonModel::new("Saving").busy().cancellable();

    let mut controller = ButtonController::new();
    let emission = match tier {
        "activated" => {
            controller.begin(&idle, ActivationSource::Pointer);
            controller
                .commit(&idle, ActivationSource::Pointer)
                .expect("a live button commits")
        }
        "cancelled" => {
            controller.begin(&idle, ActivationSource::Pointer);
            controller.cancel(&idle).expect("an open gesture cancels")
        }
        "cancel-requested" => controller
            .begin(&busy, ActivationSource::Pointer)
            .expect("a busy cancellable button requests"),
        other => panic!("unknown tier {other}"),
    };

    let mut cell = button_cell_at(ButtonVariant::Primary, REST_STATE, Rung::Standard);
    cell.id = format!("button/tier/{tier}");
    cell.spec = "UIF-ATOM-001a";
    cell.resolved.extend([
        StyleEntry::text("tier", tier),
        StyleEntry::text(
            "dispatched",
            matches!(emission, ButtonEmission::Activated(_)).to_string(),
        ),
        StyleEntry::text(
            "restores-activation",
            match emission.settlement() {
                Some(settlement) => format!("{:?}", settlement.activated),
                None => "n/a".to_string(),
            },
        ),
        StyleEntry::text(
            "pending-at-settle",
            format!("{:?}", emission.settlement().map(|s| s.pending)),
        ),
        StyleEntry::text("fired-count", controller.fired().to_string()),
        StyleEntry::text("suppressed-count", controller.suppressed().to_string()),
    ]);
    cell.note = Some(format!("produced by: {}", how(tier)));
    cell
}

/// How a user produces one of the [`BUTTON_TIERS`] outcomes.
fn how(tier: &'static str) -> &'static str {
    BUTTON_TIERS
        .iter()
        .find(|(name, _)| *name == tier)
        .map(|(_, how)| *how)
        .unwrap_or("unknown")
}

/// The `rest` visual state, for cells that snapshot a control at rest.
const REST_STATE: VisualState = VisualState {
    base: BaseState::Rest,
    focused: false,
};

/// The height a cell recorded, so a busy cell can assert it did not change.
fn cell_height(cell: &Cell) -> f32 {
    cell.resolved
        .iter()
        .find(|entry| entry.channel == "geometry.height")
        .and_then(|entry| match entry.value {
            StyleValue::Number(value) => Some(value),
            _ => None,
        })
        .unwrap_or(0.0)
}

/// Every `UIF-ATOM-001a` button cell: busy and collapsed per variant, plus
/// the three activation tiers.
#[must_use]
pub fn button_controlled_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for variant in BUTTON_VARIANTS {
        cells.push(button_busy_cell(variant));
        cells.push(button_collapsed_cell(variant));
    }
    for (tier, _) in BUTTON_TIERS {
        cells.push(button_tier_cell(tier));
    }
    cells
}

/// An [`InputKind`] as a cell-id slug.
fn slug_kind(kind: InputKind) -> &'static str {
    match kind {
        InputKind::TextEntry => "text-entry",
        InputKind::Search => "search",
        InputKind::DropdownTrigger => "dropdown-trigger",
        InputKind::Password => "password",
        InputKind::TextArea => "text-area",
    }
}

/// The text input cell for one `(kind, state)` pair (`UIF-ATOM-002`).
///
/// Includes the invalid-and-focused combination the atom exists to settle:
/// the error keeps the perimeter and focus relocates to a ring drawn
/// outside the control, so neither signal is lost. The two rectangles the
/// zero-shift invariant is about — the text area and the reserved trailing
/// slot — are recorded per cell and are functions of `(bounds, kind)`
/// alone, which is why they are identical across every slot content.
#[must_use]
pub fn text_input_cell(kind: InputKind, state: VisualState) -> Cell {
    text_input_cell_at(kind, state, Rung::Standard)
}

/// The text input cell for one `(kind, state, rung)` triple
/// (`UIF-ATOM-002`, §3.2's `32px` / `28px` compact-toolbar pair).
///
/// Includes the invalid-and-focused combination the atom exists to settle:
/// the error keeps the perimeter and focus relocates to a ring drawn
/// outside the control, so neither signal is lost. The two rectangles the
/// zero-shift invariant is about — the text area and the reserved trailing
/// slot — are recorded per cell and are functions of `(bounds, kind)`
/// alone, which is why they are identical across every slot content.
#[must_use]
pub fn text_input_cell_at(kind: InputKind, state: VisualState, rung: Rung) -> Cell {
    let palette = Palette::dark();
    let style = text_input::resolve_input_style(kind, state, &palette);
    let bounds = atom_sample_at(rung.height());
    let invalid = state.base == BaseState::Invalid;

    let mut layers: Vec<(&'static str, Layer)> = Vec::new();
    if let Some(layer) = text_input::background_layer(bounds, &style) {
        layers.push(("background", layer));
    }
    if let Some(layer) = text_input::focus_ring_layer(bounds, &style) {
        layers.push(("focus-ring", layer));
    }

    let mut audit = vec![
        AuditPair {
            channel: "value",
            foreground: style.text,
            background: style.background,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "placeholder",
            foreground: style.placeholder,
            background: style.background,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "icon",
            foreground: style.icon,
            background: style.background,
            role: ContrastRole::Text,
        },
    ];

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

    Cell {
        id: format!(
            "input/{}/{}{}",
            slug_kind(kind),
            state_slug(state),
            rung.slug()
        ),
        kind: CellKind::Atom,
        subject: "input",
        spec: "UIF-ATOM-002",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::colour("background", style.background),
            StyleEntry::colour("value", style.text),
            StyleEntry::colour("placeholder", style.placeholder),
            StyleEntry::colour("icon", style.icon),
            StyleEntry::colour("selection.background", style.selection_background),
            StyleEntry::colour("selection.foreground", style.selection_foreground),
            StyleEntry::colour("border.color", style.border_color),
            StyleEntry::number("border.width", style.border_width),
            StyleEntry::colour("focus-ring.color", style.focus_ring_color),
            StyleEntry::number("focus-ring.width", style.focus_ring_width),
            StyleEntry::text(
                "slot.empty",
                slot_label(text_input::slot_content(kind, false, invalid)),
            ),
            StyleEntry::text(
                "slot.value",
                slot_label(text_input::slot_content(kind, true, invalid)),
            ),
            StyleEntry::number("slot.width", kind.trailing_slot_width()),
            StyleEntry::text("error-border", style.is_error_border(&palette).to_string()),
            StyleEntry::text(
                "geometry.text-area",
                rect_text(text_input::text_area_bounds(bounds, kind)),
            ),
            StyleEntry::text(
                "geometry.trailing-slot",
                rect_text(text_input::trailing_slot_bounds(bounds, kind)),
            ),
            StyleEntry::number("geometry.height", bounds.height),
            StyleEntry::text("geometry.height-token", rung.token()),
            StyleEntry::number("geometry.radius", text_input::RADIUS),
        ],
        frame: paint_frame(Namer::Standard, bounds, &layers),
        audit,
        note: None,
    }
}

/// The three §3.2 kinds in the order the spec names them.
pub const INPUT_KINDS: [InputKind; 3] = [
    InputKind::TextEntry,
    InputKind::Search,
    InputKind::DropdownTrigger,
];

/// Every text input cell: three kinds across every reachable state, at
/// both control-height rungs.
#[must_use]
pub fn text_input_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for kind in INPUT_KINDS {
        for state in snapshot_states() {
            for rung in RUNGS {
                cells.push(text_input_cell_at(kind, state, rung));
            }
        }
    }
    cells
}

/// The editing outcome a `002a` tier cell records, and how a user produces
/// it.
pub const TEXT_INPUT_TIERS: [(&str, &str); 6] = [
    ("changed", "type a character"),
    ("committed-enter", "press Enter"),
    ("committed-blur", "leave the field with an open session"),
    ("cleared", "press the trailing clear glyph"),
    ("cancelled", "press Escape inside an open session"),
    ("focus-changed", "focus the field"),
];

/// The `UIF-ATOM-002a` tier cell for one editing outcome.
///
/// Drives a real [`TextInputController`] and records the tiers it
/// actually emitted, so the cell is a transcript rather than a caption —
/// the discipline `button_tier_cell` sets. The paint underneath is the
/// search field's own `rest` cell, deliberately unchanged: a tier is a
/// fact about the wire, not a visual state, and nothing here may suggest
/// otherwise.
#[must_use]
pub fn text_input_tier_cell(tier: &'static str) -> Cell {
    let mut controller = TextInputController::new();
    controller.observe(&TextInputModel::new("report").with_placeholder("Search reports…"));

    let emissions: Vec<TextInputEmission> = match tier {
        "changed" => {
            controller.note_edit("report 2".to_owned());
            controller
                .take_live()
                .map(|value| vec![TextInputEmission::Changed(Live::new(value))])
                .unwrap_or_default()
        }
        "committed-enter" => {
            controller.note_edit("report 2026".to_owned());
            controller.take_live();
            vec![controller.commit().expect("Enter settles what it holds")]
        }
        "committed-blur" => {
            controller.note_focus(true);
            controller.note_edit("report 2026".to_owned());
            controller.take_live();
            controller.note_focus(false)
        }
        "cleared" => vec![controller.clear().expect("a non-empty field clears")],
        "cancelled" => {
            controller.note_edit("report 2026".to_owned());
            vec![controller.revert().expect("an open session reverts")]
        }
        "focus-changed" => controller.note_focus(true),
        other => panic!("unknown tier {other}"),
    };

    let primary = emissions
        .first()
        .unwrap_or_else(|| panic!("tier {tier} emitted nothing"));
    let rest: Vec<String> = emissions
        .iter()
        .skip(1)
        .map(|emission| format!("{}({})", emission.tier(), emission.value().unwrap_or("")))
        .collect();

    let mut cell = text_input_cell(InputKind::Search, REST_STATE);
    cell.id = format!("input/tier/{tier}");
    cell.spec = "UIF-ATOM-002a";
    cell.resolved.extend([
        StyleEntry::text("tier", tier),
        StyleEntry::text("tier-class", primary.tier()),
        StyleEntry::text(
            "emitted-value",
            primary.value().unwrap_or("<no value>").to_string(),
        ),
        StyleEntry::text("settles-session", primary.settles().to_string()),
        StyleEntry::text(
            "also-emitted",
            if rest.is_empty() {
                "none".to_string()
            } else {
                rest.join(",")
            },
        ),
        StyleEntry::text("session-open", controller.session_open().to_string()),
        StyleEntry::text(
            "snapshot",
            controller.snapshot().unwrap_or("<none>").to_string(),
        ),
        StyleEntry::text("engine-value", controller.value().to_string()),
        StyleEntry::text("coalesced", controller.coalesced().to_string()),
        StyleEntry::text("superseded", controller.superseded().to_string()),
        StyleEntry::text("clears", controller.clears().to_string()),
    ]);
    cell.note = Some(format!("produced by: {}", how(tier)));
    cell
}

/// Every `UIF-ATOM-002a` tier cell.
#[must_use]
pub fn text_input_tier_cells() -> Vec<Cell> {
    TEXT_INPUT_TIERS
        .iter()
        .map(|(tier, _)| text_input_tier_cell(tier))
        .collect()
}

/// A rectangle rendered compactly for a resolved-value entry.
fn rect_text(rectangle: Rectangle) -> String {
    format!(
        "{} {} {} {}",
        fixed(rectangle.x),
        fixed(rectangle.y),
        fixed(rectangle.width),
        fixed(rectangle.height)
    )
}

// ---------------------------------------------------------------------------
// Composite: the menu card in every row state (§4.9.2)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Composite: the modal host (§4.6)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Cell builders — the input dialog (UIF-INP-001 … 005)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Cell builders — the movable overlay (UIF-MOV-001 … 004)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Cell builder — the z-band stack (§5 `UIF-ZORD-*`, `UIF-WIN-003`, G-13)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Cells for the inverted editor palette (§2.5 `color.editor.inverted.*`)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Cells for the modal's header / banner / footer bands (§4.6.1)
// ---------------------------------------------------------------------------

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

/// The footer band's height: its own vertical padding either side of one
/// standard control.
fn footer_height(frame: &ModalFrame) -> f32 {
    frame.footer_padding_vertical * 2.0 + size::CONTROL_MD
}

/// The error every banded cell carries.
fn gallery_error(code: &str, message: &str) -> ModalError {
    ModalError::new(code, message)
        .at(modal::ErrorLocation::Position {
            line: 12,
            column: 4,
        })
        .with_recovery(modal::Recovery::Retry)
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

// ---------------------------------------------------------------------------
// Cells for menu separators and glyph placement (§4.8.1, §4.9.2)
// ---------------------------------------------------------------------------

/// A 1px stroked ink box: where a glyph's ink lands, drawn in the token
/// it is drawn with.
///
/// A glyph is drawn by a text pass, not a `fill_quad`, so the recorded
/// quad list cannot show the glyph itself. What it can honestly show is
/// the glyph's reserved box and the token its ink carries — which is the
/// pair of facts the spec's slot anatomy is actually asserting. Stroking
/// rather than filling keeps the quad from claiming a solid fill where
/// there is type.
fn ink_box(bounds: Rectangle, colour: Color) -> Layer {
    Layer {
        bounds,
        fill: Color::TRANSPARENT,
        border: Border::default()
            .color(colour)
            .width(shape::BORDER_HAIRLINE)
            .rounded(0.0),
    }
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

// ---------------------------------------------------------------------------
// Cells for the movable overlay's trailing actions (§4.7.1, UIF-MOV-001)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Cells the framework does not own (D-2)
// ---------------------------------------------------------------------------

/// A cell for a spec surface this crate has no frame for.
///
/// The cell draws nothing. Its whole content is the note, which says what
/// the framework *does* contribute and what it does not, so the compliance
/// audit can read "out of scope under `D-2`" off the manifest instead of
/// inferring it from an absent row.
fn not_owned_cell(subject: &'static str, spec: &'static str, note: &str) -> Cell {
    Cell {
        id: format!("{subject}/not-owned"),
        kind: CellKind::NotOwned,
        subject,
        spec,
        namer: Namer::Standard,
        resolved: Vec::new(),
        frame: Frame {
            viewport: rect_array(viewport()),
            quads: Vec::new(),
        },
        audit: Vec::new(),
        note: Some(note.to_string()),
    }
}

/// The `NotOwned` cells for the three composite surfaces §4 names that this
/// crate has no module for.
#[must_use]
pub fn not_owned_cells() -> Vec<Cell> {
    vec![
        not_owned_cell(
            "navbar",
            "UIF-NAV-001",
            "No navbar module exists in iced-pro. The framework contributes the \
             tokens (color.surface.*, space.*, typography) and the atoms \
             (button, text-input) a host composes a navbar from, but the bar's \
             own layout, its item set and its iconography are app-owned content \
             under D-2, so there is no framework-owned frame to snapshot.",
        ),
        not_owned_cell(
            "panes",
            "UIF-PANE-001",
            "No splitter or multi-pane module exists in iced-pro. Pane sizing, \
             the drag-to-resize divider and the pane contents are app-owned \
             under D-2; the framework owns no divider geometry, no min/max pane \
             policy and therefore no cell to render.",
        ),
        not_owned_cell(
            "tabs",
            "UIF-TAB-001",
            "No tab-strip module exists in iced-pro. A tab is not a variant of \
             the 3.1 button - the spec's selected/unselected/close affordances \
             are a distinct widget the crate has not been given - so \
             snapshotting a button here would misrepresent coverage rather than \
             provide it. Under D-2 the tab strip and its contents are \
             app-owned.",
        ),
    ]
}

// ---------------------------------------------------------------------------
// Tooltip cells (`UIF-ATOM-001` icon-only cell, `UIF-NAV-003`, `UIF-TAB-003`)
// ---------------------------------------------------------------------------

use crate::surface::tooltip::{
    self, ArrivalSource, HoverController, TimeMs, TooltipAnchor, TooltipStyle, Visibility,
};

/// The tooltip card's painted size.
///
/// A stand-in for a measured label: the resolver takes the card size from the host, so the
/// gallery supplies one rather than inventing a text measurer. Width is `space.xl * 7`,
/// height the minimum hit target, which is roughly what a short label occupies at
/// `type.caption`.
const TOOLTIP_CARD_SIZE: Size = Size::new(space::XL * 7.0, size::HIT_TARGET_MIN);

/// The navbar the placement cells are anchored in: a `size.control.lg` bar with a
/// `size.control.sm` entry inset inside it — the exact geometry that makes the
/// bar-versus-button anchor choice matter.
fn gallery_navbar(width: f32) -> (Rectangle, Rectangle) {
    let band = Rectangle::new(Point::new(0.0, 0.0), Size::new(width, size::CONTROL_LG));
    let entry = Rectangle::new(
        Point::new(space::SM, 4.0),
        Size::new(space::XL * 4.0, size::CONTROL_SM),
    );
    (band, entry)
}

/// Paint the tooltip card plus the anchor it hangs from, so a cell shows the *relationship*
/// rather than a floating swatch.
fn tooltip_card_layers(
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
    cells
}

// ---------------------------------------------------------------------------
// The whole gallery
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Cell builders - the `000b` binding tiers on the surface hosts (`INC-36`)
// ---------------------------------------------------------------------------

/// Look up how a user produces a tier outcome, for a cell's note line.
fn tier_how(table: &[(&'static str, &'static str)], tier: &'static str) -> &'static str {
    table
        .iter()
        .find(|(name, _)| *name == tier)
        .map(|(_, how)| *how)
        .unwrap_or("unknown")
}

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
/// [`button_tier_cell`] sets. The paint underneath is the menu row's own
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

/// Every cell in the `UIF-VER-004` matrix, in a fixed order.
///
/// The order is the module's own, not an iteration artefact: atoms first
/// in spec order, then composites bottom of the z-stack upward, then the
/// `NotOwned` findings. Two calls return the same list in the same order.
#[must_use]
pub fn matrix() -> Vec<Cell> {
    let mut cells = Vec::new();
    cells.extend(button_cells());
    cells.extend(button_controlled_cells());
    cells.extend(crate::gallery_segmented::segmented_cells());
    cells.extend(crate::gallery_split_button::split_button_cells());
    cells.extend(crate::gallery_hyperlink::hyperlink_cells());
    cells.extend(text_input_cells());
    cells.extend(text_input_tier_cells());
    cells.extend(crate::gallery_secret::secret_cells());
    cells.extend(crate::gallery_toggle::toggle_cells());
    cells.extend(crate::gallery_combo::combo_cells());
    cells.extend(crate::gallery_range::range_cells());
    cells.extend(crate::gallery_badge::badge_cells());
    cells.extend(crate::gallery_chip::chip_cells());
    cells.extend(crate::gallery_progress::progress_cells());
    cells.extend(menu_cells());
    cells.extend(menu_glyph_cells());
    cells.extend(menu_row_tier_cells());
    cells.extend(modal_cells());
    cells.extend(modal_band_cells());
    cells.extend(modal_tier_cells());
    cells.extend(input_dialog_cells());
    cells.extend(input_dialog_row_cells());
    cells.extend(dialog_button_tier_cells());
    cells.extend(movable_cells());
    cells.extend(movable_action_cells());
    cells.extend(drag_tier_cells());
    cells.extend(editor_palette_cells());
    cells.push(zband_cell());
    cells.extend(tooltip_cells());
    cells.extend(crate::gallery_popover::popover_cells());
    cells.extend(not_owned_cells());
    cells
}

/// How one audit pair is judged.
///
/// Three of the four classes are *not* gated, and each for a reason the
/// threshold itself cannot express. Measuring every produced pair against
/// `3.0:1` regardless of class would fail a stroke that deliberately
/// shares its fill's channels, a `border.subtle` hairline that is by
/// definition not indicator material, and a channel painted over nothing.
/// The classification is the same one the `INC-08` button audit settled
/// on, kept here so the gallery's gate and the widget audits cannot drift
/// into two different definitions of "passes".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditClass {
    /// Judged against the role's threshold.
    Judged,
    /// Nothing is painted behind this channel, so its contrast is the
    /// container's business rather than the style's.
    UnpaintedBackground,
    /// The stroke carries the same channels as the fill it sits on, so it
    /// paints nothing visible. It exists to hold geometry
    /// (`UIF-PRIN-003`), not to signal a state.
    CoincidentWithFill,
    /// `border.subtle` — a **resting** divider. `UIF-TOK-006` rule 2
    /// governs *active* chrome, and this token is not indicator material.
    ///
    /// Matched on the token's own channels rather than on the channel name,
    /// because the gallery meets `border.subtle` under several names —
    /// `border`, `control.border`, `divider` — and the exemption is about
    /// what the token *is*, not what the caller called it. Every
    /// `border_subtle` use in the crate is a resting border or a divider;
    /// no focus ring, selection or destructive indicator is drawn with it,
    /// so the resting-divider class covers the token completely.
    RestingDivider,
}

/// The contrast floor a role owes.
#[must_use]
pub const fn threshold(role: ContrastRole) -> f32 {
    match role {
        ContrastRole::Text => contrast::MIN_TEXT_CONTRAST,
        ContrastRole::Chrome => contrast::MIN_CHROME_CONTRAST,
    }
}

/// Every surface token in the elevation ladder, darkest to lightest.
///
/// Also the container set a translucent fill can be mounted over: a
/// translucent background is judged against the *worst* of these, because
/// the resolved style does not know what it is mounted to and the audit
/// must not assume the friendliest answer.
#[must_use]
pub fn all_surfaces() -> [(&'static str, Color); 7] {
    let palette = Palette::dark();
    [
        ("surface.sunken", palette.surface_sunken),
        ("surface.base", palette.surface_base),
        ("surface.raised", palette.surface_raised),
        ("surface.code", palette.surface_code),
        ("surface.overlay", palette.surface_overlay),
        ("surface.hover", palette.surface_hover),
        ("surface.active", palette.surface_active),
    ]
}

/// Measure one audit pair: the ratio, the painted background it was
/// measured against, and how it is classified.
///
/// For an opaque background the painted background is the background. For
/// a translucent one it is the composite over the worst surface in the
/// ladder, so the reported ratio is the one the cell can actually end up
/// painting rather than the best case.
#[must_use]
pub fn measure(pair: &AuditPair) -> (f32, Color, AuditClass) {
    let palette = Palette::dark();
    let foreground = pair.foreground;
    let background = pair.background;

    if background.a == 0.0 {
        return (f32::NAN, background, AuditClass::UnpaintedBackground);
    }
    if same_channels(foreground, background) {
        return (f32::NAN, background, AuditClass::CoincidentWithFill);
    }
    if same_channels(foreground, palette.border_subtle) {
        return (
            contrast::contrast_ratio(foreground, background),
            background,
            AuditClass::RestingDivider,
        );
    }
    if background.a >= 1.0 {
        return (
            contrast::contrast_ratio(foreground, background),
            background,
            AuditClass::Judged,
        );
    }

    let mut worst_ratio = f32::MAX;
    let mut worst_painted = background;
    for (_, surface) in all_surfaces() {
        let painted = contrast::composite_over(background, surface);
        let ratio = contrast::contrast_ratio(foreground, painted);
        if ratio < worst_ratio {
            worst_ratio = ratio;
            worst_painted = painted;
        }
    }
    (worst_ratio, worst_painted, AuditClass::Judged)
}

/// Whether one audit pair clears its gate.
///
/// Only [`AuditClass::Judged`] pairs can fail; the other classes are
/// reported without a verdict because no threshold describes them.
#[must_use]
pub fn judge(pair: &AuditPair) -> bool {
    let (ratio, _, class) = measure(pair);
    match class {
        AuditClass::Judged => ratio >= threshold(pair.role),
        AuditClass::UnpaintedBackground
        | AuditClass::CoincidentWithFill
        | AuditClass::RestingDivider => true,
    }
}

/// Every colour the gallery emitted that no [`Palette`] token accounts for.
///
/// An empty list is the pass condition for `UIF-VER-001`: it says every
/// pixel the gallery can produce traces back to a named token. A non-empty
/// list is a hard failure, not a ledger entry - an unmapped colour is a
/// value no theme can change, which is the exact defect the token rule
/// exists to prevent.
#[must_use]
pub fn unmapped_colours(cells: &[Cell]) -> Vec<String> {
    let mut out = Vec::new();

    for cell in cells {
        for entry in &cell.resolved {
            if let Some(colour) = entry.value.colour() {
                let name = cell.namer.name(colour);
                if name.starts_with("UNMAPPED") {
                    out.push(format!("{} resolved.{} -> {name}", cell.id, entry.channel));
                }
            }
        }

        for quad in &cell.frame.quads {
            if quad.fill.starts_with("UNMAPPED") {
                out.push(format!(
                    "{} frame.{}.fill -> {}",
                    cell.id, quad.layer, quad.fill
                ));
            }
            if quad.border.starts_with("UNMAPPED") {
                out.push(format!(
                    "{} frame.{}.border -> {}",
                    cell.id, quad.layer, quad.border
                ));
            }
        }

        for pair in &cell.audit {
            for (slot, colour) in [
                ("foreground", pair.foreground),
                ("background", pair.background),
            ] {
                let name = cell.namer.name(colour);
                if name.starts_with("UNMAPPED") {
                    out.push(format!(
                        "{} audit.{}.{slot} -> {name}",
                        cell.id, pair.channel
                    ));
                }
            }
        }
    }

    out
}

/// Every audit pair that fails its role's contrast threshold.
///
/// The companion to [`unmapped_colours`]: where an unmapped colour is a
/// token defect, this is a legibility defect. The list is empty today, and
/// keeping it empty is what makes the gallery a gate rather than a report.
#[must_use]
pub fn contrast_failures(cells: &[Cell]) -> Vec<String> {
    let mut out = Vec::new();
    for cell in cells {
        for pair in &cell.audit {
            if judge(pair) {
                continue;
            }
            let (ratio, painted, _) = measure(pair);
            out.push(format!(
                "{} {} {} on {} (role {:?}, ratio {}, worst-painted {})",
                cell.id,
                pair.channel,
                cell.namer.name(pair.foreground),
                cell.namer.name(pair.background),
                pair.role,
                fixed(ratio),
                rgb8(painted),
            ));
        }
    }
    out
}

/// Channels the spec's matrices name that no recorded frame can carry.
///
/// Recorded rather than left absent, so the manifest's silence is never
/// misread as coverage. The iced text caret is the one such channel: the
/// backend's text pass draws it after the widget's own layers, and nothing
/// about it — bounds, fill, token — crosses the [`renderer::Renderer`]
/// protocol a [`FrameRecorder`] can see. Snapshotting it would mean the
/// gallery inventing geometry the framework does not own, which is the
/// exact thing `UIF-VER-001` exists to prevent. It stays out of the
/// snapshot surface for the life of the recorder-based gallery.
pub const SURFACE_EXCLUSIONS: [(&str, &str); 1] = [(
    "text.caret",
    "Drawn by the iced text backend rather than by a Layer: no bounds, no \
     fill and no token reaches the renderer protocol, so it cannot be \
     snapshotted without inventing framework geometry the crate does not \
     own. Permanently outside the recorder-based gallery's surface.",
)];

/// Render the whole gallery as the manifest text `bin/gallery` writes.
///
/// A header, then one [`Cell::snapshot`] per cell, then the gate ledgers.
/// The gate output is part of the manifest so an inspectable artifact
/// carries its own pass condition rather than leaving the reader to
/// re-derive it.
#[must_use]
pub fn manifest() -> String {
    let cells = matrix();

    let mut out = String::new();
    out.push_str("iced-pro visual gallery - UIF-VER-004\n");
    out.push_str("snapshot target: resolved style values + recorded quad list\n");
    out.push_str(&format!("cells: {}\n\n", cells.len()));

    for cell in &cells {
        out.push_str(&cell.snapshot());
        out.push('\n');
    }

    out.push_str("== gate: token mapping (UIF-VER-001) ==\n");
    let unmapped = unmapped_colours(&cells);
    if unmapped.is_empty() {
        out.push_str("PASS - every emitted colour names a Palette token\n");
    } else {
        out.push_str("FAIL\n");
        for line in unmapped {
            out.push_str(&format!("  {line}\n"));
        }
    }

    out.push_str("\n== gate: contrast (UIF-TOK-006) ==\n");
    let failures = contrast_failures(&cells);
    if failures.is_empty() {
        out.push_str("PASS - every judged pair clears its role threshold\n");
    } else {
        out.push_str("FAIL\n");
        for line in failures {
            out.push_str(&format!("  {line}\n"));
        }
    }

    out.push_str("\n== gate: scope (D-2) ==\n");
    let not_owned: Vec<&Cell> = cells
        .iter()
        .filter(|cell| cell.kind == CellKind::NotOwned)
        .collect();
    out.push_str(&format!(
        "{} surfaces emitted as NotOwned rather than omitted\n",
        not_owned.len()
    ));
    for cell in not_owned {
        out.push_str(&format!("  {} - {}\n", cell.id, cell.spec));
    }

    out.push_str("\n== recorded exclusions: named channels with no frame ==\n");
    out.push_str(&format!(
        "{} channels recorded as unreachable from a snapshot\n",
        SURFACE_EXCLUSIONS.len()
    ));
    for (channel, why) in SURFACE_EXCLUSIONS {
        out.push_str(&format!("  {channel} - {why}\n"));
    }

    out
}

#[cfg(test)]
#[path = "gallery_tests.rs"]
mod gallery_tests;
