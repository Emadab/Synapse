//! `SqliteStorage` — the SQLite-backed implementation of
//! [`synapse_core::ports::Storage`].
//!
//! Wraps the connection in a `Mutex` so the adapter is `Send + Sync` (rusqlite's
//! `Connection` is `Send` but not `Sync`) and the application layer can hold it
//! behind a `&self` trait object.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, OptionalExtension, Row};
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::ipc::{NoteDetail, NoteOverview, StatsDto};
use synapse_core::model::{CanonicalModel, Deck, ImportSummary, NoteIndexRow, Revlog, StudyCard};
use synapse_core::ports::Storage;
use synapse_core::scheduling::CardState;

use crate::schema::grave_kind;
use crate::{browse, import, migrations, stats, study};

const DECK_COLUMNS: &str = r#"id, name, parent_id, config_id, "mod", usn, collapsed, is_filtered"#;

fn storage_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Storage(e.to_string())
}

fn map_deck(row: &Row<'_>) -> rusqlite::Result<Deck> {
    Ok(Deck {
        id: row.get(0)?,
        name: row.get(1)?,
        parent_id: row.get(2)?,
        config_id: row.get(3)?,
        mod_ms: row.get(4)?,
        usn: row.get(5)?,
        collapsed: row.get::<_, i64>(6)? != 0,
        is_filtered: row.get::<_, i64>(7)? != 0,
    })
}

pub struct SqliteStorage {
    conn: Mutex<Connection>,
}

impl SqliteStorage {
    /// Open (creating if needed) a collection database at `path`.
    pub fn open(path: impl AsRef<Path>) -> CoreResult<Self> {
        let conn = Connection::open(path).map_err(storage_err)?;
        Self::init(conn)
    }

    /// In-memory database, for tests.
    pub fn open_in_memory() -> CoreResult<Self> {
        let conn = Connection::open_in_memory().map_err(storage_err)?;
        Self::init(conn)
    }

    fn init(mut conn: Connection) -> CoreResult<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(storage_err)?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(storage_err)?;
        conn.pragma_update(None, "foreign_keys", true)
            .map_err(storage_err)?;
        migrations::run(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().expect("collection mutex poisoned")
    }

    /// Resolve the immediate parent id for an Anki-style `A::B::C` name.
    fn parent_id_for(conn: &Connection, name: &str) -> CoreResult<Option<i64>> {
        match name.rsplit_once("::") {
            None => Ok(None),
            Some((parent, _)) => conn
                .query_row("SELECT id FROM decks WHERE name = ?1", [parent], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(storage_err),
        }
    }
}

impl Storage for SqliteStorage {
    fn schema_version(&self) -> CoreResult<i64> {
        self.lock()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(storage_err)
    }

    fn create_deck(&self, name: &str, now_ms: i64) -> CoreResult<Deck> {
        let conn = self.lock();
        let parent_id = Self::parent_id_for(&conn, name)?;
        conn.execute(
            r#"INSERT INTO decks (name, parent_id, config_id, "mod", usn) VALUES (?1, ?2, 1, ?3, -1)"#,
            params![name, parent_id, now_ms],
        )
        .map_err(storage_err)?;
        let id = conn.last_insert_rowid();
        conn.query_row(
            &format!("SELECT {DECK_COLUMNS} FROM decks WHERE id = ?1"),
            [id],
            map_deck,
        )
        .map_err(storage_err)
    }

    fn deck_by_id(&self, id: i64) -> CoreResult<Option<Deck>> {
        self.lock()
            .query_row(
                &format!("SELECT {DECK_COLUMNS} FROM decks WHERE id = ?1"),
                [id],
                map_deck,
            )
            .optional()
            .map_err(storage_err)
    }

    fn deck_by_name(&self, name: &str) -> CoreResult<Option<Deck>> {
        self.lock()
            .query_row(
                &format!("SELECT {DECK_COLUMNS} FROM decks WHERE name = ?1"),
                [name],
                map_deck,
            )
            .optional()
            .map_err(storage_err)
    }

    fn list_decks(&self) -> CoreResult<Vec<Deck>> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(&format!("SELECT {DECK_COLUMNS} FROM decks ORDER BY name"))
            .map_err(storage_err)?;
        let rows = stmt.query_map([], map_deck).map_err(storage_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage_err)
    }

    fn rename_deck(&self, id: i64, name: &str, now_ms: i64) -> CoreResult<()> {
        let affected = self
            .lock()
            .execute(
                r#"UPDATE decks SET name = ?1, "mod" = ?2, usn = -1 WHERE id = ?3"#,
                params![name, now_ms, id],
            )
            .map_err(storage_err)?;
        if affected == 0 {
            return Err(CoreError::NotFound(format!("deck {id}")));
        }
        Ok(())
    }

    fn remove_deck(&self, id: i64) -> CoreResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        tx.execute(
            "INSERT OR REPLACE INTO graves (oid, type, usn) VALUES (?1, ?2, -1)",
            params![id, grave_kind::DECK],
        )
        .map_err(storage_err)?;
        let affected = tx
            .execute("DELETE FROM decks WHERE id = ?1", [id])
            .map_err(storage_err)?;
        if affected == 0 {
            return Err(CoreError::NotFound(format!("deck {id}")));
        }
        tx.commit().map_err(storage_err)?;
        Ok(())
    }

    fn insert_deck(&self, deck: &Deck) -> CoreResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        tx.execute(
            r#"INSERT INTO decks (id, name, parent_id, config_id, "mod", usn, collapsed, is_filtered)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params![
                deck.id,
                deck.name,
                deck.parent_id,
                deck.config_id,
                deck.mod_ms,
                deck.usn,
                deck.collapsed as i64,
                deck.is_filtered as i64,
            ],
        )
        .map_err(storage_err)?;
        // Resurrect: drop the tombstone left by the deletion we're undoing.
        tx.execute(
            "DELETE FROM graves WHERE oid = ?1 AND type = ?2",
            params![deck.id, grave_kind::DECK],
        )
        .map_err(storage_err)?;
        tx.commit().map_err(storage_err)?;
        Ok(())
    }

    fn import(&self, model: &CanonicalModel) -> CoreResult<ImportSummary> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        let summary = import::import(&tx, model)?;
        tx.commit().map_err(storage_err)?;
        Ok(summary)
    }

    fn ensure_collection(&self, now_ms: i64) -> CoreResult<i64> {
        study::ensure_collection(&self.lock(), now_ms)
    }

    fn due_card_ids(&self, deck_id: i64, today: i32) -> CoreResult<Vec<i64>> {
        study::due_card_ids(&self.lock(), deck_id, today)
    }

    fn study_card(&self, card_id: i64) -> CoreResult<Option<StudyCard>> {
        study::study_card(&self.lock(), card_id)
    }

    fn apply_answer(
        &self,
        card_id: i64,
        next: &CardState,
        due: i64,
        log: &Revlog,
    ) -> CoreResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        study::apply_answer(&tx, card_id, next, due, log)?;
        tx.commit().map_err(storage_err)?;
        Ok(())
    }

    fn list_notes(&self, query: Option<&str>, limit: i64) -> CoreResult<Vec<NoteOverview>> {
        browse::list_notes(&self.lock(), query, limit)
    }

    fn note_detail(&self, note_id: i64) -> CoreResult<Option<NoteDetail>> {
        browse::note_detail(&self.lock(), note_id)
    }

    fn update_note(
        &self,
        note_id: i64,
        fields: &[String],
        tags: &[String],
        now_ms: i64,
    ) -> CoreResult<()> {
        browse::update_note(&self.lock(), note_id, fields, tags, now_ms)
    }

    fn stats(&self, today: i32, now_ms: i64) -> CoreResult<StatsDto> {
        stats::stats(&self.lock(), today, now_ms)
    }

    fn index_rows(&self) -> CoreResult<Vec<NoteIndexRow>> {
        browse::index_rows(&self.lock())
    }

    fn notes_by_ids(&self, ids: &[i64]) -> CoreResult<Vec<NoteOverview>> {
        browse::notes_by_ids(&self.lock(), ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage() -> SqliteStorage {
        SqliteStorage::open_in_memory().unwrap()
    }

    #[test]
    fn migrations_apply_and_seed_defaults() {
        let s = storage();
        assert_eq!(s.schema_version().unwrap(), 1);
        // The seeded Default deck is present.
        let decks = s.list_decks().unwrap();
        assert_eq!(decks.len(), 1);
        assert_eq!(decks[0].name, "Default");
        assert_eq!(decks[0].id, 1);
    }

    #[test]
    fn create_links_parent_and_persists() {
        let s = storage();
        let med = s.create_deck("Med", 100).unwrap();
        let head = s.create_deck("Med::Anatomy", 100).unwrap();
        assert_eq!(head.parent_id, Some(med.id));
        assert_eq!(s.deck_by_name("Med::Anatomy").unwrap().unwrap().id, head.id);
        assert_eq!(head.usn, -1);
    }

    #[test]
    fn rename_and_remove_and_reinsert() {
        let s = storage();
        let deck = s.create_deck("Spanish", 100).unwrap();
        s.rename_deck(deck.id, "Español", 200).unwrap();
        assert_eq!(s.deck_by_id(deck.id).unwrap().unwrap().name, "Español");

        s.remove_deck(deck.id).unwrap();
        assert!(s.deck_by_id(deck.id).unwrap().is_none());

        // Re-insert (undo path) resurrects the row and clears the grave.
        let restored = Deck {
            name: "Español".into(),
            ..deck
        };
        s.insert_deck(&restored).unwrap();
        assert!(s.deck_by_id(deck.id).unwrap().is_some());
        let graves: i64 = s
            .lock()
            .query_row(
                "SELECT COUNT(*) FROM graves WHERE oid = ?1",
                [deck.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(graves, 0);
    }
}
