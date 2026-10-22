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

use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::geometry::clamp::ResizeHandle;
use crate::state::BaseState;
use crate::state::InteractionInput;
use crate::state::VisualState;
use crate::surface::input_dialog::{self, InputDialogHost};
use crate::surface::menu::model::Menu;
use crate::surface::menu::model::Row;
use crate::surface::menu::view as menu_view;
use crate::surface::modal::{self, ModalError, ModalFrame, ModalHost, ModalSizing};
use crate::surface::movable::{
    AutoScrollSetting, MemoryGeometryStorage, MovableAnatomy, MovableGeometry, MovableHost,
    MovableSpec, PointerPhase, ScrollGeometry, SurfaceTarget,
};
use crate::surface::zorder::{SurfaceId, SurfaceKind, SurfaceRegistry, ZBand};
use crate::token::color::Palette;
use crate::token::contrast;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::button::{self, ButtonVariant};
use crate::widget::button_widget::Layer;
use crate::widget::button_widget::background_layer as button_background_layer;
use crate::widget::button_widget::focus_ring_layer as button_focus_ring_layer;
use crate::widget::text_input::{self, InputKind, SlotContent};

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
    /// A colour channel.
    #[must_use]
    pub fn colour(channel: &'static str, colour: Color) -> Self {
        Self {
            channel,
            value: StyleValue::Colour {
                token: token_name(colour),
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
                token_name(pair.foreground),
                token_name(pair.background),
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

/// The token table a resolved colour is named against.
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
        ("accent.primary", p.accent_primary),
        ("accent.text", p.accent_text),
        ("text.primary", p.text_primary),
        ("text.strong", p.text_strong),
        ("text.muted", p.text_muted),
        ("text.disabled", p.text_disabled),
        ("status.success", p.status_success),
        ("status.warning", p.status_warning),
        ("status.danger", p.status_danger),
        ("status.danger_fill", p.status_danger_fill),
    ]
}

/// Name the token a resolved colour came from.
///
/// A fully transparent colour is `transparent` — the word the spec's own
/// matrices print for the Ghost resting fill, not a token lookup. A colour
/// whose channels match a token at a different alpha is named
/// `token @ NN%`, which is how the pressed fills read.
///
/// Anything else is `UNMAPPED`, and the gallery sidecar treats that string
/// as a failure: it means the matrix emitted a colour the token set does
/// not contain, which `UIF-VER-001` forbids outright.
#[must_use]
pub fn token_name(color: Color) -> String {
    if color.a == 0.0 {
        return "transparent".to_string();
    }
    for (name, token) in palette_table() {
        if same_channels(color, token) {
            if (color.a - token.a).abs() < CHANNEL_EPS {
                return name.to_string();
            }
            return format!("{name} @ {:.0}%", color.a * 100.0);
        }
    }
    format!("UNMAPPED {}", rgb8(color))
}

/// A [`Background`] as the manifest renders it.
fn background_name(background: &Background) -> String {
    match background {
        Background::Color(colour) => token_name(*colour),
        Background::Gradient(_) => "gradient".to_string(),
    }
}

/// A [`Border`] as the manifest renders it — `none` when nothing is stroked.
fn border_name(border: &Border) -> String {
    if border.width == 0.0 {
        "none".to_string()
    } else {
        token_name(border.color)
    }
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
pub fn paint_frame(viewport: Rectangle, layers: &[(&'static str, Layer)]) -> Frame {
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
            fill: background_name(&background),
            border: border_name(&border),
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

/// The sample box an atom cell is rendered in.
///
/// Wide enough that the widest atom chrome — a search field's reserved
/// `size.control.sm` slot plus its padding — still leaves real text area,
/// and exactly one control tall so the cell is the atom and nothing else.
#[must_use]
pub fn atom_sample() -> Rectangle {
    Rectangle::new(Point::ORIGIN, Size::new(space::XL * 10.0, size::CONTROL_MD))
}

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
    let palette = Palette::dark();
    let style = button::resolve_style(variant, state, &palette);
    let bounds = atom_sample();

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
        id: format!("button/{}/{}", slug(variant), state_slug(state)),
        kind: CellKind::Atom,
        subject: "button",
        spec: "UIF-ATOM-001",
        resolved: vec![
            StyleEntry::colour("background", style.background),
            StyleEntry::colour("foreground", style.foreground),
            StyleEntry::colour("border.color", style.border_color),
            StyleEntry::number("border.width", style.border_width),
            StyleEntry::colour("focus-ring.color", style.focus_ring_color),
            StyleEntry::number("focus-ring.width", style.focus_ring_width),
            StyleEntry::number("geometry.height", bounds.height),
            StyleEntry::number("geometry.radius", button::RADIUS),
            StyleEntry::number("geometry.padding-x", button::HORIZONTAL_PADDING),
            StyleEntry::text(
                "transparent-background",
                style.is_transparent_background().to_string(),
            ),
        ],
        frame: paint_frame(bounds, &layers),
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

/// Every button cell: four variants across every reachable state.
#[must_use]
pub fn button_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for variant in BUTTON_VARIANTS {
        for state in snapshot_states() {
            cells.push(button_cell(variant, state));
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

/// An [`InputKind`] as a cell-id slug.
fn slug_kind(kind: InputKind) -> &'static str {
    match kind {
        InputKind::TextEntry => "text-entry",
        InputKind::Search => "search",
        InputKind::DropdownTrigger => "dropdown-trigger",
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
    let palette = Palette::dark();
    let style = text_input::resolve_input_style(kind, state, &palette);
    let bounds = atom_sample();
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
        id: format!("input/{}/{}", slug_kind(kind), state_slug(state)),
        kind: CellKind::Atom,
        subject: "input",
        spec: "UIF-ATOM-002",
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
            StyleEntry::number("geometry.radius", text_input::RADIUS),
        ],
        frame: paint_frame(bounds, &layers),
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

/// Every text input cell: three kinds across every reachable state.
#[must_use]
pub fn text_input_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for kind in INPUT_KINDS {
        for state in snapshot_states() {
            cells.push(text_input_cell(kind, state));
        }
    }
    cells
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
    if let Some(layer) = menu_view::focus_ring_layer(bounds, &style) {
        layers.push(("focus-ring", layer));
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

    if style.focus_ring_width > 0.0 {
        audit.push(AuditPair {
            channel: "ring",
            foreground: style.focus_ring_color,
            background: style.background,
            role: ContrastRole::Chrome,
        });
    }

    Cell {
        id: format!("menu/row/{row_slug}", row_slug = row_slug(state)),
        kind: CellKind::Composite,
        subject: "menu",
        spec: "UIF-CMENU-001",
        resolved: vec![
            StyleEntry::colour("background", style.background),
            StyleEntry::colour("leading", style.leading),
            StyleEntry::colour("label", style.label),
            StyleEntry::colour("trailing", style.trailing),
            StyleEntry::colour("focus-ring.color", style.focus_ring_color),
            StyleEntry::number("focus-ring.width", style.focus_ring_width),
            StyleEntry::text("cursor", format!("{:?}", style.cursor)),
            StyleEntry::text("matrix-row", state.matrix_row()),
            StyleEntry::text("row-kind", format!("{:?}", row.kind())),
            StyleEntry::number("geometry.height", bounds.height),
            StyleEntry::number("geometry.padding-x", menu_view::ROW_PADDING_X),
            StyleEntry::number("geometry.radius", menu_view::ROW_RADIUS),
        ],
        frame: paint_frame(bounds, &layers),
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
            foreground: palette.status_danger,
            background: frame.surface,
            role: ContrastRole::Text,
        });
    }

    Cell {
        id: format!("modal/{name}"),
        kind: CellKind::Composite,
        subject: "modal",
        spec: "UIF-MOD-001",
        resolved,
        frame: paint_frame(viewport, &layers),
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
        resolved.push(StyleEntry::colour("status.danger", palette.status_danger));
    }

    Cell {
        id: format!("input-dialog/{name}"),
        kind: CellKind::Composite,
        subject: "input-dialog",
        spec: "UIF-INP-001",
        resolved,
        frame: paint_frame(viewport, &layers),
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

/// The movable-overlay cell for one gesture (`UIF-MOV-001` … `UIF-MOV-004`).
///
/// Opened through [`MovableHost::open`] against a [`MemoryGeometryStorage`]
/// so the clamp, the Band 1 registration and the modeless focus
/// registration are the real ones, then driven through
/// [`MovableHost::on_pointer`] — press then move — rather than by asserting
/// the drag flag directly. The cell therefore records `is_dragging()` and
/// `resizing_handle()` as the host actually reports them after a real
/// gesture, not as values the test wrote.
fn movable_cell(name: &str, gesture: MovableGesture) -> Cell {
    let palette = Palette::dark();
    let anatomy = MovableAnatomy::resolve(&palette);
    let viewport = viewport();

    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let mut storage = MemoryGeometryStorage::new();

    let spec = MovableSpec::new("gallery.movable", "Background Operations Log")
        .with_geometry(MovableGeometry::new(
            Point::new(space::XL * 4.0, space::XL * 3.0),
            Size::new(space::XL * 16.0, space::XL * 11.0),
        ))
        .with_auto_scroll(AutoScrollSetting::Enabled);

    let mut host = MovableHost::open(spec, viewport, &mut focus, &mut registry, &mut storage);

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
                &mut focus,
                &mut registry,
                &mut storage,
            );
            let _ = host.on_pointer(
                PointerPhase::Moved,
                to,
                SurfaceTarget::TitleBar,
                &mut focus,
                &mut registry,
                &mut storage,
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
                    &mut focus,
                    &mut registry,
                    &mut storage,
                );
                let _ = host.on_pointer(
                    PointerPhase::Moved,
                    to,
                    target,
                    &mut focus,
                    &mut registry,
                    &mut storage,
                );
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
        resolved,
        frame: paint_frame(viewport, &layers),
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
// Cell builder — the z-band stack (UIF-ZORD-001 … 003, G-13)
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
        spec: "UIF-ZORD-001",
        resolved,
        frame: paint_frame(viewport, &layers),
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
    cells.extend(button_cells());
    cells.extend(text_input_cells());
    cells.extend(menu_cells());
    cells.extend(modal_cells());
    cells.extend(input_dialog_cells());
    cells.extend(movable_cells());
    cells.push(zband_cell());
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
                let name = token_name(colour);
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
                let name = token_name(colour);
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
                token_name(pair.foreground),
                token_name(pair.background),
                pair.role,
                fixed(ratio),
                rgb8(painted),
            ));
        }
    }
    out
}

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

    out
}

#[cfg(test)]
#[path = "gallery_tests.rs"]
mod gallery_tests;
