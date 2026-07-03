import { useState } from "react";
import { Filter, Layers, Play } from "lucide-react";
import type { DeckSummary } from "@synapse/ipc-types";
import { Button } from "@/components/ui/button";
import { DeckCounts } from "./DeckCounts";
import { DeckActionsMenu } from "./DeckActionsMenu";
import { IncreaseLimitControl } from "./IncreaseLimitControl";

function deckLabel(name: string): string {
  const parts = name.split("::");
  return parts[parts.length - 1];
}

/** Ring showing today's queue composition (new / learning / review share). */
function QueueRing({ deck }: { deck: DeckSummary }) {
  const total = deck.new_count + deck.learning_count + deck.review_count;
  const r = 15;
  const c = 2 * Math.PI * r;
  if (total === 0) {
    return (
      <svg width={36} height={36} viewBox="0 0 36 36" className="shrink-0">
        <circle cx={18} cy={18} r={r} fill="none" stroke="hsl(var(--secondary))" strokeWidth={4} />
      </svg>
    );
  }
  const segs = [
    { count: deck.new_count, color: "hsl(213 68% 50%)" },
    { count: deck.learning_count, color: "hsl(38 92% 50%)" },
    { count: deck.review_count, color: "hsl(142 71% 45%)" },
  ];
  let offset = 0;
  return (
    <svg width={36} height={36} viewBox="0 0 36 36" className="shrink-0 -rotate-90">
      <circle cx={18} cy={18} r={r} fill="none" stroke="hsl(var(--secondary))" strokeWidth={4} />
      {segs.map((s, i) => {
        const frac = s.count / total;
        const dash = frac * c;
        const el = (
          <circle
            key={i}
            cx={18}
            cy={18}
            r={r}
            fill="none"
            stroke={s.color}
            strokeWidth={4}
            strokeDasharray={`${dash} ${c - dash}`}
            strokeDashoffset={-offset}
            strokeLinecap="butt"
          />
        );
        offset += dash;
        return el;
      })}
    </svg>
  );
}

export function DeckGridCard({
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
  const total = deck.new_count + deck.learning_count + deck.review_count;

  function submitRename() {
    const trimmed = renameValue.trim();
    setRenaming(false);
    if (trimmed && trimmed !== deck.name) onRename(trimmed);
    else setRenameValue(deck.name);
  }

  return (
    <div className="group flex flex-col gap-3 rounded-xl border border-border bg-card p-4 transition-colors hover:border-primary/40">
      <div className="flex items-start justify-between gap-2">
        <div className="flex min-w-0 flex-1 items-center gap-2">
          {deck.is_filtered ? (
            <Filter className="size-4 shrink-0 text-purple-500" />
          ) : (
            <Layers className="size-4 shrink-0 text-muted-foreground" />
          )}
          {renaming ? (
            <input
              autoFocus
              value={renameValue}
              onChange={(e) => setRenameValue(e.target.value)}
              onBlur={submitRename}
              onKeyDown={(e) => {
                if (e.key === "Enter") submitRename();
                if (e.key === "Escape") {
                  setRenameValue(deck.name);
                  setRenaming(false);
                }
              }}
              className="h-7 min-w-0 flex-1 rounded-md border border-input bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
          ) : (
            <span className="truncate text-sm font-medium">{deckLabel(deck.name)}</span>
          )}
        </div>
        <div className="opacity-0 transition-opacity group-hover:opacity-100">
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

      <div className="flex items-center justify-between gap-3">
        <QueueRing deck={deck} />
        <DeckCounts
          newCount={deck.new_count}
          learningCount={deck.learning_count}
          reviewCount={deck.review_count}
        />
      </div>

      {limitOpen && (
        <IncreaseLimitControl deckId={deck.id} onDone={() => setLimitOpen(false)} />
      )}

      <Button size="sm" variant={total > 0 ? "default" : "outline"} onClick={onStudy}>
        <Play className="size-3.5" /> Study
      </Button>
    </div>
  );
}
