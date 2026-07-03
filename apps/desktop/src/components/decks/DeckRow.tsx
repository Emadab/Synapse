import { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Filter, Layers, Play, RefreshCw, Settings, X } from "lucide-react";
import type { DeckSummary } from "@synapse/ipc-types";
import { Button } from "@/components/ui/button";
import { dur, ease } from "@/lib/motion";
import { DeckCounts } from "./DeckCounts";
import { DeckActionsMenu } from "./DeckActionsMenu";
import { IncreaseLimitControl } from "./IncreaseLimitControl";

function deckDepth(name: string): number {
  return name.split("::").length - 1;
}

function deckLabel(name: string): string {
  const parts = name.split("::");
  return parts[parts.length - 1];
}

export function DeckRow({
  deck,
  onStudy,
  onRename,
  onOptions,
  onDelete,
  onRebuild,
  onEmpty,
}: {
  deck: DeckSummary;
  onStudy: () => void;
  onRename: (newName: string) => void;
  onOptions: () => void;
  onDelete: () => void;
  onRebuild: () => void;
  onEmpty: () => void;
}) {
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(deck.name);
  const [limitOpen, setLimitOpen] = useState(false);

  const depth = Math.min(deckDepth(deck.name), 6);

  function submitRename() {
    const trimmed = renameValue.trim();
    setRenaming(false);
    if (trimmed && trimmed !== deck.name) onRename(trimmed);
    else setRenameValue(deck.name);
  }

  return (
    <div className="border-b border-border last:border-0">
      <div
        className="group flex items-center gap-3 px-8 py-3 hover:bg-accent/40"
        style={{ paddingLeft: `${2 + depth * 1.25}rem` }}
      >
        <button
          className="flex flex-1 items-center gap-3 overflow-hidden text-left"
          onClick={() => !renaming && onStudy()}
        >
          {deck.is_filtered ? (
            <Filter className="size-4 shrink-0 text-purple-500" />
          ) : (
            <Layers className="size-4 shrink-0 text-muted-foreground" />
          )}
          {renaming ? (
            <input
              autoFocus
              value={renameValue}
              onClick={(e) => e.stopPropagation()}
              onChange={(e) => setRenameValue(e.target.value)}
              onBlur={submitRename}
              onKeyDown={(e) => {
                if (e.key === "Enter") submitRename();
                if (e.key === "Escape") {
                  setRenameValue(deck.name);
                  setRenaming(false);
                }
              }}
              className="h-7 flex-1 rounded-md border border-input bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
          ) : (
            <span className="flex-1 truncate text-sm font-medium">{deckLabel(deck.name)}</span>
          )}
          {deck.is_filtered && !renaming && (
            <span className="shrink-0 rounded bg-purple-100 px-1.5 py-0.5 text-xs font-medium text-purple-700 dark:bg-purple-900/30 dark:text-purple-300">
              filtered
            </span>
          )}
        </button>

        <DeckCounts
          newCount={deck.new_count}
          learningCount={deck.learning_count}
          reviewCount={deck.review_count}
          columnWidth="w-9"
        />

        <div className="flex w-36 shrink-0 items-center justify-end gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
          <Button
            variant="ghost"
            size="icon"
            aria-label={`Study ${deck.name}`}
            onClick={(e) => {
              e.stopPropagation();
              onStudy();
            }}
            title="Study"
          >
            <Play className="size-4" />
          </Button>
          {deck.is_filtered ? (
            <>
              <Button
                variant="ghost"
                size="icon"
                aria-label={`Rebuild ${deck.name}`}
                onClick={(e) => {
                  e.stopPropagation();
                  onRebuild();
                }}
                title="Rebuild filtered deck"
              >
                <RefreshCw className="size-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                aria-label={`Empty ${deck.name}`}
                onClick={(e) => {
                  e.stopPropagation();
                  onEmpty();
                }}
                title="Return all cards to original decks"
              >
                <X className="size-4" />
              </Button>
            </>
          ) : (
            <Button
              variant="ghost"
              size="icon"
              aria-label={`Settings for ${deck.name}`}
              onClick={(e) => {
                e.stopPropagation();
                onOptions();
              }}
              title="Deck options"
            >
              <Settings className="size-4" />
            </Button>
          )}
          <DeckActionsMenu
            deck={deck}
            onStudy={onStudy}
            onRename={() => setRenaming(true)}
            onIncreaseLimit={() => setLimitOpen((o) => !o)}
            onOptions={onOptions}
            onDelete={onDelete}
            onRebuild={onRebuild}
            onEmpty={onEmpty}
          />
        </div>
      </div>

      <AnimatePresence initial={false}>
        {limitOpen && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: dur.fast, ease }}
            className="overflow-hidden"
            style={{ paddingLeft: `${2 + depth * 1.25}rem` }}
          >
            <div className="px-8 py-3">
              <IncreaseLimitControl deckId={deck.id} onDone={() => setLimitOpen(false)} />
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
