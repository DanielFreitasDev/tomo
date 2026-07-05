//! Bridge core watch events into Tauri events, re-scanning the tree and
//! re-parsing changed request files for the frontend reconciler.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tomo_core::facade::{list_environments, read_request_hashed};
use tomo_core::fsops::scan_collection;
use tomo_core::watch::{WatchEvent, WriteSuppressor, watch_collection};

use crate::dto::CollectionTreeDto;
use crate::state::CollectionRuntime;

#[derive(Serialize, Clone)]
struct TreeChangedPayload {
    id: String,
    tree: serde_json::Value,
}

#[derive(Serialize, Clone)]
struct FileChangedPayload {
    id: String,
    rel: String,
    hash: String,
    request: Option<serde_json::Value>,
}

pub fn start(app: AppHandle, id: String, runtime: Arc<CollectionRuntime>) {
    let root = runtime.root.clone();
    let suppressor: Arc<WriteSuppressor> = runtime.suppressor.clone();
    let selected = runtime.selected_env.lock().ok().and_then(|g| g.clone());

    let handler = move |event: WatchEvent| match event {
        WatchEvent::TreeChanged => {
            if let Ok(tree) = scan_collection(&root) {
                let envs = list_environments(&root);
                let dto = CollectionTreeDto::build(&id, &tree, envs, selected.clone());
                let _ = app.emit(
                    "watcher:tree-changed",
                    TreeChangedPayload {
                        id: id.clone(),
                        tree: serde_json::to_value(dto).unwrap_or(serde_json::Value::Null),
                    },
                );
            }
        }
        WatchEvent::FileChanged { rel, hash } => {
            let request = read_request_hashed(&root, &rel)
                .ok()
                .and_then(|(req, _)| serde_json::to_value(req).ok());
            let _ = app.emit(
                "watcher:file-changed",
                FileChangedPayload {
                    id: id.clone(),
                    rel,
                    hash,
                    request,
                },
            );
        }
    };

    if let Ok(watcher) = watch_collection(runtime.root.clone(), suppressor, handler)
        && let Ok(mut slot) = runtime._watcher.lock()
    {
        *slot = Some(watcher);
    }
}
