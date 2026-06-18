//! # synapse-core
//!
//! The innermost ring of Synapse's clean architecture: the domain model, the
//! application use-cases, the typed event bus and the *ports* (traits) that
//! outer layers implement.
//!
//! This crate must never depend on a UI framework (React/Tauri), on a concrete
//! database (rusqlite), or on a search engine (Tantivy). Those are wired in via
//! the traits declared in [`ports`]. Keeping this boundary clean is what lets
//! the same engine power the desktop app today and a mobile/CLI app later.

pub mod error;
pub mod events;
pub mod ipc;
pub mod model;
pub mod ports;

pub use error::{CoreError, CoreResult};
pub use events::DomainEvent;
pub use model::{Algorithm, Rating};
pub use ports::Clock;
