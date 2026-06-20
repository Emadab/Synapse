//! The `Collection` aggregate — the application layer's entry point.
//!
//! It owns the [`Storage`] port, a [`Clock`], the [`EventBus`] and the
//! [`UndoLog`], and exposes use-cases (create/rename/remove deck, undo). Each
//! mutating use-case: validates input, calls storage, records an undo step, and
//! emits a domain event. The shell (Tauri) constructs it with a concrete
//! storage + clock and never reaches past these methods.

use std::sync::{Arc, Mutex};

use crate::error::{CoreError, CoreResult};
use crate::events::{DomainEvent, EventBus, EventSink};
use crate::ipc::{NoteDetail, NoteOverview, StatsDto};
use crate::model::{CanonicalModel, Deck, ImportSummary, NoteIndexRow, Revlog, StudyCard};
use crate::ports::{Clock, Storage};
use crate::scheduling::CardState;
use crate::undo::UndoLog;

const MS_PER_DAY: i64 = 86_400_000;

pub struct Collection {
    storage: Box<dyn Storage>,
    clock: Arc<dyn Clock>,
    events: Arc<EventBus>,
    undo: Mutex<UndoLog>,
    /// Collection creation time (ms); anchors the scheduling day-number.
    created_ms: i64,
}

impl Collection {
    /// Build a collection over the given storage + clock. Ensures the
    /// collection row exists and emits [`DomainEvent::CollectionOpened`].
    pub fn new(storage: Box<dyn Storage>, clock: Arc<dyn Clock>) -> Self {
        let created_ms = storage.ensure_collection(clock.now_ms()).unwrap_or(0);
        let collection = Self {
            storage,
            clock,
            events: Arc::new(EventBus::new()),
            undo: Mutex::new(UndoLog::default()),
            created_ms,
        };
        collection.events.emit(DomainEvent::CollectionOpened);
        collection
    }

    /// Today's day-number (days since collection creation).
    pub fn today(&self) -> i32 {
        ((self.clock.now_ms() - self.created_ms) / MS_PER_DAY) as i32
    }

    /// Current wall-clock time in ms (from the injected clock).
    pub fn now_ms(&self) -> i64 {
        self.clock.now_ms()
    }

    /// Remaining daily limits `(new, review)` for `deck_id` after subtracting
    /// cards already studied today.
    fn remaining_limits(&self, deck_id: i64) -> CoreResult<(u32, u32)> {
        let deck = self
            .storage
            .deck_by_id(deck_id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {deck_id}")))?;
        let (new_per_day, rev_per_day) = self.storage.deck_limits(deck.config_id)?;
        let today_start_ms = self.created_ms + i64::from(self.today()) * MS_PER_DAY;
        let (new_studied, rev_studied) = self.storage.today_studied(deck_id, today_start_ms)?;
        Ok((
            new_per_day.saturating_sub(new_studied),
            rev_per_day.saturating_sub(rev_studied),
        ))
    }

    /// Decks with per-type card counts `(new, learning, review)`, capped by daily limits.
    pub fn list_decks_with_counts(&self) -> CoreResult<Vec<(Deck, (u32, u32, u32))>> {
        let decks = self.storage.list_decks()?;
        let raw_counts = self.storage.deck_due_counts(self.today())?;
        let all_limits = self.storage.all_deck_limits()?;
        let today_start_ms = self.created_ms + i64::from(self.today()) * MS_PER_DAY;
        let studied = self.storage.all_today_studied(today_start_ms)?;
        Ok(decks
            .into_iter()
            .map(|d| {
                let (new_raw, learning, review_raw) =
                    raw_counts.get(&d.id).copied().unwrap_or((0, 0, 0));
                let (new_per_day, rev_per_day) =
                    all_limits.get(&d.config_id).copied().unwrap_or((20, 200));
                let (new_studied, rev_studied) =
                    studied.get(&d.id).copied().unwrap_or((0, 0));
                let capped = (
                    new_raw.min(new_per_day.saturating_sub(new_studied)),
                    learning,
                    review_raw.min(rev_per_day.saturating_sub(rev_studied)),
                );
                (d, capped)
            })
            .collect())
    }

    /// Count of studyable cards in `deck_id` right now (respects daily limits).
    pub fn count_due(&self, deck_id: i64) -> CoreResult<u32> {
        let (new_limit, review_limit) = self.remaining_limits(deck_id)?;
        self.storage.count_due(deck_id, self.today(), new_limit, review_limit)
    }

    /// The next card to study in a deck, if any (respects daily limits).
    pub fn next_card(&self, deck_id: i64) -> CoreResult<Option<StudyCard>> {
        let (new_limit, review_limit) = self.remaining_limits(deck_id)?;
        match self
            .storage
            .due_card_ids(deck_id, self.today(), new_limit, review_limit)?
            .first()
        {
            Some(&id) => self.storage.study_card(id),
            None => Ok(None),
        }
    }

    /// Current `(new_per_day, review_per_day)` limit for a deck.
    pub fn get_deck_options(&self, deck_id: i64) -> CoreResult<(u32, u32)> {
        let deck = self
            .storage
            .deck_by_id(deck_id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {deck_id}")))?;
        self.storage.deck_limits(deck.config_id)
    }

    /// Persist updated daily limits for a deck.
    pub fn set_deck_options(
        &self,
        deck_id: i64,
        new_per_day: u32,
        rev_per_day: u32,
    ) -> CoreResult<()> {
        let deck = self
            .storage
            .deck_by_id(deck_id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {deck_id}")))?;
        self.storage
            .set_deck_limits(deck.config_id, new_per_day, rev_per_day, self.now_ms())
    }

    /// Render inputs + scheduling state for a specific card.
    pub fn study_card(&self, card_id: i64) -> CoreResult<Option<StudyCard>> {
        self.storage.study_card(card_id)
    }

    /// Notes for the browser, optionally filtered by a substring.
    pub fn list_notes(&self, query: Option<&str>) -> CoreResult<Vec<NoteOverview>> {
        self.storage.list_notes(query, 1000)
    }

    /// Full note for the editor.
    pub fn note_detail(&self, note_id: i64) -> CoreResult<Option<NoteDetail>> {
        self.storage.note_detail(note_id)
    }

    /// Save edited note field values + tags.
    pub fn update_note(&self, note_id: i64, fields: &[String], tags: &[String]) -> CoreResult<()> {
        self.storage
            .update_note(note_id, fields, tags, self.clock.now_ms())?;
        self.events.emit(DomainEvent::NoteUpdated { note_id });
        Ok(())
    }

    /// Aggregate statistics for the dashboards.
    pub fn stats(&self) -> CoreResult<StatsDto> {
        self.storage.stats(self.today(), self.clock.now_ms())
    }

    /// All notes flattened for (re)building the search index.
    pub fn index_rows(&self) -> CoreResult<Vec<NoteIndexRow>> {
        self.storage.index_rows()
    }

    /// Dump the full collection for export (`.apkg`/`.colpkg`).
    pub fn dump_collection(&self) -> CoreResult<CanonicalModel> {
        self.storage.dump_collection()
    }

    /// Browser rows for a set of note ids (search hits).
    pub fn notes_by_ids(&self, ids: &[i64]) -> CoreResult<Vec<NoteOverview>> {
        self.storage.notes_by_ids(ids)
    }

    /// Persist an answered card's new state + review log, then notify.
    pub fn apply_answer(
        &self,
        card_id: i64,
        next: &CardState,
        due: i64,
        log: &Revlog,
    ) -> CoreResult<()> {
        self.storage.apply_answer(card_id, next, due, log)?;
        self.events.emit(DomainEvent::CardAnswered { card_id });
        Ok(())
    }

    /// Shared handle to the event bus, for wiring external subscribers
    /// (e.g. the Tauri → webview bridge).
    pub fn events(&self) -> Arc<EventBus> {
        self.events.clone()
    }

    pub fn schema_version(&self) -> CoreResult<i64> {
        self.storage.schema_version()
    }

    pub fn list_decks(&self) -> CoreResult<Vec<Deck>> {
        self.storage.list_decks()
    }

    pub fn create_deck(&self, name: &str) -> CoreResult<Deck> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::Invalid("deck name is empty".into()));
        }
        if self.storage.deck_by_name(name)?.is_some() {
            return Err(CoreError::Invalid(format!(
                "a deck named \"{name}\" already exists"
            )));
        }
        let deck = self.storage.create_deck(name, self.clock.now_ms())?;
        let id = deck.id;
        self.record_undo(format!("Create deck \"{name}\""), move |s, _now| {
            s.remove_deck(id)
        });
        self.events.emit(DomainEvent::DeckChanged { deck_id: id });
        Ok(deck)
    }

    pub fn rename_deck(&self, id: i64, name: &str) -> CoreResult<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::Invalid("deck name is empty".into()));
        }
        let old = self
            .storage
            .deck_by_id(id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {id}")))?;
        self.storage.rename_deck(id, name, self.clock.now_ms())?;
        let old_name = old.name;
        self.record_undo(format!("Rename deck to \"{name}\""), move |s, now| {
            s.rename_deck(id, &old_name, now)
        });
        self.events.emit(DomainEvent::DeckChanged { deck_id: id });
        Ok(())
    }

    pub fn remove_deck(&self, id: i64) -> CoreResult<()> {
        let deck = self
            .storage
            .deck_by_id(id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {id}")))?;
        self.storage.remove_deck(id)?;
        let description = format!("Delete deck \"{}\"", deck.name);
        self.record_undo(description, move |s, _now| s.insert_deck(&deck));
        self.events.emit(DomainEvent::DeckChanged { deck_id: id });
        Ok(())
    }

    /// Merge a parsed package (from `synapse-ankifmt`) into this collection.
    /// Import is not undoable via the per-op log (it is bulk and transactional);
    /// the pre-import backup is the recovery path (added in a later milestone).
    pub fn import(&self, model: &CanonicalModel) -> CoreResult<ImportSummary> {
        let summary = self.storage.import(model)?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(summary)
    }

    /// Description of the next undoable operation, if any.
    pub fn undo_status(&self) -> Option<String> {
        self.undo.lock().unwrap().peek().map(str::to_owned)
    }

    /// Undo the most recent operation; returns its description.
    pub fn undo(&self) -> CoreResult<Option<String>> {
        let step = self.undo.lock().unwrap().pop();
        match step {
            None => Ok(None),
            Some(step) => {
                let description = step.description.clone();
                step.run(self.storage.as_ref(), self.clock.now_ms())?;
                self.events.emit(DomainEvent::SchemaChanged);
                Ok(Some(description))
            }
        }
    }

    fn record_undo(
        &self,
        description: impl Into<String>,
        action: impl FnOnce(&dyn Storage, i64) -> CoreResult<()> + Send + 'static,
    ) {
        self.undo
            .lock()
            .unwrap()
            .record(description, Box::new(action));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::FixedClock;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Minimal in-memory `Storage` fake so the application layer can be tested
    /// without `synapse-db`. (synapse-db has its own SQLite-backed tests.)
    #[derive(Default)]
    struct FakeStorage {
        decks: Mutex<Vec<Deck>>,
        next_id: AtomicUsize,
    }

    impl FakeStorage {
        fn new_id(&self) -> i64 {
            self.next_id.fetch_add(1, Ordering::SeqCst) as i64 + 1
        }
    }

    impl Storage for FakeStorage {
        fn schema_version(&self) -> CoreResult<i64> {
            Ok(1)
        }
        fn create_deck(&self, name: &str, now_ms: i64) -> CoreResult<Deck> {
            let deck = Deck {
                id: self.new_id(),
                name: name.to_string(),
                parent_id: None,
                config_id: 1,
                mod_ms: now_ms,
                usn: -1,
                collapsed: false,
                is_filtered: false,
            };
            self.decks.lock().unwrap().push(deck.clone());
            Ok(deck)
        }
        fn deck_by_id(&self, id: i64) -> CoreResult<Option<Deck>> {
            Ok(self
                .decks
                .lock()
                .unwrap()
                .iter()
                .find(|d| d.id == id)
                .cloned())
        }
        fn deck_by_name(&self, name: &str) -> CoreResult<Option<Deck>> {
            Ok(self
                .decks
                .lock()
                .unwrap()
                .iter()
                .find(|d| d.name == name)
                .cloned())
        }
        fn list_decks(&self) -> CoreResult<Vec<Deck>> {
            Ok(self.decks.lock().unwrap().clone())
        }
        fn rename_deck(&self, id: i64, name: &str, now_ms: i64) -> CoreResult<()> {
            let mut decks = self.decks.lock().unwrap();
            let deck = decks
                .iter_mut()
                .find(|d| d.id == id)
                .ok_or(CoreError::NotFound("deck".into()))?;
            deck.name = name.to_string();
            deck.mod_ms = now_ms;
            Ok(())
        }
        fn remove_deck(&self, id: i64) -> CoreResult<()> {
            self.decks.lock().unwrap().retain(|d| d.id != id);
            Ok(())
        }
        fn insert_deck(&self, deck: &Deck) -> CoreResult<()> {
            self.decks.lock().unwrap().push(deck.clone());
            Ok(())
        }
        fn import(&self, model: &CanonicalModel) -> CoreResult<ImportSummary> {
            let mut summary = ImportSummary::default();
            for deck in &model.decks {
                if self.deck_by_name(&deck.name)?.is_none() {
                    self.create_deck(&deck.name, deck.mod_ms)?;
                    summary.decks_added += 1;
                }
            }
            summary.notes_added = model.notes.len() as u32;
            Ok(summary)
        }
        fn ensure_collection(&self, _now_ms: i64) -> CoreResult<i64> {
            Ok(0)
        }
        fn due_card_ids(
            &self,
            _deck_id: i64,
            _today: i32,
            _new_limit: u32,
            _review_limit: u32,
        ) -> CoreResult<Vec<i64>> {
            Ok(vec![])
        }
        fn count_due(
            &self,
            _deck_id: i64,
            _today: i32,
            _new_limit: u32,
            _review_limit: u32,
        ) -> CoreResult<u32> {
            Ok(0)
        }
        fn deck_due_counts(
            &self,
            _today: i32,
        ) -> CoreResult<std::collections::HashMap<i64, (u32, u32, u32)>> {
            Ok(std::collections::HashMap::new())
        }
        fn deck_limits(&self, _config_id: i64) -> CoreResult<(u32, u32)> {
            Ok((20, 200))
        }
        fn all_deck_limits(&self) -> CoreResult<std::collections::HashMap<i64, (u32, u32)>> {
            Ok(std::collections::HashMap::new())
        }
        fn today_studied(&self, _deck_id: i64, _today_start_ms: i64) -> CoreResult<(u32, u32)> {
            Ok((0, 0))
        }
        fn all_today_studied(
            &self,
            _today_start_ms: i64,
        ) -> CoreResult<std::collections::HashMap<i64, (u32, u32)>> {
            Ok(std::collections::HashMap::new())
        }
        fn set_deck_limits(
            &self,
            _config_id: i64,
            _new_per_day: u32,
            _rev_per_day: u32,
            _now_ms: i64,
        ) -> CoreResult<()> {
            Ok(())
        }
        fn study_card(&self, _card_id: i64) -> CoreResult<Option<StudyCard>> {
            Ok(None)
        }
        fn apply_answer(
            &self,
            _card_id: i64,
            _next: &CardState,
            _due: i64,
            _log: &Revlog,
        ) -> CoreResult<()> {
            Ok(())
        }
        fn list_notes(&self, _query: Option<&str>, _limit: i64) -> CoreResult<Vec<NoteOverview>> {
            Ok(vec![])
        }
        fn note_detail(&self, _note_id: i64) -> CoreResult<Option<NoteDetail>> {
            Ok(None)
        }
        fn update_note(
            &self,
            _note_id: i64,
            _fields: &[String],
            _tags: &[String],
            _now_ms: i64,
        ) -> CoreResult<()> {
            Ok(())
        }
        fn stats(&self, _today: i32, _now_ms: i64) -> CoreResult<crate::ipc::StatsDto> {
            Ok(crate::ipc::StatsDto::default())
        }
        fn index_rows(&self) -> CoreResult<Vec<NoteIndexRow>> {
            Ok(vec![])
        }
        fn notes_by_ids(&self, _ids: &[i64]) -> CoreResult<Vec<NoteOverview>> {
            Ok(vec![])
        }
        fn dump_collection(&self) -> CoreResult<CanonicalModel> {
            Ok(CanonicalModel::default())
        }
    }

    fn collection() -> Collection {
        Collection::new(
            Box::new(FakeStorage::default()),
            Arc::new(FixedClock(1_000)),
        )
    }

    #[test]
    fn create_then_list() {
        let c = collection();
        c.create_deck("Spanish").unwrap();
        let decks = c.list_decks().unwrap();
        assert_eq!(decks.len(), 1);
        assert_eq!(decks[0].name, "Spanish");
    }

    #[test]
    fn rejects_blank_and_duplicate_names() {
        let c = collection();
        c.create_deck("Spanish").unwrap();
        assert!(c.create_deck("   ").is_err());
        assert!(c.create_deck("Spanish").is_err());
    }

    #[test]
    fn undo_reverses_create_rename_delete() {
        let c = collection();
        let deck = c.create_deck("Spanish").unwrap();

        c.rename_deck(deck.id, "Español").unwrap();
        assert_eq!(c.list_decks().unwrap()[0].name, "Español");

        // undo rename
        assert_eq!(
            c.undo().unwrap().as_deref(),
            Some("Rename deck to \"Español\"")
        );
        assert_eq!(c.list_decks().unwrap()[0].name, "Spanish");

        // undo create
        assert!(c.undo().unwrap().is_some());
        assert!(c.list_decks().unwrap().is_empty());

        // nothing left
        assert!(c.undo().unwrap().is_none());
    }

    #[test]
    fn import_creates_decks_and_counts_notes() {
        let c = collection();
        let model = CanonicalModel {
            decks: vec![Deck {
                id: 5,
                name: "Imported".into(),
                parent_id: None,
                config_id: 1,
                mod_ms: 0,
                usn: -1,
                collapsed: false,
                is_filtered: false,
            }],
            notes: vec![crate::model::Note {
                id: 1700000000000,
                guid: "abc".into(),
                notetype_id: 1,
                mod_ms: 0,
                usn: -1,
                tags: vec![],
                fields: vec!["Front".into(), "Back".into()],
                sort_field: Some("Front".into()),
                checksum: None,
            }],
            ..Default::default()
        };
        let summary = c.import(&model).unwrap();
        assert_eq!(summary.decks_added, 1);
        assert_eq!(summary.notes_added, 1);
        assert!(c.list_decks().unwrap().iter().any(|d| d.name == "Imported"));
    }

    #[test]
    fn undo_restores_deleted_deck() {
        let c = collection();
        let deck = c.create_deck("Spanish").unwrap();
        c.remove_deck(deck.id).unwrap();
        assert!(c.list_decks().unwrap().is_empty());
        c.undo().unwrap();
        assert_eq!(c.list_decks().unwrap()[0].name, "Spanish");
    }
}
