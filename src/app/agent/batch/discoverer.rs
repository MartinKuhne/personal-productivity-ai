//! Job discoverer implementations — resolves which paths a batch job should process (files or directories).
//!
//! Unit tests live in the sibling `discoverer_tests.rs` sidecar.

use super::types::{BatchConfig, BatchMode};
use std::path::PathBuf;

pub struct FileMatcherDiscoverer {
    pub directory: PathBuf,
    pub pattern: String,
}

impl FileMatcherDiscoverer {
    fn discover(&self) -> Result<Vec<PathBuf>, String> {
        super::file_matcher::find_matching_files(&self.directory, &self.pattern)
    }
}

pub struct DirectoryDiscoverer {
    pub directory: PathBuf,
}

impl DirectoryDiscoverer {
    fn discover(&self) -> Result<Vec<PathBuf>, String> {
        Ok(super::file_matcher::find_subdirectories(&self.directory))
    }
}

pub enum Discoverer {
    File(FileMatcherDiscoverer),
    Directory(DirectoryDiscoverer),
}

impl Discoverer {
    pub fn discover(&self) -> Result<Vec<PathBuf>, String> {
        match self {
            Self::File(d) => d.discover(),
            Self::Directory(d) => d.discover(),
        }
    }

    pub fn from_config(config: &BatchConfig) -> Self {
        match config.mode {
            BatchMode::File => Self::File(FileMatcherDiscoverer {
                directory: config.directory.clone(),
                pattern: config.pattern.clone(),
            }),
            BatchMode::Directory => Self::Directory(DirectoryDiscoverer {
                directory: config.directory.clone(),
            }),
        }
    }
}

#[cfg(test)]
#[path = "discoverer_tests.rs"]
mod tests;
