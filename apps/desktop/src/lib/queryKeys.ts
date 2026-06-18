/**
 * Centralized TanStack Query keys. Domain events (re-emitted from the Rust core)
 * map to invalidations of these keys, keeping the UI live without polling.
 */
export const queryKeys = {
  appInfo: ["app-info"] as const,
  decks: ["decks"] as const,
  deck: (deckId: string) => ["decks", deckId] as const,
  queue: (deckId: string) => ["queue", deckId] as const,
  notes: (query: string) => ["notes", query] as const,
  stats: ["stats"] as const,
};
