//! Static metadata for an LLM-callable tool — name, description,
//! parameter schema, safety, group ownership, and declarative
//! configuration requirements.
//!
//! `ToolDescriptor` is a `'static` value shared by every caller that
//! only needs metadata (the agent loop, the prompt builder, the UI
//! dialog, the tool registry's schema-fragment builders). Tools
//! themselves implement [`crate::tools::Tool`] and return
//! `&'static ToolDescriptor` from [`crate::tools::Tool::descriptor`].
//!
//! The companion [`ToolConfigSpec`] carries the configuration
//! requirements a tool needs in order to be enabled. Combined with
//! [`crate::config::AgentConfig`] and the current prompt, it is the
//! single source of truth for "is this tool currently offered to the
//! LLM?".
//!
//! Unit tests live in the sibling `descriptor_tests.rs` sidecar.

use crate::config::AgentConfig;
use crate::tools::Safety;
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use std::any::TypeId;
use std::borrow::Cow;

/// Latency class for an LLM-callable tool. Used by future
/// scheduling and UI affordances (a "Slow" tool may show a
/// different progress indicator than a "Fast" one). The agent
/// loop does not read this today; the data is here so the
/// caller can declare it without re-plumbing the descriptor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LatencyClass {
    /// A `ReadOnly` tool backed by an in-memory or local-disk
    /// source (filesystem search, tag read, list notes). Returns
    /// in well under a second.
    #[default]
    Fast,
    /// A `ReadOnly` tool that makes a network call (SearXNG
    /// search, web fetch, MCP server round-trip). Returns in a
    /// few seconds.
    Interactive,
    /// A tool that may take tens of seconds (MCP `tools/list`
    /// on a slow server, browser navigation, OCR pass). The UI
    /// should show a long-running affordance.
    Slow,
}

/// Whether the tool shares a long-lived resource across calls.
/// Today the only consumer is the browser session; future tools
/// that pin a connection (a long-lived SSH session, a
/// streaming MCP server) can declare the same kind of
/// sharing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SessionRequirement {
    /// The tool has no long-lived resource requirements.
    #[default]
    None,
    /// The tool shares the named session with any other tool
    /// that lists the same name. Calls against the same session
    /// are serialised.
    Shared(Cow<'static, str>),
}

/// Additional behavioural metadata for a tool, beyond
/// [`Safety`]. Today nothing reads this — it's here so the
/// caller can declare idempotency, latency, user-confirmation,
/// and session requirements without re-plumbing the descriptor.
/// Future work (a retry path that uses `idempotent`, a UI
/// progress indicator driven by `latency_class`, a confirmation
/// dialog driven by `requires_user_confirmation`, a per-session
/// serialiser driven by `session`) reads the data from here.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolProfile {
    /// Whether the tool is safe to re-run after a transient
    /// failure. `false` for tools that have a non-idempotent
    /// effect (e.g. `send_email`).
    pub idempotent: bool,
    /// Latency hint. Default: [`LatencyClass::Fast`].
    pub latency_class: LatencyClass,
    /// Whether the UI should require a user confirmation step
    /// before invoking this tool. Default: `false`.
    pub requires_user_confirmation: bool,
    /// Long-lived resource requirements. Default:
    /// [`SessionRequirement::None`].
    pub session: SessionRequirement,
}

impl ToolProfile {
    /// All defaults. Equivalent to [`Default::default`] but
    /// named for readability at the call site.
    pub fn defaults() -> Self {
        Self::default()
    }
}

/// Static metadata for one LLM-callable tool. Cheap to clone (`Cow`
/// + small `Value`) and safe to share across threads.
#[derive(Clone, Debug)]
pub struct ToolDescriptor {
    /// Tool name as it appears to the LLM (e.g. `"search_notes"`).
    pub name: Cow<'static, str>,
    /// Description shown to the LLM in the `tools` array.
    pub description: Cow<'static, str>,
    /// JSON Schema for the tool's input. Generated from the input
    /// DTO via `schemars`.
    pub parameters_schema: serde_json::Value,
    /// Compile-time identifier for the input DTO type. Lets the
    /// dispatch layer bind the JSON arguments to the right Rust
    /// struct without an extra match.
    pub input_type: TypeId,
    /// Whether the tool mutates user-visible state. Drives
    /// parallel-vs-sequential dispatch in the tool executor.
    pub safety: Safety,
    /// Configuration requirements and enable rules. See
    /// [`ToolConfigSpec`].
    pub config: ToolConfigSpec,
    /// Which group owns this tool — used to compute the per-group
    /// enable flag and to populate the UI dialog's group list.
    pub group: ToolGroupId,
    /// Additional behavioural metadata (idempotency, latency,
    /// user-confirmation, session requirements). See
    /// [`ToolProfile`].
    pub profile: ToolProfile,
}

impl ToolDescriptor {
    /// Build a descriptor whose `parameters_schema` is a JSON Schema
    /// for `I` (must be `schemars::JsonSchema`). The caller supplies
    /// the rest. `profile` defaults to [`ToolProfile::default`] —
    /// tools that need to declare non-default behavioural metadata
    /// (idempotency, latency, user confirmation, session
    /// requirements) use [`ToolDescriptor::with_profile`].
    pub fn new<I>(
        name: impl Into<Cow<'static, str>>,
        description: impl Into<Cow<'static, str>>,
        safety: Safety,
        config: ToolConfigSpec,
        group: ToolGroupId,
    ) -> Self
    where
        I: schemars::JsonSchema + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            parameters_schema: serde_json::to_value(schemars::schema_for!(I))
                .expect("schemars produced an unserialisable schema"),
            input_type: TypeId::of::<I>(),
            safety,
            config,
            group,
            profile: ToolProfile::default(),
        }
    }

    /// Build a descriptor from a precomputed JSON Schema `Value`.
    /// Used by the MCP adapter, which receives the schema over the
    /// wire as `serde_json::Value` and has no DTO type to bind to.
    pub fn with_json_schema(
        name: impl Into<Cow<'static, str>>,
        description: impl Into<Cow<'static, str>>,
        parameters_schema: serde_json::Value,
        safety: Safety,
        config: ToolConfigSpec,
        group: ToolGroupId,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters_schema,
            input_type: TypeId::of::<serde_json::Value>(),
            safety,
            config,
            group,
            profile: ToolProfile::default(),
        }
    }

    /// Replace the default `profile` with a caller-supplied one.
    /// Builder-style so the existing [`ToolDescriptor::new`] /
    /// [`ToolDescriptor::with_json_schema`] constructors stay
    /// short. The agent loop, the prompt builder, and the UI dialog
    /// never need this — they read the default profile and the
    /// defaults are right for the current set of callers. It's
    /// here so future audit work (e.g. tagging the
    /// `send_email` tool as `idempotent: false`,
    /// `requires_user_confirmation: true`) doesn't have to touch
    /// the constructor signatures.
    pub fn with_profile(mut self, profile: ToolProfile) -> Self {
        self.profile = profile;
        self
    }
}

// ---------------------------------------------------------------------------
// ToolConfigSpec — declarative configuration requirements.
// ---------------------------------------------------------------------------

/// Declarative configuration requirements for a tool. Drives
/// [`crate::tools::Tool::is_enabled`] in combination with the
/// group's enable flag and an optional prompt-content rule.
#[derive(Clone, Debug, Default)]
pub struct ToolConfigSpec {
    /// Group-level enable flag that must be on for the tool to run.
    /// Almost every tool sets this; the registry uses it to build
    /// the per-group enable view.
    pub group: Option<ToolGroupId>,
    /// Soft requirements on `AgentConfig` fields. All must be
    /// satisfied. Used for "this tool needs an API key", "this tool
    /// needs a JMAP client", etc.
    pub requires: Vec<ConfigPredicate>,
    /// Optional prompt-content rule. When set, the tool is only
    /// offered to the LLM when the rule matches the current prompt
    /// (used by the CSV tools — see TOOL-001).
    pub prompt_rule: Option<PromptPredicate>,
}

impl ToolConfigSpec {
    /// A spec that requires only the group's enable flag to be on.
    pub fn group_only(group: ToolGroupId) -> Self {
        Self {
            group: Some(group),
            requires: Vec::new(),
            prompt_rule: None,
        }
    }

    /// Decide whether a tool with this spec should be offered to the
    /// LLM, given the current `AgentConfig` and the user's prompt.
    ///
    /// Note: prompt-content gating rules themselves live in the
    /// specifications module (see
    /// [`crate::tools::specs`]). The tool system here
    /// just evaluates whatever [`PromptPredicate`] the spec carries.
    pub fn is_enabled_for(&self, config: &AgentConfig, prompt: &str) -> bool {
        if let Some(group) = &self.group
            && !group_enabled(config, group)
        {
            return false;
        }
        if self.requires.iter().any(|p| !p.eval(config)) {
            return false;
        }
        match &self.prompt_rule {
            Some(rule) => rule.matches(prompt),
            None => true,
        }
    }
}

/// A single boolean predicate over `AgentConfig`. See
/// [`ToolConfigSpec::requires`].
#[derive(Clone, Debug)]
pub enum ConfigPredicate {
    /// `config.searxng_url` is `Some(_)`.
    SearxngConfigured,
    /// `config.trello_client` is `Some(_)`.
    TrelloConfigured,
    /// `config.jmap_clients` is non-empty.
    JmapClientsPresent,
    /// `config.caldav_clients` is non-empty.
    CalDavClientsPresent,
    /// At least one of `caldav_clients` / `jmap_clients` is non-empty
    /// (whichever the contact family is configured to use). The
    /// `useDAVForContacts` feature flag decides which one is
    /// consulted.
    DavOrJmapClients,
    /// Always `false`. Used to keep a tool in the catalog but
    /// hide it from the LLM (e.g. `DeleteContactTool`, which is
    /// kept in the schema for future re-enable but disabled
    /// today).
    Never,
}

impl ConfigPredicate {
    /// Evaluate the predicate against the live config.
    pub fn eval(&self, config: &AgentConfig) -> bool {
        match self {
            Self::SearxngConfigured => config.searxng_url.is_some(),
            Self::TrelloConfigured => config.trello_client.is_some(),
            Self::JmapClientsPresent => !config.jmap_clients.is_empty(),
            Self::CalDavClientsPresent => !config.caldav_clients.is_empty(),
            Self::DavOrJmapClients => {
                let use_dav = config
                    .feature_flags
                    .get("useDAVForContacts")
                    .copied()
                    .unwrap_or(false);
                if use_dav {
                    !config.caldav_clients.is_empty()
                } else {
                    !config.jmap_clients.is_empty()
                }
            }
            Self::Never => false,
        }
    }
}

/// Prompt-content rule. Today the only variant is keyword
/// matching. The actual keyword lists live in the specifications
/// module (see [`crate::tools::specs`]); the tool
/// system here just provides the matching mechanism.
#[derive(Clone, Debug)]
pub enum PromptPredicate {
    /// The lowercased prompt contains any of the listed keywords.
    ContainsAny(&'static [&'static str]),
}

impl PromptPredicate {
    /// Evaluate against the current prompt.
    pub fn matches(&self, prompt: &str) -> bool {
        let p = prompt.to_lowercase();
        match self {
            Self::ContainsAny(keywords) => keywords.iter().any(|kw| p.contains(kw)),
        }
    }
}

// ---------------------------------------------------------------------------
// group_enabled — look up whether a `ToolGroupId` is currently on.
// ---------------------------------------------------------------------------

/// Whether the given group is currently enabled according to
/// `AgentConfig`. The MCP-server branch is consulted by the
/// descriptor's default `is_enabled`; the internal branch mirrors
/// the logic the registry uses to build the UI dialog's per-group
/// view.
pub fn group_enabled(config: &AgentConfig, group: &ToolGroupId) -> bool {
    match group {
        ToolGroupId::Internal(g) => internal_group_enabled(config, *g),
        ToolGroupId::Mcp(name) => config
            .mcp_servers
            .get(name)
            .is_some_and(|entry| entry.is_enabled()),
    }
}

fn internal_group_enabled(config: &AgentConfig, g: InternalToolGroup) -> bool {
    use InternalToolGroup::*;
    match g {
        Filesystem => config.tool_groups.filesystem,
        Web => config.tool_groups.web,
        Browser => config.tool_groups.browser,
        Email => config.tool_groups.email,
        Contacts => config.tool_groups.contacts,
        Calendar => config.tool_groups.calendar,
        CsvDb => config.tool_groups.csv_db,
        Weather => config.tool_groups.weather,
        Trello => config.tool_groups.trello,
    }
}

// ---------------------------------------------------------------------------
// tool_descriptor! — helper macro for the common `OnceLock<ToolDescriptor>`
// boilerplate. Used inside `impl Tool for XxxTool { ... }` blocks to
// emit the `descriptor()` method. Tools whose `is_enabled` needs a
// custom override (CSV prompt gate, etc.) write the `descriptor()`
// body by hand instead.
// ---------------------------------------------------------------------------

/// Emit a `descriptor()` method body that lazily initialises a
/// static `ToolDescriptor` on first call. Use inside an
/// `impl Tool for XxxTool { ... }` block:
///
/// ```ignore
/// impl Tool for PatchNoteTool {
///     crate::tool_descriptor! {
///         name: "patch_note",
///         desc: strings::PATCH_NOTE_DESCRIPTION,
///         input: crate::tools::dtos::PatchNoteInput,
///         safety: crate::tools::Safety::Mutating,
///         group: Filesystem,
///     }
///     fn execute(&self, ctx: &ToolContext, args: &str)
///         -> Result<serde_json::Value, String> { ... }
/// }
/// ```
#[macro_export]
macro_rules! tool_descriptor {
    (
        name: $name:expr,
        desc: $desc:expr,
        input: $input:ty,
        safety: $safety:expr,
        group: $group:ident $(,)?
        profile: $profile:expr $(,)?
    ) => {
        fn descriptor(&self) -> &$crate::tools::ToolDescriptor {
            static D: ::std::sync::OnceLock<$crate::tools::ToolDescriptor> =
                ::std::sync::OnceLock::new();
            D.get_or_init(|| {
                let group_id = $crate::tools::registry::groups::ToolGroupId::Internal(
                    $crate::tools::registry::groups::InternalToolGroup::$group,
                );
                $crate::tools::ToolDescriptor::new::<$input>(
                    $name,
                    $desc,
                    $safety,
                    $crate::tools::descriptor::ToolConfigSpec::group_only(group_id.clone()),
                    group_id,
                )
                .with_profile($profile)
            })
        }
    };
    (
        name: $name:expr,
        desc: $desc:expr,
        input: $input:ty,
        safety: $safety:expr,
        group: $group:ident $(,)?
    ) => {
        fn descriptor(&self) -> &$crate::tools::ToolDescriptor {
            static D: ::std::sync::OnceLock<$crate::tools::ToolDescriptor> =
                ::std::sync::OnceLock::new();
            D.get_or_init(|| {
                let group_id = $crate::tools::registry::groups::ToolGroupId::Internal(
                    $crate::tools::registry::groups::InternalToolGroup::$group,
                );
                $crate::tools::ToolDescriptor::new::<$input>(
                    $name,
                    $desc,
                    $safety,
                    $crate::tools::descriptor::ToolConfigSpec::group_only(group_id.clone()),
                    group_id,
                )
            })
        }
    };
}
