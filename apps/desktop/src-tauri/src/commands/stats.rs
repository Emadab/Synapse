//! Statistics command.

use synapse_core::ipc::{IpcError, StatsDto};
use synapse_core::Collection;
use tauri::State;

#[tauri::command]
pub fn get_stats(collection: State<'_, Collection>) -> Result<StatsDto, IpcError> {
    Ok(collection.stats()?)
}
