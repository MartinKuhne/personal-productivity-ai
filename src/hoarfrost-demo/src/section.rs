//! The titled-card *section* primitive every pane is built from.
//!
//! A section is one heading plus the widget(s) under test, painted on a
//! hoarfrost card. It is the demo's only structural unit: the left pane is
//! a column of sections, the right pane is a column of sections, and no
//! pane ever lays out a bare widget. Keeping the frame in one function is
//! what stops a hundred demo call sites from each inventing their own
//! heading size, card padding and border.
//!
//! The frame itself is not invented here either — the card's fill, border,
//! radius and padding come from [`card::resolve_style`]
//! (`UIF-COMP-005`) and the heading's type rung from
//! [`typography::TITLE_SM`]
//! (`UIF-TOK-004`).
//!
//! Unit tests live in the sibling `section_tests.rs` sidecar.

use iced::widget::{Column, Row, container};
use iced::{Alignment, Background, Border, Element, Length, Pixels, Shadow};

use hoarfrost::composite::card::{self, CardElevation};
use hoarfrost::token::color::Palette;
use hoarfrost::token::{space, typography};

use crate::app::Message;

/// Vertical gap between a section heading and its first body row.
pub const HEADING_GAP: f32 = space::SM;

/// Horizontal gap between the heading text and the widget-count badge.
pub const HEADING_BADGE_GAP: f32 = space::XS;

/// Vertical gap between two sections inside one pane.
pub const SECTION_GAP: f32 = space::MD;

/// Prefix every derived section layout id carries.
pub const SECTION_ID_PREFIX: &str = "demo.section.";

/// Suffix the **framework's own** derived label captions carry.
///
/// `hoarfrost` publishes the text a widget draws for itself as
/// `<widget-id>.<role>` (`hoarfrost/src/identity.rs`), and `label` is one
/// of the five roles, so every caption this crate reads but did not write
/// ends here: `demo.atoms.checkbox.0.label`,
/// `demo.composites.setting.1.label`, and the indexed
/// `demo.atoms.radio.label-0`.
///
/// This crate authors nothing under this suffix — [`DEMO_LABEL_ID_SUFFIX`]
/// is the one it writes — so the binary never reads it. Only the test-only
/// alignment audit does, to tell a caption the framework published from
/// every other record it is handed.
#[cfg_attr(not(test), allow(dead_code))]
pub const LABEL_ID_SUFFIX: &str = ".label";

/// Suffix every **demo-authored** label id carries.
///
/// A bare `iced::widget::Text` cannot be named. `iced_core::widget::Text`
/// has no `id` builder at all, and its `Widget::operate` reports
/// `operation.text(None, ..)` — the widget's identity is thrown away
/// before the operation tree ever sees it. Wrapping the rung in a
/// padding-free `container` is the only way in iced 0.14 to put a
/// stable `Id` on a label, and it costs nothing geometrically: the
/// container's recorded bounds are byte-identical to the text's own
/// (`labeled_keeps_the_rungs_own_bounds` pins this).
///
/// The suffix — rather than a separate registry — is what tells the audit
/// "this container *is* a label", so [`labeled`] is the single place the
/// convention is applied and a label cannot be declared by accident.
///
/// # Why it is not `.label`
///
/// `.label` is inside the framework's **reserved role namespace**:
/// `hoarfrost::identity::ROLES` is `title`, `subtitle`, `header`, `hint`
/// and `label`, and [`is_role`] also accepts the indexed form
/// `<role>-<digits>`. A host that parses ids reads anything whose last
/// dotted segment is one of those as a **derived sub-part** of the widget
/// named before it — `demo.atoms.buttons.danger.label` reads as "the
/// caption of a widget called `demo.atoms.buttons.danger`", which does
/// not exist. `identity.rs` only *documents* that hazard ("The convention
/// owns that risk"); the test-only `id_namespace` module turns it into an
/// enforced invariant, and this suffix is what keeps the demo on the right
/// side of it.
///
/// `.demo-label` cannot collide: [`is_role`] accepts only a bare role or a
/// role followed by `-` and ASCII digits, and `demo-label` is neither —
/// stripping the `label` prefix from it fails because the string does not
/// *start* with `label`. The `demo` in front also says who wrote it, which
/// is the part a consumer actually needs.
///
/// [`is_role`]: hoarfrost::identity::is_role
pub const DEMO_LABEL_ID_SUFFIX: &str = ".demo-label";

/// Suffix every **explicitly declared label row** container id carries.
///
/// Where [`LABEL_ID_SUFFIX`] names the label itself, this names the box
/// that holds a label and exactly one control. It exists for rows whose
/// label text the demo does not render — `settings_row::horizontal`
/// builds its own title/description widgets, so those can never carry a
/// demo id — and the audit therefore needs the *row* to declare the
/// grouping instead. See the `declared_rows` module of the (test-only)
/// `alignment_audit` subsystem.
pub const ROW_ID_SUFFIX: &str = ".row";

/// Suffix every section caption id carries.
pub const CAPTION_ID_SUFFIX: &str = ".caption";

/// A [`rung`] the layout probe can address by name.
///
/// `id` is the caller's full stable id — built by [`label_id`] for a
/// label, or from [`CAPTION_ID_SUFFIX`] for a section heading. The wrapper
/// carries no padding, no background and no width policy, so the label
/// occupies exactly the box the text alone would have.
#[must_use]
pub fn labeled<'a>(
    value: impl Into<String>,
    type_rung: typography::TypeRung,
    color: iced::Color,
    id: &str,
) -> Element<'a, Message> {
    container(rung(value, type_rung, color))
        .id(iced::widget::Id::from(id.to_string()))
        .into()
}

/// A [`labeled`] rung pinned to a fixed horizontal **column**.
///
/// Same id convention as [`labeled`]; the only difference is that the
/// wrapper carries `Length::Fixed(width)`. The text stays left-aligned,
/// so the label still reads off the same left guide — what changes is
/// that the *cell* no longer shrinks to the word.
///
/// That is the whole reason this exists. In a stack of label/control
/// rows, a natural-width label makes the control after it start at
/// `row left + word width + gap`, i.e. at a different `x` on every row.
/// Pinning the label to a column makes the control's left edge a
/// function of the column instead, so column *k* collapses onto one
/// guide and R1's spread across the controls becomes zero.
#[must_use]
pub fn labeled_column<'a>(
    value: impl Into<String>,
    type_rung: typography::TypeRung,
    color: iced::Color,
    id: &str,
    width: f32,
) -> Element<'a, Message> {
    container(rung(value, type_rung, color))
        .id(iced::widget::Id::from(id.to_string()))
        .width(Length::Fixed(width))
        .into()
}

/// The stable, **demo-authored** label id `name` maps to under `prefix`.
///
/// Always ends in [`DEMO_LABEL_ID_SUFFIX`], never in the framework's
/// reserved `.label` role — this crate authors labels, it does not publish
/// sub-parts on someone else's widget. See
/// [`DEMO_LABEL_ID_SUFFIX`] for the collision that rule exists to avoid.
#[must_use]
pub fn label_id(prefix: &str, name: &str) -> String {
    format!("{prefix}{name}{DEMO_LABEL_ID_SUFFIX}")
}

/// Whether `id` names a label under either suffix the audit recognises.
///
/// Both shapes exist at once in a rendered demo: the framework's derived
/// `<widget-id>.label` captions, and the containers this crate authored
/// under [`DEMO_LABEL_ID_SUFFIX`]. Anything that asks "is this box a
/// label" must accept both and must not confuse either with a section card.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use]
pub fn is_label_id(id: &str) -> bool {
    id.ends_with(LABEL_ID_SUFFIX) || id.ends_with(DEMO_LABEL_ID_SUFFIX)
}

/// The stable declared-row id `name` maps to under `prefix`.
#[must_use]
pub fn row_id(prefix: &str, name: &str) -> String {
    format!("{prefix}{name}{}", ROW_ID_SUFFIX)
}

/// Paints `value` on a type rung at its resting weight.
///
/// The single place this crate turns a string into a `text` widget, so no
/// call site restates a point size ([RUST-021], `UIF-TOK-004`).
///
/// **`line_height` must reach iced as a resolved `LineHeight`, not a bare
/// `f32`.** `impl From<f32> for iced::LineHeight` produces
/// `LineHeight::Relative(factor)` — a *multiple* of the font size — so
/// `.line_height(20.0)` on a 14px rung means 280px per line.
/// [`typography::TypeRung::iced_line_height`] is the crossing point that makes
/// picking the wrong unit impossible.
#[must_use]
pub fn rung(
    value: impl Into<String>,
    rung: typography::TypeRung,
    color: iced::Color,
) -> iced::widget::Text<'static> {
    iced::widget::text(value.into())
        .size(Pixels(rung.size))
        .line_height(rung.iced_line_height())
        .color(color)
}

/// One built section: its rendered card plus how many demo widgets it holds.
///
/// The count travels with the section rather than being recomputed at the
/// status bar, so the number the window reports is the number the builder
/// actually laid down.
pub struct Section<'a> {
    /// The rendered card.
    pub element: Element<'a, Message>,
    /// How many demo widgets this section lays down.
    pub widgets: usize,
    /// The card's stable layout id, derived from the title.
    ///
    /// Every section is addressable by the layout gate without each of the
    /// ~45 builders having to invent and keep unique an id of its own.
    /// Deriving it from the title means the id is the thing a human already
    /// reads, and it cannot silently collide the way a hand-typed constant
    /// can — [`section_slug`] is deterministic and the section list is
    /// asserted duplicate-free.
    pub id: String,
}

/// The stable, id-safe form of a section title.
///
/// Lower-cased, non-alphanumerics collapsed to a single `-`, leading and
/// trailing separators trimmed. Deterministic, so the layout goldens stay
/// byte-stable across runs.
#[must_use]
pub fn section_slug(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    let mut last_was_separator = false;

    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            slug.push('-');
            last_was_separator = true;
        }
    }

    slug.trim_matches('-').to_string()
}

/// The layout id a section title maps to.
#[must_use]
pub fn section_id(title: &str) -> String {
    format!("{}{}", SECTION_ID_PREFIX, section_slug(title))
}

/// A titled card holding `body`.
///
/// `title` is the section heading, `widgets` how many demo controls
/// `body` contains, and `body` the already-spaced column of widgets the
/// section demonstrates. The card is `surface.raised` at the resting
/// elevation, so a page of sections reads as one flat sheet of grouped
/// controls rather than a stack of competing surfaces.
#[must_use]
pub fn section<'a>(title: &'static str, widgets: usize, body: Column<'a, Message>) -> Section<'a> {
    let palette = Palette::dark();
    let style = card::resolve_style(CardElevation::Rest, &palette);

    let id = section_id(title);
    let caption_id = format!("{id}{}", CAPTION_ID_SUFFIX);

    let mut heading = Row::new()
        .align_y(Alignment::Center)
        .spacing(HEADING_BADGE_GAP)
        .push(labeled(
            title,
            typography::TITLE_SM,
            palette.text_strong,
            &caption_id,
        ));

    if widgets > 0 {
        heading = heading.push(rung(
            format!("{widgets} widget{}", if widgets == 1 { "" } else { "s" }),
            typography::CAPTION,
            palette.text_muted,
        ));
    }

    let content = Column::new()
        .width(Length::Fill)
        .spacing(HEADING_GAP)
        .push(heading)
        .push(body);

    let element = container(content)
        .id(iced::widget::Id::from(id.clone()))
        .width(Length::Fill)
        .padding(style.padding)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(style.background)),
            border: Border {
                color: style.border_color,
                width: style.border_width,
                radius: style.radius.into(),
            },
            shadow: Shadow::default(),
            text_color: Some(palette.text_primary),
            snap: true,
        })
        .into();

    Section {
        element,
        widgets,
        id,
    }
}

impl<'a> From<Section<'a>> for Element<'a, Message> {
    fn from(section: Section<'a>) -> Self {
        section.element
    }
}

/// A pane's worth of sections, stacked and totalled.
pub struct PaneBody<'a> {
    /// The sections, stacked on the token's [`SECTION_GAP`].
    pub sections: Column<'a, Message>,
    /// Total demo widgets laid down across every section.
    pub widgets: usize,
}

impl<'a> PaneBody<'a> {
    /// Stacks `sections` into a pane body and sums their widget counts.
    #[must_use]
    pub fn new(sections: Vec<Section<'a>>) -> Self {
        let widgets = sections.iter().map(|section| section.widgets).sum();
        Self {
            sections: Column::with_children(sections.into_iter().map(|s| s.element))
                .width(Length::Fill)
                .spacing(SECTION_GAP),
            widgets,
        }
    }
}

#[cfg(test)]
#[path = "section_tests.rs"]
mod tests;
