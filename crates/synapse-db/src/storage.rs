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
use synapse_core::ipc::{
    FieldRemoveWarning, FilteredDeckConfig, NoteDetail, NoteOverview, NotetypeDetail, StatsDto,
};
use synapse_core::model::{
    CanonicalModel, Card, Deck, Field, ImportSummary, Note, NoteIndexRow, Notetype, Revlog,
    StudyCard, Template,
};
use synapse_core::ports::{RemovedDeck, Storage};
use synapse_core::scheduling::{CardState, SchedConfig};

use crate::schema::grave_kind;
use crate::{
    backup, browse, cards, export, filtered, import, migrations, notetype, search, stats, study,
    tags,
};

const DECK_COLUMNS: &str = r#"id, name, parent_id, config_id, "mod", usn, collapsed, is_filtered"#;
const CARD_COLUMNS: &str = r#"id, note_id, deck_id, ord, "mod", usn, type, queue, due, interval,
    ease_factor, reps, lapses, remaining, original_due, original_deck_id, flags,
    fsrs_stability, fsrs_difficulty, fsrs_last_review, data"#;
const NOTE_COLUMNS: &str =
    r#"id, guid, notetype_id, "mod", usn, tags, fields, sort_field, checksum"#;
/// Field separator inside the `notes.fields` blob (matches import/export).
const FIELD_SEP: char = '\u{1f}';

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

fn map_card(row: &Row<'_>) -> rusqlite::Result<Card> {
    Ok(Card {
        id: row.get(0)?,
        note_id: row.get(1)?,
        deck_id: row.get(2)?,
        ord: row.get(3)?,
        mod_ms: row.get(4)?,
        usn: row.get(5)?,
        ctype: row.get(6)?,
        queue: row.get(7)?,
        due: row.get(8)?,
        interval: row.get(9)?,
        ease_factor: row.get(10)?,
        reps: row.get(11)?,
        lapses: row.get(12)?,
        remaining: row.get(13)?,
        original_due: row.get(14)?,
        original_deck_id: row.get(15)?,
        flags: row.get(16)?,
        fsrs_stability: row.get(17)?,
        fsrs_difficulty: row.get(18)?,
        fsrs_last_review: row.get(19)?,
        data: row.get(20)?,
    })
}

fn map_note(row: &Row<'_>) -> rusqlite::Result<Note> {
    let tags: String = row.get(5)?;
    let fields: String = row.get(6)?;
    Ok(Note {
        id: row.get(0)?,
        guid: row.get(1)?,
        notetype_id: row.get(2)?,
        mod_ms: row.get(3)?,
        usn: row.get(4)?,
        tags: tags.split_whitespace().map(str::to_string).collect(),
        fields: fields.split(FIELD_SEP).map(str::to_string).collect(),
        sort_field: row.get(7)?,
        checksum: row.get(8)?,
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

    pub(crate) fn lock(&self) -> MutexGuard<'_, Connection> {
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

    fn remove_deck(&self, id: i64) -> CoreResult<RemovedDeck> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;

        // Resolve the subtree: the deck plus every sub-deck (`name::*`).
        let name: String = tx
            .query_row("SELECT name FROM decks WHERE id = ?1", [id], |r| r.get(0))
            .optional()
            .map_err(storage_err)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {id}")))?;

        // Parent-first order so the subtree can be restored (and re-linked) safely.
        let decks: Vec<Deck> = {
            let mut stmt = tx
                .prepare(&format!(
                    "SELECT {DECK_COLUMNS} FROM decks WHERE id = ?1 OR name LIKE ?2 \
                     ORDER BY LENGTH(name)"
                ))
                .map_err(storage_err)?;
            let rows = stmt
                .query_map(params![id, format!("{name}::%")], map_deck)
                .map_err(storage_err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(storage_err)?
        };

        // All cards held by those decks, captured for undo.
        let cards: Vec<Card> = {
            let mut stmt = tx
                .prepare(&format!(
                    "SELECT {CARD_COLUMNS} FROM cards c
                     WHERE c.deck_id IN (SELECT id FROM decks WHERE id = ?1 OR name LIKE ?2)"
                ))
                .map_err(storage_err)?;
            let rows = stmt
                .query_map(params![id, format!("{name}::%")], map_card)
                .map_err(storage_err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(storage_err)?
        };

        // Tombstone cards + decks (sync change-log), then delete bottom-up:
        // revlog → cards → decks (children before parents to honour parent_id).
        for card in &cards {
            tx.execute(
                "INSERT OR REPLACE INTO graves (oid, type, usn) VALUES (?1, ?2, -1)",
                params![card.id, grave_kind::CARD],
            )
            .map_err(storage_err)?;
            tx.execute("DELETE FROM revlog WHERE card_id = ?1", [card.id])
                .map_err(storage_err)?;
        }
        for deck in decks.iter().rev() {
            tx.execute("DELETE FROM cards WHERE deck_id = ?1", [deck.id])
                .map_err(storage_err)?;
            tx.execute(
                "INSERT OR REPLACE INTO graves (oid, type, usn) VALUES (?1, ?2, -1)",
                params![deck.id, grave_kind::DECK],
            )
            .map_err(storage_err)?;
            tx.execute("DELETE FROM decks WHERE id = ?1", [deck.id])
                .map_err(storage_err)?;
        }

        // Cards are gone; delete notes they left orphaned (no card anywhere).
        let mut orphan_note_ids: Vec<i64> = cards.iter().map(|c| c.note_id).collect();
        orphan_note_ids.sort_unstable();
        orphan_note_ids.dedup();
        let mut notes: Vec<Note> = Vec::new();
        for note_id in orphan_note_ids {
            let remaining: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM cards WHERE note_id = ?1",
                    [note_id],
                    |r| r.get(0),
                )
                .map_err(storage_err)?;
            if remaining > 0 {
                continue; // still has cards in another deck — keep it.
            }
            let note = tx
                .query_row(
                    &format!("SELECT {NOTE_COLUMNS} FROM notes WHERE id = ?1"),
                    [note_id],
                    map_note,
                )
                .optional()
                .map_err(storage_err)?;
            if let Some(note) = note {
                tx.execute(
                    "INSERT OR REPLACE INTO graves (oid, type, usn) VALUES (?1, ?2, -1)",
                    params![note_id, grave_kind::NOTE],
                )
                .map_err(storage_err)?;
                tx.execute("DELETE FROM notes WHERE id = ?1", [note_id])
                    .map_err(storage_err)?;
                notes.push(note);
            }
        }

        tx.commit().map_err(storage_err)?;
        Ok(RemovedDeck {
            decks,
            cards,
            notes,
        })
    }

    fn restore_deck(&self, removed: &RemovedDeck) -> CoreResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        // Decks first (parent-first, as captured), then their cards.
        for deck in &removed.decks {
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
            tx.execute(
                "DELETE FROM graves WHERE oid = ?1 AND type = ?2",
                params![deck.id, grave_kind::DECK],
            )
            .map_err(storage_err)?;
        }
        // Notes before cards (cards reference notes).
        for note in &removed.notes {
            let tags = if note.tags.is_empty() {
                String::new()
            } else {
                format!(" {} ", note.tags.join(" "))
            };
            let fields = note.fields.join(&FIELD_SEP.to_string());
            tx.execute(
                r#"INSERT INTO notes (id, guid, notetype_id, "mod", usn, tags, fields, sort_field, checksum)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
                params![
                    note.id,
                    note.guid,
                    note.notetype_id,
                    note.mod_ms,
                    note.usn,
                    tags,
                    fields,
                    note.sort_field,
                    note.checksum,
                ],
            )
            .map_err(storage_err)?;
            tx.execute(
                "DELETE FROM graves WHERE oid = ?1 AND type = ?2",
                params![note.id, grave_kind::NOTE],
            )
            .map_err(storage_err)?;
        }
        for card in &removed.cards {
            tx.execute(
                r#"INSERT INTO cards
                   (id, note_id, deck_id, ord, "mod", usn, type, queue, due, interval, ease_factor,
                    reps, lapses, remaining, original_due, original_deck_id, flags,
                    fsrs_stability, fsrs_difficulty, fsrs_last_review, data)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                    ?17, ?18, ?19, ?20, ?21)"#,
                params![
                    card.id,
                    card.note_id,
                    card.deck_id,
                    card.ord,
                    card.mod_ms,
                    card.usn,
                    card.ctype,
                    card.queue,
                    card.due,
                    card.interval,
                    card.ease_factor,
                    card.reps,
                    card.lapses,
                    card.remaining,
                    card.original_due,
                    card.original_deck_id,
                    card.flags,
                    card.fsrs_stability,
                    card.fsrs_difficulty,
                    card.fsrs_last_review,
                    card.data,
                ],
            )
            .map_err(storage_err)?;
            tx.execute(
                "DELETE FROM graves WHERE oid = ?1 AND type = ?2",
                params![card.id, grave_kind::CARD],
            )
            .map_err(storage_err)?;
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
        self.import_with_progress(model, &mut |_, _| {})
    }

    fn import_with_progress(
        &self,
        model: &CanonicalModel,
        on_progress: &mut dyn FnMut(u32, u32),
    ) -> CoreResult<ImportSummary> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        let summary = import::import(&tx, model, on_progress)?;
        tx.commit().map_err(storage_err)?;
        Ok(summary)
    }

    fn ensure_collection(&self, now_ms: i64) -> CoreResult<i64> {
        let created = study::ensure_collection(&self.lock(), now_ms)?;
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        crate::stock::seed_if_empty(&tx, now_ms)?;
        tx.commit().map_err(storage_err)?;
        Ok(created)
    }

    fn study_queue(
        &self,
        deck_id: i64,
        today: i32,
        now_ms: i64,
        today_end_ms: i64,
        new_limit: u32,
        review_limit: u32,
    ) -> CoreResult<synapse_core::ports::StudyQueue> {
        study::study_queue(
            &self.lock(),
            deck_id,
            today,
            now_ms,
            today_end_ms,
            new_limit,
            review_limit,
        )
    }

    fn count_due_by_type(
        &self,
        deck_id: i64,
        today: i32,
        now_ms: i64,
        today_end_ms: i64,
        new_limit: u32,
        review_limit: u32,
    ) -> CoreResult<(u32, u32, u32)> {
        study::count_due_by_type(
            &self.lock(),
            deck_id,
            today,
            now_ms,
            today_end_ms,
            new_limit,
            review_limit,
        )
    }

    fn count_due(
        &self,
        deck_id: i64,
        today: i32,
        now_ms: i64,
        today_end_ms: i64,
        new_limit: u32,
        review_limit: u32,
    ) -> CoreResult<u32> {
        study::count_due(
            &self.lock(),
            deck_id,
            today,
            now_ms,
            today_end_ms,
            new_limit,
            review_limit,
        )
    }

    fn deck_due_counts(
        &self,
        today: i32,
        now_ms: i64,
        today_end_ms: i64,
    ) -> CoreResult<std::collections::HashMap<i64, (u32, u32, u32)>> {
        study::deck_due_counts(&self.lock(), today, now_ms, today_end_ms)
    }

    fn cards_due_ms(&self, card_ids: &[i64]) -> CoreResult<std::collections::HashMap<i64, i64>> {
        study::cards_due_ms(&self.lock(), card_ids)
    }

    fn deck_limits(&self, config_id: i64) -> CoreResult<(u32, u32)> {
        study::deck_limits(&self.lock(), config_id)
    }

    fn all_deck_limits(&self) -> CoreResult<std::collections::HashMap<i64, (u32, u32)>> {
        study::all_deck_limits(&self.lock())
    }

    fn today_studied(&self, deck_id: i64, today_start_ms: i64) -> CoreResult<(u32, u32)> {
        study::today_studied(&self.lock(), deck_id, today_start_ms)
    }

    fn all_today_studied(
        &self,
        today_start_ms: i64,
    ) -> CoreResult<std::collections::HashMap<i64, (u32, u32)>> {
        study::all_today_studied(&self.lock(), today_start_ms)
    }

    fn set_deck_limits(
        &self,
        config_id: i64,
        new_per_day: u32,
        rev_per_day: u32,
        now_ms: i64,
    ) -> CoreResult<()> {
        study::set_deck_limits(&self.lock(), config_id, new_per_day, rev_per_day, now_ms)
    }

    fn get_deck_config(&self, config_id: i64) -> CoreResult<SchedConfig> {
        study::get_deck_config(&self.lock(), config_id)
    }

    fn day_extra_new(&self, deck_id: i64, day: i32) -> CoreResult<u32> {
        study::day_extra_new(&self.lock(), deck_id, day)
    }

    fn all_day_extra_new(&self, day: i32) -> CoreResult<std::collections::HashMap<i64, u32>> {
        study::all_day_extra_new(&self.lock(), day)
    }

    fn set_day_extra_new(&self, deck_id: i64, day: i32, extra_new: u32) -> CoreResult<()> {
        study::set_day_extra_new(&self.lock(), deck_id, day, extra_new)
    }

    fn set_deck_config(&self, config_id: i64, config: &SchedConfig, now_ms: i64) -> CoreResult<()> {
        study::set_deck_config(&self.lock(), config_id, config, now_ms)
    }

    fn get_rollover_hour(&self) -> CoreResult<u8> {
        study::get_rollover_hour(&self.lock())
    }

    fn set_rollover_hour(&self, hour: u8, now_ms: i64) -> CoreResult<()> {
        study::set_rollover_hour(&self.lock(), hour, now_ms)
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

    #[allow(clippy::too_many_arguments)]
    fn stats(
        &self,
        deck_ids: Option<&[i64]>,
        days: Option<u32>,
        tz_offset_minutes: i32,
        fsrs_weights: &[f64; 21],
        retention_goal_pct: f64,
        today: i32,
        now_ms: i64,
        created_ms: i64,
    ) -> CoreResult<StatsDto> {
        stats::stats(
            &self.lock(),
            deck_ids,
            days,
            tz_offset_minutes,
            fsrs_weights,
            retention_goal_pct,
            today,
            now_ms,
            created_ms,
        )
    }

    fn index_rows(&self) -> CoreResult<Vec<NoteIndexRow>> {
        browse::index_rows(&self.lock())
    }

    fn notes_by_ids(&self, ids: &[i64]) -> CoreResult<Vec<NoteOverview>> {
        browse::notes_by_ids(&self.lock(), ids)
    }

    fn dump_collection(&self) -> CoreResult<CanonicalModel> {
        export::dump_collection(&self.lock())
    }

    fn list_notetypes(&self) -> CoreResult<Vec<Notetype>> {
        browse::list_notetypes(&self.lock())
    }

    fn fields_for_notetype(&self, notetype_id: i64) -> CoreResult<Vec<Field>> {
        browse::fields_for_notetype(&self.lock(), notetype_id)
    }

    fn templates_for_notetype(&self, notetype_id: i64) -> CoreResult<Vec<Template>> {
        browse::templates_for_notetype(&self.lock(), notetype_id)
    }

    fn add_note_with_cards(
        &self,
        notetype_id: i64,
        deck_id: i64,
        fields: &[String],
        tags: &[String],
        now_ms: i64,
    ) -> CoreResult<(i64, u32)> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        let result = browse::add_note_with_cards(&tx, notetype_id, deck_id, fields, tags, now_ms)?;
        tx.commit().map_err(storage_err)?;
        Ok(result)
    }

    fn get_notetype_detail(&self, notetype_id: i64) -> CoreResult<Option<NotetypeDetail>> {
        notetype::get_notetype_detail(&self.lock(), notetype_id)
    }

    fn create_notetype(&self, name: &str, kind: i64, now_ms: i64) -> CoreResult<i64> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        let id = notetype::create_notetype(&tx, name, kind, now_ms)?;
        tx.commit().map_err(storage_err)?;
        Ok(id)
    }

    fn delete_notetype(&self, notetype_id: i64, now_ms: i64) -> CoreResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        notetype::delete_notetype(&tx, notetype_id, now_ms)?;
        tx.commit().map_err(storage_err)?;
        Ok(())
    }

    fn rename_notetype(&self, notetype_id: i64, name: &str, now_ms: i64) -> CoreResult<()> {
        notetype::rename_notetype(&self.lock(), notetype_id, name, now_ms)
    }

    fn stock_notetype_names(&self) -> Vec<&'static str> {
        crate::stock::stock_names()
    }

    fn add_stock_notetype(&self, index: usize, now_ms: i64) -> CoreResult<i64> {
        let id = crate::stock::add_stock(&self.lock(), index, now_ms)?;
        Ok(id)
    }

    fn save_notetype_css(&self, notetype_id: i64, css: &str, now_ms: i64) -> CoreResult<()> {
        notetype::save_notetype_css(&self.lock(), notetype_id, css, now_ms)
    }

    fn add_field(&self, notetype_id: i64, name: &str, now_ms: i64) -> CoreResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        notetype::add_field(&tx, notetype_id, name, now_ms)?;
        tx.commit().map_err(storage_err)?;
        Ok(())
    }

    fn check_field_remove(&self, notetype_id: i64, ord: i64) -> CoreResult<FieldRemoveWarning> {
        notetype::check_field_remove(&self.lock(), notetype_id, ord)
    }

    fn remove_field(&self, notetype_id: i64, ord: i64, now_ms: i64) -> CoreResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        notetype::remove_field(&tx, notetype_id, ord, now_ms)?;
        tx.commit().map_err(storage_err)?;
        Ok(())
    }

    fn rename_field(&self, notetype_id: i64, ord: i64, name: &str, now_ms: i64) -> CoreResult<()> {
        notetype::rename_field(&self.lock(), notetype_id, ord, name, now_ms)
    }

    fn reorder_fields(&self, notetype_id: i64, new_order: &[i64], now_ms: i64) -> CoreResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        notetype::reorder_fields(&tx, notetype_id, new_order, now_ms)?;
        tx.commit().map_err(storage_err)?;
        Ok(())
    }

    fn add_template(
        &self,
        notetype_id: i64,
        name: &str,
        qfmt: &str,
        afmt: &str,
        now_ms: i64,
    ) -> CoreResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        notetype::add_template(&tx, notetype_id, name, qfmt, afmt, now_ms)?;
        tx.commit().map_err(storage_err)?;
        Ok(())
    }

    fn remove_template(&self, notetype_id: i64, ord: i64, now_ms: i64) -> CoreResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(storage_err)?;
        notetype::remove_template(&tx, notetype_id, ord, now_ms)?;
        tx.commit().map_err(storage_err)?;
        Ok(())
    }

    fn save_template(
        &self,
        notetype_id: i64,
        ord: i64,
        name: &str,
        qfmt: &str,
        afmt: &str,
        now_ms: i64,
    ) -> CoreResult<()> {
        notetype::save_template(&self.lock(), notetype_id, ord, name, qfmt, afmt, now_ms)
    }

    fn suspend_cards(&self, card_ids: &[i64]) -> CoreResult<()> {
        cards::suspend_cards(&self.lock(), card_ids)
    }

    fn unsuspend_cards(&self, card_ids: &[i64]) -> CoreResult<()> {
        cards::unsuspend_cards(&self.lock(), card_ids)
    }

    fn bury_cards(&self, card_ids: &[i64]) -> CoreResult<()> {
        cards::bury_cards(&self.lock(), card_ids)
    }

    fn bury_siblings(&self, note_id: i64, answered_card_id: i64) -> CoreResult<()> {
        cards::bury_siblings(&self.lock(), note_id, answered_card_id)
    }

    fn unbury_deck(&self, deck_id: i64) -> CoreResult<()> {
        cards::unbury_deck(&self.lock(), deck_id)
    }

    fn set_card_flag(&self, card_ids: &[i64], flag: u8) -> CoreResult<()> {
        cards::set_card_flag(&self.lock(), card_ids, flag)
    }

    fn add_note_tag(&self, note_id: i64, tag: &str, now_ms: i64) -> CoreResult<()> {
        cards::add_note_tag(&self.lock(), note_id, tag, now_ms)
    }

    fn search_cards(
        &self,
        query: &str,
        today: i32,
        now_ms: i64,
        limit: i64,
    ) -> CoreResult<Vec<synapse_core::ipc::CardRow>> {
        search::search_cards(&self.lock(), query, today, now_ms, limit)
    }

    fn delete_notes(&self, note_ids: &[i64], now_ms: i64) -> CoreResult<()> {
        search::delete_notes(&self.lock(), note_ids, now_ms)
    }

    fn move_cards_to_deck(&self, card_ids: &[i64], deck_id: i64) -> CoreResult<()> {
        search::move_cards_to_deck(&self.lock(), card_ids, deck_id)
    }

    fn remove_note_tag(&self, note_id: i64, tag: &str, now_ms: i64) -> CoreResult<()> {
        search::remove_note_tag(&self.lock(), note_id, tag, now_ms)
    }

    fn list_tags(&self) -> CoreResult<Vec<String>> {
        tags::list_tags(&self.lock())
    }

    fn rename_tag(&self, old_tag: &str, new_tag: &str, now_ms: i64) -> CoreResult<u32> {
        tags::rename_tag(&self.lock(), old_tag, new_tag, now_ms)
    }

    fn delete_tag(&self, tag: &str, now_ms: i64) -> CoreResult<u32> {
        tags::delete_tag(&self.lock(), tag, now_ms)
    }

    fn merge_tags(&self, sources: &[String], target: &str, now_ms: i64) -> CoreResult<()> {
        tags::merge_tags(&self.lock(), sources, target, now_ms)
    }

    fn create_filtered_deck(
        &self,
        name: &str,
        search: &str,
        order: u8,
        limit: u32,
        today: i32,
        now_ms: i64,
    ) -> CoreResult<Deck> {
        filtered::create_filtered_deck(&self.lock(), name, search, order, limit, today, now_ms)
    }

    fn rebuild_filtered(&self, deck_id: i64, today: i32, now_ms: i64) -> CoreResult<u32> {
        filtered::rebuild_filtered(&self.lock(), deck_id, today, now_ms)
    }

    fn empty_filtered(&self, deck_id: i64, now_ms: i64) -> CoreResult<()> {
        filtered::empty_filtered(&self.lock(), deck_id, now_ms)
    }

    fn get_filtered_config(&self, deck_id: i64) -> CoreResult<Option<FilteredDeckConfig>> {
        filtered::get_config(&self.lock(), deck_id)
    }

    fn integrity_check(&self) -> CoreResult<Vec<String>> {
        backup::integrity_check(&self.lock())
    }

    fn optimize(&self) -> CoreResult<()> {
        backup::optimize(&self.lock())
    }

    fn note_media_refs(&self) -> CoreResult<Vec<String>> {
        backup::note_media_refs(&self.lock())
    }

    fn backup_db(&self, dest_path: &std::path::Path) -> CoreResult<()> {
        backup::backup_db(&self.lock(), dest_path)
    }

    fn revlogs_for_optimize(
        &self,
        deck_id: Option<i64>,
    ) -> CoreResult<Vec<synapse_core::model::Revlog>> {
        stats::revlogs_for_optimize(&self.lock(), deck_id)
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
        assert_eq!(s.schema_version().unwrap(), 2);
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

    #[test]
    fn remove_deck_cascades_to_cards_and_subdecks_then_undoes() {
        use synapse_core::model::{CanonicalModel, Card, Deck, Field, Note, Notetype, Template};

        let count = |s: &SqliteStorage, sql: &str| -> i64 {
            s.lock().query_row(sql, [], |r| r.get(0)).unwrap()
        };
        let card_count = |s: &SqliteStorage| count(s, "SELECT COUNT(*) FROM cards");
        let note_count = |s: &SqliteStorage| count(s, "SELECT COUNT(*) FROM notes");
        let card = |note_id: i64, deck_id: i64, ord: i64| Card {
            id: note_id * 10 + ord, // overwritten on import; just unique here
            note_id,
            deck_id,
            ord,
            mod_ms: 0,
            usn: -1,
            ctype: 2,
            queue: 2,
            due: 0,
            interval: 5,
            ease_factor: 2500,
            reps: 1,
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
        let note = |id: i64, guid: &str| Note {
            id,
            guid: guid.into(),
            notetype_id: 10,
            mod_ms: 0,
            usn: -1,
            tags: vec![],
            fields: vec!["q".into(), "a".into()],
            sort_field: Some("q".into()),
            checksum: None,
        };

        let s = storage();
        s.ensure_collection(1_700_000_000_000).unwrap();
        // Deck "Med" + sub-deck "Med::Anatomy". note 100 lives only in Med,
        // note 101 only in the sub-deck, note 102 has a card in Med AND one in
        // the seeded "Default" deck — so it must SURVIVE the delete.
        s.import(&CanonicalModel {
            decks: vec![
                Deck {
                    id: 2,
                    name: "Med".into(),
                    parent_id: None,
                    config_id: 1,
                    mod_ms: 0,
                    usn: -1,
                    collapsed: false,
                    is_filtered: false,
                },
                Deck {
                    id: 3,
                    name: "Med::Anatomy".into(),
                    parent_id: Some(2),
                    config_id: 1,
                    mod_ms: 0,
                    usn: -1,
                    collapsed: false,
                    is_filtered: false,
                },
            ],
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
            notes: vec![note(100, "g1"), note(101, "g2"), note(102, "g3")],
            cards: vec![
                card(100, 2, 0),
                card(101, 3, 0),
                card(102, 2, 0),
                card(102, 1, 1),
            ],
            ..Default::default()
        })
        .unwrap();
        assert_eq!((card_count(&s), note_count(&s)), (4, 3));

        let med = s.deck_by_name("Med").unwrap().unwrap();
        let removed = s.remove_deck(med.id).unwrap();

        // Both decks, the 3 cards under them, and the 2 orphaned notes captured.
        assert_eq!(removed.decks.len(), 2);
        assert_eq!(removed.cards.len(), 3);
        assert_eq!(removed.notes.len(), 2);
        assert!(s.deck_by_name("Med").unwrap().is_none());
        assert!(s.deck_by_name("Med::Anatomy").unwrap().is_none());
        // note 102 survives (still has a card in Default); the other two are gone.
        assert_eq!((card_count(&s), note_count(&s)), (1, 1));

        // Undo restores the whole subtree and clears every tombstone.
        s.restore_deck(&removed).unwrap();
        assert!(s.deck_by_name("Med").unwrap().is_some());
        assert!(s.deck_by_name("Med::Anatomy").unwrap().is_some());
        assert_eq!((card_count(&s), note_count(&s)), (4, 3));
        let graves: i64 = s
            .lock()
            .query_row("SELECT COUNT(*) FROM graves", [], |r| r.get(0))
            .unwrap();
        assert_eq!(graves, 0);
    }
}
