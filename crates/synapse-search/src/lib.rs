//! # synapse-search
//!
//! SQLite FTS5-backed full-text + faceted search over notes. The query
//! language is Anki-flavoured: bare words match note text (AND by default),
//! and `tag:`, `deck:` and `note:` filter by facet (e.g.
//! `tag:verb deck:Spanish hola`). The index lives in an in-memory SQLite
//! database and is rebuilt from `NoteIndexRow`s; the application keeps it in
//! sync by rebuilding on the relevant `DomainEvent`s. The on-disk SQLite
//! database (via synapse-db) stays the transactional source of truth.
//!
//! `is:` selectors (due/new/suspended) depend on live scheduling state and
//! are intersected at the storage layer — not in the index — so any
//! `prefix:value` token that isn't `tag:`/`deck:`/`note:` degrades to a plain
//! text match rather than being treated as a facet.

use std::cell::RefCell;

use rusqlite::Connection;
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::model::NoteIndexRow;

fn err(e: rusqlite::Error) -> CoreError {
    CoreError::Other(Box::new(e))
}

/// In-memory note search index.
pub struct NoteIndex {
    conn: RefCell<Connection>,
}

impl NoteIndex {
    pub fn new() -> CoreResult<Self> {
        let conn = Connection::open_in_memory().map_err(err)?;
        conn.execute_batch("CREATE VIRTUAL TABLE notes USING fts5(text, tag, deck, note);")
            .map_err(err)?;

        Ok(Self {
            conn: RefCell::new(conn),
        })
    }

    /// Replace the entire index contents with `rows`.
    pub fn rebuild(&self, rows: &[NoteIndexRow]) -> CoreResult<()> {
        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction().map_err(err)?;
        tx.execute("DELETE FROM notes;", []).map_err(err)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO notes(rowid, text, tag, deck, note) VALUES (?1, ?2, ?3, ?4, ?5);",
                )
                .map_err(err)?;
            for row in rows {
                stmt.execute(rusqlite::params![
                    row.note_id,
                    row.text,
                    row.tags,
                    row.deck,
                    row.notetype,
                ])
                .map_err(err)?;
            }
        }
        tx.commit().map_err(err)?;
        Ok(())
    }

    /// Note ids matching `query`, best-match first, capped at `limit`. Invalid
    /// query syntax degrades to a plain text search; an unparseable query
    /// yields no results rather than an error.
    pub fn search(&self, query: &str, limit: usize) -> CoreResult<Vec<i64>> {
        let Some(match_expr) = build_match_expr(query) else {
            return Ok(vec![]);
        };

        let conn = self.conn.borrow();
        let mut stmt = match conn
            .prepare("SELECT rowid FROM notes WHERE notes MATCH ?1 ORDER BY rank LIMIT ?2;")
        {
            Ok(stmt) => stmt,
            Err(_) => return Ok(vec![]),
        };

        let rows = stmt.query_map(rusqlite::params![match_expr, limit as i64], |r| {
            r.get::<_, i64>(0)
        });
        let rows = match rows {
            Ok(rows) => rows,
            Err(_) => return Ok(vec![]),
        };

        let mut ids = Vec::new();
        for row in rows {
            match row {
                Ok(id) => ids.push(id),
                Err(_) => return Ok(vec![]),
            }
        }
        Ok(ids)
    }
}

/// Quote `value` as an FTS5 string literal, escaping embedded `"` by
/// doubling it. This is the injection-safety boundary: no raw user substring
/// is ever concatenated unquoted into the MATCH expression, so `AND`/`OR`/
/// `NOT`/`*`/parens inside user input can't hijack the query.
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// Translate an Anki-flavoured query string into an FTS5 MATCH expression.
/// Returns `None` if the query has no usable terms.
fn build_match_expr(query: &str) -> Option<String> {
    let mut clauses = Vec::new();

    for token in query.split_whitespace() {
        if let Some((prefix, value)) = token.split_once(':') {
            if value.is_empty() {
                continue;
            }
            match prefix {
                "tag" | "deck" | "note" => {
                    clauses.push(format!("{prefix}:{}", quote(value)));
                }
                _ => {
                    // Unknown facet (e.g. `is:due`): degrade to plain terms.
                    clauses.push(quote(prefix));
                    clauses.push(quote(value));
                }
            }
        } else if !token.is_empty() {
            clauses.push(quote(token));
        }
    }

    if clauses.is_empty() {
        None
    } else {
        Some(clauses.join(" AND "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, text: &str, tags: &str, deck: &str) -> NoteIndexRow {
        NoteIndexRow {
            note_id: id,
            text: text.into(),
            tags: tags.into(),
            deck: deck.into(),
            notetype: "Basic".into(),
        }
    }

    fn index() -> NoteIndex {
        let index = NoteIndex::new().unwrap();
        index
            .rebuild(&[
                row(1, "hola means hello", "spanish greeting", "Spanish"),
                row(2, "bonjour means hello", "french greeting", "French"),
            ])
            .unwrap();
        index
    }

    #[test]
    fn full_text_search() {
        let index = index();
        assert_eq!(index.search("hola", 10).unwrap(), vec![1]);
        // "hello" is in both notes.
        let both = index.search("hello", 10).unwrap();
        assert_eq!(both.len(), 2);
    }

    #[test]
    fn faceted_search() {
        let index = index();
        assert_eq!(index.search("tag:french", 10).unwrap(), vec![2]);
        assert_eq!(index.search("deck:Spanish", 10).unwrap(), vec![1]);
        // Facet + text, ANDed.
        assert_eq!(index.search("tag:spanish hola", 10).unwrap(), vec![1]);
    }

    #[test]
    fn unsupported_syntax_degrades_gracefully() {
        let index = index();
        // `is:due` isn't a known field; should not error.
        assert!(index.search("is:due hola", 10).is_ok());
    }

    #[test]
    fn special_characters_do_not_error() {
        let index = index();
        assert!(index
            .search("O'Brien \"quoted\" AND (test) * -foo", 10)
            .is_ok());
    }
}
