import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { motion } from "framer-motion";
import { RefreshCw, X } from "lucide-react";
import type { FilteredDeckConfig } from "@synapse/ipc-types";
import { Button } from "@/components/ui/button";
import { errorMessage, ipc } from "@/lib/ipc";
import { scaleIn } from "@/lib/motion";

const ORDER_LABELS = ["Random", "Due date (oldest)", "Added (oldest)", "Interval ↑", "Most lapses"];

/** Create or rebuild a filtered (custom study) deck. */
export function FilteredDeckDialog({
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
