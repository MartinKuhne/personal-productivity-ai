//! Tests for [`BusRouter`] (`bus::router::bus_router`).

use super::*;
use std::sync::mpsc::channel;
use tempfile::tempdir;

#[test]
fn test_pdf_router_forwards_pdf() {
    let bus: Bus<FileChanged> = Bus::new();
    let (tx_pdf, rx_pdf) = channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = channel();

    #[cfg(feature = "image-library")]
    let router = BusRouter::new(bus.clone(), tx_pdf, tx_img);
    #[cfg(not(feature = "image-library"))]
    let router = BusRouter::new(bus.clone(), tx_pdf);
    router.spawn();
    std::thread::sleep(std::time::Duration::from_millis(50));

    bus.publish(FileChanged::discovered_one(PathBuf::from("test.pdf")));

    let path = rx_pdf.recv_timeout(std::time::Duration::from_millis(500));
    assert!(path.is_ok());
    assert_eq!(path.unwrap(), PathBuf::from("test.pdf"));
}

#[test]
fn test_pdf_router_ignores_md() {
    let bus: Bus<FileChanged> = Bus::new();
    let (tx_pdf, rx_pdf) = channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = channel();

    #[cfg(feature = "image-library")]
    let router = BusRouter::new(bus.clone(), tx_pdf, tx_img);
    #[cfg(not(feature = "image-library"))]
    let router = BusRouter::new(bus.clone(), tx_pdf);
    router.spawn();
    std::thread::sleep(std::time::Duration::from_millis(50));

    bus.publish(FileChanged::discovered_one(PathBuf::from("test.md")));

    let path = rx_pdf.recv_timeout(std::time::Duration::from_millis(100));
    assert!(path.is_err());
}

#[cfg(feature = "image-library")]
#[test]
fn test_img_router_forwards_image() {
    let bus: Bus<FileChanged> = Bus::new();
    let (tx_pdf, _rx_pdf) = channel();
    let (tx_img, rx_img) = channel();

    let router = BusRouter::new(bus.clone(), tx_pdf, tx_img);
    router.spawn();
    std::thread::sleep(std::time::Duration::from_millis(50));

    bus.publish(FileChanged::discovered_one(PathBuf::from("test.png")));

    let path = rx_img.recv_timeout(std::time::Duration::from_millis(500));
    assert!(path.is_ok());
    assert_eq!(path.unwrap(), PathBuf::from("test.png"));
}

#[cfg(feature = "image-library")]
#[test]
fn test_img_router_ignores_pdf() {
    let bus: Bus<FileChanged> = Bus::new();
    let (tx_pdf, _rx_pdf) = channel();
    let (tx_img, rx_img) = channel();

    let router = BusRouter::new(bus.clone(), tx_pdf, tx_img);
    router.spawn();
    std::thread::sleep(std::time::Duration::from_millis(50));

    bus.publish(FileChanged::discovered_one(PathBuf::from("test.pdf")));

    let path = rx_img.recv_timeout(std::time::Duration::from_millis(100));
    assert!(path.is_err());
}

#[test]
fn test_pdf_router_ignores_no_extension() {
    let bus: Bus<FileChanged> = Bus::new();
    let (tx_pdf, rx_pdf) = channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = channel();

    #[cfg(feature = "image-library")]
    let router = BusRouter::new(bus.clone(), tx_pdf, tx_img);
    #[cfg(not(feature = "image-library"))]
    let router = BusRouter::new(bus.clone(), tx_pdf);
    router.spawn();
    std::thread::sleep(std::time::Duration::from_millis(50));

    bus.publish(FileChanged::discovered_one(PathBuf::from("test")));

    let path = rx_pdf.recv_timeout(std::time::Duration::from_millis(100));
    assert!(path.is_err());
}

#[test]
fn test_pdf_router_normalizes_uppercase_extension() {
    let bus: Bus<FileChanged> = Bus::new();
    let (tx_pdf, rx_pdf) = channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = channel();

    #[cfg(feature = "image-library")]
    let router = BusRouter::new(bus.clone(), tx_pdf, tx_img);
    #[cfg(not(feature = "image-library"))]
    let router = BusRouter::new(bus.clone(), tx_pdf);
    router.spawn();
    std::thread::sleep(std::time::Duration::from_millis(50));

    bus.publish(FileChanged::discovered_one(PathBuf::from("test.PDF")));

    let path = rx_pdf.recv_timeout(std::time::Duration::from_millis(500));
    assert!(path.is_ok());
    assert_eq!(path.unwrap(), PathBuf::from("test.PDF"));
}

/// Deleted events are not conversion work; directory events ride the
/// separate directory bus and can never reach this router by type.
#[test]
fn test_pdf_router_filters_deleted_events() {
    let bus: Bus<FileChanged> = Bus::new();
    let (tx_pdf, rx_pdf) = channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = channel();

    #[cfg(feature = "image-library")]
    let router = BusRouter::new(bus.clone(), tx_pdf, tx_img);
    #[cfg(not(feature = "image-library"))]
    let router = BusRouter::new(bus.clone(), tx_pdf);
    router.spawn();
    std::thread::sleep(std::time::Duration::from_millis(50));

    bus.publish(FileChanged::removed_one(PathBuf::from("gone.pdf")));
    // [DirectoryChanged] events (e.g. for a directory named "dir.pdf")
    // live on `Bus<DirectoryChanged>` — the router is typed on the file
    // bus and cannot observe them at all.

    let path = rx_pdf.recv_timeout(std::time::Duration::from_millis(100));
    assert!(path.is_err());
}

#[test]
fn test_pdf_router_forwards_updated_pdf() {
    let bus: Bus<FileChanged> = Bus::new();
    let (tx_pdf, rx_pdf) = channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = channel();

    #[cfg(feature = "image-library")]
    let router = BusRouter::new(bus.clone(), tx_pdf, tx_img);
    #[cfg(not(feature = "image-library"))]
    let router = BusRouter::new(bus.clone(), tx_pdf);
    router.spawn();
    std::thread::sleep(std::time::Duration::from_millis(50));

    bus.publish(FileChanged::updated_one(PathBuf::from("upd.pdf")));

    let path = rx_pdf.recv_timeout(std::time::Duration::from_millis(500));
    assert!(path.is_ok());
    assert_eq!(path.unwrap(), PathBuf::from("upd.pdf"));
}

#[test]
fn test_pdf_router_empty_payload_is_noop() {
    let bus: Bus<FileChanged> = Bus::new();
    let (tx_pdf, rx_pdf) = channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = channel();

    #[cfg(feature = "image-library")]
    let router = BusRouter::new(bus.clone(), tx_pdf, tx_img);
    #[cfg(not(feature = "image-library"))]
    let router = BusRouter::new(bus.clone(), tx_pdf);
    router.spawn();
    std::thread::sleep(std::time::Duration::from_millis(50));

    bus.publish(FileChanged::discovered(Vec::new()));

    let path = rx_pdf.recv_timeout(std::time::Duration::from_millis(100));
    assert!(path.is_err());
}

#[test]
fn test_pdf_manager_does_not_queue_without_converter_configuration() {
    let directory = tempdir().unwrap();
    let pdf = directory.path().join("report.pdf");
    std::fs::write(&pdf, b"pdf").unwrap();
    let bus: Bus<FileChanged> = Bus::new();
    let (tx_pdf, rx_pdf) = channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = channel();

    #[cfg(feature = "image-library")]
    let router = BusRouter::new_pdf_only(bus.clone(), tx_pdf, false, tx_img);
    #[cfg(not(feature = "image-library"))]
    let router = BusRouter::new_pdf_only(bus.clone(), tx_pdf, false);
    router.spawn();
    assert_eq!(bus.subscriber_count(), 1);
    bus.publish(FileChanged::discovered_one(pdf));

    assert!(
        rx_pdf
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err()
    );
}

#[test]
fn test_pdf_manager_skips_conversion_when_markdown_is_newer() {
    let directory = tempdir().unwrap();
    let pdf = directory.path().join("report.pdf");
    let markdown = directory.path().join("report.md");
    std::fs::write(&pdf, b"pdf").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(25));
    std::fs::write(&markdown, "newer markdown").unwrap();
    assert!(!PdfConversionJob::new(pdf.clone()).should_convert());

    let bus: Bus<FileChanged> = Bus::new();
    let (tx_pdf, rx_pdf) = channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = channel();

    #[cfg(feature = "image-library")]
    let router = BusRouter::new_pdf_only(bus.clone(), tx_pdf, true, tx_img);
    #[cfg(not(feature = "image-library"))]
    let router = BusRouter::new_pdf_only(bus.clone(), tx_pdf, true);
    router.spawn();
    bus.publish(FileChanged::discovered_one(pdf));

    assert!(
        rx_pdf
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err()
    );
}
