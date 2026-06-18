//! # synapse-db
//!
//! SQLite storage adapter. Owns every SQL string, the canonical schema and a
//! versioned migration runner, and implements [`synapse_core::ports::Storage`].
//! SQLite is the transactional source of truth; full-text search lives in
//! Tantivy (`synapse-search`), not here.
//!
//! Schema, migrations and `rusqlite` wiring land in M1.
