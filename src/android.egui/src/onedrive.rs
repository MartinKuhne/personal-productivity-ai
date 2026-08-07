//! Microsoft Graph client for OneDrive.
//!
//! Faithful port of `OneDriveManager.kt`. The blocking `ureq` agent replaces
//! OkHttp; the JSON shape and pagination handling are identical. Filtering
//! and sorting live in [`crate::file_node::FileTreeProcessor`] (where the
//! Kotlin app puts them) so the wire-format types stay separate from the
//! processed types the UI consumes.

use serde::Deserialize;
use url::Url;

use crate::error::{AppError, AppResult};
use crate::file_node::FileNode;

/// One item returned by the Graph API. We only deserialize the fields we
/// actually use; `serde` ignores the rest.
#[derive(Debug, Deserialize)]
struct GraphItem {
    id: String,
    name: String,
    #[serde(default)]
    folder: Option<serde_json::Value>,
    #[serde(rename = "@microsoft.graph.downloadUrl", default)]
    download_url: Option<String>,
}

impl GraphItem {
    fn into_node(self) -> FileNode {
        let is_directory = self.folder.is_some();
        let mut node = FileNode::new(self.id, self.name, is_directory);
        node.download_url = self.download_url;
        node
    }
}

#[derive(Debug, Deserialize)]
struct GraphChildrenResponse {
    #[serde(default)]
    value: Vec<GraphItem>,
    #[serde(rename = "@odata.nextLink", default)]
    next_link: Option<String>,
}

pub struct OneDriveClient {
    access_token: String,
    http: ureq::Agent,
}

impl OneDriveClient {
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            http: ureq::AgentBuilder::new()
                .timeout_connect(std::time::Duration::from_secs(10))
                .timeout_read(std::time::Duration::from_secs(30))
                .build(),
        }
    }

    /// Fetch the full tree under `folder_path` and return a synthetic root
    /// node containing the processed (filtered, sorted) children.
    ///
    /// `folder_path` mirrors the Kotlin API: leading `./` and `/` are
    /// stripped, an empty path means the drive root.
    pub fn fetch_root_tree(&self, folder_path: &str) -> AppResult<FileNode> {
        let clean = folder_path.trim_start_matches("./").trim_start_matches('/');

        let url = if clean.is_empty() {
            "https://graph.microsoft.com/v1.0/me/drive/root/children".to_string()
        } else {
            let encoded = clean
                .split('/')
                .map(|segment| urlencoding::encode(segment).into_owned())
                .collect::<Vec<_>>()
                .join("/");
            format!("https://graph.microsoft.com/v1.0/me/drive/root:/{encoded}:/children")
        };

        let display = if clean.is_empty() { "Root" } else { clean };
        let mut root = FileNode::synthetic_root(display);
        root.children = self.fetch_children(&url)?;
        Ok(root)
    }

    /// Recursive + paginated child fetch. Mirrors the Kotlin
    /// `fetchChildren(initialUrl)` exactly: walk the `value` array, recurse
    /// into folders, and follow `@odata.nextLink` until exhausted.
    fn fetch_children(&self, initial_url: &str) -> AppResult<Vec<FileNode>> {
        let mut out = Vec::new();
        let mut url: Option<String> = Some(initial_url.to_string());

        while let Some(this_url) = url.take() {
            let resp: GraphChildrenResponse = self
                .http
                .get(&this_url)
                .set("Authorization", &format!("Bearer {}", self.access_token))
                .set("Accept", "application/json")
                .call()?
                .into_json()?;

            for item in resp.value {
                let mut node = item.into_node();
                if node.is_directory {
                    let child_url = format!(
                        "https://graph.microsoft.com/v1.0/me/drive/items/{}/children",
                        node.id
                    );
                    node.children = self.fetch_children(&child_url)?;
                }
                out.push(node);
            }

            url = resp
                .next_link
                .filter(|s| !s.is_empty());
        }

        Ok(out)
    }

    /// Download a single file's content. The `download_url` is the
    /// pre-authenticated Graph `@microsoft.graph.downloadUrl`; we don't
    /// need to add the Authorization header for it.
    pub fn fetch_file_content(&self, download_url: &str) -> AppResult<String> {
        // Sanity check the URL — the Kotlin app doesn't, but a malformed
        // input would otherwise produce a confusing ureq error.
        let _: Url = Url::parse(download_url)
            .map_err(|e| AppError::Invalid(format!("download url: {e}")))?;

        let body = self
            .http
            .get(download_url)
            .set("Accept", "text/plain, text/markdown, */*;q=0.1")
            .call()?
            .into_string()?;

        Ok(body)
    }
}
