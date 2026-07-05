//! The TOML format layer — the soul of the product.
//!
//! Reading is serde-based (`de`). Writing has two paths:
//! - `write`: canonical documents for NEW files (beautiful, hand-editable style)
//! - `sync`: surgical edits into EXISTING text, preserving user comments,
//!   ordering and whitespace (git-friendliness is the feature)

pub mod de;
pub mod sync;
pub mod value;
pub mod write;

pub use de::{
    parse_collection, parse_environment, parse_folder, parse_request, parse_secrets, parse_settings,
};
pub use sync::{sync_collection, sync_environment, sync_folder, sync_request};
pub use write::{
    collection_to_string, environment_to_string, folder_to_string, request_to_string,
    secrets_to_string, settings_to_string,
};
