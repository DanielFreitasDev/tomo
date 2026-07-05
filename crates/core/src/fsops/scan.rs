//! Scan a collection directory into an ordered tree.
//!
//! Ordering: `seq` ascending, entries without `seq` last, ties broken by file
//! name — deterministic even after git merges. Files that fail to parse are
//! collected into `invalid` instead of failing the whole scan.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::{COLLECTION_FILE, ENVIRONMENTS_DIR, FOLDER_FILE, SECRETS_FILE};
use crate::CoreError;
use crate::format::{parse_collection, parse_folder, parse_request};
use crate::fsops::atomic::read_text;
use crate::model::CollectionFile;

#[derive(Debug, Clone, Serialize)]
pub struct CollectionTree {
    #[serde(skip)]
    pub root: PathBuf,
    pub collection: CollectionFile,
    pub nodes: Vec<Node>,
    pub invalid: Vec<InvalidFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Node {
    Folder(FolderNode),
    Request(RequestNode),
}

impl Node {
    pub fn rel(&self) -> &str {
        match self {
            Node::Folder(f) => &f.rel,
            Node::Request(r) => &r.rel,
        }
    }

    fn sort_key(&self) -> (i64, String) {
        let (seq, rel) = match self {
            Node::Folder(f) => (f.seq, &f.rel),
            Node::Request(r) => (r.seq, &r.rel),
        };
        let file_name = rel.rsplit('/').next().unwrap_or(rel).to_string();
        (seq.unwrap_or(i64::MAX), file_name)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FolderNode {
    /// '/'-separated path relative to the collection root.
    pub rel: String,
    pub name: String,
    pub seq: Option<i64>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestNode {
    pub rel: String,
    pub name: String,
    pub method: String,
    pub seq: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvalidFile {
    pub rel: String,
    pub error: String,
}

pub fn scan_collection(root: &Path) -> Result<CollectionTree, CoreError> {
    let manifest_path = root.join(COLLECTION_FILE);
    let manifest_text = read_text(&manifest_path)?;
    let collection = parse_collection(&manifest_text, &manifest_path)?;

    let mut invalid = Vec::new();
    let nodes = scan_dir(root, "", 0, &mut invalid)?;

    Ok(CollectionTree {
        root: root.to_path_buf(),
        collection,
        nodes,
        invalid,
    })
}

const MAX_DEPTH: usize = 32;

fn scan_dir(
    dir: &Path,
    rel_prefix: &str,
    depth: usize,
    invalid: &mut Vec<InvalidFile>,
) -> Result<Vec<Node>, CoreError> {
    if depth > MAX_DEPTH {
        return Ok(Vec::new());
    }
    let mut nodes = Vec::new();

    let entries = std::fs::read_dir(dir).map_err(|e| CoreError::io(dir, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| CoreError::io(dir, e))?;
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy().to_string();

        // hidden files, git internals
        if name_str.starts_with('.') {
            continue;
        }
        // symlinks are skipped defensively (loop protection)
        let meta = entry
            .metadata()
            .map_err(|e| CoreError::io(entry.path(), e))?;
        if entry
            .path()
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            continue;
        }

        let rel = if rel_prefix.is_empty() {
            name_str.clone()
        } else {
            format!("{rel_prefix}/{name_str}")
        };

        if meta.is_dir() {
            // the environments dir is special only at the collection root
            if depth == 0 && name_str == ENVIRONMENTS_DIR {
                continue;
            }
            let folder_toml = entry.path().join(FOLDER_FILE);
            let (name, seq) = if folder_toml.exists() {
                match read_text(&folder_toml).and_then(|t| parse_folder(&t, &folder_toml)) {
                    Ok(f) => (f.meta.name, f.meta.seq),
                    Err(e) => {
                        invalid.push(InvalidFile {
                            rel: format!("{rel}/{FOLDER_FILE}"),
                            error: e.to_string(),
                        });
                        (name_str.clone(), None)
                    }
                }
            } else {
                (name_str.clone(), None)
            };
            let children = scan_dir(&entry.path(), &rel, depth + 1, invalid)?;
            nodes.push(Node::Folder(FolderNode {
                rel,
                name,
                seq,
                children,
            }));
        } else if name_str.ends_with(".toml") {
            if name_str == COLLECTION_FILE || name_str == FOLDER_FILE || name_str == SECRETS_FILE {
                continue;
            }
            match read_text(&entry.path()).and_then(|t| parse_request(&t, &entry.path())) {
                Ok(req) => nodes.push(Node::Request(RequestNode {
                    rel,
                    name: req.meta.name,
                    method: req.http.method,
                    seq: req.meta.seq,
                })),
                Err(e) => invalid.push(InvalidFile {
                    rel,
                    error: e.to_string(),
                }),
            }
        }
    }

    nodes.sort_by_key(Node::sort_key);
    Ok(nodes)
}
