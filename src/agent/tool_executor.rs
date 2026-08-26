//! Tool-call dispatcher — receives tool-call JSON from the LLM,
//! dispatches through the [`AgentToolContext`]'s registry, and
//! feeds results back.

use crate::AgentToolContext;
use crate::config::AgentConfig;
use crate::events::ToolSideEffect;
use crate::tools::Safety;
use crate::tools::execute_tool;
use std::path::Path;
use std::sync::Arc;

/// Cheap, shallow-clone handle to a shared cache. The
/// `ToolExecutor` does not own a `ToolCache` directly; the
/// cache is a process-wide singleton exposed by
/// [`crate::tools::registry::cache::cache`].
type SharedCache = Arc<crate::tools::registry::cache::ToolCache>;

/// Result record for an individual executed tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallRecord {
    /// Unique identifier for the tool call.
    pub call_id: String,
    /// Name of the tool executed.
    pub name: String,
    /// Raw JSON arguments passed to the tool.
    pub arguments: String,
    /// Output result string from the tool execution.
    pub result: String,
}

pub struct ToolExecutor {
    /// Global `AgentConfig` shared with every tool call (used by
    /// the tool context for VFS resolution and integration
    /// clients). Cheap to clone per parallel worker (single
    /// `Arc` refcount bump).
    config: Arc<AgentConfig>,
    file_observer: std::sync::Arc<dyn crate::tools::observer::OnFileChanged>,
    /// When the `browser` Cargo feature is off the session is a
    /// stub that returns `SessionError::Disabled`.
    policy: std::sync::Arc<dyn crate::tools::policy::ToolCallPolicy>,
    cache: SharedCache,
    /// Catalog-level bundle. The executor snapshots this per
    /// parallel worker (`ArcSwap::load`) and per sequential
    /// call, so every dispatch sees a consistent registry view.
    tool_context: std::sync::Arc<arc_swap::ArcSwap<AgentToolContext>>,
    uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
    extensions: crate::tools::extensions::Extensions,
}

impl std::fmt::Debug for ToolExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolExecutor")
            .field("config", &self.config)
            .field("cache", &self.cache)
            .field("uuid_gen", &self.uuid_gen)
            .finish_non_exhaustive()
    }
}

pub struct ToolExecutorBuilder {
    config: Arc<AgentConfig>,
    file_observer: std::sync::Arc<dyn crate::tools::observer::OnFileChanged>,
    cache: SharedCache,
    tool_context: std::sync::Arc<arc_swap::ArcSwap<AgentToolContext>>,
    policy: Option<std::sync::Arc<dyn crate::tools::policy::ToolCallPolicy>>,
    uuid_gen: Option<std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>>,
    extensions: crate::tools::extensions::Extensions,
}

impl std::fmt::Debug for ToolExecutorBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolExecutorBuilder")
            .field("config", &self.config)
            .field("cache", &self.cache)
            .field("uuid_gen", &self.uuid_gen)
            .finish_non_exhaustive()
    }
}

impl ToolExecutorBuilder {
    pub fn new(
        config: Arc<AgentConfig>,
        file_observer: std::sync::Arc<dyn crate::tools::observer::OnFileChanged>,
        cache: SharedCache,
        tool_context: std::sync::Arc<arc_swap::ArcSwap<AgentToolContext>>,
    ) -> Self {
        Self {
            config,
            file_observer,
            cache,
            tool_context,
            policy: None,
            uuid_gen: None,
            extensions: crate::tools::extensions::Extensions::default(),
        }
    }

    /// Set the policy deciding whether a tool call is allowed.
    pub fn with_tool_call_policy(
        mut self,
        policy: std::sync::Arc<dyn crate::tools::policy::ToolCallPolicy>,
    ) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Set the generator used to mint run UUIDs.
    pub fn with_uuid_gen(
        mut self,
        uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
    ) -> Self {
        self.uuid_gen = Some(uuid_gen);
        self
    }

    pub fn with_extensions(mut self, extensions: crate::tools::extensions::Extensions) -> Self {
        self.extensions.extend(&extensions);
        self
    }

    pub fn build(self) -> ToolExecutor {
        ToolExecutor {
            config: self.config,
            file_observer: self.file_observer,
            policy: self
                .policy
                .unwrap_or_else(|| Arc::new(crate::tools::policy::DefaultToolCallPolicy)),
            cache: self.cache,
            tool_context: self.tool_context,
            uuid_gen: self
                .uuid_gen
                .unwrap_or_else(|| Arc::new(crate::utils::uuid::SystemUuidGenerator)),
            extensions: self.extensions,
        }
    }
}

impl ToolExecutor {
    pub fn execute_all(
        &self,
        tool_calls: &[serde_json::Value],
    ) -> (Vec<ToolCallRecord>, Vec<ToolSideEffect>) {
        let mut safe_calls = Vec::new();
        let mut unsafe_calls = Vec::new();
        for tc in tool_calls {
            let func_name = tc
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            if self.classify(func_name) == Safety::ReadOnly {
                safe_calls.push(tc.clone());
            } else {
                unsafe_calls.push(tc.clone());
            }
        }
        let mut results = self.execute_parallel(&safe_calls);
        results.extend(self.execute_sequential(&unsafe_calls));
        self.record_tool_errors(&results);
        let side_effects = self.extract_side_effects(&results);
        (results, side_effects)
    }

    /// Per-TOOL-021: record the most recent execution-kind error on
    /// each tool's group, or clear it on success. Called once per
    /// turn after all tool calls have completed. Per-group error
    /// state is what the UI dialog's "needs attention" badge reads.
    fn record_tool_errors(&self, results: &[ToolCallRecord]) {
        use crate::tools::registry::errors::{ToolErrorKind, ToolGroupError};
        for record in results {
            let func_name = &record.name;
            let result = &record.result;
            let group = self.tool_context.load().registry.tool_group(func_name);
            let Some(group) = group else {
                continue;
            };
            let ok = serde_json::from_str::<serde_json::Value>(result)
                .ok()
                .and_then(|v| v.get("status").and_then(|s| s.as_str()).map(str::to_string))
                .as_deref()
                == Some("success");
            self.tool_context.rcu(|ctx| {
                let mut new_ctx = (**ctx).clone();
                if ok {
                    new_ctx.registry.clear_error(&group);
                } else {
                    let msg = serde_json::from_str::<serde_json::Value>(result)
                        .ok()
                        .and_then(|v| {
                            v.get("message")
                                .and_then(|m| m.as_str())
                                .map(str::to_string)
                        })
                        .unwrap_or_else(|| "Tool execution failed.".to_string());
                    new_ctx
                        .registry
                        .record_error(&group, ToolGroupError::now(ToolErrorKind::Execution, msg));
                }
                new_ctx
            });
        }
    }

    /// Look up a tool by name through the registry and ask it for its
    /// [`Safety`] classification. Falls back to [`Safety::Mutating`]
    /// (the conservative choice) when the name is unknown — that way
    /// an LLM-emitted call to a missing tool runs sequentially instead
    /// of in parallel, and the registry returns its normal "tool not
    /// found" error.
    pub fn classify(&self, name: &str) -> Safety {
        self.tool_context.load().registry.safety_of(name)
    }

    fn execute_parallel(&self, calls: &[serde_json::Value]) -> Vec<ToolCallRecord> {
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!(name = "agent.runtime.build_failed", error = %e);
                return Vec::new();
            }
        };
        let policy = self.policy.clone();
        let extensions = self.extensions.clone();
        let mut completed = Vec::new();
        rt.block_on(async {
            let mut join_set = tokio::task::JoinSet::new();
            for tc in calls {
                let call_id = extract_str(tc, &["id"]).to_string();
                let func_name = extract_str(tc, &["function", "name"]).to_string();
                let func_args = extract_str(tc, &["function", "arguments"]).to_string();
                let cfg = self.config.clone();
                let bus = self.file_observer.clone();

                let pdf = policy.clone();
                let cache = self.cache.clone();
                let tc_arc = self.tool_context.clone();
                let uuid_gen = self.uuid_gen.clone();
                let extensions = extensions.clone();
                join_set.spawn_blocking(move || {
                    let snapshot = tc_arc.load();
                    let dispatcher = &snapshot.registry;
                    let ctx = crate::tools::context::ToolContextBuilder::new(cfg, bus)
                        .with_extension(std::sync::Arc::new(crate::tools::context::ToolCacheExt(
                            cache,
                        )))
                        .with_extension(std::sync::Arc::new(
                            crate::tools::context::UuidGeneratorExt(uuid_gen),
                        ))
                        .with_tool_call_policy(pdf)
                        .with_extensions(extensions.clone())
                        .build();
                    let result = execute_tool(dispatcher, &ctx, &func_name, &func_args);
                    ToolCallRecord {
                        call_id,
                        name: func_name,
                        arguments: func_args,
                        result,
                    }
                });
            }
            while let Some(res) = join_set.join_next().await {
                if let Ok(data) = res {
                    completed.push(data);
                }
            }
        });
        completed
    }

    fn execute_sequential(&self, calls: &[serde_json::Value]) -> Vec<ToolCallRecord> {
        let mut results = Vec::new();
        let extensions = self.extensions.clone();
        for tc in calls {
            let call_id = extract_str(tc, &["id"]).to_string();
            let func_name = extract_str(tc, &["function", "name"]).to_string();
            let func_args = extract_str(tc, &["function", "arguments"]).to_string();

            let pdf = self.policy.clone();
            let snapshot = self.tool_context.load();
            let dispatcher = &snapshot.registry;
            let ctx = crate::tools::context::ToolContextBuilder::new(
                self.config.clone(),
                self.file_observer.clone(),
            )
            .with_extension(std::sync::Arc::new(crate::tools::context::ToolCacheExt(
                self.cache.clone(),
            )))
            .with_extension(std::sync::Arc::new(
                crate::tools::context::UuidGeneratorExt(self.uuid_gen.clone()),
            ))
            .with_tool_call_policy(pdf)
            .with_extensions(extensions.clone())
            .build();
            let result = execute_tool(dispatcher, &ctx, &func_name, &func_args);
            results.push(ToolCallRecord {
                call_id,
                name: func_name,
                arguments: func_args,
                result,
            });
        }
        results
    }

    fn extract_side_effects(&self, results: &[ToolCallRecord]) -> Vec<ToolSideEffect> {
        let mut effects = Vec::new();
        for record in results {
            let func_name = &record.name;
            let func_args_str = &record.arguments;
            let result = &record.result;
            if func_name != "create_note" {
                continue;
            }
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(result) {
                if parsed.get("status").and_then(|s| s.as_str()) != Some("success") {
                    continue;
                }
                let path_owned: String =
                    match serde_json::from_str::<serde_json::Value>(func_args_str) {
                        Ok(v) => match v
                            .get("path")
                            .and_then(|p| p.as_str())
                            .map(|s| s.to_string())
                        {
                            Some(s) => s,
                            None => continue,
                        },
                        Err(_) => continue,
                    };
                let vpath = Path::new(&path_owned);
                let mut comps = vpath.components().peekable();
                while let Some(c) = comps.peek() {
                    match c {
                        std::path::Component::RootDir | std::path::Component::CurDir => {
                            comps.next();
                        }
                        _ => break,
                    }
                }
                if let Some(std::path::Component::Normal(first)) = comps.next() {
                    let lib_name = first.to_string_lossy();
                    for lib in self.config.content_libraries() {
                        if lib.name == lib_name {
                            let rest: std::path::PathBuf = comps.collect();
                            let abs_path = Path::new(&lib.root_folder).join(rest);
                            let tags = crate::utils::tags::extract_tags_from_file(&abs_path);
                            effects.push(ToolSideEffect::FileCreated {
                                path: abs_path,
                                tags,
                            });
                            break;
                        }
                    }
                }
            }
        }
        effects
    }
}

fn extract_str<'a>(val: &'a serde_json::Value, path: &[&str]) -> &'a str {
    let mut current = val;
    for key in path {
        match current.get(key) {
            Some(v) => current = v,
            None => return "",
        }
    }
    current.as_str().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AgentConfig;
    use std::sync::Arc;

    #[test]
    fn test_classify() {
        let tm = Arc::new(arc_swap::ArcSwap::from_pointee(AgentToolContext::new(
            crate::tools::registry::ToolRegistry::new(),
        )));
        assert_eq!(tm.load().registry.safety_of("read_note"), Safety::ReadOnly);
        assert_eq!(
            tm.load().registry.safety_of("search_notes"),
            Safety::ReadOnly
        );
        assert_eq!(
            tm.load().registry.safety_of("create_note"),
            Safety::Mutating
        );
        assert_eq!(
            tm.load().registry.safety_of("nonexistent"),
            Safety::Mutating
        );
    }

    #[test]
    fn test_extract_str_nested() {
        let val = serde_json::json!({
            "function": { "name": "test", "arguments": "{}" },
            "id": "call_1"
        });
        assert_eq!(extract_str(&val, &["id"]), "call_1");
        assert_eq!(extract_str(&val, &["function", "name"]), "test");
        assert_eq!(extract_str(&val, &["missing"]), "");
    }

    #[test]
    fn test_tool_executor_new() {
        let config = AgentConfig::default();
        let bus = std::sync::Arc::new(crate::tools::observer::DefaultFileObserver);
        let policy = std::sync::Arc::new(crate::tools::policy::DefaultToolCallPolicy);
        let tm = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(AgentToolContext::new(
            crate::tools::registry::ToolRegistry::new(),
        )));
        let uuid_gen = std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator);
        let cache = std::sync::Arc::new(crate::tools::registry::cache::ToolCache::new());
        let executor = ToolExecutorBuilder::new(std::sync::Arc::new(config), bus, cache, tm)
            .with_tool_call_policy(policy)
            .with_uuid_gen(uuid_gen)
            .build();
        assert!(executor.config.models().is_empty());
    }
}
