//! The transient notification stack (`UIF-COMP-002`).
//!
//! A toast is a [`NotificationModel`]
//! that is allowed to leave. That single difference — it can go away
//! without the host doing anything — is where every hard part of this
//! module comes from, because "it leaves on its own" is a statement about
//! time, and time is the one input this crate is not allowed to read.
//!
//! # The clock is a parameter, never a read
//!
//! [`ToastTimer`] takes a [`TimeMs`] from the host on every call and is
//! pure about it: the same timer probed at the same instant answers the
//! same way however many times it is asked, and no code path here calls
//! `Instant::now`, reads a system clock, or spawns a task. The host's
//! frame loop calls [`ToastStack::advance`] once per frame with that
//! frame's reading and gets back whatever expired.
//!
//! That shape is not a convenience. A timer that read its own clock could
//! not be tested without sleeping, could not be replayed, and would make
//! a notification's lifetime depend on when the test machine happened to
//! be built.
//!
//! # Pause preserves; it does not reset
//!
//! §5 says hovering or focusing a toast pauses its auto-dismiss. The
//! obvious implementation — restart the countdown on exit — is wrong in
//! a way that is easy to miss and impossible to argue with: a user who
//! hovers for four seconds of a five-second toast and then moves away
//! would get five *more* seconds, and a user who waves the pointer
//! across the stack repeatedly could keep a notification alive forever
//! by accident.
//!
//! [`ToastTimer::pause`] therefore banks the **remaining** duration and
//! [`ToastTimer::resume`] restarts from the banked remainder. Hover for
//! four seconds of five and one second is left, not five.
//!
//! # `Warning` and `Danger` cannot be armed
//!
//! The severity gate is upstream of the timer rather than inside it.
//! [`effective_auto_dismiss_ms`]
//! returns `None` for those two severities whatever the model asked for,
//! so the timer a `Danger` toast gets is [`ToastTimer::never`] — a timer
//! with no budget, that [`ToastTimer::pause`] refuses, and that cannot
//! expire at any instant. There is no value the host can pass that makes
//! one time out.
//!
//! # Max-visible: nothing is destroyed that the user has not dismissed
//!
//! §3B.2 says "max 3 visible" and stops there. It does not say what
//! happens to the fourth, and the two readings are not equivalent: push
//! the oldest off and you have auto-dismissed it, which §3B.2 forbids
//! for `Warning` and `Danger`. So the policy is the strict one —
//!
//! * the newest toast always takes the anchor;
//! * to make room, the oldest **auto-dismissible** toast is evicted
//!   ([`DismissReason::Overflow`](super::notification::DismissReason::Overflow));
//! * if every visible slot is held by a notification that must stay, the
//!   newcomer **waits** in a pending queue rather than displacing it;
//! * a queued toast's countdown starts when it becomes **visible**, not
//!   when it was posted — a notification the user never saw should not
//!   spend its life being unseen.
//!
//! Recorded as a spec gap: §3B.2 should say which of these it means.
//!
//! # `Escape` is conditional, and the condition is focus
//!
//! §5's Band 1 row says `Escape` unwinds **only** when focus is inside
//! the overlay. [`ToastStack::unwind_answer`] implements exactly that
//! and nothing more: no toast focused →
//! [`DeclineReason::NotFocused`],
//! and the key moves on down the chain. Because a toast never takes focus
//! on its own, a stack that is merely *showing* cannot steal `Escape`
//! from the workspace, and cannot steal it from a modal trap either.
//!
//! # Stacking, and what a dismissal moves
//!
//! The stack is anchored bottom-right and grows upward. It is a Band 1
//! overlay: it reserves no layout, so **nothing outside the stack moves
//! when a toast leaves**. Inside the stack the surviving toasts do shift
//! — the anchor is a fixed point, so removing the bottom toast drops the
//! ones above it by that toast's height plus the gap. That is the
//! decision, stated rather than discovered: `UIF-PRIN-003` protects
//! reserved bands inside a container, and a stack that reserves nothing
//! has no band for a dismissal to shift.
//!
//! Unit tests live in the sibling `toast_tests.rs` sidecar.

use std::collections::VecDeque;

use crate::composite::info_bar::{action_focus_id, dismiss_focus_id};
use crate::composite::notification::{
    NotificationId, NotificationModel, Severity, effective_auto_dismiss_ms,
};
use crate::focus::{FocusId, FocusMember, FocusState, GroupId};
use crate::surface::tooltip::TimeMs;
use crate::surface::zorder::{DeclineReason, UnwindAnswer};
use crate::token::space;
use iced::Rectangle;

/// The toast's width — §3B.2's `360px` (`size.toast.width`).
///
/// A registered Surface Geometry Constant: §2.3 has no `size.toast.width`
/// rung for the spec to have named, so the figure is carried here against
/// its §-reference rather than passed off as a token read.
pub const WIDTH: f32 = 360.0;

/// The anchor's inset from the viewport's right edge — `space.lg`
/// (§3B.2's `16px`).
pub const ANCHOR_GUTTER: f32 = space::LG;

/// The clearance above the Bottom Command Bar — `space.lg` (§3B.2's
/// `16px above Bottom Command Bar`).
pub const BARRIER_GUTTER: f32 = space::LG;

/// The gap between stacked toasts — `space.sm` (§3B.2's `8px`).
pub const STACK_GAP: f32 = space::SM;

/// How many toasts may be on screen at once (§3B.2's "max 3 visible").
pub const MAX_VISIBLE: usize = 3;

/// The clamp §3B.2 applies to [`WIDTH`] — `viewport.width - 32px`.
///
/// `space.xxl` is the same `32px` the spec names, and it is twice
/// [`ANCHOR_GUTTER`]: the spec's inset and its clamp are one measurement
/// used at both edges. Spelled as the token rather than as
/// `ANCHOR_GUTTER * 2.0` so the constant carries no bare literal for a
/// multiplier that only restates the token's own value.
pub const WIDTH_CLAMP: f32 = space::XXL;

/// The toast's resolved width in a viewport.
///
/// Clamped so a narrow window never pushes a toast off its own screen.
/// The floor at `0.0` is what a viewport narrower than the clamp gives:
/// a zero-width toast paints nothing, which is the honest answer, rather
/// than a negative width that no renderer agrees about.
#[must_use]
pub fn toast_width(viewport_width: f32) -> f32 {
    WIDTH.min((viewport_width - WIDTH_CLAMP).max(0.0))
}

/// The anchor rectangle — where the newest toast sits.
///
/// `barrier_top` is the top edge of whatever the stack must clear (the
/// Bottom Command Bar), or the viewport's bottom edge when there is
/// nothing below.
#[must_use]
pub fn anchor_bounds(viewport: Rectangle, barrier_top: f32, width: f32, height: f32) -> Rectangle {
    Rectangle {
        x: (viewport.x + viewport.width - ANCHOR_GUTTER - width).max(viewport.x),
        y: barrier_top - BARRIER_GUTTER - height,
        width,
        height,
    }
}

/// Every visible toast's rectangle, in arrival order.
///
/// The newest (last in) sits at the anchor and each earlier one sits
/// above it, which is what "stacking upward" means. Uniform height is
/// assumed because the stack's geometry is a function of *how many*
/// toasts there are, not of what any of them says — a toast with a long
/// message does not push its neighbours further up than a short one, so
/// the layout cannot be made to depend on content.
#[must_use]
pub fn stack_bounds(
    viewport: Rectangle,
    barrier_top: f32,
    height: f32,
    count: usize,
) -> Vec<Rectangle> {
    let width = toast_width(viewport.width);
    let mut out = Vec::with_capacity(count);
    let mut cursor = barrier_top - BARRIER_GUTTER;
    for _ in 0..count {
        let top = cursor - height;
        out.push(Rectangle {
            x: (viewport.x + viewport.width - ANCHOR_GUTTER - width).max(viewport.x),
            y: top,
            width,
            height,
        });
        cursor = top - STACK_GAP;
    }
    out.reverse();
    out
}

/// What a toast's timer is doing at a given instant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerState {
    /// This notification has no budget and never will. `Warning`, `Danger`,
    /// and any model that asked for no timer.
    Never,
    /// Counting down.
    Running {
        /// Milliseconds left.
        remaining_ms: u64,
    },
    /// Held by hover or focus, with its banked remainder.
    Paused {
        /// Milliseconds left when the hold began.
        remaining_ms: u64,
    },
    /// The budget is spent.
    Expired,
}

impl TimerState {
    /// Whether this state can still lead to an auto-dismiss.
    #[must_use]
    pub const fn is_live(self) -> bool {
        matches!(self, Self::Running { .. } | Self::Paused { .. })
    }
}

/// A countdown that can be held without being reset.
///
/// Every method that cares about *when* takes the instant as an
/// argument. The struct holds no clock and reads none, so
/// [`tick`](Self::tick) is pure and a test can probe the same timer at a
/// hundred instants without a single sleep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToastTimer {
    /// `None` means this notification never auto-dismisses.
    budget_ms: Option<u64>,
    /// The remainder banked by a pause. Only meaningful while paused.
    banked_ms: u64,
    /// `Some` while running: the instant the current run began.
    running_since: Option<TimeMs>,
    /// Latched once the budget is spent, so a spent timer stays spent
    /// rather than becoming runnable again.
    expired: bool,
}

impl ToastTimer {
    /// A timer that cannot expire at any instant.
    ///
    /// What a `Warning` or `Danger` toast gets. Not "a very long timer" —
    /// a timer with no budget, so there is no number a host can wait out.
    #[must_use]
    pub const fn never() -> Self {
        Self {
            budget_ms: None,
            banked_ms: 0,
            running_since: None,
            expired: false,
        }
    }

    /// A timer armed for `total_ms`, starting now.
    ///
    /// A `total_ms` of zero is expired the moment it is armed, which is
    /// the honest reading of "dismiss immediately" rather than a special
    /// case.
    #[must_use]
    pub fn armed(total_ms: u64, now: TimeMs) -> Self {
        let mut timer = Self {
            budget_ms: Some(total_ms),
            banked_ms: total_ms,
            running_since: Some(now),
            expired: false,
        };
        if total_ms == 0 {
            timer.expired = true;
            timer.running_since = None;
        }
        timer
    }

    /// Build the timer a severity and a requested window actually earn.
    ///
    /// The severity gate is applied here, so the function that constructs
    /// a toast's timer is also the function that refuses the ones the
    /// spec forbids. Asking for `Some(5_000)` on a `Danger` yields
    /// [`never`](Self::never).
    #[must_use]
    pub fn for_severity(severity: Severity, requested_ms: Option<u32>, now: TimeMs) -> Self {
        match effective_auto_dismiss_ms(severity, requested_ms) {
            None => Self::never(),
            Some(ms) => Self::armed(u64::from(ms), now),
        }
    }

    /// Whether this timer has no budget at all.
    #[must_use]
    pub const fn is_never(&self) -> bool {
        self.budget_ms.is_none()
    }

    /// Whether the timer has already spent its budget.
    #[must_use]
    pub const fn has_expired(&self) -> bool {
        self.expired
    }

    /// Milliseconds left at `now`, or `None` for a timer that never has.
    #[must_use]
    pub fn remaining_at(&self, now: TimeMs) -> Option<u64> {
        // A never-timer has no remainder to report at all, as opposed to
        // a spent one, which has zero.
        self.budget_ms?;
        if self.expired {
            return Some(0);
        }
        match self.running_since {
            None => Some(self.banked_ms),
            Some(since) => Some(self.banked_ms.saturating_sub(now.elapsed_since(since))),
        }
    }

    /// What the timer is doing at `now`. Pure.
    #[must_use]
    pub fn tick(&self, now: TimeMs) -> TimerState {
        if self.budget_ms.is_none() {
            return TimerState::Never;
        }
        if self.expired {
            return TimerState::Expired;
        }
        match self.running_since {
            None => TimerState::Paused {
                remaining_ms: self.banked_ms,
            },
            Some(since) => {
                let remaining = self.banked_ms.saturating_sub(now.elapsed_since(since));
                if remaining == 0 {
                    TimerState::Expired
                } else {
                    TimerState::Running {
                        remaining_ms: remaining,
                    }
                }
            }
        }
    }

    /// Hold the countdown, banking what is left.
    ///
    /// Returns whether a hold actually began. A never-timer and an
    /// already-held or already-spent timer both refuse, and refusing is
    /// what keeps a hover over a `Danger` toast from being mistaken for
    /// a paused one.
    pub fn pause(&mut self, now: TimeMs) -> bool {
        if self.budget_ms.is_none() || self.expired || self.running_since.is_none() {
            return false;
        }
        self.banked_ms = self.remaining_at(now).unwrap_or_default();
        self.running_since = None;
        true
    }

    /// Release the hold, continuing from the banked remainder.
    ///
    /// **Not** a reset. The run restarts at `now` against the remainder
    /// that was banked, so time spent held is time the toast keeps.
    pub fn resume(&mut self, now: TimeMs) -> bool {
        if self.budget_ms.is_none() || self.expired || self.running_since.is_some() {
            return false;
        }
        if self.banked_ms == 0 {
            self.expired = true;
            return false;
        }
        self.running_since = Some(now);
        true
    }

    /// Settle the timer against `now`, reporting whether it just expired.
    ///
    /// Idempotent: once spent it reports `false` on every later call, so
    /// a host that advances a dismissed toast's timer cannot get a
    /// second dismissal out of it.
    pub fn advance(&mut self, now: TimeMs) -> bool {
        if self.expired || self.budget_ms.is_none() || self.running_since.is_none() {
            return false;
        }
        if self.remaining_at(now).unwrap_or(1) != 0 {
            return false;
        }
        self.expired = true;
        self.running_since = None;
        self.banked_ms = 0;
        true
    }
}

/// One toast on the stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastEntry<ActionId> {
    /// What the host posted.
    model: NotificationModel<ActionId>,
    timer: ToastTimer,
    hovered: bool,
    focused: bool,
}

impl<ActionId> ToastEntry<ActionId> {
    /// The posted model.
    #[must_use]
    pub fn model(&self) -> &NotificationModel<ActionId> {
        &self.model
    }

    /// The notification's identity.
    #[must_use]
    pub fn id(&self) -> &NotificationId {
        &self.model.id
    }

    /// The severity.
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.model.severity
    }

    /// Whether a hover is currently holding this toast's countdown.
    #[must_use]
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Whether this toast currently holds focus.
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Whether anything is currently holding the countdown.
    #[must_use]
    pub fn is_held(&self) -> bool {
        self.hovered || self.focused
    }

    /// The timer's state at `now`.
    #[must_use]
    pub fn timer_state(&self, now: TimeMs) -> TimerState {
        self.timer.tick(now)
    }

    /// Milliseconds left, or `None` if this toast never times out.
    #[must_use]
    pub fn remaining_ms(&self, now: TimeMs) -> Option<u64> {
        self.timer.remaining_at(now)
    }
}

/// What posting a toast did.
///
/// The new toast itself is not carried out — the caller reads it back
/// from [`ToastStack::visible`] — because the stack owns it and handing
/// out a second copy would let the host hold a `ToastEntry` that the
/// stack's timers, holds and dismissal do not reach.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PushOutcome<ActionId> {
    /// The toast took the anchor. `evicted` is the older
    /// auto-dismissible toast that had to leave to make room, if one did.
    Shown {
        /// What overflow pushed off, if anything.
        evicted: Option<Box<ToastEntry<ActionId>>>,
    },
    /// Every visible slot is held by a notification that must not be
    /// pushed off, so this one waits.
    Queued,
}

/// The transient notification stack.
///
/// Owns the visible toasts, the pending queue, and the hover/focus holds
/// that pause them. It does not own a clock, a renderer, or the host's
/// [`FocusState`]; it is handed each of those when asked.
#[derive(Clone, Debug)]
pub struct ToastStack<ActionId> {
    visible: Vec<ToastEntry<ActionId>>,
    pending: VecDeque<NotificationModel<ActionId>>,
}

impl<ActionId> Default for ToastStack<ActionId> {
    fn default() -> Self {
        Self::new()
    }
}

impl<ActionId> ToastStack<ActionId> {
    /// An empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self {
            visible: Vec::new(),
            pending: VecDeque::new(),
        }
    }

    /// The visible toasts, oldest first; the newest is last.
    #[must_use]
    pub fn visible(&self) -> &[ToastEntry<ActionId>] {
        &self.visible
    }

    /// How many toasts are waiting for a visible slot.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Whether nothing is showing or waiting.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.visible.is_empty() && self.pending.is_empty()
    }

    /// Post a notification.
    ///
    /// The newest toast always takes the anchor. Room is made by evicting
    /// the oldest **auto-dismissible** toast; a `Warning` or `Danger` in
    /// a visible slot is never evicted, and if all three slots are held
    /// by those the newcomer waits instead.
    ///
    /// `now` starts the new toast's countdown. A toast admitted from the
    /// pending queue is armed at the instant it becomes visible, not at
    /// the instant it was posted.
    pub fn push(
        &mut self,
        model: NotificationModel<ActionId>,
        now: TimeMs,
    ) -> PushOutcome<ActionId> {
        if self.visible.len() < MAX_VISIBLE {
            self.visible.push(self.make_entry(model, now));
            return PushOutcome::Shown { evicted: None };
        }

        let Some(index) = self.oldest_dismissible_index() else {
            self.pending.push_back(model);
            return PushOutcome::Queued;
        };

        let evicted = Box::new(self.visible.remove(index));
        self.visible.push(self.make_entry(model, now));
        PushOutcome::Shown {
            evicted: Some(evicted),
        }
    }

    /// Advance every visible timer to `now` and return what expired.
    ///
    /// Called once per frame with that frame's clock reading. A held
    /// toast's timer does not move, so hovering genuinely stops the
    /// clock rather than merely delaying the check.
    pub fn advance(&mut self, now: TimeMs) -> Vec<ToastEntry<ActionId>> {
        let mut expired = Vec::new();
        let mut i = 0;
        while i < self.visible.len() {
            if self.visible[i].timer.advance(now) {
                expired.push(self.visible.remove(i));
            } else {
                i += 1;
            }
        }
        self.admit_from_pending(now);
        expired
    }

    /// Set the hover hold on a visible toast.
    ///
    /// Returns whether the toast was found. Hovering a toast that has
    /// already left the stack answers `false` rather than remembering the
    /// hover against an id that no longer has a timer — the pointer
    /// cannot be stranded on something that is gone.
    pub fn set_hovered(&mut self, id: &NotificationId, hovered: bool, now: TimeMs) -> bool {
        let Some(entry) = self.visible.iter_mut().find(|e| e.id() == id) else {
            return false;
        };
        if entry.hovered == hovered {
            return true;
        }
        entry.hovered = hovered;
        // Focus may still be holding the toast, so only release the
        // countdown when neither channel is holding it.
        Self::apply_hold(entry, entry.is_held(), now);
        true
    }

    /// Set the focus hold on a visible toast. Same semantics as
    /// [`set_hovered`](Self::set_hovered).
    pub fn set_focused(&mut self, id: &NotificationId, focused: bool, now: TimeMs) -> bool {
        let Some(entry) = self.visible.iter_mut().find(|e| e.id() == id) else {
            return false;
        };
        if entry.focused == focused {
            return true;
        }
        entry.focused = focused;
        // Hover may still be holding the toast, so only release the
        // countdown when neither channel is holding it.
        Self::apply_hold(entry, entry.is_held(), now);
        true
    }

    /// Remove a toast by id, whatever state it is in.
    ///
    /// The entry comes back with its `hovered`/`focused` flags still set,
    /// so a caller can tell that the pointer was on the toast when it
    /// went away. What matters is that the **stack** no longer holds the
    /// id: a later [`set_hovered`](Self::set_hovered) for it answers
    /// `false` instead of pausing a timer that is gone, and the next
    /// [`advance`](Self::advance) admits whatever was waiting.
    pub fn remove(&mut self, id: &NotificationId, now: TimeMs) -> Option<ToastEntry<ActionId>> {
        if let Some(index) = self.visible.iter().position(|e| e.id() == id) {
            let entry = self.visible.remove(index);
            self.admit_from_pending(now);
            return Some(entry);
        }
        let index = self.pending.iter().position(|m| m.id == *id)?;
        let model = self.pending.remove(index)?;
        // A toast pulled out of the pending queue never had a timer to
        // stop, so it comes back held-shut rather than as a live entry
        // that could still expire in the caller's hands.
        Some(ToastEntry {
            model,
            timer: ToastTimer::never(),
            hovered: false,
            focused: false,
        })
    }

    /// The focus group this stack owns for one toast.
    #[must_use]
    pub fn group_for(id: &NotificationId) -> GroupId {
        GroupId::new(format!("toast:{}", id.as_str()))
    }

    /// The tab stops one toast contributes: its action button, then its
    /// `[×]`.
    ///
    /// The toast body is **not** a member. A notification you can tab to
    /// but not tab *through* is a trap, so only the two controls that
    /// make it leave are stops; both absent means the toast is not
    /// focusable at all and `Escape` will never find it.
    #[must_use]
    pub fn focus_members(model: &NotificationModel<ActionId>) -> Vec<FocusMember> {
        let mut members = Vec::new();
        if model.action.is_some() {
            members.push(FocusMember::new(action_focus_id(&model.id)));
        }
        if model.dismissible {
            members.push(FocusMember::new(dismiss_focus_id(&model.id)));
        }
        members
    }

    /// The ids of every control in the stack, across every visible toast.
    #[must_use]
    pub fn owned_focus_ids(&self) -> Vec<FocusId> {
        self.visible
            .iter()
            .flat_map(|entry| Self::focus_members(entry.model()))
            .map(|member| member.id)
            .collect()
    }

    /// Whether focus currently sits inside the stack.
    ///
    /// **This is the whole `Escape` condition** from §5's Band 1 row.
    /// Computed from the focus state rather than remembered, so a toast
    /// that was focused and then lost focus does not keep claiming the
    /// key.
    #[must_use]
    pub fn holds_focus(&self, focus: &FocusState) -> bool {
        let Some(current) = focus.current() else {
            return false;
        };
        self.visible.iter().any(|entry| {
            let members = Self::focus_members(entry.model());
            members.iter().any(|m| m.id == *current)
        })
    }

    /// The visible toast that currently holds focus, if any.
    #[must_use]
    pub fn focused_entry(&self, focus: &FocusState) -> Option<&ToastEntry<ActionId>> {
        let current = focus.current()?;
        self.visible.iter().find(|entry| {
            Self::focus_members(entry.model())
                .iter()
                .any(|m| m.id == *current)
        })
    }

    /// Answer an `Escape` walk.
    ///
    /// Declines with [`DeclineReason::NotFocused`] unless focus is inside
    /// the stack, which is §5's "`Escape` unwinds **only** when focus is
    /// inside the overlay" said as code. Dismissing the focused toast is
    /// the unwind; the stack never dismisses a toast the user was not on,
    /// so one `Escape` retires exactly one notification.
    pub fn unwind_answer(&mut self, focus: &FocusState, now: TimeMs) -> UnwindAnswer {
        if self.visible.is_empty() {
            return UnwindAnswer::Declined(DeclineReason::NotOpen);
        }
        let Some(entry) = self.focused_entry(focus) else {
            return UnwindAnswer::Declined(DeclineReason::NotFocused);
        };
        let id = entry.id().clone();
        self.remove(&id, now);
        UnwindAnswer::Dismissed
    }

    /// Whether a toast may be shown without stealing focus.
    ///
    /// A toast never focuses itself, so this is trivially true for the
    /// stack itself — but the *host* needs the same answer before it is
    /// tempted to call `focus()` on a newly posted toast. With a modal
    /// trap open the answer is no, and the trap keeps what it already had.
    #[must_use]
    pub fn may_take_focus(focus: &FocusState) -> bool {
        !focus.is_trapped()
    }

    fn make_entry(&self, model: NotificationModel<ActionId>, now: TimeMs) -> ToastEntry<ActionId> {
        ToastEntry {
            timer: ToastTimer::for_severity(model.severity, model.auto_dismiss_ms, now),
            model,
            hovered: false,
            focused: false,
        }
    }

    /// The oldest visible toast whose severity permits being pushed off.
    fn oldest_dismissible_index(&self) -> Option<usize> {
        self.visible
            .iter()
            .position(|entry| entry.severity().may_auto_dismiss())
    }

    fn admit_one(&mut self, now: TimeMs) -> Option<ToastEntry<ActionId>> {
        let model = self.pending.pop_front()?;
        Some(self.make_entry(model, now))
    }

    fn admit_from_pending(&mut self, now: TimeMs) {
        while self.visible.len() < MAX_VISIBLE {
            match self.admit_one(now) {
                Some(entry) => self.visible.push(entry),
                None => break,
            }
        }
    }

    fn apply_hold(entry: &mut ToastEntry<ActionId>, held: bool, now: TimeMs) {
        if held {
            entry.timer.pause(now);
        } else {
            entry.timer.resume(now);
        }
    }
}

#[cfg(test)]
#[path = "toast_tests.rs"]
mod toast_tests;
