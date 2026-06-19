//! # synapse-ankifmt
//!
//! Bidirectional Anki compatibility. [`read_package`] detects the container
//! (`.apkg`/`.colpkg`) and schema version and emits a [`CanonicalModel`];
//! writers (M6) serialise it back. Round-trip fidelity is a test gate.
//!
//! Status: legacy schema **v11** import is implemented; modern **v18**
//! (protobuf-in-SQLite) is detected and rejected with a clear error.

pub mod reader;
pub mod testkit;

pub use reader::read_package;
