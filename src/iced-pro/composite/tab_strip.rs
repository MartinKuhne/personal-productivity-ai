//! Tab strip composite with dirty close guards, MRU focus handoff, and external-deletion sync (`UIF-TAB-003`).
//!
//! The tab strip manages open document tabs, tracking their active state, unsaved-change ("dirty") status,
//! and most-recently-used (MRU) activation history. When a tab is closed, a dirty close guard intercepts
//! tabs with unsaved edits to raise confirmation dialogs, while external file deletions automatically
//! close the corresponding tab and transfer focus to the most-recently-active remaining document.
//!
//! Unit tests live in the sibling `tab_strip_tests.rs` sidecar.

use iced::alignment;
use iced::{Border, Color, Element, Length, Pixels, Shadow};

use crate::surface::tooltip::Tooltip;
use crate::token::{color::Palette, shape, size, space, typography};

/// Standard height of a tab strip in pixels (`size.control.md` - 32px).
pub const TAB_STRIP_HEIGHT: f32 = size::CONTROL_MD;

/// Minimum width of an individual tab in pixels (64px).
pub const TAB_MIN_WIDTH: f32 = 64.0;

/// Maximum width of an individual tab in pixels (200px).
pub const TAB_MAX_WIDTH: f32 = 200.0;

/// Diameter of the unsaved-state dirty dot in pixels (`size.indicator.dot` - 6px).
pub const DIRTY_DOT_DIAMETER: f32 = size::INDICATOR_DOT;

/// Size of the tab close button in pixels (`size.icon.md` - 16px).
pub const CLOSE_BUTTON_SIZE: f32 = size::ICON_MD;

/// Close verdict returned when a tab close is requested (`UIF-TAB-003`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TabCloseVerdict {
    /// The tab was clean and closed immediately, yielding the new active tab ID if any.
    CloseImmediately {
        /// The new active tab ID, chosen from MRU history.
        new_active_id: Option<String>,
    },
    /// The tab has unsaved changes and requires user confirmation before closing (`UIF-MOD-002`).
    RequiresConfirmation {
        /// The ID of the dirty tab.
        tab_id: String,
        /// The title of the dirty tab.
        title: String,
    },
}

/// A single tab item within a [`TabStripModel`] (`UIF-TAB-003`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabItem {
    /// Unique identifier for this tab (often a document path or UUID).
    pub id: String,
    /// Display title of the tab.
    pub title: String,
    /// Optional tooltip text displaying full file path or details.
    pub tooltip: Option<String>,
    /// Whether this tab has unsaved changes.
    pub is_dirty: bool,
    /// Whether this tab is pinned to the leading edge of the strip.
    pub is_pinned: bool,
}

impl TabItem {
    /// Create a new tab item with `id` and `title`.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            tooltip: None,
            is_dirty: false,
            is_pinned: false,
        }
    }

    /// Set whether the tab has unsaved modifications.
    #[must_use]
    pub fn with_dirty(mut self, dirty: bool) -> Self {
        self.is_dirty = dirty;
        self
    }

    /// Set an explicit tooltip path or description.
    #[must_use]
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Set whether the tab is pinned.
    #[must_use]
    pub fn with_pinned(mut self, pinned: bool) -> Self {
        self.is_pinned = pinned;
        self
    }
}

/// Declarative model governing a tab strip's tabs, selection, MRU history, and close guards (`UIF-TAB-003`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TabStripModel {
    /// Ordered list of open tabs.
    pub tabs: Vec<TabItem>,
    /// Identifier of the currently active tab.
    pub active_id: Option<String>,
    /// Most-recently-used activation history, newest at the end.
    pub mru_history: Vec<String>,
}

impl TabStripModel {
    /// Create an empty tab strip model.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a tab to the strip and select it.
    pub fn open_tab(&mut self, tab: TabItem) {
        let id = tab.id.clone();
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs[pos] = tab;
        } else {
            self.tabs.push(tab);
        }
        self.select_tab(&id);
    }

    /// Select the tab identified by `id`, recording it in the MRU history.
    pub fn select_tab(&mut self, id: &str) {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active_id = Some(id.to_string());
            self.mru_history.retain(|existing| existing != id);
            self.mru_history.push(id.to_string());
        }
    }

    /// Request closing a tab by `id` (`UIF-TAB-003`).
    ///
    /// If the tab is dirty (`is_dirty == true`), returns [`TabCloseVerdict::RequiresConfirmation`]
    /// so the host can display a save/discard modal.
    /// If the tab is clean, it is closed immediately, and focus hands off to the most recently
    /// active remaining tab per MRU history.
    pub fn request_close(&mut self, id: &str) -> TabCloseVerdict {
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else {
            return TabCloseVerdict::CloseImmediately {
                new_active_id: self.active_id.clone(),
            };
        };

        if tab.is_dirty {
            TabCloseVerdict::RequiresConfirmation {
                tab_id: tab.id.clone(),
                title: tab.title.clone(),
            }
        } else {
            let new_active_id = self.close_tab_internal(id);
            TabCloseVerdict::CloseImmediately { new_active_id }
        }
    }

    /// Confirm and force-close a tab even if it has unsaved changes (`UIF-TAB-003`).
    pub fn confirm_close(&mut self, id: &str) -> Option<String> {
        self.close_tab_internal(id)
    }

    /// Synchronize external file deletion (`UIF-TAB-003`).
    ///
    /// When an underlying file is externally deleted, its tab closes immediately without
    /// blocking on user confirmation, and activation transfers to the most-recently-active
    /// remaining tab.
    pub fn external_deletion_sync(&mut self, vanished_path: &str) -> Option<String> {
        self.close_tab_internal(vanished_path)
    }

    /// Collect all dirty tabs that require confirmation before closing all tabs.
    #[must_use]
    pub fn request_close_all(&self) -> Vec<String> {
        self.tabs
            .iter()
            .filter(|t| t.is_dirty)
            .map(|t| t.id.clone())
            .collect()
    }

    /// Internal close implementation removing the tab and restoring the next MRU active tab.
    fn close_tab_internal(&mut self, id: &str) -> Option<String> {
        self.tabs.retain(|t| t.id != id);
        self.mru_history.retain(|existing| existing != id);

        if self.active_id.as_deref() == Some(id) {
            self.active_id = self
                .mru_history
                .iter()
                .rev()
                .find(|candidate| self.tabs.iter().any(|t| t.id == **candidate))
                .cloned()
                .or_else(|| self.tabs.first().map(|t| t.id.clone()));
        }

        self.active_id.clone()
    }
}

/// Visual styling of a single tab item (`UIF-TAB-003`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabItemStyle {
    /// Background fill of the tab.
    pub background: Color,
    /// Perimeter border of the tab.
    pub border: Border,
    /// Label text color.
    pub text_color: Color,
    /// Dirty indicator color.
    pub dirty_color: Color,
}

/// Resolve the styling of a tab item (`UIF-TAB-003`).
#[must_use]
pub fn resolve_tab_style(
    palette: &Palette,
    is_active: bool,
    is_hovered: bool,
    is_dirty: bool,
) -> TabItemStyle {
    let background = if is_active {
        palette.surface_raised
    } else if is_hovered {
        palette.surface_hover
    } else {
        Color::TRANSPARENT
    };

    let border = if is_active {
        Border::default()
            .color(palette.border_strong)
            .width(1.0)
            .rounded(shape::RADIUS_WIDGET)
    } else {
        Border::default()
    };

    let text_color = if is_active {
        palette.text_strong
    } else {
        palette.text_primary
    };

    let dirty_color = if is_dirty {
        palette.accent_fill
    } else {
        Color::TRANSPARENT
    };

    TabItemStyle {
        background,
        border,
        text_color,
        dirty_color,
    }
}

/// Tab strip widget rendering the horizontal document tabs (`UIF-TAB-003`).
pub struct TabStrip<'a, Message> {
    model: &'a TabStripModel,
    palette: Palette,
    on_select_tab: Option<Box<dyn Fn(String) -> Message + 'a>>,
    on_close_tab: Option<Box<dyn Fn(String) -> Message + 'a>>,
}

impl<'a, Message> TabStrip<'a, Message> {
    /// Create a new tab strip widget from `model`.
    #[must_use]
    pub fn new(model: &'a TabStripModel) -> Self {
        Self {
            model,
            palette: Palette::dark(),
            on_select_tab: None,
            on_close_tab: None,
        }
    }

    /// Set the design token color palette.
    #[must_use]
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Callback invoked when a tab is selected.
    #[must_use]
    pub fn on_select_tab(mut self, f: impl Fn(String) -> Message + 'a) -> Self {
        self.on_select_tab = Some(Box::new(f));
        self
    }

    /// Callback invoked when a tab's close button is clicked.
    #[must_use]
    pub fn on_close_tab(mut self, f: impl Fn(String) -> Message + 'a) -> Self {
        self.on_close_tab = Some(Box::new(f));
        self
    }
}

impl<'a, Message: 'a + Clone> From<TabStrip<'a, Message>> for Element<'a, Message> {
    fn from(tab_strip: TabStrip<'a, Message>) -> Self {
        let palette = tab_strip.palette;
        let mut row = iced::widget::Row::new()
            .spacing(space::XXS)
            .align_y(alignment::Vertical::Center);

        for tab in &tab_strip.model.tabs {
            let is_active = tab_strip.model.active_id.as_deref() == Some(&tab.id);
            let style = resolve_tab_style(&palette, is_active, false, tab.is_dirty);

            let mut tab_content = iced::widget::Row::new()
                .spacing(space::XS)
                .align_y(alignment::Vertical::Center);

            if tab.is_dirty {
                tab_content = tab_content.push(
                    iced::widget::container(
                        iced::widget::text("•")
                            .size(Pixels(typography::CAPTION.size))
                            .color(style.dirty_color),
                    )
                    .width(Length::Fixed(DIRTY_DOT_DIAMETER)),
                );
            }

            tab_content = tab_content.push(
                iced::widget::text(tab.title.clone())
                    .size(Pixels(typography::BODY.size))
                    .color(style.text_color),
            );

            if let Some(on_close) = &tab_strip.on_close_tab {
                let close_msg = on_close(tab.id.clone());
                tab_content = tab_content.push(
                    iced::widget::button(
                        iced::widget::text("×").size(Pixels(typography::CAPTION.size)),
                    )
                    .on_press(close_msg)
                    .padding(iced::Padding::new(space::XXS))
                    .style(move |_theme, _status| {
                        iced::widget::button::Style {
                            background: Some(Color::TRANSPARENT.into()),
                            text_color: style.text_color,
                            border: Border::default(),
                            shadow: Shadow::default(),
                            snap: true,
                        }
                    }),
                );
            }

            let tab_id = tab.id.clone();
            let mut tab_btn = iced::widget::button(tab_content)
                .padding(iced::Padding::new(space::XS).horizontal(space::SM))
                .style(move |_theme, status| {
                    let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                    let s = resolve_tab_style(&palette, is_active, is_hovered, false);
                    iced::widget::button::Style {
                        background: Some(s.background.into()),
                        text_color: s.text_color,
                        border: s.border,
                        shadow: Shadow::default(),
                        snap: true,
                    }
                });

            if let Some(on_select) = &tab_strip.on_select_tab {
                tab_btn = tab_btn.on_press(on_select(tab_id));
            }

            let el: Element<'a, Message> = tab_btn.into();

            if let Some(tooltip_text) = &tab.tooltip {
                row = row.push(Tooltip::new(el, tooltip_text.clone()).with_palette(palette));
            } else {
                row = row.push(el);
            }
        }

        iced::widget::container(row)
            .width(Length::Fill)
            .height(Length::Fixed(TAB_STRIP_HEIGHT))
            .padding(iced::Padding::new(0.0).horizontal(space::XS))
            .style(move |_theme| iced::widget::container::Style {
                background: Some(palette.surface_base.into()),
                border: Border::default().color(palette.border_subtle).width(1.0),
                text_color: Some(palette.text_primary),
                shadow: Shadow::default(),
                snap: true,
            })
            .into()
    }
}

#[cfg(test)]
#[path = "tab_strip_tests.rs"]
mod tab_strip_tests;
