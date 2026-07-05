//! The HTTP engine: inheritance resolution, URL building, client construction,
//! cookie jar, streamed capture with spill, and the pipeline orchestrator.

pub mod auth;
pub mod build;
pub mod capture;
pub mod client;
pub mod cookies;
pub mod engine;
pub mod resolve;

pub use build::build_url;
pub use client::{ClientOptions, build_client};
pub use cookies::{CookieDto, TomoJar};
pub use engine::{EngineConfig, RunSpec, execute};
pub use resolve::{Chain, ResolvedInputs, resolve_chain};
