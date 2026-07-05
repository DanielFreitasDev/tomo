//! Node CRUD with path-traversal guards. All `rel` paths are '/'-separated
//! and relative to the collection root; conversion to OS paths happens here.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::atomic::{atomic_write, read_text};
use super::gitignore::upsert_gitignore;
use super::slug::{slugify, unique_slug};
use super::{COLLECTION_FILE, FOLDER_FILE};
use crate::CoreError;
use crate::format::{
    collection_to_string, folder_to_string, parse_folder, parse_request, request_to_string,
    sync_folder, sync_request,
};
use crate::model::{CollectionFile, FolderFile, HttpDef, RequestFile, RequestMeta};

/// Resolve a wire-format rel path against the collection root, rejecting
/// traversal, absolute paths and backslashes.
pub fn resolve_rel(root: &Path, rel: &str) -> Result<PathBuf, CoreError> {
    if rel.contains('\\') {
        return Err(CoreError::Invalid(format!(
            "invalid path separator in `{rel}`"
        )));
    }
    if rel.starts_with('/') {
        return Err(CoreError::Invalid(format!(
            "absolute paths are not allowed: `{rel}`"
        )));
    }
    let mut path = root.to_path_buf();
    for seg in rel.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            return Err(CoreError::Invalid(format!(
                "path traversal rejected: `{rel}`"
            )));
        }
        path.push(seg);
    }
    Ok(path)
}

/// Slugs already taken inside a directory (file stems and dir names, case-folded).
fn taken_slugs(dir: &Path) -> HashSet<String> {
    let mut taken = HashSet::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            let stem = name.strip_suffix(".toml").unwrap_or(&name);
            taken.insert(stem.to_ascii_lowercase());
        }
    }
    taken
}

pub fn create_collection(parent_dir: &Path, name: &str) -> Result<PathBuf, CoreError> {
    let slug = unique_slug(name, &taken_slugs(parent_dir));
    let dir = parent_dir.join(&slug);
    std::fs::create_dir_all(&dir).map_err(|e| CoreError::io(&dir, e))?;

    let manifest = CollectionFile::new(name);
    atomic_write(
        &dir.join(COLLECTION_FILE),
        &collection_to_string(&manifest)?,
    )?;
    upsert_gitignore(&dir)?;
    Ok(dir)
}

pub fn create_folder(root: &Path, parent_rel: &str, name: &str) -> Result<String, CoreError> {
    let parent = resolve_rel(root, parent_rel)?;
    let slug = unique_slug(name, &taken_slugs(&parent));
    let dir = parent.join(&slug);
    std::fs::create_dir_all(&dir).map_err(|e| CoreError::io(&dir, e))?;

    let folder = FolderFile::new(name);
    atomic_write(&dir.join(FOLDER_FILE), &folder_to_string(&folder)?)?;
    Ok(join_rel(parent_rel, &slug))
}

pub fn create_request(root: &Path, parent_rel: &str, name: &str) -> Result<String, CoreError> {
    let parent = resolve_rel(root, parent_rel)?;
    std::fs::create_dir_all(&parent).map_err(|e| CoreError::io(&parent, e))?;
    let slug = unique_slug(name, &taken_slugs(&parent));

    let req = RequestFile {
        meta: RequestMeta {
            name: name.to_string(),
            seq: None,
        },
        http: HttpDef {
            method: "GET".into(),
            url: String::new(),
            ..Default::default()
        },
        ..Default::default()
    };
    let file = parent.join(format!("{slug}.toml"));
    atomic_write(&file, &request_to_string(&req)?)?;
    Ok(join_rel(parent_rel, &format!("{slug}.toml")))
}

/// Rename a request: updates `meta.name` surgically and re-slugs the file name.
/// Returns the (possibly unchanged) new rel path.
pub fn rename_request(root: &Path, rel: &str, new_name: &str) -> Result<String, CoreError> {
    let path = resolve_rel(root, rel)?;
    let text = read_text(&path)?;
    let mut req = parse_request(&text, &path)?;
    req.meta.name = new_name.to_string();
    let synced = sync_request(&text, &req, &path)?;

    let parent = path.parent().expect("request file has a parent");
    let current_stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let desired = slugify(new_name);
    let new_stem = if desired.eq_ignore_ascii_case(&current_stem) {
        current_stem.clone()
    } else {
        let mut taken = taken_slugs(parent);
        taken.remove(&current_stem.to_ascii_lowercase());
        unique_slug(new_name, &taken)
    };

    let new_path = parent.join(format!("{new_stem}.toml"));
    atomic_write(&new_path, &synced)?;
    if new_path != path {
        std::fs::remove_file(&path).map_err(|e| CoreError::io(&path, e))?;
    }
    Ok(replace_last_segment(rel, &format!("{new_stem}.toml")))
}

/// Rename a folder: updates folder.toml (created if missing) and re-slugs the dir.
pub fn rename_folder(root: &Path, rel: &str, new_name: &str) -> Result<String, CoreError> {
    let dir = resolve_rel(root, rel)?;
    if !dir.is_dir() {
        return Err(CoreError::Invalid(format!("not a folder: `{rel}`")));
    }
    let folder_toml = dir.join(FOLDER_FILE);
    if folder_toml.exists() {
        let text = read_text(&folder_toml)?;
        let mut folder = parse_folder(&text, &folder_toml)?;
        folder.meta.name = new_name.to_string();
        atomic_write(&folder_toml, &sync_folder(&text, &folder, &folder_toml)?)?;
    } else {
        atomic_write(&folder_toml, &folder_to_string(&FolderFile::new(new_name))?)?;
    }

    let parent = dir.parent().expect("folder has a parent");
    let current = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let desired = slugify(new_name);
    if desired.eq_ignore_ascii_case(&current) {
        return Ok(rel.to_string());
    }
    let mut taken = taken_slugs(parent);
    taken.remove(&current.to_ascii_lowercase());
    let new_slug = unique_slug(new_name, &taken);
    let new_dir = parent.join(&new_slug);
    std::fs::rename(&dir, &new_dir).map_err(|e| CoreError::io(&dir, e))?;
    Ok(replace_last_segment(rel, &new_slug))
}

/// Move a node (request file or folder) into another folder. Returns new rel.
pub fn move_node(root: &Path, rel: &str, new_parent_rel: &str) -> Result<String, CoreError> {
    let src = resolve_rel(root, rel)?;
    let dst_dir = resolve_rel(root, new_parent_rel)?;
    if !dst_dir.is_dir() {
        return Err(CoreError::Invalid(format!(
            "target folder does not exist: `{new_parent_rel}`"
        )));
    }
    let file_name = src
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .ok_or_else(|| CoreError::Invalid("cannot move the collection root".into()))?;

    // moving a folder into itself/descendant would corrupt the tree
    if src.is_dir() && dst_dir.starts_with(&src) {
        return Err(CoreError::Invalid(
            "cannot move a folder into itself".into(),
        ));
    }

    let stem = file_name
        .strip_suffix(".toml")
        .unwrap_or(&file_name)
        .to_string();
    let mut taken = taken_slugs(&dst_dir);
    let final_name = if taken.contains(&stem.to_ascii_lowercase()) {
        let unique = unique_slug(&stem, &taken);
        taken.insert(unique.to_ascii_lowercase());
        if file_name.ends_with(".toml") {
            format!("{unique}.toml")
        } else {
            unique
        }
    } else {
        file_name.clone()
    };

    let dst = dst_dir.join(&final_name);
    std::fs::rename(&src, &dst).map_err(|e| CoreError::io(&src, e))?;
    Ok(join_rel(new_parent_rel, &final_name))
}

/// Duplicate a request file next to itself as "<Name> (copy)".
pub fn duplicate_request(root: &Path, rel: &str) -> Result<String, CoreError> {
    let path = resolve_rel(root, rel)?;
    let text = read_text(&path)?;
    let mut req = parse_request(&text, &path)?;
    req.meta.name = format!("{} (copy)", req.meta.name);

    let parent = path.parent().expect("request file has a parent");
    let slug = unique_slug(&req.meta.name, &taken_slugs(parent));
    let new_path = parent.join(format!("{slug}.toml"));
    atomic_write(&new_path, &sync_request(&text, &req, &path)?)?;
    Ok(replace_last_segment(rel, &format!("{slug}.toml")))
}

pub fn delete_node(root: &Path, rel: &str) -> Result<(), CoreError> {
    if rel.trim_matches('/').is_empty() {
        return Err(CoreError::Invalid(
            "cannot delete the collection root".into(),
        ));
    }
    let path = resolve_rel(root, rel)?;
    if path == root {
        return Err(CoreError::Invalid(
            "cannot delete the collection root".into(),
        ));
    }
    if path.is_dir() {
        std::fs::remove_dir_all(&path).map_err(|e| CoreError::io(&path, e))
    } else {
        std::fs::remove_file(&path).map_err(|e| CoreError::io(&path, e))
    }
}

/// Rewrite `seq` so siblings appear in the given order (1-based). Only files
/// whose seq actually changes are touched.
pub fn reorder_nodes(root: &Path, ordered_rels: &[String]) -> Result<(), CoreError> {
    for (i, rel) in ordered_rels.iter().enumerate() {
        let seq = (i + 1) as i64;
        let path = resolve_rel(root, rel)?;
        if path.is_dir() {
            let folder_toml = path.join(FOLDER_FILE);
            if folder_toml.exists() {
                let text = read_text(&folder_toml)?;
                let mut folder = parse_folder(&text, &folder_toml)?;
                if folder.meta.seq != Some(seq) {
                    folder.meta.seq = Some(seq);
                    atomic_write(&folder_toml, &sync_folder(&text, &folder, &folder_toml)?)?;
                }
            } else {
                let dir_name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let mut folder = FolderFile::new(dir_name);
                folder.meta.seq = Some(seq);
                atomic_write(&folder_toml, &folder_to_string(&folder)?)?;
            }
        } else {
            let text = read_text(&path)?;
            let mut req = parse_request(&text, &path)?;
            if req.meta.seq != Some(seq) {
                req.meta.seq = Some(seq);
                atomic_write(&path, &sync_request(&text, &req, &path)?)?;
            }
        }
    }
    Ok(())
}

fn join_rel(parent: &str, child: &str) -> String {
    let parent = parent.trim_matches('/');
    if parent.is_empty() {
        child.to_string()
    } else {
        format!("{parent}/{child}")
    }
}

fn replace_last_segment(rel: &str, new_last: &str) -> String {
    match rel.rsplit_once('/') {
        Some((parent, _)) => format!("{parent}/{new_last}"),
        None => new_last.to_string(),
    }
}
