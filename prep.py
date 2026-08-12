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

replacements = [
    (r'\.read\(\)\.unwrap\(\)', r'.load()')
]

for root, dirs, files in os.walk('src/desktop/src'):
    for file in files:
        if file.endswith('.rs'):
            with open(os.path.join(root, file), 'r', encoding='utf-8') as f:
                content = f.read()
            if 'tool_manager' in content or 'tm.read().unwrap()' in content or 'mgr' in content:
                # Be careful, we only want to replace .read().unwrap() for tool_manager.
                # Actually, there might be other RwLocks. Let's just find them and review.
                pass
