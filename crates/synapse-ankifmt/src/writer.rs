//! Write a [`CanonicalModel`] as an Anki v11 `.apkg` file.
//!
//! Format: ZIP containing `collection.anki2` (SQLite v11 schema with JSON-blob
//! `col` table) + `media` (JSON index `{"0":"filename",...}`) + numbered media
//! blobs copied from the on-disk media directory (if provided).
//!
//! v11 is chosen for maximum compatibility: every Anki version ≥ 2.0 can open
//! it. v18 / `.colpkg` export can be added later behind the same `write_apkg`
//! signature.

use std::io::Write;
use std::path::Path;

use rusqlite::{params, Connection};
use serde_json::{json, Map, Value};
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::model::{CanonicalModel, Card, DeckConfig, Note, Notetype, Revlog};

const FIELD_SEP: char = '\u{1f}';

fn fmt(e: impl std::fmt::Display) -> CoreError {
    CoreError::Format(e.to_string())
}

/// Write `model` as an `.apkg` file to `dest_path`. If `media_dir` is `Some`,
/// all files in that directory are bundled as media entries.
pub fn write_apkg(
    model: &CanonicalModel,
    dest_path: &Path,
    media_dir: Option<&Path>,
) -> CoreResult<u32> {
    // 1. Build the v11 SQLite DB in memory, then grab its bytes.
    let db_bytes = build_sqlite(model)?;

    // 2. Collect media files.
    let media_files = collect_media(media_dir)?;

    // 3. Write the ZIP.
    let file = std::fs::File::create(dest_path).map_err(fmt)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    // collection.anki2
    zip.start_file("collection.anki2", opts).map_err(fmt)?;
    zip.write_all(&db_bytes).map_err(fmt)?;

    // numbered media blobs + JSON index
    let mut index: Map<String, Value> = Map::new();
    for (i, (filename, bytes)) in media_files.iter().enumerate() {
        let entry = i.to_string();
        zip.start_file(&entry, opts).map_err(fmt)?;
        zip.write_all(bytes).map_err(fmt)?;
        index.insert(entry, Value::String(filename.clone()));
    }
    let media_count = media_files.len() as u32;

    zip.start_file("media", opts).map_err(fmt)?;
    zip.write_all(serde_json::to_string(&Value::Object(index)).map_err(fmt)?.as_bytes())
        .map_err(fmt)?;

    zip.finish().map_err(fmt)?;
    Ok(media_count)
}

fn collect_media(media_dir: Option<&Path>) -> CoreResult<Vec<(String, Vec<u8>)>> {
    let Some(dir) = media_dir else {
        return Ok(vec![]);
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let bytes = std::fs::read(&path).map_err(fmt)?;
        out.push((name.to_string(), bytes));
    }
    Ok(out)
}

// ── SQLite v11 writer ────────────────────────────────────────────────────────

fn build_sqlite(model: &CanonicalModel) -> CoreResult<Vec<u8>> {
    let dir = tempfile::tempdir().map_err(fmt)?;
    let db_path = dir.path().join("collection.anki2");

    {
        let conn = Connection::open(&db_path).map_err(fmt)?;
        create_v11_schema(&conn)?;
        insert_col(&conn, model)?;
        insert_notes(&conn, &model.notes)?;
        insert_cards(&conn, &model.cards)?;
        insert_revlog(&conn, &model.revlog)?;
    }

    let bytes = std::fs::read(&db_path).map_err(fmt)?;
    Ok(bytes)
}

fn create_v11_schema(conn: &Connection) -> CoreResult<()> {
    conn.execute_batch(
        "PRAGMA journal_mode=DELETE;
         CREATE TABLE IF NOT EXISTS col (
             id    INTEGER PRIMARY KEY,
             crt   INTEGER NOT NULL,
             mod   INTEGER NOT NULL,
             scm   INTEGER NOT NULL,
             ver   INTEGER NOT NULL,
             dty   INTEGER NOT NULL,
             usn   INTEGER NOT NULL,
             ls    INTEGER NOT NULL,
             conf  TEXT NOT NULL,
             models TEXT NOT NULL,
             decks  TEXT NOT NULL,
             dconf  TEXT NOT NULL,
             tags   TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS notes (
             id    INTEGER PRIMARY KEY,
             guid  TEXT NOT NULL,
             mid   INTEGER NOT NULL,
             mod   INTEGER NOT NULL,
             usn   INTEGER NOT NULL,
             tags  TEXT NOT NULL,
             flds  TEXT NOT NULL,
             sfld  INTEGER NOT NULL,
             csum  INTEGER NOT NULL,
             flags INTEGER NOT NULL,
             data  TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS cards (
             id    INTEGER PRIMARY KEY,
             nid   INTEGER NOT NULL,
             did   INTEGER NOT NULL,
             ord   INTEGER NOT NULL,
             mod   INTEGER NOT NULL,
             usn   INTEGER NOT NULL,
             type  INTEGER NOT NULL,
             queue INTEGER NOT NULL,
             due   INTEGER NOT NULL,
             ivl   INTEGER NOT NULL,
             factor INTEGER NOT NULL,
             reps  INTEGER NOT NULL,
             lapses INTEGER NOT NULL,
             left  INTEGER NOT NULL,
             odue  INTEGER NOT NULL,
             odid  INTEGER NOT NULL,
             flags INTEGER NOT NULL,
             data  TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS revlog (
             id      INTEGER PRIMARY KEY,
             cid     INTEGER NOT NULL,
             usn     INTEGER NOT NULL,
             ease    INTEGER NOT NULL,
             ivl     INTEGER NOT NULL,
             lastIvl INTEGER NOT NULL,
             factor  INTEGER NOT NULL,
             time    INTEGER NOT NULL,
             type    INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS graves (
             usn  INTEGER NOT NULL,
             oid  INTEGER NOT NULL,
             type INTEGER NOT NULL
         );",
    )
    .map_err(fmt)
}

fn insert_col(conn: &Connection, model: &CanonicalModel) -> CoreResult<()> {
    let now_sec = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let models_json = build_models_json(model);
    let decks_json = build_decks_json(model);
    let dconf_json = build_dconf_json(model);

    conn.execute(
        "INSERT INTO col (id, crt, mod, scm, ver, dty, usn, ls, conf, models, decks, dconf, tags)
         VALUES (1, ?1, ?2, ?3, 11, 0, 0, 0, '{}', ?4, ?5, ?6, '{}')",
        params![now_sec, now_sec, now_sec, models_json, decks_json, dconf_json],
    )
    .map_err(fmt)?;
    Ok(())
}

fn build_models_json(model: &CanonicalModel) -> String {
    let mut map = Map::new();
    for nt in &model.notetypes {
        let fields: Vec<Value> = model
            .fields
            .iter()
            .filter(|f| f.notetype_id == nt.id)
            .map(|f| {
                json!({
                    "name": f.name,
                    "ord": f.ord,
                    "sticky": false,
                    "rtl": false,
                    "font": "Arial",
                    "size": 20,
                    "searchable": false
                })
            })
            .collect();

        let tmpls: Vec<Value> = model
            .templates
            .iter()
            .filter(|t| t.notetype_id == nt.id)
            .map(|t| {
                json!({
                    "name": t.name,
                    "ord": t.ord,
                    "qfmt": t.qfmt,
                    "afmt": t.afmt,
                    "bqfmt": "",
                    "bafmt": "",
                    "did": null,
                    "bfont": "",
                    "bsize": 0
                })
            })
            .collect();

        let entry = notetype_to_v11(nt, fields, tmpls);
        map.insert(nt.id.to_string(), entry);
    }
    serde_json::to_string(&Value::Object(map)).unwrap_or_else(|_| "{}".into())
}

fn notetype_to_v11(nt: &Notetype, fields: Vec<Value>, tmpls: Vec<Value>) -> Value {
    // Try to extract css from config_json; default to empty.
    let css = serde_json::from_str::<Value>(&nt.config_json)
        .ok()
        .and_then(|v| v.get("css").and_then(|c| c.as_str()).map(str::to_string))
        .unwrap_or_default();

    json!({
        "id": nt.id,
        "name": nt.name,
        "type": nt.kind,
        "mod": nt.mod_ms / 1000,
        "usn": nt.usn,
        "sortf": 0,
        "did": null,
        "tmpls": tmpls,
        "flds": fields,
        "css": css,
        "latexPre": "\\documentclass[12pt]{article}\n\\special{papersize=3in,5in}\n\\usepackage[utf8]{inputenc}\n\\usepackage{amssymb,amsmath}\n\\pagestyle{empty}\n\\setlength{\\parindent}{0in}\n\\begin{document}\n",
        "latexPost": "\\end{document}",
        "latexsvg": false,
        "req": [],
        "tags": [],
        "vers": []
    })
}

fn build_decks_json(model: &CanonicalModel) -> String {
    if model.decks.is_empty() {
        return r#"{"1":{"id":1,"name":"Default","conf":1,"mod":0,"usn":-1,"lrnToday":[0,0],"revToday":[0,0],"newToday":[0,0],"timeToday":[0,0],"collapsed":false,"dyn":0,"desc":""}}"#.into();
    }

    let mut map = Map::new();
    for d in &model.decks {
        let entry = json!({
            "id": d.id,
            "name": d.name,
            "conf": d.config_id,
            "mod": d.mod_ms / 1000,
            "usn": d.usn,
            "lrnToday": [0, 0],
            "revToday": [0, 0],
            "newToday": [0, 0],
            "timeToday": [0, 0],
            "collapsed": d.collapsed,
            "dyn": if d.is_filtered { 1 } else { 0 },
            "desc": ""
        });
        map.insert(d.id.to_string(), entry);
    }
    serde_json::to_string(&Value::Object(map)).unwrap_or_else(|_| "{}".into())
}

fn build_dconf_json(model: &CanonicalModel) -> String {
    let mut map = Map::new();
    let configs: &[DeckConfig] = &model.deck_configs;

    // If no configs in model, emit a minimal default.
    if configs.is_empty() {
        map.insert(
            "1".into(),
            json!({
                "id": 1,
                "name": "Default",
                "mod": 0,
                "usn": -1,
                "maxTaken": 60,
                "autoplay": true,
                "timer": 0,
                "replayq": true,
                "new": {"bury": true, "delays": [1, 10], "initialFactor": 2500, "ints": [1, 4, 7], "order": 1, "perDay": 20},
                "lapse": {"delays": [10], "leechAction": 0, "leechFails": 8, "minInt": 1, "mult": 0.0},
                "rev": {"bury": false, "ease4": 1.3, "fuzz": 0.05, "ivlFct": 1.0, "maxIvl": 36500, "minSpace": 1, "perDay": 100}
            }),
        );
    } else {
        for cfg in configs {
            // Try to reuse existing config JSON; fall back to minimal.
            let entry = match serde_json::from_str::<Value>(&cfg.config_json) {
                Ok(mut v) => {
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("id".into(), json!(cfg.id));
                        obj.insert("name".into(), json!(cfg.name));
                        obj.insert("mod".into(), json!(cfg.mod_ms / 1000));
                        obj.insert("usn".into(), json!(cfg.usn));
                    }
                    v
                }
                Err(_) => json!({"id": cfg.id, "name": cfg.name, "mod": cfg.mod_ms / 1000, "usn": cfg.usn}),
            };
            map.insert(cfg.id.to_string(), entry);
        }
    }
    serde_json::to_string(&Value::Object(map)).unwrap_or_else(|_| "{}".into())
}

fn insert_notes(conn: &Connection, notes: &[Note]) -> CoreResult<()> {
    let mut stmt = conn
        .prepare(
            "INSERT INTO notes (id, guid, mid, mod, usn, tags, flds, sfld, csum, flags, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, '')",
        )
        .map_err(fmt)?;

    for n in notes {
        let tags = if n.tags.is_empty() {
            String::new()
        } else {
            format!(" {} ", n.tags.join(" "))
        };
        let flds = n.fields.join(&FIELD_SEP.to_string());
        let sfld = n.sort_field.as_deref().unwrap_or_default();
        let csum = n.checksum.unwrap_or(0);
        stmt.execute(params![
            n.id,
            n.guid,
            n.notetype_id,
            n.mod_ms / 1000,
            n.usn,
            tags,
            flds,
            sfld,
            csum,
        ])
        .map_err(fmt)?;
    }
    Ok(())
}

fn insert_cards(conn: &Connection, cards: &[Card]) -> CoreResult<()> {
    let mut stmt = conn
        .prepare(
            "INSERT INTO cards (id, nid, did, ord, mod, usn, type, queue, due, ivl,
                                factor, reps, lapses, left, odue, odid, flags, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                     ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        )
        .map_err(fmt)?;

    for c in cards {
        stmt.execute(params![
            c.id,
            c.note_id,
            c.deck_id,
            c.ord,
            c.mod_ms / 1000,
            c.usn,
            c.ctype,
            c.queue,
            c.due,
            c.interval,
            c.ease_factor,
            c.reps,
            c.lapses,
            c.remaining,
            c.original_due,
            c.original_deck_id,
            c.flags,
            c.data.as_deref().unwrap_or(""),
        ])
        .map_err(fmt)?;
    }
    Ok(())
}

fn insert_revlog(conn: &Connection, revlog: &[Revlog]) -> CoreResult<()> {
    let mut stmt = conn
        .prepare(
            "INSERT INTO revlog (id, cid, usn, ease, ivl, lastIvl, factor, time, type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .map_err(fmt)?;

    for r in revlog {
        stmt.execute(params![
            r.id,
            r.card_id,
            r.usn,
            r.ease,
            r.interval,
            r.last_interval,
            r.ease_factor,
            r.taken_ms,
            r.review_kind,
        ])
        .map_err(fmt)?;
    }
    Ok(())
}

