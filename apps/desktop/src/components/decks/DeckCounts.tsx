function CountBadge({
  count,
  color,
  columnWidth,
}: {
  count: number;
  color: string;
  columnWidth?: string;
}) {
  if (count === 0) {
    // In fixed-column mode (the deck list header lines up "New/Learn/Due"
    // above these), a hidden badge still reserves its column so the visible
    // ones don't drift out from under their label.
    return columnWidth ? <span className={`inline-block ${columnWidth}`} /> : null;
  }
  return (
    <span
      className={`rounded px-1.5 py-0.5 text-center text-xs font-semibold tabular-nums ${color} ${columnWidth ?? ""}`}
    >
      {count}
    </span>
  );
}

/**
 * The blue/amber/green new-learning-review badge triple shown for a deck or
 * card. Pass `columnWidth` (e.g. "w-9") when this sits under fixed column
 * headers (the deck list) so each badge reserves its slot even at zero;
 * omit it for the compact, self-collapsing form (session header).
 */
export function DeckCounts({
  newCount,
  learningCount,
  reviewCount,
  columnWidth,
}: {
  newCount: number;
  learningCount: number;
  reviewCount: number;
  columnWidth?: string;
}) {
  return (
    <span className="flex items-center gap-1">
      <CountBadge
        count={newCount}
        color="bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300"
        columnWidth={columnWidth}
      />
      <CountBadge
        count={learningCount}
        color="bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300"
        columnWidth={columnWidth}
      />
      <CountBadge
        count={reviewCount}
        color="bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300"
        columnWidth={columnWidth}
      />
    </span>
  );
}
