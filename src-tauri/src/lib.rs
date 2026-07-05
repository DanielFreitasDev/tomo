//! Tomo Tauri shell — thin command/event layer over `tomo-core`.
//! Command modules land at milestone M8.

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running Tomo");
}
