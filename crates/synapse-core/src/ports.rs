//! Ports — the traits outer layers implement (hexagonal / ports-and-adapters).
//!
//! `synapse-core` depends on *none* of the implementations. `synapse-db`
//! implements [`Storage`], `synapse-media` implements [`MediaStore`],
//! `synapse-scheduler` implements [`Scheduler`], and so on. Tests inject fakes
//! (notably [`FixedClock`]) so behaviour is deterministic.

use std::collections::HashMap;

use crate::error::CoreResult;
use crate::ipc::{NoteDetail, NoteOverview, StatsDto};
use crate::model::{CanonicalModel, Deck, ImportSummary, NoteIndexRow, Revlog, StudyCard};
use crate::scheduling::CardState;

/// Source of "now", injectable so scheduler tests are deterministic across the
/// day cutoff and time zones. The engine must never read the wall clock except
/// through this port.
pub trait Clock: Send + Sync {
    /// Milliseconds since the Unix epoch.
    fn now_ms(&self) -> i64;
}

/// Real system clock used by the application at runtime.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}

/// Deterministic clock for tests.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.0
    }
}

/// Transactional persistence for the collection. Implemented by `synapse-db`
/// over SQLite. The method set grows per milestone; M1 covers schema access and
/// deck CRUD (the vertical slice that proves the stack end to end).
///
/// Methods take `&self`: the implementation owns interior mutability (a
/// `Mutex<Connection>`), which keeps the trait object `Send + Sync` and the
/// application layer free of borrow-checker juggling.
pub trait Storage: Send + Sync {
    /// The database `user_version` (our migration revision).
    fn schema_version(&self) -> CoreResult<i64>;

    /// Insert a new deck with the given full name; returns the stored row.
    fn create_deck(&self, name: &str, now_ms: i64) -> CoreResult<Deck>;
    fn deck_by_id(&self, id: i64) -> CoreResult<Option<Deck>>;
    fn deck_by_name(&self, name: &str) -> CoreResult<Option<Deck>>;
    fn list_decks(&self) -> CoreResult<Vec<Deck>>;
    fn rename_deck(&self, id: i64, name: &str, now_ms: i64) -> CoreResult<()>;
    fn remove_deck(&self, id: i64) -> CoreResult<()>;
    /// Re-insert a deck verbatim (used to undo a deletion).
    fn insert_deck(&self, deck: &Deck) -> CoreResult<()>;

    /// Merge a parsed collection into this one in a single transaction.
    /// Decks/notetypes match by name, notes by `guid`; see `CanonicalModel`.
    /// The `media_imported` field of the result is left at 0 — the caller
    /// (which owns the media directory) fills it in.
    fn import(&self, model: &CanonicalModel) -> CoreResult<ImportSummary>;

    /// Ensure the singleton collection row exists; returns its creation time
    /// (ms), which anchors the day-number used for scheduling.
    fn ensure_collection(&self, now_ms: i64) -> CoreResult<i64>;

    /// Ids of cards in `deck_id` that are studyable on `today` (new cards, due
    /// reviews, and learning/relearning), in study order.
    fn due_card_ids(&self, deck_id: i64, today: i32) -> CoreResult<Vec<i64>>;

    /// Count of studyable cards in `deck_id` on `today` (no LIMIT).
    fn count_due(&self, deck_id: i64, today: i32) -> CoreResult<u32>;

    /// Per-deck card-type counts for all decks. Returns `(new, learning, review)`
    /// keyed by deck_id. Only decks with at least one non-suspended card appear.
    fn deck_due_counts(&self, today: i32) -> CoreResult<HashMap<i64, (u32, u32, u32)>>;

    /// Render inputs + scheduling state for one card.
    fn study_card(&self, card_id: i64) -> CoreResult<Option<StudyCard>>;

    /// Persist a card's post-answer state and append a review-log row.
    fn apply_answer(
        &self,
        card_id: i64,
        next: &CardState,
        due: i64,
        log: &Revlog,
    ) -> CoreResult<()>;

    /// List notes for the browser, optionally filtered by a substring of the
    /// fields/tags, newest first, capped at `limit`.
    fn list_notes(&self, query: Option<&str>, limit: i64) -> CoreResult<Vec<NoteOverview>>;

    /// Full note (ordered fields + tags + note-type name) for the editor.
    fn note_detail(&self, note_id: i64) -> CoreResult<Option<NoteDetail>>;

    /// Update a note's field values (in order) and tags.
    fn update_note(
        &self,
        note_id: i64,
        fields: &[String],
        tags: &[String],
        now_ms: i64,
    ) -> CoreResult<()>;

    /// Aggregate statistics (review history, forecast, card maturity).
    fn stats(&self, today: i32, now_ms: i64) -> CoreResult<StatsDto>;

    /// All notes flattened for (re)building the search index.
    fn index_rows(&self) -> CoreResult<Vec<NoteIndexRow>>;

    /// Browser rows for a set of note ids (e.g. search hits), any order.
    fn notes_by_ids(&self, ids: &[i64]) -> CoreResult<Vec<NoteOverview>>;

    /// Dump the full collection as a `CanonicalModel` for export.
    fn dump_collection(&self) -> CoreResult<CanonicalModel>;
}

/// On-disk media store (checksums, dedup, cleanup). Implemented by
/// `synapse-media`. Fleshed out in the media milestone.
pub trait MediaStore: Send + Sync {}

/// Network sync. Architected now, implemented post-MVP. The local change-log
/// (`usn`/`mod`/`graves`) keeps the collection sync-ready in the meantime.
pub trait SyncProvider: Send + Sync {}

// The `Scheduler` port lives in `crate::scheduling` alongside its value types.
