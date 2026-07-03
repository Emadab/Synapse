//! Ports — the traits outer layers implement (hexagonal / ports-and-adapters).
//!
//! `synapse-core` depends on *none* of the implementations. `synapse-db`
//! implements [`Storage`], `synapse-media` implements [`MediaStore`],
//! `synapse-scheduler` implements [`Scheduler`], and so on. Tests inject fakes
//! (notably [`FixedClock`]) so behaviour is deterministic.

use std::collections::HashMap;

use crate::error::CoreResult;
use crate::ipc::{
    CardRow, FieldRemoveWarning, FilteredDeckConfig, NoteDetail, NoteOverview, NotetypeDetail,
    StatsDto,
};
use crate::model::{
    CanonicalModel, Card, Deck, Field, ImportSummary, Note, NoteIndexRow, Notetype, Revlog,
    StudyCard, Template,
};
use crate::scheduling::{CardState, SchedConfig};

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

/// A deck subtree captured at deletion time, so the operation can be undone.
/// `decks` are ordered parent-first; `cards` are every card those decks held;
/// `notes` are the notes those cards left orphaned (no cards anywhere else).
#[derive(Debug, Default, Clone)]
pub struct RemovedDeck {
    pub decks: Vec<Deck>,
    pub cards: Vec<Card>,
    pub notes: Vec<Note>,
}

/// The studyable cards of a deck, split into their three streams plus a single
/// learn-ahead fallback. [`Storage::study_queue`] gates each stream by due time;
/// [`crate::Collection::next_card`] decides the order (learning first, then a
/// proportional new/review interleave, then learn-ahead).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StudyQueue {
    /// Learning/relearning cards due now, ordered by due (soonest first).
    pub learning: Vec<i64>,
    /// New cards in roughly frequency-rank order (lightly jittered), capped by
    /// the remaining daily new limit.
    pub new: Vec<i64>,
    /// Review cards due today (random order), capped by the remaining limit.
    pub review: Vec<i64>,
    /// Soonest learning card due within the learn-ahead window, if any — shown
    /// only when every other stream is empty.
    pub learning_ahead: Option<i64>,
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
    /// Delete a deck and everything under it — its sub-decks (`name::*`) and all
    /// their cards plus those cards' review-log rows — in one transaction, and
    /// tombstone each in `graves`. Returns the deleted rows so the caller can
    /// undo the deletion via [`Storage::restore_deck`].
    fn remove_deck(&self, id: i64) -> CoreResult<RemovedDeck>;
    /// Restore a previously [removed](Storage::remove_deck) deck subtree (decks
    /// parent-first, then cards), clearing the tombstones. Inverse of `remove_deck`.
    fn restore_deck(&self, removed: &RemovedDeck) -> CoreResult<()>;
    /// Re-insert a single deck verbatim (used to undo a rename/delete of an
    /// empty deck).
    fn insert_deck(&self, deck: &Deck) -> CoreResult<()>;

    /// Merge a parsed collection into this one in a single transaction.
    /// Decks/notetypes match by name, notes by `guid`; see `CanonicalModel`.
    /// The `media_imported` field of the result is left at 0 — the caller
    /// (which owns the media directory) fills it in.
    fn import(&self, model: &CanonicalModel) -> CoreResult<ImportSummary>;

    /// Same as [`Storage::import`], but calls `on_progress(done, total)`
    /// periodically as notes/cards are merged, for long-running imports.
    /// Default implementation ignores progress and defers to `import`.
    fn import_with_progress(
        &self,
        model: &CanonicalModel,
        on_progress: &mut dyn FnMut(u32, u32),
    ) -> CoreResult<ImportSummary> {
        let _ = on_progress;
        self.import(model)
    }

    /// Ensure the singleton collection row exists; returns its creation time
    /// (ms), which anchors the day-number used for scheduling.
    fn ensure_collection(&self, now_ms: i64) -> CoreResult<i64>;

    /// The studyable cards of `deck_id` split into streams, gated by due time
    /// (`today` for reviews, `now_ms` for learning) and capped by daily limits.
    /// `today_end_ms` is the start of tomorrow (ms); learning cards due before
    /// that time are eligible for the learn-ahead fallback when nothing else is due.
    /// The caller assembles the final order from the [`StudyQueue`].
    fn study_queue(
        &self,
        deck_id: i64,
        today: i32,
        now_ms: i64,
        today_end_ms: i64,
        new_limit: u32,
        review_limit: u32,
    ) -> CoreResult<StudyQueue>;

    /// Count of studyable cards by type `(new, learning, review)`, capped by limits.
    /// `today_end_ms` gates the learning count: cards due before midnight count even
    /// if not yet due right now.
    fn count_due_by_type(
        &self,
        deck_id: i64,
        today: i32,
        now_ms: i64,
        today_end_ms: i64,
        new_limit: u32,
        review_limit: u32,
    ) -> CoreResult<(u32, u32, u32)>;

    /// Total count of studyable cards in `deck_id` on `today`, capped by limits.
    fn count_due(
        &self,
        deck_id: i64,
        today: i32,
        now_ms: i64,
        today_end_ms: i64,
        new_limit: u32,
        review_limit: u32,
    ) -> CoreResult<u32>;

    /// Per-deck card-type counts (raw, pre-limit). Keyed by deck_id.
    /// `today_end_ms` gates the learning count (cards due before midnight).
    fn deck_due_counts(
        &self,
        today: i32,
        now_ms: i64,
        today_end_ms: i64,
    ) -> CoreResult<HashMap<i64, (u32, u32, u32)>>;

    /// `due` for a set of card ids, keyed by id. Used to merge several decks'
    /// learning streams into one soonest-first order when studying a deck
    /// with subdecks.
    fn cards_due_ms(&self, card_ids: &[i64]) -> CoreResult<HashMap<i64, i64>>;

    /// `(new_per_day, rev_per_day)` from the deck config's JSON.
    fn deck_limits(&self, config_id: i64) -> CoreResult<(u32, u32)>;

    /// `(new_per_day, rev_per_day)` for every config row. Keyed by config_id.
    fn all_deck_limits(&self) -> CoreResult<HashMap<i64, (u32, u32)>>;

    /// Cards from `deck_id` studied today, split by type. `today_start_ms` is the
    /// epoch-ms start of the current scheduling day.
    fn today_studied(&self, deck_id: i64, today_start_ms: i64) -> CoreResult<(u32, u32)>;

    /// Today-studied counts for all decks in one query. Keyed by deck_id.
    fn all_today_studied(&self, today_start_ms: i64) -> CoreResult<HashMap<i64, (u32, u32)>>;

    /// Persist updated `new_per_day` / `rev_per_day` in the config JSON.
    fn set_deck_limits(
        &self,
        config_id: i64,
        new_per_day: u32,
        rev_per_day: u32,
        now_ms: i64,
    ) -> CoreResult<()>;

    /// Read full scheduling config for a `deck_config` row.
    fn get_deck_config(&self, config_id: i64) -> CoreResult<SchedConfig>;

    /// Persist full scheduling config for a `deck_config` row.
    fn set_deck_config(&self, config_id: i64, config: &SchedConfig, now_ms: i64) -> CoreResult<()>;

    /// Configured day-rollover hour (0-23, local time; Anki default is 4am),
    /// read from the `collection.config` JSON blob.
    fn get_rollover_hour(&self) -> CoreResult<u8>;

    /// Persist the day-rollover hour into the `collection.config` JSON blob.
    fn set_rollover_hour(&self, hour: u8, now_ms: i64) -> CoreResult<()>;

    /// Extra new-card allowance for `deck_id` on collection-day `day` (0 if none set).
    fn day_extra_new(&self, deck_id: i64, day: i32) -> CoreResult<u32>;

    /// Extra new-card allowances for every deck on collection-day `day`, keyed by deck_id.
    fn all_day_extra_new(&self, day: i32) -> CoreResult<HashMap<i64, u32>>;

    /// Upsert the extra new-card allowance for `deck_id` on `day`.
    fn set_day_extra_new(&self, deck_id: i64, day: i32, extra_new: u32) -> CoreResult<()>;

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
    ///
    /// `deck_ids` restricts to those decks (already resolved to include subdeck
    /// rollup), or `None` for the whole collection. `days` restricts
    /// range-scoped aggregates (totals, retention, answer buttons, hourly), or
    /// `None` for all time. `tz_offset_minutes` shifts only the hourly bucketing
    /// into local time. `fsrs_weights`/`retention_goal_pct` are the relevant
    /// deck's trained FSRS weights and desired retention (or the collection
    /// defaults when no single deck is selected), used for the retrievability
    /// panel and the retention goal line.
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
    ) -> CoreResult<StatsDto>;

    /// All notes flattened for (re)building the search index.
    fn index_rows(&self) -> CoreResult<Vec<NoteIndexRow>>;

    /// Browser rows for a set of note ids (e.g. search hits), any order.
    fn notes_by_ids(&self, ids: &[i64]) -> CoreResult<Vec<NoteOverview>>;

    /// Dump the full collection as a `CanonicalModel` for export.
    fn dump_collection(&self) -> CoreResult<CanonicalModel>;

    /// All note types, ordered by name. Used to populate the Add Note picker.
    fn list_notetypes(&self) -> CoreResult<Vec<Notetype>>;

    /// Field definitions for one note type, in `ord` order.
    fn fields_for_notetype(&self, notetype_id: i64) -> CoreResult<Vec<Field>>;

    /// Template definitions for one note type, in `ord` order.
    fn templates_for_notetype(&self, notetype_id: i64) -> CoreResult<Vec<Template>>;

    /// Insert a new note and generate its cards (one per template for standard
    /// notetypes; one per cloze ordinal for cloze notetypes). Returns
    /// `(note_id, cards_added)`.
    fn add_note_with_cards(
        &self,
        notetype_id: i64,
        deck_id: i64,
        fields: &[String],
        tags: &[String],
        now_ms: i64,
    ) -> CoreResult<(i64, u32)>;

    // ── Note-type editor ──────────────────────────────────────────────────────

    /// Full detail (fields + templates) for one note type.
    fn get_notetype_detail(&self, notetype_id: i64) -> CoreResult<Option<NotetypeDetail>>;

    /// Create a new note type seeded with default fields/template. Returns the new id.
    fn create_notetype(&self, name: &str, kind: i64, now_ms: i64) -> CoreResult<i64>;

    /// Names of the built-in stock note types (Basic, Cloze, …), in a stable
    /// order matching `add_stock_notetype`'s index.
    fn stock_notetype_names(&self) -> Vec<&'static str>;

    /// Add one built-in stock note type (by index into `stock_notetype_names`)
    /// to the collection. Returns the new id.
    fn add_stock_notetype(&self, index: usize, now_ms: i64) -> CoreResult<i64>;

    /// Delete a note type. Fails with `Invalid` if any notes reference it.
    fn delete_notetype(&self, notetype_id: i64, now_ms: i64) -> CoreResult<()>;

    /// Rename a note type.
    fn rename_notetype(&self, notetype_id: i64, name: &str, now_ms: i64) -> CoreResult<()>;

    /// Save a note type's custom card CSS.
    fn save_notetype_css(&self, notetype_id: i64, css: &str, now_ms: i64) -> CoreResult<()>;

    /// Add a field at the end of `notetype_id`; appends an empty value to every
    /// existing note of that type.
    fn add_field(&self, notetype_id: i64, name: &str, now_ms: i64) -> CoreResult<()>;

    /// Count notes of `notetype_id` that have non-empty content in field `ord`.
    fn check_field_remove(&self, notetype_id: i64, ord: i64) -> CoreResult<FieldRemoveWarning>;

    /// Remove field at `ord`, shifting higher ords down by 1 and splicing the
    /// value out of every note's field blob. Fails if it would leave 0 fields.
    fn remove_field(&self, notetype_id: i64, ord: i64, now_ms: i64) -> CoreResult<()>;

    /// Rename field at `ord`.
    fn rename_field(&self, notetype_id: i64, ord: i64, name: &str, now_ms: i64) -> CoreResult<()>;

    /// Reorder fields. `new_order[i]` = old `ord` to place at new position `i`.
    /// Length must equal the current field count.
    fn reorder_fields(&self, notetype_id: i64, new_order: &[i64], now_ms: i64) -> CoreResult<()>;

    /// Add a template at the end of `notetype_id`; generates one new card per
    /// existing note (using the note's current deck).
    fn add_template(
        &self,
        notetype_id: i64,
        name: &str,
        qfmt: &str,
        afmt: &str,
        now_ms: i64,
    ) -> CoreResult<()>;

    /// Remove template at `ord`, deleting its cards and shifting higher ords
    /// down by 1. Fails if it would leave 0 templates.
    fn remove_template(&self, notetype_id: i64, ord: i64, now_ms: i64) -> CoreResult<()>;

    /// Update a template's name / front / back format strings.
    fn save_template(
        &self,
        notetype_id: i64,
        ord: i64,
        name: &str,
        qfmt: &str,
        afmt: &str,
        now_ms: i64,
    ) -> CoreResult<()>;

    // ── Card lifecycle ────────────────────────────────────────────────────────

    /// Set `queue = -1` for each card id.
    fn suspend_cards(&self, card_ids: &[i64]) -> CoreResult<()>;

    /// Restore suspended cards (`queue = -1`) to their natural `queue = type`.
    fn unsuspend_cards(&self, card_ids: &[i64]) -> CoreResult<()>;

    /// Manually bury cards (`queue = -2`).
    fn bury_cards(&self, card_ids: &[i64]) -> CoreResult<()>;

    /// Sibling bury: set `queue = -3` on all new/review cards from `note_id`
    /// other than `answered_card_id` (called immediately after answering).
    fn bury_siblings(&self, note_id: i64, answered_card_id: i64) -> CoreResult<()>;

    /// Restore all buried cards in `deck_id` (manual -2 and sibling -3) to
    /// their natural queue. Called at day rollover / session start.
    fn unbury_deck(&self, deck_id: i64) -> CoreResult<()>;

    /// Set the flag byte (0–7) on a list of cards.
    fn set_card_flag(&self, card_ids: &[i64], flag: u8) -> CoreResult<()>;

    /// Append `tag` to a note's tag blob if not already present (idempotent).
    fn add_note_tag(&self, note_id: i64, tag: &str, now_ms: i64) -> CoreResult<()>;

    // ── M16: rich search + bulk ops ───────────────────────────────────────────

    /// Anki-flavoured query → card rows. `limit` caps result count.
    fn search_cards(
        &self,
        query: &str,
        today: i32,
        now_ms: i64,
        limit: i64,
    ) -> CoreResult<Vec<CardRow>>;

    /// Delete notes (and their cards + revlogs) by id; write graves.
    fn delete_notes(&self, note_ids: &[i64], now_ms: i64) -> CoreResult<()>;

    /// Reassign cards to a different deck.
    fn move_cards_to_deck(&self, card_ids: &[i64], deck_id: i64) -> CoreResult<()>;

    /// Remove `tag` from a note's tag blob (idempotent).
    fn remove_note_tag(&self, note_id: i64, tag: &str, now_ms: i64) -> CoreResult<()>;

    // ── M17: tag manager ─────────────────────────────────────────────────────

    /// All distinct tag names from the registry, sorted alphabetically.
    fn list_tags(&self) -> CoreResult<Vec<String>>;

    /// Rename `old_tag` to `new_tag` in all note tag blobs + registry.
    /// Returns the number of notes affected.
    fn rename_tag(&self, old_tag: &str, new_tag: &str, now_ms: i64) -> CoreResult<u32>;

    /// Remove `tag` from all note tag blobs + registry. Returns notes affected.
    fn delete_tag(&self, tag: &str, now_ms: i64) -> CoreResult<u32>;

    /// Rename each tag in `sources` to `target` (merge). Idempotent.
    fn merge_tags(&self, sources: &[String], target: &str, now_ms: i64) -> CoreResult<()>;

    // ── M17: filtered decks ───────────────────────────────────────────────────

    /// Create a new filtered deck with `search` query; gather cards immediately.
    /// `order` (0=random, 1=due-date, 2=added, 3=ivl-asc, 4=lapses).
    fn create_filtered_deck(
        &self,
        name: &str,
        search: &str,
        order: u8,
        limit: u32,
        today: i32,
        now_ms: i64,
    ) -> CoreResult<Deck>;

    /// Empty then re-gather cards for an existing filtered deck. Returns card count.
    fn rebuild_filtered(&self, deck_id: i64, today: i32, now_ms: i64) -> CoreResult<u32>;

    /// Return all cards in a filtered deck to their original decks.
    fn empty_filtered(&self, deck_id: i64, now_ms: i64) -> CoreResult<()>;

    /// Filtered deck configuration (search, order, limit) for the rebuild dialog.
    fn get_filtered_config(&self, deck_id: i64) -> CoreResult<Option<FilteredDeckConfig>>;

    /// Run `PRAGMA integrity_check`. Returns empty vec when healthy.
    fn integrity_check(&self) -> CoreResult<Vec<String>>;

    /// Run `PRAGMA optimize; VACUUM` to compact and tune the database.
    fn optimize(&self) -> CoreResult<()>;

    /// Extract all media filenames referenced in note fields.
    fn note_media_refs(&self) -> CoreResult<Vec<String>>;

    /// Hot-copy the main database to `dest_path`.
    fn backup_db(&self, dest_path: &std::path::Path) -> CoreResult<()>;

    /// Revlog entries suitable for FSRS optimization: review_kind in {0,1,2},
    /// optionally scoped to cards in a specific deck.
    fn revlogs_for_optimize(&self, deck_id: Option<i64>) -> CoreResult<Vec<Revlog>>;
}

/// On-disk media store (checksums, dedup, cleanup). Implemented by
/// `synapse-media`. Fleshed out in the media milestone.
pub trait MediaStore: Send + Sync {}

/// Network sync. Architected now, implemented post-MVP. The local change-log
/// (`usn`/`mod`/`graves`) keeps the collection sync-ready in the meantime.
pub trait SyncProvider: Send + Sync {}

// The `Scheduler` port lives in `crate::scheduling` alongside its value types.
