import os

path = r'src\desktop\src\ui\modals_tests.rs'
with open(path, 'r', encoding='utf-8') as file:
    content = file.read()

nc = content.replace('tags: &mut app.orchestrator.tags,', 'app_tags: &mut app.orchestrator.tags,')

if nc != content:
    with open(path, 'w', encoding='utf-8', newline='\n') as file:
        file.write(nc)
