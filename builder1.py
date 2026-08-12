import os
import re

path = 'src/desktop/src/agent/tools/context.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

builder_code = '''
pub struct ToolContextBuilder {
    config: Arc<crate::config::AppConfig>,
    file_event_bus: Bus<FileEvent>,
    tool_manager: std::sync::Arc<std::sync::RwLock<crate::agent::tools::registry::ToolRegistry>>,
    cache: std::sync::Arc<crate::agent::tools::registry::cache::ToolCache>,
    uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
    browser_session: Option<Arc<BrowserSession>>,
    pdf_backing: Option<std::sync::Arc<crate::app::session::PdfBackingTracker>>,
}

impl ToolContextBuilder {
    pub fn new(
        config: Arc<crate::config::AppConfig>,
        file_event_bus: Bus<FileEvent>,
        tool_manager: std::sync::Arc<std::sync::RwLock<crate::agent::tools::registry::ToolRegistry>>,
        cache: std::sync::Arc<crate::agent::tools::registry::cache::ToolCache>,
        uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
    ) -> Self {
        Self {
            config,
            file_event_bus,
            tool_manager,
            cache,
            uuid_gen,
            browser_session: None,
            pdf_backing: None,
        }
    }

    pub fn with_browser_session(mut self, browser_session: Arc<BrowserSession>) -> Self {
        self.browser_session = Some(browser_session);
        self
    }

    pub fn with_pdf_backing(mut self, pdf_backing: std::sync::Arc<crate::app::session::PdfBackingTracker>) -> Self {
        self.pdf_backing = Some(pdf_backing);
        self
    }

    pub fn build(self) -> ToolContext {
        let resolver = VfsResolver::new(self.config.clone());
        let publisher = EventPublisher::new(self.file_event_bus.clone());
        ToolContext {
            config: self.config,
            file_event_bus: self.file_event_bus,
            resolver,
            publisher,
            browser_session: self.browser_session.unwrap_or_else(|| Arc::new(BrowserSession::disabled())),
            pdf_backing: self.pdf_backing.unwrap_or_else(|| Arc::new(crate::app::session::PdfBackingTracker::new())),
            cache: self.cache,
            tool_manager: self.tool_manager,
            uuid_gen: self.uuid_gen,
        }
    }
}
'''

content = content.replace('impl ToolContext {', builder_code + '\\nimpl ToolContext {')

# Remove the old 
ew method
content = re.sub(r'    pub fn new\(.*?\) -> Self \{.*?\n    \}\n', '', content, flags=re.DOTALL)

with open(path, 'w', encoding='utf-8', newline='\\n') as f:
    f.write(content)
