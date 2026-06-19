//! Read an Anki `.apkg` / `.colpkg` package into a [`CanonicalModel`].
//!
//! Pipeline: unzip → locate the collection DB (`collection.anki2` /
//! `.anki21` / zstd-compressed `.anki21b`) → detect schema → map to canonical.
//!
//! Legacy **schema v11** (decks/models/dconf stored as JSON blobs in the `col`
//! table) is fully supported. Modern **v18** (tabular, protobuf-encoded configs)
//! is detected and rejected with a clear error until its protobuf decoder lands.

use std::io::Read;
use std::path::Path;

use rusqlite::Connection;
use serde_json::Value;
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::model::{
    CanonicalModel, Card, Deck, DeckConfig, Field, Note, Notetype, Revlog, Template,
};

type Archive = zip::ZipArchive<std::fs::File>;

fn fmt(e: impl std::fmt::Display) -> CoreError {
    CoreError::Format(e.to_string())
}

/// (entry name, zstd-compressed?) in priority order.
const COLLECTION_CANDIDATES: &[(&str, bool)] = &[
    ("collection.anki21b", true),
    ("collection.anki21", false),
    ("collection.anki2", false),
];

/// Read a package. If `media_dir` is `Some`, media files are extracted there
/// (deduped by the on-disk filename) and the count is returned.
pub fn read_package(path: &Path, media_dir: Option<&Path>) -> CoreResult<(CanonicalModel, u32)> {
    let file = std::fs::File::open(path).map_err(fmt)?;
    let mut archive = zip::ZipArchive::new(file).map_err(fmt)?;

    let names: Vec<String> = archive.file_names().map(str::to_string).collect();
    let (coll_name, zstd_compressed) = COLLECTION_CANDIDATES
        .iter()
        .copied()
        .find(|(name, _)| names.iter().any(|n| n == name))
        .ok_or_else(|| CoreError::Format("no collection database found in package".into()))?;

    let mut bytes = Vec::new();
    {
        let mut entry = archive.by_name(coll_name).map_err(fmt)?;
        entry.read_to_end(&mut bytes).map_err(fmt)?;
    }
    if zstd_compressed {
        bytes = zstd::stream::decode_all(std::io::Cursor::new(bytes)).map_err(fmt)?;
    }

    // SQLite needs a path; use a temp dir (no lingering file handle, unlike a
    // NamedTempFile, which can clash with SQLite's own open on Windows).
    let dir = tempfile::tempdir().map_err(fmt)?;
    let db_path = dir.path().join("collection.anki2");
    std::fs::write(&db_path, &bytes).map_err(fmt)?;
    let conn = Connection::open(&db_path).map_err(fmt)?;

    let model = read_collection(&conn)?;
    drop(conn);

    let media_count = match media_dir {
        Some(target) => extract_media(&mut archive, target)?,
        None => 0,
    };

    Ok((model, media_count))
}

fn table_exists(conn: &Connection, name: &str) -> CoreResult<bool> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [name],
            |r| r.get(0),
        )
        .map_err(fmt)?;
    Ok(count > 0)
}

fn read_collection(conn: &Connection) -> CoreResult<CanonicalModel> {
    if table_exists(conn, "notetypes")? {
        return Err(CoreError::Format(
            "this looks like a modern Anki collection (schema v18). Import of the v18/.colpkg \
             protobuf format isn't supported yet — re-export with \"Support older Anki versions\" \
             to produce a v11 .apkg."
                .into(),
        ));
    }

    let mut model = CanonicalModel::default();
    read_v11_meta(conn, &mut model)?;
    read_notes(conn, &mut model)?;
    read_cards(conn, &mut model)?;
    read_revlog(conn, &mut model)?;
    Ok(model)
}

/// Parse the `col` table's JSON blobs (models, decks, dconf) — the v11 layout.
fn read_v11_meta(conn: &Connection, model: &mut CanonicalModel) -> CoreResult<()> {
    let (models, decks, dconf): (String, String, String) = conn
        .query_row("SELECT models, decks, dconf FROM col LIMIT 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map_err(fmt)?;

    let models: Value = serde_json::from_str(&models).map_err(fmt)?;
    for entry in models
        .as_object()
        .ok_or_else(|| fmt("models is not an object"))?
        .values()
    {
        let id = json_i64(entry, "id").ok_or_else(|| fmt("notetype missing id"))?;
        model.notetypes.push(Notetype {
            id,
            name: json_str(entry, "name"),
            kind: json_i64(entry, "type").unwrap_or(0),
            mod_ms: json_i64(entry, "mod").unwrap_or(0) * 1000,
            usn: -1,
            config_json: entry.to_string(),
        });
        if let Some(flds) = entry.get("flds").and_then(Value::as_array) {
            for f in flds {
                model.fields.push(Field {
                    notetype_id: id,
                    ord: json_i64(f, "ord").unwrap_or(0),
                    name: json_str(f, "name"),
                    config_json: "{}".into(),
                });
            }
        }
        if let Some(tmpls) = entry.get("tmpls").and_then(Value::as_array) {
            for t in tmpls {
                model.templates.push(Template {
                    notetype_id: id,
                    ord: json_i64(t, "ord").unwrap_or(0),
                    name: json_str(t, "name"),
                    qfmt: json_str(t, "qfmt"),
                    afmt: json_str(t, "afmt"),
                    config_json: "{}".into(),
                });
            }
        }
    }

    let decks: Value = serde_json::from_str(&decks).map_err(fmt)?;
    for entry in decks
        .as_object()
        .ok_or_else(|| fmt("decks is not an object"))?
        .values()
    {
        let id = json_i64(entry, "id").ok_or_else(|| fmt("deck missing id"))?;
        model.decks.push(Deck {
            id,
            // Anki may store the deck-name separator as 0x1f; normalise to "::".
            name: json_str(entry, "name").replace('\u{1f}', "::"),
            parent_id: None,
            config_id: json_i64(entry, "conf").unwrap_or(1),
            mod_ms: json_i64(entry, "mod").unwrap_or(0) * 1000,
            usn: -1,
            collapsed: entry
                .get("collapsed")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            is_filtered: json_i64(entry, "dyn").unwrap_or(0) != 0,
        });
    }

    let dconf: Value = serde_json::from_str(&dconf).map_err(fmt)?;
    if let Some(obj) = dconf.as_object() {
        for entry in obj.values() {
            if let Some(id) = json_i64(entry, "id") {
                model.deck_configs.push(DeckConfig {
                    id,
                    name: json_str(entry, "name"),
                    mod_ms: 0,
                    usn: -1,
                    config_json: entry.to_string(),
                });
            }
        }
    }

    Ok(())
}

fn read_notes(conn: &Connection, model: &mut CanonicalModel) -> CoreResult<()> {
    let mut stmt = conn
        .prepare("SELECT id, guid, mid, mod, tags, flds, CAST(sfld AS TEXT), csum FROM notes")
        .map_err(fmt)?;
    let rows = stmt
        .query_map([], |r| {
            let tags: String = r.get(4)?;
            let flds: String = r.get(5)?;
            Ok(Note {
                id: r.get(0)?,
                guid: r.get(1)?,
                notetype_id: r.get(2)?,
                mod_ms: r.get::<_, i64>(3)? * 1000,
                usn: -1,
                tags: tags.split_whitespace().map(str::to_string).collect(),
                fields: flds.split('\u{1f}').map(str::to_string).collect(),
                sort_field: r.get::<_, Option<String>>(6)?,
                checksum: r.get::<_, Option<i64>>(7)?,
            })
        })
        .map_err(fmt)?;
    model.notes = rows.collect::<rusqlite::Result<_>>().map_err(fmt)?;
    Ok(())
}

fn read_cards(conn: &Connection, model: &mut CanonicalModel) -> CoreResult<()> {
    let mut stmt = conn
        .prepare(
            "SELECT id, nid, did, ord, mod, type, queue, due, ivl, factor, reps, lapses, left, \
             odue, odid, flags, data FROM cards",
        )
        .map_err(fmt)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Card {
                id: r.get(0)?,
                note_id: r.get(1)?,
                deck_id: r.get(2)?,
                ord: r.get(3)?,
                mod_ms: r.get::<_, i64>(4)? * 1000,
                usn: -1,
                ctype: r.get(5)?,
                queue: r.get(6)?,
                due: r.get(7)?,
                interval: r.get(8)?,
                ease_factor: r.get(9)?,
                reps: r.get(10)?,
                lapses: r.get(11)?,
                remaining: r.get(12)?,
                original_due: r.get(13)?,
                original_deck_id: r.get(14)?,
                flags: r.get(15)?,
                fsrs_stability: None,
                fsrs_difficulty: None,
                fsrs_last_review: None,
                data: r.get::<_, Option<String>>(16)?,
            })
        })
        .map_err(fmt)?;
    model.cards = rows.collect::<rusqlite::Result<_>>().map_err(fmt)?;
    Ok(())
}

fn read_revlog(conn: &Connection, model: &mut CanonicalModel) -> CoreResult<()> {
    let mut stmt = conn
        .prepare("SELECT id, cid, ease, ivl, lastIvl, factor, time, type FROM revlog")
        .map_err(fmt)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Revlog {
                id: r.get(0)?,
                card_id: r.get(1)?,
                usn: -1,
                ease: r.get(2)?,
                interval: r.get(3)?,
                last_interval: r.get(4)?,
                ease_factor: r.get(5)?,
                taken_ms: r.get(6)?,
                review_kind: r.get(7)?,
            })
        })
        .map_err(fmt)?;
    model.revlog = rows.collect::<rusqlite::Result<_>>().map_err(fmt)?;
    Ok(())
}

/// Extract media into `dir` using the v2 JSON `media` map ({"0":"name.png"}).
/// v3 (protobuf) media maps are not yet handled and yield 0.
fn extract_media(archive: &mut Archive, dir: &Path) -> CoreResult<u32> {
    let map_json = match read_entry_to_string(archive, "media") {
        Some(s) => s,
        None => return Ok(0),
    };
    let map: Value = serde_json::from_str(&map_json).map_err(fmt)?;
    let Some(entries) = map.as_object() else {
        return Ok(0);
    };

    std::fs::create_dir_all(dir).map_err(fmt)?;
    let mut count = 0;
    for (index, filename) in entries {
        let Some(filename) = filename.as_str() else {
            continue;
        };
        // Use only the file-name component to prevent path traversal.
        let Some(safe) = Path::new(filename).file_name() else {
            continue;
        };

        let mut bytes = Vec::new();
        match archive.by_name(index) {
            Ok(mut entry) => entry.read_to_end(&mut bytes).map_err(fmt)?,
            Err(_) => continue,
        };
        std::fs::write(dir.join(safe), &bytes).map_err(fmt)?;
        count += 1;
    }
    Ok(count)
}

fn read_entry_to_string(archive: &mut Archive, name: &str) -> Option<String> {
    let mut entry = archive.by_name(name).ok()?;
    let mut s = String::new();
    entry.read_to_string(&mut s).ok()?;
    Some(s)
}

fn json_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    })
}

fn json_str(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}
