//! Tool-call dispatcher — receives tool-call JSON from the LLM, dispatches through the registry, and feeds results back.

use crate::agent::events::ToolSideEffect;
use crate::agent::tools::Safety;
use crate::agent::tools::execute_tool;
use crate::app::session::BrowserSession;
use crate::bus::core::Bus;
use crate::bus::events::file::FileEvent;
use crate::config::AppConfig;
use std::path::Path;
use std::sync::Arc;

/// Cheap, shallow-clone handle to a shared cache. The
/// `ToolExecutor` does not own a `ToolCache` directly; the
/// cache is a process-wide singleton exposed by
/// [`crate::agent::tools::registry::cache::cache`].
type SharedCache = Arc<crate::agent::tools::registry::cache::ToolCache>;

pub struct ToolExecutor {
    /// Global `AppConfig` shared with every tool call (used by
    /// the tool context for VFS resolution and integration
    /// clients). Cheap to clone per parallel worker (single
    /// `Arc` refcount bump).
    config: Arc<AppConfig>,
    file_event_bus: Bus<FileEvent>,
    /// When the `browser` Cargo feature is off the session is a
    /// stub that returns
    /// [`crate::app::session::SessionError::Disabled`].
    browser_session: Arc<BrowserSession>,
    pdf_backing: std::sync::Arc<crate::app::session::PdfBackingTracker>,
    cache: SharedCache,
    tool_manager: std::sync::Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>,
    uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
}

pub struct ToolExecutorBuilder {
    config: Arc<AppConfig>,
    file_event_bus: Bus<FileEvent>,
    cache: SharedCache,
    tool_manager: std::sync::Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>,
    browser_session: Option<Arc<BrowserSession>>,
    pdf_backing: Option<std::sync::Arc<crate::app::session::PdfBackingTracker>>,
    uuid_gen: Option<std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>>,
}

impl ToolExecutorBuilder {
    pub fn new(
        config: Arc<AppConfig>,
        file_event_bus: Bus<FileEvent>,
        cache: SharedCache,
        tool_manager: std::sync::Arc<
            arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>,
        >,
    ) -> Self {
        Self {
            config,
            file_event_bus,
            cache,
            tool_manager,
            browser_session: None,
            pdf_backing: None,
            uuid_gen: None,
        }
    }

    pub fn with_browser_session(mut self, browser_session: Arc<BrowserSession>) -> Self {
        self.browser_session = Some(browser_session);
        self
    }

    pub fn with_pdf_backing(
        mut self,
        pdf_backing: std::sync::Arc<crate::app::session::PdfBackingTracker>,
    ) -> Self {
        self.pdf_backing = Some(pdf_backing);
        self
    }

    pub fn with_uuid_gen(
        mut self,
        uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
    ) -> Self {
        self.uuid_gen = Some(uuid_gen);
        self
    }

    pub fn build(self) -> ToolExecutor {
        let default_browser = Arc::new(BrowserSession::new(&self.config));
        ToolExecutor {
            config: self.config,
            file_event_bus: self.file_event_bus,
            browser_session: self.browser_session.unwrap_or(default_browser),
            pdf_backing: self
                .pdf_backing
                .unwrap_or_else(|| Arc::new(crate::app::session::PdfBackingTracker::new())),
            cache: self.cache,
            tool_manager: self.tool_manager,
            uuid_gen: self
                .uuid_gen
                .unwrap_or_else(|| Arc::new(crate::utils::uuid::SystemUuidGenerator)),
        }
    }
}

impl ToolExecutor {
    #[allow(clippy::type_complexity)]
    pub fn execute_all(
        &self,
        tool_calls: &[serde_json::Value],
    ) -> (Vec<(String, String, String, String)>, Vec<ToolSideEffect>) {
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
        let side_effects = self.extract_side_effects(&results);
        (results, side_effects)
    }

    /// Look up a tool by name through the registry and ask it for its
    /// [`Safety`] classification. Falls back to [`Safety::Mutating`]
    /// (the conservative choice) when the name is unknown — that way
    /// an LLM-emitted call to a missing tool runs sequentially instead
    /// of in parallel, and the registry returns its normal "tool not
    /// found" error.
    fn classify(&self, name: &str) -> Safety {
        self.tool_manager.load().safety_of(name)
    }

    fn execute_parallel(
        &self,
        calls: &[serde_json::Value],
    ) -> Vec<(String, String, String, String)> {
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
        let pdf_backing = self.pdf_backing.clone();
        let mut completed = Vec::new();
        rt.block_on(async {
            let mut join_set = tokio::task::JoinSet::new();
            for tc in calls {
                let call_id = extract_str(tc, &["id"]).to_string();
                let func_name = extract_str(tc, &["function", "name"]).to_string();
                let func_args = extract_str(tc, &["function", "arguments"]).to_string();
                let cfg = self.config.clone();
                let bus = self.file_event_bus.clone();
                let browser = self.browser_session.clone();
                let pdf = pdf_backing.clone();
                let cache = self.cache.clone();
                let tm = self.tool_manager.clone();
                let uuid_gen = self.uuid_gen.clone();
                join_set.spawn_blocking(move || {
                    let ctx = crate::agent::tools::context::ToolContextBuilder::new(
                        cfg, bus, tm, cache, uuid_gen,
                    )
                    .with_browser_session(browser)
                    .with_pdf_backing(pdf)
                    .build();
                    let result = execute_tool(&ctx, &func_name, &func_args);
                    (call_id, func_name, func_args, result)
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

    fn execute_sequential(
        &self,
        calls: &[serde_json::Value],
    ) -> Vec<(String, String, String, String)> {
        let mut results = Vec::new();
        for tc in calls {
            let call_id = extract_str(tc, &["id"]).to_string();
            let func_name = extract_str(tc, &["function", "name"]).to_string();
            let func_args = extract_str(tc, &["function", "arguments"]).to_string();
            let browser = self.browser_session.clone();
            let pdf = self.pdf_backing.clone();
            let tm = self.tool_manager.clone();
            let ctx = crate::agent::tools::context::ToolContextBuilder::new(
                self.config.clone(),
                self.file_event_bus.clone(),
                tm,
                self.cache.clone(),
                self.uuid_gen.clone(),
            )
            .with_browser_session(browser)
            .with_pdf_backing(pdf)
            .build();
            let result = execute_tool(&ctx, &func_name, &func_args);
            results.push((call_id, func_name, func_args, result));
        }
        results
    }

    fn extract_side_effects(
        &self,
        results: &[(String, String, String, String)],
    ) -> Vec<ToolSideEffect> {
        let mut effects = Vec::new();
        for (_call_id, func_name, func_args_str, result) in results {
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
                    for lib in &self.config.content_libraries {
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

    #[test]
    fn test_classify() {
        let tm = crate::agent::tools::registry::ToolRegistry::new();
        // The registry doesn't exist anymore as a global, but the manager
        // exposes a single `safety_of(name)` lookup that returns
        // Safety::ReadOnly / Safety::Mutating.
        assert_eq!(tm.safety_of("read_note"), Safety::ReadOnly);
        assert_eq!(tm.safety_of("search_notes"), Safety::ReadOnly);
        assert_eq!(tm.safety_of("create_note"), Safety::Mutating);
        // Unknown tools fall back to Mutating (the conservative choice).
        assert_eq!(tm.safety_of("nonexistent"), Safety::Mutating);
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
        let config = AppConfig::default();
        let bus = Bus::new();
        let browser_session = std::sync::Arc::new(crate::app::session::BrowserSession::new(
            &crate::config::AppConfig::default(),
        ));
        let pdf_backing = std::sync::Arc::new(crate::app::session::PdfBackingTracker::new());
        let tm = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
            crate::agent::tools::registry::ToolRegistry::new(),
        ));
        let uuid_gen = std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator);
        let cache = std::sync::Arc::new(crate::agent::tools::registry::cache::ToolCache::new());
        let executor = ToolExecutorBuilder::new(std::sync::Arc::new(config), bus, cache, tm)
            .with_browser_session(browser_session)
            .with_pdf_backing(pdf_backing)
            .with_uuid_gen(uuid_gen)
            .build();
        assert!(executor.config.models.is_empty());
    }

    /// T018: Verify `extract_side_effects` returns `FileCreated` for
    /// successful `create_note` calls whose path starts with a known
    /// content-library name. Also verifies non-`create_note` tools and
    /// failed calls produce no side effects (quickstart scenario 4, SC-005).
    #[test]
    fn test_extract_side_effects_returns_file_created() {
        let mut config = AppConfig::default();
        config
            .content_libraries
            .push(crate::config::ContentLibrary {
                name: "notes".to_string(),
                root_folder: std::env::temp_dir()
                    .join("fastmd_test_extract_side_effects")
                    .to_string_lossy()
                    .to_string(),
                kind: "notes".to_string(),
                readonly: false,
                priority: 0,
            });
        let bus = Bus::new();
        let browser_session = std::sync::Arc::new(crate::app::session::BrowserSession::new(
            &crate::config::AppConfig::default(),
        ));
        let pdf_backing = std::sync::Arc::new(crate::app::session::PdfBackingTracker::new());
        let tm = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
            crate::agent::tools::registry::ToolRegistry::new(),
        ));
        let uuid_gen = std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator);
        let cache = std::sync::Arc::new(crate::agent::tools::registry::cache::ToolCache::new());
        let executor = ToolExecutorBuilder::new(std::sync::Arc::new(config), bus, cache, tm)
            .with_browser_session(browser_session)
            .with_pdf_backing(pdf_backing)
            .with_uuid_gen(uuid_gen)
            .build();

        // Synthetic results: a successful create_note and a failed one, plus a non-create_note tool.
        let results = vec![
            (
                "call_1".to_string(),
                "create_note".to_string(),
                r#"{"path":"notes/test.md","content":"hello"}"#.to_string(),
                r#"{"status":"success","data":{"size_bytes":10}}"#.to_string(),
            ),
            (
                "call_2".to_string(),
                "create_note".to_string(),
                r#"{"path":"notes/other.md","content":"content"}"#.to_string(),
                r#"{"status":"error","message":"file exists"}"#.to_string(),
            ),
            (
                "call_3".to_string(),
                "search_notes".to_string(),
                r#"{"query":"test"}"#.to_string(),
                r#"{"status":"success","data":{"matches":0}}"#.to_string(),
            ),
        ];

        let effects = executor.extract_side_effects(&results);
        // Only the successful create_note should produce a side effect
        assert_eq!(
            effects.len(),
            1,
            "only successful create_note produces side effect"
        );
        match &effects[0] {
            ToolSideEffect::FileCreated { path, .. } => {
                assert!(
                    path.to_string_lossy().contains("test.md"),
                    "path should contain test.md; got: {:?}",
                    path
                );
            }
        }
    }

    /// T018: Verify `execute_all` returns `Vec<ToolSideEffect>` and does
    /// NOT send `FsEvent` on any channel (the old `notify_file_creations`
    /// that sent on `tx_gui` was deleted in T012). We verify by checking
    /// that the `Bus<FileEvent>` has no events after `execute_all` (SC-005).
    #[test]
    fn test_execute_all_no_fs_events_sent() {
        let config = AppConfig::default();
        let bus = Bus::new();
        let bus_reader = bus.subscribe();
        let browser_session = std::sync::Arc::new(crate::app::session::BrowserSession::new(
            &crate::config::AppConfig::default(),
        ));
        let pdf_backing = std::sync::Arc::new(crate::app::session::PdfBackingTracker::new());
        let tm = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
            crate::agent::tools::registry::ToolRegistry::new(),
        ));
        let uuid_gen = std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator);
        let cache = std::sync::Arc::new(crate::agent::tools::registry::cache::ToolCache::new());
        let executor = ToolExecutorBuilder::new(std::sync::Arc::new(config), bus, cache, tm)
            .with_browser_session(browser_session)
            .with_pdf_backing(pdf_backing)
            .with_uuid_gen(uuid_gen)
            .build();

        // Call execute_all with an empty tool_calls list — should return empty results and side effects
        let (results, side_effects) = executor.execute_all(&[]);
        assert!(results.is_empty());
        assert!(side_effects.is_empty());

        // Verify no FsEvent was sent on the file event bus
        match bus_reader.try_recv() {
            Ok(ev) => panic!("execute_all must not send FsEvent; got: {:?}", ev),
            Err(std::sync::mpsc::TryRecvError::Empty) => {} // expected
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {} // also fine
        }
    }
}
