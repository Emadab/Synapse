import { invoke } from "@tauri-apps/api/core";
import type { AppInfo, DeckSummary, IpcError } from "@synapse/ipc-types";

/**
 * Typed wrapper over Tauri's `invoke`. The UI imports from here and never calls
 * `invoke` directly, so the command surface is discoverable and type-checked
 * against the Rust-generated `@synapse/ipc-types`. Rejections are `IpcError`.
 */
export const ipc = {
  appInfo: () => invoke<AppInfo>("app_info"),

  // Decks
  listDecks: () => invoke<DeckSummary[]>("list_decks"),
  createDeck: (name: string) => invoke<DeckSummary>("create_deck", { name }),
  renameDeck: (id: number, name: string) => invoke<void>("rename_deck", { id, name }),
  deleteDeck: (id: number) => invoke<void>("delete_deck", { id }),

  // Undo
  undo: () => invoke<string | null>("undo"),
  undoStatus: () => invoke<string | null>("undo_status"),
};

/** True when running inside the Tauri webview (vs a plain browser via `dev:web`). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Narrow an unknown rejection to its human-readable message. */
export function errorMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error) {
    return String((error as IpcError).message);
  }
  return String(error);
}
