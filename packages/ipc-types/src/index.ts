// Barrel for the Rust-generated IPC types. The files under `./generated` are
// produced by ts-rs when `cargo test` runs (see `pnpm bindings`); never edit
// them by hand. Add a re-export line here when a new exported type appears.

export type { AppInfo } from "./generated/AppInfo";
export type { Algorithm } from "./generated/Algorithm";
export type { Rating } from "./generated/Rating";
export type { DeckSummary } from "./generated/DeckSummary";
export type { ImportSummary } from "./generated/ImportSummary";
export type { IpcError } from "./generated/IpcError";
export type { IpcErrorKind } from "./generated/IpcErrorKind";
export type { StudyCardDto } from "./generated/StudyCardDto";
