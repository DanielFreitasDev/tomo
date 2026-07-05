//! .env loading (parse-only — never mutates the process environment).

use std::path::Path;

use indexmap::IndexMap;

/// Parse `<collection>/.env` if present. Malformed lines are skipped.
pub fn load_dotenv(collection_root: &Path) -> IndexMap<String, String> {
    let path = collection_root.join(".env");
    let mut out = IndexMap::new();
    if let Ok(iter) = dotenvy::from_path_iter(&path) {
        for item in iter.flatten() {
            out.insert(item.0, item.1);
        }
    }
    out
}

/// Snapshot of the process environment (taken once per run by callers).
pub fn process_env_snapshot() -> IndexMap<String, String> {
    std::env::vars().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_env_file_without_polluting_process_env() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".env"),
            "TOMO_TEST_SECRET=abc123\n# comment\nQUOTED=\"hello world\"\n",
        )
        .unwrap();

        let vars = load_dotenv(dir.path());
        assert_eq!(vars.get("TOMO_TEST_SECRET").unwrap(), "abc123");
        assert_eq!(vars.get("QUOTED").unwrap(), "hello world");
        assert!(
            std::env::var("TOMO_TEST_SECRET").is_err(),
            "process env untouched"
        );
    }

    #[test]
    fn missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_dotenv(dir.path()).is_empty());
    }
}
