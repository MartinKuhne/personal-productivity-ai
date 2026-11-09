//! Background subsystem data types — `ImageJob` and `PdfConversionJob`.
//!
//! `LogCategory` and `BackgroundLogEntry` are defined in
//! [`crate::bus::events::messages`] so the `bus` module does not depend on
//! the application layer. They are re-exported here so the canonical
//! accessor `crate::background::LogCategory` keeps resolving for in-tree
//! call sites.
//!
//! ## `image-library` feature
//!
//! `ImageJob` is gated behind the `image-library` Cargo feature so
//! builds that opt out of the image-vision worker don't pull in the
//! type. The image-vision worker itself lives in
//! `crate::background::vision_processor`. `LogCategory`
//! (including the `ImageVision` variant) and
//! `crate::background::PdfConversionJob`
//! stay always compiled — the log filter chip for `Image Vision`
//! still renders in the UI even when no worker is emitting into it.
//!
//! Unit tests live in the sibling `models_tests.rs` sidecar.

pub(crate) use crate::bus::events::messages::{BackgroundLogEntry, LogCategory};

#[cfg(test)]
#[path = "models_tests.rs"]
mod tests;

#[cfg(feature = "image-library")]
#[derive(Debug, Clone)]
pub struct ImageJob {
    pub image_path: std::path::PathBuf,
    pub md_path: std::path::PathBuf,
}

#[cfg(feature = "image-library")]
impl ImageJob {
    pub fn new(image_path: std::path::PathBuf) -> Self {
        let mut md_path = image_path.clone();
        md_path.set_extension("md");
        Self {
            image_path,
            md_path,
        }
    }

    pub fn should_process(&self) -> bool {
        if !self.md_path.exists() {
            return true;
        }
        if let (Ok(img_meta), Ok(md_meta)) = (
            std::fs::metadata(&self.image_path),
            std::fs::metadata(&self.md_path),
        ) && let (Ok(img_time), Ok(md_time)) = (img_meta.modified(), md_meta.modified())
        {
            return img_time > md_time;
        }
        false
    }
}
