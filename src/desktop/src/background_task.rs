use crate::messages::BackgroundMessage;
use notify::Watcher;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct BackgroundTask {
    pub rx: Receiver<BackgroundMessage>,
    pub tx: Sender<BackgroundMessage>,
    pub _watcher: Option<notify::RecommendedWatcher>,
}

impl BackgroundTask {
    pub fn new(config: crate::config::AppConfig) -> Self {
        let (tx, rx) = channel();
        let tx_clone = tx.clone();
        
        std::thread::spawn(move || {
            Self::run_indexing(config.content_libraries, config.pdf_converter_command, tx_clone);
        });

        Self {
            rx,
            tx,
            _watcher: None,
        }
    }

    fn run_indexing(libraries: Vec<crate::config::ContentLibrary>, pdf_command: Option<Vec<String>>, tx: Sender<BackgroundMessage>) {
        let (tx_work, rx_work) = channel::<PathBuf>();
        let rx_work = std::sync::Arc::new(std::sync::Mutex::new(rx_work));

        let mut workers = Vec::new();
        for _ in 0..4 {
            let rx_work_clone = rx_work.clone();
            let tx_gui_clone = tx.clone();
            let handle = std::thread::spawn(move || {
                loop {
                    let path = {
                        let rx = rx_work_clone.lock().unwrap();
                        match rx.recv() {
                            Ok(p) => p,
                            Err(_) => break,
                        }
                    };

                    let tags = crate::utils::tags::extract_tags_from_file(&path);
                    let _ = tx_gui_clone.send(BackgroundMessage::FileParsed { path, tags });
                }
            });
            workers.push(handle);
        }

        let mut files_scanned = 0;
        let mut pdfs_queued = 0;
        let mut last_log_time = std::time::Instant::now();

        for lib in &libraries {
            let root_path = PathBuf::from(&lib.root_folder);
            let walker = walkdir::WalkDir::new(&root_path)
                .into_iter()
                .filter_entry(|e| e.file_name() != ".git");

            for entry in walker.filter_map(|e| e.ok()) {
                files_scanned += 1;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_string_lossy().to_lowercase();
                        if ext_str == "md" || ext_str == "markdown" || ext_str == "txt" {
                            let _ = tx_work.send(path.to_path_buf());
                        } else if ext_str == "pdf" {
                            let job = crate::background::PdfConversionJob::new(path.to_path_buf());
                            if job.should_convert() {
                                pdfs_queued += 1;
                                let cmd = pdf_command.clone();
                                let tx_clone = tx.clone();
                                std::thread::spawn(move || {
                                    if let Ok(rt) = tokio::runtime::Runtime::new() {
                                        rt.block_on(async {
                                            let _ = job.execute(cmd, tx_clone).await;
                                        });
                                    }
                                });
                            }
                        }
                    }
                } else if path.is_dir() {
                    let _ = tx.send(BackgroundMessage::DirParsed { path: path.to_path_buf() });
                }
                
                if files_scanned % 500 == 0 || last_log_time.elapsed().as_secs() >= 5 {
                    let _ = tx.send(BackgroundMessage::LogEntry(crate::background::BackgroundLogEntry::new(
                        crate::background::LogCategory::Indexer,
                        format!("Scanned {} files, queued {} PDFs", files_scanned, pdfs_queued)
                    )));
                    last_log_time = std::time::Instant::now();
                }
            }
        }
        
        let _ = tx.send(BackgroundMessage::LogEntry(crate::background::BackgroundLogEntry::new(
            crate::background::LogCategory::Indexer,
            format!("Initial indexing complete. Scanned {} files, queued {} PDFs.", files_scanned, pdfs_queued)
        )));

        drop(tx_work);

        for worker in workers {
            let _ = worker.join();
        }

        let tx_notify = tx.clone();
        let pdf_command_watcher = pdf_command.clone();
        let watcher_result = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                for path in event.paths {
                    if path.components().any(|c| c.as_os_str() == ".git") {
                        continue;
                    }
                    
                    let event_type = match event.kind {
                        notify::EventKind::Create(_) => "created",
                        notify::EventKind::Modify(_) => "modified",
                        notify::EventKind::Remove(_) => "deleted",
                        _ => "changed",
                    };
                    
                    let mut is_md = false;
                    let mut is_pdf = false;
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_string_lossy().to_lowercase();
                        if ext_str == "md" || ext_str == "markdown" || ext_str == "txt" {
                            is_md = true;
                        } else if ext_str == "pdf" {
                            is_pdf = true;
                        }
                    }

                    if is_md || is_pdf {
                        let _ = tx_notify.send(BackgroundMessage::LogEntry(crate::background::BackgroundLogEntry::new(
                            crate::background::LogCategory::Watcher,
                            format!("File {} {:?}", event_type, path.file_name().unwrap_or_default())
                        )));
                    }

                    if is_md {
                        match event.kind {
                            notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                                if path.is_file() {
                                    let tags = crate::utils::tags::extract_tags_from_file(&path);
                                    let _ = tx_notify.send(
                                        BackgroundMessage::FileModified {
                                            path: path.clone(),
                                            tags,
                                        },
                                    );
                                }
                            }
                            notify::EventKind::Remove(_) => {
                                let _ =
                                    tx_notify.send(BackgroundMessage::FileDeleted {
                                        path: path.clone(),
                                    });
                            }
                            _ => {}
                        }
                    } else if is_pdf {
                        match event.kind {
                            notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                                let job = crate::background::PdfConversionJob::new(path.clone());
                                if job.should_convert() {
                                    let cmd = pdf_command_watcher.clone();
                                    let tx_c = tx_notify.clone();
                                    std::thread::spawn(move || {
                                        if let Ok(rt) = tokio::runtime::Runtime::new() {
                                            rt.block_on(async {
                                                let _ = job.execute(cmd, tx_c).await;
                                            });
                                        }
                                    });
                                }
                            }
                            _ => {}
                        }
                    } else if !path.exists() {
                        let _ = tx_notify
                            .send(BackgroundMessage::FileDeleted { path: path.clone() });
                    }
                }
            }
        });

        if let Ok(mut watcher) = watcher_result {
            for lib in &libraries {
                let root_path = PathBuf::from(&lib.root_folder);
                let _ = watcher.watch(&root_path, notify::RecursiveMode::Recursive);
            }
            let _ = tx.send(BackgroundMessage::Finished(watcher));
        } else {
            let _ = tx.send(BackgroundMessage::FinishedWithoutWatcher);
        }
    }
}