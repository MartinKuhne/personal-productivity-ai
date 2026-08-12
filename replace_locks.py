import os
import re

def replace_in_file(path, replacements):
    with open(path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    orig = content
    for p, r in replacements:
        content = re.sub(p, r, content)
        
    if orig != content:
        with open(path, 'w', encoding='utf-8', newline='\n') as f:
            f.write(content)

# We want to be very targeted for agent_impl.rs, orchestrator.rs, tools_dialog.rs, mod.rs
