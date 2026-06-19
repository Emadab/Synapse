//! Domain row entities mirrored from the canonical schema. These are plain data
//! (no IO). The import/export adapters build them; `synapse-db` persists them.

use serde::{Deserialize, Serialize};

/// A deck. `name` is the full Anki-style path (e.g. `"Med::Anatomy::Head"`);
/// `parent_id` is the denormalized immediate parent for fast tree rendering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deck {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub config_id: i64,
    /// Last-modified time (ms since epoch).
    pub mod_ms: i64,
    /// Update sequence number; `-1` means "modified locally, needs sync".
    pub usn: i64,
    pub collapsed: bool,
    pub is_filtered: bool,
}

/// A deck options group (limits, learning steps, scheduler choice, FSRS params),
/// stored as opaque JSON until the scheduler milestones interpret it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckConfig {
    pub id: i64,
    pub name: String,
    pub mod_ms: i64,
    pub usn: i64,
    /// Options JSON (Anki `dconf` entry). Defaults to `{}`.
    pub config_json: String,
}

/// A note type (model): its templates, fields, CSS, and kind (standard/cloze).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notetype {
    pub id: i64,
    pub name: String,
    /// 0 = standard, 1 = cloze.
    pub kind: i64,
    pub mod_ms: i64,
    pub usn: i64,
    /// Config JSON (css, latex pre/post, sort field index, …).
    pub config_json: String,
}

/// A field definition belonging to a [`Notetype`], ordered by `ord`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub notetype_id: i64,
    pub ord: i64,
    pub name: String,
    pub config_json: String,
}

/// A card template (front/back) belonging to a [`Notetype`], ordered by `ord`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Template {
    pub notetype_id: i64,
    pub ord: i64,
    pub name: String,
    pub qfmt: String,
    pub afmt: String,
    pub config_json: String,
}

/// A note: the user's content. `fields` are the field values in order; `tags`
/// are individual tags (no leading/trailing spaces).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub guid: String,
    pub notetype_id: i64,
    pub mod_ms: i64,
    pub usn: i64,
    pub tags: Vec<String>,
    pub fields: Vec<String>,
    pub sort_field: Option<String>,
    pub checksum: Option<i64>,
}

/// A card: a scheduled instance of one template of a note. Fields mirror Anki
/// 1:1 for lossless round-trip; FSRS memory state lives alongside SM-2 state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub id: i64,
    pub note_id: i64,
    pub deck_id: i64,
    pub ord: i64,
    pub mod_ms: i64,
    pub usn: i64,
    /// 0=new 1=learn 2=review 3=relearn.
    pub ctype: i64,
    /// -3..2 (suspended/buried/new/learn/review).
    pub queue: i64,
    pub due: i64,
    pub interval: i64,
    pub ease_factor: i64,
    pub reps: i64,
    pub lapses: i64,
    pub remaining: i64,
    pub original_due: i64,
    pub original_deck_id: i64,
    pub flags: i64,
    pub fsrs_stability: Option<f64>,
    pub fsrs_difficulty: Option<f64>,
    pub fsrs_last_review: Option<i64>,
    /// Verbatim Anki `cards.data` JSON, preserved for byte-faithful round-trip.
    pub data: Option<String>,
}

/// A single review event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revlog {
    pub id: i64,
    pub card_id: i64,
    pub usn: i64,
    pub ease: i64,
    pub interval: i64,
    pub last_interval: i64,
    pub ease_factor: i64,
    pub taken_ms: i64,
    /// 0=learn 1=review 2=relearn 3=cram 4=manual.
    pub review_kind: i64,
}
