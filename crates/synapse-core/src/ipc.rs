//! DTOs that cross the IPC boundary to the frontend. Every type here derives
//! [`ts_rs::TS`] with `#[ts(export)]`, so `cargo test` regenerates the matching
//! TypeScript in `packages/ipc-types/src/generated/`. The Rust definitions are
//! the single source of truth; a mismatch breaks the TS build (intended).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Basic identity of the running application, surfaced on the home screen and
/// the About page. Also the M0 end-to-end proof of the IPC + ts-rs pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub tauri_version: String,
}
