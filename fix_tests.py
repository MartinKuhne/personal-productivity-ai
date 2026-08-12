import os
import re

path = 'src/desktop/src/agent/tool_executor.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()
content = content.replace(
    'let tm = Arc::new(RwLock::new(ToolRegistry::new()));',
    'let tm = Arc::new(arc_swap::ArcSwap::from_pointee(ToolRegistry::new()));'
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)


path = 'src/desktop/src/agent/tools/registry/tests.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace(
    'Arc::new(std::sync::RwLock::new(ToolRegistry::new()))',
    'Arc::new(arc_swap::ArcSwap::from_pointee(ToolRegistry::new()))'
)
# Fix the ToolContextBuilder error at line 18
# The test was calling ToolContextBuilder::new(Arc::new(config), Bus::new(), tm, cache, uuid).with_browser_session().with_pdf_backing().build()
# But wait, it gave error about with_browser_session not found on Arc<SystemUuidGenerator>. 
# Because the builder takes uuid_gen, and it was passing it wrong or missing something. Wait, in 	ests.rs line 18, it is missing uuid_gen! Let's check ToolContextBuilder::new arguments: config, file_event_bus, tool_manager, cache, uuid_gen.
content = content.replace(
    'crate::agent::tools::context::ToolContextBuilder::new(Arc::new(config.clone()), Bus::new(), Arc::new(arc_swap::ArcSwap::from_pointee(ToolRegistry::new())), Arc::new(crate::agent::tools::registry::cache::ToolCache::new()), Arc::new(crate::utils::uuid::SystemUuidGenerator))',
    'crate::agent::tools::context::ToolContextBuilder::new(Arc::new(config.clone()), Bus::new(), Arc::new(arc_swap::ArcSwap::from_pointee(ToolRegistry::new())), Arc::new(crate::agent::tools::registry::cache::ToolCache::new()), Arc::new(crate::utils::uuid::SystemUuidGenerator))'
)
# Let's fix the ToolContextBuilder usage with regex just in case
content = re.sub(
    r'crate::agent::tools::context::ToolContextBuilder::new\([^,]+,\s*[^,]+,\s*Arc::new\(arc_swap::ArcSwap::from_pointee\(ToolRegistry::new\(\)\)\),\s*Arc::new\(crate::agent::tools::registry::cache::ToolCache::new\(\)\)\)',
    r'crate::agent::tools::context::ToolContextBuilder::new(Arc::new(config.clone()), Bus::new(), Arc::new(arc_swap::ArcSwap::from_pointee(ToolRegistry::new())), Arc::new(crate::agent::tools::registry::cache::ToolCache::new()), Arc::new(crate::utils::uuid::SystemUuidGenerator))',
    content
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)


path = 'src/desktop/src/agent/tools/tool_call_dispatch_proptests.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace(
    'ToolContext::new(',
    '''crate::agent::tools::context::ToolContextBuilder::new(
            std::sync::Arc::new(AppConfig::default()),
            crate::bus::core::Bus::new(),
            std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(crate::agent::tools::registry::ToolRegistry::new())),
            std::sync::Arc::new(crate::agent::tools::registry::cache::ToolCache::new()),
            std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator)
        )
        .with_browser_session(Arc::new(crate::app::session::BrowserSession::new(&AppConfig::default())))
        .with_pdf_backing(Arc::new(crate::app::session::PdfBackingTracker::new()))
        .build()'''
)
# Actually in proptests it probably passed arguments to ToolContext::new(...), so we should replace the whole block!
