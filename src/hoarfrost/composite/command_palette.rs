//! The `CommandPalette` / modal `SearchView` composite (`UIF-COMP-010`, `UIF-PRIN-003`, `UIF-TOK-006`).
//!
//! §3B.10 specifies the application command palette and quick-open modal surface:
//! a 560×380px overlay card mounted in Z-Band 3 (`ZBand::Modal`) at Escape Rank 2
//! over `surface.scrim`, fixed at a 64px top offset, with a 40px search input row,
//! removable mode prefix chips (`>`, `#`, `@`), a two-stage `Escape` dismiss contract,
//! grouped result lists with matched-span emphasis, active-row Rule 4 `text.strong`
//! subtitle promotion, mono shortcut pills, and a 24px keyboard legend footer.
//!
//! Unit tests live in the sibling `command_palette_tests.rs` sidecar.

use std::marker::PhantomData;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as text_core, LineHeight, Paragraph};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::{Element, Event, Length, Pixels, Point, Rectangle, Size, alignment, mouse};

use crate::binding::{Cancel, Commit, Live};
use crate::token::{color::Palette, shape, size, space, typography};
use crate::widget::text_input_model::TextRange;

// ---------------------------------------------------------------------------
// Surface Geometry Constants (§3B.10, UIF-PRIN-001 §1.1)
// ---------------------------------------------------------------------------

/// Width of the command palette card — `560px` (§3B.10, `UIF-COMP-010`).
///
/// A Surface Geometry Constant registered under `UIF-PRIN-001` §1.1.
pub const CARD_WIDTH: f32 = 560.0;

/// Height of the command palette card — `380px` (§3B.10, `UIF-COMP-010`).
///
/// A Surface Geometry Constant registered under `UIF-PRIN-001` §1.1.
pub const CARD_HEIGHT: f32 = 380.0;

/// Fixed offset from top of viewport — `64px` (§3B.10, `UIF-COMP-010`).
///
/// A Surface Geometry Constant registered under `UIF-PRIN-001` §1.1.
pub const TOP_OFFSET: f32 = 64.0;

/// Height of the top search input row — `40px` (§3B.10, `UIF-COMP-010`).
///
/// A Surface Geometry Constant registered under `UIF-PRIN-001` §1.1.
pub const SEARCH_HEIGHT: f32 = 40.0;

/// Single-line item row height — `size::CONTROL_MD` (`32px`, §3B.10, `UIF-COMP-010`).
pub const ROW_SINGLE_HEIGHT: f32 = size::CONTROL_MD;

/// Two-line item row height with subtitle — `44px` (§3B.10, `UIF-COMP-010`).
///
/// A Surface Geometry Constant registered under `UIF-PRIN-001` §1.1.
pub const ROW_DOUBLE_HEIGHT: f32 = 44.0;

/// Height of the footer keyboard legend — `size::BAR_STATUS` (`24px`, §3B.10, `UIF-COMP-010`).
pub const FOOTER_HEIGHT: f32 = size::BAR_STATUS;

/// Optical icon size — `size::ICON_MD` (`16px`, §3B.10, `UIF-COMP-010`).
pub const ICON_SIZE: f32 = size::ICON_MD;

/// Corner radius of the modal card — `shape::RADIUS_CARD` (`6px`, §3B.10, `UIF-COMP-010`).
pub const RADIUS: f32 = shape::RADIUS_CARD;

/// Section header height for grouped results — `size::BAR_STATUS` (`24px`, §3B.10, `UIF-COMP-010`).
pub const GROUP_HEADER_HEIGHT: f32 = size::BAR_STATUS;

/// Hairline divider stroke — `shape::BORDER_HAIRLINE` (`1px`, §3B.10, `UIF-TOK-006`).
pub const DIVIDER_STROKE: f32 = shape::BORDER_HAIRLINE;

// ---------------------------------------------------------------------------
// String Constants ([RUST-021])
// ---------------------------------------------------------------------------

/// Search glyph displayed in the leading slot of the search header.
pub const GLYPH_SEARCH: &str = "\u{1F50D}";

/// Keyboard legend text displayed in the 24px footer bar.
pub const LEGEND_KEYBOARD: &str = "↑↓ Navigate · ↵ Run · Esc Close";

/// Label displayed on the trailing Escape hint chip.
pub const LABEL_ESC: &str = "Esc";

/// Placeholder shown in Files mode when query is empty.
pub const PLACEHOLDER_FILES: &str = "Search files by name...";

/// Placeholder shown in Commands mode when query is empty.
pub const PLACEHOLDER_COMMANDS: &str = "Type a command...";

/// Placeholder shown in Symbols mode when query is empty.
pub const PLACEHOLDER_SYMBOLS: &str = "Search symbols...";

/// Placeholder shown in Tags mode when query is empty.
pub const PLACEHOLDER_TAGS: &str = "Search by tag...";

/// Empty state message displayed when query yields no results.
pub const NO_RESULTS: &str = "No matching results";

/// Busy state message displayed while provider query is in flight.
pub const SEARCHING: &str = "Searching...";

// ---------------------------------------------------------------------------
// PaletteMode
// ---------------------------------------------------------------------------

/// Mode of search operation in the command palette (§3B.10, `UIF-COMP-010`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PaletteMode {
    /// Default file search mode.
    #[default]
    Files,
    /// Action commands mode (prefix `>`).
    Commands,
    /// Symbol navigation mode (prefix `@`).
    Symbols,
    /// Tag search mode (prefix `#`).
    Tags,
}

impl PaletteMode {
    /// Returns the character prefix associated with this mode, or `None` for [`Self::Files`].
    #[must_use]
    pub const fn prefix(self) -> Option<char> {
        match self {
            Self::Files => None,
            Self::Commands => Some('>'),
            Self::Tags => Some('#'),
            Self::Symbols => Some('@'),
        }
    }

    /// Match a leading character to a palette mode.
    #[must_use]
    pub const fn from_prefix(ch: char) -> Option<Self> {
        match ch {
            '>' => Some(Self::Commands),
            '#' => Some(Self::Tags),
            '@' => Some(Self::Symbols),
            _ => None,
        }
    }

    /// User-facing label for the mode.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Commands => "Commands",
            Self::Tags => "Tags",
            Self::Symbols => "Symbols",
        }
    }

    /// Default placeholder string for this mode.
    #[must_use]
    pub const fn placeholder(self) -> &'static str {
        match self {
            Self::Files => PLACEHOLDER_FILES,
            Self::Commands => PLACEHOLDER_COMMANDS,
            Self::Symbols => PLACEHOLDER_SYMBOLS,
            Self::Tags => PLACEHOLDER_TAGS,
        }
    }
}

// ---------------------------------------------------------------------------
// PaletteItem & PaletteGroup
// ---------------------------------------------------------------------------

/// A single item result in the command palette (§3B.10, `UIF-COMP-010`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteItem<ItemId> {
    /// Unique identifier for this item.
    pub id: ItemId,
    /// Primary title text.
    pub title: String,
    /// Optional secondary subtitle text. If `Some`, item renders as a 44px double-row.
    pub subtitle: Option<String>,
    /// Substring ranges in `title` that matched the search query.
    pub match_spans: Vec<TextRange>,
    /// Optional keyboard shortcut label (e.g. `"Ctrl+P"`), rendered in Mono caption.
    pub shortcut: Option<String>,
}

impl<ItemId> PaletteItem<ItemId> {
    /// Construct a new palette item with `id` and `title`.
    pub fn new(id: ItemId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            subtitle: None,
            match_spans: Vec::new(),
            shortcut: None,
        }
    }

    /// Attach an optional subtitle, upgrading this item to a 44px two-line row.
    #[must_use]
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Attach matched character spans for visual emphasis.
    #[must_use]
    pub fn with_match_spans(mut self, match_spans: Vec<TextRange>) -> Self {
        self.match_spans = match_spans;
        self
    }

    /// Attach a right-aligned shortcut pill.
    #[must_use]
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

/// A section group of palette items with a section heading (§3B.10, `UIF-COMP-010`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteGroup<ItemId> {
    /// Section heading label (rendered in 24px caption header).
    pub heading: String,
    /// Items belonging to this group.
    pub items: Vec<PaletteItem<ItemId>>,
}

impl<ItemId> PaletteGroup<ItemId> {
    /// Construct a new palette group with `heading` and `items`.
    pub fn new(heading: impl Into<String>, items: Vec<PaletteItem<ItemId>>) -> Self {
        Self {
            heading: heading.into(),
            items,
        }
    }
}

// ---------------------------------------------------------------------------
// EscapeDisposition
// ---------------------------------------------------------------------------

/// Outcome of invoking `handle_escape` on a [`CommandPaletteModel`] (§3B.10, `UIF-COMP-010`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeDisposition {
    /// The search query was non-empty and has now been cleared to `""`.
    ClearedQuery,
    /// The search query was already empty, so the command palette should dismiss.
    Dismiss,
}

// ---------------------------------------------------------------------------
// CommandPaletteModel
// ---------------------------------------------------------------------------

/// The state model driving a `CommandPalette` composite (§3B.10, `UIF-COMP-010`).
#[derive(Debug, Clone, PartialEq)]
pub struct CommandPaletteModel<ItemId> {
    /// The current search query string (excluding mode prefix).
    pub query: String,
    /// The current operational search mode.
    pub mode: PaletteMode,
    /// Grouped search results.
    pub groups: Vec<PaletteGroup<ItemId>>,
    /// Currently highlighted item id (roving focus target).
    pub highlighted_id: Option<ItemId>,
    /// Monotonic query generation counter preventing out-of-order async responses.
    pub generation: u64,
    /// Whether an asynchronous search provider query is currently in flight.
    pub busy: bool,
    /// Whether the command palette modal overlay is open.
    pub open: bool,
}

impl<ItemId: Clone + PartialEq> CommandPaletteModel<ItemId> {
    /// Construct a new command palette model with initial groups.
    pub fn new(groups: Vec<PaletteGroup<ItemId>>) -> Self {
        let mut model = Self {
            query: String::new(),
            mode: PaletteMode::Files,
            groups,
            highlighted_id: None,
            generation: 0,
            busy: false,
            open: true,
        };
        model.highlight_first();
        model
    }

    /// Set search query.
    #[must_use]
    pub fn with_query(mut self, q: impl Into<String>) -> Self {
        self.query = q.into();
        self
    }

    /// Set palette mode.
    #[must_use]
    pub fn with_mode(mut self, m: PaletteMode) -> Self {
        self.mode = m;
        self
    }

    /// Set open status.
    #[must_use]
    pub fn with_open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Set busy status.
    #[must_use]
    pub fn with_busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Set highlighted item id.
    #[must_use]
    pub fn with_highlighted_id(mut self, id: Option<ItemId>) -> Self {
        self.highlighted_id = id;
        self
    }

    /// Collect flat references to all items in all groups in traversal order.
    #[must_use]
    pub fn all_items(&self) -> Vec<&PaletteItem<ItemId>> {
        self.groups.iter().flat_map(|g| &g.items).collect()
    }

    /// Total count of items across all groups.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.groups.iter().map(|g| g.items.len()).sum()
    }

    /// Rove highlight to first item if available.
    pub fn highlight_first(&mut self) {
        let items = self.all_items();
        self.highlighted_id = items.first().map(|i| i.id.clone());
    }

    /// Rove highlight to last item if available.
    pub fn highlight_last(&mut self) {
        let items = self.all_items();
        self.highlighted_id = items.last().map(|i| i.id.clone());
    }

    /// Rove highlight to next item, wrapping around at boundaries.
    pub fn highlight_next(&mut self) {
        let items = self.all_items();
        if items.is_empty() {
            self.highlighted_id = None;
            return;
        }
        match &self.highlighted_id {
            Some(current_id) => {
                if let Some(pos) = items.iter().position(|i| &i.id == current_id) {
                    let next_idx = (pos + 1) % items.len();
                    self.highlighted_id = Some(items[next_idx].id.clone());
                } else {
                    self.highlighted_id = Some(items[0].id.clone());
                }
            }
            None => {
                self.highlighted_id = Some(items[0].id.clone());
            }
        }
    }

    /// Rove highlight to previous item, wrapping around at boundaries.
    pub fn highlight_prev(&mut self) {
        let items = self.all_items();
        if items.is_empty() {
            self.highlighted_id = None;
            return;
        }
        match &self.highlighted_id {
            Some(current_id) => {
                if let Some(pos) = items.iter().position(|i| &i.id == current_id) {
                    let prev_idx = if pos == 0 { items.len() - 1 } else { pos - 1 };
                    self.highlighted_id = Some(items[prev_idx].id.clone());
                } else {
                    self.highlighted_id = Some(items[items.len() - 1].id.clone());
                }
            }
            None => {
                self.highlighted_id = Some(items[items.len() - 1].id.clone());
            }
        }
    }

    /// Read the currently selected item if one matches `highlighted_id`.
    #[must_use]
    pub fn selected_item(&self) -> Option<&PaletteItem<ItemId>> {
        let id = self.highlighted_id.as_ref()?;
        self.groups
            .iter()
            .flat_map(|g| &g.items)
            .find(|item| &item.id == id)
    }

    /// Two-stage escape handler (§3B.10, `UIF-COMP-010`).
    ///
    /// If `query` is non-empty, clears `query` to `""`, increments `generation`,
    /// and returns [`EscapeDisposition::ClearedQuery`].
    /// If `query` is already empty, returns [`EscapeDisposition::Dismiss`].
    pub fn handle_escape(&mut self) -> EscapeDisposition {
        if !self.query.is_empty() {
            self.query.clear();
            self.generation = self.generation.wrapping_add(1);
            EscapeDisposition::ClearedQuery
        } else {
            EscapeDisposition::Dismiss
        }
    }

    /// Update query from raw user text input, detecting leading mode prefix chars (`>`, `#`, `@`).
    ///
    /// - If input starts with a known prefix char, updates `mode`, strips prefix from query,
    ///   increments `generation`, and returns `Some(new_mode)` if mode changed.
    /// - If `input` is empty, `query` was already empty, and `mode != PaletteMode::Files`
    ///   (representing Backspace at caret 0 with active prefix), resets `mode` to `Files`,
    ///   increments `generation`, and returns `Some(PaletteMode::Files)`.
    /// - Otherwise, updates `query` to `input`, increments `generation`, and returns `None`.
    pub fn set_query_from_input(&mut self, input: &str) -> Option<PaletteMode> {
        let old_mode = self.mode;
        self.generation = self.generation.wrapping_add(1);

        if let Some(first_char) = input.chars().next()
            && let Some(mode_from_ch) = PaletteMode::from_prefix(first_char)
        {
            self.mode = mode_from_ch;
            let stripped = &input[first_char.len_utf8()..];
            self.query = stripped.to_string();
            if self.mode != old_mode {
                return Some(self.mode);
            }
            return None;
        }

        // Backspace at caret 0 with prefix active:
        if input.is_empty() && self.query.is_empty() && self.mode != PaletteMode::Files {
            self.mode = PaletteMode::Files;
            return Some(PaletteMode::Files);
        }

        self.query = input.to_string();
        None
    }

    /// Reset mode to [`PaletteMode::Files`], returning `Some(PaletteMode::Files)` if changed.
    pub fn backspace_prefix(&mut self) -> Option<PaletteMode> {
        if self.mode != PaletteMode::Files {
            self.mode = PaletteMode::Files;
            self.generation = self.generation.wrapping_add(1);
            Some(PaletteMode::Files)
        } else {
            None
        }
    }

    /// Programmatically close the palette.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Programmatically open the palette.
    pub fn open_palette(&mut self) {
        self.open = true;
    }
}

// ---------------------------------------------------------------------------
// Pure Layout and Highlighting Helpers
// ---------------------------------------------------------------------------

/// Top-centered modal card rectangle clamped to `viewport.width - 32px` (§3B.10, `UIF-COMP-010`).
#[must_use]
pub fn card_bounds(viewport: Rectangle) -> Rectangle {
    let width = CARD_WIDTH.min(viewport.width - 32.0).max(0.0);
    let height = CARD_HEIGHT.min(viewport.height - TOP_OFFSET).max(0.0);
    let x = viewport.x + (viewport.width - width) / 2.0;
    let y = viewport.y + TOP_OFFSET;
    Rectangle {
        x,
        y,
        width,
        height,
    }
}

/// Slices `text` into non-matching and matching segments based on `match_spans` (`UIF-COMP-010`).
#[must_use]
pub fn highlight_spans<'s>(text: &'s str, spans: &[TextRange]) -> Vec<(&'s str, bool)> {
    if spans.is_empty() {
        return vec![(text, false)];
    }

    let mut result = Vec::new();
    let mut cursor = 0;

    for span in spans {
        let start = span.start.min(text.len());
        let end = span.end.min(text.len()).max(start);

        if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            continue;
        }

        if start > cursor {
            result.push((&text[cursor..start], false));
        }
        if end > start {
            result.push((&text[start..end], true));
        }
        cursor = end;
    }

    if cursor < text.len() {
        result.push((&text[cursor..], false));
    }

    result
}

/// Calculate total content height for a list of groups.
#[must_use]
pub fn total_content_height<ItemId>(groups: &[PaletteGroup<ItemId>]) -> f32 {
    let mut total = 0.0;
    for group in groups {
        if !group.heading.is_empty() {
            total += GROUP_HEADER_HEIGHT;
        }
        for item in &group.items {
            total += if item.subtitle.is_some() {
                ROW_DOUBLE_HEIGHT
            } else {
                ROW_SINGLE_HEIGHT
            };
        }
    }
    total
}

// ---------------------------------------------------------------------------
// State Tree
// ---------------------------------------------------------------------------

/// Ephemeral interaction state stored in the widget tree for `CommandPalette`.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandPaletteState<ItemId> {
    /// Scroll offset within the middle result list.
    pub scroll_offset: f32,
    /// Id of item currently under cursor.
    pub hovered_id: Option<ItemId>,
    /// Whether cursor is hovering over the Esc hint chip.
    pub esc_hovered: bool,
    /// Whether cursor is hovering over the mode prefix chip.
    pub chip_hovered: bool,
}

impl<ItemId> CommandPaletteState<ItemId> {
    /// Construct empty state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scroll_offset: 0.0,
            hovered_id: None,
            esc_hovered: false,
            chip_hovered: false,
        }
    }
}

impl<ItemId> Default for CommandPaletteState<ItemId> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CommandPalette Widget
// ---------------------------------------------------------------------------

/// Query change callback handler type.
pub type QueryChangeHandler<'a, Message> =
    Box<dyn Fn(Live<(PaletteMode, String, u64)>) -> Message + 'a>;

/// The CommandPalette / SearchView composite widget (§3B.10, `UIF-COMP-010`).
pub struct CommandPalette<'a, Message, ItemId, Theme = iced::Theme, Renderer = iced::Renderer> {
    model: CommandPaletteModel<ItemId>,
    on_query_change: Option<QueryChangeHandler<'a, Message>>,
    on_mode_change: Option<Box<dyn Fn(Commit<PaletteMode>) -> Message + 'a>>,
    on_highlight_change: Option<Box<dyn Fn(Live<ItemId>) -> Message + 'a>>,
    on_execute: Option<Box<dyn Fn(Commit<ItemId>) -> Message + 'a>>,
    on_dismiss: Option<Box<dyn Fn(Cancel<()>) -> Message + 'a>>,
    palette: Option<Palette>,
    _marker: PhantomData<(&'a (), Theme, Renderer)>,
}

impl<'a, Message, ItemId: 'a + Clone + PartialEq, Theme, Renderer>
    CommandPalette<'a, Message, ItemId, Theme, Renderer>
{
    /// Construct a new command palette widget around `model`.
    #[must_use]
    pub fn new(model: CommandPaletteModel<ItemId>) -> Self {
        Self {
            model,
            on_query_change: None,
            on_mode_change: None,
            on_highlight_change: None,
            on_execute: None,
            on_dismiss: None,
            palette: None,
            _marker: PhantomData,
        }
    }

    /// Attach search query change callback emitting [`Live`].
    #[must_use]
    pub fn on_query_change(
        mut self,
        f: impl Fn(Live<(PaletteMode, String, u64)>) -> Message + 'a,
    ) -> Self {
        self.on_query_change = Some(Box::new(f));
        self
    }

    /// Attach mode change callback emitting [`Commit`].
    #[must_use]
    pub fn on_mode_change(mut self, f: impl Fn(Commit<PaletteMode>) -> Message + 'a) -> Self {
        self.on_mode_change = Some(Box::new(f));
        self
    }

    /// Attach highlight change callback emitting [`Live`].
    #[must_use]
    pub fn on_highlight_change(mut self, f: impl Fn(Live<ItemId>) -> Message + 'a) -> Self {
        self.on_highlight_change = Some(Box::new(f));
        self
    }

    /// Attach execution callback emitting [`Commit`].
    #[must_use]
    pub fn on_execute(mut self, f: impl Fn(Commit<ItemId>) -> Message + 'a) -> Self {
        self.on_execute = Some(Box::new(f));
        self
    }

    /// Attach dismiss callback emitting [`Cancel`].
    #[must_use]
    pub fn on_dismiss(mut self, f: impl Fn(Cancel<()>) -> Message + 'a) -> Self {
        self.on_dismiss = Some(Box::new(f));
        self
    }

    /// Override token palette.
    #[must_use]
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = Some(palette);
        self
    }

    /// Access the underlying model.
    #[must_use]
    pub fn model(&self) -> &CommandPaletteModel<ItemId> {
        &self.model
    }

    /// Whether the palette is currently open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.model.open
    }
}

/// Helper function to measure text width through paragraph layout.
fn measure_text_width<Renderer: text_core::Renderer<Font = iced::Font>>(
    _renderer: &Renderer,
    content: &str,
    size: f32,
    font: iced::Font,
) -> f32 {
    let paragraph = <Renderer::Paragraph as text_core::Paragraph>::with_text(text_core::Text {
        content,
        bounds: Size::INFINITE,
        size: Pixels(size),
        line_height: LineHeight::default(),
        font,
        align_x: text_core::Alignment::Left,
        align_y: alignment::Vertical::Top,
        shaping: text_core::Shaping::default(),
        wrapping: text_core::Wrapping::None,
    });
    let measured = paragraph.min_bounds().width;
    if measured > 0.0 {
        measured
    } else {
        content.len() as f32 * (size * 0.6)
    }
}

impl<Message, Theme, Renderer, ItemId> Widget<Message, Theme, Renderer>
    for CommandPalette<'_, Message, ItemId, Theme, Renderer>
where
    ItemId: 'static + Clone + PartialEq,
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<CommandPaletteState<ItemId>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(CommandPaletteState::<ItemId>::new())
    }

    fn children(&self) -> Vec<Tree> {
        Vec::new()
    }

    fn diff(&self, _tree: &mut Tree) {}

    fn size(&self) -> Size<Length> {
        if self.model.open {
            Size::new(Length::Fill, Length::Fill)
        } else {
            Size::new(Length::Fixed(0.0), Length::Fixed(0.0))
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        if !self.model.open {
            layout::Node::new(Size::ZERO)
        } else {
            layout::Node::new(limits.max())
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if !self.model.open {
            return;
        }

        let bounds = layout.bounds();
        let card_rect = card_bounds(bounds);
        let esc_rect = Rectangle {
            x: card_rect.x + card_rect.width - space::MD - 32.0,
            y: card_rect.y + (SEARCH_HEIGHT - 20.0) / 2.0,
            width: 32.0,
            height: 20.0,
        };
        let chip_rect = Rectangle {
            x: card_rect.x + space::MD + ICON_SIZE + space::SM,
            y: card_rect.y + (SEARCH_HEIGHT - 24.0) / 2.0,
            width: 28.0,
            height: 24.0,
        };
        let results_clip = Rectangle {
            x: card_rect.x,
            y: card_rect.y + SEARCH_HEIGHT + DIVIDER_STROKE,
            width: card_rect.width,
            height: (card_rect.height
                - SEARCH_HEIGHT
                - DIVIDER_STROKE
                - FOOTER_HEIGHT
                - DIVIDER_STROKE)
                .max(0.0),
        };

        let state = tree.state.downcast_mut::<CommandPaletteState<ItemId>>();
        let total_h = total_content_height(&self.model.groups);
        let max_scroll = (total_h - results_clip.height).max(0.0);
        state.scroll_offset = state.scroll_offset.clamp(0.0, max_scroll);

        // Hover tracking
        state.esc_hovered = cursor.is_over(esc_rect);
        state.chip_hovered = self.model.mode.prefix().is_some() && cursor.is_over(chip_rect);

        // Find hovered item
        state.hovered_id = None;
        if cursor.is_over(results_clip)
            && let Some(pos) = cursor.position()
        {
            let mut curr_y = results_clip.y - state.scroll_offset;
            'outer: for group in &self.model.groups {
                if !group.heading.is_empty() {
                    curr_y += GROUP_HEADER_HEIGHT;
                }
                for item in &group.items {
                    let row_h = if item.subtitle.is_some() {
                        ROW_DOUBLE_HEIGHT
                    } else {
                        ROW_SINGLE_HEIGHT
                    };
                    let item_rect = Rectangle {
                        x: results_clip.x + space::XS,
                        y: curr_y,
                        width: (results_clip.width - space::XS * 2.0).max(0.0),
                        height: row_h,
                    };
                    if item_rect.contains(pos) {
                        state.hovered_id = Some(item.id.clone());
                        break 'outer;
                    }
                    curr_y += row_h;
                }
            }
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if cursor.is_over(esc_rect) {
                    if let Some(on_dismiss) = &self.on_dismiss {
                        shell.publish((on_dismiss)(Cancel::new(())));
                        shell.capture_event();
                    }
                } else if self.model.mode.prefix().is_some() && cursor.is_over(chip_rect) {
                    if let Some(on_mode) = &self.on_mode_change {
                        shell.publish((on_mode)(Commit::new(PaletteMode::Files)));
                        shell.capture_event();
                    }
                } else if cursor.is_over(results_clip) {
                    if let Some(ref hit_id) = state.hovered_id {
                        if let Some(on_highlight) = &self.on_highlight_change {
                            shell.publish((on_highlight)(Live::new(hit_id.clone())));
                        }
                        if let Some(on_execute) = &self.on_execute {
                            shell.publish((on_execute)(Commit::new(hit_id.clone())));
                        }
                        shell.capture_event();
                    }
                } else if cursor.is_over(bounds) && !cursor.is_over(card_rect) {
                    // Click on scrim dismisses modal (§3B.10, Escape Rank 2)
                    if let Some(on_dismiss) = &self.on_dismiss {
                        shell.publish((on_dismiss)(Cancel::new(())));
                        shell.capture_event();
                    }
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.is_over(results_clip) {
                    let y_delta = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => *y * ROW_SINGLE_HEIGHT,
                        mouse::ScrollDelta::Pixels { y, .. } => *y,
                    };
                    state.scroll_offset = (state.scroll_offset - y_delta).clamp(0.0, max_scroll);
                    shell.capture_event();
                }
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => match key {
                Key::Named(Named::Escape) => {
                    if !self.model.query.is_empty() {
                        if let Some(on_query) = &self.on_query_change {
                            shell.publish((on_query)(Live::new((
                                self.model.mode,
                                String::new(),
                                self.model.generation + 1,
                            ))));
                        }
                    } else if let Some(on_dismiss) = &self.on_dismiss {
                        shell.publish((on_dismiss)(Cancel::new(())));
                    }
                    shell.capture_event();
                }
                Key::Named(Named::ArrowDown) => {
                    let items = self.model.all_items();
                    if !items.is_empty() {
                        let next_id = match &self.model.highlighted_id {
                            Some(current_id) => {
                                if let Some(pos) = items.iter().position(|i| &i.id == current_id) {
                                    let next_idx = (pos + 1) % items.len();
                                    items[next_idx].id.clone()
                                } else {
                                    items[0].id.clone()
                                }
                            }
                            None => items[0].id.clone(),
                        };
                        if let Some(on_highlight) = &self.on_highlight_change {
                            shell.publish((on_highlight)(Live::new(next_id)));
                        }
                    }
                    shell.capture_event();
                }
                Key::Named(Named::ArrowUp) => {
                    let items = self.model.all_items();
                    if !items.is_empty() {
                        let prev_id = match &self.model.highlighted_id {
                            Some(current_id) => {
                                if let Some(pos) = items.iter().position(|i| &i.id == current_id) {
                                    let prev_idx = if pos == 0 { items.len() - 1 } else { pos - 1 };
                                    items[prev_idx].id.clone()
                                } else {
                                    items[items.len() - 1].id.clone()
                                }
                            }
                            None => items[items.len() - 1].id.clone(),
                        };
                        if let Some(on_highlight) = &self.on_highlight_change {
                            shell.publish((on_highlight)(Live::new(prev_id)));
                        }
                    }
                    shell.capture_event();
                }
                Key::Named(Named::Enter) => {
                    if let Some(item) = self.model.selected_item()
                        && let Some(on_execute) = &self.on_execute
                    {
                        shell.publish((on_execute)(Commit::new(item.id.clone())));
                    }
                    shell.capture_event();
                }
                Key::Named(Named::Backspace) => {
                    if self.model.query.is_empty() {
                        if self.model.mode != PaletteMode::Files
                            && let Some(on_mode) = &self.on_mode_change
                        {
                            shell.publish((on_mode)(Commit::new(PaletteMode::Files)));
                        }
                    } else {
                        let mut new_query = self.model.query.clone();
                        new_query.pop();
                        if let Some(on_query) = &self.on_query_change {
                            shell.publish((on_query)(Live::new((
                                self.model.mode,
                                new_query,
                                self.model.generation + 1,
                            ))));
                        }
                    }
                    shell.capture_event();
                }
                Key::Character(ch_str) => {
                    let mut input = self.model.query.clone();
                    input.push_str(ch_str.as_str());
                    let mut model_copy = self.model.clone();
                    let maybe_new_mode = model_copy.set_query_from_input(&input);
                    if let Some(new_mode) = maybe_new_mode
                        && let Some(on_mode) = &self.on_mode_change
                    {
                        shell.publish((on_mode)(Commit::new(new_mode)));
                    }
                    if let Some(on_query) = &self.on_query_change {
                        shell.publish((on_query)(Live::new((
                            model_copy.mode,
                            model_copy.query,
                            model_copy.generation,
                        ))));
                    }
                    shell.capture_event();
                }
                _ => {}
            },
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
        if !self.model.open {
            return;
        }

        let palette = self.palette.unwrap_or_else(Palette::dark);
        let bounds = layout.bounds();
        let state = tree.state.downcast_ref::<CommandPaletteState<ItemId>>();

        // 1. Full-window Scrim (Z-Band 3 / surface.scrim)
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            iced::Background::Color(palette.surface_scrim),
        );

        // 2. Card bounds & surface.overlay background
        let card_rect = card_bounds(bounds);
        renderer.fill_quad(
            renderer::Quad {
                bounds: card_rect,
                border: iced::Border {
                    color: palette.border_strong,
                    width: shape::BORDER_HAIRLINE,
                    radius: RADIUS.into(),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            },
            iced::Background::Color(palette.surface_overlay),
        );

        // 3. Search header: 40px height
        let search_rect = Rectangle {
            x: card_rect.x,
            y: card_rect.y,
            width: card_rect.width,
            height: SEARCH_HEIGHT,
        };

        // Leading 16px search glyph
        let icon_rect = Rectangle {
            x: search_rect.x + space::MD,
            y: search_rect.y + (SEARCH_HEIGHT - ICON_SIZE) / 2.0,
            width: ICON_SIZE,
            height: ICON_SIZE,
        };
        renderer.fill_text(
            text_core::Text {
                content: GLYPH_SEARCH.to_string(),
                bounds: icon_rect.size(),
                size: Pixels(ICON_SIZE),
                line_height: LineHeight::Absolute(Pixels(ICON_SIZE)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            icon_rect.center(),
            palette.text_muted,
            *viewport,
        );

        let mut text_x = icon_rect.x + ICON_SIZE + space::SM;

        // Mode prefix chip if active
        if let Some(prefix_ch) = self.model.mode.prefix() {
            let chip_w = 28.0;
            let chip_h = 24.0;
            let chip_rect = Rectangle {
                x: text_x,
                y: search_rect.y + (SEARCH_HEIGHT - chip_h) / 2.0,
                width: chip_w,
                height: chip_h,
            };
            let chip_bg = if state.chip_hovered {
                palette.surface_active
            } else {
                palette.surface_hover
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: chip_rect,
                    border: iced::Border {
                        color: palette.border_subtle,
                        width: shape::BORDER_HAIRLINE,
                        radius: shape::RADIUS_WIDGET.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
                iced::Background::Color(chip_bg),
            );
            renderer.fill_text(
                text_core::Text {
                    content: prefix_ch.to_string(),
                    bounds: chip_rect.size(),
                    size: Pixels(typography::CAPTION.size),
                    line_height: LineHeight::Absolute(Pixels(chip_h)),
                    font: typography::CAPTION
                        .font(typography::FAMILY_MONO, typography::Weight::SemiBold),
                    align_x: text_core::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                    shaping: text_core::Shaping::default(),
                    wrapping: text_core::Wrapping::None,
                },
                chip_rect.center(),
                palette.accent_text,
                *viewport,
            );
            text_x += chip_w + space::SM;
        }

        // Trailing Esc hint chip (Mono caption)
        let esc_w = 32.0;
        let esc_h = 20.0;
        let esc_rect = Rectangle {
            x: search_rect.x + search_rect.width - space::MD - esc_w,
            y: search_rect.y + (SEARCH_HEIGHT - esc_h) / 2.0,
            width: esc_w,
            height: esc_h,
        };
        let esc_bg = if state.esc_hovered {
            palette.surface_active
        } else {
            palette.surface_hover
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: esc_rect,
                border: iced::Border {
                    color: palette.border_subtle,
                    width: shape::BORDER_HAIRLINE,
                    radius: shape::RADIUS_WIDGET.into(),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            },
            iced::Background::Color(esc_bg),
        );
        renderer.fill_text(
            text_core::Text {
                content: LABEL_ESC.to_string(),
                bounds: esc_rect.size(),
                size: Pixels(typography::CAPTION.size),
                line_height: LineHeight::Absolute(Pixels(esc_h)),
                font: typography::CAPTION
                    .font(typography::FAMILY_MONO, typography::Weight::Regular),
                align_x: text_core::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            esc_rect.center(),
            palette.text_muted,
            *viewport,
        );

        // Search text / placeholder
        let query_w = (esc_rect.x - text_x - space::SM).max(0.0);
        let query_rect = Rectangle {
            x: text_x,
            y: search_rect.y,
            width: query_w,
            height: SEARCH_HEIGHT,
        };

        let (display_text, text_color) = if self.model.query.is_empty() {
            (self.model.mode.placeholder(), palette.text_disabled)
        } else {
            (self.model.query.as_str(), palette.text_primary)
        };

        renderer.fill_text(
            text_core::Text {
                content: display_text.to_string(),
                bounds: query_rect.size(),
                size: Pixels(typography::UI_COMPACT.size),
                line_height: LineHeight::Absolute(Pixels(SEARCH_HEIGHT)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Left,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            Point::new(query_rect.x, query_rect.center_y()),
            text_color,
            *viewport,
        );

        // 4. Hairline divider below search
        let divider1_y = card_rect.y + SEARCH_HEIGHT;
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: card_rect.x,
                    y: divider1_y,
                    width: card_rect.width,
                    height: DIVIDER_STROKE,
                },
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            iced::Background::Color(palette.border_subtle),
        );

        // 5. Footer divider & keyboard legend
        let footer_y = card_rect.y + card_rect.height - FOOTER_HEIGHT;
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: card_rect.x,
                    y: footer_y - DIVIDER_STROKE,
                    width: card_rect.width,
                    height: DIVIDER_STROKE,
                },
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            iced::Background::Color(palette.border_subtle),
        );

        let footer_text_rect = Rectangle {
            x: card_rect.x + space::MD,
            y: footer_y,
            width: card_rect.width - space::MD * 2.0,
            height: FOOTER_HEIGHT,
        };
        renderer.fill_text(
            text_core::Text {
                content: LEGEND_KEYBOARD.to_string(),
                bounds: footer_text_rect.size(),
                size: Pixels(typography::CAPTION.size),
                line_height: LineHeight::Absolute(Pixels(FOOTER_HEIGHT)),
                font: renderer.default_font(),
                align_x: text_core::Alignment::Left,
                align_y: alignment::Vertical::Center,
                shaping: text_core::Shaping::default(),
                wrapping: text_core::Wrapping::None,
            },
            Point::new(footer_text_rect.x, footer_text_rect.center_y()),
            palette.text_muted,
            *viewport,
        );

        // 6. Scrollable result list area
        let results_clip = Rectangle {
            x: card_rect.x,
            y: divider1_y + DIVIDER_STROKE,
            width: card_rect.width,
            height: (footer_y - DIVIDER_STROKE - (divider1_y + DIVIDER_STROKE)).max(0.0),
        };

        if self.model.busy {
            renderer.fill_text(
                text_core::Text {
                    content: SEARCHING.to_string(),
                    bounds: results_clip.size(),
                    size: Pixels(typography::CAPTION.size),
                    line_height: LineHeight::Absolute(Pixels(results_clip.height)),
                    font: renderer.default_font(),
                    align_x: text_core::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                    shaping: text_core::Shaping::default(),
                    wrapping: text_core::Wrapping::None,
                },
                results_clip.center(),
                palette.text_muted,
                results_clip,
            );
            return;
        }

        if self.model.item_count() == 0 {
            renderer.fill_text(
                text_core::Text {
                    content: NO_RESULTS.to_string(),
                    bounds: results_clip.size(),
                    size: Pixels(typography::CAPTION.size),
                    line_height: LineHeight::Absolute(Pixels(results_clip.height)),
                    font: renderer.default_font(),
                    align_x: text_core::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                    shaping: text_core::Shaping::default(),
                    wrapping: text_core::Wrapping::None,
                },
                results_clip.center(),
                palette.text_muted,
                results_clip,
            );
            return;
        }

        let mut curr_y = results_clip.y - state.scroll_offset;

        for group in &self.model.groups {
            if !group.heading.is_empty() {
                let header_rect = Rectangle {
                    x: results_clip.x + space::MD,
                    y: curr_y,
                    width: (results_clip.width - space::MD * 2.0).max(0.0),
                    height: GROUP_HEADER_HEIGHT,
                };
                if curr_y + GROUP_HEADER_HEIGHT >= results_clip.y
                    && curr_y <= results_clip.y + results_clip.height
                {
                    renderer.fill_text(
                        text_core::Text {
                            content: group.heading.clone(),
                            bounds: header_rect.size(),
                            size: Pixels(typography::CAPTION.size),
                            line_height: LineHeight::Absolute(Pixels(GROUP_HEADER_HEIGHT)),
                            font: iced::Font {
                                weight: iced::font::Weight::Semibold,
                                ..renderer.default_font()
                            },
                            align_x: text_core::Alignment::Left,
                            align_y: alignment::Vertical::Center,
                            shaping: text_core::Shaping::default(),
                            wrapping: text_core::Wrapping::None,
                        },
                        Point::new(header_rect.x, header_rect.center_y()),
                        palette.text_muted,
                        results_clip,
                    );
                }
                curr_y += GROUP_HEADER_HEIGHT;
            }

            for item in &group.items {
                let is_double = item.subtitle.is_some();
                let row_h = if is_double {
                    ROW_DOUBLE_HEIGHT
                } else {
                    ROW_SINGLE_HEIGHT
                };
                let item_rect = Rectangle {
                    x: results_clip.x + space::XS,
                    y: curr_y,
                    width: (results_clip.width - space::XS * 2.0).max(0.0),
                    height: row_h,
                };

                let is_active = self.model.highlighted_id.as_ref() == Some(&item.id);
                let is_hovered = state.hovered_id.as_ref() == Some(&item.id);

                if curr_y + row_h >= results_clip.y
                    && curr_y <= results_clip.y + results_clip.height
                {
                    // Item background
                    if is_active {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: item_rect,
                                border: iced::Border {
                                    radius: shape::RADIUS_WIDGET.into(),
                                    ..Default::default()
                                },
                                shadow: iced::Shadow::default(),
                                snap: true,
                            },
                            iced::Background::Color(palette.accent_fill),
                        );
                    } else if is_hovered {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: item_rect,
                                border: iced::Border {
                                    radius: shape::RADIUS_WIDGET.into(),
                                    ..Default::default()
                                },
                                shadow: iced::Shadow::default(),
                                snap: true,
                            },
                            iced::Background::Color(palette.surface_hover),
                        );
                    }

                    // Rule 4 foreground lock: on accent_fill, text & subtitle are promoted to text_strong!
                    let (title_fg, subtitle_fg, shortcut_fg, match_fg) = if is_active {
                        (
                            palette.text_strong,
                            palette.text_strong,
                            palette.text_strong,
                            palette.text_strong,
                        )
                    } else {
                        (
                            palette.text_primary,
                            palette.text_muted,
                            palette.text_muted,
                            palette.accent_text,
                        )
                    };

                    // Shortcut pill on the right
                    let mut shortcut_reserved = 0.0;
                    if let Some(sc) = &item.shortcut {
                        let sc_font = typography::CAPTION
                            .font(typography::FAMILY_MONO, typography::Weight::Regular);
                        let sc_w =
                            measure_text_width(renderer, sc, typography::CAPTION.size, sc_font);
                        shortcut_reserved = sc_w + space::MD;
                        let sc_rect = Rectangle {
                            x: item_rect.x + item_rect.width - space::MD - sc_w,
                            y: item_rect.y,
                            width: sc_w,
                            height: row_h,
                        };
                        renderer.fill_text(
                            text_core::Text {
                                content: sc.clone(),
                                bounds: sc_rect.size(),
                                size: Pixels(typography::CAPTION.size),
                                line_height: LineHeight::Absolute(Pixels(row_h)),
                                font: sc_font,
                                align_x: text_core::Alignment::Right,
                                align_y: alignment::Vertical::Center,
                                shaping: text_core::Shaping::default(),
                                wrapping: text_core::Wrapping::None,
                            },
                            Point::new(sc_rect.x + sc_rect.width, sc_rect.center_y()),
                            shortcut_fg,
                            results_clip,
                        );
                    }

                    let content_x = item_rect.x + space::MD;
                    let content_w =
                        (item_rect.width - space::MD * 2.0 - shortcut_reserved).max(0.0);

                    if let Some(sub) = &item.subtitle {
                        // Two-line layout (44px)
                        let title_y = item_rect.y + 4.0;
                        let title_h = 18.0;
                        let sub_y = item_rect.y + 22.0;
                        let sub_h = 16.0;

                        // Title with matched spans
                        let spans = highlight_spans(&item.title, &item.match_spans);
                        let mut span_x = content_x;
                        for (slice, is_match) in spans {
                            let (f_weight, f_color) = if is_match {
                                (typography::Weight::SemiBold, match_fg)
                            } else {
                                (typography::Weight::Regular, title_fg)
                            };
                            let font = iced::Font {
                                weight: f_weight.iced(),
                                ..renderer.default_font()
                            };
                            let span_w = measure_text_width(
                                renderer,
                                slice,
                                typography::UI_COMPACT.size,
                                font,
                            );
                            let span_rect = Rectangle {
                                x: span_x,
                                y: title_y,
                                width: span_w,
                                height: title_h,
                            };
                            renderer.fill_text(
                                text_core::Text {
                                    content: slice.to_string(),
                                    bounds: span_rect.size(),
                                    size: Pixels(typography::UI_COMPACT.size),
                                    line_height: LineHeight::Absolute(Pixels(title_h)),
                                    font,
                                    align_x: text_core::Alignment::Left,
                                    align_y: alignment::Vertical::Center,
                                    shaping: text_core::Shaping::default(),
                                    wrapping: text_core::Wrapping::None,
                                },
                                Point::new(span_rect.x, span_rect.center_y()),
                                f_color,
                                results_clip,
                            );
                            span_x += span_w;
                        }

                        // Subtitle
                        let sub_rect = Rectangle {
                            x: content_x,
                            y: sub_y,
                            width: content_w,
                            height: sub_h,
                        };
                        renderer.fill_text(
                            text_core::Text {
                                content: sub.clone(),
                                bounds: sub_rect.size(),
                                size: Pixels(typography::CAPTION.size),
                                line_height: LineHeight::Absolute(Pixels(sub_h)),
                                font: renderer.default_font(),
                                align_x: text_core::Alignment::Left,
                                align_y: alignment::Vertical::Center,
                                shaping: text_core::Shaping::default(),
                                wrapping: text_core::Wrapping::None,
                            },
                            Point::new(sub_rect.x, sub_rect.center_y()),
                            subtitle_fg,
                            results_clip,
                        );
                    } else {
                        // Single-line layout (32px)
                        let spans = highlight_spans(&item.title, &item.match_spans);
                        let mut span_x = content_x;
                        for (slice, is_match) in spans {
                            let (f_weight, f_color) = if is_match {
                                (typography::Weight::SemiBold, match_fg)
                            } else {
                                (typography::Weight::Regular, title_fg)
                            };
                            let font = iced::Font {
                                weight: f_weight.iced(),
                                ..renderer.default_font()
                            };
                            let span_w = measure_text_width(
                                renderer,
                                slice,
                                typography::UI_COMPACT.size,
                                font,
                            );
                            let span_rect = Rectangle {
                                x: span_x,
                                y: item_rect.y,
                                width: span_w,
                                height: row_h,
                            };
                            renderer.fill_text(
                                text_core::Text {
                                    content: slice.to_string(),
                                    bounds: span_rect.size(),
                                    size: Pixels(typography::UI_COMPACT.size),
                                    line_height: LineHeight::Absolute(Pixels(row_h)),
                                    font,
                                    align_x: text_core::Alignment::Left,
                                    align_y: alignment::Vertical::Center,
                                    shaping: text_core::Shaping::default(),
                                    wrapping: text_core::Wrapping::None,
                                },
                                Point::new(span_rect.x, span_rect.center_y()),
                                f_color,
                                results_clip,
                            );
                            span_x += span_w;
                        }
                    }
                }

                curr_y += row_h;
            }
        }
    }
}

impl<'a, Message: 'a, ItemId: 'static + Clone + PartialEq, Theme: 'a, Renderer: 'a>
    From<CommandPalette<'a, Message, ItemId, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn from(palette: CommandPalette<'a, Message, ItemId, Theme, Renderer>) -> Self {
        Element::new(palette)
    }
}

/// Construct a `CommandPalette` composite widget (§3B.10, `UIF-COMP-010`).
#[must_use]
pub fn command_palette<'a, Message, ItemId: 'static + Clone + PartialEq, Theme, Renderer>(
    model: CommandPaletteModel<ItemId>,
) -> CommandPalette<'a, Message, ItemId, Theme, Renderer> {
    CommandPalette::new(model)
}

#[cfg(test)]
#[path = "command_palette_tests.rs"]
mod command_palette_tests;
