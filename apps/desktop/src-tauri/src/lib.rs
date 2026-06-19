//! Synapse desktop shell.
//!
//! This crate is deliberately thin: it owns the Tauri window/runtime, opens the
//! collection at startup, registers the IPC command surface, and bridges core
//! [`DomainEvent`]s to the webview. All real work lives in `synapse-core`.

mod commands;

use std::sync::Arc;

use synapse_core::{Collection, DomainEvent, SystemClock};
use synapse_db::SqliteStorage;
use tauri::{Emitter, Manager};

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

            // Bridge domain events → webview so TanStack Query stays live.
            let handle = app.handle().clone();
            collection.events().subscribe(move |event| {
                let _ = handle.emit("synapse://event", event_name(event));
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
