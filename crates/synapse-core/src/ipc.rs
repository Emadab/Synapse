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
use crate::model::Deck;

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
}

impl DeckSummary {
    pub fn with_counts(deck: Deck, counts: (u32, u32, u32)) -> Self {
        Self {
            id: deck.id,
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
            id: deck.id,
            name: deck.name,
            parent_id: deck.parent_id,
            new_count: 0,
            learning_count: 0,
            review_count: 0,
        }
    }
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
    /// Cards still due in this deck (including this one).
    pub remaining: u32,
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
