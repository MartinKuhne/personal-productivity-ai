//! Tests for the sub-part identity convention in [`super`].

use std::sync::{Arc, Mutex};

use iced::Rectangle;
use iced::advanced::widget::operation::Outcome;
use iced::advanced::widget::{Id, Operation};

use super::role;
use super::{
    ROLES, custom_name, indexed_role, is_role, is_sub_part_name, publish_text, split_sub_part_id,
    split_sub_part_name, sub_part_id,
};

/// One published text record: the id it carried, where it was drawn, and
/// the glyphs themselves.
type Published = (Option<String>, Rectangle, String);

/// An [`Operation`] that records every `text` call it is handed.
///
/// Every other method keeps its default no-op, so a test that asserts on
/// `texts` also proves nothing else was touched.
#[derive(Clone, Default)]
struct TextRecorder {
    texts: Arc<Mutex<Vec<Published>>>,
}

impl TextRecorder {
    fn new() -> Self {
        Self::default()
    }

    fn captured(&self) -> Vec<Published> {
        self.texts
            .lock()
            .expect("the recorder is not poisoned")
            .clone()
    }
}

impl Operation for TextRecorder {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn text(&mut self, id: Option<&Id>, bounds: Rectangle, text: &str) {
        self.texts
            .lock()
            .expect("the recorder is not poisoned")
            .push((id.map(|id| format!("{id:?}")), bounds, text.to_owned()));
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(iced::Point::new(x, y), iced::Size::new(w, h))
}

#[test]
fn custom_name_reads_the_custom_variant() {
    assert_eq!(
        custom_name(&Id::new("demo.atoms.checkbox")),
        Some("demo.atoms.checkbox".to_string())
    );
    assert_eq!(
        custom_name(&Id::from(String::from("owned.name"))),
        Some("owned.name".to_string())
    );
}

#[test]
fn custom_name_rejects_a_counter_id() {
    assert_eq!(custom_name(&Id::unique()), None);
}

#[test]
fn sub_part_id_is_deterministic() {
    let base = Id::new("demo.composites.setting.0");

    let first = sub_part_id(&base, role::TITLE).expect("a named base yields a sub-part id");
    let second = sub_part_id(&base, role::TITLE).expect("a named base yields a sub-part id");

    assert_eq!(first, second);
}

#[test]
fn sub_part_id_round_trips_through_split() {
    let base = Id::new("demo.composites.setting.0");

    for role_name in [
        role::TITLE,
        role::SUBTITLE,
        role::HEADER,
        role::HINT,
        role::LABEL,
    ] {
        let derived = sub_part_id(&base, role_name).expect("a named base yields a sub-part id");
        let (split_base, split_role) =
            split_sub_part_id(&derived).expect("a derived id splits back into base and role");

        assert_eq!(split_base, "demo.composites.setting.0");
        assert_eq!(split_role, role_name);
    }
}

#[test]
fn sub_part_id_keeps_a_dotted_base_intact() {
    let derived =
        sub_part_id(&Id::new("a.b.c"), role::LABEL).expect("a named base yields a sub-part id");

    assert_eq!(custom_name(&derived), Some("a.b.c.label".to_string()));
    assert_eq!(
        split_sub_part_id(&derived),
        Some(("a.b.c".to_string(), "label".to_string()))
    );
}

#[test]
fn sub_part_id_refuses_to_name_an_anonymous_widget() {
    assert_eq!(sub_part_id(&Id::unique(), role::LABEL), None);
}

#[test]
fn sub_part_id_refuses_an_empty_role() {
    assert_eq!(sub_part_id(&Id::new("demo.widget"), ""), None);
}

#[test]
fn split_needs_something_on_both_sides_of_the_separator() {
    assert_eq!(split_sub_part_name("no-separator"), None);
    assert_eq!(split_sub_part_name(".leading"), None);
    assert_eq!(split_sub_part_name("trailing."), None);
}

#[test]
fn split_of_an_id_without_a_stable_name_is_none() {
    assert_eq!(split_sub_part_id(&Id::unique()), None);
}

#[test]
fn every_published_role_is_recognised_bare_and_indexed() {
    for base in ROLES {
        assert!(is_role(base), "{base} must read as a role");
        assert!(
            is_role(&indexed_role(base, 0)),
            "{base}-0 must read as a role"
        );
        assert!(
            is_role(&indexed_role(base, 17)),
            "{base}-17 must read as a role"
        );
    }
}

#[test]
fn a_word_that_is_not_a_role_is_not_one() {
    for word in [
        "", "file", "Title", "card", "row", "label-x", "label-", "hint2", "title-1a",
    ] {
        assert!(!is_role(word), "{word:?} must not read as a role");
    }
}

#[test]
fn a_derived_name_is_told_apart_from_its_widgets_own_id() {
    // The whole point: the caption sits *inside* the row's own prefix, so
    // only the tail tells them apart.
    assert!(is_sub_part_name("demo.menu.row.file.label"));
    assert!(is_sub_part_name("demo.atoms.radio.label-2"));
    assert!(is_sub_part_name("a.b.title"));
    assert!(!is_sub_part_name("demo.menu.row.file"));
    assert!(!is_sub_part_name("demo.atoms.radio"));
    // A bare role word with no widget in front of it is not a sub-part:
    // there is nothing for it to belong to.
    assert!(!is_sub_part_name("label"));
    assert!(!is_sub_part_name("label-3"));
}

#[test]
fn a_sub_part_of_a_sub_part_is_still_only_its_own_tail() {
    // Nesting is not part of the convention, but the read must stay
    // well-defined rather than chase the chain.
    assert!(is_sub_part_name("a.b.title.subtitle"));
    assert!(!is_sub_part_name("a.b.subtitle.titlex"));
}

#[test]
fn indexed_roles_stay_distinct_and_keep_their_base() {
    let base = Id::new("demo.atoms.radio");

    let zero = sub_part_id(&base, &indexed_role(role::LABEL, 0)).expect("named base");
    let one = sub_part_id(&base, &indexed_role(role::LABEL, 1)).expect("named base");

    assert_ne!(zero, one);
    assert_eq!(
        custom_name(&zero),
        Some("demo.atoms.radio.label-0".to_string())
    );
    assert_eq!(
        split_sub_part_id(&one),
        Some(("demo.atoms.radio".to_string(), "label-1".to_string()))
    );
}

#[test]
fn publish_text_emits_one_named_candidate() {
    let recorder = TextRecorder::new();
    let mut operation: Box<dyn Operation> = Box::new(recorder.clone());
    let bounds = rect(10.0, 20.0, 120.0, 24.0);

    publish_text(
        operation.as_mut(),
        &Id::new("demo.atoms.toggle"),
        role::LABEL,
        bounds,
        "Enable thing",
    );

    let captured = recorder.captured();
    assert_eq!(captured.len(), 1);
    let (id, published, content) = &captured[0];
    assert_eq!(
        id.as_deref(),
        Some("Id(Custom(\"demo.atoms.toggle.label\"))"),
        "the published id must render as a Custom variant so a host's \
         stable-name filter keeps it"
    );
    assert_eq!(*published, bounds);
    assert_eq!(content, "Enable thing");
}

#[test]
fn publish_text_skips_empty_content() {
    let recorder = TextRecorder::new();
    let mut operation: Box<dyn Operation> = Box::new(recorder.clone());

    publish_text(
        operation.as_mut(),
        &Id::new("demo.atoms.toggle"),
        role::LABEL,
        rect(0.0, 0.0, 10.0, 10.0),
        "",
    );

    assert!(recorder.captured().is_empty());
}

#[test]
fn publish_text_skips_an_unnamed_base() {
    let recorder = TextRecorder::new();
    let mut operation: Box<dyn Operation> = Box::new(recorder.clone());

    publish_text(
        operation.as_mut(),
        &Id::unique(),
        role::TITLE,
        rect(0.0, 0.0, 10.0, 10.0),
        "Orphan",
    );

    assert!(recorder.captured().is_empty());
}

#[test]
fn publish_text_leaves_the_operation_open() {
    let recorder = TextRecorder::new();
    let mut operation: Box<dyn Operation> = Box::new(recorder);

    publish_text(
        operation.as_mut(),
        &Id::new("demo.widget"),
        role::HINT,
        rect(0.0, 0.0, 1.0, 1.0),
        "hint",
    );

    assert!(
        matches!(operation.finish(), Outcome::None),
        "publishing must not finish or chain the host operation"
    );
}
