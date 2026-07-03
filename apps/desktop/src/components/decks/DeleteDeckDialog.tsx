import { AnimatePresence, motion } from "framer-motion";
import { AlertTriangle, Trash2 } from "lucide-react";
import type { DeckSummary } from "@synapse/ipc-types";
import { Button } from "@/components/ui/button";
import { dur, scaleIn } from "@/lib/motion";

export function DeleteDeckDialog({
  deck,
  onCancel,
  onConfirm,
  isPending,
}: {
  deck: DeckSummary | null;
  onCancel: () => void;
  onConfirm: () => void;
  isPending: boolean;
}) {
  return (
    <AnimatePresence>
      {deck && (
        <motion.div
          key="delete-backdrop"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: dur.fast }}
          className="absolute inset-0 z-50 flex items-center justify-center bg-background/60 backdrop-blur-sm"
          onClick={onCancel}
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
                  {deck.is_filtered
                    ? "Cards will be returned to their original decks first."
                    : "This cannot be undone."}
                </p>
              </div>
            </div>
            <p className="mb-6 rounded-md bg-secondary/60 px-3 py-2 text-sm font-medium">
              {deck.name}
            </p>
            <div className="flex justify-end gap-2">
              <Button variant="ghost" size="sm" onClick={onCancel}>
                Cancel
              </Button>
              <Button
                size="sm"
                className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                disabled={isPending}
                onClick={onConfirm}
              >
                <Trash2 className="size-3.5" />
                Delete
              </Button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
