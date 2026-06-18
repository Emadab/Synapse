import { invoke } from "@tauri-apps/api/core";
import type { AppInfo } from "@synapse/ipc-types";

/**
 * Typed wrapper over Tauri's `invoke`. The UI imports from here and never calls
 * `invoke` directly, so the command surface is discoverable and type-checked
 * against the Rust-generated `@synapse/ipc-types`.
 */
export const ipc = {
  appInfo: () => invoke<AppInfo>("app_info"),
};

/** True when running inside the Tauri webview (vs a plain browser via `dev:web`). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
