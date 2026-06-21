# Contributing to Synapse

## Prerequisites

| Tool | Version |
|------|---------|
| Rust | stable (1.78+) |
| Node.js | 20 LTS |
| pnpm | 9+ |
| Tauri CLI | v2 (`cargo install tauri-cli`) |

## Setup

```sh
git clone https://github.com/synapse-srs/synapse
cd synapse
pnpm install
```

Rust crates are in `crates/`; the desktop app is in `apps/desktop/`.

## Development

```sh
# Run dev server (hot-reload frontend + Tauri window)
pnpm dev

# Rust tests
cargo test --workspace

# Frontend tests
pnpm test

# TypeScript type check
pnpm exec tsc --noEmit

# Lint
pnpm lint
cargo clippy -- -D warnings

# Format check
cargo fmt --check
pnpm fmt:check

# Regenerate IPC types (after changing Rust structs with #[derive(TS)])
pnpm bindings
```

## Architecture

Clean architecture; layers must not import downward:

```
synapse-core   — ports + domain types (no IO)
   ↓
synapse-db / synapse-scheduler / synapse-render / synapse-ankifmt / synapse-plugin
   ↓
apps/desktop/src-tauri — Tauri shell (implements ports, exposes IPC)
   ↓
apps/desktop/src      — React 19 frontend
```

See `docs/ARCHITECTURE.md` for full detail.

## Adding an IPC command

1. Write Rust handler in `apps/desktop/src-tauri/src/commands/<feature>.rs`.
2. Register in `apps/desktop/src-tauri/src/lib.rs` `generate_handler!`.
3. Add typed wrapper in `apps/desktop/src/lib/ipc.ts`.
4. Run `pnpm bindings` to regenerate TypeScript types.
5. Emit the appropriate `DomainEvent` so TanStack Query keys invalidate.

## Pull requests

- Keep PRs vertically sliced: one feature + its tests + relevant hardening.
- Every new IPC command needs a Rust test and a TypeScript wrapper.
- `cargo test --workspace` + `pnpm test` must pass.
- `cargo clippy -- -D warnings` must be clean.
- No `unwrap()`/`expect()` in `commands/*` or import/export paths.

## Commit style

```
feat: add FSRS weight optimizer command
fix: study queue excludes buried siblings
test: golden vector for cloze template render
docs: ADR-003 plugin sandbox decision
```

## Reporting issues

Open an issue at https://github.com/synapse-srs/synapse/issues with:
- OS + version
- Steps to reproduce
- Expected vs actual behaviour
- Log from `Help → Open Log Folder` if relevant
