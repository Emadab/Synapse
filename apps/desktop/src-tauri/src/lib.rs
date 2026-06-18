//! Synapse desktop shell.
//!
//! This crate is deliberately thin: it owns the Tauri window/runtime and the
//! IPC command surface, and it delegates all real work to `synapse-core`. No
//! domain logic lives here.

mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![commands::app::app_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
