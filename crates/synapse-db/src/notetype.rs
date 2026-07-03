//! Note-type CRUD: fields, templates, and note-type lifecycle.

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::ipc::{FieldRemoveWarning, FieldSummary, NotetypeDetail, TemplateSummary};

const FIELD_SEP: char = '\u{1f}';

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Storage(e.to_string())
}

fn bump_schema_mod(conn: &Connection, now_ms: i64) -> CoreResult<()> {
    conn.execute(
        "UPDATE collection SET schema_mod = ?1, modified = ?1 WHERE id = 1",
        [now_ms],
    )
    .map_err(err)?;
    Ok(())
}

pub fn get_notetype_detail(
    conn: &Connection,
    notetype_id: i64,
) -> CoreResult<Option<NotetypeDetail>> {
    let row = conn
        .query_row(
            "SELECT id, name, kind, coalesce(json_extract(config, '$.css'), '') FROM notetypes WHERE id = ?1",
            [notetype_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(err)?;
    let Some((id, name, kind, css)) = row else {
        return Ok(None);
    };

    let mut stmt = conn
        .prepare("SELECT ord, name FROM fields WHERE notetype_id = ?1 ORDER BY ord")
        .map_err(err)?;
    let fields: Vec<FieldSummary> = stmt
        .query_map([id], |r| {
            Ok(FieldSummary {
                ord: r.get(0)?,
                name: r.get(1)?,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;

    let mut stmt = conn
        .prepare(
            "SELECT ord, name, qfmt, afmt FROM templates \
             WHERE notetype_id = ?1 ORDER BY ord",
        )
        .map_err(err)?;
    let templates: Vec<TemplateSummary> = stmt
        .query_map([id], |r| {
            Ok(TemplateSummary {
                ord: r.get(0)?,
                name: r.get(1)?,
                qfmt: r.get(2)?,
                afmt: r.get(3)?,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;

    Ok(Some(NotetypeDetail {
        id,
        name,
        kind,
        fields,
        templates,
        css,
    }))
}

/// Save a notetype's custom card CSS, preserving other keys in `config`.
pub fn save_notetype_css(
    conn: &Connection,
    notetype_id: i64,
    css: &str,
    now_ms: i64,
) -> CoreResult<()> {
    conn.execute(
        r#"UPDATE notetypes
           SET config = json_set(config, '$.css', ?2), "mod" = ?3, usn = -1
           WHERE id = ?1"#,
        params![notetype_id, css, now_ms],
    )
    .map_err(err)?;
    bump_schema_mod(conn, now_ms)?;
    Ok(())
}

pub fn create_notetype(
    tx: &Transaction<'_>,
    name: &str,
    kind: i64,
    now_ms: i64,
) -> CoreResult<i64> {
    let css_escaped = serde_json::to_string(crate::stock::DEFAULT_CARD_CSS).map_err(err)?;
    let config = format!("{{\"css\":{css_escaped}}}");
    tx.execute(
        r#"INSERT INTO notetypes (name, kind, "mod", usn, config) VALUES (?1, ?2, ?3, -1, ?4)"#,
        params![name, kind, now_ms, config],
    )
    .map_err(err)?;
    let id = tx.last_insert_rowid();

    if kind == 0 {
        // Standard: two fields + one Card 1 template
        tx.execute(
            "INSERT INTO fields (notetype_id, ord, name, config) VALUES (?1, 0, 'Front', '{}')",
            [id],
        )
        .map_err(err)?;
        tx.execute(
            "INSERT INTO fields (notetype_id, ord, name, config) VALUES (?1, 1, 'Back', '{}')",
            [id],
        )
        .map_err(err)?;
        tx.execute(
            r#"INSERT INTO templates (notetype_id, ord, name, qfmt, afmt, config)
               VALUES (?1, 0, 'Card 1', '{{Front}}',
                       '{{FrontSide}}<hr id=answer>{{Back}}', '{}')"#,
            [id],
        )
        .map_err(err)?;
    } else {
        // Cloze: Text + Extra fields, one Cloze template
        tx.execute(
            "INSERT INTO fields (notetype_id, ord, name, config) VALUES (?1, 0, 'Text', '{}')",
            [id],
        )
        .map_err(err)?;
        tx.execute(
            "INSERT INTO fields (notetype_id, ord, name, config) VALUES (?1, 1, 'Extra', '{}')",
            [id],
        )
        .map_err(err)?;
        tx.execute(
            r#"INSERT INTO templates (notetype_id, ord, name, qfmt, afmt, config)
               VALUES (?1, 0, 'Cloze', '{{cloze:Text}}',
                       '{{cloze:Text}}<br>{{Extra}}', '{}')"#,
            [id],
        )
        .map_err(err)?;
    }

    bump_schema_mod(tx, now_ms)?;
    Ok(id)
}

pub fn delete_notetype(tx: &Transaction<'_>, notetype_id: i64, now_ms: i64) -> CoreResult<()> {
    let note_count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM notes WHERE notetype_id = ?1",
            [notetype_id],
            |r| r.get(0),
        )
        .map_err(err)?;
    if note_count > 0 {
        return Err(CoreError::Invalid(format!(
            "cannot delete note type: {note_count} note{} reference it",
            if note_count == 1 { "" } else { "s" }
        )));
    }
    let affected = tx
        .execute("DELETE FROM notetypes WHERE id = ?1", [notetype_id])
        .map_err(err)?;
    if affected == 0 {
        return Err(CoreError::NotFound(format!("notetype {notetype_id}")));
    }
    bump_schema_mod(tx, now_ms)?;
    Ok(())
}

pub fn rename_notetype(
    conn: &Connection,
    notetype_id: i64,
    name: &str,
    now_ms: i64,
) -> CoreResult<()> {
    let affected = conn
        .execute(
            r#"UPDATE notetypes SET name = ?1, "mod" = ?2, usn = -1 WHERE id = ?3"#,
            params![name, now_ms, notetype_id],
        )
        .map_err(err)?;
    if affected == 0 {
        return Err(CoreError::NotFound(format!("notetype {notetype_id}")));
    }
    Ok(())
}

pub fn add_field(
    tx: &Transaction<'_>,
    notetype_id: i64,
    name: &str,
    now_ms: i64,
) -> CoreResult<()> {
    let next_ord: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(ord) + 1, 0) FROM fields WHERE notetype_id = ?1",
            [notetype_id],
            |r| r.get(0),
        )
        .map_err(err)?;
    tx.execute(
        "INSERT INTO fields (notetype_id, ord, name, config) VALUES (?1, ?2, ?3, '{}')",
        params![notetype_id, next_ord, name],
    )
    .map_err(err)?;

    // Append FIELD_SEP + empty value to every note of this notetype.
    let note_ids: Vec<i64> = {
        let mut stmt = tx
            .prepare("SELECT id FROM notes WHERE notetype_id = ?1")
            .map_err(err)?;
        let ids = stmt
            .query_map([notetype_id], |r| r.get(0))
            .map_err(err)?
            .collect::<rusqlite::Result<_>>()
            .map_err(err)?;
        ids
    };
    for note_id in note_ids {
        tx.execute(
            r#"UPDATE notes SET fields = fields || ?1, "mod" = ?2, usn = -1 WHERE id = ?3"#,
            params![FIELD_SEP.to_string(), now_ms, note_id],
        )
        .map_err(err)?;
    }

    bump_schema_mod(tx, now_ms)?;
    Ok(())
}

pub fn check_field_remove(
    conn: &Connection,
    notetype_id: i64,
    ord: i64,
) -> CoreResult<FieldRemoveWarning> {
    let mut stmt = conn
        .prepare("SELECT fields FROM notes WHERE notetype_id = ?1")
        .map_err(err)?;
    let blobs: Vec<String> = stmt
        .query_map([notetype_id], |r| r.get(0))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;

    let ord_idx = ord as usize;
    let count = blobs
        .iter()
        .filter(|blob| {
            let parts: Vec<&str> = blob.split(FIELD_SEP).collect();
            parts
                .get(ord_idx)
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false)
        })
        .count();

    Ok(FieldRemoveWarning {
        notes_with_content: count as u32,
    })
}

pub fn remove_field(
    tx: &Transaction<'_>,
    notetype_id: i64,
    ord: i64,
    now_ms: i64,
) -> CoreResult<()> {
    let field_count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM fields WHERE notetype_id = ?1",
            [notetype_id],
            |r| r.get(0),
        )
        .map_err(err)?;
    if field_count <= 1 {
        return Err(CoreError::Invalid("cannot remove the last field".into()));
    }

    tx.execute(
        "DELETE FROM fields WHERE notetype_id = ?1 AND ord = ?2",
        params![notetype_id, ord],
    )
    .map_err(err)?;
    tx.execute(
        "UPDATE fields SET ord = ord - 1 WHERE notetype_id = ?1 AND ord > ?2",
        params![notetype_id, ord],
    )
    .map_err(err)?;

    // Splice value at position `ord` out of every note's field blob.
    let note_rows: Vec<(i64, String)> = {
        let mut stmt = tx
            .prepare("SELECT id, fields FROM notes WHERE notetype_id = ?1")
            .map_err(err)?;
        let rows = stmt
            .query_map([notetype_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(err)?
            .collect::<rusqlite::Result<_>>()
            .map_err(err)?;
        rows
    };
    let ord_idx = ord as usize;
    let sep = FIELD_SEP.to_string();
    for (note_id, blob) in note_rows {
        let mut parts: Vec<&str> = blob.split(FIELD_SEP).collect();
        if ord_idx < parts.len() {
            parts.remove(ord_idx);
        }
        let new_blob = parts.join(&sep);
        tx.execute(
            r#"UPDATE notes SET fields = ?1, "mod" = ?2, usn = -1 WHERE id = ?3"#,
            params![new_blob, now_ms, note_id],
        )
        .map_err(err)?;
    }

    bump_schema_mod(tx, now_ms)?;
    Ok(())
}

pub fn rename_field(
    conn: &Connection,
    notetype_id: i64,
    ord: i64,
    name: &str,
    now_ms: i64,
) -> CoreResult<()> {
    let affected = conn
        .execute(
            "UPDATE fields SET name = ?1 WHERE notetype_id = ?2 AND ord = ?3",
            params![name, notetype_id, ord],
        )
        .map_err(err)?;
    if affected == 0 {
        return Err(CoreError::NotFound(format!(
            "field {ord} in notetype {notetype_id}"
        )));
    }
    conn.execute(
        r#"UPDATE notetypes SET "mod" = ?1, usn = -1 WHERE id = ?2"#,
        params![now_ms, notetype_id],
    )
    .map_err(err)?;
    Ok(())
}

pub fn reorder_fields(
    tx: &Transaction<'_>,
    notetype_id: i64,
    new_order: &[i64],
    now_ms: i64,
) -> CoreResult<()> {
    // Fetch all fields ordered by their current ord (which is 0..n-1 by invariant).
    let old_fields: Vec<(String, String)> = {
        let mut stmt = tx
            .prepare(
                "SELECT name, config FROM fields \
                 WHERE notetype_id = ?1 ORDER BY ord",
            )
            .map_err(err)?;
        let fields = stmt
            .query_map([notetype_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(err)?
            .collect::<rusqlite::Result<_>>()
            .map_err(err)?;
        fields
    };

    if new_order.len() != old_fields.len() {
        return Err(CoreError::Invalid(format!(
            "reorder_fields: expected {} elements, got {}",
            old_fields.len(),
            new_order.len()
        )));
    }

    // Rebuild the fields table in the new order.
    tx.execute("DELETE FROM fields WHERE notetype_id = ?1", [notetype_id])
        .map_err(err)?;
    for (new_ord, &old_ord) in new_order.iter().enumerate() {
        let (ref name, ref config) = old_fields[old_ord as usize];
        tx.execute(
            "INSERT INTO fields (notetype_id, ord, name, config) VALUES (?1, ?2, ?3, ?4)",
            params![notetype_id, new_ord as i64, name, config],
        )
        .map_err(err)?;
    }

    // Permute each note's field values in the same order.
    let note_rows: Vec<(i64, String)> = {
        let mut stmt = tx
            .prepare("SELECT id, fields FROM notes WHERE notetype_id = ?1")
            .map_err(err)?;
        let rows = stmt
            .query_map([notetype_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(err)?
            .collect::<rusqlite::Result<_>>()
            .map_err(err)?;
        rows
    };
    let sep = FIELD_SEP.to_string();
    for (note_id, blob) in note_rows {
        let parts: Vec<&str> = blob.split(FIELD_SEP).collect();
        let new_blob: String = new_order
            .iter()
            .map(|&old_ord| parts.get(old_ord as usize).copied().unwrap_or(""))
            .collect::<Vec<_>>()
            .join(&sep);
        tx.execute(
            r#"UPDATE notes SET fields = ?1, "mod" = ?2, usn = -1 WHERE id = ?3"#,
            params![new_blob, now_ms, note_id],
        )
        .map_err(err)?;
    }

    bump_schema_mod(tx, now_ms)?;
    Ok(())
}

pub fn add_template(
    tx: &Transaction<'_>,
    notetype_id: i64,
    name: &str,
    qfmt: &str,
    afmt: &str,
    now_ms: i64,
) -> CoreResult<()> {
    let next_ord: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(ord) + 1, 0) FROM templates WHERE notetype_id = ?1",
            [notetype_id],
            |r| r.get(0),
        )
        .map_err(err)?;
    tx.execute(
        "INSERT INTO templates (notetype_id, ord, name, qfmt, afmt, config) \
         VALUES (?1, ?2, ?3, ?4, ?5, '{}')",
        params![notetype_id, next_ord, name, qfmt, afmt],
    )
    .map_err(err)?;

    // Generate a new card at `next_ord` for every existing note of this notetype,
    // placing it in the same deck as the note's first existing card.
    let note_ids: Vec<i64> = {
        let mut stmt = tx
            .prepare("SELECT id FROM notes WHERE notetype_id = ?1")
            .map_err(err)?;
        let ids = stmt
            .query_map([notetype_id], |r| r.get(0))
            .map_err(err)?
            .collect::<rusqlite::Result<_>>()
            .map_err(err)?;
        ids
    };
    for note_id in note_ids {
        let deck_id: Option<i64> = tx
            .query_row(
                "SELECT deck_id FROM cards WHERE note_id = ?1 LIMIT 1",
                [note_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        let Some(deck_id) = deck_id else { continue };

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
            params![note_id, deck_id, next_ord, now_ms, next_pos],
        )
        .map_err(err)?;
    }

    bump_schema_mod(tx, now_ms)?;
    Ok(())
}

pub fn remove_template(
    tx: &Transaction<'_>,
    notetype_id: i64,
    ord: i64,
    now_ms: i64,
) -> CoreResult<()> {
    let tmpl_count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM templates WHERE notetype_id = ?1",
            [notetype_id],
            |r| r.get(0),
        )
        .map_err(err)?;
    if tmpl_count <= 1 {
        return Err(CoreError::Invalid("cannot remove the last template".into()));
    }

    // Delete cards for this template ord across all notes of the notetype.
    tx.execute(
        "DELETE FROM cards WHERE ord = ?1 \
         AND note_id IN (SELECT id FROM notes WHERE notetype_id = ?2)",
        params![ord, notetype_id],
    )
    .map_err(err)?;
    // Shift card ords above the removed ord down by 1.
    tx.execute(
        "UPDATE cards SET ord = ord - 1 WHERE ord > ?1 \
         AND note_id IN (SELECT id FROM notes WHERE notetype_id = ?2)",
        params![ord, notetype_id],
    )
    .map_err(err)?;

    tx.execute(
        "DELETE FROM templates WHERE notetype_id = ?1 AND ord = ?2",
        params![notetype_id, ord],
    )
    .map_err(err)?;
    tx.execute(
        "UPDATE templates SET ord = ord - 1 WHERE notetype_id = ?1 AND ord > ?2",
        params![notetype_id, ord],
    )
    .map_err(err)?;

    bump_schema_mod(tx, now_ms)?;
    Ok(())
}

pub fn save_template(
    conn: &Connection,
    notetype_id: i64,
    ord: i64,
    name: &str,
    qfmt: &str,
    afmt: &str,
    now_ms: i64,
) -> CoreResult<()> {
    let affected = conn
        .execute(
            "UPDATE templates SET name = ?1, qfmt = ?2, afmt = ?3 \
             WHERE notetype_id = ?4 AND ord = ?5",
            params![name, qfmt, afmt, notetype_id, ord],
        )
        .map_err(err)?;
    if affected == 0 {
        return Err(CoreError::NotFound(format!(
            "template {ord} in notetype {notetype_id}"
        )));
    }
    conn.execute(
        r#"UPDATE notetypes SET "mod" = ?1, usn = -1 WHERE id = ?2"#,
        params![now_ms, notetype_id],
    )
    .map_err(err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::storage::SqliteStorage;
    use synapse_core::ports::Storage;

    /// Import creates a "Basic" notetype + 1 note + 1 card; IDs are
    /// auto-assigned by the DB (import remaps source IDs), so we look them up.
    struct Setup {
        s: SqliteStorage,
        nt_id: i64,
        note_id: i64,
    }

    fn setup() -> Setup {
        let s = SqliteStorage::open_in_memory().unwrap();
        s.ensure_collection(1_700_000_000_000).unwrap();
        s.import(&basic_model()).unwrap();
        let nt_id: i64 = s
            .lock()
            .query_row("SELECT id FROM notetypes WHERE name = 'Basic'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let note_id: i64 = s
            .lock()
            .query_row(
                "SELECT id FROM notes WHERE notetype_id = ?1",
                [nt_id],
                |r| r.get(0),
            )
            .unwrap();
        Setup { s, nt_id, note_id }
    }

    fn basic_model() -> synapse_core::model::CanonicalModel {
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
                afmt: "{{FrontSide}}<hr>{{Back}}".into(),
                config_json: "{}".into(),
            }],
            notes: vec![Note {
                id: 100,
                guid: "g1".into(),
                notetype_id: 10,
                mod_ms: 0,
                usn: -1,
                tags: vec![],
                fields: vec!["hello".into(), "world".into()],
                sort_field: Some("hello".into()),
                checksum: None,
            }],
            cards: vec![Card {
                id: 1001,
                note_id: 100,
                deck_id: 1,
                ord: 0,
                mod_ms: 0,
                usn: -1,
                ctype: 0,
                queue: 0,
                due: 1,
                interval: 0,
                ease_factor: 0,
                reps: 0,
                lapses: 0,
                remaining: 0,
                original_due: 0,
                original_deck_id: 0,
                flags: 0,
                fsrs_stability: None,
                fsrs_difficulty: None,
                fsrs_last_review: None,
                data: None,
            }],
            ..Default::default()
        }
    }

    fn card_count(s: &SqliteStorage) -> i64 {
        s.lock()
            .query_row("SELECT COUNT(*) FROM cards", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn get_detail_returns_fields_and_templates() {
        let Setup { s, nt_id, .. } = setup();
        let detail = s.get_notetype_detail(nt_id).unwrap().unwrap();
        assert_eq!(detail.name, "Basic");
        assert_eq!(detail.kind, 0);
        assert_eq!(detail.fields.len(), 2);
        assert_eq!(detail.fields[0].name, "Front");
        assert_eq!(detail.fields[1].name, "Back");
        assert_eq!(detail.templates.len(), 1);
        assert_eq!(detail.templates[0].name, "Card 1");
    }

    #[test]
    fn create_standard_notetype_seeds_fields_and_template() {
        let Setup { s, .. } = setup();
        let id = s.create_notetype("New Basic", 0, 1_000).unwrap();
        let detail = s.get_notetype_detail(id).unwrap().unwrap();
        assert_eq!(detail.name, "New Basic");
        assert_eq!(detail.fields.len(), 2);
        assert_eq!(detail.templates.len(), 1);
    }

    #[test]
    fn create_cloze_notetype_seeds_text_and_extra() {
        let Setup { s, .. } = setup();
        let id = s.create_notetype("New Cloze", 1, 1_000).unwrap();
        let detail = s.get_notetype_detail(id).unwrap().unwrap();
        assert_eq!(detail.kind, 1);
        assert_eq!(detail.fields[0].name, "Text");
        assert_eq!(detail.fields[1].name, "Extra");
    }

    #[test]
    fn delete_notetype_rejects_when_notes_exist() {
        let Setup { s, nt_id, .. } = setup();
        assert!(s.delete_notetype(nt_id, 1_000).is_err());
    }

    #[test]
    fn delete_empty_notetype_succeeds() {
        let Setup { s, .. } = setup();
        let id = s.create_notetype("Empty", 0, 1_000).unwrap();
        s.delete_notetype(id, 2_000).unwrap();
        assert!(s.get_notetype_detail(id).unwrap().is_none());
    }

    #[test]
    fn rename_notetype() {
        let Setup { s, nt_id, .. } = setup();
        s.rename_notetype(nt_id, "Renamed", 1_000).unwrap();
        let detail = s.get_notetype_detail(nt_id).unwrap().unwrap();
        assert_eq!(detail.name, "Renamed");
    }

    #[test]
    fn add_field_appends_to_notes() {
        let Setup { s, nt_id, note_id } = setup();
        s.add_field(nt_id, "Extra", 1_000).unwrap();
        let detail = s.get_notetype_detail(nt_id).unwrap().unwrap();
        assert_eq!(detail.fields.len(), 3);
        assert_eq!(detail.fields[2].name, "Extra");

        let note = s.note_detail(note_id).unwrap().unwrap();
        assert_eq!(note.fields.len(), 3);
        assert_eq!(note.fields[2].value, "");
    }

    #[test]
    fn check_field_remove_counts_non_empty() {
        let Setup { s, nt_id, .. } = setup();
        let warn = s.check_field_remove(nt_id, 0).unwrap();
        assert_eq!(warn.notes_with_content, 1); // "hello" in Front

        let warn2 = s.check_field_remove(nt_id, 1).unwrap();
        assert_eq!(warn2.notes_with_content, 1); // "world" in Back
    }

    #[test]
    fn remove_field_splices_note_blob() {
        let Setup { s, nt_id, note_id } = setup();
        s.remove_field(nt_id, 0, 1_000).unwrap(); // remove Front
        let detail = s.get_notetype_detail(nt_id).unwrap().unwrap();
        assert_eq!(detail.fields.len(), 1);
        assert_eq!(detail.fields[0].name, "Back");
        assert_eq!(detail.fields[0].ord, 0); // shifted

        let note = s.note_detail(note_id).unwrap().unwrap();
        assert_eq!(note.fields.len(), 1);
        assert_eq!(note.fields[0].value, "world");
    }

    #[test]
    fn remove_last_field_is_rejected() {
        let Setup { s, nt_id, .. } = setup();
        s.remove_field(nt_id, 0, 1_000).unwrap(); // now 1 field
        assert!(s.remove_field(nt_id, 0, 2_000).is_err());
    }

    #[test]
    fn rename_field_updates_name() {
        let Setup { s, nt_id, .. } = setup();
        s.rename_field(nt_id, 0, "Question", 1_000).unwrap();
        let detail = s.get_notetype_detail(nt_id).unwrap().unwrap();
        assert_eq!(detail.fields[0].name, "Question");
    }

    #[test]
    fn reorder_fields_permutes_note_blob() {
        let Setup { s, nt_id, note_id } = setup();
        // Swap Front (ord=0) and Back (ord=1): new_order = [1, 0]
        s.reorder_fields(nt_id, &[1, 0], 1_000).unwrap();
        let detail = s.get_notetype_detail(nt_id).unwrap().unwrap();
        assert_eq!(detail.fields[0].name, "Back");
        assert_eq!(detail.fields[1].name, "Front");

        let note = s.note_detail(note_id).unwrap().unwrap();
        assert_eq!(note.fields[0].value, "world"); // was Back
        assert_eq!(note.fields[1].value, "hello"); // was Front
    }

    #[test]
    fn add_template_generates_new_cards() {
        let Setup { s, nt_id, .. } = setup();
        let before = card_count(&s);
        s.add_template(nt_id, "Card 2", "{{Back}}", "{{Front}}", 1_000)
            .unwrap();
        assert_eq!(card_count(&s), before + 1);

        let detail = s.get_notetype_detail(nt_id).unwrap().unwrap();
        assert_eq!(detail.templates.len(), 2);
    }

    #[test]
    fn remove_template_deletes_cards_and_shifts_ords() {
        let Setup { s, nt_id, .. } = setup();
        s.add_template(nt_id, "Card 2", "{{Back}}", "{{Front}}", 1_000)
            .unwrap();
        assert_eq!(card_count(&s), 2);

        s.remove_template(nt_id, 0, 2_000).unwrap(); // remove Card 1
        let detail = s.get_notetype_detail(nt_id).unwrap().unwrap();
        assert_eq!(detail.templates.len(), 1);
        assert_eq!(detail.templates[0].ord, 0); // shifted from 1 to 0

        assert_eq!(card_count(&s), 1);
    }

    #[test]
    fn remove_last_template_is_rejected() {
        let Setup { s, nt_id, .. } = setup();
        assert!(s.remove_template(nt_id, 0, 1_000).is_err());
    }

    #[test]
    fn save_template_updates_formats() {
        let Setup { s, nt_id, .. } = setup();
        s.save_template(nt_id, 0, "Updated", "{{Back}}", "{{Front}}", 1_000)
            .unwrap();
        let detail = s.get_notetype_detail(nt_id).unwrap().unwrap();
        assert_eq!(detail.templates[0].name, "Updated");
        assert_eq!(detail.templates[0].qfmt, "{{Back}}");
        assert_eq!(detail.templates[0].afmt, "{{Front}}");
    }
}
