use synapse_core::ipc::{FilteredDeckConfig, IpcError};
use synapse_core::Collection;
use tauri::State;

type IpcResult<T> = Result<T, IpcError>;

#[tauri::command]
pub fn list_tags(collection: State<'_, Collection>) -> IpcResult<Vec<String>> {
    Ok(collection.list_tags()?)
}

#[tauri::command]
pub fn rename_tag(
    collection: State<'_, Collection>,
    old_tag: String,
    new_tag: String,
) -> IpcResult<u32> {
    Ok(collection.rename_tag(&old_tag, &new_tag)?)
}

#[tauri::command]
pub fn delete_tag(collection: State<'_, Collection>, tag: String) -> IpcResult<u32> {
    Ok(collection.delete_tag(&tag)?)
}

#[tauri::command]
pub fn merge_tags(
    collection: State<'_, Collection>,
    sources: Vec<String>,
    target: String,
) -> IpcResult<()> {
    Ok(collection.merge_tags(sources, &target)?)
}

#[tauri::command]
pub fn create_filtered_deck(
    collection: State<'_, Collection>,
    name: String,
    search: String,
    order: u8,
    limit: u32,
) -> IpcResult<synapse_core::ipc::DeckSummary> {
    Ok(collection.create_filtered_deck(&name, &search, order, limit)?)
}

#[tauri::command]
pub fn rebuild_filtered(collection: State<'_, Collection>, deck_id: i64) -> IpcResult<u32> {
    Ok(collection.rebuild_filtered(deck_id)?)
}

#[tauri::command]
pub fn empty_filtered(collection: State<'_, Collection>, deck_id: i64) -> IpcResult<()> {
    Ok(collection.empty_filtered(deck_id)?)
}

#[tauri::command]
pub fn get_filtered_config(
    collection: State<'_, Collection>,
    deck_id: i64,
) -> IpcResult<Option<FilteredDeckConfig>> {
    Ok(collection.get_filtered_config(deck_id)?)
}
