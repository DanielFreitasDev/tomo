//! Tomo Tauri shell — thin command/event layer over `tomo-core`.

mod commands;
mod dto;
mod error;
mod state;
mod watchbridge;

use tauri::Manager;

use state::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("tomo"));
            let cache_dir = app
                .path()
                .app_cache_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("tomo-cache"));
            app.manage(AppState::new(config_dir, cache_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pick_collection_folder,
            commands::list_recent_collections,
            commands::open_collection,
            commands::create_collection,
            commands::close_collection,
            commands::reload_collection,
            commands::create_folder,
            commands::create_request,
            commands::read_request,
            commands::save_request,
            commands::rename_node,
            commands::move_node,
            commands::duplicate_request,
            commands::delete_node,
            commands::reorder_nodes,
            commands::read_environment,
            commands::save_environment,
            commands::delete_environment,
            commands::select_environment,
            commands::read_secrets,
            commands::save_secrets,
            commands::send_request,
            commands::cancel_request,
            commands::get_response_body,
            commands::save_response_body,
            commands::get_cookies,
            commands::clear_cookies,
            commands::get_runtime_vars,
            commands::set_runtime_var,
            commands::clear_runtime_vars,
            commands::import_curl,
            commands::export_curl,
            commands::get_settings,
            commands::save_settings,
            commands::get_ui_state,
            commands::save_ui_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tomo");
}
