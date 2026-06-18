//! Domain row entities mirrored from the canonical schema. M1 introduces the
//! deck-related entities; notes, cards, notetypes, templates and revlog rows
//! land alongside import (M2) when they are first populated.

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

/// A deck options group (limits, learning steps, scheduler choice, FSRS params).
/// M1 stores the row; the rich config JSON is exercised from M3 onward.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckConfig {
    pub id: i64,
    pub name: String,
    pub mod_ms: i64,
    pub usn: i64,
}
