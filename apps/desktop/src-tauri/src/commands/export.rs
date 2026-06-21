//! Export command: dump the collection and write it as an .apkg file.

use std::path::Path;

use synapse_core::ipc::{IpcError, IpcErrorKind};
use synapse_core::Collection;
use tauri::{Manager, State};

type IpcResult<T> = Result<T, IpcError>;

fn ipc_err(msg: impl std::fmt::Display) -> IpcError {
    IpcError {
        kind: IpcErrorKind::Internal,
        message: msg.to_string(),
    }
}

/// Export the collection to the given path as an .apkg file.
/// Returns the number of media files included.
#[tauri::command]
pub fn export_package(
    app: tauri::AppHandle,
    collection: State<'_, Collection>,
    path: String,
) -> IpcResult<u32> {
    let model = collection.dump_collection().map_err(ipc_err)?;

    let media_dir = app
        .path()
        .app_data_dir()
        .map_err(ipc_err)?
        .join("collection.media");

    let media_dir = if media_dir.is_dir() {
        Some(media_dir)
    } else {
        None
    };

    let count = synapse_ankifmt::write_apkg(&model, Path::new(&path), media_dir.as_deref())
        .map_err(ipc_err)?;

    Ok(count)
}
