//! DTOs that cross the IPC boundary to the frontend. Every type here derives
//! [`ts_rs::TS`] with `#[ts(export)]`, so `cargo test` regenerates the matching
//! TypeScript in `packages/ipc-types/src/generated/`. The Rust definitions are
//! the single source of truth; a mismatch breaks the TS build (intended).
//!
//! 64-bit ids are annotated `#[ts(type = "number")]`: serde serialises them as
//! JSON numbers, and Anki-style ids (epoch-ms, < 2^53) are exact in JS.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::CoreError;
use crate::model::{Algorithm, Deck};

/// Basic identity of the running application, surfaced on the home screen and
/// the About page.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub tauri_version: String,
    /// Absolute path to the `collection.media` directory on disk.
    pub media_dir: String,
}

/// Full per-deck scheduling configuration for the options dialog (M14).
/// Replaces the narrow `DeckOptions` (new/rev limits only).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DeckConfig {
    #[ts(type = "number")]
    pub deck_id: i64,
    #[ts(type = "number")]
    pub config_id: i64,
    pub algorithm: Algorithm,
    // General limits
    pub new_per_day: u32,
    pub review_per_day: u32,
    // New cards
    pub learning_steps_min: Vec<u32>,
    pub graduating_interval_days: u32,
    pub easy_interval_days: u32,
    pub starting_ease_milli: u32,
    // Reviews
    pub easy_bonus: f64,
    pub hard_interval_factor: f64,
    pub interval_modifier: f64,
    pub maximum_interval_days: u32,
    // Lapses
    pub relearning_steps_min: Vec<u32>,
    pub lapse_interval_factor: f64,
    pub minimum_interval_days: u32,
    pub leech_threshold: u32,
    // FSRS
    pub fsrs_weights: Vec<f64>,
    pub desired_retention: f64,
}

/// Kept for backward-compatibility with study-preview code that only needs limits.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DeckOptions {
    #[ts(type = "number")]
    pub deck_id: i64,
    pub new_per_day: u32,
    pub review_per_day: u32,
}

/// A deck as shown in the deck browser / sidebar tree, with due-card counts.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DeckSummary {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    #[ts(type = "number | null")]
    pub parent_id: Option<i64>,
    pub new_count: u32,
    pub learning_count: u32,
    pub review_count: u32,
    pub is_filtered: bool,
}

impl DeckSummary {
    pub fn with_counts(deck: Deck, counts: (u32, u32, u32)) -> Self {
        Self {
            id: deck.id,
            is_filtered: deck.is_filtered,
            name: deck.name,
            parent_id: deck.parent_id,
            new_count: counts.0,
            learning_count: counts.1,
            review_count: counts.2,
        }
    }
}

impl From<Deck> for DeckSummary {
    fn from(deck: Deck) -> Self {
        Self {
            is_filtered: deck.is_filtered,
            id: deck.id,
            name: deck.name,
            parent_id: deck.parent_id,
            new_count: 0,
            learning_count: 0,
            review_count: 0,
        }
    }
}

/// Config for a filtered (custom study) deck — returned for the rebuild dialog.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FilteredDeckConfig {
    #[ts(type = "number")]
    pub deck_id: i64,
    pub name: String,
    pub search: String,
    pub order: u8,
    pub limit: u32,
}

/// A card presented for study: rendered HTML for both sides plus the
/// human-readable next-interval label for each of the four answer buttons.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StudyCardDto {
    #[ts(type = "number")]
    pub card_id: i64,
    #[ts(type = "number")]
    pub deck_id: i64,
    pub question: String,
    pub answer: String,
    pub again: String,
    pub hard: String,
    pub good: String,
    pub easy: String,
    /// Remaining new cards due today (including this one if new).
    pub new_count: u32,
    /// Remaining learning/relearning cards due now.
    pub learning_count: u32,
    /// Remaining review cards due today.
    pub review_count: u32,
    /// "new" | "learning" | "review" | "relearning"
    pub card_phase: String,
    /// Deck's active scheduling algorithm.
    pub algorithm: Algorithm,
}

/// A single note field (name + HTML value), in note order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NoteField {
    pub name: String,
    pub value: String,
}

/// A row in the card/note browser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NoteOverview {
    #[ts(type = "number")]
    pub note_id: i64,
    /// Display value (the note's sort field), may contain HTML.
    pub sort_field: String,
    pub tags: Vec<String>,
}

/// A rich card-level browser row returned by `search_cards`.
/// Carries card scheduling state + note display data so the browser table can
/// show deck, type, due date, flags and tags without a second query.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CardRow {
    #[ts(type = "number")]
    pub card_id: i64,
    #[ts(type = "number")]
    pub note_id: i64,
    /// Plaintext of the note's sort field (HTML stripped).
    pub sort_field: String,
    pub deck: String,
    pub notetype: String,
    pub tags: Vec<String>,
    /// Raw queue value: -3 sibling-buried, -2 buried, -1 suspended, 0 new, 1 learn, 2 review.
    pub queue: i32,
    /// Card type: 0 new, 1 learning, 2 review, 3 relearning.
    pub card_type: i32,
    /// Due value: day number for reviews, epoch-ms for learning, position for new.
    #[ts(type = "number")]
    pub due: i64,
    pub interval: i32,
    pub lapses: i32,
    pub reps: i32,
    pub flags: i32,
}

/// A note type as shown in the Add Note form — id, name, kind, and ordered field names.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NotetypeSummary {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    /// 0 = standard, 1 = cloze.
    pub kind: i64,
    pub field_names: Vec<String>,
}

/// A single field definition as shown in the note-type editor.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FieldSummary {
    pub ord: i64,
    pub name: String,
}

/// A card template as shown in the note-type editor (includes format strings).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TemplateSummary {
    pub ord: i64,
    pub name: String,
    pub qfmt: String,
    pub afmt: String,
}

/// Full note-type detail for the note-type editor: fields + templates.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NotetypeDetail {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    /// 0 = standard, 1 = cloze.
    pub kind: i64,
    pub fields: Vec<FieldSummary>,
    pub templates: Vec<TemplateSummary>,
}

/// Rendered question + answer HTML for the template preview pane.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RenderedPreview {
    pub question: String,
    pub answer: String,
}

/// Returned by `check_field_remove`: how many existing notes have non-empty
/// content in the field about to be removed (lets the UI warn before confirming).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FieldRemoveWarning {
    pub notes_with_content: u32,
}

/// Returned after successfully adding a note.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AddNoteResult {
    #[ts(type = "number")]
    pub note_id: i64,
    pub cards_added: u32,
}

/// Full note for the editor: ordered fields + tags + its note type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NoteDetail {
    #[ts(type = "number")]
    pub note_id: i64,
    pub notetype_name: String,
    pub fields: Vec<NoteField>,
    pub tags: Vec<String>,
}

/// A count for one day. For review history `day` is an epoch-day number
/// (ms / 86_400_000); for the forecast it is a day offset from today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DayCount {
    #[ts(type = "number")]
    pub day: i64,
    pub count: u32,
}

/// Aggregate collection statistics for the dashboards.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StatsDto {
    pub total_reviews: u32,
    pub studied_days: u32,
    /// Pass rate over the last 30 days, as a percentage (0–100).
    pub retention_pct: f64,
    #[ts(type = "number")]
    pub total_time_ms: i64,
    pub new_count: u32,
    pub learning_count: u32,
    pub young_count: u32,
    pub mature_count: u32,
    pub suspended_count: u32,
    /// Reviews per epoch-day (all time), ascending.
    pub reviews: Vec<DayCount>,
    /// Due review cards per day-offset (0..=30 from today).
    pub forecast: Vec<DayCount>,
}

/// Result of an FSRS weight-optimization run (M20).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FsrsOptimizeResult {
    pub weights: Vec<f64>,
    pub log_loss_before: f64,
    pub log_loss_after: f64,
    pub review_count: usize,
    pub card_count: usize,
}

/// A single backup entry as shown in the maintenance UI.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BackupInfo {
    pub name: String,
    #[ts(type = "number")]
    pub created_ms: i64,
    #[ts(type = "number")]
    pub size_bytes: i64,
}

/// A plugin as shown in the plugin manager UI.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    /// Declared capability strings, e.g. `["ui:command", "events:listen"]`.
    pub permissions: Vec<String>,
    pub enabled: bool,
}

/// Results of a media consistency scan.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MediaReport {
    /// Files on disk not referenced by any note.
    pub orphan_files: Vec<String>,
    /// Filenames referenced in notes but absent from disk.
    pub missing_files: Vec<String>,
}

/// The serialisable error union returned across the IPC boundary. The frontend
/// receives `{ kind, message }` and can branch on `kind`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct IpcError {
    pub kind: IpcErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum IpcErrorKind {
    NotOpen,
    Storage,
    Format,
    Scheduler,
    NotFound,
    Invalid,
    Internal,
}

impl From<CoreError> for IpcError {
    fn from(error: CoreError) -> Self {
        let kind = match &error {
            CoreError::NotOpen => IpcErrorKind::NotOpen,
            CoreError::Storage(_) => IpcErrorKind::Storage,
            CoreError::Format(_) => IpcErrorKind::Format,
            CoreError::Scheduler(_) => IpcErrorKind::Scheduler,
            CoreError::NotFound(_) => IpcErrorKind::NotFound,
            CoreError::Invalid(_) => IpcErrorKind::Invalid,
            CoreError::Other(_) => IpcErrorKind::Internal,
        };
        Self {
            kind,
            message: error.to_string(),
        }
    }
}
