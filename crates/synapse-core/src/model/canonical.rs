//! The format-neutral intermediate representation that import readers produce
//! and export writers consume. Both Anki schema v11 and v18 map to this single
//! model, so the rest of the app never sees on-disk format differences.
//!
//! Ids here are the *source* ids (Anki epoch-ms ids). The storage layer merges
//! them into the open collection: decks/notetypes are matched by name, notes by
//! `guid`; ids are kept when free and remapped on collision.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Card, Deck, DeckConfig, Field, Note, Notetype, Revlog, Template};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanonicalModel {
    pub deck_configs: Vec<DeckConfig>,
    pub decks: Vec<Deck>,
    pub notetypes: Vec<Notetype>,
    pub fields: Vec<Field>,
    pub templates: Vec<Template>,
    pub notes: Vec<Note>,
    pub cards: Vec<Card>,
    pub revlog: Vec<Revlog>,
}

impl CanonicalModel {
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty() && self.decks.is_empty() && self.notetypes.is_empty()
    }
}

/// What an import changed. Surfaced to the UI as a summary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ImportSummary {
    pub decks_added: u32,
    pub notetypes_added: u32,
    pub notes_added: u32,
    pub notes_updated: u32,
    pub cards_added: u32,
    pub revlog_added: u32,
    pub media_imported: u32,
}
