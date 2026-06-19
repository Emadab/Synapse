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
use crate::model::{CanonicalModel, Deck, ImportSummary, Revlog, StudyCard};
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

    /// The next card to study in a deck, if any.
    pub fn next_card(&self, deck_id: i64) -> CoreResult<Option<StudyCard>> {
        match self.storage.due_card_ids(deck_id, self.today())?.first() {
            Some(&id) => self.storage.study_card(id),
            None => Ok(None),
        }
    }

    /// Render inputs + scheduling state for a specific card.
    pub fn study_card(&self, card_id: i64) -> CoreResult<Option<StudyCard>> {
        self.storage.study_card(card_id)
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
        fn due_card_ids(&self, _deck_id: i64, _today: i32) -> CoreResult<Vec<i64>> {
            Ok(vec![])
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
