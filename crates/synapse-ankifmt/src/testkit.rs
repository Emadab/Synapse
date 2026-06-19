//! Builders for synthetic Anki packages, used by tests and the
//! `gen_fixtures` example. Currently emits the legacy v11 layout (the format
//! the reader supports). This is also an early seed of the M6 export writer.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use rusqlite::Connection;
use synapse_core::error::{CoreError, CoreResult};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Format(e.to_string())
}

/// A 1×1 transparent PNG, used as sample media.
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x62, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

/// Write a small but representative legacy (v11) `.apkg` to `path`:
/// a Basic note type, the `Spanish` / `Spanish::Verbs` deck hierarchy, two
/// notes, two cards, one review, and one media image.
pub fn write_sample_v11_apkg(path: &Path) -> CoreResult<()> {
    let dir = tempfile::tempdir().map_err(err)?;
    let db_path = dir.path().join("collection.anki2");
    build_collection(&db_path)?;
    let sqlite_bytes = std::fs::read(&db_path).map_err(err)?;

    let file = File::create(path).map_err(err)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    zip.start_file("collection.anki2", options).map_err(err)?;
    zip.write_all(&sqlite_bytes).map_err(err)?;
    zip.start_file("media", options).map_err(err)?;
    zip.write_all(br#"{"0":"hola.png"}"#).map_err(err)?;
    zip.start_file("0", options).map_err(err)?;
    zip.write_all(PNG_1X1).map_err(err)?;
    zip.finish().map_err(err)?;
    Ok(())
}

fn build_collection(db_path: &Path) -> CoreResult<()> {
    let conn = Connection::open(db_path).map_err(err)?;
    conn.execute_batch(
        r#"
        CREATE TABLE col (
            id INTEGER PRIMARY KEY, crt INTEGER, mod INTEGER, scm INTEGER, ver INTEGER,
            dty INTEGER, usn INTEGER, ls INTEGER, conf TEXT, models TEXT, decks TEXT,
            dconf TEXT, tags TEXT
        );
        CREATE TABLE notes (
            id INTEGER PRIMARY KEY, guid TEXT, mid INTEGER, mod INTEGER, usn INTEGER,
            tags TEXT, flds TEXT, sfld TEXT, csum INTEGER, flags INTEGER, data TEXT
        );
        CREATE TABLE cards (
            id INTEGER PRIMARY KEY, nid INTEGER, did INTEGER, ord INTEGER, mod INTEGER,
            usn INTEGER, type INTEGER, queue INTEGER, due INTEGER, ivl INTEGER, factor INTEGER,
            reps INTEGER, lapses INTEGER, left INTEGER, odue INTEGER, odid INTEGER, flags INTEGER,
            data TEXT
        );
        CREATE TABLE revlog (
            id INTEGER PRIMARY KEY, cid INTEGER, usn INTEGER, ease INTEGER, ivl INTEGER,
            lastIvl INTEGER, factor INTEGER, time INTEGER, type INTEGER
        );
        "#,
    )
    .map_err(err)?;

    let models = serde_json::json!({
        "1675000000000": {
            "id": 1675000000000i64,
            "name": "Basic",
            "type": 0,
            "css": ".card { font-family: arial; }",
            "flds": [{ "name": "Front", "ord": 0 }, { "name": "Back", "ord": 1 }],
            "tmpls": [{ "name": "Card 1", "ord": 0, "qfmt": "{{Front}}", "afmt": "{{FrontSide}}<hr id=answer>{{Back}}" }]
        }
    })
    .to_string();
    let decks = serde_json::json!({
        "1": { "id": 1, "name": "Default", "conf": 1 },
        "2": { "id": 2, "name": "Spanish", "conf": 1 },
        "3": { "id": 3, "name": "Spanish::Verbs", "conf": 1 }
    })
    .to_string();
    let dconf = serde_json::json!({ "1": { "id": 1, "name": "Default" } }).to_string();

    conn.execute(
        "INSERT INTO col (id, crt, mod, scm, ver, dty, usn, ls, conf, models, decks, dconf, tags)
         VALUES (1, 1600000000, 1600000000, 1600000000, 11, 0, 0, 0, '{}', ?1, ?2, ?3, '{}')",
        rusqlite::params![models, decks, dconf],
    )
    .map_err(err)?;

    // Two notes (one references the media image), two cards, one review.
    let notes = [
        (1001i64, "guid0001", "hola\u{1f}hello", "hola"),
        (
            1002i64,
            "guid0002",
            "<img src=\"hola.png\">uno\u{1f}one",
            "uno",
        ),
    ];
    for (id, guid, flds, sfld) in notes {
        conn.execute(
            "INSERT INTO notes (id, guid, mid, mod, usn, tags, flds, sfld, csum, flags, data)
             VALUES (?1, ?2, 1675000000000, 1600000000, -1, 'spanish', ?3, ?4, 0, 0, '')",
            rusqlite::params![id, guid, flds, sfld],
        )
        .map_err(err)?;
    }
    let cards = [(2001i64, 1001i64, 1u8), (2002i64, 1002i64, 2u8)];
    for (id, nid, due) in cards {
        conn.execute(
            "INSERT INTO cards (id, nid, did, ord, mod, usn, type, queue, due, ivl, factor,
             reps, lapses, left, odue, odid, flags, data)
             VALUES (?1, ?2, 3, 0, 1600000000, -1, 0, 0, ?3, 0, 0, 0, 0, 0, 0, 0, 0, '')",
            rusqlite::params![id, nid, due],
        )
        .map_err(err)?;
    }
    conn.execute(
        "INSERT INTO revlog (id, cid, usn, ease, ivl, lastIvl, factor, time, type)
         VALUES (1600000001000, 2001, -1, 3, 1, 0, 2500, 1200, 0)",
        [],
    )
    .map_err(err)?;

    drop(conn);
    Ok(())
}
