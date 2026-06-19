//! # synapse-core
//!
//! The innermost ring of Synapse's clean architecture: the domain model, the
//! application use-cases ([`Collection`]), the typed event bus and the *ports*
//! (traits) that outer layers implement.
//!
//! This crate must never depend on a UI framework (React/Tauri), on a concrete
//! database (rusqlite), or on a search engine (Tantivy). Those are wired in via
//! the traits declared in [`ports`]. Keeping this boundary clean is what lets
//! the same engine power the desktop app today and a mobile/CLI app later.

pub mod collection;
pub mod error;
pub mod events;
pub mod ipc;
pub mod model;
pub mod ports;
pub mod scheduling;
pub mod undo;

pub use collection::Collection;
pub use error::{CoreError, CoreResult};
pub use events::{DomainEvent, EventBus, EventSink};
pub use model::{
    Algorithm, CanonicalModel, Card, CardRender, Deck, DeckConfig, Field, ImportSummary, Note,
    Notetype, Rating, Revlog, StudyCard, Template,
};
pub use ports::{Clock, Storage, SystemClock};
pub use scheduling::{
    AnswerOutcome, CardPhase, CardState, Interval, RatingPreviews, SchedConfig, SchedContext,
    Scheduler,
};
