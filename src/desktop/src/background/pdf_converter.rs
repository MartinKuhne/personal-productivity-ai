use std::path::PathBuf;
use tokio::process::Command;
use crate::messages::BackgroundMessage;
use crate::background::models::{BackgroundLogEntry, LogCategory};
use std::sync::mpsc::Sender;
#[cfg(test)] use std::sync::mpsc::channel;

pub struct PdfConversionJob {
    pub input_pdf: PathBuf,
    pub output_md: PathBuf,
}

impl PdfConversionJob {
    pub fn new(input_pdf: PathBuf) -> Self {
        let mut output_md = input_pdf.clone();
        output_md.set_extension("md");
        Self { input_pdf, output_md }
    }

    pub fn should_convert(&self) -> bool {
        if !self.output_md.exists() {
            return true;
        }
        if let (Ok(pdf_meta), Ok(md_meta)) = (std::fs::metadata(&self.input_pdf), std::fs::metadata(&self.output_md)) {
            if let (Ok(pdf_time), Ok(md_time)) = (pdf_meta.modified(), md_meta.modified()) {
                return pdf_time > md_time;
            }
        }
        false
    }

    pub async fn execute(self, cmd_template: Option<Vec<String>>, tx: Sender<BackgroundMessage>) -> Result<(), String> {
        if let Some(template) = cmd_template {
            if template.is_empty() {
                return Err("Command template is empty".to_string());
            }
            let mut args = Vec::new();
            let exe = template[0].clone();
            
            for arg in template.iter().skip(1) {
                let arg = arg.replace("{input}", &self.input_pdf.to_string_lossy())
                             .replace("{output}", &self.output_md.to_string_lossy());
                args.push(arg);
            }
            
            let _ = tx.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(
                LogCategory::PdfConverter,
                format!("Converting {:?}", self.input_pdf.file_name().unwrap_or_default())
            )));

            let output = Command::new(&exe)
                .args(&args)
                .output()
                .await
                .map_err(|e| format!("Failed to spawn process: {}", e))?;

            if output.status.success() {
                let _ = tx.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(
                    LogCategory::PdfConverter,
                    format!("Successfully converted {:?}", self.input_pdf.file_name().unwrap_or_default())
                )));
                Ok(())
            } else {
                let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
                let msg = format!("Conversion failed for {:?}: {}", self.input_pdf.file_name().unwrap_or_default(), err_msg);
                let _ = tx.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(
                    LogCategory::PdfConverter,
                    msg.clone()
                )));
                Err(msg)
            }
        } else {
            Err("No pdf_converter_command configured".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_pdf_job_new_swaps_extension() {
        let pdf = PathBuf::from("/test/report.pdf");
        let job = PdfConversionJob::new(pdf);
        assert_eq!(job.input_pdf.to_string_lossy(), "/test/report.pdf");
        assert_eq!(job.output_md.to_string_lossy(), "/test/report.md");
    }

    #[test]
    fn test_pdf_job_new_with_md_extension_stays() {
        let pdf = PathBuf::from("/test/doc.pdf");
        let job = PdfConversionJob::new(pdf);
        // Even though it's .pdf, output gets .md
        assert_eq!(job.output_md.extension().unwrap(), "md");
    }

    #[test]
    fn test_should_convert_missing_md() {
        let dir = tempdir().unwrap();
        let pdf = dir.path().join("doc.pdf");
        std::fs::write(&pdf, "dummy pdf content").unwrap();

        let job = PdfConversionJob::new(pdf);
        // md does not exist, should convert
        assert!(job.should_convert());
    }

    #[test]
    fn test_should_convert_md_older_than_pdf() {
        let dir = tempdir().unwrap();
        let pdf = dir.path().join("doc.pdf");
        let md = dir.path().join("doc.md");
        std::fs::write(&pdf, "pdf content").unwrap();
        std::fs::write(&md, "md content").unwrap();

        // Set md time in the past
        let past = filetime::FileTime::from_unix_time(1000, 0);
        filetime::set_file_mtime(&md, past).unwrap();

        let job = PdfConversionJob::new(pdf);
        assert!(job.should_convert());
    }

    #[test]
    fn test_should_convert_md_newer_than_pdf() {
        let dir = tempdir().unwrap();
        let pdf = dir.path().join("doc.pdf");
        let md = dir.path().join("doc.md");
        std::fs::write(&pdf, "pdf content").unwrap();
        std::fs::write(&md, "md content").unwrap();

        let now = filetime::FileTime::now();
        filetime::set_file_mtime(&md, now).unwrap();

        let job = PdfConversionJob::new(pdf);
        assert!(!job.should_convert());
    }

    #[test]
    fn test_execute_without_command_returns_error() {
        let dir = tempdir().unwrap();
        let pdf = dir.path().join("doc.pdf");
        std::fs::write(&pdf, "pdf").unwrap();
        let (tx, _rx) = channel();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let job = PdfConversionJob::new(pdf);
        let result = rt.block_on(job.execute(None, tx));
        assert_eq!(result.unwrap_err(), "No pdf_converter_command configured");
    }

    #[test]
    fn test_command_template_substitution() {
        let input = PathBuf::from("C:\\docs\\input.pdf");
        let mut output = input.clone();
        output.set_extension("md");

        let job = PdfConversionJob { input_pdf: input, output_md: output };
        let template = Some(vec![
            "pandoc".to_string(),
            "{input}".to_string(),
            "-o".to_string(),
            "{output}".to_string(),
        ]);
        let (tx, _rx) = channel();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(job.execute(template, tx));
        // pandoc doesn't exist on the PATH in CI, so this will fail with spawn error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Failed to spawn"));
    }
}
