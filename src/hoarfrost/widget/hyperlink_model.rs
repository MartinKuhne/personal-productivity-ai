//! The `Hyperlink` data model and its emissions — §3.1d
//! (`UIF-ATOM-001d`).
//!
//! # Activation carries a target, not a message
//!
//! §3.1d's callback is `on_open(String)`. Taken literally that hands the
//! host a bare string and leaves it to guess what was clicked, from which
//! control, and whether the user meant to leave the application.
//! [`HyperlinkModel::activate`] hands back a [`Commit<HyperlinkTarget>`]
//! instead: the URI, and the `external` flag that says whether honouring
//! it means opening something outside this program. The `Commit` wrapper
//! is what makes it an intent that fires once rather than an event that
//! can be re-issued, which is the same tier discipline `UIF-ATOM-001a`
//! settled on for buttons.
//!
//! # The hover preview is a separate tier
//!
//! `on_hover_uri(Option<String>)` fires on every pointer transition,
//! which is far more often than a click and must not be confused with one.
//! It is an `Option` rather than an empty string because the status bar
//! has two genuinely different things to show — a target, and no target —
//! and an empty string would render as a preview of nothing rather than
//! as the absence of a preview.
//!
//! # A dead link emits neither
//!
//! A disabled link produces no activation and no hover preview. The
//! second half matters as much as the first: a link that still shows its
//! target in the status bar while refusing to open it tells the user the
//! control is working and something else is wrong.
//!
//! Unit tests live in the sibling `hyperlink_model_tests.rs` sidecar.

use std::fmt;

use crate::binding::Commit;

/// What a link activation points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HyperlinkTarget {
    /// The target URI, verbatim as the model was given it.
    pub uri: String,
    /// Whether honouring this leaves the application.
    pub external: bool,
}

impl HyperlinkTarget {
    /// A target with the given URI and externality.
    #[must_use]
    pub fn new(uri: impl Into<String>, external: bool) -> Self {
        Self {
            uri: uri.into(),
            external,
        }
    }
}

impl fmt::Display for HyperlinkTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.uri)
    }
}

/// One piece of output a hyperlink produces.
#[derive(Clone, Debug)]
pub enum HyperlinkEmission {
    /// The link was activated — `on_open`, carrying the target rather
    /// than a bare string.
    Opened(Commit<HyperlinkTarget>),
    /// The status-bar preview changed — `on_hover_uri`. `None` clears it.
    HoverUri(Option<String>),
}

impl PartialEq for HyperlinkEmission {
    /// Compares the payload each tier carries.
    ///
    /// Hand-written because [`Commit`] is a linear wrapper that
    /// deliberately does not implement `PartialEq`. Matching the tiers
    /// first means an opening can never compare equal to a preview of the
    /// same URI.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Opened(a), Self::Opened(b)) => a.value() == b.value(),
            (Self::HoverUri(a), Self::HoverUri(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for HyperlinkEmission {}

impl HyperlinkEmission {
    /// The target this emission concerns, for either tier.
    #[must_use]
    pub fn uri(&self) -> Option<&str> {
        match self {
            Self::Opened(commit) => Some(commit.value().uri.as_str()),
            Self::HoverUri(uri) => uri.as_deref(),
        }
    }

    /// Whether this emission came from a click rather than a hover.
    #[must_use]
    pub fn is_open(&self) -> bool {
        matches!(self, Self::Opened(_))
    }
}

/// The state of one hyperlink control (§3.1d, `UIF-ATOM-001d`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HyperlinkModel {
    /// The visible label.
    pub label: String,
    /// The target URI.
    pub uri: String,
    /// Whether opening this leaves the application.
    pub external: bool,
    /// Whether the control is inert.
    pub disabled: bool,
}

impl HyperlinkModel {
    /// A live internal link.
    #[must_use]
    pub fn new(label: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            uri: uri.into(),
            external: false,
            disabled: false,
        }
    }

    /// A link that opens outside the application.
    #[must_use]
    pub fn external(label: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            external: true,
            ..Self::new(label, uri)
        }
    }

    /// Mark the link inert.
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// Set the externality explicitly.
    #[must_use]
    pub const fn with_external(mut self, external: bool) -> Self {
        self.external = external;
        self
    }

    /// Whether the link accepts interaction.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        !self.disabled
    }

    /// The target this model points at.
    #[must_use]
    pub fn target(&self) -> HyperlinkTarget {
        HyperlinkTarget {
            uri: self.uri.clone(),
            external: self.external,
        }
    }

    /// Activate the link (`on_open`).
    ///
    /// `None` for a disabled link: nothing was opened, and there is no
    /// revert to hand back either.
    #[must_use]
    pub fn activate(&self) -> Option<Commit<HyperlinkTarget>> {
        self.enabled().then(|| Commit::new(self.target()))
    }

    /// What the status bar should show while the pointer is on this link.
    ///
    /// `None` when the link is disabled — see the
    /// [module docs](self#the-hover-preview-is-a-separate-tier).
    #[must_use]
    pub fn hover_uri(&self) -> Option<String> {
        self.enabled().then(|| self.uri.clone())
    }

    /// What the status bar should show when the pointer leaves.
    #[must_use]
    pub fn hover_cleared() -> HyperlinkEmission {
        HyperlinkEmission::HoverUri(None)
    }

    /// The emission pair a pointer entering this link produces.
    #[must_use]
    pub fn hover_entered(&self) -> Option<HyperlinkEmission> {
        self.enabled()
            .then(|| HyperlinkEmission::HoverUri(Some(self.uri.clone())))
    }
}

#[cfg(test)]
#[path = "hyperlink_model_tests.rs"]
mod hyperlink_model_tests;
