use std::sync::Mutex;

use synapse_core::ipc::{IpcError, IpcErrorKind, PluginInfo};
use synapse_plugin::{PluginError, PluginManager, PluginRecord};
use tauri::State;

type IpcResult<T> = Result<T, IpcError>;

fn plugin_err(e: PluginError) -> IpcError {
    let (kind, message) = match e {
        PluginError::NotFound(msg) => (IpcErrorKind::NotFound, msg),
        PluginError::InvalidManifest(msg) => (IpcErrorKind::Invalid, msg),
        PluginError::Json(e) => (IpcErrorKind::Format, e.to_string()),
        PluginError::Io(e) => (IpcErrorKind::Internal, e.to_string()),
    };
    IpcError { kind, message }
}

fn to_info(r: &PluginRecord) -> PluginInfo {
    PluginInfo {
        id: r.manifest.id.clone(),
        name: r.manifest.name.clone(),
        version: r.manifest.version.clone(),
        description: r.manifest.description.clone(),
        author: r.manifest.author.clone(),
        permissions: r.manifest.permission_strings(),
        enabled: r.enabled,
    }
}

#[tauri::command]
pub fn list_plugins(mgr: State<'_, Mutex<PluginManager>>) -> IpcResult<Vec<PluginInfo>> {
    let mgr = mgr.lock().unwrap();
    mgr.list()
        .map(|records| records.iter().map(to_info).collect())
        .map_err(plugin_err)
}

#[tauri::command]
pub fn enable_plugin(id: String, mgr: State<'_, Mutex<PluginManager>>) -> IpcResult<()> {
    mgr.lock().unwrap().enable(&id).map_err(plugin_err)
}

#[tauri::command]
pub fn disable_plugin(id: String, mgr: State<'_, Mutex<PluginManager>>) -> IpcResult<()> {
    mgr.lock().unwrap().disable(&id).map_err(plugin_err)
}

/// Install a plugin from a directory chosen by the user.
#[tauri::command]
pub fn install_plugin(path: String, mgr: State<'_, Mutex<PluginManager>>) -> IpcResult<PluginInfo> {
    let src = std::path::Path::new(&path);
    let record = mgr.lock().unwrap().install(src).map_err(plugin_err)?;
    Ok(to_info(&record))
}

/// Return the entry-point JS source for a plugin so the frontend can create a Worker.
#[tauri::command]
pub fn get_plugin_entry(id: String, mgr: State<'_, Mutex<PluginManager>>) -> IpcResult<String> {
    mgr.lock().unwrap().read_entry(&id).map_err(plugin_err)
}
