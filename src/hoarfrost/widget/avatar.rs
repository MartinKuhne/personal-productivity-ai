//! The Avatar and PersonPicture widget (§3.13, `UIF-ATOM-013`).
//!
//! An avatar represents a user or entity with a deterministic three-stage
//! fallback cascade: an image when available, one-or-two uppercase initials
//! on a compliant background, or a generic person glyph when no name exists.
//! It also supports an optional bottom-right presence dot indicating availability
//! or status.
//!
//! Unit tests live in the sibling `avatar_tests.rs` sidecar.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text as text_core;
use iced::advanced::widget::{Tree, Widget};
use iced::alignment;
use iced::{Background, Border, Color, Element, Length, Pixels, Rectangle, Shadow, Size};

use crate::token::color::Palette;
use crate::token::shape;
use crate::token::size;
use crate::token::typography::{self, Weight};
use crate::widget::badge_model::BadgeTone;

/// Generic fallback person glyph optical character (`UIF-ATOM-013`, §3.13).
pub const PERSON_GLYPH: &str = "👤";

/// Visual sizing rungs for [`Avatar`] (`UIF-ATOM-013`, §3.13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum AvatarSize {
    /// Small: `24 × 24px` ([`size::AVATAR_SM`]).
    #[default]
    Sm,
    /// Medium / standard: `32 × 32px` ([`size::AVATAR_MD`]).
    Md,
    /// Large: `36 × 36px` ([`size::AVATAR_LG`]).
    Lg,
}

impl AvatarSize {
    /// Diameter in logical pixels (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub const fn diameter(self) -> f32 {
        match self {
            Self::Sm => size::AVATAR_SM,
            Self::Md => size::AVATAR_MD,
            Self::Lg => size::AVATAR_LG,
        }
    }
}

/// Geometric shape for [`Avatar`] (`UIF-ATOM-013`, §3.13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum AvatarShape {
    /// Fully rounded circle (`radius.pill`).
    #[default]
    Circle,
    /// Rounded square container (`radius.card`).
    RoundedSquare,
}

impl AvatarShape {
    /// Corner radius for this shape (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub const fn radius(self) -> f32 {
        match self {
            Self::Circle => shape::RADIUS_PILL,
            Self::RoundedSquare => shape::RADIUS_CARD,
        }
    }
}

/// Visual stage of the deterministic avatar fallback cascade (`UIF-ATOM-013`, §3.13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FallbackStage {
    /// 1st stage: image URI is present.
    Image,
    /// 2nd stage: initials (1–2 uppercase letters).
    Initials,
    /// 3rd stage: generic person glyph/icon.
    Glyph,
}

/// Declarative data model driving an [`Avatar`] widget (`UIF-ATOM-013`, §3.13).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvatarModel {
    /// Display name of the person or entity.
    pub display_name: String,
    /// Extracted or explicitly assigned initials (1–2 uppercase letters).
    pub initials: String,
    /// Optional image resource URI or path.
    pub image_uri: Option<String>,
    /// Optional presence indicator tone.
    pub presence: Option<BadgeTone>,
    /// Sizing rung.
    pub size: AvatarSize,
    /// Geometric shape.
    pub shape: AvatarShape,
}

impl AvatarModel {
    /// Construct a new model from `display_name`, extracting initials automatically.
    #[must_use]
    pub fn new(display_name: impl Into<String>) -> Self {
        let name = display_name.into();
        let initials = extract_initials(&name);
        Self {
            display_name: name,
            initials,
            image_uri: None,
            presence: None,
            size: AvatarSize::default(),
            shape: AvatarShape::default(),
        }
    }

    /// Override the initials explicitly (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub fn with_initials(mut self, initials: impl Into<String>) -> Self {
        self.initials = initials.into();
        self
    }

    /// Set an image URI (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub fn with_image_uri(mut self, uri: impl Into<String>) -> Self {
        self.image_uri = Some(uri.into());
        self
    }

    /// Set an optional presence status (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub fn with_presence(mut self, presence: impl Into<Option<BadgeTone>>) -> Self {
        self.presence = presence.into();
        self
    }

    /// Set the avatar sizing rung (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub fn with_size(mut self, size: AvatarSize) -> Self {
        self.size = size;
        self
    }

    /// Set the avatar geometric shape (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub fn with_shape(mut self, shape: AvatarShape) -> Self {
        self.shape = shape;
        self
    }

    /// Determine the active fallback stage based on present data (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub fn fallback_stage(&self) -> FallbackStage {
        if let Some(uri) = &self.image_uri
            && !uri.trim().is_empty()
        {
            return FallbackStage::Image;
        }
        if !self.initials.trim().is_empty() {
            return FallbackStage::Initials;
        }
        FallbackStage::Glyph
    }
}

/// Extract 1–2 uppercase initials from a display name (`UIF-ATOM-013`, §3.13).
///
/// For multi-word names, takes the first alphanumeric character of the first word
/// and the first alphanumeric character of the last word. For single-word names,
/// takes the first alphanumeric character.
#[must_use]
pub fn extract_initials(display_name: &str) -> String {
    let words: Vec<&str> = display_name.split_whitespace().collect();
    match words.len() {
        0 => String::new(),
        1 => words[0]
            .chars()
            .find(|c| c.is_alphanumeric())
            .map(|c| c.to_uppercase().collect::<String>())
            .unwrap_or_default(),
        _ => {
            let first = words
                .first()
                .and_then(|w| w.chars().find(|c| c.is_alphanumeric()));
            let last = words
                .last()
                .and_then(|w| w.chars().find(|c| c.is_alphanumeric()));
            match (first, last) {
                (Some(f), Some(l)) => {
                    let mut s = String::with_capacity(4);
                    s.extend(f.to_uppercase());
                    s.extend(l.to_uppercase());
                    s
                }
                (Some(f), None) => f.to_uppercase().collect(),
                (None, Some(l)) => l.to_uppercase().collect(),
                (None, None) => String::new(),
            }
        }
    }
}

/// Resolve presence dot color from [`BadgeTone`] (`UIF-ATOM-013`, §3.13).
#[must_use]
pub fn resolve_presence_color(tone: BadgeTone, palette: &Palette) -> Color {
    match tone {
        BadgeTone::Success => palette.status_success,
        BadgeTone::Warning => palette.status_warning,
        BadgeTone::Danger => palette.status_danger_text,
        BadgeTone::Neutral => palette.text_muted,
        BadgeTone::Accent => palette.accent_border,
    }
}

/// The Avatar / PersonPicture widget (`UIF-ATOM-013`, §3.13).
#[derive(Clone, Debug)]
pub struct Avatar {
    model: AvatarModel,
    palette: Palette,
}

impl Avatar {
    /// Construct a new avatar widget from the given model (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub fn new(model: AvatarModel) -> Self {
        Self {
            model,
            palette: Palette::dark(),
        }
    }

    /// Supply a custom palette for semantic colors (`UIF-ATOM-013`).
    #[must_use]
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Reference to the underlying data model.
    #[must_use]
    pub fn model(&self) -> &AvatarModel {
        &self.model
    }

    /// Layout size of the avatar in logical pixels (`UIF-ATOM-013`, §3.13).
    #[must_use]
    pub fn size(&self) -> Size<Length> {
        let d = self.model.size.diameter();
        Size::new(Length::Fixed(d), Length::Fixed(d))
    }
}

/// Construct an avatar widget from an [`AvatarModel`] (`UIF-ATOM-013`, §3.13).
#[must_use]
pub fn avatar(model: AvatarModel) -> Avatar {
    Avatar::new(model)
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Avatar
where
    Renderer: renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn size(&self) -> Size<Length> {
        Avatar::size(self)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let d = self.model.size.diameter();
        let size = limits.resolve(Length::Fixed(d), Length::Fixed(d), Size::new(d, d));
        layout::Node::new(size)
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: iced::mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let d = self.model.size.diameter();
        let shape_radius = self.model.shape.radius();
        let stage = self.model.fallback_stage();

        // 1. Container base & fallback cascade rendering
        match stage {
            FallbackStage::Image => {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        border: Border::default()
                            .color(self.palette.border_strong)
                            .width(shape::BORDER_HAIRLINE)
                            .rounded(shape_radius),
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    Background::Color(self.palette.surface_active),
                );
            }
            FallbackStage::Initials => {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        border: Border::default()
                            .color(self.palette.border_strong)
                            .width(shape::BORDER_HAIRLINE)
                            .rounded(shape_radius),
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    Background::Color(self.palette.accent_fill),
                );

                let font_size = match self.model.size {
                    AvatarSize::Sm => typography::CAPTION.size,
                    AvatarSize::Md | AvatarSize::Lg => typography::UI_COMPACT.size,
                };
                let font = typography::CAPTION.sans(Weight::SemiBold);

                renderer.fill_text(
                    text_core::Text {
                        content: self.model.initials.clone(),
                        bounds: bounds.size(),
                        size: Pixels(font_size),
                        line_height: text_core::LineHeight::Absolute(Pixels(bounds.height)),
                        font,
                        align_x: text_core::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    bounds.position(),
                    self.palette.text_strong,
                    *viewport,
                );
            }
            FallbackStage::Glyph => {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        border: Border::default()
                            .color(self.palette.border_strong)
                            .width(shape::BORDER_HAIRLINE)
                            .rounded(shape_radius),
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    Background::Color(self.palette.surface_active),
                );

                renderer.fill_text(
                    text_core::Text {
                        content: PERSON_GLYPH.to_owned(),
                        bounds: bounds.size(),
                        size: Pixels(size::ICON_MD),
                        line_height: text_core::LineHeight::Absolute(Pixels(bounds.height)),
                        font: renderer.default_font(),
                        align_x: text_core::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    bounds.position(),
                    self.palette.text_primary,
                    *viewport,
                );
            }
        }

        // 2. Optional presence dot with 1px surface.base cutout ring
        if let Some(presence_tone) = self.model.presence {
            let cutout_size = size::INDICATOR_DOT + 2.0 * shape::BORDER_HAIRLINE;
            let cutout_bounds = Rectangle {
                x: bounds.x + d - cutout_size,
                y: bounds.y + d - cutout_size,
                width: cutout_size,
                height: cutout_size,
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: cutout_bounds,
                    border: Border::default().rounded(shape::RADIUS_PILL),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(self.palette.surface_base),
            );

            let dot_bounds = Rectangle {
                x: bounds.x + d - cutout_size + shape::BORDER_HAIRLINE,
                y: bounds.y + d - cutout_size + shape::BORDER_HAIRLINE,
                width: size::INDICATOR_DOT,
                height: size::INDICATOR_DOT,
            };

            let tone_color = resolve_presence_color(presence_tone, &self.palette);
            renderer.fill_quad(
                renderer::Quad {
                    bounds: dot_bounds,
                    border: Border::default().rounded(shape::RADIUS_PILL),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Background::Color(tone_color),
            );
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Avatar> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer<Font = iced::Font>,
{
    fn from(avatar: Avatar) -> Self {
        Element::new(avatar)
    }
}

#[cfg(test)]
#[path = "avatar_tests.rs"]
mod avatar_tests;
