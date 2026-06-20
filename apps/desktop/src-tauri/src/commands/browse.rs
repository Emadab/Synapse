//! Card/note browser commands: list, fetch, and save notes.

use synapse_core::ipc::{IpcError, IpcErrorKind, NoteDetail, NoteOverview};
use synapse_core::Collection;
use tauri::State;

use crate::SearchState;

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

/// Full-text + faceted Tantivy search. Falls back to SQL LIKE when query is
/// empty or index is unavailable.
#[tauri::command]
pub fn search_notes(
    collection: State<'_, Collection>,
    search: State<'_, SearchState>,
    query: String,
) -> IpcResult<Vec<NoteOverview>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(collection.list_notes(None)?);
    }
    let ids = search
        .0
        .lock()
        .map_err(|_| IpcError { kind: IpcErrorKind::Internal, message: "search lock poisoned".into() })?
        .search(q, 500)
        .map_err(|e| IpcError { kind: IpcErrorKind::Internal, message: e.to_string() })?;
    Ok(collection.notes_by_ids(&ids)?)
}
