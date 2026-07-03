import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "framer-motion";
import { Download, Filter, LayoutGrid, Layers, List, Plus, Sparkles, Undo2 } from "lucide-react";
import type {
  DeckSummary,
  FilteredDeckConfig,
  ImportProgress,
  ImportSummary,
} from "@synapse/ipc-types";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { DeckOptionsDialog } from "@/components/DeckOptionsDialog";
import { CreateDeckForm } from "@/components/decks/CreateDeckForm";
import { DeckRow } from "@/components/decks/DeckRow";
import { DeckGridCard } from "@/components/decks/DeckGridCard";
import { HomeHero } from "@/components/decks/HomeHero";
import { DeleteDeckDialog } from "@/components/decks/DeleteDeckDialog";
import { FilteredDeckDialog } from "@/components/decks/FilteredDeckDialog";
import { TodaySummary } from "@/components/decks/TodaySummary";
import { errorMessage, ipc, isTauri, pickAndImportPackage } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";
import { listItem, staggerList } from "@/lib/motion";
import { useUi, type HomeLayout } from "@/stores/ui";
import { cn } from "@/lib/utils";

const LAYOUT_OPTIONS: { value: HomeLayout; icon: typeof List; label: string }[] = [
  { value: "list", icon: List, label: "List" },
  { value: "grid", icon: LayoutGrid, label: "Grid" },
  { value: "hero", icon: Sparkles, label: "List + hero" },
];

function LayoutSwitcher({
  value,
  onChange,
}: {
  value: HomeLayout;
  onChange: (v: HomeLayout) => void;
}) {
  return (
    <div className="flex items-center gap-0.5 rounded-md border border-border bg-secondary/50 p-0.5">
      {LAYOUT_OPTIONS.map(({ value: v, icon: Icon, label }) => (
        <button
          key={v}
          type="button"
          title={label}
          aria-label={label}
          aria-pressed={value === v}
          onClick={() => onChange(v)}
          className={cn(
            "flex h-6 w-6 items-center justify-center rounded transition-colors",
            value === v
              ? "bg-card text-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          <Icon className="size-3.5" />
        </button>
      ))}
    </div>
  );
}

export function DeckBrowserScreen() {
  const queryClient = useQueryClient();
  const navigate = useNavigate({ from: "/" });
  const { create } = useSearch({ from: "/" });
  const tauri = isTauri();

  useQuery({
    queryKey: queryKeys.appInfo,
    queryFn: ipc.appInfo,
    enabled: tauri,
  });

  const decksQuery = useQuery({
    queryKey: queryKeys.decks,
    queryFn: ipc.listDecks,
    enabled: tauri,
    // Backend "today" rolls over independently of user activity, and cache
    // invalidation elsewhere only fires on explicit mutation events — poll so
    // new-card counts don't stay frozen if the deck browser is left open
    // across a day boundary.
    refetchInterval: 60_000,
  });

  const invalidateDecks = () => queryClient.invalidateQueries({ queryKey: queryKeys.decks });

  const [creating, setCreating] = useState(false);
  const [lastImport, setLastImport] = useState<ImportSummary | null>(null);
  const [pendingDelete, setPendingDelete] = useState<DeckSummary | null>(null);
  const [optionsDeck, setOptionsDeck] = useState<DeckSummary | null>(null);
  const [showFilteredDialog, setShowFilteredDialog] = useState(false);
  const [rebuildTarget, setRebuildTarget] = useState<FilteredDeckConfig | null>(null);
  const [importProgress, setImportProgress] = useState<ImportProgress | null>(null);

  // CommandPalette's "New deck" lands here with `?create=true`; open the form
  // once, then clear the search param so it doesn't reopen on back/refresh.
  useEffect(() => {
    if (create) {
      setCreating(true);
      void navigate({ search: {}, replace: true });
    }
  }, [create, navigate]);

  useEffect(() => {
    if (!tauri) return;
    const unlisten = listen<ImportProgress>("synapse://import-progress", ({ payload }) => {
      setImportProgress(payload);
    });
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, [tauri]);

  const importMut = useMutation({
    mutationFn: () => {
      setImportProgress(null);
      return pickAndImportPackage();
    },
    onSuccess: (summary) => {
      if (summary) {
        setLastImport(summary);
        void invalidateDecks();
      }
    },
    onSettled: () => setImportProgress(null),
  });

  const deleteMut = useMutation({
    mutationFn: (id: number) => ipc.deleteDeck(id),
    onSuccess: () => void invalidateDecks(),
  });

  const renameMut = useMutation({
    mutationFn: ({ id, name }: { id: number; name: string }) => ipc.renameDeck(id, name),
    onSuccess: () => void invalidateDecks(),
  });

  const emptyMut = useMutation({
    mutationFn: (id: number) => ipc.emptyFiltered(id),
    onSuccess: () => void invalidateDecks(),
  });

  const undoMut = useMutation({
    mutationFn: () => ipc.undo(),
    onSuccess: () => void invalidateDecks(),
  });

  const decks = decksQuery.data ?? [];
  const homeLayout = useUi((s) => s.homeLayout);
  const setHomeLayout = useUi((s) => s.setHomeLayout);

  async function openRebuild(deck: DeckSummary) {
    const cfg = await ipc.getFilteredConfig(deck.id);
    if (cfg) setRebuildTarget(cfg);
  }

  function studyDeck(deck: DeckSummary) {
    void navigate({ to: "/study/$deckId", params: { deckId: deck.id } });
  }

  return (
    <div className="relative flex h-full flex-col">
      <ScreenHeader
        title="Decks"
        description={<TodaySummary decks={decks} />}
        actions={
          <>
            <Button
              variant="outline"
              size="sm"
              onClick={() => importMut.mutate()}
              disabled={!tauri || importMut.isPending}
              title="Import an Anki .apkg / .colpkg"
            >
              <Download />{" "}
              {importMut.isPending
                ? importProgress && importProgress.total > 0
                  ? `Importing… ${importProgress.done}/${importProgress.total}`
                  : "Importing…"
                : "Import"}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => undoMut.mutate()}
              disabled={!tauri || undoMut.isPending}
              title="Undo the last change"
            >
              <Undo2 /> Undo
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setShowFilteredDialog(true)}
              disabled={!tauri}
              title="Create a filtered (custom study) deck"
            >
              <Filter /> Filtered
            </Button>
            <Button size="sm" onClick={() => setCreating((value) => !value)} disabled={!tauri}>
              <Plus /> New deck
            </Button>
            <LayoutSwitcher value={homeLayout} onChange={setHomeLayout} />
          </>
        }
      />

      <div className="relative flex-1 overflow-auto">
        <AnimatePresence initial={false}>
          {creating && <CreateDeckForm onClose={() => setCreating(false)} />}
        </AnimatePresence>

        {tauri && homeLayout === "hero" && decks.length > 0 && (
          <HomeHero decks={decks} onStudy={studyDeck} />
        )}

        {importMut.isError ? (
          <p className="px-8 py-2 text-sm text-destructive">
            Import failed: {errorMessage(importMut.error)}
          </p>
        ) : null}
        {deleteMut.isError ? (
          <p className="px-8 py-2 text-sm text-destructive">
            Delete failed: {errorMessage(deleteMut.error)}
          </p>
        ) : null}
        {renameMut.isError ? (
          <p className="px-8 py-2 text-sm text-destructive">
            Rename failed: {errorMessage(renameMut.error)}
          </p>
        ) : null}

        {lastImport ? (
          <p className="border-b border-border bg-secondary/40 px-8 py-2 text-sm text-muted-foreground">
            Imported {lastImport.notes_added} notes, {lastImport.cards_added} cards,{" "}
            {lastImport.decks_added} decks, {lastImport.media_imported} media
            {lastImport.notes_updated > 0 ? ` · ${lastImport.notes_updated} updated` : ""}.
          </p>
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
            description="Start a new deck, or bring in an existing Anki collection."
            action={
              <div className="flex items-center gap-2">
                <Button size="sm" onClick={() => setCreating(true)}>
                  <Plus /> New deck
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => importMut.mutate()}
                  disabled={importMut.isPending}
                >
                  <Download /> Import from Anki
                </Button>
              </div>
            }
          />
        ) : homeLayout === "grid" ? (
          <motion.div
            variants={staggerList}
            initial="hidden"
            animate="show"
            className="grid grid-cols-1 gap-3 p-8 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4"
          >
            <AnimatePresence>
              {decks.map((deck: DeckSummary) => (
                <motion.div key={deck.id} variants={listItem} exit={{ opacity: 0, scale: 0.96 }}>
                  <DeckGridCard
                    deck={deck}
                    onStudy={() => studyDeck(deck)}
                    onRename={(name) => renameMut.mutate({ id: deck.id, name })}
                    onOptions={() => setOptionsDeck(deck)}
                    onDelete={() => setPendingDelete(deck)}
                    onRebuild={() => void openRebuild(deck)}
                    onEmpty={() => emptyMut.mutate(deck.id)}
                  />
                </motion.div>
              ))}
            </AnimatePresence>
          </motion.div>
        ) : (
          <>
            <div className="flex items-center justify-end gap-1 px-8 pb-1 pt-3 text-xs font-medium text-muted-foreground">
              <span className="w-9 text-center">New</span>
              <span className="w-9 text-center">Learn</span>
              <span className="w-9 text-center">Due</span>
              <span className="w-36" />
            </div>
            <motion.ul variants={staggerList} initial="hidden" animate="show">
              <AnimatePresence>
                {decks.map((deck: DeckSummary) => (
                  <motion.li key={deck.id} variants={listItem} exit={{ opacity: 0, x: -16 }}>
                    <DeckRow
                      deck={deck}
                      onStudy={() => studyDeck(deck)}
                      onRename={(name) => renameMut.mutate({ id: deck.id, name })}
                      onOptions={() => setOptionsDeck(deck)}
                      onDelete={() => setPendingDelete(deck)}
                      onRebuild={() => void openRebuild(deck)}
                      onEmpty={() => emptyMut.mutate(deck.id)}
                    />
                  </motion.li>
                ))}
              </AnimatePresence>
            </motion.ul>
          </>
        )}
      </div>

      <DeleteDeckDialog
        deck={pendingDelete}
        onCancel={() => setPendingDelete(null)}
        isPending={deleteMut.isPending}
        onConfirm={() => {
          if (!pendingDelete) return;
          deleteMut.mutate(pendingDelete.id);
          setPendingDelete(null);
        }}
      />

      <AnimatePresence>
        {optionsDeck && (
          <DeckOptionsDialog
            key={optionsDeck.id}
            deckId={optionsDeck.id}
            deckName={optionsDeck.name}
            onClose={() => setOptionsDeck(null)}
            onSaved={() => void invalidateDecks()}
          />
        )}
      </AnimatePresence>

      <AnimatePresence>
        {showFilteredDialog && (
          <FilteredDeckDialog
            key="filtered-new"
            onClose={() => setShowFilteredDialog(false)}
            onSaved={() => void invalidateDecks()}
          />
        )}
      </AnimatePresence>

      <AnimatePresence>
        {rebuildTarget && (
          <FilteredDeckDialog
            key={rebuildTarget.deck_id}
            initial={rebuildTarget}
            onClose={() => setRebuildTarget(null)}
            onSaved={() => void invalidateDecks()}
          />
        )}
      </AnimatePresence>
    </div>
  );
}
