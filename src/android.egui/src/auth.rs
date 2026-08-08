//! OAuth 2.0 Authorization Code with PKCE against Microsoft Identity v2.0.
//!
//! This is a hand-rolled implementation — there is no production-grade
//! `com.microsoft.identity.client` equivalent in Rust. We use `ring` for
//! SHA-256 and CSPRNG, the same approach the desktop `fastmd` crate uses for
//! its MCP OAuth 2.1 flow (`src/desktop/Cargo.toml` documents the rationale
//! for the `ring` direct dependency).
//!
//! The flow:
//! 1. [`PkceSession::generate`] produces a fresh verifier, challenge, and
//!    state. Persist this between the authorize step and the token exchange
//!    so the verifier sent to the token endpoint matches the challenge sent
//!    to the authorize endpoint.
//! 2. [`build_authorize_url`] renders the v2.0 `/authorize` URL. The caller
//!    opens it in the system browser (Android `Intent.ACTION_VIEW`).
//! 3. After the user signs in, the browser redirects to
//!    `msauth://com.fastmd.android.egui?code=...&state=...`.
//! 4. [`parse_auth_code_from_uri`] pulls the code out of the deep link, and
//!    [`exchange_code`] performs the back-channel token exchange.
//!
//! The deep-link handoff itself (catching the redirected activity intent) is
//! wired in [`crate::app`], not here. This module is egui-free and pure
//! logic so it can be unit-tested on the host.

use base64::Engine;
use ring::digest::SHA256;
use ring::rand::{SecureRandom, SystemRandom};
use url::Url;

use crate::config::AuthConfig;
use crate::error::{AppError, AppResult};

/// Scopes requested from Microsoft Identity. `Files.Read.All` matches the
/// Kotlin app; `offline_access` is what gives us a refresh token, which we'd
/// need for production silent re-auth. The current crate only stores the
/// access token in memory.
pub const SCOPES: &str = "openid profile offline_access Files.Read.All";

/// One in-flight PKCE session. Keep it until the token exchange succeeds or
/// the user cancels; the verifier must match the challenge that was sent.
#[derive(Debug, Clone)]
pub struct PkceSession {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

impl PkceSession {
    pub fn generate() -> AppResult<Self> {
        let rng = SystemRandom::new();
        let mut verifier_bytes = [0u8; 64];
        let mut state_bytes = [0u8; 32];
        rng.fill(&mut verifier_bytes)
            .map_err(|e| AppError::Crypto(format!("CSPRNG fill: {e}")))?;
        rng.fill(&mut state_bytes)
            .map_err(|e| AppError::Crypto(format!("CSPRNG fill: {e}")))?;

        // OAuth 2.0 PKCE spec: verifier is the unpadded base64url of 32-64
        // random octets. 64 bytes / 86 base64url chars is comfortably in the
        // 43-128 allowed range.
        let verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(verifier_bytes);
        let state = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(state_bytes);

        let challenge_bytes = ring::digest::digest(&SHA256, verifier.as_bytes());
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(challenge_bytes);

        Ok(Self {
            verifier,
            challenge,
            state,
        })
    }
}

/// Build the v2.0 `/authorize` URL the caller opens in the system browser.
pub fn build_authorize_url(cfg: &AuthConfig, session: &PkceSession) -> AppResult<Url> {
    let mut url = Url::parse(&cfg.authorize_endpoint())
        .map_err(|e| AppError::Auth(format!("authorize endpoint: {e}")))?;

    url.query_pairs_mut()
        .append_pair("client_id", &cfg.client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", &cfg.redirect_uri)
        .append_pair("scope", SCOPES)
        .append_pair("code_challenge", &session.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &session.state)
        .append_pair("response_mode", "query");

    Ok(url)
}

/// Pull the `code` and `state` out of the `msauth://` deep-link URI the
/// browser redirected to. Returns `None` for URIs that don't look like a
/// successful auth response.
pub fn parse_auth_code_from_uri(uri: &str) -> AppResult<AuthCallback> {
    let parsed = Url::parse(uri)
        .map_err(|e| AppError::Auth(format!("redirect uri parse: {e}")))?;

    // The msauth scheme is non-standard so url::Url may not accept it on
    // older versions; if that happens we work around it by stripping the
    // scheme and re-parsing the query.
    let mut params: Vec<(String, String)> = if parsed.scheme() == "msauth" {
        // url::Url handled it; pull from the query.
        parsed
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect()
    } else {
        return Err(AppError::Auth(format!(
            "unexpected scheme: {}",
            parsed.scheme()
        )));
    };

    // Microsoft also reports errors via query params. Surface those.
    let error = take_param(&mut params, "error");
    let error_description = take_param(&mut params, "error_description");

    if let Some(err) = error {
        return Err(AppError::Auth(format!(
            "{}: {}",
            err,
            error_description.unwrap_or_default()
        )));
    }

    let code = take_param(&mut params, "code")
        .ok_or_else(|| AppError::Auth("redirect missing 'code' param".to_string()))?;
    let state = take_param(&mut params, "state")
        .ok_or_else(|| AppError::Auth("redirect missing 'state' param".to_string()))?;

    Ok(AuthCallback { code, state })
}

/// The two params we care about from a successful auth redirect.
#[derive(Debug, Clone)]
pub struct AuthCallback {
    pub code: String,
    pub state: String,
}

fn take_param(params: &mut Vec<(String, String)>, key: &str) -> Option<String> {
    params
        .iter()
        .position(|(k, _)| k == key)
        .map(|i| params.swap_remove(i).1)
}

/// Trade the auth code for an access token via the v2.0 token endpoint.
pub fn exchange_code(
    cfg: &AuthConfig,
    session: &PkceSession,
    code: &str,
) -> AppResult<TokenSet> {
    let form = [
        ("client_id", cfg.client_id.as_str()),
        ("scope", SCOPES),
        ("code", code),
        ("redirect_uri", cfg.redirect_uri.as_str()),
        ("grant_type", "authorization_code"),
        ("code_verifier", session.verifier.as_str()),
    ];

    let resp = ureq::post(&cfg.token_endpoint())
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("Accept", "application/json")
        .send_form(&form)?;

    let parsed: TokenResponse = resp.into_json()?;
    parsed.try_into()
}

/// Subset of the v2.0 token response we care about. Microsoft returns more
/// fields (`id_token`, `refresh_token`, `expires_in`, etc.) which we ignore
/// for now — see `auth.rs` module doc for the production gap.
#[derive(Debug, serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    token_type: Option<String>,
}

impl TryFrom<TokenResponse> for TokenSet {
    type Error = AppError;
    fn try_from(t: TokenResponse) -> AppResult<Self> {
        if t.access_token.is_empty() {
            return Err(AppError::Auth("token response missing access_token".to_string()));
        }
        let expires_at = match t.expires_in {
            Some(secs) => chrono::Utc::now() + chrono::Duration::seconds(secs),
            None => chrono::Utc::now() + chrono::Duration::seconds(3600),
        };
        Ok(TokenSet {
            access_token: t.access_token,
            expires_at,
            scope: t.scope,
        })
    }
}

/// What we keep in memory after a successful sign-in. The expiry is
/// advisory — we don't proactively refresh; the next Graph call will fail
/// with 401 and the UI surfaces the auth error.
#[derive(Debug, Clone)]
pub struct TokenSet {
    pub access_token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub scope: Option<String>,
}

impl TokenSet {
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now() >= self.expires_at
    }
}
