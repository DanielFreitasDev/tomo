//! The HTTP engine: inheritance resolution, URL building, client construction,
//! cookie jar, streamed capture with spill, and the pipeline orchestrator.

pub mod auth;
pub mod build;
pub mod capture;
pub mod client;
pub mod cookies;
pub mod digest;
pub mod engine;
pub mod oauth2;
pub mod resolve;

pub use build::build_url;
pub use client::{ClientOptions, build_client, resolve_client_identity};
pub use cookies::{CookieDto, TomoJar};
pub use engine::{EngineConfig, RunSpec, execute};
pub use oauth2::TokenCache;
pub use resolve::{Chain, ResolvedInputs, resolve_chain};
