//! Pure link resolver for determining actions when links are activated in FastMD.
//!
//! Unit tests live in the sibling `link_resolver_tests.rs` sidecar.

use std::path::{Path, PathBuf};

/// Action to perform in response to an activated link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkAction {
    /// Open a workspace file inside FastMD in a new or existing tab, optionally scrolling to an anchor.
    OpenWorkspaceFile {
        /// Target file path to open.
        path: PathBuf,
        /// Optional heading anchor to scroll to once loaded.
        anchor: Option<String>,
    },
    /// Scroll to a heading anchor inside the currently active document.
    ScrollToAnchor {
        /// Target heading text or slug.
        anchor: String,
    },
    /// Open an external URL or resource via the OS default handler (browser, mail client, etc.).
    OpenExternal(String),
    /// The link resolved to a path that is not a known file, so there is nothing
    /// to open. Carries the best-guess path for logging.
    ///
    /// Plain markdown links never create files on click: only `wikilink:` targets
    /// do, and those keep their own create-on-click contract.
    NotFound {
        /// Path the link resolved to, which does not exist.
        path: PathBuf,
    },
}

/// Resolves a clicked link string into an actionable [`LinkAction`].
///
/// Handles in-page anchors (`#heading`), external web/mail URLs (`https://...`, `mailto:...`),
/// wikilinks (`wikilink:<target>`), and local/relative document paths (`doc.md`, `../notes.md`).
pub fn resolve_link(
    link: &str,
    current_file: Option<&Path>,
    workspace_files: &[PathBuf],
    content_roots: &[PathBuf],
) -> LinkAction {
    let trimmed = link.trim();
    if trimmed.is_empty() {
        return LinkAction::OpenExternal(String::new());
    }

    // 1. In-page anchor: `#anchor-id`
    if let Some(anchor) = trimmed.strip_prefix('#') {
        return LinkAction::ScrollToAnchor {
            anchor: anchor.to_string(),
        };
    }

    // 2. External URL schemes (http, https, mailto, ftp)
    if is_external_scheme(trimmed) {
        return LinkAction::OpenExternal(trimmed.to_string());
    }

    // 3. Wikilink: `wikilink:TargetNote` or `wikilink:TargetNote#Anchor`
    if let Some(wiki_content) = trimmed.strip_prefix("wikilink:") {
        return resolve_wikilink(wiki_content, current_file, workspace_files, content_roots);
    }

    // 4. Local or relative file paths (with optional #anchor)
    resolve_file_link(trimmed, current_file, workspace_files, content_roots)
}

/// Number of leading bytes needed to recognise any supported external scheme
/// — the longest recognised scheme is `https://`.
const SCHEME_PROBE_LEN: usize = 8;

/// Checks if a string starts with a standard external URL scheme.
///
/// The scheme is compared case-insensitively (RFC 3986 §6.2.1: "scheme names
/// ... are case-insensitive"), so `HTTPS://example.com` and `Http://example.com`
/// are external URLs and never fall through to workspace-path resolution.
///
/// A Windows drive path is **not** a scheme: the recognised set contains no
/// single-letter scheme, so `C:\notes\x.md` stays a workspace path even though
/// a `:` follows the drive letter.
fn is_external_scheme(s: &str) -> bool {
    // Only the leading bytes can carry a scheme. `get` yields `None` when the
    // slice would cut a multi-byte character, and such a string cannot start
    // with an ASCII scheme anyway, so the whole string is used as the probe.
    let head = s.get(..SCHEME_PROBE_LEN).unwrap_or(s).to_ascii_lowercase();
    head.starts_with("http://")
        || head.starts_with("https://")
        || head.starts_with("mailto:")
        || head.starts_with("ftp://")
}

/// Resolves a wikilink target name into a workspace file path.
fn resolve_wikilink(
    wiki_content: &str,
    current_file: Option<&Path>,
    workspace_files: &[PathBuf],
    content_roots: &[PathBuf],
) -> LinkAction {
    let (target, anchor) = match wiki_content.split_once('#') {
        Some((t, a)) => (t.trim(), Some(a.trim().to_string())),
        None => (wiki_content.trim(), None),
    };

    if target.is_empty() {
        if let Some(a) = anchor {
            return LinkAction::ScrollToAnchor { anchor: a };
        }
        return LinkAction::OpenExternal(wiki_content.to_string());
    }

    // Search workspace files by stem or path suffix (case-insensitive)
    let target_lower = target.to_lowercase();
    let target_with_md = format!("{target_lower}.md");

    // 1. Exact or stem match in workspace files
    for file in workspace_files {
        if let Some(stem) = file.file_stem()
            && stem.to_string_lossy().to_lowercase() == target_lower
        {
            return LinkAction::OpenWorkspaceFile {
                path: file.clone(),
                anchor,
            };
        }
        let file_lossy = file.to_string_lossy().to_lowercase();
        if file_lossy.ends_with(&format!("/{target_with_md}"))
            || file_lossy.ends_with(&format!("\\{target_with_md}"))
            || file_lossy.ends_with(&format!("/{target_lower}"))
            || file_lossy.ends_with(&format!("\\{target_lower}"))
        {
            return LinkAction::OpenWorkspaceFile {
                path: file.clone(),
                anchor,
            };
        }
    }

    // 2. Relative to current file parent
    if let Some(cur) = current_file
        && let Some(parent) = cur.parent()
    {
        let direct = parent.join(target);
        if direct.exists() && direct.is_file() {
            return LinkAction::OpenWorkspaceFile {
                path: normalize_path(&direct),
                anchor,
            };
        }
        let with_md = parent.join(format!("{target}.md"));
        if with_md.exists() && with_md.is_file() {
            return LinkAction::OpenWorkspaceFile {
                path: normalize_path(&with_md),
                anchor,
            };
        }
    }

    // 3. Search under content roots
    for root in content_roots {
        let direct = root.join(target);
        if direct.exists() && direct.is_file() {
            return LinkAction::OpenWorkspaceFile {
                path: normalize_path(&direct),
                anchor,
            };
        }
        let with_md = root.join(format!("{target}.md"));
        if with_md.exists() && with_md.is_file() {
            return LinkAction::OpenWorkspaceFile {
                path: normalize_path(&with_md),
                anchor,
            };
        }
    }

    // If not found on disk, but current_file parent exists, produce default path
    if let Some(cur) = current_file
        && let Some(parent) = cur.parent()
    {
        return LinkAction::OpenWorkspaceFile {
            path: normalize_path(&parent.join(format!("{target}.md"))),
            anchor,
        };
    }

    LinkAction::OpenExternal(format!("wikilink:{wiki_content}"))
}

/// Resolves a file link (relative path, absolute path, or `file://` URI).
///
/// Returns [`LinkAction::NotFound`] when no candidate names a real file, so a
/// click on a dead link never opens a phantom tab or launches an editor on a
/// missing file. Unlike `wikilink:` targets, plain file links do not create
/// files on click.
fn resolve_file_link(
    link: &str,
    current_file: Option<&Path>,
    workspace_files: &[PathBuf],
    content_roots: &[PathBuf],
) -> LinkAction {
    let (path_part, anchor) = match link.split_once('#') {
        Some((p, a)) => (p.trim(), Some(a.trim().to_string())),
        None => (link.trim(), None),
    };

    if path_part.is_empty() {
        if let Some(a) = anchor {
            return LinkAction::ScrollToAnchor { anchor: a };
        }
        return LinkAction::OpenExternal(link.to_string());
    }

    // Strip `file://` or `file:///` prefix if present
    let raw_path = if let Some(stripped) = path_part.strip_prefix("file:///") {
        stripped
    } else if let Some(stripped) = path_part.strip_prefix("file://") {
        stripped
    } else {
        path_part
    };

    let target_path = Path::new(raw_path);

    // Absolute path: open it only when it names a real file.
    if target_path.is_absolute() {
        return open_or_not_found(normalize_path(target_path), anchor, workspace_files);
    }

    // Best guess: relative to the current file's directory (or as given when
    // there is no current file). This is also the path reported when nothing
    // matches, so the log names what the user clicked.
    let guess = match current_file {
        Some(cur) => match cur.parent() {
            Some(parent) => normalize_path(&parent.join(target_path)),
            None => normalize_path(target_path),
        },
        None => normalize_path(target_path),
    };
    if is_known_file(&guess, workspace_files) {
        return LinkAction::OpenWorkspaceFile {
            path: guess,
            anchor,
        };
    }

    // Relative to a content root.
    for root in content_roots {
        let candidate = normalize_path(&root.join(target_path));
        if is_known_file(&candidate, workspace_files) {
            return LinkAction::OpenWorkspaceFile {
                path: candidate,
                anchor,
            };
        }
    }

    // Nothing on disk matched. Report the best guess as missing rather than
    // handing a phantom path to the tab bar or the system editor.
    LinkAction::NotFound { path: guess }
}

/// Whether `path` names a file the application can actually open: present on
/// disk, or listed in the workspace index (the authoritative listing when links
/// are resolved against a scanned tree).
fn is_known_file(path: &Path, workspace_files: &[PathBuf]) -> bool {
    path.exists() || workspace_files.iter().any(|f| normalize_path(f) == path)
}

/// Wraps a resolved path as an open action when it is a known file, and as
/// [`LinkAction::NotFound`] when it is not.
fn open_or_not_found(
    path: PathBuf,
    anchor: Option<String>,
    workspace_files: &[PathBuf],
) -> LinkAction {
    if is_known_file(&path, workspace_files) {
        LinkAction::OpenWorkspaceFile { path, anchor }
    } else {
        LinkAction::NotFound { path }
    }
}

/// Lexically normalizes a path by resolving `.` and `..` components.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
#[path = "link_resolver_tests.rs"]
mod link_resolver_tests;
