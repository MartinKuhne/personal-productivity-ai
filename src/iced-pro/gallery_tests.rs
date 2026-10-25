//! Unit tests for the deterministic `(Variant × State)` gallery
//! (`UIF-VER-004`).
//!
//! Five things are asserted here, and each one is a different failure mode
//! a gallery can have:
//!
//! 1. **Completeness** — the id list is written out in full rather than
//!    counted, so a cell that silently stops being produced shows up as a
//!    missing line rather than as a number that happens to still be right.
//! 2. **Determinism** — two runs are byte-identical. This is the gate's
//!    actual subject: a snapshot that cannot be reproduced cannot be
//!    diffed, and a snapshot that cannot be diffed asserts nothing.
//! 3. **Resolved-style content** — the snapshot names the token behind
//!    every channel, so a regression reads as a named colour moving.
//! 4. **Scope honesty** — the `NotOwned` cells are an explicit list with
//!    a justification each, and they draw nothing.
//! 5. **The gates** — no unmapped colour, and every judged pair clears
//!    its threshold with an empty accepted-failure ledger. The three
//!    non-judged classes are themselves cross-checked, so an exemption
//!    cannot be used as a place to hide a real failure.
//! 6. **The gap families** — the inverted editor palette, the compact
//!    control-height rung, the modal's header/banner/footer bands, menu
//!    separators and glyph placement, the input dialog's per-row bodies
//!    and the movable overlay's trailing actions are each snapshotted as
//!    resolved tokens over an asserted ordered quad list.

use super::*;

use crate::state::BaseState;
use crate::state::VisualState;
use crate::surface::menu::view as menu_view;
use crate::surface::movable::TITLE_BAR_PADDING_HORIZONTAL;
use crate::surface::zorder::ZBand;
use crate::token::color::Palette;
use crate::token::contrast;
use crate::widget::button;
use crate::widget::button::ButtonVariant;
use crate::widget::text_input::InputKind;

// ---------------------------------------------------------------------------
// 1 — completeness
// ---------------------------------------------------------------------------

/// The nine reachable visual states, in snapshot order.
fn expected_states() -> [&'static str; 9] {
    [
        "rest",
        "rest+focused",
        "hovered",
        "hovered+focused",
        "pressed",
        "pressed+focused",
        "invalid",
        "invalid+focused",
        "disabled",
    ]
}

/// Every cell id the gallery must produce, in the order it produces them.
///
/// Written out rather than derived. A count-only assertion passes when a
/// cell disappears and a spurious one appears; this list cannot.
fn expected_ids() -> Vec<String> {
    let mut ids = Vec::new();

    for variant in ["primary", "secondary", "ghost", "danger"] {
        for state in expected_states() {
            for rung in RUNGS {
                ids.push(format!("button/{variant}/{state}{}", rung.slug()));
            }
        }
    }
    // `UIF-ATOM-001a`: the controlled button's busy and density-collapse
    // cells, per variant, then the three activation tiers.
    for variant in ["primary", "secondary", "ghost", "danger"] {
        ids.push(format!("button/busy/{variant}"));
        ids.push(format!("button/collapsed/{variant}"));
    }
    ids.extend(
        [
            "button/tier/activated",
            "button/tier/cancelled",
            "button/tier/cancel-requested",
        ]
        .iter()
        .map(|id| (*id).to_string()),
    );
    // `UIF-ATOM-001b` / `INC-40`: the segmented strip. Two subjects (the
    // one-segment ToggleButton and a three-segment strip sampled at its
    // middle), two polarities, two height rungs, five state columns.
    let strip_states = ["rest", "hovered", "pressed", "rest+focused", "disabled"];
    for subject in ["toggle", "strip"] {
        for polarity in ["off", "on"] {
            for rung in ["compact", "standard"] {
                for state in strip_states {
                    ids.push(format!("{subject}/{polarity}/{rung}-{state}"));
                }
            }
        }
    }
    // `UIF-ATOM-001c` / `INC-40`: the split button, sampled per zone so
    // the two independent hit targets each get their own cell.
    for variant in ["primary", "secondary"] {
        for zone in ["label", "chevron"] {
            for rung in ["compact", "standard"] {
                for state in strip_states {
                    ids.push(format!("split-button/{variant}/{zone}-{rung}-{state}"));
                }
            }
        }
    }
    // `UIF-ATOM-001d` / `INC-40`: the hyperlink against both mounts the
    // spec quotes figures for.
    for state in strip_states {
        for mount in ["base", "overlay"] {
            ids.push(format!("hyperlink/{mount}-{state}"));
        }
    }
    for kind in ["text-entry", "search", "dropdown-trigger"] {
        for state in expected_states() {
            for rung in RUNGS {
                ids.push(format!("input/{kind}/{state}{}", rung.slug()));
            }
        }
    }
    // `UIF-ATOM-002a`: the six tiers the editing engine puts on the wire.
    ids.extend(
        [
            "input/tier/changed",
            "input/tier/committed-enter",
            "input/tier/committed-blur",
            "input/tier/cleared",
            "input/tier/cancelled",
            "input/tier/focus-changed",
        ]
        .iter()
        .map(|id| (*id).to_string()),
    );
    // `UIF-ATOM-002c` / `INC-43`: the password field's own cells. The
    // state columns are the atom's own list, because a password field
    // that sampled a different set of states would be a second control.
    for rung in RUNGS {
        for state in snapshot_states() {
            ids.push(format!("password/{}{}", state_slug(state), rung.slug()));
        }
    }
    for posture in [
        "masked-unfocused",
        "reveal-refused-unfocused",
        "masked-focused",
        "revealed-focused",
        "reveal-refused-disabled",
        "reveal-refused-readonly",
    ] {
        ids.push(format!("password/reveal/{posture}"));
    }
    for tier in [
        "changed",
        "committed",
        "cancelled",
        "revealed",
        "reveal-refused",
        "focus-changed",
    ] {
        ids.push(format!("password/tier/{tier}"));
    }
    // `UIF-ATOM-002e` / `INC-43`: the multi-line well - same state
    // columns, plus the growth ladder and the driven tiers.
    for rung in RUNGS {
        for state in snapshot_states() {
            ids.push(format!("text-area/{}{}", state_slug(state), rung.slug()));
        }
    }
    for band in ["1-3", "3-inf", "2-6"] {
        for lines in [1u32, 2, 3, 4, 6, 9] {
            ids.push(format!("text-area/growth/{band}-{lines}-lines"));
        }
    }
    for tier in [
        "changed",
        "submitted",
        "committed-blur",
        "cancelled",
        "undone",
        "redone",
        "cursor-changed",
    ] {
        ids.push(format!("text-area/tier/{tier}"));
    }
    // `UIF-ATOM-002d` / `INC-44`: the autosuggest field - same state
    // columns and rungs as the plain input, sampled with the completion
    // flyout shut and open - plus the eleven driven tier cells.
    for state in snapshot_states() {
        for rung in RUNGS {
            for open in ["closed", "open"] {
                ids.push(format!(
                    "suggest/{}-{}{}",
                    state_slug(state),
                    open,
                    rung.slug()
                ));
            }
        }
    }
    for tier in [
        "debounce-live",
        "request",
        "highlight",
        "commit-enter",
        "commit-click",
        "commit-tab",
        "dismiss",
        "clear",
        "no-match",
        "stale-answer",
        "focus-loss",
    ] {
        ids.push(format!("suggest-tier/{tier}"));
    }
    // `UIF-ATOM-003`: the three toggle atoms, both polarities, the five
    // columns §3.3 prints. "Focused" is `Rest + ring`, so it reads
    // `rest+focused` the way every other subject spells it.
    for control in ["checkbox", "radio", "switch"] {
        for polarity in ["off", "on"] {
            for column in ["rest", "hovered", "pressed", "rest+focused", "disabled"] {
                ids.push(format!("{control}/{polarity}/{column}"));
            }
        }
    }
    // `UIF-ATOM-004` / `INC-41`: the combobox trigger, sampled with the
    // flyout shut and open, because §3.4 states its chevron rule against
    // the open axis. Six columns rather than the usual five: §3.4 gives
    // the control a `validation` field, so `invalid` is a state the
    // trigger paints differently.
    for rung in ["std", "cmp"] {
        for column in [
            "rest",
            "hovered",
            "pressed",
            "rest+focused",
            "invalid",
            "disabled",
        ] {
            for flyout in ["closed", "open"] {
                ids.push(format!("combo/{rung}-{column}-{flyout}"));
            }
        }
    }
    // The five binding tiers, each driven through a real ComboHost.
    for tier in ["open", "filter-live", "commit", "cancel", "no-match"] {
        ids.push(format!("combo-tier/{tier}"));
    }
    // `UIF-ATOM-005` / `INC-42`: the slider in two tick configurations
    // over the six state columns, then the four drag tiers, each driven
    // through a real SliderHost. The range controls use the six-column
    // skin vocabulary rather than the nine-state list: §3.5 gives the
    // slider one invalid skin (the `2px status.danger.text` thumb ring)
    // and no separate focused-invalid column, and both hosts report focus
    // in the same column as rest.
    let range_states = [
        "rest",
        "hovered",
        "pressed",
        "rest+focused",
        "invalid",
        "disabled",
    ];
    for ticks in ["plain", "ticks"] {
        for state in range_states.iter() {
            ids.push(format!("slider/{ticks}-{state}"));
        }
    }
    for tier in ["live", "committed", "cancelled", "release-outside-track"] {
        ids.push(format!("slider-tier/{tier}"));
    }
    // The number box in both stepper shapes over the same five columns,
    // then the seven step tiers the keyboard and the hold produce.
    for shape in ["stacked", "side-by-side"] {
        for state in range_states.iter() {
            ids.push(format!("number-box/{shape}-{state}"));
        }
    }
    for tier in [
        "step-up",
        "step-down",
        "page-up",
        "page-down",
        "home",
        "end",
        "held",
    ] {
        ids.push(format!("number-box-tier/{tier}"));
    }
    // `UIF-ATOM-008` / `INC-38`: the three badge kinds over the five tones.
    // The dot is tone-only; the count pill samples five boundary values
    // (`0`, `1`, `42`, `99`, `150` — the last two straddle the `99+` cap);
    // the chip pairs each of the four mandatory glyphs with each tone.
    for tone in ["neutral", "accent", "success", "warning", "danger"] {
        ids.push(format!("badge/dot/{tone}"));
    }
    for value in [0, 1, 42, 99, 150] {
        for tone in ["neutral", "accent", "success", "warning", "danger"] {
            ids.push(format!("badge/count/{value}/{tone}"));
        }
    }
    for icon in ["ok", "warn", "fail", "info"] {
        for tone in ["neutral", "accent", "success", "warning", "danger"] {
            ids.push(format!("badge/chip/{icon}-status/{tone}"));
        }
    }
    // `UIF-ATOM-006` / `INC-39`: four chip shapes over the two height
    // rungs and the five §3.3 state columns.
    for shape in ["assist", "filter-off", "filter-on", "input"] {
        for rung in ["compact", "bar"] {
            for state in ["rest", "hovered", "pressed", "rest+focused", "disabled"] {
                ids.push(format!("chip/{shape}/{rung}-{state}"));
            }
        }
    }
    // `UIF-ATOM-009` / `INC-38`: the bar in three statuses at five fill
    // conditions, the spinner at both sizes on both mounts, and the
    // cancellable stop in each status.
    for status in ["active", "paused", "error"] {
        for condition in ["quarter", "half", "full", "indeterminate", "reduced-static"] {
            ids.push(format!("progress/bar/{status}/{condition}"));
        }
    }
    for size in ["sm", "md"] {
        for mount in ["plain", "solid"] {
            ids.push(format!("progress/spinner/{size}/{mount}"));
        }
    }
    for status in ["active", "paused", "error"] {
        ids.push(format!("progress/stop/{status}"));
    }
    // `UIF-COMP-001` / `INC-45`: the stacked field's three caption
    // postures, then the same control slot hosting three atom kinds; the
    // settings row in three content shapes.
    for posture in ["empty", "helper", "error"] {
        ids.push(format!("form-field/{posture}"));
    }
    for host in ["button", "input", "row"] {
        ids.push(format!("form-field/host/{host}"));
    }
    for shape in ["title-only", "with-description", "disabled"] {
        ids.push(format!("settings-row/{shape}"));
    }
    // `UIF-COMP-002` / `INC-47`: the docked banner across both permitted
    // fills, three docked postures and all four severities. The severity
    // axis is the recomputed-contrast evidence: the title and body keep
    // their colours at every severity, so only the stripe and icon move.
    for surface in ["raised", "overlay"] {
        for posture in ["bare", "action", "action-dismiss"] {
            for severity in ["info", "success", "warning", "danger"] {
                ids.push(format!("info-bar/{severity}/{surface}/{posture}"));
            }
        }
    }
    // The transient in every timer posture the framework can be in,
    // including the two severities that refuse to auto-dismiss and the
    // overflow that queues rather than dropping an unacknowledged one.
    for posture in [
        "armed",
        "hover-held",
        "focus-held",
        "persistent-warning",
        "persistent-danger",
        "stack-three",
        "queued",
        "narrow-viewport",
    ] {
        ids.push(format!("toast/{posture}"));
    }
    // `UIF-COMP-003` / `INC-46`: the disclosure across both rungs and the
    // reachable pointer states. The collapsed and open pair is the zero-shift
    // evidence; the collapsed row is also the tab-reachability evidence.
    for posture in [
        "collapsed",
        "open",
        "hovered",
        "pressed",
        "disabled",
        "compact-collapsed",
        "compact-open",
    ] {
        ids.push(format!("expander/{posture}"));
    }
    // `UIF-COMP-004` / `INC-46`: the scroll contract in both layout modes,
    // every track phase, the extreme content ratio, the unscrollable view and
    // the annotated track.
    for posture in [
        "overlay-rest",
        "overlay-hovered",
        "overlay-dragged",
        "reserved-rest",
        "reserved-hovered",
        "extreme-ratio",
        "hidden-unscrollable",
        "annotated",
    ] {
        ids.push(format!("scroll-view/{posture}"));
    }
    // `UIF-COMP-005` / `INC-45`: the card at both elevations, and the
    // tile across both rungs, both selection postures, the pointer states
    // and the inert row.
    for elevation in ["rest", "elevated"] {
        ids.push(format!("card/{elevation}"));
    }
    for state in [
        "single",
        "two-line",
        "single-selected",
        "two-line-selected",
        "hovered",
        "pressed",
        "disabled",
        "non-interactive",
    ] {
        ids.push(format!("list-tile/{state}"));
    }
    // `UIF-COMP-006` / `INC-48`: the tree row across its indent tiers and
    // adaptive marks, then the recycling primitive across both rungs and
    // both corpus sizes.
    for posture in [
        "rest/depth-0",
        "rest/depth-3",
        "hovered/depth-2",
        "pressed/depth-2",
        "selected/depth-1",
        "selected/depth-4",
        "selected-warning/depth-2",
        "warning/depth-2",
        "disabled/depth-2",
        "narrow/depth-9-compressed",
        "narrow/depth-20-collapsed",
    ] {
        ids.push(format!("tree-row/{posture}"));
    }
    for sample in [
        "compact/1k",
        "standard/1k",
        "compact/100k",
        "compact/1m",
        "standard/1m",
    ] {
        ids.push(format!("virtual-list/{sample}"));
    }
    for band in ["base", "raised", "hover", "selected"] {
        ids.push(format!("table-cell/{band}"));
    }
    for regime in ["surplus", "at-target", "deficit", "oversubscribed"] {
        ids.push(format!("table-column/{regime}"));
    }
    for posture in [
        "sortable-resizable",
        "sorted-ascending",
        "sorted-descending",
        "last-column",
    ] {
        ids.push(format!("table-header/{posture}"));
    }
    // `UIF-COMP-008` / `INC-50`: the trail's emphasis ladder, the
    // separator pairing, the five elision regimes, the `...` trigger,
    // the five eviction regimes and the three overflow-row postures.
    for role in ["root", "middle", "terminal"] {
        for state in ["rest", "hovered", "pressed", "disabled"] {
            ids.push(format!("breadcrumb-crumb/{role}/{state}"));
        }
    }
    ids.push("breadcrumb-separator/rest".to_string());
    for regime in ["fits", "elide-1", "elide-2", "elide-3", "below-floor"] {
        ids.push(format!("breadcrumb-collapse/{regime}"));
    }
    for state in ["rest", "hovered", "pressed", "disabled"] {
        ids.push(format!("overflow-trigger/{state}"));
    }
    for regime in ["fits", "evict-1", "evict-2", "evict-3", "last-standing"] {
        ids.push(format!("overflow-strip/{regime}"));
    }
    for posture in ["enabled", "disabled", "busy"] {
        ids.push(format!("overflow-row/{posture}"));
    }
    ids.extend(
        [
            "rest",
            "hovered",
            "pressed",
            "hovered+focused",
            "submenu-parent",
            "destructive",
            "destructive-pressed",
            "destructive+focused",
            "submenu-parent+focused",
            "rest+focused",
            "disabled",
            "heading",
        ]
        .iter()
        .map(|state| format!("menu/row/{state}")),
    );
    ids.extend(
        [
            "separator",
            "check-on",
            "check-off",
            "radio-selected",
            "radio-unselected",
            "submenu-chevron",
            "submenu-chevron-open",
            "check-on-hovered",
            "shortcut",
            "icon",
        ]
        .iter()
        .map(|name| format!("menu-glyph/{name}")),
    );
    // `UIF-ATOM-000b` / `INC-36`: the five tiers the menu-row action
    // engine puts on the wire.
    ids.extend(
        [
            "committed",
            "cancelled",
            "cancel-requested",
            "suppressed",
            "fire-and-settle",
        ]
        .iter()
        .map(|tier| format!("menu-tier/{tier}")),
    );
    ids.extend(
        ["clean", "dirty", "error"]
            .iter()
            .map(|name| format!("modal/{name}")),
    );
    ids.extend(
        ["no-banner", "banner"]
            .iter()
            .map(|name| format!("modal-bands/{name}")),
    );
    // `UIF-ATOM-000b` / `INC-36`: the modal's pending-action states.
    ids.extend(
        ["idle", "pending", "cancel-requested", "dismiss-refused"]
            .iter()
            .map(|tier| format!("modal-tier/{tier}")),
    );
    ids.extend(
        [
            "rename-valid",
            "rename-invalid",
            "create-valid",
            "move-folder-list",
            "delete-danger",
        ]
        .iter()
        .map(|name| format!("input-dialog/{name}")),
    );
    ids.extend(
        [
            "text-entry",
            "folder-list",
            "folder-list-capped",
            "confirmation",
        ]
        .iter()
        .map(|name| format!("input-dialog-rows/{name}")),
    );
    // `UIF-ATOM-000b` / `INC-36`: the dialog footer's four tier outcomes.
    ids.extend(
        [
            "primary-committed",
            "secondary-cancelled",
            "primary-cancel-requested",
            "primary-suppressed",
        ]
        .iter()
        .map(|tier| format!("dialog-tier/{tier}")),
    );
    ids.extend(
        ["idle", "dragging", "resizing", "auto-scroll-paused"]
            .iter()
            .map(|name| format!("movable/{name}")),
    );
    ids.extend(
        ["idle", "dragged", "resized"]
            .iter()
            .map(|name| format!("movable-actions/{name}")),
    );
    // `UIF-ATOM-000b` / `INC-36`: the `on_drag_settle` tiers - many
    // `Live` frames, one `Commit`, one `Cancel` (§3.0.4.2).
    ids.extend(
        ["live", "committed", "cancelled"]
            .iter()
            .map(|tier| format!("drag-tier/{tier}")),
    );
    ids.extend(
        ["rest", "error"]
            .iter()
            .map(|name| format!("editor-inverted/{name}")),
    );
    ids.push("zband/overlay-menu-modal".to_string());
    ids.extend(
        [
            "card",
            "state-hidden",
            "state-pending",
            "state-shown",
            "state-keyboard",
            "place-flip-above",
            "place-clamped",
            // `UIF-TOK-007` / `B-5`: the three warm-handoff verdicts, each
            // probed at the arrival instant.
            "warm-cold-first-hover",
            "warm-handoff",
            "warm-cooled",
        ]
        .iter()
        .map(|name| format!("tooltip/{name}")),
    );
    // `UIF-ATOM-007` / `INC-39`: the teaching popover across the three
    // width regimes and the two footer shapes.
    for regime in ["under-min", "in-band", "over-max"] {
        for footer in ["with-action", "dismiss-only"] {
            ids.push(format!("popover/{regime}/{footer}"));
        }
    }
    ids.extend(
        ["navbar", "panes", "tabs"]
            .iter()
            .map(|subject| format!("{subject}/not-owned")),
    );

    ids
}

#[test]
fn the_matrix_produces_every_expected_cell_in_the_expected_order() {
    let cells = matrix();
    let actual: Vec<String> = cells.iter().map(|cell| cell.id.clone()).collect();

    assert_eq!(actual, expected_ids(), "the cell inventory moved");
}

#[test]
fn the_matrix_has_no_duplicate_cell_ids() {
    let cells = matrix();
    let mut seen = Vec::new();
    for cell in &cells {
        assert!(
            !seen.contains(&cell.id),
            "cell id {} is produced twice",
            cell.id
        );
        seen.push(cell.id.clone());
    }
}

#[test]
fn per_subject_cell_counts_match_the_plan() {
    let cells = matrix();
    let counts = subject_counts_for(&cells);

    let mut expected: Vec<(&str, usize)> = vec![
        ("button", 83),
        // `UIF-ATOM-001b` / `INC-40`: the one-segment ToggleButton strip
        // and a three-segment strip, each 2 polarities x 2 rungs x 5
        // state columns.
        ("toggle-button", 20),
        ("segmented-button", 20),
        // `UIF-ATOM-001c` / `INC-40`: 2 variants x 2 zones x 2 rungs x
        // 5 state columns.
        ("split-button", 40),
        // `UIF-ATOM-001d` / `INC-40`: 5 state columns x 2 mounts.
        ("hyperlink", 10),
        ("input", 60),
        // `UIF-ATOM-002c` / `INC-43`: the password field's own cells —
        // 2 rungs x 9 reachable states plus the six reveal-policy
        // postures, plus the six driven tier cells.
        ("password", 24),
        ("password-tier", 6),
        // `UIF-ATOM-002e` / `INC-43`: 2 rungs x 9 reachable states,
        // the 3 growth bands x 6 content sizes, and the seven driven
        // tier cells.
        ("text-area", 18),
        ("text-area-growth", 18),
        ("text-area-tier", 7),
        // `UIF-ATOM-002d` / `INC-44`: 9 reachable states x 2 rungs x 2
        // flyout states, plus the eleven driven tier cells.
        ("suggest", 36),
        ("suggest-tier", 11),
        // `UIF-ATOM-003`: 3 controls x 2 polarities x 5 columns each.
        ("checkbox", 10),
        ("radio", 10),
        ("switch", 10),
        // `UIF-ATOM-004` / `INC-41`: 2 rungs x 6 state columns x 2
        // flyout states, plus the five driven binding-tier cells.
        ("combo", 24),
        ("combo-tier", 5),
        // `UIF-ATOM-005` / `INC-42`: 2 tick configurations x 5 state
        // columns, plus the four driven drag-tier cells.
        ("slider", 12),
        ("slider-tier", 4),
        // The number box: 2 stepper shapes x 5 state columns, plus the
        // six keyboard-step cells and the one press-and-hold cell.
        ("number-box", 12),
        ("number-box-tier", 7),
        // `UIF-ATOM-008` / `INC-38`: 5 dots + 5 counts x 5 tones + 4 glyphs x 5 tones.
        ("badge", 50),
        // `UIF-ATOM-006` / `INC-39`: 4 shapes x 2 rungs x 5 columns.
        ("chip", 40),
        // `UIF-ATOM-009` / `INC-38`: 3 statuses x 5 fills + 4 spinners + 3 stops.
        ("progress", 22),
        // `UIF-COMP-001` / `INC-45`: 3 caption postures + 3 hosted kinds,
        // and the settings row in 3 content shapes.
        ("form-field", 6),
        ("settings-row", 3),
        // `UIF-COMP-002` / `INC-47`: the banner across 2 fills x 3
        // docked postures x 4 severities, and the transient in 8 timer
        // postures — armed, the two holds, the two severities that refuse
        // to auto-dismiss, the full stack, the queued overflow and the
        // clamped narrow viewport.
        ("info-bar", 24),
        ("toast", 8),
        // `UIF-COMP-003` / `INC-46`: the disclosure across 7 postures —
        // collapsed, open, hovered, pressed, disabled, and the compact rung
        // collapsed and open.
        ("expander", 7),
        // `UIF-COMP-004` / `INC-46`: the scroll contract across 8
        // postures — the three overlay phases, the reserved gutter at rest
        // and hover, the extreme content ratio, the unscrollable view, and
        // the annotated track.
        ("scroll-view", 8),
        // `UIF-COMP-005` / `INC-45`: the card at 2 elevations, the tile
        // across 8 postures.
        ("card", 2),
        ("list-tile", 8),
        // `UIF-COMP-006` / `INC-48`: the recycling primitive across 5
        // postures, and the tree row across 11 — the three indent tiers,
        // the adaptive chevron and guide marks, the fill-locked warning
        // dot, the disabled rung and the narrow-viewport floor.
        ("virtual-list", 5),
        ("table-cell", 4),
        ("table-column", 4),
        ("table-header", 4),
        ("tree-row", 11),
        // `UIF-COMP-008` / `INC-50`: the trail's three roles across four
        // postures, the separator pairing, the five elision regimes, the
        // trigger's four postures, the five eviction regimes and the
        // three overflow-row postures.
        ("breadcrumb-crumb", 12),
        ("breadcrumb-separator", 1),
        ("breadcrumb-collapse", 5),
        ("overflow-trigger", 4),
        ("overflow-strip", 5),
        ("overflow-row", 3),
        ("menu", 12),
        ("menu-glyph", 10),
        // `INC-36`: the `000b` tiers on the four surface hosts.
        ("menu-tier", 5),
        ("modal", 3),
        ("modal-bands", 2),
        ("modal-tier", 4),
        ("input-dialog", 5),
        ("input-dialog-rows", 4),
        ("dialog-tier", 4),
        ("movable", 4),
        ("movable-actions", 3),
        ("drag-tier", 3),
        ("editor", 2),
        ("zband", 1),
        // `UIF-TOK-007` / `B-5` adds the three warm-handoff verdict cells.
        ("tooltip", 10),
        // `UIF-ATOM-007` / `INC-39`: 3 width regimes x 2 footer shapes.
        ("popover", 6),
        ("navbar", 1),
        ("panes", 1),
        ("tabs", 1),
    ];
    expected.sort();
    let mut actual = counts.clone();
    actual.sort();

    assert_eq!(actual, expected, "per-subject coverage moved");
}

/// Per-subject counts, sorted so the assertion is order independent.
fn subject_counts_for(cells: &[Cell]) -> Vec<(&'static str, usize)> {
    let mut counts: Vec<(&'static str, usize)> = Vec::new();
    for cell in cells {
        if let Some(entry) = counts.iter_mut().find(|(s, _)| *s == cell.subject) {
            entry.1 += 1;
        } else {
            counts.push((cell.subject, 1));
        }
    }
    counts
}

#[test]
fn every_button_variant_is_covered_across_every_reachable_state() {
    for variant in BUTTON_VARIANTS {
        for state in snapshot_states() {
            let id = format!("button/{}/{}", slug(variant), state_slug(state));
            assert!(
                matrix().iter().any(|cell| cell.id == id),
                "missing button cell {id}"
            );
        }
    }
}

#[test]
fn every_input_kind_is_covered_across_every_reachable_state() {
    for kind in INPUT_KINDS {
        for state in snapshot_states() {
            let id = format!("input/{}/{}", slug_kind(kind), state_slug(state));
            assert!(
                matrix().iter().any(|cell| cell.id == id),
                "missing input cell {id}"
            );
        }
    }
}

#[test]
fn the_reachable_state_set_is_nine_and_snapshot_order_agrees_with_it() {
    let reachable = reachable_visual_states();
    let ordered = snapshot_states();

    assert_eq!(reachable.len(), 9, "the reachable state set moved");
    assert_eq!(ordered.len(), 9);

    for state in &reachable {
        assert!(
            ordered.contains(state),
            "reachable state {:?}+focused={} is missing from the snapshot order",
            state.base,
            state.focused
        );
    }
    for state in &ordered {
        assert!(reachable.contains(state), "snapshot state is not reachable");
    }
}

#[test]
fn disabled_and_focused_is_not_snapshotted_because_resolve_zeroes_it() {
    let dead = VisualState {
        base: BaseState::Disabled,
        focused: true,
    };
    assert!(
        !snapshot_states().contains(&dead),
        "a dead control cannot show a ring, so the gallery must not claim one"
    );
}

// ---------------------------------------------------------------------------
// 2 — determinism
// ---------------------------------------------------------------------------

#[test]
fn two_matrix_calls_are_byte_identical() {
    let first: String = matrix().iter().map(Cell::snapshot).collect();
    let second: String = matrix().iter().map(Cell::snapshot).collect();

    assert_eq!(first, second, "the gallery is not deterministic");
}

#[test]
fn two_manifest_calls_are_byte_identical() {
    assert_eq!(manifest(), manifest(), "the manifest is not deterministic");
}

#[test]
fn a_cell_built_twice_equals_itself() {
    for state in snapshot_states() {
        assert_eq!(
            button_cell(ButtonVariant::Primary, state),
            button_cell(ButtonVariant::Primary, state)
        );
    }
    for kind in INPUT_KINDS {
        for state in snapshot_states() {
            assert_eq!(text_input_cell(kind, state), text_input_cell(kind, state));
        }
    }
    for state in menu_view::RowState::ALL {
        assert_eq!(menu_row_cell(state), menu_row_cell(state));
    }
    assert_eq!(zband_cell(), zband_cell());
}

#[test]
fn the_gallery_reads_no_wall_clock_or_random_source() {
    // The determinism contract is structural: the same call, twice, with
    // nothing between the calls, produces the same bytes. A clock or a
    // random source anywhere in the build path would show up here, and
    // `two_matrix_calls_are_byte_identical` is the cheap form of it.
    let a = manifest();
    let b = manifest();
    let c = manifest();
    assert_eq!(a, b);
    assert_eq!(b, c);
}

// ---------------------------------------------------------------------------
// 3 — resolved-style content
// ---------------------------------------------------------------------------

/// The snapshot line for one resolved channel of one cell.
fn resolved_line(cell: &Cell, channel: &str) -> String {
    cell.resolved
        .iter()
        .find(|entry| entry.channel == channel)
        .unwrap_or_else(|| panic!("cell {} has no channel {channel}", cell.id))
        .value
        .display()
}

#[test]
fn every_resolved_colour_channel_names_a_real_token() {
    for cell in matrix() {
        for entry in &cell.resolved {
            if let StyleValue::Colour { token, .. } = &entry.value {
                assert!(
                    !token.starts_with("UNMAPPED"),
                    "{} resolved.{} is {}",
                    cell.id,
                    entry.channel,
                    token
                );
            }
        }
    }
}

#[test]
fn the_snapshot_names_the_token_behind_a_primary_resting_fill() {
    let cell = button_cell(
        ButtonVariant::Primary,
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
    );
    let palette = Palette::dark();

    assert_eq!(
        resolved_line(&cell, "background"),
        format!("accent.fill ({})", rgb8(palette.accent_fill)),
        "the primary resting fill must be named by its token, not by a literal"
    );
    assert!(cell.snapshot().contains("button/primary/rest"));
}

#[test]
fn a_ghost_at_rest_records_a_transparent_fill_rather_than_an_invented_one() {
    let rest = VisualState {
        base: BaseState::Rest,
        focused: false,
    };
    let ghost = button_cell(ButtonVariant::Ghost, rest);
    let primary = button_cell(ButtonVariant::Primary, rest);

    let ghost_fill = ghost
        .frame
        .quads
        .iter()
        .find(|quad| quad.layer == "background")
        .expect("the Ghost cell records its background layer");
    assert_eq!(
        ghost_fill.fill, "transparent",
        "a Ghost at rest must record the spec's own word for its fill, not a colour"
    );
    assert_eq!(ghost_fill.border, "transparent");
    assert_eq!(resolved_line(&ghost, "transparent-background"), "true");

    let primary_fill = primary
        .frame
        .quads
        .iter()
        .find(|quad| quad.layer == "background")
        .expect("the Primary cell must still paint its fill");
    assert_ne!(
        primary_fill.fill, "transparent",
        "the control cell must be distinguishable from the Ghost in the manifest"
    );
}

#[test]
fn the_focus_ring_is_a_quad_outside_the_background_layer() {
    let focused = VisualState {
        base: BaseState::Rest,
        focused: true,
    };
    let cell = button_cell(ButtonVariant::Secondary, focused);
    let layers: Vec<&str> = cell
        .frame
        .quads
        .iter()
        .map(|quad| quad.layer.as_str())
        .collect();

    assert_eq!(layers, vec!["background", "focus-ring"]);
    assert!(
        cell.resolved
            .iter()
            .any(|entry| entry.channel == "focus-ring.width"
                && matches!(entry.value, StyleValue::Number(w) if w > 0.0))
    );
}

#[test]
fn an_unfocused_cell_records_no_ring_layer() {
    let cell = button_cell(
        ButtonVariant::Secondary,
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
    );
    assert!(
        cell.frame
            .quads
            .iter()
            .all(|quad| quad.layer != "focus-ring")
    );
}

#[test]
fn the_input_zero_shift_rectangles_are_identical_across_every_state_of_a_kind() {
    // `UIF-INP-003`: the text area and the reserved trailing slot are a
    // function of (bounds, kind) alone. If an error state moved either
    // one, this is the assertion that catches it.
    for kind in INPUT_KINDS {
        let reference = text_input_cell(
            kind,
            VisualState {
                base: BaseState::Rest,
                focused: false,
            },
        );
        let text_area = resolved_line(&reference, "geometry.text-area");
        let slot = resolved_line(&reference, "geometry.trailing-slot");

        for state in snapshot_states() {
            let cell = text_input_cell(kind, state);
            assert_eq!(
                resolved_line(&cell, "geometry.text-area"),
                text_area,
                "{} moved the text area",
                cell.id
            );
            assert_eq!(
                resolved_line(&cell, "geometry.trailing-slot"),
                slot,
                "{} moved the trailing slot",
                cell.id
            );
        }
    }
}

#[test]
fn the_invalid_input_is_the_only_one_flagged_as_an_error_border() {
    for kind in INPUT_KINDS {
        let invalid = text_input_cell(
            kind,
            VisualState {
                base: BaseState::Invalid,
                focused: false,
            },
        );
        let rest = text_input_cell(
            kind,
            VisualState {
                base: BaseState::Rest,
                focused: false,
            },
        );

        assert_eq!(resolved_line(&invalid, "error-border"), "true");
        assert_eq!(resolved_line(&rest, "error-border"), "false");
    }
}

#[test]
fn the_invalid_and_focused_input_keeps_the_error_and_relocates_focus_to_a_ring() {
    let cell = text_input_cell(
        InputKind::TextEntry,
        VisualState {
            base: BaseState::Invalid,
            focused: true,
        },
    );

    assert_eq!(resolved_line(&cell, "error-border"), "true");
    assert!(
        cell.frame
            .quads
            .iter()
            .any(|quad| quad.layer == "focus-ring"),
        "focus must stay visible while the error holds the perimeter"
    );
    assert_eq!(
        resolved_line(&cell, "geometry.text-area"),
        resolved_line(
            &text_input_cell(
                InputKind::TextEntry,
                VisualState {
                    base: BaseState::Rest,
                    focused: false,
                }
            ),
            "geometry.text-area"
        ),
        "the ring is drawn outside the control, so the text area cannot shift"
    );
}

#[test]
fn every_rendered_cell_records_a_finite_frame_viewport() {
    for cell in matrix() {
        if cell.kind == CellKind::NotOwned {
            continue;
        }
        for value in cell.frame.viewport {
            assert!(
                value.is_finite(),
                "{} recorded a non-finite viewport",
                cell.id
            );
        }
        assert!(
            cell.frame.viewport[2] > 0.0 && cell.frame.viewport[3] > 0.0,
            "{} recorded an empty viewport",
            cell.id
        );
    }
}

#[test]
fn every_emitted_quad_is_finite_and_non_negative() {
    for cell in matrix() {
        for quad in &cell.frame.quads {
            for value in quad.bounds {
                assert!(
                    value.is_finite(),
                    "{} {} bounds are not finite",
                    cell.id,
                    quad.layer
                );
            }
            assert!(
                quad.border_width >= 0.0 && quad.radius >= 0.0,
                "{} {} has a negative stroke or radius",
                cell.id,
                quad.layer
            );
        }
    }
}

#[test]
fn the_modal_cells_differ_only_in_the_error_channels_they_add() {
    let cells = modal_cells();
    let clean = cells.iter().find(|c| c.id == "modal/clean").unwrap();
    let errored = cells.iter().find(|c| c.id == "modal/error").unwrap();

    assert!(!clean.resolved.iter().any(|e| e.channel == "error.code"));
    assert!(errored.resolved.iter().any(|e| e.channel == "error.code"));
    assert!(
        errored
            .resolved
            .iter()
            .any(|e| e.channel == "error.recovery")
    );
    assert_eq!(
        errored
            .resolved
            .iter()
            .filter(|e| e.channel.starts_with("frame."))
            .count(),
        clean
            .resolved
            .iter()
            .filter(|e| e.channel.starts_with("frame."))
            .count(),
        "an error must not change the card chrome"
    );
}

#[test]
fn the_input_dialog_validation_slot_is_recorded_whether_or_not_it_is_showing() {
    let cells = input_dialog_cells();
    let valid = cells
        .iter()
        .find(|c| c.id == "input-dialog/rename-valid")
        .unwrap();
    let invalid = cells
        .iter()
        .find(|c| c.id == "input-dialog/rename-invalid")
        .unwrap();

    assert_eq!(
        resolved_line(valid, "geometry.validation-slot"),
        resolved_line(invalid, "geometry.validation-slot"),
        "the slot occupies the same rectangle full or empty"
    );
    assert_eq!(
        resolved_line(valid, "geometry.footer"),
        resolved_line(invalid, "geometry.footer"),
        "the footer cannot shift when an error appears"
    );
    assert_eq!(resolved_line(valid, "ok-enabled"), "true");
    assert_eq!(resolved_line(invalid, "ok-enabled"), "false");
}

#[test]
fn the_delete_confirmation_carries_the_danger_intent_and_the_button_follows_it() {
    let cells = input_dialog_cells();
    let delete = cells
        .iter()
        .find(|c| c.id == "input-dialog/delete-danger")
        .unwrap();

    assert_eq!(resolved_line(delete, "intent"), "Danger");
    let primary = delete
        .frame
        .quads
        .iter()
        .find(|quad| quad.layer == "footer-primary")
        .expect("the delete card must paint a primary footer button");

    // `UIF-INP-004` fixes the *intent*, and the intent selects the
    // variant. The crate's `Danger` button rests on a neutral surface and
    // carries the error red as its border — the red fill only arrives on
    // hover and press — so the proof that the intent drove the choice is
    // that the footer resolved to the Danger resting style rather than to
    // the `Primary` one a non-destructive card would have used.
    let palette = Palette::dark();
    let rest = VisualState {
        base: BaseState::Rest,
        focused: false,
    };
    let danger = button::resolve_style(ButtonVariant::Danger, rest, &palette);
    let standard = button::resolve_style(ButtonVariant::Primary, rest, &palette);

    assert_eq!(
        primary.fill,
        token_name(danger.background),
        "the Danger intent must render the primary with the Danger variant's fill"
    );
    assert_eq!(primary.border, token_name(danger.border_color));
    assert_ne!(
        primary.fill,
        token_name(standard.background),
        "a destructive primary must not be the ordinary primary"
    );
    assert_eq!(
        token_name(danger.border_color),
        token_name(palette.status_danger_text),
        "the destructive signal is the error red on the border"
    );
}

#[test]
fn the_rename_dialog_rejects_an_unchanged_value_and_accepts_a_changed_one() {
    let cells = input_dialog_cells();
    let unchanged = cells
        .iter()
        .find(|c| c.id == "input-dialog/rename-invalid")
        .unwrap();
    let changed = cells
        .iter()
        .find(|c| c.id == "input-dialog/rename-valid")
        .unwrap();

    assert_eq!(resolved_line(unchanged, "ok-enabled"), "false");
    assert_eq!(resolved_line(changed, "ok-enabled"), "true");
}

#[test]
fn the_movable_cells_report_the_gesture_they_were_driven_with() {
    let cells = movable_cells();
    let idle = cells.iter().find(|c| c.id == "movable/idle").unwrap();
    let dragging = cells.iter().find(|c| c.id == "movable/dragging").unwrap();
    let resizing = cells.iter().find(|c| c.id == "movable/resizing").unwrap();
    let paused = cells
        .iter()
        .find(|c| c.id == "movable/auto-scroll-paused")
        .unwrap();

    assert_eq!(resolved_line(idle, "dragging"), "false");
    assert_eq!(resolved_line(dragging, "dragging"), "true");
    assert_eq!(resolved_line(resizing, "resizing"), "Some(BottomRight)");
    assert_eq!(resolved_line(paused, "follow"), "Paused");
    assert_eq!(resolved_line(idle, "follow"), "Following");

    assert_ne!(
        resolved_line(idle, "geometry.card"),
        resolved_line(dragging, "geometry.card"),
        "the drag must actually have moved the card"
    );
    assert_ne!(
        resolved_line(idle, "geometry.card"),
        resolved_line(resizing, "geometry.card"),
        "the resize must actually have changed the card"
    );
}

#[test]
fn every_movable_cell_paints_the_full_card_anatomy() {
    for cell in movable_cells() {
        let layers: Vec<&str> = cell
            .frame
            .quads
            .iter()
            .map(|quad| quad.layer.as_str())
            .collect();
        assert_eq!(
            layers,
            vec![
                "card",
                "title-bar",
                "filter-bar",
                "content-viewport",
                "footer"
            ],
            "{} did not paint the whole card",
            cell.id
        );
    }
}

// ---------------------------------------------------------------------------
// 4 — the six gap families (§2.5, §3.1, §4.5.1, §4.6.1, §4.8.1, §4.9.2)
// ---------------------------------------------------------------------------

/// Tolerance for geometry read back out of a frame.
///
/// The manifest prints two decimals; the assertion is made on the stored
/// `f32`, so the epsilon only has to cover the arithmetic, not the print.
const GEOM_EPS: f32 = 1.0e-3;

/// One recorded quad by layer name.
fn quad<'a>(cell: &'a Cell, layer: &str) -> &'a FrameQuad {
    quad_opt(cell, layer).unwrap_or_else(|| panic!("{} painted no `{layer}` layer", cell.id))
}

/// [`quad`] without the panic, for a layer a cell paints only sometimes.
fn quad_opt<'a>(cell: &'a Cell, layer: &str) -> Option<&'a FrameQuad> {
    cell.frame.quads.iter().find(|q| q.layer == layer)
}

/// A recorded quad's left, top, right and bottom edges.
fn edges(quad: &FrameQuad) -> (f32, f32, f32, f32) {
    (
        quad.bounds[0],
        quad.bounds[1],
        quad.bounds[0] + quad.bounds[2],
        quad.bounds[1] + quad.bounds[3],
    )
}

/// Whether one recorded quad lies wholly inside another.
fn contains(outer: &FrameQuad, inner: &FrameQuad) -> bool {
    let (l0, t0, r0, b0) = edges(outer);
    let (l1, t1, r1, b1) = edges(inner);
    l1 >= l0 - GEOM_EPS && t1 >= t0 - GEOM_EPS && r1 <= r0 + GEOM_EPS && b1 <= b0 + GEOM_EPS
}

/// A recorded numeric channel.
fn number(cell: &Cell, channel: &str) -> f32 {
    resolved_line(cell, channel)
        .parse()
        .unwrap_or_else(|_| panic!("{}.{} is not a number", cell.id, channel))
}

/// A `rect_text` value — `"x y width height"` — read back out.
fn parse_rect(text: &str) -> (f32, f32, f32, f32) {
    let parts: Vec<f32> = text
        .split_whitespace()
        .map(|part| {
            part.parse()
                .unwrap_or_else(|_| panic!("\"{text}\" is not a recorded rectangle"))
        })
        .collect();
    assert_eq!(parts.len(), 4, "\"{text}\" is not a recorded rectangle");
    (parts[0], parts[1], parts[2], parts[3])
}

/// The cell's colour channels as `(channel, token)` pairs.
///
/// Comparing these between two cells is how "the same style, a different
/// box" is asserted: the geometry channels are numbers and drop out.
fn colour_channels(cell: &Cell) -> Vec<(&'static str, String)> {
    cell.resolved
        .iter()
        .filter_map(|entry| match &entry.value {
            StyleValue::Colour { token, .. } => Some((entry.channel, token.clone())),
            _ => None,
        })
        .collect()
}

/// The audit pair recorded for one channel.
fn audit_pair<'a>(cell: &'a Cell, channel: &str) -> &'a AuditPair {
    cell.audit
        .iter()
        .find(|pair| pair.channel == channel)
        .unwrap_or_else(|| panic!("{} audited no `{channel}` channel", cell.id))
}

// --- 4.1 — the inverted editor palette (§2.5) ------------------------------

#[test]
fn the_editor_cells_declare_the_inverted_palette_as_their_naming_world() {
    let cells = editor_palette_cells();
    assert_eq!(cells.len(), 2, "the editor row has two ink channels");
    for cell in &cells {
        assert_eq!(
            cell.namer,
            Namer::InvertedEditor,
            "{} must name through the inverted editor palette",
            cell.id
        );
    }

    let standard = button_cell(
        ButtonVariant::Primary,
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
    );
    assert_eq!(standard.namer, Namer::Standard);
}

#[test]
fn the_two_namers_disagree_about_white_and_only_the_editor_one_names_the_error_red() {
    let editor = EditorPalette::inverted();

    // The canvas is white, and so is the standard palette's `text.strong`.
    // Value-based naming alone cannot tell the two apart, which is why the
    // cell declares its world.
    assert_eq!(Namer::Standard.name(editor.background), "text.strong");
    assert_eq!(
        Namer::InvertedEditor.name(editor.background),
        "editor.inverted.bg"
    );

    // The editor's error red belongs to no standard token. Naming it from
    // the standard table would report it as UNMAPPED, which the gate
    // treats as a hard failure — so the fallback order matters.
    assert!(
        Namer::Standard.name(editor.error).starts_with("UNMAPPED"),
        "the standard palette must not claim the editor's error red, got {}",
        Namer::Standard.name(editor.error)
    );
    assert_eq!(
        Namer::InvertedEditor.name(editor.error),
        "editor.inverted.err"
    );
    assert_eq!(
        Namer::InvertedEditor.name(editor.text),
        "editor.inverted.fg"
    );
}

#[test]
fn the_editor_cells_resolve_the_canvas_and_each_ink_channel_against_it() {
    let editor = EditorPalette::inverted();
    let body = &editor_palette_cells()[0];
    let error = &editor_palette_cells()[1];

    for cell in [body, error] {
        assert_eq!(
            resolved_line(cell, "canvas.fill"),
            format!("editor.inverted.bg ({})", rgb8(editor.background)),
            "{} must name its canvas from the editor palette",
            cell.id
        );
        assert!(
            edges(quad(cell, "editor-canvas")).2 > 0.0,
            "{} painted no canvas to sit on",
            cell.id
        );
        assert!(
            contains(quad(cell, "editor-canvas"), quad(cell, "ink-run")),
            "{}'s ink run must lie inside the canvas it is measured against",
            cell.id
        );
    }

    assert_eq!(
        resolved_line(body, "ink.foreground"),
        format!("editor.inverted.fg ({})", rgb8(editor.text))
    );
    assert_eq!(
        resolved_line(error, "ink.foreground"),
        format!("editor.inverted.err ({})", rgb8(editor.error))
    );
}

#[test]
fn the_editor_contrast_is_recomputed_rather_than_copied_from_the_spec_print() {
    for cell in editor_palette_cells() {
        let pair = audit_pair(
            &cell,
            if cell.id.ends_with("error") {
                "error.text"
            } else {
                "buffer.text"
            },
        );
        let (ratio, _, class) = measure(pair);
        assert_eq!(
            class,
            AuditClass::Judged,
            "{} is not a judged pair",
            cell.id
        );
        assert_eq!(
            fixed(ratio),
            resolved_line(&cell, "contrast.computed"),
            "{}'s recorded contrast must be the solver's own number",
            cell.id
        );
        assert!(ratio >= 4.5, "{} reads at {ratio:.2}:1", cell.id);
    }
}

#[test]
fn the_error_on_white_pair_clears_the_floor_the_spec_prints_but_is_not_the_figure_it_prints() {
    // §2.5 prints `7.00:1` for the inverted editor's error ink on white.
    // The crate's own solver says `7.14:1`. The requirement is a floor, so
    // the cell satisfies it either way; the drift is recorded in the cell
    // rather than reconciled, because the printed column is not the gate.
    let cell = &editor_palette_cells()[1];

    assert_eq!(
        resolved_line(cell, "contrast.spec-figure"),
        "7.00:1 (spec 2.5)"
    );
    assert_eq!(
        resolved_line(cell, "contrast.computed"),
        "7.14",
        "the solver moved off the value this test records"
    );
    assert!(
        number(cell, "contrast.computed") >= 7.0,
        "the floor §2.5 asks for is 7.00:1"
    );
}

// --- 4.2 — the compact control-height rung (§3.1, §3.2) --------------------

#[test]
fn the_two_rungs_are_the_two_control_tokens_the_spec_prints() {
    assert_eq!(Rung::Standard.height(), size::CONTROL_MD);
    assert_eq!(Rung::Compact.height(), size::CONTROL_SM);
    assert_eq!(Rung::Standard.token(), "size.control.md");
    assert_eq!(Rung::Compact.token(), "size.control.sm");
    assert!(
        Rung::Compact.height() < Rung::Standard.height(),
        "the compact rung must be the shorter box"
    );
    assert_eq!(
        Rung::Standard.slug(),
        "",
        "the standard rung keeps the ids the gallery has always emitted"
    );
    assert_eq!(Rung::Compact.slug(), "@compact");
    assert_eq!(RUNGS, [Rung::Standard, Rung::Compact]);
}

#[test]
fn every_atom_state_is_snapshotted_at_both_rungs() {
    let cells = matrix();

    for variant in BUTTON_VARIANTS {
        for state in snapshot_states() {
            let base = format!("button/{}/{}", slug(variant), state_slug(state));
            assert_rung_pair(&cells, &base);
        }
    }
    for kind in INPUT_KINDS {
        for state in snapshot_states() {
            let base = format!("input/{}/{}", slug_kind(kind), state_slug(state));
            assert_rung_pair(&cells, &base);
        }
    }
}

/// Assert one atom's standard cell and its compact twin agree on style and
/// differ only in the box.
fn assert_rung_pair(cells: &[Cell], base: &str) {
    let standard = cells
        .iter()
        .find(|cell| cell.id == base)
        .unwrap_or_else(|| panic!("no standard cell for {base}"));
    let compact = cells
        .iter()
        .find(|cell| cell.id == format!("{base}@compact"))
        .unwrap_or_else(|| panic!("no compact cell for {base}"));

    assert_eq!(
        fixed(standard.frame.viewport[3]),
        fixed(size::CONTROL_MD),
        "{base} is not the standard rung"
    );
    assert_eq!(
        fixed(compact.frame.viewport[3]),
        fixed(size::CONTROL_SM),
        "{base}@compact is not the compact rung"
    );
    assert_eq!(
        resolved_line(standard, "geometry.height-token"),
        "size.control.md"
    );
    assert_eq!(
        resolved_line(compact, "geometry.height-token"),
        "size.control.sm"
    );
    assert_eq!(
        colour_channels(standard),
        colour_channels(compact),
        "{base}@compact changed a colour: the rung is geometry, not palette"
    );
}

#[test]
fn the_compact_rung_is_a_geometry_dimension_the_resolvers_never_see() {
    // A resolver takes no height: the same call at either rung must return
    // the same style. If this ever fails, the rung leaked into resolution
    // and the two cells are no longer the same control at two sizes.
    let state = VisualState {
        base: BaseState::Rest,
        focused: false,
    };
    for variant in BUTTON_VARIANTS {
        let standard = button_cell_at(variant, state, Rung::Standard);
        let compact = button_cell_at(variant, state, Rung::Compact);
        assert_eq!(
            colour_channels(&standard),
            colour_channels(&compact),
            "button/{} resolution depends on the rung",
            slug(variant)
        );
    }
}

// --- 4.3 — the modal's header / banner / footer bands (§4.6.1) -------------

#[test]
fn the_band_cells_split_the_card_into_ordered_disjoint_regions() {
    for cell in modal_band_cells() {
        let card = edges(quad(&cell, "card"));
        let header = edges(quad(&cell, "header-band"));
        let body = edges(quad(&cell, "body-band"));
        let footer = edges(quad(&cell, "footer-band"));

        assert!(header.3 <= body.1 + GEOM_EPS, "header overlaps the body");
        assert!(body.3 <= footer.1 + GEOM_EPS, "body overlaps the footer");
        for region in [header, body, footer] {
            assert!(region.1 >= card.1 - GEOM_EPS && region.3 <= card.3 + GEOM_EPS);
            assert!(
                (region.0 - card.0).abs() <= 1.0 && (region.2 - card.2).abs() <= 1.0,
                "bands must span the card inside its border"
            );
        }
    }
}

#[test]
fn the_band_heights_are_composed_from_the_frames_own_paddings() {
    let frame = ModalFrame::resolve(&Palette::dark());
    let cell = &modal_band_cells()[0];

    assert_eq!(
        quad(cell, "header-band").bounds[3],
        frame.content_padding * 2.0 + frame.close_target,
        "the header band is the close target between two content paddings"
    );
    assert_eq!(
        quad(cell, "footer-band").bounds[3],
        frame.footer_padding_vertical * 2.0 + size::CONTROL_MD,
        "the footer band is one standard control between two footer paddings"
    );
    assert_eq!(
        resolved_line(cell, "band.header.composition"),
        "content_padding + close_target + content_padding"
    );
    assert_eq!(
        resolved_line(cell, "band.footer.composition"),
        "footer_padding_vertical x2 + size.control.md"
    );
}

#[test]
fn the_banner_region_exists_only_when_the_modal_carries_an_error() {
    let without = &modal_band_cells()[0];
    let with = &modal_band_cells()[1];

    assert_eq!(resolved_line(without, "banner-present"), "false");
    assert_eq!(resolved_line(with, "banner-present"), "true");
    assert!(
        quad_opt(without, "banner-band").is_none(),
        "a clean modal has no banner region to snapshot"
    );

    let banner = quad(with, "banner-band");
    assert_eq!(
        banner.bounds[3],
        ModalFrame::resolve(&Palette::dark()).content_padding * 2.0 + typography::BODY.line_height,
        "the banner band is one body line between two content paddings"
    );
    assert!(
        edges(quad(with, "body-band")).1 > edges(quad(without, "body-band")).1,
        "adding the banner must push the body down"
    );
    assert!(
        edges(banner).3 <= edges(quad(with, "body-band")).1 + GEOM_EPS,
        "the banner must not run into the body"
    );
    assert!(
        quad(with, "body-band").bounds[3] < quad(without, "body-band").bounds[3],
        "a clean modal reclaims the banner's space rather than leaving a gap"
    );
    assert_eq!(
        edges(quad(without, "body-band")).1,
        edges(quad(without, "header-band")).3,
        "with no banner the body starts at the header's own bottom edge"
    );
}

#[test]
fn the_band_dividers_are_the_resting_border_token_on_the_card_surface() {
    let palette = Palette::dark();
    for cell in modal_band_cells() {
        for layer in ["header-divider", "footer-divider"] {
            let rule = quad(&cell, layer);
            assert_eq!(rule.border, "none", "{layer} is a fill, not a stroke");
            assert_eq!(
                fixed(rule.bounds[3]),
                fixed(shape::BORDER_HAIRLINE),
                "{layer} must be one hairline thick"
            );
            assert!(rule.fill.starts_with("border.subtle"));
        }
        for channel in ["divider.header", "divider.footer"] {
            let pair = audit_pair(&cell, channel);
            assert_eq!(pair.foreground, palette.border_subtle);
            assert_eq!(pair.background, palette.surface_overlay);
            assert_eq!(
                measure(pair).2,
                AuditClass::RestingDivider,
                "{channel} must be classified as a resting divider, not waved through"
            );
        }
    }
}

#[test]
fn each_band_audits_its_own_foreground_against_the_surface_it_sits_on() {
    let palette = Palette::dark();
    let with = &modal_band_cells()[1];

    assert_eq!(
        audit_pair(with, "header.title").foreground,
        palette.text_strong
    );
    assert_eq!(
        audit_pair(with, "header.close-glyph").foreground,
        palette.text_muted
    );
    assert_eq!(
        audit_pair(with, "footer.status").foreground,
        palette.text_muted
    );
    assert_eq!(
        audit_pair(with, "banner.text").foreground,
        palette.status_danger_text
    );
    for channel in [
        "header.title",
        "header.close-glyph",
        "footer.status",
        "banner.text",
    ] {
        let pair = audit_pair(with, channel);
        assert_eq!(pair.background, palette.surface_overlay, "{channel}");
        assert_eq!(measure(pair).2, AuditClass::Judged, "{channel}");
        assert!(
            measure(pair).0 >= 4.5,
            "{channel} reads at {:.2}:1",
            measure(pair).0
        );
    }
}

// --- 4.4 — menu separators and glyph placement (§4.8.1, §4.9.2) -----------

#[test]
fn the_separator_is_a_hairline_rule_inside_its_own_band() {
    let cell = menu_glyph_cells()
        .into_iter()
        .find(|cell| cell.id == "menu-glyph/separator")
        .expect("the separator cell exists");

    let rule = quad(&cell, "separator-rule");
    assert_eq!(
        number(&cell, "separator.thickness"),
        menu_view::SEPARATOR_THICKNESS
    );
    assert_eq!(number(&cell, "separator.band"), menu_view::SEPARATOR_HEIGHT);
    assert_eq!(fixed(rule.bounds[3]), fixed(menu_view::SEPARATOR_THICKNESS));
    assert!(rule.fill.starts_with("border.subtle"));

    let band = (
        0.0,
        0.0,
        cell.frame.viewport[2],
        number(&cell, "separator.band"),
    );
    let (left, top, right, bottom) = edges(rule);
    assert!(
        left >= band.0
            && right <= band.2 + GEOM_EPS
            && top >= band.1
            && bottom <= band.3 + GEOM_EPS,
        "the rule must lie inside the band it was given"
    );
    assert!(
        top > 0.0 && bottom < band.3,
        "the rule must be centred in its band, not flush with it"
    );
    assert_eq!(
        measure(audit_pair(&cell, "separator")).2,
        AuditClass::RestingDivider,
        "a resting separator is a resting divider, not a judged pair"
    );
}

#[test]
fn every_leading_glyph_is_placed_inside_the_leading_slot() {
    for cell in menu_glyph_cells() {
        let Some(glyph) = quad_opt(&cell, "leading-glyph") else {
            continue;
        };
        let pad = number(&cell, "row.padding-x");
        let slot = number(&cell, "leading-slot.width");
        let (left, _, right, _) = edges(glyph);

        assert!(
            (left - pad).abs() <= GEOM_EPS,
            "{}'s leading glyph starts at {left}, the row pads at {pad}",
            cell.id
        );
        assert!(
            right <= pad + slot + GEOM_EPS,
            "{}'s leading glyph overruns the {}px slot",
            cell.id,
            slot
        );
        assert_eq!(
            fixed(glyph.bounds[2]),
            fixed(slot),
            "{}'s glyph must fill the slot it is placed in",
            cell.id
        );
    }
}

#[test]
fn every_trailing_glyph_is_right_aligned_inside_the_row() {
    for cell in menu_glyph_cells() {
        let Some(glyph) = quad_opt(&cell, "trailing-glyph") else {
            continue;
        };
        let pad = number(&cell, "row.padding-x");
        let width = cell.frame.viewport[2];
        let (left, _, right, _) = edges(glyph);

        assert!(
            (width - pad - right).abs() <= GEOM_EPS,
            "{}'s trailing glyph must end one pad from the row's right edge",
            cell.id
        );
        assert!(
            left >= width / 2.0,
            "{}'s trailing glyph must sit in the trailing half",
            cell.id
        );
    }
}

#[test]
fn the_chevron_is_muted_at_rest_and_accented_only_while_its_branch_is_open() {
    let palette = Palette::dark();
    let rest = menu_glyph_cells()
        .into_iter()
        .find(|c| c.id == "menu-glyph/submenu-chevron")
        .expect("the resting chevron cell exists");
    let open = menu_glyph_cells()
        .into_iter()
        .find(|c| c.id == "menu-glyph/submenu-chevron-open")
        .expect("the open chevron cell exists");

    assert_eq!(
        audit_pair(&rest, "trailing-glyph").foreground,
        palette.text_muted,
        "a closed branch is not the thing the menu is pointing at"
    );
    assert_eq!(
        audit_pair(&open, "trailing-glyph").foreground,
        palette.accent_text,
        "an open branch's chevron must carry the accent"
    );
    for cell in [&rest, &open] {
        let pair = audit_pair(cell, "trailing-glyph");
        assert_eq!(pair.role, ContrastRole::Text);
        assert_eq!(measure(pair).2, AuditClass::Judged, "{}", cell.id);
        assert!(
            measure(pair).0 >= 4.5,
            "{}'s chevron reads at {:.2}:1",
            cell.id,
            measure(pair).0
        );
    }
}

#[test]
fn a_resting_row_measures_its_glyph_against_the_menu_card_not_against_nothing() {
    // A resting row paints no fill of its own. Recording the glyph as
    // `UnpaintedBackground` would let an unreadable checkmark pass; the
    // cell measures it against `surface.overlay`, the surface the card
    // actually puts under it.
    let cell = menu_glyph_cells()
        .into_iter()
        .find(|c| c.id == "menu-glyph/check-on")
        .expect("the checked row exists");
    let pair = audit_pair(&cell, "leading-glyph");

    assert!(quad_opt(&cell, "row-background").is_none());
    assert_eq!(pair.background, Palette::dark().surface_overlay);
    assert_eq!(
        measure(pair).2,
        AuditClass::Judged,
        "a glyph over a card surface is judged, not exempt"
    );
}

#[test]
fn a_hovered_row_measures_its_glyph_against_the_hover_fill() {
    let cell = menu_glyph_cells()
        .into_iter()
        .find(|c| c.id == "menu-glyph/check-on-hovered")
        .expect("the hovered checked row exists");
    let pair = audit_pair(&cell, "leading-glyph");

    assert!(
        quad(&cell, "row-background")
            .fill
            .starts_with("surface.hover")
    );
    assert_eq!(pair.background, Palette::dark().surface_hover);
    assert_eq!(measure(pair).2, AuditClass::Judged);
}

// --- 4.5 — the input dialog's per-row bodies (§4.5.1) ----------------------

#[test]
fn every_row_body_lies_between_the_card_top_and_the_footer_buttons() {
    for cell in input_dialog_row_cells() {
        let card = edges(quad(&cell, "card"));
        let cancel = edges(quad(&cell, "footer-cancel"));
        let primary = edges(quad(&cell, "footer-primary"));
        let footer_top = cancel.1.min(primary.1);
        let footer_bottom = cancel.3.max(primary.3);

        assert!(
            footer_top >= card.1 && footer_bottom <= card.3 + GEOM_EPS,
            "{}'s footer buttons must sit inside the card",
            cell.id
        );
        for layer in ["row-title", "row-context"] {
            let row = edges(quad(&cell, layer));
            assert!(
                row.1 >= card.1 && row.3 <= footer_top + GEOM_EPS,
                "{}'s {layer} must sit between the card top and the buttons",
                cell.id
            );
        }
    }
}

#[test]
fn the_row_bodies_are_stacked_without_overlapping_each_other() {
    for cell in input_dialog_row_cells() {
        let mut rows: Vec<(f32, f32)> = cell
            .frame
            .quads
            .iter()
            .filter(|q| q.layer.starts_with("row-") || q.layer.starts_with("folder-row-"))
            .map(|q| (q.bounds[1], q.bounds[1] + q.bounds[3]))
            .collect();
        rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        for window in rows.windows(2) {
            let (_, previous_bottom) = window[0];
            let (next_top, _) = window[1];
            assert!(
                previous_bottom <= next_top + GEOM_EPS,
                "{} overlaps two row bodies",
                cell.id
            );
        }
    }
}

#[test]
fn the_folder_list_is_the_compact_rung_and_the_well_caps_it() {
    let cell = input_dialog_row_cells()
        .into_iter()
        .find(|c| c.id == "input-dialog-rows/folder-list-capped")
        .expect("the capped folder list exists");
    let well = quad(&cell, "list-well");

    assert_eq!(
        number(&cell, "folder-row.height"),
        input_dialog::FOLDER_ROW_HEIGHT
    );
    assert_eq!(
        number(&cell, "folder-row.height"),
        size::CONTROL_SM,
        "a folder row is the compact rung, not a third height"
    );
    assert_eq!(
        number(&cell, "folder-list.max-height"),
        input_dialog::FOLDER_LIST_MAX_HEIGHT
    );
    assert_eq!(number(&cell, "folder-rows.total"), 12.0);
    assert_eq!(
        number(&cell, "folder-rows.visible"),
        7.0,
        "the well shows floor(max / row) rows and no more"
    );
    assert_eq!(
        fixed(well.bounds[3]),
        fixed(input_dialog::FOLDER_LIST_MAX_HEIGHT)
    );

    let mut previous_bottom = well.bounds[1];
    for index in 0..7 {
        let row = quad(&cell, &format!("folder-row-{index}"));
        assert_eq!(fixed(row.bounds[3]), fixed(input_dialog::FOLDER_ROW_HEIGHT));
        assert_eq!(
            fixed(row.bounds[1]),
            fixed(previous_bottom),
            "folder row {index} must butt against the one above it"
        );
        previous_bottom = row.bounds[1] + row.bounds[3];
    }
    assert!(
        previous_bottom <= well.bounds[1] + well.bounds[3] + GEOM_EPS,
        "the visible rows must fit inside the well"
    );
}

#[test]
fn the_row_cells_differ_only_in_the_control_row_the_kind_brings() {
    let cells = input_dialog_row_cells();
    let title = |cell: &Cell| resolved_line(cell, "geometry.title");
    let context = |cell: &Cell| resolved_line(cell, "geometry.context");

    // Every dialog opens with the same title and context rows; only the
    // body between the context row and the footer moves.
    for cell in &cells {
        assert_eq!(
            title(cell),
            title(&cells[0]),
            "{}'s title row moved",
            cell.id
        );
        assert_eq!(
            context(cell),
            context(&cells[0]),
            "{}'s context row moved",
            cell.id
        );
    }
    // The control row is the only body row whose depth the kind changes,
    // and the footer follows it rather than overlapping it.
    let controls: Vec<String> = cells
        .iter()
        .map(|cell| resolved_line(cell, "geometry.control"))
        .collect();
    assert!(
        controls.windows(2).any(|pair| pair[0] != pair[1]),
        "the control row must actually differ in depth between kinds"
    );
    for cell in &cells {
        let control = parse_rect(&resolved_line(cell, "geometry.control"));
        let footer = parse_rect(&resolved_line(cell, "geometry.footer"));
        assert!(
            control.1 + control.3 <= footer.1 + GEOM_EPS,
            "{}'s control row runs into the footer",
            cell.id
        );
    }
}

// --- 4.6 — the movable overlay's trailing actions (§4.7.1) -----------------

#[test]
fn the_trailing_action_cluster_is_right_aligned_inside_the_title_bar() {
    for cell in movable_action_cells() {
        let bar = edges(quad(&cell, "title-bar"));
        let cluster = edges(quad(&cell, "action-cluster"));

        assert!(
            (cluster.3 - bar.3).abs() <= GEOM_EPS && cluster.1 >= bar.1,
            "{}'s action cluster must span the title bar's height",
            cell.id
        );
        assert!(
            (bar.2 - cluster.2 - TITLE_BAR_PADDING_HORIZONTAL).abs() <= GEOM_EPS,
            "{}'s cluster must sit one title-bar pad from the right edge",
            cell.id
        );
    }
}

#[test]
fn the_action_cluster_width_is_derived_from_the_action_geometry() {
    for cell in movable_action_cells() {
        let size = number(&cell, "action.control-size");
        let gap = number(&cell, "action.gap");
        let derived = number(&cell, "action.derived-width");

        assert_eq!(size, ACTION_CONTROL_SIZE);
        assert_eq!(gap, ACTION_GAP);
        assert_eq!(
            derived,
            ACTION_CONTROL_SIZE * 2.0 + ACTION_GAP,
            "two controls and the gap between them"
        );
        assert_eq!(
            fixed(quad(&cell, "action-cluster").bounds[2]),
            fixed(derived),
            "{}'s cluster must be exactly the derived width",
            cell.id
        );
        assert_eq!(
            resolved_line(&cell, "action.width-matches-host"),
            "true",
            "{}'s host disagrees with the gallery's derivation",
            cell.id
        );
    }
}

#[test]
fn the_drag_zone_stops_where_the_action_cluster_begins() {
    for cell in movable_action_cells() {
        let zone = edges(quad(&cell, "drag-zone"));
        let cluster = edges(quad(&cell, "action-cluster"));

        assert!(
            zone.2 <= cluster.0 + GEOM_EPS,
            "{}'s drag zone would swallow the action cluster",
            cell.id
        );
        assert_eq!(resolved_line(&cell, "drag-zone.ends-at-cluster"), "true");
        assert!(
            zone.2 > cluster.0 - 1.0,
            "{}'s drag zone must reach the cluster, not stop short",
            cell.id
        );
    }
}

#[test]
fn each_action_glyph_is_its_own_hit_box_in_the_meta_ink() {
    let palette = Palette::dark();
    for cell in movable_action_cells() {
        let dock = quad(&cell, "action-dock");
        let close = quad(&cell, "action-close");

        for action in [dock, close] {
            assert_eq!(
                fixed(action.bounds[2]),
                fixed(ACTION_CONTROL_SIZE),
                "{}'s action box must be the hit-target minimum",
                cell.id
            );
            assert_eq!(fixed(action.bounds[3]), fixed(ACTION_CONTROL_SIZE));
            assert!(action.fill.starts_with("transparent"));
            assert!(action.border.starts_with("text.muted"));
        }
        assert!(
            edges(close).0 >= edges(dock).2 + ACTION_GAP - GEOM_EPS,
            "{}'s actions must not crowd each other",
            cell.id
        );
        assert_eq!(
            audit_pair(&cell, "action.glyph").foreground,
            palette.text_muted
        );
    }
}

// ---------------------------------------------------------------------------
// 5 — scope honesty (D-2)
// ---------------------------------------------------------------------------

#[test]
fn the_not_owned_cells_are_the_three_named_surfaces_and_nothing_else() {
    let cells: Vec<Cell> = matrix()
        .into_iter()
        .filter(|cell| cell.kind == CellKind::NotOwned)
        .collect();

    let ids: Vec<&str> = cells.iter().map(|cell| cell.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["navbar/not-owned", "panes/not-owned", "tabs/not-owned"]
    );
}

#[test]
fn every_not_owned_cell_carries_a_justification_and_draws_nothing() {
    for cell in not_owned_cells() {
        let note = cell
            .note
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no justification note", cell.id));
        assert!(
            note.contains("D-2"),
            "{}'s note must cite D-2, got: {note}",
            cell.id
        );
        assert!(
            note.len() > 80,
            "{}'s note is too thin to be a justification",
            cell.id
        );
        assert!(
            cell.frame.quads.is_empty(),
            "{} must not paint a frame it does not own",
            cell.id
        );
        assert!(cell.resolved.is_empty());
        assert!(cell.audit.is_empty());
    }
}

#[test]
fn every_rendered_cell_resolves_channels_and_audit_pairs() {
    for cell in matrix() {
        if cell.kind == CellKind::NotOwned {
            continue;
        }
        assert!(!cell.resolved.is_empty(), "{} resolved nothing", cell.id);
        if paints_nothing(&cell) {
            continue;
        }
        assert!(!cell.audit.is_empty(), "{} audited nothing", cell.id);
    }
}

/// Whether a cell's own resolver declared that it paints nothing.
///
/// A `Count(0)` badge is invisible by design, and an invisible thing has no
/// contrast to measure. The exemption is keyed on the resolver's own
/// `painted` declaration rather than on a list of cell ids so that a cell
/// cannot opt out of the audit by being renamed, and so the opt-out is
/// visible in the manifest next to the cell that claims it.
fn paints_nothing(cell: &Cell) -> bool {
    cell.resolved.iter().any(|entry| {
        entry.channel == "painted"
            && matches!(&entry.value, StyleValue::Text(text) if text == "false")
    })
}

/// A cell may legitimately draw zero quads, and that is not the same thing
/// as being `NotOwned`.
///
/// A menu row at Rest paints no background at all — the resolver returns
/// `None` because there is no highlight to draw — and a Ghost button draws
/// a transparent one. Both are the resolvers' own answers, recorded as
/// given. What the gallery must never do is turn a surface it does not own
/// into a cell that looks rendered, which is what the `NotOwned` kind is
/// for and what the test above pins.
#[test]
fn a_resting_cell_that_paints_nothing_is_still_a_rendered_cell() {
    let cells = matrix();
    let empty: Vec<&Cell> = cells
        .iter()
        .filter(|cell| cell.kind != CellKind::NotOwned && cell.frame.quads.is_empty())
        .collect();

    for cell in &empty {
        assert!(
            !cell.resolved.is_empty(),
            "{} drew nothing and resolved nothing, which is indistinguishable from NotOwned",
            cell.id
        );
    }

    assert!(
        empty.iter().any(|cell| cell.id == "menu/row/rest"),
        "the menu Rest row is expected to paint no background layer"
    );
}

// ---------------------------------------------------------------------------
// 5 — the z-band stack (G-13)
// ---------------------------------------------------------------------------

#[test]
fn the_zband_cell_paints_overlay_then_menu_then_scrim_then_modal() {
    let cell = zband_cell();
    let layers: Vec<&str> = cell
        .frame
        .quads
        .iter()
        .map(|quad| quad.layer.as_str())
        .collect();

    assert_eq!(
        layers,
        vec![
            "band1-overlay-card",
            "band2-menu-card",
            "band3-scrim",
            "band3-modal-card"
        ]
    );
}

#[test]
fn the_band_three_scrim_is_painted_over_the_band_two_menu_card() {
    let cell = zband_cell();
    let index = |wanted: &str| {
        cell.frame
            .quads
            .iter()
            .position(|quad| quad.layer == wanted)
            .unwrap_or_else(|| panic!("no {} quad", wanted))
    };

    assert!(
        index("band3-scrim") > index("band2-menu-card"),
        "G-13: the Modal band dims the menu as well as Bands 0 and 1"
    );
    assert!(index("band3-scrim") > index("band1-overlay-card"));
}

#[test]
fn escape_order_is_not_band_order() {
    assert_eq!(ZBand::Menu.index(), 2);
    assert_eq!(ZBand::Modal.index(), 3);
    assert!(
        ZBand::Menu.escape_rank() > ZBand::Modal.escape_rank(),
        "G-13: Menu unwinds before Modal even though it paints below it"
    );
    assert_eq!(ZBand::UtilityOverlay.escape_rank(), 1);
    assert_eq!(ZBand::Workspace.escape_rank(), 0);
}

#[test]
fn the_zband_escape_ranks_are_read_from_a_real_registry() {
    let cell = zband_cell();
    assert_eq!(resolved_line(&cell, "highest-open-band"), "Some(Modal)");
    assert_eq!(resolved_line(&cell, "topmost"), "zband/modal");
    assert_eq!(resolved_line(&cell, "escape-rank.menu"), "3");
    assert_eq!(resolved_line(&cell, "escape-rank.modal"), "2");
}

#[test]
fn the_zband_cell_states_the_finding_in_its_note() {
    let cell = zband_cell();
    let note = cell.note.expect("the z-band cell must state G-13");
    assert!(note.contains("G-13"));
    assert!(note.contains("paint order"));
    assert!(note.contains("escape order"));
}

// ---------------------------------------------------------------------------
// 6 — the gates
// ---------------------------------------------------------------------------

#[test]
fn no_cell_emits_a_colour_the_token_set_does_not_account_for() {
    let unmapped = unmapped_colours(&matrix());
    assert!(
        unmapped.is_empty(),
        "unmapped colours:\n{}",
        unmapped.join("\n")
    );
}

#[test]
fn the_contrast_ledger_is_empty() {
    let failures = contrast_failures(&matrix());
    assert!(
        failures.is_empty(),
        "accepted contrast failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_judged_pair_clears_its_role_threshold() {
    let mut judged = 0usize;
    for cell in matrix() {
        for pair in &cell.audit {
            let (ratio, _, class) = measure(pair);
            if class != AuditClass::Judged {
                continue;
            }
            judged += 1;
            assert!(
                ratio >= threshold(pair.role),
                "{} {} {} on {} measures {ratio} against a floor of {}",
                cell.id,
                pair.channel,
                token_name(pair.foreground),
                token_name(pair.background),
                threshold(pair.role)
            );
        }
    }
    assert!(judged > 100, "the audit judged only {judged} pairs");
}

#[test]
fn the_role_predicates_agree_with_the_measured_verdicts() {
    for cell in matrix() {
        for pair in &cell.audit {
            let (ratio, painted, class) = measure(pair);
            if class != AuditClass::Judged {
                continue;
            }
            let predicate = match pair.role {
                ContrastRole::Text => contrast::meets_text_contrast(pair.foreground, painted),
                ContrastRole::Chrome => contrast::meets_chrome_contrast(pair.foreground, painted),
            };
            assert_eq!(
                predicate,
                ratio >= threshold(pair.role),
                "{} {} : the role predicate and the threshold disagree",
                cell.id,
                pair.channel
            );
        }
    }
}

#[test]
fn the_unpainted_background_class_only_covers_truly_unpainted_pairs() {
    for cell in matrix() {
        for pair in &cell.audit {
            let (_, _, class) = measure(pair);
            if class == AuditClass::UnpaintedBackground {
                assert_eq!(
                    pair.background.a, 0.0,
                    "{} {} is classed unpainted but its background has alpha {}",
                    cell.id, pair.channel, pair.background.a
                );
            }
        }
    }
}

#[test]
fn the_coincident_class_only_covers_strokes_that_share_their_fill() {
    for cell in matrix() {
        for pair in &cell.audit {
            let (_, _, class) = measure(pair);
            if class == AuditClass::CoincidentWithFill {
                assert!(
                    same_channels(pair.foreground, pair.background),
                    "{} {} is classed coincident but its channels differ",
                    cell.id,
                    pair.channel
                );
            }
        }
    }
}

#[test]
fn the_resting_divider_class_only_covers_border_subtle() {
    let palette = Palette::dark();
    for cell in matrix() {
        for pair in &cell.audit {
            let (_, _, class) = measure(pair);
            if class == AuditClass::RestingDivider {
                assert!(
                    same_channels(pair.foreground, palette.border_subtle),
                    "{} {} is classed a resting divider but is not border.subtle",
                    cell.id,
                    pair.channel
                );
            }
        }
    }
}

/// Both focus indicators are active chrome and are judged by the same
/// rules, whichever channel they arrive on: the 2px outward ring the
/// button and text-input families emit, and the 1px inset outline the
/// menu row (and any app-owned row) emits.
fn is_focus_channel(channel: &str) -> bool {
    channel == "ring" || channel == "focus-outline"
}

#[test]
fn a_focus_ring_is_exempt_only_where_nothing_is_painted_behind_it() {
    // A focus ring is active chrome, so the only exemption it may ever
    // take is the one that says the measurement cannot be made: nothing
    // opaque was painted behind it. A Ghost at Rest is exactly that case
    // — its ring belongs to whatever the button is mounted to, and the
    // gallery cannot see the container. What must never happen is a ring
    // being waved through as a resting divider or a coincident stroke,
    // because those exemptions describe a stroke that deliberately does
    // not signal.
    let mut judged = 0usize;
    let mut exempt = 0usize;

    for cell in matrix() {
        for pair in &cell.audit {
            if !is_focus_channel(pair.channel) {
                continue;
            }
            assert_eq!(pair.role, ContrastRole::Chrome, "{} ring role", cell.id);

            let (_, _, class) = measure(pair);
            match class {
                AuditClass::Judged => judged += 1,
                AuditClass::UnpaintedBackground => {
                    assert_eq!(
                        pair.background.a, 0.0,
                        "{}'s ring was exempted as unpainted but its background has alpha {}",
                        cell.id, pair.background.a
                    );
                    exempt += 1;
                }
                other => panic!("{}'s focus ring was exempted as {:?}", cell.id, other),
            }
        }
    }

    assert!(judged > 0, "no focus ring was judged at all");
    assert!(
        exempt > 0,
        "the Ghost ring-over-nothing case is not covered"
    );
}

#[test]
fn every_ring_on_an_opaque_background_is_judged_against_the_chrome_floor() {
    for cell in matrix() {
        for pair in &cell.audit {
            if !is_focus_channel(pair.channel) || pair.background.a < 1.0 {
                continue;
            }
            let (ratio, _, class) = measure(pair);
            assert_eq!(class, AuditClass::Judged, "{} ring class", cell.id);
            assert!(
                ratio >= contrast::MIN_CHROME_CONTRAST,
                "{} ring {} on {} measures {ratio}",
                cell.id,
                token_name(pair.foreground),
                token_name(pair.background)
            );
        }
    }
}

#[test]
fn the_manifest_carries_its_own_pass_conditions() {
    let text = manifest();
    assert!(text.contains("gate: token mapping (UIF-VER-001)"));
    assert!(text.contains("gate: contrast (UIF-TOK-006)"));
    assert!(text.contains("gate: scope (D-2)"));
    assert!(text.contains("PASS - every emitted colour names a Palette token"));
    assert!(text.contains("PASS - every judged pair clears its role threshold"));
    assert!(text.contains("3 surfaces emitted as NotOwned rather than omitted"));
    assert!(!text.contains("FAIL"));
}

#[test]
fn the_caret_is_recorded_as_permanently_outside_the_snapshot_surface() {
    // The spec's editor row names a caret. The recorder cannot see it, so
    // the manifest says so instead of leaving the reader to notice the
    // omission and assume it was covered.
    assert_eq!(
        SURFACE_EXCLUSIONS.len(),
        1,
        "the exclusion list is written out, not counted"
    );
    assert_eq!(SURFACE_EXCLUSIONS[0].0, "text.caret");
    assert!(SURFACE_EXCLUSIONS[0].1.contains("renderer protocol"));

    let text = manifest();
    assert!(
        text.contains("recorded exclusions: named channels with no frame"),
        "the manifest must carry the exclusion section"
    );
    assert!(text.contains("1 channels recorded as unreachable from a snapshot"));
    assert!(text.contains("text.caret"));
}

#[test]
fn the_manifest_records_every_cell_snapshot() {
    let text = manifest();
    for cell in matrix() {
        assert!(
            text.contains(&format!("cell: {}\n", cell.id)),
            "the manifest omits {}",
            cell.id
        );
    }
}
