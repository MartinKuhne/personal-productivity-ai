//! Tab strip composite with dirty close guards, pointer bindings, MRU focus handoff, and external-deletion sync (`UIF-TAB-001`, `UIF-TAB-002`, `UIF-TAB-003`).
//!
//! The tab strip manages open document tabs, tracking their active state, unsaved-change ("dirty") status,
//! and most-recently-used (MRU) activation history. When a tab is closed, a dirty close guard intercepts
//! tabs with unsaved edits to raise confirmation dialogs, while external file deletions automatically
//! close the corresponding tab and transfer focus to the most-recently-active remaining document.
//! Pointer bindings allow selecting tabs with left-click, closing tabs with middle-click or close button,
//! and opening context menus with right-click. The tabs are wrapped in a horizontal scrollable container
//! respecting [TAB_MIN_WIDTH, TAB_MAX_WIDTH] and 24x24 close button targets.
//!
//! Unit tests live in the sibling `tab_strip_tests.rs` sidecar.

use iced::alignment;
use iced::{Border, Color, Element, Length, Pixels, Shadow};

use crate::surface::menu::{Menu, Row};
use crate::surface::tooltip::Tooltip;
use crate::token::{color::Palette, shape, size, space, typography};

/// Standard height of a tab strip in pixels (`size.control.md` - 32px).
pub const TAB_STRIP_HEIGHT: f32 = size::CONTROL_MD;

/// Minimum width of an individual tab in pixels (96px, §2.2, §4.4.3).
pub const TAB_MIN_WIDTH: f32 = 96.0;

/// Maximum width of an individual tab in pixels (220px, §2.2, §4.4.3).
pub const TAB_MAX_WIDTH: f32 = 220.0;

/// Diameter of the unsaved-state dirty dot in pixels (`size.indicator.dot` - 6px).
pub const DIRTY_DOT_DIAMETER: f32 = size::INDICATOR_DOT;

/// Size of the tab close button target in pixels (`size.hit_target.min` - 24px, §4.4.3).
pub const CLOSE_BUTTON_SIZE: f32 = size::HIT_TARGET_MIN;

/// Clamp a proposed tab width into [`TAB_MIN_WIDTH`, `TAB_MAX_WIDTH`] (`UIF-TAB-002`).
#[must_use]
pub fn clamp_tab_width(width: f32) -> f32 {
    width.clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH)
}

/// Standardized context menu model for document tabs (`UIF-TAB-001`, spec §4.9.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabContextMenuModel {
    /// Identifier / path of the target tab.
    pub tab_id: String,
}

impl TabContextMenuModel {
    /// Create a new tab context menu model for `tab_id`.
    #[must_use]
    pub fn new(tab_id: impl Into<String>) -> Self {
        Self {
            tab_id: tab_id.into(),
        }
    }

    /// Build the standardized [`Menu`] with §4.9.1 functional groups.
    #[must_use]
    pub fn build_menu(&self) -> Menu {
        tab_context_menu(&self.tab_id)
    }
}

/// Helper function creating the standardized context menu for a tab (`UIF-TAB-001`, spec §4.9.1).
#[must_use]
pub fn tab_context_menu(tab_id: &str) -> Menu {
    let menu_id = format!("menu:tab:{tab_id}");
    let rows = vec![
        // Group 1: Tab Lifecycle
        Row::action("close", "Close Tab"),
        Row::action("close_others", "Close Other Tabs"),
        Row::action("close_all", "Close All Tabs"),
        // Divider
        Row::separator(),
        // Group 2: Edit & Automation
        Row::action("edit", "Edit"),
        Row::action("open_in_editor", "Open in System Editor"),
        Row::action("format_markdown", "Format Markdown"),
        // Divider
        Row::separator(),
        // Group 3: Location
        Row::action("copy_path", "Copy Relative Path"),
        Row::action("show_in_explorer", "Show in File Explorer"),
    ];

    Menu::new(menu_id, rows)
}

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

/// Tab strip widget rendering the horizontal document tabs (`UIF-TAB-001`, `UIF-TAB-002`, `UIF-TAB-003`).
pub struct TabStrip<'a, Message> {
    model: &'a TabStripModel,
    palette: Palette,
    on_select_tab: Option<Box<dyn Fn(String) -> Message + 'a>>,
    on_close_tab: Option<Box<dyn Fn(String) -> Message + 'a>>,
    on_context_menu: Option<Box<dyn Fn(String, iced::Point) -> Message + 'a>>,
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
            on_context_menu: None,
        }
    }

    /// Set the design token color palette.
    #[must_use]
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Callback invoked when a tab is selected (`UIF-TAB-001`).
    #[must_use]
    pub fn on_select_tab(mut self, f: impl Fn(String) -> Message + 'a) -> Self {
        self.on_select_tab = Some(Box::new(f));
        self
    }

    /// Callback invoked when a tab's close button or middle-click is triggered (`UIF-TAB-001`).
    #[must_use]
    pub fn on_close_tab(mut self, f: impl Fn(String) -> Message + 'a) -> Self {
        self.on_close_tab = Some(Box::new(f));
        self
    }

    /// Callback invoked when a tab's context menu is requested (`UIF-TAB-001`).
    #[must_use]
    pub fn on_context_menu(mut self, f: impl Fn(String, iced::Point) -> Message + 'a) -> Self {
        self.on_context_menu = Some(Box::new(f));
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
                        iced::widget::container(
                            iced::widget::text("×")
                                .size(Pixels(typography::CAPTION.size))
                                .align_x(alignment::Horizontal::Center)
                                .align_y(alignment::Vertical::Center),
                        )
                        .width(Length::Fixed(CLOSE_BUTTON_SIZE))
                        .height(Length::Fixed(CLOSE_BUTTON_SIZE))
                        .align_x(alignment::Horizontal::Center)
                        .align_y(alignment::Vertical::Center),
                    )
                    .on_press(close_msg)
                    .padding(iced::Padding::ZERO)
                    .width(Length::Fixed(CLOSE_BUTTON_SIZE))
                    .height(Length::Fixed(CLOSE_BUTTON_SIZE))
                    .style(move |_theme, status| {
                        let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                        let bg = if is_hovered {
                            palette.surface_hover
                        } else {
                            Color::TRANSPARENT
                        };
                        let text_color = if is_hovered {
                            palette.status_danger_text
                        } else {
                            style.text_color
                        };
                        iced::widget::button::Style {
                            background: Some(bg.into()),
                            text_color,
                            border: Border::default().rounded(shape::RADIUS_WIDGET),
                            shadow: Shadow::default(),
                            snap: true,
                        }
                    }),
                );
            }

            let tab_id = tab.id.clone();
            // Width clamping: guarantee at least TAB_MIN_WIDTH (96px) and at most TAB_MAX_WIDTH (220px)
            let tab_inner = iced::widget::container(iced::widget::stack![
                iced::widget::Space::new().width(Length::Fixed(TAB_MIN_WIDTH)),
                tab_content,
            ])
            .padding(iced::Padding::new(space::XS).horizontal(space::SM))
            .max_width(Pixels(TAB_MAX_WIDTH))
            .style(move |_theme| iced::widget::container::Style {
                background: Some(style.background.into()),
                border: style.border,
                shadow: Shadow::default(),
                snap: true,
                text_color: Some(style.text_color),
            });

            let mut tab_area = iced::widget::mouse_area(tab_inner);

            if let Some(on_select) = &tab_strip.on_select_tab {
                tab_area = tab_area.on_press(on_select(tab_id.clone()));
            }

            if let Some(on_close) = &tab_strip.on_close_tab {
                tab_area = tab_area.on_middle_press(on_close(tab_id.clone()));
            }

            if let Some(on_ctx) = &tab_strip.on_context_menu {
                tab_area = tab_area.on_right_press(on_ctx(tab_id.clone(), iced::Point::ORIGIN));
            }

            let el: Element<'a, Message> = tab_area.into();

            if let Some(tooltip_text) = &tab.tooltip {
                row = row.push(Tooltip::new(el, tooltip_text.clone()).with_palette(palette));
            } else {
                row = row.push(el);
            }
        }

        let scroll = iced::widget::scrollable(row).direction(
            iced::widget::scrollable::Direction::Horizontal(
                iced::widget::scrollable::Scrollbar::default(),
            ),
        );

        iced::widget::container(scroll)
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
