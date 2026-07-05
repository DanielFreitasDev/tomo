//! Atomic file writes: temp file in the SAME directory (cross-device rename
//! fails), fsync file + parent dir on unix, `persist` (MoveFileEx with
//! REPLACE_EXISTING on Windows), with retries for transient Windows locks
//! (editors/antivirus briefly holding the destination).

use std::io::Write;
use std::path::Path;

use crate::CoreError;

pub fn read_text(path: &Path) -> Result<String, CoreError> {
    std::fs::read_to_string(path).map_err(|e| CoreError::io(path, e))
}

pub fn atomic_write(path: &Path, contents: &str) -> Result<(), CoreError> {
    let dir = path
        .parent()
        .ok_or_else(|| CoreError::Invalid(format!("no parent dir for {}", path.display())))?;
    std::fs::create_dir_all(dir).map_err(|e| CoreError::io(dir, e))?;

    let mut tmp = tempfile::Builder::new()
        .prefix(".tomo-write-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| CoreError::io(dir, e))?;
    tmp.write_all(contents.as_bytes())
        .map_err(|e| CoreError::io(path, e))?;
    tmp.flush().map_err(|e| CoreError::io(path, e))?;
    tmp.as_file()
        .sync_all()
        .map_err(|e| CoreError::io(path, e))?;

    let mut attempt = 0;
    let tmp_path = loop {
        match tmp.persist(path) {
            Ok(_) => break None,
            Err(e) if attempt < 3 => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(30 * attempt));
                tmp = e.file;
            }
            Err(e) => break Some(e),
        }
    };
    if let Some(e) = tmp_path {
        return Err(CoreError::io(path, e.error));
    }

    #[cfg(unix)]
    {
        // fsync the directory so the rename itself is durable
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.toml");
        atomic_write(&path, "one").unwrap();
        assert_eq!(read_text(&path).unwrap(), "one");
        atomic_write(&path, "two").unwrap();
        assert_eq!(read_text(&path).unwrap(), "two");
    }

    #[test]
    fn creates_missing_parent_dirs_and_cleans_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/deep/a.toml");
        atomic_write(&path, "x").unwrap();
        assert_eq!(read_text(&path).unwrap(), "x");
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(".tomo-write-"))
            .collect();
        assert!(leftovers.is_empty());
    }
}
