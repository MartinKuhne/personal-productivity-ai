//! Vision-model inference worker — generates markdown descriptions for discovered images using an LLM.

use crate::background::models::{BackgroundLogEntry, ImageJob, LogCategory};
use crate::config::AppConfig;
use crate::file_events::{Bus, FileEvent, FileEventProducer};
use crate::messages::BackgroundMessage;
use base64::{Engine as _, engine::general_purpose::STANDARD as b64};
use serde_json::json;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

pub async fn process_image<'a>(
    job: ImageJob,
    config: AppConfig,
    tx: Sender<BackgroundMessage>,
    producer: &FileEventProducer<'a>,
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

    let _ = tx.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(
        LogCategory::ImageVision,
        format!("Analyzing image {:?}", img_name),
    )));

    // Perform blocking HTTP request
    let response = tokio::task::spawn_blocking(move || {
        ureq::post(&format!("{}/chat/completions", api_url.trim_end_matches('/')))
            .set("Authorization", &format!("Bearer {}", api_key))
            .set("Content-Type", "application/json")
            .send_json(payload)
    }).await.map_err(|e| {
        tracing::error!(name = "vision.api.spawn_failed", error = %e, "Failed to spawn blocking task for vision API request. Operator should check system resources.");
        format!("Spawn blocking error: {}", e)
    })?;

    match response {
        Ok(resp) => {
            let json: serde_json::Value = resp.into_json().map_err(|e| {
                tracing::error!(name = "vision.api.invalid_json", error = %e, image = %img_name, "Failed to parse JSON response from vision API. Operator should check model provider.");
                format!("Invalid JSON response: {}", e)
            })?;
            if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                if let Err(e) = std::fs::write(&job.md_path, content) {
                    tracing::error!(name = "vision.output.write_failed", path = %job.md_path.display(), error = %e, "Failed to write markdown output from vision analysis. Operator should verify disk space and write permissions.");
                    let msg = format!("Failed to write markdown for {:?}: {}", img_name, e);
                    let _ = tx.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(
                        LogCategory::ImageVision,
                        msg.clone(),
                    )));
                    return Err(msg);
                }

                // Tell the rest of the app the new `.md` exists so the
                // directory tree, tag manager, and render tab pick it
                // up without waiting for the notify watcher to fire.
                // Same pattern as `tool_create_file` and `editor.save`.
                producer.publish_discovered(&job.md_path);

                let _ = tx.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(
                    LogCategory::ImageVision,
                    format!("Successfully analyzed {:?}", img_name),
                )));
                Ok(())
            } else {
                tracing::error!(name = "vision.api.no_content", image = %img_name, response = ?json, "Vision API returned no content in choices. Operator should check model compatibility.");
                let msg = format!("No content in response for {:?}", img_name);
                let _ = tx.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(
                    LogCategory::ImageVision,
                    msg.clone(),
                )));
                Err(msg)
            }
        }
        Err(e) => {
            let mut err_msg = format!("API request failed for {:?}: {}", img_name, e);
            if let ureq::Error::Status(code, r) = e {
                let text = r.into_string().unwrap_or_default();
                err_msg = format!("{} - {}", err_msg, text);
                tracing::error!(name = "vision.api.request_failed", image = %img_name, status = code, response = %text, "Vision API request failed with HTTP error. Operator should verify API key and model limits.");
            } else {
                tracing::error!(name = "vision.api.network_error", image = %img_name, error = %e, "Vision API request failed completely. Operator should check network connectivity.");
            }
            let _ = tx.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(
                LogCategory::ImageVision,
                err_msg.clone(),
            )));
            Err(err_msg)
        }
    }
}

pub struct ImageVisionWorker {
    rx: Receiver<PathBuf>,
    tx: Sender<BackgroundMessage>,
    config: AppConfig,
    bus: Bus<FileEvent>,
}

impl ImageVisionWorker {
    pub fn new(
        rx: Receiver<PathBuf>,
        tx: Sender<BackgroundMessage>,
        config: AppConfig,
        bus: Bus<FileEvent>,
    ) -> Self {
        Self { rx, tx, config, bus }
    }

    pub fn spawn(self) {
        std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    while let Ok(path) = self.rx.recv() {
                        let job = ImageJob::new(path);
                        if job.should_process() {
                            let producer = FileEventProducer::new(&self.bus);
                            let _ = process_image(
                                job,
                                self.config.clone(),
                                self.tx.clone(),
                                &producer,
                            )
                            .await;
                        }
                    }
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, LlmConfig};
    use crate::file_events::{Bus, FileEvent};
    use std::path::PathBuf;
    use std::sync::mpsc;

    /// Build a `FileEventProducer` backed by a leaked (and therefore
    /// `'static`) no-op bus. Useful for tests that exercise
    /// `process_image` without caring about what (if anything) is
    /// published.
    fn noop_producer() -> FileEventProducer<'static> {
        let bus: &'static Bus<FileEvent> = Box::leak(Box::new(Bus::new()));
        FileEventProducer::new(bus)
    }

    #[tokio::test]
    async fn test_process_image_no_model() {
        let job = ImageJob {
            image_path: PathBuf::from("test.jpg"),
            md_path: PathBuf::from("test.md"),
        };
        let config = AppConfig::default();
        let (tx, _rx) = mpsc::channel();
        let result = process_image(job, config, tx, &noop_producer()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No vision model configured");
    }

    #[tokio::test]
    async fn test_process_image_missing_file() {
        let job = ImageJob {
            image_path: PathBuf::from("nonexistent.jpg"),
            md_path: PathBuf::from("test.md"),
        };
        let mut config = AppConfig::default();
        config.models.insert(
            "test".to_string(),
            LlmConfig {
                model: "test-vision".to_string(),
                api_key: "dummy".to_string(),
                api_url: "dummy".to_string(),
                cost: None,
                use_case: vec!["vision".to_string()],
            },
        );
        let (tx, _rx) = mpsc::channel();
        let result = process_image(job, config, tx, &noop_producer()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read image"));
    }

    #[tokio::test]
    async fn test_process_image_success() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mock_server = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let body = r#"{"choices": [{"message": {"content": "Mock description"}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if let Ok(mut stream) = stream {
                        use std::io::{Read, Write};
                        let mut buf = [0; 4096];
                        let _ = stream.read(&mut buf);
                        let _ = stream.write_all(response.as_bytes());
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        break;
                    }
                }
            });
            format!("http://127.0.0.1:{}", port)
        };

        let temp_dir = tempfile::tempdir().unwrap();
        let image_path = temp_dir.path().join("test_image.png");
        std::fs::write(&image_path, b"fake image data").unwrap();
        let md_path = temp_dir.path().join("test_output.md");

        let job = ImageJob {
            image_path: image_path.clone(),
            md_path: md_path.clone(),
        };

        let mut config = AppConfig::default();
        config.models.insert(
            "test".to_string(),
            LlmConfig {
                model: "test-vision".to_string(),
                api_key: "dummy".to_string(),
                api_url: mock_server,
                cost: None,
                use_case: vec!["vision".to_string()],
            },
        );

        // Wire up a real (leaked) bus + reader so we can verify
        // that `process_image` publishes a Discovered event for
        // the produced `.md` once the file is on disk.
        let bus: &'static Bus<FileEvent> = Box::leak(Box::new(Bus::new()));
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(bus);

        let (tx, _rx) = mpsc::channel();
        let result = process_image(job, config, tx, &producer).await;

        assert!(result.is_ok());
        let md_content = std::fs::read_to_string(&md_path).unwrap();
        assert_eq!(md_content, "Mock description");

        // The bus must have received a Discovered event for the
        // output `.md`. This is what the directory tree and
        // render tab rely on to pick up the new file.
        let event = reader
            .recv_timeout(std::time::Duration::from_millis(200))
            .expect("process_image should publish a Discovered event for the output .md");
        assert_eq!(event.kind, crate::file_events::FileEventKind::Discovered);
        assert_eq!(event.paths[0], md_path);
    }

    #[tokio::test]
    async fn test_process_image_api_error() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mock_server = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let body = r#"{"error": "bad request"}"#;
            let response = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if let Ok(mut stream) = stream {
                        use std::io::{Read, Write};
                        let mut buf = [0; 4096];
                        let _ = stream.read(&mut buf);
                        let _ = stream.write_all(response.as_bytes());
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        break;
                    }
                }
            });
            format!("http://127.0.0.1:{}", port)
        };

        let temp_dir = tempfile::tempdir().unwrap();
        let image_path = temp_dir.path().join("test_image2.png");
        std::fs::write(&image_path, b"fake image data").unwrap();
        let md_path = temp_dir.path().join("test_output2.md");

        let job = ImageJob {
            image_path: image_path.clone(),
            md_path: md_path.clone(),
        };

        let mut config = AppConfig::default();
        config.models.insert(
            "test".to_string(),
            LlmConfig {
                model: "test-vision".to_string(),
                api_key: "dummy".to_string(),
                api_url: mock_server,
                cost: None,
                use_case: vec!["vision".to_string()],
            },
        );

        let (tx, _rx) = mpsc::channel();
        let result = process_image(job, config, tx, &noop_producer()).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API request failed"));
    }

    #[tokio::test]
    async fn test_process_image_no_content() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mock_server = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let body = r#"{"choices": [{"message": {}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if let Ok(mut stream) = stream {
                        use std::io::{Read, Write};
                        let mut buf = [0; 4096];
                        let _ = stream.read(&mut buf);
                        let _ = stream.write_all(response.as_bytes());
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        break;
                    }
                }
            });
            format!("http://127.0.0.1:{}", port)
        };

        let temp_dir = tempfile::tempdir().unwrap();
        let image_path = temp_dir.path().join("test_image3.png");
        std::fs::write(&image_path, b"fake image data").unwrap();
        let md_path = temp_dir.path().join("test_output3.md");

        let job = ImageJob {
            image_path: image_path.clone(),
            md_path: md_path.clone(),
        };

        let mut config = AppConfig::default();
        config.models.insert(
            "test".to_string(),
            LlmConfig {
                model: "test-vision".to_string(),
                api_key: "dummy".to_string(),
                api_url: mock_server,
                cost: None,
                use_case: vec!["vision".to_string()],
            },
        );

        let (tx, _rx) = mpsc::channel();
        let result = process_image(job, config, tx, &noop_producer()).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No content in response"));
    }
}
