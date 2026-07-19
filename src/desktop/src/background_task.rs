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
    pub fn new(libraries: Vec<crate::config::ContentLibrary>) -> Self {
        let (tx, rx) = channel();
        let tx_clone = tx.clone();
        
        std::thread::spawn(move || {
            Self::run_indexing(libraries, tx_clone);
        });

        Self {
            rx,
            tx,
            _watcher: None,
        }
    }

    fn run_indexing(libraries: Vec<crate::config::ContentLibrary>, tx: Sender<BackgroundMessage>) {
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

        for lib in &libraries {
            let root_path = PathBuf::from(&lib.root_folder);
            let walker = walkdir::WalkDir::new(&root_path)
                .into_iter()
                .filter_entry(|e| e.file_name() != ".git");

            for entry in walker.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "md" || ext == "markdown" || ext == "txt" {
                            let _ = tx_work.send(path.to_path_buf());
                        }
                    }
                } else if path.is_dir() {
                    let _ = tx.send(BackgroundMessage::DirParsed { path: path.to_path_buf() });
                }
            }
        }

        drop(tx_work);

        for worker in workers {
            let _ = worker.join();
        }

        let tx_notify = tx.clone();
        let watcher_result = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                for path in event.paths {
                    if path.components().any(|c| c.as_os_str() == ".git") {
                        continue;
                    }
                    let mut is_md = false;
                    if let Some(ext) = path.extension() {
                        if ext == "md" || ext == "markdown" || ext == "txt" {
                            is_md = true;
                        }
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