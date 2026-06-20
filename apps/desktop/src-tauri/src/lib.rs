//! Synapse desktop shell.
//!
//! This crate is deliberately thin: it owns the Tauri window/runtime, opens the
//! collection at startup, registers the IPC command surface, and bridges core
//! [`DomainEvent`]s to the webview. All real work lives in `synapse-core`.

mod commands;

use std::sync::{Arc, Mutex};

use synapse_core::{Collection, DomainEvent, SystemClock};
use synapse_db::SqliteStorage;
use synapse_search::NoteIndex;
use tauri::{Emitter, Manager};

/// Tauri managed state wrapping the search index.
pub struct SearchState(pub Mutex<NoteIndex>);

/// Stable event name emitted to the webview, which maps it to query-cache
/// invalidations. Keep in sync with the frontend's event handler.
fn event_name(event: &DomainEvent) -> &'static str {
    match event {
        DomainEvent::DeckChanged { .. } => "deck-changed",
        DomainEvent::SchemaChanged => "schema-changed",
        DomainEvent::CollectionOpened => "collection-opened",
        DomainEvent::CollectionClosed => "collection-closed",
        DomainEvent::CardAnswered { .. } => "card-answered",
        DomainEvent::NoteAdded { .. }
        | DomainEvent::NoteUpdated { .. }
        | DomainEvent::NoteRemoved { .. } => "notes-changed",
        DomainEvent::MediaChanged => "media-changed",
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Open (or create) the collection under the OS app-data directory.
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let storage = SqliteStorage::open(dir.join("collection.sqlite"))?;
            let collection = Collection::new(Box::new(storage), Arc::new(SystemClock));

            // Build the initial search index.
            let index = NoteIndex::new().expect("failed to create search index");
            if let Ok(rows) = collection.index_rows() {
                let _ = index.rebuild(&rows);
            }
            app.manage(SearchState(Mutex::new(index)));

            // Bridge domain events → webview + rebuild search index on mutations.
            let handle = app.handle().clone();
            collection.events().subscribe(move |event| {
                let _ = handle.emit("synapse://event", event_name(event));

                let needs_reindex = matches!(
                    event,
                    DomainEvent::CollectionOpened
                        | DomainEvent::SchemaChanged
                        | DomainEvent::NoteAdded { .. }
                        | DomainEvent::NoteUpdated { .. }
                        | DomainEvent::NoteRemoved { .. }
                );
                if needs_reindex {
                    if let Some(state) = handle.try_state::<SearchState>() {
                        if let Some(collection) = handle.try_state::<Collection>() {
                            if let Ok(rows) = collection.index_rows() {
                                if let Ok(idx) = state.0.lock() {
                                    let _ = idx.rebuild(&rows);
                                }
                            }
                        }
                    }
                }
            });

            app.manage(collection);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::deck::list_decks,
            commands::deck::create_deck,
            commands::deck::rename_deck,
            commands::deck::delete_deck,
            commands::deck::undo,
            commands::deck::undo_status,
            commands::import::import_package,
            commands::study::get_next_card,
            commands::study::answer_card,
            commands::browse::list_notes,
            commands::browse::get_note,
            commands::browse::save_note,
            commands::browse::search_notes,
            commands::export::export_package,
            commands::stats::get_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
