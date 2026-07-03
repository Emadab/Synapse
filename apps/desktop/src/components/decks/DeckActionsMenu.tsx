import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { MoreHorizontal, Pencil, Play, PlusCircle, RefreshCw, Settings, Trash2, X } from "lucide-react";
import type { DeckSummary } from "@synapse/ipc-types";
import { dur, ease } from "@/lib/motion";

function MenuItem({
  icon,
  label,
  onClick,
  danger,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      role="menuitem"
      className={`flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-accent [&_svg]:size-4 ${
        danger ? "text-destructive [&_svg]:text-destructive" : "[&_svg]:text-muted-foreground"
      }`}
      onClick={onClick}
    >
      {icon}
      {label}
    </button>
  );
}

/** Overflow menu for a deck row: study, rename, limit, options, delete (and
 * rebuild/empty for filtered decks). */
export function DeckActionsMenu({
  deck,
  onStudy,
  onRename,
  onIncreaseLimit,
  onOptions,
  onDelete,
  onRebuild,
  onEmpty,
}: {
  deck: DeckSummary;
  onStudy: () => void;
  onRename: () => void;
  onIncreaseLimit: () => void;
  onOptions: () => void;
  onDelete: () => void;
  onRebuild: () => void;
  onEmpty: () => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    if (open) document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const pick = (fn: () => void) => () => {
    setOpen(false);
    fn();
  };

  return (
    <div className="relative" ref={ref} onClick={(e) => e.stopPropagation()}>
      <button
        className="rounded p-1.5 text-muted-foreground transition-opacity hover:bg-secondary hover:text-foreground"
        aria-label={`More actions for ${deck.name}`}
        onClick={() => setOpen((o) => !o)}
      >
        <MoreHorizontal className="size-4" />
      </button>
      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ opacity: 0, scale: 0.95, y: 4 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 4 }}
            transition={{ duration: dur.fast, ease }}
            role="menu"
            aria-label={`Actions for ${deck.name}`}
            className="absolute right-0 top-full z-20 mt-1 w-60 overflow-hidden rounded-lg border border-border bg-popover py-1 shadow-md"
          >
            <MenuItem icon={<Play />} label="Study" onClick={pick(onStudy)} />
            {!deck.is_filtered && (
              <MenuItem icon={<Pencil />} label="Rename" onClick={pick(onRename)} />
            )}
            {!deck.is_filtered && (
              <MenuItem
                icon={<PlusCircle />}
                label="Increase today's new limit"
                onClick={pick(onIncreaseLimit)}
              />
            )}
            {!deck.is_filtered && (
              <MenuItem icon={<Settings />} label="Deck options" onClick={pick(onOptions)} />
            )}
            {deck.is_filtered && (
              <MenuItem icon={<RefreshCw />} label="Rebuild" onClick={pick(onRebuild)} />
            )}
            {deck.is_filtered && <MenuItem icon={<X />} label="Empty" onClick={pick(onEmpty)} />}
            <div className="my-1 border-t border-border" />
            <MenuItem icon={<Trash2 />} label="Delete" onClick={pick(onDelete)} danger />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
