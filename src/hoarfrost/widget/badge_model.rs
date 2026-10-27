//! The declarative `BadgeModel` a host owns and hands to the badge each
//! frame — §3.8's three assertions and the four tones they come in
//! (`UIF-ATOM-008`).
//!
//! A badge is the framework's only atom with **no interaction at all**.
//! §3.8 gives it no hover, no pressed state, no focus, no callback: it is
//! a mark some other thing has earned. That absence is the reason this
//! module exists separately rather than as a flag on a control — there is
//! no controller to hang it on, no gesture slot, no emission tier. The
//! model is data in, data out, and the resolver in [`crate::widget::badge`]
//! is a pure function of it.
//!
//! ## The three assertions
//!
//! §3.8 prints three badges, and they differ in *what they assert*, not
//! in how loud they are:
//!
//! * [`BadgeKind::Dot`] asserts presence. "There is something here." It
//!   carries no quantity and no word, which is why it is the only kind
//!   that can sit on top of another control without pushing anything.
//! * [`BadgeKind::Count`] asserts a quantity. It is the only kind whose
//!   *width* is not known ahead of the value, and the only one with a
//!   ceiling: past `99` the number stops being information and becomes
//!   noise, so it folds to `99+`.
//! * [`BadgeKind::Status`] asserts a judgement. It is the only kind that
//!   carries a word, and §3.8 makes its leading glyph **mandatory** —
//!   the tone alone is never allowed to carry the meaning, because tone
//!   is the one property a colour-blind user may not receive.
//!
//! ## Why the tone is a sum and not a colour
//!
//! [`BadgeTone`] names four *semantic* positions rather than reaching
//! into the raw `status.*` palette entries. The status palette has more
//! entries than §3.8 has badge tones, and it has none of the two neutral
//! positions the badge table needs: a badge that means nothing in
//! particular, and a badge that means "this one is the app's". Binding
//! the badge to the raw palette would have meant inventing spec content
//! to close the gap. Instead the four tones are resolved to palette
//! entries in one place — [`crate::widget::badge::resolve_tone`] — so
//! the mapping is auditable and the model stays free of colour.
//!
//! Unit tests live in the sibling `badge_model_tests.rs` sidecar.

/// What the badge is asserting.
///
/// The three variants are not interchangeable by size or colour: they
/// differ in *kind of claim*. Picking the variant is the whole decision
/// a host makes; the resolver derives everything else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BadgeKind {
    /// Presence, with no quantity and no word — §3.8's `6px` dot.
    ///
    /// The dot is the badge a host reaches for when the fact is that
    /// something exists and nothing more. It is also the only kind that
    /// never changes its own width, which is what lets it be pinned to a
    /// corner of another control without resizing it.
    Dot,

    /// A quantity — §3.8's `18px` pill.
    ///
    /// `0` is a legal value and means *nothing to report*. It is not the
    /// same as [`BadgeKind::Dot`]: a zero count is an answered question,
    /// a dot is an unanswered one. [`BadgeModel::is_empty`] lets a host
    /// skip painting the answered-and-empty case without the resolver
    /// deciding on its own whether zero deserves pixels.
    Count(u32),

    /// A judgement — §3.8's `24px` chip.
    ///
    /// The `label` is `&'static str` on purpose. A badge that carried an
    /// owned `String` would suggest the label is dynamic content, and
    /// dynamic words inside a fixed-height pill is how a layout breaks.
    /// The status a badge reports is a chosen state with a chosen word,
    /// not a stream.
    Status {
        /// The mandatory leading glyph.
        ///
        /// Mandatory in the strict sense: there is no way to construct a
        /// `Status` badge without one, and the resolver paints it before
        /// anything else. A status with no glyph is a bug this type
        /// refuses rather than one it renders.
        icon: StatusIcon,
        /// The word the glyph is qualifying.
        label: &'static str,
    },
}

/// The leading glyph a status chip carries.
///
/// Four glyphs, matching the four tones one-for-one. The pairing is
/// fixed: [`StatusIcon::Success`] always paints with
/// [`BadgeTone::Success`]'s colour when the tone is left to follow the
/// icon, so a host cannot accidentally ship a warning triangle in a
/// success colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusIcon {
    /// `✓` — the thing finished correctly.
    Success,
    /// `⚠` — the thing finished, but not cleanly.
    Warning,
    /// `✕` — the thing did not finish.
    Danger,
    /// `●` — nothing to report either way; the mark is the message.
    Info,
}

impl StatusIcon {
    /// The glyph itself, as §3.8 prints it.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::Success => '✓',
            Self::Warning => '⚠',
            Self::Danger => '✕',
            Self::Info => '●',
        }
    }

    /// The tone this glyph implies.
    ///
    /// A host that overrides the tone away from this pairing is making a
    /// deliberate statement — the resolver allows it, but the gallery
    /// paints the implied tone by default so the default is the one on
    /// the record.
    #[must_use]
    pub const fn implied_tone(self) -> BadgeTone {
        match self {
            Self::Success => BadgeTone::Success,
            Self::Warning => BadgeTone::Warning,
            Self::Danger => BadgeTone::Danger,
            Self::Info => BadgeTone::Accent,
        }
    }
}

/// The tone a badge carries.
///
/// Four semantic positions plus [`BadgeTone::Neutral`], which is not a
/// fifth status but the absence of one: the resting badge that is there
/// for shape rather than meaning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BadgeTone {
    /// No meaning attached — the resting mark.
    #[default]
    Neutral,
    /// The app's own colour — "this is the one you care about".
    Accent,
    /// Finished correctly.
    Success,
    /// Finished, not cleanly.
    Warning,
    /// Did not finish.
    Danger,
}

impl BadgeTone {
    /// Whether this tone is allowed to paint a solid fill behind a
    /// foreground.
    ///
    /// §3.8's count badge names exactly two solid backgrounds —
    /// `accent.fill` and `status.danger.fill` — so exactly the two tones
    /// that own them paint solid. `Neutral`, `Success` and `Warning` take
    /// the section's *subtle* branch (`surface.active` behind
    /// `text.strong`), which keeps a wall of green or amber chips from
    /// becoming a wall of alarm the way a solid red one would.
    ///
    /// The pairing is an inference from the section's two named solids
    /// rather than a rule the text states outright: §3.8 says which
    /// backgrounds exist, and this reads the tone that owns each as the
    /// one entitled to wear it.
    #[must_use]
    pub const fn paints_solid(self) -> bool {
        matches!(self, Self::Accent | Self::Danger)
    }
}

/// The declarative badge a host owns (`UIF-ATOM-008`).
///
/// Construct one with [`BadgeModel::dot`], [`BadgeModel::count`] or
/// [`BadgeModel::status`] and adjust the tone with
/// [`BadgeModel::with_tone`]. The type has no `&mut self` methods: a
/// badge does not change itself, the thing it reports on does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadgeModel {
    kind: BadgeKind,
    tone: BadgeTone,
}

impl BadgeModel {
    /// The `6px` presence dot.
    #[must_use]
    pub fn dot() -> Self {
        Self {
            kind: BadgeKind::Dot,
            tone: BadgeTone::Accent,
        }
    }

    /// The count pill for `n`.
    ///
    /// `n` is the raw count; the ceiling is applied at paint time by
    /// [`format_count`], never by mutating the model. A host that has
    /// already clamped its own number has thrown away information the
    /// resolver needs to decide whether the `+` belongs.
    #[must_use]
    pub fn count(n: u32) -> Self {
        Self {
            kind: BadgeKind::Count(n),
            tone: BadgeTone::Accent,
        }
    }

    /// The status chip for `icon` + `label`, in the tone the icon
    /// implies.
    #[must_use]
    pub fn status(icon: StatusIcon, label: &'static str) -> Self {
        Self {
            kind: BadgeKind::Status { icon, label },
            tone: icon.implied_tone(),
        }
    }

    /// Return a copy carrying `tone` instead of the current one.
    #[must_use]
    pub fn with_tone(mut self, tone: BadgeTone) -> Self {
        self.tone = tone;
        self
    }

    /// What this badge asserts.
    #[must_use]
    pub fn kind(&self) -> &BadgeKind {
        &self.kind
    }

    /// The tone this badge carries.
    #[must_use]
    pub fn tone(&self) -> BadgeTone {
        self.tone
    }

    /// Whether this badge has nothing to show.
    ///
    /// Exactly one case: a count of zero. A host that would rather show
    /// a dot than nothing should construct [`BadgeModel::dot`] itself
    /// rather than rely on the resolver folding one into the other — the
    /// two assertions mean different things and the choice belongs to
    /// the code that knows which.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self.kind, BadgeKind::Count(0))
    }

    /// The text this badge paints, or `None` when it paints no text.
    ///
    /// [`BadgeKind::Dot`] has no text. [`BadgeKind::Count`] has the
    /// folded count. [`BadgeKind::Status`] has its label — the glyph is
    /// *not* included here, because it is painted as a separate run at a
    /// fixed offset rather than as part of the string.
    #[must_use]
    pub fn text(&self) -> Option<String> {
        match &self.kind {
            BadgeKind::Dot => None,
            BadgeKind::Count(n) => Some(format_count(*n)),
            BadgeKind::Status { label, .. } => Some((*label).to_string()),
        }
    }
}

#[cfg(test)]
#[path = "badge_model_tests.rs"]
mod badge_model_tests;

/// The count above which the pill stops printing the real number.
///
/// §3.8 prints `99+`. The threshold is named rather than inlined so the
/// boundary is testable at `99` and `100` without a reader having to
/// work out which one folds.
pub const COUNT_CEILING: u32 = 99;

/// Render a count the way §3.8's pill renders it.
///
/// `0..=99` print as themselves; anything above prints `99+`. The
/// folded form is deliberately not the real number: a badge that said
/// `137` would be read as precise, and it is not — the pill stopped
/// being precise at `99`.
#[must_use]
pub fn format_count(n: u32) -> String {
    if n > COUNT_CEILING {
        format!("{COUNT_CEILING}+")
    } else {
        n.to_string()
    }
}
