//! IPC command surface. Each submodule groups related commands. Commands are
//! thin: validate, delegate to `synapse-core`, (de)serialise. They must not
//! contain domain logic.

pub mod app;
