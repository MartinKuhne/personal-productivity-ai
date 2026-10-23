//! The composite widgets of the framework (`UIF-COMP-*`).
//!
//! A composite is not an atom with more parts. An atom
//! ([`crate::widget`]) owns an interaction state machine for one control;
//! a composite owns the *relationship between* controls and the text that
//! explains them � the vertical rhythm of a labelled field, the horizontal
//! split of a settings row, the three slots of a list tile. �3B of
//! `doc/planning/ui-framework.md` specifies those relationships, and this
//! subsystem is where they become code.
//!
//! The same split that runs through [`crate::widget`] runs through here, and
//! for the same reason. The **resolver** is a pure function from a small
//! declarative input to rectangles and resolved colours; the **widget** is
//! the iced `Element` that lays out to those rectangles and paints them.
//! Keeping the geometry pure is what makes the requirements checkable at
//! all, because the two things �3B actually demands of a composite are
//! facts about geometry rather than facts about pixels:
//!
//! * **A band that is reserved is a band that is measured.** `UIF-PRIN-003`
//!   forbids a state change from moving content. In a form field that means
//!   the caption slot must exist at its full height whether or not anything
//!   is written in it, and the only way to *prove* that rather than observe
//!   it is a geometry function that cannot see the caption's content. Every
//!   reserved band in this subsystem is computed by such a function � the
//!   absence of the parameter is the proof.
//! * **A slot the spec fixes is a slot the host does not get to choose.**
//!   Decision **D-8** requires the `ListTile`'s slot geometry to be
//!   computed here rather than left to whatever lays the row out, because a
//!   host that invents its own leading-box width invents a different list on
//!   every surface.
//!
//! Like [`crate::token`] and [`crate::widget`], this module does not
//! glob-re-export its children; call sites stay qualified.
//!
//! * [`form_field`] � the stacked label / control / reserved-caption field
//!   whose total rhythm is fixed so a validation transition moves nothing
//!   (`UIF-COMP-001`, `UIF-PRIN-003`)
//! * [`settings_row`] � the horizontal split: a filling text column and a
//!   shrinking control column on one raised card row (`UIF-COMP-001`)
//! * [`card`] � the grouping container: `surface.raised`, `radius.card`,
//!   `space.lg` padding, and the resting-vs-elevated border (`UIF-COMP-005`)
//! * [`list_tile`] � the three-slot row: leading mark, filling text,
//!   shrinking trailing targets, with the hit target resolved separately
//!   for the interactive and the inert tile (`UIF-COMP-005`)
//! * [`virtual_list`] - the recycling primitive: fixed-height row metrics that
//!   bound realized rows independently of the total row count, so a
//!   1 000 000-row list realises exactly what a 100 000-row one does, and
//!   never a prefix sum (`UIF-COMP-006`)
//! * [`tree_row`] - the framework-owned tree row: the depth-indent policy, the
//!   row slot layout, and the D-6 contrast adaptation behind every mark a row
//!   paints, including the ones `fastmd` never drew (decision **D-8**)
//! * [`tree_view`] - the tree model: visible-row flattening, single and multi
//!   selection keyed by node id, the keyboard contract, and the
//!   scroll-into-view handoff to [`virtual_list`]
//!
//! Every number comes from [`crate::token`] (`UIF-PRIN-001`). The three
//! figures �3B prints that no rung carries � the `20px` field header, the
//! `48px` settings-row floor and the `44px` two-line tile � are registered
//! Surface Geometry Constants in [`crate::token_lint`] rather than passed
//! off as token reads.

pub mod card;
pub mod expander;
pub mod form_field;
pub mod info_bar;
pub mod list_tile;
pub mod notification;
pub mod scroll_view;
pub mod settings_row;
pub mod toast;
pub mod tree_row;
pub mod tree_view;
pub mod virtual_list;
