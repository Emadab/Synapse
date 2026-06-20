//! Synapse desktop shell.
//!
//! This crate is deliberately thin: it owns the Tauri window/runtime, opens the
//! collection at startup, registers the IPC command surface, and bridges core
//! [`DomainEvent`]s to the webview. All real work lives in `synapse-core`.

mod commands;

use std::sync::{Arc, Mutex};

use percent_encoding::percent_decode_str;
use synapse_core::{Collection, DomainEvent, SystemClock};
use synapse_db::SqliteStorage;
use synapse_search::NoteIndex;
use tauri::{Emitter, Manager};

fn mime_for(filename: &str) -> &'static str {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

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
        .register_uri_scheme_protocol("synapse-media", |app, request| {
            // Serve files from <app-data>/collection.media/<filename>.
            // Only the last path component is used (no traversal).
            let uri = request.uri();
            let encoded = uri.path().trim_start_matches('/');
            let decoded = percent_decode_str(encoded)
                .decode_utf8_lossy()
                .into_owned();
            let filename = std::path::Path::new(&decoded)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let media_dir = app
                .app_handle()
                .path()
                .app_data_dir()
                .map(|p| p.join("collection.media"))
                .unwrap_or_default();
            let file_path = media_dir.join(&filename);

            match std::fs::read(&file_path) {
                Ok(body) => {
                    let mime = mime_for(&filename);
                    tauri::http::Response::builder()
                        .header("Content-Type", mime)
                        .header("Access-Control-Allow-Origin", "*")
                        .body(body)
                        .unwrap()
                }
                Err(_) => tauri::http::Response::builder()
                    .status(404)
                    .body(Vec::new())
                    .unwrap(),
            }
        })
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
