import os
import re

path = 'src/desktop/src/agent/context.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace(
    'pub tool_manager: std::sync::Arc<std::sync::RwLock<crate::agent::tools::registry::ToolRegistry>>,',
    'pub tool_manager: std::sync::Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>,'
)
content = content.replace(
    'tool_manager: Option<std::sync::Arc<std::sync::RwLock<crate::agent::tools::registry::ToolRegistry>>>,',
    'tool_manager: Option<std::sync::Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>>,'
)
content = content.replace(
    'pub fn with_tool_manager(mut self, tool_manager: std::sync::Arc<std::sync::RwLock<crate::agent::tools::registry::ToolRegistry>>) -> Self {',
    'pub fn with_tool_manager(mut self, tool_manager: std::sync::Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>) -> Self {'
)

with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)


path = 'src/desktop/src/agent/session.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()
content = content.replace(
    'tool_manager: Arc<std::sync::RwLock<crate::agent::tools::registry::ToolRegistry>>,',
    'tool_manager: Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>,'
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)


path = 'src/desktop/src/ui/app/init.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()
content = content.replace(
    '''            tool_manager: Arc::new(arc_swap::ArcSwap::from_pointee(
                crate::agent::tools::registry::ToolRegistry::new(),
            )),''',
    '''            tool_manager: Arc::new(arc_swap::ArcSwap::from_pointee(
                crate::agent::tools::registry::ToolRegistry::new(),
            )),'''
)
# Actually, let's just use regex to replace Arc::new(RwLock::new) globally in init.rs if it's there
content = re.sub(
    r'Arc::new\(\s*std::sync::RwLock::new\(\s*crate::agent::tools::registry::ToolRegistry::new\(\),\s*\)\)',
    r'Arc::new(arc_swap::ArcSwap::from_pointee(crate::agent::tools::registry::ToolRegistry::new()))',
    content
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)


path = 'src/desktop/src/ui/tools_dialog.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()
content = content.replace(
    '''                    app.orchestrator
                        .tool_manager
                        .write()
                        .unwrap()
                        .clear_error(&id);''',
    '''                    app.orchestrator.tool_manager.rcu(|mgr| {
                        let mut new_mgr = (**mgr).clone();
                        new_mgr.clear_error(&id);
                        new_mgr
                    });'''
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)
