import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { PlusCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ipc } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";

/**
 * Inline "study N more new cards today" form. Persists via
 * `ipc.increaseTodayLimit` and refreshes the deck list badge before calling
 * `onDone`. This only raises `deckId`'s own limit — for a deck studied via
 * its subtree, subdecks each keep their own daily cap.
 */
export function IncreaseLimitControl({ deckId, onDone }: { deckId: number; onDone: () => void }) {
  const queryClient = useQueryClient();
  const [extra, setExtra] = useState(10);

  const increaseMut = useMutation({
    mutationFn: (extraNew: number) => ipc.increaseTodayLimit(deckId, extraNew),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.decks });
      onDone();
    },
  });

  return (
    <div className="flex items-center gap-2 rounded-lg bg-secondary/40 px-3 py-2 text-sm">
      <span className="text-muted-foreground">Study</span>
      <input
        type="number"
        min={1}
        max={9999}
        value={extra}
        onChange={(e) => setExtra(Math.max(1, Number(e.target.value)))}
        className="h-7 w-16 rounded-md border border-input bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
        autoFocus
      />
      <span className="text-muted-foreground">more new cards today</span>
      <Button
        size="sm"
        className="h-7"
        disabled={increaseMut.isPending}
        onClick={() => increaseMut.mutate(extra)}
      >
        Add
      </Button>
    </div>
  );
}

/** Toggle button + inline form; used at the "all done" screen once a session hits the day's cap. */
export function ExtendTodayLimit({ deckId, onDone }: { deckId: number; onDone: () => void }) {
  const [open, setOpen] = useState(false);

  if (!open) {
    return (
      <Button
        variant="ghost"
        size="sm"
        className="gap-1.5 text-xs text-muted-foreground"
        onClick={() => setOpen(true)}
      >
        <PlusCircle className="size-3.5" />
        Increase today's new card limit
      </Button>
    );
  }

  return (
    <IncreaseLimitControl
      deckId={deckId}
      onDone={() => {
        setOpen(false);
        onDone();
      }}
    />
  );
}
