use std::path::PathBuf;
use tokio::process::Command;
use crate::messages::BackgroundMessage;
use crate::background::models::{BackgroundLogEntry, LogCategory};
use std::sync::mpsc::Sender;

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
