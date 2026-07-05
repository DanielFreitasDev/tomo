//! All #[tauri::command] handlers. Thin: each resolves state, calls tomo-core,
//! and shapes DTOs. Self-writes register with the suppressor so the watcher
//! doesn't echo them back.

use std::path::PathBuf;
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
use tomo_core::model::{EnvironmentFile, RequestFile, SecretsFile, Settings};
use tomo_core::vars::process_env_snapshot;
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

// ---------------------------------------------------------------------------
// collections
// ---------------------------------------------------------------------------

#[tauri::command]
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

#[derive(serde::Serialize)]
pub struct RecentEntry {
    pub path: String,
    pub name: String,
}

#[tauri::command]
pub fn list_recent_collections(state: State<'_, AppState>) -> ApiResult<Vec<RecentEntry>> {
    let path = state.config_dir.join("recents.json");
    Ok(std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default())
}

#[tauri::command]
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
    tree_dto(&id, &runtime)
}

#[tauri::command]
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

#[tauri::command]
pub fn close_collection(state: State<'_, AppState>, id: String) -> ApiResult<()> {
    state.collections.remove(&id);
    Ok(())
}

#[tauri::command]
pub fn reload_collection(state: State<'_, AppState>, id: String) -> ApiResult<CollectionTreeDto> {
    let runtime = state.collection(&id)?;
    tree_dto(&id, &runtime)
}

// ---------------------------------------------------------------------------
// nodes
// ---------------------------------------------------------------------------

fn register_write(runtime: &CollectionRuntime, rel: &str, text: &str) {
    if let Ok(path) = resolve_rel(&runtime.root, rel) {
        runtime.suppressor.register(&path, text);
    }
}

#[tauri::command]
pub fn create_folder(
    state: State<'_, AppState>,
    id: String,
    parent_rel: String,
    name: String,
) -> ApiResult<String> {
    let runtime = state.collection(&id)?;
    Ok(fsops::create_folder(&runtime.root, &parent_rel, &name)?)
}

#[tauri::command]
pub fn create_request(
    state: State<'_, AppState>,
    id: String,
    parent_rel: String,
    name: String,
) -> ApiResult<String> {
    let runtime = state.collection(&id)?;
    Ok(fsops::create_request(&runtime.root, &parent_rel, &name)?)
}

#[tauri::command]
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

#[tauri::command]
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

    let current_text = read_text(&path).unwrap_or_default();
    let current_hash = content_hash(current_text.as_bytes());

    if let Some(base) = &base_hash
        && !current_text.is_empty()
        && base != &current_hash
    {
        return Ok(SaveResultDto::Conflict {
            current_text,
            current_hash,
        });
    }

    // surgical edit if the file exists, canonical write otherwise
    let out = if current_text.is_empty() {
        request_to_string(&req)?
    } else {
        sync_request(&current_text, &req, &path)?
    };
    register_write(&runtime, &rel, &out);
    atomic_write(&path, &out)?;
    Ok(SaveResultDto::Saved {
        hash: content_hash(out.as_bytes()),
    })
}

#[tauri::command]
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

#[tauri::command]
pub fn move_node(
    state: State<'_, AppState>,
    id: String,
    rel: String,
    new_parent_rel: String,
) -> ApiResult<String> {
    let runtime = state.collection(&id)?;
    Ok(fsops::move_node(&runtime.root, &rel, &new_parent_rel)?)
}

#[tauri::command]
pub fn duplicate_request(state: State<'_, AppState>, id: String, rel: String) -> ApiResult<String> {
    let runtime = state.collection(&id)?;
    Ok(fsops::duplicate_request(&runtime.root, &rel)?)
}

#[tauri::command]
pub fn delete_node(state: State<'_, AppState>, id: String, rel: String) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    Ok(fsops::delete_node(&runtime.root, &rel)?)
}

#[tauri::command]
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

#[tauri::command]
pub fn read_environment(state: State<'_, AppState>, id: String, name: String) -> ApiResult<Value> {
    let runtime = state.collection(&id)?;
    let path = runtime
        .root
        .join(ENVIRONMENTS_DIR)
        .join(format!("{name}.toml"));
    let env = parse_environment(&read_text(&path)?, &path)?;
    Ok(to_value(&env))
}

#[tauri::command]
pub fn save_environment(
    state: State<'_, AppState>,
    id: String,
    name: String,
    env: Value,
    previous_name: Option<String>,
) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    let env: EnvironmentFile = from_value(env)?;
    let dir = runtime.root.join(ENVIRONMENTS_DIR);
    let path = dir.join(format!("{name}.toml"));

    let text = match read_text(&path) {
        Ok(existing) => sync_environment(&existing, &env, &path)?,
        Err(_) => environment_to_string(&env)?,
    };
    atomic_write(&path, &text)?;
    if let Some(prev) = previous_name
        && prev != name
    {
        let _ = std::fs::remove_file(dir.join(format!("{prev}.toml")));
    }
    Ok(())
}

#[tauri::command]
pub fn delete_environment(state: State<'_, AppState>, id: String, name: String) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    let path = runtime
        .root
        .join(ENVIRONMENTS_DIR)
        .join(format!("{name}.toml"));
    std::fs::remove_file(&path).map_err(|e| ApiError::from(tomo_core::CoreError::io(&path, e)))
}

#[tauri::command]
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

#[tauri::command]
pub fn read_secrets(state: State<'_, AppState>, id: String) -> ApiResult<Value> {
    let runtime = state.collection(&id)?;
    let path = runtime.root.join(SECRETS_FILE);
    let secrets = match read_text(&path) {
        Ok(text) => parse_secrets(&text, &path)?,
        Err(_) => SecretsFile::default(),
    };
    Ok(to_value(&secrets))
}

#[tauri::command]
pub fn save_secrets(state: State<'_, AppState>, id: String, secrets: Value) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    let secrets: SecretsFile = from_value(secrets)?;
    // gitignore MUST be updated before secrets ever hit disk
    fsops::upsert_gitignore(&runtime.root)?;
    let path = runtime.root.join(SECRETS_FILE);
    atomic_write(&path, &secrets_to_string(&secrets))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// http
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, Clone)]
struct StartedPayload {
    run_id: String,
}

#[tauri::command]
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
        Some(name) => {
            let path = runtime
                .root
                .join(ENVIRONMENTS_DIR)
                .join(format!("{name}.toml"));
            read_text(&path)
                .ok()
                .and_then(|t| parse_environment(&t, &path).ok())
        }
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
                cache.put(run_id.clone(), Arc::new(data));
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

#[tauri::command]
pub fn cancel_request(state: State<'_, AppState>, run_id: String) -> ApiResult<bool> {
    if let Some((_, handle)) = state.runs.remove(&run_id) {
        handle.token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Raw response bytes — never travel through JSON.
#[tauri::command]
pub fn get_response_body(state: State<'_, AppState>, run_id: String) -> tauri::ipc::Response {
    let bytes = state
        .bodies
        .lock()
        .ok()
        .and_then(|cache| cache.peek(&run_id).map(|d| d.body.bytes.clone()))
        .unwrap_or_default();
    tauri::ipc::Response::new(bytes)
}

#[tauri::command]
pub fn save_response_body(
    state: State<'_, AppState>,
    run_id: String,
    dest: String,
) -> ApiResult<()> {
    let bytes = state
        .bodies
        .lock()
        .ok()
        .and_then(|cache| cache.peek(&run_id).map(|d| d.body.bytes.clone()))
        .ok_or_else(|| ApiError::new("not_found", "response body no longer cached"))?;
    let dest = PathBuf::from(dest);
    std::fs::write(&dest, &bytes).map_err(|e| ApiError::from(tomo_core::CoreError::io(&dest, e)))
}

#[tauri::command]
pub fn get_cookies(state: State<'_, AppState>, id: String) -> ApiResult<Value> {
    let runtime = state.collection(&id)?;
    Ok(to_value(&runtime.jar.list()))
}

#[tauri::command]
pub fn clear_cookies(
    state: State<'_, AppState>,
    id: String,
    domain: Option<String>,
) -> ApiResult<()> {
    let runtime = state.collection(&id)?;
    runtime.jar.clear(domain.as_deref());
    Ok(())
}

#[tauri::command]
pub fn get_runtime_vars(state: State<'_, AppState>, id: String) -> ApiResult<Value> {
    let runtime = state.collection(&id)?;
    Ok(runtime
        .runtime_vars
        .lock()
        .map(|g| to_value(&*g))
        .unwrap_or(Value::Null))
}

#[tauri::command]
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

#[tauri::command]
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

#[tauri::command]
pub fn import_curl(text: String) -> ApiResult<Value> {
    Ok(to_value(&tomo_core::curl::from_curl(&text)?))
}

#[tauri::command]
pub fn export_curl(
    state: State<'_, AppState>,
    id: String,
    rel: String,
    draft: Option<Value>,
    _interpolated: bool,
) -> ApiResult<String> {
    let request: RequestFile = match draft {
        Some(d) => from_value(d)?,
        None => {
            let runtime = state.collection(&id)?;
            read_request_hashed(&runtime.root, &rel)?.0
        }
    };
    Ok(tomo_core::curl::to_curl(&request))
}

// ---------------------------------------------------------------------------
// settings & ui state
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> ApiResult<Value> {
    Ok(to_value(&load_settings(&state.config_dir)))
}

#[tauri::command]
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

#[tauri::command]
pub fn get_ui_state(state: State<'_, AppState>, id: Option<String>) -> ApiResult<Value> {
    let path = ui_state_path(&state, &id);
    Ok(std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null))
}

#[tauri::command]
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
