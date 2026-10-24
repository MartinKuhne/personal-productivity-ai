//! The shared vocabulary behind both halves of `UIF-COMP-002`.
//!
//! §3B.2 specifies the inline banner and the transient toast as **one**
//! requirement carrying **one** data model — `NotificationModel<ActionId>`
//! — and two different lifetimes. The banner is docked, occupies layout,
//! and stays until someone dismisses it; the toast floats in Band 1 and
//! mostly dismisses itself. What is *the same* in both lives here, so the
//! two halves cannot drift apart on the points where the spec says they
//! agree: the severity ladder, the severity→colour mapping, and which
//! severities are allowed to time out.
//!
//! # The severity ladder is a closed set
//!
//! [`Severity::ALL`] is the enumeration the gallery walks and the tests
//! loop over. A fifth level cannot arrive quietly: it would need a colour
//! in [`Severity::accent`] and a timer rule in
//! [`Severity::may_auto_dismiss`] in the same commit as its variant, and
//! both of those are asserted exhaustively in the sidecar.
//!
//! # `Warning` and `Danger` cannot be talked into a timer
//!
//! §3B.2's rule is a `MUST NEVER`, and a `MUST NEVER` that holds only
//! while the host remembers to pass `None` is not a `MUST NEVER` — it is
//! a convention. [`effective_auto_dismiss_ms`] therefore *overrides*
//! whatever the model carries rather than trusting it: ask for
//! `Some(5_000)` on a `Danger` notification and the answer is `None`.
//! The refusal is a function the host calls, not a silent surprise — the
//! returned value is what the toast's timer is built from, so there is no
//! second path by which the refused duration could still be honoured.
//!
//! # Colour is resolved, never assumed
//!
//! [`Severity::accent`] hands back the token §3B.2 names for each level.
//! What those colours *measure* is recomputed from raw sRGB by
//! [`crate::token::contrast`] in the sidecar and in the gallery rather
//! than copied from the spec's printed column, because the printed column
//! is the thing most likely to be wrong and is not what ships.
//!
//! Unit tests live in the sibling `notification_tests.rs` sidecar.

use crate::token::color::Palette;
use crate::token::time;
use iced::Color;

/// §3B.2's default auto-dismiss window for the severities that may have
/// one — `5000ms`.
///
/// Read through
/// [`time.toast.auto_dismiss`](crate::token::time::TOAST_AUTO_DISMISS),
/// the `UIF-TOK-007` rung §2.6 binds to this exact countdown, including
/// the two conditions attached to it: paused and banked while the toast
/// is hovered or focused, and forced to `None` for `Warning` and
/// `Danger`. The literal lives only under `token/`; `token::time`'s
/// sidecar asserts the two stay equal, and asserts the rung never grows
/// past `u32::MAX` so this narrowing cast cannot truncate silently.
///
/// A `u32` rather than an `f32` because it is a duration, not a
/// measurement: it never becomes a rectangle, so Rule B's visual-scalar
/// rule does not reach it. The width is kept because the whole
/// auto-dismiss API — [`effective_auto_dismiss_ms`],
/// [`NotificationModel::auto_dismiss_ms`] — is cut in `u32`; widening
/// it would be a public-API change this increment has no reason to make.
pub const DEFAULT_AUTO_DISMISS_MS: u32 = time::TOAST_AUTO_DISMISS.ms as u32;

/// How serious a notification is, and therefore how it looks and how long
/// it is allowed to stay (§3B.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Severity {
    /// Neutral news. `accent.text`. May auto-dismiss.
    #[default]
    Info,
    /// Something finished well. `status.success`. May auto-dismiss.
    Success,
    /// Something the user should see and decide about. `status.warning`.
    /// Must stay until acknowledged.
    Warning,
    /// Something failed. `status.danger.text`. Must stay until
    /// acknowledged.
    Danger,
}

impl Severity {
    /// Every severity, in spec order.
    pub const ALL: [Self; 4] = [Self::Info, Self::Success, Self::Warning, Self::Danger];

    /// The lowercase slug used in cell ids and manifests.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Danger => "danger",
        }
    }

    /// The leading glyph §3B.2 puts in front of the title.
    ///
    /// A glyph rather than a colour-only cue is what makes the severity
    /// readable without colour perception (`UIF-PRIN-005`); the colour
    /// carries the same information twice rather than carrying it alone.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Info => "ⓘ",
            Self::Success => "✓",
            Self::Warning => "⚠",
            Self::Danger => "✕",
        }
    }

    /// The severity's accent colour — the stripe, the icon, and the
    /// severity-tinted border.
    ///
    /// §3B.2 names the four tokens explicitly, including the asymmetry
    /// worth noticing: `Danger` reads in `status.danger.text`, **not**
    /// `status.danger.fill`. The fill is a button background and is far
    /// too dark to read a title against; the `.text` token is the one
    /// that clears the text floor on a raised surface.
    #[must_use]
    pub fn accent(self, palette: &Palette) -> Color {
        match self {
            Self::Info => palette.accent_text,
            Self::Success => palette.status_success,
            Self::Warning => palette.status_warning,
            Self::Danger => palette.status_danger_text,
        }
    }

    /// Whether this severity is allowed to time out at all (§3B.2).
    ///
    /// `Info` and `Success` **may** auto-dismiss; `Warning` and `Danger`
    /// **must not**. This is the gate, not a default: a timer built for a
    /// severity returning `false` here is a bug the caller cannot reach,
    /// because [`effective_auto_dismiss_ms`] refuses to produce the
    /// duration the timer would be built from.
    #[must_use]
    pub const fn may_auto_dismiss(self) -> bool {
        matches!(self, Self::Info | Self::Success)
    }

    /// The auto-dismiss window this severity is offered by default.
    ///
    /// `Some(5_000)` for the two that may time out, `None` for the two
    /// that must not. A *default*, not a ceiling — the host may pick any
    /// window for `Info`/`Success`, and no window for the others.
    #[must_use]
    pub const fn default_auto_dismiss_ms(self) -> Option<u32> {
        match self {
            Self::Info | Self::Success => Some(DEFAULT_AUTO_DISMISS_MS),
            Self::Warning | Self::Danger => None,
        }
    }
}

/// A notification's stable identity.
///
/// Its own newtype rather than a [`FocusId`](crate::focus::FocusId)
/// because one notification owns several focusable things — an action
/// button and a dismiss button — and the notification's id is the stem
/// they are all derived from, not one of them.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NotificationId(String);

impl NotificationId {
    /// Wrap an identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The identifier as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for NotificationId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for NotificationId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// The action button §3B.2 lets a notification carry.
///
/// §3B.2 writes this as a bare tuple — `Option<(String, ActionId)>` —
/// and it is a struct here instead, because the two fields are not
/// interchangeable and a tuple lets a reader pass them the wrong way
/// round with no help from the compiler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationAction<ActionId> {
    /// The button's label.
    pub label: String,
    /// What the host is told to do when the button is taken.
    pub id: ActionId,
}

impl<ActionId> NotificationAction<ActionId> {
    /// An action from a label and an id.
    #[must_use]
    pub fn new(label: impl Into<String>, id: ActionId) -> Self {
        Self {
            label: label.into(),
            id,
        }
    }
}

/// §3B.2's `NotificationModel<ActionId>`.
///
/// The model is the *host's* declaration of what it wants shown. It is
/// not the truth about how long the notification will stay: that is
/// [`effective_auto_dismiss_ms`], which reads the model and can overrule
/// it. Keeping those separate is what lets a sloppy host be wrong safely.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationModel<ActionId> {
    /// Stable identity.
    pub id: NotificationId,
    /// How serious it is.
    pub severity: Severity,
    /// The bold first line.
    pub title: String,
    /// The explanatory second line.
    pub message: String,
    /// The optional action button.
    pub action: Option<NotificationAction<ActionId>>,
    /// Whether a `[×]` is offered.
    pub dismissible: bool,
    /// What the host *asked* for. Not what it gets — see
    /// [`effective_auto_dismiss_ms`].
    pub auto_dismiss_ms: Option<u32>,
}

impl<ActionId> NotificationModel<ActionId> {
    /// A model with no action, dismissible, and no timer.
    #[must_use]
    pub fn new(
        id: impl Into<NotificationId>,
        severity: Severity,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            severity,
            title: title.into(),
            message: message.into(),
            action: None,
            dismissible: true,
            auto_dismiss_ms: None,
        }
    }

    /// Attach an action button.
    #[must_use]
    pub fn with_action(mut self, label: impl Into<String>, id: ActionId) -> Self {
        self.action = Some(NotificationAction::new(label, id));
        self
    }

    /// Set whether a `[×]` is offered.
    #[must_use]
    pub fn with_dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    /// Ask for an auto-dismiss window.
    ///
    /// A request, not a decision. For a `Warning` or `Danger` this field
    /// is read and then refused.
    #[must_use]
    pub fn with_auto_dismiss(mut self, ms: u32) -> Self {
        self.auto_dismiss_ms = Some(ms);
        self
    }

    /// The window this model will actually get. See
    /// [`effective_auto_dismiss_ms`].
    #[must_use]
    pub fn effective_auto_dismiss_ms(&self) -> Option<u32> {
        effective_auto_dismiss_ms(self.severity, self.auto_dismiss_ms)
    }
}

/// The window a notification actually gets, given what it asked for.
///
/// **The refusal is the point of this function.** For
/// [`Warning`](Severity::Warning) and [`Danger`](Severity::Danger) it
/// returns `None` whatever was requested, because §3B.2 says those
/// severities `MUST NEVER` auto-dismiss. Returning the request unchanged
/// for a severity that forbids a timer would make the rule the host's
/// job; returning `None` makes it the framework's.
#[must_use]
pub fn effective_auto_dismiss_ms(severity: Severity, requested_ms: Option<u32>) -> Option<u32> {
    if severity.may_auto_dismiss() {
        requested_ms
    } else {
        None
    }
}

/// Why a notification went away.
///
/// The taxonomy is part of the contract rather than decoration: the exits
/// are not equivalent, and a host that logs them identically cannot tell
/// "the user acknowledged this" from "this timed out while nobody was
/// looking".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DismissReason {
    /// The user pressed the `[×]`.
    User,
    /// The user took the action button. The notification did its job.
    Action,
    /// The countdown elapsed. Only reachable for a severity where
    /// [`Severity::may_auto_dismiss`] was true.
    Timeout,
    /// The max-visible policy pushed it off the stack to make room.
    Overflow,
    /// `Escape` unwound it while focus was inside the stack.
    Escape,
    /// The host withdrew it.
    Host,
}

impl DismissReason {
    /// Whether this exit means the user actually saw and acted on the
    /// notification.
    ///
    /// `Timeout` and `Overflow` are deliberately **not** acknowledgements:
    /// a notification that vanished on a timer or to make room for a
    /// newer one was never confirmed, and a host that treats it as
    /// confirmed is claiming the user read something they may have been
    /// away from the screen for.
    #[must_use]
    pub const fn acknowledged(self) -> bool {
        matches!(self, Self::User | Self::Action | Self::Escape)
    }
}

#[cfg(test)]
#[path = "notification_tests.rs"]
mod notification_tests;
