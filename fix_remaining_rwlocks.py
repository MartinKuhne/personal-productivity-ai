import os
import re

path = 'src/desktop/src/agent/context.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = re.sub(
    r'tool_manager:\s*Option<Arc<std::sync::RwLock<crate::agent::tools::registry::ToolRegistry>>>,',
    r'tool_manager: Option<Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>>,',
    content
)

content = re.sub(
    r'pub fn with_tool_manager\(mut self, tool_manager: Arc<std::sync::RwLock<crate::agent::tools::registry::ToolRegistry>>\) -> Self {',
    r'pub fn with_tool_manager(mut self, tool_manager: Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>) -> Self {',
    content
)

content = re.sub(
    r'Arc::new\(arc_swap::ArcSwap::from_pointee\(crate::agent::tools::registry::ToolRegistry::new\(\)\)\)',
    r'Arc::new(arc_swap::ArcSwap::from_pointee(crate::agent::tools::registry::ToolRegistry::new()))',
    content
)

with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)

path = 'src/desktop/src/agent/session.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = re.sub(
    r'tool_manager:\s*Arc<std::sync::RwLock<crate::agent::tools::registry::ToolRegistry>>,',
    r'tool_manager: Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>,',
    content
)

with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)

path = 'src/desktop/src/ui/tools_dialog.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = re.sub(
    r'\.tool_manager\s*\n\s*\.write\(\)\s*\n\s*\.unwrap\(\)\s*\n\s*\.mcp_manager\(\)',
    r'.tool_manager\n                        .load()\n                        .mcp_manager()',
    content
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)
