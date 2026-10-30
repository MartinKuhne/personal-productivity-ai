//! The modal dialog host — scrim, focus trap, card-level click absorption,
//! the unsaved-edit guard and the structured error banner
//! (`UIF-MOD-001`, `UIF-MOD-002`, `UIF-MOD-003`, `UIF-VER-003`).
//!
//! # What the host is, and what it is not
//!
//! §4.6 gives a modal its anatomy and three invariants, and the card's
//! actual contents are the application's business. This module is therefore
//! **not** a widget. It is the *frame*: the thing that decides, for every
//! key press and every pointer press, whether the dialog closes, refuses,
//! moves focus, or simply swallows the event — and it decides all of that
//! from geometry and declared state, with no renderer in sight.
//!
//! That separation is what makes the three invariants structural rather
//! than conventional. Each one is enforced by *where the code can reach*,
//! not by a check the caller is asked to remember:
//!
//! * **`UIF-MOD-001`** is [`FocusState::push_trap`] around the card's own
//!   focus group. The host never *asks* whether a move is inside the card;
//!   the model cannot produce one that is not, and every path — `Tab`,
//!   click-to-focus, programmatic focus — passes the same gate.
//! * **`UIF-MOD-002`** is a single private function, `attempt_dismiss`, and
//!   it is the *only* route to a user-initiated close. Escape, the scrim and
//!   the header's Close control all go through it, so there is no second
//!   door to leave unlocked.
//! * **`UIF-MOD-003`** is the error slot's mutation set: `set_error`,
//!   `clear_error`, and the banner's own recovery affordance. Nothing else
//!   in the host can write it, which is why "clicking elsewhere in the card
//!   must not dismiss the error" is not a rule but a non-event.
//!
//! # The scrim
//!
//! The scrim is not recomposed here. [`scrim_for`]
//! is the crate's one composition of
//! [`surface.scrim`](crate::token::color::Palette::surface_scrim), and
//! [`ModalFrame`] resolves straight through it, so the `0.55` alpha cannot
//! drift from the token. The scrim's *bounds* are the viewport: it is a
//! full-window surface, which is also why the host registers its z-order
//! region as the viewport rectangle (see the z-order section below).
//!
//! # The card-level absorption boundary
//!
//! `UIF-VER-003` names the two cases that must never dismiss: a click on a
//! **disabled** primary button inside the card, and a click on **empty card
//! padding**. Both are handled by the shape of
//! [`on_pointer`](ModalHost::on_pointer) rather than by a case in a list.
//!
//! The function tests the card rectangle **first**. If the point is inside,
//! control flow can only continue into the `match` on what the caller says
//! was hit — and no arm of that `match` can reach the scrim, because the
//! scrim branch is the `else` of the containment test that has already been
//! taken. A disabled control cannot leak upward through a boundary it was
//! never outside of.
//!
//! Two further guards sit behind that one:
//!
//! * A `Control(id)` report for an id that is **not a member of the card's
//!   focus group**, or that the focus model refuses (disabled, or confined
//!   by a trap further up), is downgraded to [`CardTarget::Disabled`] and
//!   absorbed. The host checks the roster rather than trusting the report.
//! * Geometry outranks the caller's claim in both directions: a
//!   `CloseButton` report whose point is *outside* the card takes the scrim
//!   path, and a `Padding` report whose point is outside takes the scrim
//!   path too. The card boundary is the host's own rectangle, not the
//!   caller's description of it.
//!
//! # The dirty guard and the Escape chain
//!
//! Dirty state is **supplied by the caller** and never inferred: the host
//! holds a flag the host itself cannot set from an interaction. When a
//! dismiss is attempted against a dirty card, the host reports
//! [`ModalOutcome::DismissRefused`] and changes *nothing* — not the trap,
//! not the focus, not the error, not the registration. Refusing is not
//! "half a close"; it is a no-op with a reason attached.
//!
//! Through the z-order chain the same guard answers
//! [`UnwindAnswer::Declined`] with [`DeclineReason::CannotUnwind`]
//! ([`unwind_answer`](ModalHost::unwind_answer)), and answers
//! [`UnwindAnswer::Dismissed`] when the card is clean. That is the whole
//! `UIF-WIN-003` step-3 integration: the band model asks, the modal
//! answers, and the refusal vocabulary is the cascade's own.
//!
//! One hard rule governs that closure: **the answer must not mutate the
//! registry.** [`SurfaceRegistry::escape`] walks by index and removes the
//! surface it was told closed; a closure that unregistered first would
//! shift the list under the walk and delete the wrong surface. So
//! `unwind_answer` tears down the trap and the focus group only, and lets
//! the walk remove the surface.
//!
//! # Tearing down in the right order
//!
//! Every close path runs the host's private `teardown`: **pop the trap,
//! then unregister the focus group**, then (outside the escape closure)
//! unregister the surface. Popping first is what makes the recorded
//! pre-modal focus the thing that gets restored under the trap's own
//! semantics; unregistering the group afterwards then cannot disturb it,
//! because the trap that carried the restore is already gone. The reverse
//! order leans on `unregister_group`'s special case for "the topmost trap
//! pointed at the group being removed" — it happens to work, but it makes
//! the restore an accident of removal order instead of the host's intent.
//!
//! If nothing held focus when the modal opened, focus **clears** on the way
//! out rather than keeping whatever the card picked up. That is
//! `pop_trap`'s rule, and the host does not paper over it.
//!
//! # Sizing (§4.6.2)
//!
//! [`ModalSizing`] is the §4.6.2 table as data: each axis is a
//! [`Desired`] size, a floor and a [`Cap`], where both the desired size and
//! the cap may be a fixed number, a fraction of the viewport, or the
//! measured content extent. [`solve`](ModalSizing::solve) applies them in
//! the order the spec implies and then applies the one clamp every row
//! shares — the card never crosses [`VIEWPORT_GUTTER`], which is the
//! "`viewport.width - 32px`" of the Tools and Batch rows generalised to
//! both axes — and centres the result.
//!
//! The floor loses to the gutter on purpose. A 520px-minimum editor in a
//! 400px window is 368px wide, not 520px overflowing: a card the user
//! cannot see all of is worse than a card that is narrower than its
//! designer wanted.
//!
//! A card the **user resizes** is bounded by the same machinery rather than
//! a second copy of it: [`ResizeFloors::resolve`] hands the dragged extent
//! to [`AxisPolicy::resolve`], so the gutter keeps exactly one owner no
//! matter which way the size was arrived at.
//!
//! # Z-order integration
//!
//! The host registers as [`SurfaceKind::Modal`] — Band 3,
//! [`blocks_lower_bands`](ZBand::blocks_lower_bands) `true` — with the
//! **viewport rectangle** as its region. That is INC-17's no-regions edge
//! case read the right way round: a surface with no region is never hit by
//! the pointer, so a full-window modal that registered nothing would send
//! every click, including the ones on its own card, to its own scrim.
//!
//! Because the region is the whole viewport, the registry answers "the
//! modal owns this point" for every point, and the card-versus-scrim
//! distinction is the host's own [`hit_test`](ModalHost::hit_test). What
//! the registration buys is the blocking: the walk stops in Band 3, so
//! `PointerTarget::Workspace` is unreachable while the modal is open and
//! an absorbed click is never forwarded downward.
//!
//! Nested modals fall out of the same invariant. Two modals both cover the
//! viewport; the topmost in the band is the one the registry hits, and
//! [`FocusState`] confines traversal to the last trap pushed. The
//! confirm-over-modal case — a dirty editor, `Escape` refused, a discard
//! confirmation stacked on top — is therefore not a special case at all.
//!
//! # Decisions this increment made
//!
//! * **An error does not block dismissal; dirt does.** `UIF-MOD-003` says
//!   the modal must *remain open* when an action fails, which is a
//!   statement about the failure not closing the dialog — not about the
//!   user being unable to close it afterwards. The banner therefore
//!   persists across every interaction that leaves the modal open, and is
//!   simply gone with the modal when the modal legitimately closes.
//! * **The header Close control is a dismiss path**, so `UIF-MOD-002`
//!   governs it exactly like `Escape` and the scrim. Only
//!   [`close`](ModalHost::close) — the programmatic path — bypasses the
//!   guard, because there the caller has already decided.
//! * **A modal with nothing focusable clears focus** rather than leaving the
//!   background lit. `push_trap` stops focus from *going* out but does not
//!   move what is already out; leaving a background control showing a ring
//!   under a scrim would be the leak `UIF-MOD-001` forbids, wearing a
//!   different hat.
//! * **A click on an enabled control reports `FocusMoved`, not `Absorbed`.**
//!   Both mean "the dialog stayed open"; the difference is whether the
//!   keyboard moved, and a host wiring sounds to the card needs to hear it.
//! * **`Enter` is trapped but not acted on.** Band 3 swallows it so it
//!   cannot reach the workspace, but which control fires is that control's
//!   business, so the host reports
//!   [`ModalNoOp::KeyOwnedByControl`] rather than inventing a default
//!   action.
//!
//! Unit tests live in the sibling `modal_tests.rs` sidecar.

use crate::binding::ActionPending;
use crate::binding::Activation;
use crate::binding::CancelRequest;
use crate::binding::Controlled;
use crate::binding::Ephemeral;
use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::focus::GroupId;
use crate::surface::modal_tier::ModalAction;
use crate::surface::zorder::DeclineReason;
use crate::surface::zorder::EscapeOutcome;
use crate::surface::zorder::KeyClass;
use crate::surface::zorder::KeyDisposition;
use crate::surface::zorder::SurfaceId;
use crate::surface::zorder::SurfaceKind;
use crate::surface::zorder::SurfaceRegistry;
use crate::surface::zorder::UnwindAnswer;
use crate::surface::zorder::ZBand;
use crate::surface::zorder::scrim_for;
use crate::token::color::Palette;
use crate::token::shape;
use crate::token::size;
use crate::token::space;

use iced::Color;
use iced::Point;
use iced::Rectangle;
use iced::Size;

// ---------------------------------------------------------------------------
// Sizing constants (§4.6.2)
// ---------------------------------------------------------------------------

/// The gutter a modal card never crosses (§4.6.2).
///
/// Every width row in the table is qualified "clamped to
/// `viewport.width - 32px`". Thirty-two is `space::XXL` in total, which is
/// `space::LG` on each side once the card is centred — the same gutter,
/// read from the two ends of the same token.
pub const VIEWPORT_GUTTER: f32 = space::XXL;

/// §4.6.2 — the Inline Document Editor takes 80% of the viewport on both
/// axes.
pub const EDITOR_VIEWPORT_FRACTION: f32 = 0.8;

/// §4.6.2 — the Inline Document Editor's width floor.
pub const EDITOR_WIDTH_MIN: f32 = 520.0;

/// §4.6.2 — the Inline Document Editor's width ceiling.
pub const EDITOR_WIDTH_MAX: f32 = 960.0;

/// §4.6.2 — the Inline Document Editor's height floor.
pub const EDITOR_HEIGHT_MIN: f32 = 380.0;

/// §4.6.2 — the Inline Document Editor's height ceiling.
pub const EDITOR_HEIGHT_MAX: f32 = 760.0;

/// §4.6.2 — Tools & Integrations is 720px wide, clamped to the gutter.
pub const TOOLS_WIDTH: f32 = 720.0;

/// §4.6.2 — Tools & Integrations is at least 320px tall.
pub const TOOLS_HEIGHT_MIN: f32 = 320.0;

/// §4.6.2 — Tools & Integrations never exceeds 80% of the viewport height.
pub const TOOLS_HEIGHT_MAX_FRACTION: f32 = 0.8;

/// §4.6.2 — Batch Processing is 560px wide, clamped to the gutter.
pub const BATCH_WIDTH: f32 = 560.0;

/// §4.6.2 — the tallest a content-fit modal body is allowed to get before
/// it has to scroll.
pub const CONTENT_HEIGHT_MAX: f32 = 480.0;

/// §4.6.2 — the About card is 440px wide.
pub const ABOUT_WIDTH: f32 = 440.0;

/// §4.6.2 — the Attributions card is 640px wide.
pub const ATTRIBUTIONS_WIDTH: f32 = 640.0;

/// §4.6.2 — the Attributions card is a 480px scrollable region.
pub const ATTRIBUTIONS_HEIGHT: f32 = 480.0;

/// §4.6.2 / `UIF-MOD-004` — the tallest the Attributions credits list
/// is allowed to get, whatever the card's solved remainder leaves it.
///
/// The spec names this in the closed §2.2 surface-geometry table, so the
/// value is registered in
/// [`SURFACE_GEOMETRY_CONSTANTS`](crate::token_lint::SURFACE_GEOMETRY_CONSTANTS)
/// rather than derived from a rung: 200 is a stated bound, not a multiple
/// of anything.
///
/// **Named `_HEIGHT`, not `_H`.** The spec writes `ATTRIBUTIONS_LIST_MAX_H`.
/// The lint only inspects a constant whose name reads as a visual
/// measurement, and its marker list
/// ([`VISUAL_NAME_MARKERS`](crate::token_lint::VISUAL_NAME_MARKERS))
/// matches `HEIGHT`, not a bare `H` — a constant named
/// `ATTRIBUTIONS_LIST_MAX_H` would be invisible to the gate that exists to
/// check it. The sibling §2.2 entry took the same decision: the spec's
/// `FOLDER_LIST_MAX_H` is `FOLDER_LIST_MAX_HEIGHT` in
/// [`input_dialog`](crate::surface::input_dialog). Same number, same
/// reason.
pub const ATTRIBUTIONS_LIST_MAX_HEIGHT: f32 = 200.0;

// ---------------------------------------------------------------------------
// Sizing policy
// ---------------------------------------------------------------------------

/// What one axis of the §4.6.2 policy *wants* to be, before any clamp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Desired {
    /// A fixed number of logical pixels.
    Fixed(f32),
    /// A fraction of the viewport's own extent on this axis.
    Viewport(f32),
    /// The measured extent of the card's content.
    Content,
}

/// The ceiling one axis may reach (§4.6.2 "max: 80% viewport" and
/// "max: 480px" are the two shapes the table actually uses).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cap {
    /// A fixed number of logical pixels.
    Pixels(f32),
    /// A fraction of the viewport's own extent on this axis.
    Viewport(f32),
    /// No ceiling of its own — only [`VIEWPORT_GUTTER`] applies.
    None,
}

impl Cap {
    /// Resolve the cap against the viewport extent for this axis.
    #[must_use]
    pub fn resolve(self, available: f32) -> f32 {
        match self {
            Self::Pixels(value) => finite(value),
            Self::Viewport(fraction) => finite(available) * finite(fraction),
            Self::None => f32::INFINITY,
        }
    }
}

/// One axis of the §4.6.2 sizing policy: what it wants, the floor it never
/// goes below, and the ceiling it never goes above.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisPolicy {
    /// What the axis wants to be before clamping.
    pub desired: Desired,
    /// The floor.
    pub min: f32,
    /// The ceiling.
    pub max: Cap,
}

impl AxisPolicy {
    /// A fixed extent, clamped only by the viewport gutter.
    #[must_use]
    pub const fn fixed(value: f32) -> Self {
        Self {
            desired: Desired::Fixed(value),
            min: 0.0,
            max: Cap::None,
        }
    }

    /// A fraction of the viewport, bounded by `min` and `max`.
    #[must_use]
    pub const fn viewport(fraction: f32, min: f32, max: f32) -> Self {
        Self {
            desired: Desired::Viewport(fraction),
            min,
            max: Cap::Pixels(max),
        }
    }

    /// Content-fit between a floor and a ceiling.
    #[must_use]
    pub const fn content(min: f32, max: Cap) -> Self {
        Self {
            desired: Desired::Content,
            min,
            max,
        }
    }

    /// Resolve this axis against the viewport extent and the measured
    /// content extent.
    ///
    /// The order is the spec's: want it, floor it, cap it, then apply the
    /// gutter every row shares. The gutter wins over the floor when the two
    /// disagree — see the [module docs](self#sizing-462) for why.
    #[must_use]
    pub fn resolve(&self, available: f32, content: f32) -> f32 {
        let available = finite(available);
        let room = (available - VIEWPORT_GUTTER).max(0.0);

        let wanted = match self.desired {
            Desired::Fixed(value) => finite(value),
            Desired::Viewport(fraction) => available * finite(fraction),
            Desired::Content => finite(content),
        };

        bound(wanted, self.min, self.max.resolve(available))
            .min(room)
            .max(0.0)
    }
}

/// The §4.6.2 width/height policy for one modal.
///
/// The five rows of the spec table are available as named constructors —
/// [`editor`](Self::editor), [`tools_and_integrations`](Self::tools_and_integrations),
/// [`batch_processing`](Self::batch_processing), [`about`](Self::about)
/// and [`attributions`](Self::attributions) — so a host states *which
/// modal it is* rather than restating five numbers that belong to the spec.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModalSizing {
    /// The width policy.
    pub width: AxisPolicy,
    /// The height policy.
    pub height: AxisPolicy,
}

impl ModalSizing {
    /// An explicit policy for both axes.
    #[must_use]
    pub const fn new(width: AxisPolicy, height: AxisPolicy) -> Self {
        Self { width, height }
    }

    /// §4.6.2 — Inline Document Editor: 80% of the viewport on both axes,
    /// `520..=960` wide and `380..=760` tall.
    #[must_use]
    pub const fn editor() -> Self {
        Self::new(
            AxisPolicy::viewport(EDITOR_VIEWPORT_FRACTION, EDITOR_WIDTH_MIN, EDITOR_WIDTH_MAX),
            AxisPolicy::viewport(
                EDITOR_VIEWPORT_FRACTION,
                EDITOR_HEIGHT_MIN,
                EDITOR_HEIGHT_MAX,
            ),
        )
    }

    /// §4.6.2 — Tools & Integrations: 720px wide clamped to the gutter,
    /// content-fit at least 320px tall and never more than 80% of the
    /// viewport.
    #[must_use]
    pub const fn tools_and_integrations() -> Self {
        Self::new(
            AxisPolicy::fixed(TOOLS_WIDTH),
            AxisPolicy::content(TOOLS_HEIGHT_MIN, Cap::Viewport(TOOLS_HEIGHT_MAX_FRACTION)),
        )
    }

    /// §4.6.2 — Batch Processing: 560px wide clamped to the gutter,
    /// content-fit up to 480px.
    #[must_use]
    pub const fn batch_processing() -> Self {
        Self::new(
            AxisPolicy::fixed(BATCH_WIDTH),
            AxisPolicy::content(0.0, Cap::Pixels(CONTENT_HEIGHT_MAX)),
        )
    }

    /// §4.6.2 — About: 440px wide, content-fit.
    #[must_use]
    pub const fn about() -> Self {
        Self::new(
            AxisPolicy::fixed(ABOUT_WIDTH),
            AxisPolicy::content(0.0, Cap::None),
        )
    }

    /// §4.6.2 — Attributions: 640px wide, a 480px scrollable region.
    #[must_use]
    pub const fn attributions() -> Self {
        Self::new(
            AxisPolicy::fixed(ATTRIBUTIONS_WIDTH),
            AxisPolicy::fixed(ATTRIBUTIONS_HEIGHT),
        )
    }

    /// Solve the card rectangle for a viewport and a measured content size.
    ///
    /// The card is centred in the viewport. Every input is treated as
    /// possibly non-finite: a `NaN` viewport produces a zero-sized card at
    /// the viewport origin rather than a `NaN` rectangle that poisons every
    /// hit test downstream.
    #[must_use]
    pub fn solve(&self, viewport: Rectangle, content: Size) -> Rectangle {
        let width = finite(viewport.width);
        let height = finite(viewport.height);

        let card_width = self.width.resolve(width, finite(content.width));
        let card_height = self.height.resolve(height, finite(content.height));

        Rectangle::new(
            Point::new(
                finite(viewport.x) + (width - card_width) / 2.0,
                finite(viewport.y) + (height - card_height) / 2.0,
            ),
            Size::new(card_width, card_height),
        )
    }
}

// ---------------------------------------------------------------------------
// User resize (§4.6.2, owner ruling D-3)
// ---------------------------------------------------------------------------

/// The floor a **user-dragged** modal card never goes below, per axis.
///
/// §4.6.2 sizes a modal from the viewport and the content and has no notion
/// of a card the reader makes smaller. Defect #6's owner ruling (D-3) added
/// one — the initial size stays whatever [`ModalSizing`] solves and a drag
/// overrides it — so the drag needs its own floor pair, and the clamp it is
/// applied with has to stay the framework's rather than a second copy in
/// whichever card installed the grip.
///
/// Deliberately **floors only**. The ceiling a dragged card cannot cross is
/// [`VIEWPORT_GUTTER`], which [`AxisPolicy::resolve`] applies last and
/// always wins; a `max` field here would be a second authority on the same
/// window edge, which is the mistake [`AxisPolicy`] exists to prevent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeFloors {
    /// Smallest width a drag may leave the card.
    pub width: f32,
    /// Smallest height a drag may leave the card.
    pub height: f32,
}

impl ResizeFloors {
    /// Both floors stated directly.
    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Resolve a size the user dragged against the viewport.
    ///
    /// The order is §4.6.2's own, reused rather than restated: each axis
    /// goes through [`AxisPolicy::resolve`] with the dragged extent as its
    /// `Desired::Fixed` want, this policy's floor for that axis, and
    /// [`Cap::None`] — so the shared [`VIEWPORT_GUTTER`] clamp is applied
    /// last and wins exactly as it is for the initial policy. A floor
    /// therefore loses to a small window for the same reason and by the same
    /// code path as in [`ModalSizing::solve`].
    ///
    /// Non-finite input collapses the way [`ModalSizing::solve`] collapses a
    /// `NaN` viewport: a `NaN` extent reads as a zero want, which the floor
    /// lifts back to a legal card instead of poisoning every hit test
    /// downstream.
    #[must_use]
    pub fn resolve(self, wanted: Size, viewport: Size) -> Size {
        let axis = |wanted: f32, floor: f32, available: f32| {
            AxisPolicy {
                desired: Desired::Fixed(finite(wanted)),
                min: floor,
                max: Cap::None,
            }
            .resolve(finite(available), 0.0)
        };

        Size::new(
            axis(wanted.width, self.width, viewport.width),
            axis(wanted.height, self.height, viewport.height),
        )
    }
}

// ---------------------------------------------------------------------------
// Scrollable sub-regions (§4.6.2 / UIF-MOD-004)
// ---------------------------------------------------------------------------

/// The vertical furniture inside a modal card that a scrollable sub-region
/// is **not** allowed to use (`UIF-MOD-004`).
///
/// # Why a struct rather than three arguments or a frame derivation
///
/// * **Three bare `f32`s are silently interchangeable.**
///   `subregion_max_height(h, 69.0, 16.0, 49.0)` compiles the same way
///   whatever order the caller meant, and a swapped pair produces a cap
///   that is wrong by a plausible amount rather than a type error. A
///   struct names each term where it is built.
/// * **[`ModalFrame`] cannot supply these, by its own design.** The frame
///   answers *what the card is made of* and deliberately carries no
///   header/body/footer sub-rectangles, because those need the title's
///   measured height and the content's measured extent. Header and footer
///   band heights are the same content-driven quantity, so deriving them
///   from the frame would put back exactly the layout knowledge the frame
///   refuses to hold.
/// * **The frame *does* own one of the three terms.** `content_padding` is
///   the body padding, so [`from_frame`](Self::from_frame) seeds it and a
///   caller adds only the two terms that depend on its own card. The
///   token-sourced part and the content-sourced part stay visibly separate
///   at the call site instead of blurring into one number.
/// * **The doubling lives here once.** §4.6.2 charges the body padding
///   twice; [`total`](Self::total) is the only place that says `2 ×`, so
///   no call site restates it ([RUST-021]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubRegionBands {
    /// Everything stacked above the sub-region — its dividers and the gaps
    /// it sits on included.
    pub header: f32,
    /// Everything stacked below the sub-region — its dividers and the gaps
    /// it sits on included.
    pub footer: f32,
    /// The card's body padding on **one** side. [`total`](Self::total)
    /// charges it twice.
    pub body_padding: f32,
}

impl SubRegionBands {
    /// Every term stated directly.
    #[must_use]
    pub const fn new(header: f32, footer: f32, body_padding: f32) -> Self {
        Self {
            header,
            footer,
            body_padding,
        }
    }

    /// The frame's own contribution: its body padding, with the two
    /// content-driven bands still to be added.
    ///
    /// A card with no header and no footer is a real case — a bare
    /// full-bleed list — so starting from the frame rather than requiring
    /// all three terms is not a placeholder for missing data.
    #[must_use]
    pub const fn from_frame(frame: &ModalFrame) -> Self {
        Self::new(0.0, 0.0, frame.content_padding)
    }

    /// Set the header band.
    #[must_use]
    pub const fn with_header(self, header: f32) -> Self {
        Self { header, ..self }
    }

    /// Set the footer band.
    #[must_use]
    pub const fn with_footer(self, footer: f32) -> Self {
        Self { footer, ..self }
    }

    /// Everything the sub-region does not get: `header + 2 × body_padding + footer`.
    #[must_use]
    pub const fn total(&self) -> f32 {
        self.header + 2.0 * self.body_padding + self.footer
    }
}

/// The tallest a scrollable sub-region may be inside a card of
/// `card_height` (`UIF-MOD-004`).
///
/// `subregion_max_h = modal_height - (header_band + 2 × body_padding + footer_band)`,
/// exactly as §4.6.2 states it. Pure: no clock, no globals, no palette
/// lookup — the card's solved height and the band terms go in and one
/// number comes out.
///
/// **Clamped at zero, and that clamp is load-bearing.** A card shorter than
/// its own furniture is reachable: the Attributions card is `480px` fixed
/// but clamps to `viewport - 32`, so a viewport under `182px` tall solves
/// to a card smaller than the `150px` of header, padding and footer it
/// carries. Without the clamp the solver would hand back a negative height,
/// which iced does not reject — it lays the widget out at a negative
/// extent and the result is a scrollable whose viewport is inverted. The
/// clamp turns that into a zero-height list, which is the honest answer:
/// there is no room.
///
/// **Non-finite input collapses to zero, the same way [`ModalSizing::solve`]
/// collapses a `NaN` viewport.** An infinite card height would otherwise
/// pass straight through the subtraction and out the other side as an
/// infinite list, which is worse than the `NaN` case because it looks
/// like it worked.
///
/// **This is the mechanism the requirement exists to install.** A
/// sub-region must be solved from the card's *solved* bounds, never from
/// the card's outer constant: sizing the credits list at
/// [`ATTRIBUTIONS_HEIGHT`] directly would spend the 480px the header and
/// footer already spent.
#[must_use]
pub fn subregion_max_height(card_height: f32, bands: SubRegionBands) -> f32 {
    (finite(card_height) - finite(bands.total())).max(0.0)
}

/// The sub-region's height: what the card leaves it, held down by `cap`
/// (`UIF-MOD-004`'s "stable cap" clause).
///
/// The cap is applied *after* the remainder is solved, not instead of it,
/// so a card too short to reach the cap still gets everything it has.
/// `min` of the two is what makes the cap a ceiling rather than a target.
#[must_use]
pub fn subregion_height(card_height: f32, bands: SubRegionBands, cap: f32) -> f32 {
    subregion_max_height(card_height, bands).min(cap)
}

// ---------------------------------------------------------------------------
// The frame (§4.6.1 anatomy, resolved)
// ---------------------------------------------------------------------------

/// The resolved visual values of the §4.6.1 modal frame.
///
/// The anatomy in the spec is a list of surfaces, strokes and paddings, and
/// every one of them is a token. Resolving them here once means the view
/// layer that eventually paints the card cannot re-derive a colour or
/// retype a padding — `UIF-VER-001`'s whole point.
///
/// Deliberately absent: sub-rectangles for the header, body, banner and
/// footer. Those are a layout, and a layout needs the title's measured
/// height and the content's measured extent. This type answers *what the
/// frame is made of*; where the pieces go is the renderer's job.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModalFrame {
    /// The card's own surface — `color.surface.overlay`.
    pub surface: Color,
    /// The card perimeter — `color.border.strong`, the stroke that says
    /// "this edge is a floating thing".
    pub border: Color,
    /// The perimeter weight — `shape::BORDER_HAIRLINE`.
    pub border_width: f32,
    /// The card radius — `shape::RADIUS_CARD`.
    pub radius: f32,
    /// The full-viewport scrim, composed from the token by [`scrim_for`].
    pub scrim: Color,
    /// The divider between header, body and footer — `color.border.subtle`.
    pub divider: Color,
    /// Header and body padding — `space::LG` (16px).
    pub content_padding: f32,
    /// The footer's horizontal padding — `space::LG` (16px).
    pub footer_padding_horizontal: f32,
    /// The footer's vertical padding — `space::MD` (12px).
    pub footer_padding_vertical: f32,
    /// The Close control's hit target — `size::HIT_TARGET_MIN` (24px).
    pub close_target: f32,
}

impl ModalFrame {
    /// Resolve the frame from a palette.
    #[must_use]
    pub const fn resolve(palette: &Palette) -> Self {
        Self {
            surface: palette.surface_overlay,
            border: palette.border_strong,
            border_width: shape::BORDER_HAIRLINE,
            radius: shape::RADIUS_CARD,
            scrim: scrim_for(palette),
            divider: palette.border_subtle,
            content_padding: space::LG,
            footer_padding_horizontal: space::LG,
            footer_padding_vertical: space::MD,
            close_target: size::HIT_TARGET_MIN,
        }
    }
}

// ---------------------------------------------------------------------------
// The error banner (UIF-MOD-003)
// ---------------------------------------------------------------------------

/// Where inside the card a failure happened (§4.6.3's "where (e.g.,
/// line/column for Markdown parse errors)").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorLocation {
    /// A position in the text the user is editing.
    Position {
        /// The 1-based line.
        line: u32,
        /// The 1-based column.
        column: u32,
    },
    /// A named place in the card — a field, a table row, a section.
    Region(String),
}

/// The recovery affordance the banner offers (§4.6.3's "providing a retry
/// or recovery action").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recovery {
    /// Re-run the action that failed.
    Retry,
    /// Move focus to the control the user must correct before trying again.
    FocusControl(FocusId),
    /// Acknowledge the failure and clear the banner.
    Acknowledge,
}

/// A structured failure the modal surfaces (`UIF-MOD-003`).
///
/// Structured rather than a string because the three things the invariant
/// names — what failed, where, and what the user can do about it — are
/// read by different parts of the frame, and because a banner that is one
/// opaque string cannot be tested for having said anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModalError {
    code: String,
    message: String,
    location: Option<ErrorLocation>,
    recovery: Recovery,
}

impl ModalError {
    /// A failure with a machine-readable code and a human message.
    ///
    /// The code is the `NFR-004` grouping handle; the message is what the
    /// banner shows.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            location: None,
            recovery: Recovery::Acknowledge,
        }
    }

    /// Say where it happened.
    #[must_use]
    pub fn at(mut self, location: ErrorLocation) -> Self {
        self.location = Some(location);
        self
    }

    /// Say what the user can do about it.
    #[must_use]
    pub fn with_recovery(mut self, recovery: Recovery) -> Self {
        self.recovery = recovery;
        self
    }

    /// The machine-readable code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// The human-readable message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Where it happened, when the failure knows.
    #[must_use]
    pub fn location(&self) -> Option<&ErrorLocation> {
        self.location.as_ref()
    }

    /// The affordance the banner offers.
    #[must_use]
    pub const fn recovery(&self) -> &Recovery {
        &self.recovery
    }
}

// ---------------------------------------------------------------------------
// Outcomes
// ---------------------------------------------------------------------------

/// Why a dismiss happened, or was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DismissCause {
    /// The `Escape` key.
    Escape,
    /// A click on the scrim, outside the card.
    Scrim,
    /// The header's Close control.
    CloseButton,
    /// [`ModalHost::close`] — the caller decided, so `UIF-MOD-002` does
    /// not apply.
    Programmatic,
}

/// What a pointer press hit inside the card.
///
/// The caller reports this because the host owns the card's *boundary* and
/// not its contents. The report is checked, not trusted: a `Control` that
/// the focus model will not accept is downgraded to
/// [`CardTarget::Disabled`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CardTarget {
    /// Empty card padding, or any non-interactive part of the card.
    Padding,
    /// A control the card shows but will not act on.
    Disabled,
    /// The header's Close control — a dismiss path, so `UIF-MOD-002`
    /// governs it.
    CloseButton,
    /// The error banner's recovery affordance.
    ErrorRecovery,
    /// An enabled control inside the card.
    Control(FocusId),
}

/// Why the host did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalNoOp {
    /// The modal is not open.
    NotOpen,
    /// The key class is not one Band 3 traps (§5 "Keyboard Routing"), so
    /// it belongs to the bands below, not to the modal.
    KeyNotTrapped,
    /// Band 3 traps the key, but the control holding focus owns the action;
    /// the host has no move to make.
    KeyOwnedByControl,
    /// The card has no focusable control to move to.
    NothingFocusable,
    /// A surface above the modal in the escape order consumed the key.
    KeyConsumedElsewhere,
    /// The modal is open but its surface is not registered, so the escape
    /// chain never reached it.
    NotRegistered,
}

/// What one interaction with the modal did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModalOutcome {
    /// The modal closed.
    Dismissed {
        /// What closed it.
        cause: DismissCause,
    },
    /// A dismiss was attempted and refused because the modal is dirty
    /// (`UIF-MOD-002`). Nothing else changed.
    DismissRefused {
        /// What tried to close it.
        cause: DismissCause,
    },
    /// The event landed inside the card and the card boundary took it. It
    /// never reached the scrim and the dialog stayed open
    /// (`UIF-VER-003`).
    Absorbed {
        /// What inside the card absorbed it.
        target: CardTarget,
    },
    /// Keyboard focus moved to another control inside the card
    /// (`UIF-MOD-001`).
    FocusMoved {
        /// The control that now holds focus.
        focused: FocusId,
    },
    /// Nothing happened.
    NoOp {
        /// Why.
        reason: ModalNoOp,
    },
}

impl ModalOutcome {
    /// Whether the modal closed.
    #[must_use]
    pub const fn is_dismissed(&self) -> bool {
        matches!(self, Self::Dismissed { .. })
    }

    /// Whether a dismiss was refused by the dirty guard.
    #[must_use]
    pub const fn is_refused(&self) -> bool {
        matches!(self, Self::DismissRefused { .. })
    }

    /// Whether the card boundary absorbed the event.
    #[must_use]
    pub const fn is_absorbed(&self) -> bool {
        matches!(self, Self::Absorbed { .. })
    }

    /// Whether the modal stayed open — absorbed, refused, focus moved, or
    /// nothing at all. The complement of [`is_dismissed`](Self::is_dismissed),
    /// and the assertion `UIF-VER-003` actually wants.
    #[must_use]
    pub const fn stays_open(&self) -> bool {
        !self.is_dismissed()
    }
}

/// Where a pointer press landed relative to the card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalHit {
    /// On the card.
    Card,
    /// On the scrim, outside the card.
    Scrim,
}

// ---------------------------------------------------------------------------
// The spec
// ---------------------------------------------------------------------------

/// Everything the caller supplies to open a modal.
///
/// "Content", from the host's point of view, is the card's declared
/// focusable roster: the controls the trap confines and the tab order
/// cycles. The iced `Element` that fills the body belongs to the view layer
/// and never enters this model — which is exactly why the invariants hold
/// without one.
#[derive(Clone, Debug, PartialEq)]
pub struct ModalSpec {
    id: SurfaceId,
    title: String,
    sizing: ModalSizing,
    dirty: bool,
    error: Option<ModalError>,
    members: Vec<FocusMember>,
    primary: Option<FocusId>,
}

impl ModalSpec {
    /// A spec for a modal of a given identity, title and size.
    pub fn new(id: impl Into<SurfaceId>, title: impl Into<String>, sizing: ModalSizing) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            sizing,
            dirty: false,
            error: None,
            members: Vec::new(),
            primary: None,
        }
    }

    /// Declare the card dirty — the caller has unsaved edits
    /// (`UIF-MOD-002`).
    #[must_use]
    pub fn with_dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    /// Open with an error already showing (`UIF-MOD-003`).
    #[must_use]
    pub fn with_error(mut self, error: ModalError) -> Self {
        self.error = Some(error);
        self
    }

    /// Declare the card's focusable roster, in tab order.
    #[must_use]
    pub fn with_controls(mut self, members: Vec<FocusMember>) -> Self {
        self.members = members;
        self
    }

    /// Name the control focus lands on when the modal opens
    /// (`UIF-MOD-001`'s "primary interactive control").
    #[must_use]
    pub fn with_primary(mut self, primary: impl Into<FocusId>) -> Self {
        self.primary = Some(primary.into());
        self
    }

    /// The modal's identity.
    #[must_use]
    pub fn id(&self) -> &SurfaceId {
        &self.id
    }

    /// The card title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The sizing policy.
    #[must_use]
    pub const fn sizing(&self) -> ModalSizing {
        self.sizing
    }

    /// Whether the card starts dirty.
    #[must_use]
    pub const fn dirty(&self) -> bool {
        self.dirty
    }

    /// The error the card starts with.
    #[must_use]
    pub fn error(&self) -> Option<&ModalError> {
        self.error.as_ref()
    }

    /// The declared focusable roster.
    #[must_use]
    pub fn controls(&self) -> &[FocusMember] {
        &self.members
    }

    /// The primary control.
    #[must_use]
    pub fn primary(&self) -> Option<&FocusId> {
        self.primary.as_ref()
    }
}

// ---------------------------------------------------------------------------
// The host
// ---------------------------------------------------------------------------

/// The focus group a modal of this id owns.
///
/// Derived rather than supplied so the host and the caller cannot disagree
/// about which group the trap confines, and so two modals never end up
/// sharing one by a copy-paste of a literal.
#[must_use]
pub fn group_for(id: &SurfaceId) -> GroupId {
    GroupId::new(format!("modal:{}", id.as_str()))
}

/// The modal host — the frame that makes `UIF-MOD-001/002/003` hold.
///
/// The host owns four things and nothing else: the card rectangle, the
/// dirty flag, the error slot, and the fact that it is open. The focus
/// group and the z-order surface are registered *into* collaborators the
/// caller passes in, because both are shared with the rest of the window.
///
/// Every state transition is reported as a [`ModalOutcome`], and every
/// outcome is a fact about what changed rather than a hint about what the
/// caller should do next.
#[derive(Clone, Debug, PartialEq)]
pub struct ModalHost {
    id: SurfaceId,
    group: GroupId,
    title: String,
    sizing: ModalSizing,
    viewport: Rectangle,
    card: Rectangle,
    primary: Option<FocusId>,
    dirty: bool,
    error: Option<ModalError>,
    retry_pending: bool,
    /// The `Ephemeral` half: what the frame knows about the action the user
    /// has under way inside the card (`UIF-ATOM-000b`).
    action: ModalAction,
    open: bool,
}

impl ModalHost {
    /// Open a modal.
    ///
    /// In order: solve the card, register the card's focus group, push the
    /// trap, move focus to the primary control, and register the Band 3
    /// surface over the viewport.
    ///
    /// The trap goes on **before** focus is moved so the primary control is
    /// focused *through* it — a primary that is not inside the card is
    /// rejected by the trap rather than silently stealing focus from behind
    /// it. If nothing inside the card can hold focus, focus is cleared
    /// rather than left on whatever the background had; a lit ring under
    /// the scrim is the leak `UIF-MOD-001` exists to prevent.
    ///
    /// `content` is the card's measured content extent, used only by
    /// content-fit axes; pass `Size::new(0.0, 0.0)` before the first
    /// layout and [`resize`](Self::resize) afterwards.
    #[must_use]
    pub fn open(
        spec: ModalSpec,
        viewport: Rectangle,
        content: Size,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> Self {
        let card = spec.sizing.solve(viewport, content);
        let group = group_for(&spec.id);

        focus.register_group(group.clone(), spec.members.clone());
        focus.push_trap(group.clone());

        let mut landed = None;
        if let Some(primary) = spec.primary.as_ref()
            && focus.focus(primary)
        {
            landed = Some(primary.clone());
        }
        if landed.is_none() {
            landed = focus.focus_first_in_group(&group);
        }
        if landed.is_none() {
            focus.clear();
        }

        registry.register(spec.id.clone(), SurfaceKind::Modal, vec![viewport]);

        Self {
            id: spec.id,
            group,
            title: spec.title,
            sizing: spec.sizing,
            viewport,
            card,
            primary: spec.primary,
            dirty: spec.dirty,
            error: spec.error,
            retry_pending: false,
            action: ModalAction::new(),
            open: true,
        }
    }

    // -- identity ----------------------------------------------------------

    /// The modal's surface identity.
    #[must_use]
    pub fn id(&self) -> &SurfaceId {
        &self.id
    }

    /// The focus group this modal's controls live in.
    #[must_use]
    pub fn group_id(&self) -> &GroupId {
        &self.group
    }

    /// The card title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The sizing policy in force.
    #[must_use]
    pub const fn sizing(&self) -> ModalSizing {
        self.sizing
    }

    /// Whether the modal is still open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    // -- geometry ----------------------------------------------------------

    /// The card rectangle, in root coordinates.
    #[must_use]
    pub const fn card(&self) -> Rectangle {
        self.card
    }

    /// The viewport the modal was last solved against.
    #[must_use]
    pub const fn viewport(&self) -> Rectangle {
        self.viewport
    }

    /// The scrim's bounds — the viewport, because the scrim is full-window
    /// (§4.6.1).
    #[must_use]
    pub const fn scrim_bounds(&self) -> Rectangle {
        self.viewport
    }

    /// Whether `at` is inside the card.
    ///
    /// Uses iced's convention, which the whole crate shares: the right and
    /// bottom edges are *outside*.
    #[must_use]
    pub fn is_inside_card(&self, at: Point) -> bool {
        self.card.contains(at)
    }

    /// Classify a point as card or scrim.
    #[must_use]
    pub fn hit_test(&self, at: Point) -> ModalHit {
        if self.is_inside_card(at) {
            ModalHit::Card
        } else {
            ModalHit::Scrim
        }
    }

    /// Re-solve the card against a new viewport and content size.
    ///
    /// Refreshes the z-order registration with the new viewport rectangle.
    /// Re-registration keeps the stacking position, so a modal that
    /// re-solves every frame does not restack.
    pub fn resize(&mut self, viewport: Rectangle, content: Size, registry: &mut SurfaceRegistry) {
        self.viewport = viewport;
        self.card = self.sizing.solve(viewport, content);
        if self.open {
            registry.register(self.id.clone(), SurfaceKind::Modal, vec![viewport]);
        }
    }

    // -- dirty state (UIF-MOD-002) -----------------------------------------

    /// Whether the card holds unsaved edits.
    ///
    /// Supplied by the caller. The host never infers dirt from an
    /// interaction: it has no idea what the card contains, and guessing
    /// would mean guessing wrong in the direction that throws work away.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Set the dirty flag; returns whether it changed.
    pub fn set_dirty(&mut self, dirty: bool) -> bool {
        let changed = self.dirty != dirty;
        self.dirty = dirty;
        changed
    }

    // -- error banner (UIF-MOD-003) ----------------------------------------

    /// The error the banner is showing.
    #[must_use]
    pub fn error(&self) -> Option<&ModalError> {
        self.error.as_ref()
    }

    /// Whether the banner is showing anything.
    #[must_use]
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }

    /// Show an error, superseding whatever was showing.
    ///
    /// Returns what it replaced. This is one of only two ways the banner
    /// changes: the other is [`clear_error`](Self::clear_error) and the
    /// banner's own recovery affordance.
    pub fn set_error(&mut self, error: ModalError) -> Option<ModalError> {
        self.retry_pending = false;
        self.error.replace(error)
    }

    /// Clear the banner; returns what it removed.
    pub fn clear_error(&mut self) -> Option<ModalError> {
        self.retry_pending = false;
        self.error.take()
    }

    /// Whether a `Retry` recovery has been requested and not yet taken.
    #[must_use]
    pub const fn retry_pending(&self) -> bool {
        self.retry_pending
    }

    /// Take a pending retry, returning the error it was requested for.
    ///
    /// The host cannot re-run the caller's failed action, so the request is
    /// handed back rather than acted on. The banner stays visible until the
    /// caller clears or supersedes it — a retry that has not run yet has
    /// not fixed anything, and hiding the error early would be a lie.
    pub fn take_retry(&mut self) -> Option<ModalError> {
        if !self.retry_pending {
            return None;
        }
        self.retry_pending = false;
        self.error.clone()
    }

    // -- pending action (§3.0.4.1, §3.0.4.3) --------------------------------

    /// Record the host's current [`ActionPending`] projection for the card's
    /// primary call to action.
    ///
    /// Call once per frame, before any key or pointer handling. This is the
    /// only way busy-ness enters the host: the value arrives as data and the
    /// host never reads a clock, so the same host drives correctly under a
    /// test harness with no time at all.
    pub fn set_pending(&mut self, pending: ActionPending) {
        self.action.observe(pending);
    }

    /// The pending-action projection currently in force.
    #[must_use]
    pub fn pending(&self) -> ActionPending {
        self.action.pending()
    }

    /// Whether the card's primary command is still running.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.action.is_busy()
    }

    /// Ask whether the primary call to action may dispatch.
    ///
    /// The gate behind this is what makes §3.0.4.3's "suppress duplicate
    /// `on_activate`" real for a modal: a second press inside the
    /// round-trip window is withheld even before the host's projection has
    /// caught up with the first.
    pub fn try_activate_primary(&mut self) -> Activation {
        self.action.try_activate()
    }

    /// Ask the running action to stop — the `on_cancel_request()` path.
    ///
    /// `None` when nothing is running, or when the running action was never
    /// offered a stop path, so the card shows no dead stop affordance.
    pub fn request_cancel(&mut self) -> Option<CancelRequest> {
        self.action.request_cancel()
    }

    /// The `Controlled` half of the modal (§3.0.4.1).
    ///
    /// Whether the card is open, dirty, or carrying an error belongs to the
    /// application; the host holds a read-only view of its own frame state
    /// and hands that view out rather than a mutable one. There is
    /// deliberately no `get_mut` on [`Controlled`] — that absence *is* the
    /// guarantee that the widget layer cannot quietly disagree with the app.
    #[must_use]
    pub fn controlled(&self) -> Controlled<'_, Self> {
        Controlled::new(self)
    }

    /// The `Ephemeral` half of the modal (§3.0.4.1).
    ///
    /// What the frame knows about the action under way is framework-side
    /// interaction state: written and read in the same frame, never
    /// round-tripped through the application, and legitimately droppable
    /// when the card is rebuilt.
    #[must_use]
    pub fn ephemeral(&self) -> Ephemeral<&ModalAction> {
        Ephemeral::new(&self.action)
    }

    // -- content roster ----------------------------------------------------

    /// Replace the card's focusable roster, as a rebuild of the card's
    /// contents would.
    ///
    /// The trap is untouched — it names the group, not the members — so a
    /// rebuilt card cannot widen the scope. Returns the id that lost focus.
    pub fn sync_members(
        &mut self,
        members: Vec<FocusMember>,
        focus: &mut FocusState,
    ) -> Option<FocusId> {
        focus.register_group(self.group.clone(), members)
    }

    // -- keyboard ----------------------------------------------------------

    /// Drive one key class through the host.
    ///
    /// What the host handles is read from
    /// [`ZBand::key_disposition`] for [`ZBand::Modal`], not from a list of
    /// keys it likes:
    /// a class Band 3 does not trap is reported as
    /// [`ModalNoOp::KeyNotTrapped`] and left to the bands below, and a
    /// class the band traps but the focused control owns is reported as
    /// [`ModalNoOp::KeyOwnedByControl`].
    pub fn on_key(
        &mut self,
        class: KeyClass,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> ModalOutcome {
        if !self.open {
            return ModalOutcome::NoOp {
                reason: ModalNoOp::NotOpen,
            };
        }

        match ZBand::Modal.key_disposition(class) {
            KeyDisposition::PassThrough => ModalOutcome::NoOp {
                reason: ModalNoOp::KeyNotTrapped,
            },
            KeyDisposition::Swallow | KeyDisposition::SwallowWhenFocused => match class {
                KeyClass::Tab => self.step_focus(focus, true),
                KeyClass::ShiftTab => self.step_focus(focus, false),
                KeyClass::Escape => self.escape_outcome(registry, focus),
                KeyClass::Enter | KeyClass::Space | KeyClass::ArrowNavigation => {
                    ModalOutcome::NoOp {
                        reason: ModalNoOp::KeyOwnedByControl,
                    }
                }
            },
        }
    }

    /// `Tab` / `Shift+Tab` inside the trap.
    fn step_focus(&mut self, focus: &mut FocusState, forward: bool) -> ModalOutcome {
        match if forward {
            focus.focus_next()
        } else {
            focus.focus_previous()
        } {
            Some(focused) => ModalOutcome::FocusMoved { focused },
            None => ModalOutcome::NoOp {
                reason: ModalNoOp::NothingFocusable,
            },
        }
    }

    // -- pointer -----------------------------------------------------------

    /// Drive one pointer press.
    ///
    /// **The card boundary is decided first, from the host's own
    /// rectangle.** If the point is inside the card, control flow cannot
    /// reach the scrim at all: the scrim branch is the `else` of a test
    /// that has already been taken. That is the structural form of
    /// `UIF-VER-003`'s "clicking disabled buttons or card whitespace
    /// inside Input/Modal dialogs never dismisses the dialog" — it is not
    /// a case that is handled, it is a case that cannot happen.
    ///
    /// `target` is what the caller's own hit test found inside the card.
    /// It is **not consulted when the point is outside the card**, and it
    /// is checked rather than trusted: a `Control` the focus model will not
    /// accept is downgraded to a disabled one.
    pub fn on_pointer(
        &mut self,
        at: Point,
        target: CardTarget,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> ModalOutcome {
        if !self.open {
            return ModalOutcome::NoOp {
                reason: ModalNoOp::NotOpen,
            };
        }

        if !self.is_inside_card(at) {
            return self.attempt_dismiss(DismissCause::Scrim, focus, registry);
        }

        match target {
            CardTarget::Padding => ModalOutcome::Absorbed {
                target: CardTarget::Padding,
            },
            CardTarget::Disabled => ModalOutcome::Absorbed {
                target: CardTarget::Disabled,
            },
            CardTarget::ErrorRecovery => self.error_recovery(focus),
            CardTarget::CloseButton => {
                self.attempt_dismiss(DismissCause::CloseButton, focus, registry)
            }
            CardTarget::Control(id) => self.click_control(&id, focus),
        }
    }

    /// A press on a control the caller says is inside the card.
    fn click_control(&mut self, id: &FocusId, focus: &mut FocusState) -> ModalOutcome {
        if !self.owns_control(focus, id) || !focus.focus(id) {
            return ModalOutcome::Absorbed {
                target: CardTarget::Disabled,
            };
        }

        ModalOutcome::FocusMoved {
            focused: id.clone(),
        }
    }

    /// Whether `id` is one of this card's own controls.
    fn owns_control(&self, focus: &FocusState, id: &FocusId) -> bool {
        focus
            .group_of(id)
            .is_some_and(|group| group.id() == &self.group)
    }

    /// The banner's recovery affordance was pressed.
    ///
    /// The banner never dismisses the modal, so every branch reports the
    /// same absorption and differs only in what it changed.
    fn error_recovery(&mut self, focus: &mut FocusState) -> ModalOutcome {
        match self.error.as_ref().map(|error| error.recovery().clone()) {
            Some(Recovery::Retry) => self.retry_pending = true,
            Some(Recovery::FocusControl(target)) => {
                focus.focus(&target);
            }
            Some(Recovery::Acknowledge) => {
                self.error = None;
            }
            None => {}
        }

        ModalOutcome::Absorbed {
            target: CardTarget::ErrorRecovery,
        }
    }

    // -- dismissal ---------------------------------------------------------

    /// The only route from a user gesture to a closed modal.
    ///
    /// `Escape`, the scrim and the header Close control all arrive here,
    /// which is the point: there is exactly one place the dirty guard lives
    /// and exactly one thing a user gesture can do to close the dialog.
    fn attempt_dismiss(
        &mut self,
        cause: DismissCause,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> ModalOutcome {
        if self.dirty {
            return ModalOutcome::DismissRefused { cause };
        }

        self.teardown(focus);
        registry.unregister(&self.id);
        ModalOutcome::Dismissed { cause }
    }

    /// Close the modal at the caller's request.
    ///
    /// The programmatic path bypasses `UIF-MOD-002` because the guard
    /// exists to stop the *user* discarding work by accident, not to stop
    /// the application doing what it asked. Focus restoration is the same
    /// as any other close.
    pub fn close(
        &mut self,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> ModalOutcome {
        if !self.open {
            return ModalOutcome::NoOp {
                reason: ModalNoOp::NotOpen,
            };
        }

        self.teardown(focus);
        registry.unregister(&self.id);
        ModalOutcome::Dismissed {
            cause: DismissCause::Programmatic,
        }
    }

    /// Tear the modal down around its focus, without touching the registry.
    ///
    /// **Pop the trap, then unregister the group** — see the
    /// [module docs](self#tearing-down-in-the-right-order) for why the
    /// order is load-bearing.
    ///
    /// The registry is deliberately *not* part of this: the escape walk
    /// removes the surface itself, by index, and unregistering from inside
    /// the closure would shift the list under its feet.
    fn teardown(&mut self, focus: &mut FocusState) {
        focus.pop_trap();
        focus.unregister_group(&self.group);
        self.open = false;
        self.action.reset();
    }

    // -- z-order integration -----------------------------------------------

    /// The modal's own answer when the escape chain asks it to unwind.
    ///
    /// `Dismissed` when clean, `Declined(CannotUnwind)` when the dirty
    /// guard blocks it **or** when the card's primary command is still
    /// running, `Declined(NotOpen)` once closed.
    ///
    /// The two refusals are different in kind, which is why they are two
    /// checks rather than one merged flag: `UIF-MOD-002` protects the
    /// user's unsaved *edits*, while the pending-action refusal protects the
    /// application from being left holding a half-finished operation with
    /// no card left to report it in. A user who wants out of a running
    /// command has [`request_cancel`](Self::request_cancel), not `Escape`.
    ///
    /// Both refusals keep the existing escape contract: a declined key keeps
    /// travelling down the bands rather than being swallowed here.
    ///
    /// **Must not mutate the registry** — the caller is normally
    /// [`SurfaceRegistry::escape`], which indexes the surface list and
    /// removes the entry it was told closed.
    pub fn unwind_answer(&mut self, focus: &mut FocusState) -> UnwindAnswer {
        if !self.open {
            return UnwindAnswer::Declined(DeclineReason::NotOpen);
        }
        if self.dirty {
            return UnwindAnswer::Declined(DeclineReason::CannotUnwind);
        }
        if self.action.blocks_dismiss() {
            return UnwindAnswer::Declined(DeclineReason::CannotUnwind);
        }

        self.teardown(focus);
        UnwindAnswer::Dismissed
    }

    /// Run one `Escape` through the z-order chain with this modal as the
    /// only surface it answers for.
    ///
    /// Every other surface is answered `Declined(NotOpen)` — "not mine, the
    /// key keeps travelling" — which is what lets a menu open above the
    /// modal unwind first, exactly as `UIF-WIN-003` orders it, without the
    /// modal knowing menus exist.
    pub fn escape_via(
        &mut self,
        registry: &mut SurfaceRegistry,
        focus: &mut FocusState,
    ) -> EscapeOutcome {
        let id = self.id.clone();
        registry.escape(move |surface| {
            if surface.id() != &id {
                return UnwindAnswer::Declined(DeclineReason::NotOpen);
            }
            self.unwind_answer(focus)
        })
    }

    /// Map the chain's answer onto a host outcome.
    fn escape_outcome(
        &mut self,
        registry: &mut SurfaceRegistry,
        focus: &mut FocusState,
    ) -> ModalOutcome {
        let outcome = self.escape_via(registry, focus);

        if outcome.consumed_by() == Some(&self.id) {
            return ModalOutcome::Dismissed {
                cause: DismissCause::Escape,
            };
        }
        if outcome.is_consumed() {
            return ModalOutcome::NoOp {
                reason: ModalNoOp::KeyConsumedElsewhere,
            };
        }

        let refused = outcome.declined_before().iter().any(|declined| {
            declined.surface == self.id && declined.reason == DeclineReason::CannotUnwind
        });

        if refused {
            ModalOutcome::DismissRefused {
                cause: DismissCause::Escape,
            }
        } else {
            ModalOutcome::NoOp {
                reason: ModalNoOp::NotRegistered,
            }
        }
    }
}

/// A total `f32` for geometry that may arrive from a caller who has not
/// measured anything.
///
/// A `NaN` width is not a width, and letting one into a `Rectangle` poisons
/// every hit test downstream — `Rectangle::contains` is `false` for a `NaN`
/// coordinate, so a card that went `NaN` would absorb nothing and dismiss
/// on every click. Collapsing to `0.0` keeps the arithmetic finite and the
/// card simply becomes degenerate, which is visible and debuggable.
fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

/// Bound `value` to `[lo, hi]` without the two panics `f32::clamp` carries.
///
/// `clamp` panics on a non-finite input and on an inverted range. Both are
/// reachable here: a spec constant could be mistyped, and a `Cap::Viewport`
/// shrinks with the window so it can fall below a fixed `min`. An inverted
/// range is repaired by raising the floor to the ceiling's value, and a
/// non-finite value falls back to the floor rather than propagating.
fn bound(value: f32, lo: f32, hi: f32) -> f32 {
    let lo = lo.max(0.0);
    let hi = if hi.is_finite() {
        hi.max(lo)
    } else {
        f32::INFINITY
    };

    if !value.is_finite() {
        return lo;
    }

    value.max(lo).min(hi)
}

#[cfg(test)]
#[path = "modal_tests.rs"]
mod modal_tests;
