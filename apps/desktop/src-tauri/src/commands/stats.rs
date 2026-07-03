//! Statistics command.

use synapse_core::ipc::{IpcError, StatsDto};
use synapse_core::Collection;
use tauri::State;

#[tauri::command]
pub fn get_stats(
    collection: State<'_, Collection>,
    deck_id: Option<i64>,
    days: Option<u32>,
    tz_offset_minutes: i32,
) -> Result<StatsDto, IpcError> {
    Ok(collection.stats(deck_id, days, tz_offset_minutes)?)
}
