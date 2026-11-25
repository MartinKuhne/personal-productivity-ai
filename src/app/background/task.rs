//! Background task orchestrator for watcher, indexing, conversion, vision, and routing workers.
//!
//! Unit tests live in the sibling `task_tests.rs` sidecar.

use super::indexer::Indexer;
use super::pdf_converter::PdfConverterWorker;
use super::tag_manager::TagManager;
#[cfg(feature = "image-library")]
use super::vision_processor::ImageVisionWorker;
use crate::bus::config::CONFIG_ARRIVAL_TIMEOUT;
use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::directory::DirectoryChanged;
use crate::bus::events::file::FileChanged;
use crate::bus::events::typed::{BackgroundEvent, BackgroundEventSender};
use crate::bus::router::BusRouter;
use crate::config::AppConfig;
use crate::workspace::watcher::FileWatcher;
use futures::channel::mpsc::UnboundedReceiver;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

const TAG_FLUSH_FAILURE_MESSAGE: &str =
    "Tag extraction did not complete before indexing finished; some tags may be missing.";

/// Owns the background worker threads and their event channel.
pub struct Task {
    /// Broadcast sender handed to every background worker.
    pub tx: BackgroundEventSender,
    /// Futures-compatible receiver for the iced subscription bridge.
    /// This is fed by a tokio bridge task that forwards from the broadcast channel.
    pub rx_futures: UnboundedReceiver<BackgroundEvent>,
    /// File-event bus — payload `[FileChanged]`, shared by background workers
    /// and the UI ([REQ-330..334]).
    pub file_event_bus: Bus<FileChanged>,
    /// Directory-event bus — payload `[DirectoryChanged]`, shared by the
    /// crawler/watcher and the UI ([REQ-320..322]).
    pub dir_event_bus: Bus<DirectoryChanged>,
    /// Slot containing the watcher after initial scan completion.
    pub finished_watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
    #[cfg(feature = "vector-search")]
    /// Shared vector-search service.
    pub vector_search_service: Arc<crate::background::VectorSearchService>,
    cancel: Arc<AtomicBool>,
}

impl Task {
    /// Wait for configuration, then spawn the background workers.
    pub fn new(config_bus: Bus<ConfigArrived>) -> Self {
        // Use broadcast channel like other buses (ConfigArrived, FileChanged, DirectoryChanged, AgentEvent)
        let (tx_broadcast, rx_broadcast) = broadcast::channel(1024);
        let tx = BackgroundEventSender::new(tx_broadcast);
        let tx_clone = tx.clone();
        let file_event_bus: Bus<FileChanged> = Bus::new();
        let dir_event_bus: Bus<DirectoryChanged> = Bus::new();
        let file_bus_clone = file_event_bus.clone();
        let dir_bus_clone = dir_event_bus.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        let finished_watcher = Arc::new(Mutex::new(None));
        #[cfg(feature = "vector-search")]
        let vector_search_service = Arc::new(crate::background::VectorSearchService::new());

        let config_reader = config_bus.subscribe();
        let finished_watcher_for_worker = finished_watcher.clone();
        #[cfg(feature = "vector-search")]
        let vector_search_service_for_thread = vector_search_service.clone();

        // Create a futures-compatible channel for the iced subscription bridge.
        // A tokio bridge task will forward events from the broadcast channel to this sender.
        let (futures_tx, futures_rx) = futures::channel::mpsc::unbounded::<BackgroundEvent>();
        let futures_tx_for_bridge = futures_tx.clone();

        // Spawn the tokio bridge task that forwards from broadcast to futures channel.
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime for broadcast->futures bridge");
            rt.block_on(async move {
                let mut rx = rx_broadcast;
                while let Ok(event) = rx.recv().await {
                    if futures_tx_for_bridge.unbounded_send(event).is_err() {
                        // Receiver dropped, bridge can stop
                        break;
                    }
                }
            });
        });

        std::thread::spawn(move || {
            #[cfg(feature = "vector-search")]
            let vector_search_service = vector_search_service_for_thread;
            let config = match config_reader.recv_timeout(CONFIG_ARRIVAL_TIMEOUT) {
                Ok(event) => {
                    tracing::info!(
                        name = "config.arrived",
                        "Background task received configuration, spawning workers"
                    );
                    event.config
                }
                Err(_) => {
                    tracing::error!(
                        name = "config.arrived.timeout",
                        timeout_ms = CONFIG_ARRIVAL_TIMEOUT.as_millis() as u64,
                        "No ConfigArrived event observed within timeout; using default configuration"
                    );
                    AppConfig::default()
                }
            };
            Self::run_indexing(
                config,
                tx_clone,
                file_bus_clone,
                dir_bus_clone,
                cancel_clone,
                finished_watcher_for_worker,
                #[cfg(feature = "vector-search")]
                vector_search_service,
            );
        });

        Self {
            tx,
            rx_futures: futures_rx,
            file_event_bus,
            dir_event_bus,
            finished_watcher,
            #[cfg(feature = "vector-search")]
            vector_search_service,
            cancel,
        }
    }

    /// Build a background task using the bus-driven initialization path.
    #[doc(hidden)]
    pub fn new_for_test(config: AppConfig) -> Self {
        let bus = Bus::new();
        let task = Self::new(bus.clone());
        bus.publish(ConfigArrived::new(config));
        task
    }

    /// Signal the initial library scan to stop.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Take the watcher handle after it has been initialized.
    pub fn take_finished_watcher(&self) -> Option<notify::RecommendedWatcher> {
        self.finished_watcher
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
    }

    /// Provide a std::sync::mpsc receiver for test compatibility.
    /// Spawns a bridge thread that forwards from the broadcast receiver.
    pub fn rx_std(&self) -> Receiver<BackgroundEvent> {
        let (std_tx, std_rx) = channel::<BackgroundEvent>();
        let mut rx = self.tx.subscribe();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime for bridge");
            rt.block_on(async move {
                while let Ok(event) = rx.recv().await {
                    if std_tx.send(event).is_err() {
                        break;
                    }
                }
            });
        });
        std_rx
    }

    fn run_indexing(
        config: crate::config::AppConfig,
        tx: BackgroundEventSender,
        file_event_bus: Bus<FileChanged>,
        dir_event_bus: Bus<DirectoryChanged>,
        cancel: Arc<AtomicBool>,
        finished_watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
        #[cfg(feature = "vector-search")] vector_search_service: Arc<
            crate::background::VectorSearchService,
        >,
    ) {
        #[cfg(feature = "vector-search")]
        vector_search_service.start(config.clone(), file_event_bus.subscribe(), tx.clone());

        let (tx_pdf, rx_pdf) = channel::<PathBuf>();
        #[cfg(feature = "image-library")]
        let (tx_img, rx_img) = channel::<PathBuf>();

        let cmd_template = config.pdf_converter_command.clone();
        let pdf_converter_configured = cmd_template.is_some();
        PdfConverterWorker::new(rx_pdf, tx.clone(), file_event_bus.clone(), cmd_template).spawn();
        #[cfg(feature = "image-library")]
        ImageVisionWorker::new(rx_img, tx.clone(), config.clone(), file_event_bus.clone()).spawn();

        let tag_manager =
            TagManager::new(file_event_bus.clone(), tx.clone(), config.clone()).spawn();
        #[cfg(feature = "image-library")]
        BusRouter::new_pdf_only(
            file_event_bus.clone(),
            tx_pdf.clone(),
            pdf_converter_configured,
            tx_img.clone(),
        )
        .spawn();
        #[cfg(not(feature = "image-library"))]
        BusRouter::new_pdf_only(
            file_event_bus.clone(),
            tx_pdf.clone(),
            pdf_converter_configured,
        )
        .spawn();

        let indexer = Indexer::new(
            config.clone(),
            tx.clone(),
            file_event_bus.clone(),
            dir_event_bus.clone(),
            cancel,
        );
        #[cfg(feature = "image-library")]
        indexer.scan_libraries(&tx_img);
        #[cfg(not(feature = "image-library"))]
        indexer.scan_libraries();
        let mut file_watcher = FileWatcher::new(
            config,
            tx.clone(),
            file_event_bus.clone(),
            dir_event_bus.clone(),
            #[cfg(feature = "image-library")]
            tx_img.clone(),
            finished_watcher,
        );
        #[cfg(feature = "image-library")]
        BusRouter::new_images_only(file_event_bus, tx_pdf, tx_img).spawn();
        finish_initial_indexing(|| tag_manager.flush(), &tx, || file_watcher.start());
    }
}

fn finish_initial_indexing(
    flush_tags: impl FnOnce() -> bool,
    tx: &BackgroundEventSender,
    start_watcher: impl FnOnce(),
) -> bool {
    let tags_flushed = flush_tags();
    if !tags_flushed {
        tracing::error!(
            name = "background.indexer.tag_flush_failed",
            "Initial scan finished without confirming complete tag extraction; the background process log records degraded indexing."
        );
        if let Err(error) = tx.send(
            crate::background::BackgroundLogEntry::new(
                crate::background::LogCategory::Indexer,
                TAG_FLUSH_FAILURE_MESSAGE.to_string(),
            )
            .into(),
        ) {
            tracing::error!(
                name = "background.indexer.tag_flush_report_failed",
                error = %error,
                "Could not report degraded initial tag extraction."
            );
        }
    }

    // A failed tag flush is degraded indexing, not a reason to skip watcher
    // startup or suppress the normal indexing-finished event.
    start_watcher();
    tags_flushed
}

#[cfg(test)]
#[path = "task_tests.rs"]
mod tests;
