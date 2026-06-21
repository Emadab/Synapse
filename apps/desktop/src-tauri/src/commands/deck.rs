//! Deck commands. Delegate to the managed [`Collection`]; `?` converts
//! `CoreError` into the serialisable [`IpcError`] union returned to the UI.

use synapse_core::ipc::{DeckConfig, DeckSummary, IpcError};
use synapse_core::Collection;
use tauri::State;

type IpcResult<T> = Result<T, IpcError>;

#[tauri::command]
pub fn list_decks(collection: State<'_, Collection>) -> IpcResult<Vec<DeckSummary>> {
    Ok(collection
        .list_decks_with_counts()?
        .into_iter()
        .map(|(deck, counts)| DeckSummary::with_counts(deck, counts))
        .collect())
}

#[tauri::command]
pub fn create_deck(collection: State<'_, Collection>, name: String) -> IpcResult<DeckSummary> {
    Ok(collection.create_deck(&name)?.into())
}

#[tauri::command]
pub fn rename_deck(collection: State<'_, Collection>, id: i64, name: String) -> IpcResult<()> {
    collection.rename_deck(id, &name)?;
    Ok(())
}

#[tauri::command]
pub fn delete_deck(collection: State<'_, Collection>, id: i64) -> IpcResult<()> {
    collection.remove_deck(id)?;
    Ok(())
}

#[tauri::command]
pub fn undo(collection: State<'_, Collection>) -> IpcResult<Option<String>> {
    Ok(collection.undo()?)
}

#[tauri::command]
pub fn undo_status(collection: State<'_, Collection>) -> IpcResult<Option<String>> {
    Ok(collection.undo_status())
}

#[tauri::command]
pub fn get_deck_config(collection: State<'_, Collection>, deck_id: i64) -> IpcResult<DeckConfig> {
    Ok(collection.get_deck_config(deck_id)?)
}

#[tauri::command]
pub fn set_deck_config(collection: State<'_, Collection>, config: DeckConfig) -> IpcResult<()> {
    collection.set_deck_config(&config)?;
    Ok(())
}
