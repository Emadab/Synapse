//! Note browser queries + note editing. Free functions over a `Connection`,
//! called by `SqliteStorage`.

use rusqlite::{params, Connection, OptionalExtension};
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::ipc::{NoteDetail, NoteField, NoteOverview};

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
