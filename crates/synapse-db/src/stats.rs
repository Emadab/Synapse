//! Aggregate statistics over `revlog` and `cards`, for the dashboards.

use rusqlite::{params, Connection};
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::ipc::{DayCount, StatsDto};

const MS_PER_DAY: i64 = 86_400_000;
const RETENTION_WINDOW_DAYS: i64 = 30;
const FORECAST_DAYS: i64 = 30;

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Storage(e.to_string())
}

fn count(conn: &Connection, sql: &str) -> CoreResult<u32> {
    conn.query_row(sql, [], |r| r.get::<_, i64>(0))
        .map(|n| n as u32)
        .map_err(err)
}

pub fn stats(conn: &Connection, today: i32, now_ms: i64) -> CoreResult<StatsDto> {
    let mut stats = StatsDto::default();

    // Reviews per epoch-day (calendar) for the heatmap.
    {
        let mut stmt = conn
            .prepare("SELECT id / 86400000 AS day, COUNT(*) FROM revlog GROUP BY day ORDER BY day")
            .map_err(err)?;
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
    stats.studied_days = stats.reviews.len() as u32;
    stats.total_reviews = stats.reviews.iter().map(|d| d.count).sum();
    stats.total_time_ms = conn
        .query_row("SELECT COALESCE(SUM(taken_ms), 0) FROM revlog", [], |r| {
            r.get(0)
        })
        .map_err(err)?;

    // Retention: pass rate over real reviews in the last 30 days.
    {
        let cutoff = now_ms - RETENTION_WINDOW_DAYS * MS_PER_DAY;
        let (total, passed): (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(CASE WHEN ease > 1 THEN 1 ELSE 0 END), 0)
                 FROM revlog WHERE id >= ?1 AND review_kind = 1",
                [cutoff],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(err)?;
        stats.retention_pct = if total > 0 {
            passed as f64 / total as f64 * 100.0
        } else {
            0.0
        };
    }

    // Forecast: due review cards per day-offset over the next 30 days.
    {
        let mut stmt = conn
            .prepare(
                "SELECT due - ?1 AS off, COUNT(*) FROM cards
                 WHERE type = 2 AND queue = 2 AND due >= ?1 AND due <= ?1 + ?2
                 GROUP BY off ORDER BY off",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map(params![today, FORECAST_DAYS], |r| {
                Ok(DayCount {
                    day: r.get(0)?,
                    count: r.get::<_, i64>(1)? as u32,
                })
            })
            .map_err(err)?;
        stats.forecast = rows.collect::<rusqlite::Result<_>>().map_err(err)?;
    }

    // Card maturity (excluding suspended/buried where noted).
    stats.new_count = count(
        conn,
        "SELECT COUNT(*) FROM cards WHERE type = 0 AND queue >= 0",
    )?;
    stats.learning_count = count(
        conn,
        "SELECT COUNT(*) FROM cards WHERE type IN (1, 3) AND queue >= 0",
    )?;
    stats.young_count = count(
        conn,
        "SELECT COUNT(*) FROM cards WHERE type = 2 AND interval < 21 AND queue >= 0",
    )?;
    stats.mature_count = count(
        conn,
        "SELECT COUNT(*) FROM cards WHERE type = 2 AND interval >= 21 AND queue >= 0",
    )?;
    stats.suspended_count = count(conn, "SELECT COUNT(*) FROM cards WHERE queue = -1")?;

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use crate::storage::SqliteStorage;
    use synapse_core::ports::Storage;

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

        let s = storage.stats(0, NOW).unwrap();
        assert_eq!(s.total_reviews, 1);
        assert_eq!(s.studied_days, 1);
        assert_eq!(s.reviews.len(), 1);
        assert_eq!(s.total_time_ms, 4200);
        assert_eq!(s.retention_pct, 100.0, "one passing review in window");
        assert_eq!(s.new_count, 2, "two new cards imported");
    }
}
