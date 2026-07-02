## What / why

## Checklist

- [ ] `pnpm lint` and `cargo clippy -- -D warnings` pass
- [ ] `cargo fmt --check` and `pnpm fmt:check` pass
- [ ] `cargo test --workspace` and `pnpm test` pass
- [ ] New IPC commands have a Rust test and a TypeScript wrapper
- [ ] Ran `pnpm bindings` if Rust `#[derive(TS)]` structs changed
