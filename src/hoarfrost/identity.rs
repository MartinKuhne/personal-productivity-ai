//! Stable sub-part identity for text a hoarfrost widget renders on its own.
//!
//! # The hole this fills
//!
//! `iced_core::widget::Text` cannot be named. The struct
//! (`iced_core-0.14.0/src/widget/text.rs:58`) carries only `fragment`,
//! `format` and `class` — no `Id` field — it exposes no `.id()` or
//! `with_id` builder (in all of `iced_widget` only `container`,
//! `scrollable`, `text_editor` and `text_input` have one), and
//! `Text::operate` reports
//!
//! ```text
//! operation.text(None, layout.bounds(), &self.fragment);
//! ```
//!
//! (`iced_core-0.14.0/src/widget/text.rs:264`) — the identity is thrown
//! away before the operation tree ever sees it. A host that wants to
//! address "the title of this settings row" therefore cannot do it from
//! the outside.
//!
//! # The mechanism
//!
//! [`Operation::text`] takes an `Option<&Id>`
//! (`iced_core-0.14.0/src/widget/operation.rs:62`), and
//! `iced_selector`'s find operation forwards it verbatim into
//! `Candidate::Text { id, .. }`
//! (`iced_selector-0.14.0/src/find.rs:238`). So a **custom** widget —
//! one that owns its `Widget::operate` — can publish its own text with a
//! stable id attached, which is exactly what [`publish_text`] does.
//!
//! This is strictly better than wrapping the text in a padding-free
//! `Container`, which is what a host with no `operate` hook has to do:
//!
//! * `operate` is a read-only traversal. It touches neither `layout` nor
//!   `draw`, so publishing a sub-part **cannot** move a bound by any
//!   amount. Container wrapping changes the node tree and relies on the
//!   wrapper being provably inert.
//! * The published bounds are the ones the widget already computed for
//!   the text, not a wrapper's re-resolution of them.
//!
//! # The convention
//!
//! A sub-part id is `<widget-id>.<role>`, built by [`sub_part_id`]. Roles
//! are the [`role`] constants, so a host can address any documented part
//! of a widget without knowing how the widget is built:
//!
//! ```text
//! demo.composites.setting.0        the row itself
//! demo.composites.setting.0.title     its title line
//! demo.composites.setting.0.subtitle  its description line
//! ```
//!
//! Where a widget renders more than one part with the same role — a radio
//! group's option captions — the role carries an ordinal
//! ([`indexed_role`]), so the parts stay distinct:
//! `demo.atoms.radio.label-0`, `demo.atoms.radio.label-1`.
//!
//! Ids are *derived*, never invented. A widget with no addressable id of
//! its own publishes nothing: an anonymous widget handing out a name
//! would let a host bind to something that changes when the tree is
//! reordered, which is worse than no name at all.
//!
//! Unit tests live in the sibling `identity_tests.rs` sidecar.

use iced::Rectangle;
use iced::advanced::widget::{Id, Operation};

/// Separator between a widget's own id and the role of one of its parts.
pub const ROLE_SEPARATOR: char = '.';

/// Separator between a role and the ordinal of the part that carries it.
///
/// Distinct from [`ROLE_SEPARATOR`] so [`split_sub_part_id`] can keep the
/// widget's own dotted id intact: `a.b.c.label-2` splits into base `a.b.c`
/// and role `label-2`, not into `a.b.c.label` and `2`.
pub const INDEX_SEPARATOR: char = '-';

/// Roles a hoarfrost widget may publish for the text it renders.
///
/// A host matches on these suffixes rather than on a widget's internals,
/// so a new widget that adopts the convention is addressable without the
/// host changing.
pub mod role {
    /// A widget's primary name — a settings-row title, a card heading.
    pub const TITLE: &str = "title";
    /// The explanatory line beneath a title.
    pub const SUBTITLE: &str = "subtitle";
    /// A field's header, above the control.
    pub const HEADER: &str = "header";
    /// A field's hint or helper text.
    pub const HINT: &str = "hint";
    /// The caption beside a control — a checkbox or toggle label.
    pub const LABEL: &str = "label";
}

/// The stable name carried by an [`Id`], if it carries one.
///
/// `Id` exposes no accessor for its name — only `Debug` — so the variant
/// has to be read off the debug rendering. `Id` is a tuple struct over its
/// private `Internal`, so `Id::new("x")` and `Id::from(String)` both
/// render as `Id(Custom("x"))`; `Id::unique()` renders as
/// `Id(Unique(n))` over a process-global counter that changes between
/// runs, so it has no stable name and yields `None`.
#[must_use]
pub fn custom_name(id: &Id) -> Option<String> {
    let rendered = format!("{id:?}");
    if !rendered.contains("Custom") {
        return None;
    }
    let start = <str>::find(&rendered, '"')?;
    let end = <str>::rfind(&rendered, '"')?;
    if end <= start {
        return None;
    }
    Some(rendered[start + 1..end].to_string())
}

/// The id of one text part of a widget that already has a stable id.
///
/// Deterministic: the same base and role always produce the same id, and
/// the result round-trips through [`split_sub_part_id`]. `None` when the
/// base carries no stable name — see the module doc for why an anonymous
/// widget is never given one.
#[must_use]
pub fn sub_part_id(base: &Id, role_name: &str) -> Option<Id> {
    let name = custom_name(base)?;
    if name.is_empty() || role_name.is_empty() {
        return None;
    }
    Some(Id::from(format!("{name}{ROLE_SEPARATOR}{role_name}")))
}

/// The role name for the `index`-th part carrying `role_name`.
///
/// Used when one widget renders several parts of the same kind, so each
/// stays separately addressable.
#[must_use]
pub fn indexed_role(role_name: &str, index: usize) -> String {
    format!("{role_name}{INDEX_SEPARATOR}{index}")
}

/// Every role a hoarfrost widget publishes.
pub const ROLES: &[&str] = &[
    role::TITLE,
    role::SUBTITLE,
    role::HEADER,
    role::HINT,
    role::LABEL,
];

/// Whether `segment` names a published role — bare (`"title"`) or indexed
/// (`"label-2"`).
///
/// The indexed form requires digits after [`INDEX_SEPARATOR`], so a widget
/// whose own name happens to end in `label-thing` is not mistaken for a
/// part carrying an ordinal.
#[must_use]
pub fn is_role(segment: &str) -> bool {
    if ROLES.contains(&segment) {
        return true;
    }
    ROLES.iter().any(|base| {
        segment
            .strip_prefix(*base)
            .and_then(|rest| rest.strip_prefix(INDEX_SEPARATOR))
            .is_some_and(|ordinal| {
                !ordinal.is_empty() && ordinal.bytes().all(|b| b.is_ascii_digit())
            })
    })
}

/// Whether `name`'s last dotted segment names a role, i.e. `name` is a
/// derived sub-part id rather than a widget's own.
///
/// A host that queries by prefix needs this: because a derived id extends
/// its widget's name, `demo.menu.row.file.label` matches the prefix
/// `demo.menu.row.` and would otherwise be counted as another row.
///
/// A widget whose *own* name ends in a role word is read as a sub-part of
/// whatever precedes it. The convention owns that risk; hosts should not
/// name widgets `*.title`.
#[must_use]
pub fn is_sub_part_name(name: &str) -> bool {
    name.rsplit(ROLE_SEPARATOR)
        .next()
        .is_some_and(|segment| segment != name && is_role(segment))
}

/// Splits a rendered id into the widget it belongs to and the role it names.
///
/// Returns the base name and the role, or `None` when the name carries no
/// [`ROLE_SEPARATOR`] with something on both sides. Used by a host to
/// group published parts back under the widget that owns them.
#[must_use]
pub fn split_sub_part_name(name: &str) -> Option<(String, String)> {
    let (base, roles) = name.rsplit_once(ROLE_SEPARATOR)?;
    if base.is_empty() || roles.is_empty() {
        return None;
    }
    Some((base.to_string(), roles.to_string()))
}

/// [`split_sub_part_name`] applied to an [`Id`]'s stable name.
#[must_use]
pub fn split_sub_part_id(id: &Id) -> Option<(String, String)> {
    split_sub_part_name(&custom_name(id)?)
}

/// Publishes one run of text a widget rendered, under its derived id.
///
/// `bounds` must be the box the widget actually drew the text into, so a
/// host measuring the published record measures the glyphs and nothing
/// else. Calling this adds a `Candidate::Text` to the operation stream
/// and changes nothing about how the widget looks or lays out.
///
/// Silent when there is nothing to publish: empty content, or a base with
/// no stable name.
pub fn publish_text(
    operation: &mut dyn Operation,
    base: &Id,
    role_name: &str,
    bounds: Rectangle,
    content: &str,
) {
    if content.is_empty() {
        return;
    }
    if let Some(id) = sub_part_id(base, role_name) {
        operation.text(Some(&id), bounds, content);
    }
}

#[cfg(test)]
#[path = "identity_tests.rs"]
mod tests;
