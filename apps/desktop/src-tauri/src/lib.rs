//! Synapse desktop shell.
//!
//! This crate is deliberately thin: it owns the Tauri window/runtime, opens the
//! collection at startup, registers the IPC command surface, and bridges core
//! [`DomainEvent`]s to the webview. All real work lives in `synapse-core`.

mod commands;

use std::sync::{Arc, Mutex};
use synapse_db::backup as db_backup;
use synapse_plugin::PluginManager;
use tracing_subscriber::{fmt, EnvFilter};

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
    // Structured log subscriber: RUST_LOG env var controls filter; defaults to info.
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(true)
        .init();

    // Capture panics → log + write a crash report file next to the DB.
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        tracing::error!("PANIC: {msg}");
        // Best-effort: write to <tmp>/synapse-crash.txt so it survives process death.
        let path = std::env::temp_dir().join("synapse-crash.txt");
        let _ = std::fs::write(&path, format!("{msg}\n"));
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("synapse-media", |app, request| {
            // Serve files from <app-data>/collection.media/<filename>.
            // Only the last path component is used (no traversal).
            let uri = request.uri();
            let encoded = uri.path().trim_start_matches('/');
            let decoded = percent_decode_str(encoded).decode_utf8_lossy().into_owned();
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
            let db_path = dir.join("collection.sqlite");
            let storage = SqliteStorage::open(&db_path)?;
            let collection = Collection::new(Box::new(storage), Arc::new(SystemClock));

            // Auto-backup on startup: create a zip if no backup in the last 24 h.
            let backup_dir = dir.join("backups");
            let _ = std::fs::create_dir_all(&backup_dir);
            let should_backup = db_backup::list_zips(&backup_dir)
                .ok()
                .and_then(|v| v.into_iter().next().map(|(_, ms, _)| ms))
                .map(|last_ms| {
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64;
                    now_ms - last_ms > 86_400_000
                })
                .unwrap_or(true);
            if should_backup && db_path.is_file() {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                let tmp = backup_dir.join(format!("{now_ms}.tmp.sqlite"));
                let zip = backup_dir.join(format!("{now_ms}.zip"));
                if collection.backup_db(&tmp).is_ok() {
                    let media_dir = dir.join("collection.media");
                    if db_backup::create_zip(&tmp, &media_dir, &zip).is_ok() {
                        let _ = db_backup::rotate_backups(&backup_dir, 20);
                        tracing::info!("auto-backup created: {now_ms}.zip");
                    }
                    let _ = std::fs::remove_file(&tmp);
                }
            }

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

            // Plugin manager — auto-install the bundled sample on first run.
            let plugin_manager = PluginManager::new(&dir);
            if let Err(e) = plugin_manager.ensure_sample() {
                tracing::warn!("could not install sample plugin: {e}");
            }
            app.manage(Mutex::new(plugin_manager));

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
            commands::deck::get_deck_config,
            commands::deck::set_deck_config,
            commands::deck::get_today_extra_new,
            commands::deck::increase_today_limit,
            commands::import::import_package,
            commands::study::get_next_card,
            commands::study::answer_card,
            commands::browse::list_notes,
            commands::browse::get_note,
            commands::browse::save_note,
            commands::browse::search_notes,
            commands::browse::search_cards,
            commands::browse::delete_notes,
            commands::browse::move_cards_to_deck,
            commands::browse::bulk_add_tag,
            commands::browse::bulk_remove_tag,
            commands::browse::list_notetypes,
            commands::browse::add_note,
            commands::export::export_package,
            commands::stats::get_stats,
            commands::notetype::get_notetype,
            commands::notetype::create_notetype,
            commands::notetype::delete_notetype,
            commands::notetype::rename_notetype,
            commands::notetype::add_field,
            commands::notetype::check_field_remove,
            commands::notetype::remove_field,
            commands::notetype::rename_field,
            commands::notetype::reorder_fields,
            commands::notetype::add_template,
            commands::notetype::remove_template,
            commands::notetype::save_template,
            commands::notetype::preview_template,
            commands::cards::suspend_cards,
            commands::cards::unsuspend_cards,
            commands::cards::bury_cards,
            commands::cards::set_card_flag,
            commands::tags::list_tags,
            commands::tags::rename_tag,
            commands::tags::delete_tag,
            commands::tags::merge_tags,
            commands::tags::create_filtered_deck,
            commands::tags::rebuild_filtered,
            commands::tags::empty_filtered,
            commands::tags::get_filtered_config,
            commands::maintenance::create_backup,
            commands::maintenance::list_backups,
            commands::maintenance::restore_backup,
            commands::maintenance::delete_backup,
            commands::maintenance::check_integrity,
            commands::maintenance::optimize_db,
            commands::maintenance::check_media,
            commands::maintenance::delete_orphan_media,
            commands::optimize::optimize_fsrs,
            commands::optimize::apply_fsrs_weights,
            commands::plugins::list_plugins,
            commands::plugins::enable_plugin,
            commands::plugins::disable_plugin,
            commands::plugins::install_plugin,
            commands::plugins::get_plugin_entry,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
