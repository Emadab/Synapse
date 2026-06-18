//! # synapse-plugin
//!
//! Plugin host abstraction. MVP defines the contract only — the manifest, the
//! capability model (`read:notes`, `write:cards`, `ui:panel`, `net:fetch`, …)
//! and the extension-point registry that core hooks register against. Untrusted
//! code never receives raw DB/file handles, only the capability-scoped API.
//! The sandboxed runtime that executes plugins is a post-MVP milestone.

/// Capabilities a plugin may request in its manifest. The host grants only what
/// is declared and enforces it at every API call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    ReadNotes,
    WriteCards,
    UiPanel,
    UiCommand,
    NetFetch,
    ListenEvents,
}
