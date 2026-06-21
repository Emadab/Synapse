//! Card/note browser commands: list, fetch, save notes; Anki-query card search; bulk ops.

use synapse_core::ipc::{
    AddNoteResult, CardRow, IpcError, IpcErrorKind, NoteDetail, NoteOverview, NotetypeSummary,
};
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

#[tauri::command]
pub fn list_notetypes(collection: State<'_, Collection>) -> IpcResult<Vec<NotetypeSummary>> {
    Ok(collection.list_notetypes()?)
}

#[tauri::command]
pub fn add_note(
    collection: State<'_, Collection>,
    notetype_id: i64,
    deck_id: i64,
    fields: Vec<String>,
    tags: Vec<String>,
) -> IpcResult<AddNoteResult> {
    Ok(collection.add_note(notetype_id, deck_id, &fields, &tags)?)
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
        .map_err(|_| IpcError {
            kind: IpcErrorKind::Internal,
            message: "search lock poisoned".into(),
        })?
        .search(q, 500)
        .map_err(|e| IpcError {
            kind: IpcErrorKind::Internal,
            message: e.to_string(),
        })?;
    Ok(collection.notes_by_ids(&ids)?)
}

/// Anki-flavoured card search: supports is:/flag:/deck:/tag:/prop:/-/or.
/// Returns up to 2000 card rows (cards, not notes) for the browser table.
#[tauri::command]
pub fn search_cards(collection: State<'_, Collection>, query: String) -> IpcResult<Vec<CardRow>> {
    Ok(collection.search_cards(query.trim(), 2000)?)
}

/// Delete notes (and their cards) by note id list.
#[tauri::command]
pub fn delete_notes(collection: State<'_, Collection>, note_ids: Vec<i64>) -> IpcResult<()> {
    collection.delete_notes(&note_ids)?;
    Ok(())
}

/// Reassign cards to a different deck.
#[tauri::command]
pub fn move_cards_to_deck(
    collection: State<'_, Collection>,
    card_ids: Vec<i64>,
    deck_id: i64,
) -> IpcResult<()> {
    collection.move_cards_to_deck(&card_ids, deck_id)?;
    Ok(())
}

/// Add a tag to multiple notes (bulk).
#[tauri::command]
pub fn bulk_add_tag(
    collection: State<'_, Collection>,
    note_ids: Vec<i64>,
    tag: String,
) -> IpcResult<()> {
    let tag = tag.trim().to_string();
    if tag.is_empty() || tag.contains(' ') {
        return Err(IpcError {
            kind: IpcErrorKind::Invalid,
            message: "tag must be non-empty and contain no spaces".into(),
        });
    }
    collection.bulk_add_tag(&note_ids, &tag)?;
    Ok(())
}

/// Remove a tag from multiple notes (bulk).
#[tauri::command]
pub fn bulk_remove_tag(
    collection: State<'_, Collection>,
    note_ids: Vec<i64>,
    tag: String,
) -> IpcResult<()> {
    collection.bulk_remove_tag(&note_ids, &tag)?;
    Ok(())
}
