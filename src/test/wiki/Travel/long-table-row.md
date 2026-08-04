<!-- Fixture: long-table-row.md
     Targets: F1 (horizontal clip), F3 (table column overflow).
     Used by: tests under src/desktop/src/ui/render.rs::tests and
     src/desktop/tests/off_viewport_text_test.rs to verify the §3.6
     fallback path engages (ScrollArea::horizontal wraps the table)
     and no text is left permanently off-viewport. -->

# Cruise Comparison: Long-Form Pricing and Cabin Notes

A compact comparison of three mid-size cruise lines operating week-long
Caribbean loops in Q4. Use this fixture to exercise the markdown table
fallback path on narrow viewports; the prose is intentionally short so
the table dominates.

| Line | Weekly Interior | Weekly Oceanview | Weekly Balcony | Loyalty Tier | Cabin Notes |
|------|-----------------|------------------|----------------|--------------|-------------|
| Coral Voyager | $1,299 | $1,649 | $2,099 | Sapphire | Compact shower, no minibar, USB-A bedside, ethernet optional, 110V only, no coffee maker, single safe |
| Northern Lights | $1,455 | $1,805 | $2,295 | Pearl | Larger shower, kettle, USB-C bedside, 220V available, in-room espresso, dual safes, blackout curtains, premium linens |
| Reef Drifter | $989 | $1,389 | $1,799 | Reef Club | Family-friendly bunk configuration, mini fridge, kid channels, 110V/220V, twin safes, deck chairs, pool towel service |

## Notes

- Prices are lead-in rates and exclude gratuities, port fees, and beverage packages.
- "Loyalty Tier" is the entry tier; upgrades require 2+ sailings.
- The table is intentionally wider than a typical 320-768px viewport so
  the §3.6 fallback (`render_table` in `src/ui/render.rs`) wraps it in
  `ScrollArea::horizontal` and the user can scroll to see every column.
