import os
import re

files = [
    'src/desktop/src/agent/context.rs',
    'src/desktop/src/agent/tool_executor.rs',
    'src/desktop/src/agent/tools/context.rs'
]

for path in files:
    with open(path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    content = content.replace('BrowserSession::disabled()', 'BrowserSession::new(&crate::config::AppConfig::default())')
    
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        f.write(content)
