use crate::config::AppConfig;
use crate::messages::BackgroundMessage;
use crate::background::models::{BackgroundLogEntry, LogCategory, ImageJob};
use std::sync::mpsc::Sender;
use base64::{Engine as _, engine::general_purpose::STANDARD as b64};
use serde_json::json;

pub async fn process_image(job: ImageJob, config: AppConfig, tx: Sender<BackgroundMessage>) -> Result<(), String> {
    // Find vision model
    let vision_model = config.models.values().find(|m| m.has_vision());
    let model_cfg = match vision_model {
        Some(m) => m,
        None => return Err("No vision model configured".to_string()),
    };
    
    // Read and encode image
    let img_data = match std::fs::read(&job.image_path) {
        Ok(data) => data,
        Err(e) => return Err(format!("Failed to read image: {}", e)),
    };
    let b64_encoded = b64.encode(&img_data);
    
    let ext = job.image_path.extension().unwrap_or_default().to_string_lossy().to_lowercase();
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
    let img_name = job.image_path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    
    let _ = tx.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(
        LogCategory::ImageVision,
        format!("Analyzing image {:?}", img_name)
    )));
    
    // Perform blocking HTTP request
    let response = tokio::task::spawn_blocking(move || {
        ureq::post(&format!("{}/chat/completions", api_url.trim_end_matches('/')))
            .set("Authorization", &format!("Bearer {}", api_key))
            .set("Content-Type", "application/json")
            .send_json(payload)
    }).await.map_err(|e| format!("Spawn blocking error: {}", e))?;
    
    match response {
        Ok(resp) => {
            let json: serde_json::Value = resp.into_json().map_err(|e| format!("Invalid JSON response: {}", e))?;
            if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                if let Err(e) = std::fs::write(&job.md_path, content) {
                    let msg = format!("Failed to write markdown for {:?}: {}", img_name, e);
                    let _ = tx.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(LogCategory::ImageVision, msg.clone())));
                    return Err(msg);
                }
                
                let _ = tx.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(
                    LogCategory::ImageVision,
                    format!("Successfully analyzed {:?}", img_name)
                )));
                Ok(())
            } else {
                let msg = format!("No content in response for {:?}", img_name);
                let _ = tx.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(LogCategory::ImageVision, msg.clone())));
                Err(msg)
            }
        },
        Err(e) => {
            let mut err_msg = format!("API request failed for {:?}: {}", img_name, e);
            if let ureq::Error::Status(_, r) = e {
                if let Ok(text) = r.into_string() {
                    err_msg = format!("{} - {}", err_msg, text);
                }
            }
            let _ = tx.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(LogCategory::ImageVision, err_msg.clone())));
            Err(err_msg)
        }
    }
}
