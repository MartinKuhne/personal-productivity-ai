//! The framework's reserved derived-sub-part namespace, and the demo's
//! guard against squatting in it.
//!
//! # The namespace, exactly
//!
//! `hoarfrost::identity` publishes the text a widget draws for itself
//! under `<widget-id>.<role>`. The roles are
//! [`ROLES`](hoarfrost::identity::ROLES) — `title`, `subtitle`, `header`,
//! `hint` and `label` — and [`is_role`](hoarfrost::identity::is_role) also
//! admits the **indexed** form `<role>-<digits>` (`label-0`, `hint-11`)
//! for a widget that renders several parts of one kind.
//! [`is_sub_part_name`](hoarfrost::identity::is_sub_part_name) is the
//! name-level test: a name is inside the namespace when its last
//! `.`-separated segment is a role **and** something precedes the dot.
//!
//! So the reserved set is, precisely:
//!
//! ```text
//! <anything>.title      <anything>.subtitle   <anything>.header
//! <anything>.hint       <anything>.label
//! <anything>.<role>-<one or more ASCII digits>
//! ```
//!
//! and nothing else. `caption`, `row`, `live`, `busy`, `lower`, `upper`
//! and `demo-label` are outside it.
//!
//! # Why the demo has to stay out
//!
//! A derived id *extends* its widget's name, so every consumer that
//! parses ids reads `x.y.label` as "the caption of the widget `x.y`".
//! A demo container that merely happens to end in `.label` is
//! indistinguishable from that: `demo.atoms.buttons.danger.label` read
//! exactly like a framework sub-part of a widget called
//! `demo.atoms.buttons.danger`, which does not exist. A host prefixing
//! `demo.atoms.buttons.danger` would find a "part" with no owner, and a
//! host grouping parts under their widget would invent a widget.
//!
//! `identity.rs` only *documents* this ("A widget whose own name ends in a
//! role word is read as a sub-part of whatever precedes it. The convention
//! owns that risk"). Documentation is not a gate. This module makes it one.
//!
//! # The invariant
//!
//! A record inside the reserved namespace must be a **`text`** candidate —
//! the kind [`publish_text`](hoarfrost::identity::publish_text) produces
//! from inside the widget that drew the glyphs. Any other kind wearing a
//! reserved name is a box somebody put there from outside, which is the
//! collision.
//!
//! Deriving the check from the live probe stream rather than a frozen list
//! is what keeps it honest as the framework grows: a caption that starts
//! appearing because the framework began publishing one arrives as `text`
//! and passes automatically, while a *newly reserved* name the demo is
//! squatting on shows up the moment it is rendered.
//!
//! Unit tests live in the sibling `id_namespace_tests.rs` sidecar.

use hoarfrost::identity::{is_sub_part_name, split_sub_part_name};

use crate::layout_probe::Entry;

/// The probe kind that carries a framework-published sub-part caption.
///
/// `publish_text` calls `operation.text(Some(&id), bounds, content)`, and
/// `iced_selector` forwards that as `Candidate::Text`, which
/// [`crate::layout_probe`] records under this kind. Nothing else in the
/// operation tree can produce a named `text` record.
const PUBLISHED_KIND: &str = "text";

/// One record found occupying the reserved namespace without owning it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Squatter {
    /// The offending id, exactly as the probe recorded it.
    pub id: String,
    /// The probe kind that carried it — the thing that says who put it there.
    pub kind: &'static str,
    /// The widget the id claims to be a part of.
    pub base: String,
    /// The reserved role the id claims to name.
    pub role: String,
}

impl Squatter {
    /// A human-readable statement of what was found and why it is reserved.
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "`{}` [{kind}] sits in the reserved derived-sub-part namespace: it reads \
             as the `{role}` part of a widget named `{base}`, but it is a {kind} \
             record, not a caption the widget published itself. Rename it outside the \
             reserved roles (title, subtitle, header, hint, label, and each of those \
             with a `-<digits>` ordinal) — a demo-authored label belongs under \
             `.demo-label`.",
            self.id,
            kind = self.kind,
            role = self.role,
            base = self.base,
        )
    }
}

/// Every record in `entries` squatting in the reserved namespace.
///
/// A record is checked only when [`is_sub_part_name`] puts it inside the
/// namespace; the overwhelming majority of demo ids never reach the body
/// of this function. Inside it, a `text` record is honest — a widget
/// published it — and everything else is reported.
///
/// The base is allowed to be missing from the stream. A disabled checkbox
/// publishes its caption but registers no focusable candidate of its own,
/// so `demo.atoms.checkbox.3.label` exists with no
/// `demo.atoms.checkbox.3` beside it, and that is the framework being
/// incomplete, not the demo squatting.
#[must_use]
pub fn reserved_namespace_squatters(entries: &[Entry]) -> Vec<Squatter> {
    let mut squatters: Vec<Squatter> = entries
        .iter()
        .filter(|entry| is_sub_part_name(&entry.id))
        .filter(|entry| entry.kind != PUBLISHED_KIND)
        .filter_map(|entry| {
            let (base, role) = split_sub_part_name(&entry.id)?;
            Some(Squatter {
                id: entry.id.clone(),
                kind: entry.kind,
                base,
                role,
            })
        })
        .collect();

    squatters.sort_by(|a, b| a.id.cmp(&b.id));
    squatters
}

/// Every record in `entries` that the framework published as a sub-part
/// caption.
///
/// Kept alongside [`reserved_namespace_squatters`] so a test can show the
/// namespace is actually being *read* in a given capture. A guard over an
/// empty stream passes for the wrong reason.
#[must_use]
pub fn published_sub_parts(entries: &[Entry]) -> Vec<&Entry> {
    entries
        .iter()
        .filter(|entry| is_sub_part_name(&entry.id) && entry.kind == PUBLISHED_KIND)
        .collect()
}

#[cfg(test)]
#[path = "id_namespace_tests.rs"]
mod tests;
