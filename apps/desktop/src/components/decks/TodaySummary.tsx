import type { DeckSummary } from "@synapse/ipc-types";

/**
 * Glanceable "N new · M learning · K to review today" line for the deck
 * page header. Sums only top-level decks — counts are already rolled up
 * over each deck's subtree, so summing every row would double-count.
 */
export function TodaySummary({ decks }: { decks: DeckSummary[] }) {
  if (decks.length === 0) return "Your collection. Import an Anki deck, or create one to get started.";

  const topLevel = decks.filter((d) => d.parent_id === null);
  const totals = topLevel.reduce(
    (acc, d) => ({
      new: acc.new + d.new_count,
      learning: acc.learning + d.learning_count,
      review: acc.review + d.review_count,
    }),
    { new: 0, learning: 0, review: 0 },
  );
  const total = totals.new + totals.learning + totals.review;

  if (total === 0) {
    return (
      <span className="text-emerald-600 dark:text-emerald-400">
        All caught up for today — nice work.
      </span>
    );
  }

  const parts: string[] = [];
  if (totals.new > 0) parts.push(`${totals.new} new`);
  if (totals.learning > 0) parts.push(`${totals.learning} learning`);
  if (totals.review > 0) parts.push(`${totals.review} to review`);
  return `${parts.join(" · ")} today.`;
}
