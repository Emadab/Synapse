//! Study-queue queries and answer persistence. Free functions over a
//! `Connection`/`Transaction`, called by `SqliteStorage`.

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::model::{CardRender, Revlog, StudyCard};
use synapse_core::scheduling::{CardPhase, CardState};

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Storage(e.to_string())
}

fn type_to_phase(card_type: i64) -> CardPhase {
    match card_type {
        1 => CardPhase::Learning,
        2 => CardPhase::Review,
        3 => CardPhase::Relearning,
        _ => CardPhase::New,
    }
}

/// (card.type, card.queue) for a phase.
pub fn phase_to_type_queue(phase: CardPhase) -> (i64, i64) {
    match phase {
        CardPhase::New => (0, 0),
        CardPhase::Learning => (1, 1),
        CardPhase::Review => (2, 2),
        CardPhase::Relearning => (3, 1),
    }
}

pub fn ensure_collection(conn: &Connection, now_ms: i64) -> CoreResult<i64> {
    conn.execute(
        "INSERT OR IGNORE INTO collection
         (id, created, modified, schema_mod, anki_ver, usn, last_sync, config)
         VALUES (1, ?1, ?1, ?1, 18, 0, 0, '{}')",
        [now_ms],
    )
    .map_err(err)?;
    conn.query_row("SELECT created FROM collection WHERE id = 1", [], |r| {
        r.get(0)
    })
    .map_err(err)
}

pub fn due_card_ids(conn: &Connection, deck_id: i64, today: i32) -> CoreResult<Vec<i64>> {
    let mut stmt = conn
        .prepare(
            "SELECT id FROM cards
             WHERE deck_id = ?1 AND queue NOT IN (-1, -2, -3)
               AND (type = 0 OR type = 1 OR type = 3 OR (type = 2 AND due <= ?2))
             ORDER BY (CASE type WHEN 0 THEN 0 WHEN 2 THEN 2 ELSE 1 END), due
             LIMIT 500",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map(params![deck_id, today], |r| r.get(0))
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

pub fn study_card(conn: &Connection, card_id: i64) -> CoreResult<Option<StudyCard>> {
    let row = conn
        .query_row(
            "SELECT c.note_id, c.deck_id, c.ord, c.type, c.interval, c.ease_factor, c.reps,
                    c.lapses, c.remaining, c.fsrs_stability, c.fsrs_difficulty,
                    c.fsrs_last_review, n.fields, n.notetype_id
             FROM cards c JOIN notes n ON n.id = c.note_id
             WHERE c.id = ?1",
            [card_id],
            |r| {
                Ok(CardRow {
                    deck_id: r.get(1)?,
                    ord: r.get(2)?,
                    card_type: r.get(3)?,
                    interval: r.get(4)?,
                    ease_factor: r.get(5)?,
                    reps: r.get(6)?,
                    lapses: r.get(7)?,
                    remaining: r.get(8)?,
                    stability: r.get(9)?,
                    difficulty: r.get(10)?,
                    last_review: r.get(11)?,
                    fields: r.get(12)?,
                    notetype_id: r.get(13)?,
                })
            },
        )
        .optional()
        .map_err(err)?;
    let Some(row) = row else { return Ok(None) };

    // Field names (ordered) zipped with the note's field values.
    let mut stmt = conn
        .prepare("SELECT name FROM fields WHERE notetype_id = ?1 ORDER BY ord")
        .map_err(err)?;
    let names: Vec<String> = stmt
        .query_map([row.notetype_id], |r| r.get(0))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    let fields = names
        .into_iter()
        .zip(row.fields.split('\u{1f}').map(str::to_string))
        .collect::<Vec<(String, String)>>();

    // Template for this card's ord, falling back to ord 0 (cloze note types
    // have a single template shared by every cloze card).
    let template = conn
        .query_row(
            "SELECT qfmt, afmt FROM templates WHERE notetype_id = ?1 AND ord = ?2",
            params![row.notetype_id, row.ord],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(err)?;
    let (qfmt, afmt) = match template {
        Some(t) => t,
        None => conn
            .query_row(
                "SELECT qfmt, afmt FROM templates WHERE notetype_id = ?1 AND ord = 0",
                [row.notetype_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(err)?
            .unwrap_or_default(),
    };

    let is_cloze: i64 = conn
        .query_row(
            "SELECT kind FROM notetypes WHERE id = ?1",
            [row.notetype_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?
        .unwrap_or(0);

    Ok(Some(StudyCard {
        id: card_id,
        deck_id: row.deck_id,
        render: CardRender {
            fields,
            qfmt,
            afmt,
            is_cloze: is_cloze == 1,
            card_ord: row.ord.max(0) as u16,
        },
        state: CardState {
            phase: type_to_phase(row.card_type),
            steps_remaining: row.remaining.max(0) as u32,
            interval_days: row.interval.max(0) as u32,
            ease_milli: if row.ease_factor > 0 {
                row.ease_factor as u32
            } else {
                2500
            },
            reps: row.reps.max(0) as u32,
            lapses: row.lapses.max(0) as u32,
            stability: row.stability,
            difficulty: row.difficulty,
            last_review_day: row.last_review.map(|d| d as i32),
        },
    }))
}

pub fn apply_answer(
    tx: &Transaction<'_>,
    card_id: i64,
    next: &CardState,
    due: i64,
    log: &Revlog,
) -> CoreResult<()> {
    let (card_type, queue) = phase_to_type_queue(next.phase);
    tx.execute(
        r#"UPDATE cards SET
             type = ?2, queue = ?3, due = ?4, interval = ?5, ease_factor = ?6,
             reps = ?7, lapses = ?8, remaining = ?9,
             fsrs_stability = ?10, fsrs_difficulty = ?11, fsrs_last_review = ?12,
             "mod" = ?13, usn = -1
           WHERE id = ?1"#,
        params![
            card_id,
            card_type,
            queue,
            due,
            next.interval_days as i64,
            next.ease_milli as i64,
            next.reps as i64,
            next.lapses as i64,
            next.steps_remaining as i64,
            next.stability,
            next.difficulty,
            next.last_review_day.map(i64::from),
            log.id,
        ],
    )
    .map_err(err)?;
    tx.execute(
        "INSERT INTO revlog
         (id, card_id, usn, ease, interval, last_interval, ease_factor, taken_ms, review_kind)
         VALUES (?1, ?2, -1, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            log.id,
            card_id,
            log.ease,
            log.interval,
            log.last_interval,
            log.ease_factor,
            log.taken_ms,
            log.review_kind,
        ],
    )
    .map_err(err)?;
    Ok(())
}

struct CardRow {
    deck_id: i64,
    ord: i64,
    card_type: i64,
    interval: i64,
    ease_factor: i64,
    reps: i64,
    lapses: i64,
    remaining: i64,
    stability: Option<f64>,
    difficulty: Option<f64>,
    last_review: Option<i64>,
    fields: String,
    notetype_id: i64,
}

#[cfg(test)]
mod tests {
    use crate::storage::SqliteStorage;
    use synapse_core::model::{
        CanonicalModel, Card, CardRender, Deck, Field, Note, Notetype, Revlog, Template,
    };
    use synapse_core::ports::Storage;
    use synapse_core::scheduling::{CardPhase, CardState};

    fn model() -> CanonicalModel {
        CanonicalModel {
            decks: vec![Deck {
                id: 1,
                name: "Default".into(),
                parent_id: None,
                config_id: 1,
                mod_ms: 0,
                usn: -1,
                collapsed: false,
                is_filtered: false,
            }],
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
                tags: vec![],
                fields: vec!["hola".into(), "hello".into()],
                sort_field: Some("hola".into()),
                checksum: None,
            }],
            cards: vec![Card {
                id: 1000,
                note_id: 100,
                deck_id: 1,
                ord: 0,
                mod_ms: 0,
                usn: -1,
                ctype: 0,
                queue: 0,
                due: 0,
                interval: 0,
                ease_factor: 2500,
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

    #[test]
    fn queue_render_and_answer_roundtrip() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        storage.ensure_collection(1_700_000_000_000).unwrap();
        storage.import(&model()).unwrap();

        // The new card is queued for the Default deck (id 1).
        let due = storage.due_card_ids(1, 0).unwrap();
        assert_eq!(due.len(), 1);
        let card_id = due[0];

        let study = storage.study_card(card_id).unwrap().unwrap();
        assert_eq!(study.state.phase, CardPhase::New);
        assert_eq!(
            study.render,
            CardRender {
                fields: vec![
                    ("Front".into(), "hola".into()),
                    ("Back".into(), "hello".into())
                ],
                qfmt: "{{Front}}".into(),
                afmt: "{{Back}}".into(),
                is_cloze: false,
                card_ord: 0,
            }
        );

        // Answer Good → becomes a Review card due tomorrow; no longer due today.
        let next = CardState {
            phase: CardPhase::Review,
            interval_days: 1,
            reps: 1,
            last_review_day: Some(0),
            ..study.state
        };
        let log = Revlog {
            id: 1_700_000_001_000,
            card_id,
            usn: -1,
            ease: 3,
            interval: 1,
            last_interval: 0,
            ease_factor: 2500,
            taken_ms: 1500,
            review_kind: 0,
        };
        storage.apply_answer(card_id, &next, 1, &log).unwrap();

        assert!(storage.due_card_ids(1, 0).unwrap().is_empty());
        assert_eq!(
            storage.study_card(card_id).unwrap().unwrap().state.phase,
            CardPhase::Review
        );
    }
}
