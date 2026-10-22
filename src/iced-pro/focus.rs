//! The keyboard focus model — which control holds focus, and the rules for
//! moving it (`UIF-PRIN-005`, `UIF-MOD-001`, `UIF-CMENU-003`).
//!
//! # Why this module exists
//!
//! Gap **G-10** in `doc/planning/iced-pro-tasks.md` established that stock
//! iced 0.14 cannot give anything except `text_input` and `text_editor` a
//! keyboard focus path: those two are the only `operation::Focusable`
//! implementations in the crate, nothing calls `focus_next` /
//! `focus_previous`, and there is no widget-level focus event at all.
//! Decision **D-7** answers that by having `iced-pro` own focus outright.
//!
//! The owning half is this module, and it is deliberately kept **free of
//! iced** — no `Widget`, no `operation`, no `Task`, no colour, no geometry.
//! What lives here is the *policy*: traversal order, skipping, roving
//! movement and modal confinement, written as plain transitions of plain
//! state. `INC-10` bolts that policy onto iced's focus router. Because the
//! rules are here rather than in the router, every one of them is verifiable
//! without a window, and none of them can drift with how iced happens to
//! route events.
//!
//! # Not to be confused with its neighbours
//!
//! * [`crate::state`] resolves *what state a control is in* — including the
//!   `focused` flag that turns a ring on.
//! * [`crate::widget::focus`] resolves *what that ring looks like* against
//!   the control's fill.
//! * This module decides *which control holds focus* before either of those
//!   questions can be asked.
//!
//! # The group model
//!
//! Focus is organised into ordered [`FocusGroup`]s — a tab strip, a menu's
//! rows, one modal's controls, the base workspace. Registration order is
//! traversal order, and re-registering an existing group replaces its
//! members while keeping its position in that order. A member carries an
//! `enabled` flag: a disabled member keeps its position in the list but
//! traversal steps over it, which is what `UIF-PRIN-004` (unavailable
//! actions stay visible) requires of a greyed-out menu row.
//!
//! Non-interactive rows — menu headings, separators — need no special case.
//! A group holds what the host registers into it, so a heading is either
//! absent or registered disabled, and both produce the same walk.
//!
//! # The trap
//!
//! [`FocusState::push_trap`] confines traversal to a single group, which is
//! what makes `UIF-MOD-001`'s "Tab MUST NEVER traverse to background panes
//! underneath the scrim" true rather than aspirational. Traps nest: each
//! records the focus that was held when it was pushed, and
//! [`FocusState::pop_trap`] restores through the stack. Only the **topmost**
//! trap governs — a dialog over a dialog confines to the upper one.
//!
//! # Four decisions, made rather than left to fall through
//!
//! **1. Unregistering the focused member: the slot is preserved.**
//! When the focused member vanishes — its group unregistered, or dropped by
//! a re-registration — focus keeps the *ordinal slot* it occupied in the
//! active traversal order: the member that now sits at that index takes
//! focus, wrapping to the front if the list shrank past the end. If nothing
//! is focusable left, focus clears. Preserving the slot means the user's
//! position in the tab order survives a rebuild instead of being reset to
//! the top of the window, and because the slot is computed inside the
//! *active scope*, no structural change can use it to escape a trap.
//!
//! One special case: a trap pointing at a group that is unregistered goes
//! with the group, because its scope no longer exists. When the *topmost*
//! trap is the one removed, its recorded focus is restored, so closing a
//! dialog puts focus back where it was when the dialog opened. Traps
//! stacked above the removed group are left in place rather than unwound
//! with it — a dialog further up the stack is still open, and dropping its
//! confinement would let focus reach the background under a scrim that is
//! still painted.
//!
//! **2. Roving focus wraps; it does not clamp.**
//! [`FocusState::move_within`] — the `Up`/`Down` primitive behind
//! `UIF-CMENU-003` — wraps at both ends. The spec's own verb is "Cycles
//! focus through enabled interactive rows", and that is how a desktop menu
//! actually behaves: past the last row, `Down` returns to the first. A
//! clamped menu leaves the keyboard stuck at the bottom pressing a key that
//! does nothing, and it makes the distance between two rows depend on where
//! you came from. Wrapping bounds that distance by the group's size.
//!
//! **3. A member disabled while focused loses focus, forward.**
//! [`FocusState::set_member_enabled`] applies the same slot rule, so focus
//! lands on the next enabled member in the active scope. It does *not*
//! stay. [`crate::state::resolve`] zeroes the focus ring for a disabled
//! control, so a focus that stayed would be invisible — the user would be
//! pressing keys into a control nothing on screen marks as selected, which
//! is worse than having no selection at all. Moving forward is also the
//! ARIA roving-tabindex convention, so it is the behaviour a keyboard user
//! already expects. If the scope has nothing enabled left, focus clears
//! rather than jumping to a group the user never asked for.
//!
//! **4. Focus outside the active trap is rejected.**
//! [`FocusState::focus`] returns `false` and changes nothing when the id is
//! unregistered, is disabled, or sits outside the topmost trap's group.
//! `UIF-MOD-001` forbids reaching the background, and a trap that only
//! held against `Tab` would be a hole: `INC-10` sets focus from clicks and
//! from programmatic commands too, so every path has to pass the same gate.
//! Rejecting rather than clamping keeps the refusal visible to the caller as
//! a `false` instead of silently moving focus somewhere it did not ask for.
//!
//! Two related edges follow from the same rule. Pushing a trap for a group
//! that is not registered yet is allowed — the trap is a claim about an
//! *id*, resolved at traversal time, so while the group is missing the
//! scope is simply empty and focus still cannot reach the background. And
//! [`FocusState::push_trap`] does not move focus by itself: `UIF-MOD-001`'s
//! "focus moves automatically to its primary interactive control" is the
//! host's explicit next call to [`FocusState::focus_first_in_group`],
//! which keeps each operation single-purpose.
//!
//! # Caller contract
//!
//! A `FocusId` should belong to exactly one group. The model resolves the
//! first match it finds rather than rejecting a duplicate, because a UI
//! that registers the same control twice is already broken and a panic in
//! the focus layer would take the window down with it.
//!
//! Unit tests live in the sibling `focus_tests.rs` sidecar.

use std::fmt;

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// A stable identifier for a focusable item.
///
/// Menu rows and dialog controls are created and destroyed dynamically, so
/// an id cannot be a widget handle or a pointer: the host has to be able to
/// name an item before it is mounted and after it is gone. A newtype over
/// `String` gives that, stays cheap to clone, and hashes so ids can key a
/// map of per-item state.
///
/// Ids compare by content — two built from the same string are the same
/// item, which is what lets a rebuilt widget tree re-claim focus.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FocusId(String);

impl FocusId {
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

impl fmt::Display for FocusId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for FocusId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for FocusId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// A stable identifier for a focus group.
///
/// Distinct from [`FocusId`] so a member id cannot be passed where a group
/// id is expected: the two are interchangeable strings at the call site but
/// name different things, and the compiler is the only thing that can keep
/// them apart.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GroupId(String);

impl GroupId {
    /// Create a group id from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Read the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for GroupId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for GroupId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

// ---------------------------------------------------------------------------
// Groups
// ---------------------------------------------------------------------------

/// One member of a focus group: an item plus whether traversal may stop on
/// it.
///
/// The enabled flag is carried rather than the member being removed because
/// `UIF-PRIN-004` requires unavailable actions to stay visible. A greyed
///-out menu row is still a row the user can see; it is simply not a row the
/// keyboard may stop at.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FocusMember {
    /// The item this member stands for.
    pub id: FocusId,
    /// Whether traversal may stop here. Disabled members keep their
    /// position but are stepped over.
    pub enabled: bool,
}

impl FocusMember {
    /// A member traversal may stop on.
    pub fn new(id: impl Into<FocusId>) -> Self {
        Self {
            id: id.into(),
            enabled: true,
        }
    }

    /// A member traversal steps over.
    pub fn disabled(id: impl Into<FocusId>) -> Self {
        Self {
            id: id.into(),
            enabled: false,
        }
    }
}

/// An ordered set of focusable members under one id.
///
/// Constructed through [`FocusState::register_group`]; exposed read-only so
/// a host can inspect the roster it registered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FocusGroup {
    id: GroupId,
    members: Vec<FocusMember>,
}

impl FocusGroup {
    /// This group's id.
    #[must_use]
    pub fn id(&self) -> &GroupId {
        &self.id
    }

    /// The members, in traversal order.
    #[must_use]
    pub fn members(&self) -> &[FocusMember] {
        &self.members
    }

    /// The enabled members, in traversal order.
    pub fn enabled_members(&self) -> impl Iterator<Item = &FocusMember> {
        self.members.iter().filter(|member| member.enabled)
    }

    fn holds(&self, id: &FocusId) -> bool {
        self.members.iter().any(|member| member.id == *id)
    }

    fn holds_enabled(&self, id: &FocusId) -> bool {
        self.members
            .iter()
            .any(|member| member.id == *id && member.enabled)
    }
}

/// One entry of the trap stack: the confining group plus the focus to come
/// back to.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Trap {
    group: GroupId,
    restore: Option<FocusId>,
}

/// What [`FocusState::pop_trap`] removed and where focus ended up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrapPop {
    /// The group whose trap was removed.
    pub group: GroupId,
    /// The focus recorded when that trap was pushed.
    pub recorded: Option<FocusId>,
    /// What holds focus after the pop — `recorded` when it is still
    /// focusable, otherwise the model's fall-back.
    pub restored: Option<FocusId>,
    /// The id that held focus before the pop and no longer does.
    pub lost: Option<FocusId>,
}

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

/// The single source of truth for keyboard focus.
///
/// Every operation mutates `self` and reports what moved; none of them
/// returns an iced `Task`, reads a colour, or touches a widget. Drive it
/// from `INC-10`'s plumbing, or drive it directly in a test with no window
/// at all — the answers are the same.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusState {
    /// Registered groups, in traversal order.
    groups: Vec<FocusGroup>,
    /// What holds focus right now.
    current: Option<FocusId>,
    /// Active focus traps, innermost last. Only the last entry governs.
    traps: Vec<Trap>,
}

impl FocusState {
    /// An empty model: no groups, nothing focused, no traps.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // -- registration ------------------------------------------------------

    /// Register a group, or replace the roster of one already registered.
    ///
    /// A new group is appended, so registration order is traversal order.
    /// Re-registering an existing id replaces its members but keeps its
    /// position in the order — a menu that rebuilds its rows must not jump
    /// to the end of the tab sequence.
    ///
    /// Returns the id that *lost* focus because of this call, under the
    /// slot-preserving rule documented in the [module
    /// docs](self#four-decisions-made-rather-than-left-to-fall-through);
    /// `None` means focus was untouched. Read
    /// [`current`](Self::current) for where focus actually is.
    pub fn register_group(&mut self, id: GroupId, members: Vec<FocusMember>) -> Option<FocusId> {
        let slot = self.slot_of_current();
        match self.groups.iter_mut().find(|group| group.id == id) {
            Some(existing) => existing.members = members,
            None => self.groups.push(FocusGroup { id, members }),
        }
        self.settle(slot, None)
    }

    /// Remove a group and every member it held.
    ///
    /// If the focused member was in this group, focus keeps its slot in the
    /// remaining traversal order. Traps pointing at the removed group are
    /// dropped with it, and when the *topmost* trap is the one being removed
    /// its recorded focus is restored — closing a dialog puts focus back
    /// where it was when the dialog opened.
    ///
    /// Traps stacked *above* a removed group are deliberately left alone
    /// rather than unwound with it. A dialog still open further up the stack
    /// is still open, and dropping its confinement would let focus reach
    /// the background underneath a scrim that is still painted. Only the
    /// trap whose scope actually vanished goes.
    ///
    /// Returns the id that lost focus, or `None` when focus was untouched
    /// (including when no such group exists).
    pub fn unregister_group(&mut self, id: &GroupId) -> Option<FocusId> {
        let slot = self.slot_of_current();
        let index = self.groups.iter().position(|group| group.id == *id)?;
        self.groups.remove(index);

        // Only the topmost trap's recorded focus may be restored: anything
        // deeper would have to cross a trap that is still governing, and
        // `settle` refuses that for exactly the same reason `focus` does.
        let preferred = if self.traps.last().is_some_and(|trap| trap.group == *id) {
            self.traps.last().and_then(|trap| trap.restore.clone())
        } else {
            None
        };
        self.traps.retain(|trap| trap.group != *id);
        self.settle(slot, preferred)
    }

    /// Set whether a member may be stopped on.
    ///
    /// Disabling the focused member moves focus forward under the slot rule
    /// (see the [module docs](self#four-decisions-made-rather-than-left-to-fall-through));
    /// enabling one never moves focus, because nothing asked for it.
    ///
    /// Returns the id that lost focus, or `None` when focus was untouched —
    /// which includes the group or member not existing, and the flag already
    /// holding the requested value.
    pub fn set_member_enabled(
        &mut self,
        group: &GroupId,
        member: &FocusId,
        enabled: bool,
    ) -> Option<FocusId> {
        let slot = self.slot_of_current();
        let group = self.groups.iter_mut().find(|g| g.id == *group)?;
        let target = group.members.iter_mut().find(|m| m.id == *member)?;
        if target.enabled == enabled {
            return None;
        }
        target.enabled = enabled;
        self.settle(slot, None)
    }

    // -- direct focus ------------------------------------------------------

    /// Move focus to `id`.
    ///
    /// Honoured only when `id` is registered, enabled, and inside the
    /// active scope. Anything else is **rejected**: the state is left
    /// exactly as it was and `false` is returned, so a stray click or a
    /// programmatic focus request cannot put focus on a dead control or —
    /// the one that matters for `UIF-MOD-001` — on a background pane
    /// underneath a modal.
    ///
    /// Returns whether focus is on `id` after the call, which is `true`
    /// when it already was.
    pub fn focus(&mut self, id: &FocusId) -> bool {
        if self.current.as_ref() == Some(id) {
            return true;
        }
        if !self.focusable_within(id) {
            return false;
        }
        self.current = Some(id.clone());
        true
    }

    /// Drop focus entirely.
    ///
    /// Always allowed: an empty focus ring cannot escape a trap any more than
    /// a background control can. Returns what was cleared, `None` if
    /// nothing held focus.
    pub fn clear(&mut self) -> Option<FocusId> {
        self.current.take()
    }

    // -- traversal ---------------------------------------------------------

    /// `Tab` — the next focusable member across the active scope, wrapping
    /// at the end.
    ///
    /// Disabled members are skipped and, while a trap is up, so is every
    /// group outside it. With no focus at all, focus lands on the first
    /// focusable member. Returns `None`, leaving the state unchanged, when
    /// nothing in the active scope can hold focus.
    pub fn focus_next(&mut self) -> Option<FocusId> {
        self.walk(1)
    }

    /// `Shift+Tab` — the mirror of [`focus_next`](Self::focus_next),
    /// wrapping to the *last* member at the start of the scope.
    pub fn focus_previous(&mut self) -> Option<FocusId> {
        self.walk(-1)
    }

    /// Focus the first enabled member of `group`.
    ///
    /// Rejected — `None`, state unchanged — when the group is unknown, has
    /// no enabled member, or lies outside the active trap.
    pub fn focus_first_in_group(&mut self, group: &GroupId) -> Option<FocusId> {
        let id = self
            .active_group(group)?
            .members
            .iter()
            .find(|member| member.enabled)
            .map(|member| member.id.clone())?;
        self.current = Some(id.clone());
        Some(id)
    }

    /// Focus the last enabled member of `group`.
    ///
    /// Same rejection rules as
    /// [`focus_first_in_group`](Self::focus_first_in_group).
    pub fn focus_last_in_group(&mut self, group: &GroupId) -> Option<FocusId> {
        let id = self
            .active_group(group)?
            .members
            .iter()
            .rev()
            .find(|member| member.enabled)
            .map(|member| member.id.clone())?;
        self.current = Some(id.clone());
        Some(id)
    }

    /// Roving focus: move `delta` steps inside one group (`UIF-CMENU-003`).
    ///
    /// This is the `Up` / `Down` primitive a menu drives — `delta` of `1`
    /// and `-1` respectively, with larger magnitudes supported for
    /// page-wise jumps. Disabled members are skipped, and movement
    /// **wraps** at both ends: past the last enabled row comes the first,
    /// because that is what a desktop menu does and because the spec's own
    /// verb for this row is "Cycles focus through enabled interactive
    /// rows". See the [module docs](self#four-decisions-made-rather-than-left-to-fall-through)
    /// for why wrapping beat clamping.
    ///
    /// When focus is not currently on an enabled member of this group —
    /// nothing is focused, or a sibling submenu just took the focus — a
    /// positive `delta` enters at the first member and a negative one at
    /// the last, which is how `Right` into a freshly opened submenu lands
    /// on its first row.
    ///
    /// `delta` of `0` never moves. Returns the newly focused id, or `None`
    /// leaving the state unchanged when the group is unknown, lies outside
    /// the active trap, or has no enabled member.
    pub fn move_within(&mut self, group: &GroupId, delta: i32) -> Option<FocusId> {
        if delta == 0 {
            return self.current.clone();
        }
        let focusable: Vec<&FocusId> = self
            .active_group(group)?
            .members
            .iter()
            .filter(|member| member.enabled)
            .map(|member| &member.id)
            .collect();
        let position = self.position_in(&focusable);
        let index = step(position, i64::from(delta), focusable.len())?;
        self.land(focusable[index].clone())
    }

    // -- traps ---------------------------------------------------------------

    /// Confine traversal to `group` until the matching
    /// [`pop_trap`](Self::pop_trap).
    ///
    /// While this trap is the topmost one, [`focus_next`](Self::focus_next),
    /// [`focus_previous`](Self::focus_previous), [`focus`](Self::focus) and
    /// [`move_within`](Self::move_within) can only reach members of this
    /// group, which is the mechanism behind `UIF-MOD-001`. Traps nest:
    /// each records the focus held at push time so a stack of dialogs
    /// unwinds back through every level.
    ///
    /// The trap does not move focus by itself. Focusing the dialog's
    /// primary control is the caller's explicit next step
    /// ([`focus_first_in_group`](Self::focus_first_in_group)), which keeps
    /// this operation to exactly one job.
    ///
    /// Pushing a trap for a group that is not registered yet is allowed.
    /// The trap names an *id* and is resolved at traversal time, so until
    /// that group appears the active scope is empty: traversal is a no-op
    /// rather than a fall-through to the background.
    pub fn push_trap(&mut self, group: GroupId) {
        let restore = self.current.clone();
        self.traps.push(Trap { group, restore });
    }

    /// Remove the topmost trap and restore the focus recorded when it was
    /// pushed.
    ///
    /// The recorded focus wins whenever it is still focusable — including
    /// over a valid focus elsewhere inside the trap, because the contract is
    /// "back to what this dialog took", not "stay where you drifted to". If
    /// it is no longer focusable, the focus the trap was holding is kept
    /// when it is still legal, and otherwise lands on the first focusable
    /// member of the now-wider scope. A trap pushed while nothing was
    /// focused restores *nothing*: focus clears rather than keeping what
    /// the dialog picked up.
    ///
    /// Returns `None` when no trap is active.
    pub fn pop_trap(&mut self) -> Option<TrapPop> {
        let trap = self.traps.pop()?;
        let recorded = trap.restore;
        let previous = self.current.clone();

        if let Some(recorded) = recorded.as_ref() {
            self.settle(None, Some(recorded.clone()));
        } else {
            // Nothing held focus when the trap was pushed, so nothing holds
            // it on the way out. Restoring means going back to an empty
            // focus, not keeping whatever the dialog picked up.
            self.current = None;
        }

        Some(TrapPop {
            group: trap.group,
            recorded,
            restored: self.current.clone(),
            lost: previous.filter(|id| self.current.as_ref() != Some(id)),
        })
    }

    /// The group the topmost trap confines traversal to.
    #[must_use]
    pub fn active_trap(&self) -> Option<&GroupId> {
        self.traps.last().map(|trap| &trap.group)
    }

    /// How many traps are stacked.
    #[must_use]
    pub fn trap_depth(&self) -> usize {
        self.traps.len()
    }

    /// Whether any trap is active.
    #[must_use]
    pub fn is_trapped(&self) -> bool {
        !self.traps.is_empty()
    }

    // -- queries -----------------------------------------------------------

    /// What holds focus right now.
    #[must_use]
    pub fn current(&self) -> Option<&FocusId> {
        self.current.as_ref()
    }

    /// Whether `id` holds focus.
    #[must_use]
    pub fn is_focused(&self, id: &FocusId) -> bool {
        self.current.as_ref() == Some(id)
    }

    /// Whether `id` is registered and enabled anywhere, trap or not.
    #[must_use]
    pub fn is_focusable(&self, id: &FocusId) -> bool {
        self.groups.iter().any(|group| group.holds_enabled(id))
    }

    /// Whether `id` is a member of a group the current trap allows.
    #[must_use]
    pub fn in_active_scope(&self, id: &FocusId) -> bool {
        self.active_scope().iter().any(|group| group.holds(id))
    }

    /// Every registered group, in traversal order.
    #[must_use]
    pub fn groups(&self) -> &[FocusGroup] {
        &self.groups
    }

    /// Look up a group by id.
    #[must_use]
    pub fn group(&self, id: &GroupId) -> Option<&FocusGroup> {
        self.groups.iter().find(|group| group.id == *id)
    }

    /// The group a member belongs to, first match wins.
    #[must_use]
    pub fn group_of(&self, id: &FocusId) -> Option<&FocusGroup> {
        self.groups.iter().find(|group| group.holds(id))
    }

    /// The enabled members of one group, in traversal order.
    #[must_use]
    pub fn focusable_members(&self, group: &GroupId) -> Vec<&FocusId> {
        self.group(group)
            .map(|group| group.enabled_members().map(|member| &member.id).collect())
            .unwrap_or_default()
    }

    // -- internals ---------------------------------------------------------

    /// The groups traversal may walk: the whole list with no trap, otherwise
    /// only the topmost trap's group — and nothing at all while that group
    /// is unregistered.
    fn active_scope(&self) -> &[FocusGroup] {
        match self.active_trap().and_then(|id| self.group(id)) {
            Some(group) => std::slice::from_ref(group),
            None if self.is_trapped() => &[],
            None => self.groups.as_slice(),
        }
    }

    /// The group named by `id`, but only if the active trap allows it.
    fn active_group(&self, id: &GroupId) -> Option<&FocusGroup> {
        if self.active_trap().is_some_and(|trap| trap != id) {
            return None;
        }
        self.group(id)
    }

    /// Whether `id` may hold focus right now: enabled, and inside the
    /// active scope.
    fn focusable_within(&self, id: &FocusId) -> bool {
        self.active_scope()
            .iter()
            .any(|group| group.holds_enabled(id))
    }

    /// Every focusable id in the active scope, in traversal order.
    fn focusable_ids(&self) -> Vec<FocusId> {
        self.active_scope()
            .iter()
            .flat_map(|group| group.enabled_members())
            .map(|member| member.id.clone())
            .collect()
    }

    /// Where the current focus sits in the active traversal order, before
    /// some structural change moves the ground under it.
    fn slot_of_current(&self) -> Option<usize> {
        let current = self.current.as_ref()?;
        self.active_scope()
            .iter()
            .flat_map(|group| group.enabled_members())
            .position(|member| member.id == *current)
    }

    /// Where `current` sits in an ordered id list.
    fn position_in(&self, ids: &[&FocusId]) -> Option<usize> {
        let current = self.current.as_ref()?;
        ids.iter().position(|id| *id == current)
    }

    /// Re-point focus after the world changed underneath it.
    ///
    /// `preferred` — where focus would like to go, honoured whenever it is
    /// focusable in the active scope. Otherwise the current focus is kept
    /// if it survived, and if it did not it lands back in the slot it
    /// occupied (`slot`) among the ids that remain, wrapping if the list
    /// shrank past the end. Nothing was focused, or nothing is focusable,
    /// and focus stays absent rather than being invented.
    ///
    /// Returns the id that lost focus.
    fn settle(&mut self, slot: Option<usize>, preferred: Option<FocusId>) -> Option<FocusId> {
        let previous = self.current.clone();
        match preferred.filter(|id| self.focusable_within(id)) {
            Some(preferred) => self.current = Some(preferred),
            None if previous
                .as_ref()
                .is_some_and(|id| self.focusable_within(id)) => {}
            None if previous.is_some() => {
                let focusable = self.focusable_ids();
                let landed =
                    slot_index(focusable.len(), slot).map(|index| focusable[index].clone());
                self.current = landed;
            }
            None => {}
        }
        previous.filter(|id| self.current.as_ref() != Some(id))
    }

    /// Set focus and report it, for the traversal helpers that always land
    /// somewhere.
    fn land(&mut self, id: FocusId) -> Option<FocusId> {
        self.current = Some(id.clone());
        Some(id)
    }

    /// Shared implementation of `focus_next` / `focus_previous`.
    fn walk(&mut self, delta: i64) -> Option<FocusId> {
        let focusable: Vec<&FocusId> = self
            .active_scope()
            .iter()
            .flat_map(|group| group.enabled_members())
            .map(|member| &member.id)
            .collect();
        let position = self.position_in(&focusable);
        let index = step(position, delta, focusable.len())?;
        self.land(focusable[index].clone())
    }
}

/// Advance `position` by `delta` steps through `len` slots, wrapping.
///
/// With no position — nothing focused, or focus outside the list being
/// walked — a positive delta enters at the first slot and a negative one
/// at the last. `None` when there is nowhere to go.
fn step(position: Option<usize>, delta: i64, len: usize) -> Option<usize> {
    let len = i64::try_from(len).ok()?;
    if len == 0 {
        return None;
    }
    let target = match position {
        Some(position) => i64::try_from(position).ok()? + delta,
        None => {
            if delta > 0 {
                0
            } else {
                len - 1
            }
        }
    };
    usize::try_from(target.rem_euclid(len)).ok()
}

/// The member that now occupies `slot`, wrapping into a shorter list.
fn slot_index(len: usize, slot: Option<usize>) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(slot.unwrap_or(0) % len)
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod focus_tests;
