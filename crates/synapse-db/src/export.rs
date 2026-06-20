//! Dump the full canonical collection for export.

use rusqlite::Connection;
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::model::{
    CanonicalModel, Card, Deck, DeckConfig, Field, Note, Notetype, Revlog, Template,
};

const FIELD_SEP: char = '\u{1f}';

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Storage(e.to_string())
}

pub fn dump_collection(conn: &Connection) -> CoreResult<CanonicalModel> {
    Ok(CanonicalModel {
        deck_configs: dump_deck_configs(conn)?,
        decks: dump_decks(conn)?,
        notetypes: dump_notetypes(conn)?,
        fields: dump_fields(conn)?,
        templates: dump_templates(conn)?,
        notes: dump_notes(conn)?,
        cards: dump_cards(conn)?,
        revlog: dump_revlog(conn)?,
        ..Default::default()
    })
}

fn dump_deck_configs(conn: &Connection) -> CoreResult<Vec<DeckConfig>> {
    let mut stmt = conn
        .prepare("SELECT id, name, mod, usn, config FROM deck_config")
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(DeckConfig {
                id: r.get(0)?,
                name: r.get(1)?,
                mod_ms: r.get::<_, i64>(2)?,
                usn: r.get(3)?,
                config_json: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

fn dump_decks(conn: &Connection) -> CoreResult<Vec<Deck>> {
    let mut stmt = conn
        .prepare(
            r#"SELECT id, name, parent_id, config_id, "mod", usn, collapsed, is_filtered
               FROM decks ORDER BY id"#,
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Deck {
                id: r.get(0)?,
                name: r.get(1)?,
                parent_id: r.get(2)?,
                config_id: r.get(3)?,
                mod_ms: r.get(4)?,
                usn: r.get(5)?,
                collapsed: r.get::<_, i64>(6)? != 0,
                is_filtered: r.get::<_, i64>(7)? != 0,
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

fn dump_notetypes(conn: &Connection) -> CoreResult<Vec<Notetype>> {
    let mut stmt = conn
        .prepare("SELECT id, name, kind, mod, usn, config FROM notetypes ORDER BY id")
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Notetype {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                mod_ms: r.get::<_, i64>(3)?,
                usn: r.get(4)?,
                config_json: r.get::<_, Option<String>>(5)?.unwrap_or_else(|| "{}".into()),
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

fn dump_fields(conn: &Connection) -> CoreResult<Vec<Field>> {
    let mut stmt = conn
        .prepare("SELECT notetype_id, ord, name, config FROM fields ORDER BY notetype_id, ord")
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Field {
                notetype_id: r.get(0)?,
                ord: r.get(1)?,
                name: r.get(2)?,
                config_json: r.get::<_, Option<String>>(3)?.unwrap_or_else(|| "{}".into()),
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

fn dump_templates(conn: &Connection) -> CoreResult<Vec<Template>> {
    let mut stmt = conn
        .prepare(
            "SELECT notetype_id, ord, name, qfmt, afmt, config
             FROM templates ORDER BY notetype_id, ord",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Template {
                notetype_id: r.get(0)?,
                ord: r.get(1)?,
                name: r.get(2)?,
                qfmt: r.get(3)?,
                afmt: r.get(4)?,
                config_json: r.get::<_, Option<String>>(5)?.unwrap_or_else(|| "{}".into()),
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

fn dump_notes(conn: &Connection) -> CoreResult<Vec<Note>> {
    let mut stmt = conn
        .prepare(
            r#"SELECT id, guid, notetype_id, "mod", usn, tags, fields, sort_field, checksum
               FROM notes ORDER BY id"#,
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            let tags: String = r.get(5)?;
            let fields: String = r.get(6)?;
            Ok(Note {
                id: r.get(0)?,
                guid: r.get(1)?,
                notetype_id: r.get(2)?,
                mod_ms: r.get(3)?,
                usn: r.get(4)?,
                tags: tags.split_whitespace().map(str::to_string).collect(),
                fields: fields.split(FIELD_SEP).map(str::to_string).collect(),
                sort_field: r.get(7)?,
                checksum: r.get(8)?,
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

fn dump_cards(conn: &Connection) -> CoreResult<Vec<Card>> {
    let mut stmt = conn
        .prepare(
            r#"SELECT id, note_id, deck_id, ord, "mod", usn, type, queue, due, interval,
                      ease_factor, reps, lapses, remaining, original_due, original_deck_id,
                      flags, fsrs_stability, fsrs_difficulty, fsrs_last_review, data
               FROM cards ORDER BY id"#,
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Card {
                id: r.get(0)?,
                note_id: r.get(1)?,
                deck_id: r.get(2)?,
                ord: r.get(3)?,
                mod_ms: r.get(4)?,
                usn: r.get(5)?,
                ctype: r.get(6)?,
                queue: r.get(7)?,
                due: r.get(8)?,
                interval: r.get(9)?,
                ease_factor: r.get(10)?,
                reps: r.get(11)?,
                lapses: r.get(12)?,
                remaining: r.get(13)?,
                original_due: r.get(14)?,
                original_deck_id: r.get(15)?,
                flags: r.get(16)?,
                fsrs_stability: r.get(17)?,
                fsrs_difficulty: r.get(18)?,
                fsrs_last_review: r.get(19)?,
                data: r.get(20)?,
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

fn dump_revlog(conn: &Connection) -> CoreResult<Vec<Revlog>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, card_id, usn, ease, interval, last_interval,
                    ease_factor, taken_ms, review_kind
             FROM revlog ORDER BY id",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Revlog {
                id: r.get(0)?,
                card_id: r.get(1)?,
                usn: r.get(2)?,
                ease: r.get(3)?,
                interval: r.get(4)?,
                last_interval: r.get(5)?,
                ease_factor: r.get(6)?,
                taken_ms: r.get(7)?,
                review_kind: r.get(8)?,
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}
