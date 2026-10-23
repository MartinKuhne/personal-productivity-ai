//! The movable overlay host — a Band-1, non-blocking, draggable card for
//! live log and inspector surfaces (`UIF-MOV-001` … `UIF-MOV-004`).
//!
//! # What this is
//!
//! §4.7 gives two surfaces a shape the rest of the crate does not have: the
//! `Background Operations Log` and the `Agent Debug Inspector`. Both are
//! watched *while* the user is doing something else — reading a document,
//! running a prompt — so the one thing they must never do is stand between
//! the user and the workspace. That rules out the modal band's whole
//! mechanism: no scrim, no focus trap, no blocking.
//!
//! What is left is a card that floats over Band 0, moves when the user
//! grabs its title bar, lets every click outside itself fall straight
//! through, raises itself when clicked, and follows a live stream only as
//! long as the user is not looking somewhere else.
//!
//! Like the modal host, this is **not** a widget. It is the frame: for every
//! pointer press, every pointer move and every key it answers what happened,
//! from geometry and declared state, with no renderer in sight.
//!
//! # Anatomy and box model (§4.7.1)
//!
//! Every dimension below is either a token read or a named constant, and the
//! constants' own doc comments say which. Nothing in this module computes a
//! size from a number that is not one of them.
//!
//! ```text
//! ┌─ Card ──────────────────────────────────────────── 680×440 default ──┐
//! │  min 420×260 · surface.overlay · 1px border.strong · radius.card     │
//! │ ┌─ Title bar ─────────────────────────────── 32px = size.control.md ─┐│
//! │ │ 0/12px pad · surface.active · cursor Grab → Grabbing               ││
//! │ │ [∷ 16px] [Title 13px Bold text.strong]  ←fill→  [summary][⚓][× ] ││
//! │ │                                       └ trailing actions: 24px ×2 ││
//! │ ├─ Filter & control bar ──────────────────── 36px = size.control.lg ─┤│
//! │ │ 4/12px pad · surface.raised · gap 8px = space.sm                  ││
//! │ │ [Category ▾] [Search filter] [☑ Auto-scroll]  ←fill→  [Copy][Clear]││
//! │ ├─ Scrollable content viewport ─────────────────── fill (348px @440) ┤│
//! │ │ surface.sunken · monospace 12px / 13px                             ││
//! │ │ timestamp text.muted · [Category] tag accent.text · payload         ││
//! │ └─ Footer status & resize bar ────────────────────────────── 24px ───┘│
//! │   0/12px pad · surface.raised ·                    [◢ grip 16px]     │
//! └──────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! The content viewport is never declared: it is whatever the card has left
//! after the three bars, [`CONTENT_VIEWPORT_MIN`] aside. A card's height is
//! user-controlled, so the viewport is derived rather than sized.
//!
//! # The four invariants, and where each one lives
//!
//! * **[`UIF-MOV-001`](#uif-mov-001--the-title-bar-is-the-only-grip)** —
//!   the drag is not a rule about where a press may start a move; it is
//!   [`zone_at`](MovableHost::zone_at) returning [`Zone::TitleDrag`] for the title bar *minus*
//!   the trailing action cluster, and nothing else in the host can reach
//!   the drag state. Every frame goes through
//!   [`clamp::drag_position`], which is where the no-accumulation contract
//!   actually lives — this module does not re-implement a clamp and could
//!   not, because it holds no clamp of its own.
//! * **[`UIF-MOV-002`](#uif-mov-002--reachability-is-unrepresentable)** —
//!   there is exactly one private route to a new geometry, `apply_geometry`,
//!   and it clamps on the way in and `debug_assert`s the invariant on the
//!   way out. An unreachable card is not forbidden by convention; it cannot
//!   be constructed.
//! * **[`UIF-MOV-003`](#uif-mov-003--the-card-owns-its-inside-and-nothing-outside-it)** —
//!   the host never decides on its own that a click was its own. It asks
//!   [`SurfaceRegistry::route_pointer`] first, and only a route that names
//!   this surface reaches the classification code. Everything else is
//!   reported as a pass-through with the route attached. Promotion is gated
//!   on [`ZBand::promotes_on_click`], so it cannot be applied to a band
//!   the spec did not ask for it in.
//! * **[`UIF-MOV-004`](#uif-mov-004--following-is-a-state-machine-not-a-checkbox)** —
//!   [`FollowState::resolve`] is the whole pause/resume matrix as a pure
//!   function of the two inputs the spec names, and the host's follow state
//!   is only ever recomputed through it.
//!
//! # Persistence is injected (`D-2`, `RUST-024`)
//!
//! §4.7.2 asks for position and dimensions to survive open/close toggles
//! *and* application restarts. The toggle half is this module's job; the
//! restart half belongs to the application, because only the application
//! knows where it is allowed to write.
//!
//! So the framework never touches a storage location. [`GeometryStorage`]
//! is a trait the *application* implements and hands in — the host calls
//! `load` when a card opens and `save` when a geometry settles, and has no
//! idea whether the other end is a registry key, a JSON file, a database or
//! nothing at all. [`NoopGeometryStorage`] and [`MemoryGeometryStorage`]
//! are the only two sinks the framework ships, and neither one can reach a
//! disk. The value that crosses the boundary, [`MovableGeometry`], is a
//! plain serialisable struct with a lossless text form, so the application
//! can put it anywhere without this crate owning a serialiser.
//!
//! # `UIF-MOV-004` — following is a state machine, not a checkbox
//!
//! The tempting implementation of "auto-scroll" is a boolean: *enabled →
//! scroll to the bottom on every append*. That is the single most annoying
//! failure mode a live log can have. The user scrolls up to read a stack
//! trace, the next heartbeat arrives, and the text they were reading is
//! gone — yanked off the page by a machine that could not tell "watching"
//! from "reading". A log that does that is worse than no log, because the
//! user stops trusting it with their attention.
//!
//! The spec's rule is a conjunction with a memory in it: follow **only**
//! when the setting is on **and** the viewport is *already* at the bottom.
//! Scrolling up pauses following; scrolling back to the bottom resumes it.
//! That is a two-input state machine, so it is modelled as one:
//!
//! ```text
//!                        ┌───────────────┐
//!   setting = Disabled ─▶│    Paused     │◀─ any input with setting off
//!                        └──────┬────────┘
//!              setting on AND at bottom
//!                               ▼
//!                        ┌───────────────┐
//!                        │  Following    │── append → scroll to bottom
//!                        └───────────────┘
//! ```
//!
//! [`FollowState::resolve`] *is* the matrix — four cells, exhaustively
//! tested — and the host's stored state is only ever the result of passing
//! the current `(setting, at_bottom)` through it. `at_bottom` is derived
//! from [`ScrollGeometry`] with a [`AT_BOTTOM_TOLERANCE`] of one
//! [`space::XXS`], because a scrollable that lands 0.4px short of the
//! bottom *is* at the bottom and must not pause the stream over a rounding
//! error.
//!
//! Entries that arrive while paused are counted in [`MovableHost::behind`]
//! rather than dropped, so a view can show "12 new below" and a resume can
//! land the user somewhere honest.
//!
//! # Decisions this increment made
//!
//! * **The clamp solver is the only clamp.** Position, resize and the
//!   viewport-change re-clamp all go through
//!   [`clamp::drag_position`] / [`clamp::resize`] / [`clamp::clamp_rect`]
//!   with [`ClampRule::MOVABLE`]. A second clamp written here would be a
//!   second opinion about where the title bar has to be, and the two would
//!   eventually disagree.
//! * **A drag keeps its capture.** Once a drag or resize starts, the host
//!   keeps handling `Moved` and `Released` even when the pointer runs off
//!   the card and the registry would route the event to the workspace.
//!   Without that, dragging fast enough to outrun the card would drop the
//!   card mid-gesture — the exact opposite of direct manipulation.
//! * **The grip band is the body's border, not the card's.** §4.7.1 paints
//!   one `16px` ◢ corner. The eight-handle layout the clamp solver
//!   supports is derived by giving every edge of the *body* — everything
//!   below the title bar — a `16px` strip. The three top handles therefore
//!   have no pointer grip, because the title bar owns the top edge and
//!   `UIF-MOV-001` gives the whole of that bar to the drag. They remain
//!   reachable through [`resize_by`](MovableHost::resize_by), which is the handle-explicit API.
//! * **Geometry outranks the caller's report in both directions**, exactly
//!   as the modal host does it: a `TitleBar` report on the Close button's
//!   rectangle does not start a drag, and a `Close` report on the title
//!   text does not close the card — it drags it, which is what the pointer
//!   is actually doing.
//! * **Promotion changes nothing else.** It does not move focus, does not
//!   change geometry, does not resume a paused stream. The click that
//!   promoted the card also does its own job (starts a drag, fires a
//!   control), and those two are reported separately rather than one being
//!   folded into the other.
//! * **A movable card never takes focus when it opens.** It is modeless;
//!   stealing the keyboard on open would contradict the whole reason it is
//!   not a modal. Its controls become focusable because its group is
//!   registered, and the user focuses them by clicking one.
//! * **Escape closes only a focused card** (§5 Band 1), and the host does
//!   not enforce that alone: [`ZBand::UtilityOverlay`] has
//!   `escape_rank` `1`, below `Modal` (`2`) and `Menu` (`3`), so the
//!   registry's walk asks those first and the card never steals the key
//!   from a surface above it (**G-13**).
//! * **Persistence happens at settled geometry** — drag release, resize
//!   release, close, programmatic set, viewport re-clamp — and never per
//!   frame. A save per frame would turn the injected sink into an I/O hot
//!   path, which is the thing `NFR-002` exists to keep off the main thread.
//!
//! Unit tests live in the sibling `movable_tests.rs` sidecar.

use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::focus::GroupId;
use crate::geometry::clamp;
use crate::geometry::clamp::ClampRule;
use crate::geometry::clamp::ResizeHandle;
use crate::surface::drag_tier::DragGesture;
use crate::surface::zorder::DeclineReason;
use crate::surface::zorder::EscapeOutcome;
use crate::surface::zorder::KeyClass;
use crate::surface::zorder::KeyDisposition;
use crate::surface::zorder::PointerRoute;
use crate::surface::zorder::SurfaceId;
use crate::surface::zorder::SurfaceKind;
use crate::surface::zorder::SurfaceRegistry;
use crate::surface::zorder::UnwindAnswer;
use crate::surface::zorder::ZBand;
use crate::token::color::Palette;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::token::typography;

use iced::Color;
use iced::Point;
use iced::Rectangle;
use iced::Size;
use iced::Vector;
use iced::mouse;

// ---------------------------------------------------------------------------
// Anatomy constants (§4.7.1)
// ---------------------------------------------------------------------------

/// §4.7.1 — the card's default width.
///
/// **Named constant, not a token.** [`crate::token::size`] sizes controls
/// and icons, not whole surfaces; there is no rung for "how wide a log card
/// wants to be", and inventing one would put a surface dimension on a
/// control scale. The *minimum* is not a constant here because it is not
/// this module's to own — see [`CARD_MIN_WIDTH`].
pub const CARD_DEFAULT_WIDTH: f32 = 680.0;

/// §4.7.1 — the card's default height. **Named constant**, same reason as
/// [`CARD_DEFAULT_WIDTH`].
pub const CARD_DEFAULT_HEIGHT: f32 = 440.0;

/// §4.7.1 — the card's minimum width, read from the clamp solver's own
/// `UIF-MOV-002` size policy rather than restated here.
///
/// Reading it instead of re-declaring `420.0` is the point: the host and
/// the solver cannot end up disagreeing about the floor, and a resize that
/// stops at the host's floor is the same stop the solver enforces.
pub const CARD_MIN_WIDTH: f32 = clamp::SizePolicy::MOVABLE.min.width;

/// §4.7.1 — the card's minimum height, from the same policy.
pub const CARD_MIN_HEIGHT: f32 = clamp::SizePolicy::MOVABLE.min.height;

/// §4.7.1 — the draggable title bar is `32px`: **token**,
/// [`size::CONTROL_MD`], the same rung `INC-13` reads for the title bar in
/// the clamp solver.
pub const TITLE_BAR_HEIGHT: f32 = size::CONTROL_MD;

/// §4.7.1 — the title bar's vertical padding is `0`. A constant rather
/// than a bare `0.0` so the box model reads as `[0, 12px]` on both axes.
pub const TITLE_BAR_PADDING_VERTICAL: f32 = 0.0;

/// §4.7.1 — the title bar's horizontal padding is `12px`: **token**,
/// [`space::MD`].
pub const TITLE_BAR_PADDING_HORIZONTAL: f32 = space::MD;

/// §4.7.1 — the `∷` grip glyph's box is `16px`: **token**,
/// [`size::ICON_MD`].
///
/// The spec draws the grip but does not size it. `size.icon.md` is the
/// rung for a standard inline glyph, which is exactly what this is.
pub const TITLE_GRIP_WIDTH: f32 = size::ICON_MD;

/// §4.7.1 — the title's font size is `13px`: **token**,
/// [`typography::UI_COMPACT`]'s size.
///
/// The other half of the pairing is [`TITLE_FONT_WEIGHT`]. What **G-12**
/// recorded as a composition — size read from the token, `Bold` bolted
/// on because `UI_COMPACT` rests at Regular and emphasises only to
/// Medium — is now legal by token: `type.ui_compact`'s `Allowed Weights`
/// column is Regular / Medium / SemiBold / **Bold**, so `Bold` sits
/// inside the set the rung permits. The sidecar asserts both halves
/// resolve through the scale and that the rendered size is unchanged.
pub const TITLE_FONT_SIZE: f32 = typography::UI_COMPACT.size;

/// §4.7.1 — the title's weight is `Bold`: **token-sanctioned**, a member
/// of [`typography::UI_COMPACT`]'s allowed-weight set.
///
/// Stated as its own constant so the overlay title's typography is two
/// token reads rather than a token plus an untracked assumption, and so
/// the pairing is checked against the scale instead of against habit.
///
/// This module is the host, not the renderer: it draws no text. The
/// weight travels here as the token value a view layer applies alongside
/// [`TITLE_FONT_SIZE`].
pub const TITLE_FONT_WEIGHT: typography::Weight = typography::Weight::Bold;

/// §4.7.1 — a trailing action control (`Dock`, `Close ×`) is `24px` wide:
/// **token**, [`size::HIT_TARGET_MIN`], the floor every interactive control
/// must clear.
///
/// Using the hit-target floor rather than a grip-sized value is what makes
/// the `UIF-MOV-001` exclusion zone honest: the zone is exactly as wide as
/// the controls that must not start a drag, and no wider.
pub const ACTION_CONTROL_SIZE: f32 = size::HIT_TARGET_MIN;

/// §4.7.1 — the gap between the trailing action controls is `8px`:
/// **token**, [`space::SM`], the standard sibling-control gap.
///
/// §4.7.1 prints no gap in the title bar. Leaving it at zero would butt
/// `Dock` against `Close` with no separation, so the scale's own sibling
/// gap is used rather than a new number.
pub const ACTION_GAP: f32 = space::SM;

/// §4.7.1 — the filter & control bar is `36px`: **token**,
/// [`size::CONTROL_LG`], the rung the token module already documents as a
/// primary structural bar.
pub const FILTER_BAR_HEIGHT: f32 = size::CONTROL_LG;

/// §4.7.1 — the filter bar's vertical padding is `4px`: **token**,
/// [`space::XS`].
pub const FILTER_BAR_PADDING_VERTICAL: f32 = space::XS;

/// §4.7.1 — the filter bar's horizontal padding is `12px`: **token**,
/// [`space::MD`].
pub const FILTER_BAR_PADDING_HORIZONTAL: f32 = space::MD;

/// §4.7.1 — the gap between filter controls is `8px`: **token**,
/// [`space::SM`].
pub const FILTER_BAR_GAP: f32 = space::SM;

/// §4.7.1 — the content viewport's small monospace size is `12px`:
/// **token**, [`typography::CAPTION`]'s size.
pub const CONTENT_FONT_SMALL: f32 = typography::CAPTION.size;

/// §4.7.1 — the content viewport's large monospace size is `13px`:
/// **token**, [`typography::UI_COMPACT`]'s size.
///
/// The monospace *family* is a token now too —
/// [`typography::FAMILY_MONO`] (`font.family.mono`) — so a view layer
/// pairs that family with this size rather than picking a family out of
/// thin air. This module still vouches for the size only: it draws
/// nothing.
pub const CONTENT_FONT_LARGE: f32 = typography::UI_COMPACT.size;

/// §4.7.1 — the footer status & resize bar is `24px`: **token**,
/// [`size::BAR_STATUS`], the rung `size.bar.status` defines for a compact
/// status and footer bar and names this footer for.
///
/// This used to be a `22.0` literal, recorded as a gap the token ladder
/// left behind — "the control rungs are 24 / 28 / 32 / 36 and there is
/// nothing at 22". The 2026-10-22 revision closed that gap by adding the
/// rung, so the footer reads it instead of inventing a height.
///
/// The card's total chrome is *derived* from this constant
/// ([`CHROME_HEIGHT`]), so the overlay stands 2px taller than it did
/// before the rung existed, and the content viewport — which is whatever
/// the card has left over — is 2px shorter. Both are recomputed, never
/// restated.
pub const FOOTER_HEIGHT: f32 = size::BAR_STATUS;

/// §4.7.1 — the footer's vertical padding is `0`. See
/// [`TITLE_BAR_PADDING_VERTICAL`] for why zero is named.
pub const FOOTER_PADDING_VERTICAL: f32 = 0.0;

/// §4.7.1 — the footer's horizontal padding is `12px`: **token**,
/// [`space::MD`].
pub const FOOTER_PADDING_HORIZONTAL: f32 = space::MD;

/// §4.7.1 — the ◢ resize grip in the bottom-right corner is `16px`, and
/// §2.2 already assigns that figure to a token: `size.icon.md` is defined
/// as the optical size for the "standard button, menu leading slot,
/// **resize grip glyph**, and tree item icon".
///
/// The re-point changes no rendered value — `size::ICON_MD` *is* `16.0` —
/// only where the number is permitted to come from. `UIF-PRIN-001` admits
/// a visual measurement through a token or a registered Surface Geometry
/// Constant and nothing else, and §2.2 admits this one, so borrowing the
/// rung is the spec's own answer rather than a loan against it.
pub const RESIZE_HANDLE_SIZE: f32 = size::ICON_MD;

/// The three bars every card height has to pay for before any content fits:
/// the title bar, the filter bar and the footer.
pub const CHROME_HEIGHT: f32 = TITLE_BAR_HEIGHT + FILTER_BAR_HEIGHT + FOOTER_HEIGHT;

/// The content viewport's floor.
///
/// **Derived, not declared.** §4.7.1 gives the viewport `Height: Fill`, so
/// it is whatever the card has left over. This is the smallest card
/// (`420 × 260`) minus its chrome — the point at which the viewport stops
/// being usable and the card's own minimum has stopped meaning anything.
/// It exists so a caller can ask whether a card is too short to read
/// without doing the arithmetic itself.
pub const CONTENT_VIEWPORT_MIN: f32 = CARD_MIN_HEIGHT - CHROME_HEIGHT;

/// How close to the bottom of its content counts as *at* the bottom
/// (`UIF-MOV-004`).
///
/// **Token composition**: one [`space::XXS`]. A scrollable that settles
/// 0.4px short of the bottom is at the bottom, and a strict `>=` would
/// pause a live stream over a sub-pixel rounding error — the failure this
/// invariant exists to prevent, arriving from the opposite direction.
pub const AT_BOTTOM_TOLERANCE: f32 = space::XXS;

// ---------------------------------------------------------------------------
// The serialisable geometry value
// ---------------------------------------------------------------------------

/// The card's position and size, as one serialisable value
/// (`UIF-MOV-001`'s "position and dimensions").
///
/// A value rather than two fields scattered across the host because it is
/// the thing that crosses the persistence boundary: the application saves
/// it, restores it, and this crate never sees the storage. [`encode`] and
/// [`decode`] give a lossless canonical text form — Rust's float `Display`
/// prints the shortest string that parses back to the same `f32`, so a
/// round trip cannot drift — which keeps the framework out of the
/// serialisation business entirely. An application that wants JSON can
/// serialise these four numbers with whatever it already uses.
///
/// [`encode`]: MovableGeometry::encode
/// [`decode`]: MovableGeometry::decode
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovableGeometry {
    /// The card's top-left corner, in root coordinates.
    pub position: Point,
    /// The card's size.
    pub size: Size,
}

impl MovableGeometry {
    /// §4.7.1's default card: `680 × 440` at the origin.
    pub const DEFAULT: Self = Self {
        position: Point::new(0.0, 0.0),
        size: Size::new(CARD_DEFAULT_WIDTH, CARD_DEFAULT_HEIGHT),
    };

    /// Build a geometry from a position and a size.
    #[must_use]
    pub const fn new(position: Point, size: Size) -> Self {
        Self { position, size }
    }

    /// The card's rectangle.
    #[must_use]
    pub const fn rect(self) -> Rectangle {
        Rectangle::new(self.position, self.size)
    }

    /// The same geometry at a different position.
    #[must_use]
    pub const fn with_position(self, position: Point) -> Self {
        Self { position, ..self }
    }

    /// The same geometry at a different size.
    #[must_use]
    pub const fn with_size(self, size: Size) -> Self {
        Self { size, ..self }
    }

    /// A copy with non-finite coordinates rewritten to `0.0` and negative
    /// extents to `0.0`.
    ///
    /// A `NaN` position parses out of a corrupt saved value just as easily
    /// as a real one, and iced propagates it into collapsed layout. Landing
    /// on zero keeps the card visible and the failure debuggable.
    #[must_use]
    pub fn sanitized(self) -> Self {
        Self {
            position: Point::new(finite(self.position.x), finite(self.position.y)),
            size: Size::new(
                finite(self.size.width).max(0.0),
                finite(self.size.height).max(0.0),
            ),
        }
    }

    /// The canonical text form: `x,y,width,height`.
    ///
    /// Lossless for every finite geometry, and stable — the same value
    /// always encodes to the same string, which is what makes it usable as
    /// a comparison key.
    #[must_use]
    pub fn encode(self) -> String {
        format!(
            "{},{},{},{}",
            self.position.x, self.position.y, self.size.width, self.size.height
        )
    }

    /// Parse a [`encode`](Self::encode) form back.
    ///
    /// Rejects anything that is not exactly four finite numbers with
    /// non-negative extents, including the `NaN` and `inf` strings that
    /// `f32::parse` accepts. A saved value that cannot be trusted is worse
    /// than no saved value at all: the caller falls back to the default
    /// card, which is always reachable.
    #[must_use]
    pub fn decode(text: &str) -> Option<Self> {
        let parts: Vec<&str> = text.split(',').map(str::trim).collect();
        if parts.len() != 4 {
            return None;
        }

        let mut values = [0.0f32; 4];
        for (slot, part) in values.iter_mut().zip(parts) {
            let parsed = part.parse::<f32>().ok()?;
            if !parsed.is_finite() {
                return None;
            }
            *slot = parsed;
        }

        let geometry = Self {
            position: Point::new(values[0], values[1]),
            size: Size::new(values[2], values[3]),
        };
        if geometry.size.width < 0.0 || geometry.size.height < 0.0 {
            return None;
        }

        Some(geometry)
    }
}

// ---------------------------------------------------------------------------
// The injected persistence sink (D-2 / RUST-024)
// ---------------------------------------------------------------------------

/// The injected geometry sink (`RUST-024`, decision **D-2**).
///
/// The application implements this; the framework only calls it. That is
/// the whole of the arrangement, and it is what keeps a UI framework from
/// deciding where a user's data lives: the host knows *that* a geometry
/// should be remembered and *when* it settled, and nothing about how.
///
/// The host calls [`load`](Self::load) once, when a card opens, and
/// [`save`](Self::save) when a geometry settles — never per frame, so the
/// sink is not on a hot path.
pub trait GeometryStorage {
    /// The geometry saved for `key`, if any.
    fn load(&mut self, key: &str) -> Option<MovableGeometry>;

    /// Remember `geometry` under `key`.
    fn save(&mut self, key: &str, geometry: MovableGeometry);

    /// Forget whatever is saved under `key`.
    fn clear(&mut self, key: &str);
}

/// A sink that remembers nothing (`RUST-006`'s noop storage).
///
/// The right choice for a test, a gallery, or an application that has
/// decided its overlay positions are not worth persisting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoopGeometryStorage;

impl GeometryStorage for NoopGeometryStorage {
    fn load(&mut self, _key: &str) -> Option<MovableGeometry> {
        None
    }

    fn save(&mut self, _key: &str, _geometry: MovableGeometry) {}

    fn clear(&mut self, _key: &str) {}
}

/// A sink that keeps values in memory and nowhere else.
///
/// Genuinely useful for an application that wants positions to survive a
/// session but not a restart, and for tests that need a real round trip
/// without a filesystem. The storage is a plain map of owned strings: there
/// is no path, no handle and no writer in this type, which is what makes
/// "the framework wrote nothing to a user location" a property of the code
/// rather than a promise.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemoryGeometryStorage {
    entries: Vec<(String, MovableGeometry)>,
}

impl MemoryGeometryStorage {
    /// An empty in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many keys currently hold a value.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The stored keys, in the order they were first written.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(key, _)| key.as_str())
    }
}

impl GeometryStorage for MemoryGeometryStorage {
    fn load(&mut self, key: &str) -> Option<MovableGeometry> {
        self.entries
            .iter()
            .find(|(stored, _)| stored == key)
            .map(|(_, geometry)| *geometry)
    }

    fn save(&mut self, key: &str, geometry: MovableGeometry) {
        match self.entries.iter_mut().find(|(stored, _)| stored == key) {
            Some(slot) => slot.1 = geometry,
            None => self.entries.push((key.to_owned(), geometry)),
        }
    }

    fn clear(&mut self, key: &str) {
        self.entries.retain(|(stored, _)| stored != key);
    }
}

/// The storage key a card's geometry is saved under.
///
/// Derived from the surface id rather than supplied, so two cards cannot
/// collide by a copy-pasted literal and so the host and the application
/// cannot disagree about which key belongs to which card.
#[must_use]
pub fn storage_key(id: &SurfaceId) -> String {
    format!("movable:{}", id.as_str())
}

/// The focus group a card of this id owns.
///
/// Same derivation rule as the modal host's [`group_for`](super::modal::group_for):
/// the group is named after the surface, not chosen by the caller.
#[must_use]
pub fn group_for(id: &SurfaceId) -> GroupId {
    GroupId::new(format!("movable:{}", id.as_str()))
}

// ---------------------------------------------------------------------------
// Resolved anatomy
// ---------------------------------------------------------------------------

/// The §4.7.1 anatomy resolved against a palette.
///
/// The dimensions are the module constants above; what this adds is the
/// colour half, resolved once so the view layer cannot re-derive a surface
/// or retype a stroke (`UIF-VER-001`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovableAnatomy {
    /// The card's own surface — `color.surface.overlay`.
    pub card_surface: Color,
    /// The card perimeter — `color.border.strong`.
    pub card_border: Color,
    /// The perimeter weight — `shape::BORDER_HAIRLINE`.
    pub card_border_width: f32,
    /// The card radius — `shape::RADIUS_CARD`, the rung whose own doc
    /// comment names movable overlay windows.
    pub card_radius: f32,
    /// The title bar's surface — `color.surface.active`.
    pub title_bar_surface: Color,
    /// The filter bar's and footer's surface — `color.surface.raised`.
    pub bar_surface: Color,
    /// The content viewport's surface — `color.surface.sunken`.
    pub content_surface: Color,
    /// The title text — `color.text.strong`.
    pub title_text: Color,
    /// Timestamps and footer status text — `color.text.muted`.
    pub meta_text: Color,
    /// The `[Category]` tag — `color.accent.text`.
    pub tag_text: Color,
    /// Bar dividers — `color.border.subtle`.
    pub divider: Color,
    /// The title bar height — [`TITLE_BAR_HEIGHT`].
    pub title_bar_height: f32,
    /// The title bar's horizontal padding — [`TITLE_BAR_PADDING_HORIZONTAL`].
    pub title_bar_padding_horizontal: f32,
    /// The filter bar height — [`FILTER_BAR_HEIGHT`].
    pub filter_bar_height: f32,
    /// The filter bar's vertical padding — [`FILTER_BAR_PADDING_VERTICAL`].
    pub filter_bar_padding_vertical: f32,
    /// The filter bar's gap — [`FILTER_BAR_GAP`].
    pub filter_bar_gap: f32,
    /// The footer height — [`FOOTER_HEIGHT`].
    pub footer_height: f32,
    /// The footer's horizontal padding — [`FOOTER_PADDING_HORIZONTAL`].
    pub footer_padding_horizontal: f32,
    /// The ◢ grip's size — [`RESIZE_HANDLE_SIZE`].
    pub resize_handle_size: f32,
    /// A trailing action control's width — [`ACTION_CONTROL_SIZE`].
    pub action_control_size: f32,
}

impl MovableAnatomy {
    /// Resolve the anatomy from a palette.
    #[must_use]
    pub const fn resolve(palette: &Palette) -> Self {
        Self {
            card_surface: palette.surface_overlay,
            card_border: palette.border_strong,
            card_border_width: shape::BORDER_HAIRLINE,
            card_radius: shape::RADIUS_CARD,
            title_bar_surface: palette.surface_active,
            bar_surface: palette.surface_raised,
            content_surface: palette.surface_sunken,
            title_text: palette.text_strong,
            meta_text: palette.text_muted,
            tag_text: palette.accent_text,
            divider: palette.border_subtle,
            title_bar_height: TITLE_BAR_HEIGHT,
            title_bar_padding_horizontal: TITLE_BAR_PADDING_HORIZONTAL,
            filter_bar_height: FILTER_BAR_HEIGHT,
            filter_bar_padding_vertical: FILTER_BAR_PADDING_VERTICAL,
            filter_bar_gap: FILTER_BAR_GAP,
            footer_height: FOOTER_HEIGHT,
            footer_padding_horizontal: FOOTER_PADDING_HORIZONTAL,
            resize_handle_size: RESIZE_HANDLE_SIZE,
            action_control_size: ACTION_CONTROL_SIZE,
        }
    }
}

// ---------------------------------------------------------------------------
// Auto-scroll (UIF-MOV-004)
// ---------------------------------------------------------------------------

/// What the user has set `[☑ Auto-scroll]` to (§4.7.1, `UIF-MOV-004`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AutoScrollSetting {
    /// The checkbox is on: following is *permitted*.
    Enabled,
    /// The checkbox is off: the view never follows on its own.
    Disabled,
}

impl AutoScrollSetting {
    /// Whether this setting permits following at all.
    #[must_use]
    pub const fn permits_following(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Whether the view is currently following the stream (`UIF-MOV-004`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FollowState {
    /// Appended entries are scrolled into view.
    Following,
    /// Appended entries are *not* scrolled into view.
    Paused,
}

impl FollowState {
    /// The whole pause/resume matrix, as a pure function of the two inputs
    /// `UIF-MOV-004` names.
    ///
    /// ```text
    ///   setting   at_bottom   result
    ///   ───────   ────────   ──────
    ///   Enabled     true     Following   ← the only cell that follows
    ///   Enabled     false    Paused      the user is reading something
    ///   Disabled    true     Paused      permission is off, position is moot
    ///   Disabled    false    Paused      permission is off
    /// ```
    ///
    /// Making the matrix a function of `(setting, at_bottom)` rather than a
    /// sequence of transitions is what makes it exhaustively testable: all
    /// four cells are reachable in one call each, and the host's stored
    /// state is only ever this function's output.
    #[must_use]
    pub const fn resolve(setting: AutoScrollSetting, at_bottom: bool) -> Self {
        match (setting, at_bottom) {
            (AutoScrollSetting::Enabled, true) => Self::Following,
            _ => Self::Paused,
        }
    }

    /// Whether this state follows the stream.
    #[must_use]
    pub const fn is_following(self) -> bool {
        matches!(self, Self::Following)
    }
}

/// Why a paused view stayed paused when entries arrived (`UIF-MOV-004`).
///
/// Two reasons, because the user needs to know which one it is: "you
/// turned this off" and "you are reading something" want different
/// affordances — a checkbox to flip versus a "jump to latest" button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HoldReason {
    /// Auto-scroll is disabled.
    Disabled,
    /// Auto-scroll is enabled but the viewport is not at the bottom.
    Inspecting,
}

/// What the host decided to do about a stream append (`UIF-MOV-004`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoScrollDecision {
    /// Scroll the viewport to the bottom so the new entries show.
    Follow {
        /// How many entries arrived.
        entries: usize,
    },
    /// Leave the viewport exactly where it is.
    Hold {
        /// How many entries arrived.
        entries: usize,
        /// Why the view did not follow.
        reason: HoldReason,
        /// How many entries are now waiting below the viewport.
        behind: usize,
    },
}

/// The content viewport's scroll geometry, as the view reports it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollGeometry {
    /// How far the viewport is scrolled from the top.
    pub offset: f32,
    /// The total height of the content.
    pub content: f32,
    /// The height of the viewport itself.
    pub viewport: f32,
}

impl ScrollGeometry {
    /// An empty viewport: nothing to scroll, therefore at the bottom.
    pub const EMPTY: Self = Self {
        offset: 0.0,
        content: 0.0,
        viewport: 0.0,
    };

    /// Build a scroll geometry.
    #[must_use]
    pub const fn new(offset: f32, content: f32, viewport: f32) -> Self {
        Self {
            offset,
            content,
            viewport,
        }
    }

    /// The largest legal offset.
    #[must_use]
    pub fn max_offset(self) -> f32 {
        (finite(self.content) - finite(self.viewport)).max(0.0)
    }

    /// Whether there is nothing to scroll.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.max_offset() <= 0.0
    }

    /// Whether the viewport sits at the bottom, within
    /// [`AT_BOTTOM_TOLERANCE`].
    ///
    /// An empty viewport counts as at the bottom: there is nowhere to go,
    /// and a log with three lines that fits on screen should still follow.
    #[must_use]
    pub fn at_bottom(self) -> bool {
        self.is_empty() || self.clamped().offset >= self.max_offset() - AT_BOTTOM_TOLERANCE
    }

    /// The same geometry with the offset pulled back into range and every
    /// non-finite value rewritten.
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            offset: finite(self.offset).clamp(0.0, self.max_offset()),
            content: finite(self.content).max(0.0),
            viewport: finite(self.viewport).max(0.0),
        }
    }

    /// The geometry after scrolling by `delta` (positive scrolls down).
    #[must_use]
    pub fn scrolled_by(self, delta: f32) -> Self {
        let delta = finite(delta);
        Self {
            offset: self.offset + delta,
            ..self
        }
        .clamped()
    }

    /// The geometry after scrolling to the bottom.
    #[must_use]
    pub fn at_bottom_now(self) -> Self {
        let max = self.max_offset();
        Self {
            offset: max,
            ..self.clamped()
        }
    }
}

// ---------------------------------------------------------------------------
// Pointer input
// ---------------------------------------------------------------------------

/// Which phase of a pointer interaction the host is being told about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerPhase {
    /// The button went down.
    Pressed,
    /// The pointer moved.
    Moved,
    /// The button came up.
    Released,
}

/// What the caller's own hit test says a pointer event landed on.
///
/// A report, not an order. The host checks it against its own rectangles and
/// lets geometry win in both directions — see the [module
/// docs](self#decisions-this-increment-made).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SurfaceTarget {
    /// Outside the card.
    Outside,
    /// Bare title bar — padding between slots.
    TitleBar,
    /// The `∷` grip glyph.
    TitleGrip,
    /// The title text.
    Title,
    /// The token / cost summary. Not a button, so it drags.
    Summary,
    /// The `Dock` button — a trailing action.
    Dock,
    /// The `Close ×` button — a trailing action.
    Close,
    /// Bare filter bar.
    FilterBar,
    /// The `☑ Auto-scroll` checkbox.
    AutoScrollToggle,
    /// A named control inside the card.
    Control(FocusId),
    /// The scrollable content viewport.
    Content,
    /// The footer status area.
    Footer,
    /// A resize grip.
    Resize(ResizeHandle),
}

/// The host's own geometric verdict on a point — computed from the host's
/// rectangles, never from the caller's report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Zone {
    /// Outside the card entirely.
    Outside,
    /// The draggable part of the title bar (`UIF-MOV-001`).
    TitleDrag,
    /// The trailing action cluster — `Dock` and `Close ×`, the part of the
    /// title bar `UIF-MOV-001` excludes.
    TitleActions,
    /// The filter & control bar.
    FilterBar,
    /// The scrollable content viewport.
    Content,
    /// The footer, excluding the resize grip.
    Footer,
    /// A resize grip.
    Grip(ResizeHandle),
}

/// A control the host can activate on a press.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CardControl {
    /// The title bar's `Dock` button.
    Dock,
    /// The filter bar's `☑ Auto-scroll` checkbox.
    AutoScrollToggle,
    /// A caller-named control.
    Custom(FocusId),
}

// ---------------------------------------------------------------------------
// Outcomes
// ---------------------------------------------------------------------------

/// An effect the caller must carry out; the host performs none itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MovableTask {
    /// Scroll the content viewport to the bottom (`UIF-MOV-004`).
    ScrollToBottom,
    /// Show this cursor over the card (`UIF-MOV-001`'s `Grab` / `Grabbing`).
    SetCursor(mouse::Interaction),
    /// Fire this control's command.
    Activate(FocusId),
}

/// Why the host did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovableNoOp {
    /// The card is not open.
    NotOpen,
    /// The card is open but not registered, so routing can never name it.
    NotRegistered,
    /// The key class is not one Band 1 swallows (§5 "Keyboard Routing").
    KeyNotTrapped,
    /// `Escape` reached the card but the card does not hold focus (§5 Band
    /// 1: "`Escape` does not close unless focused").
    NotFocused,
    /// The card is already being dragged or resized.
    AlreadyCapturing,
    /// The card is already open.
    AlreadyOpen,
    /// The event arrived with no gesture in progress — a move or a release
    /// with nothing captured, or a programmatic set that changed nothing.
    Idle,
    /// Another surface owns the point.
    OwnedByOtherSurface,
}

/// What one interaction with the movable host did.
#[derive(Clone, Debug, PartialEq)]
pub enum MovableAction {
    /// Nothing happened.
    NoOp(MovableNoOp),
    /// A press on the draggable title bar entered `Dragging`
    /// (`UIF-MOV-001`).
    DragStarted {
        /// The card's geometry at the moment the drag began.
        geometry: MovableGeometry,
    },
    /// A drag frame translated the card (`UIF-MOV-001`).
    Dragged {
        /// The card's new, clamped geometry.
        geometry: MovableGeometry,
        /// The pointer delta this frame was built from.
        delta: Vector,
    },
    /// The drag ended at a settled, persisted geometry (`UIF-MOV-001`).
    DragEnded {
        /// The settled geometry.
        geometry: MovableGeometry,
    },
    /// A press on a resize grip entered the resizing state
    /// (`UIF-MOV-002`).
    ResizeStarted {
        /// The handle being dragged.
        handle: ResizeHandle,
    },
    /// A resize frame changed the card (`UIF-MOV-002`).
    Resized {
        /// The handle being dragged.
        handle: ResizeHandle,
        /// The card's new, clamped geometry.
        geometry: MovableGeometry,
        /// The pointer delta this frame was built from.
        delta: Vector,
    },
    /// The resize ended at a settled, persisted geometry.
    ResizeEnded {
        /// The handle that was being dragged.
        handle: ResizeHandle,
        /// The settled geometry.
        geometry: MovableGeometry,
    },
    /// The event fell outside the card and belongs to the workspace
    /// underneath (`UIF-MOV-003`).
    PassedThrough {
        /// The route the event took, including which bands let it past.
        route: PointerRoute,
    },
    /// A control inside the card was pressed and should fire.
    Activated {
        /// Which control.
        control: CardControl,
    },
    /// A press inside the card that no control claimed — the card boundary
    /// took it.
    Absorbed {
        /// The zone that absorbed it.
        zone: Zone,
    },
    /// The content viewport scrolled (`UIF-MOV-004`).
    Scrolled {
        /// The follow state the new position produced.
        follow: FollowState,
    },
    /// The auto-scroll setting changed (`UIF-MOV-004`).
    AutoScrollChanged {
        /// The new setting.
        setting: AutoScrollSetting,
        /// The follow state it produced.
        follow: FollowState,
    },
    /// The card closed, at the user's or the caller's request.
    Closed {
        /// The geometry that was persisted on the way out.
        geometry: MovableGeometry,
    },
    /// The geometry settled at a value the clamp allowed (`UIF-MOV-002`).
    ///
    /// `adjusted` says whether the clamp actually moved something: a
    /// request that was already legal reports the geometry it was given,
    /// with `adjusted == false`.
    Reclamped {
        /// The geometry after the clamp.
        geometry: MovableGeometry,
        /// Whether the clamp changed anything.
        adjusted: bool,
    },
}

/// One interaction's full result.
///
/// The primary transition plus the two orthogonal facts a caller has to
/// know about a Band-1 card: whether it was raised, and whether the event
/// fell through to the workspace. Both are separate from the action because
/// both can accompany any action — the press that starts a drag also
/// promotes the card, and neither is a side effect of the other.
#[derive(Clone, Debug, PartialEq)]
pub struct MovableResponse {
    /// The transition this interaction produced.
    pub action: MovableAction,
    /// Whether the card moved to the top of Band 1 (`UIF-MOV-003`).
    pub promoted: bool,
    /// Whether the event reached the workspace underneath (`UIF-MOV-003`).
    pub passed_through: bool,
    /// Effects the caller must perform.
    pub tasks: Vec<MovableTask>,
}

impl MovableResponse {
    /// A response carrying one action.
    #[must_use]
    pub fn new(action: MovableAction) -> Self {
        Self {
            action,
            promoted: false,
            passed_through: false,
            tasks: Vec::new(),
        }
    }

    /// A response that did nothing.
    #[must_use]
    pub fn no_op(reason: MovableNoOp) -> Self {
        Self::new(MovableAction::NoOp(reason))
    }

    /// Attach a task.
    #[must_use]
    pub fn with_task(mut self, task: MovableTask) -> Self {
        self.tasks.push(task);
        self
    }

    /// Record that the card was raised.
    #[must_use]
    pub fn raised(mut self, promoted: bool) -> Self {
        self.promoted = promoted;
        self
    }

    /// Record that the event fell through to the workspace.
    #[must_use]
    pub fn through(mut self, passed_through: bool) -> Self {
        self.passed_through = passed_through;
        self
    }

    /// Whether the card is still open after this response.
    #[must_use]
    pub fn stays_open(&self) -> bool {
        !matches!(self.action, MovableAction::Closed { .. })
    }

    /// Whether this response is a drag transition.
    #[must_use]
    pub fn is_drag(&self) -> bool {
        matches!(
            self.action,
            MovableAction::DragStarted { .. }
                | MovableAction::Dragged { .. }
                | MovableAction::DragEnded { .. }
        )
    }

    /// Whether this response is a resize transition.
    #[must_use]
    pub fn is_resize(&self) -> bool {
        matches!(
            self.action,
            MovableAction::ResizeStarted { .. }
                | MovableAction::Resized { .. }
                | MovableAction::ResizeEnded { .. }
        )
    }
}

// ---------------------------------------------------------------------------
// The spec
// ---------------------------------------------------------------------------

/// Everything the caller supplies to open a movable overlay.
///
/// The card's *contents* — the log lines, the JSON tree, the filter
/// widgets — belong to the view layer and never enter this model, which is
/// why the four invariants hold without one.
#[derive(Clone, Debug, PartialEq)]
pub struct MovableSpec {
    id: SurfaceId,
    title: String,
    geometry: Option<MovableGeometry>,
    auto_scroll: AutoScrollSetting,
    controls: Vec<FocusMember>,
}

impl MovableSpec {
    /// A spec for a card of a given identity and title.
    pub fn new(id: impl Into<SurfaceId>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            geometry: None,
            auto_scroll: AutoScrollSetting::Enabled,
            controls: Vec::new(),
        }
    }

    /// Open at an explicit geometry instead of the default or the
    /// persisted one.
    #[must_use]
    pub fn with_geometry(mut self, geometry: MovableGeometry) -> Self {
        self.geometry = Some(geometry);
        self
    }

    /// Open with auto-scroll on or off (`UIF-MOV-004`).
    #[must_use]
    pub fn with_auto_scroll(mut self, setting: AutoScrollSetting) -> Self {
        self.auto_scroll = setting;
        self
    }

    /// Declare the card's focusable roster, in tab order.
    #[must_use]
    pub fn with_controls(mut self, controls: Vec<FocusMember>) -> Self {
        self.controls = controls;
        self
    }

    /// The card's identity.
    #[must_use]
    pub fn id(&self) -> &SurfaceId {
        &self.id
    }

    /// The card title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// An explicit geometry, if one was given.
    #[must_use]
    pub const fn geometry(&self) -> Option<MovableGeometry> {
        self.geometry
    }

    /// The starting auto-scroll setting.
    #[must_use]
    pub const fn auto_scroll(&self) -> AutoScrollSetting {
        self.auto_scroll
    }

    /// The declared focusable roster.
    #[must_use]
    pub fn controls(&self) -> &[FocusMember] {
        &self.controls
    }
}

// ---------------------------------------------------------------------------
// The resize capture
// ---------------------------------------------------------------------------

/// An in-progress resize (`UIF-MOV-002`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct ResizeCapture {
    /// The handle the pointer holds.
    handle: ResizeHandle,
}

// ---------------------------------------------------------------------------
// The host
// ---------------------------------------------------------------------------

/// The movable overlay host — the frame that makes `UIF-MOV-001` …
/// `UIF-MOV-004` hold.
///
/// The host owns the card's geometry, its drag gesture and resize capture,
/// its scroll and auto-scroll state, and the fact that it is open. The
/// z-order surface and the persistence sink are collaborators the caller
/// passes in, because both are shared with the rest of the window — and, in
/// the sink's case, because the framework must not own a storage location
/// at all (`RUST-024`).
///
/// The drag is a [`DragGesture`] rather than a bare capture record because
/// §3.0.4.2 names this interaction as the `on_drag_settle` case: the drag
/// owes a `Live` tick per frame, **one** `Commit` at the release, and a
/// `Cancel` that puts the card back where the user grabbed it.
#[derive(Clone, Debug, PartialEq)]
pub struct MovableHost {
    id: SurfaceId,
    group: GroupId,
    title: String,
    kind: SurfaceKind,
    viewport: Rectangle,
    geometry: MovableGeometry,
    pointer: Point,
    /// The one drag gesture the host can have open, holding the pre-drag
    /// geometry its `Commit` and `Cancel` are about (`UIF-ATOM-000b`).
    drag: DragGesture,
    resize: Option<ResizeCapture>,
    auto_scroll: AutoScrollSetting,
    follow: FollowState,
    behind: usize,
    scroll: ScrollGeometry,
    controls: Vec<FocusMember>,
    open: bool,
}

impl MovableHost {
    /// Open a movable overlay.
    ///
    /// The geometry is resolved in the spec's order of authority: an
    /// explicit [`MovableSpec::with_geometry`] wins, then whatever the
    /// injected sink has saved for this card (which is how a restart
    /// restores the card), then §4.7.1's `680 × 440` default. The resolved
    /// rectangle is then run through the size policy and the reachability
    /// clamp, because a card saved on a 2560px monitor has to survive
    /// being opened on a 1024px one without losing its title bar.
    ///
    /// The card's focus group is registered but **nothing is focused**: a
    /// modeless card that grabbed the keyboard on open would contradict the
    /// only reason it is not a modal.
    #[must_use]
    pub fn open(
        spec: MovableSpec,
        viewport: Rectangle,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
        storage: &mut dyn GeometryStorage,
    ) -> Self {
        let key = storage_key(spec.id());
        let wanted = spec
            .geometry
            .or_else(|| storage.load(&key))
            .unwrap_or(MovableGeometry::DEFAULT);
        let rect = clamp::clamp_size(wanted.rect(), viewport, ClampRule::MOVABLE);
        let group = group_for(&spec.id);
        let controls = spec.controls;

        registry.register(spec.id.clone(), SurfaceKind::MovableCard, vec![rect]);
        // Registering a roster moves no focus on its own: the card stays
        // modeless until the user clicks one of its controls.
        focus.register_group(group.clone(), controls.clone());

        let mut host = Self {
            id: spec.id,
            group,
            title: spec.title,
            kind: SurfaceKind::MovableCard,
            viewport,
            geometry: MovableGeometry::new(rect.position(), rect.size()),
            pointer: rect.position(),
            drag: DragGesture::new(),
            resize: None,
            auto_scroll: spec.auto_scroll,
            follow: FollowState::Paused,
            behind: 0,
            scroll: ScrollGeometry::EMPTY,
            controls,
            open: true,
        };
        host.refresh_follow();
        host
    }

    // -- identity ----------------------------------------------------------

    /// The card's surface identity.
    #[must_use]
    pub fn id(&self) -> &SurfaceId {
        &self.id
    }

    /// The card title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The key this card's geometry is saved under.
    #[must_use]
    pub fn storage_key(&self) -> String {
        storage_key(&self.id)
    }

    /// The focus group this card's controls live in.
    #[must_use]
    pub fn group_id(&self) -> &GroupId {
        &self.group
    }

    /// The surface kind — always [`SurfaceKind::MovableCard`], i.e. Band 1.
    #[must_use]
    pub const fn kind(&self) -> SurfaceKind {
        self.kind
    }

    /// The band this card lives in.
    #[must_use]
    pub const fn band(&self) -> ZBand {
        self.kind.band()
    }

    /// Whether the card is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    // -- geometry ----------------------------------------------------------

    /// The card's current geometry.
    #[must_use]
    pub const fn geometry(&self) -> MovableGeometry {
        self.geometry
    }

    /// The card's rectangle.
    #[must_use]
    pub fn card(&self) -> Rectangle {
        self.geometry.rect()
    }

    /// The viewport the card is clamped against.
    #[must_use]
    pub const fn viewport(&self) -> Rectangle {
        self.viewport
    }

    /// The title bar's rectangle — the top [`TITLE_BAR_HEIGHT`] of the card.
    #[must_use]
    pub fn title_bar(&self) -> Rectangle {
        clamp::title_bar_rect(self.card(), ClampRule::MOVABLE)
    }

    /// The trailing action cluster: the `Dock` and `Close ×` buttons at
    /// the right end of the title bar.
    ///
    /// This is `UIF-MOV-001`'s exclusion zone, and its width is derived
    /// from the controls themselves — two [`ACTION_CONTROL_SIZE`] squares
    /// and one [`ACTION_GAP`] — inset from the card's right edge by the
    /// title bar's own [`TITLE_BAR_PADDING_HORIZONTAL`].
    #[must_use]
    pub fn trailing_actions(&self) -> Rectangle {
        let bar = self.title_bar();
        let width = ACTION_CONTROL_SIZE * 2.0 + ACTION_GAP;
        Rectangle::new(
            Point::new(
                bar.x + bar.width - TITLE_BAR_PADDING_HORIZONTAL - width,
                bar.y,
            ),
            Size::new(width, bar.height),
        )
    }

    /// The filter & control bar's rectangle.
    #[must_use]
    pub fn filter_bar(&self) -> Rectangle {
        let card = self.card();
        Rectangle::new(
            Point::new(card.x, card.y + TITLE_BAR_HEIGHT),
            Size::new(card.width, FILTER_BAR_HEIGHT),
        )
    }

    /// The scrollable content viewport: whatever the card has left between
    /// the filter bar and the footer.
    #[must_use]
    pub fn content_viewport(&self) -> Rectangle {
        let card = self.card();
        Rectangle::new(
            Point::new(card.x, card.y + TITLE_BAR_HEIGHT + FILTER_BAR_HEIGHT),
            Size::new(card.width, (card.height - CHROME_HEIGHT).max(0.0)),
        )
    }

    /// The footer status & resize bar's rectangle.
    #[must_use]
    pub fn footer(&self) -> Rectangle {
        let card = self.card();
        Rectangle::new(
            Point::new(card.x, card.y + card.height - FOOTER_HEIGHT),
            Size::new(card.width, FOOTER_HEIGHT),
        )
    }

    /// The pointer grip for one resize handle, if the anatomy gives that
    /// handle one.
    ///
    /// The grip band is the [`RESIZE_HANDLE_SIZE`] border of the card's
    /// **body** — everything below the title bar. §4.7.1 paints only the
    /// ◢ corner; the other grips are the layout a view adds if it wants
    /// edge resizing. The three handles that would sit on the title bar
    /// (`TopLeft`, `Top`, `TopRight`) return `None`, because
    /// `UIF-MOV-001` gives the whole of that bar to the drag and a resize
    /// grip there would take the user's grip away from it. They remain
    /// reachable through [`resize_by`](Self::resize_by).
    #[must_use]
    pub fn resize_grip(&self, handle: ResizeHandle) -> Option<Rectangle> {
        let body = self.body();
        let g = RESIZE_HANDLE_SIZE;
        let grip = match handle {
            ResizeHandle::TopLeft | ResizeHandle::Top | ResizeHandle::TopRight => return None,
            ResizeHandle::BottomLeft => Rectangle::new(
                Point::new(body.x, body.y + body.height - g),
                Size::new(g, g),
            ),
            ResizeHandle::Bottom => Rectangle::new(
                Point::new(body.x + g, body.y + body.height - g),
                Size::new((body.width - g * 2.0).max(0.0), g),
            ),
            ResizeHandle::BottomRight => Rectangle::new(
                Point::new(body.x + body.width - g, body.y + body.height - g),
                Size::new(g, g),
            ),
            ResizeHandle::Left => Rectangle::new(
                Point::new(body.x, body.y + g),
                Size::new(g, (body.height - g * 2.0).max(0.0)),
            ),
            ResizeHandle::Right => Rectangle::new(
                Point::new(body.x + body.width - g, body.y + g),
                Size::new(g, (body.height - g * 2.0).max(0.0)),
            ),
        };
        Some(grip)
    }

    /// The card minus its title bar — the region the grip band is measured
    /// against.
    #[must_use]
    pub fn body(&self) -> Rectangle {
        let card = self.card();
        Rectangle::new(
            Point::new(card.x, card.y + TITLE_BAR_HEIGHT),
            Size::new(card.width, (card.height - TITLE_BAR_HEIGHT).max(0.0)),
        )
    }

    /// Whether `at` is inside the card.
    ///
    /// Iced's convention, shared crate-wide: the right and bottom edges are
    /// *outside*.
    #[must_use]
    pub fn is_inside_card(&self, at: Point) -> bool {
        self.card().contains(at)
    }

    /// The host's own verdict on where `at` landed (`UIF-MOV-001`).
    ///
    /// Computed from the host's rectangles only. The caller's report never
    /// enters here, which is what lets the host overrule a report that
    /// disagrees with the geometry it owns.
    #[must_use]
    pub fn zone_at(&self, at: Point) -> Zone {
        if !self.is_inside_card(at) {
            return Zone::Outside;
        }
        if self.title_bar().contains(at) {
            return if self.trailing_actions().contains(at) {
                Zone::TitleActions
            } else {
                Zone::TitleDrag
            };
        }
        if let Some(handle) = self.grip_at(at) {
            return Zone::Grip(handle);
        }
        if self.filter_bar().contains(at) {
            return Zone::FilterBar;
        }
        if self.footer().contains(at) {
            return Zone::Footer;
        }
        Zone::Content
    }

    /// Whether a press at `at` may start a drag (`UIF-MOV-001`).
    #[must_use]
    pub fn is_draggable_at(&self, at: Point) -> bool {
        self.zone_at(at) == Zone::TitleDrag
    }

    /// The handle whose grip contains `at`, if any.
    fn grip_at(&self, at: Point) -> Option<ResizeHandle> {
        ResizeHandle::ALL
            .into_iter()
            .filter_map(|handle| Some((handle, self.resize_grip(handle)?)))
            .find(|(_, grip)| grip.contains(at))
            .map(|(handle, _)| handle)
    }

    /// Whether `at` is on a resize grip the anatomy exposes.
    #[must_use]
    pub fn is_grip_at(&self, at: Point) -> bool {
        self.grip_at(at).is_some()
    }

    /// The card's reachability right now (`UIF-MOV-002`).
    ///
    /// Exposed so a caller — and every test — can assert the invariant
    /// rather than trust that the clamp held.
    #[must_use]
    pub fn is_reachable(&self) -> bool {
        clamp::is_reachable(self.card(), self.viewport, ClampRule::MOVABLE)
    }

    // -- cursor ------------------------------------------------------------

    /// The cursor the window should show for the host's current state.
    #[must_use]
    pub fn cursor(&self) -> mouse::Interaction {
        if self.drag.is_open() {
            mouse::Interaction::Grabbing
        } else if let Some(capture) = &self.resize {
            resize_interaction(capture.handle)
        } else {
            mouse::Interaction::None
        }
    }

    /// The cursor the window should show with the pointer at `at`.
    #[must_use]
    pub fn cursor_at(&self, at: Point) -> mouse::Interaction {
        match self.zone_at(at) {
            Zone::TitleDrag => {
                if self.drag.is_open() {
                    mouse::Interaction::Grabbing
                } else {
                    mouse::Interaction::Grab
                }
            }
            Zone::Grip(handle) => resize_interaction(handle),
            _ => mouse::Interaction::None,
        }
    }

    // -- drag state --------------------------------------------------------

    /// Whether a title-bar drag is in progress (`UIF-MOV-001`).
    #[must_use]
    pub fn is_dragging(&self) -> bool {
        self.drag.is_open()
    }

    /// The handle currently being dragged, if a resize is in progress.
    #[must_use]
    pub fn resizing_handle(&self) -> Option<ResizeHandle> {
        self.resize.map(|capture| capture.handle)
    }

    /// Whether the host holds pointer capture — a drag or a resize that
    /// keeps receiving moves even when the pointer leaves the card.
    #[must_use]
    pub fn is_capturing(&self) -> bool {
        self.drag.is_open() || self.resize.is_some()
    }

    // -- pointer -----------------------------------------------------------

    /// Drive one pointer event through the host.
    ///
    /// **Routing is decided first, by the registry, not by the host.** A
    /// route that does not name this surface cannot reach the
    /// classification code below, which is the structural form of
    /// `UIF-MOV-003`: the card owns its inside and nothing outside it. The
    /// one exception is pointer capture — once a drag or resize has begun,
    /// the host keeps the gesture even though the card is running out from
    /// under the pointer.
    pub fn on_pointer(
        &mut self,
        phase: PointerPhase,
        at: Point,
        target: SurfaceTarget,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
        storage: &mut dyn GeometryStorage,
    ) -> MovableResponse {
        if !self.open {
            return MovableResponse::no_op(MovableNoOp::NotOpen);
        }

        let capturing = self.is_capturing();
        if !capturing {
            if !registry.contains(&self.id) {
                return MovableResponse::no_op(MovableNoOp::NotRegistered);
            }
            let route = registry.route_pointer(at);
            if route.target().surface_id() != Some(&self.id) {
                self.pointer = at;
                return if route.reaches_workspace() {
                    MovableResponse::new(MovableAction::PassedThrough { route }).through(true)
                } else {
                    MovableResponse::no_op(MovableNoOp::OwnedByOtherSurface)
                };
            }
        }

        let promoted = if capturing {
            false
        } else {
            self.promote_if_band_says_so(phase, registry)
        };

        let action = match phase {
            PointerPhase::Pressed => self.on_press(at, target, focus, registry, storage),
            PointerPhase::Moved => self.on_move(at),
            PointerPhase::Released => self.on_release(at, storage),
        };

        let cursor = self.cursor_at(at);
        MovableResponse::new(action)
            .raised(promoted)
            .with_task(MovableTask::SetCursor(cursor))
    }

    /// Raise the card to the top of its band — but only because the band
    /// itself says it promotes on click (§5 Band 1).
    fn promote_if_band_says_so(
        &mut self,
        phase: PointerPhase,
        registry: &mut SurfaceRegistry,
    ) -> bool {
        if phase != PointerPhase::Pressed || !self.band().promotes_on_click() {
            return false;
        }
        registry.promote(&self.id)
    }

    /// A press inside the card. Geometry picks the zone; the caller's
    /// report only disambiguates *within* it.
    fn on_press(
        &mut self,
        at: Point,
        target: SurfaceTarget,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
        storage: &mut dyn GeometryStorage,
    ) -> MovableAction {
        match self.zone_at(at) {
            Zone::Grip(handle) => {
                self.resize = Some(ResizeCapture { handle });
                self.pointer = at;
                MovableAction::ResizeStarted { handle }
            }
            Zone::TitleDrag => {
                // The snapshot is captured here, before any delta is applied,
                // so the `Cancel` tier restores the rectangle the user
                // actually grabbed (§3.0.4.2).
                self.drag.begin(self.geometry);
                self.pointer = at;
                MovableAction::DragStarted {
                    geometry: self.geometry,
                }
            }
            Zone::TitleActions => match target {
                SurfaceTarget::Close => {
                    self.pointer = at;
                    self.close(focus, registry, storage).action
                }
                SurfaceTarget::Dock => {
                    self.pointer = at;
                    MovableAction::Activated {
                        control: CardControl::Dock,
                    }
                }
                // The caller says the press is on an action control but the
                // geometry says it is not on one the host recognises. Take
                // the safe reading: absorb it rather than guess.
                _ => {
                    self.pointer = at;
                    MovableAction::Absorbed {
                        zone: Zone::TitleActions,
                    }
                }
            },
            Zone::FilterBar => {
                self.pointer = at;
                self.activate_in_filter_bar(target)
            }
            zone => {
                self.pointer = at;
                MovableAction::Absorbed { zone }
            }
        }
    }

    /// What a press inside the filter bar activates.
    fn activate_in_filter_bar(&self, target: SurfaceTarget) -> MovableAction {
        match target {
            SurfaceTarget::AutoScrollToggle => MovableAction::Activated {
                control: CardControl::AutoScrollToggle,
            },
            SurfaceTarget::Control(id) => MovableAction::Activated {
                control: CardControl::Custom(id),
            },
            _ => MovableAction::Absorbed {
                zone: Zone::FilterBar,
            },
        }
    }

    /// A pointer move: translate an active drag or resize.
    fn on_move(&mut self, at: Point) -> MovableAction {
        let delta = at - self.pointer;
        self.pointer = at;

        if self.drag.is_open() {
            return self.drag_by(delta);
        }
        if let Some(capture) = self.resize {
            return self.resize_by(capture.handle, delta);
        }
        MovableAction::NoOp(MovableNoOp::Idle)
    }

    /// A button release: end whatever gesture was captured and settle the
    /// geometry.
    fn on_release(&mut self, at: Point, storage: &mut dyn GeometryStorage) -> MovableAction {
        self.pointer = at;

        // `commit` *takes* the open gesture, so this branch can run at most
        // once per drag: the `Commit` tier — the spec's `on_drag_settle` —
        // is the only thing that persists, and a second release has nothing
        // left to settle.
        if self.drag.commit(self.geometry).is_some() {
            self.persist(storage);
            return MovableAction::DragEnded {
                geometry: self.geometry,
            };
        }
        if let Some(capture) = self.resize.take() {
            self.persist(storage);
            return MovableAction::ResizeEnded {
                handle: capture.handle,
                geometry: self.geometry,
            };
        }
        MovableAction::NoOp(MovableNoOp::Idle)
    }

    // -- drag / resize frames ----------------------------------------------

    /// Translate the card by one frame's pointer delta (`UIF-MOV-001`).
    ///
    /// Goes through [`clamp::drag_position`], which clamps from the card's
    /// *current* clamped position rather than from drag-start plus a
    /// running total. That is the whole no-accumulation contract: a delta
    /// that a bound absorbs is discarded at the moment it is absorbed, so
    /// dragging `500px` past the edge and `1px` back moves the card
    /// `1px`, not `0px` until the pointer has re-travelled `500px`.
    ///
    /// Every frame emits one `Live` tick (§3.0.4.2). A host that drives its
    /// own pointer events may reach this without a press having gone
    /// through [`on_pointer`](Self::on_pointer), so the gesture is opened
    /// here when it is not already open — the snapshot is still captured
    /// *before* the first delta is applied, which is the only ordering the
    /// `Cancel` tier requires.
    pub fn drag_by(&mut self, delta: Vector) -> MovableAction {
        if !self.drag.is_open() {
            self.drag.begin(self.geometry);
        }
        let position = clamp::drag_position(
            self.geometry.position,
            delta,
            self.geometry.size,
            self.viewport,
            ClampRule::MOVABLE,
        );
        self.apply_geometry(self.geometry.with_position(position));
        // The tick is recorded in the gesture's transcript rather than
        // returned: the `Live` tier's contract is that the host may drop
        // any number of them, so the host-visible action stays the single
        // `Dragged` it always was.
        let _ = self.drag.live(self.geometry);
        MovableAction::Dragged {
            geometry: self.geometry,
            delta,
        }
    }

    /// Resize the card from `handle` by one frame's delta (`UIF-MOV-002`).
    ///
    /// The clamp solver owns the whole resize: which edge follows the
    /// pointer, which edge is held, where the size floor stops it, and what
    /// happens when the held edge and the reachability invariant disagree.
    pub fn resize_by(&mut self, handle: ResizeHandle, delta: Vector) -> MovableAction {
        let rect = clamp::resize(
            self.card(),
            handle,
            delta,
            self.viewport,
            ClampRule::MOVABLE,
        );
        self.apply_geometry(MovableGeometry::new(rect.position(), rect.size()));
        MovableAction::Resized {
            handle,
            geometry: self.geometry,
            delta,
        }
    }

    /// End a drag and settle its geometry.
    ///
    /// Settles *through* the gesture, so the `Commit` tier is what closes
    /// the drag and the persist stays on the settle path exactly as before.
    pub fn end_drag(&mut self, storage: &mut dyn GeometryStorage) -> MovableAction {
        self.drag.commit(self.geometry);
        self.persist(storage);
        MovableAction::DragEnded {
            geometry: self.geometry,
        }
    }

    /// End a resize and settle its geometry.
    pub fn end_resize(&mut self, storage: &mut dyn GeometryStorage) -> MovableAction {
        let handle = self.resize.take().map(|capture| capture.handle);
        self.persist(storage);
        match handle {
            Some(handle) => MovableAction::ResizeEnded {
                handle,
                geometry: self.geometry,
            },
            None => MovableAction::NoOp(MovableNoOp::Idle),
        }
    }

    /// Set the geometry programmatically.
    ///
    /// The value goes through the same clamp every other geometry change
    /// does, so a caller cannot place a card somewhere its title bar would
    /// be lost.
    pub fn set_geometry(
        &mut self,
        geometry: MovableGeometry,
        storage: &mut dyn GeometryStorage,
    ) -> MovableAction {
        let adjusted = self.apply_geometry(geometry);
        if adjusted {
            self.persist(storage);
        }
        MovableAction::Reclamped {
            geometry: self.geometry,
            adjusted,
        }
    }

    /// Move the card to a new viewport, re-clamping it (`UIF-MOV-002`).
    ///
    /// The size is preserved and only the origin is pulled back: a window
    /// that shrinks must not resize the user's card, but it must stop that
    /// card's title bar from ending up off-screen.
    pub fn set_viewport(
        &mut self,
        viewport: Rectangle,
        registry: &mut SurfaceRegistry,
        storage: &mut dyn GeometryStorage,
    ) -> MovableAction {
        self.viewport = viewport;
        let adjusted = self.apply_geometry(self.geometry);
        if self.open {
            registry.register(self.id.clone(), self.kind, vec![self.card()]);
        }
        if adjusted {
            self.persist(storage);
        }
        MovableAction::Reclamped {
            geometry: self.geometry,
            adjusted,
        }
    }

    /// The single private route to a new geometry (`UIF-MOV-002`).
    ///
    /// Every geometry change in this host goes through here, and this
    /// function clamps before it stores. An unreachable card is therefore
    /// not something the host promises not to create — it is something the
    /// host cannot create. The `debug_assert` is the belt: it fires in test
    /// and debug builds if the clamp and the predicate ever disagree, which
    /// is precisely the drift `INC-13` was written to make impossible.
    ///
    /// The return value is *whether the clamp altered the request*, not
    /// whether the host's state moved. A caller that asked for something
    /// legal and got it back unchanged needs to be able to tell that from a
    /// caller whose request was pulled back to a bound.
    fn apply_geometry(&mut self, geometry: MovableGeometry) -> bool {
        let rect = clamp::clamp_rect(geometry.rect(), self.viewport, ClampRule::MOVABLE);
        let clamped = MovableGeometry::new(rect.position(), rect.size());
        let adjusted = clamped != geometry;
        self.geometry = clamped;

        debug_assert!(
            clamp::is_reachable(clamped.rect(), self.viewport, ClampRule::MOVABLE),
            "UIF-MOV-002: a geometry leaving `apply_geometry` must keep the title bar reachable"
        );

        adjusted
    }

    // -- scroll and auto-scroll (UIF-MOV-004) -------------------------------

    /// The auto-scroll setting.
    #[must_use]
    pub const fn auto_scroll(&self) -> AutoScrollSetting {
        self.auto_scroll
    }

    /// Whether the view is currently following the stream.
    #[must_use]
    pub const fn follow_state(&self) -> FollowState {
        self.follow
    }

    /// Whether the viewport is at the bottom of its content.
    #[must_use]
    pub fn at_bottom(&self) -> bool {
        self.scroll.at_bottom()
    }

    /// How many entries have arrived below the viewport while paused.
    #[must_use]
    pub const fn behind(&self) -> usize {
        self.behind
    }

    /// The content viewport's scroll geometry.
    #[must_use]
    pub const fn scroll(&self) -> ScrollGeometry {
        self.scroll
    }

    /// Set the auto-scroll setting and recompute the follow state.
    ///
    /// Turning auto-scroll on while the user is scrolled up does **not**
    /// jump them to the bottom: the setting grants permission, it does not
    /// move the viewport. Following starts at the next append once the
    /// viewport is at the bottom.
    pub fn set_auto_scroll(&mut self, setting: AutoScrollSetting) -> MovableAction {
        self.auto_scroll = setting;
        let follow = self.refresh_follow();
        MovableAction::AutoScrollChanged { setting, follow }
    }

    /// Adopt the scroll geometry the view reports and recompute following.
    pub fn sync_scroll(&mut self, scroll: ScrollGeometry) -> MovableAction {
        self.scroll = scroll.clamped();
        let follow = self.refresh_follow();
        MovableAction::Scrolled { follow }
    }

    /// Scroll the viewport by `delta` (positive is down) and recompute
    /// following.
    pub fn scroll_by(&mut self, delta: f32) -> MovableAction {
        self.scroll = self.scroll.scrolled_by(delta);
        let follow = self.refresh_follow();
        MovableAction::Scrolled { follow }
    }

    /// Scroll the viewport to the bottom and resume following.
    pub fn scroll_to_bottom(&mut self) -> MovableAction {
        self.scroll = self.scroll.at_bottom_now();
        let follow = self.refresh_follow();
        MovableAction::Scrolled { follow }
    }

    /// Decide what to do about `entries` new stream entries
    /// (`UIF-MOV-004`).
    pub fn on_append(&mut self, entries: usize) -> AutoScrollDecision {
        if self.follow.is_following() {
            self.behind = 0;
            self.scroll = self.scroll.at_bottom_now();
            AutoScrollDecision::Follow { entries }
        } else {
            self.behind += entries;
            AutoScrollDecision::Hold {
                entries,
                reason: if self.auto_scroll.permits_following() {
                    HoldReason::Inspecting
                } else {
                    HoldReason::Disabled
                },
                behind: self.behind,
            }
        }
    }

    /// Route a scroll event through the card.
    ///
    /// Outside the card the scroll belongs to the workspace, exactly like a
    /// click — `UIF-MOV-003` names clicks, scroll and text selection
    /// together and does not rank them. Inside, only the content viewport
    /// scrolls; the title bar, filter bar and footer take the event and go
    /// nowhere with it.
    pub fn on_scroll(
        &mut self,
        at: Point,
        delta: f32,
        registry: &mut SurfaceRegistry,
    ) -> MovableResponse {
        if !self.open {
            return MovableResponse::no_op(MovableNoOp::NotOpen);
        }
        if !registry.contains(&self.id) {
            return MovableResponse::no_op(MovableNoOp::NotRegistered);
        }

        let route = registry.route_pointer(at);
        if route.target().surface_id() != Some(&self.id) {
            return if route.reaches_workspace() {
                MovableResponse::new(MovableAction::PassedThrough { route }).through(true)
            } else {
                MovableResponse::no_op(MovableNoOp::OwnedByOtherSurface)
            };
        }

        match self.zone_at(at) {
            Zone::Content => {
                let follow = match self.scroll_by(delta) {
                    MovableAction::Scrolled { follow } => follow,
                    _ => self.follow,
                };
                MovableResponse::new(MovableAction::Scrolled { follow })
            }
            zone => MovableResponse::new(MovableAction::Absorbed { zone }),
        }
    }

    /// Recompute the follow state from the two inputs and clear the backlog
    /// if the view resumed.
    fn refresh_follow(&mut self) -> FollowState {
        self.follow = FollowState::resolve(self.auto_scroll, self.scroll.at_bottom());
        if self.follow.is_following() {
            self.behind = 0;
        }
        self.follow
    }

    // -- keyboard ----------------------------------------------------------

    /// Whether this card holds keyboard focus — the condition §5 Band 1
    /// attaches to `Escape`.
    #[must_use]
    pub fn holds_focus(&self, focus: &FocusState) -> bool {
        focus
            .current()
            .and_then(|id| focus.group_of(id))
            .is_some_and(|group| group.id() == &self.group)
    }

    /// Drive one key class through the host.
    ///
    /// What the host handles is read from [`ZBand::key_disposition`] for
    /// Band 1, not from a list of keys it likes. Band 1 swallows `Escape`
    /// and only while focused, and passes everything else down — which is
    /// what keeps a floating log from eating the workspace's keystrokes.
    pub fn on_key(
        &mut self,
        class: KeyClass,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
        storage: &mut dyn GeometryStorage,
    ) -> MovableResponse {
        if !self.open {
            return MovableResponse::no_op(MovableNoOp::NotOpen);
        }

        match self.band().key_disposition(class) {
            KeyDisposition::PassThrough => MovableResponse::no_op(MovableNoOp::KeyNotTrapped),
            KeyDisposition::Swallow | KeyDisposition::SwallowWhenFocused => match class {
                KeyClass::Escape => self.escape_outcome(focus, registry, storage),
                _ => MovableResponse::no_op(MovableNoOp::KeyNotTrapped),
            },
        }
    }

    /// The card's own answer when the escape chain asks it to unwind
    /// (`UIF-WIN-003`, **G-13**).
    ///
    /// A drag in progress answers first, with the `Cancel` tier: the card
    /// puts back the rectangle the user grabbed and reports `Unwound` —
    /// key consumed, card still open — so wanting its position back never
    /// turns into being dismissed. Otherwise `Dismissed` when the card is
    /// open and focused, `Declined(NotFocused)` when it is open but the
    /// keyboard is elsewhere, and `Declined(NotOpen)` once it is closed.
    ///
    /// **Escape-rank safety (G-13):** this method is only ever reached from
    /// [`SurfaceRegistry::escape`]'s walk, which asks every higher-ranked
    /// band first. A `Cancel` introduced here therefore cannot consume an
    /// `Escape` a [`ZBand::Menu`] or
    /// a [`ZBand::Modal`] above the
    /// card still needs — the walk order guarantees it, not a check here.
    ///
    /// **Must not mutate the registry** — the caller is normally
    /// [`SurfaceRegistry::escape`], which indexes the surface list and
    /// removes the entry it was told closed.
    pub fn unwind_answer(
        &mut self,
        focus: &mut FocusState,
        storage: &mut dyn GeometryStorage,
    ) -> UnwindAnswer {
        if !self.open {
            return UnwindAnswer::Declined(DeclineReason::NotOpen);
        }

        // Checked ahead of the focus test because the drag is held by
        // pointer capture, not by the keyboard: a card the user is dragging
        // without having focused it still owns the gesture, and still owes
        // the revert.
        if let Some(cancel) = self.drag.cancel() {
            // Back through `apply_geometry`, so a restore cannot create a
            // rectangle the clamp forbids. Nothing is persisted: the card
            // has returned to what was already saved, not moved anywhere.
            let restored = *cancel.restored();
            let _ = self.apply_geometry(restored);
            return UnwindAnswer::Unwound;
        }

        if !self.holds_focus(focus) {
            return UnwindAnswer::Declined(DeclineReason::NotFocused);
        }

        self.teardown(focus, storage);
        UnwindAnswer::Dismissed
    }

    /// Run one `Escape` through the z-order chain.
    ///
    /// Every other surface is answered `Declined(NotOpen)` — "not mine, the
    /// key keeps travelling". Because [`ZBand::Menu`] has `escape_rank` `3`
    /// and [`ZBand::Modal`] `2` against this band's `1`, a menu or a modal
    /// open above the card is asked **first** and the card never steals the
    /// key from them. The registry's walk order does it; the card does not
    /// know menus exist.
    pub fn escape_via(
        &mut self,
        registry: &mut SurfaceRegistry,
        focus: &mut FocusState,
        storage: &mut dyn GeometryStorage,
    ) -> EscapeOutcome {
        let id = self.id.clone();
        registry.escape(move |surface| {
            if surface.id() != &id {
                return UnwindAnswer::Declined(DeclineReason::NotOpen);
            }
            self.unwind_answer(focus, storage)
        })
    }

    /// Map the chain's answer onto a host response.
    fn escape_outcome(
        &mut self,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
        storage: &mut dyn GeometryStorage,
    ) -> MovableResponse {
        let outcome = self.escape_via(registry, focus, storage);

        if outcome.consumed_by() == Some(&self.id) {
            return MovableResponse::new(MovableAction::Closed {
                geometry: self.geometry,
            });
        }
        // Nothing above the card can take the key *on the card's behalf*: the
        // closure [`Self::escape_via`] installs answers every other surface
        // `Declined(NotOpen)`, so the walk keeps travelling past them. A
        // consumption this response did not cause would mean that contract
        // broke, and there is no honest no-op for it.
        debug_assert!(
            !outcome.is_consumed(),
            "movable: the escape chain was consumed by a surface the card did not ask"
        );
        if outcome.declined_before().iter().any(|declined| {
            declined.surface == self.id && declined.reason == DeclineReason::NotFocused
        }) {
            return MovableResponse::no_op(MovableNoOp::NotFocused);
        }

        MovableResponse::no_op(MovableNoOp::NotRegistered)
    }

    // -- open / close ------------------------------------------------------

    /// Close the card, persisting its geometry on the way out.
    pub fn close(
        &mut self,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
        storage: &mut dyn GeometryStorage,
    ) -> MovableResponse {
        if !self.open {
            return MovableResponse::no_op(MovableNoOp::NotOpen);
        }

        self.teardown(focus, storage);
        registry.unregister(&self.id);
        MovableResponse::new(MovableAction::Closed {
            geometry: self.geometry,
        })
    }

    /// Re-open a closed card, restoring the geometry it closed with
    /// (`UIF-MOV-001`'s "preserved across open/close toggles").
    ///
    /// The card's own retained geometry wins here — it is the same card the
    /// user just closed, and it did not move while it was shut. A *new*
    /// host for the same id picks the same value up from the injected sink
    /// instead, which is the restart half of the same contract.
    pub fn reopen(
        &mut self,
        viewport: Rectangle,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
        storage: &mut dyn GeometryStorage,
    ) -> MovableResponse {
        if self.open {
            return MovableResponse::no_op(MovableNoOp::AlreadyOpen);
        }

        self.viewport = viewport;
        let adjusted = self.apply_geometry(self.geometry);
        registry.register(self.id.clone(), self.kind, vec![self.card()]);
        focus.register_group(self.group.clone(), self.controls.clone());
        self.open = true;
        let follow = self.refresh_follow();
        if adjusted {
            self.persist(storage);
        }

        MovableResponse::new(MovableAction::AutoScrollChanged {
            setting: self.auto_scroll,
            follow,
        })
    }

    /// Write the current geometry through the injected sink.
    ///
    /// The host's only persistence call other than the load at open. It is
    /// invoked at *settled* geometry — drag release, resize release, close,
    /// programmatic set, viewport re-clamp — and never per frame, so the
    /// sink is never on the frame path.
    pub fn persist(&mut self, storage: &mut dyn GeometryStorage) {
        storage.save(&self.storage_key(), self.geometry);
    }

    /// Tear the card down without touching the registry.
    ///
    /// The open drag is **aborted**, not settled: a card going away mid-drag
    /// owes nobody a `Commit` or a `Cancel`, and the geometry that gets
    /// persisted below is the card's current one.
    fn teardown(&mut self, focus: &mut FocusState, storage: &mut dyn GeometryStorage) {
        self.drag.abort();
        self.resize = None;
        self.open = false;
        focus.unregister_group(&self.group);
        self.persist(storage);
    }

    /// Replace the card's focusable roster, as a rebuild of its contents
    /// would.
    ///
    /// The host keeps the roster too, so a [`reopen`](Self::reopen) after a
    /// close re-registers what the card actually has rather than what it
    /// opened with.
    pub fn sync_members(
        &mut self,
        members: Vec<FocusMember>,
        focus: &mut FocusState,
    ) -> Option<FocusId> {
        self.controls = members.clone();
        focus.register_group(self.group.clone(), members)
    }
}

/// The cursor a resize handle asks for.
fn resize_interaction(handle: ResizeHandle) -> mouse::Interaction {
    match handle {
        ResizeHandle::Left | ResizeHandle::Right => mouse::Interaction::ResizingHorizontally,
        ResizeHandle::Top | ResizeHandle::Bottom => mouse::Interaction::ResizingVertically,
        ResizeHandle::TopLeft | ResizeHandle::BottomRight => {
            mouse::Interaction::ResizingDiagonallyDown
        }
        ResizeHandle::TopRight | ResizeHandle::BottomLeft => {
            mouse::Interaction::ResizingDiagonallyUp
        }
    }
}

/// A total `f32` for values that may arrive from a caller that has not
/// measured anything.
fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

#[cfg(test)]
#[path = "movable_tests.rs"]
mod movable_tests;
