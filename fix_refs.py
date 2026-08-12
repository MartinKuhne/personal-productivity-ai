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
    ('crate::agent::tools::manager::', 'crate::agent::tools::registry::'),
    ('crate::agent::tools::manager', 'crate::agent::tools::registry'),
    ('crate::integrations::mcp::McpClientManager', 'crate::integrations::mcp::McpClients'),
    ('McpClientManager', 'McpClients'),
])

print("Global replace complete.")
