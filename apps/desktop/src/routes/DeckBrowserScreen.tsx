import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AnimatePresence, motion } from "framer-motion";
import {
  AlertTriangle,
  Download,
  Filter,
  Layers,
  Plus,
  RefreshCw,
  Settings,
  Trash2,
  Undo2,
  X,
} from "lucide-react";
import type { DeckSummary, FilteredDeckConfig, ImportSummary } from "@synapse/ipc-types";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { DeckOptionsDialog } from "@/components/DeckOptionsDialog";
import { errorMessage, ipc, isTauri, pickAndImportPackage } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";
import { dur, ease, listItem, scaleIn, staggerList } from "@/lib/motion";

function CountBadge({ count, color }: { count: number; color: string }) {
  if (count === 0) return null;
  return (
    <span className={`rounded px-1.5 py-0.5 text-xs font-semibold tabular-nums ${color}`}>
      {count}
    </span>
  );
}

function deckDepth(name: string): number {
  return name.split("::").length - 1;
}

function deckLabel(name: string): string {
  const parts = name.split("::");
  return parts[parts.length - 1];
}

// ── Filtered deck builder dialog ──────────────────────────────────────────────

const ORDER_LABELS = ["Random", "Due date (oldest)", "Added (oldest)", "Interval ↑", "Most lapses"];

function FilteredDeckDialog({
  initial,
  onClose,
  onSaved,
}: {
  initial?: FilteredDeckConfig;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [name, setName] = useState(initial?.name ?? "Custom Study");
  const [search, setSearch] = useState(initial?.search ?? "is:due");
  const [order, setOrder] = useState(initial?.order ?? 0);
  const [limit, setLimit] = useState(initial?.limit ?? 100);

  const createMut = useMutation({
    mutationFn: () => ipc.createFilteredDeck(name.trim(), search.trim(), order, limit),
    onSuccess: () => {
      onSaved();
      onClose();
    },
  });

  const rebuildMut = useMutation({
    mutationFn: () => ipc.rebuildFiltered(initial!.deck_id),
    onSuccess: () => {
      onSaved();
      onClose();
    },
  });

  const isRebuild = !!initial;
  const isPending = createMut.isPending || rebuildMut.isPending;
  const error = createMut.error ?? rebuildMut.error;

  return (
    <div
      className="absolute inset-0 z-50 flex items-center justify-center bg-background/60 backdrop-blur-sm"
      onClick={onClose}
    >
      <motion.div
        variants={scaleIn}
        initial="hidden"
        animate="show"
        exit="exit"
        className="mx-4 w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-5 flex items-center justify-between">
          <h2 className="text-base font-semibold">
            {isRebuild ? "Rebuild Filtered Deck" : "New Filtered Deck"}
          </h2>
          <Button variant="ghost" size="icon" onClick={onClose}>
            <X className="size-4" />
          </Button>
        </div>

        <div className="space-y-4">
          {!isRebuild && (
            <div className="space-y-1.5">
              <label className="text-sm font-medium">Name</label>
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              />
            </div>
          )}

          <div className="space-y-1.5">
            <label className="text-sm font-medium">Search query</label>
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="is:due deck:Spanish"
              className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm font-mono outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
            <p className="text-xs text-muted-foreground">
              Same syntax as the browser search (is:due, deck:, tag:, prop:, etc.)
            </p>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <label className="text-sm font-medium">Order</label>
              <select
                value={order}
                onChange={(e) => setOrder(Number(e.target.value))}
                className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                {ORDER_LABELS.map((label, i) => (
                  <option key={i} value={i}>
                    {label}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-1.5">
              <label className="text-sm font-medium">Limit</label>
              <input
                type="number"
                min={1}
                max={9999}
                value={limit}
                onChange={(e) => setLimit(Math.max(1, Math.min(9999, Number(e.target.value))))}
                className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              />
            </div>
          </div>
        </div>

        {error && <p className="mt-3 text-sm text-destructive">{errorMessage(error)}</p>}

        <div className="mt-5 flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Cancel
          </Button>
          <Button
            size="sm"
            disabled={isPending || !search.trim() || (!isRebuild && !name.trim())}
            onClick={() => (isRebuild ? rebuildMut.mutate() : createMut.mutate())}
          >
            <RefreshCw className="size-3.5" />
            {isPending ? "Building…" : isRebuild ? "Rebuild" : "Create"}
          </Button>
        </div>
      </motion.div>
    </div>
  );
}

// ── Main screen ───────────────────────────────────────────────────────────────

export function DeckBrowserScreen() {
  const queryClient = useQueryClient();
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
  });

  const invalidateDecks = () => queryClient.invalidateQueries({ queryKey: queryKeys.decks });

  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [lastImport, setLastImport] = useState<ImportSummary | null>(null);
  const [pendingDelete, setPendingDelete] = useState<DeckSummary | null>(null);
  const [optionsDeck, setOptionsDeck] = useState<DeckSummary | null>(null);
  const [showFilteredDialog, setShowFilteredDialog] = useState(false);
  const [rebuildTarget, setRebuildTarget] = useState<FilteredDeckConfig | null>(null);

  const importMut = useMutation({
    mutationFn: pickAndImportPackage,
    onSuccess: (summary) => {
      if (summary) {
        setLastImport(summary);
        void invalidateDecks();
      }
    },
  });

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

  const emptyMut = useMutation({
    mutationFn: (id: number) => ipc.emptyFiltered(id),
    onSuccess: () => void invalidateDecks(),
  });

  const undoMut = useMutation({
    mutationFn: () => ipc.undo(),
    onSuccess: () => void invalidateDecks(),
  });

  const decks = decksQuery.data ?? [];

  async function openRebuild(deck: DeckSummary) {
    const cfg = await ipc.getFilteredConfig(deck.id);
    if (cfg) setRebuildTarget(cfg);
  }

  return (
    <div className="relative flex h-full flex-col">
      <ScreenHeader
        title="Decks"
        description="Your collection. Import an Anki deck, or create one to get started."
        actions={
          <>
            <Button
              variant="outline"
              onClick={() => importMut.mutate()}
              disabled={!tauri || importMut.isPending}
              title="Import an Anki .apkg / .colpkg"
            >
              <Download /> {importMut.isPending ? "Importing…" : "Import"}
            </Button>
            <Button
              variant="outline"
              onClick={() => undoMut.mutate()}
              disabled={!tauri || undoMut.isPending}
              title="Undo the last change"
            >
              <Undo2 /> Undo
            </Button>
            <Button
              variant="outline"
              onClick={() => setShowFilteredDialog(true)}
              disabled={!tauri}
              title="Create a filtered (custom study) deck"
            >
              <Filter /> Filtered
            </Button>
            <Button onClick={() => setCreating((value) => !value)} disabled={!tauri}>
              <Plus /> New deck
            </Button>
          </>
        }
      />

      <div className="relative flex-1 overflow-auto">
        <AnimatePresence initial={false}>
          {creating && (
            <motion.form
              key="create-form"
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              transition={{ duration: dur.base, ease }}
              className="flex items-center gap-2 border-b border-border bg-secondary/40 px-8 py-3 overflow-hidden"
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
            </motion.form>
          )}
        </AnimatePresence>

        {createMut.isError ? (
          <p className="px-8 py-2 text-sm text-destructive">{errorMessage(createMut.error)}</p>
        ) : null}
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
            description="Create a deck above, or import an .apkg / .colpkg with the Import button."
          />
        ) : (
          <motion.ul
            className="divide-y divide-border"
            variants={staggerList}
            initial="hidden"
            animate="show"
          >
            <AnimatePresence>
              {decks.map((deck: DeckSummary) => (
                <motion.li
                  key={deck.id}
                  variants={listItem}
                  exit={{ opacity: 0, x: -16, transition: { duration: dur.fast } }}
                  className="group flex items-center gap-3 px-8 py-3 hover:bg-accent/40"
                  style={{ paddingLeft: `${2 + deckDepth(deck.name) * 1.25}rem` }}
                >
                  {deck.is_filtered ? (
                    <Filter className="size-4 shrink-0 text-purple-500" />
                  ) : (
                    <Layers className="size-4 shrink-0 text-muted-foreground" />
                  )}
                  <span className="flex-1 truncate text-sm font-medium">
                    {deckLabel(deck.name)}
                  </span>
                  {deck.is_filtered && (
                    <span className="rounded bg-purple-100 px-1.5 py-0.5 text-xs font-medium text-purple-700 dark:bg-purple-900/30 dark:text-purple-300">
                      filtered
                    </span>
                  )}
                  <span className="flex items-center gap-1">
                    <CountBadge
                      count={deck.new_count}
                      color="bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300"
                    />
                    <CountBadge
                      count={deck.learning_count}
                      color="bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300"
                    />
                    <CountBadge
                      count={deck.review_count}
                      color="bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300"
                    />
                  </span>
                  {deck.is_filtered && (
                    <>
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={`Rebuild ${deck.name}`}
                        className="opacity-0 transition-opacity group-hover:opacity-100"
                        onClick={() => void openRebuild(deck)}
                        title="Rebuild filtered deck"
                      >
                        <RefreshCw className="size-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={`Empty ${deck.name}`}
                        className="opacity-0 transition-opacity group-hover:opacity-100"
                        onClick={() => emptyMut.mutate(deck.id)}
                        disabled={emptyMut.isPending}
                        title="Return all cards to original decks"
                      >
                        <X className="size-4" />
                      </Button>
                    </>
                  )}
                  {!deck.is_filtered && (
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label={`Settings for ${deck.name}`}
                      className="opacity-0 transition-opacity group-hover:opacity-100"
                      onClick={() => setOptionsDeck(deck)}
                    >
                      <Settings className="size-4" />
                    </Button>
                  )}
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={`Delete ${deck.name}`}
                    className="opacity-0 transition-opacity group-hover:opacity-100"
                    onClick={() => setPendingDelete(deck)}
                    disabled={deleteMut.isPending}
                  >
                    <Trash2 className="text-destructive" />
                  </Button>
                </motion.li>
              ))}
            </AnimatePresence>
          </motion.ul>
        )}
      </div>

      {/* Delete confirmation dialog */}
      <AnimatePresence>
        {pendingDelete && (
          <motion.div
            key="delete-backdrop"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: dur.fast }}
            className="absolute inset-0 z-50 flex items-center justify-center bg-background/60 backdrop-blur-sm"
            onClick={() => setPendingDelete(null)}
          >
            <motion.div
              variants={scaleIn}
              initial="hidden"
              animate="show"
              exit="exit"
              className="mx-4 w-full max-w-sm rounded-xl border border-border bg-card p-6 shadow-2xl"
              onClick={(e) => e.stopPropagation()}
            >
              <div className="mb-4 flex items-center gap-3">
                <span className="flex size-10 shrink-0 items-center justify-center rounded-full bg-destructive/10">
                  <AlertTriangle className="size-5 text-destructive" />
                </span>
                <div>
                  <p className="text-sm font-semibold">Delete deck?</p>
                  <p className="text-xs text-muted-foreground">
                    {pendingDelete.is_filtered
                      ? "Cards will be returned to their original decks first."
                      : "This cannot be undone."}
                  </p>
                </div>
              </div>
              <p className="mb-6 rounded-md bg-secondary/60 px-3 py-2 text-sm font-medium">
                {pendingDelete.name}
              </p>
              <div className="flex justify-end gap-2">
                <Button variant="ghost" size="sm" onClick={() => setPendingDelete(null)}>
                  Cancel
                </Button>
                <Button
                  size="sm"
                  className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                  disabled={deleteMut.isPending}
                  onClick={() => {
                    deleteMut.mutate(pendingDelete.id);
                    setPendingDelete(null);
                  }}
                >
                  <Trash2 className="size-3.5" />
                  Delete
                </Button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

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
