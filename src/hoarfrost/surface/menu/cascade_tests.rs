//! Proofs about the cascade controller: single-tree mutual exclusion,
//! persistent parents, sibling replacement, the §4.9.3 keyboard walk,
//! close-policy handling, click-away, focus return and trap confinement
//! (`UIF-CMENU-002`, `UIF-CMENU-003`, `UIF-FMENU-002`, `UIF-FMENU-003`).

use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::focus::GroupId;
use crate::surface::menu::cascade::CascadeOutcome;
use crate::surface::menu::cascade::FocusTransition;
use crate::surface::menu::cascade::IgnoredReason;
use crate::surface::menu::cascade::MenuCascade;
use crate::surface::menu::cascade::MenuKey;
use crate::surface::menu::cascade::MenuTreeKind;
use crate::surface::menu::cascade::SubmenuTrigger;
use crate::surface::menu::model::ActivationEffect;
use crate::surface::menu::model::ClosePolicy;
use crate::surface::menu::model::Menu;
use crate::surface::menu::model::MenuId;
use crate::surface::menu::model::Row;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// §4.9.1 "Single File (Tree)".
///
/// Navigable rows: 1, 2, 3, 6, 7, 8, 11, 12, 13, 14, 17. Everything else
/// is a heading or a separator.
fn single_file_menu() -> Menu {
    Menu::new(
        "ctx-file",
        vec![
            Row::heading("Open & Output"),
            Row::action("edit", "Edit"),
            Row::action("open-editor", "Open in Editor"),
            Row::action("print", "Print"),
            Row::separator(),
            Row::heading("AI & Automation"),
            Row::action("run-prompt", "Run as prompt"),
            Row::action("format-markdown", "Format Markdown"),
            Row::submenu(
                "note-skills",
                "Note Skills",
                Menu::new("skills", vec![Row::action("summarise", "Summarise")]),
            ),
            Row::separator(),
            Row::heading("File Management"),
            Row::action("rename", "Rename...").with_shortcut("F2"),
            Row::action("move", "Move..."),
            Row::action("copy-path", "Copy path"),
            Row::action("show-in-explorer", "Show in File Explorer"),
            Row::separator(),
            Row::heading("Destructive"),
            Row::action("delete", "Delete...").destructive(),
        ],
    )
}

/// A card whose furniture and disabled rows are interleaved with the live
/// ones, so a walk cannot pass by accident.
///
/// Navigable rows: 1, 4, 7.
fn skip_menu() -> Menu {
    Menu::new(
        "skip",
        vec![
            Row::heading("Top"),
            Row::action("a", "A"),
            Row::separator(),
            Row::action("b", "B").disabled(),
            Row::action("c", "C"),
            Row::heading("Middle"),
            Row::action("d", "D").disabled(),
            Row::action("e", "E"),
            Row::separator(),
        ],
    )
}

/// The same card with row 1 disabled as well — the "a row became disabled
/// while focused" world. Navigable rows: 4, 7.
fn skip_menu_with_a_disabled() -> Menu {
    Menu::new(
        "skip",
        skip_menu()
            .rows()
            .iter()
            .cloned()
            .map(|row| {
                if row.key() == "a" {
                    row.disabled()
                } else {
                    row
                }
            })
            .collect(),
    )
}

/// Three submenu levels deep, with furniture at each level.
///
/// * `nav` — navigable 0, 1, 2
/// * `nav/l1` — navigable 1, 2
/// * `nav/l1/l2` — navigable 0, 1
/// * `nav/l1/l2/l3` — navigable 0
fn deep_menu() -> Menu {
    Menu::new(
        "nav",
        vec![
            Row::action("plain", "Plain Action"),
            Row::submenu(
                "l1",
                "Level 1...",
                Menu::new(
                    "placeholder-1",
                    vec![
                        Row::heading("Level one"),
                        Row::action("l1-action", "Level one action"),
                        Row::submenu(
                            "l2",
                            "Level 2...",
                            Menu::new(
                                "placeholder-2",
                                vec![
                                    Row::action("l2-action", "Level two action"),
                                    Row::submenu(
                                        "l3",
                                        "Level 3...",
                                        Menu::new(
                                            "placeholder-3",
                                            vec![Row::action("l3-action", "Level three action")],
                                        ),
                                    ),
                                ],
                            ),
                        ),
                    ],
                ),
            ),
            Row::action("last", "Last Row"),
        ],
    )
}

/// `deep_menu` with the whole first-level branch rendered inert — every row
/// in `nav/l1` is furniture or disabled.
fn deep_menu_with_l1_inert() -> Menu {
    Menu::new(
        "nav",
        vec![
            Row::action("plain", "Plain Action"),
            Row::submenu(
                "l1",
                "Level 1...",
                Menu::new(
                    "placeholder-1",
                    vec![
                        Row::heading("Level one"),
                        Row::action("l1-action", "Level one action").disabled(),
                        Row::separator(),
                    ],
                ),
            ),
            Row::action("last", "Last Row"),
        ],
    )
}

/// `deep_menu`'s root with the submenu row demoted to a plain action, so
/// the open branch has nothing to hang from any more.
fn root_without_l1_submenu() -> Menu {
    Menu::new(
        "nav",
        vec![
            Row::action("plain", "Plain Action"),
            Row::action("l1", "Level 1..."),
            Row::action("last", "Last Row"),
        ],
    )
}

/// Two sibling submenu branches over a stay-open checkable.
///
/// Navigable rows: 0, 1, 2.
fn twins_menu() -> Menu {
    Menu::new(
        "twins",
        vec![
            Row::submenu(
                "a",
                "Branch A...",
                Menu::new(
                    "placeholder-a",
                    vec![
                        Row::action("a1", "A one"),
                        Row::submenu(
                            "a-deep",
                            "A deep...",
                            Menu::new("placeholder-deep", vec![Row::action("deep", "Deep")]),
                        ),
                    ],
                ),
            ),
            Row::submenu(
                "b",
                "Branch B...",
                Menu::new("placeholder-b", vec![Row::action("b1", "B one")]),
            ),
            Row::checkable("keep", "Keep Me Open", false),
        ],
    )
}

/// Two sibling branches hanging off a *deep* card, so a branch switch can
/// be anchored somewhere other than the root.
///
/// * `fork` — navigable 0, 1
/// * `fork/x` — navigable 0, 1, 2
/// * `fork/x/x2` — navigable 0
/// * `fork/x/y` — navigable 0
fn fork_menu() -> Menu {
    Menu::new(
        "fork",
        vec![
            Row::action("stem", "Stem"),
            Row::submenu(
                "x",
                "Branch X...",
                Menu::new(
                    "placeholder-x",
                    vec![
                        Row::action("x1", "X one"),
                        Row::submenu(
                            "x2",
                            "Branch X two...",
                            Menu::new("placeholder-x2", vec![Row::action("x2a", "X two action")]),
                        ),
                        Row::submenu(
                            "y",
                            "Branch Y...",
                            Menu::new("placeholder-y", vec![Row::action("y1", "Y one")]),
                        ),
                    ],
                ),
            ),
        ],
    )
}

/// A submenu parent whose child has nothing focusable in it.
///
/// Navigable rows of the root: 0, 1, 2.
fn dead_submenu_menu() -> Menu {
    Menu::new(
        "dead",
        vec![
            Row::action("ok", "Fine"),
            Row::submenu(
                "dead-branch",
                "Nothing Useful...",
                Menu::new(
                    "placeholder-dead",
                    vec![
                        Row::heading("Unavailable"),
                        Row::separator(),
                        Row::action("nope", "Not right now").disabled(),
                    ],
                ),
            ),
            Row::action("after", "After"),
        ],
    )
}

/// A card that is nothing but furniture.
fn furniture_only_menu() -> Menu {
    Menu::new("furniture", vec![Row::heading("Nothing"), Row::separator()])
}

/// Every close policy in one card.
///
/// Navigable rows: 0, 1, 2, 3, 4.
fn policy_menu() -> Menu {
    Menu::new(
        "policy",
        vec![
            Row::checkable("stay", "Stay open", false),
            Row::action("fire", "Fire and close"),
            Row::checkable("flip-close", "Close on toggle", true).close_on_toggle(),
            Row::radio("r1", "One", "mode", true),
            Row::radio("r2", "Two", "mode", false),
        ],
    )
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// The focus group a card's rows register under.
fn gid(card: &str) -> GroupId {
    GroupId::new(format!("menu:{card}"))
}

/// The focus id of a row.
fn fid(card: &str, key: &str) -> FocusId {
    FocusId::new(format!("menu:{card}#{key}"))
}

/// A [`MenuId`] from a bare path.
fn mid(path: &str) -> MenuId {
    MenuId::new(path)
}

/// The focused id as a plain string, so failures read as text.
fn focused(focus: &FocusState) -> Option<String> {
    focus.current().map(|id| id.as_str().to_owned())
}

/// The focused row key of one open card.
fn focused_key(cascade: &MenuCascade, card: &str) -> Option<String> {
    cascade
        .focused_row_in(&mid(card))
        .and_then(|row| cascade.card_menu(&mid(card)).and_then(|menu| menu.row(row)))
        .map(|row| row.key().to_owned())
}

/// Open a tree as a context menu and assert it actually opened.
fn open_context(cascade: &mut MenuCascade, menu: &Menu, focus: &mut FocusState) -> CascadeOutcome {
    let outcome = cascade.open(menu, MenuTreeKind::Context, focus);
    assert!(
        !outcome.is_ignored(),
        "opening `{}` must succeed: {outcome:?}",
        menu.id()
    );
    outcome
}

/// Drive a cascade down the full three-level `deep_menu` path.
///
/// Each hop needs a `Down` first: the row a card opens on is its first
/// enabled row, and the submenu that carries the next level sits below it.
fn descend_deep(cascade: &mut MenuCascade, focus: &mut FocusState) {
    open_context(cascade, &deep_menu(), focus);
    assert_eq!(cascade.focused_row(), Some(0));
    cascade.on_key(MenuKey::Down, focus);
    cascade.on_key(MenuKey::Right, focus);
    cascade.on_key(MenuKey::Down, focus);
    cascade.on_key(MenuKey::Right, focus);
    cascade.on_key(MenuKey::Down, focus);
    cascade.on_key(MenuKey::Right, focus);
    assert_eq!(cascade.depth(), 4);
}

/// Run a fixed script against a fresh cascade and return the whole state.
fn run_script(script: &[MenuKey]) -> (MenuCascade, FocusState) {
    let menu = single_file_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    cascade.open(&menu, MenuTreeKind::Context, &mut focus);
    for key in script {
        cascade.on_key(*key, &mut focus);
    }
    (cascade, focus)
}

// ---------------------------------------------------------------------------
// Opening a tree
// ---------------------------------------------------------------------------

#[test]
fn opening_reports_the_root_open_and_focuses_its_first_navigable_row() {
    let menu = single_file_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();

    let outcome = open_context(&mut cascade, &menu, &mut focus);

    assert!(cascade.is_open());
    assert_eq!(cascade.kind(), Some(MenuTreeKind::Context));
    assert_eq!(cascade.root_id(), Some(menu.id()));
    assert_eq!(cascade.depth(), 1);
    assert_eq!(outcome.opened().to_vec(), vec![menu.id().clone()]);
    assert!(outcome.closed().is_empty());
    assert_eq!(cascade.focused_row(), Some(1));
    assert_eq!(focused(&focus), Some("menu:ctx-file#edit".to_owned()));
    assert_eq!(focus.active_trap(), Some(&gid("ctx-file")));
}

#[test]
fn the_active_card_is_the_deepest_open_one() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    descend_deep(&mut cascade, &mut focus);

    let cards = cascade.open_cards();
    assert_eq!(cards.len(), 4);
    assert_eq!(
        cascade.active_card().map(|card| card.id()),
        Some(&mid("nav/l1/l2/l3"))
    );
    assert_eq!(
        cascade.active_menu().map(Menu::id),
        cards.last().map(|card| card.id())
    );
}

#[test]
fn open_cards_are_ordered_root_first_and_each_trigger_names_its_parent() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    descend_deep(&mut cascade, &mut focus);

    let cards = cascade.open_cards();
    assert_eq!(cards[0].id(), &mid("nav"));
    assert!(cards[0].is_root());
    assert_eq!(cards[0].trigger(), None);

    assert_eq!(
        cards[1].trigger(),
        Some(&SubmenuTrigger {
            parent: mid("nav"),
            row: 1
        })
    );
    assert_eq!(
        cards[2].trigger(),
        Some(&SubmenuTrigger {
            parent: mid("nav/l1"),
            row: 2
        })
    );
    assert_eq!(
        cards[3].trigger(),
        Some(&SubmenuTrigger {
            parent: mid("nav/l1/l2"),
            row: 1
        })
    );
    assert!(cards[1..].iter().all(|card| !card.is_root()));
}

#[test]
fn every_open_card_registers_only_its_navigable_rows() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    descend_deep(&mut cascade, &mut focus);

    for card in cascade.open_cards() {
        let group = focus
            .group(card.group_id())
            .unwrap_or_else(|| panic!("card `{}` has no registered group", card.id()));

        assert_eq!(
            group.members().len(),
            card.menu().navigable_indices().len(),
            "the roster of `{}` must count only rows the keyboard can be on",
            card.id()
        );
        assert!(
            group.members().iter().all(|member| member.enabled),
            "furniture is excluded rather than registered disabled in `{}`",
            card.id()
        );
    }
}

#[test]
fn a_card_with_nothing_focusable_does_not_open() {
    let furniture = furniture_only_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();

    let outcome = cascade.open(&furniture, MenuTreeKind::Context, &mut focus);

    assert_eq!(
        outcome.ignored_reason(),
        Some(&IgnoredReason::NoFocusableRows {
            card: furniture.id().clone()
        })
    );
    assert!(!cascade.is_open());
    assert!(!focus.is_trapped());
    assert_eq!(focused(&focus), None);
}

#[test]
fn a_refused_open_leaves_the_previous_tree_untouched() {
    let menu = single_file_menu();
    let furniture = furniture_only_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &menu, &mut focus);
    let before = cascade.clone();

    let outcome = cascade.open(&furniture, MenuTreeKind::Context, &mut focus);

    assert!(outcome.is_ignored());
    assert_eq!(cascade, before);
    assert_eq!(focused(&focus), Some("menu:ctx-file#edit".to_owned()));
}

#[test]
fn reopening_the_same_tree_is_refused_rather_than_restarted() {
    let menu = single_file_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &menu, &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);
    let before = cascade.clone();

    let outcome = cascade.open(&menu, MenuTreeKind::Context, &mut focus);

    assert_eq!(
        outcome.ignored_reason(),
        Some(&IgnoredReason::AlreadyOpen {
            card: menu.id().clone()
        })
    );
    assert_eq!(cascade, before);
    assert_eq!(cascade.depth(), 1);
    assert_eq!(focus.trap_depth(), 1);
}

#[test]
fn the_same_root_under_a_different_kind_is_a_different_tree() {
    let menu = single_file_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &menu, &mut focus);

    let outcome = cascade.open(&menu, MenuTreeKind::Fixed, &mut focus);

    assert!(!outcome.is_ignored());
    assert_eq!(cascade.kind(), Some(MenuTreeKind::Fixed));
    assert_eq!(cascade.depth(), 1);
}

// ---------------------------------------------------------------------------
// Mutual exclusion (UIF-CMENU-002)
// ---------------------------------------------------------------------------

#[test]
fn a_context_menu_opened_over_a_fixed_menu_closes_the_fixed_one() {
    let fixed = deep_menu();
    let context = single_file_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();

    cascade.open(&fixed, MenuTreeKind::Fixed, &mut focus);
    assert_eq!(cascade.kind(), Some(MenuTreeKind::Fixed));

    let outcome = cascade.open(&context, MenuTreeKind::Context, &mut focus);

    assert_eq!(cascade.kind(), Some(MenuTreeKind::Context));
    assert_eq!(cascade.root_id(), Some(context.id()));
    assert_eq!(cascade.depth(), 1);
    assert_eq!(outcome.closed().to_vec(), vec![fixed.id().clone()]);
    assert_eq!(outcome.opened().to_vec(), vec![context.id().clone()]);
    assert!(focus.group(&gid("nav")).is_none());
    assert!(focus.group(&gid("ctx-file")).is_some());
    assert_eq!(focus.trap_depth(), 1);
}

#[test]
fn a_fixed_menu_opened_over_a_context_menu_closes_the_context_one() {
    let fixed = deep_menu();
    let context = single_file_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();

    open_context(&mut cascade, &context, &mut focus);
    let outcome = cascade.open(&fixed, MenuTreeKind::Fixed, &mut focus);

    assert_eq!(cascade.kind(), Some(MenuTreeKind::Fixed));
    assert_eq!(cascade.root_id(), Some(fixed.id()));
    assert_eq!(outcome.closed().to_vec(), vec![context.id().clone()]);
    assert!(focus.group(&gid("ctx-file")).is_none());
    assert_eq!(focus.trap_depth(), 1);
    assert_eq!(focused(&focus), Some("menu:nav#plain".to_owned()));
}

#[test]
fn switching_trees_repeatedly_never_leaves_two_roots_live() {
    let fixed = deep_menu();
    let context = single_file_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();

    for _ in 0..5 {
        cascade.open(&fixed, MenuTreeKind::Fixed, &mut focus);
        cascade.open(&context, MenuTreeKind::Context, &mut focus);

        assert_eq!(cascade.depth(), 1);
        assert_eq!(focus.trap_depth(), 1);
        assert_eq!(
            focus.groups().len(),
            1,
            "only the live tree's group may remain registered"
        );
    }
}

#[test]
fn a_submenu_opened_under_a_switched_root_replaces_the_whole_previous_tree() {
    let fixed = deep_menu();
    let context = twins_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();

    cascade.open(&fixed, MenuTreeKind::Fixed, &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);
    cascade.on_key(MenuKey::Right, &mut focus);
    assert_eq!(cascade.depth(), 2);

    open_context(&mut cascade, &context, &mut focus);
    cascade.on_key(MenuKey::Right, &mut focus);

    assert_eq!(cascade.depth(), 2);
    assert_eq!(cascade.root_id(), Some(context.id()));
    assert!(focus.group(&gid("nav")).is_none());
    assert!(focus.group(&gid("nav/l1")).is_none());
}

// ---------------------------------------------------------------------------
// Persistent parents and sibling replacement (UIF-FMENU-002)
// ---------------------------------------------------------------------------

#[test]
fn opening_a_submenu_keeps_every_ancestor_open() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    descend_deep(&mut cascade, &mut focus);

    for path in ["nav", "nav/l1", "nav/l1/l2", "nav/l1/l2/l3"] {
        assert!(
            cascade.is_card_open(&mid(path)),
            "`{path}` must still be open at depth 4"
        );
    }
}

#[test]
fn no_ancestor_closes_while_a_descendant_is_being_opened() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &deep_menu(), &mut focus);

    for level in 1..=3 {
        cascade.on_key(MenuKey::Down, &mut focus);
        let before = cascade
            .open_cards()
            .iter()
            .map(|card| card.id().clone())
            .collect::<Vec<_>>();
        let outcome = cascade.open_focused_submenu(&mut focus);

        assert!(
            !outcome.is_ignored(),
            "hop {level} must open a branch: {outcome:?}"
        );
        assert!(
            outcome.closed().is_empty(),
            "opening deeper must close nothing: {outcome:?}"
        );
        let after = cascade
            .open_cards()
            .iter()
            .map(|card| card.id().clone())
            .collect::<Vec<_>>();
        assert_eq!(
            after[..before.len()],
            before[..],
            "hop {level} must leave every ancestor where it was"
        );
        assert_eq!(cascade.depth(), level + 1);
    }
}

#[test]
fn a_sibling_submenu_replaces_the_open_branch_inst_of_stacking() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &twins_menu(), &mut focus);

    cascade.open_submenu(&mid("twins"), 0, &mut focus);
    cascade.open_submenu(&mid("twins"), 0, &mut focus);
    assert_eq!(cascade.depth(), 2);
    cascade.open_submenu(&mid("twins/a"), 1, &mut focus);
    assert_eq!(cascade.depth(), 3);

    let outcome = cascade.open_submenu(&mid("twins"), 1, &mut focus);

    assert_eq!(cascade.depth(), 2);
    assert!(cascade.is_card_open(&mid("twins")));
    assert!(cascade.is_card_open(&mid("twins/b")));
    assert!(!cascade.is_card_open(&mid("twins/a")));
    assert!(!cascade.is_card_open(&mid("twins/a/a-deep")));
    assert_eq!(
        outcome.closed().to_vec(),
        vec![mid("twins/a/a-deep"), mid("twins/a")]
    );
    assert_eq!(outcome.opened().to_vec(), vec![mid("twins/b")]);
}

#[test]
fn cycling_sibling_branches_never_grows_the_depth() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &twins_menu(), &mut focus);

    for _ in 0..10 {
        cascade.open_submenu(&mid("twins"), 0, &mut focus);
        assert_eq!(cascade.depth(), 2);
        cascade.open_submenu(&mid("twins"), 1, &mut focus);
        assert_eq!(cascade.depth(), 2);
    }
    assert_eq!(focus.trap_depth(), 2);
}

#[test]
fn reopening_the_branch_that_is_already_open_changes_nothing() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &twins_menu(), &mut focus);
    cascade.open_submenu(&mid("twins"), 0, &mut focus);
    let before = cascade.clone();

    let outcome = cascade.open_submenu(&mid("twins"), 0, &mut focus);

    assert_eq!(
        outcome.ignored_reason(),
        Some(&IgnoredReason::AlreadyOpen {
            card: mid("twins/a")
        })
    );
    assert_eq!(cascade, before);
}

#[test]
fn sibling_replacement_at_a_deep_level_prunes_only_below_the_anchor() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &fork_menu(), &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);
    cascade.on_key(MenuKey::Right, &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);
    cascade.on_key(MenuKey::Right, &mut focus);
    assert_eq!(cascade.depth(), 3);

    // Switch the branch hanging off the *middle* card: X-two out, Y in.
    let outcome = cascade.open_submenu(&mid("fork/x"), 2, &mut focus);

    assert_eq!(cascade.depth(), 3);
    assert!(cascade.is_card_open(&mid("fork")));
    assert!(cascade.is_card_open(&mid("fork/x")));
    assert!(cascade.is_card_open(&mid("fork/x/y")));
    assert!(!cascade.is_card_open(&mid("fork/x/x2")));
    assert_eq!(outcome.closed().to_vec(), vec![mid("fork/x/x2")]);
    assert_eq!(outcome.opened().to_vec(), vec![mid("fork/x/y")]);
    assert_eq!(
        focused(&focus),
        Some("menu:fork/x/y#y1".to_owned()),
        "focus lands in the new branch, not the one it replaced"
    );
}

// ---------------------------------------------------------------------------
// The keyboard walk (UIF-CMENU-003)
// ---------------------------------------------------------------------------

#[test]
fn down_cycles_exactly_the_navigable_rows_of_the_single_file_fixture() {
    let menu = single_file_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &menu, &mut focus);

    let order = [
        "edit",
        "open-editor",
        "print",
        "run-prompt",
        "format-markdown",
    ];
    for key in order {
        assert_eq!(focused_key(&cascade, "ctx-file").as_deref(), Some(key));
        cascade.on_key(MenuKey::Down, &mut focus);
    }
    assert_eq!(
        focused_key(&cascade, "ctx-file").as_deref(),
        Some("note-skills")
    );
}

#[test]
fn a_full_down_cycle_visits_every_navigable_row_once_and_wraps() {
    let menu = single_file_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &menu, &mut focus);

    let navigable = menu.navigable_indices();
    let mut visited = Vec::new();
    for _ in 0..navigable.len() {
        visited.push(cascade.focused_row().expect("a row must be focused"));
        cascade.on_key(MenuKey::Down, &mut focus);
    }

    assert_eq!(visited, navigable);
    assert_eq!(
        cascade.focused_row(),
        Some(1),
        "`Down` past the last row wraps to the first"
    );
}

#[test]
fn up_walks_the_same_roster_backwards_and_wraps() {
    let menu = single_file_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &menu, &mut focus);

    let mut visited = Vec::new();
    for _ in 0..menu.navigable_indices().len() {
        visited.push(cascade.focused_row().expect("a row must be focused"));
        cascade.on_key(MenuKey::Up, &mut focus);
    }

    let mut expected = menu.navigable_indices();
    expected.reverse();
    expected.rotate_right(1);
    assert_eq!(visited, expected);
}

#[test]
fn the_walk_skips_every_heading_separator_and_disabled_row() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &skip_menu(), &mut focus);

    let mut visited = Vec::new();
    for _ in 0..6 {
        visited.push(cascade.focused_row().expect("a row must be focused"));
        cascade.on_key(MenuKey::Down, &mut focus);
    }

    assert_eq!(visited, vec![1, 4, 7, 1, 4, 7]);
    for row in [0usize, 2, 3, 5, 6, 8] {
        assert!(
            !visited.contains(&row),
            "row {row} is furniture or disabled and must never be focused"
        );
    }
}

#[test]
fn no_focus_id_ever_names_a_non_navigable_row() {
    let menu = single_file_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &menu, &mut focus);

    let allowed = menu.navigable_focus_ids();
    for _ in 0..menu.len() * 2 {
        let current = focus.current().expect("focus stays inside the card");
        assert!(
            allowed.contains(current),
            "`{current}` is not a navigable row of `{}`",
            menu.id()
        );
        cascade.on_key(MenuKey::Down, &mut focus);
    }
}

#[test]
fn delta_zero_does_not_move_focus() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &skip_menu(), &mut focus);
    let before = focused(&focus);

    let outcome = cascade.move_focus(0, &mut focus);

    assert_eq!(focused(&focus), before);
    assert_eq!(outcome.focus(), &FocusTransition::Unchanged);
}

#[test]
fn right_walks_down_three_levels_focusing_each_first_enabled_child() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &deep_menu(), &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);

    cascade.on_key(MenuKey::Right, &mut focus);
    assert_eq!(cascade.depth(), 2);
    assert_eq!(focused(&focus), Some("menu:nav/l1#l1-action".to_owned()));

    cascade.on_key(MenuKey::Down, &mut focus);
    cascade.on_key(MenuKey::Right, &mut focus);
    assert_eq!(cascade.depth(), 3);
    assert_eq!(focused(&focus), Some("menu:nav/l1/l2#l2-action".to_owned()));

    cascade.on_key(MenuKey::Down, &mut focus);
    cascade.on_key(MenuKey::Right, &mut focus);
    assert_eq!(cascade.depth(), 4);
    assert_eq!(
        focused(&focus),
        Some("menu:nav/l1/l2/l3#l3-action".to_owned())
    );
}

#[test]
fn right_on_a_non_submenu_row_does_nothing() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &single_file_menu(), &mut focus);
    let before = cascade.clone();
    let before_focus = focused(&focus);

    let outcome = cascade.on_key(MenuKey::Right, &mut focus);

    assert_eq!(
        outcome.ignored_reason(),
        Some(&IgnoredReason::NotSubmenu {
            card: mid("ctx-file"),
            row: 1
        })
    );
    assert_eq!(cascade, before);
    assert_eq!(focused(&focus), before_focus);
    assert_eq!(cascade.depth(), 1);
}

#[test]
fn right_on_a_submenu_with_nothing_focusable_inside_is_refused() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &dead_submenu_menu(), &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);
    assert_eq!(cascade.focused_row(), Some(1));
    let before = cascade.clone();

    let outcome = cascade.on_key(MenuKey::Right, &mut focus);

    assert_eq!(
        outcome.ignored_reason(),
        Some(&IgnoredReason::NoFocusableRows {
            card: mid("dead/dead-branch")
        })
    );
    assert_eq!(cascade, before);
    assert_eq!(cascade.depth(), 1);
    assert_eq!(focused(&focus), Some("menu:dead#dead-branch".to_owned()));
    assert_eq!(focus.active_trap(), Some(&gid("dead")));
}

#[test]
fn right_without_a_focused_row_is_refused() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    focus.register_group(gid("modal"), vec![FocusMember::new(fid("modal", "ok"))]);
    focus.focus(&fid("modal", "ok"));
    open_context(&mut cascade, &single_file_menu(), &mut focus);

    // A trap above the menu's own means the keyboard is not inside the
    // card, so the cascade has no row to act on.
    focus.push_trap(gid("modal"));
    focus.focus(&fid("modal", "ok"));
    cascade.sync(&single_file_menu(), &mut focus);
    assert_eq!(cascade.focused_row(), None);

    assert_eq!(
        cascade.open_focused_submenu(&mut focus).ignored_reason(),
        Some(&IgnoredReason::NothingFocused {
            card: mid("ctx-file")
        })
    );
    assert_eq!(
        cascade.activate_focused(&mut focus).ignored_reason(),
        Some(&IgnoredReason::NothingFocused {
            card: mid("ctx-file")
        })
    );
}

#[test]
fn left_unwinds_one_level_at_a_time_to_the_exact_parent_row() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    descend_deep(&mut cascade, &mut focus);

    cascade.on_key(MenuKey::Left, &mut focus);
    assert_eq!(cascade.depth(), 3);
    assert_eq!(focused(&focus), Some("menu:nav/l1/l2#l3".to_owned()));

    cascade.on_key(MenuKey::Left, &mut focus);
    assert_eq!(cascade.depth(), 2);
    assert_eq!(focused(&focus), Some("menu:nav/l1#l2".to_owned()));

    cascade.on_key(MenuKey::Left, &mut focus);
    assert_eq!(cascade.depth(), 1);
    assert_eq!(focused(&focus), Some("menu:nav#l1".to_owned()));
}

#[test]
fn escape_at_the_root_closes_the_whole_tree() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    descend_deep(&mut cascade, &mut focus);

    cascade.on_key(MenuKey::Left, &mut focus);
    cascade.on_key(MenuKey::Left, &mut focus);
    cascade.on_key(MenuKey::Left, &mut focus);
    assert_eq!(cascade.depth(), 1);

    cascade.on_key(MenuKey::Escape, &mut focus);

    assert!(!cascade.is_open());
    assert_eq!(cascade.kind(), None);
    assert!(focus.groups().is_empty());
    assert_eq!(focus.trap_depth(), 0);
    assert_eq!(focused(&focus), None);
}

#[test]
fn left_and_escape_agree_at_every_level() {
    let script = [
        MenuKey::Down,
        MenuKey::Right,
        MenuKey::Right,
        MenuKey::Left,
        MenuKey::Escape,
        MenuKey::Escape,
        MenuKey::Escape,
    ];
    let (left, _) = run_script(&script);

    let menu = single_file_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    cascade.open(&menu, MenuTreeKind::Context, &mut focus);
    for key in script {
        let key = match key {
            MenuKey::Left => MenuKey::Escape,
            other => other,
        };
        cascade.on_key(key, &mut focus);
    }

    assert_eq!(cascade.depth(), left.depth());
    assert_eq!(cascade.is_open(), left.is_open());
    assert_eq!(cascade.focused_row(), left.focused_row());
}

#[test]
fn a_closed_cascade_refuses_every_key() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();

    for key in [
        MenuKey::Up,
        MenuKey::Down,
        MenuKey::Left,
        MenuKey::Right,
        MenuKey::Enter,
        MenuKey::Space,
        MenuKey::Escape,
    ] {
        let outcome = cascade.on_key(key, &mut focus);
        assert_eq!(
            outcome.ignored_reason(),
            Some(&IgnoredReason::NotOpen),
            "`{key:?}` against a closed cascade must be refused"
        );
    }
    assert!(!focus.is_trapped());
}

// ---------------------------------------------------------------------------
// Activation and close policy (UIF-FMENU-003)
// ---------------------------------------------------------------------------

#[test]
fn enter_on_a_stay_open_checkable_flips_it_and_keeps_the_tree_open() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &policy_menu(), &mut focus);
    assert_eq!(cascade.focused_row(), Some(0));

    let outcome = cascade.on_key(MenuKey::Enter, &mut focus);
    let activation = outcome.activation().expect("a row was activated");

    assert_eq!(
        activation.effect,
        ActivationEffect::Toggled { checked: true }
    );
    assert!(!activation.closes_menu);
    assert!(cascade.is_open());
    assert_eq!(cascade.depth(), 1);
    assert!(cascade.active_menu().unwrap().row(0).unwrap().is_checked());
    assert_eq!(cascade.focused_row(), Some(0));
}

#[test]
fn space_toggles_the_same_row_in_place_and_repeated_presses_alternate() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &policy_menu(), &mut focus);

    cascade.on_key(MenuKey::Space, &mut focus);
    assert!(cascade.active_menu().unwrap().row(0).unwrap().is_checked());
    cascade.on_key(MenuKey::Space, &mut focus);
    assert!(!cascade.active_menu().unwrap().row(0).unwrap().is_checked());
    assert!(cascade.is_open());
    assert_eq!(cascade.depth(), 1);
}

#[test]
fn enter_on_an_action_closes_the_whole_tree() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &policy_menu(), &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);
    assert_eq!(cascade.focused_row(), Some(1));

    let outcome = cascade.on_key(MenuKey::Enter, &mut focus);
    let activation = outcome.activation().expect("a row was activated");

    assert_eq!(activation.effect, ActivationEffect::Command);
    assert!(activation.closes_menu);
    assert!(!cascade.is_open());
    assert_eq!(outcome.closed().to_vec(), vec![mid("policy")]);
    assert_eq!(focus.trap_depth(), 0);
}

#[test]
fn a_close_on_toggle_checkable_takes_the_tree_with_it() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &policy_menu(), &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);
    assert_eq!(cascade.focused_row(), Some(2));

    let outcome = cascade.on_key(MenuKey::Enter, &mut focus);
    let activation = outcome.activation().expect("a row was activated");

    assert_eq!(
        activation.effect,
        ActivationEffect::Toggled { checked: false }
    );
    assert!(activation.closes_menu);
    assert!(!cascade.is_open());
}

#[test]
fn enter_on_a_radio_selects_it_clears_its_sibling_and_closes() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &policy_menu(), &mut focus);
    for _ in 0..4 {
        cascade.on_key(MenuKey::Down, &mut focus);
    }
    assert_eq!(cascade.focused_row(), Some(4));

    let outcome = cascade.on_key(MenuKey::Enter, &mut focus);
    let activation = outcome.activation().expect("a row was activated");

    match &activation.effect {
        ActivationEffect::Selected { group, cleared } => {
            assert_eq!(group.as_str(), "mode");
            assert_eq!(cleared, &vec![3usize]);
        }
        other => panic!("expected a radio selection, got {other:?}"),
    }
    assert!(!cascade.is_open());
}

#[test]
fn enter_on_a_submenu_row_opens_the_branch_instead_of_closing() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &twins_menu(), &mut focus);

    let outcome = cascade.on_key(MenuKey::Enter, &mut focus);

    assert_eq!(
        outcome.activation().map(|a| a.effect.clone()),
        Some(ActivationEffect::OpenSubmenu {
            child: mid("twins/a")
        })
    );
    assert_eq!(cascade.depth(), 2);
    assert!(cascade.is_open());
    assert_eq!(focused(&focus), Some("menu:twins/a#a1".to_owned()));
}

#[test]
fn a_heading_click_is_absorbed_and_dismisses_nothing() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &single_file_menu(), &mut focus);
    let before_focus = focused(&focus);

    let outcome = cascade.activate_row(&mid("ctx-file"), 0, &mut focus);
    let activation = outcome.activation().expect("the click reached a row");

    assert!(activation.is_absorbed());
    assert!(!activation.closes_menu);
    assert!(cascade.is_open());
    assert_eq!(focused(&focus), before_focus);
}

#[test]
fn a_disabled_row_click_is_absorbed_and_dismisses_nothing() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &skip_menu(), &mut focus);
    let before_focus = focused(&focus);

    let outcome = cascade.activate_row(&mid("skip"), 3, &mut focus);

    assert!(outcome.activation().expect("row taken").is_absorbed());
    assert!(cascade.is_open());
    assert_eq!(
        focused(&focus),
        before_focus,
        "a disabled row cannot take the keyboard"
    );
}

#[test]
fn a_separator_click_is_absorbed_and_dismisses_nothing() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &skip_menu(), &mut focus);

    let outcome = cascade.activate_row(&mid("skip"), 2, &mut focus);

    assert!(outcome.activation().expect("row taken").is_absorbed());
    assert!(cascade.is_open());
}

#[test]
fn activating_a_stay_open_row_in_an_ancestor_card_closes_the_branch_below_it() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &twins_menu(), &mut focus);
    cascade.open_submenu(&mid("twins"), 0, &mut focus);
    cascade.open_submenu(&mid("twins/a"), 1, &mut focus);
    assert_eq!(cascade.depth(), 3);

    let outcome = cascade.activate_row(&mid("twins"), 2, &mut focus);

    assert_eq!(
        outcome.activation().map(|a| a.effect.clone()),
        Some(ActivationEffect::Toggled { checked: true })
    );
    assert_eq!(cascade.depth(), 1);
    assert!(cascade.is_open());
    assert_eq!(
        outcome.closed().to_vec(),
        vec![mid("twins/a/a-deep"), mid("twins/a")]
    );
    assert_eq!(focused(&focus), Some("menu:twins#keep".to_owned()));
}

#[test]
fn activating_the_trigger_of_the_open_branch_leaves_the_branch_alone() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &twins_menu(), &mut focus);
    cascade.open_submenu(&mid("twins"), 0, &mut focus);
    let before = cascade.clone();

    let outcome = cascade.activate_row(&mid("twins"), 0, &mut focus);

    assert_eq!(cascade, before);
    assert!(outcome.closed().is_empty());
    assert!(outcome.opened().is_empty());
    assert_eq!(cascade.depth(), 2);
}

#[test]
fn activating_an_action_in_a_deep_card_closes_the_whole_tree() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    descend_deep(&mut cascade, &mut focus);

    let outcome = cascade.activate_row(&mid("nav/l1/l2/l3"), 0, &mut focus);

    assert!(outcome.activation().expect("row fired").closes_menu);
    assert!(!cascade.is_open());
    assert_eq!(
        outcome.closed().to_vec(),
        vec![
            mid("nav/l1/l2/l3"),
            mid("nav/l1/l2"),
            mid("nav/l1"),
            mid("nav")
        ]
    );
}

// ---------------------------------------------------------------------------
// Click-away (UIF-CMENU-002)
// ---------------------------------------------------------------------------

#[test]
fn click_away_closes_the_entire_hierarchy_immediately() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    descend_deep(&mut cascade, &mut focus);

    let outcome = cascade.close_tree(&mut focus);

    assert!(!cascade.is_open());
    assert_eq!(cascade.kind(), None);
    assert_eq!(
        outcome.closed().to_vec(),
        vec![
            mid("nav/l1/l2/l3"),
            mid("nav/l1/l2"),
            mid("nav/l1"),
            mid("nav")
        ]
    );
    assert!(focus.groups().is_empty());
    assert_eq!(focus.trap_depth(), 0);
}

#[test]
fn click_away_when_nothing_is_open_is_a_no_op() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    let outside = FocusId::new("bg#x");
    focus.register_group(GroupId::new("bg"), vec![FocusMember::new(outside.clone())]);
    focus.focus(&outside);

    let outcome = cascade.close_tree(&mut focus);

    assert!(outcome.closed().is_empty());
    assert_eq!(outcome.focus(), &FocusTransition::Unchanged);
    assert_eq!(focused(&focus), Some("bg#x".to_owned()));
}

#[test]
fn click_away_restores_the_focus_the_menu_took() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    let trigger = FocusId::new("navbar#burger");
    focus.register_group(
        GroupId::new("navbar"),
        vec![FocusMember::new(trigger.clone())],
    );
    focus.focus(&trigger);

    descend_deep(&mut cascade, &mut focus);
    assert_eq!(
        focused(&focus),
        Some("menu:nav/l1/l2/l3#l3-action".to_owned())
    );

    cascade.close_tree(&mut focus);

    assert_eq!(focused(&focus), Some("navbar#burger".to_owned()));
}

#[test]
fn click_away_with_no_prior_focus_clears_focus_rather_than_inventing_one() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &single_file_menu(), &mut focus);

    cascade.close_tree(&mut focus);

    assert_eq!(focused(&focus), None);
    assert_eq!(focus.trap_depth(), 0);
}

#[test]
fn closing_the_root_card_is_the_same_as_closing_the_tree() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &single_file_menu(), &mut focus);

    let outcome = cascade.close_deepest(&mut focus);

    assert!(!cascade.is_open());
    assert_eq!(outcome.closed().to_vec(), vec![mid("ctx-file")]);
}

#[test]
fn closing_the_deepest_card_against_a_closed_cascade_is_refused() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();

    assert_eq!(
        cascade.close_deepest(&mut focus).ignored_reason(),
        Some(&IgnoredReason::NotOpen)
    );
}

// ---------------------------------------------------------------------------
// Focus return and the trap
// ---------------------------------------------------------------------------

#[test]
fn the_parent_card_remembers_the_row_its_branch_hung_from() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    descend_deep(&mut cascade, &mut focus);

    assert_eq!(cascade.focused_row_in(&mid("nav")), Some(1));
    assert_eq!(cascade.focused_row_in(&mid("nav/l1")), Some(2));
    assert_eq!(cascade.focused_row_in(&mid("nav/l1/l2")), Some(1));
    assert_eq!(cascade.focused_row_in(&mid("nav/l1/l2/l3")), Some(0));
}

#[test]
fn tab_cannot_reach_a_background_group_while_a_card_is_active() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    focus.register_group(
        gid("background"),
        vec![
            FocusMember::new(fid("background", "tree")),
            FocusMember::new(fid("background", "editor")),
        ],
    );
    focus.focus(&fid("background", "tree"));

    open_context(&mut cascade, &single_file_menu(), &mut focus);

    for _ in 0..20 {
        focus.focus_next();
        let current = focused(&focus).expect("focus stays put");
        assert!(
            current.starts_with("menu:ctx-file#"),
            "`Tab` escaped the open menu card to `{current}`"
        );
    }
    for _ in 0..20 {
        focus.focus_previous();
        let current = focused(&focus).expect("focus stays put");
        assert!(
            current.starts_with("menu:ctx-file#"),
            "`Shift+Tab` escaped the open menu card to `{current}`"
        );
    }
}

#[test]
fn tab_cycles_the_active_card_and_never_leaves_it_for_a_deeper_one() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &deep_menu(), &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);
    cascade.on_key(MenuKey::Right, &mut focus);

    for _ in 0..10 {
        focus.focus_next();
        assert!(
            focused(&focus)
                .expect("focus stays put")
                .starts_with("menu:nav/l1#"),
            "the trap must follow the active card, not stay on the root"
        );
    }
    assert_eq!(focus.active_trap(), Some(&gid("nav/l1")));
}

#[test]
fn each_open_card_owns_exactly_one_trap() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &deep_menu(), &mut focus);

    assert_eq!(focus.trap_depth(), cascade.depth());
    // Descending adds exactly one trap per card opened...
    for _ in 0..3 {
        cascade.on_key(MenuKey::Down, &mut focus);
        cascade.on_key(MenuKey::Right, &mut focus);
        assert_eq!(focus.trap_depth(), cascade.depth());
    }
    assert_eq!(cascade.depth(), 4);
    // ...and unwinding from the deepest end pops exactly one each time.
    while cascade.is_open() {
        cascade.close_deepest(&mut focus);
        assert_eq!(focus.trap_depth(), cascade.depth());
    }
    assert_eq!(focus.trap_depth(), 0);
}

#[test]
fn closing_a_card_releases_its_trap_and_unregisters_its_group() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &deep_menu(), &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);
    cascade.on_key(MenuKey::Right, &mut focus);
    assert!(focus.group(&gid("nav/l1")).is_some());

    cascade.close_deepest(&mut focus);

    assert!(focus.group(&gid("nav/l1")).is_none());
    assert_eq!(focus.active_trap(), Some(&gid("nav")));
    assert_eq!(focused(&focus), Some("menu:nav#l1".to_owned()));
}

#[test]
fn the_background_is_reachable_again_once_the_tree_is_gone() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    let outside = FocusId::new("background#tree");
    focus.register_group(
        GroupId::new("background"),
        vec![FocusMember::new(outside.clone())],
    );
    focus.focus(&outside);
    open_context(&mut cascade, &single_file_menu(), &mut focus);
    assert_eq!(focused(&focus), Some("menu:ctx-file#edit".to_owned()));

    cascade.close_tree(&mut focus);
    focus.focus_next();

    assert_eq!(focused(&focus), Some("background#tree".to_owned()));
}

// ---------------------------------------------------------------------------
// Model adoption (`sync`)
// ---------------------------------------------------------------------------

#[test]
fn a_row_disabled_while_focused_moves_focus_forward_in_place() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &skip_menu(), &mut focus);
    assert_eq!(cascade.focused_row(), Some(1));

    let outcome = cascade.sync(&skip_menu_with_a_disabled(), &mut focus);

    assert!(outcome.closed().is_empty());
    assert!(cascade.is_open());
    assert_eq!(cascade.depth(), 1);
    assert_eq!(
        cascade.focused_row(),
        Some(4),
        "focus keeps the slot the disabled row occupied"
    );
    assert_eq!(focused(&focus), Some("menu:skip#c".to_owned()));
    assert_eq!(focus.active_trap(), Some(&gid("skip")));
}

#[test]
fn a_card_that_loses_every_focusable_row_closes_back_to_its_trigger() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &deep_menu(), &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);
    cascade.on_key(MenuKey::Right, &mut focus);
    assert_eq!(cascade.depth(), 2);

    let outcome = cascade.sync(&deep_menu_with_l1_inert(), &mut focus);

    assert_eq!(cascade.depth(), 1);
    assert!(!cascade.is_card_open(&mid("nav/l1")));
    assert_eq!(
        outcome.closed().to_vec(),
        vec![mid("nav/l1")],
        "the inert card is reported closed"
    );
    assert_eq!(
        focused(&focus),
        Some("menu:nav#l1".to_owned()),
        "focus returns to the trigger row"
    );
}

#[test]
fn a_card_the_source_no_longer_has_is_pruned() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &deep_menu(), &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);
    cascade.on_key(MenuKey::Right, &mut focus);
    assert_eq!(cascade.depth(), 2);

    let outcome = cascade.sync(&root_without_l1_submenu(), &mut focus);

    assert_eq!(cascade.depth(), 1);
    assert_eq!(outcome.closed().to_vec(), vec![mid("nav/l1")]);
    assert!(cascade.is_open());
}

#[test]
fn sync_closes_the_tree_when_the_root_identity_changes() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &deep_menu(), &mut focus);

    let other = Menu::new("other-root", vec![Row::action("x", "X")]);
    let outcome = cascade.sync(&other, &mut focus);

    assert!(!cascade.is_open());
    assert_eq!(outcome.closed().to_vec(), vec![mid("nav")]);
    assert_eq!(focus.trap_depth(), 0);
}

#[test]
fn sync_against_an_equivalent_rebuild_moves_nothing() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &deep_menu(), &mut focus);
    cascade.on_key(MenuKey::Down, &mut focus);
    cascade.on_key(MenuKey::Right, &mut focus);
    let before = cascade.clone();
    let before_focus = focused(&focus);

    let outcome = cascade.sync(&deep_menu(), &mut focus);

    assert_eq!(cascade, before);
    assert_eq!(focused(&focus), before_focus);
    assert_eq!(outcome.focus(), &FocusTransition::Unchanged);
    assert_eq!(focus.trap_depth(), 2);
}

#[test]
fn sync_against_a_closed_cascade_is_a_no_op() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();

    let outcome = cascade.sync(&single_file_menu(), &mut focus);

    assert!(outcome.closed().is_empty());
    assert!(outcome.opened().is_empty());
    assert_eq!(outcome.focus(), &FocusTransition::Unchanged);
}

#[test]
fn sync_replaces_the_rendered_model_with_the_callers_current_one() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &skip_menu(), &mut focus);

    cascade.sync(&skip_menu_with_a_disabled(), &mut focus);

    let card = cascade.active_menu().expect("a card is open");
    assert!(
        !card.row(1).expect("row present").is_enabled(),
        "the cascade renders the caller's current model, not the one it opened with"
    );
}

// ---------------------------------------------------------------------------
// Refusals against unknown targets
// ---------------------------------------------------------------------------

#[test]
fn a_card_that_is_not_open_cannot_be_addressed() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &twins_menu(), &mut focus);

    assert_eq!(
        cascade
            .open_submenu(&mid("twins/a"), 0, &mut focus)
            .ignored_reason(),
        Some(&IgnoredReason::UnknownCard {
            card: mid("twins/a")
        })
    );
    assert_eq!(
        cascade
            .activate_row(&mid("twins/a"), 0, &mut focus)
            .ignored_reason(),
        Some(&IgnoredReason::UnknownCard {
            card: mid("twins/a")
        })
    );
    assert_eq!(cascade.depth(), 1);
}

#[test]
fn addressing_a_card_that_is_not_open_against_a_closed_cascade_is_unknown() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();

    assert_eq!(
        cascade
            .activate_row(&mid("ctx-file"), 0, &mut focus)
            .ignored_reason(),
        Some(&IgnoredReason::UnknownCard {
            card: mid("ctx-file")
        })
    );
}

#[test]
fn a_row_past_the_end_of_a_card_is_refused() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &skip_menu(), &mut focus);

    let outcome = cascade.activate_row(&mid("skip"), 99, &mut focus);

    assert_eq!(
        outcome.ignored_reason(),
        Some(&IgnoredReason::RowOutOfRange {
            card: mid("skip"),
            row: 99
        })
    );
    assert!(outcome.activation().is_none());
    assert!(cascade.is_open());
}

// ---------------------------------------------------------------------------
// The trigger latch (UIF-NAV-001)
// ---------------------------------------------------------------------------

#[test]
fn toggle_opens_a_tree_that_is_not_showing() {
    let menu = deep_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();

    let outcome = cascade.toggle(&menu, MenuTreeKind::Fixed, &mut focus);

    assert!(!outcome.is_ignored());
    assert_eq!(cascade.kind(), Some(MenuTreeKind::Fixed));
    assert!(cascade.is_open());
}

#[test]
fn toggle_closes_the_tree_it_just_opened() {
    let menu = deep_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    cascade.toggle(&menu, MenuTreeKind::Fixed, &mut focus);

    let outcome = cascade.toggle(&menu, MenuTreeKind::Fixed, &mut focus);

    assert!(!cascade.is_open());
    assert_eq!(outcome.closed().to_vec(), vec![menu.id().clone()]);
}

#[test]
fn toggle_from_a_context_menu_to_a_fixed_menu_replaces_it() {
    let context = single_file_menu();
    let fixed = deep_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &context, &mut focus);

    cascade.toggle(&fixed, MenuTreeKind::Fixed, &mut focus);

    assert_eq!(cascade.kind(), Some(MenuTreeKind::Fixed));
    assert_eq!(cascade.root_id(), Some(fixed.id()));
}

// ---------------------------------------------------------------------------
// The focus task
// ---------------------------------------------------------------------------

#[test]
fn a_moved_focus_projects_a_task_and_a_stay_put_projects_none() {
    let menu = single_file_menu();
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();

    let opened = open_context(&mut cascade, &menu, &mut focus);
    assert!(opened.focus_moved());
    assert!(
        opened.focus_task::<()>(&focus).units() > 0,
        "a focus move must hand the router something to do"
    );

    let refused = cascade.open(&menu, MenuTreeKind::Context, &mut focus);
    assert!(!refused.focus_moved());
    assert_eq!(
        refused.focus_task::<()>(&focus).units(),
        0,
        "nothing moved, so the router must not be woken"
    );
}

#[test]
fn clearing_focus_still_projects_a_task() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &single_file_menu(), &mut focus);

    let outcome = cascade.close_tree(&mut focus);

    assert_eq!(
        outcome.focus(),
        &FocusTransition::Cleared {
            from: fid("ctx-file", "edit")
        }
    );
    assert!(outcome.focus_task::<()>(&focus).units() > 0);
}

#[test]
fn a_focus_move_reports_where_it_came_from_and_where_it_went() {
    let mut cascade = MenuCascade::new();
    let mut focus = FocusState::new();
    open_context(&mut cascade, &skip_menu(), &mut focus);

    let outcome = cascade.move_focus(1, &mut focus);

    assert_eq!(
        outcome.focus(),
        &FocusTransition::Moved {
            from: Some(fid("skip", "a")),
            to: fid("skip", "c"),
        }
    );
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

#[test]
fn the_same_input_sequence_yields_the_same_state() {
    let script = [
        MenuKey::Down,
        MenuKey::Down,
        MenuKey::Right,
        MenuKey::Down,
        MenuKey::Right,
        MenuKey::Left,
        MenuKey::Up,
        MenuKey::Enter,
        MenuKey::Down,
        MenuKey::Right,
    ];

    let first = run_script(&script);
    let second = run_script(&script);

    assert_eq!(first.0, second.0);
    assert_eq!(first.1, second.1);
}

#[test]
fn two_cascades_driven_identically_converge_at_every_step() {
    let script = [
        MenuKey::Down,
        MenuKey::Right,
        MenuKey::Down,
        MenuKey::Right,
        MenuKey::Left,
        MenuKey::Escape,
        MenuKey::Up,
        MenuKey::Enter,
    ];
    let menu = single_file_menu();
    let mut a = MenuCascade::new();
    let mut b = MenuCascade::new();
    let mut fa = FocusState::new();
    let mut fb = FocusState::new();
    a.open(&menu, MenuTreeKind::Context, &mut fa);
    b.open(&menu, MenuTreeKind::Context, &mut fb);

    for key in script {
        a.on_key(key, &mut fa);
        b.on_key(key, &mut fb);
        assert_eq!(a, b, "diverged after `{key:?}`");
        assert_eq!(fa, fb, "focus diverged after `{key:?}`");
    }
}

#[test]
fn the_cascade_starts_empty_and_default_matches_new() {
    assert_eq!(MenuCascade::default(), MenuCascade::new());
    assert!(!MenuCascade::new().is_open());
    assert_eq!(MenuCascade::new().depth(), 0);
    assert!(MenuCascade::new().open_cards().is_empty());
    assert_eq!(MenuCascade::new().active_card(), None);
    assert_eq!(MenuCascade::new().focused_row(), None);
}

#[test]
fn the_close_policy_of_a_stay_open_row_is_what_keeps_the_tree_alive() {
    let menu = policy_menu();
    assert_eq!(
        menu.row(0).unwrap().close_policy(),
        ClosePolicy::StayOpen,
        "the fixture's premise: row 0 stays open"
    );
    assert_eq!(menu.row(1).unwrap().close_policy(), ClosePolicy::Close);
    assert_eq!(menu.row(2).unwrap().close_policy(), ClosePolicy::Close);
}
