//! Typography scale: size, line height, weight, tracking and family
//! (`UIF-TOK-004`).
//!
//! Unlike the scalar scales beside it, a rung of the type scale carries
//! several properties at once, so a rung is a struct rather than a bare
//! `f32`: [`TypeRung`] bundles the size, line height, resting weight,
//! emphasis weight, the set of weights the rung may carry and the
//! tracking for one rung, and the six `pub const` rungs below are the
//! only ones the interface may render with.
//!
//! # Three weight notions, one authority
//!
//! * [`TypeRung::weight`] is the **resting** weight — what text on the
//!   rung uses when nothing is emphasising it.
//! * [`TypeRung::emphasis_weight`] is what that same text switches to
//!   when emphasised.
//! * [`TypeRung::allowed_weights`] is the **permitted set** — every
//!   weight the spec lets the rung carry at all, transcribed from its
//!   `Allowed Weights` column.
//!
//! The set is the authority and the pair lives inside it: the resting
//! weight is the set's lightest member, the emphasis weight sits at or
//! above the resting one, and both are members of the set — which is
//! what [`TypeRung::allows`] states. A rung therefore cannot advertise
//! an emphasis its spec forbids, and a caller cannot put a weight on a
//! rung the rung does not permit.
//!
//! The pair is not redundant with the set, because the set is routinely
//! wider than the pair. `type.caption` permits Regular / Medium /
//! SemiBold yet rests at Regular and emphasises to Medium: SemiBold is
//! legal on that rung without being *the* emphasis — it is what a menu
//! section heading wears. Before the set existed that pairing was
//! unrepresentable and call sites composed it by hand (ledger G-12).
//!
//! Where the spec lists a single weight ([`BODY`], [`TITLE_MD`],
//! [`TITLE_LG`]) **both** weight fields hold that same weight: emphasis
//! on such a rung is defined to leave the weight alone, not left
//! undefined. A caller can therefore read
//! [`TypeRung::emphasis_weight`] unconditionally without asking which
//! shape a rung has.
//!
//! # Font families
//!
//! [`Family`] (aliased as [`FontFamily`] for spec parity) carries the spec's two
//! `font.family.*` tokens: [`FAMILY_SANS`] (`font.family.sans`) and
//! [`FAMILY_MONO`] (`font.family.mono`). A caller asks for a family by role —
//! "the mono family" — and gets an `iced::Font` through [`Family::font`],
//! [`TypeRung::font`], or the rung helpers [`TypeRung::mono`],
//! [`TypeRung::sans`], [`TypeRung::mono_default`] and [`TypeRung::sans_default`];
//! iced's own family enum stays behind that mapping.
//!
//! Both tokens resolve to iced's *generic* families, `SansSerif` and
//! `Monospace`, which cosmic-text resolves through the host's system
//! font fallback. No bundled TTF is required for either, and that is
//! also the one thing this module cannot promise:
//!
//! **Glyph availability is not the crate's to guarantee.** iced's
//! `fira-sans` feature bundles exactly one text face —
//! `FiraSans-Regular.ttf`; the only other file it ships is
//! `Iced-Icons.ttf`, an icon font. There is no bundled Medium, SemiBold
//! or Bold face, and no bundled monospace face. Asking the sans family
//! for [`Weight::Bold`], or the mono family for any weight at all, is
//! answered at render time by the operating system: which face actually
//! appears depends on the machine the crate runs on. The token layer can
//! express all four weights on both families correctly, and does — but it
//! cannot make a face exist. What the sidecar verifies is the *mapping*
//! and the permitted-set rule, never the rendered glyph.
//!
//! # Monospace usages are a caller concern
//!
//! §2.4 names six monospace usages: code blocks, inline code, file
//! paths, timestamps, the editor buffer and structured logs. They are
//! deliberately **not** modelled as tokens. A usage is a property of the
//! content being drawn, and this framework does not own content —
//! decision D-2 in `doc/planning/iced-pro-tasks.md` leaves the editor,
//! tree and log views in `fastmd`. A `font.usage.log_entry` token would
//! name something this crate can never resolve, and would put the burden
//! of knowing which widget is a log entry on the layer that is told the
//! least about it. Family selection therefore stays with the caller, who
//! picks [`FAMILY_MONO`] and pairs it with a rung.
//!
//! The rung half of each pairing is still worth recording, and the spec
//! pins only three of the six:
//!
//! | Monospace usage | Rung the spec names |
//! |---|---|
//! | timestamps | [`CAPTION`] (§2.4 `type.caption`) |
//! | log entries | [`UI_COMPACT`] (§2.4 `type.ui_compact`) |
//! | editor buffer | [`BODY`] (§2.4 `type.body`) |
//! | code blocks | *unstated* |
//! | inline code | *unstated* |
//! | file paths | *unstated* |
//!
//! A metadata table in the token layer would be half empty while reading
//! as complete, which is worse than the documented list above: the three
//! unstated usages are a spec gap, and a token that silently picked a
//! rung for them would hide it.
//!
//! [`MIN_SIZE`] is not a rung. It is the 12px floor the whole interface
//! is measured against, kept here because the scale's smallest rung is
//! what carries that rule.
//!
//! Sizes and line heights are logical pixels; tracking is in em.
//!
//! Unit tests live in the sibling `typography_tests.rs` sidecar.

/// A typographic weight used by the `UIF-TOK-004` scale.
///
/// The scale deliberately draws on four weights and no more, which keeps
/// the hierarchy readable without a caller having to reason about
/// numeric weight classes. [`Weight::rank`] supplies the ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weight {
    /// 400 — the resting weight of every rung below the titles.
    Regular,
    /// 500 — the emphasis weight of the two smallest rungs.
    Medium,
    /// 600 — the resting weight of `type.title_sm`.
    SemiBold,
    /// 700 — the emphasis weight of `type.title_sm`, and the resting
    /// weight of both larger titles.
    Bold,
}

impl Weight {
    /// Heaviness rank: [`Weight::Regular`] < [`Weight::Medium`] <
    /// [`Weight::SemiBold`] < [`Weight::Bold`].
    ///
    /// An explicit rank rather than a derived `Ord`, so comparing weights
    /// states a property of the scale instead of relying on the order the
    /// variants happen to be declared in.
    #[must_use]
    pub const fn rank(&self) -> u8 {
        match self {
            Self::Regular => 0,
            Self::Medium => 1,
            Self::SemiBold => 2,
            Self::Bold => 3,
        }
    }

    /// The numeric weight class the spec prints beside the `weight.*`
    /// token names: `400`, `500`, `600`, `700`.
    ///
    /// Kept for callers that must hand a weight to something that speaks
    /// numbers rather than enum variants. Nothing in the scale branches on
    /// it — [`rank`](Self::rank) is the ordering primitive.
    #[must_use]
    pub const fn css(&self) -> u16 {
        match self {
            Self::Regular => 400,
            Self::Medium => 500,
            Self::SemiBold => 600,
            Self::Bold => 700,
        }
    }

    /// The `iced::font::Weight` this token maps to.
    ///
    /// iced spells the 400 class `Normal`, not `Regular`, and its 600
    /// class `Semibold`, not `SemiBold`. This is the only place that
    /// translation is allowed to live: a total mapping between two
    /// closed sets, so no numeric weight class and no iced spelling of
    /// one appears at a call site.
    #[must_use]
    pub const fn iced(&self) -> iced::font::Weight {
        match self {
            Self::Regular => iced::font::Weight::Normal,
            Self::Medium => iced::font::Weight::Medium,
            Self::SemiBold => iced::font::Weight::Semibold,
            Self::Bold => iced::font::Weight::Bold,
        }
    }
}

/// A `font.family.*` token (`UIF-TOK-004`).
///
/// The spec names two families and no more. A caller asks for one by
/// role — "the mono family", not a cosmic-text enum variant — and gets
/// an `iced::Font` through [`font`](Family::font) or
/// [`TypeRung::font`]; iced's own [`iced::font::Family`] stays behind
/// this mapping.
///
/// Both arms resolve to iced's *generic* families and so depend on the
/// host's system font fallback for the actual face. See the module doc
/// for the bundled-font limitation this implies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    /// `font.family.sans` — proportional UI chrome and document prose.
    Sans,
    /// `font.family.mono` — code blocks, inline code, file paths,
    /// timestamps, the editor buffer and structured logs.
    Mono,
}

/// An alias for [`Family`] matching the `FontFamily::Mono` / `FontFamily::Sans`
/// naming in `spec.md` §2.4 (`UIF-TOK-004`).
pub type FontFamily = Family;

/// `font.family.sans` — the proportional family, [`Family::Sans`].
pub const FAMILY_SANS: Family = Family::Sans;

/// `font.family.mono` — the fixed-pitch family, [`Family::Mono`].
pub const FAMILY_MONO: Family = Family::Mono;

impl Family {
    /// The spec's own dotted name for this token.
    #[must_use]
    pub const fn token_name(&self) -> &'static str {
        match self {
            Self::Sans => "font.family.sans",
            Self::Mono => "font.family.mono",
        }
    }

    /// The `iced::font::Family` this token resolves to.
    ///
    /// Both arms are iced's generic families rather than a named face on
    /// purpose. Naming `"Fira Sans"` here would claim a four-weight
    /// family the crate does not bundle — `fira-sans` ships the Regular
    /// face only — so the honest resolution is the generic one, which
    /// cosmic-text answers from whatever the host has.
    #[must_use]
    pub const fn iced(&self) -> iced::font::Family {
        match self {
            Self::Sans => iced::font::Family::SansSerif,
            Self::Mono => iced::font::Family::Monospace,
        }
    }

    /// Whether this token names the fixed-pitch family.
    ///
    /// Derived from the token, not from the resolved iced family, so the
    /// answer is a property of the scale rather than of how iced happens
    /// to spell its variants.
    #[must_use]
    pub const fn is_monospace(&self) -> bool {
        matches!(self, Self::Mono)
    }

    /// A `Font` in this family carrying `weight`.
    ///
    /// Stretch and style are left `Normal`: the scale specifies neither,
    /// and inventing an italic or a condensed face here would be a
    /// decision the spec has not made.
    #[must_use]
    pub const fn font(&self, weight: Weight) -> iced::Font {
        iced::Font {
            family: self.iced(),
            weight: weight.iced(),
            stretch: iced::font::Stretch::Normal,
            style: iced::font::Style::Normal,
        }
    }

    /// This family at the scale's lightest weight, [`Weight::Regular`].
    ///
    /// A convenience for callers that want the family alone. It is *not*
    /// a rung's resting weight — that belongs to the rung, and only a
    /// rung knows it.
    #[must_use]
    pub const fn regular(&self) -> iced::Font {
        self.font(Weight::Regular)
    }
}

/// One rung of the `UIF-TOK-004` type scale.
///
/// A rung is a value, not a knob: the six constants in this module are
/// the scale, and a hand-built [`TypeRung`] outside them is a spec
/// violation in the same way a raw numeric literal in a widget is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeRung {
    /// Font size in logical pixels. No rung may sit below [`MIN_SIZE`].
    pub size: f32,
    /// Line height in logical pixels.
    ///
    /// The spec states this as a pixel value and prints the resulting
    /// multiplier alongside it; read [`TypeRung::line_height_ratio`] for
    /// the multiplier rather than carrying a second copy of it.
    pub line_height: f32,
    /// The rung's resting weight — what text on this rung renders at when
    /// nothing is emphasising it.
    ///
    /// Always the lightest member of
    /// [`allowed_weights`](Self::allowed_weights).
    pub weight: Weight,
    /// The weight text on this rung takes when emphasised.
    ///
    /// Where the spec lists two weights, this is the heavier one. Where
    /// it lists only one, this carries the same weight as
    /// [`TypeRung::weight`]: emphasised text on such a rung is specified
    /// to be no heavier, which is a decision rather than a gap.
    ///
    /// Always a member of [`allowed_weights`](Self::allowed_weights), and
    /// never lighter than [`weight`](Self::weight).
    pub emphasis_weight: Weight,
    /// Every weight this rung may carry — the spec's `Allowed Weights`
    /// column, listed lightest first.
    ///
    /// This is the authority the resting/emphasis pair is checked
    /// against, and it is wider than that pair wherever the spec permits a
    /// weight that is neither. `type.caption` is the case that motivated
    /// it: Regular / Medium / **SemiBold** are all legal on the rung,
    /// but the rung rests at Regular and emphasises to Medium, so
    /// SemiBold is a permitted weight that is *not* the rung's emphasis.
    /// A menu section heading wears it, and does so by token rather than
    /// by composition.
    ///
    /// A `&'static [Weight]` rather than a bitflag set so the column is
    /// transcribed literally from the spec table and stays readable in
    /// the constant that carries it.
    pub allowed_weights: &'static [Weight],
    /// Letter-spacing in **em**, so the spec's `+0.01em` is `0.01` and
    /// its `-0.015em` is `-0.015`.
    ///
    /// Positive tracking opens the glyphs up, negative tracking tightens
    /// them. The scale tightens as it grows: the smallest rung is the
    /// only one opened up, and the largest is the tightest.
    pub tracking: f32,
}

impl TypeRung {
    /// The line-height multiplier the spec prints beside the pixel pair,
    /// derived as `line_height / size` so the two figures can never
    /// quietly disagree.
    ///
    /// The helper is total. A rung with a non-positive
    /// [`size`](Self::size) has no meaningful multiplier, and the raw
    /// division would escape as `inf` or `NaN` — values a layout engine
    /// propagates silently into collapsed or unbounded text — so the
    /// guard returns `0.0` instead. That value is unreachable for any
    /// rung on the scale, where every ratio is at least `1.33×`, so a
    /// caller that cares can tell "no ratio" from a real one. Guarding
    /// rather than panicking keeps this usable on a render path, where a
    /// malformed hand-built rung must not take the frame down with it.
    #[must_use]
    pub const fn line_height_ratio(&self) -> f32 {
        if self.size > 0.0 {
            self.line_height / self.size
        } else {
            0.0
        }
    }

    /// This rung's [`line_height`](Self::line_height) as an
    /// `iced::LineHeight` in absolute pixels.
    ///
    /// **Why this exists, and why no call site may skip it.**
    /// `line_height` is a *pixel* figure: `type.body` is `14/20`, twenty
    /// pixels per line. iced does not read a bare `f32` that way.
    /// `impl From<f32> for LineHeight` yields `LineHeight::Relative(factor)`
    /// (`iced_core-0.14.0/src/text.rs:221-225`), and `Relative(f)` means
    /// *f times the font size* — `to_absolute` returns `Pixels(f * size)`
    /// (same file, line 209). So handing the rung's `20.0` straight to
    /// `.line_height(...)` asks iced for 20 × 14 = **280px per line**,
    /// not 20. Only `From<Pixels>` reaches `LineHeight::Absolute`.
    ///
    /// The overshoot is not cosmetic. Measured in this crate's headless
    /// harness, a single-line `type.ui_compact` widget lays out at
    /// **234px** instead of 18px (13 × 18), and the glyph is drawn at
    /// the vertical centre of that inflated box — 112.5px below the top
    /// of the 32px menu-row slot it was placed in, i.e. ~80px below the
    /// slot's own bottom edge. cosmic_text clips the paragraph to the
    /// buffer height it was given, so the glyph is removed entirely and
    /// the row renders **no text at all**. That is what every one of
    /// this crate's menu rows was doing before this helper existed.
    ///
    /// Routing every call site through here removes the ambiguous
    /// conversion from the crate instead of fixing each site by hand: a
    /// widget author cannot pick the wrong unit by accident, and
    /// [`crate::token_lint`] can require it.
    #[must_use]
    pub const fn iced_line_height(&self) -> iced::advanced::text::LineHeight {
        iced::advanced::text::LineHeight::Absolute(iced::Pixels(self.line_height))
    }

    /// Whether this rung permits `weight` — the `Allowed Weights` column
    /// read as a predicate (`UIF-TOK-004`).
    ///
    /// Compares [`Weight::rank`] rather than calling `PartialEq` so the
    /// predicate stays a `const fn`. That is what lets a call site state
    /// a rung-plus-weight composition as a constant and have the scale
    /// vouch for it, instead of a caller inventing the pairing at runtime
    /// and nothing checking it afterwards.
    #[must_use]
    pub const fn allows(&self, weight: Weight) -> bool {
        let wanted = weight.rank();
        let mut i = 0;
        while i < self.allowed_weights.len() {
            if self.allowed_weights[i].rank() == wanted {
                return true;
            }
            i += 1;
        }
        false
    }

    /// This rung carried on `family` at `weight`, as an `iced::Font`.
    ///
    /// The three-way composition the spec actually describes — a rung, a
    /// family and a weight — resolved in one place instead of being
    /// assembled at the call site from parts. The rung contributes the
    /// permission, the family the pitch, the weight the heaviness.
    ///
    /// Debug builds assert the rung permits the weight, so an illegal
    /// pairing is caught in development against the real constants. A
    /// release build renders what it is handed: this sits on a paint path,
    /// where panicking over a weight would take the frame down with the
    /// violation.
    #[must_use]
    pub const fn font(&self, family: Family, weight: Weight) -> iced::Font {
        debug_assert!(self.allows(weight));
        family.font(weight)
    }

    /// This rung carried on [`Family::Mono`] at `weight`, as an `iced::Font` (`UIF-TOK-004`).
    ///
    /// Delegates to [`font`](Self::font) with [`Family::Mono`], validating
    /// in debug builds that `weight` is permitted by this rung's
    /// [`allowed_weights`](Self::allowed_weights).
    #[must_use]
    pub const fn mono(&self, weight: Weight) -> iced::Font {
        self.font(Family::Mono, weight)
    }

    /// This rung carried on [`Family::Sans`] at `weight`, as an `iced::Font` (`UIF-TOK-004`).
    ///
    /// Delegates to [`font`](Self::font) with [`Family::Sans`], validating
    /// in debug builds that `weight` is permitted by this rung's
    /// [`allowed_weights`](Self::allowed_weights).
    #[must_use]
    pub const fn sans(&self, weight: Weight) -> iced::Font {
        self.font(Family::Sans, weight)
    }

    /// This rung carried on [`Family::Mono`] at its resting
    /// [`weight`](Self::weight), as an `iced::Font` (`UIF-TOK-004`).
    ///
    /// Delegates to [`font`](Self::font) with [`Family::Mono`] and `self.weight`.
    #[must_use]
    pub const fn mono_default(&self) -> iced::Font {
        self.font(Family::Mono, self.weight)
    }

    /// This rung carried on [`Family::Sans`] at its resting
    /// [`weight`](Self::weight), as an `iced::Font` (`UIF-TOK-004`).
    ///
    /// Delegates to [`font`](Self::font) with [`Family::Sans`] and `self.weight`.
    #[must_use]
    pub const fn sans_default(&self) -> iced::Font {
        self.font(Family::Sans, self.weight)
    }
}

/// `type.caption` — 12px on a 16px line (`1.33×`).
///
/// Primary usage: shortcut hints, validation error captions, status
/// chips, menu section headings and timestamps.
///
/// The smallest rung in the scale, and therefore the one that sits
/// exactly on the [`MIN_SIZE`] floor. Its `+0.01em` is the only positive
/// tracking on the scale: at this size the glyphs need opening up rather
/// than tightening.
///
/// Three weights are permitted — Regular, Medium and **SemiBold** — yet
/// the rung rests at Regular and emphasises only to Medium. SemiBold is
/// legal here without being *the* emphasis: it is what a menu section
/// heading wears (§4.8.1), the pairing G-12 could not express while the
/// scale carried only the resting/emphasis pair.
pub const CAPTION: TypeRung = TypeRung {
    size: 12.0,
    line_height: 16.0,
    weight: Weight::Regular,
    emphasis_weight: Weight::Medium,
    allowed_weights: &[Weight::Regular, Weight::Medium, Weight::SemiBold],
    tracking: 0.01,
};

/// `type.ui_compact` — 13px on an 18px line (`1.38×`).
///
/// Primary usage: dense UI lists — directory tree rows, table of
/// contents, tab labels, menu items and table cells.
///
/// The widest permitted set on the scale: all four weights. The rung
/// rests at Regular and emphasises to SemiBold (§2.4), while an active
/// tab label at SemiBold and a movable overlay title at Bold (§4.7.1)
/// both sit inside what the rung allows. Four legal weights, one resting
/// voice, and an emphasis step that is a full weight rather than a nudge
/// — dense lists have to be able to shout without leaving the rung.
pub const UI_COMPACT: TypeRung = TypeRung {
    size: 13.0,
    line_height: 18.0,
    weight: Weight::Regular,
    emphasis_weight: Weight::SemiBold,
    allowed_weights: &[
        Weight::Regular,
        Weight::Medium,
        Weight::SemiBold,
        Weight::Bold,
    ],
    tracking: 0.0,
};

/// `type.body` — 14px on a 20px line (`1.43×`).
///
/// Primary usage: document prose, dialog prompt text, text input values
/// and the inline editor buffer.
///
/// Per §2.4 the rung rests at Regular and emphasises to **Medium**, and
/// permits Regular, Medium and SemiBold. It is not a single-weight rung:
/// a body paragraph that needs a voice of its own can take one without
/// leaving the rung, and `emphasis_font` actually moves the weight.
/// SemiBold is permitted but is not the emphasis target — prose that
/// wants SemiBold asks for it explicitly rather than getting it by
/// emphasising.
pub const BODY: TypeRung = TypeRung {
    size: 14.0,
    line_height: 20.0,
    weight: Weight::Regular,
    emphasis_weight: Weight::Medium,
    allowed_weights: &[Weight::Regular, Weight::Medium, Weight::SemiBold],
    tracking: 0.0,
};

/// `type.title_sm` — 16px on a 24px line (`1.50×`).
///
/// Primary usage: dialog titles, the navbar brand title, movable overlay
/// headers and panel section titles.
///
/// The only rung that rests on [`Weight::SemiBold`] and emphasises to
/// [`Weight::Bold`], and the first rung to track negative. Its permitted
/// set excludes Regular and Medium outright: a title cannot be softened
/// below SemiBold by asking, and [`allows`](TypeRung::allows) says so
/// before a renderer is ever handed the request.
pub const TITLE_SM: TypeRung = TypeRung {
    size: 16.0,
    line_height: 24.0,
    weight: Weight::SemiBold,
    emphasis_weight: Weight::Bold,
    allowed_weights: &[Weight::SemiBold, Weight::Bold],
    tracking: -0.01,
};

/// `type.title_md` — 20px on a 28px line (`1.40×`).
///
/// Primary usage: document H2 headings and the About dialog header.
///
/// Rests at Bold and emphasises to Bold, so it has no emphasis range —
/// but per §2.4 its permitted set is SemiBold **and** Bold. The rung
/// chooses Bold, it does not forbid SemiBold: a heading one degree
/// quieter is legal at this size and [`allows`](TypeRung::allows) says
/// so. Only Regular and Medium are refused outright.
pub const TITLE_MD: TypeRung = TypeRung {
    size: 20.0,
    line_height: 28.0,
    weight: Weight::Bold,
    emphasis_weight: Weight::Bold,
    allowed_weights: &[Weight::SemiBold, Weight::Bold],
    tracking: -0.015,
};

/// `type.title_lg` — 24px on a 32px line (`1.33×`).
///
/// Primary usage: document H1 headings.
///
/// The largest rung and the tightest-tracked one: at `-0.02em` the
/// letters are pulled together far enough that a heading reads as one
/// shape rather than a row of letters. Like [`TITLE_MD`] its permitted
/// set is Bold alone.
pub const TITLE_LG: TypeRung = TypeRung {
    size: 24.0,
    line_height: 32.0,
    weight: Weight::Bold,
    emphasis_weight: Weight::Bold,
    allowed_weights: &[Weight::Bold],
    tracking: -0.02,
};

/// The absolute floor for rendered text: nothing anywhere in the
/// interface may render smaller than `12px` (`UIF-TOK-004`).
///
/// This is a rule token rather than a rung — the same role
/// [`size::HIT_TARGET_MIN`](super::size::HIT_TARGET_MIN) plays for
/// pointer targets. It is checked against a chosen size, never picked
/// as one. [`CAPTION`] sits exactly on it, which is what makes the
/// smallest rung the one that carries the rule.
pub const MIN_SIZE: f32 = 12.0;

#[cfg(test)]
#[path = "typography_tests.rs"]
mod typography_tests;
