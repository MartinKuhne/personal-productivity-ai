//! OneDrive item tree model + the markdown filter that turns a raw
//! `/me/drive/.../children` response into something the UI can render.
//!
//! This is a faithful port of `FileNode.kt` + `FileTreeProcessor` from the
//! Kotlin/Compose app in `src/android/`. The four regression tests in
//! `tests/file_tree_processor.rs` were translated from `AppTest.kt` and must
//! stay green.

use serde::{Deserialize, Serialize};

/// One item in the OneDrive tree, recursive.
///
/// `download_url` is the pre-authenticated `@microsoft.graph.downloadUrl`
/// returned by the Graph API; it's only populated for files. The URL is
/// short-lived (typically one hour) so we refetch on every tree load rather
/// than caching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileNode {
    pub id: String,
    pub name: String,
    pub is_directory: bool,
    #[serde(default)]
    pub children: Vec<FileNode>,
    pub download_url: Option<String>,
}

impl FileNode {
    pub fn new(id: impl Into<String>, name: impl Into<String>, is_directory: bool) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            is_directory,
            children: Vec::new(),
            download_url: None,
        }
    }

    /// Build the synthetic root node the Graph API responses attach children
    /// to. We don't get a real `id` for the root from the API, so we
    /// synthesize one for tree-keying purposes.
    pub fn synthetic_root(display_name: impl Into<String>) -> Self {
        Self::new("root", display_name, true)
    }
}

/// Applies the same filtering and sorting rules as the Kotlin
/// `FileTreeProcessor`:
/// 1. Files that do not end in `.md` (case-insensitive) are dropped.
/// 2. Directories that contain no markdown files (after their own
///    children are filtered) are dropped.
/// 3. Within a directory, children are sorted: directories first, then files,
///    each group sorted alphabetically (case-insensitive).
pub struct FileTreeProcessor;

impl FileTreeProcessor {
    /// Recursively process a tree. Returns `None` if the root ends up empty
    /// after filtering, mirroring the Kotlin `processTree` contract.
    pub fn process_tree(root: FileNode) -> Option<FileNode> {
        if !root.is_directory {
            return if is_markdown(&root.name) {
                Some(root)
            } else {
                None
            };
        }

        let mut processed_children: Vec<FileNode> = root
            .children
            .into_iter()
            .filter_map(Self::process_tree)
            .collect();

        if processed_children.is_empty() {
            return None;
        }

        processed_children.sort_by(|a, b| {
            // Directories first.
            b.is_directory
                .cmp(&a.is_directory)
                // Then alphabetical, case-insensitive.
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        Some(FileNode {
            id: root.id,
            name: root.name,
            is_directory: true,
            children: processed_children,
            download_url: None,
        })
    }
}

fn is_markdown(name: &str) -> bool {
    name.rsplit_once('.')
        .map(|(_, ext)| ext.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}
