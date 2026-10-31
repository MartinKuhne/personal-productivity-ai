//! Unit tests for [`crate::id_namespace`] — the reserved-namespace guard.
//!
//! A guard that has never failed is a comment with a function signature, so
//! every rule here is paired with a synthetic violation that must make it
//! fire, and the live demo is checked against the same predicate rather
//! than a transcription of it.

use hoarfrost::identity::{ROLES, is_role, is_sub_part_name};
use iced::{Point, Rectangle, Size};

use super::*;
use crate::app::DemoApp;
use crate::layout_probe::Entry;
use crate::section::{CAPTION_ID_SUFFIX, DEMO_LABEL_ID_SUFFIX, ROW_ID_SUFFIX};

/// The demo window's reference viewport, same as the gate's.
const ROOT: Size = Size::new(1400.0, 900.0);

/// Builds an entry with the geometry a test wants to reason about.
fn entry(id: &str, kind: &'static str) -> Entry {
    Entry {
        id: id.to_string(),
        kind,
        bounds: Rectangle::new(Point::new(0.0, 0.0), Size::new(64.0, 18.0)),
        visible: None,
    }
}

/// The four demo-authored Buttons-row label ids, as the demo builds them now.
fn demo_button_label_ids() -> Vec<String> {
    ["primary", "secondary", "ghost", "danger"]
        .iter()
        .map(|variant| {
            crate::section::label_id(crate::pane_atoms::BUTTON_ROW_LABEL_PREFIX, variant)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The namespace, stated exactly
// ---------------------------------------------------------------------------

#[test]
fn the_reserved_roles_are_the_five_identity_publishes() {
    assert_eq!(
        ROLES,
        &["title", "subtitle", "header", "hint", "label"],
        "the guard reads the framework's own role list; if this assertion trips, \
         the namespace changed and every demo id has to be re-checked against it"
    );
}

#[test]
fn every_bare_role_and_every_indexed_role_is_reserved() {
    for role in ROLES {
        assert!(is_role(role), "`{role}` must read as a role");
        for ordinal in [0usize, 1, 7, 42] {
            let indexed = hoarfrost::identity::indexed_role(role, ordinal);
            assert!(
                is_role(&indexed),
                "`{indexed}` is the indexed form of a role and must read as one"
            );
            assert!(
                is_sub_part_name(&format!("demo.anything.{indexed}")),
                "`demo.anything.{indexed}` sits inside the reserved namespace"
            );
        }
    }
}

#[test]
fn a_role_shaped_suffix_that_is_not_an_ordinal_is_not_reserved() {
    for name in [
        "label-thing",
        "title-",
        "hint-x1",
        "labels",
        "caption",
        "row",
        "live",
        "busy",
        "lower",
        "upper",
        "disabled",
    ] {
        assert!(
            !is_role(name),
            "`{name}` is not a role and must not be read as one"
        );
    }
}

#[test]
fn a_bare_role_with_no_widget_in_front_of_it_is_not_a_sub_part() {
    assert!(
        !is_sub_part_name("label"),
        "a name with no `.` before the role is a bare name, not a derived part"
    );
}

// ---------------------------------------------------------------------------
// The guard fires — synthetic violations
// ---------------------------------------------------------------------------

#[test]
fn a_container_wearing_a_reserved_label_is_flagged() {
    let squatters =
        reserved_namespace_squatters(&[entry("demo.atoms.buttons.danger.label", "container")]);

    assert_eq!(
        squatters.len(),
        1,
        "a demo container ending in the reserved `.label` role is exactly the \
         collision this guard exists for"
    );
    assert_eq!(squatters[0].id, "demo.atoms.buttons.danger.label");
    assert_eq!(squatters[0].kind, "container");
    assert_eq!(squatters[0].base, "demo.atoms.buttons.danger");
    assert_eq!(squatters[0].role, "label");
}

#[test]
fn a_container_wearing_an_indexed_reserved_role_is_flagged() {
    let squatters =
        reserved_namespace_squatters(&[entry("demo.composites.card.title-2", "container")]);

    assert_eq!(
        squatters.len(),
        1,
        "the indexed form `title-2` is reserved just as `title` is"
    );
    assert_eq!(squatters[0].role, "title-2");
    assert_eq!(squatters[0].base, "demo.composites.card");
}

#[test]
fn a_focusable_wearing_a_reserved_role_is_flagged_too() {
    let squatters = reserved_namespace_squatters(&[entry("demo.atoms.field.hint", "focusable")]);

    assert_eq!(
        squatters.len(),
        1,
        "only a published `text` caption may carry a reserved name; a focusable \
         wearing one is the same lie in a different kind"
    );
    assert_eq!(squatters[0].kind, "focusable");
}

#[test]
fn every_reserved_role_is_caught_on_a_container() {
    let entries: Vec<Entry> = ROLES
        .iter()
        .map(|role| entry(&format!("demo.sut.{role}"), "container"))
        .collect();

    let squatters = reserved_namespace_squatters(&entries);
    assert_eq!(
        squatters.len(),
        ROLES.len(),
        "the guard must catch a container on *every* reserved role, not just `label`"
    );
}

#[test]
fn the_squatter_report_says_who_and_what() {
    let squatters =
        reserved_namespace_squatters(&[entry("demo.atoms.buttons.danger.label", "container")]);
    let described = squatters[0].describe();

    assert!(described.contains("demo.atoms.buttons.danger"));
    assert!(described.contains("reserved"));
    assert!(described.contains(".demo-label"));
}

// ---------------------------------------------------------------------------
// The guard stays quiet — legitimate records
// ---------------------------------------------------------------------------

#[test]
fn a_framework_published_caption_is_never_a_squatter() {
    let squatters = reserved_namespace_squatters(&[
        entry("demo.atoms.checkbox.0", "focusable"),
        entry("demo.atoms.checkbox.0.label", "text"),
    ]);

    assert_eq!(
        squatters,
        Vec::<Squatter>::new(),
        "`demo.atoms.checkbox.0.label` is the widget's own published caption"
    );
}

#[test]
fn a_caption_whose_widget_registered_nothing_is_still_not_a_squatter() {
    let squatters = reserved_namespace_squatters(&[entry("demo.atoms.checkbox.3.label", "text")]);

    assert_eq!(
        squatters,
        Vec::<Squatter>::new(),
        "a disabled widget publishes its caption but no focusable of its own; an \
         absent base is the framework being incomplete, not the demo squatting"
    );
}

#[test]
fn ids_outside_the_namespace_are_never_flagged() {
    let entries = [
        entry(
            &format!("demo.atoms.buttons.danger{DEMO_LABEL_ID_SUFFIX}"),
            "container",
        ),
        entry(
            &format!("demo.section.buttons{CAPTION_ID_SUFFIX}"),
            "container",
        ),
        entry(
            &format!("demo.composites.setting-row.0{ROW_ID_SUFFIX}"),
            "container",
        ),
        entry("demo.atoms.range.live.lower", "focusable"),
        entry("demo.atoms.button.Primary.busy", "focusable"),
        entry("demo.atoms.text.readonly", "text_input"),
        entry("demo.chrome.status_bar", "container"),
    ];

    assert_eq!(
        reserved_namespace_squatters(&entries),
        Vec::<Squatter>::new(),
        "none of these ends in a reserved role"
    );
}

// ---------------------------------------------------------------------------
// The live demo
// ---------------------------------------------------------------------------

#[test]
fn the_live_demo_holds_the_reserved_namespace_clear() {
    let app = DemoApp::boot().0;
    let probe = crate::layout_probe::Probe::capture(ROOT, app.view());

    let squatters = reserved_namespace_squatters(probe.entries());
    assert!(
        squatters.is_empty(),
        "the rendered demo occupies the framework's reserved derived-sub-part \
         namespace without owning it:\n{}",
        squatters
            .iter()
            .map(Squatter::describe)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_live_demo_actually_populates_the_reserved_namespace() {
    let app = DemoApp::boot().0;
    let probe = crate::layout_probe::Probe::capture(ROOT, app.view());
    let published = published_sub_parts(probe.entries());

    assert!(
        !published.is_empty(),
        "the guard would pass vacuously if the capture carried no reserved names \
         at all; the framework publishes captions on every labelled control, so \
         an empty set here means the probe stopped seeing them"
    );
}

#[test]
fn the_renamed_demo_button_labels_are_present_and_outside_the_namespace() {
    let app = DemoApp::boot().0;
    let probe = crate::layout_probe::Probe::capture(ROOT, app.view());

    for id in demo_button_label_ids() {
        assert!(
            probe.find(&id).is_some(),
            "`{id}` is the demo-authored row label and must still be addressable"
        );
        assert!(
            !is_sub_part_name(&id),
            "`{id}` must not read as a derived sub-part"
        );
    }
}

#[test]
fn the_guard_bites_when_a_reserved_id_is_injected_into_the_live_stream() {
    let app = DemoApp::boot().0;
    let probe = crate::layout_probe::Probe::capture(ROOT, app.view());

    let mut entries: Vec<Entry> = probe.entries().to_vec();
    let injected = entry("demo.atoms.buttons.primary.label", "container");
    entries.push(injected);

    let squatters = reserved_namespace_squatters(&entries);
    assert_eq!(
        squatters.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        vec!["demo.atoms.buttons.primary.label"],
        "putting the pre-rename id back into the live stream must be caught by \
         name, and nothing else in the real demo may be swept up with it"
    );
}
