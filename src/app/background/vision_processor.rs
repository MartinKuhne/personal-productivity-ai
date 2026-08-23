//! Vision-model inference worker — generates markdown descriptions for discovered images using an LLM.
//!
//! Unit tests live in the sibling `vision_processor_tests.rs` sidecar.

use crate::background::models::{BackgroundLogEntry, ImageJob, LogCategory};
use crate::bus::core::Bus;
use crate::bus::events::file::{FileEvent, FileEventProducer};
use crate::bus::events::typed::BackgroundEventSender;
use crate::config::AppConfig;
use base64::{Engine as _, engine::general_purpose::STANDARD as b64};
use serde_json::json;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

#[tracing::instrument(skip(config, tx, producer), name = "vision.process_image", fields(image = %job.image_path.display()))]
pub async fn process_image(
    job: ImageJob,
    config: AppConfig,
    tx: BackgroundEventSender,
    producer: &FileEventProducer,
) -> Result<(), String> {
    // Find vision model
    let vision_model = config.models.values().find(|m| m.has_vision());
    let model_cfg = match vision_model {
        Some(m) => m,
        None => {
            tracing::warn!(
                name = "vision.model.missing",
                "No vision model configured. Image processing skipped. Operator should configure a model with the 'vision' use case."
            );
            return Err("No vision model configured".to_string());
        }
    };

    // Read and encode image
    let img_data = match std::fs::read(&job.image_path) {
        Ok(data) => data,
        Err(e) => {
            tracing::error!(name = "vision.image.read_failed", path = %job.image_path.display(), error = %e, "Failed to read image file from disk. Likely cause: missing file or permission denied. Operator should verify file permissions.");
            return Err(format!("Failed to read image: {}", e));
        }
    };
    let b64_encoded = b64.encode(&img_data);

    let ext = job
        .image_path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tiff" => "image/tiff",
        "avif" => "image/avif",
        _ => "image/jpeg",
    };

    let payload = json!({
        "model": model_cfg.model,
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Describe this image in detailed Markdown. Include text, objects, scenes, charts, diagrams, and UI elements."
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:{};base64,{}", mime, b64_encoded)
                        }
                    }
                ]
            }
        ]
    });

    let api_url = model_cfg.api_url.clone();
    let api_key = model_cfg.api_key.clone();
    let img_name = job
        .image_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let _ = tx.send(
        BackgroundLogEntry::new(
            LogCategory::ImageVision,
            format!("Analyzing image {:?}", img_name),
        )
        .into(),
    );

    // Perform async HTTP request
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", api_url.trim_end_matches('/'));
    let response = client
        .post(url)
        .bearer_auth(&api_key)
        .json(&payload)
        .send()
        .await;

    match response {
        Ok(resp) => {
            if !resp.status().is_success() {
                let status = resp.status();
                let err_msg = format!("API request failed for {:?}: HTTP {}", img_name, status);
                tracing::error!(name = "vision.api.request_failed", image = %img_name, status = %status, "Vision API request failed with HTTP error.");
                let _ = tx.send(
                    BackgroundLogEntry::new(LogCategory::ImageVision, err_msg.clone()).into(),
                );
                return Err(err_msg);
            }
            let json: serde_json::Value = resp.json().await.map_err(|e| {
                tracing::error!(name = "vision.api.invalid_json", error = %e, image = %img_name, "Failed to parse JSON response from vision API. Operator should check model provider.");
                format!("Invalid JSON response: {}", e)
            })?;
            if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                if let Err(e) = std::fs::write(&job.md_path, content) {
                    tracing::error!(name = "vision.output.write_failed", path = %job.md_path.display(), error = %e, "Failed to write markdown output from vision analysis. Operator should verify disk space and write permissions.");
                    let msg = format!("Failed to write markdown for {:?}: {}", img_name, e);
                    let _ = tx.send(
                        BackgroundLogEntry::new(LogCategory::ImageVision, msg.clone()).into(),
                    );
                    return Err(msg);
                }

                // Tell the rest of the app the new `.md` exists so the
                // directory tree, tag manager, and render tab pick it
                // up without waiting for the notify watcher to fire.
                // Same pattern as `tool_create_note` and `editor.save`.
                producer.publish_discovered(&job.md_path);

                let _ = tx.send(
                    BackgroundLogEntry::new(
                        LogCategory::ImageVision,
                        format!("Successfully analyzed {:?}", img_name),
                    )
                    .into(),
                );
                Ok(())
            } else {
                tracing::error!(name = "vision.api.no_content", image = %img_name, response = ?json, "Vision API returned no content in choices. Operator should check model compatibility.");
                let msg = format!("No content in response for {:?}", img_name);
                let _ =
                    tx.send(BackgroundLogEntry::new(LogCategory::ImageVision, msg.clone()).into());
                Err(msg)
            }
        }
        Err(e) => {
            let err_msg = format!("API request failed for {:?}: {}", img_name, e);
            // reqwest's `reqwest::Error` does not carry the response
            // body on its own (the body lives on the response, which
            // is dropped by `send()` on error). The status is
            // available via `e.status()` when the failure was an
            // HTTP status; transport errors return `None`.
            if let Some(status) = e.status() {
                tracing::error!(name = "vision.api.request_failed", image = %img_name, status = %status, "Vision API request failed with HTTP error. Operator should verify API key and model limits.");
            } else {
                tracing::error!(name = "vision.api.network_error", image = %img_name, error = %e, "Vision API request failed completely. Operator should check network connectivity.");
            }
            let _ =
                tx.send(BackgroundLogEntry::new(LogCategory::ImageVision, err_msg.clone()).into());
            Err(err_msg)
        }
    }
}

pub struct ImageVisionWorker {
    rx: Receiver<PathBuf>,
    tx: BackgroundEventSender,
    config: AppConfig,
    bus: Bus<FileEvent>,
}

impl ImageVisionWorker {
    pub fn new(
        rx: Receiver<PathBuf>,
        tx: BackgroundEventSender,
        config: AppConfig,
        bus: Bus<FileEvent>,
    ) -> Self {
        Self {
            rx,
            tx,
            config,
            bus,
        }
    }

    pub fn spawn(self) {
        let ImageVisionWorker {
            rx,
            tx,
            config,
            bus,
        } = self;
        crate::bus::router::spawn_path_worker(rx, move |path| {
            let bus = bus.clone();
            let tx = tx.clone();
            let config = config.clone();
            async move {
                let job = ImageJob::new(path);
                if job.should_process() {
                    let producer = FileEventProducer::new(bus.clone());
                    let _ = process_image(job, config, tx, &producer).await;
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `vision_processor_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "vision_processor_tests.rs"]
mod tests;
