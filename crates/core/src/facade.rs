//! High-level helpers the app layer composes: assemble a request's full
//! inheritance chain from disk, and settings persistence. Keeps the Tauri
//! command layer thin and this logic unit-testable.

use std::path::Path;

use crate::CoreError;
use crate::format::{
    parse_collection, parse_folder, parse_request, parse_settings, settings_to_string,
};
use crate::fsops::{FOLDER_FILE, read_text, resolve_rel};
use crate::model::{CollectionFile, FolderFile, RequestFile, Settings};
use crate::watch::content_hash;

/// The owned files needed to run a request: the collection manifest, the folder
/// chain (outer→inner) and the request itself.
pub struct ChainFiles {
    pub collection: CollectionFile,
    pub folders: Vec<FolderFile>,
    pub request: RequestFile,
}

/// Load a request and every ancestor needed to resolve inheritance.
pub fn load_chain(root: &Path, rel: &str) -> Result<ChainFiles, CoreError> {
    let manifest_path = root.join(crate::fsops::COLLECTION_FILE);
    let collection = parse_collection(&read_text(&manifest_path)?, &manifest_path)?;

    // walk the rel path's directory segments, loading each folder.toml if present
    let mut folders = Vec::new();
    let segments: Vec<&str> = rel.split('/').collect();
    let mut acc = String::new();
    for seg in &segments[..segments.len().saturating_sub(1)] {
        if seg.is_empty() {
            continue;
        }
        if acc.is_empty() {
            acc = (*seg).to_string();
        } else {
            acc = format!("{acc}/{seg}");
        }
        let folder_toml = resolve_rel(root, &acc)?.join(FOLDER_FILE);
        if folder_toml.exists() {
            folders.push(parse_folder(&read_text(&folder_toml)?, &folder_toml)?);
        }
    }

    let req_path = resolve_rel(root, rel)?;
    let request = parse_request(&read_text(&req_path)?, &req_path)?;

    Ok(ChainFiles {
        collection,
        folders,
        request,
    })
}

/// Read a request plus the blake3 hash of its on-disk text (for conflict detection).
pub fn read_request_hashed(root: &Path, rel: &str) -> Result<(RequestFile, String), CoreError> {
    let path = resolve_rel(root, rel)?;
    let text = read_text(&path)?;
    let request = parse_request(&text, &path)?;
    Ok((request, content_hash(text.as_bytes())))
}

/// List environment names under `environments/`.
pub fn list_environments(root: &Path) -> Vec<String> {
    let dir = root.join(crate::fsops::ENVIRONMENTS_DIR);
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.strip_suffix(".toml") {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    names
}

pub fn load_settings(config_dir: &Path) -> Settings {
    let path = config_dir.join("settings.toml");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| parse_settings(&text, &path).ok())
        .unwrap_or_default()
}

pub fn save_settings(config_dir: &Path, settings: &Settings) -> Result<(), CoreError> {
    std::fs::create_dir_all(config_dir).map_err(|e| CoreError::io(config_dir, e))?;
    let path = config_dir.join("settings.toml");
    crate::fsops::atomic_write(&path, &settings_to_string(settings)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::{create_collection, create_folder, create_request};

    #[test]
    fn loads_the_full_inheritance_chain() {
        let tmp = tempfile::tempdir().unwrap();
        let root = create_collection(tmp.path(), "Acme").unwrap();
        let outer = create_folder(&root, "", "Users").unwrap();
        let inner = create_folder(&root, &outer, "Admins").unwrap();
        let rel = create_request(&root, &inner, "Ban user").unwrap();

        let chain = load_chain(&root, &rel).unwrap();
        assert_eq!(chain.collection.meta.name, "Acme");
        assert_eq!(chain.folders.len(), 2);
        assert_eq!(chain.folders[0].meta.name, "Users"); // outer first
        assert_eq!(chain.folders[1].meta.name, "Admins");
        assert_eq!(chain.request.meta.name, "Ban user");
    }

    #[test]
    fn read_request_hashed_is_stable() {
        let tmp = tempfile::tempdir().unwrap();
        let root = create_collection(tmp.path(), "C").unwrap();
        let rel = create_request(&root, "", "Ping").unwrap();
        let (_, h1) = read_request_hashed(&root, &rel).unwrap();
        let (_, h2) = read_request_hashed(&root, &rel).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn settings_round_trip_on_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Settings {
            theme: crate::model::Theme::Dark,
            ..Default::default()
        };
        save_settings(tmp.path(), &s).unwrap();
        assert_eq!(load_settings(tmp.path()).theme, crate::model::Theme::Dark);
        // missing file -> defaults
        assert_eq!(
            load_settings(&tmp.path().join("nope")).theme,
            crate::model::Theme::System
        );
    }
}
