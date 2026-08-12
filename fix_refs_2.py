import os
import re

def replace_in_dir(directory, replacements):
    for root, dirs, files in os.walk(directory):
        for name in files:
            if not name.endswith('.rs'):
                continue
            path = os.path.join(root, name)
            with open(path, 'r', encoding='utf-8') as file:
                content = file.read()
            
            nc = content
            for old, new in replacements:
                nc = nc.replace(old, new)
                
            if nc != content:
                with open(path, 'w', encoding='utf-8', newline='\n') as file:
                    file.write(nc)

replace_in_dir('src/desktop', [
    ('super::ToolManager', 'super::ToolRegistry'),
    ('crate::agent::AgentSessionManager', 'crate::agent::AgentSession'),
    ('use manager::{ToolGroupId, ToolGroupKind};', 'use registry::{ToolGroupId, ToolGroupKind};'),
    ('manager::ToolGroupKind::', 'registry::ToolGroupKind::'),
    ('group: &manager::ToolGroupState', 'group: &registry::ToolGroupState'),
    ('pub use manager::execute_tool;', 'pub use registry::execute_tool;'),
    ('registry::ToolManager', 'registry::ToolRegistry'),
    ('registry::ToolManager::new()', 'registry::ToolRegistry::new()'),
    ('tool_manager: Arc<std::sync::RwLock<crate::agent::tools::registry::ToolManager>>', 'tool_manager: Arc<std::sync::RwLock<crate::agent::tools::registry::ToolRegistry>>'),
])

print("Global replace complete.")
