//! PDF-to-markdown converter worker — invokes an external tool to produce a sibling `.md` file for each PDF.
//!
//! REQ-455B: background processes spawned by the FastMD Viewer SHALL NOT create a visible console
//! window on Windows. See `build_pdf_command` for the `CREATE_NO_WINDOW` enforcement.
//!
//! Unit tests live in the sibling `pdf_converter_tests.rs` sidecar.

use crate::background::models::{BackgroundLogEntry, LogCategory};
use crate::bus::core::Bus;
use crate::bus::events::file::FileChanged;
use crate::bus::events::typed::BackgroundEventSender;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use tokio::process::Command;
#[cfg(test)]
use tokio::sync::broadcast;

/// Win32 `CREATE_NO_WINDOW` flag for [`CreateProcessW`].
///
/// Passed to [`tokio::process::Command::creation_flags`] to prevent a
/// console-subsystem child from popping a visible console window when
/// launched from a GUI-subsystem parent. The child still receives a
/// (hidden) console buffer so stdout/stderr redirection to pipes keeps
/// working — only the visible window is suppressed.
/// See `doc/adr/cmd-substitution.md` for rationale.
///
/// [`CreateProcessW`]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build a [`tokio::process::Command`] for the PDF converter process.
///
/// On Windows, applies [`CREATE_NO_WINDOW`] (REQ-455B) so that the
/// child process does not flash an empty console window when the
/// application runs as a GUI-subsystem binary. On non-Windows
/// platforms the `creation_flags` call is gated away and this
/// function is a thin wrapper around `Command::new`.
pub(crate) fn build_pdf_command(exe: &str, args: &[String]) -> Command {
    let mut cmd = Command::new(exe);
    cmd.args(args);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub struct PdfConversionJob {
    pub input_pdf: PathBuf,
    pub output_md: PathBuf,
}

impl PdfConversionJob {
    pub fn new(input_pdf: PathBuf) -> Self {
        let mut output_md = input_pdf.clone();
        output_md.set_extension("md");
        Self {
            input_pdf,
            output_md,
        }
    }

    pub fn should_convert(&self) -> bool {
        if !self.output_md.exists() {
            return true;
        }
        if let (Ok(pdf_meta), Ok(md_meta)) = (
            std::fs::metadata(&self.input_pdf),
            std::fs::metadata(&self.output_md),
        ) && let (Ok(pdf_time), Ok(md_time)) = (pdf_meta.modified(), md_meta.modified())
        {
            return pdf_time > md_time;
        }
        false
    }

    #[tracing::instrument(skip(self, tx), name = "pdf_converter.execute", fields(pdf = %self.input_pdf.display()))]
    pub async fn execute(
        self,
        cmd_template: Option<Vec<String>>,
        tx: BackgroundEventSender,
    ) -> Result<(), String> {
        if let Some(template) = cmd_template {
            if template.is_empty() {
                tracing::warn!(
                    name = "pdf_converter.config.empty_template",
                    "PDF converter command template is empty. Skipping conversion. Operator should verify pdf_converter_command in configuration."
                );
                return Err("Command template is empty".to_string());
            }
            let mut args: Vec<String> = Vec::with_capacity(3);
            let exe = template[0].clone();

            let exe_lower = exe.to_lowercase();
            let is_marker = exe_lower.ends_with("marker")
                || exe_lower.ends_with("marker.exe")
                || exe_lower.ends_with("marker_single")
                || exe_lower.ends_with("marker_single.exe")
                || exe_lower.ends_with("marker_pdf")
                || exe_lower.ends_with("marker_pdf.exe");

            let mut actual_output_dir = None;

            if is_marker {
                let temp_name = format!(
                    "marker_{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                );
                let temp = std::env::temp_dir().join(temp_name);
                let _ = std::fs::create_dir_all(&temp);
                actual_output_dir = Some(temp);
            }

            for arg in template.iter().skip(1) {
                let replacement_out = if let Some(ref temp) = actual_output_dir {
                    temp.to_string_lossy().to_string()
                } else {
                    self.output_md.to_string_lossy().to_string()
                };

                let arg = arg
                    .replace("{input}", &self.input_pdf.to_string_lossy())
                    .replace("{output}", &replacement_out);
                args.push(arg);
            }

            let _ = tx.send(
                BackgroundLogEntry::new(
                    LogCategory::PdfConverter,
                    format!(
                        "Converting {:?}",
                        self.input_pdf.file_name().unwrap_or_default()
                    ),
                )
                .into(),
            );

            let output = build_pdf_command(&exe, &args)
                .output()
                .await
                .map_err(|e| {
                    tracing::error!(name = "pdf_converter.process.spawn_failed", path = %self.input_pdf.display(), exe = %exe, error = %e, "Failed to spawn PDF converter process. Likely cause: executable not found in PATH or insufficient permissions. Operator should verify pdf_converter_command in configuration and ensure the tool is installed.");
                    format!("Failed to spawn process: {}", e)
                })?;

            if output.status.success() {
                if let Some(temp) = actual_output_dir {
                    let mut md_found = false;
                    for entry in walkdir::WalkDir::new(&temp)
                        .into_iter()
                        .filter_map(Result::ok)
                        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                    {
                        if std::fs::copy(entry.path(), &self.output_md).is_ok() {
                            md_found = true;
                            if let Some(parent_dir) = entry.path().parent()
                                && let Ok(siblings) = std::fs::read_dir(parent_dir)
                            {
                                for sibling in siblings.flatten() {
                                    if sibling.path() != entry.path()
                                        && let Some(out_parent) = self.output_md.parent()
                                    {
                                        let target = out_parent.join(sibling.file_name());
                                        let _ = std::fs::rename(sibling.path(), target);
                                    }
                                }
                            }
                            break;
                        }
                    }
                    let _ = std::fs::remove_dir_all(&temp);

                    if !md_found {
                        let _ = tx.send(
                            BackgroundLogEntry::new(
                                LogCategory::PdfConverter,
                                format!(
                                    "Warning: Could not find output markdown from marker for {:?}",
                                    self.input_pdf.file_name().unwrap_or_default()
                                ),
                            )
                            .into(),
                        );
                    }
                }

                let _ = tx.send(
                    BackgroundLogEntry::new(
                        LogCategory::PdfConverter,
                        format!(
                            "Successfully converted {:?}",
                            self.input_pdf.file_name().unwrap_or_default()
                        ),
                    )
                    .into(),
                );
                Ok(())
            } else {
                let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
                let msg = format!(
                    "Conversion failed for {:?}: {}",
                    self.input_pdf.file_name().unwrap_or_default(),
                    err_msg
                );
                tracing::error!(name = "pdf_converter.process.failed", path = %self.input_pdf.display(), exit_code = ?output.status.code(), stderr = %err_msg, "PDF conversion process returned a non-zero exit status. Operator should check the stderr output for details.");
                let _ =
                    tx.send(BackgroundLogEntry::new(LogCategory::PdfConverter, msg.clone()).into());
                Err(msg)
            }
        } else {
            tracing::warn!(
                name = "pdf_converter.config.not_configured",
                "No pdf_converter_command configured. Skipping PDF conversion. Operator should provide a command template in configuration if PDF conversion is desired."
            );
            Err("No pdf_converter_command configured".to_string())
        }
    }
}

pub struct PdfConverterWorker {
    rx: Receiver<PathBuf>,
    tx: BackgroundEventSender,
    bus: Bus<FileChanged>,
    cmd: Option<Vec<String>>,
}

impl PdfConverterWorker {
    pub fn new(
        rx: Receiver<PathBuf>,
        tx: BackgroundEventSender,
        bus: Bus<FileChanged>,
        cmd: Option<Vec<String>>,
    ) -> Self {
        Self { rx, tx, bus, cmd }
    }

    pub fn spawn(self) {
        let PdfConverterWorker { rx, tx, bus, cmd } = self;
        crate::bus::router::spawn_path_worker(rx, move |path| {
            let bus = bus.clone();
            let tx = tx.clone();
            let cmd = cmd.clone();
            async move {
                let job = PdfConversionJob::new(path);
                if job.should_convert() {
                    let output_md = job.output_md.clone();
                    if job.execute(cmd, tx).await.is_ok() {
                        bus.publish(FileChanged::discovered_one(output_md));
                    }
                }
            }
        });
    }
}

#[cfg(test)]
#[path = "pdf_converter_tests.rs"]
mod tests;
