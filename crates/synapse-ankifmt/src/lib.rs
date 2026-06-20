//! # synapse-ankifmt
//!
//! Bidirectional Anki compatibility. [`read_package`] detects the container
//! (`.apkg`/`.colpkg`) and schema version and emits a [`CanonicalModel`];
//! writers (M6) serialise it back. Round-trip fidelity is a test gate.
//!
//! Status: import of both legacy schema **v11** (JSON-blob `col`) and modern
//! **v18** (tabular + protobuf template configs, zstd `.anki21b`) is supported.

mod pb;
pub mod reader;
pub mod testkit;
pub mod writer;

pub use reader::read_package;
pub use writer::write_apkg;
