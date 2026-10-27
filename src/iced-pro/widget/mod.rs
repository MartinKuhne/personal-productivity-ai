//! The atomic widgets of the framework (`UIF-ATOM-*`).
//!
//! An atom is the smallest thing in the interface that has a complete
//! interaction state machine: it can rest, be hovered, be pressed, hold
//! focus, be disabled and — where it carries a value — be invalid. §3 of
//! `spec.md` at this crate's root specifies each atom as a matrix of
//! those states, and this subsystem is where those matrices become code.
//!
//! Each atom here has two halves, kept deliberately separate. The
//! **resolver** is a pure function from `(variant, state, palette)` to a
//! resolved style value — that is what makes every cell of the spec's
//! matrices checkable without a window, a renderer or a GPU, and it is
//! what the `UIF-VER-001` and `UIF-VER-003` gates assert on. The
//! **widget** is the iced `Element` that paints that value and drives the
//! state from real pointer and keyboard events.
//!
//! Keeping them apart is not ceremony. A widget whose colour logic lives
//! inside its `draw` cannot be audited for contrast; a resolver with no
//! widget cannot be seen. Both halves exist so the audit and the artifact
//! are each checkable on their own terms.
//!
//! Like [`crate::token`], this module does **not** glob-re-export its
//! children. Atom type names collide across the set — every atom will want
//! to call its own resolved paint a `Style` — so call sites stay
//! qualified: [`button::ButtonStyle`], [`button::resolve_style`],
//! [`focus::resolve_ring_color`].
//!
//! * [`button`] — the four button variants across the full state matrix
//!   (`UIF-ATOM-001`)
//! * [`button_widget`] — the rendering, keyboard-focusable button that
//!   composes those resolvers into a real iced widget, the first atom in
//!   the framework to paint (`UIF-ATOM-001`, `UIF-PRIN-005`)
//! * [`button_model`] — the declarative `ButtonModel` a host owns and hands
//!   to the button each frame: label, icons, variant, declared density, busy
//!   and cancellation, and the tooltip a collapsed rung owes
//!   (`UIF-ATOM-001a`)
//! * [`button_controller`] — the button's activation engine: the linear
//!   gesture slot, the busy / double-activation gate and the three emission
//!   tiers a button can put on the wire (`UIF-ATOM-001a`, §3.0.4.3)
//! * [`focus`] — the adaptive keyboard focus rule every widget with a
//!   focus indicator resolves through, rather than re-deriving it per
//!   variant: one colour rule, two placements — the `2px` outer ring for
//!   buttons and inputs and the `1px` inset outline for menu and tree
//!   rows (`UIF-TOK-006` rule 2, `UIF-ATOM-000` §3.0.2, decision **D-6**)
//! * [`focusable`] — the wrapper that registers a widget with iced's
//!   focus router, the mechanism decision **D-7** rests on and the
//!   reason a custom widget can hold keyboard focus at all
//!   (`UIF-PRIN-005`, gap **G-10**)
//! * [`text_input`] — the text entry, search field and dropdown trigger
//!   across the five-state matrix of §3.2, including the trailing slot
//!   that is reserved whether or not anything is in it
//!   (`UIF-ATOM-002`, `UIF-PRIN-003`)
//! * [`text_input_model`] — the declarative `TextInputModel` a host owns
//!   and hands to the field each frame: value, placeholder, selection,
//!   the host's own validation words, and the read-only / disabled /
//!   clearable flags that decide what the trailing slot may do
//!   (`UIF-ATOM-002a`)
//! * [`text_input_controller`] — the field's editing engine: the editing
//!   session and its snapshot, the coalesced live tier, and the four
//!   `002a` callbacks as sum-type arms rather than four loose closures
//!   (`UIF-ATOM-002a`, §3.0.4.2)
//! * [`password_model`] — the secret-entry model: a `SecretInput` whose
//!   `Debug` prints the mask rather than the password, and the reveal
//!   policy that keeps a shown secret inside the focused control
//!   (`UIF-ATOM-002c`)
//! * [`password_host`] — the reveal transcript and the tier translation
//!   that make a password field the `002a` atom with masking on, rather
//!   than a second text field (`UIF-ATOM-002c`)
//! * [`auto_suggest_model`] — the completion model: the query, the
//!   candidates, the highlight, the busy flag, and the flyout geometry
//!   §3.2 fixes at `240px` / `32px` rows / `4px` anchor
//!   (`UIF-ATOM-002d`)
//! * [`auto_suggest_host`] — the request lifecycle neither the field nor
//!   the menu engine can own: the clock-as-value debounce, the generation
//!   token that refuses late answers, the two-stage `Escape`, and the
//!   three commit sources (`UIF-ATOM-002d`)
//! * [`text_area_model`] — the multi-line model: `LineCol`, the
//!   `min_rows`/`max_rows` growth solver, the scroll extent and the
//!   undo/redo ledger §3.2 asks for and iced does not provide
//!   (`UIF-ATOM-002e`)
//! * [`text_area_host`] — the multi-line editing host: the atom's slot
//!   partition and tier engine applied to a wrapping editor, with
//!   `Enter` inserting a newline and `Ctrl+Enter` submitting
//!   (`UIF-ATOM-002e`)
//! * [`toggle`] — the paint resolver and geometry for the three
//!   boolean/choice atoms: one two-row polarity matrix shared by
//!   `Checkbox`, `Radio` and `ToggleSwitch`, the adaptive ring that
//!   reproduces both of §3.3's printed ring colours by arithmetic, and
//!   the hit target that extends across the inline label
//!   (`UIF-ATOM-003`)
//! * [`toggle_model`] — the three §3.3 data models and their pure
//!   transitions: the deterministic tri-state rule, the roving
//!   radio group, and the switch's busy refusal
//!   (`UIF-ATOM-003`, §3.0.4.1)
//! * [`toggle_widget`] — the live, interactive `iced::Widget`s for
//!   `Checkbox`, `RadioGroup` and `ToggleSwitch` (`UIF-ATOM-003`, §3.3)
//! * [`badge`] — the paint resolver and geometry for the three §3.8
//!   badges: the `6px` dot, the count pill that folds at `99+`, and the
//!   status chip whose leading glyph is mandatory. None of the three is
//!   interactive, so the resolver has no hover, pressed or focus arm to
//!   invent one for (`UIF-ATOM-008`)
//! * [`badge_model`] — the declarative `BadgeModel` a host owns: the
//!   `Dot` / `Count` / `Status` sum and the four tones, with the tone
//!   vocabulary resolved against §3.8 rather than guessed from the
//!   status palette's names (`UIF-ATOM-008`)
//! * [`chip`] — the paint resolver and geometry for the §3.6 chip: the
//!   two height rungs, the unselected/selected polarity matrix, the
//!   reserved `24 × 24` trailing `×` target, and the `Assist` radius
//!   gap §3.6 leaves open and this module resolves rather than guesses
//!   through (`UIF-ATOM-006`)
//! * [`chip_model`] — the declarative `ChipModel<Id>` a host owns: the
//!   `Assist` / `Filter { selected }` / `Input { removable }` sum, the
//!   height rung, and the one `ChipEvent` each gesture produces — the
//!   variant-decides-the-callback rule §3.6 states but does not spell out
//!   (`UIF-ATOM-006`)
//! * [`progress`] — the paint resolver and geometry for the §3.9
//!   `ProgressBar` and `Spinner`: the `4px` track, the status-coloured
//!   fill, the `35%` indeterminate segment whose position arrives as an
//!   injected phase, and the static stripe that replaces it under
//!   injected reduced motion (`UIF-ATOM-009`)
//! * [`progress_model`] — the declarative `ProgressModel` a host owns:
//!   determinate versus indeterminate, the three statuses, and the
//!   cancellation flag that decides whether the stop button exists at
//!   all (`UIF-ATOM-009`)
//! * [`range_model`] — the one interval both §3.5 controls share: the
//!   clamp that tests finiteness before narrowing, the four edge policies
//!   the spec leaves open, and the grid-anchored step that does not drift
//!   (`UIF-ATOM-005`)
//! * [`slider`] — the paint resolver and geometry for the §3.5 slider:
//!   the `4px` pill track, the filled rail, the `16px` thumb on its
//!   `24px` hit target, and the fill-on-fill judgement that measurement
//!   forced rather than the one the spec printed
//!   (`UIF-ATOM-005`, `UIF-TOK-006`)
//! * [`slider_host`] — the drag engine: pointer capture that keeps the
//!   thumb tracking past the end of the track, the `Live` frame per
//!   movement, the `Commit` that fires exactly once on release, and the
//!   `Escape` that hands back the pre-drag value (`UIF-ATOM-005`,
//!   §3.0.4.3)
//! * [`number_box`] — the paint resolver and geometry for the §3.5
//!   numeric entry: the mono `32px` field, the stacked and side-by-side
//!   stepper rungs, and the two stepper targets resolved as buttons
//!   without being buttons (`UIF-ATOM-005`)
//! * [`number_box_host`] — the stepper and entry engine: click, hold and
//!   typed-text commits, with the hold cadence arriving as a value rather
//!   than being read from a clock (`UIF-ATOM-005`, `UIF-PRIN-007`)
//! * [`mod@range_slider`] — the paint resolver, geometry and live interactive
//!   widget for the §3.5 dual-thumb interval control: 4px pill rail,
//!   16px thumbs with 24px hit targets, non-crossing invariant, tabular-numeral
//!   readout pill, roving tab focus and the three-tier scrub contract
//!   (`UIF-ATOM-005b`, `UIF-ATOM-005`)
//! * [`range_slider_model`] — the declarative `RangeSliderModel` and `RangeThumb`
//!   a host owns: the non-crossing clamp, endpoint disambiguation for coincident
//!   thumbs, and drift-free stepping (`UIF-ATOM-005b`, `UIF-ATOM-005`)
//! * [`rove`] — the single-focus walk every roving group shares: skip
//!   the disabled members, wrap at both ends, and never lose the
//!   keyboard on a member that was disabled underneath the focus
//!   (`UIF-ATOM-003`, `UIF-ATOM-001b`)
//! * [`segmented`] — the paint resolver and geometry for the §3.1b
//!   joined strip: every unselected segment is the `Secondary` button
//!   resolver passed straight through, the selected segment *is* its
//!   `Pressed` cell, and the `✓` slot is reserved on every segment so
//!   selection moves no label (`UIF-ATOM-001b`)
//! * [`segmented_model`] — the declarative `SegmentedModel<Id>` a host
//!   owns: the single/multi-select set transition, the shared rove, and
//!   the one-segment strip a `ToggleButton` is built from
//!   (`UIF-ATOM-001b`)
//! * [`split_button`] — the geometry and paint resolver for the §3.1c
//!   two-zone control: which half a point fell in, the `28px` chevron
//!   zone, and both halves resolved through the button atom so a Primary
//!   split button carries the fill/foreground lock the button already
//!   asserted (`UIF-ATOM-001c`)
//! * [`split_button_model`] — the declarative `SplitButtonModel<ActionId>`
//!   a host owns: a contained `ButtonModel` as the primary half, and
//!   flyout rows that build the engine's own `Menu` on demand
//!   (`UIF-ATOM-001c`)
//! * [`split_button_host`] — the composition: one `MenuCascade`, one
//!   `ButtonController`, and the emission set that keeps the primary
//!   half's three tiers and the menu's open/select events distinguishable
//!   (`UIF-ATOM-001c`)
//! * [`hyperlink`] — the paint resolver, geometry and focus state for
//!   the §3.1d inline text control: no border and no resting fill, the
//!   underline as the non-colour cue, and a ring resolved against the
//!   mount because the control paints nothing under itself
//!   (`UIF-ATOM-001d`)
//! * [`hyperlink_model`] — the declarative `HyperlinkModel` a host owns,
//!   and the two tiers it emits: an activation carrying a
//!   [`HyperlinkTarget`](crate::widget::hyperlink_model::HyperlinkTarget)
//!   rather than a bare string, and a status-bar preview that clears to
//!   `None` (`UIF-ATOM-001d`)
//! * [`mod@date_picker`] — the DatePicker, DateRangePicker & CalendarView widgets
//!   with pure civil calendar arithmetic, 42-cell fixed day grid, and
//!   redundancy cues (§3.10, `UIF-ATOM-010`, `UIF-PRIN-003`, `UIF-PRIN-005`)
//! * [`mod@time_picker`] — the TimePicker widget with civil time arithmetic,
//!   segmented trigger, drum flyout and three-tier callbacks (§3.11, `UIF-ATOM-011`, `UIF-PRIN-003`, `UIF-PRIN-005`)
//! * [`mod@color_picker`] — the ColorPicker & ColorSwatchButton widgets with
//!   Rgba8 / Hsva models, 2D SV canvas, sliders, preset swatches and contrast readout (§3.12, `UIF-ATOM-012`, `UIF-TOK-006`, `UIF-PRIN-005`)
//!
//! Atoms read every visual number from [`crate::token`] and never
//! restate it (`UIF-PRIN-001`). The only numeric literals an atom file
//! may carry are the absence of a property — a zero-width ring, a fully
//! transparent fill — and the opacity modifiers the spec itself prints,
//! and in both cases the value is a named constant rather than an inline
//! number.

pub mod auto_suggest_host;
pub mod auto_suggest_model;
pub mod avatar;
pub mod badge;
pub mod badge_model;
pub mod button;
pub mod button_controller;
pub mod button_model;
pub mod button_widget;
pub mod chip;
pub mod chip_model;
pub mod color_picker;
pub mod combo_box;
pub mod combo_host;
pub mod combo_model;
pub mod date_picker;
pub mod focus;
pub mod focusable;
pub mod hyperlink;
pub mod hyperlink_model;
pub mod number_box;
pub mod number_box_host;
pub mod password_host;
pub mod password_model;
pub mod progress;
pub mod progress_model;
pub mod range_model;
pub mod range_slider;
pub mod range_slider_model;
pub mod rating;
pub mod rove;
pub mod search_box;
pub mod segmented;
pub mod segmented_model;
pub mod slider;
pub mod slider_host;
pub mod split_button;
pub mod split_button_host;
pub mod split_button_model;
pub mod text_area_host;
pub mod text_area_model;
pub mod text_input;
pub mod text_input_controller;
pub mod text_input_model;
pub mod time_picker;
pub mod toggle;
pub mod toggle_model;
pub mod toggle_widget;

pub use crate::surface::tooltip::{Tooltip, TooltipContent, TooltipState, tooltip};
pub use avatar::{Avatar, AvatarModel, AvatarShape, AvatarSize, avatar};
pub use color_picker::{
    ColorPicker, ColorPickerModel, ColorSwatchButton, Hsva, Rgba8, color_picker,
    color_swatch_button,
};
pub use combo_box::{ComboBox, ComboRung, combo_box};
pub use combo_model::{ComboBoxModel, SelectOption};
pub use date_picker::{
    CalendarView, CalendarViewMode, CivilDate, DatePicker, DatePickerModel, DateRangePicker,
    DateSelection, Weekday, YearMonth, calendar_view, date_picker, date_range_picker,
};
pub use range_slider::{RangeSlider, range_slider};
pub use range_slider_model::{RangeSliderModel, RangeThumb};
pub use rating::{RatingControl, RatingModel, RatingStep, rating_control};
pub use search_box::{
    GLYPH_CLEAR, GLYPH_SEARCH, SEARCH_ACTION_SIZE, SEARCH_INPUT_HEIGHT, SEARCH_PLACEHOLDER,
    SearchBox, SearchBoxModel,
};
pub use time_picker::{AmPm, CivilTime, TimePicker, TimePickerModel, TimeSegment, time_picker};
pub use toggle_widget::{Checkbox, RadioGroup, ToggleSwitch};
