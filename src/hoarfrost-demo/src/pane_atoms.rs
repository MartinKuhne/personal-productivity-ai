//! The LEFT pane — every atom the framework exposes.
//!
//! Each atom gets its own [`crate::section`] and each section is a small
//! pure builder function `fn <atom>_section(..) -> Section`. The pane
//! itself is nothing but the ordered list of those builders, so adding an
//! atom means adding one function and one line to [`live_sections`].
//!
//! # Where the state lives
//!
//! Several framework widgets are **controlled**: they borrow a host-owned
//! model each frame rather than owning state. Those models therefore live
//! in [`AtomsState`], which [`crate::app::DemoApp`] holds. Every callback
//! the widgets fire lands in [`AtomsMessage`], which [`AtomsState::apply`]
//! folds in — nothing leaves this module and nothing is persisted.
//!
//! The *content* of those models — labels, options, sample values — comes
//! from [`crate::data`], never from this module, so the pane stays a
//! renderer and the fixtures stay in one place.
//!
//! Resolver-only atoms are appended from [`crate::previews`] after the
//! live ones, so the interactive controls come first.
//!
//! Unit tests live in the sibling `pane_atoms_tests.rs` sidecar.

use std::collections::BTreeSet;

use iced::widget::{Column, Row, container};
use iced::{Alignment, Length};

use hoarfrost::binding::Controlled;
use hoarfrost::surface::tooltip::{TooltipContent, tooltip};
use hoarfrost::token::color::Palette;
use hoarfrost::token::typography;
use hoarfrost::validation::EditStance;
use hoarfrost::widget::avatar::{AvatarModel, AvatarSize, avatar};
use hoarfrost::widget::button::ButtonVariant;
use hoarfrost::widget::button_model::ButtonModel;
use hoarfrost::widget::button_widget::Button;
use hoarfrost::widget::color_picker::{ColorPickerModel, Rgba8, color_picker, color_swatch_button};
use hoarfrost::widget::combo_box::combo_box;
use hoarfrost::widget::combo_model::ComboBoxModel;
use hoarfrost::widget::date_picker::{
    DatePickerModel, DateSelection, date_picker, date_range_picker,
};
use hoarfrost::widget::hyperlink::Hyperlink;
use hoarfrost::widget::hyperlink_model::HyperlinkTarget;
use hoarfrost::widget::range_slider::range_slider;
use hoarfrost::widget::range_slider_model::RangeSliderModel;
use hoarfrost::widget::rating::{RatingModel, rating_control};
use hoarfrost::widget::search_box::SearchBox;
use hoarfrost::widget::segmented::SegmentedButton;
use hoarfrost::widget::segmented_model::{SegmentItem, SegmentedModel};
use hoarfrost::widget::split_button::SplitButton;
use hoarfrost::widget::split_button_host::SplitButtonEmission;
use hoarfrost::widget::split_button_model::{SplitButtonModel, SplitMenuRow};
use hoarfrost::widget::text_input::{InputKind, TextInput};
use hoarfrost::widget::text_input_controller::TextInputEmission;
use hoarfrost::widget::text_input_model::TextInputModel;
use hoarfrost::widget::time_picker::{CivilTime, TimePickerModel, time_picker};
use hoarfrost::widget::toggle_model::{CheckState, RadioGroupModel, RadioOption, RadioOrientation};
use hoarfrost::widget::toggle_widget::{Checkbox, RadioGroup, ToggleSwitch};

use crate::app::Message;
use crate::data;
use crate::section::{PaneBody, Section, label_id, rung, section_slug};

/// Width every demo button's label box is pinned to.
///
/// `Button::is_icon_only` is a **geometric** test — content no wider and
/// no taller than `size::CONTROL_SM` (28px) is classed icon-only, and an
/// icon-only button with no tooltip trips a `debug_assert!`. Pinning the
/// label box to the framework's dialog-button minimum keeps every demo
/// button unambiguously labelled at ordinary widths.
///
/// It is not sufficient on its own: iced clamps `Length::Fixed` to the
/// width the parent actually offers, so a narrow viewport can squeeze the
/// box under the icon-only threshold anyway. Every demo button therefore
/// also carries a tooltip, which is the framework's own escape hatch for
/// the icon-only cell (`UIF-ATOM-007`).
const LABEL_WIDTH: f32 = hoarfrost::surface::input_dialog::BUTTON_MIN_WIDTH;

/// Stable id of the atoms pane's scrollable.
pub const PANE_SCROLL_ID: &str = "demo.pane.atoms.scroll";

/// Prefix of every Buttons-row **label** id.
///
/// The variant name in front of each button row is the one label in this
/// pane the demo renders itself rather than a framework widget, so it is
/// the one atom label the audit can address. See
/// [`DEMO_LABEL_ID_SUFFIX`](crate::section::DEMO_LABEL_ID_SUFFIX) for why
/// naming it needs a container wrapper, and why that wrapper's suffix is
/// `.demo-label` rather than the framework's reserved `.label` role.
pub const BUTTON_ROW_LABEL_PREFIX: &str = "demo.atoms.buttons.";

/// Width of the Buttons-row **label column**.
///
/// Each row used to let its variant name take its natural text width, so
/// the first control after it started at a different `x` on every row:
///
/// ```text
/// Primary    45.60px  ->  col1 69.60   col2 261.60
/// Secondary  61.59px  ->  col1 85.59   col2 277.59
/// Ghost      34.18px  ->  col1 58.18   col2 250.18
/// Danger     41.59px  ->  col1 65.59   col2 257.59
/// ```
///
/// a 27.42px scatter across four rows whose buttons are all exactly
/// 88 × 32. Identical controls cannot be why they are out of line — the
/// label was doing all the moving, which is exactly what R1 attributes
/// to the composition.
///
/// Pinning the label to a fixed column removes the scatter: every row's
/// controls now start at `card padding + LABEL_COLUMN + space::SM`, so
/// column 1 and column 2 each collapse onto a single guide.
///
/// **Why 64.** `64.0` is the smallest value on the token scale's base-4
/// grid that clears the longest variant label — `Secondary` at 61.59px —
/// and it is the smallest multiple of the largest rung,
/// [`space::XXL`](hoarfrost::token::space::XXL). The column is therefore
/// no wider than the words it carries, and it is a rung rather than a
/// measured magic number ([RUST-021], `UIF-TOK-001`).
const LABEL_COLUMN: f32 = hoarfrost::token::space::XXL * 2.0;

/// The four button variants shown in the Buttons section.
const BUTTON_VARIANTS: [(&str, ButtonVariant); 4] = [
    ("Primary", ButtonVariant::Primary),
    ("Secondary", ButtonVariant::Secondary),
    ("Ghost", ButtonVariant::Ghost),
    ("Danger", ButtonVariant::Danger),
];

/// How many widgets the Buttons section lays down: 4 variants × 3 states.
const BUTTON_WIDGET_COUNT: usize = BUTTON_VARIANTS.len() * 3;

/// What an atom in this pane reported.
#[derive(Debug, Clone)]
pub enum AtomsMessage {
    /// Checkbox `index` moved to a new tri-state.
    Checkbox(usize, CheckState),
    /// The radio group's selected id changed.
    RadioSelected(String),
    /// The live toggle switch flipped.
    SwitchToggled(bool),
    /// The segmented strip's selection set changed.
    SegmentedChanged(BTreeSet<String>),
    /// The split button's flyout opened or closed.
    SplitMenuToggled(bool),
    /// The split button's flyout row `ActionId` was chosen.
    SplitRowChosen(String),
    /// A hyperlink was activated.
    LinkOpened(String),
    /// The live text field's value changed (legacy `on_input` path).
    TextTyped(String),
    /// The controlled text field produced a tier: `(tier name, value)`.
    ///
    /// The tier name is carried so the status bar shows *which* of the
    /// `UIF-ATOM-002a` tiers fired, not just the resulting string.
    TextInputTier(&'static str, String),
    /// The password field's value changed.
    PasswordTyped(String),
    /// The password field's reveal toggle fired; payload is the new state.
    RevealToggled(bool),
    /// The combo box flyout opened or closed.
    ComboToggled(bool),
    /// The combo box's selected option id.
    ComboChosen(String),
    /// The search box's query changed.
    SearchChanged(String),
    /// The search box's Enter was pressed.
    SearchSubmitted(String),
    /// The search box's trailing clear button was pressed.
    SearchCleared,
    /// The rating control's committed value (`None` = cleared).
    RatingPicked(Option<f32>),
    /// The range slider's thumbs during a scrub, `(lower, upper)`.
    RangeLive(f64, f64),
    /// The range slider's thumbs once the drag settled, `(lower, upper)`.
    RangeCommitted(f64, f64),
    /// The single-date picker's selection.
    DatePicked(DateSelection),
    /// The date-range picker's selection.
    DateRangePicked(DateSelection),
    /// The date picker flyout opened or closed.
    DateFlyoutToggled(bool),
    /// The time picker's committed time.
    TimePicked(CivilTime),
    /// The time picker flyout opened or closed.
    TimeFlyoutToggled(bool),
    /// The colour picker's committed colour.
    ColorPicked(Rgba8),
    /// The colour picker flyout opened or closed.
    ColorFlyoutToggled(bool),
    /// A plain status note for the status bar.
    Note(String),
}

/// Everything the atoms pane owns between frames.
#[derive(Debug)]
pub struct AtomsState {
    /// The tri-state of each demo checkbox.
    pub checkboxes: Vec<CheckState>,
    /// The radio group's model (the widget is controlled).
    pub radio: RadioGroupModel<String>,
    /// The live toggle switch's position.
    pub switch_on: bool,
    /// The segmented strip's model (the widget is controlled).
    pub segmented: SegmentedModel<String>,
    /// The split button's model (the widget is controlled).
    pub split: SplitButtonModel<String>,
    /// One busy [`ButtonModel`] per demo variant, kept alive so the
    /// controlled buttons can borrow them across frames.
    pub busy_models: Vec<ButtonModel>,
    /// The live text field's value (legacy `on_input` path).
    pub text: String,
    /// The controlled text field's model (`UIF-ATOM-002a`).
    pub text_model: TextInputModel,
    /// The invalid text field's model — a bound field whose host verdict
    /// is already recorded and whose stance is already `Touched`, so the
    /// error is painted rather than withheld (`UIF-PRIN-009`).
    pub text_invalid_model: TextInputModel,
    /// The password field's value.
    pub password: String,
    /// Whether the password field is currently revealed.
    pub revealed: bool,
    /// The live combo box's model (open/closed and selection live here).
    pub combo: ComboBoxModel<String>,
    /// The invalid combo box's model, held so the widget can borrow it.
    pub combo_invalid: ComboBoxModel<String>,
    /// The disabled combo box's model, held so the widget can borrow it.
    pub combo_disabled: ComboBoxModel<String>,
    /// The search box's query.
    pub search: String,
    /// The rating control's model.
    pub rating: RatingModel,
    /// The live range slider's model.
    pub range: RangeSliderModel,
    /// The disabled range slider's model.
    pub range_disabled: RangeSliderModel,
    /// The single-date picker's model.
    pub date: DatePickerModel,
    /// The date-range picker's model.
    pub date_range: DatePickerModel,
    /// The time picker's model.
    pub time: TimePickerModel,
    /// The colour picker's model.
    pub color: ColorPickerModel,
}

impl Default for AtomsState {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomsState {
    /// The atoms pane's initial state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            checkboxes: data::CHECKBOX_ROWS.iter().map(|row| row.1).collect(),
            radio: radio_model(data::RADIO_SELECTED),
            switch_on: true,
            segmented: segmented_model(data::SEGMENT_SELECTED),
            split: split_model(),
            busy_models: BUTTON_VARIANTS
                .iter()
                .map(|(_, variant)| ButtonModel::new("Busy").with_variant(*variant).busy())
                .collect(),
            text: String::from(data::TEXT_SAMPLE),
            text_model: TextInputModel::new(data::TEXT_SAMPLE)
                .with_placeholder(data::TEXT_PLACEHOLDER)
                .with_clearable(true),
            text_invalid_model: TextInputModel::new(data::TEXT_INVALID_SAMPLE)
                .with_validation(data::TEXT_ERROR)
                .with_stance(EditStance::Touched),
            password: String::from(data::PASSWORD_SAMPLE),
            revealed: false,
            combo: ComboBoxModel::new(data::combo_options(), data::COMBO_PLACEHOLDER)
                .with_selected(String::from(data::COMBO_SELECTED)),
            combo_invalid: ComboBoxModel::new(data::combo_options(), data::COMBO_PLACEHOLDER)
                .with_validation(data::COMBO_ERROR),
            combo_disabled: ComboBoxModel::new(data::combo_options(), data::COMBO_PLACEHOLDER)
                .with_selected(String::from(data::COMBO_SELECTED))
                .disabled(),
            search: String::from(data::SEARCH_SAMPLE),
            rating: RatingModel::new(data::RATING_MAX)
                .with_value(data::RATING_VALUE)
                .with_allow_clear(true),
            range: range_model(data::RANGE_BOUNDS.2, data::RANGE_BOUNDS.3),
            range_disabled: range_model(data::RANGE_BOUNDS.2, data::RANGE_BOUNDS.3)
                .with_disabled(true),
            date: DatePickerModel::new(data::date(data::DATE_SELECTED))
                .with_selection(DateSelection::SingleDate(Some(data::date(
                    data::DATE_SELECTED,
                ))))
                .with_blackout_dates(data::DATE_BLACKOUTS.iter().map(|day| data::date(*day))),
            date_range: DatePickerModel::new(data::date(data::DATE_RANGE.0)).with_selection(
                DateSelection::DateRange {
                    start: Some(data::date(data::DATE_RANGE.0)),
                    end: Some(data::date(data::DATE_RANGE.1)),
                },
            ),
            time: TimePickerModel::new(Some(data::time(data::TIME_SELECTED))),
            color: ColorPickerModel::new(data::rgb(data::COLOR_SELECTED))
                .with_contrast_reference(data::rgb(data::COLOR_CONTRAST_REF)),
        }
    }

    /// Folds one [`AtomsMessage`] into the pane's state.
    pub fn apply(&mut self, message: &AtomsMessage) {
        match message {
            AtomsMessage::Checkbox(index, state) => {
                if let Some(slot) = self.checkboxes.get_mut(*index) {
                    *slot = *state;
                }
            }
            AtomsMessage::RadioSelected(id) => {
                self.radio = radio_model(id.clone());
            }
            AtomsMessage::SwitchToggled(on) => self.switch_on = *on,
            AtomsMessage::SegmentedChanged(set) => {
                self.segmented = self.segmented.clone().with_selection(set.clone());
            }
            AtomsMessage::SplitMenuToggled(open) => self.split.menu_open = *open,
            AtomsMessage::TextTyped(value) => self.text = value.clone(),
            AtomsMessage::TextInputTier(_, value) => self.text_model.value = value.clone(),
            AtomsMessage::PasswordTyped(value) => self.password = value.clone(),
            AtomsMessage::RevealToggled(revealed) => self.revealed = *revealed,
            AtomsMessage::ComboToggled(open) => {
                self.combo = self.combo.clone().with_open(*open);
            }
            AtomsMessage::ComboChosen(id) => {
                self.combo = self
                    .combo
                    .clone()
                    .with_selected(Some(id.clone()))
                    .with_open(false);
            }
            AtomsMessage::SearchChanged(query) => self.search = query.clone(),
            AtomsMessage::SearchSubmitted(query) => self.search = query.clone(),
            AtomsMessage::SearchCleared => self.search = String::new(),
            AtomsMessage::RatingPicked(value) => {
                self.rating = self.rating.clone().with_value(*value);
            }
            AtomsMessage::RangeLive(lower, upper) | AtomsMessage::RangeCommitted(lower, upper) => {
                self.range = range_model(*lower, *upper);
            }
            AtomsMessage::DatePicked(selection) => {
                self.date = self.date.clone().with_selection(selection.clone());
            }
            AtomsMessage::DateRangePicked(selection) => {
                self.date_range = self.date_range.clone().with_selection(selection.clone());
            }
            AtomsMessage::DateFlyoutToggled(open) => {
                self.date = self.date.clone().with_open(*open);
                self.date_range = self.date_range.clone().with_open(*open);
            }
            AtomsMessage::TimePicked(value) => {
                self.time.close_flyout();
                self.time.select_time(*value);
            }
            AtomsMessage::TimeFlyoutToggled(open) => {
                if *open {
                    self.time.open_flyout();
                } else {
                    self.time.close_flyout();
                }
            }
            AtomsMessage::ColorPicked(value) => self.color.set_rgba(*value),
            AtomsMessage::ColorFlyoutToggled(open) => {
                if *open {
                    self.color.open_flyout();
                } else {
                    self.color.close_flyout();
                }
            }
            AtomsMessage::SplitRowChosen(_)
            | AtomsMessage::LinkOpened(_)
            | AtomsMessage::Note(_) => {}
        }
    }

    /// A human-readable description of `message` for the status bar.
    #[must_use]
    pub fn describe(message: &AtomsMessage) -> String {
        match message {
            AtomsMessage::Checkbox(index, state) => {
                format!("checkbox {index} → {state:?}")
            }
            AtomsMessage::RadioSelected(id) => format!("radio → {id}"),
            AtomsMessage::SwitchToggled(on) => {
                format!("switch → {}", if *on { "on" } else { "off" })
            }
            AtomsMessage::SegmentedChanged(set) => {
                format!(
                    "segmented → [{}]",
                    set.iter().cloned().collect::<Vec<_>>().join(", ")
                )
            }
            AtomsMessage::SplitMenuToggled(open) => {
                format!("split menu → {}", if *open { "open" } else { "closed" })
            }
            AtomsMessage::SplitRowChosen(id) => format!("split action → {id}"),
            AtomsMessage::LinkOpened(uri) => format!("link → {uri}"),
            AtomsMessage::TextTyped(value) => format!("text → {value}"),
            AtomsMessage::TextInputTier(tier, value) => {
                format!("text tier {tier} → {value}")
            }
            // The secret is never echoed — only its length is reported.
            AtomsMessage::PasswordTyped(value) => {
                format!("password → {} chars", value.chars().count())
            }
            AtomsMessage::RevealToggled(revealed) => {
                format!("reveal → {}", if *revealed { "shown" } else { "masked" })
            }
            AtomsMessage::ComboToggled(open) => {
                format!("combo flyout → {}", if *open { "open" } else { "closed" })
            }
            AtomsMessage::ComboChosen(id) => format!("combo → {id}"),
            AtomsMessage::SearchChanged(query) => format!("search → {query}"),
            AtomsMessage::SearchSubmitted(query) => format!("search submitted → {query}"),
            AtomsMessage::SearchCleared => String::from("search cleared"),
            AtomsMessage::RatingPicked(value) => match value {
                Some(v) => format!("rating → {v}"),
                None => String::from("rating → cleared"),
            },
            AtomsMessage::RangeLive(lower, upper) => {
                format!("range live → {lower}…{upper}")
            }
            AtomsMessage::RangeCommitted(lower, upper) => {
                format!("range → {lower}…{upper}")
            }
            AtomsMessage::DatePicked(selection) => {
                format!("date → {}", selection.format_display())
            }
            AtomsMessage::DateRangePicked(selection) => {
                format!("date range → {}", selection.format_display())
            }
            AtomsMessage::DateFlyoutToggled(open) => {
                format!("date flyout → {}", if *open { "open" } else { "closed" })
            }
            AtomsMessage::TimePicked(value) => {
                format!("time → {}", value.format_display(true, false))
            }
            AtomsMessage::TimeFlyoutToggled(open) => {
                format!("time flyout → {}", if *open { "open" } else { "closed" })
            }
            AtomsMessage::ColorPicked(value) => format!("colour → {}", value.to_hex_rgb()),
            AtomsMessage::ColorFlyoutToggled(open) => {
                format!("colour flyout → {}", if *open { "open" } else { "closed" })
            }
            AtomsMessage::Note(text) => text.clone(),
        }
    }
}

/// Builds the radio group model for a selected id.
fn radio_model(selected: impl Into<String>) -> RadioGroupModel<String> {
    let options = data::RADIO_OPTIONS
        .iter()
        .map(|(id, label)| RadioOption::new((*id).to_string(), *label))
        .collect();
    RadioGroupModel::new(options)
        .with_selected(selected.into())
        .with_orientation(RadioOrientation::HorizontalWrap)
}

/// Builds the segmented strip model for a selected id.
fn segmented_model(selected: &str) -> SegmentedModel<String> {
    let segments = data::SEGMENT_ITEMS
        .iter()
        .map(|(id, label)| SegmentItem::new((*id).to_string(), *label))
        .collect();
    SegmentedModel::new(segments).with_selected(selected.to_string())
}

/// Builds the split button model with the demo's flyout rows.
fn split_model() -> SplitButtonModel<String> {
    let rows = data::SPLIT_MENU_ROWS
        .iter()
        .map(|(id, label)| SplitMenuRow::new((*id).to_string(), *label))
        .collect();
    SplitButtonModel::primary(data::SPLIT_PRIMARY_LABEL, rows).with_menu_id("demo.atoms.split")
}

/// A labelled, unambiguously-not-icon-only button label box.
fn button_label(text: &'static str) -> Column<'static, Message> {
    Column::new()
        .width(Length::Fixed(LABEL_WIDTH))
        .align_x(Alignment::Center)
        .push(rung(text, typography::BODY, Palette::dark().text_strong))
}

/// Routes a controlled field's tier into the demo's message channel.
///
/// The focus edge is dropped — the demo has nothing to do when a field
/// gains or loses the keyboard — and every value tier is reported with its
/// tier name, so the status bar shows *which* of the `UIF-ATOM-002a`
/// tiers fired rather than only the string that came with it.
fn tier_message(tier: TextInputEmission) -> Option<Message> {
    match tier {
        TextInputEmission::FocusChanged(_) => None,
        _ => Some(Message::Atoms(AtomsMessage::TextInputTier(
            tier.tier(),
            tier.value().unwrap_or_default().to_string(),
        ))),
    }
}

/// Builds the atoms pane's live sections, in display order.
#[must_use]
pub fn live_sections(state: &AtomsState) -> Vec<Section<'_>> {
    vec![
        buttons_section(state),
        checkbox_section(state),
        radio_section(state),
        switch_section(state),
        segmented_section(state),
        split_button_section(state),
        hyperlink_section(),
        text_input_section(state),
        password_section(state),
        combo_box_section(state),
        search_box_section(state),
        rating_section(state),
        range_slider_section(state),
        avatar_section(),
        date_picker_section(state),
        time_picker_section(state),
        color_picker_section(state),
        tooltip_section(),
    ]
}

/// Builds the whole atoms pane: live sections then resolver previews.
#[must_use]
pub fn atoms_pane(state: &AtomsState) -> PaneBody<'_> {
    let mut sections = live_sections(state);
    sections.extend(crate::previews::atom_preview_sections());
    PaneBody::new(sections)
}

/// The Buttons section: 4 variants × live / disabled / busy.
#[must_use]
pub fn buttons_section(state: &AtomsState) -> Section<'_> {
    let mut grid = Column::new().spacing(hoarfrost::token::space::XS);

    for (index, (name, variant)) in BUTTON_VARIANTS.iter().enumerate() {
        let live = Button::new(
            *variant,
            button_label("Live"),
            Message::Atoms(AtomsMessage::Note(format!("{name} button pressed"))),
            format!("demo.atoms.button.{name}.live"),
        )
        .with_tooltip(format!("{name} button — live"));
        let disabled = Button::new(
            *variant,
            button_label("Disabled"),
            Message::Atoms(AtomsMessage::Note(format!("{name} disabled"))),
            format!("demo.atoms.button.{name}.disabled"),
        )
        .disabled()
        .with_tooltip(format!("{name} button — disabled"));

        let busy = Button::controlled(
            state.busy_models[index].controlled(),
            button_label("Busy"),
            format!("demo.atoms.button.{name}.busy"),
            |_emission| None,
        )
        .with_tooltip(format!("{name} button — busy"));

        grid = grid.push(
            Row::new()
                .spacing(hoarfrost::token::space::SM)
                .align_y(Alignment::Center)
                .push(crate::section::labeled_column(
                    *name,
                    typography::UI_COMPACT,
                    Palette::dark().text_muted,
                    &label_id(BUTTON_ROW_LABEL_PREFIX, &section_slug(name)),
                    LABEL_COLUMN,
                ))
                .push(live)
                .push(disabled)
                .push(busy),
        );
    }

    crate::section::section("Buttons", BUTTON_WIDGET_COUNT, grid)
}

/// The Checkbox section: off / on / indeterminate / disabled.
#[must_use]
pub fn checkbox_section(state: &AtomsState) -> Section<'_> {
    let mut rows = Column::new().spacing(hoarfrost::token::space::XS);

    for (index, row) in data::CHECKBOX_ROWS.iter().enumerate() {
        let shown = state.checkboxes.get(index).copied().unwrap_or(row.1);
        let box_ = Checkbox::standalone(
            row.0,
            shown,
            format!("demo.atoms.checkbox.{index}"),
            move |next| Some(Message::Atoms(AtomsMessage::Checkbox(index, next))),
        )
        .with_children(row.2)
        .with_palette(Palette::dark());

        rows = rows.push(if row.3 {
            box_.disabled(true)
        } else {
            box_.disabled(false)
        });
    }

    crate::section::section("Checkbox", data::CHECKBOX_ROWS.len(), rows)
}

/// The Radio group section.
#[must_use]
pub fn radio_section(state: &AtomsState) -> Section<'_> {
    let group = RadioGroup::controlled(
        Controlled::new(&state.radio),
        "demo.atoms.radio",
        |commit| {
            Some(Message::Atoms(AtomsMessage::RadioSelected(
                commit.into_value(),
            )))
        },
    )
    .with_palette(Palette::dark());

    crate::section::section("Radio group", 1, Column::new().push(group))
}

/// The Toggle switch section: on / off / busy / disabled.
#[must_use]
pub fn switch_section(state: &AtomsState) -> Section<'_> {
    let live = ToggleSwitch::standalone(
        "Live switch",
        state.switch_on,
        "demo.atoms.switch.live",
        |on| Some(Message::Atoms(AtomsMessage::SwitchToggled(on))),
    )
    .with_palette(Palette::dark());

    let busy = ToggleSwitch::standalone("Busy switch", true, "demo.atoms.switch.busy", |_on| None)
        .busy(true)
        .with_palette(Palette::dark());

    let disabled = ToggleSwitch::standalone(
        "Disabled switch",
        false,
        "demo.atoms.switch.disabled",
        |_on| None,
    )
    .disabled(true)
    .with_palette(Palette::dark());

    let rows = Column::new()
        .spacing(hoarfrost::token::space::XS)
        .push(live)
        .push(busy)
        .push(disabled);

    crate::section::section("Toggle switch", 3, rows)
}

/// The Segmented control section.
#[must_use]
pub fn segmented_section(state: &AtomsState) -> Section<'_> {
    let strip = SegmentedButton::controlled(
        Controlled::new(&state.segmented),
        "demo.atoms.segmented",
        |set| Some(Message::Atoms(AtomsMessage::SegmentedChanged(set))),
    )
    .palette(Palette::dark());

    crate::section::section("Segmented", 1, Column::new().push(strip))
}

/// The Split button section.
#[must_use]
pub fn split_button_section(state: &AtomsState) -> Section<'_> {
    let control = SplitButton::controlled(
        Controlled::new(&state.split),
        "demo.atoms.split.control",
        |emission| match emission {
            SplitButtonEmission::PrimaryActivated(_) => Some(Message::Atoms(AtomsMessage::Note(
                "split primary activated".to_string(),
            ))),
            SplitButtonEmission::MenuToggled(open) => {
                Some(Message::Atoms(AtomsMessage::SplitMenuToggled(open)))
            }
            SplitButtonEmission::MenuSelected(id) => {
                Some(Message::Atoms(AtomsMessage::SplitRowChosen(id)))
            }
            SplitButtonEmission::PrimaryCancelled(_) | SplitButtonEmission::StopRequested(_) => {
                None
            }
        },
    )
    .palette(Palette::dark());

    crate::section::section("Split button", 1, Column::new().push(control))
}

/// The Hyperlink section: internal, external and disabled.
#[must_use]
pub fn hyperlink_section() -> Section<'static> {
    let open = |uri: &'static str| {
        move |_target: HyperlinkTarget| {
            Some(Message::Atoms(AtomsMessage::LinkOpened(uri.to_string())))
        }
    };

    let internal = Hyperlink::standalone(
        "Internal link",
        "hoarfrost://atoms",
        "demo.atoms.link.internal",
        open("hoarfrost://atoms"),
    )
    .palette(Palette::dark());

    let external = Hyperlink::standalone(
        "External link",
        "https://example.com/",
        "demo.atoms.link.external",
        open("https://example.com/"),
    )
    .external(true)
    .palette(Palette::dark());

    let disabled = Hyperlink::standalone(
        "Disabled link",
        "hoarfrost://nowhere",
        "demo.atoms.link.disabled",
        open("hoarfrost://nowhere"),
    )
    .enabled(false)
    .palette(Palette::dark());

    let rows = Column::new()
        .spacing(hoarfrost::token::space::XS)
        .push(internal)
        .push(external)
        .push(disabled);

    crate::section::section("Hyperlink", 3, rows)
}

/// The Text input section (`UIF-ATOM-002`, `-002a`).
///
/// Three fields on the legacy `on_input` path — live, read-only and
/// disabled — plus two bound to a [`TextInputModel`], whose edits leave
/// as tiers rather than as a raw string. The invalid field is one of the
/// bound pair because §3.2's Invalid row is a *reported* state: the host's
/// verdict travels in the model's validation message, and its
/// [`EditStance::Touched`] is what stops 'punish late' from withholding
/// the error that is already on screen.
///
/// **The 12px this section appears to inset is not the demo's.** The
/// fields record at `x = 28` while the caption sits at `x = 16`, which
/// reads as a misaligned section beside every other flush one. It is
/// not: the probe records the *inner editable text area*, not the
/// field's frame, because `TextInput::layout` moves the iced editor to
/// `text_area_bounds(outer, kind)`. The frame itself starts at `x = 16`,
/// flush with the caption — `28 = card padding 16 + HORIZONTAL_PADDING
/// 12`, and the width closes the same way (`663 - 12 - 44 = 607`). See
/// the `What a text_input record actually bounds` section of the
/// test-only `alignment_audit` module, and the pinning test
/// `text_field_frames_are_flush_with_their_section_caption`.
#[must_use]
pub fn text_input_section(state: &AtomsState) -> Section<'_> {
    let live = TextInput::new(
        InputKind::TextEntry,
        data::TEXT_PLACEHOLDER,
        state.text.clone(),
        |value| Message::Atoms(AtomsMessage::TextTyped(value)),
        "demo.atoms.text.live",
    )
    .palette(Palette::dark());

    let read_only = TextInput::new(
        InputKind::TextEntry,
        "",
        data::TEXT_READONLY_SAMPLE,
        |value| {
            Message::Atoms(AtomsMessage::Note(format!(
                "read-only field refused {value:?}"
            )))
        },
        "demo.atoms.text.readonly",
    )
    .read_only()
    .palette(Palette::dark());

    let disabled = TextInput::new(
        InputKind::TextEntry,
        "",
        "disabled field",
        |value| Message::Atoms(AtomsMessage::Note(format!("disabled field {value:?}"))),
        "demo.atoms.text.disabled",
    )
    .disabled()
    .palette(Palette::dark());

    let invalid = TextInput::controlled(
        InputKind::TextEntry,
        state.text_invalid_model.controlled(),
        "demo.atoms.text.invalid",
        tier_message,
    )
    .palette(Palette::dark());

    let controlled = TextInput::controlled(
        InputKind::TextEntry,
        state.text_model.controlled(),
        "demo.atoms.text.controlled",
        tier_message,
    )
    .palette(Palette::dark());

    let rows = Column::new()
        .spacing(hoarfrost::token::space::XS)
        .push(live)
        .push(read_only)
        .push(disabled)
        .push(invalid)
        .push(controlled);

    crate::section::section("Text input", 5, rows)
}

/// The Password field section (`UIF-ATOM-002c`).
///
/// The reveal toggle lives in the atom's reserved trailing slot; the
/// reveal *policy* — whether the value is masked this frame — is the
/// host's, held in [`AtomsState::revealed`].
#[must_use]
pub fn password_section(state: &AtomsState) -> Section<'_> {
    let live = TextInput::new(
        InputKind::Password,
        data::PASSWORD_PLACEHOLDER,
        state.password.clone(),
        |value| Message::Atoms(AtomsMessage::PasswordTyped(value)),
        "demo.atoms.password.live",
    )
    .masked(!state.revealed)
    .on_reveal(Message::Atoms(AtomsMessage::RevealToggled(!state.revealed)))
    .palette(Palette::dark());

    let disabled = TextInput::new(
        InputKind::Password,
        "",
        "hunter2",
        |value| Message::Atoms(AtomsMessage::Note(format!("disabled password {value:?}"))),
        "demo.atoms.password.disabled",
    )
    .disabled()
    .palette(Palette::dark());

    let rows = Column::new()
        .spacing(hoarfrost::token::space::XS)
        .push(live)
        .push(disabled);

    crate::section::section("Password", 2, rows)
}

/// The Combo box section (`UIF-ATOM-004`).
///
/// The flyout is part of the widget's own element tree, so the open/closed
/// flag lives in the model and the demo drives it through
/// [`AtomsMessage::ComboToggled`].
#[must_use]
pub fn combo_box_section(state: &AtomsState) -> Section<'_> {
    let live = combo_box(&state.combo)
        .with_width(Length::Fill)
        .with_palette(Palette::dark())
        .on_toggle_open(Message::Atoms(AtomsMessage::ComboToggled(
            !state.combo.is_open(),
        )))
        .on_select(|id| Message::Atoms(AtomsMessage::ComboChosen(id)));

    let invalid = combo_box(&state.combo_invalid)
        .with_width(Length::Fill)
        .with_palette(Palette::dark());

    let disabled = combo_box(&state.combo_disabled)
        .with_width(Length::Fill)
        .with_palette(Palette::dark());

    let rows = Column::new()
        .spacing(hoarfrost::token::space::XS)
        .push(live)
        .push(invalid)
        .push(disabled);

    crate::section::section("Combo box", 3, rows)
}

/// The Search box section (`UIF-PANE-003`).
///
/// One live field: the trailing glyph is `×` while the query is non-empty
/// and falls back to `🔍` once it is cleared, which is the affordance the
/// fixture is here to show.
#[must_use]
pub fn search_box_section(state: &AtomsState) -> Section<'_> {
    let search = SearchBox::new(state.search.clone())
        .with_palette(Palette::dark())
        .on_input(|query| Message::Atoms(AtomsMessage::SearchChanged(query)))
        .on_submit(Message::Atoms(AtomsMessage::SearchSubmitted(
            state.search.clone(),
        )))
        .on_clear(Message::Atoms(AtomsMessage::SearchCleared));

    crate::section::section("Search box", 1, Column::new().push(search))
}

/// Builds a range slider model around a chosen thumb pair.
///
/// `RangeSliderModel` is builder-only — there is no thumb setter — so a
/// scrub is folded back by rebuilding the model from the fixture's bounds
/// with the new thumbs and the same step and unit.
fn range_model(lower: f64, upper: f64) -> RangeSliderModel {
    let (min, max, _, _) = data::RANGE_BOUNDS;
    RangeSliderModel::new(min, max, lower, upper)
        .with_step(Some(data::RANGE_STEP))
        .with_unit_suffix(data::RANGE_UNIT)
}

/// The Rating section: live, read-only and disabled.
#[must_use]
pub fn rating_section(state: &AtomsState) -> Section<'_> {
    let live = rating_control(state.rating.clone())
        .palette(Palette::dark())
        .on_commit(|commit| Message::Atoms(AtomsMessage::RatingPicked(*commit.value())));

    let read_only =
        rating_control(state.rating.clone().with_read_only(true)).palette(Palette::dark());

    let disabled =
        rating_control(state.rating.clone().with_disabled(true)).palette(Palette::dark());

    let rows = Column::new()
        .spacing(hoarfrost::token::space::XS)
        .push(live)
        .push(read_only)
        .push(disabled);

    crate::section::section("Rating", 3, rows)
}

/// The Range slider section: a live dual-thumb slider and a dead one.
#[must_use]
pub fn range_slider_section(state: &AtomsState) -> Section<'_> {
    let live = range_slider(state.range.clone())
        .with_palette(Palette::dark())
        .with_focus_id("demo.atoms.range.live")
        .on_change(|live| {
            let (lower, upper) = *live.value();
            Some(Message::Atoms(AtomsMessage::RangeLive(lower, upper)))
        })
        .on_commit(|commit| {
            let (lower, upper) = *commit.value();
            Some(Message::Atoms(AtomsMessage::RangeCommitted(lower, upper)))
        });

    let disabled = range_slider(state.range_disabled.clone())
        .with_palette(Palette::dark())
        .with_focus_id("demo.atoms.range.disabled");

    let rows = Column::new()
        .spacing(hoarfrost::token::space::XS)
        .push(live)
        .push(disabled);

    crate::section::section("Range slider", 2, rows)
}

/// The Avatar section: the roster at three sizes with presence marks.
#[must_use]
pub fn avatar_section() -> Section<'static> {
    let sizes = [AvatarSize::Sm, AvatarSize::Md, AvatarSize::Lg];
    let mut row = Row::new()
        .spacing(hoarfrost::token::space::SM)
        .align_y(Alignment::Center);

    for (index, (name, tone)) in data::AVATAR_ROSTER.iter().enumerate() {
        let model = AvatarModel::new(*name)
            .with_presence(*tone)
            .with_size(sizes[index % sizes.len()]);
        row = row.push(avatar(model).palette(Palette::dark()));
    }

    crate::section::section("Avatar", data::AVATAR_ROSTER.len(), Column::new().push(row))
}

/// The Date picker section: a single date and a date range.
///
/// The calendar is anchored to the fixture's fixed "today", never to a
/// clock (PLAN C4).
#[must_use]
pub fn date_picker_section(state: &AtomsState) -> Section<'_> {
    let single = date_picker(state.date.clone())
        .with_today(data::date(data::TODAY))
        .with_palette(Palette::dark())
        .on_open_change(|live| Message::Atoms(AtomsMessage::DateFlyoutToggled(*live.value())))
        .on_commit(|commit| Message::Atoms(AtomsMessage::DatePicked(commit.value().clone())));

    let range = date_range_picker(state.date_range.clone())
        .with_today(data::date(data::TODAY))
        .with_palette(Palette::dark())
        .on_commit(|commit| Message::Atoms(AtomsMessage::DateRangePicked(commit.value().clone())));

    let rows = Column::new()
        .spacing(hoarfrost::token::space::XS)
        .push(single)
        .push(range);

    crate::section::section("Date picker", 2, rows)
}

/// The Time picker section: a live 24-hour picker and a disabled one.
#[must_use]
pub fn time_picker_section(state: &AtomsState) -> Section<'_> {
    let live = time_picker(state.time.clone())
        .palette(Palette::dark())
        .on_open_change(|live| Message::Atoms(AtomsMessage::TimeFlyoutToggled(*live.value())))
        .on_commit(|commit| Message::Atoms(AtomsMessage::TimePicked(*commit.value())));

    let disabled = time_picker(state.time.clone().with_disabled(true)).palette(Palette::dark());

    let rows = Column::new()
        .spacing(hoarfrost::token::space::XS)
        .push(live)
        .push(disabled);

    crate::section::section("Time picker", 2, rows)
}

/// The Colour picker section: the full picker and a bare swatch trigger.
#[must_use]
pub fn color_picker_section(state: &AtomsState) -> Section<'_> {
    let picker = color_picker(state.color.clone())
        .on_open_change(|live| Message::Atoms(AtomsMessage::ColorFlyoutToggled(*live.value())))
        .on_commit(|commit| Message::Atoms(AtomsMessage::ColorPicked(*commit.value())));

    let swatch = color_swatch_button(&state.color)
        .on_press(|| Message::Atoms(AtomsMessage::ColorFlyoutToggled(true)));

    let rows = Column::new()
        .spacing(hoarfrost::token::space::XS)
        .push(picker)
        .push(swatch);

    crate::section::section("Colour picker", 2, rows)
}

/// The Tooltip section (`UIF-ATOM-007`).
///
/// A tooltip is a *wrapper*, not a control, so the section shows two:
/// one with a plain label and one carrying the trailing shortcut pill.
#[must_use]
pub fn tooltip_section() -> Section<'static> {
    let target = |label: &'static str| {
        container(rung(label, typography::BODY, Palette::dark().text_primary))
            .padding(hoarfrost::token::space::SM)
    };

    let plain = tooltip(
        target("Hover for a plain tooltip"),
        TooltipContent::new("A framework tooltip"),
    );

    let with_shortcut = tooltip(
        target("Hover for a shortcut tooltip"),
        TooltipContent::with_shortcut("Opens the palette", "Ctrl+K"),
    );

    let rows = Row::new()
        .spacing(hoarfrost::token::space::MD)
        .align_y(Alignment::Center)
        .push(plain)
        .push(with_shortcut);

    crate::section::section("Tooltip", 2, Column::new().push(rows))
}

#[cfg(test)]
#[path = "pane_atoms_tests.rs"]
mod tests;
