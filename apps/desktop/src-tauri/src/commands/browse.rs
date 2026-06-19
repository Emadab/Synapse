//! Card/note browser commands: list, fetch, and save notes.

use synapse_core::ipc::{IpcError, NoteDetail, NoteOverview};
use synapse_core::Collection;
use tauri::State;

type IpcResult<T> = Result<T, IpcError>;

#[tauri::command]
pub fn list_notes(
    collection: State<'_, Collection>,
    query: Option<String>,
) -> IpcResult<Vec<NoteOverview>> {
    Ok(collection.list_notes(query.as_deref())?)
}

#[tauri::command]
pub fn get_note(collection: State<'_, Collection>, note_id: i64) -> IpcResult<Option<NoteDetail>> {
    Ok(collection.note_detail(note_id)?)
}

#[tauri::command]
pub fn save_note(
    collection: State<'_, Collection>,
    note_id: i64,
    fields: Vec<String>,
    tags: Vec<String>,
) -> IpcResult<()> {
    collection.update_note(note_id, &fields, &tags)?;
    Ok(())
}
