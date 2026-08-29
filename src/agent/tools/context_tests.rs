//! Unit tests for the tool context builder: default-extension
//! injection and the `.expect(...)` panic paths for missing
//! extensions.
//!
//! Sidecar of `context.rs`.

use super::*;
use crate::tools::observer::{DefaultFileObserver, OnFileChangedExt};
use crate::tools::policy::ToolCallPolicy;
use crate::tools::vfs::MockVirtualFileSystem;

fn default_ctx() -> ToolContext {
    ToolContext::default()
}

// ---- build() default injection ----

#[test]
fn test_build_injects_default_vfs() {
    let ctx = default_ctx();
    // VFS is default-injected, so resolve_virtual_path works (no panic).
    let _ = ctx.vfs();
}

#[test]
fn test_build_injects_default_policy() {
    let ctx = default_ctx();
    // Policy is default-injected; the default policy always allows writes.
    assert!(ctx.check_write_allowed(Path::new("/tmp/x.md")).is_ok());
}

#[test]
fn test_build_default_file_observer_when_none_injected() {
    let ctx = default_ctx();
    // No OnFileChangedExt injected → falls back to DefaultFileObserver.
    let obs = ctx.file_observer();
    // DefaultFileObserver.on_file_changed is a no-op; calling publish must not panic.
    ctx.publish_file_event(Path::new("/tmp/x.md"));
    let _ = obs;
}

// ---- custom extension injection ----

#[test]
fn test_injected_vfs_is_returned() {
    let ctx = ToolContextBuilder::new(
        Arc::new(AgentConfig::default()),
        Arc::new(DefaultFileObserver),
    )
    .with_extension(std::sync::Arc::new(
        crate::tools::vfs::VirtualFileSystemExt(Arc::new(MockVirtualFileSystem::new())),
    ))
    .build();
    let _ = ctx.vfs();
}

#[test]
fn test_injected_policy_is_called() {
    struct RejectingPolicy;
    impl ToolCallPolicy for RejectingPolicy {
        fn check_write_allowed(&self, _path: &Path) -> Result<(), String> {
            Err("blocked by test policy".to_string())
        }
    }
    let ctx = ToolContextBuilder::new(
        Arc::new(AgentConfig::default()),
        Arc::new(DefaultFileObserver),
    )
    .with_tool_call_policy(Arc::new(RejectingPolicy))
    .build();
    let err = ctx.check_write_allowed(Path::new("/tmp/x.md")).unwrap_err();
    assert!(err.contains("blocked by test policy"));
}

#[test]
fn test_injected_cache_and_uuid_gen_are_returned() {
    let cache = Arc::new(crate::tools::registry::cache::ToolCache::new());
    let uuid: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator> =
        std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator);
    let ctx = ToolContextBuilder::new(
        Arc::new(AgentConfig::default()),
        Arc::new(DefaultFileObserver),
    )
    .with_extension(std::sync::Arc::new(ToolCacheExt(cache.clone())))
    .with_extension(std::sync::Arc::new(UuidGeneratorExt(uuid.clone())))
    .build();
    assert!(std::sync::Arc::ptr_eq(&ctx.cache(), &cache));
    assert!(std::sync::Arc::ptr_eq(&ctx.uuid_gen(), &uuid));
}

// ---- missing-extension panic paths ----

#[test]
fn test_cache_without_injection_panics() {
    let ctx = default_ctx();
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.cache();
    }));
    assert!(
        res.is_err(),
        "cache() must panic when ToolCache is not injected"
    );
}

#[test]
fn test_uuid_gen_without_injection_panics() {
    let ctx = default_ctx();
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.uuid_gen();
    }));
    assert!(
        res.is_err(),
        "uuid_gen() must panic when UuidGenerator is not injected"
    );
}

// ---- with_tool_call_policy vs ToolCallPolicyExt ----

#[test]
fn test_no_policy_extension_returns_ok() {
    let ctx = default_ctx();
    assert!(ctx.check_write_allowed(Path::new("/x")).is_ok());
}

#[test]
fn test_file_observer_ext_used_when_injected() {
    let obs = Arc::new(DefaultFileObserver);
    let ctx = ToolContextBuilder::new(Arc::new(AgentConfig::default()), obs.clone())
        .with_extension(Arc::new(OnFileChangedExt(obs)))
        .build();
    ctx.publish_file_event(Path::new("/x"));
    let _ = ctx.file_observer();
}
