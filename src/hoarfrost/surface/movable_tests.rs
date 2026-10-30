//! Tests for [`movable`] — the Band-1 movable overlay host: the title-bar
//! drag, the reachability clamp, the pass-through boundary, the auto-scroll
//! machine and the injected persistence sink (`UIF-MOV-001` …
//! `UIF-MOV-004`).
//!
//! # The groups, in order
//!
//! 1. **Anatomy and token trace** — every number §4.7.1 names is either a
//!    token read or a declared constant, and the derived chrome and
//!    viewport floors follow from them.
//! 2. **The geometry value** — `encode`/`decode` round trips, and the
//!    rejections that keep a corrupt saved value out of layout.
//! 3. **Injected persistence** — the `D-2` / `RUST-024` contract, closed
//!    with a source scan proving this module cannot name a filesystem.
//! 4. **`UIF-MOV-001`** — the drag, the trailing-action exclusion,
//!    real-time deltas and the no-accumulation overshoot contract. The
//!    `on_drag_settle` tier discipline around it — the revert target is
//!    the grab, one `Commit` per drag, and `Escape` spends itself on the
//!    revert rather than closing the card (§3.0.4.2, `INC-36`) — is
//!    group 8b, immediately after the keyboard group.
//! 5. **`UIF-MOV-002`** — all eight resize handles, the size floor, the
//!    untouched axis, and a sweep proving no requested geometry can
//!    produce an unreachable card.
//! 6. **`UIF-MOV-003`** — clicks and scrolls outside the card fall
//!    through, promotion is the band's decision, and a surface above the
//!    card owns the point instead.
//! 7. **`UIF-MOV-004`** — the full follow/pause matrix, the transitions
//!    between them, and the rule that a paused viewport never moves.
//! 8. **Keyboard** — `Escape` closes a focused card, declines an
//!    unfocused one, and never steals the key from a menu or a modal
//!    (**G-13**).
//! 9. **Lifecycle** — open, close, reopen, roster rebuild.
//! 10. **The frame** — determinism, and the router agreeing with the model
//!     through the shared harness.
//! 11. **The surface index follows the card** — the `defect_` regressions:
//!     every gesture that moves the card must republish its rectangle, so
//!     the index never hands the card ground it has left or denies it ground
//!     it now owns.
//!
//! # Why sweeps rather than a handful of points
//!
//! `UIF-MOV-002` is a universal claim: *no* geometry the host can be
//! asked for may lose the title bar. Four example positions show the clamp
//! works four times. The sweeps in groups 5 and 7 iterate the boundary
//! conditions — every handle, every out-of-bounds direction, every cell of
//! the follow matrix — so the claim is tested as the universal it is.

use super::*;

use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::focus::GroupId;
use crate::surface::zorder::Declination;
use crate::surface::zorder::EscapeRefusal;
use crate::surface::zorder::PointerTarget;
use crate::test_helpers::Harness;
use crate::test_helpers::assert_agree;
use crate::test_helpers::iced_id;
use crate::test_helpers::mixed_focusable_tree;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::token::typography;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The window a card is clamped against: comfortably larger than the card,
/// so a test can say "off the right edge" without also being off the top.
fn viewport() -> Rectangle {
    rect(0.0, 0.0, 1600.0, 1000.0)
}

fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

fn vec(dx: f32, dy: f32) -> Vector {
    Vector::new(dx, dy)
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(pt(x, y), Size::new(w, h))
}

fn geo(x: f32, y: f32, w: f32, h: f32) -> MovableGeometry {
    MovableGeometry::new(pt(x, y), Size::new(w, h))
}

fn sid(name: &str) -> SurfaceId {
    SurfaceId::new(name)
}

fn fid(name: &str) -> FocusId {
    FocusId::new(name)
}

fn gid(name: &str) -> GroupId {
    GroupId::new(name)
}

/// The centre of a rectangle — the safest point a test can land inside it.
fn centre_of(bounds: Rectangle) -> Point {
    Point::new(
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    )
}

/// The centre of the card's `Dock` control, read from the host's own
/// geometry so the probe cannot drift from the anatomy.
fn dock_centre(host: &MovableHost) -> Point {
    let zone = host.trailing_actions();
    centre_of(rect(zone.x, zone.y, ACTION_CONTROL_SIZE, zone.height))
}

/// The centre of the card's `Close ×` control.
fn close_centre(host: &MovableHost) -> Point {
    let zone = host.trailing_actions();
    centre_of(rect(
        zone.x + ACTION_CONTROL_SIZE + ACTION_GAP,
        zone.y,
        ACTION_CONTROL_SIZE,
        zone.height,
    ))
}

/// A point well inside the content viewport, clear of every grip band.
fn content_centre(host: &MovableHost) -> Point {
    let view = host.content_viewport();
    centre_of(rect(
        view.x + RESIZE_HANDLE_SIZE * 2.0,
        view.y,
        (view.width - RESIZE_HANDLE_SIZE * 4.0).max(1.0),
        view.height,
    ))
}

/// A card, its focus model, its registry and its sink, carried together.
///
/// Every interaction the host has takes three collaborators; passing four
/// separate locals to forty tests is how one of them quietly stops being
/// updated, so the fixture owns them as one thing.
struct Scenario {
    host: MovableHost,
    focus: FocusState,
    registry: SurfaceRegistry,
    storage: MemoryGeometryStorage,
}

impl Scenario {
    /// Open `name` at the default geometry in the default viewport.
    fn open(name: &str) -> Self {
        Self::open_in(name, viewport(), None)
    }

    /// Open `name` at an explicit geometry.
    fn at(x: f32, y: f32) -> Self {
        Self::open_in("logs", viewport(), Some(geo(x, y, 680.0, 440.0)))
    }

    /// Open `name` in an arbitrary viewport at an arbitrary geometry.
    fn open_in(name: &str, view: Rectangle, geometry: Option<MovableGeometry>) -> Self {
        let mut focus = FocusState::new();
        let mut registry = SurfaceRegistry::new();
        let mut storage = MemoryGeometryStorage::new();

        let mut spec = MovableSpec::new(name, "Background Operations Log");
        if let Some(geometry) = geometry {
            spec = spec.with_geometry(geometry);
        }

        let host = MovableHost::open(spec, view, &mut focus, &mut registry, &mut storage);
        Self {
            host,
            focus,
            registry,
            storage,
        }
    }

    /// Drive one pointer phase through the host.
    fn pointer(
        &mut self,
        phase: PointerPhase,
        at: Point,
        target: SurfaceTarget,
    ) -> MovableResponse {
        self.host.on_pointer(
            phase,
            at,
            target,
            &mut self.focus,
            &mut self.registry,
            &mut self.storage,
        )
    }

    /// Press, move and release a drag in one call, returning the response
    /// for each step.
    fn drag_gesture(&mut self, grab: Point, moves: &[Point]) -> Vec<MovableResponse> {
        let mut steps = vec![self.pointer(PointerPhase::Pressed, grab, SurfaceTarget::TitleBar)];
        for at in moves {
            steps.push(self.pointer(PointerPhase::Moved, *at, SurfaceTarget::TitleBar));
        }
        steps.push(self.pointer(
            PointerPhase::Released,
            moves.last().copied().unwrap_or(grab),
            SurfaceTarget::TitleBar,
        ));
        steps
    }

    /// The geometry the sink currently holds for this card.
    fn saved(&mut self) -> Option<MovableGeometry> {
        let key = self.host.storage_key();
        self.storage.load(&key)
    }

    /// Put the keyboard inside the card, the way a click on one of its own
    /// controls would.
    ///
    /// The member has to live in the card's *own* focus group for
    /// [`MovableHost::holds_focus`] to be true, which is why this goes
    /// through `sync_members` rather than a separately named group.
    fn focus_card(&mut self) {
        self.host
            .sync_members(vec![FocusMember::new("card.filter")], &mut self.focus);
        assert!(self.focus.focus(&fid("card.filter")));
        assert!(self.host.holds_focus(&self.focus));
    }
}

// ---------------------------------------------------------------------------
// 1. Anatomy and token trace
// ---------------------------------------------------------------------------

#[test]
fn test_every_anatomy_number_that_is_a_token_reads_its_token() {
    assert_eq!(TITLE_BAR_HEIGHT, size::CONTROL_MD);
    assert_eq!(TITLE_BAR_PADDING_HORIZONTAL, space::MD);
    assert_eq!(TITLE_GRIP_WIDTH, size::ICON_MD);
    assert_eq!(TITLE_FONT_SIZE, typography::UI_COMPACT.size);
    assert_eq!(ACTION_CONTROL_SIZE, size::HIT_TARGET_MIN);
    assert_eq!(ACTION_GAP, space::SM);
    assert_eq!(FILTER_BAR_HEIGHT, size::CONTROL_LG);
    assert_eq!(FILTER_BAR_PADDING_VERTICAL, space::XS);
    assert_eq!(FILTER_BAR_PADDING_HORIZONTAL, space::MD);
    assert_eq!(FILTER_BAR_GAP, space::SM);
    assert_eq!(CONTENT_FONT_SMALL, typography::CAPTION.size);
    assert_eq!(CONTENT_FONT_LARGE, typography::UI_COMPACT.size);
}

#[test]
fn test_the_two_zero_paddings_are_named_rather_than_bare() {
    assert_eq!(TITLE_BAR_PADDING_VERTICAL, 0.0);
    assert_eq!(FOOTER_PADDING_VERTICAL, 0.0);
}

/// §4.7.1's title is `13px Bold`, and both halves now come from the
/// scale: the size is `type.ui_compact`'s, and `Bold` is a member of
/// that rung's allowed-weight set. What G-12 recorded as a hand-composed
/// pairing is legal by token, and the rendered size is exactly what it
/// was before — a provenance change, not a visual one.
#[test]
fn test_the_title_weight_is_permitted_by_the_rung_its_size_comes_from() {
    assert_eq!(TITLE_FONT_SIZE, typography::UI_COMPACT.size);
    assert_eq!(TITLE_FONT_SIZE, 13.0, "the title size must not have moved");
    assert_eq!(TITLE_FONT_WEIGHT, typography::Weight::Bold);
    assert!(
        typography::UI_COMPACT.allows(TITLE_FONT_WEIGHT),
        "type.ui_compact must permit the Bold the §4.7.1 overlay title wears; \
         its allowed set is {:?}",
        typography::UI_COMPACT.allowed_weights
    );
}

#[test]
fn test_the_untokenised_numbers_are_declared_constants() {
    // These three are the token gaps §4.7.1 leaves behind. They are named
    // here so that a future tokenisation has something to point at rather
    // than a literal buried in a layout function. The footer is no longer
    // one of them — see the test below.
    assert_eq!(CARD_DEFAULT_WIDTH, 680.0);
    assert_eq!(CARD_DEFAULT_HEIGHT, 440.0);
    assert_eq!(RESIZE_HANDLE_SIZE, 16.0);
}

/// The footer stopped being a token gap when the 2026-10-22 revision of
/// §2.2 added `size.bar.status` for exactly this bar. It reads the rung
/// now, and the rung is 24 — two pixels taller than the literal this
/// module used to carry.
#[test]
fn test_the_footer_height_is_the_status_bar_token_not_a_literal() {
    assert_eq!(FOOTER_HEIGHT, size::BAR_STATUS);
    assert_eq!(
        FOOTER_HEIGHT, 24.0,
        "the footer is size.bar.status (24px), not the 22px literal it was"
    );
}

#[test]
fn test_the_size_floor_is_read_from_the_clamp_solver_not_restated() {
    assert_eq!(CARD_MIN_WIDTH, clamp::SizePolicy::MOVABLE.min.width);
    assert_eq!(CARD_MIN_HEIGHT, clamp::SizePolicy::MOVABLE.min.height);
    assert_eq!(CARD_MIN_WIDTH, 420.0);
    assert_eq!(CARD_MIN_HEIGHT, 260.0);
}

#[test]
fn test_chrome_and_viewport_floor_are_derived_from_the_bars() {
    assert_eq!(
        CHROME_HEIGHT,
        TITLE_BAR_HEIGHT + FILTER_BAR_HEIGHT + FOOTER_HEIGHT
    );
    // 32 + 36 + 24. The 2px over the 90 this test used to pin is the
    // footer moving off its old 22px literal onto size.bar.status; the
    // chrome is derived, so it moved by itself and this line is the proof
    // that it moved by exactly 2 and nothing else did.
    assert_eq!(CHROME_HEIGHT, 92.0);
    assert_eq!(
        CONTENT_VIEWPORT_MIN,
        CARD_MIN_HEIGHT - CHROME_HEIGHT,
        "the viewport floor is the smallest card minus its chrome"
    );
    // 260 - 92: the same 2px, arriving on the other side of the ledger.
    assert_eq!(CONTENT_VIEWPORT_MIN, 168.0);
    assert_eq!(AT_BOTTOM_TOLERANCE, space::XXS);
}

#[test]
fn test_the_anatomy_resolves_colours_and_dimensions_from_one_palette() {
    let palette = Palette::dark();
    let anatomy = MovableAnatomy::resolve(&palette);

    assert_eq!(anatomy.card_surface, palette.surface_overlay);
    assert_eq!(anatomy.card_border, palette.border_strong);
    assert_eq!(anatomy.card_border_width, shape::BORDER_HAIRLINE);
    assert_eq!(anatomy.card_radius, shape::RADIUS_CARD);
    assert_eq!(anatomy.title_bar_surface, palette.surface_active);
    assert_eq!(anatomy.bar_surface, palette.surface_raised);
    assert_eq!(anatomy.content_surface, palette.surface_sunken);
    assert_eq!(anatomy.title_text, palette.text_strong);
    assert_eq!(anatomy.meta_text, palette.text_muted);
    assert_eq!(anatomy.tag_text, palette.accent_text);
    assert_eq!(anatomy.divider, palette.border_subtle);

    assert_eq!(anatomy.title_bar_height, TITLE_BAR_HEIGHT);
    assert_eq!(
        anatomy.title_bar_padding_horizontal,
        TITLE_BAR_PADDING_HORIZONTAL
    );
    assert_eq!(anatomy.filter_bar_height, FILTER_BAR_HEIGHT);
    assert_eq!(
        anatomy.filter_bar_padding_vertical,
        FILTER_BAR_PADDING_VERTICAL
    );
    assert_eq!(anatomy.filter_bar_gap, FILTER_BAR_GAP);
    assert_eq!(anatomy.footer_height, FOOTER_HEIGHT);
    assert_eq!(anatomy.footer_padding_horizontal, FOOTER_PADDING_HORIZONTAL);
    assert_eq!(anatomy.resize_handle_size, RESIZE_HANDLE_SIZE);
    assert_eq!(anatomy.action_control_size, ACTION_CONTROL_SIZE);
}

#[test]
fn test_the_anatomy_is_the_same_for_every_card_of_a_palette() {
    let a = MovableAnatomy::resolve(&Palette::dark());
    let b = MovableAnatomy::resolve(&Palette::dark());
    assert_eq!(
        a, b,
        "the anatomy is a pure resolution, not a per-card guess"
    );
}

// ---------------------------------------------------------------------------
// 2. The geometry value
// ---------------------------------------------------------------------------

#[test]
fn test_default_geometry_is_the_spec_card() {
    assert_eq!(
        MovableGeometry::DEFAULT,
        geo(0.0, 0.0, CARD_DEFAULT_WIDTH, CARD_DEFAULT_HEIGHT)
    );
}

#[test]
fn test_encode_decode_round_trips_exactly() {
    let cases = [
        geo(0.0, 0.0, 680.0, 440.0),
        geo(-320.5, 44.25, 420.0, 260.0),
        geo(1599.999, 999.001, 1024.5, 768.25),
        geo(1.0 / 3.0, 2.0 / 3.0, 1.0 / 7.0, 123.456),
        geo(f32::MIN, f32::MAX, 1.0, 1.0),
        geo(0.0, 0.0, 0.0, 0.0),
    ];

    for case in cases {
        let text = case.encode();
        let back = MovableGeometry::decode(&text)
            .unwrap_or_else(|| panic!("`{text}` must decode; it came from `encode`"));
        assert_eq!(
            back, case,
            "`{text}` must parse back to the value that produced it"
        );
        assert_eq!(back.encode(), text, "the canonical form must be stable");
    }
}

#[test]
fn test_decode_rejects_every_shape_that_is_not_four_finite_numbers() {
    for text in [
        "",
        " ",
        "680",
        "0,0,680",
        "0,0,680,440,0",
        "a,b,c,d",
        "0,0,680,abc",
        "nan,0,680,440",
        "0,nan,680,440",
        "0,0,nan,440",
        "0,0,680,nan",
        "inf,0,680,440",
        "-inf,0,680,440",
        "0,0,inf,440",
        "0,0,680,-inf",
    ] {
        assert_eq!(
            MovableGeometry::decode(text),
            None,
            "`{text}` must not be accepted as a saved geometry"
        );
    }
}

#[test]
fn test_decode_rejects_negative_extents() {
    assert_eq!(MovableGeometry::decode("0,0,-680,440"), None);
    assert_eq!(MovableGeometry::decode("0,0,680,-440"), None);
    assert_eq!(MovableGeometry::decode("0,0,-1,-1"), None);
}

#[test]
fn test_decode_tolerates_surrounding_whitespace_around_each_field() {
    assert_eq!(
        MovableGeometry::decode(" 0 , 36 , 680 , 440 "),
        Some(geo(0.0, 36.0, 680.0, 440.0))
    );
}

#[test]
fn test_sanitized_lands_non_finite_and_negative_values_on_zero() {
    let dirty = MovableGeometry::new(
        Point::new(f32::NAN, f32::NEG_INFINITY),
        Size::new(f32::INFINITY, -50.0),
    );
    assert_eq!(dirty.sanitized(), geo(0.0, 0.0, 0.0, 0.0));

    let clean = geo(10.0, 20.0, 30.0, 40.0);
    assert_eq!(clean.sanitized(), clean);
}

// ---------------------------------------------------------------------------
// 3. Injected persistence (`D-2` / `RUST-024`)
// ---------------------------------------------------------------------------

#[test]
fn test_the_key_and_the_group_are_derived_from_the_surface_id() {
    let id = sid("agent-debug");
    assert_eq!(storage_key(&id), "movable:agent-debug");
    assert_eq!(group_for(&id), gid("movable:agent-debug"));

    let other = sid("logs");
    assert_ne!(storage_key(&id), storage_key(&other));
    assert_ne!(group_for(&id), group_for(&other));
}

#[test]
fn test_the_host_reports_the_key_and_group_of_its_own_id() {
    let scenario = Scenario::open("agent-debug");
    assert_eq!(
        scenario.host.storage_key(),
        storage_key(&sid("agent-debug"))
    );
    assert_eq!(scenario.host.group_id(), &group_for(&sid("agent-debug")));
}

#[test]
fn test_opening_loads_the_saved_geometry_instead_of_the_default() {
    let mut storage = MemoryGeometryStorage::new();
    storage.save("movable:logs", geo(321.0, 65.0, 700.0, 500.0));

    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let host = MovableHost::open(
        MovableSpec::new("logs", "Logs"),
        viewport(),
        &mut focus,
        &mut registry,
        &mut storage,
    );

    assert_eq!(host.geometry(), geo(321.0, 65.0, 700.0, 500.0));
}

#[test]
fn test_an_explicit_spec_geometry_beats_the_saved_one() {
    let mut storage = MemoryGeometryStorage::new();
    storage.save("movable:logs", geo(321.0, 65.0, 700.0, 500.0));

    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let host = MovableHost::open(
        MovableSpec::new("logs", "Logs").with_geometry(geo(10.0, 50.0, 500.0, 300.0)),
        viewport(),
        &mut focus,
        &mut registry,
        &mut storage,
    );

    assert_eq!(host.geometry(), geo(10.0, 50.0, 500.0, 300.0));
}

#[test]
fn test_a_saved_geometry_is_still_clamped_on_the_way_in() {
    // Saved on a 2560-wide monitor, opened on a 1024 one: the card comes
    // back, but its title bar comes back with it.
    let mut storage = MemoryGeometryStorage::new();
    storage.save("movable:logs", geo(2400.0, 10.0, 680.0, 440.0));

    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let host = MovableHost::open(
        MovableSpec::new("logs", "Logs"),
        rect(0.0, 0.0, 1024.0, 768.0),
        &mut focus,
        &mut registry,
        &mut storage,
    );

    assert!(host.is_reachable(), "UIF-MOV-002 survives a restart");
    assert_eq!(
        host.geometry().size,
        Size::new(680.0, 440.0),
        "the size is preserved; only the origin is pulled back"
    );
    assert!(host.geometry().position.x <= 1024.0 - 120.0);
    assert!(host.geometry().position.y >= 36.0);
}

#[test]
fn test_a_restart_through_the_sink_restores_where_the_user_left_the_card() {
    let mut scenario = Scenario::open("logs");
    scenario.host.drag_by(vec(123.0, 45.0));
    scenario
        .host
        .end_drag(&mut scenario.registry, &mut scenario.storage);
    let settled = scenario.host.geometry();

    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let restored = MovableHost::open(
        MovableSpec::new("logs", "Logs"),
        viewport(),
        &mut focus,
        &mut registry,
        &mut scenario.storage,
    );

    assert_eq!(restored.geometry(), settled);
}

#[test]
fn test_persistence_happens_at_settled_geometry_and_not_during_the_drag() {
    let mut scenario = Scenario::open("logs");
    scenario
        .storage
        .save(&scenario.host.storage_key(), geo(1.0, 40.0, 680.0, 440.0));
    let baseline = scenario.saved();

    scenario.host.drag_by(vec(50.0, 0.0));
    scenario.host.drag_by(vec(50.0, 0.0));
    assert_eq!(
        scenario.saved(),
        baseline,
        "a drag frame must not put the sink on the frame path"
    );

    scenario.host.drag_by(vec(50.0, 0.0));
    scenario
        .host
        .end_drag(&mut scenario.registry, &mut scenario.storage);
    assert_eq!(
        scenario.saved(),
        Some(scenario.host.geometry()),
        "the release writes the settled geometry"
    );
}

#[test]
fn test_close_persists_and_the_sink_holds_the_final_geometry() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.host.drag_by(vec(-30.0, 30.0));
    let before_close = scenario.host.geometry();

    let response = scenario.host.close(
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );

    assert!(!scenario.host.is_open());
    assert_eq!(scenario.saved(), Some(before_close));
    assert!(!response.stays_open());
    assert!(!scenario.registry.contains(&sid("logs")));
}

#[test]
fn test_the_noop_sink_absorbs_every_write_and_reads_nothing_back() {
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let mut storage = NoopGeometryStorage;

    let mut host = MovableHost::open(
        MovableSpec::new("logs", "Logs"),
        viewport(),
        &mut focus,
        &mut registry,
        &mut storage,
    );
    host.drag_by(vec(40.0, 40.0));
    host.end_drag(&mut registry, &mut storage);
    host.persist(&mut storage);
    host.close(&mut focus, &mut registry, &mut storage);

    assert_eq!(
        storage.load("movable:logs"),
        None,
        "the noop sink must not remember anything"
    );
}

#[test]
fn test_the_memory_sink_is_a_plain_map_with_no_path_in_it() {
    let mut storage = MemoryGeometryStorage::new();
    assert!(storage.is_empty());

    storage.save("movable:a", geo(0.0, 36.0, 680.0, 440.0));
    storage.save("movable:b", geo(10.0, 46.0, 680.0, 440.0));
    storage.save("movable:a", geo(20.0, 56.0, 680.0, 440.0));

    assert_eq!(storage.len(), 2, "a re-save must not add a second entry");
    assert_eq!(
        storage.keys().collect::<Vec<_>>(),
        vec!["movable:a", "movable:b"],
        "keys keep the order they were first written in"
    );
    assert_eq!(
        storage.load("movable:a"),
        Some(geo(20.0, 56.0, 680.0, 440.0))
    );

    storage.clear("movable:a");
    assert_eq!(storage.load("movable:a"), None);
    assert_eq!(storage.len(), 1);
    storage.clear("movable:missing");
    assert_eq!(
        storage.len(),
        1,
        "clearing a key that is not there changes nothing"
    );
}

/// `movable.rs` with every comment line stripped, so the scan reads code
/// rather than the prose that explains why the code is absent.
fn executable_source() -> String {
    include_str!("movable.rs")
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn test_the_module_cannot_name_a_storage_location() {
    let code = executable_source();

    for forbidden in [
        "std::fs",
        "std::env",
        "std::io",
        "APPDATA",
        "USERPROFILE",
        "home_dir",
        "dirs::",
        "PathBuf",
        "File::",
    ] {
        assert!(
            !code.contains(forbidden),
            "`{forbidden}` must never appear in `movable.rs`: the framework \
             never owns a storage location (`RUST-024`, decision `D-2`). \
             Position survives a restart through `GeometryStorage`, which \
             the application implements."
        );
    }

    assert!(
        code.contains("trait GeometryStorage"),
        "the guard is not vacuous: the injected sink must still be the only \
         persistence seam in the module"
    );
}

// ---------------------------------------------------------------------------
// 4. UIF-MOV-001 — the title bar is the only grip
// ---------------------------------------------------------------------------

#[test]
fn test_a_card_opens_below_the_navbar_at_the_default_size() {
    let scenario = Scenario::open("logs");
    assert_eq!(scenario.host.geometry(), geo(0.0, 36.0, 680.0, 440.0));
    assert!(scenario.host.is_reachable());
}

#[test]
fn test_the_title_bar_is_the_top_control_md_of_the_card() {
    let scenario = Scenario::at(200.0, 200.0);
    assert_eq!(
        scenario.host.title_bar(),
        rect(200.0, 200.0, 680.0, TITLE_BAR_HEIGHT)
    );
}

#[test]
fn test_the_trailing_action_zone_is_exactly_two_controls_and_a_gap() {
    let scenario = Scenario::at(200.0, 200.0);
    let zone = scenario.host.trailing_actions();

    let expected_width = ACTION_CONTROL_SIZE * 2.0 + ACTION_GAP;
    assert_eq!(expected_width, 56.0);
    assert_eq!(
        zone,
        rect(
            200.0 + 680.0 - TITLE_BAR_PADDING_HORIZONTAL - expected_width,
            200.0,
            expected_width,
            TITLE_BAR_HEIGHT
        )
    );
    assert!(
        zone.x + zone.width <= 200.0 + 680.0 - TITLE_BAR_PADDING_HORIZONTAL,
        "the exclusion zone stops at the title bar's own padding, not past it"
    );
}

#[test]
fn test_a_press_on_bare_title_bar_starts_a_drag() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let response = scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );

    assert!(matches!(
        response.action,
        MovableAction::DragStarted { geometry } if geometry == scenario.host.geometry()
    ));
    assert!(scenario.host.is_dragging());
    assert!(response.is_drag());
    assert!(response.stays_open());
}

#[test]
fn test_a_press_on_the_grip_glyph_or_the_title_text_still_drags() {
    let mut scenario = Scenario::at(200.0, 200.0);

    for target in [
        SurfaceTarget::TitleGrip,
        SurfaceTarget::Title,
        SurfaceTarget::Summary,
    ] {
        let response = scenario.pointer(PointerPhase::Pressed, pt(400.0, 216.0), target.clone());
        assert!(
            matches!(response.action, MovableAction::DragStarted { .. }),
            "a press on {target:?} inside the draggable title bar must drag"
        );
        scenario
            .host
            .end_drag(&mut scenario.registry, &mut scenario.storage);
    }
}

#[test]
fn test_a_press_in_the_trailing_action_cluster_never_starts_a_drag() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let dock = dock_centre(&scenario.host);
    assert!(scenario.host.trailing_actions().contains(dock));

    let response = scenario.pointer(PointerPhase::Pressed, dock, SurfaceTarget::Dock);
    assert_eq!(
        response.action,
        MovableAction::Activated {
            control: CardControl::Dock
        }
    );
    assert!(!scenario.host.is_dragging(), "UIF-MOV-001 exclusion");
    assert!(!response.is_drag());
}

#[test]
fn test_a_lying_report_cannot_start_a_drag_on_the_close_button() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let close = close_centre(&scenario.host);
    assert!(scenario.host.trailing_actions().contains(close));

    let response = scenario.pointer(PointerPhase::Pressed, close, SurfaceTarget::TitleBar);
    assert_eq!(
        response.action,
        MovableAction::Absorbed {
            zone: Zone::TitleActions
        },
        "geometry outranks the report: the point is on the action cluster, so \
         no drag starts"
    );
    assert!(!scenario.host.is_dragging());
}

#[test]
fn test_a_lying_report_cannot_close_a_card_the_pointer_is_dragging() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let title_text = pt(400.0, 216.0);
    assert!(!scenario.host.trailing_actions().contains(title_text));

    let response = scenario.pointer(PointerPhase::Pressed, title_text, SurfaceTarget::Close);
    assert!(
        matches!(response.action, MovableAction::DragStarted { .. }),
        "the other direction of the same rule: the pointer is on the title, \
         so it drags rather than closes"
    );
    assert!(scenario.host.is_open());
}

#[test]
fn test_a_press_on_the_close_button_closes_the_card() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.focus.focus(&fid("card.filter"));
    let close = close_centre(&scenario.host);

    let response = scenario.pointer(PointerPhase::Pressed, close, SurfaceTarget::Close);
    assert!(matches!(response.action, MovableAction::Closed { .. }));
    assert!(!scenario.host.is_open());
    assert!(!scenario.registry.contains(&sid("logs")));
}

#[test]
fn test_drag_frames_move_the_card_by_the_pointer_delta() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );

    let response = scenario.pointer(
        PointerPhase::Moved,
        pt(430.0, 236.0),
        SurfaceTarget::TitleBar,
    );
    assert_eq!(
        response.action,
        MovableAction::Dragged {
            geometry: geo(230.0, 220.0, 680.0, 440.0),
            delta: vec(30.0, 20.0),
        }
    );

    let response = scenario.pointer(
        PointerPhase::Moved,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    assert_eq!(
        response.action,
        MovableAction::Dragged {
            geometry: geo(200.0, 200.0, 680.0, 440.0),
            delta: vec(-30.0, -20.0),
        },
        "the card tracks the pointer in real time, not a smoothed total"
    );
}

#[test]
fn test_a_drag_past_the_edge_does_not_bank_its_overshoot() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );

    // 1400px right of the origin is past the reachable limit for a
    // 680px card in a 1600px viewport.
    scenario.pointer(
        PointerPhase::Moved,
        pt(1800.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    let pinned = scenario.host.geometry();
    assert_eq!(
        pinned.position.x,
        1600.0 - 120.0,
        "the card is pinned with 120px of title bar showing"
    );

    let response = scenario.pointer(
        PointerPhase::Moved,
        pt(1799.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    let back_one = match response.action {
        MovableAction::Dragged { geometry, .. } => geometry.position.x,
        other => panic!("expected a Dragged frame, got {other:?}"),
    };
    assert_eq!(
        back_one,
        pinned.position.x - 1.0,
        "1px back must move the card 1px, not 0px until the pointer has \
         re-travelled the 1400px of overshoot"
    );
}

#[test]
fn test_a_drag_past_the_navbar_does_not_bank_its_overshoot() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );

    scenario.pointer(
        PointerPhase::Moved,
        pt(400.0, -800.0),
        SurfaceTarget::TitleBar,
    );
    assert_eq!(
        scenario.host.geometry().position.y,
        36.0,
        "the navbar is the top bound"
    );

    let response = scenario.pointer(
        PointerPhase::Moved,
        pt(400.0, -799.0),
        SurfaceTarget::TitleBar,
    );
    let back_one = match response.action {
        MovableAction::Dragged { geometry, .. } => geometry.position.y,
        other => panic!("expected a Dragged frame, got {other:?}"),
    };
    assert_eq!(back_one, 37.0);
}

#[test]
fn test_a_drag_keeps_its_capture_when_the_pointer_leaves_the_card() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );

    let response = scenario.pointer(
        PointerPhase::Moved,
        pt(1500.0, 900.0),
        SurfaceTarget::Outside,
    );
    assert!(
        matches!(response.action, MovableAction::Dragged { .. }),
        "a fast pointer must not outrun the card and drop the gesture"
    );
    assert!(scenario.host.is_capturing());

    let response = scenario.pointer(
        PointerPhase::Released,
        pt(1500.0, 900.0),
        SurfaceTarget::Outside,
    );
    assert!(matches!(response.action, MovableAction::DragEnded { .. }));
    assert!(!scenario.host.is_capturing());
}

#[test]
fn test_releasing_without_a_drag_is_an_idle_no_op() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let response = scenario.pointer(
        PointerPhase::Released,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    assert_eq!(response.action, MovableAction::NoOp(MovableNoOp::Idle));
}

#[test]
fn test_a_moved_event_with_no_gesture_underway_is_idle() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let response = scenario.pointer(
        PointerPhase::Moved,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    assert_eq!(response.action, MovableAction::NoOp(MovableNoOp::Idle));
    assert!(!scenario.host.is_dragging());
}

#[test]
fn test_the_cursor_follows_the_drag_state() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let title = pt(400.0, 216.0);

    assert_eq!(scenario.host.cursor_at(title), mouse::Interaction::Grab);
    scenario.pointer(PointerPhase::Pressed, title, SurfaceTarget::TitleBar);
    assert_eq!(scenario.host.cursor_at(title), mouse::Interaction::Grabbing);
    assert_eq!(scenario.host.cursor(), mouse::Interaction::Grabbing);

    scenario
        .host
        .end_drag(&mut scenario.registry, &mut scenario.storage);
    assert_eq!(scenario.host.cursor(), mouse::Interaction::None);
    assert_eq!(scenario.host.cursor_at(title), mouse::Interaction::Grab);
}

#[test]
fn test_every_pointer_response_carries_the_cursor_task() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let response = scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    assert!(
        response
            .tasks
            .contains(&MovableTask::SetCursor(mouse::Interaction::Grabbing)),
        "the host performs no side effect itself; the caller sets the cursor \
         from the task list"
    );
}

#[test]
fn test_a_whole_gesture_settles_and_persists_once() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .storage
        .save(&scenario.host.storage_key(), geo(1.0, 40.0, 680.0, 440.0));
    let steps = scenario.drag_gesture(pt(400.0, 216.0), &[pt(420.0, 236.0), pt(450.0, 266.0)]);

    assert!(matches!(steps[0].action, MovableAction::DragStarted { .. }));
    assert!(matches!(steps[1].action, MovableAction::Dragged { .. }));
    assert!(matches!(steps[2].action, MovableAction::Dragged { .. }));
    assert!(matches!(steps[3].action, MovableAction::DragEnded { .. }));
    assert_eq!(
        scenario.host.geometry(),
        geo(250.0, 250.0, 680.0, 440.0),
        "the card moved by the pointer's total, 50px on each axis"
    );
    assert_eq!(
        scenario.saved(),
        Some(geo(250.0, 250.0, 680.0, 440.0)),
        "exactly one write, at the release"
    );
}

// ---------------------------------------------------------------------------
// 5. UIF-MOV-002 — resize and reachability is unrepresentable
// ---------------------------------------------------------------------------

#[test]
fn test_the_three_top_handles_have_no_pointer_grip() {
    let scenario = Scenario::at(200.0, 200.0);
    for handle in [
        ResizeHandle::TopLeft,
        ResizeHandle::Top,
        ResizeHandle::TopRight,
    ] {
        assert_eq!(
            scenario.host.resize_grip(handle),
            None,
            "{handle:?} would sit on the title bar, and UIF-MOV-001 gives \
             the whole of that bar to the drag"
        );
    }
}

#[test]
fn test_the_five_body_handles_have_a_grip_band() {
    let scenario = Scenario::at(200.0, 200.0);
    let body = scenario.host.body();
    let g = RESIZE_HANDLE_SIZE;

    assert_eq!(body, rect(200.0, 232.0, 680.0, 408.0));
    for handle in [
        ResizeHandle::BottomLeft,
        ResizeHandle::Bottom,
        ResizeHandle::BottomRight,
        ResizeHandle::Left,
        ResizeHandle::Right,
    ] {
        let grip = scenario
            .host
            .resize_grip(handle)
            .unwrap_or_else(|| panic!("{handle:?} must have a grip on the body"));
        assert!(
            body.x + body.width >= grip.x + grip.width - 0.001
                && body.y + body.height >= grip.y + grip.height - 0.001
                && grip.x >= body.x - 0.001
                && grip.y >= body.y - 0.001,
            "the {handle:?} grip must lie inside the body it is a border of"
        );
        let thin_axis_is_handle_size = if handle.drags_start_x() || handle.drags_end_x() {
            (grip.width - g).abs() < 0.001
        } else {
            (grip.height - g).abs() < 0.001
        };
        assert!(
            thin_axis_is_handle_size,
            "the {handle:?} grip is {g}px thick along the axis it drags"
        );
    }
}

#[test]
fn test_the_corner_grips_are_exactly_handle_size() {
    let scenario = Scenario::at(200.0, 200.0);
    let g = RESIZE_HANDLE_SIZE;

    assert_eq!(
        scenario.host.resize_grip(ResizeHandle::BottomRight),
        Some(rect(200.0 + 680.0 - g, 232.0 + 408.0 - g, g, g))
    );
    assert_eq!(
        scenario.host.resize_grip(ResizeHandle::BottomLeft),
        Some(rect(200.0, 232.0 + 408.0 - g, g, g))
    );
}

#[test]
fn test_zone_at_reports_the_grip_before_the_footer() {
    let scenario = Scenario::at(200.0, 200.0);
    let corner = pt(872.0, 632.0);

    assert!(scenario.host.footer().contains(corner));
    assert_eq!(
        scenario.host.zone_at(corner),
        Zone::Grip(ResizeHandle::BottomRight)
    );
    assert!(scenario.host.is_grip_at(corner));
    assert!(
        !scenario.host.is_draggable_at(corner),
        "a grip is not a drag"
    );
}

#[test]
fn test_each_body_edge_grip_maps_to_its_own_handle() {
    let scenario = Scenario::at(200.0, 200.0);
    let probes = [
        (pt(205.0, 400.0), ResizeHandle::Left),
        (pt(875.0, 400.0), ResizeHandle::Right),
        (pt(500.0, 632.0), ResizeHandle::Bottom),
        (pt(205.0, 632.0), ResizeHandle::BottomLeft),
        (pt(875.0, 632.0), ResizeHandle::BottomRight),
    ];

    for (at, handle) in probes {
        assert_eq!(
            scenario.host.zone_at(at),
            Zone::Grip(handle),
            "{at:?} must read as the {handle:?} grip"
        );
    }
}

#[test]
fn test_a_press_on_a_grip_starts_a_resize_not_a_drag() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let response = scenario.pointer(
        PointerPhase::Pressed,
        pt(872.0, 632.0),
        SurfaceTarget::TitleBar,
    );

    assert_eq!(
        response.action,
        MovableAction::ResizeStarted {
            handle: ResizeHandle::BottomRight
        }
    );
    assert!(!scenario.host.is_dragging());
    assert_eq!(
        scenario.host.resizing_handle(),
        Some(ResizeHandle::BottomRight)
    );
    assert!(response.is_resize());
}

#[test]
fn test_each_handle_moves_the_edge_it_holds_and_leaves_the_other_axis_alone() {
    let cases = [
        (
            ResizeHandle::Right,
            vec(10.0, 999.0),
            rect(200.0, 200.0, 690.0, 440.0),
        ),
        (
            ResizeHandle::Left,
            vec(10.0, 999.0),
            rect(210.0, 200.0, 670.0, 440.0),
        ),
        (
            ResizeHandle::Bottom,
            vec(999.0, 10.0),
            rect(200.0, 200.0, 680.0, 450.0),
        ),
        (
            ResizeHandle::Top,
            vec(999.0, 10.0),
            rect(200.0, 210.0, 680.0, 430.0),
        ),
        (
            ResizeHandle::BottomRight,
            vec(10.0, 10.0),
            rect(200.0, 200.0, 690.0, 450.0),
        ),
        (
            ResizeHandle::BottomLeft,
            vec(10.0, 10.0),
            rect(210.0, 200.0, 670.0, 450.0),
        ),
        (
            ResizeHandle::TopRight,
            vec(10.0, 10.0),
            rect(200.0, 210.0, 690.0, 430.0),
        ),
        (
            ResizeHandle::TopLeft,
            vec(10.0, 10.0),
            rect(210.0, 210.0, 670.0, 430.0),
        ),
    ];

    for (handle, delta, expected) in cases {
        let mut scenario = Scenario::at(200.0, 200.0);
        let action = scenario.host.resize_by(handle, delta);
        assert_eq!(
            action,
            MovableAction::Resized {
                handle,
                geometry: MovableGeometry::new(
                    Point::new(expected.x, expected.y),
                    Size::new(expected.width, expected.height)
                ),
                delta,
            },
            "{handle:?} must move only the edges it drags"
        );
        assert!(scenario.host.is_reachable());
    }
}

#[test]
fn test_a_resize_stops_at_the_size_floor() {
    let mut scenario = Scenario::at(200.0, 200.0);

    scenario
        .host
        .resize_by(ResizeHandle::Right, vec(-1000.0, 0.0));
    assert_eq!(
        scenario.host.geometry().size,
        Size::new(CARD_MIN_WIDTH, 440.0),
        "the width stops at the policy floor rather than inverting"
    );

    scenario
        .host
        .resize_by(ResizeHandle::Bottom, vec(0.0, -1000.0));
    assert_eq!(scenario.host.geometry().size.width, CARD_MIN_WIDTH);
    assert_eq!(scenario.host.geometry().size.height, CARD_MIN_HEIGHT);
    assert!(scenario.host.is_reachable());
}

#[test]
fn test_a_resize_from_the_leading_edge_also_stops_at_the_floor() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .host
        .resize_by(ResizeHandle::Left, vec(1000.0, 0.0));
    assert_eq!(scenario.host.geometry().size.width, CARD_MIN_WIDTH);
    assert_eq!(
        scenario.host.geometry().position.x,
        880.0 - CARD_MIN_WIDTH,
        "the trailing edge is what the drag holds still"
    );

    scenario.host.resize_by(ResizeHandle::Top, vec(0.0, 1000.0));
    assert_eq!(scenario.host.geometry().size.height, CARD_MIN_HEIGHT);
    assert_eq!(scenario.host.geometry().position.y, 640.0 - CARD_MIN_HEIGHT);
}

#[test]
fn test_a_resize_release_persists_the_settled_size() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.pointer(
        PointerPhase::Pressed,
        pt(872.0, 632.0),
        SurfaceTarget::Resize(ResizeHandle::BottomRight),
    );
    scenario.pointer(
        PointerPhase::Moved,
        pt(902.0, 662.0),
        SurfaceTarget::Resize(ResizeHandle::BottomRight),
    );
    let response = scenario.pointer(
        PointerPhase::Released,
        pt(902.0, 662.0),
        SurfaceTarget::Resize(ResizeHandle::BottomRight),
    );

    assert_eq!(
        response.action,
        MovableAction::ResizeEnded {
            handle: ResizeHandle::BottomRight,
            geometry: geo(200.0, 200.0, 710.0, 470.0),
        }
    );
    assert_eq!(scenario.saved(), Some(geo(200.0, 200.0, 710.0, 470.0)));
    assert_eq!(scenario.host.resizing_handle(), None);
}

#[test]
fn test_end_resize_without_an_active_resize_is_idle() {
    let mut scenario = Scenario::at(200.0, 200.0);
    assert_eq!(
        scenario
            .host
            .end_resize(&mut scenario.registry, &mut scenario.storage),
        MovableAction::NoOp(MovableNoOp::Idle)
    );
}

#[test]
fn test_a_programmatic_geometry_is_clamped_and_reports_whether_it_moved() {
    let mut scenario = Scenario::at(200.0, 200.0);

    let legal = scenario.host.set_geometry(
        geo(300.0, 300.0, 680.0, 440.0),
        &mut scenario.registry,
        &mut scenario.storage,
    );
    assert_eq!(
        legal,
        MovableAction::Reclamped {
            geometry: geo(300.0, 300.0, 680.0, 440.0),
            adjusted: false,
        }
    );

    let illegal = scenario.host.set_geometry(
        geo(99_999.0, -99_999.0, 680.0, 440.0),
        &mut scenario.registry,
        &mut scenario.storage,
    );
    match illegal {
        MovableAction::Reclamped { geometry, adjusted } => {
            assert!(adjusted, "the clamp must report that it moved something");
            assert_eq!(geometry, scenario.host.geometry());
            assert!(scenario.host.is_reachable());
        }
        other => panic!("expected Reclamped, got {other:?}"),
    }
}

#[test]
fn test_a_shrinking_viewport_re_clamps_without_resizing_the_card() {
    let mut scenario = Scenario::at(1200.0, 800.0);
    let before = scenario.host.geometry();

    let action = scenario.host.set_viewport(
        rect(0.0, 0.0, 1024.0, 768.0),
        &mut scenario.registry,
        &mut scenario.storage,
    );

    match action {
        MovableAction::Reclamped { geometry, adjusted } => {
            assert!(adjusted);
            assert_eq!(
                geometry.size, before.size,
                "a smaller window must not resize the user's card"
            );
            assert!(scenario.host.is_reachable());
            assert!(geometry.position.x <= 1024.0 - 120.0);
            assert!(geometry.position.y <= 768.0 - TITLE_BAR_HEIGHT);
        }
        other => panic!("expected Reclamped, got {other:?}"),
    }
    assert_eq!(scenario.saved(), Some(scenario.host.geometry()));
}

#[test]
fn test_no_requested_geometry_can_produce_an_unreachable_card() {
    let viewports = [
        rect(0.0, 0.0, 2560.0, 1440.0),
        rect(0.0, 0.0, 1600.0, 1000.0),
        rect(0.0, 0.0, 1024.0, 768.0),
        rect(0.0, 0.0, 800.0, 600.0),
        rect(0.0, 0.0, 640.0, 480.0),
        rect(100.0, 50.0, 1280.0, 900.0),
    ];
    let offsets = [-50_000.0_f32, -1000.0, -1.0, 0.0, 1.0, 1000.0, 50_000.0];
    let sizes = [
        Size::new(1.0, 1.0),
        Size::new(CARD_MIN_WIDTH, CARD_MIN_HEIGHT),
        Size::new(680.0, 440.0),
        Size::new(4000.0, 3000.0),
    ];

    let mut checked = 0usize;
    for view in viewports {
        for size in sizes {
            for dx in offsets {
                for dy in offsets {
                    let mut scenario = Scenario::open_in("logs", view, None);
                    scenario.host.set_geometry(
                        geo(view.x + dx, view.y + dy, size.width, size.height),
                        &mut scenario.registry,
                        &mut scenario.storage,
                    );
                    assert!(
                        scenario.host.is_reachable(),
                        "UIF-MOV-002 broken for view {:?} size {:?} origin ({dx}, {dy}): {:?}",
                        view.size(),
                        size,
                        scenario.host.card()
                    );
                    assert!(
                        scenario.host.card().width >= CARD_MIN_WIDTH.min(size.width) - 0.001,
                        "the clamp must not shrink a card below its own floor \
                         without cause: {:?}",
                        scenario.host.card()
                    );
                    checked += 1;
                }
            }
        }
    }

    assert_eq!(checked, 6 * 4 * 7 * 7, "the sweep must cover every cell");
}

#[test]
fn test_a_card_wider_than_the_viewport_still_shows_its_title_bar() {
    let scenario = Scenario::open_in(
        "logs",
        rect(0.0, 0.0, 640.0, 480.0),
        Some(geo(-2000.0, 100.0, 4000.0, 300.0)),
    );

    assert!(scenario.host.is_reachable());
    let visible = clamp::visible_title_width(scenario.host.card(), scenario.host.viewport());
    assert!(
        visible >= 120.0,
        "at least MIN_VISIBLE_TITLE of title bar must show, got {visible}"
    );
}

// ---------------------------------------------------------------------------
// 6. UIF-MOV-003 — the card owns its inside and nothing outside it
// ---------------------------------------------------------------------------

#[test]
fn test_a_click_outside_the_card_falls_through_to_the_workspace() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let response = scenario.pointer(
        PointerPhase::Pressed,
        pt(50.0, 900.0),
        SurfaceTarget::Outside,
    );

    match response.action {
        MovableAction::PassedThrough { ref route } => {
            assert!(route.reaches_workspace());
            assert_eq!(route.target(), &PointerTarget::Workspace);
            assert_eq!(
                route.passed_through(),
                [ZBand::UtilityOverlay],
                "the trace must show the Band 1 card let the click past"
            );
        }
        other => panic!("expected a pass-through, got {other:?}"),
    }
    assert!(response.passed_through);
    assert!(!scenario.host.is_dragging());
    assert!(scenario.host.is_open());
}

#[test]
fn test_a_scroll_outside_the_card_falls_through_just_like_a_click() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let response = scenario
        .host
        .on_scroll(pt(50.0, 900.0), 120.0, &mut scenario.registry);

    assert!(response.passed_through);
    assert!(matches!(
        response.action,
        MovableAction::PassedThrough { .. }
    ));
    assert_eq!(
        scenario.host.scroll().offset,
        0.0,
        "the card did not scroll"
    );
}

#[test]
fn test_a_point_another_surface_owns_is_not_reported_as_a_pass_through() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.registry.register(
        sid("pane"),
        SurfaceKind::Workspace,
        vec![rect(0.0, 800.0, 800.0, 200.0)],
    );

    let response = scenario.pointer(
        PointerPhase::Pressed,
        pt(100.0, 900.0),
        SurfaceTarget::Outside,
    );
    assert_eq!(
        response.action,
        MovableAction::NoOp(MovableNoOp::OwnedByOtherSurface)
    );
    assert!(!response.passed_through);
}

#[test]
fn test_a_card_not_in_the_registry_can_never_be_routed_to() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.registry.unregister(&sid("logs"));

    let response = scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    assert_eq!(
        response.action,
        MovableAction::NoOp(MovableNoOp::NotRegistered),
        "routing is decided by the registry first, so an unregistered card \
         cannot reach its own classification code"
    );
    assert!(!scenario.host.is_dragging());
}

#[test]
fn test_a_closed_card_answers_nothing() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.host.close(
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );

    let response = scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    assert_eq!(response.action, MovableAction::NoOp(MovableNoOp::NotOpen));

    let scroll = scenario
        .host
        .on_scroll(pt(500.0, 400.0), 10.0, &mut scenario.registry);
    assert_eq!(scroll.action, MovableAction::NoOp(MovableNoOp::NotOpen));
}

#[test]
fn test_a_click_inside_the_card_never_reaches_the_workspace() {
    let scenario = Scenario::at(200.0, 200.0);
    for at in [
        pt(400.0, 216.0),
        pt(500.0, 300.0),
        pt(500.0, 400.0),
        pt(500.0, 630.0),
        pt(205.0, 400.0),
    ] {
        let route = scenario.registry.route_pointer(at);
        assert_eq!(
            route.target(),
            &PointerTarget::Surface(sid("logs")),
            "{at:?} is inside the card and must not be routed past it"
        );
        assert!(route.passed_through().is_empty());
    }
}

/// Two Band-1 cards sharing one registry: `a` registered first, so it
/// starts at the bottom of the band, and `b` on top of it.
struct Pair {
    a: MovableHost,
    b: MovableHost,
    focus: FocusState,
    registry: SurfaceRegistry,
    storage: MemoryGeometryStorage,
}

impl Pair {
    fn new() -> Self {
        let mut focus = FocusState::new();
        let mut registry = SurfaceRegistry::new();
        let mut storage = MemoryGeometryStorage::new();

        let a = MovableHost::open(
            MovableSpec::new("a", "Background Operations Log")
                .with_geometry(geo(900.0, 200.0, 680.0, 440.0)),
            viewport(),
            &mut focus,
            &mut registry,
            &mut storage,
        );
        let b = MovableHost::open(
            MovableSpec::new("b", "Agent Debug Inspector")
                .with_geometry(geo(100.0, 100.0, 680.0, 440.0)),
            viewport(),
            &mut focus,
            &mut registry,
            &mut storage,
        );

        Self {
            a,
            b,
            focus,
            registry,
            storage,
        }
    }

    fn band_order(&self) -> Vec<String> {
        self.registry
            .surfaces_in(ZBand::UtilityOverlay)
            .map(|surface| surface.id().to_string())
            .collect()
    }
}

#[test]
fn test_the_registration_order_puts_the_first_card_at_the_bottom() {
    let pair = Pair::new();
    assert_eq!(pair.band_order(), vec!["a", "b"]);
}

#[test]
fn test_a_click_raises_the_lower_card_to_the_top_of_band_1() {
    let mut pair = Pair::new();
    let at = centre_of(pair.a.title_bar());
    let response = pair.a.on_pointer(
        PointerPhase::Pressed,
        at,
        SurfaceTarget::TitleBar,
        &mut pair.focus,
        &mut pair.registry,
        &mut pair.storage,
    );

    assert!(response.promoted, "Band 1 promotes on click");
    assert_eq!(pair.band_order(), vec!["b", "a"]);
    assert_eq!(
        pair.registry
            .topmost_in(ZBand::UtilityOverlay)
            .map(|surface| surface.id().to_string()),
        Some("a".to_owned())
    );
    assert_eq!(
        pair.registry
            .topmost_in(ZBand::Menu)
            .map(|surface| surface.id().to_string()),
        None,
        "promotion happens inside Band 1 and touches no other band"
    );
}

#[test]
fn test_clicking_the_already_top_card_reports_no_promotion() {
    let mut pair = Pair::new();
    let at = centre_of(pair.b.title_bar());
    let response = pair.b.on_pointer(
        PointerPhase::Pressed,
        at,
        SurfaceTarget::TitleBar,
        &mut pair.focus,
        &mut pair.registry,
        &mut pair.storage,
    );

    assert!(matches!(response.action, MovableAction::DragStarted { .. }));
    assert!(
        !response.promoted,
        "a card already on top of its band must not report a reorder"
    );
    assert_eq!(pair.band_order(), vec!["a", "b"]);
}

#[test]
fn test_promotion_reorders_only_its_own_band() {
    let mut pair = Pair::new();
    pair.registry.register(
        sid("pane"),
        SurfaceKind::Workspace,
        vec![rect(0.0, 800.0, 1600.0, 200.0)],
    );
    let workspace_before = pair
        .registry
        .surfaces_in(ZBand::Workspace)
        .map(|surface| surface.id().to_string())
        .collect::<Vec<_>>();
    let before = pair.registry.len();

    let at = centre_of(pair.a.title_bar());
    let response = pair.a.on_pointer(
        PointerPhase::Pressed,
        at,
        SurfaceTarget::TitleBar,
        &mut pair.focus,
        &mut pair.registry,
        &mut pair.storage,
    );

    assert!(response.promoted);
    assert_eq!(
        pair.registry
            .surfaces_in(ZBand::Workspace)
            .map(|surface| surface.id().to_string())
            .collect::<Vec<_>>(),
        workspace_before,
        "the base band keeps its own stacking"
    );
    assert_eq!(pair.band_order(), vec!["b", "a"]);
    assert_eq!(before, pair.registry.len());
}

#[test]
fn test_a_surface_above_the_card_owns_the_point_and_the_card_never_promotes() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.registry.register(
        sid("menu"),
        SurfaceKind::MenuCard,
        vec![rect(0.0, 0.0, 300.0, 400.0)],
    );
    scenario.registry.register(
        sid("modal"),
        SurfaceKind::Modal,
        vec![rect(600.0, 300.0, 400.0, 300.0)],
    );
    let before = scenario.registry.surfaces().to_vec();

    let response = scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    assert_eq!(
        response.action,
        MovableAction::NoOp(MovableNoOp::OwnedByOtherSurface),
        "Band 3 blocks Band 1 completely; the card never sees the press"
    );
    assert!(!response.promoted);
    assert!(!scenario.host.is_dragging());
    assert_eq!(
        scenario.registry.surfaces(),
        before,
        "the bands above and below are untouched by a click they absorbed"
    );
}

#[test]
fn test_promotion_moves_no_focus_and_changes_no_geometry() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .focus
        .register_group(gid("ws"), vec![FocusMember::new("ws.editor")]);
    scenario.focus.focus(&fid("ws.editor"));
    let geometry = scenario.host.geometry();

    scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    scenario
        .host
        .end_drag(&mut scenario.registry, &mut scenario.storage);

    assert_eq!(scenario.focus.current(), Some(&fid("ws.editor")));
    assert_eq!(
        scenario.host.geometry(),
        geometry,
        "a press alone must not move the card"
    );
}

#[test]
fn test_only_band_1_promotes_on_click() {
    assert!(ZBand::UtilityOverlay.promotes_on_click());
    for band in [ZBand::Workspace, ZBand::Menu, ZBand::Modal] {
        assert!(
            !band.promotes_on_click(),
            "{band:?} must not promote on click; only Band 1 does"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. UIF-MOV-004 — following is a state machine, not a checkbox
// ---------------------------------------------------------------------------

#[test]
fn test_the_follow_matrix_has_exactly_one_following_cell() {
    use AutoScrollSetting::{Disabled, Enabled};

    assert_eq!(
        FollowState::resolve(Enabled, true),
        FollowState::Following,
        "permission on AND already at the bottom"
    );
    assert_eq!(
        FollowState::resolve(Enabled, false),
        FollowState::Paused,
        "the user is reading something above the end"
    );
    assert_eq!(
        FollowState::resolve(Disabled, true),
        FollowState::Paused,
        "permission is off; position is moot"
    );
    assert_eq!(FollowState::resolve(Disabled, false), FollowState::Paused);
}

#[test]
fn test_the_matrix_is_a_pure_function_of_the_two_inputs_the_spec_names() {
    for setting in [AutoScrollSetting::Enabled, AutoScrollSetting::Disabled] {
        for at_bottom in [true, false] {
            assert_eq!(
                FollowState::resolve(setting, at_bottom),
                FollowState::resolve(setting, at_bottom),
                "no clock, no history: the same inputs give the same state"
            );
        }
    }
}

#[test]
fn test_at_bottom_uses_the_xxs_tolerance() {
    let content = 2000.0;
    let view = 350.0;
    let max = content - view;

    assert!(ScrollGeometry::new(max, content, view).at_bottom());
    assert!(ScrollGeometry::new(max - AT_BOTTOM_TOLERANCE, content, view).at_bottom());
    assert!(
        !ScrollGeometry::new(max - AT_BOTTOM_TOLERANCE - 1.0, content, view).at_bottom(),
        "one pixel past the tolerance is not at the bottom"
    );
    assert_eq!(AT_BOTTOM_TOLERANCE, 2.0);
}

#[test]
fn test_an_empty_viewport_counts_as_at_the_bottom() {
    assert!(ScrollGeometry::EMPTY.at_bottom());
    assert!(ScrollGeometry::EMPTY.is_empty());
    assert_eq!(ScrollGeometry::EMPTY.max_offset(), 0.0);

    let fits = ScrollGeometry::new(0.0, 100.0, 350.0);
    assert!(
        fits.at_bottom(),
        "content that fits on screen is at the bottom and must still follow"
    );
}

#[test]
fn test_scroll_geometry_clamps_its_offset_into_range() {
    let g = ScrollGeometry::new(-50.0, 1000.0, 300.0).clamped();
    assert_eq!(g.offset, 0.0);

    let g = ScrollGeometry::new(9_999.0, 1000.0, 300.0).clamped();
    assert_eq!(g.offset, 700.0);

    let dirty = ScrollGeometry::new(f32::NAN, f32::INFINITY, f32::NEG_INFINITY).clamped();
    assert_eq!(dirty, ScrollGeometry::new(0.0, 0.0, 0.0));
}

#[test]
fn test_scrolled_by_clamps_at_both_ends() {
    let g = ScrollGeometry::new(100.0, 1000.0, 300.0);
    assert_eq!(g.scrolled_by(50.0).offset, 150.0);
    assert_eq!(g.scrolled_by(-1_000.0).offset, 0.0);
    assert_eq!(g.scrolled_by(1_000.0).offset, 700.0);
    assert_eq!(g.scrolled_by(f32::NAN).offset, 100.0);
}

#[test]
fn test_at_bottom_now_lands_on_the_maximum_offset() {
    let g = ScrollGeometry::new(10.0, 1000.0, 300.0).at_bottom_now();
    assert_eq!(g.offset, 700.0);
    assert!(g.at_bottom());
}

#[test]
fn test_a_fresh_card_enabled_and_at_the_bottom_is_following() {
    let scenario = Scenario::at(200.0, 200.0);
    assert_eq!(scenario.host.auto_scroll(), AutoScrollSetting::Enabled);
    assert_eq!(scenario.host.follow_state(), FollowState::Following);
    assert_eq!(scenario.host.behind(), 0);
}

#[test]
fn test_scrolling_up_pauses_following() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .host
        .sync_scroll(ScrollGeometry::new(700.0, 1000.0, 300.0));
    assert_eq!(scenario.host.follow_state(), FollowState::Following);

    scenario.host.scroll_by(-100.0);
    assert_eq!(scenario.host.follow_state(), FollowState::Paused);
    assert_eq!(scenario.host.scroll().offset, 600.0);
}

#[test]
fn test_scrolling_back_to_the_bottom_resumes_following() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .host
        .sync_scroll(ScrollGeometry::new(400.0, 1000.0, 300.0));
    assert_eq!(scenario.host.follow_state(), FollowState::Paused);

    scenario.host.scroll_to_bottom();
    assert_eq!(scenario.host.follow_state(), FollowState::Following);
    assert_eq!(scenario.host.scroll().offset, 700.0);
    assert_eq!(scenario.host.behind(), 0, "a resume clears the backlog");
}

#[test]
fn test_an_append_while_following_scrolls_to_the_bottom() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .host
        .sync_scroll(ScrollGeometry::new(700.0, 1000.0, 300.0));

    assert_eq!(
        scenario.host.on_append(3),
        AutoScrollDecision::Follow { entries: 3 }
    );
    assert_eq!(scenario.host.scroll().offset, 700.0);
    assert_eq!(scenario.host.behind(), 0);
}

#[test]
fn test_an_append_while_paused_does_not_move_the_viewport() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .host
        .sync_scroll(ScrollGeometry::new(400.0, 1000.0, 300.0));
    let reading = scenario.host.scroll();
    assert_eq!(scenario.host.follow_state(), FollowState::Paused);

    assert_eq!(
        scenario.host.on_append(5),
        AutoScrollDecision::Hold {
            entries: 5,
            reason: HoldReason::Inspecting,
            behind: 5,
        }
    );
    assert_eq!(
        scenario.host.scroll(),
        reading,
        "the text the user is reading must not move by a single pixel"
    );

    assert_eq!(
        scenario.host.on_append(7),
        AutoScrollDecision::Hold {
            entries: 7,
            reason: HoldReason::Inspecting,
            behind: 12,
        },
        "entries arriving while paused are counted, not dropped"
    );
    assert_eq!(scenario.host.scroll(), reading);
}

#[test]
fn test_a_disabled_setting_holds_for_its_own_reason() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .host
        .sync_scroll(ScrollGeometry::new(700.0, 1000.0, 300.0));
    scenario.host.set_auto_scroll(AutoScrollSetting::Disabled);

    assert_eq!(scenario.host.follow_state(), FollowState::Paused);
    assert_eq!(
        scenario.host.on_append(4),
        AutoScrollDecision::Hold {
            entries: 4,
            reason: HoldReason::Disabled,
            behind: 4,
        },
        "\"you turned this off\" and \"you are reading\" are different \
         affordances and must be distinguishable"
    );
}

#[test]
fn test_turning_the_setting_on_does_not_jump_the_viewport() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .host
        .sync_scroll(ScrollGeometry::new(100.0, 1000.0, 300.0));
    scenario.host.set_auto_scroll(AutoScrollSetting::Enabled);

    assert_eq!(
        scenario.host.scroll().offset,
        100.0,
        "the setting grants permission; it does not move the viewport"
    );
    assert_eq!(scenario.host.follow_state(), FollowState::Paused);
}

#[test]
fn test_following_begins_at_the_next_append_once_the_view_reaches_the_bottom() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .host
        .sync_scroll(ScrollGeometry::new(100.0, 1000.0, 300.0));
    scenario.host.set_auto_scroll(AutoScrollSetting::Enabled);
    scenario.host.scroll_to_bottom();

    assert_eq!(scenario.host.follow_state(), FollowState::Following);
    assert_eq!(
        scenario.host.on_append(1),
        AutoScrollDecision::Follow { entries: 1 }
    );
}

#[test]
fn test_only_the_content_viewport_scrolls_inside_the_card() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .host
        .sync_scroll(ScrollGeometry::new(300.0, 1000.0, 300.0));

    let response =
        scenario
            .host
            .on_scroll(content_centre(&scenario.host), 50.0, &mut scenario.registry);
    assert!(matches!(response.action, MovableAction::Scrolled { .. }));
    assert_eq!(scenario.host.scroll().offset, 350.0);

    let footer = scenario.host.footer();
    for at in [
        centre_of(scenario.host.title_bar()),
        centre_of(scenario.host.filter_bar()),
        centre_of(rect(
            footer.x + RESIZE_HANDLE_SIZE * 2.0,
            footer.y,
            (footer.width - RESIZE_HANDLE_SIZE * 4.0).max(1.0),
            footer.height,
        )),
    ] {
        let mut probe = scenario.host.clone();
        let response = probe.on_scroll(at, 50.0, &mut scenario.registry);
        assert!(
            matches!(response.action, MovableAction::Absorbed { .. }),
            "{:?} must take the scroll event and go nowhere with it",
            probe.zone_at(at)
        );
    }
}

#[test]
fn test_a_scroll_through_the_card_reports_the_resulting_follow_state() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .host
        .sync_scroll(ScrollGeometry::new(700.0, 1000.0, 300.0));

    let response = scenario.host.on_scroll(
        content_centre(&scenario.host),
        -200.0,
        &mut scenario.registry,
    );
    assert_eq!(
        response.action,
        MovableAction::Scrolled {
            follow: FollowState::Paused
        }
    );
    assert_eq!(scenario.host.follow_state(), FollowState::Paused);
}

// ---------------------------------------------------------------------------
// 8. Keyboard — Escape and the unwinding order (G-13)
// ---------------------------------------------------------------------------

#[test]
fn test_band_1_escapes_last_of_the_three_above_it() {
    assert_eq!(ZBand::UtilityOverlay.escape_rank(), 1);
    assert_eq!(ZBand::Modal.escape_rank(), 2);
    assert_eq!(ZBand::Menu.escape_rank(), 3);
    assert!(
        ZBand::Menu.escape_rank() > ZBand::Modal.escape_rank(),
        "UIF-WIN-003 unwinds menus before dialogs even though dialogs sit \
         higher on the screen"
    );
}

#[test]
fn test_escape_closes_a_focused_card() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.focus_card();

    let response = scenario.host.on_key(
        KeyClass::Escape,
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );
    assert!(matches!(response.action, MovableAction::Closed { .. }));
    assert!(!scenario.host.is_open());
}

#[test]
fn test_escape_declines_a_card_that_does_not_hold_focus() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .focus
        .register_group(gid("ws"), vec![FocusMember::new("ws.editor")]);
    scenario.focus.focus(&fid("ws.editor"));
    assert!(!scenario.host.holds_focus(&scenario.focus));

    let response = scenario.host.on_key(
        KeyClass::Escape,
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );
    assert_eq!(
        response.action,
        MovableAction::NoOp(MovableNoOp::NotFocused)
    );
    assert!(
        scenario.host.is_open(),
        "§5 Band 1: Escape does not close unless focused"
    );
}

#[test]
fn test_band_1_traps_no_key_other_than_escape() {
    for class in KeyClass::ALL {
        let mut scenario = Scenario::at(200.0, 200.0);
        scenario
            .focus
            .register_group(gid("ws"), vec![FocusMember::new("ws.editor")]);
        scenario.focus.focus(&fid("ws.editor"));

        let response = scenario.host.on_key(
            class,
            &mut scenario.focus,
            &mut scenario.registry,
            &mut scenario.storage,
        );
        if class == KeyClass::Escape {
            assert_eq!(
                response.action,
                MovableAction::NoOp(MovableNoOp::NotFocused),
                "Escape is the conditional one"
            );
        } else {
            assert_eq!(
                response.action,
                MovableAction::NoOp(MovableNoOp::KeyNotTrapped),
                "a modeless card must not eat the workspace's {class:?}"
            );
        }
    }
}

#[test]
fn test_a_closed_card_answers_escape_with_not_open() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.host.close(
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );

    assert_eq!(
        scenario
            .host
            .unwind_answer(&mut scenario.focus, &mut scenario.storage),
        UnwindAnswer::Declined(DeclineReason::NotOpen)
    );
}

#[test]
fn test_escape_reaches_the_menu_first_and_the_card_stays_open() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.focus_card();
    scenario.registry.register(
        sid("menu"),
        SurfaceKind::MenuCard,
        vec![rect(0.0, 0.0, 300.0, 400.0)],
    );

    let Scenario {
        host,
        focus,
        registry,
        storage,
    } = &mut scenario;
    let card = host.id().clone();
    let outcome = registry.escape(move |surface| {
        if surface.id() == &card {
            host.unwind_answer(focus, storage)
        } else {
            UnwindAnswer::Unwound
        }
    });

    assert_eq!(outcome.consumed_by(), Some(&sid("menu")));
    assert!(scenario.host.is_open(), "the card never steals the key");
    assert!(scenario.registry.contains(&sid("logs")));
}

#[test]
fn test_escape_reaches_the_modal_before_the_card() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.focus_card();
    scenario.registry.register(
        sid("modal"),
        SurfaceKind::Modal,
        vec![rect(400.0, 300.0, 400.0, 300.0)],
    );

    let Scenario {
        host,
        focus,
        registry,
        storage,
    } = &mut scenario;
    let card = host.id().clone();
    let outcome = registry.escape(move |surface| {
        if surface.id() == &card {
            host.unwind_answer(focus, storage)
        } else {
            UnwindAnswer::Dismissed
        }
    });

    assert_eq!(outcome.consumed_by(), Some(&sid("modal")));
    assert!(scenario.host.is_open());
}

#[test]
fn test_once_the_stack_above_is_gone_escape_closes_the_card() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.focus_card();
    scenario.registry.register(
        sid("menu"),
        SurfaceKind::MenuCard,
        vec![rect(0.0, 0.0, 300.0, 400.0)],
    );
    scenario.registry.unregister(&sid("menu"));

    let outcome = scenario.host.escape_via(
        &mut scenario.registry,
        &mut scenario.focus,
        &mut scenario.storage,
    );
    assert_eq!(outcome.consumed_by(), Some(&sid("logs")));
    assert!(outcome.dismissed());
    assert!(!scenario.host.is_open());
}

#[test]
fn test_an_unfocused_escape_reports_the_decline_it_received() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .focus
        .register_group(gid("ws"), vec![FocusMember::new("ws.editor")]);
    scenario.focus.focus(&fid("ws.editor"));

    let response = scenario.host.on_key(
        KeyClass::Escape,
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );
    assert_eq!(
        response.action,
        MovableAction::NoOp(MovableNoOp::NotFocused)
    );
}

// ---------------------------------------------------------------------------
// 8b. The drag tiers on the host (§3.0.4.2, `INC-36`)
// ---------------------------------------------------------------------------
//
// `§3.0.4.2` names this interaction `on_drag_settle`: many `Live` frames,
// exactly one `Commit`, and a `Cancel` that puts the card back where the
// user grabbed it. Group 4 already proves the *geometry* of the drag; what
// this group pins is the *tier discipline* around it — that the revert
// target is the grab, that the commit cannot happen twice, and that
// `Escape` spends itself on the revert instead of closing the card.

#[test]
fn test_escape_during_a_drag_restores_the_grab_geometry_and_keeps_the_card_open() {
    let mut scenario = Scenario::at(120.0, 90.0);
    scenario.focus_card();
    let grabbed = scenario.host.geometry();
    let saved_before = scenario.saved();

    scenario.pointer(
        PointerPhase::Pressed,
        pt(200.0, 100.0),
        SurfaceTarget::TitleBar,
    );
    scenario.pointer(
        PointerPhase::Moved,
        pt(400.0, 300.0),
        SurfaceTarget::TitleBar,
    );
    assert_ne!(
        scenario.host.geometry(),
        grabbed,
        "the drag must actually have moved the card or this proves nothing"
    );

    let answer = scenario
        .host
        .unwind_answer(&mut scenario.focus, &mut scenario.storage);

    assert_eq!(
        answer,
        UnwindAnswer::Unwound,
        "a drag answers Escape by handing the rectangle back, not by closing"
    );
    assert_eq!(
        scenario.host.geometry(),
        grabbed,
        "the card is back where it was grabbed, not where the last frame left it"
    );
    assert!(
        scenario.host.is_open(),
        "the key was spent on the revert; the card is still here"
    );
    assert_eq!(
        scenario.saved(),
        saved_before,
        "a revert persists nothing — the sink already holds this geometry"
    );
}

/// **Defect, fixed.** `unwind_answer` answers `Unwound` for a cancelled drag and
/// the registry reports that as consumed with `dismissed: false`.
/// `escape_outcome` treated *any* consumption by this card as a close, so
/// `on_key(Escape)` during a drag reported `MovableAction::Closed` while the
/// card stayed open — the host was told to tear down a card the user could still
/// see and keep using. The tests above prove the revert through `unwind_answer`
/// directly; this one drives the real key entry point, which nothing covered.
#[test]
fn defect_12_escape_during_a_drag_reports_the_cancel_not_a_close() {
    let mut scenario = Scenario::at(120.0, 90.0);
    scenario.focus_card();
    let grabbed = scenario.host.geometry();

    scenario.pointer(
        PointerPhase::Pressed,
        pt(200.0, 100.0),
        SurfaceTarget::TitleBar,
    );
    scenario.pointer(
        PointerPhase::Moved,
        pt(400.0, 300.0),
        SurfaceTarget::TitleBar,
    );
    assert_ne!(
        scenario.host.geometry(),
        grabbed,
        "precondition: the drag must actually have moved the card"
    );

    let response = scenario.host.on_key(
        KeyClass::Escape,
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );

    assert!(
        matches!(
            response.action,
            MovableAction::NoOp(MovableNoOp::GestureCancelled)
        ),
        "the key cancelled the gesture, so the response must say so"
    );
    assert!(
        !matches!(response.action, MovableAction::Closed { .. }),
        "a cancelled drag must never be reported as a close"
    );
    assert!(
        scenario.host.is_open(),
        "the card is still open, so a `Closed` response would have been a lie"
    );
    assert_eq!(
        scenario.host.geometry(),
        grabbed,
        "and the card sits back at the geometry it was grabbed at"
    );
}

#[test]
fn test_escape_during_a_drag_needs_no_keyboard_focus() {
    let mut scenario = Scenario::at(120.0, 90.0);
    let grabbed = scenario.host.geometry();

    // Deliberately no `focus_card()`: the drag is held by pointer capture,
    // so a card the user never focused still owns the gesture and still
    // owes the revert.
    scenario.pointer(
        PointerPhase::Pressed,
        pt(200.0, 100.0),
        SurfaceTarget::TitleBar,
    );
    scenario.pointer(
        PointerPhase::Moved,
        pt(360.0, 240.0),
        SurfaceTarget::TitleBar,
    );

    let answer = scenario
        .host
        .unwind_answer(&mut scenario.focus, &mut scenario.storage);

    assert_eq!(answer, UnwindAnswer::Unwound, "the drag answers first");
    assert_eq!(scenario.host.geometry(), grabbed);
    assert!(scenario.host.is_open());
}

#[test]
fn test_a_drag_releases_exactly_one_commit_and_a_second_release_is_a_no_op() {
    let mut scenario = Scenario::at(120.0, 90.0);
    scenario.pointer(
        PointerPhase::Pressed,
        pt(200.0, 100.0),
        SurfaceTarget::TitleBar,
    );
    scenario.pointer(
        PointerPhase::Moved,
        pt(400.0, 300.0),
        SurfaceTarget::TitleBar,
    );

    let first = scenario.pointer(
        PointerPhase::Released,
        pt(400.0, 300.0),
        SurfaceTarget::TitleBar,
    );
    assert!(
        matches!(first.action, MovableAction::DragEnded { .. }),
        "the release settles the drag"
    );

    let second = scenario.pointer(
        PointerPhase::Released,
        pt(400.0, 300.0),
        SurfaceTarget::TitleBar,
    );
    assert_eq!(
        second.action,
        MovableAction::NoOp(MovableNoOp::Idle),
        "there is no second `Commit` to spend: one settle per drag"
    );
}

#[test]
fn test_a_settled_drag_leaves_no_revert_target_behind() {
    let mut scenario = Scenario::at(120.0, 90.0);
    scenario.focus_card();
    scenario.drag_gesture(pt(200.0, 100.0), &[pt(400.0, 300.0)]);
    let settled = scenario.host.geometry();

    let answer = scenario
        .host
        .unwind_answer(&mut scenario.focus, &mut scenario.storage);

    assert_eq!(
        answer,
        UnwindAnswer::Dismissed,
        "with the drag committed, Escape is the ordinary close again"
    );
    assert_eq!(
        scenario.host.geometry(),
        settled,
        "the settled position stands: a `Commit` cannot be reverted afterwards"
    );
}

#[test]
fn test_drag_by_without_a_press_still_owns_a_revert_target() {
    let mut scenario = Scenario::at(120.0, 90.0);
    scenario.focus_card();
    let before = scenario.host.geometry();

    // A host that drives its own pointer events may call `drag_by` with no
    // press having gone through `on_pointer`. The gesture opens on the
    // first frame, snapshot taken before the delta is applied.
    let frame = scenario.host.drag_by(Vector::new(80.0, 60.0));
    assert!(
        matches!(frame, MovableAction::Dragged { .. }),
        "the direct call still produces the frame the caller expects"
    );
    assert_ne!(scenario.host.geometry(), before);

    let answer = scenario
        .host
        .unwind_answer(&mut scenario.focus, &mut scenario.storage);

    assert_eq!(answer, UnwindAnswer::Unwound);
    assert_eq!(
        scenario.host.geometry(),
        before,
        "the snapshot was taken before the first delta, so the revert is exact"
    );
}

#[test]
fn test_closing_mid_drag_aborts_the_gesture_without_restoring() {
    let mut scenario = Scenario::at(120.0, 90.0);
    scenario.pointer(
        PointerPhase::Pressed,
        pt(200.0, 100.0),
        SurfaceTarget::TitleBar,
    );
    scenario.pointer(
        PointerPhase::Moved,
        pt(400.0, 300.0),
        SurfaceTarget::TitleBar,
    );
    let mid_drag = scenario.host.geometry();

    let response = scenario.host.close(
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );
    assert!(
        matches!(response.action, MovableAction::Closed { .. }),
        "the close still reports itself as a close"
    );

    assert_eq!(
        scenario.saved(),
        Some(mid_drag),
        "a card going away mid-drag persists where it stands; nobody owed a `Commit`"
    );
    assert!(!scenario.host.is_open());
}

// ---------------------------------------------------------------------------
// 9. Lifecycle — open, close, reopen, roster rebuild
// ---------------------------------------------------------------------------

#[test]
fn test_open_registers_the_surface_and_its_group_but_focuses_nothing() {
    let scenario = Scenario::open("logs");

    assert!(scenario.host.is_open());
    assert_eq!(scenario.host.kind(), SurfaceKind::MovableCard);
    assert_eq!(scenario.host.band(), ZBand::UtilityOverlay);
    assert!(scenario.registry.contains(&sid("logs")));
    assert_eq!(
        scenario.registry.surfaces_in(ZBand::UtilityOverlay).count(),
        1,
        "the card registers exactly one surface"
    );
    assert_eq!(
        scenario.focus.current(),
        None,
        "a modeless card that grabbed the keyboard on open would contradict \
         the only reason it is not a modal"
    );
}

#[test]
fn test_open_registers_the_declared_controls() {
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let mut storage = MemoryGeometryStorage::new();

    let spec = MovableSpec::new("logs", "Logs").with_controls(vec![
        FocusMember::new("card.filter"),
        FocusMember::disabled("card.copy"),
    ]);
    let host = MovableHost::open(spec, viewport(), &mut focus, &mut registry, &mut storage);

    let group = focus
        .group_of(&fid("card.filter"))
        .expect("the card's controls must be registered");
    assert_eq!(group.id(), host.group_id());
    assert_eq!(group.members().len(), 2);
    assert!(!group.enabled_members().any(|m| m.id == fid("card.copy")));
}

#[test]
fn test_closing_unregisters_the_surface_and_the_roster() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .host
        .sync_members(vec![FocusMember::new("card.filter")], &mut scenario.focus);

    scenario.host.close(
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );

    assert!(!scenario.registry.contains(&sid("logs")));
    assert!(focus_group_gone(&scenario.focus, &fid("card.filter")));
}

fn focus_group_gone(focus: &FocusState, member: &FocusId) -> bool {
    focus.group_of(member).is_none()
}

#[test]
fn test_reopen_restores_the_geometry_the_card_closed_with() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.host.drag_by(vec(70.0, -40.0));
    let left = scenario.host.geometry();

    scenario.host.close(
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );
    let response = scenario.host.reopen(
        viewport(),
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );

    assert!(scenario.host.is_open());
    assert_eq!(
        scenario.host.geometry(),
        left,
        "the card did not move while it was shut"
    );
    assert!(scenario.registry.contains(&sid("logs")));
    assert!(response.stays_open());
}

#[test]
fn test_reopen_brings_back_the_latest_roster_not_the_one_it_opened_with() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.host.sync_members(
        vec![FocusMember::new("card.a"), FocusMember::new("card.b")],
        &mut scenario.focus,
    );
    scenario.host.close(
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );

    scenario.host.reopen(
        viewport(),
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );
    let group = scenario
        .focus
        .group_of(&fid("card.a"))
        .expect("the rebuilt roster must come back with the card");
    assert_eq!(group.members().len(), 2);
    assert_eq!(group.id(), scenario.host.group_id());
}

#[test]
fn test_reopen_re_clamps_against_a_viewport_that_shrank_while_it_was_shut() {
    let mut scenario = Scenario::at(1200.0, 800.0);
    scenario.host.close(
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );

    scenario.host.reopen(
        rect(0.0, 0.0, 1024.0, 768.0),
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );
    assert!(scenario.host.is_reachable());
    assert_eq!(scenario.host.viewport(), rect(0.0, 0.0, 1024.0, 768.0));
}

#[test]
fn test_reopen_on_an_open_card_is_a_no_op() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let geometry = scenario.host.geometry();

    let response = scenario.host.reopen(
        viewport(),
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );
    assert_eq!(
        response.action,
        MovableAction::NoOp(MovableNoOp::AlreadyOpen)
    );
    assert_eq!(scenario.host.geometry(), geometry);
}

#[test]
fn test_close_on_a_closed_card_is_a_no_op() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.host.close(
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );

    let response = scenario.host.close(
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );
    assert_eq!(response.action, MovableAction::NoOp(MovableNoOp::NotOpen));
}

#[test]
fn test_sync_members_does_not_move_focus_it_did_not_ask_for() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .focus
        .register_group(gid("ws"), vec![FocusMember::new("ws.editor")]);
    scenario.focus.focus(&fid("ws.editor"));

    scenario.host.sync_members(
        vec![FocusMember::new("card.a"), FocusMember::new("card.b")],
        &mut scenario.focus,
    );
    assert_eq!(scenario.focus.current(), Some(&fid("ws.editor")));

    scenario.focus.focus(&fid("card.b"));
    scenario
        .host
        .sync_members(vec![FocusMember::new("card.a")], &mut scenario.focus);
    assert_ne!(
        scenario.focus.current(),
        Some(&fid("card.b")),
        "the member that held focus was removed, so focus keeps its slot \
         elsewhere — but it must not land back on the card by accident"
    );
}

// ---------------------------------------------------------------------------
// 10. The frame — determinism, and the router through the shared harness
// ---------------------------------------------------------------------------

/// One scripted interaction sequence, run identically twice.
fn script(
    host: &mut MovableHost,
    focus: &mut FocusState,
    registry: &mut SurfaceRegistry,
    storage: &mut MemoryGeometryStorage,
) {
    host.on_pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
        focus,
        registry,
        storage,
    );
    host.on_pointer(
        PointerPhase::Moved,
        pt(437.0, 251.0),
        SurfaceTarget::TitleBar,
        focus,
        registry,
        storage,
    );
    host.on_pointer(
        PointerPhase::Released,
        pt(437.0, 251.0),
        SurfaceTarget::TitleBar,
        focus,
        registry,
        storage,
    );
    host.sync_scroll(ScrollGeometry::new(500.0, 1200.0, 350.0));
    host.scroll_by(-120.0);
    host.on_append(9);
    host.set_auto_scroll(AutoScrollSetting::Disabled);
    host.resize_by(ResizeHandle::BottomRight, vec(25.0, -15.0));
    host.on_scroll(pt(500.0, 400.0), 30.0, registry);
}

#[test]
fn test_the_same_script_produces_the_same_state_every_time() {
    let mut first = Scenario::at(200.0, 200.0);
    let mut second = Scenario::at(200.0, 200.0);

    script(
        &mut first.host,
        &mut first.focus,
        &mut first.registry,
        &mut first.storage,
    );
    script(
        &mut second.host,
        &mut second.focus,
        &mut second.registry,
        &mut second.storage,
    );

    assert_eq!(first.host, second.host);
    assert_eq!(first.storage, second.storage);
    assert!(first.host.is_reachable());
}

#[test]
fn test_a_card_never_ends_a_script_unreachable_or_capturing() {
    for start in [
        geo(-5000.0, -5000.0, 680.0, 440.0),
        geo(5000.0, 5000.0, 680.0, 440.0),
        geo(200.0, 200.0, 1.0, 1.0),
        geo(200.0, 200.0, 5000.0, 4000.0),
    ] {
        let mut scenario = Scenario::open_in("logs", viewport(), Some(start));
        script(
            &mut scenario.host,
            &mut scenario.focus,
            &mut scenario.registry,
            &mut scenario.storage,
        );
        assert!(
            scenario.host.is_reachable(),
            "UIF-MOV-002 broke from {start:?}"
        );
        assert!(
            !scenario.host.is_capturing(),
            "a released gesture must not stay captured"
        );
    }
}

#[test]
fn test_an_empty_registry_escapes_to_nothing_rather_than_panicking() {
    let mut registry = SurfaceRegistry::new();
    let outcome = registry.escape(|_| UnwindAnswer::Unwound);
    assert_eq!(
        outcome.refusal(),
        Some(&EscapeRefusal::EmptyStack),
        "the defined no-op at the bottom of the stack"
    );
    assert!(
        outcome
            .refusal()
            .is_some_and(|refusal| refusal.is_empty_stack())
    );
}

#[test]
fn test_a_card_that_declines_is_recorded_in_the_decline_trace() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario
        .focus
        .register_group(gid("ws"), vec![FocusMember::new("ws.editor")]);
    scenario.focus.focus(&fid("ws.editor"));

    let outcome = scenario.host.escape_via(
        &mut scenario.registry,
        &mut scenario.focus,
        &mut scenario.storage,
    );
    assert!(!outcome.is_consumed());
    assert!(outcome.declined_before().contains(&Declination {
        surface: sid("logs"),
        reason: DeclineReason::NotFocused,
    }));
}

#[test]
fn test_the_router_agrees_that_opening_a_card_moves_no_focus() {
    const ROWS: &[(&str, bool)] = &[
        ("ws.editor", true),
        ("card.filter", true),
        ("card.autoscroll", true),
        ("card.copy", true),
    ];

    let mut harness = Harness::headless(|| mixed_focusable_tree(ROWS));
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let mut storage = MemoryGeometryStorage::new();

    focus.register_group(gid("ws"), vec![FocusMember::new("ws.editor")]);
    focus.focus(&fid("ws.editor"));

    let spec = MovableSpec::new("logs", "Background Operations Log").with_controls(vec![
        FocusMember::new("card.filter"),
        FocusMember::new("card.autoscroll"),
        FocusMember::new("card.copy"),
    ]);
    let host = MovableHost::open(spec, viewport(), &mut focus, &mut registry, &mut storage);
    assert!(host.is_open());
    assert_eq!(focus.current(), Some(&fid("ws.editor")));

    harness.sync(&focus);
    assert_agree(&mut harness, &focus, "opening the card");
    assert_ne!(harness.focused(), Some(iced_id("card.filter")));
}

#[test]
fn test_the_card_has_no_trap_so_the_workspace_stays_reachable_by_keyboard() {
    const ROWS: &[(&str, bool)] = &[
        ("ws.editor", true),
        ("card.filter", true),
        ("card.copy", true),
    ];

    let mut harness = Harness::headless(|| mixed_focusable_tree(ROWS));
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let mut storage = MemoryGeometryStorage::new();

    let spec = MovableSpec::new("logs", "Logs").with_controls(vec![
        FocusMember::new("card.filter"),
        FocusMember::new("card.copy"),
    ]);
    let _host = MovableHost::open(spec, viewport(), &mut focus, &mut registry, &mut storage);

    focus.register_group(gid("ws"), vec![FocusMember::new("ws.editor")]);
    assert!(
        focus.focus(&fid("card.filter")),
        "the card's controls must be focusable"
    );
    harness.sync(&focus);
    assert_agree(&mut harness, &focus, "a click on the card's filter field");

    assert!(
        focus.focus(&fid("ws.editor")),
        "a Band 1 card confines nothing: the user can leave it with the \
         keyboard, which is the whole difference from a modal"
    );
    harness.sync(&focus);
    assert_agree(&mut harness, &focus, "returning to the workspace");
}

#[test]
fn test_the_router_agrees_with_the_model_after_the_card_closes() {
    const ROWS: &[(&str, bool)] = &[
        ("ws.editor", true),
        ("card.filter", true),
        ("card.copy", true),
    ];

    let mut harness = Harness::headless(|| mixed_focusable_tree(ROWS));
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let mut storage = MemoryGeometryStorage::new();

    focus.register_group(gid("ws"), vec![FocusMember::new("ws.editor")]);
    let spec = MovableSpec::new("logs", "Logs").with_controls(vec![
        FocusMember::new("card.filter"),
        FocusMember::new("card.copy"),
    ]);
    let mut host = MovableHost::open(spec, viewport(), &mut focus, &mut registry, &mut storage);

    assert!(focus.focus(&fid("card.copy")));
    harness.sync(&focus);
    assert_agree(&mut harness, &focus, "focusing a card control");

    host.close(&mut focus, &mut registry, &mut storage);
    assert!(
        focus
            .current()
            .map(|id| id.as_str() != "card.copy")
            .unwrap_or(true),
        "closing the card must not leave focus on a control that no longer exists"
    );
    harness.sync(&focus);
    assert_agree(&mut harness, &focus, "closing the card");
    assert_ne!(harness.focused(), Some(iced_id("card.copy")));
}

#[test]
fn test_the_router_sees_the_roster_after_a_rebuild() {
    const ROWS: &[(&str, bool)] = &[
        ("ws.editor", true),
        ("card.filter", true),
        ("card.copy", true),
    ];

    let mut harness = Harness::headless(|| mixed_focusable_tree(ROWS));
    let mut focus = FocusState::new();
    let mut registry = SurfaceRegistry::new();
    let mut storage = MemoryGeometryStorage::new();

    let spec =
        MovableSpec::new("logs", "Logs").with_controls(vec![FocusMember::new("card.filter")]);
    let mut host = MovableHost::open(spec, viewport(), &mut focus, &mut registry, &mut storage);

    host.sync_members(
        vec![
            FocusMember::new("card.filter"),
            FocusMember::new("card.copy"),
        ],
        &mut focus,
    );
    assert!(focus.focus(&fid("card.copy")));
    harness.sync(&focus);
    assert_agree(
        &mut harness,
        &focus,
        "a content rebuild that added a control",
    );

    let seen = harness
        .registered()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    assert!(
        seen.contains(&iced_id("card.copy")),
        "the router must see the control the rebuild added; saw {seen:?}"
    );
}

// ---------------------------------------------------------------------------
// Response shape
// ---------------------------------------------------------------------------

#[test]
fn test_a_drag_response_is_a_drag_and_keeps_the_card_open() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let response = scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    assert!(response.is_drag());
    assert!(!response.is_resize());
    assert!(response.stays_open());
}

#[test]
fn test_a_resize_response_is_a_resize_and_keeps_the_card_open() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let response = scenario.pointer(
        PointerPhase::Pressed,
        pt(872.0, 632.0),
        SurfaceTarget::Resize(ResizeHandle::BottomRight),
    );
    assert!(response.is_resize());
    assert!(!response.is_drag());
    assert!(response.stays_open());
}

#[test]
fn test_a_closed_response_is_neither_drag_nor_resize() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let close = close_centre(&scenario.host);

    let response = scenario.pointer(PointerPhase::Pressed, close, SurfaceTarget::Close);
    assert!(!response.is_drag());
    assert!(!response.is_resize());
    assert!(!response.stays_open());
}

// ---------------------------------------------------------------------------
// 11. DEFECT — the surface index follows the card
// ---------------------------------------------------------------------------

/// The rectangle the surface index believes the card occupies.
///
/// The index is a *derived* index of the host's geometry, not a second
/// source of truth, so after any gesture that moves the card this must
/// equal `host.card()`. Every `defect_` case below is a different way of
/// proving that equality holds — or, before the fix, of proving it did not.
fn indexed_rect(registry: &SurfaceRegistry, name: &str) -> Rectangle {
    let surface = registry
        .get(&sid(name))
        .unwrap_or_else(|| panic!("the card {name:?} must still be registered"));

    assert_eq!(
        surface.regions().len(),
        1,
        "[{name}] a movable card publishes exactly one region, got {:?}",
        surface.regions()
    );
    surface.regions()[0]
}

/// `UIF-MOV-003` says the registry decides what a card owns, so a card that
/// moved without republishing its rectangle owns the ground it left and is a
/// stranger where it landed. This is the drag form of that defect: the index
/// is checked against the host, and the point that proves the drift is the
/// centre of the card's *new* home — a point the stale index routes past the
/// card to the workspace.
#[test]
fn defect_01_a_dragged_card_owns_the_ground_it_moved_to() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let before = scenario.host.card();
    assert_eq!(
        before,
        rect(200.0, 200.0, 680.0, 440.0),
        "[defect_01] precondition: the card opens where the fixture put it"
    );

    scenario.drag_gesture(pt(400.0, 216.0), &[pt(700.0, 516.0)]);
    let after = scenario.host.card();

    assert_eq!(
        after,
        rect(500.0, 500.0, 680.0, 440.0),
        "[defect_01] the drag must actually have moved the card, or the \
         index claim below proves nothing"
    );
    assert_eq!(
        indexed_rect(&scenario.registry, "logs"),
        after,
        "[defect_01] after a drag the surface index must hold the card's NEW \
         rectangle; it still holds the pre-drag {before:?}"
    );
    assert_eq!(
        scenario.registry.route_pointer(centre_of(after)).target(),
        &PointerTarget::Surface(sid("logs")),
        "[defect_01] the centre of the ground the card moved onto must route \
         to the card, not fall through to the workspace"
    );
}

/// The other half of the same defect, and the half the user feels: the ground
/// the card vacated keeps resolving to it, so a surface that was always
/// underneath stays unreachable there. A workspace-band pane is placed under
/// the card to make "unreachable" observable rather than rhetorical.
#[test]
fn defect_02_the_ground_a_card_vacated_stops_belonging_to_it() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.registry.register(
        sid("pane"),
        SurfaceKind::Workspace,
        vec![rect(0.0, 0.0, 1600.0, 1000.0)],
    );
    let vacated = centre_of(scenario.host.card());

    // Underneath the card, the pane cannot be reached — that is correct, and
    // the precondition the rest of the case is measured against.
    assert_eq!(
        scenario.registry.route_pointer(vacated).target(),
        &PointerTarget::Surface(sid("logs")),
        "[defect_02] precondition: before the drag the card covers this \
         point and outranks the pane"
    );

    scenario.drag_gesture(pt(400.0, 216.0), &[pt(700.0, 516.0)]);

    assert_eq!(
        scenario.registry.route_pointer(vacated).target(),
        &PointerTarget::Surface(sid("pane")),
        "[defect_02] the card has left this point: the pane beneath it must \
         become reachable again instead of being shadowed by a card that is \
         no longer there"
    );
}

/// The resize form of the defect: growing the card onto new ground never
/// handed that ground to the card, so the newly covered area kept routing
/// past it.
#[test]
fn defect_03_a_resized_card_owns_the_ground_it_grew_onto() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.pointer(
        PointerPhase::Pressed,
        pt(872.0, 632.0),
        SurfaceTarget::Resize(ResizeHandle::BottomRight),
    );
    scenario.pointer(
        PointerPhase::Moved,
        pt(1072.0, 832.0),
        SurfaceTarget::Resize(ResizeHandle::BottomRight),
    );
    let released = scenario.pointer(
        PointerPhase::Released,
        pt(1072.0, 832.0),
        SurfaceTarget::Resize(ResizeHandle::BottomRight),
    );
    assert!(
        matches!(released.action, MovableAction::ResizeEnded { .. }),
        "[defect_03] the release must settle the resize or the index claim \
         below proves nothing"
    );

    let after = scenario.host.card();
    assert_eq!(
        after,
        rect(200.0, 200.0, 880.0, 640.0),
        "[defect_03] the resize must actually have grown the card"
    );
    assert_eq!(
        indexed_rect(&scenario.registry, "logs"),
        after,
        "[defect_03] after a resize the surface index must hold the card's \
         NEW rectangle"
    );

    let newly_covered = pt(1000.0, 780.0);
    assert!(
        scenario.host.is_inside_card(newly_covered),
        "[defect_03] precondition: {newly_covered:?} must be inside the \
         grown card"
    );
    assert_eq!(
        scenario.registry.route_pointer(newly_covered).target(),
        &PointerTarget::Surface(sid("logs")),
        "[defect_03] {newly_covered:?} is ground the card grew onto and \
         must belong to it"
    );
}

/// Corner case: the pointer is released far outside the viewport. The card
/// itself is clamped back inside, and the index must publish the clamped
/// rectangle — not the off-screen position the pointer ended at, and not the
/// position the drag started from.
#[test]
fn defect_04_a_drag_released_outside_the_viewport_still_settles_the_index() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let before = scenario.host.card();

    scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    scenario.pointer(
        PointerPhase::Moved,
        pt(1200.0, 900.0),
        SurfaceTarget::TitleBar,
    );
    let released = scenario.pointer(
        PointerPhase::Released,
        pt(2400.0, 1800.0),
        SurfaceTarget::Outside,
    );
    assert!(
        matches!(released.action, MovableAction::DragEnded { .. }),
        "[defect_04] a release outside the viewport still ends the drag: \
         pointer capture keeps the gesture"
    );

    let after = scenario.host.card();
    assert!(
        scenario.host.is_reachable(),
        "[defect_04] the clamp must have kept the card reachable"
    );
    assert_ne!(
        after, before,
        "[defect_04] the card must have moved or the case proves nothing"
    );
    assert_eq!(
        indexed_rect(&scenario.registry, "logs"),
        after,
        "[defect_04] the index must publish the clamped rectangle the card \
         settled at"
    );
    assert_eq!(
        scenario.registry.route_pointer(centre_of(after)).target(),
        &PointerTarget::Surface(sid("logs")),
        "[defect_04] the card must be grabbable where it actually is"
    );
}

/// Corner case: press and release without moving. Nothing moved, so the index
/// must be byte-for-byte what it was — the fix must not turn a no-op release
/// into a re-publish that changes anything.
#[test]
fn defect_05_a_zero_movement_drag_leaves_the_index_exactly_where_it_was() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let untouched = scenario.host.card();
    scenario.registry.register(
        sid("pane"),
        SurfaceKind::Workspace,
        vec![rect(0.0, 0.0, 1600.0, 1000.0)],
    );

    scenario.drag_gesture(pt(400.0, 216.0), &[]);

    assert_eq!(
        scenario.host.card(),
        untouched,
        "[defect_05] a drag with no move must not move the card"
    );
    assert_eq!(
        indexed_rect(&scenario.registry, "logs"),
        untouched,
        "[defect_05] the index must still hold the same rectangle"
    );
    assert_eq!(
        scenario
            .registry
            .route_pointer(centre_of(untouched))
            .target(),
        &PointerTarget::Surface(sid("logs")),
        "[defect_05] the card must still own its own centre"
    );
}

/// Corner case: a resize driven past the size floor. The card stops at the
/// policy minimum, and the index must publish that minimum rectangle rather
/// than the opening one.
#[test]
fn defect_06_a_resize_held_at_the_size_floor_publishes_the_minimum_rect() {
    let mut scenario = Scenario::at(200.0, 200.0);

    scenario.pointer(
        PointerPhase::Pressed,
        pt(872.0, 632.0),
        SurfaceTarget::Resize(ResizeHandle::BottomRight),
    );
    scenario.pointer(
        PointerPhase::Moved,
        pt(100.0, 100.0),
        SurfaceTarget::Resize(ResizeHandle::BottomRight),
    );
    scenario.pointer(
        PointerPhase::Released,
        pt(100.0, 100.0),
        SurfaceTarget::Resize(ResizeHandle::BottomRight),
    );
    let settled = scenario.host.card();

    assert_eq!(
        settled.size(),
        Size::new(CARD_MIN_WIDTH, CARD_MIN_HEIGHT),
        "[defect_06] the card must stop exactly at the policy floor"
    );
    assert_eq!(
        indexed_rect(&scenario.registry, "logs"),
        settled,
        "[defect_06] the index must publish the floored rectangle, not the \
         680x440 the card opened with"
    );
    assert_eq!(
        scenario.registry.route_pointer(centre_of(settled)).target(),
        &PointerTarget::Surface(sid("logs")),
        "[defect_06] the card must own its own floored centre"
    );
    assert_eq!(
        scenario.registry.route_pointer(pt(860.0, 620.0)).target(),
        &PointerTarget::Workspace,
        "[defect_06] the ground the card shrank away from must stop being \
         the card's; the stale 680x440 index still claims it"
    );
}

/// Corner case: a card dragged on top of a second card. The card that moved
/// must end up owning the overlap (the press promoted it), the card beneath
/// must stay reachable wherever it is still uncovered, and the ground the
/// moving card vacated must belong to neither.
#[test]
fn defect_07_a_card_dragged_onto_another_card_leaves_the_one_beneath_reachable() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.registry.register(
        sid("debug"),
        SurfaceKind::MovableCard,
        vec![rect(700.0, 450.0, 400.0, 300.0)],
    );
    scenario.registry.register(
        sid("pane"),
        SurfaceKind::Workspace,
        vec![rect(0.0, 0.0, 1600.0, 1000.0)],
    );

    scenario.drag_gesture(pt(400.0, 216.0), &[pt(760.0, 560.0)]);
    let moved = scenario.host.card();
    assert_eq!(
        moved,
        rect(560.0, 544.0, 680.0, 440.0),
        "[defect_07] the card must land overlapping the second card"
    );

    // The overlap: the press promoted `logs`, so it is the topmost Band 1
    // surface over this point and must answer for it.
    let overlap = pt(900.0, 600.0);
    assert!(
        scenario.host.is_inside_card(overlap) && rect(700.0, 450.0, 400.0, 300.0).contains(overlap),
        "[defect_07] precondition: {overlap:?} must sit in both cards"
    );
    assert_eq!(
        scenario.registry.route_pointer(overlap).target(),
        &PointerTarget::Surface(sid("logs")),
        "[defect_07] the card that moved on top must own the overlap"
    );

    // The card beneath, where the moved card does not cover it.
    let uncovered = pt(750.0, 470.0);
    assert!(
        !scenario.host.is_inside_card(uncovered),
        "[defect_07] precondition: {uncovered:?} must be clear of the \
         moved card"
    );
    assert_eq!(
        scenario.registry.route_pointer(uncovered).target(),
        &PointerTarget::Surface(sid("debug")),
        "[defect_07] the card beneath must still be reachable where it is \
         not covered"
    );

    // The ground the moved card vacated belongs to neither card.
    assert_eq!(
        scenario.registry.route_pointer(pt(300.0, 300.0)).target(),
        &PointerTarget::Surface(sid("pane")),
        "[defect_07] the vacated ground must fall through to the pane, not \
         stay claimed by the card that left it"
    );
}

/// Corner case: the drag is rolled back with `Escape` instead of committed.
/// The card returns to the rectangle it was grabbed at, and the index must
/// agree with the restored card. That agreement is only possible because a
/// drag's frames never published a rectangle in the first place, so there is
/// nothing for the rollback to undo in the index.
#[test]
fn defect_08_a_cancelled_drag_leaves_the_index_with_the_restored_card() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let grabbed = scenario.host.card();

    scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    scenario.pointer(
        PointerPhase::Moved,
        pt(700.0, 516.0),
        SurfaceTarget::TitleBar,
    );
    assert_ne!(
        scenario.host.card(),
        grabbed,
        "[defect_08] the drag must have moved the card before it is undone"
    );

    let outcome = scenario.host.escape_via(
        &mut scenario.registry,
        &mut scenario.focus,
        &mut scenario.storage,
    );
    assert!(outcome.is_consumed(), "[defect_08] the key was spent");
    assert!(
        !outcome.dismissed(),
        "[defect_08] the card handed the rectangle back, it did not close"
    );
    assert_eq!(
        scenario.host.card(),
        grabbed,
        "[defect_08] the card is back where it was grabbed"
    );
    assert_eq!(
        indexed_rect(&scenario.registry, "logs"),
        grabbed,
        "[defect_08] after a rollback the index must hold the restored \
         rectangle"
    );
    assert_eq!(
        scenario.registry.route_pointer(centre_of(grabbed)).target(),
        &PointerTarget::Surface(sid("logs")),
        "[defect_08] the restored card must own its own centre again"
    );
}

/// The same rollback reached through the chain's answer directly, which is
/// how the escape walk calls it. Called on its own — with no caller to repair
/// anything — it must leave the index still agreeing with the card, and must
/// neither add a region nor drop the card's own.
#[test]
fn defect_09_unwinding_the_answer_alone_never_touches_the_registry() {
    let mut scenario = Scenario::at(200.0, 200.0);
    let grabbed = scenario.host.card();

    scenario.pointer(
        PointerPhase::Pressed,
        pt(400.0, 216.0),
        SurfaceTarget::TitleBar,
    );
    scenario.pointer(
        PointerPhase::Moved,
        pt(700.0, 516.0),
        SurfaceTarget::TitleBar,
    );

    let answer = scenario
        .host
        .unwind_answer(&mut scenario.focus, &mut scenario.storage);

    assert_eq!(
        answer,
        UnwindAnswer::Unwound,
        "[defect_09] an open drag answers the unwind with a revert"
    );
    assert_eq!(
        indexed_rect(&scenario.registry, "logs"),
        scenario.host.card(),
        "[defect_09] whatever the answer does to the card, the index and \
         the card must still agree"
    );
    assert_eq!(
        scenario.host.card(),
        grabbed,
        "[defect_09] and the card is where it was grabbed"
    );
}

/// The direct gesture API — the entry points a host that drives its own
/// pointer events uses instead of [`on_pointer`](MovableHost::on_pointer) —
/// owes the same invariant: every settle republishes the rectangle.
#[test]
fn defect_10_a_settled_gesture_publishes_through_the_direct_api() {
    let mut scenario = Scenario::at(200.0, 200.0);

    scenario.host.drag_by(vec(150.0, 90.0));
    scenario
        .host
        .end_drag(&mut scenario.registry, &mut scenario.storage);
    assert_eq!(
        indexed_rect(&scenario.registry, "logs"),
        scenario.host.card(),
        "[defect_10] `end_drag` must publish the rectangle it settled on"
    );

    scenario
        .host
        .resize_by(ResizeHandle::BottomRight, vec(60.0, 40.0));
    scenario
        .host
        .end_resize(&mut scenario.registry, &mut scenario.storage);
    assert_eq!(
        indexed_rect(&scenario.registry, "logs"),
        scenario.host.card(),
        "[defect_10] `end_resize` must publish the rectangle it settled on"
    );

    scenario.host.set_geometry(
        geo(60.0, 80.0, 680.0, 440.0),
        &mut scenario.registry,
        &mut scenario.storage,
    );
    assert_eq!(
        indexed_rect(&scenario.registry, "logs"),
        scenario.host.card(),
        "[defect_10] `set_geometry` must publish the rectangle it moved to"
    );
    assert_eq!(
        scenario
            .registry
            .route_pointer(centre_of(scenario.host.card()))
            .target(),
        &PointerTarget::Surface(sid("logs")),
        "[defect_10] and the card must be routable where it was placed"
    );
}

/// Failure mode for the publish guard: a card that is no longer open must
/// never be put back into the index by a stray settle. `close` unregisters,
/// and every later geometry call has to respect that.
#[test]
fn defect_11_a_closed_card_is_never_published_back_into_the_index() {
    let mut scenario = Scenario::at(200.0, 200.0);
    scenario.host.close(
        &mut scenario.focus,
        &mut scenario.registry,
        &mut scenario.storage,
    );
    assert!(
        !scenario.registry.contains(&sid("logs")),
        "[defect_11] precondition: closing unregisters the card"
    );

    scenario.host.set_geometry(
        geo(300.0, 300.0, 680.0, 440.0),
        &mut scenario.registry,
        &mut scenario.storage,
    );
    scenario
        .host
        .end_drag(&mut scenario.registry, &mut scenario.storage);
    scenario
        .host
        .end_resize(&mut scenario.registry, &mut scenario.storage);

    assert!(
        !scenario.registry.contains(&sid("logs")),
        "[defect_11] a settled gesture on a closed card must not resurrect \
         its surface"
    );
    assert_eq!(
        scenario.registry.route_pointer(pt(400.0, 400.0)).target(),
        &PointerTarget::Workspace,
        "[defect_11] the closed card's old ground must be the workspace's"
    );
}
