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

# Replace types
type_replacements = [
    (r'std::sync::Arc<std::sync::RwLock<crate::agent::tools::registry::ToolRegistry>>', 
     r'std::sync::Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>'),
    (r'Arc<std::sync::RwLock<ToolRegistry>>',
     r'Arc<arc_swap::ArcSwap<ToolRegistry>>'),
    (r'Arc<RwLock<ToolRegistry>>',
     r'Arc<arc_swap::ArcSwap<ToolRegistry>>'),
    (r'std::sync::Arc::new\(std::sync::RwLock::new\(crate::agent::tools::registry::ToolRegistry::new\(\)\)\)',
     r'std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(crate::agent::tools::registry::ToolRegistry::new()))'),
    (r'Arc::new\(std::sync::RwLock::new\(crate::agent::tools::registry::ToolRegistry::new\(\)\)\)',
     r'Arc::new(arc_swap::ArcSwap::from_pointee(crate::agent::tools::registry::ToolRegistry::new()))'),
    (r'Arc::new\(RwLock::new\(ToolRegistry::new\(\)\)\)',
     r'Arc::new(arc_swap::ArcSwap::from_pointee(ToolRegistry::new()))'),
]

for root, dirs, files in os.walk('src/desktop/src'):
    for file in files:
        if file.endswith('.rs'):
            replace_in_file(os.path.join(root, file), type_replacements)

