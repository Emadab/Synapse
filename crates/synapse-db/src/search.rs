//! Anki-flavoured browser query parser → SQL.
//!
//! Converts a query string like `deck:Spanish is:due tag:verb -flag:1 hello`
//! into a parameterised SQL WHERE clause over the cards+notes+decks join.
//! Supported tokens:
//!   is:due|new|review|learn|suspended|buried
//!   flag:N          (0–7)
//!   deck:name       (prefix match with `*`, exact otherwise)
//!   tag:name        (exact tag match)
//!   note:name       (notetype exact match)
//!   added:N         (cards added in the last N days)
//!   prop:ivl/lapses/reps/ease/due/stability/difficulty OP value  (OP = > < = >= <=)
//!   answered:<phase>:<ease>[:sinceDays]  (phase = learning|young|mature; mirrors the
//!     stats-dashboard answer-buttons aggregate — distinct cards with a revlog row of
//!     that grade/phase, optionally within the last N days)
//!   -token          (negate any token)
//!   "multi word"    (quoted phrase, text LIKE)
//!   or              (infix OR between surrounding conditions)
//!   bare text       (LIKE match on fields + sort_field)

use rusqlite::Connection;
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::ipc::CardRow;

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Storage(e.to_string())
}

/// Typed SQL bind parameter.
#[derive(Debug, Clone)]
enum Param {
    Int(i64),
    Real(f64),
    Text(String),
}

impl rusqlite::ToSql for Param {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        match self {
            Param::Int(n) => n.to_sql(),
            Param::Real(f) => f.to_sql(),
            Param::Text(s) => s.to_sql(),
        }
    }
}

/// One parsed condition: SQL fragment + associated bindings.
/// `negated` wraps the fragment in `NOT (...)`.
struct Cond {
    negated: bool,
    sql: String,
    params: Vec<Param>,
}

impl Cond {
    fn pos(sql: impl Into<String>, params: Vec<Param>) -> Self {
        Cond {
            negated: false,
            sql: sql.into(),
            params,
        }
    }
}

/// Tokenise respecting double-quoted strings.
fn tokenize(query: &str) -> Vec<String> {
    let mut tokens = vec![];
    let mut cur = String::new();
    let mut in_quote = false;
    for ch in query.chars() {
        match ch {
            '"' => {
                in_quote = !in_quote;
                cur.push(ch);
            }
            ' ' | '\t' if !in_quote => {
                let tok = cur.trim().to_string();
                if !tok.is_empty() {
                    tokens.push(tok);
                }
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    let tok = cur.trim().to_string();
    if !tok.is_empty() {
        tokens.push(tok);
    }
    tokens
}

/// Parse a single token (without leading `-`) into a condition SQL fragment.
fn parse_token(token: &str, today: i32, now_ms: i64) -> Option<Cond> {
    // is:
    if let Some(rest) = token.strip_prefix("is:") {
        let sql = match rest {
            "new" => "c.queue = 0".into(),
            "review" => "c.queue = 2".into(),
            "learn" => "c.queue = 1".into(),
            "due" => {
                format!("(c.queue = 2 AND c.due <= {today}) OR (c.queue = 1 AND c.due <= {now_ms})")
            }
            "suspended" => "c.queue = -1".into(),
            "buried" => "c.queue IN (-2, -3)".into(),
            _ => return None,
        };
        return Some(Cond::pos(sql, vec![]));
    }

    // flag:N
    if let Some(rest) = token.strip_prefix("flag:") {
        if let Ok(n) = rest.parse::<i64>() {
            return Some(Cond::pos("c.flags = ?", vec![Param::Int(n)]));
        }
        return None;
    }

    // deck:name or deck:name*
    if let Some(rest) = token.strip_prefix("deck:") {
        let name = dequote(rest);
        if name.ends_with('*') {
            let prefix = name.trim_end_matches('*');
            let pattern = format!("{prefix}%");
            return Some(Cond::pos("d.name LIKE ?", vec![Param::Text(pattern)]));
        }
        return Some(Cond::pos("d.name = ?", vec![Param::Text(name)]));
    }

    // tag:name
    if let Some(rest) = token.strip_prefix("tag:") {
        let name = dequote(rest);
        let pattern = format!("% {name} %");
        return Some(Cond::pos("n.tags LIKE ?", vec![Param::Text(pattern)]));
    }

    // note:name (notetype)
    if let Some(rest) = token.strip_prefix("note:") {
        let name = dequote(rest);
        return Some(Cond::pos("nt.name = ?", vec![Param::Text(name)]));
    }

    // added:N — cards created in the last N days
    if let Some(rest) = token.strip_prefix("added:") {
        if let Ok(days) = rest.parse::<i64>() {
            // card.id encodes creation ms as the rowid assigned at insert time;
            // approximate: id > (now_ms - days * 86400 * 1000)
            let threshold = now_ms - days * 86_400_000;
            return Some(Cond::pos("c.id > ?", vec![Param::Int(threshold)]));
        }
        return None;
    }

    // prop:field OP value
    if let Some(rest) = token.strip_prefix("prop:") {
        return parse_prop(rest);
    }

    // answered:<phase>:<ease>[:sinceDays] — distinct cards with a revlog row
    // of that grade/phase, mirroring the stats answer-buttons aggregate.
    if let Some(rest) = token.strip_prefix("answered:") {
        return parse_answered(rest, now_ms);
    }

    // Quoted phrase or bare text → LIKE on fields and sort_field
    let text = dequote(token);
    if !text.is_empty() {
        let pattern = format!("%{text}%");
        return Some(Cond::pos(
            "(n.fields LIKE ? OR n.sort_field LIKE ?)",
            vec![Param::Text(pattern.clone()), Param::Text(pattern)],
        ));
    }

    None
}

/// Parse `prop:field OP value` where OP ∈ {>, <, =, >=, <=}.
fn parse_prop(rest: &str) -> Option<Cond> {
    // Determine field and OP: greedily match field name up to operator.
    let ops = [">=", "<=", ">", "<", "="];
    for &op in &ops {
        if let Some(pos) = rest.find(op) {
            let field = &rest[..pos];
            let val_str = &rest[pos + op.len()..];
            let col = match field {
                "ivl" => "c.interval",
                "lapses" => "c.lapses",
                "reps" => "c.reps",
                "due" => "c.due",
                "ease" => "c.ease_factor",
                "stability" => "c.fsrs_stability",
                "difficulty" => "c.fsrs_difficulty",
                _ => return None,
            };
            // ease is stored in milli-percent (2500 = 250%); user writes 250.
            if field == "ease" {
                if let Ok(v) = val_str.parse::<f64>() {
                    let v_milli = (v * 10.0).round() as i64;
                    return Some(Cond::pos(
                        format!("{col} {op} ?"),
                        vec![Param::Int(v_milli)],
                    ));
                }
            } else if field == "stability" || field == "difficulty" {
                if let Ok(v) = val_str.parse::<f64>() {
                    return Some(Cond::pos(format!("{col} {op} ?"), vec![Param::Real(v)]));
                }
            } else if let Ok(v) = val_str.parse::<i64>() {
                return Some(Cond::pos(format!("{col} {op} ?"), vec![Param::Int(v)]));
            }
            return None;
        }
    }
    None
}

/// Parse `answered:<phase>:<ease>[:sinceDays]` into a distinct-card subquery
/// over `revlog`, mirroring the phase/ease bucketing in `synapse-db::stats`
/// (answer-buttons aggregate): phase 0 = learning (review_kind IN (0,2)),
/// young = review_kind 1 with last_interval < 21, mature = >= 21.
fn parse_answered(rest: &str, now_ms: i64) -> Option<Cond> {
    let parts: Vec<&str> = rest.split(':').collect();
    let phase = *parts.first()?;
    let ease: i64 = parts.get(1)?.parse().ok()?;
    if !(1..=4).contains(&ease) {
        return None;
    }
    let phase_sql = match phase {
        "learning" => "r.review_kind IN (0, 2)",
        "young" => "r.review_kind = 1 AND r.last_interval < 21",
        "mature" => "r.review_kind = 1 AND r.last_interval >= 21",
        _ => return None,
    };

    let mut sql = format!(
        "c.id IN (SELECT r.card_id FROM revlog r WHERE r.ease = ? AND r.review_kind <= 2 AND {phase_sql}"
    );
    let mut params = vec![Param::Int(ease)];
    if let Some(days_str) = parts.get(2) {
        let days: i64 = days_str.parse().ok()?;
        sql.push_str(" AND r.id >= ?");
        params.push(Param::Int(now_ms - days * 86_400_000));
    }
    sql.push(')');
    Some(Cond::pos(sql, params))
}

fn dequote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

/// Build the SQL WHERE clause and bind-param list from a query string.
/// Returns `("1=1", [])` for an empty query (all cards).
fn build_where(query: &str, today: i32, now_ms: i64) -> (String, Vec<Param>) {
    if query.trim().is_empty() {
        return ("1=1".into(), vec![]);
    }

    let tokens = tokenize(query);
    let mut parts: Vec<String> = vec![];
    let mut params: Vec<Param> = vec![];
    let mut pending_or = false;
    for tok in &tokens {
        // `or` token: flag next condition to be ORed with the previous.
        if tok.eq_ignore_ascii_case("or") {
            pending_or = true;
            continue;
        }

        let (negated, raw) = if tok.starts_with('-') && tok.len() > 1 {
            (true, &tok[1..])
        } else {
            (false, tok.as_str())
        };

        let Some(mut cond) = parse_token(raw, today, now_ms) else {
            pending_or = false;
            continue;
        };
        if negated {
            cond.negated = !cond.negated;
        }

        let frag = renumber_placeholders(&cond.sql, params.len() + 1);

        let full = if cond.negated {
            format!("NOT ({frag})")
        } else {
            frag
        };

        if pending_or && !parts.is_empty() {
            let prev = parts.pop().unwrap();
            parts.push(format!("({prev} OR {full})"));
            pending_or = false;
        } else {
            parts.push(full);
            pending_or = false;
        }
        params.extend(cond.params);
    }

    if parts.is_empty() {
        return ("1=1".into(), vec![]);
    }
    (parts.join(" AND "), params)
}

/// Replace each bare `?` in `sql` with numbered `?1`, `?2` starting at `start`.
fn renumber_placeholders(sql: &str, start: usize) -> String {
    let mut out = String::with_capacity(sql.len() + 8);
    let mut n = start;
    for ch in sql.chars() {
        if ch == '?' {
            out.push('?');
            out.push_str(&n.to_string());
            n += 1;
        } else {
            out.push(ch);
        }
    }
    out
}

const BASE_SQL: &str = "
SELECT c.id, c.note_id, COALESCE(n.sort_field, ''), d.name, nt.name, n.tags,
       c.queue, c.type, c.due, c.interval, c.lapses, c.reps, c.ease_factor, c.flags
FROM cards c
JOIN notes n ON n.id = c.note_id
JOIN decks d ON d.id = c.deck_id
JOIN notetypes nt ON nt.id = n.notetype_id
WHERE ";

pub fn search_cards(
    conn: &Connection,
    query: &str,
    today: i32,
    now_ms: i64,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<CardRow>> {
    let (where_clause, mut params) = build_where(query, today, now_ms);
    let limit_idx = params.len() + 1;
    params.push(Param::Int(limit));
    let offset_idx = params.len() + 1;
    params.push(Param::Int(offset));

    let sql = format!(
        "{BASE_SQL}{where_clause} ORDER BY c.id DESC LIMIT ?{limit_idx} OFFSET ?{offset_idx}"
    );

    let mut stmt = conn.prepare(&sql).map_err(err)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |r| {
            let tags: String = r.get(5)?;
            Ok(CardRow {
                card_id: r.get(0)?,
                note_id: r.get(1)?,
                sort_field: r.get(2)?,
                deck: r.get(3)?,
                notetype: r.get(4)?,
                tags: tags
                    .split_whitespace()
                    .map(str::to_string)
                    .collect::<Vec<_>>(),
                queue: r.get(6)?,
                card_type: r.get(7)?,
                due: r.get(8)?,
                interval: r.get(9)?,
                lapses: r.get(10)?,
                reps: r.get(11)?,
                flags: r.get(13)?,
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

// ── Bulk helpers ───────────────────────────────────────────────────────────────

/// Delete notes (and their cards + revlogs) by note id list, writing graves.
pub fn delete_notes(conn: &Connection, note_ids: &[i64], now_ms: i64) -> CoreResult<()> {
    if note_ids.is_empty() {
        return Ok(());
    }
    let json = serde_json::to_string(note_ids).map_err(err)?;
    // Revlog rows for all cards of these notes.
    conn.execute(
        "DELETE FROM revlog WHERE card_id IN (SELECT id FROM cards WHERE note_id IN (SELECT value FROM json_each(?1)))",
        [&json],
    )
    .map_err(err)?;
    // Cards.
    conn.execute(
        "DELETE FROM cards WHERE note_id IN (SELECT value FROM json_each(?1))",
        [&json],
    )
    .map_err(err)?;
    // Graves (type 1 = note).
    let insert_graves = format!(
        "INSERT OR IGNORE INTO graves (usn, oid, type) SELECT -1, value, 1 FROM json_each('{}')",
        json.replace('\'', "''")
    );
    conn.execute(&insert_graves, []).map_err(err)?;
    // Notes.
    conn.execute(
        "DELETE FROM notes WHERE id IN (SELECT value FROM json_each(?1))",
        [&json],
    )
    .map_err(err)?;
    // Bump schema mod.
    conn.execute(
        "UPDATE collection SET modified = ?1, schema_mod = ?1 WHERE id = 1",
        [now_ms],
    )
    .map_err(err)?;
    Ok(())
}

/// Reassign a list of cards to a different deck.
pub fn move_cards_to_deck(conn: &Connection, card_ids: &[i64], deck_id: i64) -> CoreResult<()> {
    if card_ids.is_empty() {
        return Ok(());
    }
    let json = serde_json::to_string(card_ids).map_err(err)?;
    conn.execute(
        "UPDATE cards SET deck_id = ?1 WHERE id IN (SELECT value FROM json_each(?2))",
        rusqlite::params![deck_id, json],
    )
    .map_err(err)?;
    Ok(())
}

/// Remove `tag` from a note's tag blob (idempotent).
pub fn remove_note_tag(conn: &Connection, note_id: i64, tag: &str, now_ms: i64) -> CoreResult<()> {
    let current: String = conn
        .query_row("SELECT tags FROM notes WHERE id = ?1", [note_id], |r| {
            r.get(0)
        })
        .map_err(err)?;
    let needle = format!(" {tag} ");
    if !current.contains(&needle) {
        return Ok(());
    }
    let new_tags = current.replace(&needle, " ").trim().to_string();
    let new_tags = if new_tags.is_empty() {
        String::new()
    } else {
        format!(" {new_tags} ")
    };
    conn.execute(
        r#"UPDATE notes SET tags = ?2, "mod" = ?3, usn = -1 WHERE id = ?1"#,
        rusqlite::params![note_id, new_tags, now_ms],
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
            notes: vec![
                Note {
                    id: 1,
                    guid: "g1".into(),
                    notetype_id: 1,
                    mod_ms: 0,
                    usn: -1,
                    tags: vec!["verb".into()],
                    fields: vec!["hello".into(), "hola".into()],
                    sort_field: Some("hello".into()),
                    checksum: None,
                },
                Note {
                    id: 2,
                    guid: "g2".into(),
                    notetype_id: 1,
                    mod_ms: 0,
                    usn: -1,
                    tags: vec!["noun".into()],
                    fields: vec!["cat".into(), "gato".into()],
                    sort_field: Some("cat".into()),
                    checksum: None,
                },
            ],
            cards: vec![
                Card {
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
                    flags: 1,
                    fsrs_stability: None,
                    fsrs_difficulty: None,
                    fsrs_last_review: None,
                    data: None,
                },
                Card {
                    id: 2,
                    note_id: 2,
                    deck_id: 1,
                    ord: 0,
                    mod_ms: 0,
                    usn: -1,
                    ctype: 2,
                    queue: 2,
                    due: 0,
                    interval: 30,
                    ease_factor: 2500,
                    reps: 5,
                    lapses: 1,
                    remaining: 0,
                    original_due: 0,
                    original_deck_id: 0,
                    flags: 0,
                    fsrs_stability: None,
                    fsrs_difficulty: None,
                    fsrs_last_review: None,
                    data: None,
                },
            ],
            ..Default::default()
        })
        .unwrap();
        let card1: i64 = s.lock().query_row("SELECT id FROM cards WHERE note_id = (SELECT id FROM notes WHERE sort_field = 'hello')", [], |r| r.get(0)).unwrap();
        let card2: i64 = s.lock().query_row("SELECT id FROM cards WHERE note_id = (SELECT id FROM notes WHERE sort_field = 'cat')", [], |r| r.get(0)).unwrap();
        (s, card1, card2)
    }

    #[test]
    fn empty_query_returns_all() {
        let (s, _, _) = setup();
        let rows = s.search_cards("", 0, 1_700_000_000_000, 100, 0).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn is_new_filter() {
        let (s, _, _) = setup();
        let rows = s
            .search_cards("is:new", 0, 1_700_000_000_000, 100, 0)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].sort_field, "hello");
    }

    #[test]
    fn is_review_filter() {
        let (s, _, _) = setup();
        let rows = s
            .search_cards("is:review", 0, 1_700_000_000_000, 100, 0)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].sort_field, "cat");
    }

    #[test]
    fn tag_filter() {
        let (s, _, _) = setup();
        let rows = s
            .search_cards("tag:verb", 0, 1_700_000_000_000, 100, 0)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].tags.contains(&"verb".to_string()));
    }

    #[test]
    fn flag_filter() {
        let (s, _, _) = setup();
        let rows = s
            .search_cards("flag:1", 0, 1_700_000_000_000, 100, 0)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].flags, 1);
    }

    #[test]
    fn negation() {
        let (s, _, _) = setup();
        let rows = s
            .search_cards("-is:new", 0, 1_700_000_000_000, 100, 0)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_ne!(rows[0].queue, 0);
    }

    #[test]
    fn text_search() {
        let (s, _, _) = setup();
        let rows = s
            .search_cards("hello", 0, 1_700_000_000_000, 100, 0)
            .unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn prop_lapses() {
        let (s, _, _) = setup();
        let rows = s
            .search_cards("prop:lapses>0", 0, 1_700_000_000_000, 100, 0)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].lapses, 1);
    }

    #[test]
    fn prop_stability_and_difficulty() {
        let (s, card1, _) = setup();
        s.lock()
            .execute(
                "UPDATE cards SET fsrs_stability = 12.5, fsrs_difficulty = 4.2 WHERE id = ?1",
                [card1],
            )
            .unwrap();
        let rows = s
            .search_cards("prop:stability>10", 0, 1_700_000_000_000, 100, 0)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].sort_field, "hello");

        let rows = s
            .search_cards(
                "prop:difficulty>=4 prop:difficulty<5",
                0,
                1_700_000_000_000,
                100,
                0,
            )
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].sort_field, "hello");
    }

    #[test]
    fn answered_filter() {
        let (s, _, card2) = setup();
        let now_ms = 1_700_000_000_000i64;
        s.lock()
            .execute(
                "INSERT INTO revlog (id, card_id, usn, ease, interval, last_interval, ease_factor, taken_ms, review_kind)
                 VALUES (?1, ?2, -1, 3, 30, 30, 2500, 0, 1)",
                rusqlite::params![now_ms - 1000, card2],
            )
            .unwrap();
        let rows = s
            .search_cards("answered:mature:3", 0, now_ms, 100, 0)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].sort_field, "cat");

        // Wrong ease matches nothing.
        let rows = s
            .search_cards("answered:mature:1", 0, now_ms, 100, 0)
            .unwrap();
        assert_eq!(rows.len(), 0);

        // sinceDays window excludes it when too narrow.
        let rows = s
            .search_cards("answered:mature:3:0", 0, now_ms + 2 * 86_400_000, 100, 0)
            .unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn delete_notes_removes_cards_and_notes() {
        let (s, _, _) = setup();
        let note_ids: Vec<i64> = s
            .lock()
            .query_row("SELECT id FROM notes WHERE sort_field = 'hello'", [], |r| {
                r.get(0)
            })
            .map(|id| vec![id])
            .unwrap();
        s.delete_notes(&note_ids, 1_000).unwrap();
        let rows = s.search_cards("", 0, 1_700_000_000_000, 100, 0).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn move_cards_to_deck() {
        let (s, card1, _) = setup();
        // Create a second deck first.
        let new_deck = s
            .lock()
            .query_row(
                "SELECT id FROM decks WHERE id = 1",
                [],
                |r: &rusqlite::Row| r.get::<_, i64>(0),
            )
            .unwrap();
        // Move card1 to same deck (no-op but exercises the path).
        s.move_cards_to_deck(&[card1], new_deck).unwrap();
        let rows = s
            .search_cards("is:new", 0, 1_700_000_000_000, 100, 0)
            .unwrap();
        assert_eq!(rows[0].deck, "Default");
    }
}
