//! Tests for the secret-entry and multi-line gallery cells
//! (`UIF-ATOM-002c`, `UIF-ATOM-002e`).

use crate::gallery::{Cell, RUNGS, StyleValue, snapshot_states};
use crate::gallery_secret::{
    password_reveal_cells, password_state_cells, password_tier_cells, secret_cells,
    text_area_growth_cells, text_area_state_cells, text_area_tier_cells,
};

/// Look up a resolved entry by channel and render it as text.
fn resolved(cell: &Cell, channel: &str) -> String {
    let entry = cell
        .resolved
        .iter()
        .find(|entry| entry.channel == channel)
        .unwrap_or_else(|| panic!("cell {} has no channel {channel}", cell.id));

    match &entry.value {
        StyleValue::Text(value) => value.clone(),
        StyleValue::Number(value) => value.to_string(),
        StyleValue::Colour { token, .. } => token.clone(),
    }
}

#[test]
fn test_every_cell_has_a_unique_id() {
    let cells = secret_cells();
    let mut ids: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "two cells share an id");
}

#[test]
fn test_every_cell_carries_at_least_one_contrast_pair() {
    for cell in secret_cells() {
        assert!(!cell.audit.is_empty(), "cell {} audits nothing", cell.id);
    }
}

#[test]
fn test_the_password_state_cells_cover_every_reachable_state() {
    let cells = password_state_cells();
    assert_eq!(
        cells.len(),
        snapshot_states().len() * RUNGS.len(),
        "every reachable state at both rungs"
    );
    for cell in &cells {
        assert_eq!(cell.spec, "UIF-ATOM-002c");
        assert_eq!(
            resolved(cell, "slot.width"),
            "24",
            "the reveal toggle did not reserve the hit-target floor"
        );
    }
}

#[test]
fn test_the_reveal_matrix_has_one_cell_per_posture() {
    let cells = password_reveal_cells();
    assert_eq!(cells.len(), 6);
    for cell in &cells {
        assert!(cell.id.starts_with("password/reveal/"));
    }
}

#[test]
fn test_only_the_focused_reveal_case_actually_reveals() {
    let cells = password_reveal_cells();
    let revealed: Vec<&str> = cells
        .iter()
        .filter(|cell| resolved(cell, "reveal.state") == "true")
        .map(|cell| cell.id.as_str())
        .collect();
    assert_eq!(revealed, vec!["password/reveal/revealed-focused"]);
}

#[test]
fn test_every_refusal_case_recorded_a_refusal() {
    for cell in &password_reveal_cells() {
        let refused = cell.id.contains("refused");
        let refusals: u32 = resolved(cell, "reveal.refusals").parse().expect("a count");
        assert_eq!(
            refusals > 0,
            refused,
            "cell {} reported refusals={refusals}",
            cell.id
        );
    }
}

#[test]
fn test_no_reveal_cell_ever_leaks_the_sample_secret() {
    for cell in &password_reveal_cells() {
        assert_eq!(resolved(cell, "reveal.debug-redacted"), "true");
    }
}

#[test]
fn test_no_reveal_mirror_is_clearable() {
    for cell in &password_reveal_cells() {
        assert_eq!(resolved(cell, "reveal.mirror-clearable"), "false");
    }
}

#[test]
fn test_the_reveal_does_not_move_the_partition() {
    let cells = password_reveal_cells();
    let area = |id: &str| -> String {
        resolved(
            cells.iter().find(|c| c.id == id).expect("a cell"),
            "geometry.text-area",
        )
    };
    let slot = |id: &str| -> String {
        resolved(
            cells.iter().find(|c| c.id == id).expect("a cell"),
            "geometry.trailing-slot",
        )
    };

    assert_eq!(
        area("password/reveal/masked-focused"),
        area("password/reveal/revealed-focused")
    );
    assert_eq!(
        slot("password/reveal/masked-focused"),
        slot("password/reveal/revealed-focused")
    );
}

#[test]
fn test_the_password_tier_cells_cover_every_tier_the_control_produces() {
    let cells = password_tier_cells();
    assert_eq!(cells.len(), 6);
    let tiers: Vec<String> = cells
        .iter()
        .map(|cell| resolved(cell, "tier-class"))
        .collect();
    assert!(tiers.iter().any(|t| t == "live"));
    assert!(tiers.iter().any(|t| t == "commit"));
    assert!(tiers.iter().any(|t| t == "cancel"));
    assert!(tiers.iter().any(|t| t == "reveal"));
    assert!(tiers.iter().any(|t| t == "focus"));
}

#[test]
fn test_no_password_tier_rendering_contains_the_plaintext_shape() {
    for cell in &password_tier_cells() {
        let rendering = resolved(cell, "emitted-rendering");
        assert!(
            !rendering.contains("supersecret"),
            "tier {} rendered a plaintext secret",
            cell.id
        );
    }
}

#[test]
fn test_the_submit_tier_does_not_settle_the_session() {
    let cells = text_area_tier_cells();
    let submitted = cells
        .iter()
        .find(|cell| cell.id == "text-area/tier/submitted")
        .expect("a submit cell");
    assert_eq!(resolved(submitted, "settles-session"), "false");
    assert_eq!(resolved(submitted, "session-open"), "true");
}

#[test]
fn test_the_cancel_tier_restores_the_first_edit_snapshot() {
    let cells = text_area_tier_cells();
    let cancelled = cells
        .iter()
        .find(|cell| cell.id == "text-area/tier/cancelled")
        .expect("a cancel cell");
    assert_eq!(resolved(cancelled, "emitted-value"), "draft one");
    assert_eq!(resolved(cancelled, "settles-session"), "true");
}

#[test]
fn test_the_blur_tier_settles_and_reports_the_edge() {
    let cells = text_area_tier_cells();
    let blurred = cells
        .iter()
        .find(|cell| cell.id == "text-area/tier/committed-blur")
        .expect("a blur cell");
    assert_eq!(resolved(blurred, "tier-class"), "commit");
    assert_eq!(resolved(blurred, "also-emitted"), "focus");
}

#[test]
fn test_undo_and_redo_cells_report_their_ledgers() {
    let cells = text_area_tier_cells();
    let undone = cells
        .iter()
        .find(|cell| cell.id == "text-area/tier/undone")
        .expect("an undo cell");
    assert_eq!(resolved(undone, "emitted-value"), "draft one");
    assert_eq!(resolved(undone, "redo-depth"), "1");

    let redone = cells
        .iter()
        .find(|cell| cell.id == "text-area/tier/redone")
        .expect("a redo cell");
    assert_eq!(resolved(redone, "emitted-value"), "draft two");
}

#[test]
fn test_the_growth_cells_stop_growing_at_the_ceiling() {
    for cell in &text_area_growth_cells() {
        let height: f32 = resolved(cell, "well.height").parse().expect("a height");
        let max_raw = resolved(cell, "band.max-height");
        if max_raw != "-1" {
            let max: f32 = max_raw.parse().expect("a height");
            assert!(height <= max + 0.001, "{} grew past its ceiling", cell.id);
        }
        assert!(
            height >= {
                let min: f32 = resolved(cell, "band.min-height").parse().expect("a height");
                min - 0.001
            }
        );
    }
}

#[test]
fn test_a_capped_band_becomes_scrollable_once_the_content_passes_it() {
    let cells = text_area_growth_cells();
    let capped = cells
        .iter()
        .filter(|cell| resolved(cell, "band.max-rows") == "3")
        .find(|cell| resolved(cell, "content.lines") == "6")
        .expect("a capped cell with six lines");
    assert_eq!(resolved(capped, "well.scrollable"), "true");
    assert_eq!(resolved(capped, "well.can-grow"), "false");
}

#[test]
fn test_an_uncapped_band_never_becomes_scrollable() {
    for cell in &text_area_growth_cells() {
        if resolved(cell, "band.max-rows") == "none" {
            assert_eq!(resolved(cell, "well.scrollable"), "false");
            assert_eq!(resolved(cell, "well.max-scroll"), "0");
        }
    }
}

#[test]
fn test_the_text_area_state_cells_reuse_the_atom_matrix() {
    let cells = text_area_state_cells();
    assert_eq!(
        cells.len(),
        snapshot_states().len() * RUNGS.len(),
        "the multi-line well is sampled at exactly the atom's own states"
    );
    for cell in &cells {
        assert_eq!(cell.spec, "UIF-ATOM-002e");
        assert_eq!(resolved(cell, "slot.width"), "24");
    }
}

#[test]
fn test_the_matrix_entry_point_is_the_union_of_its_groups() {
    let total = password_state_cells().len()
        + password_reveal_cells().len()
        + password_tier_cells().len()
        + text_area_state_cells().len()
        + text_area_growth_cells().len()
        + text_area_tier_cells().len();
    assert_eq!(secret_cells().len(), total);
}
