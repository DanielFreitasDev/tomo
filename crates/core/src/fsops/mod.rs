//! Filesystem operations for collections: scanning, slugs, atomic writes,
//! CRUD node ops with path-traversal guards, and the managed .gitignore block.

pub mod atomic;
pub mod gitignore;
pub mod ops;
pub mod scan;
pub mod slug;

pub use atomic::{atomic_write, read_text};
pub use gitignore::upsert_gitignore;
pub use ops::{
    create_collection, create_folder, create_request, delete_node, duplicate_request, move_node,
    node_path_for_delete, rename_folder, rename_request, reorder_nodes, resolve_rel,
};
pub use scan::{CollectionTree, FolderNode, InvalidFile, Node, RequestNode, scan_collection};
pub use slug::{slugify, unique_slug};

/// File names with special meaning inside a collection (never request slugs).
pub const COLLECTION_FILE: &str = "collection.toml";
pub const FOLDER_FILE: &str = "folder.toml";
pub const SECRETS_FILE: &str = "secrets.toml";
pub const ENVIRONMENTS_DIR: &str = "environments";
