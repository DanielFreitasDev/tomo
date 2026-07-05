//! tomo-core — pure, fully unit-testable logic for the Tomo API client.
//!
//! No Tauri dependencies live here. Modules are added milestone by milestone:
//! `model` (typed schema), `toml` (round-trip de/ser), `fsops`, `vars`,
//! `http`, `script`, `asserts`, `curl`, `history`.

pub mod error;
pub mod format;
pub mod model;

pub use error::CoreError;

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
