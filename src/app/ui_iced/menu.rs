//! Reusable, message-driven menu surfaces with directional placement and nested submenus.
//!
//! Unit tests live in the sibling `menu_tests.rs` sidecar.

#[cfg(test)]
#[path = "menu_tests.rs"]
mod tests;

use iced::widget::{Column, Row, button, container, mouse_area, rule, stack, text};
use iced::{Alignment, Background, Element, Length, Padding, Point, Rectangle, Size};

use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::overlay::{self, MENU_ANCHOR_GAP};

/// Default width of a menu surface in logical UI pixels.
pub const MENU_SURFACE_WIDTH: f32 = 224.0;

/// Default height of a selectable menu row in logical UI pixels.
pub const MENU_ROW_HEIGHT: f32 = 32.0;

/// Default height of a section heading in logical UI pixels.
pub const MENU_HEADING_HEIGHT: f32 = 24.0;

/// Default height of a separator row in logical UI pixels.
pub const MENU_SEPARATOR_HEIGHT: f32 = 9.0;

/// Default inset between menu rows and the menu card edge in logical UI pixels.
pub const MENU_SURFACE_PADDING: f32 = 6.0;

/// Default spacing between menu rows in logical UI pixels.
pub const MENU_ROW_SPACING: f32 = 1.0;

const MENU_SUBMENU_ARROW: &str = ">";

/// A stable identifier for a submenu entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuItemId(&'static str);

impl MenuItemId {
    /// Creates a stable identifier from a string literal.
    #[must_use]
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the identifier's stable string value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// A menu row. User-facing labels are supplied by the caller.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuItem<Message> {
    /// An action that emits `message` when activated.
    Action {
        /// The row label.
        label: String,
        /// The message emitted when the row is activated.
        message: Message,
        /// Whether the row can be activated.
        enabled: bool,
    },
    /// A checkable row whose new value is reported to the caller.
    Toggle {
        /// Stable identifier for this row.
        id: MenuItemId,
        /// The row label.
        label: String,
        /// Whether the row is currently checked.
        checked: bool,
        /// Whether the row can be activated.
        enabled: bool,
    },
    /// A single-choice row whose selection is reported to the caller.
    Radio {
        /// Stable identifier for this row.
        id: MenuItemId,
        /// The row label.
        label: String,
        /// Whether this row is currently selected.
        selected: bool,
        /// Whether the row can be activated.
        enabled: bool,
    },
    /// A row that opens a child menu while keeping its parent visible.
    Submenu {
        /// Stable identifier for this submenu.
        id: MenuItemId,
        /// The row label.
        label: String,
        /// The child menu rows.
        children: Vec<Self>,
        /// Whether the child menu can be opened.
        enabled: bool,
    },
    /// A non-interactive, muted row.
    Disabled {
        /// The row label.
        label: String,
    },
    /// A non-interactive group heading.
    Heading {
        /// The heading text.
        label: String,
    },
    /// A visual divider between menu groups.
    Separator,
}

impl<Message> MenuItem<Message> {
    /// Creates an enabled action row.
    #[must_use]
    pub fn action(label: impl Into<String>, message: Message) -> Self {
        Self::Action {
            label: label.into(),
            message,
            enabled: true,
        }
    }

    /// Creates an action row with an explicit enabled state.
    #[must_use]
    pub fn action_enabled(label: impl Into<String>, message: Message, enabled: bool) -> Self {
        Self::Action {
            label: label.into(),
            message,
            enabled,
        }
    }

    /// Creates a toggle row with caller-controlled checked state.
    #[must_use]
    pub fn toggle(id: MenuItemId, label: impl Into<String>, checked: bool) -> Self {
        Self::Toggle {
            id,
            label: label.into(),
            checked,
            enabled: true,
        }
    }

    /// Creates a radio row with caller-controlled selected state.
    #[must_use]
    pub fn radio(id: MenuItemId, label: impl Into<String>, selected: bool) -> Self {
        Self::Radio {
            id,
            label: label.into(),
            selected,
            enabled: true,
        }
    }

    /// Creates a submenu row.
    #[must_use]
    pub fn submenu(id: MenuItemId, label: impl Into<String>, children: Vec<Self>) -> Self {
        Self::Submenu {
            id,
            label: label.into(),
            children,
            enabled: true,
        }
    }

    /// Creates an inert row, for example an unavailable action or placeholder.
    #[must_use]
    pub fn disabled(label: impl Into<String>) -> Self {
        Self::Disabled {
            label: label.into(),
        }
    }

    /// Creates a non-interactive section heading.
    #[must_use]
    pub fn heading(label: impl Into<String>) -> Self {
        Self::Heading {
            label: label.into(),
        }
    }

    /// Creates a visual separator.
    #[must_use]
    pub const fn separator() -> Self {
        Self::Separator
    }

    /// Returns the row's display label, if it has one.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        match self {
            Self::Action { label, .. }
            | Self::Toggle { label, .. }
            | Self::Radio { label, .. }
            | Self::Submenu { label, .. }
            | Self::Disabled { label }
            | Self::Heading { label } => Some(label),
            Self::Separator => None,
        }
    }

    fn height(&self, style: MenuStyle) -> f32 {
        match self {
            Self::Heading { .. } => style.heading_height,
            Self::Separator => style.separator_height,
            _ => style.row_height,
        }
    }
}

/// A menu interaction emitted by [`menu_overlay`].
#[derive(Clone, Debug, PartialEq)]
pub enum MenuEvent<Message> {
    /// An action row was activated.
    Action(Message),
    /// A toggle was activated, carrying its requested new value.
    Toggle {
        /// Toggle row identifier.
        id: MenuItemId,
        /// Requested checked state after activation.
        checked: bool,
    },
    /// A radio row was selected.
    RadioSelected {
        /// Radio row identifier.
        id: MenuItemId,
    },
    /// A disabled or informational row consumed a click without changing menu state.
    Ignored,
    /// A submenu row was activated.
    OpenSubmenu {
        /// Path to the parent menu, from the root.
        parent_path: Vec<MenuItemId>,
        /// Submenu to open under that parent.
        id: MenuItemId,
    },
    /// The caller should close the deepest currently open submenu.
    CloseSubmenu,
    /// The user clicked outside the open menu tree.
    Dismissed,
}

/// Reducer-owned path of currently open submenu identifiers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuState {
    open_path: Vec<MenuItemId>,
}

impl MenuState {
    /// The active submenu path, from the root menu to the deepest open child.
    #[must_use]
    pub fn open_path(&self) -> &[MenuItemId] {
        &self.open_path
    }

    /// Opens `child` beneath `parent_path`, replacing any deeper or sibling branch.
    pub fn open_child(&mut self, parent_path: &[MenuItemId], child: MenuItemId) {
        self.open_path.clear();
        self.open_path.extend_from_slice(parent_path);
        self.open_path.push(child);
    }

    /// Closes the deepest submenu, returning whether one was open.
    pub fn close_deepest(&mut self) -> bool {
        self.open_path.pop().is_some()
    }

    /// Closes every submenu.
    pub fn close_all(&mut self) {
        self.open_path.clear();
    }

    /// Whether `id` is the deepest open submenu.
    #[must_use]
    pub fn is_deepest(&self, id: MenuItemId) -> bool {
        self.open_path.last() == Some(&id)
    }
}

/// Horizontal placement preference for a point anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HorizontalPreference {
    /// Place the menu to the left of the anchor.
    Left,
    /// Place the menu to the right of the anchor.
    Right,
    /// Prefer right, flip left when needed, and use the roomier side if neither fits.
    Auto,
}

/// Vertical placement preference for a point anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerticalPreference {
    /// Place the menu above the anchor.
    Above,
    /// Place the menu below the anchor.
    Below,
    /// Prefer below, flip above when needed, and use the roomier side if neither fits.
    Auto,
}

/// Cardinal side for a rectangle-anchored menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuSide {
    /// Place the menu to the left of the anchor.
    Left,
    /// Place the menu to the right of the anchor.
    Right,
    /// Place the menu above the anchor.
    Above,
    /// Place the menu below the anchor.
    Below,
}

/// Alignment on the axis perpendicular to a rectangle anchor's side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAlignment {
    /// Align the menu's start edge with the anchor's start edge.
    Start,
    /// Center the menu against the anchor.
    Center,
    /// Align the menu's end edge with the anchor's end edge.
    End,
}

/// Root-space placement input for a menu surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuPosition {
    /// A point in root viewport coordinates, usually the pointer location.
    Point {
        /// Root-space point.
        point: Point,
        /// Preferred horizontal direction.
        horizontal: HorizontalPreference,
        /// Preferred vertical direction.
        vertical: VerticalPreference,
    },
    /// A root-space target rectangle such as a button or parent menu row.
    Anchor {
        /// Root-space bounds of the anchor.
        bounds: Rectangle,
        /// Preferred side of the anchor.
        side: MenuSide,
        /// Alignment on the perpendicular axis.
        alignment: MenuAlignment,
        /// Whether to flip to the opposite side when the preferred side does not fit.
        auto_flip: bool,
    },
}

/// Sizing and spacing shared by all levels of a menu tree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuStyle {
    /// Fixed surface width.
    pub width: f32,
    /// Selectable row height.
    pub row_height: f32,
    /// Heading row height.
    pub heading_height: f32,
    /// Separator row height.
    pub separator_height: f32,
    /// Card padding.
    pub padding: f32,
    /// Gap between rows.
    pub row_spacing: f32,
    /// Gap from an anchor to the menu surface.
    pub anchor_gap: f32,
}

impl Default for MenuStyle {
    fn default() -> Self {
        Self {
            width: MENU_SURFACE_WIDTH,
            row_height: MENU_ROW_HEIGHT,
            heading_height: MENU_HEADING_HEIGHT,
            separator_height: MENU_SEPARATOR_HEIGHT,
            padding: MENU_SURFACE_PADDING,
            row_spacing: MENU_ROW_SPACING,
            anchor_gap: MENU_ANCHOR_GAP,
        }
    }
}

impl MenuStyle {
    fn normalized(self) -> Self {
        Self {
            width: finite_nonnegative(self.width),
            row_height: finite_nonnegative(self.row_height),
            heading_height: finite_nonnegative(self.heading_height),
            separator_height: finite_nonnegative(self.separator_height),
            padding: finite_nonnegative(self.padding),
            row_spacing: finite_nonnegative(self.row_spacing),
            anchor_gap: finite_nonnegative(self.anchor_gap),
        }
    }
}

/// The rendered size of a menu containing `items` under `style`.
#[must_use]
pub fn menu_size<Message>(items: &[MenuItem<Message>], style: MenuStyle) -> Size {
    let style = style.normalized();
    let row_height = items.iter().map(|item| item.height(style)).sum::<f32>();
    let row_spacing = style.row_spacing * items.len().saturating_sub(1) as f32;
    Size::new(style.width, row_height + row_spacing + style.padding * 2.0)
}

/// Resolves a menu's root-space top-left position, including directional flips and viewport clamping.
#[must_use]
pub fn resolve_menu_position(
    position: MenuPosition,
    size: Size,
    viewport: Size,
    gap: f32,
) -> Point {
    let viewport = sanitized_size(viewport);
    let size = sanitized_size(size);
    let gap = finite_nonnegative(gap);

    let origin = match position {
        MenuPosition::Point {
            point,
            horizontal,
            vertical,
        } => {
            let point = Point::new(finite_nonnegative(point.x), finite_nonnegative(point.y));
            Point::new(
                point_horizontal(point.x, size.width, viewport.width, gap, horizontal),
                point_vertical(point.y, size.height, viewport.height, gap, vertical),
            )
        }
        MenuPosition::Anchor {
            bounds,
            side,
            alignment,
            auto_flip,
        } => anchor_origin(
            sanitized_rect(bounds),
            size,
            viewport,
            gap,
            side,
            alignment,
            auto_flip,
        ),
    };

    clamp_origin(origin, size, viewport)
}

/// Wraps `base` with a click-away layer and all open surfaces in a menu tree.
///
/// The open submenu path is projected in order, so each parent stays mounted
/// beneath its child. The reducer owns [`MenuState`] and maps [`MenuEvent`]s to
/// application messages. Menu anchors and `viewport` must use the same root-space
/// logical coordinate system. Mount this element only through the application's
/// root `OverlayLayer` host, not directly inside a panel. Toggle events do not
/// mutate menu state or dismiss the overlay; the reducer remains authoritative.
/// Disabled and informational rows emit [`MenuEvent::Ignored`] only to consume
/// their hit so it cannot fall through to the click-away layer.
#[must_use]
pub fn menu_overlay<'a, Message>(
    base: Element<'a, MenuEvent<Message>>,
    items: &[MenuItem<Message>],
    state: &MenuState,
    position: MenuPosition,
    viewport: Size,
    style: MenuStyle,
) -> Element<'a, MenuEvent<Message>>
where
    Message: Clone + 'a,
{
    if items.is_empty() {
        return base;
    }

    let style = style.normalized();

    let root_size = menu_size(items, style);
    let root_origin = resolve_menu_position(position, root_size, viewport, style.anchor_gap);
    let mut surfaces = vec![(items, root_origin, root_size, Vec::new())];
    let mut parent_items = items;
    let mut parent_origin = root_origin;
    let mut parent_path = Vec::new();

    for child_id in state.open_path() {
        let Some((row_index, children)) = find_submenu(parent_items, *child_id) else {
            break;
        };
        let parent_row = row_bounds(parent_items, row_index, parent_origin, style);
        let child_size = menu_size(children, style);
        let child_origin = resolve_menu_position(
            MenuPosition::Anchor {
                bounds: parent_row,
                side: MenuSide::Right,
                alignment: MenuAlignment::Start,
                auto_flip: true,
            },
            child_size,
            viewport,
            style.anchor_gap,
        );
        parent_path.push(*child_id);
        surfaces.push((
            children.as_slice(),
            child_origin,
            child_size,
            parent_path.clone(),
        ));
        parent_items = children;
        parent_origin = child_origin;
    }

    let mut layers: Vec<Element<'a, MenuEvent<Message>>> =
        vec![base, overlay::input_blocker(MenuEvent::Dismissed).into()];
    for (level_items, origin, size, path) in surfaces {
        layers.push(positioned_surface(
            render_menu(level_items, &path, state, style),
            origin,
            size,
        ));
    }

    stack(layers)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn render_menu<'a, Message>(
    items: &[MenuItem<Message>],
    parent_path: &[MenuItemId],
    state: &MenuState,
    style: MenuStyle,
) -> Element<'a, MenuEvent<Message>>
where
    Message: Clone + 'a,
{
    let palette = Palette::fastmd_dark();
    let rows = items.iter().fold(
        Column::<MenuEvent<Message>>::new().spacing(style.row_spacing),
        |rows, item| rows.push(render_row(item, parent_path, state, style, palette)),
    );

    container(rows)
        .width(Length::Fixed(style.width))
        .padding(style.padding)
        .style(overlay::card_style)
        .into()
}

fn render_row<'a, Message>(
    item: &MenuItem<Message>,
    parent_path: &[MenuItemId],
    state: &MenuState,
    style: MenuStyle,
    palette: Palette,
) -> Element<'a, MenuEvent<Message>>
where
    Message: Clone + 'a,
{
    match item {
        MenuItem::Action {
            label,
            message,
            enabled,
        } => {
            if *enabled {
                action_row(
                    label.clone(),
                    None,
                    MenuEvent::Action(message.clone()),
                    style,
                    palette,
                )
            } else {
                passive_row(label.clone(), None, style, palette, true)
            }
        }
        MenuItem::Toggle {
            id,
            label,
            checked,
            enabled,
        } => {
            let check_slot = strings::format_menu_row_label(None, "", *checked);
            if *enabled {
                action_row(
                    label.clone(),
                    Some(check_slot),
                    MenuEvent::Toggle {
                        id: *id,
                        checked: !checked,
                    },
                    style,
                    palette,
                )
            } else {
                passive_row(label.clone(), Some(check_slot), style, palette, true)
            }
        }
        MenuItem::Radio {
            id,
            label,
            selected,
            enabled,
        } => {
            let check_slot = strings::format_menu_row_label(None, "", *selected);
            if *enabled && !selected {
                action_row(
                    label.clone(),
                    Some(check_slot),
                    MenuEvent::RadioSelected { id: *id },
                    style,
                    palette,
                )
            } else {
                passive_row(label.clone(), Some(check_slot), style, palette, !*enabled)
            }
        }
        MenuItem::Submenu {
            id, label, enabled, ..
        } => {
            let mut path = parent_path.to_vec();
            path.push(*id);
            let active = state.open_path().starts_with(&path);
            let label = format!("{label} {MENU_SUBMENU_ARROW}");
            let mut row = button(text(label.clone()).size(14.0))
                .width(Length::Fill)
                .height(Length::Fixed(style.row_height))
                .padding(Padding::ZERO);
            if !enabled {
                passive_row(label, None, style, palette, true)
            } else {
                row = row.on_press(MenuEvent::OpenSubmenu {
                    parent_path: parent_path.to_vec(),
                    id: *id,
                });
                if active {
                    active_submenu_row(row, style, palette)
                } else {
                    row.into()
                }
            }
        }
        MenuItem::Disabled { label } => passive_row(label.clone(), None, style, palette, true),
        MenuItem::Heading { label } => inert_row(
            container(
                Row::new()
                    .push(text(label.clone()).size(13.0).color(palette.text_muted))
                    .align_y(Alignment::Center)
                    .width(Length::Fill)
                    .height(Length::Fixed(style.heading_height)),
            )
            .width(Length::Fill),
        ),
        MenuItem::Separator => inert_row(
            container(rule::horizontal(1))
                .width(Length::Fill)
                .height(Length::Fixed(style.separator_height))
                .center_y(Length::Fill),
        ),
    }
}

fn action_row<'a, Message>(
    label: String,
    leading: Option<String>,
    message: MenuEvent<Message>,
    style: MenuStyle,
    palette: Palette,
) -> Element<'a, MenuEvent<Message>>
where
    Message: Clone + 'a,
{
    button(row_content(
        label,
        leading,
        style.row_height,
        palette,
        false,
    ))
    .width(Length::Fill)
    .height(Length::Fixed(style.row_height))
    .padding(Padding::ZERO)
    .on_press(message)
    .into()
}

fn row_content<'a, Message: 'a>(
    label: String,
    leading: Option<String>,
    height: f32,
    palette: Palette,
    muted: bool,
) -> Element<'a, MenuEvent<Message>> {
    let color = if muted {
        palette.text_muted
    } else {
        palette.text
    };
    let mut row = Row::new().align_y(Alignment::Center).spacing(8.0);
    if let Some(leading) = leading {
        row = row.push(
            container(text(leading).size(14.0).color(color))
                .width(Length::Fixed(20.0))
                .center_x(Length::Fixed(20.0)),
        );
    }
    row = row.push(text(label).size(14.0).color(color));
    container(row.width(Length::Fill).height(Length::Fixed(height)))
        .width(Length::Fill)
        .into()
}

fn passive_row<'a, Message: Clone + 'a>(
    label: String,
    leading: Option<String>,
    style: MenuStyle,
    palette: Palette,
    muted: bool,
) -> Element<'a, MenuEvent<Message>> {
    inert_row(
        button(row_content(
            label,
            leading,
            style.row_height,
            palette,
            muted,
        ))
        .width(Length::Fill)
        .height(Length::Fixed(style.row_height))
        .padding(Padding::ZERO)
        .on_press_maybe(None::<MenuEvent<Message>>),
    )
}

fn inert_row<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, MenuEvent<Message>>>,
) -> Element<'a, MenuEvent<Message>> {
    mouse_area(content).on_press(MenuEvent::Ignored).into()
}

fn active_submenu_row<'a, Message>(
    row: iced::widget::Button<'a, MenuEvent<Message>>,
    style: MenuStyle,
    palette: Palette,
) -> Element<'a, MenuEvent<Message>>
where
    Message: Clone + 'a,
{
    let background = container(row)
        .width(Length::Fill)
        .height(Length::Fixed(style.row_height))
        .style(move |_theme| container::Style {
            background: Some(Background::Color(palette.accent)),
            ..container::Style::default()
        });
    background.into()
}

fn positioned_surface<'a, Message: 'a>(
    surface: Element<'a, MenuEvent<Message>>,
    origin: Point,
    _size: Size,
) -> Element<'a, MenuEvent<Message>> {
    container(surface)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: origin.y,
            right: 0.0,
            bottom: 0.0,
            left: origin.x,
        })
        .into()
}

fn find_submenu<Message>(
    items: &[MenuItem<Message>],
    id: MenuItemId,
) -> Option<(usize, &Vec<MenuItem<Message>>)> {
    items
        .iter()
        .enumerate()
        .find_map(|(index, item)| match item {
            MenuItem::Submenu {
                id: item_id,
                children,
                ..
            } if *item_id == id => Some((index, children)),
            _ => None,
        })
}

fn row_bounds(
    items: &[MenuItem<impl Sized>],
    row_index: usize,
    menu_origin: Point,
    style: MenuStyle,
) -> Rectangle {
    let rows_before = items
        .iter()
        .take(row_index)
        .map(|item| item.height(style) + style.row_spacing)
        .sum::<f32>();
    Rectangle {
        x: menu_origin.x,
        y: menu_origin.y + style.padding + rows_before,
        width: style.width,
        height: items[row_index].height(style),
    }
}

fn point_horizontal(
    x: f32,
    width: f32,
    viewport_width: f32,
    gap: f32,
    preference: HorizontalPreference,
) -> f32 {
    let right = x + gap;
    let left = x - width - gap;
    let right_space = (viewport_width - x - gap).max(0.0);
    let left_space = (x - gap).max(0.0);
    match preference {
        HorizontalPreference::Left => left,
        HorizontalPreference::Right => right,
        HorizontalPreference::Auto if width <= right_space => right,
        HorizontalPreference::Auto if width <= left_space => left,
        HorizontalPreference::Auto if right_space >= left_space => right,
        HorizontalPreference::Auto => left,
    }
}

fn point_vertical(
    y: f32,
    height: f32,
    viewport_height: f32,
    gap: f32,
    preference: VerticalPreference,
) -> f32 {
    let below = y + gap;
    let above = y - height - gap;
    let below_space = (viewport_height - y - gap).max(0.0);
    let above_space = (y - gap).max(0.0);
    match preference {
        VerticalPreference::Above => above,
        VerticalPreference::Below => below,
        VerticalPreference::Auto if height <= below_space => below,
        VerticalPreference::Auto if height <= above_space => above,
        VerticalPreference::Auto if below_space >= above_space => below,
        VerticalPreference::Auto => above,
    }
}

fn anchor_origin(
    anchor: Rectangle,
    size: Size,
    viewport: Size,
    gap: f32,
    side: MenuSide,
    alignment: MenuAlignment,
    auto_flip: bool,
) -> Point {
    let preferred = anchor_candidate(anchor, size, gap, side, alignment);
    let opposite = anchor_candidate(anchor, size, gap, opposite_side(side), alignment);
    let chosen = if !auto_flip || fits(preferred, size, viewport) {
        preferred
    } else if fits(opposite, size, viewport) {
        opposite
    } else if available_space(anchor, viewport, gap, side)
        >= available_space(anchor, viewport, gap, opposite_side(side))
    {
        preferred
    } else {
        opposite
    };
    clamp_origin(chosen, size, viewport)
}

fn anchor_candidate(
    anchor: Rectangle,
    size: Size,
    gap: f32,
    side: MenuSide,
    alignment: MenuAlignment,
) -> Point {
    match side {
        MenuSide::Left => Point::new(
            anchor.x - size.width - gap,
            align_axis(anchor.y, anchor.height, size.height, alignment),
        ),
        MenuSide::Right => Point::new(
            anchor.x + anchor.width + gap,
            align_axis(anchor.y, anchor.height, size.height, alignment),
        ),
        MenuSide::Above => Point::new(
            align_axis(anchor.x, anchor.width, size.width, alignment),
            anchor.y - size.height - gap,
        ),
        MenuSide::Below => Point::new(
            align_axis(anchor.x, anchor.width, size.width, alignment),
            anchor.y + anchor.height + gap,
        ),
    }
}

fn align_axis(
    anchor_start: f32,
    anchor_length: f32,
    menu_length: f32,
    alignment: MenuAlignment,
) -> f32 {
    match alignment {
        MenuAlignment::Start => anchor_start,
        MenuAlignment::Center => anchor_start + (anchor_length - menu_length) / 2.0,
        MenuAlignment::End => anchor_start + anchor_length - menu_length,
    }
}

fn available_space(anchor: Rectangle, viewport: Size, gap: f32, side: MenuSide) -> f32 {
    match side {
        MenuSide::Left => anchor.x - gap,
        MenuSide::Right => viewport.width - anchor.x - anchor.width - gap,
        MenuSide::Above => anchor.y - gap,
        MenuSide::Below => viewport.height - anchor.y - anchor.height - gap,
    }
    .max(0.0)
}

fn opposite_side(side: MenuSide) -> MenuSide {
    match side {
        MenuSide::Left => MenuSide::Right,
        MenuSide::Right => MenuSide::Left,
        MenuSide::Above => MenuSide::Below,
        MenuSide::Below => MenuSide::Above,
    }
}

fn fits(origin: Point, size: Size, viewport: Size) -> bool {
    origin.x >= 0.0
        && origin.y >= 0.0
        && origin.x + size.width <= viewport.width
        && origin.y + size.height <= viewport.height
}

fn clamp_origin(origin: Point, size: Size, viewport: Size) -> Point {
    let max_x = (viewport.width - size.width).max(0.0);
    let max_y = (viewport.height - size.height).max(0.0);
    Point::new(
        finite_nonnegative(origin.x).min(max_x),
        finite_nonnegative(origin.y).min(max_y),
    )
}

fn sanitized_size(size: Size) -> Size {
    Size::new(
        finite_nonnegative(size.width),
        finite_nonnegative(size.height),
    )
}

fn sanitized_rect(rect: Rectangle) -> Rectangle {
    Rectangle {
        x: finite_or_zero(rect.x),
        y: finite_or_zero(rect.y),
        width: finite_nonnegative(rect.width),
        height: finite_nonnegative(rect.height),
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}
