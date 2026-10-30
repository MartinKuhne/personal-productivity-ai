//! Resolver-driven static previews for the atoms and composites that have
//! no live `iced::Widget` behind them.
//!
//! Several framework pieces are **resolver-only**: the pure
//! `(variant × state × palette) → style` function exists and is audited,
//! but nothing in the framework paints it into a control a user can touch.
//! Those cannot be demoed interactively, so this module paints what the
//! resolver actually returned through an iced `container` — the same way a
//! host renders a resolved value it has been handed.
//!
//! A preview is honest about what it is. It shows the resolved colour of
//! every channel the resolver produced, names the channel, and does not
//! respond to hover, press or focus. Every preview therefore reports
//! **zero interactive widgets**, which is why the status bar's count never
//! moves when a preview scrolls past.
//!
//! This mirrors the position `hoarfrost::gallery` takes: snapshot the
//! resolved *values*, not the picture, because a raster is dependent on
//! the renderer, the font and the hinting settings of the machine that
//! drew it.
//!
//! Unit tests live in the sibling `previews_tests.rs` sidecar.

use iced::widget::{Column, Row, container};
use iced::{Alignment, Background, Border, Color, Element, Length};

use hoarfrost::composite::breadcrumb::{self, BreadcrumbItem, CrumbRole};
use hoarfrost::composite::card::{self, CardElevation};
use hoarfrost::composite::info_bar::{self, BarSurface};
use hoarfrost::composite::list_tile;
use hoarfrost::composite::notification::Severity;
use hoarfrost::composite::overflow_toolbar::{self, OverflowItem, OverflowRank};
use hoarfrost::composite::table_cell::{CellBand, CellStyle};
use hoarfrost::composite::table_column::{self, CellAlign, ColumnSpec};
use hoarfrost::state::{BaseState, VisualState};
use hoarfrost::token::color::Palette;
use hoarfrost::token::{space, typography};
use hoarfrost::widget::badge;
use hoarfrost::widget::badge_model::{BadgeModel, StatusIcon};
use hoarfrost::widget::chip::{self, ChipPolarity};
use hoarfrost::widget::chip_model::ChipHeight;
use hoarfrost::widget::number_box;
use hoarfrost::widget::progress;
use hoarfrost::widget::progress_model::{ProgressModel, ProgressStatus};
use hoarfrost::widget::slider;

use crate::app::Message;
use crate::section::{Section, rung, section};

/// Edge length of a painted colour chip.
const SWATCH_W: f32 = 56.0;
/// Height of a painted colour chip.
const SWATCH_H: f32 = 22.0;
/// Gap between the chips inside one resolved variant.
const CHANNEL_GAP: f32 = space::XS;
/// Gap between resolved variants inside one preview section.
const VARIANT_GAP: f32 = space::SM;

/// One resolved colour channel, named and painted.
struct Channel {
    /// The resolver's own field name, so the reader can look it up.
    name: &'static str,
    /// The colour the resolver actually returned for it.
    colour: Color,
}

impl Channel {
    fn new(name: &'static str, colour: Color) -> Self {
        Self { name, colour }
    }
}

/// A resolved variant: a label plus every channel it produced.
struct Variant {
    /// What was resolved, e.g. `"Selected · Bar · Hovered"`.
    label: String,
    /// The channels, in the resolver's declaration order.
    channels: Vec<Channel>,
}

impl Variant {
    fn new(label: impl Into<String>, channels: Vec<Channel>) -> Self {
        Self {
            label: label.into(),
            channels,
        }
    }
}

/// Formats a [`Color`] as `#RRGGBBAA` so a resolved value is readable as
/// text and not only as a patch of paint.
#[must_use]
pub fn hex(colour: Color) -> String {
    let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        channel(colour.r),
        channel(colour.g),
        channel(colour.b),
        channel(colour.a)
    )
}

/// Paints one channel: the colour as a box, its name and hex beneath.
///
/// The box always carries a hairline perimeter so a fully transparent
/// resolved colour is visibly *empty* rather than indistinguishable from
/// the card behind it.
#[must_use]
fn swatch(channel: &Channel) -> Element<'static, Message> {
    let palette = Palette::dark();
    // Copy the resolved colour out of the borrow so the style closure the
    // container captures is `'static` and not tied to `channel`'s lifetime.
    let colour = channel.colour;
    container(
        Column::new()
            .align_x(Alignment::Center)
            .spacing(2.0)
            .push(
                container(iced::widget::Space::new().width(SWATCH_W).height(SWATCH_H))
                    .width(Length::Fixed(SWATCH_W))
                    .height(Length::Fixed(SWATCH_H))
                    .style(move |_theme: &iced::Theme| container::Style {
                        background: Some(Background::Color(colour)),
                        border: Border {
                            color: palette.border_strong,
                            width: 1.0,
                            radius: 2.0.into(),
                        },
                        text_color: None,
                        shadow: Default::default(),
                        snap: true,
                    }),
            )
            .push(rung(channel.name, typography::CAPTION, palette.text_muted))
            .push(rung(
                hex(channel.colour),
                typography::CAPTION,
                palette.text_disabled,
            )),
    )
    .into()
}

/// Renders one resolved variant: its label above a row of channel swatches.
#[must_use]
fn variant_row(variant: &Variant) -> Element<'static, Message> {
    let palette = Palette::dark();
    let mut chips = Row::new().spacing(CHANNEL_GAP).align_y(Alignment::Start);
    for channel in &variant.channels {
        chips = chips.push(swatch(channel));
    }

    Column::new()
        .spacing(2.0)
        .push(rung(
            &variant.label,
            typography::UI_COMPACT,
            palette.text_primary,
        ))
        .push(chips)
        .into()
}

/// Builds a preview section: the variants plus the standing disclaimer that
/// nothing here responds to input.
fn preview_section(title: &'static str, variants: Vec<Variant>) -> Section<'static> {
    let palette = Palette::dark();
    let mut body = Column::new().width(Length::Fill).spacing(VARIANT_GAP);
    for variant in &variants {
        body = body.push(variant_row(variant));
    }
    body = body.push(rung(
        "Resolved value only — non-interactive, 0 widgets.",
        typography::CAPTION,
        palette.text_disabled,
    ));
    section(title, 0, body)
}

/// The five base states a preview sweeps, in the framework's own order.
const BASE_STATES: [(BaseState, &str); 5] = [
    (BaseState::Rest, "Rest"),
    (BaseState::Hovered, "Hovered"),
    (BaseState::Pressed, "Pressed"),
    (BaseState::Disabled, "Disabled"),
    (BaseState::Invalid, "Invalid"),
];

/// A [`VisualState`] for a base state plus the keyboard-focus flag.
fn vs(base: BaseState, focused: bool) -> VisualState {
    VisualState {
        base,
        focused: focused && base != BaseState::Disabled,
    }
}

/// The resting visual state, for resolvers swept across some other axis
/// rather than the interaction-state matrix.
const REST: VisualState = VisualState {
    base: BaseState::Rest,
    focused: false,
};

// ---------------------------------------------------------------------------
// Atoms-pane previews
// ---------------------------------------------------------------------------

/// Builds every preview section destined for the atoms pane.
#[must_use]
pub fn atom_preview_sections() -> Vec<Section<'static>> {
    vec![
        chip_preview(),
        badge_preview(),
        progress_preview(),
        slider_preview(),
        number_box_preview(),
    ]
}

/// `chip::resolve_style(polarity, height, state, palette)`.
#[must_use]
pub fn chip_preview() -> Section<'static> {
    let palette = Palette::dark();
    let mut variants = Vec::new();

    for (polarity, polarity_name) in [
        (ChipPolarity::Unselected, "Unselected"),
        (ChipPolarity::Selected, "Selected"),
    ] {
        for (height, height_name) in [(ChipHeight::Compact, "Compact"), (ChipHeight::Bar, "Bar")] {
            for base in [BaseState::Rest, BaseState::Hovered, BaseState::Disabled] {
                let style = chip::resolve_style(polarity, height, vs(base, false), &palette);
                variants.push(Variant::new(
                    format!("{polarity_name} · {height_name} · {base:?}"),
                    vec![
                        Channel::new("fill", style.fill),
                        Channel::new("border", style.border),
                        Channel::new("text", style.text),
                        Channel::new("glyph", style.glyph),
                        Channel::new("focus_ring", style.focus_ring_color),
                    ],
                ));
                let _ = height_name;
            }
        }
    }

    // The focus ring is a separate axis: same paint, ring drawn.
    let style = chip::resolve_style(
        ChipPolarity::Selected,
        ChipHeight::Bar,
        vs(BaseState::Rest, true),
        &palette,
    );
    variants.push(Variant::new(
        "Selected · Bar · Rest + focus ring",
        vec![
            Channel::new("fill", style.fill),
            Channel::new("focus_ring", style.focus_ring_color),
        ],
    ));

    preview_section("Chip resolver (UIF-ATOM-006)", variants)
}

/// `badge::resolve_style(model, palette)` for each of the three kinds.
#[must_use]
pub fn badge_preview() -> Section<'static> {
    let palette = Palette::dark();
    let models = [
        ("Dot · Neutral", BadgeModel::dot()),
        (
            "Count(7) · Accent",
            BadgeModel::count(7).with_tone(hoarfrost::widget::badge_model::BadgeTone::Accent),
        ),
        (
            "Status · Success",
            BadgeModel::status(StatusIcon::Success, "Done")
                .with_tone(hoarfrost::widget::badge_model::BadgeTone::Success),
        ),
        (
            "Status · Danger",
            BadgeModel::status(StatusIcon::Danger, "Failed")
                .with_tone(hoarfrost::widget::badge_model::BadgeTone::Danger),
        ),
    ];

    let variants = models
        .into_iter()
        .map(|(label, model)| {
            let style = badge::resolve_style(&model, &palette);
            Variant::new(
                label,
                vec![
                    Channel::new("fill", style.fill),
                    Channel::new("border", style.border),
                    Channel::new("text", style.text),
                    Channel::new("glyph", style.glyph),
                ],
            )
        })
        .collect();

    preview_section("Badge resolver (UIF-ATOM-008)", variants)
}

/// `progress::resolve_style(model, palette)` across status and mode.
#[must_use]
pub fn progress_preview() -> Section<'static> {
    let palette = Palette::dark();
    let cases = [
        (
            "Determinate · Active · 62%",
            ProgressModel::determinate(62, 100),
        ),
        (
            "Determinate · Paused · 62%",
            ProgressModel::determinate(62, 100).with_status(ProgressStatus::Paused),
        ),
        (
            "Determinate · Error · 62%",
            ProgressModel::determinate(62, 100).with_status(ProgressStatus::Error),
        ),
        ("Indeterminate · Active", ProgressModel::indeterminate()),
    ];

    let variants = cases
        .into_iter()
        .map(|(label, model)| {
            let style = progress::resolve_style(&model, &palette);
            Variant::new(
                label,
                vec![
                    Channel::new("track", style.track),
                    Channel::new("fill", style.fill),
                ],
            )
        })
        .collect();

    preview_section("Progress resolver (UIF-ATOM-009)", variants)
}

/// `slider::resolve_style(state, mount, palette)` across the base states.
#[must_use]
pub fn slider_preview() -> Section<'static> {
    let palette = Palette::dark();
    let mount = palette.surface_base;

    let variants = BASE_STATES
        .into_iter()
        .map(|(base, name)| {
            let style = slider::resolve_style(vs(base, false), mount, &palette);
            Variant::new(
                format!("Slider · {name}"),
                vec![
                    Channel::new("rail_unfilled", style.rail_unfilled),
                    Channel::new("rail_filled", style.rail_filled),
                    Channel::new("thumb_fill", style.thumb_fill),
                    Channel::new("thumb_border", style.thumb_border),
                    Channel::new("tick", style.tick),
                ],
            )
        })
        .collect();

    preview_section("Slider resolver (UIF-ATOM-010)", variants)
}

/// `number_box::resolve_style(state, palette)` across the base states.
#[must_use]
pub fn number_box_preview() -> Section<'static> {
    let palette = Palette::dark();

    let variants = BASE_STATES
        .into_iter()
        .map(|(base, name)| {
            let style = number_box::resolve_style(vs(base, false), &palette);
            Variant::new(
                format!("Number box · {name}"),
                vec![
                    Channel::new("entry.bg", style.entry.background),
                    Channel::new("entry.text", style.entry.text),
                    Channel::new("entry.border", style.entry.border_color),
                    Channel::new("stepper.bg", style.stepper_background),
                    Channel::new("stepper.glyph", style.stepper_glyph),
                ],
            )
        })
        .collect();

    preview_section("Number box resolver (UIF-ATOM-011)", variants)
}

// ---------------------------------------------------------------------------
// Composites-pane previews
// ---------------------------------------------------------------------------

/// Builds every preview section destined for the composites pane.
#[must_use]
pub fn composite_preview_sections() -> Vec<Section<'static>> {
    vec![
        card_preview(),
        list_tile_preview(),
        table_preview(),
        notification_preview(),
        breadcrumb_preview(),
        overflow_toolbar_preview(),
    ]
}

/// `card::resolve_style(elevation, palette)`.
#[must_use]
pub fn card_preview() -> Section<'static> {
    let palette = Palette::dark();
    let variants = [
        (CardElevation::Rest, "Rest"),
        (CardElevation::Elevated, "Elevated"),
    ]
    .into_iter()
    .map(|(elevation, name)| {
        let style = card::resolve_style(elevation, &palette);
        Variant::new(
            format!("Card · {name}"),
            vec![
                Channel::new("background", style.background),
                Channel::new("border", style.border_color),
            ],
        )
    })
    .collect();

    preview_section("Card resolver (UIF-COMP-005)", variants)
}

/// `list_tile::resolve_style(selected, disabled, base, palette)`.
#[must_use]
pub fn list_tile_preview() -> Section<'static> {
    let palette = Palette::dark();
    let cases = [
        ("Tile · plain", false, false, BaseState::Rest),
        ("Tile · hovered", false, false, BaseState::Hovered),
        ("Tile · selected", true, false, BaseState::Rest),
        ("Tile · selected + hovered", true, false, BaseState::Hovered),
        ("Tile · disabled", false, true, BaseState::Disabled),
    ];

    let variants = cases
        .into_iter()
        .map(|(label, selected, disabled, base)| {
            let style = list_tile::resolve_style(selected, disabled, base, &palette);
            Variant::new(
                label,
                vec![
                    Channel::new("background", style.background),
                    Channel::new("title", style.title_colour),
                    Channel::new("subtitle", style.subtitle_colour),
                    Channel::new("leading", style.leading_colour),
                ],
            )
        })
        .collect();

    preview_section("List tile resolver (UIF-COMP-006)", variants)
}

/// `table_column::resolve_widths` plus `CellStyle::resolve` per band.
#[must_use]
pub fn table_preview() -> Section<'static> {
    let palette = Palette::dark();

    let specs = vec![
        ColumnSpec::fixed(0, "Status", 96.0),
        ColumnSpec::content(1, "Name", 220.0).with_min(120.0),
        ColumnSpec::proportional(2, "Size", 1.0).with_min(64.0),
        ColumnSpec::proportional(3, "Modified", 2.0)
            .with_min(96.0)
            .with_align(CellAlign::End),
    ];
    let layout = table_column::resolve_widths(&specs, TABLE_PREVIEW_WIDTH);

    let widths = layout
        .widths()
        .iter()
        .map(|w| format!("{w:.0}"))
        .collect::<Vec<_>>()
        .join(" / ");

    let mut variants = vec![Variant::new(
        format!("Column widths @ {:.0}px → {widths}", TABLE_PREVIEW_WIDTH),
        vec![
            Channel::new("band.base", palette.surface_base),
            Channel::new("band.raised", palette.surface_raised),
        ],
    )];

    for band in [
        CellBand::Base,
        CellBand::Raised,
        CellBand::Hover,
        CellBand::Selected,
    ] {
        let style = CellStyle::resolve(band, &palette);
        variants.push(Variant::new(
            format!("Cell · {band:?}"),
            vec![
                Channel::new("background", style.background),
                Channel::new("foreground", style.foreground),
                Channel::new("grid_line", style.grid_line),
                Channel::new("outline", style.outline),
            ],
        ));
    }

    preview_section("Table resolver (UIF-COMP-011)", variants)
}

/// `info_bar::resolve_style(severity, surface, palette)` — the paint a
/// toast or notification carries.
#[must_use]
pub fn notification_preview() -> Section<'static> {
    let palette = Palette::dark();
    let variants = Severity::ALL
        .into_iter()
        .map(|severity| {
            let style = info_bar::resolve_style(severity, BarSurface::Overlay, &palette);
            Variant::new(
                format!("Notification · {severity:?} (overlay)"),
                vec![
                    Channel::new("background", style.background),
                    Channel::new("stripe", style.stripe_colour),
                    Channel::new("icon", style.icon_colour),
                    Channel::new("title", style.title_colour),
                    Channel::new("body", style.body_colour),
                    Channel::new("dismiss", style.dismiss_colour),
                ],
            )
        })
        .collect();

    preview_section("Toast / notification resolver (UIF-COMP-009)", variants)
}

/// `breadcrumb::collapse` and the crumb colour resolver.
#[must_use]
pub fn breadcrumb_preview() -> Section<'static> {
    let palette = Palette::dark();
    let items: Vec<BreadcrumbItem<String>> = [
        ("vault", "Vault", 64.0),
        ("doc", "Documents", 96.0),
        ("sub", "2026", 64.0),
        ("dir", "Planning", 88.0),
        ("file", "roadmap.md", 104.0),
    ]
    .into_iter()
    .map(|(id, label, measure)| BreadcrumbItem::new(id.to_string(), label, measure))
    .collect();

    let layout = breadcrumb::collapse(&items, BREADCRUMB_PREVIEW_WIDTH);

    let mut variants = vec![Variant::new(
        format!(
            "Trail @ {:.0}px → {} inline, {} hidden",
            BREADCRUMB_PREVIEW_WIDTH,
            layout.inline_count(),
            layout.hidden_count()
        ),
        vec![
            Channel::new("separator", breadcrumb::separator_colour(&palette)),
            Channel::new(
                "crumb.root",
                breadcrumb::crumb_colour(&palette, CrumbRole::Root, REST),
            ),
            Channel::new(
                "crumb.terminal",
                breadcrumb::crumb_colour(&palette, CrumbRole::Terminal, REST),
            ),
            Channel::new(
                "crumb.hover",
                breadcrumb::crumb_colour(
                    &palette,
                    CrumbRole::Middle,
                    VisualState {
                        base: BaseState::Hovered,
                        focused: false,
                    },
                ),
            ),
        ],
    )];

    // The same trail in a box too narrow for it, which is what makes the
    // elision visible without a live resize.
    let tight = breadcrumb::collapse(&items, BREADCRUMB_TIGHT_WIDTH);
    variants.push(Variant::new(
        format!(
            "Trail @ {:.0}px → {} inline, {} hidden (elided)",
            BREADCRUMB_TIGHT_WIDTH,
            tight.inline_count(),
            tight.hidden_count()
        ),
        vec![Channel::new(
            "trigger.glyph",
            breadcrumb::trigger_glyph_colour(&palette, REST),
        )],
    ));

    preview_section("Breadcrumb resolver (UIF-COMP-004)", variants)
}

/// `overflow_toolbar::fit(items, width)` at two widths.
#[must_use]
pub fn overflow_toolbar_preview() -> Section<'static> {
    let palette = Palette::dark();
    let items: Vec<OverflowItem<String>> = [
        ("save", "Save", 64.0, OverflowRank::PRIMARY),
        ("copy", "Copy", 60.0, OverflowRank::DISPOSABLE),
        ("share", "Share", 68.0, OverflowRank::SECONDARY),
        ("print", "Print", 64.0, OverflowRank::DISPOSABLE),
        ("export", "Export", 72.0, OverflowRank::LAST),
    ]
    .into_iter()
    .map(|(id, label, measure, rank)| OverflowItem::new(id.to_string(), label, measure, rank))
    .collect();

    let mut variants = Vec::new();
    for width in [OVERFLOW_WIDE_WIDTH, OVERFLOW_TIGHT_WIDTH] {
        let layout = overflow_toolbar::fit(&items, width);
        let inline: Vec<&str> = layout
            .plans()
            .iter()
            .filter(|plan| plan.is_inline())
            .map(|plan| items[plan.index].label.as_str())
            .collect();
        let hidden: Vec<&str> = layout
            .plans()
            .iter()
            .filter(|plan| plan.is_overflow())
            .map(|plan| items[plan.index].label.as_str())
            .collect();

        variants.push(Variant::new(
            format!(
                "Strip @ {width:.0}px → inline [{}] · overflow [{}]",
                inline.join(", "),
                hidden.join(", ")
            ),
            vec![
                Channel::new("inline", palette.text_primary),
                Channel::new("overflow", palette.text_muted),
            ],
        ));
    }

    preview_section("Overflow toolbar resolver (UIF-COMP-008)", variants)
}

/// Width the table preview solves its columns against.
pub const TABLE_PREVIEW_WIDTH: f32 = 640.0;
/// Width the breadcrumb preview collapses its trail against.
pub const BREADCRUMB_PREVIEW_WIDTH: f32 = 520.0;
/// A width too small for the whole trail, so elision is visible.
pub const BREADCRUMB_TIGHT_WIDTH: f32 = 220.0;
/// A strip width that holds every item.
pub const OVERFLOW_WIDE_WIDTH: f32 = 480.0;
/// A strip width that forces eviction.
pub const OVERFLOW_TIGHT_WIDTH: f32 = 200.0;

#[cfg(test)]
#[path = "previews_tests.rs"]
mod tests;
