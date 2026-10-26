//! The 2D virtualized tile and card grid composite widget (`UIF-COMP-013`, `UIF-VER-002`, `UIF-PRIN-006`).
//!
//! §3B.13 specifies a responsive, virtualized two-dimensional card grid. The column count and
//! tile width are derived deterministically by [`solve_columns`]:
//!
//! * Effective width deducts the reserved `10px` [`GUTTER_WIDTH`] ([`size::SPLITTER_GRAB`]):
//!   `effective_width = (available_width - gutter).max(min_tile_width)`.
//! * Column count: `cols = (((effective_width + gap) / (min_tile_width + gap)).floor() as usize).max(1)`.
//! * Symmetrically expanded tile width: `tile_width = (effective_width - (cols - 1) as f32 * gap) / cols as f32`.
//!
//! Two-dimensional navigation clamps strictly at grid boundaries without cross-row wrapping:
//! - `Left`: `index.saturating_sub(1)` (clamped at 0).
//! - `Right`: `(index + 1).min(items.len() - 1)`.
//! - `Up`: `index.saturating_sub(cols)` (clamped, does not wrap to bottom).
//! - `Down`: `(index + cols).min(items.len() - 1)`.
//! - `Home`: `0`.
//! - `End`: `items.len() - 1`.
//!
//! Visual styling adheres to design tokens:
//! - Resting card container: [`RADIUS`] ([`shape::RADIUS_CARD`] = 6px), [`BORDER_REST_WIDTH`] ([`shape::BORDER_HAIRLINE`] = 1px) in `border.subtle`.
//! - Hovered tile: `surface.hover` background.
//! - Selected tile: `surface.active` background, [`BORDER_SELECTED_WIDTH`] ([`shape::BORDER_FOCUS`] = 2px) in `accent.border`
//!   (4.79:1 contrast), and `text.strong` (14.29:1 contrast).
//! - In [`TreeSelectionMode::ExtendedMulti`] mode: top-left 16×16px ([`CHECKBOX_SIZE`]) checkbox overlay
//!   (checked if selected, unchecked if unselected).
//!
//! Virtualized rendering culls tiles outside the active scroll viewport, rendering only rows intersecting the viewport.
//!
//! Unit tests live in the sibling `grid_view_tests.rs` sidecar.

use crate::binding::Commit;
use crate::composite::Icon;
use crate::composite::tree_view::TreeSelectionMode;
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as text_core, LineHeight};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::alignment;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::mouse;
use iced::{Element, Length, Pixels, Point, Rectangle, Size};
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// Surface Geometry Constants & Tokens (§3B.13)
// ---------------------------------------------------------------------------

/// Default minimum tile width — 160px (§3B.13, `UIF-COMP-013`).
pub const DEFAULT_MIN_TILE_WIDTH: f32 = 160.0;

/// Default tile height — 120px (§3B.13, `UIF-COMP-013`).
pub const DEFAULT_TILE_HEIGHT: f32 = 120.0;

/// Grid gap between tiles — `space::MD` (12px, §3B.13, `UIF-COMP-013`).
pub const GAP: f32 = space::MD;

/// Reserved gutter width for ScrollView scrollbar — `size::SPLITTER_GRAB` (10px, §3B.13, `UIF-COMP-013`).
pub const GUTTER_WIDTH: f32 = size::SPLITTER_GRAB;

/// Checkbox overlay box dimension in `ExtendedMulti` mode — `size::CHECKBOX_BOX` (16px, §3B.13, `UIF-COMP-013`).
pub const CHECKBOX_SIZE: f32 = size::CHECKBOX_BOX;

/// Corner radius of the grid tile card container — `shape::RADIUS_CARD` (6px, §3B.13, `UIF-PRIN-006`).
pub const RADIUS: f32 = shape::RADIUS_CARD;

/// Border stroke weight for selected tiles — `shape::BORDER_FOCUS` (2px, §3B.13, `UIF-PRIN-006`).
pub const BORDER_SELECTED_WIDTH: f32 = shape::BORDER_FOCUS;

/// Border stroke weight for resting and hovered tiles — `shape::BORDER_HAIRLINE` (1px, §3B.13, `UIF-PRIN-006`).
pub const BORDER_REST_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Checkbox inset from the top-left tile corner — `space::SM` (8px, §3B.13, `UIF-COMP-013`).
pub const CHECKBOX_INSET: f32 = space::SM;

/// Internal padding of each tile — `space::SM` (8px, §3B.13, `UIF-COMP-013`).
pub const TILE_PADDING: f32 = space::SM;

/// Preview icon optical dimension — `size::HIT_TARGET_MIN` (24px, §3B.13, `UIF-COMP-013`).
pub const ICON_SIZE: f32 = size::HIT_TARGET_MIN;

/// Checkmark glyph for the checkbox overlay — `"✓"` (§3B.13).
pub const GLYPH_CHECK: &str = "✓";

// ---------------------------------------------------------------------------
// Pure Layout & Geometry Solvers (§3B.13)
// ---------------------------------------------------------------------------

/// Deterministic column count and tile width solver (§3B.13, `UIF-VER-002`, `UIF-COMP-013`).
///
/// Computes columns and tile width according to:
/// `effective_width = (available_width - gutter).max(min_tile_width)`
/// `cols = (((effective_width + gap) / (min_tile_width + gap)).floor() as usize).max(1)`
/// `tile_width = (effective_width - (cols - 1) as f32 * gap) / cols as f32`
#[must_use]
pub fn solve_columns(
    available_width: f32,
    min_tile_width: f32,
    gap: f32,
    gutter: f32,
) -> (usize, f32) {
    let min_tile = min_tile_width.max(1.0);
    let gap_val = gap.max(0.0);
    let gutter_val = gutter.max(0.0);
    let effective_width = (available_width - gutter_val).max(min_tile);
    let cols = (((effective_width + gap_val) / (min_tile + gap_val)).floor() as usize).max(1);
    let tile_width = (effective_width - (cols - 1) as f32 * gap_val) / cols as f32;
    (cols, tile_width)
}

/// Compute the tile bounding rectangle at `index` given grid origin and layout geometry (`UIF-COMP-013`).
#[must_use]
pub fn tile_rect(
    origin: Point,
    index: usize,
    cols: usize,
    tile_width: f32,
    tile_height: f32,
    gap: f32,
) -> Rectangle {
    let columns = cols.max(1);
    let col = index % columns;
    let row = index / columns;
    let x = origin.x + col as f32 * (tile_width + gap);
    let y = origin.y + row as f32 * (tile_height + gap);
    Rectangle {
        x,
        y,
        width: tile_width,
        height: tile_height,
    }
}

// ---------------------------------------------------------------------------
// Navigation Direction
// ---------------------------------------------------------------------------

/// Direction for 2D keyboard navigation steps (`UIF-COMP-013`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridNavKey {
    /// Move left by 1 tile (clamped at 0).
    Left,
    /// Move right by 1 tile (clamped at last item).
    Right,
    /// Move up by `cols` tiles (clamped at 0, no wrap).
    Up,
    /// Move down by `cols` tiles (clamped at last item).
    Down,
    /// Jump to the first tile (index 0).
    Home,
    /// Jump to the last tile.
    End,
}

// ---------------------------------------------------------------------------
// Declarative Data Models
// ---------------------------------------------------------------------------

/// Declarative description of a single tile in the grid (`UIF-COMP-013`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridTile<Id> {
    /// Unique identifier for this tile.
    pub id: Id,
    /// Primary title text.
    pub title: String,
    /// Optional secondary subtitle text.
    pub subtitle: Option<String>,
    /// Optional leading or centered preview icon.
    pub preview_icon: Option<Icon>,
    /// Whether this tile is disabled and non-interactive.
    pub disabled: bool,
}

impl<Id> GridTile<Id> {
    /// Construct a new grid tile with the given identifier and title.
    #[must_use]
    pub fn new(id: Id, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            subtitle: None,
            preview_icon: None,
            disabled: false,
        }
    }

    /// Set an optional subtitle.
    #[must_use]
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Set an optional preview icon.
    #[must_use]
    pub fn preview_icon(mut self, icon: impl Into<Icon>) -> Self {
        self.preview_icon = Some(icon.into());
        self
    }

    /// Set whether the tile is disabled.
    #[must_use]
    pub const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Declarative state model for the 2D grid view composite (`UIF-COMP-013`).
#[derive(Debug, Clone, PartialEq)]
pub struct GridViewModel<Id> {
    /// Items displayed as tiles in the grid.
    pub items: Vec<GridTile<Id>>,
    /// Set of selected tile identifiers.
    pub selected_ids: BTreeSet<Id>,
    /// Anchor identifier for range selection (`Shift`+click/arrow).
    pub anchor_id: Option<Id>,
    /// Currently keyboard-focused tile identifier, if any.
    pub focused_id: Option<Id>,
    /// Minimum width of each tile in logical pixels.
    pub min_tile_width: f32,
    /// Fixed height of each tile in logical pixels.
    pub tile_height: f32,
    /// Selection mode (`Single` or `ExtendedMulti`).
    pub selection_mode: TreeSelectionMode,
}

impl<Id: Clone + Ord> GridViewModel<Id> {
    /// Construct a new grid view model with default dimensions and single selection mode.
    #[must_use]
    pub fn new(items: Vec<GridTile<Id>>) -> Self {
        Self {
            items,
            selected_ids: BTreeSet::new(),
            anchor_id: None,
            focused_id: None,
            min_tile_width: DEFAULT_MIN_TILE_WIDTH,
            tile_height: DEFAULT_TILE_HEIGHT,
            selection_mode: TreeSelectionMode::Single,
        }
    }

    /// Set the selection mode (`Single` or `ExtendedMulti`).
    #[must_use]
    pub const fn selection_mode(mut self, mode: TreeSelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// Set the minimum tile width.
    #[must_use]
    pub const fn min_tile_width(mut self, width: f32) -> Self {
        self.min_tile_width = width;
        self
    }

    /// Set the fixed tile height.
    #[must_use]
    pub const fn tile_height(mut self, height: f32) -> Self {
        self.tile_height = height;
        self
    }

    /// Find the index of an item by identifier.
    #[must_use]
    pub fn index_of(&self, id: &Id) -> Option<usize> {
        self.items.iter().position(|item| &item.id == id)
    }

    /// Get the identifier at a specific index.
    #[must_use]
    pub fn id_at(&self, index: usize) -> Option<Id> {
        self.items.get(index).map(|item| item.id.clone())
    }

    /// Compute the next index for a 2D navigation step in a grid of `cols` columns (`UIF-COMP-013`).
    #[must_use]
    pub fn step_nav(&self, current_index: usize, cols: usize, key: GridNavKey) -> usize {
        if self.items.is_empty() {
            return 0;
        }
        let last = self.items.len().saturating_sub(1);
        let current = current_index.min(last);
        let columns = cols.max(1);
        match key {
            GridNavKey::Left => current.saturating_sub(1),
            GridNavKey::Right => (current + 1).min(last),
            GridNavKey::Up => current.saturating_sub(columns),
            GridNavKey::Down => (current + columns).min(last),
            GridNavKey::Home => 0,
            GridNavKey::End => last,
        }
    }

    /// Step left by 1 tile (clamped at 0).
    #[must_use]
    pub fn step_left(&self, current_index: usize, cols: usize) -> usize {
        self.step_nav(current_index, cols, GridNavKey::Left)
    }

    /// Step right by 1 tile (clamped at end).
    #[must_use]
    pub fn step_right(&self, current_index: usize, cols: usize) -> usize {
        self.step_nav(current_index, cols, GridNavKey::Right)
    }

    /// Step up by `cols` tiles (clamped at 0, no wrap).
    #[must_use]
    pub fn step_up(&self, current_index: usize, cols: usize) -> usize {
        self.step_nav(current_index, cols, GridNavKey::Up)
    }

    /// Step down by `cols` tiles (clamped at end).
    #[must_use]
    pub fn step_down(&self, current_index: usize, cols: usize) -> usize {
        self.step_nav(current_index, cols, GridNavKey::Down)
    }

    /// Step to the first tile (index 0).
    #[must_use]
    pub fn step_home(&self) -> usize {
        self.step_nav(0, 1, GridNavKey::Home)
    }

    /// Step to the last tile.
    #[must_use]
    pub fn step_end(&self) -> usize {
        self.step_nav(0, 1, GridNavKey::End)
    }

    /// Resolve the target ID after a 2D navigation step from `from_id`.
    #[must_use]
    pub fn step_from_id(&self, from_id: &Id, cols: usize, key: GridNavKey) -> Option<Id> {
        let index = self.index_of(from_id)?;
        let next_index = self.step_nav(index, cols, key);
        self.id_at(next_index)
    }

    /// Select a single item, clearing existing selections and setting anchor and focus (`UIF-COMP-013`).
    pub fn select_single(&mut self, id: Id) -> bool {
        let changed = self.selected_ids.len() != 1 || !self.selected_ids.contains(&id);
        self.selected_ids.clear();
        self.selected_ids.insert(id.clone());
        self.anchor_id = Some(id.clone());
        self.focused_id = Some(id);
        changed
    }

    /// Select all items in the range from `anchor_id` to `id` (`UIF-COMP-013`).
    pub fn select_range_to(&mut self, id: Id) -> bool {
        if !self.selection_mode.is_multi() {
            return self.select_single(id);
        }
        let Some(to_idx) = self.index_of(&id) else {
            return false;
        };
        let from_idx = self
            .anchor_id
            .as_ref()
            .and_then(|a| self.index_of(a))
            .unwrap_or(to_idx);
        let (lo, hi) = if from_idx <= to_idx {
            (from_idx, to_idx)
        } else {
            (to_idx, from_idx)
        };
        let next: BTreeSet<Id> = self.items[lo..=hi]
            .iter()
            .map(|item| item.id.clone())
            .collect();
        let changed = next != self.selected_ids;
        self.selected_ids = next;
        self.focused_id = Some(id);
        changed
    }

    /// Toggle selection of an item (`UIF-COMP-013`).
    pub fn toggle_selection(&mut self, id: Id) -> bool {
        self.anchor_id = Some(id.clone());
        self.focused_id = Some(id.clone());
        if !self.selection_mode.is_multi() {
            return self.select_single(id);
        }
        if self.selected_ids.contains(&id) {
            self.selected_ids.remove(&id);
        } else {
            self.selected_ids.insert(id);
        }
        true
    }

    /// Select all items in `ExtendedMulti` mode (`UIF-COMP-013`).
    pub fn select_all(&mut self) -> bool {
        if !self.selection_mode.is_multi() {
            return false;
        }
        let next: BTreeSet<Id> = self.items.iter().map(|item| item.id.clone()).collect();
        let changed = next != self.selected_ids;
        self.selected_ids = next;
        changed
    }

    /// Clear all selections (`UIF-COMP-013`).
    pub fn clear_selection(&mut self) -> bool {
        let changed = !self.selected_ids.is_empty();
        self.selected_ids.clear();
        changed
    }
}

impl<Id: Clone + Ord> Default for GridViewModel<Id> {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// Ephemeral State
// ---------------------------------------------------------------------------

/// Ephemeral interaction state for the grid view widget (`UIF-COMP-013`).
#[derive(Debug, Clone)]
pub struct GridViewState<Id> {
    /// Currently hovered tile identifier, if any.
    pub hovered_id: Option<Id>,
    /// Currently active keyboard modifier keys.
    pub modifiers: iced::keyboard::Modifiers,
}

impl<Id> Default for GridViewState<Id> {
    fn default() -> Self {
        Self {
            hovered_id: None,
            modifiers: iced::keyboard::Modifiers::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

/// Callback invoked when grid selection changes.
pub type OnSelectionChange<'a, Message, Id> = Box<dyn Fn(Commit<BTreeSet<Id>>) -> Message + 'a>;

/// Callback invoked when a context menu is requested on selected items at a point.
pub type OnContextMenu<'a, Message, Id> =
    Box<dyn Fn(Commit<(BTreeSet<Id>, Point)>) -> Message + 'a>;

// ---------------------------------------------------------------------------
// Widget Implementation
// ---------------------------------------------------------------------------

/// The 2D virtualized tile/card grid composite widget (`UIF-COMP-013`, `UIF-VER-002`, `UIF-PRIN-006`).
pub struct GridView<'a, Message, Id, Theme = iced::Theme, Renderer = iced::Renderer> {
    model: GridViewModel<Id>,
    available_width: f32,
    on_selection_change: Option<OnSelectionChange<'a, Message, Id>>,
    on_activate: Option<Box<dyn Fn(Commit<Id>) -> Message + 'a>>,
    on_context_menu: Option<OnContextMenu<'a, Message, Id>>,
    _marker: std::marker::PhantomData<(&'a (), Theme, Renderer)>,
}

impl<'a, Message, Id: Clone + Ord + 'a, Theme, Renderer>
    GridView<'a, Message, Id, Theme, Renderer>
{
    /// Construct a new [`GridView`] widget.
    #[must_use]
    pub fn new(model: GridViewModel<Id>, available_width: f32) -> Self {
        Self {
            model,
            available_width,
            on_selection_change: None,
            on_activate: None,
            on_context_menu: None,
            _marker: std::marker::PhantomData,
        }
    }

    /// Access the underlying grid model.
    #[must_use]
    pub fn model(&self) -> &GridViewModel<Id> {
        &self.model
    }

    /// Access the configured available width.
    #[must_use]
    pub fn available_width(&self) -> f32 {
        self.available_width
    }

    /// Register a callback fired when the selected items change (`UIF-COMP-013`).
    #[must_use]
    pub fn on_selection_change(mut self, f: impl Fn(Commit<BTreeSet<Id>>) -> Message + 'a) -> Self {
        self.on_selection_change = Some(Box::new(f));
        self
    }

    /// Register a callback fired when an item is activated via `Enter` or double click (`UIF-COMP-013`).
    #[must_use]
    pub fn on_activate(mut self, f: impl Fn(Commit<Id>) -> Message + 'a) -> Self {
        self.on_activate = Some(Box::new(f));
        self
    }

    /// Register a callback fired when a context menu is requested on an item (`UIF-COMP-013`).
    #[must_use]
    pub fn on_context_menu(
        mut self,
        f: impl Fn(Commit<(BTreeSet<Id>, Point)>) -> Message + 'a,
    ) -> Self {
        self.on_context_menu = Some(Box::new(f));
        self
    }
}

impl<Message, Id: Clone + Ord + 'static, Theme, Renderer> Widget<Message, Theme, Renderer>
    for GridView<'_, Message, Id, Theme, Renderer>
where
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<GridViewState<Id>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(GridViewState::<Id>::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(
            if self.available_width > 0.0 {
                Length::Fixed(self.available_width)
            } else {
                Length::Fill
            },
            Length::Shrink,
        )
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let effective_w = if self.available_width > 0.0 {
            self.available_width
        } else {
            limits.max().width.max(self.model.min_tile_width)
        };
        let (cols, _tile_width) =
            solve_columns(effective_w, self.model.min_tile_width, GAP, GUTTER_WIDTH);
        let count = self.model.items.len();
        let rows = if count == 0 { 0 } else { count.div_ceil(cols) };
        let total_height = if rows == 0 {
            0.0
        } else {
            rows as f32 * self.model.tile_height + (rows - 1) as f32 * GAP
        };
        layout::Node::new(Size::new(effective_w, total_height))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<GridViewState<Id>>();
        let bounds = layout.bounds();
        let effective_w = if self.available_width > 0.0 {
            self.available_width
        } else {
            bounds.width.max(self.model.min_tile_width)
        };
        let (cols, tile_width) =
            solve_columns(effective_w, self.model.min_tile_width, GAP, GUTTER_WIDTH);

        // Track hovered tile
        state.hovered_id = None;
        if let Some(pos) = cursor.position()
            && bounds.contains(pos)
        {
            let col_pitch = tile_width + GAP;
            let row_pitch = self.model.tile_height + GAP;
            let rel_x = pos.x - bounds.x;
            let rel_y = pos.y - bounds.y;
            let c = (rel_x / col_pitch).floor() as usize;
            let r = (rel_y / row_pitch).floor() as usize;
            if c < cols {
                let idx = r * cols + c;
                if idx < self.model.items.len() {
                    let rect = tile_rect(
                        bounds.position(),
                        idx,
                        cols,
                        tile_width,
                        self.model.tile_height,
                        GAP,
                    );
                    if rect.contains(pos) {
                        state.hovered_id = Some(self.model.items[idx].id.clone());
                    }
                }
            }
        }

        match event {
            iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
                state.modifiers = *mods;
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                state.modifiers = *modifiers;
                let is_shift = modifiers.shift();
                let is_ctrl = modifiers.control();

                // Select all: Ctrl+A
                if is_ctrl && matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("a")) {
                    if self.model.selection_mode.is_multi() {
                        let changed = self.model.select_all();
                        if changed && let Some(on_change) = &self.on_selection_change {
                            shell
                                .publish((on_change)(Commit::new(self.model.selected_ids.clone())));
                        }
                        shell.capture_event();
                    }
                    return;
                }

                // Enter: Activate focused tile
                if matches!(key, Key::Named(Named::Enter)) {
                    if let Some(focused) = &self.model.focused_id
                        && let Some(on_act) = &self.on_activate
                    {
                        shell.publish((on_act)(Commit::new(focused.clone())));
                        shell.capture_event();
                    }
                    return;
                }

                // Space: Toggle or select focused tile
                if matches!(key, Key::Named(Named::Space)) {
                    if let Some(focused) = self.model.focused_id.clone() {
                        if self.model.selection_mode.is_multi() {
                            self.model.toggle_selection(focused);
                        } else {
                            self.model.select_single(focused);
                        }
                        if let Some(on_change) = &self.on_selection_change {
                            shell
                                .publish((on_change)(Commit::new(self.model.selected_ids.clone())));
                        }
                        shell.capture_event();
                    }
                    return;
                }

                // Navigation keys: ArrowLeft, ArrowRight, ArrowUp, ArrowDown, Home, End
                let nav_key = match key {
                    Key::Named(Named::ArrowLeft) => Some(GridNavKey::Left),
                    Key::Named(Named::ArrowRight) => Some(GridNavKey::Right),
                    Key::Named(Named::ArrowUp) => Some(GridNavKey::Up),
                    Key::Named(Named::ArrowDown) => Some(GridNavKey::Down),
                    Key::Named(Named::Home) => Some(GridNavKey::Home),
                    Key::Named(Named::End) => Some(GridNavKey::End),
                    _ => None,
                };

                if let Some(nav) = nav_key {
                    if self.model.items.is_empty() {
                        return;
                    }
                    let current_idx = self
                        .model
                        .focused_id
                        .as_ref()
                        .and_then(|id| self.model.index_of(id))
                        .unwrap_or(0);
                    let target_idx = self.model.step_nav(current_idx, cols, nav);
                    if let Some(target_id) = self.model.id_at(target_idx) {
                        if is_shift {
                            // Shift: Range select from anchor to target
                            let changed = self.model.select_range_to(target_id);
                            if changed && let Some(on_change) = &self.on_selection_change {
                                shell.publish((on_change)(Commit::new(
                                    self.model.selected_ids.clone(),
                                )));
                            }
                        } else if is_ctrl {
                            // Ctrl: Move focus only
                            self.model.focused_id = Some(target_id);
                        } else {
                            // Plain: Select single item
                            let changed = self.model.select_single(target_id);
                            if changed && let Some(on_change) = &self.on_selection_change {
                                shell.publish((on_change)(Commit::new(
                                    self.model.selected_ids.clone(),
                                )));
                            }
                        }
                        shell.capture_event();
                    }
                }
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(ref hit_id) = state.hovered_id {
                    let idx = self.model.index_of(hit_id);
                    if let Some(i) = idx {
                        let item = &self.model.items[i];
                        if !item.disabled {
                            let id = hit_id.clone();
                            let is_shift = state.modifiers.shift();
                            let is_ctrl = state.modifiers.control();

                            // Check if hit was specifically on the checkbox in ExtendedMulti
                            let is_checkbox_hit = if self.model.selection_mode.is_multi() {
                                let rect = tile_rect(
                                    bounds.position(),
                                    i,
                                    cols,
                                    tile_width,
                                    self.model.tile_height,
                                    GAP,
                                );
                                let cb_rect = Rectangle {
                                    x: rect.x + CHECKBOX_INSET,
                                    y: rect.y + CHECKBOX_INSET,
                                    width: CHECKBOX_SIZE,
                                    height: CHECKBOX_SIZE,
                                };
                                cursor.position().is_some_and(|p| cb_rect.contains(p))
                            } else {
                                false
                            };

                            let changed = if is_checkbox_hit || is_ctrl {
                                self.model.toggle_selection(id)
                            } else if is_shift {
                                self.model.select_range_to(id)
                            } else {
                                self.model.select_single(id)
                            };

                            if changed && let Some(on_change) = &self.on_selection_change {
                                shell.publish((on_change)(Commit::new(
                                    self.model.selected_ids.clone(),
                                )));
                            }
                            shell.capture_event();
                        }
                    }
                }
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                if let Some(ref hit_id) = state.hovered_id {
                    let idx = self.model.index_of(hit_id);
                    if let Some(i) = idx {
                        let item = &self.model.items[i];
                        if !item.disabled {
                            let id = hit_id.clone();
                            let mut changed = false;
                            if !self.model.selected_ids.contains(&id) {
                                changed = self.model.select_single(id.clone());
                            }
                            if changed && let Some(on_change) = &self.on_selection_change {
                                shell.publish((on_change)(Commit::new(
                                    self.model.selected_ids.clone(),
                                )));
                            }
                            if let Some(on_ctx) = &self.on_context_menu
                                && let Some(pos) = cursor.position()
                            {
                                shell.publish((on_ctx)(Commit::new((
                                    self.model.selected_ids.clone(),
                                    pos,
                                ))));
                            }
                            shell.capture_event();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<GridViewState<Id>>();
        let bounds = layout.bounds();
        let effective_w = if self.available_width > 0.0 {
            self.available_width
        } else {
            bounds.width.max(self.model.min_tile_width)
        };
        let (cols, tile_width) =
            solve_columns(effective_w, self.model.min_tile_width, GAP, GUTTER_WIDTH);
        let palette = Palette::dark();
        let total_items = self.model.items.len();
        if total_items == 0 {
            return;
        }

        // Virtualized row culling
        let row_pitch = self.model.tile_height + GAP;
        let rel_top = (viewport.y - bounds.y).max(0.0);
        let rel_bottom = ((viewport.y + viewport.height) - bounds.y).max(0.0);
        let start_row = (rel_top / row_pitch).floor() as usize;
        let total_rows = if total_items == 0 {
            0
        } else {
            total_items.div_ceil(cols)
        };
        let end_row = (((rel_bottom / row_pitch).ceil() as usize) + 1).min(total_rows);

        let start_idx = start_row * cols;
        let end_idx = (end_row * cols).min(total_items);

        for idx in start_idx..end_idx {
            let item = &self.model.items[idx];
            let rect = tile_rect(
                bounds.position(),
                idx,
                cols,
                tile_width,
                self.model.tile_height,
                GAP,
            );

            if !rect.intersects(viewport) {
                continue;
            }

            let is_selected = self.model.selected_ids.contains(&item.id);
            let is_hovered = state.hovered_id.as_ref() == Some(&item.id);
            let is_disabled = item.disabled;

            // Resolve background & border
            let (bg_color, border_color, border_w, title_color) = if is_disabled {
                (
                    palette.surface_raised,
                    palette.border_subtle,
                    BORDER_REST_WIDTH,
                    palette.text_disabled,
                )
            } else if is_selected {
                (
                    palette.surface_active,
                    palette.accent_border,
                    BORDER_SELECTED_WIDTH,
                    palette.text_strong,
                )
            } else if is_hovered {
                (
                    palette.surface_hover,
                    palette.border_subtle,
                    BORDER_REST_WIDTH,
                    palette.text_primary,
                )
            } else {
                (
                    palette.surface_raised,
                    palette.border_subtle,
                    BORDER_REST_WIDTH,
                    palette.text_primary,
                )
            };

            // 1. Tile card container
            renderer.fill_quad(
                renderer::Quad {
                    bounds: rect,
                    border: iced::Border {
                        color: border_color,
                        width: border_w,
                        radius: RADIUS.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
                iced::Background::Color(bg_color),
            );

            // 2. Checkbox overlay in ExtendedMulti mode (top-left 16x16px)
            if self.model.selection_mode.is_multi() {
                let cb_rect = Rectangle {
                    x: rect.x + CHECKBOX_INSET,
                    y: rect.y + CHECKBOX_INSET,
                    width: CHECKBOX_SIZE,
                    height: CHECKBOX_SIZE,
                };

                let (cb_bg, cb_border, cb_border_w) = if is_selected {
                    (
                        iced::Background::Color(palette.accent_border),
                        palette.accent_border,
                        BORDER_REST_WIDTH,
                    )
                } else {
                    (
                        iced::Background::Color(palette.surface_raised),
                        palette.border_strong,
                        BORDER_REST_WIDTH,
                    )
                };

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: cb_rect,
                        border: iced::Border {
                            color: cb_border,
                            width: cb_border_w,
                            radius: shape::RADIUS_WIDGET.into(),
                        },
                        shadow: iced::Shadow::default(),
                        snap: true,
                    },
                    cb_bg,
                );

                if is_selected {
                    renderer.fill_text(
                        text_core::Text {
                            content: GLYPH_CHECK.to_string(),
                            bounds: cb_rect.size(),
                            size: Pixels(typography::CAPTION.size),
                            line_height: LineHeight::Absolute(Pixels(CHECKBOX_SIZE)),
                            font: typography::CAPTION
                                .font(typography::FAMILY_SANS, typography::Weight::Bold),
                            align_x: text_core::Alignment::Center,
                            align_y: alignment::Vertical::Center,
                            shaping: text_core::Shaping::default(),
                            wrapping: text_core::Wrapping::None,
                        },
                        cb_rect.position(),
                        palette.surface_base,
                        *viewport,
                    );
                }
            }

            // 3. Tile content: Preview icon
            let mut content_y = rect.y + TILE_PADDING;
            if let Some(ref icon) = item.preview_icon {
                let icon_rect = Rectangle {
                    x: rect.x + (rect.width - ICON_SIZE) / 2.0,
                    y: content_y + space::XS,
                    width: ICON_SIZE,
                    height: ICON_SIZE,
                };

                renderer.fill_text(
                    text_core::Text {
                        content: icon.0.clone(),
                        bounds: icon_rect.size(),
                        size: Pixels(ICON_SIZE),
                        line_height: LineHeight::Absolute(Pixels(ICON_SIZE)),
                        font: typography::CAPTION
                            .font(typography::FAMILY_SANS, typography::Weight::Regular),
                        align_x: text_core::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    icon_rect.position(),
                    if is_disabled {
                        palette.text_disabled
                    } else if is_selected {
                        palette.accent_text
                    } else {
                        palette.text_primary
                    },
                    *viewport,
                );

                content_y += ICON_SIZE + space::SM;
            } else {
                content_y += space::LG;
            }

            // 4. Title
            let title_line_h = typography::CAPTION.line_height;
            let title_rect = Rectangle {
                x: rect.x + TILE_PADDING,
                y: content_y,
                width: (rect.width - TILE_PADDING * 2.0).max(0.0),
                height: title_line_h,
            };

            let title_weight = if is_selected {
                typography::Weight::SemiBold
            } else {
                typography::Weight::Regular
            };

            renderer.fill_text(
                text_core::Text {
                    content: item.title.clone(),
                    bounds: title_rect.size(),
                    size: Pixels(typography::CAPTION.size),
                    line_height: LineHeight::Absolute(Pixels(title_line_h)),
                    font: typography::CAPTION.font(typography::FAMILY_SANS, title_weight),
                    align_x: text_core::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                    shaping: text_core::Shaping::default(),
                    wrapping: text_core::Wrapping::None,
                },
                title_rect.position(),
                title_color,
                *viewport,
            );

            // 5. Optional subtitle
            if let Some(ref subtitle) = item.subtitle {
                let sub_y = title_rect.y + title_line_h + space::XXS;
                let sub_rect = Rectangle {
                    x: rect.x + TILE_PADDING,
                    y: sub_y,
                    width: (rect.width - TILE_PADDING * 2.0).max(0.0),
                    height: title_line_h,
                };
                let sub_color = if is_disabled {
                    palette.text_disabled
                } else {
                    palette.text_muted
                };

                renderer.fill_text(
                    text_core::Text {
                        content: subtitle.clone(),
                        bounds: sub_rect.size(),
                        size: Pixels(typography::CAPTION.size),
                        line_height: LineHeight::Absolute(Pixels(title_line_h)),
                        font: typography::CAPTION
                            .font(typography::FAMILY_SANS, typography::Weight::Regular),
                        align_x: text_core::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    sub_rect.position(),
                    sub_color,
                    *viewport,
                );
            }
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a, Message: 'a, Id: 'static + Clone + Ord, Theme: 'a, Renderer>
    From<GridView<'a, Message, Id, Theme, Renderer>> for Element<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font> + 'a,
{
    fn from(grid: GridView<'a, Message, Id, Theme, Renderer>) -> Self {
        Element::new(grid)
    }
}

/// Construct a new 2D virtualized grid view widget (§3B.13, `UIF-COMP-013`).
#[must_use]
pub fn grid_view<'a, Message, Id: Clone + Ord + 'a>(
    model: GridViewModel<Id>,
    available_width: f32,
) -> GridView<'a, Message, Id> {
    GridView::new(model, available_width)
}

#[cfg(test)]
#[path = "grid_view_tests.rs"]
mod grid_view_tests;
