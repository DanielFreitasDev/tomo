//! cURL import (command string → RequestFile) and export (→ command string).

mod export;
mod import;

pub use export::{to_curl, to_curl_interpolated};
pub use import::from_curl;
