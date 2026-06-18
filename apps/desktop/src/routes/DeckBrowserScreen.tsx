import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Layers, Plus, Trash2, Undo2 } from "lucide-react";
import type { DeckSummary } from "@synapse/ipc-types";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { errorMessage, ipc, isTauri } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";

function deckDepth(name: string): number {
  return name.split("::").length - 1;
}

function deckLabel(name: string): string {
  const parts = name.split("::");
  return parts[parts.length - 1];
}

export function DeckBrowserScreen() {
  const queryClient = useQueryClient();
  const tauri = isTauri();

  const { data: app } = useQuery({
    queryKey: queryKeys.appInfo,
    queryFn: ipc.appInfo,
    enabled: tauri,
  });

  const decksQuery = useQuery({
    queryKey: queryKeys.decks,
    queryFn: ipc.listDecks,
    enabled: tauri,
  });

  const invalidateDecks = () => queryClient.invalidateQueries({ queryKey: queryKeys.decks });

  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");

  const createMut = useMutation({
    mutationFn: (deckName: string) => ipc.createDeck(deckName),
    onSuccess: () => {
      setName("");
      setCreating(false);
      void invalidateDecks();
    },
  });

  const deleteMut = useMutation({
    mutationFn: (id: number) => ipc.deleteDeck(id),
    onSuccess: () => void invalidateDecks(),
  });

  const undoMut = useMutation({
    mutationFn: () => ipc.undo(),
    onSuccess: () => void invalidateDecks(),
  });

  const decks = decksQuery.data ?? [];

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        title="Decks"
        description="Your collection. Import an Anki deck, or create one to get started."
        actions={
          <>
            <Button
              variant="outline"
              onClick={() => undoMut.mutate()}
              disabled={!tauri || undoMut.isPending}
              title="Undo the last change"
            >
              <Undo2 /> Undo
            </Button>
            <Button onClick={() => setCreating((value) => !value)} disabled={!tauri}>
              <Plus /> New deck
            </Button>
          </>
        }
      />

      <div className="relative flex-1 overflow-auto">
        {creating ? (
          <form
            className="flex items-center gap-2 border-b border-border bg-secondary/40 px-8 py-3"
            onSubmit={(e) => {
              e.preventDefault();
              if (name.trim()) createMut.mutate(name.trim());
            }}
          >
            <input
              autoFocus
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Deck name (use :: for sub-decks)"
              className="h-9 flex-1 rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
            <Button type="submit" size="sm" disabled={!name.trim() || createMut.isPending}>
              Create
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => {
                setCreating(false);
                setName("");
              }}
            >
              Cancel
            </Button>
          </form>
        ) : null}

        {createMut.isError ? (
          <p className="px-8 py-2 text-sm text-destructive">{errorMessage(createMut.error)}</p>
        ) : null}

        {!tauri ? (
          <EmptyState
            icon={Layers}
            title="Run the desktop app"
            description="Deck data is served by the Rust core over Tauri. Launch with `pnpm dev` to load your collection."
          />
        ) : decks.length === 0 ? (
          <EmptyState
            icon={Layers}
            title="No decks yet"
            description="Create a deck above, or import an .apkg / .colpkg (milestone M2)."
          />
        ) : (
          <ul className="divide-y divide-border">
            {decks.map((deck: DeckSummary) => (
              <li
                key={deck.id}
                className="group flex items-center gap-3 px-8 py-3 hover:bg-accent/40"
                style={{ paddingLeft: `${2 + deckDepth(deck.name) * 1.25}rem` }}
              >
                <Layers className="size-4 shrink-0 text-muted-foreground" />
                <span className="flex-1 truncate text-sm font-medium">{deckLabel(deck.name)}</span>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Delete ${deck.name}`}
                  className="opacity-0 transition-opacity group-hover:opacity-100"
                  onClick={() => deleteMut.mutate(deck.id)}
                  disabled={deleteMut.isPending}
                >
                  <Trash2 className="text-destructive" />
                </Button>
              </li>
            ))}
          </ul>
        )}

        {app ? (
          <div className="pointer-events-none absolute bottom-4 right-6 text-xs text-muted-foreground">
            {app.name} v{app.version} · Tauri v{app.tauri_version}
          </div>
        ) : null}
      </div>
    </div>
  );
}
