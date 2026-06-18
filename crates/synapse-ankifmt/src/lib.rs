//! # synapse-ankifmt
//!
//! Bidirectional Anki compatibility. Readers detect the container
//! (`.apkg`/`.colpkg`) and schema version (legacy v11 JSON-blob vs modern v18
//! tabular) and emit a single `CanonicalModel`; writers serialise it back to
//! `.apkg` v2 (zlib) / v3 (zstd) and `.colpkg`. Round-trip fidelity is a test
//! gate: import → export → re-import must be a no-op diff.
//!
//! Readers + merge land in M2; writers in M6.
