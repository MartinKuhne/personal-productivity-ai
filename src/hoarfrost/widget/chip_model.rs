//! The declarative `ChipModel` a host owns and the one event a chip gesture
//! produces — §3.6's data model and callbacks (`UIF-ATOM-006`).
//!
//! A chip is the framework's first atom whose **variant decides which
//! callback exists**. §3.6 prints three variants and three callbacks in the
//! same breath and never says which pair with which, so the pairing is a
//! decision this module has to make and defend rather than transcribe:
//!
//! * [`ChipVariant::Assist`] has no selection to report. Its gesture emits
//!   [`ChipEvent::Activated`].
//! * [`ChipVariant::Filter`] *is* its selection. Its gesture emits
//!   [`ChipEvent::Toggled`] carrying the new value, because a host that
//!   received `Activated` would have to read the old selection back out of
//!   its own model to work out what happened.
//! * [`ChipVariant::Input`] is a token the user made, not a choice they are
//!   making. Its gesture emits [`ChipEvent::Activated`], and its `×`
//!   emits [`ChipEvent::Removed`].
//!
//! One gesture, one event. Firing `Activated` *and* `Toggled` for a filter
//! chip would make every host decide which of the two it trusts, and the
//! two would disagree the moment a host handled both.
//!
//! # The height is a rung on the model, not a property of the variant
//!
//! §3.6 gives the chip two heights — `24px` (`size.bar.status`, "compact
//! inline") and `28px` (`size.control.sm`, "interactive filter bar") —
//! and ties the choice to *where the chip sits*, not to which variant it
//! is. A filter chip in a dense toolbar is still 24px and a status-style
//! chip in a roomy filter bar is still 28px, so the rung is carried as
//! [`ChipHeight`] and every variant may take either rung. Deriving height
//! from the variant would have made one of the spec's two sentences false.
//!
//! # The radius gap in §3.6
//!
//! §3.6's radius sentence assigns `radius.pill` to "status/count pills"
//! and `radius.widget` to "filter/input chips". **`Assist` is named by
//! neither group.** That is a gap in the spec, not in this model, and it
//! is resolved here rather than left to fall through:
//!
//! [`ChipVariant::Assist`] takes [`shape::RADIUS_PILL`](crate::token::shape::RADIUS_PILL).
//! The pill branch is the one for a chip that is *not* a filter and not an
//! input, which is exactly what an assist chip is. The competing reading —
//! that "chips" means every chip and so `Assist` takes `radius.widget` —
//! cannot hold, because §3.8's *status chip* is also called a chip and
//! §3.6's own sentence puts it in the pill branch. The gap is recorded in
//! `spec_coverage` rather than papered over.
//!
//! # Why there is no controller half
//!
//! [`button_controller`](crate::widget::button_controller) exists because
//! a button's command can be in flight, and "in flight" needs an
//! [`ActionPending`](crate::binding::ActionPending) projection, a
//! double-activation gate and three emission tiers. A chip has none of
//! that: all three of its callbacks are synchronous notifications about a
//! fact the *host* is going to change anyway. There is nothing to be busy
//! with, so there is nothing to gate.
//!
//! What the chip does adopt from [`crate::binding`] is
//! [`Controlled`]: the selection is the host's value, handed down each
//! frame and never mutated by the chip itself. [`ChipModel::respond`] is a
//! pure function of `(model, gesture)` that returns the event or nothing,
//! which is what lets every branch of §3.6's callback list be asserted
//! without a window.
//!
//! Unit tests live in the sibling `chip_model_tests.rs` sidecar.

use crate::binding::Controlled;
use crate::token::size;

/// What a chip is, and therefore what pressing it means (§3.6).
///
/// The three variants are not sizes or colours wearing different names:
/// each one has a different *answer* to the question "what changed when
/// the user pressed this", and that answer is what selects the callback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChipVariant {
    /// A quiet action chip with no selection — §3.6's `Assist`.
    ///
    /// Pressing it activates something; there is no state to flip. This is
    /// the only variant §3.6 gives no *Selected* skin for, and the reason
    /// is structural: an assist chip is not reporting whether a filter is
    /// on, so there is nothing for a selected row to describe.
    #[default]
    Assist,

    /// A toggle that wears a chip — §3.6's `Filter { selected: bool }`.
    ///
    /// The only variant with a selected skin (`accent.fill`, `text.strong`,
    /// `1px accent.border`, leading `✓`), and the only one whose callback
    /// carries a value.
    Filter {
        /// Whether the filter is currently applied.
        selected: bool,
    },

    /// A token the user produced — §3.6's `Input { removable: bool }`.
    ///
    /// A removable input chip reserves the trailing `24 × 24` `×` slot;
    /// a non-removable one reserves nothing, which is why removability is
    /// a field rather than a property of the variant being `Input`.
    Input {
        /// Whether the chip may be deleted.
        removable: bool,
    },
}

impl ChipVariant {
    /// Whether this variant can be in the selected polarity.
    ///
    /// Exactly one variant can. [`ChipModel::is_selected`] goes through
    /// this rather than matching inline so the fact "only a filter chip
    /// selects" has one owner.
    #[must_use]
    pub const fn is_selectable(self) -> bool {
        matches!(self, Self::Filter { .. })
    }

    /// Whether this variant currently *is* selected.
    #[must_use]
    pub const fn is_selected(self) -> bool {
        match self {
            Self::Filter { selected } => selected,
            Self::Assist | Self::Input { .. } => false,
        }
    }

    /// Whether a chip of this variant reserves the trailing `×` slot.
    #[must_use]
    pub const fn is_removable(self) -> bool {
        match self {
            Self::Input { removable } => removable,
            Self::Assist | Self::Filter { .. } => false,
        }
    }

    /// The variant with its selection replaced.
    ///
    /// A no-op for the two variants that cannot be selected, which is the
    /// honest behaviour: asking an assist chip to become selected is not
    /// an error the model can fix and not a change it can make, so it
    /// returns what it was given.
    #[must_use]
    pub const fn with_selected(self, selected: bool) -> Self {
        match self {
            Self::Filter { .. } => Self::Filter { selected },
            other => other,
        }
    }

    /// The variant with its removability replaced — a no-op outside
    /// [`Self::Input`].
    #[must_use]
    pub const fn with_removable(self, removable: bool) -> Self {
        match self {
            Self::Input { .. } => Self::Input { removable },
            other => other,
        }
    }
}

/// Which of §3.6's two chip heights a cell is built at.
///
/// A context rung, not a style: §3.6 prices the two heights by *where the
/// chip sits* ("compact inline" versus "interactive filter bar"), so the
/// host picks it and every variant may take either.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChipHeight {
    /// `size.bar.status` (`24px`) — compact inline chips.
    #[default]
    Compact,
    /// `size.control.sm` (`28px`) — chips in an interactive filter bar.
    Bar,
}

impl ChipHeight {
    /// The rung's height in logical pixels.
    #[must_use]
    pub const fn px(self) -> f32 {
        match self {
            Self::Compact => size::BAR_STATUS,
            Self::Bar => size::CONTROL_SM,
        }
    }
}

/// What a chip can put in its leading slot.
///
/// The chip's own slot type rather than
/// [`ButtonIcon`](crate::widget::button_model::ButtonIcon) so that a
/// change to the button's icon vocabulary cannot silently resize a chip.
/// Both occupy [`size::ICON_SM`], and both are named for the thing they
/// answer — "how wide is a button's icon" and "how wide is a chip's".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChipIcon {
    /// A glyph from the icon font, painted through the chip's icon colour.
    Glyph(&'static str),
    /// A reserved, currently empty slot that occupies the same width.
    Reserved,
}

impl ChipIcon {
    /// The width this icon occupies — `size.icon.sm`, the same rung §3.6
    /// gives the selected chip's `✓`.
    #[must_use]
    pub const fn width(self) -> f32 {
        size::ICON_SM
    }
}

/// The declarative chip a host owns (`UIF-ATOM-006`).
///
/// Construct one with [`ChipModel::assist`], [`ChipModel::filter`] or
/// [`ChipModel::input`] and adjust with the `with_*` builders. The type
/// has no `&mut self` methods: a chip does not change itself, the host's
/// reaction to [`ChipEvent`] does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChipModel<Id> {
    id: Id,
    label: String,
    variant: ChipVariant,
    height: ChipHeight,
    leading_icon: Option<ChipIcon>,
    disabled: bool,
}

/// The read-only half: every accessor and every builder works for any `Id`,
/// because none of them needs to copy or compare the host's identifier.
/// Keeping the bound off is what lets a chip with a `Copy`-less key still
/// be resolved and measured.
impl<Id> ChipModel<Id> {
    /// A quiet action chip.
    #[must_use]
    pub fn assist(id: Id, label: impl Into<String>) -> Self {
        Self::new(id, label, ChipVariant::Assist)
    }

    /// A filter chip carrying `selected`.
    #[must_use]
    pub fn filter(id: Id, label: impl Into<String>, selected: bool) -> Self {
        Self::new(id, label, ChipVariant::Filter { selected })
    }

    /// An input token chip carrying `removable`.
    #[must_use]
    pub fn input(id: Id, label: impl Into<String>, removable: bool) -> Self {
        Self::new(id, label, ChipVariant::Input { removable })
    }

    /// A chip of any variant.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>, variant: ChipVariant) -> Self {
        Self {
            id,
            label: label.into(),
            variant,
            height: ChipHeight::Compact,
            leading_icon: None,
            disabled: false,
        }
    }

    /// Return a copy built at `height`.
    #[must_use]
    pub fn with_height(mut self, height: ChipHeight) -> Self {
        self.height = height;
        self
    }

    /// Return a copy carrying a leading icon.
    #[must_use]
    pub fn with_leading(mut self, icon: ChipIcon) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    /// Return a copy that cannot be interacted with.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// The host's identifier for this chip.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The chip's text.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// What this chip is.
    #[must_use]
    pub const fn variant(&self) -> ChipVariant {
        self.variant
    }

    /// The height rung this chip is built at.
    #[must_use]
    pub const fn height(&self) -> ChipHeight {
        self.height
    }

    /// The leading slot, if the host supplied one.
    #[must_use]
    pub const fn leading_icon(&self) -> Option<ChipIcon> {
        self.leading_icon
    }

    /// Whether the chip accepts interaction.
    #[must_use]
    pub const fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Whether this chip is in the selected polarity.
    #[must_use]
    pub fn is_selected(&self) -> bool {
        self.variant.is_selected()
    }

    /// Whether this chip reserves the trailing `×` slot.
    #[must_use]
    pub fn is_removable(&self) -> bool {
        self.variant.is_removable()
    }

    /// The read-only frame view of this chip (`UIF-ATOM-000b`).
    ///
    /// The selection belongs to the host and arrives through this view each
    /// frame; the chip never writes it back.
    #[must_use]
    pub fn controlled(&self) -> Controlled<'_, Self> {
        Controlled::new(self)
    }
}

/// The emission half, which needs to hand the host a copy of its own
/// identifier and so is the only bounded block on this type.
impl<Id: Clone> ChipModel<Id> {
    /// The event one gesture produces, or `None` when it produces none.
    ///
    /// The whole of §3.6's callback contract as one pure function. Three
    /// things can silence a gesture, and each is a different reason:
    ///
    /// * a **disabled** chip emits nothing at all. A dead control that
    ///   still put an event on the wire would be a control that is not
    ///   dead;
    /// * [`ChipGesture::Remove`] on a chip that is **not removable**
    ///   emits nothing, because there is no `×` painted and no
    ///   `Delete`/`Backspace` target to act on. The gesture is not an
    ///   error — a host may route keys uniformly — but it has no meaning;
    /// * [`ChipGesture::Remove`] on a **selected filter** still emits
    ///   nothing for the same reason. Only an `Input { removable }` chip
    ///   can be removed.
    #[must_use]
    pub fn respond(&self, gesture: ChipGesture) -> Option<ChipEvent<Id>> {
        if self.disabled {
            return None;
        }

        match gesture {
            ChipGesture::Activate => Some(match self.variant {
                ChipVariant::Filter { selected } => ChipEvent::Toggled(self.id.clone(), !selected),
                ChipVariant::Assist | ChipVariant::Input { .. } => {
                    ChipEvent::Activated(self.id.clone())
                }
            }),
            ChipGesture::Remove if self.is_removable() => Some(ChipEvent::Removed(self.id.clone())),
            ChipGesture::Remove => None,
        }
    }
}

/// The two gestures a chip recognises (§3.6).
///
/// Two rather than five because §3.6 collapses the input devices itself:
/// "`Space`/`Enter`/click" is one gesture with three routes, and
/// "`Delete`/`Backspace` when focused" is one gesture with two. Keeping
/// the sum at the *meaning* level is what lets the resolver and the model
/// agree without a routing table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChipGesture {
    /// A click on the chip body, or `Space` / `Enter` while it holds focus.
    Activate,
    /// A click on the trailing `×`, or `Delete` / `Backspace` while the
    /// chip holds focus.
    Remove,
}

/// The one event a chip gesture produced (§3.6).
///
/// A sum rather than three loose callbacks so a host can match on what
/// happened and an unmatched arm is a compile miss rather than a silently
/// unwired closure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChipEvent<Id> {
    /// An `Assist` or `Input` chip was pressed.
    Activated(Id),
    /// A `Filter` chip was pressed; the payload is the **new** selection.
    Toggled(Id, bool),
    /// A removable `Input` chip was deleted.
    Removed(Id),
}

#[cfg(test)]
#[path = "chip_model_tests.rs"]
mod chip_model_tests;
