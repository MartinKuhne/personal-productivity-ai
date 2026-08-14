//! Background task orchestrator — spawns and owns all worker threads (watcher, indexer, PDF converter, vision processor, bus router).
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
use crate::bus::events::typed::BackgroundEvent;
use crate::bus::router::BusRouter;
use crate::config::AppConfig;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

pub struct Task {
    /// Receiver for typed background events. The UI drains this on
    /// every frame and dispatches by domain (Agent / Fs / Process).
    pub rx: Receiver<BackgroundEvent>,
    /// Sender handed to every background worker. Producers send
    /// typed [`BackgroundEvent`] values directly.
    pub tx: Sender<BackgroundEvent>,
    /// File-event bus shared with the watcher, indexer, PDF/vision
    /// workers, and the UI.
    pub file_event_bus: Bus<FileEvent>,
    /// Slot for the `notify::RecommendedWatcher` handle. The
    /// file-watcher thread writes the handle here when initial
    /// scan completes, then sends [`crate::bus::events::typed::FsEvent::Finished`]
    /// on the typed channel. The UI calls
    /// [`Task::take_finished_watcher`] after observing that event to
    /// take ownership.
    pub finished_watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
    #[cfg(feature = "vector-search")]
    pub vector_search_service: Arc<crate::app::background::VectorSearchService>,
    /// Cancellation signal shared with the indexer. Set to `true` via
    /// [`Task::cancel`] to ask the initial library scan to stop early.
    cancel: Arc<AtomicBool>,
}

impl Task {
    /// Build a background task that waits for a [`ConfigArrived`] event
    /// on `config_bus` before spawning its worker threads. The
    /// subscription is registered before this returns, so callers may
    /// publish any time afterwards and the spawned thread will
    /// observe the first arrival (or fall back to
    /// [`AppConfig::default`] if no event arrives within
    /// [`CONFIG_ARRIVAL_TIMEOUT`]).
    pub fn new(config_bus: Bus<ConfigArrived>) -> Self {
        let (tx, rx) = channel();
        let tx_clone = tx.clone();
        let file_event_bus = Bus::new();
        let bus_clone = file_event_bus.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        let finished_watcher = Arc::new(Mutex::new(None));
        #[cfg(feature = "vector-search")]
        let vector_search_service = Arc::new(crate::app::background::VectorSearchService::new());

        // Subscribe before spawning so the thread's reader is in place
        // by the time the caller publishes.
        let config_reader = config_bus.subscribe();

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
                finished_watcher,
                #[cfg(feature = "vector-search")]
                vector_search_service.clone(),
            );
        });

        Self {
            rx,
            tx,
            file_event_bus,
            finished_watcher: Arc::new(Mutex::new(None)),
            #[cfg(feature = "vector-search")]
            vector_search_service,
            cancel,
        }
    }

    /// Build a background task whose workers start immediately, using
    /// the supplied configuration. This is a convenience for tests
    /// that do not need the bus-driven init path.
    #[doc(hidden)]
    pub fn new_for_test(config: AppConfig) -> Self {
        // Subscribe before publishing so the broadcast delivery
        // matches the event. `Task::new` registers the subscription
        // during construction; we publish after it returns.
        let bus = Bus::new();
        let task = Self::new(bus.clone());
        bus.publish(ConfigArrived::new(config));
        task
    }

    /// Signal the initial library scan to stop. The watcher and the
    /// post-scan workers are not affected (they have their own
    /// shutdown paths). Calling this after the scan has already
    /// completed is a no-op.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Take the `notify::RecommendedWatcher` handle, if one was
    /// written by the file-watcher thread. Returns `None` if the
    /// watcher has not initialized yet, or if it has already been
    /// taken. The UI calls this after observing
    /// [`crate::bus::events::typed::FsEvent::Finished`] on the typed
    /// background-event channel.
    pub fn take_finished_watcher(&self) -> Option<notify::RecommendedWatcher> {
        self.finished_watcher
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
    }

    fn run_indexing(
        config: crate::config::AppConfig,
        tx: Sender<BackgroundEvent>,
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
        // Image-vision channel + worker are only spun up when the
        // `image-library` Cargo feature is enabled. Without it the
        // channel is dropped immediately and the `BusRouter`'s
        // image-routing branch is a no-op (the field is gated in
        // `bus/router/bus_router.rs`).
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
            config.clone(),
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

// ---------------------------------------------------------------------------
// Tests live in the sibling `background_task_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "background_task_tests.rs"]
mod tests;
