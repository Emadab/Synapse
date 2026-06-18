//! Typed domain events + a minimal in-process dispatch bus.
//!
//! Mutations in the application layer emit a [`DomainEvent`] *after* their
//! transaction commits. Subscribers include the search-index updater, the stats
//! cache invalidator, the plugin host, and a Tauri bridge that re-emits events
//! to the webview so the UI can invalidate its query cache.

use std::sync::Mutex;

/// A fact about something that has already happened to the collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEvent {
    CollectionOpened,
    CollectionClosed,
    SchemaChanged,
    CardAnswered { card_id: i64 },
    NoteAdded { note_id: i64 },
    NoteUpdated { note_id: i64 },
    NoteRemoved { note_id: i64 },
    DeckChanged { deck_id: i64 },
    MediaChanged,
}

/// Anything that can receive domain events.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: DomainEvent);
}

type Listener = Box<dyn Fn(&DomainEvent) + Send + Sync + 'static>;

/// Synchronous fan-out event bus. Listeners are invoked in registration order
/// on the thread that emits. Cheap and deterministic — good enough until a
/// milestone needs async or ordering guarantees.
#[derive(Default)]
pub struct EventBus {
    listeners: Mutex<Vec<Listener>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a listener for all events.
    pub fn subscribe(&self, listener: impl Fn(&DomainEvent) + Send + Sync + 'static) {
        self.listeners.lock().unwrap().push(Box::new(listener));
    }
}

impl EventSink for EventBus {
    fn emit(&self, event: DomainEvent) {
        let listeners = self.listeners.lock().unwrap();
        for listener in listeners.iter() {
            listener(&event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn fans_out_to_subscribers() {
        let bus = EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        bus.emit(DomainEvent::CollectionOpened);
        bus.emit(DomainEvent::DeckChanged { deck_id: 1 });
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
}
