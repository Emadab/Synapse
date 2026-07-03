//! Aggregate statistics over `revlog` and `cards`, for the dashboards.
//!
//! Day bucketing is collection-relative (`(id - created_ms) / MS_PER_DAY`),
//! matching the scheduler's notion of "today" (`Collection::today()`), so the
//! heatmap/streaks/weekly buckets line up with due-date math elsewhere. The
//! hourly breakdown is the only aggregate shifted into the caller's local
//! timezone (`tz_offset_minutes`), since hour-of-day only makes sense in local
//! time. Deck ids passed in are always resolved by `Collection` from our own
//! `list_decks()` (never raw user text), so they're interpolated directly into
//! the SQL rather than bound as params — simpler than a dynamic `IN (?,?,..)`
//! list for no injection risk.

use rusqlite::Connection;
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::ipc::{
    AnswerButtons, DayCount, DeckStat, FsrsStats, HourlyStat, RetentionWeek, StatsDto,
};
use synapse_core::model::Revlog;
use synapse_scheduler::retrievability;

const MS_PER_DAY: i64 = 86_400_000;
const FORECAST_DAYS: i64 = 30;
const RETENTION_30D_MS: i64 = 30 * MS_PER_DAY;
const REVIEWS_7D_MS: i64 = 7 * MS_PER_DAY;
/// Upper edges (days) of the FSRS stability histogram buckets: <1, 1-7, 7-21,
/// 21-90, 90-180, 180-365, 365+ (7 buckets).
const STABILITY_EDGES: [f64; 6] = [1.0, 7.0, 21.0, 90.0, 180.0, 365.0];

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Storage(e.to_string())
}

fn count(conn: &Connection, sql: &str) -> CoreResult<u32> {
    conn.query_row(sql, [], |r| r.get::<_, i64>(0))
        .map(|n| n as u32)
        .map_err(err)
}

/// SQL predicate restricting `{alias}.deck_id` to `deck_ids`, or unrestricted
/// when `None`. See module docs for why literal interpolation is safe here.
fn deck_clause(alias: &str, deck_ids: Option<&[i64]>) -> String {
    match deck_ids {
        None => "1=1".to_string(),
        Some(ids) if ids.is_empty() => "0=1".to_string(),
        Some(ids) => {
            let list = ids
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            format!("{alias}.deck_id IN ({list})")
        }
    }
}

/// A first-answer-per-card-per-day subquery: true retention counts only the
/// first time a card was answered on a given collection-day, so same-day
/// re-reviews (cramming, "Again" retries) don't skew the pass rate.
fn first_answer_clause(created_ms: i64) -> String {
    format!(
        "r.id IN (SELECT MIN(id) FROM revlog GROUP BY card_id, (id - {created_ms}) / {MS_PER_DAY})"
    )
}

#[allow(clippy::too_many_arguments)]
pub fn stats(
    conn: &Connection,
    deck_ids: Option<&[i64]>,
    days: Option<u32>,
    tz_offset_minutes: i32,
    fsrs_weights: &[f64; 21],
    retention_goal_pct: f64,
    today: i32,
    now_ms: i64,
    created_ms: i64,
) -> CoreResult<StatsDto> {
    let mut stats = StatsDto {
        day0_ms: created_ms,
        retention_goal_pct,
        ..Default::default()
    };

    let deck_c = deck_clause("c", deck_ids);
    let cards_c = deck_clause("cards", deck_ids);
    let cutoff_ms = days.map(|d| now_ms - i64::from(d) * MS_PER_DAY);
    let range_clause = match cutoff_ms {
        Some(c) => format!("r.id >= {c}"),
        None => "1=1".to_string(),
    };
    let first_answer = first_answer_clause(created_ms);
    let tz_off_ms = i64::from(tz_offset_minutes) * 60_000;

    // Reviews per collection-relative day (all time, deck-filtered) — heatmap + streaks.
    {
        let sql = format!(
            "SELECT (r.id - {created_ms}) / {MS_PER_DAY} AS day, COUNT(*)
             FROM revlog r JOIN cards c ON c.id = r.card_id
             WHERE {deck_c} GROUP BY day ORDER BY day"
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(DayCount {
                    day: r.get(0)?,
                    count: r.get::<_, i64>(1)? as u32,
                })
            })
            .map_err(err)?;
        stats.reviews = rows.collect::<rusqlite::Result<_>>().map_err(err)?;
    }

    // Totals (range-scoped).
    {
        let sql = format!(
            "SELECT COUNT(*), COALESCE(SUM(r.taken_ms), 0)
             FROM revlog r JOIN cards c ON c.id = r.card_id
             WHERE {deck_c} AND {range_clause}"
        );
        let (total, time_ms): (i64, i64) = conn
            .query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(err)?;
        stats.total_reviews = total as u32;
        stats.total_time_ms = time_ms;
    }
    {
        let sql = format!(
            "SELECT COUNT(DISTINCT (r.id - {created_ms}) / {MS_PER_DAY})
             FROM revlog r JOIN cards c ON c.id = r.card_id
             WHERE {deck_c} AND {range_clause}"
        );
        stats.studied_days = count(conn, &sql)?;
    }

    // True retention (range-scoped): first answer per card/day, review_kind = 1.
    {
        let sql = format!(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN r.ease > 1 THEN 1 ELSE 0 END), 0)
             FROM revlog r JOIN cards c ON c.id = r.card_id
             WHERE {deck_c} AND {range_clause} AND r.review_kind = 1 AND {first_answer}"
        );
        let (total, passed): (i64, i64) = conn
            .query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(err)?;
        stats.retention_pct = if total > 0 {
            passed as f64 / total as f64 * 100.0
        } else {
            0.0
        };
    }

    // Retention weekly, split young/mature (range-scoped, true retention).
    {
        let sql = format!(
            "SELECT ((r.id - {created_ms}) / {MS_PER_DAY}) / 7 AS wk,
                    SUM(CASE WHEN r.last_interval BETWEEN 1 AND 20 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN r.last_interval BETWEEN 1 AND 20 AND r.ease > 1 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN r.last_interval >= 21 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN r.last_interval >= 21 AND r.ease > 1 THEN 1 ELSE 0 END)
             FROM revlog r JOIN cards c ON c.id = r.card_id
             WHERE {deck_c} AND {range_clause} AND r.review_kind = 1 AND {first_answer}
             GROUP BY wk ORDER BY wk"
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(RetentionWeek {
                    week_index: r.get(0)?,
                    young_total: r.get::<_, i64>(1)? as u32,
                    young_passed: r.get::<_, i64>(2)? as u32,
                    mature_total: r.get::<_, i64>(3)? as u32,
                    mature_passed: r.get::<_, i64>(4)? as u32,
                })
            })
            .map_err(err)?;
        stats.retention_weekly = rows.collect::<rusqlite::Result<_>>().map_err(err)?;
    }

    // Answer-button counts per phase (range-scoped, all real answers).
    {
        let sql = format!(
            "SELECT
                CASE WHEN r.review_kind IN (0, 2) THEN 0
                     WHEN r.last_interval < 21 THEN 1
                     ELSE 2 END AS phase,
                r.ease, COUNT(*)
             FROM revlog r JOIN cards c ON c.id = r.card_id
             WHERE {deck_c} AND {range_clause} AND r.review_kind <= 2 AND r.ease BETWEEN 1 AND 4
             GROUP BY phase, r.ease"
        );
        let mut buttons = AnswerButtons {
            learning: vec![0; 4],
            young: vec![0; 4],
            mature: vec![0; 4],
        };
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)? as u32,
                ))
            })
            .map_err(err)?;
        for row in rows {
            let (phase, ease, n) = row.map_err(err)?;
            let idx = (ease - 1).clamp(0, 3) as usize;
            match phase {
                0 => buttons.learning[idx] = n,
                1 => buttons.young[idx] = n,
                _ => buttons.mature[idx] = n,
            }
        }
        stats.answer_buttons = buttons;
    }

    // Hourly breakdown, shifted to local time (range-scoped).
    {
        let sql = format!(
            "SELECT ((r.id + {tz_off_ms}) % {MS_PER_DAY}) / 3600000 AS hour,
                    COUNT(*), SUM(CASE WHEN r.ease > 1 THEN 1 ELSE 0 END)
             FROM revlog r JOIN cards c ON c.id = r.card_id
             WHERE {deck_c} AND {range_clause} AND r.review_kind <= 2
             GROUP BY hour ORDER BY hour"
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(HourlyStat {
                    hour: r.get::<_, i64>(0)? as u8,
                    total: r.get::<_, i64>(1)? as u32,
                    passed: r.get::<_, i64>(2)? as u32,
                })
            })
            .map_err(err)?;
        stats.hourly = rows.collect::<rusqlite::Result<_>>().map_err(err)?;
    }

    // Forecast + backlog (deck-filtered snapshot, not range-scoped).
    {
        let sql = format!(
            "SELECT due - {today} AS off, COUNT(*) FROM cards
             WHERE type = 2 AND queue = 2 AND due >= {today} AND due <= {today} + {FORECAST_DAYS}
             AND {cards_c}
             GROUP BY off ORDER BY off"
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(DayCount {
                    day: r.get(0)?,
                    count: r.get::<_, i64>(1)? as u32,
                })
            })
            .map_err(err)?;
        stats.forecast = rows.collect::<rusqlite::Result<_>>().map_err(err)?;
    }
    stats.backlog_count = count(
        conn,
        &format!(
            "SELECT COUNT(*) FROM cards WHERE type = 2 AND queue = 2 AND due < {today} AND {cards_c}"
        ),
    )?;

    // Card maturity (deck-filtered snapshot).
    stats.new_count = count(
        conn,
        &format!("SELECT COUNT(*) FROM cards WHERE type = 0 AND queue >= 0 AND {cards_c}"),
    )?;
    stats.learning_count = count(
        conn,
        &format!("SELECT COUNT(*) FROM cards WHERE type IN (1, 3) AND queue >= 0 AND {cards_c}"),
    )?;
    stats.young_count = count(
        conn,
        &format!(
            "SELECT COUNT(*) FROM cards WHERE type = 2 AND interval < 21 AND queue >= 0 AND {cards_c}"
        ),
    )?;
    stats.mature_count = count(
        conn,
        &format!(
            "SELECT COUNT(*) FROM cards WHERE type = 2 AND interval >= 21 AND queue >= 0 AND {cards_c}"
        ),
    )?;
    stats.suspended_count = count(
        conn,
        &format!("SELECT COUNT(*) FROM cards WHERE queue = -1 AND {cards_c}"),
    )?;

    // FSRS distributions (deck-filtered snapshot).
    {
        let sql = format!(
            "SELECT fsrs_stability, fsrs_difficulty, fsrs_last_review FROM cards
             WHERE fsrs_stability IS NOT NULL AND fsrs_difficulty IS NOT NULL
             AND queue >= 0 AND {cards_c}"
        );
        let mut fsrs = FsrsStats {
            stability_buckets: vec![0; STABILITY_EDGES.len() + 1],
            difficulty_buckets: vec![0; 10],
            ..Default::default()
        };
        let mut sum_r = 0.0;
        let mut n_r = 0u32;
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, f64>(0)?,
                    r.get::<_, f64>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            })
            .map_err(err)?;
        for row in rows {
            let (stability, difficulty, last_review) = row.map_err(err)?;
            fsrs.card_count += 1;

            let s_idx = STABILITY_EDGES
                .iter()
                .position(|&edge| stability < edge)
                .unwrap_or(STABILITY_EDGES.len());
            fsrs.stability_buckets[s_idx] += 1;

            let d_idx = ((difficulty - 1.0).clamp(0.0, 8.999)) as usize;
            fsrs.difficulty_buckets[d_idx.min(9)] += 1;

            if let Some(last_day) = last_review {
                let elapsed = (i64::from(today) - last_day).max(0) as f64;
                sum_r += retrievability(elapsed, stability, fsrs_weights) * 100.0;
                n_r += 1;
            }
        }
        fsrs.avg_retrievability = if n_r > 0 {
            Some(sum_r / f64::from(n_r))
        } else {
            None
        };
        stats.fsrs = fsrs;
    }

    // Per-deck rollup table (always all decks, independent of the `deck_ids` filter).
    stats.deck_stats = deck_stats(conn, today, now_ms, created_ms)?;

    Ok(stats)
}

fn deck_stats(conn: &Connection, today: i32, now_ms: i64, created_ms: i64) -> CoreResult<Vec<DeckStat>> {
    use std::collections::HashMap;

    let mut by_deck: HashMap<i64, DeckStat> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT id, name, parent_id FROM decks")
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            })
            .map_err(err)?;
        for row in rows {
            let (id, name, parent_id) = row.map_err(err)?;
            by_deck.insert(
                id,
                DeckStat {
                    deck_id: id,
                    name,
                    parent_id,
                    total_cards: 0,
                    due_today: 0,
                    new_count: 0,
                    retention_pct: 0.0,
                    reviews_7d: 0,
                },
            );
        }
    }

    {
        let sql = format!(
            "SELECT deck_id, COUNT(*),
                    SUM(CASE WHEN type = 0 AND queue >= 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN type = 2 AND queue = 2 AND due <= {today} THEN 1 ELSE 0 END)
             FROM cards GROUP BY deck_id"
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)? as u32,
                    r.get::<_, i64>(2)? as u32,
                    r.get::<_, i64>(3)? as u32,
                ))
            })
            .map_err(err)?;
        for row in rows {
            let (deck_id, total, new_count, due) = row.map_err(err)?;
            if let Some(d) = by_deck.get_mut(&deck_id) {
                d.total_cards = total;
                d.new_count = new_count;
                d.due_today = due;
            }
        }
    }

    {
        let retention_cutoff = now_ms - RETENTION_30D_MS;
        let reviews_cutoff = now_ms - REVIEWS_7D_MS;
        let first_answer = first_answer_clause(created_ms);
        let sql = format!(
            "SELECT c.deck_id,
                    SUM(CASE WHEN r.id >= {reviews_cutoff} THEN 1 ELSE 0 END),
                    SUM(CASE WHEN r.id >= {retention_cutoff} AND r.review_kind = 1 AND {first_answer}
                             THEN 1 ELSE 0 END),
                    SUM(CASE WHEN r.id >= {retention_cutoff} AND r.review_kind = 1 AND r.ease > 1
                             AND {first_answer} THEN 1 ELSE 0 END)
             FROM revlog r JOIN cards c ON c.id = r.card_id
             GROUP BY c.deck_id"
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)? as u32,
                    r.get::<_, i64>(2)? as u32,
                    r.get::<_, i64>(3)? as u32,
                ))
            })
            .map_err(err)?;
        for row in rows {
            let (deck_id, reviews_7d, retention_total, retention_passed) = row.map_err(err)?;
            if let Some(d) = by_deck.get_mut(&deck_id) {
                d.reviews_7d = reviews_7d;
                d.retention_pct = if retention_total > 0 {
                    f64::from(retention_passed) / f64::from(retention_total) * 100.0
                } else {
                    0.0
                };
            }
        }
    }

    let mut out: Vec<DeckStat> = by_deck.into_values().collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn row_to_revlog(r: &rusqlite::Row<'_>) -> rusqlite::Result<Revlog> {
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
}

/// Revlog entries for FSRS weight optimization (review_kind ∈ {0,1,2}).
/// Pass `Some(deck_id)` to restrict to one deck; `None` for the full collection.
pub fn revlogs_for_optimize(conn: &Connection, deck_id: Option<i64>) -> CoreResult<Vec<Revlog>> {
    match deck_id {
        None => {
            let mut stmt = conn
                .prepare(
                    "SELECT r.id, r.card_id, r.usn, r.ease, r.interval, r.last_interval,
                            r.ease_factor, r.taken_ms, r.review_kind
                     FROM revlog r WHERE r.review_kind <= 2 ORDER BY r.card_id, r.id",
                )
                .map_err(err)?;
            let rows: Vec<Revlog> = stmt
                .query_map([], row_to_revlog)
                .map_err(err)?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        }
        Some(did) => {
            let mut stmt = conn
                .prepare(
                    "SELECT r.id, r.card_id, r.usn, r.ease, r.interval, r.last_interval,
                            r.ease_factor, r.taken_ms, r.review_kind
                     FROM revlog r
                     JOIN cards c ON c.id = r.card_id
                     WHERE r.review_kind <= 2 AND c.deck_id = ?1
                     ORDER BY r.card_id, r.id",
                )
                .map_err(err)?;
            let rows: Vec<Revlog> = stmt
                .query_map([did], row_to_revlog)
                .map_err(err)?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::SqliteStorage;
    use synapse_core::ports::Storage;
    use synapse_core::scheduling::FSRS6_DEFAULT_WEIGHTS;

    const NOW: i64 = 1_700_000_000_000;

    fn model() -> synapse_core::model::CanonicalModel {
        use synapse_core::model::*;
        let card = |id: i64| Card {
            id,
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
        };
        CanonicalModel {
            notetypes: vec![Notetype {
                id: 10,
                name: "Basic".into(),
                kind: 0,
                mod_ms: 0,
                usn: -1,
                config_json: "{}".into(),
            }],
            fields: vec![Field {
                notetype_id: 10,
                ord: 0,
                name: "Front".into(),
                config_json: "{}".into(),
            }],
            templates: vec![Template {
                notetype_id: 10,
                ord: 0,
                name: "Card 1".into(),
                qfmt: "{{Front}}".into(),
                afmt: "{{Front}}".into(),
                config_json: "{}".into(),
            }],
            notes: vec![Note {
                id: 100,
                guid: "g1".into(),
                notetype_id: 10,
                mod_ms: 0,
                usn: -1,
                tags: vec![],
                fields: vec!["hola".into()],
                sort_field: Some("hola".into()),
                checksum: None,
            }],
            cards: vec![card(1000), {
                let mut c = card(1001);
                c.ord = 1;
                c
            }],
            revlog: vec![Revlog {
                id: NOW - 1000,
                card_id: 1000,
                usn: -1,
                ease: 3,
                interval: 1,
                last_interval: 0,
                ease_factor: 2500,
                taken_ms: 4200,
                review_kind: 1,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn aggregates_reviews_and_card_counts() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        storage.import(&model()).unwrap();

        let s = storage
            .stats(None, Some(30), 0, &FSRS6_DEFAULT_WEIGHTS, 90.0, 0, NOW, 0)
            .unwrap();
        assert_eq!(s.total_reviews, 1);
        assert_eq!(s.studied_days, 1);
        assert_eq!(s.reviews.len(), 1);
        assert_eq!(s.total_time_ms, 4200);
        assert_eq!(s.retention_pct, 100.0, "one passing review in window");
        assert_eq!(s.new_count, 2, "two new cards imported");
    }

    #[test]
    fn deck_filter_excludes_other_decks() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        storage.import(&model()).unwrap();

        let s = storage
            .stats(
                Some(&[999]),
                None,
                0,
                &FSRS6_DEFAULT_WEIGHTS,
                90.0,
                0,
                NOW,
                0,
            )
            .unwrap();
        assert_eq!(s.total_reviews, 0);
        assert_eq!(s.new_count, 0);
    }
}
