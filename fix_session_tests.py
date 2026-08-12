import os
import re

path = 'src/desktop/src/agent/session_tests.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()
content = content.replace(
    'std::sync::RwLock::new(',
    'arc_swap::ArcSwap::from_pointee('
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)
