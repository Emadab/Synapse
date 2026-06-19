//! Domain model. M0 established the cross-cutting enums; M1 adds the deck
//! entities (see [`entities`]). Remaining entities (Note, Card, Notetype,
//! Template, Revlog) arrive with import in M2.

pub mod canonical;
pub mod entities;
pub use canonical::{CanonicalModel, ImportSummary};
pub use entities::{Card, Deck, DeckConfig, Field, Note, Notetype, Revlog, Template};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The grade a user gives a card when answering. Matches Anki's four buttons so
/// review history round-trips losslessly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Rating {
    Again = 1,
    Hard = 2,
    Good = 3,
    Easy = 4,
}

/// Which scheduling algorithm a deck uses. Selected per deck and switchable at
/// runtime because both SM-2 and FSRS state always persist together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Algorithm {
    Sm2,
    Fsrs,
}
