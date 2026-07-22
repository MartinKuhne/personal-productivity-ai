use crate::file_events::{Bus, FileEvent, FileEventKind};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

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
        let bus_pdf = self.bus.clone();
        let tx_pdf = self.tx_pdf;
        std::thread::spawn(move || {
            let reader = bus_pdf.subscribe();
            loop {
                let event = match reader.recv() {
                    Ok(e) => e,
                    Err(_) => break,
                };
                if !matches!(
                    event.kind,
                    FileEventKind::Discovered | FileEventKind::Updated
                ) {
                    continue;
                }
                let is_pdf = event
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("pdf"))
                    .unwrap_or(false);
                if !is_pdf {
                    continue;
                }
                if let Err(e) = tx_pdf.send(event.path) {
                    tracing::warn!(
                        name = "background_task.pdf_bus.tx_closed",
                        error = %e,
                        "PDF bus subscriber could not deliver to tx_pdf. Channel is closed."
                    );
                    break;
                }
            }
        });

        let bus_img = self.bus.clone();
        let tx_img = self.tx_img;
        std::thread::spawn(move || {
            let reader = bus_img.subscribe();
            loop {
                let event = match reader.recv() {
                    Ok(e) => e,
                    Err(_) => break,
                };
                if !matches!(
                    event.kind,
                    FileEventKind::Discovered | FileEventKind::Updated
                ) {
                    continue;
                }
                let is_img = event
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| {
                        matches!(
                            e.to_lowercase().as_str(),
                            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "avif"
                        )
                    })
                    .unwrap_or(false);
                if !is_img {
                    continue;
                }
                if let Err(e) = tx_img.send(event.path) {
                    tracing::warn!(
                        name = "background_task.img_bus.tx_closed",
                        error = %e,
                        "Image bus subscriber could not deliver to tx_img. Channel is closed."
                    );
                    break;
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

        bus.publish(FileEvent::discovered(PathBuf::from("test.pdf")));

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

        bus.publish(FileEvent::discovered(PathBuf::from("test.md")));

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

        bus.publish(FileEvent::discovered(PathBuf::from("test.png")));

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

        bus.publish(FileEvent::discovered(PathBuf::from("test.pdf")));

        let path = rx_img.recv_timeout(std::time::Duration::from_millis(100));
        assert!(path.is_err());
    }
}
