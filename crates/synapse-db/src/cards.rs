//! Card lifecycle operations: suspend/bury/flag, sibling bury, leech tagging.

use rusqlite::{params, Connection};
use synapse_core::error::{CoreError, CoreResult};

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Storage(e.to_string())
}

/// Execute a `queue` update on a JSON list of card ids.
fn set_queue_for_ids(conn: &Connection, card_ids: &[i64], queue: i64) -> CoreResult<()> {
    if card_ids.is_empty() {
        return Ok(());
    }
    let json = serde_json::to_string(card_ids).map_err(err)?;
    conn.execute(
        "UPDATE cards SET queue = ?1 WHERE id IN (SELECT value FROM json_each(?2))",
        params![queue, json],
    )
    .map_err(err)?;
    Ok(())
}

pub fn suspend_cards(conn: &Connection, card_ids: &[i64]) -> CoreResult<()> {
    set_queue_for_ids(conn, card_ids, -1)
}

pub fn unsuspend_cards(conn: &Connection, card_ids: &[i64]) -> CoreResult<()> {
    if card_ids.is_empty() {
        return Ok(());
    }
    let json = serde_json::to_string(card_ids).map_err(err)?;
    // Restore each card to its natural queue (type column mirrors queue for non-buried states).
    conn.execute(
        "UPDATE cards SET queue = type WHERE id IN (SELECT value FROM json_each(?1)) AND queue = -1",
        [json],
    )
    .map_err(err)?;
    Ok(())
}

/// Manually bury cards (queue = -2).
pub fn bury_cards(conn: &Connection, card_ids: &[i64]) -> CoreResult<()> {
    set_queue_for_ids(conn, card_ids, -2)
}

/// Sibling bury: when a note's card is answered, bury all other new/review
/// cards of that note so they won't appear in the same session (queue = -3).
pub fn bury_siblings(conn: &Connection, note_id: i64, answered_card_id: i64) -> CoreResult<()> {
    conn.execute(
        "UPDATE cards SET queue = -3
         WHERE note_id = ?1 AND id != ?2 AND queue IN (0, 2)",
        params![note_id, answered_card_id],
    )
    .map_err(err)?;
    Ok(())
}

/// Unbury all buried cards in `deck_id` (both manual -2 and sibling -3).
/// Called at the start of each study session so the day cutoff is respected.
pub fn unbury_deck(conn: &Connection, deck_id: i64) -> CoreResult<()> {
    conn.execute(
        "UPDATE cards SET queue = type WHERE deck_id = ?1 AND queue IN (-2, -3)",
        [deck_id],
    )
    .map_err(err)?;
    Ok(())
}

/// Set the flag (0–7) on a list of cards.
pub fn set_card_flag(conn: &Connection, card_ids: &[i64], flag: u8) -> CoreResult<()> {
    if card_ids.is_empty() {
        return Ok(());
    }
    let json = serde_json::to_string(card_ids).map_err(err)?;
    conn.execute(
        "UPDATE cards SET flags = ?1 WHERE id IN (SELECT value FROM json_each(?2))",
        params![flag as i64, json],
    )
    .map_err(err)?;
    Ok(())
}

/// Ensure the `leech` tag is present on a note (idempotent).
pub fn add_note_tag(conn: &Connection, note_id: i64, tag: &str, now_ms: i64) -> CoreResult<()> {
    let current: String = conn
        .query_row("SELECT tags FROM notes WHERE id = ?1", [note_id], |r| {
            r.get(0)
        })
        .map_err(err)?;
    // Tags blob: ` tag1 tag2 ` (space-padded). Check inclusion before writing.
    let needle = format!(" {tag} ");
    if current.contains(&needle) {
        return Ok(());
    }
    let new_tags = if current.trim().is_empty() {
        format!(" {tag} ")
    } else {
        format!("{} ", current.trim_end()) + &format!("{tag} ")
    };
    conn.execute(
        r#"UPDATE notes SET tags = ?2, "mod" = ?3, usn = -1 WHERE id = ?1"#,
        params![note_id, new_tags, now_ms],
    )
    .map_err(err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::storage::SqliteStorage;
    use synapse_core::ports::Storage;

    fn setup() -> (SqliteStorage, i64, i64) {
        use synapse_core::model::*;
        let s = SqliteStorage::open_in_memory().unwrap();
        s.ensure_collection(1_700_000_000_000).unwrap();
        s.import(&CanonicalModel {
            notetypes: vec![Notetype {
                id: 1,
                name: "Basic".into(),
                kind: 0,
                mod_ms: 0,
                usn: -1,
                config_json: "{}".into(),
            }],
            fields: vec![
                Field {
                    notetype_id: 1,
                    ord: 0,
                    name: "Front".into(),
                    config_json: "{}".into(),
                },
                Field {
                    notetype_id: 1,
                    ord: 1,
                    name: "Back".into(),
                    config_json: "{}".into(),
                },
            ],
            templates: vec![Template {
                notetype_id: 1,
                ord: 0,
                name: "Card 1".into(),
                qfmt: "{{Front}}".into(),
                afmt: "{{Back}}".into(),
                config_json: "{}".into(),
            }],
            notes: vec![Note {
                id: 1,
                guid: "g1".into(),
                notetype_id: 1,
                mod_ms: 0,
                usn: -1,
                tags: vec![],
                fields: vec!["A".into(), "B".into()],
                sort_field: None,
                checksum: None,
            }],
            cards: vec![Card {
                id: 1,
                note_id: 1,
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
        })
        .unwrap();
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
        let card_id: i64 = s
            .lock()
            .query_row("SELECT id FROM cards WHERE note_id = ?1", [note_id], |r| {
                r.get(0)
            })
            .unwrap();
        (s, note_id, card_id)
    }

    #[test]
    fn suspend_and_unsuspend() {
        let (s, _, card_id) = setup();
        let q_before: i64 = s
            .lock()
            .query_row("SELECT queue FROM cards WHERE id = ?1", [card_id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(q_before, 0);

        s.suspend_cards(&[card_id]).unwrap();
        let q: i64 = s
            .lock()
            .query_row("SELECT queue FROM cards WHERE id = ?1", [card_id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(q, -1);

        // Suspended card is excluded from study queue
        let queue = s
            .study_queue(1, 0, 1_700_000_000_000, 1_700_086_400_000, 20, 200)
            .unwrap();
        assert!(queue.new.is_empty());

        s.unsuspend_cards(&[card_id]).unwrap();
        let q: i64 = s
            .lock()
            .query_row("SELECT queue FROM cards WHERE id = ?1", [card_id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(q, 0); // restored to type

        let queue = s
            .study_queue(1, 0, 1_700_000_000_000, 1_700_086_400_000, 20, 200)
            .unwrap();
        assert_eq!(queue.new.len(), 1);
    }

    #[test]
    fn bury_and_unbury() {
        let (s, _, card_id) = setup();
        s.bury_cards(&[card_id]).unwrap();
        let queue = s
            .study_queue(1, 0, 1_700_000_000_000, 1_700_086_400_000, 20, 200)
            .unwrap();
        assert!(queue.new.is_empty());

        s.unbury_deck(1).unwrap();
        let queue = s
            .study_queue(1, 0, 1_700_000_000_000, 1_700_086_400_000, 20, 200)
            .unwrap();
        assert_eq!(queue.new.len(), 1);
    }

    #[test]
    fn set_flag_persists() {
        let (s, _, card_id) = setup();
        s.set_card_flag(&[card_id], 3).unwrap();
        let flag: i64 = s
            .lock()
            .query_row("SELECT flags FROM cards WHERE id = ?1", [card_id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(flag, 3);
    }

    #[test]
    fn add_note_tag_idempotent() {
        let (s, note_id, _) = setup();
        s.add_note_tag(note_id, "leech", 1_000).unwrap();
        s.add_note_tag(note_id, "leech", 2_000).unwrap(); // idempotent
        let tags: String = s
            .lock()
            .query_row("SELECT tags FROM notes WHERE id = ?1", [note_id], |r| {
                r.get(0)
            })
            .unwrap();
        // Should appear exactly once
        assert_eq!(tags.matches("leech").count(), 1);
    }
}
