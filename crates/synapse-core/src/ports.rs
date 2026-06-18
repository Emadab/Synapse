//! Ports — the traits outer layers implement (hexagonal / ports-and-adapters).
//!
//! `synapse-core` depends on *none* of the implementations. `synapse-db`
//! implements [`Storage`], `synapse-media` implements [`MediaStore`],
//! `synapse-scheduler` implements [`Scheduler`], and so on. Tests inject fakes
//! (notably [`FixedClock`]) so behaviour is deterministic.

use crate::error::CoreResult;
use crate::model::Deck;

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
}

/// On-disk media store (checksums, dedup, cleanup). Implemented by
/// `synapse-media`. Fleshed out in the media milestone.
pub trait MediaStore: Send + Sync {}

/// Network sync. Architected now, implemented post-MVP. The local change-log
/// (`usn`/`mod`/`graves`) keeps the collection sync-ready in the meantime.
pub trait SyncProvider: Send + Sync {}

/// Spaced-repetition scheduler. Implemented by `synapse-scheduler`
/// (SM-2 and FSRS). Defined here so the application layer depends only on the
/// trait, never on a concrete algorithm.
pub trait Scheduler: Send + Sync {
    fn algorithm(&self) -> crate::model::Algorithm;
}
