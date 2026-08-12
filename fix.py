import os
import re

def fix():
    for root, dirs, files in os.walk('src/desktop/src'):
        for f in files:
            if not f.endswith('.rs'):
                continue
            path = os.path.join(root, f)
            
            try:
                with open(path, 'r', encoding='utf-8') as file:
                    content = file.read()
            except Exception:
                try:
                    with open(path, 'r', encoding='utf-16') as file:
                        content = file.read()
                except Exception:
                    continue
            
            nc = content
            nc = nc.replace('DialogManager', 'Dialogs')
            nc = nc.replace('dialog_manager', 'dialogs')
            
            nc = nc.replace('SelectionManager', 'FileSelection')
            nc = nc.replace('selection_manager', 'selection')
            
            nc = nc.replace('TabManager', 'Tabs')
            nc = nc.replace('tab_manager', 'tabs')
            
            nc = nc.replace('TagManager', 'Tags')
            nc = nc.replace('tag_manager', 'tags')
            
            nc = nc.replace('BackgroundProcessManager', 'BackgroundLogs')
            nc = nc.replace('SharedProcessManager', 'SharedBackgroundLogs')
            nc = nc.replace('background::manager', 'background::logs')
            nc = nc.replace('manager_tests.rs', 'logs_tests.rs')
            nc = nc.replace('mod manager;', 'mod logs;')
            
            nc = nc.replace('pub tags: &\'a mut crate::app::tags::Tags,', 'pub app_tags: &\'a mut crate::app::tags::Tags,')
            nc = nc.replace('tags,\n        expanded_dirs', 'app_tags,\n        expanded_dirs')
            nc = nc.replace('tags.remove_file(file);', 'app_tags.remove_file(file);')
            nc = nc.replace('tags.add_tags(new_path.clone(), tags);', 'app_tags.add_tags(new_path.clone(), tags);')
            
            with open(path, 'w', encoding='utf-8', newline='\n') as file:
                file.write(nc)

fix()
