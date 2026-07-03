import type { DeckSummary } from "@synapse/ipc-types";
import { cn } from "@/lib/utils";

export type StatsRange = "7d" | "1m" | "3m" | "1y" | "all";

export const RANGE_DAYS: Record<StatsRange, number | null> = {
  "7d": 7,
  "1m": 30,
  "3m": 90,
  "1y": 365,
  all: null,
};

const RANGE_LABELS: { value: StatsRange; label: string }[] = [
  { value: "7d", label: "7d" },
  { value: "1m", label: "1M" },
  { value: "3m", label: "3M" },
  { value: "1y", label: "1Y" },
  { value: "all", label: "All" },
];

function deckDepth(name: string): number {
  return name.split("::").length - 1;
}

function deckLabel(name: string): string {
  const parts = name.split("::");
  return parts[parts.length - 1];
}

export function StatsFilters({
  decks,
  deckId,
  range,
  onDeckChange,
  onRangeChange,
}: {
  decks: DeckSummary[];
  deckId: number | null;
  range: StatsRange;
  onDeckChange: (deckId: number | null) => void;
  onRangeChange: (range: StatsRange) => void;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <select
        value={deckId ?? "all"}
        onChange={(e) => onDeckChange(e.target.value === "all" ? null : Number(e.target.value))}
        className="h-9 rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <option value="all">All decks</option>
        {decks.map((d) => (
          <option key={d.id} value={d.id}>
            {"  ".repeat(deckDepth(d.name))}
            {deckLabel(d.name)}
          </option>
        ))}
      </select>

      <div className="flex items-center gap-1 rounded-md border border-border bg-secondary/50 p-1">
        {RANGE_LABELS.map(({ value, label }) => (
          <button
            key={value}
            type="button"
            onClick={() => onRangeChange(value)}
            className={cn(
              "rounded px-2.5 py-1 text-xs font-medium transition-colors",
              range === value
                ? "bg-card text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}
