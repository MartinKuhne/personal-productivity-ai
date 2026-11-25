//! Tests for [`TokenStore`] (`agent::lib::mcp::oauth::store`).

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
    assert!(!t.is_expired(now + Duration::from_secs(3_000), DEFAULT_EXPIRY_SKEW));
    assert!(t.is_expired(now + Duration::from_secs(4_000), DEFAULT_EXPIRY_SKEW));
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
    std::fs::write(&path, r#"{"schema_version":9999,"tokens":{}}"#).unwrap();
    // The store treats a corrupt version as empty (logs a
    // warning) rather than failing open. Both behaviors are
    // defensible; today's spec is "log and continue with no
    // tokens", which is the safer user experience.
    let store = TokenStore::open(dir.path()).expect("open");
    assert!(store.get("anywhere").is_none());
}

#[test]
fn is_expired_before_the_unix_epoch_is_reported_as_fresh() {
    // `is_expired` cannot compute a duration since the epoch for a
    // pre-epoch clock, and the implementation chooses "not expired"
    // in that case rather than forcing a pointless refresh.
    let now = std::time::UNIX_EPOCH
        .checked_sub(Duration::from_secs(1))
        .expect("a clock one second before the epoch");
    let token = StoredToken::from_response(&fake_response(), now, None, None);
    assert!(
        token.expires_at.is_some(),
        "the fixture carries an expiry, otherwise the arm is not exercised"
    );
    assert!(
        !token.is_expired(now, DEFAULT_EXPIRY_SKEW),
        "a pre-epoch clock cannot be past the expiry"
    );
}

#[test]
fn from_response_without_optional_fields_leaves_them_empty() {
    let response = TokenResponse {
        access_token: "bare".to_owned(),
        token_type: "bearer".to_owned(),
        expires_in: None,
        refresh_token: None,
        scope: vec![],
    };
    let token = StoredToken::from_response(&response, SystemTime::now(), None, None);
    assert_eq!(token.access_token, "bare", "access token passes through");
    assert_eq!(token.token_type, "bearer", "token type is kept verbatim");
    assert!(
        token.expires_at.is_none(),
        "no expires_in means no computed expiry"
    );
    assert!(token.refresh_token.is_none(), "no refresh token was issued");
    assert!(token.scope.is_empty(), "no scopes were granted");
    assert!(token.client_id.is_none(), "no client id supplied");
    assert!(token.issuer.is_none(), "no issuer supplied");
}

#[test]
fn set_skew_widens_the_freshness_window() {
    let response = TokenResponse {
        access_token: "short-lived".to_owned(),
        token_type: "Bearer".to_owned(),
        expires_in: Some(60),
        refresh_token: None,
        scope: vec![],
    };
    let token = StoredToken::from_response(&response, SystemTime::now(), None, None);

    let default_store = TokenStore::in_memory();
    default_store.put("res", token.clone());
    assert!(
        default_store.is_fresh("res"),
        "with the default 30s skew a 60s token is still fresh"
    );

    let mut eager_store = TokenStore::in_memory();
    eager_store.set_skew(Duration::from_secs(3_600));
    eager_store.put("res", token);
    assert!(
        !eager_store.is_fresh("res"),
        "a skew longer than the lifetime makes the token expire immediately"
    );
}

#[test]
fn is_fresh_is_false_for_an_unknown_resource() {
    let store = TokenStore::in_memory();
    assert!(
        !store.is_fresh("https://never-seen.example.com"),
        "an unknown resource is never fresh"
    );
}

#[test]
fn invalidate_and_clear_on_an_empty_store_stay_clean() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = TokenStore::open(dir.path()).expect("open");

    store.invalidate("https://absent.example.com");
    store.clear();
    store.flush().expect("flush");

    let path = dir.path().join(TOKEN_STORE_FILE_NAME);
    assert!(
        !path.exists(),
        "removing nothing must not mark the store dirty"
    );
}

#[test]
fn drop_flushes_unwritten_tokens() {
    let dir = tempfile::tempdir().expect("tempdir");
    let token = StoredToken::from_response(&fake_response(), SystemTime::now(), None, None);
    {
        let store = TokenStore::open(dir.path()).expect("open");
        store.put("https://mcp.example.com/mcp", token.clone());
        // No explicit flush: Drop must persist the dirty state.
    }

    let reopened = TokenStore::open(dir.path()).expect("reopen");
    assert_eq!(
        reopened.get("https://mcp.example.com/mcp"),
        Some(token),
        "the token survived the drop-flush"
    );
}

#[test]
fn flush_creates_missing_parent_directories() {
    let dir = tempfile::tempdir().expect("tempdir");
    let nested = dir.path().join("nested").join("deeper");
    let store = TokenStore::open(&nested).expect("open a store in a missing directory");
    store.put(
        "https://mcp.example.com/mcp",
        StoredToken::from_response(&fake_response(), SystemTime::now(), None, None),
    );
    store.flush().expect("flush creates the parent directories");
    assert!(
        nested.join(TOKEN_STORE_FILE_NAME).exists(),
        "the store file is written under the created parents"
    );
}

#[test]
fn clear_then_flush_persists_an_empty_store() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = TokenStore::open(dir.path()).expect("open");
    store.put(
        "https://mcp.example.com/mcp",
        StoredToken::from_response(&fake_response(), SystemTime::now(), None, None),
    );
    store.flush().expect("first flush");
    assert!(
        dir.path().join(TOKEN_STORE_FILE_NAME).exists(),
        "the first flush writes the file"
    );

    store.clear();
    store.flush().expect("second flush");
    drop(store);

    let reopened = TokenStore::open(dir.path()).expect("reopen");
    assert!(
        reopened.get("https://mcp.example.com/mcp").is_none(),
        "the cleared state is what got persisted"
    );
}

#[test]
fn open_recovers_from_an_unparseable_store_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(TOKEN_STORE_FILE_NAME);
    std::fs::write(&path, "not json at all").expect("write corrupt store");

    let store = TokenStore::open(dir.path()).expect("a corrupt store must not fail startup");
    assert!(
        store.get("anywhere").is_none(),
        "the corrupt contents are discarded"
    );

    // The corrupt file must still be replaceable by a later flush.
    store.put(
        "https://mcp.example.com/mcp",
        StoredToken::from_response(&fake_response(), SystemTime::now(), None, None),
    );
    store.flush().expect("flush overwrites the corrupt file");
    let reopened = TokenStore::open(dir.path()).expect("reopen");
    assert!(
        reopened.get("https://mcp.example.com/mcp").is_some(),
        "the store recovers permanently"
    );
}

#[test]
fn flush_reports_a_write_failure_when_the_path_is_a_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let blocked = dir.path().join(TOKEN_STORE_FILE_NAME);
    std::fs::create_dir_all(&blocked).expect("occupy the store path with a directory");

    // Opening degrades to an empty store instead of failing startup.
    let store = TokenStore::open(dir.path()).expect("open degrades to empty");
    assert!(store.get("anywhere").is_none(), "nothing was read");

    store.put(
        "https://mcp.example.com/mcp",
        StoredToken::from_response(&fake_response(), SystemTime::now(), None, None),
    );
    let err = store.flush().expect_err("renaming over a directory fails");
    assert!(
        matches!(err, OAuthError::Transport(_)),
        "the failure is reported as a transport error: {err:?}"
    );
}

#[test]
fn stored_token_serde_round_trip() {
    let token = StoredToken {
        access_token: "a".to_owned(),
        token_type: "Bearer".to_owned(),
        expires_at: Some(1_000_000),
        refresh_token: Some("r".to_owned()),
        scope: vec!["read".to_owned(), "write".to_owned()],
        client_id: Some("cid".to_owned()),
        issuer: Some("https://auth.example.com".to_owned()),
    };
    let json = serde_json::to_string(&token).expect("serialize");
    let back: StoredToken = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, token, "every field survives the round trip");

    // Every field except the two required ones is `#[serde(default)]`,
    // so a minimal object must deserialize rather than fail.
    let minimal: StoredToken =
        serde_json::from_str(r#"{"access_token":"x","token_type":"Bearer"}"#)
            .expect("optional fields default instead of erroring");
    assert_eq!(minimal.access_token, "x", "required field parsed");
    assert!(minimal.expires_at.is_none(), "expires_at defaults to None");
    assert!(
        minimal.refresh_token.is_none(),
        "refresh_token defaults to None"
    );
    assert!(minimal.scope.is_empty(), "scope defaults to empty");
    assert!(minimal.client_id.is_none(), "client_id defaults to None");
    assert!(minimal.issuer.is_none(), "issuer defaults to None");
}
