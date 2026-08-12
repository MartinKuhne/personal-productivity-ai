import os
import re

missing_docs = []
for root, dirs, files in os.walk('src/desktop/src'):
    for name in files:
        if not name.endswith('.rs'): continue
        if 'tests' in name or 'test_' in name: continue
        path = os.path.join(root, name)
        
        with open(path, 'r', encoding='utf-8') as f:
            lines = f.readlines()
            
        for i, line in enumerate(lines):
            if re.match(r'^\s*pub const [A-Z_0-9]+:', line) or re.match(r'^\s*const [A-Z_0-9]+:', line):
                # check if previous line is a comment
                has_doc = False
                for j in range(i-1, -1, -1):
                    prev = lines[j].strip()
                    if prev.startswith('//'):
                        has_doc = True
                        break
                    elif prev == '' or prev.startswith('#['):
                        continue
                    else:
                        break
                
                if not has_doc:
                    missing_docs.append(f"{path}:{i+1} {line.strip()}")

for m in missing_docs:
    print(m)
