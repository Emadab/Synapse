import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { motion } from "framer-motion";
import { Button } from "@/components/ui/button";
import { errorMessage, ipc } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";
import { dur, ease } from "@/lib/motion";

/** Inline `::`-aware create-deck form shown above the deck tree. */
export function CreateDeckForm({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [name, setName] = useState("");

  const createMut = useMutation({
    mutationFn: (deckName: string) => ipc.createDeck(deckName),
    onSuccess: () => {
      setName("");
      onClose();
      void queryClient.invalidateQueries({ queryKey: queryKeys.decks });
    },
  });

  return (
    <>
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
            onClose();
            setName("");
          }}
        >
          Cancel
        </Button>
      </motion.form>
      {createMut.isError ? (
        <p className="px-8 py-2 text-sm text-destructive">{errorMessage(createMut.error)}</p>
      ) : null}
    </>
  );
}
