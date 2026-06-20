//! App-level commands.

use tauri::{AppHandle, Manager};

use synapse_core::ipc::AppInfo;

/// Returns basic identity of the running app, including the media directory
/// path so the frontend can resolve card images and audio via the asset protocol.
#[tauri::command]
pub fn app_info(app: AppHandle) -> AppInfo {
    let media_dir = app
        .path()
        .app_data_dir()
        .map(|p| p.join("collection.media").to_string_lossy().into_owned())
        .unwrap_or_default();
    AppInfo {
        name: "Synapse".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        tauri_version: tauri::VERSION.to_string(),
        media_dir,
    }
}
