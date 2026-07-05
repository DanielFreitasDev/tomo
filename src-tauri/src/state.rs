//! App-wide state: open collections (each with its jar/token-cache/watcher),
//! in-flight request registry for cancellation, and a small LRU of response
//! bodies awaiting `get_response_body`.

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use dashmap::DashMap;
use lru::LruCache;
use tokio_util::sync::CancellationToken;
use tomo_core::http::{TokenCache, TomoJar};
use tomo_core::model::ResponseData;
use tomo_core::watch::{Watcher, WriteSuppressor};

/// A collection's live runtime: root path, cookie jar, token cache, runtime
/// vars set by scripts, watcher handle and self-write suppressor.
pub struct CollectionRuntime {
    pub root: PathBuf,
    pub jar: Arc<TomoJar>,
    pub token_cache: Arc<TokenCache>,
    pub runtime_vars: Mutex<indexmap::IndexMap<String, serde_json::Value>>,
    pub suppressor: Arc<WriteSuppressor>,
    pub selected_env: Mutex<Option<String>>,
    /// Held to keep the watch alive; dropped on close.
    pub _watcher: Mutex<Option<Watcher>>,
}

impl CollectionRuntime {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            jar: TomoJar::new(),
            token_cache: TokenCache::new(),
            runtime_vars: Mutex::new(indexmap::IndexMap::new()),
            suppressor: WriteSuppressor::new(),
            selected_env: Mutex::new(None),
            _watcher: Mutex::new(None),
        }
    }
}

pub struct RunHandle {
    pub token: CancellationToken,
}

pub struct AppState {
    /// Keyed by collection id (its canonical root path as a string).
    pub collections: DashMap<String, Arc<CollectionRuntime>>,
    pub runs: DashMap<String, RunHandle>,
    /// Completed response bodies parked for retrieval (bounded).
    pub bodies: Mutex<LruCache<String, Arc<ResponseData>>>,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl AppState {
    pub fn new(config_dir: PathBuf, cache_dir: PathBuf) -> Self {
        Self {
            collections: DashMap::new(),
            runs: DashMap::new(),
            bodies: Mutex::new(LruCache::new(NonZeroUsize::new(20).expect("nonzero"))),
            config_dir,
            cache_dir,
        }
    }

    pub fn collection(&self, id: &str) -> Result<Arc<CollectionRuntime>, crate::error::ApiError> {
        self.collections.get(id).map(|r| r.clone()).ok_or_else(|| {
            crate::error::ApiError::new("not_found", format!("collection not open: {id}"))
        })
    }
}
