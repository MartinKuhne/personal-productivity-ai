//! The skeleton placeholder and shimmer composite (`UIF-COMP-014`, `UIF-PRIN-003`).
//!
//! §3B.14 specifies that loading states must reserve the exact dimensions of their
//! target components (24px for tree rows, 28px/32px for table rows, `radius.widget` = 4px
//! or `radius.pill`) to ensure that data arrival produces 0px layout shift (`UIF-PRIN-003`).
//!
//! The skeleton animation interpolates from `surface.raised` to `surface.hover` on an
//! injected `phase` parameter over a 1200ms cycle, eliminating timer leaks and clock
//! dependencies. When injected `reduced_motion` is true, it renders a static `surface.raised`
//! fill with a `1px border.subtle` perimeter stroke.
//!
//! Unit tests live in the sibling `skeleton_tests.rs` sidecar.

use crate::token::color::Palette;
use crate::token::{shape, size, space};
use iced::advanced::Widget;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::tree::Tree;
use iced::mouse;
use iced::{Background, Color, Element, Length, Rectangle, Size};

/// Default corner radius for widget loading placeholders — `radius.widget` (4px, §3B.14).
pub const DEFAULT_RADIUS: f32 = shape::RADIUS_WIDGET;

/// Hairline border width for reduced motion placeholders — `1px border.subtle` (§3B.14).
pub const BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Default height for a tree row placeholder — `space::XL` (24px, §3B.14).
pub const TREE_ROW_HEIGHT: f32 = space::XL;

/// Default height for a standard table row placeholder — `size::CONTROL_MD` (32px, §3B.14).
pub const TABLE_ROW_HEIGHT: f32 = size::CONTROL_MD;

/// Default height for a compact table row placeholder — `size::CONTROL_SM` (28px, §3B.14).
pub const TABLE_ROW_COMPACT_HEIGHT: f32 = size::CONTROL_SM;

/// Nominal duration of the shimmer cycle in milliseconds — 1200ms (§3B.14).
pub const SHIMMER_PERIOD_MS: u32 = 1200;

/// A layout-reserving loading placeholder widget (`UIF-COMP-014`, `UIF-PRIN-003`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Skeleton {
    /// Width of the placeholder in logical pixels (or <= 0.0 for fill).
    pub width: f32,
    /// Height of the placeholder in logical pixels.
    pub height: f32,
    /// Corner radius in logical pixels.
    pub radius: f32,
    /// Normalized phase of the shimmer animation `[0.0, 1.0]`.
    pub phase: f32,
    /// Whether reduced motion is active, disabling animation.
    pub reduced_motion: bool,
}

impl Skeleton {
    /// Construct a skeleton with fixed width and height.
    #[must_use]
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            radius: DEFAULT_RADIUS,
            phase: 0.0,
            reduced_motion: false,
        }
    }

    /// Construct a full-width row placeholder with the given height.
    #[must_use]
    pub fn row(height: f32) -> Self {
        Self::new(0.0, height)
    }

    /// Construct a circular placeholder with the given diameter.
    #[must_use]
    pub fn circle(diameter: f32) -> Self {
        Self {
            width: diameter,
            height: diameter,
            radius: diameter / 2.0,
            phase: 0.0,
            reduced_motion: false,
        }
    }

    /// Set the corner radius.
    #[must_use]
    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Set the animation phase `[0.0, 1.0]`.
    #[must_use]
    pub fn with_phase(mut self, phase: f32) -> Self {
        self.phase = phase;
        self
    }

    /// Set the reduced motion preference.
    #[must_use]
    pub fn with_reduced_motion(mut self, reduced_motion: bool) -> Self {
        self.reduced_motion = reduced_motion;
        self
    }
}

/// The resolved paint style for a skeleton placeholder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkeletonStyle {
    /// Fill color of the placeholder box.
    pub background: Color,
    /// Perimeter border stroke color.
    pub border_color: Color,
    /// Stroke width of the perimeter border.
    pub border_width: f32,
    /// Corner radius of the placeholder box.
    pub radius: f32,
}

/// Linear interpolation between two colors in sRGB space.
#[must_use]
pub fn interpolate_color(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

/// Resolve the paint style of a skeleton against the design tokens.
#[must_use]
pub fn resolve_style(skeleton: &Skeleton, palette: &Palette) -> SkeletonStyle {
    if skeleton.reduced_motion {
        SkeletonStyle {
            background: palette.surface_raised,
            border_color: palette.border_subtle,
            border_width: BORDER_WIDTH,
            radius: skeleton.radius,
        }
    } else {
        let t = (1.0 + (skeleton.phase * std::f32::consts::TAU).sin()) / 2.0;
        let background = interpolate_color(palette.surface_raised, palette.surface_hover, t);
        SkeletonStyle {
            background,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            radius: skeleton.radius,
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Skeleton
where
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        let w = if self.width <= 0.0 {
            Length::Fill
        } else {
            Length::Fixed(self.width)
        };
        Size::new(w, Length::Fixed(self.height))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let width = if self.width <= 0.0 {
            limits.max().width
        } else {
            self.width
        };
        let size = limits.resolve(
            if self.width <= 0.0 {
                Length::Fill
            } else {
                Length::Fixed(self.width)
            },
            Length::Fixed(self.height),
            Size::new(width, self.height),
        );
        layout::Node::new(size)
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let palette = Palette::dark();
        let style = resolve_style(self, &palette);

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: iced::Border {
                    color: style.border_color,
                    width: style.border_width,
                    radius: style.radius.into(),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            },
            Background::Color(style.background),
        );
    }
}

impl<'a, Message, Theme, Renderer> From<Skeleton> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer,
{
    fn from(skeleton: Skeleton) -> Self {
        Element::new(skeleton)
    }
}

/// Helper function to create a skeleton placeholder element.
#[must_use]
pub fn skeleton(width: f32, height: f32) -> Skeleton {
    Skeleton::new(width, height)
}

#[cfg(test)]
#[path = "skeleton_tests.rs"]
mod skeleton_tests;
