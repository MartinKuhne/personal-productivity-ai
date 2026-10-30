# Reusable Menu Widget — Proposal

> **Status:** Draft proposal for review; not implemented.
> **Date:** 2026-10-19
> **Scope:** A shared menu model and widget for the hamburger menu, directory-tree context menu, and tab context menu in `src/app/ui_iced/`.
> **Requirements touched:** [UI-006, UI-023, UI-025, UI-026, UI-061..UI-066](../../src/app/ui/SPEC.md); [REQ-001](../../src/app/SPEC.md).
> **Design inspiration:** [Material Design 3 — Menus](https://m3.material.io/components/menus/specs).

This is an aspirational design proposal, not a change to either authoritative `SPEC.md` file. Requirements below use provisional `MENU-*` identifiers for discussion.

## 1. Summary

Refactor the three menu experiences onto one reusable, message-driven menu component. Callers provide a menu tree and a root-space position or anchor; the shared component renders consistent rows, resolves placement and overflow, manages nested-menu presentation, and emits caller-provided messages. Application commands and toggle state remain owned by the existing view models and reducer.

The component should support:

- An explicit point in the application root's logical coordinate space, or an anchor rectangle in that same space.
- Left/right and above/below placement, with configurable preference and automatic flipping when space is insufficient.
- Nested submenus whose parent surfaces remain visible while the child is open.
- Actions, checkable toggles, radio-style choices, disabled rows, section labels, and dividers.
- Consistent dark-theme styling, hover/focus/pressed/disabled/selected states, and keyboard navigation.

## 2. Material 3 inspiration and adaptation

The Material 3 menu guidance describes menus as temporary surfaces and presents expressive vertical menus with grouped layouts, richer selection states, and submenu motion. It identifies row-content slots such as a leading icon, primary and supporting text, trailing icon or text, badge, label, and divider. The guidance also illustrates enabled, disabled, hovered, focused, pressed, and active states, including a parent item that reveals a submenu.

Those patterns are useful here: a shared row layout should support optional leading/trailing content; groups should be visually distinguishable; focused, selected, and active-parent states should be apparent; and opening a submenu should preserve the visible parent context.

The Material 3 baseline measurements are reference material, not FastMD requirements. **[INFERENCE]** Its 48 dp list item and 112–280 dp container range are not automatically appropriate for FastMD's compact desktop layout. FastMD should retain its compact character and use its own palette and sizing tokens. As an initial design target, use a 32–36 logical-pixel minimum row height, 8–12 pixels of horizontal inset, and a content-driven width constrained to a practical maximum. Validate those values with screenshots and actual long menu labels before finalizing them. Keep the existing 4 px widget corner-radius contract unless a separate design decision changes it.

Do not copy Material color tokens or introduce numeric color literals in menu code. Resolve all menu colors through `crate::ui::theme::Palette`; add documented palette tokens there if existing tokens cannot express the needed hover or active-parent treatments.

## 3. Current implementation and design constraints

- [`overlay.rs`](../../src/app/ui_iced/overlay.rs) owns `MenuItem<Message>`, context-menu rendering, shared card chrome, and point clamping. Its current menu sizing assumes a fixed 200 px width and uniform 24 px rows.
- [`app.rs`](../../src/app/ui_iced/app.rs) stores context-menu state separately from hamburger-open and hamburger-submenu state.
- [`view.rs`](../../src/app/ui_iced/view.rs) mounts the menus in the existing overlay host. The host is the right integration point for shared placement and click-away behavior.
- [`panels/top.rs`](../../src/app/ui_iced/panels/top.rs) owns the hamburger's `MenuEntry` model and its bespoke card renderer. Its submenu navigation replaces the root contents with a submenu and a back row.
- [`panels/left.rs`](../../src/app/ui_iced/panels/left.rs) and [`panels/center.rs`](../../src/app/ui_iced/panels/center.rs) build tree and tab context-menu actions respectively.
- [`update.rs`](../../src/app/ui_iced/update.rs) resolves menu-open requests and routes menu messages. This remains the boundary for application state changes and command execution.
- [`message.rs`](../../src/app/ui_iced/message.rs) documents pointer positions from `MouseArea::on_move` as widget-relative.

**[INFERENCE] Coordinate-space risk:** tree rows and tabs report pointer movement from widget-local mouse areas, while [`view.rs`](../../src/app/ui_iced/view.rs) mounts their menus over the application-level viewport. The current path stores the reported point and uses it as the viewport-level anchor. The reusable widget must not rely on those coordinate spaces being interchangeable: triggers must supply a root-space point or a root-space anchor rectangle.

## 4. Proposed requirements

### Menu data and rendering

**MENU-001 — Shared component.** The hamburger, directory-tree context menu, and tab context menu shall use the same menu model, row renderer, placement resolver, and dismissal behavior.

**MENU-002 — Supported rows.** A menu shall support action, toggle, radio-choice, submenu, disabled, heading, and separator rows. Row descriptions may include optional leading icons, supporting text, trailing icons or text, shortcuts, and selection indicators. Callers supply all user-facing text and messages.

**MENU-003 — Pure menu surface.** The component shall render caller-provided data and emit caller-provided messages. It shall not read files, access domain state, or publish commands directly.

**MENU-004 — Shared styling.** Menus shall share card chrome, row metrics, spacing, typography, and visual states. Styling shall use FastMD palette tokens and remain consistent between root menus and submenu surfaces.

### Placement and nesting

**MENU-005 — Root point.** A menu shall accept an explicit root-space point in logical UI coordinates. Placement preferences shall control whether the surface opens left or right of the point and above or below it.

**MENU-006 — Rectangle anchor.** A menu shall accept an anchor rectangle in root-space coordinates, a preferred cardinal side (left, right, above, or below), alignment, and a flip policy.

**MENU-007 — Overflow resolution.** In automatic mode, placement shall use the requested side when it fits, flip to the opposite side when that fits, and otherwise choose the side with more available space before clamping. An explicitly requested direction shall be retained when possible and clamped when it cannot fully fit.

**MENU-008 — Constrained layout.** Menu width and height shall be constrained to the visible viewport. If the rows exceed available height, the menu shall scroll rather than render off-screen. Placement shall use dimensions that match the rendered surface, not only an item-count estimate.

**MENU-009 — Persistent parent surfaces.** Opening a submenu shall keep its parent and all ancestors visible and interactive. The active item shall identify which parent row owns the open child. Activating a different sibling submenu shall replace only that parent's child branch.

**MENU-010 — Root-space trigger contract.** Menu-opening triggers shall provide either a root-space point or a root-space anchor rectangle. Any widget-relative pointer position must be converted before the reducer stores it as root-space data.

### Selection, dismissal, and accessibility

**MENU-011 — Controlled toggles.** Toggle and radio rows shall display state from the current view model and emit a message that expresses the requested new state or selection. The renderer shall not mutate their state locally.

**MENU-012 — Toggle dismissal policy.** Checkable toggle rows shall remain open after activation by default, allowing consecutive changes. The row may opt into closing. Ordinary actions and radio choices shall close the menu by default, with an explicit override for callers.

**MENU-013 — Click-away.** A click outside all open menu surfaces shall close the whole menu tree. A click inside any open parent or child surface shall not be treated as click-away.

**MENU-014 — Escape and keyboard navigation.** Escape shall close the deepest open submenu first and then close parent levels one at a time. Keyboard focus shall be visible; Enter/Space shall activate a row, Right shall open a submenu, and Left shall return to its parent. Disabled and informational rows shall not emit action messages.

**MENU-015 — Existing behavior preservation.** Migration shall preserve the existing menu labels, row ordering, action messages, availability conditions, hamburger checkmarks, dialog ellipses, and placeholders required by the linked UI requirements. The proposed keep-open default for toggle rows is a deliberate interaction change from the current hamburger reducer, which closes after row activation; confirm this change during review or configure the existing window toggles to close.

## 5. Suggested model and open state

Use a recursive menu description, generic over the application message type:

```text
MenuNode<Message>
  Action { label, message, enabled, icon?, shortcut?, close_policy }
  Toggle { label, checked, message, enabled, indicator, close_policy }
  Radio   { label, selected, group, message, enabled, close_policy }
  Submenu { label, children, enabled }
  Heading { label }
  Separator
```

The `message` for a toggle should already contain the intended value (for example, “set visible to false”), consistent with the app's self-contained command/message flow. A radio row carries its selected value. The component only reports the message; `update` and view-model rebuilding remain responsible for application state.

Replace the split menu-open fields with one reducer-owned menu state containing the menu owner, menu tree or source, root placement, and active submenu path. A stable item identifier should identify the active path across view rebuilds. The overlay host should mount a single click-away blocker behind the complete menu tree, with child surfaces drawn and hit-tested above their parents.

Suggested module boundary: `src/app/ui_iced/menu.rs` for the model, row renderer, placement resolver, and menu tests. Keep menu definitions in the top/left/center panel modules and use the existing overlay host for z-order. If the new module grows substantially, split it by concern rather than adding more menu-specific branches to `overlay.rs` or `view.rs`.

## 6. Placement contract

Use one coordinate space for all menu placement: logical coordinates rooted at the top-left of the application viewport.

- `RootPoint(point)` uses the point as a pivot. Horizontal and vertical preferences independently determine the initial opening quadrant; the default for pointer context menus is right and down.
- `AnchorRect(rect)` is used for controls and rows. It accepts a cardinal side and alignment. A hamburger can prefer below and align its right edge; a submenu can prefer right and align to the parent row's start edge.
- `Auto` placement tests the preferred side, then its opposite, and chooses the roomier side if neither fully fits. Apply the configured gap only once. Clamp after choosing the side.
- If a surface is larger than the viewport, constrain its dimensions and scroll its contents. A tiny window must not produce a negative origin or an unclickable off-screen menu.
- A child submenu anchors to its parent row's bounds, not to the root menu's original pointer point.

## 7. Visual direction

- Use one raised card surface, subtle border, and consistent corner radius for all three entry points.
- Rows use shared leading, label/supporting-text, and trailing slots so icons, checkmarks/toggles, shortcuts, and submenu chevrons align vertically.
- Use clear but restrained hover and active-parent backgrounds, a visible keyboard-focus treatment, pressed feedback, and legible disabled/muted content.
- Use headings and dividers to group long menus; reserve supporting text or badges for cases where they add useful information.
- Keep the hamburger's existing `✓` selection meaning and `>` submenu cue unless the UI requirements are explicitly revised.
- Submenu transitions may use a subtle motion or shape emphasis if the iced implementation supports it without complicating interaction or tests. Motion is not required for the first implementation; stable geometry and clear focus are.

## 8. Migration and verification

1. Add pure placement and menu-state models, with table-driven unit tests before changing the renderers.
2. Add the shared menu widget and test row projection, disabled behavior, toggle messages, and child/parent hit regions.
3. Convert the hamburger model to recursive rows. Preserve UI-061b..UI-066 ordering and row behavior; confirm the toggle-dismissal policy called out in MENU-015.
4. Convert tree and tab context-menu builders without changing their emitted commands or messages.
5. Unify overlay state and reducer transitions; wire tree/tab triggers to root-space anchors and preserve existing overlay z-order.
6. Add interaction tests using the real `view` and reducer for opening, nesting, selection, click-away, and Escape unwinding. Add visual snapshots for the shared surface and active submenu state.

Acceptance tests should cover all placement directions at each viewport edge, forced and automatic placement, tiny viewports, nested siblings, parent visibility, click-away across the complete menu tree, toggles that remain open, action selection that closes, disabled rows, and preservation of the existing menu commands and labels.

## 9. Traceability and source references

- Hamburger menu: [UI-061..UI-066 and UI-061b..UI-061e](../../src/app/ui/SPEC.md); About entry [REQ-001](../../src/app/SPEC.md).
- Directory-tree context menu: [UI-006, UI-023, UI-025](../../src/app/ui/SPEC.md).
- Tab context menu: [UI-026..UI-027](../../src/app/ui/SPEC.md).
- Shared menu and placement primitives: [`src/app/ui_iced/overlay.rs`](../../src/app/ui_iced/overlay.rs), [`src/app/ui_iced/view.rs`](../../src/app/ui_iced/view.rs), [`src/app/ui_iced/app.rs`](../../src/app/ui_iced/app.rs), [`src/app/ui_iced/update.rs`](../../src/app/ui_iced/update.rs).
- Menu definitions and triggers: [`panels/top.rs`](../../src/app/ui_iced/panels/top.rs), [`panels/left.rs`](../../src/app/ui_iced/panels/left.rs), [`panels/center.rs`](../../src/app/ui_iced/panels/center.rs), [`message.rs`](../../src/app/ui_iced/message.rs).
- Existing palette contract: [`src/app/ui/theme.rs`](../../src/app/ui/theme.rs).
- Interaction/accessibility review: [`doc/planning/UI-Feedback.md`](UI-Feedback.md).
- Material 3 inspiration: [Menus — specs](https://m3.material.io/components/menus/specs).
