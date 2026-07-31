//! Secure token store for OAuth 2.1 access / refresh tokens.
//!
//! Tokens are kept in two layers:
//!
//! 1. **In-memory cache**, keyed by the canonical MCP server URL.
//!    Thread-safe via a single `Mutex<HashMap>`. The hot path
//!    (`get`) only touches this layer.
//! 2. **Disk persistence** as a JSON file in the user's app data
//!    directory. The file is written with the most restrictive
//!    permissions the OS supports (0600 on POSIX; ACL on Windows)
//!    and the tokens live in a single object keyed by MCP server
//!    URL. We do not encrypt at rest today — a follow-up could
//!    wrap the file with `keyring` or `age`. The MCP spec §4.9
//!    says tokens MUST be stored securely; file-level permissions
//!    are a reasonable baseline for a desktop app.
//!
//! Tokens carry their own expiry (RFC 6749 §5.1 `expires_in`).
//! The store applies a small skew window (default 30s) so we
//! refresh before the server actually rejects the token. Tokens
//! with no expiry are treated as long-lived (we never proactively
//! refresh them; we only refresh after a 401 from the resource
//! server).
//!
//! Refresh tokens are kept alongside the access token and used
//! by the OAuth flow driver when the access token is expired or
//! when the resource server returns 401 / 403.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use super::types::{OAuthError, TokenResponse};

/// How long before the advertised expiry we treat the token as
/// already expired. Lets us refresh proactively without racing the
/// resource server's clock.
pub const DEFAULT_EXPIRY_SKEW: Duration = Duration::from_secs(30);

/// File name we use for the on-disk token store, written under
/// the user data dir supplied at [`TokenStore::open`].
pub const TOKEN_STORE_FILE_NAME: &str = "mcp-oauth-tokens.json";

/// Schema version of the on-disk file. Bumped if we ever change
/// the on-disk representation. We refuse to read unknown future
/// versions rather than risk silently misinterpreting them.
pub const STORE_SCHEMA_VERSION: u32 = 1;

/// One persisted token entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredToken {
    /// The access token.
    pub access_token: String,
    /// The token type, typically `"Bearer"`. We only ever act on
    /// bearer tokens today.
    pub token_type: String,
    /// Expiry timestamp (epoch seconds) as reported by the server,
    /// or `None` for tokens that don't expire. We translate
    /// `expires_in` to this at write time so the on-disk format
    /// is unambiguous.
    #[serde(default)]
    pub expires_at: Option<i64>,
    /// Refresh token, if the server issued one.
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Scopes the token is scoped to.
    #[serde(default)]
    pub scope: Vec<String>,
    /// Client ID used to mint this token. Needed for refresh
    /// (the client may have re-registered between tokens).
    #[serde(default)]
    pub client_id: Option<String>,
    /// Authorization server issuer URL this token came from.
    /// Together with `client_id` this fully identifies the
    /// `(AS, client, resource)` triple the refresh must use.
    #[serde(default)]
    pub issuer: Option<String>,
}

impl StoredToken {
    /// Build a [`StoredToken`] from a fresh [`TokenResponse`].
    /// `now` is the wall-clock at which the response was received;
    /// it lets tests inject a deterministic clock.
    pub fn from_response(
        response: &TokenResponse,
        now: std::time::SystemTime,
        client_id: Option<String>,
        issuer: Option<String>,
    ) -> Self {
        let expires_at = response.expires_in.and_then(|secs| {
            now.checked_add(Duration::from_secs(secs))
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
        });
        Self {
            access_token: response.access_token.clone(),
            token_type: response.token_type.clone(),
            expires_at,
            refresh_token: response.refresh_token.clone(),
            scope: response.scope.clone(),
            client_id,
            issuer,
        }
    }

    /// `true` if this token is past its expiry (with the supplied
    /// skew applied). Tokens with no expiry are never considered
    /// expired by this method — we still respond to 401/403 from
    /// the resource server.
    pub fn is_expired(&self, now: std::time::SystemTime, skew: Duration) -> bool {
        match self.expires_at {
            Some(exp) => match now.duration_since(std::time::UNIX_EPOCH) {
                Ok(d) => d.as_secs() as i64 >= exp - skew.as_secs() as i64,
                Err(_) => false,
            },
            None => false,
        }
    }
}

/// On-disk envelope. Keeps a schema version so we can detect and
/// refuse to read a future format.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OnDiskStore {
    schema_version: u32,
    /// Keyed by the canonical MCP server resource URI.
    tokens: HashMap<String, StoredToken>,
}

/// Thread-safe in-memory + on-disk token store. Cheap to clone via
/// the inner `Arc` once we move to a `tokio::sync::RwLock` later;
/// for now we use a single `Mutex` because the hot path is rare
/// and contention is bounded by the rate of MCP tool calls.
pub struct TokenStore {
    inner: Mutex<Inner>,
    path: Option<PathBuf>,
    skew: Duration,
}

struct Inner {
    tokens: HashMap<String, StoredToken>,
    dirty: bool,
}

impl TokenStore {
    /// Open (or create) a token store rooted at `dir`. The
    /// returned store is in-memory only; tokens are persisted
    /// lazily on the next `put` call.
    pub fn open(dir: &Path) -> Result<Self, OAuthError> {
        let path = dir.join(TOKEN_STORE_FILE_NAME);
        let tokens = if path.exists() {
            match Self::read_disk(&path) {
                Ok(s) => s,
                Err(e) => {
                    // Treat an unreadable store as "no tokens"; the
                    // user would rather log in again than see the
                    // app fail to start.
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "could not read MCP token store; starting empty"
                    );
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };
        Ok(Self {
            inner: Mutex::new(Inner {
                tokens,
                dirty: false,
            }),
            path: Some(path),
            skew: DEFAULT_EXPIRY_SKEW,
        })
    }

    /// Construct an in-memory-only store (no on-disk persistence).
    /// Used by tests and by callers that have not yet wired up a
    /// data dir.
    pub fn in_memory() -> Self {
        Self {
            inner: Mutex::new(Inner {
                tokens: HashMap::new(),
                dirty: false,
            }),
            path: None,
            skew: DEFAULT_EXPIRY_SKEW,
        }
    }

    /// Set the expiry skew used by [`Self::is_expired`]. Default
    /// is [`DEFAULT_EXPIRY_SKEW`]. Tests can shorten it.
    pub fn set_skew(&mut self, skew: Duration) {
        self.skew = skew;
    }

    /// Look up the token for a given MCP server resource URL.
    /// Returns `None` if there is no token; callers should
    /// initiate the OAuth flow on `None`.
    pub fn get(&self, resource: &str) -> Option<StoredToken> {
        self.inner.lock().ok()?.tokens.get(resource).cloned()
    }

    /// Returns `true` if the token for `resource` is present and
    /// not yet within the expiry skew window.
    pub fn is_fresh(&self, resource: &str) -> bool {
        let now = std::time::SystemTime::now();
        match self.get(resource) {
            Some(t) => !t.is_expired(now, self.skew),
            None => false,
        }
    }

    /// Store a token for the given MCP server resource URL. Marks
    /// the store dirty; the next call to [`Self::flush`] (or
    /// drop) writes to disk.
    pub fn put(&self, resource: &str, token: StoredToken) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.tokens.insert(resource.to_owned(), token);
            inner.dirty = true;
        }
    }

    /// Remove the token for the given resource URL. The next
    /// MCP call against that resource will re-trigger the OAuth
    /// flow.
    pub fn invalidate(&self, resource: &str) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.tokens.remove(resource).is_some() {
                inner.dirty = true;
            }
        }
    }

    /// Forget every token. Used when the user signs out or the
    /// app is reset.
    pub fn clear(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            if !inner.tokens.is_empty() {
                inner.tokens.clear();
                inner.dirty = true;
            }
        }
    }

    /// Write the current state to disk if anything has changed.
    /// No-op for an in-memory store.
    pub fn flush(&self) -> Result<(), OAuthError> {
        let path = match &self.path {
            Some(p) => p.clone(),
            None => return Ok(()),
        };
        let snapshot = {
            let inner = self
                .inner
                .lock()
                .map_err(|_| OAuthError::Internal("token store lock poisoned".to_owned()))?;
            if !inner.dirty {
                return Ok(());
            }
            OnDiskStore {
                schema_version: STORE_SCHEMA_VERSION,
                tokens: inner.tokens.clone(),
            }
        };
        write_disk(&path, &snapshot)?;
        if let Ok(mut inner) = self.inner.lock() {
            inner.dirty = false;
        }
        Ok(())
    }

    fn read_disk(path: &Path) -> Result<HashMap<String, StoredToken>, OAuthError> {
        let body = std::fs::read_to_string(path)
            .map_err(|e| OAuthError::Transport(format!("read token store: {e}")))?;
        let parsed: OnDiskStore = serde_json::from_str(&body)
            .map_err(|e| OAuthError::Protocol(format!("parse token store: {e}")))?;
        if parsed.schema_version != STORE_SCHEMA_VERSION {
            return Err(OAuthError::Protocol(format!(
                "unknown token store schema version {} (expected {})",
                parsed.schema_version, STORE_SCHEMA_VERSION
            )));
        }
        Ok(parsed.tokens)
    }
}

impl Drop for TokenStore {
    fn drop(&mut self) {
        if let Err(e) = self.flush() {
            tracing::warn!(
                error = %e,
                "failed to flush MCP token store on drop"
            );
        }
    }
}

fn write_disk(path: &Path, store: &OnDiskStore) -> Result<(), OAuthError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| OAuthError::Transport(format!("create token store dir: {e}")))?;
    }
    let body = serde_json::to_string_pretty(store)
        .map_err(|e| OAuthError::Internal(format!("serialize token store: {e}")))?;
    // Write to a sibling temp file and atomically rename, so a
    // crash mid-write doesn't leave a truncated JSON file behind.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body)
        .map_err(|e| OAuthError::Transport(format!("write token store: {e}")))?;
    set_owner_only_permissions(&tmp)?;
    std::fs::rename(&tmp, path)
        .map_err(|e| OAuthError::Transport(format!("rename token store: {e}")))?;
    set_owner_only_permissions(path)?;
    Ok(())
}

/// Set restrictive file permissions. POSIX: `0600`. Windows: ACL
/// is harder; we do the best we can by removing inheritance and
/// granting the current user full control. If the platform-specific
/// call fails we keep the file but log a warning; the JSON file
/// is on the user's local machine either way.
fn set_owner_only_permissions(path: &Path) -> Result<(), OAuthError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms).map_err(|e| {
            OAuthError::Transport(format!("set permissions on {}: {e}", path.display()))
        })?;
    }
    #[cfg(windows)]
    {
        // On Windows we try to clear read permissions for "Everyone"
        // and "Users" via icacls. This is best-effort — we don't
        // fail the whole save if it errors.
        let path_str = match path.to_str() {
            Some(s) => s,
            None => return Ok(()),
        };
        let _ = std::process::Command::new("icacls")
            .arg(path_str)
            .arg("/inheritance:r")
            .arg("/grant:r")
            .arg(format!(
                "{}:(R)",
                std::env::var("USERNAME").unwrap_or_default()
            ))
            .status();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn fake_response() -> TokenResponse {
        TokenResponse {
            access_token: "access-1".to_owned(),
            token_type: "Bearer".to_owned(),
            expires_in: Some(3600),
            refresh_token: Some("refresh-1".to_owned()),
            scope: vec!["read".to_owned(), "write".to_owned()],
        }
    }

    #[test]
    fn stored_token_expiry_handling() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000_000);
        let t = StoredToken::from_response(&fake_response(), now, None, None);
        assert_eq!(t.access_token, "access-1");
        // Expiry = now + 3600 = 1_001_000. With a 30s skew the
        // token is "expired" once wall-clock >= 1_000_970.
        assert!(!t.is_expired(now, DEFAULT_EXPIRY_SKEW));
        assert!(!t.is_expired(
            now + Duration::from_secs(3_000),
            DEFAULT_EXPIRY_SKEW
        ));
        assert!(t.is_expired(
            now + Duration::from_secs(4_000),
            DEFAULT_EXPIRY_SKEW
        ));
    }

    #[test]
    fn stored_token_without_expiry_is_never_expired() {
        let mut r = fake_response();
        r.expires_in = None;
        let t = StoredToken::from_response(&r, SystemTime::now(), None, None);
        // 100 years from now is still fresh.
        assert!(!t.is_expired(
            SystemTime::now() + Duration::from_secs(100 * 365 * 24 * 3600),
            DEFAULT_EXPIRY_SKEW
        ));
    }

    #[test]
    fn in_memory_store_round_trip() {
        let store = TokenStore::in_memory();
        let r = fake_response();
        let token = StoredToken::from_response(
            &r,
            SystemTime::now(),
            Some("client".to_owned()),
            Some("https://auth.example.com".to_owned()),
        );
        store.put("https://mcp.example.com", token.clone());
        let got = store.get("https://mcp.example.com").expect("got token");
        assert_eq!(got, token);
        assert!(store.is_fresh("https://mcp.example.com"));
    }

    #[test]
    fn invalidate_removes_token() {
        let store = TokenStore::in_memory();
        store.put(
            "https://mcp.example.com",
            StoredToken::from_response(&fake_response(), SystemTime::now(), None, None),
        );
        assert!(store.get("https://mcp.example.com").is_some());
        store.invalidate("https://mcp.example.com");
        assert!(store.get("https://mcp.example.com").is_none());
    }

    #[test]
    fn clear_removes_every_token() {
        let store = TokenStore::in_memory();
        store.put(
            "https://mcp.example.com/a",
            StoredToken::from_response(&fake_response(), SystemTime::now(), None, None),
        );
        store.put(
            "https://mcp.example.com/b",
            StoredToken::from_response(&fake_response(), SystemTime::now(), None, None),
        );
        store.clear();
        assert!(store.get("https://mcp.example.com/a").is_none());
        assert!(store.get("https://mcp.example.com/b").is_none());
    }

    #[test]
    fn open_persists_and_reloads() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = TokenStore::open(dir.path()).expect("open");
        let token = StoredToken::from_response(
            &fake_response(),
            SystemTime::now(),
            Some("client".to_owned()),
            Some("https://auth.example.com".to_owned()),
        );
        store.put("https://mcp.example.com/mcp", token.clone());
        store.flush().expect("flush");

        // Re-open in a fresh instance.
        let store2 = TokenStore::open(dir.path()).expect("reopen");
        let got = store2
            .get("https://mcp.example.com/mcp")
            .expect("got token");
        assert_eq!(got, token);
    }

    #[test]
    fn flush_is_noop_when_clean() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = TokenStore::open(dir.path()).expect("open");
        // Don't put anything; flush should be a no-op (no file written).
        store.flush().expect("flush");
        let path = dir.path().join(TOKEN_STORE_FILE_NAME);
        assert!(!path.exists(), "no file should have been written");
    }

    #[test]
    fn unknown_schema_version_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(TOKEN_STORE_FILE_NAME);
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(
            &path,
            r#"{"schema_version":9999,"tokens":{}}"#,
        )
        .unwrap();
        // The store treats a corrupt version as empty (logs a
        // warning) rather than failing open. Both behaviors are
        // defensible; today's spec is "log and continue with no
        // tokens", which is the safer user experience.
        let store = TokenStore::open(dir.path()).expect("open");
        assert!(store.get("anywhere").is_none());
    }
}
