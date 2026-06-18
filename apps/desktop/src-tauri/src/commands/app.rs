//! App-level commands.

use synapse_core::ipc::AppInfo;

/// Returns basic identity of the running app. M0 end-to-end proof that the
/// IPC + ts-rs type pipeline works: the `AppInfo` type is defined once in Rust
/// and consumed as generated TypeScript on the frontend.
#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Synapse".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        tauri_version: tauri::VERSION.to_string(),
    }
}
