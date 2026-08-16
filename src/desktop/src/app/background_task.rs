//! Background task orchestrator for watcher, indexing, conversion, vision, and routing workers.
//!
//! Unit tests live in the sibling `background_task_tests.rs` sidecar.

use crate::app::background::indexer::Indexer;
use crate::app::background::pdf_converter::PdfConverterWorker;
#[cfg(feature = "image-library")]
use crate::app::background::vision_processor::ImageVisionWorker;
use crate::app::watcher::file_watcher::FileWatcher;
use crate::bus::config::CONFIG_ARRIVAL_TIMEOUT;
use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::file::FileEvent;
use crate::bus::events::typed::{BackgroundEvent, BackgroundEventSender};
use crate::bus::router::BusRouter;
use crate::config::AppConfig;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

/// Owns the background worker threads and their event channel.
pub struct Task {
    /// Receiver for typed background events drained by the UI.
    pub rx: Receiver<BackgroundEvent>,
    /// Sender handed to every background worker.
    pub tx: BackgroundEventSender,
    /// File-event bus shared by background workers and the UI.
    pub file_event_bus: Bus<FileEvent>,
    /// Slot containing the watcher after initial scan completion.
    pub finished_watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
    #[cfg(feature = "vector-search")]
    /// Shared vector-search service.
    pub vector_search_service: Arc<crate::app::background::VectorSearchService>,
    cancel: Arc<AtomicBool>,
}

impl Task {
    /// Wait for configuration, then spawn the background workers.
    pub fn new(config_bus: Bus<ConfigArrived>) -> Self {
        let (raw_tx, rx) = channel();
        let tx = BackgroundEventSender::new(raw_tx);
        let tx_clone = tx.clone();
        let file_event_bus = Bus::new();
        let bus_clone = file_event_bus.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        let finished_watcher = Arc::new(Mutex::new(None));
        #[cfg(feature = "vector-search")]
        let vector_search_service = Arc::new(crate::app::background::VectorSearchService::new());

        let config_reader = config_bus.subscribe();
        let finished_watcher_for_worker = finished_watcher.clone();
        #[cfg(feature = "vector-search")]
        let vector_search_service_for_thread = vector_search_service.clone();

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
                bus_clone,
                cancel_clone,
                finished_watcher_for_worker,
                #[cfg(feature = "vector-search")]
                vector_search_service,
            );
        });

        Self {
            rx,
            tx,
            file_event_bus,
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

    fn run_indexing(
        config: crate::config::AppConfig,
        tx: BackgroundEventSender,
        file_event_bus: Bus<FileEvent>,
        cancel: Arc<AtomicBool>,
        finished_watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
        #[cfg(feature = "vector-search")] vector_search_service: Arc<
            crate::app::background::VectorSearchService,
        >,
    ) {
        #[cfg(feature = "vector-search")]
        vector_search_service.start(config.clone(), file_event_bus.subscribe(), tx.clone());

        let (tx_work, rx_work) = channel::<PathBuf>();
        let rx_work = Arc::new(Mutex::new(rx_work));
        let (tx_pdf, rx_pdf) = channel::<PathBuf>();
        #[cfg(feature = "image-library")]
        let (tx_img, rx_img) = channel::<PathBuf>();

        let cmd_template = config.pdf_converter_command.clone();
        PdfConverterWorker::new(rx_pdf, tx.clone(), file_event_bus.clone(), cmd_template).spawn();
        #[cfg(feature = "image-library")]
        ImageVisionWorker::new(rx_img, tx.clone(), config.clone(), file_event_bus.clone()).spawn();

        let workers = Indexer::spawn_workers(4, rx_work, tx.clone());
        let indexer = Indexer::new(config.clone(), tx.clone(), file_event_bus.clone(), cancel);
        #[cfg(feature = "image-library")]
        indexer.scan_libraries(&tx_work, &tx_pdf, &tx_img);
        #[cfg(not(feature = "image-library"))]
        indexer.scan_libraries(&tx_work, &tx_pdf);

        drop(tx_work);
        for worker in workers {
            let _ = worker.join();
        }

        let mut file_watcher = FileWatcher::new(
            config,
            tx.clone(),
            file_event_bus.clone(),
            tx_pdf.clone(),
            #[cfg(feature = "image-library")]
            tx_img.clone(),
            finished_watcher,
        );
        file_watcher.start();

        #[cfg(feature = "image-library")]
        BusRouter::new(file_event_bus.clone(), tx_pdf, tx_img).spawn();
        #[cfg(not(feature = "image-library"))]
        BusRouter::new(file_event_bus.clone(), tx_pdf).spawn();
    }
}

#[cfg(test)]
#[path = "background_task_tests.rs"]
mod tests;
