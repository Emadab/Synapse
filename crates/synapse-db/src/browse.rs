//! Note browser queries + note editing. Free functions over a `Connection`,
//! called by `SqliteStorage`.

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::ipc::{NoteDetail, NoteField, NoteOverview};
use synapse_core::model::{Field, NoteIndexRow, Notetype, Template};

const FIELD_SEP: char = '\u{1f}';

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Storage(e.to_string())
}

fn split_tags(tags: &str) -> Vec<String> {
    tags.split_whitespace().map(str::to_string).collect()
}

/// Anki stores tags space-delimited with surrounding spaces; we mirror that.
fn join_tags(tags: &[String]) -> String {
    if tags.is_empty() {
        String::new()
    } else {
        format!(" {} ", tags.join(" "))
    }
}

/// Remove HTML tags for the plaintext sort field.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

pub fn list_notes(
    conn: &Connection,
    query: Option<&str>,
    limit: i64,
) -> CoreResult<Vec<NoteOverview>> {
    let map = |r: &rusqlite::Row<'_>| {
        let tags: String = r.get(2)?;
        Ok(NoteOverview {
            note_id: r.get(0)?,
            sort_field: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            tags: split_tags(&tags),
        })
    };

    match query.map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => {
            let like = format!("%{q}%");
            let mut stmt = conn
                .prepare(
                    "SELECT id, sort_field, tags FROM notes
                     WHERE fields LIKE ?1 OR tags LIKE ?1
                     ORDER BY id DESC LIMIT ?2",
                )
                .map_err(err)?;
            let rows = stmt.query_map(params![like, limit], map).map_err(err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(err)
        }
        None => {
            let mut stmt = conn
                .prepare("SELECT id, sort_field, tags FROM notes ORDER BY id DESC LIMIT ?1")
                .map_err(err)?;
            let rows = stmt.query_map([limit], map).map_err(err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(err)
        }
    }
}

pub fn note_detail(conn: &Connection, note_id: i64) -> CoreResult<Option<NoteDetail>> {
    let row = conn
        .query_row(
            "SELECT notetype_id, fields, tags FROM notes WHERE id = ?1",
            [note_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(err)?;
    let Some((notetype_id, fields, tags)) = row else {
        return Ok(None);
    };

    let notetype_name: String = conn
        .query_row(
            "SELECT name FROM notetypes WHERE id = ?1",
            [notetype_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?
        .unwrap_or_default();

    let mut stmt = conn
        .prepare("SELECT name FROM fields WHERE notetype_id = ?1 ORDER BY ord")
        .map_err(err)?;
    let names: Vec<String> = stmt
        .query_map([notetype_id], |r| r.get(0))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;

    let fields = names
        .into_iter()
        .zip(fields.split(FIELD_SEP).map(str::to_string))
        .map(|(name, value)| NoteField { name, value })
        .collect();

    Ok(Some(NoteDetail {
        note_id,
        notetype_name,
        fields,
        tags: split_tags(&tags),
    }))
}

pub fn update_note(
    conn: &Connection,
    note_id: i64,
    fields: &[String],
    tags: &[String],
    now_ms: i64,
) -> CoreResult<()> {
    let joined = fields.join(&FIELD_SEP.to_string());
    let sort_field = fields.first().map(|f| strip_tags(f)).unwrap_or_default();
    let affected = conn
        .execute(
            r#"UPDATE notes SET fields = ?2, sort_field = ?3, tags = ?4, "mod" = ?5, usn = -1
               WHERE id = ?1"#,
            params![note_id, joined, sort_field, join_tags(tags), now_ms],
        )
        .map_err(err)?;
    if affected == 0 {
        return Err(CoreError::NotFound(format!("note {note_id}")));
    }
    Ok(())
}

/// Flatten every note for the search index: plaintext field content + the
/// note's note-type and (one of) its deck names.
pub fn index_rows(conn: &Connection) -> CoreResult<Vec<NoteIndexRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT n.id, n.fields, n.tags,
                COALESCE((SELECT d.name FROM cards c JOIN decks d ON d.id = c.deck_id
                          WHERE c.note_id = n.id LIMIT 1), ''),
                COALESCE(nt.name, '')
             FROM notes n LEFT JOIN notetypes nt ON nt.id = n.notetype_id",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            let fields: String = r.get(1)?;
            Ok(NoteIndexRow {
                note_id: r.get(0)?,
                text: strip_tags(&fields.replace(FIELD_SEP, " ")),
                tags: r.get(2)?,
                deck: r.get(3)?,
                notetype: r.get(4)?,
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

/// Browser rows for a set of note ids (search hits). Order is unspecified;
/// callers that need rank order should reorder by their id list.
pub fn notes_by_ids(conn: &Connection, ids: &[i64]) -> CoreResult<Vec<NoteOverview>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let json = serde_json::to_string(ids).map_err(err)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, sort_field, tags FROM notes
             WHERE id IN (SELECT value FROM json_each(?1))",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([json], |r| {
            let tags: String = r.get(2)?;
            Ok(NoteOverview {
                note_id: r.get(0)?,
                sort_field: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                tags: split_tags(&tags),
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

pub fn list_notetypes(conn: &Connection) -> CoreResult<Vec<Notetype>> {
    let mut stmt = conn
        .prepare(r#"SELECT id, name, kind, "mod", usn, config FROM notetypes ORDER BY name"#)
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Notetype {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                mod_ms: r.get(3)?,
                usn: r.get(4)?,
                config_json: r.get(5)?,
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

pub fn fields_for_notetype(conn: &Connection, notetype_id: i64) -> CoreResult<Vec<Field>> {
    let mut stmt = conn
        .prepare(
            "SELECT notetype_id, ord, name, config FROM fields \
             WHERE notetype_id = ?1 ORDER BY ord",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([notetype_id], |r| {
            Ok(Field {
                notetype_id: r.get(0)?,
                ord: r.get(1)?,
                name: r.get(2)?,
                config_json: r.get(3)?,
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

pub fn templates_for_notetype(conn: &Connection, notetype_id: i64) -> CoreResult<Vec<Template>> {
    let mut stmt = conn
        .prepare(
            "SELECT notetype_id, ord, name, qfmt, afmt, config FROM templates \
             WHERE notetype_id = ?1 ORDER BY ord",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([notetype_id], |r| {
            Ok(Template {
                notetype_id: r.get(0)?,
                ord: r.get(1)?,
                name: r.get(2)?,
                qfmt: r.get(3)?,
                afmt: r.get(4)?,
                config_json: r.get(5)?,
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

/// Scan field values for `{{cN::…}}` markers and return the highest ordinal found.
/// Returns 0 when no cloze markers exist.
fn max_cloze_ordinal(fields: &[String]) -> u32 {
    let mut max = 0u32;
    for field in fields {
        let mut s = field.as_str();
        while let Some(pos) = s.find("{{c") {
            s = &s[pos + 3..];
            let end = s.find(':').unwrap_or(0);
            if let Ok(n) = s[..end].parse::<u32>() {
                if n > max {
                    max = n;
                }
            }
        }
    }
    max
}

fn insert_new_card(
    tx: &Transaction<'_>,
    note_id: i64,
    deck_id: i64,
    ord: i64,
    now_ms: i64,
) -> CoreResult<()> {
    // `due` for new cards = position in the global new queue (1-indexed, monotone).
    let next_pos: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(due), 0) + 1 FROM cards WHERE queue = 0",
            [],
            |r| r.get(0),
        )
        .map_err(err)?;
    tx.execute(
        r#"INSERT INTO cards
           (note_id, deck_id, ord, "mod", usn, type, queue, due,
            interval, ease_factor, reps, lapses, remaining,
            original_due, original_deck_id, flags)
           VALUES (?1, ?2, ?3, ?4, -1, 0, 0, ?5, 0, 0, 0, 0, 0, 0, 0, 0)"#,
        params![note_id, deck_id, ord, now_ms, next_pos],
    )
    .map_err(err)?;
    Ok(())
}

/// Insert a note and generate its cards in a single transaction.
/// Standard notetypes (kind = 0): one card per template.
/// Cloze notetypes (kind = 1): one card per cloze ordinal found in the fields.
pub fn add_note_with_cards(
    tx: &Transaction<'_>,
    notetype_id: i64,
    deck_id: i64,
    fields: &[String],
    tags: &[String],
    now_ms: i64,
) -> CoreResult<(i64, u32)> {
    let kind: i64 = tx
        .query_row(
            "SELECT kind FROM notetypes WHERE id = ?1",
            [notetype_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?
        .ok_or_else(|| CoreError::NotFound(format!("notetype {notetype_id}")))?;

    let fields_blob = fields.join("\u{1f}");
    let sort_field = fields.first().map(|f| strip_tags(f)).unwrap_or_default();
    let tags_str = join_tags(tags);

    // Insert note with a placeholder GUID; update it to the rowid afterwards.
    tx.execute(
        r#"INSERT INTO notes (guid, notetype_id, "mod", usn, tags, fields, sort_field)
           VALUES ('', ?1, ?2, -1, ?3, ?4, ?5)"#,
        params![notetype_id, now_ms, tags_str, fields_blob, sort_field],
    )
    .map_err(err)?;
    let note_id = tx.last_insert_rowid();
    tx.execute(
        "UPDATE notes SET guid = ?1 WHERE id = ?2",
        params![note_id.to_string(), note_id],
    )
    .map_err(err)?;

    let cards_added = if kind == 1 {
        // Cloze: one card per cloze ordinal (0-indexed, so ord = cloze_num - 1).
        let max_ord = max_cloze_ordinal(fields);
        for ord in 0..max_ord {
            insert_new_card(tx, note_id, deck_id, ord as i64, now_ms)?;
        }
        max_ord
    } else {
        // Standard: one card per template.
        let template_count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM templates WHERE notetype_id = ?1",
                [notetype_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        for ord in 0..template_count {
            insert_new_card(tx, note_id, deck_id, ord, now_ms)?;
        }
        template_count as u32
    };

    Ok((note_id, cards_added))
}

#[cfg(test)]
mod tests {
    use crate::storage::SqliteStorage;
    use synapse_core::ports::Storage;

    fn model() -> synapse_core::model::CanonicalModel {
        use synapse_core::model::*;
        CanonicalModel {
            notetypes: vec![Notetype {
                id: 10,
                name: "Basic".into(),
                kind: 0,
                mod_ms: 0,
                usn: -1,
                config_json: "{}".into(),
            }],
            fields: vec![
                Field {
                    notetype_id: 10,
                    ord: 0,
                    name: "Front".into(),
                    config_json: "{}".into(),
                },
                Field {
                    notetype_id: 10,
                    ord: 1,
                    name: "Back".into(),
                    config_json: "{}".into(),
                },
            ],
            templates: vec![Template {
                notetype_id: 10,
                ord: 0,
                name: "Card 1".into(),
                qfmt: "{{Front}}".into(),
                afmt: "{{Back}}".into(),
                config_json: "{}".into(),
            }],
            notes: vec![Note {
                id: 100,
                guid: "g1".into(),
                notetype_id: 10,
                mod_ms: 0,
                usn: -1,
                tags: vec!["spanish".into()],
                fields: vec!["<b>hola</b>".into(), "hello".into()],
                sort_field: Some("hola".into()),
                checksum: None,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn list_detail_and_update() {
        let s = SqliteStorage::open_in_memory().unwrap();
        s.import(&model()).unwrap();

        let all = s.list_notes(None, 100).unwrap();
        assert_eq!(all.len(), 1);
        let note_id = all[0].note_id;
        assert_eq!(all[0].tags, vec!["spanish".to_string()]);

        // Substring search hits the field text.
        assert_eq!(s.list_notes(Some("hello"), 100).unwrap().len(), 1);
        assert_eq!(s.list_notes(Some("nope"), 100).unwrap().len(), 0);

        let detail = s.note_detail(note_id).unwrap().unwrap();
        assert_eq!(detail.notetype_name, "Basic");
        assert_eq!(detail.fields.len(), 2);
        assert_eq!(detail.fields[0].name, "Front");
        assert_eq!(detail.fields[0].value, "<b>hola</b>");

        s.update_note(
            note_id,
            &["<i>adios</i>".into(), "bye".into()],
            &["es".into()],
            999,
        )
        .unwrap();
        let updated = s.note_detail(note_id).unwrap().unwrap();
        assert_eq!(updated.fields[0].value, "<i>adios</i>");
        assert_eq!(updated.tags, vec!["es".to_string()]);
        // Sort field is the plaintext of the first field.
        assert_eq!(s.list_notes(None, 100).unwrap()[0].sort_field, "adios");
    }
}
