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

use crate::gallery_audit::{measure, threshold};
use crate::state::BaseState;
use crate::state::InteractionInput;
use crate::state::VisualState;
use crate::surface::modal::{self, ModalError, ModalFrame};
use crate::token::color::{EditorPalette, Palette};
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::button_widget::Layer;

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
pub(crate) const CHANNEL_EPS: f32 = 1.0e-4;

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
///
/// `pub(crate)` because the contrast audit (`gallery_audit`) classifies a
/// stroke as coincident with its fill through this same test, and the
/// gallery's gate and the audit must not hold two different definitions
/// of "same channels".
pub(crate) fn same_channels(a: Color, b: Color) -> bool {
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
pub(crate) fn surface_layer(
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
// Shared cell-construction helpers
//
// These five are the vocabulary the split gallery modules (`gallery_button`,
// `gallery_input`, `gallery_menu`, `gallery_modal`, `gallery_movable`,
// `gallery_editor`, `gallery_tooltip`) build their cells with. Each one is
// used by more than one of them, so it stays here rather than being
// duplicated into the first module that wanted it; a helper with exactly one
// user moved out with that user instead. They are `pub(crate)` and not `pub`:
// widening was needed for the split, and nothing outside this crate should
// take a dependency on the gallery's internal drawing vocabulary.
// ---------------------------------------------------------------------------

/// The `rest` visual state, for cells that snapshot a control at rest.
pub(crate) const REST_STATE: VisualState = VisualState {
    base: BaseState::Rest,
    focused: false,
};

/// A rectangle rendered compactly for a resolved-value entry.
pub(crate) fn rect_text(rectangle: Rectangle) -> String {
    format!(
        "{} {} {} {}",
        fixed(rectangle.x),
        fixed(rectangle.y),
        fixed(rectangle.width),
        fixed(rectangle.height)
    )
}

/// The footer band's height: its own vertical padding either side of one
/// standard control.
pub(crate) fn footer_height(frame: &ModalFrame) -> f32 {
    frame.footer_padding_vertical * 2.0 + size::CONTROL_MD
}

/// The error every banded cell carries.
pub(crate) fn gallery_error(code: &str, message: &str) -> ModalError {
    ModalError::new(code, message)
        .at(modal::ErrorLocation::Position {
            line: 12,
            column: 4,
        })
        .with_recovery(modal::Recovery::Retry)
}

/// A 1px stroked ink box: where a glyph's ink lands, drawn in the token
/// it is drawn with.
///
/// A glyph is drawn by a text pass, not a `fill_quad`, so the recorded
/// quad list cannot show the glyph itself. What it can honestly show is
/// the glyph's reserved box and the token its ink carries — which is the
/// pair of facts the spec's slot anatomy is actually asserting. Stroking
/// rather than filling keeps the quad from claiming a solid fill where
/// there is type.
pub(crate) fn ink_box(bounds: Rectangle, colour: Color) -> Layer {
    Layer {
        bounds,
        fill: Color::TRANSPARENT,
        border: Border::default()
            .color(colour)
            .width(shape::BORDER_HAIRLINE)
            .rounded(0.0),
    }
}

// ---------------------------------------------------------------------------
// Cell builders - the `000b` binding tiers on the surface hosts (`INC-36`)
// ---------------------------------------------------------------------------

/// Look up how a user produces a tier outcome, for a cell's note line.
///
/// Shared by every `000b` tier family the split modules emit — the menu
/// row's, the dialog button's, the modal's and the drag's — so a tier
/// name and its "how a user does this" string are looked up the same way
/// in all four.
pub(crate) fn tier_how(table: &[(&'static str, &'static str)], tier: &'static str) -> &'static str {
    table
        .iter()
        .find(|(name, _)| *name == tier)
        .map(|(_, how)| *how)
        .unwrap_or("unknown")
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
            "No navbar module exists in hoarfrost. The framework contributes the \
             tokens (color.surface.*, space.*, typography) and the atoms \
             (button, text-input) a host composes a navbar from, but the bar's \
             own layout, its item set and its iconography are app-owned content \
             under D-2, so there is no framework-owned frame to snapshot.",
        ),
        not_owned_cell(
            "panes",
            "UIF-PANE-001",
            "No splitter or multi-pane module exists in hoarfrost. Pane sizing, \
             the drag-to-resize divider and the pane contents are app-owned \
             under D-2; the framework owns no divider geometry, no min/max pane \
             policy and therefore no cell to render.",
        ),
        not_owned_cell(
            "tabs",
            "UIF-TAB-001",
            "No tab-strip module exists in hoarfrost. A tab is not a variant of \
             the 3.1 button - the spec's selected/unselected/close affordances \
             are a distinct widget the crate has not been given - so \
             snapshotting a button here would misrepresent coverage rather than \
             provide it. Under D-2 the tab strip and its contents are \
             app-owned.",
        ),
    ]
}

// ---------------------------------------------------------------------------
// SplitDivider snapshot quads — UIF-COMP-016, UIF-PANE-001
// ---------------------------------------------------------------------------

/// Ordered snapshot quads for [`SplitDivider`](crate::composite::SplitDivider)
/// across resting, hovered, dragged, and disabled states (§3B.16, `UIF-COMP-016`, `UIF-PANE-001`, `UIF-PRIN-006`).
///
/// Returns a quad list for each of the four interaction postures:
/// `[("resting", ...), ("hovered", ...), ("dragged", ...), ("disabled", ...)]`.
#[must_use]
pub fn split_divider_snapshot_quads(
    axis: crate::composite::SplitAxis,
    bounds: Rectangle,
    palette: &Palette,
) -> Vec<(&'static str, Vec<FrameQuad>)> {
    vec![
        (
            "resting",
            split_divider_quads(
                VisualState {
                    base: BaseState::Rest,
                    focused: false,
                },
                axis,
                bounds,
                palette,
            ),
        ),
        (
            "hovered",
            split_divider_quads(
                VisualState {
                    base: BaseState::Hovered,
                    focused: false,
                },
                axis,
                bounds,
                palette,
            ),
        ),
        (
            "dragged",
            split_divider_quads(
                VisualState {
                    base: BaseState::Pressed,
                    focused: false,
                },
                axis,
                bounds,
                palette,
            ),
        ),
        (
            "disabled",
            split_divider_quads(
                VisualState {
                    base: BaseState::Disabled,
                    focused: false,
                },
                axis,
                bounds,
                palette,
            ),
        ),
    ]
}

/// The ordered quad list a [`SplitDivider`](crate::composite::SplitDivider) frame draws in `state`
/// (§3B.16, `UIF-COMP-016`, `UIF-PANE-001`, `UIF-PRIN-006`).
#[must_use]
pub fn split_divider_quads(
    state: VisualState,
    axis: crate::composite::SplitAxis,
    bounds: Rectangle,
    palette: &Palette,
) -> Vec<FrameQuad> {
    let layers = crate::composite::split_divider_layers(bounds, axis, state, palette);
    paint_frame(Namer::Standard, bounds, &layers).quads
}

// ---------------------------------------------------------------------------
// The whole gallery
// ---------------------------------------------------------------------------

/// Every cell in the `UIF-VER-004` matrix, in a fixed order.
///
/// The order is the module's own, not an iteration artefact: atoms first
/// in spec order, then composites bottom of the z-stack upward, then the
/// `NotOwned` findings. Two calls return the same list in the same order.
#[must_use]
pub fn matrix() -> Vec<Cell> {
    let mut cells = Vec::new();
    cells.extend(crate::gallery_button::button_cells());
    cells.extend(crate::gallery_button::button_controlled_cells());
    cells.extend(crate::gallery_segmented::segmented_cells());
    cells.extend(crate::gallery_split_button::split_button_cells());
    cells.extend(crate::gallery_hyperlink::hyperlink_cells());
    cells.extend(crate::gallery_input::text_input_cells());
    cells.extend(crate::gallery_input::text_input_tier_cells());
    cells.extend(crate::gallery_secret::secret_cells());
    cells.extend(crate::gallery_suggest::suggest_cells());
    cells.extend(crate::gallery_toggle::toggle_cells());
    cells.extend(crate::gallery_combo::combo_cells());
    cells.extend(crate::gallery_range::range_cells());
    cells.extend(crate::gallery_badge::badge_cells());
    cells.extend(crate::gallery_chip::chip_cells());
    cells.extend(crate::gallery_progress::progress_cells());
    cells.extend(crate::gallery_composite::composite_cells());
    cells.extend(crate::gallery_tree::tree_cells());
    cells.extend(crate::gallery_table::table_cells());
    cells.extend(crate::gallery_breadcrumb::breadcrumb_cells());
    cells.extend(crate::gallery_menu::menu_cells());
    cells.extend(crate::gallery_menu::menu_glyph_cells());
    cells.extend(crate::gallery_menu::menu_row_tier_cells());
    cells.extend(crate::gallery_modal::modal_cells());
    cells.extend(crate::gallery_modal::modal_band_cells());
    cells.extend(crate::gallery_modal::modal_tier_cells());
    cells.extend(crate::gallery_modal::input_dialog_cells());
    cells.extend(crate::gallery_modal::input_dialog_row_cells());
    cells.extend(crate::gallery_modal::dialog_button_tier_cells());
    cells.extend(crate::gallery_movable::movable_cells());
    cells.extend(crate::gallery_movable::movable_action_cells());
    cells.extend(crate::gallery_movable::drag_tier_cells());
    cells.extend(crate::gallery_editor::editor_palette_cells());
    cells.push(crate::gallery_movable::zband_cell());
    cells.extend(crate::gallery_tooltip::tooltip_cells());
    cells.extend(crate::gallery_popover::popover_cells());
    cells.extend(not_owned_cells());
    cells
}

#[cfg(test)]
#[path = "gallery_tests.rs"]
mod gallery_tests;
