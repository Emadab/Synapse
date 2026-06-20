import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AppInfo,
  DeckOptions,
  DeckSummary,
  ImportSummary,
  IpcError,
  NoteDetail,
  NoteOverview,
  StatsDto,
  StudyCardDto,
} from "@synapse/ipc-types";

/** Answer-button rating values (match Rust `Rating`). */
export const Rating = { Again: 1, Hard: 2, Good: 3, Easy: 4 } as const;
export type RatingValue = (typeof Rating)[keyof typeof Rating];

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

  // Deck options
  getDeckOptions: (deckId: number) =>
    invoke<DeckOptions>("get_deck_options", { deckId }),
  setDeckOptions: (deckId: number, newPerDay: number, reviewPerDay: number) =>
    invoke<void>("set_deck_options", { deckId, newPerDay, reviewPerDay }),

  // Import
  importPackage: (path: string) => invoke<ImportSummary>("import_package", { path }),

  // Study
  getNextCard: (deckId: number) => invoke<StudyCardDto | null>("get_next_card", { deckId }),
  answerCard: (cardId: number, rating: RatingValue) =>
    invoke<StudyCardDto | null>("answer_card", { cardId, rating }),

  // Browser / editor
  listNotes: (query?: string) => invoke<NoteOverview[]>("list_notes", { query: query ?? null }),
  searchNotes: (query: string) => invoke<NoteOverview[]>("search_notes", { query }),
  getNote: (noteId: number) => invoke<NoteDetail | null>("get_note", { noteId }),
  saveNote: (noteId: number, fields: string[], tags: string[]) =>
    invoke<void>("save_note", { noteId, fields, tags }),

  // Undo
  undo: () => invoke<string | null>("undo"),
  undoStatus: () => invoke<string | null>("undo_status"),

  // Export
  exportPackage: (path: string) => invoke<number>("export_package", { path }),

  // Statistics
  getStats: () => invoke<StatsDto>("get_stats"),
};

/**
 * Prompt for a save location and export the full collection as `.apkg`.
 * Returns the number of media files written, or `null` if user cancelled.
 */
export async function pickAndExportPackage(): Promise<number | null> {
  const { save } = await import("@tauri-apps/plugin-dialog");
  const selected = await save({
    defaultPath: "collection.apkg",
    filters: [{ name: "Anki package", extensions: ["apkg"] }],
  });
  if (typeof selected !== "string") return null;
  return ipc.exportPackage(selected);
}

/**
 * Prompt for an .apkg/.colpkg and import it. Returns the summary, or `null` if
 * the user cancelled the file picker.
 */
export async function pickAndImportPackage(): Promise<ImportSummary | null> {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Anki package", extensions: ["apkg", "colpkg"] }],
  });
  if (typeof selected !== "string") return null;
  return ipc.importPackage(selected);
}

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
