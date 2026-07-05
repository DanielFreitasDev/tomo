//! Authentication config — `[auth]` table, discriminated by `type`.
//! An absent `[auth]` table means "inherit from parent"; `type = "none"` opts out.

use serde::{Deserialize, Serialize};

use super::default_true;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Auth {
    /// Explicit "no auth" (opts out of inheritance).
    None,
    /// Explicit inherit (same as omitting the `[auth]` table).
    Inherit,
    Basic {
        username: String,
        password: String,
    },
    Bearer {
        token: String,
    },
    ApiKey {
        key: String,
        value: String,
        #[serde(default)]
        placement: ApiKeyPlacement,
    },
    Digest {
        username: String,
        password: String,
    },
    Oauth2(OAuth2Config),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyPlacement {
    #[default]
    Header,
    Query,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuth2Config {
    pub grant: OAuth2Grant,
    pub token_url: String,
    pub client_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub client_secret: String,
    /// Only for the password grant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub client_auth: ClientAuth,
    #[serde(default = "default_true")]
    pub cache_token: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuth2Grant {
    ClientCredentials,
    Password,
}

/// How client credentials are sent to the token endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClientAuth {
    #[default]
    BasicHeader,
    Body,
}
