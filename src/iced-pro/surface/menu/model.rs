//! The menu tree data model — rows, groups, selection and close policy, with
//! no rendering, no widgets and no event handling (`UIF-FMENU-*`,
//! `UIF-CMENU-*`).
//!
//! # What this module decides
//!
//! A menu is a value: an ordered list of [`Row`]s under a stable [`MenuId`].
//! Everything the rest of the menu engine needs is a pure query over that
//! value, and every change is a method that reports what it changed so the
//! cascade (`INC-16`) can react without re-reading the world.
//!
//! Nothing here paints. There is no colour, no font, no length and no
//! geometry in this file — §4.9.2's state matrix is the renderer's job
//! (`INC-15`), and it reads tokens. What this module supplies is the set of
//! *distinctions* that matrix names: Rest / Hovered / Pressed / Keyboard
//! Focused / Active Submenu Parent / Destructive-hovered /
//! Disabled-Placeholder / Section Heading are all answers to questions the
//! model can already answer ("is it destructive?", "is it navigable?",
//! "is it a submenu parent?"), plus the pointer and keyboard facts the
//! renderer cannot know on its own.
//!
//! # The seven row kinds, and the two things that are *not* kinds
//!
//! [`RowKind`] carries [`RowKind::Action`], [`RowKind::Check`],
//! [`RowKind::Radio`], [`RowKind::Submenu`], [`RowKind::Heading`] and
//! [`RowKind::Separator`]. Two entries of §4.9.1's vocabulary are
//! deliberately **not** kinds:
//!
//! * **Disabled / placeholder** is [`Row::is_enabled`] set to `false` on an
//!   interactive kind, not its own kind. A greyed-out `Delete…` is still a
//!   destructive action row — its label, its leading slot, its command
//!   identity and its close policy all survive; only the ability to fire is
//!   gone. Making `Disabled` a kind would throw all of that away and force
//!   every consumer to carry the real kind alongside it.
//! * **Destructive** is [`Row::destructive`], an orthogonal flag. §4.9.1
//!   puts destructive *actions* in the final group, and a destructive
//!   *toggle* ("Close all windows and quit") is a perfectly ordinary
//!   checkable. A kind cannot express "a checkable that is also
//!   destructive", so the flag does.
//!
//! # Navigability is not interactivity
//!
//! Three nested predicates, deliberately separate names for three separate
//! questions:
//!
//! | Predicate | Means |
//! |---|---|
//! | [`Row::interactive`] | the kind can carry a command — Action, Check, Radio, Submenu |
//! | [`Row::activatable`] | `interactive && enabled` — a click here does something |
//! | [`Row::navigable`] | `interactive && enabled` — `Up`/`Down` may stop here (`UIF-CMENU-003`) |
//!
//! `activatable` and `navigable` coincide today and are still kept apart,
//! because they answer different consumers: the renderer asks `interactive`
//! to decide the pointer cursor, the traversal layer asks `navigable` to
//! build the focus group, and activation is what a click or `Enter` does.
//! Headings and separators are excluded **by kind**; disabled rows are
//! excluded **by the flag**. Either way the exclusion happens before
//! registration, which is the part that matters — see the next section.
//!
//! # Mapping onto [`crate::focus::FocusState`]: exclusion, not registration-and-disabled
//!
//! [`Menu::focus_members`] produces the roster for exactly one
//! [`crate::focus::FocusState`] group, and it contains **only navigable rows**. A heading
//! is not registered as a disabled member; it is absent.
//!
//! That is a real difference, not a style point. [`crate::focus::FocusState`] keeps a
//! disabled member in the roster and steps over it, which is right for a
//! toolbar where the disabled control still holds a tab stop's *position*.
//! In a menu, registering headings would mean the group's size counts rows
//! that can never be focused, so `move_within`'s wrap distance, the
//! slot-preserving fall-forward in [`crate::focus::FocusState::register_group`], and
//! every "how far to the last item" answer would be computed over a list
//! that includes furniture. Excluding them makes the group's length equal
//! the number of rows the keyboard can actually be on, which is what
//! `UIF-CMENU-003` cycles over.
//!
//! The mapping itself:
//!
//! * [`Menu::group_id`] — `menu:<path>`, one group per menu card.
//! * [`Menu::focus_id`] — `menu:<path>#<row key>`.
//! * [`Menu::step_navigable`] — the model-side mirror of
//!   [`crate::focus::FocusState::move_within`]: same wrap, same entry-from-outside rule.
//!   The sidecar asserts the two agree, so neither can drift.
//!
//! # Identity: derived, path-based, rebuild-stable
//!
//! A row's [`FocusId`] is derived from its menu's path and its own key, so
//! rebuilding an equivalent menu produces the same ids and the focus model
//! can hold a position across a `view` rebuild.
//!
//! Attaching a child menu **rebases** its path: `Menu::new(parent, rows)`
//! rewrites every submenu child's [`MenuId`] to `<parent path>/<row key>`,
//! recursively. Whatever id the child was built with is discarded. That is
//! the whole point — two callers mounting the same child value under
//! different parents get different, collision-free identities, and the same
//! parent mounting an equivalent child twice gets the *same* identity both
//! times. [`submenu_id`] is exposed so the cascade can compute a child's
//! id from a parent path and a key without holding the child.
//!
//! # Where the model stops and `INC-16` begins
//!
//! The model owns **structure and selection**. It does not own **openness**.
//!
//! A [`RowKind::Submenu`] carries its child [`Menu`] and exposes
//! [`Row::child_id`], but there is no `is_open` anywhere in this file.
//! Which branch of the cascade is open is `INC-16`'s state, for three
//! reasons that the model cannot satisfy:
//!
//! 1. **Mutual exclusion spans siblings.** `UIF-FMENU-002` says moving to
//!    a sibling submenu row *replaces* the open child branch. A child
//!    cannot know its siblings, so an `open` flag on the row could not
//!    enforce that without a back-reference to the parent.
//! 2. **Opening a child must not mutate the parent.** The model is a value
//!    that gets compared, cloned and rebuilt. If hovering a row dirtied the
//!    parent menu, every hover would be a rebuild of the tree above it.
//! 3. **Dismissal is tree-wide.** `UIF-CMENU-002` closes the whole
//!    hierarchy from outside; that is a statement about the cascade, not
//!    about any row.
//!
//! So: the model answers *"what could open here, and under what id"*. The
//! cascade answers *"what is open now"*. The one thing the model does say
//! about dismissal is per-row and static — the [`ClosePolicy`] below.
//!
//! # Close policy is per row, not inferred from kind
//!
//! [`ClosePolicy`] is stored explicitly on every row. Each kind has a
//! documented default that its constructor applies — Action closes
//! (`UIF-FMENU-003`: "Emit their command and close the entire menu tree
//! immediately"), Radio closes, Check **stays open** so several toggles
//! can be flipped in one visit, Submenu stays open — and the caller
//! overrides it per row with [`Row::stays_open`] / [`Row::closes_menu`]
//! (or [`Row::close_on_toggle`] for the checkable case the spec calls out).
//!
//! Storing rather than deriving matters because the spec itself makes the
//! checkable's behaviour configurable: a policy read off the kind could
//! not express "this particular toggle closes the menu".
//!
//! [`Menu::activate`] is the single entry point. It performs the state
//! change and returns an [`Activation`] carrying the effect and whether the
//! tree should close. Non-activatable rows — heading, separator, disabled —
//! return [`ActivationEffect::Absorbed`] with `closes_menu: false`, which
//! is `UIF-CMENU-002`'s "absorbed without dismissing the menu" written as
//! a return value rather than left to the click handler's good intentions.
//!
//! # The radio-group selection rule
//!
//! A radio group is a set of [`RowKind::Radio`] rows sharing a
//! [`RadioGroupId`]. The rule [`Menu::select_radio`] enforces:
//!
//! * selecting a member sets it selected and clears every other member of
//!   the same group **atomically** — the cleared indices come back in the
//!   return value so the caller never has to diff;
//! * **re-selecting the already-selected member is a no-op on the
//!   selection** — it does not clear it. A menu radio models the current
//!   value of a setting, and the spec's `•` indicator always marks one
//!   member; a click that left the group with no selection would describe a
//!   state the setting cannot be in. The activation still reports
//!   `Selected { cleared: [] }` and still honours the row's close policy,
//!   so the caller sees one uniform outcome either way;
//! * groups are independent: selecting in one never touches another.
//!
//! Unit tests live in the sibling `model_tests.rs` sidecar.

use std::collections::HashMap;
use std::fmt;

use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::GroupId;

/// Namespace prefix every menu focus group carries.
///
/// A constant because the group id and the row focus id are built from it
/// and must agree: a focus id that does not start with its own group's
/// prefix is an id no lookup can resolve.
const GROUP_PREFIX: &str = "menu:";

/// Separator between a menu path and a row key inside a [`FocusId`].
const ROW_SEPARATOR: &str = "#";

/// Separator between a parent menu path and the key of the submenu row that
/// owns a child menu.
const CHILD_SEPARATOR: &str = "/";

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// A stable identifier for one menu card, carrying its full path in the tree.
///
/// The root menu's path is whatever the caller names it; a submenu's path is
/// derived from its parent (see [`submenu_id`]). Ids compare by content, so
/// a rebuilt menu claims the identity of the one it replaced.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MenuId(String);

impl MenuId {
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

impl fmt::Display for MenuId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for MenuId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for MenuId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// A stable identifier for a set of mutually exclusive [`RowKind::Radio`]
/// rows.
///
/// Named rather than positional: a group is "wrap mode", not "rows 3–5", so
/// the caller can select by meaning and the model can keep groups apart when
/// one menu carries several settings.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RadioGroupId(String);

impl RadioGroupId {
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

impl fmt::Display for RadioGroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for RadioGroupId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for RadioGroupId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// The id a submenu child gets under `parent` when it is attached to the
/// row keyed `key`.
///
/// Exposed as a free function so `INC-16` can name a child branch from the
/// parent's path and the row key alone — which is exactly what a cascade
/// needs when it stores "the branch open under row 8 of the root menu"
/// without holding the child value.
#[must_use]
pub fn submenu_id(parent: &MenuId, key: &str) -> MenuId {
    MenuId::new(format!("{parent}{CHILD_SEPARATOR}{key}"))
}

// ---------------------------------------------------------------------------
// Row kinds and policies
// ---------------------------------------------------------------------------

/// What a row is.
///
/// Each variant carries exactly the state that kind owns and nothing else,
/// which is why there is no `Option` soup on [`Row`]: a separator has no
/// checked flag to forget to clear, and a submenu has its child inline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// A plain command row: fires and closes (`UIF-FMENU-003`).
    Action,
    /// An independent on/off toggle with a `✓` leading slot.
    ///
    /// Stays open by default so several can be flipped in one visit
    /// (`UIF-FMENU-003`).
    Check {
        /// The current state of the toggle.
        checked: bool,
    },
    /// One member of a mutually exclusive choice, with a `•` leading slot.
    ///
    /// Selecting this row clears every other row sharing the same
    /// [`RadioGroupId`] (`UIF-FMENU-003`).
    Radio {
        /// The group this choice belongs to.
        group: RadioGroupId,
        /// Whether this member is the group's current selection.
        selected: bool,
    },
    /// A row that owns a child menu, with a `>` trailing slot.
    ///
    /// Carries the child's *structure*. Whether the child is **open** is
    /// `INC-16`'s cascade state, not the row's — see the
    /// [module docs](self#where-the-model-stops-and-inc-16-begins).
    Submenu {
        /// The child menu this row opens.
        child: Box<Menu>,
    },
    /// A non-interactive section label.
    ///
    /// Never focusable, never clickable, never counted as a group's
    /// interactive content.
    Heading,
    /// A `1px border.subtle` divider between semantic groups (§4.9.1).
    ///
    /// Non-interactive; also the boundary [`Menu::groups`] splits on.
    Separator,
}

/// Whether activating a row takes the menu with it.
///
/// Stored per row rather than derived from [`RowKind`], because the spec
/// makes the checkable's behaviour a caller option (`UIF-FMENU-003`) and
/// any rule that reads the kind cannot express that.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClosePolicy {
    /// The menu tree closes when this row activates.
    Close,
    /// The menu tree stays open when this row activates.
    StayOpen,
}

impl ClosePolicy {
    /// The policy a row of `kind` gets when the caller does not choose one.
    ///
    /// Action and Radio close; Check, Submenu, Heading and Separator stay
    /// open. See [`Row::close_policy`] for why each default is what it is.
    #[must_use]
    pub const fn default_for(kind: &RowKind) -> Self {
        match kind {
            RowKind::Action | RowKind::Radio { .. } => Self::Close,
            RowKind::Check { .. }
            | RowKind::Submenu { .. }
            | RowKind::Heading
            | RowKind::Separator => Self::StayOpen,
        }
    }
}

/// What the model says goes in a row's 16px leading slot.
///
/// A semantic answer, not a glyph: the renderer picks the character and the
/// token, the model says whether the slot carries a checkmark, a radio dot,
/// the row's own icon, or nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LeadingSlot {
    /// The `✓` of a [`RowKind::Check`] (its own state is
    /// [`Row::is_checked`]).
    Check,
    /// The `•` of a [`RowKind::Radio`] (its own state is
    /// [`Row::is_selected`]).
    Radio,
    /// The row's own icon, where the caller supplied one.
    Icon,
    /// An empty slot — reserved space with nothing drawn.
    None,
}

/// What the model says goes in a row's trailing slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrailingSlot {
    /// A keyboard shortcut hint, where the caller supplied one.
    Shortcut,
    /// The `>` of a submenu parent.
    Chevron,
    /// An empty slot.
    None,
}

// ---------------------------------------------------------------------------
// The row
// ---------------------------------------------------------------------------

/// One row of a menu.
///
/// Built through the kind constructors ([`Row::action`], [`Row::checkable`]
/// …) and refined with consuming modifiers (`.disabled()`, `.destructive()`,
/// `.closes_menu(…)`). Every modifier returns `Self` so a row reads as one
/// expression, and every property is readable back through a query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    key: String,
    label: String,
    shortcut: Option<String>,
    icon: Option<String>,
    enabled: bool,
    destructive: bool,
    close: ClosePolicy,
    kind: RowKind,
}

impl Row {
    /// A plain command row that fires and closes (`UIF-FMENU-003`).
    #[must_use]
    pub fn action(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(key, label, RowKind::Action)
    }

    /// A checkable toggle row with a `✓` leading slot.
    ///
    /// Defaults to [`ClosePolicy::StayOpen`] so the user can flip several
    /// toggles in one visit; call [`close_on_toggle`](Self::close_on_toggle)
    /// for the exception the spec allows.
    #[must_use]
    pub fn checkable(key: impl Into<String>, label: impl Into<String>, checked: bool) -> Self {
        Self::new(key, label, RowKind::Check { checked })
    }

    /// A radio-choice row in group `group`.
    #[must_use]
    pub fn radio(
        key: impl Into<String>,
        label: impl Into<String>,
        group: impl Into<RadioGroupId>,
        selected: bool,
    ) -> Self {
        Self::new(
            key,
            label,
            RowKind::Radio {
                group: group.into(),
                selected,
            },
        )
    }

    /// A submenu parent carrying `child`.
    ///
    /// The child's [`MenuId`] is rebased onto this row's path by
    /// [`Menu::new`]; the id `child` was built with does not survive
    /// attachment.
    #[must_use]
    pub fn submenu(key: impl Into<String>, label: impl Into<String>, child: Menu) -> Self {
        Self::new(
            key,
            label,
            RowKind::Submenu {
                child: Box::new(child),
            },
        )
    }

    /// A non-interactive section heading.
    ///
    /// Carries no key: a heading is never focusable, so there is no
    /// identity it needs to keep stable.
    #[must_use]
    pub fn heading(label: impl Into<String>) -> Self {
        Self::new(String::new(), label, RowKind::Heading)
    }

    /// A non-interactive `1px border.subtle` separator (§4.9.1).
    #[must_use]
    pub fn separator() -> Self {
        Self::new(String::new(), String::new(), RowKind::Separator)
    }

    fn new(key: impl Into<String>, label: impl Into<String>, kind: RowKind) -> Self {
        let close = ClosePolicy::default_for(&kind);
        Self {
            key: key.into(),
            label: label.into(),
            shortcut: None,
            icon: None,
            enabled: true,
            destructive: false,
            close,
            kind,
        }
    }

    // -- modifiers ---------------------------------------------------------

    /// Mark the row inert: it renders, absorbs the click, and does not
    /// close the menu (`UIF-CMENU-002`).
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Set the enabled flag explicitly.
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Flag the row destructive.
    ///
    /// Orthogonal to the kind: a destructive Action and a destructive
    /// Checkable are both expressible. The grouping rule that puts
    /// destructive rows last is checked by [`Menu::validate`].
    #[must_use]
    pub const fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// Set the destructive flag explicitly.
    #[must_use]
    pub const fn with_destructive(mut self, destructive: bool) -> Self {
        self.destructive = destructive;
        self
    }

    /// Attach a keyboard shortcut hint for the trailing slot.
    #[must_use]
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Attach an icon for the leading slot of an Action row.
    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Override this row's close policy.
    #[must_use]
    pub const fn with_close_policy(mut self, policy: ClosePolicy) -> Self {
        self.close = policy;
        self
    }

    /// Make the row leave the menu open when it activates.
    #[must_use]
    pub const fn stays_open(mut self) -> Self {
        self.close = ClosePolicy::StayOpen;
        self
    }

    /// Make the row close the menu tree when it activates.
    #[must_use]
    pub const fn closes_menu(mut self) -> Self {
        self.close = ClosePolicy::Close;
        self
    }

    /// The checkable case of [`closes_menu`](Self::closes_menu): the
    /// caller-configured close-on-toggle that `UIF-FMENU-003` permits.
    #[must_use]
    pub const fn close_on_toggle(self) -> Self {
        self.closes_menu()
    }

    // -- identity ----------------------------------------------------------

    /// The row's stable key within its menu.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The row's label text.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The keyboard shortcut hint, if any.
    #[must_use]
    pub fn shortcut(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// The leading-slot icon, if any.
    #[must_use]
    pub fn icon(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    /// This row's kind.
    #[must_use]
    pub const fn kind(&self) -> &RowKind {
        &self.kind
    }

    // -- predicates --------------------------------------------------------

    /// Whether the row's *kind* can carry a command.
    ///
    /// True for Action, Check, Radio and Submenu regardless of
    /// [`enabled`](Self::is_enabled) — a greyed-out action is still an
    /// action. The renderer uses this for the pointer cursor; traversal
    /// uses [`navigable`](Self::navigable).
    #[must_use]
    pub const fn interactive(&self) -> bool {
        match self.kind {
            RowKind::Action
            | RowKind::Check { .. }
            | RowKind::Radio { .. }
            | RowKind::Submenu { .. } => true,
            RowKind::Heading | RowKind::Separator => false,
        }
    }

    /// Whether activating this row does something.
    #[must_use]
    pub const fn activatable(&self) -> bool {
        self.interactive() && self.enabled
    }

    /// Whether `Up` / `Down` may stop on this row (`UIF-CMENU-003`).
    ///
    /// Headings and separators are excluded by kind, disabled rows by the
    /// flag. This is the predicate [`Menu::focus_members`] builds the focus
    /// group from, so a row that is not navigable is never registered at
    /// all rather than registered and skipped.
    #[must_use]
    pub const fn navigable(&self) -> bool {
        self.activatable()
    }

    /// Whether the row is enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Whether the row is a disabled placeholder (§4.9.2).
    #[must_use]
    pub const fn is_disabled(&self) -> bool {
        !self.enabled
    }

    /// Whether the row is flagged destructive (§4.9.1).
    #[must_use]
    pub const fn is_destructive(&self) -> bool {
        self.destructive
    }

    /// Whether this row is a section heading.
    #[must_use]
    pub const fn is_heading(&self) -> bool {
        matches!(self.kind, RowKind::Heading)
    }

    /// Whether this row is a separator.
    #[must_use]
    pub const fn is_separator(&self) -> bool {
        matches!(self.kind, RowKind::Separator)
    }

    /// Whether this row is a checkable toggle.
    #[must_use]
    pub const fn is_checkable(&self) -> bool {
        matches!(self.kind, RowKind::Check { .. })
    }

    /// Whether this row is a radio member.
    #[must_use]
    pub const fn is_radio(&self) -> bool {
        matches!(self.kind, RowKind::Radio { .. })
    }

    /// Whether this row opens a submenu.
    #[must_use]
    pub const fn is_submenu_parent(&self) -> bool {
        matches!(self.kind, RowKind::Submenu { .. })
    }

    /// The toggle state of a [`RowKind::Check`]; `false` for every other
    /// kind.
    #[must_use]
    pub const fn is_checked(&self) -> bool {
        match self.kind {
            RowKind::Check { checked } => checked,
            _ => false,
        }
    }

    /// The selection state of a [`RowKind::Radio`]; `false` for every
    /// other kind.
    #[must_use]
    pub const fn is_selected(&self) -> bool {
        match self.kind {
            RowKind::Radio { selected, .. } => selected,
            _ => false,
        }
    }

    /// The group a radio member belongs to.
    #[must_use]
    pub fn radio_group(&self) -> Option<&RadioGroupId> {
        match &self.kind {
            RowKind::Radio { group, .. } => Some(group),
            _ => None,
        }
    }

    /// The child menu this submenu parent carries.
    #[must_use]
    pub fn child(&self) -> Option<&Menu> {
        match &self.kind {
            RowKind::Submenu { child } => Some(child),
            _ => None,
        }
    }

    /// The [`MenuId`] of this submenu parent's child.
    ///
    /// This is the handle `INC-16` keys its open/closed state on. The model
    /// deliberately offers the *identity* of the child rather than its
    /// openness — see the
    /// [module docs](self#where-the-model-stops-and-inc-16-begins).
    #[must_use]
    pub fn child_id(&self) -> Option<&MenuId> {
        self.child().map(Menu::id)
    }

    // -- slots -------------------------------------------------------------

    /// What belongs in this row's leading slot.
    #[must_use]
    pub fn leading_slot(&self) -> LeadingSlot {
        match self.kind {
            RowKind::Check { .. } => LeadingSlot::Check,
            RowKind::Radio { .. } => LeadingSlot::Radio,
            RowKind::Action if self.icon.is_some() => LeadingSlot::Icon,
            RowKind::Action | RowKind::Submenu { .. } | RowKind::Heading | RowKind::Separator => {
                LeadingSlot::None
            }
        }
    }

    /// What belongs in this row's trailing slot.
    ///
    /// A submenu's chevron outranks a shortcut hint: the chevron is what
    /// says the row opens something, and a row cannot be both a leaf with a
    /// shortcut and a branch.
    #[must_use]
    pub fn trailing_slot(&self) -> TrailingSlot {
        if self.is_submenu_parent() {
            TrailingSlot::Chevron
        } else if self.shortcut.is_some() {
            TrailingSlot::Shortcut
        } else {
            TrailingSlot::None
        }
    }

    // -- policy ------------------------------------------------------------

    /// This row's close policy.
    ///
    /// The default for the kind is applied at construction
    /// ([`ClosePolicy::default_for`]): Action and Radio close
    /// (`UIF-FMENU-003`), Check stays open so several toggles fit in one
    /// visit, Submenu stays open because opening a branch dismisses
    /// nothing. The caller overrides per row.
    #[must_use]
    pub const fn close_policy(&self) -> ClosePolicy {
        self.close
    }

    /// Whether activating this row would close the menu tree.
    ///
    /// `false` for anything that cannot activate: a disabled row absorbs
    /// the click and the menu stays (`UIF-CMENU-002`).
    #[must_use]
    pub const fn closes_menu_on_activate(&self) -> bool {
        self.activatable() && matches!(self.close, ClosePolicy::Close)
    }

    // -- internal mutation -------------------------------------------------

    fn set_checked(&mut self, checked: bool) -> Option<bool> {
        match &mut self.kind {
            RowKind::Check { checked: current } => {
                let previous = *current;
                *current = checked;
                Some(previous)
            }
            _ => None,
        }
    }

    fn set_selected(&mut self, selected: bool) -> bool {
        match &mut self.kind {
            RowKind::Radio {
                selected: current, ..
            } => {
                let previous = *current;
                *current = selected;
                previous != selected
            }
            _ => false,
        }
    }

    fn select_self(&mut self) {
        let _ = self.set_selected(true);
    }

    /// Mutable access to the child menu, used by the path-binding pass.
    fn child_mut(&mut self) -> Option<&mut Menu> {
        match &mut self.kind {
            RowKind::Submenu { child } => Some(child),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Groups
// ---------------------------------------------------------------------------

/// A run of rows between separators — one of §4.9.1's "functional groups".
///
/// The separator itself is the boundary and belongs to no group. A group
/// records which of its rows are interactive and which of those are
/// destructive so the ordering rule and the renderer both read the same
/// partition rather than recomputing it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowGroup {
    /// First row index, inclusive.
    pub start: usize,
    /// Last row index, exclusive.
    pub end: usize,
    /// Interactive rows inside this run, in order.
    pub interactive: Vec<usize>,
    /// Destructive interactive rows inside this run, in order.
    pub destructive: Vec<usize>,
}

impl RowGroup {
    /// How many rows this group spans, heading included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Whether the group spans no rows (it cannot, but `len` without
    /// `is_empty` is a lint).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether the group contains any interactive row.
    #[must_use]
    pub fn has_interactive(&self) -> bool {
        !self.interactive.is_empty()
    }

    /// Whether the group contains a destructive interactive row.
    #[must_use]
    pub fn has_destructive(&self) -> bool {
        !self.destructive.is_empty()
    }

    /// Whether the group mixes destructive and non-destructive interactive
    /// rows, which §4.9.1 forbids.
    #[must_use]
    pub fn mixes_destructive(&self) -> bool {
        self.has_destructive() && self.destructive.len() != self.interactive.len()
    }
}

/// The current contents of one radio group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadioGroupInfo {
    /// The group's id.
    pub id: RadioGroupId,
    /// Every member row index, in menu order.
    pub members: Vec<usize>,
    /// The selected member's row index, if the group has one.
    pub selected: Option<usize>,
}

impl RadioGroupInfo {
    /// Whether `row` is the group's current selection.
    #[must_use]
    pub fn is_selected(&self, row: usize) -> bool {
        self.selected == Some(row)
    }
}

// ---------------------------------------------------------------------------
// Activation
// ---------------------------------------------------------------------------

/// What happened when a row was activated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Activation {
    /// The row index that was activated.
    pub row: usize,
    /// The state change the model made.
    pub effect: ActivationEffect,
    /// Whether the menu tree should close.
    pub closes_menu: bool,
}

impl Activation {
    /// Whether the activation changed nothing and merely consumed the click
    /// (`UIF-CMENU-002`).
    #[must_use]
    pub fn is_absorbed(&self) -> bool {
        matches!(self.effect, ActivationEffect::Absorbed)
    }
}

/// The effect of activating a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActivationEffect {
    /// An Action fired its command.
    Command,
    /// A checkable flipped to `checked` (`UIF-FMENU-003`).
    Toggled {
        /// The toggle's new state.
        checked: bool,
    },
    /// A radio member was selected and its siblings cleared atomically.
    Selected {
        /// The group whose selection changed.
        group: RadioGroupId,
        /// Member rows that were selected before and are now cleared, in
        /// menu order. Empty when the re-selected member already held the
        /// selection.
        cleared: Vec<usize>,
    },
    /// A submenu parent was activated: the cascade should open `child`.
    OpenSubmenu {
        /// The child's (path-derived) id.
        child: MenuId,
    },
    /// Nothing happened: the click is consumed and the menu must stay open.
    ///
    /// This is what a heading, a separator or a disabled row produces
    /// (`UIF-CMENU-002`).
    Absorbed,
}

/// What is wrong with a menu that should not be shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuViolation {
    /// An interactive row has no key, so it cannot have a stable
    /// [`FocusId`].
    MissingKey {
        /// The offending row index.
        row: usize,
        /// Its label, to make the failure findable.
        label: String,
    },
    /// Two interactive rows in one menu share a key, so they would claim
    /// the same [`FocusId`].
    DuplicateKey {
        /// The later row.
        row: usize,
        /// The key it shares.
        key: String,
        /// The first row that claimed it.
        first: usize,
    },
    /// A destructive row sits in a group that is not the final interactive
    /// group, so §4.9.1's "destructive last" rule is broken.
    DestructiveNotLast {
        /// The destructive row.
        row: usize,
        /// The group it sits in.
        group: usize,
    },
    /// A group holds both destructive and non-destructive interactive
    /// rows (§4.9.1 keeps them in separate groups).
    MixedGroup {
        /// The group index.
        group: usize,
        /// A destructive row inside it.
        row: usize,
    },
    /// More than one member of a radio group is selected, so the `•`
    /// indicator would mark two rows.
    RadioOverSelected {
        /// The group.
        group: RadioGroupId,
        /// Every member currently marked selected.
        rows: Vec<usize>,
    },
}

// ---------------------------------------------------------------------------
// The menu
// ---------------------------------------------------------------------------

/// One menu card: an ordered list of rows under a stable path.
///
/// A value. Clone it, compare it, rebuild it every frame — the focus ids it
/// produces depend only on its path and its row keys, so an equivalent
/// rebuild is the same menu as far as the focus model is concerned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    id: MenuId,
    rows: Vec<Row>,
}

impl Menu {
    /// Build a menu from `rows` and bind every submenu child to its path.
    ///
    /// Binding is recursive: a child's id becomes
    /// `<this path>/<row key>` and the child binds its own children the
    /// same way, so the whole subtree's identity is fixed by the root's id
    /// and the keys along the way.
    #[must_use]
    pub fn new(id: impl Into<MenuId>, rows: Vec<Row>) -> Self {
        let mut menu = Self {
            id: id.into(),
            rows,
        };
        menu.bind_children();
        menu
    }

    /// A menu with no rows.
    #[must_use]
    pub fn empty(id: impl Into<MenuId>) -> Self {
        Self::new(id, Vec::new())
    }

    /// This menu's path-derived id.
    #[must_use]
    pub fn id(&self) -> &MenuId {
        &self.id
    }

    /// Every row, in display order.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// The row at `index`.
    #[must_use]
    pub fn row(&self, index: usize) -> Option<&Row> {
        self.rows.get(index)
    }

    /// How many rows the card holds, furniture included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the card holds no rows at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Rebase this menu's identity onto `id` and re-bind its children.
    fn rebase(&mut self, id: MenuId) {
        self.id = id;
        self.bind_children();
    }

    fn bind_children(&mut self) {
        let parent = self.id.clone();
        for row in &mut self.rows {
            let key = row.key().to_owned();
            if let Some(child) = row.child_mut() {
                child.rebase(submenu_id(&parent, &key));
            }
        }
    }

    // -- focus identity ----------------------------------------------------

    /// The [`GroupId`] this menu's navigable rows register under.
    #[must_use]
    pub fn group_id(&self) -> GroupId {
        GroupId::new(format!("{GROUP_PREFIX}{}", self.id))
    }

    /// The [`FocusId`] of the row at `index`.
    #[must_use]
    pub fn focus_id(&self, index: usize) -> Option<FocusId> {
        self.rows.get(index).map(|row| self.row_focus_id(row))
    }

    /// The [`FocusId`] a row would carry in this menu.
    ///
    /// Derived purely from the menu path and the row key, so two equivalent
    /// rebuilds produce the same id and the focus model's slot-preserving
    /// fall-forward can hold a position across a `view` rebuild.
    #[must_use]
    pub fn row_focus_id(&self, row: &Row) -> FocusId {
        FocusId::new(format!("{}{ROW_SEPARATOR}{}", self.group_id(), row.key()))
    }

    /// The row index a [`FocusId`] names in this menu.
    #[must_use]
    pub fn index_of_focus_id(&self, id: &FocusId) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| !row.key().is_empty() && self.row_focus_id(row) == *id)
    }

    // -- navigability ------------------------------------------------------

    /// The indices `Up` / `Down` may stop on, in order (§4.9.3
    /// `UIF-CMENU-003`).
    #[must_use]
    pub fn navigable_indices(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.navigable())
            .map(|(index, _)| index)
            .collect()
    }

    /// The [`FocusId`]s of the navigable rows, in traversal order.
    #[must_use]
    pub fn navigable_focus_ids(&self) -> Vec<FocusId> {
        self.rows
            .iter()
            .filter(|row| row.navigable())
            .map(|row| self.row_focus_id(row))
            .collect()
    }

    /// The roster to register into [`crate::focus::FocusState`] for this card.
    ///
    /// Contains **only** navigable rows, all of them enabled. Headings,
    /// separators and disabled rows are absent rather than registered
    /// disabled — see the
    /// [module docs](self#mapping-onto-focusstate-exclusion-not-registration-and-disabled).
    #[must_use]
    pub fn focus_members(&self) -> Vec<FocusMember> {
        self.rows
            .iter()
            .filter(|row| row.navigable())
            .map(|row| FocusMember::new(self.row_focus_id(row)))
            .collect()
    }

    /// The first navigable row index — where `Right` into a freshly opened
    /// submenu lands.
    #[must_use]
    pub fn first_navigable(&self) -> Option<usize> {
        self.rows.iter().position(Row::navigable)
    }

    /// The last navigable row index — where `Up` entering from above lands.
    #[must_use]
    pub fn last_navigable(&self) -> Option<usize> {
        self.rows.iter().rposition(Row::navigable)
    }

    /// Move through the navigable rows by `delta`, wrapping at both ends.
    ///
    /// The model-side mirror of [`crate::focus::FocusState::move_within`]: same wrap, and
    /// the same entry rule when `current` is not on the list — a positive
    /// delta enters at the first row, a negative one at the last. `delta`
    /// of `0` leaves the position alone.
    ///
    /// `current` is a row index, not a position in the navigable list, so
    /// the caller never has to translate.
    #[must_use]
    pub fn step_navigable(&self, current: Option<usize>, delta: i32) -> Option<usize> {
        if delta == 0 {
            return current
                .filter(|index| self.rows.get(*index).is_some_and(|row| row.navigable()));
        }

        let order = self.navigable_indices();
        let len = i64::try_from(order.len()).ok()?;
        if len == 0 {
            return None;
        }

        let position = current.and_then(|current| order.iter().position(|index| *index == current));
        let target = match position {
            Some(position) => {
                let position = i64::try_from(position).ok()?;
                (position + i64::from(delta)).rem_euclid(len)
            }
            None if delta > 0 => 0,
            None => len - 1,
        };

        usize::try_from(target)
            .ok()
            .and_then(|index| order.get(index).copied())
    }

    // -- grouping ----------------------------------------------------------

    /// The §4.9.1 functional groups: maximal runs of rows between
    /// separators.
    #[must_use]
    pub fn groups(&self) -> Vec<RowGroup> {
        let mut groups = Vec::new();
        let mut start = 0usize;

        for (index, row) in self.rows.iter().enumerate() {
            if row.is_separator() {
                if index > start {
                    groups.push(self.make_group(start, index));
                }
                start = index + 1;
            }
        }

        if self.rows.len() > start {
            groups.push(self.make_group(start, self.rows.len()));
        }

        groups
    }

    fn make_group(&self, start: usize, end: usize) -> RowGroup {
        let slice = &self.rows[start..end];
        RowGroup {
            start,
            end,
            interactive: slice
                .iter()
                .enumerate()
                .filter(|(_, row)| row.interactive())
                .map(|(offset, _)| start + offset)
                .collect(),
            destructive: slice
                .iter()
                .enumerate()
                .filter(|(_, row)| row.interactive() && row.is_destructive())
                .map(|(offset, _)| start + offset)
                .collect(),
        }
    }

    // -- radio groups ------------------------------------------------------

    /// Every radio group this menu carries, in first-appearance order.
    #[must_use]
    pub fn radio_group_ids(&self) -> Vec<RadioGroupId> {
        let mut ids = Vec::new();
        for row in &self.rows {
            if let Some(group) = row.radio_group()
                && !ids.contains(group)
            {
                ids.push(group.clone());
            }
        }
        ids
    }

    /// The current contents of one radio group.
    #[must_use]
    pub fn radio_group(&self, group: &RadioGroupId) -> Option<RadioGroupInfo> {
        let members = self.radio_members(group);
        if members.is_empty() {
            return None;
        }

        Some(RadioGroupInfo {
            id: group.clone(),
            selected: self.radio_selection_in(&members),
            members,
        })
    }

    /// The selected member's row index in `group`.
    #[must_use]
    pub fn radio_selection(&self, group: &RadioGroupId) -> Option<usize> {
        self.radio_selection_in(&self.radio_members(group))
    }

    /// Every member row index of `group`, in menu order.
    #[must_use]
    pub fn radio_members(&self, group: &RadioGroupId) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.radio_group() == Some(group))
            .map(|(index, _)| index)
            .collect()
    }

    fn radio_selection_in(&self, members: &[usize]) -> Option<usize> {
        members
            .iter()
            .copied()
            .find(|index| self.rows[*index].is_selected())
    }

    /// Select the radio member at `index` and clear its siblings.
    ///
    /// Returns the indices that were cleared. Re-selecting the member that
    /// already holds the selection clears nothing and returns an empty vec —
    /// the group never ends up with no selection, because the `•` indicator
    /// describes the current value of a setting that always has one.
    ///
    /// Returns `None` when `index` is out of range or is not a radio row.
    pub fn select_radio(&mut self, index: usize) -> Option<Vec<usize>> {
        let group = self.rows.get(index).and_then(Row::radio_group).cloned()?;

        Some(self.apply_radio_selection(&group, index))
    }

    fn apply_radio_selection(&mut self, group: &RadioGroupId, index: usize) -> Vec<usize> {
        let mut cleared = Vec::new();
        for (row, item) in self.rows.iter_mut().enumerate() {
            if row == index {
                continue;
            }
            if item.radio_group() == Some(group) && item.set_selected(false) {
                cleared.push(row);
            }
        }
        self.rows[index].select_self();
        cleared
    }

    // -- activation --------------------------------------------------------

    /// Activate the row at `index` and report what changed.
    ///
    /// The single entry point for "the user picked this row". The model
    /// makes the state change and returns an [`Activation`] describing it,
    /// so `INC-16` never has to re-read the tree to find out what just
    /// happened.
    ///
    /// Non-activatable rows produce [`ActivationEffect::Absorbed`] with
    /// `closes_menu: false` — `UIF-CMENU-002`'s absorbed click. Returns
    /// `None` only when `index` is out of range.
    pub fn activate(&mut self, index: usize) -> Option<Activation> {
        if index >= self.rows.len() {
            return None;
        }
        if !self.rows[index].activatable() {
            return Some(Activation {
                row: index,
                effect: ActivationEffect::Absorbed,
                closes_menu: false,
            });
        }

        let closes_menu = self.rows[index].closes_menu_on_activate();
        let effect = if self.rows[index].is_checkable() {
            let next = !self.rows[index].is_checked();
            let _ = self.rows[index].set_checked(next);
            ActivationEffect::Toggled { checked: next }
        } else if let Some(group) = self.rows[index].radio_group().cloned() {
            let cleared = self.apply_radio_selection(&group, index);
            ActivationEffect::Selected { group, cleared }
        } else if let Some(child) = self.rows[index].child() {
            ActivationEffect::OpenSubmenu {
                child: child.id().clone(),
            }
        } else {
            ActivationEffect::Command
        };

        Some(Activation {
            row: index,
            effect,
            closes_menu,
        })
    }

    /// Set a checkable row's state directly, without activating it.
    ///
    /// For callers syncing the model to a setting from outside the menu.
    /// Returns the previous value, or `None` when `index` is not a
    /// checkable row.
    pub fn set_checked(&mut self, index: usize, checked: bool) -> Option<bool> {
        self.rows.get_mut(index)?.set_checked(checked)
    }

    // -- validation --------------------------------------------------------

    /// Whether the menu satisfies every construction-time rule.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }

    /// Check the §4.9.1 ordering rule and the identity rules.
    ///
    /// Catches, in one pass:
    ///
    /// * an interactive row with no key, or two sharing a key — either one
    ///   breaks [`FocusId`] stability;
    /// * a destructive row outside the final interactive group, and a
    ///   group that mixes destructive with non-destructive interactive
    ///   rows (§4.9.1 puts destructive actions in the final group);
    /// * a radio group with more than one member selected.
    ///
    /// Non-interactive rows are exempt from the destructive rule: a
    /// heading titled "Danger" is not a destructive action. Children are
    /// checked by calling `validate` on them — a parent's verdict says
    /// nothing about its subtree.
    #[must_use]
    pub fn validate(&self) -> Vec<MenuViolation> {
        let mut violations = Vec::new();
        violations.extend(self.validate_keys());
        violations.extend(self.validate_destructive_order());
        violations.extend(self.validate_radio_groups());
        violations
    }

    fn validate_keys(&self) -> Vec<MenuViolation> {
        let mut violations = Vec::new();
        let mut seen: HashMap<&str, usize> = HashMap::new();

        for (index, row) in self.rows.iter().enumerate() {
            if !row.interactive() {
                continue;
            }
            if row.key().is_empty() {
                violations.push(MenuViolation::MissingKey {
                    row: index,
                    label: row.label().to_owned(),
                });
                continue;
            }
            if let Some(first) = seen.insert(row.key(), index) {
                violations.push(MenuViolation::DuplicateKey {
                    row: index,
                    key: row.key().to_owned(),
                    first,
                });
            }
        }

        violations
    }

    fn validate_destructive_order(&self) -> Vec<MenuViolation> {
        let groups = self.groups();
        let last_interactive = groups.iter().rposition(RowGroup::has_interactive);
        let mut violations = Vec::new();

        for (group_index, group) in groups.iter().enumerate() {
            if !group.has_destructive() {
                continue;
            }
            if Some(group_index) != last_interactive {
                violations.extend(group.destructive.iter().map(|row| {
                    MenuViolation::DestructiveNotLast {
                        row: *row,
                        group: group_index,
                    }
                }));
            }
            if group.mixes_destructive() {
                violations.extend(
                    group
                        .destructive
                        .iter()
                        .map(|row| MenuViolation::MixedGroup {
                            group: group_index,
                            row: *row,
                        }),
                );
            }
        }

        violations
    }

    fn validate_radio_groups(&self) -> Vec<MenuViolation> {
        let mut violations = Vec::new();

        for group in self.radio_group_ids() {
            let selected: Vec<usize> = self
                .rows
                .iter()
                .enumerate()
                .filter(|(_, row)| row.radio_group() == Some(&group) && row.is_selected())
                .map(|(index, _)| index)
                .collect();
            if selected.len() > 1 {
                violations.push(MenuViolation::RadioOverSelected {
                    group: group.clone(),
                    rows: selected,
                });
            }
        }

        violations
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod model_tests;
