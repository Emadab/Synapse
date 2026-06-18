//! Domain model. M0 establishes the stable, cross-cutting enums shared by the
//! scheduler, the IPC layer and the UI. Entities (Note, Card, Deck, Notetype,
//! Template, Revlog) land in M1 alongside the canonical schema.

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
