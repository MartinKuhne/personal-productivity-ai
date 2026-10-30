//! Expander / collapsible disclosure (`UIF-COMP-003`).
//!
//! A disclosure is **one focusable header row that never moves**, plus a body
//! that is either mounted below it or not mounted at all. Every number here is a
//! pure resolver output, and the header box is a function of the row's width and
//! the trailing slot â€” deliberately **not** of whether the body is open. That
//! absence is the whole zero-shift guarantee: there is no call in this module that
//! can hand the header its own open state, so a host cannot make the header shift
//! even by accident.
//!
//! # The header is the row, and the row is the target
//!
//! The header takes the single-line list-tile rung, [`size::CONTROL_MD`], because
//! a disclosure header *is* a list row with a chevron in front of it, and the
//! crate already prints that row's height. The hit target is the whole row rather
//! than the chevron: a `16px` icon is a small thing to aim at when the entire
//! line reads as clickable, and Â§3B.3 asks that the rotation not change the hit
//! target â€” which is trivially satisfied when the target is a box the rotation
//! never touches.
//!
//! # The toggle is a [`GestureSlot`], not a timer
//!
//! [`DisclosureToggle`] wraps [`binding::GestureSlot<bool>`], the same transcript
//! [`crate::surface`] uses for drags. Pressing opens the slot against the value
//! the user was shown; releasing commits the flip exactly once; cancelling reverts
//! to the captured snapshot; a release with no open slot commits nothing. The open
//! state is `Controlled` â€” the host owns it, the resolver only reports what the
//! interaction decided.
//!
//! There is deliberately **no `Live` tier**. A boolean has no continuous value to
//! stream: the press is already visible as the header's `surface.active` fill, so
//! emitting `Live(false)` on every frame of a press would be a tier that says
//! nothing and could be mistaken for a value the host should act on.
//!
//! # Motion, and why nothing here owns a clock
//!
//! Â§3B.3 asks for a chevron rotation and says the body "mounts immediately below
//! the header". Both are honoured without a timer: [`chevron_rotation`] resolves
//! the two settled angles and the mount is a discrete layout decision. An
//! *animated* height is not modelled, because a height that changes every frame
//! is a layout shift for everything below it â€” the exact cost
//! the `UIF-PRIN-003` invariant (`Layout Stability`) exists to price. A
//! host that still wants one feeds [`TimeMs`](crate::surface::tooltip::TimeMs) into its own
//! animation and interpolates between the two settled angles. The angle is the
//! only quantity a host may interpolate, because it changes no box.
//!
//! # Contrast
//!
//! Every pair below is recomputed from raw sRGB by [`crate::token::contrast`] in
//! the sidecar, not copied from the spec's printed column:
//!
//! | Foreground | Fill | Ratio |
//! | --- | --- | --- |
//! | `text.primary` (title) | `surface.raised` | 12.3755:1 |
//! | `text.muted` (chevron) | `surface.raised` | 7.1553:1 |
//! | `text.disabled` (disabled title) | `surface.raised` | 5.5643:1 |
//! | `text.primary` (body) | `surface.base` | 13.1590:1 |
//!
//! The disabled title lands inside the `[4.0, 6.0]` band
//! [`meets_disabled_text_band`](crate::token::contrast::meets_disabled_text_band)
//! enforces. The body takes `surface.base` rather than `surface.sunken` â€” Â§3B.3
//! offers both, and `base` keeps the body the same surface as the pane holding it,
//! so mounting a body moves no visual boundary.
//!
//! Unit tests live in the sibling `expander_tests.rs` sidecar.

use crate::binding::{self, Cancel, Commit, GestureSlot, GestureStart};
use crate::focus::{FocusId, FocusMember};
use crate::state::BaseState;
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use crate::widget::button_widget::Layer;
use iced::{Color, Rectangle};

/// The standard header row's height â€” `size.control.md` (Â§3B.3's `32px`).
///
/// A disclosure header is a list row with a chevron in front of it, so it takes
/// the row height the spec already prints rather than a new number. It clears
/// `size.hit_target.min` with room to spare.
pub const HEADER_HEIGHT: f32 = size::CONTROL_MD;

/// The compact header row's height â€” `size.control.sm` (Â§3B.3's `28px`, used
/// inside tree and JSON views).
pub const HEADER_HEIGHT_COMPACT: f32 = size::CONTROL_SM;

/// The chevron's box edge â€” `size.icon.md` (Â§3B.3's `16px`).
pub const CHEVRON_BOX: f32 = size::ICON_MD;

/// The gap between the chevron box and the title â€” `space.sm`.
pub const CHEVRON_TITLE_GAP: f32 = space::SM;

/// Horizontal padding inside the header row â€” `space.md` (Â§3B.3's `[0, 12px]`).
pub const HEADER_PADDING_X: f32 = space::MD;

/// Chevron rotation with the body closed.
pub const CHEVRON_ROTATE_COLLAPSED: f32 = 0.0;

/// Chevron rotation with the body open.
pub const CHEVRON_ROTATE_EXPANDED: f32 = 90.0;

/// The rotation the chevron travels between the two settled faces.
///
/// Exposed so a host animating the chevron animates *this* span rather than
/// inventing a sweep that disagrees with the settled end states.
pub const CHEVRON_SWEEP: f32 = CHEVRON_ROTATE_EXPANDED - CHEVRON_ROTATE_COLLAPSED;

/// The body's padding on every side â€” `space.md`.
pub const BODY_PADDING: f32 = space::MD;

/// The body's border weight.
pub const BODY_BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The title's rung â€” `type.ui_compact`, the same rung a list-tile title uses.
pub const TITLE_RUNG: &typography::TypeRung = &typography::UI_COMPACT;

/// A trailing control in the header's trailing slot â€” `24Ã—24` (Â§3B.5).
pub const TRAILING_TARGET: f32 = size::HIT_TARGET_MIN;

/// How a group of disclosures coordinates.
///
/// Â§3B.3 does not say whether opening one closes its siblings, so the policy is
/// stated rather than assumed. Recorded as a spec gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AccordionPolicy {
    /// Every disclosure stands on its own; opening one changes nothing else.
    #[default]
    Independent,
    /// Opening one closes every other member of the group.
    SingleOpen,
}

impl AccordionPolicy {
    /// Whether opening `index` closes its siblings.
    #[must_use]
    pub const fn closes_siblings(self) -> bool {
        matches!(self, Self::SingleOpen)
    }
}

/// Which of Â§3B.3's two permitted header rungs a disclosure uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HeaderRung {
    /// The standard `32px` header.
    #[default]
    Standard,
    /// The `28px` header used inside tree and JSON views.
    Compact,
}

impl HeaderRung {
    /// The header height for this rung.
    #[must_use]
    pub const fn height(self) -> f32 {
        match self {
            Self::Standard => HEADER_HEIGHT,
            Self::Compact => HEADER_HEIGHT_COMPACT,
        }
    }
}

/// The header row box for a bounds at a given rung.
///
/// The open state is **not a parameter**. Two disclosures occupying the same
/// bounds resolve the identical header box whether their bodies are mounted or
/// not, which is the zero-shift invariant stated as a signature.
#[must_use]
pub fn header_bounds_at(bounds: Rectangle, rung: HeaderRung) -> Rectangle {
    Rectangle {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width.max(0.0),
        height: rung.height(),
    }
}

/// The standard-rung header row box. See [`header_bounds_at`].
#[must_use]
pub fn header_bounds(bounds: Rectangle) -> Rectangle {
    header_bounds_at(bounds, HeaderRung::Standard)
}

/// The chevron box, vertically centred in the header at its leading edge.
///
/// The box is the same in both faces and the rotation happens inside it, so the
/// rotation cannot move or resize the target.
#[must_use]
pub fn chevron_bounds(header: Rectangle) -> Rectangle {
    Rectangle {
        x: header.x + HEADER_PADDING_X,
        y: header.y + (header.height - CHEVRON_BOX) / 2.0,
        width: CHEVRON_BOX,
        height: CHEVRON_BOX,
    }
}

/// Where the title may be drawn: after the chevron, before the trailing slot.
#[must_use]
pub fn title_bounds(header: Rectangle, trailing_width: f32) -> Rectangle {
    let x = header.x + HEADER_PADDING_X + CHEVRON_BOX + CHEVRON_TITLE_GAP;
    let right = header.x + header.width - HEADER_PADDING_X - trailing_width.max(0.0);
    Rectangle {
        x,
        y: header.y,
        width: (right - x).max(0.0),
        height: header.height,
    }
}

/// The chevron's settled rotation for an open flag.
#[must_use]
pub const fn chevron_rotation(open: bool) -> f32 {
    if open {
        CHEVRON_ROTATE_EXPANDED
    } else {
        CHEVRON_ROTATE_COLLAPSED
    }
}

/// The body box, or `None` when the body is not mounted.
///
/// Collapsed returns `None` rather than a zero-height box at the header's foot, so
/// a host cannot reserve space for content that is not there. A mounted body
/// starts exactly at the header's bottom edge and never overlaps it.
#[must_use]
pub fn body_bounds_at(
    bounds: Rectangle,
    rung: HeaderRung,
    open: bool,
    content_height: f32,
) -> Option<Rectangle> {
    if !open {
        return None;
    }
    let header = header_bounds_at(bounds, rung);
    Some(Rectangle {
        x: bounds.x,
        y: header.y + header.height,
        width: bounds.width.max(0.0),
        height: content_height.max(0.0),
    })
}

/// The body box at the standard rung. See [`body_bounds_at`].
#[must_use]
pub fn body_bounds(bounds: Rectangle, open: bool, content_height: f32) -> Option<Rectangle> {
    body_bounds_at(bounds, HeaderRung::Standard, open, content_height)
}

/// The body's content box, inset by [`BODY_PADDING`].
#[must_use]
pub fn body_content_bounds(body: Rectangle) -> Rectangle {
    Rectangle {
        x: body.x + BODY_PADDING,
        y: body.y + BODY_PADDING,
        width: (body.width - BODY_PADDING * 2.0).max(0.0),
        height: (body.height - BODY_PADDING * 2.0).max(0.0),
    }
}

/// Total height a disclosure occupies at a rung: the header always, the body only
/// when mounted.
#[must_use]
pub const fn total_height_at(rung: HeaderRung, open: bool, content_height: f32) -> f32 {
    if open {
        rung.height() + content_height
    } else {
        rung.height()
    }
}

/// Total height at the standard rung. See [`total_height_at`].
#[must_use]
pub const fn total_height(open: bool, content_height: f32) -> f32 {
    total_height_at(HeaderRung::Standard, open, content_height)
}

/// The focus members a disclosure contributes.
///
/// This is the tab-reachability contract. The header is always a member when the
/// disclosure is enabled. The body's controls are members **only while the body is
/// mounted** â€” collapsed, they are registered as disabled members so the crate's
/// own [`FocusState`](crate::focus::FocusState) walk skips them, and a collapsed body therefore
/// cannot be
/// tabbed into no matter how many focusable things it would have held.
///
/// Registering rather than omitting is deliberate: an omitted member leaves the
/// host to remember to unregister, and a forgotten unregister leaves a live tab
/// stop pointing at content that is not on screen.
#[must_use]
pub fn focus_members(
    header_id: impl Into<FocusId>,
    body_ids: &[FocusId],
    open: bool,
    disabled: bool,
) -> Vec<FocusMember> {
    let mut members = vec![if disabled {
        FocusMember::disabled(header_id)
    } else {
        FocusMember::new(header_id)
    }];
    members.extend(body_ids.iter().map(|id| {
        if open && !disabled {
            FocusMember::new(id.clone())
        } else {
            FocusMember::disabled(id.clone())
        }
    }));
    members
}

/// The header's resolved paint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeaderStyle {
    /// The row fill.
    pub background: Color,
    /// The title's colour.
    pub title_colour: Color,
    /// The chevron's colour.
    pub chevron_colour: Color,
    /// The row's border, `0.0` wide when the header paints none.
    pub border_color: Color,
    /// The border weight.
    pub border_width: f32,
}

/// Resolve the header's paint.
///
/// A dead header is read in `text.disabled` over the resting raised surface â€” it
/// stays visible because a disclosure you cannot open still has to say what it
/// would have opened. A live header takes the crate's usual hover and pressed
/// lifts.
#[must_use]
pub fn resolve_header_style(base: BaseState, palette: &Palette) -> HeaderStyle {
    if base == BaseState::Disabled {
        return HeaderStyle {
            background: palette.surface_raised,
            title_colour: palette.text_disabled,
            chevron_colour: palette.text_disabled,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
        };
    }

    let background = match base {
        BaseState::Pressed => palette.surface_active,
        BaseState::Hovered => palette.surface_hover,
        _ => palette.surface_raised,
    };
    let chevron_colour = match base {
        BaseState::Hovered | BaseState::Pressed => palette.text_primary,
        _ => palette.text_muted,
    };

    HeaderStyle {
        background,
        title_colour: palette.text_primary,
        chevron_colour,
        border_color: Color::TRANSPARENT,
        border_width: 0.0,
    }
}

/// The body's resolved paint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyStyle {
    /// The body fill.
    pub background: Color,
    /// The body's divider stroke.
    pub border_color: Color,
    /// The border weight.
    pub border_width: f32,
}

/// Resolve the body's paint.
///
/// `surface.base` so the body reads as part of the pane rather than as a well
/// sunk into it, with a `border.subtle` divider â€” the `RestingDivider` class â€”
/// so the seam between header and body is drawn rather than implied by alignment.
#[must_use]
pub fn body_style(palette: &Palette) -> BodyStyle {
    BodyStyle {
        background: palette.surface_base,
        border_color: palette.border_subtle,
        border_width: BODY_BORDER_WIDTH,
    }
}

/// The header's painted layer.
#[must_use]
pub fn header_layer(bounds: Rectangle, style: &HeaderStyle) -> Layer {
    Layer {
        bounds,
        fill: style.background,
        border: iced::Border {
            color: style.border_color,
            width: style.border_width,
            radius: shape::RADIUS_WIDGET.into(),
        },
    }
}

/// The body's painted layer for an already-resolved body rectangle.
///
/// The layer consumes a rect rather than re-deriving one, so the rung and the
/// mount decision are made once, in the resolver, and cannot disagree with what
/// gets painted.
#[must_use]
pub fn body_layer(body: Rectangle, style: &BodyStyle) -> Layer {
    Layer {
        bounds: body,
        fill: style.background,
        border: iced::Border {
            color: style.border_color,
            width: style.border_width,
            radius: shape::RADIUS_WIDGET.into(),
        },
    }
}

/// The committed-and-live state of one disclosure's open flag.
///
/// A thin, purpose-named wrapper over [`GestureSlot<bool>`] so the disclosure's
/// transcript is the *same* transcript every other interactive thing in the
/// crate produces: `begin` captures what the user was shown, `commit` settles it,
/// `cancel` reverts to the capture. Nothing here is a new kind of state machine.
#[derive(Clone, Debug, Default)]
pub struct DisclosureToggle {
    open: bool,
    slot: GestureSlot<bool>,
}

impl DisclosureToggle {
    /// A toggle at a settled value, with no gesture open.
    #[must_use]
    pub fn new(open: bool) -> Self {
        Self {
            open,
            slot: GestureSlot::new(),
        }
    }

    /// The settled open flag.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether a press currently holds the slot.
    #[must_use]
    pub fn is_pressed(&self) -> bool {
        self.slot.is_open()
    }

    /// Open the gesture slot against the value the user is currently shown.
    ///
    /// A second press while one is open is refused, not layered â€” the outer
    /// snapshot keeps governing, per [`GestureSlot`]'s nesting rule.
    #[must_use]
    pub fn press(&mut self) -> GestureStart {
        self.slot.begin(&self.open)
    }

    /// Release the header: flip the flag and settle the slot, exactly once.
    ///
    /// Returns `None` with no state change when nothing was pressed, so a stray
    /// release cannot toggle a disclosure.
    pub fn release(&mut self) -> Option<Commit<bool>> {
        let next = !self.open;
        let commit = self.slot.commit(next)?;
        self.open = next;
        Some(commit)
    }

    /// Escape the pending press: revert to the value the press captured.
    ///
    /// With no gesture open there is nothing to revert, so this emits nothing
    /// rather than inventing a `Cancel` for a value nobody changed.
    pub fn cancel(&mut self) -> Option<Cancel<bool>> {
        let outcome = self.slot.cancel()?;
        self.open = *outcome.restored();
        Some(outcome)
    }

    /// Drive the flag to an explicit value and report the commit.
    ///
    /// For keyboard activation, where there is a decision but no press to
    /// release. A slot that is already open is settled through the same path so a
    /// keyboard toggle arriving during a stuck press cannot leave the slot
    /// dangling; with no slot open the commit is produced directly.
    pub fn set_open(&mut self, open: bool) -> Commit<bool> {
        self.open = open;
        match self.slot.commit(open) {
            Some(commit) => commit,
            None => binding::Commit::new(open),
        }
    }
}

/// Apply an [`AccordionPolicy`] when `index` was just opened.
///
/// Under [`AccordionPolicy::SingleOpen`] every *other* open toggle is closed and
/// its commit is returned, so the host sees one honest transition per sibling
/// rather than a silent mutation. Under [`AccordionPolicy::Independent`] nothing
/// happens and the returned vector is empty.
///
/// A sibling whose own gesture slot is open is **not** force-closed: closing it
/// underneath an in-flight press would revert a value that user is still holding.
/// It is skipped and reported by omission.
#[must_use]
pub fn apply_accordion_policy(
    policy: AccordionPolicy,
    toggles: &mut [DisclosureToggle],
    index: usize,
) -> Vec<Commit<bool>> {
    if !policy.closes_siblings() {
        return Vec::new();
    }
    let mut commits = Vec::new();
    for (i, toggle) in toggles.iter_mut().enumerate() {
        if i == index || !toggle.is_open() || toggle.is_pressed() {
            continue;
        }
        commits.push(toggle.set_open(false));
    }
    commits
}

/// A keyboard activation on a focused disclosure header (Â§3B.3: `Enter`/`Space`
/// toggles, `Right` expands, `Left` collapses).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisclosureKey {
    /// `Enter` / `Space` â€” flip whichever way it currently sits.
    Toggle,
    /// `Right` arrow â€” expand.
    Expand,
    /// `Left` arrow â€” collapse.
    Collapse,
}

/// Apply a keyboard activation and report the commit.
///
/// Returns `None` when the key would not have changed anything â€” `Right` on an
/// already-open disclosure, `Left` on an already-closed one. Suppressing the
/// no-op commit matters: a host that persists every commit it is handed would write
/// the same value twice and could mistake the second write for a user decision.
pub fn apply_key(toggle: &mut DisclosureToggle, key: DisclosureKey) -> Option<Commit<bool>> {
    let target = match key {
        DisclosureKey::Toggle => !toggle.is_open(),
        DisclosureKey::Expand => true,
        DisclosureKey::Collapse => false,
    };
    if target == toggle.is_open() {
        return None;
    }
    Some(toggle.set_open(target))
}

#[cfg(test)]
#[path = "expander_tests.rs"]
mod expander_tests;
