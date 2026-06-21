# ADR-001: Clean Architecture with Port/Adapter separation

**Status:** Accepted  
**Date:** 2024-11-01

## Context

Synapse needs to run on Windows, macOS, and Linux today, with iOS and Android planned for v1.1. The storage and scheduling logic must not depend on Tauri, the OS, or any desktop-specific IO.

## Decision

Enforce strict layer separation across the Rust workspace:

```
synapse-core       — traits (ports) + domain types only; no IO, no framework
synapse-db         — SQLite adapter implementing storage ports
synapse-scheduler  — SM-2 and FSRS-5 algorithms (pure computation)
synapse-render     — Anki template engine (pure text transform)
synapse-ankifmt    — .apkg/.colpkg read/write (zip + SQLite)
synapse-plugin     — plugin manifest loader + capability enforcer
apps/desktop       — Tauri shell; glues adapters + exposes IPC
```

`synapse-core` defines `CollectionPort`, `StudyPort`, `SchedulerPort`, `RenderPort`, and `ExportPort` as Rust traits. Adapters in the lower crates implement them. `apps/desktop/src-tauri` wires the adapters together and exposes commands via Tauri IPC.

## Consequences

- No `tauri`, `tokio`, or OS-specific IO in `synapse-core`, `synapse-db`, `synapse-scheduler`, `synapse-render`, or `synapse-ankifmt`.
- Porting to a mobile shell (Tauri 2 iOS/Android) requires only new adapter implementations in a new app crate.
- Tests for core logic (`synapse-db`, `synapse-scheduler`, `synapse-render`) use in-memory SQLite or pure functions — no app harness needed.
- IPC boundary is the only place where `Result` maps to `IpcError` for the frontend.
