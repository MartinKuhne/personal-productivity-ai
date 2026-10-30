//! Standardized error codes for structured logging (NFR-004).
//!
//! Every `ERROR` and `FATAL` level log MUST include an error code from this module
//! to facilitate automated grouping, filtering, and alerting.
//!
//! Format: `SUBSYSTEM-NNNN` where subsystem is 3-4 chars and NNNN is a 4-digit number.

/// Agent subsystem error codes (AGENT-1xxx)
pub mod agent {
    /// Agent driver channel closed unexpectedly; prompt dropped
    pub const CHANNEL_CLOSED: &str = "AGENT-1001";
    /// Agent launch failed (thread spawn or initial send failed)
    pub const LAUNCH_FAILED: &str = "AGENT-1002";
    /// Agent session cancelled by user
    pub const CANCELLED: &str = "AGENT-1003";
    /// LLM client error (API, network, auth)
    pub const LLM_CLIENT_ERROR: &str = "AGENT-1004";
    /// Tool execution failed in agent context
    pub const TOOL_EXECUTION_FAILED: &str = "AGENT-1005";
}

/// Tool execution subsystem error codes (TOOL-2xxx)
pub mod tool {
    /// Tool not found in registry
    pub const NOT_FOUND: &str = "TOOL-2001";
    /// Tool execution failed (returned error status)
    pub const EXECUTION_FAILED: &str = "TOOL-2002";
    /// Tool call policy denied the call
    pub const POLICY_DENIED: &str = "TOOL-2003";
    /// Tool arguments invalid or missing
    pub const INVALID_ARGUMENTS: &str = "TOOL-2004";
    /// Tool timeout
    pub const TIMEOUT: &str = "TOOL-2005";
}

/// File system / workspace error codes (FS-3xxx)
pub mod fs {
    /// File watcher failed to start or encountered error
    pub const WATCHER_FAILED: &str = "FS-3001";
    /// File read failed
    pub const READ_FAILED: &str = "FS-3002";
    /// File write failed
    pub const WRITE_FAILED: &str = "FS-3003";
    /// File delete failed
    pub const DELETE_FAILED: &str = "FS-3004";
    /// Directory scan failed
    pub const SCAN_FAILED: &str = "FS-3005";
    /// PDF conversion failed
    pub const PDF_CONVERSION_FAILED: &str = "FS-3006";
    /// Image processing failed
    pub const IMAGE_PROCESSING_FAILED: &str = "FS-3007";
}

/// Configuration subsystem error codes (CFG-4xxx)
pub mod config {
    /// Configuration load failed
    pub const LOAD_FAILED: &str = "CFG-4001";
    /// Configuration save failed
    pub const SAVE_FAILED: &str = "CFG-4002";
    /// Configuration validation failed
    pub const VALIDATION_FAILED: &str = "CFG-4003";
    /// Secrets load failed
    pub const SECRETS_LOAD_FAILED: &str = "CFG-4004";
}

/// Indexer / search subsystem error codes (IDX-5xxx)
pub mod indexer {
    /// Index build failed
    pub const BUILD_FAILED: &str = "IDX-5001";
    /// Vector search failed
    pub const VECTOR_SEARCH_FAILED: &str = "IDX-5002";
    /// Embedding generation failed
    pub const EMBEDDING_FAILED: &str = "IDX-5003";
}

/// Background worker error codes (BG-6xxx)
pub mod background {
    /// Background task spawn failed
    pub const TASK_SPAWN_FAILED: &str = "BG-6001";
    /// Worker thread panic
    pub const WORKER_PANIC: &str = "BG-6002";
    /// Background channel closed unexpectedly
    pub const CHANNEL_CLOSED: &str = "BG-6003";
}

/// UI / rendering error codes (UI-7xxx)
pub mod ui {
    /// Font registration failed
    pub const FONT_LOAD_FAILED: &str = "UI-7001";
    /// Theme loading failed
    pub const THEME_LOAD_FAILED: &str = "UI-7002";
    /// View model rebuild failed
    pub const VIEW_MODEL_REBUILD_FAILED: &str = "UI-7003";
}

/// MCP integration error codes (MCP-8xxx)
pub mod mcp {
    /// MCP server connection failed
    pub const CONNECTION_FAILED: &str = "MCP-8001";
    /// MCP authentication failed
    pub const AUTH_FAILED: &str = "MCP-8002";
    /// MCP tool call failed
    pub const TOOL_CALL_FAILED: &str = "MCP-8003";
}

/// Discord integration error codes (DISC-9xxx)
pub mod discord {
    /// Discord gateway connection failed
    pub const GATEWAY_CONNECTION_FAILED: &str = "DISC-9001";
    /// Discord command handling failed
    pub const COMMAND_FAILED: &str = "DISC-9002";
    /// Discord rate limit exceeded
    pub const RATE_LIMIT_EXCEEDED: &str = "DISC-9003";
}

/// Menu surface error codes (MENU-1xxx … MENU-5xxx) — NFR-001 / NFR-004.
///
/// A menu request passes through five places, and the thousands digit names
/// the one that spoke, so a filtered log reads in pipeline order:
///
/// | Block | Route |
/// | --- | --- |
/// | `1xxx` | hamburger open request |
/// | `2xxx` | context-menu open request |
/// | `3xxx` | card geometry / rect registration |
/// | `4xxx` | row activation and intent routing |
/// | `5xxx` | hover-aim corridor (submenu open) |
///
/// The level is **not** encoded in the code and is stated on each constant.
/// [NFR-001] requires an `ERROR` for a *failure*, and not every refusal is a
/// failure: an idempotent re-open of a menu that is already showing is normal
/// interaction and is logged `DEBUG`, while a card that opened and then cannot
/// be painted is broken and is logged `ERROR`.
pub mod menu {
    /// Hamburger open request was a no-op because that exact tree is already
    /// showing (`IgnoredReason::AlreadyOpen`). Benign: the caller re-issues
    /// "open" as an idempotent statement and the cascade refuses so the card
    /// cannot flicker closed-and-open. Logged `DEBUG`.
    pub const HAMBURGER_ALREADY_OPEN: &str = "MENU-1001";
    /// Hamburger open refused because the built menu has no keyboard-navigable
    /// row (`IgnoredReason::NoFocusableRows`) — every row is a heading, a
    /// separator or disabled, so no card can mount and the user's click does
    /// nothing. A failure, not a refusal: logged `ERROR`.
    pub const HAMBURGER_NO_FOCUSABLE_ROWS: &str = "MENU-1002";
    /// Hamburger open refused for a cascade reason that `open` should never
    /// return (`NotOpen`, `UnknownCard`, …). The menu builder and the cascade
    /// disagree about the tree; logged `ERROR`.
    pub const HAMBURGER_OPEN_REFUSED: &str = "MENU-1003";

    /// A tree context-menu was requested for a path the left-panel view-model
    /// does not contain, so no target and no row set could be built. The menu
    /// renders nothing. Logged `ERROR`.
    pub const CONTEXT_TREE_ROW_MISSING: &str = "MENU-2001";
    /// A tab context-menu was requested for an index outside the tab strip.
    /// The menu renders nothing. Logged `ERROR`.
    pub const CONTEXT_TAB_MISSING: &str = "MENU-2002";
    /// [`ContextMenu::open`](crate::ui_iced::app::ContextMenu::open) returned
    /// `None`: the cascade refused the built context menu, so the surface was
    /// never mounted. Logged `ERROR` — the user right-clicked and got nothing.
    pub const CONTEXT_OPEN_REFUSED: &str = "MENU-2003";
    /// No cursor position had been reported when a pointer-anchored menu was
    /// requested, so the anchor fell back to the window origin. The menu still
    /// opens, but at `(0,0)` rather than under the pointer. Degraded rather
    /// than broken: logged `WARN`.
    pub const CONTEXT_ANCHOR_MISSING: &str = "MENU-2004";

    /// An open card was dropped from the solved rectangle list because its
    /// **parent card never received a rectangle**, so the trigger row inside it
    /// cannot be derived. The card is open in the cascade but never paints.
    /// Logged `ERROR`.
    pub const CARD_PARENT_RECT_MISSING: &str = "MENU-3001";
    /// An open card was dropped because its trigger names a parent card the
    /// cascade does not hold. The card is open but never paints. Logged
    /// `ERROR`.
    pub const CARD_PARENT_MENU_MISSING: &str = "MENU-3002";
    /// An open card was dropped because the trigger row index is past the end
    /// of the parent menu, so no row rectangle exists to anchor to. The card
    /// is open but never paints. Logged `ERROR`.
    pub const CARD_ROW_RECT_MISSING: &str = "MENU-3003";
    /// The overlay was handed a rectangle list whose length disagrees with the
    /// cascade's open-card list. Because the two are zipped index-for-index,
    /// every card after the shortfall is painted at another card's rectangle
    /// and the last card(s) are not painted at all. Logged `ERROR`.
    pub const RECT_COUNT_MISMATCH: &str = "MENU-3004";

    /// A context-menu row pick arrived with no context menu mounted — a stale
    /// click landing after the surface was already dismissed. The intent is
    /// dropped. Racy rather than broken: logged `WARN`.
    pub const ROW_PICK_NO_SURFACE: &str = "MENU-4001";
    /// A context-menu row was activated but carries no entry in the surface's
    /// routing table, so the click resolves to no intent. The row is dead.
    /// A builder gap: logged `ERROR`.
    pub const ROW_INTENT_MISSING: &str = "MENU-4002";
    /// Context-menu keyboard navigation arrived with no context menu mounted.
    /// Racy rather than broken: logged `WARN`.
    pub const KEY_NAV_NO_SURFACE: &str = "MENU-4003";
    /// A hamburger row activation resolved to no
    /// [`UserCommand`](crate::bus::events::user_command::UserCommand)
    /// (`command_for_activation` returned `None` for the row's key, or the
    /// card snapshot was already gone). The user clicked a live row and
    /// nothing happened. Logged `ERROR`.
    pub const ACTIVATION_UNRESOLVED: &str = "MENU-4004";

    /// Hovering a submenu parent opened no card — the aim corridor held the
    /// switch, or the branch refused to open. Usually transient (the deadline
    /// may still land the switch), so it is not a failure on its own: logged
    /// `DEBUG`.
    pub const SUBMENU_HOVER_NOT_OPENED: &str = "MENU-5001";
    /// A row hover report arrived while no hamburger menu was open, so it was
    /// ignored. Expected during the frame between dismissal and the widget's
    /// own unhover: logged `DEBUG`.
    pub const HOVER_WHEN_MENU_CLOSED: &str = "MENU-5002";
}
