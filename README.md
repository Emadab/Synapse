# Synapse

> A beautiful, modern spaced-repetition desktop app — fully compatible with Anki.

Synapse wraps Anki's proven scheduling data model in a Linear/Obsidian-grade
experience, while guaranteeing you can move decks in and out of Anki losslessly.

- **100% Anki-compatible** import/export (`.apkg` v2/v3, `.colpkg`; schema v11 & v18)
- **SM-2 and FSRS** scheduling, switchable per deck
- **Offline-first**, native-feeling, keyboard-first
- **Tauri + React/TypeScript** over a **UI-agnostic Rust core**

## Architecture

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). In short: a platform-agnostic
Rust core (`crates/synapse-*`) with clean-architecture dependency rules, a thin
Tauri shell (`apps/desktop`), and shared TS packages (`packages/*`). The Rust
type definitions are the single source of truth for the IPC boundary; ts-rs
generates the matching TypeScript.

## Layout

```
crates/        Rust core: core, db, scheduler, ankifmt, search, media, render, plugin
apps/desktop/  Tauri shell (src-tauri) + React frontend (src)
packages/      Shared TS: ipc-types (generated), ui-tokens, plugin-sdk
fixtures/      Real .apkg/.colpkg samples + golden scheduler vectors
docs/          Architecture & ADRs
```

## Prerequisites

- Rust (stable) with the MSVC toolchain on Windows
- Node ≥ 20 and pnpm ≥ 11
- The Tauri prerequisites for your OS (WebView2 on Windows)

## Develop

```bash
pnpm install          # install JS deps + link the workspace
pnpm bindings         # regenerate TS types from Rust (cargo test -p synapse-core)
pnpm dev              # run the desktop app (Tauri + Vite)
pnpm dev:web          # run just the web frontend in a browser
```

## Quality gates

```bash
pnpm lint             # eslint + clippy (-D warnings)
pnpm fmt:check        # prettier + rustfmt
pnpm test             # vitest + cargo test (workspace)
pnpm build            # type-check + production frontend build
```

## License

MIT OR Apache-2.0
