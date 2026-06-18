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
use crate::model::Deck;
use crate::ports::{Clock, Storage};
use crate::undo::UndoLog;

pub struct Collection {
    storage: Box<dyn Storage>,
    clock: Arc<dyn Clock>,
    events: Arc<EventBus>,
    undo: Mutex<UndoLog>,
}

impl Collection {
    /// Build a collection over the given storage + clock. Emits
    /// [`DomainEvent::CollectionOpened`].
    pub fn new(storage: Box<dyn Storage>, clock: Arc<dyn Clock>) -> Self {
        let collection = Self {
            storage,
            clock,
            events: Arc::new(EventBus::new()),
            undo: Mutex::new(UndoLog::default()),
        };
        collection.events.emit(DomainEvent::CollectionOpened);
        collection
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
    fn undo_restores_deleted_deck() {
        let c = collection();
        let deck = c.create_deck("Spanish").unwrap();
        c.remove_deck(deck.id).unwrap();
        assert!(c.list_decks().unwrap().is_empty());
        c.undo().unwrap();
        assert_eq!(c.list_decks().unwrap()[0].name, "Spanish");
    }
}
