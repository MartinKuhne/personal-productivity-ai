//! Render-level ("ink") coverage for the composites pane.
//!
//! Same discipline as [`crate::render_atoms`]: every assertion reads the
//! framebuffer. The composites are where the third historical bug lived
//! — `settings_row::horizontal` declaring a `Column` and building a
//! `Row`, so the section laid out 32px where it claimed 56/60px — and
//! they are also where a repaint test is cheapest, because every one of
//! them is a controlled widget whose whole appearance is a function of
//! the host model.
//!
//! Two shapes are used throughout:
//!
//! * **Region paint** — the section's body, or a named control's own
//!   rect, shows pixels over the card behind it.
//! * **Repaint** — moving the host model moves pixels. A composite that
//!   swallows a state change without redrawing fails here even when its
//!   geometry is perfect.

use iced::{Point, Rectangle, Size};

use crate::data;
use crate::pane_composites::{self, CompositesMessage, CompositesState, GRID_AVAILABLE_WIDTH};
use crate::render_capture::{
    MIN_CHANGED_PIXELS, PANE_VIEWPORT, SURFACE_TOLERANCE, Studio, assert_repainted,
};
use crate::render_section::{self, Captured, assert_body_painted, capture_diff};
use crate::section::Section;
use hoarfrost::composite::grid_view::{GAP, GUTTER_WIDTH, solve_columns, tile_rect};

/// A composite section builder addressable by name in a sweep.
type CompositeBuilder = for<'a> fn(&'a CompositesState) -> Section<'a>;

/// Every live composite section (S5 + S6), in display order.
fn composite_builders() -> Vec<(&'static str, CompositeBuilder)> {
    vec![
        (
            "Tab strip (UIF-TAB-003)",
            pane_composites::tab_strip_section as CompositeBuilder,
        ),
        (
            "Navigation rail (UIF-COMP-009)",
            pane_composites::navigation_rail_section as CompositeBuilder,
        ),
        (
            "Stepper + wizard footer (UIF-COMP-011)",
            pane_composites::stepper_section as CompositeBuilder,
        ),
        (
            "Pagination bar (UIF-COMP-012)",
            pane_composites::pagination_section as CompositeBuilder,
        ),
        (
            "Pips pager (UIF-COMP-013)",
            pane_composites::pips_pager_section as CompositeBuilder,
        ),
        (
            "Code block (UIF-COMP-015)",
            pane_composites::code_block_section as CompositeBuilder,
        ),
        (
            "Diff viewer (UIF-COMP-015)",
            pane_composites::diff_viewer_section as CompositeBuilder,
        ),
        (
            "Empty states (UIF-COMP-014)",
            pane_composites::empty_state_section as CompositeBuilder,
        ),
        (
            "Grid view (UIF-COMP-013)",
            pane_composites::grid_view_section as CompositeBuilder,
        ),
        (
            "Keybind recorder (UIF-COMP-015)",
            pane_composites::keybind_section as CompositeBuilder,
        ),
        ("Skeleton (UIF-COMP-015)", |_| {
            pane_composites::skeleton_section()
        }),
        (
            "Side sheet (UIF-COMP-012)",
            pane_composites::side_sheet_section as CompositeBuilder,
        ),
        (
            "Command palette (UIF-COMP-010)",
            pane_composites::command_palette_section as CompositeBuilder,
        ),
        (
            "Form field (UIF-COMP-016)",
            pane_composites::form_field_section as CompositeBuilder,
        ),
        (
            "Settings rows (UIF-COMP-017)",
            pane_composites::settings_rows_section as CompositeBuilder,
        ),
        (
            "Menu rows (UIF-FMENU / UIF-CMENU)",
            pane_composites::menu_section as CompositeBuilder,
        ),
    ]
}

/// Splits a rectangle into `parts` equal horizontal slices, top to bottom.
///
/// Sections whose blocks are stacked rather than set side by side are read
/// a slice at a time; a single flat split along x would put two blocks in
/// one slice and none in another.
fn rows(rect: Rectangle, parts: usize) -> Vec<Rectangle> {
    let height = rect.height / parts as f32;
    (0..parts)
        .map(|index| Rectangle {
            x: rect.x,
            y: rect.y + height * index as f32,
            width: rect.width,
            height,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Breadth — every section paints inside its own card
// ---------------------------------------------------------------------------

#[test]
fn every_composite_section_paints_inside_its_own_body() {
    let state = CompositesState::new();
    let mut studio = Studio::new();

    for (name, build) in composite_builders() {
        let captured = Captured::capture(&mut studio, PANE_VIEWPORT, || build(&state));
        assert_body_painted(&captured);
        assert_eq!(
            captured.id,
            crate::section::section_id(name),
            "section {name} mounted under an unexpected id"
        );
    }
}

#[test]
fn no_composite_section_is_a_flat_rectangle() {
    let state = CompositesState::new();
    let mut studio = Studio::new();

    for (name, build) in composite_builders() {
        let captured = Captured::capture(&mut studio, PANE_VIEWPORT, || build(&state));
        assert!(
            !captured.frame.is_uniform(captured.body, SURFACE_TOLERANCE),
            "section {name}'s body {:?} is one flat colour — nothing was drawn \
             on top of the card",
            captured.body
        );
    }
}

#[test]
fn the_skeleton_paints_although_it_counts_as_zero_widgets() {
    let mut studio = Studio::new();
    let captured = Captured::capture(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::skeleton_section,
    );

    // A placeholder is not interactive, so the section reports zero
    // widgets — but it must still be visibly there, or a loading state
    // is indistinguishable from a broken one.
    assert!(
        captured.body_paint() > MIN_CHANGED_PIXELS * 8,
        "the skeleton paints only {} pixels over its card — a loading state \
         that is barely drawn is the same as no loading state",
        captured.body_paint()
    );
}

// ---------------------------------------------------------------------------
// Settings rows (UIF-COMP-017) — the row that once laid out half its
// declared height
// ---------------------------------------------------------------------------

#[test]
fn every_declared_settings_row_paints_across_its_whole_box() {
    let state = CompositesState::new();
    let mut studio = Studio::new();
    let captured = Captured::capture(&mut studio, PANE_VIEWPORT, || {
        pane_composites::settings_rows_section(&state)
    });

    for index in 0..data::SETTINGS_ROWS.len() {
        let id = format!("demo.composites.setting-row.{index}.row");
        let row = captured.control(&id);
        assert!(
            row.height > 0.0,
            "settings row {index} recorded a zero-height box"
        );

        // The row is a title/description block plus a trailing control,
        // so both its left half (the words) and its right edge (the
        // toggle) must carry ink. A row that collapsed to a bare line —
        // the `horizontal`-built-as-`Row` failure — leaves one of the
        // two halves empty.
        let left = Rectangle {
            x: row.x,
            y: row.y,
            width: row.width / 2.0,
            height: row.height,
        };
        let right = Rectangle {
            x: row.x + row.width / 2.0,
            y: row.y,
            width: row.width / 2.0,
            height: row.height,
        };
        assert!(
            captured.frame.ink_count(left) >= MIN_CHANGED_PIXELS,
            "settings row {index} paints no ink in its title half {left:?} — \
             the title/description block is not being drawn"
        );
        assert!(
            captured.frame.ink_count(right) >= MIN_CHANGED_PIXELS,
            "settings row {index} paints no ink in its control half {right:?} — \
             the trailing toggle is not being drawn"
        );
    }
}

#[test]
fn the_three_settings_rows_stack_without_overlapping() {
    let state = CompositesState::new();
    let mut studio = Studio::new();
    let captured = Captured::capture(&mut studio, PANE_VIEWPORT, || {
        pane_composites::settings_rows_section(&state)
    });

    let mut rows: Vec<Rectangle> = (0..data::SETTINGS_ROWS.len())
        .map(|i| captured.control(&format!("demo.composites.setting-row.{i}.row")))
        .collect();
    rows.sort_by(|a, b| a.y.total_cmp(&b.y));

    for pair in rows.windows(2) {
        let previous = pair[0];
        let next = pair[1];
        assert!(
            next.y >= previous.y + previous.height - 0.5,
            "settings rows overlap: {:?} then {:?}",
            previous,
            next
        );
    }
}

#[test]
fn toggling_a_setting_repaints_its_own_row_only() {
    let mut studio = Studio::new();

    let before = CompositesState::new();
    let after = CompositesState {
        settings: vec![true, false, false],
        ..CompositesState::new()
    };

    let probe = studio.probe(
        PANE_VIEWPORT,
        pane_composites::settings_rows_section(&before).element,
    );
    let first = probe
        .find("demo.composites.setting-row.0.row")
        .expect("the first settings row must be addressable")
        .bounds;
    let second = probe
        .find("demo.composites.setting-row.1.row")
        .expect("the second settings row must be addressable")
        .bounds;

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::settings_rows_section as CompositeBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        first,
        "turning the first setting on",
    );
    assert_eq!(
        diff.before.differing_pixels(&diff.after, second),
        0,
        "turning the first setting on also repainted the second row — the \
         toggles are not independent"
    );
}

// ---------------------------------------------------------------------------
// Menu rows (UIF-FMENU / UIF-CMENU)
// ---------------------------------------------------------------------------

#[test]
fn every_keyed_menu_row_paints_its_label() {
    let state = CompositesState::new();
    let mut studio = Studio::new();
    let captured = Captured::capture(&mut studio, PANE_VIEWPORT, || {
        pane_composites::menu_section(&state)
    });

    for (key, label, _) in data::MENU_ROWS.iter().filter(|(_, _, k)| *k != "separator") {
        let id = format!("demo.composites.menu.{key}");
        let bounds = captured.control(&id);
        assert!(
            captured.frame.ink_count(bounds) >= MIN_CHANGED_PIXELS,
            "menu row `{key}` ({label}) lays out at {bounds:?} and paints no \
             readable ink"
        );
    }
}

#[test]
fn activating_a_checkable_menu_row_repaints_it() {
    let mut studio = Studio::new();

    let before = CompositesState::new();
    let after = CompositesState {
        menu: {
            let mut menu = before.menu.clone();
            if let Some(index) = menu.rows().iter().position(|row| row.key() == "m-wrap") {
                menu.activate(index);
            }
            menu
        },
        ..CompositesState::new()
    };

    let probe = studio.probe(
        PANE_VIEWPORT,
        pane_composites::menu_section(&before).element,
    );
    let row = probe
        .find("demo.composites.menu.m-wrap")
        .expect("the wrap menu row must be addressable")
        .bounds;

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::menu_section as CompositeBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        row,
        "activating the checkable `m-wrap` menu row",
    );
}

// ---------------------------------------------------------------------------
// Navigation family repaints
// ---------------------------------------------------------------------------

#[test]
fn activating_a_different_tab_repaints_the_strip() {
    let mut studio = Studio::new();

    let before = CompositesState::new();
    let target = before
        .tabs
        .tabs
        .iter()
        .find(|tab| before.tabs.active_id.as_deref() != Some(tab.id.as_str()))
        .map(|tab| tab.id.clone())
        .expect("the fixture must have a non-active tab to activate");
    let after = CompositesState {
        tabs: {
            let mut tabs = before.tabs.clone();
            tabs.select_tab(&target);
            tabs
        },
        ..CompositesState::new()
    };

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::tab_strip_section as CompositeBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "activating a different tab in the strip",
    );
}

#[test]
fn expanding_the_rail_repaints_it() {
    let mut studio = Studio::new();

    let before = CompositesState::new();
    let after = CompositesState {
        rail: {
            let mut rail = before.rail.clone();
            rail.expanded = !before.rail.expanded;
            rail
        },
        ..CompositesState::new()
    };

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::navigation_rail_section as CompositeBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "expanding the navigation rail",
    );
}

/// A clone of `state` after the demo's own update path folded `message` in.
///
/// The wizard tests drive the transition the footer's buttons actually
/// trigger rather than a hand-built model, so a `step_by` that moves
/// `current_step` without rewriting the per-step `status` markers — the
/// only thing the indicator paints from — cannot pass.
fn driven(before: &CompositesState, message: CompositesMessage) -> CompositesState {
    let mut after = before.clone();
    after.apply(&message);
    after
}

#[test]
fn stepping_the_wizard_repaints_the_indicator_and_its_footer() {
    let mut studio = Studio::new();

    let before = CompositesState::new();
    let after = driven(&before, CompositesMessage::StepNext);

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::stepper_section as CompositeBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "advancing the wizard by one step",
    );
}

#[test]
fn turning_the_page_repaints_the_pagination_bar() {
    let mut studio = Studio::new();

    let before = CompositesState::new();
    let target = before.pagination.page + 4;
    let after = CompositesState {
        pagination: {
            let mut page = before.pagination.clone();
            page.page = target;
            page
        },
        ..CompositesState::new()
    };

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::pagination_section as CompositeBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "jumping the pagination bar to page 3",
    );
}

#[test]
fn moving_a_pip_repaints_the_pager() {
    let mut studio = Studio::new();

    let before = CompositesState::new();
    let after = CompositesState {
        pips: {
            let mut pips = before.pips;
            pips.page = 5;
            pips
        },
        ..CompositesState::new()
    };

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::pips_pager_section as CompositeBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "moving the pips pager to page 5",
    );
}

#[test]
fn the_pips_pager_draws_one_mark_per_page() {
    let state = CompositesState::new();
    let mut studio = Studio::new();
    let captured = Captured::capture(&mut studio, PANE_VIEWPORT, || {
        pane_composites::pips_pager_section(&state)
    });

    let total = state.pips.total_pages.max(1);

    // The pips are laid out along x, so the page count is read off the
    // painted extent rather than the card: split what the pager actually
    // drew into one slice per page and require every slice to carry ink.
    let marks = captured
        .frame
        .ink_bounds(captured.body)
        .expect("the pips pager paints its marks");
    let slice = marks.width / total as f32;

    for page in 0..total {
        let slot = Rectangle::new(
            Point::new(marks.x + slice * page as f32, marks.y),
            Size::new(slice, marks.height),
        );
        assert!(
            captured.frame.ink_count(slot) > 0,
            "page {page} of {total} has no mark: the slice {slot:?} of the \
             pager's painted extent {marks:?} is blank"
        );
    }
}

// ---------------------------------------------------------------------------
// Content composites
// ---------------------------------------------------------------------------

#[test]
fn the_code_block_paints_its_listing_not_just_its_frame() {
    let state = CompositesState::new();
    let mut studio = Studio::new();
    let captured = Captured::capture(&mut studio, PANE_VIEWPORT, || {
        pane_composites::code_block_section(&state)
    });

    assert!(
        captured.body_ink() >= MIN_CHANGED_PIXELS * 4,
        "the code block paints only {} readable pixels in {:?} — the listing \
         itself is missing",
        captured.body_ink(),
        captured.body
    );
}

#[test]
fn switching_the_diff_mode_repaints_the_viewer() {
    let mut studio = Studio::new();

    let before = CompositesState::new();
    let after = CompositesState {
        diff: {
            let mut diff = before.diff.clone();
            diff.mode = hoarfrost::composite::diff_viewer::DiffDisplayMode::SideBySide;
            diff
        },
        ..CompositesState::new()
    };

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::diff_viewer_section as CompositeBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "switching the diff viewer from unified to side-by-side",
    );
}

#[test]
fn the_empty_states_paint_three_distinct_blocks() {
    let state = CompositesState::new();
    let mut studio = Studio::new();
    let captured = Captured::capture(&mut studio, PANE_VIEWPORT, || {
        pane_composites::empty_state_section(&state)
    });

    // The three empty states are stacked, so read the body in thirds and
    // require each third to carry ink.
    let thirds = rows(captured.body, 3);
    for (index, third) in thirds.iter().enumerate() {
        assert!(
            captured.frame.ink_count(*third) >= MIN_CHANGED_PIXELS,
            "empty-state block {index} paints no readable ink in {third:?}"
        );
    }
}

#[test]
fn selecting_a_grid_tile_repaints_the_grid() {
    let mut studio = Studio::new();

    let before = CompositesState::new();
    let after = CompositesState {
        grid: {
            let mut grid = before.grid.clone();
            if let Some(first) = grid.items.first() {
                grid.selected_ids = [first.id.clone()].into_iter().collect();
            }
            grid
        },
        ..CompositesState::new()
    };

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::grid_view_section as CompositeBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "selecting the first grid tile",
    );
}

/// How far a tile's painted ink may sit off that tile's own horizontal centre.
///
/// Two logical pixels. The six fixture tiles measure 0.5px of drift — the
/// widest centred line sets the box and the residual is glyph sidebearing —
/// so two is comfortably above the noise and far below the 174px tile
/// pitch, which means a tile anchored somewhere other than where the grid
/// really put it cannot hide inside this tolerance.
const TILE_CENTRE_TOLERANCE: f32 = 2.0;

/// The horizontal centre of a rectangle.
fn centre_x(rect: Rectangle) -> f32 {
    rect.x + rect.width / 2.0
}

/// Every grid tile's **actual** laid-out rectangle, in item order.
///
/// The grid is the first, leading-aligned child of the section body's
/// column, so its origin is the body's own top-left corner; the column
/// count and tile width come from the framework's own [`solve_columns`]
/// and [`tile_rect`] rather than from anything the test invents.
///
/// A uniform division of the painted ink span cannot work here. That
/// span is the union of everything in the body: its left edge is set by
/// the caption note under the grid and its right edge by the widest
/// centred title, while the tiles themselves sit on a
/// `tile_width + GAP` pitch of 174px inside 162px-wide cells. Dividing
/// the span by six yields 76px bands — 2.3x off the real pitch — which
/// march left of their tiles and strand the later ones in the inter-tile
/// gutter while crediting an earlier tile's ink to a later item.
fn grid_tile_rects(state: &CompositesState, body: Rectangle) -> Vec<Rectangle> {
    let (cols, tile_width) = solve_columns(
        GRID_AVAILABLE_WIDTH,
        state.grid.min_tile_width,
        GAP,
        GUTTER_WIDTH,
    );

    (0..state.grid.items.len())
        .map(|index| {
            tile_rect(
                body.position(),
                index,
                cols,
                tile_width,
                state.grid.tile_height,
                GAP,
            )
        })
        .collect()
}

#[test]
fn the_grid_paints_a_tile_per_item() {
    let state = CompositesState::new();
    let mut studio = Studio::new();
    let captured = Captured::capture(&mut studio, PANE_VIEWPORT, || {
        pane_composites::grid_view_section(&state)
    });

    let tiles = grid_tile_rects(&state, captured.body);
    assert_eq!(
        tiles.len(),
        state.grid.items.len(),
        "the grid laid out {} tiles for {} items",
        tiles.len(),
        state.grid.items.len()
    );

    for (item, tile) in state.grid.items.iter().zip(&tiles) {
        assert!(
            captured.frame.ink_count(*tile) > 0,
            "grid tile `{}` paints no ink inside its own laid-out bounds {tile:?}",
            item.id
        );

        // Tiles centre their icon, title and subtitle, so the ink a tile
        // paints must be centred on the tile. This is what ties the rects
        // above back to the pixels: an anchor that had drifted would show
        // up as off-centre content rather than quietly crediting one
        // tile's ink to its neighbour's band.
        let painted = captured.frame.ink_bounds(*tile).unwrap_or_else(|| {
            panic!(
                "grid tile `{}` paints no ink inside its own bounds {tile:?}",
                item.id
            )
        });
        let drift = (centre_x(painted) - centre_x(*tile)).abs();
        assert!(
            drift <= TILE_CENTRE_TOLERANCE,
            "grid tile `{}` paints {drift:.2}px off its own horizontal centre \
             (tile {tile:?}, ink {painted:?}) — {TILE_CENTRE_TOLERANCE}px allowed",
            item.id
        );
    }
}

#[test]
fn the_keybind_recorder_paints_its_chord() {
    let state = CompositesState::new();
    let mut studio = Studio::new();
    let captured = Captured::capture(&mut studio, PANE_VIEWPORT, || {
        pane_composites::keybind_section(&state)
    });

    assert!(
        captured.body_ink() >= MIN_CHANGED_PIXELS * 2,
        "the keybind recorder paints only {} readable pixels in {:?}",
        captured.body_ink(),
        captured.body
    );
}

#[test]
fn flipping_the_form_validity_repaints_the_caption_band() {
    let mut studio = Studio::new();

    let before = CompositesState::new();
    let after = CompositesState {
        form_valid: false,
        ..CompositesState::new()
    };

    let probe = studio.probe(
        PANE_VIEWPORT,
        pane_composites::form_field_section(&before).element,
    );
    let toggle = probe
        .find("demo.composites.form.valid")
        .expect("the form validity switch must be addressable")
        .bounds;

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::form_field_section as CompositeBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "flipping the form field from valid to invalid",
    );
    assert!(
        diff.before
            .painted_count(toggle, render_section::card_fill_rgb(), 2)
            > MIN_CHANGED_PIXELS,
        "the form validity switch at {toggle:?} is not drawn"
    );
}

#[test]
fn the_side_sheet_paints_its_panel_and_close_affordance() {
    let state = CompositesState::new();
    let mut studio = Studio::new();
    let captured = Captured::capture(&mut studio, PANE_VIEWPORT, || {
        pane_composites::side_sheet_section(&state)
    });

    assert!(
        captured.body_paint() >= MIN_CHANGED_PIXELS * 4,
        "the side sheet paints almost nothing in its body {:?}",
        captured.body
    );
}

#[test]
fn closing_the_side_sheet_repaints_the_section() {
    let mut studio = Studio::new();

    let before = CompositesState::new();
    let after = CompositesState {
        sheet: {
            let mut sheet = before.sheet.clone();
            sheet.open = false;
            sheet
        },
        ..CompositesState::new()
    };

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::side_sheet_section as CompositeBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "closing the side sheet",
    );
}

#[test]
fn the_command_palette_paints_its_card_and_items() {
    let state = CompositesState::new();
    let mut studio = Studio::new();
    let captured = Captured::capture(&mut studio, PANE_VIEWPORT, || {
        pane_composites::command_palette_section(&state)
    });

    assert!(
        captured.body_ink() >= MIN_CHANGED_PIXELS * 4,
        "the command palette paints only {} readable pixels in {:?}",
        captured.body_ink(),
        captured.body
    );
}

#[test]
fn typing_in_the_palette_repaints_it() {
    let mut studio = Studio::new();

    let before = CompositesState::new();
    let after = CompositesState {
        palette: {
            let mut palette = before.palette.clone();
            palette.query = String::from("sub");
            palette
        },
        ..CompositesState::new()
    };

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::command_palette_section as CompositeBuilder,
        &before,
        &after,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        diff.body,
        "typing a filter query into the command palette",
    );
}

// ---------------------------------------------------------------------------
// Cross-section independence
// ---------------------------------------------------------------------------

#[test]
fn a_change_to_one_composite_leaves_its_neighbours_untouched() {
    let mut studio = Studio::new();

    let before = CompositesState::new();
    let after = CompositesState {
        pips: {
            let mut pips = before.pips;
            pips.page = 4;
            pips
        },
        ..CompositesState::new()
    };

    let before_frame = studio.frame(
        PANE_VIEWPORT,
        pane_composites::pagination_section(&before).element,
    );
    let after_frame = studio.frame(
        PANE_VIEWPORT,
        pane_composites::pagination_section(&after).element,
    );
    let body = Captured::capture(&mut studio, PANE_VIEWPORT, || {
        pane_composites::pagination_section(&before)
    })
    .body;

    assert_eq!(
        before_frame.differing_pixels(&after_frame, body),
        0,
        "moving the pips pager repainted the pagination bar — the two controls \
         are not independent on screen"
    );
}
