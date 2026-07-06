//! All #[tauri::command] handlers. Thin: each resolves state, calls tomo-core,
//! and shapes DTOs. Self-writes register with the suppressor so the watcher
//! doesn't echo them back.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;
use tomo_core::facade::{
    ChainFiles, list_environments, load_chain, load_settings, read_request_hashed,
    save_settings as facade_save_settings,
};
use tomo_core::format::{
    environment_to_string, parse_environment, parse_secrets, request_to_string, secrets_to_string,
    sync_environment, sync_request,
};
use tomo_core::fsops::{
    self, COLLECTION_FILE, ENVIRONMENTS_DIR, SECRETS_FILE, atomic_write, read_text, resolve_rel,
    scan_collection,
};
use tomo_core::http::{Chain, EngineConfig, RunSpec, execute};
use tomo_core::model::{EnvironmentFile, RequestFile, ResponseData, SecretsFile, Settings};
use tomo_core::vars::{StackInputs, VarStack, process_env_snapshot};
use tomo_core::watch::content_hash;

use crate::dto::{CollectionTreeDto, ReadRequestDto, ResponseMetaDto, SaveResultDto};
use crate::error::{ApiError, ApiResult};
use crate::state::{AppState, CollectionRuntime, RunHandle};

fn to_value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

fn from_value<T: serde::de::DeserializeOwned>(v: Value) -> ApiResult<T> {
    serde_json::from_value(v).map_err(|e| ApiError::new("invalid", format!("bad payload: {e}")))
}

fn tree_dto(id: &str, runtime: &CollectionRuntime) -> ApiResult<CollectionTreeDto> {
    let tree = scan_collection(&runtime.root)?;
    let envs = list_environments(&runtime.root);
    let selected = runtime.selected_env.lock().ok().and_then(|g| g.clone());
    Ok(CollectionTreeDto::build(id, &tree, envs, selected))
}

/// Resolve an environment file path from a user-supplied name, guarding against
/// path traversal — the name is a bare file stem, never a rel path. Unlike
/// `rel` (guarded by `resolve_rel`), env names reach the fs join directly.
fn env_file_path(root: &Path, name: &str) -> ApiResult<PathBuf> {
    let bad = name.is_empty()
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':' | '\0') || c.is_control());
    if bad {
        return Err(ApiError::new(
            "invalid",
            format!("invalid environment name: {name}"),
        ));
    }
    Ok(root.join(ENVIRONMENTS_DIR).join(format!("{name}.toml")))
}

// ---------------------------------------------------------------------------
// collections
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub async fn pick_collection_folder(app: AppHandle) -> ApiResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog().file().pick_folder(move |path| {
        let _ = tx.send(path);
    });
    let picked = rx
        .recv()
        .map_err(|_| ApiError::new("dialog", "folder picker closed"))?;
    Ok(picked.map(|p| p.to_string()))
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct RecentEntry {
    pub path: String,
    pub name: String,
}

fn recents_path(state: &AppState) -> PathBuf {
    state.config_dir.join("recents.json")
}

fn load_recents(state: &AppState) -> Vec<RecentEntry> {
    std::fs::read_to_string(recents_path(state))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn store_recents(state: &AppState, recents: &[RecentEntry]) -> ApiResult<()> {
    std::fs::create_dir_all(&state.config_dir)
        .map_err(|e| ApiError::from(tomo_core::CoreError::io(&state.config_dir, e)))?;
    let path = recents_path(state);
    let text = serde_json::to_string_pretty(recents)
        .map_err(|e| ApiError::new("invalid", format!("failed to serialize recents: {e}")))?;
    atomic_write(&path, &text)?;
    Ok(())
}

fn touch_recent_collection(state: &AppState, path: &str, name: &str) -> ApiResult<()> {
    let mut recents = load_recents(state)
        .into_iter()
        .filter(|entry| entry.path != path)
        .collect::<Vec<_>>();
    recents.insert(
        0,
        RecentEntry {
            path: path.to_string(),
            name: name.to_string(),
        },
    );
    recents.truncate(12);
    store_recents(state, &recents)
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_recent_collections(state: State<'_, AppState>) -> ApiResult<Vec<RecentEntry>> {
    Ok(load_recents(&state))
}

#[tauri::command(rename_all = "snake_case")]
pub fn open_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> ApiResult<CollectionTreeDto> {
    let root = dunce::canonicalize(&path).unwrap_or_else(|_| PathBuf::from(&path));
    let id = root.to_string_lossy().to_string();

    if !root.join(COLLECTION_FILE).exists() {
        return Err(ApiError::new(
            "not_found",
            format!("no {COLLECTION_FILE} in {id}"),
        ));
    }

    let runtime = Arc::new(CollectionRuntime::new(root));
    state.collections.insert(id.clone(), runtime.clone());
    crate::watchbridge::start(app, id.clone(), runtime.clone());
    let dto = tree_dto(&id, &runtime)?;
    touch_recent_collection(&state, &id, &dto.name)?;
    Ok(dto)
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    parent_dir: String,
    name: String,
) -> ApiResult<CollectionTreeDto> {
    let root = fsops::create_collection(&PathBuf::from(parent_dir), &name)?;
    let id = dunce::canonicalize(&root)
        .unwrap_or(root)
        .to_string_lossy()
        .to_string();
    open_collection(app, state, id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn close_collection(state: State<'_, AppState>, id: String) -> ApiResult<()> {
    state.collections.remove(&id);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn reload_collection(state: State<'_, AppState>, id: String) -> ApiResult<CollectionTreeDto> {
    let runtime = state.collection(&id)?;
    tree_dto(&id, &runtime)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn pick_save_file(
    app: AppHandle,
    default_name: Option<String>,
) -> ApiResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    let builder = match default_name {
        Some(name) if !name.trim().is_empty() => app.dialog().file().set_file_name(name),
        _ => app.dialog().file(),
    };
    builder.save_file(move |path| {
        let _ = tx.send(path);
    });
    let picked = rx
        .recv()
        .map_err(|_| ApiError::new("dialog", "save picker closed"))?;
    Ok(picked.map(|p| p.to_string()))
}

// ---------------------------------------------------------------------------
// nodes
// ---------------------------------------------------------------------------

fn register_write(runtime: &CollectionRuntime, rel: &str, text: &str) {
    if let Ok(path) = resolve_rel(&runtime.root, rel) {
        runtime.suppressor.register(&path, text);
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_folder(
    state: State<'_, AppState>,
    id: String,
    parent_rel: String,
    name: String,
) -> ApiResult<String> {
    let runtime = state.collection(&id)?;
    Ok(fsops::create_folder(&runtime.root, &parent_rel, &name)?)
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_request(
    state: State<'_, AppState>,
    id: String,
    parent_rel: String,
    name: String,
) -> ApiResult<String> {
    let runtime = state.collection(&id)?;
    Ok(fsops::create_request(&runtime.root, &parent_rel, &name)?)
}

#[tauri::command(rename_all = "snake_case")]
pub fn read_request(
    state: State<'_, AppState>,
    id: String,
    rel: String,
) -> ApiResult<ReadRequestDto> {
    let runtime = state.collection(&id)?;
    let (request, hash) = read_request_hashed(&runtime.root, &rel)?;
    Ok(ReadRequestDto {
        request: to_value(&request),
        hash,
    })
}

/// Read the on-disk text for a save. A missing file is a legitimate "save as
/// new" (`Ok(None)`). Any OTHER read error — permission, invalid UTF-8 from a
/// hand edit, a transient lock — is a hard error, never silently treated as an
/// empty file (which used to skip the conflict check and clobber disk).
fn read_existing_for_save(path: &Path) -> ApiResult<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ApiError::new(
            "io",
            format!("cannot read {} to save it safely: {e}", path.display()),
        )),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_request(
    state: State<'_, AppState>,
    id: String,
    rel: String,
    request: Value,
    base_hash: Option<String>,
) -> ApiResult<SaveResultDto> {
    let runtime = state.collection(&id)?;
    let req: RequestFile = from_value(request)?;
    let path = resolve_rel(&runtime.root, &rel)?;

    let existing = read_existing_for_save(&path)?;

    if let (Some(base), Some(text)) = (&base_hash, &existing) {
        let current_hash = content_hash(text.as_bytes());
        // Covers external truncation too: an existing-but-empty file whose hash
        // no longer matches the tab's base is a conflict, not a silent overwrite.
        if base != &current_hash {
            return Ok(SaveResultDto::Conflict {
                current_text: text.clone(),
                current_hash,
            });
        }
    }

    // surgical edit if the file exists with content, canonical write otherwise
    let out = match &existing {
        Some(text) if !text.is_empty() => sync_request(text, &req, &path)?,
        _ => request_to_string(&req)?,
    };
    register_write(&runtime, &rel, &out);
    atomic_write(&path, &out)?;
    Ok(SaveResultDto::Saved {
        hash: content_hash(out.as_bytes()),
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn rename_node(
    state: State<'_, AppState>,
    id: String,
    rel: String,
    new_name: String,
    kind: String,
) -> ApiResult<String> {
    let runtime = state.collection(&id)?;
    Ok(if kind == "folder" {
        fsops::rename_folder(&runtime.root, &rel, &new_name)?
    } else {
        fsops::rename_request(&runtime.root, &rel, &new_name)?
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn move_node(
    state: State<'_, AppState>,
    id: String,
    rel: String,
    new_parent_rel: String,
) -> ApiResult<String> {
    let runtime = state.collection(&id)?;
    Ok(fsops::move_node(&runtime.root, &rel, &new_parent_rel)?)
}

#[tauri::command(rename_all = "snake_case")]
pub fn duplicate_request(state: State<'_, AppState>, id: String, rel: String) -> ApiResult<String> {
    let runtime = state.collection(&id)?;
    Ok(fsops::duplicate_request(&runtime.root, &rel)?)
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_node(state: State<'_, AppState>, id: String, rel: String) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    // Move to the OS trash instead of an unrecoverable remove_dir_all: a mis-click
    // in the tree should be undoable from the system trash, not permanent.
    let path = fsops::node_path_for_delete(&runtime.root, &rel)?;
    trash::delete(&path).map_err(|e| {
        ApiError::new(
            "io",
            format!("could not move {} to trash: {e}", path.display()),
        )
    })?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn reorder_nodes(
    state: State<'_, AppState>,
    id: String,
    ordered_rels: Vec<String>,
) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    Ok(fsops::reorder_nodes(&runtime.root, &ordered_rels)?)
}

// ---------------------------------------------------------------------------
// environments & secrets
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn read_environment(state: State<'_, AppState>, id: String, name: String) -> ApiResult<Value> {
    let runtime = state.collection(&id)?;
    let path = env_file_path(&runtime.root, &name)?;
    let env = parse_environment(&read_text(&path)?, &path)?;
    Ok(to_value(&env))
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_environment(
    state: State<'_, AppState>,
    id: String,
    name: String,
    env: Value,
    previous_name: Option<String>,
) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    let env: EnvironmentFile = from_value(env)?;
    let path = env_file_path(&runtime.root, &name)?;

    let text = match read_text(&path) {
        Ok(existing) => sync_environment(&existing, &env, &path)?,
        Err(_) => environment_to_string(&env)?,
    };
    runtime.suppressor.register(&path, &text);
    atomic_write(&path, &text)?;
    if let Some(prev) = previous_name
        && prev != name
    {
        let prev_path = env_file_path(&runtime.root, &prev)?;
        let _ = std::fs::remove_file(prev_path);
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_environment(state: State<'_, AppState>, id: String, name: String) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    let path = env_file_path(&runtime.root, &name)?;
    std::fs::remove_file(&path).map_err(|e| ApiError::from(tomo_core::CoreError::io(&path, e)))
}

#[tauri::command(rename_all = "snake_case")]
pub fn select_environment(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    if let Ok(mut slot) = runtime.selected_env.lock() {
        *slot = name;
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn read_secrets(state: State<'_, AppState>, id: String) -> ApiResult<Value> {
    let runtime = state.collection(&id)?;
    let path = runtime.root.join(SECRETS_FILE);
    let secrets = match read_text(&path) {
        Ok(text) => parse_secrets(&text, &path)?,
        Err(_) => SecretsFile::default(),
    };
    Ok(to_value(&secrets))
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_secrets(state: State<'_, AppState>, id: String, secrets: Value) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    let secrets: SecretsFile = from_value(secrets)?;
    // gitignore MUST be updated before secrets ever hit disk
    fsops::upsert_gitignore(&runtime.root)?;
    let path = runtime.root.join(SECRETS_FILE);
    let text = secrets_to_string(&secrets);
    runtime.suppressor.register(&path, &text);
    atomic_write(&path, &text)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// http
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, Clone)]
struct StartedPayload {
    run_id: String,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn send_request(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    rel: String,
    run_id: String,
    draft: Option<Value>,
    env: Option<String>,
) -> ApiResult<ResponseMetaDto> {
    let runtime = state.collection(&id)?;

    // load the inheritance chain, overriding the request with the live draft
    let ChainFiles {
        collection,
        folders,
        request: disk_request,
    } = load_chain(&runtime.root, &rel)?;
    let request: RequestFile = match draft {
        Some(d) => from_value(d)?,
        None => disk_request,
    };

    // environment + secrets + dotenv snapshots
    let environment = match &env {
        Some(name) => env_file_path(&runtime.root, name).ok().and_then(|path| {
            read_text(&path)
                .ok()
                .and_then(|t| parse_environment(&t, &path).ok())
        }),
        None => None,
    };
    let secrets = {
        let path = runtime.root.join(SECRETS_FILE);
        read_text(&path)
            .ok()
            .and_then(|t| parse_secrets(&t, &path).ok())
    };
    let dotenv = tomo_core::vars::load_dotenv(&runtime.root);
    let runtime_vars = runtime
        .runtime_vars
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_default();

    let settings = load_settings(&state.config_dir);
    let cfg = EngineConfig {
        network: settings.network.clone(),
        spill_dir: state.cache_dir.join("bodies"),
    };

    // register the cancellation token BEFORE awaiting (UI-generated run_id)
    let token = CancellationToken::new();
    state.runs.insert(
        run_id.clone(),
        RunHandle {
            token: token.clone(),
        },
    );
    let _ = app.emit(
        "request:started",
        StartedPayload {
            run_id: run_id.clone(),
        },
    );

    let result = execute(
        &cfg,
        RunSpec {
            chain: Chain {
                collection: &collection,
                folders: folders.iter().collect(),
                request: &request,
            },
            environment: environment.as_ref(),
            secrets: secrets.as_ref(),
            runtime_vars: Some(&runtime_vars),
            process_env: process_env_snapshot(),
            dotenv,
            collection_root: &runtime.root,
            jar: runtime.jar.clone(),
            token_cache: runtime.token_cache.clone(),
            cancel: token,
        },
    )
    .await;

    state.runs.remove(&run_id);

    match result {
        Ok(data) => {
            // persist runtime vars the scripts set
            if let Ok(mut vars) = runtime.runtime_vars.lock() {
                for (k, v) in &data.runtime_sets {
                    vars.insert(k.clone(), v.clone());
                }
            }
            let cookies = to_value(&runtime.jar.list());
            let meta = ResponseMetaDto::from_data(&data, cookies);
            // park the full body (incl. bytes) for get_response_body
            if let Ok(mut cache) = state.bodies.lock() {
                // push returns the evicted entry (LRU capacity or same key);
                // delete its spill file so full bodies don't accumulate on disk.
                // Skip if another reader still holds it (a concurrent download).
                if let Some((_, evicted)) = cache.push(run_id.clone(), Arc::new(data))
                    && Arc::strong_count(&evicted) == 1
                    && let Some(path) = &evicted.body.spill_path
                {
                    let _ = std::fs::remove_file(path);
                }
            }
            let _ = app.emit(
                "request:completed",
                serde_json::json!({ "run_id": run_id, "meta": &meta }),
            );
            Ok(meta)
        }
        Err(tomo_core::CoreError::Cancelled) => {
            let _ = app.emit("request:cancelled", StartedPayload { run_id });
            Err(ApiError::new("cancelled", "request cancelled"))
        }
        Err(e) => {
            let api = ApiError::from(e);
            let _ = app.emit(
                "request:failed",
                serde_json::json!({ "run_id": run_id, "error": api.message }),
            );
            Err(api)
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn cancel_request(state: State<'_, AppState>, run_id: String) -> ApiResult<bool> {
    if let Some((_, handle)) = state.runs.remove(&run_id) {
        handle.token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

fn response_body_preview(data: &ResponseData) -> Vec<u8> {
    data.body.bytes.clone()
}

fn save_response_body_data(data: &ResponseData, dest: &Path) -> ApiResult<()> {
    if let Some(spill) = &data.body.spill_path {
        std::fs::copy(spill, dest)
            .map(|_| ())
            .map_err(|e| ApiError::from(tomo_core::CoreError::io(dest, e)))
    } else {
        std::fs::write(dest, &data.body.bytes)
            .map_err(|e| ApiError::from(tomo_core::CoreError::io(dest, e)))
    }
}

/// Raw response preview bytes — never travel through JSON. Large responses
/// return only the in-memory preview; use `save_response_body` for full bodies.
#[tauri::command(rename_all = "snake_case")]
pub fn get_response_body(state: State<'_, AppState>, run_id: String) -> tauri::ipc::Response {
    let bytes = state
        .bodies
        .lock()
        .ok()
        .and_then(|cache| cache.peek(&run_id).map(|d| response_body_preview(d)))
        .unwrap_or_default();
    tauri::ipc::Response::new(bytes)
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_response_body(
    state: State<'_, AppState>,
    run_id: String,
    dest: String,
) -> ApiResult<()> {
    let data = state
        .bodies
        .lock()
        .ok()
        .and_then(|cache| cache.peek(&run_id).cloned())
        .ok_or_else(|| ApiError::new("not_found", "response body no longer cached"))?;
    let dest = PathBuf::from(dest);
    save_response_body_data(&data, &dest)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_cookies(state: State<'_, AppState>, id: String) -> ApiResult<Value> {
    let runtime = state.collection(&id)?;
    Ok(to_value(&runtime.jar.list()))
}

#[tauri::command(rename_all = "snake_case")]
pub fn clear_cookies(
    state: State<'_, AppState>,
    id: String,
    domain: Option<String>,
) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    runtime.jar.clear(domain.as_deref());
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_runtime_vars(state: State<'_, AppState>, id: String) -> ApiResult<Value> {
    let runtime = state.collection(&id)?;
    Ok(runtime
        .runtime_vars
        .lock()
        .map(|g| to_value(&*g))
        .unwrap_or(Value::Null))
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_runtime_var(
    state: State<'_, AppState>,
    id: String,
    key: String,
    value: Value,
) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    if let Ok(mut vars) = runtime.runtime_vars.lock() {
        vars.insert(key, value);
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn clear_runtime_vars(state: State<'_, AppState>, id: String) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    if let Ok(mut vars) = runtime.runtime_vars.lock() {
        vars.clear();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// curl
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn import_curl(text: String) -> ApiResult<Value> {
    Ok(to_value(&tomo_core::curl::from_curl(&text)?))
}

#[tauri::command(rename_all = "snake_case")]
pub fn export_curl(
    state: State<'_, AppState>,
    id: String,
    rel: String,
    draft: Option<Value>,
    interpolated: bool,
) -> ApiResult<String> {
    let runtime = state.collection(&id)?;
    let ChainFiles {
        collection,
        folders,
        request: disk_request,
    } = load_chain(&runtime.root, &rel)?;
    let request: RequestFile = match draft {
        Some(d) => from_value(d)?,
        None => disk_request,
    };
    if !interpolated {
        return Ok(tomo_core::curl::to_curl(&request));
    }

    let selected_env = runtime.selected_env.lock().ok().and_then(|g| g.clone());
    let environment = selected_env.as_ref().and_then(|name| {
        let path = env_file_path(&runtime.root, name).ok()?;
        read_text(&path)
            .ok()
            .and_then(|t| parse_environment(&t, &path).ok())
    });
    let secrets = {
        let path = runtime.root.join(SECRETS_FILE);
        read_text(&path)
            .ok()
            .and_then(|t| parse_secrets(&t, &path).ok())
    };
    let dotenv = tomo_core::vars::load_dotenv(&runtime.root);
    let runtime_vars = runtime
        .runtime_vars
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_default();
    let folder_vars = folders.iter().map(|folder| &folder.vars).collect();
    let stack = VarStack::build(StackInputs {
        process_env: process_env_snapshot(),
        dotenv,
        collection_vars: Some(&collection.vars),
        environment: environment.as_ref(),
        secrets: secrets.as_ref(),
        folder_vars,
        request_vars: Some(&request.vars),
        runtime_vars: Some(&runtime_vars),
    });
    Ok(tomo_core::curl::to_curl_interpolated(&request, &stack))
}

// ---------------------------------------------------------------------------
// settings & ui state
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn get_settings(state: State<'_, AppState>) -> ApiResult<Value> {
    Ok(to_value(&load_settings(&state.config_dir)))
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_settings(state: State<'_, AppState>, settings: Value) -> ApiResult<()> {
    let settings: Settings = from_value(settings)?;
    facade_save_settings(&state.config_dir, &settings)?;
    Ok(())
}

fn ui_state_path(state: &AppState, id: &Option<String>) -> PathBuf {
    let name = match id {
        Some(id) => format!("ui-{}.json", content_hash(id.as_bytes())),
        None => "ui-app.json".to_string(),
    };
    state.config_dir.join(name)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_ui_state(state: State<'_, AppState>, id: Option<String>) -> ApiResult<Value> {
    let path = ui_state_path(&state, &id);
    Ok(std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null))
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_ui_state(
    state: State<'_, AppState>,
    id: Option<String>,
    state_json: Value,
) -> ApiResult<()> {
    std::fs::create_dir_all(&state.config_dir)
        .map_err(|e| ApiError::from(tomo_core::CoreError::io(&state.config_dir, e)))?;
    let path = ui_state_path(&state, &id);
    let text = serde_json::to_string(&state_json).unwrap_or_default();
    atomic_write(&path, &text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        env_file_path, read_existing_for_save, response_body_preview, save_response_body_data,
    };
    use tomo_core::model::{BodyCapture, ResponseData, Timing};

    #[test]
    fn read_existing_for_save_distinguishes_missing_from_unreadable() {
        let dir = tempfile::tempdir().unwrap();

        // missing file -> save-as-new, not an error
        let missing = dir.path().join("nope.toml");
        assert_eq!(read_existing_for_save(&missing).unwrap(), None);

        // present file -> its content
        let file = dir.path().join("r.toml");
        std::fs::write(&file, "name = \"x\"\n").unwrap();
        assert_eq!(
            read_existing_for_save(&file).unwrap().as_deref(),
            Some("name = \"x\"\n")
        );

        // unreadable (a directory stands in for any non-NotFound error) must be
        // an error, never silently "empty" — that used to clobber disk.
        assert!(read_existing_for_save(dir.path()).is_err());
    }

    fn response(bytes: Vec<u8>, total_size: u64, truncated: bool) -> ResponseData {
        ResponseData {
            status: 200,
            status_text: "OK".to_string(),
            http_version: "HTTP/1.1".to_string(),
            headers: Vec::new(),
            final_url: "https://api.test".to_string(),
            timing: Timing::default(),
            body: BodyCapture {
                bytes,
                total_size,
                truncated,
                ..Default::default()
            },
            warnings: Vec::new(),
            console: Vec::new(),
            tests: Vec::new(),
            asserts: Vec::new(),
            script_error: None,
            runtime_sets: indexmap::IndexMap::new(),
        }
    }

    #[test]
    fn get_response_body_contract_is_preview_bytes() {
        let data = response(b"preview".to_vec(), 13, true);

        assert_eq!(response_body_preview(&data), b"preview");
    }

    #[test]
    fn save_response_body_writes_small_body_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("response.bin");
        let data = response(b"complete".to_vec(), 8, false);

        save_response_body_data(&data, &dest).unwrap();

        assert_eq!(std::fs::read(dest).unwrap(), b"complete");
    }

    #[test]
    fn save_response_body_copies_spilled_full_body() {
        let tmp = tempfile::tempdir().unwrap();
        let spill = tmp.path().join("spill.bin");
        let dest = tmp.path().join("response.bin");
        std::fs::write(&spill, b"preview-and-tail").unwrap();
        let mut data = response(b"preview".to_vec(), 16, true);
        data.body.spill_path = Some(spill);

        save_response_body_data(&data, &dest).unwrap();

        assert_eq!(std::fs::read(dest).unwrap(), b"preview-and-tail");
    }

    #[test]
    fn env_file_path_blocks_traversal() {
        let root = std::path::Path::new("/tmp/col");
        assert!(env_file_path(root, "dev").is_ok());
        assert!(env_file_path(root, "../../etc/passwd").is_err());
        assert!(env_file_path(root, "a/b").is_err());
        assert!(env_file_path(root, "..").is_err());
        assert!(env_file_path(root, "").is_err());
        assert!(env_file_path(root, "C:evil").is_err());
        assert!(
            env_file_path(root, "prod")
                .unwrap()
                .ends_with("environments/prod.toml")
        );
    }
}

/// Exercises commands through the REAL Tauri IPC pipeline (MockRuntime), which
/// serializes/deserializes args exactly like the shipped app. This is the layer
/// the in-memory mock transport bypasses — the gap that let the snake_case vs
/// camelCase arg-casing bug ship green. The wire contract is snake_case.
#[cfg(test)]
mod ipc_tests {
    use serde_json::json;
    use tauri::ipc::{CallbackFn, InvokeResponseBody};
    use tauri::test::{INVOKE_KEY, MockRuntime, mock_builder, mock_context, noop_assets};
    use tauri::webview::InvokeRequest;
    use tauri::{WebviewWindow, WebviewWindowBuilder};

    use crate::state::AppState;

    fn test_webview() -> WebviewWindow<MockRuntime> {
        let tmp = std::env::temp_dir().join("tomo-ipc-tests");
        let app = mock_builder()
            .manage(AppState::new(tmp.join("config"), tmp.join("cache")))
            .invoke_handler(tauri::generate_handler![
                crate::commands::cancel_request,
                crate::commands::create_request
            ])
            .build(mock_context(noop_assets()))
            .expect("mock app builds");
        WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview builds")
    }

    fn invoke(
        webview: &WebviewWindow<MockRuntime>,
        cmd: &str,
        body: serde_json::Value,
    ) -> Result<InvokeResponseBody, serde_json::Value> {
        tauri::test::get_ipc_response(
            webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: body.into(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
    }

    // Single-word args (`id`, `rel`) were never affected; a multi-word arg is
    // the regression surface. `run_id` must arrive as snake_case and reach the
    // handler (no matching run -> Ok(false)).
    #[test]
    fn cancel_request_accepts_snake_case_run_id() {
        let wv = test_webview();
        let body = invoke(&wv, "cancel_request", json!({ "run_id": "no-such-run" }))
            .expect("snake_case run_id must deserialize and reach the handler");
        assert!(!body.deserialize::<bool>().unwrap());
    }

    // The contract is snake_case; a camelCase key must NOT satisfy the arg —
    // proving args are bound by name, not silently defaulted. This assertion
    // fails against the original (unfixed) code, where camelCase was required.
    #[test]
    fn cancel_request_rejects_camel_case_run_id() {
        let wv = test_webview();
        let err = invoke(&wv, "cancel_request", json!({ "runId": "x" }))
            .expect_err("camelCase runId must not satisfy the snake_case `run_id` arg");
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("run_id") || msg.contains("missing") || msg.contains("invalid args"),
            "expected an argument error, got: {msg}"
        );
    }

    // A different multi-word arg (`parent_rel`) on a stateful command: with no
    // collection open it must REACH the handler and fail with a DOMAIN error,
    // not an argument-deserialization error — proving the arg bound.
    #[test]
    fn create_request_binds_snake_case_parent_rel() {
        let wv = test_webview();
        let err = invoke(
            &wv,
            "create_request",
            json!({ "id": "unopened", "parent_rel": "", "name": "New" }),
        )
        .expect_err("no collection open -> domain error");
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("collection") || msg.contains("not open") || msg.contains("not_found"),
            "expected a domain error (arg bound), got: {msg}"
        );
    }
}
