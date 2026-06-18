//! # synapse-search
//!
//! Tantivy-backed search index plus a query parser that accepts Anki-style
//! search syntax (`deck:`, `tag:`, `is:due`, `prop:ivl>=21`, …). The index is
//! kept in sync by subscribing to [`synapse_core::DomainEvent`]s; SQLite
//! remains the transactional truth.
//!
//! Index + parser land in M7.
