//! File-event bus router — subscribes to bus events and routes newly
//! discovered files to PDF/vision worker queues by extension.

use crate::background::PdfConversionJob;
use crate::bus::core::Bus;
use crate::bus::events::file::{FileChanged, FileChangedKind};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Subscribes to a [`Bus<FileChanged>`] and fans out newly discovered /
/// updated files to per-format worker channels by extension.
pub struct BusRouter {
    bus: Bus<FileChanged>,
    tx_pdf: Sender<PathBuf>,
    pdf_converter_configured: bool,
    route_pdf: bool,
    /// Image-vision sender. Only present when the `image-library`
    /// Cargo feature is enabled; the image-routing branch below
    /// compiles out without it.
    #[cfg(feature = "image-library")]
    tx_img: Sender<PathBuf>,
    #[cfg(feature = "image-library")]
    route_images: bool,
}

impl BusRouter {
    /// The `tx_img` parameter is only present when the
    /// `image-library` Cargo feature is enabled.
    pub fn new(
        bus: Bus<FileChanged>,
        tx_pdf: Sender<PathBuf>,
        #[cfg(feature = "image-library")] tx_img: Sender<PathBuf>,
    ) -> Self {
        Self {
            bus,
            tx_pdf,
            pdf_converter_configured: true,
            route_pdf: true,
            #[cfg(feature = "image-library")]
            tx_img,
            #[cfg(feature = "image-library")]
            route_images: true,
        }
    }

    /// Creates the production PDF manager, which listens before initial
    /// scanning starts and does not duplicate the image watcher's direct queue.
    #[cfg(feature = "image-library")]
    pub(crate) fn new_pdf_only(
        bus: Bus<FileChanged>,
        tx_pdf: Sender<PathBuf>,
        pdf_converter_configured: bool,
        tx_img: Sender<PathBuf>,
    ) -> Self {
        Self {
            bus,
            tx_pdf,
            pdf_converter_configured,
            route_pdf: true,
            tx_img,
            route_images: false,
        }
    }

    /// Creates the post-scan image router without duplicating PDF jobs,
    /// which are owned by the pre-scan PDF manager.
    #[cfg(feature = "image-library")]
    pub(crate) fn new_images_only(
        bus: Bus<FileChanged>,
        tx_pdf: Sender<PathBuf>,
        tx_img: Sender<PathBuf>,
    ) -> Self {
        Self {
            bus,
            tx_pdf,
            pdf_converter_configured: false,
            route_pdf: false,
            tx_img,
            route_images: true,
        }
    }

    /// Creates the production PDF manager, which listens before initial
    /// scanning starts.
    #[cfg(not(feature = "image-library"))]
    pub(crate) fn new_pdf_only(
        bus: Bus<FileChanged>,
        tx_pdf: Sender<PathBuf>,
        pdf_converter_configured: bool,
    ) -> Self {
        Self {
            bus,
            tx_pdf,
            pdf_converter_configured,
            route_pdf: true,
        }
    }

    /// Starts routing. The bus subscription is registered synchronously before
    /// this method returns so scan-time events are not missed.
    pub fn spawn(self) {
        let bus = self.bus.clone();
        let tx_pdf = self.tx_pdf;
        let pdf_converter_configured = self.pdf_converter_configured;
        let route_pdf = self.route_pdf;
        #[cfg(feature = "image-library")]
        let tx_img = self.tx_img;
        #[cfg(feature = "image-library")]
        let route_images = self.route_images;

        // Register synchronously, before handing the future to the runtime.
        // This is required because the initial crawler can publish immediately.
        let mut reader = bus.subscribe_async();

        let router_task = async move {
            let mut pdf_open = route_pdf;
            #[cfg(feature = "image-library")]
            let mut img_open = route_images;

            loop {
                let event = match reader.recv().await {
                    Ok(event) => event,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                        tracing::warn!(
                            name = "file_bus_router.lagged",
                            lagged_events = count,
                            "The file bus router lagged; it will resume with the next event."
                        );
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                };
                #[cfg(feature = "image-library")]
                if !pdf_open && !img_open {
                    break;
                }
                #[cfg(not(feature = "image-library"))]
                if !pdf_open {
                    break;
                }

                // [REQ-330..332]: route file events with kind Added/Changed;
                // deletions are not conversion work. Directory events ride
                // the separate directory bus and never reach this router.
                let paths = match &event {
                    FileChanged {
                        kind: FileChangedKind::Added | FileChangedKind::Changed,
                        files,
                    } => files.iter().map(|f| f.path.clone()).collect::<Vec<_>>(),
                    _ => continue,
                };

                for p in &paths {
                    let ext = p
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase());
                    let ext_str = ext.as_deref().unwrap_or("");

                    if ext_str == "pdf" {
                        let job = PdfConversionJob::new(p.clone());
                        if pdf_open
                            && route_pdf
                            && pdf_converter_configured
                            && job.should_convert()
                            && let Err(error) = tx_pdf.send(p.clone())
                        {
                            tracing::warn!(
                                name = "background_task.pdf_bus.tx_closed",
                                error = %error,
                                "PDF manager could not deliver a conversion job because its channel is closed."
                            );
                            pdf_open = false;
                        }
                        continue;
                    }

                    #[cfg(feature = "image-library")]
                    if route_images
                        && img_open
                        && matches!(
                            ext_str,
                            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "avif"
                        )
                        && let Err(error) = tx_img.send(p.clone())
                    {
                        tracing::warn!(
                            name = "background_task.img_bus.tx_closed",
                            error = %error,
                            "Image bus subscriber could not deliver to tx_img. Channel is closed."
                        );
                        img_open = false;
                    }
                }
            }
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(router_task);
        } else {
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    rt.block_on(router_task);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
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
}
