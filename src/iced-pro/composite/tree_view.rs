//! The tree model — expansion, selection, the shift anchor, and the
//! keyboard choreography (`UIF-COMP-006`, decision **D-8**).
//!
//! # The binding split, made structural
//!
//! §3.0.4.1 divides state into what the application owns and what the
//! widget owns, and the tree is the clearest case in the framework of
//! why the two must not be conflated:
//!
//! * **The structure is `Controlled`.** Nodes, parent links and sibling
//!   order come from the host and are *borrowed* on every call. Nothing
//!   in this module mutates a [`TreeStructure`], and the type has no
//!   public mutator to misuse.
//! * **Expansion is `Ephemeral`.** Which folders are open is the
//!   framework's, held in [`Ephemeral<BTreeSet<Id>>`] and never
//!   round-tripped. The host learns a folder opened by receiving an
//!   [`TreeEvent::ExpandToggle`], not by reading a field back — which
//!   is exactly the rule in [`Controlled`]'s own docs: if ephemeral
//!   state becomes something the app must read, it was never ephemeral.
//!
//! [`Controlled`]: crate::binding::Controlled
//! [`GestureSlot`]: crate::binding::GestureSlot
//!
//! # The row list is the domain
//!
//! Every keyboard rule is a statement about *visible* rows, so
//! [`visible_rows`] is the function everything else works from: a
//! depth-tagged, DFS-ordered list of the rows the expansion set makes
//! visible. Selection ranges, focus movement, the anchor's position and
//! the virtualised viewport all read the same list, which is what keeps
//! "select from the anchor to here" meaning the same thing to the
//! keyboard, the mouse and the scrollbar.
//!
//! # The anchor is an id, never an index
//!
//! `Shift`-select is defined against the **anchor**, and the whole
//! question is what happens when the world moves underneath it. Storing
//! an index answers that badly: a collapse shifts every row after it and
//! the next `Shift+Down` selects a range the user never dragged across.
//!
//! So the anchor is an id, resolved to a position at the moment of the
//! call ([`TreeView::resolve_anchor`]), in three steps:
//!
//! 1. The anchor itself is visible → its position.
//! 2. It is not visible (collapsed away) → the position of its
//!    **nearest visible ancestor**. The user's anchor is a *place in the
//!    tree*, and when its branch is folded the place it occupies on
//!    screen is where its parent is. Ranges stay contiguous and never
//!    jump to the top of the list.
//! 3. No visible ancestor either (the node was removed) → no anchor, and
//!    the range degenerates to the target row alone, which becomes the
//!    new anchor.
//!
//! Because the id is retained rather than cleared, re-expanding the
//! branch restores the exact original range behaviour. The sidecar walks
//! collapse → shift → re-expand → shift and asserts both halves.
//!
//! # Selection survives structural change because it is a set of ids
//!
//! A removed node drops out of `selected` on the next
//! [`sync_to`](TreeView::sync_to). A selected node whose parent is
//! collapsed stays selected — it is simply not on screen, and comes
//! back selected. A subtree re-parented keeps every selection in it,
//! because re-parenting changes links and not identities. Those three are
//! the cases the task of "keep selection in sync with the tree" actually
//! consists of, and each is a test rather than a hope.
//!
//! Focus is the one thing that cannot simply survive by id, because a
//! focused row that is not visible is a keyboard pointed at nothing.
//! When the focused row is collapsed away, focus moves to its **nearest
//! visible ancestor** — the node whose folding hid it — which is the
//! same rule the `Left` arrow already implements, so the two agree. If
//! no visible ancestor exists, focus falls back to the crate's
//! slot-preserving rule ([`focus`](crate::focus::FocusState)'s decision
//! 1): the row now occupying the ordinal slot, wrapping if the list
//! shrank past the end.
//!
//! # Auto-scroll yields to the pointer
//!
//! [`TreeView::scroll_into_view`] moves the viewport only when the
//! focused row is actually outside it, and returns `None` while a thumb
//! drag is open. The reason is the [`GestureSlot`] contract
//! [`scroll_view`](crate::composite::scroll_view) established: a drag
//! in flight owns the offset and will *commit* its own value when it
//! settles. An auto-scroll issued mid-drag is either overwritten by
//! that commit or overwrites the drag's live ticks, and either way the
//! thumb jumps out from under the pointer. Refusing is the only
//! non-losing move, and it costs nothing, because a keyboard user is not
//! holding a thumb.
//!
//! The same "smallest possible move" rule that stops the list
//! recentring on every arrow key is
//! [`VirtualMetrics::offset_to_reveal`](crate::composite::virtual_list::VirtualMetrics::offset_to_reveal).
//!
//! # Keyboard table
//!
//! | Key | Shift | Ctrl | Result |
//! |---|---|---|---|
//! | `Up` / `Down` | – | – | focus moves one visible row, **clamped** at both ends |
//! | `Up` / `Down` | ✓ | – | focus moves, selection becomes the range anchor → new focus |
//! | `Up` / `Down` | – | ✓ | focus moves only; the selection is untouched |
//! | `Left` | – | – | expanded and expandable → collapse; otherwise focus the parent |
//! | `Right` | – | – | collapsed and expandable → expand; otherwise focus the first child |
//! | `Home` / `End` | – | – | focus the first / last visible row |
//! | `Home` / `End` | ✓ | – | same, plus the range anchor → that row |
//! | `Enter` | – | – | `Activate(focused)` |
//! | `Space` | – | – | `ExtendedMulti`: toggle focused in the selection. `Single`: select it. Anchor = focused |
//! | `Ctrl+A` | – | ✓ | every visible row selected; anchor unchanged |
//! | `ContextMenu` | – | – | target outside the selection resets to it; inside preserves it |
//!
//! **Why the tree clamps where the menu wraps.**
//! [`FocusState::move_within`](crate::focus::FocusState::move_within)
//! wraps, and that is right for a menu: it is small, transient, and the
//! distance from the last row to the first is short. In a virtualised
//! tree `Up` at row zero would teleport the viewport to the bottom of a
//! hundred-thousand-row document — an expensive jump the user did not
//! ask for, with a long way back. The tree has `Home` and `End` for the
//! far ends, so wrapping buys nothing here and costs a scroll.
//!
//! **`Ctrl`+arrow is a focus move, not a selection edit.** §3B.6 names
//! `Ctrl` only against *clicks* and `Space`, and prints nothing for
//! `Ctrl`+arrow. Reading it as "toggle each row you pass over" would
//! make a held arrow key rewrite the selection on every repeat, so the
//! conservative reading is taken and recorded as a spec gap.
//!
//! Unit tests live in the sibling `tree_view_tests.rs` sidecar.

use crate::binding::{Commit, Ephemeral};
use crate::composite::virtual_list::VirtualMetrics;
use crate::focus::FocusMember;
use iced::Point;
use std::collections::BTreeSet;

/// The deepest tree this module will flatten.
///
/// **An invented value**, taken from the same bound the application's own
/// flattener carries. It exists to stop a cyclic `parent` link from
/// turning a `Down` press into a stack overflow, and no real workspace
/// reaches it.
pub const MAX_TREE_DEPTH: usize = 1000;

/// Whether more than one row may be selected at a time (§3B.6's
/// `Single | ExtendedMulti`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SelectionMode {
    /// One row at a time; every selection replaces the last.
    #[default]
    Single,
    /// `Ctrl` and `Shift` widen the set.
    ExtendedMulti,
}

impl SelectionMode {
    /// Whether this mode may hold more than one row.
    #[must_use]
    pub const fn is_multi(self) -> bool {
        matches!(self, Self::ExtendedMulti)
    }
}

/// Type alias for tree selection mode (`UIF-COMP-006`, `UIF-COMP-013`).
pub type TreeSelectionMode = SelectionMode;

/// The nodes and links of one tree, as the host owns them.
///
/// Built once from a flat `(id, parent)` list in document order and then
/// read-only. Sibling order is the order the host supplied, which is
/// where the application's sort rule (folders above files, then by name)
/// lives — the framework renders the order it is given and invents none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeStructure<Id> {
    nodes: Vec<Id>,
    children: std::collections::BTreeMap<Id, Vec<Id>>,
    parent: std::collections::BTreeMap<Id, Id>,
}

/// Written by hand rather than derived: the derive would demand
/// `Id: Default`, which an empty tree has no business requiring.
impl<Id> Default for TreeStructure<Id> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            children: std::collections::BTreeMap::new(),
            parent: std::collections::BTreeMap::new(),
        }
    }
}

impl<Id: Clone + Ord + core::fmt::Debug> TreeStructure<Id> {
    /// An empty structure.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from `(id, parent)` pairs in document order.
    ///
    /// A pair whose parent is never itself declared still links: the
    /// child records the parent, and the parent simply has no entry in
    /// [`nodes`](Self::nodes) of its own, which makes the child an
    /// orphan rather than a root. That is the honest reading — silently
    /// promoting orphans to roots would show the user nodes the host
    /// never said were top-level.
    #[must_use]
    pub fn from_pairs(pairs: &[(Id, Option<Id>)]) -> Self {
        let mut structure = Self::new();
        for (id, parent) in pairs {
            structure.nodes.push(id.clone());
            if let Some(parent) = parent {
                structure.parent.insert(id.clone(), parent.clone());
                structure
                    .children
                    .entry(parent.clone())
                    .or_default()
                    .push(id.clone());
            }
        }
        structure
    }

    /// Every declared node, in document order.
    #[must_use]
    pub fn nodes(&self) -> &[Id] {
        &self.nodes
    }

    /// Whether `id` is a declared node.
    #[must_use]
    pub fn contains(&self, id: &Id) -> bool {
        self.nodes.iter().any(|node| node == id)
    }

    /// A node's parent, if it has one.
    #[must_use]
    pub fn parent_of(&self, id: &Id) -> Option<&Id> {
        self.parent.get(id)
    }

    /// A node's children, in the order supplied.
    #[must_use]
    pub fn children_of(&self, id: &Id) -> &[Id] {
        self.children.get(id).map_or(&[], Vec::as_slice)
    }

    /// Whether this node has children and is therefore expandable.
    #[must_use]
    pub fn is_expandable(&self, id: &Id) -> bool {
        !self.children_of(id).is_empty()
    }

    /// The top-level nodes, in document order.
    #[must_use]
    pub fn roots(&self) -> Vec<&Id> {
        self.nodes
            .iter()
            .filter(|id| !self.parent.contains_key(*id))
            .collect()
    }

    /// A node's ancestors, nearest first.
    ///
    /// Bounded by [`MAX_TREE_DEPTH`] so a cyclic `parent` link returns
    /// what it walked rather than looping forever. A tree that hits the
    /// bound is malformed, and the truncation is the least-bad answer:
    /// the caller still gets the ancestors it can use.
    #[must_use]
    pub fn ancestors(&self, id: &Id) -> Vec<Id> {
        let mut chain = Vec::new();
        let mut cursor = self.parent_of(id).cloned();
        while let Some(current) = cursor {
            if chain.len() >= MAX_TREE_DEPTH {
                break;
            }
            cursor = self.parent_of(&current).cloned();
            chain.push(current);
        }
        chain
    }

    /// Whether `ancestor` is an ancestor of `id`.
    #[must_use]
    pub fn is_ancestor_of(&self, ancestor: &Id, id: &Id) -> bool {
        self.ancestors(id).contains(ancestor)
    }
}

/// One row the expansion set makes visible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibleRow<Id> {
    /// The node this row shows.
    pub id: Id,
    /// Its depth — `0` for a top-level node.
    pub depth: usize,
    /// Whether it has children and so owns a live disclosure chevron.
    pub expandable: bool,
    /// Whether those children are currently shown.
    pub expanded: bool,
}

/// Flatten the tree to the visible row list, DFS pre-order.
///
/// Iterative rather than recursive, with a visited set: a cyclic
/// `children` link would otherwise turn one `Down` press into a stack
/// overflow, and the depth bound alone does not stop a two-node cycle
/// from pushing forever before the bound is reached.
#[must_use]
pub fn visible_rows<Id: Clone + Ord + core::fmt::Debug>(
    structure: &TreeStructure<Id>,
    expanded: &BTreeSet<Id>,
) -> Vec<VisibleRow<Id>> {
    let mut rows: Vec<VisibleRow<Id>> = Vec::new();
    let mut visited: BTreeSet<Id> = BTreeSet::new();
    let mut stack: Vec<(Id, usize)> = structure
        .roots()
        .into_iter()
        .rev()
        .map(|id| (id.clone(), 0usize))
        .collect();

    while let Some((id, depth)) = stack.pop() {
        if depth > MAX_TREE_DEPTH || !visited.insert(id.clone()) {
            continue;
        }
        let children = structure.children_of(&id);
        let is_expanded = expanded.contains(&id);
        rows.push(VisibleRow {
            id: id.clone(),
            depth,
            expandable: !children.is_empty(),
            expanded: is_expanded,
        });
        if is_expanded {
            for child in children.iter().rev() {
                stack.push((child.clone(), depth + 1));
            }
        }
    }

    rows
}

/// Where a row sits in the visible list.
fn position_of<Id: PartialEq>(rows: &[VisibleRow<Id>], id: &Id) -> Option<usize> {
    rows.iter().position(|row| &row.id == id)
}

/// The keys the tree answers to (§3B.6's choreography, and `UIF-VER-003`'s
/// `TreeView` keyboard contract).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeKey {
    /// `Up`.
    Up,
    /// `Down`.
    Down,
    /// `Left` — collapse, or jump to the parent.
    Left,
    /// `Right` — expand, or jump to the first child.
    Right,
    /// `Home` — the first visible row.
    Home,
    /// `End` — the last visible row.
    End,
    /// `Enter` — activate the focused row.
    Activate,
    /// `Space` — toggle the focused row in the selection.
    ToggleSelect,
    /// `Ctrl+A` — select every visible row.
    SelectAll,
}

/// The modifier state carried with a key.
///
/// A struct rather than nine key variants because the modifiers are
/// orthogonal to the key, and a host reading a real key event has them
/// as independent bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// `Shift` held.
    pub shift: bool,
    /// `Ctrl` (or `Cmd`) held.
    pub ctrl: bool,
}

impl Modifiers {
    /// No modifiers.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            shift: false,
            ctrl: false,
        }
    }

    /// `Shift` held.
    #[must_use]
    pub const fn shift() -> Self {
        Self {
            shift: true,
            ctrl: false,
        }
    }

    /// `Ctrl` held.
    #[must_use]
    pub const fn ctrl() -> Self {
        Self {
            shift: false,
            ctrl: true,
        }
    }
}

/// What a tree operation tells the host happened (§3B.6's callbacks).
///
/// `Eq` is deliberately absent: [`Point`] is a float pair, and
/// a context-menu position is not something to compare for equality.
#[derive(Clone, Debug, PartialEq)]
pub enum TreeEvent<Id> {
    /// The selection changed; the whole set is handed over so the host
    /// never has to diff.
    SelectionChanged(BTreeSet<Id>),
    /// `Enter` or double-click on a row.
    Activate(Id),
    /// A node's expansion flipped, with the new state.
    ExpandToggle(Id, bool),
    /// The context menu opened for the current selection at a point.
    ContextMenu(BTreeSet<Id>, Point),
}

/// What a structural change cost.
///
/// Reported rather than silently absorbed, because a host that cannot see
/// what the tree dropped cannot tell "the user deselected" from "the
/// file was deleted underneath a selected row" — and those deserve
/// different log lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncReport<Id> {
    /// Selected nodes that no longer exist.
    pub dropped_selected: Vec<Id>,
    /// Expanded nodes that no longer exist, or lost their children.
    pub dropped_expanded: Vec<Id>,
    /// The anchor, if it was dropped.
    pub dropped_anchor: Option<Id>,
    /// The focused node, if it was lost.
    pub lost_focus: Option<Id>,
    /// Where focus landed instead.
    pub landed_focus: Option<Id>,
}

/// Hand-written: the derive would require `Id: Default`.
impl<Id> Default for SyncReport<Id> {
    fn default() -> Self {
        Self {
            dropped_selected: Vec::new(),
            dropped_expanded: Vec::new(),
            dropped_anchor: None,
            lost_focus: None,
            landed_focus: None,
        }
    }
}

impl<Id: PartialEq> SyncReport<Id> {
    /// Whether the change dropped nothing at all.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.dropped_selected.is_empty()
            && self.dropped_expanded.is_empty()
            && self.dropped_anchor.is_none()
            && self.lost_focus.is_none()
    }
}

/// The tree's own state: expansion, selection, anchor and focus.
///
/// The structure is never held — it arrives borrowed on every call,
/// which *is* the `Controlled` half of §3.0.4.1. What lives here is
/// the half the host must not own.
#[derive(Clone, Debug, Default)]
pub struct TreeView<Id> {
    expanded: Ephemeral<BTreeSet<Id>>,
    selected: BTreeSet<Id>,
    anchor: Option<Id>,
    focused: Option<Id>,
    focus_slot: Option<usize>,
    mode: SelectionMode,
}

impl<Id: Clone + Ord + core::fmt::Debug> TreeView<Id> {
    /// A tree in a given selection mode, with nothing expanded,
    /// selected or focused.
    #[must_use]
    pub fn new(mode: SelectionMode) -> Self {
        Self {
            expanded: Ephemeral::new(BTreeSet::new()),
            selected: BTreeSet::new(),
            anchor: None,
            focused: None,
            focus_slot: None,
            mode,
        }
    }

    /// The selection mode.
    #[must_use]
    pub const fn mode(&self) -> SelectionMode {
        self.mode
    }

    /// The expansion set — the `Ephemeral` half of the model.
    #[must_use]
    pub fn expanded(&self) -> &BTreeSet<Id> {
        self.expanded.get()
    }

    /// Whether `id` is expanded.
    #[must_use]
    pub fn is_expanded(&self, id: &Id) -> bool {
        self.expanded.get().contains(id)
    }

    /// The current selection.
    #[must_use]
    pub fn selected(&self) -> &BTreeSet<Id> {
        &self.selected
    }

    /// Whether `id` is selected.
    #[must_use]
    pub fn is_selected(&self, id: &Id) -> bool {
        self.selected.contains(id)
    }

    /// The anchor row, by id.
    #[must_use]
    pub fn anchor(&self) -> Option<&Id> {
        self.anchor.as_ref()
    }

    /// The focused row, by id.
    #[must_use]
    pub fn focused(&self) -> Option<&Id> {
        self.focused.as_ref()
    }

    /// The ordinal slot focus last occupied in the visible list.
    ///
    /// Kept so a structural change can put focus back where the user
    /// was, the same slot-preserving rule
    /// [`FocusState`](crate::focus::FocusState) applies to tab order.
    #[must_use]
    pub const fn focus_slot(&self) -> Option<usize> {
        self.focus_slot
    }

    // -- expansion ---------------------------------------------------------

    /// Drive a node's expansion to `open`, reporting the commit.
    ///
    /// Returns `None` when the flag already held that value, so a host
    /// persisting every commit is not written to twice for one no-op.
    pub fn set_expanded(&mut self, id: &Id, open: bool) -> Option<Commit<bool>> {
        let changed = if open {
            self.expanded.get_mut().insert(id.clone())
        } else {
            self.expanded.get_mut().remove(id)
        };
        changed.then(|| Commit::new(open))
    }

    /// Flip a node's expansion.
    pub fn toggle_expanded(&mut self, id: &Id) -> Commit<bool> {
        let open = !self.is_expanded(id);
        self.set_expanded(id, open)
            .unwrap_or_else(|| Commit::new(open))
    }

    // -- selection ---------------------------------------------------------

    /// Select one row and make it the anchor.
    #[must_use]
    pub fn select_single(&mut self, id: &Id) -> bool {
        let changed = self.selected.len() != 1 || !self.selected.contains(id);
        self.selected.clear();
        self.selected.insert(id.clone());
        self.anchor = Some(id.clone());
        changed
    }

    /// Toggle a row in the selection.
    ///
    /// In [`SelectionMode::Single`] a toggle *is* a select, because
    /// there is no set to add to. The anchor moves either way.
    #[must_use]
    pub fn toggle_selected(&mut self, id: &Id) -> bool {
        self.anchor = Some(id.clone());
        if !self.mode.is_multi() {
            return self.select_single(id);
        }
        if self.selected.remove(id) {
            true
        } else {
            self.selected.insert(id.clone());
            true
        }
    }

    /// Clear the selection. The anchor goes with it.
    pub fn clear_selection(&mut self) -> bool {
        let had = !self.selected.is_empty();
        self.selected.clear();
        self.anchor = None;
        had
    }

    /// Select the inclusive range between two positions of the visible
    /// list.
    ///
    /// Out-of-range positions clamp rather than failing, and an inverted
    /// range is read backwards, because the caller is a drag or an arrow
    /// key and neither is wrong for moving up instead of down.
    pub fn select_range(&mut self, rows: &[VisibleRow<Id>], from: usize, to: usize) -> bool {
        if rows.is_empty() {
            return false;
        }
        let last = rows.len() - 1;
        let start = from.min(last);
        let end = to.min(last);
        let (lo, hi) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };

        self.selected = rows[lo..=hi].iter().map(|row| row.id.clone()).collect();
        true
    }

    /// Select every visible row.
    pub fn select_all(&mut self, rows: &[VisibleRow<Id>]) -> bool {
        if !self.mode.is_multi() {
            return false;
        }
        let next: BTreeSet<Id> = rows.iter().map(|row| row.id.clone()).collect();
        let changed = next != self.selected;
        self.selected = next;
        changed
    }

    // -- focus -------------------------------------------------------------

    /// Focus the row at `index`, recording its slot.
    pub fn focus_index(&mut self, rows: &[VisibleRow<Id>], index: usize) -> Option<Id> {
        let row = rows.get(index)?;
        self.focused = Some(row.id.clone());
        self.focus_slot = Some(index);
        Some(row.id.clone())
    }

    /// Move focus by `delta` visible rows, **clamped** at both ends.
    ///
    /// With no focus, a positive delta enters at the first row and a
    /// negative one at the last, matching
    /// [`FocusState`](crate::focus::FocusState)'s entry convention.
    /// Clamping rather than wrapping is the tree's own decision — see the
    /// [module docs](self#keyboard-table).
    pub fn move_focus(&mut self, rows: &[VisibleRow<Id>], delta: i32) -> Option<Id> {
        if rows.is_empty() {
            return None;
        }
        let last = rows.len() - 1;
        let current = position_of(rows, self.focused.as_ref()?)?;
        let next = if delta >= 0 {
            current.saturating_add(delta as usize).min(last)
        } else {
            current.saturating_sub(delta.unsigned_abs() as usize)
        };
        self.focus_index(rows, next)
    }

    /// Move focus with no focused row yet: enter at the near end.
    pub fn enter_focus(&mut self, rows: &[VisibleRow<Id>], delta: i32) -> Option<Id> {
        if rows.is_empty() {
            return None;
        }
        let index = if delta >= 0 { 0 } else { rows.len() - 1 };
        self.focus_index(rows, index)
    }

    /// Resolve the anchor to a position in the visible list.
    ///
    /// The three-step rule is in the
    /// [module docs](self#the-anchor-is-an-id-never-an-index).
    #[must_use]
    pub fn resolve_anchor(
        &self,
        structure: &TreeStructure<Id>,
        rows: &[VisibleRow<Id>],
    ) -> Option<usize> {
        let anchor = self.anchor.as_ref()?;
        if let Some(position) = position_of(rows, anchor) {
            return Some(position);
        }
        structure
            .ancestors(anchor)
            .iter()
            .find_map(|ancestor| position_of(rows, ancestor))
    }

    // -- keyboard ----------------------------------------------------------

    /// Apply one key and report what the host must be told.
    ///
    /// `rows` is the current visible list from
    /// [`visible_rows`]. The
    /// returned events are the spec's callbacks; everything else the key
    /// did is readable through the accessors.
    #[must_use]
    pub fn handle_key(
        &mut self,
        key: TreeKey,
        mods: Modifiers,
        structure: &TreeStructure<Id>,
        rows: &[VisibleRow<Id>],
    ) -> Vec<TreeEvent<Id>> {
        let mut events = Vec::new();

        match key {
            TreeKey::Up | TreeKey::Down => {
                let delta = if key == TreeKey::Up { -1 } else { 1 };
                let moved = if self.focused.is_some() {
                    self.move_focus(rows, delta)
                } else {
                    self.enter_focus(rows, delta)
                };
                let Some(id) = moved else {
                    return events;
                };
                if mods.shift {
                    self.apply_shift_range(rows, structure);
                    events.push(TreeEvent::SelectionChanged(self.selected.clone()));
                } else if !mods.ctrl && !self.mode.is_multi() && self.select_single(&id) {
                    events.push(TreeEvent::SelectionChanged(self.selected.clone()));
                }
            }
            TreeKey::Home | TreeKey::End => {
                if rows.is_empty() {
                    return events;
                }
                let index = if key == TreeKey::Home {
                    0
                } else {
                    rows.len() - 1
                };
                let Some(id) = self.focus_index(rows, index) else {
                    return events;
                };
                if mods.shift {
                    self.apply_shift_range(rows, structure);
                    events.push(TreeEvent::SelectionChanged(self.selected.clone()));
                } else if !mods.ctrl && !self.mode.is_multi() && self.select_single(&id) {
                    events.push(TreeEvent::SelectionChanged(self.selected.clone()));
                }
            }
            TreeKey::Left => {
                let Some(current) = self.focused.clone() else {
                    return events;
                };
                if self.is_expanded(&current) && structure.is_expandable(&current) {
                    if let Some(commit) = self.set_expanded(&current, false) {
                        events.push(TreeEvent::ExpandToggle(current, *commit.value()));
                    }
                } else if let Some(parent) = structure.parent_of(&current)
                    && let Some(index) = position_of(rows, parent)
                {
                    self.focus_index(rows, index);
                }
            }
            TreeKey::Right => {
                let Some(current) = self.focused.clone() else {
                    return events;
                };
                if structure.is_expandable(&current) && !self.is_expanded(&current) {
                    if let Some(commit) = self.set_expanded(&current, true) {
                        events.push(TreeEvent::ExpandToggle(current, *commit.value()));
                    }
                } else if let Some(child) = structure.children_of(&current).first()
                    && let Some(index) = position_of(rows, child)
                {
                    self.focus_index(rows, index);
                }
            }
            TreeKey::Activate => {
                if let Some(id) = self.focused.clone() {
                    events.push(TreeEvent::Activate(id));
                }
            }
            TreeKey::ToggleSelect => {
                let Some(id) = self.focused.clone() else {
                    return events;
                };
                if self.toggle_selected(&id) {
                    events.push(TreeEvent::SelectionChanged(self.selected.clone()));
                }
            }
            TreeKey::SelectAll => {
                if self.select_all(rows) {
                    events.push(TreeEvent::SelectionChanged(self.selected.clone()));
                }
            }
        }

        events
    }

    /// The `Shift` rule: the selection becomes the range from the
    /// resolved anchor to the current focus.
    fn apply_shift_range(&mut self, rows: &[VisibleRow<Id>], structure: &TreeStructure<Id>) {
        let Some(focus) = self.focused.as_ref() else {
            return;
        };
        let Some(to) = position_of(rows, focus) else {
            return;
        };
        match self.resolve_anchor(structure, rows) {
            Some(from) => {
                self.select_range(rows, from, to);
            }
            None => {
                self.selected.clear();
                if let Some(row) = rows.get(to) {
                    self.selected.insert(row.id.clone());
                    self.anchor = Some(row.id.clone());
                }
            }
        }
    }

    /// The context-menu rule (§3B.6).
    ///
    /// A row outside the selection resets the selection to itself
    /// before the menu opens; a row inside it preserves the
    /// multi-selection, because the user is acting *on* the group.
    ///
    /// `SelectionChanged` is emitted only when the reset actually
    /// changed the set — a right-click on an already-single-selected row
    /// produces the menu and nothing else.
    #[must_use]
    pub fn context_menu(&mut self, target: &Id, at: Point, preserve: bool) -> Vec<TreeEvent<Id>> {
        let mut events = Vec::new();
        if !preserve && self.select_single(target) {
            events.push(TreeEvent::SelectionChanged(self.selected.clone()));
        }
        events.push(TreeEvent::ContextMenu(self.selected.clone(), at));
        events
    }

    /// The offset that brings the focused row into view, or `None` when
    /// no scroll should happen.
    ///
    /// `drag_active` is the thumb drag's
    /// [`is_active`](crate::composite::scroll_view::ThumbDrag::is_active)
    /// flag. While it is set this returns `None` unconditionally: the
    /// drag owns the offset and will commit its own value on release, so
    /// an auto-scroll here would fight it and the thumb would jump.
    #[must_use]
    pub fn scroll_into_view(
        &self,
        rows: &[VisibleRow<Id>],
        metrics: &VirtualMetrics,
        current: f32,
        drag_active: bool,
    ) -> Option<f32> {
        if drag_active {
            return None;
        }
        let index = position_of(rows, self.focused.as_ref()?)?;
        metrics.offset_to_reveal(index, current)
    }

    // -- structural change -------------------------------------------------

    /// Reconcile the model with a structure that changed underneath it.
    ///
    /// `rows` is the *new* visible list, computed from the new
    /// structure and the current expansion set. The report says what
    /// was dropped and where focus landed; nothing is silently discarded.
    pub fn sync_to(
        &mut self,
        structure: &TreeStructure<Id>,
        rows: &[VisibleRow<Id>],
    ) -> SyncReport<Id> {
        let mut report = SyncReport::default();

        let dropped: Vec<Id> = self
            .selected
            .iter()
            .filter(|id| !structure.contains(id))
            .cloned()
            .collect();
        for id in &dropped {
            self.selected.remove(id);
        }
        report.dropped_selected = dropped;

        let dropped_expanded: Vec<Id> = self
            .expanded
            .get()
            .iter()
            .filter(|id| !structure.contains(id) || !structure.is_expandable(id))
            .cloned()
            .collect();
        for id in &dropped_expanded {
            self.expanded.get_mut().remove(id);
        }
        report.dropped_expanded = dropped_expanded;

        if let Some(anchor) = self.anchor.clone()
            && !structure.contains(&anchor)
        {
            self.anchor = None;
            report.dropped_anchor = Some(anchor);
        }

        let slot = self.focus_slot;
        match self.focused.clone() {
            Some(id) if structure.contains(&id) => {
                if let Some(index) = position_of(rows, &id) {
                    self.focus_slot = Some(index);
                    report.landed_focus = Some(id);
                } else {
                    // Collapsed away: focus the ancestor whose folding
                    // hid it, which is where `Left` would have put it.
                    let landed = structure
                        .ancestors(&id)
                        .into_iter()
                        .find(|ancestor| position_of(rows, ancestor).is_some());
                    if let Some(landed) = landed {
                        let index = position_of(rows, &landed).unwrap_or(0);
                        self.focused = Some(landed.clone());
                        self.focus_slot = Some(index);
                        report.lost_focus = Some(id);
                        report.landed_focus = Some(landed);
                    } else {
                        self.focused = None;
                        self.focus_slot = slot;
                        report.lost_focus = Some(id);
                        report.landed_focus = self.land_from_slot(rows);
                    }
                }
            }
            Some(id) => {
                self.focused = None;
                report.lost_focus = Some(id);
                report.landed_focus = self.land_from_slot(rows);
            }
            None => {}
        }

        report
    }

    /// Land focus on whoever now holds the remembered slot.
    fn land_from_slot(&mut self, rows: &[VisibleRow<Id>]) -> Option<Id> {
        if rows.is_empty() {
            self.focus_slot = None;
            return None;
        }
        let index = self.focus_slot.unwrap_or(0) % rows.len();
        self.focus_index(rows, index)
    }

    // -- focus registration ----------------------------------------------

    /// The focus members this tree contributes to the crate's traversal.
    ///
    /// Every *visible* row is a member, and a row in `disabled` is
    /// registered as a disabled member rather than omitted — the
    /// [`FocusState`](crate::focus::FocusState) walk steps over a
    /// disabled member and cannot reach one that is absent, so both
    /// readings refuse the same control, and registering keeps the
    /// row's place in the tab order.
    ///
    /// Rows hidden by a collapse are simply not in `rows`, which is what
    /// makes a collapsed subtree unreachable rather than merely greyed.
    #[must_use]
    pub fn focus_members(
        &self,
        rows: &[VisibleRow<Id>],
        disabled: &BTreeSet<Id>,
    ) -> Vec<FocusMember>
    where
        Id: core::fmt::Display,
    {
        rows.iter()
            .map(|row| {
                let id = FocusMember::new(row.id.to_string());
                if disabled.contains(&row.id) {
                    FocusMember::disabled(row.id.to_string())
                } else {
                    id
                }
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "tree_view_tests.rs"]
mod tree_view_tests;
