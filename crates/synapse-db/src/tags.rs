use rusqlite::{params, Connection};
use synapse_core::error::{CoreError, CoreResult};

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Storage(e.to_string())
}

/// All distinct tag names from the `tags` registry, sorted.
pub fn list_tags(conn: &Connection) -> CoreResult<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT name FROM tags ORDER BY name")
        .map_err(err)?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)?;
    Ok(names)
}

/// Rename `old_tag` to `new_tag` in every note's tag blob and update the
/// registry. Returns the number of notes affected.
pub fn rename_tag(conn: &Connection, old_tag: &str, new_tag: &str, now_ms: i64) -> CoreResult<u32> {
    let old_needle = format!(" {old_tag} ");
    let new_needle = format!(" {new_tag} ");
    let n = conn
        .execute(
            r#"UPDATE notes SET tags = replace(tags, ?1, ?2), "mod" = ?3
               WHERE instr(tags, ?1) > 0"#,
            params![old_needle, new_needle, now_ms],
        )
        .map_err(err)?;
    conn.execute("DELETE FROM tags WHERE name = ?1", [old_tag])
        .map_err(err)?;
    conn.execute(
        "INSERT OR IGNORE INTO tags (name, usn, expanded) VALUES (?1, -1, 0)",
        [new_tag],
    )
    .map_err(err)?;
    Ok(n as u32)
}

/// Remove `tag` from every note's tag blob and from the registry.
/// Returns the number of notes affected.
pub fn delete_tag(conn: &Connection, tag: &str, now_ms: i64) -> CoreResult<u32> {
    let needle = format!(" {tag} ");
    let n = conn
        .execute(
            r#"UPDATE notes SET tags = replace(tags, ?1, ' '), "mod" = ?2
               WHERE instr(tags, ?1) > 0"#,
            params![needle, now_ms],
        )
        .map_err(err)?;
    conn.execute("DELETE FROM tags WHERE name = ?1", [tag])
        .map_err(err)?;
    Ok(n as u32)
}

/// Rename each `sources` tag to `target` (and dedup the `target` if already
/// present). Idempotent if `sources` contains `target`.
pub fn merge_tags(
    conn: &Connection,
    sources: &[String],
    target: &str,
    now_ms: i64,
) -> CoreResult<()> {
    for src in sources {
        if src != target {
            rename_tag(conn, src, target, now_ms)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SqliteStorage;
    use synapse_core::ports::Storage;

    fn setup() -> SqliteStorage {
        let s = SqliteStorage::open_in_memory().unwrap();
        s.ensure_collection(1_700_000_000_000).unwrap();
        s
    }

    fn add_note_with_tags(s: &SqliteStorage, note_id: i64, tags_blob: &str) {
        let conn = s.lock();
        conn.execute(
            r#"INSERT INTO notetypes (id, name, kind, "mod", usn) VALUES (1, 'Basic', 0, 0, -1)
               ON CONFLICT DO NOTHING"#,
            [],
        )
        .unwrap();
        conn.execute(
            r#"INSERT OR REPLACE INTO notes (id, guid, notetype_id, "mod", usn, tags, fields)
               VALUES (?1, 'guid1', 1, 0, -1, ?2, 'A\x1fB')"#,
            params![note_id, tags_blob],
        )
        .unwrap();
        for tag in tags_blob.split_whitespace() {
            conn.execute(
                "INSERT OR IGNORE INTO tags (name, usn, expanded) VALUES (?1, -1, 0)",
                [tag],
            )
            .unwrap();
        }
    }

    #[test]
    fn list_tags_returns_registry() {
        let s = setup();
        {
            let conn = s.lock();
            conn.execute(
                "INSERT INTO tags (name, usn, expanded) VALUES ('verb', -1, 0), ('noun', -1, 0)",
                [],
            )
            .unwrap();
            let tags = list_tags(&conn).unwrap();
            assert_eq!(tags, vec!["noun".to_string(), "verb".to_string()]);
        }
    }

    #[test]
    fn rename_tag_updates_notes_and_registry() {
        let s = setup();
        add_note_with_tags(&s, 1, " verb noun ");
        {
            let conn = s.lock();
            let n = rename_tag(&conn, "verb", "v2", 999).unwrap();
            assert_eq!(n, 1);
            let row: String = conn
                .query_row("SELECT tags FROM notes", [], |r| r.get(0))
                .unwrap();
            assert!(row.contains(" v2 "), "tag renamed in notes: {row}");
            let tags = list_tags(&conn).unwrap();
            assert!(tags.contains(&"v2".to_string()));
            assert!(!tags.contains(&"verb".to_string()));
        }
    }

    #[test]
    fn delete_tag_removes_from_notes_and_registry() {
        let s = setup();
        add_note_with_tags(&s, 1, " verb noun ");
        {
            let conn = s.lock();
            let n = delete_tag(&conn, "verb", 999).unwrap();
            assert_eq!(n, 1);
            let row: String = conn
                .query_row("SELECT tags FROM notes", [], |r| r.get(0))
                .unwrap();
            assert!(!row.contains(" verb "), "tag removed from notes: {row}");
            let tags = list_tags(&conn).unwrap();
            assert!(!tags.contains(&"verb".to_string()));
        }
    }

    #[test]
    fn merge_tags_renames_sources_to_target() {
        let s = setup();
        add_note_with_tags(&s, 1, " v1 noun ");
        add_note_with_tags(&s, 2, " v2 noun ");
        {
            let conn = s.lock();
            merge_tags(&conn, &["v1".to_string(), "v2".to_string()], "verb", 999).unwrap();
            let n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM notes WHERE instr(tags, ' verb ') > 0",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 2, "both notes should have verb tag");
        }
    }
}
