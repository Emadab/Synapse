//! # synapse-db
//!
//! SQLite storage adapter. Owns every SQL string, the canonical schema and a
//! versioned migration runner, and implements [`synapse_core::ports::Storage`]
//! via [`SqliteStorage`]. SQLite is the transactional source of truth;
//! full-text search lives in Tantivy (`synapse-search`), not here.

pub mod import;
pub mod migrations;
pub mod schema;
pub mod storage;

pub use storage::SqliteStorage;
