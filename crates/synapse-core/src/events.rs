//! Typed domain events.
//!
//! Mutations in the application layer emit a [`DomainEvent`] *after* their
//! transaction commits. Subscribers include the search-index updater, the stats
//! cache invalidator, the plugin host, and a Tauri bridge that re-emits events
//! to the webview so the UI can invalidate its query cache. The full dispatch
//! bus arrives in M1; this is the stable event vocabulary it will carry.

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

/// Anything that can receive domain events. The concrete in-process bus and the
/// Tauri bridge both implement this in later milestones.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: DomainEvent);
}
