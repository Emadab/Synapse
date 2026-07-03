//! Import commands. Parse a package with `synapse-ankifmt`, then merge it into
//! the open collection. Media is extracted into `<app-data>/collection.media`.

use std::path::Path;

use synapse_core::ipc::{ImportProgress, IpcError, IpcErrorKind};
use synapse_core::{Collection, ImportSummary};
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub async fn import_package(
    app: AppHandle,
    collection: State<'_, Collection>,
    path: String,
) -> Result<ImportSummary, IpcError> {
    let media_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| IpcError {
            kind: IpcErrorKind::Internal,
            message: e.to_string(),
        })?
        .join("collection.media");

    let (model, media_imported) =
        synapse_ankifmt::read_package(Path::new(&path), Some(&media_dir))?;
    let mut on_progress = |done: u32, total: u32| {
        let _ = app.emit("synapse://import-progress", ImportProgress { done, total });
    };
    let mut summary = collection.import_with_progress(&model, &mut on_progress)?;
    summary.media_imported = media_imported;
    Ok(summary)
}
