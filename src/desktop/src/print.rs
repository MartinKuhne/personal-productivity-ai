use crate::background::{BackgroundLogEntry, LogCategory};
use crate::messages::BackgroundMessage as MsgBackgroundMessage;
use pulldown_cmark::{html, Options, Parser};
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub struct PrintJob {
    pub markdown_path: PathBuf,
    pub markdown_content: String,
    pub title: String,
}

impl PrintJob {
    pub fn new(markdown_path: PathBuf) -> Self {
        let markdown_content = std::fs::read_to_string(&markdown_path).unwrap_or_default();
        let title = markdown_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Document")
            .to_string();
        Self {
            markdown_path,
            markdown_content,
            title,
        }
    }

    pub fn from_content(markdown_content: String, title: String) -> Self {
        let markdown_path = PathBuf::from(&title);
        Self {
            markdown_path,
            markdown_content,
            title,
        }
    }
}

fn markdown_to_html(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, Options::all());
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

fn build_html_document(title: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>{title}</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
            font-size: 10pt;
            line-height: 1.5;
            max-width: 800px;
            margin: 0 auto;
            padding: 30px 20px;
            color: #333;
        }}
        h1, h2, h3, h4, h5, h6 {{
            margin-top: 18px;
            margin-bottom: 12px;
            font-weight: 600;
            line-height: 1.25;
        }}
        h1 {{ font-size: 1.4em; border-bottom: 1px solid #eaecef; padding-bottom: 0.25em; }}
        h2 {{ font-size: 1.2em; border-bottom: 1px solid #eaecef; padding-bottom: 0.2em; }}
        h3 {{ font-size: 1.1em; }}
        p {{ margin: 0 0 10px 0; }}
        code {{
            font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
            font-size: 85%;
            background-color: rgba(27, 31, 35, 0.05);
            padding: 0.2em 0.4em;
            border-radius: 3px;
        }}
        pre {{
            font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
            font-size: 85%;
            line-height: 1.45;
            background-color: #f6f8fa;
            padding: 16px;
            border-radius: 6px;
            overflow: auto;
        }}
        pre code {{ background-color: transparent; padding: 0; }}
        blockquote {{
            margin: 0;
            padding: 0 1em;
            color: #6a737d;
            border-left: 0.25em solid #dfe2e5;
        }}
        table {{ border-collapse: collapse; width: 100%; margin-bottom: 16px; }}
        th, td {{ border: 1px solid #dfe2e5; padding: 6px 13px; }}
        th {{ background-color: #f6f8fa; }}
        img {{ max-width: 100%; height: auto; }}
        @media print {{ body {{ padding: 0; max-width: none; }} }}
    </style>
</head>
<body>
    {content}
</body>
</html>"#,
        title = html_escape::encode_text(title),
        content = content
    )
}

/// Opens the rendered HTML in the default browser for printing.
pub fn execute_print_blocking(
    job: PrintJob,
    tx: Option<Sender<MsgBackgroundMessage>>,
) -> Result<(), String> {
    let html_content = markdown_to_html(&job.markdown_content);
    let html_document = build_html_document(&job.title, &html_content);

    tracing::info!(
        name = "print.execute",
        title = %job.title,
        html_length = html_document.len(),
        "Opening in browser for printing"
    );

    let _ = tx.as_ref().map(|sender| {
        let _ = sender.send(MsgBackgroundMessage::LogEntry(BackgroundLogEntry::new(
            LogCategory::Print,
            format!("Opening browser for printing: {}", job.title),
        )));
    });

    let temp_dir = std::env::temp_dir();
    let file_name = format!("fastmd_print_{}.html", job.title.replace([' ', '.'], "_"));
    let file_path = temp_dir.join(&file_name);

    let mut file = std::fs::File::create(&file_path)
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    file.write_all(html_document.as_bytes())
        .map_err(|e| format!("Failed to write temp file: {}", e))?;
    drop(file);

    let path_str = file_path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", &path_str])
            .spawn()
            .map_err(|e| format!("Failed to open browser: {}", e))?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let status = std::process::Command::new("xdg-open")
            .arg(&path_str)
            .status()
            .map_err(|e| format!("Failed to open browser: {}", e))?;
        if !status.success() {
            std::process::Command::new("open")
                .arg(&path_str)
                .spawn()
                .map_err(|e| format!("Failed to open browser: {}", e))?;
        }
    }

    let _ = tx.as_ref().map(|sender| {
        let _ = sender.send(MsgBackgroundMessage::LogEntry(BackgroundLogEntry::new(
            LogCategory::Print,
            format!("Browser opened for printing: {}", job.title),
        )));
    });

    tracing::info!(
        name = "print.browser_opened",
        title = %job.title,
        path = %path_str,
        "Browser opened with document. User can print via Ctrl+P."
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_print_job_new() {
        let dir = tempdir().unwrap();
        let md_path = dir.path().join("test.md");
        std::fs::write(&md_path, "# Test\n\nContent").unwrap();

        let job = PrintJob::new(md_path.clone());
        assert_eq!(job.markdown_path, md_path);
        assert_eq!(job.title, "test");
        assert_eq!(job.markdown_content, "# Test\n\nContent");
    }

    #[test]
    fn test_print_job_from_content() {
        let job = PrintJob::from_content("# Test\n\nContent".to_string(), "My Doc".to_string());
        assert_eq!(job.title, "My Doc");
        assert_eq!(job.markdown_content, "# Test\n\nContent");
    }

    #[test]
    fn test_markdown_to_html() {
        let html = markdown_to_html("# Hello\n\n**Bold** text");
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>Bold</strong>"));
    }

    #[test]
    fn test_build_html_document() {
        let html = markdown_to_html("# Hi");
        let doc = build_html_document("Test", &html);
        assert!(doc.contains("<!DOCTYPE html>"));
        assert!(doc.contains("<title>Test</title>"));
    }
}
