import os
import re

path = 'src/desktop/src/ui/tools_dialog.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

# Replace .read().unwrap() with .load()
content = content.replace('.read().unwrap()', '.load()')
content = content.replace('.read()\n                    .unwrap()', '.load()')
content = content.replace('.read()\n        .unwrap()', '.load()')
# .write().unwrap() that don't need mutation return value can be rcu
content = content.replace(
    '.write()\n        .unwrap()\n        .groups_snapshot(app.config())',
    '.load()\n        .groups_snapshot(app.config())'
)

# For set_group_enabled
content = re.sub(
    r'\.tool_manager\s*\.write\(\)\s*\.unwrap\(\)\s*\.set_group_enabled\(&mut new_config, &id, enabled\);',
    r'.tool_manager.rcu(|mgr| {\n                    let mut new_mgr = (**mgr).clone();\n                    new_mgr.set_group_enabled(&mut new_config, &id, enabled);\n                    new_mgr\n                });',
    content
)

# For clear_error
content = re.sub(
    r'\.tool_manager\s*\.write\(\)\s*\.unwrap\(\)\s*\.clear_error\(&id\);',
    r'.tool_manager.rcu(|mgr| {\n                            let mut new_mgr = (**mgr).clone();\n                            new_mgr.clear_error(&id);\n                            new_mgr\n                        });',
    content
)

# For mcp_manager().mcp_forget
content = re.sub(
    r'\.tool_manager\s*\.write\(\)\s*\.unwrap\(\)\s*\.mcp_manager\(\)\s*\.mcp_forget\(&server\);',
    r'.tool_manager.load().mcp_manager().mcp_forget(&server);',
    content
)

with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)
