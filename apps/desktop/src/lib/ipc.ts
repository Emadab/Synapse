import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AddNoteResult,
  AppInfo,
  BackupInfo,
  CardRow,
  FsrsOptimizeResult,
  DeckConfig,
  DeckSummary,
  FieldRemoveWarning,
  FilteredDeckConfig,
  ImportSummary,
  IpcError,
  MediaReport,
  NoteDetail,
  NoteOverview,
  NotetypeDetail,
  NotetypeSummary,
  PluginInfo,
  RenderedPreview,
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

  // Full deck config (M14)
  getDeckConfig: (deckId: number) =>
    invoke<DeckConfig>("get_deck_config", { deckId }),
  setDeckConfig: (config: DeckConfig) =>
    invoke<void>("set_deck_config", { config }),

  // Import
  importPackage: (path: string) => invoke<ImportSummary>("import_package", { path }),

  // Study
  getNextCard: (deckId: number) => invoke<StudyCardDto | null>("get_next_card", { deckId }),
  answerCard: (cardId: number, rating: RatingValue) =>
    invoke<StudyCardDto | null>("answer_card", { cardId, rating }),

  // Note types (summary list — used by Add Note picker)
  listNotetypes: () => invoke<NotetypeSummary[]>("list_notetypes"),

  // Add note
  addNote: (notetypeId: number, deckId: number, fields: string[], tags: string[]) =>
    invoke<AddNoteResult>("add_note", { notetypeId, deckId, fields, tags }),

  // Note-type editor
  getNotetype: (notetypeId: number) =>
    invoke<NotetypeDetail | null>("get_notetype", { notetypeId }),
  createNotetype: (name: string, kind: number) =>
    invoke<NotetypeDetail>("create_notetype", { name, kind }),
  deleteNotetype: (notetypeId: number) =>
    invoke<void>("delete_notetype", { notetypeId }),
  renameNotetype: (notetypeId: number, name: string) =>
    invoke<void>("rename_notetype", { notetypeId, name }),
  addField: (notetypeId: number, name: string) =>
    invoke<void>("add_field", { notetypeId, name }),
  checkFieldRemove: (notetypeId: number, ord: number) =>
    invoke<FieldRemoveWarning>("check_field_remove", { notetypeId, ord }),
  removeField: (notetypeId: number, ord: number) =>
    invoke<void>("remove_field", { notetypeId, ord }),
  renameField: (notetypeId: number, ord: number, name: string) =>
    invoke<void>("rename_field", { notetypeId, ord, name }),
  reorderFields: (notetypeId: number, newOrder: number[]) =>
    invoke<void>("reorder_fields", { notetypeId, newOrder }),
  addTemplate: (notetypeId: number, name: string, qfmt: string, afmt: string) =>
    invoke<void>("add_template", { notetypeId, name, qfmt, afmt }),
  removeTemplate: (notetypeId: number, ord: number) =>
    invoke<void>("remove_template", { notetypeId, ord }),
  saveTemplate: (notetypeId: number, ord: number, name: string, qfmt: string, afmt: string) =>
    invoke<void>("save_template", { notetypeId, ord, name, qfmt, afmt }),
  previewTemplate: (notetypeId: number, templateOrd: number, sampleFields: string[]) =>
    invoke<RenderedPreview>("preview_template", { notetypeId, templateOrd, sampleFields }),

  // Browser / editor
  listNotes: (query?: string) => invoke<NoteOverview[]>("list_notes", { query: query ?? null }),
  searchNotes: (query: string) => invoke<NoteOverview[]>("search_notes", { query }),
  searchCards: (query: string) => invoke<CardRow[]>("search_cards", { query }),
  getNote: (noteId: number) => invoke<NoteDetail | null>("get_note", { noteId }),
  saveNote: (noteId: number, fields: string[], tags: string[]) =>
    invoke<void>("save_note", { noteId, fields, tags }),

  // Browser bulk ops (M16)
  deleteNotes: (noteIds: number[]) => invoke<void>("delete_notes", { noteIds }),
  moveCardsToDeck: (cardIds: number[], deckId: number) =>
    invoke<void>("move_cards_to_deck", { cardIds, deckId }),
  bulkAddTag: (noteIds: number[], tag: string) => invoke<void>("bulk_add_tag", { noteIds, tag }),
  bulkRemoveTag: (noteIds: number[], tag: string) =>
    invoke<void>("bulk_remove_tag", { noteIds, tag }),

  // Tag manager (M17)
  listTags: () => invoke<string[]>("list_tags"),
  renameTag: (oldTag: string, newTag: string) =>
    invoke<number>("rename_tag", { oldTag, newTag }),
  deleteTag: (tag: string) => invoke<number>("delete_tag", { tag }),
  mergeTags: (sources: string[], target: string) =>
    invoke<void>("merge_tags", { sources, target }),

  // Filtered decks (M17)
  createFilteredDeck: (name: string, search: string, order: number, limit: number) =>
    invoke<DeckSummary>("create_filtered_deck", { name, search, order, limit }),
  rebuildFiltered: (deckId: number) => invoke<number>("rebuild_filtered", { deckId }),
  emptyFiltered: (deckId: number) => invoke<void>("empty_filtered", { deckId }),
  getFilteredConfig: (deckId: number) =>
    invoke<FilteredDeckConfig | null>("get_filtered_config", { deckId }),

  // Card lifecycle (M15)
  suspendCards: (cardIds: number[]) => invoke<void>("suspend_cards", { cardIds }),
  unsuspendCards: (cardIds: number[]) => invoke<void>("unsuspend_cards", { cardIds }),
  buryCards: (cardIds: number[]) => invoke<void>("bury_cards", { cardIds }),
  setCardFlag: (cardIds: number[], flag: number) => invoke<void>("set_card_flag", { cardIds, flag }),

  // Undo
  undo: () => invoke<string | null>("undo"),
  undoStatus: () => invoke<string | null>("undo_status"),

  // Export
  exportPackage: (path: string) => invoke<number>("export_package", { path }),

  // Statistics
  getStats: () => invoke<StatsDto>("get_stats"),

  // FSRS optimizer (M20)
  optimizeFsrs: (deckId: number | null) =>
    invoke<FsrsOptimizeResult>("optimize_fsrs", { deckId }),
  applyFsrsWeights: (deckId: number, weights: number[]) =>
    invoke<void>("apply_fsrs_weights", { deckId, weights }),

  // Plugins (M21)
  listPlugins: () => invoke<PluginInfo[]>("list_plugins"),
  enablePlugin: (id: string) => invoke<void>("enable_plugin", { id }),
  disablePlugin: (id: string) => invoke<void>("disable_plugin", { id }),
  installPlugin: (path: string) => invoke<PluginInfo>("install_plugin", { path }),
  getPluginEntry: (id: string) => invoke<string>("get_plugin_entry", { id }),

  // Maintenance (M19)
  createBackup: () => invoke<BackupInfo>("create_backup"),
  listBackups: () => invoke<BackupInfo[]>("list_backups"),
  restoreBackup: (name: string) => invoke<void>("restore_backup", { name }),
  checkIntegrity: () => invoke<string[]>("check_integrity"),
  optimizeDb: () => invoke<void>("optimize_db"),
  checkMedia: () => invoke<MediaReport>("check_media"),
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
