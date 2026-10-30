//! The z-band model — the four layered bands of the window, pointer routing
//! through them, and strict `Escape` unwinding
//! (`UIF-ZORD-*`, `UIF-WIN-003`, `UIF-VER-003`).
//!
//! # What this is, and what it is not
//!
//! §5 of the spec describes a layered window in prose and a table. This
//! module turns that table into **data the compiler and the tests can
//! query**, and adds the one thing the table implies but never spells out:
//! an answer to "a click at `(x, y)` — who gets it?".
//!
//! Nothing here renders, paints, or knows what a widget is. It is a model
//! of the layered window: a registry of open surfaces, a routing function
//! over a point, and an unwinding chain. Every rule is a pure transition,
//! which is what makes `UIF-VER-003`'s "Escape unwinds submenus, menus,
//! and modals in strict reverse z-order" a test rather than a hope.
//!
//! # The four bands
//!
//! ```text
//! Band 3  Modals         dimmed scrim · blocks Bands 0 and 1 · traps the keyboard
//! Band 2  Menu trees     transparent click-away catcher · roving arrow focus
//! Band 1  Utility cards  pass-through outside the card · Z-promotion on click
//! Band 0  Workspace      navbar, panes, tabs · standard Tab traversal
//! ```
//!
//! Each band answers four questions about itself, and those answers *are*
//! the §5 table — [`ZBand::backdrop`], [`ZBand::pointer_rule`],
//! [`ZBand::key_disposition`] and [`ZBand::escape_rank`]. A host that
//! asks them never re-derives the table; a host that hardcodes "band 3
//! blocks" has started re-deriving it.
//!
//! # The registry
//!
//! [`SurfaceRegistry`] holds the open surfaces, kept sorted by band with
//! the highest band last, and within a band in bottom-first stacking order.
//! That single invariant is what makes [`SurfaceRegistry::topmost`] — the
//! surface that currently owns input routing — a last-element lookup
//! instead of a search.
//!
//! Re-registering an id refreshes its geometry and keeps its position, so
//! a card that moves, resizes or rebuilds every frame does not silently
//! change the stacking order, and a Band 1 card the user promoted stays
//! promoted.
//!
//! # Pointer routing
//!
//! [`SurfaceRegistry::route_pointer`] walks the bands **top down** and
//! stops at the first band that has something open. Inside that band the
//! topmost surface containing the point wins; missing the band fires that
//! band's outside rule, which is where the three bands genuinely differ:
//!
//! * **Band 3** turns the miss into a scrim hit. Nothing below is ever
//!   consulted — that is what "blocks Band 0 & Band 1 completely" means
//!   operationally, and it is why the walk *stops* rather than continuing.
//! * **Band 2** dismisses the whole band and consumes the click. The
//!   catcher is full-window; the workspace underneath does not get it.
//! * **Band 1** records the pass-through and keeps walking down.
//!
//! The function is pure. Promotion is a separate mutation
//! ([`SurfaceRegistry::pointer_down`]) so "who would get this click" stays
//! answerable without changing anything.
//!
//! # Escape unwinding
//!
//! `UIF-WIN-003`'s order is **deepest submenu → root menu → topmost
//! dialog → search/filter**, and that is *not* the band order: the menu
//! band unwinds above the modal band. See
//! [`ZBand::escape_rank`](ZBand#escape-rank-is-not-band-rank) for why the
//! two are modelled apart.
//!
//! The chain is delegation, not orchestration. The model walks its
//! surfaces in escape order and asks each one, through a caller-supplied
//! closure, to unwind a level. The first that accepts ends the walk and is
//! reported as the consumer; each that refuses is recorded and the walk
//! continues. An empty stack is a defined no-op
//! ([`EscapeRefusal::EmptyStack`]), never a panic.
//!
//! The refusal shape is the cascade's own.
//! [`MenuCascade`](crate::surface::menu::cascade::MenuCascade) already answers
//! a second `Escape` with [`IgnoredReason::NotOpen`] — "my level is
//! spent, the key keeps travelling" — and
//! [`UnwindAnswer::from_cascade_close`] adapts that directly rather than
//! inventing a parallel vocabulary. The band model never reaches into a
//! menu, a modal or a filter: it asks, and the answer decides.
//!
//! Registering **each open menu card as its own Band 2 surface** is what
//! makes the spec's "deepest submenu, then root menu" fall out of the
//! generic walk: the deepest card is simply the last one registered in
//! the band, so it is asked first, and `Dismissed` removes it so the
//! root is next.
//!
//! # The Pre-Rank runs before this walk
//!
//! `UIF-WIN-003` step 1 is not a band and not a surface: it is the
//! pointer-captured drag the user is still holding. [`SurfaceRegistry::escape`]
//! is steps 2 through 5 only. A host with any drag-capable control must run
//! the Pre-Rank first, which is what [`escape_with_pre_rank`] does — it asks
//! [`PreRankCoordinator`] whether a drag owns the pointer, and only if that answer is "no" does it
//! walk the ranks. Calling `escape` alone on such a host is the bug the rule
//! exists to prevent: the focused surface dismisses while the drag keeps
//! running.
//!
//! # The scrim
//!
//! §5 fixes the Band 3 backdrop as `surface.base` at `0.55` alpha. The
//! alpha is a named constant ([`SCRIM_ALPHA`], `RUST-021`) and
//! [`scrim`] is the only place it meets the token, so the composition
//! cannot drift from [`Palette::surface_scrim`](crate::token::color::Palette::surface_scrim).
//!
//! # Decisions this increment made
//!
//! * **A surface with no regions is never hit by the pointer.** It still
//!   registers, still stacks and still takes part in Escape. A full-window
//!   modal must therefore be given the viewport rectangle; leaving its
//!   regions empty would make every click land on its own scrim.
//! * **Hit testing follows iced's convention**: the right and bottom
//!   edges are *outside* the rectangle. Two cards that abut exactly do
//!   not overlap.
//! * **The Band 3 scrim covers Band 2 as well as Bands 0 and 1.** The
//!   spec only names Bands 0 and 1, but the scrim is full-window, so a
//!   menu open under a modal is unreachable by pointer. Escape still
//!   unwinds that menu first, because `UIF-WIN-003` orders it that way.
//! * **Band 1 promotion is the only automatic reordering.** Bands 2 and 3
//!   stack in registration order; clicking a lower modal does not raise
//!   it, because nothing in the spec asks for stacked-modal promotion and
//!   inventing it would make the "topmost dialog" of `UIF-WIN-003` mean
//!   something the host did not choose.
//! * **Declining is how a conditional band says so.** Band 1's
//!   "`Escape` does not close unless focused" is not special-cased in the
//!   walk; the surface answers [`DeclineReason::NotFocused`] and the key
//!   moves on. The band's [`ZBand::key_disposition`] tells the host that
//!   the condition exists.
//!
//! Unit tests live in the sibling `zorder_tests.rs` sidecar.

use std::cmp::Reverse;
use std::fmt;

use crate::binding::pre_rank::DragCapture;
use crate::binding::pre_rank::PreRankCoordinator;
use crate::binding::pre_rank::PreRankOutcome;
use crate::surface::menu::cascade::CascadeOutcome;
use crate::surface::menu::cascade::IgnoredReason;
use crate::token::color::Palette;

use iced::Color;
use iced::Point;
use iced::Rectangle;

// ---------------------------------------------------------------------------
// Scrim
// ---------------------------------------------------------------------------

/// Alpha of the Band 3 dimmed scrim (§5, `UIF-ZORD-*`).
///
/// The spec states the scrim as `color.surface.base` at 55% alpha: it is
/// not a surface painted *instead of* the canvas but the canvas pulled
/// forward and dimmed. The alpha is the rule; the colour it is applied to
/// is a token. Naming the constant here keeps `0.55` from appearing a
/// second time wherever the scrim is composed (`RUST-021`), and
/// [`scrim`] is the only place the two meet.
pub const SCRIM_ALPHA: f32 = 0.55;

/// Compose the Band 3 scrim from a base surface colour.
///
/// Keeps the base's channels and replaces the alpha with
/// [`SCRIM_ALPHA`]. Applied to
/// [`Palette::surface_base`](crate::token::color::Palette::surface_base)
/// this reproduces the `color.surface.scrim` token exactly, which the
/// sidecar asserts.
///
/// Written as a struct update rather than `Color::from_rgba(base.r, ..)`
/// because that is what the rule actually is: the colour is inherited,
/// only the alpha is overridden. No colour constructor appears here, so
/// no raw colour value enters the surface layer and the `UIF-PRIN-001`
/// source scan needs no exception for it — the base's own channels carry
/// through untouched, including any field `Color` gains later.
#[must_use]
pub const fn scrim(base: Color) -> Color {
    Color {
        a: SCRIM_ALPHA,
        ..base
    }
}

/// [`scrim`] over a palette's `color.surface.base` token.
///
/// The production entry point for `INC-18`: the modal host asks for "the
/// scrim" and gets the token-composed value without naming a colour.
#[must_use]
pub const fn scrim_for(palette: &Palette) -> Color {
    scrim(palette.surface_base)
}

// ---------------------------------------------------------------------------
// Bands
// ---------------------------------------------------------------------------

/// One of the four strict z-bands of the layered window (`UIF-ZORD-*`).
///
/// The discriminants are the spec's band numbers, and the derived
/// ordering is bottom-to-top: `Workspace < UtilityOverlay < Menu < Modal`.
///
/// # Escape rank is not band rank
///
/// `UIF-WIN-003` unwinds **menus before dialogs**, which is the reverse
/// of the elevation in this enum. The two orderings answer different
/// questions — "what is painted in front" versus "what is the user
/// unwinding" — so they are modelled apart rather than one being derived
/// from the other and then contradicted.
///
/// A menu is normally opened *from* the surface it sits above: the user's
/// attention is on the menu, and the dialog behind it is its owner.
/// Dismissing the owner while its menu is still up would throw away the
/// context the menu was opened in. That is what the spec's numbered list
/// encodes, so [`escape_rank`](Self::escape_rank) carries it explicitly
/// instead of the walk guessing from band order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ZBand {
    /// Band 0 — the main window workspace: navbar, multi-panes, tabs.
    Workspace = 0,
    /// Band 1 — movable utility overlays (`Logs`, `Agent Debug`).
    UtilityOverlay = 1,
    /// Band 2 — fixed and context menu trees, with their submenus.
    Menu = 2,
    /// Band 3 — blocking modal and input dialogs.
    Modal = 3,
}

impl ZBand {
    /// Every band, bottom to top.
    pub const ALL: [Self; 4] = [
        Self::Workspace,
        Self::UtilityOverlay,
        Self::Menu,
        Self::Modal,
    ];

    /// The spec's band number, as printed in §5.
    #[must_use]
    pub const fn index(self) -> u8 {
        match self {
            Self::Workspace => 0,
            Self::UtilityOverlay => 1,
            Self::Menu => 2,
            Self::Modal => 3,
        }
    }

    /// What this band paints behind itself (§5 "Backdrop Behaviour").
    #[must_use]
    pub const fn backdrop(self) -> Backdrop {
        match self {
            Self::Workspace => Backdrop::None,
            Self::UtilityOverlay => Backdrop::None,
            Self::Menu => Backdrop::TransparentCatcher,
            Self::Modal => Backdrop::DimmedScrim,
        }
    }

    /// How this band treats a pointer event (§5 "Pointer Routing").
    #[must_use]
    pub const fn pointer_rule(self) -> PointerRule {
        match self {
            Self::Workspace => PointerRule::Direct,
            Self::UtilityOverlay => PointerRule::CaptureInsidePassOutside,
            Self::Menu => PointerRule::CaptureInsideDismissOutside,
            Self::Modal => PointerRule::CaptureInsideAbsorbOutside,
        }
    }

    /// Whether this band stops every lower band from receiving a pointer
    /// event at all (§5 Band 3: "Blocks Band 0 & Band 1 completely").
    ///
    /// Band 2 is deliberately *not* blocking in this sense: it swallows
    /// the outside click too, but by dismissing itself, not by standing
    /// between the pointer and everything below.
    #[must_use]
    pub const fn blocks_lower_bands(self) -> bool {
        matches!(self, Self::Modal)
    }

    /// Whether clicking this band's card raises it to the top of its own
    /// band and nothing else (§5 Band 1: "Z-promotion on click").
    #[must_use]
    pub const fn promotes_on_click(self) -> bool {
        matches!(self, Self::UtilityOverlay)
    }

    /// Where this band sits in the `UIF-WIN-003` unwinding order.
    ///
    /// Higher unwinds first: `Menu` → `Modal` → `UtilityOverlay` →
    /// `Workspace`. See the [module docs](self#escape-rank-is-not-band-rank)
    /// for why this is not the band ordering.
    #[must_use]
    pub const fn escape_rank(self) -> u8 {
        match self {
            Self::Workspace => 0,
            Self::UtilityOverlay => 1,
            Self::Menu => 3,
            Self::Modal => 2,
        }
    }

    /// What this band does with one class of key (§5 "Keyboard Routing").
    ///
    /// The table, band by band:
    ///
    /// | Key class | Band 0 | Band 1 | Band 2 | Band 3 |
    /// |---|---|---|---|---|
    /// | `Tab` / `Shift+Tab` | Swallow | Pass through | Swallow | Swallow |
    /// | Arrow navigation | Pass through | Pass through | Swallow | Pass through |
    /// | `Enter` | Pass through | Pass through | Swallow | Swallow |
    /// | `Space` | Pass through | Pass through | Swallow | Pass through |
    /// | `Escape` | Pass through | Swallow *when focused* | Swallow | Swallow |
    ///
    /// Band 0 swallows `Tab` because traversal *is* its keyboard routing;
    /// it swallows nothing else because it has nothing below to protect.
    /// Band 1 swallows only `Escape`, and only while its card holds focus
    /// — the modeless card must not eat keys the workspace needs. Band 2
    /// swallows the whole set: the cascade drives the arrows and the
    /// card's trap confines `Tab`. Band 3 traps `Tab`/`Shift+Tab`,
    /// `Enter` and `Escape` strictly inside the topmost modal
    /// (`UIF-MOD-001`); arrows and `Space` belong to whatever control
    /// inside the modal holds focus, not to the band.
    #[must_use]
    pub const fn key_disposition(self, class: KeyClass) -> KeyDisposition {
        match (self, class) {
            (Self::Workspace, KeyClass::Tab) | (Self::Workspace, KeyClass::ShiftTab) => {
                KeyDisposition::Swallow
            }
            (Self::Workspace, _) => KeyDisposition::PassThrough,
            (Self::UtilityOverlay, KeyClass::Escape) => KeyDisposition::SwallowWhenFocused,
            (Self::UtilityOverlay, _) => KeyDisposition::PassThrough,
            (Self::Menu, _) => KeyDisposition::Swallow,
            (
                Self::Modal,
                KeyClass::Tab | KeyClass::ShiftTab | KeyClass::Enter | KeyClass::Escape,
            ) => KeyDisposition::Swallow,
            (Self::Modal, _) => KeyDisposition::PassThrough,
        }
    }

    /// Every key class this band swallows unconditionally.
    ///
    /// The `SwallowWhenFocused` class is excluded — it is conditional, and
    /// folding it in would overstate what the band claims. Ask
    /// [`key_disposition`](Self::key_disposition) for the full answer.
    #[must_use]
    pub fn swallowed_keys(self) -> Vec<KeyClass> {
        KeyClass::ALL
            .into_iter()
            .filter(|class| self.key_disposition(*class) == KeyDisposition::Swallow)
            .collect()
    }
}

/// What a band paints behind itself (§5 "Backdrop Behaviour").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Backdrop {
    /// Nothing at all — the band floats without dimming or catching.
    None,
    /// A full-window transparent catcher: invisible, but every click
    /// outside the card belongs to the band (`UIF-CMENU-002`'s
    /// click-away).
    TransparentCatcher,
    /// The full-window dimmed scrim of [`SCRIM_ALPHA`]
    /// (`UIF-MOD-001`).
    DimmedScrim,
}

/// How a band treats a pointer event that lands outside its card(s)
/// (§5 "Pointer Routing").
///
/// The three non-base rules are the whole difference between the three
/// upper bands, which is why they are one enum rather than three
/// booleans: they are mutually exclusive, and a band that both passed
/// through and dismissed is not a thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerRule {
    /// No capture: the band is the base and receives whatever is left.
    Direct,
    /// Capture inside the card; outside, let the event fall to the band
    /// below (§5 Band 1: "passes outside clicks to Band 0").
    CaptureInsidePassOutside,
    /// Capture inside any open card; outside, dismiss the whole band and
    /// consume the event (§5 Band 2).
    CaptureInsideDismissOutside,
    /// Capture inside the card; outside, the scrim absorbs the event and
    /// no lower band is ever consulted (§5 Band 3).
    CaptureInsideAbsorbOutside,
}

/// A class of key, as the §5 keyboard column groups them.
///
/// A class rather than a physical key: the band model must not know what
/// the platform reported, only which bucket the host's mapping landed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyClass {
    /// `Tab`.
    Tab,
    /// `Shift+Tab`.
    ShiftTab,
    /// `Up` / `Down` / `Left` / `Right` as a single navigation class.
    ArrowNavigation,
    /// `Enter`.
    Enter,
    /// `Space`.
    Space,
    /// `Escape`.
    Escape,
}

impl KeyClass {
    /// Every key class, in table order.
    pub const ALL: [Self; 6] = [
        Self::Tab,
        Self::ShiftTab,
        Self::ArrowNavigation,
        Self::Enter,
        Self::Space,
        Self::Escape,
    ];
}

/// What a band does with a key class when it owns routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyDisposition {
    /// The band consumes the key and acts on it.
    Swallow,
    /// The band consumes the key only while its card holds keyboard focus
    /// (§5 Band 1: "`Escape` does not close unless focused").
    SwallowWhenFocused,
    /// The band leaves the key to the bands below, or to the focused
    /// control itself.
    PassThrough,
}

impl KeyDisposition {
    /// Whether this disposition takes the key out of circulation at all.
    #[must_use]
    pub const fn swallowed(self) -> bool {
        matches!(self, Self::Swallow | Self::SwallowWhenFocused)
    }
}

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// A stable identifier for a registered surface.
///
/// Same reasoning as [`FocusId`](crate::focus::FocusId): surfaces are
/// created and destroyed dynamically, so identity has to be a name the
/// host can speak before the surface is mounted and after it is gone.
///
/// [`FocusId`](crate::focus::FocusId) is deliberately *not* reused. A
/// surface is not a focus member — it is a region of the window that owns
/// routing, and it contains many focusable controls. Letting the two share
/// a type would make "register the surface" and "focus the control"
/// interchangeable at the call site, and they are not.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SurfaceId(String);

impl SurfaceId {
    /// Create an id from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Read the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SurfaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for SurfaceId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for SurfaceId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

// ---------------------------------------------------------------------------
// Surfaces
// ---------------------------------------------------------------------------

/// What kind of surface is being registered — which decides its band.
///
/// The band is *derived* from the kind rather than passed alongside it,
/// because the two arriving separately is how a modal ends up registered
/// into Band 1. There is exactly one place that can know a `Modal` is a
/// Band 3 surface, and this is it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SurfaceKind {
    /// Band 0: a workspace region — the navbar, a pane, a tab strip.
    Workspace,
    /// Band 0: the active search or filter field, the last stop of
    /// `UIF-WIN-003`'s step 4.
    SearchFilter,
    /// Band 1: a movable utility overlay card (`UIF-MOV-*`).
    MovableCard,
    /// Band 1: the transient toast / snackbar notification stack
    /// (§3B.2, `UIF-COMP-002`).
    ///
    /// Its own kind rather than a borrowed [`MovableCard`](Self::MovableCard)
    /// even though the band is identical, because the two are not the
    /// same thing to a keyboard: a movable card is a long-lived window
    /// the user owns, and a toast is a thing that leaves. Keeping them
    /// distinct is what lets an `Escape` trace, a pointer route and a
    /// gallery cell say *which* Band 1 surface acted, and it costs one
    /// variant that maps to a band that already exists.
    ///
    /// §5's Band 1 row already names the "Toast / SnackBar notification
    /// stack" beside the movable overlays, so this borrows that row's
    /// pass-through backdrop, its click-to-promote rule and its
    /// `Escape Rank 1 when focused` unchanged.
    ToastStack,
    /// Band 2: one open card of a menu tree.
    ///
    /// Register **each open card separately**, root first. That is what
    /// makes "deepest submenu, then root menu" fall out of the generic
    /// escape walk rather than needing a depth parameter.
    MenuCard,
    /// Band 2: an interactive teaching popover (§3.7, `UIF-ATOM-007`).
    ///
    /// §3.7 says the callout "mounts in `Z-Band 2` (`Escape Rank 3`)",
    /// and that reconciles with this enum exactly rather than needing a
    /// band of its own: [`ZBand::Menu`] *is* index `2` and its
    /// [`escape_rank`](ZBand::escape_rank) *is* `3`, and §5's band
    /// table already lists `TeachingPopover` in that row beside the
    /// menus. So the kind borrows the `Menu` band and inherits its
    /// `TransparentCatcher` backdrop, its `CaptureInsideDismissOutside`
    /// pointer rule and its rank unchanged.
    ///
    /// This is the opposite of the tooltip's decision to stay out of the
    /// registry, and the reason is interactivity: a pointer-transparent
    /// label has no pointer ownership to register, and a callout with a
    /// click-away catcher and a consumed `Escape` is nothing *but*
    /// pointer and keyboard ownership. See the
    /// [popover module docs](crate::surface::popover) for the three
    /// tooltip reasons this inverts.
    TeachingPopover,
    /// Band 3: a blocking modal dialog (`UIF-MOD-*`).
    Modal,
    /// Band 3: a cursor-anchored input dialog (`UIF-INP-*`).
    InputDialog,
}

impl SurfaceKind {
    /// The band this kind belongs to (`UIF-ZORD-*`).
    #[must_use]
    pub const fn band(self) -> ZBand {
        match self {
            Self::Workspace | Self::SearchFilter => ZBand::Workspace,
            Self::MovableCard | Self::ToastStack => ZBand::UtilityOverlay,
            Self::MenuCard | Self::TeachingPopover => ZBand::Menu,
            Self::Modal | Self::InputDialog => ZBand::Modal,
        }
    }
}

/// One registered surface: its identity, its kind, and the regions of the
/// root coordinate space it occupies (`UIF-WIN-002`).
///
/// Regions are a *list* because one surface can own more than one
/// rectangle. A menu tree registered as a single surface needs the union
/// of its open cards; more commonly a card with a detached footer or a
/// torn-off title bar is two rectangles under one identity.
///
/// A surface with **no regions is never hit by the pointer**. It still
/// stacks, still counts, and still takes part in Escape. That is a real
/// and useful case for a keyboard-only surface, but it means a full-window
/// modal must be registered with the viewport rectangle — leaving the
/// list empty would make every click land on its own scrim.
#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    id: SurfaceId,
    kind: SurfaceKind,
    regions: Vec<Rectangle>,
}

impl Surface {
    /// Build a surface. Usually reached through
    /// [`SurfaceRegistry::register`] rather than directly.
    #[must_use]
    pub fn new(id: SurfaceId, kind: SurfaceKind, regions: Vec<Rectangle>) -> Self {
        Self { id, kind, regions }
    }

    /// This surface's stable identity.
    #[must_use]
    pub fn id(&self) -> &SurfaceId {
        &self.id
    }

    /// What kind of surface this is.
    #[must_use]
    pub const fn kind(&self) -> SurfaceKind {
        self.kind
    }

    /// The band this surface lives in.
    #[must_use]
    pub const fn band(&self) -> ZBand {
        self.kind.band()
    }

    /// The rectangles this surface occupies, in root coordinates.
    #[must_use]
    pub fn regions(&self) -> &[Rectangle] {
        &self.regions
    }

    /// Whether this surface has any region at all.
    #[must_use]
    pub fn has_regions(&self) -> bool {
        !self.regions.is_empty()
    }

    /// Whether `point` lands on this surface.
    ///
    /// Hit testing is iced's own: the right and bottom edges are
    /// *excluded*, so two rectangles that abut exactly do not overlap and
    /// a `NaN` coordinate is contained by nothing.
    #[must_use]
    pub fn contains(&self, point: Point) -> bool {
        self.regions.iter().any(|region| region.contains(point))
    }
}

// ---------------------------------------------------------------------------
// Pointer routing
// ---------------------------------------------------------------------------

/// Who receives a pointer event at a point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PointerTarget {
    /// A registered surface owns the point and receives the event.
    Surface(SurfaceId),
    /// The point missed the topmost open band and that band's outside rule
    /// fired by dismissing the whole band (§5 Band 2's click-away).
    ///
    /// The event is consumed: the workspace underneath does **not** get
    /// it, which is the difference between a click-away catcher and a
    /// pass-through card.
    DismissBand(ZBand),
    /// The point missed the Band 3 card and landed on the scrim.
    ///
    /// The event is absorbed — no lower band sees it — but whether the
    /// dialog actually closes is not this model's call: that is the
    /// dirty-state guard `UIF-MOD-002`, and it belongs to `INC-18`.
    Scrim(SurfaceId),
    /// Nothing above the base band claimed the event; the workspace
    /// receives it directly.
    Workspace,
}

impl PointerTarget {
    /// The surface named by this target, if it names one.
    #[must_use]
    pub fn surface_id(&self) -> Option<&SurfaceId> {
        match self {
            Self::Surface(id) | Self::Scrim(id) => Some(id),
            Self::DismissBand(_) | Self::Workspace => None,
        }
    }

    /// Whether a registered surface receives the event (as opposed to a
    /// band rule firing).
    #[must_use]
    pub fn is_surface_hit(&self) -> bool {
        matches!(self, Self::Surface(_))
    }
}

/// A pointer routing decision, with the pass-through trace.
///
/// The trace matters because "the workspace got the click" and "the
/// workspace got the click *after* a Band 1 card passed it down" are the
/// same answer to a different question. A host deciding whether to
/// dismiss a modeless card, or a test asserting a pass-through happened,
/// needs the path and not just the destination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointerRoute {
    target: PointerTarget,
    passed_through: Vec<ZBand>,
}

impl PointerRoute {
    /// Where the event landed.
    #[must_use]
    pub fn target(&self) -> &PointerTarget {
        &self.target
    }

    /// The bands that let the event fall past them, highest first.
    #[must_use]
    pub fn passed_through(&self) -> &[ZBand] {
        &self.passed_through
    }

    /// Whether the event reached the base workspace.
    #[must_use]
    pub fn reaches_workspace(&self) -> bool {
        matches!(self.target, PointerTarget::Workspace)
    }

    /// Whether the event was stopped by a band's outside rule rather than
    /// by a surface.
    #[must_use]
    pub fn is_band_rule(&self) -> bool {
        matches!(
            self.target,
            PointerTarget::DismissBand(_) | PointerTarget::Scrim(_)
        )
    }
}

// ---------------------------------------------------------------------------
// Escape unwinding
// ---------------------------------------------------------------------------

/// A surface's answer when the escape chain asks it to unwind one level.
///
/// Three answers, because there are three things a surface can do: close
/// a level and stay open, close and go away, or say no. "Say no" is not a
/// failure — it is how the key travels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnwindAnswer {
    /// The surface consumed the key and is still open. It may be asked
    /// again — a search field that cleared its text, a menu that closed a
    /// level but kept its card.
    Unwound,
    /// The surface consumed the key and closed. The registry removes it,
    /// so the next `Escape` reaches what was underneath.
    Dismissed,
    /// The surface has nothing to unwind, so the key moves on.
    Declined(DeclineReason),
}

impl UnwindAnswer {
    /// Adapt a
    /// [`MenuCascade`](crate::surface::menu::cascade::MenuCascade) close attempt into an unwind answer.
    ///
    /// This is the seam `INC-16` left on purpose. The cascade already
    /// answers a second `Escape` with [`IgnoredReason::NotOpen`] — its
    /// level is spent and the key keeps travelling — so the band model
    /// adopts that shape instead of inventing a parallel one. Anything
    /// else the cascade refused (`UnknownCard`, `NothingFocused`, …) is a
    /// dead end for this key rather than a "keep going", and maps to
    /// [`DeclineReason::CannotUnwind`] so it is not mistaken for the
    /// band having drained.
    ///
    /// Call it on a **close** outcome ([`close_deepest`],
    /// [`close_tree`]). A close that closed at least one card is
    /// [`UnwindAnswer::Dismissed`]; an accepted close that closed nothing is
    /// [`UnwindAnswer::Unwound`], which is as much as the band model can tell.
    ///
    /// [`close_deepest`]: crate::surface::menu::cascade::MenuCascade::close_deepest
    /// [`close_tree`]: crate::surface::menu::cascade::MenuCascade::close_tree
    #[must_use]
    pub fn from_cascade_close(outcome: &CascadeOutcome) -> Self {
        if let Some(reason) = outcome.ignored_reason() {
            return match reason {
                IgnoredReason::NotOpen => Self::Declined(DeclineReason::NotOpen),
                _ => Self::Declined(DeclineReason::CannotUnwind),
            };
        }
        if outcome.closed().is_empty() {
            Self::Unwound
        } else {
            Self::Dismissed
        }
    }
}

/// Why a surface refused to unwind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclineReason {
    /// The surface is not open. Mirrors the cascade's
    /// [`IgnoredReason::NotOpen`]: this layer's turn has passed.
    NotOpen,
    /// The surface is open but this `Escape` is not its turn to act —
    /// §5 Band 1's "`Escape` does not close unless focused" is the
    /// standing example.
    NotFocused,
    /// The surface is open but cannot unwind right now, for a reason that
    /// is its own and not "there is nothing left".
    CannotUnwind,
}

/// A surface that refused, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declination {
    /// The surface that refused.
    pub surface: SurfaceId,
    /// Why it refused.
    pub reason: DeclineReason,
}

/// Why no surface consumed the `Escape`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EscapeRefusal {
    /// Nothing is registered in any band. The defined no-op
    /// `UIF-WIN-003` needs at the bottom of the stack — not a panic, not
    /// a swallowed key.
    EmptyStack,
    /// Surfaces are open but every one of them declined.
    AllDeclined {
        /// Every surface that refused, in the order it was asked.
        declined: Vec<Declination>,
    },
}

impl EscapeRefusal {
    /// Whether the refusal was simply an empty stack.
    #[must_use]
    pub fn is_empty_stack(&self) -> bool {
        matches!(self, Self::EmptyStack)
    }
}

/// What one `Escape` did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EscapeOutcome {
    /// A surface consumed the key.
    Consumed {
        /// The consumer.
        surface: SurfaceId,
        /// What kind of surface it was, so the report reads as "the
        /// submenu" rather than as an opaque id.
        kind: SurfaceKind,
        /// The band it lived in.
        band: ZBand,
        /// Whether it closed (`Dismissed`) or merely unwound a level and
        /// stayed registered.
        dismissed: bool,
        /// Surfaces that were asked first and refused, in the order they
        /// were asked. Empty in the common case.
        declined_before: Vec<Declination>,
    },
    /// Nothing consumed the key.
    Ignored(EscapeRefusal),
}

impl EscapeOutcome {
    /// Whether a surface consumed the key.
    #[must_use]
    pub fn is_consumed(&self) -> bool {
        matches!(self, Self::Consumed { .. })
    }

    /// The consumer's id, when there was one.
    #[must_use]
    pub fn consumed_by(&self) -> Option<&SurfaceId> {
        match self {
            Self::Consumed { surface, .. } => Some(surface),
            Self::Ignored(_) => None,
        }
    }

    /// Whether the consumer closed outright.
    #[must_use]
    pub fn dismissed(&self) -> bool {
        matches!(
            self,
            Self::Consumed {
                dismissed: true,
                ..
            }
        )
    }

    /// The refusal, when nothing consumed the key.
    #[must_use]
    pub fn refusal(&self) -> Option<&EscapeRefusal> {
        match self {
            Self::Consumed { .. } => None,
            Self::Ignored(refusal) => Some(refusal),
        }
    }

    /// Surfaces asked and refused before the consumer was reached.
    #[must_use]
    pub fn declined_before(&self) -> &[Declination] {
        match self {
            Self::Consumed {
                declined_before, ..
            } => declined_before,
            Self::Ignored(EscapeRefusal::AllDeclined { declined }) => declined,
            Self::Ignored(EscapeRefusal::EmptyStack) => &[],
        }
    }
}

// ---------------------------------------------------------------------------
// The registry
// ---------------------------------------------------------------------------

/// The registry of open surfaces across all four bands.
///
/// The invariant is one sentence and everything else follows from it:
/// **the list is sorted by band, lowest band first, and within a band in
/// bottom-first stacking order.** `topmost()` is therefore the last
/// element, the highest open band is the last band present, and
/// promotion inside a band is a move to the end of that band's run.
///
/// A pure model: nothing renders, nothing reads a clock, nothing runs a
/// task. The host registers what it has mounted, asks where a click goes,
/// and drives Escape.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceRegistry {
    surfaces: Vec<Surface>,
}

impl SurfaceRegistry {
    /// An empty registry — nothing open, the workspace owns everything.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // -- registration ------------------------------------------------------

    /// Register a surface into its kind's band, or refresh one already
    /// registered.
    ///
    /// A new surface goes on top of its band. Re-registering an existing
    /// id replaces its regions but **keeps its position**, so a card that
    /// moves, resizes or is rebuilt every frame cannot change the
    /// stacking order — and a Band 1 card the user promoted stays
    /// promoted across the rebuild.
    ///
    /// Re-registering with a kind in a *different* band is honoured: the
    /// surface moves to the top of its new band, and every other surface
    /// keeps its relative order.
    pub fn register(
        &mut self,
        id: SurfaceId,
        kind: SurfaceKind,
        regions: Vec<Rectangle>,
    ) -> &Surface {
        let band = kind.band();
        match self.surfaces.iter().position(|surface| surface.id == id) {
            Some(index) if self.surfaces[index].band() == band => {
                self.surfaces[index].kind = kind;
                self.surfaces[index].regions = regions;
                &self.surfaces[index]
            }
            Some(index) => {
                self.surfaces.remove(index);
                let target = self
                    .surfaces
                    .partition_point(|surface| surface.band() <= band);
                self.surfaces
                    .insert(target, Surface::new(id, kind, regions));
                &self.surfaces[target]
            }
            None => {
                let index = self
                    .surfaces
                    .partition_point(|surface| surface.band() <= band);
                self.surfaces.insert(index, Surface::new(id, kind, regions));
                &self.surfaces[index]
            }
        }
    }

    /// Remove a surface.
    ///
    /// Returns what was removed, or `None` when no such surface is
    /// registered. Removing a surface that was not the topmost does not
    /// disturb the order of anything else.
    pub fn unregister(&mut self, id: &SurfaceId) -> Option<Surface> {
        let index = self.surfaces.iter().position(|surface| surface.id == *id)?;
        Some(self.surfaces.remove(index))
    }

    /// Forget every surface.
    pub fn clear(&mut self) {
        self.surfaces.clear();
    }

    // -- queries -----------------------------------------------------------

    /// Whether nothing is open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }

    /// How many surfaces are registered, across all bands.
    #[must_use]
    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    /// Every surface, lowest band first and bottom-of-band first.
    #[must_use]
    pub fn surfaces(&self) -> &[Surface] {
        &self.surfaces
    }

    /// Look a surface up by id.
    #[must_use]
    pub fn get(&self, id: &SurfaceId) -> Option<&Surface> {
        self.surfaces.iter().find(|surface| surface.id == *id)
    }

    /// Whether `id` is registered.
    #[must_use]
    pub fn contains(&self, id: &SurfaceId) -> bool {
        self.get(id).is_some()
    }

    /// The surfaces in one band, bottom first.
    pub fn surfaces_in(&self, band: ZBand) -> impl DoubleEndedIterator<Item = &Surface> {
        self.surfaces
            .iter()
            .filter(move |surface| surface.band() == band)
    }

    /// How many surfaces are open in one band.
    #[must_use]
    pub fn count_in(&self, band: ZBand) -> usize {
        self.surfaces_in(band).count()
    }

    /// Whether a band has anything open.
    #[must_use]
    pub fn band_is_empty(&self, band: ZBand) -> bool {
        self.count_in(band) == 0
    }

    /// The topmost surface within one band.
    #[must_use]
    pub fn topmost_in(&self, band: ZBand) -> Option<&Surface> {
        self.surfaces_in(band).next_back()
    }

    /// The surface that currently owns input routing: the top of the
    /// highest band with anything open.
    ///
    /// This is the whole point of the sorted invariant — with a modal and
    /// three menu cards open, the modal is the answer, and it is the last
    /// element of the list.
    #[must_use]
    pub fn topmost(&self) -> Option<&Surface> {
        self.surfaces.last()
    }

    /// The highest band with anything open.
    #[must_use]
    pub fn highest_open_band(&self) -> Option<ZBand> {
        self.topmost().map(Surface::band)
    }

    /// The band a host should consult first for key routing.
    ///
    /// The highest band with anything open, because that band's trap or
    /// capture is what the keyboard is inside. **`Escape` is the
    /// exception**: it does not go to the highest band, it goes through
    /// the [`escape`](Self::escape) chain, which puts the menu band above
    /// the modal band.
    #[must_use]
    pub fn keyboard_owner(&self) -> Option<ZBand> {
        self.highest_open_band()
    }

    // -- ordering ----------------------------------------------------------

    /// Raise a surface to the top of its own band, changing nothing else
    /// (§5 Band 1: "promotes clicked card to top of Band 1").
    ///
    /// Only bands whose [`promotes_on_click`](ZBand::promotes_on_click)
    /// is set reorder automatically; this method can raise any band on
    /// explicit request, but no band but Band 1 does it by itself.
    ///
    /// Returns whether the order actually changed — `false` for an
    /// unknown id and for a surface already on top.
    pub fn promote(&mut self, id: &SurfaceId) -> bool {
        let Some(index) = self.surfaces.iter().position(|surface| surface.id == *id) else {
            return false;
        };
        let band = self.surfaces[index].band();
        if index + 1
            == self
                .surfaces
                .partition_point(|surface| surface.band() <= band)
        {
            return false;
        }
        let surface = self.surfaces.remove(index);
        let target = self
            .surfaces
            .partition_point(|surface| surface.band() <= band);
        self.surfaces.insert(target, surface);
        true
    }

    // -- pointer routing ---------------------------------------------------

    /// Route a pointer event at `point` through the bands.
    ///
    /// Pure: answers "who would get this click" without changing
    /// anything, promotion included. Walks the bands top down and stops
    /// at the first band with anything open:
    ///
    /// 1. the topmost surface in that band containing `point` receives it;
    /// 2. otherwise the band's [`PointerRule`] fires — pass through and
    ///    keep walking, dismiss the band, or absorb on the scrim;
    /// 3. a miss on the base band, or no band open at all, is
    ///    [`PointerTarget::Workspace`].
    #[must_use]
    pub fn route_pointer(&self, point: Point) -> PointerRoute {
        let mut passed_through = Vec::new();
        for band in ZBand::ALL.iter().rev().copied() {
            let Some(top) = self.topmost_in(band) else {
                continue;
            };
            if let Some(surface) = self.hit_in(band, point) {
                return PointerRoute {
                    target: PointerTarget::Surface(surface.id.clone()),
                    passed_through,
                };
            }
            match band.pointer_rule() {
                PointerRule::Direct => {
                    return PointerRoute {
                        target: PointerTarget::Workspace,
                        passed_through,
                    };
                }
                PointerRule::CaptureInsidePassOutside => passed_through.push(band),
                PointerRule::CaptureInsideDismissOutside => {
                    return PointerRoute {
                        target: PointerTarget::DismissBand(band),
                        passed_through,
                    };
                }
                PointerRule::CaptureInsideAbsorbOutside => {
                    return PointerRoute {
                        target: PointerTarget::Scrim(top.id().clone()),
                        passed_through,
                    };
                }
            }
        }
        PointerRoute {
            target: PointerTarget::Workspace,
            passed_through,
        }
    }

    /// Route a pointer *press* and apply the band's own follow-up.
    ///
    /// Same answer as [`route_pointer`](Self::route_pointer), then
    /// [`promote`](Self::promote) when the surface hit lives in a band
    /// that promotes on click. The promotion is driven entirely by
    /// [`ZBand::promotes_on_click`], so it cannot be applied to a band
    /// the spec did not ask for it in.
    #[must_use]
    pub fn pointer_down(&mut self, point: Point) -> PointerRoute {
        let route = self.route_pointer(point);
        if let PointerTarget::Surface(id) = route.target()
            && self
                .get(id)
                .is_some_and(|surface| surface.band().promotes_on_click())
        {
            self.promote(id);
        }
        route
    }

    /// The topmost surface in `band` containing `point`.
    fn hit_in(&self, band: ZBand, point: Point) -> Option<&Surface> {
        self.surfaces_in(band)
            .rev()
            .find(|surface| surface.contains(point))
    }

    // -- escape ------------------------------------------------------------

    /// Drive one `Escape` through the delegation chain
    /// (`UIF-WIN-003`, `UIF-VER-003`).
    ///
    /// Walks the registered surfaces in escape order — highest
    /// [`escape_rank`](ZBand::escape_rank) first, and within a band the
    /// topmost surface first, which is what puts the deepest submenu
    /// ahead of the root menu — and asks each one to unwind through
    /// `unwind`. The first surface that accepts ends the walk:
    ///
    /// * [`UnwindAnswer::Unwound`] — consumed, still registered;
    /// * [`UnwindAnswer::Dismissed`] — consumed and removed, so the next
    ///   `Escape` reaches what was underneath it;
    /// * [`UnwindAnswer::Declined`] — recorded and the walk continues.
    ///
    /// An empty registry never calls `unwind` and answers
    /// [`EscapeRefusal::EmptyStack`]: the defined no-op, not a panic.
    ///
    /// The model never decides what "unwind" means for a menu, a modal or
    /// a filter. It asks. For a menu card the answer comes straight from
    /// the cascade's own refusal via
    /// [`UnwindAnswer::from_cascade_close`].
    ///
    /// **This is `UIF-WIN-003` steps 2 through 5, not step 1.** A
    /// pointer-captured drag is not a registered surface, so this walk cannot
    /// see one. A host with drag-capable controls must run the Pre-Rank
    /// cancel first — see [`escape_with_pre_rank`], which composes the two
    /// and is the entry point such a host should be calling.
    pub fn escape<F>(&mut self, mut unwind: F) -> EscapeOutcome
    where
        F: FnMut(&Surface) -> UnwindAnswer,
    {
        if self.surfaces.is_empty() {
            return EscapeOutcome::Ignored(EscapeRefusal::EmptyStack);
        }

        let mut declined: Vec<Declination> = Vec::new();
        for index in self.escape_order() {
            let answer = unwind(&self.surfaces[index]);
            match answer {
                UnwindAnswer::Unwound => {
                    let surface = &self.surfaces[index];
                    return EscapeOutcome::Consumed {
                        surface: surface.id.clone(),
                        kind: surface.kind,
                        band: surface.band(),
                        dismissed: false,
                        declined_before: declined,
                    };
                }
                UnwindAnswer::Dismissed => {
                    let band = self.surfaces[index].band();
                    let surface = self.surfaces.remove(index);
                    return EscapeOutcome::Consumed {
                        surface: surface.id,
                        kind: surface.kind,
                        band,
                        dismissed: true,
                        declined_before: declined,
                    };
                }
                UnwindAnswer::Declined(reason) => declined.push(Declination {
                    surface: self.surfaces[index].id.clone(),
                    reason,
                }),
            }
        }

        EscapeOutcome::Ignored(EscapeRefusal::AllDeclined { declined })
    }

    /// The escape walk order: indices, highest rank first, and within a
    /// rank the highest position in the registry first.
    fn escape_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.surfaces.len()).collect();
        order.sort_by_key(|&index| {
            (
                Reverse(self.surfaces[index].band().escape_rank()),
                Reverse(index),
            )
        });
        order
    }
}

// ---------------------------------------------------------------------------
// The Pre-Rank composition
// ---------------------------------------------------------------------------

/// One `Escape` driven through the whole `UIF-WIN-003` chain, Pre-Rank first.
///
/// The two steps have different subjects — one is a control's drag, the other
/// is a surface — so the outcome says which of them the key went to rather
/// than pretending both are a surface dismissal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreRankedEscape {
    /// Step 1 fired: the user was holding a pointer-captured drag, and it was
    /// cancelled with the control's `Cancel` tier. The key is spent and no
    /// surface was asked anything.
    DragCancelled {
        /// Which control's drag took the key.
        capture: DragCapture,
    },
    /// Step 1 found nothing to cancel, so the rank walk ran.
    RankWalked {
        /// What the rank walk did.
        outcome: EscapeOutcome,
        /// Why the Pre-Rank stepped aside — no capture, or a stale one.
        pre_rank: PreRankOutcome,
    },
}

impl PreRankedEscape {
    /// Whether the Pre-Rank consumed the key on a drag.
    #[must_use]
    pub fn drag_cancelled(&self) -> bool {
        matches!(self, Self::DragCancelled { .. })
    }

    /// The surface outcome, if the rank walk ran. `None` when the Pre-Rank
    /// consumed the key, because no surface was asked.
    #[must_use]
    pub fn outcome(&self) -> Option<&EscapeOutcome> {
        match self {
            Self::DragCancelled { .. } => None,
            Self::RankWalked { outcome, .. } => Some(outcome),
        }
    }

    /// What the Pre-Rank step answered, either way.
    #[must_use]
    pub fn pre_rank(&self) -> PreRankOutcome {
        match self {
            Self::DragCancelled { capture } => PreRankOutcome::Cancelled {
                capture: capture.clone(),
            },
            Self::RankWalked { pre_rank, .. } => pre_rank.clone(),
        }
    }
}

/// Drive one `Escape` through `UIF-WIN-003` in full: the Pre-Rank drag
/// cancel first, then the rank walk.
///
/// This is the entry point a host with **any** drag-capable control should
/// call, and the only way to get the spec's ordering without the host having
/// to remember the rule itself:
///
/// 1. **Pre-Rank (§3.0.4.2, `UIF-WIN-003` step 1).** If
///    [`PreRankCoordinator`]
///    holds a pointer-captured drag, `cancel_capture` is called with it and
///    must settle that control through its own cancel path — the one that
///    restores the pre-drag snapshot. If it reports a `Cancel` was produced,
///    the key is spent and [`SurfaceRegistry::escape`] is **not** called.
/// 2. **Rank walk (steps 2 through 5).** Otherwise the registry walks as it
///    always has, unchanged.
///
/// The Pre-Rank is unconditional and reads no focus, which is what makes the
/// rule work when the drag and the keyboard focus are in different places:
/// nothing here compares them, because the drag wins regardless.
///
/// `cancel_capture` returns `true` when the control produced a `Cancel` tier
/// and `false` when it had nothing open; the latter is a stale registration
/// and lets the key continue rather than vanish.
#[must_use]
pub fn escape_with_pre_rank<C, U>(
    captures: &mut PreRankCoordinator,
    registry: &mut SurfaceRegistry,
    cancel_capture: C,
    unwind: U,
) -> PreRankedEscape
where
    C: FnMut(&DragCapture) -> bool,
    U: FnMut(&Surface) -> UnwindAnswer,
{
    match captures.pre_rank_cancel(cancel_capture) {
        PreRankOutcome::Cancelled { capture } => PreRankedEscape::DragCancelled { capture },
        pre_rank @ (PreRankOutcome::Stale { .. } | PreRankOutcome::NotCaptured) => {
            PreRankedEscape::RankWalked {
                outcome: registry.escape(unwind),
                pre_rank,
            }
        }
    }
}

#[cfg(test)]
#[path = "zorder_tests.rs"]
mod zorder_tests;
