//! The menu cascade controller — the state machine that owns **openness**
//! (`UIF-CMENU-002`, `UIF-CMENU-003`, `UIF-FMENU-002`).
//!
//! # Why openness lives here and not in the model
//!
//! [`model`](super::model) owns structure and selection and deliberately
//! refuses to hold a single `is_open`. Three of this spec's rules cannot be
//! stated from inside a row:
//!
//! * **Mutual exclusion spans trees.** `UIF-CMENU-002` says at most one
//!   menu is live; a row cannot see the other tree.
//! * **Sibling replacement is a rule across siblings.** `UIF-FMENU-002`
//!   says opening one submenu branch retires the branch its parent already
//!   had open. A child does not know it has siblings.
//! * **Dismissal is tree-wide.** Click-away closes everything from the
//!   outside; no row owns that.
//!
//! The model answers *"what could open here, and under what id"*. This
//! module answers *"what is open now, in what order, and where the keyboard
//! is"*. Every rule above is one transition of [`MenuCascade`] state.
//!
//! # The state
//!
//! A [`MenuCascade`] holds an ordered stack of [`OpenCard`]s, root first.
//! Each card carries its own [`Menu`] value, the [`SubmenuTrigger`] it
//! hangs from (the parent card plus the row index that opened it — which is
//! what the anchor solver needs to place the card, `UIF-FMENU-002`), and
//! the row focus sits on. The stack *is* the open path: `open[0]` is the
//! root card, `open[len-1]` is the deepest visible card, and the trigger
//! of `open[i]` names the row inside `open[i-1]` that it cascades from.
//!
//! ## The cascade owns cloned models
//!
//! `model.rs` exposes no `row_mut` / `kind_mut` / `child_mut`, so **no
//! consumer of that API can obtain a `&mut Menu` for a nested card** — not
//! the app, not this controller. Activating a checkable three levels down
//! requires exactly that. Rather than leave `UIF-CMENU-003`'s `Enter`
//! unreachable below the root, the cascade *owns the models of the cards it
//! has open*, cloned in at open time from the caller's tree.
//!
//! The consequences are deliberate and are the caller's contract:
//!
//! * render from [`MenuCascade::open_cards`], not from the tree that was
//!   opened — the cascade's copy is the live one;
//! * [`MenuCascade::sync`] re-adopts the caller's tree for every open card,
//!   which is how a row becomes disabled, appears or disappears under a menu
//!   that is already open;
//! * `sync` **overwrites** whatever the cascade changed locally, so the
//!   [`Activation`] each input reports must be applied to the caller's own
//!   state before the next `sync`. The report is the durable record; the
//!   cascade's copy is the display.
//!
//! # The rules
//!
//! **Mutual exclusion (`UIF-CMENU-002`).** One tree at a time.
//! [`MenuCascade::open`] closes whatever is open first — a context menu
//! launched while the fixed navbar menu is showing retires it, and vice
//! versa — so there is no state in which both are live. Re-opening the
//! *same* tree (same kind, same root id) is a no-op rather than a
//! close-and-reopen, so a caller that re-issues the open every frame
//! cannot make the menu flicker.
//!
//! **Persistent parent (`UIF-FMENU-002`).** Opening a card pushes onto the
//! stack; nothing above it is touched. Every ancestor stays open, visible
//! and interactive for as long as the branch is.
//!
//! **Sibling replacement.** [`MenuCascade::open_submenu`] closes the
//! branch *below the card it was called on* before pushing the new one, at
//! the same depth. Opening siblings in turn never grows the stack.
//!
//! **Keyboard (`UIF-CMENU-003`).** `Up`/`Down` cycle the active card's
//! navigable roster — headings, separators and disabled rows are excluded
//! by [`Menu::focus_members`] before the roster is ever built, so skipping
//! is absence rather than a step-over. `Right` opens the focused submenu
//! row and focuses its first enabled child. `Left` and `Escape` close the
//! deepest card and return focus to the row it hung from; at the root that
//! closes the tree. `Enter`/`Space` run [`Menu::activate`] and honour the
//! row's [`ClosePolicy`](super::model::ClosePolicy) — a stay-open checkable
//! flips in place and the tree stays.
//!
//! **Click-away.** [`MenuCascade::close_tree`] is the dismissal: the whole
//! hierarchy goes, immediately, in one unwinding.
//!
//! **Absorbed rows never dismiss.** A heading, separator or disabled row
//! produces [`ActivationEffect::Absorbed`] with `closes_menu: false`, and
//! the cascade closes nothing. The click is consumed inside the card, which
//! is what stops the click-away layer from treating it as outside.
//!
//! # Focus
//!
//! Each open card registers its navigable rows into
//! [`FocusState`](crate::focus) as its own group, and every card pushes a
//! **trap** over its group. Only the topmost trap governs, so `Tab` cycles
//! inside the deepest open card and cannot reach the background —
//! `UIF-MOD-001`'s confinement, borrowed for `UIF-CMENU-002`'s Band 2.
//!
//! The trap stack *is* the open path, one trap per card, and that buys the
//! return-focus rule for free: each trap recorded the focus held when the
//! card opened, which is the parent's submenu row, so
//! [`FocusState::pop_trap`] during a close lands the keyboard back on
//! exactly the row the user came from, at every level.
//!
//! Tearing a card down pops its trap (when it is still the topmost one) and
//! unregisters its group, in that order, so the recorded focus is restored
//! while the card's own members can no longer be landed on.
//! `stabilise` runs after every input and re-asserts the
//! invariant: the active card holds focus, and every open card's roster
//! matches its model.
//!
//! Focus is pushed *through* the model, never through iced's traversal —
//! [`bridge`](crate::bridge) owns that direction and [`CascadeOutcome::focus_task`]
//! is the only iced value this module can hand back.
//!
//! # Decisions this increment had to make
//!
//! **A card with nothing focusable does not open.** A card whose rows are
//! all headings, separators or disabled rows is refused
//! ([`IgnoredReason::NoFocusableRows`]) rather than shown. Its trap would
//! confine an empty scope while focus sat outside it on a background
//! control — a menu painted over a live focus ring underneath. Refusing
//! keeps the invariant *every open card holds focus*, which is what makes
//! the trap sound.
//!
//! **`Right` on a row that cannot open a branch does nothing.** Not a
//! close, not a wrap — the keyboard stays where it is
//! ([`IgnoredReason::NotSubmenu`]).
//!
//! **Interacting with an ancestor card closes the branch below it.**
//! Clicking or activating a row in a card that is not the deepest one
//! collapses every card beneath it first. The pointer left that branch;
//! keeping it open would leave a card hanging from a row the user is no
//! longer on, and the trap stack cannot put focus in two cards at once. The
//! one exception is the row that *is* the open branch's own trigger:
//! activating it again leaves the branch where it is instead of closing and
//! re-opening it under the cursor.
//!
//! **A row disabled while focused moves focus forward, in place.** `sync`
//! re-registers the card's roster and [`FocusState`]'s slot-preserving
//! fall-forward lands focus on the enabled row that took its slot — same
//! card, same level, no jump. If the card has nothing focusable left, the
//! card itself closes and focus returns to the trigger row above.
//!
//! **A focused row whose submenu cannot open leaves the keyboard where it
//! is.** `Right` on a submenu with no enabled children is refused before
//! anything closes; the parent branch is untouched.
//!
//! **Closing the whole tree restores the focus the menu took.** The root
//! card's trap recorded whatever held focus when the tree opened — the
//! navbar trigger, the tree row that was right-clicked — and unwinding to
//! the bottom restores it. If nothing was focused before, focus clears
//! rather than being invented.
//!
//! # Composing with the z-band model (`INC-17`)
//!
//! The cascade owns unwinding *within* the menu tree; the band model owns
//! the order *across* surfaces. The seam is the return value:
//! [`MenuKey::Escape`] on the deepest card closes that card, and on the
//! root card closes the tree and leaves [`MenuCascade::is_open`] `false`.
//! A second `Escape` then reports
//! [`IgnoredReason::NotOpen`] — the cascade has consumed the menu level
//! and is telling the band model its turn has passed. The band model must
//! not ask the cascade to close something it does not own, and the cascade
//! never reaches into a modal: it refuses, and the refusal is how the key
//! propagates upward.
//!
//! Unit tests live in the sibling `cascade_tests.rs` sidecar.

use crate::bridge::focus_task;
use crate::focus::FocusId;
use crate::focus::FocusState;
use crate::focus::GroupId;
use crate::surface::menu::model::Activation;
use crate::surface::menu::model::ActivationEffect;
use crate::surface::menu::model::Menu;
use crate::surface::menu::model::MenuId;

use iced::Task;

// ---------------------------------------------------------------------------
// Which tree
// ---------------------------------------------------------------------------

/// Which kind of root the open tree was launched as.
///
/// `UIF-CMENU-002`'s mutual exclusion is stated over two named things —
/// "a Context Menu or a Fixed Navbar Menu" — so the cascade records which
/// one it is showing rather than inferring it. Two trees that happen to
/// carry identical rows are still different trees, and the kind is what
/// keeps them apart when the root ids collide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuTreeKind {
    /// A pointer-anchored context menu (`UIF-CMENU-001`).
    Context,
    /// A trigger-anchored fixed navbar menu (`UIF-FMENU-001`).
    Fixed,
}

/// Where an open card hangs: the parent card, and the row that opened it.
///
/// The row index is part of the open state rather than something the caller
/// has to remember beside it, because `UIF-FMENU-002` places the child
/// card against the **row** — aligned to the parent row's top edge — not
/// against the parent card. The anchor solver reads this directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmenuTrigger {
    /// The card holding the row that opened this card.
    pub parent: MenuId,
    /// That row's index inside the parent card.
    pub row: usize,
}

// ---------------------------------------------------------------------------
// The open cards
// ---------------------------------------------------------------------------

/// One open card: its model, where it hangs, and where focus sits.
///
/// A card in the stack is not a reference into a tree someone else owns —
/// it carries the [`Menu`] the cascade will mutate when the user activates
/// a row in it. See the [module docs](self#the-cascade-owns-cloned-models)
/// for why that is forced and what the caller must do about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenCard {
    menu: Menu,
    group_id: GroupId,
    trigger: Option<SubmenuTrigger>,
    focused: Option<usize>,
}

impl OpenCard {
    /// The root card of a freshly opened tree.
    fn root(menu: Menu) -> Self {
        let group_id = menu.group_id();
        Self {
            menu,
            group_id,
            trigger: None,
            focused: None,
        }
    }

    /// A submenu card hanging from `trigger`.
    fn child(menu: Menu, trigger: SubmenuTrigger) -> Self {
        let group_id = menu.group_id();
        Self {
            menu,
            group_id,
            trigger: Some(trigger),
            focused: None,
        }
    }

    /// This card's menu id.
    #[must_use]
    pub fn id(&self) -> &MenuId {
        self.menu.id()
    }

    /// The [`FocusState`] group this card's navigable rows are registered
    /// under.
    #[must_use]
    pub fn group_id(&self) -> &GroupId {
        &self.group_id
    }

    /// The model to render for this card.
    #[must_use]
    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    /// Where this card hangs; `None` for the root card.
    #[must_use]
    pub fn trigger(&self) -> Option<&SubmenuTrigger> {
        self.trigger.as_ref()
    }

    /// Whether this is the tree's root card.
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.trigger.is_none()
    }

    /// The row index focus sits on in this card, if it has one.
    ///
    /// For the deepest card this is live focus. For a card with an open
    /// branch below it it is the row that branch hangs from, which is where
    /// `Left` will bring focus back to
    /// (`UIF-CMENU-003`).
    #[must_use]
    pub fn focused_row(&self) -> Option<usize> {
        self.focused
    }
}

// ---------------------------------------------------------------------------
// Input vocabulary
// ---------------------------------------------------------------------------

/// The keys `UIF-CMENU-003` defines for a menu.
///
/// A private vocabulary rather than `iced::keyboard::Key` so the state
/// machine stays free of iced and the app owns the mapping from whatever
/// the platform reported.
///
/// `Tab` is deliberately **absent**. It is not a menu key: it is the
/// traversal key the focus trap confines, and the cascade must not
/// reinterpret it. The host hands `Tab` to
/// [`bridge::tab_task`](crate::bridge::tab_task), which asks
/// [`FocusState`] for the next member — and because the active card's trap
/// is the topmost scope, that answer is always inside the card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuKey {
    /// `Up` — previous navigable row, wrapping.
    Up,
    /// `Down` — next navigable row, wrapping.
    Down,
    /// `Left` — close the deepest card, back to its parent row.
    Left,
    /// `Right` — open the focused submenu row, focus its first child.
    Right,
    /// `Enter` — activate the focused row.
    Enter,
    /// `Space` — activate the focused row.
    Space,
    /// `Escape` — same as `Left`; at the root, close the tree.
    Escape,
}

// ---------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------

/// Why an input changed nothing.
///
/// A typed reason rather than a bare `false`, because the caller's next
/// move depends on which it was: `NotOpen` is how `INC-17` learns the
/// `Escape` key must keep travelling up the z-bands, while `NotSubmenu` is
/// a dead end that stops right there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IgnoredReason {
    /// No menu tree is open.
    NotOpen,
    /// This exact tree is already open. Opening again is a no-op so a
    /// caller that re-issues the open every frame cannot make the menu
    /// flicker closed and open again.
    AlreadyOpen {
        /// The card that is already open.
        card: MenuId,
    },
    /// The addressed card is not part of the open path.
    UnknownCard {
        /// The card that was addressed.
        card: MenuId,
    },
    /// The row does not own a submenu, so `Right` has nowhere to go.
    NotSubmenu {
        /// The card holding the row.
        card: MenuId,
        /// The row index.
        row: usize,
    },
    /// The card cannot hold keyboard focus: every row is a heading, a
    /// separator or disabled. Opening it is refused — see the
    /// [module docs](self#decisions-this-increment-had-to-make).
    NoFocusableRows {
        /// The card with nothing focusable in it.
        card: MenuId,
    },
    /// The row index is past the end of the card.
    RowOutOfRange {
        /// The card holding the row.
        card: MenuId,
        /// The offending index.
        row: usize,
    },
    /// No row is focused, so there is nothing to activate or open.
    NothingFocused {
        /// The card with no focused row.
        card: MenuId,
    },
}

// ---------------------------------------------------------------------------
// Focus transitions
// ---------------------------------------------------------------------------

/// Where focus landed as a result of one cascade input.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FocusTransition {
    /// Focus did not move.
    #[default]
    Unchanged,
    /// Focus moved to `to`. `from` is `None` when nothing held focus
    /// before the input.
    Moved {
        /// Where focus was, if anywhere.
        from: Option<FocusId>,
        /// Where focus is now.
        to: FocusId,
    },
    /// Nothing holds focus now.
    Cleared {
        /// What held focus before the input.
        from: FocusId,
    },
}

/// Describe where focus went between two snapshots.
fn transition(before: Option<FocusId>, after: Option<FocusId>) -> FocusTransition {
    if before == after {
        return FocusTransition::Unchanged;
    }
    match (before, after) {
        (before, Some(to)) => FocusTransition::Moved { from: before, to },
        (Some(from), None) => FocusTransition::Cleared { from },
        (None, None) => FocusTransition::Unchanged,
    }
}

// ---------------------------------------------------------------------------
// The outcome
// ---------------------------------------------------------------------------

/// Everything one cascade input did.
///
/// The cascade never runs anything: it reports. The caller decides which
/// of these to act on — emit the command an [`Activation`] carried,
/// mount and place the [`opened`](Self::opened) cards, tear down the
/// [`closed`](Self::closed) ones, and project
/// [`focus`](Self::focus) onto iced's router with
/// [`focus_task`](Self::focus_task).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CascadeOutcome {
    opened: Vec<MenuId>,
    closed: Vec<MenuId>,
    activation: Option<Activation>,
    focus: FocusTransition,
    ignored: Option<IgnoredReason>,
}

impl CascadeOutcome {
    /// An outcome that says nothing happened.
    #[must_use]
    pub fn noop() -> Self {
        Self::default()
    }

    /// An outcome refusing an input for `reason`.
    #[must_use]
    pub fn ignored(reason: IgnoredReason) -> Self {
        Self {
            ignored: Some(reason),
            ..Self::default()
        }
    }

    /// The cards this input opened, shallowest first.
    #[must_use]
    pub fn opened(&self) -> &[MenuId] {
        &self.opened
    }

    /// The cards this input closed, deepest first.
    #[must_use]
    pub fn closed(&self) -> &[MenuId] {
        &self.closed
    }

    /// The model activation this input performed, when it was a click,
    /// `Enter` or `Space`.
    ///
    /// Carries the effect and the close decision so the caller never
    /// re-reads the tree to learn what just happened.
    #[must_use]
    pub fn activation(&self) -> Option<&Activation> {
        self.activation.as_ref()
    }

    /// Where focus landed.
    #[must_use]
    pub fn focus(&self) -> &FocusTransition {
        &self.focus
    }

    /// Why the input was refused, if it was.
    #[must_use]
    pub fn ignored_reason(&self) -> Option<&IgnoredReason> {
        self.ignored.as_ref()
    }

    /// Whether this input was refused and changed nothing.
    #[must_use]
    pub fn is_ignored(&self) -> bool {
        self.ignored.is_some()
    }

    /// Whether this input moved focus.
    #[must_use]
    pub fn focus_moved(&self) -> bool {
        matches!(self.focus, FocusTransition::Moved { .. })
    }

    /// The [`Task`] that projects this outcome's focus decision onto iced's
    /// focus router.
    ///
    /// `Task::none()` when focus did not move, so a caller can return this
    /// unconditionally from `update` without waking the router for nothing.
    /// The cascade itself never runs a task: the state machine stays
    /// drivable with no runtime, which is what makes every rule in this
    /// file testable without a window.
    pub fn focus_task<Message: Send + 'static>(&self, focus: &FocusState) -> Task<Message> {
        match self.focus {
            FocusTransition::Unchanged => Task::none(),
            FocusTransition::Moved { .. } | FocusTransition::Cleared { .. } => focus_task(focus),
        }
    }
}

// ---------------------------------------------------------------------------
// The cascade
// ---------------------------------------------------------------------------

/// The controller that owns which menu cards are open.
///
/// A pure state machine: every input is a method that takes
/// [`&mut FocusState`](FocusState), mutates the open path, and returns a
/// [`CascadeOutcome`]. Nothing here renders, reads a clock, or runs a
/// task.
///
/// ```text
/// open: [ root ]──▶ [ submenu ]──▶ [ submenu of submenu ]   ← active card
///           ▲            ▲                ▲
///           │            └── trigger ─────┘   (parent card + row index)
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuCascade {
    /// Which tree is open, and as what.
    kind: Option<MenuTreeKind>,
    /// The open path, root card first.
    open: Vec<OpenCard>,
}

impl MenuCascade {
    /// A cascade with nothing open.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // -- queries -----------------------------------------------------------

    /// Whether any menu card is open.
    ///
    /// `false` is also the signal `INC-17` reads to keep an `Escape`
    /// travelling up the z-bands.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.open.is_empty()
    }

    /// The kind of the open tree, if any.
    #[must_use]
    pub fn kind(&self) -> Option<MenuTreeKind> {
        self.kind
    }

    /// The root card's id, if a tree is open.
    #[must_use]
    pub fn root_id(&self) -> Option<&MenuId> {
        self.open.first().map(OpenCard::id)
    }

    /// Every open card, root first.
    ///
    /// This is the anchor solver's work list: card `i` is placed against
    /// `open[i].trigger()`, and its contents come from
    /// [`OpenCard::menu`].
    #[must_use]
    pub fn open_cards(&self) -> &[OpenCard] {
        &self.open
    }

    /// How many cards are open — the cascade depth.
    ///
    /// Sibling replacement keeps this constant, which is the property
    /// `UIF-FMENU-002` is really asserting: opening ten sibling submenus
    /// in a row must not stack ten cards.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.open.len()
    }

    /// The deepest open card — the one that holds focus and owns the trap.
    #[must_use]
    pub fn active_card(&self) -> Option<&OpenCard> {
        self.open.last()
    }

    /// The active card's model.
    #[must_use]
    pub fn active_menu(&self) -> Option<&Menu> {
        self.active_card().map(OpenCard::menu)
    }

    /// The row focus sits on in the active card.
    #[must_use]
    pub fn focused_row(&self) -> Option<usize> {
        self.active_card().and_then(OpenCard::focused_row)
    }

    /// The position of `card` in the open path.
    #[must_use]
    pub fn level_of(&self, card: &MenuId) -> Option<usize> {
        self.open.iter().position(|open| open.id() == card)
    }

    /// Whether `card` is open.
    #[must_use]
    pub fn is_card_open(&self, card: &MenuId) -> bool {
        self.level_of(card).is_some()
    }

    /// The live model of an open card, by id.
    #[must_use]
    pub fn card_menu(&self, card: &MenuId) -> Option<&Menu> {
        self.level_of(card).map(|level| &self.open[level].menu)
    }

    /// The row focus sits on inside one open card.
    #[must_use]
    pub fn focused_row_in(&self, card: &MenuId) -> Option<usize> {
        self.level_of(card)
            .and_then(|level| self.open[level].focused)
    }

    // -- opening -----------------------------------------------------------

    /// Open `source` as a tree of `kind`
    /// (`UIF-CMENU-002`, `UIF-FMENU-001`).
    ///
    /// Any tree already open is closed first, so a context menu over a fixed
    /// navbar menu retires the navbar menu and the other way round. The
    /// root card's focus is its first navigable row.
    ///
    /// Re-opening the same kind over the same root id is refused with
    /// [`IgnoredReason::AlreadyOpen`] rather than closed and reopened: the
    /// alternative is a menu that flickers at any caller that treats
    /// "open" as an idempotent per-frame statement.
    pub fn open(
        &mut self,
        source: &Menu,
        kind: MenuTreeKind,
        focus: &mut FocusState,
    ) -> CascadeOutcome {
        if self.is_open() && self.kind == Some(kind) && self.root_id() == Some(source.id()) {
            return CascadeOutcome::ignored(IgnoredReason::AlreadyOpen {
                card: source.id().clone(),
            });
        }
        if source.first_navigable().is_none() {
            return CascadeOutcome::ignored(IgnoredReason::NoFocusableRows {
                card: source.id().clone(),
            });
        }

        let before = focus.current().cloned();
        let mut outcome = self.close_tree(focus);

        self.kind = Some(kind);
        self.open.push(OpenCard::root(source.clone()));
        self.mount(focus);
        outcome.opened.push(source.id().clone());

        self.finish(focus, before, &mut outcome);
        outcome
    }

    /// Open `source` if a different tree is showing, close it if this one
    /// already is (`UIF-NAV-001`'s trigger latch).
    pub fn toggle(
        &mut self,
        source: &Menu,
        kind: MenuTreeKind,
        focus: &mut FocusState,
    ) -> CascadeOutcome {
        if self.is_open() && self.kind == Some(kind) && self.root_id() == Some(source.id()) {
            return self.close_tree(focus);
        }
        self.open(source, kind, focus)
    }

    /// Open the submenu owned by row `row` of the open card `card`
    /// (`UIF-FMENU-002`).
    ///
    /// Every card below `card` closes first, so a sibling branch is
    /// *replaced* rather than stacked and the depth cannot grow. `card`
    /// itself and everything above it stay open and interactive — that is
    /// the persistent-parent half of the rule.
    ///
    /// This is the one open path for both the pointer and the keyboard:
    /// hovering a submenu row and pressing `Right` are the same request.
    pub fn open_submenu(
        &mut self,
        card: &MenuId,
        row: usize,
        focus: &mut FocusState,
    ) -> CascadeOutcome {
        let Some(level) = self.level_of(card) else {
            return CascadeOutcome::ignored(IgnoredReason::UnknownCard { card: card.clone() });
        };
        let Some(child) = self.open[level]
            .menu
            .row(row)
            .and_then(|row| row.child())
            .cloned()
        else {
            return CascadeOutcome::ignored(IgnoredReason::NotSubmenu {
                card: card.clone(),
                row,
            });
        };
        if child.first_navigable().is_none() {
            return CascadeOutcome::ignored(IgnoredReason::NoFocusableRows {
                card: child.id().clone(),
            });
        }
        if self
            .open
            .get(level + 1)
            .is_some_and(|open| open.id() == child.id())
        {
            return CascadeOutcome::ignored(IgnoredReason::AlreadyOpen {
                card: child.id().clone(),
            });
        }

        let before = focus.current().cloned();
        let mut outcome = CascadeOutcome::noop();
        outcome.closed = self.close_from(focus, level + 1);

        let child_id = child.id().clone();
        self.open.push(OpenCard::child(
            child,
            SubmenuTrigger {
                parent: card.clone(),
                row,
            },
        ));
        self.mount(focus);
        outcome.opened.push(child_id);

        self.finish(focus, before, &mut outcome);
        outcome
    }

    // -- pointer -----------------------------------------------------------

    /// Activate row `row` of the open card `card` — the click path
    /// (`UIF-CMENU-002`, `UIF-FMENU-003`).
    ///
    /// The clicked row takes focus before it fires, so a stay-open row
    /// leaves the keyboard sitting on the thing the user just touched. The
    /// row's own [`ClosePolicy`](crate::surface::menu::model::ClosePolicy) then decides whether the tree goes with it.
    ///
    /// Clicking in a card that is not the deepest one first closes every
    /// card below it: the pointer left that branch, and the trap stack
    /// cannot hold focus in two cards at once. The exception is the row
    /// that is the open branch's own trigger, which is left alone rather
    /// than closed and re-opened under the cursor.
    pub fn activate_row(
        &mut self,
        card: &MenuId,
        row: usize,
        focus: &mut FocusState,
    ) -> CascadeOutcome {
        let Some(level) = self.level_of(card) else {
            return CascadeOutcome::ignored(IgnoredReason::UnknownCard { card: card.clone() });
        };
        if row >= self.open[level].menu.len() {
            return CascadeOutcome::ignored(IgnoredReason::RowOutOfRange {
                card: card.clone(),
                row,
            });
        }

        let before = focus.current().cloned();
        let mut outcome = CascadeOutcome::noop();

        let reopens_open_branch = self.open[level]
            .menu
            .row(row)
            .and_then(|row| row.child_id())
            .is_some_and(|child| {
                self.open
                    .get(level + 1)
                    .is_some_and(|open| open.id() == child)
            });
        if !reopens_open_branch {
            outcome.closed = self.close_from(focus, level + 1);
        }

        self.focus_row(level, row, focus);
        let Some(activation) = self.open[level].menu.activate(row) else {
            self.finish(focus, before, &mut outcome);
            return outcome;
        };

        let opens_child = match &activation.effect {
            ActivationEffect::OpenSubmenu { child } if !reopens_open_branch => Some(child.clone()),
            _ => None,
        };
        let closes_tree = activation.closes_menu;
        outcome.activation = Some(activation);

        if closes_tree {
            outcome.closed.extend(self.close_from(focus, 0));
            self.kind = None;
        } else if let Some(child_id) = opens_child
            && let Some(child) = self.open[level]
                .menu
                .row(row)
                .and_then(|row| row.child())
                .cloned()
        {
            self.open.push(OpenCard::child(
                child,
                SubmenuTrigger {
                    parent: card.clone(),
                    row,
                },
            ));
            self.mount(focus);
            outcome.opened.push(child_id);
        }

        self.finish(focus, before, &mut outcome);
        outcome
    }

    // -- keyboard ----------------------------------------------------------

    /// Run one menu key (`UIF-CMENU-003`).
    ///
    /// The single entry point a host drives from its key handler. `Tab` is
    /// not here and must not be mapped to anything in this module — see
    /// [`MenuKey`].
    pub fn on_key(&mut self, key: MenuKey, focus: &mut FocusState) -> CascadeOutcome {
        match key {
            MenuKey::Up => self.move_focus(-1, focus),
            MenuKey::Down => self.move_focus(1, focus),
            MenuKey::Right => self.open_focused_submenu(focus),
            MenuKey::Left | MenuKey::Escape => self.close_deepest(focus),
            MenuKey::Enter | MenuKey::Space => self.activate_focused(focus),
        }
    }

    /// Cycle focus through the active card's navigable rows, wrapping at
    /// both ends (`UIF-CMENU-003`).
    ///
    /// Headings, separators and disabled rows are not in the roster at all,
    /// so they are skipped by absence rather than stepped over — the walk
    /// distance is the number of rows the keyboard can actually be on.
    pub fn move_focus(&mut self, delta: i32, focus: &mut FocusState) -> CascadeOutcome {
        let Some(level) = self.open.len().checked_sub(1) else {
            return CascadeOutcome::ignored(IgnoredReason::NotOpen);
        };
        let before = focus.current().cloned();
        let mut outcome = CascadeOutcome::noop();

        match self.open[level]
            .menu
            .step_navigable(self.open[level].focused, delta)
        {
            Some(row) => self.focus_row(level, row, focus),
            None => {
                return CascadeOutcome::ignored(IgnoredReason::NoFocusableRows {
                    card: self.open[level].id().clone(),
                });
            }
        }

        self.finish(focus, before, &mut outcome);
        outcome
    }

    /// `Right` — open the focused row's submenu and focus its first
    /// enabled child (`UIF-CMENU-003`).
    pub fn open_focused_submenu(&mut self, focus: &mut FocusState) -> CascadeOutcome {
        let Some(level) = self.open.len().checked_sub(1) else {
            return CascadeOutcome::ignored(IgnoredReason::NotOpen);
        };
        let Some(row) = self.open[level].focused else {
            return CascadeOutcome::ignored(IgnoredReason::NothingFocused {
                card: self.open[level].id().clone(),
            });
        };
        let card = self.open[level].id().clone();
        self.open_submenu(&card, row, focus)
    }

    /// `Enter` / `Space` — activate the focused row and honour its close
    /// policy (`UIF-CMENU-003`, `UIF-FMENU-003`).
    pub fn activate_focused(&mut self, focus: &mut FocusState) -> CascadeOutcome {
        let Some(level) = self.open.len().checked_sub(1) else {
            return CascadeOutcome::ignored(IgnoredReason::NotOpen);
        };
        let Some(row) = self.open[level].focused else {
            return CascadeOutcome::ignored(IgnoredReason::NothingFocused {
                card: self.open[level].id().clone(),
            });
        };
        let card = self.open[level].id().clone();
        self.activate_row(&card, row, focus)
    }

    // -- closing -----------------------------------------------------------

    /// Close the deepest open card and return focus to the row it hung
    /// from — or close the whole tree when the root card is the deepest
    /// (`UIF-CMENU-003`).
    pub fn close_deepest(&mut self, focus: &mut FocusState) -> CascadeOutcome {
        let Some(level) = self.open.len().checked_sub(1) else {
            return CascadeOutcome::ignored(IgnoredReason::NotOpen);
        };
        let before = focus.current().cloned();
        let mut outcome = CascadeOutcome::noop();
        outcome.closed = self.close_from(focus, level);
        if !self.is_open() {
            self.kind = None;
        }
        self.finish(focus, before, &mut outcome);
        outcome
    }

    /// Close the entire hierarchy (`UIF-CMENU-002`'s click-away).
    ///
    /// Every card is unwound from the deepest end, each popping its own
    /// trap, so focus lands back on whatever held it before the tree
    /// opened — the navbar trigger, or the tree row that was right-clicked
    /// — rather than being cleared into nowhere.
    pub fn close_tree(&mut self, focus: &mut FocusState) -> CascadeOutcome {
        if !self.is_open() {
            return CascadeOutcome::noop();
        }
        let before = focus.current().cloned();
        let mut outcome = CascadeOutcome::noop();
        outcome.closed = self.close_from(focus, 0);
        self.kind = None;
        self.finish(focus, before, &mut outcome);
        outcome
    }

    // -- model adoption ----------------------------------------------------

    /// Re-adopt the caller's tree for every open card.
    ///
    /// This is how the world changes underneath an open menu: a row gets
    /// disabled, a submenu gains an item, a card's content is rebuilt. Each
    /// open card is resolved against `source` along its trigger path and
    /// replaced with the current value; a card `source` no longer has
    /// closes, and `stabilise` then applies the
    /// focus rules to what is left.
    ///
    /// A row that became disabled while focused does not keep the ring:
    /// re-registering the card's roster lets [`FocusState`]'s
    /// slot-preserving fall-forward move focus to the enabled row that
    /// took its slot, in the same card at the same level.
    ///
    /// **Adoption overwrites local changes.** Apply the
    /// [`Activation`] each input reported to the caller's own state before
    /// calling this, or the toggle the user just flipped is replaced by
    /// the stale value.
    pub fn sync(&mut self, source: &Menu, focus: &mut FocusState) -> CascadeOutcome {
        if !self.is_open() {
            return CascadeOutcome::noop();
        }
        let before = focus.current().cloned();
        let mut outcome = CascadeOutcome::noop();

        let mut level = 0;
        while level < self.open.len() {
            match resolve_from(source, &self.open, level).cloned() {
                Some(menu) => {
                    self.open[level].menu = menu;
                    level += 1;
                }
                None => {
                    outcome.closed.extend(self.close_from(focus, level));
                    break;
                }
            }
        }
        if !self.is_open() {
            self.kind = None;
        }

        self.finish(focus, before, &mut outcome);
        outcome
    }

    // -- internals ---------------------------------------------------------

    /// Register the card just pushed, trap it, and focus its first row.
    ///
    /// Registration comes first so the trap names a scope that exists, and
    /// the focus call is allowed by the trap it just installed. The
    /// remembered row index is left to [`stabilise`](Self::stabilise),
    /// which reads it back out of the focus model rather than assuming the
    /// focus call landed.
    fn mount(&mut self, focus: &mut FocusState) {
        let Some(card) = self.open.last() else {
            return;
        };
        let group = card.group_id.clone();
        let members = card.menu.focus_members();
        focus.register_group(group.clone(), members);
        focus.push_trap(group.clone());
        focus.focus_first_in_group(&group);
    }

    /// Move focus to a row of an open card, remembering it.
    ///
    /// A row that is not navigable is not registered, so the focus call
    /// refuses and the card's remembered position is left alone — a heading
    /// never becomes the keyboard's location.
    fn focus_row(&mut self, level: usize, row: usize, focus: &mut FocusState) {
        let Some(id) = self.open[level].menu.focus_id(row) else {
            return;
        };
        if focus.focus(&id) {
            self.open[level].focused = Some(row);
        }
    }

    /// Close every card from `from` to the deepest, deepest first.
    ///
    /// Each card pops its own trap while that trap is still the topmost one
    /// — which it is, because closing always runs from the deepest end —
    /// and the pop restores the focus recorded when the card opened: the
    /// parent's submenu row. The parent's remembered focus is then pinned
    /// to that same row, so `focused` and the focus model agree without
    /// either one having to guess.
    fn close_from(&mut self, focus: &mut FocusState, from: usize) -> Vec<MenuId> {
        let mut closed = Vec::new();
        while self.open.len() > from {
            let card = match self.open.pop() {
                Some(card) => card,
                None => break,
            };
            closed.push(card.id().clone());

            if focus.active_trap() == Some(card.group_id()) {
                focus.pop_trap();
            }
            focus.unregister_group(card.group_id());

            if let Some(trigger) = card.trigger.as_ref()
                && let Some(parent) = self.open.last_mut()
                && parent.id() == &trigger.parent
            {
                parent.focused = Some(trigger.row);
            }
        }
        closed
    }

    /// Re-assert the invariants after a change, then report where focus
    /// ended up.
    ///
    /// The invariants are: no open card is one the keyboard cannot be in;
    /// every open card's registered roster matches its model; focus sits
    /// inside the active card; and each card's remembered row matches the
    /// focus model.
    fn finish(
        &mut self,
        focus: &mut FocusState,
        before: Option<FocusId>,
        outcome: &mut CascadeOutcome,
    ) {
        self.stabilise(focus, outcome);
        outcome.focus = transition(before, focus.current().cloned());
    }

    /// The invariant pass. See [`finish`](Self::finish).
    fn stabilise(&mut self, focus: &mut FocusState, outcome: &mut CascadeOutcome) {
        // A card the keyboard cannot enter must not stay open: its trap
        // would confine an empty scope while focus sat outside it.
        while let Some(level) = self.open.len().checked_sub(1) {
            if self.open[level].menu.first_navigable().is_some() {
                break;
            }
            outcome.closed.extend(self.close_from(focus, level));
        }
        if !self.is_open() {
            self.kind = None;
            return;
        }

        for card in &self.open {
            focus.register_group(card.group_id.clone(), card.menu.focus_members());
        }

        let last = self.open.len() - 1;
        let active = self.open[last].group_id.clone();
        let outside = match focus.current() {
            Some(current) => focus
                .group_of(current)
                .is_none_or(|group| group.id() != &active),
            None => true,
        };
        if outside && focus.focus_first_in_group(&active).is_none() {
            // The focus model refused to place the keyboard in the active
            // card — a trap above ours, nothing enabled left in scope. Say
            // so by clearing the remembered row rather than keeping a
            // position the keyboard is not on, which would let `Enter`
            // fire a row nothing marks as selected.
            self.open[last].focused = None;
        }

        if let Some(current) = focus.current().cloned() {
            for card in &mut self.open {
                if let Some(row) = card.menu.index_of_focus_id(&current) {
                    card.focused = Some(row);
                }
            }
        }
    }
}

/// Resolve the `level`th open card against `source` along its trigger path.
///
/// Identity is checked at every hop: a card whose recorded trigger no
/// longer names it is treated as missing rather than being matched by name
/// alone, so a rebuilt tree that moved a submenu to a different row closes
/// the stale branch instead of rendering the wrong card under the right
/// id.
fn resolve_from<'s>(source: &'s Menu, open: &[OpenCard], level: usize) -> Option<&'s Menu> {
    let card = open.get(level)?;
    if level == 0 {
        return (source.id() == card.id()).then_some(source);
    }
    let parent = resolve_from(source, open, level - 1)?;
    let parent_card = open.get(level - 1)?;
    let trigger = card.trigger()?;
    if trigger.parent != *parent_card.id() {
        return None;
    }
    let child = parent.row(trigger.row)?.child()?;
    (child.id() == card.id()).then_some(child)
}

#[cfg(test)]
#[path = "cascade_tests.rs"]
mod cascade_tests;
