//! File-event bus router — subscribes to bus events and routes newly
//! discovered files to PDF/vision worker queues by extension.

use crate::bus::core::Bus;
use crate::bus::events::file::{FileEvent, FileEventKind};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Subscribes to a [`Bus<FileEvent>`] and fans out newly discovered /
/// updated files to per-format worker channels by extension.
pub struct BusRouter {
    bus: Bus<FileEvent>,
    tx_pdf: Sender<PathBuf>,
    tx_img: Sender<PathBuf>,
}

impl BusRouter {
    pub fn new(bus: Bus<FileEvent>, tx_pdf: Sender<PathBuf>, tx_img: Sender<PathBuf>) -> Self {
        Self {
            bus,
            tx_pdf,
            tx_img,
        }
    }

    pub fn spawn(self) {
        let bus = self.bus.clone();
        let tx_pdf = self.tx_pdf;
        let tx_img = self.tx_img;

        std::thread::spawn(move || {
            let reader = bus.subscribe();
            let mut pdf_open = true;
            let mut img_open = true;

            while let Ok(event) = reader.recv() {
                if !pdf_open && !img_open {
                    break;
                }

                if !matches!(
                    event.kind,
                    FileEventKind::Discovered | FileEventKind::Updated
                ) {
                    continue;
                }

                for p in &event.paths {
                    let ext = p
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase());
                    let ext_str = ext.as_deref().unwrap_or("");

                    if pdf_open
                        && ext_str == "pdf"
                        && let Err(e) = tx_pdf.send(p.clone())
                    {
                        tracing::warn!(
                            name = "background_task.pdf_bus.tx_closed",
                            error = %e,
                            "PDF bus subscriber could not deliver to tx_pdf. Channel is closed."
                        );
                        pdf_open = false;
                    } else if img_open
                        && matches!(
                            ext_str,
                            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "avif"
                        )
                        && let Err(e) = tx_img.send(p.clone())
                    {
                        tracing::warn!(
                            name = "background_task.img_bus.tx_closed",
                            error = %e,
                            "Image bus subscriber could not deliver to tx_img. Channel is closed."
                        );
                        img_open = false;
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn test_pdf_router_forwards_pdf() {
        let bus: Bus<FileEvent> = Bus::new();
        let (tx_pdf, rx_pdf) = channel();
        let (tx_img, _rx_img) = channel();

        let router = BusRouter::new(bus.clone(), tx_pdf, tx_img);
        router.spawn();
        std::thread::sleep(std::time::Duration::from_millis(50));

        bus.publish(FileEvent::discovered_one(PathBuf::from("test.pdf")));

        let path = rx_pdf.recv_timeout(std::time::Duration::from_millis(500));
        assert!(path.is_ok());
        assert_eq!(path.unwrap(), PathBuf::from("test.pdf"));
    }

    #[test]
    fn test_pdf_router_ignores_md() {
        let bus: Bus<FileEvent> = Bus::new();
        let (tx_pdf, rx_pdf) = channel();
        let (tx_img, _rx_img) = channel();

        let router = BusRouter::new(bus.clone(), tx_pdf, tx_img);
        router.spawn();
        std::thread::sleep(std::time::Duration::from_millis(50));

        bus.publish(FileEvent::discovered_one(PathBuf::from("test.md")));

        let path = rx_pdf.recv_timeout(std::time::Duration::from_millis(100));
        assert!(path.is_err());
    }

    #[test]
    fn test_img_router_forwards_image() {
        let bus: Bus<FileEvent> = Bus::new();
        let (tx_pdf, _rx_pdf) = channel();
        let (tx_img, rx_img) = channel();

        let router = BusRouter::new(bus.clone(), tx_pdf, tx_img);
        router.spawn();
        std::thread::sleep(std::time::Duration::from_millis(50));

        bus.publish(FileEvent::discovered_one(PathBuf::from("test.png")));

        let path = rx_img.recv_timeout(std::time::Duration::from_millis(500));
        assert!(path.is_ok());
        assert_eq!(path.unwrap(), PathBuf::from("test.png"));
    }

    #[test]
    fn test_img_router_ignores_pdf() {
        let bus: Bus<FileEvent> = Bus::new();
        let (tx_pdf, _rx_pdf) = channel();
        let (tx_img, rx_img) = channel();

        let router = BusRouter::new(bus.clone(), tx_pdf, tx_img);
        router.spawn();
        std::thread::sleep(std::time::Duration::from_millis(50));

        bus.publish(FileEvent::discovered_one(PathBuf::from("test.pdf")));

        let path = rx_img.recv_timeout(std::time::Duration::from_millis(100));
        assert!(path.is_err());
    }
}
