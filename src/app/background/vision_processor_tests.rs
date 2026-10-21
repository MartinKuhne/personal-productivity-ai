//! Tests for `background/vision_processor.rs`.

use super::*;
use crate::bus::events::file::{FileChanged, FileChangedKind};
use crate::config::{AppConfig, LlmConfig};
use std::path::PathBuf;

use crate::ui_iced::test_helpers::noop_producer;

#[tokio::test]
async fn test_process_image_no_model() {
    let job = ImageJob {
        image_path: PathBuf::from("test.jpg"),
        md_path: PathBuf::from("test.md"),
    };
    let config = AppConfig::default();
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let result = process_image(
        job,
        config,
        BackgroundEventSender::new(tx),
        &noop_producer(),
    )
    .await;
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
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let result = process_image(
        job,
        config,
        BackgroundEventSender::new(tx),
        &noop_producer(),
    )
    .await;
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
            if let Some(mut stream) = listener.incoming().flatten().next() {
                use std::io::{Read, Write};
                let mut buf = [0; 4096];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(response.as_bytes());
                std::thread::sleep(std::time::Duration::from_millis(200));
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

    // Wire up a real bus + reader so we can verify that
    // `process_image` publishes a Discovered event for the
    // produced `.md` once the file is on disk. The bus is
    // owned (no leak needed since `FileEventProducer` is now
    // lifetime-free).
    let bus: Bus<FileChanged> = Bus::new();
    let reader = bus.subscribe();
    let producer = FileEventProducer::new(bus);

    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let result = process_image(job, config, BackgroundEventSender::new(tx), &producer).await;

    assert!(result.is_ok());
    let md_content = std::fs::read_to_string(&md_path).unwrap();
    assert_eq!(md_content, "Mock description");

    // The bus must have received a `[FileChanged][Added]` event for the
    // output `.md` ([REQ-332]: a not-previously-seen file is an addition).
    // This is what the directory tree and render tab rely on to pick up
    // the new file.
    let event = reader
        .recv_timeout(std::time::Duration::from_millis(200))
        .expect("process_image should publish an Added file event for the output .md");
    assert!(matches!(
        &event,
        changed
            if changed.kind == FileChangedKind::Added
                && changed.files.len() == 1
                && changed.files[0].path == md_path
    ));
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
            if let Some(mut stream) = listener.incoming().flatten().next() {
                use std::io::{Read, Write};
                let mut buf = [0; 4096];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(response.as_bytes());
                std::thread::sleep(std::time::Duration::from_millis(200));
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

    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let result = process_image(
        job,
        config,
        BackgroundEventSender::new(tx),
        &noop_producer(),
    )
    .await;

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
            if let Some(mut stream) = listener.incoming().flatten().next() {
                use std::io::{Read, Write};
                let mut buf = [0; 4096];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(response.as_bytes());
                std::thread::sleep(std::time::Duration::from_millis(200));
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

    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let result = process_image(
        job,
        config,
        BackgroundEventSender::new(tx),
        &noop_producer(),
    )
    .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("No content in response"));
}
