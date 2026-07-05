//! Managed .gitignore block — created before secrets are ever written.

use std::path::Path;

use super::atomic::atomic_write;
use crate::CoreError;

const BLOCK_START: &str = "# --- tomo managed (do not edit inside this block) ---";
const BLOCK_END: &str = "# --- end tomo managed ---";
const BLOCK_BODY: &str = "secrets.toml\n.env\n";

/// Idempotently ensure the collection's .gitignore contains the managed block.
/// User content outside the block is preserved untouched.
pub fn upsert_gitignore(collection_root: &Path) -> Result<(), CoreError> {
    let path = collection_root.join(".gitignore");
    let existing = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(CoreError::io(&path, e)),
    };

    let block = format!("{BLOCK_START}\n{BLOCK_BODY}{BLOCK_END}\n");

    let updated = match (existing.find(BLOCK_START), existing.find(BLOCK_END)) {
        (Some(start), Some(end_at)) => {
            let end = end_at + BLOCK_END.len();
            // swallow one trailing newline of the old block
            let end = if existing[end..].starts_with('\n') {
                end + 1
            } else {
                end
            };
            let mut out = String::with_capacity(existing.len() + block.len());
            out.push_str(&existing[..start]);
            out.push_str(&block);
            out.push_str(&existing[end..]);
            out
        }
        _ => {
            let mut out = existing.clone();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&block);
            out
        }
    };

    if updated != existing {
        atomic_write(&path, &updated)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        upsert_gitignore(dir.path()).unwrap();
        let first = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(first.contains("secrets.toml"));
        assert!(first.contains(BLOCK_START));

        upsert_gitignore(dir.path()).unwrap();
        let second = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(first, second, "second run must not change the file");
    }

    #[test]
    fn preserves_user_content_around_the_block() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "node_modules/\n").unwrap();
        upsert_gitignore(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(text.starts_with("node_modules/\n"));
        assert!(text.contains("secrets.toml"));

        // user edits below the block survive a re-run
        std::fs::write(
            dir.path().join(".gitignore"),
            format!("{text}\n# my stuff\ndist/\n"),
        )
        .unwrap();
        upsert_gitignore(dir.path()).unwrap();
        let text2 = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(text2.contains("# my stuff"));
        assert!(text2.contains("dist/"));
        assert_eq!(
            text2.matches(BLOCK_START).count(),
            1,
            "exactly one managed block"
        );
    }
}
