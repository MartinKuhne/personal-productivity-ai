import os

path = 'src/desktop/src/agent/tools/registry/mod.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace(
    'pub struct ToolRegistry {',
    '#[derive(Clone)]\npub struct ToolRegistry {'
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)


path = 'src/desktop/src/ui/app/init.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace(
    'std::sync::RwLock::new(',
    'arc_swap::ArcSwap::from_pointee('
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
                        .mcp_manager()
                        .mcp_forget(&server);''',
    '''                    app.orchestrator
                        .tool_manager
                        .load()
                        .mcp_manager()
                        .mcp_forget(&server);'''
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)
