use crate::messages::BackgroundMessage;
use notify::Watcher;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct Task {
    pub rx: Receiver<BackgroundMessage>,
    pub tx: Sender<BackgroundMessage>,
    pub _watcher: Option<notify::RecommendedWatcher>,
}

impl Task {
    pub fn new(config: crate::config::AppConfig) -> Self {
        let (tx, rx) = channel();
        let tx_clone = tx.clone();
        
        let config_clone = config.clone();
        std::thread::spawn(move || {
            Self::run_indexing(config_clone, tx_clone);
        });

        Self {
            rx,
            tx,
            _watcher: None,
        }
    }

    fn run_indexing(config: crate::config::AppConfig, tx: Sender<BackgroundMessage>) {
        let (tx_work, rx_work) = channel::<PathBuf>();
        let rx_work = std::sync::Arc::new(std::sync::Mutex::new(rx_work));

        let (tx_pdf, rx_pdf) = channel::<PathBuf>();
        let cmd_template = config.pdf_converter_command.clone();
        let tx_gui_pdf = tx.clone();
        std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    while let Ok(path) = rx_pdf.recv() {
                        let job = crate::background::PdfConversionJob::new(path);
                        if job.should_convert() {
                            let _ = job.execute(cmd_template.clone(), tx_gui_pdf.clone()).await;
                        }
                    }
                });
            }
        });

        let (tx_img, rx_img) = channel::<PathBuf>();
        let tx_gui_img = tx.clone();
        let config_img = config.clone();
        std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    while let Ok(path) = rx_img.recv() {
                        let job = crate::background::models::ImageJob::new(path);
                        if job.should_process() {
                            let _ = crate::background::vision_processor::process_image(job, config_img.clone(), tx_gui_img.clone()).await;
                        }
                    }
                });
            }
        });

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
                    std::thread::yield_now();
                }
            });
            workers.push(handle);
        }

        let mut files_scanned = 0;
        let mut pdfs_queued = 0;
        let mut images_queued = 0;
        let mut last_log_time = std::time::Instant::now();

        for lib in &config.content_libraries {
            let is_image_lib = lib.kind == "image";
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
                                let _ = tx_pdf.send(path.to_path_buf());
                            }
                        } else if matches!(ext_str.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "avif") {
                            if is_image_lib {
                                let job = crate::background::models::ImageJob::new(path.to_path_buf());
                                if job.should_process() {
                                    images_queued += 1;
                                    let _ = tx_img.send(path.to_path_buf());
                                }
                            }
                        }
                    }
                } else if path.is_dir() {
                    let _ = tx.send(BackgroundMessage::DirParsed { path: path.to_path_buf() });
                }
                
                if files_scanned % 500 == 0 || last_log_time.elapsed().as_secs() >= 5 {
                    let _ = tx.send(BackgroundMessage::LogEntry(crate::background::BackgroundLogEntry::new(
                        crate::background::LogCategory::Indexer,
                        format!("Scanned {} files, queued {} PDFs, queued {} images", files_scanned, pdfs_queued, images_queued)
                    )));
                    last_log_time = std::time::Instant::now();
                }
                if files_scanned % 50 == 0 {
                    std::thread::yield_now();
                }
            }
        }
        
        let _ = tx.send(BackgroundMessage::LogEntry(crate::background::BackgroundLogEntry::new(
            crate::background::LogCategory::Indexer,
            format!("Initial indexing complete. Scanned {} files, queued {} PDFs, queued {} images.", files_scanned, pdfs_queued, images_queued)
        )));

        drop(tx_work);

        for worker in workers {
            let _ = worker.join();
        }

        let tx_notify = tx.clone();
        let config_watcher = config.clone();
        let tx_pdf_watcher = tx_pdf.clone();
        let tx_img_watcher = tx_img.clone();
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
                    
                    let mut is_image_lib = false;
                    for lib in &config_watcher.content_libraries {
                        let lib_path = std::path::PathBuf::from(&lib.root_folder);
                        if lib.kind == "image" && path.starts_with(&lib_path) {
                            is_image_lib = true;
                            break;
                        }
                    }
                    
                    let mut is_md = false;
                    let mut is_pdf = false;
                    let mut is_img = false;
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_string_lossy().to_lowercase();
                        if ext_str == "md" || ext_str == "markdown" || ext_str == "txt" {
                            is_md = true;
                        } else if ext_str == "pdf" {
                            is_pdf = true;
                        } else if matches!(ext_str.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "avif") {
                            if is_image_lib {
                                is_img = true;
                            }
                        }
                    }

                    if is_md || is_pdf || is_img {
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
                                    let _ = tx_pdf_watcher.send(path.clone());
                                }
                            }
                            _ => {}
                        }
                    } else if is_img {
                        match event.kind {
                            notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                                let job = crate::background::models::ImageJob::new(path.clone());
                                if job.should_process() {
                                    let _ = tx_img_watcher.send(path.clone());
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
            for lib in &config.content_libraries {
                let root_path = PathBuf::from(&lib.root_folder);
                let _ = watcher.watch(&root_path, notify::RecursiveMode::Recursive);
            }
            let _ = tx.send(BackgroundMessage::Finished(watcher));
        } else {
            let _ = tx.send(BackgroundMessage::FinishedWithoutWatcher);
        }
    }
}