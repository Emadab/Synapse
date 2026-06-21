//! Study-queue queries and answer persistence. Free functions over a
//! `Connection`/`Transaction`, called by `SqliteStorage`.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::model::{Algorithm, CardRender, Revlog, StudyCard};
use synapse_core::scheduling::{CardPhase, CardState, SchedConfig, FSRS5_DEFAULT_WEIGHTS};

fn parse_limits(config_json: &str) -> (u32, u32) {
    let v: serde_json::Value = serde_json::from_str(config_json).unwrap_or_default();
    let new_per_day = v["new"]["perDay"].as_u64().unwrap_or(20) as u32;
    let rev_per_day = v["rev"]["perDay"].as_u64().unwrap_or(200) as u32;
    (new_per_day, rev_per_day)
}

pub fn deck_limits(conn: &Connection, config_id: i64) -> CoreResult<(u32, u32)> {
    let json: String = conn
        .query_row(
            "SELECT config FROM deck_config WHERE id = ?1",
            [config_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?
        .unwrap_or_else(|| "{}".to_string());
    Ok(parse_limits(&json))
}

pub fn all_deck_limits(conn: &Connection) -> CoreResult<HashMap<i64, (u32, u32)>> {
    let mut stmt = conn
        .prepare("SELECT id, config FROM deck_config")
        .map_err(err)?;
    let mut map = HashMap::new();
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map_err(err)?;
    for row in rows {
        let (id, json) = row.map_err(err)?;
        map.insert(id, parse_limits(&json));
    }
    Ok(map)
}

pub fn today_studied(
    conn: &Connection,
    deck_id: i64,
    today_start_ms: i64,
) -> CoreResult<(u32, u32)> {
    conn.query_row(
        "SELECT
           COUNT(DISTINCT CASE WHEN r.review_kind = 0 THEN r.card_id END),
           COUNT(DISTINCT CASE WHEN r.review_kind = 1 THEN r.card_id END)
         FROM revlog r JOIN cards c ON c.id = r.card_id
         WHERE c.deck_id = ?1 AND r.id >= ?2",
        params![deck_id, today_start_ms],
        |r| Ok((r.get::<_, u32>(0)?, r.get::<_, u32>(1)?)),
    )
    .map_err(err)
}

pub fn all_today_studied(
    conn: &Connection,
    today_start_ms: i64,
) -> CoreResult<HashMap<i64, (u32, u32)>> {
    let mut stmt = conn
        .prepare(
            "SELECT c.deck_id,
               COUNT(DISTINCT CASE WHEN r.review_kind = 0 THEN r.card_id END),
               COUNT(DISTINCT CASE WHEN r.review_kind = 1 THEN r.card_id END)
             FROM revlog r JOIN cards c ON c.id = r.card_id
             WHERE r.id >= ?1
             GROUP BY c.deck_id",
        )
        .map_err(err)?;
    let mut map = HashMap::new();
    let rows = stmt
        .query_map([today_start_ms], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, u32>(1)?, r.get::<_, u32>(2)?))
        })
        .map_err(err)?;
    for row in rows {
        let (deck_id, new_studied, rev_studied) = row.map_err(err)?;
        map.insert(deck_id, (new_studied, rev_studied));
    }
    Ok(map)
}

pub fn set_deck_limits(
    conn: &Connection,
    config_id: i64,
    new_per_day: u32,
    rev_per_day: u32,
    now_ms: i64,
) -> CoreResult<()> {
    let json: String = conn
        .query_row(
            "SELECT config FROM deck_config WHERE id = ?1",
            [config_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?
        .unwrap_or_else(|| "{}".to_string());
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
    v["new"]["perDay"] = serde_json::Value::Number(new_per_day.into());
    v["rev"]["perDay"] = serde_json::Value::Number(rev_per_day.into());
    let new_json = serde_json::to_string(&v).map_err(err)?;
    conn.execute(
        r#"UPDATE deck_config SET config = ?1, "mod" = ?2 WHERE id = ?3"#,
        params![new_json, now_ms, config_id],
    )
    .map_err(err)?;
    Ok(())
}

pub fn get_deck_config(conn: &Connection, config_id: i64) -> CoreResult<SchedConfig> {
    let json: String = conn
        .query_row(
            "SELECT config FROM deck_config WHERE id = ?1",
            [config_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?
        .unwrap_or_else(|| "{}".to_string());
    Ok(parse_sched_config(&json))
}

pub fn set_deck_config(
    conn: &Connection,
    config_id: i64,
    cfg: &SchedConfig,
    now_ms: i64,
) -> CoreResult<()> {
    let algo_str = match cfg.algorithm {
        Algorithm::Sm2 => "sm2",
        Algorithm::Fsrs => "fsrs",
    };
    let v = serde_json::json!({
        "algo": algo_str,
        "new": {
            "perDay": cfg.new_per_day,
            "delays": cfg.learning_steps_min,
            "graduating": cfg.graduating_interval_days,
            "easy": cfg.easy_interval_days,
            "initialFactor": cfg.starting_ease_milli
        },
        "rev": {
            "perDay": cfg.review_per_day,
            "ease4": cfg.easy_bonus,
            "hardFactor": cfg.hard_interval_factor,
            "ivlFct": cfg.interval_modifier,
            "maxIvl": cfg.maximum_interval_days
        },
        "lapse": {
            "delays": cfg.relearning_steps_min,
            "minInt": cfg.minimum_interval_days,
            "mult": cfg.lapse_interval_factor,
            "leechThreshold": cfg.leech_threshold
        },
        "fsrs": {
            "weights": cfg.fsrs_weights.to_vec(),
            "desiredRetention": cfg.desired_retention
        }
    });
    conn.execute(
        r#"UPDATE deck_config SET config = ?1, "mod" = ?2 WHERE id = ?3"#,
        params![v.to_string(), now_ms, config_id],
    )
    .map_err(err)?;
    Ok(())
}

fn parse_sched_config(json: &str) -> SchedConfig {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let algorithm = match v["algo"].as_str().unwrap_or("sm2") {
        "fsrs" => Algorithm::Fsrs,
        _ => Algorithm::Sm2,
    };
    let fsrs_weights = {
        let mut arr = FSRS5_DEFAULT_WEIGHTS;
        if let Some(weights) = v["fsrs"]["weights"].as_array() {
            for (i, w) in weights.iter().enumerate().take(19) {
                if let Some(f) = w.as_f64() {
                    arr[i] = f;
                }
            }
        }
        arr
    };
    SchedConfig {
        algorithm,
        new_per_day: v["new"]["perDay"].as_u64().unwrap_or(20) as u32,
        review_per_day: v["rev"]["perDay"].as_u64().unwrap_or(200) as u32,
        learning_steps_min: v["new"]["delays"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u32)).collect())
            .unwrap_or_else(|| vec![1, 10]),
        graduating_interval_days: v["new"]["graduating"].as_u64().unwrap_or(1) as u32,
        easy_interval_days: v["new"]["easy"].as_u64().unwrap_or(4) as u32,
        starting_ease_milli: v["new"]["initialFactor"].as_u64().unwrap_or(2500) as u32,
        easy_bonus: v["rev"]["ease4"].as_f64().unwrap_or(1.3),
        hard_interval_factor: v["rev"]["hardFactor"].as_f64().unwrap_or(1.2),
        interval_modifier: v["rev"]["ivlFct"].as_f64().unwrap_or(1.0),
        maximum_interval_days: v["rev"]["maxIvl"].as_u64().unwrap_or(36_500) as u32,
        relearning_steps_min: v["lapse"]["delays"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u32)).collect())
            .unwrap_or_else(|| vec![10]),
        lapse_interval_factor: v["lapse"]["mult"].as_f64().unwrap_or(0.0),
        minimum_interval_days: v["lapse"]["minInt"].as_u64().unwrap_or(1) as u32,
        leech_threshold: v["lapse"]["leechThreshold"].as_u64().unwrap_or(8) as u32,
        fsrs_weights,
        desired_retention: v["fsrs"]["desiredRetention"].as_f64().unwrap_or(0.9),
    }
}

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

/// Learn-ahead window (ms): when nothing else is due, a learning card whose due
/// time is within this window of `now` may be shown early. Matches Anki's
/// default collapse / learn-ahead limit of 20 minutes.
const LEARN_AHEAD_MS: i64 = 20 * 60 * 1000;

/// New-card order jitter. New cards carry their import position in `due` — for a
/// frequency-ranked deck that means most-frequent first. We sort by that rank
/// plus a small random offset in `0..NEW_ORDER_JITTER`, so the run stays roughly
/// ranked (frequent words early) without being perfectly predictable: a card can
/// drift at most `NEW_ORDER_JITTER` positions, which also breaks up sibling runs.
const NEW_ORDER_JITTER: i64 = 5;

/// The studyable cards in a deck, split into their three streams plus a single
/// learn-ahead fallback. The application layer ([`Collection::next_card`])
/// decides the order; this just gates each stream correctly.
///
/// `cards.due` is dual-unit (Anki convention): epoch-**milliseconds** for
/// learning/relearning cards, day-**number** for review cards. So learning is
/// gated by `now_ms` and review by `today`.
pub fn study_queue(
    conn: &Connection,
    deck_id: i64,
    today: i32,
    now_ms: i64,
    new_limit: u32,
    review_limit: u32,
) -> CoreResult<synapse_core::ports::StudyQueue> {
    // 1. Learning/relearning that is due now — time-critical, no cap, by due.
    let mut stmt = conn
        .prepare(
            "SELECT id FROM cards WHERE deck_id = ?1 AND queue NOT IN (-1,-2,-3)
             AND type IN (1, 3) AND due <= ?2 ORDER BY due",
        )
        .map_err(err)?;
    let learning: Vec<i64> = stmt
        .query_map(params![deck_id, now_ms], |r| r.get(0))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;

    // 2. New — frequency rank (`due`) with a small random jitter, so the deck's
    //    ordering shows through without being fully predictable. `RANDOM() &
    //    0x7FFF_FFFF` keeps a non-negative key (avoids ABS(i64::MIN) overflow).
    let mut new: Vec<i64> = Vec::new();
    if new_limit > 0 {
        let mut stmt = conn
            .prepare(
                "SELECT id FROM cards WHERE deck_id = ?1 AND queue NOT IN (-1,-2,-3)
                 AND type = 0 ORDER BY due + ((RANDOM() & 2147483647) % ?3), id LIMIT ?2",
            )
            .map_err(err)?;
        new = stmt
            .query_map(params![deck_id, new_limit, NEW_ORDER_JITTER], |r| r.get(0))
            .map_err(err)?
            .collect::<rusqlite::Result<_>>()
            .map_err(err)?;
    }

    // 3. Reviews — random order within today's due set.
    let mut review: Vec<i64> = Vec::new();
    if review_limit > 0 {
        let mut stmt = conn
            .prepare(
                "SELECT id FROM cards WHERE deck_id = ?1 AND queue NOT IN (-1,-2,-3)
                 AND type = 2 AND due <= ?2 ORDER BY RANDOM() LIMIT ?3",
            )
            .map_err(err)?;
        review = stmt
            .query_map(params![deck_id, today, review_limit], |r| r.get(0))
            .map_err(err)?
            .collect::<rusqlite::Result<_>>()
            .map_err(err)?;
    }

    // 4. Learn-ahead fallback — the soonest learning card due within the window.
    let learning_ahead: Option<i64> = conn
        .query_row(
            "SELECT id FROM cards WHERE deck_id = ?1 AND queue NOT IN (-1,-2,-3)
             AND type IN (1, 3) AND due > ?2 AND due <= ?3 ORDER BY due LIMIT 1",
            params![deck_id, now_ms, now_ms + LEARN_AHEAD_MS],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;

    Ok(synapse_core::ports::StudyQueue {
        learning,
        new,
        review,
        learning_ahead,
    })
}

pub fn count_due_by_type(
    conn: &Connection,
    deck_id: i64,
    today: i32,
    now_ms: i64,
    new_limit: u32,
    review_limit: u32,
) -> CoreResult<(u32, u32, u32)> {
    let learning: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE deck_id = ?1 AND queue NOT IN (-1,-2,-3)
             AND type IN (1, 3) AND due <= ?2",
            params![deck_id, now_ms],
            |r| r.get(0),
        )
        .map_err(err)?;
    let new_total: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE deck_id = ?1 AND queue NOT IN (-1,-2,-3)
             AND type = 0",
            [deck_id],
            |r| r.get(0),
        )
        .map_err(err)?;
    let review_total: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE deck_id = ?1 AND queue NOT IN (-1,-2,-3)
             AND type = 2 AND due <= ?2",
            params![deck_id, today],
            |r| r.get(0),
        )
        .map_err(err)?;
    Ok((new_total.min(new_limit), learning, review_total.min(review_limit)))
}

pub fn count_due(
    conn: &Connection,
    deck_id: i64,
    today: i32,
    now_ms: i64,
    new_limit: u32,
    review_limit: u32,
) -> CoreResult<u32> {
    let (n, l, r) = count_due_by_type(conn, deck_id, today, now_ms, new_limit, review_limit)?;
    Ok(n + l + r)
}

pub fn deck_due_counts(
    conn: &Connection,
    today: i32,
    now_ms: i64,
) -> CoreResult<HashMap<i64, (u32, u32, u32)>> {
    let mut stmt = conn
        .prepare(
            "SELECT deck_id,
                    SUM(CASE WHEN type = 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN type IN (1, 3) AND due <= ?2 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN type = 2 AND due <= ?1 THEN 1 ELSE 0 END)
             FROM cards
             WHERE queue NOT IN (-1, -2, -3)
             GROUP BY deck_id",
        )
        .map_err(err)?;
    let mut map = HashMap::new();
    let rows = stmt
        .query_map(params![today, now_ms], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, u32>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, u32>(3)?,
            ))
        })
        .map_err(err)?;
    for row in rows {
        let (deck_id, new, learning, review) = row.map_err(err)?;
        map.insert(deck_id, (new, learning, review));
    }
    Ok(map)
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
                Ok((r.get::<_, i64>(0)?, CardRow {
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
                }))
            },
        )
        .optional()
        .map_err(err)?;
    let Some((note_id, row)) = row else { return Ok(None) };

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
        note_id,
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
        let queue = storage.study_queue(1, 0, 1_700_000_000_000, 20, 200).unwrap();
        assert_eq!(queue.new.len(), 1);
        assert!(queue.learning.is_empty() && queue.review.is_empty());
        let card_id = queue.new[0];

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

        let queue = storage.study_queue(1, 0, 1_700_000_000_000, 20, 200).unwrap();
        assert!(queue.new.is_empty() && queue.learning.is_empty() && queue.review.is_empty());
        assert_eq!(
            storage.study_card(card_id).unwrap().unwrap().state.phase,
            CardPhase::Review
        );
    }

    /// A learning card builder: `due` is epoch-ms, `ctype`/`queue` mark it as
    /// (re)learning. Reuses the `model()` deck/notetype/template.
    fn model_with_cards(cards: Vec<Card>) -> CanonicalModel {
        let mut m = model();
        // Give each card its own note so the notes table stays consistent.
        m.notes = cards
            .iter()
            .enumerate()
            .map(|(i, c)| Note {
                id: c.note_id,
                guid: format!("g{i}"),
                notetype_id: 10,
                mod_ms: 0,
                usn: -1,
                tags: vec![],
                fields: vec!["q".into(), "a".into()],
                sort_field: Some("q".into()),
                checksum: None,
            })
            .collect();
        m.cards = cards;
        m
    }

    fn card(id: i64, ctype: i64, queue: i64, due: i64) -> Card {
        Card {
            id,
            note_id: id + 1_000,
            deck_id: 1,
            ord: 0,
            mod_ms: 0,
            usn: -1,
            ctype,
            queue,
            due,
            interval: 0,
            ease_factor: 2500,
            reps: 0,
            lapses: 0,
            remaining: 1,
            original_due: 0,
            original_deck_id: 0,
            flags: 0,
            fsrs_stability: None,
            fsrs_difficulty: None,
            fsrs_last_review: None,
            data: None,
        }
    }

    const NOW: i64 = 1_700_000_000_000;
    const MIN_MS: i64 = 60_000;

    #[test]
    fn learning_card_due_in_future_is_not_served() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        storage.ensure_collection(NOW).unwrap();
        // A learning card due in 10 min and a brand-new card.
        storage
            .import(&model_with_cards(vec![
                card(1, 1, 1, NOW + 10 * MIN_MS), // learning, future
                card(2, 0, 0, 0),                 // new
            ]))
            .unwrap();

        let q = storage.study_queue(1, 0, NOW, 20, 200).unwrap();
        // Future learning card is gated out; the new card is available.
        assert!(q.learning.is_empty());
        assert_eq!(q.new, vec![2]);
        // It sits inside the learn-ahead window, so it is the fallback.
        assert_eq!(q.learning_ahead, Some(1));

        // The badge count must exclude the not-yet-due learning card.
        let (new, learning, review) = storage.count_due_by_type(1, 0, NOW, 20, 200).unwrap();
        assert_eq!((new, learning, review), (1, 0, 0));
    }

    #[test]
    fn learning_card_due_now_is_served_first() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        storage.ensure_collection(NOW).unwrap();
        storage
            .import(&model_with_cards(vec![
                card(1, 1, 1, NOW - MIN_MS), // learning, overdue
                card(2, 0, 0, 0),            // new
            ]))
            .unwrap();

        let q = storage.study_queue(1, 0, NOW, 20, 200).unwrap();
        assert_eq!(q.learning, vec![1]);
        let (_, learning, _) = storage.count_due_by_type(1, 0, NOW, 20, 200).unwrap();
        assert_eq!(learning, 1);
    }

    #[test]
    fn new_cards_keep_rough_frequency_order() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        storage.ensure_collection(NOW).unwrap();
        // 50 new cards, `due` = frequency rank 1..=50 (most frequent first).
        let cards: Vec<Card> = (1..=50).map(|p| card(p, 0, 0, p)).collect();
        storage.import(&model_with_cards(cards)).unwrap();

        let q = storage.study_queue(1, 0, NOW, 100, 200).unwrap();
        assert_eq!(q.new.len(), 50);

        // Jitter is bounded: a card at rank `p` gets a key in `[p, p+JITTER)`, so
        // at most JITTER-1 lower-ranked cards can overtake it. Rank-1 card (id 1)
        // therefore cannot drift past index JITTER-1.
        let jitter = 5usize; // mirrors NEW_ORDER_JITTER
        let pos_of_1 = q.new.iter().position(|&id| id == 1).unwrap();
        assert!(pos_of_1 < jitter, "rank-1 card drifted to {pos_of_1}");

        // But it is jittered, not a strict sort — over the first chunk the order
        // should differ from perfectly ascending at least once across a few runs.
        let strictly_sorted = (0..20).all(|i| q.new[i] == (i as i64) + 1);
        let mut any_jitter = !strictly_sorted;
        for _ in 0..8 {
            let q2 = storage.study_queue(1, 0, NOW, 100, 200).unwrap();
            if (0..20).any(|i| q2.new[i] != (i as i64) + 1) {
                any_jitter = true;
            }
        }
        assert!(any_jitter, "order never jittered across runs");
    }

    #[test]
    fn learn_ahead_only_within_window() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        storage.ensure_collection(NOW).unwrap();
        // One learning card due well beyond the 20-min window, nothing else.
        storage
            .import(&model_with_cards(vec![card(1, 1, 1, NOW + 60 * MIN_MS)]))
            .unwrap();

        let q = storage.study_queue(1, 0, NOW, 20, 200).unwrap();
        assert!(q.learning.is_empty() && q.new.is_empty() && q.review.is_empty());
        assert_eq!(q.learning_ahead, None);
    }

    // ── Performance regression guards ─────────────────────────────────────────

    /// Build a storage with `n` review cards (all due today) in the Default deck.
    /// Migrations seed deck_config(1) and deck(1). Insert notetype with explicit
    /// id=10 so bulk note inserts satisfy the FK on notes.notetype_id.
    fn storage_with_reviews(n: usize) -> SqliteStorage {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let conn = storage.lock();
        conn.execute(
            r#"INSERT INTO notetypes (id, name, kind, "mod", usn, config)
               VALUES (10, 'Basic', 0, 0, -1, '{}')"#,
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO fields (notetype_id, ord, name, config) VALUES (10, 0, 'Front', '{}')",
            [],
        ).unwrap();
        conn.execute_batch("BEGIN").unwrap();
        for i in 1..=(n as i64) {
            let note_id = i + 1_000_000;
            conn.execute(
                r#"INSERT INTO notes (id, guid, notetype_id, "mod", usn, tags, fields, sort_field, checksum)
                   VALUES (?1, ?2, 10, 0, -1, '', 'q', 'q', 0)"#,
                rusqlite::params![note_id, format!("pg{i}")],
            ).unwrap();
            conn.execute(
                r#"INSERT INTO cards (id, note_id, deck_id, ord, "mod", usn, type, queue, due,
                    interval, ease_factor, reps, lapses, remaining, original_due,
                    original_deck_id, flags, fsrs_stability, fsrs_difficulty,
                    fsrs_last_review, data)
                   VALUES (?1, ?2, 1, 0, 0, -1, 2, 2, 0, 10, 2500, 3, 0, 0, 0, 0, 0,
                           4.0, 5.0, 0, '')"#,
                rusqlite::params![i, note_id],
            ).unwrap();
        }
        conn.execute_batch("COMMIT").unwrap();
        drop(conn);
        storage
    }

    #[test]
    fn study_queue_builds_under_500ms_for_10k_reviews() {
        let storage = storage_with_reviews(10_000);
        let start = std::time::Instant::now();
        let q = storage.study_queue(1, 0, NOW, 20, 9999).unwrap();
        let elapsed = start.elapsed();
        // Queue is capped at rev_per_day=9999 but all 10k are eligible.
        assert!(!q.review.is_empty(), "expected non-empty review queue");
        assert!(
            elapsed.as_millis() < 500,
            "study_queue took {}ms for 10k cards (budget 500ms)",
            elapsed.as_millis()
        );
    }

    #[test]
    fn due_count_under_200ms_for_10k_cards() {
        let storage = storage_with_reviews(10_000);
        let start = std::time::Instant::now();
        let (new, learning, review) = storage.count_due_by_type(1, 0, NOW, 20, 9999).unwrap();
        let elapsed = start.elapsed();
        assert_eq!(new, 0);
        assert_eq!(learning, 0);
        assert_eq!(review, 9999); // capped at rev_per_day
        assert!(
            elapsed.as_millis() < 200,
            "count_due_by_type took {}ms for 10k cards (budget 200ms)",
            elapsed.as_millis()
        );
    }
}
