import os

for filename in ['tests/background_manager_test.rs', 'tests/log_persistence_test.rs']:
    if not os.path.exists(filename):
        # Maybe they are in src/desktop/tests/
        filename = os.path.join('src/desktop', filename)
        
    if os.path.exists(filename):
        with open(filename, 'r', encoding='utf-8') as file:
            content = file.read()

        nc = content.replace('background::manager', 'background::logs')
        nc = nc.replace('BackgroundProcessManager', 'BackgroundLogs')

        if nc != content:
            with open(filename, 'w', encoding='utf-8', newline='\n') as file:
                file.write(nc)
                
# Let's also rename the files themselves to match
if os.path.exists('src/desktop/tests/background_manager_test.rs'):
    os.rename('src/desktop/tests/background_manager_test.rs', 'src/desktop/tests/background_logs_test.rs')
