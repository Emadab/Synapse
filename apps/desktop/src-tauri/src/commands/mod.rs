//! IPC command surface. Each submodule groups related commands. Commands are
//! thin: validate, delegate to `synapse-core`, (de)serialise. They must not
//! contain domain logic. Errors surface as the typed `IpcError` union.

pub mod app;
pub mod deck;
pub mod import;
pub mod study;
