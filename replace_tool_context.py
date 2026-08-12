import os
import re

def replace_in_dir(directory):
    for root, dirs, files in os.walk(directory):
        for name in files:
            if not name.endswith('.rs'):
                continue
            path = os.path.join(root, name)
            with open(path, 'r', encoding='utf-8') as file:
                content = file.read()

            nc = re.sub(
                r'ToolContext::new\(\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,\)]+)\s*\)',
                r'crate::agent::tools::context::ToolContextBuilder::new(\1, \2, \6, \5, \7)\n        .with_browser_session(\3)\n        .with_pdf_backing(\4)\n        .build()',
                content
            )
            
            if nc != content:
                with open(path, 'w', encoding='utf-8') as file:
                    file.write(nc)

replace_in_dir('src/desktop/src')
print("Replaced ToolContext::new calls")
