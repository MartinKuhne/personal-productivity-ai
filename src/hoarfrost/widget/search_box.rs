//! Pinned search box widget for tree views and search panels (`UIF-PANE-003`, `UIF-PRIN-006`).
//!
//! Encloses a 32px height text input with a trailing 28x28px action button that swaps
//! between a passive search glyph `🔍` when idle and an actionable clear `×` when a query or filter is active.
//!
//! Unit tests live in the sibling `search_box_tests.rs` sidecar.

use iced::alignment;
use iced::{Border, Color, Element, Length, Pixels, Shadow};

use crate::token::{color::Palette, shape, size, space, typography};

/// Height of the search input field in pixels (`size.control.md` = 32px, `UIF-PANE-003`).
pub const SEARCH_INPUT_HEIGHT: f32 = size::CONTROL_MD;

/// Size of the trailing search / clear action button in pixels (`size.control.sm` = 28px, `UIF-PANE-003`).
pub const SEARCH_ACTION_SIZE: f32 = size::CONTROL_SM;

/// Default placeholder text for search inputs (`UIF-PANE-003`).
pub const SEARCH_PLACEHOLDER: &str = "Search content…";

/// Search glyph displayed when idle (`UIF-PANE-003`).
pub const GLYPH_SEARCH: char = '🔍';

/// Clear glyph displayed when a search filter or query is active (`UIF-PANE-003`).
pub const GLYPH_CLEAR: char = '×';

/// Declarative state model for a search box (`UIF-PANE-003`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchBoxModel {
    /// The current search query string.
    pub query: String,
    /// Explicit flag indicating whether a filter is active even if `query` is empty.
    pub is_active_override: bool,
}

impl SearchBoxModel {
    /// Create a new model with `query`.
    #[must_use]
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            is_active_override: false,
        }
    }

    /// Set an explicit filter-active override.
    #[must_use]
    pub fn with_active(mut self, active: bool) -> Self {
        self.is_active_override = active;
        self
    }

    /// Whether this search box is currently active (has query text or explicit active flag).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.is_active_override || !self.query.is_empty()
    }

    /// Resolve the trailing button glyph (`🔍` when idle, `×` when active).
    #[must_use]
    pub fn trailing_glyph(&self) -> char {
        if self.is_active() {
            GLYPH_CLEAR
        } else {
            GLYPH_SEARCH
        }
    }
}

/// Search box widget combining text entry with trailing action button (`UIF-PANE-003`).
pub struct SearchBox<'a, Message> {
    query: String,
    placeholder: String,
    is_active_override: bool,
    palette: Palette,
    on_input: Option<Box<dyn Fn(String) -> Message + 'a>>,
    on_submit: Option<Message>,
    on_clear: Option<Message>,
}

impl<'a, Message: 'a + Clone> SearchBox<'a, Message> {
    /// Create a new search box widget for `query`.
    #[must_use]
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            placeholder: SEARCH_PLACEHOLDER.to_string(),
            is_active_override: false,
            palette: Palette::dark(),
            on_input: None,
            on_submit: None,
            on_clear: None,
        }
    }

    /// Set custom placeholder text.
    #[must_use]
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Set an explicit active filter flag.
    #[must_use]
    pub fn with_active(mut self, active: bool) -> Self {
        self.is_active_override = active;
        self
    }

    /// Set the design token color palette.
    #[must_use]
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Set the text input change callback.
    #[must_use]
    pub fn on_input(mut self, f: impl Fn(String) -> Message + 'a) -> Self {
        self.on_input = Some(Box::new(f));
        self
    }

    /// Set the action emitted when Enter is pressed.
    #[must_use]
    pub fn on_submit(mut self, msg: Message) -> Self {
        self.on_submit = Some(msg);
        self
    }

    /// Set the action emitted when the trailing clear button is pressed.
    #[must_use]
    pub fn on_clear(mut self, msg: Message) -> Self {
        self.on_clear = Some(msg);
        self
    }
}

impl<'a, Message: 'a + Clone> From<SearchBox<'a, Message>> for Element<'a, Message> {
    fn from(search_box: SearchBox<'a, Message>) -> Self {
        let palette = search_box.palette;
        let is_active = search_box.is_active_override || !search_box.query.is_empty();
        let trailing_glyph = if is_active { GLYPH_CLEAR } else { GLYPH_SEARCH };

        let mut input = iced::widget::text_input(&search_box.placeholder, &search_box.query)
            .size(Pixels(typography::UI_COMPACT.size))
            .padding(iced::Padding::new(space::XXS).horizontal(space::XS));

        if let Some(on_input) = search_box.on_input {
            input = input.on_input(on_input);
        }

        if let Some(on_submit) = search_box.on_submit {
            input = input.on_submit(on_submit);
        }

        let input_el: Element<'a, Message> = input.into();

        let mut action_btn = iced::widget::button(
            iced::widget::container(
                iced::widget::text(trailing_glyph.to_string())
                    .size(Pixels(typography::CAPTION.size))
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center),
            )
            .width(Length::Fixed(SEARCH_ACTION_SIZE))
            .height(Length::Fixed(SEARCH_ACTION_SIZE))
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center),
        )
        .padding(iced::Padding::ZERO)
        .width(Length::Fixed(SEARCH_ACTION_SIZE))
        .height(Length::Fixed(SEARCH_ACTION_SIZE))
        .style(move |_theme, status| {
            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
            let bg = if is_hovered {
                palette.surface_hover
            } else {
                Color::TRANSPARENT
            };
            let text_color = if is_hovered {
                palette.text_strong
            } else {
                palette.text_muted
            };
            iced::widget::button::Style {
                background: Some(bg.into()),
                text_color,
                border: Border::default().rounded(shape::RADIUS_WIDGET),
                shadow: Shadow::default(),
                snap: true,
            }
        });

        if is_active && let Some(on_clear) = search_box.on_clear {
            action_btn = action_btn.on_press(on_clear);
        }

        iced::widget::row![input_el, action_btn,]
            .spacing(space::XS)
            .align_y(alignment::Vertical::Center)
            .height(Length::Fixed(SEARCH_INPUT_HEIGHT))
            .into()
    }
}

#[cfg(test)]
#[path = "search_box_tests.rs"]
mod search_box_tests;
