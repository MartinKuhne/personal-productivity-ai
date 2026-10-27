//! Tests for the hyperlink model and its emissions — `UIF-ATOM-001d`,
//! §3.1d.
//!
//! Four groups, in order:
//!
//! 1. **Activation carries a target.** The value that reaches the host
//!    says *what* to open and *whether it leaves the application*, not
//!    just a string.
//! 2. **The hover preview is its own tier.** It fires on entry, clears
//!    on exit, and never looks like an activation.
//! 3. **A dead link emits neither.**
//! 4. **Model plumbing.** The declarative fields and the builders that
//!    set them.

use super::*;

fn link() -> HyperlinkModel {
    HyperlinkModel::new("the docs", "fastmd://doc/welcome")
}

// ---------------------------------------------------------------------------
// 1. Activation carries a target
// ---------------------------------------------------------------------------

#[test]
fn activation_carries_a_target_not_a_bare_string() {
    let commit = link().activate().expect("a live link activates");
    let target: &HyperlinkTarget = commit.value();
    assert_eq!(target.uri, "fastmd://doc/welcome");
    assert!(!target.external);
}

#[test]
fn an_external_link_says_so_in_its_target() {
    let commit = HyperlinkModel::external("home", "https://example.com")
        .activate()
        .expect("a live link activates");
    assert!(commit.value().external);
    assert_eq!(commit.value().uri, "https://example.com");
}

#[test]
fn the_uri_is_carried_verbatim_rather_than_normalised() {
    let raw = "fastmd://note/Some%20Title#section";
    let target = HyperlinkModel::new("n", raw)
        .activate()
        .unwrap()
        .into_value();
    assert_eq!(target.uri, raw);
}

#[test]
fn a_target_displays_as_its_uri() {
    let target = link().target();
    assert_eq!(target.to_string(), "fastmd://doc/welcome");
}

#[test]
fn the_open_tier_reports_its_uri_through_the_shared_accessor() {
    let emission = HyperlinkEmission::Opened(Commit::new(link().target()));
    assert_eq!(emission.uri(), Some("fastmd://doc/welcome"));
    assert!(emission.is_open());
}

// ---------------------------------------------------------------------------
// 2. The hover preview is its own tier
// ---------------------------------------------------------------------------

#[test]
fn hovering_a_live_link_previews_its_uri() {
    let emission = link().hover_entered().expect("a live link previews");
    assert_eq!(
        emission,
        HyperlinkEmission::HoverUri(Some("fastmd://doc/welcome".into()))
    );
    assert!(!emission.is_open());
}

#[test]
fn leaving_a_link_clears_the_preview_to_none_not_an_empty_string() {
    let emission = HyperlinkModel::hover_cleared();
    assert_eq!(emission, HyperlinkEmission::HoverUri(None));
    assert_eq!(emission.uri(), None);
}

#[test]
fn an_open_and_a_preview_of_the_same_uri_are_different_events() {
    let uri = "fastmd://doc/welcome";
    let opened = HyperlinkEmission::Opened(Commit::new(HyperlinkTarget::new(uri, false)));
    let preview = HyperlinkEmission::HoverUri(Some(uri.to_owned()));
    assert_ne!(opened, preview);
    assert_eq!(opened.uri(), preview.uri());
}

#[test]
fn the_preview_tier_carries_its_uri_through_the_shared_accessor() {
    let emission = HyperlinkEmission::HoverUri(Some("x".into()));
    assert_eq!(emission.uri(), Some("x"));
    assert!(!emission.is_open());
}

// ---------------------------------------------------------------------------
// 3. A dead link emits neither
// ---------------------------------------------------------------------------

#[test]
fn a_disabled_link_does_not_activate() {
    assert!(link().disabled().activate().is_none());
}

#[test]
fn a_disabled_link_does_not_preview_its_uri() {
    let dead = link().disabled();
    assert!(dead.hover_entered().is_none());
    assert!(dead.hover_uri().is_none());
}

#[test]
fn a_disabled_link_is_not_enabled_and_vice_versa() {
    assert!(link().enabled());
    assert!(!link().disabled().enabled());
}

// ---------------------------------------------------------------------------
// 4. Model plumbing
// ---------------------------------------------------------------------------

#[test]
fn a_new_link_is_live_and_internal() {
    let m = link();
    assert_eq!(m.label, "the docs");
    assert_eq!(m.uri, "fastmd://doc/welcome");
    assert!(!m.external);
    assert!(!m.disabled);
}

#[test]
fn the_externality_can_be_set_explicitly_in_both_directions() {
    assert!(HyperlinkModel::new("x", "y").with_external(true).external);
    assert!(
        !HyperlinkModel::external("x", "y")
            .with_external(false)
            .external
    );
}

#[test]
fn target_tracks_the_models_current_fields() {
    let mut m = link();
    assert_eq!(m.target().uri, "fastmd://doc/welcome");
    m.uri = "fastmd://doc/other".into();
    m.external = true;
    assert_eq!(m.target().uri, "fastmd://doc/other");
    assert!(m.target().external);
}

#[test]
fn activating_twice_produces_two_independent_targets() {
    let m = link();
    let a = m.activate().unwrap().into_value();
    let b = m.activate().unwrap().into_value();
    assert_eq!(a, b);
}

#[test]
fn an_empty_uri_is_still_a_target_the_model_honours() {
    // The atom does not validate URIs; that is the host's policy. What it
    // must do is carry whatever it was given without inventing one.
    let m = HyperlinkModel::new("x", "");
    assert_eq!(m.activate().unwrap().into_value().uri, "");
    assert_eq!(m.hover_uri(), Some(String::new()));
}
