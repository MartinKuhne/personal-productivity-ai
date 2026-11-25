//! File-event bus router — subscribes to bus events and routes newly
//! discovered files to PDF/vision worker queues by extension.
//!
//! Unit tests live in the sibling `bus_router_tests.rs` sidecar.

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
#[path = "bus_router_tests.rs"]
mod tests;
