//! Typed schema for everything Tomo stores on disk.
//!
//! These structs are the single source of truth consumed by the TOML layer,
//! the HTTP engine, the scripting bridge and (as DTOs) the frontend.

pub mod auth;
pub mod body;
pub mod collection;
pub mod environment;
pub mod request;
pub mod response;
pub mod settings;

pub use auth::{ApiKeyPlacement, Auth, ClientAuth, OAuth2Config, OAuth2Grant};
pub use body::{Body, MultipartPart, PartKind};
pub use collection::{
    ClientCert, CollectionFile, CollectionMeta, Defaults, FolderFile, FolderMeta, Tls,
};
pub use environment::{EnvMeta, EnvironmentFile, SecretsFile};
pub use request::{
    Assert, AssertOp, Docs, HttpDef, Pair, RequestFile, RequestMeta, RequestOptions, Scripts, Tests,
};
pub use response::{BodyCapture, ResponseData, Timing};
pub use settings::{NetworkSettings, ProxyMode, ProxySettings, Settings, Theme};

/// Variable values are JSON-shaped: strings, numbers, booleans, arrays, objects.
/// (TOML datetimes are not supported in vars — store them as strings.)
pub type VarValue = serde_json::Value;

pub(crate) fn default_true() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) fn is_true(v: &bool) -> bool {
    *v
}
