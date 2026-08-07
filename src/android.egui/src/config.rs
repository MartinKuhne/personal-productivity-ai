//! Application configuration.
//!
//! The MSAL-style config JSON is bundled at compile time via `include_str!`
//! so the crate is fully self-contained — no runtime file lookups for the
//! client id or redirect URI. Override the placeholder client id in
//! `assets/auth_config_single_account.json` and rebuild.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Single-account MSAL-style config. Mirrors the shape of the JSON file the
/// Kotlin/Compose app reads from `R.raw.auth_config_single_account` so the
/// two apps can share an Azure app registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub client_id: String,
    pub redirect_uri: String,
    #[serde(default = "default_authority_type")]
    pub authorization_user_agent: String,
    #[serde(default)]
    pub authorities: Vec<Authority>,
}

fn default_authority_type() -> String {
    "DEFAULT".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Authority {
    #[serde(rename = "type")]
    pub kind: String,
    pub audience: Audience,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Audience {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default = "default_tenant")]
    pub tenant_id: String,
}

fn default_tenant() -> String {
    "common".to_string()
}

impl AuthConfig {
    /// The bundled placeholder config. Replace the file under `assets/` and
    /// rebuild to use a real client id.
    pub const PLACEHOLDER_CLIENT_ID: &'static str = "YOUR_CLIENT_ID_HERE";

    pub fn load_bundled() -> AppResult<Self> {
        let raw = include_str!("../assets/auth_config_single_account.json");
        let cfg: AuthConfig = serde_json::from_str(raw)
            .map_err(|e| AppError::Config(format!("bundled auth config: {e}")))?;
        if cfg.client_id == Self::PLACEHOLDER_CLIENT_ID {
            tracing::warn!(
                "bundled auth config has placeholder client_id; \
                 edit assets/auth_config_single_account.json and rebuild"
            );
        }
        Ok(cfg)
    }

    /// The Microsoft v2.0 authorize endpoint for this config's tenant.
    pub fn authorize_endpoint(&self) -> String {
        let tenant = self
            .authorities
            .first()
            .map(|a| a.audience.tenant_id.as_str())
            .unwrap_or("common");
        format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize")
    }

    /// The Microsoft v2.0 token endpoint for this config's tenant.
    pub fn token_endpoint(&self) -> String {
        let tenant = self
            .authorities
            .first()
            .map(|a| a.audience.tenant_id.as_str())
            .unwrap_or("common");
        format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token")
    }
}
