//! # synapse-db
//!
//! SQLite storage adapter. Owns every SQL string, the canonical schema and a
//! versioned migration runner, and implements [`synapse_core::ports::Storage`]
//! via [`SqliteStorage`]. SQLite is the transactional source of truth;
//! full-text search lives in Tantivy (`synapse-search`), not here.

pub mod backup;
pub mod browse;
pub mod cards;
pub mod export;
pub mod filtered;
pub mod import;
pub mod migrations;
pub mod notetype;
pub mod schema;
pub mod search;
pub mod stats;
pub mod stock;
pub mod storage;
pub mod study;
pub mod tags;

pub use storage::SqliteStorage;
